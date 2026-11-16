//! SPICEネットリスト生成 (spec §3.5)。電気検証のngspiceバックエンド用。
//!
//! ノードは「接続点クラスタ」(座標一致するピン・ジャンクション・ワイヤ端点)。
//! ネット単位ではない: ワイヤは抵抗素子になり、ワイヤ抵抗でノードが分かれる。
//!
//! シンボル定義に `sim` (役割) があれば、それに従って電源・負荷・ヒューズ・導通・変換器を作る
//! (ユーザーシンボルでもシミュレーションできる。値は置いた部品の属性)。無ければカテゴリで決める従来の扱い。

use std::collections::BTreeMap;

use crate::geometry::Point;
use crate::model::{Sheet, SymbolInstance};
use crate::symbol::{SimRole, SymbolDef};
use crate::EntityId;

/// 銅の抵抗率 (Ω·mm²/m)。verify.rsと共通。
pub const COPPER_RESISTIVITY: f64 = 0.0175;
/// 長さ未設定ワイヤ・導通部品・ラベル橋の微小抵抗 (Ω)。
pub const BRIDGE_OHMS: f64 = 1e-3;
/// current_a未設定の負荷の等価抵抗 (Ω)。存在だけ表現する。
pub const UNKNOWN_LOAD_OHMS: f64 = 1e6;
/// 役割付きの部品がある図面で、全ノードから接地へ入れる漏れ抵抗 (Ω)。浮いた回路でも解けるようにする。
pub const SHUNT_OHMS: f64 = 1e9;
/// 変換器の効率の既定値。
pub const DEFAULT_ETA: f64 = 0.9;

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum SpiceError {
    #[error("電源(battery)がありません")]
    NoSource,
}

/// 抵抗素子の由来の種類 (判定で使う)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElementKind {
    /// ワイヤの区間。
    Wire,
    /// 同じ番号のピンどうしの内部短絡 (端子台・コネクタの貫通)。ピン番号付き。
    Pin(String),
    /// 導通部品・ヒューズの橋。
    Bridge,
    /// 負荷 (等価抵抗、または定電流源の直列の検出抵抗)。
    Load,
    /// 電源の内部抵抗。
    Source,
    /// 変換器の入力側の検出抵抗。
    ConverterIn,
    /// 変換器の出力側の検出抵抗。
    ConverterOut,
    /// 同名ネットラベル・ネット名シンボルの橋。
    Label,
    /// 短絡指定 (ネットから接地へ)。ネット名付き。
    Short(String),
    /// 接地につながっていない回路 (絶縁された変換器の出力など) の負極を接地へ結ぶ基準。電流は数えない。
    Tie,
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
    pub kind: ElementKind,
}

/// 役割が負荷の部品の端子 (端子電圧の判定に使う)。
#[derive(Debug, Clone)]
pub struct LoadTerminals {
    pub entity_id: EntityId,
    pub plus: String,
    pub minus: String,
}

#[derive(Debug)]
pub struct SpiceDeck {
    /// SPICEネットリスト本文 (.end含まず。解析コマンドはランナーが付ける)。
    pub deck: String,
    /// 電源電圧 (V)。最初の電源の電圧。
    pub voltage: f64,
    /// ワイヤ由来の抵抗素子 (分割されたワイヤは複数エントリ)。
    pub wire_elements: Vec<SpiceElement>,
    /// 部品・ラベル橋由来の抵抗素子 (ヒューズ電流の算出などに使う)。
    pub component_elements: Vec<SpiceElement>,
    /// 役割が負荷の部品の端子ノード。
    pub load_terminals: Vec<LoadTerminals>,
    /// 組み立て時の注意 (見つからない短絡ネット・参照記号など)。
    pub warnings: Vec<String>,
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
    /// 部品の属性の置き換え (参照記号 → {属性: 値})。同じ参照記号の全部品に効く。
    pub overrides: BTreeMap<String, BTreeMap<String, String>>,
    /// 接地へ短絡させるネットの名前 (1mΩ)。
    pub shorts: Vec<String>,
}

impl DeckOptions {
    /// 部品の属性 (置き換えがあればそちら)。
    pub fn attr<'a>(&'a self, s: &'a SymbolInstance, key: &str) -> Option<&'a str> {
        self.overrides
            .get(&s.reference)
            .and_then(|m| m.get(key))
            .or_else(|| s.attrs.get(key))
            .map(String::as_str)
    }

    /// 部品の属性を数で読む (例: "2.5A" → 2.5)。
    pub fn number(&self, s: &SymbolInstance, key: &str) -> Option<f64> {
        self.attr(s, key).and_then(crate::verify::parse_number)
    }
}

