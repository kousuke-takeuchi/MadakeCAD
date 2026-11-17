# Function Sheet Draft Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A command that drafts one A3 function sheet from a read-only source project (the caller picks units and columns; MadakeCAD draws boxes, pin order, wires, tags, power and ground marks), and a command that checks all drafted sheets against the source netlist.

**Architecture:** A new pure module `madake-core/src/draft.rs` builds a complete `Sheet` in memory in three stages (model, layout, wires) and never touches the document. `SharedDoc::draft_function_sheet` loads the source file, calls the planner and applies the sheet as one edit. Two small prerequisite changes (contact pole as pin key, block views) come first because the check and the multi-sheet netlist depend on them.

**Tech Stack:** Rust (madake-core, madake-mcp with rmcp, madake-cli), TypeScript (canvas, only where the look changes), Python scripts for spec generation and audits.

**Spec:** `docs/superpowers/specs/2026-11-18-function-sheet-draft-design.md`. Read it before any task; it holds the rules and the exact JSON shapes. This plan does not repeat them.

## Global Constraints

- Every document edit goes through a `Command`. The draft is one edit, one undo.
- The planner is pure: `draft.rs` takes `&Project` values and returns a plan. No engine, no file access.
- Rust tests: two `///` lines (English, then Japanese) right before `#[test]`. Vitest: `// ja:` before `it(`. After adding or changing tests run `python3 scripts/gen_spec.py` and commit `docs/13-specification*.md`.
- `FORMAT_VERSION` is 29. This plan adds only string attributes and no struct field: the format version does not change. If a task finds it must add a field, it bumps the version once with a migration and a round-trip test.
- The canvas (TypeScript) draws the same as SVG / PDF.
- Docs in general terms with invented names only (`U7`, `J7.2`, `P24X`). Nothing read or measured from `docs/references` goes into code, tests, docs or commit messages. Run `python3 scripts/audit_confidential.py` before every commit; it must print `no confidential terms found`.
- `docs/superpowers` is English only. Other docs keep the English + `.ja.md` pair.
- Commit messages in Japanese with a conventional prefix. No `Co-Authored-By` or other attribution trailer.
- No UI button. Do not touch port 9310; tests that start a server use `MADAKE_MCP_PORT`.
- Determinism: no iteration over `HashMap` / `HashSet` may decide an output. Use `BTreeMap`, `BTreeSet` or sorted vectors. Entity ids are fresh UUIDs; everything else in the plan is a function of the input.
- Full verification before the branch is done:
  `export PATH="$HOME/.cargo/bin:$PATH" MADAKE_USER_FRAME_TEMPLATES_PATH=<main checkout>/.superpowers/sdd/2026-11-15-stroke-font/empty-frames; (cd src-tauri && cargo test --workspace); npx vitest run; npx vue-tsc --noEmit; python3 -m unittest discover -s freecad-addon/tests; python3 scripts/gen_spec.py --check; python3 scripts/gen_licenses.py --check; python3 scripts/audit_confidential.py`

## Review Focus

1. Two sheets that show the same unlabelled net compute the same tag name, whichever is drafted first (Task 3 test `unlabelled_net_name_is_the_same_on_every_sheet`).
2. A net with pins on three sheets: a tag on every sheet, and the check reports one net (Task 6 test `a_net_over_three_sheets_is_one_net`).
3. A power net whose name is also an ordinary tag elsewhere: marks and tags are one net (Task 6 test `power_marks_and_tags_of_one_name_are_one_net`).
4. A unit with only power and ground pins is still drawn, with marks only (Task 4 test `a_unit_with_only_power_pins_is_drawn_with_marks`).
5. Drafting twice is refused, and one undo restores the project byte for byte (Task 7 test `undo_after_a_draft_restores_the_project_bytes`).

## File Structure

