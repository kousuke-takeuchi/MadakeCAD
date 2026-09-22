//! 注記テキストの装飾 (G4) と、シンボルの機能ラベル。
//!
//! 図面の注記は、色で情報の層を分ける (機能名・配線仕様・ネット名を別の色にする) と
//! 一気に読みやすくなる。ただし**生の色コードは受け付けない**。使えるのは
//! [`TEXT_COLORS`] の**トークン名**だけで、画面 (暗い作図領域) と紙 (白地) では
//! それぞれ見やすい色に置き換えて描く。こうしておくと、後から配色を変えても
//! 図面データを触らずに済む。

use crate::geometry::Point;
use crate::model::{SymbolInstance, TextAnchor, TextEntity};
use crate::symbol::{SymbolDef, SLOT_FUNCTION};

/// 注記色のセマンティックトークンと**印刷色** (紙・PDF・SVG)。
///
/// 画面 (キャンバス) 側の色は `src/canvas/theme.ts` の `textColors` が持つ。
/// 両者は同じトークン名で対応する。
///
/// `default`だけ`#000`という短い記法なのは、色を指定していない従来の注記の出力を
/// **1バイトも変えない**ため。
pub const TEXT_COLORS: &[(&str, &str)] = &[
    // 通常の注記。
    ("default", "#000"),
    // 強調・参照 (青系)。
    ("accent", "#0000c0"),
    // 注意喚起 (赤系)。
    ("warning", "#c00000"),
    // 確認済み・良好 (緑系)。
    ("success", "#008040"),
    // 補足・控えめ (灰)。
    ("muted", "#808080"),
];

/// 色を指定していない注記が使うトークン。
pub const DEFAULT_TEXT_COLOR: &str = "default";

/// 機能ラベル (第3のテキスト欄) をシンボルインスタンスへ入れる属性キー。
pub const FUNCTION_LABEL_ATTR: &str = "function_label";

/// 機能ラベルのベースラインを、シンボル外形の下端からどれだけ下に置くか (mm)。
/// 参照記号・型番と同じ行送り (文字高さ2.5 + 行間1.0)。
pub const FUNCTION_LABEL_DY: f64 = 3.5;

/// 使える注記色のトークン名 (定義順)。
pub fn text_color_tokens() -> impl Iterator<Item = &'static str> {
    TEXT_COLORS.iter().map(|(token, _)| *token)
}

/// トークン名が使える注記色か。
pub fn is_text_color_token(token: &str) -> bool {
    TEXT_COLORS.iter().any(|(name, _)| *name == token)
}

/// トークン名 → 印刷色。知らない名前や生の色コードは`None`。
pub fn text_print_color(token: &str) -> Option<&'static str> {
    TEXT_COLORS
        .iter()
        .find(|(name, _)| *name == token)
        .map(|(_, hex)| *hex)
}

/// 注記を紙へ描くときの色。色指定が無ければ既定色 (黒)。
pub fn text_fill(text: &TextEntity) -> &'static str {
    text.color
        .as_deref()
        .and_then(text_print_color)
        .unwrap_or_else(|| text_print_color(DEFAULT_TEXT_COLOR).expect("既定色は必ずある"))
}

/// 基準位置 → SVGの`text-anchor`。
pub fn svg_anchor(anchor: TextAnchor) -> &'static str {
    match anchor {
        TextAnchor::Start => "start",
        TextAnchor::Middle => "middle",
        TextAnchor::End => "end",
    }
}

/// シンボルに付けられた機能ラベルの文字列 (例「非常停止ボタン」)。
/// 属性が無い・空白だけなら`None`。
pub fn function_label(inst: &SymbolInstance) -> Option<&str> {
    inst.attrs
        .get(FUNCTION_LABEL_ATTR)
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
}

/// 機能ラベルの描画指示 (用紙座標)。
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionLabel {
    /// ベースラインの基準点 (中央揃えで描く)。
    pub at: Point,
    /// 文字高さ mm。
    pub height: f64,
}

