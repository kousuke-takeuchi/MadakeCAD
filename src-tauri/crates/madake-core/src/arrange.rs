//! 部品の並べ直し (整列と間隔の調整) の幾何の部品。
//!
//! 一緒に動くもの (単位)、列・行 (帯)、帯のずらし量、単調な座標の写像、整列の移動量を求める。
//! この模塊の関数は全て**純粋**で、図面 (モデル) を書き換えない。

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

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
            Entity::Symbol(s) if idx.get(s).is_some_and(|d| d.net_name().is_some()) => Some(s.id),
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
/// keys と同じ並びで返す。
pub fn align_moves(keys: &[f64], tol: f64) -> Vec<f64> {
    let mut moves = vec![0.0; keys.len()];
    let mut order: Vec<usize> = (0..keys.len()).collect();
    order.sort_by(|&a, &b| keys[a].total_cmp(&keys[b]).then(a.cmp(&b)));
    let mut group: Vec<usize> = Vec::new();
    let flush = |group: &mut Vec<usize>, moves: &mut Vec<f64>| {
        if group.is_empty() {
            return;
        }
        let target = keys[group[(group.len() - 1) / 2]];
        for &i in group.iter() {
            moves[i] = ((target - keys[i]) / STEP_MM).round() * STEP_MM;
        }
        group.clear();
    };
    let mut prev: Option<f64> = None;
    for i in order {
        if let Some(p) = prev {
            if keys[i] - p >= tol {
                flush(&mut group, &mut moves);
            }
        }
        group.push(i);
        prev = Some(keys[i]);
    }
    flush(&mut group, &mut moves);
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
struct Fallback {
    net: String,
    name: String,
    terms: Vec<(Point, Option<Dir>, Wire)>,
    /// 新しいラベルの向きと属性の手本。
    template: Option<NetLabel>,
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

fn overlap(a: &Bounds, b: &Bounds) -> bool {
    a.0 < b.2 - EPS && b.0 < a.2 - EPS && a.1 < b.3 - EPS && b.1 < a.3 - EPS
}

/// 基準の座標を、差が tol 未満のものどうしの組に分ける ([`align_moves`] と同じ分け方)。組は keys の添字。
fn key_groups(keys: &[f64], tol: f64) -> Vec<Vec<usize>> {
    let mut order: Vec<usize> = (0..keys.len()).collect();
    order.sort_by(|&a, &b| keys[a].total_cmp(&keys[b]).then(a.cmp(&b)));
    let mut out: Vec<Vec<usize>> = Vec::new();
    let mut prev: Option<f64> = None;
    for i in order {
        match prev {
            Some(p) if keys[i] - p < tol => out.last_mut().expect("組がある").push(i),
            _ => out.push(vec![i]),
        }
        prev = Some(keys[i]);
    }
    out
}

/// 組 (ids) を新しい外接矩形 (newb) へ動かすと、今まで重なっていなかった2つの単位が重なるか。
fn creates_overlap(cur: &[Bounds], ids: &[usize], newb: &[Bounds]) -> bool {
    for (p, &i) in ids.iter().enumerate() {
        for j in 0..cur.len() {
            if j == i {
                continue;
            }
            match ids.iter().position(|&x| x == j) {
                Some(q) => {
                    if j > i && overlap(&newb[p], &newb[q]) && !overlap(&cur[i], &cur[j]) {
                        return true;
                    }
                }
                None => {
                    if overlap(&newb[p], &cur[j]) && !overlap(&cur[i], &cur[j]) {
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
    let Some(b) = b else { return true };
    let f = ctx.frame;
    if b.0 < f.0 - EPS || b.1 < f.1 - EPS || b.2 > f.2 + EPS || b.3 > f.3 + EPS {
        return false;
    }
    hit_title || !rects_hit(b, ctx.title_block)
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
}

/// ラベルでつなぐときの名前と、消すラベルのid。
/// かたまりのラベルの名前 (ラベルは消す)、なければネット名シンボルのピンの名前、なければ未使用の `ARR_<n>`。
/// 他から指されているラベル (`pointed`) と、ピンの端子が無いかたまりのラベル (代わりのラベルが作られない) は消さない。
fn fallback_name(
    sheet: &Sheet,
    idx: &DefIndex,
    pins: &[PinPt],
    wires: &[EntityId],
    terms: &[(Point, Option<Dir>)],
    pointed: &BTreeSet<EntityId>,
    taken: &mut BTreeSet<String>,
) -> FallbackName {
    let labels = cluster_labels(sheet, wires);
    if let Some(first) = labels.first() {
        let has_pin = terms.iter().any(|t| t.1.is_some());
        let delete: Vec<EntityId> = if has_pin { labels.iter().map(|l| l.0).filter(|id| !pointed.contains(id)).collect() } else { vec![] };
        let template = match sheet.entities.get(&labels[0].0) {
            Some(Entity::NetLabel(l)) => Some(l.clone()),
            _ => None,
        };
        return FallbackName { name: first.1.clone(), delete, template, fresh: false };
    }
    let at: Vec<Point> = terms.iter().map(|t| t.0).collect();
    if let Some(n) = term_symbol_names(sheet, idx, pins, &at).into_iter().next() {
        return FallbackName { name: n, delete: vec![], template: None, fresh: false };
    }
    let mut n = 1;
    loop {
        let name = format!("ARR_{n}");
        if !taken.contains(&name) {
            taken.insert(name.clone());
            return FallbackName { name, delete: vec![], template: None, fresh: true };
        }
        n += 1;
    }
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
struct Stub {
    wire: EntityId,
    label: EntityId,
    name: String,
}

/// ラベルでつなぐ短い線とラベル。ピンから引き出し線の先 (なければ向きへ2.5mm) まで線を引き、その先にラベルを置く。
/// ラベルの向きと属性は、消したラベルがあればその先頭に倣う。
fn stub_entities(grid: &RouteGrid, fb: &Fallback) -> (Vec<Entity>, Vec<Stub>) {
    let mut out = Vec::new();
    let mut stubs = Vec::new();
    for (pos, dir, attrs) in &fb.terms {
        let Some(dir) = dir else { continue };
        let pts = match make_lead(grid, *pos, Some(*dir)) {
            Some(l) if l.points.len() >= 2 => simplify(l.points),
            _ => {
                let (sx, sy) = dir_step(*dir);
                vec![*pos, pos.translated(sx * STUB_MM, sy * STUB_MM)]
            }
        };
        let end = *pts.last().expect("2点以上");
        let (wire, label) = (uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
        let (rotation, label_attrs) = fb.template.as_ref().map_or((0, Default::default()), |t| (t.rotation, t.attrs.clone()));
        out.push(Entity::Wire(Wire { id: wire, points: pts, ..attrs.clone() }));
        out.push(Entity::NetLabel(NetLabel { id: label, at: end, name: fb.name.clone(), rotation, attrs: label_attrs }));
        stubs.push(Stub { wire, label, name: fb.name.clone() });
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
    for s in stubs {
        let Some(Entity::Wire(w)) = after.entities.get(&s.wire) else { continue };
        let Some(start) = w.points.first().copied() else { continue };
        let own: BTreeSet<EntityId> = stubs.iter().filter(|o| o.name == s.name).flat_map(|o| [o.wire, o.label]).collect();
        let ends = |x: &Wire| -> Vec<Point> { [x.points.first(), x.points.last()].into_iter().flatten().copied().collect() };
        let touched = after.entities.values().any(|e| match e {
            _ if own.contains(&e.id()) => false,
            Entity::Wire(o) => ends(o).iter().any(|q| on_wire(w, q)) || ends(w).iter().any(|q| on_wire(o, q)),
            Entity::Junction(j) => on_wire(w, &j.at),
            Entity::NetLabel(l) => on_wire(w, &l.at),
            _ => false,
        }) || pp.iter().any(|q| on_wire(w, &q.pos) && !near(q.pos, start));
        if touched {
            return Err(format!("並べ直しの検算で、ラベル {} の引き出し線が他の配線やピンに触れたため、何も変えませんでした", s.name));
        }
    }
    Ok(())
}

/// 検算: 計画を当てた写しで、ピンの分け方・ネットごとの名前 (新しい `ARR_<n>` を除く)・ラベルでつなぐ短い線の孤立を確かめる。
fn verify_after(before: &Sheet, after: &Sheet, defs: &[SymbolDef], fresh: &BTreeSet<String>, stubs: &[Stub]) -> Result<(), String> {
    if pin_partition(before, defs) != pin_partition(after, defs) {
        return Err("並べ直しの検算で接続が変わったため、何も変えませんでした".into());
    }
    if name_key(before, defs, fresh) != name_key(after, defs, fresh) {
        return Err("並べ直しの検算でネットの名前 (ラベル・ネット名シンボル) が変わったため、何も変えませんでした".into());
    }
    // 配線の無いピンの上のラベル・ネット名シンボルはネットリストに出ないので、位置の重なりで比べる
    let (pb, pa) = (pin_contacts(before, defs), pin_contacts(after, defs));
    let both = |set: &BTreeSet<(EntityId, EntityId, String)>, other: &Sheet| -> BTreeSet<(EntityId, EntityId, String)> {
        set.iter().filter(|c| other.entities.contains_key(&c.0) && other.entities.contains_key(&c.1)).cloned().collect()
    };
    if both(&pb, after) != both(&pa, before) {
        return Err("並べ直しの検算で、ピンの上のラベルやネット名シンボルが離れたため、何も変えませんでした".into());
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
            Entity::Symbol(s) if idx.get(s).is_some_and(|d| d.net_name().is_some()) => {
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

/// 端子の単位が全て動かず、配線とジャンクションの全ての点が写像で動かないか (それなら元の配線をそのまま残せる)。
fn unit_stays(sheet: &Sheet, wires: &[EntityId], junctions: &[EntityId], owns: &[usize], lay: &Layout) -> bool {
    if owns.is_empty() || owns.iter().any(|&o| lay.dx[o].abs() > EPS || lay.dy[o].abs() > EPS) {
        return false;
    }
    let fixed = |p: &Point| (lay.wx.map(p.x) - p.x).abs() < EPS && (lay.wy.map(p.y) - p.y).abs() < EPS;
    wires.iter().chain(junctions).all(|id| sheet.entities.get(id).map_or(true, |e| shape_points(e).iter().all(fixed)))
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
                let fb = fallback_name(sheet, &idx, &pins, &k.wires, &k.terms, &pointed, &mut taken);
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
            c.units.iter().zip(&unit_terms).map(|(u, owns)| unit_stays(sheet, &u.wires, &u.junctions, owns, &lay)).collect();
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
    let mut remove: BTreeSet<EntityId> = deleted.clone();
    let mut labelled: Vec<(String, String)> = Vec::new();
    for (ci, name) in kept_fallbacks {
        let k = &c.kept[ci];
        let owns = term_units(&k.net, &k.terms, &owners)?;
        let terms = k
            .terms
            .iter()
            .zip(&owns)
            .map(|(t, &o)| (t.0.translated(lay.dx[o], lay.dy[o]), t.1, old_wire_at(sheet, &k.wires, t.0)))
            .collect();
        let fb = Fallback { net: k.net.clone(), name: name.name, terms, template: name.template };
        let (es, ss) = stub_entities(&grid, &fb);
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

    // 配線の引き直し。引けなかった単位はラベルでつなぐ
    let results = route_units(&grid, &mut route, fixed_lines, fixed_spots);
    let mut routed: BTreeSet<String> = BTreeSet::new();
    for ((u, r), trip) in route.iter().zip(results).zip(&trips) {
        remove.extend(u.wires.iter().chain(&u.junctions).copied());
        match r {
            Ok(r) => {
                add.extend(emit(u, &r));
                routed.insert(u.net.clone());
            }
            Err(_) => {
                let old_terms: Vec<(Point, Option<Dir>)> = trip.iter().map(|t| (t.0, t.2)).collect();
                let name = fallback_name(sheet, &idx, &pins, &u.wires, &old_terms, &pointed, &mut taken);
                if name.fresh {
                    fresh.insert(name.name.clone());
                }
                deleted.extend(name.delete.iter().copied());
                remove.extend(name.delete.iter().copied());
                let mut attrs = u.template.clone();
                attrs.length_m = None;
                attrs.length_source = Default::default();
                let terms = trip.iter().map(|t| (t.1, t.2, attrs.clone())).collect();
                let fb = Fallback { net: u.net.clone(), name: name.name, terms, template: name.template };
                let (es, ss) = stub_entities(&grid, &fb);
                add.extend(es);
                stubs.extend(ss);
                labelled.push((fb.net, fb.name));
            }
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
        size_before: size_of(before),
        ..Default::default()
    };

    // 検算: 計画を写しに当てて、接続と名前が変わっていないこと、引いた後も図枠に収まることを確かめる
    let after = apply_plan(sheet, &plan);
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
}
