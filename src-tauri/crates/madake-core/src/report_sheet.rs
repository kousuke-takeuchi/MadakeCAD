//! 帳票の図面シート化。CSVと同じ内容を**図枠+表題欄付きのA4横ページ**として描き、
//! 回路図と同じ紙にまとめて印刷・PDF化できるようにする。
//!
//! - 汎用テーブルページ [`report_sheet_svg`] が全帳票の土台。行が1ページに収まらなければ
//!   自動でページを分け、各ページに列見出しを再掲する
//! - 各帳票 (From-Toリスト・端子台チャート・部品表・クロスリファレンス表) は行データを
//!   それぞれのモジュール (`reports` / `terminal_chart` / `xref`) から取り、ここでは描画だけを行う
//! - 表紙 [`cover_sheet_svg`] はPDF一括出力 ([`crate::pdf::export_project_pdf`]) の1ページ目

use std::fmt::Write as _;

use crate::model::{EntityId, PaperSize, Project};
use crate::svg::{
    line_el, n, rect_el, text_el, CELL_PAD, FRAME_MARGIN, FRAME_STROKE, ROW_H, RULE_STROKE,
    TITLE_FONT, TITLE_W,
};

/// 帳票ページの用紙 (A4横)。回路図がA3でも帳票はA4に統一する。
pub const REPORT_PAPER: PaperSize = PaperSize::A4;
/// 帳票名 (ページ左上) の文字高さ (mm)。
const REPORT_TITLE_FONT: f64 = 5.0;
/// 図枠上端から帳票名ベースラインまで (mm)。
const REPORT_TITLE_BASELINE: f64 = 7.0;
/// 帳票名の下から表の上端までの余白 (mm)。
const TITLE_GAP: f64 = 3.0;
/// 帳票ページの表題欄の行数 (プロジェクト行・日付/ページ行)。
const REPORT_TITLE_ROWS: f64 = 2.0;
/// 表題欄のラベル列の幅 (mm)。
const LABEL_COL_W: f64 = 24.0;
/// 表と表題欄の間の余白 (mm)。
const TABLE_BOTTOM_GAP: f64 = 3.0;
/// 表の本文行の高さ (mm)。
const TABLE_ROW_H: f64 = 6.0;
/// 表の列見出し行の高さ (mm)。
const TABLE_HEADER_H: f64 = 7.0;
/// 表の本文・列見出しの文字高さ (mm)。
const TABLE_FONT: f64 = 2.6;
/// 半角1文字の幅 (文字高さに対する比)。全角はこの2倍として省略位置を見積もる。
const HALF_CHAR_RATIO: f64 = 0.55;
/// 列幅を超えたセルの末尾に付ける記号。
const ELLIPSIS: &str = "…";
/// 表紙のプロジェクト名の文字高さ (mm)。
const COVER_NAME_FONT: f64 = 8.0;
/// 表紙の見出し・注記の文字高さ (mm)。
const COVER_NOTE_FONT: f64 = 3.0;
/// 表紙のシート一覧の列見出し。
pub const COVER_SHEET_COLUMNS: [&str; 5] = ["No", "シート名", "図番", "品名", "Rev"];

/// 帳票ページの体裁 (表題欄に入れる情報と列幅の相対比)。
#[derive(Debug, Clone, Default)]
pub struct ReportMeta {
    /// 表題欄の「プロジェクト」欄。
    pub project_name: String,
    /// 表題欄の「日付」欄。
    pub date: String,
    /// 列幅の相対比。列数と長さが一致するときだけ採用し、それ以外 (空を含む) は等分にする。
    pub col_ratios: Vec<f64>,
}

impl ReportMeta {
    /// プロジェクトから既定の体裁を作る (日付は1枚目のシートの表題欄から借りる)。
    pub fn from_project(project: &Project) -> Self {
        Self {
            project_name: project.name.clone(),
            date: project
                .sheets
                .first()
                .map(|s| s.title_block.date.clone())
                .unwrap_or_default(),
            col_ratios: Vec::new(),
        }
    }

    /// 列幅の相対比を指定した体裁を返す。
    pub fn with_col_ratios(mut self, ratios: &[f64]) -> Self {
        self.col_ratios = ratios.to_vec();
        self
    }
}

