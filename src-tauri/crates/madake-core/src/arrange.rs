//! 部品の並べ直し (整列と間隔の調整) の幾何の部品。
//!
//! 一緒に動くもの (単位)、列・行 (帯)、帯のずらし量、単調な座標の写像、整列の移動量を求める。
//! この模塊の関数は全て**純粋**で、図面 (モデル) を書き換えない。

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::frame_template::FrameTemplate;
use crate::geometry::Point;
use crate::leader::entity_bounds;
use crate::model::{Entity, EntityId, NetLabel, Sheet, SheetKind, Wire};
use crate::netlist::{extract_netlist, on_wire, CONNECT_EPS};
use crate::route::{
    collect_route_units, emit, make_lead, pin_points, route_units, simplify, Collected, Dir, KeepReason, KeptCluster, PinPt, RouteUnit,
    RerouteOptions, RouteGrid, RouteOptions,
};
use crate::symbol::{DefIndex, SymbolDef};

/// 移動量の刻み (mm)。ピンが格子の上に残るよう、全ての移動はこの倍数にする。
pub const STEP_MM: f64 = 2.5;
/// 近くの文字を部品の付属とみなす距離 (mm)。
pub const SATELLITE_TEXT_MM: f64 = 3.0;

/// 並べ直しの設定。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default)]
pub struct ArrangeOptions {
    /// 列と列・行と行の間にあける通り道の幅 mm (既定8)。
    pub channel_mm: f64,
    /// これより広い空きはこの幅まで詰める mm (既定20)。
    pub max_gap_mm: f64,
    /// 中心の差がこれ未満の部品をそろえる mm (既定4)。
    pub align_mm: f64,
    /// 経路探索の設定 (引き直しと同じ)。
    pub route: RouteOptions,
}

impl Default for ArrangeOptions {
    fn default() -> Self {
        Self { channel_mm: 8.0, max_gap_mm: 20.0, align_mm: 4.0, route: RouteOptions::default() }
    }
}

impl ArrangeOptions {
    /// 設定の範囲を確かめる。外れていれば項目名を含む日本語のメッセージを返す。
    pub fn validate(&self) -> Result<(), String> {
        if !self.channel_mm.is_finite() || !(STEP_MM..=50.0).contains(&self.channel_mm) {
            return Err(format!("channel_mm は 2.5〜50 mm の範囲で指定してください (指定値: {})", self.channel_mm));
        }
        if !self.max_gap_mm.is_finite() || self.max_gap_mm < self.channel_mm || self.max_gap_mm > 200.0 {
            return Err(format!(
                "max_gap_mm は channel_mm 以上 200 mm 以下で指定してください (指定値: {})",
                self.max_gap_mm
            ));
        }
        if !self.align_mm.is_finite() || !(0.0..=20.0).contains(&self.align_mm) {
            return Err(format!("align_mm は 0〜20 mm の範囲で指定してください (指定値: {})", self.align_mm));
        }
        self.route.validate()
    }
}

/// 一緒に動くもの: 主のシンボル (またはラベル・1ピンのシンボル) と、その衛星・近くの文字。
#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    pub main: EntityId,
    /// main を含む、id順。
    pub members: Vec<EntityId>,
    /// main の位置 (シンボルの at、ラベルの at)。
    pub anchor: Point,
    /// (x0, y0, x1, y1) 全メンバーの外接。
    pub bounds: (f64, f64, f64, f64),
}

fn anchor_of(e: &Entity) -> Option<Point> {
    match e {
        Entity::Symbol(s) => Some(s.at),
        Entity::NetLabel(l) => Some(l.at),
        _ => None,
    }
}

fn rect_tuple(r: &crate::tidy::Rect) -> (f64, f64, f64, f64) {
    (r.min.x, r.min.y, r.max.x, r.max.y)
}

/// 点から矩形までの距離 (中なら0)。
fn dist_to_rect(p: &Point, b: &(f64, f64, f64, f64)) -> f64 {
    let dx = (b.0 - p.x).max(0.0).max(p.x - b.2);
    let dy = (b.1 - p.y).max(0.0).max(p.y - b.3);
    dx.hypot(dy)
}

/// 名前では結ばない、配線で物理的につながったまとまり (ピンとネットラベル)。
#[derive(Debug, Default, Clone)]
struct PhysGroup {
    /// (シンボルid, ピン番号)
    pins: BTreeSet<(EntityId, String)>,
    labels: BTreeSet<EntityId>,
}

/// シートを、名前では結ばない物理的なまとまりに分ける。つなぎ方はネットリストと同じで、同名のラベルどうし
/// (ネット名シンボルどうしも) だけは結ばない。ネット名シンボルのピンは、同じ位置の他のピンとつながる
/// (ネットリストの暗黙のラベルと同じ働き)。
fn phys_groups(sheet: &Sheet, defs: &[SymbolDef]) -> Vec<PhysGroup> {
    let idx = DefIndex::new(defs);
    let pp = pin_points(sheet, defs);
    let named: BTreeSet<EntityId> = sheet
        .entities
        .values()
        .filter_map(|e| match e {
            Entity::Symbol(s) if idx.get(s).is_some_and(|d| d.is_net_mark()) => Some(s.id),
            _ => None,
        })
        .collect();
    let wires: Vec<&Wire> = sheet.entities.values().filter_map(|e| if let Entity::Wire(w) = e { Some(w) } else { None }).collect();
    let juncs: Vec<Point> = sheet.entities.values().filter_map(|e| if let Entity::Junction(j) = e { Some(j.at) } else { None }).collect();
    let labels: Vec<&NetLabel> = sheet.entities.values().filter_map(|e| if let Entity::NetLabel(l) = e { Some(l) } else { None }).collect();
    // 節点の番号: [配線][ピン][ジャンクション][ラベル]
    let (pi, ji) = (wires.len(), wires.len() + pp.len());
    let li = ji + juncs.len();
    let mut parent: Vec<usize> = (0..li + labels.len()).collect();
    fn find(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let mut union = |a: usize, b: usize| {
        let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
        if ra != rb {
            parent[ra.max(rb)] = ra.min(rb);
        }
    };
    let ends = |w: &Wire| -> Vec<Point> { [w.points.first(), w.points.last()].into_iter().flatten().copied().collect() };
    for a in 0..wires.len() {
        for b in a + 1..wires.len() {
            if ends(wires[a]).iter().any(|x| ends(wires[b]).iter().any(|y| near(*x, *y))) {
                union(a, b);
            }
        }
        for (j, q) in juncs.iter().enumerate() {
            if on_wire(wires[a], q) {
                union(a, ji + j);
            }
        }
        for (p, q) in pp.iter().enumerate() {
            if on_wire(wires[a], &q.pos) {
                union(a, pi + p);
            }
        }
        for (l, q) in labels.iter().enumerate() {
            if on_wire(wires[a], &q.at) {
                union(a, li + l);
            }
        }
    }
    for (p, a) in pp.iter().enumerate() {
        for (p2, b) in pp.iter().enumerate().skip(p + 1) {
            let same_pin = a.entity == b.entity && a.number == b.number;
            let implicit = (named.contains(&a.entity) || named.contains(&b.entity)) && near(a.pos, b.pos);
            if same_pin || implicit {
                union(pi + p, pi + p2);
            }
        }
        for (l, q) in labels.iter().enumerate() {
            if near(a.pos, q.at) {
                union(pi + p, li + l);
            }
        }
    }
    let mut groups: BTreeMap<usize, PhysGroup> = BTreeMap::new();
    for (p, q) in pp.iter().enumerate() {
        let r = find(&mut parent, pi + p);
        groups.entry(r).or_default().pins.insert((q.entity, q.number.clone()));
    }
    for (l, q) in labels.iter().enumerate() {
        let r = find(&mut parent, li + l);
        groups.entry(r).or_default().labels.insert(q.id);
    }
    groups.into_values().collect()
}

/// シートの単位を、主のid順に返す。
pub fn units(sheet: &Sheet, defs: &[SymbolDef]) -> Vec<Unit> {
    let idx = DefIndex::new(defs);

    // シンボルごとのピン数。
    let pp = pin_points(sheet, defs);
    let mut pin_count: BTreeMap<EntityId, usize> = BTreeMap::new();
    for q in &pp {
        *pin_count.entry(q.entity).or_insert(0) += 1;
    }
    let multi = |id: &EntityId| pin_count.get(id).copied().unwrap_or(0) >= 2;
    let groups = phys_groups(sheet, defs);

    // 位置に重なる他のシンボルのピンから親を選ぶ。複数ピンのシンボルが先で、同じならid順。
    // 1ピンのシンボルどうしは、自分よりidの小さい方だけを親にする (親子が輪にならない)。
    let coincident = |at: Point, me: Option<EntityId>| -> Option<EntityId> {
        let mut cands: Vec<EntityId> = pp.iter().filter(|q| Some(q.entity) != me && near(q.pos, at)).map(|q| q.entity).collect();
        cands.sort();
        cands.dedup();
        if let Some(m) = cands.iter().find(|c| multi(c)) {
            return Some(*m);
        }
        cands.into_iter().find(|c| me.map_or(true, |m| *c < m))
    };
    // まとまりの、自分以外の全てのピンが1つのシンボル S のものなら S。1ピンのシンボルの親は複数ピンのシンボルだけ。
    let group_parent = |g: &PhysGroup, me: Option<EntityId>| -> Option<EntityId> {
        let others: BTreeSet<EntityId> = g.pins.iter().map(|p| p.0).filter(|o| Some(*o) != me).collect();
        let s = *others.iter().next()?;
        (others.len() == 1 && (me.is_none() || multi(&s))).then_some(s)
    };

    // 衛星の判定: 衛星の候補 (1ピンのシンボル・ラベル) -> 親のシンボル。
    // ピンの上に直接あれば、そのピンのシンボルの衛星。そうでなければ配線のまとまりで決める。
    let mut parent_of: BTreeMap<EntityId, EntityId> = BTreeMap::new();
    for e in sheet.entities.values() {
        let found = match e {
            Entity::Symbol(s) if pin_count.get(&s.id) == Some(&1) => {
                let Some(q) = pp.iter().find(|q| q.entity == s.id) else { continue };
                coincident(q.pos, Some(s.id)).or_else(|| {
                    groups.iter().find(|g| g.pins.contains(&(s.id, q.number.clone()))).and_then(|g| group_parent(g, Some(s.id)))
                })
            }
            Entity::NetLabel(l) => coincident(l.at, None)
                .or_else(|| groups.iter().find(|g| g.labels.contains(&l.id)).and_then(|g| group_parent(g, None))),
            _ => continue,
        };
        if let Some(p) = found {
            parent_of.insert(e.id(), p);
        }
    }
    // 親が衛星なら、その親へたどる (輪にならないので必ず主で止まる)
    let root = |mut id: EntityId| -> EntityId {
        for _ in 0..=parent_of.len() {
            match parent_of.get(&id) {
                Some(p) => id = *p,
                None => break,
            }
        }
        id
    };
    let parent_of: BTreeMap<EntityId, EntityId> = parent_of.keys().map(|k| (*k, root(*k))).collect();

    // 主の一覧 (id順)。
    let mut mains: Vec<EntityId> = Vec::new();
    for e in sheet.entities.values() {
        if matches!(e, Entity::Symbol(_) | Entity::NetLabel(_)) && !parent_of.contains_key(&e.id()) {
            mains.push(e.id());
        }
    }
    mains.sort();

    let mut members: BTreeMap<EntityId, BTreeSet<EntityId>> =
        mains.iter().map(|m| (*m, BTreeSet::from([*m]))).collect();
    for (sat, par) in &parent_of {
        if let Some(set) = members.get_mut(par) {
            set.insert(*sat);
        }
    }

    // 主の外接矩形 (文字の振り分け用)。
    let main_bounds = |m: &EntityId| -> (f64, f64, f64, f64) {
        let e = &sheet.entities[m];
        match entity_bounds(e, &idx) {
            Some(r) => rect_tuple(&r),
            None => {
                let a = anchor_of(e).unwrap_or(Point::new(0.0, 0.0));
                (a.x, a.y, a.x, a.y)
            }
        }
    };
    let main_b: BTreeMap<EntityId, (f64, f64, f64, f64)> = mains.iter().map(|m| (*m, main_bounds(m))).collect();

    // 文字の振り分け。
    for e in sheet.entities.values() {
        let Entity::Text(t) = e else { continue };
        if let Some(target) = t.leader_to {
            if let Some(set) = members.values_mut().find(|s| s.contains(&target)) {
                set.insert(t.id);
                continue;
            }
        }
        let mut best: Option<(f64, EntityId)> = None;
        for m in &mains {
            let d = dist_to_rect(&t.at, &main_b[m]);
            if d > SATELLITE_TEXT_MM {
                continue;
            }
            // mains は id順なので、同じ距離なら先 (小さいid) が残る。
            if best.map_or(true, |(bd, _)| d < bd) {
                best = Some((d, *m));
            }
        }
        if let Some((_, m)) = best {
            members.get_mut(&m).unwrap().insert(t.id);
        }
    }

    mains
        .iter()
        .map(|m| {
            let set = &members[m];
            let anchor = anchor_of(&sheet.entities[m]).unwrap_or(Point::new(0.0, 0.0));
            let mut b: Option<(f64, f64, f64, f64)> = None;
            for id in set {
                let Some(e) = sheet.entities.get(id) else { continue };
                let Some(r) = entity_bounds(e, &idx) else { continue };
                let r = rect_tuple(&r);
                b = Some(match b {
                    None => r,
                    Some(o) => (o.0.min(r.0), o.1.min(r.1), o.2.max(r.2), o.3.max(r.3)),
                });
            }
            Unit {
                main: *m,
                members: set.iter().copied().collect(),
                anchor,
                bounds: b.unwrap_or((anchor.x, anchor.y, anchor.x, anchor.y)),
            }
        })
        .collect()
}

/// 帯 (列または行): 区間と、属する単位の添字。
#[derive(Debug, Clone, PartialEq)]
pub struct Band {
    pub lo: f64,
    pub hi: f64,
    pub members: Vec<usize>,
}

/// 区間 (lo, hi) の並びを、重なるものどうしまとめた帯にする。lo順。
pub fn bands(spans: &[(f64, f64)]) -> Vec<Band> {
    let mut order: Vec<usize> = (0..spans.len()).collect();
    order.sort_by(|&a, &b| spans[a].0.total_cmp(&spans[b].0).then(a.cmp(&b)));
    let mut out: Vec<Band> = Vec::new();
    for i in order {
        let (lo, hi) = spans[i];
        match out.last_mut() {
            Some(band) if lo < band.hi - 1e-6 => {
                band.hi = band.hi.max(hi);
                band.members.push(i);
            }
            _ => out.push(Band { lo, hi, members: vec![i] }),
        }
    }
    out
}

/// 帯ごとのずらし量 (2.5mmの倍数)。間の空きを [channel, max_gap] に収める。先頭の帯は動かない。
pub fn band_shifts(bands: &[Band], channel: f64, max_gap: f64) -> Vec<f64> {
    let mut s = Vec::with_capacity(bands.len());
    if bands.is_empty() {
        return s;
    }
    s.push(0.0);
    for i in 0..bands.len() - 1 {
        let g = bands[i + 1].lo - bands[i].hi;
        let d = if g < channel {
            ((channel - g) / STEP_MM).ceil() * STEP_MM
        } else if g > max_gap {
            -((g - max_gap) / STEP_MM).floor() * STEP_MM
        } else {
            0.0
        };
        s.push(s[i] + d);
    }
    s
}

/// 帯の外の点を写す単調な区分線形の写像。
#[derive(Debug, Clone)]
pub struct Warp {
    /// (旧, 新)。旧の昇順。
    knots: Vec<(f64, f64)>,
}

impl Warp {
    pub fn new(bands: &[Band], shifts: &[f64]) -> Self {
        let mut knots = Vec::with_capacity(bands.len() * 2);
        for (b, s) in bands.iter().zip(shifts) {
            knots.push((b.lo, b.lo + s));
            knots.push((b.hi, b.hi + s));
        }
        Self { knots }
    }

    pub fn map(&self, v: f64) -> f64 {
        let (Some(first), Some(last)) = (self.knots.first(), self.knots.last()) else {
            return v;
        };
        if v <= first.0 {
            return v + (first.1 - first.0);
        }
        if v >= last.0 {
            return v + (last.1 - last.0);
        }
        for w in self.knots.windows(2) {
            let (a, b) = (w[0], w[1]);
            if v >= a.0 && v <= b.0 {
                let span = b.0 - a.0;
                if span < 1e-12 {
                    return a.1;
                }
                return a.1 + (v - a.0) / span * (b.1 - a.1);
            }
        }
        v
    }
}

/// 基準の座標 (keys) の差が tol 未満の組を、組の下側の中央値へそろえるずらし量 (2.5mmの倍数)。
/// keys と同じ並びで返す。組は [`key_groups`] で分ける (組の幅は tol 未満)。2.5mmへの丸めで
/// 並び順が入れ替わる組は動かさない。
pub fn align_moves(keys: &[f64], tol: f64) -> Vec<f64> {
    let mut moves = vec![0.0; keys.len()];
    let groups = key_groups(keys, tol);
    let mut group_of = vec![0usize; keys.len()];
    for (gi, group) in groups.iter().enumerate() {
        let target = keys[group[(group.len() - 1) / 2]];
        for &i in group {
            group_of[i] = gi;
            moves[i] = ((target - keys[i]) / STEP_MM).round() * STEP_MM;
        }
    }
    // 並び順が入れ替わる組は動かさない (元に戻すと順は保たれるので、繰り返せば必ず止まる)
    let mut order: Vec<usize> = (0..keys.len()).collect();
    order.sort_by(|&a, &b| keys[a].total_cmp(&keys[b]).then(a.cmp(&b)));
    loop {
        let mut undo: BTreeSet<usize> = BTreeSet::new();
        for w in order.windows(2) {
            let (a, b) = (w[0], w[1]);
            if keys[a] < keys[b] && keys[a] + moves[a] > keys[b] + moves[b] + EPS {
                undo.insert(group_of[a]);
                undo.insert(group_of[b]);
            }
        }
        if undo.is_empty() {
            break;
        }
        for (i, m) in moves.iter_mut().enumerate() {
            if undo.contains(&group_of[i]) {
                *m = 0.0;
            }
        }
    }
    // -0.0 を 0.0 に直す。
    for m in &mut moves {
        if *m == 0.0 {
            *m = 0.0;
        }
    }
    moves
}

// ----------------------------------------------------------------------
// 並べ直しの計画 (整列・通り道・引き直し・ラベルでの接続・検算)
// ----------------------------------------------------------------------

/// 外接矩形 (x0, y0, x1, y1)。
type Bounds = (f64, f64, f64, f64);

/// 浮動小数の比較に使う許容誤差 (mm)。
const EPS: f64 = 1e-6;
/// ラベルでつなぐときの、ピンからの引き出しが作れないときの直線の長さ (mm)。
const STUB_MM: f64 = 2.5;

/// 並べ直しに要る図面の外の情報。
#[derive(Debug, Clone, Default)]
pub struct ArrangeContext {
    /// シートの外から指されているid (FreeCADとの対応付け)。
    pub protected: Vec<EntityId>,
    /// 図枠の内側 (x0, y0, x1, y1)。
    pub frame: (f64, f64, f64, f64),
    /// 表題欄 (x0, y0, x1, y1)。
    pub title_block: (f64, f64, f64, f64),
    /// プロジェクトで使われているラベル名とネット名シンボルの名前。
    pub taken_names: Vec<String>,
    /// シートの書き方がシート名のとき、ラベル名 → 旗の中の自動の行き先 ([`crate::xref::sheet_tag_dests`])。
    pub tag_dests: BTreeMap<String, String>,
}

/// 図枠の内側と、表題欄 (上に積む改訂欄の行を含む) の矩形 (x0, y0, x1, y1) を、図枠テンプレートとシートから求める。
/// 表題欄はテンプレートの隅 (`title_block.origin`) に、表題欄の幅と高さで置かれる。改訂欄は表題欄の直上に
/// 同じ幅で、(改訂の行数 + 見出し1行) × 行高だけ積まれる (改訂が無ければ描かない)。
pub fn frame_areas(template: &FrameTemplate, sheet: &Sheet) -> ((f64, f64, f64, f64), (f64, f64, f64, f64)) {
    let (pw, ph) = sheet.paper_mm();
    let frame = template.frame_rect(pw, ph);
    let (tx, ty) = template.title_block_origin(pw, ph);
    let tb = &template.title_block;
    let rb = &template.revision_block;
    let rows = rb.rows_fixed.unwrap_or_else(|| sheet.revisions.len().min(rb.rows_max));
    let rev_h = if rows == 0 { 0.0 } else { (rows as f64 + 1.0) * template.revision_row_h() };
    (frame, (tx, ty - rev_h, tx + tb.w, ty + tb.h))
}

/// 並べ直しの計画。図面は書き換えない。
#[derive(Debug, Clone, Default, Serialize)]
pub struct ArrangePlan {
    /// まとめて動かすid と (dx, dy)。
    pub moves: Vec<(Vec<EntityId>, f64, f64)>,
    /// 形を写して置き換えるエンティティ (図形・離れた配線・ハーネスなど)。
    pub updates: Vec<Entity>,
    pub remove: Vec<EntityId>,
    pub add: Vec<Entity>,
    pub moved: usize,
    pub columns: usize,
    pub rows: usize,
    /// 配線を引き直したネット (重複なし、名前順)。
    pub routed: Vec<String>,
    /// ラベルでつないだかたまり (ネット名, ラベル名)。
    pub labelled: Vec<(String, String)>,
    /// 引けずに、元の配線の形のまま伸び縮みさせたネット (重複なし、名前順)。
    pub kept_shape: Vec<String>,
    pub size_before: (f64, f64),
    pub size_after: (f64, f64),
    /// 計画を当てた後のシート (数値を測るための写し)。
    #[serde(skip)]
    pub after: Option<Sheet>,
}

/// 軸 (列=X、行=Y)。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Axis {
    X,
    Y,
}

/// 列と行の配置の結果: 単位ごとの移動量と、単位以外を写す座標の写像。
struct Layout {
    dx: Vec<f64>,
    dy: Vec<f64>,
    wx: Warp,
    wy: Warp,
    columns: usize,
    rows: usize,
}

/// 動かさずに残したかたまり (引き直さないもの) の扱い。
enum Fate {
    /// 端子が1つの単位 (None なら端子なし) だけにある: 形を変えずその単位 (または写像) と一緒に動く。
    Rigid(Option<usize>),
    /// 複数の単位にまたがる: ラベルでつなぐ。
    Labels,
}

/// ラベルでつなぐかたまり1つ分: ラベル名と、端子 (移動後の位置・向き・引き出しの配線の属性)。
pub(crate) struct Fallback {
    pub(crate) net: String,
    pub(crate) name: String,
    pub(crate) terms: Vec<(Point, Option<Dir>, Wire)>,
    /// 新しいラベルの向きと属性の手本。
    pub(crate) template: Option<NetLabel>,
}

fn near(a: Point, b: Point) -> bool {
    a.distance_to(&b) < CONNECT_EPS
}

fn union_bounds(a: Option<Bounds>, b: Bounds) -> Option<Bounds> {
    Some(match a {
        None => b,
        Some(o) => (o.0.min(b.0), o.1.min(b.1), o.2.max(b.2), o.3.max(b.3)),
    })
}

fn rects_hit(a: Bounds, b: Bounds) -> bool {
    a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}

/// 全エンティティの外接矩形 (外形を持たないものは数えない)。
fn content_bounds(sheet: &Sheet, idx: &DefIndex) -> Option<Bounds> {
    let mut b = None;
    for e in sheet.entities.values() {
        if let Some(r) = entity_bounds(e, idx) {
            b = union_bounds(b, rect_tuple(&r));
        }
    }
    b
}

/// 外接矩形の (幅, 高さ)。無ければ (0, 0)。
fn size_of(b: Option<Bounds>) -> (f64, f64) {
    b.map_or((0.0, 0.0), |b| (b.2 - b.0, b.3 - b.1))
}

/// 小数の揺れを丸める (写像の計算誤差で図形が「変わった」と見えないように)。
fn rnd(v: f64) -> f64 {
    (v * 1e6).round() / 1e6
}

/// 回路図シート以外・ケーブル・盤レイアウトのエンティティがあれば断る。
fn refuse_unsupported(sheet: &Sheet) -> Result<(), String> {
    if sheet.kind != SheetKind::Schematic {
        return Err("回路図シートではないため、並べ直せません".into());
    }
    let bad = sheet.entities.values().any(|e| {
        matches!(
            e,
            Entity::Cable(_)
                | Entity::PanelOutline(_)
                | Entity::DinRail(_)
                | Entity::Duct(_)
                | Entity::Footprint(_)
                | Entity::PanelLegend(_)
                | Entity::Nameplate(_)
                | Entity::Dimension(_)
        )
    });
    if bad {
        return Err("ケーブルまたは盤レイアウトの要素があるシートは並べ直せません".into());
    }
    Ok(())
}

/// 引き直しの単位とかたまりを集める。動かせない配線 (ポートの接点・ケーブル・他から指されている・長さの出所がまちまち) があれば、ネット名を挙げて断る。
fn collect(sheet: &Sheet, defs: &[SymbolDef], opts: &ArrangeOptions, ctx: &ArrangeContext) -> Result<Collected, String> {
    let c = collect_route_units(sheet, defs, &RerouteOptions { nets: vec![], route: opts.route.clone() }, &ctx.protected);
    let kept_reasons: Vec<&str> = [
        KeepReason::NoPath,
        KeepReason::Pointed,
        KeepReason::Mixed,
        KeepReason::LengthSources,
        KeepReason::FreeEnd,
        KeepReason::FewTerminals,
    ]
    .iter()
    .map(|r| r.as_str())
    .collect();
    let mut bad: BTreeSet<String> = BTreeSet::new();
    for (net, why) in &c.unchanged {
        if why == "not selected" || why == "no such net" || kept_reasons.contains(&why.as_str()) {
            continue;
        }
        bad.insert(net.clone());
    }
    for k in &c.kept {
        if matches!(k.reason, KeepReason::Pointed | KeepReason::LengthSources) {
            bad.insert(k.net.clone());
        }
    }
    if bad.is_empty() {
        return Ok(c);
    }
    let names: Vec<String> = bad.into_iter().collect();
    Err(format!("動かせない配線を含むネットがあるため、並べ直せません: {}", names.join(", ")))
}

