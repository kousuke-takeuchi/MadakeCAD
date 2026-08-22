//! 印刷品質のPDFエクスポート。[`crate::svg::sheet_to_svg`]の出力を
//! svg2pdfでベクタのままPDF化する(SVGが単一の描画ソース)。
//!
//! 複数のSVGページ (回路図シート・帳票ページ) を1つのPDF文書へ束ねる
//! [`export_project_pdf`] が図面一式の配布物になる。

use std::collections::HashMap;
use std::sync::OnceLock;

use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref};
use svg2pdf::usvg;

use crate::model::{Project, Sheet};
use crate::report_sheet::ReportKind;
use crate::symbol::{sheet_symbol_defs, SymbolDef};

/// SVGのユーザー単位が前提とするDPI ([`usvg::Options::dpi`] の既定)。
/// PDFの単位は1/72インチなので、この比で縮めるとmm 1:1の用紙寸法になる。
const SVG_USER_UNIT_DPI: f32 = 96.0;
/// 1インチあたりのPDFポイント数。
const PDF_POINTS_PER_INCH: f32 = 72.0;
/// ページ内容として置くSVG XObjectの名前。
const PAGE_XOBJECT: Name<'static> = Name(b"S1");

#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error("svg parse error: {0}")]
    Svg(String),
    #[error("pdf conversion error: {0}")]
    Convert(String),
}

/// PDF一括出力の内容。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct PdfBookOptions {
    /// 回路図の後ろに付ける帳票 (指定した順に並ぶ)。空なら帳票ページを付けない。
    #[serde(default)]
    pub include_reports: Vec<ReportKind>,
    /// 先頭に表紙を付けるか (既定: 付ける)。
    #[serde(default = "default_cover")]
    pub cover: bool,
}

fn default_cover() -> bool {
    true
}

impl Default for PdfBookOptions {
    fn default() -> Self {
        Self {
            include_reports: Vec::new(),
            cover: true,
        }
    }
}

/// フォントDB。システムフォント走査は高コストなのでプロセスで1回だけ行う。
fn fontdb() -> &'static usvg::fontdb::Database {
    static DB: OnceLock<usvg::fontdb::Database> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        db.load_system_fonts();
        // SVGはfont-family="sans-serif"。日本語グリフを持つ書体へ割り当てる
        #[cfg(target_os = "macos")]
        {
            db.set_sans_serif_family("Hiragino Sans");
            // 線番は font-family="monospace"
            db.set_monospace_family("Menlo");
        }
        db
    })
}

/// シート1枚をPDF文書(バイト列)として書き出す。用紙寸法はmm 1:1。
/// シート単体の出力なのでシート間クロスリファレンスは描かない
/// (描くには [`project_sheet_to_pdf`] を使う)。
pub fn sheet_to_pdf(sheet: &Sheet, symbols: &[SymbolDef]) -> Result<Vec<u8>, PdfError> {
    svg_to_pdf(crate::svg::sheet_to_svg(sheet, symbols))
}

/// プロジェクト内の1シートをPDFとして書き出す。ネットラベルの脇に他シートの同名ラベルの
/// 住所「/シート.ゾーン」が入る。シートが見つからなければNone。
pub fn project_sheet_to_pdf(
    project: &crate::model::Project,
    sheet_id: crate::model::SheetId,
    symbols: &[SymbolDef],
) -> Option<Result<Vec<u8>, PdfError>> {
    crate::svg::project_sheet_to_svg(project, sheet_id, symbols).map(svg_to_pdf)
}

/// PDF一括出力のページ順を組み立てる (1要素=1ページのSVG)。
/// **表紙 → 回路図の全シート → 選択した帳票**の順。表紙・帳票は [`PdfBookOptions`] で選ぶ。
pub fn project_pdf_pages(project: &Project, options: &PdfBookOptions) -> Vec<String> {
    let mut pages = Vec::new();
    if options.cover {
        pages.push(crate::report_sheet::cover_sheet_svg(project));
    }
    for sheet in &project.sheets {
        if let Some(svg) = crate::svg::project_sheet_to_svg(project, sheet.id, &sheet_symbol_defs(sheet))
        {
            pages.push(svg);
        }
    }
    for kind in &options.include_reports {
        pages.extend(kind.pages(project));
    }
    pages
}

