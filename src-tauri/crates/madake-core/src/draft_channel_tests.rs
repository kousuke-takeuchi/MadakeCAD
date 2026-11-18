//! 機能シートの下書きの通り道のトラック (`draft_channel.rs`) のテスト。元の図は `source_of` で組み立てる作り物の機器 A B C D E。
use super::channel::*;
use super::look_tests::*;
use super::*;
use crate::route::RouteOptions;
use std::collections::BTreeSet;

// ---- helpers

type Pairs<'a> = &'a [(&'a str, &'a [(&'a str, &'a str)])];

/// The model, the layout and the plan of units (reference, number of pins) with the given nets, drafted in the given columns (no power or ground).
fn drafted(units: &[(&str, usize)], nets: Pairs, columns: &[&[&str]], pitch: f64) -> (Project, DraftModel, DraftLayout, DraftPlan) {
    let src = source_of("draft-channel", units, nets);
    let req = stacked_request(columns, pitch);
    let m = build_model(&src, &open(), &req).expect("the request is valid");
    let l = lay_out(&m, &open(), &req).expect("the layout fits");
    let p = plan_function_sheet(&src, &open(), &req).expect("the plan is made");
    (src, m, l, p)
}

/// The vertical wire segments that lie strictly inside channel k (between column k and column k+1): (x, top y, bottom y).
fn trunks(sheet: &Sheet, l: &DraftLayout, k: usize) -> Vec<(f64, f64, f64)> {
    let (r, left) = (l.columns[k].right, l.columns[k + 1].left);
    let mut out = Vec::new();
    for w in wires_of(sheet) {
        for s in w.points.windows(2) {
            if (s[0].x - s[1].x).abs() < 1e-9 && s[0].x > r + 1e-6 && s[0].x < left - 1e-6 {
                out.push((s[0].x, s[0].y.min(s[1].y), s[0].y.max(s[1].y)));
            }
        }
    }
    out.sort_by(|a, b| a.partial_cmp(b).unwrap());
    out
}

fn on_grid(v: f64) -> bool {
    ((v / 2.5) - (v / 2.5).round()).abs() < 1e-6
}

fn names(m: &DraftModel, keys: impl Iterator<Item = usize>) -> BTreeSet<String> {
    keys.map(|k| m.nets[k].name.clone()).collect()
}

fn chan_net(key: usize, name: &str, terms: &[(f64, bool)]) -> ChanNet {
    ChanNet { key, name: name.into(), terms: terms.iter().map(|&(y, left)| ChanTerm { y, left }).collect() }
}

fn track_of(a: &Assignment, key: usize) -> Option<usize> {
    a.placed.iter().find(|p| p.key == key).expect("the net is placed").track
}

/// A source whose channel 0 carries three nets between two stacked units A and B (column 1 holds C with an inner jumper only).
fn stacked_three() -> (Project, DraftModel, DraftLayout, DraftPlan) {
    drafted(
        &[("A", 4), ("B", 4), ("C", 2)],
        &[("N1", &[("A", "1"), ("B", "1")]), ("N2", &[("A", "2"), ("B", "2")]), ("N3", &[("A", "3"), ("B", "3")]), ("Z", &[("C", "1"), ("C", "2")])],
        &[&["A", "B"], &["C"]],
        5.0,
    )
}

// ---- the track assignment (pure)

/// A single net takes the leftmost position, the first track.
/// 1本だけのネットは、いちばん左の位置、つまり最初のトラックに置く。
#[test]
fn a_single_net_takes_the_first_track() {
    let a = assign_tracks(&[chan_net(0, "N", &[(10.0, true), (30.0, false)])]);
    assert_eq!((a.tracks, track_of(&a, 0)), (1, Some(0)));
}

/// Two nets whose trunk spans, each widened by 5 mm, do not overlap share one track.
/// 幹の範囲を両側に5mm広げても重ならない2本のネットは、1つのトラックを分け合える。
#[test]
fn nets_with_distant_spans_share_one_track() {
    let a = assign_tracks(&[chan_net(0, "N1", &[(0.0, true), (10.0, false)]), chan_net(1, "N2", &[(30.0, true), (40.0, false)])]);
    assert_eq!(a.tracks, 1);
    assert_eq!((track_of(&a, 0), track_of(&a, 1)), (Some(0), Some(0)));
}

