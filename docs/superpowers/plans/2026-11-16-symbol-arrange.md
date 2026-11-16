# Symbol Arrange Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A command that aligns the symbols of a sheet into columns and rows, opens routing channels between them without changing their order, and redraws every wire, with connectivity unchanged and one undo.

**Architecture:** A pure planner in `madake-core/src/arrange.rs` computes units, bands, moves and a monotone coordinate warp, then routes the recorded wire clusters on the moved geometry with the reroute engine (split out of `plan_reroute` into `collect_route_units` and `route_units`). Clusters that cannot be routed are connected by labels. The MCP tool, the Link API and the CLI apply the plan as one batch of existing commands.

**Tech Stack:** Rust (madake-core, madake-mcp with rmcp and axum, madake-cli with clap), serde, schemars.

**Spec:** `docs/superpowers/specs/2026-11-16-symbol-arrange-design.md`

## Global Constraints

- Every edit goes through existing `Command`s (`DeleteEntities`, `MoveEntities`, `UpdateEntity`, `AddEntity`) applied with `Engine::execute_batch` as one undo step. No new `Command`, no format change, no `format_version` bump.
- Every Rust test has two `///` lines right before `#[test]`/`#[tokio::test]`: English first, Japanese second, written so a beginner understands what happens.
- After adding or changing tests run `python3 scripts/gen_spec.py` and commit `docs/13-specification.md` and `docs/13-specification.ja.md`.
- Code comments and user-facing messages are Japanese (match the surrounding code); the plan and spec are English.
- Defaults: `channel_mm` 8.0, `max_gap_mm` 20.0, `align_mm` 4.0, satellite text distance 3.0 mm, move step 2.5 mm.
- Option validation: `channel_mm` in [2.5, 50], `max_gap_mm` >= `channel_mm` and <= 200, `align_mm` in [0, 20], all finite; the nested `route: RouteOptions` must pass `RouteOptions::validate`. Invalid options are an error before the engine lock is taken, nothing changes.
- Reroute behaviour must not change: every existing test in `route.rs` and `crates/madake-mcp/tests/reroute_api.rs` passes unchanged.
- Test data is invented (no names, values or layouts from private drawings).
- Commit messages: Japanese, conventional prefix, never any `Co-Authored-By` or other attribution trailer.
- Run `python3 scripts/audit_confidential.py` before each commit; it must print `no confidential terms found`.
- Rust lives in `~/.cargo/bin`: `export PATH="$HOME/.cargo/bin:$PATH"`. Tests run from `src-tauri/`.
- Never start or stop the app on port 9310.

## Review Focus

- A label or power mark wired to two different symbols: it must become its own unit, not be dragged with one of them (Task 2 test `a_label_between_two_symbols_is_its_own_unit`).
- A sheet with nothing but wires and texts (no symbols): arrange must succeed with no moves, not panic on empty bands (Task 2 test `an_empty_sheet_gives_no_bands`, Task 3 test `a_sheet_without_symbols_changes_nothing`).
- Symbols placed off the 2.5 mm grid (`free_pins`): moves are still multiples of 2.5 mm, so pins keep their offset and wires still land on them (Task 3 test `off_grid_symbols_keep_their_offset_and_stay_connected`).
- Two clusters of the same unnamed net on one sheet (joined only by a junction-free label pair): label fallback must not merge them with an unrelated net (Task 3 test `fallback_names_never_join_unrelated_nets`).
- Running arrange twice: the second run must not move anything further when the first already met the channel and alignment rules (Task 3 test `arranging_twice_does_not_move_more`).

---

## File Structure

- Modify `src-tauri/crates/madake-core/src/route.rs`: split `plan_reroute` into `collect_route_units` and `route_units`; expose `RouteUnit`, `KeptCluster`, `KeepReason`, `make_lead`, `emit` as `pub(crate)`.
- Create `src-tauri/crates/madake-core/src/arrange.rs`: options, units, bands, warp (Task 2) and `plan_arrange` (Task 3).
- Modify `src-tauri/crates/madake-core/src/lib.rs`: `pub mod arrange;`.
- Modify `src-tauri/crates/madake-mcp/src/lib.rs`: `SharedDoc::arrange_sheet`, MCP tool `arrange_sheet`.
- Modify `src-tauri/crates/madake-mcp/src/link_api.rs`: `POST /api/v1/route/arrange`.
- Create `src-tauri/crates/madake-mcp/tests/arrange_api.rs`.
- Modify `src-tauri/crates/madake-cli/src/{client.rs,cli.rs,format.rs}`: `madake arrange`.
- Modify docs: `docs/10-automation-api.md`, `docs/10-automation-api.ja.md`, `docs/internal/feature-inventory.md`, `CLAUDE.md` (CLI line), regenerated `docs/13-specification*.md`.

---

### Task 1: Split the reroute engine into collect and route

**Files:**
- Modify: `src-tauri/crates/madake-core/src/route.rs` (the `Unit` struct near line 762, `emit` near 1002, `make_lead` near 1134, `plan_reroute` 1157-1525, tests module)

**Interfaces:**
- Consumes: nothing new.
- Produces (all `pub(crate)` in `crate::route`):

