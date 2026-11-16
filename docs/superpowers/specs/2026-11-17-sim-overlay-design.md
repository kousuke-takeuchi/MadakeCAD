# Simulation results on the drawing (design)

Date: 2026-11-17. Status: design approved in conversation; written spec awaiting review.
Sub-project 1 of 4 of the simulation usability work (1 results on the drawing, 2 shorts by click and named
study cases, 3 an "Electrical" section in the properties, 4 time-current curves for protection coordination).

## Problem

The DC operating point simulation now works for any drawing (symbol `sim` roles, conditions, findings), but its
results only appear as lists under the canvas: about 160 net voltages with automatic names (`N045`), component
currents, and findings as sentences. A user cannot see where on the drawing the current is high, which wire or
part is overloaded, or how close the rest is to its limit. Other electrical tools show results on the diagram
itself (values on hover, overloaded items coloured).

## Goals

- See at a glance which wires and parts are overloaded or close to their limit, on the drawing.
- Read the current and voltages of any wire or part by pointing at it.
- Keep the panel short: what is wrong, and the details of what is selected.

Non-goals (later sub-projects): shorts chosen by clicking, named study cases, editing roles and ratings in the
properties, time-current curves.

## Design

### 1. Core: per-entity loads (`madake-core/src/sim.rs`)

`SimOpResult` gains `loads: Vec<EntityLoad>`:

```
EntityLoad {
  entity_id,
  kind: "wire" | "component",
  amps,            // wire: largest current of its segments; component: as in `components`
  volts: [f64],    // wire: voltage at each end (first point, last point); component: voltage at each pin, in pin order
  limit: Option<f64>,
  limit_kind: Option<"wire_ampacity" | "fuse_rating" | "pin_rating" | "source_max" | "converter_out">,
  ratio: Option<f64>,   // amps / limit
}
```

- One entry per wire that has at least one SPICE element and per symbol that has elements (labels, shorts and
  ground ties are not loads).
- The limit is the one the findings already use: wire ampacity from `sq` (none for `sq` 0 or thicker than the
  table), fuse `rating_a` / value text, the highest bridge current against `pin_rating_a`, source `i_max_a`,
  converter output against `i_out_a`. A component with several limits reports the one with the highest ratio.
- `findings` are unchanged; an entry with `ratio > 1` is exactly an item that also has a finding.
- CLI `madake sim --json`, the Link API and the MCP tool return `loads` too (no new options).

### 2. Overlay on the canvas (`src/canvas/simOverlay.ts` + `renderer.ts`)

- `simOverlay.ts` (pure logic, no DOM): from a `SimOpResult` builds a map entity id -> `{ level, label }`:
  level `none` (ratio < 0.8 or no limit), `near` (0.8 <= ratio <= 1.0), `over` (ratio > 1.0); label such as
  `70.9A / 50A` for `over` only.
- `renderer.ts` draws, below the normal ink: a wide translucent stroke under coloured wires and a rectangle around
  the bounds of coloured symbols (amber for `near`, red for `over`, colours from `theme.ts`); then the `over`
  labels at a constant screen size next to the item.
- Probe: when the pointer rests over a wire or a symbol that has a load entry, a small tooltip shows current,
  end/pin voltages, limit and ratio. Hit testing reuses the existing entity hit test of the canvas.
- A toggle "Show on drawing" (default on while the panel is open) hides the overlay; closing the panel hides it.

### 3. Stale results

- The store records the document revision a result belongs to. When the current revision differs, the result
  is stale: the overlay is drawn at reduced opacity, labels are hidden, and the panel says "The drawing changed.
  Run again to update." Nothing recomputes automatically (large drawings stay fast to edit).

### 4. Panel (redesigned in `MadakeCAD.pen` first)

- Header and conditions bar as today.
- Body, left: findings (filter by severity; click selects and centres, as today).
- Body, right: details of the selected wire or part (reference or wire, current, voltages, limit and kind,
  ratio), or a hint to select something.
- "Show all values": a collapsible section with the full net voltage and component current lists.
- The "Show on drawing" toggle in the header.

### 5. Errors and edge cases

- A failed run clears the overlay (no colours from an older result).
- Entities deleted after a run are skipped by the overlay.
- Results of another sheet are not drawn on the active sheet (the store keeps the sheet id of the run).

### 6. Tests (the specification)

- Rust (`sim.rs`): `loads` for a wire (amps, end voltages, ampacity limit and ratio), a fuse, a connector pin, a
  source, a converter output; no limit for a wire without `sq` or thicker than the table; labels/shorts/ties
  not listed; the highest-ratio limit wins for a component with several.
- Vitest: overlay levels at 0.79 / 0.8 / 1.0 / 1.01 and without a limit; label text; stale detection by revision;
  sheet filtering; the details column for a selected wire and a selected part; the toggle.
- Manual: the A0 analysis copy in a dev build (worst case and a short show red wires and parts).
