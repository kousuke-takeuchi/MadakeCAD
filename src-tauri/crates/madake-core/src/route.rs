//! 配線の自動の引き直し (経路探索)。
//!
//! シートの配線を、接続を変えずに引き直すための計算。格子 ([`RouteGrid`]) の上で A* により
//! 直交の経路を探す。この模塊の関数は全て**純粋**で、図面 (モデル) を書き換えない。
//!
//! ネットリスト ([`crate::netlist`]) は「配線の線分上のどこにあっても」ピンを繋ぎ、「端点が一致する」
//! 配線どうしを繋ぐ。そのため経路は、他のネットのピン・ラベル・ジャンクションの上を通らず、
//! 他のネットの配線と端点を共有せず、同じ向きに重ならない (直角に横切るだけは許す)。

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use serde::{Deserialize, Serialize};

use crate::geometry::Point;
use crate::model::{Entity, EntityId, Orientation, Sheet};
use crate::netlist::{pin_positions, transform_local, CONNECT_EPS};
use crate::pin_wiring::world_dir;
use crate::symbol::{DefIndex, PinDir, SymbolDef};
use crate::tidy::{symbol_bounds, text_boxes, Rect, TextKind};

/// 経路探索の設定。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default)]
pub struct RouteOptions {
    /// 格子の間隔 (mm)。
    pub grid_mm: f64,
    /// 曲がり1回の費用 (mm換算)。
    pub bend_cost: f64,
    /// 他のネットを直角に横切る1回の費用 (mm換算)。
    pub cross_cost: f64,
    /// 他のネットの配線の隣を通る1歩の費用 (mm換算)。
    pub near_cost: f64,
    /// シンボルの外形と文字の枠をこの量 (mm) だけ広げて通れない領域にする。
    pub clearance_mm: f64,
    /// 図枠の余白 (mm)。
    pub frame_margin_mm: f64,
    /// 右下の表題欄の (幅, 高さ) (mm)。通れない領域にする。
    pub title_block_mm: [f64; 2],
}

impl Default for RouteOptions {
    fn default() -> Self {
        Self {
            grid_mm: 1.25,
            bend_cost: 6.0,
            cross_cost: 15.0,
            near_cost: 2.0,
            clearance_mm: 0.6,
            frame_margin_mm: 10.0,
            title_block_mm: [120.0, 32.0],
        }
    }
}

/// 4方向。用紙座標はYが下向きなので、Nはyが減る向き。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    N,
    E,
    S,
    W,
}

impl Dir {
    const ALL: [Dir; 4] = [Dir::N, Dir::E, Dir::S, Dir::W];

    fn idx(self) -> usize {
        match self {
            Dir::N => 0,
            Dir::E => 1,
            Dir::S => 2,
            Dir::W => 3,
        }
    }

    /// 向きを表すビット (N=1, E=2, S=4, W=8)。
    fn bit(self) -> u8 {
        1 << self.idx()
    }

    fn opp(self) -> Dir {
        match self {
            Dir::N => Dir::S,
            Dir::E => Dir::W,
            Dir::S => Dir::N,
            Dir::W => Dir::E,
        }
    }

    fn horizontal(self) -> bool {
        matches!(self, Dir::E | Dir::W)
    }

    fn delta(self) -> (i64, i64) {
        match self {
            Dir::N => (0, -1),
            Dir::E => (1, 0),
            Dir::S => (0, 1),
            Dir::W => (-1, 0),
        }
    }

    fn from_pin(d: PinDir) -> Dir {
        match d {
            PinDir::Up => Dir::N,
            PinDir::Down => Dir::S,
            PinDir::Left => Dir::W,
            PinDir::Right => Dir::E,
        }
    }

    /// 2点が軸に平行なとき、a から b へ向かう向き。
    fn between(a: Point, b: Point) -> Option<Dir> {
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        if dx.abs() >= dy.abs() && dx.abs() > 1e-9 {
            Some(if dx > 0.0 { Dir::E } else { Dir::W })
        } else if dy.abs() > 1e-9 {
            Some(if dy > 0.0 { Dir::S } else { Dir::N })
        } else {
            None
        }
    }
}

const NS: u8 = 0b0101;
const EW: u8 = 0b1010;
/// 持ち主のいない場所の印。
const NONE: u32 = u32::MAX;