```rust
/// 引き直さずに残したかたまりの理由 (かたまり単位で決まるもの)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeepReason { NoPath, Pointed, Mixed, LengthSources, FreeEnd, FewTerminals }

impl KeepReason {
    /// 今までの返り値と同じ英語の理由。
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            KeepReason::NoPath => "a wire has no path",
            KeepReason::Pointed => "another entity points at a wire or junction here (cable core, note leader, dimension or FreeCAD link)",
            KeepReason::Mixed => "wires differ in colour, cross-section, part number, wire number, dash or weight",
            KeepReason::LengthSources => "wires carry lengths from different sources (length_source)",
            KeepReason::FreeEnd => "a wire has a free end that touches nothing",
            KeepReason::FewTerminals => "fewer than two terminals",
        }
    }
}

/// 引き直さずに残した、配線で物理的につながった1かたまり。
#[derive(Debug, Clone)]
pub(crate) struct KeptCluster {
    pub net: String,
    pub wires: Vec<EntityId>,
    pub junctions: Vec<EntityId>,
    /// かたまりの上のピンとラベルの位置と向き (ラベルはNone)。
    pub terms: Vec<(Point, Option<Dir>)>,
    pub reason: KeepReason,
}

/// ネット単位で残したネットの理由 (選ばれていない・ポートの接点で共有・ケーブル)。
pub(crate) struct Collected {
    pub units: Vec<RouteUnit>,
    /// 動かない配線 (ネット単位で残した配線と、かたまり単位で残した配線)。
    pub fixed_wires: Vec<EntityId>,
    pub kept: Vec<KeptCluster>,
    /// (ネット名, 理由) — 今までの `ReroutePlan::unchanged` に入っていたものと同じ順・同じ文。
    pub unchanged: Vec<(String, String)>,
}

pub(crate) struct RouteUnit { /* the old private `Unit`, renamed, fields made pub(crate):
    net, wires, junctions, template, old_lines, old_junctions, terms, leads */ }

pub(crate) fn collect_route_units(sheet: &Sheet, defs: &[SymbolDef], opts: &RerouteOptions, protected: &[EntityId]) -> Collected;

/// 単位の並びを格子の上に引く。返り値は単位ごとに、引けた経路か、諦めた理由。
/// `fixed_lines`/`fixed_spots` は動かない配線の点列とジャンクションの位置 (占有と、引き出し線の接触の検査に使う)。
pub(crate) fn route_units(grid: &RouteGrid, units: &mut [RouteUnit], fixed_lines: Vec<Vec<Point>>, fixed_spots: Vec<Point>) -> Vec<Result<UnitRoute, &'static str>>;

pub(crate) fn emit(unit: &RouteUnit, route: &UnitRoute) -> Vec<Entity>;
pub(crate) fn make_lead(grid: &RouteGrid, pos: Point, dir: Option<Dir>) -> Option<Lead>;
pub(crate) struct UnitRoute { /* unchanged fields, pub(crate) */ }
```

- [ ] **Step 1: Write the failing tests** (in the existing `mod tests` of `route.rs`; reuse its helpers `new_sheet`, `place`, `defs`, `p`; add a local `wire` helper only if the module has none, by reading the existing tests first)

```rust
    /// Collecting the wires of a sheet sorts two simple clusters into routable units and keeps none.
    /// シートの配線を集めると、単純な2つのかたまりは引き直せる単位になり、残すものは無い。
    #[test]
    fn collecting_finds_simple_clusters_as_units() {
        let mut sheet = new_sheet();
        place(&mut sheet, "box2", (50.0, 50.0), "A1");
        place(&mut sheet, "box2", (100.0, 50.0), "A2");
        place(&mut sheet, "box2", (50.0, 100.0), "A3");
        place(&mut sheet, "box2", (100.0, 100.0), "A4");
        add_wire(&mut sheet, &[p(60.0, 50.0), p(90.0, 50.0)]);
        add_wire(&mut sheet, &[p(60.0, 100.0), p(90.0, 100.0)]);
        let c = collect_route_units(&sheet, &defs(), &RerouteOptions::default(), &[]);
        assert_eq!(c.units.len(), 2);
        assert!(c.kept.is_empty());
        assert!(c.fixed_wires.is_empty());
    }

    /// A cluster with a free end is kept, and the kept record lists its wire and the reason.
    /// 端が浮いたかたまりは残され、残した記録にその配線と理由が入る。
    #[test]
    fn collecting_records_a_free_end_cluster() {
        let mut sheet = new_sheet();
        place(&mut sheet, "box2", (50.0, 50.0), "A1");
        let w = add_wire(&mut sheet, &[p(60.0, 50.0), p(80.0, 50.0)]);
        let c = collect_route_units(&sheet, &defs(), &RerouteOptions::default(), &[]);
        assert!(c.units.is_empty());
        assert_eq!(c.kept.len(), 1);
        assert_eq!(c.kept[0].wires, vec![w]);
        assert_eq!(c.kept[0].reason, KeepReason::FreeEnd);
        assert_eq!(c.kept[0].terms.len(), 1);
    }
```

If the tests module has no wire helper, add:

```rust
    fn add_wire(sheet: &mut Sheet, points: &[Point]) -> EntityId {
        let id = Uuid::new_v4();
        sheet.entities.insert(id, Entity::Wire(Wire { id, points: points.to_vec(), ..Default::default() }));
        id
    }
```

(Check that `Wire` implements `Default`; if it does not, build it the way the existing tests build wires and reuse that helper instead.)

- [ ] **Step 2: Run the tests to see them fail**

Run: `cd src-tauri && cargo test -p madake-core route::tests::collecting_ 2>&1 | tail -5`
Expected: compile error, `collect_route_units` not found.

- [ ] **Step 3: Refactor**

1. Rename `struct Unit` to `pub(crate) struct RouteUnit` and make its fields `pub(crate)`; make `UnitRoute` and its fields `pub(crate)`; make `emit` and `make_lead` `pub(crate)`. Update all uses.
2. Add `KeepReason`, `KeptCluster`, `Collected` exactly as in Interfaces.
3. Move lines from `let mut plan = ReroutePlan::default();` through the loop that pushes `"no such net"` (currently 1160-1305) into `collect_route_units`. Changes inside:
   - push net-level reasons (`not selected`, port contacts, cable) to `unchanged` as now, and their wires to `fixed_wires` as now;
   - in the cluster loop, the `keep` closure gets a `KeepReason` instead of a string: it pushes `(net.name, reason.as_str())` to `unchanged`, extends `fixed_wires`, and pushes a `KeptCluster { net, wires, junctions: group_junctions ids, terms, reason }` to `kept`;
   - terminals (`terms`) are computed **before** the checks so that every `KeptCluster` has them (same code as now: pins then labels on the group, sorted by `quant`); the `terms.len() < 2` check stays last, with `KeepReason::FewTerminals`.
   - order of checks unchanged: `NoPath`, `Pointed`, `Mixed`, `FreeEnd`, `LengthSources`, `FewTerminals`.
