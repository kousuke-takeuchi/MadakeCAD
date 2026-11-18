//! 機能シートの下書きの、通り道の幹の延長 (他の列を通る枝) と、幹につける札のテスト。元の図は、この中で組み立てる作り物の機器。
use super::channel::*;
use super::look_tests::*;
use super::*;
use std::collections::BTreeSet;

// ---- fixtures

/// A bus over three columns: the connector `C1` (column 1, two pins on the bus), two small units `S1` and `S2` (column 2) and two large units `L1` and `L2`
/// (column 3). The large units start with two power pins, so their bus pin sits low, level with the free gap between S1 and S2 in column 2.
/// With `blocked`, S1 also carries a jumper between two more pins, so its box grows down over the row of the bus pin of L1.
pub(crate) fn bus_fixture(blocked: bool) -> Project {
    let mut nets: Vec<(&str, Vec<(&str, &str)>)> = vec![
        ("BUS", vec![("C1", "1"), ("C1", "2"), ("S1", "1"), ("S2", "1"), ("L1", "3"), ("L2", "3")]),
        ("K1", vec![("C1", "3"), ("S1", "2")]),
        ("K2", vec![("C1", "4"), ("S2", "2")]),
        ("M", vec![("S2", "3"), ("L2", "4")]),
        ("P24X", vec![("L1", "1"), ("L1", "2"), ("L2", "1"), ("L2", "2")]),
    ];
    if blocked {
        nets.push(("J", vec![("S1", "3"), ("S1", "4")]));
    }
    let refs: Vec<(&str, &[(&str, &str)])> = nets.iter().map(|(n, m)| (*n, m.as_slice())).collect();
    source_of("draft-bus", &[("C1", 4), ("S1", if blocked { 4 } else { 2 }), ("S2", 3), ("L1", 4), ("L2", 4)], &refs)
}

pub(crate) fn bus_source() -> Project {
    bus_fixture(false)
}

pub(crate) fn bus_request() -> DraftRequest {
    let mut r = stacked_request(&[&["C1"], &["S1", "S2"], &["L1", "L2"]], 5.0);
    r.power_nets = vec!["P24X".into()];
    r
}

/// Two units in adjacent columns joined by a net `T` that also leaves the sheet (its third pin is on the unit `X`, which is not drafted), plus a plain net `P`.
pub(crate) fn tagged_source() -> Project {
    source_of(
        "draft-tagged",
        &[("A", 4), ("B", 4), ("X", 2)],
        &[("T", &[("A", "1"), ("B", "1"), ("X", "1")]), ("P", &[("A", "2"), ("B", "3")]), ("Z", &[("A", "3"), ("B", "2")])],
    )
}

/// Like [`tagged_source`], but the tagged net is the lowest of the three, so the free positions for its tag are below it.
pub(crate) fn tagged_low_source() -> Project {
    source_of(
        "draft-tagged-low",
        &[("A", 4), ("B", 4), ("X", 2)],
        &[("P", &[("A", "1"), ("B", "1")]), ("Z", &[("A", "2"), ("B", "2")]), ("T", &[("A", "3"), ("B", "3"), ("X", "1")])],
    )
}

/// The request for the tagged fixture. `planned_sheets` names only this sheet, so no other sheet will print its name in a flag and a flag needs room for its own name only.
pub(crate) fn tagged_request() -> DraftRequest {
    let mut r = stacked_request(&[&["A"], &["B"]], 5.0);
    r.planned_sheets = vec!["Look".into()];
    r
}

// ---- the pictures

