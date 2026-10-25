# Connector pictograms: what kind of connector a block pin is, drawn on the box edge

Date: 2026-10-25 · Status: design decisions approved by the user, spec ready for review · Japanese: [2026-10-25-connector-pictograms-design.ja.md](2026-10-25-connector-pictograms-design.ja.md)

## Goal

Let a block pin show **what kind of physical connector** it is, as a small monochrome picture drawn on the block's edge, so a wiring drawing reads like the real device: a USB hub with four USB-A sockets, an amplifier with a screw terminal block, a custom board with JST sockets (user: "端子台が付いているイラストのシンボル", "USBもシンボルとして見た目的に分かりやすい表現").

Success, with the AOGU drawing (applied after merge, see Delivery):

- Hub block `39` shows four USB-A sockets in a row on its top edge (`U1`–`U4`), a micro-B socket `PWR` on the bottom edge and the captive OTG cable as a micro-B plug `OTG` on the right edge.
- Amplifier `53` shows a 3.5 mm jack `IN` and one screw terminal body `TB1` around its `5V` and `GND` pins.
- Board `33` shows `J3` and `J9` as JST PH sockets labelled `J3 PH` / `J9 PH`; board `44` shows a JST XH socket.
- The same picture appears on the canvas, in SVG / PDF / PNG / `get_sheet_image`, in DXF and in KiCad export.
- Drawings without connectors look, export and save exactly as before (apart from the file format number).

## The problem today

A block pin is a lead and two texts. `block_symbol_from_spec` draws every pin the same way: a 2.5 mm lead from the box edge, the number above the lead outside the box, the name inside (`block.rs:309-357`). The only way to say "this is a USB-A socket" is the pin name (`USB-A メス`). On a drawing with a hub, a dongle, a DCDC, a battery and three custom boards, the reader has to read every name to see which pins are USB, which are screw terminals and which are JST headers, and whether a cable plugs in (socket) or is attached (plug).

The library mate symbols (`connector_plug` / `connector_socket`) are separate parts placed next to a pin; they do not describe the block's own receptacles and stay as they are.

## What exists today (verified in code)

- **Block pin table** (`madake-core/src/block.rs`): `BlockPin { no, name, contacts }` is `#[serde(deny_unknown_fields)]` (`block.rs:50-62`), as is `BlockPins` (`block.rs:64-76`). The table is the JSON **string** `attrs["block_pins"]`; Rust only deserialises it, never writes it back, so the saved text is whatever the editor or the agent wrote.
- **Validation**: `parse_block_spec` (`block.rs:149-197`) checks JSON, pin count (`BLOCK_PIN_MAX` = 200), empty / duplicate numbers and `check_contacts` (`block.rs:201-227`). Error texts are **English**, e.g. `block pin {no}: duplicate contact {c}`. `Engine` runs the same parse on add / update (`command.rs:1778` `validate_entity`), so a bad table is refused at command time. A block whose attributes do not parse resolves to no definition and is not drawn.
- **Constants** (`block.rs:29-48`): `BLOCK_PIN_PITCH_MM` 5.0, `BLOCK_LEAD_MM` 2.5, `BLOCK_PIN_TEXT_MM` 2.0, `BLOCK_TEXT_PAD_MM` 1.5, `BLOCK_SIZE_STEP_MM` 5.0, `BLOCK_MIN_W_MM` 15, `BLOCK_MIN_H_MM` 10.
- **Geometry** (`block_symbol_from_spec`, `block.rs:298-380`): outline = one `Rect` centred on the origin (`block.rs:301-305`). Pins are centred per side at 5 mm pitch (`offset`, `block.rs:119-121`). Left / right: pin point 2.5 mm outside, number at `(edge ∓ 1.25, off − 2.0)`, name inside starting 1.5 mm from the edge. Top / bottom: number **and** name inside, at 2.0 mm and 4.5 mm from the edge (`block.rs:327-336`). Lead = `Line` edge → pin point.
- **Box size** (`block_box`, `block.rs:246-267`): explicit `block_size` wins; otherwise width = max(15, 2 × 1.5 + widest left name + widest right name + 5, cols × 5 + 5) and height = max(10, rows × 5 + 5, `caption_inset(top)` + `caption_inset(bottom)` + 5), each rounded up to 5 mm. `caption_inset` (`block.rs:134-142`) is 0 / 5.0 / 7.5. The explicit-size minimum is `min_size` (`block.rs:230-237`): n × 5 per side. `block_box` has no other caller than the symbol builder and the editor's size display.
- **Primitives** (`symbol.rs:68-82`): `Line { pts }`, `Circle { center, r, filled }`, `Arc { center, r, start_deg, end_deg }` (clockwise on paper, Y down), `Rect { p1, p2, filled }`, `Text { at, text, height }` (always horizontal, centred on `at`). Serialised with `"type"` tags, the same shape the frontend uses.
- **Renderers**: SVG `render_symbol` (`svg.rs:385-455`) draws filled `Circle` / `Rect` with `fill="#000"` and unfilled with `fill="none"`, stroke `SYMBOL_STROKE` 0.3 mm (`svg.rs:21`); there is **no white fill**, so anything drawn under an unfilled shape shows through. PDF, PNG and `get_sheet_image` come from this SVG. DXF (`dxf.rs:420-459`): a filled `Rect` adds a `SOLID`; a filled `Circle` adds a square `SOLID` inscribed in the circle outline. KiCad (`kicad.rs:562-590`): filled → `(fill (type outline))`; pins are exported with length 0 (`kicad.rs:605`), so leads come only from our `Line`s. Canvas `drawSymbol` (`src/canvas/renderer.ts:405-476`): filled circles and rects are filled without a stroke, unfilled ones stroked.
- **Text extents**: `tidy.rs:210-213` measures a symbol `Text` as `at ± (text_width / 2, height / 2)`; `text_width` (`tidy.rs:108-114`) = height × (0.6 per narrow, 1.0 per wide character). TS twin: `textWidthMm` (`src/canvas/textWidth.ts:27-31`).
- **TS twin** (`src/canvas/dynamicSymbol.ts`): constants (`:226-249`), `BlockPin { no, name?, contacts? }` (`:254-259`), `checkContacts` (`:452-467`, same texts as Rust), `parseBlockSpec` (`:469-530`; it checks side keys but not pin keys), `blockBox` (`:533-551`), `blockSymbol` (`:557-`). Twin equality is enforced by tests with the same expected coordinates in `dynamicSymbol.test.ts` (`describe("blockSymbol")`, `:171`) and `block.rs` tests. Precedent for a shared golden file: `madake-core/tests/golden/*.svg` (`svg.rs:1587`), and TS tests import JSON from `src-tauri/` (`frameTemplate.test.ts:5`).
- **Editor**: `src/canvas/blockEditor.ts` (`BlockPinRow { no, name, contacts? }` `:40`, `isFilled` `:90`, `pinsOf` `:141`, `attrsFromDraft` `:162`, `draftFromAttrs` `:173`, `draftIssues` `:211`, `BlockIssue` `:53`); `src/components/BlockPinColumn.vue` (three inputs per row: number, name, contacts); issue texts in `BlockPinsEditor.vue:25-53`; `stores/blockEditor.ts:175` `updateRow`. `PropertiesPanel.vue:476-477` shows the pin count plus `portCount`. Six locales `src/locales/{en,ja,zh,fr,de,es}.json`, key parity checked by `src/i18n/i18n.test.ts`.
- **File format** 18 (`model.rs:60`). A block with contacts is written with the file-only entity kind `port_block` (`model.rs:612-616` `is_port_block`, `:649`, `:678`) while `io::save_project` holds `FileWriteGuard` (`io.rs:38-40`), so `v0.4.0` (no newer-format refusal) fails on the unknown kind instead of silently dropping the block; format-17 builds refuse by number (`io.rs:60` `check_format_version`). IPC / MCP keep `kind: "symbol"`.
- **MCP / agent**: `place_symbol` attrs doc (`madake-mcp/src/lib.rs:475-480`) and tool description (`lib.rs:1242`) describe `block_pins` and `contacts`; `knowledge.rs:65-71` has the port-pin rule. MCP tests for blocks live in `madake-mcp/tests/drawing_tools.rs:640-`.
- **Pin-group brackets** (`pin_groups.rs`) and the tidy metrics read `PinDef.at` and the primitives, so they follow a moved pin point without change.

