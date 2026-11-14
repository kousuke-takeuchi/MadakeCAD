# Stroke font for drawing text (KST32B bundled)

Date: 2026-11-14 · Status: approved by the user (2026-11-15) · English only

## Goal

Draw the text of a drawing with a CAD stroke font (letters made of lines) instead of the system sans-serif, so that a drawing made in MadakeCAD can carry the lettering that industrial drawings use. A stroke font whose licence allows it is bundled and usable out of the box.

This closes the last difference left in region 1 of the A0 reference reproduction (`2026-11-14-a0-reference-reproduction-design.md`): the lettering. The reference's own font cannot be obtained, so the aim is the same kind of lettering (stroke letters of the same height, width and spacing), not identical glyphs.

Success:

- A project can select a stroke font. Every text of its sheets (frame, title block, revision table, designators, values, wire annotations, net labels, notes, report sheets) is then drawn with it in SVG export, PDF export and on the canvas.
- KST32B version 3.0 is bundled with its original documentation and listed in the third-party licence inventory.
- A frame template can ask for the font's full-width Latin letters and can set letter spacing per cell.
- A project that selects no stroke font produces exactly the same output as before (existing byte-for-byte tests unchanged).
- Selecting a stroke font never moves a pin, a symbol outline or a wire.

Out of scope:

- UI for choosing the font (a later UI sub-project; the setting is made through the Command API, the CLI and scripts).
- The symbol editor, the frame editor and the frame template preview canvases (they keep the system font).
- DXF export (text stays a TEXT entity).
- More fonts. The registry is built so that another font with a suitable licence can be added; only KST32B is added now.
- Searchable text in PDFs made with a stroke font (the letters are lines).

## What exists today (verified)

- SVG: `madake-core/src/svg.rs` writes `<text>` in 36 places; `text_family_el` / `text_el` / `text_weight_el` are the shared writers, the rest write the element directly. `report_sheet.rs` and `terminal_diagram.rs` write one each. PDF is converted from the SVG (`pdf.rs`).
- Canvas: `src/canvas/textSize.ts` (`setDrawingFont`, `drawingText`) is the shared path of `src/canvas/renderer.ts`; a few editor canvases call `fillText` directly.
- Text width is estimated by `tidy::text_width` (0.6 × height per narrow character, 1.0 per wide character) and its TypeScript twin `textWidthMm`. The estimate is used both for label layout and for geometry: block symbols size themselves from pin names (`block.rs`).
- Project-wide settings with their own patch exist (`set_frame_template` → `PatchOp::FrameTemplateChanged`, mirrored by the frontend store); `FORMAT_VERSION` is 20.
- The frontend reaches the backend through one interface in `src/ipc.ts` with two implementations: Tauri IPC and the Link API over HTTP (`/api/v1/...`).
- `scripts/gen_licenses.py` generates `docs/internal/third-party-licenses.md` and states that no font is bundled.
- KST32B 3.0 (checked in its distributed archive): 7 514 glyphs (ASCII, half-width kana, ISO 8859-15, JIS X 0208 level 1 and level 2), a plain-text format `CSF/1` of straight segments on a 30 × 32 grid, half-width glyphs in the left 15 columns. Its documentation states that it is freeware, that use, distribution, modification and redistribution are free, and that there is no warranty; copyright (C) 2017 Saka.N.

## Design

### Bundled fonts and the registry

- `src-tauri/resources/stroke_fonts/kst32b/` holds `KST32B.TXT`, `KST32B.RDM` and `KST32B.MAN`, unmodified.
- `madake-core/src/stroke_font.rs` embeds the data (`include_bytes!`) and exposes a registry: `fonts() -> &[StrokeFontInfo { id, name, license_note }]` and `font(id) -> Option<&StrokeFont>`. The only id is `kst32b`. A font is parsed once, on first use.
- `gen_licenses.py` gains a "Bundled fonts" table with the font's name, author, version, the licence statement quoted from its documentation, and the path of the documentation in the repository.

### The setting

`Project.text_font: Option<TextFont>` with `TextFont { id: String, latin: Latin }`, `Latin = half | full` (default `half`). `None` (the default) means the system font as today. It is set with a new command `set_text_font { text_font }` (an unknown id is refused; undo restores the previous value) that emits `PatchOp::TextFontChanged`, in the same way as `set_frame_template`. The project format becomes version 21.