/// PDF一括出力に含められる帳票の種類。JSON表記はCLI・Link APIと同じケバブケース
/// (`"wire-list"` / `"terminal-chart"` / `"terminal-diagram"` / `"bom"` / `"xref"`)。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum ReportKind {
    /// From-To電線リスト。
    WireList,
    /// 端子台チャート (端子台1つにつき1表)。
    TerminalChart,
    /// 端子接続図 (端子台1つにつき1ページのグラフィカル図)。
    TerminalDiagram,
    /// 部品表。
    Bom,
    /// クロスリファレンス表 (ネット所在一覧)。
    Xref,
}

impl ReportKind {
    /// 帳票名 (ページ左上と表題欄に出る)。
    pub fn title(self) -> &'static str {
        match self {
            ReportKind::WireList => "From-To 電線リスト",
            ReportKind::TerminalChart => "端子台チャート",
            ReportKind::TerminalDiagram => "端子接続図",
            ReportKind::Bom => "部品表 (BOM)",
            ReportKind::Xref => "クロスリファレンス表",
        }
    }

    /// この帳票の図面ページ (1要素=1ページのSVG)。データが空でも見出しだけのページを1枚返す。
    pub fn pages(self, project: &Project) -> Vec<String> {
        match self {
            ReportKind::WireList => wire_list_sheet_svg(project),
            ReportKind::TerminalChart => terminal_charts_sheet_svg(project),
            ReportKind::TerminalDiagram => {
                crate::terminal_diagram::terminal_diagrams_svg(project)
            }
            ReportKind::Bom => bom_sheet_svg(project),
            ReportKind::Xref => xref_table_sheet_svg(project),
        }
    }
}

/// 表の1ページに入る本文行数。用紙・行高から決まる固定値。
pub fn rows_per_page() -> usize {
    let body = body_bottom() - table_top() - TABLE_HEADER_H;
    ((body / TABLE_ROW_H).floor() as usize).max(1)
}

/// 帳票ページの本文の上端Y (mm)。帳票名の下。
pub(crate) fn table_top() -> f64 {
    FRAME_MARGIN + REPORT_TITLE_BASELINE + TITLE_GAP
}

/// 帳票ページの本文の下端Y (mm)。表題欄の上。ここより下には描かない。
pub(crate) fn body_bottom() -> f64 {
    let (_, ph) = REPORT_PAPER.dimensions_mm();
    ph - FRAME_MARGIN - ROW_H * REPORT_TITLE_ROWS - TABLE_BOTTOM_GAP
}

/// 文字列の表示幅 (半角=1, 全角=2)。
fn text_units(s: &str) -> f64 {
    s.chars().map(|c| if c.is_ascii() { 1.0 } else { 2.0 }).sum()
}

/// セル内テキストを列幅に収める。溢れる場合は末尾を落として [`ELLIPSIS`] を付ける。
pub(crate) fn fit_text(s: &str, max_w: f64, font: f64) -> String {
    let unit = font * HALF_CHAR_RATIO;
    if max_w <= 0.0 {
        return String::new();
    }
    if text_units(s) * unit <= max_w {
        return s.to_string();
    }
    // 省略記号 (全角1文字) の分を残す
    let budget = max_w / unit - 2.0;
    let mut out = String::new();
    let mut used = 0.0;
    for c in s.chars() {
        let w = if c.is_ascii() { 1.0 } else { 2.0 };
        if used + w > budget {
            break;
        }
        out.push(c);
        used += w;
    }
    out.push_str(ELLIPSIS);
    out
}

/// 列の左端X (列数+1個。最後の要素は表の右端)。
fn column_lefts(x: f64, width: f64, columns: usize, ratios: &[f64]) -> Vec<f64> {
    let cols = columns.max(1);
    let equal = vec![1.0; cols];
    let ratios = if ratios.len() == cols && ratios.iter().all(|r| *r > 0.0) {
        ratios
    } else {
        &equal
    };
    let total: f64 = ratios.iter().sum();
    let mut lefts = Vec::with_capacity(cols + 1);
    let mut acc = 0.0;
    for r in ratios {
        lefts.push(x + width * acc / total);
        acc += r;
    }
    lefts.push(x + width);
    lefts
}

