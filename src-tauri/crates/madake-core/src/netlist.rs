//! ネットリスト抽出: 配置済みシンボルのピン座標解決と、配線・ジャンクション・
//! ネットラベルからの接続グラフ導出。

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::geometry::Point;
use crate::model::{Cable, CableCore, CoreEnd, Entity, EntityId, Sheet, SymbolInstance};
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
    /// このネットで運ぶポートの接点 (ポートのピン・かみ合わせの片側だけ。例 "VBUS")。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact: Option<String>,
}

/// 電気的に接続された1ネット。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Net {
    /// 表示名。優先順は ネットラベル > 線番 > 自動名 (N001…)。ポートの接点のネットは「<名前>.<接点>」。
    pub name: String,
    pub pins: Vec<NetPin>,
    pub wire_ids: Vec<EntityId>,
    /// ネットラベル由来の名前 (手動指定)。接点のネットでは「<ラベル>.<接点>」。
    #[serde(default)]
    pub label: Option<String>,
    /// 線番 (`Wire::net`)。ネット内の全Wireで同値になる。
    #[serde(default)]
    pub wire_no: Option<String>,
    /// このネットを通る区間ケーブルの芯 (v17)。ケーブルid・参照記号・芯の呼びの順に並ぶ。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cable_cores: Vec<NetCableCore>,
    /// ポートから分けたネットなら、その接点 (v18。例 "VBUS")。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact: Option<String>,
    /// ポートから分けたネットが、シートを跨いで呼ばれる名前 (ポートのラベルは「L.接点」、導体1本の部分の
    /// ラベルはそのまま)。空なら [`crate::xref`] は今までどおりシートの上でラベルを探す。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contact_labels: Vec<String>,
}

impl std::fmt::Display for NetPin {
    /// `REF:PIN`、接点を運ぶなら `REF:PIN/接点` (例 `39:U3/VBUS`)。
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.reference, self.pin)?;
        match &self.contact {
            Some(c) => write!(f, "/{c}"),
            None => Ok(()),
        }
    }
}

/// ネットを通る区間ケーブルの芯1本。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NetCableCore {
    pub cable_id: EntityId,
    /// ケーブルの参照記号 (例 "W45")。
    pub reference: String,
    /// 芯の呼び (例 "D-")。
    pub core: String,
}

/// ポート (接点を持つピン) のまわりで ERC が知らせること ([`crate::port::check_ports`])。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PortReport {
    /// ポートのかたまりに載っているネットラベルの名前。
    pub port_labels: BTreeSet<String>,
    /// ポートのかたまりの中で、どの接点とも名前が合わない普通のピン。
    pub plain_pins: Vec<PlainOnPort>,
    /// ポートのピンが2本以上あるかたまりで、1本にしか無い接点。
    pub lonely_contacts: Vec<LonelyContact>,
    /// 接点を決められない区間ケーブルの芯の端。
    pub stray_cores: Vec<StrayCore>,
}

/// ポートのネットにいる、接点の分からない普通のピン。
#[derive(Debug, Clone, PartialEq)]
pub struct PlainOnPort {
    pub entity_id: EntityId,
    /// "J45:1"
    pub pin: String,
    /// 同じかたまりのポートのピン ("39:U3")。
    pub ports: Vec<String>,
    /// そのかたまりの接点の並び。
    pub contacts: Vec<String>,
}

/// 1本のポートのピンにしか無い接点。
#[derive(Debug, Clone, PartialEq)]
pub struct LonelyContact {
    pub contact: String,
    /// "39:U3"
    pub port: String,
    pub entity_ids: Vec<EntityId>,
}

/// 接点を決められない区間ケーブルの芯。
#[derive(Debug, Clone, PartialEq)]
pub struct StrayCore {
    pub cable_id: EntityId,
    pub reference: String,
    pub core: String,
    pub why: String,
}

/// 配置済みピン1本 (ネットリストのノード)。
struct PinNode {
    entity_id: EntityId,
    reference: String,
    no: String,
    /// ピン名 (前後の空白を落としたもの)。ポートの接点と名前で合わせる。
    name: String,
    /// ポートの接点。空 = 普通のピン。
    contacts: Vec<String>,
    /// かみ合わせの片側のピン (ポートの接点を全部素通しにする)。
    pass: bool,
    pos: Point,
}

impl PinNode {
    fn label(&self) -> String {
        format!("{}:{}", self.reference, self.no)
    }
}

/// かたまり (芯を入れずに作った接続のまとまり) ごとの枠の割り振り。導体1本のかたまりは枠1つ、
/// ポートのかたまりは接点ごとに1つと残りの枠1つ (最後)。
struct Slots {
    first: BTreeMap<usize, usize>,
    contacts: BTreeMap<usize, Vec<String>>,
    count: usize,
}

impl Slots {
    fn new(roots: &[usize], contacts: BTreeMap<usize, Vec<String>>) -> Self {
        let mut first = BTreeMap::new();
        let mut count = 0;
        for &r in roots {
            if let std::collections::btree_map::Entry::Vacant(v) = first.entry(r) {
                v.insert(count);
                count += contacts.get(&r).map_or(1, |l| l.len() + 1);
            }
        }
        Self { first, contacts, count }
    }

    fn is_port(&self, root: usize) -> bool {
        self.contacts.contains_key(&root)
    }

    /// 導体1本のかたまりならその枠、ポートのかたまりなら残りの枠。
    fn single_or_residual(&self, root: usize) -> usize {
        self.first[&root] + self.contacts.get(&root).map_or(0, Vec::len)
    }

    fn contact(&self, root: usize, name: &str) -> Option<usize> {
        let i = self.contacts.get(&root)?.iter().position(|c| c == name)?;
        Some(self.first[&root] + i)
    }

    /// 配線・ラベルが入る枠 (導体1本ならその枠、ポートなら接点の枠全部と残りの枠)。
    fn every(&self, root: usize) -> std::ops::RangeInclusive<usize> {
        self.first[&root]..=self.single_or_residual(root)
    }
}

/// 線番の選び方: 手動の名前 (数字以外) を自動採番より先に、その次は辞書順。
fn pick_wire_no(nos: &[String]) -> Option<String> {
    nos.iter()
        .min_by(|a, b| {
            (crate::wire_no::is_generated_number(a), *a).cmp(&(crate::wire_no::is_generated_number(b), *b))
        })
        .cloned()
}

