use serde::{Deserialize, Serialize};

use crate::geometry::Point;

/// シンボル定義。ローカル座標はmm、原点=配置基準点、ピンは2.5mmグリッド上に置く。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SymbolDef {
    /// ライブラリキー(例: "relay_coil")。
    pub id: String,
    pub name: String,
    pub name_ja: String,
    pub category: String,
    /// 参照記号の接頭辞(例: "R", "K", "J")。
    pub ref_prefix: String,
    pub primitives: Vec<Primitive>,
    pub pins: Vec<PinDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Primitive {
    /// 折れ線。
    Line { pts: Vec<Point> },
    Circle { center: Point, r: f64, filled: bool },
    Arc {
        center: Point,
        r: f64,
        start_deg: f64,
        end_deg: f64,
    },
    Rect { p1: Point, p2: Point, filled: bool },
    Text { at: Point, text: String, height: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PinDef {
    pub number: String,
    #[serde(default)]
    pub name: String,
    /// 接続点(ローカル座標)。
    pub at: Point,
}

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn line(pts: &[(f64, f64)]) -> Primitive {
    Primitive::Line {
        pts: pts.iter().map(|&(x, y)| p(x, y)).collect(),
    }
}

fn pin(number: &str, name: &str, x: f64, y: f64) -> PinDef {
    PinDef {
        number: number.into(),
        name: name.into(),
        at: p(x, y),
    }
}

/// 初期同梱のJIS C 0617系基本シンボルセット。
pub fn builtin_symbols() -> Vec<SymbolDef> {
    vec![
        SymbolDef {
            id: "resistor".into(),
            name: "Resistor".into(),
            name_ja: "抵抗器".into(),
            category: "passive".into(),
            ref_prefix: "R".into(),
            primitives: vec![
                Primitive::Rect {
                    p1: p(-5.0, -2.0),
                    p2: p(5.0, 2.0),
                    filled: false,
                },
                line(&[(-7.5, 0.0), (-5.0, 0.0)]),
                line(&[(5.0, 0.0), (7.5, 0.0)]),
            ],
            pins: vec![pin("1", "", -7.5, 0.0), pin("2", "", 7.5, 0.0)],
        },
        SymbolDef {
            id: "fuse".into(),
            name: "Fuse".into(),
            name_ja: "ヒューズ".into(),
            category: "protection".into(),
            ref_prefix: "F".into(),
            primitives: vec![
                Primitive::Rect {
                    p1: p(-5.0, -2.0),
                    p2: p(5.0, 2.0),
                    filled: false,
                },
                line(&[(-7.5, 0.0), (7.5, 0.0)]),
            ],
            pins: vec![pin("1", "", -7.5, 0.0), pin("2", "", 7.5, 0.0)],
        },
        SymbolDef {
            id: "capacitor".into(),
            name: "Capacitor".into(),
            name_ja: "コンデンサ".into(),
            category: "passive".into(),
            ref_prefix: "C".into(),
            primitives: vec![
                line(&[(-7.5, 0.0), (-1.0, 0.0)]),
                line(&[(1.0, 0.0), (7.5, 0.0)]),
                line(&[(-1.0, -4.0), (-1.0, 4.0)]),
                line(&[(1.0, -4.0), (1.0, 4.0)]),
            ],
            pins: vec![pin("1", "", -7.5, 0.0), pin("2", "", 7.5, 0.0)],
        },
        SymbolDef {
            id: "diode".into(),
            name: "Diode".into(),
            name_ja: "ダイオード".into(),
            category: "semiconductor".into(),
            ref_prefix: "D".into(),
            primitives: vec![
                line(&[(-7.5, 0.0), (-2.5, 0.0)]),
                line(&[(2.5, 0.0), (7.5, 0.0)]),
                line(&[(-2.5, -3.0), (-2.5, 3.0), (2.5, 0.0), (-2.5, -3.0)]),
                line(&[(2.5, -3.0), (2.5, 3.0)]),
            ],
            pins: vec![pin("1", "A", -7.5, 0.0), pin("2", "K", 7.5, 0.0)],
        },
        SymbolDef {
            id: "led".into(),
            name: "LED".into(),
            name_ja: "発光ダイオード".into(),
            category: "semiconductor".into(),
            ref_prefix: "LED".into(),
            primitives: vec![
                line(&[(-7.5, 0.0), (-2.5, 0.0)]),
                line(&[(2.5, 0.0), (7.5, 0.0)]),
                line(&[(-2.5, -3.0), (-2.5, 3.0), (2.5, 0.0), (-2.5, -3.0)]),
                line(&[(2.5, -3.0), (2.5, 3.0)]),
                line(&[(0.0, -3.5), (2.0, -5.5)]),
                line(&[(1.0, -5.5), (2.0, -5.5), (2.0, -4.5)]),
                line(&[(2.5, -3.5), (4.5, -5.5)]),
                line(&[(3.5, -5.5), (4.5, -5.5), (4.5, -4.5)]),
            ],
            pins: vec![pin("1", "A", -7.5, 0.0), pin("2", "K", 7.5, 0.0)],
        },
        SymbolDef {
            id: "switch_spst".into(),
            name: "Switch (SPST)".into(),
            name_ja: "スイッチ".into(),
            category: "switch".into(),
            ref_prefix: "SW".into(),
            primitives: vec![
                line(&[(-7.5, 0.0), (-2.5, 0.0)]),
                line(&[(2.5, 0.0), (7.5, 0.0)]),
                line(&[(-2.5, 0.0), (2.5, -3.5)]),
                Primitive::Circle {
                    center: p(-2.5, 0.0),
                    r: 0.5,
                    filled: false,
                },
                Primitive::Circle {
                    center: p(2.5, 0.0),
                    r: 0.5,
                    filled: false,
                },
            ],
            pins: vec![pin("1", "", -7.5, 0.0), pin("2", "", 7.5, 0.0)],
        },
        SymbolDef {
            id: "pushbutton_no".into(),
            name: "Pushbutton (NO)".into(),
            name_ja: "押しボタン(a接点)".into(),
            category: "switch".into(),
            ref_prefix: "PB".into(),
            primitives: vec![
                line(&[(-7.5, 0.0), (-2.5, 0.0)]),
                line(&[(2.5, 0.0), (7.5, 0.0)]),
                line(&[(-2.5, -2.5), (2.5, -2.5)]),
                line(&[(0.0, -2.5), (0.0, -5.0)]),
                line(&[(-2.0, -5.0), (2.0, -5.0)]),
                Primitive::Circle {
                    center: p(-2.5, 0.0),
                    r: 0.5,
                    filled: false,
                },
                Primitive::Circle {
                    center: p(2.5, 0.0),
                    r: 0.5,
                    filled: false,
                },
            ],
            pins: vec![pin("1", "", -7.5, 0.0), pin("2", "", 7.5, 0.0)],
        },
        SymbolDef {
            id: "relay_coil".into(),
            name: "Relay coil".into(),
            name_ja: "リレーコイル".into(),
            category: "relay".into(),
            ref_prefix: "K".into(),
            primitives: vec![
                Primitive::Rect {
                    p1: p(-5.0, -3.0),
                    p2: p(5.0, 3.0),
                    filled: false,
                },
                line(&[(-7.5, 0.0), (-5.0, 0.0)]),
                line(&[(5.0, 0.0), (7.5, 0.0)]),
            ],
            pins: vec![pin("A1", "", -7.5, 0.0), pin("A2", "", 7.5, 0.0)],
        },
        SymbolDef {
            id: "relay_contact_no".into(),
            name: "Relay contact (NO)".into(),
            name_ja: "リレー接点(a接点)".into(),
            category: "relay".into(),
            ref_prefix: "K".into(),
            primitives: vec![
                line(&[(-7.5, 0.0), (-2.5, 0.0)]),
                line(&[(2.5, 0.0), (7.5, 0.0)]),
                line(&[(-2.5, 0.0), (2.5, -3.5)]),
            ],
            pins: vec![pin("1", "", -7.5, 0.0), pin("2", "", 7.5, 0.0)],
        },
        SymbolDef {
            id: "lamp".into(),
            name: "Lamp".into(),
            name_ja: "ランプ".into(),
            category: "output".into(),
            ref_prefix: "L".into(),
            primitives: vec![
                Primitive::Circle {
                    center: p(0.0, 0.0),
                    r: 4.0,
                    filled: false,
                },
                line(&[(-2.83, -2.83), (2.83, 2.83)]),
                line(&[(-2.83, 2.83), (2.83, -2.83)]),
                line(&[(-7.5, 0.0), (-4.0, 0.0)]),
                line(&[(4.0, 0.0), (7.5, 0.0)]),
            ],
            pins: vec![pin("1", "", -7.5, 0.0), pin("2", "", 7.5, 0.0)],
        },
        SymbolDef {
            id: "motor".into(),
            name: "Motor".into(),
            name_ja: "モータ".into(),
            category: "output".into(),
            ref_prefix: "M".into(),
            primitives: vec![
                Primitive::Circle {
                    center: p(0.0, 0.0),
                    r: 5.0,
                    filled: false,
                },
                Primitive::Text {
                    at: p(0.0, 0.0),
                    text: "M".into(),
                    height: 4.0,
                },
                line(&[(-7.5, 0.0), (-5.0, 0.0)]),
                line(&[(5.0, 0.0), (7.5, 0.0)]),
            ],
            pins: vec![pin("1", "", -7.5, 0.0), pin("2", "", 7.5, 0.0)],
        },
        SymbolDef {
            id: "battery".into(),
            name: "DC source / Battery".into(),
            name_ja: "直流電源".into(),
            category: "power".into(),
            ref_prefix: "BT".into(),
            primitives: vec![
                line(&[(-7.5, 0.0), (-1.0, 0.0)]),
                line(&[(1.0, 0.0), (7.5, 0.0)]),
                line(&[(-1.0, -4.0), (-1.0, 4.0)]),
                line(&[(1.0, -2.0), (1.0, 2.0)]),
                Primitive::Text {
                    at: p(-3.0, -5.5),
                    text: "+".into(),
                    height: 3.0,
                },
            ],
            pins: vec![pin("1", "+", -7.5, 0.0), pin("2", "-", 7.5, 0.0)],
        },
        SymbolDef {
            id: "ground".into(),
            name: "Ground".into(),
            name_ja: "接地".into(),
            category: "power".into(),
            ref_prefix: "GND".into(),
            primitives: vec![
                line(&[(0.0, 0.0), (0.0, 2.5)]),
                line(&[(-4.0, 2.5), (4.0, 2.5)]),
                line(&[(-2.5, 4.0), (2.5, 4.0)]),
                line(&[(-1.0, 5.5), (1.0, 5.5)]),
            ],
            pins: vec![pin("1", "", 0.0, 0.0)],
        },
        SymbolDef {
            id: "terminal".into(),
            name: "Terminal".into(),
            name_ja: "端子".into(),
            category: "connector".into(),
            ref_prefix: "T".into(),
            primitives: vec![
                Primitive::Circle {
                    center: p(0.0, 0.0),
                    r: 1.2,
                    filled: false,
                },
                line(&[(1.2, 0.0), (5.0, 0.0)]),
            ],
            pins: vec![pin("1", "", 5.0, 0.0)],
        },
        SymbolDef {
            id: "connector_2p".into(),
            name: "Connector 2P".into(),
            name_ja: "コネクタ(2極)".into(),
            category: "connector".into(),
            ref_prefix: "J".into(),
            primitives: vec![
                Primitive::Rect {
                    p1: p(-4.0, -5.0),
                    p2: p(4.0, 5.0),
                    filled: false,
                },
                Primitive::Text {
                    at: p(-2.0, -2.5),
                    text: "1".into(),
                    height: 2.0,
                },
                Primitive::Text {
                    at: p(-2.0, 2.5),
                    text: "2".into(),
                    height: 2.0,
                },
                line(&[(4.0, -2.5), (7.5, -2.5)]),
                line(&[(4.0, 2.5), (7.5, 2.5)]),
            ],
            pins: vec![pin("1", "P", 7.5, -2.5), pin("2", "N", 7.5, 2.5)],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_symbols_have_unique_ids_and_pins() {
        let syms = builtin_symbols();
        assert!(syms.len() >= 10);
        let mut ids: Vec<_> = syms.iter().map(|s| s.id.clone()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), syms.len(), "duplicate symbol ids");
        for s in &syms {
            assert!(!s.pins.is_empty(), "symbol {} has no pins", s.id);
        }
    }

    #[test]
    fn symbol_json_roundtrip() {
        let syms = builtin_symbols();
        let json = serde_json::to_string(&syms).unwrap();
        let back: Vec<SymbolDef> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), syms.len());
    }
}
