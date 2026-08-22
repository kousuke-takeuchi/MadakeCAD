//! 同梱シンボルライブラリを一覧シートとしてSVGに書き出す目視確認用example。
//!
//! ```bash
//! cargo run -p madake-core --example symbol_sheet -- /tmp/symbols.svg
//! ```
//!
//! 参照記号=接頭辞+連番、型番欄=シンボルidを入れて、記号本体と注記の重なりも一緒に確認する。

use madake_core::model::{Entity, Orientation, PaperSize, Sheet, SymbolInstance, TextEntity};
use madake_core::symbol::builtin_symbols;
use madake_core::{svg, Point};
use uuid::Uuid;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "symbols.svg".into());
    let mut sheet = Sheet::new("Symbol library", PaperSize::A2, Orientation::Landscape);
    sheet.title_block.title = "同梱シンボルライブラリ (JIS C 0617 / IEC 60617)".into();

    let defs = builtin_symbols();
    let (cols, dx, dy) = (8usize, 55.0, 45.0);
    let (x0, y0) = (45.0, 45.0);
    let mut push = |e: Entity| {
        sheet.entities.insert(e.id(), e);
    };
    for (i, def) in defs.iter().enumerate() {
        let x = x0 + (i % cols) as f64 * dx;
        let y = y0 + (i / cols) as f64 * dy;
        push(Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: def.id.clone(),
            at: Point::new(x, y),
            rotation: 0,
            mirror: false,
            reference: format!("{}{}", def.ref_prefix, i + 1),
            value: def.id.clone(),
            attrs: Default::default(),
        }));
        push(Entity::Text(TextEntity {
            id: Uuid::new_v4(),
            at: Point::new(x - 20.0, y + 17.0),
            text: def.name_ja.clone(),
            height: 2.5,
            rotation: 0,
        }));
    }
    // 回転の追従確認: 3極機器と縦長記号を90/180/270度でも並べる
    let rotated = ["contactor_3p", "breaker_3p", "transformer", "motor_3ph"];
    let base_y = y0 + (defs.len().div_ceil(cols)) as f64 * dy + 10.0;
    for (i, id) in rotated.iter().enumerate() {
        for (j, rot) in [90u16, 180, 270].iter().enumerate() {
            push(Entity::Symbol(SymbolInstance {
                id: Uuid::new_v4(),
                symbol_id: (*id).into(),
                at: Point::new(x0 + i as f64 * 3.0 * dx + j as f64 * dx, base_y),
                rotation: *rot,
                mirror: false,
                reference: format!("{rot}"),
                value: String::new(),
                attrs: Default::default(),
            }));
        }
    }

    let out = svg::sheet_to_svg(&sheet, &defs);
    std::fs::write(&path, out).expect("書き込み");
    println!("wrote {path} ({} symbols)", defs.len());
}
