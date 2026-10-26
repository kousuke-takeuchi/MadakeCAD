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
    /// 明示指定の外形寸法(5mm刻みへ切り上げ済み)。未指定なら`None`。
    pub size: Option<BlockSize>,
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
/// - `block_size`は5mm刻みへ切り上げ、ピンが収まらない大きさならエラー
/// - `connector_ref` が空・まとめない種類に付いている・2つのまとまりに使われている → エラー ([`check_connectors`])
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
    let mut seen: std::collections::BTreeSet<&str> = Default::default();
    for pin in pins.all() {
        let no = pin.no.trim();
        if no.is_empty() {
            return Err("block pin number must not be empty".into());
        }
        if !seen.insert(no) {
            return Err(format!("duplicate block pin number: {no}"));
        }
        check_contacts(no, &pin.contacts)?;
    }
    check_connectors(&pins)?;

    let size = match attrs.get(BLOCK_SIZE_ATTR) {
        Some(raw) if !raw.trim().is_empty() => {
            let s: BlockSize = serde_json::from_str(raw)
                .map_err(|e| format!("{BLOCK_SIZE_ATTR} is not valid JSON: {e}"))?;
            if !(s.w.is_finite() && s.h.is_finite()) || s.w <= 0.0 || s.h <= 0.0 {
                return Err("block_size must be positive".into());
            }
            Some(BlockSize {
                w: snap_up(s.w, BLOCK_SIZE_STEP_MM),
                h: snap_up(s.h, BLOCK_SIZE_STEP_MM),
            })
        }
        _ => None,
    };
    if let Some(size) = size {
        let (need_w, need_h) = min_size(&pins);
        if size.w < need_w - 1e-9 || size.h < need_h - 1e-9 {
            return Err(format!(
                "block_size {}x{}mm is too small for its pins (needs at least {}x{}mm)",
                size.w, size.h, need_w, need_h
            ));
        }
    }
    Ok(BlockSpec { pins, size })
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