/// 表 (外枠・列見出し・罫線・セル) を描く。`rows`はこのページに載る分だけ渡す。
fn draw_table(out: &mut String, x: f64, top: f64, width: f64, columns: &[&str], rows: &[Vec<String>], ratios: &[f64]) {
    let lefts = column_lefts(x, width, columns.len(), ratios);
    let h = TABLE_HEADER_H + TABLE_ROW_H * rows.len() as f64;
    rect_el(out, x, top, width, h, "#fff", "#000", FRAME_STROKE);
    // 列見出し
    let head_base = top + TABLE_HEADER_H / 2.0 + TABLE_FONT / 2.0;
    for (i, label) in columns.iter().enumerate() {
        let w = lefts[i + 1] - lefts[i] - CELL_PAD * 2.0;
        text_el(
            out,
            lefts[i] + CELL_PAD,
            head_base,
            TABLE_FONT,
            "#000",
            "start",
            &fit_text(label, w, TABLE_FONT),
        );
    }
    line_el(out, x, top + TABLE_HEADER_H, x + width, top + TABLE_HEADER_H, FRAME_STROKE, "#000");
    // 本文
    for (r, row) in rows.iter().enumerate() {
        let row_top = top + TABLE_HEADER_H + TABLE_ROW_H * r as f64;
        if r > 0 {
            line_el(out, x, row_top, x + width, row_top, RULE_STROKE, "#000");
        }
        let base = row_top + TABLE_ROW_H / 2.0 + TABLE_FONT / 2.0;
        for (i, cell) in row.iter().take(columns.len()).enumerate() {
            if cell.is_empty() {
                continue;
            }
            let w = lefts[i + 1] - lefts[i] - CELL_PAD * 2.0;
            text_el(
                out,
                lefts[i] + CELL_PAD,
                base,
                TABLE_FONT,
                "#000",
                "start",
                &fit_text(cell, w, TABLE_FONT),
            );
        }
    }
    // 列の区切り線 (表の上端から下端まで)
    for left in lefts.iter().take(columns.len()).skip(1) {
        line_el(out, *left, top, *left, top + h, RULE_STROKE, "#000");
    }
}

/// ページ共通: SVGの開始タグ+白地+図枠。左上に帳票名を書く。
pub(crate) fn page_open(title: &str) -> String {
    let (pw, ph) = REPORT_PAPER.dimensions_mm();
    let mut out = String::new();
    let _ = write!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}mm\" height=\"{}mm\" viewBox=\"0 0 {} {}\">\n",
        n(pw), n(ph), n(pw), n(ph)
    );
    let _ = write!(
        out,
        "<rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"#ffffff\"/>\n",
        n(pw), n(ph)
    );
    rect_el(
        &mut out,
        FRAME_MARGIN,
        FRAME_MARGIN,
        pw - 2.0 * FRAME_MARGIN,
        ph - 2.0 * FRAME_MARGIN,
        "none",
        "#000",
        FRAME_STROKE,
    );
    text_el(
        &mut out,
        FRAME_MARGIN + CELL_PAD,
        FRAME_MARGIN + REPORT_TITLE_BASELINE,
        REPORT_TITLE_FONT,
        "#000",
        "start",
        title,
    );
    out
}

/// 帳票ページの表題欄 (右下)。プロジェクト名・帳票名・日付・ページ n/N を入れる。
pub(crate) fn report_title_block(
    out: &mut String,
    title: &str,
    meta: &ReportMeta,
    page: usize,
    total: usize,
) {
    let (pw, ph) = REPORT_PAPER.dimensions_mm();
    let h = ROW_H * REPORT_TITLE_ROWS;
    let tx = pw - FRAME_MARGIN - TITLE_W;
    let ty = ph - FRAME_MARGIN - h;
    rect_el(out, tx, ty, TITLE_W, h, "#fff", "#000", FRAME_STROKE);
    line_el(out, tx, ty + ROW_H, tx + TITLE_W, ty + ROW_H, RULE_STROKE, "#000");
    line_el(out, tx + LABEL_COL_W, ty, tx + LABEL_COL_W, ty + h, RULE_STROKE, "#000");
    let rows: [(&str, String); 2] = [
        ("プロジェクト", meta.project_name.clone()),
        ("帳票", format!("{}    {}    {}/{}", title, meta.date, page, total)),
    ];
    let value_w = TITLE_W - LABEL_COL_W - CELL_PAD * 2.0;
    for (i, (label, value)) in rows.iter().enumerate() {
        let base = ty + ROW_H * i as f64 + ROW_H / 2.0 + TITLE_FONT / 2.0;
        text_el(out, tx + CELL_PAD, base, TITLE_FONT, "#000", "start", label);
        text_el(
            out,
            tx + LABEL_COL_W + CELL_PAD,
            base,
            TITLE_FONT,
            "#000",
            "start",
            &fit_text(value, value_w, TITLE_FONT),
        );
    }
}

