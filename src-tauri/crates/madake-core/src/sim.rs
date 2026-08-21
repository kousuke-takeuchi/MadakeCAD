//! DC動作点シミュレーション (spec §3.6)。ngspice(サブプロセス)で解き、
//! 各ネットの電圧・各部品の電流/電力をユーザー向けに返す。
//!
//! 検証エンジン(verify)と違い近似フォールバックはしない: シミュレーションは
//! 「実解を見る」機能なので、ngspice未導入は明示エラーで導入を案内する。

use std::collections::BTreeMap;

use serde::Serialize;

use crate::model::{Entity, Sheet};
use crate::symbol::SymbolDef;
use crate::EntityId;

#[derive(Debug, thiserror::Error)]
pub enum SimError {
    #[error(
        "ngspiceが見つかりません。導入してください (macOS: brew install ngspice / Linux: apt install ngspice / Windows: 公式インストーラ)。実行ファイルは環境変数MADAKE_NGSPICEでも指定できます"
    )]
    NgspiceNotFound,
    #[error("SPICE変換に失敗しました: {0}")]
    Deck(#[from] crate::spice::SpiceError),
    #[error("ngspiceの実行に失敗しました: {0}")]
    Ngspice(#[from] crate::ngspice::NgspiceError),
}

/// ネット1つの電圧。配線抵抗でノードが分かれるため範囲(min/max)で報告する。
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct NetVoltage {
    pub name: String,
    pub volts_min: f64,
    pub volts_max: f64,
    pub wire_ids: Vec<EntityId>,
}

/// 部品1つの電流と電力。
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct ComponentCurrent {
    pub reference: String,
    pub entity_id: EntityId,
    pub amps: f64,
    pub watts: f64,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SimOpResult {
    /// 電源電圧 (V)。
    pub voltage: f64,
    pub nets: Vec<NetVoltage>,
    pub components: Vec<ComponentCurrent>,
    pub warnings: Vec<String>,
}

/// DC動作点を解く。`open_switches`の参照記号の導通部品は開路として扱う。
pub fn simulate_op(
    sheet: &Sheet,
    symbols: &[SymbolDef],
    open_switches: &[String],
) -> Result<SimOpResult, SimError> {
    let exe = crate::ngspice::find_ngspice().ok_or(SimError::NgspiceNotFound)?;
    simulate_op_with(&exe, sheet, symbols, open_switches)
}

/// 実行ファイルを指定して解く (テスト・カスタムパス用)。
pub fn simulate_op_with(
    exe: &std::path::Path,
    sheet: &Sheet,
    symbols: &[SymbolDef],
    open_switches: &[String],
) -> Result<SimOpResult, SimError> {
    let opts = crate::spice::DeckOptions {
        open_switches: open_switches.to_vec(),
    };
    let deck = crate::spice::build_deck_with(sheet, symbols, &opts)?;
    let volts = crate::ngspice::run_op(exe, &deck.deck)?;
    let volt_of = |node: &str| -> f64 {
        if node == "0" {
            0.0
        } else {
            volts.get(node).copied().unwrap_or(0.0)
        }
    };

    let mut warnings = Vec::new();
    if !open_switches.is_empty() {
        warnings.push(format!("開路指定: {}", open_switches.join(", ")));
    }

    // ネット電圧: ネットの配線に対応するSPICE素子の両端ノード電圧のmin/max
    let nets = crate::netlist::extract_netlist(sheet, symbols);
    let mut net_voltages = Vec::new();
    for net in &nets {
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        for el in &deck.wire_elements {
            if net.wire_ids.contains(&el.entity_id) {
                for node in [&el.node_a, &el.node_b] {
                    let v = volt_of(node);
                    min = min.min(v);
                    max = max.max(v);
                }
            }
        }
        if min.is_finite() {
            net_voltages.push(NetVoltage {
                name: net.name.clone(),
                volts_min: min,
                volts_max: max,
                wire_ids: net.wire_ids.clone(),
            });
        }
    }

    // 部品電流・電力: 部品由来の橋/負荷素子から算出
    let mut per_component: BTreeMap<EntityId, (f64, f64)> = BTreeMap::new();
    for el in &deck.component_elements {
        let dv = (volt_of(&el.node_a) - volt_of(&el.node_b)).abs();
        let amps = if el.ohms > 0.0 { dv / el.ohms } else { 0.0 };
        let e = per_component.entry(el.entity_id).or_insert((0.0, 0.0));
        e.0 = e.0.max(amps);
        e.1 += dv * amps;
    }
    let mut components = Vec::new();
    for (entity_id, (amps, watts)) in per_component {
        let Some(Entity::Symbol(s)) = sheet.entities.get(&entity_id) else { continue };
        let reference = if s.reference.is_empty() {
            s.symbol_id.clone()
        } else {
            s.reference.clone()
        };
        components.push(ComponentCurrent {
            reference,
            entity_id,
            amps,
            watts,
        });
    }
    components.sort_by(|a, b| a.reference.cmp(&b.reference));

    Ok(SimOpResult {
        voltage: deck.voltage,
        nets: net_voltages,
        components,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::*;
    use crate::symbol::sheet_symbol_defs;
    use uuid::Uuid;

    fn series_circuit(lamp_current: &str) -> Sheet {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let mut lamp_attrs = std::collections::BTreeMap::new();
        lamp_attrs.insert("current_a".to_string(), lamp_current.to_string());
        let syms = [
            ("battery", "BT1", 60.0, "DC24V", std::collections::BTreeMap::new()),
            ("fuse", "F1", 100.0, "10A", std::collections::BTreeMap::new()),
            ("switch_spst", "SW1", 140.0, "", std::collections::BTreeMap::new()),
            ("lamp", "L1", 190.0, "", lamp_attrs),
        ];
        for (sid, r, x, v, attrs) in syms {
            let e = Entity::Symbol(SymbolInstance {
                id: Uuid::new_v4(),
                symbol_id: sid.into(),
                at: Point::new(x, 100.0),
                rotation: 0,
                mirror: false,
                reference: r.into(),
                value: v.into(),
                attrs,
            });
            sheet.entities.insert(e.id(), e);
        }
        let wires = [
            vec![(67.5, 100.0), (92.5, 100.0)],
            vec![(107.5, 100.0), (132.5, 100.0)],
            vec![(147.5, 100.0), (182.5, 100.0)],
            vec![
                (197.5, 100.0),
                (220.0, 100.0),
                (220.0, 140.0),
                (40.0, 140.0),
                (40.0, 100.0),
                (52.5, 100.0),
            ],
        ];
        for pts in wires {
            let e = Entity::Wire(Wire {
                id: Uuid::new_v4(),
                points: pts.into_iter().map(|(x, y)| Point::new(x, y)).collect(),
                color: "red".into(),
                sq: 0.75,
                length_m: None,
                part_no: None,
                net: None,
            });
            sheet.entities.insert(e.id(), e);
        }
        sheet
    }

    #[test]
    fn missing_ngspice_is_an_explicit_error() {
        let sheet = series_circuit("2");
        let err = simulate_op_with(
            std::path::Path::new("/nonexistent/ngspice"),
            &sheet,
            &sheet_symbol_defs(&sheet),
            &[],
        )
        .unwrap_err();
        assert!(matches!(err, SimError::Ngspice(_)), "{err:?}");
    }

    #[test]
    fn op_reports_net_voltages_and_component_currents() {
        let Some(exe) = crate::ngspice::find_ngspice() else {
            eprintln!("ngspice未検出のためスキップ");
            return;
        };
        let sheet = series_circuit("2");
        let r = simulate_op_with(&exe, &sheet, &sheet_symbol_defs(&sheet), &[]).unwrap();
        assert!((r.voltage - 24.0).abs() < 1e-9);
        // 2A負荷 (24V/12Ω)。配線・橋は1mΩなのでほぼ2A
        let l1 = r.components.iter().find(|c| c.reference == "L1").unwrap();
        assert!((l1.amps - 2.0).abs() < 0.01, "{l1:?}");
        assert!((l1.watts - 48.0).abs() < 0.5, "{l1:?}");
        let f1 = r.components.iter().find(|c| c.reference == "F1").unwrap();
        assert!((f1.amps - 2.0).abs() < 0.01, "{f1:?}");
        // ネットは4つ、電圧は0V〜24Vの範囲
        assert_eq!(r.nets.len(), 4, "{:?}", r.nets);
        let vmax = r.nets.iter().map(|n| n.volts_max).fold(0.0, f64::max);
        assert!((vmax - 24.0).abs() < 0.01, "{vmax}");
    }

    #[test]
    fn open_switch_cuts_the_current() {
        let Some(exe) = crate::ngspice::find_ngspice() else {
            eprintln!("ngspice未検出のためスキップ");
            return;
        };
        let sheet = series_circuit("2");
        let r = simulate_op_with(
            &exe,
            &sheet,
            &sheet_symbol_defs(&sheet),
            &["SW1".to_string()],
        )
        .unwrap();
        let l1 = r.components.iter().find(|c| c.reference == "L1").unwrap();
        assert!(l1.amps < 1e-3, "開路で電流はほぼ0: {l1:?}");
        assert!(r.warnings.iter().any(|w| w.contains("SW1")));
    }
}
