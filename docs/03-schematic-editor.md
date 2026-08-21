# Schematic Editor

[日本語](03-schematic-editor.ja.md)

A purpose-built editor for industrial electrical schematics, modeled on the conventions of AutoCAD Electrical-class tools: ribbon UI, a dark drafting canvas with millimeter coordinates, and strict grid discipline (2.5 mm pin pitch).

## Available

- **Canvas**: pan (middle button / Space), wheel zoom, grid display, paper outline, crosshair cursor. Coordinates in mm, top-left origin
- **Symbols**: JIS C 0617 symbol library (resistor, fuse, relay coil/contact, switches, lamp, motor, battery, ground, terminal, …) plus **parametric symbols** — `connector_{n}p` and `terminal_block_{n}p` with 1–50 poles at 5 mm pitch. Terminal blocks model feed-through terminals (left/right points of a terminal are internally connected)
- **Placement**: insert dialog with search, per-category grid, pole-count stepper for parametric parts, and a parts-database section that applies part number and rated current on placement. Ghost preview, `R` to rotate, automatic reference designators (F1, K2, TB1, …)
- **Wiring**: orthogonal polyline tool with grid snap and pin snap (diamond marker); junctions; net labels; free text annotations
- **Editing**: click/shift/marquee selection, drag-move (grid-snapped), delete, full undo/redo (⌘Z / ⇧⌘Z) — every edit from every entry point (UI, AI, CLI, API) shares one history
- **Sheets**: multiple sheets per project with tab switching and adding
- **View classes (layers)**: nine display toggles — wires, symbols, reference designators, net labels, wire numbers, harnesses, annotations, frame, grid — on the ribbon View tab (display-only; outputs always contain everything)
- **Properties panel**: edit reference designator and part number/value of the selection

## Planned

- Wire number placement and automatic numbering (M2)
- Harness boundary tool (dashed enclosure) (M2)
- Circuit macros: save/insert reusable circuit fragments with automatic re-designation; copy/paste (M4)
- Symbol editor for user-defined symbols and an expanded bundled library (M4)
- Project-wide device search and navigation (M4)
