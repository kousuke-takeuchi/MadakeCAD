//! デモ図面をPDFに書き出す手動確認用example(印刷品質・フォント埋め込みの確認)。
//!
//! ```bash
//! # シート単体 (1ページ)
//! cargo run -p madake-core --example pdf_demo -- /tmp/sheet1.pdf [/tmp/sheet2.pdf]
//! # 一括出力 (表紙+回路図全シート+全帳票)
//! cargo run -p madake-core --example pdf_demo -- --book /tmp/book.pdf
//! ```
//!
//! 内容はSVG出力(`render_sample`)と同一: 改訂欄・線番・ハーネス破線囲み・
//! シート間クロスリファレンスを含む2シートのデモ図面。

#[path = "demo/project.rs"]
mod demo;

use madake_core::pdf::{export_project_pdf, project_pdf_pages, PdfBookOptions};
use madake_core::report_sheet::ReportKind;
use madake_core::{pdf, sheet_symbol_defs};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let project = demo::demo_project();

    if args.first().map(String::as_str) == Some("--book") {
        args.remove(0);
        let path = args.first().cloned().unwrap_or_else(|| "pdf_demo_book.pdf".into());
        let options = PdfBookOptions {
            include_reports: vec![
                ReportKind::WireList,
                ReportKind::TerminalChart,
                ReportKind::Bom,
                ReportKind::Xref,
            ],
            cover: true,
        };
        let pages = project_pdf_pages(&project, &options);
        let bytes = export_project_pdf(&project, &options).expect("PDF出力");
        std::fs::write(&path, &bytes).expect("書き込み");
        println!("written: {path} ({} bytes, {} pages)", bytes.len(), pages.len());
        return;
    }

    let (sheet1, sheet2) = demo::demo_sheets(&project);
    let path1 = args.first().cloned().unwrap_or_else(|| "pdf_demo.pdf".into());
    let path2 = args.get(1).cloned();
    for (sheet_id, path) in [(sheet1, Some(path1)), (sheet2, path2)] {
        let Some(path) = path else { continue };
        let sheet = project.sheet(sheet_id).expect("シート");
        let bytes = pdf::project_sheet_to_pdf(&project, sheet_id, &sheet_symbol_defs(sheet))
            .expect("シート")
            .expect("PDF出力");
        std::fs::write(&path, &bytes).expect("書き込み");
        println!("written: {path} ({} bytes, {})", bytes.len(), sheet.name);
    }
}
