# Schematic Editor

[日本語](03-schematic-editor.ja.md)

A purpose-built editor for industrial electrical schematics, modeled on the conventions of AutoCAD Electrical-class tools: ribbon UI, a dark drafting canvas with millimeter coordinates, and strict grid discipline (2.5 mm pin pitch).

## Available

- **Canvas**: pan (middle button / Space), wheel zoom, grid display, paper outline, crosshair cursor. Coordinates in mm, top-left origin
- **Symbols**: 47 bundled JIS C 0617 / IEC 60617 symbols — supplies and conversion (AC source, transformer, bridge rectifier, protective/frame earth), protection (fuse, 1/2/3-pole circuit breakers, disconnectors), operating switches (pushbuttons NO/NC, emergency stop, changeover and three-position switches, limit switches), relays and contactors (coil, NO/NC/changeover contacts, 3-pole contactor), semiconductors, passive parts, loads (motors 3-phase/single-phase/DC, lamp, bell), instruments (voltmeter, ammeter, current transformer) and connectors — plus **parametric symbols** `connector_{n}p` and `terminal_block_{n}p` with 1–50 poles at 5 mm pitch. Every symbol carries pin connection directions, attribute slots (reference, part number, description, rating) and bilingual search keywords; multi-pole devices are numbered pole by pole (1/2, 3/4, 5/6) and conduct pole by pole. Terminal blocks model feed-through terminals (left/right points of a terminal are internally connected)
- **Placement**: insert dialog with search, per-category grid, pole-count stepper for parametric parts, and a parts-database section that applies part number and rated current on placement. Ghost preview, `R` to rotate, automatic reference designators (F1, K2, TB1, …)
- **Wiring**: orthogonal polyline tool with grid snap and pin snap (diamond marker); junctions; net labels; free text annotations
- **Editing**: click/shift/marquee selection, drag-move (grid-snapped), delete, full undo/redo (⌘Z / ⇧⌘Z) — every edit from every entry point (UI, AI, CLI, API) shares one history
- **Sheets**: multiple sheets per project with tab switching and adding
- **Wire numbers**: net-scoped automatic numbering (top-up / renumber, start number, one sheet or the whole project), per-net editing in the properties panel, and a display class of their own
- **Harness boundaries**: named dashed enclosures; every wire fully inside one is reported in the harness column of the wire list
- **Circuit macros**: save the current selection as a reusable macro (name, category, base point = the bottom-left pin of the selection, variant `A`) into `~/MadakeCAD/macros`, then insert it from the **Macros** tab of the insert dialog. Placement shows a ghost, `R` rotates, `Tab` switches variants. On insert, reference designators are renumbered above the highest already used in the project and wire numbers are cleared, so a macro can be dropped twice on the same sheet without collisions. `⌘C` / `⌘V` copy and paste a selection through the same mechanism as an unnamed macro. One insert is one edit — a single `⌘Z` takes the whole circuit back
- **Coil ⇔ contact cross-references**: symbols that share a reference designator (a `K1` coil and its contacts) are one device. A contact map is drawn under the coil — IEC 60947-1 terminal pairs (`13-14`, `23-24`, …) against each contact's `/sheet.zone` address — and every contact carries the coil's address beside it. When the part's contact configuration is known (`contact_config`, e.g. `2NO+2NC`), unused contacts are listed as `—`, and using more contacts than the device has is an ERC error
- **Project-wide search (⌘F)**: a floating search bar over the canvas with filter chips (designator, part no., net, text) and a docked result panel (designator / kind / location / note). `Enter` and `Shift+Enter` walk the hits, `Esc` closes; picking a hit switches sheet, selects the entity and zooms to it
- **Device navigator**: the third tab of the left panel lists every device as a reference-designator tree expanded to its functions (coil, contacts with their terminal pairs, terminals, body) with each function's `/sheet.zone`. Click a row to jump to it; right-click to jump or delete (deletion goes through the command engine, so `⌘Z` restores it)
- **Reference surfer**: Alt(Option)-click a symbol with a designator, a net label, or a numbered wire to open a popup listing everywhere it appears; the arrow keys and `Enter` surf between the locations, `Esc` closes
- **View classes (layers)**: nine display toggles — wires, symbols, reference designators, net labels, wire numbers, harnesses, annotations, frame, grid — on the ribbon View tab (display-only; outputs always contain everything)
- **Properties panel**: edit reference designator and part number/value of the selection

## Planned

- Macro value sets: placeholders that apply ratings and part numbers across a whole macro in one go, and adding variants beyond `A` from the save dialog (M4 phase 3)
- Dragging an unplaced device function (a reserved contact) from the device navigator onto the drawing (M4 phase 3)
- PLC I/O drawings and reports (M4)
- Symbol editor for user-defined symbols (M4)
- 2D panel layout sheets; customizable frame / title-block templates (M4)
