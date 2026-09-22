//! シート間クロスリファレンス (IEC 61082-1)。
//!
//! 複数シートの図面では、同じネットラベル名を別のシートにも置くことで回路が続く。
//! このモジュールはその「続き先」を図面上の住所「/シート.ゾーン」(例 `/2.B3`) として
//! 求め、ラベルの脇に描くための位置も決める。
//!
//! - 統合キー = **ネットラベル名**。同名ラベルは所属シートを問わず1つのネットになる
//! - 相手先 = 同名ラベルの**他シート**での所在。自シート内の所在は相手ではないので除外する
//! - ゾーン = [`Sheet::zone_cols`]/[`Sheet::zone_rows`] の分割をラベル座標に当てたもの
//!   (行=英字を上から、列=数字を左から)

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::geometry::Point;
use crate::model::{Entity, EntityId, NetLabel, Project, Sheet, SheetId};
use crate::netlist::{extract_netlist, on_wire, Net};

/// XRefテキストの文字高さ (mm)。ネットラベル本文より一回り小さい。
pub const XREF_FONT: f64 = 2.0;
/// ネットラベル本文の文字高さ (mm)。svg.rsのネットラベル描画と一致させること。
pub const NET_LABEL_FONT: f64 = 2.5;
/// ネットラベル本文とXRefテキストの間隔 (mm)。
pub const XREF_GAP: f64 = 1.0;
/// ネットラベル本文のベースラインを座標から持ち上げる量 (mm)。svg.rsと一致させること。
pub const NET_LABEL_RISE: f64 = 1.0;
/// 文字幅の見積り係数 (文字高さに対する1文字の平均幅)。等幅でない書体の概算。
const CHAR_WIDTH_RATIO: f64 = 0.6;
/// 複数の相手先を1行に並べるときの区切り。
pub const XREF_SEPARATOR: &str = " ";

/// ネットラベルが置かれている場所 (どのシートのどのゾーンか)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NetSite {
    pub sheet_id: SheetId,
    /// 1始まりのシート表示順 (`Project::sheets` の並び)。
    pub sheet_no: usize,
    pub sheet_name: String,
    /// ゾーンアドレス (例 "B3")。行=英字、列=数字。
    pub zone: String,
    /// この所在を作っているネットラベルのentity id。
    pub label_id: EntityId,
    /// ネットラベル名。
    pub name: String,
}

impl NetSite {
    /// 図面上の住所表記 (IEC 61082-1)。「/シート.ゾーン」= 例 `/2.B3`。
    pub fn address(&self) -> String {
        format!("/{}.{}", self.sheet_no, self.zone)
    }
}

/// プロジェクト全体で見た1ネット。同名ネットラベルでシートを跨いで統合されている。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectNet {
    /// 表示名。ラベル名 (辞書順最小) > 線番 > シート単位ネットの自動名。
    pub name: String,
    /// このネットに付いている全ラベル名 (2つ以上なら異電位の直結を疑う)。
    pub label_names: Vec<String>,
    /// このネットが現れるシート・ゾーンの一覧 (シート順→ゾーン順)。
    pub sites: Vec<NetSite>,
    /// 統合元のシート単位ネット。
    pub members: Vec<NetMember>,
}

/// 統合元となったシート1枚分のネット。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NetMember {
    pub sheet_id: SheetId,
    pub sheet_no: usize,
    pub net: Net,
    /// このシート内でネットに載っているネットラベルのentity id。
    pub label_ids: Vec<EntityId>,
}

/// プロジェクト全体のネットリスト。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProjectNetlist {
    pub nets: Vec<ProjectNet>,
}

/// 用紙座標のゾーンアドレス (例 "B3")。行=英字を上から、列=数字を左から数える。
/// 図枠の外の点は最も近いゾーンに丸める。
pub fn zone_at(sheet: &Sheet, p: Point) -> String {
    let (pw, ph) = sheet.paper_mm();
    let m = crate::svg::FRAME_MARGIN;
    let cols = sheet.zone_cols.max(1) as f64;
    let rows = sheet.zone_rows.max(1) as f64;
    let zw = (pw - 2.0 * m) / cols;
    let zh = (ph - 2.0 * m) / rows;
    let col = (((p.x - m) / zw).floor() as i64).clamp(0, cols as i64 - 1);
    let row = (((p.y - m) / zh).floor() as i64).clamp(0, rows as i64 - 1);
    let letter = char::from(b'A' + (row % 26) as u8);
    format!("{letter}{}", col + 1)
}