/// Spans that are exactly 10 mm apart (the widened spans only touch) may share a track; spans closer than that may not.
/// 範囲のあいだがちょうど10mm (広げた範囲が触れるだけ) なら相乗りでき、それより近いと相乗りできない。
#[test]
fn spans_closer_than_ten_millimetres_do_not_share_a_track() {
    let touching = assign_tracks(&[chan_net(0, "N1", &[(0.0, true), (10.0, false)]), chan_net(1, "N2", &[(20.0, true), (30.0, false)])]);
    assert_eq!(touching.tracks, 1);
    let close = assign_tracks(&[chan_net(0, "N1", &[(0.0, true), (10.0, false)]), chan_net(1, "N2", &[(19.0, true), (30.0, false)])]);
    assert_eq!(close.tracks, 2);
}

/// Of all positions the one with the fewest crossings wins: the second net goes left of the first, where nothing crosses, not right of it, where two wires would.
/// 全ての位置のうち、交差がいちばん少ない位置に置く。2本目は、2か所で交差する右ではなく、交差しない左に置く。
#[test]
fn the_position_with_the_fewest_crossings_wins() {
    let a = assign_tracks(&[chan_net(0, "N1", &[(0.0, true), (20.0, false)]), chan_net(1, "N2", &[(10.0, true), (30.0, false)])]);
    assert_eq!(a.tracks, 2);
    assert!(track_of(&a, 1) < track_of(&a, 0), "{a:?}");
}

/// Nested spans need no crossing: the net with the widest span takes the outer track, and every drop reaches its trunk without crossing another.
/// 入れ子の範囲は交差なしで引ける。範囲がいちばん広いネットは外側のトラックに行き、どの枝も他の幹を横切らない。
#[test]
fn nested_spans_are_assigned_without_a_crossing() {
    let nets = [
        chan_net(0, "N1", &[(0.0, true), (60.0, true)]),
        chan_net(1, "N2", &[(10.0, true), (50.0, true)]),
        chan_net(2, "N3", &[(20.0, true), (40.0, true)]),
    ];
    let a = assign_tracks(&nets);
    assert_eq!(a.tracks, 3);
    // left drops run from the left to the trunk: the inner nets must lie further left
    assert!(track_of(&a, 2) < track_of(&a, 1) && track_of(&a, 1) < track_of(&a, 0), "{a:?}");
}

/// Two terminals at the same height on the two sides are one straight wire and need no track.
/// 両側の同じ高さにある2つの端は1本の横線になり、トラックは要らない。
#[test]
fn two_terminals_at_one_height_need_no_track() {
    let a = assign_tracks(&[chan_net(0, "N", &[(15.0, true), (15.0, false)])]);
    assert_eq!((a.tracks, track_of(&a, 0)), (0, None));
}

/// The result does not depend on the order the nets are given in, and planning the same nets twice gives the same result.
/// ネットを渡す順に関わらず結果は同じで、同じネットを2回割り当てても同じ結果になる。
#[test]
fn the_assignment_does_not_depend_on_the_input_order() {
    let nets = vec![
        chan_net(0, "N1", &[(0.0, true), (20.0, false)]),
        chan_net(1, "N2", &[(10.0, true), (30.0, false)]),
        chan_net(2, "N10", &[(25.0, true), (45.0, false)]),
        chan_net(3, "N3", &[(50.0, true), (60.0, true)]),
    ];
    let mut reversed = nets.clone();
    reversed.reverse();
    assert_eq!(assign_tracks(&nets), assign_tracks(&reversed));
    assert_eq!(assign_tracks(&nets), assign_tracks(&nets));
    let order: Vec<usize> = assign_tracks(&nets).placed.iter().map(|p| p.key).collect();
    assert_eq!(order, vec![0, 1, 3, 2], "natural order of the names: N1, N2, N3, N10");
}

