# Reproduction gap ledger — reference wiring diagram, sheet 1

Created: 2026-08-23. Japanese version: [reproduction-gaps.ja.md](reproduction-gaps.ja.md).

## What this is

An attempt to redraw sheet 1 of the project's origin reference drawing (a 5-sheet
A4-portrait harness/wiring diagram, currently produced in another EDA tool) using
**only the features MadakeCAD ships today**, and a ledger of everything that got in
the way. This is the input to the phase-5 feature design.

The reproduction was built deterministically through the Link API
(`POST /api/v1/commands`), not through the AI chat, so every verdict below is the
result of actually issuing the command and looking at the exported PDF — not a guess.
The reproduction data, the exported PDF/SVG, the reports and the side-by-side crops
live outside version control (the reference material is confidential).

The bar is **"does the same information land on the sheet"**, not pixel parity.

## Result at a glance

313 commands produced a complete A4-portrait sheet: JIS frame with zone numbers,
title block, three revision rows, a dashed enclosure, 20 connectors, two terminal
blocks, a relay, a controller block, ~70 annotated wires, ~60 net labels and notes.
It exports to SVG/PDF and to a 14-page book with wire list, terminal chart, terminal
diagram, BOM and cross-reference report. ERC on the finished sheet reports 11
diagnostics, all of them real (genuinely unconnected pins).

So the **structure and the electrical model are there**. What is missing is almost
entirely in the **annotation and symbol layers** — the parts that make an industrial
drawing readable on paper.

### By element type

| Element | Verdict | Note |
|---|---|---|
| A4 portrait JIS frame, zone numbers | ✅ | `jis-a4-portrait`, 4 cols x 6 rows |
| Title block | 🔶 | fields exist; several are crammed into one cell, no project name/number, no logo |
| Revision table | ✅ | mark/date/description/by, latest revision binds into the title block |
| 2-pole / 4-pole connectors with designator + type | ✅ | `connector_{n}p`, `reference` + `value` |
| Connector facing left (load side) | ✅ | `mirror: true` (note: `rotation: 180` reverses pin order instead) |
| Named connector pins (e.g. `P`/`N`) | ❌ | G1 |
| Function label above a device (a third text field) | 🔶 | G4 — free text placed by hand |
| Terminal blocks, through terminals | ✅ | `terminal_block_{n}p` |
| Terminal block with two terminal columns as one device | 🔶 | G6 — approximated with two blocks side by side |
| Jumper bars | 🔶 | G5 — stored and reported, never drawn on the schematic |
| Multi-pin controller block with arbitrary pin names | ❌ | G1 — approximated with a plain connector + hand-placed text |
| Pin-group brackets with rotated group labels | ❌ | G8 |
| Filled / shaded device blocks | ❌ | G10 |
| Relay: coil + two changeover contacts | 🔶 | G7 — three symbols instead of one physical device with manufacturer pin numbers |
| Net labels merging nets by name | ✅ | verified: same-named labels on opposite sides of the sheet form one net |
| Cross-sheet reference on net labels | ✅ | renders the `/sheet.zone` address of the twin label |
| Net label drawn as a directional flag tag | ❌ | G3 — renders as bare text |
| Wire numbers | ✅ | drawn once per net at the longest segment |
| Per-wire gauge / colour / length annotation on the sheet | ❌ | G2 |
| Per-wire-end crimp contact / ferrule part annotation | ❌ | G2 |
| Wire colour | ✅ | rendered; English and Japanese colour names both resolve |
| Wire part number from colour + gauge | ❌ | G9 |
| Harness boundary (dashed enclosure) | ✅ | |
| Enclosure / case boundary distinct from a harness | 🔶 | G11 — reusing harness pollutes harness reports |
| Highlight box (coloured rectangle annotation) | ❌ | G10 |
| Pictorial device symbols (fan, speaker, display module) | ❌ | G12 |
| Free notes | 🔶 | G4 — no colour, no anchor, no rotation, no multi-line |
| Junctions | ✅ | |
| Device-level cross-sheet block reference | ❌ | G13 |
| Mating connector pairs | ❌ | G14 |
| Reports (wire list, terminal chart/diagram, BOM, xref) | ✅ | ahead of the reference drawing |
| ERC / electrical verification | ✅ | |

