//! ネットラベルの方向付きタグ形状 (G3)。
//!
//! ネットラベルは既定では文字だけを描くが、`attrs["tag"]` に `left` / `right` を入れると
//! **旗のような五角形**で囲んで描く。先端の三角がその向きを指すので、信号がその点から
//! 出ていくのか入ってくるのかが一目で分かる (参考図面の矢印タグ相当)。
//!
//! 枠の大きさは文字幅 ([`crate::tidy::text_width`]) から決まるので、長いネット名でも
//! はみ出さない。グリフはSVG出力とキャンバスで同一の数式を使う。

use crate::geometry::Point;
use crate::model::NetLabel;
use crate::svg::{LABEL_FONT, NET_LABEL_DY};

/// タグ枠と文字の間の余白 (mm)。
pub const TAG_PAD: f64 = 1.0;
/// 文字幅の見積りに持たせる余裕。ラベル本文はサンセリフで描かれ、大文字が多いと
/// 等幅前提の見積り ([`crate::tidy::text_width`]) より1割ほど広くなるため。
pub const TAG_WIDTH_SLACK: f64 = 1.1;
/// 先端の三角の長さ (mm)。
pub const TAG_TIP: f64 = 2.0;
/// タグ枠の線幅 (mm)。
pub const TAG_STROKE: f64 = 0.25;
/// `attrs` でタグの向きを指定するキー。
pub const TAG_ATTR: &str = "tag";

/// タグの向き。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagDir {
    /// 先端が左を指す (本体は右へ伸びる)。
    Left,
    /// 先端が右を指す (本体は左=基準点側から伸びる)。
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
    /// グリフの右端。シート間クロスリファレンスはこの右へ逃がす。
    pub right: f64,
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
pub fn net_tag(label: &NetLabel) -> Option<NetTag> {
    let dir = tag_dir(label)?;
    let text_w = crate::tidy::text_width(&label.name, LABEL_FONT) * TAG_WIDTH_SLACK;
    let body_w = text_w + TAG_PAD * 2.0;
    let baseline = label.at.y - NET_LABEL_DY;
    let top = baseline - LABEL_FONT - TAG_PAD;
    let bottom = baseline + TAG_PAD;
    let cy = (top + bottom) / 2.0;
    let x0 = label.at.x;
    let (points, text_x, right) = match dir {
        TagDir::Right => (
            vec![
                Point::new(x0, top),
                Point::new(x0 + body_w, top),
                Point::new(x0 + body_w + TAG_TIP, cy),
                Point::new(x0 + body_w, bottom),
                Point::new(x0, bottom),
            ],
            x0 + TAG_PAD,
            x0 + body_w + TAG_TIP,
        ),
        TagDir::Left => (
            vec![
                Point::new(x0, cy),
                Point::new(x0 + TAG_TIP, top),
                Point::new(x0 + TAG_TIP + body_w, top),
                Point::new(x0 + TAG_TIP + body_w, bottom),
                Point::new(x0 + TAG_TIP, bottom),
            ],
            x0 + TAG_TIP + TAG_PAD,
            x0 + TAG_TIP + body_w,
        ),
    };
    Some(NetTag {
        dir,
        points,
        text_at: Point::new(text_x, baseline),
        right,
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

    /// A net label with tag "right" is drawn as a five-cornered flag whose point sticks out to the right.
    /// タグ「右」のネットラベルは、右へ先端が飛び出した五角形の旗として描かれる。
    #[test]
    fn tag_right_points_to_the_right() {
        let tag = net_tag(&label("24V", Some("right"))).unwrap();
        assert_eq!(tag.dir, TagDir::Right);
        assert_eq!(tag.points.len(), 5, "五角形");
        let tip = tag.points[2];
        assert!(tip.x > 100.0, "先端は基準点の右: {tip:?}");
        assert!((tip.x - tag.right).abs() < 1e-9, "先端がグリフの右端");
        // 先端は上下の中央 (角の高さの真ん中) に来る
        let (top, bottom) = (tag.points[0].y, tag.points[4].y);
        assert!((tip.y - (top + bottom) / 2.0).abs() < 1e-9);
    }

    /// A net label with tag "left" is drawn as the mirrored flag, with the point on the left.
    /// タグ「左」のネットラベルは、先端が左にある左右反転した旗として描かれる。
    #[test]
    fn tag_left_points_to_the_left() {
        let tag = net_tag(&label("24V", Some("left"))).unwrap();
        assert_eq!(tag.dir, TagDir::Left);
        assert_eq!(tag.points.len(), 5);
        let tip = tag.points[0];
        assert!((tip.x - 100.0).abs() < 1e-9, "先端は基準点そのもの: {tip:?}");
        for p in &tag.points[1..] {
            assert!(p.x > tip.x, "本体は先端の右: {p:?}");
        }
    }

    /// The tag text always sits inside the flag, and the flag grows with the length of the net name.
    /// タグの文字は必ず旗の内側に収まり、旗の幅はネット名の長さに合わせて伸びる。
    #[test]
    fn tag_box_follows_the_text_width() {
        let short = net_tag(&label("A", Some("right"))).unwrap();
        let long = net_tag(&label("24V_CONTROL_1", Some("right"))).unwrap();
        assert!(long.right > short.right, "長い名前ほど広い");
        for tag in [&short, &long] {
            let left = tag.points[0].x;
            assert!(tag.text_at.x > left, "文字は枠の内側から始まる");
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
        let right = net_tag(&label("24V", Some("right"))).unwrap();
        let left = net_tag(&label("24V", Some("left"))).unwrap();
        let width = |t: &NetTag| t.right - t.points.iter().map(|p| p.x).fold(f64::MAX, f64::min);
        assert!((width(&right) - width(&left)).abs() < 1e-9);
    }

    /// A net label without the tag attribute keeps the plain text look it has always had.
    /// タグ属性の無いネットラベルは、これまでどおり文字だけの見た目のままになる。
    #[test]
    fn label_without_tag_attribute_stays_plain_text() {
        assert!(net_tag(&label("24V", None)).is_none());
        assert!(tag_dir(&label("24V", None)).is_none());
    }

    /// An unknown tag value (a typo, or "up") is ignored and the label falls back to plain text instead of failing.
    /// 知らないタグの値(打ち間違いや "up")は無視され、失敗せずに文字だけの表示に戻る。
    #[test]
    fn unknown_tag_value_is_ignored() {
        for value in ["up", "", "LEFT", "右"] {
            assert!(net_tag(&label("24V", Some(value))).is_none(), "{value}");
        }
    }
}
