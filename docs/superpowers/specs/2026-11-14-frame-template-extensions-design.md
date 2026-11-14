# Frame template extensions: margins, marks, revision table, rule weights, rotated text, free fields, text fitting, logo image

Date: 2026-11-14 · Status: design approved by the user in chat, spec awaiting review · Japanese: [2026-11-14-frame-template-extensions-design.ja.md](2026-11-14-frame-template-extensions-design.ja.md)

## Goal

Let a frame template describe the frame of a large industrial sheet as it is really drawn, so that region 1 of the A0 reference reproduction (`2026-11-14-a0-reference-reproduction-design.md`) matches the reference. This closes gaps G24 to G30 of `docs/internal/specs/reproduction-gaps.md` and one more found while reading the code (G31, the logo image).

This spec is **sub-project A: data and drawing**. Editing the new items in the UI (frame editor, title block dialog, revisions dialog) is sub-project B, designed in `MadakeCAD.pen` first and specified separately. B depends on the data format fixed here.

Success:

- A user frame template can state: a margin per side, hidden zone labels, trim and centre marks, a revision table with its own columns, headers, row height and fixed empty rows, rule weights, rotated and boxed cells, free title block fields, capital-height text sizes, text fitted to a cell, and a logo image.
- SVG export, PDF export and the canvas draw all of it the same way.
- A template or a project that uses none of the new items produces **exactly the same output as before** (the existing byte-for-byte SVG tests stay unchanged and green).
- The frame editor does not lose the new items when it loads and saves a template that has them.

Out of scope:

- UI for editing the new items (sub-project B).
- The typeface and a width factor for lettering.
- Zone label styles beyond the existing numeric and alpha.

## What exists today (verified in code)

- `madake-core/src/frame_template.rs`: `FrameTemplate { id, name, name_ja, description, description_ja, paper, frame_margin, zones, title_block, revision_block, logo, builtin }`. `Zones { cols, rows, col_labels, row_labels }`. `TitleBlockDef { origin, w, h, row_h, grid_lines, cells }`. `GridLine { x1, y1, x2, y2 }`. `Cell { x, y, w, h, text, bind, size, weight, align, omit_if_empty }`. `RevisionBlockDef { rows_max, col_w }` with `REVISION_COLUMNS = 4`. `LogoFrame { x, y, w, h, path }`. `validate()` checks the template; `bind_value()` resolves `{project.*}`, `{sheet.*}`, `{rev.latest}`, `{title_block.*}` and four aliases; unknown binds become empty text.
- `madake-core/src/svg.rs`: `frame_and_title_block()` draws the border with one margin, always draws zone labels (`zone_cols.max(1)`), draws every rule with `RULE_STROKE` (0.25) and the border with `FRAME_STROKE` (0.5), draws cells with `text_weight_el`, and draws the logo as an empty rectangle ("image embedding is not supported"). `revision_block()` draws four fixed columns with the built-in headers `REV_HEADERS`, uses `title_block.row_h`, draws nothing when there are no revisions, and neither clips nor wraps text.
- `madake-core/src/model.rs`: `Revision { mark, date, description, by }`; `TitleBlock` with ten string fields; `FORMAT_VERSION = 19`.
- `madake-core/src/pdf.rs` converts the SVG with `svg2pdf`; the SVG is the single drawing source. The crates `image`, `png`, `zune-jpeg` and `base64` are already in `Cargo.lock`.
- `madake-core/src/tidy.rs`: `text_width(text, height)` estimates the width of a string.
- `src/canvas/frameTemplate.ts` is the canvas twin (`frameLayout`, `revisionLayout`, `bindValue`, `REV_HEADERS`). `src/canvas/frameEditor.ts` and `src/stores/frameEditor.ts` edit templates (`REVISION_COLUMNS = 4` is repeated there).
- Commands: `set_frame_template`, `set_title_block`, `set_revisions`. Link API: `GET /frame-templates`, `POST /frame-templates/save`.

## Design

Every new item is optional and defaults to today's behaviour. Unknown values are rejected by `validate()` with a message that names the item.

### G24 — Margin per side

`frame_margins: { left, right, top, bottom }` (mm, each ≥ 0). When present it replaces `frame_margin` for the border, the zone ticks and `title_block_origin`. `frame_margin` stays required so that old readers of the file still get a number; when `frame_margins` is present `frame_margin` is ignored.

### G25 — Zone labels and marks

- `zones.show: bool`, default `true`. When `false`, neither labels nor zone ticks are drawn. Zone addresses used by cross references are computed as before.
- `marks: { trim: bool, centre: bool }`, both default `false`.
  - `trim`: an L at each paper corner, 4 mm in from the paper edge, arms 15 mm long, stroke 0.5.
  - `centre`: at the middle of each border side, a line from the border outwards, length = that side's margin or 10 mm, whichever is smaller, stroke 0.5.

### G26 — Revision table

`revision_block` gains:

