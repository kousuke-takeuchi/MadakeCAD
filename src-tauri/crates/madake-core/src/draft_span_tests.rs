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
    let mut default_room = tagged_request();
    default_room.planned_sheets = Vec::new();
    write("tagged-default", &tagged_source(), &open(), &default_room);
    write("busy-default", &busy_source(), &open(), &stacked_request(&[&["A"], &["B"]], 5.0));
    let spaced: DraftRequest = serde_json::from_value(serde_json::json!({
        "name": "Look", "columns": [["C1"], [{"gap_mm": 30}, "S1", "S2"], ["L1", "L2"]],
        "power_nets": ["P24X"], "ground_nets": [], "pitch_mm": 5.0
    }))
    .expect("a request with a spacer");
    write("bus-spacer", &bus_fixture(true), &open(), &spaced);
    write("bus-blocked", &bus_fixture(true), &open(), &bus_request());
}


// ---- helpers

use super::channel::{flag_block, FlagCheck, Obstacles, Scene};
use super::channel_tests::{drafted, settled, trunks};
use crate::model::{Orientation, PaperSize};
use crate::route::RouteGrid;
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

/// Which tag position (0 to 31: 4 per extension length, in the fixed order, lengths 5, 10, ... 40 mm) the tag of T took, or `None` if it hangs on its branch.
fn position_of(tag: &NetLabel, x: f64, lo: f64, hi: f64) -> Option<usize> {
    if (tag.at.x - x).abs() > 1e-6 {
        return None;
    }
    (0..32).find(|&i| {
        let (ext, top, right) = (5.0 * (i / 4 + 1) as f64, (i % 4) < 2, i % 2 == 0);
        let at = if top { lo - ext } else { hi + ext };
        (tag.at.y - at).abs() < 1e-6 && tag.attrs.get(crate::net_tag::TAG_ATTR).map(String::as_str) == Some(if right { "left" } else { "right" })
    })
}

/// Plans T's sheet again and again, each time blocking the free half of the flag (away from the trunk) at the position the tag just took, until it hangs on its branch.
/// Returns the positions taken, in order.
fn taken_positions(src: &Project, req: &DraftRequest) -> Vec<usize> {
    let m = build_model(src, &open(), req).expect("valid");
    let t_key = net_key(&m, "T");
    let base = settled(&m, req);
    let rows: Vec<f64> = base.terminals.iter().filter(|t| t.net == t_key && !t.is_tag).map(|t| t.at.y).collect();
    let (lo, hi) = (rows.iter().cloned().fold(f64::MAX, f64::min), rows.iter().cloned().fold(f64::MIN, f64::max));
    let first = plan_function_sheet(src, &open(), req).expect("plan");
    let x = labels_of(&first.sheet).iter().find(|l| l.name == "T").map_or(0.0, |l| l.at.x);
    let mut blocked: Vec<Rect> = Vec::new();
    let mut taken = Vec::new();
    for _ in 0..34 {
        let mut l = settled(&m, req);
        l.keep_out.extend(blocked.iter().cloned());
        let (sheet, wired, labelled, ..) = route_wires_try(&m, &l, &RouteOptions::default());
        assert!(wired.contains(&"T".to_string()) && labelled.is_empty(), "{wired:?} {labelled:?}");
        assert_eq!(device_groups(&sheet, &["A", "B"]), device_groups(&src.sheets[0], &["A", "B"]));
        assert!(wires_touching_flags(&with_sheet(&open(), &sheet), &sheet).is_empty());
        let tag = tag_named(&sheet, "T");
        let Some(i) = position_of(&tag, x, lo, hi) else {
            let pin = stub_end(&sheet, "A", "1");
            let pin = if lo > pin.y + 1.0 { stub_end(&sheet, "A", "3") } else { pin };
            assert!(tag.at.distance_to(&Point::new(pin.x + 5.0, pin.y)) < 1e-6, "no position left: the tag hangs on the branch: {:?}", tag.at);
            return taken;
        };
        taken.push(i);
        let right = i % 2 == 0;
        let f = flag_rect(&trunk_tag("T", tag.at, right, None), l.label_font, l.sheet.xref_style, l.reserve.text("T")).expect("a flag");
        // the half of the flag away from the trunk, so that the trunk end itself stays free
        blocked.push(if right { Rect::from_corners(Point::new(tag.at.x + 2.0, f.min.y), Point::new(f.max.x, f.max.y)) } else { Rect::from_corners(Point::new(f.min.x, f.min.y), Point::new(tag.at.x - 2.0, f.max.y)) });
    }
    panic!("the tag never went back to its branch: {taken:?}");
}