/// 端子の位置から、それを持つ単位の添字を引く索引。
struct Owners {
    pins: Vec<(Point, EntityId)>,
    labels: Vec<(Point, EntityId)>,
    unit_of: BTreeMap<EntityId, usize>,
}

impl Owners {
    /// シートのピンとラベルと、単位のメンバーから作る。
    fn new(sheet: &Sheet, defs: &[SymbolDef], us: &[Unit]) -> Self {
        let pins = pin_points(sheet, defs).into_iter().map(|p| (p.pos, p.entity)).collect();
        let labels = sheet
            .entities
            .values()
            .filter_map(|e| if let Entity::NetLabel(l) = e { Some((l.at, l.id)) } else { None })
            .collect();
        let mut unit_of = BTreeMap::new();
        for (i, u) in us.iter().enumerate() {
            for m in &u.members {
                unit_of.insert(*m, i);
            }
        }
        Self { pins, labels, unit_of }
    }

    /// 位置にあるピン (なければラベル) を持つ単位の添字。
    fn of(&self, p: Point) -> Option<usize> {
        let hit = self.pins.iter().find(|(q, _)| near(*q, p)).or_else(|| self.labels.iter().find(|(q, _)| near(*q, p)));
        hit.and_then(|(_, id)| self.unit_of.get(id).copied())
    }
}

/// 端子のリストを単位の添字へ直す。持ち主が見つからなければ断る。
fn term_units(net: &str, terms: &[(Point, Option<Dir>)], owners: &Owners) -> Result<Vec<usize>, String> {
    terms
        .iter()
        .map(|t| owners.of(t.0).ok_or_else(|| format!("ネット {net} の端子の持ち主が見つからないため、並べ直せません")))
        .collect()
}

/// 配線のかたまりの上にあるネットラベル (id, 名前)。id順。
fn cluster_labels(sheet: &Sheet, wires: &[EntityId]) -> Vec<(EntityId, String)> {
    let ws: Vec<&Wire> = wires
        .iter()
        .filter_map(|id| if let Some(Entity::Wire(w)) = sheet.entities.get(id) { Some(w) } else { None })
        .collect();
    sheet
        .entities
        .values()
        .filter_map(|e| match e {
            Entity::NetLabel(l) if ws.iter().any(|w| on_wire(w, &l.at)) => Some((l.id, l.name.clone())),
            _ => None,
        })
        .collect()
}

/// 動かさずに残したかたまりを、端子の持ち主で分ける。
fn classify_kept(kept: &[KeptCluster], owners: &Owners) -> Result<Vec<Fate>, String> {
    let mut fates = Vec::new();
    for k in kept {
        let set: BTreeSet<usize> = term_units(&k.net, &k.terms, owners)?.into_iter().collect();
        if set.len() <= 1 {
            fates.push(Fate::Rigid(set.iter().next().copied()));
        } else {
            fates.push(Fate::Labels);
        }
    }
    Ok(fates)
}

/// 端子の位置にピンがあるネット名シンボルの名前。
fn term_symbol_names(sheet: &Sheet, idx: &DefIndex, pins: &[PinPt], terms: &[Point]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for t in terms {
        for pin in pins.iter().filter(|p| near(p.pos, *t)) {
            let Some(Entity::Symbol(s)) = sheet.entities.get(&pin.entity) else { continue };
            if let Some(n) = idx.get(s).and_then(|d| d.net_name().map(str::to_string)) {
                out.insert(n);
            }
        }
    }
    out
}

/// 名前の違うラベル (ネット名シンボルを含む) を持つかたまりがあれば、ネット名を挙げて断る。
/// 引き直すかたまりも残すかたまりも、引き直す前に調べる (ラベルでつなぐときに名前を1つしか残せないため)。
fn refuse_mixed_names(sheet: &Sheet, idx: &DefIndex, pins: &[PinPt], c: &Collected) -> Result<(), String> {
    let mut bad: BTreeSet<String> = BTreeSet::new();
    let clusters = c
        .units
        .iter()
        .map(|u| (&u.net, &u.wires, &u.terms))
        .chain(c.kept.iter().map(|k| (&k.net, &k.wires, &k.terms)));
    for (net, wires, terms) in clusters {
        let at: Vec<Point> = terms.iter().map(|t| t.0).collect();
        let mut names: BTreeSet<String> = cluster_labels(sheet, wires).into_iter().map(|l| l.1).collect();
        names.extend(term_symbol_names(sheet, idx, pins, &at));
        if names.len() > 1 {
            bad.insert(format!("{net} ({})", names.into_iter().collect::<Vec<_>>().join("/")));
        }
    }
    if bad.is_empty() {
        return Ok(());
    }
    Err(format!("名前の違うラベルを持つネットがあるため、並べ直せません: {}", bad.into_iter().collect::<Vec<_>>().join(", ")))
}

fn span(b: &Bounds, ax: Axis) -> (f64, f64) {
    match ax {
        Axis::X => (b.0, b.2),
        Axis::Y => (b.1, b.3),
    }
}

fn shifted(b: Bounds, ax: Axis, d: f64) -> Bounds {
    match ax {
        Axis::X => (b.0 + d, b.1, b.2 + d, b.3),
        Axis::Y => (b.0, b.1 + d, b.2, b.3 + d),
    }
}

/// 基準の座標を、組の最初の座標との差が tol 未満のものどうしの組に分ける ([`align_moves`] の分け方)。
/// 組の幅が tol を超えないので、近い座標が鎖のように続いても1つの組にはならない。組は keys の添字。
fn key_groups(keys: &[f64], tol: f64) -> Vec<Vec<usize>> {
    let mut order: Vec<usize> = (0..keys.len()).collect();
    order.sort_by(|&a, &b| keys[a].total_cmp(&keys[b]).then(a.cmp(&b)));
    let mut out: Vec<Vec<usize>> = Vec::new();
    let mut first: Option<f64> = None;
    for i in order {
        match first {
            Some(f) if keys[i] - f < tol => out.last_mut().expect("組がある").push(i),
            _ => {
                out.push(vec![i]);
                first = Some(keys[i]);
            }
        }
    }
    out
}

/// 2つの外接矩形が同じだけ動いたか。
fn same_shift(a0: &Bounds, a1: &Bounds, b0: &Bounds, b1: &Bounds) -> bool {
    ((a1.0 - a0.0) - (b1.0 - b0.0)).abs() < EPS && ((a1.1 - a0.1) - (b1.1 - b0.1)).abs() < EPS
}

/// 外接矩形が重なるか、0.5mm 以内に近いか (ピンは外接矩形の辺にあるので、接しただけでもピンどうしが触れる)。
fn touch(a: &Bounds, b: &Bounds) -> bool {
    const M: f64 = 0.5;
    a.0 < b.2 + M && b.0 < a.2 + M && a.1 < b.3 + M && b.1 < a.3 + M
}

