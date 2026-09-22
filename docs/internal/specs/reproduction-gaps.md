# Reproduction gap ledger — reference wiring diagram, sheet 1

Created: 2026-08-23. Second pass: 2026-08-23 (after phase 5). Third pass: 2026-08-23 (after phase 6).
Japanese version: [reproduction-gaps.ja.md](reproduction-gaps.ja.md).

## What this is

An attempt to redraw sheet 1 of the project's origin reference drawing (a 5-sheet
A4-portrait harness/wiring diagram, currently produced in another EDA tool) using
**only the features MadakeCAD ships today**, and a ledger of everything that got in
the way. This was the input to the phase-5 feature design, and it is re-measured
after each phase that touches drawing output.

The reproduction was built deterministically through the Link API
(`POST /api/v1/commands`), not through the AI chat, so every verdict below is the
result of actually issuing the command and looking at the exported PDF — not a guess.
The reproduction data, the exported PDF/SVG, the reports and the side-by-side crops
live outside version control (the reference material is confidential).

The bar is **"does the same information land on the sheet"**, not pixel parity.

## Result at a glance — first pass (phase 4 features)

313 commands produced a complete A4-portrait sheet: JIS frame with zone numbers,
title block, three revision rows, a dashed enclosure, 20 connectors, two terminal
blocks, a relay, a controller block, ~70 annotated wires, ~60 net labels and notes.
It exports to SVG/PDF and to a 14-page book with wire list, terminal chart, terminal
diagram, BOM and cross-reference report. ERC on the finished sheet reports 11
diagnostics, all of them real (genuinely unconnected pins).

So the **structure and the electrical model are there**. What is missing is almost
entirely in the **annotation and symbol layers** — the parts that make an industrial
drawing readable on paper.

## Second pass — after phase 5 (2026-08-23)

Phase 5 implemented the four essential gaps (G1 named-pin device blocks, G2 wire
annotation on the sheet, G3 directional net-label tags, G4 text styling and a third
symbol text field) plus two foundation fixes (G17 silent failures, G9 wire part number
resolution). The reproduction script was rewritten to use them and the sheet was
rebuilt from scratch through the same Link API path.

258 commands produced the same sheet with **no hand-placed annotation at all**: every
one of the 111 conductors carries its gauge/colour/length note automatically, every
net label is a directional flag, the controller and the amplifier are single
named-pin blocks, and each device carries a function label. ERC + electrical
verification report **3 diagnostics** (down from 11), and **`erc.dangling_label` is
zero** — the silent failure that cost a full rebuild on the first pass cannot recur
undetected. The wire list's part-number column is filled on 111 of 111 rows from a
three-row colour+gauge table imported as CSV.

**All four essential gaps are closed.** What is left is quality and polish, plus
three new findings recorded below as G19–G21.

### By element type

Verdicts are "first pass" (phase 4 features) → "second pass" (phase 5 features).

