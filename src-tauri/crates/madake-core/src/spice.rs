//! SPICEネットリスト生成 (spec §3.5)。電気検証のngspiceバックエンド用。
//!
//! ノードは「接続点クラスタ」(座標一致するピン・ジャンクション・ワイヤ端点)。
//! ネット単位ではない: ワイヤは抵抗素子になり、ワイヤ抵抗でノードが分かれる。

use crate::geometry::Point;
use crate::model::Sheet;
use crate::symbol::SymbolDef;
use crate::EntityId;

/// 銅の抵抗率 (Ω·mm²/m)。verify.rsと共通。
pub const COPPER_RESISTIVITY: f64 = 0.0175;
/// 長さ未設定ワイヤ・導通部品・ラベル橋の微小抵抗 (Ω)。
pub const BRIDGE_OHMS: f64 = 1e-3;
/// current_a未設定の負荷の等価抵抗 (Ω)。存在だけ表現する。
pub const UNKNOWN_LOAD_OHMS: f64 = 1e6;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SpiceError {
    #[error("電源(battery)がありません")]
    NoSource,
}

/// デッキ中の抵抗素子1つ。電流はノード電圧差/ohmsで後計算する。
#[derive(Debug, Clone)]
pub struct SpiceElement {
    /// 由来エンティティ(ワイヤ/シンボル/ネットラベル)。
    pub entity_id: EntityId,
    /// SPICE素子名 (例: "R3")。
    pub name: String,
    pub node_a: String,
    pub node_b: String,
    pub ohms: f64,
}

#[derive(Debug)]
pub struct SpiceDeck {
    /// SPICEネットリスト本文 (.end含まず。解析コマンドはランナーが付ける)。
    pub deck: String,
    /// 電源電圧 (V)。
    pub voltage: f64,
    /// ワイヤ由来の抵抗素子 (分割されたワイヤは複数エントリ)。
    pub wire_elements: Vec<SpiceElement>,
    /// 部品・ラベル橋由来の抵抗素子 (ヒューズ電流の算出などに使う)。
    pub component_elements: Vec<SpiceElement>,
}

/// 座標→ノードクラスタキー (0.01mm量子化。実座標は2.5mmグリッド上なので十分)。
fn node_key(p: Point) -> (i64, i64) {
    (((p.x * 100.0).round()) as i64, ((p.y * 100.0).round()) as i64)
}

/// デッキ生成オプション。
#[derive(Debug, Default, Clone)]
pub struct DeckOptions {
    /// 開路として扱う導通部品の参照記号 (スイッチ/接点のwhat-if)。
    pub open_switches: Vec<String>,
}

/// シートからSPICEデッキを組み立てる。
pub fn build_deck(sheet: &Sheet, symbols: &[SymbolDef]) -> Result<SpiceDeck, SpiceError> {
    build_deck_with(sheet, symbols, &DeckOptions::default())
}

