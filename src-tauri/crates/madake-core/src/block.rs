//! ブロックシンボル(多ピンの汎用機器)。
//!
//! コントローラ・ドライブ・アンプのような「箱に名前付きピンが並ぶ機器」を、
//! シンボルライブラリを増やさずに1インスタンスずつ定義するための仕組み。
//! 形は [`crate::model::SymbolInstance::attrs`] が持ち、定義 ([`crate::symbol::SymbolDef`])
//! はそこから毎回生成する(`terminal_block_{n}p` などの動的シンボルの発展形)。
//!
//! ```text
//! symbol_id = "block"
//! attrs["block_pins"] = {"left":[{"no":"IN1","name":"入力1"}],"right":[{"no":"OUT1"}]}
//! attrs["block_size"] = {"w":40,"h":30}      // 省略時はピンから自動
//! ```

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::connector::{
    connector_groups, connector_primitives, group_label, outline_segments, ConnectorGroup, ConnectorKind,
    CONNECTOR_IN_MM, CONNECTOR_LABEL_GAP_MM, CONNECTOR_LABEL_INSET_MM, CONNECTOR_LABEL_TOP_MM,
    CONNECTOR_LEAD_MM, CONNECTOR_NUM_GAP_MM, CONNECTOR_NUM_SIDE_MM, CONNECTOR_OUT_MM,
};
use crate::geometry::Point;
use crate::symbol::{PinDef, PinDir, Primitive, SymbolDef, TextSlot};
use crate::symbol::{SLOT_DESC, SLOT_PART, SLOT_RATING, SLOT_TAG};

/// ブロックシンボルの`symbol_id`。
pub const BLOCK_SYMBOL_ID: &str = "block";
/// ピン表を持つ属性キー(JSON)。
pub const BLOCK_PINS_ATTR: &str = "block_pins";
/// 外形寸法を持つ属性キー(JSON `{"w":mm,"h":mm}`)。
pub const BLOCK_SIZE_ATTR: &str = "block_size";

/// ピン間隔を持つ属性キー (`"5"` か `"2.5"`。無ければ5)。
pub const BLOCK_PITCH_ATTR: &str = "block_pitch";
/// ピン番号を外形の外に描くか内に描くかを持つ属性キー (`"outside"` か `"inside"`。無ければ外)。
pub const BLOCK_NUMBERS_ATTR: &str = "block_numbers";
/// リード線 (外形からピン接続点までの線) を描くかを持つ属性キー (`"lead"` か `"none"`。無ければ描く)。
/// `"none"` はピンを外形の辺の上に置く: 配線が箱に直接つながる。番号を内側に描くブロックだけで使える。
pub const BLOCK_LEADS_ATTR: &str = "block_leads";
/// 参照記号・型番を外形の上に描くか中に描くかを持つ属性キー (`"above"` か `"inside"`。無ければ上)。
pub const BLOCK_CAPTION_ATTR: &str = "block_caption";
/// 細かいピン間隔 (mm)。グリッドと同じ。
pub const BLOCK_FINE_PITCH_MM: f64 = 2.5;
/// 細かい間隔のブロックの最小の外形 (mm、幅・高さとも)。
pub const BLOCK_FINE_MIN_MM: f64 = 5.0;
/// 番号を内側に描くとき、左右の番号のあいだに空けるすき間 (mm)。
pub const BLOCK_INSIDE_GAP_MM: f64 = 1.0;

/// ブロックのピン間隔。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockPitch {
    /// 5mm (これまでどおり)。
    #[default]
    Coarse,
    /// 2.5mm。
    Fine,
}

impl BlockPitch {
    /// ピン間隔 (mm)。
    pub fn mm(self) -> f64 {
        match self {
            BlockPitch::Coarse => BLOCK_PIN_PITCH_MM,
            BlockPitch::Fine => BLOCK_FINE_PITCH_MM,
        }
    }
    /// 外形寸法の刻み (mm)。
    pub fn size_step(self) -> f64 {
        match self {
            BlockPitch::Coarse => BLOCK_SIZE_STEP_MM,
            BlockPitch::Fine => BLOCK_FINE_PITCH_MM,
        }
    }
}

/// リード線の有無。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockLeads {
    /// 外形から [`BLOCK_LEAD_MM`] のリード線を描き、その先がピン接続点 (これまでどおり)。
    #[default]
    Lead,
    /// リード線なし。ピン接続点は外形の辺の上。
    None,
}

impl BlockLeads {
    /// 外形からピン接続点までの長さ (mm)。
    pub fn mm(self) -> f64 {
        match self {
            BlockLeads::Lead => BLOCK_LEAD_MM,
            BlockLeads::None => 0.0,
        }
    }
}

/// 参照記号・型番を描く場所。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockCaption {
    /// 外形の上 (これまでどおり。上辺にピンがあれば左上の角)。
    #[default]
    Above,
    /// 外形の中央。
    Inside,
}

/// ピン番号を描く場所 (左右の辺)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockNumbers {
    /// 外形の外、リードの上 (これまでどおり)。
    #[default]
    Outside,
    /// 外形の内側、辺のすぐ内。ピン名は描かない。
    Inside,
}

/// ピンの並びピッチ (mm)。2.5mmグリッドの倍数。
pub const BLOCK_PIN_PITCH_MM: f64 = 5.0;
/// 外形からピン接続点までのリード長 (mm)。
pub const BLOCK_LEAD_MM: f64 = 2.5;
/// ピン番号・ピン名の文字高さ (mm)。
pub const BLOCK_PIN_TEXT_MM: f64 = 2.0;
/// 外形の内側にピン名を置くときの余白 (mm)。
pub const BLOCK_TEXT_PAD_MM: f64 = 1.5;
/// 外形寸法の刻み (mm)。半分にしても2.5mmグリッドに載るように5mm刻みで丸める。
pub const BLOCK_SIZE_STEP_MM: f64 = 5.0;
/// 外形の最小幅 (mm)。
pub const BLOCK_MIN_W_MM: f64 = 15.0;
/// 外形の最小高さ (mm)。
pub const BLOCK_MIN_H_MM: f64 = 10.0;
/// 1つのブロックに置けるピンの最大数(全辺の合計)。
pub const BLOCK_PIN_MAX: usize = 200;
/// 1本のピンが持てる接点の最大数。
pub const BLOCK_CONTACT_MAX: usize = 32;
/// 属性スロット(TAG/型番/機能/定格)の行送り (mm)。
pub const BLOCK_SLOT_PITCH_MM: f64 = 2.5;

/// ブロックの1ピン。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockPin {
    /// ピン番号(文字列。数字に限らず "A0" "VIN" など)。シンボル内で一意。
    pub no: String,
    /// ピン名(空可)。外形の内側に書く。
    #[serde(default)]
    pub name: String,
    /// ポートの接点 (例 `["VBUS","D-","D+","GND"]`)。空 = 普通のピン。描き方は変わらない。
    #[serde(default)]
    pub contacts: Vec<String>,
    /// 外形の辺に描くコネクタの形。`None` = 普通のピン (今までどおりに描く)。絵だけで、接点・ネットは変えない。
    #[serde(default)]
    pub connector: Option<ConnectorKind>,
    /// まとめて1つの本体に描くコネクタ (端子台・JST) の名前 (例 `"J3"`, `"TB1"`)。
    #[serde(default)]
    pub connector_ref: Option<String>,
}

/// ブロックのピン配置(辺ごと)。各辺は上→下 / 左→右の順。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockPins {
    #[serde(default)]
    pub left: Vec<BlockPin>,
    #[serde(default)]
    pub right: Vec<BlockPin>,
    #[serde(default)]
    pub top: Vec<BlockPin>,
    #[serde(default)]
    pub bottom: Vec<BlockPin>,
}

/// ブロックの外形寸法 (mm)。
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockSize {
    pub w: f64,
    pub h: f64,
}

/// 属性から読み取ったブロックの仕様。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BlockSpec {
    pub pins: BlockPins,
    /// 明示指定の外形寸法(ピン間隔の刻みへ切り上げ済み: 5mm間隔は5mm刻み、2.5mm間隔は2.5mm刻み)。未指定なら`None`。
    pub size: Option<BlockSize>,
    /// ピン間隔。
    pub pitch: BlockPitch,
    /// ピン番号を描く場所。
    pub numbers: BlockNumbers,
    /// 参照記号・型番を描く場所。
    pub caption: BlockCaption,
    /// リード線の有無。
    pub leads: BlockLeads,
}

impl BlockSpec {
    /// どれかのピンが接点を持つか (ファイルでは種類 `port_block` で書く。[`crate::model::EntityRepr`])。
    pub fn has_contacts(&self) -> bool {
        self.pins.all().any(|p| !p.contacts.is_empty())
    }

    /// どれかのピンがコネクタの絵を持つか (外形を線分で描き、ファイルでは種類 `port_block` で書く)。
    pub fn has_connectors(&self) -> bool {
        self.pins.all().any(|p| p.connector.is_some())
    }
}

impl BlockPins {
    /// 全辺のピンを (辺, ピン) の順に列挙する。
    fn all(&self) -> impl Iterator<Item = &BlockPin> {
        self.left
            .iter()
            .chain(&self.right)
            .chain(&self.top)
            .chain(&self.bottom)
    }
}

/// `v`を`step`刻みへ切り上げる(浮動小数の誤差ぶんは切り上げない)。
fn snap_up(v: f64, step: f64) -> f64 {
    (v / step - 1e-9).ceil() * step
}

/// 辺のピン`i`番目(全`n`個)の中央揃えオフセット (mm)。
/// 他の動的シンボル([`crate::symbol::dynamic_symbol`])と同じ規則。
pub(crate) fn offset(i: usize, n: usize) -> f64 {
    (i as f64 - (n - 1) as f64 / 2.0) * BLOCK_PIN_PITCH_MM
}

fn is_grid(v: f64) -> bool {
    let k = v / BLOCK_FINE_PITCH_MM;
    (k - k.round()).abs() < 1e-9
}

/// 外形をその軸でずらす量 (mm)。2.5mm間隔で、大きさが2.5mmの奇数倍のとき1.25、ほかは0。
/// ずらすと、原点がグリッドにあるかぎり外形の両端がグリッドに乗る。
fn axis_shift(size: f64, pitch: BlockPitch) -> f64 {
    if pitch == BlockPitch::Fine && !is_grid(size / 2.0) {
        BLOCK_FINE_PITCH_MM / 2.0
    } else {
        0.0
    }
}

/// 辺のピンn本の、外形の中心から測ったオフセット (mm)。中央揃えにしたあと、
/// 外形のずれを足してもグリッドに乗らない列は、列ごと1.25mm (下または右へ) 寄せる。
/// 5mm間隔のブロックでは寄せることが無く、これまでの `offset` と同じ値になる。
fn row_offsets(n: usize, size: f64, pitch: BlockPitch) -> Vec<f64> {
    if n == 0 {
        return Vec::new();
    }
    let first = -((n - 1) as f64) / 2.0 * pitch.mm();
    let nudge = if is_grid(first + axis_shift(size, pitch)) { 0.0 } else { BLOCK_FINE_PITCH_MM / 2.0 };
    (0..n).map(|i| first + i as f64 * pitch.mm() + nudge).collect()
}

/// 文字列を[`BLOCK_PIN_TEXT_MM`]の高さで描いたときの幅 (mm)。全角は半角の約1.7倍。
fn name_width(text: &str) -> f64 {
    crate::tidy::text_width(text, BLOCK_PIN_TEXT_MM)
}

/// 辺のピン名のうち一番長いものの幅 (mm)。ピンが無ければ0。
fn max_name_width(side: &[BlockPin]) -> f64 {
    side.iter().map(|p| name_width(&p.name)).fold(0.0, f64::max)
}

/// 上辺・下辺のピンが外形の内側で使う高さ (mm)。番号だけなら1行、名前もあれば2行、
/// コネクタのまとまりの名前を書く辺はその下にもう1行。
fn caption_inset(side: &[BlockPin]) -> f64 {
    if side.is_empty() {
        0.0
    } else if connector_groups(side).iter().any(|g| group_label(g).is_some()) {
        CONNECTOR_LABEL_INSET_MM
    } else if side.iter().any(|p| !p.name.is_empty()) {
        7.5
    } else {
        5.0
    }
}

/// 属性からブロックの仕様を読み取る。
///
/// - `block_pins`が無ければピン0本(注記だけの箱)
/// - JSONが壊れている・知らないキーがある・ピン番号が空または重複・ピンが多すぎる → エラー
///   (同じ番号は向かい合う辺に1回ずつだけ置ける貫通ピン。両側で同じ名前にし、接点・コネクタの絵は付けない)
/// - `block_pitch` / `block_numbers` が知らない値 → エラー
/// - `block_size`はピン間隔の刻み (5mmまたは2.5mm) へ切り上げ、ピンが収まらない大きさならエラー
/// - `connector_ref` が空・まとめない種類に付いている・2つのまとまりに使われている → エラー ([`check_connectors`])
/// - `block_pitch` 2.5 または `block_numbers` inside のブロックに、コネクタの絵を持つピンがある → エラー
pub fn parse_block_spec(attrs: &BTreeMap<String, String>) -> Result<BlockSpec, String> {
    let pins: BlockPins = match attrs.get(BLOCK_PINS_ATTR) {
        Some(raw) if !raw.trim().is_empty() => serde_json::from_str(raw)
            .map_err(|e| format!("{BLOCK_PINS_ATTR} is not valid JSON: {e}"))?,
        _ => BlockPins::default(),
    };
    let count = pins.all().count();
    if count > BLOCK_PIN_MAX {
        return Err(format!(
            "block has too many pins: {count} (max {BLOCK_PIN_MAX})"
        ));
    }
    // 同じ番号は、向かい合う辺に1回ずつだけ置ける (貫通ピン)。辺は左・右・上・下の順
    let sides: [&Vec<BlockPin>; 4] = [&pins.left, &pins.right, &pins.top, &pins.bottom];
    let mut seen: std::collections::BTreeMap<&str, (usize, &BlockPin, bool)> = Default::default();
    for (side, list) in sides.iter().enumerate() {
        for pin in list.iter() {
            let no = pin.no.trim();
            if no.is_empty() {
                return Err("block pin number must not be empty".into());
            }
            match seen.get_mut(no) {
                None => {
                    seen.insert(no, (side, pin, false));
                }
                // 2本目は、1本目と向かい合う辺 (左右・上下) で、まだ3本目でないときだけ通す
                Some((first_side, first, paired)) => {
                    if *paired || *first_side / 2 != side / 2 || *first_side == side {
                        return Err(format!("duplicate block pin number: {no}"));
                    }
                    if first.name.trim() != pin.name.trim() {
                        return Err(format!(
                            "block pin {no}: both sides of a feed-through pin must have the same name"
                        ));
                    }
                    let plain = |p: &BlockPin| p.contacts.is_empty() && p.connector.is_none();
                    if !plain(first) || !plain(pin) {
                        return Err(format!(
                            "block pin {no}: a feed-through pin cannot have contacts or a connector picture"
                        ));
                    }
                    *paired = true;
                }
            }
            check_contacts(no, &pin.contacts)?;
        }
    }
    check_connectors(&pins)?;

    let pitch = match attrs.get(BLOCK_PITCH_ATTR).map(|s| s.trim()) {
        None | Some("5") => BlockPitch::Coarse,
        Some("2.5") => BlockPitch::Fine,
        Some(other) => return Err(format!("{BLOCK_PITCH_ATTR} must be \"5\" or \"2.5\": {other:?}")),
    };
    let numbers = match attrs.get(BLOCK_NUMBERS_ATTR).map(|s| s.trim()) {
        None | Some("outside") => BlockNumbers::Outside,
        Some("inside") => BlockNumbers::Inside,
        Some(other) => return Err(format!("{BLOCK_NUMBERS_ATTR} must be \"outside\" or \"inside\": {other:?}")),
    };
    let caption = match attrs.get(BLOCK_CAPTION_ATTR).map(|s| s.trim()) {
        None | Some("above") => BlockCaption::Above,
        Some("inside") => BlockCaption::Inside,
        Some(other) => return Err(format!("{BLOCK_CAPTION_ATTR} must be \"above\" or \"inside\": {other:?}")),
    };
    if pitch == BlockPitch::Fine {
        if let Some(pin) = pins.all().find(|p| p.connector.is_some()) {
            return Err(format!(
                "block pin {}: a connector picture cannot be used with {BLOCK_PITCH_ATTR} 2.5",
                pin.no.trim()
            ));
        }
    }
    // コネクタのピンは番号を絵の外に描き、内側の番号の幅も絵の深さを数えないので、組み合わせない
    if numbers == BlockNumbers::Inside {
        if let Some(pin) = pins.all().find(|p| p.connector.is_some()) {
            return Err(format!(
                "block pin {}: a connector picture cannot be used with {BLOCK_NUMBERS_ATTR} inside",
                pin.no.trim()
            ));
        }
    }

    let size = match attrs.get(BLOCK_SIZE_ATTR) {
        Some(raw) if !raw.trim().is_empty() => {
            let s: BlockSize = serde_json::from_str(raw)
                .map_err(|e| format!("{BLOCK_SIZE_ATTR} is not valid JSON: {e}"))?;
            if !(s.w.is_finite() && s.h.is_finite()) || s.w <= 0.0 || s.h <= 0.0 {
                return Err("block_size must be positive".into());
            }
            Some(BlockSize {
                w: snap_up(s.w, pitch.size_step()),
                h: snap_up(s.h, pitch.size_step()),
            })
        }
        _ => None,
    };
    if let Some(size) = size {
        let (need_w, need_h) = min_size(&pins, pitch);
        if size.w < need_w - 1e-9 || size.h < need_h - 1e-9 {
            return Err(format!(
                "block_size {}x{}mm is too small for its pins (needs at least {}x{}mm)",
                size.w, size.h, need_w, need_h
            ));
        }
    }
    let leads = match attrs.get(BLOCK_LEADS_ATTR).map(|s| s.trim()) {
        None | Some("lead") => BlockLeads::Lead,
        Some("none") => BlockLeads::None,
        Some(other) => return Err(format!("{BLOCK_LEADS_ATTR} must be \"lead\" or \"none\": {other:?}")),
    };
    if leads == BlockLeads::None && numbers != BlockNumbers::Inside {
        return Err(format!("{BLOCK_LEADS_ATTR} none needs {BLOCK_NUMBERS_ATTR} inside"));
    }
    Ok(BlockSpec { pins, size, pitch, numbers, caption, leads })
}

/// ピン`no`の接点を調べる: 空でない・ピンの中で重ならない・2つ以上・[`BLOCK_CONTACT_MAX`]まで。
/// 接点が無いのは普通のピンなので通す。
fn check_contacts(no: &str, contacts: &[String]) -> Result<(), String> {
    if contacts.is_empty() {
        return Ok(());
    }
    if contacts.len() > BLOCK_CONTACT_MAX {
        return Err(format!(
            "block pin {no}: too many contacts: {} (max {BLOCK_CONTACT_MAX})",
            contacts.len()
        ));
    }
    if contacts.len() < 2 {
        return Err(format!(
            "block pin {no}: a port needs at least two contacts (one conductor is a plain pin)"
        ));
    }
    let mut seen: std::collections::BTreeSet<&str> = Default::default();
    for c in contacts {
        let c = c.trim();
        if c.is_empty() {
            return Err(format!("block pin {no}: contact names must not be empty"));
        }
        if !seen.insert(c) {
            return Err(format!("block pin {no}: duplicate contact {c}"));
        }
    }
    Ok(())
}

/// コネクタの名前 (`connector_ref`) を調べる: 空でない・まとめる種類 (端子台・JST) だけが持つ・
/// 1つの名前は1つのまとまり (1辺の隣り合うピン、同じ種類) だけが使う。
fn check_connectors(pins: &BlockPins) -> Result<(), String> {
    for pin in pins.all() {
        let Some(reference) = &pin.connector_ref else { continue };
        let no = pin.no.trim();
        if reference.trim().is_empty() {
            return Err(format!("block pin {no}: connector_ref must not be empty"));
        }
        match pin.connector {
            None => {
                return Err(format!(
                    "block pin {no}: connector_ref needs a grouped connector (screw, jst_ph_f, jst_xh_f)"
                ))
            }
            Some(kind) if !kind.is_grouped() => {
                return Err(format!(
                    "block pin {no}: connector_ref is only for screw, jst_ph_f and jst_xh_f, not {}",
                    kind.name()
                ))
            }
            Some(_) => {}
        }
    }
    let mut seen: std::collections::BTreeSet<String> = Default::default();
    for side in [&pins.left, &pins.right, &pins.top, &pins.bottom] {
        for group in connector_groups(side) {
            let Some(reference) = group.reference else { continue };
            if !seen.insert(reference.clone()) {
                return Err(format!(
                    "block pin {}: connector_ref {reference} is used by another group; its pins must be next to each other on one side",
                    side[group.start].no.trim()
                ));
            }
        }
    }
    Ok(())
}

/// ピンがn本の辺に要る長さ (mm)。5mm間隔は n×5 (最小5)、2.5mm間隔は (n+1)×2.5 (最小5)。
fn side_need(n: usize, pitch: BlockPitch) -> f64 {
    match pitch {
        BlockPitch::Coarse => (n as f64 * BLOCK_PIN_PITCH_MM).max(BLOCK_SIZE_STEP_MM),
        BlockPitch::Fine => ((n + 1) as f64 * BLOCK_FINE_PITCH_MM).max(BLOCK_FINE_MIN_MM),
    }
}