/// The tag positions are tried in a fixed order: lengths 5, 10, 15 ... 40 mm, and at each length top right, top left, bottom right, bottom left. Blocking the position the tag took moves it to a later one, only ever forward, lengths beyond 10 mm are used, and with every position blocked it stays on its branch.
/// 札の位置は決まった順に試す。長さは5・10・15…40mm、各長さで上の右・上の左・下の右・下の左。札が取った位置をふさぐと、より後ろの位置へだけ動き、10mmより長い位置も使われ、全部ふさぐと札は枝に残る。
#[test]
fn the_tag_positions_are_tried_in_a_fixed_order_over_lengths_up_to_forty_millimetres() {
    let taken = taken_positions(&tagged_source(), &tagged_request());
    assert!(taken.windows(2).all(|w| w[0] < w[1]), "only forward: {taken:?}");
    assert!(taken.iter().any(|&i| i >= 8), "lengths beyond 10 mm are used when 5 and 10 are blocked: {taken:?}");
    assert_eq!(taken.first().copied(), Some(0), "the first position is top 5 mm right when it is free: {taken:?}");
    assert!(taken.len() >= 4, "{taken:?}");
}

/// For the lowest net of the pair the free positions are below it, so the first one taken is a bottom position, and the order is again only forward.
/// 組のいちばん下のネットでは、空いているのは下の位置なので、最初に取る位置は下の位置で、順は前へだけ進む。
#[test]
fn the_tag_of_a_bottom_net_takes_a_bottom_position_first() {
    let mut req = tagged_request();
    req.planned_sheets = vec!["Look".into()];
    let taken = taken_positions(&tagged_low_source(), &req);
    assert!(taken.windows(2).all(|w| w[0] < w[1]), "{taken:?}");
    assert!(matches!(taken.first(), Some(i) if i % 4 >= 2), "a bottom position first: {taken:?}");
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


// ---- the routing log

/// The routing log names what blocked a net: for the bus whose drop runs into a box of column 2, the candidate channel is rejected with the row and the box, and the net is drawn by the grid router.
/// 配線の記録は、ネットを邪魔したものを名前で挙げる。2列目の箱に枝が当たるバスは、候補の通り道が「どの高さで、どの箱に」と理由つきで外され、格子の経路探索が引いたと記録される。
#[test]
fn the_routing_log_names_the_box_that_blocked_a_drop() {
    let (_, _, _, _, p) = bus_layout(true);
    let bus = p.route_log.iter().find(|e| e.net == "BUS").expect("the bus is logged");
    assert_eq!(bus.router, "grid");
    assert_eq!(bus.channel, None);
    assert_eq!(bus.candidates.len(), 1, "{bus:?}");
    assert_eq!(bus.candidates[0].channel, 0);
    assert!(bus.candidates[0].drop_length_mm > 0.0);
    let why = bus.candidates[0].rejected.as_deref().expect("the channel was rejected");
    assert!(why.starts_with("drop at y=") && why.contains("box S1"), "{why}");
    let (_, _, _, _, ok) = bus_layout(false);
    let bus = ok.route_log.iter().find(|e| e.net == "BUS").expect("the bus is logged");
    assert_eq!((bus.router.as_str(), bus.channel), ("channel", Some(0)));
    assert_eq!(bus.candidates[0].rejected, None);
    assert!(bus.tag.is_none());
    assert!(ok.route_log.iter().all(|e| e.net != "P24X"), "a power net has no wire to log");
}

/// The routing log lists the tag positions that were tried: with the room kept exactly, the first position is taken and the log says so.
/// 配線の記録は、試した札の位置を並べる。幅を正確に確保したときは、最初の位置が使われ、記録にそう残る。
#[test]
fn the_routing_log_lists_the_tag_positions_that_were_tried() {
    let p = plan_function_sheet(&tagged_source(), &open(), &tagged_request()).expect("plan");
    let t = p.route_log.iter().find(|e| e.net == "T").expect("T is logged");
    let tag = t.tag.as_ref().expect("T has a tag");
    assert_eq!(tag.placed, "top+5 right");
    assert_eq!(tag.tried.len(), 1);
    assert_eq!(tag.tried[0].rejected, None);
    assert_eq!((t.router.as_str(), t.channel), ("channel", Some(0)));
    let json = serde_json::to_value(&p.route_log).expect("the log is JSON");
    assert_eq!(json[0]["router"], serde_json::json!(p.route_log[0].router));
    assert!(serde_json::to_value(&p).expect("the plan is JSON").get("route_log").is_none(), "the public reply does not carry the log");
}

// ---- change A: flags are checked against real obstacles

/// A tagged net in a normal 20 mm channel with the default room for other sheets' names gets its tag at a trunk end where the next column is empty: the flag lies over the band of the next column, clear of every box, text, wire and mark, and the connections stay the same.
/// 通常の20mmの通り道で、他のシートの名前の既定の余白があっても、札付きのネットの札は、隣の列が空いている幹の端に付く。旗は隣の列の帯の上にかかっても、箱・文字・配線・印のどれにも触れず、つながりは変わらない。
#[test]
fn a_tag_in_a_narrow_channel_goes_where_the_next_column_is_empty() {
    let mut req = tagged_request();
    req.planned_sheets = Vec::new();
    req.destinations.insert("T".into(), "cabinet B terminal side".into());
    let src = tagged_source();
    let m = build_model(&src, &open(), &req).expect("valid");
    let l = settled(&m, &req);
    let p = plan_function_sheet(&src, &open(), &req).expect("plan");
    let tag = tag_named(&p.sheet, "T");
    let pin = stub_end(&p.sheet, "A", "1");
    assert!(tag.at.distance_to(&Point::new(pin.x + 5.0, pin.y)) > 1.0, "the tag left its branch: {:?}", tag.at);
    assert!(tag.at.x > l.columns[0].right && tag.at.x < l.columns[1].left, "the tag is on the trunk in the channel: {:?}", tag.at);
    let t = p.route_log.iter().find(|e| e.net == "T").expect("T is logged");
    assert_eq!(t.router, "channel");
    assert_ne!(t.tag.as_ref().expect("a tag").placed, "branch");
    let f = flag_of(&p.sheet, &tag, None).expect("a flag");
    assert!(f.min.x < l.columns[0].right || f.max.x > l.columns[1].left, "the flag reaches over the band of a column: {f:?} {:?}", l.columns);
    for r in ["A", "B"] {
        let b = bounds_of(&p.sheet, r);
        assert!(!f.overlaps(&b), "the flag is on the box {r}: {f:?} {b:?}");
    }
    let defs = sheet_symbol_defs(&p.sheet);
    for tb in crate::tidy::text_boxes(&p.sheet, &defs).iter().filter(|t| t.kind != crate::tidy::TextKind::WireNumber && t.text != "T" && t.text != "cabinet B terminal side") {
        assert!(!f.overlaps(&tb.rect), "the flag is on the text {}", tb.text);
    }
    assert!(wires_touching_flags(&with_sheet(&open(), &p.sheet), &p.sheet).is_empty());
    assert_eq!(device_groups(&p.sheet, &["A", "B"]), device_groups(&src.sheets[0], &["A", "B"]));
}

/// A position the frame's top edge blocks is skipped and the log names the frame; the tag goes to a bottom position.
/// 図枠の上の縁がふさぐ位置は飛ばされ、記録は「図枠」と名前を挙げる。札は下の位置へ行く。
#[test]
fn a_position_cut_by_the_frame_is_skipped_and_named() {
    let src = tagged_source();
    let req = tagged_request();
    let m = build_model(&src, &open(), &req).expect("valid");
    let mut l = settled(&m, &req);
    l.frame.1 = 25.5;
    let (sheet, _, _, _, partial, tries) = route_wires_try(&m, &l, &RouteOptions::default());
    let t_key = net_key(&m, "T");
    let tried = &tries[&t_key];
    assert!(tried.iter().any(|x| x.rejected.as_deref() == Some("frame")), "{tried:?}");
    let tag = partial.iter().find(|x| x.net == t_key).and_then(|x| x.tag.as_ref()).expect("a tag record");
    assert!(tag.placed.starts_with("bottom"), "{}", tag.placed);
    assert_eq!(device_groups(&sheet, &["A", "B"]), device_groups(&src.sheets[0], &["A", "B"]));
}

/// A position the title block blocks is skipped and the log names the title block.
/// 表題欄がふさぐ位置は飛ばされ、記録は「表題欄」と名前を挙げる。
#[test]
fn a_position_in_the_title_block_is_skipped_and_named() {
    let src = tagged_source();
    let req = tagged_request();
    let m = build_model(&src, &open(), &req).expect("valid");
    let mut l = settled(&m, &req);
    l.title = (66.0, 0.0, 120.0, 28.0);
    let (_, _, _, _, _, tries) = route_wires_try(&m, &l, &RouteOptions::default());
    let tried = &tries[&net_key(&m, "T")];
    assert_eq!(tried[0].position, "top+5 right");
    assert_eq!(tried[0].rejected.as_deref(), Some("title block"), "{tried:?}");
}

/// When the flag would lie on a flag that was already moved, the position is skipped and the log names that net.
/// すでに動かした札の旗の上に重なる位置は飛ばされ、記録はその札のネットの名前を挙げる。
#[test]
fn a_position_on_another_moved_flag_is_named_by_its_net() {
    use std::cell::RefCell;
    let sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
    let opts = RouteOptions::default();
    let grid = RouteGrid::build(&sheet, &[], &opts);
    let flag = Rect::from_corners(Point::new(100.0, 100.0), Point::new(112.0, 104.0));
    let others = RefCell::new(vec![(flag, "T1".to_string())]);
    let check = FlagCheck { grid: &grid, gap: 0.6, others: &others };
    let owner = |_: &[Point]| String::new();
    let scene = Scene { named: &[], line_owner: &owner };
    let free = |_: Point, _: Point| true;
    let obs = Obstacles { fixed_lines: &[], fixed_spots: &[], branches: &[], free: &free };
    let route = route_geometry_ext(&[ChanTermAt { at: Point::new(100.0, 110.0), left: true }, ChanTermAt { at: Point::new(120.0, 110.0), left: false }], Some(110.0), Some((true, 5.0)));
    let overlapping = Rect::from_corners(Point::new(104.0, 101.0), Point::new(116.0, 105.0));
    assert_eq!(flag_block(&overlapping, &route, &obs, &[], &check, &scene).as_deref(), Some("flag of net T1"));
    let far = Rect::from_corners(Point::new(140.0, 101.0), Point::new(152.0, 105.0));
    assert_eq!(flag_block(&far, &route, &obs, &[], &check, &scene), None);
}

/// With every position blocked by a keep-out area, all 32 positions are tried, each is rejected with the name of what is in the way (never a bare "column extent"), and the tag stays on its branch as before.
/// 32の位置が全て立入禁止の領域でふさがれると、全て試し、どれも「何が邪魔か」の名前つきで外れ(「列の帯」という理由はもう無い)、札は従来どおり枝に残る。
#[test]
fn a_tag_with_no_free_position_lists_all_32_with_their_obstacles_and_stays_on_the_branch() {
    let src = tagged_source();
    let req = tagged_request();
    let m = build_model(&src, &open(), &req).expect("valid");
    let mut l = settled(&m, &req);
    let x = labels_of(&plan_function_sheet(&src, &open(), &req).expect("plan").sheet).iter().find(|t| t.name == "T").expect("T").at.x;
    // a band over and under the trunk, wide enough for every flag, that leaves the drops at the terminal rows free
    l.keep_out.push(Rect::from_corners(Point::new(x - 60.0, 10.0), Point::new(x + 60.0, 30.0)));
    l.keep_out.push(Rect::from_corners(Point::new(x - 60.0, 34.0), Point::new(x + 60.0, 80.0)));
    let (sheet, wired, labelled, _, partial, tries) = route_wires_try(&m, &l, &RouteOptions::default());
    let t = net_key(&m, "T");
    let tag = partial.iter().find(|p| p.net == t).and_then(|p| p.tag.as_ref()).expect("a tag record");
    assert_eq!(tag.placed, "branch");
    assert_eq!(tries[&t].len(), 32, "{:?}", tries[&t]);
    assert!(tries[&t].iter().all(|x| x.rejected.as_deref().is_some_and(|r| !r.is_empty() && r != "column extent")), "{:?}", tries[&t]);
    assert!(tries[&t].iter().any(|x| x.rejected.as_deref().is_some_and(|r| r.contains("keep-out"))), "{:?}", tries[&t]);
    // the keep-out bands also stop the grid router, so the net is joined by tags; the point here is the log
    assert!(wired.contains(&"T".to_string()) || labelled.contains(&"T".to_string()), "{wired:?} {labelled:?}");
    let _ = sheet;
}

/// With the default room, later nets (those the grid router draws too) keep 0.5 mm clear of a flag that moved to a trunk, on the busy fixture.
/// 既定の余白でも、幹へ動かした旗から、あとのネット(格子の経路探索が引くものも)は0.5mm以上離れる(込み入った見本で)。
#[test]
fn with_the_default_room_later_nets_keep_clear_of_a_moved_flag() {
    let src = busy_source();
    let mut req = stacked_request(&[&["A"], &["B"]], 5.0);
    req.planned_sheets = Vec::new();
    let p = plan_function_sheet(&src, &open(), &req).expect("plan");
    assert_eq!(device_groups(&p.sheet, &["A", "B"]), device_groups(&src.sheets[0], &["A", "B"]));
    assert!(wires_touching_flags(&with_sheet(&open(), &p.sheet), &p.sheet).is_empty());
    assert!(p.route_log.iter().any(|e| e.tag.as_ref().is_some_and(|t| t.placed != "branch")), "{:?}", p.route_log);
    assert_flags_clear(&p, 0.5);
}

/// With complete planned_sheets on a three-sheet plan drafted in every order, every flag is inside the frame and inside its reserve, the check lists no tag outside, and the netlist equals the source.
/// 完全な planned_sheets のある3枚の計画をどの順に下書きしても、旗は全て図枠の中で確保した幅に収まり、検算は「はみ出す札なし」で、つながりは元の図と同じ。
#[test]
fn planned_three_sheets_keep_every_flag_inside_the_frame_and_its_reserve() {
    let planned: Vec<PlannedSheet> = [("One", vec!["SA", "SB"]), ("Two", vec!["SC", "SD"]), ("Three", vec!["SX", "SY"])]
        .iter()
        .map(|(n, u)| PlannedSheet::from_value(serde_json::json!({"name": n, "units": u})).expect("an entry"))
        .collect();
    let sheets: [(&str, Vec<&[&str]>); 3] = [("One", vec![&["SA", "SB"][..]]), ("Two", vec![&["SC", "SD"][..]]), ("Three", vec![&["SX"][..], &["SY"][..]])];
    let src = stacked_source();
    for order in [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
        let mut project = open();
        for &i in &order {
            let (name, cols) = &sheets[i];
            let mut req = stacked_request(cols, 5.0);
            req.name = (*name).into();
            req.planned_sheets = planned.clone();
            let p = plan_function_sheet(&src, &project, &req).expect("the sheet fits");
            // inside the reserve: the flag drawn with the names that exist now is inside the flag drawn with the full reserve
            let m = build_model(&src, &project, &req).expect("valid");
            let l = lay_out(&m, &project, &req).expect("layout");
            let with = with_sheet(&project, &p.sheet);
            let xrefs = crate::xref::sheet_xrefs(&with, p.sheet.id);
            for label in labels_of(&p.sheet) {
                let actual = flag_of(&p.sheet, &label, xrefs.get(&label.id).map(String::as_str)).expect("a flag");
                let reserved = flag_rect(&label, l.label_font, p.sheet.xref_style, l.reserve.text(&label.name)).expect("a flag");
                assert!(actual.min.x >= reserved.min.x - 1e-6 && actual.max.x <= reserved.max.x + 1e-6, "{order:?} {name} {}: {actual:?} not inside {reserved:?}", label.name);
                assert!(actual.min.x >= l.frame.0 - 1e-6 && actual.max.x <= l.frame.2 + 1e-6 && actual.min.y >= l.frame.1 - 1e-6 && actual.max.y <= l.frame.3 + 1e-6, "{order:?} {name} {}: {actual:?} outside the frame {:?}", label.name, l.frame);
            }
            project.sheets.push(p.sheet);
        }
        let check = crate::draft_check::check_function_sheets(&src, &project, &[]);
        assert!(check.tags_outside.is_empty() && check.layout_ok && check.ok, "{order:?}: {check:?}");
    }
}

fn busy_source() -> Project {
    source_of(
        "draft-busy",
        &[("A", 6), ("B", 6), ("X", 3)],
        &[
            ("T1", &[("A", "1"), ("B", "2"), ("X", "1")]),
            ("T2", &[("A", "2"), ("B", "1"), ("X", "2")]),
            ("T3", &[("A", "3"), ("B", "3"), ("X", "3")]),
            ("P1", &[("A", "4"), ("B", "5")]),
            ("P2", &[("A", "5"), ("B", "4")]),
        ],
    )
}

/// Every flag of the plan keeps `clearance` mm from every wire that is not its own net's (a wire that has an end at the tag point is its own).
fn assert_flags_clear(p: &DraftPlan, clearance: f64) {
    let project = with_sheet(&open(), &p.sheet);
    let xrefs = crate::xref::sheet_xrefs(&project, p.sheet.id);
    let font = crate::svg::LABEL_FONT * p.sheet.annotation_scale();
    for l in labels_of(&p.sheet) {
        let Some(tag) = crate::net_tag::net_tag_with(&l, font, p.sheet.xref_style, xrefs.get(&l.id).map(String::as_str)) else { continue };
        let r = rect_of(&tag.points);
        let inflated = Rect::from_corners(Point::new(r.min.x - clearance, r.min.y - clearance), Point::new(r.max.x + clearance, r.max.y + clearance));
        for w in wires_of(&p.sheet) {
            if w.points.iter().any(|q| q.distance_to(&l.at) < 1e-6) {
                continue;
            }
            for s in w.points.windows(2) {
                assert!(!seg_rect(s[0], s[1]).overlaps(&inflated), "{} flag {r:?}: wire {:?}-{:?}", l.name, s[0], s[1]);
            }
        }
    }
}

// ---- change B: spacers in `columns`

/// A request read from JSON, as a caller sends it: the columns may hold spacer objects.
fn json_request(columns: serde_json::Value) -> Result<DraftRequest, String> {
    serde_json::from_value(serde_json::json!({"name": "Look", "columns": columns, "power_nets": [], "ground_nets": [], "pitch_mm": 5.0})).map_err(|e| e.to_string())
}

fn y_of(p: &DraftPlan, reference: &str) -> f64 {
    unit_of(&p.sheet, reference).at.y
}

fn stacked_plan(columns: serde_json::Value) -> Result<DraftPlan, String> {
    plan_function_sheet(&stacked_source(), &stacked_open(), &json_request(columns)?)
}

/// A spacer at the start of a column moves its first unit down by exactly that many millimetres, the other columns stay where they are, and everything stays on the 2.5 mm grid.
/// 列の先頭の隙間は、その列の最初の機器をちょうどその mm だけ下げる。他の列は動かず、全てが2.5mmの格子の上に残る。
#[test]
fn a_spacer_at_the_start_of_a_column_moves_the_first_unit_down_by_that_many_millimetres() {
    let plain = stacked_plan(serde_json::json!([["SY"], ["SA", "SB"]])).expect("plan");
    let spaced = stacked_plan(serde_json::json!([["SY"], [{"gap_mm": 25}, "SA", "SB"]])).expect("plan");
    assert!((y_of(&spaced, "SA") - y_of(&plain, "SA") - 25.0).abs() < 1e-9);
    assert!((y_of(&spaced, "SB") - y_of(&plain, "SB") - 25.0).abs() < 1e-9, "the next unit follows");
    assert!((y_of(&spaced, "SY") - y_of(&plain, "SY")).abs() < 1e-9, "the other column stays");
    assert!((unit_of(&spaced.sheet, "SA").at.x - unit_of(&plain.sheet, "SA").at.x).abs() < 1e-9);
    for r in ["SA", "SB", "SY"] {
        let at = unit_of(&spaced.sheet, r).at;
        assert!(((at.y / 2.5) - (at.y / 2.5).round()).abs() < 1e-9 && ((at.x / 2.5) - (at.x / 2.5).round()).abs() < 1e-9, "{r} {at:?}");
    }
}

/// A spacer between two units adds to the normal gap below the first one only: the second unit moves down, the first stays.
/// 2つの機器のあいだの隙間は、普通のすき間に足される。後ろの機器だけが下がり、前の機器は動かない。
#[test]
fn a_spacer_between_two_units_moves_only_the_second_down() {
    let plain = stacked_plan(serde_json::json!([["SA", "SB", "SC"]])).expect("plan");
    let spaced = stacked_plan(serde_json::json!([["SA", {"gap_mm": 15}, "SB", "SC"]])).expect("plan");
    assert!((y_of(&spaced, "SA") - y_of(&plain, "SA")).abs() < 1e-9);
    assert!((y_of(&spaced, "SB") - y_of(&plain, "SB") - 15.0).abs() < 1e-9);
    assert!((y_of(&spaced, "SC") - y_of(&plain, "SC") - 15.0).abs() < 1e-9);
}

/// Spacers in a row add up: 10 and 15 move a unit as far as 25 does.
/// 続く隙間は足し合わされる。10と15は、25と同じだけ機器を下げる。
#[test]
fn consecutive_spacers_add_up() {
    let two = stacked_plan(serde_json::json!([[{"gap_mm": 10}, {"gap_mm": 15}, "SA", "SB"]])).expect("plan");
    let one = stacked_plan(serde_json::json!([[{"gap_mm": 25}, "SA", "SB"]])).expect("plan");
    assert!((y_of(&two, "SA") - y_of(&one, "SA")).abs() < 1e-9 && (y_of(&two, "SB") - y_of(&one, "SB")).abs() < 1e-9);
    let plain = stacked_plan(serde_json::json!([["SA", "SB"]])).expect("plan");
    assert!((y_of(&one, "SA") - y_of(&plain, "SA") - 25.0).abs() < 1e-9);
}

/// A spacer at the end of a column, a value off the 2.5 mm grid or outside 2.5 to 200, are refused with the column number and the entry number, and nothing is drafted.
/// 列の末尾の隙間、2.5mmの格子に乗らない値、2.5〜200の外の値は、列の番号と項目の番号を挙げて断る。何も下書きされない。
#[test]
fn bad_spacers_are_refused_with_the_column_and_the_entry() {
    let cases = [
        (serde_json::json!([["SA", {"gap_mm": 10}]]), "column 1, entry 2", "end of the column"),
        (serde_json::json!([["SY"], ["SA", {"gap_mm": 10}, "SB", {"gap_mm": 5}]]), "column 2, entry 4", "end of the column"),
        (serde_json::json!([["SA", {"gap_mm": 3.0}, "SB"]]), "column 1, entry 2", "multiple of 2.5"),
        (serde_json::json!([["SA", {"gap_mm": 0}, "SB"]]), "column 1, entry 2", "2.5 to 200"),
        (serde_json::json!([["SA", {"gap_mm": 202.5}, "SB"]]), "column 1, entry 2", "2.5 to 200"),
        (serde_json::json!([["SA", {"gap_mm": -5}, "SB"]]), "column 1, entry 2", "2.5 to 200"),
        (serde_json::json!([[{"gap_mm": 10}]]), "column 1, entry 1", "end of the column"),
    ];
    for (columns, place, what) in cases {
        let e = stacked_plan(columns.clone()).expect_err(&columns.to_string());
        assert!(e.contains(place) && e.contains(what), "{columns}: {e}");
    }
}

/// An entry that is neither a reference nor a {"gap_mm": number} is refused when the request is read, naming columns[c][i] and the column and entry numbers.
/// 参照記号でも {"gap_mm": 数} でもない項目は、依頼を読むときに、columns[c][i] と列・項目の番号を挙げて断る。
#[test]
fn a_malformed_column_entry_is_refused_naming_its_place() {
    for (columns, place) in [
        (serde_json::json!([["SA", 5]]), "columns[0][1] (column 1, entry 2)"),
        (serde_json::json!([["SA"], ["SB", {"gap": 5}, "SC"]]), "columns[1][1] (column 2, entry 2)"),
        (serde_json::json!([["SA", {"gap_mm": "x"}]]), "columns[0][1] (column 1, entry 2)"),
        (serde_json::json!([[{"gap_mm": 5, "extra": 1}, "SA"]]), "columns[0][0] (column 1, entry 1)"),
        (serde_json::json!([["SA", null]]), "columns[0][1] (column 1, entry 2)"),
        (serde_json::json!(["SA"]), "columns[0]"),
    ] {
        let e = json_request(columns.clone()).expect_err(&columns.to_string());
        assert!(e.contains(place), "{columns}: {e}");
    }
    assert!(json_request(serde_json::json!([["SA", {"gap_mm": 5}, "SB"]])).is_ok());
}

/// The height refusal counts the spacers: a column that fits is refused once spacers push it past the drawing area.
/// 高さの断りは隙間も数える。収まる列も、隙間で描画範囲を越えれば断られる。
#[test]
fn the_height_refusal_counts_the_spacers() {
    assert!(stacked_plan(serde_json::json!([["SA", "SB"]])).is_ok());
    let e = stacked_plan(serde_json::json!([["SA", {"gap_mm": 200}, "SB", {"gap_mm": 200}, "SC"]])).expect_err("too tall");
    assert!(e.contains("taller than the drawing area") && e.contains("Column 1"), "{e}");
}

/// Requests without spacers read as before: the columns hold plain references and the plan is the one the plain request gives.
/// 隙間の無い依頼は従来どおりに読まれ、列は参照記号だけで、計画も従来の依頼と同じになる。
#[test]
fn a_request_without_spacers_plans_as_before() {
    let json = stacked_plan(serde_json::json!([["SY"], ["SA", "SB"]])).expect("plan");
    let plain = plan_function_sheet(&stacked_source(), &stacked_open(), &stacked_request(&[&["SY"], &["SA", "SB"]], 5.0)).expect("plan");
    assert_eq!(wire_key(&json), wire_key(&plain));
    for r in ["SY", "SA", "SB"] {
        assert_eq!(unit_of(&json.sheet, r).at, unit_of(&plain.sheet, r).at);
    }
}

/// The bus whose drop a box of column 2 blocked becomes one trunk with straight drops once a spacer moves that box out of the row, and the routing log shows the change.
/// 2列目の箱が枝をふさいでいたバスは、隙間でその箱を高さからはずすと、幹1本とまっすぐな枝で引かれ、配線の記録にもそう出る。
#[test]
fn a_spacer_moves_the_blocking_box_out_of_the_row_and_the_bus_gets_its_trunk() {
    let src = bus_fixture(true);
    let mut blocked = bus_request();
    let before = plan_function_sheet(&src, &open(), &blocked).expect("plan");
    let b = before.route_log.iter().find(|e| e.net == "BUS").expect("BUS");
    assert_eq!(b.router, "grid");
    assert!(b.candidates[0].rejected.as_deref().is_some_and(|r| r.contains("box S1")), "{b:?}");
    let mut req: DraftRequest = serde_json::from_value(serde_json::json!({
        "name": "Look", "columns": [["C1"], [{"gap_mm": 30}, "S1", "S2"], ["L1", "L2"]],
        "power_nets": ["P24X"], "ground_nets": [], "pitch_mm": 5.0
    }))
    .expect("a request with a spacer");
    req.planned_sheets = Vec::new();
    let after = plan_function_sheet(&src, &open(), &req).expect("plan");
    let a = after.route_log.iter().find(|e| e.net == "BUS").expect("BUS");
    assert_eq!((a.router.as_str(), a.channel), ("channel", Some(0)), "{a:?}");
    assert_eq!(a.candidates[0].rejected, None);
    assert_eq!(wires_through_boxes(&after.sheet, &BUS_REFS), 0);
    assert_eq!(device_groups(&after.sheet, &BUS_REFS), device_groups(&src.sheets[0], &BUS_REFS));
    blocked.planned_sheets = Vec::new();
}