/// プロジェクト内の全ネットラベルの所在を、ラベル名ごとにまとめて返す。
/// 並びはシート順→ゾーン順→ラベルid順で決定的。
pub fn label_sites(project: &Project) -> BTreeMap<String, Vec<NetSite>> {
    let mut out: BTreeMap<String, Vec<NetSite>> = BTreeMap::new();
    for (i, sheet) in project.sheets.iter().enumerate() {
        for entity in sheet.entities.values() {
            let Entity::NetLabel(l) = entity else { continue };
            let name = l.name.trim();
            if name.is_empty() {
                continue;
            }
            out.entry(name.to_string()).or_default().push(NetSite {
                sheet_id: sheet.id,
                sheet_no: i + 1,
                sheet_name: sheet.name.clone(),
                zone: zone_at(sheet, l.at),
                label_id: l.id,
                name: name.to_string(),
            });
        }
    }
    for sites in out.values_mut() {
        sites.sort_by(|a, b| {
            (a.sheet_no, &a.zone, a.label_id).cmp(&(b.sheet_no, &b.zone, b.label_id))
        });
    }
    out
}

/// あるネットラベルの相手先 (同名ラベルが置かれている**他シート**の所在)。
/// 自分のシート内の所在は相手ではないので除外する。相手が無ければ空。
pub fn xref_sites(project: &Project, sheet_id: SheetId, label_name: &str) -> Vec<NetSite> {
    let name = label_name.trim();
    if name.is_empty() {
        return Vec::new();
    }
    label_sites(project)
        .remove(name)
        .unwrap_or_default()
        .into_iter()
        .filter(|s| s.sheet_id != sheet_id)
        .collect()
}

/// 相手先の住所一覧 (例 ["/2.B3", "/3.A1"])。同じ住所は1つにまとめる。
pub fn xref_addresses(project: &Project, sheet_id: SheetId, label_name: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    xref_sites(project, sheet_id, label_name)
        .into_iter()
        .map(|s| s.address())
        .filter(|a| seen.insert(a.clone()))
        .collect()
}

/// ラベル脇に描くXRefテキスト (例 "/2.B3 /3.A1")。相手がいなければNone(=何も描かない)。
pub fn xref_text(project: &Project, sheet_id: SheetId, label_name: &str) -> Option<String> {
    let addresses = xref_addresses(project, sheet_id, label_name);
    if addresses.is_empty() {
        None
    } else {
        Some(addresses.join(XREF_SEPARATOR))
    }
}

/// シート1枚分のXRefテキスト表 (ネットラベルのentity id → 表示テキスト)。
/// 相手のいないラベルは含まれない。SVG・キャンバスの描画はこれを引く。
pub fn sheet_xrefs(project: &Project, sheet_id: SheetId) -> BTreeMap<EntityId, String> {
    let Some(sheet) = project.sheet(sheet_id) else {
        return BTreeMap::new();
    };
    let sites = label_sites(project);
    let mut out = BTreeMap::new();
    for entity in sheet.entities.values() {
        let Entity::NetLabel(l) = entity else { continue };
        let name = l.name.trim();
        if name.is_empty() {
            continue;
        }
        let mut seen = BTreeSet::new();
        let addresses: Vec<String> = sites
            .get(name)
            .map(|v| v.as_slice())
            .unwrap_or_default()
            .iter()
            .filter(|s| s.sheet_id != sheet_id)
            .map(|s| s.address())
            .filter(|a| seen.insert(a.clone()))
            .collect();
        if !addresses.is_empty() {
            out.insert(l.id, addresses.join(XREF_SEPARATOR));
        }
    }
    out
}

/// デバイス (参照記号) が置かれている場所 (どのシートのどのゾーンか)。G13:
/// ネットラベルのシート間クロスリファレンス ([`NetSite`]) を、参照記号を持つシンボル
/// インスタンス一般へ広げたもの。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeviceSite {
    pub sheet_id: SheetId,
    /// 1始まりのシート表示順。
    pub sheet_no: usize,
    pub sheet_name: String,
    /// ゾーンアドレス (例 "B3")。
    pub zone: String,
    /// この所在を作っているシンボルインスタンスのentity id。
    pub entity_id: EntityId,
    pub reference: String,
}

impl DeviceSite {
    /// 図面上の住所表記 (IEC 61082-1)。「/シート.ゾーン」= 例 `/2.B3`。
    pub fn address(&self) -> String {
        format!("/{}.{}", self.sheet_no, self.zone)
    }
}

