//! デモ図面(2シート)の組み立て。SVG/PDFの手動確認用exampleが共有する。
//!
//! M2で入った図面要素を一通り含む:
//! - 改訂欄2行(シート1)+表題欄Revの連動
//! - ネット単位の線番(`renumber_wires`コマンドで自動採番)
//! - ハーネス境界(破線囲み+名前)
//! - シート間クロスリファレンス(同名ネットラベル「48-P1」「0V」が2シートに現れる)
//!
//! 図面の組み立ても全てCommandエンジン経由(UI・AI・CLIと同じ入口)。

#![allow(dead_code)]

use madake_core::model::{
    Entity, Harness, Junction, NetLabel, Orientation, PaperSize, Project, Revision, SheetId,
    SymbolInstance, TextEntity, TitleBlock, Wire,
};
use madake_core::wire_no::RenumberMode;
use madake_core::{Command, Engine, Point};
use uuid::Uuid;

fn symbol(symbol_id: &str, reference: &str, value: &str, x: f64, y: f64, rotation: u16) -> Entity {
    Entity::Symbol(SymbolInstance {
        id: Uuid::new_v4(),
        symbol_id: symbol_id.into(),
        at: Point::new(x, y),
        rotation,
        mirror: false,
        reference: reference.into(),
        value: value.into(),
        attrs: Default::default(),
    })
}

fn wire(points: &[(f64, f64)], color: &str, sq: f64, length_m: f64) -> Entity {
    Entity::Wire(Wire {
        id: Uuid::new_v4(),
        points: points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
        color: color.into(),
        sq,
        length_m: Some(length_m),
        length_source: Default::default(),
        part_no: None,
        net: None,
    })
}

/// シンボルに属性を1つ付けて返す (端子台のジャンパ設定など)。
fn with_attr(entity: Entity, key: &str, value: &str) -> Entity {
    match entity {
        Entity::Symbol(mut s) => {
            s.attrs.insert(key.into(), value.into());
            Entity::Symbol(s)
        }
        other => other,
    }
}

fn junction(x: f64, y: f64) -> Entity {
    Entity::Junction(Junction {
        id: Uuid::new_v4(),
        at: Point::new(x, y),
    })
}

fn label(name: &str, x: f64, y: f64) -> Entity {
    Entity::NetLabel(NetLabel {
        id: Uuid::new_v4(),
        at: Point::new(x, y),
        name: name.into(),
        rotation: 0,
    })
}

fn harness(name: &str, note: &str, x0: f64, y0: f64, x1: f64, y1: f64) -> Entity {
    Entity::Harness(Harness {
        id: Uuid::new_v4(),
        points: madake_core::harness::rect_points(Point::new(x0, y0), Point::new(x1, y1)),
        name: name.into(),
        note: note.into(),
    })
}

fn note(text: &str, x: f64, y: f64) -> Entity {
    Entity::Text(TextEntity {
        id: Uuid::new_v4(),
        at: Point::new(x, y),
        text: text.into(),
        height: 2.5,
        rotation: 0,
    })
}

fn title_block(title: &str, drawing_no: &str, sheet_no: &str) -> TitleBlock {
    TitleBlock {
        company: "MadakeCAD".into(),
        title: format!("{title}  ({sheet_no})"),
        drawing_no: drawing_no.into(),
        scale: "1:1".into(),
        date: "2026-08-22".into(),
        designed: "K.T".into(),
        drawn: "K.T".into(),
        checked: "S.M".into(),
        approved: "S.M".into(),
        rev: "A".into(),
    }
}

fn add_all(engine: &mut Engine, sheet_id: SheetId, entities: Vec<Entity>) {
    for entity in entities {
        engine
            .execute(Command::AddEntity { sheet_id, entity })
            .expect("エンティティ追加");
    }
}

