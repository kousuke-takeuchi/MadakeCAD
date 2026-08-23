//! ネットラベルの方向付きタグ形状 (G3 / G19)。
//!
//! ネットラベルは既定では文字だけを描くが、`attrs["tag"]` に `left` / `right` を入れると
//! **旗のような五角形**で囲んで描く。先端の三角がその向きを指すので、信号がその点から
//! 出ていくのか入ってくるのかが一目で分かる (産業図面で慣習的に使われる矢印タグ相当)。
//!
//! **基準点は先端の付け根** (G19で変更)。ネットラベルはネットに載るために必ず配線の
//! 頂点の上に置かれるので、旗の本体を基準点から**先端と反対側**へ伸ばすと、本体が
//! 自分の配線や配線注記の上に乗らない。`right` は本体が左・先端が右、`left` はその鏡像で、
//! 外形の寸法は左右で同じ。
//!
//! 枠の大きさは文字幅 ([`crate::tidy::text_width`]) から決まるので、長いネット名でも
//! はみ出さない。文字高さはシートの注記文字高 ([`crate::model::Sheet::annotation_scale`])
//! に従って伸縮し、余白・先端・逃がし量も同じ倍率で一緒に縮む。グリフはSVG出力と
//! キャンバスで同一の数式を使う。

use crate::geometry::Point;
use crate::model::{NetLabel, TextAnchor};
use crate::svg::{LABEL_FONT, NET_LABEL_DY};
use crate::xref::XREF_GAP;

/// タグ枠と文字の間の余白 (mm。注記文字高が既定のときの値)。
pub const TAG_PAD: f64 = 1.0;
/// 文字幅の見積りに持たせる余裕。ラベル本文はサンセリフで描かれ、大文字が多いと
/// 等幅前提の見積り ([`crate::tidy::text_width`]) より1割ほど広くなるため。
pub const TAG_WIDTH_SLACK: f64 = 1.1;
/// 先端の三角の長さ (mm。注記文字高が既定のときの値)。
pub const TAG_TIP: f64 = 2.0;
/// タグ枠の線幅 (mm)。
pub const TAG_STROKE: f64 = 0.25;
/// `attrs` でタグの向きを指定するキー。
pub const TAG_ATTR: &str = "tag";

/// タグの向き。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagDir {
    /// 先端が左を指す (本体は基準点から右へ伸びる)。
    Left,
    /// 先端が右を指す (本体は基準点から左へ伸びる)。
    Right,
}

/// 描画するタグ形状。座標は用紙座標 (mm)。
#[derive(Debug, Clone, PartialEq)]
pub struct NetTag {
    pub dir: TagDir,
    /// 五角形の頂点 (閉じた多角形として描く)。
    pub points: Vec<Point>,
    /// ネット名のベースライン左端 (左揃えで描く)。
    pub text_at: Point,
    /// グリフの左端 (先端を含む)。
    pub left: f64,
    /// グリフの右端 (先端を含む)。
    pub right: f64,
    /// シート間クロスリファレンスの基準点。本体側 (先端と反対側) の外へ逃がす。
    pub xref_at: Point,
    /// クロスリファレンスの寄せ方。左へ逃がすときは右揃え。
    pub xref_anchor: TextAnchor,
}

/// ラベルのタグ向き。`attrs["tag"]`が無い・空・知らない値なら`None` (従来どおり文字だけ)。
pub fn tag_dir(label: &NetLabel) -> Option<TagDir> {
    match label.attrs.get(TAG_ATTR).map(|s| s.trim()) {
        Some("left") => Some(TagDir::Left),
        Some("right") => Some(TagDir::Right),
        _ => None,
    }
}

