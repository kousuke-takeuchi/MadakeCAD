//! 機能シートの下書き (function sheet draft)。
//!
//! 元の図 (読み取り専用の `Project`) の結線から、新しい1枚のシートを作る計画を立てる純関数の層。
//! ドキュメントは変えない (`route::plan_reroute` / `arrange::plan_arrange` と同じ型)。
//! 3段で作る: model (どのピンを持つか、ネットの種類と名前) → layout (箱・辺・順・置き場所) → wires (配線)。
//! この節は model: 入力の形 [`DraftRequest`]・その検査・元のネットリストの読み取り [`build_model`]。
//!
//! 仕様: `docs/superpowers/specs/2026-11-18-function-sheet-draft-design.md` §3。
// 次のタスク (layout・wires) が使うまで、model の項目は外から使われない。使われたらこの許可を外す。
#![allow(dead_code)]

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use crate::block::{self, BLOCK_SYMBOL_ID};
use crate::model::{Entity, Project, SymbolInstance};
use crate::symbol::{sheet_symbol_defs, DefIndex, SymbolDef};

// --- model

/// 1枚のシートに置ける列の最大数。
pub const MAX_COLUMNS: usize = 8;

/// 用紙 (横置き)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, schemars::JsonSchema)]
pub enum DraftPaper {
    #[default]
    A3,
    A4,
}

fn default_pitch() -> f64 {
    5.0
}

/// 下書きの入力。元の図のファイルの場所 (`source_path`) は含まない (呼び出し側が読む)。
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DraftRequest {
    /// 新しいシートの名前。空白だけは不可、開いているプロジェクトの既存のシート名とも重ならない。
    pub name: String,
    /// 列 (左から右)。1〜8列、各列は参照記号の並び (上から下)。参照記号は1回だけ。
    pub columns: Vec<Vec<String>>,
    /// 機器ごとにピンを限る (参照記号 → ピンの鍵の並び)。無い機器は使用中の全ピン。
    #[serde(default)]
    pub pins: BTreeMap<String, Vec<String>>,
    /// 電源のネット名 (元の図のネット)。
    #[serde(default)]
    pub power_nets: Vec<String>,
    /// 接地のネット名。省略時は、ネットの印を持つ全てのネット (電源に挙げたものを除く)。
    #[serde(default)]
    pub ground_nets: Option<Vec<String>>,
    /// 図の外へ続くネットの行き先 (ネット名 → 自由な文字)。
    #[serde(default)]
    pub destinations: BTreeMap<String, String>,
    /// 用紙。既定は A3。
    #[serde(default)]
    pub paper: DraftPaper,
    /// 箱のピン間隔 (mm)。2.5 か 5.0 (既定 5.0)。
    #[serde(default = "default_pitch")]
    pub pitch_mm: f64,
}

/// ネットの種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NetClass {
    /// 電源: 配線せず、持っているピンごとに印を付ける。
    Power,
    /// 接地: 配線せず、持っているピンごとに印を付ける。
    Ground,
    /// 内部: ネットの全てのピンがこのシートにあり、配線でつなぐ。
    Internal,
    /// 出ていく: ネットのピンの一部が他のシートにあり、タグを付ける。
    Leaving,
}

/// 箱が持つ1本のピン。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DraftPin {
    /// 元のネットリストでのピンの鍵 (箱の外に書く番号)。
    pub key: String,
    /// 元の定義でのピン名 (箱の中に書く名前。無ければ空)。
    pub name: String,
    /// [`DraftModel::nets`] の添字。
    pub net: usize,
}

/// 新しいシートに置く機器1つ (元の図で参照記号を共有する全てのシンボル)。
#[derive(Debug, Clone)]
pub(crate) struct DraftUnit {
    pub reference: String,
    /// 列の番号 (0始まり)。
    pub column: usize,
    /// 列の中の行 (0始まり。ピンを持たず外した機器を除いた順)。
    pub row: usize,
    /// 元の最初のシンボル (シート順、id順)。回転・値などの元になる。
    pub source: SymbolInstance,
    /// ブロックか (箱を作る)。ライブラリのシンボルなら偽 (同じシンボルを置く)。
    pub is_block: bool,
    /// 持つピン。元の定義の並び。
    pub pins: Vec<DraftPin>,
}

/// このシートが触れる元のネット1本。
#[derive(Debug, Clone)]
pub(crate) struct DraftNet {
    /// タグ・ラベル・印の名前。ネットラベルの名前、無ければ全ピンの自然順で最初の `REF.PIN`。
    pub name: String,
    pub class: NetClass,
    /// 接地のネットで、元の図が使う印のシンボルid (無ければ `None` = 同梱の接地の印)。
    pub ground_symbol: Option<String>,
    /// `destinations` で指定された行き先。
    pub dest: Option<String>,
}

/// model 段の結果。
#[derive(Debug, Clone)]
pub(crate) struct DraftModel {
    pub units: Vec<DraftUnit>,
    pub nets: Vec<DraftNet>,
    /// 外したピン (参照記号, ピンの鍵, 理由)。
    pub skipped: Vec<(String, String, String)>,
}

/// 自然順の比較: 数字の並びは数の大きさで比べる (`U2.9` は `U2.10` の前、`U2` は `U10` の前)。
pub(crate) fn natural_cmp(a: &str, b: &str) -> Ordering {
    fn chunks(s: &str) -> Vec<(bool, &str)> {
        let mut out = Vec::new();
        let mut start = 0;
        let mut kind: Option<bool> = None;
        for (i, c) in s.char_indices() {
            let digit = c.is_ascii_digit();
            if kind.is_some_and(|k| k != digit) {
                out.push((kind.unwrap(), &s[start..i]));
                start = i;
            }
            kind = Some(digit);
        }
        if let Some(k) = kind {
            out.push((k, &s[start..]));
        }
        out
    }
    fn number_cmp(a: &str, b: &str) -> Ordering {
        let (a, b) = (a.trim_start_matches('0'), b.trim_start_matches('0'));
        a.len().cmp(&b.len()).then_with(|| a.cmp(b))
    }
    let (ca, cb) = (chunks(a), chunks(b));
    for (x, y) in ca.iter().zip(cb.iter()) {
        let o = match (x.0, y.0) {
            (true, true) => number_cmp(x.1, y.1),
            (false, false) => x.1.cmp(y.1),
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
        };
        if o != Ordering::Equal {
            return o;
        }
    }
    ca.len().cmp(&cb.len()).then_with(|| a.cmp(b))
}

