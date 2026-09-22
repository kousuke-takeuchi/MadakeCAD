//! 注記の引き出し線 (G20)。
//!
//! `TextEntity.leader_to`に対象のエンティティIDが入っていれば、注記の外接矩形のうち
//! **対象にいちばん近い辺の中点**から、**対象の外接矩形上のいちばん近い点**まで直線を引く
//! (対象端には配線の分岐点と同じ大きさの丸印)。位置は保存せず描画のたびに求め直すので、
//! 注記か対象のどちらを動かしても線は追従する。
//!
//! ここは純関数だけを置く (モデルは変更しない)。SVG/PDF出力 (`svg.rs`) とcanvas描画
//! (`src/canvas/leader.ts`) の双方がこの1か所のジオメトリを使う。
//!
//! **対象の外接矩形は[`crate::svg::symbol_bounds`]1つだけを使う** (canvas側の双子
//! `dynamicSymbol.ts`の`symbolBounds`と同じ定義: 配置基準点を含め、文字プリミティブは
//! 基準点だけを見る)。図面に描かれる外形と、引き出し線が指す先を必ず一致させるため。

use crate::model::{Entity, Sheet, TextAnchor, TextEntity};
use crate::symbol::DefIndex;
use crate::tidy::{text_width, Rect};
use crate::Point;

/// 注記の外接矩形 (用紙座標)。基準位置 (`anchor`) に応じて左揃え・中央揃え・右揃えで求める。
pub fn text_bounds(text: &TextEntity) -> Rect {
    let w = text_width(&text.text, text.height);
    let x0 = match text.anchor {
        TextAnchor::Start => text.at.x,
        TextAnchor::Middle => text.at.x - w / 2.0,
        TextAnchor::End => text.at.x - w,
    };
    Rect::from_corners(Point::new(x0, text.at.y - text.height), Point::new(x0 + w, text.at.y))
}

/// 任意のエンティティの外接矩形 (用紙座標)。求まらない種類 (盤レイアウト専用など) や
/// 頂点の無い図形はNone。
pub fn entity_bounds(entity: &Entity, defs: &DefIndex) -> Option<Rect> {
    match entity {
        Entity::Symbol(s) => {
            let def = defs.get(s)?;
            // 描画側と同じ外形 (canvas双子 dynamicSymbol.ts の symbolBounds と同一定義)
            let (min, max) = crate::svg::symbol_bounds(s, &def);
            Some(Rect::from_corners(min, max))
        }
        Entity::Text(t) => Some(text_bounds(t)),
        Entity::Wire(w) => bounds_of_points(&w.points),
        Entity::Junction(j) => Some(Rect::from_corners(j.at, j.at)),
        Entity::NetLabel(l) => Some(Rect::from_corners(l.at, l.at)),
        Entity::Harness(h) => {
            crate::harness::bounds(h).map(|(a, b)| Rect::from_corners(a, b))
        }
        Entity::Shape(sh) => crate::shape::bounds(sh).map(|(a, b)| Rect::from_corners(a, b)),
        // 盤レイアウト専用エンティティは回路図シートには載らないので対象になり得ない
        Entity::PanelOutline(_)
        | Entity::DinRail(_)
        | Entity::Duct(_)
        | Entity::Footprint(_)
        | Entity::PanelLegend(_) => None,
    }
}

fn bounds_of_points(points: &[Point]) -> Option<Rect> {
    let mut it = points.iter();
    let first = *it.next()?;
    let mut rect = Rect::from_corners(first, first);
    for p in it {
        rect.extend(*p);
    }
    Some(rect)
}

/// 矩形上で`p`にいちばん近い点 (`p`が内側ならそのもの)。
fn nearest_point_on_rect(rect: &Rect, p: Point) -> Point {
    Point::new(
        p.x.clamp(rect.min.x, rect.max.x),
        p.y.clamp(rect.min.y, rect.max.y),
    )
}

