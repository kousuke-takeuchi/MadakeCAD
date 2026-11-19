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
  "pitch_mm": 5.0,
  "planned_sheets": ["1 Power", "2 Control", "3 Motors"],
  "title_block": {"company": "Example Works", "rev": "A"}
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
- `planned_sheets`: optional. The sheets the caller plans to draft, in any order; it may include this request's own name and names that already exist. Pass the names of all sheets you will draft so that tags keep room for them. Each entry is either a string (the sheet name only) or an object `{"name": "...", "units": ["U1", ...], "pins": {"U1": ["1", "2"]}}` (`pins` optional, with the meaning of the request's `pins`: it limits a unit on that planned sheet). Refused: a blank name or the same name twice (spaces around a name are ignored); in an object, a blank or unknown unit, a pin key the unit does not have, or `pins` naming a unit that is not in `units`. An entry that is neither a string nor such an object (a number, an object without `units`, an unknown field) is refused with a message that names its position, `planned_sheets[i] (entry i+1)`, through the Link API (400), MCP and the CLI. Default: empty (the room is chosen as before, see Placement).

- `title_block`: optional overrides for the new sheet's title block, with the field names `TitleBlock` serialises (`company`, `title`, `drawing_no`, `scale`, `date`, `designed`, `drawn`, `checked`, `approved`, `rev`, `fields`), all optional; a field that is given replaces the default (an empty string empties it). An unknown field or a `title_block` that is not an object is refused with a message that starts with `title_block:`, and a `fields` name that `set_title_block` would refuse (not 1 to 32 characters of a-z, 0-9 and _) is refused naming `title_block.fields`. Defaults: `title` (printed by the frame as the drawing name, 品名) is the trimmed sheet `name`; when `planned_sheets` is non-empty and `drawing_no` is not given, `drawing_no` is `k/n` with n the number of planned entries and k the 1-based position of this sheet's name among them (trimmed comparison; a name that is not planned leaves it empty); every other field is what `AddSheet` gives a new sheet (empty). The bundled frames (`jis-a3`, `jis-a4-portrait`) print the item name with `fit`, so a name wider than its 92 / 84 mm cell is printed smaller and stays inside the title block.

### Which pins a view carries