/// Writes every picture of the span work as SVG into the directory in MADAKE_DRAFT_SVG_DIR (run with --ignored).
/// 見て確かめるために、通り道の幹と札の見本を下書きしたシートを、MADAKE_DRAFT_SVG_DIR のフォルダへSVGで書き出す (--ignored で実行する)。
#[test]
#[ignore]
fn write_the_span_sheets_as_svg() {
    let Ok(dir) = std::env::var("MADAKE_DRAFT_SVG_DIR") else { return };
    let mut write = |name: &str, src: &Project, open: &Project, req: &DraftRequest| {
        let p = plan_function_sheet(src, open, req).expect("fits");
        let project = with_sheet(open, &p.sheet);
        let svg = crate::svg::project_sheet_to_svg(&project, p.sheet.id, &crate::symbol::builtin_symbols()).unwrap();
        std::fs::write(format!("{dir}/{name}.svg"), svg).unwrap();
    };
    let mut golden = request(&[&["U1", "M1"], &["U2"], &["TB1", "R1"]]);
    golden.ground_nets = None;
    write("golden", &fixture_source(), &open(), &golden);
    write("bars", &bars_source(), &open(), &request(&[&["UA"], &["UB"], &["UC"]]));
    for (name, columns) in [("stacked-right", vec![&STACK[..], &["SY"][..]]), ("stacked-left", vec![&["SY"][..], &STACK[..]])] {
        write(name, &stacked_source(), &stacked_open(), &stacked_request(&columns, 5.0));
    }
    write("bus", &bus_source(), &open(), &bus_request());
    write("tagged", &tagged_source(), &open(), &tagged_request());
}


// ---- helpers

use super::channel_tests::{drafted, settled, trunks};
use crate::route::RouteOptions;

fn net_key(m: &DraftModel, name: &str) -> usize {
    m.nets.iter().position(|n| n.name == name).expect("the net exists")
}

fn bus_layout(blocked: bool) -> (Project, DraftRequest, DraftModel, DraftLayout, DraftPlan) {
    let src = bus_fixture(blocked);
    let req = bus_request();
    let m = build_model(&src, &open(), &req).expect("valid");
    let l = settled(&m, &req);
    let p = plan_function_sheet(&src, &open(), &req).expect("plan");
    (src, req, m, l, p)
}

/// The number of wire segments of the sheet that run through the inside of a box of one of `refs`.
fn wires_through_boxes(sheet: &Sheet, refs: &[&str]) -> usize {
    let mut n = 0;
    for r in refs {
        let b = bounds_of(sheet, r);
        let inner = Rect::from_corners(Point::new(b.min.x + 0.3, b.min.y + 0.3), Point::new(b.max.x - 0.3, b.max.y - 0.3));
        for w in wires_of(sheet) {
            n += w.points.windows(2).filter(|s| seg_rect(s[0], s[1]).overlaps(&inner)).count();
        }
    }
    n
}

fn wire_key(p: &DraftPlan) -> Vec<String> {
    let mut v: Vec<String> = wires_of(&p.sheet).iter().map(|w| format!("{:?}", w.points)).collect();
    v.extend(junctions_of(&p.sheet).iter().map(|j| format!("J{j:?}")));
    v.extend(labels_of(&p.sheet).iter().map(|l| format!("L{} {:?} {:?}", l.name, l.at, l.attrs)));
    v.sort();
    v
}

const BUS_REFS: [&str; 5] = ["C1", "S1", "S2", "L1", "L2"];

// ---- change 1: a trunk with drops through other columns

/// A bus with terminals in three columns is drawn as one trunk in the channel between columns 1 and 2, with one straight horizontal drop per terminal, and no wire runs over a box.
/// 3つの列に端があるバスは、列1と列2のあいだの通り道に幹1本で引かれ、端ごとに1本のまっすぐな横の枝が付く。どの箱の上も配線は通らない。
#[test]
fn a_bus_over_three_columns_is_one_trunk_with_straight_drops() {
    let (src, _, m, l, p) = bus_layout(false);
    let bus = net_key(&m, "BUS");
    assert!(l.channels[0].placed.iter().any(|pl| pl.key == bus && pl.track.is_some()), "{:?}", l.channels);
    assert!(l.channels[1].placed.iter().all(|pl| pl.key != bus));
    let ts: Vec<Point> = l.terminals.iter().filter(|t| t.net == bus).map(|t| t.at).collect();
    assert_eq!(ts.len(), 5, "one terminal per pin group");
    let (lo, hi) = (ts.iter().map(|t| t.y).fold(f64::MAX, f64::min), ts.iter().map(|t| t.y).fold(f64::MIN, f64::max));
    let trunk: Vec<_> = trunks(&p.sheet, &l, 0).into_iter().filter(|t| (t.1 - lo).abs() < 1e-6 && (t.2 - hi).abs() < 1e-6).collect();
    assert_eq!(trunk.len(), 1, "one trunk from the top terminal to the lowest: {:?}", trunks(&p.sheet, &l, 0));
    let x = trunk[0].0;
    for t in &ts {
        let drop = wires_of(&p.sheet).into_iter().find(|w| w.points.len() == 2 && w.points.iter().any(|q| q.distance_to(t) < 1e-6) && w.points.iter().any(|q| q.distance_to(&Point::new(x, t.y)) < 1e-6));
        assert!(drop.is_some(), "a straight drop from {t:?} to the trunk at x={x}");
    }
    assert_eq!(wires_through_boxes(&p.sheet, &BUS_REFS), 0);
    assert!(p.wired.contains(&"BUS".to_string()) && p.labelled.is_empty(), "{:?} {:?}", p.wired, p.labelled);
    assert_eq!(device_groups(&p.sheet, &BUS_REFS), device_groups(&src.sheets[0], &BUS_REFS));
    assert!(wires_touching_flags(&with_sheet(&open(), &p.sheet), &p.sheet).is_empty());
}