## Design

### 1. Data

**Block pin JSON** (`block.rs`): two optional keys.

```rust
pub struct BlockPin {
    pub no: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub contacts: Vec<String>,
    /// Connector kind drawn on the box edge. None = a plain pin (drawn as today).
    #[serde(default)]
    pub connector: Option<ConnectorKind>,
    /// Name of a grouped connector (terminal block, JST socket), e.g. "J3", "TB1".
    #[serde(default)]
    pub connector_ref: Option<String>,
}
```

```json
{"top":[{"no":"1","name":"U1","connector":"usb_a_f","contacts":["VBUS","D-","D+","GND"]}],
 "left":[{"no":"4","name":"5V","connector":"screw","connector_ref":"TB1"},
         {"no":"5","name":"GND","connector":"screw","connector_ref":"TB1"}]}
```

**`ConnectorKind`** (new `madake-core/src/connector.rs`), `#[serde(rename_all = "snake_case")]`, fixed, no free text:

| serde name | Picture | Grouped | Typical use (user-confirmed) |
|---|---|---|---|
| `usb_a_f` | USB-A socket | no | hub ports, DCDC and battery USB outputs |
| `usb_a_m` | USB-A plug | no | LTE dongle |
| `usb_c_f` | USB-C socket | no | board 33 USB-C IN |
| `usb_c_m` | USB-C plug | no | |
| `micro_b_f` | micro-B socket | no | ReSpeaker, hub power |
| `micro_b_m` | micro-B plug | no | hub OTG cable end |
| `jack35_f` | 3.5 mm jack | no | mic 49, isolator 51, amp 53 input, X58 |
| `jack35_m` | 3.5 mm plug | no | |
| `dc_f` | DC jack | no | battery input |
| `dc_m` | DC plug | no | DCDC 12 V output |
| `screw` | screw terminal block | yes | amp 5V / GND, speaker outputs |
| `jst_ph_f` | JST PH socket | yes | board 33 J3 / J9 |
| `jst_xh_f` | JST XH socket | yes | board 44 |

