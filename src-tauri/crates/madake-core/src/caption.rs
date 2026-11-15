//! 参照記号・型番の置き場所を部品ごとに決める属性 `caption_layout`。
//!
//! 属性が無ければ、これまでの置き場所 (外形の上、ブロックは [`crate::block::BlockCaption`]) のまま。
//! 属性は JSON `{"ref": {...}, "value": {...}}`。どちらの行も省ける (省いた行はこれまでの置き場所)。
//! 行は `x` / `y` (配置点からのずれ mm、用紙の向き。回転・ミラーでは回らない。`y` はベースライン)、
//! `h` (文字サイズ mm、無ければ参照記号・型番の既定)、`anchor` (`start` / `middle` / `end`、無ければ `middle`)、
//! `hide` (true ならその行を描かない)。
//! TS 側 `src/canvas/caption.ts` と同じ規則。

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::model::TextAnchor;

/// 置き場所を持つ属性キー。
pub const CAPTION_LAYOUT_ATTR: &str = "caption_layout";
/// 文字サイズの下限・上限 (mm)。
pub const MIN_CAPTION_H_MM: f64 = 0.5;
pub const MAX_CAPTION_H_MM: f64 = 20.0;

/// 1行 (参照記号か型番) の置き場所。
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptionLine {
    /// 配置点からのずれ (mm)。
    #[serde(default)]
    pub x: f64,
    /// 配置点からのずれ (mm)。文字のベースライン。
    #[serde(default)]
    pub y: f64,
    /// 文字サイズ (mm)。
    #[serde(default)]
    pub h: Option<f64>,
    /// 文字の寄せ。
    #[serde(default = "middle")]
    pub anchor: TextAnchor,
    /// その行を描かない。
    #[serde(default)]
    pub hide: bool,
}

fn middle() -> TextAnchor {
    TextAnchor::Middle
}

/// 参照記号と型番の置き場所。
#[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptionLayout {
    /// 参照記号の行。
    #[serde(default, rename = "ref")]
    pub reference: Option<CaptionLine>,
    /// 型番・名前の行。
    #[serde(default)]
    pub value: Option<CaptionLine>,
}

/// 属性から置き場所を読む。属性が無い・空なら `Ok(None)`。JSON が壊れている・知らないキーがある・
/// 数が有限でない・文字サイズが範囲外ならエラー。
pub fn parse_layout(attrs: &BTreeMap<String, String>) -> Result<Option<CaptionLayout>, String> {
    let raw = match attrs.get(CAPTION_LAYOUT_ATTR) {
        Some(raw) if !raw.trim().is_empty() => raw,
        _ => return Ok(None),
    };
    let layout: CaptionLayout =
        serde_json::from_str(raw).map_err(|e| format!("{CAPTION_LAYOUT_ATTR} is not valid JSON: {e}"))?;
    for (name, line) in [("ref", layout.reference), ("value", layout.value)] {
        let Some(line) = line else { continue };
        if !line.x.is_finite() || !line.y.is_finite() {
            return Err(format!("{CAPTION_LAYOUT_ATTR}.{name}: x and y must be finite numbers"));
        }
        if let Some(h) = line.h {
            if !h.is_finite() || !(MIN_CAPTION_H_MM..=MAX_CAPTION_H_MM).contains(&h) {
                return Err(format!(
                    "{CAPTION_LAYOUT_ATTR}.{name}: h out of range: {h} (expected {MIN_CAPTION_H_MM}..{MAX_CAPTION_H_MM} mm)"
                ));
            }
        }
    }
    Ok(Some(layout))
}

/// 読める置き場所だけを返す (描画用。壊れた属性は無いものとして、これまでの置き場所で描く)。
pub fn layout_of(attrs: &BTreeMap<String, String>) -> Option<CaptionLayout> {
    parse_layout(attrs).ok().flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs(raw: &str) -> BTreeMap<String, String> {
        let mut a = BTreeMap::new();
        a.insert(CAPTION_LAYOUT_ATTR.to_string(), raw.to_string());
        a
    }

    /// A symbol without caption_layout, or with an empty one, has no caption layout: its reference and value stay where they were.
    /// caption_layout の無い (または空の) シンボルには置き場所の指定が無い: 参照記号と型番はこれまでの場所のまま。
    #[test]
    fn no_attribute_means_no_layout() {
        assert_eq!(parse_layout(&BTreeMap::new()), Ok(None));
        assert_eq!(parse_layout(&attrs("  ")), Ok(None));
    }

    /// A caption line gives an offset from the placement point, a text size, an alignment and a hide switch; what is left out gets its default (offset 0, the usual size, centred, shown).
    /// 行の指定は、配置点からのずれ・文字サイズ・寄せ・非表示を持つ。省いたものは既定になる (ずれ0、いつもの大きさ、中央寄せ、表示)。
    #[test]
    fn a_line_reads_offset_size_alignment_and_hide() {
        let layout = parse_layout(&attrs(r#"{"ref":{"x":-3,"y":-4.5,"h":2.0,"anchor":"start"},"value":{"hide":true}}"#)).unwrap().unwrap();
        let r = layout.reference.unwrap();
        assert_eq!((r.x, r.y, r.h, r.anchor, r.hide), (-3.0, -4.5, Some(2.0), TextAnchor::Start, false));
        let v = layout.value.unwrap();
        assert_eq!((v.x, v.y, v.h, v.anchor, v.hide), (0.0, 0.0, None, TextAnchor::Middle, true));
        let only_value = parse_layout(&attrs(r#"{"value":{"y":6}}"#)).unwrap().unwrap();
        assert!(only_value.reference.is_none(), "省いた行はこれまでの置き場所");
    }

    /// Broken JSON, an unknown key, a text size outside 0.5 to 20 mm and a non-number offset are refused with the attribute name in the message.
    /// 壊れた JSON・知らないキー・0.5〜20mm の外の文字サイズ・数でないずれは断り、メッセージに属性名を出す。
    #[test]
    fn bad_layouts_are_refused() {
        for bad in [
            "{",
            r#"{"ref":{"x":1,"size":2}}"#,
            r#"{"title":{"x":1}}"#,
            r#"{"ref":{"h":0.2}}"#,
            r#"{"value":{"h":25}}"#,
            r#"{"ref":{"anchor":"left"}}"#,
            r#"{"ref":{"x":"a"}}"#,
        ] {
            let err = parse_layout(&attrs(bad)).unwrap_err();
            assert!(err.contains(CAPTION_LAYOUT_ATTR), "{bad}: {err}");
            assert_eq!(layout_of(&attrs(bad)), None, "描画は壊れた属性を無いものとして扱う");
        }
    }
}