/// When a box of column 2 stands in the row of a drop, the bus is not drawn on a trunk: the grid router takes it, nothing runs over a box, and the connections stay the same.
/// 2列目の箱が枝の高さに立ちふさがるときは、バスは幹で引かれない。格子の経路探索が引き、箱の上は通らず、つながりは変わらない。
#[test]
fn a_box_in_the_row_of_a_drop_sends_the_bus_to_the_grid_router() {
    let (src, _, m, l, p) = bus_layout(true);
    let bus = net_key(&m, "BUS");
    let row = l.terminals.iter().find(|t| t.net == bus && m.units[t.u].reference == "L1").expect("L1 is on the bus").at.y;
    let s1 = bounds_of(&p.sheet, "S1");
    assert!(s1.min.y < row && row < s1.max.y + 2.5, "S1 stands in the row {row}: {s1:?}");
    assert!(l.channels.iter().all(|c| c.placed.iter().all(|pl| pl.key != bus)), "no channel carries the bus: {:?}", l.channels);
    assert!(p.wired.contains(&"BUS".to_string()) && p.labelled.is_empty(), "{:?} {:?}", p.wired, p.labelled);
    assert_eq!(wires_through_boxes(&p.sheet, &BUS_REFS), 0);
    assert_eq!(device_groups(&p.sheet, &BUS_REFS), device_groups(&src.sheets[0], &BUS_REFS));
}

/// A net is a candidate for a channel when every terminal is on the right side of a unit in that column or before, or on the left side of a unit after it; its drops run through the channels in between.
/// 端が全て、通り道より前の列の機器の右辺か、後ろの列の機器の左辺にあるネットは、その通り道の候補になる。枝は間の通り道を通り抜ける。
#[test]
fn a_net_is_a_candidate_for_every_channel_that_all_its_terminals_face() {
    let (_, _, m, l, _) = bus_layout(false);
    let cands = candidates(&m, &l.terminals);
    let bus = net_key(&m, "BUS");
    let mine: Vec<&Candidate> = cands.iter().filter(|c| c.net == bus).collect();
    assert_eq!(mine.len(), 1, "only channel 0 faces all five terminals");
    assert_eq!(mine[0].channel, 0);
    assert_eq!(mine[0].through.len(), 2, "the drops of L1 and L2 run through channel 1");
    assert!(mine[0].through.iter().all(|(j, _)| *j == 1));
    // a net inside the last column (both pins on the left side) could use either channel
    let src = source_of("draft-near", &[("R", 2), ("T", 2), ("P", 2), ("Q", 2)], &[("Z", &[("R", "1"), ("T", "1")]), ("W", &[("P", "1"), ("Q", "1")])]);
    let req = stacked_request(&[&["R"], &["T"], &["P", "Q"]], 5.0);
    let m = build_model(&src, &open(), &req).expect("valid");
    let l = lay_out(&m, &open(), &req).expect("fits");
    let w = net_key(&m, "W");
    let channels: Vec<usize> = candidates(&m, &l.terminals).iter().filter(|c| c.net == w).map(|c| c.channel).collect();
    assert_eq!(channels, vec![0, 1]);
}