/// シート上のピン1本 (用紙座標と、電線が出ていく向き)。
#[derive(Debug, Clone)]
pub struct PinPt {
    pub entity: EntityId,
    pub reference: String,
    pub number: String,
    pub pos: Point,
    pub dir: Dir,
}

/// シート上の全ピン。シンボルのentity id順で決定的。
pub fn pin_points(sheet: &Sheet, defs: &[SymbolDef]) -> Vec<PinPt> {
    let idx = DefIndex::new(defs);
    let mut out = Vec::new();
    for e in sheet.entities.values() {
        let Entity::Symbol(s) = e else { continue };
        let Some(def) = idx.get(s) else { continue };
        for (pin, (number, pos)) in def.pins.iter().zip(pin_positions(s, &def)) {
            out.push(PinPt {
                entity: s.id,
                reference: s.reference.clone(),
                number,
                pos,
                dir: Dir::from_pin(world_dir(pin.dir, s)),
            });
        }
    }
    out
}

/// 2点間の距離が許容誤差内か。
fn same(a: Point, b: Point) -> bool {
    a.distance_to(&b) < CONNECT_EPS
}

/// 経路探索の格子。通れない場所は動かないもの (シンボル・文字・図枠・ピンの位置) だけをここに持つ。
/// 他のネットが使う場所は [`Occ`] が持つ。
pub struct RouteGrid {
    pub(crate) opts: RouteOptions,
    pub(crate) nx: usize,
    pub(crate) ny: usize,
    /// シンボルの外形・文字の枠・図枠・表題欄の中の格子点。
    blocked: Vec<bool>,
    /// ピンやラベルがある格子点。
    pin_blocked: Vec<bool>,
    /// 格子点nからn+1 (東隣) への辺が外形に食い込むか・ピンの上か。
    h_blocked: Vec<bool>,
    /// 格子点nからn+nx (南隣) への辺。
    v_blocked: Vec<bool>,
    /// ピンとラベルの位置 (引き出し線が他のピンの上を通らないかを調べる)。
    points: Vec<Point>,
}

/// ピンから格子点までの引き出し線。
#[derive(Debug, Clone)]
pub struct Lead {
    /// ピンの位置から始まり、格子点で終わる点列。
    pub points: Vec<Point>,
    /// 終わりの格子点。
    pub node: usize,
    /// 終わりの点から先へ出ていく向き。分からなければNone。
    pub out: Option<Dir>,
}

impl Lead {
    /// 終わりの点から見てピンのある側の向き (引き出し線が辺を使っているときだけ)。
    fn back(&self) -> Option<Dir> {
        let n = self.points.len();
        if n < 2 {
            return None;
        }
        Dir::between(self.points[n - 1], self.points[n - 2])
    }
}

/// 他のネットが使っている場所。
pub struct Occ {
    /// 格子点ごとの、そこから出る辺の向きのビット。
    pub(crate) mask: Vec<u8>,
    /// 引き出し線の終わりの点など、持ち主のネットだけが使える点 (持ち主の番号)。
    pub(crate) reserved: Vec<u32>,
    /// 通れない点 (動かない他の配線の曲がり・端・ジャンクションなど)。
    pub(crate) dyn_block: Vec<bool>,
}

impl Occ {
    pub fn new(grid: &RouteGrid) -> Self {
        let n = grid.nx * grid.ny;
        Self { mask: vec![0; n], reserved: vec![NONE; n], dyn_block: vec![false; n] }
    }
}

/// ネットの木。格子点ごとに、すでに使っている辺の向きのビットを持つ。
#[derive(Debug, Clone, Default)]
pub(crate) struct Tree {
    pub(crate) mask: HashMap<usize, u8>,
}

impl Tree {
    /// 点に枝を足せるか。向き`d_in`に動いて着くので、着く側の辺は`d_in`の反対向き。
    /// すでに3方向ある点には足せない (4方向の点を作らない)。
    fn can_join(&self, n: usize, d_in: Dir) -> bool {
        match self.mask.get(&n) {
            Some(m) => m.count_ones() <= 2 && m & d_in.opp().bit() == 0,
            None => false,
        }
    }

    pub(crate) fn add_lead(&mut self, lead: &Lead) {
        let bit = lead.back().map_or(0, Dir::bit);
        *self.mask.entry(lead.node).or_insert(0) |= bit;
    }

