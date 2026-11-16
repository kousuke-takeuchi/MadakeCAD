# Wire reroute — design

Date: 2026-11-16. Status: draft for review.

## Problem

Drawings that come from somewhere else keep the wire geometry of their source: a traced drawing copies
long detours and bundle routes, a drawing split from a large sheet keeps runs that were laid out for the
large sheet, and an imported KiCad or DXF file keeps whatever its author drew. The result is hard to read:
wires take long detours, run close to or over symbols and text, and cross more than they need to.
MadakeCAD can measure this (`tidy.rs`: crossings, label overlaps) but cannot fix it; `pin_wiring::route`
only lays an L or Z shape between two pins and does not avoid anything.

## Goal

A command that redraws the wires of a sheet (or of chosen nets) automatically:

- connectivity is unchanged: every net connects exactly the same pins, labels and marks as before;
- wires are orthogonal, avoid symbols, text and other nets, and use few bends and few crossings;
- one undo step restores the previous drawing.

Out of scope here (later design): moving symbols (alignment, compaction, left-to-right ordering), a canvas
button for the command (needs a design in `MadakeCAD.pen` first), routing of cable runs and ports.

## User-facing behaviour

### Entry points

- MCP tool `reroute_wires { sheet_id?, nets?: [name], options? }`.
- Link API `POST /api/v1/route/reroute` with the same body.
- CLI `madake reroute [--sheet ID] [--net NAME ...]`.

All three plan the result first (pure function in madake-core) and then apply it as **one batch of
existing commands** (`delete_entities` + `add_entity`), the same pattern `connect_cable` uses. No new
`Command` variant and no file format change. Undo restores the old wires in one step.

### What is rerouted

- Default: every net on the sheet that is made only of plain wires and junctions.
- `nets`: only the named nets (net names as the netlist reports them).
- A net is **left unchanged** (and listed in the reply with the reason) when it:
  - has a wire that belongs to a cable run, a harness core or a mate pair;
  - has wires with different colour, cross-section, part number, wire number, dash or weight (the
    reroute cannot know which new segment should carry which attribute);
  - cannot be routed (no path found) — its old wires stay.

### Terminals

The points a net must connect: the symbol pins on the net, the net labels on the net, and the pins of
net-name symbols (ground and power marks). Junctions and wire bends are not terminals; they are
recreated where the new route needs them.

### Result shape

- Wires run horizontally and vertically on a 1.25 mm routing grid. A pin that is not on that grid (user
  symbols with `free_pins`) gets a short straight lead along its pin direction to the nearest grid line.
- A wire leaves a pin in the pin's direction.
- A net with three or more terminals is a tree: each further terminal joins the existing tree with a T,
  and the T gets a junction dot. No point carries four branches (a four-way meeting is split into two
  Ts at least one grid step apart).
- A route never passes over a pin of another net, never runs along (overlaps) a wire of another net, and
  never passes through a symbol outline or a text box. Crossing another net at right angles is allowed
  but costs.
- Attributes of the old wires (colour, cross-section, part number, wire number, dash, weight, length)
  are copied to every new segment of the net. `length_m` is cleared when it came from geometry
  (`length_source` derived) and kept when it was entered by hand.

### Reply

`{ rerouted: [net], unchanged: [{ net, reason }], before: TidyMetrics, after: TidyMetrics,
wire_length_mm: { before, after } }`.

## Algorithm (madake-core `route.rs`)

1. **Netlist and selection.** `extract_netlist(sheet, defs)`; pick nets per the rules above; collect
   terminals and the old wire and junction ids per net.
2. **Obstacle map.** A grid over the paper area at 1.25 mm. Blocked cells: symbol bounds
   (`tidy::symbol_bounds`, inflated by 0.6 mm) except the lead cells in front of each pin, text boxes
   (`tidy::text_boxes`), frame and title block. Wires of nets that are not rerouted are fixed obstacles.
3. **Order.** Nets with fewer terminals and shorter bounding boxes first (ties by net name, so the
   result is deterministic).
4. **Route a net.** Start the tree at the terminal nearest the net's centre. Repeatedly connect the
   nearest unconnected terminal to the tree with A* on the grid:
   - cost = length (1 per mm) + bend (6) + crossing another net (15) + running next to another net's
     wire in the adjacent cell (2);
   - forbidden: blocked cells, cells on another net's pin or junction, moving along a cell edge that
     another net already uses in the same direction;
   - joining the tree at an interior point of a tree segment creates a T and a junction; joining at a
     point that already has three directions is forbidden.
   After a net is routed its segments become obstacles for later nets.
5. **Rip-up and retry.** If a net fails, remove the most recently routed net that blocks it, route the
   failed net, then retry the removed one; at most three rounds per net. Still failing: the net keeps its
   old wires and is reported.
6. **Emit.** Merge consecutive collinear grid steps into polyline vertices; one `Wire` per tree edge
   between terminals/junctions; `Junction` entities at Ts.

Bounds: the search is limited to the paper; the grid is about 340 × 480 cells for A2 portrait, so A* per
connection stays well under a second for the sheet sizes MadakeCAD supports.

## Verification

- Unit tests in `route.rs` (each with the English and Japanese specification lines):
  straight pin-to-pin; L shape around nothing; detour around a symbol; detour around a text box; does not
  pass over another net's pin; does not overlap another net's wire; crosses at right angles when it must;
  three-terminal net makes one T with a junction; four terminals around one point make two Ts, never a
  four-way point; off-grid pin gets a lead to the grid; net with mixed wire colours is left unchanged and
  reported; unroutable net keeps its wires and is reported; result is deterministic (same input, same
  output).
- Property test: for each sample sheet under `src-tauri/crates/madake-core/tests/` and a set of generated
  sheets, the netlist before and after reroute has the same pin sets per net.
- MCP and Link API tests: the tool applies one batch; one undo restores the original entities.
- Locally (not committed, private drawings): run it on the split reproduction sheets and compare tidy
  metrics and total wire length before and after.

## Documents

`docs/internal/feature-inventory.md`, `docs/10-automation-api(.ja).md` (tool and endpoint), CLI help in
`README(.ja).md`, regenerated `docs/13-specification*.md`.
