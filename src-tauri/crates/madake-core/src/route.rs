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
use crate::model::{Entity, EntityId, Orientation, Sheet, Wire};
use crate::netlist::{extract_netlist, on_wire, pin_positions, transform_local, CONNECT_EPS};
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

/// 引き直しの設定。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default)]
pub struct RerouteOptions {
    /// 引き直すネットの名前。空なら全てのネット。
    pub nets: Vec<String>,
    /// 経路探索の設定。
    pub route: RouteOptions,
}

/// 引き直しの計画。図面は書き換えず、消すものと足すものだけを返す。
#[derive(Debug, Clone, Default, Serialize)]
pub struct ReroutePlan {
    /// 消すエンティティ (古い配線とジャンクション)。
    pub remove: Vec<EntityId>,
    /// 足すエンティティ (新しい配線とジャンクション)。
    pub add: Vec<Entity>,
    /// 引き直したネットの名前 (重複なし、名前順)。
    pub rerouted: Vec<String>,
    /// 変えなかったネットの (名前, 理由)。
    pub unchanged: Vec<(String, String)>,
}

/// 引き直しの1単位: ネットのうち、配線で物理的につながった1かたまり。
///
/// ネットは同名のネットラベルで離れたかたまりをつなぐので、かたまりごとに別々に引き直す
/// (離れたかたまりを新しい配線でつなぐと、図面の見た目が変わるため)。
struct Unit {
    net: String,
    wires: Vec<EntityId>,
    junctions: Vec<EntityId>,
    template: Wire,
    old_lines: Vec<Vec<Point>>,
    old_junctions: Vec<Point>,
    /// 端子 (ピン・ラベル) の位置と向き。位置順。
    terms: Vec<(Point, Option<Dir>)>,
    leads: Vec<Lead>,
}

/// 経路が使う場所。他のネットの探索に対する占有を表す。
struct UnitRoute {
    segs: Vec<(Point, Point)>,
    nodes: HashMap<usize, u8>,
    /// 経路が収まる格子の範囲 (x0, y0, x1, y1)。
    bbox: (i64, i64, i64, i64),
}

/// 配線の線分を格子の占有に直す。(格子点, 辺のビット, 通れない印) の並び。
///
/// 格子の上の軸に平行な線分は、通る点にその辺のビットを付ける (真っすぐ通る点は直角に横切れる)。
/// それ以外の線分は、近い格子点を通れなくする。
fn rasterize(grid: &RouteGrid, line: &[Point]) -> Vec<(usize, u8, bool)> {
    let g = grid.opts.grid_mm;
    let on = |v: f64| (v / g - (v / g).round()).abs() * g < 1e-6;
    let mut out = Vec::new();
    for w in line.windows(2) {
        let (a, b) = (w[0], w[1]);
        let dir = Dir::between(a, b);
        let aligned = (a.x - b.x).abs() < 1e-9 || (a.y - b.y).abs() < 1e-9;
        match (dir, aligned && on(a.x) && on(a.y) && on(b.x) && on(b.y)) {
            (Some(d), true) => {
                let (Some(na), Some(nb)) = (grid.nearest(a), grid.nearest(b)) else { continue };
                let mut cur = na;
                out.push((cur, d.bit(), false));
                while cur != nb {
                    let Some(next) = grid.step(cur, d) else { break };
                    out.push((next, d.opp().bit() | if next == nb { 0 } else { d.bit() }, false));
                    cur = next;
                }
            }
            _ => {
                let (x0, x1) = ((a.x.min(b.x) / g).floor() as i64 - 1, (a.x.max(b.x) / g).ceil() as i64 + 1);
                let (y0, y1) = ((a.y.min(b.y) / g).floor() as i64 - 1, (a.y.max(b.y) / g).ceil() as i64 + 1);
                for iy in y0..=y1 {
                    for ix in x0..=x1 {
                        if let Some(n) = grid.node(ix, iy) {
                            if crate::netlist::segment_distance(&grid.point(n), &a, &b) < g / 2.0 + 1e-6 {
                                out.push((n, 0, true));
                            }
                        }
                    }
                }
            }
        }
    }
    out
}

/// 点の近くの格子点を通れなくする記述。
fn block_point(grid: &RouteGrid, q: Point) -> Vec<(usize, u8, bool)> {
    rasterize(grid, &[q, q]).into_iter().chain(point_nodes(grid, q)).collect()
}

fn point_nodes(grid: &RouteGrid, q: Point) -> Vec<(usize, u8, bool)> {
    let g = grid.opts.grid_mm;
    let (cx, cy) = ((q.x / g).round() as i64, (q.y / g).round() as i64);
    let mut out = Vec::new();
    for iy in cy - 1..=cy + 1 {
        for ix in cx - 1..=cx + 1 {
            if let Some(n) = grid.node(ix, iy) {
                if grid.point(n).distance_to(&q) < g / 2.0 + 1e-6 {
                    out.push((n, 0, true));
                }
            }
        }
    }
    out
}

fn apply_raster(occ: &mut Occ, entries: &[(usize, u8, bool)]) {
    for &(n, bits, block) in entries {
        if block {
            occ.dyn_block[n] = true;
        } else {
            occ.mask[n] |= bits;
        }
    }
}

/// 経路が、動かない配線の占有 (`fixed`) とぶつかるか。
fn conflicts(route: &UnitRoute, fixed: &[(usize, u8, bool)]) -> bool {
    let mut by_node: HashMap<usize, (u8, bool)> = HashMap::new();
    for &(n, bits, block) in fixed {
        let e = by_node.entry(n).or_insert((0, false));
        e.0 |= bits;
        e.1 |= block;
    }
    route.nodes.iter().any(|(n, &mine)| match by_node.get(n) {
        None => false,
        Some(&(theirs, block)) => {
            if block {
                return true;
            }
            if theirs == 0 {
                return false;
            }
            let crossing = (mine == NS && theirs == EW) || (mine == EW && theirs == NS);
            !crossing
        }
    })
}

fn add_route(occ: &mut Occ, r: &UnitRoute) {
    for (&n, &m) in &r.nodes {
        occ.mask[n] |= m;
    }
}

fn remove_route(occ: &mut Occ, r: &UnitRoute) {
    for (&n, &m) in &r.nodes {
        occ.mask[n] &= !m;
    }
}

