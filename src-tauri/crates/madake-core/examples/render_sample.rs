//! サンプル図面をSVGに書き出す動作確認用example。
//! cargo run -p madake-core --example render_sample -- /tmp/out.svg

use madake_core::*;
use uuid::Uuid;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "sample.svg".into());
    let mut sheet = Sheet::new("TB1 前部ボックス", PaperSize::A3, Orientation::Landscape);
    sheet.title_block = TitleBlock {
        company: "MadakeCAD".into(),
        title: "48V 電源系統図".into(),
        drawing_no: "MDK-0001".into(),
        scale: "1:5".into(),
        date: "2026-08-20".into(),
        designed: "K.T".into(),
        drawn: "K.T".into(),
        checked: "-".into(),
        approved: "-".into(),
        rev: "A".into(),
    };
    let mut add = |e: Entity| {
        sheet.entities.insert(e.id(), e);
    };
    let sym = |id: &str, r: &str, v: &str, x: f64, y: f64, rot: u16| {
        Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: id.into(),
            at: Point::new(x, y),
            rotation: rot,
            mirror: false,
            reference: r.into(),
            value: v.into(),
            attrs: Default::default(),
        })
    };
    let wire = |pts: &[(f64, f64)], color: &str, sq: f64| {
        Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: pts.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            color: color.into(),
            sq,
            length_m: Some(0.4),
            part_no: None,
            net: None,
        })
    };
    // +48V/0V 母線ラダー: F1 -> SW1 -> K1 / K1a -> M1
    add(wire(&[(60.0, 60.0), (300.0, 60.0)], "red", 3.5));
    add(wire(&[(60.0, 220.0), (300.0, 220.0)], "black", 3.5));
    add(sym("fuse", "F1", "5A", 100.0, 90.0, 90));
    add(wire(&[(100.0, 60.0), (100.0, 82.5)], "red", 0.75));
    add(sym("switch_spst", "SW1", "", 100.0, 125.0, 90));
    add(wire(&[(100.0, 97.5), (100.0, 117.5)], "red", 0.75));
    add(sym("relay_coil", "K1", "JZX-22F", 100.0, 165.0, 90));
    add(wire(&[(100.0, 132.5), (100.0, 157.5)], "red", 0.75));
    add(wire(&[(100.0, 172.5), (100.0, 220.0)], "black", 0.75));
    add(sym("relay_contact_no", "K1a", "", 200.0, 100.0, 90));
    add(wire(&[(200.0, 60.0), (200.0, 92.5)], "red", 0.75));
    add(sym("motor", "M1", "BLM6400", 200.0, 160.0, 90));
    add(wire(&[(200.0, 107.5), (200.0, 152.5)], "blue", 0.75));
    add(wire(&[(200.0, 167.5), (200.0, 220.0)], "black", 0.75));
    add(Entity::Junction(Junction { id: Uuid::new_v4(), at: Point::new(100.0, 60.0) }));
    add(Entity::Junction(Junction { id: Uuid::new_v4(), at: Point::new(200.0, 60.0) }));
    add(Entity::NetLabel(NetLabel {
        id: Uuid::new_v4(),
        at: Point::new(62.0, 58.0),
        name: "48-P1".into(),
        rotation: 0,
    }));
    let svg = svg::sheet_to_svg(&sheet, &builtin_symbols());
    std::fs::write(&path, svg).unwrap();
    println!("wrote {path}");
}