| Element | 1st | 2nd | Note |
|---|---|---|---|
| A4 portrait JIS frame, zone numbers | ✅ | ✅ | `jis-a4-portrait`, 4 cols x 6 rows |
| Title block | 🔶 | 🔶 | G16 — still no project name/number, no logo |
| Revision table | ✅ | ✅ | mark/date/description/by, latest revision binds into the title block |
| 2-pole / 4-pole connectors with designator + type | ✅ | ✅ | |
| Connector facing left (load side) | ✅ | ✅ | a block chooses the edge its pins sit on, so no mirror is needed |
| Named connector pins (e.g. `P`/`N`) | ❌ | **✅** | G1 — `block` + `block_pins` |
| Function label above a device (a third text field) | 🔶 | **✅** | G4 — `attrs.function_label`; it prints *below* the outline, the reference prints it above |
| Terminal blocks, through terminals | ✅ | ✅ | `terminal_block_{n}p` |
| Terminal block with two terminal columns as one device | 🔶 | **✅** | G6 — `terminal_block_{n}p_{levels}l`, one column per level, one device |
| Jumper bars on the schematic | 🔶 | **✅** | G5 — drawn on the symbol as a shorting bar, level-aware on a multi-level block |
| Multi-pin controller block with arbitrary pin names | ❌ | **✅** | G1 — 14 + 14 named pins, names inside the outline, numbers outside |
| Pin-group brackets with rotated group labels | ❌ | ❌ | G8 |
| Filled / shaded device blocks | ❌ | **✅** | G10 — `shape` with a fill token |
| Relay: coil + two changeover contacts | 🔶 | 🔶 | G7; and see G21 — changeover contacts are invisible to the relay cross-reference. **✅ phase 6 task 3**: `attrs.device_block` draws the coil as one outline with the contact image; manufacturer pin numbers and the mechanical linkage are still not drawn |
| Net labels merging nets by name | ✅ | ✅ | |
| Cross-sheet reference on net labels | ✅ | ✅ | renders the `/sheet.zone` address of the twin label |
| Net label drawn as a directional flag tag | ❌ | **✅** | G3 — pentagon flag; see G19 for the anchor caveat |
| Wire numbers | ✅ | ✅ | drawn once per net at the longest segment |
| Per-wire gauge / colour / length annotation on the sheet | ❌ | **✅** | G2 — automatic on all 111 conductors |
| Per-wire-end crimp contact / ferrule part annotation | ❌ | ❌ | rest of G2 — `Wire` still has no per-end terminal field |
| Wire colour | ✅ | ✅ | English and Japanese colour names both resolve |
| Wire part number from colour + gauge | ❌ | **✅** | G9 — 111/111 wire-list rows filled from an imported CSV |
| Harness boundary (dashed enclosure) | ✅ | ✅ | |
| Enclosure / case boundary distinct from a harness | 🔶 | **✅** | G11 — draw it as a dashed `shape`; the harness column stays empty |
| Highlight box (coloured rectangle annotation) | ❌ | **✅** | G10 — `shape` rectangle with a colour token |
| Pictorial device symbols (fan, speaker, display module) | ❌ | 🔶 | G12 — M4 shipped the user symbol library and editor, so these can be authored; none are bundled |
| Free notes | 🔶 | 🔶 | G4 gave colour, weight and anchor; rotation and multi-line are still missing |
| Junctions | ✅ | ✅ | |
| Device-level cross-sheet block reference | ❌ | ❌ | G13. **✅ phase 6 task 3**: any designator spanning sheets gets the `/sheet.zone` note automatically, relay coils/contacts excluded |
| Mating connector pairs | ❌ | ❌ | G14 |
| Reports (wire list, terminal chart/diagram, BOM, xref) | ✅ | ✅ | 17-page book |
| ERC / electrical verification | ✅ | ✅ | 3 diagnostics |

### Rejection of malformed commands (G17) — measured

Every case that used to return 200 and draw nothing now returns HTTP 400 with a
reason, `failed_command_index` and `applied: 0` — nothing is written:

| Posted | Response |
|---|---|
| unresolvable `symbol_id` | 400 `unknown symbol_id: "…" (no such symbol in the library)` |
| parametric symbol beyond its pin limit | 400 `unknown symbol_id: … (pin count out of range)` |
| raw colour code on a note | 400 `unknown text color token: … (expected one of default, accent, warning, success, muted)` |
| block with a duplicate pin number | 400 `duplicate block pin number: 1` |

---

## Third pass — after phase 6 (2026-08-23)

Phase 6 closed the three readability findings of the second pass (G19 tag anchor, G20
annotation text height, G21 changeover contacts). The reproduction script was re-run
through the same Link API path, with the sheet's annotation text height set to **1.4 mm**
— the size the reference drawing uses — and with three label anchors moved, because a tag
body now grows to the *left* of its anchor where it used to grow to the right.

**260 commands** produce the same sheet. Measured on it:

| Measure | 2nd pass | 3rd pass |
|---|---|---|
| Overlapping labels on sheet 1 (`tidy-metrics`) | — | **0** (12 for the same drawing at the default 2.0 mm) |
| ERC + electrical verification | 3 | **2** (the false `erc.coil_without_contact` on the relay is gone) |
| Wire-list rows with a part number | 111 / 111 | 111 / 111 |
| Book pages | 17 | 17 |

The two remaining diagnostics are both about the same relay and both genuine
(`elec.no_current_attr` info, `elec.unreachable_load` warning). The `right` tags of the
terminal blocks, the controller and the right-hand connector column now stand clear of
their own conductors, and at 1.4 mm a wire note clears the tag of the row below by
0.1 mm on the 5 mm pin pitch — crowded on purpose, exactly as on the reference sheet.
One new overlap was found in the process and is recorded as G22.

---

## Gaps

Impact levels: **essential** = the reference drawing cannot be reproduced faithfully
without it · **quality** = reproducible but visibly below the target standard ·
**minor** = friction, not a blocker.

### G1 — Multi-pin device blocks cannot have named pins · essential