/// 汎用の帳票ページ。表 (列見出し+行) を図枠・表題欄付きのA4横ページとして描く。
///
/// 戻り値は1要素=1ページのSVG文書。行が1ページに収まらないときは
/// [`rows_per_page`] ごとに分割し、**どのページにも列見出しを再掲**する。
/// 行が0件でも列見出しだけのページを1枚返す。列幅は `meta.col_ratios` の相対比、
/// 指定がなければ等分。列幅に収まらないセルは末尾を省略記号にする。
pub fn report_sheet_svg(
    title: &str,
    columns: &[&str],
    rows: &[Vec<String>],
    meta: &ReportMeta,
) -> Vec<String> {
    let (pw, _) = REPORT_PAPER.dimensions_mm();
    let width = pw - 2.0 * FRAME_MARGIN;
    let per_page = rows_per_page();
    let chunks: Vec<&[Vec<String>]> = if rows.is_empty() {
        vec![&[]]
    } else {
        rows.chunks(per_page).collect()
    };
    let total = chunks.len();
    chunks
        .into_iter()
        .enumerate()
        .map(|(i, chunk)| {
            let mut out = page_open(title);
            draw_table(
                &mut out,
                FRAME_MARGIN,
                table_top(),
                width,
                columns,
                chunk,
                &meta.col_ratios,
            );
            report_title_block(&mut out, title, meta, i + 1, total);
            out.push_str("</svg>\n");
            out
        })
        .collect()
}

/// From-To電線リストの図面ページ。内容はCSV ([`crate::reports::wire_list_csv`]) と同一。
pub fn wire_list_sheet_svg(project: &Project) -> Vec<String> {
    let meta = ReportMeta::from_project(project)
        // シート・From・To・線番を広めに、線色/sq/長さを狭く
        .with_col_ratios(&[10.0, 16.0, 16.0, 10.0, 8.0, 6.0, 6.0, 14.0, 10.0]);
    report_sheet_svg(
        ReportKind::WireList.title(),
        &crate::reports::WIRE_LIST_COLUMNS,
        &crate::reports::wire_list_rows(project),
        &meta,
    )
}

/// 端子台1つのチャートの図面ページ。`tb_id`が端子台シンボルでなければNone。
/// ページ左上の帳票名は「端子台チャート TB1」のように参照記号入りになる。
pub fn terminal_chart_sheet_svg(project: &Project, tb_id: EntityId) -> Option<Vec<String>> {
    let chart = project
        .sheets
        .iter()
        .find_map(|s| crate::terminal_chart::terminal_chart(s, tb_id))?;
    let meta = ReportMeta::from_project(project)
        .with_col_ratios(&[6.0, 20.0, 10.0, 24.0, 20.0, 8.0]);
    let title = format!("{} {}", ReportKind::TerminalChart.title(), chart.reference);
    Some(report_sheet_svg(
        &title,
        &crate::terminal_chart::TERMINAL_CHART_COLUMNS,
        &chart.cells(),
        &meta,
    ))
}

/// プロジェクト内の全端子台チャートの図面ページ (1端子台=1表、シート順→参照記号順)。
/// 端子台が1つも無ければ空の表を1ページ返す。
pub fn terminal_charts_sheet_svg(project: &Project) -> Vec<String> {
    let ids = crate::terminal_chart::terminal_block_ids(project);
    if ids.is_empty() {
        return report_sheet_svg(
            ReportKind::TerminalChart.title(),
            &crate::terminal_chart::TERMINAL_CHART_COLUMNS,
            &[],
            &ReportMeta::from_project(project),
        );
    }
    ids.into_iter()
        .filter_map(|id| terminal_chart_sheet_svg(project, id))
        .flatten()
        .collect()
}

/// 部品表の図面ページ。内容はCSV ([`crate::reports::bom_csv`]) と同一。
pub fn bom_sheet_svg(project: &Project) -> Vec<String> {
    let meta =
        ReportMeta::from_project(project).with_col_ratios(&[30.0, 24.0, 30.0, 8.0]);
    report_sheet_svg(
        ReportKind::Bom.title(),
        &crate::reports::BOM_COLUMNS,
        &crate::reports::bom_rows(project),
        &meta,
    )
}

/// クロスリファレンス表 (ネット所在一覧) の図面ページ。
pub fn xref_table_sheet_svg(project: &Project) -> Vec<String> {
    let meta =
        ReportMeta::from_project(project).with_col_ratios(&[16.0, 10.0, 40.0, 10.0, 16.0]);
    report_sheet_svg(
        ReportKind::Xref.title(),
        &crate::xref::XREF_TABLE_COLUMNS,
        &crate::xref::xref_table_rows(project),
        &meta,
    )
}