/// A net that would have to touch an earlier net whatever its position is left out, so the grid router takes it.
/// どの位置に置いても先に置いたネットに触れてしまうネットは割り当てずに残し、格子の経路探索に任せる。
#[test]
fn a_net_that_would_short_at_every_position_is_left_unplaced() {
    // N1 runs from the left at y=0 to the right at y=10; N2 from the left at y=10 to the right at y=0
    let a = assign_tracks(&[chan_net(0, "N1", &[(0.0, true), (10.0, false)]), chan_net(1, "N2", &[(10.0, true), (0.0, false)])]);
    assert_eq!(a.unplaced, vec![1], "{a:?}");
    assert_eq!(a.tracks, 1);
}

// ---- the validation of one net (pure)

fn at(x: f64, y: f64, left: bool) -> ChanTermAt {
    ChanTermAt { at: Point::new(x, y), left }
}

fn always_free(_: Point, _: Point) -> bool {
    true
}

/// A drop that would run through the end of another net's stub is a short, so the net is not valid; without that stub it is.
/// 枝が別のネットの短い線の先を通ってしまうと短絡なので、そのネットは使えない。その短い線が無ければ使える。
#[test]
fn a_drop_through_a_foreign_stub_end_is_refused() {
    let terms = [at(100.0, 10.0, true), at(150.0, 40.0, false)];
    let route = route_geometry(&terms, Some(125.0));
    let stub = vec![Point::new(110.0, 10.0), Point::new(115.0, 10.0)];
    let free = always_free;
    let none: Vec<Vec<Point>> = Vec::new();
    let with = Obstacles { fixed_lines: std::slice::from_ref(&stub), fixed_spots: &[], branches: &[], free: &free };
    let without = Obstacles { fixed_lines: &none, fixed_spots: &[], branches: &[], free: &free };
    assert!(!net_is_valid(&route, &terms, &with, &[]));
    assert!(net_is_valid(&route, &terms, &without, &[]));
}

/// A drop that runs through a fixed junction dot, or crosses a tag's branch, is refused; crossing the middle of a foreign wire at a right angle is not.
/// 枝が動かない接点の丸を通る・札の枝を横切るときは使えない。別の配線の途中を直角に横切るだけなら使える。
#[test]
fn a_junction_or_a_tag_branch_on_the_way_is_refused_but_a_plain_crossing_is_not() {
    let terms = [at(100.0, 10.0, true), at(150.0, 40.0, false)];
    let route = route_geometry(&terms, Some(125.0));
    let free = always_free;
    let crossing = vec![Point::new(110.0, 5.0), Point::new(110.0, 15.0)];
    let plain = Obstacles { fixed_lines: std::slice::from_ref(&crossing), fixed_spots: &[], branches: &[], free: &free };
    assert!(net_is_valid(&route, &terms, &plain, &[]));
    let dot = [Point::new(110.0, 10.0)];
    let spot = Obstacles { fixed_lines: &[], fixed_spots: &dot, branches: &[], free: &free };
    assert!(!net_is_valid(&route, &terms, &spot, &[]));
    let branch = [(Point::new(110.0, 5.0), Point::new(110.0, 15.0))];
    let tag = Obstacles { fixed_lines: &[], fixed_spots: &[], branches: &branch, free: &free };
    assert!(!net_is_valid(&route, &terms, &tag, &[]));
}

/// A net whose wire would touch another accepted net's wire at a vertex or along a line is refused, while a right-angle crossing is fine.
/// 別のネットの配線と頂点や線の上で触れてしまうネットは使えない。直角に交わるだけなら使える。
#[test]
fn a_wire_touching_an_accepted_net_is_refused() {
    let free = always_free;
    let obs = Obstacles { fixed_lines: &[], fixed_spots: &[], branches: &[], free: &free };
    let first_terms = [at(100.0, 10.0, true), at(150.0, 40.0, false)];
    let first = route_geometry(&first_terms, Some(125.0));
    let crossing_terms = [at(100.0, 20.0, true), at(150.0, 60.0, false)];
    let crossing = route_geometry(&crossing_terms, Some(130.0));
    assert!(net_is_valid(&crossing, &crossing_terms, &obs, &[&first]));
    let touching_terms = [at(100.0, 40.0, true), at(150.0, 60.0, false)];
    let touching = route_geometry(&touching_terms, Some(130.0));
    assert!(!net_is_valid(&touching, &touching_terms, &obs, &[&first]), "the drop at y=40 meets the end of the first trunk");
}

