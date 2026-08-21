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

/// シート1枚のERC+電気検証。電流・電圧はngspiceのDC動作点解析を優先し、
/// ngspice未導入(またはSPICE化不能)ならグラフ近似へフォールバックする。
pub fn verify_sheet(sheet: &Sheet, symbols: &[SymbolDef]) -> Vec<Diagnostic> {
    let sim = simulate(sheet, symbols);
    verify_sheet_with(sheet, symbols, sim.as_ref())
}

/// プロジェクト全体のERC+電気検証。シートごとの検証に加え、ネットラベル関連のチェック
/// (`erc.label_conflict`) は**シートを跨いだ統合ネット** ([`crate::xref`]) で評価する。
/// これにより、同名ラベルで繋がった複数シートのネットは1ネットとして扱われ、
/// 1件の指摘にまとまる。
pub fn verify_project(project: &crate::model::Project) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for sheet in &project.sheets {
        diags.extend(
            verify_sheet(sheet, &crate::symbol::sheet_symbol_defs(sheet))
                .into_iter()
                // ラベル競合はシート単位ではなくプロジェクト全体で評価し直す
                .filter(|d| d.code != LABEL_CONFLICT),
        );
    }
    diags.extend(project_label_conflicts(project));
    diags
}

/// 1つのネットに異なるネットラベルが混在している状態 (異電位ネットの直結) を
/// プロジェクト全体の統合ネットで探す。シートを跨いで繋がったネットも1件にまとまる。
fn project_label_conflicts(project: &crate::model::Project) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for net in crate::xref::extract_netlist_project(project).nets {
        if net.label_names.len() < 2 {
            continue;
        }
        let sheet_id = net
            .members
            .first()
            .map(|m| m.sheet_id)
            .unwrap_or_else(uuid::Uuid::nil);
        let entity_ids: Vec<EntityId> =
            net.members.iter().flat_map(|m| m.label_ids.clone()).collect();
        diags.push(Diagnostic {
            severity: Severity::Error,
            code: LABEL_CONFLICT.into(),
            message: format!(
                "1つのネットに異なるネットラベルが混在しています: {}",
                net.label_names.join(", ")
            ),
            sheet_id,
            entity_ids,
        });
    }
    diags
}

/// ネットラベル競合の診断コード。シート単位・プロジェクト全体で共通。
const LABEL_CONFLICT: &str = "erc.label_conflict";

pub(crate) fn verify_sheet_with(
    sheet: &Sheet,
    symbols: &[SymbolDef],
    sim: Option<&SimResult>,
) -> Vec<Diagnostic> {
    let mut diags = erc(sheet, symbols);
    diags.extend(electrical(sheet, symbols, sim));
    diags
}

/// ngspiceによるDC動作点の解。
pub(crate) struct SimResult {
    /// ノード名(小文字)→電圧。枝電流("v1#branch")も含む。
    pub voltages: std::collections::BTreeMap<String, f64>,
    pub deck: crate::spice::SpiceDeck,
}

impl SimResult {
    fn volt(&self, node: &str) -> f64 {
        if node == "0" {
            0.0
        } else {
            self.voltages.get(node).copied().unwrap_or(0.0)
        }
    }
}

/// SPICE化してngspiceで解く。ngspice未検出・電源なし・実行失敗はNone。
fn simulate(sheet: &Sheet, symbols: &[SymbolDef]) -> Option<SimResult> {
    let deck = crate::spice::build_deck(sheet, symbols).ok()?;
    let exe = crate::ngspice::find_ngspice()?;
    let voltages = crate::ngspice::run_op(&exe, &deck.deck).ok()?;
    Some(SimResult { voltages, deck })
}

/// 銅の抵抗率 (Ω·mm²/m)。
const COPPER_RESISTIVITY: f64 = 0.0175;
/// 電圧降下の許容率 (電源電圧比)。
const VOLTAGE_DROP_RATIO: f64 = 0.03;
/// 電源電圧が読み取れないときの既定値 (V)。
const DEFAULT_VOLTAGE: f64 = 24.0;