/// 元のネット1本 (シートを跨いで統合済み)。ネットの印・貫通のピンは含めない。
struct SrcNet {
    name: String,
    /// ネットラベル・印が持つ名前 (辞書順)。
    label_names: Vec<String>,
    /// 機器のピン (参照記号, ピンの鍵)。
    pins: BTreeSet<(String, String)>,
    /// このネットにある印のシンボルid。
    marks: BTreeSet<String>,
    /// 通っている区間ケーブルの芯 (`W1.D+` の形)。
    cores: Vec<String>,
}

/// 元の図を読んだ結果。
struct Source {
    nets: Vec<SrcNet>,
    /// ピン → ネット (ポートの接点で複数のネットに載るピンは最初のもの)。
    pin_net: BTreeMap<(String, String), usize>,
    /// ネット名 (ラベル名を先に、次に自動の名前) → ネット。
    by_name: BTreeMap<String, usize>,
    /// 参照記号 → 機器のシンボル (シート順、id順)。ネットの印・貫通は除く。
    instances: BTreeMap<String, Vec<SymbolInstance>>,
    /// 参照記号 → ピン (鍵, 名前, ポートか)。定義の並び、同じ鍵は最初のもの。
    unit_pins: BTreeMap<String, Vec<(String, String, bool)>>,
}

fn is_plain_device(def: &SymbolDef) -> bool {
    !def.is_net_mark() && !def.through
}

fn analyse(source: &Project) -> Source {
    let defs: Vec<Vec<SymbolDef>> = source.sheets.iter().map(sheet_symbol_defs).collect();

    let mut instances: BTreeMap<String, Vec<SymbolInstance>> = BTreeMap::new();
    let mut unit_pins: BTreeMap<String, Vec<(String, String, bool)>> = BTreeMap::new();
    for (sheet, sheet_defs) in source.sheets.iter().zip(&defs) {
        let index = DefIndex::new(sheet_defs);
        for entity in sheet.entities.values() {
            let Entity::Symbol(s) = entity else { continue };
            let Some(def) = index.get(s) else { continue };
            let reference = s.reference.trim();
            if !is_plain_device(&def) || reference.is_empty() {
                continue;
            }
            instances.entry(reference.to_string()).or_default().push(s.clone());
            let pins = unit_pins.entry(reference.to_string()).or_default();
            for p in &def.pins {
                if !pins.iter().any(|(k, _, _)| k == &p.number) {
                    pins.push((p.number.clone(), p.name.trim().to_string(), p.is_port()));
                }
            }
        }
    }

    let netlist = crate::xref::extract_netlist_project(source);
    let mut nets: Vec<SrcNet> = Vec::new();
    for pn in &netlist.nets {
        let mut pins = BTreeSet::new();
        let mut marks = BTreeSet::new();
        let mut cores = Vec::new();
        for m in &pn.members {
            let Some(sheet) = m.sheet_no.checked_sub(1).and_then(|i| source.sheets.get(i)) else { continue };
            let index = DefIndex::new(&defs[m.sheet_no - 1]);
            for p in &m.net.pins {
                let Some(Entity::Symbol(s)) = sheet.entities.get(&p.entity_id) else { continue };
                let Some(def) = index.get(s) else { continue };
                if def.is_net_mark() {
                    marks.insert(s.symbol_id.clone());
                } else if !def.through {
                    pins.insert((p.reference.clone(), p.pin.clone()));
                }
            }
            for c in &m.net.cable_cores {
                cores.push(format!("{}.{}", c.reference, c.core));
            }
        }
        cores.sort();
        cores.dedup();
        let mut sorted: Vec<&(String, String)> = pins.iter().collect();
        sorted.sort_by(|a, b| natural_cmp(&a.0, &b.0).then_with(|| natural_cmp(&a.1, &b.1)));
        let name = match pn.label_names.first() {
            Some(l) => l.clone(),
            None => match sorted.first() {
                Some((r, p)) => format!("{r}.{p}"),
                None => pn.name.clone(),
            },
        };
        nets.push(SrcNet { name, label_names: pn.label_names.clone(), pins, marks, cores });
    }

    let mut pin_net = BTreeMap::new();
    let mut by_name = BTreeMap::new();
    for (i, n) in nets.iter().enumerate() {
        for pin in &n.pins {
            pin_net.entry(pin.clone()).or_insert(i);
        }
        for l in &n.label_names {
            by_name.entry(l.clone()).or_insert(i);
        }
    }
    for (i, n) in nets.iter().enumerate() {
        by_name.entry(n.name.clone()).or_insert(i);
    }
    Source { nets, pin_net, by_name, instances, unit_pins }
}

impl DraftRequest {
    /// 元の図を見なくてもわかる誤りを調べる。
    fn check_shape(&self, open: &Project) -> Result<(), String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("The sheet name is blank: give the new sheet a name".to_string());
        }
        if open.sheets.iter().any(|s| s.name.trim() == name) {
            return Err(format!("The sheet name '{name}' is already used in the open project"));
        }
        if self.columns.is_empty() || self.columns.len() > MAX_COLUMNS {
            return Err(format!(
                "columns must hold 1 to {MAX_COLUMNS} columns, got {}",
                self.columns.len()
            ));
        }
        let mut seen = BTreeSet::new();
        for (c, column) in self.columns.iter().enumerate() {
            for r in column {
                let r = r.trim();
                if !seen.insert(r.to_string()) {
                    return Err(format!("The reference {r} is listed more than once in columns (column {})", c + 1));
                }
            }
        }
        if self.pitch_mm != 2.5 && self.pitch_mm != 5.0 {
            return Err(format!("pitch_mm must be 2.5 or 5.0, got {}", self.pitch_mm));
        }
        Ok(())
    }
}