A pin of a unit is **in use** when its source net has two or more pins. The view carries the pins in use, limited by `pins` when given. The pin name printed inside the box is the pin name of the source definition (block pin name or library pin name); the number outside is the source pin key. A block pin without a number keeps its name as key. A pin whose key equals its name is written without a number unless another numbered pin of the unit carries that name (numbered pins may share names with each other and with other pins' numbers; only the keys must be unique), so every block the source holds can be drafted with its names intact. A view that still is not a valid block (two pins with one key) is refused with the unit and the pin named.

A unit whose source symbol is not a block (a library symbol such as a resistor or a relay coil) is not turned into a box: the same symbol is placed with the source rotation, reference and value. Section 2 does not apply to it; it may appear on one function sheet only (a second use is refused).

### Net classes

For each source net that touches a carried pin:

0. **Pin groups (bars)**: carried pins of one unit on one side that belong to the same net are placed in adjacent rows (in natural order of the pin key) and joined at their stub ends by one straight vertical wire, the bar, with a junction wherever three ends meet (the stub end of every middle pin). This applies to every net class. Rows are left free between two power groups (and between two ground groups) of one side so that the mark of a group does not touch the rows next to it.
1. **Power or ground** (name in `power_nets` or `ground_nets`): no wire. Each group gets one mark: `power_rail` with the net name as value for power; for ground, the net mark symbol that the source uses on that net, or the library ground mark when the source has none. The mark is led out from the group's end (the upper end of the bar for power, the lower end for ground; the stub end for a single pin): the line continues one pitch up (power) or down (ground), turns away from the box, and the mark stands at its end, as far out as half the wider of its printed text and its figure plus 1.25 mm, rounded up to the 2.5 mm grid. So the mark's text is clear of the bar, the stubs and the pin numbers. Where the lead leaves a bar, three ends meet and get a junction. (A stub that is vertical, on a library symbol, continues in its own direction and turns right.)
2. **Internal**: every pin of the net is carried on this sheet. The groups are joined by wires; each group is one terminal of the router, a point on its bar: the end nearest to its peers, the middle when peers are above and below. A net whose pins are all in one group (a jumper inside one unit) needs no routed wire: the bar is the whole net.
3. **Leaving**: some pins are not on this sheet. The carried groups are joined by wires when there are two or more, and one tag (a net label with `tag`) is placed at the group that comes first in column order. With one carried group the tag sits at the end of its stub (or bar end). With two or more, the tag hangs on its own branch: the group's terminal point, then 5 mm more outward on the same row to the tag, with the flag pointing away from the box; the wire to the other groups leaves the branch before the flag, so the tag is a free end and no wire runs through or along a flag. When the net is drawn on a channel trunk (see Channel tracks) its tag is moved to the end of the trunk instead and the pins have no branch. The pins, the tag and the other pins are one net.

The tag name is the source net's label name. A source net without a label gets the name `<reference>.<pin>` of the pin that comes first when all pins of the net are sorted by reference and pin key in natural order. The rule uses the whole source net, so every sheet that shows the net computes the same name.

The new sheet has `xref_style` = `sheet_name`, so a tag shows the names of the other sheets that carry the same name as soon as those sheets exist. A pin on the right side of a box gets `tag_flow` = `out`.

### Pin sides and order

- The side is decided per unit and net, not per pin: all carried pins of one net on one unit go on the same side and form one group (bar). The decision uses only the net's peers on other units (a unit's own pins of the net are not peers):
  - If there are peers in other columns, only those decide: all in columns to the left, left; all to the right, right; both, the side with more peers (left on a tie). Peers in the unit's own column do not vote. Units stacked in one column that share a net with the same other-column peers therefore land on the same side, so such a net (a bus over two stacked units and a unit in the neighbouring column, say) lies in one channel.
  - If all other-unit peers are in the unit's own column, right, except in the last column, where it is left. Units stacked in one column then join each other through the channel to their right, not through the left margin where the tags sit.
  - If no other unit carries the net (a jumper inside one unit, or a pin whose other pins are on other sheets), right for a unit in the last column, otherwise left.