/// プロジェクト一式を1つのPDF文書として書き出す (表紙+回路図全シート+選択帳票)。
/// ページの用紙寸法はそれぞれの元シートのまま (回路図A3・帳票A4の混在も可)。
pub fn export_project_pdf(
    project: &Project,
    options: &PdfBookOptions,
) -> Result<Vec<u8>, PdfError> {
    svgs_to_pdf(&project_pdf_pages(project, options))
}

/// SVG文字列を1枚1ページとして1つのPDF文書に束ねる。
/// ページ寸法はSVGの`width`/`height` (mm) をそのままptへ換算した値になる。
pub fn svgs_to_pdf(svgs: &[String]) -> Result<Vec<u8>, PdfError> {
    let mut alloc = Ref::new(1);
    let catalog_id = alloc.bump();
    let page_tree_id = alloc.bump();
    let mut pdf = Pdf::new();
    let mut page_ids: Vec<Ref> = Vec::with_capacity(svgs.len());

    for svg in svgs {
        let tree = parse_svg(svg)?;
        let (chunk, svg_ref) = svg2pdf::to_chunk(&tree, svg2pdf::ConversionOptions::default())
            .map_err(|e| PdfError::Convert(e.to_string()))?;
        // 各SVGは独立に1から採番されているので、文書全体の採番へ振り直す
        let mut map: HashMap<Ref, Ref> = HashMap::new();
        let chunk = chunk.renumber(|old| *map.entry(old).or_insert_with(|| alloc.bump()));
        let svg_ref = *map
            .get(&svg_ref)
            .ok_or_else(|| PdfError::Convert("svg xobject not found in chunk".into()))?;

        let (w, h) = page_size_pt(&tree);
        let page_id = alloc.bump();
        let content_id = alloc.bump();
        let mut page = pdf.page(page_id);
        page.media_box(Rect::new(0.0, 0.0, w, h));
        page.parent(page_tree_id);
        page.contents(content_id);
        let mut resources = page.resources();
        resources.x_objects().pair(PAGE_XOBJECT, svg_ref);
        resources.finish();
        page.finish();

        // XObjectは1pt角なので、用紙いっぱいへ伸ばして置く
        let mut content = Content::new();
        content.transform([w, 0.0, 0.0, h, 0.0, 0.0]);
        content.x_object(PAGE_XOBJECT);
        pdf.stream(content_id, &content.finish());
        pdf.extend(&chunk);
        page_ids.push(page_id);
    }

    pdf.catalog(catalog_id).pages(page_tree_id);
    pdf.pages(page_tree_id)
        .count(page_ids.len() as i32)
        .kids(page_ids);
    Ok(pdf.finish())
}

fn parse_svg(svg: &str) -> Result<usvg::Tree, PdfError> {
    let mut options = usvg::Options::default();
    *options.fontdb_mut() = fontdb().clone();
    usvg::Tree::from_str(svg, &options).map_err(|e| PdfError::Svg(e.to_string()))
}

/// SVGの寸法 (96dpiのユーザー単位) からPDFのページ寸法 (pt) を求める。
fn page_size_pt(tree: &usvg::Tree) -> (f32, f32) {
    let scale = PDF_POINTS_PER_INCH / SVG_USER_UNIT_DPI;
    (tree.size().width() * scale, tree.size().height() * scale)
}

