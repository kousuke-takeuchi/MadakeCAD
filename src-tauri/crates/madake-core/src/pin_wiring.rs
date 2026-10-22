//! ピン名で配線するための計算 (MCPの get_pins / connect_pins / place_mate)。
//!
//! エージェントがピン座標を手で計算しなくて済むように、配置済み部品のピンの用紙座標・向きと、
//! ピンどうしを結ぶ直角配線の頂点列をここで求める。編集そのものは呼び出し側がCommandで行う。

use serde::Serialize;

use crate::geometry::Point;
use crate::model::{Entity, EntityId, Sheet, SymbolInstance};
use crate::netlist::{rotate_local, transform_local};
use crate::symbol::{instance_symbol_def, sheet_symbol_defs, PinDir, SymbolDef};
use crate::tidy::{symbol_bounds, Rect};

/// 配置済みピン1本 (用紙座標・回転と反転の後の向き)。
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct PinInfo {
    pub number: String,
    pub name: String,
    pub x: f64,
    pub y: f64,
    /// 電線が出ていく向き (回転・反転の後)。
    pub dir: PinDir,
}

/// 配置済み部品1個のピン一覧と外形。
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct PartPins {
    pub entity_id: EntityId,
    pub reference: String,
    pub symbol_id: String,
    /// 用紙上の外接矩形 (ブロックは5mm丸めの後)。図形の無いシンボルはnull。
    pub bbox: Option<Rect>,
    pub pins: Vec<PinInfo>,
}

/// ピンの向き (回転0度の定義) を、配置の回転・反転を通した用紙上の向きにする。
pub fn world_dir(dir: PinDir, inst: &SymbolInstance) -> PinDir {
    let v = match dir {
        PinDir::Up => Point::new(0.0, -1.0),
        PinDir::Down => Point::new(0.0, 1.0),
        PinDir::Left => Point::new(-1.0, 0.0),
        PinDir::Right => Point::new(1.0, 0.0),
    };
    let r = rotate_local(v, inst.rotation, inst.mirror);
    if r.x.abs() > r.y.abs() {
        if r.x > 0.0 {
            PinDir::Right
        } else {
            PinDir::Left
        }
    } else if r.y > 0.0 {
        PinDir::Down
    } else {
        PinDir::Up
    }
}

/// 部品1個のピン (用紙座標・向き) と外形。
pub fn part_pins(inst: &SymbolInstance, def: &SymbolDef) -> PartPins {
    let pins = def
        .pins
        .iter()
        .map(|pin| {
            let at = transform_local(pin.at, inst);
            PinInfo {
                number: pin.number.clone(),
                name: pin.name.clone(),
                x: at.x,
                y: at.y,
                dir: world_dir(pin.dir, inst),
            }
        })
        .collect();
    PartPins {
        entity_id: inst.id,
        reference: inst.reference.clone(),
        symbol_id: inst.symbol_id.clone(),
        bbox: symbol_bounds(inst, def),
        pins,
    }
}

/// シート上の部品を参照記号 (完全一致) か entity id で探す。entity id順に返す。
pub fn find_parts(sheet: &Sheet, defs: &[SymbolDef], key: &str) -> Vec<PartPins> {
    let by_id = key.parse::<EntityId>().ok();
    sheet
        .entities
        .values()
        .filter_map(|e| match e {
            Entity::Symbol(s) if Some(s.id) == by_id || s.reference == key => Some(s),
            _ => None,
        })
        .filter_map(|s| instance_symbol_def(s, defs).map(|def| part_pins(s, &def)))
        .collect()
}