- `columns: [{ field, header, w, align }]` — 1 to 8 columns. `field` is one of `mark`, `count`, `description`, `rev_no`, `date`, `by`, `approved`; each at most once. `header` is the text printed in the header row. `w` in mm; the widths must sum to `title_block.w` (tolerance 0.01). `align` is `start`, `middle` or `end` (default `start`). When `columns` is absent, the table is the four columns of today built from `col_w` and `REV_HEADERS`.
- `row_h` (mm): row height of the table. Default: `title_block.row_h`.
- `rows_fixed: n`: always draw `n` revision rows plus the header row, ruled, even when there are fewer revisions (including none). The newest `n` revisions are shown. Default: absent (rows follow the number of revisions, limited by `rows_max`, nothing drawn for none).
- `mark_shape: "none" | "triangle"`, default `none`. `triangle` draws an upright triangle around the mark text, fitted to the row height.

Text in a revision cell is fitted to its column (see G30 `fit`) and may contain `\n` for a second line.

`Revision` gains `rev_no`, `approved` and `count`, all strings, default empty. `count` is printed as stored (for example `x7`).

### G27 — Rule weights

`GridLine.weight: "thin" | "medium" | "heavy"`, default `thin`. Strokes: 0.25, 0.5, 1.0 mm.

### G28 — Rotated and boxed cells

- `Cell.rotation: 0 | 90 | 180 | 270`, default 0. The text is rotated clockwise on screen about its anchor point (`x`, `y`).
- `Cell.boxed: bool`, default `false`. Draws a thin rectangle around the text, 0.8 mm away from the estimated text box, rotated with it.

### G29 — Free title block fields

`TitleBlock.fields: map<string, string>` (default empty, not written to the file when empty). A bind `{field.<name>}` resolves to the value, or to empty text when the name is absent. Names: 1 to 32 characters of `a-z`, `0-9`, `_`. `set_title_block` carries the map like the other fields.

### G30 — Capital-height sizes and fitting

- `text_size_is: "font" | "cap"` on the template, default `font`. With `cap`, every `Cell.size` and the revision table text sizes are capital heights; the font size drawn is `size / 0.72`.
- `Cell.fit: bool`, default `false`. With `fit` and a `Cell.w`, the font size is reduced so that `tidy::text_width` of the widest line is at most `w`. It is never enlarged.
- A cell value may contain `\n`. Lines are drawn downwards from the cell's baseline with a pitch of 1.25 × font size (for a rotated cell, along the rotated axis).

### G31 — Logo image

`logo.path` names a PNG, JPEG or SVG file relative to the folder the template was loaded from. When it can be read and is at most 2 MB, the image is drawn inside the logo box, aspect ratio kept, centred, and the box rectangle is not drawn. Otherwise the empty rectangle is drawn as today. A path that leaves the template folder (`..`, absolute) is rejected by `validate()`.

- SVG: `<image>` with a `data:` URI.
- PDF: through `svg2pdf`. The plan's first task proves this with a test; if `svg2pdf` cannot place an embedded SVG image, G31 is limited to PNG and JPEG and the spec is updated.
- Canvas: the template list (`GET /frame-templates`, the IPC twin) carries a runtime field `logo.data_url` filled when the template is loaded (never read from or written to the template file, like `builtin`).

### File format

`FORMAT_VERSION` becomes 20: `Revision.rev_no / approved / count` and `TitleBlock.fields`. Older files load unchanged (serde defaults); the migration only raises the number. A build that reads format 19 refuses a version 20 file by its number, as for earlier bumps. Frame template files have no version number; the new items are optional keys.

### Drawing

`svg.rs` and `src/canvas/frameTemplate.ts` implement the same rules. Shared numbers (strokes, trim geometry, cap ratio, line pitch) are named constants on both sides with a test on each side that fixes the value.

### Editor (sub-project A only)

The TypeScript types gain the optional items. The frame editor's operations keep items they do not know how to edit: loading a template with every new item and saving it returns the same JSON. `REVISION_COLUMNS` checks in the editor apply only to templates without `columns`.

## Testing

Per item, in Rust and in Vitest, with the bilingual specification sentences required by `CLAUDE.md`:

- the item draws as specified (positions and strokes asserted on the SVG text or the layout object);
- absent item: output identical to today (the existing byte-for-byte tests are not edited);
- invalid value: `validate()` refuses it and names the item (negative margin, unknown revision field, a field twice, widths not summing to the title block width, bad rotation, bad field name, logo path leaving the folder, image above 2 MB);
- format: a version 19 project loads into version 20 with empty new fields; a version 20 project round-trips;
- editor: a template with every new item survives load, an unrelated edit, and save.

`python3 scripts/gen_spec.py` is regenerated and committed with the tests.

End-to-end: `make_frame.py` of the reproduction is updated to use the new items, `run.sh r1_frame` is run, and the title block corner of the overlay shows no red or blue rule (text glyph differences excepted).

## Risks

- Byte-for-byte SVG tests breaking by accident. Mitigation: new code paths run only when a new item is present; the old paths are not reformatted.
- The two renderers drifting. Mitigation: the same test cases on both sides, named alike.
- `text_width` being an estimate, so `fit` may leave a small overflow or shrink a little too much. Accepted; the estimate is the one the overlap check already uses.
