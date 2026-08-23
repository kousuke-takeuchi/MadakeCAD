//! 配線注記 (G2): 1本のワイヤが持つ **線径 sq・線色・電線長** を図面へ書き出す注記。
//!
//! 参考図面の主情報で、これが載っていないと現場で電線を作れない。線番 ([`crate::wire_no`]) が
//! **ネット単位**で1回だけ描かれるのに対し、配線注記は **ワイヤ1本ごと**に描く (長さが1本ごとに
//! 違うため)。位置は線番と同じ「最長線分の中点」を使い、線番と重ならないよう**反対側**へ逃がす
//! (横線=下、縦線=右。線番は横線=上、縦線=左)。

use crate::geometry::Point;
use crate::model::{Entity, EntityId, Sheet, Wire};

/// 配線注記の文字高さ (mm)。線番より一回り小さい補助注記 (注記文字高が既定のときの値)。
pub const WIRE_NOTE_FONT: f64 = 2.0;
/// 配線注記と配線の間隔 (mm)。線番と同じグリッドピッチ (注記文字高が既定のときの値)。
pub const WIRE_NOTE_GAP: f64 = 2.5;

/// このシートで配線注記を描く文字高さ (mm)。シートの注記文字高
/// ([`Sheet::annotation_text_mm`]) そのもので、未設定なら [`WIRE_NOTE_FONT`]。
pub fn wire_note_font(sheet: &Sheet) -> f64 {
    WIRE_NOTE_FONT * sheet.annotation_scale()
}

/// 注記テキストの寄せ方。横線の下は中央揃え、縦線の右は左揃え。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteAlign {
    /// 中央揃え (SVGの`middle`)。
    Center,
    /// 左揃え (SVGの`start`)。
    Left,
}

/// 図面へ描く配線注記1件。
#[derive(Debug, Clone, PartialEq)]
pub struct WireNote {
    /// 注記の元になったワイヤ。
    pub wire_id: EntityId,
    /// 描く文字列 (例 `0.75sq 白 0.4m`)。
    pub text: String,
    /// 用紙座標 (mm) のテキスト基準点 (ベースライン)。
    pub at: Point,
    pub align: NoteAlign,
}

/// 線色名 → 図面に書く日本語の色名。未知の色名・`#`始まりの色はそのまま返す。
pub fn color_ja(color: &str) -> &str {
    match color {
        "black" => "黒",
        "white" => "白",
        "red" => "赤",
        "blue" => "青",
        "yellow" => "黄",
        "green" => "緑",
        "light_blue" | "sky" => "水色",
        "orange" => "橙",
        "brown" => "茶",
        "gray" | "grey" => "灰",
        "purple" | "violet" => "紫",
        "pink" => "桃",
        other => other,
    }
}