4. Move the routing part (from `// 動かない配線と、引き直さないネットのジャンクション` through the end of the `while let Some(u) = queue.pop_front()` loop) into `route_units`. It takes the fixed geometry as point lists instead of ids:
   - occupancy: `rasterize` every line of `fixed_lines`, `block_point` every spot of `fixed_spots`;
   - `fixed_lines`/`fixed_spots` are the mutable lists used by the lead checks and by `settle_failed` (as now);
   - returns `Vec<Result<UnitRoute, &'static str>>`: `Ok(route)` when `routes[i]` is `Some` and `!failed[i]`, else `Err(why[i].unwrap_or("no route found; the old wires are kept"))`.
5. `plan_reroute` becomes:

```rust
pub fn plan_reroute(sheet: &Sheet, defs: &[SymbolDef], opts: &RerouteOptions, protected: &[EntityId]) -> Result<ReroutePlan, String> {
    use std::collections::{BTreeSet, HashSet};
    opts.route.validate()?;
    let grid = RouteGrid::build(sheet, defs, &opts.route);
    let mut c = collect_route_units(sheet, defs, opts, protected);
    let fixed_lines: Vec<Vec<Point>> = c
        .fixed_wires
        .iter()
        .filter_map(|id| if let Some(Entity::Wire(w)) = sheet.entities.get(id) { Some(w.points.clone()) } else { None })
        .collect();
    let moving: HashSet<EntityId> = c.units.iter().flat_map(|u| u.junctions.iter().copied()).collect();
    let fixed_spots: Vec<Point> = sheet
        .entities
        .values()
        .filter_map(|e| if let Entity::Junction(j) = e { (!moving.contains(&j.id)).then_some(j.at) } else { None })
        .collect();
    let results = route_units(&grid, &mut c.units, fixed_lines, fixed_spots);
    let mut plan = ReroutePlan { unchanged: c.unchanged, ..Default::default() };
    let mut names: BTreeSet<String> = BTreeSet::new();
    for (u, r) in c.units.iter().zip(results) {
        match r {
            Ok(route) => {
                plan.remove.extend(u.wires.iter().copied());
                plan.remove.extend(u.junctions.iter().copied());
                plan.add.extend(emit(u, &route));
                names.insert(u.net.clone());
            }
            Err(why) => plan.unchanged.push((u.net.clone(), why.to_string())),
        }
    }
    plan.rerouted = names.into_iter().collect();
    Ok(plan)
}
```

   Keep the existing doc comment of `plan_reroute`. Check that the order of `fixed_lines` matches the old code (old code: `fixed_wires` order, then failed units' old lines appended inside the loop); `fixed_spots` order does not affect results (only `any` checks).

- [ ] **Step 4: Run all route and reroute tests**

Run: `cd src-tauri && cargo test -p madake-core route:: 2>&1 | tail -3 && cargo test -p madake-mcp --test reroute_api 2>&1 | tail -3`
Expected: all pass, including the two new tests; no existing test changed.

- [ ] **Step 5: Regenerate the spec and commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/audit_confidential.py
git add src-tauri/crates/madake-core/src/route.rs docs/13-specification.md docs/13-specification.ja.md
git commit -m "refactor: 配線の引き直しを、かたまりの収集と経路の探索に分ける"
```

---

### Task 2: Units, bands and the coordinate warp

**Files:**
- Create: `src-tauri/crates/madake-core/src/arrange.rs`
- Modify: `src-tauri/crates/madake-core/src/lib.rs` (add `pub mod arrange;` in alphabetical order, before `pub mod block;`)

**Interfaces:**
- Consumes: `crate::route::{pin_points, RouteOptions}`, `crate::leader::entity_bounds`, `crate::symbol::DefIndex` (check its path with `grep -rn "pub struct DefIndex" src-tauri/crates/madake-core/src`), `crate::netlist::extract_netlist`, `crate::tidy::Rect` (check fields and constructors in `tidy.rs` before use; if `leader::entity_bounds` returns a different `Rect` type, use that one).
- Produces (`pub` in `crate::arrange`):

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default)]
pub struct ArrangeOptions {
    /// 列と列・行と行の間にあける通り道の幅 mm (既定8)。
    pub channel_mm: f64,
    /// これより広い空きはこの幅まで詰める mm (既定20)。
    pub max_gap_mm: f64,
    /// 中心の差がこれ未満の部品をそろえる mm (既定4)。
    pub align_mm: f64,
    /// 経路探索の設定 (引き直しと同じ)。
    pub route: crate::route::RouteOptions,
}
impl Default for ArrangeOptions { /* 8.0, 20.0, 4.0, RouteOptions::default() */ }
impl ArrangeOptions { pub fn validate(&self) -> Result<(), String>; }

/// 一緒に動くもの: 主のシンボル (またはラベル・1ピンのシンボル) と、その衛星・近くの文字。
#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    pub main: EntityId,
    pub members: Vec<EntityId>,      // main を含む、id順
    pub anchor: Point,               // main の位置 (シンボルの at、ラベルの at)
    pub bounds: (f64, f64, f64, f64), // (x0, y0, x1, y1) 全メンバーの外接
}

pub fn units(sheet: &Sheet, defs: &[SymbolDef]) -> Vec<Unit>;

/// 帯 (列または行): 区間と、属する単位の添字。
#[derive(Debug, Clone, PartialEq)]
pub struct Band { pub lo: f64, pub hi: f64, pub members: Vec<usize> }

/// 区間 (lo, hi) の並びを、重なるものどうしまとめた帯にする。lo順。
pub fn bands(spans: &[(f64, f64)]) -> Vec<Band>;

/// 帯ごとのずらし量 (2.5mmの倍数)。間の空きを [channel, max_gap] に収める。先頭の帯は動かない。
pub fn band_shifts(bands: &[Band], channel: f64, max_gap: f64) -> Vec<f64>;

/// 帯の外の点を写す単調な区分線形の写像。
pub struct Warp { /* knots: Vec<(old, new)> */ }
impl Warp {
    pub fn new(bands: &[Band], shifts: &[f64]) -> Self;
    pub fn map(&self, v: f64) -> f64;
}

/// 帯の中で、基準の座標 (keys) の差が tol 未満の組を、組の下側の中央値へそろえるずらし量 (2.5mmの倍数)。
pub fn align_moves(keys: &[f64], tol: f64) -> Vec<f64>;

pub const STEP_MM: f64 = 2.5;
pub const SATELLITE_TEXT_MM: f64 = 3.0;
```

Rules for `units` (from the spec):
1. Pins of the sheet come from `pin_points`. Wire clusters: use `extract_netlist(sheet, defs)` and, for each net, its `pins` (`NetPin`; read its fields in `netlist.rs`) together with the labels touching the net's wires (a `NetLabel` whose `at` lies on one of the net's wires; use the same on-wire test the netlist uses — find it with `grep -n "fn on_wire" src-tauri/crates/madake-core/src/*.rs`).
2. A symbol whose definition has exactly one pin, or a `NetLabel`, is a **satellite** of symbol S when every other pin on its net belongs to S and the net has no other label and no other one-pin symbol. Otherwise it is its own unit.
3. Every other symbol is the main of a unit.
4. A `Text` whose `leader_to` is a member of a unit joins that unit. Any other `Text` joins the unit whose main symbol's bounds are nearest to the text's `at`, if that distance is at most `SATELLITE_TEXT_MM`; a tie goes to the smaller main id.
5. `bounds` is the union of `entity_bounds` of all members (skip members with no bounds). A unit whose main has no bounds uses the anchor point.
6. Order: units sorted by main id (deterministic).