/// ピンが収まる最小の外形寸法 (mm)。辺のピン数×ピッチ(両端に半ピッチの余白)。
fn min_size(pins: &BlockPins) -> (f64, f64) {
    let rows = pins.left.len().max(pins.right.len());
    let cols = pins.top.len().max(pins.bottom.len());
    (
        (cols as f64 * BLOCK_PIN_PITCH_MM).max(BLOCK_SIZE_STEP_MM),
        (rows as f64 * BLOCK_PIN_PITCH_MM).max(BLOCK_SIZE_STEP_MM),
    )
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
/// - 高さ = 左右のピン数の多い方 × 5mm + 5mm(上下の余白)。上下辺のピン名ぶんも足す
/// - 幅 = 左右のピン名の最大幅 + 内側余白 + 中央の空き5mm。上下辺のピン数×5mm+5mmも下回らない
///
/// いずれも5mm刻みへ切り上げるので、外形の四辺は必ず2.5mmグリッドに載る。
pub fn block_box(spec: &BlockSpec) -> (f64, f64) {
    if let Some(size) = spec.size {
        // 指定の幅でも、左右のまとまりの名前が同じ行で重なるなら、収まる幅まで広げる
        let w = size.w.max(snap_up(shared_row_width(&spec.pins), BLOCK_SIZE_STEP_MM));
        return (w, size.h);
    }
    let pins = &spec.pins;
    let rows = pins.left.len().max(pins.right.len());
    let cols = pins.top.len().max(pins.bottom.len());
    // コネクタの無いブロックでは 1.5 + 1.5 = 3.0 で、今までの 2.0 × 1.5 と同じ値になる
    let w_names = side_pad(&pins.left)
        + side_pad(&pins.right)
        + side_text_width(&pins.left)
        + side_text_width(&pins.right)
        + BLOCK_PIN_PITCH_MM;
    let w = BLOCK_MIN_W_MM
        .max(w_names)
        .max(cols as f64 * BLOCK_PIN_PITCH_MM + BLOCK_PIN_PITCH_MM);
    let h = BLOCK_MIN_H_MM
        .max(rows as f64 * BLOCK_PIN_PITCH_MM + BLOCK_PIN_PITCH_MM)
        .max(caption_inset(&pins.top) + caption_inset(&pins.bottom) + BLOCK_PIN_PITCH_MM);
    (
        snap_up(w, BLOCK_SIZE_STEP_MM),
        snap_up(h, BLOCK_SIZE_STEP_MM),
    )
}

/// 属性からブロックのシンボル定義を生成する。
///
/// 外形は原点中心の長方形。ピンは辺ごとに中央揃え・5mmピッチで並び、
/// **ピン番号は外形の外側(リードの上)・ピン名は外形の内側**に描く
/// (上下辺は縦に書けないので番号・名前とも内側に置く)。
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
fn pin_layout(bp: &BlockPin, off: f64, dir: PinDir, hw: f64, hh: f64) -> PinLayout {
    let edge = edge_point(dir, off, hw, hh);
    if bp.connector.is_none() {
        let (at, num_at, name_at) = match dir {
            PinDir::Left => (
                Point::new(-hw - BLOCK_LEAD_MM, off),
                Point::new(-hw - BLOCK_LEAD_MM / 2.0, off - BLOCK_PIN_TEXT_MM),
                Point::new(-hw + BLOCK_TEXT_PAD_MM + name_width(&bp.name) / 2.0, off),
            ),
            PinDir::Right => (
                Point::new(hw + BLOCK_LEAD_MM, off),
                Point::new(hw + BLOCK_LEAD_MM / 2.0, off - BLOCK_PIN_TEXT_MM),
                Point::new(hw - BLOCK_TEXT_PAD_MM - name_width(&bp.name) / 2.0, off),
            ),
            PinDir::Up => (
                Point::new(off, -hh - BLOCK_LEAD_MM),
                Point::new(off, -hh + BLOCK_PIN_TEXT_MM),
                Point::new(off, -hh + BLOCK_PIN_TEXT_MM + BLOCK_PIN_PITCH_MM / 2.0),
            ),
            PinDir::Down => (
                Point::new(off, hh + BLOCK_LEAD_MM),
                Point::new(off, hh - BLOCK_PIN_TEXT_MM),
                Point::new(off, hh - BLOCK_PIN_TEXT_MM - BLOCK_PIN_PITCH_MM / 2.0),
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
            Point::new(off, -hh + BLOCK_PIN_TEXT_MM + BLOCK_PIN_PITCH_MM / 2.0),
        ),
        PinDir::Down => (
            Point::new(off, hh + CONNECTOR_LEAD_MM),
            Point::new(off, hh + CONNECTOR_OUT_MM),
            Point::new(off + CONNECTOR_NUM_SIDE_MM + num_w / 2.0, hh + num_tb),
            Point::new(off, hh - BLOCK_PIN_TEXT_MM - BLOCK_PIN_PITCH_MM / 2.0),
        ),
    };
    PinLayout { at, lead_from, num_at, name_at }
}

/// コネクタのまとまりの名前の位置: 左右の2本以上はまとまりの中ほどのピンの間の空いた行、
/// 1本だけはピン名の後ろ、上下はまとまりの中央で外形から7mm内側。
fn label_at(list: &[BlockPin], group: &ConnectorGroup, label: &str, dir: PinDir, hw: f64, hh: f64) -> Point {
    let n = list.len();
    let first = offset(group.start, n);
    let label_w = name_width(label);
    let name_in = BLOCK_TEXT_PAD_MM + CONNECTOR_IN_MM;
    match dir {
        PinDir::Left | PinDir::Right => {
            let count = group.end - group.start;
            let (y, start) = if count >= 2 {
                let k = ((count - 1) / 2) as f64;
                (first + BLOCK_PIN_PITCH_MM / 2.0 + BLOCK_PIN_PITCH_MM * k, name_in)
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
            let mid = (first + offset(group.end - 1, n)) / 2.0;
            let y = if dir == PinDir::Up {
                -hh + CONNECTOR_LABEL_TOP_MM
            } else {
                hh - CONNECTOR_LABEL_TOP_MM
            };
            Point::new(mid, y)
        }
    }
}

fn block_symbol_from_spec(spec: &BlockSpec) -> SymbolDef {
    let (w, h) = block_box(spec);
    let (hw, hh) = (w / 2.0, h / 2.0);
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

    let side = |list: &[BlockPin], dir: PinDir, primitives: &mut Vec<Primitive>, pins: &mut Vec<PinDef>| {
        let n = list.len();
        let groups = connector_groups(list);
        for (i, bp) in list.iter().enumerate() {
            // まとまりの最初のピンの前に、そのまとまりの絵と名前を描く
            if let Some(group) = groups.iter().find(|g| g.start == i) {
                let edges: Vec<Point> = (group.start..group.end)
                    .map(|j| edge_point(dir, offset(j, n), hw, hh))
                    .collect();
                primitives.extend(connector_primitives(group.kind, dir, &edges));
                if let Some(label) = group_label(group) {
                    let at = label_at(list, group, &label, dir, hw, hh);
                    primitives.push(text(at.x, at.y, &label, BLOCK_PIN_TEXT_MM));
                }
            }
            let layout = pin_layout(bp, offset(i, n), dir, hw, hh);
            primitives.push(lead(layout.lead_from.x, layout.lead_from.y, layout.at.x, layout.at.y));
            primitives.push(text(layout.num_at.x, layout.num_at.y, bp.no.trim(), BLOCK_PIN_TEXT_MM));
            if !bp.name.is_empty() {
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
        text_slots: center_slots(),
    }
}

/// ブロックのピン表を読めないとき、DXF/KiCadの読み込みが出す警告 (参照の後ろに続ける)。
pub const BLOCK_UNREADABLE_WARNING: &str = "ブロックのピン表を読めないので見本の箱にしました";

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

/// 読み込んだ`block_pins` / `block_size`の空でない値。ブロックとして読めないときは`None`。
pub fn imported_block_attrs(pins: Option<&str>, size: Option<&str>) -> Option<BTreeMap<String, String>> {
    let mut attrs = BTreeMap::new();
    for (k, v) in [(BLOCK_PINS_ATTR, pins), (BLOCK_SIZE_ATTR, size)] {
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

    /// Two pins with the same number, or a pin with an empty number, are rejected instead of silently drawn.
    /// 同じピン番号が2つある、またはピン番号が空のブロックは、黙って描かれずにエラーになる。
    #[test]
    fn duplicate_or_empty_pin_numbers_are_rejected() {
        let dup = block_symbol(&attrs(&[(
            "block_pins",
            r#"{"left":[{"no":"IN1"}],"right":[{"no":"IN1"}]}"#,
        )]))
        .expect_err("重複は拒否");
        assert!(dup.contains("duplicate"), "{dup}");
        let empty = block_symbol(&attrs(&[("block_pins", r#"{"left":[{"no":"  "}]}"#)]))
            .expect_err("空番号は拒否");
        assert!(empty.contains("empty"), "{empty}");
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
        let (inst, _) = placed(json, 0, false);
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

    /// The captions of a block that has no top pins stay exactly where they were (the top of the box plus the usual gaps).
    /// 上辺にピンの無いブロックの参照記号・型番は今までと同じ位置 (外形の上端+いつもの間隔) のまま。
    #[test]
    fn captions_of_a_block_without_top_pins_do_not_move() {
        let (inst, def) = placed(AMP_53, 0, false);
        let (min, _) = crate::svg::symbol_bounds(&inst, &def);
        assert!((crate::svg::symbol_top_y(&inst, &def) - min.y).abs() < 1e-9);
    }
}
