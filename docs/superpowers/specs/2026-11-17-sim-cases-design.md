# Simulation study cases and faults by click (design)

Date: 2026-11-17. Status: design approved in conversation; written spec awaiting review.
Sub-project 2 of 4 of the simulation usability work (1 results on the drawing — done; 2 this; 3 an "Electrical"
section in the properties; 4 time-current curves).

## Problem

Conditions for a simulation (attribute overrides, shorts) are typed as text (`DRV01.current_a=31`, `短絡 N045`),
live only in the open window, and shorts name automatic nets (`N045`) that change when the drawing is edited.
Comparing a worst case with a fault means retyping everything.

## Goals

- Named study cases saved with the project ("motor maximum", "P24OUT short", ...), one click to switch.
- Shorts and wire breaks chosen on the drawing by right click, remembered by wire / pin, not by net name.
- Attribute overrides edited next to the selected part, not typed.

Out of scope: running all cases and comparing them in a table; shorts between two points; editing roles and
ratings of the drawing itself (sub-project 3); time-current curves (sub-project 4).

## Design

### 1. Data (`madake-core/src/model.rs`, format 27)

```
Project.sim_cases: Vec<SimCase>        // #[serde(default)], omitted when empty
SimCase {
  id: Uuid,
  name: String,                        // not empty; unique within the project
  open_switches: Vec<String>,          // references
  overrides: BTreeMap<String, BTreeMap<String, String>>,   // reference -> {attribute: value}
  faults: Vec<SimFault>,
}
SimFault = { "kind": "short", "target": FaultTarget } | { "kind": "open", "wire_id": Uuid }
FaultTarget = { "wire_id": Uuid } | { "entity_id": Uuid, "pin": String }
```

- "Normal" (no conditions) is the implicit base state and is not stored.
- `FORMAT_VERSION` 26 -> 27; files without `sim_cases` load with an empty list (serde default), so the migration
  only bumps the version.
- Commands (through `Engine::execute`, undoable, sent to the UI as patches):
  - `SetSimCase { case: SimCase }`: insert, or replace the case with the same id. Inverse: the previous case, or
    `RemoveSimCase` when it was new. Refused when the name is empty or another case already has it.
  - `RemoveSimCase { id }`: inverse is `SetSimCase` with the removed case at its old position.
- Which case is selected is UI state, not saved.

### 2. Simulation (`spice.rs`, `sim.rs`)

- `SimOptions.faults: Vec<SimFault>` (serde default). Also `DeckOptions.faults`.
  - Short on a wire: the wire's first point is joined to ground (node 0) through 1 mΩ (an element of kind
    `Short("wire")`).
  - Short on a pin: that pin's point is joined to ground the same way.
  - Open wire: the wire gets no resistor elements (its ends are no longer joined through it).
  - A target that no longer exists (deleted wire, symbol or pin) is skipped with a warning naming it.
- The existing name-based `shorts` stay (CLI `--short NET`).
- `case_id`: Link API `POST /api/v1/simulate/op`, the MCP tool and the Tauri command accept an optional
  `case_id`. The case's `open_switches`, `overrides` and `faults` are used; explicit fields in the same request are
  added on top. An unknown id is an error. CLI: `madake sim --case NAME_OR_ID`.

### 3. User interface (designed in `MadakeCAD.pen` first)

- Header: a case selector ("Normal" + saved cases) with New, Rename, Duplicate, Delete.
- Conditions bar: chips of the selected case (overrides, shorts, opens); × removes from the case. The text input
  stays for advanced use: `REF.ATTR=VALUE` adds an override, `短絡 NET` / `short NET` adds a short on the first
  wire of that net (stored by wire id).
- Right click on the canvas (only while the simulation panel is open):
  - on a wire: "Short here", "Break here";
  - on a symbol: "Short at pin X" for the pin nearest the pointer;
  - when the target already has a fault in the case: "Remove this condition".
- Adding a condition while "Normal" is selected creates a case "Case N" (first free number), selects it and adds
  the condition there.
- Details column (selected part): the attributes that matter for its role (source: voltage, r_int_ohm, i_max_a;
  load: current_a, v_min; fuse: rating_a; converter: v_out, eta, i_out_a, i_limit_a, v_in_min; any part:
  pin_rating_a) with the drawing value and an input for "value in this case". Entering a value saves it in the
  case (creating one from "Normal" as above) and re-runs.
- Every change to the selected case re-runs the simulation when a result is shown (as today).
- Drawing markers: a short is drawn as a lightning mark at its point, an open as a cross at the middle of the
  wire (canvas overlay, colours from `theme.ts`), while the panel is open and the case is selected.

### 4. Errors and edge cases

- Faults whose target is gone: skipped in the simulation with a warning; the chip is shown greyed with "(missing)".
- Deleting a case that is selected selects "Normal".
- Undo of a case edit restores the previous case; the panel follows the patch.

### 5. Tests (the specification)

- Rust: format 27 load/save (old files load with no cases); `SetSimCase` insert / replace / undo, refusal of an
  empty or duplicate name; `RemoveSimCase` and undo; short on a wire and on a pin give the fault current; open wire
  stops the current through it; missing targets give warnings; `case_id` applies the case (and an unknown id is an
  error) in core, Link API and MCP; CLI `--case`.
- Vitest: store (select case, conditions shown from the case, adding while "Normal" creates "Case N", removing a
  chip edits the case, deleting the selected case falls back to "Normal"); context menu items for a wire, a symbol
  (nearest pin), an existing fault; details-column attributes per role; text input `短絡 NET` maps to the net's
  first wire.
- Manual: the A0 analysis copy in a dev build: cases "motor maximum" and "P24OUT short" made by click, switched,
  saved and reopened.
