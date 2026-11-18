# Function sheet draft: design

Date: 2026-11-18. Status: design approved by the user in chat; this written spec awaits review.

## Why

A large wiring drawing reads well on paper when it is divided into A3 sheets with one function per sheet. Drawing such a sheet by script works, but the script has to decide every box size, pin order, tag position and ground mark by hand. Those decisions follow fixed rules, so the application can make them.

The work is divided in two:

- A person or an AI decides **what** goes on a sheet: which units, and in which column from left to right.
- MadakeCAD decides **how** it is drawn: box contents, pin sides and order, wires, tags, power and ground marks, spacing.

A second command checks that all drafted sheets together carry the same connections as the source drawing.

## Terms

- **Source**: a project file that holds the complete wiring (for example one large sheet). It is read only. It is never the open project.
- **Unit**: all symbol instances of the source that share one reference (for example `U7`).
- **Function sheet**: a sheet created by the draft command in the open project.
- **View**: a block on a function sheet that shows a subset of the pins of one unit. One unit may have views on several sheets.

## Global constraints

- Every document edit goes through a `Command`. The draft is applied as one edit, so one undo removes the whole sheet.
- The plan is made by a pure function in madake-core (`draft.rs`) that does not change the document, in the pattern of `route::plan_reroute` and `arrange::plan_arrange`.
- Rust tests carry two `///` lines (English, then Japanese) right before `#[test]`; Vitest tests carry `// ja:`. Regenerate `docs/13-specification*.md` with `python3 scripts/gen_spec.py`.
- A file format change bumps `FORMAT_VERSION` (now 29) with a migration and a round-trip test. New string attributes in `attrs` alone do not change the format.
- The canvas (TypeScript) draws the same as SVG / PDF.
- Docs: `docs/10-automation-api.md` + `.ja.md`, `docs/internal/data-model.md`, `docs/internal/feature-inventory.md`, MCP tool descriptions, CLI help, `README.md` + `.ja.md`, and the gap ledger. No names, numbers or wording taken from any reference drawing: examples use invented names such as `U7`, `J7.2`, `P24X`.
- Commit messages in Japanese with a conventional prefix. No `Co-Authored-By` or other attribution trailer.
- No UI button in this version (same as reroute and arrange).

## 1. Prerequisite: a connector contact is identified by its pole

Today every `connector_contact` instance has the pin key `1`, so two contacts of connector `J7` on different nets both appear in the netlist as `J7:1`. Change: the pin key of a `through` symbol is the instance's value (the pole number) when the value is not blank; a blank value keeps the key `1`. `get_pins`, the netlist, `connect_pins`, the wire list and KiCad export use the same key. Two contacts with the same reference and the same pole on different nets are reported by verification (`erc.contact_pole_repeated`, error). Two on the same net are allowed (the same contact drawn twice).

## 2. Views: one unit on several sheets

A block with the attribute `block_view` = `"1"` is a view. Rules for blocks that share a reference and are all views:

- They are one part. `erc.duplicate_reference` does not report them, on one sheet or across sheets. A view together with a non-view symbol of the same reference is still reported.
- The BOM counts them once. Part number and value are taken from the first view in sheet order that has them; views that disagree are reported (`erc.view_part_mismatch`, warning).
- A pin key may appear in only one view. A repeat is reported (`erc.view_pin_repeated`, error) with both entities.
- The cross-sheet location text for a repeated reference is not printed for views.
- The netlist lists each pin under the shared reference, so the project netlist of all views equals the netlist of the whole unit.

The draft command sets `block_view` on every block it creates. A person may set it by hand through `update_entity`.

## 3. Draft command

### Input

```json
{
  "source_path": "/path/to/source.mdkproj",
  "name": "2 Control",
  "columns": [["U1", "U2"], ["TB1"], ["M1", "M2"]],
  "pins": {"U1": ["1", "2", "5"]},
  "power_nets": ["P24X"],
  "ground_nets": ["GND"],
  "destinations": {"J7.2": "cabinet B"},
  "paper": "A3",
  "pitch_mm": 5.0
}
```

- `source_path`: required. The file is read with the normal loader (old formats migrate in memory); it is not modified.
- `name`: required, not blank, not the name of an existing sheet.
- `columns`: required. One to eight columns, each a list of unit references, top to bottom. A reference may appear once.
- `pins`: optional. Limits a unit to the listed pin keys (for a unit that is divided over several sheets). Without an entry, the view carries every pin of the unit that is in use.
- `power_nets`, `ground_nets`: optional lists of source net names. Default for `ground_nets`: every source net that carries a net mark symbol. Default for `power_nets`: empty.
- `destinations`: optional. Net name to free text, for a net that continues outside the project. It becomes `tag_dest`.
- `paper`: `A3` (default) or `A4`, landscape.
- `pitch_mm`: pin pitch of the boxes, 2.5 or 5.0 (default 5.0).