/// Of several candidate channels the one with the shortest drops is taken, not the lowest: two stacked units in the last column use the channel next to them; in the first column, the channel next to that.
/// 候補の通り道が複数あるときは、番号が小さい通り道ではなく、枝の合計がいちばん短い通り道を使う。最後の列に積んだ2つの機器は隣の通り道、最初の列に積んだ2つは、その隣の通り道を使う。
#[test]
fn the_channel_with_the_shortest_drops_wins() {
    for (columns, want) in [(vec![vec!["R"], vec!["T"], vec!["P", "Q"]], 1usize), (vec![vec!["P", "Q"], vec!["R"], vec!["T"]], 0)] {
        let src = source_of("draft-near", &[("R", 2), ("T", 2), ("P", 2), ("Q", 2)], &[("Z", &[("R", "1"), ("T", "1")]), ("W", &[("P", "1"), ("Q", "1")])]);
        let cols: Vec<&[&str]> = columns.iter().map(|c| c.as_slice()).collect();
        let req = stacked_request(&cols, 5.0);
        let m = build_model(&src, &open(), &req).expect("valid");
        let l = lay_out(&m, &open(), &req).expect("fits");
        let w = net_key(&m, "W");
        let at: Vec<usize> = l.channels.iter().enumerate().filter(|(_, c)| c.placed.iter().any(|pl| pl.key == w)).map(|(k, _)| k).collect();
        assert_eq!(at, vec![want], "{columns:?}");
    }
}

/// With equal drop lengths the lower channel wins, and a channel that was ruled out is skipped for the next one.
/// 枝の合計が同じなら小さい番号の通り道を選び、外された通り道は飛ばして次の通り道を選ぶ。
#[test]
fn equal_lengths_take_the_lower_channel_and_an_excluded_channel_is_skipped() {
    let term = |x: f64, left: bool| ChanTermAt { at: Point::new(x, 10.0), left };
    let cand = |channel: usize, xs: [f64; 2]| Candidate { channel, net: 7, name: "N".into(), terms: vec![term(xs[0], true), term(xs[1], false)], tagged: false, through: Vec::new() };
    let cands = [cand(0, [0.0, 40.0]), cand(1, [0.0, 40.0])];
    let trunk_x = |_: usize| 20.0;
    assert_eq!(choose_channels(&cands, &BTreeSet::new(), &trunk_x).get(&7), Some(&0));
    let out: BTreeSet<(usize, usize)> = [(7, 0)].into_iter().collect();
    assert_eq!(choose_channels(&cands, &out, &trunk_x).get(&7), Some(&1));
    let shorter = |c: usize| if c == 1 { 20.0 } else { 5.0 };
    assert_eq!(choose_channels(&cands, &BTreeSet::new(), &shorter).get(&7), Some(&0), "5 and 35 are 40 mm in all; 20 and 20 are 40 mm too, and the lower channel wins");
    let uneven = [cand(0, [0.0, 40.0]), cand(1, [30.0, 40.0])];
    let x_near_right = |_: usize| 35.0;
    assert_eq!(choose_channels(&uneven, &BTreeSet::new(), &x_near_right).get(&7), Some(&1), "the shorter sum wins over the lower index");
}