/// シートからSPICEデッキを組み立てる。
pub fn build_deck(sheet: &Sheet, symbols: &[SymbolDef]) -> Result<SpiceDeck, SpiceError> {
    build_deck_with(sheet, symbols, &DeckOptions::default())
}

/// デッキのノード: 接続点クラスタか、部品の内部ノード。
#[derive(Debug, Clone)]
enum Node {
    Key((i64, i64)),
    Internal(String),
}

/// 抵抗以外の素子 (ノード名が決まってから書く)。
enum Extra {
    /// 電圧源 (+, −, 電圧)。
    Volt { name: String, plus: Node, minus: Node, volts: f64, owner: (EntityId, String) },
    /// 負荷の電流源 (端子電圧 1V 以上で定電流)。電流は from から素子を通って to へ流れる。
    Amp { name: String, from: Node, to: Node, amps: f64 },
    /// 変換器の入力 (定電力の電流源)。出力側の電圧源の電流から入力電流を決める。
    ConverterInput { name: String, from: Node, to: Node, out_src: String, out_p: Node, out_m: Node, eta: f64 },
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
    use std::collections::BTreeSet;

    let defs: BTreeMap<&str, &SymbolDef> = symbols.iter().map(|d| (d.id.as_str(), d)).collect();
    let mut warnings: Vec<String> = Vec::new();

    // 電源: 同梱の battery と役割 source (BTreeMap順で決定的)。最初の電源の負極が接地
    let mut batteries = Vec::new();
    let mut role_sources: Vec<(&SymbolInstance, Vec<(String, Point)>, Point, Point)> = Vec::new();
    let mut has_roles = false;
    let mut attachments: Vec<crate::geometry::Point> = Vec::new(); // ワイヤ分割点になる点
    let mut labels: BTreeMap<&str, Vec<(EntityId, crate::geometry::Point)>> = BTreeMap::new();
    let pin_of = |pins: &[(String, Point)], no: &str| pins.iter().find(|(n, _)| n.trim() == no.trim()).map(|(_, p)| *p);
    for e in sheet.entities.values() {
        match e {
            Entity::Symbol(s) => {
                if let Some(&def) = defs.get(s.symbol_id.as_str()) {
                    let pins = pin_positions(s, def);
                    attachments.extend(pins.iter().map(|(_, p)| *p));
                    // ネット名を持つシンボル (接地の印など) のピンは、その名前のネットラベルとして橋を張る
                    if let Some(name) = def.net_name() {
                        labels.entry(name).or_default().extend(pins.iter().map(|(_, p)| (s.id, *p)));
                    }
                    if let Some(sim) = &def.sim {
                        has_roles = true;
                        if sim.role == SimRole::Source {
                            if let (Some(p), Some(m)) = (
                                sim.pins.first().and_then(|n| pin_of(&pins, n)),
                                sim.pins.get(1).and_then(|n| pin_of(&pins, n)),
                            ) {
                                role_sources.push((s, pins.clone(), p, m));
                            }
                        }
                    } else if def.id == "battery" {
                        batteries.push((s, def, pins));
                    }
                }
            }
            Entity::Junction(j) => attachments.push(j.at),
            Entity::NetLabel(l) => {
                attachments.push(l.at);
                labels.entry(l.name.as_str()).or_default().push((l.id, l.at));
            }
            Entity::Wire(_)
            | Entity::Text(_)
            | Entity::Shape(_)
            | Entity::Harness(_)
            | Entity::PanelOutline(_)
            | Entity::DinRail(_)
            | Entity::Duct(_)
            | Entity::Footprint(_)
            | Entity::PanelLegend(_)
            | Entity::WireLegend(_)
            | Entity::Cable(_) | Entity::Nameplate(_)
            | Entity::Dimension(_) => {}
        }
    }
    let source_volts = |s: &SymbolInstance| {
        opts.number(s, "voltage").or_else(|| parse_number(&s.value)).unwrap_or(24.0)
    };
    let (voltage, gnd_key) = if let Some((s, _, pins)) = batteries.first() {
        let minus = pins.last().map(|(_, p)| *p).expect("battery has pins");
        (source_volts(s), node_key(minus))
    } else if let Some((s, _, _, m)) = role_sources.first() {
        (source_volts(s), node_key(*m))
    } else {
        return Err(SpiceError::NoSource);
    };
    for r in opts.overrides.keys() {
        let found = sheet
            .entities
            .values()
            .any(|e| matches!(e, Entity::Symbol(s) if &s.reference == r));
        if !found {
            warnings.push(format!("置き換え指定の参照記号 {r} が図面にありません"));
        }
    }