/// entity id の部品1個のピン (定義はシートから集める)。シンボルでなければNone。
pub fn sheet_part_pins(sheet: &Sheet, entity_id: EntityId) -> Option<PartPins> {
    let defs = sheet_symbol_defs(sheet);
    match sheet.entities.get(&entity_id)? {
        Entity::Symbol(s) => instance_symbol_def(s, &defs).map(|def| part_pins(s, &def)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Entity, Orientation, PaperSize, Sheet, SymbolInstance};
    use crate::symbol::{resolve_symbol, sheet_symbol_defs};
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn inst(symbol_id: &str, x: f64, y: f64, rotation: u16, mirror: bool, reference: &str) -> SymbolInstance {
        SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: symbol_id.into(),
            at: Point::new(x, y),
            rotation,
            mirror,
            reference: reference.into(),
            value: String::new(),
            attrs: BTreeMap::new(),
        }
    }

    fn block(reference: &str, x: f64, y: f64) -> SymbolInstance {
        let mut b = inst("block", x, y, 0, false, reference);
        b.attrs.insert(
            "block_pins".into(),
            r#"{"left":[{"no":"IN1","name":"入力1"}],"right":[{"no":"OUT1","name":"出力1"}]}"#.into(),
        );
        b
    }

    fn sheet_with(parts: Vec<SymbolInstance>) -> Sheet {
        let mut sheet = Sheet::new("試験", PaperSize::A3, Orientation::Landscape);
        for p in parts {
            sheet.entities.insert(p.id, Entity::Symbol(p));
        }
        sheet
    }

    /// A plug's pin is reported at its paper position, and its direction follows rotation and mirror.
    /// プラグのピンは用紙上の位置で返り、向きは回転と反転に合わせて変わる。
    #[test]
    fn pin_direction_follows_rotation_and_mirror() {
        let def = resolve_symbol("connector_plug").unwrap();
        let plain = part_pins(&inst("connector_plug", 50.0, 40.0, 0, false, "J1"), &def);
        assert_eq!((plain.pins[0].x, plain.pins[0].y), (42.5, 40.0));
        assert_eq!(plain.pins[0].dir, PinDir::Left);
        let mirrored = part_pins(&inst("connector_plug", 50.0, 40.0, 0, true, "J1"), &def);
        assert_eq!(mirrored.pins[0].dir, PinDir::Right);
        let turned = part_pins(&inst("connector_plug", 50.0, 40.0, 90, false, "J1"), &def);
        assert_eq!((turned.pins[0].x, turned.pins[0].y), (50.0, 32.5));
        assert_eq!(turned.pins[0].dir, PinDir::Up);
        let both = part_pins(&inst("connector_plug", 50.0, 40.0, 90, true, "J1"), &def);
        assert_eq!((both.pins[0].x, both.pins[0].y), (50.0, 47.5));
        assert_eq!(both.pins[0].dir, PinDir::Down);
    }

    /// A block's named pins are reported with their numbers, names, outside positions and the rounded outline.
    /// ブロックの名前付きピンは番号・名前・外側の位置で返り、外形は丸めた後の大きさで返る。
    #[test]
    fn block_pins_and_outline_are_reported() {
        let b = block("CTRL1", 100.0, 50.0);
        let sheet = sheet_with(vec![b.clone()]);
        let parts = find_parts(&sheet, &sheet_symbol_defs(&sheet), "CTRL1");
        assert_eq!(parts.len(), 1);
        let p = &parts[0];
        assert_eq!(p.entity_id, b.id);
        let in1 = p.pins.iter().find(|x| x.number == "IN1").unwrap();
        assert_eq!(in1.name, "入力1");
        assert_eq!(in1.dir, PinDir::Left);
        let bbox = p.bbox.expect("外形");
        assert!(bbox.min.x <= in1.x && in1.x < 100.0);
        let out1 = p.pins.iter().find(|x| x.number == "OUT1").unwrap();
        assert_eq!(out1.dir, PinDir::Right);
        assert!(out1.x <= bbox.max.x && out1.x > 100.0);
    }

    /// Looking up a reference returns every part that carries it, and an entity id finds exactly that part.
    /// 参照記号で引くとその参照記号の部品を全部返し、entity id で引くとその部品だけを返す。
    #[test]
    fn parts_are_found_by_reference_or_entity_id() {
        let a = inst("connector_socket", 10.0, 10.0, 0, false, "J2");
        let b = inst("connector_socket", 10.0, 30.0, 0, false, "J2");
        let sheet = sheet_with(vec![a.clone(), b.clone()]);
        let defs = sheet_symbol_defs(&sheet);
        assert_eq!(find_parts(&sheet, &defs, "J2").len(), 2);
        let by_id = find_parts(&sheet, &defs, &a.id.to_string());
        assert_eq!(by_id.len(), 1);
        assert_eq!(by_id[0].entity_id, a.id);
        assert!(find_parts(&sheet, &defs, "J9").is_empty());
    }
}