/// 矩形の4辺それぞれの中点。
fn edge_midpoints(rect: &Rect) -> [Point; 4] {
    let cx = (rect.min.x + rect.max.x) / 2.0;
    let cy = (rect.min.y + rect.max.y) / 2.0;
    [
        Point::new(cx, rect.min.y),       // 上辺
        Point::new(cx, rect.max.y),       // 下辺
        Point::new(rect.min.x, cy),       // 左辺
        Point::new(rect.max.x, cy),       // 右辺
    ]
}

/// 注記の外接矩形から対象の外接矩形への引き出し線 (始点, 終点)。
///
/// 始点は注記の4辺の中点のうち、対象の外接矩形にいちばん近いもの。終点はそこから対象の
/// 外接矩形上のいちばん近い点 (対象の内側に注記の辺が食い込んでいれば、始点そのもの)。
/// 注記と対象が同じ矩形になる (重なって区別が付かない) ときは始点=終点になる。
pub fn leader_line(text_rect: &Rect, target_rect: &Rect) -> (Point, Point) {
    let start = edge_midpoints(text_rect)
        .into_iter()
        .map(|m| (m, nearest_point_on_rect(target_rect, m)))
        .min_by(|(m1, e1), (m2, e2)| {
            let d1 = m1.distance_to(e1);
            let d2 = m2.distance_to(e2);
            d1.partial_cmp(&d2).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(m, _)| m)
        .expect("矩形は常に4辺を持つ");
    let end = nearest_point_on_rect(target_rect, start);
    (start, end)
}

/// シート上の注記1つぶんの引き出し線ジオメトリ。`leader_to`が対象を指していない・
/// 対象が求まらないときはNone (描画側はパニックせず何も描かない)。
pub fn leader_for(sheet: &Sheet, text: &TextEntity, defs: &DefIndex) -> Option<(Point, Point)> {
    let target_id = text.leader_to?;
    let target = sheet.entities.get(&target_id)?;
    let target_rect = entity_bounds(target, defs)?;
    let text_rect = text_bounds(text);
    Some(leader_line(&text_rect, &target_rect))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Entity, Junction, Project, SymbolInstance, TextEntity};
    use crate::symbol::SymbolDef;
    use uuid::Uuid;

    fn note(x: f64, y: f64, text: &str, leader_to: Option<crate::model::EntityId>) -> TextEntity {
        TextEntity {
            id: Uuid::new_v4(),
            at: Point::new(x, y),
            text: text.into(),
            height: 2.5,
            rotation: 0,
            anchor: TextAnchor::Start,
            color: None,
            bold: false,
            leader_to,
        }
    }

    /// A leader line drawn from a note above its target starts at the midpoint of the note's bottom edge and ends at the nearest point on the target's top edge.
    /// 対象の真上にある注記から引く引き出し線は、注記の下辺の中点から始まり、対象の上辺のいちばん近い点で終わる。
    #[test]
    fn a_leader_above_its_target_runs_from_the_bottom_edge_to_the_top() {
        let text = note(100.0, 50.0, "注意", None);
        let text_rect = text_bounds(&text);
        let target_rect = Rect::from_corners(Point::new(90.0, 60.0), Point::new(110.0, 80.0));
        let (start, end) = leader_line(&text_rect, &target_rect);
        assert_eq!(start.y, text_rect.max.y, "始点は注記の下辺の上");
        assert_eq!(end.y, target_rect.min.y, "終点は対象の上辺の上");
    }

    /// A leader line drawn from a note beside its target runs horizontally from the note's near vertical edge to the target's bounding box.
    /// 対象の真横にある注記から引く引き出し線は、注記の近いほうの縦辺から対象の外接矩形まで水平に伸びる。
    #[test]
    fn a_leader_beside_its_target_runs_horizontally() {
        let text = note(0.0, 50.0, "注意", None);
        let text_rect = text_bounds(&text); // x in [0, w], y in [47.5, 50]
        let target_rect = Rect::from_corners(Point::new(50.0, 40.0), Point::new(70.0, 60.0));
        let (start, end) = leader_line(&text_rect, &target_rect);
        assert_eq!(start.x, text_rect.max.x, "始点は注記の右辺の上 (対象は右にある)");
        assert_eq!(end.x, target_rect.min.x, "終点は対象の左辺の上");
    }

    /// leader_for follows a note that is moved: recomputing after the note's position changes yields a different geometry without touching the model.
    /// leader_forは注記の移動に追従する: 位置を変えて求め直すとモデルを書き換えずに違うジオメトリが返る。
    #[test]
    fn leader_for_recomputes_after_the_note_moves() {
        let mut project = Project::new("leader");
        let target_id = Uuid::new_v4();
        let target = Entity::Junction(Junction {
            id: target_id,
            at: Point::new(100.0, 100.0),
        });
        project.sheets[0].entities.insert(target.id(), target);

        let mut text = note(0.0, 0.0, "対象", Some(target_id));
        let defs = DefIndex::new(&[]);
        let sheet = &project.sheets[0];
        let before = leader_for(sheet, &text, &defs).expect("対象が実在するので線が引ける");

        text.at = Point::new(200.0, 200.0);
        let after = leader_for(sheet, &text, &defs).expect("移動後も対象は変わらず実在する");
        assert_ne!(before, after, "注記を動かせば引き出し線も変わる");
    }

    /// leader_for returns None when leader_to is unset, so an ordinary note draws no leader line.
    /// leader_forはleader_to未設定ならNoneを返す。普通の注記は引き出し線を描かない。
    #[test]
    fn leader_for_is_none_without_a_leader_target() {
        let project = Project::new("leader");
        let sheet = &project.sheets[0];
        let text = note(0.0, 0.0, "普通の注記", None);
        let defs = DefIndex::new(&[]);
        assert!(leader_for(sheet, &text, &defs).is_none());
    }

    /// A leader aimed at a symbol ends on exactly the outline the drawing shows: the placement point is always inside it, and a text mark inside the symbol counts as its own reference point, not as a box of letters.
    /// シンボルを指す引き出し線は、図面に描かれている外形ちょうどで止まる: 配置基準点は必ず外形の内側に入り、シンボルの中の文字は文字の箱ではなく基準点1点として数える。
    #[test]
    fn a_leader_targets_exactly_the_outline_the_drawing_shows() {
        // 外形が配置基準点を囲まず、文字が外形の外にはみ出す定義 (2つの数え方で差が出る形)
        let def = SymbolDef {
            id: "leader_probe".into(),
            name: "Probe".into(),
            name_ja: "試験".into(),
            category: "misc".into(),
            ref_prefix: "X".into(),
            keywords: vec![],
            primitives: vec![
                crate::symbol::Primitive::Rect {
                    p1: Point::new(10.0, 10.0),
                    p2: Point::new(20.0, 20.0),
                    filled: false,
                },
                crate::symbol::Primitive::Text {
                    at: Point::new(30.0, 0.0),
                    text: "XYZ".into(),
                    height: 2.5,
                },
            ],
            pins: vec![],
            text_slots: vec![],
        };
        let inst = Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "leader_probe".into(),
            at: Point::new(100.0, 100.0),
            rotation: 0,
            mirror: false,
            reference: "X1".into(),
            value: String::new(),
            attrs: Default::default(),
        });
        let defs = DefIndex::new(std::slice::from_ref(&def));
        let rect = entity_bounds(&inst, &defs).expect("外形が求まる");
        assert_eq!(rect.min, Point::new(100.0, 100.0), "配置基準点が外形に入る");
        assert_eq!(rect.max, Point::new(130.0, 120.0), "文字は基準点1点ぶんだけ広がる");
    }

    /// entity_bounds resolves a symbol instance's bounding box from its definition, so a leader can target a symbol.
    /// entity_boundsはシンボルの外形をその定義から求めるので、引き出し線はシンボルも対象にできる。
    #[test]
    fn entity_bounds_resolves_a_symbol_instance() {
        let inst = Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "relay_coil".into(),
            at: Point::new(50.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "K1".into(),
            value: String::new(),
            attrs: Default::default(),
        });
        let def = crate::symbol::resolve_symbol("relay_coil").unwrap();
        let defs = DefIndex::new(std::slice::from_ref(&def));
        assert!(entity_bounds(&inst, &defs).is_some());
    }
}