    #[allow(dead_code)] // タスク2 (ネットの木) で使う
    pub(crate) fn add_path(&mut self, path: &[usize], grid: &RouteGrid) {
        for w in path.windows(2) {
            let d = grid.dir_between(w[0], w[1]);
            *self.mask.entry(w[0]).or_insert(0) |= d.bit();
            *self.mask.entry(w[1]).or_insert(0) |= d.opp().bit();
        }
    }

    /// 木が収まる格子の範囲 (x0, y0, x1, y1)。空ならNone。
    fn bbox(&self, grid: &RouteGrid) -> Option<(i64, i64, i64, i64)> {
        let mut it = self.mask.keys().map(|&n| grid.xy(n));
        let (x, y) = it.next()?;
        Some(it.fold((x, y, x, y), |(a, b, c, d), (x, y)| (a.min(x), b.min(y), c.max(x), d.max(y))))
    }
}

impl RouteGrid {
    /// シートから格子を作る。通れない場所: シンボルの外形と文字の枠 (どちらも`clearance_mm`だけ広げる)、
    /// 図枠の外側、表題欄、ピンとラベルの位置。線番の文字は配線のそばに後から付くので数えない。
    pub fn build(sheet: &Sheet, defs: &[SymbolDef], opts: &RouteOptions) -> Self {
        let (mut w, mut h) = sheet.size.dimensions_mm();
        if sheet.orientation == Orientation::Portrait {
            std::mem::swap(&mut w, &mut h);
        }
        let g = opts.grid_mm;
        let nx = (w / g + 1e-9).floor() as usize + 1;
        let ny = (h / g + 1e-9).floor() as usize + 1;
        let n = nx * ny;
        let mut grid = Self {
            opts: opts.clone(),
            nx,
            ny,
            blocked: vec![false; n],
            pin_blocked: vec![false; n],
            h_blocked: vec![false; n],
            v_blocked: vec![false; n],
            points: Vec::new(),
        };

        // 図枠の外側と表題欄
        let m = opts.frame_margin_mm;
        for iy in 0..ny {
            for ix in 0..nx {
                let (x, y) = (ix as f64 * g, iy as f64 * g);
                if x < m - 1e-9 || x > w - m + 1e-9 || y < m - 1e-9 || y > h - m + 1e-9 {
                    grid.blocked[iy * nx + ix] = true;
                }
            }
        }
        let [tw, th] = opts.title_block_mm;
        grid.mark_rect(&Rect { min: Point::new(w - m - tw, h - m - th), max: Point::new(w - m, h - m) });

        // シンボルの外形とピン
        let idx = DefIndex::new(defs);
        let c = opts.clearance_mm;
        for e in sheet.entities.values() {
            match e {
                Entity::Symbol(s) => {
                    let Some(def) = idx.get(s) else { continue };
                    if let Some(r) = symbol_bounds(s, &def) {
                        grid.mark_rect(&inflate(&r, c));
                    }
                    for pin in &def.pins {
                        grid.points.push(transform_local(pin.at, s));
                    }
                }
                Entity::NetLabel(l) => grid.points.push(l.at),
                _ => {}
            }
        }
        // 文字の枠
        for b in text_boxes(sheet, defs) {
            if b.kind != TextKind::WireNumber {
                grid.mark_rect(&inflate(&b.rect, c));
            }
        }
        // ピンとラベルの点
        for q in std::mem::take(&mut grid.points) {
            grid.mark_point(q);
            grid.points.push(q);
        }
        grid
    }

    pub(crate) fn node(&self, ix: i64, iy: i64) -> Option<usize> {
        if ix < 0 || iy < 0 || ix >= self.nx as i64 || iy >= self.ny as i64 {
            None
        } else {
            Some(iy as usize * self.nx + ix as usize)
        }
    }

    pub(crate) fn xy(&self, n: usize) -> (i64, i64) {
        ((n % self.nx) as i64, (n / self.nx) as i64)
    }

    pub(crate) fn point(&self, n: usize) -> Point {
        let (x, y) = self.xy(n);
        Point::new(x as f64 * self.opts.grid_mm, y as f64 * self.opts.grid_mm)
    }

    /// 点に最も近い格子点。紙の外ならNone。
    pub(crate) fn nearest(&self, p: Point) -> Option<usize> {
        let g = self.opts.grid_mm;
        self.node((p.x / g).round() as i64, (p.y / g).round() as i64)
    }

    pub(crate) fn step(&self, n: usize, d: Dir) -> Option<usize> {
        let (x, y) = self.xy(n);
        let (dx, dy) = d.delta();
        self.node(x + dx, y + dy)
    }