> **✅ Resolved 2026-08-23 (phase 5, task 1–2).** A `block` symbol whose shape lives in
> `SymbolInstance.attrs` (`block_pins` = per-edge lists of `{no, name}`, optional
> `block_size`) is generated into a `SymbolDef` on every use, so a device block is
> defined per instance without growing the library. Pin names print inside the outline,
> pin numbers outside on the lead, and the netlist, ERC, BOM, terminal chart and both
> renderers use the same generated definition. Reproduction: the controller (14 + 14
> named pins), the amplifier and every 2-/4-pole connector (`P`/`N`) are blocks.
> **Not covered:** per-instance pin naming for the *parametric* families
> (`connector_{n}p` etc.) — those still number their pins 1…n. Use a block instead.
> Pin-group brackets stay open as G8.

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

> **✅ Resolved for gauge / colour / length 2026-08-23 (phase 5, task 3).** Each wire
> gets an automatic `{sq}sq {colour} {length}m` note placed on its longest segment,
> opposite the wire number (below a horizontal wire, right of a vertical one). Colour
> names are translated for print; missing attributes are simply left out. It is its own
> display class on screen and is **always** exported to SVG/PDF because it is the
> drawing's primary information. Reproduction: all 111 conductors annotated, with no
> hand-placed text.
> **Still open:** (a) the per-wire-end crimp/ferrule part — `Wire` has no per-end
> terminal field, so the reference's second annotation line cannot be produced;
> (b) note size and placement are fixed — see G20.

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

> **✅ Resolved 2026-08-23 (phase 5, task 3).** `NetLabel.attrs["tag"] = "left" | "right"`
> draws the label inside a five-cornered flag whose point follows that direction; the
> frame width follows the text width so long net names still fit. Untagged labels are
> byte-identical to before, so existing drawings do not change. SVG and canvas share
> one formula, and the cross-sheet reference is pushed clear of the glyph.
> **Still open:** `NetLabel.rotation` is still ignored, and the anchor convention makes
> `right` tags overlap their own wire — see G19.

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

> **✅ Resolved for colour / anchor / weight / third text field 2026-08-23 (phase 5,
> task 4).** `TextEntity` gained `anchor` (start/middle/end), `color` and `bold`
> (`format_version` 5, old files open unchanged and an undecorated note exports
> byte-identically). Colour is a **semantic token only** (`default` / `accent` /
> `warning` / `success` / `muted`) so screen and paper can each pick a legible colour;
> raw colour codes are rejected by the command engine. Symbols gained a third text
> field, `attrs.function_label`, placed by the definition's `FUNCTION` text slot or, if
> there is none, under the outline.
> **Still open:** `TextEntity.rotation` is still not rendered, there is no multi-line
> note, no bounding box/fill, and no leader anchoring a note to an entity. The function
> label prints below the outline whereas the reference prints it above, and `block`
> has no `FUNCTION` slot to move it.

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

> **Status 2026-08-23 (phase 6, task 2): resolved.** `attrs["jumpers"]` is now
> drawn on the schematic symbol itself (SVG export and the canvas renderer), as a
> thick bar filling the gap between the two jumpered terminals' marks, reusing the
> same `parse_jumpers` validation the terminal chart and terminal diagram use
> (non-adjacent or unknown terminal numbers draw nothing). Still also reported in
> the terminal chart and terminal diagram, unchanged.
>
> The bar's shape follows the real saddle jumper, after
> [EPLAN "Using Jumpers"](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2025/Content/htm/terminalgui_k_verwendungbruecken.htm)
> (a saddle jumper sits in a duct in the middle of the terminal, joins neighbouring
> terminals, and uses saddle-jumper connection points separate from the wire connection
> points): the bar lies **on the axis joining the two neighbouring terminal centres and
> only in the gap between their round marks**, drawn at 0.8 mm — more than twice an
> ordinary symbol line — so that it reads as a shorting bar in print. It carries **no end
> marks of its own**: at a 5 mm terminal pitch the gap between two marks is only 1.4 mm,
> which cannot hold a thick bar and two distinguishable end dots without covering the
> terminal numbers, and the edges of the two round marks already show where the bar starts
> and ends. It therefore never crosses a terminal
> number, never sits on a wire connection point of a column, and always reads as joining
> the pair it actually joins. `terminals_are_adjacent` limits a jumper to two neighbours,
> so a bar never passes over a third terminal.
>
> On a **multi-level** block (G6) the jumper spec is level-aware: `1a-1b` links the
> two levels of one pole, `1a-2a` two neighbouring poles of one level, and a
> diagonal pair (`1a-2b`) is not adjacent. A plain numeric spec (`1-2`) on a
> multi-level block is refused with the `terminal.jumper_invalid` diagnostic, and a
> reference to a terminal the block does not have with
> `terminal.jumper_unknown_terminal`, so the chart rows, the schematic bar and the
> ERC can never disagree. Single-level blocks keep plain numeric specs.
>
> **Status 2026-08-23 (phase 5): unchanged.** Still reports only; nothing is drawn
> on the schematic sheet.

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