Rules for `bands`: sort spans by `lo` (ties by index); start a band with the first span; a span joins the current band when `lo < band.hi - 1e-6`; `band.hi = max(band.hi, hi)`.

Rules for `band_shifts`: `s[0] = 0`; for each gap `g = bands[i+1].lo - bands[i].hi`: if `g < channel`, `d = ceil((channel - g) / STEP_MM) * STEP_MM`; else if `g > max_gap`, `d = -floor((g - max_gap) / STEP_MM) * STEP_MM`; else `d = 0`; `s[i+1] = s[i] + d`.

Rules for `Warp`: knots are, for each band, `(lo, lo + s)` and `(hi, hi + s)`; `map` interpolates linearly between consecutive knots, and before the first or after the last knot adds that knot's shift. No bands: identity.

Rules for `align_moves`: sort indices by key; walk in order and start a new group whenever `key - previous key >= tol`; a group's target is the key of its lower-median member (`members[(n - 1) / 2]` in sorted order); move = `((target - key) / STEP_MM).round() * STEP_MM`. `tol == 0` gives all zeros.

`validate` messages are Japanese and name the field, for example `"channel_mm は 2.5〜50 mm の範囲で指定してください"`.

- [ ] **Step 1: Write the failing tests** (`#[cfg(test)] mod tests` at the end of `arrange.rs`; copy the small `def`/`pin`/`rect`/`place` helpers from `route.rs` tests, they are private there)

```rust
    /// Overlapping spans merge into one band and separate spans stay apart, in order.
    /// 重なる区間は1つの帯にまとまり、離れた区間は別の帯のまま順に並ぶ。
    #[test]
    fn overlapping_spans_merge_into_bands() {
        let b = bands(&[(30.0, 40.0), (0.0, 10.0), (5.0, 15.0)]);
        assert_eq!(b.len(), 2);
        assert_eq!((b[0].lo, b[0].hi), (0.0, 15.0));
        assert_eq!(b[0].members, vec![1, 2]);
        assert_eq!(b[1].members, vec![0]);
    }

    /// No spans give no bands.
    /// 区間が無ければ帯も無い。
    #[test]
    fn an_empty_sheet_gives_no_bands() {
        assert!(bands(&[]).is_empty());
        assert_eq!(Warp::new(&[], &[]).map(12.3), 12.3);
    }

    /// A gap narrower than the channel widens to at least the channel, in 2.5 mm steps.
    /// 通り道より狭い空きは、2.5mm刻みで通り道の幅以上に広がる。
    #[test]
    fn narrow_gaps_widen_to_the_channel() {
        let b = bands(&[(0.0, 10.0), (13.0, 20.0)]);
        let s = band_shifts(&b, 8.0, 20.0);
        assert_eq!(s, vec![0.0, 5.0]);
    }

    /// A gap wider than the maximum shrinks, but never below the maximum.
    /// 上限より広い空きは詰まるが、上限より狭くはならない。
    #[test]
    fn wide_gaps_shrink_to_the_maximum() {
        let b = bands(&[(0.0, 10.0), (51.0, 60.0)]);
        let s = band_shifts(&b, 8.0, 20.0);
        assert_eq!(s, vec![0.0, -20.0]);
        assert!(51.0 + s[1] - 10.0 >= 20.0);
    }

    /// The warp moves points inside a band with the band and stretches the gaps between bands, keeping order.
    /// 写像は帯の中の点を帯と一緒に動かし、帯の間の空きは順序を保ったまま伸び縮みさせる。
    #[test]
    fn the_warp_is_monotone_and_follows_bands() {
        let b = bands(&[(0.0, 10.0), (13.0, 20.0)]);
        let w = Warp::new(&b, &[0.0, 5.0]);
        assert_eq!(w.map(5.0), 5.0);
        assert_eq!(w.map(15.0), 20.0);
        assert_eq!(w.map(11.5), 14.0);
        assert_eq!(w.map(100.0), 105.0);
        let xs: Vec<f64> = (0..250).map(|i| i as f64 * 0.1).collect();
        assert!(xs.windows(2).all(|p| w.map(p[0]) <= w.map(p[1])));
    }

    /// Nearly equal keys line up on the lower median; distant keys stay put.
    /// ほぼ同じ座標は下側の中央値にそろい、離れた座標は動かない。
    #[test]
    fn nearly_aligned_keys_snap_to_the_median() {
        let m = align_moves(&[100.0, 102.5, 140.0], 4.0);
        assert_eq!(m, vec![0.0, -2.5, 0.0]);
        assert_eq!(align_moves(&[100.0, 102.5], 0.0), vec![0.0, 0.0]);
    }

    /// A power mark wired only to one part becomes that part's satellite and moves with it.
    /// 1つの部品だけにつながる電源マークは、その部品の衛星になり一緒に動く。
    #[test]
    fn a_mark_wired_to_one_part_is_its_satellite() { /* place box2 A1 at (50,50); a one-pin symbol "mark" (define it with one pin at (0,-5) pointing Up) at (60, 60) wired from A1 pin 2 (60,50) to the mark pin (60,55); assert units(...) has 1 unit whose members contain both ids */ }

    /// A label wired to pins of two different parts is its own unit.
    /// 2つの部品のピンにつながるラベルは、それだけで1つの単位になる。
    #[test]
    fn a_label_between_two_symbols_is_its_own_unit() { /* A1 at (50,50), A2 at (100,50), wire (60,50)-(90,50), NetLabel at (75,50); assert 3 units, one with main = label id */ }

    /// A note within 3 mm of a part moves with it; a note far away belongs to no unit.
    /// 部品から3mm以内の注記は部品と一緒に動き、遠い注記はどの単位にも入らない。
    #[test]
    fn nearby_notes_join_their_part() { /* box2 at (50,50) has bounds x 40..60 (pins included or not: read svg::symbol_bounds); Text at (50, 57) joins; Text at (50, 80) joins none */ }

    /// Out-of-range options are refused with a message naming the field.
    /// 範囲外の設定は、項目名を含むメッセージで断られる。
    #[test]
    fn bad_options_are_refused() {
        let mut o = ArrangeOptions::default();
        o.channel_mm = 1.0;
        assert!(o.validate().unwrap_err().contains("channel_mm"));
        let mut o = ArrangeOptions::default();
        o.max_gap_mm = 5.0;
        assert!(o.validate().unwrap_err().contains("max_gap_mm"));
        let mut o = ArrangeOptions::default();
        o.align_mm = f64::NAN;
        assert!(o.validate().unwrap_err().contains("align_mm"));
        assert!(ArrangeOptions::default().validate().is_ok());
    }
```

