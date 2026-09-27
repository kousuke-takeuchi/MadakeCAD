//! 図形エンティティ (G10) — 電気的な意味を持たない、純粋な作図要素。
//!
//! 筐体の囲み・強調枠・表のような注記の枠・塗りつぶしの機器ブロックは、どれも
//! 「図面の上では見えるが、回路としては何でもない」もの。これまではハーネス境界
//! ([`crate::harness`]) や配線を流用するしかなく、**電線リストのハーネス列が汚れる**
//! ・**ネットリストが配線でないものを導体として数える**という副作用があった (G11)。
//!
//! [`Shape`] は矩形・線・円・折れ線の4種で、ネットリスト・ERC・部品表・From-To表・
//! 整頓メトリクスのどれからも**必ず除外される**。色は注記と同じ**トークン名**
//! ([`crate::text_style::TEXT_COLORS`]) だけを受け付け、生の色コードは
//! [`crate::command::Engine::execute`] が拒否する。

use crate::geometry::Point;
use crate::model::{Entity, Shape, ShapeKind, Sheet};
use crate::text_style::{is_text_color_token, text_color_tokens, text_print_color};

/// 破線の線の長さと間隔 (mm)。ハーネス境界 (IEC 61082-1) と同じ刻みにして、
/// 図面上で破線の見え方がばらつかないようにする。
pub const SHAPE_DASH: (f64, f64) = (3.0, 2.0);

/// 線幅の既定値 (mm)。図枠の細線・ハーネス境界と同じ太さ。
pub const DEFAULT_SHAPE_STROKE_MM: f64 = 0.25;
/// 線幅の下限 (mm)。これより細いとA3に印刷したとき線が消える。
pub const MIN_SHAPE_STROKE_MM: f64 = 0.1;
/// 線幅の上限 (mm)。これより太いと2.5mmピッチの図面で隣の要素を潰す。
pub const MAX_SHAPE_STROKE_MM: f64 = 2.0;

/// 塗りの不透明度。**塗っても下の配線・記号が透けて読める**濃さにする
/// (図面の主情報は回路であって、塗りは背景の強調にすぎない)。
pub const SHAPE_FILL_OPACITY: f64 = 0.15;

/// 色を指定していない図形が使うトークン (注記の既定色と同じ)。
pub const DEFAULT_SHAPE_COLOR: &str = crate::text_style::DEFAULT_TEXT_COLOR;

/// 新しく作る図形の既定値 (ツールが使う)。
pub fn new_shape(kind: ShapeKind, points: Vec<Point>) -> Shape {
    Shape {
        id: uuid::Uuid::new_v4(),
        shape: kind,
        points,
        stroke: DEFAULT_SHAPE_COLOR.to_string(),
        stroke_width_mm: DEFAULT_SHAPE_STROKE_MM,
        fill: None,
        dashed: false,
    }
}

/// その種別が必要とする頂点の数 (`None`は「2点以上なら何点でもよい」= 折れ線)。
pub fn required_points(kind: ShapeKind) -> Option<usize> {
    match kind {
        ShapeKind::Rect | ShapeKind::Line | ShapeKind::Circle => Some(2),
        ShapeKind::Polyline => None,
    }
}

/// 図形が図面へ置ける形になっているか。エラーは人が読める理由の文字列。
///
/// - 頂点の数が種別に合っているか (矩形・線・円は2点ちょうど、折れ線は2点以上)
/// - 座標が有限の数か (NaN・無限大の図形は描けない)
/// - 線色・塗り色が**色トークン名**か (生の色コードは受け付けない)
/// - 線幅が [`MIN_SHAPE_STROKE_MM`]〜[`MAX_SHAPE_STROKE_MM`] mm か
pub fn validate(shape: &Shape) -> Result<(), String> {
    match required_points(shape.shape) {
        Some(n) if shape.points.len() != n => {
            return Err(format!(
                "shape {:?} needs exactly {n} points (got {})",
                shape.shape,
                shape.points.len()
            ));
        }
        None if shape.points.len() < 2 => {
            return Err(format!(
                "shape {:?} needs at least 2 points (got {})",
                shape.shape,
                shape.points.len()
            ));
        }
        _ => {}
    }
    if shape.points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        return Err("shape has a non-finite coordinate".to_string());
    }
    check_color("stroke", Some(shape.stroke.as_str()))?;
    check_color("fill", shape.fill.as_deref())?;
    if !shape.stroke_width_mm.is_finite()
        || shape.stroke_width_mm < MIN_SHAPE_STROKE_MM
        || shape.stroke_width_mm > MAX_SHAPE_STROKE_MM
    {
        return Err(format!(
            "shape stroke width out of range: {} (expected {MIN_SHAPE_STROKE_MM}..{MAX_SHAPE_STROKE_MM} mm)",
            shape.stroke_width_mm
        ));
    }
    Ok(())
}