> **Status 2026-08-23 (phase 6, task 2): resolved (dynamic symbol id).** A new
> parametric symbol family, `terminal_block_{n}p_{levels}l` (1–3 levels, e.g.
> `terminal_block_8p_2l`), draws **one terminal column per level, side by side**
> (2 levels = 2 columns), with the poles as rows, so the same pole of every level
> sits on one row. Column centres are 10 mm apart, which keeps every connection
> point on the 2.5 mm grid, leaves adjacent columns' inner points 5 mm apart (no
> two terminals share a point) and leaves room for a wire between the columns.
> Each level's terminals are numbered with a level letter (`1a`/`1b`/…), so levels
> never short together (independent nets). The terminal chart carries a level
> (`Lv`) column, the terminal diagram splits the terminal strip into one column
> per level (headed by the level letter), and the whole thing is still **one**
> symbol instance, so it is one device in the terminal chart, the terminal
> diagram and the BOM. No `.mdkproj` format change: it is a dynamic symbol id
> plus the existing `attrs`, and the plain `terminal_block_{n}p` id (1 level,
> chosen by default in the part-insert dialog) produces exactly the same geometry
> and pin numbering as before, so existing projects stay valid. The terminal
> editor grid does not yet show the `Lv` column or let a level be added after the
> fact — the level count is chosen once, at insertion.
>
> **Status 2026-08-23 (phase 5): unchanged.** The two-column block was again
> approximated with two `terminal_block_{n}p` symbols and two designators.

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

> **Status 2026-08-23 (phase 5): unchanged, and a new defect found.** G1's device
> block is now available as the cheaper route proposed below, but no relay was
> rebuilt on it. See **G21**: the bundled changeover contact is not recognised by the
> relay cross-reference at all.

> **✅ Resolved 2026-09-22 (phase 6, task 3).** Not a new symbol family: a **display
> mode** on the existing coil. `relay_coil` gained an instance attribute
> `attrs["device_block"]`; when it is `"true"` the coil is drawn as one outline
> carrying the designator, the part number (the instance's `value`, the same field the
> BOM already reads) and a **contact image** — one line per contact sharing the
> designator, with that contact's IEC 60947-1 terminal pair and its `/sheet.zone`
> address (the same content the coil ⇔ contact contact map already carries, now
> merged into the block instead of hung below it as a separate table). The default
> (`device_block` unset) keeps the distributed placement unchanged. The contacts stay
> individually placed and wired either way, and the coil's own A1/A2 pins do not move:
> the outline is never narrower than the pin span, and content that needs more room
> only stretches a horizontal lead from pin to outline edge, so toggling the checkbox
> never breaks a wire. Geometry is one shared helper
> (`relay_xref::device_block_layout` / `relayXref.ts` `deviceBlockLayout`) drawn by
> both `svg.rs::render_device_block` and `renderer.ts::drawDeviceBlock`, with a parity
> test on both sides. The properties panel gained a checkbox ("実機ブロックで表示" /
> "Show as physical block") on relay coils, writing the attribute through the existing
> `update_entity` command. No manufacturer per-pole numbering scheme and no mechanical
> linkage drawing were added — those remain open (see **Proposed**, second half).

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

**Still open (2026-09-22).** The manufacturer's own per-pole pin numbering scheme (the
block still shows the IEC generic pairs `13-14`/`21-22`/…, not a vendor's silkscreen
numbers) and the dashed mechanical linkage between poles were out of scope for this
task and are not implemented.

**Existing spec.** M4 §4 covers the xref. The physical-device rendering is
`relay_xref::device_block_layout` (task 3, phase 6).

---

### G8 — No pin-group brackets or rotated group labels · quality

> **Status 2026-08-23 (phase 5): unchanged.** The `block` family from G1 landed
> without pin groups, so this is now a well-defined extension of `block_pins`
> (a group = a pin range + a caption) rather than a new symbol family.

**Needed.** Large device blocks on the reference group their pins visually (a bracket
spanning the group with a rotated caption alongside).