/// ピンが収まる最小の外形寸法 (mm)。
fn min_size(pins: &BlockPins, pitch: BlockPitch) -> (f64, f64) {
    let rows = pins.left.len().max(pins.right.len());
    let cols = pins.top.len().max(pins.bottom.len());
    (side_need(cols, pitch), side_need(rows, pitch))
}

/// 向かい合う2辺 (`a`と`b`) の貫通ピンの組のうち、2本が同じ線上 (辺に沿ったオフセットが同じ) に並ぶものの
/// (aの添字, bの添字)。`size`はその辺に沿った外形の寸法。番号を内側に描くとき、この組の番号は1つだけ描く。
fn same_line_pairs(a: &[BlockPin], b: &[BlockPin], size: f64, pitch: BlockPitch) -> Vec<(usize, usize)> {
    let (oa, ob) = (row_offsets(a.len(), size, pitch), row_offsets(b.len(), size, pitch));
    let mut pairs = Vec::new();
    for (i, pa) in a.iter().enumerate() {
        if let Some(j) = b.iter().position(|pb| pb.no.trim() == pa.no.trim()) {
            if (oa[i] - ob[j]).abs() < 1e-9 {
                pairs.push((i, j));
            }
        }
    }
    pairs
}

/// 番号を内側に描くとき、左右の番号が収まる幅 (mm)。左右に同じ線上で向かい合う貫通ピンの組の番号は
/// 外形の中央に1つだけ描くので、その最大幅 (`merged`) と、残りの左の最大幅+残りの右の最大幅+間 (両方あるとき)
/// の大きいほうが収まればよい (上下にピンがあるブロックは組をまとめない)。`h`は外形の高さ (組が同じ線上かは高さで決まる)。
fn inside_numbers_width(pins: &BlockPins, h: f64, pitch: BlockPitch) -> f64 {
    // 上下にピンがあるブロックは組をまとめない (番号は左右それぞれに描く) ので、幅も左右の番号ぶん要る
    let pairs = if pins.top.is_empty() && pins.bottom.is_empty() {
        same_line_pairs(&pins.left, &pins.right, h, pitch)
    } else {
        Vec::new()
    };
    // 組にならなかったピンの、番号の最大幅と本数
    let rest = |list: &[BlockPin], second: bool| -> (f64, usize) {
        let mut widest = 0.0_f64;
        let mut count = 0;
        for (i, p) in list.iter().enumerate() {
            if pairs.iter().any(|q| if second { q.1 == i } else { q.0 == i }) {
                continue;
            }
            widest = widest.max(name_width(p.no.trim()));
            count += 1;
        }
        (widest, count)
    };
    let ((lw, ln), (rw, rn)) = (rest(&pins.left, false), rest(&pins.right, true));
    let merged = pairs.iter().map(|&(i, _)| name_width(pins.left[i].no.trim())).fold(0.0, f64::max);
    let sides = lw + rw + if ln > 0 && rn > 0 { BLOCK_INSIDE_GAP_MM } else { 0.0 };
    2.0 * BLOCK_TEXT_PAD_MM + merged.max(sides)
}

/// 左右の辺の、外形からピン名までの余白 (mm)。コネクタのある辺は絵の内側の深さぶん広い。
fn side_pad(side: &[BlockPin]) -> f64 {
    if side.iter().any(|p| p.connector.is_some()) {
        BLOCK_TEXT_PAD_MM + CONNECTOR_IN_MM
    } else {
        BLOCK_TEXT_PAD_MM
    }
}

/// 左右の辺の内側に書く文字の最大幅 (mm): ピン名と、コネクタのまとまりの名前
/// (1本だけのまとまりはピン名の後ろに続けて書くので、その合計)。
fn side_text_width(side: &[BlockPin]) -> f64 {
    let mut width = max_name_width(side);
    for group in connector_groups(side) {
        let Some(label) = group_label(&group) else { continue };
        let label_w = name_width(&label);
        let name = &side[group.start].name;
        let need = if group.end - group.start >= 2 || name.is_empty() {
            label_w
        } else {
            name_width(name) + CONNECTOR_LABEL_GAP_MM + label_w
        };
        width = width.max(need);
    }
    width
}

/// 左右の辺の内側に書く文字の1つ分: 縦の位置 (外形の中心から) と、外形の内壁からの張り出し (mm)。
struct SideText {
    y: f64,
    extent: f64,
    /// まとまりの名前か (ピン名ではない)。
    label: bool,
}

/// 左右の辺の内側に書く文字 (ピン名とまとまりの名前) の位置。[`pin_layout`] と [`label_at`] の式と同じ。
fn side_texts(side: &[BlockPin]) -> Vec<SideText> {
    let n = side.len();
    let name_in = BLOCK_TEXT_PAD_MM + CONNECTOR_IN_MM;
    let mut out = Vec::new();
    for (i, pin) in side.iter().enumerate() {
        if !pin.name.is_empty() {
            let start = if pin.connector.is_some() { name_in } else { BLOCK_TEXT_PAD_MM };
            out.push(SideText { y: offset(i, n), extent: start + name_width(&pin.name), label: false });
        }
    }
    for group in connector_groups(side) {
        let Some(label) = group_label(&group) else { continue };
        let label_w = name_width(&label);
        let first = offset(group.start, n);
        let count = group.end - group.start;
        let (y, start) = if count >= 2 {
            (first + BLOCK_PIN_PITCH_MM / 2.0 + BLOCK_PIN_PITCH_MM * ((count - 1) / 2) as f64, name_in)
        } else {
            let name = &side[group.start].name;
            (first, if name.is_empty() { name_in } else { name_in + name_width(name) + CONNECTOR_LABEL_GAP_MM })
        };
        out.push(SideText { y, extent: start + label_w, label: true });
    }
    out
}

/// 左右の辺の文字が同じ行 (文字の高さの範囲が重なる) に並び、そのどちらかがまとまりの名前のとき、
/// 左の文字・[`CONNECTOR_LABEL_GAP_MM`]・右の文字が収まる幅 (mm)。そういう行が無ければ0。
/// ピン名どうしだけが並ぶ行は数えない (今までどおり)。
fn shared_row_width(pins: &BlockPins) -> f64 {
    let (left, right) = (side_texts(&pins.left), side_texts(&pins.right));
    let mut need = 0.0_f64;
    for l in &left {
        for r in &right {
            if (l.label || r.label) && (l.y - r.y).abs() < BLOCK_PIN_TEXT_MM - 1e-9 {
                need = need.max(l.extent + CONNECTOR_LABEL_GAP_MM + r.extent);
            }
        }
    }
    need
}

/// 外形寸法を決める。明示指定があればそれ、無ければピン数とピン名の長さから自動。
///
/// 自動のときは
/// - 高さ = 左右のピン数の多い方 × ピン間隔 + ピン間隔(上下の余白)。上下辺のピン名ぶんも足す
/// - 幅 = 左右のピン名の最大幅 + 内側余白 + 中央の空き(ピン間隔)。上下辺のピン数×ピン間隔+ピン間隔も下回らない
///
/// ピン間隔の刻み (5mm間隔は5mm、2.5mm間隔は2.5mm。2.5mm間隔の最小は5×5mm) へ切り上げるので、
/// 大きさは必ずその刻みの倍数になる。外形がグリッドに載るようにずらすのは [`axis_shift`]。
pub fn block_box(spec: &BlockSpec) -> (f64, f64) {
    if let Some(size) = spec.size {
        // 指定の幅でも、左右のまとまりの名前が同じ行で重なるなら、収まる幅まで広げる
        let step = spec.pitch.size_step();
        let mut w = size.w.max(snap_up(shared_row_width(&spec.pins), step));
        if spec.numbers == BlockNumbers::Inside {
            w = w.max(snap_up(inside_numbers_width(&spec.pins, size.h, spec.pitch), step));
        }
        return (w, size.h);
    }
    let pins = &spec.pins;
    let rows = pins.left.len().max(pins.right.len());
    let cols = pins.top.len().max(pins.bottom.len());
    // コネクタの無いブロックでは 1.5 + 1.5 = 3.0 で、今までの 2.0 × 1.5 と同じ値になる
    let p = spec.pitch.mm();
    let (min_w, min_h) = match spec.pitch {
        BlockPitch::Coarse => (BLOCK_MIN_W_MM, BLOCK_MIN_H_MM),
        BlockPitch::Fine => (BLOCK_FINE_MIN_MM, BLOCK_FINE_MIN_MM),
    };
    let need_rows = match spec.pitch {
        BlockPitch::Coarse => rows as f64 * p + p,
        BlockPitch::Fine => side_need(rows, spec.pitch),
    };
    let h = snap_up(
        min_h.max(need_rows).max(caption_inset(&pins.top) + caption_inset(&pins.bottom) + p),
        spec.pitch.size_step(),
    );
    let w_text = match spec.numbers {
        BlockNumbers::Outside => {
            side_pad(&pins.left)
                + side_pad(&pins.right)
                + side_text_width(&pins.left)
                + side_text_width(&pins.right)
                + p
        }
        BlockNumbers::Inside => inside_numbers_width(pins, h, spec.pitch),
    };
    let need_cols = match spec.pitch {
        BlockPitch::Coarse => cols as f64 * p + p,
        BlockPitch::Fine => side_need(cols, spec.pitch),
    };
    let w = min_w.max(w_text).max(need_cols);
    (snap_up(w, spec.pitch.size_step()), h)
}

/// 属性からブロックのシンボル定義を生成する。
///
/// 外形は原点中心の長方形。ピンは辺ごとに中央揃え・5mmピッチで並び、
/// **ピン番号は外形の外側(リードの上)・ピン名は外形の内側**に描く
/// (上下辺は縦に書けないので番号・名前とも内側に置く)。
/// `block_pitch` = "2.5" ならピン間隔2.5mm・大きさの刻み2.5mmで、大きさが2.5mmの奇数倍の軸は外形を1.25mm (右・下へ)、
/// グリッドから外れるピンの列も1.25mmずらす ([`axis_shift`]・[`row_offsets`])。
/// `block_numbers` = "inside" なら左右の番号を外形の内側に描き、ピン名は描かない。
/// ピンに `connector` があると、そのピンは外形から5mmまで伸び、外形の辺にコネクタの絵を描く ([`crate::connector`])。
pub fn block_symbol(attrs: &BTreeMap<String, String>) -> Result<SymbolDef, String> {
    let spec = parse_block_spec(attrs)?;
    Ok(block_symbol_from_spec(&spec))
}

/// ピン表を持たない既定のブロック(部品挿入の見本・`resolve_symbol("block")`の戻り)。
pub fn block_template() -> SymbolDef {
    block_symbol_from_spec(&BlockSpec::default())
}

fn text(x: f64, y: f64, t: &str, height: f64) -> Primitive {
    Primitive::Text {
        at: Point::new(x, y),
        text: t.into(),
        height,
    }
}

fn lead(x1: f64, y1: f64, x2: f64, y2: f64) -> Primitive {
    Primitive::Line {
        pts: vec![Point::new(x1, y1), Point::new(x2, y2)],
    }
}

/// ピンが外形と交わる点。
fn edge_point(dir: PinDir, off: f64, hw: f64, hh: f64) -> Point {
    match dir {
        PinDir::Left => Point::new(-hw, off),
        PinDir::Right => Point::new(hw, off),
        PinDir::Up => Point::new(off, -hh),
        PinDir::Down => Point::new(off, hh),
    }
}

/// 1本のピンの接続点・リードの始点・番号の位置・名前の位置。
struct PinLayout {
    at: Point,
    lead_from: Point,
    num_at: Point,
    name_at: Point,
}

/// ピンの描き位置。コネクタの無いピンは今までどおり。コネクタのあるピンは接続点が外形から5mm、
/// リードは絵の外端から、番号は絵の外 (左右はリードの上、上下はリードの右)、左右の名前は3mm内側から。
fn pin_layout(bp: &BlockPin, off: f64, dir: PinDir, hw: f64, hh: f64, numbers: BlockNumbers, pitch_mm: f64, lead_mm: f64) -> PinLayout {
    let edge = edge_point(dir, off, hw, hh);
    if bp.connector.is_none() {
        let inside = numbers == BlockNumbers::Inside;
        let num_w = name_width(bp.no.trim());
        let (at, num_at, name_at) = match dir {
            PinDir::Left => (
                Point::new(-hw - lead_mm, off),
                if inside {
                    Point::new(-hw + BLOCK_TEXT_PAD_MM + num_w / 2.0, off)
                } else {
                    Point::new(-hw - lead_mm / 2.0, off - BLOCK_PIN_TEXT_MM)
                },
                Point::new(-hw + BLOCK_TEXT_PAD_MM + name_width(&bp.name) / 2.0, off),
            ),
            PinDir::Right => (
                Point::new(hw + lead_mm, off),
                if inside {
                    Point::new(hw - BLOCK_TEXT_PAD_MM - num_w / 2.0, off)
                } else {
                    Point::new(hw + lead_mm / 2.0, off - BLOCK_PIN_TEXT_MM)
                },
                Point::new(hw - BLOCK_TEXT_PAD_MM - name_width(&bp.name) / 2.0, off),
            ),
            PinDir::Up => (
                Point::new(off, -hh - lead_mm),
                Point::new(off, -hh + BLOCK_PIN_TEXT_MM),
                Point::new(off, -hh + BLOCK_PIN_TEXT_MM + pitch_mm / 2.0),
            ),
            PinDir::Down => (
                Point::new(off, hh + lead_mm),
                Point::new(off, hh - BLOCK_PIN_TEXT_MM),
                Point::new(off, hh - BLOCK_PIN_TEXT_MM - pitch_mm / 2.0),
            ),
        };
        return PinLayout { at, lead_from: edge, num_at, name_at };
    }
    let num_w = name_width(bp.no.trim());
    let name_w = name_width(&bp.name);
    let num_out = CONNECTOR_OUT_MM + CONNECTOR_NUM_GAP_MM;
    let name_in = BLOCK_TEXT_PAD_MM + CONNECTOR_IN_MM;
    let num_tb = (CONNECTOR_OUT_MM + CONNECTOR_LEAD_MM) / 2.0;
    let (at, lead_from, num_at, name_at) = match dir {
        PinDir::Left => (
            Point::new(-hw - CONNECTOR_LEAD_MM, off),
            Point::new(-hw - CONNECTOR_OUT_MM, off),
            Point::new(-(hw + num_out + num_w / 2.0), off - BLOCK_PIN_TEXT_MM),
            Point::new(-hw + name_in + name_w / 2.0, off),
        ),
        PinDir::Right => (
            Point::new(hw + CONNECTOR_LEAD_MM, off),
            Point::new(hw + CONNECTOR_OUT_MM, off),
            Point::new(hw + num_out + num_w / 2.0, off - BLOCK_PIN_TEXT_MM),
            Point::new(hw - name_in - name_w / 2.0, off),
        ),
        PinDir::Up => (
            Point::new(off, -hh - CONNECTOR_LEAD_MM),
            Point::new(off, -hh - CONNECTOR_OUT_MM),
            Point::new(off + CONNECTOR_NUM_SIDE_MM + num_w / 2.0, -(hh + num_tb)),
            Point::new(off, -hh + BLOCK_PIN_TEXT_MM + pitch_mm / 2.0),
        ),
        PinDir::Down => (
            Point::new(off, hh + CONNECTOR_LEAD_MM),
            Point::new(off, hh + CONNECTOR_OUT_MM),
            Point::new(off + CONNECTOR_NUM_SIDE_MM + num_w / 2.0, hh + num_tb),
            Point::new(off, hh - BLOCK_PIN_TEXT_MM - pitch_mm / 2.0),
        ),
    };
    PinLayout { at, lead_from, num_at, name_at }
}

/// コネクタのまとまりの名前の位置: 左右の2本以上はまとまりの中ほどのピンの間の空いた行、
/// 1本だけはピン名の後ろ、上下はまとまりの中央で外形から7mm内側。
fn label_at(list: &[BlockPin], offs: &[f64], group: &ConnectorGroup, label: &str, dir: PinDir, hw: f64, hh: f64, pitch_mm: f64) -> Point {
    let first = offs[group.start];
    let label_w = name_width(label);
    let name_in = BLOCK_TEXT_PAD_MM + CONNECTOR_IN_MM;
    match dir {
        PinDir::Left | PinDir::Right => {
            let count = group.end - group.start;
            let (y, start) = if count >= 2 {
                let k = ((count - 1) / 2) as f64;
                (first + pitch_mm / 2.0 + pitch_mm * k, name_in)
            } else {
                let name = &list[group.start].name;
                let start = if name.is_empty() {
                    name_in
                } else {
                    name_in + name_width(name) + CONNECTOR_LABEL_GAP_MM
                };
                (first, start)
            };
            let x = if dir == PinDir::Left {
                -hw + start + label_w / 2.0
            } else {
                hw - start - label_w / 2.0
            };
            Point::new(x, y)
        }
        PinDir::Up | PinDir::Down => {
            let mid = (first + offs[group.end - 1]) / 2.0;
            let y = if dir == PinDir::Up {
                -hh + CONNECTOR_LABEL_TOP_MM
            } else {
                hh - CONNECTOR_LABEL_TOP_MM
            };
            Point::new(mid, y)
        }
    }
}

/// 図形のすべての点を (dx, dy) だけ動かす。図形の種類を増やしたらここがコンパイルエラーになる。
fn translate_primitive(p: &mut Primitive, dx: f64, dy: f64) {
    let mv = |pt: &mut Point| *pt = Point::new(pt.x + dx, pt.y + dy);
    match p {
        Primitive::Line { pts } => pts.iter_mut().for_each(mv),
        Primitive::Circle { center, .. } => mv(center),
        Primitive::Arc { center, .. } => mv(center),
        Primitive::Rect { p1, p2, .. } => {
            mv(p1);
            mv(p2);
        }
        Primitive::Text { at, .. } => mv(at),
    }
}

/// 描かれる外形の2つの角 (シンボルのローカル座標)。2.5mm間隔で大きさが2.5mmの奇数倍の軸は [`axis_shift`] だけずれる。
/// 参照記号の置き場所など、外形を知りたいコードは [`block_box`] から作らずこれを使う。
pub(crate) fn block_outline(spec: &BlockSpec) -> (Point, Point) {
    let (w, h) = block_box(spec);
    let (sx, sy) = (axis_shift(w, spec.pitch), axis_shift(h, spec.pitch));
    (Point::new(-w / 2.0 + sx, -h / 2.0 + sy), Point::new(w / 2.0 + sx, h / 2.0 + sy))
}

