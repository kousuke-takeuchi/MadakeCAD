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
    ]
}

/// 動的シンボルの最大極数。
pub const DYNAMIC_PIN_MAX: usize = 50;

/// `connector_{n}p` / `terminal_block_{n}p` 形式のIDからピン数可変シンボルを生成する。
/// 端子は縦並び・ピッチ5mm・中央揃え(オフセットは常に2.5mmグリッド倍数)。
pub fn dynamic_symbol(id: &str) -> Option<SymbolDef> {
    let parse = |rest: &str| -> Option<usize> {
        let n: usize = rest.strip_suffix('p')?.parse().ok()?;
        (1..=DYNAMIC_PIN_MAX).contains(&n).then_some(n)
    };
    let offset = |i: usize, n: usize| (i as f64 - (n - 1) as f64 / 2.0) * 5.0;

    if let Some(n) = id.strip_prefix("connector_").and_then(parse) {
        let mut primitives = vec![Primitive::Rect {
            p1: p(-4.0, offset(0, n) - 2.5),
            p2: p(4.0, offset(n - 1, n) + 2.5),
            filled: false,
        }];
        let mut pins = Vec::with_capacity(n);
        for i in 0..n {
            let y = offset(i, n);
            primitives.push(Primitive::Text {
                at: p(-2.0, y),
                text: (i + 1).to_string(),
                height: 2.0,
            });
            primitives.push(line(&[(4.0, y), (7.5, y)]));
            pins.push(pin(&(i + 1).to_string(), "", 7.5, y));
        }
        return Some(SymbolDef {
            id: id.into(),
            name: format!("Connector {n}P"),
            name_ja: format!("コネクタ({n}極)"),
            category: "connector".into(),
            ref_prefix: "J".into(),
            primitives,
            pins,
        });
    }
    if let Some(n) = id.strip_prefix("terminal_block_").and_then(parse) {
        let mut primitives = vec![Primitive::Rect {
            p1: p(-2.5, offset(0, n) - 2.5),
            p2: p(2.5, offset(n - 1, n) + 2.5),
            filled: false,
        }];
        let mut pins = Vec::with_capacity(n * 2);
        for i in 0..n {
            let y = offset(i, n);
            let no = (i + 1).to_string();
            primitives.push(Primitive::Circle {
                center: p(0.0, y),
                r: 1.8,
                filled: false,
            });
            primitives.push(Primitive::Text {
                at: p(0.0, y - 1.0),
                text: no.clone(),
                height: 2.0,
            });
            primitives.push(line(&[(-2.5, y), (-1.8, y)]));
            primitives.push(line(&[(1.8, y), (2.5, y)]));
            // 貫通端子: 左右2接続点に同一ピン番号(ネットリストで内部短絡)
            pins.push(pin(&no, "", -2.5, y));
            pins.push(pin(&no, "", 2.5, y));
        }
        return Some(SymbolDef {
            id: id.into(),
            name: format!("Terminal block {n}P"),
            name_ja: format!("端子台({n}極)"),
            category: "connector".into(),
            ref_prefix: "TB".into(),
            primitives,
            pins,
        });
    }
    None
}

/// symbol_idから定義を解決する。静的ライブラリ優先、なければ動的生成。
pub fn resolve_symbol(id: &str) -> Option<SymbolDef> {
    builtin_symbols()
        .into_iter()
        .find(|s| s.id == id)
        .or_else(|| dynamic_symbol(id))
}

/// シートの描画・ネットリストに必要な全シンボル定義(静的+使用中の動的)を集める。
pub fn sheet_symbol_defs(sheet: &crate::model::Sheet) -> Vec<SymbolDef> {
    let mut defs = builtin_symbols();
    let mut seen: std::collections::BTreeSet<&str> =
        defs.iter().map(|d| d.id.as_str()).collect();
    let mut ids: Vec<&str> = Vec::new();
    for e in sheet.entities.values() {
        if let crate::model::Entity::Symbol(s) = e {
            if !seen.contains(s.symbol_id.as_str()) {
                seen.insert(s.symbol_id.as_str());
                ids.push(s.symbol_id.as_str());
            }
        }
    }
    defs.extend(ids.into_iter().filter_map(dynamic_symbol));
    defs
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
    fn dynamic_terminal_block_has_through_pins_on_grid() {
        let def = resolve_symbol("terminal_block_8p").expect("dynamic terminal block");
        assert_eq!(def.ref_prefix, "TB");
        // 8端子 x 左右2接続点 = 16ピン、各番号がちょうど2回
        assert_eq!(def.pins.len(), 16);
        for i in 1..=8 {
            let same: Vec<_> = def
                .pins
                .iter()
                .filter(|p| p.number == i.to_string())
                .collect();
            assert_eq!(same.len(), 2, "terminal {i}");
            // 左右対称
            assert!((same[0].at.x + same[1].at.x).abs() < 1e-9);
            assert!((same[0].at.y - same[1].at.y).abs() < 1e-9);
        }
        // 全ピン2.5mmグリッド上、中央揃え(y合計=0)
        let mut ysum: f64 = 0.0;
        for p in &def.pins {
            assert!((p.at.x / 2.5 - (p.at.x / 2.5).round()).abs() < 1e-9, "{:?}", p.at);
            assert!((p.at.y / 2.5 - (p.at.y / 2.5).round()).abs() < 1e-9, "{:?}", p.at);
            ysum += p.at.y;
        }
        assert!(ysum.abs() < 1e-9);
    }

    #[test]
    fn dynamic_connector_2p_matches_legacy_static_def() {
        // 旧静的connector_2pと同一のピン座標(既存図面の互換性)
        let def = resolve_symbol("connector_2p").expect("dynamic connector");
        assert_eq!(def.ref_prefix, "J");
        let pins: Vec<_> = def.pins.iter().map(|p| (p.number.as_str(), p.at.x, p.at.y)).collect();
        assert_eq!(pins, vec![("1", 7.5, -2.5), ("2", 7.5, 2.5)]);
    }

    #[test]
    fn resolve_symbol_rejects_invalid_ids_and_finds_builtins() {
        assert!(resolve_symbol("resistor").is_some());
        assert!(resolve_symbol("connector_0p").is_none());
        assert!(resolve_symbol("connector_51p").is_none());
        assert!(resolve_symbol("connector_p").is_none());
        assert!(resolve_symbol("terminal_block_xp").is_none());
        assert!(resolve_symbol("unknown").is_none());
    }

    #[test]
    fn sheet_symbol_defs_includes_dynamic_ids_in_use() {
        use crate::model::*;
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        let s = Entity::Symbol(SymbolInstance {
            id: uuid::Uuid::new_v4(),
            symbol_id: "terminal_block_3p".into(),
            at: Point::new(100.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "TB1".into(),
            value: String::new(),
            attrs: Default::default(),
        });
        sheet.entities.insert(s.id(), s);
        let defs = sheet_symbol_defs(&sheet);
        assert!(defs.iter().any(|d| d.id == "resistor"), "builtin含む");
        assert!(defs.iter().any(|d| d.id == "terminal_block_3p"), "使用中の動的ID含む");
    }

    #[test]
    fn symbol_json_roundtrip() {
        let syms = builtin_symbols();
        let json = serde_json::to_string(&syms).unwrap();
        let back: Vec<SymbolDef> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), syms.len());
    }
}