**Current behaviour (verified).** No bracket primitive on an instance, and rotated
text is ignored (see G4).

**Proposed.** Fold into the device-block symbol family (G1): a group is a pin range
plus a caption, and the symbol draws the bracket.

---

### G9 — Wire part numbers are never resolved from colour + gauge · quality

> **✅ Resolved for the lookup 2026-08-23 (phase 5, task 5).** A wire's own
> `part_no` wins; if it is empty the wire's colour + gauge is looked up in
> `Project.wire_parts` (first matching row wins, blank if none), and the wire list and
> From-To reports follow. Colour matching ignores case and surrounding space and
> **treats the English and Japanese names as the same colour**, so a table written
> `青` resolves a wire drawn `blue`. The table can be imported from CSV
> (`線色,線径sq,電線品番[,備考]`) as a single undoable command.
> Reproduction: 111 of 111 wire-list rows filled from a three-row CSV.
> **Still open:** no wire-part legend block that can be placed on a sheet, no
> per-part-number length roll-up in the BOM, and no diagnostic for an unmatched
> colour+gauge combination.
>
> *(The phase-5 plan refers to this gap as "G11"; the ledger number is G9.)*

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

> **✅ Resolved 2026-08-23 (phase 6, task 1).** A `shape` entity ships: rectangle
> (two diagonal corners), line (two ends), circle (centre plus a point on the edge)
> and polyline (any number of points), each with a stroke colour **token**, a stroke
> width of 0.1–2.0 mm, an optional fill and a dashed flag. It is drawn behind the
> wires in both the canvas and the SVG/PDF export, has its own view class
> ("Graphics", the 12th), and is **excluded by construction** from the netlist, the
> ERC, the BOM, the From-To wire list and the tidy metrics (tests pin all five).
> Colours reuse the note colour tokens, so raw colour codes are refused by the
> command engine. File format 8; drawings without shapes open unchanged.
> **Still open:** no title-block logo image (that belongs to the frame template).

> **Status 2026-08-23 (phase 5): unchanged.** The entity kinds gained only the
> panel-layout ones (outline, DIN rail, duct, footprint, legend), which are restricted
> to panel sheets. A schematic still has no free rectangle, line or fill.

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

> **✅ Resolved 2026-08-23 (phase 6, task 1) — by a change of practice, not a new
> entity.** With the G10 `shape` entity in place, **an enclosure is drawn as a dashed
> rectangle shape with a free text caption**, and the harness boundary goes back to
> being what it was designed for: the group of wires that are made and bought as one
> bundle. A shape has no bearing on wire grouping, so the harness column of the wire
> list and the terminal chart stays empty for conductors that merely sit inside an
> enclosure. This is written into the drawing-notation section of
> `docs/internal/design-system.md` and into the harness tool's own help text; no code
> change was needed on the harness side. The re-measurement of the reproduction
> belongs to phase 6 task 6.

> **Status 2026-08-23 (phase 5): unchanged.** Measured again: all 111 wire-list rows
> report the enclosure name in the harness column.

**Needed.** The reference wraps the in-case portion of the circuit in a dashed
rectangle captioned with the enclosure type — a mechanical boundary, not a harness.

**Current behaviour (verified).** Only `harness` draws a dashed captioned rectangle.
Using it for an enclosure works visually, but every wire inside it is then reported as
belonging to that "harness": the terminal chart and wire list both attributed the
enclosure name to unrelated conductors.

**Proposed (adopted: the first option).** Either the generic `shape` entity from G10, or a first-class
`enclosure` / `location` boundary with its own semantics (which would also feed the
"installation location" concept that panel layout and FreeCAD linking will need).

**Existing spec.** M2 §3 (harness). New sibling concept; note the overlap with M4 §8
(2D panel layout) and M5.

---

### G12 — Pictorial device symbols are missing, with no user library · quality

> **Status 2026-08-23 (phase 5): partly resolved — downgraded to 🔶.** M4 §7 shipped
> the user symbol library (`~/MadakeCAD/symbols/*.json`) and the symbol editor, so a
> pictorial symbol can now be authored without touching the build, and phase 5's G17
> work made an unresolvable `symbol_id` a hard command error instead of a silent
> no-op. What remains is that **no pictorial symbols are bundled**.

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

> **Status 2026-08-23 (phase 5): unchanged.** The only device-level cross-reference
> is the relay coil↔contact link from M4 §4.