fn svg_to_pdf(svg: String) -> Result<Vec<u8>, PdfError> {
    svgs_to_pdf(std::slice::from_ref(&svg))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::*;
    use crate::report_sheet::ReportKind;
    use crate::symbol::sheet_symbol_defs;
    use uuid::Uuid;

    /// PDF内の全ページの /MediaBox [0 0 w h] を (幅, 高さ) ptで取り出す。
    fn media_boxes(pdf: &[u8]) -> Vec<(f64, f64)> {
        let needle = b"/MediaBox [0 0 ";
        let mut out = Vec::new();
        let mut from = 0;
        while let Some(i) = pdf[from..]
            .windows(needle.len())
            .position(|w| w == needle)
            .map(|p| from + p)
        {
            let rest = &pdf[i + needle.len()..];
            let end = rest.iter().position(|b| *b == b']').expect("MediaBoxの終端");
            let text = std::str::from_utf8(&rest[..end]).expect("MediaBox");
            let nums: Vec<f64> = text
                .split_whitespace()
                .map(|s| s.parse().expect("数値"))
                .collect();
            out.push((nums[0], nums[1]));
            from = i + needle.len();
        }
        out
    }

    /// PDFのページ数 (/Count)。
    fn page_count(pdf: &[u8]) -> usize {
        let needle = b"/Count ";
        let i = pdf
            .windows(needle.len())
            .position(|w| w == needle)
            .expect("/Count");
        let rest = &pdf[i + needle.len()..];
        let end = rest.iter().position(|b| !b.is_ascii_digit()).expect("終端");
        std::str::from_utf8(&rest[..end]).expect("数値").parse().expect("数値")
    }

    /// 2シート (A3横) + 端子台TB1 + 改訂を持つデモプロジェクト。
    fn book_project() -> Project {
        let mut project = Project::new("PDF一括デモ");
        project.sheets[0].title_block.drawing_no = "MDK-001".into();
        project.sheets[0].revisions = vec![Revision {
            mark: "A".into(),
            date: "2026-02-01".into(),
            description: "初版".into(),
            by: "山田".into(),
        }];
        let tb = Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "terminal_block_4p".into(),
            at: Point::new(100.0, 100.0),
            rotation: 0,
            mirror: false,
            reference: "TB1".into(),
            value: "BN 4P".into(),
            attrs: Default::default(),
        });
        let wire = Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(50.0, 92.5), Point::new(97.5, 92.5)],
            color: "red".into(),
            sq: 0.75,
            length_m: None,
            part_no: None,
            net: None,
        });
        for e in [tb, wire] {
            project.sheets[0].entities.insert(e.id(), e);
        }
        let mut s2 = Sheet::new("2 現場配線", PaperSize::A3, Orientation::Landscape);
        s2.title_block.drawing_no = "MDK-002".into();
        project.sheets.push(s2);
        project
    }

    /// PDF export produces a valid PDF document (%PDF- header) of non-trivial size, including Japanese text.
    /// PDF出力は日本語を含む正しいPDF文書(%PDF-ヘッダ)を非自明なサイズで生成する。
    #[test]
    fn sheet_to_pdf_produces_pdf_bytes() {
        let mut sheet = Sheet::new("TB1", PaperSize::A3, Orientation::Landscape);
        sheet.title_block.drawing_no = "MDK-001".into();
        sheet.title_block.title = "動力系統図".into();
        let s = Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "terminal_block_4p".into(),
            at: Point::new(100.0, 100.0),
            rotation: 0,
            mirror: false,
            reference: "TB1".into(),
            value: String::new(),
            attrs: Default::default(),
        });
        let w = Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(50.0, 92.5), Point::new(97.5, 92.5)],
            color: "red".into(),
            sq: 0.75,
            length_m: None,
            part_no: None,
            net: None,
        });
        for e in [s, w] {
            sheet.entities.insert(e.id(), e);
        }
        let pdf = sheet_to_pdf(&sheet, &sheet_symbol_defs(&sheet)).expect("pdf");
        assert!(pdf.starts_with(b"%PDF-"), "PDFヘッダ");
        assert!(pdf.len() > 1000, "非自明なサイズ: {}", pdf.len());
    }

    /// The PDF page is exactly the size of the paper (A3 landscape = 420x297mm), so printing at 100% is 1:1.
    /// PDFのページ寸法は用紙そのもの (A3横=420×297mm) になり、100%で印刷すると原寸になる。
    #[test]
    fn pdf_page_is_the_size_of_the_paper() {
        let sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let pdf = sheet_to_pdf(&sheet, &sheet_symbol_defs(&sheet)).expect("pdf");
        let (w, h) = media_boxes(&pdf)[0];
        // 1mm = 72/25.4 pt
        assert!((w - 420.0 * 72.0 / 25.4).abs() < 0.1, "幅 {w}pt");
        assert!((h - 297.0 * 72.0 / 25.4).abs() < 0.1, "高さ {h}pt");
    }

    /// A PDF book is ordered cover page, then every circuit sheet, then the selected report pages.
    /// PDF一括出力のページは 表紙 → 回路図の全シート → 選択した帳票 の順に並ぶ。
    #[test]
    fn pdf_book_is_cover_then_sheets_then_reports() {
        let project = book_project();
        let options = PdfBookOptions {
            include_reports: vec![ReportKind::WireList, ReportKind::TerminalChart],
            cover: true,
        };
        let pages = project_pdf_pages(&project, &options);
        // 表紙1 + 回路2 + From-To 1 + 端子台チャート 1
        assert_eq!(pages.len(), 5, "ページ数");
        assert!(pages[0].contains("図面一覧"), "1枚目は表紙");
        assert!(pages[1].contains("MDK-001"), "2枚目は1シート目の回路図");
        assert!(pages[2].contains("MDK-002"), "3枚目は2シート目の回路図");
        assert!(pages[3].contains("From-To"), "4枚目はFrom-Toリスト");
        assert!(pages[4].contains("TB1"), "5枚目は端子台チャート");
    }

    /// With no reports selected a PDF book holds just the cover and the circuit sheets.
    /// 帳票を選ばなければPDF一括出力は表紙と回路図シートだけになる。
    #[test]
    fn pdf_book_without_reports_is_cover_and_sheets_only() {
        let project = book_project();
        let pages = project_pdf_pages(&project, &PdfBookOptions::default());
        assert_eq!(pages.len(), 3);
    }

    /// The cover page can be turned off, leaving the circuit sheets first.
    /// 表紙は外すことができ、その場合は回路図シートが先頭になる。
    #[test]
    fn pdf_book_can_omit_the_cover() {
        let project = book_project();
        let options = PdfBookOptions { include_reports: Vec::new(), cover: false };
        let pages = project_pdf_pages(&project, &options);
        assert_eq!(pages.len(), 2);
        assert!(pages[0].contains("MDK-001"), "1枚目から回路図");
    }

    /// Exporting the book writes one PDF document holding every page, with report pages on A4 even when the circuit is A3.
    /// 一括出力は全ページを1つのPDF文書にまとめ、回路図がA3でも帳票ページはA4になる。
    #[test]
    fn pdf_book_merges_every_page_into_one_document() {
        let project = book_project();
        let options = PdfBookOptions {
            include_reports: vec![ReportKind::Bom],
            cover: true,
        };
        let pdf = export_project_pdf(&project, &options).expect("PDF");
        assert!(pdf.starts_with(b"%PDF-"), "PDFヘッダ");
        let boxes = media_boxes(&pdf);
        assert_eq!(boxes.len(), 4, "表紙+回路2+BOM");
        assert_eq!(page_count(&pdf), 4, "/Count");
        let a4 = (297.0 * 72.0 / 25.4, 210.0 * 72.0 / 25.4);
        let a3 = (420.0 * 72.0 / 25.4, 297.0 * 72.0 / 25.4);
        for (i, expected) in [a4, a3, a3, a4].iter().enumerate() {
            assert!(
                (boxes[i].0 - expected.0).abs() < 0.1 && (boxes[i].1 - expected.1).abs() < 0.1,
                "{}ページ目 {:?} != {:?}",
                i + 1,
                boxes[i],
                expected
            );
        }
    }

    /// A book of a project with no sheet at all still produces a valid one-page PDF (the cover).
    /// シートが1枚も無いプロジェクトでも、表紙だけの正しい1ページPDFになる。
    #[test]
    fn pdf_book_of_an_empty_project_is_just_the_cover() {
        let mut project = Project::new("空");
        project.sheets.clear();
        let pdf = export_project_pdf(&project, &PdfBookOptions::default()).expect("PDF");
        assert!(pdf.starts_with(b"%PDF-"));
        assert_eq!(page_count(&pdf), 1);
    }
}