The three tests with `/* ... */` bodies must be written out in full by the implementer following the comment; they are behaviour tests, each with at least one `assert`.

- [ ] **Step 2: Run them to see them fail**

Run: `cd src-tauri && cargo test -p madake-core arrange:: 2>&1 | tail -5`
Expected: compile errors (functions missing).

- [ ] **Step 3: Implement** `ArrangeOptions`, `units`, `bands`, `band_shifts`, `Warp`, `align_moves` following the rules above. Module doc comment (Japanese) states what arrange does and that it never edits the sheet itself.

- [ ] **Step 4: Run the tests**

Run: `cd src-tauri && cargo test -p madake-core arrange:: 2>&1 | tail -3`
Expected: all pass.

- [ ] **Step 5: Regenerate the spec and commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/audit_confidential.py
git add src-tauri/crates/madake-core/src/arrange.rs src-tauri/crates/madake-core/src/lib.rs docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat: 部品の並べ直しの単位・帯・座標の写像を追加"
```

---

### Task 3: `plan_arrange`

**Files:**
- Modify: `src-tauri/crates/madake-core/src/arrange.rs`

**Interfaces:**
- Consumes: Task 1 `collect_route_units`, `route_units`, `emit`, `make_lead`, `RouteGrid::build`, `KeptCluster`, `KeepReason`; Task 2 everything.
- Produces:

```rust
/// 並べ直しに要る図面の外の情報。
#[derive(Debug, Clone, Default)]
pub struct ArrangeContext {
    /// シートの外から指されているid (FreeCADとの対応付け)。
    pub protected: Vec<EntityId>,
    /// 図枠の内側 (x0, y0, x1, y1)。
    pub frame: (f64, f64, f64, f64),
    /// 表題欄 (x0, y0, x1, y1)。
    pub title_block: (f64, f64, f64, f64),
    /// プロジェクトで使われているラベル名とネット名シンボルの名前。
    pub taken_names: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ArrangePlan {
    /// まとめて動かすid と (dx, dy)。
    pub moves: Vec<(Vec<EntityId>, f64, f64)>,
    /// 形を写して置き換えるエンティティ (図形・離れた配線・ハーネスなど)。
    pub updates: Vec<Entity>,
    pub remove: Vec<EntityId>,
    pub add: Vec<Entity>,
    pub moved: usize,
    pub columns: usize,
    pub rows: usize,
    /// 配線を引き直したネット (重複なし、名前順)。
    pub routed: Vec<String>,
    /// ラベルでつないだかたまり (ネット名, ラベル名)。
    pub labelled: Vec<(String, String)>,
    pub size_before: (f64, f64),
    pub size_after: (f64, f64),
    /// 計画を当てた後のシート (数値を測るための写し)。
    #[serde(skip)]
    pub after: Option<Sheet>,
}

pub fn plan_arrange(sheet: &Sheet, defs: &[SymbolDef], opts: &ArrangeOptions, ctx: &ArrangeContext) -> Result<ArrangePlan, String>;
```

Algorithm (each step is a private function with its own doc comment):

1. `opts.validate()?`. Refuse (`Err`, Japanese message) when `sheet.kind` is not the schematic kind (check the enum in `model.rs`) or the sheet holds any `Entity::Cable` or panel-layout entity.
2. `collect_route_units(sheet, defs, &RerouteOptions { nets: vec![], route: opts.route.clone() }, &ctx.protected)`. Refuse when `unchanged` contains a net-level reason other than `"not selected"` (port contacts, cable), or when any `kept` cluster has `KeepReason::Pointed` or `KeepReason::LengthSources`; the message names the nets.
3. `let us = units(sheet, defs)`. Map every entity id to its unit index. A terminal point belongs to the unit of the symbol whose pin is there (`pin_points`) or of the label at that point.
4. Classify kept clusters: terminals all in one unit, or no terminal: **rigid** (wires and junctions move with that unit, or through the warp when no terminal); else when the cluster's labels have different names: refuse; else **label fallback**.
5. Columns: `bands` over unit x-spans; `align_moves` per column over the units' `anchor.x`; skip a column's alignment group if applying it makes two units' bounds overlap that did not overlap before. Recompute x-spans with the alignment moves, `bands` again, `band_shifts(channel_mm, max_gap_mm)`, `Warp`. Unit dx = alignment move + its band's shift. Same on y with rows. `columns`/`rows` = band counts after alignment.
6. Fit: the moved content bounds (all unit bounds moved, all warped shapes/texts/wires) must lie inside `ctx.frame` and, if the original content bounds did not intersect `ctx.title_block`, must not intersect it. If not, redo step 5 with `max_gap_mm = channel_mm`; if still not, refuse with `"並べ直すと図枠に収まりません (幅 W mm, 高さ H mm)"` giving the size.
7. Build the moved copy: clone the sheet; translate unit members and rigid clusters' wires and junctions by their unit's (dx, dy); map every other `Shape`, `Text`, `Harness`, `Wire`, `Junction`, `WireLegend` through the warps (points via `Warp::map`; single-anchor entities by the warp of their anchor); remove the wires and junctions of all routable units and of all label-fallback clusters.
8. Terminals on the moved copy: for every `RouteUnit` from step 2, translate each `terms` point by the (dx, dy) of the unit that owns it; clear `old_lines`, `old_junctions` and `leads`. Build `RouteGrid::build(&moved, defs, &opts.route)` and call `route_units(&grid, &mut route_units_vec, rigid_lines, rigid_spots)` where `rigid_*` are the moved rigid/warped wires and the junctions left on the copy.
9. Each `Err` from step 8 and each label-fallback cluster from step 4 becomes labels: name = the cluster's label name if it has labels (then delete those labels), else the net name of a net-name symbol pin in the cluster (`SymbolDef::net`), else `ARR_<n>` with the smallest `n >= 1` not in `ctx.taken_names` and not given earlier in this plan. For every pin terminal: `make_lead(&grid, pos, Some(dir))`; the stub wire runs from the pin through the lead points (or 2.5 mm straight out along `dir` when there is no lead); a `NetLabel { name, at: stub end, rotation: 0, attrs: empty }` is added at the stub end. The stub wire takes the attributes of the old wire touching that terminal (for routed-unit failures, the unit's `template` with `length_m: None`). Record `(net, name)` in `labelled`.
10. Routed units: `emit(unit, &route)` into `add`, their old wires/junctions into `remove`, net names into `routed`.
11. Express the moves: group member ids by identical (dx, dy) into `moves` (skip (0, 0)); warped entities whose geometry changed go to `updates`; `remove` gets old wires/junctions of routed and fallback clusters and deleted labels; `add` gets new wires, junctions, stubs and labels.
12. Self-check: apply the plan to a clone (`remove`, then moves, then updates, then `add`), run `extract_netlist` on the clone and on the original, and compare the **pin partitions**: the set of sets of `(symbol id, pin number)` over nets with at least one pin. A difference is `Err("並べ直しの検算で接続が変わったため、何も変えませんでした")`. Store the clone in `after`; `size_before`/`size_after` are the content bounds' width and height.
13. `moved` = number of units with a non-zero move.

- [ ] **Step 1: Write the failing tests** (in `arrange.rs` tests; a helper `fn ctx() -> ArrangeContext` returns frame `(10.0, 10.0, 410.0, 287.0)` and title block `(230.0, 240.0, 410.0, 287.0)`; a helper `fn pin_partition(sheet, defs) -> BTreeSet<BTreeSet<(EntityId, String)>>`; a helper `fn apply(sheet, plan) -> Sheet` applying the plan the same way step 12 does)

```rust
    /// Two parts squeezed 3 mm apart are pushed to at least the channel width and stay wired the same way.
    /// 3mmしか離れていない2つの部品は通り道の幅以上に離され、つながり方は変わらない。
    #[test]
    fn squeezed_parts_get_a_channel_and_keep_their_wiring() { /* A1 box2 at (50,50) (bounds 45..55 box, pins at 40 and 60), A2 at (73,50): gap between unit bounds is small; wire A1.2 (60,50) to A2.1 (63,50). plan_arrange; apply; assert x-gap between unit bounds >= 8.0; assert pin_partition(before) == pin_partition(after); assert plan.routed non-empty */ }