/// ネット名の一覧を元のネットの番号にする。無ければ、どの項目の名前かを挙げて誤りにする。
fn resolve_nets<'a>(
    src: &Source,
    option: &str,
    names: impl Iterator<Item = &'a String>,
) -> Result<BTreeSet<usize>, String> {
    let mut out = BTreeSet::new();
    for n in names {
        let Some(&i) = src.by_name.get(n.trim()) else {
            return Err(format!("The net '{n}' named in {option} does not exist in the source"));
        };
        out.insert(i);
    }
    Ok(out)
}

/// 元の図のネットリストを読み、各機器が持つピンと、各ネットの種類・名前を決める。
///
/// `source` は読み取り専用の元の図 (全シートのプロジェクトネットリストを使う)、`open` は今開いているプロジェクト。
/// 誤りがあれば何も作らず、参照記号・ピン・ネットの名前を挙げた英語の文で返す。
pub(crate) fn build_model(source: &Project, open: &Project, req: &DraftRequest) -> Result<DraftModel, String> {
    req.check_shape(open)?;
    let src = analyse(source);

    // 列の中の参照記号が元の図にあるか
    for column in &req.columns {
        for r in column {
            if !src.instances.contains_key(r.trim()) {
                return Err(format!("The reference {} does not exist in the source", r.trim()));
            }
        }
    }
    let listed: BTreeSet<&str> = req.columns.iter().flatten().map(|r| r.trim()).collect();
    for r in req.pins.keys() {
        if !listed.contains(r.trim()) {
            return Err(format!("pins names the reference {r}, which is not in columns"));
        }
    }

    // ネットの指定
    let power = resolve_nets(&src, "power_nets", req.power_nets.iter())?;
    let ground_listed = match &req.ground_nets {
        Some(list) => Some(resolve_nets(&src, "ground_nets", list.iter())?),
        None => None,
    };
    let dest_nets = resolve_nets(&src, "destinations", req.destinations.keys())?;
    if let Some(g) = &ground_listed {
        if let Some(&both) = power.intersection(g).next() {
            return Err(format!(
                "The net '{}' is listed in both power_nets and ground_nets",
                src.nets[both].name
            ));
        }
    }
    let ground: BTreeSet<usize> = match ground_listed {
        Some(g) => g,
        None => src
            .nets
            .iter()
            .enumerate()
            .filter(|(i, n)| !n.marks.is_empty() && !power.contains(i))
            .map(|(i, _)| i)
            .collect(),
    };
    let dest_text: BTreeMap<usize, String> = req
        .destinations
        .iter()
        .filter_map(|(k, v)| src.by_name.get(k.trim()).map(|&i| (i, v.clone())))
        .collect();
    debug_assert_eq!(dest_text.len(), dest_nets.len());

    // 機器ごとに持つピンを決める
    struct Carried {
        reference: String,
        column: usize,
        pins: Vec<(String, String, usize)>,
    }
    let mut carried: Vec<Carried> = Vec::new();
    let mut skipped: Vec<(String, String, String)> = Vec::new();
    for (column, refs) in req.columns.iter().enumerate() {
        for r in refs {
            let reference = r.trim().to_string();
            let all = &src.unit_pins[&reference];
            let wanted: Option<BTreeSet<&str>> = req.pins.get(&reference).map(|l| l.iter().map(|k| k.trim()).collect());
            if let Some(w) = &wanted {
                for k in w {
                    if !all.iter().any(|(key, _, _)| key == k) {
                        return Err(format!("The unit {reference} has no pin '{k}'"));
                    }
                }
            }
            let mut pins = Vec::new();
            for (key, name, is_port) in all {
                if wanted.as_ref().is_some_and(|w| !w.contains(key.as_str())) {
                    continue;
                }
                let net = src.pin_net.get(&(reference.clone(), key.clone())).copied();
                let in_use = match net {
                    None => {
                        skipped.push((reference.clone(), key.clone(), "pin is not connected".to_string()));
                        continue;
                    }
                    Some(i) => src.nets[i].pins.len() >= 2 || power.contains(&i) || ground.contains(&i),
                };
                if !in_use {
                    skipped.push((reference.clone(), key.clone(), "net has one pin".to_string()));
                    continue;
                }
                let net = net.unwrap();
                if *is_port {
                    return Err(format!(
                        "The pin {reference}.{key} is a port with contacts, which a function sheet cannot draw"
                    ));
                }
                if let Some(core) = src.nets[net].cores.first() {
                    return Err(format!(
                        "The net '{}' of the pin {reference}.{key} runs through the cable core {core}, which a function sheet cannot draw",
                        src.nets[net].name
                    ));
                }
                pins.push((key.clone(), name.clone(), net));
            }
            carried.push(Carried { reference, column, pins });
        }
    }
    if carried.iter().all(|c| c.pins.is_empty()) {
        return Err("No pin is left to draw: every pin of the listed units is unused or not connected".to_string());
    }

    // 開いているプロジェクトとの重なり
    for c in carried.iter().filter(|c| !c.pins.is_empty()) {
        let first = &src.instances[&c.reference][0];
        if first.symbol_id == BLOCK_SYMBOL_ID {
            for sheet in &open.sheets {
                for e in sheet.entities.values() {
                    let Entity::Symbol(s) = e else { continue };
                    if !block::is_view(s) || s.reference.trim() != c.reference {
                        continue;
                    }
                    let shown = block::pin_keys(&s.attrs);
                    if let Some((key, _, _)) = c.pins.iter().find(|(k, _, _)| shown.contains(k)) {
                        return Err(format!(
                            "The pin {}.{key} is already shown by a view on the sheet '{}' of the open project",
                            c.reference, sheet.name
                        ));
                    }
                }
            }
        } else if let Some(sheet) = open.sheets.iter().find(|sheet| {
            sheet
                .entities
                .values()
                .any(|e| matches!(e, Entity::Symbol(s) if s.reference.trim() == c.reference))
        }) {
            return Err(format!(
                "The library symbol {} is already on the sheet '{}' of the open project, and may appear on one function sheet only",
                c.reference, sheet.name
            ));
        }
    }

    // 触れるネットを、最初に現れた順に並べる
    let carried_pins: BTreeSet<(String, String)> = carried
        .iter()
        .flat_map(|c| c.pins.iter().map(|(k, _, _)| (c.reference.clone(), k.clone())))
        .collect();
    let mut nets: Vec<DraftNet> = Vec::new();
    let mut model_index: BTreeMap<usize, usize> = BTreeMap::new();
    let mut units: Vec<DraftUnit> = Vec::new();
    let mut rows: BTreeMap<usize, usize> = BTreeMap::new();
    for c in carried.iter().filter(|c| !c.pins.is_empty()) {
        let mut pins = Vec::new();
        for (key, name, net) in &c.pins {
            let idx = *model_index.entry(*net).or_insert_with(|| {
                let n = &src.nets[*net];
                let class = if power.contains(net) {
                    NetClass::Power
                } else if ground.contains(net) {
                    NetClass::Ground
                } else if n.pins.iter().all(|p| carried_pins.contains(p)) {
                    NetClass::Internal
                } else {
                    NetClass::Leaving
                };
                nets.push(DraftNet {
                    name: n.name.clone(),
                    class,
                    ground_symbol: if class == NetClass::Ground { n.marks.iter().next().cloned() } else { None },
                    dest: dest_text.get(net).cloned(),
                });
                nets.len() - 1
            });
            pins.push(DraftPin { key: key.clone(), name: name.clone(), net: idx });
        }
        let row = rows.entry(c.column).or_insert(0);
        let source_inst = src.instances[&c.reference][0].clone();
        units.push(DraftUnit {
            reference: c.reference.clone(),
            column: c.column,
            row: *row,
            is_block: source_inst.symbol_id == BLOCK_SYMBOL_ID,
            source: source_inst,
            pins,
        });
        *row += 1;
    }
    Ok(DraftModel { units, nets, skipped })
}

