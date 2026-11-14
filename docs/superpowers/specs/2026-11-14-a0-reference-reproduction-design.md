# Reproducing an A0 reference drawing, region by region

Date: 2026-11-14 · Status: design approved by the user in chat, spec awaiting review · Japanese: [2026-11-14-a0-reference-reproduction-design.ja.md](2026-11-14-a0-reference-reproduction-design.ja.md)

## Goal

Redraw one confidential reference drawing (a single A0 landscape sheet, called **the reference** below) in MadakeCAD so that the result **looks the same** and **is electrically correct**, and close every MadakeCAD gap that the work exposes.

The reference is not in the repository. It lives in `docs/references/` (git-ignored); `docs/references/README.md` says which file it is. This spec and everything else that is committed describe the reference only in general terms. `scripts/audit_confidential.py` must stay clean.

Success:

- A MadakeCAD project with one A0 landscape sheet whose exported PDF, laid over the reference at the same resolution, matches in part positions, wire routes, wire colours, text content and text position, frame, title block and revision block.
- The netlist of that project has no unconnected pin and no dangling wire except the ones the reference itself leaves open.
- Every gap found is recorded in the gap ledger, and each one is closed in MadakeCAD unless the user agrees to leave it open. The aim is zero open gaps at the end.
- The project is an electrical model that verification and simulation can use as input, not a picture:
  - every part is a symbol or a block with pins and a kind (a fuse is a fuse, a source is a source), never a bare shape;
  - a rating printed on the reference is entered as an attribute as well as text;
  - every wire carries its colour and its size as attributes, the size taken from the part-number legend;
  - a value the reference does not give stays empty. It is never guessed.

Out of scope:

- The typeface. The reference uses a CAD stroke font; MadakeCAD keeps its own font. Text height, anchor and position must match, the glyph shapes need not.
- Sub-grid positions. Coordinates are snapped to the 2.5 mm grid; the snapping error is accepted.
- The earlier attempt made inside the app (7 sheets, 57 entities). It is kept only as a backup for a later review of the agent conversation and is not reused.
- Extending simulation. Entering the missing ratings and finding problems such as an undersized protective device is the next sub-project, with its own design, spec and plan. Open points for it: how to model the circuits on a board (DC operating point or an input/output model of the block), transients such as inrush current, and the UI for entering load power.

## What exists today (verified)

- The reference PDF is one A3 page that is a reduction of an A0 sheet. It is entirely vector; text is outlined, so there is no text to extract. Line geometry, colour and width can be extracted.
- An earlier reproduction (another reference drawing, sheet 1) set the method: a deterministic Python script in `docs/references/repro/` sends Commands to the Link API, and generalised gaps go to `docs/internal/specs/reproduction-gaps.md` (G1 to G23).
- The installed app listens on port 9310. A development build can listen elsewhere with `MADAKE_MCP_PORT`.
- The earlier attempt and its conversation are saved in `docs/references/repro/a0-attempt1/` (git-ignored).

## Method

### 1. Sheet and coordinates

One A0 landscape sheet, drawn at full size. A PDF point maps to paper millimetres by one scale factor (A3 to A0, about 2.83) and one offset, both measured from the frame corners of the reference. Converted positions are snapped to the 2.5 mm grid.

### 2. Ground truth

- Geometry: `pdftocairo -svg` on the reference, then a script that reduces the paths to line segments with colour and width, split per region, written as JSON under `docs/references/repro/a0/truth/`.
- Text: read from 600 dpi crops. Text that cannot be read with confidence goes into a list that the user resolves; it is never guessed.

### 3. Reproduction script

`docs/references/repro/a0/` holds one Python file per region plus a runner. The runner sends Commands to the Link API of a **development build on port 9320**, so that features added during the work are available at once. It is deterministic and can be rerun from an empty project. The installed app on 9310 is not used.

### 4. Comparison

After each run the sheet is exported to PDF, rasterised at the same resolution as the reference, and an overlay difference image is written per region. A region passes when:

- every part, wire, junction and text of the reference region is present at the matching position,
- wire colours match,
- `GET /verify` reports no unconnected pin or dangling wire inside the region other than the intended ones,
- the overlay and a visual check of the crop show no overlap or unreadable text.

### 5. Gap handling

- Each gap gets the next number (G24 onward) in `docs/internal/specs/reproduction-gaps.md` and its Japanese pair, in general terms.
- Each gap is closed on its own branch, following the project rules: spec or short design first, plan, TDD, tests carry the bilingual specification sentences, `python3 scripts/gen_spec.py` regenerated, every edit goes through a `Command` in `madake-core`.
- A feature that changes how the UI looks is designed in `MadakeCAD.pen` first.
- A gap is left open only when the user agrees. The default is to close it.
- Customer-specific content (logo, company name, part numbers) never enters the repository. It goes into user data (`~/MadakeCAD/templates`, the project file) or into `docs/references/`.

### 6. Order of regions

1. Frame, title block, revision block
2. to 7. Six regions of the drawing itself, in the order listed in the git-ignored `docs/references/repro/a0/NOTES.md`

Region 1 comes first because it fixes the coordinate mapping.

## Gaps expected

Not listed here. What the reference needs and MadakeCAD lacks is recorded, in general terms, in the gap ledger as each region is drawn.

## Testing

- Feature work: Rust and Vitest tests as required by `CLAUDE.md`.
- The reproduction itself: the comparison of section 4, run after every change, per region and for the whole sheet at the end.

## Risks

- Reading outlined text wrongly. Mitigation: 600 dpi crops, an explicit list of doubtful strings for the user.
- A feature built for an early region not fitting a later one. Mitigation: before designing a gap, check the other regions for the same element.
- Leaking confidential terms. Mitigation: run `scripts/audit_confidential.py` before every commit.