- Power pins are the first rows of the left side, ground pins the last rows of the left side, each group separated from the signal pins by one spacer row (`{"gap": 1}`).
- Signal groups on a side are ordered to reduce crossings: two sweeps of the barycentre method over the vertical positions of their peers (a group moves as one item; its barycentre is the mean of its pins'), ties broken by natural order of the first pin key. Power groups come first and ground groups last, each ordered by the key of its first pin. The result must not depend on hash order: the same input gives the same sheet.

### Placement

- The drawing area is the frame's inner area minus the title block.
- Stub length is per unit side: max(5 mm, width of the widest printed outside pin number on that side + 2.5 mm, measured with the block's own text height), rounded up to the 2.5 mm grid, the same for every pin of that side. A long number that the block itself draws into its box is a block rendering matter and is not changed here.
- Columns are packed, not spread: the first starts at the left edge of the drawing area plus its own left extent; column k+1 starts at the right extent of column k (boxes, stubs, bars, tags, marks) plus a channel of max(20 mm, 10 mm + 2.5 mm × the tracks the channel needs, see Wires), rounded up to the 2.5 mm grid. The track count follows from the terminal heights, which do not depend on the horizontal positions, so it is known before the columns are placed. Units in a column are stacked top to bottom in the given order, top aligned, with a gap of at least four pitches measured between everything one unit draws (marks and bars included) and the caption of the next.
- Box width holds the longest pin name on both sides plus the reference; `block_pin_text` is chosen so that names are at least 2.2 mm high. Every box has `block_number_align` = `"edge"`: an outside pin number ends 0.5 mm outside the box edge and runs away from the box along the lead and the stub, so a long number never covers the box edge or a pin name. (The attribute is a general block attribute; without it a block draws its numbers centred on the lead as before.)
- Tag text size is chosen so that tags on adjacent pins do not touch (flag height at most pitch minus 1 mm; at a pitch of 2.5 mm the text cannot be smaller than the smallest annotation size, so the flag is taller and adjacent flags are only 0.25 mm apart).
- A tag's width reserves room for the names of other sheets that appear inside it later: the wider of its `tag_dest`, the names of the sheets of the open project that already carry the net name (joined as `xref::tag_dest_names` joins them: at most two names plus the other mark), the longest sheet name of the open project and 10 characters. Column extents include every tag flag with that room. When `planned_sheets` is given, the room is exact per tag instead. For a tag of net N, the destination sheets are: (1) the sheets of the open project that already show a net label named N (the lookup the export uses for the destination text; a sheet that has only a power or ground mark of that name is not listed by the export and does not count); (2) the object entries that do not exist yet, are not this sheet, and carry at least one pin of source net N, with the carried pins decided exactly as the draft decides them for a sheet (the pins in use of the entry's `units`, limited by its `pins`; the power and ground nets of this request apply); (3) the string entries that do not exist yet and are not this sheet, which are unknown and count for every tag. The reserved text is: one name gives that name; two or more give the two widest names joined as `xref::tag_dest_names` joins them, plus the other mark when the set has three or more; an empty set gives no room, so the flag shows its name only. There is no floor of 10 characters in this mode. A `tag_dest` given through `destinations` keeps its own exact width when it is wider. Without `planned_sheets` the old rule stays: the wider of the longest sheet name of the open project, the names that already carry the net name (joined) and 10 characters. Column extents, packed columns, the edge refusals and the router obstacles all use this room, for every tag of the sheet whatever net it carries.
- Symbols, pins, the ends of stubs, tags and marks are on the 2.5 mm grid. The bends of routed wires may lie on the router's 1.25 mm grid.
- A column that is taller than the drawing area, or columns whose right extent (tags and marks included) would cross the right edge of the drawing area with the channels above, refuse the draft. The message names the column and the excess in mm, so that the caller can move units to another sheet. Nothing is created.

### Wires

Internal wires are planned with the existing router (`route.rs`) on the new sheet's geometry: orthogonal, off symbols and text, right-angle crossings only, junctions where three or more ends meet. Every tag flag (with the reserved destination width) and every mark with its text is an obstacle kept at the router's clearance, and wires stay inside the frame's inner area and out of the title block with its revision rows, as `arrange::frame_areas` reports them for the open project's frame template. A net the router cannot place is joined instead by tagged labels with the net name (`tag`, flag pointing away from the box, no `tag_dest`) at the upper end of each group's bar (the stub end of a single pin; a flag is drawn above its point, so the bar leaves the flag at the point only), the tagged group keeping its straight line to its tag, and listed in the result (`labelled`).

No wire may cross or touch any tag flag, in any case. Every flag, with its reserved destination room, is a hard obstacle for all nets, and so is every fallback flag: routing runs in rounds; after a round the nets that failed get their fallback tags, those flags are blocked and their points and terminals become fixed spots, and every other net is routed again; this repeats until no new net fails. A route whose wires or leads touch a flag (other than its own wire ending at its own tag's point, at or below the flag's lower edge, or its own trunk along the flat edge of a flag moved to the trunk's end) counts as failed and falls back to tags.

#### Channel tracks

Before the grid router runs, a deterministic channel router draws the common nets with straight wires.

- **Channels.** The channel c is the vertical strip between the right extent of column c and the left extent of column c+1 (extents include stubs, bars, tag flags with their reserves and marks). There is one channel per adjacent pair of columns.
- **Channel-routable nets.** A net is a candidate for channel c when every one of its router terminals (one per pin group, at the stub or bar end; the tag terminal of a tag branch is not counted) lies on the right side of a unit in a column up to c, or on the left side of a unit in a column from c+1 on, and it has two or more terminals. A net whose terminal is on a vertical pin of a library symbol has no candidate channel. A drop therefore may run through other channels and through the vertical gaps between the units of the columns in between; the validation below decides whether it may. A Leaving net with only one group on the sheet has no routed wire and is unaffected.
- **Choosing the channel.** A net with several candidate channels takes the one with the shortest total drop length (the sum, over its terminals, of the horizontal distance to the middle of the channel, measured with the columns packed at the minimum channel width), then the lowest channel; the choice is made before the columns are placed, because the track counts decide the channel widths. If the route in the chosen channel does not validate (or no track position is free), that channel is ruled out for the net and the whole sheet is laid out and routed again; this repeats until no net is newly ruled out (a channel is only ruled out once, so it ends). A net with no candidate left goes to the grid router. Only the nets of the chosen channel count for its width (a drop that crosses another channel does not widen it).
- **Wires.** Two terminals at the same height on the two sides are one straight horizontal wire. Otherwise the net gets one vertical trunk on a track, from its topmost to its lowest terminal, and one horizontal drop from each terminal to the trunk, with a junction wherever three or more ends meet. Every vertex is on the 2.5 mm grid.
- **Tracks.** Tracks are 2.5 mm apart on the 2.5 mm grid, at least 5 mm from the column extents on both sides, centred in the channel. Two nets may share one track only if their trunk spans, each widened by 5 mm, do not overlap (a gap of exactly 10 mm is allowed).
- **Assignment.** Nets are placed in natural order of the net name. Each net takes the position (a shared track where allowed, or a new track opened between any two tracks or at either end) with the fewest crossings between its trunk and drops and the wires already placed; ties prefer sharing a track (so the channel stays narrow), then the leftmost position. A position at which a wire would touch or overlap an already placed wire (a vertex, a collinear overlap, a drop through another trunk's end) is not available; a net with no available position is left to the grid router. A drop of a net of another channel that crosses this channel is a fixed horizontal line here: it counts in the crossings, and a position at which a trunk would end at its height, or run along it, is not available (a trunk is only crossed at right angles).
- **Channel width.** 10 mm + 2.5 mm × the tracks needed, at least 20 mm. The track count is computed from the terminal heights before the column positions are fixed.
- **Validation.** For every net, its wires must keep the router's clearance from every box, pin number and other text, tag flag (with its reserve), mark and mark text, the frame and the title block; must not touch or cross the tag branch of any net, including its own; must not touch another net's stub end, bar, junction or wire, or overlap another wire along a line; and may cross another net's wire only at a right angle, never at a vertex of it. A drop that would pass through another net's stub end, bar or junction is therefore refused. Contact at the net's own terminal is not checked. A net that fails is removed from the channel result and the layout is made again without that channel for it (see above); once no channel is left it is handed to the grid router (a track no other net uses stays empty). Accepted wires are shared by all channels, so a drop through another channel is checked against every trunk.
- **Tags on trunks.** A Leaving net with two or more pin groups that is channel-routed has its tag moved to the trunk (a net with two level terminals and a tag gets a trunk too, only as long as the tag extension). The trunk is extended beyond its topmost terminal, or beyond its lowest one, by 5 mm, then by 10 mm; the tag flag sits at that end, horizontal, pointing right or pointing left, with the label point on the trunk's end (`tag_flow` = `out`: the pointed end points away from the trunk). The positions are tried in this fixed order: top +5 right, top +5 left, bottom +5 right, bottom +5 left, then the same four at 10 mm; the first whose trunk extension and flag (with its full reserved destination width) validate like wires is taken: the flag must be inside the channel (between the column extents), clear of boxes, texts, other flags and marks, the frame and the title block, of every wire and fixed spot, and of the own drops. The tag is placed after the wires of all other channel nets are accepted, so a flag never pushes a plain net off its track, and a moved flag is a hard obstacle for the grid router and for the tag fallback. The flat edge of the flag lies on the trunk, so a flag at the lower end runs along the trunk for its height (the net's own trunk is the one wire that may touch its flag). The pins' tag branch is removed. If no position validates, or the net is not channel-routed, the tag stays on its branch at the first pin and the grid router draws the net exactly as before. Order of work: the layout (column extents, channel widths, frame-edge refusals) is always made with the branch tags and their reserve at the pins; tags are moved only after routing and the columns are never shrunk afterwards, so `tags_outside` stays empty for sheets drafted with complete `planned_sheets`. Without `planned_sheets` the reserve of a flag is at least 10 characters wide, which usually does not fit a 20 mm channel; such tags stay on their branch.
- **Then the grid router** runs for the remaining nets exactly as before, with every channel wire as a fixed line and its terminals and junctions as fixed spots. A channel wire that comes within the router clearance of a fallback tag added later is handed to the grid router too. The fallback by tags stays for nets the grid router cannot place. `wired` lists the nets routed by either router, and the connections are the same whichever router draws a net.

### Result

`marked` counts marks (one per power or ground group), not pins.

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

The plan in core also carries a routing log `route_log` that is not part of any public reply (`#[serde(skip)]`; MCP, Link API and CLI are unchanged): one record per net with two or more pin groups on the sheet, `{net, router: channel | grid | fallback_tags, channel, candidates: [{channel, drop_length_mm, rejected}], tag: null | {placed: branch | "top+5 right" ..., tried: [{position, rejected}]}}`. `rejected` is a short reason built from the first obstacle the validation met (for example `drop at y=47.5 hits box S1`, `trunk at x=77.5 touches wire of net Z`, `no free track`, `column extent`, `flag of net X`). The ignored test `draft_requests_from_a_file_for_looking` writes it under each request of `summary.json` as `route_log`.

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

The reply also lists the tags whose flag does not fit its sheet:

```json
{
  "layout_ok": false,
  "tags_outside": [{"sheet": "2 Control", "net": "SIG_A", "excess_mm": 12.3}]
}
```

`tags_outside` holds every tagged net label of the open project whose flag, drawn with the destination text the project export prints (`xref::sheet_tag_dests`, or the tag's own `tag_dest`; shape from `net_tag::net_tag_with`), extends beyond the inner frame area of its sheet (`arrange::frame_areas` with the project's frame template) on any side, or into the title block with its revision rows. `excess_mm` is the largest overhang, in mm with one decimal. The list is sorted by sheet order, then net in natural order. This happens when sheets drafted later add their names to a flag that did not reserve room for them (see `planned_sheets`). `layout_ok` is true when `tags_outside` is empty. `ok` keeps meaning that the connections are equal and is not affected by `layout_ok`. The CLI `draft-check` prints the list and exits non-zero when `ok` or `layout_ok` is false.

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
- A net whose peers on the sheet are on both sides of a unit is drawn on one side of that unit only (the side with more peers), so its wire to the peers on the other side runs round the box.
- Where several nets run between units of one column or between two columns at slightly different heights, each net is a trunk with short drops; the drops of nets on neighbouring tracks cross the trunks of the others at right angles. The tracks only avoid crossings the pin order allows.
- A net with a tag branch whose tag cannot move to a trunk (no position validates, or the flag is wider than the channel), and a net with a drop that a box, text or another net's wire blocks in every candidate channel, is drawn by the grid router and can still bend or loop around boxes.
- A trunk net with a tag keeps its tag at the trunk end only; the flag of a net that is not channel-routed still hangs on the branch.

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
