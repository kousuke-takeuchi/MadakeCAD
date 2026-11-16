//! DC動作点シミュレーション (spec §3.6)。ngspice(サブプロセス)で解き、
//! 各ネットの電圧・各部品の電流/電力をユーザー向けに返す。
//!
//! 検証エンジン(verify)と違い近似フォールバックはしない: シミュレーションは
//! 「実解を見る」機能なので、ngspice未導入は明示エラーで導入を案内する。
//!
//! 条件 ([`SimOptions`]): スイッチの開路、部品の属性の置き換え (例: モーターの拘束電流)、
//! ネットの地絡。結果の [`SimFinding`] は定格を超えた所 (ヒューズ・ピン・電源・変換器・電線・負荷の電圧)。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::{Entity, Sheet};
use crate::spice::ElementKind;
use crate::symbol::{SimRole, SymbolDef};
use crate::verify::Severity;
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

/// シミュレーションの条件。全項目が省略可 (省略 = 図面のまま)。
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SimOptions {
    /// 開路として扱う導通部品の参照記号。
    #[serde(default)]
    pub open_switches: Vec<String>,
    /// 部品の属性の置き換え (参照記号 → {属性: 値})。例: {"M1": {"current_a": "40"}}。
    #[serde(default)]
    pub overrides: BTreeMap<String, BTreeMap<String, String>>,
    /// 接地へ短絡させるネットの名前。
    #[serde(default)]
    pub shorts: Vec<String>,
    /// 故障 (電線・ピンで指定した短絡、電線の断線)。
    #[serde(default)]
    pub faults: Vec<crate::model::SimFault>,
}

