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
//! - 行は**線色→線径**の順に並べる (同じ組み合わせは無いのでこれで一意に決まる)
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
        let color = w.color.trim().to_string();
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
            (color, sq_text, sq, part_no, crate::reports::fmt_num(total))
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
            "4".to_string(),
        ]];
        assert_eq!(rows, expected);
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
