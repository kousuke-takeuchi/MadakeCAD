//! 電線品番凡例ブロック (G9残)。回路図シート専用。
//!
//! 盤の凡例表 ([`crate::panel::PanelLegend`]/[`crate::panel::legend_rows`]) と対になる
//! 存在で、**中身は持たない**。表に並ぶ行は描画のたびに同じシートの配線から導出する
//! ([`legend_rows`]) ので、配線を足しても電線品番表 ([`crate::model::Project::wire_parts`])
//! を直しても手直しが要らない (帳票と同じ「図面から導出する」原則)。
//!
//! ## 決めごと
//!
//! - 行は**そのシートの配線**を (線色, 線径sq, 電線品番) の組ごとにまとめ、使用長 (m) を
//!   合算した1行にする。品番はワイヤリスト ([`crate::reports::wire_list_rows`]) と同じ規則
//!   ([`crate::wire_parts::wire_part_number`]) で引き、表に無い組み合わせは空欄のまま
//! - 線色は**図面に印字する名前へそろえてから**まとめる ([`crate::wire_note::color_display`])。
//!   同じシートの配線注記と同じ色名になり、`red`と`赤`が2行に割れない
//! - 行は**線色→線径→電線品番**の順に並べる。線色+線径が同じでも、ワイヤ自身が持つ品番と
//!   表から引いた品番が違えば別の行になるので、並び順を一意に決めるには品番まで要る
//! - 使用長は**m単位・小数第2位**で書く (BOMの電線集約行 [`crate::reports::bom_rows`] と
//!   同じ書式。合算値なので、丸めずに整形すると二進小数の誤差が桁となって図面に出る)
//! - 配線が1本も無いシートでも、列見出しの1行だけは描く
//! - ネットリスト・ERC・部品表・電線リストのどれにも出てこない (図形・ハーネス境界と同じ
//!   扱い): 電気的な意味を持たない作図要素

use std::collections::BTreeMap;

use crate::geometry::Point;
use crate::model::{Entity, Sheet, Wire, WireLegend, WirePart};

/// 凡例ブロックの列見出し。
pub const LEGEND_COLUMNS: [&str; 4] = ["線色", "sq", "電線品番", "使用長 m"];
/// 列幅 (mm)。合計が表の幅。
pub const LEGEND_COL_W: [f64; 4] = [24.0, 14.0, 30.0, 22.0];
/// 行の高さ (mm)。列見出しの行も同じ高さ。
pub const LEGEND_ROW_H: f64 = 5.0;
/// 文字高さ (mm、G9残の指針どおり2.0mm)。
pub const LEGEND_FONT: f64 = 2.0;

/// シート上の電線品番凡例ブロック (id順)。
pub fn legends(sheet: &Sheet) -> impl Iterator<Item = &WireLegend> {
    sheet.entities.values().filter_map(|e| match e {
        Entity::WireLegend(l) => Some(l),
        _ => None,
    })
}

fn wires(sheet: &Sheet) -> impl Iterator<Item = &Wire> {
    sheet.entities.values().filter_map(|e| match e {
        Entity::Wire(w) => Some(w),
        _ => None,
    })
}

/// 凡例ブロックに並ぶ行 ([`LEGEND_COLUMNS`] と同じ並び、表はスライスで渡す)。
/// [`legend_rows`]の本体で、プロジェクト全体を持たない文脈からも呼べるようにしてある。
pub fn legend_rows_with(sheet: &Sheet, wire_parts: &[WirePart]) -> Vec<Vec<String>> {
    // (線色, 線径sqの表示, 電線品番) ごとに使用長を合算する。sqは表示用の文字列を鍵の一部にして、
    // 浮動小数の比較を避けつつ「同じ見た目の行は1行にまとめる」を保証する。並べ替え用に
    // 実際のsq値(表示前の浮動小数)も別に持っておく
    let mut totals: BTreeMap<(String, String, String), f64> = BTreeMap::new();
    let mut sq_values: BTreeMap<String, f64> = BTreeMap::new();
    for w in wires(sheet) {
        // 図面へ印字する色名にそろえてからまとめる (配線注記と同じ表記。`red`と`赤`は同じ行)
        let color = crate::wire_note::color_display(&w.color);
        let sq_text = crate::reports::fmt_num(w.sq);
        sq_values.insert(sq_text.clone(), w.sq);
        let part_no = crate::wire_parts::wire_part_number_in(wire_parts, w).unwrap_or_default();
        *totals.entry((color, sq_text, part_no)).or_insert(0.0) += w.length_m.unwrap_or(0.0);
    }
    // 表示は線色→線径の順 (文字列ではなく実際のsq値で比較し、二桁以上のsqでも正しく並ぶ)
    let mut rows: Vec<(String, String, f64, String, String)> = totals
        .into_iter()
        .map(|((color, sq_text, part_no), total)| {
            let sq = sq_values.get(&sq_text).copied().unwrap_or(0.0);
            // 使用長は合算なので小数第2位で止める。BOMの電線集約行と同じ書式で、
            // 二進小数の誤差が桁となって図面に出ることもない
            // (0.4を8本足すと3.1999999999999997。整形せずに書くとそれがそのまま出る)
            (color, sq_text, sq, part_no, format!("{total:.2}"))
        })
        .collect();
    rows.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
            .then(a.3.cmp(&b.3))
    });
    rows.into_iter()
        .map(|(color, sq_text, _sq, part_no, total_text)| vec![color, sq_text, part_no, total_text])
        .collect()
}