    /// Parts keep their left-to-right and top-to-bottom order.
    /// 部品の左右・上下の並び順は変わらない。
    #[test]
    fn order_is_kept_in_both_directions() { /* 3x3 grid of box2 with irregular gaps 3..40 mm; after apply, sort by old x equals sort by new x, same for y */ }

    /// Parts in one column whose origins differ by 2.5 mm end on the same x.
    /// 同じ列で原点の差が2.5mmの部品は、同じxにそろう。
    #[test]
    fn nearly_aligned_parts_line_up() { /* cap at (100,50) and cap at (102.5,120): after, both at.x equal */ }

    /// A power mark, a tag label and a nearby note move together with their part.
    /// 電源マーク・タグのラベル・近くの注記は、部品と一緒に動く。
    #[test]
    fn satellites_move_with_their_part() { /* box2 A1 + one-pin mark wired to A1.1 + NetLabel on a stub from A1.2 + Text 2 mm below; another box2 squeezed next to it so A1's unit moves; assert all four moved by the same (dx, dy) */ }

    /// An enclosure rectangle around two parts stretches with the gap between them.
    /// 2つの部品を囲む枠は、部品の間の空きに合わせて伸びる。
    #[test]
    fn an_enclosure_stretches_with_its_contents() { /* Shape rect points around A1 and A2 squeezed; after, rect still contains both unit bounds */ }

