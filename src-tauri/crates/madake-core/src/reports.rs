//! 帳票生成: BOM(部品表)と電線リストのCSV出力。

use std::collections::BTreeMap;

use crate::model::{Entity, Project};

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

/// 電線リストCSV。Wireごとに1行(線番・品番なしは空欄)。
pub fn wire_list_csv(project: &Project) -> String {
    let mut out = String::from("シート,線番,電線品番,線色,線径sq,長さm\n");
    for sheet in &project.sheets {
        for entity in sheet.entities.values() {
            if let Entity::Wire(w) = entity {
                out.push_str(&csv_row(&[
                    sheet.name.clone(),
                    w.net.clone().unwrap_or_default(),
                    w.part_no.clone().unwrap_or_default(),
                    w.color.clone(),
                    fmt_num(w.sq),
                    w.length_m.map(fmt_num).unwrap_or_default(),
                ]));
                out.push('\n');
            }
        }
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

    /// The wire list has a wire-number column, filled with the number assigned to the wire's net (empty when unnumbered).
    /// 電線リストには線番の列があり、そのワイヤのネットに振られた線番が入る (未採番なら空欄)。
    #[test]
    fn wire_list_has_a_wire_number_column() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        for net in [Some("12".to_string()), None] {
            let e = Entity::Wire(Wire {
                id: Uuid::new_v4(),
                points: vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)],
                color: "red".into(),
                sq: 0.75,
                length_m: None,
                part_no: None,
                net,
            });
            sheet.entities.insert(e.id(), e);
        }
        let csv = wire_list_csv(&project);
        assert_eq!(csv.lines().next().unwrap(), "シート,線番,電線品番,線色,線径sq,長さm");
        assert!(csv.lines().any(|l| l.starts_with("Sheet1,12,")), "線番12の行: {csv}");
        assert!(csv.lines().any(|l| l.starts_with("Sheet1,,")), "未採番は空欄: {csv}");
    }

    /// The wire list contains each wire's part number, color, gauge and length.
    /// 電線リストには各ワイヤの品番・線色・線径・長さが載る。
    #[test]
    fn wire_list_contains_attributes() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        let e = Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)],
            color: "red".into(),
            sq: 0.75,
            length_m: Some(0.4),
            part_no: Some("SAMPLE0001".into()),
            net: None,
        });
        sheet.entities.insert(e.id(), e);
        let csv = wire_list_csv(&project);
        assert!(csv.lines().nth(1).unwrap().contains("SAMPLE0001,red,0.75,0.4"));
    }
}