| File | Responsibility |
|---|---|
| `src-tauri/crates/madake-core/src/draft.rs` (new) | Request types, validation, source model, layout, wires, plan |
| `src-tauri/crates/madake-core/src/draft_check.rs` (new) | Source against open project comparison |
| `src-tauri/crates/madake-core/src/lib.rs` | `pub mod draft; pub mod draft_check;` |
| `src-tauri/crates/madake-core/src/netlist.rs`, `symbol.rs`, `pin_wiring.rs`, `reports.rs`, `kicad.rs` | Pin key of a `through` symbol (Task 1) |
| `src-tauri/crates/madake-core/src/block.rs`, `verify.rs`, `reports.rs`, `xref.rs` | Block views (Task 2) |
| `src-tauri/crates/madake-core/tests/fixtures/draft-source.mdkproj` (new) | Invented source used by tests in Tasks 3 to 7 |
| `src-tauri/crates/madake-core/tests/golden/draft-sheet.svg` (new) | Look of a drafted sheet |
| `src-tauri/crates/madake-mcp/src/lib.rs`, `link_api.rs` | `SharedDoc::draft_function_sheet`, `check_function_sheets`, MCP tools, routes |
| `src-tauri/crates/madake-cli/src/main.rs` | `madake draft`, `madake draft-check` |
| `docs/10-automation-api*.md`, `docs/internal/data-model.md`, `docs/internal/feature-inventory.md`, `README*.md`, gap ledger | Docs (with the task that adds the behaviour) |

`draft.rs` will be large. Keep three sections in this order, each with its own tests module section: `// --- model`, `// --- layout`, `// --- wires`. If it passes about 2500 lines, move layout to `draft_layout.rs`.

---

### Task 1: A connector contact is identified by its pole

**Files:**
- Modify: where a symbol's pin keys are produced for the netlist, `get_pins`, `connect_pins`, the wire list and KiCad export (start at `netlist.rs` test `a_connector_contact_passes_the_wire_through_and_is_listed_once`, `symbol.rs` near `contact.through = true`, `pin_wiring.rs`, `reports.rs`, `kicad.rs` test `a_connector_contact_round_trips_through_kicad`)
- Modify: `verify.rs` (new code `erc.contact_pole_repeated`)
- Docs: `docs/10-automation-api*.md`, `docs/internal/data-model.md`, gap ledger

**Interfaces:**
- Produces: for a symbol whose definition has `through == true`, the pin key is `instance.value.trim()` when not blank, else the definition's pin number (today's `1`). One helper decides it; every consumer calls the helper. Name it `symbol::through_pin_key(inst: &SymbolInstance, def: &SymbolDef, pin_no: &str) -> String` unless an existing helper fits better (then extend that one and say so in the report).

- [ ] **Step 1: Read** how `NetPin.pin` is filled for a `through` symbol and list every place that prints or parses that key. Write the list into the task report.
- [ ] **Step 2: Failing tests** (one behaviour each, with the two doc lines):
  - `two_contacts_of_one_connector_are_listed_by_their_poles`: `J7` value `2` and `J7` value `5` on two nets give `J7:2` and `J7:5`.
  - `a_contact_without_a_pole_keeps_the_key_one`.
  - `connect_pins_reaches_a_contact_by_its_pole`: `"J7.5"` resolves to the second contact.
  - `the_same_pole_on_two_nets_is_reported`: `erc.contact_pole_repeated`, error, both entity ids.
  - `the_same_pole_twice_on_one_net_is_allowed`.
  - `the_wire_list_names_a_contact_by_its_pole`.
  - KiCad round trip keeps the pole (extend the existing test).
- [ ] **Step 3: Run** `cargo test -p madake-core contact` and see them fail.
- [ ] **Step 4: Implement** the helper and route every consumer through it.
- [ ] **Step 5: Run** `cargo test --workspace`, regenerate the spec, update docs and the gap ledger (close the "contact pin key" entry), audit, commit: `fix: 配線上の接点のピンを極の番号で区別する`.

### Task 2: Views (one unit on several sheets)

**Files:**
- Modify: `block.rs` (`pub const BLOCK_VIEW_ATTR: &str = "block_view";`, `pub fn is_view(inst: &SymbolInstance) -> bool` true when the symbol is a block and the attribute is `"1"`)
- Modify: `verify.rs` (duplicate reference rule inside a sheet near the `by_ref` map; new project-level pass in `verify_project`), `reports.rs` (`bom_rows_with` grouping), `xref.rs` (`sheet_device_xrefs` text)
- Docs: `docs/internal/data-model.md`, `docs/10-automation-api*.md`

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces: `block::is_view`, `block::BLOCK_VIEW_ATTR`; diagnostics `erc.view_pin_repeated` (error), `erc.view_part_mismatch` (warning), both evaluated over the whole project in `verify_project`.