/// プロジェクト内の全シンボルインスタンスの所在を、参照記号ごとにまとめて返す。
/// 参照記号が空のインスタンスは含まない。コイル・接点はコイル⇔接点クロスリファレンス
/// ([`crate::relay_xref`]) が既に自動で所在を注記するので、ここでは対象から外す
/// (二重に描かないため)。並びは[`label_sites`]と同じくシート順→ゾーン順→id順で決定的。
pub fn device_sites(project: &Project) -> BTreeMap<String, Vec<DeviceSite>> {
    let mut out: BTreeMap<String, Vec<DeviceSite>> = BTreeMap::new();
    for (i, sheet) in project.sheets.iter().enumerate() {
        for entity in sheet.entities.values() {
            let Entity::Symbol(s) = entity else { continue };
            if crate::relay_xref::relay_role(&s.symbol_id).is_some() {
                continue;
            }
            let reference = s.reference.trim();
            if reference.is_empty() {
                continue;
            }
            out.entry(reference.to_string()).or_default().push(DeviceSite {
                sheet_id: sheet.id,
                sheet_no: i + 1,
                sheet_name: sheet.name.clone(),
                zone: zone_at(sheet, s.at),
                entity_id: s.id,
                reference: reference.to_string(),
            });
        }
    }
    for sites in out.values_mut() {
        sites.sort_by(|a, b| (a.sheet_no, &a.zone, a.entity_id).cmp(&(b.sheet_no, &b.zone, b.entity_id)));
    }
    out
}

/// あるデバイス (参照記号) の相手先 (同じ参照記号を持つインスタンスが置かれている
/// **他シート**での所在)。自分のシート内の所在は相手ではないので除外する。相手がいなければ空。
pub fn device_xref_sites(project: &Project, sheet_id: SheetId, reference: &str) -> Vec<DeviceSite> {
    let reference = reference.trim();
    if reference.is_empty() {
        return Vec::new();
    }
    device_sites(project)
        .remove(reference)
        .unwrap_or_default()
        .into_iter()
        .filter(|s| s.sheet_id != sheet_id)
        .collect()
}

/// 相手先の住所一覧 (例 ["/2.B3", "/3.A1"])。同じ住所は1つにまとめる。
pub fn device_xref_addresses(project: &Project, sheet_id: SheetId, reference: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    device_xref_sites(project, sheet_id, reference)
        .into_iter()
        .map(|s| s.address())
        .filter(|a| seen.insert(a.clone()))
        .collect()
}

/// シート1枚分のデバイス単位XRefテキスト表 (シンボルのentity id → 表示テキスト、G13)。
/// 複数シートに跨る参照記号を持つインスタンスだけが入る。コイル・接点は含まない
/// (コイル⇔接点クロスリファレンスが別に注記する)。SVG・キャンバスの描画はこれを引く。
pub fn sheet_device_xrefs(project: &Project, sheet_id: SheetId) -> BTreeMap<EntityId, String> {
    let Some(sheet) = project.sheet(sheet_id) else {
        return BTreeMap::new();
    };
    let sites = device_sites(project);
    let mut out = BTreeMap::new();
    for entity in sheet.entities.values() {
        let Entity::Symbol(s) = entity else { continue };
        if crate::relay_xref::relay_role(&s.symbol_id).is_some() {
            continue;
        }
        let reference = s.reference.trim();
        if reference.is_empty() {
            continue;
        }
        let mut seen = BTreeSet::new();
        let addresses: Vec<String> = sites
            .get(reference)
            .map(|v| v.as_slice())
            .unwrap_or_default()
            .iter()
            .filter(|site| site.sheet_id != sheet_id)
            .map(|site| site.address())
            .filter(|a| seen.insert(a.clone()))
            .collect();
        if !addresses.is_empty() {
            out.insert(s.id, addresses.join(XREF_SEPARATOR));
        }
    }
    out
}

/// 参照記号の脇に置くデバイスXRefテキストの基準点 (テキストは左揃え、G13)。
/// シンボル外形の右端から [`XREF_GAP`] だけ空け、高さは参照記号のラベルと同じ行に揃える
/// (コイル所在が接点の中央高さに出るのと違い、参照記号は常に外形の上に出るため)。
pub fn device_xref_at(
    inst: &crate::model::SymbolInstance,
    def: &crate::symbol::SymbolDef,
) -> Point {
    let (_, max) = crate::svg::symbol_bounds(inst, def);
    let top = crate::svg::symbol_top_y(inst, def);
    Point::new(max.x + XREF_GAP, top - crate::svg::REF_LABEL_DY)
}