/// A drop that crosses another channel crosses that channel's trunk at a right angle, in the middle of it, and never ends on a trunk position; the width of that channel still follows its own tracks.
/// 他の通り道を通り抜ける枝は、その通り道の幹を途中で直角に横切るだけで、幹の端には乗らない。その通り道の幅は、その通り道自身のトラックの数のままになる。
#[test]
fn a_drop_through_another_channel_crosses_its_trunk_at_a_right_angle() {
    let (_, _, m, l, p) = bus_layout(false);
    let bus = net_key(&m, "BUS");
    let m_trunk = trunks(&p.sheet, &l, 1);
    assert_eq!(m_trunk.len(), 1, "M is the only trunk of channel 1: {m_trunk:?}");
    let y = l.terminals.iter().find(|t| t.net == bus && m.units[t.u].reference == "L2").expect("L2 is on the bus").at.y;
    let (x, y0, y1) = m_trunk[0];
    assert!(y0 + 1e-6 < y && y < y1 - 1e-6, "the drop at y={y} is in the middle of the trunk {y0}..{y1}");
    let drop = wires_of(&p.sheet).into_iter().find(|w| w.points.len() == 2 && (w.points[0].y - y).abs() < 1e-6 && w.points.iter().any(|q| q.x < x - 1.0) && w.points.iter().any(|q| q.x > x + 1.0)).expect("the drop runs across the trunk");
    assert_eq!(seg_rel((drop.points[0], drop.points[1]), (Point::new(x, y0), Point::new(x, y1))), Rel::Cross);
    assert!(junctions_of(&p.sheet).iter().all(|j| j.distance_to(&Point::new(x, y)) > 1e-6), "a plain crossing has no dot");
    let want = grid_up(f64::max(20.0, 10.0 + 2.5 * l.channels[1].tracks as f64));
    assert!((l.columns[2].left - l.columns[1].right - want).abs() < 1e-6, "{:?} want {want}", l.columns);
    assert_eq!(l.channels[1].tracks, 1);
}

/// In the track assignment of a channel, a drop that passes through it is a fixed line: it counts as a crossing, and a trunk may not end at its height.
/// 通り道のトラックの割り当てでは、その通り道を通り抜ける枝は動かない線として数える。幹の端がその枝の高さになる位置は選べない。
#[test]
fn a_through_drop_is_a_fixed_line_in_the_track_assignment() {
    let net = |a: f64, b: f64| vec![ChanNet { key: 0, name: "N".into(), terms: vec![ChanTerm { y: a, left: true }, ChanTerm { y: b, left: true }], tagged: false }];
    let crossing = assign_tracks_with(&net(10.0, 30.0), &[20.0]);
    assert_eq!((crossing.tracks, crossing.unplaced.len()), (1, 0), "{crossing:?}");
    let at_the_end = assign_tracks_with(&net(10.0, 30.0), &[30.0]);
    assert_eq!(at_the_end.unplaced, vec![0], "every position meets the drop at the end of the trunk: {at_the_end:?}");
    let two = vec![
        ChanNet { key: 0, name: "A".into(), terms: vec![ChanTerm { y: 10.0, left: true }, ChanTerm { y: 30.0, left: false }], tagged: false },
        ChanNet { key: 1, name: "B".into(), terms: vec![ChanTerm { y: 12.0, left: true }, ChanTerm { y: 28.0, left: false }], tagged: false },
    ];
    let a = assign_tracks_with(&two, &[20.0]);
    assert_eq!(a.tracks, 2, "two nested nets on two tracks, both crossed by the fixed line: {a:?}");
}

/// The same request planned twice gives the same wires, and so does the settled layout.
/// 同じ依頼を2回計画すると配線は全く同じで、決まった配置も同じになる。
#[test]
fn the_bus_sheet_is_the_same_every_time() {
    let (_, _, _, l1, p1) = bus_layout(false);
    let (_, _, _, l2, p2) = bus_layout(false);
    assert_eq!(wire_key(&p1), wire_key(&p2));
    assert_eq!(format!("{:?}", l1.channels), format!("{:?}", l2.channels));
    let t = |_: ()| plan_function_sheet(&tagged_source(), &open(), &tagged_request()).expect("plan");
    assert_eq!(wire_key(&t(())), wire_key(&t(())));
    let (_, _, _, _, b1) = bus_layout(true);
    let (_, _, _, _, b2) = bus_layout(true);
    assert_eq!(wire_key(&b1), wire_key(&b2));
}

// ---- change 2: a tag on the trunk

fn tag_named(sheet: &Sheet, name: &str) -> NetLabel {
    let all: Vec<NetLabel> = labels_of(sheet).into_iter().filter(|l| l.name == name).collect();
    assert_eq!(all.len(), 1, "one tag named {name}: {all:?}");
    all.into_iter().next().unwrap()
}