---

## Gaps

Impact levels: **essential** = the reference drawing cannot be reproduced faithfully
without it · **quality** = reproducible but visibly below the target standard ·
**minor** = friction, not a blocker.

### G1 — Multi-pin device blocks cannot have named pins · essential

**Needed.** The reference sheet is built around devices whose pins carry names, not
ordinals: a controller block with ~30 pins (analog / digital / power group names), and
every field connector labelled with polarity letters rather than 1/2.

**Current behaviour (verified).** The parametric families `connector_{n}p`,
`terminal_block_{n}p`, `plc_di_{n}p`, `plc_do_{n}p` number their pins `"1"`…`"n"` and
expose no way to override the numbers or supply names. The 47 bundled symbols are
fixed. There is no user symbol directory, no symbol-registration command, and no API
that accepts a `SymbolDef`. Only `reference` and `value` are drawn per instance;
`SymbolDef.text_slots` (TAG/PART/DESC/RATING) is computed but the renderers ignore it
and hard-code two centred lines above the outline.

**Workaround used.** Place a plain multi-pole connector and lay one free text entity
next to each pin. This is ~30 extra entities per device, the names sit outside the
outline instead of inside it, and nothing ties a name to its pin — the netlist,
reports and ERC still only know the ordinals.

**Proposed.** Two independent pieces:
1. **Per-instance pin naming.** Let a symbol instance override pin labels, e.g.
   `attrs["pin_names"] = "P,N"` or a dedicated map, applied by the renderer, the
   netlist, the terminal chart and the wire list. This alone fixes the connectors and
   costs no new entity kind.
2. **A parametric "device block" symbol family**: a rectangle with a caption, pins on
   either side, each with a name, optional group brackets. This is the shape that
   large controllers, drives and amplifiers need, and it is the single highest-value
   missing symbol.

**Existing spec.** M4 §7 (symbol editor) already promises a user library and
`text_slots`; it does **not** cover per-instance pin naming or a parametric named-pin
block. Both should be added there.

---

### G2 — Wire attributes are stored but never drawn on the sheet · essential

**Needed.** Every conductor on the reference sheet carries two annotation lines:
gauge + colour + cut length above the wire, and the crimp contact / ferrule part with
the net name below it. This is the densest information layer on the drawing and the
main reason it is manufacturable.

