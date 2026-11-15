# Block Fine Pitch — Design

Date: 2026-11-15. Status: draft for review. Gap: G34 (`docs/internal/specs/reproduction-gaps.md`).

## Goal

Let a block symbol be drawn at the density of hand-drawn harness diagrams: pins 2.5 mm apart, a small outline, and pin numbers inside the outline. A reference drawing being reproduced packs conductors closer than 5 mm and uses connector boxes smaller than the smallest block; with today's block every part comes out larger than on the reference and the overlay comparison cannot match.

Success means:

- A block can opt in to a 2.5 mm pin pitch and to an outline as small as 5 x 5 mm.
- Every pin connection point and every outline edge of every block still lies on the 2.5 mm grid, so wiring, netlist and verification work unchanged.
- A block that does not opt in is drawn exactly as before (same primitives, same pin positions, same export ids).

## Non-goals

- Any pitch other than 5 mm and 2.5 mm. Off-grid pins are not allowed.
- A control in the block editor for choosing the pitch or the number position. The editor keeps the attributes and steps the size correctly; the control is designed in `MadakeCAD.pen` in the later editor sub-project.
- Connector pictures (`connector` on a pin) on a fine-pitch block or on a block with numbers inside. The picture body is 4 mm along the edge and does not fit the fine pitch, and a connector pin keeps its number outside the picture; both combinations are refused.
- Plug/socket mate symbols at a finer pitch. A small connector is drawn as a fine-pitch block.
- Moving existing entities. Nothing is migrated except the format version.

## What exists today (verified)

- A block is a `SymbolInstance` with `symbol_id = "block"`. Its shape comes from two attributes: `block_pins` (JSON, pins per side) and `block_size` (JSON `{w,h}`, optional).
- `madake-core/src/block.rs` builds the `SymbolDef`; `src/canvas/dynamicSymbol.ts` mirrors it for the canvas. Constants on both sides: pin pitch 5 mm, lead 2.5 mm, pin text 2.0 mm, size step 5 mm, minimum outline 15 x 10 mm.
- The outline is a rectangle centred on the instance origin. Pins of one side are centred on that side (`offset(i, n) = (i − (n − 1) / 2) × pitch`). Pin numbers are drawn outside the outline above the lead (left/right sides) or inside (top/bottom); pin names are drawn inside.
- `block_size` is rounded up to the size step and refused when the pins do not fit.
- `block_export_id` hashes the drawn shape for DXF and KiCad export.
- `src/canvas/blockEditor.ts` and `src/components/BlockPinsEditor.vue` edit pins and step the size by 5 mm.

## Design

### 1. Two new attributes

| Attribute | Values | Default (absent) |
|---|---|---|
| `block_pitch` | `"5"`, `"2.5"` | `"5"` |
| `block_numbers` | `"outside"`, `"inside"` | `"outside"` |

Any other value is an error from `parse_block_spec` (the same error path as a bad `block_pins`). The attributes are independent of each other: `block_numbers: inside` also works on a 5 mm block. Neither can be combined with a connector picture (section 5).

`BlockSpec` gains `pitch: BlockPitch` (`Coarse` | `Fine`) and `numbers: BlockNumbers` (`Outside` | `Inside`). All geometry reads the pitch from the spec; the constant `BLOCK_PIN_PITCH_MM` stays as the coarse value.

### 2. Geometry of a fine-pitch block

- Pin pitch 2.5 mm. Lead 2.5 mm, pin text height 2.0 mm (unchanged).
- Size step 2.5 mm. Minimum outline 5 x 5 mm. The size a side needs for its pins is given in section 3.
- Automatic size (no `block_size`): height = (rows + 1) × 2.5, width from the names as today with 2.5 in place of 5 for the centre gap and not below (columns + 1) × 2.5, both rounded up to 2.5 and not below the minimum.

### 3. Keeping everything on the grid

The instance origin is always on the grid. With a 2.5 mm step, half of the outline can be an odd multiple of 1.25 mm, and a side with an even number of pins has offsets that are odd multiples of 1.25 mm. Two rules bring everything back to the grid:

1. **Outline shift.** For each axis, when the size along that axis is an odd multiple of 2.5 mm, the whole outline is shifted by +1.25 mm along that axis (right for width, down for height). The outline then spans from `−(size − 2.5) / 2` to `(size + 2.5) / 2` and both edges are on the grid.
2. **Pin row shift.** The pins of a side are centred on the shifted outline. If the resulting offsets are not on the grid, the whole row of that side is moved by +1.25 mm (down for left/right sides, right for top/bottom sides). Each side is decided on its own.

A coarse block has sizes that are multiples of 5 mm and offsets that are multiples of 2.5 mm, so neither rule ever applies to it and its output is unchanged.

Size needed: a side with `n` pins needs `(n + 1) × 2.5` mm (minimum 5 mm). At exactly that size no row shift is needed and the first and last pin are 2.5 mm from the ends of the side. A larger size is allowed; the row shift then leaves the pins at least 2.5 mm from each end. A smaller `block_size` is refused with the needed size in the message. Automatic sizing uses the needed size.