/// sq→許容電流(A) の第一版テーブル (AVS系の慣用値)。表にないsqは直近下位を使う。
const AMPACITY: &[(f64, f64)] = &[
    (0.3, 7.0),
    (0.5, 9.0),
    (0.75, 12.0),
    (1.25, 16.0),
    (2.0, 22.0),
    (3.5, 33.0),
];

fn ampacity(sq: f64) -> f64 {
    let mut amps = AMPACITY[0].1;
    for &(s, a) in AMPACITY {
        if sq + 1e-9 >= s {
            amps = a;
        }
    }
    amps
}

/// 文字列先頭付近の数値を読む (例: "5A"→5, "DC24V"→24)。
pub(crate) fn parse_number(s: &str) -> Option<f64> {
    let start = s.find(|c: char| c.is_ascii_digit())?;
    let rest = &s[start..];
    let end = rest
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// 導通扱いの2ピン部品 (静的検証なのでスイッチ・接点は閉として扱う)。
pub(crate) fn is_conductor(def: &SymbolDef) -> bool {
    def.category == "switch" || def.category == "protection" || def.id.contains("contact")
}

/// 負荷 (電流を消費する部品)。
pub(crate) fn is_load(def: &SymbolDef) -> bool {
    def.category == "output" || def.id == "relay_coil" || def.id == "led"
}

fn electrical(sheet: &Sheet, symbols: &[SymbolDef], sim: Option<&SimResult>) -> Vec<Diagnostic> {
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

    // (シンボルid, ピン番号) → ネット番号
    let mut pin_net: BTreeMap<(EntityId, &str), usize> = BTreeMap::new();
    for (ni, net) in nets.iter().enumerate() {
        for p in &net.pins {
            pin_net.insert((p.entity_id, p.pin.as_str()), ni);
        }
    }
    // シンボルの接続先ネット一覧 (重複除去)
    let nets_of = |s: &crate::model::SymbolInstance, def: &SymbolDef| -> Vec<usize> {
        let mut out = BTreeSet::new();
        for pin in &def.pins {
            if let Some(ni) = pin_net.get(&(s.id, pin.number.as_str())) {
                out.insert(*ni);
            }
        }
        out.into_iter().collect()
    };

    // ネットごとの負荷電流合計 + 電流不明の負荷をInfo
    let mut net_current = vec![0.0f64; nets.len()];
    let mut source_voltage = None;
    let mut has_battery = false;
    for e in sheet.entities.values() {
        let Entity::Symbol(s) = e else { continue };
        let Some(def) = defs.get(s.symbol_id.as_str()) else { continue };
        if def.id == "battery" {
            has_battery = true;
            if source_voltage.is_none() {
                source_voltage = parse_number(&s.value);
            }
        }
        if !is_load(def) {
            continue;
        }
        let label = if s.reference.is_empty() { def.id.as_str() } else { s.reference.as_str() };
        match s.attrs.get("current_a").and_then(|v| parse_number(v)) {
            Some(amps) => {
                for ni in nets_of(s, def) {
                    net_current[ni] += amps;
                }
            }
            None => diags.push(diag(
                Severity::Info,
                "elec.no_current_attr",
                format!("{label}: 属性 current_a が未設定のため電流計算から除外しました"),
                vec![s.id],
            )),
        }
    }
    let voltage = source_voltage.unwrap_or(DEFAULT_VOLTAGE);

    // 電源到達性: 電源(battery)のネットから導通部品を橋渡しにBFS
    let mut reachable = vec![false; nets.len()];
    let mut queue: Vec<usize> = Vec::new();
    for e in sheet.entities.values() {
        let Entity::Symbol(s) = e else { continue };
        let Some(def) = defs.get(s.symbol_id.as_str()) else { continue };
        if def.id == "battery" {
            for ni in nets_of(s, def) {
                if !reachable[ni] {
                    reachable[ni] = true;
                    queue.push(ni);
                }
            }
        }
    }
    // 導通部品の (ネット, ネット) 橋
    let mut bridges: Vec<Vec<usize>> = Vec::new();
    for e in sheet.entities.values() {
        let Entity::Symbol(s) = e else { continue };
        let Some(def) = defs.get(s.symbol_id.as_str()) else { continue };
        if is_conductor(def) {
            let ns = nets_of(s, def);
            if ns.len() >= 2 {
                bridges.push(ns);
            }
        }
    }
    while let Some(ni) = queue.pop() {
        for bridge in &bridges {
            if bridge.contains(&ni) {
                for &other in bridge {
                    if !reachable[other] {
                        reachable[other] = true;
                        queue.push(other);
                    }
                }
            }
        }
    }
    for e in sheet.entities.values() {
        let Entity::Symbol(s) = e else { continue };
        let Some(def) = defs.get(s.symbol_id.as_str()) else { continue };
        if !is_load(def) {
            continue;
        }
        let ns = nets_of(s, def);
        // 全く配線されていない負荷はERC(未接続ピン)に任せる
        if ns.is_empty() {
            continue;
        }
        let label = if s.reference.is_empty() { def.id.as_str() } else { s.reference.as_str() };
        let mut pin_count = BTreeSet::new();
        for pin in &def.pins {
            pin_count.insert(pin.number.as_str());
        }
        let all_pins_wired = ns.len() >= pin_count.len().min(2);
        if !all_pins_wired || ns.iter().any(|&ni| !reachable[ni]) {
            diags.push(diag(
                Severity::Warning,
                "elec.unreachable_load",
                format!("{label}: 電源から到達できません (断線または開路)"),
                vec![s.id],
            ));
        }
    }

    // ngspiceのDC動作点が得られた場合: 実解の電流・電圧で判定する
    if let Some(sim) = sim {
        // ワイヤごとの電流(分割素子の最大)と電圧降下(素子の合計)
        let mut per_wire: BTreeMap<EntityId, (f64, f64)> = BTreeMap::new();
        for el in &sim.deck.wire_elements {
            let dv = (sim.volt(&el.node_a) - sim.volt(&el.node_b)).abs();
            let amps = if el.ohms > 0.0 { dv / el.ohms } else { 0.0 };
            let e = per_wire.entry(el.entity_id).or_insert((0.0, 0.0));
            e.0 = e.0.max(amps);
            e.1 += dv;
        }
        for (wid, (amps, drop)) in &per_wire {
            let Some(Entity::Wire(w)) = sheet.entities.get(wid) else { continue };
            if w.sq > 0.0 && *amps > 1e-6 {
                let cap = ampacity(w.sq);
                if *amps > cap + 1e-9 {
                    diags.push(diag(
                        Severity::Error,
                        "elec.wire_ampacity",
                        format!(
                            "ワイヤ {}sq の許容電流 {cap}A を電流 {amps:.2}A が超えています (シミュレーション値)",
                            w.sq
                        ),
                        vec![w.id],
                    ));
                }
            }
            if w.length_m.is_some() {
                let limit = voltage * VOLTAGE_DROP_RATIO;
                if *drop > limit {
                    diags.push(diag(
                        Severity::Warning,
                        "elec.voltage_drop",
                        format!(
                            "電圧降下 {drop:.2}V が許容値 {limit:.2}V ({voltage}Vの{}%) を超えています (シミュレーション値)",
                            (VOLTAGE_DROP_RATIO * 100.0) as u32
                        ),
                        vec![w.id],
                    ));
                }
            }
        }
        // ヒューズの実電流
        let mut comp_amps: BTreeMap<EntityId, f64> = BTreeMap::new();
        for el in &sim.deck.component_elements {
            let dv = (sim.volt(&el.node_a) - sim.volt(&el.node_b)).abs();
            let amps = if el.ohms > 0.0 { dv / el.ohms } else { 0.0 };
            let e = comp_amps.entry(el.entity_id).or_insert(0.0);
            *e = e.max(amps);
        }
        for e in sheet.entities.values() {
            let Entity::Symbol(s) = e else { continue };
            let Some(def) = defs.get(s.symbol_id.as_str()) else { continue };
            if def.id != "fuse" {
                continue;
            }
            let Some(rating) = parse_number(&s.value) else { continue };
            let amps = comp_amps.get(&s.id).copied().unwrap_or(0.0);
            let label = if s.reference.is_empty() { "fuse" } else { s.reference.as_str() };
            if amps > rating + 1e-9 {
                diags.push(diag(
                    Severity::Warning,
                    "elec.fuse_rating",
                    format!(
                        "{label}: 電流 {amps:.2}A がヒューズ定格 {rating}A を超えています (シミュレーション値)"
                    ),
                    vec![s.id],
                ));
            }
        }
        return diags;
    }

    // ngspice未検出時のフォールバック: グラフ近似で判定 (Infoで明示)
    if has_battery {
        diags.push(diag(
            Severity::Info,
            "elec.approximate_mode",
            "ngspice未検出のため電流・電圧はグラフ近似で判定しました (ngspiceを導入すると実回路解析になります)".into(),
            Vec::new(),
        ));
    }

    // 直列経路の近似: 導通部品(ヒューズ・スイッチ・接点)越しに電流を伝播する。
    // 橋の両側は同じ電流が流れるとみなしmaxを採る(並列分岐では過小評価になり得る第一版の割り切り)
    let mut changed = true;
    while changed {
        changed = false;
        for bridge in &bridges {
            let m = bridge.iter().map(|&ni| net_current[ni]).fold(0.0, f64::max);
            for &ni in bridge {
                if net_current[ni] + 1e-12 < m {
                    net_current[ni] = m;
                    changed = true;
                }
            }
        }
    }

    // ワイヤの許容電流と電圧降下
    for (ni, net) in nets.iter().enumerate() {
        let amps = net_current[ni];
        if amps <= 0.0 {
            continue;
        }
        for wid in &net.wire_ids {
            let Some(Entity::Wire(w)) = sheet.entities.get(wid) else { continue };
            if w.sq > 0.0 {
                let cap = ampacity(w.sq);
                if amps > cap + 1e-9 {
                    diags.push(diag(
                        Severity::Error,
                        "elec.wire_ampacity",
                        format!(
                            "ワイヤ {}sq の許容電流 {cap}A を負荷電流 {amps}A が超えています (ネット {})",
                            w.sq, net.name
                        ),
                        vec![w.id],
                    ));
                }
                if let Some(len) = w.length_m {
                    let drop = 2.0 * COPPER_RESISTIVITY * len / w.sq * amps;
                    let limit = voltage * VOLTAGE_DROP_RATIO;
                    if drop > limit {
                        diags.push(diag(
                            Severity::Warning,
                            "elec.voltage_drop",
                            format!(
                                "電圧降下 {drop:.2}V が許容値 {limit:.2}V ({}Vの{}%) を超えています (ワイヤ {}m {}sq, {}A)",
                                voltage,
                                (VOLTAGE_DROP_RATIO * 100.0) as u32,
                                len, w.sq, amps
                            ),
                            vec![w.id],
                        ));
                    }
                }
            }
        }
    }

    // ヒューズ定格の簡易チェック
    for e in sheet.entities.values() {
        let Entity::Symbol(s) = e else { continue };
        let Some(def) = defs.get(s.symbol_id.as_str()) else { continue };
        if def.id != "fuse" {
            continue;
        }
        let Some(rating) = parse_number(&s.value) else { continue };
        let label = if s.reference.is_empty() { "fuse" } else { s.reference.as_str() };
        let max_amps = nets_of(s, def)
            .into_iter()
            .map(|ni| net_current[ni])
            .fold(0.0f64, f64::max);
        if max_amps > rating + 1e-9 {
            diags.push(diag(
                Severity::Warning,
                "elec.fuse_rating",
                format!("{label}: 負荷電流 {max_amps}A がヒューズ定格 {rating}A を超えています"),
                vec![s.id],
            ));
        }
    }

    diags
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
            // 注記とハーネス境界は電気的な接続を持たない
            Entity::Text(_) | Entity::Harness(_) => {}
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
                LABEL_CONFLICT,
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

    /// A fully wired circuit produces no ERC findings.
    /// 完全に結線された回路ではERCの指摘は出ない。
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

    /// Unconnected pins are reported as one warning per symbol, listing the affected pin numbers.
    /// 未接続ピンはシンボルごとに1件の警告として、該当ピン番号を列挙して報告される。
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

    /// A terminal-block terminal counts as connected if either its left or right side is wired.
    /// 端子台の端子は左右どちらか一方が結線されていれば接続済みとみなす。
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

    /// Missing and duplicate reference designators are flagged, but relay coil + contacts legitimately share one designator.
    /// 参照記号の未設定・重複は指摘されるが、リレーのコイル+接点が同じ記号を共有するのは正当として許容される。
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

    /// Wire ends attached to nothing are flagged, including an endpoint resting mid-wire without a junction dot.
    /// どこにも接続されていないワイヤ端点は指摘される(ジャンクション無しで他ワイヤの途中に乗る端点も含む)。
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

    /// battery→fuse→switch→lamp→(戻り)battery の直列回路。
    /// lamp_attrs/fuse_value/wire_sq/戻り以外のワイヤを差し替えて各検証を試す。
    fn series_circuit(
        lamp_current: Option<&str>,
        fuse_value: &str,
        sq: f64,
        length_m: Option<f64>,
        skip_fuse_to_switch: bool,
    ) -> Sheet {
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
        if let Some(c) = lamp_current {
            lamp.attrs.insert("current_a".into(), c.into());
        }
        let mut fuse = SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "fuse".into(),
            at: Point::new(100.0, 100.0),
            rotation: 0,
            mirror: false,
            reference: "F1".into(),
            value: fuse_value.into(),
            attrs: Default::default(),
        };
        fuse.value = fuse_value.into();
        let mut entities = vec![
            Entity::Symbol(SymbolInstance {
                id: Uuid::new_v4(),
                symbol_id: "battery".into(),
                at: Point::new(60.0, 100.0),
                rotation: 0,
                mirror: false,
                reference: "BT1".into(),
                value: "DC24V".into(),
                attrs: Default::default(),
            }),
            Entity::Symbol(fuse),
            Entity::Symbol(SymbolInstance {
                id: Uuid::new_v4(),
                symbol_id: "switch_spst".into(),
                at: Point::new(140.0, 100.0),
                rotation: 0,
                mirror: false,
                reference: "SW1".into(),
                value: String::new(),
                attrs: Default::default(),
            }),
            Entity::Symbol(lamp),
        ];
        let mk_wire = |pts: &[(f64, f64)]| {
            Entity::Wire(Wire {
                id: Uuid::new_v4(),
                points: pts.iter().map(|&(x, y)| Point::new(x, y)).collect(),
                color: "red".into(),
                sq,
                length_m,
                part_no: None,
                net: None,
            })
        };
        entities.push(mk_wire(&[(67.5, 100.0), (92.5, 100.0)]));
        if !skip_fuse_to_switch {
            entities.push(mk_wire(&[(107.5, 100.0), (132.5, 100.0)]));
        }
        entities.push(mk_wire(&[(147.5, 100.0), (182.5, 100.0)]));
        entities.push(mk_wire(&[
            (197.5, 100.0),
            (220.0, 100.0),
            (220.0, 140.0),
            (40.0, 140.0),
            (40.0, 100.0),
            (52.5, 100.0),
        ]));
        sheet_with(entities)
    }

    /// A healthy series circuit (source, fuse, switch, lamp) raises no electrical findings.
    /// 健全な直列回路(電源・ヒューズ・スイッチ・ランプ)では電気検証の指摘は出ない。
    #[test]
    fn healthy_series_circuit_has_no_elec_findings() {
        let diags = run(&series_circuit(Some("1.0"), "5A", 0.75, None, false));
        // 近似モードのInfo(ngspice未導入環境)は「問題」ではないので除外して判定
        let elec: Vec<_> = diags
            .iter()
            .filter(|d| d.code.starts_with("elec.") && d.code != "elec.approximate_mode")
            .collect();
        assert!(elec.is_empty(), "{elec:?}");
    }

    /// A load cut off from the power source (broken wire or open path) is reported as unreachable.
    /// 電源から切り離された負荷(断線・開路)は「到達不能」として報告される。
    #[test]
    fn load_cut_off_from_source_is_unreachable() {
        // fuse→switch間のワイヤを外す: lampの両ピンは配線済みだが電源から届かない
        let diags = run(&series_circuit(Some("1.0"), "5A", 0.75, None, true));
        let unreachable: Vec<_> = diags
            .iter()
            .filter(|d| d.code == "elec.unreachable_load")
            .collect();
        assert_eq!(unreachable.len(), 1, "{unreachable:?}");
        assert!(unreachable[0].message.contains("L1"));
    }

    /// A load current exceeding the wire's ampacity is an error, and exceeding the fuse rating is a warning.
    /// 負荷電流がワイヤの許容電流を超えるとエラー、ヒューズ定格を超えると警告になる。
    #[test]
    fn overloaded_wire_and_fuse_are_flagged() {
        // 10A負荷: 等価抵抗モデルの実電流(約8.4A)でも0.3sq(許容7A)超過 + ヒューズ5A定格超過
        let diags = run(&series_circuit(Some("10"), "5A", 0.3, None, false));
        assert!(
            diags.iter().filter(|d| d.code == "elec.wire_ampacity").count() >= 1,
            "{diags:?}"
        );
        assert!(
            diags
                .iter()
                .any(|d| d.code == "elec.fuse_rating" && d.message.contains("F1")),
            "{diags:?}"
        );
    }

    /// Voltage drop above 3 % of the supply voltage on a long wire is flagged; a short wire passes.
    /// 長い配線で電源電圧の3%を超える電圧降下は指摘され、短い配線では出ない。
    #[test]
    fn excessive_voltage_drop_is_flagged() {
        // 15m x 0.75sq x 3A負荷: 近似(往復2.1V)でもシミュレーション(実電流~2.55Aで1本0.89V)でも
        // 24Vの3% (0.72V) を超える
        let diags = run(&series_circuit(Some("3"), "10A", 0.75, Some(15.0), false));
        assert!(
            diags.iter().any(|d| d.code == "elec.voltage_drop"),
            "{diags:?}"
        );
        // 0.5mなら降下0.07Vで問題なし
        let ok = run(&series_circuit(Some("3"), "10A", 0.75, Some(0.5), false));
        assert!(!ok.iter().any(|d| d.code == "elec.voltage_drop"), "{ok:?}");
    }

    /// A load without a current_a attribute is excluded from current checks and reported as an Info note.
    /// current_a属性が無い負荷は電流計算から除外され、Infoとして通知される。
    #[test]
    fn load_without_current_attr_gets_info_and_no_current_checks() {
        let diags = run(&series_circuit(None, "5A", 0.3, Some(10.0), false));
        assert!(
            diags
                .iter()
                .any(|d| d.code == "elec.no_current_attr" && d.severity == Severity::Info),
            "{diags:?}"
        );
        assert!(!diags.iter().any(|d| d.code == "elec.wire_ampacity"));
        assert!(!diags.iter().any(|d| d.code == "elec.voltage_drop"));
    }

    /// With ngspice installed, electrical findings carry solver-measured values and no approximate-mode note appears.
    /// ngspiceがあれば電気検証はソルバの実測値で報告され、近似モードの通知は出ない。
    #[test]
    fn simulation_mode_reports_measured_values_when_ngspice_installed() {
        if crate::ngspice::find_ngspice().is_none() {
            eprintln!("ngspice未検出のためスキップ");
            return;
        }
        let diags = run(&series_circuit(Some("3"), "10A", 0.75, Some(15.0), false));
        assert!(
            diags
                .iter()
                .any(|d| d.code == "elec.voltage_drop" && d.message.contains("シミュレーション値")),
            "{diags:?}"
        );
        assert!(!diags.iter().any(|d| d.code == "elec.approximate_mode"));
    }

    /// Without a solver result, checks fall back to a graph approximation and say so with an Info note.
    /// ソルバ結果が無い場合はグラフ近似にフォールバックし、その旨をInfoで明示する。
    #[test]
    fn fallback_mode_emits_approximate_info() {
        let sheet = series_circuit(Some("1.0"), "5A", 0.75, None, false);
        let diags = verify_sheet_with(&sheet, &sheet_symbol_defs(&sheet), None);
        assert!(
            diags
                .iter()
                .any(|d| d.code == "elec.approximate_mode" && d.severity == Severity::Info),
            "{diags:?}"
        );
    }

    /// Two different net labels on one net (a short between potentials) is an error.
    /// 1つのネットに異なるネットラベルが混在する状態(異電位の短絡)はエラーになる。
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

    fn net_label(name: &str, x: f64, y: f64) -> Entity {
        Entity::NetLabel(NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(x, y),
            name: name.into(),
            rotation: 0,
        })
    }

    /// A single-sheet label conflict is reported exactly once when the whole project is verified.
    /// シート1枚の中のラベル競合は、プロジェクト全体を検証しても1件だけ報告される。
    #[test]
    fn project_verification_reports_a_single_sheet_label_conflict_once() {
        let mut project = crate::model::Project::new("t");
        let sid = project.sheets[0].id;
        for e in [
            wire(&[(20.0, 20.0), (60.0, 20.0)]),
            net_label("24V", 20.0, 20.0),
            net_label("0V", 60.0, 20.0),
        ] {
            project.sheet_mut(sid).unwrap().entities.insert(e.id(), e);
        }
        let conflicts: Vec<_> = verify_project(&project)
            .into_iter()
            .filter(|d| d.code == LABEL_CONFLICT)
            .collect();
        assert_eq!(conflicts.len(), 1, "{conflicts:?}");
        assert!(conflicts[0].message.contains("0V") && conflicts[0].message.contains("24V"));
    }

    /// Nets joined across sheets by a shared label name are checked as one net, so a conflict spanning two sheets is reported once with all offending labels.
    /// 同名ラベルでシートを跨いで繋がったネットは1ネットとして検査されるので、2枚に跨る競合も全ラベルを挙げた1件として報告される。
    #[test]
    fn project_verification_merges_label_conflicts_across_sheets() {
        let mut project = crate::model::Project::new("t");
        project
            .sheets
            .push(Sheet::new("Sheet2", PaperSize::A3, Orientation::Landscape));
        let (s1, s2) = (project.sheets[0].id, project.sheets[1].id);
        // シート1: 24V と BRIDGE が同じネットに乗る
        for e in [
            wire(&[(20.0, 20.0), (60.0, 20.0)]),
            net_label("24V", 20.0, 20.0),
            net_label("BRIDGE", 60.0, 20.0),
        ] {
            project.sheet_mut(s1).unwrap().entities.insert(e.id(), e);
        }
        // シート2: BRIDGE と 0V が同じネットに乗る → 全体では 24V/BRIDGE/0V が1ネット
        for e in [
            wire(&[(20.0, 20.0), (60.0, 20.0)]),
            net_label("BRIDGE", 20.0, 20.0),
            net_label("0V", 60.0, 20.0),
        ] {
            project.sheet_mut(s2).unwrap().entities.insert(e.id(), e);
        }
        let conflicts: Vec<_> = verify_project(&project)
            .into_iter()
            .filter(|d| d.code == LABEL_CONFLICT)
            .collect();
        assert_eq!(conflicts.len(), 1, "跨ぎネットは1件にまとまる: {conflicts:?}");
        for name in ["0V", "24V", "BRIDGE"] {
            assert!(conflicts[0].message.contains(name), "{:?}", conflicts[0]);
        }
        assert_eq!(conflicts[0].entity_ids.len(), 4, "両シートのラベルを指す");
    }

    /// A net continued onto another sheet with the same label name is not a conflict.
    /// 同じラベル名でシートを跨いで続くネットは、競合ではない。
    #[test]
    fn project_verification_accepts_a_net_continued_onto_another_sheet() {
        let mut project = crate::model::Project::new("t");
        project
            .sheets
            .push(Sheet::new("Sheet2", PaperSize::A3, Orientation::Landscape));
        let (s1, s2) = (project.sheets[0].id, project.sheets[1].id);
        for sid in [s1, s2] {
            for e in [wire(&[(20.0, 20.0), (60.0, 20.0)]), net_label("24V_1", 20.0, 20.0)] {
                project.sheet_mut(sid).unwrap().entities.insert(e.id(), e);
            }
        }
        assert!(
            !verify_project(&project).iter().any(|d| d.code == LABEL_CONFLICT),
            "{:?}",
            verify_project(&project)
        );
        // 帳票・検証では1ネットとして数える
        assert_eq!(crate::xref::extract_netlist_project(&project).nets.len(), 1);
    }
}