/// A leaving net that joins two units in adjacent columns is drawn on a trunk with its tag at the trunk's end: no tag branch at the pin, the flag clear of everything, and the sheet's netlist joins both pins and the tag name.
/// 出ていくネットが隣り合う列の2つの機器を結ぶとき、幹の端に札を付けて引く。ピンに札の枝は無く、旗は何にも触れず、シートのネットリストは両方のピンと札の名前をつなぐ。
#[test]
fn a_tagged_net_between_two_units_has_its_tag_at_the_end_of_the_trunk() {
    let src = tagged_source();
    let req = tagged_request();
    let m = build_model(&src, &open(), &req).expect("valid");
    let l = settled(&m, &req);
    let p = plan_function_sheet(&src, &open(), &req).expect("plan");
    let tag = tag_named(&p.sheet, "T");
    let pin = stub_end(&p.sheet, "A", "1");
    assert!(tag.at.x > l.columns[0].right && tag.at.x < l.columns[1].left, "the tag is in the channel: {:?}", tag.at);
    assert!((tag.at.y - (pin.y - 5.0)).abs() < 1e-6, "5 mm above the topmost terminal: {:?} {pin:?}", tag.at);
    assert_eq!(tag.attrs.get(crate::net_tag::TAG_ATTR).map(String::as_str), Some("left"), "the flag points right");
    assert_eq!(tag.attrs.get(crate::net_tag::FLOW_ATTR).map(String::as_str), Some("out"));
    let trunk: Vec<_> = trunks(&p.sheet, &l, 0).into_iter().filter(|t| (t.0 - tag.at.x).abs() < 1e-6).collect();
    assert_eq!(trunk.len(), 1);
    assert!((trunk[0].1 - tag.at.y).abs() < 1e-6, "the trunk ends at the tag point: {trunk:?}");
    assert!(labels_of(&p.sheet).iter().all(|l2| l2.at.distance_to(&Point::new(pin.x + 5.0, pin.y)) > 1e-6), "no tag branch at the pin");
    assert!(wires_of(&p.sheet).iter().all(|w| w.points.iter().all(|q| q.distance_to(&Point::new(pin.x + 5.0, pin.y)) > 1e-6 || q.x > pin.x + 6.0)), "no branch wire at the pin");
    assert!(wires_touching_flags(&with_sheet(&open(), &p.sheet), &p.sheet).is_empty());
    let defs = sheet_symbol_defs(&p.sheet);
    let nets = crate::netlist::extract_netlist(&p.sheet, &defs);
    let t = nets.iter().find(|n| n.pins.iter().any(|q| q.reference == "A" && q.pin == "1")).expect("the net of A.1");
    assert!(t.pins.iter().any(|q| q.reference == "B" && q.pin == "1"), "{t:?}");
    assert_eq!(t.label.as_deref(), Some("T"));
    assert_eq!(device_groups(&p.sheet, &["A", "B"]), device_groups(&src.sheets[0], &["A", "B"]));
    assert!(p.wired.contains(&"T".to_string()) && p.tagged.iter().any(|g| g.net == "T"));
}