- [ ] **Step 1: Failing tests:**
  - `views_of_one_reference_on_one_sheet_are_not_duplicates`.
  - `a_view_and_a_plain_symbol_of_one_reference_are_duplicates`.
  - `views_on_two_sheets_count_once_in_the_bom` (quantity 1, reference listed once).
  - `the_bom_takes_the_part_number_from_the_first_view_that_has_one`.
  - `views_that_disagree_on_the_part_number_are_reported`.
  - `a_pin_shown_by_two_views_is_reported` (same sheet and two sheets: two tests).
  - `views_show_no_location_text` (SVG has no `/2.B3` style text for the reference).
  - `the_project_netlist_joins_the_pins_of_all_views_under_one_reference`.
- [ ] **Step 2: Run** and see them fail.
- [ ] **Step 3: Implement.** In the per-sheet duplicate rule add a kind `VIEW_KIND` in the same way as `THROUGH_KIND`. Add `view_diagnostics(project)` and call it from `verify_project`.
- [ ] **Step 4: Check the canvas.** If the TypeScript side draws the device location text itself, give it the same rule and a Vitest twin.
- [ ] **Step 5:** workspace tests, spec regeneration, docs, audit, commit: `feat: 1つの機器を複数の図に分けて描く部分表示のブロックを足す`.

### Task 3: Draft model (request, validation, source analysis)

**Files:**
- Create: `draft.rs` (model section), `tests/fixtures/draft-source.mdkproj`
- Modify: `lib.rs`

**Interfaces:**
- Consumes: `netlist::extract_netlist`, `xref::extract_netlist_project`, `symbol::sheet_symbol_defs`, `block::BlockPins`, Task 1 pin keys.
- Produces:

```rust
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DraftRequest {
    pub name: String,
    pub columns: Vec<Vec<String>>,
    #[serde(default)] pub pins: BTreeMap<String, Vec<String>>,
    #[serde(default)] pub power_nets: Vec<String>,
    #[serde(default)] pub ground_nets: Option<Vec<String>>,
    #[serde(default)] pub destinations: BTreeMap<String, String>,
    #[serde(default)] pub paper: DraftPaper,      // A3 (default) | A4
    #[serde(default = "default_pitch")] pub pitch_mm: f64,   // 2.5 | 5.0
}

pub(crate) enum NetClass { Power, Ground, Internal, Leaving }

pub(crate) struct DraftPin { pub key: String, pub name: String, pub net: usize }
pub(crate) struct DraftUnit {
    pub reference: String, pub column: usize, pub row: usize,
    pub source: SymbolInstance,           // first instance of the reference
    pub is_block: bool, pub pins: Vec<DraftPin>,
}
pub(crate) struct DraftNet {
    pub name: String,                     // tag / label / mark name
    pub class: NetClass,
    pub ground_symbol: Option<String>,    // symbol id of the source's mark
    pub dest: Option<String>,
}
pub(crate) struct DraftModel {
    pub units: Vec<DraftUnit>, pub nets: Vec<DraftNet>,
    pub skipped: Vec<(String, String, String)>,   // reference, pin, reason
}
pub(crate) fn build_model(source: &Project, open: &Project, req: &DraftRequest) -> Result<DraftModel, String>
```

The source netlist is the project netlist of the source (all sheets). `source_path` is not part of `DraftRequest`: the caller in madake-mcp reads the file.