**Current behaviour (verified).** `Wire` carries `color`, `sq`, `length_m`, `part_no`
and `net`. Only `color` (as stroke colour) and `net` (as the wire number, once per net
at the net's longest segment) are rendered. `sq`, `length_m` and `part_no` appear only
in the CSV/PDF reports. Stroke width is a constant regardless of gauge. There is no
field at all for the terminal/ferrule part at each wire end.

**Workaround used.** Two free text entities per wire, positioned by hand from the wire
geometry. ~140 extra entities on one sheet, and they do not move when the wire moves.

**Proposed.**
- A per-sheet or per-wire **annotation display setting**: which of
  `{gauge, colour, length, part no, wire no}` to print, on which side, at what size,
  auto-placed on the longest segment the way wire numbers already are. The renderer
  already solves the "where does the label go" problem for wire numbers; extend it.
- Add **end-terminal fields** to `Wire` (`from_terminal_part` / `to_terminal_part`, or
  a small per-end struct) so the crimp/ferrule annotation is data rather than text,
  and so it can flow into the wire list and the BOM.

**Existing spec.** M2 §2 covers wire numbers only. Not covered by M4. New.

---

### G3 — Net labels render as bare text, with no direction · essential

**Needed.** On the reference sheet, net labels are directional tag outlines — a flag
pointing away from the wire — so the reader instantly sees whether a signal leaves or
arrives at that point. They are also the visual anchor of the whole sheet, since it is
drawn as label-to-label wiring rather than long routed conductors.

**Current behaviour (verified).** A net label draws its name as plain text offset
above the anchor point, in the same style as a free note. `NetLabel.rotation` is part
of the model but both the SVG exporter and the canvas renderer ignore it, so a label
looks identical at 0° and 180°. There is no outline. The **logic** underneath is
correct and good: same-named labels merge distant nets into one, and a label whose
twin lives on another sheet gets a `/sheet.zone` cross-reference beside it.

**Workaround used.** None available — accepted as-is. The exported sheet reads as
loose text scattered along the wires.

**Proposed.** Draw the label as a tag/flag outline whose point follows the attachment
direction (derived from the attached wire's direction, with `rotation` as the manual
override). Offer at least the two conventional shapes (directional flag for
sheet-crossing signals, plain box for local labels). Honour `rotation` in both
renderers.

**Existing spec.** M2 §4 covers the cross-reference text but not the label glyph. New.

---

### G4 — Text has no anchor, colour, rotation, or box · essential (for this drawing)

**Needed.** The reference sheet uses colour to separate information layers (function
names, wire specs, net names, part types are each their own colour), right-aligned
text against the devices on the right-hand side, rotated labels for pin groups, and a
third text line per device for its function name.

**Current behaviour (verified).** `TextEntity` has only `at`, `text`, `height`,
`rotation`. Colour is hard-coded black in both renderers; `text-anchor` is hard-coded
`start`; `rotation` is ignored; there is no multi-line handling, no bounding box and
no fill. Symbol instances render exactly two text lines (`reference`, `value`), both
centred above the outline, with no way to move or add one.

**Consequence, measured.** On the reproduction the annotation is legible but visibly
collides: a device's function name overlaps its designator, and a wire's spec line
overlaps the net label beside it. Every position is absolute and hand-computed, so any
later edit desynchronises the annotation from the geometry.

**Workaround used.** Manual coordinate arithmetic, iterated twice. Still overlapping.

**Proposed.**
- Extend `TextEntity` with `color`, `align` (start/middle/end), honoured `rotation`,
  and multi-line support.
- Add a third instance text field (function/description) and make the renderers use
  `SymbolDef.text_slots` instead of hard-coded offsets, so slot positions become a
  property of the symbol.
- Consider a lightweight **leader/attachment** so a note can be anchored to an entity
  and move with it.

**Existing spec.** `text_slots` exist in M4 §7 but are not wired to the renderers.
Text styling is new.

---

### G5 — Jumper bars are invisible on the schematic · quality

**Needed.** The reference sheet annotates each terminal block with its jumpering
scheme, because that is what the panel builder wires.

**Current behaviour (verified).** `attrs["jumpers"] = "1-2,2-3,…"` is parsed,
validated, and rendered in the terminal chart and the terminal diagram report — all of
which work well. Nothing is drawn on the schematic sheet itself.

**Workaround used.** A free text note next to the block describing the jumpering.

**Proposed.** Draw the jumper links on the terminal block symbol in the schematic
(a short bar spanning the jumpered terminals), reusing the terminal-diagram renderer's
existing jumper drawing.

**Existing spec.** M4 §1. Extend it to the schematic view.

---

### G6 — A terminal block is one column of terminals · quality

**Needed.** The reference uses multi-level terminal blocks: one physical device
presenting two terminal columns (positive rail and negative rail) side by side, with
independent numbering per column.

**Current behaviour (verified).** `terminal_block_{n}p` is a single vertical column of
through terminals, 1…n, up to 50.

**Workaround used.** Two separate terminal-block symbols placed side by side with
different designators. They then appear as two devices in the terminal chart and BOM,
which is wrong for procurement.

**Proposed.** A `levels` / `columns` parameter on the terminal-block family (or an
`attrs` key), with per-level terminal numbering, carried through the terminal chart,
the terminal diagram and the BOM as **one** device.

**Existing spec.** M4 §1 covers the chart but assumes single-level blocks. Extend.

---

### G7 — A relay is modelled as separate coil and contact symbols · quality

**Needed.** The reference draws the relay as one physical block: coil plus both
changeover contacts inside a single outline, with the manufacturer's pin numbers and
the mechanical linkage dashed between the poles.

**Current behaviour (verified).** `relay_coil` + `relay_contact_co` are placed
separately and linked by sharing a designator. The coil↔contact cross-reference works
and is genuinely useful. But there is no single-outline multi-pole relay, and contact
pins are numbered 1/2/3 rather than by the manufacturer's scheme.

**Workaround used.** Three symbols sharing a designator prefix. Acceptable
schematically; loses the physical pin numbering the panel builder needs.

**Proposed.** Either a parametric relay family (`relay_{n}co`, one outline, mechanical
linkage drawn, per-pole pin numbers), or resolve this via G1's per-instance pin naming
plus a device-block symbol. G1 is the cheaper route.

**Existing spec.** M4 §4 covers the xref. The physical-device rendering is new.

---

### G8 — No pin-group brackets or rotated group labels · quality

**Needed.** Large device blocks on the reference group their pins visually (a bracket
spanning the group with a rotated caption alongside).

**Current behaviour (verified).** No bracket primitive on an instance, and rotated
text is ignored (see G4).

**Proposed.** Fold into the device-block symbol family (G1): a group is a pin range
plus a caption, and the symbol draws the bracket.

---

### G9 — Wire part numbers are never resolved from colour + gauge · quality

**Needed.** The target-quality reference manages wire stock as a colour+gauge →
part-number table printed on the drawing, and every conductor draws its part number
from it.

**Current behaviour (verified).** `Project.wire_parts` (`part_no`, `color`, `sq`,
`note`) is stored, round-tripped through save/load, and settable via `set_wire_parts`,
but **nothing consumes it**. No lookup resolves a wire's `color` + `sq` to a part
number; `Wire.part_no` must be filled in per wire by hand, and the wire-list report's
part-number column stays empty otherwise. There is no way to print the wire-part table
on a sheet.

**Workaround used.** Left empty — filling ~70 wires by hand was not worthwhile.

**Proposed.**
1. Resolve `part_no` from `wire_parts` by `(color, sq)` at report time when the wire
   has no explicit part number, and surface unmatched combinations as a diagnostic.
2. A **wire-part legend block** that can be placed on a sheet (the reference prints
   exactly this list in a corner).
3. Roll wire length per part number into the BOM.

**Existing spec.** M4 §5 (reports). New sub-item.

---

### G10 — No graphics-only entity (rectangle, line, fill) · quality

**Needed.** Highlight boxes around a region, filled device blocks, table-like note
frames, a title-block logo.

**Current behaviour (verified).** Six entity kinds exist: symbol, wire, junction,
net_label, text, harness. Anything rectangular is either a `harness` (dashed, named,
and it participates in harness reports) or four `wire` entities (which the netlist and
ERC then treat as conductors). `Primitive::Rect { filled }` exists inside `SymbolDef`
but there is no path to author one.

**Proposed.** A `shape` entity — line/rect/circle/polyline with stroke colour, width,
dash pattern and optional fill — explicitly excluded from the netlist and ERC. This is
a small addition that unblocks G10, G11 and part of G4.

**Existing spec.** New.

---

### G11 — Enclosure boundaries have to borrow the harness entity · quality

**Needed.** The reference wraps the in-case portion of the circuit in a dashed
rectangle captioned with the enclosure type — a mechanical boundary, not a harness.

**Current behaviour (verified).** Only `harness` draws a dashed captioned rectangle.
Using it for an enclosure works visually, but every wire inside it is then reported as
belonging to that "harness": the terminal chart and wire list both attributed the
enclosure name to unrelated conductors.

**Proposed.** Either the generic `shape` entity from G10, or a first-class
`enclosure` / `location` boundary with its own semantics (which would also feed the
"installation location" concept that panel layout and FreeCAD linking will need).

**Existing spec.** M2 §3 (harness). New sibling concept; note the overlap with M4 §8
(2D panel layout) and M5.

---

### G12 — Pictorial device symbols are missing, with no user library · quality

**Needed.** The reference and the target-quality drawings both use pictorial symbols
for field devices (fans, speakers, display modules, sensors) so the sheet can be read
without a legend.

**Current behaviour (verified).** 47 bundled symbols, none pictorial beyond
motor/lamp/bell. **Unknown symbol ids are accepted silently** — the entity is stored
and simply does not draw, with no error and no ERC diagnostic (confirmed by posting a
non-existent symbol id and an out-of-range pin count; both returned 200 and rendered
nothing).

**Proposed.** The user symbol library already specified in M4 §7, plus **validation**:
reject or diagnose an unresolvable `symbol_id` at command time rather than failing
silently.

**Existing spec.** M4 §7. Add the validation requirement.

---

### G13 — No device-level cross-sheet reference · quality

**Needed.** A device that continues on another sheet should say so on both sheets.

**Current behaviour (verified).** Cross-sheet reference is **net-label based** only:
a label whose name also appears on another sheet gets that sheet's zone address. There
is no reference from a device to its continuation, and no "sheet N of M for this
device" annotation.

**Proposed.** Extend the cross-reference engine from nets to devices, keyed on the
designator, reusing the existing `/sheet.zone` address formatting.

**Existing spec.** M2 §4 and M4 §4. Extend both.

---

### G14 — No mating-connector-pair concept · minor for this sheet, essential for harness work

**Needed.** Field connectors come in mating pairs; a harness drawing wants to state
which plug mates with which socket, and reports want to list a mated pair once.

**Current behaviour (verified).** `connector_plug` and `connector_socket` are two
static one-pin glyphs that face each other. Nothing links two connector instances as
a pair. Parametric connectors have no gender at all.

**Proposed.** A `mates_with` link between two connector instances (designator
reference), gender on the parametric connector family, and pair-aware wire-list and
BOM output.

**Existing spec.** New. Overlaps M5 (the mechanical side cares about mated pairs too).

---

### G15 — Sheet paper size cannot be changed after creation · minor

`add_sheet` takes `size` and `orientation`; no command changes them afterwards. To get
an A4-portrait sheet the reproduction had to add a new sheet and delete the default
one, losing anything already on it. Add `set_sheet_size` (or fold size/orientation
into a `set_sheet_properties` command).

---

### G16 — No command sets the project name or number · minor

`Project.name` is only set at creation; reports and the title block therefore printed
the placeholder project name throughout. `{project.no}` is a title-block bind that is
always empty because nothing populates it. Add a `set_project_meta` command.

---

### G17 — Silent failures give no feedback · minor

Three cases confirmed during the reproduction, all returning success and drawing
nothing:
- an unresolvable `symbol_id` (see G12);
- a parametric symbol beyond its pin limit;
- a net label that is not within 0.01 mm of a wire vertex, which silently fails to
  join the net (this cost one full rebuild — the first attempt placed labels 0.5 mm
  off and produced 69 disconnected single-pin nets instead of 48 correct ones).

At minimum the last one deserves an ERC diagnostic ("net label not attached to any
wire"), and the first two an error at command time.

---

### G18 — The From-To wire list degrades when one end is a net label · minor

Where a conductor ends at a net label rather than a device pin, the wire list prints
the net name in the From column instead of a device:pin pair. On a sheet drawn
label-to-label — which the reference is, throughout — roughly half of every row is a
net name. Resolving through the label to the device on the other side would make the
report directly usable for wiring.

---

## Summary by impact

| Impact | Gaps |
|---|---|
| **essential** | G1 named pins on multi-pin blocks · G2 wire annotation on the sheet · G3 directional net-label flags · G4 text styling, anchoring and a third symbol text field |
| **quality** | G5 jumper bars on the schematic · G6 multi-level terminal blocks · G7 physical relay blocks · G8 pin-group brackets · G9 wire part number resolution + legend · G10 graphics-only entity · G11 enclosure boundary · G12 pictorial symbols + user library · G13 device-level cross-sheet reference |
| **minor** | G14 mating connector pairs · G15 sheet size command · G16 project meta command · G17 silent failures · G18 From-To list through net labels |

## Reading of the result

The engine, the electrical model and the reporting are **already ahead of the drawing
being replaced** — the reference tool produces no wire list, no terminal chart and no
cross-reference report, and MadakeCAD produced all three from the reproduction without
extra work. Nothing in the reproduction was blocked by the command architecture: every
element went in through `Command`, and undo/redo and patch delivery held throughout.

What separates the exported sheet from the reference is presentation density. An
industrial harness drawing is roughly half annotation by area, and MadakeCAD currently
treats annotation as unstyled, unanchored free text. **G1–G4 are the whole gap**: fix
named pins, wire annotation, net-label glyphs and text styling, and the reproduction
would be at parity with the drawing it replaces. The rest is polish toward the
target-quality standard.