/// シート1「電源・制御」: 母線ラダー+モータ回路+端子台。改訂2行入り。
fn build_sheet1(engine: &mut Engine, sheet_id: SheetId) {
    engine
        .execute(Command::RenameSheet {
            sheet_id,
            name: "1 電源・制御".into(),
        })
        .expect("シート名");
    engine
        .execute(Command::SetTitleBlock {
            sheet_id,
            title_block: title_block("48V 電源・制御系統図", "MDK-0001", "1/2"),
        })
        .expect("表題欄");
    engine
        .execute(Command::SetRevisions {
            sheet_id,
            revisions: vec![
                Revision {
                    mark: "A".into(),
                    date: "2026-08-01".into(),
                    description: "初版".into(),
                    by: "S.M".into(),
                },
                Revision {
                    mark: "B".into(),
                    date: "2026-08-22".into(),
                    description: "モータ回路とハーネスW1を追加".into(),
                    by: "S.M".into(),
                },
            ],
        })
        .expect("改訂欄");

    add_all(
        engine,
        sheet_id,
        vec![
            // 母線 (+48V / 0V)
            wire(&[(60.0, 55.0), (300.0, 55.0)], "red", 3.5, 2.4),
            wire(&[(60.0, 240.0), (300.0, 240.0)], "black", 3.5, 2.4),
            // 直流電源 BT1
            symbol("battery", "BT1", "48V", 60.0, 145.0, 90),
            wire(&[(60.0, 55.0), (60.0, 137.5)], "red", 3.5, 0.9),
            wire(&[(60.0, 152.5), (60.0, 240.0)], "black", 3.5, 0.9),
            // 制御分岐: F1 -> SW1 -> K1コイル
            symbol("fuse", "F1", "5A", 100.0, 85.0, 90),
            symbol("switch_spst", "SW1", "運転", 100.0, 125.0, 90),
            symbol("relay_coil", "K1", "JZX-22F", 100.0, 175.0, 90),
            wire(&[(100.0, 55.0), (100.0, 77.5)], "red", 0.75, 0.3),
            wire(&[(100.0, 92.5), (100.0, 117.5)], "red", 0.75, 0.3),
            wire(&[(100.0, 132.5), (100.0, 167.5)], "red", 0.75, 0.4),
            wire(&[(100.0, 182.5), (100.0, 240.0)], "black", 0.75, 0.6),
            junction(100.0, 55.0),
            junction(100.0, 240.0),
            // 動力分岐: K1a接点 -> M1 (ハーネスW1で束ねる)
            symbol("relay_contact_no", "K1a", "", 180.0, 90.0, 90),
            symbol("motor", "M1", "BLM6400", 180.0, 165.0, 90),
            wire(&[(180.0, 55.0), (180.0, 82.5)], "red", 3.5, 0.3),
            wire(&[(180.0, 97.5), (180.0, 157.5)], "blue", 3.5, 0.7),
            wire(&[(180.0, 172.5), (180.0, 240.0)], "black", 3.5, 0.7),
            junction(180.0, 55.0),
            junction(180.0, 240.0),
            harness("W1", "モータ配線 (3.5sq×2)", 165.0, 95.0, 197.5, 245.0),
            // 電源表示灯 L1
            symbol("lamp", "L1", "48V", 245.0, 110.0, 90),
            wire(&[(245.0, 55.0), (245.0, 102.5)], "red", 0.3, 0.5),
            wire(&[(245.0, 117.5), (245.0, 240.0)], "black", 0.3, 1.3),
            junction(245.0, 55.0),
            junction(245.0, 240.0),
            // 端子台TB1 (シート2への送り出し)。端子1-2はサドルジャンパで渡り
            with_attr(
                symbol("terminal_block_4p", "TB1", "", 310.0, 145.0, 0),
                "jumpers",
                "1-2",
            ),
            wire(&[(300.0, 55.0), (300.0, 137.5), (307.5, 137.5)], "red", 3.5, 1.0),
            wire(&[(300.0, 240.0), (300.0, 152.5), (307.5, 152.5)], "black", 3.5, 1.0),
            wire(&[(312.5, 137.5), (340.0, 137.5)], "red", 3.5, 0.3),
            wire(&[(312.5, 152.5), (340.0, 152.5)], "black", 3.5, 0.3),
            harness("W3", "TB1 外部配線 (3.5sq×2)", 312.0, 136.0, 345.0, 156.0),
            label("48-P1", 340.0, 137.5),
            label("0V", 340.0, 152.5),
            note("注記1: 電線は KIV。線番はネット単位で図面全体に一意。", 60.0, 258.0),
            note("注記2: ハーネスW1は制御盤内で束線する。", 60.0, 265.0),
        ],
    );
}