fn block_symbol_from_spec(spec: &BlockSpec) -> SymbolDef {
    let (w, h) = block_box(spec);
    let (hw, hh) = (w / 2.0, h / 2.0);
    let (sx, sy) = (axis_shift(w, spec.pitch), axis_shift(h, spec.pitch));
    let (pitch_mm, numbers) = (spec.pitch.mm(), spec.numbers);
    let lead_mm = spec.leads.mm();
    // コネクタの無いブロックは今までどおり長方形1つ。あれば絵の所を空けた線分 (白い口に見せる)
    let mut primitives = if spec.has_connectors() {
        outline_segments(&spec.pins, hw, hh)
    } else {
        vec![Primitive::Rect {
            p1: Point::new(-hw, -hh),
            p2: Point::new(hw, hh),
            filled: false,
        }]
    };
    let mut pins: Vec<PinDef> = Vec::new();
    let pins_spec = &spec.pins;
    // 番号を内側に描くとき、同じ線上で向かい合う貫通ピンの組は番号を1つだけ外形の中央に描く (組の最初のピンが描き、2本目は描かない)
    let (lr_pairs, tb_pairs) = if numbers == BlockNumbers::Inside {
        // 交わる辺 (左右の組なら上下、上下の組なら左右) にピンがあると中央の1つが他の番号と重なるので、そのときはまとめない
        let lr_free = pins_spec.top.is_empty() && pins_spec.bottom.is_empty();
        let tb_free = pins_spec.left.is_empty() && pins_spec.right.is_empty();
        (
            if lr_free { same_line_pairs(&pins_spec.left, &pins_spec.right, h, spec.pitch) } else { Vec::new() },
            if tb_free { same_line_pairs(&pins_spec.top, &pins_spec.bottom, w, spec.pitch) } else { Vec::new() },
        )
    } else {
        (Vec::new(), Vec::new())
    };

    let side = |list: &[BlockPin], dir: PinDir, primitives: &mut Vec<Primitive>, pins: &mut Vec<PinDef>| {
        let vertical = matches!(dir, PinDir::Left | PinDir::Right);
        let offs = row_offsets(list.len(), if vertical { h } else { w }, spec.pitch);
        let groups = connector_groups(list);
        for (i, bp) in list.iter().enumerate() {
            // まとまりの最初のピンの前に、そのまとまりの絵と名前を描く
            if let Some(group) = groups.iter().find(|g| g.start == i) {
                let edges: Vec<Point> = (group.start..group.end)
                    .map(|j| edge_point(dir, offs[j], hw, hh))
                    .collect();
                primitives.extend(connector_primitives(group.kind, dir, &edges));
                if let Some(label) = group_label(group) {
                    let at = label_at(list, &offs, group, &label, dir, hw, hh, pitch_mm);
                    primitives.push(text(at.x, at.y, &label, BLOCK_PIN_TEXT_MM));
                }
            }
            let layout = pin_layout(bp, offs[i], dir, hw, hh, numbers, pitch_mm, lead_mm);
            // リード線なしのブロックは線を描かない (ピン接続点が外形の辺の上)
            if (layout.lead_from.x - layout.at.x).abs() > 1e-9 || (layout.lead_from.y - layout.at.y).abs() > 1e-9 {
                primitives.push(lead(layout.lead_from.x, layout.lead_from.y, layout.at.x, layout.at.y));
            }
            let (is_first, is_second) = match dir {
                PinDir::Left => (lr_pairs.iter().any(|p| p.0 == i), false),
                PinDir::Right => (false, lr_pairs.iter().any(|p| p.1 == i)),
                PinDir::Up => (tb_pairs.iter().any(|p| p.0 == i), false),
                PinDir::Down => (false, tb_pairs.iter().any(|p| p.1 == i)),
            };
            if is_first {
                let centre = if vertical { Point::new(0.0, offs[i]) } else { Point::new(offs[i], 0.0) };
                primitives.push(text(centre.x, centre.y, bp.no.trim(), BLOCK_PIN_TEXT_MM));
            } else if !is_second {
                primitives.push(text(layout.num_at.x, layout.num_at.y, bp.no.trim(), BLOCK_PIN_TEXT_MM));
            }
            if !bp.name.is_empty() && numbers == BlockNumbers::Outside {
                primitives.push(text(layout.name_at.x, layout.name_at.y, &bp.name, BLOCK_PIN_TEXT_MM));
            }
            pins.push(PinDef {
                number: bp.no.trim().into(),
                name: bp.name.clone(),
                at: layout.at,
                dir,
                contacts: bp.contacts.iter().map(|c| c.trim().to_string()).collect(),
            });
        }
    };
    side(&pins_spec.left, PinDir::Left, &mut primitives, &mut pins);
    side(&pins_spec.right, PinDir::Right, &mut primitives, &mut pins);
    side(&pins_spec.top, PinDir::Up, &mut primitives, &mut pins);
    side(&pins_spec.bottom, PinDir::Down, &mut primitives, &mut pins);

    let mut text_slots = center_slots();
    if (sx, sy) != (0.0, 0.0) {
        for p in &mut primitives {
            translate_primitive(p, sx, sy);
        }
        for pin in &mut pins {
            pin.at = Point::new(pin.at.x + sx, pin.at.y + sy);
        }
        for slot in &mut text_slots {
            slot.at = Point::new(slot.at.x + sx, slot.at.y + sy);
        }
    }

    SymbolDef {
        id: BLOCK_SYMBOL_ID.into(),
        name: "Block (generic device)".into(),
        name_ja: "ブロック(汎用機器)".into(),
        category: "block".into(),
        ref_prefix: "U".into(),
        keywords: [
            "block", "device", "module", "controller", "unit", "ブロック", "機器", "モジュール",
            "コントローラ", "汎用",
        ]
        .iter()
        .map(|k| (*k).to_string())
        .collect(),
        primitives,
        pins,
        text_slots,
        free_pins: false,
        net: None,
        sim: None,
    }
}

/// ブロックの属性 (ピン表・大きさ・間隔・番号の位置のどれか) を読めないとき、DXF/KiCadの読み込みが出す警告 (参照の後ろに続ける)。
pub const BLOCK_UNREADABLE_WARNING: &str = "ブロックの属性を読めないので見本の箱にしました";

fn shape_key(def: &SymbolDef) -> String {
    serde_json::to_string(&(&def.primitives, &def.pins)).expect("SymbolDef serialises")
}

/// ブロックの描かれた形 (図形とピン) の書き出し名。見本と同じ形は`block`、他は`block_`+FNV-1a 64のハッシュ16桁 (DXF/KiCad)。
pub fn block_export_id(def: &SymbolDef) -> String {
    let key = shape_key(def);
    if key == shape_key(&block_template()) {
        return BLOCK_SYMBOL_ID.to_string();
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in key.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{BLOCK_SYMBOL_ID}_{h:016x}")
}

/// `block`、または`block_`+小文字16進16桁か。
pub fn is_block_export_id(id: &str) -> bool {
    id == BLOCK_SYMBOL_ID
        || id.strip_prefix("block_").is_some_and(|h| {
            h.len() == 16 && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        })
}

/// 書き出し用の定義。ブロックは形ごとの名前 ([`block_export_id`]) を`id`に持つ複製、ライブラリのシンボルはそのまま借りる。
/// 解決できない (ピン表が読めないブロックなど) ときは`None`。
pub fn export_def<'a>(
    inst: &crate::model::SymbolInstance,
    index: &crate::symbol::DefIndex<'a>,
) -> Option<std::borrow::Cow<'a, SymbolDef>> {
    let def = index.get(inst)?;
    if inst.symbol_id != BLOCK_SYMBOL_ID {
        return Some(def);
    }
    let mut def = def.into_owned();
    def.id = block_export_id(&def);
    Some(std::borrow::Cow::Owned(def))
}

/// 読み込んだ`block_pins` / `block_size` / `block_pitch` / `block_numbers`の空でない値。ブロックとして読めないときは`None`。
pub fn imported_block_attrs(
    pins: Option<&str>,
    size: Option<&str>,
    pitch: Option<&str>,
    numbers: Option<&str>,
    caption: Option<&str>,
    leads: Option<&str>,
) -> Option<BTreeMap<String, String>> {
    let mut attrs = BTreeMap::new();
    for (k, v) in [
        (BLOCK_PINS_ATTR, pins),
        (BLOCK_SIZE_ATTR, size),
        (BLOCK_PITCH_ATTR, pitch),
        (BLOCK_NUMBERS_ATTR, numbers),
        (BLOCK_CAPTION_ATTR, caption),
        (BLOCK_LEADS_ATTR, leads),
    ] {
        if let Some(v) = v.filter(|v| !v.is_empty()) {
            attrs.insert(k.to_string(), v.to_string());
        }
    }
    block_symbol(&attrs).ok().map(|_| attrs)
}

/// テスト用: `block_pins`を持つブロックのシンボルインスタンス(他モジュールのテストが使う)。
#[cfg(test)]
pub(crate) fn test_block(
    reference: &str,
    value: &str,
    x: f64,
    y: f64,
    pins_json: &str,
) -> crate::model::Entity {
    crate::model::Entity::Symbol(crate::model::SymbolInstance {
        id: uuid::Uuid::new_v4(),
        symbol_id: BLOCK_SYMBOL_ID.into(),
        at: Point::new(x, y),
        rotation: 0,
        mirror: false,
        reference: reference.into(),
        value: value.into(),
        attrs: [(BLOCK_PINS_ATTR.to_string(), pins_json.to_string())]
            .into_iter()
            .collect(),
    })
}