    let mut used_keys: BTreeSet<(i64, i64)> = BTreeSet::new();
    let mut seq = 0usize;
    let mut wire_elements = Vec::new();
    let mut component_elements = Vec::new();
    let mut extras: Vec<Extra> = Vec::new();
    let mut raw_loads: Vec<(EntityId, (i64, i64), (i64, i64))> = Vec::new();
    let next_name = |seq: &mut usize| {
        *seq += 1;
        format!("R{seq}")
    };
    let mut internal_seq = 0usize;
    let mut next_internal = |prefix: &str| {
        internal_seq += 1;
        format!("{prefix}{internal_seq}")
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
                kind: ElementKind::Wire,
            });
        }
    }

    // シンボル → 導通橋(1mΩ)・端子台の同番号貫通・負荷・電源・変換器
    for e in sheet.entities.values() {
        let Entity::Symbol(s) = e else { continue };
        let Some(def) = defs.get(s.symbol_id.as_str()) else { continue };
        let pins = pin_positions(s, def);
        // 抵抗素子 (a, b はキーか内部ノードの名前)
        let mut push = |a: String, b: String, ohms: f64, kind: ElementKind, used: &mut BTreeSet<(i64, i64)>| {
            if a == b {
                return;
            }
            for n in [&a, &b] {
                if let Some(k) = parse_key(n) {
                    used.insert(k);
                }
            }
            component_elements.push(SpiceElement {
                entity_id: s.id,
                name: next_name(&mut seq),
                node_a: a,
                node_b: b,
                ohms,
                kind,
            });
        };
        let key = |p: Point| format!("{:?}", node_key(p));
        // 同一ピン番号の複数接続点(端子台・コネクタの左右)は内部短絡
        let mut by_no: BTreeMap<&str, Vec<crate::geometry::Point>> = BTreeMap::new();
        for (no, p) in &pins {
            by_no.entry(no.trim()).or_default().push(*p);
        }
        for (no, pts) in &by_no {
            for pair in pts.windows(2) {
                if node_key(pair[0]) != node_key(pair[1]) {
                    push(key(pair[0]), key(pair[1]), BRIDGE_OHMS, ElementKind::Pin(no.to_string()), &mut used_keys);
                }
            }
        }
        let first_of = |no: &str| by_no.get(no.trim()).map(|v| v[0]);
        // 役割が使うピンが見つからなければ、黙って落とさず知らせる
        if let Some(sim) = &def.sim {
            if let Some(missing) = sim.pins.iter().find(|n| first_of(n).is_none()) {
                warnings.push(format!(
                    "{}: シミュレーションの役割が使うピン{missing}が見つからないため、この部品を除きました",
                    if s.reference.is_empty() { def.id.as_str() } else { s.reference.as_str() }
                ));
            }
        }
        let is_open = opts.open_switches.iter().any(|r| r == &s.reference);
        if let Some(sim) = &def.sim {
            match sim.role {
                SimRole::None => {}
                SimRole::Fuse | SimRole::Conductor => {
                    if !is_open {
                        // ピン指定が無ければ同梱と同じ極の規則 (1..2n の番号なら 1-2 / 3-4 …、それ以外は全ピン)
                        let groups: Vec<Vec<&str>> = if sim.pins.is_empty() {
                            crate::symbol::conducting_pin_groups(def)
                        } else {
                            vec![sim.pins.iter().map(String::as_str).collect()]
                        };
                        for numbers in groups {
                            let pts: Vec<Point> = numbers.iter().filter_map(|n| first_of(n)).collect();
                            for p in pts.iter().skip(1) {
                                push(key(pts[0]), key(*p), BRIDGE_OHMS, ElementKind::Bridge, &mut used_keys);
                            }
                        }
                    }
                }
                SimRole::Source => {
                    let (Some(p), Some(m)) = (sim.pins.first().and_then(|n| first_of(n)), sim.pins.get(1).and_then(|n| first_of(n))) else { continue };
                    let inner = next_internal("s");
                    let r = opts.number(s, "r_int_ohm").filter(|r| *r > 0.0).unwrap_or(BRIDGE_OHMS);
                    used_keys.insert(node_key(m));
                    extras.push(Extra::Volt {
                        name: format!("V{}", inner.to_uppercase()),
                        plus: Node::Internal(inner.clone()),
                        minus: Node::Key(node_key(m)),
                        volts: source_volts(s),
                        owner: (s.id, s.reference.clone()),
                    });
                    push(inner, key(p), r, ElementKind::Source, &mut used_keys);
                }
                SimRole::Load => {
                    let (Some(p), Some(m)) = (sim.pins.first().and_then(|n| first_of(n)), sim.pins.get(1).and_then(|n| first_of(n))) else { continue };
                    raw_loads.push((s.id, node_key(p), node_key(m)));
                    match opts.number(s, "current_a").filter(|a| *a > 0.0) {
                        Some(amps) => {
                            let inner = next_internal("l");
                            used_keys.insert(node_key(m));
                            extras.push(Extra::Amp {
                                name: format!("B{}", inner.to_uppercase()),
                                from: Node::Internal(inner.clone()),
                                to: Node::Key(node_key(m)),
                                amps,
                            });
                            push(key(p), inner, BRIDGE_OHMS, ElementKind::Load, &mut used_keys);
                        }
                        None => push(key(p), key(m), UNKNOWN_LOAD_OHMS, ElementKind::Load, &mut used_keys),
                    }
                }
                SimRole::Converter => {
                    let ends: Option<Vec<Point>> = sim.pins.iter().map(|n| first_of(n)).collect();
                    let Some(ends) = ends.filter(|e| e.len() == 4) else { continue };
                    let (ip, im, op, om) = (ends[0], ends[1], ends[2], ends[3]);
                    let v_out = opts.number(s, "v_out").or_else(|| parse_number(&s.value)).unwrap_or(12.0);
                    let eta = match opts.number(s, "eta") {
                        Some(e) if e > 0.0 && e <= 1.0 => e,
                        Some(e) => {
                            warnings.push(format!(
                                "{}: 効率 eta={e} は 0より大きく1以下でないため {DEFAULT_ETA} を使いました",
                                s.reference
                            ));
                            DEFAULT_ETA
                        }
                        None => DEFAULT_ETA,
                    };
                    let (ci, co) = (next_internal("ci"), next_internal("co"));
                    let out_src = format!("V{}", co.to_uppercase());
                    used_keys.insert(node_key(im));
                    used_keys.insert(node_key(om));
                    extras.push(Extra::Volt {
                        name: out_src.clone(),
                        plus: Node::Internal(co.clone()),
                        minus: Node::Key(node_key(om)),
                        volts: v_out,
                        owner: (s.id, s.reference.clone()),
                    });
                    extras.push(Extra::ConverterInput {
                        name: format!("B{}", ci.to_uppercase()),
                        from: Node::Internal(ci.clone()),
                        to: Node::Key(node_key(im)),
                        out_src,
                        out_p: Node::Internal(co.clone()),
                        out_m: Node::Key(node_key(om)),
                        eta,
                    });
                    push(key(ip), ci, BRIDGE_OHMS, ElementKind::ConverterIn, &mut used_keys);
                    push(co, key(op), BRIDGE_OHMS, ElementKind::ConverterOut, &mut used_keys);
                }
            }
            continue;
        }
        // 番号代表点 (番号ごとの先頭)
        let reps: Vec<crate::geometry::Point> = by_no.values().map(|pts| pts[0]).collect();
        if is_conductor(def) {
            // what-if: 指定された参照記号の導通部品は開路扱い(橋を張らない)
            if !is_open {
                // 多極機器は極ごと(1-2 / 3-4 / …)に橋を張り、相どうしは短絡させない
                for group in crate::symbol::conducting_pin_groups(def) {
                    let pts: Vec<crate::geometry::Point> =
                        group.iter().filter_map(|no| by_no.get(*no).map(|v| v[0])).collect();
                    for p in pts.iter().skip(1) {
                        push(key(pts[0]), key(*p), BRIDGE_OHMS, ElementKind::Bridge, &mut used_keys);
                    }
                }
            }
        } else if is_load(def) && reps.len() >= 2 {
            let ohms = match opts.number(s, "current_a") {
                Some(a) if a > 0.0 => voltage / a,
                _ => UNKNOWN_LOAD_OHMS,
            };
            push(key(reps[0]), key(reps[1]), ohms, ElementKind::Load, &mut used_keys);
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
                kind: ElementKind::Label,
            });
        }
    }

    // 接地につながっていない回路の基準: 電圧源 (電源・変換器の出力) の負極が接地へ届かなければ 1mΩ で結ぶ。
    // 絶縁された変換器の出力が漏れ抵抗だけで決まる変な電位にならないようにする (共通の帰路を持つ機器と同じ扱い)
    {
        let gnd = format!("{gnd_key:?}");
        let mut parent: BTreeMap<String, String> = BTreeMap::new();
        fn find(parent: &mut BTreeMap<String, String>, x: &str) -> String {
            let p = parent.get(x).cloned().unwrap_or_else(|| x.to_string());
            if p == x {
                return p;
            }
            let r = find(parent, &p);
            parent.insert(x.to_string(), r.clone());
            r
        }
        let mut union = |parent: &mut BTreeMap<String, String>, a: &str, b: &str| {
            let (ra, rb) = (find(parent, a), find(parent, b));
            if ra != rb {
                parent.insert(ra, rb);
            }
        };
        let raw = |n: &Node| match n {
            Node::Key(k) => format!("{k:?}"),
            Node::Internal(s) => s.clone(),
        };
        for el in wire_elements.iter().chain(component_elements.iter()) {
            union(&mut parent, &el.node_a, &el.node_b);
        }
        if let Some((s, _, pins)) = batteries.first() {
            let _ = s;
            union(&mut parent, &format!("{:?}", node_key(pins[0].1)), &gnd);
        }
        for x in &extras {
            if let Extra::Volt { plus, minus, .. } = x {
                union(&mut parent, &raw(plus), &raw(minus));
            }
        }
        for x in &extras {
            let Extra::Volt { minus, owner, .. } = x else { continue };
            let m = raw(minus);
            if find(&mut parent, &m) != find(&mut parent, &gnd) {
                union(&mut parent, &m, &gnd);
                used_keys.insert(gnd_key);
                component_elements.push(SpiceElement {
                    entity_id: owner.0,
                    name: next_name(&mut seq),
                    node_a: m,
                    node_b: gnd.clone(),
                    ohms: BRIDGE_OHMS,
                    kind: ElementKind::Tie,
                });
                warnings.push(format!("{}: 接地につながっていない回路のため、負極を接地の基準にしました", owner.1));
            }
        }
    }

    // 短絡指定: ネットの最初の接続点から接地へ 1mΩ
    if !opts.shorts.is_empty() {
        let nets = crate::netlist::extract_netlist(sheet, symbols);
        for name in &opts.shorts {
            let Some(net) = nets.iter().find(|n| &n.name == name || n.label.as_deref() == Some(name.as_str())) else {
                warnings.push(format!("短絡指定のネット {name} が図面にありません"));
                continue;
            };
            let point = net.wire_ids.iter().find_map(|id| match sheet.entities.get(id) {
                Some(Entity::Wire(w)) => w.points.first().copied(),
                _ => None,
            });
            let point = point.or_else(|| {
                net.pins.iter().find_map(|np| match sheet.entities.get(&np.entity_id) {
                    Some(Entity::Symbol(s)) => defs
                        .get(s.symbol_id.as_str())
                        .and_then(|d| pin_positions(s, d).into_iter().find(|(n, _)| n == &np.pin).map(|(_, p)| p)),
                    _ => None,
                })
            });
            let Some(point) = point else {
                warnings.push(format!("短絡指定のネット {name} に接続点がありません"));
                continue;
            };
            let k = node_key(point);
            used_keys.insert(k);
            used_keys.insert(gnd_key);
            component_elements.push(SpiceElement {
                entity_id: net.wire_ids.first().copied().unwrap_or_else(|| net.pins[0].entity_id),
                name: next_name(&mut seq),
                node_a: format!("{k:?}"),
                node_b: format!("{gnd_key:?}"),
                ohms: BRIDGE_OHMS,
                kind: ElementKind::Short(name.clone()),
            });
            warnings.push(format!("短絡指定: {name}"));
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
        match parse_key(raw) {
            Some(key) => names.get(&key).cloned().unwrap_or_else(|| "0".into()),
            None => raw.to_string(), // 部品の内部ノード
        }
    };
    let node_name = |node: &Node| match node {
        Node::Key(k) => names.get(k).cloned().unwrap_or_else(|| "0".into()),
        Node::Internal(s) => s.clone(),
    };
    for el in wire_elements.iter_mut().chain(component_elements.iter_mut()) {
        el.node_a = resolve(&el.node_a, &names);
        el.node_b = resolve(&el.node_b, &names);
    }
    let load_terminals = raw_loads
        .into_iter()
        .map(|(entity_id, p, m)| LoadTerminals {
            entity_id,
            plus: node_name(&Node::Key(p)),
            minus: node_name(&Node::Key(m)),
        })
        .collect();

    // デッキ本文
    let mut deck = String::from("* MadakeCAD verification\n");
    for (i, (s, _, pins)) in batteries.iter().enumerate() {
        let v = source_volts(s);
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
    for x in &extras {
        match x {
            Extra::Volt { name, plus, minus, volts, .. } => {
                deck.push_str(&format!("{name} {} {} DC {volts}\n", node_name(plus), node_name(minus)));
            }
            Extra::Amp { name, from, to, amps } => {
                // 定電流。ただし端子電圧が 1V を下回ると電流も比例して減る (経路が切れたとき電圧が暴れないように)
                let (f, t) = (node_name(from), node_name(to));
                deck.push_str(&format!("{name} {f} {t} I = {amps}*min(max(v({f},{t}),0),1)\n"));
            }
            Extra::ConverterInput { name, from, to, out_src, out_p, out_m, eta } => {
                // 入力電流 = 出力電力 / (効率 × 入力電圧)。電圧源の電流は流れ込む向きが正なので符号を反転
                let (f, t) = (node_name(from), node_name(to));
                deck.push_str(&format!(
                    "{name} {f} {t} I = max(v({},{}),0)*max(-i({out_src}),0)/({eta}*max(v({f},{t}),1))\n",
                    node_name(out_p),
                    node_name(out_m),
                ));
            }
        }
    }
    for el in wire_elements.iter().chain(component_elements.iter()) {
        deck.push_str(&format!(
            "{} {} {} {}\n",
            el.name, el.node_a, el.node_b, el.ohms
        ));
    }
    if has_roles {
        deck.push_str(&format!(".options rshunt={SHUNT_OHMS}\n"));
    }

    Ok(SpiceDeck {
        deck,
        voltage,
        wire_elements,
        component_elements,
        load_terminals,
        warnings,
    })
}

/// "(x, y)" 形式のデバッグキーを読む。内部ノード名なら None。
fn parse_key(raw: &str) -> Option<(i64, i64)> {
    let t = raw.strip_prefix('(')?.strip_suffix(')')?;
    let mut it = t.split(',').map(|s| s.trim().parse::<i64>());
    match (it.next(), it.next()) {
        (Some(Ok(x)), Some(Ok(y))) => Some((x, y)),
        _ => None,
    }
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
            length_source: Default::default(),
            part_no: None,
            net: None,
            dashed: false, weight_mm: None,
        })
    }

    fn sheet_with(entities: Vec<Entity>) -> Sheet {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        for e in entities {
            sheet.entities.insert(e.id(), e);
        }
        sheet
    }

    /// A series circuit converts to a SPICE deck: one voltage source, ground at the battery minus pin, one resistor per wire (rho*L/A) plus component bridges and the load's equivalent resistance.
    /// 直列回路はSPICEデッキに変換される: 電圧源1つ・battery負極がGND・ワイヤごとの抵抗(ρL/A)・部品の橋・負荷の等価抵抗。
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

    /// A junction resting mid-wire splits that wire's resistance proportionally to the geometric length on each side.
    /// ワイヤ途中のジャンクションは、その位置の幾何学的長さ比でワイヤ抵抗を分割する。
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

    /// Same-named net labels in different places are bridged with a milliohm resistor, preserving their logical connection.
    /// 離れた場所の同名ネットラベルはmΩ抵抗で橋渡しされ、論理接続が保たれる。
    #[test]
    fn same_name_labels_are_bridged() {
        let mk_label = |x: f64, y: f64| {
            Entity::NetLabel(NetLabel {
                id: Uuid::new_v4(),
                at: Point::new(x, y),
                name: "N-P1".into(),
                rotation: 0,
                attrs: Default::default(),
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

    /// A return path through two ground marks (symbols with a net name) with no wire between them is one connected node in the deck.
    /// 間に配線の無い2つの接地の印 (ネット名を持つシンボル) を通る帰り道は、デッキでも1つにつながったノードになる。
    #[test]
    fn a_return_path_through_two_ground_marks_is_one_node() {
        let far = wire(&[(100.0, 50.0), (120.0, 50.0)], 0.75, None);
        let far_id = far.id();
        let sheet = sheet_with(vec![
            symbol("battery", "BT1", -10.0, 0.0, "24"),
            // battery負極 (-2.5,0) から1つ目の印へ
            wire(&[(-2.5, 0.0), (20.0, 0.0)], 0.75, None),
            symbol("test_ground_mark", "", 20.0, 0.0, ""),
            // 離れた2つ目の印から先の配線
            far,
            symbol("test_ground_mark", "", 100.0, 50.0, ""),
        ]);
        let mut symbols = crate::symbol::builtin_symbols();
        symbols.push(crate::symbol::test_net_mark("test_ground_mark", "GND"));
        let deck = build_deck(&sheet, &symbols).expect("deck");
        // 素子の両端を結んで、GND (0) と同じかたまりに入るノードを集める
        let elements: Vec<&SpiceElement> = deck.wire_elements.iter().chain(&deck.component_elements).collect();
        let mut joined: std::collections::BTreeSet<&str> = ["0"].into();
        loop {
            let before = joined.len();
            for e in &elements {
                if joined.contains(e.node_a.as_str()) || joined.contains(e.node_b.as_str()) {
                    joined.insert(&e.node_a);
                    joined.insert(&e.node_b);
                }
            }
            if joined.len() == before {
                break;
            }
        }
        let far_el = deck.wire_elements.iter().find(|e| e.entity_id == far_id).expect("far wire element");
        assert!(joined.contains(far_el.node_a.as_str()) && joined.contains(far_el.node_b.as_str()), "{}", deck.deck);
    }

    /// A drawing without a power source cannot be converted (explicit NoSource error).
    /// 電源の無い図面は変換できない(明示的なNoSourceエラー)。
    #[test]
    fn no_battery_is_an_error() {
        let sheet = sheet_with(vec![wire(&[(0.0, 0.0), (10.0, 0.0)], 0.75, None)]);
        assert_eq!(
            build_deck(&sheet, &sheet_symbol_defs(&sheet)).unwrap_err(),
            SpiceError::NoSource
        );
    }

    /// 役割付きのユーザーシンボル (JSON で定義)。
    fn role_symbol(id: &str, role: &str, pins: &[(&str, f64)], sim_pins: &[&str]) -> SymbolDef {
        let pins: Vec<serde_json::Value> =
            pins.iter().map(|(n, y)| serde_json::json!({"number": n, "at": {"x": 0.0, "y": y}})).collect();
        serde_json::from_value(serde_json::json!({
            "id": id, "name": id, "name_ja": id, "category": "user", "ref_prefix": "X",
            "primitives": [], "pins": pins, "free_pins": true,
            "sim": {"role": role, "pins": sim_pins}
        }))
        .unwrap()
    }

    /// A drawing whose only source is a user symbol with the source role can be converted: the source's minus pin is ground, the load becomes a constant current source, and every node gets a high-value shunt so isolated parts still solve.
    /// 電源が「電源の役割を付けたユーザーシンボル」だけの図面も変換できる: 電源の負極が接地になり、負荷は定電流源になり、浮いた部分も解けるよう全ノードに大きな漏れ抵抗が入る。
    #[test]
    fn a_user_source_and_load_become_spice_sources() {
        let defs = vec![
            role_symbol("u_src", "source", &[("+", 0.0), ("-", 10.0)], &["+", "-"]),
            role_symbol("u_load", "load", &[("+", 0.0), ("-", 10.0)], &["+", "-"]),
        ];
        let mut load = symbol("u_load", "LD1", 50.0, 100.0, "");
        if let Entity::Symbol(s) = &mut load {
            s.attrs.insert("current_a".into(), "2".into());
        }
        let sheet = sheet_with(vec![
            symbol("u_src", "BT1", 0.0, 100.0, "DC12V"),
            load,
            wire(&[(0.0, 100.0), (50.0, 100.0)], 0.75, None),
            wire(&[(50.0, 110.0), (0.0, 110.0)], 0.75, None),
        ]);
        let deck = build_deck(&sheet, &defs).unwrap();
        assert!((deck.voltage - 12.0).abs() < 1e-9, "電圧は値の文字から");
        assert!(deck.deck.lines().any(|l| l.starts_with('V') && l.contains(" DC 12")), "{}", deck.deck);
        assert!(deck.deck.lines().any(|l| l.starts_with("BL") && l.contains("I = 2*")), "{}", deck.deck);
        assert!(deck.deck.contains(".options rshunt="), "{}", deck.deck);
        assert_eq!(deck.load_terminals.len(), 1);
        // 負荷の − から電線1本で電源の − (接地 0) へ戻る
        let minus = &deck.load_terminals[0].minus;
        assert!(
            deck.wire_elements.iter().any(|w| (w.node_a == *minus && w.node_b == "0") || (w.node_b == *minus && w.node_a == "0")),
            "{}",
            deck.deck
        );
    }

    /// When a built-in battery and a user source are both on the sheet, ground is the battery's minus pin.
    /// 同梱の battery と電源の役割のユーザーシンボルが両方あるときは、battery の負極が接地になる。
    #[test]
    fn a_built_in_battery_is_ground_before_a_user_source() {
        let defs: Vec<SymbolDef> = sheet_symbol_defs(&sheet_with(vec![symbol("battery", "BT1", 0.0, 0.0, "DC24V")]))
            .into_iter()
            .chain([role_symbol("u_src", "source", &[("+", 0.0), ("-", 10.0)], &["+", "-"])])
            .collect();
        let sheet = sheet_with(vec![
            symbol("u_src", "BT2", 100.0, 100.0, "12"),
            symbol("battery", "BT1", 0.0, 0.0, "DC24V"),
        ]);
        let deck = build_deck(&sheet, &defs).unwrap();
        assert!((deck.voltage - 24.0).abs() < 1e-9, "{}", deck.deck);
        assert!(deck.deck.lines().any(|l| l.starts_with("V1 ") && l.contains(" 0 DC 24")), "{}", deck.deck);
    }

    /// A drawing with only built-in symbols gets the same deck as before (no shunt option is added).
    /// 同梱シンボルだけの図面は、これまでと同じデッキになる (漏れ抵抗の指定は足さない)。
    #[test]
    fn a_built_in_only_drawing_gets_no_shunt_option() {
        let sheet = sheet_with(vec![
            symbol("battery", "BT1", 0.0, 0.0, "DC24V"),
            wire(&[(0.0, 0.0), (10.0, 0.0)], 0.75, None),
        ]);
        let deck = build_deck(&sheet, &sheet_symbol_defs(&sheet)).unwrap();
        assert!(!deck.deck.contains("rshunt"), "{}", deck.deck);
    }

    /// A two-pole breaker with the fuse role and pins numbered 1 to 4 conducts per pole (1-2 and 3-4), never between the poles, as built-in multi-pole devices do.
    /// ヒューズの役割を付けたピン番号1〜4の2極ブレーカーは、同梱の多極機器と同じく極ごと (1-2 と 3-4) に導通し、極どうしはつながない。
    #[test]
    fn a_two_pole_protective_device_conducts_per_pole() {
        let pins: Vec<serde_json::Value> = [("1", 0.0, 0.0), ("2", 10.0, 0.0), ("3", 0.0, 5.0), ("4", 10.0, 5.0)]
            .iter()
            .map(|(n, x, y)| serde_json::json!({"number": n, "at": {"x": x, "y": y}}))
            .collect();
        let breaker: SymbolDef = serde_json::from_value(serde_json::json!({
            "id": "u_mcb", "name": "mcb", "name_ja": "mcb", "category": "user", "ref_prefix": "Q",
            "primitives": [], "pins": pins, "free_pins": true, "sim": {"role": "fuse"}
        }))
        .unwrap();
        let defs = vec![role_symbol("u_src", "source", &[("+", 0.0), ("-", 10.0)], &["+", "-"]), breaker];
        let sheet = sheet_with(vec![symbol("u_src", "BT1", 0.0, 100.0, "24"), symbol("u_mcb", "Q1", 40.0, 100.0, "")]);
        let deck = build_deck(&sheet, &defs).unwrap();
        let bridges: Vec<&SpiceElement> =
            deck.component_elements.iter().filter(|e| e.kind == ElementKind::Bridge).collect();
        assert_eq!(bridges.len(), 2, "{bridges:?}");
        assert_ne!(bridges[0].node_a, bridges[1].node_a, "極どうしはつながない");
    }

    /// A symbol with the "none" role takes no part in the circuit: it neither bridges its pins nor acts as a load.
    /// 役割「なし」のシンボルは回路に加わらない: ピンどうしをつながず、負荷にもならない。
    #[test]
    fn a_symbol_with_no_role_adds_no_element() {
        let defs = vec![
            role_symbol("u_src", "source", &[("+", 0.0), ("-", 10.0)], &["+", "-"]),
            role_symbol("u_lamp", "none", &[("1", 0.0), ("2", 10.0)], &[]),
        ];
        let sheet = sheet_with(vec![symbol("u_src", "BT1", 0.0, 100.0, "24"), symbol("u_lamp", "H1", 40.0, 100.0, "")]);
        let deck = build_deck(&sheet, &defs).unwrap();
        let h1 = match sheet.entities.values().find(|e| matches!(e, Entity::Symbol(s) if s.reference == "H1")) {
            Some(e) => e.id(),
            None => unreachable!(),
        };
        assert!(deck.component_elements.iter().all(|el| el.entity_id != h1), "{:?}", deck.component_elements);
    }

    /// Deck output is deterministic across runs, and the ground node 0 is the battery's minus pin.
    /// デッキ出力は毎回同一(決定的)で、GNDノード0はbatteryの負極ピンになる。
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