/// The geometry of a net with three drops is one trunk, one wire per drop, and a junction at every trunk point where three or more ends meet.
/// 端が3つあるネットの配線は、幹1本と、端ごとに1本の枝と、3本以上の端が会う幹の点ごとの接点になる。
#[test]
fn a_trunk_with_three_drops_has_a_junction_at_the_middle_drop_only() {
    let terms = [at(100.0, 10.0, true), at(100.0, 20.0, true), at(150.0, 30.0, false)];
    let r = route_geometry(&terms, Some(125.0));
    assert_eq!(r.wires.len(), 4);
    assert_eq!(r.junctions, vec![Point::new(125.0, 20.0)]);
    let both = [at(100.0, 10.0, true), at(150.0, 10.0, false), at(150.0, 30.0, false)];
    let r = route_geometry(&both, Some(125.0));
    assert_eq!(r.junctions, vec![Point::new(125.0, 10.0)], "two drops meet the trunk end");
}

// ---- the sheet

/// Two stacked units with three nets between them get three vertical trunks on three different tracks and no jog: every wire of the sheet is a straight line of two points.
/// 縦に積んだ2つの機器のあいだの3本のネットは、別々の3本のトラックに縦の幹を1本ずつ持つ。途中の折れは無く、シートの配線は全て2点の直線になる。
#[test]
fn three_nets_between_stacked_units_get_three_trunks_and_no_jog() {
    let (_, m, l, p) = stacked_three();
    let t = trunks(&p.sheet, &l, 0);
    assert_eq!(t.len(), 3, "{t:?}");
    let xs: BTreeSet<i64> = t.iter().map(|(x, _, _)| (x * 1000.0).round() as i64).collect();
    assert_eq!(xs.len(), 3, "three tracks: {t:?}");
    for w in wires_of(&p.sheet) {
        assert_eq!(w.points.len(), 2, "a jog: {:?}", w.points);
    }
    assert_eq!(names(&m, 0..m.nets.len()).len(), 4);
    for n in ["N1", "N2", "N3"] {
        assert!(p.wired.contains(&n.to_string()), "{n} is wired: {:?}", p.wired);
    }
}

/// The tracks are 2.5 mm apart, on the 2.5 mm grid, and at least 5 mm from the column extents on both sides.
/// トラックは2.5mm間隔で2.5mmの格子の上にあり、両側の列の張り出しから5mm以上離れている。
#[test]
fn tracks_are_on_the_grid_and_keep_their_distance_from_the_columns() {
    let (_, _, l, p) = stacked_three();
    let t = trunks(&p.sheet, &l, 0);
    let (r, left) = (l.columns[0].right, l.columns[1].left);
    for (x, _, _) in &t {
        assert!(on_grid(*x), "{x}");
        assert!(*x >= r + 5.0 - 1e-6 && *x <= left - 5.0 + 1e-6, "{x} in {r}..{left}");
    }
    for pair in t.windows(2) {
        assert!(pair[1].0 - pair[0].0 >= 2.5 - 1e-6, "{pair:?}");
    }
}

/// Every vertex of a channel wire is on the 2.5 mm grid.
/// 通り道の配線の頂点は全て2.5mmの格子の上にある。
#[test]
fn every_vertex_of_a_channel_wire_is_on_the_grid() {
    let (_, _, l, p) = stacked_three();
    let (r, left) = (l.columns[0].right, l.columns[1].left);
    for w in wires_of(&p.sheet) {
        for q in &w.points {
            if q.x > r - 40.0 && q.x < left + 40.0 {
                assert!(on_grid(q.x) && on_grid(q.y), "{q:?}");
            }
        }
    }
}

