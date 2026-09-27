//! ハーネス境界 (IEC 61082-1 のグループ囲み)。
//!
//! ハーネスは「まとめて製作・購入する電線束」の範囲を破線で囲んだもので、
//! [`crate::model::Harness`] エンティティとして図面に置く。どのワイヤがその束に
//! 属するかは**幾何学的な内包**だけで決まる (明示割当は将来)。囲みからはみ出す
//! ワイヤは、一部が入っていても所属しない。

use crate::geometry::Point;
use crate::model::{Entity, Harness, Sheet, Wire};

/// 境界線上の点を「内側」と扱うための許容誤差 (mm)。
/// 2.5mmグリッド上の点が浮動小数の誤差で外に落ちるのを防ぐ。
const EPS: f64 = 1e-9;

/// 囲みの外接矩形 (左上, 右下)。頂点が無ければNone。
pub fn bounds(harness: &Harness) -> Option<(Point, Point)> {
    let mut it = harness.points.iter();
    let first = it.next()?;
    let (mut min, mut max) = (*first, *first);
    for p in it {
        min.x = min.x.min(p.x);
        min.y = min.y.min(p.y);
        max.x = max.x.max(p.x);
        max.y = max.y.max(p.y);
    }
    Some((min, max))
}

/// 点が囲みの内側 (境界線上を含む) にあるか。
pub fn contains_point(harness: &Harness, p: &Point) -> bool {
    let Some((min, max)) = bounds(harness) else {
        return false;
    };
    p.x >= min.x - EPS && p.x <= max.x + EPS && p.y >= min.y - EPS && p.y <= max.y + EPS
}

/// ワイヤが囲みに所属するか。**全ての点**が内側 (境界線上を含む) のときだけ所属する。
/// 一部だけ入っているワイヤ・点の無いワイヤは所属しない。
pub fn contains_wire(harness: &Harness, wire: &Wire) -> bool {
    !wire.points.is_empty() && wire.points.iter().all(|p| contains_point(harness, p))
}

/// シート上の全ハーネス (図面に置かれた順ではなくid順)。
pub fn harnesses(sheet: &Sheet) -> impl Iterator<Item = &Harness> {
    sheet.entities.values().filter_map(|e| match e {
        Entity::Harness(h) => Some(h),
        _ => None,
    })
}

/// ワイヤが所属するハーネス。入れ子の囲みでは最も小さい (内側の) ハーネスを選ぶ。
pub fn harness_of_wire<'a>(sheet: &'a Sheet, wire: &Wire) -> Option<&'a Harness> {
    harnesses(sheet)
        .filter(|h| contains_wire(h, wire))
        .min_by(|a, b| {
            area(a)
                .partial_cmp(&area(b))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.id.cmp(&b.id))
        })
}

/// ワイヤが所属するハーネスの名前 (所属しない・名前が空なら空文字)。電線リストの列に使う。
pub fn harness_name_of_wire(sheet: &Sheet, wire: &Wire) -> String {
    harness_of_wire(sheet, wire)
        .map(|h| h.name.clone())
        .unwrap_or_default()
}

/// 囲みの面積 (mm2)。入れ子の優先順位に使う。
fn area(harness: &Harness) -> f64 {
    match bounds(harness) {
        Some((min, max)) => (max.x - min.x) * (max.y - min.y),
        None => 0.0,
    }
}

/// ハーネスに含まれるワイヤの本数 (プロパティパネルの「含む電線」表示用)。
pub fn wire_count(sheet: &Sheet, harness: &Harness) -> usize {
    sheet
        .entities
        .values()
        .filter(|e| match e {
            Entity::Wire(w) => harness_of_wire(sheet, w).map(|h| h.id) == Some(harness.id),
            _ => false,
        })
        .count()
}

/// ハーネス名の接頭辞。参照記号と同じ命名規則 (W1, W2 …)。
pub const HARNESS_PREFIX: &str = "W";