/// 図面用の数値整形。小数の末尾の0を落とす (0.75 → `0.75`、1.0 → `1`)。
fn num(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

/// ワイヤ属性から注記テキストを作る。書式は `{sq}sq {色} {長さ}m`。
/// 欠けている属性はその場ごと省き、全て欠けていれば`None` (注記を描かない)。
pub fn wire_note_text(wire: &Wire) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if wire.sq > 0.0 {
        parts.push(format!("{}sq", num(wire.sq)));
    }
    let color = wire.color.trim();
    if !color.is_empty() {
        parts.push(color_ja(color).to_string());
    }
    if let Some(length) = wire.length_m.filter(|l| *l > 0.0) {
        parts.push(format!("{}m", num(length)));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

/// シートの配線注記を、ワイヤid順 (決定的) に返す。属性の無いワイヤは含まない。
pub fn wire_notes(sheet: &Sheet) -> Vec<WireNote> {
    let mut out = Vec::new();
    // 注記文字高を下げたシートでは、配線からの逃がしも同じ比率で詰める (注記の層ごと小さくする)
    let gap = WIRE_NOTE_GAP * sheet.annotation_scale();
    for (id, entity) in &sheet.entities {
        let Entity::Wire(w) = entity else { continue };
        let Some(text) = wire_note_text(w) else {
            continue;
        };
        let Some((a, b)) = crate::svg::longest_segment(sheet, &[*id]) else {
            continue;
        };
        let mid = crate::svg::midpoint(&a, &b);
        // 線番は横線の上・縦線の左。注記はその反対側 (下・右) に置いて重ねない
        let (at, align) = if (b.x - a.x).abs() >= (b.y - a.y).abs() {
            (Point::new(mid.x, mid.y + gap), NoteAlign::Center)
        } else {
            (Point::new(mid.x + gap, mid.y), NoteAlign::Left)
        };
        out.push(WireNote {
            wire_id: *id,
            text,
            at,
            align,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use uuid::Uuid;

    fn wire(points: &[(f64, f64)], color: &str, sq: f64, length: Option<f64>) -> Wire {
        Wire {
            id: Uuid::new_v4(),
            points: points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            color: color.into(),
            sq,
            length_m: length,
            part_no: None,
            net: None,
        }
    }

    fn sheet_with(wires: Vec<Wire>) -> Sheet {
        let mut sheet = Sheet::new("S1", PaperSize::A3, Orientation::Landscape);
        for w in wires {
            sheet.entities.insert(w.id, Entity::Wire(w));
        }
        sheet
    }

    /// A wire that has a gauge, a colour and a cut length is annotated "0.75sq 白 0.4m" on the drawing.
    /// 線径・線色・電線長がそろったワイヤには「0.75sq 白 0.4m」という注記が付く。
    #[test]
    fn note_shows_gauge_colour_and_length() {
        let w = wire(&[(0.0, 0.0), (10.0, 0.0)], "white", 0.75, Some(0.4));
        assert_eq!(wire_note_text(&w).as_deref(), Some("0.75sq 白 0.4m"));
    }

    /// A missing attribute is simply left out of the note, so a wire without a length reads "0.75sq 白".
    /// 欠けている属性は注記から省かれ、長さの無いワイヤは「0.75sq 白」と書かれる。
    #[test]
    fn note_omits_missing_attributes() {
        let w = wire(&[(0.0, 0.0), (10.0, 0.0)], "white", 0.75, None);
        assert_eq!(wire_note_text(&w).as_deref(), Some("0.75sq 白"));
        let w = wire(&[(0.0, 0.0), (10.0, 0.0)], "", 0.0, Some(0.4));
        assert_eq!(wire_note_text(&w).as_deref(), Some("0.4m"));
    }

    /// A wire with no gauge, no colour and no length gets no note at all instead of an empty one.
    /// 線径も色も長さも無いワイヤには、空の注記ではなく注記そのものが付かない。
    #[test]
    fn wire_without_attributes_has_no_note() {
        let w = wire(&[(0.0, 0.0), (10.0, 0.0)], "", 0.0, None);
        assert_eq!(wire_note_text(&w), None);
    }

    /// A colour name the drawing does not know is printed as it was typed, so nothing is lost.
    /// 変換表に無い色名はそのまま書かれるので、入力した情報が消えることはない。
    #[test]
    fn unknown_colour_names_are_printed_unchanged() {
        let w = wire(&[(0.0, 0.0), (10.0, 0.0)], "turquoise", 0.3, None);
        assert_eq!(wire_note_text(&w).as_deref(), Some("0.3sq turquoise"));
        let w = wire(&[(0.0, 0.0), (10.0, 0.0)], "白", 0.3, None);
        assert_eq!(wire_note_text(&w).as_deref(), Some("0.3sq 白"), "日本語の色名はそのまま");
    }

    /// The note of a horizontal wire sits below the wire, on the opposite side from its wire number.
    /// 横向きのワイヤの注記は配線の下に置かれ、上に出る線番とは反対側になる。
    #[test]
    fn horizontal_wire_note_sits_below_the_wire() {
        let sheet = sheet_with(vec![wire(&[(0.0, 20.0), (40.0, 20.0)], "red", 0.5, Some(1.2))]);
        let notes = wire_notes(&sheet);
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].text, "0.5sq 赤 1.2m");
        assert!((notes[0].at.x - 20.0).abs() < 1e-9, "中点の真下: {:?}", notes[0].at);
        assert!((notes[0].at.y - (20.0 + WIRE_NOTE_GAP)).abs() < 1e-9, "配線の下");
        assert_eq!(notes[0].align, NoteAlign::Center);
    }

    /// The note of a vertical wire sits to the right of the wire, on the opposite side from its wire number.
    /// 縦向きのワイヤの注記は配線の右に置かれ、左に出る線番とは反対側になる。
    #[test]
    fn vertical_wire_note_sits_right_of_the_wire() {
        let sheet = sheet_with(vec![wire(&[(30.0, 0.0), (30.0, 40.0)], "black", 2.0, None)]);
        let notes = wire_notes(&sheet);
        assert_eq!(notes.len(), 1);
        assert!((notes[0].at.x - (30.0 + WIRE_NOTE_GAP)).abs() < 1e-9, "配線の右");
        assert!((notes[0].at.y - 20.0).abs() < 1e-9, "中点の高さ");
        assert_eq!(notes[0].align, NoteAlign::Left);
    }

    /// Every wire gets its own note, unlike the wire number which is drawn once per net.
    /// 注記はワイヤ1本ごとに付く(ネットごとに1回だけ描かれる線番とは違う)。
    #[test]
    fn each_wire_gets_its_own_note() {
        let sheet = sheet_with(vec![
            wire(&[(0.0, 10.0), (20.0, 10.0)], "red", 0.5, Some(0.3)),
            wire(&[(20.0, 10.0), (20.0, 40.0)], "red", 0.5, Some(0.9)),
        ]);
        let notes = wire_notes(&sheet);
        assert_eq!(notes.len(), 2, "同じネットでも1本ずつ注記が付く");
        let texts: Vec<&str> = notes.iter().map(|n| n.text.as_str()).collect();
        assert!(texts.contains(&"0.5sq 赤 0.3m"), "{texts:?}");
        assert!(texts.contains(&"0.5sq 赤 0.9m"), "{texts:?}");
    }

    /// A sheet that has no annotation text height set keeps the 2.0mm notes it has always had.
    /// 注記文字高を設定していないシートは、これまでどおり2.0mmの注記で描かれる。
    #[test]
    fn a_sheet_without_a_setting_keeps_the_default_note_height() {
        let sheet = sheet_with(vec![wire(&[(0.0, 20.0), (40.0, 20.0)], "red", 0.5, None)]);
        assert!((wire_note_font(&sheet) - 2.0).abs() < 1e-9);
        assert!((wire_notes(&sheet)[0].at.y - (20.0 + WIRE_NOTE_GAP)).abs() < 1e-9);
    }

    /// Setting the sheet's annotation text height to 1.4mm draws the wire notes at exactly that height.
    /// シートの注記文字高を1.4mmにすると、配線注記はちょうどその高さで描かれる。
    #[test]
    fn the_sheet_setting_is_the_wire_note_height() {
        let mut sheet = sheet_with(vec![wire(&[(0.0, 20.0), (40.0, 20.0)], "red", 0.5, None)]);
        sheet.annotation_text_mm = Some(1.4);
        assert!((wire_note_font(&sheet) - 1.4).abs() < 1e-9);
    }

    /// Smaller annotation text also moves the note closer to its wire, so a row of wires 5mm apart stays readable.
    /// 注記文字を小さくすると注記が配線に寄るので、5mm間隔で並ぶ配線でも読める余白が残る。
    #[test]
    fn smaller_annotation_text_moves_the_note_closer_to_the_wire() {
        let mut sheet = sheet_with(vec![wire(&[(0.0, 20.0), (40.0, 20.0)], "red", 0.5, None)]);
        sheet.annotation_text_mm = Some(1.4);
        let note = &wire_notes(&sheet)[0];
        assert!((note.at.y - (20.0 + WIRE_NOTE_GAP * 0.7)).abs() < 1e-9, "{note:?}");
        // 5mmピッチの隣の行 (y=25) の配線には届かない
        assert!(note.at.y + 1.4 < 25.0 - 1.4, "隣の行の注記と重ならない: {note:?}");
    }

    /// On a bent wire the note is placed on the longest straight run, where there is room to read it.
    /// 折れ曲がったワイヤの注記は、いちばん長い直線部分に置かれて読みやすくなる。
    #[test]
    fn note_is_placed_on_the_longest_straight_run() {
        let sheet = sheet_with(vec![wire(
            &[(0.0, 10.0), (5.0, 10.0), (5.0, 90.0)],
            "blue",
            0.3,
            None,
        )]);
        let notes = wire_notes(&sheet);
        assert_eq!(notes.len(), 1);
        assert!((notes[0].at.y - 50.0).abs() < 1e-9, "縦の長い区間の中点: {:?}", notes[0].at);
        assert_eq!(notes[0].align, NoteAlign::Left);
    }
}
