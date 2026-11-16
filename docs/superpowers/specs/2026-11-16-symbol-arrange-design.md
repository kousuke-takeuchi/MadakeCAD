# Symbol arrange — design

Date: 2026-11-16. Status: draft for review.

## Problem

Wire reroute (`route.rs`, `reroute_wires`) keeps connectivity but cannot improve a sheet whose symbols
sit too close together: on dense drawings (traced or split from a large sheet) most wire clusters have no
free path, and the few that do get longer. The layout itself needs room and alignment before wires can be
drawn cleanly.

## Goal

A command that arranges the symbols of a sheet and then redraws all wires:

- the left-to-right and top-to-bottom **order** of everything on the sheet is kept (the arrangement the
  author chose stays recognisable);
- symbols that are nearly in line are put exactly in line (same column centre or same row centre);
- between columns and between rows there is a routing channel of at least `channel_mm`; empty bands
  wider than `max_gap_mm` are shrunk to that width;
- connectivity is unchanged, and one undo restores the previous drawing.

Out of scope: changing the order (left-to-right signal flow by graph layering), rotating symbols, a canvas
button (needs a design in `MadakeCAD.pen` first).

## User-facing behaviour

### Entry points

- MCP tool `arrange_sheet { sheet_id?, options? }`.
- Link API `POST /api/v1/route/arrange` with the same body.
- CLI `madake arrange [--sheet ID] [--channel MM]`.

All plan first (pure function in madake-core) and apply one batch of existing commands
(`move_entities` / `update_entity` / `delete_entities` / `add_entity`). No new `Command`, no format change.

### Options

`channel_mm` (default 8): width kept free between columns and between rows. `max_gap_mm` (default 20):
wider empty bands are shrunk to this. `align_mm` (default 4): symbols whose centres differ by less than
this in x (or y) inside one column (row) are aligned. The reroute options (`grid_mm`, costs, clearance) are
passed through. All options are validated before the engine lock is taken.

### What moves together (units)

- A **unit** is one symbol plus what belongs to it:
  - satellite symbols: net-name marks (ground, power) and other one-pin symbols whose only wire goes to a
    pin of this symbol;
  - net labels whose only wire goes to a pin of this symbol;
  - free texts and leader lines whose anchor lies within 3 mm of the symbol's bounds (and not closer to
    another symbol).
- A unit moves rigidly (translation only).

### How positions are computed

1. Bounds of every unit (symbol bounds united with its satellites and texts).
2. **Columns**: sort units by left edge and group units whose x-ranges overlap into one column band.
   Inside a band, cluster unit centres closer than `align_mm` and move each cluster to its median x.
3. **Rows**: the same on y.
4. New band positions: bands keep their order; the gap between consecutive bands becomes
   `clamp(old_gap, channel_mm, max_gap_mm)`. The first band keeps its left (top) edge.
5. A piecewise-linear, monotone coordinate map `fx` (and `fy`) is built from old band edges to new band
   edges. Units move by the map of their aligned anchor. Everything that is not part of a unit
   (enclosure shapes, free texts far from symbols, sheet notes) has each point mapped through `fx`/`fy`,
   so outlines stretch with their contents and nothing changes order.
6. If the result does not fit inside the frame, the gaps are reduced towards `channel_mm` first and then
   proportionally; if it still does not fit, the command refuses with a message (nothing changes).

### Wires

- Before moving, the wire clusters of the sheet are recorded with their terminals (pins by symbol id and
  pin number, net labels by id, net-name symbol pins) and their wire attributes, exactly as reroute does.
- After moving, every cluster is routed on the new geometry with the reroute engine.
- A cluster that cannot be routed is **connected by labels** instead of being left broken: each terminal
  gets a short lead and a net label with the cluster's net name, so connectivity is kept. When that name
  is not unique to the cluster across the project (for example automatic N001 names or names shared by
  other clusters), a fresh name `ARR_<n>` is used. The reply lists these clusters.
- Clusters that reroute would leave unchanged for a reason other than "no route" (cable cores, mixed
  attributes, wires referenced by other entities, free ends) make the whole arrange refuse, because their
  wires cannot follow the moved symbols; the reply names them.

### Reply

`{ moved: n_units, columns, rows, routed: [net], labelled: [{net, label}], before: TidyMetrics,
after: TidyMetrics, wire_length_mm: {before, after}, size_mm: {before:[w,h], after:[w,h]} }`.

## Code shape

- `src-tauri/crates/madake-core/src/arrange.rs`: units, bands, maps, fit, `plan_arrange(sheet, defs,
  &ArrangeOptions, protected) -> Result<ArrangePlan, String>`.
- `route.rs` gains a function that routes recorded clusters on a given sheet
  (`plan_route_clusters`), shared by reroute and arrange, and the label fallback.
- MCP, Link API and CLI follow the reroute entry points (plan outside the lock where reroute does,
  options validated first, one batch, one undo).

## Verification

- Unit tests in `arrange.rs` (English + Japanese spec lines): order of units kept in x and y; nearly
  aligned symbols become aligned; narrow gaps widen to `channel_mm`; wide gaps shrink to `max_gap_mm`;
  satellites, labels and nearby texts move with their symbol; enclosure shapes stretch with their
  contents; result outside the frame refuses with nothing changed; cluster that cannot be routed is
  connected by labels with a unique name; netlist before/after has the same pin sets (generated sheets
  with off-grid pins, marks, labels); one undo restores everything (MCP/Link API tests).
- Locally (private drawings, not committed): run arrange on the nine split sheets; compare crossings,
  label overlaps, wire length, labelled clusters and the images.

## Documents

`docs/10-automation-api(.ja).md`, `docs/internal/feature-inventory.md`, `CLAUDE.md` CLI line, regenerated
`docs/13-specification*.md`.