/// A net of five pins on five units in two adjacent columns is drawn as one trunk with five drops and a junction at each drop where three ends meet; nothing else is drawn in the channel.
/// 隣り合う2つの列の5つの機器にまたがる5ピンのネットは、幹1本と枝5本で描かれ、3本が会う枝ごとに接点の丸がある。通り道にそれ以外の線は無い。
#[test]
fn a_net_over_five_units_in_two_columns_is_one_trunk_with_five_drops() {
    let (_, _, l, p) = drafted(
        &[("A", 2), ("B", 2), ("C", 2), ("D", 2), ("E", 2)],
        &[("N", &[("A", "1"), ("B", "1"), ("C", "1"), ("D", "1"), ("E", "1")]), ("X", &[("A", "2"), ("B", "2")])],
        &[&["A", "B", "C"], &["D", "E"]],
        5.0,
    );
    let t = trunks(&p.sheet, &l, 0);
    let n_trunks: Vec<_> = t.iter().filter(|(_, y0, y1)| y1 > y0).collect();
    assert!(!n_trunks.is_empty());
    // the only vertical wires in the channel belong to N and X: N has one trunk
    let tallest = n_trunks.iter().max_by(|a, b| (a.2 - a.1).partial_cmp(&(b.2 - b.1)).unwrap()).unwrap();
    let x = tallest.0;
    let drops: Vec<Wire> = wires_of(&p.sheet).into_iter().filter(|w| w.points.len() == 2 && (w.points[0].y - w.points[1].y).abs() < 1e-9 && w.points.iter().any(|q| (q.x - x).abs() < 1e-6)).collect();
    assert_eq!(drops.len(), 5, "five drops on the trunk of N: {drops:?}");
    for d in &drops {
        let y = d.points[0].y;
        let arms = arms_at(&p.sheet, Point::new(x, y));
        let dot = junctions_of(&p.sheet).iter().any(|j| j.distance_to(&Point::new(x, y)) < 1e-6);
        assert_eq!(dot, arms >= 3, "junction at the drop y={y}: {arms} arms");
    }
    for j in junctions_of(&p.sheet) {
        if j.x > l.columns[0].right + 1e-6 && j.x < l.columns[1].left - 1e-6 {
            assert!(drops.iter().any(|d| (d.points[0].y - j.y).abs() < 1e-6 && (j.x - x).abs() < 1e-6), "a junction that is not at a drop: {j:?}");
        }
    }
}

/// Five pins of one net on three units (two units with two pins each, which form bars) are drawn with one trunk and three drops.
/// 1つのネットの5ピンが3つの機器にあるとき (2つの機器は2ピンずつで棒になる)、幹1本と枝3本で描かれる。
#[test]
fn bars_count_as_one_terminal_each() {
    let (_, _, l, p) = drafted(
        &[("A", 3), ("B", 3), ("C", 2)],
        &[("N", &[("A", "1"), ("A", "2"), ("B", "1"), ("B", "2"), ("C", "1")]), ("X", &[("A", "3"), ("B", "3")])],
        &[&["A", "B"], &["C"]],
        5.0,
    );
    let t = trunks(&p.sheet, &l, 0);
    assert!(!t.is_empty());
    let n = &p.wired;
    assert!(n.contains(&"N".to_string()), "{n:?}");
    let tallest = t.iter().max_by(|a, b| (a.2 - a.1).partial_cmp(&(b.2 - b.1)).unwrap()).unwrap();
    let drops = wires_of(&p.sheet).into_iter().filter(|w| w.points.len() == 2 && (w.points[0].y - w.points[1].y).abs() < 1e-9 && w.points.iter().any(|q| (q.x - tallest.0).abs() < 1e-6)).count();
    assert!(drops >= 3, "{drops}");
}

/// Two terminals at the same height on the two sides are one straight horizontal wire from stub end to stub end, with no trunk and no junction.
/// 両側の同じ高さにある2つの端は、短い線の先から先まで1本の横線になる。幹も接点も無い。
#[test]
fn terminals_at_one_height_are_one_straight_wire() {
    let (_, _, l, p) = drafted(&[("A", 2), ("B", 2)], &[("N", &[("A", "1"), ("B", "1")])], &[&["A"], &["B"]], 5.0);
    let (ea, eb) = (stub_end(&p.sheet, "A", "1"), stub_end(&p.sheet, "B", "1"));
    assert!((ea.y - eb.y).abs() < 1e-9, "the pins are level: {ea:?} {eb:?}");
    assert!(trunks(&p.sheet, &l, 0).is_empty());
    let joined: Vec<Wire> = wires_of(&p.sheet).into_iter().filter(|w| w.points.len() == 2 && w.points.iter().any(|q| q.distance_to(&ea) < 1e-6) && w.points.iter().any(|q| q.distance_to(&eb) < 1e-6)).collect();
    assert_eq!(joined.len(), 1, "one wire joins the stub ends");
    assert!(junctions_of(&p.sheet).is_empty());
    assert_eq!(l.channels[0].tracks, 0);
}

