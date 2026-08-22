//! 帳票生成: BOM(部品表)とFrom-To電線リストのCSV出力。

use std::collections::BTreeMap;

use crate::geometry::Point;
use crate::model::{Entity, Project, Sheet, Wire};
use crate::netlist::{pin_positions, CONNECT_EPS};
use crate::symbol::{sheet_symbol_defs, SymbolDef};

/// CSVフィールドのエスケープ(カンマ・引用符・改行を含む場合はクォート)。
fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn csv_row(fields: &[String]) -> String {
    fields
        .iter()
        .map(|f| csv_escape(f))
        .collect::<Vec<_>>()
        .join(",")
}

/// 数値をCSV向けに整形(整数なら小数点なし)。
fn fmt_num(v: f64) -> String {
    if v == 0.0 {
        String::new()
    } else if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v}")
    }
}

/// 部品表CSV。全シート横断で (型番/値, シンボル) ごとに集計し参照記号を列挙する。
pub fn bom_csv(project: &Project) -> String {
    let mut groups: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for sheet in &project.sheets {
        for entity in sheet.entities.values() {
            if let Entity::Symbol(s) = entity {
                groups
                    .entry((s.value.clone(), s.symbol_id.clone()))
                    .or_default()
                    .push(s.reference.clone());
            }
        }
    }
    let mut out = String::from("参照記号,型番/値,シンボル,数量\n");
    for ((value, symbol_id), mut refs) in groups {
        refs.sort();
        out.push_str(&csv_row(&[
            refs.join(", "),
            value,
            symbol_id,
            refs.len().to_string(),
        ]));
        out.push('\n');
    }
    out
}

/// From-To電線リストの列見出し。
pub const WIRE_LIST_COLUMNS: [&str; 9] = [
    "シート",
    "From",
    "To",
    "線番",
    "線色",
    "線径sq",
    "長さm",
    "電線品番",
    "ハーネス",
];

/// ワイヤ端点1つの接続先表記。
///
/// - シンボルのピンに一致 → `"参照記号:ピン番号"` (例 `K1:A1`、`TB1:3`)
/// - ネットラベルに一致 → ラベル名
/// - どちらでもない (未接続) → 空文字
///
/// 同じ点に複数の候補があるときはピンを優先し、同種の候補同士では辞書順で最小のものを選ぶ
/// (出力を決定的にするため)。貫通端子台の左右の接続点は同じピン番号を持つので、
/// 内側・外側どちらのワイヤも同じ `TB1:1` 表記になる。
pub fn endpoint_label(sheet: &Sheet, defs: &[SymbolDef], at: &Point) -> String {
    let by_id: BTreeMap<&str, &SymbolDef> = defs.iter().map(|d| (d.id.as_str(), d)).collect();
    let mut pin_hit: Option<String> = None;
    let mut label_hit: Option<String> = None;
    for entity in sheet.entities.values() {
        match entity {
            Entity::Symbol(s) => {
                let Some(def) = by_id.get(s.symbol_id.as_str()) else {
                    continue;
                };
                for (no, pos) in pin_positions(s, def) {
                    if pos.distance_to(at) < CONNECT_EPS {
                        let text = format!("{}:{}", s.reference, no);
                        if pin_hit.as_ref().is_none_or(|cur| text < *cur) {
                            pin_hit = Some(text);
                        }
                    }
                }
            }
            Entity::NetLabel(l)
                if l.at.distance_to(at) < CONNECT_EPS
                    && label_hit.as_ref().is_none_or(|cur| l.name < *cur) =>
            {
                label_hit = Some(l.name.clone());
            }
            _ => {}
        }
    }
    pin_hit.or(label_hit).unwrap_or_default()
}

/// ワイヤの (From, To)。両端の接続先を [`endpoint_label`] で解決し、
/// **表記の辞書順で小さい方をFrom**、大きい方をToとする。未接続 (空文字) の端は常にTo側へ回す。
/// この規則によって、描いた向きに関係なく同じワイヤは常に同じFrom/Toになる。
pub fn wire_from_to(sheet: &Sheet, defs: &[SymbolDef], wire: &Wire) -> (String, String) {
    let (Some(a), Some(b)) = (wire.points.first(), wire.points.last()) else {
        return (String::new(), String::new());
    };
    let mut ends = [endpoint_label(sheet, defs, a), endpoint_label(sheet, defs, b)];
    ends.sort_by(|x, y| (x.is_empty(), x.as_str()).cmp(&(y.is_empty(), y.as_str())));
    let [from, to] = ends;
    (from, to)
}