`ConnectorKind::is_grouped()` is true for `screw`, `jst_ph_f`, `jst_xh_f`. `ConnectorKind::series()` is `Some("PH")` / `Some("XH")` for the JST kinds, `None` otherwise.

**Validation** (added to `parse_block_spec`, in a new `check_connectors(&BlockPins)` called after the per-pin loop; English messages in the style of `check_contacts`):

1. An unknown `connector` value fails deserialisation: `block_pins is not valid JSON: unknown variant ...` (serde's text; this is the existing JSON error path).
2. `connector_ref` is trimmed. Empty after trimming: `block pin {no}: connector_ref must not be empty`.
3. `connector_ref` without `connector`: `block pin {no}: connector_ref needs a grouped connector (screw, jst_ph_f, jst_xh_f)`.
4. `connector_ref` on a non-grouped kind: `block pin {no}: connector_ref is only for screw, jst_ph_f and jst_xh_f, not {kind}`.
5. A non-empty `connector_ref` that forms two groups (on two sides, or split by another pin, or with two different kinds): `block pin {no}: connector_ref {ref} is used by another group; its pins must be next to each other on one side`.

**Grouping** (one function `connector_groups(side: &[BlockPin]) -> Vec<Group>`, twin in TS): walking one side in table order, a pin with a grouped kind starts a group; the next pin joins it when it has the **same kind and the same trimmed `connector_ref`** (absent equals absent). Pins on different sides never group. A pin with a non-grouped kind is a group of one. Pins without `connector` are not in any group. Two neighbouring unnamed screw blocks therefore merge; giving them refs (`TB1`, `TB2`) splits them.

**Label** of a grouped connector: `<ref> PH` / `<ref> XH` for JST, `<ref>` for `screw`; JST without ref prints `PH` / `XH`; screw without ref prints nothing.

**Independence.** `connector` is drawing-only. It does not touch `contacts` (a USB port normally has both `connector: usb_a_f` and contacts `VBUS, D-, D+, GND`), the netlist, ERC, reports or BOM. `PinDef` does **not** gain a connector field; `get_pins` is unchanged (the agent reads the kind from `block_pins`).

**TS twin** (`src/canvas/connector.ts`): `export type ConnectorKind = "usb_a_f" | ... | "jst_xh_f"`, `CONNECTOR_KINDS` (the 13 in the table order), `isGroupedConnector(kind)`, `connectorSeries(kind)`, `connectorGroups(side)`. `BlockPin` in `dynamicSymbol.ts` gains `connector?: ConnectorKind; connector_ref?: string`. `parseBlockSpec` runs the same five checks in the same order with the same texts, except rule 1, which throws `block_pins is not valid JSON: unknown connector {value}` (serde's wording cannot be copied; tests compare only the `block_pins is not valid JSON` prefix for this case).

### 2. Geometry

All numbers in mm. Every picture is built in a **local frame** at the pin's **edge point** E (where the pin crosses the box outline): `u` points outward from the box, `v` runs along the edge. The frames are rotations of the right-side frame (V = U turned 90° clockwise on paper, Y down):

| Side | U (outward) | V | Rotation θ |
|---|---|---|---|
| right | (1, 0) | (0, 1) | 0° |
| bottom | (0, 1) | (−1, 0) | 90° |
| left | (−1, 0) | (0, −1) | 180° |
| top | (0, −1) | (1, 0) | 270° |

A local point (u, v) maps to E + u·U + v·V. Rect corners are mapped and then normalised to (min, max). Arc centres are mapped and angles become `start + θ`, `end + θ` (start reduced to [0, 360), the sweep kept), so clockwise stays clockwise. Because the frames are rotations, a "/" on the left and right sides is a "\" on top and bottom; that is accepted.

**New constants** (`connector.rs`, twin in `connector.ts`):

| Constant | Value | Meaning |
|---|---|---|
| `CONNECTOR_IN_MM` | 1.5 | body depth inside the box |
| `CONNECTOR_OUT_MM` | 2.5 | body depth outside the box |
| `CONNECTOR_HALF_SPAN_MM` | 2.0 | half the body length along the edge (body = 4 mm) |
| `CONNECTOR_LEAD_MM` | 5.0 | edge → pin point for pins with a connector |
| `CONNECTOR_NUM_GAP_MM` | 0.25 | body → pin number |
| `CONNECTOR_LABEL_TOP_MM` | 7.0 | top / bottom group label centre, inside the edge |
| `CONNECTOR_LABEL_INSET_MM` | 10.0 | `caption_inset` of a top / bottom side that prints a group label |

Fit at 5 mm pitch: neighbouring 4 mm bodies leave 1.0 mm between outlines, 0.7 mm visible with the 0.3 mm stroke. Auto-sized boxes leave ≥ 5 mm from the outermost pin to the corner and the explicit minimum leaves 2.5 mm, so a 2 mm half body never reaches a corner of its own side.

#### 2.1 Pictures (local coordinates)

Shared parts: **E-rect** = `Rect (−1.5, −2) – (2.5, 2)` unfilled (the female body, straddling the edge); **H-rect** = `Rect (−1.5, −2) – (0, 2)` unfilled (the male housing inside the box). Female = an open (white) body set into the edge; male = a **filled insert** from the edge outward to u = 2.5. All pictures end at u = 2.5; the lead (§2.2) starts there.

| Kind | Primitives, in this order |
|---|---|
| `usb_a_f` | E-rect; tongue `Rect (−0.8, −0.9) – (1.8, −0.1)` filled |
| `usb_a_m` | H-rect; insert `Rect (0, −1.2) – (2.5, 1.2)` filled |
| `usb_c_f` | E-rect; opening (stadium): `Line (−0.2, −0.8) – (1.2, −0.8)`, `Arc c(1.2, 0) r 0.8, −90° → 90°`, `Line (1.2, 0.8) – (−0.2, 0.8)`, `Arc c(−0.2, 0) r 0.8, 90° → 270°`; tongue `Line (0, 0) – (1.0, 0)` |
| `usb_c_m` | H-rect; insert `Rect (0, −0.8) – (1.7, 0.8)` filled; rounded tip `Circle c(1.7, 0) r 0.8` filled |
| `micro_b_f` | body `Line` closed: (−1.5, −2), (0, −2), (2.5, −1.4), (2.5, 1.4), (0, 2), (−1.5, 2), (−1.5, −2) (straight inside, trapezoid outside); tongue `Line (−0.6, 0) – (1.6, 0)` |
| `micro_b_m` | H-rect; insert `Rect (0, −0.6) – (2.5, 0.6)` filled |
| `jack35_f` | E-rect; hole `Circle c(0.5, 0) r 1.0`; centre pin `Circle c(0.5, 0) r 0.35` filled |
| `jack35_m` | H-rect; collar `Rect (0, −1.0) – (0.8, 1.0)` filled; tip `Rect (0.8, −0.4) – (2.5, 0.4)` filled |
| `dc_f` | E-rect; hole `Circle c(0.5, 0) r 1.5`; centre pin `Circle c(0.5, 0) r 0.6` filled |
| `dc_m` | H-rect; hollow barrel as two walls `Rect (0, −1.4) – (2.5, −0.5)` filled, `Rect (0, 0.5) – (2.5, 1.4)` filled |
| `screw` (group) | body `Rect (−1.5, v₀ − 2) – (2.5, v₁ + 2)`; per pin at vᵢ: `Circle c(0.5, vᵢ) r 1.2`, slash `Line (1.3, vᵢ − 0.8) – (−0.3, vᵢ + 0.8)` |
| `jst_ph_f`, `jst_xh_f` (group) | body `Rect (−1.5, v₀ − 2) – (2.5, v₁ + 2)`; mating face `Line (1.5, v₀ − 2) – (1.5, v₁ + 2)`; per pin a contact `Rect (−0.6, vᵢ − 0.6) – (0.6, vᵢ + 0.6)` unfilled |

For a group, E is the first pin's edge point, vᵢ = (Eᵢ − E)·V, v₀ = min vᵢ, v₁ = max vᵢ; a single-kind pin has v₀ = v₁ = 0. Inner details keep ≥ 0.3 mm from the body outline (≥ 0.15 mm visible with the stroke). PH and XH differ only by the label.

Two pictures differ from the approved mockups because only black fill exists: the DC plug's "white hole at the tip" becomes a hollow barrel (two filled walls with the lead entering between them), and the USB-C rounded rectangle is drawn with two arcs. The mockups' white body fill is replaced by gaps in the box outline (§2.3).

#### 2.2 Pins, numbers, names

For a pin **with** a connector (any kind):

- **Pin point** `PinDef.at` = E + 5.0·U (`CONNECTOR_LEAD_MM`), instead of 2.5. Existing wires to that pin no longer touch it and must be re-attached (accepted by the user).
- **Lead** = `Line` E + 2.5·U → E + 5.0·U (from the picture's outer end, not from the edge).
- **Number**, w = `text_width(no, 2.0)`; `off` is the pin's offset along its side (`offset(i, n)`), hw / hh the half box size:
  - left / right: x = −(hw + 2.75 + w / 2) on the left, hw + 2.75 + w / 2 on the right; y = off − 2.0 (above the lead, the same height as today). Its inner end is always 0.25 mm beyond the body, so a long number grows outward, never over its own body.
  - top / bottom: y = −(hh + 3.75) on top, hh + 3.75 at the bottom (in the 2.5 mm of lead beyond the body); x = off + 0.5 + w / 2 (right of the lead on paper).
- **Name**, nw = its width:
  - left / right: starts `BLOCK_TEXT_PAD_MM + CONNECTOR_IN_MM` = 3.0 mm inside the edge (was 1.5): x = −hw + 3.0 + nw / 2 on the left, hw − 3.0 − nw / 2 on the right; y = off.
  - top / bottom: unchanged (4.5 mm inside, centred on the pin); its text box (3.5–5.5 mm inside) is already 2 mm clear of the 1.5 mm body, and keeping it lines it up with plain pins on the same side.

Pins **without** a connector are drawn exactly as today, also in a block that has connectors on other pins.

**Group label** (`Text`, height 2.0, w = its width):

- left / right, group of n ≥ 2 pins: in the gap between pin k and pin k + 1 of the group, k = ⌊(n − 1) / 2⌋ (0-based), i.e. y = y(first pin) + 2.5 + 5k; aligned like the names (starts 3.0 mm inside the edge and runs inward: x = −hw + 3.0 + w / 2 on the left, hw − 3.0 − w / 2 on the right). The gap row is free: names sit at ±1 mm around their pins (0.5 mm clear), numbers are outside the box.
- left / right, group of 1 pin: on the pin's own row, after its name: start = 3.0 + nw + 1.0 (or 3.0 when the name is empty).
- top / bottom: centred on the midpoint of the group's pins, 7.0 mm inside the edge (band 6.0–8.0, below the names' band 3.5–5.5).

#### 2.3 Box outline

A block with **no** connector keeps its single outline `Rect` (primitives identical to today). A block with **any** connector draws the outline as `Line` segments with a **gap** wherever a picture crosses the edge (the picture's v-range [v₀ − 2, v₁ + 2] mapped onto the edge). This is what makes a female opening look white. Order: top (x ascending), right (y ascending), bottom (x ascending), left (y ascending); each segment a 2-point `Line`; zero-length segments are skipped. Male housings and group bodies close their gap with their own outline.

#### 2.4 Box size (`block_box` / `blockBox`)

Unchanged when no pin has a connector. Otherwise:

- per side pad = `BLOCK_TEXT_PAD_MM` + (1.5 if any pin on that side has a connector);
- per side text width (left / right) = max over the side of: name width; for a 1-pin labelled group, name width + 1.0 + label width (label width alone when the name is empty); for an n ≥ 2 labelled group, label width;
- width term = pad(left) + pad(right) + text(left) + text(right) + 5;
- `caption_inset(top/bottom)` = 10.0 when the side prints a group label, else today's rule.

The 5 mm lead lies outside the box and does not change its size (the symbol's bounds, which include pin points, grow by 2.5 mm on that side as they should). The explicit-size minimum (`min_size`) is unchanged.

#### 2.5 No-overlap rule

Model: a text is the box `at ± (w/2, h/2)` of `tidy.rs:210-213`, a picture is the axis-aligned bounds of its primitives, a lead is its segment; touching edges are allowed, a positive-area intersection is an overlap. Guarantees, within one side and between the left and right sides: (a) no two texts (numbers, names, group labels) overlap; (b) no text overlaps a picture or a lead; (c) connector-pin numbers and group labels do not cross the box outline (plain-pin numbers wider than 2.5 mm already cross it today and still do). This holds for any pin numbers on left / right, and for top / bottom pin numbers up to 4.0 mm wide (three narrow characters; today's top / bottom numbers already meet their neighbours above 5 mm). Top / bottom group labels are guaranteed not to meet each other when each is no wider than its body ((n − 1) × 5 + 4). Corners where a top / bottom side meets a left / right side keep today's behaviour (names can already meet there), and a hand-set minimum `block_size` with connectors on two sides of one corner can make two bodies touch at that corner; both are accepted.

One case is "touching": a plain pin's number whose pin sits 5 mm below a connector pin has its text box touching the body box; the glyphs sit on the baseline, about 0.4 mm clear of the body on paper.

### 3. Rendering path

- **`madake-core/src/connector.rs`** (new, `pub mod connector`): `ConnectorKind`, the constants, `connector_groups`, `group_label`, `connector_primitives(kind, dir: PinDir, edges: &[Point]) -> Vec<Primitive>` (§2.1 for one group or single pin, given its pins' edge points; no leads, no texts) and `outline_segments(pins: &BlockPins, hw, hh) -> Vec<Primitive>` (§2.3).
- **`block.rs`**: `BlockPin` fields, `check_connectors`, `block_box` (§2.4), `block_symbol_from_spec`: when any pin has a connector, the outline lines of §2.3; then per side, per pin in table order: when the pin starts a group, that group's picture primitives and then its label `Text`; then the pin's lead, number and name (as today, with §2.2 positions for connector pins). `BlockSpec::has_connectors()`.
- **`src/canvas/connector.ts`** (new) and **`dynamicSymbol.ts`** (`BlockPin`, `parseBlockSpec`, `blockBox`, `blockSymbol`): the same, in the same order.
- **No renderer changes.** SVG / PDF / PNG / `get_sheet_image`, DXF, KiCad and the canvas draw the new primitives with their existing code. Accepted differences, all existing behaviour: DXF draws a filled circle as a square `SOLID` inside its outline (`dxf.rs:420-436`), and the canvas fills filled shapes without a stroke (0.15 mm smaller each side than SVG).

### 4. File format 19

- `FORMAT_VERSION` 19 with a history line in `model.rs`. Format-17 and format-18 builds refuse the file by number. A file without connectors is written byte for byte as before apart from the version (the `block_pins` text is stored as written, and nothing else changes).
- `is_port_block` (`model.rs:613`) becomes true for a block with any contact **or any connector**, so a block with connectors is also saved as the file-only kind `port_block` and `v0.4.0` (which has no newer-format check and whose `BlockPin` is `deny_unknown_fields`) refuses the file instead of silently dropping the block. The kind keeps its name for compatibility with format-18 files. IPC / MCP stay `symbol`.
- Old files open unchanged (no connectors).

### 5. UI

- **`BlockPinColumn.vue`**: two more cells per row after *Contacts*: **Connector** (`<select>`: an empty "none" option plus the 13 kinds in table order, labels from i18n) and **Group name** (`connector_ref`, text, placeholder `e.g. J3, TB1`), enabled only when the selected kind is grouped. The column labels row gains the two labels. The dialog widens to fit (CSS only).
- **`blockEditor.ts`**: `BlockPinRow` gains `connector?: ConnectorKind | ""` and `connectorRef?: string`. `isFilled` also counts a set connector or ref. `pinsOf` writes `connector` when set and `connector_ref` when its trimmed text is non-empty (keys after `contacts`); `draftFromAttrs` reads them back, so attrs → draft → attrs round-trips. `updateRow` (store) clears `connectorRef` when the connector changes to none or to a non-grouped kind. `draftIssues` gains `connectorRefNotGrouped { side, index }` and `connectorRefSplit { ref }` (rules 3 / 4 and 5 of §1; rule 2 cannot occur because an empty ref is not written).
- **Locales** (all six; `i18n.test.ts` checks key parity): `blockEditor.colConnector`, `blockEditor.colConnectorRef`, `blockEditor.connectorRefPlaceholder`, `blockEditor.connectorNone`, `blockEditor.connector.<kind>` × 13, `blockEditor.issue.connectorRefNotGrouped`, `blockEditor.issue.connectorRefSplit`. English / Japanese labels: USB-A socket / USB-A メス, USB-A plug / USB-A オス, USB-C socket / USB-C メス, USB-C plug / USB-C オス, micro-B socket / micro-B メス, micro-B plug / micro-B オス, 3.5 mm jack / 3.5mm ジャック, 3.5 mm plug / 3.5mm プラグ, DC jack / DC ジャック, DC plug / DC プラグ, Screw terminals / 端子台 (ネジ), JST PH socket / JST PH メス, JST XH socket / JST XH メス; column names Connector / コネクタ, Group name / まとめ名. zh / fr / de / es get translations of the same.
- **Properties panel**: unchanged (the summary stays pins + ports; the picture is on the drawing itself).
- The preview in the editor uses `blockSymbol`, so it shows the pictures with no extra work.

### 6. MCP, agent, docs

- `place_symbol`: the attrs doc (`lib.rs:475-480`) and the tool description (`lib.rs:1242`) add: "a pin may have `connector` (one of `usb_a_f usb_a_m usb_c_f usb_c_m micro_b_f micro_b_m jack35_f jack35_m dc_f dc_m screw jst_ph_f jst_xh_f`; `_f` = socket, `_m` = plug) to draw that connector on the box edge; consecutive `screw` / `jst_*` pins with the same `connector_ref` (e.g. `J3`) form one body; a connector pin's point is 5 mm from the box, so read pin positions with `get_pins`". `get_pins`, `connect_pins` and the netlist tools are unchanged.
- `knowledge.rs`: one bullet next to the port rule: "Give a block pin that is a real connector its `connector` kind (USB / 3.5 mm / DC: `_f` socket on the device, `_m` plug); put terminal blocks and JST sockets as consecutive pins with `screw` / `jst_ph_f` / `jst_xh_f` and one `connector_ref`. `connector` is only the picture: keep `contacts` for the conductors. Read pin positions with `get_pins` after adding connectors."
- Docs (EN + JA): `docs/03-schematic-editor(.ja).md` (block bullet: connector column, kinds, groups), `docs/10-automation-api(.ja).md` (`block_pins` `connector` / `connector_ref`), `docs/05-wire-management(.ja).md` (port-pin paragraph: connector is independent of contacts; format 19; connector blocks also saved as `port_block`), `docs/internal/feature-inventory.md`. `docs/13-specification(.ja).md` regenerated by `python3 scripts/gen_spec.py`.

## Testing

Rust tests carry the two doc lines (English, then Japanese) above `#[test]`; Vitest tests carry a `// ja:` line above `it(...)`. Regenerate `docs/13-specification*.md` with `python3 scripts/gen_spec.py`.

- **Twin golden** `madake-core/tests/golden/connector-blocks.json`: for each of the 13 kinds × 4 sides, the `primitives` and `pins` of a block with that connector between two plain pins (grouped kinds with 2 pins and a ref), plus the AOGU hub block (§Goal). A Rust test compares `block_symbol` output (coordinates rounded to 1e-6) with the file and rewrites it when `MADAKE_UPDATE_GOLDEN=1`; `dynamicSymbol.test.ts` imports the same file and compares `blockSymbol` output the same way. This is the Rust ↔ TS equality check for every kind and side.
- `connector.rs`: each kind's primitives at the left side match §2.1 exactly (hand-written expectations, so the golden file cannot drift silently); frame rotation (a point at (1, 0.5) lands at the four expected paper points; an arc keeps its sweep); every picture stays inside u ∈ [−1.5, 2.5], v ∈ [v₀ − 2, v₁ + 2].
- `block.rs` validation: unknown kind refused; empty / ref-without-connector / ref-on-`usb_a_f` / split ref (two sides, interrupted, different kinds) refused with the texts of §1; absent refs may repeat; `connector` together with `contacts` accepted.
- `block.rs` grouping: same kind + same ref group; absent = absent; different ref, different kind, or another pin in between splits; pins on different sides never group.
- `block.rs` geometry: a connector pin's point is 5 mm out, its lead 2.5 → 5.0, its number's inner end 2.75 mm out (left / right) or right of the lead (top / bottom), its left / right name starts 3.0 mm in; a plain pin in the same block is unchanged; the outline has gaps exactly at the pictures and is a single `Rect` without connectors; group labels (`J3 PH`, `PH`, `TB1`, none for unnamed screw) at the §2.2 positions for n = 1, 2, 3 and on top.
- `block.rs` size: `block_box` unchanged for every existing test table; width grows by 1.5 per side with connectors and by a label wider than the names; `caption_inset` 10.0 with a top label.
- **No-overlap** (`block.rs` and the twin): for a block with every kind on left and right (mixed with plain pins, numbers of 1–4 characters, names), a hub with four `usb_a_f` on top, a JST group of 1 and of 3, and screw groups, no two text boxes overlap and no text box overlaps another pin's picture, lead or outline segment (§2.5).
- Renderers: an SVG of a block with `usb_a_m` contains a black-filled polygon; DXF has a `SOLID` for it; KiCad has `(fill (type outline))`; output of a block without connectors is unchanged (existing golden / tests keep passing).
- `io.rs` / `model.rs`: format 19; a block with only connectors (no contacts) is saved as `port_block` and loads back with its connectors; a file without connectors is unchanged apart from the version; a format-18 file opens unchanged; IPC JSON keeps `kind: "symbol"`.
- `command.rs`: `add_entity` / `update_entity` with a bad connector table is refused.
- MCP (`drawing_tools.rs`): `place_symbol` with `connector: usb_a_f` returns the pin 5 mm from the box; an unknown kind and a ref on `usb_a_f` are refused with the §1 text; `get_sheet_image` of such a block succeeds.
- Agent (`madake-agent/tests/knowledge.rs`): the prompt mentions `connector` and `connector_ref`.
- Vitest `blockEditor.test.ts`: connector / ref round trip (attrs → draft → attrs identical); ref cleared when the kind becomes non-grouped; `connectorRefNotGrouped`, `connectorRefSplit`; a row with only a connector is filled (and reports `emptyNo`). `dynamicSymbol.test.ts`: same validation as Rust; golden equality. `i18n.test.ts`: key parity (existing test).

**Review focus candidates:** the §2.5 overlap rule and its test model; outline gaps vs. the female look in SVG and on the canvas; the frame/arc angle mapping (twin equality on arcs); format 19 + `port_block` extension; that a block without connectors produces byte-identical primitives, SVG and files.

## Delivery

One PR to `master` from `feature/connector-pictograms` (user rule: one PR per change), built as separate commits:

1. Core: `connector.rs`, `BlockPin` fields + validation + grouping, geometry and outline gaps, `block_box`, format 19 + `port_block` extension, golden file, Rust tests.
2. TS twin: `connector.ts`, `dynamicSymbol.ts`, golden equality and validation tests.
3. UI: editor columns, store, issues, six locales.
4. MCP descriptions, `knowledge.rs`, docs EN + JA, spec regeneration.

**Out of scope:** library symbols (`connector_plug` / `connector_socket` mates stay as they are); the `MadakeCAD.pen` design file; colour; filling `contacts` automatically from the kind; any electrical meaning of the kind (no ERC for plug ↔ socket mismatch, no netlist or BOM change); pictures on non-block symbols.

**Follow-up (not in this PR):** apply connectors to the AOGU drawing (hub 39, dongle, DCDC, battery, 33, 44, 49, 51, 53, X58) and re-attach the wires to the moved pin points; update the `drawing-madakecad-wiring` skill (kept outside this repository) with the connector rule.

## Open questions

None. The rulings below are the author's; each states what it costs if the user disagrees.

## Decision log

| Date | Decision | Why |
|---|---|---|
| 2026-10-25 | User decision: a block pin can show its physical connector kind as a small monochrome picture on the block edge | The drawing should read like the real device (hub, amp terminal block, JST sockets on boards 33 / 44). |
| 2026-10-25 | User decision: style B, the connector body straddles the box edge; screw terminals and JST draw one body across their consecutive pins | Chosen from mockups A / B / C (style.html, kinds.html). |
| 2026-10-25 | User decision: female = open (white) body set into the edge; male = black filled insert sticking out | One rule for every kind (kinds.html, kinds-dc.html). |
| 2026-10-25 | User decision: 13 fixed kinds, serde snake_case, no free text (`usb_a_f` … `jst_xh_f`) | Covers every connector in the AOGU drawing (uses confirmed per device). |
| 2026-10-25 | User decision: optional `connector` and `connector_ref`; a ref only on `screw`, `jst_ph_f`, `jst_xh_f`, refused elsewhere | Only grouped connectors have a name of their own. |
| 2026-10-25 | User decision: group = consecutive pins on one side with the same kind and ref (absent = absent); label `<ref> PH/XH`, `<ref>` for screw, `PH`/`XH` alone, nothing for an unnamed screw block | Approved section 1 + 3. |
| 2026-10-25 | User decision: `connector` is drawing-only, independent of `contacts` and nets | A USB port carries both the picture and its four contacts. |
| 2026-10-25 | User decision: pins without a connector draw exactly as today | Existing drawings stay as they are. |
| 2026-10-25 | User decision: format 19; files without connectors are byte-identical apart from the version | Old builds would silently drop a block with an unknown key; they must refuse clearly. |
| 2026-10-25 | User decision: pitch 5 mm; body 4 mm along the edge, 1.5 mm inside and 2.5 mm outside; 5 mm lead with the number on the 2.5 mm beyond the body; names move inward by the inside depth; `block_box` accounts for it | Approved section 2 (layout.html, dc-hub.html). |
| 2026-10-25 | User decision: adding a connector moves that pin point 2.5 mm outward; wires are re-attached by hand (AOGU by the coordinator) | Accepted cost of the longer lead. |
| 2026-10-25 | User decision: one Rust module `connector.rs` + one TS twin, twin tests; only existing primitives, no colour; all renderers get it for free | Approved section 3. |
| 2026-10-25 | User decision: block editor gets a Connector select and a Group name column, six locales | Approved section 3. |
| 2026-10-25 | User decision: MCP descriptions and the agent rule list the kinds and usage; docs EN + JA | Approved section 3. |
| 2026-10-25 | User decision: out of scope: library symbols, MadakeCAD.pen, colour; AOGU drawing applied after merge | Approved section 3. |
| 2026-10-25 | User decision: hub layout: four USB-A F on one long side (U1–U4), micro-B F power on the front side, the 3-position switch as drawing text only, the captive OTG cable as a `micro_b_m` pin on the right | From the user's photo of the hub. |
| 2026-10-25 | User decision: one PR; EN canonical docs + `.ja.md`; record decisions in this log | User process rules. |
| 2026-10-25 | Author ruling: the text (1.5 mm in, 2.5 mm out, number on the lead beyond the body) wins over layout.html / dc-hub.html, which draw 2.5 mm in and 1.5 mm out | decisions.md states the numbers explicitly. Cost if wrong: swap `CONNECTOR_IN_MM` / `CONNECTOR_OUT_MM`, move the details, regenerate the golden file. |
| 2026-10-25 | Author ruling: with any connector, the box outline is drawn as line segments with gaps at the pictures | There is no white fill; without gaps the edge would cross every female opening. Cost if wrong: none found that reads `primitives[0]` of a block; reverting means one `Rect` again. |
| 2026-10-25 | Author ruling: a male is an unfilled housing inside the box (u −1.5 … 0) plus a filled insert outside (0 … 2.5) | Keeps every picture inside the approved 4 × 4 mm envelope. Cost if wrong: redraw the five male kinds. |
| 2026-10-25 | Author ruling: `dc_m` is a hollow barrel (two filled walls), not a black barrel with a white hole; USB-C openings use arcs | No white fill exists. Cost if wrong: redraw one kind. |
| 2026-10-25 | Author ruling: `micro_b_f` body is straight inside the box and a trapezoid outside | The body then meets the edge at ±2 mm like every other kind, so the outline gap is the same rule. Cost if wrong: a full trapezoid needs a per-kind gap width. |
| 2026-10-25 | Author ruling: local frames are rotations of the right-side frame (a "/" slash becomes "\" on top and bottom) | One mapping for all sides, no mirrored arcs. Cost if wrong: add a mirror for two sides. |
| 2026-10-25 | Author ruling: the number's inner end is 0.25 mm beyond the body (left / right, above the lead) or right of the lead (top / bottom) | A long number then never covers its own body. Cost if wrong: numbers move by a constant. |
| 2026-10-25 | Author ruling: top / bottom names of connector pins stay at 4.5 mm inside | They were already clear of the 1.5 mm body; moving them would break the row with plain pins. Cost if wrong: one offset. |
| 2026-10-25 | Author ruling: group labels go in a free gap row (n ≥ 2), after the name (n = 1), or 7.0 mm inside on top / bottom with `caption_inset` 10.0; box width counts labels | The only places that are free of numbers and names at 5 mm pitch. Cost if wrong: move the label, keep the overlap test. |
| 2026-10-25 | Author ruling: validation messages are English like `check_contacts` | The dispatch asked for Japanese messages "in the style of check_contacts", but those are English; the editor shows translated issues instead. Cost if wrong: translate five strings in Rust and TS. |
| 2026-10-25 | Author ruling: an empty ref is refused; a ref may not form two groups; absent refs may repeat | Same strictness as contacts; a split `J3` is most likely a mistake. Cost if wrong: drop rule 5 if a split connector must be drawn as two bodies. |
| 2026-10-25 | Author ruling: blocks with connectors are also saved as `port_block` | Format 19 stops format-17/18 builds only; `v0.4.0` would otherwise drop the block silently. Cost if wrong: one condition. |
| 2026-10-25 | Author ruling: `PinDef` and `get_pins` do not carry the kind; Properties summary unchanged | Drawing-only data stays in `block_pins`. Cost if wrong: add an optional field later without a format change. |
| 2026-10-25 | Author ruling: a shared golden JSON file proves Rust ↔ TS equality for every kind and side | 13 × 4 hand-copied expectations would drift. Cost if wrong: none; hand tests are kept for the left side. |
| 2026-10-25 | Author ruling: corners of a hand-set minimum `block_size` with connectors on both sides, wide top / bottom labels and top / bottom numbers over 4 mm are not guaranteed overlap-free | Same as today's limits for names; guarding them would enlarge `min_size`. Cost if wrong: add margins to `min_size`. |
| 2026-10-25 | Author ruling: DXF's square fill for filled circles and the canvas's strokeless fill are accepted | Existing renderer behaviour, same for every symbol. Cost if wrong: renderer change outside this feature. |