    /// When the arranged drawing would not fit in the frame, nothing is planned and the reason gives the size.
    /// 並べた図面が図枠に収まらないときは計画を作らず、理由に大きさを書く。
    #[test]
    fn a_drawing_that_would_not_fit_is_refused() { /* frame (10,10,120,120), 10 box2 in a row 1 mm apart filling it; expect Err containing "図枠" */ }

    /// A cluster that cannot be routed is connected by labels with a fresh name, and pins still share the same nets.
    /// 引けないかたまりは新しい名前のラベルでつながり、ピンのネットの分け方は変わらない。
    #[test]
    fn an_unroutable_cluster_is_joined_by_labels() { /* A1 at (50,50) fully walled in by "wall_h"/"wall_v" defs (copy from route tests) except its pin stub, A2 far away, wired; plan; assert labelled has one entry with name "ARR_1"; assert partitions equal */ }

    /// Fallback names skip names already used in the project, so unrelated nets never join.
    /// 代わりの名前はプロジェクトで使われている名前を避けるので、関係ないネットがつながることはない。
    #[test]
    fn fallback_names_never_join_unrelated_nets() { /* same as above with ctx.taken_names = ["ARR_1"]; assert name "ARR_2" */ }

    /// A sheet with a cable is refused and nothing is planned.
    /// ケーブルのあるシートは断られ、計画は作られない。
    #[test]
    fn a_sheet_with_a_cable_is_refused() { /* insert a minimal Entity::Cable (build it as cable.rs tests do); expect Err */ }

    /// A wire stub with a free end on one part moves with that part unchanged in shape.
    /// 1つの部品から出て先が浮いている線は、形を変えずに部品と一緒に動く。
    #[test]
    fn a_free_stub_moves_with_its_part() { /* A1 with wire from (60,50) to (70,50) free end; squeezed neighbour forces a move; assert stub points translated by A1's move */ }

    /// A sheet with only wires and notes changes nothing.
    /// 配線と注記だけのシートでは何も変わらない。
    #[test]
    fn a_sheet_without_symbols_changes_nothing() { /* plan.moves, updates, remove, add all empty */ }

    /// Parts off the 2.5 mm grid move in 2.5 mm steps, keep their offset and stay connected.
    /// 2.5mmグリッドから外れた部品も2.5mm刻みで動き、ずれを保ったまま接続も保つ。
    #[test]
    fn off_grid_symbols_keep_their_offset_and_stay_connected() { /* freebox at (50,50) wired to box2 squeezed next to it; assert dx % 2.5 == 0 and partitions equal */ }