> **✅ Resolved 2026-09-22 (phase 6, task 3).** The net-label cross-reference engine
> (`xref.rs`) gained a device-keyed twin: `device_sites`/`sheet_device_xrefs` group
> every symbol instance by **reference designator** the same way `label_sites` groups
> net labels by name, and produce the identical `/sheet.zone` text
> (`device_xref_at`/`deviceXrefAt` place it beside the designator, one space between
> several addresses, exactly as `xref_text_at` already does for net labels). Any
> designator drawn on two or more sheets now gets the note on every instance it has,
> automatically, with no new command needed. Relay coils and contacts are excluded
> (`relay_xref::relay_role(...).is_some()`) so the two mechanisms never draw on top of
> each other — a relay keeps only its coil ⇔ contact reference. No new view class was
> added: the annotation reuses the existing "reference designators" (`refs`) class in
> the canvas renderer and the SVG/PDF export (unconditional, per the existing
> screen-vs-output split). Benchmark: EPLAN's cross-reference types distinguish a
> "device reference" from a "coil/contact reference"
> (<https://www.eplan.help/en-us/Infoportal/Content/Plattform/2022/Content/htm/xessettingsgui_k_querverweisarten.htm>);
> this closes the gap between the two the same way, without adding EPLAN's full
> configurable type system.

**Needed.** A device that continues on another sheet should say so on both sheets.

**Current behaviour (verified).** Cross-sheet reference is **net-label based** only:
a label whose name also appears on another sheet gets that sheet's zone address. There
is no reference from a device to its continuation, and no "sheet N of M for this
device" annotation.

**Proposed.** Extend the cross-reference engine from nets to devices, keyed on the
designator, reusing the existing `/sheet.zone` address formatting.

**Existing spec.** M2 §4 and M4 §4, extended by `xref::sheet_device_xrefs` (task 3,
phase 6).

---

### G14 — No mating-connector-pair concept · minor for this sheet, essential for harness work

> **Status 2026-08-23 (phase 5): unchanged.**

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

> **Status 2026-08-23 (phase 5): unchanged.** The second pass again had to add a new
> A4-portrait sheet and delete the default one.

`add_sheet` takes `size` and `orientation`; no command changes them afterwards. To get
an A4-portrait sheet the reproduction had to add a new sheet and delete the default
one, losing anything already on it. Add `set_sheet_size` (or fold size/orientation
into a `set_sheet_properties` command).

---

### G16 — No command sets the project name or number · minor

> **Status 2026-08-23 (phase 5): unchanged.** The reports still print the placeholder
> project name.

`Project.name` is only set at creation; reports and the title block therefore printed
the placeholder project name throughout. `{project.no}` is a title-block bind that is
always empty because nothing populates it. Add a `set_project_meta` command.

---

### G17 — Silent failures give no feedback · minor

> **✅ Resolved 2026-08-23 (phase 5, task 5).** All three cases are fixed and were
> re-measured on the second pass:
> - a net label that is not on a wire segment or a symbol pin is an ERC Warning
>   `erc.dangling_label`; the netlist and the ERC now use the **same** attachment test,
>   and a label placed directly on a pin joins that net (it used to need a wire);
> - an unresolvable `symbol_id` — or a parametric symbol beyond its pin limit, which
>   says so — is rejected by `Engine::execute` as an invalid command;
> - `POST /api/v1/commands` answers 400 with `{error, failed_command_index, applied}`,
>   and the MCP tool reports which command in the batch was rejected and why.
>
> The rebuilt sheet reports **zero** `erc.dangling_label`, and all four malformed
> commands listed in the table above are refused without writing anything.

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

> **Status 2026-08-23 (phase 5): unchanged.** Measured again: the From column of all
> 111 wire-list rows is a net name.

Where a conductor ends at a net label rather than a device pin, the wire list prints
the net name in the From column instead of a device:pin pair. On a sheet drawn
label-to-label — which the reference is, throughout — roughly half of every row is a
net name. Resolving through the label to the device on the other side would make the
report directly usable for wiring.

---

### G19 — A directional net tag always grows to the right of its anchor · quality

> **✅ Resolved 2026-08-23 (phase 6).** The anchor is now the **root of the point**, and
> the flag body grows away from the point: a `right` tag puts its body to the left of the
> label position with the point sticking out to the right, and a `left` tag is its exact
> mirror image. Both directions keep the same outline size, and the cross-sheet reference
> is pushed out on the **body** side (a `right` tag prints it to the left, right-aligned).
> Since a net label must sit on a wire vertex, no flag can land on its own conductor or on
> the wire note any more. SVG, canvas and the tidy metrics share one formula.
> **Compatibility:** this moves every existing tagged label by up to the width of its flag.
> That break is accepted because the tag glyph shipped in phase 5 and no production drawing
> uses it yet; the reproduction script needed two label anchors moved for the new direction.

**Found on the second pass (2026-08-23).**

**Current behaviour (verified).** Both tag directions lay the flag body on the **+x**
side of the label's anchor point: `left` puts its point at the anchor and the body to
the right of it, `right` puts the body at the anchor and the point beyond it. Because a
net label must sit exactly on a wire vertex to join the net, the anchor is always an
end of the wire. That makes `left` correct — the point touches the conductor and the
body runs off into free space — but `right` lays the body over its own conductor and
over the wire note that G2 centres on the same segment.

**Consequence, measured.** Every wire that leaves a device leftwards (the terminal
blocks' left side, the controller's left side, the right-hand connector column) draws
its tag on top of itself. The left-hand connector column, whose wires leave rightwards
and end in `left` tags, is clean.

**Proposed.** Anchor a `right` tag at its **point**, so the body runs to the left, and
send its cross-sheet reference text left as well. Both renderers share the formula, so
this is one change in `net_tag.rs` plus the canvas twin.

---

### G20 — Annotation size and placement are not settable · quality

> **✅ Resolved for the text height 2026-08-23 (phase 6).** `Sheet.annotation_text_mm`
> (`format_version` 7, 1.0–3.5 mm, unset = the previous 2.0 mm) sets the size of the
> **wire notes, the wire numbers and the net tags** on that sheet, because the size of the
> annotation layer is a property of the drawing standard rather than of any one wire. The
> distance from the conductor and the tag's padding and point scale with the same factor,
> so the whole annotation layer shrinks together. It is set with the `set_sheet_annotation_text`
> command (out-of-range values are refused with a reason) and edited in the properties
> palette when nothing is selected. Measured on the reproduction: at 1.4 mm the sheet has
> **0 label overlaps**, against 12 for the same drawing at the default 2.0 mm.
> **Still open:** leader lines are still not a thing, and the reference / type / function
> labels still print at a fixed 2.5 mm — the setting deliberately does not cover them.

**Found on the second pass (2026-08-23).**

**Current behaviour (verified).** The automatic wire note is a fixed 2.0 mm high, on a
fixed side, one grid pitch away from the conductor. Where pins sit on the 5 mm pin
pitch — a terminal block, a device block, any parametric family — a note on one row
overlaps the tag of the row below by 0.5–2 mm.

**Consequence, measured.** The 18-pole terminal block, the 28-pin controller and the
6-pin amplifier are all annotated correctly but crowded. The reference drawing solves
exactly this with roughly 1.4 mm text laid out as two lines.

**Proposed.** Make annotation text height (and ideally leader placement) a per-sheet
setting, since it is a property of the drawing standard rather than of any one wire.
The same setting should govern the reference/type/function labels, whose fixed 2.5 mm
size is what pushed the reproduction's left-hand column past the title-block band.

---

### G21 — The changeover contact is invisible to the relay cross-reference · quality

> **✅ Resolved 2026-08-23 (phase 6).** `relay_contact_co` now has a relay role, so a relay
> built from changeover contacts is a proper device: the false `erc.coil_without_contact`
> is gone, the contact map is drawn under the coil, and the contact count is checked. A
> changeover pole carries **three** terminals — common = 1, break = 2, make = 4 (IEC 60947-5-1)
> — so its row reads `11-12-14` rather than a pair. A contact configuration may now say
> `2CO`, and **one changeover pole can serve as a make or as a break contact**: the check
> lends a spare changeover pole to a plain contact that has run out of dedicated poles, but
> never the other way round (a drawn changeover symbol needs a changeover pole).

**Found on the second pass (2026-08-23).**

**Current behaviour (verified).** `relay_xref::relay_role()` recognises `relay_coil`,
`relay_contact_no` and `relay_contact_nc`. The bundled `relay_contact_co` (changeover)
returns nothing, so a relay drawn with changeover contacts is grouped as a coil with no
contacts: `erc.coil_without_contact` is reported although the contacts exist and share
the coil's designator, the contact map under the coil is never drawn, and the
contact-count check against `contact_config` silently ignores them.

**Proposed.** Give the changeover contact a `RelayRole` and decide how it counts
against a contact configuration (one changeover pole is both an NO and an NC pole).

---

### G22 — The coil location on a contact collides with the wire note above it · minor

**Found on the third pass (2026-08-23).**

> **✅ Resolved 2026-09-22 (phase 6, task 3).** `relay_xref::coil_location_at` (and
> its canvas twin `coilLocationAt`) now try, in order, the right side (the previous
> fixed position), then above the outline, then below it, and use the first one that
> does not overlap a wire note on that sheet (`wire_note::wire_notes`; the overlap
> check approximates both texts as boxes at the fixed `CONTACT_MAP_FONT` height, the
> same simplification the rest of the relay cross-reference already makes — annotation
> text height (G20) is not applied to these two mm-scale labels). This is the same
> "print on whichever side is free" idea the wire note itself uses relative to the
> wire number, generalised from a fixed two-way choice to try three sides in turn. A
> test reproduces the exact changeover-contact geometry from this gap (a horizontal
> wire off the upper pin, whose note lands exactly on the old fixed position) and
> confirms the text moves off it, plus a second test confirming the fallback to
> "below" when both right and above are occupied.

**Current behaviour (verified).** The coil location (`(/1.A3)`) is printed to the right of
a contact symbol at the symbol's mid height. The wire note of the pin **above** that
midpoint is printed just under its conductor, at the same height. On a changeover contact,
whose two right-hand pins straddle the mid height by one grid pitch, the two annotations
land on each other whenever both pins run horizontally.

**Consequence, measured.** One spot on the reproduction sheet (the relay's changeover
contact). It was worse before phase 6 — the 2.0 mm note covered the location text
completely — and the smaller note only moves it off by a fraction.

**Proposed.** Give the coil location the same "opposite side" rule the wire note already
follows (print it above the outline, or offset it by half a row), or let it move to the
first free side. This is one placement rule in `relay_xref::coil_location_at` plus the
canvas twin.

---

## Summary by impact

| Impact | Gaps |
|---|---|
| **essential** | *(all closed in phase 5)* ✅ G1 named pins on multi-pin blocks · ✅ G2 wire annotation on the sheet · ✅ G3 directional net-label flags · ✅ G4 text styling, anchoring and a third symbol text field |
| **quality** | ✅ G5 jumper bars on the schematic · ✅ G6 multi-level terminal blocks · ✅ G7 physical relay blocks *(display mode + contact image; manufacturer pin numbering and mechanical linkage still open)* · G8 pin-group brackets · ✅ G9 wire part number resolution *(legend and BOM roll-up still open)* · ✅ G10 graphics-only entity · ✅ G11 enclosure boundary *(drawn with a shape; reproduction re-measured in task 6)* · 🔶 G12 pictorial symbols *(user library shipped; none bundled)* · ✅ G13 device-level cross-sheet reference · ✅ G19 net tag anchor side · ✅ G20 annotation text height *(leader placement and the 2.5 mm device labels still fixed)* · ✅ G21 changeover contact cross-reference |
| **minor** | G14 mating connector pairs · G15 sheet size command · G16 project meta command · ✅ G17 silent failures · G18 From-To list through net labels · ✅ G22 coil location vs wire note |

## Reading of the result

The engine, the electrical model and the reporting are **already ahead of the drawing
being replaced** — the reference tool produces no wire list, no terminal chart and no
cross-reference report, and MadakeCAD produced all three from the reproduction without
extra work. Nothing in the reproduction was blocked by the command architecture: every
element went in through `Command`, and undo/redo and patch delivery held throughout.

What separated the exported sheet from the reference was presentation density. An
industrial harness drawing is roughly half annotation by area, and MadakeCAD treated
annotation as unstyled, unanchored free text. **G1–G4 were the whole gap.**

**Second pass (after phase 5): that prediction held.** With named-pin blocks, automatic
wire annotation, net-label flags and styled text, the same sheet was rebuilt with no
hand-placed annotation at all and now carries the same information as the drawing it
replaces — **parity reached**. The reproduction script shrank from 313 commands to 258
while gaining ~110 automatic annotations, because roughly 210 hand-placed text entities
were replaced by data on the wires and the symbols.

**Third pass (after phase 6): legibility closed too.** The three findings of the second
pass were small and well understood, and they behaved that way: with the tag anchored at
the root of its point and the annotation text height set to the reference drawing's
1.4 mm, the same sheet measures **zero overlapping labels** where the default size gives
twelve, and the relay's false ERC warning is gone. The sheet now carries the same
information as the drawing it replaces **and reads like it**. What is left in the ledger
is no longer parity or legibility but the target-quality standard: physical device
blocks, pin groups, filled shapes, pictorial symbols and a wire-part legend.