/// テスト用: 形の違うブロックを並べたシート (DXF/KiCad 書き出しのテストが共有する)。
#[cfg(test)]
pub(crate) fn export_blocks_sheet() -> crate::model::Sheet {
    use crate::model::{Orientation, PaperSize, Sheet};
    let mut sheet = Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape);
    for e in [
        test_block("B0", "", 40.0, 40.0, "{}"),
        test_block("B1", "", 80.0, 40.0, r#"{"left":[{"no":"1","name":"IN"}]}"#),
        test_block("B2", "", 120.0, 40.0, r#"{ "left": [ {"no": "1", "name": "IN"} ] }"#),
        test_block(
            "B8", "", 160.0, 40.0,
            r#"{"left":[{"no":"1"},{"no":"2"},{"no":"3"},{"no":"4"}],"right":[{"no":"5"},{"no":"6"},{"no":"7"},{"no":"8","name":"OUT"}]}"#,
        ),
        test_block("BU", "", 200.0, 40.0, r#"{"left":[{"no":"1","name":"USB","connector":"usb_a_m"}]}"#),
    ] {
        sheet.entities.insert(e.id(), e);
    }
    sheet
}

/// テスト用: 2.5mm間隔・番号を内側に描くブロック1つだけの用紙と、そのブロックの属性 (往復の試験が共有する)。
#[cfg(test)]
pub(crate) fn fine_block_sheet() -> (crate::model::Sheet, BTreeMap<String, String>) {
    use crate::model::{Orientation, PaperSize, Sheet};
    let mut sheet = Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape);
    let e = test_block("BF", "", 60.0, 60.0, r#"{"left":[{"no":"1"},{"no":"2"}],"right":[{"no":"3"}]}"#);
    let crate::model::Entity::Symbol(mut inst) = e else { unreachable!() };
    inst.attrs.insert(BLOCK_PITCH_ATTR.into(), "2.5".into());
    inst.attrs.insert(BLOCK_NUMBERS_ATTR.into(), "inside".into());
    inst.attrs.insert(BLOCK_SIZE_ATTR.into(), r#"{"w":7.5,"h":7.5}"#.into());
    let attrs = inst.attrs.clone();
    let e = crate::model::Entity::Symbol(inst);
    sheet.entities.insert(e.id(), e);
    (sheet, attrs)
}

/// テスト用: ブロックの属性から引いた、ピンの (番号, 位置) と外形の図形 (往復の前後の比較に使う)。
#[cfg(test)]
pub(crate) fn drawn_like(attrs: &BTreeMap<String, String>) -> (Vec<String>, String) {
    let def = block_symbol(attrs).expect("ブロック定義");
    let pins = def.pins.iter().map(|p| format!("{}@{:?}", p.number, p.at)).collect();
    (pins, format!("{:?}", def.primitives))
}

/// テスト用: ポート (接点を持つピン) のブロックと、ピンの位置 (他モジュールのテストが共有する)。
#[cfg(test)]
pub(crate) mod port_fixtures {
    use crate::geometry::Point;
    use crate::model::{Entity, EntityId, Sheet, SymbolInstance};

    /// USB 2.0 の接点。
    pub(crate) const USB: [&str; 4] = ["VBUS", "D-", "D+", "GND"];

    /// 辺 `side` ("left" / "right") にピン `no` を1本だけ持ち、そのピンが接点 `contacts` を持つブロック。
    pub(crate) fn port_block(reference: &str, x: f64, y: f64, side: &str, no: &str, contacts: &[&str]) -> SymbolInstance {
        let json = format!(
            r#"{{"{side}":[{{"no":"{no}","contacts":{}}}]}}"#,
            serde_json::to_string(contacts).unwrap()
        );
        match super::test_block(reference, "", x, y, &json) {
            Entity::Symbol(s) => s,
            _ => unreachable!(),
        }
    }

    /// 部品 `id` のピン `no` の用紙上の位置 (最初の接続点)。
    pub(crate) fn pin_at(sheet: &Sheet, id: EntityId, no: &str) -> Point {
        let Some(Entity::Symbol(s)) = sheet.entities.get(&id) else { panic!("部品 {id} が無い") };
        let defs = crate::symbol::sheet_symbol_defs(sheet);
        let def = crate::symbol::instance_symbol_def(s, &defs).expect("定義");
        crate::netlist::pin_positions(s, &def)
            .into_iter()
            .find(|(n, _)| n == no)
            .unwrap_or_else(|| panic!("ピン {no} が無い"))
            .1
    }
}

/// 属性スロットは外形の中央に縦積み(TAG→型番→機能→定格)。
fn center_slots() -> Vec<TextSlot> {
    use crate::svg::LABEL_FONT;
    let keys = [SLOT_TAG, SLOT_PART, SLOT_DESC, SLOT_RATING];
    keys.iter()
        .enumerate()
        .map(|(i, key)| TextSlot {
            key: (*key).into(),
            at: Point::new(0.0, (i as f64 - 1.5) * BLOCK_SLOT_PITCH_MM),
            height: LABEL_FONT,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    fn pin_at(def: &SymbolDef, no: &str) -> Point {
        def.pins
            .iter()
            .find(|p| p.number == no)
            .unwrap_or_else(|| panic!("ピン {no} が無い: {:?}", def.pins))
            .at
    }

    fn texts(def: &SymbolDef) -> Vec<(String, Point)> {
        def.primitives
            .iter()
            .filter_map(|p| match p {
                Primitive::Text { at, text, .. } => Some((text.clone(), *at)),
                _ => None,
            })
            .collect()
    }

    fn text_at(def: &SymbolDef, needle: &str) -> Point {
        texts(def)
            .into_iter()
            .find(|(t, _)| t == needle)
            .unwrap_or_else(|| panic!("文字 {needle} が描かれていない: {:?}", texts(def)))
            .1
    }

    fn rect_of(def: &SymbolDef) -> (Point, Point) {
        def.primitives
            .iter()
            .find_map(|p| match p {
                Primitive::Rect { p1, p2, .. } => Some((*p1, *p2)),
                _ => None,
            })
            .expect("外形の長方形")
    }

    const SAMPLE: &str = r#"{"left":[{"no":"IN1","name":"入力1"},{"no":"IN2","name":"入力2"}],
        "right":[{"no":"OUT1","name":"出力1"}]}"#;

    /// A block's export name follows its drawn shape only (pins and picture): whitespace in the table does not matter, a different name, pin, connector or size does, and the template keeps the name `block`.
    /// ブロックの書き出し名は描かれた形(ピンと絵)だけで決まる。表の空白は無関係、名前・ピン・コネクタ・大きさの違いは別名になり、見本は`block`のまま。
    #[test]
    fn block_export_id_follows_the_drawn_shape_only() {
        let id = |pins: &str, size: Option<&str>| {
            let mut a = attrs(&[(BLOCK_PINS_ATTR, pins)]);
            if let Some(sz) = size {
                a.insert(BLOCK_SIZE_ATTR.into(), sz.into());
            }
            block_export_id(&block_symbol(&a).unwrap())
        };
        assert_eq!(block_export_id(&block_template()), "block");
        let b1 = id(r#"{"left":[{"no":"1","name":"IN"}]}"#, None);
        assert_eq!(b1, id(r#"{ "left": [ {"no": "1", "name": "IN"} ] }"#, None));
        let others = [
            id(r#"{"left":[{"no":"1","name":"IN2"}]}"#, None),
            id(r#"{"left":[{"no":"1","name":"IN"},{"no":"2"}]}"#, None),
            id(r#"{"left":[{"no":"1","name":"IN","connector":"usb_a_f"}]}"#, None),
            id(r#"{"left":[{"no":"1","name":"IN"}]}"#, Some(r#"{"w":30,"h":20}"#)),
        ];
        let mut all = vec![b1];
        all.extend(others);
        for (i, a) in all.iter().enumerate() {
            assert!(is_block_export_id(a), "{a}");
            assert_ne!(a, "block");
            for b in &all[i + 1..] {
                assert_ne!(a, b);
            }
        }
        let hex = "0123456789abcdef";
        assert!(is_block_export_id(&format!("block_{hex}")));
        for bad in [
            "block_x".to_string(),
            format!("block_{}", &hex[..15]),
            format!("block_{hex}0"),
            format!("block_{}A", &hex[..15]),
            "relay_coil".to_string(),
        ] {
            assert!(!is_block_export_id(&bad), "{bad}");
        }
    }

    /// One block's export name is pinned to a literal, so a change to the shape key or its serialisation (which would orphan blocks in files written earlier) fails here.
    /// 1つのブロックの書き出し名を文字どおりに固定する。形の鍵や直列化が変わると (前に書いたファイルのブロックが読めなくなるので) ここで失敗する。
    #[test]
    fn block_export_id_is_stable_for_a_known_shape() {
        let def = block_symbol(&attrs(&[(BLOCK_PINS_ATTR, r#"{"left":[{"no":"1"}]}"#)])).unwrap();
        assert_eq!(block_export_id(&def), "block_25f4c16dc8d5fc46");
    }

    /// A block symbol puts its named pins on both sides of a box, 5 mm apart and centred on the 2.5 mm grid, each pointing outwards.
    /// ブロックシンボルは名前付きピンを箱の左右に5mmピッチ・2.5mmグリッド上で中央揃えに並べ、ピンはそれぞれ外向きになる。
    #[test]
    fn block_pins_sit_on_both_sides_five_millimetres_apart() {
        let def = block_symbol(&attrs(&[("block_pins", SAMPLE)])).expect("ブロック定義");
        let (min, max) = rect_of(&def);
        assert_eq!((min.x, min.y, max.x, max.y), (-10.0, -7.5, 10.0, 7.5));
        // 左は2本(±2.5)、右は1本(0)。どちらも辺ごとに中央揃え
        assert_eq!(pin_at(&def, "IN1"), Point::new(-12.5, -2.5));
        assert_eq!(pin_at(&def, "IN2"), Point::new(-12.5, 2.5));
        assert_eq!(pin_at(&def, "OUT1"), Point::new(12.5, 0.0));
        for pin in &def.pins {
            assert_eq!(pin.at.x % 2.5, 0.0, "{}: グリッド外", pin.number);
            assert_eq!(pin.at.y % 2.5, 0.0, "{}: グリッド外", pin.number);
        }
        let dirs: Vec<PinDir> = def.pins.iter().map(|p| p.dir).collect();
        assert_eq!(dirs, vec![PinDir::Left, PinDir::Left, PinDir::Right]);
    }

    /// Pin numbers are drawn outside the outline next to the lead, and pin names inside it.
    /// ピン番号は外形の外側(リードの脇)に、ピン名は外形の内側に描かれる。
    #[test]
    fn pin_numbers_are_drawn_outside_and_pin_names_inside() {
        let def = block_symbol(&attrs(&[("block_pins", SAMPLE)])).expect("ブロック定義");
        let (min, max) = rect_of(&def);
        let number = text_at(&def, "IN1");
        assert!(number.x < min.x, "ピン番号が外形の外に無い: {number:?}");
        let name = text_at(&def, "入力1");
        assert!(
            name.x > min.x && name.x < max.x,
            "ピン名が外形の中に無い: {name:?}"
        );
        assert_eq!(name.y, -2.5, "ピン名はピンと同じ高さ");
        let right_name = text_at(&def, "出力1");
        assert!(right_name.x < max.x && right_name.x > 0.0, "右のピン名は右寄せ");
        assert!(text_at(&def, "OUT1").x > max.x, "右のピン番号は外側");
    }

    /// The box grows automatically with the number of pins and the length of the pin names, always in 5 mm steps.
    /// 外形はピン数とピン名の長さに応じて自動で大きくなり、寸法は必ず5mm刻みになる。
    #[test]
    fn auto_size_grows_with_pin_count_and_name_length() {
        let many = r#"{"left":[{"no":"1"},{"no":"2"},{"no":"3"},{"no":"4"},{"no":"5"},{"no":"6"}]}"#;
        let def = block_symbol(&attrs(&[("block_pins", many)])).expect("ブロック定義");
        let (min, max) = rect_of(&def);
        assert_eq!((max.y - min.y, max.x - min.x), (35.0, 15.0), "6ピン=6×5+5mm、幅は最小の15mm");

        let long = r#"{"left":[{"no":"1","name":"MOTOR-DRIVE-ENABLE"}]}"#;
        let wide = block_symbol(&attrs(&[("block_pins", long)])).expect("ブロック定義");
        let (wmin, wmax) = rect_of(&wide);
        assert!(wmax.x - wmin.x >= 25.0, "長い名前で幅が広がる: {}", wmax.x - wmin.x);
        assert_eq!((wmax.x - wmin.x) % 5.0, 0.0, "5mm刻み");
    }

    /// A block with no pin table at all is still a valid, minimum-sized empty box.
    /// ピン表がまったく無いブロックも、最小サイズの空の箱として有効に作られる。
    #[test]
    fn a_block_without_a_pin_table_is_an_empty_box() {
        let def = block_symbol(&attrs(&[])).expect("ピン表なしでも作れる");
        assert!(def.pins.is_empty());
        let (min, max) = rect_of(&def);
        assert_eq!((max.x - min.x, max.y - min.y), (15.0, 10.0));
        assert_eq!(def.id, BLOCK_SYMBOL_ID);
        assert_eq!(def.ref_prefix, "U");
    }

    /// Top and bottom pins run left to right, and their captions stay inside the box because text cannot be drawn sideways.
    /// 上辺・下辺のピンは左から右へ並び、文字を縦向きに描けないので番号・名前は箱の内側に置かれる。
    #[test]
    fn top_and_bottom_pins_run_left_to_right() {
        let spec = r#"{"top":[{"no":"V+"},{"no":"V-"}],"bottom":[{"no":"PE","name":"接地"}]}"#;
        let def = block_symbol(&attrs(&[("block_pins", spec)])).expect("ブロック定義");
        let (min, max) = rect_of(&def);
        assert_eq!(pin_at(&def, "V+"), Point::new(-2.5, min.y - 2.5));
        assert_eq!(pin_at(&def, "V-"), Point::new(2.5, min.y - 2.5));
        assert_eq!(pin_at(&def, "PE"), Point::new(0.0, max.y + 2.5));
        let up: Vec<PinDir> = def.pins.iter().map(|p| p.dir).collect();
        assert_eq!(up, vec![PinDir::Up, PinDir::Up, PinDir::Down]);
        let caption = text_at(&def, "接地");
        assert!(caption.y < max.y && caption.y > min.y, "上下辺の名前は内側");
    }

    /// An explicit size is rounded up to the 5 mm step so the outline keeps sitting on the grid.
    /// 外形寸法を明示しても5mm刻みへ切り上げられ、外形はグリッド上に載ったままになる。
    #[test]
    fn an_explicit_size_is_rounded_up_to_the_grid() {
        let def = block_symbol(&attrs(&[
            ("block_pins", r#"{"left":[{"no":"1"}]}"#),
            ("block_size", r#"{"w":18,"h":11}"#),
        ]))
        .expect("ブロック定義");
        let (min, max) = rect_of(&def);
        assert_eq!((max.x - min.x, max.y - min.y), (20.0, 15.0));
        assert_eq!(pin_at(&def, "1"), Point::new(-12.5, 0.0));
    }

    /// Two pins with the same number on one side, or a pin with an empty number, are rejected instead of silently drawn.
    /// 同じ辺に同じピン番号が2つある、またはピン番号が空のブロックは、黙って描かれずにエラーになる。
    #[test]
    fn duplicate_or_empty_pin_numbers_are_rejected() {
        let dup = block_symbol(&attrs(&[(
            "block_pins",
            r#"{"left":[{"no":"IN1"},{"no":"IN1"}]}"#,
        )]))
        .expect_err("重複は拒否");
        assert!(dup.contains("duplicate"), "{dup}");
        let empty = block_symbol(&attrs(&[("block_pins", r#"{"left":[{"no":"  "}]}"#)]))
            .expect_err("空番号は拒否");
        assert!(empty.contains("empty"), "{empty}");
    }

    fn feedthrough_err(pins: &str) -> String {
        block_symbol(&attrs(&[("block_pins", pins)])).expect_err("拒否されること")
    }

    /// A pin number may appear once on the left and once on the right, or once on the top and once on the bottom; each side gets its own pin.
    /// 同じピン番号を左と右に1回ずつ、または上と下に1回ずつ置ける。辺ごとにピンが1本ずつできる。
    #[test]
    fn a_pin_number_may_be_on_two_opposite_sides() {
        let lr = block_symbol(&attrs(&[("block_pins", r#"{"left":[{"no":"1"}],"right":[{"no":"1"}]}"#)])).expect("左右は通る");
        let ones: Vec<&crate::symbol::PinDef> = lr.pins.iter().filter(|p| p.number == "1").collect();
        assert_eq!(ones.len(), 2);
        assert!(ones[0].at.x < 0.0 && ones[1].at.x > 0.0, "左のピンが先、右のピンが後: {ones:?}");
        let tb = block_symbol(&attrs(&[("block_pins", r#"{"top":[{"no":"A"}],"bottom":[{"no":"A"}]}"#)])).expect("上下は通る");
        let aa: Vec<&crate::symbol::PinDef> = tb.pins.iter().filter(|p| p.number == "A").collect();
        assert_eq!(aa.len(), 2);
        assert!(aa[0].at.y < 0.0 && aa[1].at.y > 0.0, "上のピンが先、下のピンが後: {aa:?}");
    }

    /// The same pin number twice on one side, on two neighbouring sides, or three times is still refused.
    /// 同じ辺に2回、隣り合う辺に1回ずつ、または3回置いた同じピン番号は、これまでどおり断られる。
    #[test]
    fn a_pin_number_on_the_same_side_neighbouring_sides_or_three_times_is_refused() {
        for pins in [
            r#"{"left":[{"no":"1"},{"no":"1"}]}"#,
            r#"{"left":[{"no":"1"}],"top":[{"no":"1"}]}"#,
            r#"{"right":[{"no":"1"}],"bottom":[{"no":"1"}]}"#,
            r#"{"left":[{"no":"1"}],"right":[{"no":"1"},{"no":"1"}]}"#,
            r#"{"left":[{"no":"1"}],"right":[{"no":"1"}],"top":[{"no":"1"}]}"#,
        ] {
            assert_eq!(feedthrough_err(pins), "duplicate block pin number: 1", "{pins}");
        }
    }

    /// Both sides of a feed-through pin must have the same name once spaces are trimmed; two empty names are the same.
    /// 貫通ピンは両側で同じ名前にする (前後の空白は無視)。名前が両方空なら同じとみなす。
    #[test]
    fn both_sides_of_a_feedthrough_pin_need_the_same_name() {
        assert_eq!(
            feedthrough_err(r#"{"left":[{"no":"1","name":"P"}],"right":[{"no":"1","name":"N"}]}"#),
            "block pin 1: both sides of a feed-through pin must have the same name"
        );
        assert_eq!(
            feedthrough_err(r#"{"left":[{"no":"1","name":"P"}],"right":[{"no":"1"}]}"#),
            "block pin 1: both sides of a feed-through pin must have the same name"
        );
        assert!(block_symbol(&attrs(&[("block_pins", r#"{"left":[{"no":"1","name":"P"}],"right":[{"no":"1","name":" P "}]}"#)])).is_ok());
        assert!(block_symbol(&attrs(&[("block_pins", r#"{"top":[{"no":"1"}],"bottom":[{"no":"1"}]}"#)])).is_ok());
    }

    /// A feed-through pin cannot carry contacts or a connector picture on either side.
    /// 貫通ピンは、どちらの辺にも接点やコネクタの絵を付けられない。
    #[test]
    fn a_feedthrough_pin_cannot_have_contacts_or_a_connector() {
        let msg = "block pin 1: a feed-through pin cannot have contacts or a connector picture";
        assert_eq!(feedthrough_err(r#"{"left":[{"no":"1","contacts":["A","B"]}],"right":[{"no":"1"}]}"#), msg);
        assert_eq!(feedthrough_err(r#"{"left":[{"no":"1"}],"right":[{"no":"1","contacts":["A","B"]}]}"#), msg);
        assert_eq!(feedthrough_err(r#"{"left":[{"no":"1","connector":"usb_a_m"}],"right":[{"no":"1"}]}"#), msg);
        assert_eq!(feedthrough_err(r#"{"left":[{"no":"1"}],"right":[{"no":"1","connector":"usb_a_m"}]}"#), msg);
    }

    /// A fine-pitch block with numbers inside accepts a feed-through pair and keeps all four pins on the 2.5 mm grid.
    /// 2.5mm間隔で番号を内側に書くブロックでも貫通ピンの組を置け、4本のピンはすべて2.5mmのグリッドに乗る。
    #[test]
    fn a_fine_block_with_numbers_inside_accepts_feedthrough_pins() {
        let mut a = fine(r#"{"left":[{"no":"1"},{"no":"2"}],"right":[{"no":"1"},{"no":"2"}]}"#, None);
        a.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
        let def = block_symbol(&a).expect("通る");
        assert_eq!(def.pins.len(), 4);
        for p in &def.pins {
            assert!(is_grid(p.at.x) && is_grid(p.at.y), "{p:?}");
        }
    }

    fn number_texts(def: &SymbolDef) -> Vec<(String, Point)> {
        texts(def).into_iter().filter(|(t, _)| t.chars().all(|c| c.is_ascii_alphanumeric())).collect()
    }

    /// With numbers inside, a feed-through pair on one line draws its number once, centred between the left and right edges at the pins' height, so the block fits in 5 mm.
    /// 番号を内側に描くとき、同じ高さで向かい合う貫通ピンの組は、番号を左右の辺の中央に1つだけ描くので、幅5mmの箱に収まる。
    #[test]
    fn an_inside_feedthrough_pair_draws_its_number_once_at_the_centre() {
        let mut a = fine(r#"{"left":[{"no":"1"},{"no":"2"}],"right":[{"no":"1"},{"no":"2"}]}"#, Some(r#"{"w":5,"h":7.5}"#));
        a.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
        let spec = parse_block_spec(&a).expect("5mm幅でも通る");
        let (p1, p2) = block_outline(&spec);
        assert!((p2.x - p1.x - 5.0).abs() < 1e-9, "幅5mm: {p1:?} {p2:?}");
        let def = block_symbol(&a).unwrap();
        assert_eq!(def.pins.len(), 4);
        for p in &def.pins {
            assert!(is_grid(p.at.x) && is_grid(p.at.y), "{p:?}");
        }
        let nums = number_texts(&def);
        assert_eq!(nums.len(), 2, "{nums:?}");
        let cx = (p1.x + p2.x) / 2.0;
        for (t, at) in &nums {
            assert!((at.x - cx).abs() < 1e-9, "{t} は箱の左右の中央: {at:?}");
            let pin = def.pins.iter().find(|p| &p.number == t).unwrap();
            assert!((at.y - pin.at.y).abs() < 1e-9, "{t} はピンと同じ高さ: {at:?} {pin:?}");
        }
        assert_eq!(nums.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(), ["1", "2"]);
    }

    /// A top and bottom pair with numbers inside draws its number once at the vertical centre, above the pin's column.
    /// 番号を内側に描く上下の貫通ピンの組は、番号を上下の辺の中央に1つだけ、ピンと同じ列の位置に描く。
    #[test]
    fn an_inside_top_bottom_pair_draws_its_number_once_at_the_vertical_centre() {
        let mut a = attrs(&[("block_pins", r#"{"top":[{"no":"7"}],"bottom":[{"no":"7"}]}"#)]);
        a.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
        let spec = parse_block_spec(&a).unwrap();
        let (p1, p2) = block_outline(&spec);
        let def = block_symbol(&a).unwrap();
        let nums = number_texts(&def);
        assert_eq!(nums.len(), 1, "{nums:?}");
        assert!((nums[0].1.y - (p1.y + p2.y) / 2.0).abs() < 1e-9, "{nums:?}");
        assert!(def.pins.iter().all(|p| (p.at.x - nums[0].1.x).abs() < 1e-9), "{nums:?} {:?}", def.pins);
    }

    /// With numbers inside, a feed-through pair is drawn once only when the sides that cross its line have no pins; otherwise every pin keeps its own number and no two numbers share a place.
    /// 番号を内側に描くとき、貫通ピンの番号を1つにまとめるのは、その線と交わる辺にピンが無いときだけ。あるときは各ピンが自分の番号を描き、番号どうしは同じ場所に重ならない。
    #[test]
    fn an_inside_feedthrough_pair_keeps_both_numbers_when_a_crossing_side_has_pins() {
        for (pins, fine_pitch, want) in [
            (r#"{"left":[{"no":"1"}],"right":[{"no":"1"}],"top":[{"no":"2"}],"bottom":[{"no":"2"}]}"#, false, 4),
            (r#"{"left":[{"no":"1"},{"no":"2"}],"right":[{"no":"1"},{"no":"2"}],"top":[{"no":"T"}]}"#, true, 5),
            (r#"{"left":[{"no":"1"}],"top":[{"no":"7"}],"bottom":[{"no":"7"}]}"#, true, 3),
        ] {
            let mut a = if fine_pitch { fine(pins, None) } else { attrs(&[("block_pins", pins)]) };
            a.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
            let def = block_symbol(&a).unwrap_or_else(|e| panic!("{pins}: {e}"));
            let nums = number_texts(&def);
            let texts: Vec<&str> = nums.iter().map(|(t, _)| t.as_str()).collect();
            assert_eq!(nums.len(), want, "{pins}: {texts:?}");
            for (i, (_, a)) in nums.iter().enumerate() {
                for (_, b) in &nums[i + 1..] {
                    assert!(a.distance_to(b) > 0.5, "{pins}: 番号が重なる {nums:?}");
                }
            }
        }
    }

    /// With numbers outside, a feed-through pair draws a number on each side's lead as before: four numbers for two pairs.
    /// 番号を外側に描くときは、貫通ピンも今までどおり各辺のリードの上に番号を描く: 2組で4つ。
    #[test]
    fn an_outside_feedthrough_pair_draws_a_number_on_each_side() {
        let a = fine(r#"{"left":[{"no":"1"},{"no":"2"}],"right":[{"no":"1"},{"no":"2"}]}"#, None);
        assert_eq!(number_texts(&block_symbol(&a).unwrap()).len(), 4);
    }

    /// A size too small for the pins is rejected, and so is a pin table that is not valid JSON or uses an unknown side.
    /// ピンが収まらない寸法は拒否され、JSONとして壊れたピン表や知らない辺名も同じく拒否される。
    #[test]
    fn a_size_too_small_or_a_malformed_pin_table_is_rejected() {
        let small = block_symbol(&attrs(&[
            ("block_pins", r#"{"left":[{"no":"1"},{"no":"2"},{"no":"3"},{"no":"4"}]}"#),
            ("block_size", r#"{"w":20,"h":15}"#),
        ]))
        .expect_err("小さすぎる寸法は拒否");
        assert!(small.contains("too small"), "{small}");

        let broken = block_symbol(&attrs(&[("block_pins", "not json")])).expect_err("壊れたJSON");
        assert!(broken.contains("not valid JSON"), "{broken}");

        let unknown = block_symbol(&attrs(&[("block_pins", r#"{"lef":[{"no":"1"}]}"#)]))
            .expect_err("知らない辺名は拒否");
        assert!(unknown.contains("valid JSON"), "{unknown}");

        let negative = block_symbol(&attrs(&[("block_size", r#"{"w":0,"h":10}"#)]))
            .expect_err("0以下の寸法は拒否");
        assert!(negative.contains("positive"), "{negative}");
    }

    /// A block cannot exceed the pin limit, so a runaway pin table fails loudly.
    /// ブロックのピン数には上限があり、桁違いのピン表ははっきりエラーになる。
    #[test]
    fn too_many_pins_are_rejected() {
        let list: Vec<String> = (0..=BLOCK_PIN_MAX)
            .map(|i| format!(r#"{{"no":"P{i}"}}"#))
            .collect();
        let json = format!(r#"{{"left":[{}]}}"#, list.join(","));
        let err = block_symbol(&attrs(&[("block_pins", &json)])).expect_err("上限超過");
        assert!(err.contains("too many pins"), "{err}");
    }

    /// The attribute slots (tag, part number, function, rating) stack at the centre of the box.
    /// 属性スロット(参照記号・型番・機能・定格)は箱の中央に縦に積まれる。
    #[test]
    fn attribute_slots_stack_at_the_centre_of_the_box() {
        let def = block_symbol(&attrs(&[("block_pins", SAMPLE)])).expect("ブロック定義");
        let keys: Vec<&str> = def.text_slots.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(keys, vec![SLOT_TAG, SLOT_PART, SLOT_DESC, SLOT_RATING]);
        assert!(def.text_slots.iter().all(|s| s.at.x == 0.0));
        let ys: Vec<f64> = def.text_slots.iter().map(|s| s.at.y).collect();
        assert_eq!(ys, vec![-3.75, -1.25, 1.25, 3.75]);
    }

    /// Rotating or mirroring a block moves its pins with the body but never renumbers them.
    /// ブロックを回転・ミラーしてもピンは本体と一緒に動くだけで、番号は付け替わらない。
    #[test]
    fn rotating_or_mirroring_keeps_the_pin_numbers() {
        use crate::model::SymbolInstance;
        use crate::netlist::transform_local;
        let def = block_symbol(&attrs(&[("block_pins", SAMPLE)])).expect("ブロック定義");
        let inst = |rotation: u16, mirror: bool| SymbolInstance {
            id: uuid::Uuid::new_v4(),
            symbol_id: BLOCK_SYMBOL_ID.into(),
            at: Point::new(100.0, 50.0),
            rotation,
            mirror,
            reference: "CTRL1".into(),
            value: String::new(),
            attrs: attrs(&[("block_pins", SAMPLE)]),
        };
        let in1 = pin_at(&def, "IN1");
        assert_eq!(transform_local(in1, &inst(0, false)), Point::new(87.5, 47.5));
        // 90度回転: 左辺のピンが上辺へ回る
        assert_eq!(transform_local(in1, &inst(90, false)), Point::new(102.5, 37.5));
        // ミラー: 左辺のピンが右へ移るが、番号はIN1のまま
        assert_eq!(transform_local(in1, &inst(0, true)), Point::new(112.5, 47.5));
        assert_eq!(def.pins[0].number, "IN1");
    }

    /// A block pin may list contacts; they reach the pin definition in order, trimmed, and a pin without contacts has none.
    /// ブロックのピンは接点を並べて持てる。接点は前後の空白を落とした順にピン定義へ届き、接点の無いピンは空のまま。
    #[test]
    fn contacts_are_trimmed() {
        let def = block_symbol(&attrs(&[(
            "block_pins",
            r#"{"right":[{"no":"U3","name":"USB-A メス","contacts":[" VBUS ","D-","D+","GND"]}],"left":[{"no":"1"}]}"#,
        )]))
        .expect("ブロック定義");
        let u3 = def.pins.iter().find(|p| p.number == "U3").unwrap();
        assert_eq!(u3.contacts, vec!["VBUS", "D-", "D+", "GND"]);
        assert!(u3.is_port());
        let one = def.pins.iter().find(|p| p.number == "1").unwrap();
        assert!(one.contacts.is_empty() && !one.is_port());
        let spec = parse_block_spec(&attrs(&[("block_pins", r#"{"left":[{"no":"1","contacts":["A","B"]}]}"#)])).unwrap();
        assert!(spec.has_contacts());
        assert!(!parse_block_spec(&attrs(&[("block_pins", r#"{"left":[{"no":"1"}]}"#)])).unwrap().has_contacts());
    }

    /// Contacts that are empty, repeated on one pin, alone (one conductor is a plain pin) or more than 32 are refused with the pin number in the message.
    /// 空の接点・1本のピンで重なる接点・1つだけの接点 (導体1本は普通のピン)・32を超える接点は、ピン番号入りの文で断る。
    #[test]
    fn bad_contacts_are_refused() {
        let err = |json: &str| block_symbol(&attrs(&[("block_pins", json)])).expect_err("断る");
        assert!(err(r#"{"left":[{"no":"P1","contacts":["A"," "]}]}"#).contains("P1"));
        assert!(err(r#"{"left":[{"no":"P1","contacts":["A","A"]}]}"#).contains("duplicate contact A"));
        assert!(err(r#"{"left":[{"no":"P1","contacts":["A"]}]}"#).contains("at least two contacts"));
        let many: Vec<String> = (0..=BLOCK_CONTACT_MAX).map(|i| format!("C{i}")).collect();
        let json = format!(r#"{{"left":[{{"no":"P1","contacts":{}}}]}}"#, serde_json::to_string(&many).unwrap());
        assert!(err(&json).contains("too many contacts"));
        // 別のピンと同じ接点名は使ってよい (USB ポートはどれも VBUS を持つ)
        block_symbol(&attrs(&[("block_pins", r#"{"left":[{"no":"1","contacts":["A","B"]},{"no":"2","contacts":["A","B"]}]}"#)]))
            .expect("別のピンなら同じ名前でよい");
    }

    // ---- コネクタの絵 (connector / connector_ref) ----

    /// A pin may name its physical connector, also together with contacts, and null means no connector; an unknown kind is refused as invalid JSON.
    /// ピンは自分のコネクタの形を書ける (接点と一緒でもよい)。null は書かなかったのと同じ。知らない種類は壊れた JSON として断る。
    #[test]
    fn a_connector_kind_is_read_and_an_unknown_one_is_refused() {
        let spec = parse_block_spec(&attrs(&[(
            "block_pins",
            r#"{"top":[{"no":"U1","connector":"usb_a_f","contacts":["VBUS","D-","D+","GND"]}]}"#,
        )]))
        .expect("コネクタと接点の両方を持てる");
        assert_eq!(spec.pins.top[0].connector, Some(ConnectorKind::UsbAF));
        assert!(spec.has_connectors() && spec.has_contacts());
        assert!(!parse_block_spec(&attrs(&[("block_pins", SAMPLE)])).unwrap().has_connectors());
        let nulls = parse_block_spec(&attrs(&[("block_pins", r#"{"left":[{"no":"1","connector":null,"connector_ref":null}]}"#)]))
            .expect("null は書かなかったのと同じ");
        assert!(!nulls.has_connectors());
        let err = parse_block_spec(&attrs(&[("block_pins", r#"{"left":[{"no":"1","connector":"usb_z"}]}"#)]))
            .expect_err("知らない種類");
        assert!(err.starts_with("block_pins is not valid JSON") && err.contains("unknown variant"), "{err}");
    }

    /// A connector name (connector_ref) must not be blank, needs a screw or JST connector and is refused on any other kind, with the pin number in the message.
    /// コネクタの名前 (connector_ref) は空ではいけない・端子台か JST のコネクタが要る・ほかの種類には付けられない。断る文にはピン番号が入る。
    #[test]
    fn a_bad_connector_ref_is_refused() {
        let err = |json: &str| parse_block_spec(&attrs(&[("block_pins", json)])).expect_err("断る");
        assert_eq!(
            err(r#"{"left":[{"no":"P1","connector":"screw","connector_ref":"  "}]}"#),
            "block pin P1: connector_ref must not be empty"
        );
        assert_eq!(
            err(r#"{"left":[{"no":"P1","connector_ref":"J3"}]}"#),
            "block pin P1: connector_ref needs a grouped connector (screw, jst_ph_f, jst_xh_f)"
        );
        assert_eq!(
            err(r#"{"left":[{"no":"P1","connector":"usb_a_f","connector_ref":"J3"}]}"#),
            "block pin P1: connector_ref is only for screw, jst_ph_f and jst_xh_f, not usb_a_f"
        );
    }

    /// One connector name may not be used by two separate connectors: on two sides, split by another pin, or with two different kinds.
    /// 1つのコネクタの名前を別々の2つのコネクタに使うことはできない (2つの辺に分かれる・別のピンで途切れる・種類が違う)。
    #[test]
    fn a_connector_ref_used_by_two_groups_is_refused() {
        let msg = "block pin 2: connector_ref J3 is used by another group; its pins must be next to each other on one side";
        for json in [
            r#"{"left":[{"no":"1","connector":"jst_ph_f","connector_ref":"J3"}],"right":[{"no":"2","connector":"jst_ph_f","connector_ref":"J3"}]}"#,
            r#"{"left":[{"no":"1","connector":"jst_ph_f","connector_ref":"J3"},{"no":"X"},{"no":"2","connector":"jst_ph_f","connector_ref":"J3"}]}"#,
            r#"{"left":[{"no":"1","connector":"screw","connector_ref":"J3"},{"no":"2","connector":"jst_ph_f","connector_ref":"J3"}]}"#,
        ] {
            let err = parse_block_spec(&attrs(&[("block_pins", json)])).expect_err("断る");
            assert_eq!(err, msg, "{json}");
        }
    }

    /// Unnamed connectors may repeat anywhere, and a named connector may run over several neighbouring pins.
    /// 名前の無いコネクタはどこで繰り返してもよく、名前のあるコネクタは隣り合う数本のピンにまたがってよい。
    #[test]
    fn unnamed_connectors_may_repeat() {
        parse_block_spec(&attrs(&[(
            "block_pins",
            r#"{"left":[{"no":"1","connector":"screw"},{"no":"2"},{"no":"3","connector":"screw"}],
                "right":[{"no":"4","connector":"jst_ph_f","connector_ref":"J3"},{"no":"5","connector":"jst_ph_f","connector_ref":"J3"}],
                "top":[{"no":"6","connector":"jst_xh_f"},{"no":"7","connector":"usb_a_f"},{"no":"8","connector":"usb_a_f"}]}"#,
        )]))
        .expect("通る");
    }

    /// AOGU のハブ 39: 上辺に USB-A メス4つ、下辺に micro-B メスの電源、右辺に OTG ケーブルの micro-B オス。
    const HUB_39: &str = r#"{"top":[{"no":"U1","connector":"usb_a_f","contacts":["VBUS","D-","D+","GND"]},{"no":"U2","connector":"usb_a_f","contacts":["VBUS","D-","D+","GND"]},{"no":"U3","connector":"usb_a_f","contacts":["VBUS","D-","D+","GND"]},{"no":"U4","connector":"usb_a_f","contacts":["VBUS","D-","D+","GND"]}],"bottom":[{"no":"PWR","connector":"micro_b_f","contacts":["VBUS","GND"]}],"right":[{"no":"OTG","connector":"micro_b_m","contacts":["VBUS","D-","D+","GND"]}]}"#;
    /// AOGU のアンプ 53: 3.5mm ジャックの入力と、5V・GND の端子台 TB1、スピーカー出力の端子台 TB2。
    const AMP_53: &str = r#"{"left":[{"no":"IN","connector":"jack35_f","contacts":["L","R","GND"]},{"no":"4","name":"5V","connector":"screw","connector_ref":"TB1"},{"no":"5","name":"GND","connector":"screw","connector_ref":"TB1"}],"right":[{"no":"6","name":"SP+","connector":"screw","connector_ref":"TB2"},{"no":"7","name":"SP-","connector":"screw","connector_ref":"TB2"}]}"#;
    /// 左右の辺に全種類と普通のピン (番号は1〜4文字、名前あり・なし)、1本と3本の JST、端子台を並べたブロック。
    const MIXED: &str = r#"{"left":[{"no":"1","name":"VIN"},{"no":"22","name":"USB","connector":"usb_a_f"},{"no":"333","name":"LTE","connector":"usb_a_m"},{"no":"4444","name":"C","connector":"usb_c_f"},{"no":"5","connector":"usb_c_m"},{"no":"6","name":"MIC","connector":"micro_b_f"},{"no":"7","name":"OTG","connector":"micro_b_m"},{"no":"P8","name":"EN"},{"no":"9","name":"SIG","connector":"jst_ph_f","connector_ref":"J9"},{"no":"10","name":"A","connector":"screw","connector_ref":"TB1"},{"no":"11","name":"B","connector":"screw","connector_ref":"TB1"}],"right":[{"no":"20","name":"LINE","connector":"jack35_f"},{"no":"21","connector":"jack35_m"},{"no":"230","name":"DC","connector":"dc_f"},{"no":"24","name":"12V","connector":"dc_m"},{"no":"V+","name":"VCC"},{"no":"31","name":"5V","connector":"jst_xh_f","connector_ref":"J3"},{"no":"32","name":"SDA","connector":"jst_xh_f","connector_ref":"J3"},{"no":"33","name":"GND","connector":"jst_xh_f","connector_ref":"J3"},{"no":"34","name":"L","connector":"screw"},{"no":"35","name":"R","connector":"screw"}]}"#;
    /// 上辺に名前付きの JST と普通のピンと USB-C、下辺に名前付きの端子台と DC ジャックを並べたブロック。
    const TOP_BOTTOM: &str = r#"{"top":[{"no":"1","name":"A","connector":"jst_ph_f","connector_ref":"J5"},{"no":"2","name":"B","connector":"jst_ph_f","connector_ref":"J5"},{"no":"3","name":"C"},{"no":"4","connector":"usb_c_f"}],"bottom":[{"no":"5","connector":"screw","connector_ref":"TB3"},{"no":"6","connector":"screw","connector_ref":"TB3"},{"no":"7","name":"D","connector":"dc_f"}]}"#;

    /// Rust と TS が同じ形を返すことを確かめる共有ファイル (`src/canvas/dynamicSymbol.test.ts` も読む)。
    const CONNECTOR_GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/connector-blocks.json");

    /// golden の例: 13種類×4辺 (普通のピン2本の間に1本、まとめる種類は名前付きの2本) と、AOGU の機器・重なりの例・コネクタの無いブロック。
    fn golden_cases() -> Vec<(String, String, Option<&'static str>)> {
        let mut cases = Vec::new();
        for kind in ConnectorKind::ALL {
            let k = kind.name();
            for side in ["left", "right", "top", "bottom"] {
                let connector = if kind.is_grouped() {
                    format!(r#"{{"no":"1","name":"A","connector":"{k}","connector_ref":"J1"}},{{"no":"2","name":"B","connector":"{k}","connector_ref":"J1"}}"#)
                } else {
                    format!(r#"{{"no":"1","name":"A","connector":"{k}"}}"#)
                };
                let json = format!(r#"{{"{side}":[{{"no":"0"}},{connector},{{"no":"9"}}]}}"#);
                cases.push((format!("{k}-{side}"), json, None));
            }
        }
        for (name, json) in [("aogu-hub-39", HUB_39), ("aogu-amp-53", AMP_53), ("mixed-left-right", MIXED), ("top-bottom", TOP_BOTTOM), ("plain", SAMPLE)] {
            cases.push((name.to_string(), json.to_string(), None));
        }
        // 上辺にコネクタのあるハブ (外形 40 x 25 の指定) と、左右にまとまりの名前を持つ 44 (外形 20 x 20 の指定は名前が収まる幅へ広がる)
        let hub_top = r#"{"top":[{"no":"U1","connector":"usb_a_f"},{"no":"U2","connector":"usb_a_f"},{"no":"U3","connector":"usb_a_f"},{"no":"U4","connector":"usb_a_f"}]}"#;
        cases.push(("hub-top-sized".to_string(), hub_top.to_string(), Some(r#"{"w":40,"h":25}"#)));
        cases.push(("board-44-both-labels".to_string(), BOARD_44.to_string(), Some(r#"{"w":20,"h":20}"#)));
        cases
    }

    /// 数を 1e-6 に丸め、-0 を 0 にそろえる (TS 側も同じ丸めで比べる)。
    fn rounded(v: serde_json::Value) -> serde_json::Value {
        match v {
            serde_json::Value::Number(n) if n.is_f64() => {
                let r = (n.as_f64().unwrap() * 1e6).round() / 1e6;
                serde_json::json!(if r == 0.0 { 0.0 } else { r })
            }
            serde_json::Value::Array(a) => serde_json::Value::Array(a.into_iter().map(rounded).collect()),
            serde_json::Value::Object(o) => serde_json::Value::Object(o.into_iter().map(|(k, v)| (k, rounded(v))).collect()),
            other => other,
        }
    }

    fn golden_json() -> serde_json::Value {
        let cases: Vec<serde_json::Value> = golden_cases()
            .into_iter()
            .map(|(name, pins, size)| {
                let mut a = attrs(&[("block_pins", &pins)]);
                if let Some(size) = size {
                    a.insert("block_size".into(), size.into());
                }
                let def = block_symbol(&a).unwrap_or_else(|e| panic!("{name}: {e}"));
                serde_json::json!({
                    "name": name,
                    "attrs": a,
                    "primitives": rounded(serde_json::to_value(&def.primitives).unwrap()),
                    "pins": rounded(serde_json::to_value(&def.pins).unwrap()),
                })
            })
            .collect();
        serde_json::json!({ "cases": cases })
    }

    /// Every connector kind on every side, and the AOGU hub and amplifier, are drawn exactly as recorded in the golden file that the TypeScript twin is also checked against (rewrite it with MADAKE_UPDATE_GOLDEN=1).
    /// 全種類のコネクタを全辺に置いた例と AOGU のハブ・アンプは、TS 側も突き合わせる golden ファイルのとおりに描かれる (MADAKE_UPDATE_GOLDEN=1 で書き直す)。
    #[test]
    fn connector_blocks_match_the_shared_golden_file() {
        let want = golden_json();
        if std::env::var("MADAKE_UPDATE_GOLDEN").as_deref() == Ok("1") {
            // 1行に1例 (差分が例ごとに読める)
            let lines: Vec<String> = want["cases"].as_array().unwrap().iter().map(|c| serde_json::to_string(c).unwrap()).collect();
            std::fs::write(CONNECTOR_GOLDEN, format!("{{\"cases\":[\n{}\n]}}\n", lines.join(",\n"))).expect("golden を書く");
        }
        let text = std::fs::read_to_string(CONNECTOR_GOLDEN)
            .unwrap_or_else(|_| panic!("{CONNECTOR_GOLDEN} が無い: MADAKE_UPDATE_GOLDEN=1 を付けて作る"));
        let got: serde_json::Value = serde_json::from_str(&text).expect("golden の JSON");
        assert!(got == want, "golden と違う: MADAKE_UPDATE_GOLDEN=1 で書き直し、git diff で変化を確かめる");
    }

    /// 線 (2点以上) の点の並び。線以外は None。
    fn line_pts(p: &Primitive) -> Option<Vec<(f64, f64)>> {
        match p {
            Primitive::Line { pts } => Some(pts.iter().map(|q| (q.x, q.y)).collect()),
            _ => None,
        }
    }

    /// 接続点 `at` で終わるリード (外形側の端, 接続点)。
    fn lead_to(def: &SymbolDef, at: Point) -> (Point, Point) {
        def.primitives
            .iter()
            .find_map(|p| match p {
                Primitive::Line { pts } if pts.len() == 2 && pts[1] == at => Some((pts[0], pts[1])),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{at:?} へのリードが無い"))
    }

    /// A pin with a connector reaches 5 mm out of the box: its lead starts at the outer end of the picture, its number sits 0.25 mm beyond the picture above the lead, and its name starts 3 mm inside; a plain pin in the same block is drawn as before.
    /// コネクタのあるピンは外形から5mm外まで伸びる: リードは絵の外端から始まり、番号はリードの上で絵の0.25mm外、名前は3mm内側から書く。同じブロックの普通のピンは今までどおり。
    #[test]
    fn a_connector_pin_reaches_five_millimetres_out() {
        let def = block_symbol(&attrs(&[(
            "block_pins",
            r#"{"left":[{"no":"1","name":"A","connector":"usb_a_f"},{"no":"2","name":"B"}],"right":[{"no":"33","name":"C","connector":"dc_m"}]}"#,
        )]))
        .expect("ブロック定義");
        // 外形 15×15 (hw = hh = 7.5)
        assert_eq!(pin_at(&def, "1"), Point::new(-12.5, -2.5));
        assert_eq!(lead_to(&def, Point::new(-12.5, -2.5)).0, Point::new(-10.0, -2.5));
        assert_eq!(text_at(&def, "1"), Point::new(-10.85, -4.5), "番号の内端は絵の0.25mm外 (-10.25)");
        assert_eq!(text_at(&def, "A"), Point::new(-3.9, -2.5), "名前は外形の3mm内側から");
        assert_eq!(pin_at(&def, "33"), Point::new(12.5, 0.0));
        assert_eq!(lead_to(&def, Point::new(12.5, 0.0)).0, Point::new(10.0, 0.0));
        assert_eq!(text_at(&def, "33"), Point::new(11.45, -2.0));
        assert_eq!(text_at(&def, "C"), Point::new(3.9, 0.0));
        // 普通のピンは今までどおり (2.5mmのリード、番号はリードの上の中央、名前は1.5mm内側)
        assert_eq!(pin_at(&def, "2"), Point::new(-10.0, 2.5));
        assert_eq!(lead_to(&def, Point::new(-10.0, 2.5)).0, Point::new(-7.5, 2.5));
        assert_eq!(text_at(&def, "2"), Point::new(-8.75, 0.5));
        assert_eq!(text_at(&def, "B"), Point::new(-5.4, 2.5));
    }

    /// On the top and bottom sides a connector pin's number moves outside the box, right of the lead beyond the picture, while its name stays inside like a plain pin's.
    /// 上辺・下辺では、コネクタのあるピンの番号は外形の外 (絵より先のリードの右) へ出て、名前は普通のピンと同じく内側に残る。
    #[test]
    fn top_and_bottom_connector_numbers_sit_beside_the_lead() {
        let def = block_symbol(&attrs(&[(
            "block_pins",
            r#"{"top":[{"no":"U1","name":"A","connector":"usb_a_f"},{"no":"2","name":"B"}],"bottom":[{"no":"P","connector":"micro_b_f"}]}"#,
        )]))
        .expect("ブロック定義");
        // 外形 15×20 (hw = 7.5, hh = 10)
        assert_eq!(pin_at(&def, "U1"), Point::new(-2.5, -15.0));
        assert_eq!(lead_to(&def, Point::new(-2.5, -15.0)).0, Point::new(-2.5, -12.5));
        assert_eq!(text_at(&def, "U1"), Point::new(-0.8, -13.75));
        assert_eq!(text_at(&def, "A"), Point::new(-2.5, -5.5), "名前は普通のピンと同じ4.5mm内側");
        assert_eq!(text_at(&def, "2"), Point::new(2.5, -8.0), "普通のピンの番号は内側のまま");
        assert_eq!(pin_at(&def, "P"), Point::new(0.0, 15.0));
        assert_eq!(text_at(&def, "P"), Point::new(1.1, 13.75));
    }

    /// A block without connectors keeps its single outline rectangle, while a block with connectors draws the outline as lines with a gap exactly where each picture crosses the edge.
    /// コネクタの無いブロックは外形の長方形1つのまま。コネクタのあるブロックは、絵が辺と交わる所だけを空けた線分で外形を描く。
    #[test]
    fn the_outline_has_gaps_exactly_at_the_pictures() {
        let plain = block_symbol(&attrs(&[("block_pins", SAMPLE)])).expect("ブロック定義");
        assert!(matches!(plain.primitives[0], Primitive::Rect { filled: false, .. }));
        assert_eq!(plain.primitives.iter().filter(|p| matches!(p, Primitive::Rect { .. })).count(), 1);

        let def = block_symbol(&attrs(&[(
            "block_pins",
            r#"{"left":[{"no":"1","connector":"usb_a_f"},{"no":"2"},{"no":"3","connector":"screw"},{"no":"4","connector":"screw"}]}"#,
        )]))
        .expect("ブロック定義");
        // 外形 15×25 (hw = 7.5, hh = 12.5)。左辺のピンは y = -7.5, -2.5, 2.5, 7.5
        let outline: Vec<Vec<(f64, f64)>> = def.primitives[..6].iter().map(|p| line_pts(p).expect("線")).collect();
        assert_eq!(
            outline,
            vec![
                vec![(-7.5, -12.5), (7.5, -12.5)],
                vec![(7.5, -12.5), (7.5, 12.5)],
                vec![(-7.5, 12.5), (7.5, 12.5)],
                vec![(-7.5, -12.5), (-7.5, -9.5)],
                vec![(-7.5, -5.5), (-7.5, 0.5)],
                vec![(-7.5, 9.5), (-7.5, 12.5)],
            ]
        );
        assert!(matches!(def.primitives[6], Primitive::Rect { .. }), "外形の次は1本目のピンの絵");
    }

    /// The automatic outline grows by 1.5 mm on each side that has a connector, because the names there start 3 mm inside instead of 1.5 mm.
    /// 外形の自動寸法は、コネクタのある辺ごとに1.5mm広がる (その辺のピン名は1.5mmではなく3mm内側から書くため)。
    #[test]
    fn the_outline_makes_room_for_connector_bodies() {
        let size = |json: &str| block_box(&parse_block_spec(&attrs(&[("block_pins", json)])).unwrap());
        assert_eq!(size(r#"{"left":[{"no":"1","name":"ABCDE"}],"right":[{"no":"2","name":"ABCDE"}]}"#), (20.0, 10.0));
        assert_eq!(
            size(r#"{"left":[{"no":"1","name":"ABCDE","connector":"usb_a_f"}],"right":[{"no":"2","name":"ABCDE"}]}"#),
            (25.0, 10.0),
            "3 + 1.5 + 6 + 6 + 5 = 21.5 → 25"
        );
    }

    /// 点が (x, y) にほぼ等しいことを確かめる (文字幅の足し算の丸め誤差を許す)。
    fn assert_near(p: Point, x: f64, y: f64) {
        assert!((p.x - x).abs() < 1e-9 && (p.y - y).abs() < 1e-9, "{p:?} は ({x}, {y}) ではない");
    }

    /// Group names are drawn where nothing else is: between the middle pins of a 2- or 3-pin group, after the pin name for a 1-pin group, 7 mm inside on the top; JST prints its series and an unnamed screw block prints nothing.
    /// まとまりの名前は空いた所に書く: 2〜3本のまとまりは中ほどのピンの間、1本だけならピン名の後ろ、上辺は7mm内側。JST はシリーズを書き、名前の無い端子台は何も書かない。
    #[test]
    fn group_labels_sit_in_the_free_places() {
        let def = block_symbol(&attrs(&[(
            "block_pins",
            r#"{"left":[{"no":"1","name":"SIG","connector":"jst_ph_f","connector_ref":"J9"},
                {"no":"2","name":"A","connector":"screw","connector_ref":"TB1"},{"no":"3","name":"B","connector":"screw","connector_ref":"TB1"},
                {"no":"4","name":"X","connector":"jst_xh_f"},{"no":"5","name":"Y","connector":"jst_xh_f"},{"no":"6","name":"Z","connector":"jst_xh_f"},
                {"no":"7","connector":"screw"},{"no":"8","connector":"screw"}],
               "right":[{"no":"9","connector":"jst_ph_f"}],
               "top":[{"no":"T1","connector":"jst_ph_f","connector_ref":"J3"},{"no":"T2","connector":"jst_ph_f","connector_ref":"J3"}]}"#,
        )]))
        .expect("ブロック定義");
        // 外形 25×45 (hw = 12.5, hh = 22.5)。左辺のピンは y = -17.5, -12.5, …, 17.5
        assert_near(text_at(&def, "J9 PH"), -1.9, -17.5); // 1本: 名前 SIG (3.6mm) の1mm後ろ
        assert_near(text_at(&def, "TB1"), -7.7, -10.0); // 2本: 1本目と2本目の間
        assert_near(text_at(&def, "XH"), -8.3, 5.0); // 3本: 2本目と3本目の間
        assert_near(text_at(&def, "PH"), 8.3, 0.0); // 右辺の1本・名前なし: 3mm内側から
        assert_near(text_at(&def, "J3 PH"), 0.0, -15.5); // 上辺: まとまりの中央、7mm内側
        let labels = texts(&def).into_iter().filter(|(t, _)| t.contains("PH") || t.contains("XH") || t.starts_with("TB")).count();
        assert_eq!(labels, 5, "名前の無い端子台 (7・8) には何も書かない");
    }

    /// The automatic outline also makes room for group names: a name wider than the pin names widens the box, and a top side that prints a group name keeps 10 mm inside the box.
    /// 外形の自動寸法はまとまりの名前の場所も取る: ピン名より広い名前で箱が広がり、まとまりの名前を書く上辺は箱の内側に10mmを取る。
    #[test]
    fn the_outline_makes_room_for_group_names() {
        let size = |json: &str| block_box(&parse_block_spec(&attrs(&[("block_pins", json)])).unwrap());
        assert_eq!(size(r#"{"left":[{"no":"1","name":"SIG","connector":"jst_ph_f","connector_ref":"J9"}]}"#), (25.0, 10.0), "3 + 1.5 + (3.6 + 1 + 6) + 5 = 20.1 → 25");
        assert_eq!(size(r#"{"left":[{"no":"1","name":"SIG","connector":"jst_ph_f"}]}"#), (20.0, 10.0), "PH だけなら 16.5 → 20");
        assert_eq!(size(r#"{"top":[{"no":"1","connector":"jst_ph_f"},{"no":"2","connector":"jst_ph_f"}]}"#), (15.0, 15.0));
        assert_eq!(size(r#"{"top":[{"no":"1"},{"no":"2"}]}"#), (15.0, 10.0));
    }

    /// 正の面積で重なるか (辺が触れるだけは重ならない)。幅0の線分の箱にもそのまま使える。
    fn overlaps(a: (Point, Point), b: (Point, Point)) -> bool {
        a.0.x < b.1.x - 1e-9 && b.0.x < a.1.x - 1e-9 && a.0.y < b.1.y - 1e-9 && b.0.y < a.1.y - 1e-9
    }

    /// 仕様 §2.5 の重なりの決まりを1つのブロックで確かめる。
    fn assert_nothing_overlaps(json: &str) {
        assert_nothing_overlaps_sized(json, None);
    }

    /// [`assert_nothing_overlaps`] に、`block_size` の指定と、参照記号・型番の文字も加えたもの。
    fn assert_nothing_overlaps_sized(json: &str, size: Option<&str>) {
        use crate::connector::primitive_extent;
        let mut pairs = vec![("block_pins", json)];
        if let Some(size) = size {
            pairs.push(("block_size", size));
        }
        let a = attrs(&pairs);
        let spec = parse_block_spec(&a).expect("ピン表");
        let def = block_symbol(&a).expect("ブロック定義");
        let (w, h) = block_box(&spec);
        let (hw, hh) = (w / 2.0, h / 2.0);
        // 普通のピンの番号は今でも外形をまたぐことがある (仕様 §2.5 (c) の対象外)
        let plain_numbers: Vec<&str> = spec.pins.all().filter(|p| p.connector.is_none()).map(|p| p.no.trim()).collect();
        let mut text_boxes: Vec<(String, (Point, Point))> = def
            .primitives
            .iter()
            .filter_map(|p| match p {
                Primitive::Text { at, text, height } => {
                    let half = crate::tidy::text_width(text, *height) / 2.0;
                    let lo = Point::new(at.x - half, at.y - height / 2.0);
                    let hi = Point::new(at.x + half, at.y + height / 2.0);
                    Some((text.clone(), (lo, hi)))
                }
                _ => None,
            })
            .collect();
        // 参照記号と型番 (ベースラインは外形の上端から REF_LABEL_DY / VALUE_LABEL_DY 上。文字はベースラインから上へ LABEL_FONT)
        // 外形の指定もインスタンスに載せる (配置されるブロックと同じ属性)
        let (mut inst, _) = placed(json, 0, false);
        if let Some(size) = size {
            inst.attrs.insert(BLOCK_SIZE_ATTR.into(), size.into());
        }
        let top = crate::svg::symbol_top_y(&inst, &def);
        for (caption, dy) in [("39", crate::svg::REF_LABEL_DY), ("USBハブ", crate::svg::VALUE_LABEL_DY)] {
            let half = crate::tidy::text_width(caption, crate::svg::LABEL_FONT) / 2.0;
            let baseline = top - dy;
            text_boxes.push((
                format!("キャプション {caption}"),
                (Point::new(-half, baseline - crate::svg::LABEL_FONT), Point::new(half, baseline)),
            ));
        }
        let mut pictures = Vec::new();
        for (list, dir) in [
            (&spec.pins.left, PinDir::Left),
            (&spec.pins.right, PinDir::Right),
            (&spec.pins.top, PinDir::Up),
            (&spec.pins.bottom, PinDir::Down),
        ] {
            let n = list.len();
            for g in connector_groups(list) {
                let edges: Vec<Point> = (g.start..g.end).map(|j| edge_point(dir, offset(j, n), hw, hh)).collect();
                let prims = connector_primitives(g.kind, dir, &edges);
                let (lo, hi) = prims.iter().map(primitive_extent).fold(
                    (Point::new(f64::INFINITY, f64::INFINITY), Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY)),
                    |(lo, hi), (a, b)| (Point::new(lo.x.min(a.x), lo.y.min(a.y)), Point::new(hi.x.max(b.x), hi.y.max(b.y))),
                );
                pictures.push((lo, hi));
            }
        }
        let leads: Vec<(Point, Point)> = def
            .pins
            .iter()
            .map(|p| {
                let (from, to) = lead_to(&def, p.at);
                primitive_extent(&lead(from.x, from.y, to.x, to.y))
            })
            .collect();
        let outline: Vec<(Point, Point)> = outline_segments(&spec.pins, hw, hh).iter().map(primitive_extent).collect();
        for (i, (ta, a)) in text_boxes.iter().enumerate() {
            for (tb, b) in &text_boxes[i + 1..] {
                assert!(!overlaps(*a, *b), "文字 {ta} と {tb} が重なる");
            }
            for p in &pictures {
                assert!(!overlaps(*a, *p), "文字 {ta} が絵 {p:?} に重なる");
            }
            for l in &leads {
                assert!(!overlaps(*a, *l), "文字 {ta} がリード {l:?} に重なる");
            }
            if !plain_numbers.contains(&ta.as_str()) {
                for o in &outline {
                    assert!(!overlaps(*a, *o), "文字 {ta} が外形 {o:?} に重なる");
                }
            }
        }
    }

    /// Regression test for Tasks 5-6 (already passes): with connectors of every kind, plain pins, pin numbers of one to four characters, names and group names, no two texts overlap and no text covers a picture, a lead or the box outline (the AOGU hub and amplifier included).
    /// Task 5〜6 の挙動の回帰テスト (最初から通る): 全種類のコネクタ・普通のピン・1〜4文字のピン番号・ピン名・まとまりの名前を並べても、文字同士は重ならず、文字が絵・リード・外形に重ならない (AOGU のハブとアンプも含む)。
    #[test]
    fn connector_texts_never_overlap() {
        for json in [MIXED, HUB_39, AMP_53, TOP_BOTTOM] {
            assert_nothing_overlaps(json);
        }
        // AOGU のハブ 39 の上辺 (外形 40 x 25) と、左右にまとまりの名前を持つ 44 (外形 20 x 20 の指定)
        assert_nothing_overlaps_sized(HUB_39, Some(r#"{"w":40,"h":25}"#));
        assert_nothing_overlaps_sized(BOARD_44, Some(r#"{"w":20,"h":20}"#));
    }

    /// 44: the LTE power board of the AOGU drawing. Pins 1-4 on the left and 5-8 on the right each form a 4-pin JST XH, and both group names are drawn on the same row.
    const BOARD_44: &str = r#"{"left":[{"no":"1","name":"VBUS","connector":"jst_xh_f","connector_ref":"X44B"},{"no":"2","name":"D-","connector":"jst_xh_f","connector_ref":"X44B"},{"no":"3","name":"D+","connector":"jst_xh_f","connector_ref":"X44B"},{"no":"4","name":"GND","connector":"jst_xh_f","connector_ref":"X44B"}],"right":[{"no":"5","name":"VBUS","connector":"jst_xh_f","connector_ref":"X44A"},{"no":"6","name":"D-","connector":"jst_xh_f","connector_ref":"X44A"},{"no":"7","name":"D+","connector":"jst_xh_f","connector_ref":"X44A"},{"no":"8","name":"GND","connector":"jst_xh_f","connector_ref":"X44A"}]}"#;

    /// The text box (lo, hi) of the text drawn at `needle`.
    fn text_box(def: &SymbolDef, needle: &str) -> (Point, Point) {
        def.primitives
            .iter()
            .find_map(|p| match p {
                Primitive::Text { at, text, height } if text == needle => {
                    let half = crate::tidy::text_width(text, *height) / 2.0;
                    Some((Point::new(at.x - half, at.y - height / 2.0), Point::new(at.x + half, at.y + height / 2.0)))
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("文字 {needle} が描かれていない"))
    }

    /// With group names on both the left and the right, the box is wide enough that the two names, which share a row, keep the text gap between them (the 20 x 20 mm size of block 44 used to put them on top of each other).
    /// 左右の両方にまとまりの名前があるときは、同じ行に並ぶ2つの名前が文字の間隔を空けて収まる幅になる (ブロック 44 の 20 x 20 mm では名前が重なっていた)。
    #[test]
    fn group_names_on_both_sides_keep_the_text_gap() {
        let a = attrs(&[("block_pins", BOARD_44), ("block_size", r#"{"w":20,"h":20}"#)]);
        let def = block_symbol(&a).expect("ブロック定義");
        let left = text_box(&def, "X44B XH");
        let right = text_box(&def, "X44A XH");
        assert!(
            right.0.x - left.1.x >= CONNECTOR_LABEL_GAP_MM - 1e-9,
            "左の名前 {left:?} と右の名前 {right:?} の間が {CONNECTOR_LABEL_GAP_MM}mm に足りない"
        );
        let (w, _) = block_box(&parse_block_spec(&a).unwrap());
        assert!(w > 20.0, "20mm のままでは収まらないので広がる: {w}");
    }

    /// A box whose group names are on one side only, or whose names already fit, keeps the size it was given.
    /// まとまりの名前が片側だけの箱、または名前がもう収まっている箱は、指定された大きさのまま。
    #[test]
    fn one_sided_group_names_keep_the_given_size() {
        let size = |json: &str, wh: &str| block_box(&parse_block_spec(&attrs(&[("block_pins", json), ("block_size", wh)])).unwrap());
        let one_side = r#"{"left":[{"no":"1","name":"VBUS","connector":"jst_xh_f","connector_ref":"X44B"},{"no":"2","name":"D-","connector":"jst_xh_f","connector_ref":"X44B"}],"right":[{"no":"5"},{"no":"6"}]}"#;
        assert_eq!(size(one_side, r#"{"w":20,"h":20}"#), (20.0, 20.0));
        assert_eq!(size(BOARD_44, r#"{"w":50,"h":20}"#), (50.0, 20.0));
    }

    /// 描かれた文字 (文字列, 基準点, 高さ)。
    fn text_primitives(def: &SymbolDef) -> Vec<(String, Point, f64)> {
        def.primitives
            .iter()
            .filter_map(|p| match p {
                Primitive::Text { at, text, height } => Some((text.clone(), *at, *height)),
                _ => None,
            })
            .collect()
    }

    /// An instance of the block at the origin, `rotation` degrees, optionally mirrored.
    fn placed(json: &str, rotation: u16, mirror: bool) -> (crate::model::SymbolInstance, SymbolDef) {
        let crate::model::Entity::Symbol(mut inst) = test_block("39", "USBハブ", 0.0, 0.0, json) else { unreachable!() };
        inst.rotation = rotation;
        inst.mirror = mirror;
        let def = block_symbol(&inst.attrs).expect("ブロック定義");
        (inst, def)
    }

    /// The reference and value captions sit above, and the function label below, everything the block draws: lead ends, pin numbers (whole text boxes, also when the block is rotated), connector pictures and group names.
    /// 参照記号・型番は、ブロックが描くもの (リードの先・ピン番号の文字の箱全体 (回転しても)・コネクタの絵・まとまりの名前) の上、機能ラベルは下に置く。
    #[test]
    fn captions_clear_everything_the_pins_draw() {
        let long_numbers = r#"{"left":[{"no":"4444","connector":"usb_c_f"},{"no":"333","connector":"usb_a_f"}],"right":[{"no":"2222","connector":"usb_a_f"}],"top":[{"no":"U1","connector":"usb_a_f"},{"no":"U2","connector":"usb_a_f"}],"bottom":[{"no":"PWR","connector":"micro_b_f"}]}"#;
        for json in [HUB_39, TOP_BOTTOM, MIXED, long_numbers] {
            for rotation in [0, 90, 180, 270] {
                for mirror in [false, true] {
                    let (mut inst, def) = placed(json, rotation, mirror);
                    inst.attrs.insert(crate::text_style::FUNCTION_LABEL_ATTR.into(), "機能".into());
                    // 描かれたものの上下端: 外形・ピンの先に、文字の箱 (文字は回転しても横書きなので高さだけ) を足す
                    let (mut lo, mut hi) = crate::svg::symbol_bounds(&inst, &def);
                    for (_, at, height) in text_primitives(&def) {
                        let c = crate::netlist::transform_local(at, &inst);
                        lo.y = lo.y.min(c.y - height / 2.0);
                        hi.y = hi.y.max(c.y + height / 2.0);
                    }
                    let drawn = crate::tidy::Rect { min: lo, max: hi };
                    let top = crate::svg::symbol_top_y(&inst, &def);
                    assert!(top <= drawn.min.y + 1e-9, "回転{rotation}・ミラー{mirror}: 型番の基準 {top} が描かれた上端 {} より下", drawn.min.y);
                    let below = crate::text_style::function_label_placement(&inst, &def).expect("機能ラベル");
                    assert!(
                        below.at.y - below.height >= drawn.max.y - 1e-9,
                        "回転{rotation}・ミラー{mirror}: 機能ラベルの上端 {} が描かれた下端 {} より上",
                        below.at.y - below.height,
                        drawn.max.y
                    );
                }
            }
        }
    }

    /// The captions of an auto-sized block that has no top pins stay exactly where they were (the top of the box plus the usual gaps).
    /// 外形が自動で上辺にピンの無いブロックの参照記号・型番は今までと同じ位置 (外形の上端+いつもの間隔) のまま。
    #[test]
    fn captions_of_an_auto_sized_block_without_top_pins_do_not_move() {
        let (inst, def) = placed(AMP_53, 0, false);
        let (min, _) = crate::svg::symbol_bounds(&inst, &def);
        assert!((crate::svg::symbol_top_y(&inst, &def) - min.y).abs() < 1e-9);
    }

    /// A block sized to its minimum height has the number of its first side pin poking 0.5 mm above the box top, so its captions move up 0.5 mm to clear that number even without top pins.
    /// 最小の高さを指定したブロックは、最初の左右のピン番号が外形の上端から 0.5 mm はみ出すので、上辺にピンが無くても参照記号・型番はその番号を避けて 0.5 mm 上がる。
    #[test]
    fn captions_of_a_minimum_height_block_clear_the_side_pin_numbers() {
        let json = r#"{"left":[{"no":"1"},{"no":"2"}]}"#;
        let (mut inst, _) = placed(json, 0, false);
        inst.attrs.insert(BLOCK_SIZE_ATTR.into(), r#"{"w":10,"h":10}"#.into());
        let def = block_symbol(&inst.attrs).expect("ブロック定義");
        let (min, _) = crate::svg::symbol_bounds(&inst, &def);
        let top = crate::svg::symbol_top_y(&inst, &def);
        assert!((top - (min.y - 0.5)).abs() < 1e-9, "外形の上端 {} から 0.5 mm 上のはず: {top}", min.y);
        for (text, at, height) in text_primitives(&def) {
            assert!(top <= at.y - height / 2.0 + 1e-9, "番号 {text} の上端 {} が型番の基準 {top} より上", at.y - height / 2.0);
        }
    }

    /// The drawn rectangle of the block box in paper coordinates, from its pin table and size.
    /// ブロックの外形を用紙座標の矩形で返す (ピン表と寸法から)。
    fn paper_box(inst: &crate::model::SymbolInstance) -> crate::tidy::Rect {
        let spec = parse_block_spec(&inst.attrs).expect("ピン表");
        let (lo, hi) = block_outline(&spec);
        let a = crate::netlist::transform_local(lo, inst);
        let b = crate::netlist::transform_local(hi, inst);
        crate::tidy::Rect::from_corners(a, b)
    }

    /// Everything the block draws except the pin leads' own end points, as paper rectangles: primitive extents and whole text boxes.
    /// ブロックが描くもの (図形の外接矩形と文字の箱全体) を用紙座標の矩形で返す。
    fn drawn_rects(inst: &crate::model::SymbolInstance, def: &SymbolDef) -> Vec<crate::tidy::Rect> {
        let mut out = Vec::new();
        for prim in &def.primitives {
            if let Primitive::Text { at, text, height } = prim {
                let c = crate::netlist::transform_local(*at, inst);
                let half = crate::tidy::text_width(text, *height) / 2.0;
                out.push(crate::tidy::Rect::from_corners(
                    Point::new(c.x - half, c.y - height / 2.0),
                    Point::new(c.x + half, c.y + height / 2.0),
                ));
            } else {
                let (lo, hi) = crate::connector::primitive_extent(prim);
                out.push(crate::tidy::Rect::from_corners(
                    crate::netlist::transform_local(lo, inst),
                    crate::netlist::transform_local(hi, inst),
                ));
            }
        }
        out
    }

    /// Which side of the pin table ends up drawn on top, written out by hand for the mirror-then-clockwise-rotation convention (Y down).
    /// ピン表のどの辺が描かれた上辺になるか (ミラー→時計回りの回転、Y下向き) を手で書いた表。
    fn side_drawn_on_top(rotation: u16, mirror: bool) -> &'static str {
        match (rotation, mirror) {
            (0, _) => "top",
            (90, false) => "left",
            (90, true) => "right",
            (180, _) => "bottom",
            (270, false) => "right",
            (270, true) => "left",
            _ => unreachable!("回転 {rotation}"),
        }
    }

    /// Whether the pin table JSON has at least one pin on the given side.
    /// ピン表の JSON がその辺にピンを1本以上持つか。
    fn json_has_side(json: &str, side: &str) -> bool {
        let v: serde_json::Value = serde_json::from_str(json).expect("ピン表");
        v.get(side).and_then(|s| s.as_array()).is_some_and(|a| !a.is_empty())
    }

    /// The corner placement follows an explicit table for a block with one top pin and one left pin: the corner is used exactly when the top or the left side is drawn on top.
    /// 上辺と左辺に1本ずつピンのあるブロックは、手で書いた表のとおりに角へ置く (描かれた上辺が上辺か左辺のときだけ角)。
    #[test]
    fn corner_placement_follows_the_rotation_table() {
        let json = r#"{"top":[{"no":"1"}],"left":[{"no":"2"}]}"#;
        for (rotation, mirror, corner) in [
            (0, false, true),
            (0, true, true),
            (90, false, true),
            (270, true, true),
            (90, true, false),
            (180, false, false),
            (180, true, false),
            (270, false, false),
        ] {
            let (inst, def) = placed(json, rotation, mirror);
            assert_eq!(crate::svg::caption_anchor(&inst, &def).end, corner, "回転{rotation}・ミラー{mirror}");
        }
    }

    /// A hand-built drawing above-left of the box corner pushes the corner captions up until their text boxes end above it.
    /// 外形の左上の角の外に描かれたもの (手で足した文字) があると、角の参照記号・型番はその上まで上がる。
    #[test]
    fn corner_captions_rise_above_a_drawing_at_the_corner() {
        let (inst, mut def) = placed(r#"{"top":[{"no":"1"}],"left":[{"no":"2"}]}"#, 0, false);
        let body = paper_box(&inst);
        let before = crate::svg::caption_anchor(&inst, &def);
        // 何も無いときの型番の行の高さに、角の左へ文字を置く
        let at = Point::new(body.min.x - 4.0, before.top - crate::svg::VALUE_LABEL_DY - 1.0);
        def.primitives.push(Primitive::Text { at, text: "OBSTACLE".into(), height: 2.5 });
        let half = crate::tidy::text_width("OBSTACLE", 2.5) / 2.0;
        let obstacle = crate::tidy::Rect::from_corners(
            Point::new(at.x - half, at.y - 1.25),
            Point::new(at.x + half, at.y + 1.25),
        );
        let a = crate::svg::caption_anchor(&inst, &def);
        assert!(a.end);
        let value_bottom = a.top - crate::svg::VALUE_LABEL_DY;
        assert!(value_bottom <= obstacle.min.y + 1e-9, "型番の下端 {value_bottom} が文字の上端 {} より下", obstacle.min.y);
        let font = crate::svg::LABEL_FONT;
        for (text, dy) in [(inst.reference.as_str(), crate::svg::REF_LABEL_DY), (inst.value.as_str(), crate::svg::VALUE_LABEL_DY)] {
            let w = crate::tidy::text_width(text, font);
            let base = a.top - dy;
            let rect = crate::tidy::Rect::from_corners(Point::new(a.x - w, base - font), Point::new(a.x, base));
            for d in drawn_rects(&inst, &def) {
                assert!(!rect.overlaps(&d), "キャプション {text} が描かれたもの {d:?} に重なる");
            }
        }
    }

    /// A block with pins on its drawn top side puts its reference and value right-aligned at the box's top-left corner (1.0 mm left of the left edge, text boxes ending at least 1.0 mm above the top edge) without overlapping anything the block draws; other blocks keep the centred caption.
    /// 描かれた上辺にピンのあるブロックは、参照記号・型番を外形の左上の角 (左辺の 1.0 mm 左・文字の箱の下端が上辺の 1.0 mm 以上上) に右揃えで置き、ブロックが描くものと重ならない。それ以外のブロックは今までの中央揃え。
    #[test]
    fn captions_of_a_block_with_top_pins_sit_at_the_top_left_corner() {
        let long_top = r#"{"left":[{"no":"4444","connector":"usb_c_f"},{"no":"333","connector":"usb_a_f"}],"top":[{"no":"UUUU","name":"長い名前","connector":"usb_a_f"},{"no":"U2","connector":"usb_a_f"}],"right":[{"no":"2222","connector":"usb_a_f"}]}"#;
        let (mut with_top, mut without_top) = (0, 0);
        for json in [HUB_39, TOP_BOTTOM, MIXED, AMP_53, long_top] {
            for rotation in [0, 90, 180, 270] {
                for mirror in [false, true] {
                    let (inst, def) = placed(json, rotation, mirror);
                    let a = crate::svg::caption_anchor(&inst, &def);
                    if !json_has_side(json, side_drawn_on_top(rotation, mirror)) {
                        without_top += 1;
                        let plain = crate::svg::CaptionAnchor {
                            x: inst.at.x,
                            top: crate::svg::symbol_top_y(&inst, &def),
                            end: false,
                        };
                        assert_eq!(a, plain, "上辺にピンの無い回転{rotation}・ミラー{mirror}は今まで通り");
                        continue;
                    }
                    with_top += 1;
                    let b = paper_box(&inst);
                    assert!(a.end, "回転{rotation}・ミラー{mirror}: 右揃えのはず");
                    assert!((a.x - (b.min.x - 1.0)).abs() < 1e-9, "回転{rotation}・ミラー{mirror}: 左辺の 1.0 mm 左のはず: {}", a.x);
                    let value_bottom = a.top - crate::svg::VALUE_LABEL_DY;
                    assert!(value_bottom <= b.min.y - 1.0 + 1e-9, "回転{rotation}・ミラー{mirror}: 型番の下端 {value_bottom} が上辺 {} の 1.0 mm 上より下", b.min.y);
                    let font = crate::svg::LABEL_FONT;
                    let drawn = drawn_rects(&inst, &def);
                    for (text, dy) in [("39", crate::svg::REF_LABEL_DY), ("USBハブ", crate::svg::VALUE_LABEL_DY)] {
                        let w = crate::tidy::text_width(text, font);
                        let base = a.top - dy;
                        let rect = crate::tidy::Rect::from_corners(Point::new(a.x - w, base - font), Point::new(a.x, base));
                        for d in &drawn {
                            assert!(!rect.overlaps(d), "回転{rotation}・ミラー{mirror}: キャプション {text} が描かれたもの {d:?} に重なる");
                        }
                    }
                }
            }
        }
        assert!(with_top > 0 && without_top > 0, "両方の場合を確かめる: {with_top} / {without_top}");
    }

    /// On a 2.5 mm block with a top pin and an odd-multiple width and height, the corner caption sits 1.0 mm left of the drawn left edge and starts from the drawn top edge, where the outline was moved onto the grid.
    /// 幅も高さも2.5mmの奇数倍で上辺にピンのある2.5mm間隔のブロックでは、左上の角の参照記号・型番は、グリッドへずらして描いた外形の左辺の1.0mm左・上辺の位置から始まる。
    #[test]
    fn corner_captions_of_a_fine_block_follow_the_shifted_outline() {
        let (inst, def) = placed_fine(r#"{"left":[{"no":"1"}],"top":[{"no":"T1"}]}"#, r#"{"w":17.5,"h":12.5}"#);
        assert_eq!(outline_of(&def), (-7.5, -5.0, 10.0, 7.5));
        let (lo, hi) = block_outline(&parse_block_spec(&inst.attrs).unwrap());
        assert_eq!((lo.x, lo.y, hi.x, hi.y), outline_of(&def), "block_outline は描かれた外形と同じ");
        let a = crate::svg::caption_anchor(&inst, &def);
        assert!(a.end);
        assert!((a.x - (-7.5 - 1.0)).abs() < 1e-9, "{a:?}");
        assert!((a.top - (-5.0)).abs() < 1e-9, "{a:?}");
    }

    /// A 2.5 mm block without top pins whose width and height are odd multiples of 2.5 mm has its captions centred on the drawn outline, after rotation and mirroring, not on the placement point; a 5 mm block keeps them exactly on the placement point.
    /// 上辺にピンが無く幅も高さも2.5mmの奇数倍の2.5mm間隔のブロックでは、参照記号・型番は配置点ではなく、回転・ミラーの後の描かれた外形の中央にそろう。5mm間隔のブロックは配置点ちょうどのまま。
    #[test]
    fn centred_captions_of_a_fine_block_follow_the_shifted_outline() {
        // 外形はローカルで (-2.5,-2.5)..(5,5)、中心 (1.25, 1.25)。配置点 (100, 50)
        let cases = [
            (0, false, 101.25),
            (90, false, 98.75),
            (180, false, 98.75),
            (270, false, 101.25),
            (0, true, 98.75),
            (90, true, 98.75),
            (180, true, 101.25),
            (270, true, 101.25),
        ];
        for (rotation, mirror, x) in cases {
            let (mut inst, def) = placed_fine("{}", r#"{"w":7.5,"h":7.5}"#);
            inst.at = Point::new(100.0, 50.0);
            inst.rotation = rotation;
            inst.mirror = mirror;
            let a = crate::svg::caption_anchor(&inst, &def);
            assert!(!a.end, "回転{rotation}・ミラー{mirror}");
            assert!((a.x - x).abs() < 1e-9, "回転{rotation}・ミラー{mirror}: {a:?}");
            assert_eq!(a.top, crate::svg::symbol_top_y(&inst, &def));
            let (mut coarse, def) = placed("{}", rotation, mirror);
            coarse.at = Point::new(100.3, 50.0);
            assert_eq!(crate::svg::caption_anchor(&coarse, &def).x, 100.3, "5mm間隔は配置点のまま");
        }
    }

    /// The import warning for a block that cannot be read speaks of the block's attributes in general, because the unreadable value can be the pitch or the number position as well as the pin table, and says a sample box was used.
    /// 読めないブロックの取り込みの警告は、読めない値がピン表だけでなく間隔や番号の位置のこともあるので、ブロックの属性全体のこととして書き、見本の箱にしたことを伝える。
    #[test]
    fn the_unreadable_block_warning_names_the_attributes() {
        assert!(BLOCK_UNREADABLE_WARNING.contains("属性"), "{BLOCK_UNREADABLE_WARNING}");
        assert!(!BLOCK_UNREADABLE_WARNING.contains("ピン表"), "{BLOCK_UNREADABLE_WARNING}");
        assert!(BLOCK_UNREADABLE_WARNING.contains("見本の箱"), "{BLOCK_UNREADABLE_WARNING}");
        assert!(imported_block_attrs(Some(r#"{"left":[{"no":"1"}]}"#), None, Some("3"), None, None, None).is_none(), "間隔が読めないときも警告の対象");
    }

    fn placed_fine(pins: &str, size: &str) -> (crate::model::SymbolInstance, SymbolDef) {
        let crate::model::Entity::Symbol(mut inst) = test_block("39", "USBハブ", 0.0, 0.0, pins) else { unreachable!() };
        inst.attrs = fine(pins, Some(size));
        let def = block_symbol(&inst.attrs).expect("ブロック定義");
        (inst, def)
    }

    /// Only blocks move their captions: a plain symbol keeps the centred caption above its top edge.
    /// 参照記号・型番が動くのはブロックだけ。普通のシンボルは今まで通り外形の上に中央揃え。
    #[test]
    fn captions_of_a_plain_symbol_keep_the_centred_anchor() {
        let (inst, _) = placed(HUB_39, 0, false);
        let def = crate::symbol::resolve_symbol("relay_coil").expect("コイル");
        let mut plain = inst.clone();
        plain.symbol_id = "relay_coil".into();
        plain.attrs.clear();
        let a = crate::svg::caption_anchor(&plain, &def);
        assert_eq!(a.x, plain.at.x);
        assert_eq!(a.top, crate::svg::symbol_top_y(&plain, &def));
        assert!(!a.end);
    }

    /// KiCad's Reference/Value, the device XRef and the tidy text boxes all follow the corner placement of a block with top pins: right-justified, left of the box.
    /// 上辺にピンのあるブロックでは、KiCad の Reference/Value・デバイスXRef・整理用の文字の箱も左上の角 (外形の左・右揃え) に従う。
    #[test]
    fn kicad_xref_and_tidy_follow_the_top_left_corner_captions() {
        let mut sheet = crate::model::Sheet::new("t", crate::model::PaperSize::A3, crate::model::Orientation::Landscape);
        // 外形より広い参照記号: 中央揃えなら右端が外形の右端を越えるので、XRef のXで新旧が区別できる
        let block = test_block("M1-CONVEYOR-DRIVE", "USBハブ", 100.0, 100.0, r#"{"top":[{"no":"1"}],"left":[{"no":"2"}]}"#);
        let crate::model::Entity::Symbol(inst) = block.clone() else { unreachable!() };
        sheet.entities.insert(block.id(), block);
        let defs = crate::symbol::sheet_symbol_defs(&sheet);
        let def = block_symbol(&inst.attrs).expect("ブロック定義");
        let a = crate::svg::caption_anchor(&inst, &def);
        assert!(a.end);
        // KiCad
        let (sch, _) = crate::kicad::export_kicad_sch(&sheet, &defs);
        let reference = sch.lines().find(|l| l.contains("(property \"Reference\" \"M1-CONVEYOR-DRIVE\"")).expect("Reference");
        assert!(reference.contains("(justify right)"), "{reference}");
        let at_x: f64 = reference.split("(at ").nth(1).and_then(|r| r.split_whitespace().next()).and_then(|v| v.parse().ok()).expect("at のX");
        assert!((at_x - a.x).abs() < 1e-3, "{reference}");
        // XRef は右揃えの参照記号の右ではなく、外形の右端から出る
        let (_, max) = crate::svg::symbol_bounds(&inst, &def);
        let centred_right = inst.at.x + crate::tidy::text_width(&inst.reference, crate::svg::LABEL_FONT) / 2.0;
        assert!(centred_right > max.x + 1.0, "参照記号が外形より十分広いこと: {centred_right} / {}", max.x);
        let x = crate::xref::device_xref_at(&inst, &def);
        assert!((x.x - (max.x + crate::xref::XREF_GAP)).abs() < 1e-9, "{x:?}");
        assert!((x.y - (a.top - crate::svg::REF_LABEL_DY)).abs() < 1e-9);
        // 整理用の文字の箱: 参照記号の箱の右端が基準X
        let boxes = crate::tidy::text_boxes(&sheet, &defs);
        let r = boxes.iter().find(|b| b.kind == crate::tidy::TextKind::Reference).expect("参照記号の箱");
        assert!((r.rect.max.x - a.x).abs() < 1e-9, "{:?}", r.rect);
    }

    fn fine(pins: &str, size: Option<&str>) -> BTreeMap<String, String> {
        let mut a = BTreeMap::new();
        a.insert(BLOCK_PINS_ATTR.to_string(), pins.to_string());
        a.insert(BLOCK_PITCH_ATTR.to_string(), "2.5".to_string());
        if let Some(s) = size {
            a.insert(BLOCK_SIZE_ATTR.to_string(), s.to_string());
        }
        a
    }

    /// A block without the pitch and number attributes is a 5 mm block with numbers outside, as before.
    /// 間隔と番号の属性が無いブロックは、これまでどおり5mm間隔・番号は外側のブロックになる。
    #[test]
    fn a_block_without_the_new_attributes_is_coarse_with_numbers_outside() {
        let spec = parse_block_spec(&BTreeMap::new()).unwrap();
        assert_eq!((spec.pitch, spec.numbers), (BlockPitch::Coarse, BlockNumbers::Outside));
    }

    /// block_pitch accepts only "5" and "2.5", and block_numbers only "outside" and "inside"; anything else is refused with the attribute name in the message.
    /// block_pitch は "5" と "2.5" だけ、block_numbers は "outside" と "inside" だけを受け付ける。それ以外は属性名を含むメッセージで断る。
    #[test]
    fn unknown_pitch_and_number_values_are_refused() {
        for (key, bad) in [(BLOCK_PITCH_ATTR, "3"), (BLOCK_PITCH_ATTR, "fine"), (BLOCK_NUMBERS_ATTR, "in"), (BLOCK_NUMBERS_ATTR, "")] {
            let mut a = BTreeMap::new();
            a.insert(key.to_string(), bad.to_string());
            let err = parse_block_spec(&a).unwrap_err();
            assert!(err.contains(key), "{key}={bad}: {err}");
        }
        let mut a = BTreeMap::new();
        a.insert(BLOCK_PITCH_ATTR.to_string(), " 2.5 ".to_string());
        a.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
        let spec = parse_block_spec(&a).unwrap();
        assert_eq!((spec.pitch, spec.numbers), (BlockPitch::Fine, BlockNumbers::Inside));
    }

    /// On a 2.5 mm block a side with n pins needs (n + 1) x 2.5 mm, at least 5 mm; a smaller block_size is refused and the message names the needed size.
    /// 2.5mm間隔のブロックでは、ピンがn本の辺は (n + 1) × 2.5mm (最小5mm) 必要。それより小さい block_size は断り、必要な大きさをメッセージに出す。
    #[test]
    fn a_fine_block_size_must_hold_its_pins() {
        let two = r#"{"left":[{"no":"1"},{"no":"2"}]}"#;
        assert!(parse_block_spec(&fine(two, Some(r#"{"w":5,"h":7.5}"#))).is_ok());
        let err = parse_block_spec(&fine(two, Some(r#"{"w":5,"h":5}"#))).unwrap_err();
        assert!(err.contains("5x7.5"), "{err}");
        let top3 = r#"{"top":[{"no":"1"},{"no":"2"},{"no":"3"}]}"#;
        let err = parse_block_spec(&fine(top3, Some(r#"{"w":7.5,"h":5}"#))).unwrap_err();
        assert!(err.contains("10x5"), "{err}");
    }

    /// A 2.5 mm block_size that is not a multiple of 2.5 mm is first rounded up and then checked: 5 x 6 mm with three left pins becomes 5 x 7.5 mm, which is refused because it needs 5 x 10 mm.
    /// 2.5mmの倍数でない2.5mm間隔の block_size は、先に切り上げてから確かめる: 左に3本で5×6mmは5×7.5mmになり、5×10mm要るので断る。
    #[test]
    fn a_fine_block_size_is_rounded_before_it_is_checked() {
        let err = parse_block_spec(&fine(&left_pins(3), Some(r#"{"w":5,"h":6}"#))).unwrap_err();
        assert!(err.contains("5x7.5mm") && err.contains("5x10"), "{err}");
    }

    /// After a block with numbers inside is widened, the numbers of the left and right side keep at least 1.0 mm between them, for numbers of every length on both pitches.
    /// 番号を内側に描くブロックを広げたあと、左右の番号の間は、どの長さの番号でも、どちらの間隔でも1.0mm以上空く。
    #[test]
    fn widened_inside_numbers_keep_a_gap_between_left_and_right() {
        for pitch in ["5", "2.5"] {
            for left in ["1", "12", "123", "1234", "12345678"] {
                for right in ["9", "98", "9876", "98765"] {
                    let json = format!(r#"{{"left":[{{"no":"{left}"}}],"right":[{{"no":"{right}"}}]}}"#);
                    let a = attrs(&[(BLOCK_PINS_ATTR, &json), (BLOCK_PITCH_ATTR, pitch), (BLOCK_NUMBERS_ATTR, "inside"), (BLOCK_SIZE_ATTR, r#"{"w":5,"h":5}"#)]);
                    let def = block_symbol(&a).unwrap();
                    let (l, r) = (text_at(&def, left), text_at(&def, right));
                    let gap = (r.x - name_width(right) / 2.0) - (l.x + name_width(left) / 2.0);
                    assert!(gap >= 1.0 - 1e-9, "{pitch} {left}/{right}: {gap}");
                }
            }
        }
    }

    /// On a 2.5 mm block block_size is rounded up to the next 2.5 mm; on a 5 mm block it is still rounded up to 5 mm.
    /// 2.5mm間隔のブロックの block_size は次の2.5mmへ切り上げる。5mm間隔のブロックはこれまでどおり5mmへ切り上げる。
    #[test]
    fn block_size_is_rounded_up_to_the_step_of_the_pitch() {
        let spec = parse_block_spec(&fine(r#"{"left":[{"no":"1"}]}"#, Some(r#"{"w":6,"h":8}"#))).unwrap();
        assert_eq!(spec.size, Some(BlockSize { w: 7.5, h: 10.0 }));
        let mut a = BTreeMap::new();
        a.insert(BLOCK_SIZE_ATTR.to_string(), r#"{"w":16,"h":11}"#.to_string());
        assert_eq!(parse_block_spec(&a).unwrap().size, Some(BlockSize { w: 20.0, h: 15.0 }));
    }

    /// A 2.5 mm block refuses a pin with a connector picture, naming the pin.
    /// 2.5mm間隔のブロックは、コネクタの絵を持つピンを断り、そのピン番号を示す。
    #[test]
    fn a_fine_block_refuses_connector_pictures() {
        let err = parse_block_spec(&fine(r#"{"left":[{"no":"U1","connector":"usb_a_f"}]}"#, None)).unwrap_err();
        assert!(err.contains("U1") && err.contains(BLOCK_PITCH_ATTR), "{err}");
    }

    /// A block with numbers inside refuses a pin with a connector picture, on a 5 mm block too, naming the pin.
    /// 番号を内側に描くブロックは、5mm間隔でも、コネクタの絵を持つピンを断り、そのピン番号を示す。
    #[test]
    fn a_block_with_numbers_inside_refuses_connector_pictures() {
        let pins = r#"{"left":[{"no":"1"},{"no":"J2","connector":"jst_xh_f"}]}"#;
        let mut a = attrs(&[(BLOCK_PINS_ATTR, pins), (BLOCK_NUMBERS_ATTR, "inside")]);
        let err = parse_block_spec(&a).unwrap_err();
        assert_eq!(err, "block pin J2: a connector picture cannot be used with block_numbers inside");
        a.insert(BLOCK_NUMBERS_ATTR.to_string(), "outside".to_string());
        assert!(parse_block_spec(&a).is_ok(), "番号が外側なら使える");
    }

    /// Without block_size a 2.5 mm block with pin numbers inside is as small as its pins allow: one pin gives 5 x 5 mm, two pins on the left give 5 x 7.5 mm.
    /// block_size が無い2.5mm間隔・番号内側のブロックは、ピンが許す最小の大きさになる: 1本なら5×5mm、左に2本なら5×7.5mm。
    #[test]
    fn a_fine_block_sizes_itself_to_its_pins() {
        let mut a = fine(r#"{"left":[{"no":"1"}]}"#, None);
        a.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
        assert_eq!(block_box(&parse_block_spec(&a).unwrap()), (5.0, 5.0));
        let mut a = fine(r#"{"left":[{"no":"1"},{"no":"2"}]}"#, None);
        a.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
        assert_eq!(block_box(&parse_block_spec(&a).unwrap()), (5.0, 7.5));
    }

    fn on_grid(v: f64) -> bool {
        let k = v / 2.5;
        (k - k.round()).abs() < 1e-9
    }

    fn outline_of(def: &SymbolDef) -> (f64, f64, f64, f64) {
        match &def.primitives[0] {
            Primitive::Rect { p1, p2, .. } => (p1.x, p1.y, p2.x, p2.y),
            other => panic!("先頭が外形でない: {other:?}"),
        }
    }

    fn left_pins(n: usize) -> String {
        let pins: Vec<String> = (1..=n).map(|i| format!(r#"{{"no":"{i}"}}"#)).collect();
        format!(r#"{{"left":[{}]}}"#, pins.join(","))
    }

    /// The outline and the pins of a 2.5 mm block follow the table of the design: a height that is an odd multiple of 2.5 mm moves the outline down by 1.25 mm, and a row of pins that would miss the grid moves down by 1.25 mm.
    /// 2.5mm間隔のブロックの外形とピンは設計の表のとおりになる: 高さが2.5mmの奇数倍なら外形が1.25mm下へ寄り、グリッドから外れるピンの列は1.25mm下へ寄る。
    #[test]
    fn a_fine_block_shifts_its_outline_and_rows_onto_the_grid() {
        // (高さ, ピン数, 外形の上, 外形の下, ピンのy)
        let table: [(f64, usize, f64, f64, &[f64]); 6] = [
            (5.0, 1, -2.5, 2.5, &[0.0]),
            (7.5, 2, -2.5, 5.0, &[0.0, 2.5]),
            (7.5, 1, -2.5, 5.0, &[2.5]),
            (10.0, 3, -5.0, 5.0, &[-2.5, 0.0, 2.5]),
            (10.0, 2, -5.0, 5.0, &[0.0, 2.5]),
            (12.5, 4, -5.0, 7.5, &[-2.5, 0.0, 2.5, 5.0]),
        ];
        for (h, n, top, bottom, ys) in table {
            let size = format!(r#"{{"w":5,"h":{h}}}"#);
            let def = block_symbol(&fine(&left_pins(n), Some(&size))).unwrap();
            let (x1, y1, x2, y2) = outline_of(&def);
            assert_eq!((x1, y1, x2, y2), (-2.5, top, 2.5, bottom), "h={h} n={n}");
            let got: Vec<f64> = def.pins.iter().map(|p| p.at.y).collect();
            assert_eq!(got, ys, "h={h} n={n}");
            assert!(def.pins.iter().all(|p| p.at.x == -5.0), "接続点は外形から2.5mm: h={h} n={n}");
        }
    }

    /// Every pin and every outline edge of a 2.5 mm block is on the 2.5 mm grid, for every size from 5 to 20 mm and every pin count that fits, on all four sides, also when the two opposite sides have different pin counts, and every pin is at least 2.5 mm from both ends of its side.
    /// 2.5mm間隔のブロックのピンと外形の辺は、5〜20mmのどの大きさ・収まるどのピン数・四辺のどれでも、向かい合う辺のピン数が違っても、2.5mmグリッドに乗り、どのピンも辺の両端から2.5mm以上離れる。
    #[test]
    fn every_fine_block_is_on_the_grid() {
        let sizes: Vec<f64> = (2..=8).map(|k| k as f64 * 2.5).collect();
        let side = |name: &str, n: usize, tag: &str| {
            let pins: Vec<String> = (1..=n).map(|i| format!(r#"{{"no":"{tag}{i}"}}"#)).collect();
            format!(r#""{name}":[{}]"#, pins.join(","))
        };
        let mut checked = 0;
        for &w in &sizes {
            for &h in &sizes {
                let max_rows = (h / 2.5) as usize - 1;
                let max_cols = (w / 2.5) as usize - 1;
                for (l, r) in [(0, max_rows), (max_rows, 0), (max_rows, max_rows.saturating_sub(1)), (1, max_rows)] {
                    for (t, b) in [(0, max_cols), (max_cols, max_cols.saturating_sub(1))] {
                        let json = format!("{{{},{},{},{}}}", side("left", l, "L"), side("right", r, "R"), side("top", t, "T"), side("bottom", b, "B"));
                        let size = format!(r#"{{"w":{w},"h":{h}}}"#);
                        let mut a = fine(&json, Some(&size));
                        a.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
                        let def = block_symbol(&a).unwrap_or_else(|e| panic!("{w}x{h} {json}: {e}"));
                        let (x1, y1, x2, y2) = outline_of(&def);
                        for v in [x1, y1, x2, y2] {
                            assert!(on_grid(v), "外形 {v}: {w}x{h} {json}");
                        }
                        for p in &def.pins {
                            assert!(on_grid(p.at.x) && on_grid(p.at.y), "ピン {:?}: {w}x{h} {json}", p.at);
                            assert!(p.at.x < x1 || p.at.x > x2 || p.at.y < y1 || p.at.y > y2, "ピンが外形の中: {:?}", p.at);
                            // 辺に沿って測り、辺の両端から2.5mm以上離れる
                            let (along, lo, hi) = if p.at.x < x1 || p.at.x > x2 { (p.at.y, y1, y2) } else { (p.at.x, x1, x2) };
                            assert!(along - lo >= 2.5 - 1e-9 && hi - along >= 2.5 - 1e-9, "ピン {:?} が辺の端に近い: {w}x{h} {json}", p.at);
                        }
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 300, "{checked}");
    }

    /// With numbers inside, the number of a left or right pin is drawn inside the outline next to its edge at the height of the pin, and pin names are not drawn.
    /// 番号を内側に描くブロックでは、左右のピンの番号は外形の内側の辺のそばに、ピンと同じ高さで描かれ、ピン名は描かれない。
    #[test]
    fn numbers_inside_are_drawn_inside_and_names_are_not_drawn() {
        let mut a = fine(r#"{"left":[{"no":"1","name":"VCC"}],"right":[{"no":"2","name":"GND"}]}"#, Some(r#"{"w":10,"h":5}"#));
        a.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
        let def = block_symbol(&a).unwrap();
        let texts: Vec<(&str, f64, f64)> = def.primitives.iter().filter_map(|p| match p {
            Primitive::Text { at, text, .. } => Some((text.as_str(), at.x, at.y)),
            _ => None,
        }).collect();
        assert_eq!(texts.iter().map(|t| t.0).collect::<Vec<_>>(), ["1", "2"]);
        let (x1, _, x2, _) = outline_of(&def);
        assert!(texts[0].1 > x1 && texts[0].1 < 0.0 && texts[0].2 == 0.0, "{texts:?}");
        assert!(texts[1].1 < x2 && texts[1].1 > 0.0 && texts[1].2 == 0.0, "{texts:?}");
        assert_eq!(def.pins[0].name, "VCC", "名前はデータに残る");
    }

    /// A 5 mm wide block with long numbers inside on both sides is widened until the numbers fit, and its pins stay on the grid.
    /// 幅5mmのブロックの両側に長い番号を内側へ描くと、番号が収まる幅まで広がり、ピンはグリッドに乗ったままになる。
    #[test]
    fn a_narrow_block_with_numbers_inside_is_widened() {
        let mut a = fine(r#"{"left":[{"no":"1234"}],"right":[{"no":"5678"}]}"#, Some(r#"{"w":5,"h":5}"#));
        a.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
        let def = block_symbol(&a).unwrap();
        let (x1, _, x2, _) = outline_of(&def);
        assert!(x2 - x1 > 5.0);
        assert!(on_grid(x1) && on_grid(x2) && def.pins.iter().all(|p| on_grid(p.at.x)));
    }

    /// Numbers inside also work on a 5 mm block, whose pins stay where they were.
    /// 番号の内側表示は5mm間隔のブロックでも使え、ピンの位置は変わらない。
    #[test]
    fn numbers_inside_work_on_a_coarse_block_without_moving_pins() {
        let pins = r#"{"left":[{"no":"1"},{"no":"2"}]}"#;
        let mut plain = BTreeMap::new();
        plain.insert(BLOCK_PINS_ATTR.to_string(), pins.to_string());
        plain.insert(BLOCK_SIZE_ATTR.to_string(), r#"{"w":15,"h":10}"#.to_string());
        let mut inside = plain.clone();
        inside.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
        let (a, b) = (block_symbol(&plain).unwrap(), block_symbol(&inside).unwrap());
        assert_eq!(a.pins.iter().map(|p| p.at).collect::<Vec<_>>(), b.pins.iter().map(|p| p.at).collect::<Vec<_>>());
    }

    /// A 2.5 mm block and a 5 mm block with the same pins get different export names, and the sample block keeps the name "block".
    /// 同じピンを持つ2.5mm間隔と5mm間隔のブロックは書き出し名が違い、見本のブロックの名前は "block" のまま。
    #[test]
    fn fine_and_coarse_blocks_get_different_export_ids() {
        let pins = r#"{"left":[{"no":"1"}]}"#;
        let mut coarse = BTreeMap::new();
        coarse.insert(BLOCK_PINS_ATTR.to_string(), pins.to_string());
        assert_ne!(block_export_id(&block_symbol(&coarse).unwrap()), block_export_id(&block_symbol(&fine(pins, None)).unwrap()));
        assert_eq!(block_export_id(&block_template()), "block");
    }

    const FINE_GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/fine-blocks.json");

    /// (名前, 属性) の例: 表の6例、四辺にピンのある例、左右のピン数の偶奇が違う例、番号内側、5mm間隔で番号内側、名前あり、
    /// 5mm間隔で大きさ指定・番号内側、上下に名前、左右に名前。
    fn fine_cases() -> Vec<(String, BTreeMap<String, String>)> {
        let mut cases = Vec::new();
        for (h, n) in [(5.0, 1), (7.5, 2), (7.5, 1), (10.0, 3), (10.0, 2), (12.5, 4)] {
            cases.push((format!("left-{n}-h{h}"), fine(&left_pins(n), Some(&format!(r#"{{"w":5,"h":{h}}}"#)))));
        }
        let four = r#"{"left":[{"no":"L1","name":"IN"},{"no":"L2"}],"right":[{"no":"R1"},{"no":"R2"},{"no":"R3"}],"top":[{"no":"T1"}],"bottom":[{"no":"B1"},{"no":"B2"}]}"#;
        cases.push(("four-sides-auto".into(), fine(four, None)));
        cases.push(("four-sides-sized".into(), fine(four, Some(r#"{"w":17.5,"h":12.5}"#))));
        let mut inside = fine(four, None);
        inside.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
        cases.push(("four-sides-inside".into(), inside));
        let mut narrow = fine(r#"{"left":[{"no":"1234"}],"right":[{"no":"5678"}]}"#, Some(r#"{"w":5,"h":5}"#));
        narrow.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
        cases.push(("inside-widened".into(), narrow));
        let mut coarse_inside = BTreeMap::new();
        coarse_inside.insert(BLOCK_PINS_ATTR.to_string(), four.to_string());
        coarse_inside.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
        cases.push(("coarse-inside".into(), coarse_inside));
        let mut coarse_inside_sized = BTreeMap::new();
        coarse_inside_sized.insert(BLOCK_PINS_ATTR.to_string(), r#"{"left":[{"no":"1"},{"no":"2"}],"right":[{"no":"3"}]}"#.to_string());
        coarse_inside_sized.insert(BLOCK_SIZE_ATTR.to_string(), r#"{"w":20,"h":15}"#.to_string());
        coarse_inside_sized.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
        cases.push(("coarse-inside-sized".into(), coarse_inside_sized));
        let top_bottom = r#"{"top":[{"no":"1","name":"A"},{"no":"2","name":"B"}],"bottom":[{"no":"3","name":"C"}]}"#;
        cases.push(("fine-top-bottom-names".into(), fine(top_bottom, None)));
        let left_right = r#"{"left":[{"no":"1","name":"IN"},{"no":"2","name":"EN"}],"right":[{"no":"3","name":"OUT"}]}"#;
        cases.push(("fine-left-right-names".into(), fine(left_right, None)));
        let mut feed_coarse = BTreeMap::new();
        feed_coarse.insert(
            BLOCK_PINS_ATTR.to_string(),
            r#"{"left":[{"no":"1","name":"P"},{"no":"2","name":"N"}],"right":[{"no":"1","name":"P"},{"no":"2","name":"N"}]}"#.to_string(),
        );
        cases.push(("feedthrough-coarse".into(), feed_coarse));
        let mut feed_fine = fine(r#"{"left":[{"no":"1"},{"no":"2"}],"right":[{"no":"1"},{"no":"2"}]}"#, Some(r#"{"w":5,"h":7.5}"#));
        feed_fine.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
        cases.push(("feedthrough-fine-inside".into(), feed_fine));
        let mut feed_offset = fine(r#"{"left":[{"no":"1"},{"no":"2"}],"right":[{"no":"2"}]}"#, None);
        feed_offset.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
        cases.push(("feedthrough-inside-offset".into(), feed_offset));
        let mut feed_cross = fine(r#"{"left":[{"no":"1"}],"top":[{"no":"7"}],"bottom":[{"no":"7"}]}"#, None);
        feed_cross.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
        cases.push(("feedthrough-inside-crossing".into(), feed_cross));
        let mut feed_top = fine(r#"{"left":[{"no":"1234"}],"right":[{"no":"1234"}],"top":[{"no":"T"}]}"#, None);
        feed_top.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
        cases.push(("feedthrough-inside-pair-with-top".into(), feed_top));
        let mut bare = fine(r#"{"left":[{"no":"1"},{"no":"2"}],"right":[{"no":"1"},{"no":"2"}]}"#, Some(r#"{"w":5,"h":7.5}"#));
        bare.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
        bare.insert(BLOCK_LEADS_ATTR.to_string(), "none".to_string());
        cases.push(("leadless-feedthrough".into(), bare));
        let mut bare_tb = BTreeMap::new();
        bare_tb.insert(BLOCK_PINS_ATTR.to_string(), r#"{"left":[{"no":"A"}],"top":[{"no":"1"}],"bottom":[{"no":"2"}]}"#.to_string());
        bare_tb.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
        bare_tb.insert(BLOCK_LEADS_ATTR.to_string(), "none".to_string());
        cases.push(("leadless-coarse-three-sides".into(), bare_tb));
        cases
    }

    /// The 2.5 mm blocks and the blocks with numbers inside are drawn exactly as recorded in the golden file that the TypeScript twin is also checked against (rewrite it with MADAKE_UPDATE_GOLDEN=1).
    /// 2.5mm間隔のブロックと番号内側のブロックは、TS 側も突き合わせる golden ファイルのとおりに描かれる (MADAKE_UPDATE_GOLDEN=1 で書き直す)。
    #[test]
    fn fine_blocks_match_the_shared_golden_file() {
        let cases: Vec<serde_json::Value> = fine_cases()
            .into_iter()
            .map(|(name, a)| {
                let def = block_symbol(&a).unwrap_or_else(|e| panic!("{name}: {e}"));
                serde_json::json!({
                    "name": name,
                    "attrs": a,
                    "primitives": rounded(serde_json::to_value(&def.primitives).unwrap()),
                    "pins": rounded(serde_json::to_value(&def.pins).unwrap()),
                    "text_slots": rounded(serde_json::to_value(&def.text_slots).unwrap()),
                })
            })
            .collect();
        let want = serde_json::json!({ "cases": cases });
        if std::env::var("MADAKE_UPDATE_GOLDEN").as_deref() == Ok("1") {
            // 1行に1例 (差分が例ごとに読める)
            let lines: Vec<String> = want["cases"].as_array().unwrap().iter().map(|c| serde_json::to_string(c).unwrap()).collect();
            std::fs::write(FINE_GOLDEN, format!("{{\"cases\":[\n{}\n]}}\n", lines.join(",\n"))).expect("golden を書く");
        }
        let text = std::fs::read_to_string(FINE_GOLDEN)
            .unwrap_or_else(|_| panic!("{FINE_GOLDEN} が無い: MADAKE_UPDATE_GOLDEN=1 を付けて作る"));
        let got: serde_json::Value = serde_json::from_str(&text).expect("golden の JSON");
        assert!(got == want, "golden と違う: MADAKE_UPDATE_GOLDEN=1 で書き直し、git diff で変化を確かめる");
    }

    /// block_caption accepts only "above" and "inside"; a block without it writes its caption above, as before.
    /// block_caption は "above" と "inside" だけを受け付ける。属性の無いブロックは、これまでどおり上に書く。
    #[test]
    fn block_caption_accepts_only_above_and_inside() {
        assert_eq!(parse_block_spec(&BTreeMap::new()).unwrap().caption, BlockCaption::Above);
        for (value, want) in [("above", BlockCaption::Above), (" inside ", BlockCaption::Inside)] {
            let mut a = BTreeMap::new();
            a.insert(BLOCK_CAPTION_ATTR.to_string(), value.to_string());
            assert_eq!(parse_block_spec(&a).unwrap().caption, want);
        }
        for bad in ["below", "", "INSIDE"] {
            let mut a = BTreeMap::new();
            a.insert(BLOCK_CAPTION_ATTR.to_string(), bad.to_string());
            let err = parse_block_spec(&a).unwrap_err();
            assert!(err.contains(BLOCK_CAPTION_ATTR), "{bad}: {err}");
        }
    }

    /// block_caption does not change what the block itself draws: outline, pins and numbers are the same with and without it.
    /// block_caption はブロック自身の絵を変えない: 外形・ピン・番号は属性の有無で同じ。
    #[test]
    fn block_caption_does_not_change_the_drawn_block() {
        let mut plain = BTreeMap::new();
        plain.insert(BLOCK_PINS_ATTR.to_string(), r#"{"left":[{"no":"1","name":"IN"}],"right":[{"no":"2"}]}"#.to_string());
        let mut inside = plain.clone();
        inside.insert(BLOCK_CAPTION_ATTR.to_string(), "inside".to_string());
        assert_eq!(block_export_id(&block_symbol(&plain).unwrap()), block_export_id(&block_symbol(&inside).unwrap()));
    }

    /// A block's caption place survives being imported back from an exported file: the importer keeps block_caption with the pin table.
    /// ブロックの参照記号の置き場所は、書き出したファイルを読み戻しても残る: 取り込みは block_caption をピン表と一緒に保つ。
    #[test]
    fn imported_block_attrs_keep_the_caption_place() {
        let attrs = imported_block_attrs(Some(r#"{"left":[{"no":"1"}]}"#), None, None, None, Some("inside"), None).unwrap();
        assert_eq!(attrs[BLOCK_CAPTION_ATTR], "inside");
        assert!(imported_block_attrs(Some(r#"{"left":[{"no":"1"}]}"#), None, None, None, Some("below"), None).is_none());
        let attrs = imported_block_attrs(Some(r#"{"left":[{"no":"1"}]}"#), None, None, None, None, None).unwrap();
        assert!(!attrs.contains_key(BLOCK_CAPTION_ATTR));
    }

    /// When a left/right feed-through pair is not merged because the top side has a pin, the box is wide enough for both numbers: with four-character numbers it is as wide as the same block without the feed-through rule.
    /// 上辺にピンがあって左右の貫通ピンの番号をまとめないとき、箱は左右2つの番号が収まる幅になる: 4文字の番号なら、貫通でない同じ形のブロックと同じ幅になる。
    #[test]
    fn an_unmerged_feedthrough_pair_gets_room_for_both_numbers() {
        let mut pair = fine(r#"{"left":[{"no":"1234"}],"right":[{"no":"1234"}],"top":[{"no":"T"}]}"#, None);
        pair.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
        let mut plain = fine(r#"{"left":[{"no":"1234"}],"right":[{"no":"5678"}],"top":[{"no":"T"}]}"#, None);
        plain.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
        let (a, b) = (block_symbol(&pair).unwrap(), block_symbol(&plain).unwrap());
        let (ax1, _, ax2, _) = outline_of(&a);
        let (bx1, _, bx2, _) = outline_of(&b);
        assert_eq!((ax1, ax2), (bx1, bx2), "貫通の組も、まとめないなら同じ幅");
        let xs = |d: &SymbolDef| -> Vec<f64> { d.pins.iter().map(|p| p.at.x).collect() };
        assert_eq!(xs(&a), xs(&b), "ピンの位置も同じ");
        // 上辺のピンを外すと番号は1つにまとまり、箱は狭くなる
        let mut merged = fine(r#"{"left":[{"no":"1234"}],"right":[{"no":"1234"}]}"#, None);
        merged.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
        let (mx1, _, mx2, _) = outline_of(&block_symbol(&merged).unwrap());
        assert!(mx2 - mx1 < ax2 - ax1, "{} < {}", mx2 - mx1, ax2 - ax1);
    }

    fn leadless(pins: &str, size: Option<&str>) -> BTreeMap<String, String> {
        let mut a = fine(pins, size);
        a.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
        a.insert(BLOCK_LEADS_ATTR.to_string(), "none".to_string());
        a
    }

    /// A block with block_leads "none" puts its pins on the outline itself and draws no lead lines, so a wire runs straight to the box; the outline and the numbers are the same as with leads.
    /// block_leads が "none" のブロックは、ピンを外形の辺の上に置き、リード線を描かない。配線は箱へ直接つながる。外形と番号はリード線ありと同じ。
    #[test]
    fn a_leadless_block_puts_its_pins_on_the_outline() {
        let pins = r#"{"left":[{"no":"1"},{"no":"2"}],"right":[{"no":"1"},{"no":"2"}]}"#;
        let size = Some(r#"{"w":5,"h":7.5}"#);
        let mut with_leads = fine(pins, size);
        with_leads.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
        let (a, b) = (block_symbol(&with_leads).unwrap(), block_symbol(&leadless(pins, size)).unwrap());
        assert_eq!(outline_of(&a), outline_of(&b));
        let (x1, _, x2, _) = outline_of(&b);
        let xs: Vec<f64> = b.pins.iter().map(|p| p.at.x).collect();
        assert_eq!(xs, [x1, x1, x2, x2], "ピンは外形の左右の辺の上");
        assert_eq!(a.pins.iter().map(|p| p.at.y).collect::<Vec<_>>(), b.pins.iter().map(|p| p.at.y).collect::<Vec<_>>());
        let lines = |d: &SymbolDef| d.primitives.iter().filter(|p| matches!(p, Primitive::Line { .. })).count();
        let texts = |d: &SymbolDef| d.primitives.iter().filter(|p| matches!(p, Primitive::Text { .. })).count();
        assert_eq!((lines(&a), lines(&b)), (4, 0), "リード線は描かない");
        assert_eq!(texts(&a), texts(&b), "番号は同じ");
        assert!(b.pins.iter().all(|p| on_grid(p.at.x) && on_grid(p.at.y)));
        // 上下のピンも辺の上
        let tb = block_symbol(&leadless(r#"{"top":[{"no":"1"}],"bottom":[{"no":"2"}]}"#, Some(r#"{"w":5,"h":5}"#))).unwrap();
        let (_, y1, _, y2) = outline_of(&tb);
        assert_eq!(tb.pins.iter().map(|p| p.at.y).collect::<Vec<_>>(), [y1, y2]);
    }

    /// block_leads accepts only "lead" and "none", and "none" is refused unless the numbers are drawn inside (a number outside sits on the lead).
    /// block_leads は "lead" と "none" だけを受け付ける。"none" は番号を内側に描くブロックでなければ断る (外側の番号はリード線の上に書くため)。
    #[test]
    fn block_leads_accepts_lead_and_none_and_needs_numbers_inside() {
        let pins = r#"{"left":[{"no":"1"}]}"#;
        assert_eq!(parse_block_spec(&fine(pins, None)).unwrap().leads, BlockLeads::Lead);
        assert_eq!(parse_block_spec(&leadless(pins, None)).unwrap().leads, BlockLeads::None);
        let mut explicit = fine(pins, None);
        explicit.insert(BLOCK_LEADS_ATTR.to_string(), " lead ".to_string());
        assert_eq!(parse_block_spec(&explicit).unwrap().leads, BlockLeads::Lead);
        let mut outside = fine(pins, None);
        outside.insert(BLOCK_LEADS_ATTR.to_string(), "none".to_string());
        let err = parse_block_spec(&outside).unwrap_err();
        assert_eq!(err, "block_leads none needs block_numbers inside");
        let mut bad = leadless(pins, None);
        bad.insert(BLOCK_LEADS_ATTR.to_string(), "short".to_string());
        assert!(parse_block_spec(&bad).unwrap_err().contains(BLOCK_LEADS_ATTR));
        // 取り込みも保つ
        let kept = imported_block_attrs(Some(pins), None, Some("2.5"), Some("inside"), None, Some("none")).unwrap();
        assert_eq!(kept[BLOCK_LEADS_ATTR], "none");
    }
}