impl SimOptions {
    /// 計算ケースの条件を足す (直接指定した置き換えはケースより優先)。
    pub fn with_case(mut self, case: &crate::model::SimCase) -> Self {
        for s in &case.open_switches {
            if !self.open_switches.contains(s) {
                self.open_switches.push(s.clone());
            }
        }
        for (r, attrs) in &case.overrides {
            let slot = self.overrides.entry(r.clone()).or_default();
            for (k, v) in attrs {
                slot.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
        self.faults.extend(case.faults.iter().cloned());
        self
    }
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

/// 定格を超えた所1件。
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct SimFinding {
    /// 安定コード (`sim.fuse_rating` など)。
    pub code: String,
    pub severity: Severity,
    /// 部品の参照記号 (電線なら空)。
    pub reference: String,
    pub entity_ids: Vec<EntityId>,
    /// 測った値 (A または V)。
    pub value: f64,
    /// 比べた限度 (A または V)。
    pub limit: f64,
    pub message: String,
}


/// 電線か部品か。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LoadKind {
    Wire,
    Component,
}

/// 負荷率の限度の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LimitKind {
    WireAmpacity,
    FuseRating,
    PinRating,
    SourceMax,
    ConverterOut,
}

/// 電線・部品1つの電流・電圧・限度 (図面に重ねる表示とプローブに使う)。
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct EntityLoad {
    pub entity_id: EntityId,
    pub kind: LoadKind,
    /// 電線は区間の最大、部品は素子の最大 (A)。
    pub amps: f64,
    /// 電線は [始点, 終点]、部品はピンの順の電圧 (V)。
    pub volts: Vec<f64>,
    pub limit: Option<f64>,
    pub limit_kind: Option<LimitKind>,
    /// amps / limit。
    pub ratio: Option<f64>,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SimOpResult {
    /// 電源電圧 (V)。最初の電源の電圧。
    pub voltage: f64,
    pub nets: Vec<NetVoltage>,
    pub components: Vec<ComponentCurrent>,
    pub warnings: Vec<String>,
    /// 定格を超えた所 (重い順: エラー → 警告 → 情報)。
    pub findings: Vec<SimFinding>,
    /// 電線・部品ごとの電流・電圧・限度・負荷率 (図面に重ねる表示とプローブ)。電線が先、それぞれ id 順。
    pub loads: Vec<EntityLoad>,
}

/// DC動作点を解く。`open_switches`の参照記号の導通部品は開路として扱う。
pub fn simulate_op(
    sheet: &Sheet,
    symbols: &[SymbolDef],
    open_switches: &[String],
) -> Result<SimOpResult, SimError> {
    let opts = SimOptions { open_switches: open_switches.to_vec(), ..Default::default() };
    simulate_op_opts(sheet, symbols, &opts)
}

/// 条件付きで DC 動作点を解く。
pub fn simulate_op_opts(sheet: &Sheet, symbols: &[SymbolDef], opts: &SimOptions) -> Result<SimOpResult, SimError> {
    let exe = crate::ngspice::find_ngspice().ok_or(SimError::NgspiceNotFound)?;
    simulate_op_opts_with(&exe, sheet, symbols, opts)
}

/// 実行ファイルを指定して解く (テスト・カスタムパス用)。
pub fn simulate_op_with(
    exe: &std::path::Path,
    sheet: &Sheet,
    symbols: &[SymbolDef],
    open_switches: &[String],
) -> Result<SimOpResult, SimError> {
    let opts = SimOptions { open_switches: open_switches.to_vec(), ..Default::default() };
    simulate_op_opts_with(exe, sheet, symbols, &opts)
}

/// 実行ファイルと条件を指定して解く。
pub fn simulate_op_opts_with(
    exe: &std::path::Path,
    sheet: &Sheet,
    symbols: &[SymbolDef],
    opts: &SimOptions,
) -> Result<SimOpResult, SimError> {
    let open_switches = &opts.open_switches;
    let deck_opts = crate::spice::DeckOptions {
        open_switches: opts.open_switches.clone(),
        overrides: opts.overrides.clone(),
        shorts: opts.shorts.clone(),
        faults: opts.faults.clone(),
    };
    let deck = crate::spice::build_deck_with(sheet, symbols, &deck_opts)?;
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
    for (r, attrs) in &opts.overrides {
        let list: Vec<String> = attrs.iter().map(|(k, v)| format!("{k}={v}")).collect();
        warnings.push(format!("置き換え: {r} {}", list.join(" ")));
    }
    warnings.extend(deck.warnings.iter().cloned());

    // ネット電圧: ネットの配線に対応するSPICE素子の両端ノード電圧のmin/max
    // SPICE はポートを知らないので、描いた配線のまとまりごと (接点に分けない)
    let nets = crate::netlist::extract_wire_nets(sheet, symbols);
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

    let amps_of = |el: &crate::spice::SpiceElement| -> f64 {
        let dv = (volt_of(&el.node_a) - volt_of(&el.node_b)).abs();
        if el.ohms > 0.0 { dv / el.ohms } else { 0.0 }
    };

    // 部品電流・電力: 部品由来の橋/負荷素子から算出
    let mut per_component: BTreeMap<EntityId, (f64, f64)> = BTreeMap::new();
    for el in &deck.component_elements {
        if matches!(el.kind, ElementKind::Label | ElementKind::Short(_) | ElementKind::Tie) {
            continue;
        }
        let dv = (volt_of(&el.node_a) - volt_of(&el.node_b)).abs();
        let amps = amps_of(el);
        let e = per_component.entry(el.entity_id).or_insert((0.0, 0.0));
        e.0 = e.0.max(amps);
        e.1 += dv * amps;
    }
    // 役割が負荷の部品の電力は端子電圧 × 電流 (定電流源の電力は検出抵抗に出ないため)
    for t in &deck.load_terminals {
        if let Some(e) = per_component.get_mut(&t.entity_id) {
            e.1 = (volt_of(&t.plus) - volt_of(&t.minus)).abs() * e.0;
        }
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

    let findings = findings(sheet, symbols, &deck_opts, &deck, &volt_of, &amps_of);
    let loads = loads(sheet, symbols, &deck_opts, &deck, &volt_of, &amps_of);

    Ok(SimOpResult {
        voltage: deck.voltage,
        nets: net_voltages,
        components,
        warnings,
        findings,
        loads,
    })
}


/// 電線・部品ごとの負荷。限度は判定 (findings) と同じ規則で、部品で複数あれば負荷率の一番高いもの。
fn loads(
    sheet: &Sheet,
    symbols: &[SymbolDef],
    opts: &crate::spice::DeckOptions,
    deck: &crate::spice::SpiceDeck,
    volt_of: &dyn Fn(&str) -> f64,
    amps_of: &dyn Fn(&crate::spice::SpiceElement) -> f64,
) -> Vec<EntityLoad> {
    use crate::verify::{ampacity, parse_number, AMPACITY};
    let defs: BTreeMap<&str, &SymbolDef> = symbols.iter().map(|d| (d.id.as_str(), d)).collect();
    let table_max_sq = AMPACITY.last().map(|&(sq, _)| sq).unwrap_or(0.0);
    let mut out = Vec::new();

    // 電線: 区間を弧長の順に並べてあるので、最初の区間の node_a が始点、最後の区間の node_b が終点
    let mut by_wire: BTreeMap<EntityId, Vec<&crate::spice::SpiceElement>> = BTreeMap::new();
    for el in &deck.wire_elements {
        by_wire.entry(el.entity_id).or_default().push(el);
    }
    for (id, els) in by_wire {
        let Some(Entity::Wire(w)) = sheet.entities.get(&id) else { continue };
        let amps = els.iter().map(|e| amps_of(e)).fold(0.0, f64::max);
        let volts = vec![volt_of(&els[0].node_a), volt_of(&els[els.len() - 1].node_b)];
        let limit = (w.sq > 0.0 && w.sq <= table_max_sq + 1e-9).then(|| ampacity(w.sq));
        out.push(EntityLoad {
            entity_id: id,
            kind: LoadKind::Wire,
            amps,
            volts,
            limit,
            limit_kind: limit.map(|_| LimitKind::WireAmpacity),
            ratio: limit.map(|l| amps / l),
        });
    }

    // 部品: ラベル橋・短絡・接地の基準は数えない
    let mut by_symbol: BTreeMap<EntityId, Vec<&crate::spice::SpiceElement>> = BTreeMap::new();
    for el in &deck.component_elements {
        if matches!(el.kind, ElementKind::Label | ElementKind::Short(_) | ElementKind::Tie) {
            continue;
        }
        by_symbol.entry(el.entity_id).or_default().push(el);
    }
    let pin_nodes: BTreeMap<EntityId, &Vec<(String, String)>> = deck.pin_nodes.iter().map(|(id, l)| (*id, l)).collect();
    for (id, els) in by_symbol {
        let Some(Entity::Symbol(s)) = sheet.entities.get(&id) else { continue };
        let Some(def) = defs.get(s.symbol_id.as_str()) else { continue };
        let role = def.sim.as_ref().map(|r| r.role);
        let max_kind = |pred: &dyn Fn(&ElementKind) -> bool| -> f64 {
            els.iter().filter(|e| pred(&e.kind)).map(|e| amps_of(e)).fold(0.0, f64::max)
        };
        let amps = els.iter().map(|e| amps_of(e)).fold(0.0, f64::max);
        let mut candidates: Vec<(LimitKind, f64, f64)> = Vec::new(); // (種類, 限度, その限度と比べる電流)
        if role == Some(SimRole::Fuse) || (role.is_none() && def.id == "fuse") {
            if let Some(r) = opts.number(s, "rating_a").or_else(|| parse_number(&s.value)) {
                candidates.push((LimitKind::FuseRating, r, max_kind(&|k| matches!(k, ElementKind::Bridge))));
            }
        }
        if let Some(r) = opts.number(s, "pin_rating_a") {
            candidates.push((LimitKind::PinRating, r, max_kind(&|k| matches!(k, ElementKind::Pin(_) | ElementKind::Bridge))));
        }
        if role == Some(SimRole::Source) {
            if let Some(r) = opts.number(s, "i_max_a") {
                candidates.push((LimitKind::SourceMax, r, max_kind(&|k| matches!(k, ElementKind::Source))));
            }
        }
        if role == Some(SimRole::Converter) {
            if let Some(r) = opts.number(s, "i_out_a") {
                candidates.push((LimitKind::ConverterOut, r, max_kind(&|k| matches!(k, ElementKind::ConverterOut))));
            }
        }
        let best = candidates
            .into_iter()
            .filter(|(_, limit, _)| *limit > 0.0)
            .max_by(|a, b| (a.2 / a.1).total_cmp(&(b.2 / b.1)));
        let volts = pin_nodes.get(&id).map(|l| l.iter().map(|(_, n)| volt_of(n)).collect()).unwrap_or_default();
        out.push(EntityLoad {
            entity_id: id,
            kind: LoadKind::Component,
            amps,
            volts,
            limit: best.map(|b| b.1),
            limit_kind: best.map(|b| b.0),
            ratio: best.map(|b| b.2 / b.1),
        });
    }
    out
}

/// 定格の判定。属性 (置き換え後) の限度と、シミュレーションの電流・電圧を比べる。
fn findings(
    sheet: &Sheet,
    symbols: &[SymbolDef],
    opts: &crate::spice::DeckOptions,
    deck: &crate::spice::SpiceDeck,
    volt_of: &dyn Fn(&str) -> f64,
    amps_of: &dyn Fn(&crate::spice::SpiceElement) -> f64,
) -> Vec<SimFinding> {
    use crate::verify::{ampacity, parse_number, AMPACITY};
    let defs: BTreeMap<&str, &SymbolDef> = symbols.iter().map(|d| (d.id.as_str(), d)).collect();
    let mut out: Vec<SimFinding> = Vec::new();
    // 部品ごとの素子
    let mut by_symbol: BTreeMap<EntityId, Vec<&crate::spice::SpiceElement>> = BTreeMap::new();
    for el in &deck.component_elements {
        by_symbol.entry(el.entity_id).or_default().push(el);
    }
    let max_of = |els: &[&crate::spice::SpiceElement], pred: &dyn Fn(&ElementKind) -> bool| -> f64 {
        els.iter().filter(|e| pred(&e.kind)).map(|e| amps_of(e)).fold(0.0, f64::max)
    };
    for e in sheet.entities.values() {
        let Entity::Symbol(s) = e else { continue };
        let Some(def) = defs.get(s.symbol_id.as_str()) else { continue };
        let els = by_symbol.get(&s.id).map(Vec::as_slice).unwrap_or(&[]);
        let label = if s.reference.is_empty() { def.id.as_str() } else { s.reference.as_str() };
        let role = def.sim.as_ref().map(|r| r.role);
        let mut push = |code: &str, severity: Severity, value: f64, limit: f64, message: String| {
            out.push(SimFinding {
                code: code.into(),
                severity,
                reference: s.reference.clone(),
                entity_ids: vec![s.id],
                value,
                limit,
                message,
            });
        };
        // ヒューズ: 役割 fuse か同梱の fuse。定格は rating_a、無ければ値の文字
        if role == Some(SimRole::Fuse) || (role.is_none() && def.id == "fuse") {
            let rating = opts.number(s, "rating_a").or_else(|| parse_number(&s.value));
            if let Some(rating) = rating {
                let amps = max_of(els, &|k| matches!(k, ElementKind::Bridge));
                if amps > rating + 1e-9 {
                    push("sim.fuse_rating", Severity::Warning, amps, rating,
                        format!("{label}: 電流 {amps:.2}A がヒューズ定格 {rating}A を超えています"));
                }
            }
        }
        // ピン定格: 同じ番号の貫通・導通の橋を流れる電流
        if let Some(limit) = opts.number(s, "pin_rating_a") {
            for el in els.iter().filter(|e| matches!(e.kind, ElementKind::Pin(_) | ElementKind::Bridge)) {
                let amps = amps_of(el);
                if amps > limit + 1e-9 {
                    let pin = match &el.kind {
                        ElementKind::Pin(no) => format!("ピン{no}"),
                        _ => "接点".into(),
                    };
                    push("sim.pin_rating", Severity::Warning, amps, limit,
                        format!("{label} {pin}: 電流 {amps:.2}A がピン定格 {limit}A を超えています"));
                }
            }
        }
        match role {
            Some(SimRole::Source) => {
                if let Some(limit) = opts.number(s, "i_max_a") {
                    let amps = max_of(els, &|k| matches!(k, ElementKind::Source));
                    if amps > limit + 1e-9 {
                        push("sim.source_overload", Severity::Warning, amps, limit,
                            format!("{label}: 電源の電流 {amps:.2}A が許容 {limit}A を超えています"));
                    }
                }
            }
            Some(SimRole::Converter) => {
                if let Some(limit) = opts.number(s, "i_out_a") {
                    let amps = max_of(els, &|k| matches!(k, ElementKind::ConverterOut));
                    if amps > limit + 1e-9 {
                        push("sim.converter_overload", Severity::Warning, amps, limit,
                            format!("{label}: 変換器の出力電流 {amps:.2}A が定格 {limit}A を超えています"));
                    }
                }
            }
            Some(SimRole::Load) => {
                if let (Some(v_min), Some(t)) =
                    (opts.number(s, "v_min"), deck.load_terminals.iter().find(|t| t.entity_id == s.id))
                {
                    let v = volt_of(&t.plus) - volt_of(&t.minus);
                    if v < v_min - 1e-9 {
                        push("sim.load_voltage", Severity::Warning, v, v_min,
                            format!("{label}: 端子電圧 {v:.2}V が下限 {v_min}V を下回っています"));
                    }
                }
            }
            _ => {}
        }
    }
    // 電線の許容電流 (sq の表)。表より太い電線は判定できないので、表の最大を超えたときだけ情報で知らせる
    let table_max = AMPACITY.last().map(|&(sq, amps)| (sq, amps)).unwrap_or((0.0, 0.0));
    let mut per_wire: BTreeMap<EntityId, f64> = BTreeMap::new();
    for el in &deck.wire_elements {
        let e = per_wire.entry(el.entity_id).or_insert(0.0);
        *e = e.max(amps_of(el));
    }
    for (wid, amps) in per_wire {
        let Some(Entity::Wire(w)) = sheet.entities.get(&wid) else { continue };
        if w.sq <= 0.0 {
            continue;
        }
        if w.sq > table_max.0 + 1e-9 {
            if amps > table_max.1 + 1e-9 {
                out.push(SimFinding {
                    code: "sim.wire_unchecked".into(),
                    severity: Severity::Info,
                    reference: String::new(),
                    entity_ids: vec![w.id],
                    value: amps,
                    limit: table_max.1,
                    message: format!(
                        "ワイヤ {}sq の電流 {amps:.2}A: 許容電流の表は {}sq までのため判定していません",
                        w.sq, table_max.0
                    ),
                });
            }
            continue;
        }
        let cap = ampacity(w.sq);
        if amps > cap + 1e-9 {
            out.push(SimFinding {
                code: "sim.wire_ampacity".into(),
                severity: Severity::Error,
                reference: String::new(),
                entity_ids: vec![w.id],
                value: amps,
                limit: cap,
                message: format!("ワイヤ {}sq の許容電流 {cap}A を電流 {amps:.2}A が超えています", w.sq),
            });
        }
    }
    let rank = |s: &Severity| match s {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
    };
    out.sort_by(|a, b| rank(&a.severity).cmp(&rank(&b.severity)).then(a.reference.cmp(&b.reference)));
    out
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
                length_source: Default::default(),
                part_no: None,
                net: None,
                dashed: false, weight_mm: None,
            });
            sheet.entities.insert(e.id(), e);
        }
        sheet
    }

    /// Simulation with an invalid ngspice binary fails with an explicit error (no silent fallback).
    /// 不正なngspice実行ファイルでのシミュレーションは明示的なエラーになる(暗黙のフォールバックはしない)。
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

    /// The DC operating point reports per-net voltages (0-24 V here) and per-component current and power (a 2 A lamp shows ~2 A / ~48 W).
    /// DC動作点はネットごとの電圧(この例では0〜24V)と部品ごとの電流・電力を報告する(2Aランプは約2A/約48W)。
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

    /// Declaring a switch open in the what-if analysis drops the load current to zero, and the result notes which switches were opened.
    /// what-ifでスイッチを開路指定すると負荷電流はゼロになり、結果にどのスイッチを開いたかが明記される。
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

    /// A wire between two ports is one net in the voltage list, not one per contact with the same copied voltage, because SPICE does not know ports.
    /// 2つのポートを結ぶ配線は、電圧の一覧で1ネットになる (SPICE はポートを知らないので、接点ごとに同じ電圧を写したネットにはしない)。
    #[test]
    fn a_port_wire_is_one_net_in_the_voltages() {
        let Some(exe) = crate::ngspice::find_ngspice() else {
            eprintln!("ngspice未検出のためスキップ");
            return;
        };
        use crate::block::port_fixtures::{pin_at, port_block, USB};
        use crate::cable::test_support::{insert, wire};
        let mut sheet = series_circuit("2");
        let a = insert(&mut sheet, Entity::Symbol(port_block("39", 100.0, 200.0, "right", "U3", &USB)));
        let b = insert(&mut sheet, Entity::Symbol(port_block("40", 200.0, 200.0, "left", "U1", &USB)));
        let (pa, pb) = (pin_at(&sheet, a, "U3"), pin_at(&sheet, b, "U1"));
        let w = insert(&mut sheet, Entity::Wire(wire(&[(pa.x, pa.y), (pb.x, pb.y)])));
        let r = simulate_op_with(&exe, &sheet, &sheet_symbol_defs(&sheet), &[]).unwrap();
        let on_port: Vec<&NetVoltage> = r.nets.iter().filter(|n| n.wire_ids.contains(&w)).collect();
        assert_eq!(on_port.len(), 1, "{on_port:?}");
    }

    // ---- 役割付きシンボル (ユーザーシンボルのシミュレーション) ----

    use crate::symbol::{PinDef, PinDir, Primitive, SimRole, SimSpec, TextSlot};

    /// 役割付きのテスト用シンボル。ピンは (番号, x, y)。
    fn role_def(id: &str, role: SimRole, pins: &[(&str, f64, f64)], sim_pins: &[&str]) -> SymbolDef {
        SymbolDef {
            id: id.into(),
            name: id.into(),
            name_ja: id.into(),
            category: "user".into(),
            ref_prefix: "X".into(),
            keywords: vec![],
            primitives: vec![Primitive::Rect { p1: Point::new(-1.0, -1.0), p2: Point::new(1.0, 1.0), filled: false }],
            pins: pins
                .iter()
                .map(|(n, x, y)| PinDef {
                    number: (*n).into(),
                    name: String::new(),
                    at: Point::new(*x, *y),
                    dir: PinDir::default(),
                    contacts: vec![],
                })
                .collect(),
            text_slots: vec![TextSlot { key: crate::symbol::SLOT_TAG.into(), at: Point::new(0.0, -3.0), height: 2.5 }],
            free_pins: true,
            net: None,
            sim: Some(SimSpec { role, pins: sim_pins.iter().map(|p| (*p).into()).collect() }),
        }
    }

    /// テスト用のシンボル定義一式: 電源・負荷・ヒューズ・変換器・貫通コネクタ (役割なし)。
    fn role_defs() -> Vec<SymbolDef> {
        vec![
            role_def("t_src", SimRole::Source, &[("+", 0.0, 0.0), ("-", 0.0, 10.0)], &["+", "-"]),
            role_def("t_load", SimRole::Load, &[("+", 0.0, 0.0), ("-", 0.0, 10.0)], &["+", "-"]),
            role_def("t_fuse", SimRole::Fuse, &[("1", 0.0, 0.0), ("2", 10.0, 0.0)], &[]),
            role_def(
                "t_conv",
                SimRole::Converter,
                &[("i+", 0.0, 0.0), ("i-", 0.0, 10.0), ("o+", 20.0, 0.0), ("o-", 20.0, 10.0)],
                &["i+", "i-", "o+", "o-"],
            ),
            role_def("t_cn", SimRole::None, &[("1", 0.0, 0.0), ("1", 10.0, 0.0)], &[]),
        ]
    }

    fn place(sheet: &mut Sheet, symbol_id: &str, reference: &str, x: f64, y: f64, attrs: &[(&str, &str)]) -> EntityId {
        let e = Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: symbol_id.into(),
            at: Point::new(x, y),
            rotation: 0,
            mirror: false,
            reference: reference.into(),
            value: String::new(),
            attrs: attrs.iter().map(|(k, v)| ((*k).into(), (*v).into())).collect(),
        });
        let id = e.id();
        sheet.entities.insert(id, e);
        id
    }

    fn add_wire(sheet: &mut Sheet, pts: &[(f64, f64)], sq: f64, length_m: Option<f64>) -> EntityId {
        let e = Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: pts.iter().map(|(x, y)| Point::new(*x, *y)).collect(),
            color: String::new(),
            sq,
            length_m,
            length_source: Default::default(),
            part_no: None,
            net: None,
            dashed: false,
            weight_mm: None,
        });
        let id = e.id();
        sheet.entities.insert(id, e);
        id
    }

    /// 電源 BT1 → ヒューズ F1 → 負荷 LD1 の直列回路 (電線は 0.75sq、長さなし)。
    fn role_series(src: &[(&str, &str)], fuse: &[(&str, &str)], load: &[(&str, &str)]) -> Sheet {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        place(&mut sheet, "t_src", "BT1", 0.0, 100.0, src);
        place(&mut sheet, "t_fuse", "F1", 20.0, 100.0, fuse);
        place(&mut sheet, "t_load", "LD1", 50.0, 100.0, load);
        add_wire(&mut sheet, &[(0.0, 100.0), (20.0, 100.0)], 0.75, None);
        add_wire(&mut sheet, &[(30.0, 100.0), (50.0, 100.0)], 0.75, None);
        add_wire(&mut sheet, &[(50.0, 110.0), (0.0, 110.0)], 0.75, None);
        sheet
    }

    fn run(sheet: &Sheet, opts: &SimOptions) -> Option<SimOpResult> {
        let exe = crate::ngspice::find_ngspice()?;
        Some(simulate_op_opts_with(&exe, sheet, &role_defs(), opts).unwrap())
    }

    fn amps(r: &SimOpResult, reference: &str) -> f64 {
        r.components.iter().find(|c| c.reference == reference).map(|c| c.amps).unwrap_or(0.0)
    }

    /// A user symbol marked as a source drives a user symbol marked as a load: the load draws exactly its current_a (2 A) and the source and fuse carry the same current.
    /// 電源の役割を付けたユーザーシンボルが、負荷の役割を付けたユーザーシンボルに電流を流す: 負荷は属性 current_a どおり 2A を流し、電源とヒューズも同じ電流になる。
    #[test]
    fn a_user_source_feeds_a_user_load_at_its_current() {
        let sheet = role_series(&[("voltage", "24")], &[("rating_a", "10")], &[("current_a", "2")]);
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        assert!((r.voltage - 24.0).abs() < 1e-9);
        for reference in ["LD1", "F1", "BT1"] {
            assert!((amps(&r, reference) - 2.0).abs() < 0.01, "{reference}: {:?}", r.components);
        }
        let ld = r.components.iter().find(|c| c.reference == "LD1").unwrap();
        assert!((ld.watts - 48.0).abs() < 0.5, "{ld:?}");
        assert!(r.findings.is_empty(), "{:?}", r.findings);
    }

    /// A load without current_a only marks its presence (a tiny placeholder current), as before.
    /// current_a の無い負荷は、これまでどおり存在を示すだけ (ごく小さい電流) になる。
    #[test]
    fn a_user_load_without_current_draws_almost_nothing() {
        let sheet = role_series(&[("voltage", "24")], &[], &[]);
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        assert!(amps(&r, "LD1") < 1e-3, "{:?}", r.components);
    }

    /// A current above the fuse's rating_a is reported as a fuse finding with the measured current and the rating.
    /// ヒューズの属性 rating_a を超える電流は、測った電流と定格を添えたヒューズの指摘になる。
    #[test]
    fn a_current_above_the_fuse_rating_is_a_finding() {
        let sheet = role_series(&[("voltage", "24")], &[("rating_a", "5")], &[("current_a", "8")]);
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        let f = r.findings.iter().find(|f| f.code == "sim.fuse_rating").expect("fuse finding");
        assert_eq!(f.reference, "F1");
        assert!((f.value - 8.0).abs() < 0.01 && (f.limit - 5.0).abs() < 1e-9, "{f:?}");
    }

    /// A fuse without rating_a takes its rating from the value text (for example "5A").
    /// rating_a の無いヒューズは、値の文字 (例 "5A") を定格として使う。
    #[test]
    fn a_fuse_without_rating_uses_its_value_text() {
        let mut sheet = role_series(&[("voltage", "24")], &[], &[("current_a", "8")]);
        for e in sheet.entities.values_mut() {
            if let Entity::Symbol(s) = e {
                if s.reference == "F1" {
                    s.value = "5A".into();
                }
            }
        }
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        assert!(r.findings.iter().any(|f| f.code == "sim.fuse_rating" && (f.limit - 5.0).abs() < 1e-9), "{:?}", r.findings);
    }

    /// An override replaces a part's attribute for one run only (here a stalled load drawing 9 A), the result names the override, and the fuse finding follows the new current.
    /// 置き換えは部品の属性をその1回だけ差し替える (ここでは拘束で 9A 流れる負荷)。結果に置き換えが書かれ、ヒューズの指摘は新しい電流で出る。
    #[test]
    fn an_override_changes_a_load_for_one_run() {
        let sheet = role_series(&[("voltage", "24")], &[("rating_a", "5")], &[("current_a", "2")]);
        let mut opts = SimOptions::default();
        opts.overrides.insert("LD1".into(), [("current_a".to_string(), "9".to_string())].into_iter().collect());
        let Some(r) = run(&sheet, &opts) else { return };
        assert!((amps(&r, "LD1") - 9.0).abs() < 0.01, "{:?}", r.components);
        assert!(r.warnings.iter().any(|w| w.contains("LD1") && w.contains("current_a=9")), "{:?}", r.warnings);
        assert!(r.findings.iter().any(|f| f.code == "sim.fuse_rating"));
        // 図面そのものは変わらない
        let Some(again) = run(&sheet, &SimOptions::default()) else { return };
        assert!((amps(&again, "LD1") - 2.0).abs() < 0.01);
    }

    /// An override for a reference that is not on the sheet is reported as a warning instead of being silently ignored.
    /// 図面に無い参照記号への置き換えは、黙って無視せず注意として知らせる。
    #[test]
    fn an_override_for_an_unknown_reference_is_a_warning() {
        let sheet = role_series(&[("voltage", "24")], &[], &[("current_a", "2")]);
        let mut opts = SimOptions::default();
        opts.overrides.insert("M99".into(), [("current_a".to_string(), "1".to_string())].into_iter().collect());
        let Some(r) = run(&sheet, &opts) else { return };
        assert!(r.warnings.iter().any(|w| w.contains("M99") && w.contains("ありません")), "{:?}", r.warnings);
    }

    /// Shorting the load's supply net to ground makes the source's internal resistance limit the current, so the fuse sees the fault current; an unknown net name is a warning.
    /// 負荷の電源側のネットを接地へ短絡させると、電流は電源の内部抵抗で決まり、ヒューズは事故電流を見る。図面に無いネット名は注意になる。
    #[test]
    fn a_short_to_ground_shows_the_fault_current() {
        let sheet = role_series(&[("voltage", "24"), ("r_int_ohm", "0.1")], &[("rating_a", "30")], &[("current_a", "2")]);
        let nets = crate::netlist::extract_netlist(&sheet, &role_defs());
        // F1 の出口と LD1 の + をつなぐネット
        let net = nets
            .iter()
            .find(|n| n.pins.iter().any(|p| p.reference == "LD1" && p.pin == "+"))
            .unwrap()
            .name
            .clone();
        let opts = SimOptions { shorts: vec![net.clone(), "NOPE".into()], ..Default::default() };
        let Some(r) = run(&sheet, &opts) else { return };
        // 24V / (0.1Ω + 配線・橋の数mΩ) ≈ 200A 強
        let f1 = amps(&r, "F1");
        assert!(f1 > 200.0 && f1 < 240.0, "{f1}");
        assert!(r.findings.iter().any(|f| f.code == "sim.fuse_rating" && f.reference == "F1"));
        assert!(r.warnings.iter().any(|w| w.contains(&net)), "{:?}", r.warnings);
        assert!(r.warnings.iter().any(|w| w.contains("NOPE") && w.contains("ありません")), "{:?}", r.warnings);
    }

    /// A source carrying more than its i_max_a is reported as a source overload.
    /// 電源の属性 i_max_a を超える電流は、電源の過負荷として指摘される。
    #[test]
    fn a_source_above_its_maximum_current_is_a_finding() {
        let sheet = role_series(&[("voltage", "24"), ("i_max_a", "3")], &[], &[("current_a", "4")]);
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        let f = r.findings.iter().find(|f| f.code == "sim.source_overload").expect("source finding");
        assert_eq!(f.reference, "BT1");
        assert!((f.value - 4.0).abs() < 0.01, "{f:?}");
    }

    /// A load whose terminal voltage falls below its v_min because of long thin wires is reported with the voltage it actually gets.
    /// 長く細い電線のせいで端子電圧が属性 v_min を下回る負荷は、実際に受ける電圧を添えて指摘される。
    #[test]
    fn a_load_below_its_minimum_voltage_is_a_finding() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        place(&mut sheet, "t_src", "BT1", 0.0, 100.0, &[("voltage", "12")]);
        place(&mut sheet, "t_load", "LD1", 50.0, 100.0, &[("current_a", "5"), ("v_min", "11")]);
        // 0.3sq × 20m の往復: 1.17Ω × 2 → 5A で約 11.7V 落ちる
        add_wire(&mut sheet, &[(0.0, 100.0), (50.0, 100.0)], 0.3, Some(20.0));
        add_wire(&mut sheet, &[(50.0, 110.0), (0.0, 110.0)], 0.3, Some(20.0));
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        let f = r.findings.iter().find(|f| f.code == "sim.load_voltage").expect("voltage finding");
        assert!(f.value < 11.0 && (f.limit - 11.0).abs() < 1e-9, "{f:?}");
    }

    /// A converter makes a second supply voltage: its output holds v_out for the load, and its input draws output power divided by efficiency and input voltage.
    /// 変換器は2つ目の電源電圧を作る: 出力は負荷に対して v_out を保ち、入力は「出力電力 ÷ (効率 × 入力電圧)」の電流を引く。
    #[test]
    fn a_converter_feeds_a_second_voltage_and_loads_its_input() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        place(&mut sheet, "t_src", "BT1", 0.0, 100.0, &[("voltage", "24")]);
        place(&mut sheet, "t_conv", "U1", 20.0, 100.0, &[("v_out", "12"), ("eta", "0.8"), ("i_out_a", "3")]);
        place(&mut sheet, "t_load", "LD1", 60.0, 100.0, &[("current_a", "4")]);
        add_wire(&mut sheet, &[(0.0, 100.0), (20.0, 100.0)], 0.75, None);
        add_wire(&mut sheet, &[(0.0, 110.0), (20.0, 110.0)], 0.75, None);
        add_wire(&mut sheet, &[(40.0, 100.0), (60.0, 100.0)], 0.75, None);
        add_wire(&mut sheet, &[(40.0, 110.0), (60.0, 110.0)], 0.75, None);
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        // 12V × 4A / (0.8 × 24V) = 2.5A
        assert!((amps(&r, "BT1") - 2.5).abs() < 0.02, "{:?}", r.components);
        let ld = r.components.iter().find(|c| c.reference == "LD1").unwrap();
        assert!((ld.watts - 48.0).abs() < 0.5, "{ld:?}");
        // 出力 4A は定格 3A を超える
        let f = r.findings.iter().find(|f| f.code == "sim.converter_overload").expect("converter finding");
        assert!((f.value - 4.0).abs() < 0.01 && (f.limit - 3.0).abs() < 1e-9, "{f:?}");
    }

    /// A connector symbol with no role still passes current through its same-numbered pins, and a pin current above pin_rating_a is reported with the pin number.
    /// 役割「なし」のコネクタでも同じ番号のピンは電流を通し、属性 pin_rating_a を超えるピン電流はピン番号付きで指摘される。
    #[test]
    fn a_connector_pin_above_its_rating_is_a_finding() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        place(&mut sheet, "t_src", "BT1", 0.0, 100.0, &[("voltage", "24")]);
        place(&mut sheet, "t_cn", "CN1", 20.0, 100.0, &[("pin_rating_a", "3")]);
        place(&mut sheet, "t_load", "LD1", 50.0, 100.0, &[("current_a", "5")]);
        add_wire(&mut sheet, &[(0.0, 100.0), (20.0, 100.0)], 0.75, None);
        add_wire(&mut sheet, &[(30.0, 100.0), (50.0, 100.0)], 0.75, None);
        add_wire(&mut sheet, &[(50.0, 110.0), (0.0, 110.0)], 0.75, None);
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        assert!((amps(&r, "LD1") - 5.0).abs() < 0.01, "{:?}", r.components);
        let f = r.findings.iter().find(|f| f.code == "sim.pin_rating").expect("pin finding");
        assert!(f.reference == "CN1" && f.message.contains("ピン1"), "{f:?}");
    }

    /// A wire carrying more than the ampacity of its size is an error; a wire thicker than the ampacity table is not judged and is only noted when its current exceeds the table.
    /// 太さの許容電流を超えて流れる電線はエラーになる。許容電流の表より太い電線は判定せず、電流が表の最大を超えたときだけ情報で知らせる。
    #[test]
    fn wire_currents_are_checked_against_their_size() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        place(&mut sheet, "t_src", "BT1", 0.0, 100.0, &[("voltage", "24")]);
        place(&mut sheet, "t_load", "LD1", 50.0, 100.0, &[("current_a", "40")]);
        let thin = add_wire(&mut sheet, &[(0.0, 100.0), (50.0, 100.0)], 2.0, None);
        let thick = add_wire(&mut sheet, &[(50.0, 110.0), (0.0, 110.0)], 8.0, None);
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        let e = r.findings.iter().find(|f| f.code == "sim.wire_ampacity").expect("ampacity finding");
        assert_eq!(e.entity_ids, vec![thin]);
        assert_eq!(e.severity, Severity::Error);
        let i = r.findings.iter().find(|f| f.code == "sim.wire_unchecked").expect("unchecked note");
        assert_eq!(i.entity_ids, vec![thick]);
        assert_eq!(i.severity, Severity::Info);
        // 重い順に並ぶ
        assert_eq!(r.findings[0].severity, Severity::Error);
    }

    /// Simulation options can be sent with any field left out; an empty object means "the drawing as it is".
    /// シミュレーションの条件はどの項目も省略して送れる。空のオブジェクトは「図面のまま」を意味する。
    #[test]
    fn simulation_options_default_to_the_drawing_as_it_is() {
        let o: SimOptions = serde_json::from_str("{}").unwrap();
        assert!(o.open_switches.is_empty() && o.overrides.is_empty() && o.shorts.is_empty());
        let o: SimOptions = serde_json::from_str(r#"{"overrides":{"M1":{"current_a":"40"}},"shorts":["P24"]}"#).unwrap();
        assert_eq!(o.overrides["M1"]["current_a"], "40");
        assert_eq!(o.shorts, vec!["P24".to_string()]);
    }

    /// 変換器 U1 (24V → 12V) と、出力側の負荷 LD1。出力の帰路は入力側とつながっていない (絶縁型)。
    fn converter_sheet(conv_attrs: &[(&str, &str)]) -> Sheet {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        place(&mut sheet, "t_src", "BT1", 0.0, 100.0, &[("voltage", "24")]);
        place(&mut sheet, "t_conv", "U1", 20.0, 100.0, conv_attrs);
        place(&mut sheet, "t_load", "LD1", 60.0, 100.0, &[("current_a", "4")]);
        add_wire(&mut sheet, &[(0.0, 100.0), (20.0, 100.0)], 0.75, None);
        add_wire(&mut sheet, &[(0.0, 110.0), (20.0, 110.0)], 0.75, None);
        add_wire(&mut sheet, &[(40.0, 100.0), (60.0, 100.0)], 0.75, None);
        add_wire(&mut sheet, &[(40.0, 110.0), (60.0, 110.0)], 0.75, None);
        sheet
    }

    /// An isolated converter output (its return not tied to the input side) is referenced to ground at its minus, so its nets read 0 V and 12 V instead of floating values, and the result says so.
    /// 絶縁された変換器の出力 (帰路が入力側とつながっていない) は負極を接地の基準にするので、ネットは浮いた値でなく 0V と 12V になり、結果にもそう書かれる。
    #[test]
    fn an_isolated_converter_output_is_referenced_to_ground() {
        let sheet = converter_sheet(&[("v_out", "12")]);
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        let mut volts: Vec<f64> = r.nets.iter().map(|n| n.volts_max).collect();
        volts.sort_by(f64::total_cmp);
        assert!(volts.iter().all(|v| *v > -0.01), "{volts:?}");
        assert!(volts.iter().any(|v| (v - 12.0).abs() < 0.05), "{volts:?}");
        assert!(r.warnings.iter().any(|w| w.contains("U1") && w.contains("接地")), "{:?}", r.warnings);
        // 出力の短絡は事故電流になる (帰路が接地の基準を通って閉じる)
        let nets = crate::netlist::extract_netlist(&sheet, &role_defs());
        let out = nets.iter().find(|n| n.pins.iter().any(|p| p.reference == "LD1" && p.pin == "+")).unwrap().name.clone();
        let Some(r) = run(&sheet, &SimOptions { shorts: vec![out], ..Default::default() }) else { return };
        assert!(amps(&r, "U1") > 1000.0, "{:?}", r.components);
    }

    /// A converter without eta uses 90 % efficiency, and an eta outside 0 to 1 also falls back to 90 % with a warning.
    /// eta の無い変換器は効率 90% で計算し、0〜1 の外の eta も注意を出して 90% にする。
    #[test]
    fn a_converter_without_a_valid_efficiency_uses_ninety_percent() {
        // 12V × 4A / (0.9 × 24V) = 2.22A
        let Some(r) = run(&converter_sheet(&[("v_out", "12")]), &SimOptions::default()) else { return };
        assert!((amps(&r, "BT1") - 2.222).abs() < 0.02, "{:?}", r.components);
        let Some(r) = run(&converter_sheet(&[("v_out", "12"), ("eta", "85")]), &SimOptions::default()) else { return };
        assert!((amps(&r, "BT1") - 2.222).abs() < 0.02, "{:?}", r.components);
        assert!(r.warnings.iter().any(|w| w.contains("eta=85")), "{:?}", r.warnings);
    }

    /// Opening a part with the fuse role (a blown fuse or an open breaker) cuts the load current.
    /// ヒューズの役割の部品を開路にすると (切れたヒューズ・切ったブレーカー)、負荷の電流は止まる。
    #[test]
    fn opening_a_fuse_role_part_cuts_the_current() {
        let sheet = role_series(&[("voltage", "24")], &[("rating_a", "10")], &[("current_a", "2")]);
        let Some(r) = run(&sheet, &SimOptions { open_switches: vec!["F1".into()], ..Default::default() }) else { return };
        assert!(amps(&r, "BT1") < 1e-3, "{:?}", r.components);
        assert!(amps(&r, "LD1") < 1e-3, "{:?}", r.components);
        // 経路が切れても電圧は暴れない (負荷は端子電圧が無くなると電流も止まる)
        assert!(r.nets.iter().all(|n| n.volts_min > -1.0 && n.volts_max < 25.0), "{:?}", r.nets);
    }

    /// An override reaches every part that shares the reference (for example a relay coil and its contact drawn apart).
    /// 置き換えは、同じ参照記号を持つ全ての部品に効く (例: 離して描いたリレーのコイルと接点)。
    #[test]
    fn an_override_reaches_every_part_with_the_reference() {
        let mut sheet = role_series(&[("voltage", "24")], &[], &[("current_a", "1")]);
        place(&mut sheet, "t_load", "LD1", 80.0, 100.0, &[("current_a", "1")]);
        add_wire(&mut sheet, &[(50.0, 100.0), (80.0, 100.0)], 0.75, None);
        add_wire(&mut sheet, &[(80.0, 110.0), (50.0, 110.0)], 0.75, None);
        let mut opts = SimOptions::default();
        opts.overrides.insert("LD1".into(), [("current_a".to_string(), "3".to_string())].into_iter().collect());
        let Some(r) = run(&sheet, &opts) else { return };
        assert!((amps(&r, "BT1") - 6.0).abs() < 0.02, "{:?}", r.components);
    }

    /// The built-in battery's voltage can be overridden too (attribute voltage), for example to run at a low state of charge.
    /// 同梱の battery の電圧も置き換えられる (属性 voltage)。例: 残量が少ないときの電圧で計算する。
    #[test]
    fn the_built_in_battery_voltage_can_be_overridden() {
        let Some(exe) = crate::ngspice::find_ngspice() else { return };
        let sheet = series_circuit("2");
        let mut opts = SimOptions::default();
        opts.overrides.insert("BT1".into(), [("voltage".to_string(), "20".to_string())].into_iter().collect());
        let r = simulate_op_opts_with(&exe, &sheet, &sheet_symbol_defs(&sheet), &opts).unwrap();
        assert!((r.voltage - 20.0).abs() < 1e-9);
        let vmax = r.nets.iter().map(|n| n.volts_max).fold(0.0, f64::max);
        assert!((vmax - 20.0).abs() < 0.05, "{vmax}");
    }

    /// A part whose role names a pin the placed symbol does not have is left out with a warning instead of vanishing silently.
    /// 役割が指すピンを持たない部品は、黙って消さず、注意を出して計算から除く。
    #[test]
    fn a_role_pin_that_is_missing_is_a_warning() {
        let mut defs = role_defs();
        defs.push(role_def("t_bad", SimRole::Load, &[("1", 0.0, 0.0), ("2", 0.0, 10.0)], &["1", "9"]));
        let mut sheet = role_series(&[("voltage", "24")], &[], &[("current_a", "2")]);
        place(&mut sheet, "t_bad", "LD9", 90.0, 100.0, &[("current_a", "5")]);
        let Some(exe) = crate::ngspice::find_ngspice() else { return };
        let r = simulate_op_opts_with(&exe, &sheet, &defs, &SimOptions::default()).unwrap();
        assert!(r.warnings.iter().any(|w| w.contains("LD9") && w.contains("ピン9")), "{:?}", r.warnings);
        assert!((amps(&r, "BT1") - 2.0).abs() < 0.02);
    }

    /// A converter with i_limit_a holds its output current at the limit when its output is shorted, instead of an ideal source driving an endless current; below the limit it behaves as before.
    /// i_limit_a を持つ変換器は、出力を短絡すると出力電流が上限で止まる (理想電源のように際限なく流さない)。上限より下ではこれまでどおり動く。
    #[test]
    fn a_current_limited_converter_stops_at_its_limit() {
        let sheet = converter_sheet(&[("v_out", "12"), ("eta", "0.8"), ("i_limit_a", "6")]);
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        assert!((amps(&r, "LD1") - 4.0).abs() < 0.02, "{:?}", r.components);
        assert!((amps(&r, "BT1") - 2.5).abs() < 0.05, "{:?}", r.components);
        let nets = crate::netlist::extract_netlist(&sheet, &role_defs());
        let out = nets.iter().find(|n| n.pins.iter().any(|p| p.reference == "LD1" && p.pin == "+")).unwrap().name.clone();
        let Some(r) = run(&sheet, &SimOptions { shorts: vec![out], ..Default::default() }) else { return };
        // 上限 6A をわずかに超えた所で止まる (超えた分だけ出力電圧を急に下げる特性)
        let u1 = amps(&r, "U1");
        assert!((6.0..6.3).contains(&u1), "{:?}", r.components);
    }

    /// A converter with v_in_min stops (no output, no input current) when its input voltage is below that value, like a real converter's undervoltage lockout.
    /// v_in_min を持つ変換器は、入力電圧がその値より低いと止まる (出力も入力電流も無い)。実際の変換器の低電圧停止と同じ。
    #[test]
    fn a_converter_below_its_minimum_input_voltage_stops() {
        let mut sheet = converter_sheet(&[("v_out", "12"), ("v_in_min", "18")]);
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        assert!((amps(&r, "LD1") - 4.0).abs() < 0.05, "24V では動く: {:?}", r.components);
        for e in sheet.entities.values_mut() {
            if let Entity::Symbol(s) = e {
                if s.reference == "BT1" {
                    s.attrs.insert("voltage".into(), "15".into());
                }
            }
        }
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        assert!(amps(&r, "BT1") < 0.01 && amps(&r, "LD1") < 0.01, "{:?}", r.components);
    }

    fn load_of<'a>(r: &'a SimOpResult, id: EntityId) -> &'a EntityLoad {
        r.loads.iter().find(|l| l.entity_id == id).expect("load entry")
    }

    fn id_of(sheet: &Sheet, reference: &str) -> EntityId {
        sheet
            .entities
            .values()
            .find_map(|e| match e {
                Entity::Symbol(s) if s.reference == reference => Some(s.id),
                _ => None,
            })
            .unwrap()
    }

    /// Each wire reports its current, the voltage at both ends and, from its size, the ampacity limit and the load ratio.
    /// 電線ごとに、電流・両端の電圧・太さから決まる許容電流と負荷率が出る。
    #[test]
    fn a_wire_load_has_current_end_voltages_and_ampacity() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        place(&mut sheet, "t_src", "BT1", 0.0, 100.0, &[("voltage", "24")]);
        place(&mut sheet, "t_load", "LD1", 50.0, 100.0, &[("current_a", "6")]);
        let w = add_wire(&mut sheet, &[(0.0, 100.0), (50.0, 100.0)], 0.75, Some(10.0));
        add_wire(&mut sheet, &[(50.0, 110.0), (0.0, 110.0)], 0.75, None);
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        let l = load_of(&r, w);
        assert_eq!(l.kind, LoadKind::Wire);
        assert!((l.amps - 6.0).abs() < 0.01, "{l:?}");
        assert_eq!(l.volts.len(), 2);
        // 0.0175 × 10m / 0.75sq = 0.233Ω → 6A で 1.4V 落ちる
        assert!((l.volts[0] - l.volts[1] - 1.4).abs() < 0.05, "{l:?}");
        assert_eq!(l.limit, Some(12.0));
        assert_eq!(l.limit_kind, Some(LimitKind::WireAmpacity));
        assert!((l.ratio.unwrap() - 0.5).abs() < 0.01);
    }

    /// A wire without a size, or thicker than the ampacity table, still reports its current but has no limit and no ratio.
    /// 太さの無い電線と、許容電流の表より太い電線も電流は出るが、限度と負荷率は無い。
    #[test]
    fn a_wire_without_a_known_ampacity_has_no_limit() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        place(&mut sheet, "t_src", "BT1", 0.0, 100.0, &[("voltage", "24")]);
        place(&mut sheet, "t_load", "LD1", 50.0, 100.0, &[("current_a", "6")]);
        let bare = add_wire(&mut sheet, &[(0.0, 100.0), (50.0, 100.0)], 0.0, None);
        let thick = add_wire(&mut sheet, &[(50.0, 110.0), (0.0, 110.0)], 8.0, None);
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        for id in [bare, thick] {
            let l = load_of(&r, id);
            assert!((l.amps - 6.0).abs() < 0.01 && l.limit.is_none() && l.ratio.is_none(), "{l:?}");
        }
    }

    /// A fuse reports its rating as the limit, and the voltage at each of its pins in pin order.
    /// ヒューズは定格を限度として出し、ピンごとの電圧をピンの順に出す。
    #[test]
    fn a_fuse_load_uses_its_rating() {
        let sheet = role_series(&[("voltage", "24")], &[("rating_a", "5")], &[("current_a", "8")]);
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        let l = load_of(&r, id_of(&sheet, "F1"));
        assert_eq!(l.kind, LoadKind::Component);
        assert_eq!((l.limit, l.limit_kind), (Some(5.0), Some(LimitKind::FuseRating)));
        assert!((l.ratio.unwrap() - 1.6).abs() < 0.01);
        assert_eq!(l.volts.len(), 2);
    }

    /// A connector without a role still gets a load entry, limited by its pin rating, because current flows through its same-numbered pins.
    /// 役割の無いコネクタも、同じ番号のピンを電流が通るので負荷の行が出て、ピン定格が限度になる。
    #[test]
    fn a_connector_load_uses_its_pin_rating() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        place(&mut sheet, "t_src", "BT1", 0.0, 100.0, &[("voltage", "24")]);
        place(&mut sheet, "t_cn", "CN1", 20.0, 100.0, &[("pin_rating_a", "10")]);
        place(&mut sheet, "t_load", "LD1", 50.0, 100.0, &[("current_a", "4")]);
        add_wire(&mut sheet, &[(0.0, 100.0), (20.0, 100.0)], 0.75, None);
        add_wire(&mut sheet, &[(30.0, 100.0), (50.0, 100.0)], 0.75, None);
        add_wire(&mut sheet, &[(50.0, 110.0), (0.0, 110.0)], 0.75, None);
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        let l = load_of(&r, id_of(&sheet, "CN1"));
        assert_eq!((l.limit, l.limit_kind), (Some(10.0), Some(LimitKind::PinRating)));
        assert!((l.ratio.unwrap() - 0.4).abs() < 0.01);
    }

    /// A source reports its maximum current as the limit and a converter its rated output current.
    /// 電源は許容電流を、変換器は定格の出力電流を限度として出す。
    #[test]
    fn source_and_converter_loads_use_their_ratings() {
        let mut sheet = converter_sheet(&[("v_out", "12"), ("eta", "0.8"), ("i_out_a", "5")]);
        for e in sheet.entities.values_mut() {
            if let Entity::Symbol(s) = e {
                if s.reference == "BT1" {
                    s.attrs.insert("i_max_a".into(), "2".into());
                }
            }
        }
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        let bt = load_of(&r, id_of(&sheet, "BT1"));
        assert_eq!((bt.limit, bt.limit_kind), (Some(2.0), Some(LimitKind::SourceMax)));
        assert!((bt.ratio.unwrap() - 1.25).abs() < 0.02, "{bt:?}");
        let u1 = load_of(&r, id_of(&sheet, "U1"));
        assert_eq!((u1.limit, u1.limit_kind), (Some(5.0), Some(LimitKind::ConverterOut)));
        assert!((u1.ratio.unwrap() - 0.8).abs() < 0.01, "{u1:?}");
    }

    /// A part with several limits (a fuse that also has a pin rating) reports the one it is closest to exceeding.
    /// 限度を複数持つ部品 (ピン定格もあるヒューズ) は、一番超えそうな限度を出す。
    #[test]
    fn the_highest_ratio_limit_wins() {
        let sheet = role_series(&[("voltage", "24")], &[("rating_a", "10"), ("pin_rating_a", "5")], &[("current_a", "4")]);
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        let l = load_of(&r, id_of(&sheet, "F1"));
        assert_eq!((l.limit, l.limit_kind), (Some(5.0), Some(LimitKind::PinRating)));
    }

    /// Exactly the parts whose ratio exceeds 1 have a fuse, pin, source or converter finding.
    /// 負荷率が 1 を超えた部品と、ヒューズ・ピン・電源・変換器の指摘が出る部品は一致する。
    #[test]
    fn loads_over_one_match_the_findings() {
        let sheet = role_series(&[("voltage", "24"), ("i_max_a", "3")], &[("rating_a", "5")], &[("current_a", "8")]);
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        let mut over: Vec<EntityId> =
            r.loads.iter().filter(|l| l.kind == LoadKind::Component && l.ratio.unwrap_or(0.0) > 1.0).map(|l| l.entity_id).collect();
        let mut found: Vec<EntityId> = r
            .findings
            .iter()
            .filter(|f| !f.code.starts_with("sim.wire") && f.code != "sim.load_voltage")
            .flat_map(|f| f.entity_ids.clone())
            .collect();
        over.sort();
        over.dedup();
        found.sort();
        found.dedup();
        assert_eq!(over, found);
    }

    use crate::model::{FaultTarget, SimCase, SimFault};

    fn first_wire_into(sheet: &Sheet, reference: &str, pin: &str) -> EntityId {
        let nets = crate::netlist::extract_netlist(sheet, &role_defs());
        let net = nets.iter().find(|n| n.pins.iter().any(|p| p.reference == reference && p.pin == pin)).unwrap();
        net.wire_ids[0]
    }

    /// A short chosen on a wire joins that wire to ground, so the fuse in front of it carries the fault current.
    /// 電線で指定した短絡はその電線を接地へつなぎ、手前のヒューズに事故電流が流れる。
    #[test]
    fn a_short_on_a_wire_gives_the_fault_current() {
        let sheet = role_series(&[("voltage", "24"), ("r_int_ohm", "0.1")], &[("rating_a", "30")], &[("current_a", "2")]);
        let w = first_wire_into(&sheet, "LD1", "+");
        let opts = SimOptions { faults: vec![SimFault::Short { target: FaultTarget::Wire { wire_id: w } }], ..Default::default() };
        let Some(r) = run(&sheet, &opts) else { return };
        assert!(amps(&r, "F1") > 200.0, "{:?}", r.components);
    }

    /// A short chosen on a part's pin joins that pin to ground the same way.
    /// 部品のピンで指定した短絡も、同じようにそのピンを接地へつなぐ。
    #[test]
    fn a_short_on_a_pin_gives_the_fault_current() {
        let sheet = role_series(&[("voltage", "24"), ("r_int_ohm", "0.1")], &[("rating_a", "30")], &[("current_a", "2")]);
        let target = FaultTarget::Pin { entity_id: id_of(&sheet, "F1"), pin: "2".into() };
        let opts = SimOptions { faults: vec![SimFault::Short { target }], ..Default::default() };
        let Some(r) = run(&sheet, &opts) else { return };
        assert!(amps(&r, "F1") > 200.0, "{:?}", r.components);
    }

    /// A break chosen on a wire removes it from the circuit, so the load behind it gets no current.
    /// 電線で指定した断線はその電線を回路から外し、その先の負荷には電流が流れない。
    #[test]
    fn an_open_wire_stops_the_current() {
        let sheet = role_series(&[("voltage", "24")], &[("rating_a", "30")], &[("current_a", "2")]);
        let w = first_wire_into(&sheet, "LD1", "+");
        let opts = SimOptions { faults: vec![SimFault::Open { wire_id: w }], ..Default::default() };
        let Some(r) = run(&sheet, &opts) else { return };
        assert!(amps(&r, "BT1") < 1e-3, "{:?}", r.components);
    }

    /// A fault whose wire, part or pin no longer exists is skipped with a warning and the run still succeeds.
    /// 電線・部品・ピンが無くなった故障は、注意を出して飛ばし、計算はそのまま成功する。
    #[test]
    fn faults_on_missing_targets_are_skipped_with_warnings() {
        let sheet = role_series(&[("voltage", "24")], &[], &[("current_a", "2")]);
        let gone = Uuid::new_v4();
        let opts = SimOptions {
            faults: vec![
                SimFault::Short { target: FaultTarget::Wire { wire_id: gone } },
                SimFault::Short { target: FaultTarget::Pin { entity_id: id_of(&sheet, "F1"), pin: "9".into() } },
                SimFault::Open { wire_id: gone },
            ],
            ..Default::default()
        };
        let Some(r) = run(&sheet, &opts) else { return };
        assert!((amps(&r, "LD1") - 2.0).abs() < 0.01);
        assert_eq!(r.warnings.iter().filter(|w| w.contains("見つかりません")).count(), 3, "{:?}", r.warnings);
    }

    /// Applying a study case adds its switches, overrides and faults; values given explicitly in the same request win over the case.
    /// 計算ケースを当てると、その開路・置き換え・故障が足される。同じ依頼で直接指定した値はケースより優先する。
    #[test]
    fn a_study_case_is_applied_on_top_of_explicit_options() {
        let case = SimCase {
            id: Uuid::new_v4(),
            name: "c".into(),
            open_switches: vec!["SW1".into()],
            overrides: [("M1".to_string(), [("current_a".to_string(), "40".to_string()), ("v_min".to_string(), "20".to_string())].into_iter().collect())].into_iter().collect(),
            faults: vec![SimFault::Open { wire_id: Uuid::nil() }],
        };
        let explicit = SimOptions {
            overrides: [("M1".to_string(), [("current_a".to_string(), "31".to_string())].into_iter().collect())].into_iter().collect(),
            ..Default::default()
        };
        let o = explicit.with_case(&case);
        assert_eq!(o.open_switches, vec!["SW1".to_string()]);
        assert_eq!(o.overrides["M1"]["current_a"], "31");
        assert_eq!(o.overrides["M1"]["v_min"], "20");
        assert_eq!(o.faults.len(), 1);
    }
}
