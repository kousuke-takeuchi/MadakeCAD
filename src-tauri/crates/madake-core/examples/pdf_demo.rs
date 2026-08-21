//! PDF出力の手動確認用デモ。
//! `cargo run -p madake-core --example pdf_demo -- /path/to/out.pdf`

use madake_core::{sheet_symbol_defs, Entity, Point, SymbolInstance, Wire};
use madake_core::model::{Orientation, PaperSize, Sheet};
use uuid::Uuid;

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "pdf_demo.pdf".into());
    let mut sheet = Sheet::new("TB1", PaperSize::A3, Orientation::Landscape);
    sheet.title_block.company = "サンプル株式会社".into();
    sheet.title_block.drawing_no = "MDK-001".into();
    sheet.title_block.title = "動力系統図(デモ)".into();
    sheet.title_block.scale = "1:1".into();
    sheet.title_block.date = "2026-08-21".into();
    sheet.title_block.designed = "竹内".into();

    let symbol = |symbol_id: &str, reference: &str, x: f64, y: f64| {
        Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: symbol_id.into(),
            at: Point::new(x, y),
            rotation: 0,
            mirror: false,
            reference: reference.into(),
            value: String::new(),
            attrs: Default::default(),
        })
    };
    let wire = |pts: &[(f64, f64)], color: &str| {
        Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: pts.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            color: color.into(),
            sq: 0.75,
            length_m: None,
            part_no: None,
            net: None,
        })
    };
    for e in [
        symbol("battery", "BT1", 60.0, 100.0),
        symbol("fuse", "F1", 105.0, 100.0),
        symbol("switch_spst", "SW1", 150.0, 100.0),
        symbol("terminal_block_4p", "TB1", 200.0, 105.0),
        symbol("connector_3p", "J1", 250.0, 100.0),
        symbol("lamp", "L1", 310.0, 100.0),
        wire(&[(67.5, 100.0), (97.5, 100.0)], "red"),
        wire(&[(112.5, 100.0), (142.5, 100.0)], "red"),
        wire(&[(157.5, 100.0), (197.5, 100.0)], "red"),
        wire(&[(202.5, 100.0), (302.5, 100.0)], "black"),
    ] {
        sheet.entities.insert(e.id(), e);
    }
    let pdf = madake_core::pdf::sheet_to_pdf(&sheet, &sheet_symbol_defs(&sheet)).expect("pdf");
    std::fs::write(&out, &pdf).expect("write");
    println!("written: {out} ({} bytes)", pdf.len());
}