/// A net with a terminal outside the channel (it reaches two columns over) is not channel-routable: the grid router draws it and the connections stay right.
/// 端が通り道の外にあるネット (2つ先の列まで届く) は通り道を使えない。格子の経路探索が引き、つながりは変わらない。
#[test]
fn a_net_reaching_two_columns_over_goes_to_the_grid_router() {
    let (src, m, l, p) = drafted(&[("A", 2), ("B", 2), ("C", 2)], &[("N", &[("A", "1"), ("C", "1")]), ("M", &[("A", "2"), ("B", "1")])], &[&["A"], &["B"], &["C"]], 5.0);
    let placed = |l: &DraftLayout| names(&m, l.channels.iter().flat_map(|c| c.placed.iter().map(|p| p.key)));
    assert_eq!(placed(&l), ["M".to_string()].into_iter().collect(), "only M is channel-routable");
    assert!(p.wired.contains(&"N".to_string()), "{:?}", p.wired);
    assert!(p.labelled.is_empty());
    assert_eq!(device_groups(&p.sheet, &["A", "B", "C"]), device_groups(&src.sheets[0], &["A", "B", "C"]));
}

/// A leaving net with two groups hangs its tag on a branch, so it is not channel-routable and the grid router draws it; a leaving net with only one group on the sheet has no wire at all.
/// 札の枝を持つ出ていくネット (組が2つ) は通り道を使えず、格子の経路探索が引く。シートに組が1つだけの出ていくネットは配線そのものが無い。
#[test]
fn a_net_with_a_tag_branch_goes_to_the_grid_router() {
    let src = source_of("draft-leaving", &[("A", 2), ("B", 2), ("C", 2)], &[("N", &[("A", "1"), ("B", "1"), ("C", "1")]), ("L", &[("A", "2"), ("C", "2")])]);
    let req = stacked_request(&[&["A"], &["B"]], 5.0);
    let m = build_model(&src, &open(), &req).expect("valid");
    let l = lay_out(&m, &open(), &req).expect("fits");
    let p = plan_function_sheet(&src, &open(), &req).expect("plan");
    assert!(l.channels.iter().all(|c| c.placed.is_empty()), "N carries a tag branch: {:?}", l.channels);
    assert!(p.wired.contains(&"N".to_string()), "{:?}", p.wired);
    assert_eq!(p.tagged.iter().map(|t| t.net.as_str()).collect::<Vec<_>>(), vec!["L", "N"].into_iter().filter(|n| p.tagged.iter().any(|t| t.net == *n)).collect::<Vec<_>>());
    assert!(wires_touching_flags(&with_sheet(&open(), &p.sheet), &p.sheet).is_empty());
}

