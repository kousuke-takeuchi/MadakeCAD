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
  - labels and one-pin symbols placed directly on a pin of this symbol (no wire needed).
  "Only wire goes to a pin of this symbol" is decided on the physically connected wire cluster: every pin
  on it other than the candidate's own belongs to this symbol. Same-named labels or marks elsewhere on the
  sheet do not matter.
  - free texts and leader lines whose anchor lies within 3 mm of the symbol's bounds (and not closer to
    another symbol).
- A unit moves rigidly (translation only). Every move is a multiple of 2.5 mm so pins stay on the grid.
- Alignment uses the main symbol's origin (`at`), not its outline, so equal parts line up pin to pin.
  An alignment move that would make two units overlap where they did not before is skipped.
- Labels and one-pin symbols that are not satellites of one symbol are units of their own.

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
6. The result must stay inside the frame and out of the title block (when the original content was out
   of it). If it does not, the gaps above `channel_mm` are reduced to `channel_mm` and the layout is
   tried again; if it still does not fit, the command refuses with a message (nothing changes).

### Wires

- Before moving, the wire clusters of the sheet are recorded with their terminals (pins by symbol id and
  pin number, net labels by id, net-name symbol pins) and their wire attributes, exactly as reroute does.
- After moving, every cluster is routed on the new geometry with the reroute engine, except clusters whose
  terminals all keep their position (and whose wire points the maps leave in place): they keep their old
  wires untouched, as fixed obstacles.
- A cluster whose labels and net-name symbols carry more than one name is refused before routing, whether
  it would be routed or kept.
- Routing runs in rounds. Clusters the router cannot route are handled one by one: first the old wire
  shape is **stretched** (below); if that is not possible the cluster is **connected by labels**. The
  stretched wires and label stubs are then placed as fixed lines, and in the next round only the clusters
  whose route touches something placed in the round before (a line end or bend on it, it ending on a new
  line, or a new label or junction within 1 mm; a plain crossing does not count) are routed again, around
  everything placed so far and the routes kept; the others keep their route. This repeats until a round
  has no new failure. Every round that goes on fails at least one more cluster, so there are at most the
  number of clusters + 1 rounds, and the routes used always keep clear of every stub and stretched wire.
- **Stretching**: every point of the old wires is mapped through the column and row warps, each end that
  sat on a terminal moves to the terminal's new position, the neighbouring point moves with it so the
  wire keeps its right angles, and a two-point wire that would turn diagonal gets a jog halfway. An end
  that sat on a bend or end of another wire of the cluster (not on a terminal) follows that point the same
  way; a junction follows its terminal or the wire point it sat on, else the warps. The result must keep
  the cluster's own connections (netlist tolerance): every terminal (pins and labels, also in the middle of
  a segment), every wire end that lay on another wire of the cluster (T-joints) and every junction must
  still lie on the same wires as before. It is used only if that holds, it touches no other line, pin,
  label or junction and it crosses no other symbol's outline (the symbols at its own terminals may be
  crossed); otherwise the cluster falls back to labels instead of the whole arrange being refused. Nets
  kept this way are listed as `kept_shape`.
- A cluster that falls back to labels is **connected by labels** instead of being left broken: each
  terminal gets a short stub wire and a net label with the chosen name, so connectivity is kept. The stub
  tries, in order, a straight line of 2.5, 5 and 7.5 mm in the pin direction, the router's grid lead, a
  bent stub (half a step out, then sideways) and a straight line sideways; the first one that keeps 0.3 mm
  from other lines and 1 mm from other pins, labels and junctions wins, else the first that does not
  electrically touch anything, else the first. Stubs already placed count as obstacles for the next one.
  The reply lists these clusters.
- Clusters that reroute would not route are handled by what they touch:
  - all their terminals belong to one unit (or they touch no terminal at all): the wires move with that
    unit (or through `fx`/`fy`), unchanged in shape; this covers stubs with a free end;
  - they touch several units and differ in colour, cross-section or another wire attribute, have a free
    end, or have fewer than two terminals: they are connected by labels as above, each lead taking the
    attributes of the old wire at that terminal;
  - they are used by a cable, shared by several nets (port contacts), pointed at by another entity or a
    FreeCAD link, carry lengths from different sources, or hold labels with different names: the whole
    arrange refuses and the reply names them.
- The fallback name, in order: the cluster's own label name (those labels are then deleted, since the new
  labels carry the same name, except labels another entity points at); else the label name of the same net
  on another cluster (a label of the same name does not change connectivity; never the `<label>.<contact>`
  name of a port-contact net, which is not label text on the sheet); else the net name of a
  net-name symbol (power or ground mark) in the cluster; else a fresh `ARR_<n>` not used by any label or
  net-name symbol in the project.
- Net-name symbols (power and ground marks) are never deleted. A wire that joins a mark's pin directly to
  a pin of the same unit is kept and moves with that unit; neither of its ends gets a stub.
- A cluster whose terminals all lie in one unit (a part and its own mark, for example) moves rigidly with
  that unit and is not routed again.
- A small circle shape (radius up to 1 mm) that sat on a wire is a connection dot: if, after the plan, it
  no longer lies on any wire, it is removed.
- Sheets that hold cables (`Cable`) or panel-layout entities are refused.
- Before returning, the plan is applied to a copy of the sheet and the pin partition (which pins share a
  net) is compared with the original; so are, per net, the names its labels and net-name symbols give it
  (ignoring only fresh `ARR_<n>` names), the labels and net-name symbols sitting directly on pins, and
  that every new label stub touches nothing but its own pin, and that no symbol was deleted (arrange never
  deletes parts or marks); any difference refuses with nothing changed.
- Label text room: a stub candidate is also judged by where its label's printed text lands (same
  rectangle as the tidy metric, via `tidy::net_label_text_rect`). The first candidate that clears the
  wires and whose text overlaps no symbol outline, no text, no wire segment and no earlier new label is
  used. Extra straight candidates (text width rounded up to the 2.5 mm grid plus one step, and up to two
  steps more) give the text room beside the stub; a tagged template gets the tag side whose body points
  away from the pin (west stub: `right`, east: `left`; vertical: both sides tried). When no candidate has
  text room, the old choice (first wire-clear candidate, then first non-touching, then the first) is kept,
  so connectivity checks behave as before.
- The title block area is taken from the frame template's corner (`title_block.origin`), its width and
  height, plus the revision rows stacked above it.

### Reply

`{ moved: n_units, columns, rows, routed: [net], labelled: [{net, label}], kept_shape: [net], before: TidyMetrics,
after: TidyMetrics, wire_length_mm: {before, after}, size_mm: {before:[w,h], after:[w,h]} }`.

## Code shape

- `src-tauri/crates/madake-core/src/arrange.rs`: units, bands, maps, fit,
  `plan_arrange(sheet, defs, &ArrangeOptions, &ArrangeContext) -> Result<ArrangePlan, String>`, where the
  context carries the protected ids, the frame area, the title block area and the names already used in
  the project.
- `route.rs` is split into collecting the clusters (`collect_route_units`) and routing them on a grid
  (`route_units`), shared by reroute and arrange; reroute's behaviour does not change.
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