/// ネットラベル本文の右脇に置くXRefテキストの基準点 (テキストは左揃え)。
/// ラベル本文の幅を文字数から見積り、その右に [`XREF_GAP`] だけ空ける。
///
/// `font`はネットラベル本文の文字高さ (mm)。シートの注記文字高 (G20) で変わるので、
/// 逃がし量とベースラインの持ち上げも同じ比率で伸縮する。
pub fn xref_text_at(label: &NetLabel, font: f64) -> Point {
    let scale = font / NET_LABEL_FONT;
    let width = font * CHAR_WIDTH_RATIO * label.name.chars().count() as f64;
    Point::new(
        label.at.x + width + XREF_GAP * scale,
        label.at.y - NET_LABEL_RISE * scale,
    )
}

/// プロジェクト全体のネットリスト。シートごとのネットリストを取り、**同名ネットラベル**を
/// 共有するネット同士をシートを跨いで1つに統合する。統合されたネットは帳票・検証で
/// 1ネットとして数える。
pub fn extract_netlist_project(project: &Project) -> ProjectNetlist {
    // シート単位ネットを平坦に並べ、載っているラベルを添える
    let mut members: Vec<NetMember> = Vec::new();
    let mut member_names: Vec<BTreeSet<String>> = Vec::new();
    for (i, sheet) in project.sheets.iter().enumerate() {
        let defs = crate::symbol::sheet_symbol_defs(sheet);
        for net in extract_netlist(sheet, &defs) {
            let (label_ids, names) = labels_on_net(sheet, &net);
            members.push(NetMember {
                sheet_id: sheet.id,
                sheet_no: i + 1,
                net,
                label_ids,
            });
            member_names.push(names);
        }
    }

    // ラベル名を共有するネット同士を統合 (同じ名前を持つ最初のネットへ寄せる)
    let mut parent: Vec<usize> = (0..members.len()).collect();
    fn find(parent: &mut Vec<usize>, i: usize) -> usize {
        if parent[i] != i {
            let r = find(parent, parent[i]);
            parent[i] = r;
        }
        parent[i]
    }
    let mut first_by_name: BTreeMap<&str, usize> = BTreeMap::new();
    for (i, names) in member_names.iter().enumerate() {
        for name in names {
            match first_by_name.get(name.as_str()) {
                Some(&j) => {
                    let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                    if a != b {
                        parent[a] = b;
                    }
                }
                None => {
                    first_by_name.insert(name.as_str(), i);
                }
            }
        }
    }

    // グループ化 (代表→メンバ番号)。並びは最初に現れた順で決定的
    let mut order: Vec<usize> = Vec::new();
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..members.len() {
        let root = find(&mut parent, i);
        if !groups.contains_key(&root) {
            order.push(root);
        }
        groups.entry(root).or_default().push(i);
    }

    let sites_by_name = label_sites(project);
    let mut nets = Vec::new();
    for root in order {
        let idx = &groups[&root];
        let mut label_names: BTreeSet<String> = BTreeSet::new();
        for &i in idx {
            label_names.extend(member_names[i].iter().cloned());
        }
        let mut sites: Vec<NetSite> = Vec::new();
        for name in &label_names {
            if let Some(v) = sites_by_name.get(name) {
                sites.extend(v.iter().cloned());
            }
        }
        sites.sort_by(|a, b| {
            (a.sheet_no, &a.zone, a.label_id).cmp(&(b.sheet_no, &b.zone, b.label_id))
        });
        let group: Vec<NetMember> = idx.iter().map(|&i| members[i].clone()).collect();
        let name = label_names
            .iter()
            .next()
            .cloned()
            .or_else(|| group.iter().find_map(|m| m.net.wire_no.clone()))
            .unwrap_or_else(|| group[0].net.name.clone());
        nets.push(ProjectNet {
            name,
            label_names: label_names.into_iter().collect(),
            sites,
            members: group,
        });
    }
    ProjectNetlist { nets }
}

/// クロスリファレンス表 (ネット所在一覧) の列見出し。
pub const XREF_TABLE_COLUMNS: [&str; 5] = ["ネット", "線番", "接続先", "シート", "所在"];

/// クロスリファレンス表の行 ([`XREF_TABLE_COLUMNS`] と同じ並び)。
///
/// プロジェクト全体の統合ネット ([`extract_netlist_project`]) 1本が1行。
/// 「接続先」はネットに繋がる全ピンの `参照記号:ピン番号`、「シート」はネットが現れるシート番号、
/// 「所在」はネットラベルの図面上の住所 (`/シート.ゾーン`)。行はネット名の昇順で決定的に並ぶ。
pub fn xref_table_rows(project: &Project) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = extract_netlist_project(project)
        .nets
        .into_iter()
        .map(|net| {
            let mut wire_nos: BTreeSet<String> = BTreeSet::new();
            let mut pins: BTreeSet<String> = BTreeSet::new();
            let mut sheet_nos: BTreeSet<usize> = BTreeSet::new();
            for m in &net.members {
                if let Some(no) = &m.net.wire_no {
                    wire_nos.insert(no.clone());
                }
                for p in &m.net.pins {
                    pins.insert(format!("{}:{}", p.reference, p.pin));
                }
                sheet_nos.insert(m.sheet_no);
            }
            vec![
                net.name,
                wire_nos.into_iter().collect::<Vec<_>>().join(", "),
                pins.into_iter().collect::<Vec<_>>().join(", "),
                sheet_nos
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
                net.sites
                    .iter()
                    .map(NetSite::address)
                    .collect::<Vec<_>>()
                    .join(XREF_SEPARATOR),
            ]
        })
        .collect();
    rows.sort_by(|a, b| (&a[0], &a[2]).cmp(&(&b[0], &b[2])));
    rows
}