/// The channel is 10 mm plus 2.5 mm per track and at least 20 mm wide, so the tracks always fit.
/// 通り道の幅は10mm + トラック1本につき2.5mmで、20mm以上。トラックは必ず収まる。
#[test]
fn the_channel_width_follows_the_track_count() {
    for n in [1usize, 3, 6] {
        let units: Vec<(&str, usize)> = vec![("A", n + 1), ("B", n + 1), ("C", 2)];
        let names: Vec<(String, [(&str, String); 2])> = (1..=n).map(|i| (format!("N{i}"), [("A", i.to_string()), ("B", i.to_string())])).collect();
        let nets: Vec<(&str, Vec<(&str, &str)>)> = names.iter().map(|(name, pins)| (name.as_str(), pins.iter().map(|(r, k)| (*r, k.as_str())).collect())).collect();
        let nets_ref: Vec<(&str, &[(&str, &str)])> = nets.iter().map(|(a, b)| (*a, b.as_slice())).collect();
        let mut all = nets_ref.clone();
        all.push(("Z", &[("C", "1"), ("C", "2")]));
        let (_, _, l, _) = drafted(&units, &all, &[&["A", "B"], &["C"]], 5.0);
        let tracks = l.channels[0].tracks;
        assert!(tracks >= 1 && tracks <= n, "{n} nets use {tracks} tracks");
        let want = grid_up(f64::max(20.0, 10.0 + 2.5 * tracks as f64));
        assert!((l.columns[1].left - l.columns[0].right - want).abs() < 1e-6, "{n}: {:?} want {want}", l.columns);
        assert!(((l.channels[0].right - l.channels[0].left) - want).abs() < 1e-6);
    }
}

/// Planning the same request twice gives the same wires once the random ids are left out.
/// 同じ依頼を2回計画すると、ランダムなidを除けば、全く同じ配線になる。
#[test]
fn planning_twice_gives_the_same_channel_wires() {
    let key = |p: &DraftPlan| {
        let mut v: Vec<String> = wires_of(&p.sheet).iter().map(|w| format!("{:?}", w.points)).collect();
        v.extend(junctions_of(&p.sheet).iter().map(|j| format!("J{j:?}")));
        v.sort();
        v
    };
    let (_, _, _, a) = stacked_three();
    let (_, _, _, b) = stacked_three();
    assert_eq!(key(&a), key(&b));
    assert_eq!((a.wired, a.labelled), (b.wired, b.labelled));
}

/// A net whose channel wire is blocked (here by a keep-out area over its trunk) is handed to the grid router, which still joins it, and the connections stay the same.
/// 通り道の配線がふさがれたネット (ここでは幹の上に立入禁止の領域を置く) は格子の経路探索に渡され、それでもつながり、つながりは変わらない。
#[test]
fn a_blocked_channel_net_is_taken_by_the_grid_router() {
    let (src, m, mut l, _) = stacked_three();
    let (free, _, _) = route_wires(&m, &l, &RouteOptions::default());
    let before = trunks(&free, &l, 0);
    assert_eq!(before.len(), 3);
    let (x, y0, y1) = before[0];
    l.keep_out.push(Rect::from_corners(Point::new(x - 0.5, (y0 + y1) / 2.0 - 2.0), Point::new(x + 0.5, (y0 + y1) / 2.0 + 2.0)));
    let (sheet, wired, labelled) = route_wires(&m, &l, &RouteOptions::default());
    assert!(labelled.is_empty(), "{labelled:?}");
    assert_eq!(wired.len(), 3);
    let after = trunks(&sheet, &l, 0);
    assert!(!after.iter().any(|t| (t.0 - x).abs() < 1e-6 && t.1 <= (y0 + y1) / 2.0 && t.2 >= (y0 + y1) / 2.0), "no wire passes the keep-out area on that track: {after:?}");
    assert_eq!(device_groups(&sheet, &["A", "B", "C"]), device_groups(&src.sheets[0], &["A", "B", "C"]));
}