- [ ] **Step 1: Build the fixture** with a throwaway test helper or by hand as JSON: one sheet with invented blocks `U1` (pins 1 to 6 with names), `U2`, `TB1`, `M1`, a resistor `R1`, a ground mark on net `GND`, a labelled net `P24X`, an unlabelled net between `U1.3` and `M1.1`, a net with three pins over `U1`, `U2`, `TB1`, one pin of `U1` on a one-pin net. Load it in a test first (`the_draft_fixture_loads_and_verifies_clean`).
- [ ] **Step 2: Failing tests:**
  - `a_view_carries_only_the_pins_in_use` and `a_pin_on_a_one_pin_net_is_skipped_with_a_reason`.
  - `the_pins_option_limits_a_view`.
  - `a_net_with_all_pins_on_the_sheet_is_internal`, `a_net_with_pins_elsewhere_is_leaving`, `listed_power_and_ground_nets_get_their_class`.
  - `ground_nets_default_to_nets_with_a_mark`.
  - `a_labelled_net_keeps_its_label_name`.
  - `unlabelled_net_name_is_the_same_on_every_sheet` (two requests with different units give the same `REF.PIN`, natural order: `U2.10` after `U2.9`).
  - `a_library_symbol_stays_a_symbol` (`is_block == false`).
  - One test per refusal of the spec section "Refusals" that this stage can see: unknown reference, reference twice, unknown pin key, unknown net name in each of the three options, a net in both lists, sheet name in use, blank name, zero or more than eight columns, pin already shown by a view in the open project, library symbol already on a function sheet, no carried pin, port with contacts, net through a cable core, `pitch_mm` other than 2.5 or 5.0.
- [ ] **Step 3: Run**, fail, implement `build_model`, pass.
- [ ] **Step 4:** spec regeneration, audit, commit: `feat: 機能シートの下書きの入力の検査と、元の図の読み取りを足す`.

### Task 4: Draft layout (boxes, sides, order, marks, tags, placement)

**Files:**
- Modify: `draft.rs` (layout section)

**Interfaces:**
- Consumes: `DraftModel`; `frame_template::for_project(open)` and `arrange::frame_areas(&template, &sheet)` for the drawing area; `block.rs` attribute names and size rules; `net_tag::net_tag_with` for the tag extent; the library ids `power_rail` and the ground mark.
- Produces:

```rust
pub(crate) struct Terminal { pub net: usize, pub at: Point, pub dir: Dir }
pub(crate) struct DraftLayout {
    pub sheet: Sheet,               // symbols, marks, tag labels, stubs; no internal wires yet
    pub terminals: Vec<Terminal>,   // one per carried pin of an Internal or Leaving net with 2+ pins here
    pub tagged: Vec<(String, usize)>, pub power_marks: usize, pub ground_marks: usize,
}
pub(crate) fn lay_out(model: &DraftModel, open: &Project, req: &DraftRequest) -> Result<DraftLayout, String>
```

The sheet is created with `xref_style = XrefStyle::SheetName`, landscape, the requested paper, and the open project's defaults for everything else. Blocks get `block_view = "1"`, `block_pins` JSON built from the ordered pins (with `{"gap": 1}` rows between the power, signal and ground groups), `block_pitch` from `pitch_mm`, `block_pin_text` at least 2.2.

- [ ] **Step 1: Failing tests:**
  - Sides: `a_pin_with_peers_on_the_left_goes_left`, `..._on_the_right_goes_right`, `a_tie_goes_left`, `a_pin_with_no_peer_goes_right_only_in_the_last_column`.
  - Groups: `power_pins_come_first_and_ground_pins_last_on_the_left_side` (with the spacer rows).
  - Order: `straight_connections_do_not_cross` (two units, three nets wired 1-1, 2-2, 3-3 after ordering although the source order is reversed), `the_same_request_gives_the_same_layout` (run twice, compare everything but ids).
  - Marks: `each_power_pin_gets_its_own_rail_mark_named_after_the_net`, `ground_uses_the_marks_symbol_of_the_source`, `ground_falls_back_to_the_library_mark`.
  - `a_unit_with_only_power_pins_is_drawn_with_marks`.
  - Tags: `a_leaving_net_gets_one_tag_at_the_first_pin_in_column_order`, `a_tag_on_a_right_side_pin_points_outward` (`tag_flow = out`), `a_destination_option_becomes_the_tags_destination`, `tags_on_adjacent_pins_do_not_touch` (extents from `net_tag_with` do not overlap at 5 mm and at 2.5 mm pitch).
  - Placement: `every_pin_and_origin_is_on_the_grid`, `columns_are_spread_over_the_drawing_area`, `nothing_enters_the_title_block`.
  - Refusals: `a_column_taller_than_the_area_is_refused_with_the_excess`, `columns_that_leave_no_channel_are_refused`.
