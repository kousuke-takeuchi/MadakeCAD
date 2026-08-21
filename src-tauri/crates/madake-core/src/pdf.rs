//! 印刷品質のPDFエクスポート。[`crate::svg::sheet_to_svg`]の出力を
//! svg2pdfでベクタのままPDF化する(SVGが単一の描画ソース)。

use std::sync::OnceLock;

use svg2pdf::usvg;

use crate::model::Sheet;
use crate::symbol::SymbolDef;

#[derive(Debug, thiserror::Error)]
pub enum PdfError {
    #[error("svg parse error: {0}")]
    Svg(String),
    #[error("pdf conversion error: {0}")]
    Convert(String),
}

/// フォントDB。システムフォント走査は高コストなのでプロセスで1回だけ行う。
fn fontdb() -> &'static usvg::fontdb::Database {
    static DB: OnceLock<usvg::fontdb::Database> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        db.load_system_fonts();
        // SVGはfont-family="sans-serif"。日本語グリフを持つ書体へ割り当てる
        #[cfg(target_os = "macos")]
        db.set_sans_serif_family("Hiragino Sans");
        db
    })
}

/// シート1枚をPDF文書(バイト列)として書き出す。用紙寸法はmm 1:1。
pub fn sheet_to_pdf(sheet: &Sheet, symbols: &[SymbolDef]) -> Result<Vec<u8>, PdfError> {
    let svg = crate::svg::sheet_to_svg(sheet, symbols);
    let mut options = usvg::Options::default();
    *options.fontdb_mut() = fontdb().clone();
    let tree = usvg::Tree::from_str(&svg, &options).map_err(|e| PdfError::Svg(e.to_string()))?;
    svg2pdf::to_pdf(
        &tree,
        svg2pdf::ConversionOptions::default(),
        svg2pdf::PageOptions::default(),
    )
    .map_err(|e| PdfError::Convert(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::*;
    use crate::symbol::sheet_symbol_defs;
    use uuid::Uuid;

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
}