/// シート2「現場配線」: シート1と同名ラベルで受けた母線+操作回路。ハーネスW2入り。
fn build_sheet2(engine: &mut Engine, sheet_id: SheetId) {
    engine
        .execute(Command::SetTitleBlock {
            sheet_id,
            title_block: title_block("48V 現場配線図", "MDK-0002", "2/2"),
        })
        .expect("表題欄");
    engine
        .execute(Command::SetRevisions {
            sheet_id,
            revisions: vec![Revision {
                mark: "A".into(),
                date: "2026-08-22".into(),
                description: "初版".into(),
                by: "S.M".into(),
            }],
        })
        .expect("改訂欄");

    add_all(
        engine,
        sheet_id,
        vec![
            // シート1から受ける母線 (同名ラベルでクロスリファレンスが付く)
            wire(&[(60.0, 70.0), (300.0, 70.0)], "red", 3.5, 2.4),
            wire(&[(60.0, 225.0), (300.0, 225.0)], "black", 3.5, 2.4),
            label("48-P1", 60.0, 70.0),
            label("0V", 60.0, 225.0),
            // 操作回路: PB1 -> L2 (ハーネスW2で束ねる)
            symbol("pushbutton_no", "PB1", "起動", 110.0, 105.0, 90),
            symbol("lamp", "L2", "48V", 110.0, 160.0, 90),
            wire(&[(110.0, 70.0), (110.0, 97.5)], "red", 0.75, 0.3),
            wire(&[(110.0, 112.5), (110.0, 152.5)], "yellow", 0.75, 0.5),
            wire(&[(110.0, 167.5), (110.0, 225.0)], "black", 0.75, 0.6),
            junction(110.0, 70.0),
            junction(110.0, 225.0),
            harness("W2", "操作盤への現場配線", 95.0, 110.0, 127.5, 230.0),
            // 警報灯 SW2 -> L3
            symbol("switch_spst", "SW2", "点検", 180.0, 105.0, 90),
            symbol("lamp", "L3", "48V", 180.0, 160.0, 90),
            wire(&[(180.0, 70.0), (180.0, 97.5)], "red", 0.75, 0.3),
            wire(&[(180.0, 112.5), (180.0, 152.5)], "red", 0.75, 0.5),
            wire(&[(180.0, 167.5), (180.0, 225.0)], "black", 0.75, 0.6),
            junction(180.0, 70.0),
            junction(180.0, 225.0),
            // K2コイル
            symbol("relay_coil", "K2", "JZX-22F", 250.0, 120.0, 90),
            wire(&[(250.0, 70.0), (250.0, 112.5)], "red", 0.75, 0.5),
            wire(&[(250.0, 127.5), (250.0, 225.0)], "black", 0.75, 1.0),
            junction(250.0, 70.0),
            junction(250.0, 225.0),
            note("注記: ハーネスW2は現場で製作する。線番はシート1から連番。", 60.0, 258.0),
        ],
    );
}

/// デモ用の2シートプロジェクトを組み立てる(全編集はCommandエンジン経由)。
pub fn demo_project() -> Project {
    let mut engine = Engine::new(Project::new("MadakeCAD デモ図面"));
    let sheet1 = engine.project().sheets[0].id;
    build_sheet1(&mut engine, sheet1);
    engine
        .execute(Command::AddSheet {
            name: "2 現場配線".into(),
            size: PaperSize::A3,
            orientation: Orientation::Landscape,
        })
        .expect("シート2追加");
    let sheet2 = engine.project().sheets[1].id;
    build_sheet2(&mut engine, sheet2);
    // 線番: 図面全体で一意になるようネット単位で自動採番する
    engine
        .execute(Command::RenumberWires {
            sheet_id: None,
            mode: RenumberMode::Append,
            start: 1,
        })
        .expect("線番採番");
    engine.project().clone()
}

/// デモ図面のシートID(1枚目・2枚目)。
pub fn demo_sheets(project: &Project) -> (SheetId, SheetId) {
    (project.sheets[0].id, project.sheets[1].id)
}