### Which pins a view carries

A pin of a unit is **in use** when its source net has two or more pins. The view carries the pins in use, limited by `pins` when given. The pin name printed inside the box is the pin name of the source definition (block pin name or library pin name); the number outside is the source pin key. A block pin without a number keeps its name as key.

A unit whose source symbol is not a block (a library symbol such as a resistor or a relay coil) is not turned into a box: the same symbol is placed with the source rotation, reference and value. Section 2 does not apply to it; it may appear on one function sheet only (a second use is refused).

### Net classes

For each source net that touches a carried pin:

1. **Power or ground** (name in `power_nets` or `ground_nets`): no wire. Each carried pin gets its own mark on a 5 mm stub: `power_rail` with the net name as value for power; for ground, the net mark symbol that the source uses on that net, or the library ground mark when the source has none.
2. **Internal**: every pin of the net is carried on this sheet. The pins are joined by wires.
3. **Leaving**: some pins are not on this sheet. The carried pins are joined by wires when there are two or more, and one tag (a net label with `tag`) is placed at the pin that comes first in column order. With one carried pin the tag sits at the end of a 5 mm stub. With two or more, the tag hangs on its own branch: the pin's 5 mm stub, then 5 mm more on the same row to the tag, with the flag pointing away from the box; the wire to the other pins leaves the branch before the flag, so the tag is a free end and no wire runs through or along a flag. The pin, the tag and the other pins are one net.

The tag name is the source net's label name. A source net without a label gets the name `<reference>.<pin>` of the pin that comes first when all pins of the net are sorted by reference and pin key in natural order. The rule uses the whole source net, so every sheet that shows the net computes the same name.

The new sheet has `xref_style` = `sheet_name`, so a tag shows the names of the other sheets that carry the same name as soon as those sheets exist. A pin on the right side of a box gets `tag_flow` = `out`.

### Pin sides and order

- A pin whose peers on this sheet are all in columns to the left goes on the left side; all to the right, on the right side; both, on the side with more peers (left on a tie).
- A pin with no peer on this sheet (leaving nets with one carried pin): right side for a unit in the last column, otherwise left side.
- Power pins are the first rows of the left side, ground pins the last rows of the left side, each group separated from the signal pins by one spacer row (`{"gap": 1}`).
- Signal pins on a side are ordered to reduce crossings: two sweeps of the barycentre method over the vertical positions of their peers, ties broken by natural order of the pin key. The result must not depend on hash order: the same input gives the same sheet.

### Placement

- The drawing area is the frame's inner area minus the title block.
- Columns are spread evenly over the width. Units in a column are stacked top to bottom in the given order, with a gap of at least four pitches.
- Box width holds the longest pin name on both sides plus the reference; `block_pin_text` is chosen so that names are at least 2.2 mm high.
- Tag text size is chosen so that tags on adjacent pins do not touch (flag height at most pitch minus 1 mm; at a pitch of 2.5 mm the text cannot be smaller than the smallest annotation size, so the flag is taller and adjacent flags are only 0.25 mm apart).
- A tag's width reserves room for the names of other sheets that appear inside it later: the wider of its `tag_dest`, the longest sheet name of the open project and 10 characters.
- Symbols, pins, the ends of stubs, tags and marks are on the 2.5 mm grid. The bends of routed wires may lie on the router's 1.25 mm grid.
- A column that is taller than the drawing area, or columns that do not fit the width with a routing channel of at least 15 mm between them, refuse the draft. The message names the column and the excess in mm, so that the caller can move units to another sheet. Nothing is created.

### Wires

Internal wires are planned with the existing router (`route.rs`) on the new sheet's geometry: orthogonal, off symbols and text, right-angle crossings only, junctions where three or more ends meet. Every tag flag (with the reserved destination width) and every mark with its text is an obstacle kept at the router's clearance, and wires stay inside the frame's inner area and out of the title block with its revision rows, as `arrange::frame_areas` reports them for the open project's frame template. A net the router cannot place is joined by a pair of net labels with the net name instead and listed in the result (`labelled`).

### Result