    /// 隣り合う2点を結ぶ向き。
    #[allow(dead_code)] // タスク2 (ネットの木) で使う
    pub(crate) fn dir_between(&self, a: usize, b: usize) -> Dir {
        let ((ax, ay), (bx, by)) = (self.xy(a), self.xy(b));
        if bx > ax {
            Dir::E
        } else if bx < ax {
            Dir::W
        } else if by > ay {
            Dir::S
        } else {
            Dir::N
        }
    }

    pub(crate) fn edge_blocked(&self, n: usize, d: Dir) -> bool {
        match d {
            Dir::E => self.h_blocked[n],
            Dir::W => self.h_blocked[n - 1],
            Dir::S => self.v_blocked[n],
            Dir::N => self.v_blocked[n - self.nx],
        }
    }

    /// 矩形の内側 (縁は含まない) の格子点と、内側にかかる辺を通れなくする。
    fn mark_rect(&mut self, r: &Rect) {
        let g = self.opts.grid_mm;
        let (fx0, fx1, fy0, fy1) = (r.min.x / g, r.max.x / g, r.min.y / g, r.max.y / g);
        // 内側の格子点は開区間。辺は、長さ1の区間が開区間と重なるもの。
        let inner = |a: f64, b: f64| ((a + 1e-9).floor() as i64 + 1, (b - 1e-9).ceil() as i64 - 1);
        let touched = |a: f64, b: f64| ((a + 1e-9).floor() as i64, (b - 1e-9).ceil() as i64 - 1);
        let (nx, ny) = (self.nx as i64, self.ny as i64);
        let (ix0, ix1) = inner(fx0, fx1);
        let (iy0, iy1) = inner(fy0, fy1);
        let (ex0, ex1) = touched(fx0, fx1);
        let (ey0, ey1) = touched(fy0, fy1);
        for iy in iy0.max(0)..=iy1.min(ny - 1) {
            for ix in ix0.max(0)..=ix1.min(nx - 1) {
                self.blocked[(iy * nx + ix) as usize] = true;
            }
            for ix in ex0.max(0)..=ex1.min(nx - 1) {
                self.h_blocked[(iy * nx + ix) as usize] = true;
            }
        }
        for ix in ix0.max(0)..=ix1.min(nx - 1) {
            for iy in ey0.max(0)..=ey1.min(ny - 1) {
                self.v_blocked[(iy * nx + ix) as usize] = true;
            }
        }
    }

    /// ピンやラベルの点を通れなくする。格子点の上なら点を、格子の線の上なら線の1辺を塞ぐ。
    fn mark_point(&mut self, q: Point) {
        let g = self.opts.grid_mm;
        let (fx, fy) = (q.x / g, q.y / g);
        let on = |v: f64| (v - v.round()).abs() * g < CONNECT_EPS / 2.0;
        match (on(fx), on(fy)) {
            (true, true) => {
                if let Some(n) = self.node(fx.round() as i64, fy.round() as i64) {
                    self.pin_blocked[n] = true;
                }
            }
            (false, true) => {
                if let Some(n) = self.node(fx.floor() as i64, fy.round() as i64) {
                    self.h_blocked[n] = true;
                }
            }
            (true, false) => {
                if let Some(n) = self.node(fx.round() as i64, fy.floor() as i64) {
                    self.v_blocked[n] = true;
                }
            }
            (false, false) => {}
        }
    }

