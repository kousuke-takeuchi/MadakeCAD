//! デモ図面をSVGに書き出す動作確認用example(README画像の生成元)。
//!
//! ```bash
//! cargo run -p madake-core --example render_sample -- /tmp/sheet1.svg [/tmp/sheet2.svg]
//! ```
//!
//! 1つ目の引数がシート1、2つ目(任意)がシート2の出力先。プロジェクト文脈で描くので、
//! ネットラベルにはシート間クロスリファレンス(「/2.B4」等)が入る。

#[path = "demo/project.rs"]
mod demo;

use madake_core::{sheet_symbol_defs, svg};

fn main() {
    let mut args = std::env::args().skip(1);
    let path1 = args.next().unwrap_or_else(|| "sample.svg".into());
    let path2 = args.next();
    let project = demo::demo_project();
    let (sheet1, sheet2) = demo::demo_sheets(&project);

    for (sheet_id, path) in [(sheet1, Some(path1)), (sheet2, path2)] {
        let Some(path) = path else { continue };
        let sheet = project.sheet(sheet_id).expect("シート");
        let out = svg::project_sheet_to_svg(&project, sheet_id, &sheet_symbol_defs(sheet))
            .expect("SVG出力");
        std::fs::write(&path, out).expect("書き込み");
        println!("wrote {path} ({})", sheet.name);
    }
}