fn check_color(field: &str, value: Option<&str>) -> Result<(), String> {
    let Some(token) = value else {
        return Ok(());
    };
    if is_text_color_token(token) {
        return Ok(());
    }
    let known: Vec<&str> = text_color_tokens().collect();
    Err(format!(
        "unknown shape {field} color token: {token:?} (expected one of {})",
        known.join(", ")
    ))
}

/// 図形の外接矩形 (左上, 右下)。円は中心±半径、他は頂点の最小・最大。頂点が無ければNone。
pub fn bounds(shape: &Shape) -> Option<(Point, Point)> {
    if shape.shape == ShapeKind::Circle {
        let (center, r) = circle(shape)?;
        return Some((
            Point::new(center.x - r, center.y - r),
            Point::new(center.x + r, center.y + r),
        ));
    }
    let mut it = shape.points.iter();
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

/// 矩形の正規化した対角 (左上, 右下)。矩形以外・頂点不足ならNone。
/// **どの向きにドラッグしても同じ矩形**になる。
pub fn rect_corners(shape: &Shape) -> Option<(Point, Point)> {
    if shape.shape != ShapeKind::Rect {
        return None;
    }
    let (a, b) = (shape.points.first()?, shape.points.get(1)?);
    Some((
        Point::new(a.x.min(b.x), a.y.min(b.y)),
        Point::new(a.x.max(b.x), a.y.max(b.y)),
    ))
}

/// 円の (中心, 半径mm)。**1点目が中心・2点目が円周上の点**。円以外・頂点不足ならNone。
pub fn circle(shape: &Shape) -> Option<(Point, f64)> {
    if shape.shape != ShapeKind::Circle {
        return None;
    }
    let (center, edge) = (shape.points.first()?, shape.points.get(1)?);
    Some((*center, center.distance_to(edge)))
}

/// シート上の全図形 (図面に置かれた順ではなくid順)。
pub fn shapes(sheet: &Sheet) -> impl Iterator<Item = &Shape> {
    sheet.entities.values().filter_map(|e| match e {
        Entity::Shape(s) => Some(s),
        _ => None,
    })
}

/// 線を紙へ描くときの色。知らない名前・未指定は既定色 (黒)。
pub fn stroke_print_color(shape: &Shape) -> &'static str {
    text_print_color(&shape.stroke)
        .unwrap_or_else(|| text_print_color(DEFAULT_SHAPE_COLOR).expect("既定色は必ずある"))
}