/// 組 (ids) を新しい外接矩形 (newb) へ動かすと、今まで重なっていなかった2つの単位が重なる (0.5mm以内に近づく) か。
/// もともと重なっていた組が、互いの位置を変えるときも真。
fn creates_overlap(cur: &[Bounds], ids: &[usize], newb: &[Bounds]) -> bool {
    for (p, &i) in ids.iter().enumerate() {
        for j in 0..cur.len() {
            if j == i {
                continue;
            }
            match ids.iter().position(|&x| x == j) {
                Some(q) => {
                    if j > i && touch(&newb[p], &newb[q]) && !touch(&cur[i], &cur[j]) {
                        return true;
                    }
                    // もともと重なっている組は、互いの位置を変えない (ピンどうし・ピンと印が触れてしまうため)
                    if touch(&cur[i], &cur[j]) && !same_shift(&cur[i], &newb[p], &cur[j], &newb[q]) {
                        return true;
                    }
                }
                None => {
                    if touch(&newb[p], &cur[j]) && !touch(&cur[i], &cur[j]) {
                        return true;
                    }
                    if touch(&cur[i], &cur[j]) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// 1つの軸について、整列 (列ごとに基準をそろえる) と帯のずらしを行い、単位ごとの移動量・座標の写像・帯の数を返す。
/// `cur`は途中の外接矩形で、移動を反映して書き換える。
fn axis_pass(cur: &mut [Bounds], anchors: &[f64], ax: Axis, opts: &ArrangeOptions, max_gap: f64) -> (Vec<f64>, Warp, usize) {
    let mut total = vec![0.0_f64; cur.len()];
    let spans: Vec<(f64, f64)> = cur.iter().map(|b| span(b, ax)).collect();
    for band in bands(&spans) {
        let keys: Vec<f64> = band.members.iter().map(|&i| anchors[i]).collect();
        let moves = align_moves(&keys, opts.align_mm);
        for g in key_groups(&keys, opts.align_mm) {
            if g.iter().all(|&k| moves[k] == 0.0) {
                continue;
            }
            let ids: Vec<usize> = g.iter().map(|&k| band.members[k]).collect();
            let newb: Vec<Bounds> = g.iter().map(|&k| shifted(cur[band.members[k]], ax, moves[k])).collect();
            if creates_overlap(cur, &ids, &newb) {
                continue;
            }
            for (pos, &i) in ids.iter().enumerate() {
                cur[i] = newb[pos];
                total[i] += moves[g[pos]];
            }
        }
    }
    let spans: Vec<(f64, f64)> = cur.iter().map(|b| span(b, ax)).collect();
    let bs = bands(&spans);
    let shifts = band_shifts(&bs, opts.channel_mm, max_gap);
    for (band, s) in bs.iter().zip(&shifts) {
        for &i in &band.members {
            cur[i] = shifted(cur[i], ax, *s);
            total[i] += *s;
        }
    }
    (total, Warp::new(&bs, &shifts), bs.len())
}

/// 列 (x) と行 (y) の配置を求める。空きの上限は `max_gap`。
fn layout(us: &[Unit], opts: &ArrangeOptions, max_gap: f64) -> Layout {
    let mut cur: Vec<Bounds> = us.iter().map(|u| u.bounds).collect();
    let ax: Vec<f64> = us.iter().map(|u| u.anchor.x).collect();
    let (dx, wx, columns) = axis_pass(&mut cur, &ax, Axis::X, opts, max_gap);
    let ay: Vec<f64> = us.iter().map(|u| u.anchor.y).collect();
    let (dy, wy, rows) = axis_pass(&mut cur, &ay, Axis::Y, opts, max_gap);
    Layout { dx, dy, wx, wy, columns, rows }
}

/// 点を写像で写す。
fn warp_point(lay: &Layout, p: &mut Point) {
    *p = Point::new(rnd(lay.wx.map(p.x)), rnd(lay.wy.map(p.y)));
}

/// 単位に属さないエンティティの全ての点 (または基準の点) を写像で写す。
fn warp_entity(e: &mut Entity, lay: &Layout) {
    match e {
        Entity::Wire(w) => w.points.iter_mut().for_each(|p| warp_point(lay, p)),
        Entity::Shape(s) => s.points.iter_mut().for_each(|p| warp_point(lay, p)),
        Entity::Harness(h) => h.points.iter_mut().for_each(|p| warp_point(lay, p)),
        Entity::Junction(j) => warp_point(lay, &mut j.at),
        Entity::Text(t) => warp_point(lay, &mut t.at),
        Entity::WireLegend(l) => warp_point(lay, &mut l.at),
        Entity::Symbol(s) => warp_point(lay, &mut s.at),
        Entity::NetLabel(l) => warp_point(lay, &mut l.at),
        _ => {}
    }
}

/// 移動後の写しを作る。単位のメンバーと剛体のかたまりは単位の移動量で、残りは写像で動かし、`dropped`は取り除く。
fn build_moved(
    sheet: &Sheet,
    lay: &Layout,
    unit_of: &BTreeMap<EntityId, usize>,
    rigid: &BTreeMap<EntityId, usize>,
    dropped: &BTreeSet<EntityId>,
) -> Sheet {
    let mut out = sheet.clone();
    for id in dropped {
        out.entities.remove(id);
    }
    for (id, e) in out.entities.iter_mut() {
        match unit_of.get(id).or_else(|| rigid.get(id)) {
            Some(&i) => e.translate(lay.dx[i], lay.dy[i]),
            None => warp_entity(e, lay),
        }
    }
    out
}

/// 図枠に収まり、(元が表題欄に掛かっていなければ) 表題欄を避けているか。
fn fits(b: Option<Bounds>, ctx: &ArrangeContext, hit_title: bool) -> bool {
    fits_in(b, ctx.frame, ctx.title_block, hit_title)
}

/// [`fits`]の図枠と表題欄を直接受け取る形。
fn fits_in(b: Option<Bounds>, f: Bounds, title_block: Bounds, hit_title: bool) -> bool {
    let Some(b) = b else { return true };
    if b.0 < f.0 - EPS || b.1 < f.1 - EPS || b.2 > f.2 + EPS || b.3 > f.3 + EPS {
        return false;
    }
    hit_title || !rects_hit(b, title_block)
}

/// エンティティの位置を決める点 (形の変化を調べるため)。
fn shape_points(e: &Entity) -> Vec<Point> {
    match e {
        Entity::Wire(w) => w.points.clone(),
        Entity::Shape(s) => s.points.clone(),
        Entity::Harness(h) => h.points.clone(),
        Entity::Junction(j) => vec![j.at],
        Entity::Text(t) => vec![t.at],
        Entity::WireLegend(l) => vec![l.at],
        Entity::Symbol(s) => vec![s.at],
        Entity::NetLabel(l) => vec![l.at],
        _ => vec![],
    }
}

fn geometry_changed(a: &Entity, b: &Entity) -> bool {
    let (pa, pb) = (shape_points(a), shape_points(b));
    pa.len() != pb.len() || pa.iter().zip(&pb).any(|(p, q)| p.distance_to(q) > 1e-5)
}

/// プロジェクトで使われている名前 (引数の名前・シートのラベル・ネット名シンボル)。
fn taken_set(sheet: &Sheet, defs: &[SymbolDef], ctx: &ArrangeContext) -> BTreeSet<String> {
    let idx = DefIndex::new(defs);
    let mut t: BTreeSet<String> = ctx.taken_names.iter().cloned().collect();
    for e in sheet.entities.values() {
        match e {
            Entity::NetLabel(l) => {
                t.insert(l.name.clone());
            }
            Entity::Symbol(s) => {
                if let Some(n) = idx.get(s).and_then(|d| d.net.clone()) {
                    t.insert(n.trim().to_string());
                }
            }
            _ => {}
        }
    }
    t
}

/// ラベルでつなぐときの名前・消すラベル・新しいラベルの手本。
struct FallbackName {
    name: String,
    /// 消すラベルのid。
    delete: Vec<EntityId>,
    /// 新しいラベルの向きと属性の手本 (消すラベルの先頭)。
    template: Option<NetLabel>,
    /// 新しく作った `ARR_<n>` の名前か。
    fresh: bool,
    /// 短い線を出さない端子の元の位置 (残す配線でつながる、電源マークのピンと相手のピン)。
    skip: Vec<Point>,
    /// そのまま残す配線 (電源マークのピンと、同じ単位のピンを直接結ぶ線)。単位と一緒に動かす。
    keep: Vec<EntityId>,
}

/// ラベルでつなぐときの名前と、消すラベル・残す配線。
///
/// 名前は、かたまりのラベルの名前 (ラベルは新しいラベルに置き換えるので消す)、なければネット全体のラベル名
/// (`net_labels`。同じネットの別のかたまりにあるラベル。同じ名前のラベルを足してもつながりは変わらない)、
/// なければ電源マーク (ネット名シンボル) の名前、なければ未使用の `ARR_<n>`。
/// 電源マークは消さない: マークのピンと同じ単位のピンを直接結ぶ線はそのまま残し、両方に短い線を出さない。
/// 他から指されているラベル (`pointed`) と、ピンの端子が無いかたまりのラベル (代わりのラベルが作られない) は消さない。
#[allow(clippy::too_many_arguments)]
fn fallback_name(
    sheet: &Sheet,
    idx: &DefIndex,
    pins: &[PinPt],
    owners: &Owners,
    net: &str,
    net_labels: &BTreeMap<String, String>,
    wires: &[EntityId],
    terms: &[(Point, Option<Dir>)],
    pointed: &BTreeSet<EntityId>,
    taken: &mut BTreeSet<String>,
) -> FallbackName {
    let labels = cluster_labels(sheet, wires);
    let at: Vec<Point> = terms.iter().map(|t| t.0).collect();
    // ネット名シンボル (電源マーク) のピンの端子: (位置, シンボルid, 名前)
    let marks: Vec<(Point, EntityId, String)> = at
        .iter()
        .flat_map(|t| pins.iter().filter(move |p| near(p.pos, *t)))
        .filter_map(|p| match sheet.entities.get(&p.entity) {
            Some(Entity::Symbol(s)) => idx.get(s).and_then(|d| d.net_name().map(|n| (p.pos, s.id, n.to_string()))),
            _ => None,
        })
        .collect();
    let is_mark = |q: Point| marks.iter().any(|m| near(m.0, q));
    // 電源マークのピンと同じ単位のピンを直接結ぶ線は残す
    let mut keep: Vec<EntityId> = Vec::new();
    let mut skip: Vec<Point> = Vec::new();
    for id in wires {
        let Some(Entity::Wire(w)) = sheet.entities.get(id) else { continue };
        let (Some(a), Some(b)) = (w.points.first().copied(), w.points.last().copied()) else { continue };
        let pin_at = |q: Point| at.iter().any(|t| near(*t, q));
        if !(pin_at(a) && pin_at(b)) || is_mark(a) == is_mark(b) {
            continue;
        }
        if owners.of(a).is_some() && owners.of(a) == owners.of(b) {
            keep.push(*id);
            skip.push(a);
            skip.push(b);
        }
    }
    let has_pin = terms.iter().any(|t| t.1.is_some() && !is_mark(t.0));
    let (name, template) = if let Some(first) = labels.first() {
        let template = match sheet.entities.get(&labels[0].0) {
            Some(Entity::NetLabel(l)) => Some(l.clone()),
            _ => None,
        };
        (Some(first.1.clone()), template)
    } else if let Some(l) = net_labels.get(net) {
        (Some(l.clone()), None)
    } else {
        (marks.iter().map(|m| m.2.clone()).min(), None)
    };
    if let Some(name) = name {
        let delete: Vec<EntityId> =
            if has_pin { labels.iter().map(|l| l.0).filter(|id| !pointed.contains(id)).collect() } else { Vec::new() };
        return FallbackName { name, delete, template, fresh: false, skip, keep };
    }
    let mut n = 1;
    loop {
        let name = format!("ARR_{n}");
        if !taken.contains(&name) {
            taken.insert(name.clone());
            return FallbackName { name, delete: vec![], template: None, fresh: true, skip, keep };
        }
        n += 1;
    }
}

/// 残す配線を、端の持ち主の単位の移動量で動かした写し (新しいid)。
fn kept_links(sheet: &Sheet, owners: &Owners, lay: &Layout, keep: &[EntityId]) -> Vec<Entity> {
    keep.iter()
        .filter_map(|id| match sheet.entities.get(id) {
            Some(Entity::Wire(w)) => {
                let o = w.points.first().copied().and_then(|q| owners.of(q))?;
                let mut c = w.clone();
                c.id = uuid::Uuid::new_v4();
                c.points.iter_mut().for_each(|p| *p = p.translated(lay.dx[o], lay.dy[o]));
                Some(Entity::Wire(c))
            }
            _ => None,
        })
        .collect()
}

/// 位置に触れる古い配線 (なければかたまりの先頭) の写し。長さは持たせない。
fn old_wire_at(sheet: &Sheet, ids: &[EntityId], at: Point) -> Wire {
    let ws: Vec<&Wire> = ids.iter().filter_map(|id| if let Some(Entity::Wire(w)) = sheet.entities.get(id) { Some(w) } else { None }).collect();
    let mut w = ws.iter().find(|w| on_wire(w, &at)).or(ws.first()).map(|w| (*w).clone()).unwrap_or_else(|| Wire {
        id: uuid::Uuid::new_v4(),
        points: vec![],
        color: "black".into(),
        sq: 0.75,
        length_m: None,
        length_source: Default::default(),
        part_no: None,
        net: None,
        dashed: false,
        weight_mm: None,
    });
    w.length_m = None;
    w.length_source = Default::default();
    w
}

fn dir_step(d: Dir) -> (f64, f64) {
    match d {
        Dir::N => (0.0, -1.0),
        Dir::E => (1.0, 0.0),
        Dir::S => (0.0, 1.0),
        Dir::W => (-1.0, 0.0),
    }
}

/// ラベルでつなぐ短い線1本と、その先のラベル (検算で、他の物に触れていないかを調べる)。
#[derive(Debug, Clone)]
pub(crate) struct Stub {
    pub(crate) wire: EntityId,
    pub(crate) label: EntityId,
    pub(crate) name: String,
    /// 検算でこの線自身の接触を調べるか (残した電源マークの線は調べず、同じ名前の仲間としてだけ数える)。
    pub(crate) check: bool,
}

/// 点と線分の距離。
fn seg_dist(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l2 = dx * dx + dy * dy;
    let t = if l2 < 1e-12 { 0.0 } else { (((p.x - a.x) * dx + (p.y - a.y) * dy) / l2).clamp(0.0, 1.0) };
    (p.x - a.x - t * dx).hypot(p.y - a.y - t * dy)
}

/// 短い線 (点列) が、他の線・点 (ピン・ラベル・ジャンクション) に触れるか。始点 (自分のピン) は数えない。
fn stub_hits(pts: &[Point], lines: &[Vec<Point>], spots: &[Point]) -> bool {
    path_hits_with(pts, lines, spots, &[pts[0]], 0.3, 1.0)
}

/// 電気的に触れるか (ネットリストと同じ細かい許容差で)。ゆとりを持って避けられないときの最後の選び方に使う。
fn stub_touches(pts: &[Point], lines: &[Vec<Point>], spots: &[Point]) -> bool {
    path_hits_with(pts, lines, spots, &[pts[0]], 0.02, 0.02)
}

/// 点列が、他の線・点に触れるか。`exempt`の点 (自分のピン・自分の他の線との継ぎ目) で触れるのは数えない。
fn path_hits(pts: &[Point], lines: &[Vec<Point>], spots: &[Point], exempt: &[Point]) -> bool {
    path_hits_with(pts, lines, spots, exempt, 0.3, 1.0)
}

/// [`path_hits`] の許容差つき: 線との距離 `touch`、点 (ピン・ラベル・ジャンクション) との距離 `spot_clear`。
fn path_hits_with(pts: &[Point], lines: &[Vec<Point>], spots: &[Point], exempt: &[Point], touch: f64, spot_clear: f64) -> bool {
    #[allow(non_snake_case)]
    let TOUCH: f64 = touch;
    let free = |q: Point| !exempt.iter().any(|e| near(*e, q));
    let segs: Vec<(Point, Point)> = pts.windows(2).map(|w| (w[0], w[1])).collect();
    let on_me = |q: Point| segs.iter().any(|&(a, b)| seg_dist(q, a, b) < TOUCH);
    // ピン・ラベル・ジャンクションからは離す (向かい合うピンの短い線どうしが重ならないように)
    #[allow(non_snake_case)]
    let SPOT_CLEAR: f64 = spot_clear;
    if spots.iter().any(|&q| free(q) && segs.iter().any(|&(a, b)| seg_dist(q, a, b) < SPOT_CLEAR)) {
        return true;
    }
    for line in lines {
        // 他の線の頂点が自分に乗る・自分の頂点が他の線に乗る・線分どうしが交わる
        if line.iter().any(|&q| free(q) && on_me(q)) {
            return true;
        }
        for w in line.windows(2) {
            if pts.iter().any(|&q| free(q) && seg_dist(q, w[0], w[1]) < TOUCH) {
                return true;
            }
            if segs.iter().any(|&(a, b)| crosses(a, b, w[0], w[1])) {
                return true;
            }
        }
    }
    false
}

/// 軸に平行な2本の線分が、端以外で交わるか。
fn crosses(a: Point, b: Point, c: Point, d: Point) -> bool {
    let h1 = (a.y - b.y).abs() < 1e-9;
    let h2 = (c.y - d.y).abs() < 1e-9;
    if h1 == h2 {
        return false;
    }
    let ((hx0, hx1, hy), (vx, vy0, vy1)) = if h1 {
        ((a.x.min(b.x), a.x.max(b.x), a.y), (c.x, c.y.min(d.y), c.y.max(d.y)))
    } else {
        ((c.x.min(d.x), c.x.max(d.x), c.y), (a.x, a.y.min(b.y), a.y.max(b.y)))
    };
    vx > hx0 + 1e-6 && vx < hx1 - 1e-6 && hy > vy0 + 1e-6 && hy < vy1 - 1e-6
}

/// 引けなかったかたまりの元の配線を、形を保ったまま伸び縮みさせる。各点を列・行の写像で写し、端子に
/// あった端は端子の新しい位置へ合わせ、隣の点を同じ向きにずらして直角を保つ (2点の線が斜めになるなら中ほどで折る)。
/// 他の線・ピン・ラベルに触れるなら None。
fn stretch_wires(
    sheet: &Sheet,
    wires: &[EntityId],
    junctions: &[EntityId],
    lay: &Layout,
    trip: &[(Point, Point, Option<Dir>)],
    lines: &[Vec<Point>],
    spots: &[Point],
    bodies: &[(EntityId, Bounds)],
    own: &BTreeSet<EntityId>,
) -> Option<(Vec<Entity>, Vec<Vec<Point>>)> {
    let map = |p: Point| Point::new(rnd(lay.wx.map(p.x)), rnd(lay.wy.map(p.y)));
    let to_term = |old: Point| trip.iter().find(|t| near(t.0, old)).map(|t| t.1);
    let mut olds: Vec<&Wire> = Vec::new();
    for id in wires {
        let Some(Entity::Wire(w)) = sheet.entities.get(id) else { return None };
        if w.points.len() < 2 {
            return None;
        }
        olds.push(w);
    }
    // 端を新しい位置へ合わせ、隣の点を同じ向きにずらして直角を保つ
    let set_end = |pts: &mut Vec<Point>, w: &Wire, end: usize, next: usize, target: Point| {
        let horizontal = (w.points[end].y - w.points[next].y).abs() < 1e-6;
        let vertical = (w.points[end].x - w.points[next].x).abs() < 1e-6;
        pts[end] = target;
        if pts.len() > 2 {
            if horizontal {
                pts[next].y = target.y;
            } else if vertical {
                pts[next].x = target.x;
            }
        }
    };
    // 1回目: 写像と、端子にあった端
    let mut raw: Vec<Vec<Point>> = Vec::new();
    for w in &olds {
        let mut pts: Vec<Point> = w.points.iter().map(|p| map(*p)).collect();
        let n = pts.len();
        for (end, next) in [(0usize, 1usize), (n - 1, n - 2)] {
            if let Some(target) = to_term(w.points[end]) {
                set_end(&mut pts, w, end, next, target);
            }
        }
        raw.push(pts);
    }
    // 2回目: 端子でない端が他の線の頂点 (曲がり角など) にあったなら、その頂点の新しい位置へ合わせる
    let first = raw.clone();
    for (i, w) in olds.iter().enumerate() {
        let n = w.points.len();
        for (end, next) in [(0usize, 1usize), (n - 1, n - 2)] {
            if to_term(w.points[end]).is_some() {
                continue;
            }
            let target = olds.iter().enumerate().filter(|(j, _)| *j != i).find_map(|(j, o)| {
                o.points.iter().position(|q| near(*q, w.points[end])).map(|k| first[j][k])
            });
            if let Some(target) = target {
                if !near(target, raw[i][end]) {
                    set_end(&mut raw[i], w, end, next, target);
                }
            }
        }
    }
    // ジャンクションの新しい位置: 端子ならその新しい位置、どれかの線の頂点ならその頂点の新しい位置、それ以外は写像
    let mut dots: Vec<(Point, Point)> = Vec::new();
    for id in junctions {
        let Some(Entity::Junction(j)) = sheet.entities.get(id) else { return None };
        let at = to_term(j.at)
            .or_else(|| olds.iter().zip(&raw).find_map(|(o, r)| o.points.iter().position(|q| near(*q, j.at)).map(|k| r[k])))
            .unwrap_or_else(|| map(j.at));
        dots.push((j.at, at));
    }
    let mut out = Vec::new();
    let mut paths: Vec<Vec<Point>> = Vec::new();
    let mut news: Vec<Wire> = Vec::new();
    for (w, pts) in olds.iter().zip(raw) {
        let mut pts = pts;
        // 2点の線が斜めになったら、中ほどで折る
        if pts.len() == 2 && (pts[0].x - pts[1].x).abs() > 1e-6 && (pts[0].y - pts[1].y).abs() > 1e-6 {
            let (a, b) = (pts[0], pts[1]);
            let horizontal = (w.points[0].y - w.points[1].y).abs() < 1e-6;
            pts = if horizontal {
                let mx = rnd((a.x + b.x) / 2.0);
                vec![a, Point::new(mx, a.y), Point::new(mx, b.y), b]
            } else {
                let my = rnd((a.y + b.y) / 2.0);
                vec![a, Point::new(a.x, my), Point::new(b.x, my), b]
            };
        }
        // 3点以上でも斜めの線分が残るなら諦める
        if pts.windows(2).any(|s| (s[0].x - s[1].x).abs() > 1e-6 && (s[0].y - s[1].y).abs() > 1e-6) {
            return None;
        }
        let pts = simplify(pts);
        if pts.len() < 2 {
            return None;
        }
        let nw = Wire { id: uuid::Uuid::new_v4(), points: pts.clone(), ..(*w).clone() };
        out.push(Entity::Wire(nw.clone()));
        news.push(nw);
        paths.push(pts);
    }
    // かたまり自身のつながりを確かめる: 古い線 j の上にあった点 (端子・他の線の端・ジャンクション) は、
    // 新しい位置でも新しい線 j の上になければならない (ネットリストと同じ許容差)。外れるなら None (ラベルでつなぐ)
    let stays_on = |old: Point, new: Point, skip: Option<usize>| -> bool {
        olds.iter().zip(&news).enumerate().all(|(j, (o, n))| Some(j) == skip || !on_wire(o, &old) || on_wire(n, &new))
    };
    if !trip.iter().all(|t| stays_on(t.0, t.1, None)) || !dots.iter().all(|d| stays_on(d.0, d.1, None)) {
        return None;
    }
    for (i, (o, p)) in olds.iter().zip(&paths).enumerate() {
        let ends = [(o.points[0], p[0]), (*o.points.last().expect("2点以上"), *p.last().expect("2点以上"))];
        if !ends.iter().all(|e| stays_on(e.0, e.1, Some(i))) {
            return None;
        }
    }
    // 自分の線どうしの継ぎ目・端子は触れてよい
    let mut exempt: Vec<Point> = trip.iter().map(|t| t.1).collect();
    for p in &paths {
        exempt.push(p[0]);
        exempt.push(*p.last().unwrap());
    }
    for (_, at) in &dots {
        exempt.push(*at);
        out.push(Entity::Junction(crate::model::Junction { id: uuid::Uuid::new_v4(), at: *at }));
    }
    if paths.iter().any(|p| path_hits(p, lines, spots, &exempt)) {
        return None;
    }
    // 端子の部品以外の部品の外形を横切らない
    let through = |b: &Bounds, a: Point, c: Point| -> bool {
        let m = 0.3;
        let (x0, y0, x1, y1) = (b.0 + m, b.1 + m, b.2 - m, b.3 - m);
        if x0 >= x1 || y0 >= y1 {
            return false;
        }
        let (sx0, sx1) = (a.x.min(c.x), a.x.max(c.x));
        let (sy0, sy1) = (a.y.min(c.y), a.y.max(c.y));
        sx1 > x0 && sx0 < x1 && sy1 > y0 && sy0 < y1
    };
    if paths.iter().any(|p| p.windows(2).any(|w| bodies.iter().any(|(id, b)| !own.contains(id) && through(b, w[0], w[1])))) {
        return None;
    }
    Some((out, paths))
}

/// 短い線の先のラベルの文字が読める場所を調べる道具: 動かした後の図面の部品の外形・文字と、置いたラベルの文字の矩形。
pub(crate) struct TextRoom {
    /// (持ち主のid・置いたラベルはNone, 矩形)。
    rects: Vec<(Option<EntityId>, crate::tidy::Rect)>,
    /// 部品の外形 (伸ばした短い線が突き抜けてはいけないもの)。
    bodies: Vec<crate::tidy::Rect>,
    /// シートの注記文字高の倍率。
    scale: f64,
    /// 図枠への収まり (無ければ調べない)。
    area: Option<StubArea>,
    /// シートのタグの行き先の書き方と、ラベル名 → 旗の中の自動の行き先 (置くラベルの旗の広がりを測る)。
    style: crate::model::XrefStyle,
    dests: BTreeMap<String, String>,
}

/// 文字の置き場で選ぶ短い線が、並べ直しの最後の「図枠に収まるか」の検算を新しく破らないための情報。
struct StubArea {
    frame: Bounds,
    title_block: Bounds,
    /// 元の図面が表題欄に掛かっていたか ([`fits_in`]と同じ)。
    hit_title: bool,
    /// 動かした後の中身と、置いた短い線の外接矩形。
    content: Option<Bounds>,
}

impl StubArea {
    fn new(ctx: &ArrangeContext, hit_title: bool, content: Option<Bounds>) -> Self {
        Self { frame: ctx.frame, title_block: ctx.title_block, hit_title, content }
    }

    /// 短い線を足しても中身が図枠に収まり、文字が図枠の内側で表題欄に掛からないか。
    fn allows(&self, pts: &[Point], text: &crate::tidy::Rect) -> bool {
        let f = self.frame;
        let inside = text.min.x >= f.0 - EPS && text.min.y >= f.1 - EPS && text.max.x <= f.2 + EPS && text.max.y <= f.3 + EPS;
        inside && !rects_hit(rect_tuple(text), self.title_block) && fits_in(self.grown(pts), f, self.title_block, self.hit_title)
    }

    fn grown(&self, pts: &[Point]) -> Option<Bounds> {
        pts.iter().fold(self.content, |b, q| union_bounds(b, (q.x, q.y, q.x, q.y)))
    }
}

impl TextRoom {
    /// 図面の全ての部品の外形と文字から作る。`gone`の持ち主のものは入れない (計画が消す・置き換えるもの)。
    /// 旗の中の自動の行き先を渡さない版で、テストだけが使う (計画は [`Self::build_with`])。
    #[cfg(test)]
    fn build(sheet: &Sheet, defs: &[SymbolDef], gone: &BTreeSet<EntityId>) -> Self {
        Self::build_with(sheet, defs, gone, BTreeMap::new())
    }

    /// [`Self::build`]に、旗の中の自動の行き先 (ラベル名 → 行き先) を渡す版。
    pub(crate) fn build_with(sheet: &Sheet, defs: &[SymbolDef], gone: &BTreeSet<EntityId>, dests: BTreeMap<String, String>) -> Self {
        let bodies: Vec<(EntityId, crate::tidy::Rect)> =
            crate::tidy::symbol_boxes(sheet, defs).into_iter().filter(|(id, _)| !gone.contains(id)).collect();
        let mut rects: Vec<(Option<EntityId>, crate::tidy::Rect)> = bodies.iter().map(|(id, r)| (Some(*id), *r)).collect();
        rects.extend(
            crate::tidy::text_boxes_with_dests(sheet, defs, &dests).into_iter().filter(|t| t.owner.is_none_or(|o| !gone.contains(&o))).map(|t| (t.owner, t.rect)),
        );
        Self { rects, bodies: bodies.into_iter().map(|b| b.1).collect(), scale: sheet.annotation_scale(), area: None, style: sheet.xref_style, dests }
    }

    /// 短い線が、自分のピンのある外形のほかの部品の外形を突き抜けないか。
    fn clear_of_bodies(&self, pts: &[Point]) -> bool {
        let pin = pts[0];
        let holds = |r: &crate::tidy::Rect| pin.x >= r.min.x - EPS && pin.x <= r.max.x + EPS && pin.y >= r.min.y - EPS && pin.y <= r.max.y + EPS;
        !self.bodies.iter().filter(|r| !holds(r)).any(|r| pts.windows(2).any(|w| seg_in_rect(w[0], w[1], r)))
    }

    /// 後から消すと分かったエンティティの矩形を外す。
    fn forget(&mut self, ids: &[EntityId]) {
        self.rects.retain(|(o, _)| o.is_none_or(|o| !ids.contains(&o)));
    }

    /// 文字の矩形が、部品・文字・線 (`lines`) のどれかと面で重なるか。
    fn blocked(&self, text: &crate::tidy::Rect, lines: &[Vec<Point>]) -> bool {
        self.rects.iter().any(|(_, r)| r.overlaps(text)) || lines.iter().any(|l| l.windows(2).any(|w| seg_in_rect(w[0], w[1], text)))
    }
}

/// 線分が矩形の内側を通るか (辺をなぞるだけ・端が触れるだけは通らない扱い。矩形どうしの重なりと同じ規則)。
fn seg_in_rect(a: Point, b: Point, r: &crate::tidy::Rect) -> bool {
    let span = |lo: f64, hi: f64, rlo: f64, rhi: f64| -> bool {
        if hi - lo < EPS {
            lo > rlo + CONNECT_EPS && lo < rhi - CONNECT_EPS
        } else {
            lo < rhi - CONNECT_EPS && hi > rlo + CONNECT_EPS
        }
    };
    span(a.x.min(b.x), a.x.max(b.x), r.min.x, r.max.x) && span(a.y.min(b.y), a.y.max(b.y), r.min.y, r.max.y)
}

/// [`stub_entities_in`]を、ラベルの文字の置き場を調べずに呼ぶ (線と点だけで選ぶ)。
#[cfg(test)]
fn stub_entities(grid: &RouteGrid, fb: &Fallback, lines: &mut Vec<Vec<Point>>, spots: &mut Vec<Point>) -> (Vec<Entity>, Vec<Stub>) {
    stub_entities_in(grid, fb, lines, spots, None)
}

/// ラベルでつなぐ短い線とラベル。ピンの向きへまっすぐ 2.5・5・7.5 mm の線を順に試し、他の線・ピン・ラベルに
/// 触れない最初のものを使う (どれも触れるなら引き出し線、それもなければ 2.5 mm。検算が断る)。
/// `room`があれば、ラベルの文字が部品の外形・他の文字・他の線に重ならない候補を先に選ぶ。文字の長さぶんの
/// まっすぐな線も候補に足し、タグ付きのラベルは旗の本体がピンと反対を向くように向きを決める。
/// そういう候補が無ければ上の選び方のまま。選んだラベルの文字の矩形は `room` に足し、次のラベルが避ける。
/// 置いた線と端は `lines`・`spots` に足し、次の短い線が避ける。ラベルの向きと属性は、消したラベルがあればその先頭に倣う。
pub(crate) fn stub_entities_in(
    grid: &RouteGrid,
    fb: &Fallback,
    lines: &mut Vec<Vec<Point>>,
    spots: &mut Vec<Point>,
    mut room: Option<&mut TextRoom>,
) -> (Vec<Entity>, Vec<Stub>) {
    let mut out = Vec::new();
    let mut stubs = Vec::new();
    let (rotation, template_attrs) = fb.template.as_ref().map_or((0, Default::default()), |t| (t.rotation, t.attrs.clone()));
    let template_tag = fb.template.as_ref().and_then(crate::net_tag::tag_dir);
    for (pos, dir, attrs) in &fb.terms {
        let Some(dir) = dir else { continue };
        let (sx, sy) = dir_step(*dir);
        let mut cands: Vec<Vec<Point>> =
            [1.0, 2.0, 3.0].iter().map(|k| vec![*pos, pos.translated(sx * STUB_MM * k, sy * STUB_MM * k)]).collect();
        if let Some(l) = make_lead(grid, *pos, Some(*dir)) {
            if l.points.len() >= 2 {
                cands.push(simplify(l.points));
            }
        }
        // ピンの向きに出せないときは、ピンの向きへ少し出てから横へ曲げる (すぐ前に別の線があるときのため)
        for k in [1.0, 2.0] {
            for (px, py) in [(sy, sx), (-sy, -sx)] {
                let a = pos.translated(sx * STUB_MM * 0.5, sy * STUB_MM * 0.5);
                cands.push(vec![*pos, a, a.translated(px * STUB_MM * k, py * STUB_MM * k)]);
            }
        }
        // それも触れるなら、ピンからすぐ横へ出す
        for k in [1.0, 2.0] {
            for (px, py) in [(sy, sx), (-sy, -sx)] {
                cands.push(vec![*pos, pos.translated(px * STUB_MM * k, py * STUB_MM * k)]);
            }
        }
        let label_at = |end: Point, tag: Option<&str>| {
            let mut attrs = template_attrs.clone();
            if let Some(t) = tag {
                attrs.insert(crate::net_tag::TAG_ATTR.to_string(), t.to_string());
            }
            NetLabel { id: uuid::Uuid::nil(), at: end, name: fb.name.clone(), rotation, attrs }
        };
        let mut tag_of: Option<&'static str> = None;
        let mut chosen: Option<Vec<Point>> = None;
        if let Some(room) = room.as_deref() {
            // 文字が横に収まる長さのまっすぐな線を、3本の短い線のあとへ足す
            let width = crate::tidy::net_label_text_rect_in(&label_at(*pos, None), room.scale, room.style, None).0;
            let width = width.max.x - width.min.x;
            let long = (width / STUB_MM).ceil() * STUB_MM + STUB_MM;
            let mut ordered = cands.clone();
            if long > STUB_MM * 3.0 + EPS {
                for (n, extra) in [0.0, 1.0, 2.0].iter().enumerate() {
                    let l = long + STUB_MM * extra;
                    ordered.insert(3 + n, vec![*pos, pos.translated(sx * l, sy * l)]);
                }
            }
            'search: for pts in &ordered {
                // 触れる線・他の部品を突き抜ける線は使わない
                if stub_hits(pts, lines, spots) || !room.clear_of_bodies(pts) {
                    continue;
                }
                let (a, b) = (pts[pts.len() - 2], pts[pts.len() - 1]);
                let end = b;
                // タグ付きの手本: 西向きなら本体は左 (右向きの旗)、東向きなら本体は右。縦なら手本の側と逆の側の両方を試す
                let tags: Vec<Option<&'static str>> = match template_tag {
                    None => vec![None],
                    Some(side) => {
                        let (own, other) = match side {
                            crate::net_tag::TagDir::Left => ("left", "right"),
                            crate::net_tag::TagDir::Right => ("right", "left"),
                        };
                        if (a.y - b.y).abs() < EPS && b.x < a.x - EPS {
                            vec![Some("right")]
                        } else if (a.y - b.y).abs() < EPS && b.x > a.x + EPS {
                            vec![Some("left")]
                        } else {
                            vec![Some(own), Some(other)]
                        }
                    }
                };
                for tag in tags {
                    let probe = label_at(end, tag);
                    let dest = crate::tidy::inside_dest(&probe, room.style, &room.dests);
                    let (text, dest_rect) = crate::tidy::net_label_text_rect_in(&probe, room.scale, room.style, dest);
                    let text = match dest_rect {
                        Some(d) => crate::tidy::Rect::from_corners(
                            Point::new(text.min.x.min(d.min.x), text.min.y.min(d.min.y)),
                            Point::new(text.max.x.max(d.max.x), text.max.y.max(d.max.y)),
                        ),
                        None => text,
                    };
                    // 自分の短い線は避ける対象に入れない (まだ `lines` に入っていない)
                    // 図枠からはみ出す・表題欄に掛かる候補も使わない (最後の収まりの検算を新しく破らない)
                    if !room.blocked(&text, lines) && room.area.as_ref().is_none_or(|a| a.allows(pts, &text)) {
                        chosen = Some(pts.clone());
                        tag_of = tag;
                        break 'search;
                    }
                }
            }
        }
        let pts = chosen.unwrap_or_else(|| {
            cands
                .iter()
                .find(|c| !stub_hits(c, lines, spots))
                .or_else(|| cands.iter().find(|c| !stub_touches(c, lines, spots)))
                .cloned()
                .unwrap_or_else(|| cands[0].clone())
        });
        let end = *pts.last().expect("2点以上");
        let placed = label_at(end, tag_of);
        if let Some(room) = room.as_deref_mut() {
            let dest = crate::tidy::inside_dest(&placed, room.style, &room.dests);
            let (r, dest_rect) = crate::tidy::net_label_text_rect_in(&placed, room.scale, room.style, dest);
            room.rects.push((None, r));
            room.rects.extend(dest_rect.map(|d| (None, d)));
            if let Some(a) = room.area.as_mut() {
                a.content = a.grown(&pts);
            }
        }
        lines.push(pts.clone());
        spots.push(end);
        let (wire, label) = (uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
        out.push(Entity::Wire(Wire { id: wire, points: pts, ..attrs.clone() }));
        out.push(Entity::NetLabel(NetLabel { id: label, ..placed }));
        stubs.push(Stub { wire, label, name: fb.name.clone(), check: true });
    }
    (out, stubs)
}

/// ピンの分け方: ピンを持つネットごとの (シンボルid, ピン番号) の集合の集合。
fn pin_partition(sheet: &Sheet, defs: &[SymbolDef]) -> BTreeSet<BTreeSet<(EntityId, String)>> {
    extract_netlist(sheet, defs)
        .iter()
        .filter(|n| !n.pins.is_empty())
        .map(|n| n.pins.iter().map(|p| (p.entity_id, p.pin.clone())).collect())
        .collect()
}

/// ピンの分け方の違いを、参照記号.ピン番号で短く書く (前だけにある組と後だけにある組を3つまで)。
fn partition_diff(sheet: &Sheet, before: &BTreeSet<BTreeSet<(EntityId, String)>>, after: &BTreeSet<BTreeSet<(EntityId, String)>>) -> String {
    let name = |(id, pin): &(EntityId, String)| -> String {
        match sheet.entities.get(id) {
            Some(Entity::Symbol(s)) if !s.reference.is_empty() => format!("{}.{pin}", s.reference),
            _ => format!("{}.{pin}", &id.to_string()[..8]),
        }
    };
    let show = |set: Vec<&BTreeSet<(EntityId, String)>>| -> String {
        set.iter().take(3).map(|g| g.iter().take(6).map(name).collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join(" / ")
    };
    format!(
        "前: {} 後: {}",
        show(before.difference(after).collect()),
        show(after.difference(before).collect())
    )
}

/// ネットごとの (ピンの集合, 名前の集合) の並び (ピンの無いネットも含む、整列済み)。
/// 名前は、ネットの配線の上かピンの上にあるネットラベルの名前と、ネット名シンボルの名前。`fresh`の名前は数えない。
fn name_key(sheet: &Sheet, defs: &[SymbolDef], fresh: &BTreeSet<String>) -> Vec<(BTreeSet<(EntityId, String)>, BTreeSet<String>)> {
    let idx = DefIndex::new(defs);
    let pp = pin_points(sheet, defs);
    let labels: Vec<&NetLabel> = sheet.entities.values().filter_map(|e| if let Entity::NetLabel(l) = e { Some(l) } else { None }).collect();
    let mut out = Vec::new();
    for n in extract_netlist(sheet, defs) {
        let pins: BTreeSet<(EntityId, String)> = n.pins.iter().map(|q| (q.entity_id, q.pin.clone())).collect();
        let pos: Vec<Point> = pp.iter().filter(|q| pins.contains(&(q.entity, q.number.clone()))).map(|q| q.pos).collect();
        let ws: Vec<&Wire> =
            n.wire_ids.iter().filter_map(|id| if let Some(Entity::Wire(w)) = sheet.entities.get(id) { Some(w) } else { None }).collect();
        let mut names: BTreeSet<String> = labels
            .iter()
            .filter(|l| ws.iter().any(|w| on_wire(w, &l.at)) || pos.iter().any(|p| near(*p, l.at)))
            .map(|l| l.name.clone())
            .collect();
        for (sym, _) in &pins {
            if let Some(Entity::Symbol(s)) = sheet.entities.get(sym) {
                if let Some(nm) = idx.get(s).and_then(|d| d.net_name().map(str::to_string)) {
                    names.insert(nm);
                }
            }
        }
        names.retain(|nm| !fresh.contains(nm));
        out.push((pins, names));
    }
    out.sort();
    out
}

/// ラベルでつなぐ短い線が、自分のピンと、同じ名前の短い線・ラベルのほかに何にも触れていないか。
fn check_stubs(after: &Sheet, defs: &[SymbolDef], stubs: &[Stub]) -> Result<(), String> {
    let pp = pin_points(after, defs);
    for s in stubs.iter().filter(|s| s.check) {
        let Some(Entity::Wire(w)) = after.entities.get(&s.wire) else { continue };
        let Some(start) = w.points.first().copied() else { continue };
        let own: BTreeSet<EntityId> = stubs.iter().filter(|o| o.name == s.name).flat_map(|o| [o.wire, o.label]).collect();
        let ends = |x: &Wire| -> Vec<Point> { [x.points.first(), x.points.last()].into_iter().flatten().copied().collect() };
        let hit = after.entities.values().find_map(|e| {
            let t = match e {
                _ if own.contains(&e.id()) => false,
                Entity::Wire(o) => ends(o).iter().any(|q| on_wire(w, q)) || ends(w).iter().any(|q| on_wire(o, q)),
                Entity::Junction(j) => on_wire(w, &j.at),
                Entity::NetLabel(l) => on_wire(w, &l.at),
                _ => false,
            };
            t.then(|| match e {
                Entity::NetLabel(l) => format!("ラベル {}", l.name),
                Entity::Junction(_) => "ジャンクション".to_string(),
                _ => "配線".to_string(),
            })
        });
        let hit = hit.or_else(|| {
            pp.iter().find(|q| on_wire(w, &q.pos) && !near(q.pos, start)).map(|q| {
                let r = if q.reference.is_empty() { "ピン".to_string() } else { q.reference.clone() };
                format!("{r}.{}", q.number)
            })
        });
        if let Some(h) = hit {
            return Err(format!(
                "並べ直しの検算で、ラベル {} の引き出し線 ({:.1}, {:.1}) が {h} に触れたため、何も変えませんでした",
                s.name, start.x, start.y
            ));
        }
    }
    Ok(())
}

/// 検算: 計画を当てた写しで、ピンの分け方・ネットごとの名前 (新しい `ARR_<n>` を除く)・ラベルでつなぐ短い線の孤立を確かめる。
fn verify_after(before: &Sheet, after: &Sheet, defs: &[SymbolDef], fresh: &BTreeSet<String>, stubs: &[Stub]) -> Result<(), String> {
    // 並べ直しは部品を消さない (消えた部品があれば断る)
    if before.entities.values().any(|e| matches!(e, Entity::Symbol(_)) && !after.entities.contains_key(&e.id())) {
        return Err("並べ直しの検算で部品が消えたため、何も変えませんでした".into());
    }
    let pb = pin_partition(before, defs);
    let pa = pin_partition(after, defs);
    if pb != pa {
        return Err(format!("並べ直しの検算で接続が変わったため、何も変えませんでした ({})", partition_diff(before, &pb, &pa)));
    }
    let mut nb: Vec<(BTreeSet<(EntityId, String)>, BTreeSet<String>)> =
        name_key(before, defs, fresh).into_iter().filter(|(g, n)| !(g.is_empty() && n.is_empty())).collect();
    nb.sort();
    let na: Vec<(BTreeSet<(EntityId, String)>, BTreeSet<String>)> =
        name_key(after, defs, fresh).into_iter().filter(|(g, n)| !(g.is_empty() && n.is_empty())).collect();
    if nb != na {
        return Err("並べ直しの検算でネットの名前 (ラベル・ネット名シンボル) が変わったため、何も変えませんでした".into());
    }
    // 配線の無いピンの上のラベル・ネット名シンボルはネットリストに出ないので、位置の重なりで比べる
    let (pb, pa) = (pin_contacts(before, defs), pin_contacts(after, defs));
    let both = |set: &BTreeSet<(EntityId, EntityId, String)>, other: &Sheet| -> BTreeSet<(EntityId, EntityId, String)> {
        set.iter().filter(|c| other.entities.contains_key(&c.0) && other.entities.contains_key(&c.1)).cloned().collect()
    };
    let (xb, xa) = (both(&pb, after), both(&pa, before));
    if xb != xa {
        let name = |id: &EntityId| -> String {
            match before.entities.get(id) {
                Some(Entity::Symbol(s)) if !s.reference.is_empty() => s.reference.clone(),
                Some(Entity::Symbol(s)) => s.symbol_id.clone(),
                Some(Entity::NetLabel(l)) => format!("ラベル {}", l.name),
                _ => id.to_string()[..8].to_string(),
            }
        };
        let gone: Vec<String> = xb.difference(&xa).take(3).map(|c| format!("{} と {}.{}", name(&c.0), name(&c.1), c.2)).collect();
        let new: Vec<String> = xa.difference(&xb).take(3).map(|c| format!("{} と {}.{}", name(&c.0), name(&c.1), c.2)).collect();
        return Err(format!(
            "並べ直しの検算で、ピンの上のラベルやネット名シンボルが離れたため、何も変えませんでした (離れた: {} / 新しく重なった: {})",
            gone.join(", "),
            new.join(", ")
        ));
    }
    check_stubs(after, defs, stubs)
}

/// ピンの上に直接あるネットラベルとネット名シンボルのピン: (ラベルかシンボルのid, ピンのシンボルid, ピン番号)。
fn pin_contacts(sheet: &Sheet, defs: &[SymbolDef]) -> BTreeSet<(EntityId, EntityId, String)> {
    let idx = DefIndex::new(defs);
    let pp = pin_points(sheet, defs);
    let mut out = BTreeSet::new();
    for e in sheet.entities.values() {
        let at: Vec<Point> = match e {
            Entity::NetLabel(l) => vec![l.at],
            Entity::Symbol(s) if idx.get(s).is_some_and(|d| d.is_net_mark()) => {
                pp.iter().filter(|q| q.entity == s.id).map(|q| q.pos).collect()
            }
            _ => continue,
        };
        for q in pp.iter().filter(|q| q.entity != e.id() && at.iter().any(|a| near(*a, q.pos))) {
            out.insert((e.id(), q.entity, q.number.clone()));
        }
    }
    out
}

/// 端子が全て同じ単位のものか (それなら配線はその単位と一緒に形を変えずに動かせる。部品と電源マークの間の線など)。
fn single_unit(owns: &[usize]) -> bool {
    owns.first().is_some_and(|f| owns.iter().all(|o| o == f))
}

/// 端子の単位が全て動かず、配線とジャンクションの全ての点が写像で動かないか (それなら元の配線をそのまま残せる)。
fn unit_stays(sheet: &Sheet, wires: &[EntityId], junctions: &[EntityId], owns: &[usize], lay: &Layout) -> bool {
    if owns.is_empty() || owns.iter().any(|&o| lay.dx[o].abs() > EPS || lay.dy[o].abs() > EPS) {
        return false;
    }
    let fixed = |p: &Point| (lay.wx.map(p.x) - p.x).abs() < EPS && (lay.wy.map(p.y) - p.y).abs() < EPS;
    wires.iter().chain(junctions).all(|id| sheet.entities.get(id).map_or(true, |e| shape_points(e).iter().all(fixed)))
}

/// 接続点として描かれた図形の小さな円 (半径1mm以下): (id, 中心)。
fn dot_shapes(sheet: &Sheet) -> Vec<(EntityId, Point)> {
    sheet
        .entities
        .values()
        .filter_map(|e| match e {
            Entity::Shape(sh) if sh.shape == crate::model::ShapeKind::Circle && sh.points.len() >= 2 => {
                let (c, r) = (sh.points[0], sh.points[1]);
                ((c.x - r.x).hypot(c.y - r.y) <= 1.0).then_some((sh.id, c))
            }
            _ => None,
        })
        .collect()
}

/// 点がどれかの配線の上にあるか。
fn on_any_wire(sheet: &Sheet, p: Point) -> bool {
    sheet.entities.values().any(|e| matches!(e, Entity::Wire(w) if on_wire(w, &p)))
}

/// 計画をシートの写しに当てる (消す → 動かす → 置き換える → 足す)。
fn apply_plan(sheet: &Sheet, plan: &ArrangePlan) -> Sheet {
    let mut s = sheet.clone();
    for id in &plan.remove {
        s.entities.remove(id);
    }
    for (ids, dx, dy) in &plan.moves {
        for id in ids {
            if let Some(e) = s.entities.get_mut(id) {
                e.translate(*dx, *dy);
            }
        }
    }
    for e in &plan.updates {
        if s.entities.contains_key(&e.id()) {
            s.entities.insert(e.id(), e.clone());
        }
    }
    for e in &plan.add {
        s.entities.insert(e.id(), e.clone());
    }
    s
}

/// ネットごとのラベル名 (同じネットの別のかたまりのラベルも含む。ポートの接点のネットは除く)。
fn net_label_names(sheet: &Sheet, defs: &[SymbolDef]) -> BTreeMap<String, String> {
    // ポートの接点のネットのラベル名は「<ラベル>.<接点>」で、図面にその文字のラベルは無いので借りない
    extract_netlist(sheet, defs)
        .into_iter()
        .filter(|n| n.contact.is_none())
        .filter_map(|n| n.label.clone().map(|l| (n.name.clone(), l)))
        .collect()
}

/// 引けた経路の形: 配線の点列とジャンクションの位置。
fn route_shape(u: &RouteUnit, r: &crate::route::UnitRoute) -> (Vec<Vec<Point>>, Vec<Point>) {
    let mut lines = Vec::new();
    let mut dots = Vec::new();
    for e in emit(u, r) {
        match e {
            Entity::Wire(w) => lines.push(w.points),
            Entity::Junction(j) => dots.push(j.at),
            _ => {}
        }
    }
    (lines, dots)
}

/// 引けた経路 (線 `lines`・ジャンクション `dots`) が、置いた線・点に触れるか。線どうしがただ交わるだけ
/// (端も頂点も相手に乗らない) ならつながらないので数えない。許容差は [`path_hits`] と同じ (線 0.3 mm・点 1 mm)。
fn route_touches(lines: &[Vec<Point>], dots: &[Point], new_lines: &[Vec<Point>], new_spots: &[Point]) -> bool {
    let near_line = |q: Point, line: &[Point], d: f64| line.windows(2).any(|w| seg_dist(q, w[0], w[1]) < d);
    lines.iter().any(|l| {
        new_spots.iter().any(|&q| near_line(q, l, 1.0))
            || new_lines.iter().any(|n| n.iter().any(|&q| near_line(q, l, 0.3)) || l.iter().any(|&q| near_line(q, n, 0.3)))
    }) || dots.iter().any(|&q| new_lines.iter().any(|n| near_line(q, n, 0.3)))
}

/// 配線の引き直しを回に分けて行う。引けなかった単位ごとに `on_fail(i)` を呼び (ラベルの短い線・伸び縮みさせた
/// 配線を置く)、置いた線と点を返してもらう。次の回は、その回に置いた線・点に触れる経路の単位だけを、置いたもの全てと
/// 残した経路を避けて引き直す。新しく引けない単位が出なくなったら終わる。引けない単位は回ごとに1つ以上減るので、
/// 回数は単位の数 + 1 以下。返り値は単位ごとの経路 (引けなければ None) と、回ごとに引いた単位の番号。
pub(crate) fn route_rounds(
    grid: &RouteGrid,
    route: &[RouteUnit],
    fixed_lines: &[Vec<Point>],
    fixed_spots: &[Point],
    mut on_fail: impl FnMut(usize) -> (Vec<Vec<Point>>, Vec<Point>),
) -> (Vec<Option<crate::route::UnitRoute>>, Vec<Vec<usize>>) {
    let mut outcome: Vec<Option<crate::route::UnitRoute>> = (0..route.len()).map(|_| None).collect();
    let mut extra_lines: Vec<Vec<Point>> = Vec::new();
    let mut extra_spots: Vec<Point> = Vec::new();
    let mut todo: Vec<usize> = (0..route.len()).collect();
    let mut rounds: Vec<Vec<usize>> = Vec::new();
    for _ in 0..=route.len() {
        if todo.is_empty() {
            break;
        }
        // 動かない配線・置いたもの全て・この回に引き直さない経路を避ける
        let mut lines: Vec<Vec<Point>> = fixed_lines.iter().chain(&extra_lines).cloned().collect();
        let mut spots: Vec<Point> = fixed_spots.iter().chain(&extra_spots).copied().collect();
        for (i, r) in outcome.iter().enumerate() {
            if let Some(r) = r.as_ref().filter(|_| !todo.contains(&i)) {
                let (l, d) = route_shape(&route[i], r);
                lines.extend(l);
                spots.extend(d);
            }
        }
        let mut batch: Vec<RouteUnit> = todo.iter().map(|&i| route[i].clone()).collect();
        let results = route_units(grid, &mut batch, lines, spots);
        rounds.push(todo.clone());
        let mut new_lines: Vec<Vec<Point>> = Vec::new();
        let mut new_spots: Vec<Point> = Vec::new();
        let mut new_failed = false;
        for (&i, r) in todo.iter().zip(results) {
            match r {
                Ok(r) => outcome[i] = Some(r),
                Err(_) => {
                    new_failed = true;
                    outcome[i] = None;
                    let (l, s) = on_fail(i);
                    new_lines.extend(l);
                    new_spots.extend(s);
                }
            }
        }
        if !new_failed {
            break;
        }
        // 次の回は、この回に置いた線・点に触れる経路だけを引き直す
        todo = (0..route.len())
            .filter(|&i| {
                outcome[i].as_ref().is_some_and(|r| {
                    let (l, d) = route_shape(&route[i], r);
                    route_touches(&l, &d, &new_lines, &new_spots)
                })
            })
            .collect();
        extra_lines.extend(new_lines);
        extra_spots.extend(new_spots);
    }
    (outcome, rounds)
}

/// 部品を並べ直し、配線を引き直す計画を作る。図面は書き換えない。
///
/// 整列と通り道の確保で部品を動かし、動かした後の形で全ての配線を引き直す。引けないかたまりはラベルでつなぐ。
/// 接続が変わる計画は作らない (計画を写しに当てて、ピンの分け方とネットごとの名前を比べる)。
/// 端子の単位が動かないかたまりは、元の配線をそのまま残す。
pub fn plan_arrange(sheet: &Sheet, defs: &[SymbolDef], opts: &ArrangeOptions, ctx: &ArrangeContext) -> Result<ArrangePlan, String> {
    opts.validate()?;
    refuse_unsupported(sheet)?;
    let c = collect(sheet, defs, opts, ctx)?;
    let us = units(sheet, defs);
    let idx = DefIndex::new(defs);
    let before = content_bounds(sheet, &idx);
    if us.is_empty() {
        let size = size_of(before);
        return Ok(ArrangePlan { size_before: size, size_after: size, after: Some(sheet.clone()), ..Default::default() });
    }
    let owners = Owners::new(sheet, defs, &us);
    let pins = pin_points(sheet, defs);
    refuse_mixed_names(sheet, &idx, &pins, &c)?;
    let fates = classify_kept(&c.kept, &owners)?;
    let unit_terms: Vec<Vec<usize>> = c.units.iter().map(|u| term_units(&u.net, &u.terms, &owners)).collect::<Result<_, _>>()?;
    let mut taken = taken_set(sheet, defs, ctx);
    // ネットごとのラベル名 (同じネットの別のかたまりのラベルも含む)
    let net_labels = net_label_names(sheet, defs);
    // 他から指されているid (消さない)
    let pointed: BTreeSet<EntityId> = sheet.entities.values().flat_map(|e| e.refs()).chain(ctx.protected.iter().copied()).collect();
    // この計画で新しく作った `ARR_<n>` の名前 (検算で数えない)
    let mut fresh: BTreeSet<String> = BTreeSet::new();

    // 取り除くもの・剛体のかたまり・ラベルでつなぐかたまりの名前
    let mut dropped: BTreeSet<EntityId> = BTreeSet::new();
    let mut deleted: BTreeSet<EntityId> = BTreeSet::new();
    let mut rigid: BTreeMap<EntityId, usize> = BTreeMap::new();
    for u in &c.units {
        dropped.extend(u.wires.iter().chain(&u.junctions).copied());
    }
    let mut kept_fallbacks: Vec<(usize, FallbackName)> = Vec::new();
    for (ci, (k, fate)) in c.kept.iter().zip(&fates).enumerate() {
        match fate {
            Fate::Rigid(Some(i)) => {
                for id in k.wires.iter().chain(&k.junctions) {
                    rigid.insert(*id, *i);
                }
            }
            Fate::Rigid(None) => {}
            Fate::Labels => {
                dropped.extend(k.wires.iter().chain(&k.junctions).copied());
                let fb = fallback_name(sheet, &idx, &pins, &owners, &k.net, &net_labels, &k.wires, &k.terms, &pointed, &mut taken);
                deleted.extend(fb.delete.iter().copied());
                if fb.fresh {
                    fresh.insert(fb.name.clone());
                }
                kept_fallbacks.push((ci, fb));
            }
        }
    }
    dropped.extend(deleted.iter().copied());

    // 列・行の配置。端子の単位が動かないかたまりは、元の配線をそのまま残す (動かない障害物)
    let arrange_with = |max_gap: f64| {
        let lay = layout(&us, opts, max_gap);
        let stays: Vec<bool> =
            c.units.iter().zip(&unit_terms).map(|(u, owns)| single_unit(owns) || unit_stays(sheet, &u.wires, &u.junctions, owns, &lay)).collect();
        let mut rigid = rigid.clone();
        let mut dropped = dropped.clone();
        for ((u, owns), s) in c.units.iter().zip(&unit_terms).zip(&stays) {
            if *s {
                for id in u.wires.iter().chain(&u.junctions) {
                    dropped.remove(id);
                    rigid.insert(*id, owns[0]);
                }
            }
        }
        let moved = build_moved(sheet, &lay, &owners.unit_of, &rigid, &dropped);
        (lay, stays, rigid, moved)
    };

    // 図枠への収まり
    let hit_title = before.is_some_and(|b| rects_hit(b, ctx.title_block));
    let (mut lay, mut stays, mut rigid, mut moved) = arrange_with(opts.max_gap_mm);
    if !fits(content_bounds(&moved, &idx), ctx, hit_title) {
        (lay, stays, rigid, moved) = arrange_with(opts.channel_mm);
        let b = content_bounds(&moved, &idx);
        if !fits(b, ctx, hit_title) {
            let (w, h) = size_of(b);
            return Err(format!("並べ直すと図枠に収まりません (幅 {w:.1} mm, 高さ {h:.1} mm)"));
        }
    }

    // 引き直す単位 (動かないかたまりを除く) の端子を、移動後の位置へ
    let mut route: Vec<RouteUnit> = Vec::new();
    let mut trips: Vec<Vec<(Point, Point, Option<Dir>)>> = Vec::new();
    for ((mut u, owns), s) in c.units.into_iter().zip(&unit_terms).zip(&stays) {
        if *s {
            continue;
        }
        let trip: Vec<(Point, Point, Option<Dir>)> =
            u.terms.iter().zip(owns).map(|(t, &o)| (t.0, t.0.translated(lay.dx[o], lay.dy[o]), t.1)).collect();
        u.terms = trip.iter().map(|t| (t.1, t.2)).collect();
        u.terms.sort_by(|a, b| a.0.x.total_cmp(&b.0.x).then(a.0.y.total_cmp(&b.0.y)));
        u.old_lines.clear();
        u.old_junctions.clear();
        u.leads.clear();
        trips.push(trip);
        route.push(u);
    }

    // 格子と動かない配線。ラベルでつなぐ短い線も動かない配線として先に置く
    let grid = RouteGrid::build(&moved, defs, &opts.route);
    let mut fixed_lines: Vec<Vec<Point>> =
        moved.entities.values().filter_map(|e| if let Entity::Wire(w) = e { Some(w.points.clone()) } else { None }).collect();
    let fixed_spots: Vec<Point> =
        moved.entities.values().filter_map(|e| if let Entity::Junction(j) = e { Some(j.at) } else { None }).collect();
    let mut add: Vec<Entity> = Vec::new();
    let mut stubs: Vec<Stub> = Vec::new();
    // 元の形で伸び縮みさせる配線が横切ってはいけない部品の外形 (動かした後)
    let bodies: Vec<(EntityId, Bounds)> = moved
        .entities
        .values()
        .filter(|e| matches!(e, Entity::Symbol(_)))
        .filter_map(|e| entity_bounds(e, &idx).map(|r| (e.id(), rect_tuple(&r))))
        .collect();
    // 短い線が避けるもの: 動かした後の配線・ピン・ラベル・ジャンクション (置いた短い線も足していく)
    let mut stub_lines: Vec<Vec<Point>> = fixed_lines.clone();
    let mut stub_spots: Vec<Point> = pin_points(&moved, defs)
        .iter()
        .map(|q| q.pos)
        .chain(moved.entities.values().filter_map(|e| match e {
            Entity::NetLabel(l) => Some(l.at),
            Entity::Junction(j) => Some(j.at),
            _ => None,
        }))
        .collect();
    // ラベルの文字が読める場所を探すための、動かした後の部品の外形と文字 (置いたラベルの文字も足していく)
    let mut room = TextRoom::build_with(&moved, defs, &dropped, ctx.tag_dests.clone());
    room.area = Some(StubArea::new(ctx, hit_title, content_bounds(&moved, &idx)));
    let mut remove: BTreeSet<EntityId> = deleted.clone();
    let mut labelled: Vec<(String, String)> = Vec::new();
    for (ci, name) in kept_fallbacks {
        let k = &c.kept[ci];
        let owns = term_units(&k.net, &k.terms, &owners)?;
        let terms = k
            .terms
            .iter()
            .zip(&owns)
            .filter(|(t, _)| !name.skip.iter().any(|q| near(*q, t.0)))
            .map(|(t, &o)| (t.0.translated(lay.dx[o], lay.dy[o]), t.1, old_wire_at(sheet, &k.wires, t.0)))
            .collect();
        for e in kept_links(sheet, &owners, &lay, &name.keep) {
            if let Entity::Wire(w) = &e {
                fixed_lines.push(w.points.clone());
                stub_lines.push(w.points.clone());
                stubs.push(Stub { wire: w.id, label: w.id, name: name.name.clone(), check: false });
            }
            add.push(e);
        }
        let fb = Fallback { net: k.net.clone(), name: name.name, terms, template: name.template };
        let (es, ss) = stub_entities_in(&grid, &fb, &mut stub_lines, &mut stub_spots, Some(&mut room));
        for e in es {
            if let Entity::Wire(w) = &e {
                fixed_lines.push(w.points.clone());
            }
            add.push(e);
        }
        stubs.extend(ss);
        remove.extend(k.wires.iter().chain(&k.junctions).copied());
        labelled.push((fb.net, fb.name));
    }

    // 配線の引き直し。引けなかった単位はラベルでつなぐ。ラベルでつなぐ短い線は他の配線が避けられるよう、
    // 動かない配線として置いてから、残りの単位を引き直す (新しく引けない単位が出なくなるまで)
    let mut names: BTreeMap<usize, FallbackName> = BTreeMap::new();
    let mut stub_parts: BTreeMap<usize, (Vec<Entity>, Vec<Stub>)> = BTreeMap::new();
    let (outcome, _rounds) = route_rounds(&grid, &route, &fixed_lines, &fixed_spots, |i| {
        let (l0, s0) = (stub_lines.len(), stub_spots.len());
        let u = &route[i];
        let trip = &trips[i];
        // まず元の形のまま伸び縮みさせる。触れるならラベルでつなぐ
        let own: BTreeSet<EntityId> = trip.iter().flat_map(|t| pins.iter().filter(move |q| near(q.pos, t.0)).map(|q| q.entity)).collect();
        if let Some((es, paths)) = stretch_wires(sheet, &u.wires, &u.junctions, &lay, trip, &stub_lines, &stub_spots, &bodies, &own) {
            stub_lines.extend(paths);
            stub_spots.extend(es.iter().filter_map(|e| if let Entity::Junction(j) = e { Some(j.at) } else { None }));
            stub_parts.insert(i, (es, Vec::new()));
        } else {
            let old_terms: Vec<(Point, Option<Dir>)> = trip.iter().map(|t| (t.0, t.2)).collect();
            let name = fallback_name(sheet, &idx, &pins, &owners, &u.net, &net_labels, &u.wires, &old_terms, &pointed, &mut taken);
            let mut attrs = u.template.clone();
            attrs.length_m = None;
            attrs.length_source = Default::default();
            let terms = trip.iter().filter(|t| !name.skip.iter().any(|q| near(*q, t.0))).map(|t| (t.1, t.2, attrs.clone())).collect();
            let fb = Fallback { net: u.net.clone(), name: name.name.clone(), terms, template: name.template.clone() };
            // 置き換える古いラベルの文字は避ける対象から外す
            room.forget(&name.delete);
            let links = kept_links(sheet, &owners, &lay, &name.keep);
            for e in &links {
                if let Entity::Wire(w) = e {
                    stub_lines.push(w.points.clone());
                }
            }
            let (mut es, mut ss) = stub_entities_in(&grid, &fb, &mut stub_lines, &mut stub_spots, Some(&mut room));
            for e in &links {
                if let Entity::Wire(w) = e {
                    ss.push(Stub { wire: w.id, label: w.id, name: fb.name.clone(), check: false });
                }
            }
            es.extend(links);
            stub_parts.insert(i, (es, ss));
            names.insert(i, name);
        }
        (stub_lines[l0..].to_vec(), stub_spots[s0..].to_vec())
    });
    let mut routed: BTreeSet<String> = BTreeSet::new();
    let mut kept_shape: BTreeSet<String> = BTreeSet::new();
    for (i, u) in route.iter().enumerate() {
        remove.extend(u.wires.iter().chain(&u.junctions).copied());
        if let Some((es, ss)) = stub_parts.remove(&i) {
            let Some(name) = names.remove(&i) else {
                // 元の形のまま伸び縮みさせた配線
                add.extend(es);
                kept_shape.insert(u.net.clone());
                continue;
            };
            if name.fresh {
                fresh.insert(name.name.clone());
            }
            deleted.extend(name.delete.iter().copied());
            remove.extend(name.delete.iter().copied());
            add.extend(es);
            stubs.extend(ss);
            labelled.push((u.net.clone(), name.name));
        } else if let Some(r) = &outcome[i] {
            add.extend(emit(u, r));
            routed.insert(u.net.clone());
        }
    }

    // 動かすもの・置き換えるもの
    let mut groups: BTreeMap<(i64, i64), BTreeSet<EntityId>> = BTreeMap::new();
    let key = |i: usize| ((lay.dx[i] * 1000.0).round() as i64, (lay.dy[i] * 1000.0).round() as i64);
    let mut moved_units = 0;
    for (i, u) in us.iter().enumerate() {
        if key(i) == (0, 0) {
            continue;
        }
        moved_units += 1;
        for id in &u.members {
            if sheet.entities.contains_key(id) && !deleted.contains(id) {
                groups.entry(key(i)).or_default().insert(*id);
            }
        }
    }
    for (id, &i) in &rigid {
        if key(i) != (0, 0) {
            groups.entry(key(i)).or_default().insert(*id);
        }
    }
    let moves: Vec<(Vec<EntityId>, f64, f64)> =
        groups.into_iter().map(|((kx, ky), ids)| (ids.into_iter().collect(), kx as f64 / 1000.0, ky as f64 / 1000.0)).collect();
    let mut updates: Vec<Entity> = Vec::new();
    for (id, e) in &moved.entities {
        if owners.unit_of.contains_key(id) || rigid.contains_key(id) {
            continue;
        }
        if sheet.entities.get(id).is_some_and(|o| geometry_changed(o, e)) {
            updates.push(e.clone());
        }
    }

    let mut plan = ArrangePlan {
        moves,
        updates,
        remove: remove.into_iter().collect(),
        add,
        moved: moved_units,
        columns: lay.columns,
        rows: lay.rows,
        routed: routed.into_iter().collect(),
        labelled,
        kept_shape: kept_shape.into_iter().collect(),
        size_before: size_of(before),
        ..Default::default()
    };

    // 検算: 計画を写しに当てて、接続と名前が変わっていないこと、引いた後も図枠に収まることを確かめる
    // 配線の上に図形の小さな円で描いた接続点 (ジャンクションの代わり) は、配線が動いて線から外れたら消す
    let mut after = apply_plan(sheet, &plan);
    let lost: Vec<EntityId> = dot_shapes(sheet)
        .into_iter()
        .filter(|(_, c)| on_any_wire(sheet, *c))
        .filter(|(id, _)| match after.entities.get(id) {
            Some(Entity::Shape(sh)) => sh.points.first().is_some_and(|c| !on_any_wire(&after, *c)),
            _ => false,
        })
        .map(|(id, _)| id)
        .collect();
    if !lost.is_empty() {
        plan.updates.retain(|e| !lost.contains(&e.id()));
        for (ids, _, _) in plan.moves.iter_mut() {
            ids.retain(|id| !lost.contains(id));
        }
        plan.moves.retain(|m| !m.0.is_empty());
        plan.remove.extend(lost.iter().copied());
        after = apply_plan(sheet, &plan);
    }
    verify_after(sheet, &after, defs, &fresh, &stubs)?;
    let b = content_bounds(&after, &idx);
    if !fits(b, ctx, hit_title) {
        let (w, h) = size_of(b);
        return Err(format!("並べ直して配線を引くと図枠に収まりません (幅 {w:.1} mm, 高さ {h:.1} mm)"));
    }
    plan.size_after = size_of(b);
    plan.after = Some(after);
    Ok(plan)
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
            net_from_value: false,
            through: false,
            sim: None,
        }
    }

    /// 20 x 10 mm box with a pin on the left (number 1) and on the right (number 2).
    fn box2() -> SymbolDef {
        def("box2", vec![rect(-5.0, -5.0, 5.0, 5.0)], vec![pin("1", -10.0, 0.0, PinDir::Left), pin("2", 10.0, 0.0, PinDir::Right)])
    }

    /// A one-pin mark whose pin is 5 mm above its origin.
    fn mark() -> SymbolDef {
        def("mark", vec![rect(-2.0, -2.0, 2.0, 2.0)], vec![pin("1", 0.0, -5.0, PinDir::Up)])
    }

    /// 10 x 20 mm box with a pin on top (number 1) and on the bottom (number 2).
    fn cap() -> SymbolDef {
        def("cap", vec![rect(-5.0, -5.0, 5.0, 5.0)], vec![pin("1", 0.0, -10.0, PinDir::Up), pin("2", 0.0, 10.0, PinDir::Down)])
    }

    /// A pin-less rectangle that only blocks space.
    fn wall(id: &str, w: f64, h: f64) -> SymbolDef {
        def(id, vec![rect(-w / 2.0, -h / 2.0, w / 2.0, h / 2.0)], vec![])
    }

    /// A box with a pin 7.3 mm to the right of the centre (off the grid, number 1) and one on the left (number 2).
    fn freebox() -> SymbolDef {
        let mut d = def(
            "freebox",
            vec![rect(-5.0, -5.0, 5.0, 5.0)],
            vec![pin("1", 7.3, 0.0, PinDir::Right), pin("2", -10.0, 0.0, PinDir::Left)],
        );
        d.free_pins = true;
        d
    }

    /// A ground mark: a one-pin mark that names its net GND.
    fn gnd() -> SymbolDef {
        let mut d = def("gnd", vec![rect(-2.0, -2.0, 2.0, 2.0)], vec![pin("1", 0.0, -5.0, PinDir::Up)]);
        d.net = Some("GND".into());
        d
    }

    fn defs() -> Vec<SymbolDef> {
        vec![box2(), mark(), cap(), wall("wall_h", 70.0, 5.0), wall("wall_v", 5.0, 40.0), freebox(), gnd()]
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

    fn label(sheet: &mut Sheet, at: (f64, f64), name: &str) -> EntityId {
        let id = Uuid::new_v4();
        sheet.entities.insert(
            id,
            Entity::NetLabel(NetLabel { id, at: p(at.0, at.1), name: name.into(), rotation: 0, attrs: Default::default() }),
        );
        id
    }

    fn junction(sheet: &mut Sheet, at: (f64, f64)) -> EntityId {
        let id = Uuid::new_v4();
        sheet.entities.insert(id, Entity::Junction(Junction { id, at: p(at.0, at.1) }));
        id
    }

    fn note(sheet: &mut Sheet, at: (f64, f64)) -> EntityId {
        let id = Uuid::new_v4();
        sheet.entities.insert(
            id,
            Entity::Text(
                serde_json::from_value(serde_json::json!({
                    "id": id,
                    "at": {"x": at.0, "y": at.1},
                    "text": "note",
                    "height": 3.5,
                }))
                .expect("text entity"),
            ),
        );
        id
    }

    fn new_sheet() -> Sheet {
        Sheet::new("S", PaperSize::A3, Orientation::Landscape)
    }

    /// Overlapping spans merge into one band and separate spans stay apart, in order.
    /// 重なる区間は1つの帯にまとまり、離れた区間は別の帯のまま順に並ぶ。
    #[test]
    fn overlapping_spans_merge_into_bands() {
        let b = bands(&[(30.0, 40.0), (0.0, 10.0), (5.0, 15.0)]);
        assert_eq!(b.len(), 2);
        assert_eq!((b[0].lo, b[0].hi), (0.0, 15.0));
        assert_eq!(b[0].members, vec![1, 2]);
        assert_eq!(b[1].members, vec![0]);
    }

    /// No spans give no bands.
    /// 区間が無ければ帯も無い。
    #[test]
    fn an_empty_sheet_gives_no_bands() {
        assert!(bands(&[]).is_empty());
        assert_eq!(Warp::new(&[], &[]).map(12.3), 12.3);
    }

    /// A gap narrower than the channel widens to at least the channel, in 2.5 mm steps.
    /// 通り道より狭い空きは、2.5mm刻みで通り道の幅以上に広がる。
    #[test]
    fn narrow_gaps_widen_to_the_channel() {
        let b = bands(&[(0.0, 10.0), (13.0, 20.0)]);
        let s = band_shifts(&b, 8.0, 20.0);
        assert_eq!(s, vec![0.0, 5.0]);
    }

    /// A gap wider than the maximum shrinks, but never below the maximum.
    /// 上限より広い空きは詰まるが、上限より狭くはならない。
    #[test]
    fn wide_gaps_shrink_to_the_maximum() {
        let b = bands(&[(0.0, 10.0), (51.0, 60.0)]);
        let s = band_shifts(&b, 8.0, 20.0);
        assert_eq!(s, vec![0.0, -20.0]);
        assert!(51.0 + s[1] - 10.0 >= 20.0);
    }

    /// The warp moves points inside a band with the band and stretches the gaps between bands, keeping order.
    /// 写像は帯の中の点を帯と一緒に動かし、帯の間の空きは順序を保ったまま伸び縮みさせる。
    #[test]
    fn the_warp_is_monotone_and_follows_bands() {
        let b = bands(&[(0.0, 10.0), (13.0, 20.0)]);
        let w = Warp::new(&b, &[0.0, 5.0]);
        assert_eq!(w.map(5.0), 5.0);
        assert_eq!(w.map(15.0), 20.0);
        assert_eq!(w.map(11.5), 14.0);
        assert_eq!(w.map(100.0), 105.0);
        let xs: Vec<f64> = (0..250).map(|i| i as f64 * 0.1).collect();
        assert!(xs.windows(2).all(|p| w.map(p[0]) <= w.map(p[1])));
    }

    /// Nearly equal keys line up on the lower median; distant keys stay put.
    /// ほぼ同じ座標は下側の中央値にそろい、離れた座標は動かない。
    #[test]
    fn nearly_aligned_keys_snap_to_the_median() {
        let m = align_moves(&[100.0, 102.5, 140.0], 4.0);
        assert_eq!(m, vec![0.0, -2.5, 0.0]);
        assert_eq!(align_moves(&[100.0, 102.5], 0.0), vec![0.0, 0.0]);
    }

    /// A power mark wired only to one part becomes that part's satellite and moves with it.
    /// 1つの部品だけにつながる電源マークは、その部品の衛星になり一緒に動く。
    #[test]
    fn a_mark_wired_to_one_part_is_its_satellite() {
        let mut sh = new_sheet();
        let a1 = place(&mut sh, "box2", (50.0, 50.0), "A1");
        let m = place(&mut sh, "mark", (60.0, 60.0), "M1");
        old_wire(&mut sh, &[(60.0, 50.0), (60.0, 55.0)]);
        let us = units(&sh, &defs());
        assert_eq!(us.len(), 1);
        assert_eq!(us[0].main, a1);
        assert!(us[0].members.contains(&a1) && us[0].members.contains(&m));
    }

    /// A label wired to pins of two different parts is its own unit.
    /// 2つの部品のピンにつながるラベルは、それだけで1つの単位になる。
    #[test]
    fn a_label_between_two_symbols_is_its_own_unit() {
        let mut sh = new_sheet();
        place(&mut sh, "box2", (50.0, 50.0), "A1");
        place(&mut sh, "box2", (100.0, 50.0), "A2");
        old_wire(&mut sh, &[(60.0, 50.0), (90.0, 50.0)]);
        let l = label(&mut sh, (75.0, 50.0), "SIG");
        let us = units(&sh, &defs());
        assert_eq!(us.len(), 3);
        let lu = us.iter().find(|u| u.main == l).expect("label unit");
        assert_eq!(lu.members, vec![l]);
    }

    /// A note within 3 mm of a part moves with it; a note far away belongs to no unit.
    /// 部品から3mm以内の注記は部品と一緒に動き、遠い注記はどの単位にも入らない。
    #[test]
    fn nearby_notes_join_their_part() {
        let mut sh = new_sheet();
        let a1 = place(&mut sh, "box2", (50.0, 50.0), "A1");
        let near = note(&mut sh, (50.0, 57.0));
        let far = note(&mut sh, (50.0, 80.0));
        let us = units(&sh, &defs());
        assert_eq!(us.len(), 1);
        assert_eq!(us[0].main, a1);
        assert!(us[0].members.contains(&near));
        assert!(!us[0].members.contains(&far));
    }

    /// Out-of-range options are refused with a message naming the field.
    /// 範囲外の設定は、項目名を含むメッセージで断られる。
    #[test]
    fn bad_options_are_refused() {
        let mut o = ArrangeOptions::default();
        o.channel_mm = 1.0;
        assert!(o.validate().unwrap_err().contains("channel_mm"));
        let mut o = ArrangeOptions::default();
        o.max_gap_mm = 5.0;
        assert!(o.validate().unwrap_err().contains("max_gap_mm"));
        let mut o = ArrangeOptions::default();
        o.align_mm = f64::NAN;
        assert!(o.validate().unwrap_err().contains("align_mm"));
        assert!(ArrangeOptions::default().validate().is_ok());
    }

    // ------------------------------------------------------------------
    // 並べ直しの計画
    // ------------------------------------------------------------------

    fn ctx() -> ArrangeContext {
        ArrangeContext { frame: (10.0, 10.0, 410.0, 287.0), title_block: (230.0, 240.0, 410.0, 287.0), ..Default::default() }
    }

    fn pin_partition(sheet: &Sheet, defs: &[SymbolDef]) -> BTreeSet<BTreeSet<(EntityId, String)>> {
        extract_netlist(sheet, defs)
            .iter()
            .filter(|n| !n.pins.is_empty())
            .map(|n| n.pins.iter().map(|q| (q.entity_id, q.pin.clone())).collect())
            .collect()
    }

    fn apply(sheet: &Sheet, plan: &ArrangePlan) -> Sheet {
        let mut s = sheet.clone();
        for id in &plan.remove {
            s.entities.remove(id);
        }
        for (ids, dx, dy) in &plan.moves {
            for id in ids {
                if let Some(e) = s.entities.get_mut(id) {
                    e.translate(*dx, *dy);
                }
            }
        }
        for e in &plan.updates {
            s.entities.insert(e.id(), e.clone());
        }
        for e in &plan.add {
            s.entities.insert(e.id(), e.clone());
        }
        s
    }

    fn at_of(sheet: &Sheet, id: EntityId) -> Point {
        match &sheet.entities[&id] {
            Entity::Symbol(s) => s.at,
            Entity::NetLabel(l) => l.at,
            Entity::Text(t) => t.at,
            other => panic!("位置を持たない: {other:?}"),
        }
    }

    fn bounds_of(sheet: &Sheet, defs: &[SymbolDef], main: EntityId) -> (f64, f64, f64, f64) {
        units(sheet, defs).into_iter().find(|u| u.main == main).expect("unit").bounds
    }

    fn arrange(sheet: &Sheet) -> ArrangePlan {
        plan_arrange(sheet, &defs(), &ArrangeOptions::default(), &ctx()).expect("plan")
    }

    /// Two parts squeezed 3 mm apart are pushed to at least the channel width and stay wired the same way.
    /// 3mmしか離れていない2つの部品は通り道の幅以上に離され、つながり方は変わらない。
    #[test]
    fn squeezed_parts_get_a_channel_and_keep_their_wiring() {
        let mut sh = new_sheet();
        let a1 = place(&mut sh, "box2", (50.0, 50.0), "A1");
        let a2 = place(&mut sh, "box2", (73.0, 50.0), "A2");
        old_wire(&mut sh, &[(60.0, 50.0), (63.0, 50.0)]);
        let plan = arrange(&sh);
        let after = apply(&sh, &plan);
        let (b1, b2) = (bounds_of(&after, &defs(), a1), bounds_of(&after, &defs(), a2));
        assert!(b2.0 - b1.2 >= 8.0 - 1e-9, "gap {}", b2.0 - b1.2);
        assert_eq!(pin_partition(&sh, &defs()), pin_partition(&after, &defs()));
        assert!(!plan.routed.is_empty());
    }

    /// Parts keep their left-to-right and top-to-bottom order.
    /// 部品の左右・上下の並び順は変わらない。
    #[test]
    fn order_is_kept_in_both_directions() {
        let mut sh = new_sheet();
        let mut ids = Vec::new();
        for (r, y) in [50.0, 65.0, 100.0].iter().enumerate() {
            for (c, x) in [50.0, 73.0, 130.0].iter().enumerate() {
                ids.push(place(&mut sh, "box2", (*x, *y), &format!("A{}", r * 3 + c)));
            }
        }
        let plan = arrange(&sh);
        let after = apply(&sh, &plan);
        for &a in &ids {
            for &b in &ids {
                let (oa, ob) = (at_of(&sh, a), at_of(&sh, b));
                let (na, nb) = (at_of(&after, a), at_of(&after, b));
                if oa.x < ob.x {
                    assert!(na.x < nb.x);
                }
                if oa.y < ob.y {
                    assert!(na.y < nb.y);
                }
                if oa.x == ob.x {
                    assert_eq!(na.x, nb.x);
                }
                if oa.y == ob.y {
                    assert_eq!(na.y, nb.y);
                }
            }
        }
        assert_eq!(plan.columns, 3);
        assert_eq!(plan.rows, 3);
    }

    /// Parts in one column whose origins differ by 2.5 mm end on the same x.
    /// 同じ列で原点の差が2.5mmの部品は、同じxにそろう。
    #[test]
    fn nearly_aligned_parts_line_up() {
        let mut sh = new_sheet();
        let c1 = place(&mut sh, "cap", (100.0, 50.0), "C1");
        let c2 = place(&mut sh, "cap", (102.5, 120.0), "C2");
        let after = apply(&sh, &arrange(&sh));
        assert_eq!(at_of(&after, c1).x, at_of(&after, c2).x);
        assert_eq!(at_of(&after, c1).x, 100.0);
    }

    /// A power mark, a tag label and a nearby note move together with their part.
    /// 電源マーク・タグのラベル・近くの注記は、部品と一緒に動く。
    #[test]
    fn satellites_move_with_their_part() {
        let mut sh = new_sheet();
        place(&mut sh, "box2", (25.0, 50.0), "B1");
        let a1 = place(&mut sh, "box2", (50.0, 50.0), "A1");
        let m = place(&mut sh, "mark", (40.0, 60.0), "M1");
        old_wire(&mut sh, &[(40.0, 50.0), (40.0, 55.0)]);
        old_wire(&mut sh, &[(60.0, 50.0), (65.0, 50.0)]);
        let l = label(&mut sh, (65.0, 50.0), "TAG");
        let t = note(&mut sh, (50.0, 57.0));
        let us = units(&sh, &defs());
        let u = us.iter().find(|u| u.main == a1).expect("unit");
        assert_eq!(u.members.len(), 4, "A1, mark, label and note form one unit");
        let plan = arrange(&sh);
        let after = apply(&sh, &plan);
        let d = at_of(&after, a1).x - 50.0;
        assert!(d > 0.0, "the unit must move");
        for (id, old) in [(a1, (50.0, 50.0)), (m, (40.0, 60.0)), (l, (65.0, 50.0)), (t, (50.0, 57.0))] {
            let now = at_of(&after, id);
            assert_eq!((now.x - old.0, now.y - old.1), (d, 0.0));
        }
    }

    /// An enclosure rectangle around two parts stretches with the gap between them.
    /// 2つの部品を囲む枠は、部品の間の空きに合わせて伸びる。
    #[test]
    fn an_enclosure_stretches_with_its_contents() {
        let mut sh = new_sheet();
        let a1 = place(&mut sh, "box2", (50.0, 50.0), "A1");
        let a2 = place(&mut sh, "box2", (73.0, 50.0), "A2");
        old_wire(&mut sh, &[(60.0, 50.0), (63.0, 50.0)]);
        let sid = Uuid::new_v4();
        sh.entities.insert(
            sid,
            Entity::Shape(
                serde_json::from_value(serde_json::json!({
                    "id": sid,
                    "shape": "rect",
                    "points": [{"x": 35.0, "y": 40.0}, {"x": 88.0, "y": 60.0}],
                }))
                .expect("shape"),
            ),
        );
        let plan = arrange(&sh);
        assert!(plan.updates.iter().any(|e| e.id() == sid));
        let after = apply(&sh, &plan);
        let Entity::Shape(s) = &after.entities[&sid] else { panic!("shape") };
        let (x0, x1) = (s.points[0].x.min(s.points[1].x), s.points[0].x.max(s.points[1].x));
        let (y0, y1) = (s.points[0].y.min(s.points[1].y), s.points[0].y.max(s.points[1].y));
        for b in [bounds_of(&after, &defs(), a1), bounds_of(&after, &defs(), a2)] {
            assert!(x0 <= b.0 && b.2 <= x1 && y0 <= b.1 && b.3 <= y1, "{b:?} not inside {x0},{y0},{x1},{y1}");
        }
        assert!(x1 - x0 > 53.0, "the rectangle grew");
    }

    /// When the arranged drawing would not fit in the frame, nothing is planned and the reason gives the size.
    /// 並べた図面が図枠に収まらないときは計画を作らず、理由に大きさを書く。
    #[test]
    fn a_drawing_that_would_not_fit_is_refused() {
        let mut sh = new_sheet();
        for i in 0..10 {
            place(&mut sh, "box2", (25.0 + 21.0 * i as f64, 50.0), &format!("A{i}"));
        }
        let c = ArrangeContext { frame: (10.0, 10.0, 230.0, 120.0), title_block: (230.0, 240.0, 410.0, 287.0), ..Default::default() };
        let err = plan_arrange(&sh, &defs(), &ArrangeOptions::default(), &c).expect_err("must refuse");
        assert!(err.contains("図枠"), "{err}");
        assert!(err.contains("幅"), "{err}");
    }

    /// A box walled in on all sides, wired to a far box. Its pin stub is the only way out.
    fn walled_sheet() -> (Sheet, EntityId, EntityId) {
        let mut sh = new_sheet();
        let a1 = place(&mut sh, "box2", (50.0, 50.0), "A1");
        place(&mut sh, "wall_h", (50.0, 35.0), "");
        place(&mut sh, "wall_h", (50.0, 65.0), "");
        place(&mut sh, "wall_v", (17.5, 50.0), "");
        place(&mut sh, "wall_v", (82.5, 50.0), "");
        let a2 = place(&mut sh, "box2", (160.0, 50.0), "A2");
        old_wire(&mut sh, &[(60.0, 50.0), (150.0, 50.0)]);
        (sh, a1, a2)
    }

    /// A cluster that cannot be routed is connected by labels with a fresh name, and pins still share the same nets.
    /// 引けないかたまりは新しい名前のラベルでつながり、ピンのネットの分け方は変わらない。
    #[test]
    fn an_unroutable_cluster_is_joined_by_labels() {
        let (sh, _, _) = walled_sheet();
        let plan = arrange(&sh);
        assert_eq!(plan.labelled.len(), 1);
        assert_eq!(plan.labelled[0].1, "ARR_1");
        let after = apply(&sh, &plan);
        assert_eq!(pin_partition(&sh, &defs()), pin_partition(&after, &defs()));
        let labels = after.entities.values().filter(|e| matches!(e, Entity::NetLabel(l) if l.name == "ARR_1")).count();
        assert_eq!(labels, 2);
    }

    /// Fallback names skip names already used in the project, so unrelated nets never join.
    /// 代わりの名前はプロジェクトで使われている名前を避けるので、関係ないネットがつながることはない。
    #[test]
    fn fallback_names_never_join_unrelated_nets() {
        let (sh, _, _) = walled_sheet();
        let c = ArrangeContext { taken_names: vec!["ARR_1".into()], ..ctx() };
        let plan = plan_arrange(&sh, &defs(), &ArrangeOptions::default(), &c).expect("plan");
        assert_eq!(plan.labelled.len(), 1);
        assert_eq!(plan.labelled[0].1, "ARR_2");
        let after = apply(&sh, &plan);
        assert_eq!(pin_partition(&sh, &defs()), pin_partition(&after, &defs()));
    }

    /// A sheet with a cable is refused and nothing is planned.
    /// ケーブルのあるシートは断られ、計画は作られない。
    #[test]
    fn a_sheet_with_a_cable_is_refused() {
        let mut sh = new_sheet();
        place(&mut sh, "box2", (50.0, 50.0), "A1");
        let id = Uuid::new_v4();
        sh.entities.insert(
            id,
            Entity::Cable(Cable {
                id,
                reference: "W1".into(),
                part_no: "SAMPLE-CBL".into(),
                core_count: 2,
                sq: 1.25,
                points: vec![p(0.0, 0.0), p(10.0, 10.0)],
                cores: Vec::new(),
                shield: None,
                length_m: None,
                run: Vec::new(),
            }),
        );
        assert!(plan_arrange(&sh, &defs(), &ArrangeOptions::default(), &ctx()).is_err());
    }

    /// A wire stub with a free end on one part moves with that part unchanged in shape.
    /// 1つの部品から出て先が浮いている線は、形を変えずに部品と一緒に動く。
    #[test]
    fn a_free_stub_moves_with_its_part() {
        let mut sh = new_sheet();
        place(&mut sh, "box2", (28.0, 50.0), "B1");
        let a1 = place(&mut sh, "box2", (50.0, 50.0), "A1");
        let w = old_wire(&mut sh, &[(60.0, 50.0), (70.0, 50.0)]);
        let plan = arrange(&sh);
        let after = apply(&sh, &plan);
        let d = at_of(&after, a1).x - 50.0;
        assert!(d > 0.0, "A1 must move");
        let (Entity::Wire(old), Entity::Wire(new)) = (&sh.entities[&w], &after.entities[&w]) else { panic!("wire") };
        assert_eq!(old.points.len(), new.points.len());
        for (a, b) in old.points.iter().zip(&new.points) {
            assert_eq!((b.x - a.x, b.y - a.y), (d, 0.0));
        }
    }

    /// A sheet with only wires and notes changes nothing.
    /// 配線と注記だけのシートでは何も変わらない。
    #[test]
    fn a_sheet_without_symbols_changes_nothing() {
        let mut sh = new_sheet();
        old_wire(&mut sh, &[(50.0, 50.0), (80.0, 50.0)]);
        note(&mut sh, (60.0, 60.0));
        let plan = arrange(&sh);
        assert!(plan.moves.is_empty() && plan.updates.is_empty() && plan.remove.is_empty() && plan.add.is_empty());
    }

    /// Parts off the 2.5 mm grid move in 2.5 mm steps, keep their offset and stay connected.
    /// 2.5mmグリッドから外れた部品も2.5mm刻みで動き、ずれを保ったまま接続も保つ。
    #[test]
    fn off_grid_symbols_keep_their_offset_and_stay_connected() {
        let mut sh = new_sheet();
        let f = place(&mut sh, "freebox", (50.0, 50.0), "F1");
        let b = place(&mut sh, "box2", (70.0, 50.0), "B1");
        old_wire(&mut sh, &[(57.3, 50.0), (60.0, 50.0)]);
        let plan = arrange(&sh);
        let after = apply(&sh, &plan);
        let dx = at_of(&after, b).x - 70.0;
        assert!(dx > 0.0, "B1 must move");
        assert!(((dx / 2.5).round() * 2.5 - dx).abs() < 1e-9, "dx {dx}");
        assert_eq!(at_of(&after, f), at_of(&sh, f));
        assert_eq!(pin_partition(&sh, &defs()), pin_partition(&after, &defs()));
    }

    /// Arranging an already arranged sheet moves nothing more.
    /// 並べ直した後のシートをもう一度並べ直しても、それ以上は動かない。
    #[test]
    fn arranging_twice_does_not_move_more() {
        let mut sh = new_sheet();
        place(&mut sh, "box2", (50.0, 50.0), "A1");
        place(&mut sh, "box2", (73.0, 50.0), "A2");
        old_wire(&mut sh, &[(60.0, 50.0), (63.0, 50.0)]);
        let first = apply(&sh, &arrange(&sh));
        let second = arrange(&first);
        assert!(second.moves.is_empty(), "{:?}", second.moves);
        assert!(second.labelled.is_empty(), "{:?}", second.labelled);
        assert!(second.remove.is_empty() && second.add.is_empty(), "wires were redrawn: {:?}", second.remove);
    }

    // ------------------------------------------------------------------
    // 総合レビューの指摘 (C1・C2・I1・I4) の確かめ
    // ------------------------------------------------------------------

    /// ネットリストで、シンボルのピンが入るネットのラベル名。
    fn net_label_of(sheet: &Sheet, sym: EntityId, pin_no: &str) -> Option<String> {
        extract_netlist(sheet, &defs())
            .into_iter()
            .find(|n| n.pins.iter().any(|q| q.entity_id == sym && q.pin == pin_no))
            .and_then(|n| n.label)
    }

    /// シンボルのピンの今の位置。
    fn pin_at(sheet: &Sheet, sym: EntityId, pin_no: &str) -> Point {
        pin_points(sheet, &defs()).into_iter().find(|q| q.entity == sym && q.number == pin_no).expect("pin").pos
    }

    /// A net label sitting on a pin that also has a wire stays on that pin when the part moves, so the net keeps its name.
    /// 配線もつながるピンの上に置いたネットラベルは、部品が動いてもそのピンの上に残り、ネットの名前は変わらない。
    #[test]
    fn a_label_on_a_wired_pin_moves_with_the_part() {
        let mut sh = new_sheet();
        place(&mut sh, "box2", (25.0, 50.0), "B1");
        let a1 = place(&mut sh, "box2", (50.0, 50.0), "A1");
        place(&mut sh, "cap", (100.0, 90.0), "C2");
        old_wire(&mut sh, &[(60.0, 50.0), (60.0, 80.0), (100.0, 80.0)]);
        let l = label(&mut sh, (60.0, 50.0), "X");
        let plan = arrange(&sh);
        let after = apply(&sh, &plan);
        assert!(at_of(&after, a1).x > 50.0, "A1 must move");
        assert_eq!(at_of(&after, l), pin_at(&after, a1, "2"));
        assert_eq!(net_label_of(&after, a1, "2").as_deref(), Some("X"));
    }

    /// A ground mark placed directly on a pin, with no wire, moves with the part and keeps naming the net.
    /// 配線なしでピンの上に直接置いた接地の印は部品と一緒に動き、ネットの名前を保つ。
    #[test]
    fn a_ground_mark_on_a_pin_moves_with_the_part() {
        let mut sh = new_sheet();
        place(&mut sh, "box2", (25.0, 50.0), "B1");
        let a1 = place(&mut sh, "box2", (50.0, 50.0), "A1");
        let g = place(&mut sh, "gnd", (60.0, 55.0), "");
        let plan = arrange(&sh);
        let after = apply(&sh, &plan);
        assert!(at_of(&after, a1).x > 50.0, "A1 must move");
        assert_eq!(pin_at(&after, g, "1"), pin_at(&after, a1, "2"));
    }

    /// A net label on a pin with no wire at all moves with the part.
    /// 配線の無いピンの上のネットラベルは、部品と一緒に動く。
    #[test]
    fn a_label_on_a_bare_pin_moves_with_the_part() {
        let mut sh = new_sheet();
        place(&mut sh, "box2", (25.0, 50.0), "B1");
        let a1 = place(&mut sh, "box2", (50.0, 50.0), "A1");
        let l = label(&mut sh, (60.0, 50.0), "Y");
        let us = units(&sh, &defs());
        assert!(us.iter().any(|u| u.main == a1 && u.members.contains(&l)), "the label belongs to A1");
        let after = apply(&sh, &arrange(&sh));
        assert_eq!(at_of(&after, l), pin_at(&after, a1, "2"));
    }

    /// An unroutable cluster holding two labels with different names is refused, naming the net, and nothing is planned.
    /// 引けないかたまりが名前の違う2つのラベルを持つときは、ネット名を挙げて断り、計画は作らない。
    #[test]
    fn an_unroutable_cluster_with_two_label_names_is_refused() {
        let (mut sh, _, _) = walled_sheet();
        label(&mut sh, (100.0, 50.0), "P");
        label(&mut sh, (120.0, 50.0), "Q");
        let err = plan_arrange(&sh, &defs(), &ArrangeOptions::default(), &ctx()).expect_err("must refuse");
        assert!(err.contains("名前の違うラベル"), "{err}");
        assert!(err.contains('P') || err.contains('Q'), "{err}");
    }

    /// A routable cluster holding two labels with different names is refused as well, before any routing.
    /// 引けるかたまりでも、名前の違う2つのラベルを持てば、引き直す前に断る。
    #[test]
    fn a_routable_cluster_with_two_label_names_is_refused() {
        let mut sh = new_sheet();
        place(&mut sh, "box2", (50.0, 50.0), "A1");
        place(&mut sh, "box2", (100.0, 50.0), "A2");
        old_wire(&mut sh, &[(60.0, 50.0), (90.0, 50.0)]);
        label(&mut sh, (70.0, 50.0), "P");
        label(&mut sh, (80.0, 50.0), "Q");
        let err = plan_arrange(&sh, &defs(), &ArrangeOptions::default(), &ctx()).expect_err("must refuse");
        assert!(err.contains("名前の違うラベル"), "{err}");
    }

    /// Two parts that each have their own ground mark keep their marks as satellites, although both marks name the same net.
    /// 2つの部品がそれぞれ自分の接地の印を持つとき、同じネット名でも印はそれぞれの部品の衛星になる。
    #[test]
    fn ground_marks_of_two_parts_are_each_a_satellite() {
        let mut sh = new_sheet();
        let a1 = place(&mut sh, "box2", (50.0, 50.0), "A1");
        let g1 = place(&mut sh, "gnd", (40.0, 60.0), "");
        old_wire(&mut sh, &[(40.0, 50.0), (40.0, 55.0)]);
        let a2 = place(&mut sh, "box2", (100.0, 50.0), "A2");
        let g2 = place(&mut sh, "gnd", (90.0, 60.0), "");
        old_wire(&mut sh, &[(90.0, 50.0), (90.0, 55.0)]);
        let us = units(&sh, &defs());
        assert_eq!(us.len(), 2, "{us:?}");
        assert!(us.iter().any(|u| u.main == a1 && u.members.contains(&g1)));
        assert!(us.iter().any(|u| u.main == a2 && u.members.contains(&g2)));
    }

    /// Tag labels with the same name on two parts are each a satellite of their own part.
    /// 2つの部品に同じ名前のタグのラベルがあっても、それぞれ自分の部品の衛星になる。
    #[test]
    fn same_named_tags_on_two_parts_are_each_a_satellite() {
        let mut sh = new_sheet();
        let a1 = place(&mut sh, "box2", (50.0, 50.0), "A1");
        old_wire(&mut sh, &[(60.0, 50.0), (65.0, 50.0)]);
        let t1 = label(&mut sh, (65.0, 50.0), "TAG");
        let a2 = place(&mut sh, "box2", (100.0, 50.0), "A2");
        old_wire(&mut sh, &[(110.0, 50.0), (115.0, 50.0)]);
        let t2 = label(&mut sh, (115.0, 50.0), "TAG");
        let us = units(&sh, &defs());
        assert!(us.iter().any(|u| u.main == a1 && u.members.contains(&t1)));
        assert!(us.iter().any(|u| u.main == a2 && u.members.contains(&t2)));
    }

    /// A box closed in by walls whose spacing already fits, wired to a part beside it, so nothing has to move.
    fn walled_tidy_sheet() -> Sheet {
        let mut sh = new_sheet();
        place(&mut sh, "box2", (50.0, 50.0), "A1");
        place(&mut sh, "wall_h", (50.0, 30.0), "");
        place(&mut sh, "wall_h", (50.0, 70.0), "");
        place(&mut sh, "wall_v", (17.5, 50.0), "");
        place(&mut sh, "wall_v", (82.5, 50.0), "");
        place(&mut sh, "box2", (110.0, 50.0), "A2");
        old_wire(&mut sh, &[(60.0, 50.0), (100.0, 50.0)]);
        sh
    }

    /// A cluster whose ends do not move keeps its old wire untouched, even when the router could not draw it again.
    /// 端が動かないかたまりは、引き直せない場合でも元の配線をそのまま残す。
    #[test]
    fn an_unmoved_cluster_keeps_its_wire() {
        let sh = walled_tidy_sheet();
        let plan = arrange(&sh);
        assert!(plan.moves.is_empty(), "{:?}", plan.moves);
        assert!(plan.labelled.is_empty(), "{:?}", plan.labelled);
        assert!(plan.remove.is_empty() && plan.add.is_empty() && plan.updates.is_empty());
    }

    // ------------------------------------------------------------------
    // 検算 (I3) と表題欄 (I2)
    // ------------------------------------------------------------------

    /// Two parts joined by one straight wire.
    fn two_wired_parts() -> (Sheet, EntityId, EntityId) {
        let mut sh = new_sheet();
        let a1 = place(&mut sh, "box2", (50.0, 50.0), "A1");
        let a2 = place(&mut sh, "box2", (100.0, 50.0), "A2");
        old_wire(&mut sh, &[(60.0, 50.0), (90.0, 50.0)]);
        (sh, a1, a2)
    }

    /// The self-check refuses a result in which a net lost the name its label gave it, although the pins still share the net.
    /// 検算は、ピンのつながりが同じでも、ネットがラベルの名前を失った結果を断る。
    #[test]
    fn the_self_check_refuses_a_lost_net_name() {
        let (mut before, _, _) = two_wired_parts();
        let l = label(&mut before, (75.0, 50.0), "P");
        let mut after = before.clone();
        after.entities.remove(&l);
        let err = verify_after(&before, &after, &defs(), &BTreeSet::new(), &[]).expect_err("must refuse");
        assert!(err.contains("名前"), "{err}");
        assert!(verify_after(&before, &before, &defs(), &BTreeSet::new(), &[]).is_ok());
    }

    /// The self-check ignores only the fresh ARR names the plan made; any other new name on a net is refused.
    /// 検算が数えないのは計画が作った新しいARRの名前だけで、それ以外の名前がネットに増えれば断る。
    #[test]
    fn the_self_check_ignores_only_fresh_names() {
        let (before, _, _) = two_wired_parts();
        let mut after = before.clone();
        label(&mut after, (75.0, 50.0), "ARR_1");
        assert!(verify_after(&before, &after, &defs(), &BTreeSet::from(["ARR_1".to_string()]), &[]).is_ok());
        assert!(verify_after(&before, &after, &defs(), &BTreeSet::new(), &[]).is_err());
    }

    /// The self-check refuses a result in which a label that sat on a pin no longer does.
    /// 検算は、ピンの上にあったラベルがピンから離れた結果を断る。
    #[test]
    fn the_self_check_refuses_a_label_moved_off_its_pin() {
        let mut before = new_sheet();
        place(&mut before, "box2", (50.0, 50.0), "A1");
        let l = label(&mut before, (60.0, 50.0), "Y");
        let mut after = before.clone();
        after.entities.get_mut(&l).expect("label").translate(10.0, 0.0);
        let err = verify_after(&before, &after, &defs(), &BTreeSet::new(), &[]).expect_err("must refuse");
        assert!(err.contains("ピンの上"), "{err}");
    }

    /// A label stub that touches a wire of another net is refused; the same stub standing free passes.
    /// ラベルでつなぐ短い線が他のネットの配線に触れれば断り、何にも触れなければ通す。
    #[test]
    fn a_label_stub_touching_another_wire_is_refused() {
        let mut sh = new_sheet();
        place(&mut sh, "box2", (50.0, 50.0), "A1");
        let w = old_wire(&mut sh, &[(40.0, 50.0), (35.0, 50.0)]);
        let l = label(&mut sh, (35.0, 50.0), "ARR_1");
        let stubs = [Stub { wire: w, label: l, name: "ARR_1".into(), check: true }];
        assert!(check_stubs(&sh, &defs(), &stubs).is_ok());
        old_wire(&mut sh, &[(35.0, 45.0), (35.0, 55.0)]);
        let err = check_stubs(&sh, &defs(), &stubs).expect_err("must refuse");
        assert!(err.contains("ARR_1"), "{err}");
    }

    /// The title block area follows the template's corner: top-left puts it in the top-left corner of the frame, with its own width and height.
    /// 表題欄の範囲は図枠テンプレートの隅に従う。左上なら図枠の左上に、表題欄の幅と高さで置く。
    #[test]
    fn the_title_block_area_follows_the_template_corner() {
        let mut t = crate::frame_template::default_template().clone();
        let sh = new_sheet();
        let (pw, ph) = sh.paper_mm();
        let f = t.frame_rect(pw, ph);
        t.title_block.origin = crate::frame_template::Origin::TopLeft;
        let (frame, tb) = frame_areas(&t, &sh);
        assert_eq!(frame, f);
        assert_eq!(tb, (f.0, f.1, f.0 + t.title_block.w, f.1 + t.title_block.h));
        t.title_block.origin = crate::frame_template::Origin::BottomRight;
        let (_, tb) = frame_areas(&t, &sh);
        assert_eq!(tb, (f.2 - t.title_block.w, f.3 - t.title_block.h, f.2, f.3));
    }

    /// Revision rows drawn above the title block are part of the area to avoid.
    /// 表題欄の上に描く改訂欄の行も、避ける範囲に入る。
    #[test]
    fn the_title_block_area_includes_the_revision_rows() {
        let t = crate::frame_template::default_template().clone();
        let mut sh = new_sheet();
        let (_, plain) = frame_areas(&t, &sh);
        sh.revisions = vec![serde_json::from_value(serde_json::json!({"mark": "A", "date": "2026-01-01", "description": "first", "by": "K"})).expect("revision")];
        let (_, with_rev) = frame_areas(&t, &sh);
        assert_eq!(with_rev.1, plain.1 - 2.0 * t.revision_row_h());
        assert_eq!((with_rev.0, with_rev.2, with_rev.3), (plain.0, plain.2, plain.3));
    }

    // ------------------------------------------------------------------
    // 仕様の確かめ項目の追加 (I5)
    // ------------------------------------------------------------------

    /// 決定的な疑似乱数 (線形合同法)。
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            self.0 >> 33
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    /// A generated sheet of about 30 parts (some with off-grid pins), ground marks, power marks and labels.
    fn generated_sheet(seed: u64) -> Sheet {
        let mut r = Lcg(seed);
        let mut sh = new_sheet();
        let mut parts: Vec<EntityId> = Vec::new();
        for row in 0..5 {
            for col in 0..6 {
                let x = 40.0 + col as f64 * 45.0 + r.below(5) as f64 * 2.5;
                let y = 40.0 + row as f64 * 40.0 + r.below(5) as f64 * 2.5;
                let kind = ["box2", "box2", "cap", "freebox"][r.below(4) as usize];
                parts.push(place(&mut sh, kind, (x, y), &format!("P{row}{col}")));
            }
        }
        let pins = pin_points(&sh, &defs());
        let pins_of = |id: EntityId| -> Vec<&PinPt> { pins.iter().filter(|q| q.entity == id).collect() };
        // 隣り合う部品のピンどうしをL字の配線でつなぐ
        let mut corners: Vec<(Point, (EntityId, String))> = Vec::new();
        for _ in 0..14 {
            let k = r.below(parts.len() as u64 - 1) as usize;
            let (pa, pb) = (pins_of(parts[k]), pins_of(parts[k + 1]));
            let a = pa[r.below(pa.len() as u64) as usize];
            let b = pb[r.below(pb.len() as u64) as usize];
            let corner = (b.pos.x, a.pos.y);
            old_wire(&mut sh, &[(a.pos.x, a.pos.y), corner, (b.pos.x, b.pos.y)]);
            corners.push((p(corner.0, corner.1), (a.entity, a.number.clone())));
        }
        // 名前は物理的なまとまりごとに1つ (名前の違うラベルで断られないように)
        let groups = phys_groups(&sh, &defs());
        let group_of = |pin: &(EntityId, String)| groups.iter().position(|g| g.pins.contains(pin)).expect("group");
        let mut named: BTreeMap<usize, String> = BTreeMap::new();
        for (at, pin) in &corners {
            if r.below(4) == 0 {
                let g = group_of(pin);
                let name = named.entry(g).or_insert_with(|| format!("SIG_{g}")).clone();
                label(&mut sh, (at.x, at.y), &name);
            }
        }
        // 接地の印・電源の印・タグのラベル
        for _ in 0..8 {
            let q = &pins[r.below(pins.len() as u64) as usize];
            let g = group_of(&(q.entity, q.number.clone()));
            match r.below(4) {
                0 | 1 => {
                    if named.get(&g).is_some_and(|n| n != "GND") {
                        continue;
                    }
                    named.insert(g, "GND".into());
                    if r.below(2) == 0 {
                        place(&mut sh, "gnd", (q.pos.x, q.pos.y + 5.0), "");
                    } else {
                        old_wire(&mut sh, &[(q.pos.x, q.pos.y), (q.pos.x, q.pos.y + 5.0)]);
                        place(&mut sh, "gnd", (q.pos.x, q.pos.y + 10.0), "");
                    }
                }
                2 => {
                    place(&mut sh, "mark", (q.pos.x, q.pos.y + 5.0), "");
                }
                _ => {
                    let name = named.entry(g).or_insert_with(|| format!("TAG_{g}")).clone();
                    label(&mut sh, (q.pos.x, q.pos.y), &name);
                }
            }
        }
        sh
    }

    /// On generated sheets with off-grid pins, marks and labels, every arrange either keeps which pins share a net and the names of every net, or refuses.
    /// ずれたピン・印・ラベルのある生成した図面では、並べ直しはピンのつながりと全てのネットの名前を保つか、断るかのどちらかになる。
    #[test]
    fn generated_sheets_keep_connections_and_names_or_refuse() {
        let mut planned = 0;
        for seed in 1..=16u64 {
            let sh = generated_sheet(seed);
            let plan = match plan_arrange(&sh, &defs(), &ArrangeOptions::default(), &ctx()) { Ok(p) => p, Err(e) => { eprintln!("seed {seed}: {e}"); continue } };
            planned += 1;
            let after = apply(&sh, &plan);
            let fresh: BTreeSet<String> = plan.labelled.iter().map(|l| l.1.clone()).filter(|n| n.starts_with("ARR_")).collect();
            assert_eq!(kept_partition(&sh, &after), pin_partition(&after, &defs()), "seed {seed}");
            let names = |s: &Sheet| -> BTreeSet<String> { name_key(s, &defs(), &fresh).into_iter().flat_map(|k| k.1).collect() };
            assert_eq!(names(&sh), names(&after), "seed {seed}");
        }
        assert!(planned >= 4, "only {planned} of 16 generated sheets were arranged");
    }

    /// The pin partition before arrange, leaving out pins of symbols the plan deleted (marks replaced by labels).
    fn kept_partition(before: &Sheet, after: &Sheet) -> BTreeSet<BTreeSet<(EntityId, String)>> {
        pin_partition(before, &defs())
            .into_iter()
            .map(|g| g.into_iter().filter(|p| after.entities.contains_key(&p.0)).collect::<BTreeSet<_>>())
            .filter(|g| !g.is_empty())
            .collect()
    }

    /// A walled box wired to a far box, placed at a vertical offset so that two such sheets can be combined.
    fn add_walled(sh: &mut Sheet, dy: f64) {
        place(sh, "box2", (50.0, 50.0 + dy), "");
        place(sh, "wall_h", (50.0, 35.0 + dy), "");
        place(sh, "wall_h", (50.0, 65.0 + dy), "");
        place(sh, "wall_v", (17.5, 50.0 + dy), "");
        place(sh, "wall_v", (82.5, 50.0 + dy), "");
        place(sh, "box2", (160.0, 50.0 + dy), "");
        old_wire(sh, &[(60.0, 50.0 + dy), (150.0, 50.0 + dy)]);
    }

    /// Two unrelated unroutable clusters get two different fresh names, so the labels never join them into one net.
    /// 関係のない2つの引けないかたまりは別々の新しい名前になり、ラベルで1つのネットにつながることはない。
    #[test]
    fn two_unroutable_clusters_get_different_names() {
        let mut sh = new_sheet();
        add_walled(&mut sh, 0.0);
        add_walled(&mut sh, 80.0);
        let plan = arrange(&sh);
        let names: BTreeSet<&String> = plan.labelled.iter().map(|l| &l.1).collect();
        assert_eq!(plan.labelled.len(), 2, "{:?}", plan.labelled);
        assert_eq!(names.len(), 2, "{:?}", plan.labelled);
        let after = apply(&sh, &plan);
        assert_eq!(pin_partition(&sh, &defs()), pin_partition(&after, &defs()));
    }

    /// A cluster that is joined by labels takes the name of its own label, and the old label is replaced.
    /// ラベルでつなぐかたまりは自分のラベルの名前を使い、古いラベルは置き換わる。
    #[test]
    fn the_fallback_uses_the_cluster_label_name() {
        let (mut sh, _, _) = walled_sheet();
        let old = label(&mut sh, (120.0, 50.0), "FEED");
        let plan = arrange(&sh);
        assert_eq!(plan.labelled.len(), 1);
        assert_eq!(plan.labelled[0].1, "FEED");
        assert!(plan.remove.contains(&old));
        let after = apply(&sh, &plan);
        let feeds = after.entities.values().filter(|e| matches!(e, Entity::NetLabel(l) if l.name == "FEED")).count();
        assert_eq!(feeds, 2);
        assert_eq!(pin_partition(&sh, &defs()), pin_partition(&after, &defs()));
    }

    /// A cluster that is joined by labels and holds a ground mark takes the mark's net name, and the mark stays on the drawing.
    /// ラベルでつなぐかたまりに接地の印があれば、その印のネット名を使い、印は図面に残る。
    #[test]
    fn the_fallback_uses_a_net_name_symbol() {
        let (mut sh, _, _) = walled_sheet();
        place(&mut sh, "gnd", (120.0, 55.0), "");
        let plan = arrange(&sh);
        assert_eq!(plan.labelled.len(), 1, "{:?}", plan.labelled);
        assert_eq!(plan.labelled[0].1, "GND");
        let after = apply(&sh, &plan);
        assert_eq!(kept_partition(&sh, &after), pin_partition(&after, &defs()));
        let marks = after.entities.values().filter(|e| matches!(e, Entity::Symbol(s) if s.symbol_id == "gnd")).count();
        let labels = after.entities.values().filter(|e| matches!(e, Entity::NetLabel(l) if l.name == "GND")).count();
        assert_eq!(marks, 1, "the ground mark is kept");
        assert!(labels >= 2, "the pins are joined by GND labels");
    }

    /// An alignment move that would make two units overlap where they did not before is skipped.
    /// 整列の移動で、今まで重なっていなかった2つの単位が重なるなら、その移動はしない。
    #[test]
    fn an_alignment_that_would_overlap_is_skipped() {
        // 単位1を単位0の x にそろえると、単位2に重なる
        let mut cur = vec![(0.0, 0.0, 10.0, 10.0), (2.5, 20.0, 12.5, 30.0), (-5.0, 20.0, 1.0, 30.0)];
        let anchors = [5.0, 7.5, -2.0];
        assert!(creates_overlap(&cur, &[1], &[shifted(cur[1], Axis::X, -2.5)]));
        let (total, _, _) = axis_pass(&mut cur, &anchors, Axis::X, &ArrangeOptions::default(), 20.0);
        assert_eq!(total, vec![0.0, 0.0, 0.0]);
    }

    /// When the layout with the normal maximum gap does not fit, the gaps are reduced towards the channel width and the arrange succeeds.
    /// 通常の空きの上限では図枠に収まらないときは、空きを通り道の幅まで詰めて並べ直す。
    #[test]
    fn a_tight_frame_reduces_the_gaps_and_fits() {
        let mut sh = new_sheet();
        let ids: Vec<EntityId> = (0..5).map(|i| place(&mut sh, "box2", (25.0 + 35.0 * i as f64, 50.0), &format!("A{i}"))).collect();
        let c = ArrangeContext { frame: (10.0, 10.0, 160.0, 120.0), title_block: (300.0, 300.0, 310.0, 310.0), ..Default::default() };
        let plan = plan_arrange(&sh, &defs(), &ArrangeOptions::default(), &c).expect("fits after reducing the gaps");
        let after = apply(&sh, &plan);
        let (b0, b1) = (bounds_of(&after, &defs(), ids[0]), bounds_of(&after, &defs(), ids[1]));
        let gap = b1.0 - b0.2;
        assert!(gap >= 8.0 - 1e-9 && gap < 15.0, "gap {gap}");
    }

    /// A layout that would run into the title block is refused when the drawing kept out of it before.
    /// 元の図面が表題欄を避けていたのに、並べると表題欄に掛かるときは断る。
    #[test]
    fn a_layout_into_the_title_block_is_refused() {
        let mut sh = new_sheet();
        place(&mut sh, "box2", (50.0, 50.0), "A1");
        place(&mut sh, "box2", (73.0, 50.0), "A2");
        let c = ArrangeContext { frame: (10.0, 10.0, 410.0, 287.0), title_block: (84.0, 10.0, 410.0, 287.0), ..Default::default() };
        let err = plan_arrange(&sh, &defs(), &ArrangeOptions::default(), &c).expect_err("must refuse");
        assert!(err.contains("図枠"), "{err}");
    }

    /// A wire that a note points at, or that is linked to FreeCAD, makes the arrange refuse and name the net.
    /// 注記が指す配線や、FreeCADと対応付けた配線があれば、並べ直しはネット名を挙げて断る。
    #[test]
    fn pointed_wires_are_refused() {
        let (mut sh, _, _) = two_wired_parts();
        let w = *sh.entities.keys().find(|id| matches!(sh.entities[*id], Entity::Wire(_))).expect("wire");
        let c = ArrangeContext { protected: vec![w], ..ctx() };
        let err = plan_arrange(&sh, &defs(), &ArrangeOptions::default(), &c).expect_err("must refuse");
        assert!(err.contains("動かせない配線"), "{err}");
        let t = note(&mut sh, (75.0, 60.0));
        if let Some(Entity::Text(text)) = sh.entities.get_mut(&t) {
            text.leader_to = Some(w);
        }
        let err = plan_arrange(&sh, &defs(), &ArrangeOptions::default(), &ctx()).expect_err("must refuse");
        assert!(err.contains("動かせない配線"), "{err}");
    }

    /// Wires of one cluster whose lengths come from different sources make the arrange refuse.
    /// 1つのかたまりの配線の長さの出所がまちまちなら、並べ直しは断る。
    #[test]
    fn mixed_length_sources_are_refused() {
        let mut sh = new_sheet();
        place(&mut sh, "box2", (50.0, 50.0), "A1");
        place(&mut sh, "box2", (100.0, 50.0), "A2");
        let w1 = old_wire(&mut sh, &[(60.0, 50.0), (75.0, 50.0)]);
        let w2 = old_wire(&mut sh, &[(75.0, 50.0), (90.0, 50.0)]);
        for (id, src) in [(w1, LengthSource::Manual), (w2, LengthSource::Freecad)] {
            if let Some(Entity::Wire(w)) = sh.entities.get_mut(&id) {
                w.length_m = Some(1.0);
                w.length_source = src;
            }
        }
        let err = plan_arrange(&sh, &defs(), &ArrangeOptions::default(), &ctx()).expect_err("must refuse");
        assert!(err.contains("動かせない配線"), "{err}");
    }

    /// Alignment groups never span more than the alignment distance, so a chain of nearby keys is split instead of moving far.
    /// 整列の組は整列の距離より広がらないので、近い座標が鎖のように続いても遠くへは動かさず、組を分ける。
    #[test]
    fn alignment_groups_do_not_chain() {
        let m = align_moves(&[40.0, 43.0, 46.0, 49.0], 4.0);
        assert_eq!(m, vec![0.0, -2.5, 0.0, -2.5]);
    }

    /// An alignment whose 2.5 mm rounding would swap the order of two parts is not done.
    /// 2.5mmへの丸めで2つの部品の順が入れ替わる整列はしない。
    #[test]
    fn alignment_never_swaps_order() {
        let keys = [49.0, 51.0, 52.0];
        let m = align_moves(&keys, 4.0);
        let new: Vec<f64> = keys.iter().zip(&m).map(|(k, d)| k + d).collect();
        assert!(new.windows(2).all(|w| w[0] <= w[1]), "{new:?}");
    }

    /// New labels made for a cluster copy the rotation and attributes of the label they replace.
    /// かたまりのために作る新しいラベルは、置き換えるラベルの向きと属性を引き継ぐ。
    #[test]
    fn new_labels_copy_the_old_label_style() {
        let (mut sh, _, _) = walled_sheet();
        let old = label(&mut sh, (120.0, 50.0), "FEED");
        if let Some(Entity::NetLabel(l)) = sh.entities.get_mut(&old) {
            l.rotation = 90;
            l.attrs.insert("note".into(), "kept".into());
        }
        let plan = arrange(&sh);
        let new: Vec<&NetLabel> = plan.add.iter().filter_map(|e| if let Entity::NetLabel(l) = e { Some(l) } else { None }).collect();
        assert_eq!(new.len(), 2);
        for l in new {
            assert_eq!(l.rotation, 90);
            assert_eq!(l.attrs.get("note").map(String::as_str), Some("kept"));
        }
    }

    /// A label that a note points at is not deleted when its cluster is joined by labels, and the net keeps its name.
    /// 注記が指すラベルは、かたまりをラベルでつなぐときも消さず、ネットの名前も変わらない。
    #[test]
    fn a_label_pointed_at_by_a_note_is_kept() {
        let (mut sh, _, _) = walled_sheet();
        let old = label(&mut sh, (120.0, 50.0), "FEED");
        let t = note(&mut sh, (120.0, 60.0));
        if let Some(Entity::Text(text)) = sh.entities.get_mut(&t) {
            text.leader_to = Some(old);
        }
        let plan = arrange(&sh);
        assert_eq!(plan.labelled.len(), 1);
        assert!(!plan.remove.contains(&old));
        let after = apply(&sh, &plan);
        assert!(after.entities.contains_key(&old));
        assert_eq!(pin_partition(&sh, &defs()), pin_partition(&after, &defs()));
    }

    // ------------------------------------------------------------------
    // 引けない配線の伸び縮み・接続点の円・ラベル名・衛星・重なり・短い線・検算の文言
    // ------------------------------------------------------------------

    /// 4 点の折れ線の配線 (60,50)-(75,50)-(75,80)-(90,80) が1本ある図面と、その端子の (旧, 新, 向き)。
    fn bent_wire_sheet() -> (Sheet, Vec<EntityId>) {
        let mut sh = new_sheet();
        let w = old_wire(&mut sh, &[(60.0, 50.0), (75.0, 50.0), (75.0, 80.0), (90.0, 80.0)]);
        (sh, vec![w])
    }

    /// x だけを写す配置 (70 より右を 10mm 右へずらす)。
    fn shift_right_layout() -> Layout {
        let bs = bands(&[(0.0, 70.0), (80.0, 200.0)]);
        Layout {
            dx: vec![],
            dy: vec![],
            wx: Warp::new(&bs, &[0.0, 10.0]),
            wy: Warp::new(&[], &[]),
            columns: 2,
            rows: 1,
        }
    }

    fn identity_layout() -> Layout {
        Layout { dx: vec![], dy: vec![], wx: Warp::new(&[], &[]), wy: Warp::new(&[], &[]), columns: 1, rows: 1 }
    }

    fn wire_points(es: &[Entity]) -> Vec<Vec<(f64, f64)>> {
        es.iter()
            .filter_map(|e| if let Entity::Wire(w) = e { Some(w.points.iter().map(|q| (q.x, q.y)).collect()) } else { None })
            .collect()
    }

    /// Stretching a wire maps every point through the column warp, so the bends move with their column.
    /// 配線を伸び縮みさせると、全ての点が列の写像で写り、曲がり角も列と一緒に動く。
    #[test]
    fn stretching_a_wire_maps_every_point_through_the_warp() {
        let (sh, ws) = bent_wire_sheet();
        let trip = [(p(60.0, 50.0), p(60.0, 50.0), None), (p(90.0, 80.0), p(100.0, 80.0), None)];
        let (es, _) = stretch_wires(&sh, &ws, &[], &shift_right_layout(), &trip, &[], &[], &[], &BTreeSet::new()).expect("stretch");
        assert_eq!(wire_points(&es), vec![vec![(60.0, 50.0), (80.0, 50.0), (80.0, 80.0), (100.0, 80.0)]]);
    }

    /// When a terminal moves sideways, the end goes to the terminal and the next point follows so the wire keeps its right angles.
    /// 端子が横へずれると、端は端子の新しい位置へ行き、隣の点も一緒にずれて直角が保たれる。
    #[test]
    fn stretching_moves_the_neighbour_point_to_keep_right_angles() {
        let (sh, ws) = bent_wire_sheet();
        let trip = [(p(60.0, 50.0), p(60.0, 50.0), None), (p(90.0, 80.0), p(100.0, 85.0), None)];
        let (es, _) = stretch_wires(&sh, &ws, &[], &shift_right_layout(), &trip, &[], &[], &[], &BTreeSet::new()).expect("stretch");
        assert_eq!(wire_points(&es), vec![vec![(60.0, 50.0), (80.0, 50.0), (80.0, 85.0), (100.0, 85.0)]]);
    }

    /// A straight two-point wire whose end would turn diagonal gets a jog in the middle.
    /// 2点だけの直線の配線は、端が動いて斜めになりそうなとき、中ほどで折れる。
    #[test]
    fn a_two_point_wire_that_would_turn_diagonal_gets_a_jog() {
        let mut sh = new_sheet();
        let w = old_wire(&mut sh, &[(60.0, 50.0), (90.0, 50.0)]);
        let trip = [(p(60.0, 50.0), p(60.0, 50.0), None), (p(90.0, 50.0), p(100.0, 60.0), None)];
        let (es, _) = stretch_wires(&sh, &[w], &[], &identity_layout(), &trip, &[], &[], &[], &BTreeSet::new()).expect("stretch");
        assert_eq!(wire_points(&es), vec![vec![(60.0, 50.0), (80.0, 50.0), (80.0, 60.0), (100.0, 60.0)]]);
    }

    /// A stretched wire that would cross another wire is refused, so the cluster falls back to labels.
    /// 伸び縮みさせた配線が他の配線と交わるなら断る (その場合はラベルでつなぐ)。
    #[test]
    fn stretching_is_refused_when_it_would_cross_another_wire() {
        let (sh, ws) = bent_wire_sheet();
        let trip = [(p(60.0, 50.0), p(60.0, 50.0), None), (p(90.0, 80.0), p(100.0, 80.0), None)];
        let other = vec![vec![p(70.0, 45.0), p(70.0, 55.0)]];
        assert!(stretch_wires(&sh, &ws, &[], &shift_right_layout(), &trip, &other, &[], &[], &BTreeSet::new()).is_none());
        let far = vec![vec![p(70.0, 90.0), p(70.0, 95.0)]];
        assert!(stretch_wires(&sh, &ws, &[], &shift_right_layout(), &trip, &far, &[], &[], &BTreeSet::new()).is_some());
    }

    /// A stretched wire that would run over a pin or label of someone else is refused.
    /// 伸び縮みさせた配線が他のピンやラベルの上に乗るなら断る。
    #[test]
    fn stretching_is_refused_when_it_would_touch_a_pin_or_label() {
        let (sh, ws) = bent_wire_sheet();
        let trip = [(p(60.0, 50.0), p(60.0, 50.0), None), (p(90.0, 80.0), p(100.0, 80.0), None)];
        let spots = vec![p(80.0, 65.0)];
        assert!(stretch_wires(&sh, &ws, &[], &shift_right_layout(), &trip, &[], &spots, &[], &BTreeSet::new()).is_none());
    }

    /// A stretched wire may not cross the outline of another part, but may pass its own terminal parts.
    /// 伸び縮みさせた配線は、他の部品の外形を横切れないが、自分の端子の部品の中は通ってよい。
    #[test]
    fn stretching_does_not_cross_other_symbols_except_its_own() {
        let mut sh = new_sheet();
        let w = old_wire(&mut sh, &[(60.0, 50.0), (90.0, 50.0)]);
        let trip = [(p(60.0, 50.0), p(60.0, 50.0), None), (p(90.0, 50.0), p(90.0, 50.0), None)];
        let other = Uuid::new_v4();
        let bodies = vec![(other, (70.0, 40.0, 80.0, 60.0))];
        assert!(stretch_wires(&sh, &[w], &[], &identity_layout(), &trip, &[], &[], &bodies, &BTreeSet::new()).is_none());
        let own = BTreeSet::from([other]);
        assert!(stretch_wires(&sh, &[w], &[], &identity_layout(), &trip, &[], &[], &bodies, &own).is_some());
    }

    /// Two boxes joined through a one-point-wide gap in a wall: the router cannot pass, but the old straight wire can.
    fn slit_sheet() -> (Sheet, Vec<SymbolDef>) {
        let mut ds = defs();
        ds.push(def("slit_t", vec![rect(-2.5, -20.0, 2.5, 0.0)], vec![]));
        ds.push(def("slit_b", vec![rect(-2.5, 0.0, 2.5, 20.0)], vec![]));
        let mut sh = new_sheet();
        place(&mut sh, "box2", (50.0, 50.0), "A1");
        place(&mut sh, "wall_h", (50.0, 35.0), "");
        place(&mut sh, "wall_h", (50.0, 65.0), "");
        place(&mut sh, "wall_v", (17.5, 50.0), "");
        place(&mut sh, "slit_t", (82.5, 50.0), "");
        place(&mut sh, "slit_b", (82.5, 50.0), "");
        place(&mut sh, "box2", (160.0, 50.0), "A2");
        old_wire(&mut sh, &[(60.0, 50.0), (150.0, 50.0)]);
        (sh, ds)
    }

    /// A cluster the router cannot draw keeps its old wire shape (stretched) and is listed in `kept_shape`, not joined by labels.
    /// 引けないかたまりは元の配線の形のまま伸び縮みし、`kept_shape` に載る (ラベルでつながない)。
    #[test]
    fn an_unroutable_cluster_keeps_its_wire_shape() {
        let (sh, ds) = slit_sheet();
        let plan = plan_arrange(&sh, &ds, &ArrangeOptions::default(), &ctx()).expect("plan");
        assert_eq!(plan.kept_shape, vec!["N001".to_string()]);
        assert!(plan.labelled.is_empty() && plan.routed.is_empty(), "{:?} {:?}", plan.labelled, plan.routed);
        let after = apply(&sh, &plan);
        assert_eq!(pin_partition(&sh, &ds), pin_partition(&after, &ds));
        let ws: Vec<&Wire> = after.entities.values().filter_map(|e| if let Entity::Wire(w) = e { Some(w) } else { None }).collect();
        assert_eq!(ws.len(), 1);
        assert!(ws[0].points.iter().all(|q| (q.y - 50.0).abs() < 1e-9), "still straight: {:?}", ws[0].points);
    }

    /// A circle of radius 1 mm or less is a connection dot; bigger circles and other shapes are not.
    /// 半径1mm以下の円は接続点として数え、大きな円や他の図形は数えない。
    #[test]
    fn only_small_circles_are_dot_shapes() {
        let mut sh = new_sheet();
        let small = dot(&mut sh, (10.0, 10.0), 0.5);
        let edge = dot(&mut sh, (20.0, 10.0), 1.0);
        dot(&mut sh, (30.0, 10.0), 1.5);
        let ids: Vec<EntityId> = dot_shapes(&sh).into_iter().map(|d| d.0).collect();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&small) && ids.contains(&edge));
    }

    /// A point counts as being on a wire anywhere along it, including its ends, and not beside it.
    /// 点は配線の途中や端にあれば配線の上にあり、わきにあれば上にない。
    #[test]
    fn on_any_wire_follows_the_wire() {
        let mut sh = new_sheet();
        old_wire(&mut sh, &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]);
        assert!(on_any_wire(&sh, p(5.0, 0.0)));
        assert!(on_any_wire(&sh, p(10.0, 10.0)));
        assert!(!on_any_wire(&sh, p(5.0, 2.0)));
    }

    /// 接続点の円 (半径 r) を置く。
    fn dot(sheet: &mut Sheet, c: (f64, f64), r: f64) -> EntityId {
        let id = Uuid::new_v4();
        sheet.entities.insert(
            id,
            Entity::Shape(Shape {
                id,
                shape: ShapeKind::Circle,
                points: vec![p(c.0, c.1), p(c.0 + r, c.1)],
                stroke: "default".into(),
                stroke_width_mm: 0.25,
                fill: None,
                dashed: false,
                arrow_end: false,
            }),
        );
        id
    }

    /// 縦にずれた2つの部品を、細い遠回りの配線で結んだ図面 (並べ直すと配線が引き直される) と、配線上の円の id。
    fn dotted_sheet(dot_at: (f64, f64)) -> (Sheet, EntityId) {
        let mut sh = new_sheet();
        place(&mut sh, "box2", (50.0, 50.0), "A1");
        place(&mut sh, "box2", (73.0, 80.0), "A2");
        old_wire(&mut sh, &[(60.0, 50.0), (61.5, 50.0), (61.5, 80.0), (63.0, 80.0)]);
        let d = dot(&mut sh, dot_at, 0.5);
        (sh, d)
    }

    /// A small circle that sat on a wire is removed when the redrawn wire no longer passes through it.
    /// 配線の上にあった小さな円は、引き直した配線がそこを通らなくなったら消える。
    #[test]
    fn a_dot_left_off_the_redrawn_wire_is_removed() {
        let (sh, d) = dotted_sheet((61.5, 65.0));
        let plan = arrange(&sh);
        assert!(!plan.routed.is_empty(), "{:?}", plan.routed);
        assert!(plan.remove.contains(&d), "the dot is removed");
        assert!(!plan.after.expect("after").entities.contains_key(&d));
    }

    /// A small circle that still lies on the wire after the plan stays.
    /// 計画の後もまだ配線の上にある小さな円は残る。
    #[test]
    fn a_dot_still_on_the_wire_stays() {
        let (sh, d) = dotted_sheet((60.0, 50.0));
        let plan = arrange(&sh);
        assert!(!plan.remove.contains(&d));
        assert!(plan.after.expect("after").entities.contains_key(&d));
    }

    /// A small circle that was never on a wire is left alone.
    /// もともと配線の上になかった小さな円には手を付けない。
    #[test]
    fn a_dot_that_was_not_on_a_wire_is_left_alone() {
        let (sh, d) = dotted_sheet((30.0, 20.0));
        let plan = arrange(&sh);
        assert!(!plan.remove.contains(&d));
        assert!(plan.after.expect("after").entities.contains_key(&d));
    }

    /// 名前が `name` のラベルの数。
    fn count_labels(sheet: &Sheet, name: &str) -> usize {
        sheet.entities.values().filter(|e| matches!(e, Entity::NetLabel(l) if l.name == name)).count()
    }

    /// A cluster with no label of its own, on a net that carries a label on another cluster, is joined by that label's name and never by a fresh ARR name.
    /// 自分のラベルが無くても、同じネットの別のかたまりにラベルがあれば、そのラベルの名前でつなぎ、新しいARRの名前は使わない。
    #[test]
    fn the_fallback_uses_the_net_wide_label() {
        let (mut sh, _, _) = walled_sheet();
        old_wire(&mut sh, &[(150.0, 50.0), (150.0, 30.0)]);
        label(&mut sh, (150.0, 30.0), "FEED");
        let plan = arrange(&sh);
        assert_eq!(plan.labelled.len(), 1, "{:?}", plan.labelled);
        assert_eq!(plan.labelled[0].1, "FEED");
        let after = apply(&sh, &plan);
        let arr = after.entities.values().filter(|e| matches!(e, Entity::NetLabel(l) if l.name.starts_with("ARR_"))).count();
        assert_eq!(arr, 0, "no ARR label is made");
        assert!(count_labels(&after, "FEED") >= 2, "both pins are joined by FEED labels");
        assert_eq!(pin_partition(&sh, &defs()), pin_partition(&after, &defs()));
    }

    /// A ground mark that is wired into a cluster joined by labels is never deleted, and it takes part under its own name.
    /// ラベルでつなぐかたまりにつながる接地の印は消されず、印の名前 (GND) でほかの端とつながる。
    #[test]
    fn a_ground_mark_in_a_label_joined_cluster_is_never_deleted() {
        let (mut sh, _, _) = walled_sheet();
        let m = place(&mut sh, "gnd", (60.0, 45.0), "");
        old_wire(&mut sh, &[(60.0, 40.0), (60.0, 50.0)]);
        let plan = arrange(&sh);
        assert_eq!(plan.labelled.len(), 1, "{:?}", plan.labelled);
        assert_eq!(plan.labelled[0].1, "GND");
        assert!(!plan.remove.contains(&m), "the mark is not deleted");
        let after = apply(&sh, &plan);
        assert!(after.entities.contains_key(&m));
        assert_eq!(kept_partition(&sh, &after), pin_partition(&after, &defs()));
    }

    /// A ground mark wired only to one part stays on the sheet together with its wire, and both move with the part.
    /// 1つの部品にだけ配線された接地の印は、配線ごと図面に残り、部品と一緒に動く。
    #[test]
    fn a_ground_mark_wired_to_a_part_stays_with_its_wire() {
        let mut sh = new_sheet();
        place(&mut sh, "box2", (50.0, 50.0), "A1");
        let a2 = place(&mut sh, "box2", (73.0, 50.0), "A2");
        let m = place(&mut sh, "gnd", (95.0, 60.0), "");
        let w = old_wire(&mut sh, &[(83.0, 50.0), (95.0, 50.0), (95.0, 55.0)]);
        let plan = arrange(&sh);
        let after = apply(&sh, &plan);
        assert!(after.entities.contains_key(&m) && after.entities.contains_key(&w));
        let dx = at_of(&after, a2).x - 73.0;
        assert!(dx > 0.0);
        assert_eq!(at_of(&after, m), p(95.0 + dx, 60.0));
        assert_eq!(pin_partition(&sh, &defs()), pin_partition(&after, &defs()));
    }

    /// A wire that joins a mark pin straight to a pin of the same unit is kept, and neither end gets a stub.
    /// 印のピンと同じ単位のピンを直接結ぶ配線は残し、両端には短い線を出さない。
    #[test]
    fn a_wire_between_a_mark_and_a_pin_of_its_unit_is_kept() {
        let mut sh = new_sheet();
        place(&mut sh, "box2", (50.0, 50.0), "A1");
        place(&mut sh, "gnd", (60.0, 45.0), "");
        let w = old_wire(&mut sh, &[(60.0, 40.0), (60.0, 50.0)]);
        let ds = defs();
        let us = units(&sh, &ds);
        assert_eq!(us.len(), 1, "the mark is a satellite of A1");
        let (idx, pins, owners) = (DefIndex::new(&ds), pin_points(&sh, &ds), Owners::new(&sh, &ds, &us));
        let terms = [(p(60.0, 40.0), Some(Dir::N)), (p(60.0, 50.0), Some(Dir::E))];
        let mut taken = BTreeSet::new();
        let fb = fallback_name(&sh, &idx, &pins, &owners, "N", &BTreeMap::new(), &[w], &terms, &BTreeSet::new(), &mut taken);
        assert_eq!(fb.name, "GND");
        assert!(!fb.fresh && fb.delete.is_empty());
        assert_eq!(fb.keep, vec![w]);
        assert!(fb.skip.contains(&p(60.0, 40.0)) && fb.skip.contains(&p(60.0, 50.0)));
    }

    /// The kept wire is copied with the movement of the unit that owns its ends.
    /// 残した配線は、端を持つ単位の移動量だけ動かした写しになる。
    #[test]
    fn a_kept_wire_is_copied_with_its_unit_movement() {
        let mut sh = new_sheet();
        place(&mut sh, "box2", (50.0, 50.0), "A1");
        place(&mut sh, "gnd", (60.0, 45.0), "");
        let w = old_wire(&mut sh, &[(60.0, 40.0), (60.0, 50.0)]);
        let ds = defs();
        let us = units(&sh, &ds);
        let owners = Owners::new(&sh, &ds, &us);
        let mut lay = identity_layout();
        lay.dx = vec![12.5];
        lay.dy = vec![-5.0];
        let es = kept_links(&sh, &owners, &lay, &[w]);
        assert_eq!(wire_points(&es), vec![vec![(72.5, 35.0), (72.5, 45.0)]]);
        assert_ne!(es[0].id(), w, "the copy has a new id");
    }

    /// Two boxes that already overlap (or sit within 0.5 mm) must not move relative to each other.
    /// もともと重なっている (0.5mm以内に近い) 2つの単位は、互いの位置を変えない。
    #[test]
    fn units_that_already_overlap_keep_their_relative_position() {
        let cur = vec![(0.0, 0.0, 10.0, 10.0), (8.0, 0.0, 18.0, 10.0)];
        // 片方だけ動かすと相対位置が変わる
        assert!(creates_overlap(&cur, &[0], &[shifted(cur[0], Axis::Y, 2.5)]));
        // 2つ一緒に同じだけ動かすのは構わない
        let both = [shifted(cur[0], Axis::Y, 2.5), shifted(cur[1], Axis::Y, 2.5)];
        assert!(!creates_overlap(&cur, &[0, 1], &both));
    }

    /// Bounds within 0.5 mm count as overlapping: moving to 0.4 mm apart is refused, 0.6 mm apart is allowed.
    /// 外接矩形が0.5mm以内に近づく移動は重なりとして断り、0.6mm離れるなら許す。
    #[test]
    fn closing_within_half_a_millimetre_counts_as_overlap() {
        let cur = vec![(0.0, 0.0, 10.0, 10.0), (20.0, 0.0, 30.0, 10.0)];
        let near_b = [(10.4, 0.0, 20.4, 10.0)];
        let far_b = [(10.6, 0.0, 20.6, 10.0)];
        assert!(creates_overlap(&cur, &[1], &near_b));
        assert!(!creates_overlap(&cur, &[1], &far_b));
        assert!(touch(&(0.0, 0.0, 10.0, 10.0), &(10.4, 0.0, 20.0, 10.0)));
        assert!(!touch(&(0.0, 0.0, 10.0, 10.0), &(10.6, 0.0, 20.0, 10.0)));
    }

    /// 向かい合うピンの手前に短い線を引くための道具: 空の図面の格子と、東向きの端子。
    fn stub_fixture() -> (RouteGrid, Fallback) {
        let sh = new_sheet();
        let grid = RouteGrid::build(&sh, &defs(), &ArrangeOptions::default().route);
        let w = old_wire_at(&sh, &[], p(0.0, 0.0));
        let fb = Fallback { net: "N".into(), name: "ARR_1".into(), terms: vec![(p(60.0, 50.0), Some(Dir::E), w)], template: None };
        (grid, fb)
    }

    fn stub_points(es: &[Entity]) -> Vec<(f64, f64)> {
        wire_points(es).remove(0)
    }

    /// With nothing in the way, the short wire goes straight out of the pin 2.5 mm and ends in a label with the chosen name.
    /// 何も邪魔が無いと、短い線はピンの向きへまっすぐ2.5mm出て、先に選んだ名前のラベルが付く。
    #[test]
    fn a_free_pin_gets_a_straight_stub_of_2_5_mm() {
        let (grid, fb) = stub_fixture();
        let (es, stubs) = stub_entities(&grid, &fb, &mut vec![], &mut vec![]);
        assert_eq!(stub_points(&es), vec![(60.0, 50.0), (62.5, 50.0)]);
        assert_eq!(count_named(&es, "ARR_1"), 1);
        assert_eq!(stubs.len(), 1);
        assert!(stubs[0].check);
    }

    fn count_named(es: &[Entity], name: &str) -> usize {
        es.iter().filter(|e| matches!(e, Entity::NetLabel(l) if l.name == name)).count()
    }

    /// When a wire stands in the way of every straight stub, the stub bends away and stays clear of it.
    /// 真っすぐな短い線がどれも他の線に当たるときは、曲げて、その線を避ける。
    #[test]
    fn a_blocked_pin_gets_a_bent_or_led_stub_that_stays_clear() {
        let (grid, fb) = stub_fixture();
        let lines = vec![vec![p(62.5, 40.0), p(62.5, 60.0)]];
        let (es, _) = stub_entities(&grid, &fb, &mut lines.clone(), &mut vec![]);
        let pts: Vec<Point> = stub_points(&es).into_iter().map(|(x, y)| p(x, y)).collect();
        assert_eq!(pts[0], p(60.0, 50.0));
        assert!(pts.len() >= 3 || pts.last().unwrap().x < 62.5, "not the straight stub: {pts:?}");
        assert!(!stub_hits(&pts, &lines, &[]), "{pts:?}");
    }

    /// When every candidate comes within the comfortable distance of something, the first one that still does not touch it is used.
    /// どの候補もゆとりの距離以内に何かが来るときは、それでも電気的に触れない最初の候補を使う。
    #[test]
    fn a_crowded_pin_takes_the_first_stub_that_does_not_touch() {
        let (grid, fb) = stub_fixture();
        // ピンのすぐわき (0.5mm) の点: どの候補も1mm以内だが、触れてはいない
        let mut spots = vec![p(60.0, 50.5)];
        let (es, _) = stub_entities(&grid, &fb, &mut vec![], &mut spots);
        assert_eq!(stub_points(&es), vec![(60.0, 50.0), (62.5, 50.0)]);
    }

    /// A plain label on a west-facing pin keeps its text off its own part: the stub grows until the text sits beside it.
    /// 西向きのピンの、タグなしのラベルは、文字が自分の部品に重ならない。文字が収まる長さまで短い線を伸ばす。
    #[test]
    fn a_plain_label_on_a_west_pin_keeps_its_text_off_its_own_part() {
        let mut sh = new_sheet();
        place(&mut sh, "box2", (60.0, 50.0), "R1");
        let grid = RouteGrid::build(&sh, &defs(), &ArrangeOptions::default().route);
        let w = old_wire_at(&sh, &[], p(0.0, 0.0));
        let fb = Fallback { net: "N".into(), name: "NET_A".into(), terms: vec![(p(50.0, 50.0), Some(Dir::W), w)], template: None };
        let part = crate::tidy::symbol_boxes(&sh, &defs()).remove(0).1;
        let text_of = |es: &[Entity]| -> crate::tidy::Rect {
            let l = es.iter().find_map(|e| if let Entity::NetLabel(l) = e { Some(l) } else { None }).expect("label");
            crate::tidy::net_label_text_rect(l, sh.annotation_scale())
        };
        // 文字を調べない選び方では、2.5 mm の線の先から右へ伸びる文字が部品に重なる
        let (plain, _) = stub_entities(&grid, &fb, &mut vec![], &mut vec![]);
        assert!(text_of(&plain).overlaps(&part), "before the fix the text sits on the part");
        let mut room = TextRoom::build(&sh, &defs(), &BTreeSet::new());
        let (es, _) = stub_entities_in(&grid, &fb, &mut vec![], &mut vec![], Some(&mut room));
        assert!(!text_of(&es).overlaps(&part), "{:?} vs {:?}", text_of(&es), part);
        assert_eq!(stub_points(&es)[0], (50.0, 50.0));
    }

    /// Fallback labels on two pins 2.5 mm apart do not print their texts on top of each other.
    /// 2.5 mm 離れた2つのピンのラベルは、文字が互いに重ならない。
    #[test]
    fn labels_on_neighbouring_pins_do_not_overlap_their_texts() {
        let mut sh = new_sheet();
        place(&mut sh, "box2", (60.0, 50.0), "R1");
        place(&mut sh, "box2", (60.0, 52.5), "R2");
        let grid = RouteGrid::build(&sh, &defs(), &ArrangeOptions::default().route);
        let w = old_wire_at(&sh, &[], p(0.0, 0.0));
        let fb = Fallback {
            net: "N".into(),
            name: "NET_A".into(),
            terms: vec![(p(50.0, 50.0), Some(Dir::W), w.clone()), (p(50.0, 52.5), Some(Dir::W), w)],
            template: None,
        };
        let mut room = TextRoom::build(&sh, &defs(), &BTreeSet::new());
        let (es, _) = stub_entities_in(&grid, &fb, &mut vec![], &mut vec![], Some(&mut room));
        let texts: Vec<crate::tidy::Rect> = es
            .iter()
            .filter_map(|e| if let Entity::NetLabel(l) = e { Some(crate::tidy::net_label_text_rect(l, sh.annotation_scale())) } else { None })
            .collect();
        assert_eq!(texts.len(), 2);
        assert!(!texts[0].overlaps(&texts[1]), "{texts:?}");
        // どちらも部品の外形に重ならない
        for part in crate::tidy::symbol_boxes(&sh, &defs()) {
            assert!(texts.iter().all(|t| !t.overlaps(&part.1)), "{texts:?} vs {:?}", part.1);
        }
    }

    /// A tagged template on a west-going stub gets the tag whose body points away from the pin.
    /// タグ付きの手本のラベルが西へ出る短い線に付くときは、旗の本体がピンと反対を向く側のタグになる。
    #[test]
    fn a_tagged_label_on_a_west_stub_gets_its_body_away_from_the_pin() {
        let mut sh = new_sheet();
        place(&mut sh, "box2", (60.0, 50.0), "R1");
        let grid = RouteGrid::build(&sh, &defs(), &ArrangeOptions::default().route);
        let w = old_wire_at(&sh, &[], p(0.0, 0.0));
        // 手本は本体が右側 ("left")。西向きでは本体を左 ("right") にしないと部品に被さる
        let mut attrs = std::collections::BTreeMap::new();
        attrs.insert(crate::net_tag::TAG_ATTR.to_string(), "left".to_string());
        let template = NetLabel { id: Uuid::new_v4(), at: p(0.0, 0.0), name: "NET_A".into(), rotation: 0, attrs };
        let fb = Fallback { net: "N".into(), name: "NET_A".into(), terms: vec![(p(50.0, 50.0), Some(Dir::W), w)], template: Some(template) };
        let mut room = TextRoom::build(&sh, &defs(), &BTreeSet::new());
        let (es, _) = stub_entities_in(&grid, &fb, &mut vec![], &mut vec![], Some(&mut room));
        let l = es.iter().find_map(|e| if let Entity::NetLabel(l) = e { Some(l) } else { None }).expect("label");
        assert_eq!(l.attrs.get(crate::net_tag::TAG_ATTR).map(String::as_str), Some("right"));
    }

    /// When no stub leaves room for the text, the stub is the one chosen without the text check, and it still starts at the pin.
    /// どの短い線にも文字の置き場が無いときは、文字を調べずに選ぶ短い線と同じものになり、ピンからつながる。
    #[test]
    fn a_boxed_in_pin_keeps_the_stub_chosen_without_the_text_check() {
        let (grid, fb) = stub_fixture();
        let lines = vec![vec![p(62.5, 40.0), p(62.5, 60.0)]];
        let (without, _) = stub_entities(&grid, &fb, &mut lines.clone(), &mut vec![]);
        let mut room = TextRoom {
            rects: vec![(None, crate::tidy::Rect::from_corners(p(-500.0, -500.0), p(500.0, 500.0)))],
            bodies: vec![],
            scale: 1.0,
            area: None,
            style: crate::model::XrefStyle::Zone,
            dests: BTreeMap::new(),
        };
        let (es, stubs) = stub_entities_in(&grid, &fb, &mut lines.clone(), &mut vec![], Some(&mut room));
        assert_eq!(stub_points(&es), stub_points(&without));
        assert_eq!(stub_points(&es)[0], (60.0, 50.0));
        assert_eq!(stubs.len(), 1);
        assert_eq!(room.rects.len(), 2, "the placed label text is added to the room");
    }

    /// A stub lengthened to give its label text room never runs through another part's outline to get there.
    /// 文字の置き場のために伸ばす短い線は、そのために他の部品の外形を突き抜けない。
    #[test]
    fn a_stub_lengthened_for_its_text_does_not_run_through_another_part() {
        let mut sh = new_sheet();
        place(&mut sh, "box2", (60.0, 50.0), "R1");
        // ピンの西に、ピンの無い細い壁 (x 42.5..47.5)
        place(&mut sh, "wall_v", (45.0, 50.0), "W1");
        let grid = RouteGrid::build(&sh, &defs(), &ArrangeOptions::default().route);
        let w = old_wire_at(&sh, &[], p(0.0, 0.0));
        let fb = Fallback { net: "N".into(), name: "NET_A".into(), terms: vec![(p(50.0, 50.0), Some(Dir::W), w)], template: None };
        let wall = crate::tidy::symbol_boxes(&sh, &defs()).into_iter().map(|b| b.1).find(|r| r.max.x <= 47.5 + EPS).expect("wall");
        let mut room = TextRoom::build(&sh, &defs(), &BTreeSet::new());
        let (es, _) = stub_entities_in(&grid, &fb, &mut vec![], &mut vec![], Some(&mut room));
        let pts: Vec<Point> = stub_points(&es).into_iter().map(|(x, y)| p(x, y)).collect();
        assert!(!pts.windows(2).any(|s| seg_in_rect(s[0], s[1], &wall)), "{pts:?} runs through {wall:?}");
    }

    /// A stub lengthened to give its label text room stays inside the drawing frame, so arranging is not refused for leaving it.
    /// 文字の置き場のために伸ばす短い線は図枠の内側にとどまり、図枠からはみ出して並べ直しが断られることはない。
    #[test]
    fn a_stub_lengthened_for_its_text_stays_inside_the_frame() {
        let mut sh = new_sheet();
        place(&mut sh, "box2", (60.0, 50.0), "R1");
        let grid = RouteGrid::build(&sh, &defs(), &ArrangeOptions::default().route);
        let w = old_wire_at(&sh, &[], p(0.0, 0.0));
        let fb = Fallback { net: "N".into(), name: "NET_A".into(), terms: vec![(p(50.0, 50.0), Some(Dir::W), w)], template: None };
        // 図枠の左端はピンの 5 mm 西
        let ctx = ArrangeContext { frame: (45.0, 10.0, 410.0, 287.0), title_block: (230.0, 240.0, 410.0, 287.0), ..Default::default() };
        let mut room = TextRoom::build(&sh, &defs(), &BTreeSet::new());
        room.area = Some(StubArea::new(&ctx, false, content_bounds(&sh, &DefIndex::new(&defs()))));
        let (es, _) = stub_entities_in(&grid, &fb, &mut vec![], &mut vec![], Some(&mut room));
        let pts = stub_points(&es);
        assert!(pts.iter().all(|q| q.0 >= 45.0 - EPS), "{pts:?} leaves the frame");
        // 置いた短い線は、次の候補のための中身の外接矩形に加わる
        let grown = room.area.as_ref().and_then(|a| a.content).expect("content");
        assert!(pts.iter().all(|q| q.0 >= grown.0 - EPS && q.0 <= grown.2 + EPS));
    }

    /// A stub placed first is avoided by the next one, because its wire and end are added to the lists the next stub avoids.
    /// 先に置いた短い線とその先は、次の短い線が避けるものに加わる。
    #[test]
    fn a_placed_stub_is_avoided_by_the_next() {
        let (grid, mut fb) = stub_fixture();
        let w = fb.terms[0].2.clone();
        fb.terms.push((p(65.0, 50.0), Some(Dir::W), w));
        let (mut lines, mut spots) = (vec![], vec![]);
        let (es, _) = stub_entities(&grid, &fb, &mut lines, &mut spots);
        assert_eq!(lines.len(), 2);
        assert_eq!(spots.len(), 2);
        let ws = wire_points(&es);
        assert_eq!(ws.len(), 2);
        let second: Vec<Point> = ws[1].iter().map(|&(x, y)| p(x, y)).collect();
        // 向かい合う2本は重ならない (先の線の先端からゆとりを取る)
        assert!(!path_hits(&second, &lines[..1], &spots[..1], &[second[0]]), "{second:?}");
    }

    /// A single-unit cluster (every terminal in one part) is detected as such.
    /// 端子が全て同じ単位にあるかたまりは、単位1つのかたまりと見なす。
    #[test]
    fn single_unit_detects_clusters_inside_one_unit() {
        assert!(single_unit(&[3, 3]));
        assert!(single_unit(&[1]));
        assert!(!single_unit(&[1, 2]));
        assert!(!single_unit(&[]));
    }

    /// A wire between a part and its own ground mark moves with the part in its old shape and is not redrawn.
    /// 部品とその部品の接地の印の間の配線は、元の形のまま部品と一緒に動き、引き直さない。
    #[test]
    fn a_wire_inside_one_unit_moves_rigidly_with_it() {
        let mut sh = new_sheet();
        place(&mut sh, "box2", (50.0, 50.0), "A1");
        let a2 = place(&mut sh, "box2", (73.0, 50.0), "A2");
        place(&mut sh, "gnd", (95.0, 60.0), "");
        let w = old_wire(&mut sh, &[(83.0, 50.0), (95.0, 50.0), (95.0, 55.0)]);
        let plan = arrange(&sh);
        let (ids, dx, dy) = plan.moves.iter().find(|m| m.0.contains(&a2)).expect("A2 moves").clone();
        assert!(dx > 0.0 && dy == 0.0);
        assert!(ids.contains(&w), "the wire moves with A2");
        assert!(!plan.remove.contains(&w) && !plan.routed.iter().any(|n| n == "N001"), "{:?}", plan.routed);
        let after = apply(&sh, &plan);
        let Entity::Wire(nw) = &after.entities[&w] else { panic!("wire") };
        let shape: Vec<(f64, f64)> = nw.points.iter().map(|q| (q.x - dx, q.y)).collect();
        assert_eq!(shape, vec![(83.0, 50.0), (95.0, 50.0), (95.0, 55.0)]);
    }

    /// The self-check names the pins whose sharing changed, as reference.pin, for what was before and what is after.
    /// 検算の断り文は、つながり方が変わったピンを「参照記号.ピン番号」で、前と後に分けて書く。
    #[test]
    fn the_self_check_names_the_pins_whose_net_changed() {
        let (before, _, _) = two_wired_parts();
        let mut after = before.clone();
        let w = after.entities.values().find_map(|e| if let Entity::Wire(w) = e { Some(w.id) } else { None }).expect("wire");
        after.entities.remove(&w);
        let err = verify_after(&before, &after, &defs(), &BTreeSet::new(), &[]).expect_err("must refuse");
        assert!(err.contains("接続が変わった"), "{err}");
        assert!(err.contains("前: ") && err.contains("後: "), "{err}");
        assert!(err.contains("A1.2") && err.contains("A2.1"), "{err}");
    }

    /// A pin of a part without a reference is named by the first characters of its id.
    /// 参照記号の無い部品のピンは、idの先頭の文字で書く。
    #[test]
    fn a_pin_without_a_reference_is_named_by_its_id() {
        let mut sh = new_sheet();
        let a = place(&mut sh, "box2", (50.0, 50.0), "");
        let one: BTreeSet<BTreeSet<(EntityId, String)>> = BTreeSet::from([BTreeSet::from([(a, "1".to_string())])]);
        let text = partition_diff(&sh, &one, &BTreeSet::new());
        assert!(text.contains(&format!("{}.1", &a.to_string()[..8])), "{text}");
    }

    /// The stub check names what the stub touched: another wire, a label, a junction or a pin of a part.
    /// 短い線の検査は、触れたものを書く: 他の配線・ラベル・ジャンクション・部品のピン。
    #[test]
    fn the_stub_check_names_a_touched_label_and_pin() {
        let mut sh = new_sheet();
        place(&mut sh, "box2", (50.0, 50.0), "A1");
        let w = old_wire(&mut sh, &[(40.0, 50.0), (35.0, 50.0)]);
        let l = label(&mut sh, (35.0, 50.0), "ARR_1");
        let stubs = [Stub { wire: w, label: l, name: "ARR_1".into(), check: true }];
        let other = label(&mut sh, (37.5, 50.0), "OTHER");
        let err = check_stubs(&sh, &defs(), &stubs).expect_err("label");
        assert!(err.contains("ラベル OTHER") && err.contains("(40.0, 50.0)"), "{err}");
        sh.entities.remove(&other);
        place(&mut sh, "box2", (25.0, 50.0), "A2");
        let err = check_stubs(&sh, &defs(), &stubs).expect_err("pin");
        assert!(err.contains("A2.2"), "{err}");
    }

    /// The stub check names a junction on the stub as "ジャンクション".
    /// 短い線の上にジャンクションがあれば、検査は「ジャンクション」と書く。
    #[test]
    fn the_stub_check_names_a_touched_junction() {
        let mut sh = new_sheet();
        let w = old_wire(&mut sh, &[(40.0, 50.0), (35.0, 50.0)]);
        let l = label(&mut sh, (35.0, 50.0), "ARR_1");
        let j = Uuid::new_v4();
        sh.entities.insert(j, Entity::Junction(Junction { id: j, at: p(37.5, 50.0) }));
        let stubs = [Stub { wire: w, label: l, name: "ARR_1".into(), check: true }];
        let err = check_stubs(&sh, &defs(), &stubs).expect_err("junction");
        assert!(err.contains("ジャンクション"), "{err}");
    }

    /// When a label leaves the pin it sat on and lands on another pin, the message names the separated and the new contact.
    /// ピンの上のラベルが離れて別のピンの上に乗ると、断り文は離れた組と新しく重なった組を書く。
    #[test]
    fn the_self_check_names_separated_and_new_pin_contacts() {
        let mut before = new_sheet();
        place(&mut before, "box2", (50.0, 50.0), "A1");
        place(&mut before, "box2", (100.0, 50.0), "A2");
        let l = label(&mut before, (60.0, 50.0), "Y");
        let mut after = before.clone();
        after.entities.get_mut(&l).expect("label").translate(30.0, 0.0);
        let err = verify_after(&before, &after, &defs(), &BTreeSet::new(), &[]).expect_err("must refuse");
        assert!(err.contains("離れた: ラベル Y と A1.2"), "{err}");
        assert!(err.contains("新しく重なった: ラベル Y と A2.1"), "{err}");
    }

    /// Nets that can be routed are still routed when another cluster fails: the failed one gets its label stubs and the rest is routed around them.
    /// 引けないかたまりがあっても、引けるネットは引き直され、引けなかったかたまりは短い線とラベルでつながる。
    #[test]
    fn routing_goes_on_around_the_stubs_of_failed_clusters() {
        let (mut sh, _, _) = walled_sheet();
        // 引けなかったかたまりの短い線 (A2 は左へ詰まって (105,50)。そのピンから西へ) の先を、縦の配線がまっすぐ通る
        place(&mut sh, "cap", (135.0, 95.0), "C1");
        place(&mut sh, "cap", (135.0, 20.0), "C2");
        old_wire(&mut sh, &[(135.0, 85.0), (135.0, 30.0)]);
        let plan = arrange(&sh);
        assert_eq!(plan.labelled.len(), 1, "{:?}", plan.labelled);
        assert_eq!(plan.routed.len(), 1, "{:?}", plan.routed);
        let after = apply(&sh, &plan);
        assert_eq!(kept_partition(&sh, &after), pin_partition(&after, &defs()));
    }

    /// The bent wire of `bent_wire_sheet` with a branch wire going up from (x, 50) to (x, 30).
    fn branched_sheet(x: f64) -> (Sheet, Vec<EntityId>) {
        let (mut sh, mut ws) = bent_wire_sheet();
        ws.push(old_wire(&mut sh, &[(x, 50.0), (x, 30.0)]));
        (sh, ws)
    }

    /// A stretch that would leave a branch wire hanging off the moved segment (a T-joint coming loose) is refused, so the cluster falls back to labels.
    /// 伸び縮みで、動いた線分から枝の線が外れる (T字のつなぎ目が離れる) なら断り、かたまりはラベルでつなぐ。
    #[test]
    fn stretching_is_refused_when_a_t_joint_would_come_loose() {
        let (sh, ws) = branched_sheet(67.5);
        let trip = [
            (p(60.0, 50.0), p(60.0, 52.5), None),
            (p(90.0, 80.0), p(90.0, 80.0), None),
            (p(67.5, 30.0), p(67.5, 30.0), None),
        ];
        assert!(stretch_wires(&sh, &ws, &[], &identity_layout(), &trip, &[], &[], &[], &BTreeSet::new()).is_none());
    }

    /// A stretch that would leave a pin in the middle of a segment off the wire is refused.
    /// 伸び縮みで、線分の途中にあるピンが線から外れるなら断る。
    #[test]
    fn stretching_is_refused_when_a_mid_segment_terminal_would_come_off() {
        let (sh, ws) = bent_wire_sheet();
        let trip = [
            (p(60.0, 50.0), p(60.0, 50.0), None),
            (p(90.0, 80.0), p(90.0, 80.0), None),
            (p(82.5, 80.0), p(82.5, 82.5), None),
        ];
        assert!(stretch_wires(&sh, &ws, &[], &identity_layout(), &trip, &[], &[], &[], &BTreeSet::new()).is_none());
    }

    /// A stretch that would leave a junction dot off the moved segment is refused.
    /// 伸び縮みで、ジャンクションが動いた線分から外れるなら断る。
    #[test]
    fn stretching_is_refused_when_a_junction_would_come_off() {
        let (mut sh, ws) = bent_wire_sheet();
        let j = junction(&mut sh, (67.5, 50.0));
        let trip = [(p(60.0, 50.0), p(60.0, 52.5), None), (p(90.0, 80.0), p(90.0, 80.0), None)];
        assert!(stretch_wires(&sh, &ws, &[j], &identity_layout(), &trip, &[], &[], &[], &BTreeSet::new()).is_none());
    }

    /// A branch that starts at a bend, and the junction there, follow the bend when it moves, so the cluster stays connected.
    /// 曲がり角から出る枝の線とそこのジャンクションは、曲がり角が動くと一緒に動き、かたまりはつながったまま。
    #[test]
    fn a_branch_at_a_bend_follows_the_bend() {
        let (mut sh, ws) = branched_sheet(75.0);
        let j = junction(&mut sh, (75.0, 50.0));
        let trip = [
            (p(60.0, 50.0), p(60.0, 52.5), None),
            (p(90.0, 80.0), p(90.0, 80.0), None),
            (p(75.0, 30.0), p(75.0, 30.0), None),
        ];
        let (es, _) = stretch_wires(&sh, &ws, &[j], &identity_layout(), &trip, &[], &[], &[], &BTreeSet::new()).expect("stretch");
        assert_eq!(
            wire_points(&es),
            vec![vec![(60.0, 52.5), (75.0, 52.5), (75.0, 80.0), (90.0, 80.0)], vec![(75.0, 52.5), (75.0, 30.0)]]
        );
        let dots: Vec<(f64, f64)> = es.iter().filter_map(|e| if let Entity::Junction(j) = e { Some((j.at.x, j.at.y)) } else { None }).collect();
        assert_eq!(dots, vec![(75.0, 52.5)]);
    }

    /// An unroutable cluster whose stretched wire would leave a moved mid-wire pin behind is joined by labels, and the arrange is not refused.
    /// 引けないかたまりで、伸び縮みさせると線の途中のピン (整列で動く部品) が外れるなら、ラベルでつなぎ、並べ直し全体は断らない。
    #[test]
    fn a_stretch_that_would_drop_a_mid_wire_pin_falls_back_to_labels() {
        let (mut sh, ds) = slit_sheet();
        // C1 のピンは線の途中 (120,50) にあり、C2 と行がそろって C1 が下へ動く
        place(&mut sh, "cap", (120.0, 60.0), "C1");
        place(&mut sh, "cap", (200.0, 57.5), "C2");
        let plan = plan_arrange(&sh, &ds, &ArrangeOptions::default(), &ctx()).expect("plan");
        assert_eq!(plan.labelled.len(), 1, "{:?}", plan.labelled);
        assert!(plan.kept_shape.is_empty(), "{:?}", plan.kept_shape);
        let after = apply(&sh, &plan);
        assert_eq!(pin_partition(&sh, &ds), pin_partition(&after, &ds));
    }

    /// A route drawn in the same round as a new label stub, and crossed by it, is drawn again in the next round around the stub; routes the stub does not touch are kept as they are.
    /// 同じ回に置いたラベルの短い線と交わる経路は、次の回にその線を避けて引き直し、触れない経路はそのまま使う。
    #[test]
    fn a_route_crossed_by_a_new_stub_is_routed_again_next_round() {
        let (mut sh, _, _) = walled_sheet();
        place(&mut sh, "cap", (135.0, 95.0), "C1");
        place(&mut sh, "cap", (135.0, 20.0), "C2");
        old_wire(&mut sh, &[(135.0, 85.0), (135.0, 30.0)]);
        place(&mut sh, "cap", (220.0, 95.0), "C3");
        place(&mut sh, "cap", (220.0, 20.0), "C4");
        old_wire(&mut sh, &[(220.0, 85.0), (220.0, 30.0)]);
        let c = collect_route_units(&sh, &defs(), &RerouteOptions::default(), &[]);
        let at_x = |x: f64| c.units.iter().position(|u| u.terms.iter().all(|t| (t.0.x - x).abs() < 1e-6)).expect("unit");
        let (mid, far) = (at_x(135.0), at_x(220.0));
        let grid = RouteGrid::build(&sh, &defs(), &RouteOptions::default());
        // 引けなかったかたまりの短い線の先 (ラベル) が、1回目の経路の最も長い線分の中ほどに来る
        let first = route_units(&grid, &mut c.units.clone(), vec![], vec![]);
        let (lines, _) = route_shape(&c.units[mid], first[mid].as_ref().expect("first route"));
        let seg = lines.iter().flat_map(|l| l.windows(2)).max_by(|a, b| a[0].distance_to(&a[1]).total_cmp(&b[0].distance_to(&b[1]))).expect("seg");
        let end = Point::new(rnd((seg[0].x + seg[1].x) / 2.0), rnd((seg[0].y + seg[1].y) / 2.0));
        let mut failed = Vec::new();
        let (outcome, rounds) = route_rounds(&grid, &c.units, &[], &[], |i| {
            failed.push(i);
            (vec![vec![end.translated(-15.0, 0.0), end]], vec![end])
        });
        assert_eq!(failed.len(), 1);
        assert_eq!(rounds, vec![(0..c.units.len()).collect::<Vec<_>>(), vec![mid]]);
        let r = outcome[mid].as_ref().expect("routed again");
        let (lines, _) = route_shape(&c.units[mid], r);
        assert!(lines.iter().all(|l| l.windows(2).all(|w| seg_dist(end, w[0], w[1]) > 1.0)), "{lines:?}");
        assert!(outcome[far].is_some());
    }

    /// The self-check refuses a plan that deletes a part, even one without pins.
    /// 検算は、部品を消す計画を断る (ピンの無い部品でも)。
    #[test]
    fn the_self_check_refuses_a_deleted_part() {
        let (mut before, _, a2) = two_wired_parts();
        let w = place(&mut before, "wall_h", (75.0, 100.0), "");
        for gone in [a2, w] {
            let mut after = before.clone();
            after.entities.remove(&gone);
            assert!(verify_after(&before, &after, &defs(), &BTreeSet::new(), &[]).is_err());
        }
    }

    /// Port-contact names such as "BUS.VBUS" are never borrowed as a fallback label name, because no label with that text is on the sheet.
    /// 「BUS.VBUS」のようなポートの接点の名前は、代わりのラベル名に借りない (その文字のラベルは図面に無いため)。
    #[test]
    fn contact_net_names_are_not_borrowed_for_labels() {
        use crate::block::port_fixtures::{pin_at as port_pin, port_block, USB};
        let mut sh = new_sheet();
        let hub = Entity::Symbol(port_block("39", 100.0, 100.0, "right", "U3", &USB));
        let dongle = Entity::Symbol(port_block("LTE1", 200.0, 100.0, "left", "P1", &USB));
        let (hub, dongle) = (hub.id(), {
            sh.entities.insert(hub.id(), hub);
            let id = dongle.id();
            sh.entities.insert(id, dongle);
            id
        });
        let (a, b) = (port_pin(&sh, hub, "U3"), port_pin(&sh, dongle, "P1"));
        old_wire(&mut sh, &[(a.x, a.y), (b.x, b.y)]);
        label(&mut sh, ((a.x + b.x) / 2.0, a.y), "BUS");
        let ds = crate::symbol::sheet_symbol_defs(&sh);
        let nets = extract_netlist(&sh, &ds);
        assert!(nets.iter().any(|n| n.contact.is_some() && n.label.is_some()), "{nets:?}");
        let names = net_label_names(&sh, &ds);
        assert!(names.values().all(|l| !l.contains('.')), "{names:?}");
    }

    /// A kept link whose wire has no points is skipped instead of panicking.
    /// 点の無い配線を残す線に指定しても、止まらずに飛ばす。
    #[test]
    fn a_kept_link_without_points_is_skipped() {
        let mut sh = new_sheet();
        let w = old_wire(&mut sh, &[]);
        let us = units(&sh, &defs());
        let owners = Owners::new(&sh, &defs(), &us);
        assert!(kept_links(&sh, &owners, &identity_layout(), &[w]).is_empty());
    }

    /// When every stub candidate would touch something, the plain 2.5 mm stub is used and the stub check then refuses it, naming the stub.
    /// どの短い線の候補も何かに触れるときは 2.5 mm のまっすぐな線を使い、検算がそれを断る (短い線のことを挙げて)。
    #[test]
    fn a_stub_with_no_free_candidate_is_refused_by_the_stub_check() {
        let (grid, fb) = stub_fixture();
        // ピンの東・北・南のすぐ先に点: どの候補もどれかの点の上を通る
        let dots = [p(61.25, 50.0), p(60.0, 51.25), p(60.0, 48.75)];
        let (es, ss) = stub_entities(&grid, &fb, &mut vec![], &mut dots.to_vec());
        assert_eq!(stub_points(&es), vec![(60.0, 50.0), (62.5, 50.0)]);
        let mut sh = new_sheet();
        for e in es {
            sh.entities.insert(e.id(), e);
        }
        junction(&mut sh, (61.25, 50.0));
        let err = check_stubs(&sh, &defs(), &ss).expect_err("touching stub");
        assert!(err.contains("ジャンクション"), "{err}");
    }
}
