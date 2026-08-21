//! 検証エンジン (spec §3.5)。ERC+電気検証をネットリストの上に構築する。
//! 読み取り専用の純関数で、モデルは変更しない。

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::geometry::Point;
use crate::model::{Entity, Sheet, SheetId};
use crate::netlist::{extract_netlist, on_wire, pin_positions, CONNECT_EPS};
use crate::symbol::SymbolDef;
use crate::EntityId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// 検証結果の1件。UIは`entity_ids`で該当エンティティを選択・ズームする。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Diagnostic {
    pub severity: Severity,
    /// 安定ID (例: "erc.unconnected_pin")。
    pub code: String,
    pub message: String,
    pub sheet_id: SheetId,
    pub entity_ids: Vec<EntityId>,
}

/// シート1枚のERC+電気検証。
pub fn verify_sheet(sheet: &Sheet, symbols: &[SymbolDef]) -> Vec<Diagnostic> {
    erc(sheet, symbols)
}

fn near(a: &Point, b: &Point) -> bool {
    a.distance_to(b) < CONNECT_EPS
}

fn erc(sheet: &Sheet, symbols: &[SymbolDef]) -> Vec<Diagnostic> {
    let defs: BTreeMap<&str, &SymbolDef> = symbols.iter().map(|d| (d.id.as_str(), d)).collect();
    let nets = extract_netlist(sheet, symbols);
    let mut diags = Vec::new();
    let diag = |severity: Severity, code: &str, message: String, ids: Vec<EntityId>| Diagnostic {
        severity,
        code: code.into(),
        message,
        sheet_id: sheet.id,
        entity_ids: ids,
    };

    // ネットに属する (シンボルid, ピン番号) の集合。端子台は左右どちらかが繋がれば接続扱い
    let connected: BTreeSet<(EntityId, &str)> = nets
        .iter()
        .flat_map(|n| n.pins.iter().map(|p| (p.entity_id, p.pin.as_str())))
        .collect();

    // 未接続ピン(シンボル単位に集約)+空参照
    for e in sheet.entities.values() {
        let Entity::Symbol(s) = e else { continue };
        let Some(def) = defs.get(s.symbol_id.as_str()) else { continue };
        let mut seen = BTreeSet::new();
        let mut unconnected = Vec::new();
        for pin in &def.pins {
            if seen.insert(pin.number.as_str())
                && !connected.contains(&(s.id, pin.number.as_str()))
            {
                unconnected.push(pin.number.as_str());
            }
        }
        let label = if s.reference.is_empty() {
            s.symbol_id.as_str()
        } else {
            s.reference.as_str()
        };
        if !unconnected.is_empty() {
            diags.push(diag(
                Severity::Warning,
                "erc.unconnected_pin",
                format!("{label}: 未接続のピン: {}", unconnected.join(", ")),
                vec![s.id],
            ));
        }
        if s.reference.trim().is_empty() {
            diags.push(diag(
                Severity::Warning,
                "erc.empty_reference",
                format!("参照記号が未設定のシンボル ({})", s.symbol_id),
                vec![s.id],
            ));
        }
    }

    // 参照記号の重複。リレー同士(コイル+接点、多接点)は正当なので除外
    let mut by_ref: BTreeMap<&str, Vec<(EntityId, &str)>> = BTreeMap::new();
    for e in sheet.entities.values() {
        if let Entity::Symbol(s) = e {
            if s.reference.trim().is_empty() {
                continue;
            }
            let category = defs
                .get(s.symbol_id.as_str())
                .map(|d| d.category.as_str())
                .unwrap_or("");
            by_ref.entry(s.reference.as_str()).or_default().push((s.id, category));
        }
    }
    for (reference, members) in &by_ref {
        if members.len() >= 2 && !members.iter().all(|(_, cat)| *cat == "relay") {
            diags.push(diag(
                Severity::Error,
                "erc.duplicate_reference",
                format!("参照記号 {reference} が{}回使われています", members.len()),
                members.iter().map(|(id, _)| *id).collect(),
            ));
        }
    }

    // 宙ぶらりんワイヤ端点(ピン・ジャンクション・他ワイヤ端点・ネットラベルのどれにも一致しない)
    let mut pin_points = Vec::new();
    let mut junctions = Vec::new();
    let mut labels = Vec::new();
    let mut wires = Vec::new();
    for e in sheet.entities.values() {
        match e {
            Entity::Symbol(s) => {
                if let Some(def) = defs.get(s.symbol_id.as_str()) {
                    pin_points.extend(pin_positions(s, def).into_iter().map(|(_, p)| p));
                }
            }
            Entity::Junction(j) => junctions.push(j.at),
            Entity::NetLabel(l) => labels.push(l.at),
            Entity::Wire(w) => wires.push(w),
            Entity::Text(_) => {}
        }
    }
    for w in &wires {
        let endpoints: Vec<Point> = match (w.points.first(), w.points.last()) {
            (Some(a), Some(b)) => vec![*a, *b],
            _ => continue,
        };
        let dangling = endpoints
            .iter()
            .filter(|ep| {
                let attached = pin_points.iter().any(|p| near(p, ep))
                    || junctions.iter().any(|p| near(p, ep))
                    || labels.iter().any(|p| near(p, ep))
                    || wires.iter().any(|other| {
                        other.id != w.id
                            && [other.points.first(), other.points.last()]
                                .into_iter()
                                .flatten()
                                .any(|p| near(p, ep))
                    });
                !attached
            })
            .count();
        if dangling > 0 {
            diags.push(diag(
                Severity::Warning,
                "erc.dangling_wire",
                format!("どこにも接続されていないワイヤ端点があります ({dangling}箇所)"),
                vec![w.id],
            ));
        }
    }

    // 1ネット上の異名ネットラベル(異電位ネットの直結の代表例)
    for net in &nets {
        let net_wires: Vec<&crate::model::Wire> = net
            .wire_ids
            .iter()
            .filter_map(|id| match sheet.entities.get(id) {
                Some(Entity::Wire(w)) => Some(w),
                _ => None,
            })
            .collect();
        let mut names = BTreeSet::new();
        let mut label_ids = Vec::new();
        for e in sheet.entities.values() {
            if let Entity::NetLabel(l) = e {
                if net_wires.iter().any(|w| on_wire(w, &l.at)) {
                    names.insert(l.name.as_str());
                    label_ids.push(l.id);
                }
            }
        }
        if names.len() >= 2 {
            diags.push(diag(
                Severity::Error,
                "erc.label_conflict",
                format!(
                    "1つのネットに異なるネットラベルが混在しています: {}",
                    names.into_iter().collect::<Vec<_>>().join(", ")
                ),
                label_ids,
            ));
        }
    }

    diags
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::*;
    use crate::symbol::sheet_symbol_defs;
    use uuid::Uuid;

    fn wire(points: &[(f64, f64)]) -> Entity {
        Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            color: "black".into(),
            sq: 0.75,
            length_m: None,
            part_no: None,
            net: None,
        })
    }

    fn symbol(sym: &str, reference: &str, x: f64, y: f64) -> Entity {
        Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: sym.into(),
            at: Point::new(x, y),
            rotation: 0,
            mirror: false,
            reference: reference.into(),
            value: String::new(),
            attrs: Default::default(),
        })
    }

    fn sheet_with(entities: Vec<Entity>) -> Sheet {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        for e in entities {
            sheet.entities.insert(e.id(), e);
        }
        sheet
    }

    fn run(sheet: &Sheet) -> Vec<Diagnostic> {
        verify_sheet(sheet, &sheet_symbol_defs(sheet))
    }

    fn codes(diags: &[Diagnostic]) -> Vec<&str> {
        diags.iter().map(|d| d.code.as_str()).collect()
    }

    #[test]
    fn fully_wired_pair_has_no_erc_findings() {
        // R1-R2を両端とも配線: ERC指摘なし
        let sheet = sheet_with(vec![
            symbol("resistor", "R1", 100.0, 50.0),
            symbol("resistor", "R2", 150.0, 50.0),
            wire(&[(107.5, 50.0), (142.5, 50.0)]),
            wire(&[(92.5, 50.0), (80.0, 50.0), (80.0, 80.0), (180.0, 80.0), (157.5, 50.0)]),
        ]);
        // 2本目のワイヤは最後の点がピンに一致するよう折り返す
        let diags = run(&sheet);
        let erc: Vec<_> = diags.iter().filter(|d| d.code.starts_with("erc.")).collect();
        assert!(erc.is_empty(), "{erc:?}");
    }

    #[test]
    fn unconnected_pins_are_reported_per_symbol() {
        // R1のピン2のみ配線 → ピン1が未接続。ワイヤの反対側は R2 ピン1 に接続
        let sheet = sheet_with(vec![
            symbol("resistor", "R1", 100.0, 50.0),
            symbol("resistor", "R2", 150.0, 50.0),
            wire(&[(107.5, 50.0), (142.5, 50.0)]),
        ]);
        let diags = run(&sheet);
        let unconn: Vec<_> = diags
            .iter()
            .filter(|d| d.code == "erc.unconnected_pin")
            .collect();
        // R1のピン1, R2のピン2 がそれぞれ1件ずつ(シンボル単位に集約)
        assert_eq!(unconn.len(), 2, "{unconn:?}");
        assert!(unconn.iter().all(|d| d.severity == Severity::Warning));
        assert!(unconn.iter().any(|d| d.message.contains("R1") && d.message.contains("1")));
    }

    #[test]
    fn terminal_block_terminal_counts_connected_if_either_side_wired() {
        // 端子台2極: 端子1は左側のみ配線(接続扱い)、端子2は未配線(未接続)
        let sheet = sheet_with(vec![
            symbol("terminal_block_2p", "TB1", 100.0, 50.0),
            // 端子1(上段, y=47.5)の左ピン(97.5, 47.5)へ、電源側はR1
            symbol("resistor", "R1", 60.0, 47.5),
            wire(&[(67.5, 47.5), (97.5, 47.5)]),
            // R1の反対側も適当に配線してR1側の指摘を消す
            symbol("resistor", "R2", 20.0, 47.5),
            wire(&[(27.5, 47.5), (52.5, 47.5)]),
            wire(&[(12.5, 47.5), (5.0, 47.5)]),
        ]);
        let diags = run(&sheet);
        let unconn: Vec<_> = diags
            .iter()
            .filter(|d| d.code == "erc.unconnected_pin" && d.message.contains("TB1"))
            .collect();
        assert_eq!(unconn.len(), 1, "{unconn:?}");
        assert!(unconn[0].message.contains("2"), "端子2のみ未接続: {}", unconn[0].message);
        assert!(!unconn[0].message.contains("端子番号 1"), "{}", unconn[0].message);
    }

    #[test]
    fn empty_and_duplicate_references_are_flagged_but_relays_allowed() {
        let sheet = sheet_with(vec![
            symbol("resistor", "", 100.0, 50.0),   // 空参照
            symbol("resistor", "R1", 150.0, 50.0), // 重複
            symbol("resistor", "R1", 200.0, 50.0), // 重複
            symbol("relay_coil", "K1", 100.0, 100.0), // リレーはコイル+接点で同参照OK
            symbol("relay_contact_no", "K1", 150.0, 100.0),
            symbol("relay_contact_no", "K1", 200.0, 100.0),
        ]);
        let diags = run(&sheet);
        assert_eq!(
            diags.iter().filter(|d| d.code == "erc.empty_reference").count(),
            1
        );
        let dups: Vec<_> = diags
            .iter()
            .filter(|d| d.code == "erc.duplicate_reference")
            .collect();
        assert_eq!(dups.len(), 1, "{dups:?}");
        assert_eq!(dups[0].severity, Severity::Error);
        assert!(dups[0].message.contains("R1"));
        assert_eq!(dups[0].entity_ids.len(), 2);
        assert!(!diags.iter().any(|d| d.code == "erc.duplicate_reference" && d.message.contains("K1")));
    }

    #[test]
    fn dangling_wire_end_is_flagged_including_missing_junction() {
        // w1は左端がR1ピンに接続、右端が空中 → 宙ぶらりん
        // w2は端点がw3の途中に乗るがジャンクション無し → 宙ぶらりん
        let sheet = sheet_with(vec![
            symbol("resistor", "R1", 100.0, 50.0),
            wire(&[(107.5, 50.0), (130.0, 50.0)]),
            wire(&[(200.0, 40.0), (200.0, 60.0)]),
            wire(&[(180.0, 50.0), (200.0, 50.0)]),
        ]);
        let diags = run(&sheet);
        let dangling: Vec<_> = diags
            .iter()
            .filter(|d| d.code == "erc.dangling_wire")
            .collect();
        // w1右端 / w2両端(40,60側は完全に浮き) / w3左端 + w3右端(ジャンクション無しでw2の途中) 相当
        assert!(dangling.len() >= 3, "{dangling:?}");
    }

    #[test]
    fn conflicting_net_labels_on_one_net_are_an_error() {
        let l1 = Entity::NetLabel(NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(0.0, 10.0),
            name: "24-P1".into(),
            rotation: 0,
        });
        let l2 = Entity::NetLabel(NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(20.0, 10.0),
            name: "0V".into(),
            rotation: 0,
        });
        let sheet = sheet_with(vec![wire(&[(0.0, 10.0), (20.0, 10.0)]), l1, l2]);
        let diags = run(&sheet);
        let conflicts: Vec<_> = diags
            .iter()
            .filter(|d| d.code == "erc.label_conflict")
            .collect();
        assert_eq!(conflicts.len(), 1, "{conflicts:?}");
        assert_eq!(conflicts[0].severity, Severity::Error);
        assert!(conflicts[0].message.contains("24-P1") && conflicts[0].message.contains("0V"));
    }
}