/// 全シートのうち最も新しい改訂 (日付が最大。同日ならシート順で後のもの)。改訂が無ければNone。
pub fn latest_revision(project: &Project) -> Option<(String, &crate::model::Revision)> {
    project
        .sheets
        .iter()
        .enumerate()
        .flat_map(|(i, s)| s.revisions.iter().enumerate().map(move |(j, r)| (i, j, s, r)))
        .max_by(|a, b| (&a.3.date, a.0, a.1).cmp(&(&b.3.date, b.0, b.1)))
        .map(|(_, _, s, r)| (s.name.clone(), r))
}

/// PDF一括出力の表紙 (1ページ)。プロジェクト名・シート一覧・最新改訂を載せる。
/// シート一覧が1ページに収まらない場合、末尾の行に残り枚数を記す。
pub fn cover_sheet_svg(project: &Project) -> String {
    let (pw, _) = REPORT_PAPER.dimensions_mm();
    let width = pw - 2.0 * FRAME_MARGIN;
    let mut out = page_open("");
    // プロジェクト名 (大きく) と最新改訂
    let name_y = FRAME_MARGIN + COVER_NAME_FONT * 2.0;
    text_el(
        &mut out,
        FRAME_MARGIN + CELL_PAD,
        name_y,
        COVER_NAME_FONT,
        "#000",
        "start",
        &fit_text(&project.name, width - CELL_PAD * 2.0, COVER_NAME_FONT),
    );
    let rev_line = match latest_revision(project) {
        Some((sheet_name, r)) => format!(
            "最新改訂: {} {} {} ({} / {})",
            r.mark, r.date, r.description, r.by, sheet_name
        ),
        None => "最新改訂: なし".to_string(),
    };
    let rev_y = name_y + COVER_NOTE_FONT * 2.0;
    text_el(
        &mut out,
        FRAME_MARGIN + CELL_PAD,
        rev_y,
        COVER_NOTE_FONT,
        "#000",
        "start",
        &fit_text(&rev_line, width - CELL_PAD * 2.0, COVER_NOTE_FONT),
    );
    // シート一覧
    let list_top = rev_y + COVER_NOTE_FONT * 2.0;
    text_el(
        &mut out,
        FRAME_MARGIN + CELL_PAD,
        list_top,
        COVER_NOTE_FONT,
        "#000",
        "start",
        "図面一覧",
    );
    let mut rows: Vec<Vec<String>> = project
        .sheets
        .iter()
        .enumerate()
        .map(|(i, s)| {
            vec![
                (i + 1).to_string(),
                s.name.clone(),
                s.title_block.drawing_no.clone(),
                s.title_block.title.clone(),
                crate::svg::effective_rev(s),
            ]
        })
        .collect();
    // 表紙は1ページ。溢れる分は最終行にまとめて残数を記す
    let table_top = list_top + COVER_NOTE_FONT;
    let cap = (((body_bottom() - table_top - TABLE_HEADER_H) / TABLE_ROW_H).floor() as usize).max(1);
    if rows.len() > cap {
        let rest = rows.len() - cap + 1;
        rows.truncate(cap - 1);
        rows.push(vec![String::new(), format!("… 他 {rest} 枚"), String::new(), String::new(), String::new()]);
    }
    draw_table(
        &mut out,
        FRAME_MARGIN,
        table_top,
        width,
        &COVER_SHEET_COLUMNS,
        &rows,
        &[6.0, 30.0, 24.0, 32.0, 8.0],
    );
    report_title_block(
        &mut out,
        "表紙",
        &ReportMeta::from_project(project),
        1,
        1,
    );
    out.push_str("</svg>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{Command, Engine};
    use crate::geometry::Point;
    use crate::model::*;
    use uuid::Uuid;

    fn meta() -> ReportMeta {
        ReportMeta {
            project_name: "デモ盤".into(),
            date: "2026-08-22".into(),
            col_ratios: Vec::new(),
        }
    }

    fn rows(n: usize) -> Vec<Vec<String>> {
        (0..n)
            .map(|i| vec![format!("R{i}"), format!("V{i}")])
            .collect()
    }

    /// 描画テキスト(`>…<`)を順番に取り出す。
    fn texts(svg: &str) -> Vec<String> {
        svg.split("<text")
            .skip(1)
            .filter_map(|s| {
                let body = s.split_once('>')?.1;
                Some(body.split_once("</text>")?.0.to_string())
            })
            .collect()
    }

    /// A report page is an A4 landscape sheet with the JIS frame, the report title and a title block carrying project name, date and page number.
    /// 帳票ページはA4横で、JIS図枠・帳票名・プロジェクト名/日付/ページ番号入りの表題欄を持つ。
    #[test]
    fn report_page_has_frame_title_and_title_block() {
        let pages = report_sheet_svg("電線リスト", &["A", "B"], &rows(3), &meta());
        assert_eq!(pages.len(), 1);
        let svg = &pages[0];
        assert!(svg.contains("width=\"297mm\" height=\"210mm\""), "{svg}");
        assert!(svg.contains("viewBox=\"0 0 297 210\""), "{svg}");
        // 図枠 (用紙端から10mm)
        assert!(
            svg.contains("<rect x=\"10\" y=\"10\" width=\"277\" height=\"190\""),
            "{svg}"
        );
        let t = texts(svg);
        assert!(t.contains(&"電線リスト".to_string()), "{t:?}");
        assert!(t.contains(&"デモ盤".to_string()), "{t:?}");
        assert!(
            t.iter().any(|s| s.contains("2026-08-22") && s.contains("1/1")),
            "{t:?}"
        );
    }

    /// Every column header and every cell of the data rows is drawn on the page.
    /// 列見出しと全データ行のセルがページに描かれる。
    #[test]
    fn report_page_draws_headers_and_all_cells() {
        let pages = report_sheet_svg("t", &["端子", "線番"], &rows(3), &meta());
        let t = texts(&pages[0]);
        for expected in ["端子", "線番", "R0", "V0", "R2", "V2"] {
            assert!(t.contains(&expected.to_string()), "{expected} が無い: {t:?}");
        }
    }

    /// A table with no rows still produces exactly one page showing the column headers.
    /// 行が0件でも列見出しだけのページを1枚出す。
    #[test]
    fn report_page_with_no_rows_still_shows_headers() {
        let pages = report_sheet_svg("t", &["端子", "線番"], &[], &meta());
        assert_eq!(pages.len(), 1);
        let t = texts(&pages[0]);
        assert!(t.contains(&"端子".to_string()), "{t:?}");
    }

    /// Rows that do not fit on one page continue on further pages, each repeating the column headers and numbering pages n/N.
    /// 1ページに収まらない行は次ページへ続き、各ページに列見出しを再掲してページ番号 n/N を振る。
    #[test]
    fn report_pages_split_and_repeat_headers() {
        let per = rows_per_page();
        let data = rows(per + 3);
        let pages = report_sheet_svg("t", &["A", "B"], &data, &meta());
        assert_eq!(pages.len(), 2, "{}行 / 1ページ{per}行", data.len());
        for (i, page) in pages.iter().enumerate() {
            let t = texts(page);
            assert!(t.contains(&"A".to_string()), "ページ{}に見出し", i + 1);
            assert!(
                t.iter().any(|s| s.contains(&format!("{}/2", i + 1))),
                "ページ番号 {}/2: {t:?}",
                i + 1
            );
        }
        // 全行がどこかのページに1回だけ出る
        let all: Vec<String> = pages.iter().flat_map(|p| texts(p)).collect();
        for i in 0..data.len() {
            assert_eq!(
                all.iter().filter(|s| *s == &format!("R{i}")).count(),
                1,
                "R{i}"
            );
        }
    }

    /// A cell longer than its column is cut off and ends with an ellipsis, so text never overruns the column.
    /// 列幅より長いセルは途中で切られ末尾が省略記号になる(文字が列からはみ出さない)。
    #[test]
    fn report_page_truncates_cells_wider_than_the_column() {
        let long = "あいうえおかきくけこさしすせそたちつてとなにぬねのはひふへほまみむめも";
        // 1列目を全体の1/10 (約27.7mm) に絞ると、この長さは収まらない
        let m = meta().with_col_ratios(&[1.0, 9.0]);
        let pages = report_sheet_svg("t", &["A", "B"], &[vec![long.into(), "x".into()]], &m);
        let t = texts(&pages[0]);
        assert!(!t.contains(&long.to_string()), "全文が出ている: {t:?}");
        let cell = t.iter().find(|s| s.starts_with("あいうえお")).expect("省略セル");
        assert!(cell.ends_with('…'), "{cell}");
        assert!(cell.chars().count() < long.chars().count(), "{cell}");
    }

    /// Column widths follow the given ratios; without ratios the columns are equal in width.
    /// 列幅は指定した相対比に従い、指定が無ければ等分になる。
    #[test]
    fn report_page_column_widths_follow_ratios() {
        // 等分: 2列なら区切り線は表の中央 (10 + 277/2 = 148.5)
        let equal = report_sheet_svg("t", &["A", "B"], &rows(1), &meta());
        assert!(equal[0].contains("x1=\"148.500\""), "{}", equal[0]);
        // 3:1 なら区切りは 10 + 277*0.75 = 217.75
        let m = meta().with_col_ratios(&[3.0, 1.0]);
        let ratio = report_sheet_svg("t", &["A", "B"], &rows(1), &m);
        assert!(ratio[0].contains("x1=\"217.750\""), "{}", ratio[0]);
    }

    /// Special XML characters in the data are escaped so the page stays valid SVG.
    /// データ中のXML特殊文字はエスケープされ、ページは正しいSVGのままになる。
    #[test]
    fn report_page_escapes_xml_special_characters() {
        let pages = report_sheet_svg("t", &["A"], &[vec!["<K1 & \"R\">".into()]], &meta());
        assert!(pages[0].contains("&lt;K1 &amp;"), "{}", pages[0]);
    }

    /// 4極端子台TB1 (100,50) の端子1の内部側に抵抗R1を繋いだ1シートのプロジェクト。
    /// 全編集はCommandエンジン経由で組み立てる。
    fn tb_project() -> (Project, EntityId) {
        let mut engine = Engine::new(Project::new("端子台デモ"));
        let sheet_id = engine.project().sheets[0].id;
        let tb = Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "terminal_block_4p".into(),
            at: Point::new(100.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "TB1".into(),
            value: "端子台 BN4P".into(),
            attrs: Default::default(),
        });
        let tb_id = tb.id();
        let r1 = Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "resistor".into(),
            at: Point::new(85.0, 42.5),
            rotation: 0,
            mirror: false,
            reference: "R1".into(),
            value: "10kΩ".into(),
            attrs: Default::default(),
        });
        let wire = Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(92.5, 42.5), Point::new(97.5, 42.5)],
            color: "red".into(),
            sq: 0.75,
            length_m: None,
            part_no: None,
            net: None,
        });
        for entity in [tb, r1, wire] {
            engine
                .execute(Command::AddEntity { sheet_id, entity })
                .expect("追加");
        }
        engine
            .execute(Command::RenumberWires {
                sheet_id: None,
                mode: crate::wire_no::RenumberMode::Append,
                start: 1,
            })
            .expect("線番");
        (engine.project().clone(), tb_id)
    }

    /// The terminal chart sheet carries the reference designator in its title and one table row per terminal, matching the chart data.
    /// 端子台チャートのシートは帳票名に参照記号を含み、チャートのデータと同じ行を端子ごとに並べる。
    #[test]
    fn terminal_chart_sheet_matches_chart_rows() {
        let (project, tb) = tb_project();
        let pages = terminal_chart_sheet_svg(&project, tb).expect("端子台");
        let t = texts(&pages[0]);
        assert!(
            t.iter().any(|s| s.contains("端子台チャート") && s.contains("TB1")),
            "{t:?}"
        );
        for head in crate::terminal_chart::TERMINAL_CHART_COLUMNS {
            assert!(t.contains(&head.to_string()), "{head}: {t:?}");
        }
        let chart = crate::terminal_chart::terminal_chart(&project.sheets[0], tb).expect("チャート");
        assert_eq!(chart.rows.len(), 4);
        for cell in chart.cells().iter().flatten().filter(|c| !c.is_empty()) {
            assert!(t.contains(cell), "{cell}: {t:?}");
        }
    }

    /// Asking for a terminal chart sheet of something that is not a terminal block yields no page at all.
    /// 端子台でないエンティティのチャートシートは1ページも出ない。
    #[test]
    fn terminal_chart_sheet_is_none_for_other_entities() {
        let (project, _) = tb_project();
        assert!(terminal_chart_sheet_svg(&project, Uuid::new_v4()).is_none());
    }

    /// The From-To wire list sheet uses the same columns and rows as the CSV report.
    /// From-To電線リストのシートはCSV帳票と同じ列・同じ行を使う。
    #[test]
    fn wire_list_sheet_matches_report_rows() {
        let (project, _) = tb_project();
        let pages = wire_list_sheet_svg(&project);
        let t = texts(&pages[0]);
        for head in crate::reports::WIRE_LIST_COLUMNS {
            assert!(t.contains(&head.to_string()), "{head}: {t:?}");
        }
        let data = crate::reports::wire_list_rows(&project);
        assert_eq!(data.len(), 1);
        assert!(t.contains(&"TB1:1".to_string()), "{t:?}");
    }

    /// The BOM sheet lists every part group with its reference designators and quantity.
    /// 部品表のシートは部品グループごとに参照記号と数量を並べる。
    #[test]
    fn bom_sheet_lists_parts_with_quantity() {
        let (project, _) = tb_project();
        let t = texts(&bom_sheet_svg(&project)[0]);
        assert!(t.contains(&"参照記号".to_string()), "{t:?}");
        assert!(t.contains(&"TB1".to_string()), "{t:?}");
        assert!(t.contains(&"端子台 BN4P".to_string()), "{t:?}");
        assert!(t.contains(&"terminal_block_4p".to_string()), "{t:?}");
    }

    /// The cross-reference sheet lists each net with the pins it connects and the sheet it appears on.
    /// クロスリファレンス表のシートはネットごとに接続先ピンと現れるシートを並べる。
    #[test]
    fn xref_table_sheet_lists_nets_and_pins() {
        let (project, _) = tb_project();
        let t = texts(&xref_table_sheet_svg(&project)[0]);
        for head in crate::xref::XREF_TABLE_COLUMNS {
            assert!(t.contains(&head.to_string()), "{head}: {t:?}");
        }
        assert!(t.iter().any(|s| s.contains("TB1:1")), "{t:?}");
    }

    /// The cover page shows the project name, every sheet with its drawing number, and the newest revision of the whole project.
    /// 表紙にはプロジェクト名・全シートと図番・プロジェクト全体で最新の改訂が出る。
    #[test]
    fn cover_page_shows_project_sheets_and_latest_revision() {
        let mut project = Project::new("受電盤 制御図");
        project.sheets[0].title_block.drawing_no = "MDK-001".into();
        project.sheets[0].title_block.title = "動力系統図".into();
        project.sheets[0].revisions = vec![Revision {
            mark: "A".into(),
            date: "2026-01-05".into(),
            description: "初版".into(),
            by: "山田".into(),
        }];
        let mut s2 = Sheet::new("2 現場配線", PaperSize::A3, Orientation::Landscape);
        s2.title_block.drawing_no = "MDK-002".into();
        s2.revisions = vec![Revision {
            mark: "B".into(),
            date: "2026-03-10".into(),
            description: "端子台追加".into(),
            by: "佐藤".into(),
        }];
        project.sheets.push(s2);
        let t = texts(&cover_sheet_svg(&project));
        assert!(t.contains(&"受電盤 制御図".to_string()), "{t:?}");
        assert!(t.contains(&"図面一覧".to_string()), "{t:?}");
        assert!(t.contains(&"MDK-001".to_string()), "{t:?}");
        assert!(t.contains(&"MDK-002".to_string()), "{t:?}");
        assert!(t.contains(&"2 現場配線".to_string()), "{t:?}");
        // 最新 = 日付が最大の改訂 (B)
        let rev = t.iter().find(|s| s.starts_with("最新改訂")).expect("改訂行");
        assert!(rev.contains('B') && rev.contains("2026-03-10"), "{rev}");
    }

    /// A project without any revision says so on the cover instead of leaving the line blank.
    /// 改訂が1件も無いプロジェクトの表紙は空欄ではなく「なし」と記す。
    #[test]
    fn cover_page_states_when_there_is_no_revision() {
        let project = Project::new("無改訂");
        let t = texts(&cover_sheet_svg(&project));
        assert!(t.contains(&"最新改訂: なし".to_string()), "{t:?}");
    }

    /// Report kinds are serialized with the same kebab-case names the CLI and Link API use.
    /// 帳票の種類はCLI・Link APIと同じケバブケース表記でJSONへ入る。
    #[test]
    fn report_kind_json_names_match_cli_spelling() {
        let json = serde_json::to_string(&ReportKind::WireList).expect("JSON");
        assert_eq!(json, "\"wire-list\"");
        assert_eq!(
            serde_json::from_str::<ReportKind>("\"terminal-chart\"").expect("JSON"),
            ReportKind::TerminalChart
        );
    }
}