/// クロスリファレンス表のCSV。見出しは [`XREF_TABLE_COLUMNS`]、行は [`xref_table_rows`]。
pub fn xref_table_csv(project: &Project) -> String {
    let mut out = XREF_TABLE_COLUMNS.join(",");
    out.push('\n');
    for row in xref_table_rows(project) {
        out.push_str(&crate::reports::csv_row(&row));
        out.push('\n');
    }
    out
}

/// シート内でそのネットの配線に載っているネットラベル (entity id, 名前)。
fn labels_on_net(sheet: &Sheet, net: &Net) -> (Vec<EntityId>, BTreeSet<String>) {
    let wires: Vec<&crate::model::Wire> = net
        .wire_ids
        .iter()
        .filter_map(|id| match sheet.entities.get(id) {
            Some(Entity::Wire(w)) => Some(w),
            _ => None,
        })
        .collect();
    let mut ids = Vec::new();
    let mut names = BTreeSet::new();
    for entity in sheet.entities.values() {
        let Entity::NetLabel(l) = entity else { continue };
        let name = l.name.trim();
        if name.is_empty() {
            continue;
        }
        if wires.iter().any(|w| on_wire(w, &l.at)) {
            ids.push(l.id);
            names.insert(name.to_string());
        }
    }
    ids.sort();
    (ids, names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use uuid::Uuid;

    fn wire(points: &[(f64, f64)]) -> Entity {
        Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            color: "black".into(),
            sq: 0.75,
            length_m: None,
            part_no: None,
            net: None,
        })
    }

    fn label(name: &str, x: f64, y: f64) -> Entity {
        Entity::NetLabel(NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(x, y),
            name: name.into(),
            rotation: 0,
            attrs: Default::default(),
        })
    }

    fn symbol(symbol_id: &str, reference: &str, x: f64, y: f64) -> Entity {
        Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: symbol_id.into(),
            at: Point::new(x, y),
            rotation: 0,
            mirror: false,
            reference: reference.into(),
            value: String::new(),
            attrs: Default::default(),
        })
    }

    /// 2枚のA3シートを持ち、それぞれに同名ラベル付きの配線を1本置いたプロジェクト。
    /// シート1のラベルはゾーンA1、シート2のラベルはゾーンB3に来る。
    fn two_sheet_project() -> Project {
        let mut project = Project::new("t");
        project
            .sheets
            .push(Sheet::new("Sheet2", PaperSize::A3, Orientation::Landscape));
        let s1 = project.sheets[0].id;
        let s2 = project.sheets[1].id;
        for e in [wire(&[(20.0, 20.0), (60.0, 20.0)]), label("24V_1", 20.0, 20.0)] {
            project.sheet_mut(s1).unwrap().entities.insert(e.id(), e);
        }
        for e in [
            wire(&[(250.0, 70.0), (290.0, 70.0)]),
            label("24V_1", 250.0, 70.0),
        ] {
            project.sheet_mut(s2).unwrap().entities.insert(e.id(), e);
        }
        project
    }

    /// A zone address combines the row letter (top to bottom) with the column number (left to right), e.g. "B3".
    /// ゾーンアドレスは行の英字(上から)と列の数字(左から)を組み合わせた「B3」形式になる。
    #[test]
    fn zone_address_combines_row_letter_and_column_number() {
        let sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        // A3横: 図枠は10..410 × 10..287。4列 × 6行
        assert_eq!(zone_at(&sheet, Point::new(20.0, 20.0)), "A1");
        assert_eq!(zone_at(&sheet, Point::new(250.0, 70.0)), "B3");
        assert_eq!(zone_at(&sheet, Point::new(400.0, 280.0)), "F4");
    }

    /// Points outside the drawing frame are rounded to the nearest zone instead of producing an invalid address.
    /// 図枠の外にある点は、無効なアドレスにはならず最も近いゾーンに丸められる。
    #[test]
    fn zone_address_clamps_points_outside_the_frame() {
        let sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        assert_eq!(zone_at(&sheet, Point::new(-50.0, -50.0)), "A1");
        assert_eq!(zone_at(&sheet, Point::new(9999.0, 9999.0)), "F4");
    }

    /// Net labels with the same name on different sheets are merged into a single project-wide net.
    /// 別々のシートに置かれた同名のネットラベルは、プロジェクト全体では1つのネットに統合される。
    #[test]
    fn same_named_labels_merge_into_one_project_net() {
        let project = two_sheet_project();
        let netlist = extract_netlist_project(&project);
        assert_eq!(netlist.nets.len(), 1, "跨ぎネットは1ネット: {netlist:?}");
        let net = &netlist.nets[0];
        assert_eq!(net.name, "24V_1");
        assert_eq!(net.members.len(), 2, "2枚のシートのネットが統合される");
        assert_eq!(net.sites.len(), 2);
        assert_eq!(net.sites[0].sheet_no, 1);
        assert_eq!(net.sites[1].sheet_no, 2);
        assert_eq!(net.sites[1].zone, "B3");
    }

    /// Nets with different label names stay separate across sheets.
    /// ラベル名が違うネットは、シートを跨いでも統合されず別のネットのままになる。
    #[test]
    fn differently_named_labels_stay_separate_nets() {
        let mut project = two_sheet_project();
        let s2 = project.sheets[1].id;
        for e in project.sheet_mut(s2).unwrap().entities.values_mut() {
            if let Entity::NetLabel(l) = e {
                l.name = "0V".into();
            }
        }
        let netlist = extract_netlist_project(&project);
        assert_eq!(netlist.nets.len(), 2, "{netlist:?}");
    }

    /// The cross-reference of a label is the address "/sheet.zone" of the same-named label on another sheet.
    /// ラベルの相手先は、他のシートにある同名ラベルの住所「/シート.ゾーン」になる。
    #[test]
    fn cross_reference_address_uses_slash_sheet_dot_zone() {
        let project = two_sheet_project();
        let s1 = project.sheets[0].id;
        assert_eq!(xref_addresses(&project, s1, "24V_1"), vec!["/2.B3"]);
        assert_eq!(xref_text(&project, s1, "24V_1").as_deref(), Some("/2.B3"));
    }

    /// A label on the destination sheet points back to the source sheet, so both sides show the counterpart.
    /// 相手側のシートのラベルからも元のシートが見えるので、双方に相手先が表示される。
    #[test]
    fn cross_reference_is_shown_on_both_sides() {
        let project = two_sheet_project();
        let s2 = project.sheets[1].id;
        assert_eq!(xref_text(&project, s2, "24V_1").as_deref(), Some("/1.A1"));
    }

    /// When the same net continues onto several sheets, every counterpart address is listed.
    /// 同じネットが複数のシートに続くときは、相手先の住所が全て列挙される。
    #[test]
    fn multiple_counterparts_are_all_listed() {
        let mut project = two_sheet_project();
        let mut s3 = Sheet::new("Sheet3", PaperSize::A3, Orientation::Landscape);
        for e in [wire(&[(20.0, 20.0), (60.0, 20.0)]), label("24V_1", 20.0, 20.0)] {
            s3.entities.insert(e.id(), e);
        }
        project.sheets.push(s3);
        let s1 = project.sheets[0].id;
        assert_eq!(xref_text(&project, s1, "24V_1").as_deref(), Some("/2.B3 /3.A1"));
    }

    /// The label's own sheet is never listed as a counterpart, even when the same name appears twice on it.
    /// 自分のシート内の所在は、同名ラベルが2つあっても相手先には出ない。
    #[test]
    fn own_sheet_is_excluded_from_counterparts() {
        let mut project = two_sheet_project();
        let s1 = project.sheets[0].id;
        let e = label("24V_1", 250.0, 70.0);
        project.sheet_mut(s1).unwrap().entities.insert(e.id(), e);
        let text = xref_text(&project, s1, "24V_1");
        assert_eq!(text.as_deref(), Some("/2.B3"), "自シートのB3は出ない");
    }

    /// A label with no counterpart on another sheet shows no cross-reference at all.
    /// 他のシートに相手がいないラベルには、クロスリファレンスが一切表示されない。
    #[test]
    fn label_without_counterpart_shows_nothing() {
        let mut project = two_sheet_project();
        let s1 = project.sheets[0].id;
        let e = label("ONLY_HERE", 100.0, 100.0);
        let only_id = e.id();
        project.sheet_mut(s1).unwrap().entities.insert(e.id(), e);
        assert_eq!(xref_text(&project, s1, "ONLY_HERE"), None);
        assert!(!sheet_xrefs(&project, s1).contains_key(&only_id));
    }

    /// The per-sheet cross-reference table maps each label entity to the text drawn beside it.
    /// シートごとのクロスリファレンス表は、各ラベルのentity idを脇に描くテキストへ対応付ける。
    #[test]
    fn sheet_cross_reference_table_maps_labels_to_text() {
        let project = two_sheet_project();
        let s1 = project.sheets[0].id;
        let label_id = project.sheets[0]
            .entities
            .values()
            .find_map(|e| match e {
                Entity::NetLabel(l) => Some(l.id),
                _ => None,
            })
            .unwrap();
        let table = sheet_xrefs(&project, s1);
        assert_eq!(table.get(&label_id).map(String::as_str), Some("/2.B3"));
    }

    /// The cross-reference text sits to the right of the label text, on the same baseline.
    /// クロスリファレンスのテキストは、ラベル本文の右側に同じベースラインで並ぶ。
    #[test]
    fn cross_reference_text_sits_right_of_the_label() {
        let l = NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(100.0, 50.0),
            name: "24V_1".into(),
            rotation: 0,
            attrs: Default::default(),
        };
        let at = xref_text_at(&l, NET_LABEL_FONT);
        assert!(at.x > 100.0, "ラベルの右に置く: {at:?}");
        assert!((at.y - (50.0 - NET_LABEL_RISE)).abs() < 1e-9, "ベースラインは同じ");
        // 5文字分の幅 + 間隔
        assert!((at.x - (100.0 + 2.5 * 0.6 * 5.0 + 1.0)).abs() < 1e-9);
    }

    /// A smaller annotation text height moves the cross-reference closer to the label, keeping the gap in proportion.
    /// 注記文字高を小さくすると、クロスリファレンスも同じ比率でラベルに寄る。
    #[test]
    fn cross_reference_follows_the_annotation_text_height() {
        let l = NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(100.0, 50.0),
            name: "24V_1".into(),
            rotation: 0,
            attrs: Default::default(),
        };
        let small = xref_text_at(&l, NET_LABEL_FONT * 0.6);
        assert!((small.x - (100.0 + (2.5 * 0.6 * 5.0 + 1.0) * 0.6)).abs() < 1e-9, "{small:?}");
        assert!((small.y - (50.0 - NET_LABEL_RISE * 0.6)).abs() < 1e-9);
    }

    /// The cross-reference table has one row per project-wide net, listing the sheets it spans and the drawing addresses of its labels.
    /// クロスリファレンス表はプロジェクト全体のネット1本につき1行で、跨るシートとラベルの図面上の住所を並べる。
    #[test]
    fn cross_reference_table_lists_one_row_per_net_with_its_sites() {
        let project = two_sheet_project();
        let rows = xref_table_rows(&project);
        assert_eq!(rows.len(), 1, "同名ラベルで1本に統合される: {rows:?}");
        assert_eq!(rows[0][0], "24V_1");
        assert_eq!(rows[0][3], "1, 2", "現れるシート番号");
        assert_eq!(rows[0][4], "/1.A1 /2.B3", "ラベルの住所");
    }

    /// The columns of the cross-reference table are net / wire number / connected pins / sheets / sites.
    /// クロスリファレンス表の列は ネット・線番・接続先・シート・所在 の5列。
    #[test]
    fn cross_reference_table_columns_are_net_wire_pins_sheets_sites() {
        assert_eq!(
            XREF_TABLE_COLUMNS,
            ["ネット", "線番", "接続先", "シート", "所在"]
        );
        assert_eq!(xref_table_rows(&two_sheet_project())[0].len(), XREF_TABLE_COLUMNS.len());
    }

    // ---- G13: デバイス単位シート間参照 ----

    /// 2枚のA3シートを持ち、それぞれに同じ参照記号のシンボルを1個ずつ置いたプロジェクト。
    fn two_sheet_device_project(symbol_id: &str, reference: &str) -> Project {
        let mut project = Project::new("t");
        project
            .sheets
            .push(Sheet::new("Sheet2", PaperSize::A3, Orientation::Landscape));
        let s1 = project.sheets[0].id;
        let s2 = project.sheets[1].id;
        let e1 = symbol(symbol_id, reference, 20.0, 20.0);
        project.sheet_mut(s1).unwrap().entities.insert(e1.id(), e1);
        let e2 = symbol(symbol_id, reference, 250.0, 70.0);
        project.sheet_mut(s2).unwrap().entities.insert(e2.id(), e2);
        project
    }

    /// A designator drawn on two different sheets gets a mutual cross-reference: each instance shows the other sheet's address.
    /// 2枚の違うシートに描かれた参照記号は相互に参照し合う: それぞれのインスタンスにもう一方のシートの住所が出る。
    #[test]
    fn a_designator_on_two_sheets_cross_references_each_other() {
        let project = two_sheet_device_project("motor", "M1");
        let s1 = project.sheets[0].id;
        let s2 = project.sheets[1].id;
        assert_eq!(device_xref_addresses(&project, s1, "M1"), vec!["/2.B3"]);
        assert_eq!(device_xref_addresses(&project, s2, "M1"), vec!["/1.A1"]);
    }

    /// A designator drawn on a single sheet only has no cross-reference at all.
    /// 1枚のシートにしか描かれていない参照記号には、クロスリファレンスが一切付かない。
    #[test]
    fn a_designator_on_a_single_sheet_has_no_cross_reference() {
        let mut project = Project::new("t");
        let s1 = project.sheets[0].id;
        let e = symbol("motor", "M1", 20.0, 20.0);
        project.sheet_mut(s1).unwrap().entities.insert(e.id(), e);
        assert!(device_xref_addresses(&project, s1, "M1").is_empty());
        assert!(sheet_device_xrefs(&project, s1).is_empty());
    }

    /// Relay coils and contacts are skipped by the device cross-reference, because relay_xref already annotates their coil location automatically.
    /// リレーのコイル・接点はデバイス単位のクロスリファレンスの対象から外れる(コイル⇔接点クロスリファレンスが既に自動で所在を注記するため)。
    #[test]
    fn relay_coil_and_contacts_are_skipped_by_the_device_cross_reference() {
        let project = two_sheet_device_project(crate::relay_xref::COIL_SYMBOL_ID, "K1");
        let s1 = project.sheets[0].id;
        assert!(device_xref_addresses(&project, s1, "K1").is_empty());
        assert!(sheet_device_xrefs(&project, s1).is_empty());
    }

    /// The per-sheet device cross-reference table maps each symbol entity to the text drawn beside it.
    /// シートごとのデバイスXRef表は、各シンボルのentity idを脇に描くテキストへ対応付ける。
    #[test]
    fn sheet_device_cross_reference_table_maps_symbols_to_text() {
        let project = two_sheet_device_project("motor", "M1");
        let s1 = project.sheets[0].id;
        let entity_id = project.sheets[0]
            .entities
            .values()
            .find_map(|e| match e {
                Entity::Symbol(s) => Some(s.id),
                _ => None,
            })
            .unwrap();
        let table = sheet_device_xrefs(&project, s1);
        assert_eq!(table.get(&entity_id).map(String::as_str), Some("/2.B3"));
    }

    /// A designator on three sheets lists every other sheet's address, in sheet order, on one line.
    /// 3枚のシートに跨る参照記号は、シート順で並んだ他シート全部の住所を1行に並べる。
    #[test]
    fn a_designator_on_three_sheets_lists_every_other_sheet() {
        let mut project = Project::new("t");
        project
            .sheets
            .push(Sheet::new("Sheet2", PaperSize::A3, Orientation::Landscape));
        project
            .sheets
            .push(Sheet::new("Sheet3", PaperSize::A3, Orientation::Landscape));
        for i in 0..3 {
            let sid = project.sheets[i].id;
            let e = symbol("motor", "M1", 20.0, 20.0);
            project.sheet_mut(sid).unwrap().entities.insert(e.id(), e);
        }
        let s1 = project.sheets[0].id;
        assert_eq!(device_xref_addresses(&project, s1, "M1"), vec!["/2.A1", "/3.A1"]);
    }

    /// The device cross-reference text sits to the right of the symbol outline, on the same row as the designator label.
    /// デバイスXRefのテキストはシンボル外形の右側、参照記号のラベルと同じ高さに置かれる。
    #[test]
    fn device_cross_reference_text_sits_right_of_the_outline() {
        let inst = SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "motor".into(),
            at: Point::new(100.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "M1".into(),
            value: String::new(),
            attrs: Default::default(),
        };
        let def = crate::symbol::resolve_symbol("motor").unwrap();
        let (_, max) = crate::svg::symbol_bounds(&inst, &def);
        let at = device_xref_at(&inst, &def);
        assert!(at.x > max.x, "外形の右に置く: {at:?}");
        assert!(
            (at.y - (crate::svg::symbol_top_y(&inst, &def) - crate::svg::REF_LABEL_DY)).abs() < 1e-9,
            "参照記号ラベルと同じ高さ: {at:?}"
        );
    }
}