/// 次に使うハーネス名 (既存の W番号 の最大+1)。1件も無ければ "W1"。
pub fn next_harness_name(sheet: &Sheet) -> String {
    let max = harnesses(sheet)
        .filter_map(|h| h.name.strip_prefix(HARNESS_PREFIX))
        .filter_map(|n| n.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    format!("{HARNESS_PREFIX}{}", max + 1)
}

/// 2点のドラッグから矩形の4頂点 (左上→右上→右下→左下) を作る。
pub fn rect_points(a: Point, b: Point) -> Vec<Point> {
    let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
    let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
    vec![
        Point::new(x0, y0),
        Point::new(x1, y0),
        Point::new(x1, y1),
        Point::new(x0, y1),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use uuid::Uuid;

    fn harness(name: &str, x0: f64, y0: f64, x1: f64, y1: f64) -> Harness {
        Harness {
            id: Uuid::new_v4(),
            points: rect_points(Point::new(x0, y0), Point::new(x1, y1)),
            name: name.into(),
            note: String::new(),
        }
    }

    fn wire(points: &[(f64, f64)]) -> Wire {
        Wire {
            id: Uuid::new_v4(),
            points: points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            color: "red".into(),
            sq: 0.75,
            length_m: None,
            length_source: Default::default(),
            part_no: None,
            net: None,
        }
    }

    fn sheet_with(entities: Vec<Entity>) -> Sheet {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        for e in entities {
            sheet.entities.insert(e.id(), e);
        }
        sheet
    }

    /// A wire belongs to a harness only when every one of its points is inside the boundary.
    /// ワイヤは、その全ての点が囲みの内側にあるときだけハーネスに所属する。
    #[test]
    fn a_fully_enclosed_wire_belongs_to_the_harness() {
        let h = harness("W1", 50.0, 50.0, 150.0, 100.0);
        assert!(contains_wire(&h, &wire(&[(60.0, 60.0), (140.0, 60.0)])));
    }

    /// A wire that touches the boundary line is still counted as inside (the line itself belongs to the harness).
    /// 境界線上に乗っているワイヤも内側として扱う (境界線そのものはハーネスに含まれる)。
    #[test]
    fn a_wire_on_the_boundary_line_still_belongs() {
        let h = harness("W1", 50.0, 50.0, 150.0, 100.0);
        assert!(contains_wire(&h, &wire(&[(50.0, 50.0), (150.0, 100.0)])));
    }

    /// A wire that only partly overlaps the boundary does not belong to the harness.
    /// 囲みに一部だけ入っているワイヤはハーネスに所属しない。
    #[test]
    fn a_partly_overlapping_wire_does_not_belong() {
        let h = harness("W1", 50.0, 50.0, 150.0, 100.0);
        assert!(!contains_wire(&h, &wire(&[(60.0, 60.0), (200.0, 60.0)])));
    }

    /// A wire drawn completely outside the boundary does not belong to the harness.
    /// 囲みの外に描かれたワイヤはハーネスに所属しない。
    #[test]
    fn a_wire_outside_the_boundary_does_not_belong() {
        let h = harness("W1", 50.0, 50.0, 150.0, 100.0);
        assert!(!contains_wire(&h, &wire(&[(200.0, 200.0), (250.0, 200.0)])));
    }

    /// Looking up the harness of a wire returns the harness name, or an empty string when it belongs to none.
    /// ワイヤの所属ハーネスを引くと名前が返り、どこにも属さないワイヤでは空文字になる。
    #[test]
    fn harness_name_lookup_is_empty_for_unassigned_wires() {
        let inside = wire(&[(60.0, 60.0), (140.0, 60.0)]);
        let outside = wire(&[(200.0, 200.0), (250.0, 200.0)]);
        let sheet = sheet_with(vec![
            Entity::Harness(harness("W1", 50.0, 50.0, 150.0, 100.0)),
            Entity::Wire(inside.clone()),
            Entity::Wire(outside.clone()),
        ]);
        assert_eq!(harness_name_of_wire(&sheet, &inside), "W1");
        assert_eq!(harness_name_of_wire(&sheet, &outside), "");
    }

    /// When harnesses are nested, a wire belongs to the smallest boundary that encloses it.
    /// 囲みが入れ子になっているときは、そのワイヤを囲む最も小さいハーネスに所属する。
    #[test]
    fn a_nested_harness_wins_over_the_outer_one() {
        let inner_wire = wire(&[(60.0, 60.0), (70.0, 60.0)]);
        let sheet = sheet_with(vec![
            Entity::Harness(harness("W1", 50.0, 50.0, 200.0, 150.0)),
            Entity::Harness(harness("W2", 55.0, 55.0, 80.0, 70.0)),
            Entity::Wire(inner_wire.clone()),
        ]);
        assert_eq!(harness_name_of_wire(&sheet, &inner_wire), "W2");
    }

    /// The wire count of a harness reports how many wires it currently encloses.
    /// ハーネスの「含む電線」本数は、その囲みが今いくつのワイヤを囲んでいるかを表す。
    #[test]
    fn wire_count_reports_the_enclosed_wires() {
        let h = harness("W1", 50.0, 50.0, 150.0, 100.0);
        let sheet = sheet_with(vec![
            Entity::Harness(h.clone()),
            Entity::Wire(wire(&[(60.0, 60.0), (140.0, 60.0)])),
            Entity::Wire(wire(&[(60.0, 80.0), (140.0, 80.0)])),
            Entity::Wire(wire(&[(200.0, 200.0), (250.0, 200.0)])),
        ]);
        assert_eq!(wire_count(&sheet, &h), 2);
    }

    /// A new harness is named W1 on an empty sheet and takes the next free number after existing ones.
    /// 新しいハーネスの名前は、何も無いシートではW1、既にあるときはその次の番号になる。
    #[test]
    fn the_next_harness_name_continues_the_w_series() {
        let empty = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        assert_eq!(next_harness_name(&empty), "W1");
        let sheet = sheet_with(vec![
            Entity::Harness(harness("W1", 0.0, 0.0, 10.0, 10.0)),
            Entity::Harness(harness("W3", 20.0, 0.0, 30.0, 10.0)),
        ]);
        assert_eq!(next_harness_name(&sheet), "W4");
    }

    /// A rectangle drag produces four corner points regardless of the direction it was dragged in.
    /// 矩形ドラッグはどの向きに引いても同じ4隅の頂点になる。
    #[test]
    fn rect_points_normalize_the_drag_direction() {
        let forward = rect_points(Point::new(10.0, 20.0), Point::new(50.0, 40.0));
        let backward = rect_points(Point::new(50.0, 40.0), Point::new(10.0, 20.0));
        assert_eq!(forward, backward);
        assert_eq!(forward.len(), 4);
        assert_eq!(forward[0], Point::new(10.0, 20.0));
        assert_eq!(forward[2], Point::new(50.0, 40.0));
    }
}
