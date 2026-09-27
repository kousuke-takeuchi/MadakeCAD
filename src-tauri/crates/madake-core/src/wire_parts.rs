//! 電線品番の引き当て (G11)。
//!
//! `Project.wire_parts` は「線色+線径 → 電線品番」の対応表 (電線の在庫表)。
//! 図面のワイヤは色と太さだけを持ち、品番はこの表から引く。
//!
//! ```text
//! wire_parts: [ { part_no: "SAMPLE0001", color: "水色", sq: 0.3 }, … ]
//! wire:       { color: "水色", sq: 0.3 }        → 品番 SAMPLE0001
//! ```
//!
//! ## 決めごと
//!
//! - **ワイヤに直接書かれた品番 ([`Wire::part_no`]) が最優先**。表から引いた品番は、
//!   品番欄が空のワイヤにだけ入る (手で決めた品番を表が上書きしない)
//! - 引き当ては**線色は前後の空白を無視・英字の大小同一視・英日の色名を同一視**
//!   (`light_blue` = `水色`)、線径は完全一致 (浮動小数の誤差 [`SQ_EPS`] まで)。
//!   同じ組み合わせが表に2行あれば**先に書いた行**が勝つ
//! - 線色が空・線径が0のワイヤは引き当てない (何色の何sqか決まっていないため)
//! - 表に無い組み合わせは**空欄のまま**。勝手に品番を作らない
//!
//! 引き当てを使うのは電線リスト (From-To、[`crate::reports::wire_list_rows`]) の品番列と、
//! プロパティの「品番 (自動)」表示。端子接続図の電線仕様欄は今のところワイヤに書かれた
//! 品番だけを出す (シート単位の帳票でプロジェクトの表を持たないため)。

use crate::model::{Project, Wire, WirePart};
use crate::{Command, CoreError, EditOrigin, Engine, Patch, Result};

/// 線径 (sq) の一致とみなす誤差。
pub const SQ_EPS: f64 = 1e-9;

/// 電線品番表のCSV列。4列目 (備考) は省略できる。
pub const WIRE_PARTS_COLUMNS: [&str; 4] = ["線色", "線径sq", "電線品番", "備考"];

/// 線色の比較キー。前後の空白を落として小文字化し、**日本語の色名へそろえる**
/// ([`crate::wire_note::color_display`])。図面のワイヤが`light_blue`でも、表に「水色」と
/// 書いてあれば同じ電線として引ける (表を作る人と図面を描く道具で表記が違うため)。
///
/// 図面へ色名を印字する関数と**同じ**なので、引き当てのキーと図面の表記は必ず一致する。
fn color_key(color: &str) -> String {
    crate::wire_note::color_display(color)
}

/// 線色+線径で電線品番表 (スライス) を引く。表の**先頭に近い行**が勝つ。
/// [`find_wire_part`]の本体で、シート単体しか無い文脈 (プロジェクト全体を持たない
/// SVG書き出しの一部の入口) から表だけを渡して引くのにも使う ([`crate::wire_legend`])。
pub fn find_wire_part_in<'a>(parts: &'a [WirePart], color: &str, sq: f64) -> Option<&'a WirePart> {
    let key = color_key(color);
    if key.is_empty() || sq <= 0.0 {
        return None;
    }
    parts
        .iter()
        .find(|p| color_key(&p.color) == key && (p.sq - sq).abs() < SQ_EPS)
}

/// 線色+線径で電線品番表を引く。表の**先頭に近い行**が勝つ。
pub fn find_wire_part<'a>(project: &'a Project, color: &str, sq: f64) -> Option<&'a WirePart> {
    find_wire_part_in(&project.wire_parts, color, sq)
}

/// ワイヤの電線品番 (表はスライスで渡す)。[`wire_part_number`]の本体。
pub fn wire_part_number_in(parts: &[WirePart], wire: &Wire) -> Option<String> {
    if let Some(no) = wire.part_no.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        return Some(no.to_string());
    }
    find_wire_part_in(parts, &wire.color, wire.sq).map(|p| p.part_no.clone())
}

/// ワイヤの電線品番。**ワイヤに書かれた品番が最優先**で、無ければ線色+線径で
/// 電線品番表から引く。どちらも無ければ`None` (帳票では空欄)。
pub fn wire_part_number(project: &Project, wire: &Wire) -> Option<String> {
    wire_part_number_in(&project.wire_parts, wire)
}