/// シンボルの機能ラベルをどこへ描くか。
///
/// シンボル定義に`FUNCTION`スロット ([`SLOT_FUNCTION`]) があればその位置 (回転・ミラーも効く)。
/// 無ければ**外形の下**へ自動で置くので、既存のシンボル定義は1つも直さなくてよい。
/// 機能ラベルが設定されていないシンボルは`None`。
pub fn function_label_placement(inst: &SymbolInstance, def: &SymbolDef) -> Option<FunctionLabel> {
    function_label(inst)?;
    let slot = def.text_slots.iter().find(|s| s.key == SLOT_FUNCTION);
    Some(match slot {
        Some(slot) => FunctionLabel {
            at: crate::netlist::transform_local(slot.at, inst),
            height: slot.height,
        },
        None => {
            let (_, max) = crate::svg::symbol_bounds(inst, def);
            return function_label_below(inst, inst.at.x, max.y);
        }
    })
}

/// 外形の下端 (`bottom_y`) と中心X (`center_x`) を指定して、その**下**へ置く機能ラベルの
/// 描画指示を求める。シンボル定義の外形とは別の外形で描くとき (実機ブロック表示 (G7) の
/// コイル) にも同じ行送りで置けるようにしたもの。機能ラベルが無ければ`None`。
pub fn function_label_below(
    inst: &SymbolInstance,
    center_x: f64,
    bottom_y: f64,
) -> Option<FunctionLabel> {
    function_label(inst)?;
    Some(FunctionLabel {
        at: Point::new(center_x, bottom_y + FUNCTION_LABEL_DY),
        height: crate::svg::LABEL_FONT,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::{builtin_symbols, TextSlot};
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn note(color: Option<&str>) -> TextEntity {
        TextEntity {
            id: Uuid::new_v4(),
            at: Point::new(10.0, 20.0),
            text: "注意".into(),
            height: 2.5,
            rotation: 0,
            anchor: TextAnchor::Start,
            color: color.map(|c| c.to_string()),
            bold: false,
            leader_to: None,
        }
    }

    fn symbol(symbol_id: &str, function: Option<&str>) -> SymbolInstance {
        let mut attrs = BTreeMap::new();
        if let Some(f) = function {
            attrs.insert(FUNCTION_LABEL_ATTR.to_string(), f.to_string());
        }
        SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: symbol_id.into(),
            at: Point::new(100.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "SW1".into(),
            value: String::new(),
            attrs,
        }
    }

    fn def_of(symbol_id: &str) -> SymbolDef {
        builtin_symbols()
            .into_iter()
            .find(|d| d.id == symbol_id)
            .expect("同梱シンボル")
    }

    /// Every note colour is a named token with a print colour, and the default token prints black.
    /// 注記の色はどれも印刷色を持つ名前付きトークンで、既定のトークンは黒で刷られる。
    #[test]
    fn every_colour_token_has_a_print_colour() {
        for token in text_color_tokens() {
            let hex = text_print_color(token).expect(token);
            assert!(hex.starts_with('#'), "{token} → {hex}");
        }
        assert_eq!(text_print_color(DEFAULT_TEXT_COLOR), Some("#000"));
        assert!(is_text_color_token("accent"));
        assert!(is_text_color_token("warning"));
    }

    /// A raw colour code is not a colour token, so it can never sneak into a drawing.
    /// 生の色コードは色トークンではないので、図面に紛れ込むことはない。
    #[test]
    fn a_raw_colour_code_is_not_a_colour_token() {
        for value in ["#ff0000", "red", "赤", "", "DEFAULT"] {
            assert!(!is_text_color_token(value), "{value}");
            assert_eq!(text_print_color(value), None, "{value}");
        }
    }

    /// A note with no colour is drawn in the default colour, and a note with a token is drawn in that token's print colour.
    /// 色を指定していない注記は既定色で描かれ、トークンを指定した注記はそのトークンの印刷色で描かれる。
    #[test]
    fn a_note_without_a_colour_uses_the_default_colour() {
        assert_eq!(text_fill(&note(None)), "#000");
        assert_eq!(text_fill(&note(Some("default"))), "#000");
        assert_eq!(text_fill(&note(Some("accent"))), "#0000c0");
        assert_eq!(text_fill(&note(Some("warning"))), "#c00000");
    }

    /// The three anchors map to the three SVG text-anchor values, and the default anchor is the one drawings have always used.
    /// 3つの基準位置はSVGの text-anchor の3つの値に対応し、既定は従来の図面が使ってきた左揃えである。
    #[test]
    fn anchors_map_to_svg_text_anchor() {
        assert_eq!(svg_anchor(TextAnchor::Start), "start");
        assert_eq!(svg_anchor(TextAnchor::Middle), "middle");
        assert_eq!(svg_anchor(TextAnchor::End), "end");
        assert_eq!(svg_anchor(TextAnchor::default()), "start");
    }

    /// A symbol with no function label attribute has nothing extra to draw.
    /// 機能ラベルの属性が無いシンボルには、余分に描くものが無い。
    #[test]
    fn a_symbol_without_a_function_label_draws_nothing_extra() {
        let inst = symbol("switch_spst", None);
        assert_eq!(function_label(&inst), None);
        assert_eq!(function_label_placement(&inst, &def_of("switch_spst")), None);
    }

    /// A blank function label counts as no label, so a stray space does not print an empty line.
    /// 空白だけの機能ラベルは「無し」と同じ扱いになるので、うっかりの空白で空行が刷られることはない。
    #[test]
    fn a_blank_function_label_counts_as_no_label() {
        assert_eq!(function_label(&symbol("switch_spst", Some("   "))), None);
        assert_eq!(function_label(&symbol("switch_spst", Some(" 非常停止 "))), Some("非常停止"));
    }

    /// With a bundled symbol (which defines no function slot) the function label lands just below the symbol outline, centred on the placement point.
    /// 同梱シンボル (機能スロットを持たない) では、機能ラベルはシンボル外形のすぐ下に、配置基準点を中心にして置かれる。
    #[test]
    fn the_function_label_sits_below_a_symbol_that_defines_no_slot() {
        let inst = symbol("switch_spst", Some("非常停止ボタン"));
        let def = def_of("switch_spst");
        let placed = function_label_placement(&inst, &def).expect("機能ラベルがある");
        let (_, max) = crate::svg::symbol_bounds(&inst, &def);
        assert!((placed.at.x - inst.at.x).abs() < 1e-9, "中心は配置基準点");
        assert!(placed.at.y > max.y, "外形より下: {:?} / {:?}", placed.at, max);
        assert!((placed.at.y - (max.y + FUNCTION_LABEL_DY)).abs() < 1e-9);
        assert_eq!(placed.height, crate::svg::LABEL_FONT);
    }

    /// When the symbol definition does place a FUNCTION slot, the label follows the slot (rotation and all) instead of the automatic position.
    /// シンボル定義がFUNCTIONスロットを持つ場合、機能ラベルは自動位置ではなくそのスロット (回転も込み) に従う。
    #[test]
    fn the_function_label_follows_a_slot_defined_by_the_symbol() {
        let mut def = def_of("switch_spst");
        def.text_slots.push(TextSlot {
            key: SLOT_FUNCTION.into(),
            at: Point::new(0.0, -12.0),
            height: 3.0,
        });
        let inst = symbol("switch_spst", Some("非常停止ボタン"));
        let placed = function_label_placement(&inst, &def).expect("機能ラベルがある");
        assert_eq!(placed.at, Point::new(100.0, 38.0), "基準点の12mm上");
        assert_eq!(placed.height, 3.0, "スロットの文字高さ");

        let mut turned = inst.clone();
        turned.rotation = 90;
        let placed = function_label_placement(&turned, &def).expect("機能ラベルがある");
        assert_eq!(placed.at, Point::new(112.0, 50.0), "90度回すとラベルも回る");
    }
}