/// ラベルのタグ形状。タグ指定の無いラベルは`None`。
///
/// `font` はネットラベル本文の文字高さ (mm)。シートの注記文字高で決まり、
/// 余白・先端・ベースラインの持ち上げも同じ比率で伸縮する。
pub fn net_tag(label: &NetLabel, font: f64) -> Option<NetTag> {
    let dir = tag_dir(label)?;
    let scale = font / LABEL_FONT;
    let (pad, tip, gap) = (TAG_PAD * scale, TAG_TIP * scale, XREF_GAP * scale);
    let text_w = crate::tidy::text_width(&label.name, font) * TAG_WIDTH_SLACK;
    let body_w = text_w + pad * 2.0;
    let baseline = label.at.y - NET_LABEL_DY * scale;
    let top = baseline - font - pad;
    let bottom = baseline + pad;
    let cy = (top + bottom) / 2.0;
    let x0 = label.at.x;
    let (points, text_x, left, right, xref_x, xref_anchor) = match dir {
        // 先端が右 = 本体は基準点の左側。逃がしも左 (右揃え)
        TagDir::Right => (
            vec![
                Point::new(x0 - body_w, top),
                Point::new(x0, top),
                Point::new(x0 + tip, cy),
                Point::new(x0, bottom),
                Point::new(x0 - body_w, bottom),
            ],
            x0 - body_w + pad,
            x0 - body_w,
            x0 + tip,
            x0 - body_w - gap,
            TextAnchor::End,
        ),
        // 先端が左 = 本体は基準点の右側。逃がしも右 (左揃え)
        TagDir::Left => (
            vec![
                Point::new(x0 - tip, cy),
                Point::new(x0, top),
                Point::new(x0 + body_w, top),
                Point::new(x0 + body_w, bottom),
                Point::new(x0, bottom),
            ],
            x0 + pad,
            x0 - tip,
            x0 + body_w,
            x0 + body_w + gap,
            TextAnchor::Start,
        ),
    };
    Some(NetTag {
        dir,
        points,
        text_at: Point::new(text_x, baseline),
        left,
        right,
        xref_at: Point::new(xref_x, baseline),
        xref_anchor,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn label(name: &str, tag: Option<&str>) -> NetLabel {
        let mut attrs = BTreeMap::new();
        if let Some(t) = tag {
            attrs.insert(TAG_ATTR.to_string(), t.to_string());
        }
        NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(100.0, 50.0),
            name: name.into(),
            rotation: 0,
            attrs,
        }
    }

    fn tag_of(name: &str, dir: &str) -> NetTag {
        net_tag(&label(name, Some(dir)), LABEL_FONT).unwrap()
    }

    /// A net label with tag "right" is drawn as a five-cornered flag whose point sticks out to the right of the label position.
    /// タグ「右」のネットラベルは、置いた点から右へ先端が飛び出した五角形の旗として描かれる。
    #[test]
    fn tag_right_points_to_the_right() {
        let tag = tag_of("24V", "right");
        assert_eq!(tag.dir, TagDir::Right);
        assert_eq!(tag.points.len(), 5, "五角形");
        let tip = tag.points[2];
        assert!(tip.x > 100.0, "先端は基準点の右: {tip:?}");
        assert!((tip.x - tag.right).abs() < 1e-9, "先端がグリフの右端");
        // 先端は上下の中央 (角の高さの真ん中) に来る
        let (top, bottom) = (tag.points[0].y, tag.points[4].y);
        assert!((tip.y - (top + bottom) / 2.0).abs() < 1e-9);
    }

    /// A net label with tag "left" is drawn as the mirrored flag, with the point sticking out to the left.
    /// タグ「左」のネットラベルは、先端が左へ飛び出した左右反転した旗として描かれる。
    #[test]
    fn tag_left_points_to_the_left() {
        let tag = tag_of("24V", "left");
        assert_eq!(tag.dir, TagDir::Left);
        assert_eq!(tag.points.len(), 5);
        let tip = tag.points[0];
        assert!(tip.x < 100.0, "先端は基準点の左: {tip:?}");
        assert!((tip.x - tag.left).abs() < 1e-9, "先端がグリフの左端");
    }

    /// The body of the flag never covers the label's own position: it grows away from the point, so the wire the label sits on stays readable.
    /// 旗の本体はラベルを置いた点を覆わず、先端と反対側へ伸びる。ラベルが載っている配線が隠れない。
    #[test]
    fn the_flag_body_grows_away_from_the_point() {
        let right = tag_of("24V", "right");
        for p in &right.points {
            assert!(p.x <= 100.0 + TAG_TIP + 1e-9, "本体は基準点の左: {p:?}");
        }
        assert!(right.right - 100.0 <= TAG_TIP + 1e-9, "右へは先端の分しか出ない");
        assert!(right.text_at.x < 100.0, "文字も基準点の左");

        let left = tag_of("24V", "left");
        for p in &left.points {
            assert!(p.x >= 100.0 - TAG_TIP - 1e-9, "本体は基準点の右: {p:?}");
        }
        assert!(left.text_at.x > 100.0, "文字は基準点の右");
    }

    /// The label position is the root of the point, so a left flag and a right flag are exact mirror images of each other.
    /// ラベルを置いた点は先端の付け根にあたるので、左向きの旗と右向きの旗はちょうど鏡像になる。
    #[test]
    fn the_label_position_is_the_root_of_the_point() {
        let right = tag_of("24V", "right");
        let left = tag_of("24V", "left");
        // 基準点 (100) を軸に折り返すと同じ形になる
        let mut mirrored: Vec<f64> = right.points.iter().map(|p| 200.0 - p.x).collect();
        mirrored.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mut xs: Vec<f64> = left.points.iter().map(|p| p.x).collect();
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for (a, b) in mirrored.iter().zip(xs.iter()) {
            assert!((a - b).abs() < 1e-9, "鏡像: {mirrored:?} vs {xs:?}");
        }
        // 付け根 (本体と先端の境目) は基準点そのもの
        assert!((right.points[1].x - 100.0).abs() < 1e-9);
        assert!((left.points[1].x - 100.0).abs() < 1e-9);
    }

    /// The tag text always sits inside the flag, and the flag grows with the length of the net name.
    /// タグの文字は必ず旗の内側に収まり、旗の幅はネット名の長さに合わせて伸びる。
    #[test]
    fn tag_box_follows_the_text_width() {
        let short = tag_of("A", "right");
        let long = tag_of("24V_CONTROL_1", "right");
        assert!(long.right - long.left > short.right - short.left, "長い名前ほど広い");
        for tag in [&short, &long] {
            assert!(tag.text_at.x > tag.left, "文字は枠の内側から始まる");
            assert!(
                tag.text_at.x + crate::tidy::text_width("A", LABEL_FONT) < tag.right,
                "文字は枠からはみ出さない"
            );
        }
    }

    /// Both tag directions produce a flag of the very same size, so left and right labels line up.
    /// 左向きと右向きのタグは同じ大きさになるので、左右のラベルが並んでも高さと幅がそろう。
    #[test]
    fn both_directions_have_the_same_size() {
        let right = tag_of("24V", "right");
        let left = tag_of("24V", "left");
        assert!(((right.right - right.left) - (left.right - left.left)).abs() < 1e-9);
        assert!((right.points[0].y - left.points[1].y).abs() < 1e-9, "高さもそろう");
    }

    /// The cross-sheet reference is pushed out on the body side of the flag, so it never lands on the point or on the wire.
    /// シート間クロスリファレンスは旗の本体側の外へ逃がされるので、先端や配線の上には出ない。
    #[test]
    fn the_cross_reference_escapes_on_the_body_side() {
        let right = tag_of("24V", "right");
        assert!(right.xref_at.x < right.left, "右向きタグは左へ逃がす: {right:?}");
        assert_eq!(right.xref_anchor, TextAnchor::End, "左へ伸びるので右揃え");
        assert!((right.xref_at.y - right.text_at.y).abs() < 1e-9, "ベースラインは本文と同じ");

        let left = tag_of("24V", "left");
        assert!(left.xref_at.x > left.right, "左向きタグは右へ逃がす");
        assert_eq!(left.xref_anchor, TextAnchor::Start);
    }

    /// A smaller annotation text height shrinks the whole flag - text, padding and point - keeping its shape.
    /// 注記文字高を小さくすると、文字・余白・先端がまとめて縮み、旗の形はそのままで小さくなる。
    #[test]
    fn a_smaller_annotation_height_shrinks_the_whole_flag() {
        let big = tag_of("24V", "right");
        let small = net_tag(&label("24V", Some("right")), LABEL_FONT * 0.6).unwrap();
        let width = |t: &NetTag| t.right - t.left;
        let height = |t: &NetTag| t.points[4].y - t.points[0].y;
        assert!((width(&small) / width(&big) - 0.6).abs() < 1e-9, "幅は同じ比率で縮む");
        assert!((height(&small) / height(&big) - 0.6).abs() < 1e-9, "高さも同じ比率");
        assert!((small.points[2].x - 100.0) < (big.points[2].x - 100.0), "先端も短くなる");
    }

    /// A net label without the tag attribute keeps the plain text look it has always had.
    /// タグ属性の無いネットラベルは、これまでどおり文字だけの見た目のままになる。
    #[test]
    fn label_without_tag_attribute_stays_plain_text() {
        assert!(net_tag(&label("24V", None), LABEL_FONT).is_none());
        assert!(tag_dir(&label("24V", None)).is_none());
    }

    /// An unknown tag value (a typo, or "up") is ignored and the label falls back to plain text instead of failing.
    /// 知らないタグの値(打ち間違いや "up")は無視され、失敗せずに文字だけの表示に戻る。
    #[test]
    fn unknown_tag_value_is_ignored() {
        for value in ["up", "", "LEFT", "右"] {
            assert!(net_tag(&label("24V", Some(value)), LABEL_FONT).is_none(), "{value}");
        }
    }
}