/// Runs the position test on one source: `expected` lists the position numbers (0 = top 5 mm right, 1 = top 5 left, 2 = bottom 5 right, 3 = bottom 5 left, 4 to 7 the same at 10 mm) that are free, in the order they must be taken.
fn tag_positions_follow(src: &Project, req: &DraftRequest, expected: &[usize]) {
    let m = build_model(src, &open(), req).expect("valid");
    let first = plan_function_sheet(src, &open(), req).expect("plan");
    let free = tag_named(&first.sheet, "T");
    let a_row = labels_of(&first.sheet).len();
    assert!(a_row >= 1);
    let x = free.at.x;
    // the topmost and the lowest terminal of T are level here (both units carry T on the same row)
    let tl = settled(&m, req);
    let t_key = net_key(&m, "T");
    let rows: Vec<f64> = tl.terminals.iter().filter(|t| t.net == t_key && !t.is_tag).map(|t| t.at.y).collect();
    let (lo, hi) = (rows.iter().cloned().fold(f64::MAX, f64::min), rows.iter().cloned().fold(f64::MIN, f64::max));
    let position = |i: usize| -> (Point, bool) {
        let (ext, top, right) = (if i < 4 { 5.0 } else { 10.0 }, (i % 4) < 2, i % 2 == 0);
        (Point::new(x, if top { lo - ext } else { hi + ext }), right)
    };
    for k in 0..=expected.len() {
        let mut l = settled(&m, req);
        for &j in &expected[..k] {
            let (at, right) = position(j);
            let f = flag_rect(&trunk_tag("T", at, right, None), l.label_font, l.sheet.xref_style, l.reserve.text("T")).expect("a flag");
            // the half of the flag away from the trunk, so that the trunk end itself stays free
            let outer = if right { Rect::from_corners(Point::new(at.x + 2.0, f.min.y), Point::new(f.max.x, f.max.y)) } else { Rect::from_corners(Point::new(f.min.x, f.min.y), Point::new(at.x - 2.0, f.max.y)) };
            l.keep_out.push(outer);
        }
        let (sheet, wired, labelled) = route_wires(&m, &l, &RouteOptions::default());
        let tag = tag_named(&sheet, "T");
        assert!(wired.contains(&"T".to_string()) && labelled.is_empty(), "{wired:?} {labelled:?}");
        assert_eq!(device_groups(&sheet, &["A", "B"]), device_groups(&src.sheets[0], &["A", "B"]), "{k}");
        if k < expected.len() {
            let (at, right) = position(expected[k]);
            assert!(tag.at.distance_to(&at) < 1e-6, "{k} blocked: the tag is at {:?}, want position {} at {at:?}", tag.at, expected[k]);
            assert_eq!(tag.attrs.get(crate::net_tag::TAG_ATTR).map(String::as_str), Some(if right { "left" } else { "right" }), "{k}");
        } else {
            let pin = stub_end(&sheet, "A", "1");
            let pin = if lo > pin.y + 1.0 { stub_end(&sheet, "A", "3") } else { pin };
            assert!(tag.at.distance_to(&Point::new(pin.x + 5.0, pin.y)) < 1e-6, "all free positions blocked: the tag hangs on the branch at the pin: {:?} {pin:?}", tag.at);
        }
        assert!(wires_touching_flags(&with_sheet(&open(), &sheet), &sheet).is_empty(), "{k}");
    }
}

/// The tag positions are tried in a fixed order (top 5 mm right, top left, bottom right, bottom left, then the same four at 10 mm); a position where another net's wire is in the way is skipped. For a net at the top the free ones are the top positions; blocking each in turn moves the tag to the next, and with all blocked it stays on its branch at the pin.
/// 札の位置は決まった順に試す(上5mmの右・左、下の右・左、次に10mmで同じ順)。他のネットの配線がじゃまな位置は飛ばす。いちばん上のネットでは上の位置だけが空いていて、順にふさぐと札は次の位置へ動き、全部ふさぐとピンの枝に残る。
#[test]
fn the_tag_positions_of_a_top_net_are_tried_in_order_and_the_branch_is_the_last_resort() {
    tag_positions_follow(&tagged_source(), &tagged_request(), &[0, 1, 4, 5]);
}

/// For the lowest net the bottom positions are the free ones: right then left at 5 mm, then at 10 mm; with all blocked the tag stays on its branch.
/// いちばん下のネットでは、下の位置が空いている。5mmの右・左、次に10mmの右・左の順で、全部ふさぐと札は枝に残る。
#[test]
fn the_tag_positions_of_a_bottom_net_are_tried_in_order() {
    let mut req = tagged_request();
    req.planned_sheets = vec!["Look".into()];
    tag_positions_follow(&tagged_low_source(), &req, &[2, 3, 6, 7]);
}

