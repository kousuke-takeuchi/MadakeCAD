# Drawing aids for function sheets: design

Date: 2026-11-17. Status: approved by the user in chat (five items).

## Why

A large wiring drawing reads well on paper when it is divided into A3 sheets with one function per sheet: unit boxes that carry only the pins in use, inputs on the left, outputs on the right, power and ground as symbols, and a tag right next to the pin wherever a wire continues on another sheet. Three such sheets were redrawn with the current features. Connectivity came out right, but five things kept the result from reading like the hand-drawn target. This spec closes those five.

## Global constraints

- Every document edit goes through a `Command`. No direct model mutation.
- Rust tests carry two `///` lines (English, then Japanese) right before `#[test]`; Vitest tests carry `// ja:`. Regenerate `docs/13-specification*.md` with `python3 scripts/gen_spec.py`.
- A file format change bumps `FORMAT_VERSION` (now 27) once per task that changes the format, with a migration and a round-trip test. Old files must load unchanged in look.
- The canvas (TypeScript) draws the same as SVG / PDF. Golden files shared by Rust and Vitest stay in step.
- KiCad and DXF export and import keep working: new attributes round-trip where the existing ones do, or are dropped with the documented behaviour of their neighbours.
- Docs: `docs/10-automation-api.md` + `.ja.md`, `docs/internal/data-model.md`, `docs/internal/feature-inventory.md`, MCP tool descriptions, and the gap ledger, in general terms. No names, numbers or wording taken from any reference drawing: examples use invented names such as `J7.2`, `P24X`.
- Commit messages in Japanese with a conventional prefix. No `Co-Authored-By` or other attribution trailer.

## 1. Tags that say where the wire continues

Today a tagged net label (`attrs.tag` = `left` | `right`) shows the net name in a flag; the cross reference is small text outside the flag in the form `/sheet.zone`, and exists only when the same name appears on another sheet.

Add:

- `Sheet.xref_style`: `"zone"` (default, today's look) or `"sheet_name"`. With `sheet_name`, a tagged label draws the destination inside the flag, after the name: the names of the other sheets that carry the same label name, in sheet order, joined with `・`; more than two become the first two plus `他`. The name is drawn bold at the label size, the destination at 0.8 of it, with a gap of one label height between them. The flag grows to hold both. No text is drawn outside the flag in this style. Untagged labels keep today's behaviour in both styles.
- `attrs.tag_dest` on a net label: free text that replaces the automatic destination (for a destination that is not a sheet of this project). An empty or blank value means automatic. It applies in both styles: in `zone` style it replaces the outside text.
- `attrs.tag_flow` = `"out"`: the flag's pointed end moves to the far end of the body, so that a tag at the end of an outgoing wire points away from the wire. Absent or any other value: today's shape (pointed end at the label point). The body side still follows `tag`.
- Command: `set_sheet_xref_style { sheet_id, style }`, undoable. MCP tool and Link API route follow the pattern of `set_sheet_wire_style`.
- Text boxes used by the tidy metrics (`tidy::text_boxes`) and by arrange's label placement follow the new extents.

## 2. Net names that contain a dot

A label whose name contains a dot (for example `J7.2`) on a plain wire leaves the wire reported as dangling, while the same label without the dot connects. Find the cause first (the suspect is the port-contact tap syntax `L.CONTACT`), then fix it so that:

- a dotted name whose prefix is not a port net on that wire's net is an ordinary net name: it joins same-named labels across sheets and the wire is not dangling;
- a real port-contact tap keeps working exactly as today.

Tests pin both.

## 3. Connector contact on a wire

A new library symbol `connector_contact` (category connector): a small circle (radius 1 mm) on the wire with two pins, left and right, 2.5 mm from the centre, that are one electrical node (the symbol is a pass-through: a wire on each side is the same net, and the net lists the contact once). The designator is the connector (`J7`), the value is the pole number, drawn small beside the circle; several contacts may share one designator without `erc.duplicate_reference`; a blank designator is still reported by `erc.empty_reference` (a contact needs its connector name). The BOM counts contacts of one designator as one connector. Implement the pass-through as a symbol definition property (`through: true`: pins with the same number are one node) rather than a special case, reusing the feed-through logic of blocks where it fits.

## 4. Block pins

`attrs.block_pins` entries gain:

- an empty `no`: allowed when the pin's `name` is unique within the block; nothing is printed outside the box; the pin's key (in `get_pins`, the netlist, `connect_pins`) is its name. Two pins that both lack a number and share a name are refused.
- spacer rows: an entry `{"gap": n}` (n = 1..20 rows) leaves n pin positions empty on that side;
- group captions: an entry `{"caption": "text"}` takes one row on that side and prints the text inside the box at that row, at the pin-name size, bold, aligned to that side. It has no connection point.

Block attribute `block_pin_text` (mm, 1.5 to 5.0, default today's size): the height of pin names, group captions and pin numbers scale with it; the box's minimum width grows to keep names from overlapping the opposite side.

`get_pins` skips spacer and caption rows. Pin positions stay on the grid of the block's pitch.

## 5. Symbols

Add to the generic library, drawn after JIS C 0617 where it defines the shape, pins on the 2.5 mm grid:

- `transistor_npn`, `transistor_pnp` (B, C, E);
- `photocoupler` (A, K, C, E; LED and phototransistor in one outline);
- `opamp` (+, −, OUT, V+, V−);
- `tvs_diode` (bidirectional surge absorber, two pins);
- `power_rail`: a one-pin power mark whose net name is the instance's value (definition property `net_from_value: true`, next to the existing fixed `net`): two marks with the same value join, across sheets too; the value is printed above the bar; a blank value is refused by verification (`erc.empty_net_name`, warning). It is left out of the BOM and exempt from the reference checks like the fixed-net marks.

## Out of scope

Per-sheet frame templates and fonts; a switch for designator location text; a terminal-block common bar; one tag for several wires; two destinations on one net.

## Testing

Each item: unit tests for the rules above including the refusals, an SVG test for the look (text and geometry present at the stated places), the TypeScript twin where the canvas draws it, a file round trip, and KiCad / DXF behaviour where a neighbour attribute has one.

## Review focus

1. Old files: a format-27 file with tagged labels, blocks and marks must render byte-identical SVG after migration.
2. A dotted label on a real port net must still tap the contact.
3. A block whose only pins are unnumbered: keys by name everywhere (`connect_pins`, netlist, KiCad export).
4. `sheet_name` style with the same label on three or more sheets, and on no other sheet (no destination: the flag shows the name only).
5. `power_rail` with the same value as an existing net label or a fixed-net mark: they are one net.
