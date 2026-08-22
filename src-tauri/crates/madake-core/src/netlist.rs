//! ネットリスト抽出: 配置済みシンボルのピン座標解決と、配線・ジャンクション・
//! ネットラベルからの接続グラフ導出。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::geometry::Point;
use crate::model::{Entity, EntityId, Sheet, SymbolInstance};
use crate::symbol::SymbolDef;

/// 接続判定の座標一致許容誤差 (mm)。
pub const CONNECT_EPS: f64 = 0.01;

/// ローカル座標をミラー→回転(0/90/180/270、時計回り、Y下向き座標系)する。
///
/// 平行移動は含まない。シンボルのピン解決 ([`transform_local`]) と回路マクロの
/// 回転挿入 ([`crate::macros`]) が同じ回転規則を共有するための共通部品。
pub fn rotate_local(p: Point, rotation: u16, mirror: bool) -> Point {
    let (x, y) = if mirror { (-p.x, p.y) } else { (p.x, p.y) };
    match rotation % 360 {
        90 => Point::new(-y, x),
        180 => Point::new(-x, -y),
        270 => Point::new(y, -x),
        _ => Point::new(x, y),
    }
}

/// 弧の開始・終了角をミラー→回転する([`rotate_local`]と同じ規則)。
///
/// 描画は常に start→end を時計回りに描くので、ミラーのときは向きが反転する分だけ
/// 始点と終点を入れ替える。
pub fn rotate_arc_angles(start_deg: f64, end_deg: f64, rotation: u16, mirror: bool) -> (f64, f64) {
    let (s, e) = if mirror {
        (180.0 - end_deg, 180.0 - start_deg)
    } else {
        (start_deg, end_deg)
    };
    let span = e - s;
    let start = (s + f64::from(rotation % 360)).rem_euclid(360.0);
    (start, start + span)
}

/// ローカル座標をミラー→回転(0/90/180/270、時計回り、Y下向き座標系)→平行移動する。
pub fn transform_local(p: Point, inst: &SymbolInstance) -> Point {
    let r = rotate_local(p, inst.rotation, inst.mirror);
    Point::new(inst.at.x + r.x, inst.at.y + r.y)
}

/// シンボルインスタンスの各ピンの (ピン番号, 用紙上絶対座標) を返す。
pub fn pin_positions(inst: &SymbolInstance, def: &SymbolDef) -> Vec<(String, Point)> {
    def.pins
        .iter()
        .map(|pin| (pin.number.clone(), transform_local(pin.at, inst)))
        .collect()
}

/// ネットに属するシンボルピン。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NetPin {
    /// 参照記号 (例: "K1")。
    pub reference: String,
    /// シンボルインスタンスのentity id。
    pub entity_id: EntityId,
    /// ピン番号。
    pub pin: String,
}

/// 電気的に接続された1ネット。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Net {
    /// 表示名。優先順は ネットラベル > 線番 > 自動名 (N001…)。
    pub name: String,
    pub pins: Vec<NetPin>,
    pub wire_ids: Vec<EntityId>,
    /// ネットラベル由来の名前 (手動指定)。
    #[serde(default)]
    pub label: Option<String>,
    /// 線番 (`Wire::net`)。ネット内の全Wireで同値になる。
    #[serde(default)]
    pub wire_no: Option<String>,
}

struct DisjointSet {
    parent: Vec<usize>,
}

impl DisjointSet {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
        }
    }
    fn find(&mut self, i: usize) -> usize {
        if self.parent[i] != i {
            let r = self.find(self.parent[i]);
            self.parent[i] = r;
        }
        self.parent[i]
    }
    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.parent[ra] = rb;
        }
    }
}

fn near(a: &Point, b: &Point) -> bool {
    a.distance_to(b) < CONNECT_EPS
}

