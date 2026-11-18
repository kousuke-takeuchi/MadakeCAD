//! 機能シートの下書き (function sheet draft)。
//!
//! 元の図 (読み取り専用の `Project`) の結線から、新しい1枚のシートを作る計画を立てる純関数の層。
//! ドキュメントは変えない (`route::plan_reroute` / `arrange::plan_arrange` と同じ型)。
//! 3段で作る: model (どのピンを持つか、ネットの種類と名前) → layout (箱・辺・順・置き場所) → wires (配線)。
//! この節は model: 入力の形 [`DraftRequest`]・その検査・元のネットリストの読み取り [`build_model`]。
//!
//! 仕様: `docs/superpowers/specs/2026-11-18-function-sheet-draft-design.md` §3。
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

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
    /// 呼び出し側が下書きするつもりの全ての機能シート (順不同。この依頼自身や、すでにあるシートを含んでよい)。
    /// 各項目は名前の文字列か、`{name, units, pins?}` (そのシートが載せる機器とピン)。札の行き先の文字のために
    /// 確保する幅は、札ごとに、その札が将来持ちうる行き先だけで決まる。空なら従来どおり。
    #[serde(default, deserialize_with = "planned_sheets_from")]
    pub planned_sheets: Vec<PlannedSheet>,
}

/// 計画した機能シート1枚。名前だけなら中身は不明 (どの札の行き先にもなりうる)。
/// `{name, units, pins?}` なら、そのシートが載せるピンを持つネットの札だけの行き先になる。
///
/// 読み取りは手書き ([`PlannedSheet::from_value`]): 形が違えば、serde の untagged の一般的な文ではなく、
/// 何が足りないかを言う。`planned_sheets` の一覧としては [`planned_sheets_from`] が何番目の項目かを添える。
#[derive(Debug, Clone, schemars::JsonSchema)]
#[serde(untagged)]
pub enum PlannedSheet {
    /// 名前だけ。
    Name(String),
    /// 名前と、載せる機器・ピン。
    Detailed(PlannedSheetDetail),
}

/// 載せる機器を添えた計画のシート。`pins` は依頼の `pins` と同じ意味 (その機器をそのシートで限る)。
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlannedSheetDetail {
    pub name: String,
    pub units: Vec<String>,
    #[serde(default)]
    pub pins: BTreeMap<String, Vec<String>>,
}

impl PlannedSheet {
    /// シートの名前 (前後の空白はそのまま)。
    pub fn name(&self) -> &str {
        match self {
            PlannedSheet::Name(n) => n,
            PlannedSheet::Detailed(d) => &d.name,
        }
    }
}

impl PlannedSheet {
    /// JSON の1項目を読む。文字列なら名前だけ、オブジェクトなら `{name, units, pins?}` (知らない項目は断る)。
    pub fn from_value(value: serde_json::Value) -> Result<Self, String> {
        match value {
            serde_json::Value::String(name) => Ok(PlannedSheet::Name(name)),
            serde_json::Value::Object(_) => serde_json::from_value::<PlannedSheetDetail>(value)
                .map(PlannedSheet::Detailed)
                .map_err(|e| format!("expected {{\"name\", \"units\", \"pins\"?}}: {e}")),
            other => Err(format!("expected a sheet name or an object {{\"name\", \"units\", \"pins\"?}}, got {other}")),
        }
    }
}

impl<'de> Deserialize<'de> for PlannedSheet {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(d)?;
        PlannedSheet::from_value(value).map_err(serde::de::Error::custom)
    }
}

/// `planned_sheets` を読む。一覧でない・形の違う項目があれば、`planned_sheets[i] (entry i+1)` を挙げて断る
/// (Link API の400、MCP のエラー、CLI の表示にそのまま出る)。
fn planned_sheets_from<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<PlannedSheet>, D::Error> {
    use serde::de::Error;
    let value = serde_json::Value::deserialize(d)?;
    let serde_json::Value::Array(items) = value else {
        return Err(D::Error::custom(format!(
            "planned_sheets must be a list of sheet names or {{\"name\", \"units\", \"pins\"?}} objects, got {value}"
        )));
    };
    items
        .into_iter()
        .enumerate()
        .map(|(i, item)| {
            PlannedSheet::from_value(item)
                .map_err(|e| D::Error::custom(format!("planned_sheets[{i}] (entry {}): {e}", i + 1)))
        })
        .collect()
}

impl From<&str> for PlannedSheet {
    fn from(name: &str) -> Self {
        PlannedSheet::Name(name.to_string())
    }
}

