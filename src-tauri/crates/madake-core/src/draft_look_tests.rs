//! 機能シートの下書きの見た目の決まり (棒・短い線の長さ・札の収まり・列の詰め方) のテスト。
//! 元の図は、この中で組み立てる見本 [`bars_source`] (機器 UA UB UC。名前は全て作り物)。
use super::*;
use crate::model::{NetLabel, Wire};
use crate::netlist::{extract_netlist, pin_positions};
use crate::route::RouteOptions;
use crate::symbol::builtin_symbols;
use crate::tidy::{text_boxes, TextKind};
use std::collections::BTreeSet;
use uuid::Uuid;

// ---- the source drawing, built in code

const UA_PINS: &str = r#"{"left":[
 {"no":"1","name":"G1"},{"no":"2","name":"G2"},{"no":"3","name":"G3"},
 {"no":"4","name":"P1"},{"no":"5","name":"P2"},
 {"no":"6","name":"J1"},{"no":"7","name":"J2"},
 {"no":"8","name":"F1"},{"no":"9","name":"F2"},
 {"no":"10","name":"B1"},{"no":"11","name":"B2"},
 {"no":"12","name":"S1"},{"no":"13","name":"S2"},{"no":"14","name":"S3"},{"no":"15","name":"S4"}]}"#;
const UB_PINS: &str = r#"{"left":[
 {"no":"P24A_2","name":"G"},{"no":"1","name":"D1"},{"no":"2","name":"D2"},{"no":"3","name":"D3"},
 {"no":"4","name":"B1"},{"no":"5","name":"B2"},{"no":"6","name":"IN"},{"no":"7","name":"P"}]}"#;
const UC_PINS: &str = r#"{"left":[{"no":"1","name":"FEED"}]}"#;

fn block(reference: &str, x: f64, y: f64, pins_json: &str, size: &str) -> SymbolInstance {
    let Entity::Symbol(mut s) = crate::block::test_block(reference, "", x, y, pins_json) else { unreachable!() };
    s.attrs.insert(crate::block::BLOCK_SIZE_ATTR.into(), size.into());
    s
}

fn wire_entity(points: &[Point]) -> Entity {
    Entity::Wire(Wire {
        id: Uuid::new_v4(),
        points: points.to_vec(),
        color: "black".into(),
        sq: 0.75,
        length_m: None,
        length_source: Default::default(),
        part_no: None,
        net: None,
        dashed: false,
        weight_mm: None,
    })
}

fn pin_at(inst: &SymbolInstance, key: &str) -> Point {
    let defs = builtin_symbols();
    let index = DefIndex::new(&defs);
    let def = index.get(inst).expect("the block has a definition");
    pin_positions(inst, &def).into_iter().find(|(k, _)| k == key).expect("pin exists").1
}

/// A stub 2.5 mm to the left of the pin that ends in a net label.
fn labelled(out: &mut Vec<Entity>, inst: &SymbolInstance, key: &str, name: &str) {
    let at = pin_at(inst, key);
    let end = Point::new(at.x - 2.5, at.y);
    out.push(wire_entity(&[at, end]));
    out.push(Entity::NetLabel(NetLabel { id: Uuid::new_v4(), at: end, name: name.into(), rotation: 0, attrs: Default::default() }));
}