    /// ピンの位置と向きから、格子の線の上の出発点までの引き出し線を作る。
    ///
    /// ピンが格子の上にあればピンから向きに沿って、外形の外の最初の格子点まで。格子の上になければ、
    /// まず向きに沿って最も近い格子の線まで (向きが分からなければ横へ丸める)、次に直角に最も近い格子点まで
    /// 進む。引き出し線は必ずピンの位置ぴったりから始まる。他のピンの上を通るとき・紙の外へ出るとき・
    /// 外形から出られないときはNone。
    pub fn lead(&self, pos: Point, dir: Option<Dir>) -> Option<Lead> {
        let g = self.opts.grid_mm;
        let on = |v: f64| (v / g - (v / g).round()).abs() * g < 1e-6;
        let mut pts = vec![pos];
        let push = |pts: &mut Vec<Point>, q: Point| {
            if pts.last().is_none_or(|l| l.distance_to(&q) > 1e-9) {
                pts.push(q);
            }
        };
        if !(on(pos.x) && on(pos.y)) {
            let snap = |v: f64, d: Option<Dir>, plus: Dir| -> f64 {
                if on(v) {
                    v
                } else {
                    match d {
                        Some(d) if d == plus => (v / g).ceil() * g,
                        Some(_) => (v / g).floor() * g,
                        None => (v / g).round() * g,
                    }
                }
            };
            let near = |v: f64| (v / g).round() * g;
            if dir.is_none_or(Dir::horizontal) {
                let x = snap(pos.x, dir, Dir::E);
                push(&mut pts, Point::new(x, pos.y));
                push(&mut pts, Point::new(x, near(pos.y)));
            } else {
                let y = snap(pos.y, dir, Dir::S);
                push(&mut pts, Point::new(pos.x, y));
                push(&mut pts, Point::new(near(pos.x), y));
            }
        }
        let end = *pts.last()?;
        let mut node = self.nearest(end)?;
        let out = if pts.len() > 1 { Dir::between(pts[pts.len() - 2], pts[pts.len() - 1]) } else { dir };
        let mut steps = 0;
        while self.blocked[node] {
            let d = out?;
            node = self.step(node, d)?;
            push(&mut pts, self.point(node));
            steps += 1;
            if steps > 8 {
                return None;
            }
        }
        // 他のピンの上を通らない
        for w in pts.windows(2) {
            let hit = self.points.iter().any(|&q| {
                !same(q, pos) && crate::netlist::segment_distance(&q, &w[0], &w[1]) < CONNECT_EPS
            });
            if hit {
                return None;
            }
        }
        Some(Lead { points: pts, node, out })
    }

    /// 2つの引き出し線の終わりの点どうしを、他のネットを避けて結ぶ。ピンから ピンまでの点列 (折れ点だけ) を返す。
    pub fn connect(&self, a: &Lead, b: &Lead, occ: &Occ) -> Option<Vec<Point>> {
        let mut tree = Tree::default();
        tree.add_lead(b);
        let path = if a.node == b.node {
            vec![a.node]
        } else {
            Searcher::new(self).search(self, occ, &tree, a.node, a.out)?
        };
        let mut pts = a.points.clone();
        pts.extend(path.iter().skip(1).map(|&n| self.point(n)));
        pts.extend(b.points.iter().rev().skip(1).copied());
        Some(simplify(pts))
    }
}

fn inflate(r: &Rect, c: f64) -> Rect {
    Rect { min: Point::new(r.min.x - c, r.min.y - c), max: Point::new(r.max.x + c, r.max.y + c) }
}

/// 重なった点と一直線上の途中の点を取り除く。
pub(crate) fn simplify(pts: Vec<Point>) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(pts.len());
    for q in pts {
        if out.last().is_some_and(|l| l.distance_to(&q) < 1e-9) {
            continue;
        }
        while out.len() >= 2 {
            let (a, b) = (out[out.len() - 2], out[out.len() - 1]);
            let cross = (b.x - a.x) * (q.y - b.y) - (b.y - a.y) * (q.x - b.x);
            if cross.abs() < 1e-9 {
                out.pop();
            } else {
                break;
            }
        }
        out.push(q);
    }
    out
}

/// A* の作業領域。呼び出しごとの確保を避けるため使い回せる。
pub(crate) struct Searcher {
    cost: Vec<i64>,
    stamp: Vec<u32>,
    parent: Vec<u32>,
    gen: u32,
}

impl Searcher {
    pub(crate) fn new(grid: &RouteGrid) -> Self {
        let n = grid.nx * grid.ny * 5;
        Self { cost: vec![0; n], stamp: vec![0; n], parent: vec![0; n], gen: 0 }
    }