/// 電線品番表のCSV ([`WIRE_PARTS_COLUMNS`] の4列)。[`parse_wire_parts_csv`]で読み戻せる。
pub fn wire_parts_csv(project: &Project) -> String {
    let mut out = WIRE_PARTS_COLUMNS.join(",");
    out.push('\n');
    for p in &project.wire_parts {
        out.push_str(&crate::reports::csv_row(&[
            p.color.clone(),
            crate::reports::fmt_num(p.sq),
            p.part_no.clone(),
            p.note.clone(),
        ]));
        out.push('\n');
    }
    out
}

/// 電線品番表のCSVを読む。列は 線色・線径sq・電線品番 (+備考) で、見出し行はあってもなくてもよい。
///
/// 列数が合わない行・線径が数値でない行・品番が空の行が1つでもあればエラーにして
/// **1行も採用しない** (半端に取り込まれた表を残さない)。空行は読み飛ばす。
pub fn parse_wire_parts_csv(text: &str) -> Result<Vec<WirePart>> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.trim_end_matches('\r');
        if line.trim().is_empty() {
            continue;
        }
        let cells = crate::plc::split_csv_line(line);
        if i == 0 && cells.first().map(|c| c.trim()) == Some(WIRE_PARTS_COLUMNS[0]) {
            continue; // 見出し行
        }
        let row = |n: usize| cells.get(n).map(|c| c.trim().to_string()).unwrap_or_default();
        if !(3..=WIRE_PARTS_COLUMNS.len()).contains(&cells.len()) {
            return Err(CoreError::InvalidCommand(format!(
                "{}行目: 列数が{}です ({}の3列で書いてください。4列目の備考は省略できます)",
                i + 1,
                cells.len(),
                WIRE_PARTS_COLUMNS[..3].join("・")
            )));
        }
        let sq: f64 = row(1).parse().map_err(|_| {
            CoreError::InvalidCommand(format!(
                "{}行目: 線径sqが数値ではありません: {:?}",
                i + 1,
                row(1)
            ))
        })?;
        if sq <= 0.0 {
            return Err(CoreError::InvalidCommand(format!(
                "{}行目: 線径sqは0より大きい値です: {sq}",
                i + 1
            )));
        }
        if row(2).is_empty() {
            return Err(CoreError::InvalidCommand(format!(
                "{}行目: 電線品番が空です",
                i + 1
            )));
        }
        out.push(WirePart {
            part_no: row(2),
            color: row(0),
            sq,
            note: row(3),
        });
    }
    Ok(out)
}