/// Source: UA (15 pins), UB (8 pins), UC (1 pin).
/// GND: UA 1-3 + UB P24A_2. P24X: UA 4-5 + UB 7. Jumper: UA 6-7 by a wire. FEED: UA 8-9 + UC 1.
/// BUS: UA 10-11 + UB 4-5. SIG_C: UA 12 + UB 6. D1..D3: UA 13-15 + UB 1-3.
pub(crate) fn bars_source() -> Project {
    let mut project = Project::new("draft-bars");
    let ua = block("UA", 60.0, 120.0, UA_PINS, r#"{"w":40,"h":80}"#);
    let ub = block("UB", 160.0, 120.0, UB_PINS, r#"{"w":40,"h":50}"#);
    let uc = block("UC", 260.0, 120.0, UC_PINS, r#"{"w":30,"h":20}"#);
    let mut e: Vec<Entity> = Vec::new();
    for k in ["1", "2", "3"] {
        labelled(&mut e, &ua, k, "GND");
    }
    labelled(&mut e, &ub, "P24A_2", "GND");
    for k in ["4", "5"] {
        labelled(&mut e, &ua, k, "P24X");
    }
    labelled(&mut e, &ub, "7", "P24X");
    let (a, b) = (pin_at(&ua, "6"), pin_at(&ua, "7"));
    let xb = a.x - 7.5;
    e.push(wire_entity(&[a, Point::new(xb, a.y), Point::new(xb, b.y), b]));
    for k in ["8", "9"] {
        labelled(&mut e, &ua, k, "FEED");
    }
    labelled(&mut e, &uc, "1", "FEED");
    for k in ["10", "11"] {
        labelled(&mut e, &ua, k, "BUS");
    }
    for k in ["4", "5"] {
        labelled(&mut e, &ub, k, "BUS");
    }
    labelled(&mut e, &ua, "12", "SIG_C");
    labelled(&mut e, &ub, "6", "SIG_C");
    for (i, (ka, kb)) in [("13", "1"), ("14", "2"), ("15", "3")].iter().enumerate() {
        let name = format!("D{}", i + 1);
        labelled(&mut e, &ua, ka, &name);
        labelled(&mut e, &ub, kb, &name);
    }
    for inst in [ua, ub, uc] {
        e.push(Entity::Symbol(inst));
    }
    for entity in e {
        project.sheets[0].entities.insert(entity.id(), entity);
    }
    project
}

/// Rebuilds tests/fixtures/draft-source-bars.mdkproj (run by hand with --ignored).
/// 棒・長い番号の見本の元の図のファイルを作り直す。普段は走らせず、手で動かしたときだけ作り直す。
#[test]
#[ignore]
fn write_the_bars_fixture() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/draft-source-bars.mdkproj");
    crate::io::save_project(&path, &bars_source()).unwrap();
}

// ---- helpers

pub(super) fn request(columns: &[&[&str]]) -> DraftRequest {
    DraftRequest {
        name: "Look".into(),
        columns: columns.iter().map(|c| c.iter().map(|r| r.to_string()).collect()).collect(),
        pins: BTreeMap::new(),
        power_nets: vec!["P24X".into()],
        ground_nets: Some(vec!["GND".into()]),
        destinations: BTreeMap::new(),
        paper: DraftPaper::A3,
        pitch_mm: 5.0,
        planned_sheets: Vec::new(),
        title_block: None,
    }
}

pub(super) fn open() -> Project {
    Project::new("open")
}

fn model_of(source: &Project, open: &Project, req: &DraftRequest) -> DraftModel {
    build_model(source, open, req).expect("the request is valid")
}

fn layout_in(open: &Project, req: &DraftRequest) -> DraftLayout {
    lay_out(&model_of(&bars_source(), open, req), open, req).expect("the layout fits")
}

fn layout(req: &DraftRequest) -> DraftLayout {
    layout_in(&open(), req)
}

fn plan(req: &DraftRequest) -> DraftPlan {
    plan_function_sheet(&bars_source(), &open(), req).expect("the plan is made")
}

pub(super) fn wires_of(sheet: &Sheet) -> Vec<Wire> {
    sheet.entities.values().filter_map(|e| if let Entity::Wire(w) = e { Some(w.clone()) } else { None }).collect()
}

pub(super) fn labels_of(sheet: &Sheet) -> Vec<NetLabel> {
    sheet.entities.values().filter_map(|e| if let Entity::NetLabel(l) = e { Some(l.clone()) } else { None }).collect()
}

pub(super) fn junctions_of(sheet: &Sheet) -> Vec<Point> {
    sheet.entities.values().filter_map(|e| if let Entity::Junction(j) = e { Some(j.at) } else { None }).collect()
}

pub(super) fn marks_of(sheet: &Sheet) -> Vec<SymbolInstance> {
    sheet.entities.values().filter_map(|e| if let Entity::Symbol(s) = e { Some(s.clone()) } else { None }).filter(|s| s.reference.is_empty()).collect()
}

pub(super) fn unit_of(sheet: &Sheet, reference: &str) -> SymbolInstance {
    sheet
        .entities
        .values()
        .find_map(|e| match e {
            Entity::Symbol(s) if s.reference == reference => Some(s.clone()),
            _ => None,
        })
        .expect("the unit is on the sheet")
}

pub(super) fn pin_pt(sheet: &Sheet, reference: &str, key: &str) -> Point {
    let defs = sheet_symbol_defs(sheet);
    pin_points(sheet, &defs)
        .into_iter()
        .find(|p| p.reference == reference && p.number == key)
        .unwrap_or_else(|| panic!("{reference}.{key} is on the sheet"))
        .pos
}

/// The far end of the stub that starts at the pin.
pub(super) fn stub_end(sheet: &Sheet, reference: &str, key: &str) -> Point {
    let p = pin_pt(sheet, reference, key);
    wires_of(sheet)
        .iter()
        .find_map(|w| (w.points.len() == 2 && w.points[0].distance_to(&p) < 1e-6).then(|| w.points[1]))
        .unwrap_or_else(|| panic!("{reference}.{key} has a stub"))
}

/// The lead from a group end to its mark: the three-point wire that starts at `anchor` (end, corner, mark).
fn mark_lead_of(sheet: &Sheet, anchor: Point) -> Vec<Point> {
    wires_of(sheet)
        .into_iter()
        .find(|w| w.points.len() == 3 && w.points[0].distance_to(&anchor) < 1e-6)
        .unwrap_or_else(|| panic!("a mark lead leaves {anchor:?}"))
        .points
}

pub(super) fn arms_at(sheet: &Sheet, p: Point) -> usize {
    wires_of(sheet)
        .iter()
        .flat_map(|w| w.points.windows(2).map(|s| (s[0], s[1])).collect::<Vec<_>>())
        .map(|(a, b)| {
            if a.distance_to(&p) < 1e-6 || b.distance_to(&p) < 1e-6 {
                1
            } else if crate::netlist::segment_distance(&p, &a, &b) < 1e-6 {
                2
            } else {
                0
            }
        })
        .sum()
}

pub(super) fn rect_of(points: &[Point]) -> Rect {
    let mut r = Rect::from_corners(points[0], points[0]);
    points.iter().for_each(|p| r.extend(*p));
    r
}

pub(super) fn seg_rect(a: Point, b: Point) -> Rect {
    Rect::from_corners(a, b)
}

/// The printed pin numbers outside a unit's box, with their rectangles (text anchored at its centre).
pub(super) fn number_rects(sheet: &Sheet, reference: &str) -> Vec<(String, Rect)> {
    let inst = unit_of(sheet, reference);
    let defs = sheet_symbol_defs(sheet);
    let index = DefIndex::new(&defs);
    let def = index.get(&inst).expect("the box has a definition");
    let spec = block::parse_block_spec(&inst.attrs).expect("the box reads");
    let (lo, hi) = block::block_outline(&spec);
    let mut out = Vec::new();
    for prim in &def.primitives {
        if let crate::symbol::Primitive::Text { at, text, height, .. } = prim {
            if at.x < lo.x || at.x > hi.x {
                let (cx, cy) = (inst.at.x + at.x, inst.at.y + at.y);
                let (hw, hh) = (text_width(text, *height) / 2.0, height / 2.0);
                out.push((text.clone(), Rect::from_corners(Point::new(cx - hw, cy - hh), Point::new(cx + hw, cy + hh))));
            }
        }
    }
    out
}

/// The figure and the printed value of a mark.
pub(super) fn mark_rect(sheet: &Sheet, mark: &SymbolInstance) -> Rect {
    let defs = sheet_symbol_defs(sheet);
    let index = DefIndex::new(&defs);
    let def = index.get(mark).expect("a mark has a definition");
    let mut r = crate::tidy::symbol_bounds(mark, &def).expect("a mark has a figure");
    for b in text_boxes(sheet, &defs).iter().filter(|b| b.owner == Some(mark.id)) {
        r.extend(b.rect.min);
        r.extend(b.rect.max);
    }
    r
}

pub(super) fn flag_of(sheet: &Sheet, l: &NetLabel, dest: Option<&str>) -> Option<Rect> {
    let font = crate::svg::LABEL_FONT * sheet.annotation_scale();
    let tag = crate::net_tag::net_tag_with(l, font, sheet.xref_style, dest.or_else(|| crate::net_tag::tag_dest(l)))?;
    Some(rect_of(&tag.points))
}

pub(super) fn bounds_of(sheet: &Sheet, reference: &str) -> Rect {
    let s = unit_of(sheet, reference);
    let defs = sheet_symbol_defs(sheet);
    let index = DefIndex::new(&defs);
    crate::tidy::symbol_bounds(&s, &index.get(&s).unwrap()).unwrap()
}

fn on_a_bar(sheet: &Sheet, a: Point, b: Point) -> bool {
    wires_of(sheet).iter().any(|w| {
        w.points.len() == 2
            && ((w.points[0].distance_to(&a) < 1e-6 && w.points[1].distance_to(&b) < 1e-6)
                || (w.points[0].distance_to(&b) < 1e-6 && w.points[1].distance_to(&a) < 1e-6))
    })
}

// ---- change 1: groups of pins on a bar

/// Three ground pins of one unit sit in adjacent rows, in key order, and share one ground mark: the vertical bar goes on one pitch below the last pin, turns away from the box, and the mark hangs at the end.
/// 1つの機器の3本の接地ピンは隣の行に番号順に並び、縦の棒でつながって、接地の印を1つだけ持つ。棒の下の端からピン間隔ぶん下へ伸び、箱から離れる向きへ曲がった先に印が下がる。
#[test]
fn three_ground_pins_of_one_unit_get_one_mark_at_the_lower_end_of_a_bar() {
    let l = layout(&request(&[&["UA"], &["UB"]]));
    let (e1, e3) = (stub_end(&l.sheet, "UA", "1"), stub_end(&l.sheet, "UA", "3"));
    let (p1, p2, p3) = (pin_pt(&l.sheet, "UA", "1"), pin_pt(&l.sheet, "UA", "2"), pin_pt(&l.sheet, "UA", "3"));
    assert!((p2.y - p1.y - 5.0).abs() < 1e-6 && (p3.y - p2.y - 5.0).abs() < 1e-6, "adjacent rows: {p1:?} {p2:?} {p3:?}");
    assert!(on_a_bar(&l.sheet, e1, e3), "one straight bar joins the first and the last stub end");
    assert!((e1.x - e3.x).abs() < 1e-9);
    let lead = mark_lead_of(&l.sheet, e3);
    assert_eq!(lead[1], Point::new(e3.x, e3.y + 5.0), "one pitch below the last pin");
    assert!(lead[2].y == lead[1].y && lead[2].x < e3.x, "then away from the box (left side)");
    let marks: Vec<_> = marks_of(&l.sheet).into_iter().filter(|m| m.value == "GND" && m.at == lead[2]).collect();
    assert_eq!(marks.len(), 1, "one mark for the three pins, at the end of the lead");
}

/// Two power pins of one unit share one power mark, led out from the upper end of the bar: one pitch up, then away from the box.
/// 1つの機器の2本の電源ピンは、電源の印を1つだけ持つ。印は棒の上の端からピン間隔ぶん上へ伸びて箱から離れる向きへ曲がった先に立つ。
#[test]
fn two_power_pins_of_one_unit_get_one_mark_at_the_upper_end_of_a_bar() {
    let l = layout(&request(&[&["UA"], &["UB"]]));
    let (e4, e5) = (stub_end(&l.sheet, "UA", "4"), stub_end(&l.sheet, "UA", "5"));
    assert!(on_a_bar(&l.sheet, e4, e5));
    let lead = mark_lead_of(&l.sheet, e4);
    assert_eq!(lead[1], Point::new(e4.x, e4.y - 5.0), "one pitch above the first pin");
    let marks: Vec<_> = marks_of(&l.sheet).into_iter().filter(|m| m.value == "P24X" && m.at == lead[2]).collect();
    assert_eq!(marks.len(), 1);
}

/// The bar has a junction wherever three ends meet: the stub end of a middle pin and the end where the mark lead leaves; the other end of the bar has none.
/// 棒には、3本の端が会う所 (真ん中のピンの短い線の先と、印の引き出しが出る端) に接点の丸が付き、反対の端には付かない。
#[test]
fn a_bar_has_a_junction_where_three_ends_meet() {
    let l = layout(&request(&[&["UA"], &["UB"]]));
    let dots = junctions_of(&l.sheet);
    let (e1, e2, e3) = (stub_end(&l.sheet, "UA", "1"), stub_end(&l.sheet, "UA", "2"), stub_end(&l.sheet, "UA", "3"));
    assert!(dots.iter().any(|d| d.distance_to(&e2) < 1e-6), "the middle pin");
    assert!(dots.iter().any(|d| d.distance_to(&e3) < 1e-6), "the lower end, where the ground lead leaves");
    assert!(!dots.iter().any(|d| d.distance_to(&e1) < 1e-6), "the upper end of a ground bar");
    for d in &dots {
        assert_eq!(arms_at(&l.sheet, *d), 3, "{d:?}");
    }
}

/// A signal net carried by two pins of one unit puts its pins in adjacent rows on one side, in key order.
/// 1つの機器の2本のピンで運ぶ信号のネットは、同じ辺の隣の行に番号順に並ぶ。
#[test]
fn pins_of_a_signal_group_are_in_adjacent_rows_in_key_order() {
    let l = layout(&request(&[&["UA"], &["UB"]]));
    let (a, b) = (pin_pt(&l.sheet, "UA", "10"), pin_pt(&l.sheet, "UA", "11"));
    assert!((b.y - a.y - 5.0).abs() < 1e-6 && (a.x - b.x).abs() < 1e-9, "{a:?} {b:?}");
    let (c, d) = (pin_pt(&l.sheet, "UB", "4"), pin_pt(&l.sheet, "UB", "5"));
    assert!((d.y - c.y - 5.0).abs() < 1e-6);
}

/// A jump inside one unit (both pins on the same net and nothing else) is only a bar: no wire is routed and no label is needed, and the two pins stay on one net.
/// 1つの機器の中だけのジャンパ (同じネットの2本のピンだけ) は棒だけでつなぐ。配線は引かず、ラベルも付けないが、2本のピンは同じネットのまま。
#[test]
fn a_jumper_inside_one_unit_is_only_a_bar() {
    let mut req = request(&[&["UA"]]);
    req.pins.insert("UA".into(), vec!["6".into(), "7".into()]);
    let p = plan(&req);
    assert!(p.wired.is_empty() && p.labelled.is_empty(), "{:?} {:?}", p.wired, p.labelled);
    assert!(labels_of(&p.sheet).is_empty());
    let (e6, e7) = (stub_end(&p.sheet, "UA", "6"), stub_end(&p.sheet, "UA", "7"));
    assert!(on_a_bar(&p.sheet, e6, e7));
    let defs = sheet_symbol_defs(&p.sheet);
    let nets = extract_netlist(&p.sheet, &defs);
    assert!(nets.iter().any(|n| n.pins.iter().any(|q| q.pin == "6") && n.pins.iter().any(|q| q.pin == "7")));
}

/// A leaving net carried by two pins of one unit has one tag, hanging at the end of the bar, and no routed wire.
/// 1つの機器の2本のピンで運ぶ出ていくネットは、札が1つだけで、棒の端に下がる。配線は引かない。
#[test]
fn a_leaving_net_carried_by_two_pins_of_one_unit_has_one_tag_on_the_bar() {
    let mut req = request(&[&["UA"]]);
    req.pins.insert("UA".into(), vec!["8".into(), "9".into()]);
    let p = plan(&req);
    let tags: Vec<_> = labels_of(&p.sheet).into_iter().filter(|l| l.name == "FEED").collect();
    assert_eq!(tags.len(), 1, "{tags:?}");
    assert!(p.wired.is_empty() && p.labelled.is_empty());
    assert_eq!(p.tagged, vec![DraftTagged { net: "FEED".into(), pins: 2 }]);
    let (e8, e9) = (stub_end(&p.sheet, "UA", "8"), stub_end(&p.sheet, "UA", "9"));
    assert!(on_a_bar(&p.sheet, e8, e9));
    assert!(tags[0].at.distance_to(&e8) < 1e-6 || tags[0].at.distance_to(&e9) < 1e-6, "the tag hangs at an end of the bar");
}

/// A net whose pins sit in a group on each of two units is routed as one wire between the two bars.
/// 2つの機器がそれぞれ棒でまとめたピンを持つネットは、2本の棒のあいだを配線1本でつなぐ。
#[test]
fn a_net_between_two_groups_is_one_routed_wire() {
    let req = request(&[&["UA"], &["UB"]]);
    let l = layout(&req);
    let m = model_of(&bars_source(), &open(), &req);
    let bus = m.nets.iter().position(|n| n.name == "BUS").expect("BUS is carried");
    assert_eq!(l.terminals.iter().filter(|t| t.net == bus).count(), 2, "one terminal per group");
    let p = plan(&req);
    assert!(p.wired.contains(&"BUS".to_string()), "{:?} {:?}", p.wired, p.labelled);
}

/// The terminal of a group is the bar end nearest to its peers.
/// 棒のまとまりの配線の端は、相手のピンにいちばん近い棒の端にある。
#[test]
fn the_terminal_of_a_group_is_the_bar_end_nearest_to_its_peers() {
    let req = request(&[&["UA"], &["UB"]]);
    let l = layout(&req);
    let m = model_of(&bars_source(), &open(), &req);
    let bus = m.nets.iter().position(|n| n.name == "BUS").unwrap();
    let ua = l.terminals.iter().filter(|t| t.net == bus).find(|t| t.at.x < pin_pt(&l.sheet, "UB", "4").x).expect("the UA terminal");
    let (top, bottom) = (stub_end(&l.sheet, "UA", "10"), stub_end(&l.sheet, "UA", "11"));
    let peers = (pin_pt(&l.sheet, "UB", "4").y + pin_pt(&l.sheet, "UB", "5").y) / 2.0;
    let nearest = if (top.y - peers).abs() <= (bottom.y - peers).abs() { top } else { bottom };
    assert!(ua.at.distance_to(&nearest) < 1e-6 || (peers > top.y && peers < bottom.y), "{:?} {top:?} {bottom:?} {peers}", ua.at);
}

/// The result counts marks, not pins: three ground pins on one bar are one ground mark.
/// 結果の marked は印の数であってピンの数ではない。3本の接地ピンを棒でまとめれば、接地の印は1つと数える。
#[test]
fn marked_counts_marks_not_pins() {
    let p = plan(&request(&[&["UA"], &["UB"]]));
    assert_eq!(p.marked, DraftMarked { power: 2, ground: 2 });
    assert_eq!(marks_of(&p.sheet).len(), 4);
}

/// A mark never overlaps a pin number, a stub or a text of the rows around it.
/// 印は、まわりの行のピン番号・短い線・文字に重ならない。
#[test]
fn marks_do_not_overlap_the_rows_around_them() {
    let p = plan(&request(&[&["UA"], &["UB"]]));
    let defs = sheet_symbol_defs(&p.sheet);
    let texts = text_boxes(&p.sheet, &defs);
    for m in marks_of(&p.sheet) {
        let r = mark_rect(&p.sheet, &m);
        for reference in ["UA", "UB"] {
            for (n, nr) in number_rects(&p.sheet, reference) {
                assert!(!r.overlaps(&nr), "mark {} at {:?} overlaps the number {n} of {reference}", m.value, m.at);
            }
        }
        for t in texts.iter().filter(|t| t.owner != Some(m.id)) {
            assert!(!r.overlaps(&t.rect), "mark {} at {:?} overlaps the text {:?}", m.value, m.at, t.text);
        }
        for w in wires_of(&p.sheet) {
            for seg in w.points.windows(2) {
                let own = seg.iter().any(|q| q.distance_to(&m.at) < 1e-6) || ((seg[0].x - m.at.x).abs() < 1e-6 && (seg[1].x - m.at.x).abs() < 1e-6);
                assert!(own || !r.overlaps(&seg_rect(seg[0], seg[1])), "mark {} at {:?} overlaps a wire {:?}", m.value, m.at, seg);
            }
        }
    }
}

// ---- change 2: stub length follows the pin number

/// The stub of every pin on a side is as long as the widest printed number of that side asks (at least 5 mm, number width + 2.5 mm, on the 2.5 mm grid): 12.5 mm beside a six character number, 7.5 mm beside two digits, 5 mm beside one digit.
/// 辺のすべてのピンの短い線は、その辺のいちばん広い番号に合わせた同じ長さになる (5mm以上、番号の幅+2.5mmを2.5mm刻みに切り上げ)。6文字の番号の辺は12.5mm、2桁の辺は7.5mm、1桁の辺は5mm。
#[test]
fn the_stub_length_follows_the_widest_number_of_a_side() {
    let l = layout(&request(&[&["UA"], &["UB"]]));
    let th = pin_text_mm(5.0);
    assert_eq!(grid_up(f64::max(5.0, text_width("P24A_2", th) + 2.5)), 12.5);
    for (reference, keys, want) in [
        ("UB", vec!["P24A_2", "1", "2", "3", "4", "5", "6", "7"], 12.5),
        ("UA", vec!["10", "11", "12", "13", "14", "15"], 7.5),
        ("UA", vec!["1", "2", "3", "4", "5", "6", "7", "8", "9"], 5.0),
    ] {
        for k in keys {
            let (p, e) = (pin_pt(&l.sheet, reference, k), stub_end(&l.sheet, reference, k));
            assert!((e.y - p.y).abs() < 1e-9 && ((e.x - p.x).abs() - want).abs() < 1e-6, "{reference}.{k}: stub {} want {want}", (e.x - p.x).abs());
        }
    }
}

/// With one-digit numbers the stub stays 5 mm.
/// 番号が1桁のときは、短い線は今までどおり5mmのまま。
#[test]
fn short_numbers_keep_the_five_millimetre_stub() {
    let l = layout(&request(&[&["UA"], &["UB"]]));
    let (p, e) = (pin_pt(&l.sheet, "UA", "1"), stub_end(&l.sheet, "UA", "1"));
    assert!(((e.x - p.x).abs() - STUB_MM).abs() < 1e-6);
}

/// No mark, tag, bar or wire overlaps a printed pin number.
/// 印・札・棒・配線のどれも、印字されたピン番号に重ならない。
#[test]
fn nothing_overlaps_a_printed_pin_number() {
    let p = plan(&request(&[&["UA"], &["UB"]]));
    let defs = sheet_symbol_defs(&p.sheet);
    let texts = text_boxes(&p.sheet, &defs);
    for reference in ["UA", "UB"] {
        for (n, nr) in number_rects(&p.sheet, reference) {
            for m in marks_of(&p.sheet) {
                assert!(!mark_rect(&p.sheet, &m).overlaps(&nr), "mark {} overlaps number {n} of {reference}", m.value);
            }
            for l in labels_of(&p.sheet) {
                if let Some(f) = flag_of(&p.sheet, &l, None) {
                    assert!(!f.overlaps(&nr), "tag {} overlaps number {n} of {reference}", l.name);
                }
            }
            for w in wires_of(&p.sheet) {
                for seg in w.points.windows(2) {
                    assert!(!seg_rect(seg[0], seg[1]).overlaps(&nr), "wire {seg:?} overlaps number {n} of {reference} {nr:?}");
                }
            }
            for t in texts.iter().filter(|t| matches!(t.kind, TextKind::NetLabel | TextKind::Value) && t.owner.is_some()) {
                let inst_owned = marks_of(&p.sheet).iter().any(|m| Some(m.id) == t.owner) || labels_of(&p.sheet).iter().any(|l| Some(l.id) == t.owner);
                assert!(!inst_owned || !t.rect.overlaps(&nr), "text {:?} overlaps number {n}", t.text);
            }
        }
    }
}

// ---- change 3: the fallback uses tags

/// When the router gives up on a net, each terminal gets a tag (no destination, flag away from the box) at the end of its stub, never a plain label.
/// 配線を引けないネットは、各端の短い線の先に札 (行き先なし、旗は箱から離れる向き) を付ける。ただのラベルは使わない。
#[test]
fn a_failed_net_gets_tags_at_the_stub_ends() {
    let opts = RouteOptions { frame_margin_mm: 100.0, ..RouteOptions::default() };
    let req = request(&[&["UA"], &["UB"]]);
    let p = plan_with_options(&bars_source(), &open(), &req, &opts).expect("valid");
    assert!(p.labelled.contains(&"BUS".to_string()) && p.labelled.contains(&"SIG_C".to_string()), "{:?}", p.labelled);
    for name in &p.labelled {
        let ls: Vec<NetLabel> = labels_of(&p.sheet).into_iter().filter(|l| &l.name == name).collect();
        assert!(ls.len() >= 2, "{name}: a label at each end");
        for l in &ls {
            assert!(l.attrs.contains_key(crate::net_tag::TAG_ATTR), "{name} is a plain label");
            assert!(!l.attrs.contains_key(crate::net_tag::DEST_ATTR));
            assert!(arms_at(&p.sheet, l.at) >= 1, "{name} hangs on a wire end");
        }
    }
    let flags: Vec<(String, Rect)> = labels_of(&p.sheet).iter().filter_map(|l| flag_of(&p.sheet, l, None).map(|f| (l.name.clone(), f))).collect();
    for (i, (na, a)) in flags.iter().enumerate() {
        for (nb, b) in &flags[i + 1..] {
            assert!(!a.overlaps(b), "tags {na} and {nb} overlap");
        }
        for reference in ["UA", "UB"] {
            assert!(!a.overlaps(&bounds_of(&p.sheet, reference)), "tag {na} overlaps the box {reference}");
        }
    }
}

// ---- change 4: tags stay inside the drawing area

fn joined_names(open: &Project, name: &str) -> Option<String> {
    crate::xref::tag_dest_names(open, Uuid::new_v4(), name)
}

fn open_with_sheets(names: &[&str], label: &str) -> Project {
    let mut project = open();
    project.sheets.clear();
    for n in names {
        let mut s = Sheet::new(n, PaperSize::A3, Orientation::Landscape);
        let l = NetLabel { id: Uuid::new_v4(), at: Point::new(50.0, 50.0), name: label.into(), rotation: 0, attrs: Default::default() };
        s.entities.insert(l.id, Entity::NetLabel(l));
        project.sheets.push(s);
    }
    project
}

/// A tag reserves room for the names of the other sheets that already carry its net, joined as the tags will show them (two names and the other marker).
/// 札は、同じネットがすでにある他のシートの名前を、旗に書かれるとおりにつないだ分の場所を空ける (2枚の名前と「他」)。
#[test]
fn a_tag_reserves_room_for_the_sheets_that_already_carry_the_net() {
    let project = open_with_sheets(&["Cabinet North wing", "Cabinet South wing", "Cabinet East wing"], "FEED");
    let joined = joined_names(&project, "FEED").expect("three sheets carry FEED");
    let req = request(&[&["UA"]]);
    let l = layout_in(&project, &req);
    let font = crate::svg::LABEL_FONT * l.sheet.annotation_scale();
    let feed = labels_of(&l.sheet).into_iter().find(|n| n.name == "FEED").expect("FEED leaves the sheet");
    let want = flag_rect(&feed, font, l.sheet.xref_style, &joined).unwrap();
    assert!(
        l.keep_out.iter().any(|k| k.min.x <= want.min.x + 1e-9 && k.max.x >= want.max.x - 1e-9 && k.min.y <= want.min.y + 1e-9 && k.max.y >= want.max.y - 1e-9),
        "no room for {joined:?}"
    );
}

/// Every tag flag, with room for its destination text, lies inside the drawing area on A3 and on A4.
/// 札の旗は、行き先の文字の場所を含めて、A3でもA4でも描画範囲の中に収まる。
#[test]
fn every_tag_flag_stays_inside_the_drawing_area() {
    let project = open_with_sheets(&["Cabinet North wing", "Cabinet South wing", "Cabinet East wing"], "FEED");
    for paper in [DraftPaper::A3, DraftPaper::A4] {
        for cols in [vec![vec!["UA"]], vec![vec!["UA"], vec!["UB"]], vec![vec!["UB"], vec!["UA"]]] {
            let cols: Vec<&[&str]> = cols.iter().map(|c| c.as_slice()).collect();
            let mut req = request(&cols);
            req.paper = paper;
            let l = layout_in(&project, &req);
            let font = crate::svg::LABEL_FONT * l.sheet.annotation_scale();
            for n in labels_of(&l.sheet) {
                let text = joined_names(&project, &n.name).unwrap_or_default();
                let floor = reserved_dest(&project);
                let reserve = if text_width(&text, 1.0) > text_width(&floor, 1.0) { text } else { floor };
                let r = flag_rect(&n, font, l.sheet.xref_style, &reserve).expect("tags are flags");
                assert!(r.min.x >= l.frame.0 + AREA_MARGIN_MM - 1e-6, "{paper:?} {}: {r:?} leaves on the left", n.name);
                assert!(r.max.x <= l.frame.2 - AREA_MARGIN_MM + 1e-6, "{paper:?} {}: {r:?} leaves on the right", n.name);
            }
        }
    }
}

/// A request whose tags (with the names of the other sheets that already carry the net) would cross the right edge of the drawing area is refused with the column number and the excess in mm, on A4 and on A3.
/// 同じネットをすでに持つ他のシートの名前まで含めると札が描画範囲の右の縁を越える依頼は、A4でもA3でも、列の番号と超えた量 (mm) を挙げて断る。
#[test]
fn tags_that_would_cross_the_edge_are_refused_with_column_and_excess() {
    for (paper, len) in [(DraftPaper::A4, 80), (DraftPaper::A3, 150)] {
        let name = "M".repeat(len);
        let project = open_with_sheets(&[&format!("{name}a"), &format!("{name}b"), &format!("{name}c")], "FEED");
        // UA is in the last column, so its FEED tag (no peer on the sheet) points to the right edge
        let mut req = request(&[&["UB"], &["UA"]]);
        req.paper = paper;
        let m = model_of(&bars_source(), &project, &req);
        let e = lay_out(&m, &project, &req).expect_err("the tags do not fit");
        assert!(e.starts_with("Column 2 does not fit the width") && e.contains(" mm more"), "{paper:?}: {e}");
        let excess: f64 = e.split("need ").nth(1).unwrap().split(' ').next().unwrap().parse().unwrap();
        assert!(excess > 0.0, "{e}");
    }
}

/// A destination text that is too long for the sheet is refused the same way.
/// 行き先の文字が長すぎて収まらないときも、同じ形で断る。
#[test]
fn a_destination_too_long_for_the_sheet_is_refused() {
    let mut req = request(&[&["UB"], &["UA"]]);
    req.paper = DraftPaper::A4;
    let far = "a destination written out in a very long sentence so that the tag grows wide ".repeat(4);
    req.destinations.insert("FEED".into(), far);
    let m = model_of(&bars_source(), &open(), &req);
    let e = lay_out(&m, &open(), &req).expect_err("the tag does not fit");
    assert!(e.starts_with("Column 2 does not fit the width"), "{e}");
}

// ---- change 5: columns are packed

/// The channel after column k: max(20 mm, 10 mm + 2.5 mm per track the layout needs), on the grid.
fn channel_after(l: &DraftLayout, k: usize) -> f64 {
    grid_up(f64::max(20.0, 10.0 + 2.5 * l.channels[k].tracks as f64))
}

/// Column k+1 starts one channel after the right extent of column k, with channel = max(20 mm, 10 mm + 2.5 mm per track), and the first column starts at the left edge of the drawing area.
/// 列k+1は、列kの右の張り出しから通り道1つぶん (20mmと、10mm+トラック1本につき2.5mmの大きい方) あけて始まり、最初の列は描画範囲の左端から始まる。
#[test]
fn columns_are_packed_with_the_channel_rule() {
    let req = request(&[&["UA"], &["UB"], &["UC"]]);
    let m = model_of(&bars_source(), &open(), &req);
    let l = lay_out(&m, &open(), &req).expect("fits");
    let x0 = grid_up(l.frame.0 + AREA_MARGIN_MM);
    assert!((l.columns[0].left - x0).abs() < 1e-6, "{:?} {x0}", l.columns[0]);
    let first = channel_after(&l, 0);
    assert!(l.channels[0].tracks >= 1, "the first channel carries nets");
    assert!((l.columns[1].left - l.columns[0].right - first).abs() < 1e-6, "{:?}", l.columns);
    let second = channel_after(&l, 1);
    assert!((l.columns[2].left - l.columns[1].right - second).abs() < 1e-6, "{:?}", l.columns);
}

/// With few nets the channel is 20 mm, and a single column sits at the left edge, not in the middle.
/// ネットが少ないときの通り道は20mmで、列が1つだけでも描画範囲の中央ではなく左端に置く。
#[test]
fn a_single_column_sits_at_the_left_edge() {
    let req = request(&[&["UA"]]);
    let l = layout(&req);
    assert!((l.columns[0].left - grid_up(l.frame.0 + AREA_MARGIN_MM)).abs() < 1e-6);
    let mut req = request(&[&["UA"], &["UC"]]);
    req.pins.insert("UA".into(), vec!["8".into(), "9".into()]);
    let m = model_of(&bars_source(), &open(), &req);
    let l = lay_out(&m, &open(), &req).unwrap();
    assert!((l.columns[1].left - l.columns[0].right - 22.5).abs() < 1e-6 || (l.columns[1].left - l.columns[0].right - 20.0).abs() < 1e-6, "{:?}", l.columns);
}

// ---- change 6: gaps between units

/// The marks of a unit do not touch the unit's own caption or the caption of the next unit below, and the gap stays at least four pitches.
/// 機器の印は、その機器自身の参照記号の文字にも、下に積んだ次の機器の参照記号の文字にも触れず、すき間はピン間隔4つぶん以上ある。
#[test]
fn marks_keep_clear_of_the_captions_and_the_gap_stays_four_pitches() {
    let req = request(&[&["UA", "UB"]]);
    let l = layout(&req);
    let defs = sheet_symbol_defs(&l.sheet);
    let texts = text_boxes(&l.sheet, &defs);
    let (ua, ub) = (unit_of(&l.sheet, "UA"), unit_of(&l.sheet, "UB"));
    let caption = |id: Uuid| -> Vec<Rect> { texts.iter().filter(|t| t.owner == Some(id) && matches!(t.kind, TextKind::Reference | TextKind::Value)).map(|t| t.rect).collect() };
    for m in marks_of(&l.sheet) {
        let r = mark_rect(&l.sheet, &m);
        for id in [ua.id, ub.id] {
            for c in caption(id) {
                assert!(!r.overlaps(&c), "mark {} at {:?} touches a caption {c:?}", m.value, m.at);
            }
        }
    }
    let ua_bottom = bounds_of(&l.sheet, "UA").max.y;
    let below_ua = marks_of(&l.sheet).iter().filter(|m| m.at.y < ua_bottom + 10.0).map(|m| mark_rect(&l.sheet, m).max.y).fold(f64::MIN, f64::max);
    let top_of_ub_caption = caption(ub.id).iter().map(|c| c.min.y).fold(f64::MAX, f64::min);
    let lowest_ua = below_ua.max(bounds_of(&l.sheet, "UA").max.y);
    assert!(top_of_ub_caption - lowest_ua >= 4.0 * 5.0 - 1e-6, "gap {}", top_of_ub_caption - lowest_ua);
}

// ---- review fixes: same-column peers (B), flags as hard obstacles (C), mark leads (D), numbers at the box edge (A)

fn numbered_pins(n: usize) -> String {
    let v: Vec<String> = (1..=n).map(|i| format!(r#"{{"no":"{i}","name":"T{i}"}}"#)).collect();
    format!(r#"{{"left":[{}]}}"#, v.join(","))
}

/// Builds a source from units (reference, number of pins) and nets (name, pins); every pin gets a short stub with a label of its net.
pub(super) fn source_of(name: &str, units: &[(&str, usize)], nets: &[(&str, &[(&str, &str)])]) -> Project {
    let mut project = Project::new(name);
    let insts: Vec<SymbolInstance> = units
        .iter()
        .enumerate()
        .map(|(i, (r, n))| {
            let Entity::Symbol(mut s) = crate::block::test_block(r, "", 60.0 + 70.0 * (i % 3) as f64, 60.0 + 90.0 * (i / 3) as f64, &numbered_pins(*n)) else { unreachable!() };
            s.attrs.remove(crate::block::BLOCK_SIZE_ATTR);
            s
        })
        .collect();
    let mut e: Vec<Entity> = Vec::new();
    for (net, members) in nets {
        for (r, k) in members.iter() {
            let inst = insts.iter().find(|s| s.reference == *r).expect("the unit is listed");
            labelled(&mut e, inst, k, net);
        }
    }
    for inst in insts {
        e.push(Entity::Symbol(inst));
    }
    for entity in e {
        project.sheets[0].entities.insert(entity.id(), entity);
    }
    project
}

/// Source: four units SA, SB, SC, SD meant to be stacked in one column, wired to each other (K1 to K5, K7 also to SY), four nets that leave the sheet (L1 to L4, the other end on SX), and K6 from SA to SY.
pub(super) fn stacked_source() -> Project {
    source_of(
        "draft-stacked",
        &[("SA", 5), ("SB", 4), ("SC", 5), ("SD", 5), ("SX", 4), ("SY", 2)],
        &[
            ("K1", &[("SA", "1"), ("SC", "1")]),
            ("K2", &[("SA", "2"), ("SD", "1")]),
            ("K3", &[("SB", "1"), ("SD", "2")]),
            ("K4", &[("SB", "2"), ("SC", "2"), ("SA", "3")]),
            ("K5", &[("SC", "3"), ("SD", "3")]),
            ("L1", &[("SA", "4"), ("SX", "1")]),
            ("L2", &[("SB", "3"), ("SX", "2")]),
            ("L3", &[("SC", "4"), ("SX", "3")]),
            ("L4", &[("SD", "4"), ("SX", "4")]),
            ("K6", &[("SA", "5"), ("SY", "1")]),
            ("K7", &[("SY", "2"), ("SB", "4"), ("SC", "5"), ("SD", "5")]),
        ],
    )
}

/// An open project whose two sheets already carry L1 and L2, so those tags show two sheet names.
pub(super) fn stacked_open() -> Project {
    open_with_sheets(&["Pump room west", "Pump room east"], "L1").tap(|p| {
        for s in &mut p.sheets {
            let l = NetLabel { id: Uuid::new_v4(), at: Point::new(50.0, 60.0), name: "L2".into(), rotation: 0, attrs: Default::default() };
            s.entities.insert(l.id, Entity::NetLabel(l));
        }
    })
}

trait Tap: Sized {
    fn tap(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }
}
impl Tap for Project {}

pub(super) const STACK: [&str; 4] = ["SA", "SB", "SC", "SD"];

pub(super) fn stacked_request(columns: &[&[&str]], pitch: f64) -> DraftRequest {
    let mut r = request(columns);
    r.power_nets.clear();
    r.ground_nets = Some(Vec::new());
    r.pitch_mm = pitch;
    r
}

/// The three ways the stacked fixture is drafted: the stack alone, the stack before SY (not the last column), the stack after SY (the last column).
pub(super) fn stacked_columns() -> Vec<Vec<&'static [&'static str]>> {
    vec![vec![&STACK[..]], vec![&STACK[..], &["SY"][..]], vec![&["SY"][..], &STACK[..]]]
}

/// The side of a drafted pin: left when its stub runs to the left of the pin.
pub(super) fn side_of(sheet: &Sheet, reference: &str, key: &str) -> &'static str {
    let (p, e) = (pin_pt(sheet, reference, key), stub_end(sheet, reference, key));
    if e.x < p.x { "left" } else { "right" }
}

/// Device pin groups of a sheet (pins of at least two), only the references in `refs`.
pub(super) fn device_groups(sheet: &Sheet, refs: &[&str]) -> BTreeSet<BTreeSet<(String, String)>> {
    let defs = sheet_symbol_defs(sheet);
    extract_netlist(sheet, &defs)
        .into_iter()
        .map(|n| n.pins.into_iter().filter(|p| refs.contains(&p.reference.as_str())).map(|p| (p.reference, p.pin)).collect::<BTreeSet<_>>())
        .filter(|g| g.len() >= 2)
        .collect()
}

/// The open project with the drafted sheet added, as the export sees it.
pub(super) fn with_sheet(open: &Project, sheet: &Sheet) -> Project {
    let mut p = open.clone();
    p.sheets.push(sheet.clone());
    p
}

/// Every wire segment that touches a tag flag. The flag is the one the export draws: `net_tag_with` with the destination `sheet_xrefs` prints. The only wire allowed to touch a flag is one that ends at the tag's point and lies at or below the flag's lower edge (the tag's own stub, branch or bar: a wire ending at the tag's point is on the tag's net, or the pin-group test reports a short).
pub(super) fn wires_touching_flags(project: &Project, sheet: &Sheet) -> Vec<String> {
    let xrefs = crate::xref::sheet_xrefs(project, sheet.id);
    let font = crate::svg::LABEL_FONT * sheet.annotation_scale();
    let mut bad = Vec::new();
    for l in labels_of(sheet) {
        let Some(tag) = crate::net_tag::net_tag_with(&l, font, sheet.xref_style, xrefs.get(&l.id).map(String::as_str)) else { continue };
        let r = rect_of(&tag.points);
        for w in wires_of(sheet) {
            for s in w.points.windows(2) {
                let (a, b) = (s[0], s[1]);
                let (x0, x1, y0, y1) = (a.x.min(b.x), a.x.max(b.x), a.y.min(b.y), a.y.max(b.y));
                let hit = x0 <= r.max.x + 1e-9 && x1 >= r.min.x - 1e-9 && y0 <= r.max.y + 1e-9 && y1 >= r.min.y - 1e-9;
                let at_anchor = a.distance_to(&l.at) < 1e-6 || b.distance_to(&l.at) < 1e-6;
                let own = at_anchor && y0 >= r.max.y - 1e-9;
                if hit && !own {
                    bad.push(format!("{} {} flag {r:?}: segment {a:?}-{b:?}", sheet.name, l.name));
                }
            }
        }
    }
    bad
}

/// A pin whose peers on the sheet are all in its own column goes on the right side, except in the last column, where it goes on the left.
/// シートの上の相手が全て同じ列にあるピンは右の辺に置く。ただし最後の列では左の辺に置く。
#[test]
fn a_pin_with_peers_only_in_its_own_column_goes_right_except_in_the_last_column() {
    for (columns, want) in [(vec![&STACK[..], &["SY"][..]], "right"), (vec![&["SY"][..], &STACK[..]], "left"), (vec![&STACK[..]], "left")] {
        let p = plan_function_sheet(&stacked_source(), &stacked_open(), &stacked_request(&columns, 5.0)).expect("the stack fits");
        for (r, k) in [("SA", "1"), ("SA", "2"), ("SA", "3"), ("SB", "1"), ("SB", "2"), ("SC", "1"), ("SC", "3"), ("SD", "3")] {
            assert_eq!(side_of(&p.sheet, r, k), want, "{columns:?}: {r}.{k}");
        }
    }
}

/// In a mixed case the peers in the own column do not vote: one peer to the left and two in the own column put the pin on the left, also in the last column.
/// 相手が混ざるときは、同じ列の相手は数えない。左に1つ・同じ列に2つの相手を持つピンは、最後の列でも左に置く。
#[test]
fn same_column_peers_do_not_vote_in_a_mixed_case() {
    // SB.4 (K7): SY on the left, SC.5 and SD.5 in its own column; SX makes the stack not the last column
    let req = stacked_request(&[&["SY"], &STACK, &["SX"]], 5.0);
    let p = plan_function_sheet(&stacked_source(), &stacked_open(), &req).expect("fits");
    assert_eq!(side_of(&p.sheet, "SB", "4"), "left", "K7: SY is in another column, so the own-column peers do not vote");
    assert_eq!(side_of(&p.sheet, "SA", "5"), "left", "K6 has its only peer to the left");
    let req = stacked_request(&[&["SY"], &STACK], 5.0);
    let p = plan_function_sheet(&stacked_source(), &stacked_open(), &req).expect("fits");
    assert_eq!(side_of(&p.sheet, "SB", "4"), "left", "in the last column same-column peers count as left");
}

/// The channel to the right of a stacked column holds the tracks of the nets that join its units, so its width follows the tracks needed (nets far apart share a track), not the number of nets.
/// 積んだ列の右の通り道には、その列の機器どうしをつなぐネットのトラックが入るので、幅は必要なトラックの数に従う (離れたネットは1つのトラックを分け合う)。ネットの数ではない。
#[test]
fn the_channel_counts_the_nets_inside_a_column() {
    let req = stacked_request(&[&STACK, &["SY"]], 5.0);
    let m = model_of(&stacked_source(), &stacked_open(), &req);
    let l = lay_out(&m, &stacked_open(), &req).expect("fits");
    // K1..K5 join the units of the stack, K6 and K7 reach SY: seven nets
    let tracks = l.channels[0].tracks;
    assert!((1..7).contains(&tracks), "seven nets share tracks: {tracks}");
    let want = grid_up(f64::max(20.0, 10.0 + 2.5 * tracks as f64));
    assert!((l.columns[1].left - l.columns[0].right - want).abs() < 1e-6, "{:?} want {want}", l.columns);
}

/// Drafting the stacked fixture, the core fixture and the bars fixture keeps the connections: the device pin groups of the sheet are those of the source, at both pitches, also when the router gives up.
/// 積んだ見本・基本の見本・棒の見本を下書きしても、つながりは変わらない。シートの機器のピンのまとまりは、どちらのピン間隔でも、経路をあきらめたときも、元の図と同じ。
#[test]
fn the_pin_groups_of_drafted_sheets_equal_the_source() {
    let narrow = RouteOptions { frame_margin_mm: 100.0, ..RouteOptions::default() };
    for pitch in [5.0, 2.5] {
        for opts in [RouteOptions::default(), narrow.clone()] {
            for cols in stacked_columns() {
                let src = stacked_source();
                let p = plan_with_options(&src, &stacked_open(), &stacked_request(&cols, pitch), &opts).expect("fits");
                let refs: Vec<&str> = cols.iter().flat_map(|c| c.iter().copied()).collect();
                assert_eq!(device_groups(&p.sheet, &refs), device_groups(&src.sheets[0], &refs), "stacked {cols:?} pitch {pitch}");
            }
            let mut r = request(&[&["UA"], &["UB"], &["UC"]]);
            r.pitch_mm = pitch;
            let p = plan_with_options(&bars_source(), &open(), &r, &opts).expect("fits");
            let refs = ["UA", "UB", "UC"];
            assert_eq!(device_groups(&p.sheet, &refs), device_groups(&bars_source().sheets[0], &refs), "bars pitch {pitch}");
            let mut r = request(&[&["U1", "M1"], &["U2"], &["TB1", "R1"]]);
            r.ground_nets = None;
            r.pitch_mm = pitch;
            let p = plan_with_options(&fixture_source(), &open(), &r, &opts).expect("fits");
            let refs = ["U1", "M1", "U2", "TB1", "R1"];
            assert_eq!(device_groups(&p.sheet, &refs), device_groups(&fixture_source().sheets[0], &refs), "core pitch {pitch}");
        }
    }
}

/// No wire crosses or touches a tag flag, drawn with the destination the export prints, on the stacked fixture (units stacked in one column wired to each other, tags with two sheet names) and on the core fixture, at both pitches and when the router gives up on nets (fallback tags are obstacles too).
/// 札の旗 (書き出しが書く行き先を入れたもの) を横切る配線も触れる配線も無い。1つの列に積んで互いに配線した機器と2枚のシート名を持つ札の見本でも、基本の見本でも、どちらのピン間隔でも、経路をあきらめたネットがあるとき (代わりの札も障害物) でも同じ。
#[test]
fn no_wire_crosses_or_touches_a_tag_flag() {
    let narrow = RouteOptions { frame_margin_mm: 60.0, ..RouteOptions::default() };
    let mut labelled = 0;
    for pitch in [5.0, 2.5] {
        for opts in [RouteOptions::default(), narrow.clone()] {
            for cols in stacked_columns() {
                let open = stacked_open();
                let p = plan_with_options(&stacked_source(), &open, &stacked_request(&cols, pitch), &opts).expect("fits");
                labelled += p.labelled.len();
                let bad = wires_touching_flags(&with_sheet(&open, &p.sheet), &p.sheet);
                assert!(bad.is_empty(), "stacked {cols:?} pitch {pitch}: {bad:#?}");
            }
            for cols in [vec![vec!["U1".to_string(), "M1".into()], vec!["U2".into()], vec!["TB1".into(), "R1".into()]], vec![vec!["U1".into()], vec!["U2".into()]]] {
                let mut r = request(&[]);
                r.columns = cols.clone();
                r.ground_nets = None;
                r.pitch_mm = pitch;
                let p = plan_with_options(&fixture_source(), &open(), &r, &opts).expect("fits");
                let bad = wires_touching_flags(&with_sheet(&open(), &p.sheet), &p.sheet);
                assert!(bad.is_empty(), "core {cols:?} pitch {pitch}: {bad:#?}");
            }
        }
    }
    assert!(labelled > 0, "the narrow frame makes the router give up on some nets, so fallback tags are checked");
}

/// A fallback tag hangs at the upper end of its group's bar, so the bar leaves the flag at the tag's point only.
/// 経路をあきらめたネットの札は組の棒の上の端に下がるので、棒は札の点でしか旗に触れない。
#[test]
fn a_fallback_tag_hangs_at_the_upper_end_of_its_bar() {
    let opts = RouteOptions { frame_margin_mm: 100.0, ..RouteOptions::default() };
    let p = plan_with_options(&bars_source(), &open(), &request(&[&["UA"], &["UB"]]), &opts).expect("valid");
    assert!(p.labelled.contains(&"BUS".to_string()), "{:?}", p.labelled);
    let tags: Vec<Point> = labels_of(&p.sheet).into_iter().filter(|l| l.name == "BUS").map(|l| l.at).collect();
    for (r, top) in [("UA", "10"), ("UB", "4")] {
        let e = stub_end(&p.sheet, r, top);
        assert!(tags.iter().any(|t| t.distance_to(&e) < 1e-6), "{r}.{top}: {tags:?}");
    }
    let bad = wires_touching_flags(&with_sheet(&open(), &p.sheet), &p.sheet);
    assert!(bad.is_empty(), "{bad:#?}");
}

/// The router terminal of every group is the bar end nearest to its peers: the upper end when all peers lie above or level, the lower end when all lie below, the middle (on the grid) when they lie above and below.
/// どの組の配線の端も、相手にいちばん近い棒の端になる。相手が全て上か同じ高さなら上の端、全て下なら下の端、上にも下にもあれば真ん中 (格子の上)。
#[test]
fn every_group_terminal_is_the_bar_end_nearest_to_its_peers() {
    let mut seen = BTreeSet::new();
    for (src, open, req) in [
        (stacked_source(), stacked_open(), stacked_request(&[&STACK, &["SY"]], 5.0)),
        (stacked_source(), stacked_open(), stacked_request(&[&["SY"], &STACK, &["SX"]], 5.0)),
        (bars_source(), self::open(), request(&[&["UA"], &["UB"]])),
    ] {
        let m = model_of(&src, &open, &req);
        let l = lay_out(&m, &open, &req).expect("fits");
        for u in &m.units {
            let mut by: BTreeMap<usize, Vec<Point>> = BTreeMap::new();
            for p in &u.pins {
                if matches!(m.nets[p.net].class, NetClass::Internal | NetClass::Leaving) {
                    by.entry(p.net).or_default().push(stub_end(&l.sheet, &u.reference, &p.key));
                }
            }
            for (net, mut ends) in by {
                let mut sides: BTreeMap<i64, Vec<Point>> = BTreeMap::new();
                for e in ends.drain(..) {
                    sides.entry((e.x * 1000.0).round() as i64).or_default().push(e);
                }
                for ends in sides.values() {
                    let (top, bottom) = (ends.iter().map(|e| e.y).fold(f64::MAX, f64::min), ends.iter().map(|e| e.y).fold(f64::MIN, f64::max));
                    if ends.len() < 2 {
                        continue;
                    }
                    let x = ends[0].x;
                    let Some(t) = l.terminals.iter().find(|t| t.net == net && !t.is_tag && (t.at.x - x).abs() < 1e-6 && t.at.y >= top - 1e-6 && t.at.y <= bottom + 1e-6) else { continue };
                    let peers: Vec<f64> = m
                        .units
                        .iter()
                        .filter(|u2| u2.reference != u.reference)
                        .flat_map(|u2| u2.pins.iter().filter(|p| p.net == net).map(|p| pin_pt(&l.sheet, &u2.reference, &p.key).y))
                        .collect();
                    let (above, below) = (peers.iter().any(|&y| y < top - 1e-9), peers.iter().any(|&y| y > bottom + 1e-9));
                    let want = match (above, below) {
                        (true, true) => grid_down((top + bottom) / 2.0),
                        (false, true) => bottom,
                        _ => top,
                    };
                    assert!((t.at.y - want).abs() < 1e-6, "{} {}: terminal {:?}, want y {want} (bar {top}..{bottom})", u.reference, m.nets[net].name, t.at);
                    seen.insert((above, below));
                }
            }
        }
    }
    assert!(seen.contains(&(false, true)), "the fixtures hold a group whose peers are all below: {seen:?}");
}

/// Source for the marks: one unit with two power nets (P24X on pins 1 and 2, P12X on pin 3) and two ground nets (GND on pins 4 and 5, 0V on pin 6), all on its left side.
fn marks_source() -> Project {
    source_of(
        "draft-marks",
        &[("UM", 6)],
        &[("P24X", &[("UM", "1"), ("UM", "2")]), ("P12X", &[("UM", "3")]), ("GND", &[("UM", "4"), ("UM", "5")]), ("0V", &[("UM", "6")])],
    )
}

/// The printed text of each mark is clear of every wire (bars, stubs, mark leads, routed wires) and of every pin number, and no two marks touch, for power and ground groups, at pitch 5.0 and 2.5, also with two power and two ground groups on one side.
/// どの印の文字も、全ての配線 (棒・短い線・印の引き出し・引いた配線) とピン番号にかからず、印どうしも触れない。電源の組でも接地の組でも、ピン間隔5.0でも2.5でも、1つの辺に電源と接地の組が2つずつあっても同じ。
#[test]
fn mark_texts_are_clear_of_wires_and_pin_numbers() {
    let mut checked = 0;
    for pitch in [5.0, 2.5] {
        let mut marks_req = request(&[&["UM"]]);
        marks_req.power_nets = vec!["P24X".into(), "P12X".into()];
        marks_req.ground_nets = Some(vec!["GND".into(), "0V".into()]);
        marks_req.pitch_mm = pitch;
        let mut bars_req = request(&[&["UA"], &["UB"], &["UC"]]);
        bars_req.pitch_mm = pitch;
        for (src, req, refs) in [(marks_source(), marks_req, vec!["UM"]), (bars_source(), bars_req, vec!["UA", "UB", "UC"])] {
            let p = plan_function_sheet(&src, &open(), &req).expect("fits");
            let defs = sheet_symbol_defs(&p.sheet);
            let texts = text_boxes(&p.sheet, &defs);
            let numbers: Vec<(String, Rect)> = refs.iter().flat_map(|r| number_rects(&p.sheet, r)).collect();
            let all = marks_of(&p.sheet);
            for (i, a) in all.iter().enumerate() {
                for b in &all[i + 1..] {
                    assert!(!mark_rect(&p.sheet, a).overlaps(&mark_rect(&p.sheet, b)), "pitch {pitch}: marks {} and {} touch", a.value, b.value);
                }
            }
            for m in marks_of(&p.sheet) {
                for t in texts.iter().filter(|t| t.owner == Some(m.id)) {
                    checked += 1;
                    for w in wires_of(&p.sheet) {
                        for s in w.points.windows(2) {
                            assert!(!t.rect.overlaps(&seg_rect(s[0], s[1])), "pitch {pitch}: text {} of a mark at {:?} on the wire {:?}-{:?}", t.text, m.at, s[0], s[1]);
                        }
                    }
                    for (n, r) in &numbers {
                        assert!(!t.rect.overlaps(r), "pitch {pitch}: text {} on the pin number {n}", t.text);
                    }
                }
                // the figure of a mark touches no wire but its own lead
                let fig = crate::tidy::symbol_bounds(&m, &DefIndex::new(&defs).get(&m).unwrap()).unwrap();
                for w in wires_of(&p.sheet) {
                    if w.points.last().is_some_and(|q| q.distance_to(&m.at) < 1e-6) {
                        continue;
                    }
                    for s in w.points.windows(2) {
                        assert!(!fig.overlaps(&seg_rect(s[0], s[1])), "pitch {pitch}: mark {} at {:?} on the wire {:?}-{:?}", m.value, m.at, s[0], s[1]);
                    }
                }
            }
        }
    }
    assert!(checked >= 8, "{checked}");
}

/// On a drafted sheet no printed pin number touches the outline of its box or any pin name, also with a six-character number and at pitch 2.5 (the boxes end their numbers at the box edge).
/// 下書きのシートでは、印字されたピン番号が箱の外形にもピン名にも触れない。6文字の番号でも、ピン間隔2.5でも同じ (箱は番号を辺の外で終わらせる)。
#[test]
fn no_pin_number_touches_the_box_outline_or_a_pin_name() {
    for pitch in [5.0, 2.5] {
        let mut r = request(&[&["UA"], &["UB"], &["UC"]]);
        r.pitch_mm = pitch;
        let p = plan(&r);
        for reference in ["UA", "UB", "UC"] {
            let inst = unit_of(&p.sheet, reference);
            assert_eq!(inst.attrs.get(crate::block::BLOCK_NUMBER_ALIGN_ATTR).map(String::as_str), Some("edge"));
            let spec = block::parse_block_spec(&inst.attrs).unwrap();
            let (lo, hi) = block::block_outline(&spec);
            let (lo, hi) = (Point::new(inst.at.x + lo.x, inst.at.y + lo.y), Point::new(inst.at.x + hi.x, inst.at.y + hi.y));
            let defs = sheet_symbol_defs(&p.sheet);
            let def = DefIndex::new(&defs).get(&inst).unwrap();
            let names: Vec<Rect> = def
                .primitives
                .iter()
                .filter_map(|q| match q {
                    crate::symbol::Primitive::Text { at, text, height, .. } if at.x > lo.x - inst.at.x && at.x < hi.x - inst.at.x => {
                        let (hw, hh) = (text_width(text, *height) / 2.0, height / 2.0);
                        Some(Rect::from_corners(Point::new(inst.at.x + at.x - hw, inst.at.y + at.y - hh), Point::new(inst.at.x + at.x + hw, inst.at.y + at.y + hh)))
                    }
                    _ => None,
                })
                .collect();
            for (n, r) in number_rects(&p.sheet, reference) {
                assert!(r.max.x < lo.x - 1e-6 || r.min.x > hi.x + 1e-6, "pitch {pitch}: number {n} of {reference} touches the box {r:?}");
                for name in &names {
                    assert!(!r.overlaps(name), "pitch {pitch}: number {n} of {reference} touches a pin name");
                }
            }
        }
    }
}

// ---- side per (unit, net)

/// A source of three units A, B, C (four pins each) and the given nets, drafted in the given columns without power or ground nets.
fn sides_of(nets: &[(&str, &[(&str, &str)])], columns: &[&[&str]]) -> DraftPlan {
    let src = source_of("draft-sides", &[("A", 4), ("B", 4), ("C", 4), ("D", 4)], nets);
    plan_function_sheet(&src, &open(), &stacked_request(columns, 5.0)).expect("the units fit")
}

/// Two pins of one net on one unit whose peers lie in columns on both sides go on the same side, in adjacent rows, joined by one bar: no pin of the net is on the other side of the box.
/// 左の列にも右の列にも相手がいるネットの、1つの機器にある2本のピンは、同じ辺の隣の行に並んで1本の棒でつながる。反対側の辺にそのネットのピンは出ない。
#[test]
fn pins_of_one_net_on_one_unit_share_one_side_and_one_bar() {
    let p = sides_of(&[("N", &[("A", "1"), ("B", "1"), ("B", "2"), ("C", "1")]), ("M", &[("B", "3"), ("C", "2")])], &[&["A"], &["B"], &["C"]]);
    let (s1, s2) = (side_of(&p.sheet, "B", "1"), side_of(&p.sheet, "B", "2"));
    assert_eq!(s1, s2);
    let (e1, e2) = (stub_end(&p.sheet, "B", "1"), stub_end(&p.sheet, "B", "2"));
    assert!(on_a_bar(&p.sheet, e1, e2), "one bar joins the two stub ends");
    assert!((pin_pt(&p.sheet, "B", "2").y - pin_pt(&p.sheet, "B", "1").y - 5.0).abs() < 1e-6, "adjacent rows");
}

/// With peers in other columns, only those decide: all to the left goes left, all to the right goes right.
/// 相手がほかの列にいるときは、その相手だけで決める。全て左の列なら左、全て右の列なら右に置く。
#[test]
fn the_side_follows_the_peers_in_other_columns() {
    let p = sides_of(&[("N", &[("A", "1"), ("B", "1")]), ("M", &[("B", "2"), ("C", "1")])], &[&["A"], &["B"], &["C"]]);
    assert_eq!(side_of(&p.sheet, "B", "1"), "left");
    assert_eq!(side_of(&p.sheet, "B", "2"), "right");
}

/// With peers on both sides the side with more peers wins, and a tie goes left.
/// 左右の両方に相手がいるときは、相手の多い側に置き、同数なら左に置く。
#[test]
fn the_side_with_more_peers_wins_and_a_tie_goes_left() {
    let more_right = sides_of(&[("N", &[("B", "1"), ("A", "1"), ("C", "1"), ("C", "2")]), ("M", &[("A", "2"), ("B", "2")])], &[&["A"], &["B"], &["C"]]);
    assert_eq!(side_of(&more_right.sheet, "B", "1"), "right");
    let tie = sides_of(&[("N", &[("B", "1"), ("A", "1"), ("C", "1")]), ("M", &[("A", "2"), ("B", "2")])], &[&["A"], &["B"], &["C"]]);
    assert_eq!(side_of(&tie.sheet, "B", "1"), "left");
}

/// A pin whose peers are all in the unit's own column goes right, and left in the last column.
/// 相手が全て同じ列の機器にあるピンは右に置く。最後の列では左に置く。
#[test]
fn peers_only_in_the_own_column_put_the_pin_right_or_left_in_the_last_column() {
    let nets: &[(&str, &[(&str, &str)])] = &[("N", &[("A", "1"), ("B", "1")]), ("M", &[("C", "1"), ("C", "2"), ("A", "2")])];
    let p = sides_of(nets, &[&["A", "B"], &["C"]]);
    assert_eq!((side_of(&p.sheet, "A", "1"), side_of(&p.sheet, "B", "1")), ("right", "right"));
    let p = sides_of(&[("N", &[("A", "1"), ("B", "1")]), ("M", &[("C", "1"), ("C", "2")])], &[&["C"], &["A", "B"]]);
    assert_eq!((side_of(&p.sheet, "A", "1"), side_of(&p.sheet, "B", "1")), ("left", "left"));
}

/// Peers in another column decide even when more peers stand in the own column: two stacked units whose net also reaches a unit to the left are both on the left side.
/// 同じ列の相手の方が多くても、ほかの列の相手が決める。積んだ2つの機器のネットが左の列の機器にも届くなら、2つとも左の辺に置く。
#[test]
fn own_column_peers_do_not_vote_when_another_column_has_a_peer() {
    let nets: &[(&str, &[(&str, &str)])] = &[("N", &[("A", "1"), ("B", "1"), ("C", "1"), ("C", "2")])];
    let p = sides_of(nets, &[&["A"], &["B", "C"]]);
    assert_eq!((side_of(&p.sheet, "B", "1"), side_of(&p.sheet, "C", "2")), ("left", "left"));
    let with_d: &[(&str, &[(&str, &str)])] = &[("N", &[("A", "1"), ("B", "1"), ("C", "1"), ("C", "2")]), ("M", &[("A", "2"), ("D", "1")])];
    let p = sides_of(with_d, &[&["A"], &["B", "C"], &["D"]]);
    assert_eq!((side_of(&p.sheet, "B", "1"), side_of(&p.sheet, "C", "2")), ("left", "left"), "not the last column: the own-column peers still do not vote");
}

/// A jumper inside one unit (a net whose pins are all on that unit) keeps today's side: left, or right in the last column.
/// 1つの機器の中だけのジャンパ (ネットのピンが全てその機器にある) は従来どおりの辺に置く。左で、最後の列では右。
#[test]
fn a_jumper_inside_one_unit_goes_left_or_right_in_the_last_column() {
    let nets: &[(&str, &[(&str, &str)])] = &[("J", &[("B", "1"), ("B", "2")]), ("N", &[("A", "1"), ("C", "1")])];
    let p = sides_of(nets, &[&["A"], &["B"], &["C"]]);
    assert_eq!((side_of(&p.sheet, "B", "1"), side_of(&p.sheet, "B", "2")), ("left", "left"));
    let p = sides_of(&[("J", &[("B", "1"), ("B", "2")]), ("N", &[("A", "1"), ("C", "1")])], &[&["A"], &["C"], &["B"]]);
    assert_eq!((side_of(&p.sheet, "B", "1"), side_of(&p.sheet, "B", "2")), ("right", "right"));
}

// ---- the picture

/// Writes the bars fixture as a drafted sheet to the path in MADAKE_DRAFT_SVG (run with --ignored).
/// 見て確かめるために、棒の見本を下書きしたシートを MADAKE_DRAFT_SVG の場所へSVGで書き出す (--ignored で実行する)。
#[test]
#[ignore]
fn write_the_bars_sheet_as_svg() {
    let Ok(path) = std::env::var("MADAKE_DRAFT_SVG") else { return };
    let p = plan(&request(&[&["UA"], &["UB"], &["UC"]]));
    let mut project = open();
    project.sheets.clear();
    project.sheets.push(p.sheet.clone());
    let svg = crate::svg::project_sheet_to_svg(&project, p.sheet.id, &builtin_symbols()).unwrap();
    std::fs::write(path, svg).unwrap();
}



/// Writes the stacked fixture drafted twice (the stack before SY, and the stack in the last column) as SVG files into the directory in MADAKE_DRAFT_SVG_DIR (run with --ignored).
/// 見て確かめるために、積んだ見本を2通り (積んだ列がSYの前・最後の列) に下書きしたシートを、MADAKE_DRAFT_SVG_DIR のフォルダへSVGで書き出す (--ignored で実行する)。
#[test]
#[ignore]
fn write_the_stacked_sheets_as_svg() {
    let Ok(dir) = std::env::var("MADAKE_DRAFT_SVG_DIR") else { return };
    for (name, columns) in [("stacked-right", vec![&STACK[..], &["SY"][..]]), ("stacked-left", vec![&["SY"][..], &STACK[..]])] {
        let open = stacked_open();
        let p = plan_function_sheet(&stacked_source(), &open, &stacked_request(&columns, 5.0)).expect("fits");
        let project = with_sheet(&open, &p.sheet);
        let svg = crate::svg::project_sheet_to_svg(&project, p.sheet.id, &builtin_symbols()).unwrap();
        std::fs::write(format!("{dir}/{name}.svg"), svg).unwrap();
    }
}

// ---- looking at real data without the app

/// Drafts the requests of a JSON file one after another and writes the sheets as SVG for a look (run with --ignored; set MADAKE_DRAFT_REQUESTS and MADAKE_DRAFT_OUT).
/// 手元のファイルの依頼を順に下書きして、シートをSVGで書き出し、見て確かめる (--ignored で実行。環境変数 MADAKE_DRAFT_REQUESTS と MADAKE_DRAFT_OUT を指定する。無ければ何もしない)。
#[test]
#[ignore]
fn draft_requests_from_a_file_for_looking() {
    let (Ok(requests), Ok(out)) = (std::env::var("MADAKE_DRAFT_REQUESTS"), std::env::var("MADAKE_DRAFT_OUT")) else { return };
    #[derive(serde::Deserialize)]
    struct File {
        source_path: String,
        requests: Vec<DraftRequest>,
    }
    let file: File = serde_json::from_str(&std::fs::read_to_string(&requests).expect("the requests file reads")).expect("the requests file is JSON");
    let source = crate::io::load_project(std::path::Path::new(&file.source_path)).expect("the source loads");
    let mut project = Project::new("draft-look");
    project.sheets.clear();
    project.frame_template = source.frame_template.clone();
    project.text_font = source.text_font.clone();
    std::fs::create_dir_all(&out).expect("the output directory is made");
    let mut summary = Vec::new();
    for request in &file.requests {
        match plan_function_sheet(&source, &project, request) {
            Err(message) => summary.push(serde_json::json!({"name": request.name, "refused": message})),
            Ok(plan) => {
                project.sheets.push(plan.sheet.clone());
                summary.push(serde_json::json!({"name": request.name, "plan": plan}));
            }
        }
    }
    for (n, sheet) in project.sheets.iter().enumerate() {
        let svg = crate::svg::project_sheet_to_svg(&project, sheet.id, &sheet_symbol_defs(sheet)).expect("the sheet renders");
        std::fs::write(format!("{out}/{}.svg", n + 1), svg).expect("the SVG is written");
    }
    let check = crate::draft_check::check_function_sheets(&source, &project, &[]);
    let summary = serde_json::json!({"requests": summary, "check": check});
    std::fs::write(format!("{out}/summary.json"), serde_json::to_string_pretty(&summary).unwrap()).expect("the summary is written");
}