/// 点pから線分ab までの距離。
pub(crate) fn segment_distance(p: &Point, a: &Point, b: &Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 < 1e-12 {
        return p.distance_to(a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    p.distance_to(&Point::new(a.x + t * dx, a.y + t * dy))
}

/// 点がWireのいずれかの線分上(許容誤差内)にあるか。
pub(crate) fn on_wire(w: &crate::model::Wire, p: &Point) -> bool {
    w.points
        .windows(2)
        .any(|seg| segment_distance(p, &seg[0], &seg[1]) < CONNECT_EPS)
}

/// シートからネットリストを導出する。
///
/// 接続ルール:
/// - Wire同士は「端点同士の一致」または「一致点上のJunction経由」でのみ接続(交差だけでは非接続)
/// - シンボルピン・NetLabelはWireの任意の頂点との一致で接続
/// - 同名NetLabelのネットは統合され、ネット名はラベル名(辞書順最小)になる
/// - 無名ネットは決定的な順序で "N001" から連番
pub fn extract_netlist(sheet: &Sheet, symbols: &[SymbolDef]) -> Vec<Net> {
    let defs: BTreeMap<&str, &SymbolDef> =
        symbols.iter().map(|d| (d.id.as_str(), d)).collect();

    // ノード列挙 (BTreeMap順なので決定的)
    let mut wires = Vec::new();
    let mut junctions = Vec::new();
    let mut labels = Vec::new();
    let mut pins = Vec::new(); // (entity_id, reference, pin_no, pos)
    for entity in sheet.entities.values() {
        match entity {
            Entity::Wire(w) => wires.push(w),
            Entity::Junction(j) => junctions.push(j),
            Entity::NetLabel(l) => labels.push(l),
            Entity::Symbol(s) => {
                if let Some(def) = defs.get(s.symbol_id.as_str()) {
                    for (no, pos) in pin_positions(s, def) {
                        pins.push((s.id, s.reference.clone(), no, pos));
                    }
                }
            }
            // 注記とハーネス境界は電気的な接続を持たない
            Entity::Text(_) | Entity::Harness(_) => {}
        }
    }

    // ノード番号: [wires][pins][junctions][labels]
    let wi = 0;
    let pi = wi + wires.len();
    let ji = pi + pins.len();
    let li = ji + junctions.len();
    let mut ds = DisjointSet::new(li + labels.len());

    let endpoints = |w: &crate::model::Wire| -> Vec<Point> {
        match (w.points.first(), w.points.last()) {
            (Some(a), Some(b)) => vec![*a, *b],
            _ => vec![],
        }
    };

    // Wire端点同士
    for (a, wa) in wires.iter().enumerate() {
        for (b, wb) in wires.iter().enumerate().skip(a + 1) {
            let touch = endpoints(wa)
                .iter()
                .any(|ea| endpoints(wb).iter().any(|eb| near(ea, eb)));
            if touch {
                ds.union(wi + a, wi + b);
            }
        }
    }
    // Junction ⇔ Wire上の任意点(線分途中も可)
    for (j, junc) in junctions.iter().enumerate() {
        for (a, w) in wires.iter().enumerate() {
            if on_wire(w, &junc.at) {
                ds.union(ji + j, wi + a);
            }
        }
    }
    // ピン ⇔ Wire上の任意点
    for (p, (_, _, _, pos)) in pins.iter().enumerate() {
        for (a, w) in wires.iter().enumerate() {
            if on_wire(w, pos) {
                ds.union(pi + p, wi + a);
            }
        }
    }
    // 同一シンボル内の同一ピン番号の接続点は内部短絡(端子台の左右貫通)
    for (p, (eid, _, no, _)) in pins.iter().enumerate() {
        for (p2, (eid2, _, no2, _)) in pins.iter().enumerate().skip(p + 1) {
            if eid == eid2 && no == no2 {
                ds.union(pi + p, pi + p2);
            }
        }
    }
    // ラベル ⇔ Wire上の任意点、同名ラベル同士
    for (l, lab) in labels.iter().enumerate() {
        for (a, w) in wires.iter().enumerate() {
            if on_wire(w, &lab.at) {
                ds.union(li + l, wi + a);
            }
        }
        for (l2, lab2) in labels.iter().enumerate().skip(l + 1) {
            if lab.name == lab2.name {
                ds.union(li + l, li + l2);
            }
        }
    }

    // グループ化 (Wireを1本以上含むグループのみネットとする)
    #[derive(Default)]
    struct Group {
        wire_ids: Vec<EntityId>,
        pins: Vec<NetPin>,
        label: Option<String>,
        /// グループ内のWireに書かれた線番 (重複可)。
        wire_nos: Vec<String>,
    }
    let mut groups: BTreeMap<usize, Group> = BTreeMap::new();
    for (a, w) in wires.iter().enumerate() {
        let g = groups.entry(ds.find(wi + a)).or_default();
        g.wire_ids.push(w.id);
        if let Some(no) = w.net.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
            g.wire_nos.push(no.to_string());
        }
    }
    for (p, (eid, reference, no, _)) in pins.iter().enumerate() {
        let root = ds.find(pi + p);
        if let Some(g) = groups.get_mut(&root) {
            // 同一シンボル同一番号の接続点(内部短絡済み)は1エントリに集約
            if !g.pins.iter().any(|np| np.entity_id == *eid && np.pin == *no) {
                g.pins.push(NetPin {
                    reference: reference.clone(),
                    entity_id: *eid,
                    pin: no.clone(),
                });
            }
        }
    }
    for (l, lab) in labels.iter().enumerate() {
        let root = ds.find(li + l);
        if let Some(g) = groups.get_mut(&root) {
            match &g.label {
                Some(cur) if *cur <= lab.name => {}
                _ => g.label = Some(lab.name.clone()),
            }
        }
    }

    // 決定的順序: グループ内最小wire idで整列し、無名ネットへ連番付与
    let mut ordered: Vec<Group> = groups.into_values().collect();
    ordered.sort_by_key(|g| g.wire_ids.iter().min().copied());
    let mut seq = 0;
    ordered
        .into_iter()
        .map(|mut g| {
            g.wire_ids.sort();
            g.pins.sort_by(|a, b| (&a.reference, &a.pin).cmp(&(&b.reference, &b.pin)));
            // 線番: 混在時は手動の名前 (数字以外) を優先し、以降は辞書順で決定的に選ぶ
            let wire_no = g
                .wire_nos
                .iter()
                .min_by(|a, b| {
                    (crate::wire_no::is_generated_number(a), *a)
                        .cmp(&(crate::wire_no::is_generated_number(b), *b))
                })
                .cloned();
            let name = g
                .label
                .clone()
                .or_else(|| wire_no.clone())
                .unwrap_or_else(|| {
                    seq += 1;
                    format!("N{seq:03}")
                });
            Net {
                name,
                pins: g.pins,
                wire_ids: g.wire_ids,
                label: g.label,
                wire_no,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::*;
    use crate::symbol::builtin_symbols;
    use uuid::Uuid;

    fn wire(points: &[(f64, f64)]) -> Entity {
        Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            color: "black".into(),
            sq: 0.3,
            length_m: None,
            part_no: None,
            net: None,
        })
    }

    fn symbol(sym: &str, reference: &str, x: f64, y: f64) -> Entity {
        Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: sym.into(),
            at: Point::new(x, y),
            rotation: 0,
            mirror: false,
            reference: reference.into(),
            value: String::new(),
            attrs: Default::default(),
        })
    }

    /// A wire whose endpoints touch two symbol pins joins those pins into one net.
    /// 両端がシンボルのピンに一致するワイヤは、それらのピンを1つのネットに結合する。
    #[test]
    fn wires_connect_symbol_pins_into_one_net() {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        // R1のピン2(x=107.5)とR2のピン1(x=142.5)を配線で接続
        for e in [
            symbol("resistor", "R1", 100.0, 50.0),
            symbol("resistor", "R2", 150.0, 50.0),
            wire(&[(107.5, 50.0), (142.5, 50.0)]),
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let nets = extract_netlist(&sheet, &builtin_symbols());
        let net = nets.iter().find(|n| n.pins.len() == 2).expect("connected net");
        let mut refs: Vec<_> = net
            .pins
            .iter()
            .map(|p| format!("{}:{}", p.reference, p.pin))
            .collect();
        refs.sort();
        assert_eq!(refs, vec!["R1:2", "R2:1"]);
    }

    /// Wires that merely cross do NOT connect; a net label attached to a wire names its net.
    /// 単に交差しただけのワイヤは接続されない。ワイヤに付いたネットラベルはそのネットの名前になる。
    #[test]
    fn crossing_without_junction_stays_separate_and_label_names_net() {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        // 十字交差(端点は不一致) + 横線にラベル
        let w1 = wire(&[(0.0, 10.0), (20.0, 10.0)]);
        let w2 = wire(&[(10.0, 0.0), (10.0, 20.0)]);
        let label = Entity::NetLabel(NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(0.0, 10.0),
            name: "24-P1".into(),
            rotation: 0,
        });
        for e in [w1, w2, label] {
            sheet.entities.insert(e.id(), e);
        }
        let nets = extract_netlist(&sheet, &builtin_symbols());
        assert_eq!(nets.len(), 2, "交差のみでは接続しない");
        assert!(nets.iter().any(|n| n.name == "24-P1"));
    }

    /// A junction dot connects crossing wires, and same-named labels merge distant nets into one.
    /// ジャンクションは交差ワイヤを接続し、同名ラベルは離れたネットを1つに統合する。
    #[test]
    fn junction_connects_crossing_wires_and_same_labels_merge() {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        let w1 = wire(&[(0.0, 10.0), (20.0, 10.0)]);
        let w2 = wire(&[(10.0, 0.0), (10.0, 20.0)]);
        let j = Entity::Junction(Junction {
            id: Uuid::new_v4(),
            at: Point::new(10.0, 10.0),
        });
        // 離れた2本を同名ラベルで論理接続
        let w3 = wire(&[(100.0, 100.0), (120.0, 100.0)]);
        let l1 = Entity::NetLabel(NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(0.0, 10.0),
            name: "48-P1".into(),
            rotation: 0,
        });
        let l2 = Entity::NetLabel(NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(100.0, 100.0),
            name: "48-P1".into(),
            rotation: 0,
        });
        for e in [w1, w2, j, w3, l1, l2] {
            sheet.entities.insert(e.id(), e);
        }
        let nets = extract_netlist(&sheet, &builtin_symbols());
        assert_eq!(nets.len(), 1, "ジャンクション+同名ラベルで全て1ネット");
        assert_eq!(nets[0].name, "48-P1");
        assert_eq!(nets[0].wire_ids.len(), 3);
    }

    /// Unnamed nets receive deterministic sequential names (N001, N002, ...).
    /// 無名ネットには決定的な連番名(N001, N002, …)が付く。
    #[test]
    fn unnamed_nets_get_deterministic_sequential_names() {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        sheet.entities.extend(
            [wire(&[(0.0, 0.0), (10.0, 0.0)]), wire(&[(0.0, 20.0), (10.0, 20.0)])]
                .map(|e| (e.id(), e)),
        );
        let nets = extract_netlist(&sheet, &builtin_symbols());
        let mut names: Vec<_> = nets.iter().map(|n| n.name.clone()).collect();
        names.sort();
        assert_eq!(names, vec!["N001", "N002"]);
    }

    /// Two connection points of the same symbol sharing a pin number (a feed-through terminal) are internally shorted, and appear once in the net's pin list.
    /// 同一シンボルで同じピン番号を持つ2つの接続点(貫通端子)は内部短絡され、ネットのピン一覧には1回だけ載る。
    #[test]
    fn same_pin_number_points_short_internally() {
        // 貫通端子: 同一シンボル内の同一ピン番号の接続点(左右)は内部短絡される
        let def = SymbolDef {
            id: "tb_test".into(),
            name: "TB test".into(),
            name_ja: "端子台テスト".into(),
            category: "connector".into(),
            ref_prefix: "T".into(),
            keywords: vec![],
            primitives: vec![],
            pins: vec![
                crate::symbol::PinDef {
                    number: "1".into(),
                    name: String::new(),
                    at: Point::new(-2.5, 0.0),
                    dir: crate::symbol::PinDir::Left,
                },
                crate::symbol::PinDef {
                    number: "1".into(),
                    name: String::new(),
                    at: Point::new(2.5, 0.0),
                    dir: crate::symbol::PinDir::Right,
                },
            ],
            text_slots: vec![],
        };
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        for e in [
            symbol("tb_test", "T1", 100.0, 50.0),
            wire(&[(80.0, 50.0), (97.5, 50.0)]),
            wire(&[(102.5, 50.0), (120.0, 50.0)]),
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let nets = extract_netlist(&sheet, &[def]);
        assert_eq!(nets.len(), 1, "左右のワイヤは端子貫通で1ネット: {nets:?}");
        assert_eq!(nets[0].wire_ids.len(), 2);
        // pinsは重複排除され1エントリ
        assert_eq!(nets[0].pins.len(), 1);
        assert_eq!(nets[0].pins[0].reference, "T1");
        assert_eq!(nets[0].pins[0].pin, "1");
    }

    /// A wire number written on the wires becomes the net's displayed name when the net has no net label.
    /// ワイヤに書かれた線番は、ネットラベルが無いネットの表示名になる。
    #[test]
    fn wire_number_names_a_net_without_a_label() {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        let mut w = wire(&[(0.0, 0.0), (10.0, 0.0)]);
        if let Entity::Wire(inner) = &mut w {
            inner.net = Some("12".into());
        }
        sheet.entities.insert(w.id(), w);
        let nets = extract_netlist(&sheet, &builtin_symbols());
        assert_eq!(nets[0].name, "12");
        assert_eq!(nets[0].wire_no.as_deref(), Some("12"));
        assert_eq!(nets[0].label, None);
    }

    /// A net label always wins over the wire number, which in turn wins over the automatic N001 name.
    /// ネットラベルは常に線番より優先され、線番は自動名 (N001) より優先される。
    #[test]
    fn net_name_prefers_label_then_wire_number_then_auto_name() {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        let mut labeled = wire(&[(0.0, 0.0), (10.0, 0.0)]);
        if let Entity::Wire(inner) = &mut labeled {
            inner.net = Some("12".into());
        }
        let label = Entity::NetLabel(NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(0.0, 0.0),
            name: "24-P1".into(),
            rotation: 0,
        });
        let plain = wire(&[(0.0, 50.0), (10.0, 50.0)]);
        let plain_id = plain.id();
        for e in [labeled, label, plain] {
            sheet.entities.insert(e.id(), e);
        }
        let nets = extract_netlist(&sheet, &builtin_symbols());
        let labeled_net = nets.iter().find(|n| n.label.is_some()).unwrap();
        assert_eq!(labeled_net.name, "24-P1", "ラベルが線番に勝つ");
        assert_eq!(labeled_net.wire_no.as_deref(), Some("12"), "線番自体は保持される");
        let plain_net = nets.iter().find(|n| n.wire_ids == vec![plain_id]).unwrap();
        assert_eq!(plain_net.name, "N001", "名前も線番も無ければ自動名");
    }

    /// Pin positions honor the symbol's rotation (clockwise in the Y-down paper coordinate system) and placement.
    /// ピン座標はシンボルの回転(Y下向き座標系で時計回り)と配置位置を反映する。
    #[test]
    fn pin_positions_apply_rotation_and_translation() {
        let def = builtin_symbols()
            .into_iter()
            .find(|s| s.id == "resistor")
            .unwrap();
        // resistorのピン: ("1", (-7.5, 0)), ("2", (7.5, 0))
        let inst = SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "resistor".into(),
            at: Point::new(100.0, 50.0),
            rotation: 90,
            mirror: false,
            reference: "R1".into(),
            value: String::new(),
            attrs: Default::default(),
        };
        let pins = pin_positions(&inst, &def);
        // 90度回転(時計回り、Y下向き座標系): (x,y) -> (-y, x)
        assert!((pins[0].1.x - 100.0).abs() < 1e-9);
        assert!((pins[0].1.y - (50.0 - 7.5)).abs() < 1e-9);
        assert!((pins[1].1.x - 100.0).abs() < 1e-9);
        assert!((pins[1].1.y - (50.0 + 7.5)).abs() < 1e-9);
    }

    /// Mirroring flips pins across the vertical axis before rotation is applied.
    /// ミラーは回転より先に、縦軸に対してピンを反転する。
    #[test]
    fn pin_positions_apply_mirror_before_rotation() {
        let def = builtin_symbols()
            .into_iter()
            .find(|s| s.id == "resistor")
            .unwrap();
        let inst = SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "resistor".into(),
            at: Point::new(10.0, 20.0),
            rotation: 0,
            mirror: true,
            reference: "R2".into(),
            value: String::new(),
            attrs: Default::default(),
        };
        let pins = pin_positions(&inst, &def);
        // ミラーでピン1(-7.5,0)は(+7.5,0)へ
        assert!((pins[0].1.x - 17.5).abs() < 1e-9);
        assert!((pins[0].1.y - 20.0).abs() < 1e-9);
    }

    /// An arc inside a symbol turns with the symbol, so the contactor's half circle keeps facing its moving contact at every rotation, and mirroring flips it without reversing the drawing direction.
    /// シンボル内の弧はシンボルと一緒に回るので、電磁接触器の半円はどの回転角でも可動接点の側を向き続け、ミラーでは描画方向を保ったまま左右が入れ替わる。
    #[test]
    fn arc_angles_follow_symbol_rotation_and_mirror() {
        // 下向きに開いた椀 (0→180) を90度ずつ回すと、開く向きも90度ずつ回る
        assert_eq!(rotate_arc_angles(0.0, 180.0, 0, false), (0.0, 180.0));
        assert_eq!(rotate_arc_angles(0.0, 180.0, 90, false), (90.0, 270.0));
        assert_eq!(rotate_arc_angles(0.0, 180.0, 180, false), (180.0, 360.0));
        assert_eq!(rotate_arc_angles(0.0, 180.0, 270, false), (270.0, 450.0));
        // 左右反転: 下向きの椀はそのまま、右向きの半円 (270→450) は左向きになる
        assert_eq!(rotate_arc_angles(0.0, 180.0, 0, true), (0.0, 180.0));
        assert_eq!(rotate_arc_angles(270.0, 450.0, 0, true), (90.0, 270.0));
    }
}