    /// `start`から木のどこかの点までの最も安い経路 (格子点の列) を探す。
    ///
    /// 状態は (格子点, 着いた向き)。費用は 1歩=格子間隔 + 曲がり + 横切り + 他の配線の隣。通れない:
    /// 外形・文字・ピンの点・持ち主のいる点・通れない点・他のネットが使う辺。他のネットが
    /// 直線で通る点は、直角に真っすぐ抜けるときだけ通れる。木の点には、3方向未満のときだけ着ける
    /// (着いたところが分岐になる)。`start`から出る最初の向きは`start_dir`の逆には進めない。
    pub(crate) fn search(
        &mut self,
        grid: &RouteGrid,
        occ: &Occ,
        tree: &Tree,
        start: usize,
        start_dir: Option<Dir>,
    ) -> Option<Vec<usize>> {
        self.gen += 1;
        let gen = self.gen;
        let milli = |v: f64| (v * 1000.0).round() as i64;
        let (step_c, bend_c, cross_c, near_c) =
            (milli(grid.opts.grid_mm), milli(grid.opts.bend_cost), milli(grid.opts.cross_cost), milli(grid.opts.near_cost));
        let (bx0, by0, bx1, by1) = tree.bbox(grid)?;
        let h = |n: usize| -> i64 {
            let (x, y) = grid.xy(n);
            let dx = (bx0 - x).max(0).max(x - bx1);
            let dy = (by0 - y).max(0).max(y - by1);
            (dx + dy) * step_c
        };
        let slot0 = start_dir.map_or(4, Dir::idx);
        let st0 = start * 5 + slot0;
        self.cost[st0] = 0;
        self.stamp[st0] = gen;
        self.parent[st0] = u32::MAX;
        let mut heap = BinaryHeap::new();
        heap.push(Reverse((h(start), st0)));
        while let Some(Reverse((f, st))) = heap.pop() {
            let node = st / 5;
            let slot = st % 5;
            let g_cost = self.cost[st];
            if f > g_cost + h(node) {
                continue;
            }
            if st != st0 && tree.mask.contains_key(&node) {
                let mut path = vec![node];
                let mut cur = st;
                while self.parent[cur] != u32::MAX {
                    cur = self.parent[cur] as usize;
                    path.push(cur / 5);
                }
                path.reverse();
                return Some(path);
            }
            let crossing = node != start && occ.mask[node] != 0;
            for d in Dir::ALL {
                if slot < 4 && d == Dir::ALL[slot].opp() {
                    continue;
                }
                if crossing && !(slot < 4 && d == Dir::ALL[slot]) {
                    continue;
                }
                let Some(n2) = grid.step(node, d) else { continue };
                if grid.edge_blocked(node, d) || occ.mask[node] & d.bit() != 0 {
                    continue;
                }
                let mut c = step_c;
                if tree.mask.contains_key(&n2) {
                    if !tree.can_join(n2, d) {
                        continue;
                    }
                } else {
                    if grid.blocked[n2] || grid.pin_blocked[n2] || occ.reserved[n2] != NONE || occ.dyn_block[n2] {
                        continue;
                    }
                    let m = occ.mask[n2];
                    if m != 0 {
                        let straight = if d.horizontal() { m == NS } else { m == EW };
                        if !straight {
                            continue;
                        }
                        c += cross_c;
                    } else {
                        let side = if d.horizontal() { [Dir::N, Dir::S] } else { [Dir::E, Dir::W] };
                        if side.iter().any(|&sd| grid.step(n2, sd).is_some_and(|nb| occ.mask[nb] != 0)) {
                            c += near_c;
                        }
                    }
                }
                if slot < 4 && d != Dir::ALL[slot] {
                    c += bend_c;
                }
                let nst = n2 * 5 + d.idx();
                let nc = g_cost + c;
                if self.stamp[nst] != gen || nc < self.cost[nst] {
                    self.stamp[nst] = gen;
                    self.cost[nst] = nc;
                    self.parent[nst] = st as u32;
                    heap.push(Reverse((nc + h(n2), nst)));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use crate::symbol::{PinDef, PinDir, Primitive};
    use uuid::Uuid;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    fn pin(number: &str, x: f64, y: f64, dir: PinDir) -> PinDef {
        PinDef { number: number.into(), name: String::new(), at: p(x, y), dir, contacts: vec![] }
    }

    fn rect(x1: f64, y1: f64, x2: f64, y2: f64) -> Primitive {
        Primitive::Rect { p1: p(x1, y1), p2: p(x2, y2), filled: false }
    }

    fn def(id: &str, primitives: Vec<Primitive>, pins: Vec<PinDef>) -> SymbolDef {
        SymbolDef {
            id: id.into(),
            name: id.into(),
            name_ja: id.into(),
            category: "test".into(),
            ref_prefix: "X".into(),
            keywords: vec![],
            primitives,
            pins,
            text_slots: vec![],
            free_pins: false,
            net: None,
        }
    }

    /// 20 x 10 mm box with a pin on the left (number 1) and on the right (number 2).
    fn box2() -> SymbolDef {
        def("box2", vec![rect(-5.0, -5.0, 5.0, 5.0)], vec![pin("1", -10.0, 0.0, PinDir::Left), pin("2", 10.0, 0.0, PinDir::Right)])
    }

    /// 10 x 20 mm box with a pin on top (number 1) and on the bottom (number 2).
    fn cap() -> SymbolDef {
        def("cap", vec![rect(-5.0, -5.0, 5.0, 5.0)], vec![pin("1", 0.0, -10.0, PinDir::Up), pin("2", 0.0, 10.0, PinDir::Down)])
    }

    /// A pin-less rectangle that only blocks space.
    fn wall(id: &str, w: f64, h: f64) -> SymbolDef {
        def(id, vec![rect(-w / 2.0, -h / 2.0, w / 2.0, h / 2.0)], vec![])
    }

    /// A box whose single pin is 7.3 mm to the right of the centre, off the 1.25 mm grid.
    fn freebox() -> SymbolDef {
        let mut d = def("freebox", vec![rect(-5.0, -5.0, 5.0, 5.0)], vec![pin("1", 7.3, 0.0, PinDir::Right)]);
        d.free_pins = true;
        d
    }

    fn defs() -> Vec<SymbolDef> {
        vec![box2(), cap(), wall("wall_h", 70.0, 5.0), wall("wall_v", 5.0, 40.0), wall("slab", 20.0, 10.0), freebox()]
    }

    fn place(sheet: &mut Sheet, symbol_id: &str, at: (f64, f64), reference: &str) -> EntityId {
        let id = Uuid::new_v4();
        sheet.entities.insert(
            id,
            Entity::Symbol(SymbolInstance {
                id,
                symbol_id: symbol_id.into(),
                at: p(at.0, at.1),
                rotation: 0,
                mirror: false,
                reference: reference.into(),
                value: String::new(),
                attrs: Default::default(),
            }),
        );
        id
    }

    fn new_sheet() -> Sheet {
        Sheet::new("t", PaperSize::A3, Orientation::Landscape)
    }

    fn pin_of(sheet: &Sheet, defs: &[SymbolDef], reference: &str, number: &str) -> PinPt {
        pin_points(sheet, defs)
            .into_iter()
            .find(|q| q.reference == reference && q.number == number)
            .expect("pin exists")
    }

    fn lead_of(grid: &RouteGrid, pin: &PinPt) -> Lead {
        grid.lead(pin.pos, Some(pin.dir)).expect("lead")
    }

    fn connect(sheet: &Sheet, a: (&str, &str), b: (&str, &str)) -> Option<Vec<Point>> {
        let defs = defs();
        let grid = RouteGrid::build(sheet, &defs, &RouteOptions::default());
        let (pa, pb) = (pin_of(sheet, &defs, a.0, a.1), pin_of(sheet, &defs, b.0, b.1));
        let (la, lb) = (lead_of(&grid, &pa), lead_of(&grid, &pb));
        grid.connect(&la, &lb, &Occ::new(&grid))
    }

    fn orthogonal(pts: &[Point]) -> bool {
        pts.windows(2).all(|w| (w[0].x - w[1].x).abs() < 1e-9 || (w[0].y - w[1].y).abs() < 1e-9)
    }

    /// True if an axis-parallel segment runs through the inside of the rectangle (touching the edge does not count).
    fn hits(a: Point, b: Point, min: Point, max: Point) -> bool {
        let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
        let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
        x0 < max.x - 1e-6 && x1 > min.x + 1e-6 && y0 < max.y - 1e-6 && y1 > min.y + 1e-6
    }

    /// Two pins that face each other on one line are joined by one straight wire.
    /// 向かい合ったピンが一直線上にあるときは、1本の直線の配線でつながる。
    #[test]
    fn facing_pins_on_one_line_get_a_straight_wire() {
        let mut sheet = new_sheet();
        place(&mut sheet, "box2", (50.0, 50.0), "R1");
        place(&mut sheet, "box2", (100.0, 50.0), "R2");
        let route = connect(&sheet, ("R1", "2"), ("R2", "1")).expect("route");
        assert_eq!(route, vec![p(60.0, 50.0), p(90.0, 50.0)]);
    }

    /// Two pins that are not in line are joined by an L shape with exactly one bend.
    /// 一直線上にないピンどうしは、曲がりが1回だけのL字の配線でつながる。
    #[test]
    fn pins_not_in_line_get_an_l_shaped_wire() {
        let mut sheet = new_sheet();
        place(&mut sheet, "box2", (50.0, 50.0), "R1");
        place(&mut sheet, "cap", (100.0, 30.0), "C1");
        let route = connect(&sheet, ("R1", "2"), ("C1", "2")).expect("route");
        assert_eq!(route, vec![p(60.0, 50.0), p(100.0, 50.0), p(100.0, 40.0)]);
    }

    /// A symbol standing between two pins makes the wire go around it, never through its outline.
    /// 2つのピンの間にシンボルがあるときは、外形の中を通らずに迂回する。
    #[test]
    fn the_wire_goes_around_a_symbol_in_the_way() {
        let mut sheet = new_sheet();
        place(&mut sheet, "box2", (50.0, 50.0), "R1");
        place(&mut sheet, "box2", (90.0, 50.0), "R2");
        place(&mut sheet, "box2", (130.0, 50.0), "R3");
        let route = connect(&sheet, ("R1", "2"), ("R3", "1")).expect("route");
        assert_eq!((route[0], *route.last().unwrap()), (p(60.0, 50.0), p(120.0, 50.0)));
        assert!(orthogonal(&route), "{route:?}");
        for w in route.windows(2) {
            assert!(!hits(w[0], w[1], p(80.0, 45.0), p(100.0, 55.0)), "{route:?}");
        }
    }

    /// A text note in the way makes the wire go around the text box.
    /// 途中に文字の注記があるときは、文字の枠を避けて迂回する。
    #[test]
    fn the_wire_goes_around_a_text_box_in_the_way() {
        let mut sheet = new_sheet();
        place(&mut sheet, "box2", (50.0, 50.0), "R1");
        place(&mut sheet, "box2", (130.0, 50.0), "R2");
        let id = Uuid::new_v4();
        sheet.entities.insert(
            id,
            Entity::Text(TextEntity {
                id,
                at: p(85.0, 52.0),
                text: "NOTE".into(),
                height: 4.0,
                rotation: 0,
                anchor: TextAnchor::Start,
                color: None,
                bold: false,
                leader_to: None,
                spacing: None,
                stretch: None,
            }),
        );
        let boxes = crate::tidy::text_boxes(&sheet, &defs());
        let note = boxes.iter().find(|b| b.text == "NOTE").expect("note box");
        let route = connect(&sheet, ("R1", "2"), ("R2", "1")).expect("route");
        assert!(route.len() > 2, "{route:?}");
        for w in route.windows(2) {
            assert!(!hits(w[0], w[1], note.rect.min, note.rect.max), "{route:?}");
        }
    }

    /// When a pin is walled in so that no wire can reach it, no route is returned.
    /// ピンが壁で完全に囲まれていて届かないときは、経路なしになる。
    #[test]
    fn a_pin_that_is_walled_in_has_no_route() {
        let mut sheet = new_sheet();
        place(&mut sheet, "box2", (50.0, 50.0), "R1");
        place(&mut sheet, "box2", (130.0, 50.0), "R2");
        place(&mut sheet, "wall_h", (130.0, 35.0), "");
        place(&mut sheet, "wall_h", (130.0, 65.0), "");
        place(&mut sheet, "wall_v", (97.5, 50.0), "");
        place(&mut sheet, "wall_v", (162.5, 50.0), "");
        assert!(connect(&sheet, ("R1", "2"), ("R2", "1")).is_none());
    }

    /// A pin that is not on the routing grid gets a short lead that starts exactly at the pin and ends on the grid.
    /// 格子の上にないピンには、ピンの位置ぴったりから始まり格子の上で終わる短い引き出し線がつく。
    #[test]
    fn an_off_grid_pin_gets_a_lead_that_starts_exactly_at_the_pin() {
        let mut sheet = new_sheet();
        place(&mut sheet, "freebox", (50.0, 50.0), "F1");
        place(&mut sheet, "box2", (100.0, 50.0), "R2");
        let defs = defs();
        let grid = RouteGrid::build(&sheet, &defs, &RouteOptions::default());
        let pin = pin_of(&sheet, &defs, "F1", "1");
        let lead = lead_of(&grid, &pin);
        assert_eq!((lead.points[0], *lead.points.last().unwrap()), (pin.pos, p(58.75, 50.0)));
        let route = connect(&sheet, ("F1", "1"), ("R2", "1")).expect("route");
        assert_eq!((route[0], *route.last().unwrap()), (pin.pos, p(90.0, 50.0)));
    }
}