`half` draws ASCII with the font's half-width glyphs; `full` draws ASCII with the font's full-width forms (twice as wide).

A frame template may override the Latin form for its own text with `text_latin: half | full` (title block cells and the revision table). A cell may set `spacing` (a factor on the advance of every character, default 1).

### Size and metrics

The font size `s` of a text keeps its meaning. The glyph grid is scaled so that the capital height of the half-width letters is `0.72 × s` (the ratio already used by `text_size_is: cap`). With KST32B the capitals span 25 grid rows, so one grid unit is `0.72 × s / 25`; a half-width character advances 15 units (`0.432 × s`), a full-width one 30 units (`0.864 × s`). The stroke width is `0.07 × s`, not below 0.13 mm, with round caps and joins.

Anchors (`start`, `middle`, `end`) and rotation behave as for `<text>`; the baseline is the grid row on which capitals stand.

### Geometry does not move

`tidy::text_width` and everything sized from it (block outlines, pin positions, label placement, overlap metrics) are unchanged. With `latin: half` every stroke character is narrower than its estimate, so text stays inside the space the estimate reserved. Where exact widths matter and nothing else depends on them — fitting a frame cell to its width, the box of a boxed cell, the revision table — the stroke font's real width is used when a stroke font is active.

### Drawing

- `stroke_font.rs`: the font is parsed once into glyphs keyed by Unicode character (JIS X 0208 addresses are converted with `encoding_rs`, already in `Cargo.lock`). `layout(text, size, latin, spacing) -> Option<StrokeText { polylines, width }>`; `None` when any character has no glyph, in which case that text is drawn with the system font.
- SVG: every `<text>` writer goes through one function. With a stroke font it writes one `<path>` (`fill="none"`, `stroke-linecap="round"`, `stroke-linejoin="round"`) holding all strokes of the text, wrapped in the same transform the `<text>` would have had. Bold text is drawn 1.5 times as thick.
- PDF: nothing to do (converted from the SVG).
- Canvas: the backend serves the parsed glyphs, keyed by character, through IPC `get_stroke_font` and `GET /api/v1/stroke-fonts/{id}`. The frontend loads them once when a project selects the font, and a TypeScript layout with the same rules draws them; `drawingText` draws strokes when the font is loaded. Until then, and for text too small to read, it behaves as today. Parsing exists only in Rust, so the two renderers cannot read the font differently.

### Compatibility

- No `text_font`: every writer takes its present path; output identical.
- A build that reads format 20 refuses a format 21 file by its number. `text_font` is not written when it is `None`.

## Testing

Rust and Vitest, with bilingual specification sentences:

- parser: every byte class of `CSF/1`; a known glyph's strokes; the glyph count; half-width versus full-width advance;
- layout: width of a string, anchors, spacing, `full` mapping of ASCII, `None` for a character without a glyph;
- SVG: a text becomes one `<path>` at the right place and scale; rotation is kept; bold is thicker; a text with an unknown character stays `<text>`; no stroke font → the golden tests still pass unedited;
- geometry: a sheet with blocks and wires has the same netlist, the same pin positions and the same `verify` result with and without the stroke font;
- frame: `text_latin`, `spacing`, fitting and boxes use the stroke width;
- command and format: `set_text_font` round trip and undo, unknown id refused, the patch reaches the frontend store, format 20 → 21;
- font service: IPC and Link API return the same glyph data; the canvas layout gives the same numbers as Rust for the same inputs;
- licence inventory: the generated document lists the bundled font.

End to end: region 1 of the reproduction is redrawn with `text_font` set and the overlay of the title block is checked; the result is recorded in the git-ignored notes.

## Risks

- A `<text>` writer missed in the SVG. Mitigation: a test that exports a sheet using every text-bearing entity with a stroke font and asserts that no `<text` remains (except texts with characters outside the font).
- Rust and TypeScript layouts drifting. Mitigation: shared numeric test cases.
- Size of the glyph data sent to the frontend (about 1 MB of JSON, once per session). Accepted.
- The font's glyph quality at large sizes (segments on a 30 × 32 grid). Accepted; it is the nature of the font.