fn stray(report: &mut PortReport, c: &Cable, core: &CableCore, why: String) {
    report.stray_cores.push(StrayCore { cable_id: c.id, reference: c.reference.clone(), core: core.core.clone(), why });
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
pub fn on_wire(w: &crate::model::Wire, p: &Point) -> bool {
    w.points
        .windows(2)
        .any(|seg| segment_distance(p, &seg[0], &seg[1]) < CONNECT_EPS)
}

/// ネットラベルが図面に**載っている**か = ラベルの位置が配線の線分上 (頂点を含む) か、
/// シンボルのピンにあるか。
///
/// ネットリスト ([`extract_netlist`]) がラベルをネットへ繋ぐ判定と、ERCの浮きラベル判定
/// ([`crate::verify`] の `erc.dangling_label`) は**この関数を共有する**。
/// 判定が2か所に分かれていると「ERCは黙っているのに繋がっていない」ラベルが生まれるため。
pub fn label_is_attached(sheet: &Sheet, symbols: &[SymbolDef], at: &Point) -> bool {
    let defs = crate::symbol::DefIndex::new(symbols);
    sheet.entities.values().any(|entity| match entity {
        Entity::Wire(w) => on_wire(w, at),
        Entity::Symbol(s) => defs
            .get(s)
            .is_some_and(|def| pin_positions(s, &def).iter().any(|(_, pos)| near(pos, at))),
        _ => false,
    })
}

/// 点`at`に**電気的な接続物が載っているか** (座標一致の判定)。
///
/// [`extract_netlist`]がネットを繋ぐのと同じ4通りを見る: 配線の線分上 ([`on_wire`]。
/// 端点だけでなく途中も含む)・ジャンクション・ネットラベル・**他のシンボルのピン**。
/// `exclude`に渡したエンティティ (通常は判定対象の端子台そのもの) は無視するので、
/// 「自分のピンがそこにある」ことは接続とは数えない。
///
/// 端子台チャートの`spare`は配線 (`Entity::Wire`) だけを数えるので、ネットラベルや
/// ジャンクションだけが載っている端子を「予備」と見てしまう。段数を減らす
/// ([`crate::command::Engine`]の`SetTerminalLevels`) ときの拒否判定は、
/// **図面から消えるピンに何か繋がっていないか**をこの関数で見る。
pub fn point_is_attached(
    sheet: &Sheet,
    symbols: &[SymbolDef],
    at: &Point,
    exclude: Option<EntityId>,
) -> bool {
    let defs = crate::symbol::DefIndex::new(symbols);
    sheet.entities.values().any(|entity| {
        if Some(entity.id()) == exclude {
            return false;
        }
        match entity {
            Entity::Wire(w) => on_wire(w, at),
            Entity::Junction(j) => near(&j.at, at),
            Entity::NetLabel(l) => near(&l.at, at),
            Entity::Symbol(s) => defs
                .get(s)
                .is_some_and(|def| pin_positions(s, &def).iter().any(|(_, pos)| near(pos, at))),
            _ => false,
        }
    })
}

/// シートからネットリストを導出する (ポートは接点ごとに分ける)。
///
/// 接続ルール:
/// - Wire同士は「端点同士の一致」または「一致点上のJunction経由」でのみ接続(交差だけでは非接続)
/// - シンボルピン・NetLabelはWireの任意の頂点との一致で接続
/// - 同名NetLabelのネットは統合され、ネット名はラベル名(辞書順最小)になる
/// - 区間ケーブル ([`crate::model::Cable::run`]) の芯は、芯の行き先 ([`CoreEnd`]) が指すピン・配線と
///   だけ繋がる。区間の線は位置では何にも繋がらない (ピン列の上を通っても短絡しない)
/// - 配線か区間ケーブルの芯を1つ以上含むまとまりをネットにする。芯だけのネットは配線のあるネットの後ろ
/// - 無名ネットは決定的な順序で "N001" から連番
/// - **ポート** (接点を持つピン、[`crate::symbol::PinDef::contacts`]) を含むまとまりは接点ごとのネット
///   「<名前>.<接点>」に分かれる。配線・ラベル・かみ合わせの片側は全部の接点を運び、普通のピンは
///   ピン名と同じ接点に入る (仕様 2026-10-25-port-contacts-design §2)。ポートが無ければ
///   [`extract_wire_nets`] と全く同じ
pub fn extract_netlist(sheet: &Sheet, symbols: &[SymbolDef]) -> Vec<Net> {
    build(sheet, symbols, true).0
}

/// 描いた配線のまとまりごとに1ネットのネットリスト (ポートを分けない、v17までの [`extract_netlist`])。
/// 線番の採番・線番の描画・電線リストのように、配線1本を1件として扱う所が使う。
pub fn extract_wire_nets(sheet: &Sheet, symbols: &[SymbolDef]) -> Vec<Net> {
    build(sheet, symbols, false).0
}

/// ポートのまわりの ERC の材料 ([`crate::port::check_ports`])。
pub(crate) fn port_report(sheet: &Sheet, symbols: &[SymbolDef]) -> PortReport {
    build(sheet, symbols, true).1
}

fn build(sheet: &Sheet, symbols: &[SymbolDef], split: bool) -> (Vec<Net>, PortReport) {
    let defs = crate::symbol::DefIndex::new(symbols);

    // ノード列挙 (BTreeMap順なので決定的)
    let mut wires = Vec::new();
    let mut junctions = Vec::new();
    let mut labels = Vec::new();
    let mut cables = Vec::new();
    let mut pins: Vec<PinNode> = Vec::new();
    for entity in sheet.entities.values() {
        match entity {
            Entity::Wire(w) => wires.push(w),
            Entity::Junction(j) => junctions.push(j),
            Entity::NetLabel(l) => labels.push(l),
            Entity::Cable(c) if c.is_run() => cables.push(c),
            Entity::Symbol(s) => {
                if let Some(def) = defs.get(s) {
                    let pass = crate::mate::is_mate_half(&s.symbol_id);
                    for pin in &def.pins {
                        pins.push(PinNode {
                            entity_id: s.id,
                            reference: s.reference.clone(),
                            no: pin.number.clone(),
                            name: pin.name.trim().to_string(),
                            contacts: pin.contacts.clone(),
                            pass,
                            pos: transform_local(pin.at, s),
                        });
                    }
                }
            }
            // 注記・図形・ハーネス境界・盤レイアウトの要素は電気的な接続を持たない
            Entity::Text(_)
            | Entity::Shape(_)
            | Entity::Harness(_)
            | Entity::PanelOutline(_)
            | Entity::DinRail(_)
            | Entity::Duct(_)
            | Entity::Footprint(_)
            | Entity::PanelLegend(_)
            | Entity::WireLegend(_)
            | Entity::Cable(_)
            | Entity::Nameplate(_)
            | Entity::Dimension(_) => {}
        }
    }

    // ノード番号: [wires][pins][junctions][labels]。芯はかたまりを作った後で枠どうしを結ぶ
    let wi = 0;
    let pi = wi + wires.len();
    let ji = pi + pins.len();
    let li = ji + junctions.len();
    let ci = li + labels.len();
    let cores: Vec<(&Cable, &CableCore)> = cables
        .iter()
        .flat_map(|c| c.cores.iter().map(move |k| (*c, k)))
        .collect();
    let mut ds = DisjointSet::new(ci);

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
    for (p, node) in pins.iter().enumerate() {
        for (a, w) in wires.iter().enumerate() {
            if on_wire(w, &node.pos) {
                ds.union(pi + p, wi + a);
            }
        }
    }
    // 同一シンボル内の同一ピン番号の接続点は内部短絡(端子台の左右貫通)
    for (p, a) in pins.iter().enumerate() {
        for (p2, b) in pins.iter().enumerate().skip(p + 1) {
            if a.entity_id == b.entity_id && a.no == b.no {
                ds.union(pi + p, pi + p2);
            }
        }
    }
    // ラベル ⇔ Wire上の任意点 / シンボルピン、同名ラベル同士
    // (載り判定は[`label_is_attached`]と同じ2つの述語。ERCの浮きラベル判定と揃える)
    for (l, lab) in labels.iter().enumerate() {
        for (a, w) in wires.iter().enumerate() {
            if on_wire(w, &lab.at) {
                ds.union(li + l, wi + a);
            }
        }
        for (p, node) in pins.iter().enumerate() {
            if near(&node.pos, &lab.at) {
                ds.union(li + l, pi + p);
            }
        }
        for (l2, lab2) in labels.iter().enumerate().skip(l + 1) {
            if lab.name == lab2.name {
                ds.union(li + l, li + l2);
            }
        }
    }

    // かたまり (芯を入れない) と、ポートのかたまりの接点の並び
    let roots: Vec<usize> = (0..ci).map(|i| ds.find(i)).collect();
    let mut port_contacts: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    if split {
        let mut order: Vec<usize> = (0..pins.len()).filter(|&p| !pins[p].contacts.is_empty()).collect();
        order.sort_by(|&a, &b| {
            (&pins[a].reference, &pins[a].no, pins[a].entity_id).cmp(&(&pins[b].reference, &pins[b].no, pins[b].entity_id))
        });
        for p in order {
            let list = port_contacts.entry(roots[pi + p]).or_default();
            for c in &pins[p].contacts {
                if !list.contains(c) {
                    list.push(c.clone());
                }
            }
        }
    }
    let slots = Slots::new(&roots, port_contacts);
    // 枠どうしを結ぶ。枠番号 slots.count + k が芯 k
    let mut sd = DisjointSet::new(slots.count + cores.len());
    let mut report = PortReport::default();

    // ピン p が入る枠と、そこで運ぶ接点
    let pin_slots = |p: usize| -> Vec<(usize, Option<String>)> {
        let node = &pins[p];
        let r = roots[pi + p];
        let Some(list) = slots.contacts.get(&r) else {
            return vec![(slots.first[&r], None)];
        };
        if !node.contacts.is_empty() {
            node.contacts.iter().filter_map(|c| slots.contact(r, c).map(|s| (s, Some(c.clone())))).collect()
        } else if node.pass {
            list.iter().enumerate().map(|(i, c)| (slots.first[&r] + i, Some(c.clone()))).collect()
        } else {
            vec![(slots.contact(r, &node.name).unwrap_or_else(|| slots.single_or_residual(r)), None)]
        }
    };

    // 芯 ⇔ 行き先 (割当てだけで結ぶ。区間の線はどの位置判定にも出てこない)
    for (k, (c, core)) in cores.iter().enumerate() {
        let me = slots.count + k;
        for end in [&core.end_a, &core.end_b].into_iter().flatten() {
            let target = match end {
                CoreEnd::Wire { wire } => match wires.iter().position(|w| w.id == *wire) {
                    Some(a) => {
                        let r = roots[wi + a];
                        if slots.is_port(r) {
                            stray(&mut report, c, core, "分岐の配線がポートのネットに触れています".into());
                        }
                        Some(slots.single_or_residual(r))
                    }
                    None => None,
                },
                // 同じ番号の接続点は内部短絡済みなので、最初の1つを見れば足りる (index は描画用)
                CoreEnd::Pin { part, pin, contact, .. } => match pins.iter().position(|n| n.entity_id == *part && n.no == *pin) {
                    None => None,
                    Some(p) => {
                        let node = &pins[p];
                        let r = roots[pi + p];
                        // 指定の接点がこのピンに無い (接点をすべて消したピンも含む)。かみ合わせの片側は下で扱う
                        let missing = contact
                            .as_ref()
                            .filter(|x| !node.contacts.contains(x) && !(node.pass && node.contacts.is_empty() && slots.is_port(r)));
                        if let Some(x) = missing {
                            let list = if node.contacts.is_empty() { "なし".to_string() } else { node.contacts.join(", ") };
                            stray(&mut report, c, core, format!("接点 {x} は {} にありません (接点: {list})", node.label()));
                            Some(slots.single_or_residual(r))
                        } else if !slots.is_port(r) {
                            Some(slots.first[&r])
                        } else if node.contacts.is_empty() && node.pass {
                            stray(&mut report, c, core, format!("{} はかみ合わせの片側なので、どの接点か決められません", node.label()));
                            Some(slots.single_or_residual(r))
                        } else if node.contacts.is_empty() {
                            Some(pin_slots(p)[0].0)
                        } else {
                            let want = contact.clone().unwrap_or_else(|| core.core.trim().to_string());
                            let found = if node.contacts.contains(&want) { slots.contact(r, &want) } else { None };
                            match found {
                                Some(s) => Some(s),
                                None => {
                                    // 接点の指定があればここに来る前に報告済み
                                    let why = format!(
                                        "{} はポート (接点: {}) ですが、芯に接点の指定が無く、芯の名前もどの接点でもありません",
                                        node.label(),
                                        node.contacts.join(", ")
                                    );
                                    stray(&mut report, c, core, why);
                                    Some(slots.single_or_residual(r))
                                }
                            }
                        }
                    }
                },
            };
            if let Some(s) = target {
                sd.union(me, s);
            }
        }
    }

    // 点付きラベル: 導体1本のかたまりの「L.接点」は、ラベル L のポートのかたまりのその接点の枠へ
    if !slots.contacts.is_empty() {
        let port_labels: Vec<(&str, usize)> = labels
            .iter()
            .enumerate()
            .filter(|(l, _)| slots.is_port(roots[li + l]))
            .map(|(l, lab)| (lab.name.as_str(), roots[li + l]))
            .collect();
        for (l, lab) in labels.iter().enumerate() {
            let r = roots[li + l];
            if slots.is_port(r) {
                continue;
            }
            for (name, pr) in &port_labels {
                for c in &slots.contacts[pr] {
                    if lab.name == format!("{name}.{c}") {
                        if let Some(s) = slots.contact(*pr, c) {
                            sd.union(slots.first[&r], s);
                        }
                    }
                }
            }
        }
    }

    // グループ化 (配線か区間ケーブルの芯を1つ以上含むグループだけをネットにする)
    #[derive(Default)]
    struct Group {
        wire_ids: Vec<EntityId>,
        pins: Vec<NetPin>,
        /// 名前の候補になるラベル (ポートの接点の枠では「L.接点」)。
        labels: BTreeSet<String>,
        /// グループ内のWireに書かれた生の線番 (重複可)。
        wire_nos: Vec<String>,
        /// 名前の候補になる線番 (ポートの接点の枠では「W.接点」)。
        name_wire_nos: Vec<String>,
        cable_cores: Vec<NetCableCore>,
        /// このまとまりのポートの枠: (かたまりの根, 接点の順位, 接点名。残りの枠は None)。
        ports: Vec<(usize, usize, Option<String>)>,
    }
    let mut groups: BTreeMap<usize, Group> = BTreeMap::new();
    let mut cluster_nos: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for (a, w) in wires.iter().enumerate() {
        let r = roots[wi + a];
        let no = w.net.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty());
        if let Some(no) = no {
            cluster_nos.entry(r).or_default().push(no.to_string());
        }
        for s in slots.every(r) {
            let g = groups.entry(sd.find(s)).or_default();
            g.wire_ids.push(w.id);
            if let Some(no) = no {
                g.wire_nos.push(no.to_string());
                if !slots.is_port(r) {
                    g.name_wire_nos.push(no.to_string());
                }
            }
        }
    }
    for (k, (c, core)) in cores.iter().enumerate() {
        groups.entry(sd.find(slots.count + k)).or_default().cable_cores.push(NetCableCore {
            cable_id: c.id,
            reference: c.reference.clone(),
            core: core.core.clone(),
        });
    }
    for (p, node) in pins.iter().enumerate() {
        for (s, contact) in pin_slots(p) {
            if let Some(g) = groups.get_mut(&sd.find(s)) {
                // 同一シンボル同一番号の接続点(内部短絡済み)は1エントリに集約
                if !g.pins.iter().any(|np| np.entity_id == node.entity_id && np.pin == node.no && np.contact == contact) {
                    g.pins.push(NetPin {
                        reference: node.reference.clone(),
                        entity_id: node.entity_id,
                        pin: node.no.clone(),
                        contact,
                    });
                }
            }
        }
    }
    for (l, lab) in labels.iter().enumerate() {
        let r = roots[li + l];
        match slots.contacts.get(&r) {
            None => {
                if let Some(g) = groups.get_mut(&sd.find(slots.first[&r])) {
                    g.labels.insert(lab.name.clone());
                }
            }
            Some(list) => {
                for (i, c) in list.iter().enumerate() {
                    if let Some(g) = groups.get_mut(&sd.find(slots.first[&r] + i)) {
                        g.labels.insert(format!("{}.{c}", lab.name));
                    }
                }
                if let Some(g) = groups.get_mut(&sd.find(slots.single_or_residual(r))) {
                    g.labels.insert(lab.name.clone());
                }
            }
        }
    }
    for (&r, list) in &slots.contacts {
        let base_no = pick_wire_no(cluster_nos.get(&r).map(Vec::as_slice).unwrap_or_default());
        for (i, c) in list.iter().enumerate() {
            if let Some(g) = groups.get_mut(&sd.find(slots.first[&r] + i)) {
                g.ports.push((r, i, Some(c.clone())));
                if let Some(w) = &base_no {
                    g.name_wire_nos.push(format!("{w}.{c}"));
                }
            }
        }
        if let Some(g) = groups.get_mut(&sd.find(slots.single_or_residual(r))) {
            g.ports.push((r, list.len(), None));
            if let Some(w) = &base_no {
                g.name_wire_nos.push(w.clone());
            }
        }
    }
    // 普通のピンも芯も無い残りの枠は出さない (共有の配線は接点のネット全部に入っている)
    groups.retain(|_, g| {
        let residual_only = !g.ports.is_empty() && g.ports.iter().all(|(_, _, c)| c.is_none());
        !(residual_only && g.pins.is_empty() && g.cable_cores.is_empty())
    });

    // ポートの報告
    if !slots.contacts.is_empty() {
        let mut port_pins_of: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (p, node) in pins.iter().enumerate() {
            let r = roots[pi + p];
            if slots.is_port(r) && !node.contacts.is_empty() {
                let list = port_pins_of.entry(r).or_default();
                if !list.iter().any(|&q| pins[q].entity_id == node.entity_id && pins[q].no == node.no) {
                    list.push(p);
                }
            }
        }
        for (l, lab) in labels.iter().enumerate() {
            if slots.is_port(roots[li + l]) {
                report.port_labels.insert(lab.name.clone());
            }
        }
        for (p, node) in pins.iter().enumerate() {
            let r = roots[pi + p];
            let Some(list) = slots.contacts.get(&r) else { continue };
            let label = node.label();
            if node.contacts.is_empty()
                && !node.pass
                && !list.contains(&node.name)
                && !report.plain_pins.iter().any(|x| x.entity_id == node.entity_id && x.pin == label)
            {
                report.plain_pins.push(PlainOnPort {
                    entity_id: node.entity_id,
                    pin: label,
                    ports: port_pins_of.get(&r).map(|v| v.iter().map(|&q| pins[q].label()).collect()).unwrap_or_default(),
                    contacts: list.clone(),
                });
            }
        }
        for (&r, list) in &port_pins_of {
            if list.len() < 2 {
                continue;
            }
            for c in &slots.contacts[&r] {
                let holders: Vec<usize> = list.iter().copied().filter(|&q| pins[q].contacts.contains(c)).collect();
                if let [only] = holders.as_slice() {
                    report.lonely_contacts.push(LonelyContact {
                        contact: c.clone(),
                        port: pins[*only].label(),
                        entity_ids: vec![pins[*only].entity_id],
                    });
                }
            }
        }
    }

    // 決定的順序: 配線のあるネットが先 (最小wire id)、芯だけのネットはその後ろ、同じかたまりの接点は接点の順
    let mut ordered: Vec<Group> = groups.into_values().collect();
    for g in &mut ordered {
        g.ports.sort();
    }
    ordered.sort_by_key(|g| {
        (
            g.wire_ids.is_empty(),
            g.wire_ids.iter().min().copied(),
            g.cable_cores.iter().map(|c| (c.cable_id, c.core.clone())).min(),
            g.ports.first().map_or(0, |p| p.1),
        )
    });
    let mut seq = 0;
    // ポートのかたまりの根 → (付けた順の番号, 元の名前 Nxxx)
    let mut port_base: BTreeMap<usize, (usize, String)> = BTreeMap::new();
    let mut nets = Vec::with_capacity(ordered.len());
    for mut g in ordered {
        g.wire_ids.sort();
        g.pins.sort_by(|a, b| (&a.reference, &a.pin, &a.contact).cmp(&(&b.reference, &b.pin, &b.contact)));
        g.cable_cores.sort_by(|a, b| (a.cable_id, &a.core).cmp(&(b.cable_id, &b.core)));
        let wire_no = pick_wire_no(&g.wire_nos);
        let label = g.labels.iter().next().cloned();
        let contact = g.ports.iter().find_map(|(_, _, c)| c.clone());
        let name = match label.clone().or_else(|| pick_wire_no(&g.name_wire_nos)) {
            Some(n) => n,
            None => match g.ports.first() {
                Some((first, ..)) => {
                    // 芯で2つ以上のポートのかたまりが繋がったら、ネットの順で先のかたまりの元の名前を使う
                    // (既に付いた名前があればそれ。新しい番号は使わない。仕様 §2.3)
                    let earlier = g.ports.iter().filter_map(|(r, ..)| port_base.get(r)).min_by_key(|(n, _)| *n);
                    let base = match earlier {
                        Some((_, b)) => b.clone(),
                        None => {
                            seq += 1;
                            let b = format!("N{seq:03}");
                            port_base.insert(*first, (seq, b.clone()));
                            b
                        }
                    };
                    match &contact {
                        Some(c) => format!("{base}.{c}"),
                        None => base,
                    }
                }
                None => {
                    seq += 1;
                    format!("N{seq:03}")
                }
            },
        };
        let contact_labels = if g.ports.is_empty() { Vec::new() } else { g.labels.iter().cloned().collect() };
        nets.push(Net {
            name,
            pins: g.pins,
            wire_ids: g.wire_ids,
            label,
            wire_no,
            cable_cores: g.cable_cores,
            contact,
            contact_labels,
        });
    }
    (nets, report)
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
            length_source: Default::default(),
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

    /// A wire that meets a block pin joins that pin to the net under the pin's own name, not a number.
    /// ブロックのピンに届いた配線は、そのピンを番号ではなくピン名(例 "OUT1")でネットに参加させる。
    #[test]
    fn block_pins_join_nets_under_their_own_names() {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        let block = Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: crate::block::BLOCK_SYMBOL_ID.into(),
            at: Point::new(100.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "CTRL1".into(),
            value: "MainController".into(),
            attrs: [(
                "block_pins".to_string(),
                r#"{"left":[{"no":"IN1","name":"入力1"}],"right":[{"no":"OUT1","name":"出力1"}]}"#
                    .to_string(),
            )]
            .into_iter()
            .collect(),
        });
        // 幅20mm・左右1ピンずつ → 右ピンは (112.5, 50)。R1のピン1 (x=142.5) へ配線
        for e in [
            block,
            symbol("resistor", "R1", 150.0, 50.0),
            wire(&[(112.5, 50.0), (142.5, 50.0)]),
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let nets = extract_netlist(&sheet, &crate::symbol::sheet_symbol_defs(&sheet));
        let net = nets.iter().find(|n| n.pins.len() == 2).expect("接続ネット");
        let mut refs: Vec<_> = net
            .pins
            .iter()
            .map(|p| format!("{}:{}", p.reference, p.pin))
            .collect();
        refs.sort();
        assert_eq!(refs, vec!["CTRL1:OUT1", "R1:1"]);
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
            name: "N-P1".into(),
            rotation: 0,
            attrs: Default::default(),
        });
        for e in [w1, w2, label] {
            sheet.entities.insert(e.id(), e);
        }
        let nets = extract_netlist(&sheet, &builtin_symbols());
        assert_eq!(nets.len(), 2, "交差のみでは接続しない");
        assert!(nets.iter().any(|n| n.name == "N-P1"));
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
            attrs: Default::default(),
        });
        let l2 = Entity::NetLabel(NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(100.0, 100.0),
            name: "48-P1".into(),
            rotation: 0,
            attrs: Default::default(),
        });
        for e in [w1, w2, j, w3, l1, l2] {
            sheet.entities.insert(e.id(), e);
        }
        let nets = extract_netlist(&sheet, &builtin_symbols());
        assert_eq!(nets.len(), 1, "ジャンクション+同名ラベルで全て1ネット");
        assert_eq!(nets[0].name, "48-P1");
        assert_eq!(nets[0].wire_ids.len(), 3);
    }

    /// A net label placed exactly on a symbol pin connects that pin to the net of the same name elsewhere, without a wire stub.
    /// シンボルのピンにぴったり置いたネットラベルは、配線を引かなくても同じ名前のネットへそのピンを繋ぐ。
    #[test]
    fn a_label_on_a_pin_joins_that_pin_to_the_net_of_the_same_name() {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        // R1のピン2 (x=107.5) の上にラベル。離れた場所の同名ラベルは配線に載っている
        let label_on_pin = Entity::NetLabel(NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(107.5, 50.0),
            name: "24V".into(),
            rotation: 0,
            attrs: Default::default(),
        });
        let label_on_wire = Entity::NetLabel(NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(0.0, 0.0),
            name: "24V".into(),
            rotation: 0,
            attrs: Default::default(),
        });
        for e in [
            symbol("resistor", "R1", 100.0, 50.0),
            wire(&[(0.0, 0.0), (20.0, 0.0)]),
            label_on_pin,
            label_on_wire,
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let nets = extract_netlist(&sheet, &builtin_symbols());
        let net = nets.iter().find(|n| n.name == "24V").expect("24Vのネット");
        assert!(
            net.pins.iter().any(|p| p.reference == "R1" && p.pin == "2"),
            "ピンに置いたラベルでR1:2が繋がる: {net:?}"
        );
    }

    /// The attachment test used by the netlist accepts a label on a wire vertex, in the middle of a wire segment, or on a symbol pin, and rejects one that floats even slightly off.
    /// ネットリストが使う載り判定は、配線の頂点上・線分の途中・シンボルのピン上のラベルを載っていると認め、わずかでも浮いたラベルは認めない。
    #[test]
    fn the_label_attachment_test_accepts_vertices_segments_and_pins() {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        for e in [
            symbol("resistor", "R1", 100.0, 50.0),
            wire(&[(0.0, 0.0), (20.0, 0.0)]),
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let defs = builtin_symbols();
        assert!(label_is_attached(&sheet, &defs, &Point::new(0.0, 0.0)), "頂点上");
        assert!(label_is_attached(&sheet, &defs, &Point::new(10.0, 0.0)), "線分の途中");
        assert!(label_is_attached(&sheet, &defs, &Point::new(107.5, 50.0)), "ピン上");
        assert!(
            label_is_attached(&sheet, &defs, &Point::new(10.0, 0.005)),
            "許容誤差 (0.01mm) の内側は載っている扱い"
        );
        assert!(!label_is_attached(&sheet, &defs, &Point::new(10.0, 0.5)), "0.5mm浮き");
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
                    contacts: Vec::new(),
                },
                crate::symbol::PinDef {
                    number: "1".into(),
                    name: String::new(),
                    at: Point::new(2.5, 0.0),
                    dir: crate::symbol::PinDir::Right,
                    contacts: Vec::new(),
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

    /// A multi-level terminal block never shorts its levels together: level a and level b of the same pole end up on two separate nets, even though each level's own left and right connection points are still shorted internally (through-terminal).
    /// 多段端子台は段どうしを決して短絡しない: 同じ極でも段aと段bは別々のネットになる(各段自身の左右接続点は貫通端子として今までどおり内部短絡される)。
    #[test]
    fn each_level_of_a_multilevel_terminal_block_is_an_independent_net() {
        let def = crate::symbol::resolve_symbol("terminal_block_1p_2l").expect("多段端子台");
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        // TB1 (100,50): 1極2段なので段aの列は x=92.5/97.5、段bの列は x=102.5/107.5 (同じ行 y=50)
        for e in [
            symbol("terminal_block_1p_2l", "TB1", 100.0, 50.0),
            wire(&[(80.0, 50.0), (92.5, 50.0)]),   // 1a 左 (外部側)
            wire(&[(97.5, 50.0), (97.5, 30.0)]),   // 1a 右 (内部側、列の間を上へ抜ける)
            wire(&[(102.5, 50.0), (102.5, 30.0)]), // 1b 左 (列の間を上へ抜ける)
            wire(&[(107.5, 50.0), (120.0, 50.0)]), // 1b 右
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let nets = extract_netlist(&sheet, &[def]);
        assert_eq!(nets.len(), 2, "段a・段bは別ネットになる: {nets:?}");
        let mut pin_names: Vec<String> = nets
            .iter()
            .flat_map(|n| n.pins.iter().map(|p| p.pin.clone()))
            .collect();
        pin_names.sort();
        assert_eq!(pin_names, vec!["1a", "1b"]);
        for net in &nets {
            assert_eq!(net.wire_ids.len(), 2, "各段は左右2本の配線を持つ: {net:?}");
            assert_eq!(net.pins.len(), 1, "各段のピンは貫通端子として1つにまとまる: {net:?}");
        }
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
            name: "N-P1".into(),
            rotation: 0,
            attrs: Default::default(),
        });
        let plain = wire(&[(0.0, 50.0), (10.0, 50.0)]);
        let plain_id = plain.id();
        for e in [labeled, label, plain] {
            sheet.entities.insert(e.id(), e);
        }
        let nets = extract_netlist(&sheet, &builtin_symbols());
        let labeled_net = nets.iter().find(|n| n.label.is_some()).unwrap();
        assert_eq!(labeled_net.name, "N-P1", "ラベルが線番に勝つ");
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

    /// The LTE harness (four board wires breaking out of a single line that lands on the USB plug) gives four separate nets, each joining one board pin to the matching plug pin through its cable core.
    /// LTEハーネス (基板側の配線4本が1本線に分岐し、その線がUSBプラグに着く) は別々の4ネットになり、各ネットは芯を通して基板のピン1本とプラグの同じ番号のピンを結ぶ。
    #[test]
    fn a_run_cable_keeps_each_core_its_own_net() {
        let lte = crate::cable::test_support::lte_sheet();
        let nets = extract_netlist(&lte.sheet, &crate::symbol::sheet_symbol_defs(&lte.sheet));
        assert_eq!(nets.len(), 4, "{nets:?}");
        for (i, (core, _)) in crate::cable::test_support::LTE_CORES.into_iter().enumerate() {
            let pin = (i + 1).to_string();
            let net = nets
                .iter()
                .find(|n| n.cable_cores.iter().any(|c| c.core == core))
                .expect(core);
            let pins: Vec<(String, String)> =
                net.pins.iter().map(|p| (p.reference.clone(), p.pin.clone())).collect();
            assert_eq!(pins, vec![("J45".to_string(), pin.clone()), ("USB1".to_string(), pin)]);
            assert_eq!(net.wire_ids, vec![lte.wires[i]]);
            assert_eq!(net.cable_cores[0].reference, "W45");
        }
    }

    /// A single line lying across every pin of a connector joins none of them: a cable's run never connects by position.
    /// コネクタの全ピンの上を通る1本線でも、どのピンも繋がない (ケーブルの区間は位置では繋がらない)。
    #[test]
    fn a_run_over_a_pin_column_joins_nothing() {
        let mut lte = crate::cable::test_support::lte_sheet();
        let Some(Entity::Cable(c)) = lte.sheet.entities.get_mut(&lte.cable) else { panic!("ケーブル") };
        // USB1 のピン列 (x=192.5、y=92.5〜107.5) に沿って区間を引き直す
        c.run = vec![
            Point::new(80.0, 100.0),
            Point::new(192.5, 100.0),
            Point::new(192.5, 85.0),
            Point::new(192.5, 115.0),
        ];
        let nets = extract_netlist(&lte.sheet, &crate::symbol::sheet_symbol_defs(&lte.sheet));
        assert_eq!(nets.len(), 4);
        assert!(
            nets.iter().all(|n| n.pins.iter().filter(|p| p.reference == "USB1").count() == 1),
            "{nets:?}"
        );
    }

    /// A single-line cable between two connectors (3.5 mm AUX: L, R, GND) gives one net per core even though no wire is drawn.
    /// 2つのコネクタを全長1本線で結ぶケーブル (3.5mm AUX の L・R・GND) は、配線が1本も無くても芯ごとに1ネットになる。
    #[test]
    fn a_connector_to_connector_run_gives_one_net_per_core() {
        let (sheet, _) = crate::cable::test_support::aux_sheet();
        let nets = extract_netlist(&sheet, &crate::symbol::sheet_symbol_defs(&sheet));
        assert_eq!(nets.len(), 3, "{nets:?}");
        for net in &nets {
            assert!(net.wire_ids.is_empty());
            assert_eq!(net.pins.len(), 2);
            assert_eq!(net.pins[0].pin, net.pins[1].pin);
        }
    }

    /// Nets made only of cable cores are named after the nets that have wires, so adding such a cable does not rename existing nets.
    /// 芯だけのネットは配線のあるネットの後ろに名前が付くので、そういうケーブルを足しても今あるネットの名前は変わらない。
    #[test]
    fn core_only_nets_are_named_after_wire_nets() {
        let (mut sheet, _) = crate::cable::test_support::aux_sheet();
        for e in [
            symbol("resistor", "R1", 100.0, 150.0),
            symbol("resistor", "R2", 150.0, 150.0),
            wire(&[(107.5, 150.0), (142.5, 150.0)]),
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let nets = extract_netlist(&sheet, &crate::symbol::sheet_symbol_defs(&sheet));
        assert_eq!(nets[0].wire_ids.len(), 1);
        assert!(nets[1..].iter().all(|n| n.wire_ids.is_empty()));
        assert_eq!(
            nets.iter().map(|n| n.name.as_str()).collect::<Vec<_>>(),
            ["N001", "N002", "N003", "N004"]
        );
    }

    /// Moving the part a single line lands on keeps every core connected to the same pin: connectivity follows the assignment, not the drawing.
    /// 1本線が着く部品を動かしても、どの芯も同じピンに繋がったまま (接続は割当てで決まり、描いた位置では決まらない)。
    #[test]
    fn moving_the_landing_part_keeps_core_connectivity() {
        let mut lte = crate::cable::test_support::lte_sheet();
        lte.sheet.entities.get_mut(&lte.usb).unwrap().translate(0.0, 40.0);
        let nets = extract_netlist(&lte.sheet, &crate::symbol::sheet_symbol_defs(&lte.sheet));
        assert_eq!(nets.len(), 4);
        assert!(nets.iter().all(|n| n.pins.len() == 2 && n.pins[0].pin == n.pins[1].pin), "{nets:?}");
    }

    use crate::block::port_fixtures::{pin_at, port_block, USB};
    use crate::cable::test_support::insert as put;


    /// Hub port 39.U3 and dongle port LTE1.P1 joined by one wire (A3 sheet).
    fn usb_link() -> (Sheet, EntityId, EntityId, EntityId) {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let hub = put(&mut sheet, Entity::Symbol(port_block("39", 100.0, 100.0, "right", "U3", &USB)));
        let dongle = put(&mut sheet, Entity::Symbol(port_block("LTE1", 200.0, 100.0, "left", "P1", &USB)));
        let (a, b) = (pin_at(&sheet, hub, "U3"), pin_at(&sheet, dongle, "P1"));
        let w = put(&mut sheet, wire(&[(a.x, a.y), (b.x, b.y)]));
        (sheet, hub, dongle, w)
    }

    fn pins_of(net: &Net) -> Vec<String> {
        net.pins.iter().map(ToString::to_string).collect()
    }

    /// Two port pins joined by one wire give one net per contact, named N001.<contact> in contact order, each holding both pins with that contact and the shared wire.
    /// 2本のポートのピンを配線1本で結ぶと接点ごとに1ネットになり、名前は接点の順に N001.<接点>、どれにも両方のピンがその接点付きで、共有の配線と一緒に載る。
    #[test]
    fn two_ports_joined_by_one_wire_split_into_one_net_per_contact() {
        let (sheet, _, _, w) = usb_link();
        let nets = extract_netlist(&sheet, &crate::symbol::sheet_symbol_defs(&sheet));
        let names: Vec<&str> = nets.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, vec!["N001.VBUS", "N001.D-", "N001.D+", "N001.GND"]);
        assert_eq!(pins_of(&nets[2]), vec!["39:U3/D+", "LTE1:P1/D+"]);
        assert_eq!(nets[2].contact.as_deref(), Some("D+"));
        assert!(nets.iter().all(|n| n.wire_ids == vec![w]));
    }

    /// The unsplit view keeps one net for the USB wire, and a drawing without ports gives exactly the same nets (and JSON, with no contact keys) from both functions.
    /// 分けない方では USB の配線は1ネットのまま。ポートの無い図面は、どちらの関数でも全く同じネット (JSON も同じで、contact のキーは無い) になる。
    #[test]
    fn without_ports_nothing_changes() {
        let (sheet, ..) = usb_link();
        let defs = crate::symbol::sheet_symbol_defs(&sheet);
        let one = extract_wire_nets(&sheet, &defs);
        assert_eq!(one.len(), 1);
        assert!(one[0].pins.iter().all(|p| p.contact.is_none()));
        let lte = crate::cable::test_support::lte_sheet().sheet;
        let defs = crate::symbol::sheet_symbol_defs(&lte);
        let split = serde_json::to_string(&extract_netlist(&lte, &defs)).unwrap();
        let wire = serde_json::to_string(&extract_wire_nets(&lte, &defs)).unwrap();
        assert_eq!(split, wire);
        assert!(!split.contains("contact"), "{split}");
    }

    /// A mate half passes every contact through inside its own net, but plug and socket are still not joined: port → socket | plug → label … port gives two groups of contact nets.
    /// かみ合わせの片側は自分のネットの中で全部の接点を素通しにするが、プラグとソケットは今も繋がらない: ポート → ソケット | プラグ → ラベル … ポートは接点のネットの組2つになる。
    #[test]
    fn a_mate_half_passes_contacts_through_but_does_not_conduct() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let hub = put(&mut sheet, Entity::Symbol(port_block("39", 50.0, 100.0, "right", "U3", &USB)));
        let socket = put(&mut sheet, symbol("connector_socket", "J1", 100.0, 100.0));
        let plug = put(&mut sheet, symbol("connector_plug", "J1", 140.0, 100.0));
        let dongle = put(&mut sheet, Entity::Symbol(port_block("LTE1", 250.0, 100.0, "left", "P1", &USB)));
        let (h, s) = (pin_at(&sheet, hub, "U3"), pin_at(&sheet, socket, "1"));
        let (p, d) = (pin_at(&sheet, plug, "1"), pin_at(&sheet, dongle, "P1"));
        put(&mut sheet, wire(&[(h.x, h.y), (s.x, s.y)]));
        put(&mut sheet, wire(&[(p.x, p.y), (p.x, 120.0)]));
        put(&mut sheet, wire(&[(d.x, d.y), (d.x, 120.0)]));
        for (x, y) in [(p.x, 120.0), (d.x, 120.0)] {
            put(&mut sheet, Entity::NetLabel(NetLabel { id: Uuid::new_v4(), at: Point::new(x, y), name: "USB_MIC".into(), rotation: 0, attrs: Default::default() }));
        }
        let nets = extract_netlist(&sheet, &crate::symbol::sheet_symbol_defs(&sheet));
        assert_eq!(nets.len(), 8, "{:?}", nets.iter().map(|n| &n.name).collect::<Vec<_>>());
        let vbus = nets.iter().find(|n| n.name == "USB_MIC.VBUS").expect("USB_MIC.VBUS");
        assert!(vbus.pins.iter().any(|x| x.entity_id == plug && x.contact.as_deref() == Some("VBUS")));
        assert!(vbus.pins.iter().any(|x| x.entity_id == dongle));
        assert!(!vbus.pins.iter().any(|x| x.entity_id == socket || x.entity_id == hub));
        let hub_side = nets.iter().find(|n| n.pins.iter().any(|x| x.entity_id == hub) && n.contact.as_deref() == Some("GND")).unwrap();
        assert!(hub_side.pins.iter().any(|x| x.entity_id == socket));
    }

    /// In a port net a plain pin named like a contact joins that contact; a plain pin with another name goes to a residual net named like the base and is listed in the port report.
    /// ポートのネットの中で、接点と同じ名前の普通のピンはその接点に入る。名前の違う普通のピンは元の名前の残りのネットに入り、ポートの報告に載る。
    #[test]
    fn a_plain_pin_joins_the_contact_with_its_name() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let hub = put(&mut sheet, Entity::Symbol(port_block("39", 100.0, 100.0, "right", "U3", &USB)));
        let pwr = put(&mut sheet, crate::block::test_block("PWR", "", 200.0, 100.0, r#"{"left":[{"no":"3","name":"GND"}]}"#));
        let r1 = put(&mut sheet, symbol("resistor", "R1", 150.0, 140.0));
        let (h, g, r) = (pin_at(&sheet, hub, "U3"), pin_at(&sheet, pwr, "3"), pin_at(&sheet, r1, "1"));
        put(&mut sheet, wire(&[(h.x, h.y), (g.x, g.y)]));
        put(&mut sheet, wire(&[(h.x, h.y), (h.x, r.y), (r.x, r.y)]));
        let defs = crate::symbol::sheet_symbol_defs(&sheet);
        let nets = extract_netlist(&sheet, &defs);
        let gnd = nets.iter().find(|n| n.name == "N001.GND").unwrap();
        assert_eq!(pins_of(gnd), vec!["39:U3/GND", "PWR:3"]);
        let residual = nets.iter().find(|n| n.name == "N001").expect("残りのネット");
        assert_eq!(pins_of(residual), vec!["R1:1"]);
        assert_eq!(residual.contact, None);
        let report = port_report(&sheet, &defs);
        assert_eq!(report.plain_pins.len(), 1);
        assert_eq!(report.plain_pins[0].pin, "R1:1");
        assert_eq!(report.plain_pins[0].ports, vec!["39:U3"]);
    }

    /// The LTE harness drawn as four board wires into one run that ends on the single port pin 39.U3 gives four nets, each one board pin and one contact of the port.
    /// LTE ハーネスを、基板側4本の配線から1本線になってポートのピン 39.U3 1本で終わる形で描くと4ネットになり、どれも基板のピン1本とポートの接点1つを持つ。
    #[test]
    fn the_lte_harness_lands_on_one_port_pin() {
        let mut lte = crate::cable::test_support::lte_sheet();
        let mut hub = port_block("39", 200.0, 100.0, "left", "U3", &USB);
        hub.id = lte.usb;
        lte.sheet.entities.insert(lte.usb, Entity::Symbol(hub));
        let Entity::Cable(c) = lte.sheet.entities.get_mut(&lte.cable).unwrap() else { panic!() };
        for core in &mut c.cores {
            core.end_b = Some(CoreEnd::Pin { part: lte.usb, pin: "U3".into(), index: None, contact: Some(core.core.clone()) });
        }
        let nets = extract_netlist(&lte.sheet, &crate::symbol::sheet_symbol_defs(&lte.sheet));
        let mut got: Vec<Vec<String>> = nets.iter().map(pins_of).collect();
        got.sort();
        assert_eq!(
            got,
            vec![
                vec!["39:U3/D+".to_string(), "J45:3".to_string()],
                vec!["39:U3/D-".to_string(), "J45:2".to_string()],
                vec!["39:U3/GND".to_string(), "J45:4".to_string()],
                vec!["39:U3/VBUS".to_string(), "J45:1".to_string()],
            ]
        );
    }

    /// A label named <port label>.<contact> on a plain wire joins that one contact of the port net.
    /// 普通の配線に「<ポートのラベル>.<接点>」という名前のラベルを置くと、ポートのネットのその接点1つに繋がる。
    #[test]
    fn a_dotted_label_taps_one_contact() {
        let (mut sheet, hub, ..) = usb_link();
        let h = pin_at(&sheet, hub, "U3");
        put(&mut sheet, Entity::NetLabel(NetLabel { id: Uuid::new_v4(), at: Point::new(h.x + 10.0, h.y), name: "USB_MIC".into(), rotation: 0, attrs: Default::default() }));
        let r1 = put(&mut sheet, symbol("resistor", "R1", 150.0, 150.0));
        let r = pin_at(&sheet, r1, "1");
        put(&mut sheet, wire(&[(r.x, r.y), (r.x - 20.0, r.y)]));
        put(&mut sheet, Entity::NetLabel(NetLabel { id: Uuid::new_v4(), at: Point::new(r.x - 20.0, r.y), name: "USB_MIC.VBUS".into(), rotation: 0, attrs: Default::default() }));
        let nets = extract_netlist(&sheet, &crate::symbol::sheet_symbol_defs(&sheet));
        let vbus = nets.iter().find(|n| n.name == "USB_MIC.VBUS").unwrap();
        assert!(pins_of(vbus).contains(&"R1:1".to_string()), "{:?}", pins_of(vbus));
        assert_eq!(vbus.contact_labels, vec!["USB_MIC.VBUS"]);
        assert_eq!(vbus.label.as_deref(), Some("USB_MIC.VBUS"));
    }

    /// A single-line run between two port pins with no wire at all still gives one net per core, each with both ports.
    /// 配線が1本も無く、2本のポートのピンの間を1本線で結んでも、芯ごとに1ネットになり、どれにも両方のポートが載る。
    #[test]
    fn a_run_between_two_ports_gives_one_net_per_core() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let hub = put(&mut sheet, Entity::Symbol(port_block("39", 100.0, 100.0, "right", "U3", &USB)));
        let dongle = put(&mut sheet, Entity::Symbol(port_block("LTE1", 200.0, 100.0, "left", "P1", &USB)));
        let (a, b) = (pin_at(&sheet, hub, "U3"), pin_at(&sheet, dongle, "P1"));
        let cores = USB
            .iter()
            .map(|c| CableCore {
                core: (*c).into(),
                end_a: Some(CoreEnd::Pin { part: hub, pin: "U3".into(), index: None, contact: Some((*c).into()) }),
                end_b: Some(CoreEnd::Pin { part: dongle, pin: "P1".into(), index: None, contact: Some((*c).into()) }),
                ..Default::default()
            })
            .collect();
        put(&mut sheet, Entity::Cable(Cable {
            id: Uuid::new_v4(), reference: "W1".into(), part_no: String::new(), core_count: 4, sq: 0.0,
            points: Vec::new(), cores, shield: None, length_m: None, run: vec![a, b],
        }));
        let nets = extract_netlist(&sheet, &crate::symbol::sheet_symbol_defs(&sheet));
        assert_eq!(nets.len(), 4);
        for n in &nets {
            let c = n.contact.clone().expect("接点のネット");
            assert_eq!(pins_of(n), vec![format!("39:U3/{c}"), format!("LTE1:P1/{c}")]);
            assert!(n.wire_ids.is_empty() && n.cable_cores.len() == 1);
        }
    }

    /// Two ports with different contact sets join only the shared contacts; the others stay alone on their port, with no residual net, and the report lists them.
    /// 接点の組が違う2つのポートは共通の接点だけが繋がる。それ以外は自分のポートだけのネットになり、残りのネットは出ず、報告に載る。
    #[test]
    fn ports_with_different_contacts_join_only_the_shared_ones() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let hub = put(&mut sheet, Entity::Symbol(port_block("39", 100.0, 100.0, "right", "U3", &USB)));
        let otg = put(&mut sheet, Entity::Symbol(port_block("OTG1", 200.0, 100.0, "left", "P1", &["VBUS", "GND", "ID"])));
        let (a, b) = (pin_at(&sheet, hub, "U3"), pin_at(&sheet, otg, "P1"));
        put(&mut sheet, wire(&[(a.x, a.y), (b.x, b.y)]));
        let defs = crate::symbol::sheet_symbol_defs(&sheet);
        let nets = extract_netlist(&sheet, &defs);
        let names: Vec<&str> = nets.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, vec!["N001.VBUS", "N001.D-", "N001.D+", "N001.GND", "N001.ID"]);
        assert_eq!(pins_of(&nets[0]), vec!["39:U3/VBUS", "OTG1:P1/VBUS"]);
        assert_eq!(pins_of(&nets[1]), vec!["39:U3/D-"]);
        let lonely: Vec<(String, String)> = port_report(&sheet, &defs).lonely_contacts.into_iter().map(|l| (l.contact, l.port)).collect();
        assert_eq!(
            lonely,
            vec![("D-".into(), "39:U3".into()), ("D+".into(), "39:U3".into()), ("ID".into(), "OTG1:P1".into())]
        );
    }

    /// When a core joins two port clusters, the contact net takes the base name a cluster already has from an earlier net, and no new number is used for it.
    /// 芯が2つのポートのかたまりを繋ぐと、接点のネットは前のネットで付いた元の名前をそのまま使い、新しい番号は使わない。
    #[test]
    fn a_contact_net_across_two_port_clusters_reuses_the_earlier_base() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let hub = put(&mut sheet, Entity::Symbol(port_block("39", 100.0, 100.0, "right", "U3", &USB)));
        let otg = put(&mut sheet, Entity::Symbol(port_block("OTG1", 200.0, 100.0, "left", "P1", &["VBUS", "GND", "ID"])));
        let (h, o) = (pin_at(&sheet, hub, "U3"), pin_at(&sheet, otg, "P1"));
        let with_id = |n: u128, mut e: Entity| {
            if let Entity::Wire(w) = &mut e {
                w.id = Uuid::from_u128(n);
            }
            e
        };
        // ハブ側は配線 1 と 3、OTG 側は配線 2。ハブのネットが先に並ぶ
        put(&mut sheet, with_id(1, wire(&[(h.x, h.y), (h.x + 10.0, h.y)])));
        put(&mut sheet, with_id(3, wire(&[(h.x + 10.0, h.y), (h.x + 10.0, h.y + 10.0)])));
        put(&mut sheet, with_id(2, wire(&[(o.x, o.y), (o.x - 10.0, o.y)])));
        let cores = ["VBUS", "GND"]
            .iter()
            .map(|c| CableCore {
                core: (*c).into(),
                end_a: Some(CoreEnd::Pin { part: hub, pin: "U3".into(), index: None, contact: Some((*c).into()) }),
                end_b: Some(CoreEnd::Pin { part: otg, pin: "P1".into(), index: None, contact: Some((*c).into()) }),
                ..Default::default()
            })
            .collect();
        put(&mut sheet, Entity::Cable(Cable {
            id: Uuid::new_v4(), reference: "W1".into(), part_no: String::new(), core_count: 2, sq: 0.0,
            points: Vec::new(), cores, shield: None, length_m: None, run: vec![Point::new(130.0, 120.0), Point::new(170.0, 120.0)],
        }));
        let nets = extract_netlist(&sheet, &crate::symbol::sheet_symbol_defs(&sheet));
        let names: Vec<&str> = nets.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, vec!["N001.D-", "N001.D+", "N001.GND", "N001.VBUS", "N002.ID"]);
        assert_eq!(pins_of(&nets[3]), vec!["39:U3/VBUS", "OTG1:P1/VBUS"]);
    }

    /// A net pin prints as REF:PIN, with /CONTACT added when it carries a port contact.
    /// ネットのピンは「参照記号:ピン」と書き、ポートの接点を運ぶなら「/接点」を添える。
    #[test]
    fn a_net_pin_prints_with_its_contact() {
        let mut p = NetPin { reference: "39".into(), entity_id: Uuid::nil(), pin: "U3".into(), contact: None };
        assert_eq!(p.to_string(), "39:U3");
        p.contact = Some("VBUS".into());
        assert_eq!(p.to_string(), "39:U3/VBUS");
    }
}
