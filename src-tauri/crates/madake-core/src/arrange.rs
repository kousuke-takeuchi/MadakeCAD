//! 部品の並べ直し (整列と間隔の調整) の幾何の部品。
//!
//! 一緒に動くもの (単位)、列・行 (帯)、帯のずらし量、単調な座標の写像、整列の移動量を求める。
//! この模塊の関数は全て**純粋**で、図面 (モデル) を書き換えない。

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::geometry::Point;
use crate::leader::entity_bounds;
use crate::model::{Entity, EntityId, Sheet};
use crate::netlist::{extract_netlist, on_wire};
use crate::route::{pin_points, RouteOptions};
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

/// シートの単位を、主のid順に返す。
pub fn units(sheet: &Sheet, defs: &[SymbolDef]) -> Vec<Unit> {
    let idx = DefIndex::new(defs);

    // シンボルごとのピン数。
    let mut pin_count: BTreeMap<EntityId, usize> = BTreeMap::new();
    for pp in pin_points(sheet, defs) {
        *pin_count.entry(pp.entity).or_insert(0) += 1;
    }
    let one_pin = |id: &EntityId| pin_count.get(id) == Some(&1);

    // ネットごとの (ピンの持ち主, ラベル) を集める。
    struct NetFacts {
        owners: BTreeSet<EntityId>,
        labels: BTreeSet<EntityId>,
    }
    let nets = extract_netlist(sheet, defs);
    let mut facts: Vec<NetFacts> = Vec::new();
    for net in &nets {
        let owners: BTreeSet<EntityId> = net.pins.iter().map(|p| p.entity_id).collect();
        let mut labels = BTreeSet::new();
        for e in sheet.entities.values() {
            let Entity::NetLabel(l) = e else { continue };
            let hit = net.wire_ids.iter().any(|wid| match sheet.entities.get(wid) {
                Some(Entity::Wire(w)) => on_wire(w, &l.at),
                _ => false,
            });
            if hit {
                labels.insert(l.id);
            }
        }
        facts.push(NetFacts { owners, labels });
    }

    // 衛星の判定: 衛星の候補 (1ピンのシンボル・ラベル) -> 親のシンボル。
    let mut parent_of: BTreeMap<EntityId, EntityId> = BTreeMap::new();
    for e in sheet.entities.values() {
        let (id, is_label) = match e {
            Entity::Symbol(s) if one_pin(&s.id) => (s.id, false),
            Entity::NetLabel(l) => (l.id, true),
            _ => continue,
        };
        let mut found: Option<EntityId> = None;
        let mut ok = true;
        let mut touched = false;
        for f in &facts {
            let on_net = if is_label { f.labels.contains(&id) } else { f.owners.contains(&id) };
            if !on_net {
                continue;
            }
            touched = true;
            // 他のラベル・他の1ピンのシンボルがあれば衛星ではない。
            let other_label = f.labels.iter().any(|l| *l != id);
            let other_one_pin = f.owners.iter().any(|o| *o != id && one_pin(o));
            // 衛星のシンボル自身がラベルの付いたネットにいるのも、ラベルが別にあるので衛星ではない。
            let others: Vec<EntityId> = f.owners.iter().copied().filter(|o| *o != id).collect();
            if other_label || other_one_pin || others.len() != 1 {
                ok = false;
                break;
            }
            match found {
                None => found = Some(others[0]),
                Some(p) if p == others[0] => {}
                _ => {
                    ok = false;
                    break;
                }
            }
        }
        if ok && touched {
            if let Some(p) = found {
                parent_of.insert(id, p);
            }
        }
    }

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
    let mut flush = |group: &mut Vec<usize>, moves: &mut Vec<f64>| {
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

    fn defs() -> Vec<SymbolDef> {
        vec![box2(), mark()]
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
}