impl From<String> for PlannedSheet {
    fn from(name: String) -> Self {
        PlannedSheet::Name(name)
    }
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
    #[allow(dead_code)] // 行番号は model の記録として残す (今のところ読む段は無い)
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

/// 計画のシート1枚 (この依頼自身を除く) が、このシートの札の行き先になりうるかを決めるもの。
#[derive(Debug, Clone)]
pub(crate) struct PlannedCarry {
    pub name: String,
    /// 名前だけなら `None` (どの札の行き先にもなりうる)。載せる機器とピンが分かっていれば、そのシートが
    /// ピンを載せる元のネットの名前 ([`DraftNet::name`] と同じ)。
    pub nets: Option<BTreeSet<String>>,
}

/// model 段の結果。
#[derive(Debug, Clone)]
pub(crate) struct DraftModel {
    pub units: Vec<DraftUnit>,
    pub nets: Vec<DraftNet>,
    /// 外したピン (参照記号, ピンの鍵, 理由)。
    pub skipped: Vec<(String, String, String)>,
    /// `planned_sheets` の項目 (この依頼自身は除く)。
    pub planned: Vec<PlannedCarry>,
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

/// プロジェクトのネットリストを、機器のピンだけの組にしたもの ([`device_nets`])。
pub(crate) struct DeviceNets {
    /// ネット名と、機器のピン (参照記号, ピンの鍵)。ネットの印・貫通のピンは含めない。全シートを統合したネット。
    pub nets: Vec<(String, BTreeSet<(String, String)>)>,
    /// 図面に描いてある機器のピン (つながっていないピンも含む)。
    pub drawn: BTreeSet<(String, String)>,
}

/// 全シートのプロジェクトネットリストを機器のピンだけに減らす。元の図の読み取り ([`build_model`]) と同じ減らし方で、
/// 下書きの検算 (`draft_check`) が両側にこれを使う。
pub(crate) fn device_nets(project: &Project) -> DeviceNets {
    let src = analyse(project);
    let drawn = src
        .unit_pins
        .iter()
        .flat_map(|(r, pins)| pins.iter().map(move |(k, _, _)| (r.clone(), k.clone())))
        .collect();
    DeviceNets { nets: src.nets.into_iter().map(|n| (n.name, n.pins)).collect(), drawn }
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
                if r.is_empty() {
                    return Err(format!("Column {} holds a blank reference: name a unit of the source", c + 1));
                }
                if !seen.insert(r.to_string()) {
                    return Err(format!("The reference {r} is listed more than once in columns (column {})", c + 1));
                }
            }
        }
        if self.pitch_mm != 2.5 && self.pitch_mm != 5.0 {
            return Err(format!("pitch_mm must be 2.5 or 5.0, got {}", self.pitch_mm));
        }
        let mut planned = BTreeSet::new();
        for (i, p) in self.planned_sheets.iter().enumerate() {
            let p = p.name().trim();
            if p.is_empty() {
                return Err(format!("planned_sheets entry {} is blank: give the name of a sheet you will draft", i + 1));
            }
            if !planned.insert(p.to_string()) {
                return Err(format!("The sheet name '{p}' is listed more than once in planned_sheets"));
            }
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

/// 機器1つの、載せるピン (鍵, 名前, ネットの番号, ポートか) と、外したピン。`wanted` はピンを限る鍵 (無ければ全て)。
/// 使用中 = 元のネットが2ピン以上、または電源・接地。下書きするシートと、計画のシートの両方がこの決め方を使う。
#[allow(clippy::type_complexity)]
fn unit_pin_nets(
    src: &Source,
    reference: &str,
    wanted: Option<&BTreeSet<&str>>,
    power: &BTreeSet<usize>,
    ground: &BTreeSet<usize>,
) -> Result<(Vec<(String, String, usize, bool)>, Vec<(String, String, String)>), String> {
    let all = &src.unit_pins[reference];
    if let Some(w) = wanted {
        for k in w {
            if !all.iter().any(|(key, _, _)| key == k) {
                return Err(format!("The unit {reference} has no pin '{k}'"));
            }
        }
    }
    let (mut pins, mut skipped) = (Vec::new(), Vec::new());
    for (key, name, is_port) in all {
        if wanted.is_some_and(|w| !w.contains(key.as_str())) {
            continue;
        }
        let net = src.pin_net.get(&(reference.to_string(), key.clone())).copied();
        let Some(i) = net else {
            skipped.push((reference.to_string(), key.clone(), "pin is not connected".to_string()));
            continue;
        };
        if src.nets[i].pins.len() >= 2 || power.contains(&i) || ground.contains(&i) {
            pins.push((key.clone(), name.clone(), i, *is_port));
        } else {
            skipped.push((reference.to_string(), key.clone(), "net has one pin".to_string()));
        }
    }
    Ok((pins, skipped))
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
    // ライブラリのシンボルの機器は、元の図で1つのシンボルのときだけ置ける (リレーのコイルと接点のように
    // 何個かのシンボルで描いた機器は、1つのシンボルとして置き直せない)
    for r in req.columns.iter().flatten() {
        let list = &src.instances[r.trim()];
        if list.len() >= 2 && list.iter().any(|s| s.symbol_id != BLOCK_SYMBOL_ID) {
            return Err(format!(
                "The unit {} is drawn by {} symbols in the source; a function sheet places a library symbol only when the unit is one symbol",
                r.trim(),
                list.len()
            ));
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
            let wanted: Option<BTreeSet<&str>> = req.pins.get(&reference).map(|l| l.iter().map(|k| k.trim()).collect());
            let (in_use, skips) = unit_pin_nets(&src, &reference, wanted.as_ref(), &power, &ground)?;
            skipped.extend(skips);
            let mut pins = Vec::new();
            for (key, name, net, is_port) in in_use {
                if is_port {
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
                pins.push((key, name, net));
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
    let planned = planned_carries(&src, open, req, &power, &ground)?;
    Ok(DraftModel { units, nets, skipped, planned })
}

/// `planned_sheets` の項目ごとに、載せるピンを持つ元のネットの名前を求める (この依頼自身の項目は除く)。
/// 載せるピンの決め方は下書きするシートと同じ ([`unit_pin_nets`])。元の図に無い機器・その機器に無いピンは断る。
fn planned_carries(
    src: &Source,
    _open: &Project,
    req: &DraftRequest,
    power: &BTreeSet<usize>,
    ground: &BTreeSet<usize>,
) -> Result<Vec<PlannedCarry>, String> {
    let own = req.name.trim();
    let mut out = Vec::new();
    for entry in &req.planned_sheets {
        let name = entry.name().trim().to_string();
        let nets = match entry {
            PlannedSheet::Name(_) => None,
            PlannedSheet::Detailed(d) => {
                let mut set = BTreeSet::new();
                let listed: BTreeSet<&str> = d.units.iter().map(|u| u.trim()).collect();
                for r in d.pins.keys() {
                    if !listed.contains(r.trim()) {
                        return Err(format!("planned_sheets entry '{name}': pins names the reference {r}, which is not in its units"));
                    }
                }
                for unit in &d.units {
                    let reference = unit.trim();
                    if reference.is_empty() {
                        return Err(format!("planned_sheets entry '{name}' holds a blank reference in units"));
                    }
                    if !src.instances.contains_key(reference) {
                        return Err(format!("planned_sheets entry '{name}': the reference {reference} does not exist in the source"));
                    }
                    let wanted: Option<BTreeSet<&str>> =
                        d.pins.iter().find(|(k, _)| k.trim() == reference).map(|(_, l)| l.iter().map(|k| k.trim()).collect());
                    let (pins, _) = unit_pin_nets(src, reference, wanted.as_ref(), power, ground)
                        .map_err(|e| format!("planned_sheets entry '{name}': {e}"))?;
                    set.extend(pins.into_iter().map(|(_, _, net, _)| src.nets[net].name.clone()));
                }
                Some(set)
            }
        };
        if name != own {
            out.push(PlannedCarry { name, nets });
        }
    }
    Ok(out)
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
            planned_sheets: Vec::new(),
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

    /// A blank planned name is refused.
    /// 空白だけの計画のシート名は断られる。
    #[test]
    fn a_blank_planned_name_is_refused() {
        let mut r = request(&[&["U1"]]);
        r.planned_sheets = vec!["Other".into(), "  ".into()];
        let msg = refuse(&fixture_source(), &Project::new("open"), &r);
        assert!(msg.contains("planned_sheets") && msg.contains("blank"), "{msg}");
    }

    /// The same planned name twice (ignoring surrounding spaces) is refused and the message names it.
    /// 同じ計画のシート名が2回 (前後の空白は無視) あると断られ、その名前が文に出る。
    #[test]
    fn a_duplicate_planned_name_is_refused() {
        let mut r = request(&[&["U1"]]);
        r.planned_sheets = vec!["Other".into(), " Other".into()];
        let msg = refuse(&fixture_source(), &Project::new("open"), &r);
        assert!(msg.contains("Other") && msg.contains("more than once"), "{msg}");
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

    /// The (number, name) of every pin of the block `reference` on a drafted sheet; a pin without a number shows ("", name).
    fn view_pins(sheet: &Sheet, reference: &str) -> BTreeSet<(String, String)> {
        let s = sheet
            .entities
            .values()
            .find_map(|e| match e {
                Entity::Symbol(s) if s.reference == reference => Some(s),
                _ => None,
            })
            .expect("the unit is drawn");
        let pins: serde_json::Value = serde_json::from_str(&s.attrs[crate::block::BLOCK_PINS_ATTR]).unwrap();
        let text = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        ["left", "right"]
            .iter()
            .flat_map(|side| pins[side].as_array().cloned().unwrap_or_default())
            .filter(|r| r.get("gap").is_none())
            .map(|r| (text(&r, "no"), text(&r, "name")))
            .collect()
    }

    /// A block whose numbered pins repeat each other's names or another pin's number (PX and PX_2 both named PX, GND and GND_2 both named GND, 1 and 2 both named COM) is valid in the source, and the draft makes a view of it that keeps every number and every name.
    /// 番号付きのピンが互いの名前や別のピンの番号と同じ名前を持つブロック (PX と PX_2 がどちらも名前 PX、GND と GND_2 がどちらも GND、1 と 2 がどちらも COM) は元の図で正しく、下書きはその全ての番号と名前を保ったビューを作る。
    #[test]
    fn numbered_pins_that_share_names_are_drafted_with_their_names() {
        let mut project = fixture_source();
        let u9 = block(
            "U9",
            60.0,
            200.0,
            r#"{"left":[{"no":"PX","name":"PX"},{"no":"PX_2","name":"PX"},{"no":"1","name":"COM"},{"no":"2","name":"COM"}],"right":[{"no":"GND","name":"GND"},{"no":"GND_2","name":"GND"}]}"#,
        );
        let u8 = block("U8", 160.0, 200.0, r#"{"left":[{"no":"1"},{"no":"2"},{"no":"3"},{"no":"4"},{"no":"5"},{"no":"6"}]}"#);
        let mut e = Vec::new();
        for (i, key) in ["PX", "PX_2", "1", "2", "GND", "GND_2"].iter().enumerate() {
            let net = format!("K{}", i + 1);
            labelled(&mut e, &u9, key, &net);
            labelled(&mut e, &u8, &(i + 1).to_string(), &net);
        }
        e.push(Entity::Symbol(u9));
        e.push(Entity::Symbol(u8));
        for x in e {
            project.sheets[0].entities.insert(x.id(), x);
        }
        crate::block::parse_block_spec(&project.sheets[0].entities.values().find_map(|e| match e {
            Entity::Symbol(s) if s.reference == "U9" => Some(s.attrs.clone()),
            _ => None,
        }).unwrap())
        .expect("the source holds the block");
        let plan = plan_function_sheet(&project, &open(), &request(&[&["U9"], &["U8"]])).expect("the block can be drafted");
        let want: BTreeSet<(String, String)> = [("PX", "PX"), ("PX_2", "PX"), ("1", "COM"), ("2", "COM"), ("GND", "GND"), ("GND_2", "GND")]
            .iter()
            .map(|(k, n)| (k.to_string(), n.to_string()))
            .collect();
        assert_eq!(view_pins(&plan.sheet, "U9"), want);
    }

    /// A pin whose number equals its name is still written without a number when no other pin of the unit is named like it, so the view looks as before.
    /// 番号と名前が同じピンは、同じ名前の別のピンがその機器に無ければ、これまでどおり番号なしで書かれ、ビューの見た目は変わらない。
    #[test]
    fn a_pin_named_like_its_number_alone_keeps_the_short_form() {
        let mut project = fixture_source();
        let u9 = block("U9", 60.0, 200.0, r#"{"left":[{"name":"EN"},{"no":"OUT","name":"OUT"}]}"#);
        let u8 = block("U8", 160.0, 200.0, r#"{"left":[{"no":"1"},{"no":"2"}]}"#);
        let mut e = Vec::new();
        for (i, key) in ["EN", "OUT"].iter().enumerate() {
            let net = format!("K{}", i + 1);
            labelled(&mut e, &u9, key, &net);
            labelled(&mut e, &u8, &(i + 1).to_string(), &net);
        }
        e.push(Entity::Symbol(u9));
        e.push(Entity::Symbol(u8));
        for x in e {
            project.sheets[0].entities.insert(x.id(), x);
        }
        let plan = plan_function_sheet(&project, &open(), &request(&[&["U9"], &["U8"]])).unwrap();
        let want: BTreeSet<(String, String)> = [("", "EN"), ("", "OUT")].iter().map(|(k, n)| (k.to_string(), n.to_string())).collect();
        assert_eq!(view_pins(&plan.sheet, "U9"), want);
    }

    /// Two pins of one unit with the same key cannot be drawn as one view; the refusal names the unit and the pin.
    /// 1つの機器に同じ鍵のピンが2本あるとビューに描けず、断る文は機器とピンを挙げる。
    #[test]
    fn a_real_key_collision_is_refused_naming_the_unit_and_the_pin() {
        let unit = DraftUnit {
            reference: "U9".into(),
            column: 0,
            row: 0,
            source: block("U9", 60.0, 200.0, r#"{"left":[{"no":"A","name":"X"}]}"#),
            is_block: true,
            pins: vec![
                DraftPin { key: "A".into(), name: "X".into(), net: 0 },
                DraftPin { key: "A".into(), name: "Y".into(), net: 1 },
            ],
        };
        let msg = view_spec(&unit, &[Row::Pin(0), Row::Pin(1)], &[], 5.0).expect_err("a repeated key is refused");
        assert!(msg.contains("U9") && msg.contains("A"), "{msg}");
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

    /// A blank reference in columns is refused with the column number.
    /// columns に空白の参照記号があると、列の番号を挙げて断る。
    #[test]
    fn a_blank_reference_is_refused() {
        let e = refuse(&fixture_source(), &open(), &request(&[&["U1"], &["  "]]));
        assert!(e.contains("blank") && e.contains("Column 2"), "{e}");
    }

    /// A library-symbol unit that the source draws with several symbols (like a relay coil and its contacts) is refused, because it cannot be placed as one symbol.
    /// 元の図で何個かのシンボルで描いたライブラリのシンボルの機器 (リレーのコイルと接点など) は、1つのシンボルとして置けないので断る。
    #[test]
    fn a_library_unit_drawn_by_several_symbols_is_refused() {
        let mut source = fixture_source();
        let extra = Entity::Symbol(plain("resistor", "R1", "10k", Point::new(350.0, 220.0)));
        source.sheets[0].entities.insert(extra.id(), extra);
        let e = refuse(&source, &open(), &request(&[&["R1"]]));
        assert!(e.contains("R1") && e.contains("2 symbols"), "{e}");
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

// --- layout
//
// 第2段: model から、新しいシートを組み立てる。箱 (ブロック) とライブラリのシンボル、電源・接地の印、
// 出ていくネットのタグ、5mm の短い線 (スタブ)、配線の端 (Terminal)。内部の配線はまだ引かない (次の段)。
// ピンの座標は自分で計算せず、組んだシートから `route::pin_points` で読む。

use crate::geometry::Point;
use crate::model::{NetLabel, Orientation, PaperSize, Sheet, Wire, XrefStyle};
use crate::netlist::pin_positions;
use crate::pin_wiring::world_dir;
use crate::route::{pin_points, Dir};
use crate::symbol::PinDir;
use crate::tidy::{symbol_bounds, text_width, Rect};

/// 印・タグを置くための、ピンから伸ばす短い線の長さ (mm)。
pub(crate) const STUB_MM: f64 = 5.0;
/// 配線もある出ていくネットで、ピンの短い線の先からさらに札を離す枝の長さ (mm)。配線は枝の途中から相手へ向かう。
pub(crate) const TAG_BRANCH_MM: f64 = 5.0;
/// 札の中に後から書かれる他のシートの名前のために、少なくとも確保する文字数。
pub(crate) const RESERVED_DEST_CHARS: usize = 10;
/// 隣り合う列のあいだにあける、配線の通り道の幅 (mm) の最小。
pub(crate) const MIN_CHANNEL_MM: f64 = 20.0;
/// 通り道の幅の基本 (mm): 通り道を横切るネットが多いとき、この値 + ネット1本につき [`CHANNEL_PER_NET_MM`]。
const CHANNEL_BASE_MM: f64 = 10.0;
const CHANNEL_PER_NET_MM: f64 = 2.5;
/// 描画範囲 (図枠の内側) の縁から空ける余白 (mm)。
const AREA_MARGIN_MM: f64 = 10.0;
/// 箱の上に書く参照記号・型番のために、箱の上へ確保する高さ (mm)。
const CAPTION_MM: f64 = 7.5;
/// 列の中で機器と機器のあいだにあける最小のすき間 (ピン間隔の何倍か)。
const GAP_PITCHES: f64 = 4.0;
const GRID_MM: f64 = 2.5;
/// 電源の印のシンボル。
const POWER_SYMBOL: &str = "power_rail";
/// 印の値 (ネット名) の文字高さ (mm)。幅の見積りに使う。
const MARK_TEXT_MM: f64 = 2.5;
/// 箱のピン名の文字高さ (mm): ピン間隔5mmなら2.5、2.5mmなら2.2 (どちらも2.2mm以上)。
fn pin_text_mm(pitch: f64) -> f64 {
    if pitch >= 5.0 { 2.5 } else { 2.2 }
}

/// 配線の端: 配線を引く相手のピン (または札の位置) と、配線が端から出ていく向き。
#[derive(Debug, Clone, Copy)]
pub(crate) struct Terminal {
    /// [`DraftModel::nets`] の添字。
    pub net: usize,
    pub at: Point,
    /// ピンなら箱から離れる向き、札なら箱へ戻る向き (旗の本体と反対側)。
    pub dir: Dir,
    /// 札を下げたピンなら、その札の位置 (ピンから同じ行を外へ `STUB_MM + TAG_BRANCH_MM`)。
    pub tag_at: Option<Point>,
    /// 札の位置そのものの端か (札を下げた組の端と、その札の位置の2つで、札を配線につなぐ)。
    pub is_tag: bool,
    /// 組の棒の上の端 (1本の組なら短い線の先)。引けずに札でつなぐとき、札はここに下げる (旗は基準点の上にあるので、
    /// 棒は旗の外へ出ていく)。
    pub top: Point,
}

/// layout 段の結果。
#[derive(Debug, Clone)]
pub(crate) struct DraftLayout {
    /// 箱・シンボル・印・タグ・スタブを置いたシート (内部の配線はまだ無い)。
    pub sheet: Sheet,
    /// 配線の端 (このシートで2本以上のピンを持つ、内部または出ていくネットのピンごとに1つ)。
    pub terminals: Vec<Terminal>,
    /// タグを付けたネット (ネット名, このシートが持つピンの数)。
    pub tagged: Vec<(String, usize)>,
    pub power_marks: usize,
    pub ground_marks: usize,
    /// 配線が近づいてはいけない矩形: 札の旗 (後から書かれる他のシートの名前の分を含む) と、印の図形と文字。
    pub keep_out: Vec<Rect>,
    /// 図枠の内側と表題欄 (改訂欄の行を含む) の矩形 (x0, y0, x1, y1)。[`crate::arrange::frame_areas`] の値。
    pub frame: (f64, f64, f64, f64),
    pub title: (f64, f64, f64, f64),
    /// 機器のある列ごとの、左端と右端 (用紙のx座標、箱・短い線・棒・タグ・印を含む)。
    pub columns: Vec<ColumnSpan>,
    /// 札の旗に後から書かれる行き先のために確保する文字 (引けずに足す札の旗の大きさにも使う)。
    pub(crate) reserve: Reserve,
    /// ネットラベル (札) の文字高さ (mm)。
    pub label_font: f64,
}

/// 列1つの横の広がり (用紙のx座標)。
#[derive(Debug, Clone, Copy)]
pub(crate) struct ColumnSpan {
    pub left: f64,
    pub right: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Row {
    Pin(usize),
    Gap,
}

/// 箱の1辺で、同じネットのピンを隣の行に並べた組 (「棒」で先をつなぐ)。ピンの添字は鍵の自然順。
#[derive(Debug, Clone)]
struct Group {
    net: usize,
    pins: Vec<usize>,
}

/// 機器1つの、ピンの辺・並び (箱の機器だけが並びを持つ)。
#[derive(Debug, Clone)]
struct UnitPlan {
    /// ピンごとの辺。
    side: Vec<Side>,
    /// ピンごとの、箱から離れる向き (スタブの向き)。
    stub: Vec<Dir>,
    /// 辺ごとの短い線の長さ (mm): [左, 右]。その辺のいちばん広い番号に合わせる。ライブラリのシンボルは [`STUB_MM`]。
    len: [f64; 2],
    /// 電源の組 (左辺の先頭)・信号の組・接地の組 (左辺の末尾)。箱の機器だけ。
    power: Vec<Group>,
    left: Vec<Group>,
    right: Vec<Group>,
    ground: Vec<Group>,
    /// ネットごとに、同じ辺の電源の組のあいだ・接地の組のあいだへ、印のために空ける行数。
    mark_gap: Vec<usize>,
}

impl UnitPlan {
    /// 辺の組を、箱の上から下の順に。電源・信号・接地の順。
    fn groups(&self, side: Side) -> Vec<&Group> {
        match side {
            Side::Left => self.power.iter().chain(&self.left).chain(&self.ground).collect(),
            Side::Right => self.right.iter().collect(),
        }
    }

    /// 辺の行 (ピンとすき間)。電源・信号・接地のうち空でない種類のあいだに1行のすき間を入れる。電源どうし・接地どうしの組の
    /// あいだには、印の高さぶんの行をあける (印が隣の行に重ならない)。
    fn rows(&self, side: Side) -> Vec<Row> {
        let sections: Vec<(u8, &Vec<Group>)> = match side {
            Side::Left => vec![(0, &self.power), (1, &self.left), (2, &self.ground)],
            Side::Right => vec![(1, &self.right)],
        }
        .into_iter()
        .filter(|(_, g)| !g.is_empty())
        .collect();
        let gap_of = |net: usize| self.mark_gap.get(net).copied().unwrap_or(1);
        let mut out = Vec::new();
        for (i, (kind, list)) in sections.iter().enumerate() {
            if i > 0 {
                out.push(Row::Gap);
            }
            for (j, g) in list.iter().enumerate() {
                if j > 0 {
                    let n = match kind {
                        0 => gap_of(g.net),
                        2 => gap_of(list[j - 1].net),
                        _ => 0,
                    };
                    out.extend(std::iter::repeat(Row::Gap).take(n));
                }
                out.extend(g.pins.iter().map(|&p| Row::Pin(p)));
            }
        }
        out
    }

    /// ピンが辺の何行目にあるか (電源・接地・信号とも)。
    fn row_index(&self, pin: usize) -> Option<usize> {
        [Side::Left, Side::Right].into_iter().find_map(|s| self.rows(s).iter().position(|r| *r == Row::Pin(pin)))
    }
}

/// 辺のいちばん広い番号の幅 (mm) から、その辺の短い線の長さを決める: 5mm以上、番号の幅 + 2.5mm、2.5mmの格子へ切り上げ。
fn stub_length(widest_number: f64) -> f64 {
    grid_up(f64::max(STUB_MM, widest_number + GRID_MM))
}

fn dir_of_pin(d: PinDir) -> Dir {
    match d {
        PinDir::Up => Dir::N,
        PinDir::Down => Dir::S,
        PinDir::Left => Dir::W,
        PinDir::Right => Dir::E,
    }
}

fn unit_vec(d: Dir) -> (f64, f64) {
    match d {
        Dir::N => (0.0, -1.0),
        Dir::E => (1.0, 0.0),
        Dir::S => (0.0, 1.0),
        Dir::W => (-1.0, 0.0),
    }
}

fn grid_up(v: f64) -> f64 {
    (v / GRID_MM - 1e-9).ceil() * GRID_MM
}

fn grid_down(v: f64) -> f64 {
    (v / GRID_MM + 1e-9).floor() * GRID_MM
}

fn origin() -> Point {
    Point::new(0.0, 0.0)
}

/// ライブラリのシンボルのピンの (辺, 箱から離れる向き)。置き場所は原点で、元の回転・反転のまま。
fn library_pin_geometry(unit: &DraftUnit, defs: &[SymbolDef]) -> Result<Vec<(Side, Dir)>, String> {
    let index = DefIndex::new(defs);
    let mut inst = unit.source.clone();
    inst.at = origin();
    let def = index
        .get(&inst)
        .ok_or_else(|| format!("The symbol of {} has no definition", unit.reference))?;
    let positions = pin_positions(&inst, &def);
    unit.pins
        .iter()
        .map(|p| {
            let i = positions
                .iter()
                .position(|(k, _)| *k == p.key)
                .ok_or_else(|| format!("The symbol {} has no pin {}", unit.reference, p.key))?;
            let dir = world_dir(def.pins[i].dir, &inst);
            let x = positions[i].1.x;
            let side = if x < -1e-9 {
                Side::Left
            } else if x > 1e-9 {
                Side::Right
            } else if matches!(dir, PinDir::Left | PinDir::Up) {
                Side::Left
            } else {
                Side::Right
            };
            Ok((side, dir_of_pin(dir)))
        })
        .collect()
}

/// 同じネットのピンを1つの組にする。組は最初のピンの鍵の自然順、組の中のピンも鍵の自然順。
fn grouped(unit: &DraftUnit, pins: Vec<usize>) -> Vec<Group> {
    let mut by: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for p in pins {
        by.entry(unit.pins[p].net).or_default().push(p);
    }
    let mut groups: Vec<Group> = by
        .into_iter()
        .map(|(net, mut pins)| {
            pins.sort_by(|a, b| natural_cmp(&unit.pins[*a].key, &unit.pins[*b].key));
            Group { net, pins }
        })
        .collect();
    groups.sort_by(|a, b| natural_cmp(&unit.pins[a.pins[0]].key, &unit.pins[b.pins[0]].key));
    groups
}

/// 機器ごとに、ピンの辺と組を決める (信号の組の並びは自然順のまま)。`mark_gap` はネットごとの、印のために空ける行数。
fn plan_units(model: &DraftModel, defs: &[SymbolDef], pitch: f64, mark_gap: &[usize]) -> Result<Vec<UnitPlan>, String> {
    let last_column = model.units.iter().map(|u| u.column).max().unwrap_or(0);
    let th = pin_text_mm(pitch);
    // ネットごとの、このシートのピン (機器の添字, ピンの添字)
    let mut by_net: BTreeMap<usize, Vec<(usize, usize)>> = BTreeMap::new();
    for (u, unit) in model.units.iter().enumerate() {
        for (p, pin) in unit.pins.iter().enumerate() {
            by_net.entry(pin.net).or_default().push((u, p));
        }
    }
    let mut plans = Vec::new();
    for (ui, unit) in model.units.iter().enumerate() {
        let mut plan = UnitPlan {
            side: vec![Side::Left; unit.pins.len()],
            stub: vec![Dir::W; unit.pins.len()],
            len: [STUB_MM, STUB_MM],
            power: Vec::new(),
            left: Vec::new(),
            right: Vec::new(),
            ground: Vec::new(),
            mark_gap: mark_gap.to_vec(),
        };
        if !unit.is_block {
            for (p, (side, dir)) in library_pin_geometry(unit, defs)?.into_iter().enumerate() {
                plan.side[p] = side;
                plan.stub[p] = dir;
            }
            plans.push(plan);
            continue;
        }
        let (mut power, mut left, mut right, mut ground) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for (p, pin) in unit.pins.iter().enumerate() {
            let net = &model.nets[pin.net];
            let side = match net.class {
                NetClass::Power => {
                    power.push(p);
                    Side::Left
                }
                NetClass::Ground => {
                    ground.push(p);
                    Side::Left
                }
                NetClass::Internal | NetClass::Leaving => {
                    // 相手の数: 左の列・右の列。同じ列の (別の機器の) 相手は右に数える (最後の列では左)。
                    // 同じ列の機器どうしの配線が、札の並ぶ左の余白ではなく右の通り道を通るように
                    let (mut to_left, mut to_right) = (0usize, 0usize);
                    for &(u2, _) in by_net.get(&pin.net).into_iter().flatten() {
                        match model.units[u2].column.cmp(&unit.column) {
                            Ordering::Less => to_left += 1,
                            Ordering::Greater => to_right += 1,
                            Ordering::Equal if u2 != ui => {
                                if unit.column == last_column {
                                    to_left += 1
                                } else {
                                    to_right += 1
                                }
                            }
                            Ordering::Equal => {}
                        }
                    }
                    let side = match (to_left, to_right) {
                        (0, 0) => {
                            if unit.column == last_column { Side::Right } else { Side::Left }
                        }
                        (_, 0) => Side::Left,
                        (0, _) => Side::Right,
                        (l, r) => {
                            if r > l { Side::Right } else { Side::Left }
                        }
                    };
                    match side {
                        Side::Left => left.push(p),
                        Side::Right => right.push(p),
                    }
                    side
                }
            };
            plan.side[p] = side;
            plan.stub[p] = if side == Side::Left { Dir::W } else { Dir::E };
        }
        plan.power = grouped(unit, power);
        plan.left = grouped(unit, left);
        plan.right = grouped(unit, right);
        plan.ground = grouped(unit, ground);
        // 辺ごとの短い線: その辺で印字される番号のうちいちばん広いものに合わせる
        for (i, side) in [Side::Left, Side::Right].into_iter().enumerate() {
            let widest = unit
                .pins
                .iter()
                .enumerate()
                .filter(|(p, _)| plan.side[*p] == side)
                .map(|(_, pin)| text_width(pin.key.trim(), th))
                .fold(0.0, f64::max);
            plan.len[i] = stub_length(widest);
        }
        plans.push(plan);
    }
    Ok(plans)
}

/// 重心法で信号の組の並びを決める。左から右、右から左の2回の掃引。決定的 (同じ重心は自然順)。
/// 組は1つの項目として動き、重心はその組のピンの重心の平均 (同じネットのピンの相手は同じなので、組の相手の位置の平均)。
fn order_pins(model: &DraftModel, plans: &mut [UnitPlan], tops: &[f64], mids: &[f64], pitch: f64) {
    let last_column = model.units.iter().map(|u| u.column).max().unwrap_or(0);
    // 信号のネットの、相手のピン (自分の機器の外)
    let mut by_net: BTreeMap<usize, Vec<(usize, usize)>> = BTreeMap::new();
    for (u, unit) in model.units.iter().enumerate() {
        for (p, pin) in unit.pins.iter().enumerate() {
            if matches!(model.nets[pin.net].class, NetClass::Internal | NetClass::Leaving) {
                by_net.entry(pin.net).or_default().push((u, p));
            }
        }
    }
    for pass in 0..2 {
        let mut columns: Vec<usize> = (0..=last_column).collect();
        if pass == 1 {
            columns.reverse();
        }
        for column in columns {
            for u in (0..model.units.len()).filter(|&u| model.units[u].column == column && model.units[u].is_block) {
                for side in [Side::Left, Side::Right] {
                    let list = if side == Side::Left { plans[u].left.clone() } else { plans[u].right.clone() };
                    if list.len() < 2 {
                        continue;
                    }
                    let bary: Vec<Option<f64>> = list
                        .iter()
                        .map(|g| {
                            let ys: Vec<f64> = by_net[&g.net]
                                .iter()
                                .filter(|(u2, _)| *u2 != u)
                                .map(|&(u2, p2)| {
                                    if model.units[u2].is_block {
                                        tops[u2] + plans[u2].row_index(p2).unwrap_or(0) as f64 * pitch
                                    } else {
                                        mids[u2]
                                    }
                                })
                                .collect();
                            if ys.is_empty() { None } else { Some(ys.iter().sum::<f64>() / ys.len() as f64) }
                        })
                        .collect();
                    let slots: Vec<usize> = (0..list.len()).filter(|&i| bary[i].is_some()).collect();
                    let mut movable: Vec<(f64, Group)> = slots.iter().map(|&i| (bary[i].unwrap(), list[i].clone())).collect();
                    movable.sort_by(|a, b| {
                        if (a.0 - b.0).abs() < 1e-9 {
                            natural_cmp(&model.units[u].pins[a.1.pins[0]].key, &model.units[u].pins[b.1.pins[0]].key)
                        } else {
                            a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal)
                        }
                    });
                    let mut next = list.clone();
                    for (slot, (_, g)) in slots.iter().zip(movable) {
                        next[*slot] = g;
                    }
                    if side == Side::Left {
                        plans[u].left = next;
                    } else {
                        plans[u].right = next;
                    }
                }
            }
        }
    }
}

fn row_json(unit: &DraftUnit, rows: &[Row]) -> serde_json::Value {
    rows.iter()
        .map(|r| match r {
            Row::Gap => serde_json::json!({"gap": 1}),
            Row::Pin(p) => {
                let pin = &unit.pins[*p];
                // 番号を書かない (鍵 = 名前) のは、番号付きで書く別のピンが同じ名前を持たないときだけ。ブロックの決まりで
                // 番号の無いピンの名前は番号付きのピンの名前と重なれないので、重なるなら番号も書く (鍵は変わらない)
                let named_alike = unit.pins.iter().any(|q| q.key != q.name && q.name == pin.key);
                if pin.key == pin.name && !named_alike {
                    serde_json::json!({"name": pin.name})
                } else if pin.name.is_empty() {
                    serde_json::json!({"no": pin.key})
                } else {
                    serde_json::json!({"no": pin.key, "name": pin.name})
                }
            }
        })
        .collect::<Vec<_>>()
        .into()
}

/// 箱の属性と外形 (幅, 高さ)。大きさは既存のブロックの規則 ([`block::block_box`]) の最小値。
fn block_attrs(unit: &DraftUnit, plan: &UnitPlan, pitch: f64) -> Result<(BTreeMap<String, String>, (f64, f64)), String> {
    let (mut attrs, spec) = view_spec(unit, &plan.rows(Side::Left), &plan.rows(Side::Right), pitch)?;
    let (w, h) = block::block_box(&spec);
    attrs.insert(block::BLOCK_SIZE_ATTR.into(), serde_json::json!({"w": w, "h": h}).to_string());
    Ok((attrs, (w, h)))
}

/// ビューの箱の属性 (大きさを除く) と、それを読んだブロックの仕様。左右の行は `left`・`right`。
/// ブロックとして成り立たない (同じ鍵のピンが2本など) ときは、機器とブロックの決まりの文 (ピンを挙げる) で断る。
fn view_spec(
    unit: &DraftUnit,
    left: &[Row],
    right: &[Row],
    pitch: f64,
) -> Result<(BTreeMap<String, String>, block::BlockSpec), String> {
    let mut attrs: BTreeMap<String, String> = unit
        .source
        .attrs
        .iter()
        .filter(|(k, _)| !k.starts_with("block_") && k.as_str() != crate::pin_groups::PIN_GROUPS_ATTR)
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let pins = serde_json::json!({
        "left": row_json(unit, left),
        "right": row_json(unit, right),
    });
    attrs.insert(block::BLOCK_PINS_ATTR.into(), pins.to_string());
    attrs.insert(block::BLOCK_VIEW_ATTR.into(), "1".into());
    attrs.insert(block::BLOCK_PITCH_ATTR.into(), if pitch >= 5.0 { "5" } else { "2.5" }.into());
    attrs.insert(block::BLOCK_PIN_TEXT_ATTR.into(), pin_text_mm(pitch).to_string());
    // 長い番号が箱の辺やピン名にかからないよう、外の番号は辺から離れる向きへ書く
    attrs.insert(block::BLOCK_NUMBER_ALIGN_ATTR.into(), "edge".into());
    let spec = block::parse_block_spec(&attrs)
        .map_err(|e| format!("The unit {} cannot be drawn as a view: {e}", unit.reference))?;
    Ok((attrs, spec))
}

/// 機器の実体 (箱かライブラリのシンボル) を `at` に置く。
fn unit_instance(unit: &DraftUnit, plan: &UnitPlan, pitch: f64, at: Point) -> Result<SymbolInstance, String> {
    let mut inst = unit.source.clone();
    inst.id = uuid::Uuid::new_v4();
    inst.at = at;
    if unit.is_block {
        inst.symbol_id = BLOCK_SYMBOL_ID.into();
        inst.rotation = 0;
        inst.mirror = false;
        inst.attrs = block_attrs(unit, plan, pitch)?.0;
    }
    Ok(inst)
}

/// 印の向き: ピンの接続方向が `want` を向く回転。
fn mark_rotation(def: &SymbolDef, want: PinDir) -> u16 {
    let Some(pin) = def.pins.first() else { return 0 };
    for rotation in [0u16, 90, 180, 270] {
        let probe = SymbolInstance {
            id: uuid::Uuid::nil(),
            symbol_id: def.id.clone(),
            at: origin(),
            rotation,
            mirror: false,
            reference: String::new(),
            value: String::new(),
            attrs: BTreeMap::new(),
        };
        if world_dir(pin.dir, &probe) == want {
            return rotation;
        }
    }
    0
}

/// 印・タグ・スタブの1つ (どの機器のものか付き)。
struct DecoItem {
    unit: usize,
    entity: Entity,
}

struct Deco {
    items: Vec<DecoItem>,
    terminals: Vec<Terminal>,
    tagged: Vec<(String, usize)>,
    power: usize,
    ground: usize,
}

fn default_wire(points: Vec<Point>) -> Wire {
    Wire {
        id: uuid::Uuid::new_v4(),
        points,
        color: "black".into(),
        sq: 0.75,
        length_m: None,
        length_source: Default::default(),
        part_no: None,
        net: None,
        dashed: false,
        weight_mm: None,
    }
}

fn stub_wire(from: Point, to: Point) -> Entity {
    Entity::Wire(default_wire(vec![from, to]))
}

/// 配置した組1つ: 箱の1辺の同じネットのピン (ライブラリのシンボルは1ピンだけの組)。
struct PlacedGroup {
    /// [`DraftModel::units`] の添字。
    u: usize,
    /// 箱から離れる向き (短い線の向き)。
    dir: Dir,
    /// 短い線の長さ (mm)。
    len: f64,
    pins: Vec<usize>,
    net: usize,
}

/// 全ての組を、機器の順 (列 → 列の中の行)、箱の中の上から下の順に並べる。
fn placed_groups(model: &DraftModel, plans: &[UnitPlan]) -> Vec<PlacedGroup> {
    let mut out = Vec::new();
    for (u, unit) in model.units.iter().enumerate() {
        if unit.is_block {
            for (i, side) in [Side::Left, Side::Right].into_iter().enumerate() {
                for g in plans[u].groups(side) {
                    out.push(PlacedGroup {
                        u,
                        dir: if side == Side::Left { Dir::W } else { Dir::E },
                        len: plans[u].len[i],
                        pins: g.pins.clone(),
                        net: g.net,
                    });
                }
            }
        } else {
            for (p, pin) in unit.pins.iter().enumerate() {
                out.push(PlacedGroup { u, dir: plans[u].stub[p], len: STUB_MM, pins: vec![p], net: pin.net });
            }
        }
    }
    out
}

/// 札のネットラベル: 旗の本体は箱から離れる側へ伸びる。左辺 (向き W) のピンは「右」(本体が基準点の左)、
/// 右辺 (向き E) のピンは「左」+外向き。行き先は与えたときだけ付ける。
fn tag_label(name: &str, at: Point, dir: Dir, dest: Option<&String>) -> NetLabel {
    let right_side = dir == Dir::E;
    let mut attrs = BTreeMap::new();
    attrs.insert(crate::net_tag::TAG_ATTR.to_string(), if right_side { "left" } else { "right" }.to_string());
    if right_side {
        attrs.insert(crate::net_tag::FLOW_ATTR.to_string(), "out".to_string());
    }
    if let Some(dest) = dest {
        attrs.insert(crate::net_tag::DEST_ATTR.to_string(), dest.clone());
    }
    NetLabel { id: uuid::Uuid::new_v4(), at, name: name.to_string(), rotation: 0, attrs }
}

/// 印のシンボルの定義: 接地は元の図の印、無ければ (電源も) 同梱の電源の印。
fn mark_def<'a>(index: &DefIndex<'a>, net: &DraftNet) -> Result<&'a SymbolDef, String> {
    let symbol_id = if net.class == NetClass::Ground { net.ground_symbol.as_deref().unwrap_or(POWER_SYMBOL) } else { POWER_SYMBOL };
    index
        .by_id(symbol_id)
        .or_else(|| index.by_id(POWER_SYMBOL))
        .ok_or_else(|| format!("The mark symbol {symbol_id} is not available"))
}

/// 電源・接地の印を `at` (棒の端) に置く。電源の印は上へ、接地の印は下へ伸びる (ピンの接続方向が下・上を向く)。
fn mark_instance(index: &DefIndex, net: &DraftNet, at: Point) -> Result<SymbolInstance, String> {
    let def = mark_def(index, net)?;
    let ground = net.class == NetClass::Ground;
    Ok(SymbolInstance {
        id: uuid::Uuid::new_v4(),
        symbol_id: def.id.clone(),
        at,
        rotation: mark_rotation(def, if ground { PinDir::Up } else { PinDir::Down }),
        mirror: false,
        reference: String::new(),
        value: if def.net_from_value { net.name.clone() } else { String::new() },
        attrs: BTreeMap::new(),
    })
}

/// 印の引き出し: 組の端 `anchor` (電源は棒の上の端、接地は下の端) から、ピン間隔ぶん棒の向き (接地は下・電源は上) へ延ばし、
/// 箱から離れる向き (短い線の向き `dir`) へ曲がった先に印を置く。短い線が縦 (ライブラリのシンボルの上下のピン) なら、
/// 短い線の向きへ延ばしてから右へ曲がる。曲がった先までの長さは、印の文字と図形の半分の幅 + 1.25mm を
/// 2.5mmへ切り上げたもの。印の文字 (印の上に中央揃え) は縦の線から離れ、棒・短い線・番号にかからない。
/// 返り値は引き出しの線の点 (端, 角, 印) と印の位置。
fn mark_lead(index: &DefIndex, net: &DraftNet, anchor: Point, dir: Dir, pitch: f64) -> Result<(Vec<Point>, Point), String> {
    let def = mark_def(index, net)?;
    let probe = mark_instance(index, net, origin())?;
    let fig = symbol_bounds(&probe, def).unwrap_or_else(|| Rect::from_corners(origin(), origin()));
    let half = f64::max(text_width(&probe.value, crate::svg::LABEL_FONT) / 2.0, f64::max(-fig.min.x, fig.max.x));
    let reach = grid_up(half + GRID_MM / 2.0);
    let (first, second) = match dir {
        Dir::E | Dir::W => ((0.0, if net.class == NetClass::Ground { 1.0 } else { -1.0 }), unit_vec(dir)),
        Dir::N | Dir::S => (unit_vec(dir), (1.0, 0.0)),
    };
    let corner = Point::new(anchor.x + first.0 * pitch, anchor.y + first.1 * pitch);
    let at = Point::new(corner.x + second.0 * reach, corner.y + second.1 * reach);
    Ok((vec![anchor, corner, at], at))
}

/// 印の図形と値の文字が占める矩形。値は印の外形の上下に書かれる。
fn mark_rect(s: &SymbolInstance, def: &SymbolDef) -> Rect {
    let mut r = symbol_bounds(s, def).unwrap_or_else(|| Rect::from_corners(s.at, s.at));
    let half = text_width(&s.value, MARK_TEXT_MM) / 2.0;
    r.extend(Point::new(s.at.x - half, r.min.y - MARK_TEXT_MM));
    r.extend(Point::new(s.at.x + half, r.max.y + MARK_TEXT_MM));
    r
}

/// ネットごとに、同じ辺の電源どうし (接地どうし) の組のあいだへ空ける行数: 印の高さ + 1mm が隣の行に掛からないだけ。
/// 電源の印は棒の上の端から上へ、接地の印は下の端から下へ伸びる。
fn mark_gaps(model: &DraftModel, defs: &[SymbolDef], pitch: f64) -> Result<Vec<usize>, String> {
    let index = DefIndex::new(defs);
    model
        .nets
        .iter()
        .map(|net| {
            if !matches!(net.class, NetClass::Power | NetClass::Ground) {
                return Ok(1);
            }
            // 印は棒の端からピン間隔ぶん延ばした先で横へ曲がった所にある。その先の印の図形と文字の深さまで空ける
            let def = mark_def(&index, net)?;
            let r = mark_rect(&mark_instance(&index, net, origin())?, def);
            let span = pitch + if net.class == NetClass::Ground { r.max.y } else { -r.min.y };
            Ok((((span + 1.0) / pitch - 1e-9).ceil() as usize).saturating_sub(1).max(1))
        })
        .collect()
}

/// 置いた機器のピンから、印・タグ・スタブ・棒・配線の端を作る。
///
/// 同じ辺の同じネットのピンは1つの組で、短い線の先を縦の棒 (1本の線) でつなぎ、3本の端が会う所に接点の丸を打つ。
/// 電源・接地の組は印を1つだけ (電源は棒の上の端、接地は下の端)。内部・出ていくネットの組は配線の端が1つ
/// (相手に近い棒の端。相手が上下にあるなら棒の真ん中)。
fn decorate(
    model: &DraftModel,
    plans: &[UnitPlan],
    sheet: &Sheet,
    ids: &[crate::model::EntityId],
    defs: &[SymbolDef],
    pitch: f64,
) -> Result<Deco, String> {
    let points: BTreeMap<(crate::model::EntityId, String), Point> = pin_points(sheet, defs)
        .into_iter()
        .map(|p| ((p.entity, p.number), p.pos))
        .collect();
    let index = DefIndex::new(defs);
    let groups = placed_groups(model, plans);
    let pin_pos = |u: usize, p: usize| -> Result<Point, String> {
        let unit = &model.units[u];
        points
            .get(&(ids[u], unit.pins[p].key.clone()))
            .copied()
            .ok_or_else(|| format!("The unit {} has no pin {} on the new sheet", unit.reference, unit.pins[p].key))
    };
    // ネットごとの、このシートの組の数とピンの数
    let mut group_count = vec![0usize; model.nets.len()];
    for g in &groups {
        group_count[g.net] += 1;
    }
    let mut pin_count = vec![0usize; model.nets.len()];
    for unit in &model.units {
        for pin in &unit.pins {
            pin_count[pin.net] += 1;
        }
    }
    // 出ていくネットのタグは、列 → 列の中の行 → 箱の中の並びで最初の組に付ける
    let mut tag_group: BTreeMap<usize, usize> = BTreeMap::new();
    for (i, g) in groups.iter().enumerate() {
        if model.nets[g.net].class == NetClass::Leaving {
            tag_group.entry(g.net).or_insert(i);
        }
    }
    let mut out = Deco { items: Vec::new(), terminals: Vec::new(), tagged: Vec::new(), power: 0, ground: 0 };
    for (gi, g) in groups.iter().enumerate() {
        let (u, net) = (g.u, &model.nets[g.net]);
        let (dx, dy) = unit_vec(g.dir);
        let mut roots = Vec::new();
        for &p in &g.pins {
            roots.push(pin_pos(u, p)?);
        }
        let ends: Vec<Point> = roots.iter().map(|r| Point::new(r.x + dx * g.len, r.y + dy * g.len)).collect();
        let (first, last) = (ends[0], ends[ends.len() - 1]);
        let mut items: Vec<Entity> = Vec::new();
        for (r, e) in roots.iter().zip(&ends) {
            items.push(stub_wire(*r, *e));
        }
        if ends.len() >= 2 {
            // 棒: 短い線の先を結ぶ1本の線。途中のピンの短い線の先は3本が会う所
            items.push(stub_wire(first, last));
            for e in &ends[1..ends.len() - 1] {
                items.push(junction_at(*e));
            }
        }
        match net.class {
            NetClass::Power | NetClass::Ground => {
                let ground = net.class == NetClass::Ground;
                let anchor = if ground { last } else { first };
                let (lead, at) = mark_lead(&index, net, anchor, g.dir, pitch)?;
                items.push(Entity::Wire(default_wire(lead)));
                if ends.len() >= 2 {
                    // 棒の端・短い線・引き出しの3本が会う
                    items.push(junction_at(anchor));
                }
                items.push(Entity::Symbol(mark_instance(&index, net, at)?));
                if ground {
                    out.ground += 1;
                } else {
                    out.power += 1;
                }
            }
            NetClass::Internal | NetClass::Leaving => {
                let wired = group_count[g.net] >= 2;
                // 配線の端: 相手に近い棒の端。相手が棒の上にも下にもあれば棒の真ん中
                let t = if !wired || ends.len() == 1 {
                    first
                } else {
                    let mut above = false;
                    let mut below = false;
                    for g2 in groups.iter().filter(|g2| g2.net == g.net && g2.u != u) {
                        for &p2 in &g2.pins {
                            let y = pin_pos(g2.u, p2)?.y;
                            above |= y < first.y - 1e-9;
                            below |= y > last.y + 1e-9;
                        }
                    }
                    match (above, below) {
                        (true, true) => Point::new(first.x, grid_down((first.y + last.y) / 2.0)),
                        (false, true) => last,
                        _ => first,
                    }
                };
                if ends.iter().all(|e| e.distance_to(&t) > 1e-6) {
                    items.push(junction_at(t));
                }
                let is_tag = tag_group.get(&g.net) == Some(&gi);
                if is_tag {
                    // 配線もあるネットの札は、棒の端の先 (枝の端) に下げる。配線は端から出て、札の手前で相手へ曲がるので、
                    // 旗の上も脇も通らない。配線の無いネットの札は端そのもの
                    let reach = if wired { TAG_BRANCH_MM } else { 0.0 };
                    let tag_at = Point::new(t.x + dx * reach, t.y + dy * reach);
                    items.push(Entity::NetLabel(tag_label(&net.name, tag_at, g.dir, net.dest.as_ref())));
                    out.tagged.push((net.name.clone(), pin_count[g.net]));
                    if wired {
                        out.terminals.push(Terminal { net: g.net, at: t, dir: g.dir, tag_at: Some(tag_at), is_tag: false, top: first });
                        out.terminals.push(Terminal { net: g.net, at: tag_at, dir: g.dir.opp(), tag_at: None, is_tag: true, top: tag_at });
                    }
                } else if wired {
                    out.terminals.push(Terminal { net: g.net, at: t, dir: g.dir, tag_at: None, is_tag: false, top: first });
                }
            }
        }
        out.items.extend(items.into_iter().map(|entity| DecoItem { unit: u, entity }));
    }
    Ok(out)
}

fn junction_at(at: Point) -> Entity {
    let id = uuid::Uuid::new_v4();
    Entity::Junction(crate::model::Junction { id, at })
}

/// 札の旗の中に書かれうる行き先の文字のうち、いちばん幅の広いもの: 自分の `tag_dest`、`reserve`
/// (確保する文字: 後から他のシートが書く名前)。
fn widest_dest<'a>(label: &'a NetLabel, reserve: &'a str, font: f64) -> &'a str {
    match crate::net_tag::tag_dest(label) {
        Some(d) if text_width(d, font) > text_width(reserve, font) => d,
        _ => reserve,
    }
}

/// 札の旗に後から書かれる他のシートの名前のために確保する文字: 開いているプロジェクトの最も長いシート名か、
/// [`RESERVED_DEST_CHARS`] 文字の、幅の広い方。
fn reserved_dest(open: &Project) -> String {
    let filler = "M".repeat(RESERVED_DEST_CHARS);
    open.sheets
        .iter()
        .map(|s| s.name.trim().to_string())
        .chain(std::iter::once(filler))
        .fold(String::new(), |best, n| if text_width(&n, 1.0) > text_width(&best, 1.0) { n } else { best })
}

/// 行き先の名前の集合から、旗が持ちうる最も広い文字: 1つならその名前、2つ以上なら幅の広い2つを
/// [`crate::xref::tag_dest_names`] と同じつなぎ方でつないだもの、3つ以上ならさらに「他」。空なら `None`。
/// 順序は幅の降順、同じ幅なら自然順で決まり、ハッシュの順序には依らない。
pub(crate) fn widest_dest_text(names: &BTreeSet<String>) -> Option<String> {
    let mut by_width: Vec<&String> = names.iter().collect();
    by_width.sort_by(|a, b| text_width(b, 1.0).total_cmp(&text_width(a, 1.0)).then_with(|| natural_cmp(a, b)));
    let sep = crate::xref::DEST_SEPARATOR;
    match by_width.as_slice() {
        [] => None,
        [a] => Some((*a).clone()),
        [a, b] => Some(format!("{a}{sep}{b}")),
        [a, b, ..] => Some(format!("{a}{sep}{b}{}", crate::xref::DEST_MORE)),
    }
}

/// 札の旗に後から書かれる行き先のために確保する文字の決め方。
#[derive(Debug, Clone)]
pub(crate) struct Reserve {
    /// どの札にも確保する文字 ([`reserved_dest`])。`planned_sheets` を使うときは空。
    floor: String,
    /// 札の名前 → 確保する文字。従来の動き (`planned_sheets` が空) では、開いているプロジェクトの同じ名前を持つシートの
    /// 名前を旗に書かれるとおりにつないだ文字 ([`crate::xref::tag_dest_names`]) で、`floor` より広いときだけ使う。
    /// `planned_sheets` を使うときは、その札が将来持ちうる行き先だけの文字 (行き先が無ければ入れない = 場所なし)。
    joined: BTreeMap<String, String>,
    /// `planned_sheets` を使う: 札ごとに `joined` の文字だけを確保し、床も10文字の下限も無い。
    exact: bool,
}

impl Reserve {
    /// 床だけ (つなぎ方を調べない)。
    #[cfg(test)]
    fn floor_only(open: &Project) -> Self {
        Reserve { floor: reserved_dest(open), joined: BTreeMap::new(), exact: false }
    }

    /// 札ごとの確保する文字を決める。
    ///
    /// `planned_sheets` が空なら従来どおり: 床 (開いているプロジェクトの最も長いシート名か10文字) と、同じ名前を
    /// すでに持つシートの名前をつないだ文字。
    ///
    /// 空でなければ、ネット N の札ごとに、行き先の名前の集合は
    /// - 開いているプロジェクトのシートのうち、すでに名前が N のネットラベルを持つもの (書き出しが行き先に使うのと同じ調べ方)、
    /// - 計画の項目のうち、まだ無く、自分でもなく、載せる機器・ピンが分かっていて N のピンを1つ以上載せるもの、
    /// - 計画の項目のうち、まだ無く、自分でもなく、名前だけのもの (中身が分からないので、どの札の行き先にもなりうる)。
    /// 電源・接地の印は書き出しの行き先に数えられない (印はネットラベルではない) ので、それだけを持つシートは数えない。
    fn of(open: &Project, model: &DraftModel, req: &DraftRequest) -> Self {
        let new_sheet = uuid::Uuid::new_v4(); // まだ無いシート: 開いているプロジェクトの全シートが相手になる
        if req.planned_sheets.is_empty() {
            let mut r = Reserve { floor: reserved_dest(open), joined: BTreeMap::new(), exact: false };
            for net in model.nets.iter().filter(|n| n.class == NetClass::Leaving) {
                if let Some(text) = crate::xref::tag_dest_names(open, new_sheet, &net.name) {
                    r.joined.insert(net.name.trim().to_string(), text);
                }
            }
            return r;
        }
        let existing: BTreeSet<String> = open.sheets.iter().map(|s| s.name.trim().to_string()).collect();
        let mut r = Reserve { floor: String::new(), joined: BTreeMap::new(), exact: true };
        for net in &model.nets {
            let name = net.name.trim().to_string();
            let mut names: BTreeSet<String> = crate::xref::dest_sheet_names(open, new_sheet, &name).into_iter().collect();
            for p in model.planned.iter().filter(|p| !existing.contains(p.name.trim())) {
                if p.nets.as_ref().map_or(true, |nets| nets.contains(&name)) {
                    names.insert(p.name.trim().to_string());
                }
            }
            if let Some(text) = widest_dest_text(&names) {
                r.joined.insert(name, text);
            }
        }
        r
    }

    /// 名前が `name` の札に確保する文字。
    fn text(&self, name: &str) -> &str {
        let joined = self.joined.get(name.trim());
        if self.exact {
            return joined.map_or("", String::as_str);
        }
        match joined {
            Some(j) if text_width(j, 1.0) > text_width(&self.floor, 1.0) => j,
            _ => &self.floor,
        }
    }
}

/// 札の旗の外形 (先端を含む)。行き先には `reserve` を含めて最も広い文字を使う。札でなければ `None`。
pub(crate) fn flag_rect(label: &NetLabel, label_font: f64, style: XrefStyle, reserve: &str) -> Option<Rect> {
    let dest = widest_dest(label, reserve, label_font);
    let tag = crate::net_tag::net_tag_with(label, label_font, style, Some(dest))?;
    let mut it = tag.points.iter();
    let first = *it.next()?;
    let mut r = Rect::from_corners(first, first);
    it.for_each(|p| r.extend(*p));
    Some(r)
}

/// 置いたものが占める矩形。札は後から書かれる行き先 (`reserve`) の分まで広げる。
fn item_rect(entity: &Entity, defs: &[SymbolDef], sheet: &Sheet, label_font: f64, reserve: &Reserve) -> Option<Rect> {
    match entity {
        Entity::Wire(w) => {
            let mut it = w.points.iter();
            let first = *it.next()?;
            let mut r = Rect::from_corners(first, first);
            it.for_each(|p| r.extend(*p));
            Some(r)
        }
        Entity::NetLabel(l) => Some(
            flag_rect(l, label_font, sheet.xref_style, reserve.text(&l.name)).unwrap_or_else(|| Rect::from_corners(l.at, l.at)),
        ),
        Entity::Symbol(s) => {
            let index = DefIndex::new(defs);
            let def = index.get(s)?;
            Some(mark_rect(s, &def))
        }
        _ => None,
    }
}

fn union(a: Option<Rect>, b: Rect) -> Option<Rect> {
    Some(match a {
        None => b,
        Some(mut r) => {
            r.extend(b.min);
            r.extend(b.max);
            r
        }
    })
}

/// モデルから新しいシートの配置を作る。何も変えない。図枠は開いているプロジェクトの図枠テンプレート。
pub(crate) fn lay_out(model: &DraftModel, open: &Project, req: &DraftRequest) -> Result<DraftLayout, String> {
    lay_out_in(model, open, req, &crate::frame_template::for_project(open))
}

/// [`lay_out`] の、図枠テンプレートを渡せる版 (別の図枠でのテストが使う)。
pub(crate) fn lay_out_in(
    model: &DraftModel,
    open: &Project,
    req: &DraftRequest,
    template: &crate::frame_template::FrameTemplate,
) -> Result<DraftLayout, String> {
    let pitch = req.pitch_mm;
    let reserve = Reserve::of(open, model, req);
    let units = &model.units;
    if units.is_empty() {
        return Err("No unit carries a pin: nothing to draw".to_string());
    }
    let size = match req.paper {
        DraftPaper::A3 => PaperSize::A3,
        DraftPaper::A4 => PaperSize::A4,
    };
    let new_sheet = |annotation: Option<f64>| {
        let mut s = Sheet::new(req.name.trim(), size, Orientation::Landscape);
        s.xref_style = XrefStyle::SheetName;
        s.annotation_text_mm = annotation;
        // 短い線に「0.75sq 黒」の注記が付くと印・タグの文字に重なる。下書きでは配線注記を描かない
        s.hide_wire_notes = true;
        s
    };
    // 旗の高さがピン間隔より1mm以上小さくなる注記文字高 (ただし下限は注記文字高の下限)
    let tag_font = (pitch - 1.0) / (1.0 + 2.0 * crate::net_tag::TAG_PAD / crate::svg::LABEL_FONT);
    let annotation = (tag_font * crate::model::DEFAULT_ANNOTATION_TEXT_MM / crate::svg::LABEL_FONT)
        .clamp(crate::model::MIN_ANNOTATION_TEXT_MM, crate::model::DEFAULT_ANNOTATION_TEXT_MM);
    let annotation = (annotation * 10.0).floor() / 10.0;
    let probe = new_sheet(Some(annotation));
    let label_font = crate::svg::LABEL_FONT * probe.annotation_scale();

    // 定義: 同梱+ライブラリのシンボルと、印のシンボル
    let mut defs_sheet = new_sheet(Some(annotation));
    let mut wanted_marks: BTreeSet<&str> = BTreeSet::new();
    wanted_marks.insert(POWER_SYMBOL);
    for n in &model.nets {
        if let Some(id) = &n.ground_symbol {
            wanted_marks.insert(id.as_str());
        }
    }
    for id in wanted_marks {
        defs_sheet.entities.insert(
            uuid::Uuid::new_v4(),
            Entity::Symbol(SymbolInstance {
                id: uuid::Uuid::new_v4(),
                symbol_id: id.to_string(),
                at: origin(),
                rotation: 0,
                mirror: false,
                reference: String::new(),
                value: String::new(),
                attrs: BTreeMap::new(),
            }),
        );
    }
    for u in units.iter().filter(|u| !u.is_block) {
        let e = Entity::Symbol(u.source.clone());
        defs_sheet.entities.insert(e.id(), e);
    }
    let defs = sheet_symbol_defs(&defs_sheet);

    let mark_gap = mark_gaps(model, &defs, pitch)?;
    let mut plans = plan_units(model, &defs, pitch, &mark_gap)?;

    // 機器の高さ (箱の高さ、またはシンボルの外形) と、列ごとに積んだときの上端 (並べ替えの目安)
    let heights: Vec<f64> = units
        .iter()
        .zip(&plans)
        .map(|(u, plan)| {
            if u.is_block {
                Ok(block_attrs(u, plan, pitch)?.1 .1)
            } else {
                let inst = unit_instance(u, plan, pitch, origin())?;
                let index = DefIndex::new(&defs);
                let def = index.get(&inst).ok_or_else(|| format!("The symbol of {} has no definition", u.reference))?;
                Ok(symbol_bounds(&inst, &def).map_or(0.0, |r| r.max.y - r.min.y))
            }
        })
        .collect::<Result<_, String>>()?;
    let gap = grid_up(GAP_PITCHES * pitch);
    let mut cursor: BTreeMap<usize, f64> = BTreeMap::new();
    let mut tops = vec![0.0; units.len()];
    let mut mids = vec![0.0; units.len()];
    for (u, unit) in units.iter().enumerate() {
        let c = cursor.entry(unit.column).or_insert(0.0);
        tops[u] = *c + CAPTION_MM;
        mids[u] = tops[u] + heights[u] / 2.0;
        *c = tops[u] + heights[u] + gap;
    }
    order_pins(model, &mut plans, &tops, &mids, pitch);

    // 原点に置いて、印・タグまで含めた各機器の占める矩形を測る
    let origin_instances: Vec<SymbolInstance> = units
        .iter()
        .zip(&plans)
        .map(|(u, plan)| unit_instance(u, plan, pitch, origin()))
        .collect::<Result<_, String>>()?;
    let mut scratch = new_sheet(Some(annotation));
    for inst in &origin_instances {
        scratch.entities.insert(inst.id, Entity::Symbol(inst.clone()));
    }
    let scratch_ids: Vec<_> = origin_instances.iter().map(|i| i.id).collect();
    let probe_deco = decorate(model, &plans, &scratch, &scratch_ids, &defs, pitch)?;
    let mut extent: Vec<Option<Rect>> = vec![None; units.len()];
    for (u, inst) in origin_instances.iter().enumerate() {
        let index = DefIndex::new(&defs);
        let def = index.get(inst).ok_or_else(|| format!("The symbol of {} has no definition", units[u].reference))?;
        if let Some(mut r) = symbol_bounds(inst, &def) {
            r.extend(Point::new(r.min.x, r.min.y - CAPTION_MM));
            extent[u] = union(extent[u], r);
        }
    }
    for item in &probe_deco.items {
        if let Some(r) = item_rect(&item.entity, &defs, &scratch, label_font, &reserve) {
            extent[item.unit] = union(extent[item.unit], r);
        }
    }
    let extent: Vec<Rect> = extent
        .into_iter()
        .map(|r| r.unwrap_or_else(|| Rect::from_corners(origin(), origin())))
        .collect();

    // 描画範囲 (図枠の内側から余白を引いたもの) と表題欄
    let (frame, title) = crate::arrange::frame_areas(template, &probe);
    let area_x0 = grid_up(frame.0 + AREA_MARGIN_MM);
    let area_x1 = grid_down(frame.2 - AREA_MARGIN_MM);
    let area_y0 = grid_up(frame.1 + AREA_MARGIN_MM);
    let area_bottom = grid_down(frame.3 - AREA_MARGIN_MM);

    // 列: 機器のある列だけ、左から右へ。列の左右の張り出し (機器の原点を列の中心線に揃える)
    let columns: Vec<usize> = {
        let mut c: Vec<usize> = units.iter().map(|u| u.column).collect();
        c.sort_unstable();
        c.dedup();
        c
    };
    let members = |c: usize| -> Vec<usize> { (0..units.len()).filter(|&u| units[u].column == c).collect() };
    let left_of: Vec<f64> = columns.iter().map(|&c| grid_up(members(c).iter().map(|&u| -extent[u].min.x).fold(0.0, f64::max))).collect();
    let right_of: Vec<f64> = columns.iter().map(|&c| grid_up(members(c).iter().map(|&u| extent[u].max.x).fold(0.0, f64::max))).collect();
    let widths: Vec<f64> = left_of.iter().zip(&right_of).map(|(l, r)| l + r).collect();
    // 通り道: 列kと列k+1のあいだは、20mmと、10mm + そこを横切る (両側に端がある) ネット1本につき2.5mm の大きい方
    let column_pos: BTreeMap<usize, usize> = columns.iter().enumerate().map(|(k, &c)| (c, k)).collect();
    let mut net_span: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
    for unit in units {
        for pin in &unit.pins {
            if matches!(model.nets[pin.net].class, NetClass::Internal | NetClass::Leaving) {
                let k = column_pos[&unit.column];
                let e = net_span.entry(pin.net).or_insert((k, k));
                e.0 = e.0.min(k);
                e.1 = e.1.max(k);
            }
        }
    }
    // 同じ列の機器どうしをつなぐネット: その列の右の通り道 (最後の列なら左の通り道) を通るので、そこで数える
    let mut same_column: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for (k, &c) in columns.iter().enumerate() {
        let last = k + 1 == columns.len();
        if last && k == 0 {
            continue;
        }
        let side = if last { Side::Left } else { Side::Right };
        let mut units_of: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
        for u in members(c) {
            for (p, pin) in units[u].pins.iter().enumerate() {
                if matches!(model.nets[pin.net].class, NetClass::Internal | NetClass::Leaving) && plans[u].side[p] == side {
                    units_of.entry(pin.net).or_default().insert(u);
                }
            }
        }
        let channel_index = if last { k - 1 } else { k };
        same_column.entry(channel_index).or_default().extend(units_of.into_iter().filter(|(_, us)| us.len() >= 2).map(|(n, _)| n));
    }
    let channel = |k: usize| -> f64 {
        let mut nets: BTreeSet<usize> = net_span.iter().filter(|(_, (lo, hi))| *lo <= k && *hi > k).map(|(&n, _)| n).collect();
        nets.extend(same_column.get(&k).into_iter().flatten().copied());
        grid_up(f64::max(MIN_CHANNEL_MM, CHANNEL_BASE_MM + CHANNEL_PER_NET_MM * nets.len() as f64))
    };
    // 詰めて並べる: 最初の列は描画範囲の左端から、次の列は前の列の右の張り出し + 通り道から。
    // 箱・短い線・棒・札・印のどれかが右の縁を越える列があれば断る
    let mut column_x = Vec::new(); // 列の左端
    let mut x = area_x0;
    for (k, w) in widths.iter().enumerate() {
        column_x.push(x);
        if x + w > area_x1 + 1e-9 {
            return Err(format!(
                "Column {} does not fit the width: the columns need {:.1} mm more than the drawing area gives (with a channel of at least {} mm between columns)",
                columns[k] + 1,
                x + w - area_x1,
                MIN_CHANNEL_MM
            ));
        }
        x += w + channel(k);
    }

    // 縦: 列ごとに上から積む。表題欄に重なる列は表題欄の上までしか使えない
    let mut positions = vec![origin(); units.len()];
    for (k, &c) in columns.iter().enumerate() {
        let (xl, xr) = (column_x[k], column_x[k] + widths[k]);
        let end = if xr > title.0 - GRID_MM && xl < title.2 + GRID_MM { grid_down(title.1 - GRID_MM) } else { area_bottom };
        let cx = column_x[k] + left_of[k];
        let mut cursor = area_y0;
        for u in members(c) {
            let top = grid_up(-extent[u].min.y);
            let bottom = grid_up(extent[u].max.y);
            positions[u] = Point::new(cx, cursor + top);
            cursor += top + bottom + gap;
        }
        let used = cursor - gap - area_y0;
        if used > end - area_y0 + 1e-9 {
            return Err(format!(
                "Column {} is {:.1} mm taller than the drawing area: move a unit to another sheet",
                c + 1,
                used - (end - area_y0)
            ));
        }
    }

    // 置いて、印・タグ・スタブを作る
    let mut sheet = new_sheet(Some(annotation));
    let mut ids = Vec::new();
    for (u, unit) in units.iter().enumerate() {
        let inst = unit_instance(unit, &plans[u], pitch, positions[u])?;
        ids.push(inst.id);
        sheet.entities.insert(inst.id, Entity::Symbol(inst));
    }
    let deco = decorate(model, &plans, &sheet, &ids, &defs, pitch)?;
    // 配線が近づいてはいけないもの: 札の旗 (後から書かれる他のシートの名前の分まで) と、印の図形と文字
    let keep_out: Vec<Rect> = deco
        .items
        .iter()
        .filter(|item| matches!(item.entity, Entity::NetLabel(_) | Entity::Symbol(_)))
        .filter_map(|item| item_rect(&item.entity, &defs, &sheet, label_font, &reserve))
        .collect();
    for item in deco.items {
        sheet.entities.insert(item.entity.id(), item.entity);
    }
    Ok(DraftLayout {
        sheet,
        terminals: deco.terminals,
        tagged: deco.tagged,
        power_marks: deco.power,
        ground_marks: deco.ground,
        keep_out,
        frame,
        title,
        columns: column_x.iter().zip(&widths).map(|(x, w)| ColumnSpan { left: *x, right: x + w }).collect(),
        reserve,
        label_font,
    })
}

#[cfg(test)]
#[path = "draft_look_tests.rs"]
mod look_tests;

#[cfg(test)]
mod layout_tests {
    use super::*;
    use crate::model::Wire;

    fn open() -> Project {
        Project::new("open")
    }

    fn request(columns: &[&[&str]]) -> DraftRequest {
        DraftRequest {
            name: "Function".into(),
            columns: columns.iter().map(|c| c.iter().map(|r| r.to_string()).collect()).collect(),
            pins: BTreeMap::new(),
            power_nets: Vec::new(),
            ground_nets: None,
            destinations: BTreeMap::new(),
            paper: DraftPaper::A3,
            pitch_mm: 5.0,
            planned_sheets: Vec::new(),
        }
    }

    fn model_of(req: &DraftRequest) -> DraftModel {
        build_model(&fixture_source(), &open(), req).expect("the request is valid")
    }

    fn layout(req: &DraftRequest) -> DraftLayout {
        lay_out(&model_of(req), &open(), req).expect("the layout fits")
    }

    fn refusal(req: &DraftRequest) -> String {
        lay_out(&model_of(req), &open(), req).expect_err("the layout is refused")
    }

    fn defs_of(l: &DraftLayout) -> Vec<SymbolDef> {
        sheet_symbol_defs(&l.sheet)
    }

    fn symbols(l: &DraftLayout) -> Vec<SymbolInstance> {
        l.sheet.entities.values().filter_map(|e| if let Entity::Symbol(s) = e { Some(s.clone()) } else { None }).collect()
    }

    fn unit_symbol(l: &DraftLayout, reference: &str) -> SymbolInstance {
        symbols(l).into_iter().find(|s| s.reference == reference).expect("the unit is on the sheet")
    }

    fn marks(l: &DraftLayout) -> Vec<SymbolInstance> {
        symbols(l).into_iter().filter(|s| s.reference.is_empty()).collect()
    }

    fn labels(l: &DraftLayout) -> Vec<NetLabel> {
        l.sheet.entities.values().filter_map(|e| if let Entity::NetLabel(n) = e { Some(n.clone()) } else { None }).collect()
    }

    fn wires(l: &DraftLayout) -> Vec<Wire> {
        l.sheet.entities.values().filter_map(|e| if let Entity::Wire(w) = e { Some(w.clone()) } else { None }).collect()
    }

    fn pin(l: &DraftLayout, reference: &str, key: &str) -> crate::route::PinPt {
        pin_points(&l.sheet, &defs_of(l))
            .into_iter()
            .find(|p| p.reference == reference && p.number == key)
            .unwrap_or_else(|| panic!("{reference}.{key} is on the sheet"))
    }

    fn is_left(l: &DraftLayout, reference: &str, key: &str) -> bool {
        pin(l, reference, key).pos.x < unit_symbol(l, reference).at.x
    }

    /// Everything that depends on the layout but not on the random entity ids, as sorted text.
    fn fingerprint(l: &DraftLayout) -> Vec<String> {
        let mut out: Vec<String> = l
            .sheet
            .entities
            .values()
            .map(|e| match e {
                Entity::Symbol(s) => format!("S {} {} {} {:?} {} {} {:?}", s.symbol_id, s.reference, s.value, s.at, s.rotation, s.mirror, s.attrs),
                Entity::Wire(w) => format!("W {:?}", w.points),
                Entity::NetLabel(n) => format!("L {} {:?} {:?}", n.name, n.at, n.attrs),
                other => format!("? {other:?}"),
            })
            .collect();
        out.sort();
        out.extend(l.terminals.iter().map(|t| format!("T {} {:?} {:?}", t.net, t.at, t.dir)));
        out.extend(l.tagged.iter().map(|t| format!("G {t:?}")));
        out
    }

    fn block_rows(l: &DraftLayout, reference: &str, side: &str) -> Vec<String> {
        let inst = unit_symbol(l, reference);
        let pins: serde_json::Value = serde_json::from_str(&inst.attrs[block::BLOCK_PINS_ATTR]).unwrap();
        pins[side]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| if r.get("gap").is_some() { "gap".to_string() } else { r["no"].as_str().unwrap_or_else(|| r["name"].as_str().unwrap()).to_string() })
            .collect()
    }

    fn on_grid(v: f64) -> bool {
        let k = v / GRID_MM;
        (k - k.round()).abs() < 1e-6
    }

    // ---- boxes

    /// A box shows only the pins the model carries, is marked as a view, and has pin text at least 2.2 mm high.
    /// 箱は、運ぶことになったピンだけを持ち、部分表示の印が付き、ピンの文字は2.2mm以上の高さになる。
    #[test]
    fn a_box_carries_only_its_pins_and_is_a_view() {
        let l = layout(&request(&[&["U1"], &["U2"]]));
        let u1 = unit_symbol(&l, "U1");
        assert_eq!(u1.symbol_id, BLOCK_SYMBOL_ID);
        assert!(block::is_view(&u1));
        let all: Vec<String> = ["left", "right"].iter().flat_map(|s| block_rows(&l, "U1", s)).filter(|r| r != "gap").collect();
        assert_eq!(all.len(), 5, "pin 6 is on a one-pin net and is not carried: {all:?}");
        assert!(!all.contains(&"6".to_string()));
        for pitch in [5.0, 2.5] {
            let mut r = request(&[&["U1"], &["U2"]]);
            r.pitch_mm = pitch;
            let u1 = unit_symbol(&layout(&r), "U1");
            assert!(u1.attrs[block::BLOCK_PIN_TEXT_ATTR].parse::<f64>().unwrap() >= 2.2);
            assert_eq!(u1.attrs[block::BLOCK_PITCH_ATTR], if pitch == 5.0 { "5" } else { "2.5" });
        }
    }

    /// A library symbol is not turned into a box: it is placed again with its own reference, value and rotation.
    /// 抵抗のようなライブラリのシンボルは箱にせず、元の参照記号・値・向きのまま同じシンボルを置く。
    #[test]
    fn a_library_symbol_is_placed_as_it_is() {
        let l = layout(&request(&[&["U1"], &["R1"]]));
        let r1 = unit_symbol(&l, "R1");
        assert_eq!(r1.symbol_id, "resistor");
        assert_eq!(r1.value, "10k");
        assert_eq!(r1.rotation, 0);
    }

    // ---- sides

    /// A pin whose peers on the sheet are all in columns to the left goes on the left side of its box.
    /// 相手のピンがすべて左の列にあるピンは、箱の左側に出る。
    #[test]
    fn a_pin_with_peers_on_the_left_goes_left() {
        let l = layout(&request(&[&["U1"], &["U2"]]));
        assert!(is_left(&l, "U2", "3"), "U2.3 meets U1.5 on its left");
    }

    /// A pin whose peers are all in columns to the right goes on the right side of its box.
    /// 相手のピンがすべて右の列にあるピンは、箱の右側に出る。
    #[test]
    fn a_pin_with_peers_on_the_right_goes_right() {
        let l = layout(&request(&[&["U1"], &["U2"]]));
        assert!(!is_left(&l, "U1", "5"), "U1.5 meets U2.3 on its right");
    }

    /// With the same number of peers on both sides, the pin goes to the left.
    /// 相手が左右に同じ数あるときは、左側に出る。
    #[test]
    fn a_tie_goes_left() {
        let l = layout(&request(&[&["U1"], &["U2"], &["TB1"]]));
        assert!(is_left(&l, "U2", "4"), "U2.4 has one peer on each side");
    }

    /// A pin with no peer on the sheet goes to the right side only for a unit in the last column; otherwise to the left.
    /// このシートに相手のいないピンは、最後の列の機器なら右側、それ以外は左側に出る。
    #[test]
    fn a_pin_with_no_peer_goes_right_only_in_the_last_column() {
        let alone = layout(&request(&[&["U1"]]));
        assert!(!is_left(&alone, "U1", "5"), "the only column is the last column");
        let first = layout(&request(&[&["U1"], &["M1"]]));
        assert!(is_left(&first, "U1", "5"), "U1 is not in the last column");
    }

    /// Power pins are the first rows of the left side and ground pins the last rows, each group set off by one spacer row.
    /// 電源のピンは左側の最初の行、接地のピンは左側の最後の行に並び、信号のピンとは1行のすき間で区切られる。
    #[test]
    fn power_pins_come_first_and_ground_pins_last_on_the_left_side() {
        let mut r = request(&[&["U1"], &["U2"]]);
        r.power_nets = vec!["P24X".into()];
        let l = layout(&r);
        assert_eq!(block_rows(&l, "U1", "left"), ["1", "gap", "3", "gap", "2"]);
    }

    /// A unit that has only power and ground pins on the sheet is still drawn, with the marks and nothing else.
    /// 電源・接地のピンしか持たない機器も、印だけを付けた箱として描かれる。
    #[test]
    fn a_unit_with_only_power_pins_is_drawn_with_marks() {
        let mut r = request(&[&["TB1"]]);
        r.pins.insert("TB1".into(), vec!["1".into()]);
        r.power_nets = vec!["P24X".into()];
        let l = layout(&r);
        assert_eq!(unit_symbol(&l, "TB1").symbol_id, BLOCK_SYMBOL_ID);
        assert_eq!(l.power_marks, 1);
        assert!(l.terminals.is_empty() && labels(&l).is_empty());
    }

    // ---- order

    /// Pins are ordered so that the wires between two boxes do not cross, even when the source order is reversed.
    /// 2つの箱のあいだの線が交差しないように、元の順序が逆でもピンの並びを入れ替える。
    #[test]
    fn straight_connections_do_not_cross() {
        let l = layout(&request(&[&["U1"], &["U2"]]));
        // U1.4 - U2.4 (SIG_B) and U1.5 - U2.3 (SIG_A): in key order they would cross
        let (a4, a5) = (pin(&l, "U1", "4").pos.y, pin(&l, "U1", "5").pos.y);
        let (b3, b4) = (pin(&l, "U2", "3").pos.y, pin(&l, "U2", "4").pos.y);
        assert_eq!(a5 < a4, b3 < b4, "the order of the two wires is the same at both ends");
    }

    /// The same request always gives the same layout.
    /// 同じ入力からは、いつも同じ配置ができる。
    #[test]
    fn the_same_request_gives_the_same_layout() {
        let mut r = request(&[&["U1", "M1"], &["U2"], &["TB1", "R1"]]);
        r.power_nets = vec!["P24X".into()];
        assert_eq!(fingerprint(&layout(&r)), fingerprint(&layout(&r)));
    }

    // ---- marks

    /// The power pin of each unit gets a rail mark named after the net: from the end of its stub (as long as the widest number of that side asks) a lead goes one pitch up and then away from the box, and the mark stands at its end.
    /// 電源のピンには、ネット名を値にした電源の印が付く。短い線 (その辺のいちばん広い番号に合わせた長さ) の先から、引き出しがピン間隔ぶん上へ伸びて箱から離れる向きへ曲がり、その先に印が立つ。
    #[test]
    fn each_power_pin_gets_its_own_rail_mark_named_after_the_net() {
        let mut r = request(&[&["U1"], &["U2"], &["TB1"]]);
        r.power_nets = vec!["P24X".into()];
        let l = layout(&r);
        let rails: Vec<_> = marks(&l).into_iter().filter(|m| m.value == "P24X").collect();
        assert_eq!(rails.len(), 3);
        assert_eq!(l.power_marks, 3);
        for (reference, key) in [("U1", "1"), ("U2", "1"), ("TB1", "1")] {
            let p = pin(&l, reference, key).pos;
            let end = wires(&l).iter().find_map(|w| (w.points.len() == 2 && w.points[0] == p).then(|| w.points[1])).expect("the stub is a real wire");
            assert!((p.x - end.x) >= STUB_MM - 1e-9 && p.y == end.y);
            let lead = wires(&l).into_iter().find(|w| w.points.len() == 3 && w.points[0] == end).expect("a lead leaves the stub end");
            assert_eq!(lead.points[1], Point::new(end.x, end.y - 5.0), "one pitch up");
            assert!(lead.points[2].y == lead.points[1].y && lead.points[2].x < end.x, "then away from the box");
            assert!(rails.iter().any(|m| m.symbol_id == "power_rail" && m.rotation == 0 && m.at == lead.points[2]), "{reference}.{key}");
        }
    }

    /// A ground mark uses the mark symbol the model names for that net, with the net name as its value when the symbol takes it from the value.
    /// 接地の印は、モデルが示す元の図の印のシンボルを使う。値からネット名を取るシンボルなら、値にネット名を入れる。
    #[test]
    fn ground_uses_the_marks_symbol_of_the_source() {
        let r = request(&[&["M1"]]);
        let mut m = model_of(&r);
        for n in m.nets.iter_mut().filter(|n| n.class == NetClass::Ground) {
            n.ground_symbol = Some("earth_protective".into());
        }
        let l = lay_out(&m, &open(), &r).unwrap();
        let mark = marks(&l).into_iter().next().expect("M1.2 is a ground pin");
        assert_eq!(mark.symbol_id, "earth_protective");
        assert_eq!(l.ground_marks, 1);
        let fixture = layout(&r);
        assert_eq!(marks(&fixture)[0].value, "GND", "the fixture marks are value-named rails");
    }

    /// When the source has no ground mark, the library rail mark is used, pointing down, with the net name as its value.
    /// 元の図に接地の印が無いときは、ライブラリの電源の印を下向き (180度回転) にして、ネット名を値にして置く。
    #[test]
    fn ground_falls_back_to_the_library_mark() {
        let r = request(&[&["M1"]]);
        let mut m = model_of(&r);
        for n in m.nets.iter_mut() {
            n.ground_symbol = None;
        }
        let l = lay_out(&m, &open(), &r).unwrap();
        let mark = marks(&l).into_iter().next().unwrap();
        assert_eq!((mark.symbol_id.as_str(), mark.rotation, mark.value.as_str()), ("power_rail", 180, "GND"));
    }

    // ---- tags

    /// A leaving net gets one tag, on the first pin in column order; when the net is also wired on the sheet, the tag hangs 5 mm beyond that pin's stub end, and the stub ends and the tag are all ends of the wiring.
    /// 出ていくネットには札が1つだけ付き、列の順で最初のピンに下がる。このシートでも配線するネットなら、札はそのピンの短い線の先のさらに5mm先にあり、短い線の先と札のすべてが配線の端になる。
    #[test]
    fn a_leaving_net_gets_one_tag_at_the_first_pin_in_column_order() {
        let l = layout(&request(&[&["U1"], &["U2"]]));
        let tags: Vec<_> = labels(&l).into_iter().filter(|n| n.name == "SIG_A").collect();
        assert_eq!(tags.len(), 1, "R1 is not on the sheet, so SIG_A leaves it");
        let p = pin(&l, "U1", "5").pos;
        let stub = Point::new(p.x + STUB_MM, p.y);
        assert_eq!(tags[0].at, Point::new(stub.x + TAG_BRANCH_MM, p.y), "U1 is in the first column");
        assert!(l.tagged.contains(&("SIG_A".to_string(), 2)));
        let net = l.terminals.iter().find(|t| t.at == tags[0].at).map(|t| t.net).expect("the tag is a terminal");
        let ends: Vec<Point> = l.terminals.iter().filter(|t| t.net == net).map(|t| t.at).collect();
        assert_eq!(ends.len(), 3, "{ends:?}");
        assert!(ends.contains(&stub), "the tagged pin is a terminal at its stub end");
        let other = pin(&l, "U2", "3").pos;
        assert!(ends.iter().any(|e| (e.x - other.x).abs() >= STUB_MM - 1e-9 && e.y == other.y), "the other pin is a terminal at its stub end");
        let tag_end = l.terminals.iter().find(|t| t.at == tags[0].at).unwrap();
        assert_eq!(tag_end.dir, Dir::W, "the wire enters the tag from the box side");
        assert!(!wires(&l).iter().any(|w| w.points.contains(&tags[0].at)), "the layout draws no wire to a wired tag: the router does");
    }

    /// A leaving net with one carried pin has a tag but no terminal.
    /// 持っているピンが1本だけの出ていくネットには、タグが付くが、配線の端は作らない。
    #[test]
    fn a_leaving_net_with_one_pin_has_no_terminal() {
        let l = layout(&request(&[&["U1"]]));
        assert!(!labels(&l).is_empty());
        assert!(l.terminals.is_empty());
    }

    /// A tag on a right-side pin points outward (tag_flow = out); a tag on a left-side pin has its body on the outer side too, and no flow.
    /// 右側のピンのタグは外向き (tag_flow = out)、左側のピンのタグは本体が外側 (左) に伸びる向きで、out は付かない。
    #[test]
    fn a_tag_on_a_right_side_pin_points_outward() {
        let right = layout(&request(&[&["U1"], &["U2"]]));
        let tag = labels(&right).into_iter().find(|n| n.name == "SIG_B").unwrap();
        assert_eq!(tag.attrs.get("tag").map(String::as_str), Some("left"), "the body grows to the right, away from the box");
        assert_eq!(tag.attrs.get("tag_flow").map(String::as_str), Some("out"));
        let left = layout(&request(&[&["U2"], &["TB1"]]));
        let tag = labels(&left).into_iter().find(|n| n.name == "SIG_A").unwrap();
        assert_eq!(tag.attrs.get("tag").map(String::as_str), Some("right"), "the body grows to the left, away from the box");
        assert!(!tag.attrs.contains_key("tag_flow"));
    }

    /// The destinations option becomes the destination text of the tag.
    /// destinations に書いた行き先は、タグの行き先の文字になる。
    #[test]
    fn a_destination_option_becomes_the_tags_destination() {
        let mut r = request(&[&["U1"], &["U2"]]);
        r.destinations.insert("SIG_A".into(), "cabinet B".into());
        let l = layout(&r);
        let tag = labels(&l).into_iter().find(|n| n.name == "SIG_A").unwrap();
        assert_eq!(tag.attrs.get("tag_dest").map(String::as_str), Some("cabinet B"));
        let other = labels(&l).into_iter().find(|n| n.name == "SIG_B").unwrap();
        assert!(!other.attrs.contains_key("tag_dest"));
    }

    /// Tags on adjacent pins do not touch, at 5 mm and at 2.5 mm pitch.
    /// 隣り合うピンのタグは、ピン間隔が5mmでも2.5mmでも、互いに触れない。
    #[test]
    fn tags_on_adjacent_pins_do_not_touch() {
        for pitch in [5.0, 2.5] {
            let mut r = request(&[&["U1"]]);
            r.pitch_mm = pitch;
            r.destinations.insert("SIG_A".into(), "cabinet B".into());
            let l = layout(&r);
            let font = crate::svg::LABEL_FONT * l.sheet.annotation_scale();
            let rects: Vec<Rect> = labels(&l)
                .iter()
                .map(|n| {
                    let tag = crate::net_tag::net_tag_with(n, font, l.sheet.xref_style, crate::net_tag::tag_dest(n)).unwrap();
                    let mut it = tag.points.iter();
                    let first = *it.next().unwrap();
                    let mut r = Rect::from_corners(first, first);
                    it.for_each(|p| r.extend(*p));
                    r
                })
                .collect();
            assert!(rects.len() >= 3, "U1 has three adjacent tagged pins");
            for (i, a) in rects.iter().enumerate() {
                for b in &rects[i + 1..] {
                    assert!(!a.overlaps(b), "pitch {pitch}: {a:?} {b:?}");
                }
            }
        }
    }

    /// A tag keeps room for the names other sheets add inside it later: its keep-out holds the flag with ten characters and with the longest sheet name of the open project.
    /// 札は、後から中に書かれる他のシートの名前の場所を空けておく。配線が近づかない範囲は、10文字の行き先と、開いているプロジェクトの最も長いシート名を入れた旗を含む。
    #[test]
    fn a_tag_reserves_room_for_ten_characters_and_the_longest_sheet_name() {
        let mut long = open();
        long.sheets[0].name = "A very long cabinet name".into();
        for (project, texts) in [(open(), vec!["0123456789".to_string()]), (long, vec!["0123456789".to_string(), "A very long cabinet name".to_string()])] {
            let r = request(&[&["U1"], &["U2"]]);
            let m = build_model(&fixture_source(), &project, &r).unwrap();
            let l = lay_out(&m, &project, &r).unwrap();
            let font = crate::svg::LABEL_FONT * l.sheet.annotation_scale();
            let tags = labels(&l);
            assert!(!tags.is_empty());
            for n in &tags {
                for text in &texts {
                    let want = flag_rect(n, font, l.sheet.xref_style, text).unwrap();
                    assert!(
                        l.keep_out.iter().any(|k| k.min.x <= want.min.x + 1e-9 && k.max.x >= want.max.x - 1e-9 && k.min.y <= want.min.y + 1e-9 && k.max.y >= want.max.y - 1e-9),
                        "{} does not keep room for {text:?}", n.name
                    );
                }
            }
        }
    }

    /// An open project without any sheet.
    fn bare() -> Project {
        let mut p = open();
        p.sheets.clear();
        p
    }

    /// The widest destination text of a set of names: one name is that name, two are joined, three or more keep the two widest and the other marker.
    /// 行き先の名前の集合から作る最も広い文字は、1つならその名前、2つならつないだもの、3つ以上なら広い2つに「他」を付けたものになる。
    #[test]
    fn the_widest_destination_text_follows_the_flag_rules() {
        let set = |l: &[&str]| l.iter().map(|s| s.to_string()).collect::<BTreeSet<String>>();
        let sep = crate::xref::DEST_SEPARATOR;
        assert_eq!(widest_dest_text(&set(&[])), None);
        assert_eq!(widest_dest_text(&set(&["Spare relay panel number two"])).as_deref(), Some("Spare relay panel number two"));
        assert_eq!(widest_dest_text(&set(&["Alpha cabinet", "Beta cabinet wide"])), Some(format!("Beta cabinet wide{sep}Alpha cabinet")));
        let three = set(&["Short", "The widest name here", "Second widest name", "Mid size"]);
        assert_eq!(
            widest_dest_text(&three),
            Some(format!("The widest name here{sep}Second widest name{}", crate::xref::DEST_MORE))
        );
    }

    /// Whatever order the sheets end up in, the text a flag prints (the first two names in sheet order, plus the other marker for three or more) is never wider than the reserved text, which joins the two widest names.
    /// シートが最後にどの順に並んでも、旗に書かれる文字 (シート順で最初の2つの名前と、3つ以上なら「他」) は、確保した文字 (いちばん広い2つの名前をつないだもの) より広くならない。
    #[test]
    fn the_reserved_text_covers_every_order_the_export_can_print() {
        let names = ["Ab", "Wide cabinet name", "M", "Second wide one", "漢字の盤"];
        let set: BTreeSet<String> = names.iter().map(|s| s.to_string()).collect();
        let reserved = widest_dest_text(&set).unwrap();
        let mut order: Vec<String> = names.iter().map(|s| s.to_string()).collect();
        // Heap's algorithm: every permutation of the five names
        fn permute(k: usize, v: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
            if k == 1 {
                out.push(v.clone());
                return;
            }
            for i in 0..k {
                permute(k - 1, v, out);
                let j = if k % 2 == 0 { i } else { 0 };
                v.swap(j, k - 1);
            }
        }
        let mut all = Vec::new();
        permute(order.len(), &mut order, &mut all);
        assert_eq!(all.len(), 120);
        for p in all {
            for n in 1..=p.len() {
                let printed = crate::xref::join_dest_names(&p[..n]).unwrap();
                assert!(text_width(&printed, 1.0) <= text_width(&reserved, 1.0) + 1e-9, "{printed} is wider than {reserved}");
            }
        }
    }

    /// A project with one sheet and a request that names the sheets it plans; the model of U1 and TB1 on two columns.
    fn planned_reserve(planned: Vec<PlannedSheet>) -> Reserve {
        let mut r = request(&[&["U1"], &["TB1"]]);
        r.planned_sheets = planned;
        let m = build_model(&fixture_source(), &bare(), &r).unwrap();
        Reserve::of(&bare(), &m, &r)
    }

    fn detailed(name: &str, units: &[&str], pins: &[(&str, &[&str])]) -> PlannedSheet {
        PlannedSheet::Detailed(PlannedSheetDetail {
            name: name.into(),
            units: units.iter().map(|u| u.to_string()).collect(),
            pins: pins.iter().map(|(u, k)| (u.to_string(), k.iter().map(|x| x.to_string()).collect())).collect(),
        })
    }

    /// Name-only planned sheets are possible destinations of every tag, and the own name never counts; no minimum of ten characters is kept.
    /// 名前だけの計画のシートは、どの札の行き先にもなりうる。自分の名前は数えず、10文字の下限は付けない。
    #[test]
    fn name_only_entries_reserve_for_every_tag_and_the_own_name_is_left_out() {
        let r = planned_reserve(vec!["Function".into(), "B".into()]);
        assert_eq!(r.text("SIG_A"), "B");
        assert_eq!(r.text("SIG_B"), "B");
    }

    /// An object entry is a destination only for the nets it carries a pin of: net A goes to one other sheet, net B to two, net C to none.
    /// 項目に機器とピンを書いた計画のシートは、そのピンを載せるネットの札だけの行き先になる。ネットAは他の1枚、Bは2枚、Cはどこへも行かない。
    #[test]
    fn an_object_entry_is_a_destination_only_for_the_nets_it_carries() {
        // U1 pin 5 = SIG_A, pin 4 = SIG_B (see the fixture); U2 pin 3 = SIG_A, pin 4 = SIG_B; TB1 pins decide the rest
        let r = planned_reserve(vec![
            detailed("Alpha cabinet", &["U2"], &[("U2", &["3"])]),
            detailed("Beta cabinet wide", &["U2"], &[("U2", &["3", "4"])]),
            detailed("Function", &["U1"], &[]),
        ]);
        let sep = crate::xref::DEST_SEPARATOR;
        assert_eq!(r.text("SIG_A"), format!("Beta cabinet wide{sep}Alpha cabinet"));
        assert_eq!(r.text("SIG_B"), "Beta cabinet wide");
        assert_eq!(r.text("P24X"), "", "no planned sheet carries P24X");
    }

    /// Object and name-only entries mix: a name-only entry joins every tag, an object entry only its own nets.
    /// 名前だけの項目と機器つきの項目を混ぜると、名前だけの項目は全ての札に、機器つきの項目はそのネットの札にだけ加わる。
    #[test]
    fn object_and_name_only_entries_mix() {
        let r = planned_reserve(vec!["Loose sheet".into(), detailed("Alpha cabinet", &["U2"], &[("U2", &["3"])])]);
        let sep = crate::xref::DEST_SEPARATOR;
        assert_eq!(r.text("SIG_A"), format!("Alpha cabinet{sep}Loose sheet"));
        assert_eq!(r.text("SIG_B"), "Loose sheet");
    }

    /// An entry that has the same name as a sheet of the open project counts through that sheet's labels, not through the plan.
    /// すでにあるシートと同じ名前の計画の項目は、計画ではなく、そのシートのラベルがあるかで数える。
    #[test]
    fn an_entry_that_already_exists_counts_by_its_labels() {
        let mut open = bare();
        let mut sheet = Sheet::new("Alpha cabinet", PaperSize::A3, Orientation::Landscape);
        let label = Entity::NetLabel(NetLabel { id: uuid::Uuid::new_v4(), at: Point::new(20.0, 20.0), name: "SIG_B".into(), rotation: 0, attrs: Default::default() });
        sheet.entities.insert(label.id(), label);
        open.sheets.push(sheet);
        let mut r = request(&[&["U1"], &["TB1"]]);
        r.planned_sheets = vec![detailed("Alpha cabinet", &["U2"], &[("U2", &["3"])])];
        let m = build_model(&fixture_source(), &open, &r).unwrap();
        let reserve = Reserve::of(&open, &m, &r);
        assert_eq!(reserve.text("SIG_B"), "Alpha cabinet");
        assert_eq!(reserve.text("SIG_A"), "", "the existing sheet has no SIG_A label, whatever the plan says");
    }

    /// On A4, a request whose planned sheets are only names is refused because every tag reserves room for all of them, while the same sheets given with their units fit: only the nets they carry reserve room.
    /// A4で、計画のシートを名前だけで渡すと、どの札も全部の名前の場所を空けるので幅が足りず断られる。同じシートを機器つきで渡すと、そのシートが載せるネットの札だけが場所を空けるので収まる。
    #[test]
    fn per_tag_room_lets_a_sheet_fit_that_the_worst_case_refuses() {
        let names: Vec<String> = (0..3).map(|i| format!("Distribution cabinet number {i} long")).collect();
        let mut r = request(&[&["U1"], &["U2"], &["TB1"]]);
        r.paper = DraftPaper::A4;
        let columns_of = |r: &DraftRequest| {
            let m = build_model(&fixture_source(), &bare(), r).unwrap();
            lay_out(&m, &bare(), r)
        };
        r.planned_sheets = names.iter().map(|n| n.as_str().into()).collect();
        let err = columns_of(&r).err().expect("the worst case does not fit");
        assert!(err.contains("does not fit the width"), "{err}");
        r.planned_sheets = names.iter().map(|n| detailed(n, &["TB1"], &[])).collect();
        columns_of(&r).expect("the per-tag room fits");
    }

    /// The names of planned sheets with units that the source does not have, or pins that a unit does not have, are refused with the entry named.
    /// 計画のシートに書いた機器が元の図に無い、またはピンがその機器に無いときは、項目を挙げて断られる。
    #[test]
    fn planned_objects_with_unknown_units_or_pins_are_refused() {
        let mut r = request(&[&["U1"]]);
        r.planned_sheets = vec![detailed("Alpha cabinet", &["NOPE9"], &[])];
        let msg = build_model(&fixture_source(), &bare(), &r).unwrap_err();
        assert!(msg.contains("Alpha cabinet") && msg.contains("NOPE9"), "{msg}");
        r.planned_sheets = vec![detailed("Alpha cabinet", &["U2"], &[("U2", &["77"])])];
        let msg = build_model(&fixture_source(), &bare(), &r).unwrap_err();
        assert!(msg.contains("Alpha cabinet") && msg.contains("77"), "{msg}");
        r.planned_sheets = vec![detailed("Alpha cabinet", &["U2"], &[("TB1", &["1"])])];
        assert!(build_model(&fixture_source(), &bare(), &r).unwrap_err().contains("TB1"));
    }

    /// A planned_sheets entry that is neither a name nor a {name, units, pins?} object (a number, an object without units, an object with an unknown field) is refused with a message that names planned_sheets and the entry's position; a list that is not a list is refused naming planned_sheets.
    /// planned_sheets の項目が名前でも {name, units, pins?} でもない (数、units の無い項目、知らない項目のある項目) と、planned_sheets と何番目の項目かを挙げた文で断られる。一覧でないものも planned_sheets を挙げて断られる。
    #[test]
    fn a_malformed_planned_entry_is_refused_naming_its_position() {
        let err = |planned: &str| {
            serde_json::from_str::<DraftRequest>(&format!(r#"{{"name":"A","columns":[["U1"]],"planned_sheets":{planned}}}"#))
                .expect_err("refused")
                .to_string()
        };
        let msg = err(r#"["B", 7]"#);
        assert!(msg.contains("planned_sheets[1]") && msg.contains("entry 2"), "{msg}");
        let msg = err(r#"["B", "C", {"name": "D"}]"#);
        assert!(msg.contains("planned_sheets[2]") && msg.contains("units"), "{msg}");
        let msg = err(r#"[{"name": "D", "units": [], "bogus": 1}]"#);
        assert!(msg.contains("planned_sheets[0]") && msg.contains("bogus"), "{msg}");
        let msg = err(r#""B""#);
        assert!(msg.contains("planned_sheets") && msg.contains("list"), "{msg}");
    }

    /// A planned sheet may be written as a string or as an object, in one list, and an object with an unknown field is refused.
    /// 計画のシートは、文字列でも、機器つきの項目でも書け、1つの一覧に混ぜてよい。知らない項目のある項目は断られる。
    #[test]
    fn planned_sheets_read_both_forms_from_json() {
        let r: DraftRequest = serde_json::from_str(
            r#"{"name":"A","columns":[["U1"]],"planned_sheets":["B",{"name":"C","units":["U2"],"pins":{"U2":["3"]}},{"name":"D","units":[]}]}"#,
        )
        .unwrap();
        assert_eq!(r.planned_sheets.len(), 3);
        assert!(matches!(&r.planned_sheets[0], PlannedSheet::Name(n) if n == "B"));
        assert!(matches!(&r.planned_sheets[1], PlannedSheet::Detailed(d) if d.name == "C" && d.pins["U2"] == vec!["3".to_string()]));
        assert!(matches!(&r.planned_sheets[2], PlannedSheet::Detailed(d) if d.pins.is_empty()));
        assert!(serde_json::from_str::<DraftRequest>(r#"{"name":"A","columns":[["U1"]],"planned_sheets":[{"name":"C","units":[],"bogus":1}]}"#).is_err());
    }

    /// With planned names the layout is wider than with none: the first column starts further right so that the widest flag fits.
    /// 計画した名前を渡すと、いちばん広い旗が収まるように、渡さないときより1列目の位置が右になる。
    #[test]
    fn planned_names_push_the_first_column_right() {
        let r = request(&[&["U1"], &["TB1"]]);
        let mut with = r.clone();
        with.planned_sheets = vec!["Cabinet B terminal side".into(), "Spare relay panel number two".into(), "Another quite long sheet".into()];
        let a = layout(&r);
        let b = layout(&with);
        assert!(b.columns[0].right >= a.columns[0].right);
        let wide = |l: &DraftLayout| l.columns[0].right - l.columns[0].left;
        assert!(wide(&b) > wide(&a), "{} vs {}", wide(&b), wide(&a));
    }

    /// A request without planned_sheets reads as an empty list and lays out exactly as one that names an empty list.
    /// planned_sheets の無い依頼は空の一覧として読まれ、空の一覧を明示した依頼とまったく同じ配置になる。
    #[test]
    fn a_request_without_planned_sheets_lays_out_as_before() {
        let r: DraftRequest = serde_json::from_str(r#"{"name":"Function","columns":[["U1"],["U2"]]}"#).unwrap();
        assert!(r.planned_sheets.is_empty());
        let mut explicit = r.clone();
        explicit.planned_sheets = Vec::new();
        let (a, b) = (layout(&r), layout(&explicit));
        let spans = |l: &DraftLayout| l.columns.iter().map(|c| (c.left, c.right)).collect::<Vec<_>>();
        assert_eq!(spans(&a), spans(&b));
        assert_eq!(a.keep_out.len(), b.keep_out.len());
        assert_eq!(a.reserve.text("SIG_A"), b.reserve.text("SIG_A"));
    }

    // ---- placement

    /// Every box origin, pin, stub end and tag lies on the 2.5 mm grid, at both pitches.
    /// 箱の原点・ピン・短い線の先・タグは、どちらのピン間隔でも2.5mmの格子の上にある。
    #[test]
    fn every_pin_and_origin_is_on_the_grid() {
        for pitch in [5.0, 2.5] {
            let mut r = request(&[&["U1", "M1"], &["U2"], &["TB1", "R1"]]);
            r.pitch_mm = pitch;
            r.power_nets = vec!["P24X".into()];
            let l = layout(&r);
            for s in symbols(&l) {
                assert!(on_grid(s.at.x) && on_grid(s.at.y), "{} at {:?}", s.symbol_id, s.at);
            }
            for p in pin_points(&l.sheet, &defs_of(&l)) {
                assert!(on_grid(p.pos.x) && on_grid(p.pos.y), "{}.{} at {:?} (pitch {pitch})", p.reference, p.number, p.pos);
            }
            for n in labels(&l) {
                assert!(on_grid(n.at.x) && on_grid(n.at.y));
            }
        }
    }

    fn unit_rect(l: &DraftLayout, reference: &str) -> Rect {
        let s = unit_symbol(l, reference);
        let defs = defs_of(l);
        let index = DefIndex::new(&defs);
        symbol_bounds(&s, &index.get(&s).unwrap()).unwrap()
    }

    /// Columns are packed from the left side of the drawing area, in the given order, with at least the minimum channel between them, and stay inside the drawing area.
    /// 列は描画範囲の左から、指定した順に、通り道の最小幅以上のすき間をあけて詰めて並び、描画範囲からはみ出さない。
    #[test]
    fn columns_are_packed_from_the_left_of_the_drawing_area() {
        let l = layout(&request(&[&["U1"], &["U2"], &["TB1"]]));
        let (a, b, c) = (unit_rect(&l, "U1"), unit_rect(&l, "U2"), unit_rect(&l, "TB1"));
        assert!(a.max.x < b.min.x && b.max.x < c.min.x);
        assert!(b.min.x - a.max.x >= MIN_CHANNEL_MM && c.min.x - b.max.x >= MIN_CHANNEL_MM);
        let template = crate::frame_template::for_project(&open());
        let (frame, _) = crate::arrange::frame_areas(&template, &l.sheet);
        assert!(a.min.x >= frame.0 + AREA_MARGIN_MM - 1e-6);
        assert!(c.max.x <= frame.2 - AREA_MARGIN_MM + 1e-6);
        assert!(a.min.x - frame.0 < 40.0, "the first column starts at the left edge, not in the middle");
    }

    /// Nothing on the new sheet enters the title block.
    /// 新しいシートの上のものは、何も表題欄に入らない。
    #[test]
    fn nothing_enters_the_title_block() {
        for paper in [DraftPaper::A3, DraftPaper::A4] {
            let mut r = request(&[&["U1"], &["U2"], &["TB1"]]);
            r.paper = paper;
            let l = layout(&r);
            let template = crate::frame_template::for_project(&open());
            let (_, tb) = crate::arrange::frame_areas(&template, &l.sheet);
            let title = Rect::from_corners(Point::new(tb.0, tb.1), Point::new(tb.2, tb.3));
            let defs = defs_of(&l);
            let font = crate::svg::LABEL_FONT * l.sheet.annotation_scale();
            for e in l.sheet.entities.values() {
                if let Some(r) = item_rect(e, &defs, &l.sheet, font, &Reserve::floor_only(&open())) {
                    assert!(!r.overlaps(&title), "{r:?} enters the title block {title:?}");
                }
            }
        }
    }

    // ---- refusals

    /// A column taller than the drawing area is refused, and the message names the column and the excess in mm.
    /// 描画範囲より背の高い列は断られ、メッセージに列の番号と超えた量 (mm) が入る。
    #[test]
    fn a_column_taller_than_the_area_is_refused_with_the_excess() {
        let mut r = request(&[&["U1", "U2", "TB1", "M1", "R1"]]);
        r.paper = DraftPaper::A4;
        let e = refusal(&r);
        assert!(e.starts_with("Column 1 is ") && e.contains(" mm taller"), "{e}");
        let excess: f64 = e["Column 1 is ".len()..].split(' ').next().unwrap().parse().unwrap();
        assert!(excess > 0.0);
    }

    /// Columns that leave no routing channel between them are refused, naming the column that does not fit and the excess in mm.
    /// 列のあいだに配線の通り道をあけられないときは断られ、入りきらない列の番号と超えた量 (mm) が入る。
    #[test]
    fn columns_that_leave_no_channel_are_refused() {
        let mut r = request(&[&["U1"], &["U2"], &["TB1"]]);
        r.paper = DraftPaper::A4;
        let far = "a destination written out in a very long sentence so that the tag grows wide ".repeat(2);
        r.destinations.insert("SIG_A".into(), far.clone());
        r.destinations.insert("SIG_B".into(), far);
        let e = refusal(&r);
        assert!(e.starts_with("Column ") && e.contains("does not fit the width"), "{e}");
        assert!(e.contains(" mm more"), "{e}");
    }

    // ---- look

    /// Writes the laid-out fixture sheet as SVG to the path in MADAKE_DRAFT_SVG so that it can be looked at (run with --ignored).
    /// 見て確かめるために、配置した見本のシートを MADAKE_DRAFT_SVG の場所へSVGで書き出す (--ignored で実行する)。
    #[test]
    #[ignore]
    fn write_the_laid_out_sheet_as_svg() {
        let Ok(path) = std::env::var("MADAKE_DRAFT_SVG") else { return };
        let mut r = request(&[&["U1", "M1"], &["U2"], &["TB1", "R1"]]);
        r.power_nets = vec!["P24X".into()];
        r.destinations.insert("SIG_A".into(), "cabinet B".into());
        let l = layout(&r);
        let svg = crate::svg::sheet_to_svg(&l.sheet, &defs_of(&l), &[]);
        std::fs::write(&path, svg).unwrap();
        // tags: only two columns, so that SIG_A and SIG_B leave the sheet
        let mut r = request(&[&["U1"], &["U2"]]);
        r.power_nets = vec!["P24X".into()];
        r.destinations.insert("SIG_A".into(), "cabinet B".into());
        let l = layout(&r);
        std::fs::write(format!("{path}.tags.svg"), crate::svg::sheet_to_svg(&l.sheet, &defs_of(&l), &[])).unwrap();
    }
}

// --- wires
//
// 第3段: layout が置いたシートに、内部ネットと出ていくネットの配線を引く。既存の経路探索 (`route.rs`) を
// そのまま使い、引けないネットは同じ名前のネットラベルでつなぐ (一部だけ引いた配線は残さない)。

use crate::route::{emit, RouteGrid, RouteOptions, RouteUnit};

/// 出ていくネットのタグ (ネット名と、このシートが持つピンの数)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DraftTagged {
    pub net: String,
    pub pins: usize,
}

/// 置いた電源・接地の印の数。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct DraftMarked {
    pub power: usize,
    pub ground: usize,
}

/// 運ばなかったピンと、その理由。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DraftSkipped {
    pub reference: String,
    pub pin: String,
    pub reason: String,
}

/// 下書きの計画。図面は書き換えない。`sheet` が新しいシートの中身 (適用は呼び出し側)。
#[derive(Debug, Clone, Serialize)]
pub struct DraftPlan {
    #[serde(skip)]
    pub sheet: Sheet,
    /// 置いた機器の数。
    pub units: usize,
    /// 運んだピンの数。
    pub pins: usize,
    /// 配線を引いたネット名 (自然順)。
    pub wired: Vec<String>,
    pub tagged: Vec<DraftTagged>,
    pub marked: DraftMarked,
    /// 配線を引けず、ネットラベルでつないだネット名 (自然順)。
    pub labelled: Vec<String>,
    pub skipped_pins: Vec<DraftSkipped>,
}

/// 小数3桁までの整数にした位置 (端子を並べるときの鍵)。
fn quantised(p: Point) -> (i64, i64) {
    ((p.x * 1000.0).round() as i64, (p.y * 1000.0).round() as i64)
}

/// 元の図と開いているプロジェクトから、新しい機能シートの計画を作る。
pub fn plan_function_sheet(source: &Project, open: &Project, req: &DraftRequest) -> Result<DraftPlan, String> {
    plan_with_options(source, open, req, &RouteOptions::default())
}

/// [`plan_function_sheet`] の、経路探索の設定を渡せる版 (引けないネットのテストが使う)。
/// 図枠の余白と表題欄の大きさは、`SharedDoc::reroute_wires` と同じく設定の既定値をそのまま使う。
pub(crate) fn plan_with_options(source: &Project, open: &Project, req: &DraftRequest, opts: &RouteOptions) -> Result<DraftPlan, String> {
    plan_in(source, open, req, opts, &crate::frame_template::for_project(open))
}

/// [`plan_with_options`] の、図枠テンプレートも渡せる版 (別の図枠でのテストが使う)。
pub(crate) fn plan_in(
    source: &Project,
    open: &Project,
    req: &DraftRequest,
    opts: &RouteOptions,
    template: &crate::frame_template::FrameTemplate,
) -> Result<DraftPlan, String> {
    opts.validate()?;
    let model = build_model(source, open, req)?;
    let layout = lay_out_in(&model, open, req, template)?;
    let (sheet, wired, labelled) = route_wires(&model, &layout, opts);
    Ok(DraftPlan {
        sheet,
        units: model.units.len(),
        pins: model.units.iter().map(|u| u.pins.len()).sum(),
        wired,
        tagged: layout.tagged.iter().map(|(net, pins)| DraftTagged { net: net.clone(), pins: *pins }).collect(),
        marked: DraftMarked { power: layout.power_marks, ground: layout.ground_marks },
        labelled,
        skipped_pins: model
            .skipped
            .iter()
            .map(|(reference, pin, reason)| DraftSkipped { reference: reference.clone(), pin: pin.clone(), reason: reason.clone() })
            .collect(),
    })
}

/// 単位ごとの配線の端1つ。
struct Term {
    /// 経路探索に渡す端 (棒の端・短い線の先、または札の位置)。
    end: Point,
    dir: Dir,
    /// 札を下げた組の端なら、その札の位置。
    tag: Option<Point>,
    /// 札の位置そのものの端か。
    is_tag: bool,
    /// 組の棒の上の端 ([`Terminal::top`])。
    top: Point,
}

/// 3本以上の端が会う点に接点の丸を打つ (まだ無ければ)。線の端が、別の線の途中や端・曲がり角に載る点が対象
/// (線どうしが十字に交わるだけの点は含めない)。端の数は、線の端1つにつき1、線の途中を通るなら2。
fn add_junctions(sheet: &mut Sheet) {
    let lines: Vec<Vec<Point>> = sheet.entities.values().filter_map(|e| if let Entity::Wire(w) = e { Some(w.points.clone()) } else { None }).collect();
    let dots: Vec<Point> = sheet.entities.values().filter_map(|e| if let Entity::Junction(j) = e { Some(j.at) } else { None }).collect();
    let mut candidates: Vec<Point> = lines.iter().filter(|l| l.len() >= 2).flat_map(|l| [l[0], l[l.len() - 1]]).collect();
    candidates.sort_by_key(|&p| quantised(p));
    candidates.dedup_by_key(|p| quantised(*p));
    for p in candidates {
        if dots.iter().any(|d| d.distance_to(&p) < 1e-6) {
            continue;
        }
        let arms: usize = lines
            .iter()
            .flat_map(|l| l.windows(2))
            .map(|seg| {
                if seg[0].distance_to(&p) < 1e-6 || seg[1].distance_to(&p) < 1e-6 {
                    1
                } else if crate::netlist::segment_distance(&p, &seg[0], &seg[1]) < 1e-6 {
                    2
                } else {
                    0
                }
            })
            .sum();
        if arms >= 3 {
            let id = uuid::Uuid::new_v4();
            sheet.entities.insert(id, Entity::Junction(crate::model::Junction { id, at: p }));
        }
    }
}

/// 配線 (経路の線と引き出し線) が、どれかの札の旗に触れるか。旗は基準点の上にあるので、同じネットの線が旗の
/// 下の辺の高さかそれより下にあり、基準点で終わるとき (札へ入る線・札から下へ続く棒) だけは触れてよい。
fn touches_a_flag(emitted: &[Entity], flags: &[(Rect, Point, String)], net: &str) -> bool {
    emitted.iter().filter_map(|e| if let Entity::Wire(w) = e { Some(w) } else { None }).any(|w| {
        w.points.windows(2).any(|s| {
            let (a, b) = (s[0], s[1]);
            flags.iter().any(|(r, at, name)| {
                let (x0, x1, y0, y1) = (a.x.min(b.x), a.x.max(b.x), a.y.min(b.y), a.y.max(b.y));
                let hit = x0 <= r.max.x + 1e-9 && x1 >= r.min.x - 1e-9 && y0 <= r.max.y + 1e-9 && y1 >= r.min.y - 1e-9;
                let own = name == net && (a.distance_to(at) < 1e-6 || b.distance_to(at) < 1e-6) && y0 >= r.max.y - 1e-9;
                hit && !own
            })
        })
    })
}

/// 配線を引き、引けないネットは、各端に札 (行き先なし) を付けてつなぐ。返り値は完成したシートと、配線したネット名・札でつないだネット名 (どちらも自然順)。
fn route_wires(model: &DraftModel, layout: &DraftLayout, opts: &RouteOptions) -> (Sheet, Vec<String>, Vec<String>) {
    let mut sheet = layout.sheet.clone();
    let defs = sheet_symbol_defs(&sheet);
    let key_of = |l: &Vec<Point>| l.iter().map(|&p| quantised(p)).collect::<Vec<_>>();

    // 端が2つ以上あるネットごとに1つの単位。名前の自然順 (同名なら添字順) に引く
    let mut by_net: BTreeMap<usize, Vec<&Terminal>> = BTreeMap::new();
    for t in &layout.terminals {
        by_net.entry(t.net).or_default().push(t);
    }
    let mut order: Vec<usize> = by_net.iter().filter(|(_, t)| t.len() >= 2).map(|(&n, _)| n).collect();
    order.sort_by(|&a, &b| natural_cmp(&model.nets[a].name, &model.nets[b].name).then(a.cmp(&b)));
    let mut terms_of: Vec<Vec<Term>> = Vec::new();
    let units: Vec<RouteUnit> = order
        .iter()
        .map(|&n| {
            let mut terms: Vec<Term> =
                by_net[&n].iter().map(|t| Term { end: t.at, dir: t.dir, tag: t.tag_at, is_tag: t.is_tag, top: t.top }).collect();
            terms.sort_by_key(|t| quantised(t.end));
            let unit = RouteUnit {
                net: model.nets[n].name.clone(),
                wires: Vec::new(),
                junctions: Vec::new(),
                template: default_wire(Vec::new()),
                old_lines: Vec::new(),
                old_junctions: Vec::new(),
                terms: terms.iter().map(|t| (t.end, Some(t.dir))).collect(),
                leads: Vec::new(),
            };
            terms_of.push(terms);
            unit
        })
        .collect();
    // 札を下げた組の端から札までの一直線 (引けずに札でつなぐときは、この線で端と札をつなぐ)
    let tag_lines_of = |i: usize| -> Vec<Vec<Point>> {
        terms_of[i].iter().filter_map(|t| t.tag.map(|tag| vec![t.end, tag])).collect()
    };

    // 動かない線: 箱の短い線・棒・印のスタブ
    let mut fixed_lines: Vec<Vec<Point>> =
        sheet.entities.values().filter_map(|e| if let Entity::Wire(w) = e { Some(w.points.clone()) } else { None }).collect();
    fixed_lines.sort_by_key(key_of);
    let mut fixed_spots: Vec<Point> = sheet.entities.values().filter_map(|e| if let Entity::Junction(j) = e { Some(j.at) } else { None }).collect();
    fixed_spots.sort_by_key(|&p| quantised(p));

    // 札の旗と印 (図形と文字) に近づかない。図枠の内側の外と表題欄 (改訂欄を含む) にも入らない
    let (fx0, fy0, fx1, fy1) = layout.frame;
    let (pw, ph) = sheet.paper_mm();
    let far = 10.0;
    let mut blocked: Vec<Rect> = layout.keep_out.clone();
    blocked.extend([
        Rect::from_corners(Point::new(-far, -far), Point::new(fx0, ph + far)),
        Rect::from_corners(Point::new(fx1, -far), Point::new(pw + far, ph + far)),
        Rect::from_corners(Point::new(-far, -far), Point::new(pw + far, fy0)),
        Rect::from_corners(Point::new(-far, fy1), Point::new(pw + far, ph + far)),
        Rect::from_corners(Point::new(layout.title.0, layout.title.1), Point::new(layout.title.2, layout.title.3)),
    ]);
    // 全ての札の旗 (後から書かれる行き先の分まで): (旗, 基準点, ネット名)。どの配線も触れてはいけない
    let flag_of = |l: &NetLabel| flag_rect(l, layout.label_font, sheet.xref_style, layout.reserve.text(&l.name));
    let mut flags: Vec<(Rect, Point, String)> = sheet
        .entities
        .values()
        .filter_map(|e| if let Entity::NetLabel(l) = e { flag_of(l).map(|r| (r, l.at, l.name.clone())) } else { None })
        .collect();
    flags.sort_by_key(|(_, at, _)| quantised(*at));

    // 回ごとに、引けていないネット以外を全て引く。引けないネット (旗に触れる経路も引けないものとする) は札でつなぎ、
    // その札の旗を次の回から通れない場所に足し、札の点と端は動かない点にする。新しく引けないネットが出なくなったら終わる
    let mut failed: BTreeSet<usize> = BTreeSet::new();
    let mut fallback: BTreeMap<usize, Vec<Entity>> = BTreeMap::new();
    let mut extra_lines: Vec<Vec<Point>> = Vec::new();
    let mut extra_spots: Vec<Point> = Vec::new();
    let mut outcome: Vec<Option<crate::route::UnitRoute>> = (0..units.len()).map(|_| None).collect();
    for _ in 0..=units.len() {
        let mut grid = RouteGrid::build(&sheet, &defs, opts);
        for r in &blocked {
            grid.block_area(r);
        }
        let todo: Vec<usize> = (0..units.len()).filter(|i| !failed.contains(i)).collect();
        let mut batch: Vec<RouteUnit> = todo.iter().map(|&i| units[i].clone()).collect();
        let lines: Vec<Vec<Point>> = fixed_lines.iter().chain(&extra_lines).cloned().collect();
        let spots: Vec<Point> = fixed_spots.iter().chain(&extra_spots).copied().collect();
        let results = crate::route::route_units(&grid, &mut batch, lines, spots);
        let mut new_failed = Vec::new();
        for (&i, r) in todo.iter().zip(results) {
            match r {
                Ok(route) if !touches_a_flag(&emit(&units[i], &route), &flags, &units[i].net) => outcome[i] = Some(route),
                _ => {
                    outcome[i] = None;
                    new_failed.push(i);
                }
            }
        }
        if new_failed.is_empty() {
            break;
        }
        // 引けないネットの札つなぎ: 各組の棒の上の端に、箱から離れる向きの札 (行き先なし)。札を下げた組には札までの線を引く
        for i in new_failed {
            failed.insert(i);
            let own = tag_lines_of(i);
            let mut es: Vec<Entity> = own.iter().cloned().map(|l| Entity::Wire(default_wire(l))).collect();
            for t in terms_of[i].iter().filter(|t| !t.is_tag && t.tag.is_none()) {
                let tag = tag_label(&units[i].net, t.top, t.dir, None);
                if let Some(r) = flag_of(&tag) {
                    blocked.push(r);
                    flags.push((r, tag.at, tag.name.clone()));
                }
                extra_spots.push(t.top);
                es.push(Entity::NetLabel(tag));
            }
            extra_spots.extend(terms_of[i].iter().map(|t| t.end));
            extra_lines.extend(own);
            fallback.insert(i, es);
        }
    }

    let mut wired = Vec::new();
    let mut labelled = Vec::new();
    for (i, u) in units.iter().enumerate() {
        match &outcome[i] {
            Some(route) => {
                wired.push(u.net.clone());
                for e in emit(u, route) {
                    sheet.entities.insert(e.id(), e);
                }
            }
            None => {
                labelled.push(u.net.clone());
                for e in fallback.remove(&i).unwrap_or_default() {
                    sheet.entities.insert(e.id(), e);
                }
            }
        }
    }
    add_junctions(&mut sheet);
    wired.sort_by(|a, b| natural_cmp(a, b));
    labelled.sort_by(|a, b| natural_cmp(a, b));
    (sheet, wired, labelled)
}

#[cfg(test)]
mod wire_tests {
    use super::*;
    use crate::netlist::extract_netlist;

    fn open() -> Project {
        Project::new("open")
    }

    /// The request the golden file is made from: three columns, P24X as power, GND as ground (the default).
    fn request() -> DraftRequest {
        DraftRequest {
            name: "Function".into(),
            columns: vec![vec!["U1".into(), "M1".into()], vec!["U2".into()], vec!["TB1".into(), "R1".into()]],
            pins: BTreeMap::new(),
            power_nets: vec!["P24X".into()],
            ground_nets: None,
            destinations: BTreeMap::new(),
            paper: DraftPaper::A3,
            pitch_mm: 5.0,
            planned_sheets: Vec::new(),
        }
    }

    fn plan() -> DraftPlan {
        plan_function_sheet(&fixture_source(), &open(), &request()).expect("the fixture request is valid")
    }

    /// Device pin groups (reference, pin key) of a sheet, only the references in `refs`, only groups of two or more pins.
    fn groups(sheet: &Sheet, refs: &[&str]) -> BTreeSet<BTreeSet<(String, String)>> {
        let defs = sheet_symbol_defs(sheet);
        extract_netlist(sheet, &defs)
            .into_iter()
            .map(|n| {
                n.pins
                    .into_iter()
                    .filter(|p| refs.contains(&p.reference.as_str()))
                    .map(|p| (p.reference, p.pin))
                    .collect::<BTreeSet<_>>()
            })
            .filter(|g| g.len() >= 2)
            .collect()
    }

    fn source_groups(refs: &[&str]) -> BTreeSet<BTreeSet<(String, String)>> {
        groups(&fixture_source().sheets[0], refs)
    }

    const REFS: [&str; 5] = ["U1", "M1", "U2", "TB1", "R1"];

    fn wires(sheet: &Sheet) -> Vec<Wire> {
        sheet.entities.values().filter_map(|e| if let Entity::Wire(w) = e { Some(w.clone()) } else { None }).collect()
    }

    fn junctions(sheet: &Sheet) -> Vec<Point> {
        sheet.entities.values().filter_map(|e| if let Entity::Junction(j) = e { Some(j.at) } else { None }).collect()
    }

    /// Replaces every distinct UUID in the text by `#<n>`, n = order of first appearance.
    fn normalise_ids(text: &str) -> String {
        let bytes = text.as_bytes();
        let is_uuid_at = |i: usize| -> bool {
            if i + 36 > bytes.len() {
                return false;
            }
            bytes[i..i + 36].iter().enumerate().all(|(k, &b)| if [8, 13, 18, 23].contains(&k) { b == b'-' } else { b.is_ascii_hexdigit() })
        };
        let mut seen: Vec<&str> = Vec::new();
        let mut out = String::new();
        let mut i = 0;
        while i < bytes.len() {
            if is_uuid_at(i) {
                let id = &text[i..i + 36];
                let n = seen.iter().position(|s| *s == id).unwrap_or_else(|| {
                    seen.push(id);
                    seen.len() - 1
                });
                out.push_str(&format!("#{n}"));
                i += 36;
            } else {
                let ch = text[i..].chars().next().unwrap();
                out.push(ch);
                i += ch.len_utf8();
            }
        }
        out
    }

    /// The sheet with its entities in an order that does not depend on the random ids: sorted by what they draw, ids counted up from 1.
    fn canonical(sheet: &Sheet) -> Sheet {
        let mut items: Vec<(String, Entity)> = sheet
            .entities
            .values()
            .map(|e| {
                let key = match e {
                    Entity::Symbol(s) => format!("S {} {} {} {:?} {} {:?}", s.symbol_id, s.reference, s.value, s.at, s.rotation, s.attrs),
                    Entity::Wire(w) => format!("W {:?}", w.points),
                    Entity::NetLabel(n) => format!("L {} {:?} {:?}", n.name, n.at, n.attrs),
                    Entity::Junction(j) => format!("J {:?}", j.at),
                    other => panic!("the planner emits no {other:?}"),
                };
                (key, e.clone())
            })
            .collect();
        items.sort_by(|a, b| a.0.cmp(&b.0));
        let mut out = sheet.clone();
        out.entities.clear();
        for (n, (_, mut e)) in items.into_iter().enumerate() {
            let id = uuid::Uuid::from_u128(n as u128 + 1);
            match &mut e {
                Entity::Symbol(s) => s.id = id,
                Entity::Wire(w) => w.id = id,
                Entity::NetLabel(l) => l.id = id,
                Entity::Junction(j) => j.id = id,
                _ => {}
            }
            out.entities.insert(id, e);
        }
        out
    }

    fn svg_of(sheet: &Sheet) -> String {
        let sheet = &canonical(sheet);
        let mut project = Project::new("draft");
        project.sheets.clear();
        project.sheets.push(sheet.clone());
        let svg = crate::svg::project_sheet_to_svg(&project, sheet.id, &crate::symbol::builtin_symbols()).expect("the sheet renders");
        normalise_ids(&svg)
    }

    /// The body of each unit: the drawn outline of a box (the pin numbers outside it and the leads are not body), the outline of any other symbol.
    fn bodies(sheet: &Sheet) -> Vec<Rect> {
        let defs = sheet_symbol_defs(sheet);
        let index = DefIndex::new(&defs);
        sheet
            .entities
            .values()
            .filter_map(|e| if let Entity::Symbol(s) = e { Some(s) } else { None })
            .filter(|s| !s.reference.is_empty())
            .filter_map(|s| {
                if s.symbol_id == BLOCK_SYMBOL_ID {
                    let (lo, hi) = block::block_outline(&block::parse_block_spec(&s.attrs).ok()?);
                    Some(Rect::from_corners(Point::new(s.at.x + lo.x, s.at.y + lo.y), Point::new(s.at.x + hi.x, s.at.y + hi.y)))
                } else {
                    let def = index.get(s)?;
                    symbol_bounds(s, &def)
                }
            })
            .collect()
    }

    /// The pins of every segment of every wire strictly inside a unit's body (not the edge) are wires through a symbol.
    fn wires_through_bodies(sheet: &Sheet) -> usize {
        let bodies = bodies(sheet);
        // 箱の外形は、ピンの線の太さぶん (0.25 mm) ピンの側へはみ出している
        let e = 0.3;
        let mut n = 0;
        for w in wires(sheet) {
            for s in w.points.windows(2) {
                let (x0, x1, y0, y1) = (s[0].x.min(s[1].x), s[0].x.max(s[1].x), s[0].y.min(s[1].y), s[0].y.max(s[1].y));
                n += bodies.iter().filter(|r| x0 < r.max.x - e && x1 > r.min.x + e && y0 < r.max.y - e && y1 > r.min.y + e).count();
            }
        }
        n
    }

    /// Pairs of segments of different wires that lie on one line and share a stretch of positive length.
    fn overlapping_wires(sheet: &Sheet) -> usize {
        let ws = wires(sheet);
        let mut n = 0;
        for (a, wa) in ws.iter().enumerate() {
            for wb in ws.iter().skip(a + 1) {
                for sa in wa.points.windows(2) {
                    for sb in wb.points.windows(2) {
                        let vertical = |s: &[Point]| (s[0].x - s[1].x).abs() < 1e-9;
                        if vertical(sa) && vertical(sb) && (sa[0].x - sb[0].x).abs() < 1e-9 {
                            let lo = sa[0].y.min(sa[1].y).max(sb[0].y.min(sb[1].y));
                            let hi = sa[0].y.max(sa[1].y).min(sb[0].y.max(sb[1].y));
                            n += usize::from(hi - lo > 1e-6);
                        } else if !vertical(sa) && !vertical(sb) && (sa[0].y - sb[0].y).abs() < 1e-9 {
                            let lo = sa[0].x.min(sa[1].x).max(sb[0].x.min(sb[1].x));
                            let hi = sa[0].x.max(sa[1].x).min(sb[0].x.max(sb[1].x));
                            n += usize::from(hi - lo > 1e-6);
                        }
                    }
                }
            }
        }
        n
    }

    /// The internal nets of the drafted sheet are joined by wires, and the device pins of the sheet fall into exactly the groups the source has.
    /// 下書きのシートの内部のネットは配線でつながり、機器のピンのまとまりは元の図と全く同じになる。
    #[test]
    fn an_internal_net_is_wired_and_the_sheet_netlist_matches_the_source() {
        let p = plan();
        assert!(p.labelled.is_empty(), "labelled: {:?}", p.labelled);
        assert_eq!(groups(&p.sheet, &REFS), source_groups(&REFS));
        assert!(!wires(&p.sheet).is_empty());
    }

    /// A net of three pins is drawn as a tree with a junction dot where the branches meet.
    /// 3本のピンをつなぐネットは、枝分かれのところに接点の丸がある木として描かれる。
    #[test]
    fn a_three_pin_net_gets_a_junction() {
        let p = plan();
        assert!(p.wired.iter().any(|n| n == "SIG_A"));
        let dots = junctions(&p.sheet);
        assert!(!dots.is_empty(), "SIG_A has three pins and needs a junction");
        for d in dots {
            let ends = wires(&p.sheet).iter().filter(|w| w.points.iter().any(|q| q.distance_to(&d) < 1e-6)).count();
            assert!(ends >= 3 || wires(&p.sheet).iter().any(|w| w.points.windows(2).any(|s| {
                (s[0].x == s[1].x && (s[0].x - d.x).abs() < 1e-6 && d.y > s[0].y.min(s[1].y) && d.y < s[0].y.max(s[1].y))
                    || (s[0].y == s[1].y && (s[0].y - d.y).abs() < 1e-6 && d.x > s[0].x.min(s[1].x) && d.x < s[0].x.max(s[1].x))
            })));
        }
    }

    /// The drafted sheet has no wire that ends in the air and no pin that is in use but unconnected.
    /// 下書きのシートには、途中で宙に浮く配線も、使うはずなのに未接続のピンも無い。
    #[test]
    fn no_wire_is_dangling_and_no_pin_in_use_is_unconnected() {
        let p = plan();
        let defs = sheet_symbol_defs(&p.sheet);
        let diags = crate::verify::verify_sheet(&p.sheet, &defs);
        assert!(!diags.iter().any(|d| d.code == "erc.dangling_wire"), "{diags:?}");
        let unconnected: Vec<_> = diags.iter().filter(|d| d.code == "erc.unconnected_pin").collect();
        assert!(unconnected.is_empty(), "{unconnected:?}");
    }

    /// Wires cross each other only at right angles, never run along one another, and never cross a unit's body.
    /// 配線どうしは直角にしか交わらず、並んで重なることも、機器の本体を横切ることも無い。
    #[test]
    fn wires_cross_only_at_right_angles_and_stay_off_boxes() {
        let p = plan();
        for w in wires(&p.sheet) {
            assert!(w.points.windows(2).all(|s| (s[0].x - s[1].x).abs() < 1e-9 || (s[0].y - s[1].y).abs() < 1e-9), "slanted wire {:?}", w.points);
        }
        assert_eq!(overlapping_wires(&p.sheet), 0);
        assert_eq!(wires_through_bodies(&p.sheet), 0);
        let m = crate::tidy::tidy_metrics(&p.sheet, &sheet_symbol_defs(&p.sheet));
        // 配線の頂点は経路探索の格子 (1.25 mm) の上にある。2.5 mm の格子から外れる数 (off_grid) は数えない
        assert_eq!((m.label_overlaps, m.symbol_overlaps), (0, 0), "{m:?}");
    }

    /// When the router cannot place a net, its pins get short stubs with same-named labels, no half-drawn wire is left, and the connections stay the same.
    /// 配線を引けないネットは、各ピンに短い線と同じ名前のラベルを付けてつなぐ。引きかけの線は残らず、つながりは変わらない。
    #[test]
    fn a_net_the_router_cannot_place_is_joined_by_labels() {
        let opts = RouteOptions { frame_margin_mm: 100.0, ..RouteOptions::default() };
        let p = plan_with_options(&fixture_source(), &open(), &request(), &opts).expect("valid");
        assert!(!p.labelled.is_empty(), "a frame margin of 100 mm leaves no room for the wires");
        for net in &p.labelled {
            assert!(!p.wired.contains(net), "{net} is both wired and labelled");
        }
        assert_eq!(groups(&p.sheet, &REFS), source_groups(&REFS));
        let names: Vec<String> =
            p.sheet.entities.values().filter_map(|e| if let Entity::NetLabel(l) = e { Some(l.name.clone()) } else { None }).collect();
        for net in &p.labelled {
            assert!(names.iter().filter(|n| *n == net).count() >= 2, "{net} needs a label at each end: {names:?}");
        }
        let defs = sheet_symbol_defs(&p.sheet);
        let diags = crate::verify::verify_sheet(&p.sheet, &defs);
        assert!(!diags.iter().any(|d| d.code == "erc.dangling_wire"), "{diags:?}");
    }

    /// The counts in the plan are the counts on the sheet.
    /// 計画の数 (機器・ピン・配線したネット・印・タグ) は、シートの中身と一致する。
    #[test]
    fn the_plan_counts_match_the_sheet() {
        let p = plan();
        let units: usize = p.sheet.entities.values().filter(|e| matches!(e, Entity::Symbol(s) if !s.reference.is_empty())).count();
        assert_eq!(p.units, units);
        assert_eq!(p.units, 5);
        assert_eq!(p.pins, 17);
        assert_eq!(p.marked, DraftMarked { power: 3, ground: 4 });
        let marks = p.sheet.entities.values().filter(|e| matches!(e, Entity::Symbol(s) if s.reference.is_empty())).count();
        assert_eq!(marks, 7);
        assert_eq!(p.wired, vec!["M1.1".to_string(), "SIG_A".into(), "SIG_B".into()], "U2.9 - U2.10 is a jumper inside one unit: a bar, no routed wire");
        assert!(p.tagged.is_empty());
        assert_eq!(p.skipped_pins.len(), 2);
    }

    /// Planning the same request twice gives the same sheet once the random entity ids are replaced by their order of appearance.
    /// 同じ依頼を2回計画すると、ランダムな id を出てくる順の番号に置き換えれば、全く同じシートになる。
    #[test]
    fn planning_twice_gives_the_same_sheet() {
        let (a, b) = (plan(), plan());
        assert_eq!(svg_of(&a.sheet), svg_of(&b.sheet));
        assert_eq!((a.wired, a.labelled, a.marked), (b.wired, b.labelled, b.marked));
    }

    /// A net that leaves the sheet keeps its tag, and the wire to the tag ends exactly on the tag's stub, so the pins and the tag are one net.
    /// シートの外へ出るネットはタグを持ち、そこへ行く配線はタグの短い線の先でちょうど止まるので、ピンとタグは1つのネットになる。
    #[test]
    fn a_leaving_net_with_two_pins_is_wired_to_its_tag() {
        let mut req = request();
        req.columns = vec![vec!["U1".into()], vec!["U2".into()]];
        req.pins.insert("U1".into(), vec!["4".into(), "5".into()]);
        req.pins.insert("U2".into(), vec!["3".into(), "4".into()]);
        let p = plan_function_sheet(&fixture_source(), &open(), &req).expect("valid");
        assert_eq!(p.tagged.len(), 2, "{:?}", p.tagged);
        assert!(p.labelled.is_empty());
        let got = groups(&p.sheet, &["U1", "U2"]);
        let want: BTreeSet<BTreeSet<(String, String)>> = [
            [("U1", "4"), ("U2", "4")].iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
            [("U1", "5"), ("U2", "3")].iter().map(|(a, b)| (a.to_string(), b.to_string())).collect(),
        ]
        .into_iter()
        .collect();
        // U1.4 and U2.4 are SIG_B (TB1.2 is elsewhere), U1.5 and U2.3 are SIG_A (R1.1 is elsewhere)
        assert_eq!(got, want);
        let defs = sheet_symbol_defs(&p.sheet);
        let nets = extract_netlist(&p.sheet, &defs);
        for (net, _) in p.tagged.iter().map(|t| (t.net.clone(), ())) {
            let n = nets.iter().find(|n| n.label.as_deref() == Some(net.as_str())).expect("the tag names a net");
            assert!(n.pins.len() >= 2, "{net}: {:?}", n.pins);
        }
    }

    /// Three planned sheets given with their units: the tag of net SIG_A reserves room for two other sheets, SIG_B for one and M1.1 for none; once all three are drafted every flag, drawn with the destination the export prints, lies inside its reserve and no tag leaves the frame.
    /// 機器つきで3枚を計画すると、SIG_Aの札は他の2枚、SIG_Bは1枚、M1.1はどれも無い分の場所を空ける。3枚を下書きし終えると、書き出しと同じ行き先で描いた旗がどれも空けた場所に収まり、枠からはみ出す札も無い。
    #[test]
    fn per_net_reserves_hold_the_real_flags_of_three_drafted_sheets() {
        let (a, b) = ("Cabinet B terminal side", "Spare relay panel number two");
        let object = |name: &str, units: &[&str], pins: &[(&str, &[&str])]| {
            let mut r = request();
            r.name = name.into();
            r.columns = units.iter().map(|u| vec![u.to_string()]).collect();
            r.pins = pins.iter().map(|(u, k)| (u.to_string(), k.iter().map(|x| x.to_string()).collect())).collect();
            r.power_nets = Vec::new();
            r
        };
        let mut requests = vec![
            object("Main", &["U1", "TB1"], &[]),
            object(a, &["R1"], &[]),
            object(b, &["U2"], &[("U2", &["3", "4"])]),
        ];
        let plan_list = |requests: &[DraftRequest]| -> Vec<PlannedSheet> {
            requests
                .iter()
                .map(|r| {
                    PlannedSheet::Detailed(PlannedSheetDetail {
                        name: r.name.clone(),
                        units: r.columns.iter().flatten().cloned().collect(),
                        pins: r.pins.clone(),
                    })
                })
                .collect()
        };
        let planned = plan_list(&requests);
        for r in &mut requests {
            r.planned_sheets = planned.clone();
        }
        let mut open = Project::new("open");
        open.sheets.clear();
        let mut reserves = Vec::new();
        for r in &requests {
            let model = build_model(&fixture_source(), &open, r).unwrap();
            reserves.push(lay_out(&model, &open, r).unwrap().reserve);
            let p = plan_function_sheet(&fixture_source(), &open, r).expect("valid");
            open.sheets.push(p.sheet);
        }
        let main = &reserves[0];
        let sep = crate::xref::DEST_SEPARATOR;
        assert_eq!(main.text("SIG_A"), format!("{b}{sep}{a}"));
        assert_eq!(main.text("SIG_B"), b);
        assert_eq!(main.text("M1.1"), "");
        for (sheet, reserve) in open.sheets.iter().zip(&reserves) {
            let dests = crate::xref::sheet_tag_dests(&open, sheet.id);
            for (l, _, real) in flags(sheet, &dests) {
                let font = crate::svg::LABEL_FONT * sheet.annotation_scale();
                let room = flag_rect(&l, font, sheet.xref_style, reserve.text(&l.name)).unwrap();
                assert!(
                    room.min.x <= real.min.x + 1e-6 && room.max.x >= real.max.x - 1e-6,
                    "{} on {}: the flag {real:?} is wider than its reserve {room:?}",
                    l.name,
                    sheet.name
                );
            }
        }
        let check = crate::draft_check::check_function_sheets(&fixture_source(), &open, &[]);
        assert!(check.tags_outside.is_empty(), "{:?}", check.tags_outside);
    }

    /// Drafts `requests` in the given order into an empty project; returns the project and, per sheet name, the room its tags reserved.
    fn draft_in_order(requests: &[DraftRequest], order: &[usize]) -> (Project, BTreeMap<String, Reserve>) {
        let mut open = Project::new("open");
        open.sheets.clear();
        let mut reserves = BTreeMap::new();
        for &i in order {
            let r = &requests[i];
            let model = build_model(&fixture_source(), &open, r).unwrap();
            reserves.insert(r.name.clone(), lay_out(&model, &open, r).unwrap().reserve);
            let p = plan_function_sheet(&fixture_source(), &open, r).unwrap_or_else(|e| panic!("{}: {e}", r.name));
            open.sheets.push(p.sheet);
        }
        (open, reserves)
    }

    /// For every order in which three planned sheets are drafted (with U2 kept on one sheet, or divided over two sheets by `pins`, and with one sheet planned by its name only), every flag the export prints fits inside the room its sheet reserved, and the check finds no tag outside the frame.
    /// 計画した3枚をどの順で下書きしても (U2を1枚に載せても、pinsで2枚に分けても、1枚を名前だけで計画しても)、書き出しが描く旗はどれも、そのシートが空けた場所に収まり、検算も枠からはみ出す札を見つけない。
    #[test]
    fn every_drafting_order_keeps_each_flag_inside_its_reserve() {
        let (a, b) = ("Cabinet B terminal side", "Spare relay panel number two");
        let object = |name: &str, columns: &[&[&str]], pins: &[(&str, &[&str])]| {
            let mut r = request();
            r.name = name.into();
            r.columns = columns.iter().map(|c| c.iter().map(|u| u.to_string()).collect()).collect();
            r.pins = pins.iter().map(|(u, k)| (u.to_string(), k.iter().map(|x| x.to_string()).collect())).collect();
            r.power_nets = Vec::new();
            r
        };
        let detail = |r: &DraftRequest| {
            PlannedSheet::Detailed(PlannedSheetDetail {
                name: r.name.clone(),
                units: r.columns.iter().flatten().cloned().collect(),
                pins: r.pins.clone(),
            })
        };
        let whole = vec![object("Main", &[&["U1"], &["TB1"]], &[]), object(a, &[&["R1"]], &[]), object(b, &[&["U2"]], &[("U2", &["3", "4"])])];
        let divided = vec![
            object("Main", &[&["U1"], &["TB1"]], &[]),
            object(a, &[&["R1"], &["U2"]], &[("U2", &["3"])]),
            object(b, &[&["U2"]], &[("U2", &["4"])]),
        ];
        let orders = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
        for (case, mut requests) in [("whole", whole.clone()), ("divided", divided), ("name only", whole)] {
            let mut planned: Vec<PlannedSheet> = requests.iter().map(detail).collect();
            if case == "name only" {
                planned[2] = PlannedSheet::Name(b.into());
            }
            for r in &mut requests {
                r.planned_sheets = planned.clone();
            }
            for order in &orders {
                let (open, reserves) = draft_in_order(&requests, order);
                for sheet in &open.sheets {
                    let reserve = &reserves[&sheet.name];
                    let dests = crate::xref::sheet_tag_dests(&open, sheet.id);
                    for (l, _, real) in flags(sheet, &dests) {
                        let font = crate::svg::LABEL_FONT * sheet.annotation_scale();
                        let room = flag_rect(&l, font, sheet.xref_style, reserve.text(&l.name)).unwrap();
                        assert!(
                            room.min.x <= real.min.x + 1e-6 && room.max.x >= real.max.x - 1e-6,
                            "case {case}, order {order:?}: {} on {}: the flag {real:?} is wider than its reserve {room:?}",
                            l.name,
                            sheet.name
                        );
                    }
                }
                let check = crate::draft_check::check_function_sheets(&fixture_source(), &open, &[]);
                assert!(check.tags_outside.is_empty(), "case {case}, order {order:?}: {:?}", check.tags_outside);
            }
        }
    }

    /// The request whose two leaving nets (SIG_A, SIG_B) have two pins each on the sheet, so each gets a tag and a wire; U1 also carries its power and ground pins (marks).
    fn leaving_request() -> DraftRequest {
        let mut req = request();
        req.columns = vec![vec!["U1".into()], vec!["U2".into()]];
        req.pins.insert("U1".into(), vec!["1".into(), "2".into(), "4".into(), "5".into()]);
        req.pins.insert("U2".into(), vec!["3".into(), "4".into()]);
        req
    }

    fn clearance() -> f64 {
        RouteOptions::default().clearance_mm
    }

    /// Shortest distance between an axis-parallel segment and a rectangle (0 when they touch or cross).
    fn seg_rect_distance(a: Point, b: Point, r: &Rect) -> f64 {
        let (x0, x1, y0, y1) = (a.x.min(b.x), a.x.max(b.x), a.y.min(b.y), a.y.max(b.y));
        let dx = (r.min.x - x1).max(x0 - r.max.x).max(0.0);
        let dy = (r.min.y - y1).max(y0 - r.max.y).max(0.0);
        dx.hypot(dy)
    }

    fn rect_of(points: &[Point]) -> Rect {
        let mut r = Rect::from_corners(points[0], points[0]);
        points.iter().for_each(|p| r.extend(*p));
        r
    }

    /// Every tag of the sheet with the flag the renderer draws: the destination text is `dests[name]` (what other sheets add) or the tag's own `tag_dest`.
    fn flags(sheet: &Sheet, dests: &BTreeMap<String, String>) -> Vec<(NetLabel, crate::net_tag::NetTag, Rect)> {
        let font = crate::svg::LABEL_FONT * sheet.annotation_scale();
        sheet
            .entities
            .values()
            .filter_map(|e| if let Entity::NetLabel(l) = e { Some(l.clone()) } else { None })
            .filter_map(|l| {
                let dest = dests.get(l.name.trim()).map(String::as_str).or_else(|| crate::net_tag::tag_dest(&l));
                let tag = crate::net_tag::net_tag_with(&l, font, sheet.xref_style, dest)?;
                let r = rect_of(&tag.points);
                Some((l, tag, r))
            })
            .collect()
    }

    /// Wire segments that come closer than `gap` to a tag flag. The only segment allowed to touch a flag is the tag's own wire, which ends on the tag's point and lies on the side away from the flag body.
    fn wires_near_flags(sheet: &Sheet, dests: &BTreeMap<String, String>, gap: f64) -> Vec<String> {
        let mut bad = Vec::new();
        for (l, tag, r) in flags(sheet, dests) {
            for w in wires(sheet) {
                for seg in w.points.windows(2) {
                    let own = (0..2).any(|k| {
                        let (end, other) = (seg[k], seg[1 - k]);
                        end.distance_to(&l.at) < 1e-6
                            && (other.y - end.y).abs() < 1e-9
                            && (other.x - end.x) * (tag.text_at.x - end.x) < 0.0
                    });
                    let d = seg_rect_distance(seg[0], seg[1], &r);
                    if !own && d < gap - 1e-6 {
                        bad.push(format!("{} flag {r:?}: segment {:?}-{:?} at {d:.2} mm", l.name, seg[0], seg[1]));
                    }
                }
            }
        }
        bad
    }

    /// How many wire arms meet at a point (an end counts once, a point inside a segment twice).
    fn arms_at(sheet: &Sheet, p: Point) -> usize {
        wires(sheet)
            .iter()
            .flat_map(|w| w.points.windows(2).map(|s| (s[0], s[1])).collect::<Vec<_>>())
            .map(|(a, b)| {
                if a.distance_to(&p) < 1e-6 || b.distance_to(&p) < 1e-6 {
                    1
                } else if crate::netlist::segment_distance(&p, &a, &b) < 1e-6 {
                    2
                } else {
                    0
                }
            })
            .sum()
    }

    /// The rectangles of the marks: graphics and the printed net name.
    fn mark_rects(sheet: &Sheet) -> Vec<(uuid::Uuid, Point, Rect)> {
        let defs = sheet_symbol_defs(sheet);
        let index = DefIndex::new(&defs);
        let texts = crate::tidy::text_boxes(sheet, &defs);
        let mut out = Vec::new();
        for e in sheet.entities.values() {
            let Entity::Symbol(s) = e else { continue };
            if !s.reference.is_empty() {
                continue;
            }
            let def = index.get(s).expect("a mark has a definition");
            let pin = pin_positions(s, &def)[0].1;
            let mut r = symbol_bounds(s, &def).expect("a mark has an outline");
            for t in texts.iter().filter(|t| t.owner == Some(s.id)) {
                r.extend(t.rect.min);
                r.extend(t.rect.max);
            }
            out.push((s.id, pin, r));
        }
        out
    }

    /// Where a leaving net is also wired, no wire runs through or along a tag flag: only the tag's own wire touches its flag, at the tag's point. At a pitch of 5 mm every other wire keeps the router clearance; at 2.5 mm, where adjacent flags are only 0.25 mm apart, no other wire touches a flag.
    /// 出ていくネットをこのシートでも配線するとき、札の旗の上や脇を通る配線は無い。旗に触れるのは、札の点で止まる札自身の線だけ。ピン間隔5mmではほかの線は経路探索のすき間を空け、隣の旗どうしが0.25mmしか離れない2.5mmでも旗に触れる線は無い。
    #[test]
    fn no_wire_runs_through_or_along_a_tag_flag() {
        for (pitch, gap) in [(5.0, clearance()), (2.5, 0.2)] {
            let mut req = leaving_request();
            req.pitch_mm = pitch;
            let p = plan_function_sheet(&fixture_source(), &open(), &req).expect("valid");
            assert_eq!(p.tagged.len(), 2);
            assert!(p.labelled.is_empty(), "pitch {pitch}: {:?}", p.labelled);
            let bad = wires_near_flags(&p.sheet, &BTreeMap::new(), gap);
            assert!(bad.is_empty(), "pitch {pitch}: {bad:#?}");
        }
    }

    /// A tag sits at a free end: exactly one wire reaches its point, and its flag touches no other flag, no unit outline and no mark.
    /// 札は線の行き止まりにある。札の点へ届く線はちょうど1本で、旗はほかの旗にも、機器の外形にも、印にも触れない。
    #[test]
    fn a_tag_hangs_at_a_free_end_clear_of_other_tags_boxes_and_marks() {
        for req in [leaving_request(), { let mut r = leaving_request(); r.pitch_mm = 2.5; r }] {
            let p = plan_function_sheet(&fixture_source(), &open(), &req).expect("valid");
            let fl = flags(&p.sheet, &BTreeMap::new());
            assert_eq!(fl.len(), 2);
            let defs = sheet_symbol_defs(&p.sheet);
            let boxes: Vec<Rect> = crate::tidy::symbol_boxes(&p.sheet, &defs)
                .into_iter()
                .filter(|(id, _)| matches!(p.sheet.entities.get(id), Some(Entity::Symbol(s)) if !s.reference.is_empty()))
                .map(|(_, r)| r)
                .collect();
            let marks = mark_rects(&p.sheet);
            for (i, (l, _, r)) in fl.iter().enumerate() {
                assert_eq!(arms_at(&p.sheet, l.at), 1, "{} is not at a free end", l.name);
                for (_, _, other) in &fl[i + 1..] {
                    assert!(!r.overlaps(other), "{} touches another flag", l.name);
                }
                for b in &boxes {
                    assert!(!r.overlaps(b), "{} touches a unit {b:?}", l.name);
                }
                for (_, _, m) in &marks {
                    assert!(!r.overlaps(m), "{} touches a mark {m:?}", l.name);
                }
            }
        }
    }

    /// Wherever three or more wire arms meet there is a junction dot, also where the wire to the peers leaves a tag's branch at a pin's stub.
    /// 3本以上の線が会う点には必ず接点の丸がある。札への枝と相手への線がピンの短い線の先で分かれるところも同じ。
    #[test]
    fn every_point_where_three_wires_meet_has_a_junction() {
        let mut tagged_with_power = request();
        tagged_with_power.columns = vec![vec!["U1".into(), "M1".into()], vec!["U2".into()]];
        tagged_with_power.pins.insert("U2".into(), vec!["1".into(), "2".into(), "3".into(), "4".into()]);
        for req in [request(), leaving_request(), tagged_with_power] {
            let p = plan_function_sheet(&fixture_source(), &open(), &req).expect("valid");
            let dots = junctions(&p.sheet);
            let ends: Vec<Point> = wires(&p.sheet).iter().flat_map(|w| w.points.clone()).collect();
            for e in ends {
                if arms_at(&p.sheet, e) >= 3 {
                    assert!(dots.iter().any(|d| d.distance_to(&e) < 1e-6), "no junction at {e:?}");
                }
            }
        }
    }

    /// When the router cannot place a leaving net, the tagged pin is joined to its tag by a straight wire and the other pins get same-named labels, so pins and tag stay one net and nothing dangles.
    /// 出ていくネットの配線を引けないときは、札のピンは札まで一直線の線でつなぎ、ほかのピンには同じ名前のラベルを付ける。ピンと札は1つのネットのままで、宙に浮く線も無い。
    #[test]
    fn a_tag_stays_joined_when_the_router_gives_up() {
        let opts = RouteOptions { frame_margin_mm: 100.0, ..RouteOptions::default() };
        let p = plan_with_options(&fixture_source(), &open(), &leaving_request(), &opts).expect("valid");
        assert_eq!(p.labelled.len(), 2, "{:?}", p.labelled);
        let defs = sheet_symbol_defs(&p.sheet);
        let nets = extract_netlist(&p.sheet, &defs);
        for (tag, pins) in [("SIG_A", [("U1", "5"), ("U2", "3")]), ("SIG_B", [("U1", "4"), ("U2", "4")])] {
            for (r, k) in pins {
                assert!(
                    nets.iter().any(|n| n.label.as_deref() == Some(tag) && n.pins.iter().any(|q| q.reference == r && q.pin == k)),
                    "{r}.{k} is not on a net named {tag}"
                );
            }
        }
        let diags = crate::verify::verify_sheet(&p.sheet, &defs);
        assert!(!diags.iter().any(|d| d.code == "erc.dangling_wire" || d.code == "erc.unconnected_pin"), "{diags:?}");
        let fl = flags(&p.sheet, &BTreeMap::new());
        for (l, _, _) in fl.iter().filter(|(l, _, _)| l.attrs.contains_key(crate::net_tag::TAG_ATTR)) {
            assert_eq!(arms_at(&p.sheet, l.at), 1, "{} is joined by one wire", l.name);
        }
    }

    /// Room is kept for the sheet names that other sheets add inside a tag later: after a second sheet with a long name is drafted, no wire of either sheet comes near the widened flags.
    /// 札の中に後から書かれる他のシートの名前の場所を空けておく。長い名前のシートを先に下書きしてから2枚目を下書きしても、どちらのシートの配線も広がった旗に近づかない。
    #[test]
    fn wires_keep_clear_of_the_sheet_names_that_tags_show_later() {
        let mut open = open();
        let mut first = request();
        first.name = "Cabinet B terminal side".into();
        first.columns = vec![vec!["TB1".into(), "R1".into()]];
        first.ground_nets = Some(vec!["GND".into()]);
        let a = plan_function_sheet(&fixture_source(), &open, &first).expect("valid");
        open.sheets.push(a.sheet.clone());
        let mut second = leaving_request();
        second.name = "Main".into();
        let b = plan_function_sheet(&fixture_source(), &open, &second).expect("valid");
        open.sheets.push(b.sheet.clone());
        for sheet in [&a.sheet, &b.sheet] {
            let dests = crate::xref::sheet_tag_dests(&open, sheet.id);
            assert!(!dests.is_empty(), "the tags of {} name the other sheet", sheet.name);
            let bad = wires_near_flags(sheet, &dests, clearance());
            assert!(bad.is_empty(), "{}: {bad:#?}", sheet.name);
        }
    }

    /// No wire comes closer than the router clearance to the graphics or the printed name of a power or ground mark, except the mark's own stub.
    /// 電源・接地の印の図形と名前の文字には、その印自身の短い線を除き、どの配線も経路探索のすき間より近づかない。
    #[test]
    fn wires_keep_clear_of_marks_and_their_names() {
        for req in [request(), leaving_request()] {
            let p = plan_function_sheet(&fixture_source(), &open(), &req).expect("valid");
            let marks = mark_rects(&p.sheet);
            assert!(!marks.is_empty());
            for (_, pin, r) in &marks {
                for w in wires(&p.sheet) {
                    if w.points.iter().any(|q| q.distance_to(pin) < 1e-6) {
                        continue;
                    }
                    for seg in w.points.windows(2) {
                        let d = seg_rect_distance(seg[0], seg[1], r);
                        assert!(d >= clearance() - 1e-6, "segment {:?}-{:?} is {d:.2} mm from the mark {r:?}", seg[0], seg[1]);
                    }
                }
            }
        }
    }

    /// On the finished sheet, unit origins, pins, the ends of stubs, tags and marks are on the 2.5 mm grid; only the bends of routed wires may lie on the 1.25 mm router grid.
    /// 仕上がったシートでは、機器の原点・ピン・短い線の先・札・印は2.5mmの格子の上にある。1.25mmの経路の格子に乗ってよいのは、引いた配線の曲がりだけ。
    #[test]
    fn everything_but_routed_bends_is_on_the_grid() {
        let on = |v: f64| ((v / 2.5) - (v / 2.5).round()).abs() < 1e-6;
        let fine = |v: f64| ((v / 1.25) - (v / 1.25).round()).abs() < 1e-6;
        for mut req in [request(), leaving_request()] {
            for pitch in [5.0, 2.5] {
                req.pitch_mm = pitch;
                let p = plan_function_sheet(&fixture_source(), &open(), &req).expect("valid");
                let defs = sheet_symbol_defs(&p.sheet);
                let pins: Vec<Point> = pin_points(&p.sheet, &defs).into_iter().map(|q| q.pos).collect();
                for e in p.sheet.entities.values() {
                    match e {
                        Entity::Symbol(s) => assert!(on(s.at.x) && on(s.at.y), "{} at {:?}", s.symbol_id, s.at),
                        Entity::NetLabel(l) => assert!(on(l.at.x) && on(l.at.y), "tag {} at {:?}", l.name, l.at),
                        Entity::Wire(w) => {
                            for q in &w.points {
                                assert!(fine(q.x) && fine(q.y), "wire vertex {q:?} is off the router grid");
                            }
                            // 短い線 (ピンから始まる2点の線) は両端とも2.5mmの格子の上
                            if w.points.len() == 2 && pins.iter().any(|q| q.distance_to(&w.points[0]) < 1e-6) {
                                assert!(on(w.points[1].x) && on(w.points[1].y), "stub end {:?}", w.points[1]);
                            }
                        }
                        _ => {}
                    }
                }
                for q in &pins {
                    assert!(on(q.x) && on(q.y), "pin at {q:?} (pitch {pitch})");
                }
            }
        }
    }

    /// No wire, tag or mark leaves the frame's inner area or enters the title block (with its revision rows), on A3 and on A4 with the default frame, and with a frame of wide margins and a large title block.
    /// 配線・札・印は図枠の内側から出ず、表題欄 (改訂欄の行を含む) にも入らない。既定の図枠の A3 と A4 でも、余白が広く表題欄の大きい図枠でも同じ。
    #[test]
    fn nothing_leaves_the_frame_or_enters_the_title_block() {
        let mut wide = crate::frame_template::default_template().clone();
        wide.frame_margins = Some(crate::frame_template::FrameMargins { left: 25.0, right: 20.0, top: 20.0, bottom: 15.0 });
        wide.title_block.w = 250.0;
        wide.title_block.h = 110.0;
        let mut high = crate::frame_template::default_template().clone();
        high.frame_margins = Some(crate::frame_template::FrameMargins { left: 10.0, right: 10.0, top: 30.0, bottom: 10.0 });
        let cases = [
            (DraftPaper::A3, crate::frame_template::default_template().clone()),
            (DraftPaper::A4, crate::frame_template::default_template().clone()),
            (DraftPaper::A3, wide),
            (DraftPaper::A3, high),
        ];
        for (paper, template) in cases {
            for mut req in [leaving_request(), request(), { let mut r = request(); r.columns = vec![vec!["U1".into(), "M1".into()], vec!["U2".into()]]; r }] {
                if paper == DraftPaper::A4 && req.columns.len() == 3 {
                    continue;
                }
                req.paper = paper;
                let p = plan_in(&fixture_source(), &open(), &req, &RouteOptions::default(), &template).expect("valid");
                let (frame, tb) = crate::arrange::frame_areas(&template, &p.sheet);
                let inner = Rect::from_corners(Point::new(frame.0, frame.1), Point::new(frame.2, frame.3));
                let title = Rect::from_corners(Point::new(tb.0, tb.1), Point::new(tb.2, tb.3));
                let inside = |r: &Rect| r.min.x >= inner.min.x && r.min.y >= inner.min.y && r.max.x <= inner.max.x && r.max.y <= inner.max.y;
                let reserve = reserved_dest(&open());
                let font = crate::svg::LABEL_FONT * p.sheet.annotation_scale();
                let mut rects: Vec<(String, Rect)> = Vec::new();
                for w in wires(&p.sheet) {
                    for seg in w.points.windows(2) {
                        rects.push((format!("wire {:?}-{:?}", seg[0], seg[1]), rect_of(seg)));
                    }
                }
                for e in p.sheet.entities.values() {
                    if let Entity::NetLabel(l) = e {
                        if let Some(r) = flag_rect(l, font, p.sheet.xref_style, &reserve) {
                            rects.push((format!("tag {}", l.name), r));
                        }
                    }
                }
                rects.extend(mark_rects(&p.sheet).into_iter().map(|(_, _, r)| ("mark".to_string(), r)));
                for (what, r) in rects {
                    assert!(inside(&r), "{paper:?}: {what} {r:?} leaves the frame {inner:?}");
                    assert!(!r.overlaps(&title), "{paper:?}: {what} enters the title block {title:?}");
                }
            }
        }
    }

    /// The router stays out of the title block rectangle that the layout hands it: a title block put over the middle of a wire that ran freely makes the wire go round it.
    /// 経路探索は、配置の段から渡された表題欄の矩形に入らない。自由に引いた配線の途中に表題欄を置き直すと、配線はそれを避けて回る。
    #[test]
    fn the_router_keeps_out_of_the_title_block_the_layout_reports() {
        let req = request();
        let m = build_model(&fixture_source(), &open(), &req).unwrap();
        let mut l = lay_out(&m, &open(), &req).unwrap();
        let (free, _, _) = route_wires(&m, &l, &RouteOptions::default());
        let (a, b) = wires(&free)
            .iter()
            .flat_map(|w| w.points.windows(2).map(|s| (s[0], s[1])).collect::<Vec<_>>())
            .max_by(|x, y| x.0.distance_to(&x.1).total_cmp(&y.0.distance_to(&y.1)))
            .unwrap();
        let mid = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        let block = Rect::from_corners(Point::new(mid.x - 5.0, mid.y - 5.0), Point::new(mid.x + 5.0, mid.y + 5.0));
        assert!(wires(&free).iter().any(|w| w.points.windows(2).any(|s| seg_rect_distance(s[0], s[1], &block) == 0.0)));
        l.title = (block.min.x, block.min.y, block.max.x, block.max.y);
        let (sheet, _, labelled) = route_wires(&m, &l, &RouteOptions::default());
        assert!(labelled.is_empty(), "{labelled:?}");
        for w in wires(&sheet) {
            for s in w.points.windows(2) {
                assert!(seg_rect_distance(s[0], s[1], &block) > 0.0, "{:?}-{:?} enters {block:?}", s[0], s[1]);
            }
        }
    }

    /// A drafted sheet draws exactly like the golden file (entity ids replaced by their order of appearance). Set MADAKE_UPDATE_DRAFT_GOLDEN=1 to rewrite the file.
    /// 下書きのシートは、ゴールデンファイル (id は出てくる順の番号に置き換え) と全く同じ絵になる。ファイルを作り直すときは MADAKE_UPDATE_DRAFT_GOLDEN=1 を付ける。
    #[test]
    fn a_drafted_sheet_renders_like_the_golden_file() {
        let svg = svg_of(&plan().sheet);
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/draft-sheet.svg");
        if std::env::var_os("MADAKE_UPDATE_DRAFT_GOLDEN").is_some() {
            std::fs::write(&path, &svg).expect("write the golden file");
        }
        let want = std::fs::read_to_string(&path).expect("the golden file exists");
        assert!(svg == want, "the drafted sheet differs from tests/golden/draft-sheet.svg");
    }
}