/// Later nets keep clear of a tag that was moved to a trunk: with three tagged nets and two plain ones between the same two units, no wire of another net comes within 0.5 mm of any flag, and the connections stay the same.
/// 幹へ動かした札を、あとのネットは避ける。同じ2つの機器のあいだに、札付きのネット3本と普通のネット2本があっても、他のネットの配線は旗の0.5mm以内に近づかず、つながりは変わらない。
#[test]
fn later_nets_avoid_a_flag_that_moved_to_a_trunk() {
    let src = source_of(
        "draft-busy",
        &[("A", 6), ("B", 6), ("X", 3)],
        &[
            ("T1", &[("A", "1"), ("B", "2"), ("X", "1")]),
            ("T2", &[("A", "2"), ("B", "1"), ("X", "2")]),
            ("T3", &[("A", "3"), ("B", "3"), ("X", "3")]),
            ("P1", &[("A", "4"), ("B", "5")]),
            ("P2", &[("A", "5"), ("B", "4")]),
        ],
    );
    let mut req = stacked_request(&[&["A"], &["B"]], 5.0);
    req.planned_sheets = vec!["Look".into()];
    let p = plan_function_sheet(&src, &open(), &req).expect("plan");
    assert_eq!(device_groups(&p.sheet, &["A", "B"]), device_groups(&src.sheets[0], &["A", "B"]));
    assert!(wires_touching_flags(&with_sheet(&open(), &p.sheet), &p.sheet).is_empty());
    let project = with_sheet(&open(), &p.sheet);
    let xrefs = crate::xref::sheet_xrefs(&project, p.sheet.id);
    let font = crate::svg::LABEL_FONT * p.sheet.annotation_scale();
    let mut moved = 0;
    for l in labels_of(&p.sheet) {
        let Some(tag) = crate::net_tag::net_tag_with(&l, font, p.sheet.xref_style, xrefs.get(&l.id).map(String::as_str)) else { continue };
        let r = rect_of(&tag.points);
        let inflated = Rect::from_corners(Point::new(r.min.x - 0.5, r.min.y - 0.5), Point::new(r.max.x + 0.5, r.max.y + 0.5));
        let pin_a = stub_end(&p.sheet, "A", "1");
        if l.at.x > pin_a.x + 7.0 {
            moved += 1;
        }
        for w in wires_of(&p.sheet) {
            if w.points.iter().any(|q| q.distance_to(&l.at) < 1e-6) {
                continue;
            }
            for s in w.points.windows(2) {
                assert!(!seg_rect(s[0], s[1]).overlaps(&inflated), "{} flag {r:?}: wire {:?}-{:?}", l.name, s[0], s[1]);
            }
        }
    }
    assert!(moved >= 1, "at least one tag sits on a trunk");
}

/// With complete planned_sheets on a three-sheet plan drafted in every order, every flag of the open project is inside the frame, and the check lists no tag outside and finds the same connections as the source.
/// 3枚の計画に完全な planned_sheets を付けてどの順に下書きしても、開いたプロジェクトの旗は全て図枠の中にあり、検算は「はみ出す札なし」で、元の図と同じつながりを返す。
#[test]
fn three_planned_sheets_drafted_in_every_order_keep_every_flag_inside() {
    let planned = |names: &[(&str, &[&str])]| -> Vec<PlannedSheet> {
        names.iter().map(|(n, u)| PlannedSheet::from_value(serde_json::json!({"name": n, "units": u})).expect("an entry")).collect()
    };
    let sheets: [(&str, Vec<&[&str]>); 3] = [("One", vec![&["SA", "SB"][..]]), ("Two", vec![&["SC", "SD"][..]]), ("Three", vec![&["SX"][..], &["SY"][..]])];
    let plan_entries = planned(&[("One", &["SA", "SB"]), ("Two", &["SC", "SD"]), ("Three", &["SX", "SY"])]);
    let src = stacked_source();
    let mut sheets_with_moved_tags = 0;
    for order in [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
        let mut project = open();
        for &i in &order {
            let (name, cols) = &sheets[i];
            let mut req = stacked_request(cols, 5.0);
            req.name = (*name).into();
            req.planned_sheets = plan_entries.clone();
            let p = plan_function_sheet(&src, &project, &req).expect("the sheet fits");
            let pin_sides: Vec<Point> = labels_of(&p.sheet).iter().map(|l| l.at).collect();
            let columns_right = p.sheet.entities.values().filter_map(|e| if let Entity::Symbol(s) = e { Some(s.at.x) } else { None }).fold(0.0, f64::max);
            if pin_sides.iter().any(|a| a.x > columns_right + 1e-6 && false) {
                sheets_with_moved_tags += 1;
            }
            project.sheets.push(p.sheet);
        }
        let check = crate::draft_check::check_function_sheets(&src, &project, &[]);
        assert!(check.tags_outside.is_empty() && check.layout_ok, "{order:?}: {:?}", check.tags_outside);
        assert!(check.ok, "{order:?}: {check:?}");
    }
    let _ = sheets_with_moved_tags;
}

