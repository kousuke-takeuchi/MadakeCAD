# Wire reroute — implementation plan

> For agentic workers: use superpowers:subagent-driven-development or superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** a planner in madake-core that redraws the wires of a sheet without changing connectivity, and
three entry points (MCP tool, Link API, CLI) that apply its result as one undoable batch.

**Spec:** `docs/superpowers/specs/2026-11-16-wire-reroute-design.md` (read it first; it is binding).

**Architecture:** new module `src-tauri/crates/madake-core/src/route.rs` (pure functions, no model mutation).
Entry points call `plan_reroute(sheet, defs, &RerouteOptions) -> ReroutePlan` and execute
`Command::DeleteEntities` + `Command::AddEntity` in one `engine.execute_batch` (pattern:
`connect_cable` in `src-tauri/crates/madake-mcp/src/lib.rs`).

## Global constraints

- Every Rust test has two `///` lines directly before `#[test]`: English sentence, then Japanese sentence,
  describing behaviour for a beginner user. One behaviour per test. Run `python3 scripts/gen_spec.py` after
  test changes and commit `docs/13-specification*.md`.
- Invented values only in tests and docs. Never describe any private reference drawing.
- No `Co-Authored-By` or other attribution trailer in commits. Commit messages in Japanese with a
  conventional prefix (`feat:`, `docs:`, `test:`).
- Before each commit: `python3 scripts/audit_confidential.py` must print `no confidential terms found`.
- No file format change, no new `Command` variant, no UI.
- Do not guess APIs: read `netlist.rs` (`extract_netlist`, `Net`, `NetPin`, `pin_positions`,
  `transform_local`, `CONNECT_EPS`), `tidy.rs` (`symbol_bounds`, `text_boxes`, `TidyMetrics`,
  `tidy_metrics`), `pin_wiring.rs` (`part_pins`, `world_dir`), `model.rs` (`Wire`, `Junction`, `Cable`,
  `NetLabel`), `symbol.rs` (`SymbolDef::net_name`, `DefIndex`).
- Focused tests while working: `cd src-tauri && PATH="$HOME/.cargo/bin:$PATH" cargo test -p madake-core --lib route::`.

## Review focus

1. A route that touches another net's pin at an interior point (the netlist joins pins anywhere on a
   wire) — must never happen; test it.
2. Two nets whose new wires share an endpoint (the netlist joins wire endpoints) — must never happen.
3. Off-grid pins (`free_pins` symbols, rotated symbols) — the lead must start exactly at the pin.
4. A net whose terminals include a net label or a net-name mark — the label/mark position is a terminal.
5. Undo after the batch restores exactly the old entities (ids included).

## Task 1: routing grid and single connection (`route.rs`)

- [ ] Types: `RouteOptions { grid_mm: f64 (default 1.25), bend_cost, cross_cost, near_cost, clearance_mm }`
  with the defaults from the spec; `RouteGrid` built from the sheet: paper size, blocked cells from
  `symbol_bounds` (inflated by `clearance_mm` = 0.6) and `text_boxes`, frame margin.
- [ ] A* on the grid between two points with direction state (cost of bends), forbidden cells and
  forbidden edges; returns the cell path or `None`.
- [ ] Pin leads: from the exact pin position, straight along the pin's world direction to the first free
  grid line; the lead cells are opened in the blocked map for that pin only.
- Tests: straight connection; L connection; detour around a symbol box; detour around a text box; no path
  when fully enclosed returns `None`; off-grid pin lead starts at the pin.
- Commit `feat: 配線の自動の引き直しの経路探索を追加 (格子と A*)`.

## Task 2: net trees, other nets, plan (`route.rs`)

- [ ] `plan_reroute(sheet, defs, opts) -> ReroutePlan { remove: Vec<EntityId>, add: Vec<Entity>,
  rerouted: Vec<String>, unchanged: Vec<(String, String)> }`.
- [ ] Net selection and terminals per the spec (pins, net labels, net-name symbol pins); skipped nets with
  reasons (cable cores, mixed attributes, `nets` filter).
- [ ] Occupancy: cells and edges of routed nets; other nets' pins and junctions forbidden; same-direction
  edge sharing forbidden; perpendicular crossing allowed with `cross_cost`; `near_cost` next to another net.
- [ ] Tree growth with T-joins and junctions; a point with three directions cannot take a fourth.
- [ ] Order and rip-up-and-retry (3 rounds); unroutable nets keep their old wires.
- [ ] Emit merged polylines, junctions, copied wire attributes (spec "Result shape").
- Tests: two-pin net replaced by one orthogonal wire; three-terminal net gets one T and a junction; four
  terminals around one point never make a four-way point; route never passes over another net's pin;
  two nets never share a wire endpoint; perpendicular crossing when unavoidable; mixed colours left
  unchanged with reason; cable-core net left unchanged; unroutable keeps old wires; deterministic output;
  netlist before/after has the same pin sets (several hand-made sheets including a net label and a
  net-name mark).
- Commit `feat: ネットごとに配線を引き直す計画を作る (分岐・他ネット回避・引き直しの再試行)`.

## Task 3: entry points and documents

- [ ] MCP tool `reroute_wires` (params `sheet_id?`, `nets?`, `options?`), plan and batch inside one engine
  lock; reply JSON per the spec (`rerouted`, `unchanged`, tidy metrics before/after, wire length
  before/after). Test in `src-tauri/crates/madake-mcp/tests/` following existing tool tests: one batch,
  one undo restores the original entities.
- [ ] Link API `POST /api/v1/route/reroute` (same body and reply) in `link_api.rs`; test like
  `stroke_fonts_api.rs`.
- [ ] CLI `madake reroute [--sheet ID] [--net NAME]...` in `src-tauri/crates/madake-cli/src/cli.rs`
  (thin client) with its existing test style if any.
- [ ] Docs: `docs/10-automation-api.md` + `.ja.md`, `README.md` + `README.ja.md` CLI list,
  `docs/internal/feature-inventory.md` paragraph, regenerate spec.
- Commit `feat: 配線の引き直しを MCP・Link API・CLI から使えるようにする` and `docs: 配線の引き直しを文書へ反映`.

## Task 4 (main session): review and local trial

- One opus review of the branch; fix findings; full verification (cargo workspace, Vitest, vue-tsc,
  freecad-addon unittest, gen_spec --check, gen_licenses --check, audit).
- Try on the private split drawings (not committed) and report metrics.