/// From-To電線リストの行 ([`WIRE_LIST_COLUMNS`] と同じ並び)。図面シート化からも使う。
/// 行はシート順、シート内はFrom→Toの辞書順で決定的に並ぶ。
pub fn wire_list_rows(project: &Project) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for sheet in &project.sheets {
        let defs = sheet_symbol_defs(sheet);
        let mut sheet_rows: Vec<Vec<String>> = sheet
            .entities
            .values()
            .filter_map(|e| match e {
                Entity::Wire(w) => Some(w),
                _ => None,
            })
            .map(|w| {
                let (from, to) = wire_from_to(sheet, &defs, w);
                vec![
                    sheet.name.clone(),
                    from,
                    to,
                    w.net.clone().unwrap_or_default(),
                    w.color.clone(),
                    fmt_num(w.sq),
                    w.length_m.map(fmt_num).unwrap_or_default(),
                    w.part_no.clone().unwrap_or_default(),
                    crate::harness::harness_name_of_wire(sheet, w),
                ]
            })
            .collect();
        sheet_rows.sort_by(|a, b| (&a[1], &a[2], &a[3]).cmp(&(&b[1], &b[2], &b[3])));
        rows.extend(sheet_rows);
    }
    rows
}

/// From-To電線リストCSV。Wireごとに1行で、両端の接続先 (From/To) と線番・電線属性・ハーネスを並べる。
/// 長さ列は測長済みのワイヤだけ埋まる (3D配線からの書き戻しはM5)。
pub fn wire_list_csv(project: &Project) -> String {
    let mut out = WIRE_LIST_COLUMNS.join(",");
    out.push('\n');
    for row in wire_list_rows(project) {
        out.push_str(&csv_row(&row));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::*;
    use uuid::Uuid;

    fn sym(reference: &str, value: &str) -> Entity {
        Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "resistor".into(),
            at: Point::new(0.0, 0.0),
            rotation: 0,
            mirror: false,
            reference: reference.into(),
            value: value.into(),
            attrs: Default::default(),
        })
    }

    /// ピン1が(x-7.5, y)、ピン2が(x+7.5, y)にある抵抗。
    fn resistor(reference: &str, x: f64, y: f64) -> Entity {
        let mut e = sym(reference, "");
        if let Entity::Symbol(s) = &mut e {
            s.at = Point::new(x, y);
        }
        e
    }

    fn wire_between(a: (f64, f64), b: (f64, f64)) -> Entity {
        Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(a.0, a.1), Point::new(b.0, b.1)],
            color: "black".into(),
            sq: 0.3,
            length_m: None,
            part_no: None,
            net: None,
        })
    }

    /// 未接続のワイヤ1本だけのプロジェクト。属性はクロージャで調整する。
    fn one_wire_project(f: impl FnOnce(&mut Wire)) -> Project {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        let mut e = wire_between((0.0, 0.0), (10.0, 0.0));
        if let Entity::Wire(w) = &mut e {
            f(w);
        }
        sheet.entities.insert(e.id(), e);
        project
    }

    /// CSVの見出し行を除く各行を、カンマ区切りのセル列へ分解する (テストの想定にクォートは出てこない)。
    fn data_rows(csv: &str) -> Vec<Vec<String>> {
        csv.lines()
            .skip(1)
            .map(|l| l.split(',').map(|c| c.to_string()).collect())
            .collect()
    }

    /// The BOM groups symbols by part number and counts quantities per group.
    /// 部品表はシンボルを型番でまとめ、数量を集計する。
    #[test]
    fn bom_groups_by_value_and_counts() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        for (r, v) in [("R1", "10k"), ("R2", "10k"), ("K1", "JZX-22F")] {
            let e = sym(r, v);
            sheet.entities.insert(e.id(), e);
        }
        let csv = bom_csv(&project);
        let lines: Vec<_> = csv.lines().collect();
        assert_eq!(lines[0], "参照記号,型番/値,シンボル,数量");
        assert!(
            lines.iter().any(|l| l.contains("R1, R2") && l.contains("10k") && l.ends_with(",2")),
            "10kが2個に集計される: {csv}"
        );
        assert!(lines.iter().any(|l| l.contains("JZX-22F") && l.ends_with(",1")));
    }

    /// BOM fields containing commas are quoted so the CSV stays valid.
    /// カンマを含む項目は引用符で囲まれ、CSVが壊れない。
    #[test]
    fn bom_escapes_fields_with_commas() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        let e = sym("K1", "JZX-22F(D), 24VDC");
        sheet.entities.insert(e.id(), e);
        let csv = bom_csv(&project);
        assert!(csv.contains("\"JZX-22F(D), 24VDC\""), "カンマ入りはクォート: {csv}");
    }

    /// The wire list is a From-To list: every row starts with the sheet name and the two ends of the wire, and the older column order (sheet, wire number, harness, ...) is no longer produced.
    /// 電線リストはFrom-To形式で、各行はシート名とワイヤ両端の接続先から始まる。旧来の列順 (シート・線番・ハーネス…) はもう出力されない。
    #[test]
    fn wire_list_is_a_from_to_list() {
        let project = one_wire_project(|_| {});
        let csv = wire_list_csv(&project);
        assert_eq!(
            csv.lines().next().unwrap(),
            "シート,From,To,線番,線色,線径sq,長さm,電線品番,ハーネス"
        );
        assert_eq!(WIRE_LIST_COLUMNS.len(), 9);
    }

    /// An end of a wire that lands on a symbol pin is written as "reference:pin number" (for example K1:A1).
    /// シンボルのピンに届いているワイヤの端は「参照記号:ピン番号」(例 K1:A1) と書かれる。
    #[test]
    fn a_wire_end_on_a_pin_is_written_as_reference_and_pin_number() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        // R1のピン2 (x=107.5) とR2のピン1 (x=142.5) を1本のワイヤで結ぶ
        for e in [
            resistor("R1", 100.0, 50.0),
            resistor("R2", 150.0, 50.0),
            wire_between((107.5, 50.0), (142.5, 50.0)),
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let row = data_rows(&wire_list_csv(&project))[0].clone();
        assert_eq!(&row[1..3], ["R1:2", "R2:1"]);
    }

    /// An end of a wire that carries a net label is written as the label name.
    /// ネットラベルが付いているワイヤの端は、そのラベル名で書かれる。
    #[test]
    fn a_wire_end_with_a_net_label_is_written_as_the_label_name() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        let label = Entity::NetLabel(NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(0.0, 0.0),
            name: "24V".into(),
            rotation: 0,
        });
        for e in [
            resistor("R1", 20.0, 0.0),
            wire_between((0.0, 0.0), (12.5, 0.0)),
            label,
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let row = data_rows(&wire_list_csv(&project))[0].clone();
        assert_eq!(&row[1..3], ["24V", "R1:1"]);
    }

    /// An end of a wire that touches nothing is left empty in the list.
    /// 何にも接続していないワイヤの端は、リストでは空欄になる。
    #[test]
    fn an_unconnected_wire_end_is_empty() {
        let project = one_wire_project(|_| {});
        let row = data_rows(&wire_list_csv(&project))[0].clone();
        assert_eq!(&row[1..3], ["", ""], "両端とも未接続なら From/To とも空欄");
    }

    /// Of the two ends, the one whose text sorts first alphabetically becomes From, so the same wire always yields the same From and To no matter which end was drawn first.
    /// 両端のうち表記の辞書順で小さい方がFromになるので、どちら向きに描いたワイヤでもFrom/Toは常に同じになる。
    #[test]
    fn from_is_the_alphabetically_smaller_of_the_two_ends() {
        for (a, b) in [((107.5, 50.0), (142.5, 50.0)), ((142.5, 50.0), (107.5, 50.0))] {
            let mut project = Project::new("t");
            let sid = project.sheets[0].id;
            let sheet = project.sheet_mut(sid).unwrap();
            for e in [
                resistor("R1", 100.0, 50.0),
                resistor("R2", 150.0, 50.0),
                wire_between(a, b),
            ] {
                sheet.entities.insert(e.id(), e);
            }
            let row = data_rows(&wire_list_csv(&project))[0].clone();
            assert_eq!(&row[1..3], ["R1:2", "R2:1"], "描いた向きで結果が変わらない");
        }
    }

    /// When only one end is connected, that end is always From and the empty one is always To.
    /// 片側だけ接続しているワイヤでは、接続している方が必ずFrom、空欄の方が必ずToになる。
    #[test]
    fn a_connected_end_becomes_from_and_the_unconnected_end_becomes_to() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        for e in [
            resistor("R1", 100.0, 50.0),
            wire_between((107.5, 50.0), (140.0, 50.0)),
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let row = data_rows(&wire_list_csv(&project))[0].clone();
        assert_eq!(&row[1..3], ["R1:2", ""]);
    }

    /// Both sides of a feed-through terminal share one terminal number, so wires on the inside and the outside of the same terminal are both written as "TB1:1".
    /// 貫通端子の左右は同じ端子番号なので、同じ端子の内側・外側につながる電線はどちらも「TB1:1」と書かれる。
    #[test]
    fn both_sides_of_a_feed_through_terminal_use_the_same_terminal_number() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        // 2極端子台TB1を(100,50)へ。端子1は y=47.5、左右の接続点は x=97.5 / 102.5
        let tb = Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "terminal_block_2p".into(),
            at: Point::new(100.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "TB1".into(),
            value: String::new(),
            attrs: Default::default(),
        });
        for e in [
            tb,
            resistor("R1", 80.0, 47.5),
            wire_between((87.5, 47.5), (97.5, 47.5)),
            wire_between((102.5, 47.5), (120.0, 47.5)),
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let rows = data_rows(&wire_list_csv(&project));
        let ends: Vec<_> = rows.iter().map(|r| (r[1].clone(), r[2].clone())).collect();
        assert!(ends.contains(&("R1:2".to_string(), "TB1:1".to_string())), "内側: {ends:?}");
        assert!(ends.contains(&("TB1:1".to_string(), String::new())), "外側: {ends:?}");
    }

    /// The wire list keeps the wire-number column introduced with wire numbering: it holds the number written on the wire, or is empty when the wire is not numbered yet.
    /// 電線リストには線番採番で入った線番の列があり、そのワイヤに書かれた線番が入る (未採番なら空欄)。
    #[test]
    fn wire_list_has_a_wire_number_column() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        for (net, y) in [(Some("12".to_string()), 0.0), (None, 20.0)] {
            let mut e = wire_between((0.0, y), (10.0, y));
            if let Entity::Wire(w) = &mut e {
                w.net = net;
            }
            sheet.entities.insert(e.id(), e);
        }
        let numbers: Vec<String> = data_rows(&wire_list_csv(&project))
            .iter()
            .map(|r| r[3].clone())
            .collect();
        assert!(numbers.contains(&"12".to_string()), "線番12の行: {numbers:?}");
        assert!(numbers.contains(&String::new()), "未採番は空欄: {numbers:?}");
    }

    /// The wire list keeps the harness column: it carries the name of the harness boundary that fully encloses the wire, and stays empty for wires outside every harness.
    /// 電線リストにはハーネス列があり、そのワイヤを完全に囲んでいるハーネス境界の名前が入る (どの囲みにも入らない線は空欄)。
    #[test]
    fn wire_list_has_a_harness_column() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        let mut inside = wire_between((60.0, 60.0), (140.0, 60.0));
        if let Entity::Wire(w) = &mut inside {
            w.net = Some("1".into());
        }
        let mut outside = wire_between((200.0, 200.0), (250.0, 200.0));
        if let Entity::Wire(w) = &mut outside {
            w.net = Some("2".into());
        }
        let h = Entity::Harness(Harness {
            id: Uuid::new_v4(),
            points: crate::harness::rect_points(Point::new(50.0, 50.0), Point::new(150.0, 100.0)),
            name: "W1".into(),
            note: String::new(),
        });
        for e in [inside, outside, h] {
            sheet.entities.insert(e.id(), e);
        }
        let rows = data_rows(&wire_list_csv(&project));
        let by_no = |no: &str| {
            rows.iter().find(|r| r[3] == no).unwrap_or_else(|| panic!("線番{no}の行: {rows:?}")).clone()
        };
        assert_eq!(by_no("1")[8], "W1", "囲まれた線");
        assert_eq!(by_no("2")[8], "", "囲みの外は空欄");
    }

    /// The wire list contains each wire's color, gauge, length and part number.
    /// 電線リストには各ワイヤの線色・線径・長さ・品番が載る。
    #[test]
    fn wire_list_contains_attributes() {
        let project = one_wire_project(|w| {
            w.color = "red".into();
            w.sq = 0.75;
            w.length_m = Some(0.4);
            w.part_no = Some("SAMPLE0001".into());
        });
        let row = data_rows(&wire_list_csv(&project))[0].clone();
        assert_eq!(&row[4..9], ["red", "0.75", "0.4", "SAMPLE0001", ""]);
    }

    /// A wire with no measured length leaves the length column empty (lengths come back from the 3D routing later).
    /// 長さが決まっていないワイヤの長さ列は空欄になる (長さは後で3D配線から書き戻される)。
    #[test]
    fn wire_list_length_is_empty_until_it_is_known() {
        let project = one_wire_project(|w| w.length_m = None);
        assert_eq!(data_rows(&wire_list_csv(&project))[0][6], "");
    }

    /// Rows are sorted by From then To within each sheet, so exporting the same drawing twice gives byte-identical files.
    /// 行はシートごとにFrom→Toの順に並ぶので、同じ図面を2回出力すると全く同じファイルになる。
    #[test]
    fn rows_are_sorted_by_from_then_to() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        for e in [
            resistor("R1", 100.0, 50.0),
            resistor("R2", 150.0, 50.0),
            resistor("R3", 200.0, 50.0),
            wire_between((157.5, 50.0), (192.5, 50.0)),
            wire_between((107.5, 50.0), (142.5, 50.0)),
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let froms: Vec<String> = data_rows(&wire_list_csv(&project))
            .iter()
            .map(|r| r[1].clone())
            .collect();
        assert_eq!(froms, vec!["R1:2", "R2:2"]);
    }
}