Examples (fine pitch, left side, values in mm, relative to the origin):

| Height | Pins | Outline top..bottom | Pin offsets | Note |
|---|---|---|---|---|
| 5 | 1 | −2.5 .. 2.5 | 0 | |
| 7.5 | 2 | −2.5 .. 5 | 0, 2.5 | outline shifted |
| 7.5 | 1 | −2.5 .. 5 | 2.5 | outline shifted, row shifted |
| 10 | 3 | −5 .. 5 | −2.5, 0, 2.5 | |
| 10 | 2 | −5 .. 5 | 0, 2.5 | row shifted |
| 12.5 | 4 | −5 .. 7.5 | −2.5, 0, 2.5, 5 | outline shifted |
| 5 | 2 | | | refused, needs 7.5 |

### 4. Pin numbers inside

With `block_numbers: inside`:

- Left/right sides: the number is drawn inside the outline, next to the edge, vertically centred on the pin (where the name is drawn today). Top/bottom sides: unchanged (numbers are already inside).
- Pin names are not drawn. The signal name belongs on the wire (net label). Names stay in the data and in the netlist.
- Width: the outline must hold the widest number of the left side and of the right side with the inner padding. Automatic width uses that instead of the name widths; an explicit `block_size` that is too narrow is widened to fit, the same way a too-narrow block is widened today for group labels.

### 5. Refusals

- `block_pitch: 2.5` together with any pin that has `connector`: error naming the pin.
- `block_numbers: inside` together with any pin that has `connector`, on either pitch: error naming the pin (`block pin {no}: a connector picture cannot be used with block_numbers inside`). A connector pin draws its number outside the picture, and the inside-number width does not count the picture depth or the group names, so the outline would come out too narrow.
- Unknown value of either attribute: error.
- `block_size` too small for the pins under the rules of section 3: error with the needed size.

### 6. Format version

`FORMAT_VERSION` 21 → 22. An older build would draw a fine-pitch block at 5 mm, move its pins and silently disconnect its wires; it must refuse the file instead. Files that use neither attribute differ only in the version number. Migration from 21 changes nothing but the number.

### 7. Rendering and export

- SVG and PDF follow from the `SymbolDef`. The canvas follows from `dynamicSymbol.ts`, which implements the same rules.
- Rust and TypeScript are checked against one shared case file (`tests/golden/`-style JSON with invented blocks: attributes → outline rectangle, pin points, number positions).
- `block_export_id` already hashes the drawn shape, so DXF and KiCad get a distinct definition per shape without change. A test confirms that a fine block and a coarse block with the same pins get different ids, and that a block without the new attributes keeps its known id.

### 8. Editor

- `blockEditor.ts` keeps `block_pitch` and `block_numbers` when it rewrites the attributes.
- Size stepping in `BlockPinsEditor.vue` and the minimum in `blockEditor.ts` use the block's pitch (2.5 mm step for a fine block).
- No new control. No visual change to the editor.

### 9. Entry points

Attributes travel through the existing commands (`place_symbol`, `set_symbol_attrs`), IPC, Link API and MCP. The MCP tool descriptions and `docs/10-automation-api(.ja).md` describe the two attributes. The drawing helper outside the repository gets `pitch=` and `numbers=` arguments.

## Testing

Rust (`block.rs`) and Vitest (`dynamicSymbol.test.ts`), each with the bilingual specification sentences:

- A block without the attributes produces the same definition as before (existing tests and the known export id stay green, untouched).
- Fine pitch: pins 2.5 mm apart; every pin point and outline edge is a multiple of 2.5 mm for all sizes from 5 to 20 mm and pin counts from 0 to the maximum that fits, on all four sides (property-style loop).
- The outline shift and the pin row shift for the cases of the table in section 3.
- Minimum and automatic sizes; refusal of a too-small `block_size` with the needed size in the message.
- `block_numbers: inside`: number positions, no name primitives, widening.
- Refusals of section 5.
- The shared case file passes on both sides.
- Format: a version-21 file opens and saves as 22; a version-23 file is refused.
- Netlist: wires ending on the pins of a fine block connect (one end-to-end test through `Engine`).
- Editor: attributes survive an edit; size steps by 2.5 mm on a fine block.

## Documents

`docs/internal/data-model.md` (format 22 row, block attributes), `docs/internal/feature-inventory.md`, the gap ledger (G34, general terms, English and Japanese), `docs/10-automation-api(.ja).md`, regenerated `docs/13-specification*.md`.

## Risks

- Text overlap at 2.5 mm pitch: pin text is 2.0 mm high, so rows almost touch. Accepted; it matches dense drawings. Verified visually on the canvas and in the PDF during the reproduction.
- Code that assumes a centred outline (`hw`, `hh`). Mitigation: build the block centred as today, then translate outline, pins and texts by the shift in one place at the end, on both sides.
- Hidden users of the 5 mm constant. Mitigation: the plan lists every use found by search (`block.rs`, `connector.rs`, `dynamicSymbol.ts`, `blockEditor.ts`, `BlockPinsEditor.vue`) and each gets a test.