/// 1つの単位の木を作る。最初に中心に最も近い端子を木の根にし、残りを近い順に木へつなぐ。
fn route_unit(grid: &RouteGrid, occ: &Occ, searcher: &mut Searcher, unit: &Unit) -> Option<UnitRoute> {
    let n = unit.terms.len();
    let (cx, cy) = (
        unit.terms.iter().map(|t| t.0.x).sum::<f64>() / n as f64,
        unit.terms.iter().map(|t| t.0.y).sum::<f64>() / n as f64,
    );
    let first = (0..n)
        .min_by(|&a, &b| {
            let da = (unit.terms[a].0.x - cx).hypot(unit.terms[a].0.y - cy);
            let db = (unit.terms[b].0.x - cx).hypot(unit.terms[b].0.y - cy);
            da.total_cmp(&db).then(a.cmp(&b))
        })
        .unwrap_or(0);
    let mut tree = Tree::default();
    let mut segs: Vec<(Point, Point)> = Vec::new();
    let lead_segs = |l: &Lead, segs: &mut Vec<(Point, Point)>| segs.extend(l.points.windows(2).map(|w| (w[0], w[1])));
    tree.add_lead(&unit.leads[first]);
    lead_segs(&unit.leads[first], &mut segs);
    let mut remaining: Vec<usize> = (0..n).filter(|&i| i != first).collect();
    while !remaining.is_empty() {
        // 木に最も近い端子 (格子上の直線距離)
        let dist = |i: usize| -> i64 {
            let (x, y) = grid.xy(unit.leads[i].node);
            tree.mask
                .keys()
                .map(|&k| {
                    let (kx, ky) = grid.xy(k);
                    (kx - x).abs() + (ky - y).abs()
                })
                .min()
                .unwrap_or(0)
        };
        let pos = (0..remaining.len()).min_by_key(|&k| (dist(remaining[k]), remaining[k]))?;
        let t = remaining.remove(pos);
        let lead = &unit.leads[t];
        let path = if tree.mask.contains_key(&lead.node) {
            let m = tree.mask[&lead.node];
            let bit = lead.back().map_or(0, Dir::bit);
            if m & bit != 0 || (m | bit).count_ones() > 3 {
                return None;
            }
            vec![lead.node]
        } else {
            searcher.search(grid, occ, &tree, lead.node, lead.out)?
        };
        tree.add_lead(lead);
        tree.add_path(&path, grid);
        lead_segs(lead, &mut segs);
        segs.extend(path.windows(2).map(|w| (grid.point(w[0]), grid.point(w[1]))));
    }
    let mut bbox = (i64::MAX, i64::MAX, i64::MIN, i64::MIN);
    for &k in tree.mask.keys() {
        let (x, y) = grid.xy(k);
        bbox = (bbox.0.min(x), bbox.1.min(y), bbox.2.max(x), bbox.3.max(y));
    }
    Some(UnitRoute { segs, nodes: tree.mask, bbox })
}

fn quant(q: Point) -> (i64, i64) {
    ((q.x * 1000.0).round() as i64, (q.y * 1000.0).round() as i64)
}

/// 経路の線分から、配線とジャンクションを作る。端子とジャンクション (3方向以上の点) の間を1本の配線にし、
/// 途中の一直線上の点は省く。
fn emit(unit: &Unit, route: &UnitRoute) -> Vec<Entity> {
    use std::collections::BTreeMap;
    use std::collections::BTreeSet;
    let mut keys: BTreeMap<(i64, i64), usize> = BTreeMap::new();
    let mut pts: Vec<Point> = Vec::new();
    let intern = |q: Point, keys: &mut BTreeMap<(i64, i64), usize>, pts: &mut Vec<Point>| -> usize {
        *keys.entry(quant(q)).or_insert_with(|| {
            pts.push(q);
            pts.len() - 1
        })
    };
    let terminal: BTreeSet<usize> = unit.terms.iter().map(|t| intern(t.0, &mut keys, &mut pts)).collect();
    let mut adj: Vec<BTreeSet<usize>> = Vec::new();
    for &(a, b) in &route.segs {
        let (ia, ib) = (intern(a, &mut keys, &mut pts), intern(b, &mut keys, &mut pts));
        adj.resize(pts.len(), BTreeSet::new());
        if ia != ib {
            adj[ia].insert(ib);
            adj[ib].insert(ia);
        }
    }
    adj.resize(pts.len(), BTreeSet::new());
    let is_vertex = |v: usize| terminal.contains(&v) || adj[v].len() != 2;
    let mut used: BTreeSet<(usize, usize)> = BTreeSet::new();
    let mut out = Vec::new();
    for v in keys.values().copied().collect::<Vec<_>>() {
        if !is_vertex(v) {
            continue;
        }
        for &nb in &adj[v] {
            if used.contains(&(v, nb)) {
                continue;
            }
            let mut path = vec![v, nb];
            used.insert((v, nb));
            used.insert((nb, v));
            let (mut prev, mut cur) = (v, nb);
            while !is_vertex(cur) {
                let Some(&next) = adj[cur].iter().find(|&&x| x != prev) else { break };
                used.insert((cur, next));
                used.insert((next, cur));
                path.push(next);
                prev = cur;
                cur = next;
            }
            let points = simplify(path.iter().map(|&i| pts[i]).collect());
            if points.len() >= 2 {
                out.push(Entity::Wire(Wire { id: uuid::Uuid::new_v4(), points, ..unit.template.clone() }));
            }
        }
    }
    for v in keys.values().copied().collect::<Vec<_>>() {
        if adj[v].len() >= 3 {
            out.push(Entity::Junction(crate::model::Junction { id: uuid::Uuid::new_v4(), at: pts[v] }));
        }
    }
    out
}

/// 配線の端点が、ピン・ラベル・ジャンクション・別の配線の端点のどれかにつながっているか。
fn attached(q: Point, owner: EntityId, others: &[(EntityId, Point)], anchors: &[Point]) -> bool {
    anchors.iter().any(|&a| same(a, q)) || others.iter().any(|&(id, e)| id != owner && same(e, q))
}

/// ネットの配線を、配線で物理的につながったかたまりに分ける (ネットリストと同じつなぎ方)。
fn clusters(wires: &[&Wire], anchors: &[Point]) -> Vec<Vec<usize>> {
    let mut parent: Vec<usize> = (0..wires.len()).collect();
    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let union = |parent: &mut Vec<usize>, a: usize, b: usize| {
        let (ra, rb) = (find(parent, a), find(parent, b));
        if ra != rb {
            parent[ra.max(rb)] = ra.min(rb);
        }
    };
    let ends = |w: &Wire| -> Vec<Point> {
        match (w.points.first(), w.points.last()) {
            (Some(a), Some(b)) => vec![*a, *b],
            _ => vec![],
        }
    };
    for a in 0..wires.len() {
        for b in a + 1..wires.len() {
            if ends(wires[a]).iter().any(|x| ends(wires[b]).iter().any(|y| same(*x, *y))) {
                union(&mut parent, a, b);
            }
        }
    }
    for &q in anchors {
        let hit: Vec<usize> = (0..wires.len()).filter(|&i| on_wire(wires[i], &q)).collect();
        for w in hit.windows(2) {
            union(&mut parent, w[0], w[1]);
        }
    }
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut root_of: HashMap<usize, usize> = HashMap::new();
    for i in 0..wires.len() {
        let r = find(&mut parent, i);
        let g = *root_of.entry(r).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[g].push(i);
    }
    groups
}

/// 端子の引き出し線。向きの分からない端子 (ラベル) は、外へ出られる最も短い向きを選ぶ。
fn make_lead(grid: &RouteGrid, pos: Point, dir: Option<Dir>) -> Option<Lead> {
    match dir {
        Some(_) => grid.lead(pos, dir),
        None => grid.lead(pos, None).or_else(|| {
            Dir::ALL
                .iter()
                .enumerate()
                .filter_map(|(i, &d)| grid.lead(pos, Some(d)).map(|l| (l.points.len(), i, l)))
                .min_by_key(|(len, i, _)| (*len, *i))
                .map(|(_, _, l)| l)
        }),
    }
}