/// 凡例ブロックに並ぶ行 ([`LEGEND_COLUMNS`] と同じ並び)。
pub fn legend_rows(project: &crate::model::Project, sheet: &Sheet) -> Vec<Vec<String>> {
    legend_rows_with(sheet, &project.wire_parts)
}

/// 凡例ブロックの矩形 (左上, 右下)。列見出しの1行＋本文の行数ぶんだけ下へ伸びる。
pub fn legend_rect_with(sheet: &Sheet, wire_parts: &[WirePart], legend: &WireLegend) -> (Point, Point) {
    let w: f64 = LEGEND_COL_W.iter().sum();
    let h = LEGEND_ROW_H * (1 + legend_rows_with(sheet, wire_parts).len()) as f64;
    (legend.at, Point::new(legend.at.x + w, legend.at.y + h))
}

/// 凡例ブロックの矩形 (左上, 右下)。
pub fn legend_rect(project: &crate::model::Project, sheet: &Sheet, legend: &WireLegend) -> (Point, Point) {
    legend_rect_with(sheet, &project.wire_parts, legend)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PaperSize, Orientation, Project, Sheet as ModelSheet, WirePart};
    use uuid::Uuid;

    fn wire(color: &str, sq: f64, length_m: Option<f64>, part_no: Option<&str>) -> Entity {
        Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)],
            color: color.into(),
            sq,
            length_m,
            part_no: part_no.map(|s| s.to_string()),
            net: None,
        })
    }

    fn sheet_with(entities: Vec<Entity>) -> ModelSheet {
        let mut sheet = ModelSheet::new("S1", PaperSize::A3, Orientation::Landscape);
        for e in entities {
            sheet.entities.insert(e.id(), e);
        }
        sheet
    }

    /// 抵抗器1個 (どこにも繋がっていないので、検証は「未接続のピン」を1件出す)。
    fn lone_resistor() -> Entity {
        Entity::Symbol(crate::model::SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "resistor".into(),
            at: Point::new(50.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "R1".into(),
            value: String::new(),
            attrs: Default::default(),
        })
    }

    /// 凡例ブロックを1つ置いた同じシート (「置く前」と「置いた後」を比べるための道具)。
    /// 「他の指摘が増減しない」ことを確かめられるよう、置く前の時点で指摘が1件出る図面にしてある。
    fn sheet_and_sheet_with_legend() -> (ModelSheet, ModelSheet) {
        let before = sheet_with(vec![
            wire("red", 0.75, Some(1.2), Some("SAMPLE0001")),
            wire("黒", 0.3, Some(0.4), None),
            lone_resistor(),
        ]);
        let mut after = before.clone();
        let legend = Entity::WireLegend(WireLegend {
            id: Uuid::new_v4(),
            at: Point::new(5.0, 5.0),
        });
        after.entities.insert(legend.id(), legend);
        (before, after)
    }

    /// The netlist of a sheet is the same before and after a legend block is placed on it: the block carries no electrical meaning.
    /// 凡例ブロックを置いてもシートのネットリストは変わらない (電気的な意味を持たない作図要素だから)。
    #[test]
    fn a_legend_block_does_not_appear_in_the_netlist() {
        let (before, after) = sheet_and_sheet_with_legend();
        let nets_before = crate::netlist::extract_netlist(&before, &crate::sheet_symbol_defs(&before));
        let nets_after = crate::netlist::extract_netlist(&after, &crate::sheet_symbol_defs(&after));
        assert_eq!(nets_after.len(), nets_before.len());
        let shape = |nets: &[crate::netlist::Net]| -> Vec<(String, usize, usize)> {
            nets.iter()
                .map(|n| (n.name.clone(), n.pins.len(), n.wire_ids.len()))
                .collect()
        };
        assert_eq!(shape(&nets_after), shape(&nets_before), "ネットの中身まで同じ");
    }

    /// Placing a legend block raises no verification diagnostic of its own, and leaves the sheet's other diagnostics untouched.
    /// 凡例ブロックを置いても、そのブロックについての検証指摘は出ず、他の指摘も増減しない。
    #[test]
    fn a_legend_block_raises_no_verification_diagnostic() {
        let (before, after) = sheet_and_sheet_with_legend();
        let diags_before = crate::verify::verify_sheet(&before, &crate::sheet_symbol_defs(&before));
        let diags_after = crate::verify::verify_sheet(&after, &crate::sheet_symbol_defs(&after));
        assert_eq!(diags_before.len(), 1, "置く前から指摘が1件ある図面で比べている");
        assert_eq!(diags_after.len(), 1, "凡例ブロックを置いても指摘は増えも減りもしない");
        let legend_id = legends(&after).next().expect("凡例").id;
        assert!(
            !diags_after.iter().any(|d| d.entity_ids.contains(&legend_id)),
            "凡例ブロックを名指しする指摘は無い"
        );
    }

    /// The BOM is the same before and after a legend block is placed: it is not a component and it adds no wire length.
    /// 凡例ブロックを置いても部品表は変わらない (部品でもなく、電線の使用長も増やさない)。
    #[test]
    fn a_legend_block_does_not_change_the_bom() {
        let (before, after) = sheet_and_sheet_with_legend();
        let mut p_before = Project::new("t");
        p_before.sheets = vec![before];
        let mut p_after = Project::new("t");
        p_after.sheets = vec![after];
        assert_eq!(
            crate::reports::bom_rows(&p_after),
            crate::reports::bom_rows(&p_before)
        );
    }

    /// The wire list is the same before and after a legend block is placed: the block is not a conductor.
    /// 凡例ブロックを置いても電線リストは変わらない (導体ではないから)。
    #[test]
    fn a_legend_block_does_not_change_the_wire_list() {
        let (before, after) = sheet_and_sheet_with_legend();
        let mut p_before = Project::new("t");
        p_before.sheets = vec![before];
        let mut p_after = Project::new("t");
        p_after.sheets = vec![after];
        assert_eq!(
            crate::reports::wire_list_rows(&p_after),
            crate::reports::wire_list_rows(&p_before)
        );
    }

    /// A sheet with no wires at all still gets a header-only legend block (no data rows).
    /// 配線が1本も無いシートでも、凡例ブロックは列見出しだけの表として出る。
    #[test]
    fn an_empty_sheet_has_only_the_header_row() {
        let project = Project::new("t");
        let sheet = sheet_with(vec![]);
        assert_eq!(legend_rows(&project, &sheet).len(), 0);
        let legend = WireLegend { id: Uuid::new_v4(), at: Point::new(10.0, 10.0) };
        let (min, max) = legend_rect(&project, &sheet, &legend);
        assert_eq!(min, Point::new(10.0, 10.0));
        assert_eq!(max.y - min.y, LEGEND_ROW_H, "行が無くても列見出しの1行ぶんは高さがある");
        assert_eq!(max.x - min.x, LEGEND_COL_W.iter().sum::<f64>());
    }

    /// Wires that share the same colour, gauge and resolved part number collapse into one row whose length is the sum of theirs.
    /// 線色・線径・引き当てた品番が同じ配線は1行にまとまり、使用長は合算される。
    #[test]
    fn wires_of_the_same_colour_gauge_and_part_number_are_summed_into_one_row() {
        let project = Project::new("t");
        let sheet = sheet_with(vec![
            wire("水色", 0.3, Some(1.5), Some("SAMPLE0001")),
            wire("水色", 0.3, Some(2.5), Some("SAMPLE0001")),
        ]);
        let rows = legend_rows(&project, &sheet);
        let expected: Vec<Vec<String>> = vec![vec![
            "水色".to_string(),
            "0.3".to_string(),
            "SAMPLE0001".to_string(),
            "4.00".to_string(),
        ]];
        assert_eq!(rows, expected);
    }

    /// The same colour written in English and in Japanese is one stock item, so it makes a single row whose length is the sum of both.
    /// 英語名と日本語名で書かれた同じ線色は同じ電線なので、1行にまとまり使用長も合算される。
    #[test]
    fn a_colour_written_in_english_and_in_japanese_makes_one_row() {
        let project = Project::new("t");
        let sheet = sheet_with(vec![
            wire("red", 0.75, Some(1.2), None),
            wire("赤", 0.75, Some(2.0), None),
            wire("RED", 0.75, Some(0.8), None),
        ]);
        let rows = legend_rows(&project, &sheet);
        assert_eq!(rows.len(), 1, "3本とも同じ電線なので1行");
        assert_eq!(rows[0][0], "赤", "色名は図面に印字するのと同じ表記");
        assert_eq!(rows[0][3], "4.00");
    }

    /// The colour printed in the legend is the same name the wire's own annotation prints on the sheet.
    /// 凡例に出る色名は、同じシートの配線注記が印字する色名と同じになる。
    #[test]
    fn the_legend_prints_the_same_colour_name_as_the_wire_annotation() {
        let project = Project::new("t");
        let w = wire("light_blue", 0.5, Some(1.0), None);
        let Entity::Wire(inner) = &w else { unreachable!() };
        let note = crate::wire_note::wire_note_text(inner).expect("注記");
        let sheet = sheet_with(vec![w.clone()]);
        let rows = legend_rows(&project, &sheet);
        assert_eq!(rows[0][0], "水色");
        assert!(
            note.contains(&rows[0][0]),
            "配線注記 {note:?} が凡例の色名 {:?} を含む",
            rows[0][0]
        );
    }

    /// Rows are sorted by colour, then by gauge within the same colour.
    /// 行は線色順、同じ線色の中では線径順に並ぶ。
    #[test]
    fn rows_are_sorted_by_colour_then_gauge() {
        let project = Project::new("t");
        let sheet = sheet_with(vec![
            wire("赤", 0.5, Some(1.0), None),
            wire("赤", 0.3, Some(1.0), None),
            wire("水色", 0.3, Some(1.0), None),
        ]);
        let rows = legend_rows(&project, &sheet);
        let colours_and_sq: Vec<(String, String)> =
            rows.iter().map(|r| (r[0].clone(), r[1].clone())).collect();
        assert_eq!(
            colours_and_sq,
            vec![
                ("水色".to_string(), "0.3".to_string()),
                ("赤".to_string(), "0.3".to_string()),
                ("赤".to_string(), "0.5".to_string()),
            ]
        );
    }

    /// Lengths that cannot be added up exactly in binary floating point still print as a short decimal, never as a long string of digits.
    /// 足し合わせると二進小数で誠実に表せない使用長も、桁の並んだ長い数字ではなく短い小数で表示される。
    #[test]
    fn a_summed_length_is_rounded_instead_of_showing_floating_point_noise() {
        let project = Project::new("t");
        // 0.4 を8本足すと、二進小数では 3.1999999999999997 になる
        let sheet = sheet_with((0..8).map(|_| wire("水色", 0.3, Some(0.4), None)).collect());
        let rows = legend_rows(&project, &sheet);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][3], "3.20", "使用長は小数第2位まで (BOMの電線集約行と同じ書式)");
    }

    /// A wire whose colour and gauge are not in the wire-part table shows a blank part number, instead of an invented one.
    /// 電線品番表に無い組み合わせの配線は品番欄が空欄のままで、勝手に品番が作られることはない。
    #[test]
    fn an_unresolved_part_number_shows_as_blank() {
        let mut project = Project::new("t");
        project.wire_parts = vec![WirePart {
            part_no: "SAMPLE0001".into(),
            color: "水色".into(),
            sq: 0.3,
            note: String::new(),
        }];
        let sheet = sheet_with(vec![wire("赤", 0.5, Some(1.0), None)]);
        let rows = legend_rows(&project, &sheet);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][2], "", "表に無い組み合わせは空欄");
    }
}