/// テスト用: 元の図の見本 (`tests/fixtures/draft-source.mdkproj`) を読む。
/// 機器 U1 U2 TB1 M1 R1、ネット P24X GND SIG_A SIG_B と、名前のない2本、ピンが1本だけのネットが1本ある。
#[cfg(test)]
pub(crate) fn fixture_source() -> Project {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/draft-source.mdkproj");
    crate::io::load_project(&path).expect("the draft fixture loads")
}

#[cfg(test)]
mod model_tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::{NetLabel, Wire};
    use crate::verify::Severity;
    use crate::netlist::pin_positions;
    use crate::symbol::{builtin_symbols, PinDir};
    use uuid::Uuid;

    // ---- the fixture is built in code (see `write_the_draft_fixture`) so that it can be rebuilt and checked

    const SIZE: &str = r#"{"w":30,"h":40}"#;

    fn block(reference: &str, x: f64, y: f64, pins_json: &str) -> SymbolInstance {
        let Entity::Symbol(mut s) = crate::block::test_block(reference, "", x, y, pins_json) else { unreachable!() };
        s.attrs.insert(crate::block::BLOCK_SIZE_ATTR.into(), SIZE.into());
        s
    }

    fn plain(symbol: &str, reference: &str, value: &str, at: Point) -> SymbolInstance {
        SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: symbol.into(),
            at,
            rotation: 0,
            mirror: false,
            reference: reference.into(),
            value: value.into(),
            attrs: Default::default(),
        }
    }

    fn wire(points: &[Point]) -> Entity {
        Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: points.to_vec(),
            color: "black".into(),
            sq: 0.75,
            length_m: None,
            length_source: Default::default(),
            part_no: None,
            net: None,
            dashed: false,
            weight_mm: None,
        })
    }

    fn label(name: &str, at: Point) -> Entity {
        Entity::NetLabel(NetLabel { id: Uuid::new_v4(), at, name: name.into(), rotation: 0, attrs: Default::default() })
    }

    /// Pin position and the point 2.5 mm outside it.
    fn pin_and_stub(inst: &SymbolInstance, key: &str) -> (Point, Point) {
        let defs = builtin_symbols();
        let index = DefIndex::new(&defs);
        let def = index.get(inst).expect("symbol has a definition");
        let at = pin_positions(inst, &def).into_iter().find(|(k, _)| k == key).expect("pin exists").1;
        let dir = def.pins.iter().find(|p| p.number == key).expect("pin exists").dir;
        let end = match dir {
            PinDir::Left => Point::new(at.x - 2.5, at.y),
            PinDir::Right => Point::new(at.x + 2.5, at.y),
            PinDir::Up => Point::new(at.x, at.y - 2.5),
            PinDir::Down => Point::new(at.x, at.y + 2.5),
        };
        (at, end)
    }

    /// A short stub from the pin that ends in a net label.
    fn labelled(out: &mut Vec<Entity>, inst: &SymbolInstance, key: &str, name: &str) {
        let (at, end) = pin_and_stub(inst, key);
        out.push(wire(&[at, end]));
        out.push(label(name, end));
    }

    /// A short stub from the pin that ends in a power rail mark of the given name.
    fn marked(out: &mut Vec<Entity>, inst: &SymbolInstance, key: &str, name: &str) {
        let (at, end) = pin_and_stub(inst, key);
        out.push(wire(&[at, end]));
        out.push(Entity::Symbol(plain("power_rail", "", name, end)));
    }

    const U1_PINS: &str = r#"{"left":[{"no":"1","name":"24V"},{"no":"2","name":"GND"},{"no":"4","name":"IN1"}],"right":[{"no":"3","name":"OUT"},{"no":"5","name":"TX"},{"no":"6","name":"SPARE"}]}"#;
    const U2_PINS: &str = r#"{"left":[{"no":"1","name":"VCC"},{"no":"2","name":"GND"},{"no":"3","name":"IN_A"},{"no":"4","name":"IN_B"}],"right":[{"no":"9","name":"X1"},{"no":"10","name":"X2"}]}"#;
    const TB1_PINS: &str = r#"{"left":[{"no":"1","name":"L+"},{"no":"2","name":"S"}],"right":[{"no":"3","name":"NC"}]}"#;
    const M1_PINS: &str = r#"{"left":[{"no":"1","name":"U"},{"no":"2","name":"GND"}]}"#;

    fn build_fixture() -> Project {
        let mut project = Project::new("draft-source");
        let u1 = block("U1", 40.0, 60.0, U1_PINS);
        let m1 = block("M1", 110.0, 60.0, M1_PINS);
        let u2 = block("U2", 180.0, 60.0, U2_PINS);
        let tb1 = block("TB1", 300.0, 60.0, TB1_PINS);
        let r1 = plain("resistor", "R1", "10k", Point::new(350.0, 150.0));
        let mut e: Vec<Entity> = Vec::new();
        // power and ground by marks
        for (inst, key) in [(&u1, "1"), (&u2, "1"), (&tb1, "1")] {
            marked(&mut e, inst, key, "P24X");
        }
        for (inst, key) in [(&u1, "2"), (&u2, "2"), (&m1, "2"), (&r1, "2")] {
            marked(&mut e, inst, key, "GND");
        }
        // labelled nets
        for (inst, key) in [(&u1, "5"), (&u2, "3"), (&r1, "1")] {
            labelled(&mut e, inst, key, "SIG_A");
        }
        for (inst, key) in [(&u1, "4"), (&u2, "4"), (&tb1, "2")] {
            labelled(&mut e, inst, key, "SIG_B");
        }
        // a net of one pin
        labelled(&mut e, &u1, "6", "SPARE");
        // an unlabelled net by a wire: U1.3 - M1.1
        let (a, _) = pin_and_stub(&u1, "3");
        let (b, _) = pin_and_stub(&m1, "1");
        e.push(if (a.y - b.y).abs() < 1e-9 {
            wire(&[a, b])
        } else {
            let mid = (a.x + b.x) / 2.0;
            wire(&[a, Point::new(mid, a.y), Point::new(mid, b.y), b])
        });
        // an unlabelled net between two pins of U2 (9 and 10)
        let (p9, _) = pin_and_stub(&u2, "9");
        let (p10, _) = pin_and_stub(&u2, "10");
        let xb = p9.x + 7.5;
        e.push(wire(&[p9, Point::new(xb, p9.y), Point::new(xb, p10.y), p10]));
        for inst in [u1, m1, u2, tb1, r1] {
            e.push(Entity::Symbol(inst));
        }
        for entity in e {
            project.sheets[0].entities.insert(entity.id(), entity);
        }
        project
    }

    /// Rebuilds the draft fixture file; ignored by default and run by hand only.
    /// 下書きの見本の元の図のファイルを作り直す。普段は走らせず、手で動かしたときだけ作り直す。
    #[test]
    #[ignore]
    fn write_the_draft_fixture() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/draft-source.mdkproj");
        crate::io::save_project(&path, &build_fixture()).unwrap();
    }

    // ---- helpers for the tests

    fn request(columns: &[&[&str]]) -> DraftRequest {
        DraftRequest {
            name: "2 Control".into(),
            columns: columns.iter().map(|c| c.iter().map(|r| r.to_string()).collect()).collect(),
            pins: BTreeMap::new(),
            power_nets: vec![],
            ground_nets: None,
            destinations: BTreeMap::new(),
            paper: DraftPaper::A3,
            pitch_mm: 5.0,
        }
    }

    fn open() -> Project {
        Project::new("open")
    }

    fn model(columns: &[&[&str]]) -> DraftModel {
        build_model(&fixture_source(), &open(), &request(columns)).expect("model builds")
    }

    fn refuse(source: &Project, open: &Project, req: &DraftRequest) -> String {
        build_model(source, open, req).expect_err("the draft is refused")
    }

    fn unit<'a>(m: &'a DraftModel, reference: &str) -> &'a DraftUnit {
        m.units.iter().find(|u| u.reference == reference).expect("unit exists")
    }

    fn keys(u: &DraftUnit) -> Vec<&str> {
        u.pins.iter().map(|p| p.key.as_str()).collect()
    }

    fn net_of<'a>(m: &'a DraftModel, reference: &str, key: &str) -> &'a DraftNet {
        let p = unit(m, reference).pins.iter().find(|p| p.key == key).expect("pin carried");
        &m.nets[p.net]
    }

    fn net_named<'a>(m: &'a DraftModel, name: &str) -> &'a DraftNet {
        m.nets.iter().find(|n| n.name == name).expect("net exists")
    }

    // ---- the fixture

    /// The invented source drawing loads and verification finds no error in it.
    /// 見本として作った元の図は読み込めて、検証しても間違い(エラー)が1件も出ない。
    #[test]
    fn the_draft_fixture_loads_and_verifies_clean() {
        let project = fixture_source();
        let errors: Vec<_> = crate::verify::verify_project(&project)
            .into_iter()
            .filter(|d| d.severity == Severity::Error)
            .collect();
        assert!(errors.is_empty(), "{errors:?}");
    }

    // ---- which pins a view carries

    /// A unit carries only the pins whose net joins two or more pins (or is a power or ground net).
    /// 機器の箱が持つのは、ほかのピンと結ばれているネットのピンだけで、つながりのないピンは持たない。
    #[test]
    fn a_view_carries_only_the_pins_in_use() {
        let m = model(&[&["U1"]]);
        assert_eq!(keys(unit(&m, "U1")), ["1", "2", "4", "3", "5"]);
        assert_eq!(unit(&m, "U1").pins[0].name, "24V");
    }

    /// A pin on a net of one pin is left out and the reason is recorded.
    /// 相手のいない1本だけのネットのピンは外し、外した理由を残す。
    #[test]
    fn a_pin_on_a_one_pin_net_is_skipped_with_a_reason() {
        let m = model(&[&["U1"]]);
        assert!(m.skipped.contains(&("U1".into(), "6".into(), "net has one pin".into())), "{:?}", m.skipped);
    }

    /// A pin that is on no net at all is left out too, with its own reason.
    /// どの配線にもつながっていないピンも外し、「つながっていない」という理由を残す。
    #[test]
    fn a_pin_on_no_net_is_skipped_with_a_reason() {
        let m = model(&[&["TB1"]]);
        assert!(m.skipped.contains(&("TB1".into(), "3".into(), "pin is not connected".into())), "{:?}", m.skipped);
    }

    /// The pins option limits a unit to the listed pins.
    /// pins の指定があると、その機器は挙げたピンだけを持つ。
    #[test]
    fn the_pins_option_limits_a_view() {
        let mut req = request(&[&["U1"]]);
        req.pins.insert("U1".into(), vec!["1".into(), "5".into()]);
        let m = build_model(&fixture_source(), &open(), &req).unwrap();
        assert_eq!(keys(unit(&m, "U1")), ["1", "5"]);
    }

    /// A unit that has no pin to draw is dropped from the sheet, and its pins are listed as skipped.
    /// 描くピンが1本もない機器はシートに置かず、そのピンは外したものとして残す。
    #[test]
    fn a_unit_without_a_drawable_pin_is_dropped() {
        let mut req = request(&[&["U1"], &["TB1"]]);
        req.pins.insert("TB1".into(), vec!["3".into()]);
        let m = build_model(&fixture_source(), &open(), &req).unwrap();
        assert!(m.units.iter().all(|u| u.reference != "TB1"));
        assert_eq!(m.skipped.iter().filter(|s| s.0 == "TB1").count(), 1);
    }

    /// Units get the column and the row they were listed in.
    /// 機器には、並べて書いたとおりの列と行が付く。
    #[test]
    fn units_keep_their_column_and_row() {
        let m = model(&[&["U1", "M1"], &["U2"]]);
        let at = |r: &str| (unit(&m, r).column, unit(&m, r).row);
        assert_eq!((at("U1"), at("M1"), at("U2")), ((0, 0), (0, 1), (1, 0)));
    }

    /// A block unit is a block, and a library symbol (the resistor) stays a symbol.
    /// ブロックの機器は箱にし、ライブラリの部品(抵抗)は部品のまま置く。
    #[test]
    fn a_library_symbol_stays_a_symbol() {
        let m = model(&[&["U1", "R1"]]);
        assert!(unit(&m, "U1").is_block);
        assert!(!unit(&m, "R1").is_block);
        assert_eq!(unit(&m, "R1").source.symbol_id, "resistor");
    }

    // ---- net classes

    /// A net whose pins are all on the sheet is internal.
    /// ネットの全てのピンがこのシートにあるなら、内部のネットになる。
    #[test]
    fn a_net_with_all_pins_on_the_sheet_is_internal() {
        let m = model(&[&["U1"], &["M1"]]);
        assert_eq!(net_of(&m, "U1", "3").class, NetClass::Internal);
        assert_eq!(net_of(&m, "M1", "1").class, NetClass::Internal);
        let all = model(&[&["U1", "U2", "TB1"], &["R1"]]);
        assert_eq!(net_named(&all, "SIG_A").class, NetClass::Internal);
        assert_eq!(net_named(&all, "SIG_B").class, NetClass::Internal);
    }

    /// A net with pins on other sheets leaves the sheet.
    /// ネットのピンの一部がほかのシートにあるなら、シートから出ていくネットになる。
    #[test]
    fn a_net_with_pins_elsewhere_is_leaving() {
        let m = model(&[&["U1"]]);
        assert_eq!(net_named(&m, "SIG_A").class, NetClass::Leaving);
        assert_eq!(net_of(&m, "U1", "3").class, NetClass::Leaving);
    }

    /// Nets listed in power_nets and ground_nets get those classes.
    /// power_nets と ground_nets に挙げたネットは、電源・接地の種類になる。
    #[test]
    fn listed_power_and_ground_nets_get_their_class() {
        let mut req = request(&[&["U1"], &["U2"]]);
        req.power_nets = vec!["P24X".into()];
        req.ground_nets = Some(vec!["GND".into()]);
        let m = build_model(&fixture_source(), &open(), &req).unwrap();
        assert_eq!(net_named(&m, "P24X").class, NetClass::Power);
        assert_eq!(net_named(&m, "GND").class, NetClass::Ground);
        assert_eq!(net_named(&m, "P24X").ground_symbol, None);
    }

    /// Without ground_nets, every net that carries a net mark is ground, except the ones listed as power; the mark the source uses is remembered.
    /// ground_nets を省くと、印のあるネットは全て接地になる(電源に挙げたネットは除く)。元の図が使う印の種類も覚える。
    #[test]
    fn ground_nets_default_to_nets_with_a_mark() {
        let mut req = request(&[&["U1"], &["U2"]]);
        req.power_nets = vec!["P24X".into()];
        let m = build_model(&fixture_source(), &open(), &req).unwrap();
        assert_eq!(net_named(&m, "GND").class, NetClass::Ground);
        assert_eq!(net_named(&m, "GND").ground_symbol.as_deref(), Some("power_rail"));
        assert_eq!(net_named(&m, "P24X").class, NetClass::Power);
        let none = model(&[&["U1"], &["U2"]]);
        assert_eq!(net_named(&none, "P24X").class, NetClass::Ground);
    }

    /// A pin that is alone on a net with a mark counts as in use, because the mark is drawn for it.
    /// 印の付いたネットに自分のピンしかなくても、そのピンには印を描くので「使われている」として持つ。
    #[test]
    fn a_ground_pin_alone_on_its_net_is_in_use() {
        let mut project = fixture_source();
        let g = block("G1", 60.0, 200.0, r#"{"left":[{"no":"1","name":"E"}]}"#);
        let mut e = Vec::new();
        marked(&mut e, &g, "1", "GNDX");
        e.push(Entity::Symbol(g));
        for x in e {
            project.sheets[0].entities.insert(x.id(), x);
        }
        let m = build_model(&project, &open(), &request(&[&["G1"]])).unwrap();
        assert_eq!(keys(&m.units[0]), ["1"]);
        assert_eq!(m.nets[0].class, NetClass::Ground);
    }

    /// The destinations option is kept on the net it names.
    /// destinations に書いた行き先は、名前を挙げたネットに付く。
    #[test]
    fn a_destination_is_kept_on_its_net() {
        let mut req = request(&[&["U1"]]);
        req.destinations.insert("SIG_A".into(), "cabinet B".into());
        let m = build_model(&fixture_source(), &open(), &req).unwrap();
        assert_eq!(net_named(&m, "SIG_A").dest.as_deref(), Some("cabinet B"));
        assert_eq!(net_named(&m, "SIG_B").dest, None);
    }

    // ---- names

    /// A labelled net keeps the name of its label.
    /// 名前(ネットラベル)のあるネットは、そのラベルの名前を使う。
    #[test]
    fn a_labelled_net_keeps_its_label_name() {
        let m = model(&[&["U1"]]);
        assert_eq!(net_of(&m, "U1", "5").name, "SIG_A");
        assert_eq!(net_of(&m, "U1", "4").name, "SIG_B");
    }

    /// An unlabelled net is named after its first pin over the whole source net, so every sheet that shows it computes the same name.
    /// 名前のないネットは、元の図のネット全体の最初のピンで名前を決めるので、どのシートから下書きしても同じ名前になる。
    #[test]
    fn unlabelled_net_name_is_the_same_on_every_sheet() {
        for columns in [vec![vec!["U1"]], vec![vec!["M1"]], vec![vec!["U1"], vec!["M1"]]] {
            let cols: Vec<&[&str]> = columns.iter().map(|c| c.as_slice()).collect();
            let m = model(&cols);
            let pin = if columns.iter().flatten().any(|r| *r == "U1") { ("U1", "3") } else { ("M1", "1") };
            assert_eq!(net_of(&m, pin.0, pin.1).name, "M1.1", "{columns:?}");
        }
    }

    /// Pin numbers are ordered by size, not letter by letter: U2.9 comes before U2.10, and U2 before U10.
    /// ピン番号の並びは数の大きさで決める。U2.9 は U2.10 の前、U2 は U10 の前に来る。
    #[test]
    fn natural_order_puts_nine_before_ten() {
        let m = model(&[&["U2"]]);
        assert_eq!(net_of(&m, "U2", "10").name, "U2.9");
        assert_eq!(natural_cmp("U2.9", "U2.10"), Ordering::Less);
        assert_eq!(natural_cmp("U2", "U10"), Ordering::Less);
        assert_eq!(natural_cmp("U2", "U2.1"), Ordering::Less);
    }

    // ---- request

    /// A request reads from JSON with its defaults, and refuses a field it does not know.
    /// 下書きの依頼はJSONから読め、省いた項目は既定値になり、知らない項目があれば読み込みを断る。
    #[test]
    fn a_request_reads_with_defaults_and_refuses_unknown_fields() {
        let r: DraftRequest = serde_json::from_str(r#"{"name":"A","columns":[["U1"]]}"#).unwrap();
        assert_eq!((r.paper, r.pitch_mm, r.ground_nets.is_none()), (DraftPaper::A3, 5.0, true));
        assert!(serde_json::from_str::<DraftRequest>(r#"{"name":"A","columns":[["U1"]],"source_path":"x"}"#).is_err());
        let a4: DraftRequest = serde_json::from_str(r#"{"name":"A","columns":[["U1"]],"paper":"A4"}"#).unwrap();
        assert_eq!(a4.paper, DraftPaper::A4);
    }

    // ---- refusals

    /// A reference the source does not have is refused, and the message names it.
    /// 元の図にない参照記号を挙げると断り、その参照記号を文に入れる。
    #[test]
    fn an_unknown_reference_is_refused() {
        let e = refuse(&fixture_source(), &open(), &request(&[&["U1", "Q9"]]));
        assert!(e.contains("Q9"), "{e}");
    }

    /// A reference listed twice is refused.
    /// 同じ参照記号を2回挙げると断る。
    #[test]
    fn a_reference_listed_twice_is_refused() {
        let e = refuse(&fixture_source(), &open(), &request(&[&["U1"], &["U1"]]));
        assert!(e.contains("U1") && e.contains("more than once"), "{e}");
    }

    /// A pin key the unit does not have is refused.
    /// その機器にないピンの番号を pins に挙げると断る。
    #[test]
    fn an_unknown_pin_key_is_refused() {
        let mut req = request(&[&["U1"]]);
        req.pins.insert("U1".into(), vec!["77".into()]);
        let e = refuse(&fixture_source(), &open(), &req);
        assert!(e.contains("U1") && e.contains("77"), "{e}");
    }

    /// A pins entry for a reference that is not in columns is refused.
    /// columns にない参照記号を pins に挙げると断る。
    #[test]
    fn a_pins_entry_for_a_reference_not_in_columns_is_refused() {
        let mut req = request(&[&["U1"]]);
        req.pins.insert("U2".into(), vec!["1".into()]);
        assert!(refuse(&fixture_source(), &open(), &req).contains("U2"));
    }

    /// A net the source does not have is refused in power_nets, in ground_nets and in destinations, naming the option and the net.
    /// 元の図にないネット名は、power_nets・ground_nets・destinations のどれに書いても断り、項目名とネット名を文に入れる。
    #[test]
    fn an_unknown_net_is_refused_in_each_option() {
        let mut a = request(&[&["U1"]]);
        a.power_nets = vec!["NOPE_P".into()];
        let mut b = request(&[&["U1"]]);
        b.ground_nets = Some(vec!["NOPE_G".into()]);
        let mut c = request(&[&["U1"]]);
        c.destinations.insert("NOPE_D".into(), "x".into());
        for (req, option, net) in [(a, "power_nets", "NOPE_P"), (b, "ground_nets", "NOPE_G"), (c, "destinations", "NOPE_D")] {
            let e = refuse(&fixture_source(), &open(), &req);
            assert!(e.contains(option) && e.contains(net), "{e}");
        }
    }

    /// A net listed as both power and ground is refused.
    /// 同じネットを電源と接地の両方に挙げると断る。
    #[test]
    fn a_net_in_both_power_and_ground_is_refused() {
        let mut req = request(&[&["U1"]]);
        req.power_nets = vec!["GND".into()];
        req.ground_nets = Some(vec!["GND".into()]);
        let e = refuse(&fixture_source(), &open(), &req);
        assert!(e.contains("GND") && e.contains("both"), "{e}");
    }

    /// A sheet name that the open project already uses is refused.
    /// 開いているプロジェクトにすでにあるシート名を付けると断る。
    #[test]
    fn a_sheet_name_in_use_is_refused() {
        let mut req = request(&[&["U1"]]);
        req.name = "Sheet1".into();
        assert!(refuse(&fixture_source(), &open(), &req).contains("Sheet1"));
    }

    /// A blank sheet name is refused.
    /// シート名が空(空白だけも同じ)だと断る。
    #[test]
    fn a_blank_name_is_refused() {
        let mut req = request(&[&["U1"]]);
        req.name = "   ".into();
        assert!(refuse(&fixture_source(), &open(), &req).contains("blank"));
    }

    /// No column, or more than eight columns, is refused.
    /// 列が0個、または9個以上だと断る。
    #[test]
    fn zero_or_nine_columns_are_refused() {
        let none = request(&[]);
        assert!(refuse(&fixture_source(), &open(), &none).contains("columns"));
        let nine = request(&[&["U1"], &["U2"], &["TB1"], &["M1"], &["R1"], &[], &[], &[], &[]]);
        assert!(refuse(&fixture_source(), &open(), &nine).contains("columns"));
        let eight = request(&[&["U1"], &["U2"], &["TB1"], &["M1"], &["R1"], &[], &[], &[]]);
        assert!(build_model(&fixture_source(), &open(), &eight).is_ok());
    }

    /// A pin pitch other than 2.5 or 5.0 is refused.
    /// ピン間隔が 2.5 でも 5.0 でもないと断る。
    #[test]
    fn a_pitch_other_than_two_values_is_refused() {
        let mut req = request(&[&["U1"]]);
        req.pitch_mm = 4.0;
        assert!(refuse(&fixture_source(), &open(), &req).contains("pitch_mm"));
        req.pitch_mm = 2.5;
        assert!(build_model(&fixture_source(), &open(), &req).is_ok());
    }

    /// A pin that a view in the open project already shows is refused, naming the pin.
    /// 開いているプロジェクトの部分表示がすでに出しているピンを使うと断り、そのピンを文に入れる。
    #[test]
    fn a_pin_already_shown_by_a_view_is_refused() {
        let mut project = open();
        let view = crate::block::test_view("U1", "", 20.0, 20.0, r#"{"left":[{"no":"4","name":"IN1"}]}"#);
        project.sheets[0].entities.insert(view.id(), view);
        let e = refuse(&fixture_source(), &project, &request(&[&["U1"]]));
        assert!(e.contains("U1.4"), "{e}");
    }

    /// A view of the same unit that shows other pins does not stop the draft.
    /// 同じ機器の部分表示でも、別のピンを出しているだけなら下書きを断らない。
    #[test]
    fn a_view_of_other_pins_does_not_stop_the_draft() {
        let mut project = open();
        let view = crate::block::test_view("U1", "", 20.0, 20.0, r#"{"left":[{"no":"9","name":"Z"}]}"#);
        project.sheets[0].entities.insert(view.id(), view);
        assert!(build_model(&fixture_source(), &project, &request(&[&["U1"]])).is_ok());
    }

    /// A library symbol that the open project already has is refused.
    /// 開いているプロジェクトにすでに同じ参照記号のライブラリの部品があると断る。
    #[test]
    fn a_library_symbol_already_on_a_function_sheet_is_refused() {
        let mut project = open();
        let r = Entity::Symbol(plain("resistor", "R1", "10k", Point::new(50.0, 50.0)));
        project.sheets[0].entities.insert(r.id(), r);
        let e = refuse(&fixture_source(), &project, &request(&[&["R1"]]));
        assert!(e.contains("R1"), "{e}");
    }

    /// When no unit has a pin to carry, the draft is refused.
    /// 描くピンがどの機器にも1本もないときは断る。
    #[test]
    fn no_carried_pin_is_refused() {
        let mut req = request(&[&["TB1"]]);
        req.pins.insert("TB1".into(), vec!["3".into()]);
        assert!(refuse(&fixture_source(), &open(), &req).contains("No pin"));
    }

    /// A carried pin that is a port with contacts is refused.
    /// 描くピンが接点をもつポートのピンだと断る。
    #[test]
    fn a_port_with_contacts_is_refused() {
        let mut project = fixture_source();
        let pins = r#"{"right":[{"no":"1","name":"USB","contacts":["VBUS","GND"]}]}"#;
        let pins2 = r#"{"left":[{"no":"1","name":"USB","contacts":["VBUS","GND"]}]}"#;
        let (a, b) = (block("PA", 60.0, 200.0, pins), block("PB", 130.0, 200.0, pins2));
        let wire_ab = wire(&[pin_and_stub(&a, "1").0, pin_and_stub(&b, "1").0]);
        for x in [wire_ab, Entity::Symbol(a), Entity::Symbol(b)] {
            project.sheets[0].entities.insert(x.id(), x);
        }
        let e = refuse(&project, &open(), &request(&[&["PA"]]));
        assert!(e.contains("PA.1") && e.contains("port"), "{e}");
    }

    /// A carried pin whose net runs through a cable core is refused.
    /// 描くピンのネットが多芯ケーブルの芯を通っていると断る。
    #[test]
    fn a_net_through_a_cable_core_is_refused() {
        use crate::model::{Cable, CableCore, CoreEnd};
        let mut project = fixture_source();
        let find = |p: &Project, r: &str| -> Uuid {
            p.sheets[0]
                .entities
                .values()
                .find_map(|e| match e {
                    Entity::Symbol(s) if s.reference == r => Some(s.id),
                    _ => None,
                })
                .unwrap()
        };
        let (u1, m1) = (find(&project, "U1"), find(&project, "M1"));
        let core = CableCore {
            core: "D+".into(),
            color: String::new(),
            wire: None,
            end_a: Some(CoreEnd::Pin { part: u1, pin: "5".into(), index: None, contact: None }),
            end_b: Some(CoreEnd::Pin { part: m1, pin: "1".into(), index: None, contact: None }),
        };
        let cable = Entity::Cable(Cable {
            id: Uuid::new_v4(),
            reference: "W1".into(),
            part_no: String::new(),
            core_count: 1,
            sq: 0.0,
            points: vec![],
            cores: vec![core],
            shield: None,
            length_m: None,
            run: vec![Point::new(100.0, 250.0), Point::new(140.0, 250.0)],
        });
        project.sheets[0].entities.insert(cable.id(), cable);
        let e = refuse(&project, &open(), &request(&[&["U1"]]));
        assert!(e.contains("W1") && e.contains("cable"), "{e}");
    }
}