- [ ] **Step 2: Run**, fail, implement, pass. Pin positions come from `route::pin_points(&sheet, &defs)` on the built sheet, never from hand arithmetic.
- [ ] **Step 3:** spec regeneration, audit, commit: `feat: 機能シートの下書きの配置とピンの並びと札と電源の印を足す`.

### Task 5: Draft wires and the plan

**Files:**
- Modify: `draft.rs` (wires section), `route.rs` only if a `pub(crate)` item needs a small addition
- Create: `tests/golden/draft-sheet.svg`

**Interfaces:**
- Consumes: `DraftLayout`; `route::{RouteGrid, RouteOptions, RouteUnit, route_units, emit, make_lead}` (all `pub(crate)`, same crate).
- Produces:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct DraftPlan {
    #[serde(skip)] pub sheet: Sheet,
    pub units: usize, pub pins: usize,
    pub wired: Vec<String>, pub tagged: Vec<DraftTagged>,       // { net, pins }
    pub marked: DraftMarked,                                     // { power, ground }
    pub labelled: Vec<String>, pub skipped_pins: Vec<DraftSkipped>,  // { reference, pin, reason }
}
pub fn plan_function_sheet(source: &Project, open: &Project, req: &DraftRequest) -> Result<DraftPlan, String>
```

Approach: build one `RouteUnit` per net with two or more terminals (`wires` and `junctions` empty, `template` a default `Wire`, `terms` from `DraftLayout.terminals`, leads from `make_lead`), call `route_units` on `RouteGrid::build(&layout.sheet, &defs, &opts)` with `opts.title_block_mm` and the frame margin taken from the drawing area, and add the entities from `emit` to the sheet. A unit that returns `Err` gets a net label with the net name at each terminal on a short stub and goes into `labelled`. A Leaving net's tag stub is a fixed line for the router.

- [ ] **Step 1: Failing tests:**
  - `an_internal_net_is_wired_and_the_sheet_netlist_matches_the_source` (compare pin sets per net with the fixture).
  - `a_three_pin_net_gets_a_junction`.
  - `no_wire_is_dangling_and_no_pin_in_use_is_unconnected` (`verify_sheet` has no `erc.dangling_wire`, no unconnected carried pin).
  - `wires_cross_only_at_right_angles_and_stay_off_boxes` (use `tidy::tidy_metrics`: overlaps 0, wires through symbols 0).
  - `a_net_the_router_cannot_place_is_joined_by_labels` (force it with a tiny `paper`/dense request or by a test hook that blocks the channel).
  - `the_plan_counts_match_the_sheet`.
  - `a_drafted_sheet_renders_like_the_golden_file` (SVG of the fixture request with ids normalised; write the golden once, then look at it rendered to PNG and attach the image path in the report).
- [ ] **Step 2: Run**, fail, implement, pass.
- [ ] **Step 3: Vitest twin** only if the canvas needs new drawing code (expected: none, the sheet uses existing entities). State the result in the report.
- [ ] **Step 4:** spec regeneration, audit, commit: `feat: 機能シートの下書きの配線を引き、計画を返す`.

### Task 6: Check command (core)

**Files:**
- Create: `draft_check.rs`
- Modify: `lib.rs`

**Interfaces:**
- Consumes: `xref::extract_netlist_project`, `symbol::sheet_symbol_defs`, Task 1 keys, Task 2 views, `plan_function_sheet` in tests.
- Produces:

```rust
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct DraftCheck {
    pub ok: bool, pub source_nets: usize, pub matched: usize,
    pub missing_pins: Vec<CheckPin>,     // { reference, pin, source_net }
    pub extra_pins: Vec<CheckPin>,       // source_net empty
    pub split: Vec<CheckSplit>,          // { source_net, parts: Vec<Vec<String>> } with "REF:PIN"
    pub merged: Vec<CheckMerged>,        // { source_nets, pins }
}
pub fn check_function_sheets(source: &Project, open: &Project, ignore_references: &[String]) -> DraftCheck
```

Pins of net mark symbols and `through` symbols are left out on both sides; nets with fewer than two remaining pins are left out of the source side. All lists are sorted (natural order of reference, then pin).

- [ ] **Step 1: Failing tests:** `a_full_set_of_drafted_sheets_passes` (two requests that cover the fixture, one unit divided by `pins`), `a_pin_on_no_sheet_is_missing`, `a_pin_the_source_does_not_have_is_extra`, `a_tag_renamed_on_one_sheet_splits_the_net`, `two_source_nets_joined_by_a_wire_are_merged`, `ignored_references_are_left_out`, `a_net_over_three_sheets_is_one_net`, `power_marks_and_tags_of_one_name_are_one_net`, `the_check_changes_nothing` (open project equal before and after).
- [ ] **Step 2: Run**, fail, implement, pass.
- [ ] **Step 3:** spec regeneration, audit, commit: `feat: 下書きした図と元の図のつながりを比べる検算を足す`.

### Task 7: Entry points and docs

**Files:**
- Modify: `madake-mcp/src/lib.rs` (`SharedDoc::draft_function_sheet`, `SharedDoc::check_function_sheets`, tools `draft_function_sheet`, `check_function_sheets`), `madake-mcp/src/link_api.rs` (`POST /api/v1/draft/function-sheet`, `POST /api/v1/draft/check`), `madake-cli/src/main.rs` (`draft <request.json>`, `draft-check <source.mdkproj> [--ignore REF]...`)
- Docs: `docs/10-automation-api*.md`, `docs/internal/feature-inventory.md`, `docs/internal/data-model.md`, `README*.md`, `CLAUDE.md` command list, gap ledger, MCP tool descriptions (Japanese, like the neighbours)

**Interfaces:**
- Consumes: `draft::plan_function_sheet`, `draft_check::check_function_sheets`, `io::load_project`.
- Produces: the request body is `DraftRequest` plus `source_path: String`. Reply: the serialised `DraftPlan` plus `sheet_id` and `metrics` (`tidy::tidy_metrics` of the new sheet).

Apply pattern: copy `arrange_sheet_with` (plan outside the engine lock on a clone of the project, compare the revision, retry up to three times, then refuse). The edit is one `Command::RestoreSheet { index: project.sheets.len(), sheet }`, whose inverse removes the sheet. First confirm in `command.rs` that `RestoreSheet` accepts a sheet that was never in the project and broadcasts `SheetAdded`; if it does not, add a command `AddSheetWithEntities { sheet: Box<Sheet> }` with `RemoveSheet` as its inverse, and document it. Record which one was used in the report.

- [ ] **Step 1: Failing tests:**
  - `a_draft_adds_one_sheet_and_one_undo_removes_it`.
  - `undo_after_a_draft_restores_the_project_bytes` (serialise before the draft and after the undo).
  - `drafting_the_same_request_twice_is_refused`.
  - `a_missing_source_file_is_refused_and_nothing_changes`.
  - `the_source_file_is_not_modified` (bytes and modification time).
  - Link API round trip for both routes; MCP tool round trip for both tools; CLI argument parsing tests in the style of `reroute` / `arrange`.
- [ ] **Step 2: Run**, fail, implement, pass.
- [ ] **Step 3: Docs.** One section in the automation API doc with an invented two-sheet example and the check result; feature inventory entry; gap ledger entry.
- [ ] **Step 4: Live check on the dev build (port 9320, never 9310):** save the fixture to a scratch path, `POST /api/v1/draft/function-sheet` twice with two requests, `POST /api/v1/draft/check`, export both sheets to PNG, look at them, and put the image paths and the check output in the report.
- [ ] **Step 5:** full verification command from Global Constraints, spec regeneration, audit, commit: `feat: 機能シートの下書きと検算を MCP・Link API・CLI から使えるようにする`.

---

## After the tasks

- One opus review of the whole branch against the spec and the Review Focus list; fixes by opus.
- Then (outside this plan, in the git-ignored reproduction folder): draft the machine-wiring sheets from the real source and compare with the hand-drawn sheets. Findings go to the git-ignored gap notes, in general terms to the ledger.
