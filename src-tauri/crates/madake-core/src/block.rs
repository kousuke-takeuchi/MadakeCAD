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
fn offset(i: usize, n: usize) -> f64 {
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

/// 上辺・下辺のピンが外形の内側で使う高さ (mm)。番号だけなら1行、名前もあれば2行。
fn caption_inset(side: &[BlockPin]) -> f64 {
    if side.is_empty() {
        0.0
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
    }

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

/// ピンが収まる最小の外形寸法 (mm)。辺のピン数×ピッチ(両端に半ピッチの余白)。
fn min_size(pins: &BlockPins) -> (f64, f64) {
    let rows = pins.left.len().max(pins.right.len());
    let cols = pins.top.len().max(pins.bottom.len());
    (
        (cols as f64 * BLOCK_PIN_PITCH_MM).max(BLOCK_SIZE_STEP_MM),
        (rows as f64 * BLOCK_PIN_PITCH_MM).max(BLOCK_SIZE_STEP_MM),
    )
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
        return (size.w, size.h);
    }
    let pins = &spec.pins;
    let rows = pins.left.len().max(pins.right.len());
    let cols = pins.top.len().max(pins.bottom.len());
    let w_names = 2.0 * BLOCK_TEXT_PAD_MM
        + max_name_width(&pins.left)
        + max_name_width(&pins.right)
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

fn block_symbol_from_spec(spec: &BlockSpec) -> SymbolDef {
    let (w, h) = block_box(spec);
    let (hw, hh) = (w / 2.0, h / 2.0);
    let mut primitives = vec![Primitive::Rect {
        p1: Point::new(-hw, -hh),
        p2: Point::new(hw, hh),
        filled: false,
    }];
    let mut pins: Vec<PinDef> = Vec::new();
    let pins_spec = &spec.pins;

    let side = |list: &[BlockPin], dir: PinDir, primitives: &mut Vec<Primitive>, pins: &mut Vec<PinDef>| {
        let n = list.len();
        for (i, bp) in list.iter().enumerate() {
            let off = offset(i, n);
            let (at, num_at, name_at) = match dir {
                PinDir::Left => (
                    Point::new(-hw - BLOCK_LEAD_MM, off),
                    Point::new(-hw - BLOCK_LEAD_MM / 2.0, off - BLOCK_PIN_TEXT_MM),
                    Point::new(
                        -hw + BLOCK_TEXT_PAD_MM + name_width(&bp.name) / 2.0,
                        off,
                    ),
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
            let edge = match dir {
                PinDir::Left => Point::new(-hw, off),
                PinDir::Right => Point::new(hw, off),
                PinDir::Up => Point::new(off, -hh),
                PinDir::Down => Point::new(off, hh),
            };
            primitives.push(lead(edge.x, edge.y, at.x, at.y));
            primitives.push(text(num_at.x, num_at.y, bp.no.trim(), BLOCK_PIN_TEXT_MM));
            if !bp.name.is_empty() {
                primitives.push(text(name_at.x, name_at.y, &bp.name, BLOCK_PIN_TEXT_MM));
            }
            pins.push(PinDef {
                number: bp.no.trim().into(),
                name: bp.name.clone(),
                at,
                dir,
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
}