/// 塗りを紙へ描くときの色。塗り無し・知らない名前はNone (塗らない)。
pub fn fill_print_color(shape: &Shape) -> Option<&'static str> {
    shape.fill.as_deref().and_then(text_print_color)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Orientation, PaperSize};

    fn shape(kind: ShapeKind, points: &[(f64, f64)]) -> Shape {
        new_shape(
            kind,
            points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
        )
    }

    /// A new shape starts in the default colour, with a hairline stroke, no fill and a solid line, so a rectangle drawn without touching any property looks exactly like the frame lines.
    /// 新しい図形は既定色・細線・塗りなし・実線で始まるので、プロパティを何も触らずに引いた矩形は図枠の線と同じ見た目になる。
    #[test]
    fn a_new_shape_starts_plain() {
        let rect = shape(ShapeKind::Rect, &[(10.0, 10.0), (50.0, 30.0)]);
        assert_eq!(rect.stroke, "default");
        assert_eq!(rect.stroke_width_mm, DEFAULT_SHAPE_STROKE_MM);
        assert_eq!(rect.fill, None);
        assert!(!rect.dashed);
        assert_eq!(validate(&rect), Ok(()));
    }

    /// A rectangle is two diagonal corners, and it normalises to the same rectangle whichever way it was dragged.
    /// 矩形は対角の2点で表し、どの向きにドラッグしても同じ矩形へ正規化される。
    #[test]
    fn a_rectangle_is_two_diagonal_corners() {
        let forward = shape(ShapeKind::Rect, &[(10.0, 20.0), (50.0, 40.0)]);
        let backward = shape(ShapeKind::Rect, &[(50.0, 40.0), (10.0, 20.0)]);
        let expected = (Point::new(10.0, 20.0), Point::new(50.0, 40.0));
        assert_eq!(rect_corners(&forward), Some(expected));
        assert_eq!(rect_corners(&backward), Some(expected));
        assert_eq!(bounds(&forward), Some(expected));
    }

    /// A circle is a centre point plus a point on its edge, so its radius is the distance between the two.
    /// 円は中心の点と円周上の点で表すので、半径はその2点の距離になる。
    #[test]
    fn a_circle_is_a_centre_and_a_point_on_its_edge() {
        let c = shape(ShapeKind::Circle, &[(100.0, 50.0), (100.0, 60.0)]);
        let (center, r) = circle(&c).expect("円である");
        assert_eq!(center, Point::new(100.0, 50.0));
        assert!((r - 10.0).abs() < 1e-9, "{r}");
        assert_eq!(
            bounds(&c),
            Some((Point::new(90.0, 40.0), Point::new(110.0, 60.0))),
            "外接矩形は中心±半径"
        );
    }

    /// A polyline keeps every point it was drawn with, in order.
    /// 折れ線は描いた順に全ての点をそのまま持つ。
    #[test]
    fn a_polyline_keeps_every_point() {
        let p = shape(ShapeKind::Polyline, &[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]);
        assert_eq!(p.points.len(), 3);
        assert_eq!(validate(&p), Ok(()));
        assert_eq!(bounds(&p), Some((Point::new(0.0, 0.0), Point::new(10.0, 10.0))));
        assert_eq!(rect_corners(&p), None, "折れ線に矩形の対角は無い");
        assert_eq!(circle(&p), None, "折れ線に中心・半径は無い");
    }

    /// A rectangle, line or circle with anything other than two points is rejected, and a polyline needs at least two.
    /// 矩形・線・円は2点ちょうどでなければ拒否され、折れ線は2点以上が必要である。
    #[test]
    fn the_number_of_points_must_match_the_shape_kind() {
        for kind in [ShapeKind::Rect, ShapeKind::Line, ShapeKind::Circle] {
            assert_eq!(required_points(kind), Some(2));
            assert!(validate(&shape(kind, &[(0.0, 0.0)])).is_err(), "{kind:?}");
            assert!(
                validate(&shape(kind, &[(0.0, 0.0), (1.0, 1.0), (2.0, 2.0)])).is_err(),
                "{kind:?}"
            );
            assert_eq!(validate(&shape(kind, &[(0.0, 0.0), (1.0, 1.0)])), Ok(()));
        }
        assert_eq!(required_points(ShapeKind::Polyline), None);
        assert!(validate(&shape(ShapeKind::Polyline, &[(0.0, 0.0)])).is_err());
        assert_eq!(validate(&shape(ShapeKind::Polyline, &[(0.0, 0.0), (1.0, 0.0)])), Ok(()));
    }

    /// A shape colour is a named token, never a raw colour code, so a shape can never print in a colour the paper cannot show.
    /// 図形の色は名前付きトークンだけで生の色コードは使えないので、紙に出せない色の図形が生まれることはない。
    #[test]
    fn a_shape_colour_must_be_a_token() {
        let mut s = shape(ShapeKind::Rect, &[(0.0, 0.0), (10.0, 10.0)]);
        for token in ["default", "accent", "warning", "success", "muted"] {
            s.stroke = token.to_string();
            s.fill = Some(token.to_string());
            assert_eq!(validate(&s), Ok(()), "{token}");
        }
        for bad in ["#ff0000", "red", "赤", ""] {
            s.stroke = bad.to_string();
            s.fill = None;
            assert!(validate(&s).is_err(), "stroke {bad:?}");
            s.stroke = "default".into();
            s.fill = Some(bad.to_string());
            assert!(validate(&s).is_err(), "fill {bad:?}");
        }
    }

    /// The stroke width has to stay between the thinnest line A3 can print and the thickest line that does not swamp a 2.5 mm grid.
    /// 線幅は、A3に印刷できる最も細い線と、2.5mmグリッドを潰さない最も太い線の間でなければならない。
    #[test]
    fn the_stroke_width_has_a_printable_range() {
        let mut s = shape(ShapeKind::Line, &[(0.0, 0.0), (10.0, 0.0)]);
        for width in [MIN_SHAPE_STROKE_MM, 0.35, 1.0, MAX_SHAPE_STROKE_MM] {
            s.stroke_width_mm = width;
            assert_eq!(validate(&s), Ok(()), "{width}");
        }
        for width in [0.0, 0.05, 2.5, -1.0, f64::NAN] {
            s.stroke_width_mm = width;
            assert!(validate(&s).is_err(), "{width}");
        }
    }

    /// A coordinate that is not a real number is rejected, so a broken shape can never make the drawing unpaintable.
    /// 実数でない座標は拒否されるので、壊れた図形が図面を描けなくすることはない。
    #[test]
    fn a_non_finite_coordinate_is_rejected() {
        let s = shape(ShapeKind::Line, &[(0.0, 0.0), (f64::INFINITY, 0.0)]);
        assert!(validate(&s).is_err());
    }

    /// Shapes print in the colour their token stands for, and a shape with no fill is not painted at all.
    /// 図形はトークンが表す色で刷られ、塗りを指定していない図形は塗られない。
    #[test]
    fn shapes_print_in_their_token_colour() {
        let mut s = shape(ShapeKind::Rect, &[(0.0, 0.0), (10.0, 10.0)]);
        assert_eq!(stroke_print_color(&s), "#000");
        assert_eq!(fill_print_color(&s), None);
        s.stroke = "accent".into();
        s.fill = Some("warning".into());
        assert_eq!(stroke_print_color(&s), "#0000c0");
        assert_eq!(fill_print_color(&s), Some("#c00000"));
    }

    /// 回路が1つ載ったシート (シンボル1個 + 配線1本)。図形を足す前後の比較に使う。
    fn circuit_sheet() -> Sheet {
        let mut sheet = Sheet::new("S1", PaperSize::A3, Orientation::Landscape);
        let coil = Entity::Symbol(crate::model::SymbolInstance {
            id: uuid::Uuid::new_v4(),
            symbol_id: "relay_coil".into(),
            at: Point::new(100.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "K1".into(),
            value: "MDK-RLY-24V".into(),
            attrs: Default::default(),
        });
        let wire = Entity::Wire(crate::model::Wire {
            id: uuid::Uuid::new_v4(),
            points: vec![Point::new(60.0, 60.0), Point::new(140.0, 60.0)],
            color: "red".into(),
            sq: 0.75,
            length_m: Some(0.4),
            part_no: None,
            net: None,
            length_source: crate::model::LengthSource::Manual,
        });
        for e in [coil, wire] {
            sheet.entities.insert(e.id(), e);
        }
        sheet
    }

    /// 囲みの図形を4つ (矩形・線・円・折れ線) 足す。座標はわざとグリッドから外す。
    fn add_every_shape(sheet: &mut Sheet) {
        for e in [
            shape(ShapeKind::Rect, &[(50.0, 50.0), (150.0, 100.0)]),
            shape(ShapeKind::Line, &[(60.0, 60.0), (140.0, 60.0)]),
            shape(ShapeKind::Circle, &[(100.0, 50.0), (110.0, 50.0)]),
            shape(ShapeKind::Polyline, &[(51.3, 47.7), (60.0, 60.0), (140.0, 60.0)]),
        ] {
            let e = Entity::Shape(e);
            sheet.entities.insert(e.id(), e);
        }
    }

    /// Shapes carry no electrical meaning: adding them changes neither the netlist nor the ERC result, even when a shape lies exactly on top of a wire.
    /// 図形は電気的な意味を持たない: 配線の真上に重ねて描いても、ネットリストも検証結果も変わらない。
    #[test]
    fn shapes_are_invisible_to_the_netlist_and_the_checks() {
        let symbols = crate::symbol::builtin_symbols();
        let plain = circuit_sheet();
        let nets_before = crate::netlist::extract_netlist(&plain, &symbols).len();
        let diagnostics_before = crate::verify::verify_sheet(&plain, &symbols).len();

        let mut drawn = plain.clone();
        add_every_shape(&mut drawn);
        assert_eq!(
            crate::netlist::extract_netlist(&drawn, &symbols).len(),
            nets_before,
            "図形はネットを増やさない"
        );
        assert_eq!(
            crate::verify::verify_sheet(&drawn, &symbols).len(),
            diagnostics_before,
            "図形は検証の指摘を増やさない"
        );
    }

    /// Shapes never appear in the parts list or the wire list, and a wire inside a shape keeps an empty harness column — which is why an enclosure is drawn with a shape and not with a harness boundary (G11).
    /// 図形は部品表にも電線リストにも出ず、図形の中に入った配線のハーネス列は空のままである (筐体の囲みをハーネスではなく図形で描く理由)。
    #[test]
    fn shapes_never_reach_the_parts_list_or_the_wire_list() {
        let mut project = crate::model::Project::new("shapes");
        project.sheets = vec![circuit_sheet()];
        let bom_before = crate::reports::bom_rows(&project);
        let wires_before = crate::reports::wire_list_rows(&project);

        add_every_shape(&mut project.sheets[0]);
        assert_eq!(crate::reports::bom_rows(&project), bom_before, "部品表は変わらない");
        let wires = crate::reports::wire_list_rows(&project);
        assert_eq!(wires.len(), wires_before.len(), "電線リストの行数は変わらない");
        assert_eq!(wires, wires_before, "電線リストの中身も変わらない");
        for row in &wires {
            assert_eq!(row.last().map(String::as_str), Some(""), "ハーネス列は空: {row:?}");
        }
    }

    /// Shapes are left out of the tidy metrics, so drawing an annotation off the grid never counts against the drawing.
    /// 図形は整頓メトリクスの対象外なので、注記の枠をグリッドから外して描いても図面の点数は下がらない。
    #[test]
    fn shapes_are_left_out_of_the_tidy_metrics() {
        let symbols = crate::symbol::builtin_symbols();
        let plain = circuit_sheet();
        let before = crate::tidy::tidy_metrics(&plain, &symbols);
        let mut drawn = plain.clone();
        add_every_shape(&mut drawn);
        let after = crate::tidy::tidy_metrics(&drawn, &symbols);
        assert_eq!(after.total(), before.total());
        assert_eq!(after.off_grid, before.off_grid, "グリッド外の図形は数えない");
        assert_eq!(after.crossings, before.crossings, "配線に重ねた線は交差でない");
    }

    /// Listing the shapes of a sheet returns only shapes, never wires, notes or harness boundaries.
    /// シートの図形を数えると図形だけが返り、配線・注記・ハーネス境界は混ざらない。
    #[test]
    fn only_shapes_are_listed() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let rect = shape(ShapeKind::Rect, &[(0.0, 0.0), (10.0, 10.0)]);
        let line = shape(ShapeKind::Line, &[(0.0, 0.0), (10.0, 0.0)]);
        for e in [Entity::Shape(rect.clone()), Entity::Shape(line.clone())] {
            sheet.entities.insert(e.id(), e);
        }
        sheet.entities.insert(
            uuid::Uuid::nil(),
            Entity::Harness(crate::model::Harness {
                id: uuid::Uuid::nil(),
                points: vec![Point::new(0.0, 0.0), Point::new(1.0, 1.0)],
                name: "W1".into(),
                note: String::new(),
            }),
        );
        let ids: Vec<_> = shapes(&sheet).map(|s| s.id).collect();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&rect.id) && ids.contains(&line.id));
    }
}