/// オプション付きでSPICEデッキを組み立てる。
pub fn build_deck_with(
    sheet: &Sheet,
    symbols: &[SymbolDef],
    opts: &DeckOptions,
) -> Result<SpiceDeck, SpiceError> {
    use crate::model::Entity;
    use crate::netlist::{pin_positions, segment_distance, CONNECT_EPS};
    use crate::verify::{is_conductor, is_load, parse_number};
    use std::collections::{BTreeMap, BTreeSet};

    let defs: BTreeMap<&str, &SymbolDef> = symbols.iter().map(|d| (d.id.as_str(), d)).collect();

    // 電源: 最初のbattery (BTreeMap順で決定的)
    let mut batteries = Vec::new();
    let mut attachments: Vec<crate::geometry::Point> = Vec::new(); // ワイヤ分割点になる点
    let mut labels: BTreeMap<&str, Vec<(EntityId, crate::geometry::Point)>> = BTreeMap::new();
    for e in sheet.entities.values() {
        match e {
            Entity::Symbol(s) => {
                if let Some(def) = defs.get(s.symbol_id.as_str()) {
                    let pins = pin_positions(s, def);
                    attachments.extend(pins.iter().map(|(_, p)| *p));
                    if def.id == "battery" {
                        batteries.push((s, *def, pins));
                    }
                }
            }
            Entity::Junction(j) => attachments.push(j.at),
            Entity::NetLabel(l) => {
                attachments.push(l.at);
                labels.entry(l.name.as_str()).or_default().push((l.id, l.at));
            }
            Entity::Wire(_) | Entity::Text(_) => {}
        }
    }
    if batteries.is_empty() {
        return Err(SpiceError::NoSource);
    }
    let voltage = parse_number(&batteries[0].0.value).unwrap_or(24.0);
    // GND = 最初のbatteryの末尾ピン(負極)
    let gnd_key = node_key(
        batteries[0]
            .2
            .last()
            .map(|(_, p)| *p)
            .expect("battery has pins"),
    );

    let mut used_keys: BTreeSet<(i64, i64)> = BTreeSet::new();
    let mut seq = 0usize;
    let mut wire_elements = Vec::new();
    let mut component_elements = Vec::new();
    let next_name = |seq: &mut usize| {
        *seq += 1;
        format!("R{seq}")
    };

    // ワイヤ → 分割点で区切った抵抗列
    for e in sheet.entities.values() {
        let Entity::Wire(w) = e else { continue };
        if w.points.len() < 2 {
            continue;
        }
        // ポリラインの弧長パラメータ
        let mut cum = vec![0.0f64];
        for seg in w.points.windows(2) {
            cum.push(cum.last().unwrap() + seg[0].distance_to(&seg[1]));
        }
        let total = *cum.last().unwrap();
        if total < 1e-9 {
            continue;
        }
        // 分割点: 端点 + ワイヤ上に乗る接続点(射影で弧長tを求める)
        let mut cuts: Vec<(f64, crate::geometry::Point)> = vec![
            (0.0, w.points[0]),
            (total, *w.points.last().unwrap()),
        ];
        for ap in &attachments {
            for (i, seg) in w.points.windows(2).enumerate() {
                if segment_distance(ap, &seg[0], &seg[1]) < CONNECT_EPS {
                    let (dx, dy) = (seg[1].x - seg[0].x, seg[1].y - seg[0].y);
                    let len2 = dx * dx + dy * dy;
                    let t = if len2 < 1e-12 {
                        0.0
                    } else {
                        (((ap.x - seg[0].x) * dx + (ap.y - seg[0].y) * dy) / len2).clamp(0.0, 1.0)
                    };
                    cuts.push((cum[i] + t * (cum[i + 1] - cum[i]), *ap));
                    break;
                }
            }
        }
        cuts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        cuts.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-6);
        let r_total = match (w.length_m, w.sq) {
            (Some(l), sq) if l > 0.0 && sq > 0.0 => COPPER_RESISTIVITY * l / sq,
            _ => BRIDGE_OHMS,
        };
        for pair in cuts.windows(2) {
            let (t0, p0) = pair[0];
            let (t1, p1) = pair[1];
            let (ka, kb) = (node_key(p0), node_key(p1));
            if ka == kb {
                continue;
            }
            used_keys.insert(ka);
            used_keys.insert(kb);
            wire_elements.push(SpiceElement {
                entity_id: w.id,
                name: next_name(&mut seq),
                node_a: format!("{ka:?}"),
                node_b: format!("{kb:?}"),
                ohms: r_total * (t1 - t0) / total,
            });
        }
    }

    // シンボル → 導通橋(1mΩ)・端子台の同番号貫通・負荷の等価抵抗
    for e in sheet.entities.values() {
        let Entity::Symbol(s) = e else { continue };
        let Some(def) = defs.get(s.symbol_id.as_str()) else { continue };
        let pins = pin_positions(s, def);
        let mut push = |a: crate::geometry::Point, b: crate::geometry::Point, ohms: f64| {
            let (ka, kb) = (node_key(a), node_key(b));
            if ka == kb {
                return;
            }
            used_keys.insert(ka);
            used_keys.insert(kb);
            component_elements.push(SpiceElement {
                entity_id: s.id,
                name: next_name(&mut seq),
                node_a: format!("{ka:?}"),
                node_b: format!("{kb:?}"),
                ohms,
            });
        };
        // 同一ピン番号の複数接続点(端子台の左右)は内部短絡
        let mut by_no: BTreeMap<&str, Vec<crate::geometry::Point>> = BTreeMap::new();
        for (no, p) in &pins {
            by_no.entry(no.as_str()).or_default().push(*p);
        }
        for pts in by_no.values() {
            for pair in pts.windows(2) {
                push(pair[0], pair[1], BRIDGE_OHMS);
            }
        }
        // 番号代表点 (番号ごとの先頭)
        let reps: Vec<crate::geometry::Point> =
            by_no.values().map(|pts| pts[0]).collect();
        if is_conductor(def) {
            // what-if: 指定された参照記号の導通部品は開路扱い(橋を張らない)
            if !opts.open_switches.iter().any(|r| r == &s.reference) {
                for p in reps.iter().skip(1) {
                    push(reps[0], *p, BRIDGE_OHMS);
                }
            }
        } else if is_load(def) && reps.len() >= 2 {
            let ohms = match s.attrs.get("current_a").and_then(|v| parse_number(v)) {
                Some(a) if a > 0.0 => voltage / a,
                _ => UNKNOWN_LOAD_OHMS,
            };
            push(reps[0], reps[1], ohms);
        }
    }

    // 同名ネットラベルの橋
    for (_, group) in labels {
        for pair in group.windows(2) {
            let (ka, kb) = (node_key(pair[0].1), node_key(pair[1].1));
            if ka == kb {
                continue;
            }
            used_keys.insert(ka);
            used_keys.insert(kb);
            component_elements.push(SpiceElement {
                entity_id: pair[1].0,
                name: next_name(&mut seq),
                node_a: format!("{ka:?}"),
                node_b: format!("{kb:?}"),
                ohms: BRIDGE_OHMS,
            });
        }
    }

    // ノード命名: GND=0、その他はキー昇順にn1..
    let mut names: BTreeMap<(i64, i64), String> = BTreeMap::new();
    names.insert(gnd_key, "0".into());
    let mut n = 0usize;
    for key in &used_keys {
        names.entry(*key).or_insert_with(|| {
            n += 1;
            format!("n{n}")
        });
    }
    let resolve = |raw: &str, names: &BTreeMap<(i64, i64), String>| -> String {
        // "{(x, y)}" 形式のデバッグキーを最終名へ置換
        let key: (i64, i64) = {
            let t = raw.trim_start_matches('(').trim_end_matches(')');
            let mut it = t.split(',').map(|s| s.trim().parse::<i64>().unwrap());
            (it.next().unwrap(), it.next().unwrap())
        };
        names.get(&key).cloned().unwrap_or_else(|| "0".into())
    };
    for el in wire_elements.iter_mut().chain(component_elements.iter_mut()) {
        el.node_a = resolve(&el.node_a, &names);
        el.node_b = resolve(&el.node_b, &names);
    }

    // デッキ本文
    let mut deck = String::from("* MadakeCAD verification\n");
    for (i, (s, _, pins)) in batteries.iter().enumerate() {
        let v = parse_number(&s.value).unwrap_or(24.0);
        let plus = names
            .get(&node_key(pins[0].1))
            .cloned()
            .unwrap_or_else(|| "0".into());
        let minus = names
            .get(&node_key(pins.last().unwrap().1))
            .cloned()
            .unwrap_or_else(|| "0".into());
        deck.push_str(&format!("V{} {} {} DC {}\n", i + 1, plus, minus, v));
    }
    for el in wire_elements.iter().chain(component_elements.iter()) {
        deck.push_str(&format!(
            "{} {} {} {}\n",
            el.name, el.node_a, el.node_b, el.ohms
        ));
    }

    Ok(SpiceDeck {
        deck,
        voltage,
        wire_elements,
        component_elements,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::*;
    use crate::symbol::sheet_symbol_defs;
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn symbol(sym: &str, reference: &str, x: f64, y: f64, value: &str) -> Entity {
        Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: sym.into(),
            at: Point::new(x, y),
            rotation: 0,
            mirror: false,
            reference: reference.into(),
            value: value.into(),
            attrs: Default::default(),
        })
    }

    fn wire(points: &[(f64, f64)], sq: f64, length_m: Option<f64>) -> Entity {
        Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            color: "red".into(),
            sq,
            length_m,
            part_no: None,
            net: None,
        })
    }

    fn sheet_with(entities: Vec<Entity>) -> Sheet {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        for e in entities {
            sheet.entities.insert(e.id(), e);
        }
        sheet
    }

    #[test]
    fn series_circuit_builds_deck_with_source_ground_and_resistors() {
        let mut lamp = SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "lamp".into(),
            at: Point::new(190.0, 100.0),
            rotation: 0,
            mirror: false,
            reference: "L1".into(),
            value: String::new(),
            attrs: Default::default(),
        };
        lamp.attrs.insert("current_a".into(), "2".into());
        let sheet = sheet_with(vec![
            symbol("battery", "BT1", 60.0, 100.0, "DC24V"),
            symbol("fuse", "F1", 100.0, 100.0, "5A"),
            symbol("switch_spst", "SW1", 140.0, 100.0, ""),
            Entity::Symbol(lamp),
            wire(&[(67.5, 100.0), (92.5, 100.0)], 0.75, Some(2.0)),
            wire(&[(107.5, 100.0), (132.5, 100.0)], 0.75, Some(2.0)),
            wire(&[(147.5, 100.0), (182.5, 100.0)], 0.75, Some(2.0)),
            wire(
                &[
                    (197.5, 100.0),
                    (220.0, 100.0),
                    (220.0, 140.0),
                    (40.0, 140.0),
                    (40.0, 100.0),
                    (52.5, 100.0),
                ],
                0.75,
                Some(2.0),
            ),
        ]);
        let deck = build_deck(&sheet, &sheet_symbol_defs(&sheet)).expect("deck");
        assert!((deck.voltage - 24.0).abs() < 1e-9);
        // 電圧源とGND(0)がある
        assert!(deck.deck.contains("DC 24"), "{}", deck.deck);
        assert!(deck.deck.lines().any(|l| l.starts_with('V') && l.contains(" 0 ")), "{}", deck.deck);
        // ワイヤ4本 → 抵抗4本、R = 0.0175*2/0.75
        assert_eq!(deck.wire_elements.len(), 4);
        for el in &deck.wire_elements {
            assert!((el.ohms - 0.0175 * 2.0 / 0.75).abs() < 1e-9, "{el:?}");
        }
        // 部品: ヒューズ+スイッチ(1mΩ) と 負荷(24/2=12Ω)
        let ohms: Vec<f64> = deck.component_elements.iter().map(|e| e.ohms).collect();
        assert_eq!(ohms.iter().filter(|&&o| (o - BRIDGE_OHMS).abs() < 1e-12).count(), 2, "{ohms:?}");
        assert_eq!(ohms.iter().filter(|&&o| (o - 12.0).abs() < 1e-9).count(), 1, "{ohms:?}");
        // デッキの素子行数 = 電圧源1 + 抵抗7
        let elems = deck.deck.lines().filter(|l| l.starts_with('R') || l.starts_with('V')).count();
        assert_eq!(elems, 8, "{}", deck.deck);
    }

    #[test]
    fn junction_splits_wire_resistance_proportionally() {
        let j = Entity::Junction(Junction {
            id: Uuid::new_v4(),
            at: Point::new(40.0, 0.0),
        });
        let sheet = sheet_with(vec![
            symbol("battery", "BT1", -10.0, 0.0, "24"),
            wire(&[(-2.5, 0.0), (0.0, 0.0)], 0.75, None),
            // 全長100mm=実長10m。ジャンクション位置40mmで 4m:6m に分割される
            wire(&[(0.0, 0.0), (100.0, 0.0)], 1.0, Some(10.0)),
            j,
            wire(&[(40.0, 0.0), (40.0, 20.0)], 0.75, None),
        ]);
        let deck = build_deck(&sheet, &sheet_symbol_defs(&sheet)).expect("deck");
        let split: Vec<f64> = deck
            .wire_elements
            .iter()
            .filter(|e| (e.ohms - 0.07).abs() < 1e-9 || (e.ohms - 0.105).abs() < 1e-9)
            .map(|e| e.ohms)
            .collect();
        assert_eq!(split.len(), 2, "{:?}", deck.wire_elements);
        // 長さ未設定のワイヤは微小抵抗
        assert!(
            deck.wire_elements.iter().filter(|e| (e.ohms - BRIDGE_OHMS).abs() < 1e-12).count() >= 2,
            "{:?}",
            deck.wire_elements
        );
    }

    #[test]
    fn same_name_labels_are_bridged() {
        let mk_label = |x: f64, y: f64| {
            Entity::NetLabel(NetLabel {
                id: Uuid::new_v4(),
                at: Point::new(x, y),
                name: "24-P1".into(),
                rotation: 0,
            })
        };
        let sheet = sheet_with(vec![
            symbol("battery", "BT1", -10.0, 0.0, "24"),
            wire(&[(-2.5, 0.0), (20.0, 0.0)], 0.75, None),
            mk_label(20.0, 0.0),
            wire(&[(100.0, 50.0), (120.0, 50.0)], 0.75, None),
            mk_label(100.0, 50.0),
        ]);
        let deck = build_deck(&sheet, &sheet_symbol_defs(&sheet)).expect("deck");
        // 同名ラベル2点の橋(1mΩ)がある
        assert!(
            deck.component_elements.iter().any(|e| (e.ohms - BRIDGE_OHMS).abs() < 1e-12),
            "{:?}",
            deck.component_elements
        );
    }

    #[test]
    fn no_battery_is_an_error() {
        let sheet = sheet_with(vec![wire(&[(0.0, 0.0), (10.0, 0.0)], 0.75, None)]);
        assert_eq!(
            build_deck(&sheet, &sheet_symbol_defs(&sheet)).unwrap_err(),
            SpiceError::NoSource
        );
    }

    #[test]
    fn node_names_are_deterministic_and_ground_is_battery_minus() {
        let sheet = sheet_with(vec![
            symbol("battery", "BT1", 60.0, 100.0, "DC24V"),
            wire(&[(67.5, 100.0), (92.5, 100.0)], 0.75, None),
        ]);
        let d1 = build_deck(&sheet, &sheet_symbol_defs(&sheet)).expect("deck");
        let d2 = build_deck(&sheet, &sheet_symbol_defs(&sheet)).expect("deck");
        assert_eq!(d1.deck, d2.deck, "決定的な出力");
        // battery負極(pin2 = 67.5,100)がGND → ワイヤ素子の片側が"0"
        let w = &d1.wire_elements[0];
        assert!(w.node_a == "0" || w.node_b == "0", "{w:?}");
        let _ = BTreeMap::<String, f64>::new();
    }
}