/// シートの配線を、接続を変えずに引き直す計画を作る。図面は書き換えない。
///
/// ネットごと、配線で物理的につながったかたまりごとに、ピン・ネットラベル・ネット名シンボルのピンを
/// 端子とする木を格子の上に引く。引き直さないネット (ケーブルの芯・属性がまちまち・端が浮いている・
/// 選ばれていない) の配線は動かない障害物として扱う。経路が見つからないかたまりは古い配線を残す。
pub fn plan_reroute(sheet: &Sheet, defs: &[SymbolDef], opts: &RerouteOptions) -> ReroutePlan {
    use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
    let mut plan = ReroutePlan::default();
    let nets = extract_netlist(sheet, defs);
    let grid = RouteGrid::build(sheet, defs, &opts.route);
    let pins = pin_points(sheet, defs);
    let labels: Vec<Point> = sheet.entities.values().filter_map(|e| if let Entity::NetLabel(l) = e { Some(l.at) } else { None }).collect();
    let junctions: Vec<(EntityId, Point)> =
        sheet.entities.values().filter_map(|e| if let Entity::Junction(j) = e { Some((j.id, j.at)) } else { None }).collect();
    let wires: BTreeMap<EntityId, &Wire> = sheet.entities.iter().filter_map(|(id, e)| if let Entity::Wire(w) = e { Some((*id, w)) } else { None }).collect();
    let anchors: Vec<Point> = pins.iter().map(|q| q.pos).chain(labels.iter().copied()).chain(junctions.iter().map(|j| j.1)).collect();
    let all_ends: Vec<(EntityId, Point)> =
        wires.values().flat_map(|w| [w.points.first(), w.points.last()].into_iter().flatten().map(|&q| (w.id, q))).collect();

    // 他のネットと共有している配線 (ポートの接点) と、ケーブルの区間が使う配線
    let mut uses: HashMap<EntityId, usize> = HashMap::new();
    for net in &nets {
        for id in &net.wire_ids {
            *uses.entry(*id).or_insert(0) += 1;
        }
    }
    let mut cable_wires: HashSet<EntityId> = HashSet::new();
    for e in sheet.entities.values() {
        if let Entity::Cable(c) = e {
            if c.is_run() {
                for core in &c.cores {
                    for end in [&core.end_a, &core.end_b].into_iter().flatten() {
                        if let crate::model::CoreEnd::Wire { wire } = end {
                            cable_wires.insert(*wire);
                        }
                    }
                }
            }
        }
    }

    let mut units: Vec<Unit> = Vec::new();
    let mut fixed_wires: Vec<EntityId> = Vec::new();
    for net in &nets {
        if net.wire_ids.is_empty() {
            continue;
        }
        let reason = if !(opts.nets.is_empty() || opts.nets.contains(&net.name)) {
            Some("not selected".to_string())
        } else if net.wire_ids.iter().any(|id| uses.get(id).copied().unwrap_or(0) > 1) {
            Some("wires are shared by several nets (port contacts)".to_string())
        } else if !net.cable_cores.is_empty() || net.wire_ids.iter().any(|id| cable_wires.contains(id)) {
            Some("a cable run uses these wires (cable)".to_string())
        } else {
            None
        };
        if let Some(r) = reason {
            plan.unchanged.push((net.name.clone(), r));
            fixed_wires.extend(net.wire_ids.iter().copied());
            continue;
        }
        let net_wires: Vec<&Wire> = net.wire_ids.iter().filter_map(|id| wires.get(id).copied()).collect();
        for group in clusters(&net_wires, &anchors) {
            let ws: Vec<&Wire> = group.iter().map(|&i| net_wires[i]).collect();
            let keep = |plan: &mut ReroutePlan, fixed: &mut Vec<EntityId>, why: &str| {
                plan.unchanged.push((net.name.clone(), why.to_string()));
                fixed.extend(ws.iter().map(|w| w.id));
            };
            let t = ws[0];
            if ws.iter().any(|w| w.points.len() < 2) {
                keep(&mut plan, &mut fixed_wires, "a wire has no path");
                continue;
            }
            let mixed = ws.iter().any(|w| {
                w.color != t.color || w.sq != t.sq || w.part_no != t.part_no || w.net != t.net || w.dashed != t.dashed || w.weight_mm != t.weight_mm
            });
            if mixed {
                keep(&mut plan, &mut fixed_wires, "wires differ in colour, cross-section, part number, wire number, dash or weight");
                continue;
            }
            let free = ws.iter().any(|w| {
                [w.points[0], *w.points.last().unwrap()].iter().any(|&e| {
                    !attached(e, w.id, &all_ends, &anchors)
                })
            });
            if free {
                keep(&mut plan, &mut fixed_wires, "a wire has a free end that touches nothing");
                continue;
            }
            // 端子: 配線の上にあるピンとラベル。同じ位置ならピン (向きを持つ) が先
            let on_group = |q: Point| ws.iter().any(|w| on_wire(w, &q));
            let mut terms: Vec<(Point, Option<Dir>)> = Vec::new();
            for q in &pins {
                if on_group(q.pos) && !terms.iter().any(|t| same(t.0, q.pos)) {
                    terms.push((q.pos, Some(q.dir)));
                }
            }
            for &q in &labels {
                if on_group(q) && !terms.iter().any(|t| same(t.0, q)) {
                    terms.push((q, None));
                }
            }
            terms.sort_by_key(|t| quant(t.0));
            if terms.len() < 2 {
                keep(&mut plan, &mut fixed_wires, "fewer than two terminals");
                continue;
            }
            units.push(Unit {
                net: net.name.clone(),
                wires: ws.iter().map(|w| w.id).collect(),
                junctions: junctions.iter().filter(|j| on_group(j.1)).map(|j| j.0).collect(),
                template: (*t).clone(),
                old_lines: ws.iter().map(|w| w.points.clone()).collect(),
                old_junctions: junctions.iter().filter(|j| on_group(j.1)).map(|j| j.1).collect(),
                terms,
                leads: Vec::new(),
            });
        }
    }
    for name in &opts.nets {
        if !nets.iter().any(|n| &n.name == name) {
            plan.unchanged.push((name.clone(), "no such net".to_string()));
        }
    }

    // 動かない配線と、引き直さないネットのジャンクション
    let mut occ = Occ::new(&grid);
    for id in &fixed_wires {
        if let Some(w) = wires.get(id) {
            apply_raster(&mut occ, &rasterize(&grid, &w.points));
        }
    }
    let moving: HashSet<EntityId> = units.iter().flat_map(|u| u.junctions.iter().copied()).collect();
    for &(id, at) in &junctions {
        if !moving.contains(&id) {
            apply_raster(&mut occ, &block_point(&grid, at));
        }
    }

    // 引き出し線。出られない端子のある単位は引き直せない
    let mut failed: Vec<bool> = vec![false; units.len()];
    for (i, u) in units.iter_mut().enumerate() {
        let leads: Option<Vec<Lead>> = u.terms.iter().map(|&(pos, dir)| make_lead(&grid, pos, dir)).collect();
        match leads {
            Some(l) => {
                for lead in &l {
                    occ.reserved[lead.node] = i as u32;
                }
                u.leads = l;
            }
            None => failed[i] = true,
        }
    }

    // 順序: 端子が少なく、範囲が小さい単位から。同点は名前と位置で決める
    let span = |u: &Unit| -> i64 {
        let (x0, x1) = u.terms.iter().fold((f64::MAX, f64::MIN), |(a, b), t| (a.min(t.0.x), b.max(t.0.x)));
        let (y0, y1) = u.terms.iter().fold((f64::MAX, f64::MIN), |(a, b), t| (a.min(t.0.y), b.max(t.0.y)));
        ((x1 - x0 + y1 - y0) * 1000.0).round() as i64
    };
    let mut order: Vec<usize> = (0..units.len()).collect();
    order.sort_by(|&a, &b| {
        let (ua, ub) = (&units[a], &units[b]);
        (ua.terms.len(), span(ua), &ua.net, quant(ua.terms[0].0), a).cmp(&(ub.terms.len(), span(ub), &ub.net, quant(ub.terms[0].0), b))
    });

    let mut searcher = Searcher::new(&grid);
    let mut routes: Vec<Option<UnitRoute>> = (0..units.len()).map(|_| None).collect();
    let mut tries: Vec<u32> = vec![0; units.len()];
    let mut routed: Vec<usize> = Vec::new();
    let mut queue: VecDeque<usize> = order.into_iter().collect();
    let max_rounds = 3;

    // 経路を諦めた単位: 古い配線を動かない障害物にし、それとぶつかる経路は引き直しに回す
    let settle_failed = |first: usize,
                         units: &[Unit],
                         occ: &mut Occ,
                         routes: &mut Vec<Option<UnitRoute>>,
                         routed: &mut Vec<usize>,
                         queue: &mut VecDeque<usize>,
                         failed: &mut Vec<bool>,
                         tries: &mut Vec<u32>| {
        let mut work = vec![first];
        while let Some(u) = work.pop() {
            failed[u] = true;
            let mut entries: Vec<(usize, u8, bool)> = Vec::new();
            for line in &units[u].old_lines {
                entries.extend(rasterize(&grid, line));
            }
            for &q in &units[u].old_junctions {
                entries.extend(block_point(&grid, q));
            }
            let hit: Vec<usize> = routed.iter().copied().filter(|&v| routes[v].as_ref().is_some_and(|r| conflicts(r, &entries))).collect();
            for v in hit {
                if let Some(r) = routes[v].take() {
                    remove_route(occ, &r);
                }
                routed.retain(|&x| x != v);
                tries[v] += 1;
                if tries[v] > max_rounds {
                    work.push(v);
                } else {
                    queue.push_front(v);
                }
            }
            apply_raster(occ, &entries);
        }
    };

    for u in 0..units.len() {
        if failed[u] {
            let mut work_queue = VecDeque::new();
            settle_failed(u, &units, &mut occ, &mut routes, &mut routed, &mut work_queue, &mut failed, &mut tries);
            for v in work_queue.into_iter().rev() {
                queue.push_front(v);
            }
        }
    }
    queue.retain(|&u| !failed[u]);

    while let Some(u) = queue.pop_front() {
        if failed[u] {
            continue;
        }
        if let Some(r) = route_unit(&grid, &occ, &mut searcher, &units[u]) {
            add_route(&mut occ, &r);
            routes[u] = Some(r);
            routed.push(u);
            continue;
        }
        // 引き直し: 邪魔をしている最新の経路をいったん外し、この単位を引いてから、外した単位を引き直す
        let want = {
            let t = &units[u].terms;
            let (x0, x1) = t.iter().fold((f64::MAX, f64::MIN), |(a, b), q| (a.min(q.0.x), b.max(q.0.x)));
            let (y0, y1) = t.iter().fold((f64::MAX, f64::MIN), |(a, b), q| (a.min(q.0.y), b.max(q.0.y)));
            let m = 20.0 / grid.opts.grid_mm;
            (
                (x0 / grid.opts.grid_mm - m) as i64,
                (y0 / grid.opts.grid_mm - m) as i64,
                (x1 / grid.opts.grid_mm + m) as i64,
                (y1 / grid.opts.grid_mm + m) as i64,
            )
        };
        let mut tried: Vec<usize> = Vec::new();
        let mut done = false;
        for _ in 0..max_rounds {
            let victim = routed.iter().rev().copied().find(|&v| {
                !tried.contains(&v)
                    && tries[v] < max_rounds
                    && routes[v].as_ref().is_some_and(|r| r.bbox.0 <= want.2 && r.bbox.2 >= want.0 && r.bbox.1 <= want.3 && r.bbox.3 >= want.1)
            });
            let Some(v) = victim else { break };
            tried.push(v);
            let at = routed.iter().position(|&x| x == v).unwrap_or(0);
            let rv = routes[v].take().expect("routed");
            remove_route(&mut occ, &rv);
            routed.remove(at);
            if let Some(r) = route_unit(&grid, &occ, &mut searcher, &units[u]) {
                add_route(&mut occ, &r);
                routes[u] = Some(r);
                routed.push(u);
                tries[v] += 1;
                queue.push_front(v);
                done = true;
                break;
            }
            add_route(&mut occ, &rv);
            routes[v] = Some(rv);
            routed.insert(at, v);
        }
        if !done {
            let mut work_queue = VecDeque::new();
            settle_failed(u, &units, &mut occ, &mut routes, &mut routed, &mut work_queue, &mut failed, &mut tries);
            for v in work_queue.into_iter().rev() {
                queue.push_front(v);
            }
            queue.retain(|&x| !failed[x]);
        }
    }

    // 結果
    let mut names: BTreeSet<String> = BTreeSet::new();
    for (i, u) in units.iter().enumerate() {
        match (&routes[i], failed[i]) {
            (Some(r), false) => {
                plan.remove.extend(u.wires.iter().copied());
                plan.remove.extend(u.junctions.iter().copied());
                plan.add.extend(emit(u, r));
                names.insert(u.net.clone());
            }
            _ => plan.unchanged.push((u.net.clone(), "no route found; the old wires are kept".to_string())),
        }
    }
    plan.rerouted = names.into_iter().collect();
    plan
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

    // ------------------------------------------------------------------
    // ネットの木・他のネット・計画 (タスク2)
    // ------------------------------------------------------------------

    use std::collections::BTreeSet;

    /// A ground mark: a net-name symbol whose single pin sits on its top edge.
    fn gnd() -> SymbolDef {
        let mut d = def(
            "gnd",
            vec![Primitive::Line { pts: vec![p(0.0, 0.0), p(0.0, 4.0)] }, Primitive::Line { pts: vec![p(-3.0, 4.0), p(3.0, 4.0)] }],
            vec![pin("1", 0.0, 0.0, PinDir::Up)],
        );
        d.net = Some("GND".into());
        d
    }

    fn all_defs() -> Vec<SymbolDef> {
        let mut d = defs();
        d.push(gnd());
        d
    }

    fn old_wire(sheet: &mut Sheet, pts: &[(f64, f64)]) -> EntityId {
        let id = Uuid::new_v4();
        sheet.entities.insert(
            id,
            Entity::Wire(Wire {
                id,
                points: pts.iter().map(|&(x, y)| p(x, y)).collect(),
                color: "black".into(),
                sq: 0.75,
                length_m: None,
                length_source: Default::default(),
                part_no: None,
                net: None,
                dashed: false,
                weight_mm: None,
            }),
        );
        id
    }

    fn old_junction(sheet: &mut Sheet, at: (f64, f64)) -> EntityId {
        let id = Uuid::new_v4();
        sheet.entities.insert(id, Entity::Junction(Junction { id, at: p(at.0, at.1) }));
        id
    }

    fn old_label(sheet: &mut Sheet, at: (f64, f64), name: &str) -> EntityId {
        let id = Uuid::new_v4();
        sheet.entities.insert(id, Entity::NetLabel(NetLabel { id, at: p(at.0, at.1), name: name.into(), rotation: 0, attrs: Default::default() }));
        id
    }

    fn run(sheet: &Sheet) -> ReroutePlan {
        plan_reroute(sheet, &all_defs(), &RerouteOptions::default())
    }

    /// Applies a plan to a copy of the sheet (test only: production code never edits the model).
    fn apply(sheet: &Sheet, plan: &ReroutePlan) -> Sheet {
        let mut out = sheet.clone();
        for id in &plan.remove {
            assert!(out.entities.remove(id).is_some(), "removed id exists");
        }
        for e in &plan.add {
            out.entities.insert(e.id(), e.clone());
        }
        out
    }

    /// The set of pin groups of all nets, e.g. {["R1:2", "R2:1"]}.
    fn pin_sets(sheet: &Sheet) -> BTreeSet<Vec<String>> {
        extract_netlist(sheet, &all_defs())
            .iter()
            .map(|n| {
                let mut v: Vec<String> = n.pins.iter().map(|q| format!("{}:{}", q.reference, q.pin)).collect();
                v.sort();
                v
            })
            .collect()
    }

    fn assert_connectivity_kept(sheet: &Sheet, plan: &ReroutePlan) {
        let after = apply(sheet, plan);
        assert_eq!(pin_sets(sheet), pin_sets(&after));
    }

    fn added_wires(plan: &ReroutePlan) -> Vec<&Wire> {
        plan.add.iter().filter_map(|e| if let Entity::Wire(w) = e { Some(w) } else { None }).collect()
    }

    fn added_junctions(plan: &ReroutePlan) -> Vec<Point> {
        plan.add.iter().filter_map(|e| if let Entity::Junction(j) = e { Some(j.at) } else { None }).collect()
    }

    /// Geometry of a plan without the random ids, for comparing two runs.
    fn shape(plan: &ReroutePlan) -> (Vec<Vec<(i64, i64)>>, Vec<(i64, i64)>, Vec<String>, Vec<(String, String)>) {
        let q = |a: Point| ((a.x * 1000.0).round() as i64, (a.y * 1000.0).round() as i64);
        let wires = added_wires(plan).iter().map(|w| w.points.iter().map(|&a| q(a)).collect()).collect();
        let junctions = added_junctions(plan).into_iter().map(q).collect();
        (wires, junctions, plan.rerouted.clone(), plan.unchanged.clone())
    }

    /// The most branches meeting at any vertex, end or junction of the added wires.
    fn max_branches(plan: &ReroutePlan) -> u32 {
        let segs: Vec<(Point, Point)> = added_wires(plan).iter().flat_map(|w| w.points.windows(2).map(|s| (s[0], s[1]))).collect();
        let mut spots: Vec<Point> = added_wires(plan).iter().flat_map(|w| w.points.iter().copied()).collect();
        spots.extend(added_junctions(plan));
        let mut best = 0;
        for s in spots {
            let mut dirs = 0u8;
            for &(a, b) in &segs {
                let dir = Dir::between(a, b).expect("segment has length");
                if same(s, a) {
                    dirs |= dir.bit();
                } else if same(s, b) {
                    dirs |= dir.opp().bit();
                } else if crate::netlist::segment_distance(&s, &a, &b) < CONNECT_EPS {
                    dirs |= dir.bit() | dir.opp().bit();
                }
            }
            best = best.max(dirs.count_ones());
        }
        best
    }

    fn net_of_pin(sheet: &Sheet, reference: &str, number: &str) -> crate::netlist::Net {
        extract_netlist(sheet, &all_defs())
            .into_iter()
            .find(|n| n.pins.iter().any(|q| q.reference == reference && q.pin == number))
            .expect("net of pin")
    }

    fn wire_ends(sheet: &Sheet, net: &crate::netlist::Net) -> Vec<Point> {
        net.wire_ids
            .iter()
            .filter_map(|id| if let Some(Entity::Wire(w)) = sheet.entities.get(id) { Some(w) } else { None })
            .flat_map(|w| [w.points[0], *w.points.last().unwrap()])
            .collect()
    }

    /// Two nets that cross: R1.2 to R4.1 and R2.2 to R3.1, drawn as long diagonals.
    fn crossing_pair() -> Sheet {
        let mut sheet = new_sheet();
        place(&mut sheet, "box2", (50.0, 50.0), "R1");
        place(&mut sheet, "box2", (50.0, 70.0), "R2");
        place(&mut sheet, "box2", (100.0, 50.0), "R3");
        place(&mut sheet, "box2", (100.0, 70.0), "R4");
        old_wire(&mut sheet, &[(60.0, 50.0), (90.0, 70.0)]);
        old_wire(&mut sheet, &[(60.0, 70.0), (90.0, 50.0)]);
        sheet
    }

    /// A closed room: its left and right ends are walls touching R1 and R2, so a wire from the top half to the bottom half must cross the line between R1 and R2.
    fn closed_room() -> Sheet {
        let mut sheet = new_sheet();
        place(&mut sheet, "box2", (45.0, 50.0), "R1");
        place(&mut sheet, "box2", (135.0, 50.0), "R2");
        place(&mut sheet, "cap", (90.0, 25.0), "C1");
        place(&mut sheet, "cap", (90.0, 75.0), "C2");
        for x in [65.0, 115.0] {
            place(&mut sheet, "wall_h", (x, 12.5), "");
            place(&mut sheet, "wall_h", (x, 87.5), "");
        }
        for y in [30.0, 70.0] {
            place(&mut sheet, "wall_v", (32.5, y), "");
            place(&mut sheet, "wall_v", (147.5, y), "");
        }
        old_wire(&mut sheet, &[(55.0, 50.0), (125.0, 50.0)]);
        old_wire(&mut sheet, &[(90.0, 35.0), (90.0, 65.0)]);
        sheet
    }

    /// Two resistors in a row whose pins face each other; the wire between them is a long loop.
    fn two_resistors_with_loop() -> (Sheet, EntityId) {
        let mut sheet = new_sheet();
        place(&mut sheet, "box2", (50.0, 50.0), "R1");
        place(&mut sheet, "box2", (100.0, 70.0), "R2");
        let id = old_wire(&mut sheet, &[(60.0, 50.0), (60.0, 30.0), (75.0, 30.0), (75.0, 70.0), (90.0, 70.0)]);
        (sheet, id)
    }

    /// A two-pin net with a long detour is replaced by one new wire that is orthogonal and starts and ends exactly on the pins.
    /// 遠回りしている2ピンのネットは、ピンぴったりに始まり終わる、直交する1本の新しい配線に置き換わる。
    #[test]
    fn a_two_pin_net_is_replaced_by_one_orthogonal_wire() {
        let (sheet, old) = two_resistors_with_loop();
        let plan = run(&sheet);
        assert_eq!(plan.remove, vec![old]);
        let wires = added_wires(&plan);
        assert_eq!(wires.len(), 1, "{plan:?}");
        assert!(orthogonal(&wires[0].points), "{:?}", wires[0].points);
        assert_eq!((wires[0].points[0], *wires[0].points.last().unwrap()), (p(60.0, 50.0), p(90.0, 70.0)));
        assert!(plan.add.iter().all(|e| matches!(e, Entity::Wire(_))));
        assert_eq!((plan.rerouted.len(), plan.unchanged.len()), (1, 0));
        assert_connectivity_kept(&sheet, &plan);
    }

    /// A net with three terminals becomes a tree with exactly one T, and the T gets a junction dot.
    /// 端子が3つのネットは、T字の分岐が1か所だけの木になり、分岐点にはジャンクションの点が付く。
    #[test]
    fn a_three_terminal_net_gets_one_t_and_a_junction() {
        let mut sheet = new_sheet();
        place(&mut sheet, "box2", (50.0, 50.0), "R1");
        place(&mut sheet, "box2", (110.0, 50.0), "R2");
        place(&mut sheet, "cap", (80.0, 80.0), "C1");
        old_wire(&mut sheet, &[(60.0, 50.0), (80.0, 50.0)]);
        old_wire(&mut sheet, &[(80.0, 50.0), (100.0, 50.0)]);
        old_wire(&mut sheet, &[(80.0, 50.0), (80.0, 70.0)]);
        old_junction(&mut sheet, (80.0, 50.0));
        let plan = run(&sheet);
        assert_eq!(plan.remove.len(), 4);
        assert_eq!(added_junctions(&plan).len(), 1, "{plan:?}");
        assert_eq!(added_wires(&plan).len(), 3);
        assert_eq!(max_branches(&plan), 3);
        assert_connectivity_kept(&sheet, &plan);
    }

    /// Four terminals around one point never meet in a four-way point: the tree makes two Ts, each with a junction.
    /// 1点のまわりに4つの端子があっても、4本が1点で出会う十字にはならず、ジャンクション付きのT字が2か所になる。
    #[test]
    fn four_terminals_around_one_point_never_make_a_four_way_point() {
        let mut sheet = new_sheet();
        place(&mut sheet, "box2", (50.0, 50.0), "R1");
        place(&mut sheet, "box2", (120.0, 50.0), "R2");
        place(&mut sheet, "cap", (85.0, 20.0), "C1");
        place(&mut sheet, "cap", (85.0, 80.0), "C2");
        old_wire(&mut sheet, &[(60.0, 50.0), (85.0, 50.0)]);
        old_wire(&mut sheet, &[(110.0, 50.0), (85.0, 50.0)]);
        old_wire(&mut sheet, &[(85.0, 30.0), (85.0, 50.0)]);
        old_wire(&mut sheet, &[(85.0, 70.0), (85.0, 50.0)]);
        old_junction(&mut sheet, (85.0, 50.0));
        let plan = run(&sheet);
        assert_eq!(plan.rerouted.len(), 1, "{plan:?}");
        assert!(max_branches(&plan) <= 3, "{plan:?}");
        assert_eq!(added_junctions(&plan).len(), 2, "{plan:?}");
        assert_connectivity_kept(&sheet, &plan);
    }

    /// A new route never passes over a pin of another part, because the netlist would join that pin to the wire.
    /// 新しい配線は、別の部品のピンの上を通らない。通るとネットリスト上でそのピンが配線につながってしまうため。
    #[test]
    fn a_route_never_passes_over_another_nets_pin() {
        let mut sheet = new_sheet();
        place(&mut sheet, "box2", (50.0, 50.0), "R1");
        place(&mut sheet, "box2", (90.0, 50.0), "R2");
        place(&mut sheet, "box2", (130.0, 50.0), "R3");
        old_wire(&mut sheet, &[(60.0, 50.0), (60.0, 30.0), (120.0, 30.0), (120.0, 50.0)]);
        let plan = run(&sheet);
        assert_eq!(plan.rerouted.len(), 1, "{plan:?}");
        let (a, b) = (pin_of(&sheet, &all_defs(), "R2", "1").pos, pin_of(&sheet, &all_defs(), "R2", "2").pos);
        for w in added_wires(&plan) {
            assert!(!on_wire(w, &a) && !on_wire(w, &b), "{:?}", w.points);
        }
        assert_connectivity_kept(&sheet, &plan);
    }

    /// A new route never runs over a lone net label, because the netlist would join the label to the wire.
    /// 新しい配線は、ぽつんと置かれたネットラベルの上を通らない。通るとネットリスト上でラベルが配線につながってしまうため。
    #[test]
    fn a_route_never_passes_over_a_lone_net_label() {
        let mut sheet = new_sheet();
        place(&mut sheet, "box2", (50.0, 50.0), "R1");
        place(&mut sheet, "box2", (100.0, 50.0), "R2");
        old_wire(&mut sheet, &[(60.0, 50.0), (60.0, 30.0), (90.0, 30.0), (90.0, 50.0)]);
        old_label(&mut sheet, (75.3, 50.0), "LONE");
        let plan = run(&sheet);
        assert_eq!(plan.rerouted.len(), 1, "{plan:?}");
        for w in added_wires(&plan) {
            assert!(!on_wire(w, &p(75.3, 50.0)), "{:?}", w.points);
        }
        assert_connectivity_kept(&sheet, &plan);
    }

    /// Two nets that are drawn close together never end up sharing a wire end, because the netlist would merge them.
    /// 近くに描かれた2つのネットの新しい配線は、端点を共有しない。共有するとネットリスト上で1つに合体してしまうため。
    #[test]
    fn two_nets_never_share_a_wire_endpoint() {
        let sheet = crossing_pair();
        let plan = run(&sheet);
        assert_eq!(plan.rerouted.len(), 2, "{plan:?}");
        let after = apply(&sheet, &plan);
        let (n1, n2) = (net_of_pin(&after, "R1", "2"), net_of_pin(&after, "R2", "2"));
        let (e1, e2) = (wire_ends(&after, &n1), wire_ends(&after, &n2));
        assert!(!e1.is_empty() && !e2.is_empty());
        for a in &e1 {
            assert!(!e2.iter().any(|b| same(*a, *b)), "shared end {a:?}");
        }
        assert_connectivity_kept(&sheet, &plan);
    }

    /// When the only way is across another net, the wire crosses it at a right angle without touching it.
    /// 他のネットを横切るしかないときは、直角に横切るだけで、つながらない。
    #[test]
    fn a_perpendicular_crossing_is_used_when_it_cannot_be_avoided() {
        let sheet = closed_room();
        let plan = run(&sheet);
        assert_eq!(plan.rerouted.len(), 2, "{plan:?}");
        let after = apply(&sheet, &plan);
        let (horizontal, vertical) = (net_of_pin(&after, "R1", "2"), net_of_pin(&after, "C1", "2"));
        let segs = |net: &crate::netlist::Net| -> Vec<(Point, Point)> {
            net.wire_ids
                .iter()
                .filter_map(|id| if let Some(Entity::Wire(w)) = after.entities.get(id) { Some(w) } else { None })
                .flat_map(|w| w.points.windows(2).map(|s| (s[0], s[1])).collect::<Vec<_>>())
                .collect()
        };
        let crossings = segs(&horizontal)
            .iter()
            .flat_map(|h| segs(&vertical).into_iter().map(move |v| (*h, v)))
            .filter(|((h0, h1), (v0, v1))| {
                h0.y == h1.y && v0.x == v1.x && v0.x > h0.x.min(h1.x) && v0.x < h0.x.max(h1.x) && h0.y > v0.y.min(v1.y) && h0.y < v0.y.max(v1.y)
            })
            .count();
        assert_eq!(crossings, 1);
        assert_connectivity_kept(&sheet, &plan);
    }

    /// A net whose wires have different colours is left as it is, and the reply says why.
    /// 線の色がまちまちなネットは、そのまま残し、理由を知らせる。
    #[test]
    fn a_net_with_mixed_wire_colours_is_left_unchanged_with_a_reason() {
        let (mut sheet, old) = two_resistors_with_loop();
        let second = old_wire(&mut sheet, &[(60.0, 50.0), (60.0, 45.0)]);
        let _ = second;
        if let Some(Entity::Wire(w)) = sheet.entities.get_mut(&old) {
            w.color = "red".into();
        }
        let plan = run(&sheet);
        assert!(plan.remove.is_empty() && plan.add.is_empty(), "{plan:?}");
        assert!(plan.rerouted.is_empty());
        assert_eq!(plan.unchanged.len(), 1);
        assert!(plan.unchanged[0].1.contains("colour"), "{:?}", plan.unchanged);
    }

    /// A net whose wire is used by a cable run is left as it is, so the cable keeps its wire.
    /// ケーブルの区間が使っている配線のネットは、ケーブルが配線を失わないよう、そのまま残す。
    #[test]
    fn a_net_used_by_a_cable_run_is_left_unchanged() {
        let (mut sheet, old) = two_resistors_with_loop();
        let id = Uuid::new_v4();
        sheet.entities.insert(
            id,
            Entity::Cable(Cable {
                id,
                reference: "W1".into(),
                part_no: String::new(),
                core_count: 1,
                sq: 0.0,
                points: vec![],
                cores: vec![CableCore { core: "1".into(), end_a: Some(CoreEnd::Wire { wire: old }), ..Default::default() }],
                shield: None,
                length_m: None,
                run: vec![p(10.0, 100.0), p(20.0, 100.0)],
            }),
        );
        let plan = run(&sheet);
        assert!(plan.remove.is_empty() && plan.add.is_empty(), "{plan:?}");
        assert_eq!(plan.unchanged.len(), 1);
        assert!(plan.unchanged[0].1.contains("cable"), "{:?}", plan.unchanged);
    }

    /// A net that cannot be routed keeps its old wires and is reported.
    /// 経路が見つからないネットは、古い配線をそのまま残し、理由を知らせる。
    #[test]
    fn an_unroutable_net_keeps_its_old_wires() {
        let mut sheet = new_sheet();
        place(&mut sheet, "box2", (50.0, 50.0), "R1");
        place(&mut sheet, "box2", (130.0, 50.0), "R2");
        place(&mut sheet, "wall_h", (130.0, 35.0), "");
        place(&mut sheet, "wall_h", (130.0, 65.0), "");
        place(&mut sheet, "wall_v", (97.5, 50.0), "");
        place(&mut sheet, "wall_v", (162.5, 50.0), "");
        old_wire(&mut sheet, &[(60.0, 50.0), (120.0, 50.0)]);
        let plan = run(&sheet);
        assert!(plan.remove.is_empty() && plan.add.is_empty(), "{plan:?}");
        assert_eq!(plan.unchanged.len(), 1);
        assert!(plan.unchanged[0].1.contains("no route"), "{:?}", plan.unchanged);
    }

    /// Running the planner twice on the same sheet gives the same wires, in the same order.
    /// 同じ図面に2回かけても、同じ配線が同じ順序で得られる。
    #[test]
    fn the_result_is_deterministic() {
        let sheet = crossing_pair();
        assert!(!shape(&run(&sheet)).0.is_empty());
        assert_eq!(shape(&run(&sheet)), shape(&run(&sheet)));
        let room = closed_room();
        assert_eq!(shape(&run(&room)), shape(&run(&room)));
    }

    /// A net named in the request is rerouted and the others are reported as not selected; an unknown name is reported too.
    /// 名前を指定したネットだけを引き直し、ほかのネットは「選ばれていない」と知らせる。存在しない名前もそう知らせる。
    #[test]
    fn only_the_named_nets_are_rerouted() {
        let sheet = crossing_pair();
        let nets = extract_netlist(&sheet, &all_defs());
        let first = nets[0].name.clone();
        let opts = RerouteOptions { nets: vec![first.clone(), "NO_SUCH_NET".into()], ..Default::default() };
        let plan = plan_reroute(&sheet, &all_defs(), &opts);
        assert_eq!(plan.rerouted, vec![first]);
        assert!(plan.unchanged.iter().any(|(n, r)| n == &nets[1].name && r.contains("not selected")), "{:?}", plan.unchanged);
        assert!(plan.unchanged.iter().any(|(n, r)| n == "NO_SUCH_NET" && r.contains("no such net")), "{:?}", plan.unchanged);
        assert_connectivity_kept(&sheet, &plan);
    }

    /// New wires carry the colour, cross-section, part number, wire number, dash, weight and length of the old wires.
    /// 新しい配線には、古い配線の色・線径・品番・線番・破線・太さ・長さがそのまま引き継がれる。
    #[test]
    fn new_wires_keep_the_attributes_of_the_old_wires() {
        let (mut sheet, old) = two_resistors_with_loop();
        if let Some(Entity::Wire(w)) = sheet.entities.get_mut(&old) {
            w.color = "red".into();
            w.sq = 1.5;
            w.part_no = Some("WIRE-15".into());
            w.net = Some("W7".into());
            w.dashed = true;
            w.weight_mm = Some(0.5);
            w.length_m = Some(2.5);
        }
        let plan = run(&sheet);
        let wires = added_wires(&plan);
        assert!(!wires.is_empty());
        for w in wires {
            assert_eq!(
                (w.color.as_str(), w.sq, w.part_no.as_deref(), w.net.as_deref(), w.dashed, w.weight_mm, w.length_m),
                ("red", 1.5, Some("WIRE-15"), Some("W7"), true, Some(0.5), Some(2.5))
            );
        }
    }

    /// A net with a wire end that touches nothing is left as it is, because the new route could not know what that end is for.
    /// 何にもつながっていない配線の端があるネットは、その端が何のためか分からないので、そのまま残す。
    #[test]
    fn a_net_with_a_free_wire_end_is_left_unchanged() {
        let (mut sheet, _) = two_resistors_with_loop();
        old_wire(&mut sheet, &[(60.0, 50.0), (60.0, 45.0)]);
        let plan = run(&sheet);
        assert!(plan.remove.is_empty() && plan.add.is_empty(), "{plan:?}");
        assert_eq!(plan.unchanged.len(), 1);
        assert!(plan.unchanged[0].1.contains("free end"), "{:?}", plan.unchanged);
    }

    /// Several plain nets keep exactly the same pin groups after the reroute.
    /// ふつうのネットが何本あっても、引き直した後のピンのまとまりは元とまったく同じになる。
    #[test]
    fn plain_nets_keep_the_same_pin_groups() {
        let sheet = crossing_pair();
        let plan = run(&sheet);
        assert!(!plan.add.is_empty());
        assert_connectivity_kept(&sheet, &plan);
        let room = closed_room();
        let plan = run(&room);
        assert!(!plan.add.is_empty());
        assert_connectivity_kept(&room, &plan);
    }

    /// Nets joined only by net labels keep their pins, and each label stays on its wire.
    /// ネットラベルだけでつながったネットも、ピンのまとまりは変わらず、ラベルは配線の上に残る。
    #[test]
    fn nets_joined_by_net_labels_keep_their_pins_and_labels() {
        let mut sheet = new_sheet();
        place(&mut sheet, "box2", (50.0, 50.0), "R1");
        place(&mut sheet, "box2", (150.0, 90.0), "R2");
        old_wire(&mut sheet, &[(60.0, 50.0), (60.0, 35.0), (80.0, 35.0)]);
        old_label(&mut sheet, (80.0, 35.0), "SIG");
        old_wire(&mut sheet, &[(140.0, 90.0), (120.0, 90.0), (120.0, 70.0)]);
        old_label(&mut sheet, (120.0, 70.0), "SIG");
        let plan = run(&sheet);
        assert_eq!(plan.rerouted, vec!["SIG".to_string()], "{plan:?}");
        assert_connectivity_kept(&sheet, &plan);
        let after = apply(&sheet, &plan);
        for at in [p(80.0, 35.0), p(120.0, 70.0)] {
            assert!(crate::netlist::label_is_attached(&after, &all_defs(), &at), "label at {at:?} is not on a wire");
        }
        assert_eq!(net_of_pin(&after, "R1", "2").name, "SIG");
    }

    /// Nets joined through ground marks (net-name symbols) keep their pins, and the marks stay connected to their wires.
    /// 接地の印 (ネット名を持つシンボル) でつながったネットも、ピンのまとまりは変わらず、印は配線につながったままになる。
    #[test]
    fn nets_joined_by_ground_marks_keep_their_pins() {
        let mut sheet = new_sheet();
        place(&mut sheet, "box2", (50.0, 50.0), "R1");
        place(&mut sheet, "box2", (130.0, 50.0), "R2");
        place(&mut sheet, "gnd", (60.0, 90.0), "G1");
        place(&mut sheet, "gnd", (140.0, 80.0), "G2");
        old_wire(&mut sheet, &[(60.0, 50.0), (75.0, 50.0), (75.0, 70.0), (60.0, 70.0), (60.0, 90.0)]);
        old_wire(&mut sheet, &[(140.0, 50.0), (150.0, 50.0), (150.0, 65.0), (140.0, 65.0), (140.0, 80.0)]);
        let plan = run(&sheet);
        assert_eq!(plan.rerouted, vec!["GND".to_string()], "{plan:?}");
        assert!(!plan.add.is_empty());
        assert_connectivity_kept(&sheet, &plan);
    }

    /// A tiny pseudo-random generator so the generated sheets are the same on every run.
    fn lcg(seed: &mut u64) -> usize {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (*seed >> 33) as usize
    }

    /// For many generated sheets (a grid of parts joined by random diagonal wires), the reroute keeps every net's pin group.
    /// 部品を格子に並べてランダムに配線した多数の図面で、引き直した後もネットのピンのまとまりは元と同じになる。
    #[test]
    fn generated_sheets_keep_the_same_pin_groups() {
        for seed in 1..=25u64 {
            let mut state = seed * 7919;
            let mut sheet = new_sheet();
            let mut pins: Vec<Point> = Vec::new();
            for row in 0..3 {
                for col in 0..4 {
                    let at = (50.0 + col as f64 * 55.0, 50.0 + row as f64 * 45.0);
                    let name = format!("R{}", row * 4 + col + 1);
                    place(&mut sheet, if (row + col) % 2 == 0 { "box2" } else { "cap" }, at, &name);
                }
            }
            for q in pin_points(&sheet, &all_defs()) {
                pins.push(q.pos);
            }
            for _ in 0..(3 + lcg(&mut state) % 6) {
                let (a, b) = (pins[lcg(&mut state) % pins.len()], pins[lcg(&mut state) % pins.len()]);
                if same(a, b) {
                    continue;
                }
                old_wire(&mut sheet, &[(a.x, a.y), (b.x, b.y)]);
            }
            let plan = run(&sheet);
            assert_connectivity_kept(&sheet, &plan);
            for w in added_wires(&plan) {
                assert!(orthogonal(&w.points), "seed {seed}: {:?}", w.points);
            }
            assert!(max_branches(&plan) <= 3, "seed {seed}");
        }
    }

    /// A part turned by 90 degrees sends its wire out in the turned pin direction, so the wire first runs down from a pin that points down.
    /// 90度回した部品は、回したあとのピンの向きに配線を出す。下向きになったピンからは、まず下へ線が出る。
    #[test]
    fn a_rotated_part_sends_its_wire_out_in_the_turned_direction() {
        let mut sheet = new_sheet();
        let r1 = place(&mut sheet, "box2", (50.0, 50.0), "R1");
        if let Some(Entity::Symbol(s)) = sheet.entities.get_mut(&r1) {
            s.rotation = 90;
        }
        place(&mut sheet, "box2", (100.0, 80.0), "R2");
        let start = pin_of(&sheet, &all_defs(), "R1", "2").pos;
        old_wire(&mut sheet, &[(start.x, start.y), (90.0, 80.0)]);
        let plan = run(&sheet);
        let wires = added_wires(&plan);
        let first = wires.iter().find(|w| same(w.points[0], start) || same(*w.points.last().unwrap(), start)).expect("wire at the pin");
        let pts: Vec<Point> = if same(first.points[0], start) { first.points.clone() } else { first.points.iter().rev().copied().collect() };
        assert_eq!(pts[0].x, pts[1].x, "{pts:?}");
        assert!(pts[1].y > pts[0].y, "{pts:?}");
        assert_connectivity_kept(&sheet, &plan);
    }

    /// A pin that is not on the routing grid keeps a wire that starts exactly on the pin.
    /// 格子の上にないピンにも、ピンの位置ぴったりから始まる配線がつく。
    #[test]
    fn a_wire_at_an_off_grid_pin_starts_exactly_on_the_pin() {
        let mut sheet = new_sheet();
        place(&mut sheet, "freebox", (50.0, 50.0), "F1");
        place(&mut sheet, "box2", (100.0, 70.0), "R2");
        let start = pin_of(&sheet, &all_defs(), "F1", "1").pos;
        old_wire(&mut sheet, &[(start.x, start.y), (90.0, 70.0)]);
        let plan = run(&sheet);
        assert_eq!(plan.rerouted.len(), 1, "{plan:?}");
        assert!(added_wires(&plan).iter().any(|w| w.points[0] == start || *w.points.last().unwrap() == start), "{plan:?}");
        assert_connectivity_kept(&sheet, &plan);
    }

    /// A net that is left unchanged stays where it is and the other nets route around it without touching it.
    /// そのまま残すネットの配線は動かさず、ほかのネットはそれにつながらないように避けて引かれる。
    #[test]
    fn a_net_left_unchanged_is_avoided_by_the_other_nets() {
        let mut sheet = new_sheet();
        place(&mut sheet, "box2", (50.0, 50.0), "R1");
        place(&mut sheet, "box2", (100.0, 50.0), "R2");
        place(&mut sheet, "cap", (75.0, 20.0), "C1");
        place(&mut sheet, "cap", (75.0, 80.0), "C2");
        let red = old_wire(&mut sheet, &[(60.0, 50.0), (75.0, 50.0)]);
        old_wire(&mut sheet, &[(75.0, 50.0), (90.0, 50.0)]);
        if let Some(Entity::Wire(w)) = sheet.entities.get_mut(&red) {
            w.color = "red".into();
        }
        old_wire(&mut sheet, &[(75.0, 30.0), (75.0, 70.0)]);
        let plan = run(&sheet);
        assert_eq!(plan.unchanged.len(), 1, "{plan:?}");
        assert!(!plan.remove.contains(&red));
        assert_eq!(plan.rerouted.len(), 1, "{plan:?}");
        assert_connectivity_kept(&sheet, &plan);
    }
}