/// On the core, bars and stacked fixtures, at both pitches, the drafted sheet carries the connections of the source, and no wire touches a tag flag, a mark, a text box or a box.
/// 基本・棒・積んだ見本のどれでも、どちらのピン間隔でも、下書きのシートは元の図と同じつながりを持ち、札の旗・印・文字の枠・箱に触れる配線は無い。
#[test]
fn drafted_fixtures_keep_the_connections_and_wires_off_everything() {
    let cases: Vec<(String, Project, DraftRequest, Vec<&str>)> = {
        let mut v = Vec::new();
        for pitch in [5.0, 2.5] {
            let mut r = request(&[&["U1", "M1"], &["U2"], &["TB1", "R1"]]);
            r.ground_nets = None;
            r.pitch_mm = pitch;
            v.push((format!("core {pitch}"), fixture_source(), r, vec!["U1", "M1", "U2", "TB1", "R1"]));
            let mut r = request(&[&["UA"], &["UB"], &["UC"]]);
            r.pitch_mm = pitch;
            v.push((format!("bars {pitch}"), bars_source(), r, vec!["UA", "UB", "UC"]));
            for cols in stacked_columns() {
                let r = stacked_request(&cols, pitch);
                v.push((format!("stacked {cols:?} {pitch}"), stacked_source(), r, cols.iter().flat_map(|c| c.iter().copied()).collect()));
            }
        }
        v
    };
    for (name, src, req, refs) in cases {
        let open = stacked_open();
        let p = plan_function_sheet(&src, &open, &req).expect("fits");
        assert_eq!(device_groups(&p.sheet, &refs), device_groups(&src.sheets[0], &refs), "{name}");
        let bad = wires_touching_flags(&with_sheet(&open, &p.sheet), &p.sheet);
        assert!(bad.is_empty(), "{name}: {bad:#?}");
        let defs = sheet_symbol_defs(&p.sheet);
        let texts = crate::tidy::text_boxes(&p.sheet, &defs);
        let index = DefIndex::new(&defs);
        let boxes: Vec<Rect> = p
            .sheet
            .entities
            .values()
            .filter_map(|e| if let Entity::Symbol(s) = e { Some(s) } else { None })
            .filter(|s| !s.reference.is_empty())
            .filter_map(|s| {
                if s.symbol_id == BLOCK_SYMBOL_ID {
                    let (lo, hi) = block::block_outline(&block::parse_block_spec(&s.attrs).ok()?);
                    Some(Rect::from_corners(Point::new(s.at.x + lo.x, s.at.y + lo.y), Point::new(s.at.x + hi.x, s.at.y + hi.y)))
                } else {
                    {
                    let def = index.get(s)?;
                    crate::tidy::symbol_bounds(s, &def)
                }
                }
            })
            .collect();
        let marks: Vec<Rect> = marks_of(&p.sheet).iter().map(|m| super::look_tests::mark_rect(&p.sheet, m)).collect();
        for w in wires_of(&p.sheet) {
            for s in w.points.windows(2) {
                let r = seg_rect(s[0], s[1]);
                for t in texts.iter().filter(|t| t.kind != crate::tidy::TextKind::WireNumber) {
                    // a wire may end at a printed pin number's own stub: text of a mark belongs to its own lead
                    if marks.iter().any(|m| m.overlaps(&t.rect)) {
                        continue;
                    }
                    assert!(!r.overlaps(&t.rect), "{name}: wire {:?}-{:?} on the text {}", s[0], s[1], t.text);
                }
                for b in &boxes {
                    let inner = Rect::from_corners(Point::new(b.min.x + 0.3, b.min.y + 0.3), Point::new(b.max.x - 0.3, b.max.y - 0.3));
                    assert!(!r.overlaps(&inner), "{name}: wire {:?}-{:?} through a box", s[0], s[1]);
                }
            }
        }
    }
}

/// Opening many new tracks between the same two tracks keeps them apart: the tracks never collapse onto one position, so a net never shorts with or is refused because of a track it does not touch.
/// 同じ2本のトラックのあいだに何度も新しいトラックを開いても、トラックは1つの位置に重ならない。触れていないトラックのせいで短絡したり断られたりはしない。
#[test]
fn many_tracks_opened_in_one_gap_stay_apart() {
    // A has its terminals on the right at y=0 and y=50. Each R(i+1) has its left terminal above and its right terminal above
    // the ones of R(i), so its best position (no crossing) is always between R(i) and A: forty tracks opened in one gap
    let mut nets = vec![chan_net(100, "A", &[(0.0, false), (50.0, false)])];
    for i in 0..40 {
        nets.push(chan_net(i, &format!("R{}", i + 1), &[(45.0 - i as f64, true), (100.0 - i as f64, false)]));
    }
    let a = assign_tracks(&nets);
    assert!(a.unplaced.is_empty(), "{a:?}");
    assert_eq!(a.tracks, 41);
    for i in 0..40 {
        assert_eq!(track_of(&a, i), Some(i), "R{} is the {i}th track from the left", i + 1);
    }
    assert_eq!(track_of(&a, 100), Some(40), "A stays right of every R");
}