One edit: add sheet, add entities. Returned JSON:

```json
{
  "sheet_id": "…",
  "units": 5,
  "pins": 42,
  "wired": ["N1", "…"],
  "tagged": [{"net": "J7.2", "pins": 1}],
  "marked": {"power": 3, "ground": 6},
  "labelled": [],
  "skipped_pins": [{"reference": "U1", "pin": "9", "reason": "net has one pin"}],
  "metrics": {}
}
```

`metrics` is the same object as `get_tidy_metrics` for the new sheet.

### Refusals

Each refusal changes nothing and states the cause: source file missing or unreadable; a reference in `columns` that the source does not have; a reference listed twice; a pin key in `pins` that the unit does not have; a net named in `power_nets` / `ground_nets` / `destinations` that the source does not have; a net in both `power_nets` and `ground_nets`; a sheet name already in use; a pin already shown by a view on another sheet of the open project; no carried pin at all; the size refusals above; the open project changed between planning and applying (same version check as arrange).

## 4. Check command

`check_function_sheets { source_path, ignore_references? }` compares the source with the open project and changes nothing.

- Both sides are reduced to sets of pins `(reference, pin key)` per net. Nets with fewer than two pins, net mark symbols and `through` symbols are left out. Net names are not compared; the grouping of pins is.
- `ignore_references`: source references to leave out (for example pass-through points that are not drawn).
- Result:

```json
{
  "ok": false,
  "source_nets": 12,
  "matched": 10,
  "missing_pins": [{"reference": "U3", "pin": "4", "source_net": "P24X"}],
  "extra_pins": [],
  "split": [{"source_net": "SIG_A", "parts": [["U1:3"], ["U2:3", "U4:1"]]}],
  "merged": [{"source_nets": ["A", "B"], "pins": ["…"]}]
}
```

`missing_pins`: in the source, on no sheet. `extra_pins`: on a sheet, not in the source. `split`: pins of one source net that are not one net in the open project (a tag name that differs between sheets shows up here). `merged`: pins of different source nets that are one net in the open project.

## 5. Entry points

- madake-core: `draft::plan_function_sheet(source: &Project, open: &Project, request: &DraftRequest) -> Result<DraftPlan, String>` and `draft::check_function_sheets(source: &Project, open: &Project, ignore: &[String]) -> DraftCheck`.
- `SharedDoc::draft_function_sheet` applies the plan as one batch.
- MCP tools `draft_function_sheet`, `check_function_sheets`.
- Link API `POST /api/v1/draft/function-sheet`, `POST /api/v1/draft/check`.
- CLI `madake draft <request.json>` and `madake draft-check <source.mdkproj> [--ignore REF]...`.

## Out of scope

- Choosing which units belong on a sheet, and splitting a sheet that is too full.
- Layout quality for board schematics made of library symbols (they are placed, wired and checked, but columns of two-pin parts will not read like a hand-drawn schematic).
- Cables, ports with contacts, mates and harnesses in the source: a unit whose carried pin is a port with contacts, or a net that runs through a cable core, refuses the draft with the reason.
- An overview sheet with a sheet index.
- Updating a function sheet after the source changed (draft again after deleting the sheet).
- Per-sheet frame templates and fonts; a terminal-block common bar; one tag for several wires.
- A pin whose peers on the sheet are on both sides of its box goes on one side only (the side with more peers), so its wire to the other side runs round the box.

## Testing

- Unit tests in `draft.rs` on invented sources: pin selection, the three net classes, the name rule for unlabelled nets, pin sides, deterministic pin order, placement on the grid, every refusal.
- A two-sheet scenario: one unit divided over two sheets by `pins`; the project netlist equals the source; the check returns `ok`.
- Check command: one test each for missing, extra, split and merged.
- Section 1 and 2: verification, BOM, netlist, KiCad export of views and contacts.
- SVG test for the look of a drafted sheet (golden file from an invented source), with the TypeScript twin where the canvas draws something new.
- Link API, MCP and CLI: one round trip each.

## Review focus

1. Two sheets that show the same unlabelled net must compute the same tag name, whichever sheet is drafted first.
2. A net with pins on three sheets: every sheet has a tag, and the check reports one net, not a split.
3. A power net that is also the label name of an ordinary tag elsewhere: marks and tags with the same name are one net.
4. A unit with only power and ground pins on a sheet (no signal pin): it is still drawn, with marks only.
5. Drafting the same request twice: the second call is refused (sheet name in use), and after one undo the project is byte-identical to before the first call.
