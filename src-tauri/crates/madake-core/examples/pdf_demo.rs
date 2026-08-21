//! デモ図面をPDFに書き出す手動確認用example(印刷品質・フォント埋め込みの確認)。
//!
//! ```bash
//! cargo run -p madake-core --example pdf_demo -- /tmp/sheet1.pdf [/tmp/sheet2.pdf]
//! ```
//!
//! 内容はSVG出力(`render_sample`)と同一: 改訂欄・線番・ハーネス破線囲み・
//! シート間クロスリファレンスを含む2シートのデモ図面。

#[path = "demo/project.rs"]
mod demo;

use madake_core::{pdf, sheet_symbol_defs};

fn main() {
    let mut args = std::env::args().skip(1);
    let path1 = args.next().unwrap_or_else(|| "pdf_demo.pdf".into());
    let path2 = args.next();
    let project = demo::demo_project();
    let (sheet1, sheet2) = demo::demo_sheets(&project);

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