/// 電線品番表のCSVを取り込む (表全体を置き換える)。**Commandを通す**ので
/// undo一発で元の表に戻る。CSVが読めなければ表は一切変わらない。
pub fn import_wire_parts_csv(engine: &mut Engine, csv: &str, origin: EditOrigin) -> Result<Patch> {
    let wire_parts = parse_wire_parts_csv(csv)?;
    engine.execute_as(Command::SetWireParts { wire_parts }, origin)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::*;
    use uuid::Uuid;

    fn part(part_no: &str, color: &str, sq: f64) -> WirePart {
        WirePart {
            part_no: part_no.into(),
            color: color.into(),
            sq,
            note: String::new(),
        }
    }

    fn wire(color: &str, sq: f64, part_no: Option<&str>) -> Wire {
        Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)],
            color: color.into(),
            sq,
            length_m: None,
            length_source: crate::model::LengthSource::Manual,
            part_no: part_no.map(|s| s.to_string()),
            net: None,
        }
    }

    fn project_with(parts: Vec<WirePart>) -> Project {
        let mut project = Project::new("t");
        project.wire_parts = parts;
        project
    }

    /// A wire's part number is looked up in the project's wire-part table by its colour and gauge.
    /// ワイヤの電線品番は、線色と線径からプロジェクトの電線品番表を引いて決まる。
    #[test]
    fn a_wire_takes_its_part_number_from_the_colour_and_gauge_table() {
        let project = project_with(vec![
            part("SAMPLE0001", "水色", 0.3),
            part("SAMPLE0002", "白", 0.75),
        ]);
        assert_eq!(
            wire_part_number(&project, &wire("白", 0.75, None)).as_deref(),
            Some("SAMPLE0002")
        );
    }

    /// A colour and gauge combination that is not in the table leaves the part number empty; no number is invented.
    /// 表に無い線色+線径の組み合わせは品番が空のままで、勝手に品番が作られることはない。
    #[test]
    fn an_unregistered_combination_stays_empty() {
        let project = project_with(vec![part("SAMPLE0001", "水色", 0.3)]);
        assert_eq!(wire_part_number(&project, &wire("水色", 0.75, None)), None, "太さ違い");
        assert_eq!(wire_part_number(&project, &wire("赤", 0.3, None)), None, "色違い");
        assert_eq!(wire_part_number(&Project::new("t"), &wire("水色", 0.3, None)), None, "表が空");
    }

    /// A part number written on the wire itself wins over the table, so a hand-picked part is never overwritten.
    /// ワイヤ自身に書かれた品番は表より優先されるので、手で決めた品番が表で上書きされることはない。
    #[test]
    fn a_part_number_written_on_the_wire_wins_over_the_table() {
        let project = project_with(vec![part("SAMPLE0001", "水色", 0.3)]);
        assert_eq!(
            wire_part_number(&project, &wire("水色", 0.3, Some("MY-WIRE-9"))).as_deref(),
            Some("MY-WIRE-9")
        );
        assert_eq!(
            wire_part_number(&project, &wire("水色", 0.3, Some("  "))).as_deref(),
            Some("SAMPLE0001"),
            "空白だけの品番は書かれていないのと同じ"
        );
    }

    /// The lookup ignores surrounding spaces and upper/lower case in the colour name.
    /// 引き当ては線色の前後の空白と英字の大小を無視する。
    #[test]
    fn the_lookup_ignores_spaces_and_letter_case_in_the_colour() {
        let project = project_with(vec![part("SAMPLE0003", " Light_Blue ", 0.5)]);
        assert_eq!(
            wire_part_number(&project, &wire("light_blue", 0.5, None)).as_deref(),
            Some("SAMPLE0003")
        );
        assert_eq!(
            wire_part_number(&project, &wire(" LIGHT_BLUE", 0.5, None)).as_deref(),
            Some("SAMPLE0003")
        );
    }

    /// The English colour name on a wire and the Japanese colour name in the table mean the same wire, so a table written in Japanese serves a drawing drawn with English colour tokens.
    /// ワイヤの英語の色名と表の日本語の色名は同じ電線を指すので、日本語で書いた表がそのまま英語の色名の図面に効く。
    #[test]
    fn the_english_and_japanese_names_of_a_colour_mean_the_same_wire() {
        let project = project_with(vec![part("SAMPLE0001", "水色", 0.3), part("SAMPLE0005", "灰", 1.25)]);
        assert_eq!(
            wire_part_number(&project, &wire("light_blue", 0.3, None)).as_deref(),
            Some("SAMPLE0001")
        );
        assert_eq!(
            wire_part_number(&project, &wire("grey", 1.25, None)).as_deref(),
            Some("SAMPLE0005"),
            "grey も gray も灰"
        );
        // 逆向き (表が英語・図面が日本語) も同じ
        let english_table = project_with(vec![part("SAMPLE0006", "light_blue", 0.5)]);
        assert_eq!(
            wire_part_number(&english_table, &wire("水色", 0.5, None)).as_deref(),
            Some("SAMPLE0006")
        );
    }

    /// A wire with no colour or no gauge is not looked up at all.
    /// 線色が空・線径が0のワイヤは引き当てない。
    #[test]
    fn a_wire_without_a_colour_or_a_gauge_is_not_looked_up() {
        let project = project_with(vec![part("SAMPLE0001", "水色", 0.3), part("SAMPLE0004", "", 0.3)]);
        assert_eq!(wire_part_number(&project, &wire("", 0.3, None)), None);
        assert_eq!(wire_part_number(&project, &wire("水色", 0.0, None)), None);
    }

    /// When the table has two rows for the same colour and gauge, the row written first wins.
    /// 同じ線色+線径の行が表に2つあるときは、先に書いた行が勝つ。
    #[test]
    fn the_first_row_wins_when_the_table_has_duplicates() {
        let project = project_with(vec![part("FIRST", "黒", 1.25), part("SECOND", "黒", 1.25)]);
        assert_eq!(
            wire_part_number(&project, &wire("黒", 1.25, None)).as_deref(),
            Some("FIRST")
        );
    }

    /// The wire-part table round trips through CSV: colour, gauge, part number and note come back unchanged.
    /// 電線品番表はCSVで往復でき、線色・線径・品番・備考がそのまま戻る。
    #[test]
    fn the_wire_part_table_round_trips_through_csv() {
        let mut project = project_with(vec![
            part("SAMPLE0001", "水色", 0.3),
            WirePart {
                part_no: "SAMPLE0002".into(),
                color: "白".into(),
                sq: 0.75,
                note: "耐熱, 100m巻".into(),
            },
        ]);
        let csv = wire_parts_csv(&project);
        assert_eq!(csv.lines().next().unwrap(), "線色,線径sq,電線品番,備考");
        assert!(csv.contains("\"耐熱, 100m巻\""), "カンマ入りはクォートされる: {csv}");
        let rows = parse_wire_parts_csv(&csv).expect("読める");
        project.wire_parts = rows;
        assert_eq!(project.wire_parts.len(), 2);
        assert_eq!(project.wire_parts[0].part_no, "SAMPLE0001");
        assert_eq!(project.wire_parts[0].sq, 0.3);
        assert_eq!(project.wire_parts[1].color, "白");
        assert_eq!(project.wire_parts[1].note, "耐熱, 100m巻");
    }

    /// A CSV without a header row is read as data, and the note column may be left out.
    /// 見出し行の無いCSVもそのままデータとして読め、備考の列は省略できる。
    #[test]
    fn a_csv_without_a_header_row_and_without_the_note_column_is_read() {
        let rows = parse_wire_parts_csv("水色,0.3,SAMPLE0001\n白,0.75,SAMPLE0002\n").expect("読める");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].color, "水色");
        assert_eq!(rows[0].note, "");
        assert_eq!(rows[1].part_no, "SAMPLE0002");
    }

    /// A malformed CSV (wrong number of columns, a non-numeric gauge, or an empty part number) is refused and no row is taken.
    /// 壊れたCSV (列数違い・線径が数値でない・品番が空) は拒否され、1行も取り込まれない。
    #[test]
    fn a_malformed_csv_is_refused() {
        assert!(parse_wire_parts_csv("水色,0.3\n").is_err(), "列が足りない");
        assert!(parse_wire_parts_csv("水色,0.3,A,備考,余分\n").is_err(), "列が多い");
        assert!(parse_wire_parts_csv("水色,ほそい,SAMPLE0001\n").is_err(), "線径が数値でない");
        assert!(parse_wire_parts_csv("水色,0,SAMPLE0001\n").is_err(), "線径が0");
        assert!(parse_wire_parts_csv("水色,0.3,\n").is_err(), "品番が空");
    }

    /// Importing a CSV replaces the whole table through the command engine, and one undo brings the previous table back.
    /// CSVの取り込みはCommandエンジンを通して表全体を置き換え、undo一発で元の表に戻る。
    #[test]
    fn importing_a_csv_replaces_the_table_and_undo_restores_it() {
        let mut engine = Engine::new(project_with(vec![part("OLD0001", "赤", 2.0)]));
        import_wire_parts_csv(&mut engine, "水色,0.3,SAMPLE0001\n", EditOrigin::Mcp)
            .expect("取り込める");
        assert_eq!(engine.project().wire_parts.len(), 1);
        assert_eq!(engine.project().wire_parts[0].part_no, "SAMPLE0001");
        engine.undo().expect("undoできる");
        assert_eq!(engine.project().wire_parts[0].part_no, "OLD0001");
    }

    /// A CSV that cannot be read leaves the table exactly as it was.
    /// 読めないCSVを取り込もうとしても、表は元のままである。
    #[test]
    fn a_csv_that_cannot_be_read_leaves_the_table_unchanged() {
        let mut engine = Engine::new(project_with(vec![part("OLD0001", "赤", 2.0)]));
        assert!(import_wire_parts_csv(&mut engine, "水色,0.3\n", EditOrigin::Mcp).is_err());
        assert_eq!(engine.project().wire_parts[0].part_no, "OLD0001");
        assert!(!engine.can_undo(), "履歴も積まれない");
    }
}