    /// Arranging an already arranged sheet moves nothing more.
    /// 並べ直した後のシートをもう一度並べ直しても、それ以上は動かない。
    #[test]
    fn arranging_twice_does_not_move_more() { /* plan, apply, plan again on the result: second plan.moves empty */ }
```

Every `/* ... */` body is written out in full by the implementer following its comment, with the asserts it names.

- [ ] **Step 2: Run them to see them fail**

Run: `cd src-tauri && cargo test -p madake-core arrange:: 2>&1 | tail -5`
Expected: compile error, `plan_arrange` missing.

- [ ] **Step 3: Implement** steps 1-13 above.

- [ ] **Step 4: Run arrange and route tests**

Run: `cd src-tauri && cargo test -p madake-core arrange:: route:: 2>&1 | tail -3`
Expected: all pass.

- [ ] **Step 5: Regenerate the spec and commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/audit_confidential.py
git add src-tauri/crates/madake-core/src/arrange.rs docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat: 部品の並べ直しの計画 (整列・通り道・引き直し・ラベルでの接続・検算) を追加"
```

---

### Task 4: MCP tool, Link API, CLI and documents

**Files:**
- Modify: `src-tauri/crates/madake-mcp/src/lib.rs` (next to `reroute_wires`, near line 157 and the tool near line 1777)
- Modify: `src-tauri/crates/madake-mcp/src/link_api.rs` (next to `post_route_reroute`, near line 712; route list near line 1384)
- Create: `src-tauri/crates/madake-mcp/tests/arrange_api.rs` (copy the `fixture`, `call`, `post`, `first_sheet` helpers from `tests/reroute_api.rs`)
- Modify: `src-tauri/crates/madake-cli/src/client.rs`, `cli.rs`, `format.rs`
- Modify: `docs/10-automation-api.md`, `docs/10-automation-api.ja.md`, `docs/internal/feature-inventory.md`, `CLAUDE.md`

**Interfaces:**
- Consumes: `madake_core::arrange::{plan_arrange, ArrangeOptions, ArrangeContext, ArrangePlan}`, `madake_core::frame_template::for_project`, `FrameTemplate::frame_rect`, `FrameTemplate::title_block_origin`, `Sheet::paper_mm`, `madake_core::tidy::tidy_metrics`, the existing private `total_wire_length_mm` in `madake-mcp/src/lib.rs`.
- Produces:
  - `SharedDoc::arrange_sheet(&self, sheet_id: Uuid, options: &ArrangeOptions, origin: EditOrigin) -> Result<serde_json::Value, String>`
  - MCP tool `arrange_sheet { sheet_id?: Uuid, options?: ArrangeOptions }`
  - `POST /api/v1/route/arrange` with body `{ sheet_id?, options? }`
  - CLI `madake arrange [--sheet ID] [--channel MM]`; `Api::arrange(&self, sheet_id: Option<&str>, channel_mm: Option<f64>) -> Result<Value, CliError>` posting `{"sheet_id": .., "options": {"channel_mm": ..}}` (omit `options` when no `--channel`).

`SharedDoc::arrange_sheet` follows `reroute_wires` exactly: validate first; up to 3 tries; under the lock read revision, sheet clone, `protected` (mech_links), the frame template (`for_project`), frame rect and title block (`title_block_origin` gives its top-left; its bottom-right is the frame rect's bottom-right), and `taken_names` (every `NetLabel::name` on every sheet plus every `SymbolDef::net` name of placed symbols; read how `SymbolDef::net` is shaped first); plan outside the lock; reply

```json
{ "moved": n, "columns": c, "rows": r, "routed": [..], "labelled": [{"net": .., "label": ..}],
  "before": TidyMetrics, "after": TidyMetrics,
  "wire_length_mm": {"before": x, "after": y}, "size_mm": {"before": [w, h], "after": [w, h]} }
```

Commands in this order: `DeleteEntities { remove }` (if any), one `MoveEntities` per `moves` entry, one `UpdateEntity` per `updates` entry, one `AddEntity` per `add` entry; applied with `execute_batch` under the lock only if the revision did not change; empty plan executes nothing. Retry message as in reroute, worded for arrange: `"並べ直しの計算中に図面が何度も変わったため、並べ直しをやめました。もう一度実行してください"`.

MCP tool description (Japanese, one paragraph, the same style as `reroute_wires`) says: what it does (aligns parts into columns and rows, opens routing channels of `channel_mm`, shrinks gaps wider than `max_gap_mm`, keeps the order, redraws all wires); what moves together; that unroutable clusters are joined by net labels (named after their label, their net-name symbol, or `ARR_<n>`) and listed in `labelled`; what it refuses (cables, panel layouts, port contacts, wires pointed at, mixed length sources, labels with different names, not fitting the frame); that one undo restores; the reply fields.

The CLI `format::arrange` prints, in Japanese: units moved, columns and rows, routed net count, each labelled `net -> label`, the tidy metrics before/after line (same format as `format::reroute`), wire length and size before/after.

- [ ] **Step 1: Write the failing tests**

In `tests/arrange_api.rs`:

```rust
/// The arrange tool is advertised and its description says that one undo restores the drawing.
/// 並べ直しのツールが公開され、説明にundo一発で戻ることが書いてある。
#[test]
fn the_arrange_tool_is_advertised() { /* tool_descriptions() contains "arrange_sheet" whose text contains "undo" */ }

/// Arranging two squeezed parts over MCP moves them apart, reroutes the wire and one undo restores every entity.
/// MCPで詰まった2つの部品を並べ直すと離れて配線が引き直され、undo一発で全てのエンティティが元に戻る。
#[tokio::test]
async fn arranging_over_mcp_moves_reroutes_and_undoes_in_one_step() { /* place two blocks 3 mm apart (as reroute_api does with place_block / connect_pins), call arrange_sheet, check reply moved >= 1 and routed non-empty, snapshot entities before, call undo once, compare entities equal to snapshot */ }

/// The Link API arrange endpoint gives the same reply shape and refuses bad options with 400 and no change.
/// Link APIの並べ直しは同じ形の返り値を返し、使えない設定は400で断って何も変えない。
#[tokio::test]
async fn the_link_api_arranges_and_refuses_bad_options() { /* POST /api/v1/route/arrange with {} -> 200 and keys moved, columns, rows, routed, labelled, before, after, wire_length_mm, size_mm; POST with {"options":{"channel_mm":1}} -> 400 and revision unchanged */ }
```

In `madake-cli/src/cli.rs` tests (follow the existing reroute test with the recording fake `Api`):

```rust
    /// madake arrange sends the sheet and the channel width only when given.
    /// madake arrangeは、シートと通り道の幅を指定されたときだけ送る。
    #[test]
    fn arrange_sends_only_what_was_given() { /* run ["arrange"] -> recorded "arrange(None, None)"; run ["arrange","--sheet","S","--channel","10"] -> "arrange(Some(\"S\"), Some(10.0))" */ }
```

In `madake-cli/src/format.rs` tests (if the module has tests for `reroute`, mirror them):

```rust
    /// The arrange summary lists labelled clusters and the size before and after.
    /// 並べ直しの要約は、ラベルでつないだかたまりと前後の大きさを書く。
    #[test]
    fn arrange_summary_lists_labels_and_size() { /* format::arrange(&json!({...})) contains "ARR_1" and "→" */ }
```

- [ ] **Step 2: Run them to see them fail**

Run: `cd src-tauri && cargo test -p madake-mcp --test arrange_api 2>&1 | tail -5 && cargo test -p madake-cli arrange 2>&1 | tail -5`
Expected: compile errors.

- [ ] **Step 3: Implement** `SharedDoc::arrange_sheet`, the MCP tool with a `ArrangeSheetParams { sheet_id: Option<Uuid>, options: Option<ArrangeOptions> }` (derive like `RerouteWiresParams`), the Link API handler and route, the CLI subcommand (`Commands::Arrange { sheet: Option<String>, channel: Option<f64> }` with Japanese `///` help lines like `Reroute`), `Api::arrange` in both the real client and the test fake, and `format::arrange`.

- [ ] **Step 4: Documents**

- `docs/10-automation-api.md` and `.ja.md`: add `arrange_sheet` / `POST /api/v1/route/arrange` next to `reroute_wires`, same table/section style, same content in both languages.
- `docs/internal/feature-inventory.md`: add the arrange feature next to the reroute line.
- `CLAUDE.md`: add after the `reroute` line:
  `cargo run -p madake-cli -- arrange [--sheet <ID>] [--channel MM]  # 部品の並べ直し (並び順を保って整列し、通り道をあけて全配線を引き直す。undo一発)`

- [ ] **Step 5: Full verification**

Run:
```bash
export PATH="$HOME/.cargo/bin:$PATH" MADAKE_USER_FRAME_TEMPLATES_PATH=/Users/ktakeuchi/Documents/MadakeCAD/.superpowers/sdd/2026-11-15-stroke-font/empty-frames
(cd src-tauri && cargo test --workspace 2>&1 | grep -E "^test result|FAILED|panicked" | tail -40)
npx vitest run 2>&1 | tail -3; npx vue-tsc --noEmit
python3 -m unittest discover -s freecad-addon/tests 2>&1 | tail -2
python3 scripts/gen_spec.py --check; python3 scripts/gen_licenses.py --check; python3 scripts/audit_confidential.py
```
Expected: no failures; spec, licenses and audit checks pass.

- [ ] **Step 6: Commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/audit_confidential.py
git add -A src-tauri docs CLAUDE.md
git commit -m "feat: 部品の並べ直しをMCP・Link API・CLIから使えるようにする"
```
