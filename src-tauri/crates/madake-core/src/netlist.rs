//! ネットリスト抽出: 配置済みシンボルのピン座標解決と、配線・ジャンクション・
//! ネットラベルからの接続グラフ導出。

use crate::geometry::Point;
use crate::model::SymbolInstance;
use crate::symbol::SymbolDef;

/// ローカル座標をミラー→回転(0/90/180/270、時計回り、Y下向き座標系)→平行移動する。
pub fn transform_local(p: Point, inst: &SymbolInstance) -> Point {
    let (x, y) = if inst.mirror { (-p.x, p.y) } else { (p.x, p.y) };
    let (rx, ry) = match inst.rotation % 360 {
        90 => (-y, x),
        180 => (-x, -y),
        270 => (y, -x),
        _ => (x, y),
    };
    Point::new(inst.at.x + rx, inst.at.y + ry)
}

/// シンボルインスタンスの各ピンの (ピン番号, 用紙上絶対座標) を返す。
pub fn pin_positions(inst: &SymbolInstance, def: &SymbolDef) -> Vec<(String, Point)> {
    def.pins
        .iter()
        .map(|pin| (pin.number.clone(), transform_local(pin.at, inst)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::*;
    use crate::symbol::builtin_symbols;
    use uuid::Uuid;

    #[test]
    fn pin_positions_apply_rotation_and_translation() {
        let def = builtin_symbols()
            .into_iter()
            .find(|s| s.id == "resistor")
            .unwrap();
        // resistorのピン: ("1", (-7.5, 0)), ("2", (7.5, 0))
        let inst = SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "resistor".into(),
            at: Point::new(100.0, 50.0),
            rotation: 90,
            mirror: false,
            reference: "R1".into(),
            value: String::new(),
            attrs: Default::default(),
        };
        let pins = pin_positions(&inst, &def);
        // 90度回転(時計回り、Y下向き座標系): (x,y) -> (-y, x)
        assert!((pins[0].1.x - 100.0).abs() < 1e-9);
        assert!((pins[0].1.y - (50.0 - 7.5)).abs() < 1e-9);
        assert!((pins[1].1.x - 100.0).abs() < 1e-9);
        assert!((pins[1].1.y - (50.0 + 7.5)).abs() < 1e-9);
    }

    #[test]
    fn pin_positions_apply_mirror_before_rotation() {
        let def = builtin_symbols()
            .into_iter()
            .find(|s| s.id == "resistor")
            .unwrap();
        let inst = SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "resistor".into(),
            at: Point::new(10.0, 20.0),
            rotation: 0,
            mirror: true,
            reference: "R2".into(),
            value: String::new(),
            attrs: Default::default(),
        };
        let pins = pin_positions(&inst, &def);
        // ミラーでピン1(-7.5,0)は(+7.5,0)へ
        assert!((pins[0].1.x - 17.5).abs() < 1e-9);
        assert!((pins[0].1.y - 20.0).abs() < 1e-9);
    }
}
