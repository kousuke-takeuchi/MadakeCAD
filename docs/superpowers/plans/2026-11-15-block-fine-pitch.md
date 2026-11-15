# Block Fine Pitch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A block symbol can opt in to a 2.5 mm pin pitch, an outline as small as 5 x 5 mm and pin numbers inside the outline, with every pin and outline edge still on the 2.5 mm grid and every existing block drawn exactly as before.

**Architecture:** Two new block attributes (`block_pitch`, `block_numbers`) are parsed into `BlockSpec`. The block is built centred as today; an outline shift and a per-side pin row shift (both 1.25 mm, fine pitch only) bring it onto the grid. Rust (`madake-core/src/block.rs`) is the source of truth; `src/canvas/dynamicSymbol.ts` mirrors it and both are checked against one shared golden file. `FORMAT_VERSION` goes to 22.

**Tech Stack:** Rust (serde), TypeScript (Vue 3, Vitest), Canvas2D.

**Spec:** `docs/superpowers/specs/2026-11-15-block-fine-pitch-design.md`

## Global Constraints

- A block without `block_pitch` and `block_numbers` produces the same `SymbolDef` as before: existing tests, `tests/golden/connector-blocks.json` and the known export id `block_25f4c16dc8d5fc46` are not edited.
- Every pin connection point and every outline edge of every block is a multiple of 2.5 mm relative to the instance origin.
- Allowed values: `block_pitch` = `"5"` | `"2.5"`; `block_numbers` = `"outside"` | `"inside"`. Anything else is an error.
- Fine pitch: size step 2.5 mm, minimum outline 5 x 5 mm, a side with `n` pins needs `(n + 1) × 2.5` mm (minimum 5 mm). Lead 2.5 mm and pin text 2.0 mm are unchanged.
- `block_pitch: 2.5` with a pin that has `connector` is refused.
- Every edit to a document goes through a `Command`. No new command is needed: the attributes travel through the existing ones.
- Every test has its specification sentences: Rust `///` two lines (English, then Japanese) before `#[test]`; Vitest `// ja: …` on the line before `it("English…")`. After adding tests: `python3 scripts/gen_spec.py`, commit the regenerated `docs/13-specification*.md`.
- Tests use invented blocks and values only. Nothing read or measured from the confidential reference drawing goes into code, tests, commit messages or committed documents. `python3 scripts/audit_confidential.py` prints `no confidential terms found` before every commit.
- Commit messages in Japanese with a conventional prefix; no `Co-Authored-By` or any other attribution or session trailer.
- Superpowers documents (specs, plans) are English only. Other documents keep the English + `.ja.md` pair where a pair exists.
- No new control in the block editor (UI design comes later in `MadakeCAD.pen`).
- Run Rust tests with `export PATH="$HOME/.cargo/bin:$PATH" MADAKE_USER_FRAME_TEMPLATES_PATH=<an empty folder>`.

## Review Focus

1. A fine block whose left and right sides have different pin-count parity (for example 2 pins left, 3 pins right): each side is shifted on its own and all five pins are on the grid.
2. A fine block rotated 90/180/270 or mirrored: the pin points returned by `place_symbol` / `get_pins` are on the grid and wires drawn to them connect.
3. An explicit `block_size` that is not a multiple of 2.5 (for example `{"w":6,"h":8}`) on a fine block: rounded up to 7.5 x 10, then checked against the pins.
4. `block_numbers: inside` with long pin numbers on both sides and a 5 mm wide `block_size`: the outline is widened so the numbers do not overlap; the pins stay on the grid after widening (the widened width can change the outline shift).
5. A fine block edited in the block editor: `block_pitch` and `block_numbers` survive, and adding a pin that no longer fits reports the needed size computed with the fine rule, not the 5 mm rule.

## File Structure

| File | Responsibility |
|---|---|
| `src-tauri/crates/madake-core/src/block.rs` | Attributes, `BlockPitch`, `BlockNumbers`, size rules, shifts, inside numbers, tests, shared golden writer |
| `src-tauri/crates/madake-core/tests/golden/fine-blocks.json` | Shared cases: attributes → primitives and pins (new, written by the Rust test) |
| `src-tauri/crates/madake-core/src/model.rs`, `io.rs` | `FORMAT_VERSION = 22`, version tests |
| `src-tauri/crates/madake-mcp/src/lib.rs` | Tool descriptions mention the two attributes |
| `src/canvas/dynamicSymbol.ts` (+ test) | TypeScript twin of the block rules |
| `src/canvas/blockEditor.ts` (+ test), `src/components/BlockPinsEditor.vue` | Keep the attributes; size step and minimum by pitch |
| `docs/internal/data-model.md`, `feature-inventory.md`, `specs/reproduction-gaps.md` + `.ja.md`, `docs/10-automation-api.md` + `.ja.md` | Documents |

Branch: `feature/block-fine-pitch`, created from `feature/stroke-font`.

---

### Task 1: Attributes and size rules (Rust)

**Files:**
- Modify: `src-tauri/crates/madake-core/src/block.rs` (constants near line 30, `BlockSpec` near line 99, `parse_block_spec` near line 169, `min_size` near line 290, `block_box` near line 384, tests module)

**Interfaces:**
- Produces: `pub const BLOCK_PITCH_ATTR: &str = "block_pitch"`, `pub const BLOCK_NUMBERS_ATTR: &str = "block_numbers"`, `pub const BLOCK_FINE_PITCH_MM: f64 = 2.5`, `pub enum BlockPitch { Coarse, Fine }` with `pub fn mm(self) -> f64` and `pub fn size_step(self) -> f64`, `pub enum BlockNumbers { Outside, Inside }`, fields `BlockSpec.pitch: BlockPitch`, `BlockSpec.numbers: BlockNumbers` (both `Default` = `Coarse` / `Outside`), `fn side_need(n: usize, pitch: BlockPitch) -> f64`.

- [ ] **Step 1: Write the failing tests** (in the `tests` module of `block.rs`; `attrs(&[("k","v")])` builds a `BTreeMap` — reuse the helper the module already has for that, or add this one):

```rust
fn fine(pins: &str, size: Option<&str>) -> BTreeMap<String, String> {
    let mut a = BTreeMap::new();
    a.insert(BLOCK_PINS_ATTR.to_string(), pins.to_string());
    a.insert(BLOCK_PITCH_ATTR.to_string(), "2.5".to_string());
    if let Some(s) = size {
        a.insert(BLOCK_SIZE_ATTR.to_string(), s.to_string());
    }
    a
}

/// A block without the pitch and number attributes is a 5 mm block with numbers outside, as before.
/// 間隔と番号の属性が無いブロックは、これまでどおり5mm間隔・番号は外側のブロックになる。
#[test]
fn a_block_without_the_new_attributes_is_coarse_with_numbers_outside() {
    let spec = parse_block_spec(&BTreeMap::new()).unwrap();
    assert_eq!((spec.pitch, spec.numbers), (BlockPitch::Coarse, BlockNumbers::Outside));
}

/// block_pitch accepts only "5" and "2.5", and block_numbers only "outside" and "inside"; anything else is refused with the attribute name in the message.
/// block_pitch は "5" と "2.5" だけ、block_numbers は "outside" と "inside" だけを受け付ける。それ以外は属性名を含むメッセージで断る。
#[test]
fn unknown_pitch_and_number_values_are_refused() {
    for (key, bad) in [(BLOCK_PITCH_ATTR, "3"), (BLOCK_PITCH_ATTR, "fine"), (BLOCK_NUMBERS_ATTR, "in"), (BLOCK_NUMBERS_ATTR, "")] {
        let mut a = BTreeMap::new();
        a.insert(key.to_string(), bad.to_string());
        let err = parse_block_spec(&a).unwrap_err();
        assert!(err.contains(key), "{key}={bad}: {err}");
    }
    let mut a = BTreeMap::new();
    a.insert(BLOCK_PITCH_ATTR.to_string(), " 2.5 ".to_string());
    a.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
    let spec = parse_block_spec(&a).unwrap();
    assert_eq!((spec.pitch, spec.numbers), (BlockPitch::Fine, BlockNumbers::Inside));
}

/// On a 2.5 mm block a side with n pins needs (n + 1) x 2.5 mm, at least 5 mm; a smaller block_size is refused and the message names the needed size.
/// 2.5mm間隔のブロックでは、ピンがn本の辺は (n + 1) × 2.5mm (最小5mm) 必要。それより小さい block_size は断り、必要な大きさをメッセージに出す。
#[test]
fn a_fine_block_size_must_hold_its_pins() {
    let two = r#"{"left":[{"no":"1"},{"no":"2"}]}"#;
    assert!(parse_block_spec(&fine(two, Some(r#"{"w":5,"h":7.5}"#))).is_ok());
    let err = parse_block_spec(&fine(two, Some(r#"{"w":5,"h":5}"#))).unwrap_err();
    assert!(err.contains("5x7.5"), "{err}");
    let top3 = r#"{"top":[{"no":"1"},{"no":"2"},{"no":"3"}]}"#;
    let err = parse_block_spec(&fine(top3, Some(r#"{"w":7.5,"h":5}"#))).unwrap_err();
    assert!(err.contains("10x5"), "{err}");
}

/// On a 2.5 mm block block_size is rounded up to the next 2.5 mm; on a 5 mm block it is still rounded up to 5 mm.
/// 2.5mm間隔のブロックの block_size は次の2.5mmへ切り上げる。5mm間隔のブロックはこれまでどおり5mmへ切り上げる。
#[test]
fn block_size_is_rounded_up_to_the_step_of_the_pitch() {
    let spec = parse_block_spec(&fine(r#"{"left":[{"no":"1"}]}"#, Some(r#"{"w":6,"h":8}"#))).unwrap();
    assert_eq!(spec.size, Some(BlockSize { w: 7.5, h: 10.0 }));
    let mut a = BTreeMap::new();
    a.insert(BLOCK_SIZE_ATTR.to_string(), r#"{"w":16,"h":11}"#.to_string());
    assert_eq!(parse_block_spec(&a).unwrap().size, Some(BlockSize { w: 20.0, h: 15.0 }));
}

/// A 2.5 mm block refuses a pin with a connector picture, naming the pin.
/// 2.5mm間隔のブロックは、コネクタの絵を持つピンを断り、そのピン番号を示す。
#[test]
fn a_fine_block_refuses_connector_pictures() {
    let err = parse_block_spec(&fine(r#"{"left":[{"no":"U1","connector":"usb_a_f"}]}"#, None)).unwrap_err();
    assert!(err.contains("U1") && err.contains(BLOCK_PITCH_ATTR), "{err}");
}

/// Without block_size a 2.5 mm block with pin numbers inside is as small as its pins allow: one pin gives 5 x 5 mm, two pins on the left give 5 x 7.5 mm.
/// block_size が無い2.5mm間隔・番号内側のブロックは、ピンが許す最小の大きさになる: 1本なら5×5mm、左に2本なら5×7.5mm。
#[test]
fn a_fine_block_sizes_itself_to_its_pins() {
    let mut a = fine(r#"{"left":[{"no":"1"}]}"#, None);
    a.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
    assert_eq!(block_box(&parse_block_spec(&a).unwrap()), (5.0, 5.0));
    let mut a = fine(r#"{"left":[{"no":"1"},{"no":"2"}]}"#, None);
    a.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
    assert_eq!(block_box(&parse_block_spec(&a).unwrap()), (5.0, 7.5));
}
```

`BlockSize` needs `PartialEq` for these asserts; add it to its derive list if missing.

- [ ] **Step 2: Run to verify they fail**

Run: `cd src-tauri && cargo test -p madake-core block::tests 2>&1 | tail -20`
Expected: compile errors (`BLOCK_PITCH_ATTR`, `BlockPitch` not found).

- [ ] **Step 3: Implement**

Constants and types (next to the existing constants):

```rust
/// ピン間隔を持つ属性キー (`"5"` か `"2.5"`。無ければ5)。
pub const BLOCK_PITCH_ATTR: &str = "block_pitch";
/// ピン番号を外形の外に描くか内に描くかを持つ属性キー (`"outside"` か `"inside"`。無ければ外)。
pub const BLOCK_NUMBERS_ATTR: &str = "block_numbers";
/// 細かいピン間隔 (mm)。グリッドと同じ。
pub const BLOCK_FINE_PITCH_MM: f64 = 2.5;
/// 細かい間隔のブロックの最小の外形 (mm、幅・高さとも)。
pub const BLOCK_FINE_MIN_MM: f64 = 5.0;
/// 番号を内側に描くとき、左右の番号のあいだに空けるすき間 (mm)。
pub const BLOCK_INSIDE_GAP_MM: f64 = 1.0;

/// ブロックのピン間隔。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockPitch {
    /// 5mm (これまでどおり)。
    #[default]
    Coarse,
    /// 2.5mm。
    Fine,
}

impl BlockPitch {
    /// ピン間隔 (mm)。
    pub fn mm(self) -> f64 {
        match self {
            BlockPitch::Coarse => BLOCK_PIN_PITCH_MM,
            BlockPitch::Fine => BLOCK_FINE_PITCH_MM,
        }
    }
    /// 外形寸法の刻み (mm)。
    pub fn size_step(self) -> f64 {
        match self {
            BlockPitch::Coarse => BLOCK_SIZE_STEP_MM,
            BlockPitch::Fine => BLOCK_FINE_PITCH_MM,
        }
    }
}

/// ピン番号を描く場所 (左右の辺)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockNumbers {
    /// 外形の外、リードの上 (これまでどおり)。
    #[default]
    Outside,
    /// 外形の内側、辺のすぐ内。ピン名は描かない。
    Inside,
}
```

`BlockSpec` gains `pub pitch: BlockPitch` and `pub numbers: BlockNumbers` (keep `#[derive(Default)]` working).

Size needed by one side:

```rust
/// ピンがn本の辺に要る長さ (mm)。5mm間隔は n×5 (最小5)、2.5mm間隔は (n+1)×2.5 (最小5)。
fn side_need(n: usize, pitch: BlockPitch) -> f64 {
    match pitch {
        BlockPitch::Coarse => (n as f64 * BLOCK_PIN_PITCH_MM).max(BLOCK_SIZE_STEP_MM),
        BlockPitch::Fine => ((n + 1) as f64 * BLOCK_FINE_PITCH_MM).max(BLOCK_FINE_MIN_MM),
    }
}
```

`min_size` takes the pitch: `fn min_size(pins: &BlockPins, pitch: BlockPitch) -> (f64, f64)` returning `(side_need(cols, pitch), side_need(rows, pitch))`.

In `parse_block_spec`, before the size is read:

```rust
let pitch = match attrs.get(BLOCK_PITCH_ATTR).map(|s| s.trim()) {
    None | Some("5") => BlockPitch::Coarse,
    Some("2.5") => BlockPitch::Fine,
    Some(other) => return Err(format!("{BLOCK_PITCH_ATTR} must be \"5\" or \"2.5\": {other:?}")),
};
let numbers = match attrs.get(BLOCK_NUMBERS_ATTR).map(|s| s.trim()) {
    None | Some("outside") => BlockNumbers::Outside,
    Some("inside") => BlockNumbers::Inside,
    Some(other) => return Err(format!("{BLOCK_NUMBERS_ATTR} must be \"outside\" or \"inside\": {other:?}")),
};
if pitch == BlockPitch::Fine {
    if let Some(pin) = pins.all().find(|p| p.connector.is_some()) {
        return Err(format!(
            "block pin {}: a connector picture cannot be used with {BLOCK_PITCH_ATTR} 2.5",
            pin.no.trim()
        ));
    }
}
```

Replace `BLOCK_SIZE_STEP_MM` by `pitch.size_step()` in the two `snap_up` calls of the size, call `min_size(&pins, pitch)`, and end with `Ok(BlockSpec { pins, size, pitch, numbers })`.

`block_box`:

```rust
/// 辺のピン番号のうち一番長いものの幅 (mm)。
fn max_number_width(side: &[BlockPin]) -> f64 {
    side.iter().map(|p| name_width(p.no.trim())).fold(0.0, f64::max)
}

/// 番号を内側に描くとき、左右の番号が収まる幅 (mm)。
fn inside_numbers_width(pins: &BlockPins) -> f64 {
    let both = !pins.left.is_empty() && !pins.right.is_empty();
    2.0 * BLOCK_TEXT_PAD_MM
        + max_number_width(&pins.left)
        + max_number_width(&pins.right)
        + if both { BLOCK_INSIDE_GAP_MM } else { 0.0 }
}
```

In `block_box`, the explicit-size branch becomes:

```rust
if let Some(size) = spec.size {
    let step = spec.pitch.size_step();
    let mut w = size.w.max(snap_up(shared_row_width(&spec.pins), step));
    if spec.numbers == BlockNumbers::Inside {
        w = w.max(snap_up(inside_numbers_width(&spec.pins), step));
    }
    return (w, size.h);
}
```

and the automatic branch keeps today's arithmetic for `Coarse` + `Outside` byte for byte. For the other combinations:

```rust
let p = spec.pitch.mm();
let (min_w, min_h) = match spec.pitch {
    BlockPitch::Coarse => (BLOCK_MIN_W_MM, BLOCK_MIN_H_MM),
    BlockPitch::Fine => (BLOCK_FINE_MIN_MM, BLOCK_FINE_MIN_MM),
};
let w_text = match spec.numbers {
    BlockNumbers::Outside => w_names_with(p), // today's w_names with `p` in place of BLOCK_PIN_PITCH_MM
    BlockNumbers::Inside => inside_numbers_width(pins),
};
let need_cols = match spec.pitch {
    BlockPitch::Coarse => cols as f64 * p + p,
    BlockPitch::Fine => side_need(cols, spec.pitch),
};
let need_rows = match spec.pitch {
    BlockPitch::Coarse => rows as f64 * p + p,
    BlockPitch::Fine => side_need(rows, spec.pitch),
};
let w = min_w.max(w_text).max(need_cols);
let h = min_h.max(need_rows).max(caption_inset(&pins.top) + caption_inset(&pins.bottom) + p);
(snap_up(w, spec.pitch.size_step()), snap_up(h, spec.pitch.size_step()))
```

Write it as one code path where `Coarse` + `Outside` evaluates to exactly the old expressions (the existing tests prove it).

- [ ] **Step 4: Run** `cargo test -p madake-core block` — all pass, including every pre-existing block test.

- [ ] **Step 5: Commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/audit_confidential.py
git add -A src-tauri/crates/madake-core/src/block.rs docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat: ブロックのピン間隔と番号の位置を属性で選べるようにする (大きさの規則)"
```

---

### Task 2: Geometry on the grid and numbers inside (Rust)

**Files:**
- Modify: `src-tauri/crates/madake-core/src/block.rs` (`offset` near line 135, `pin_layout`, `label_at`, `block_symbol_from_spec`, tests)
- Create: `src-tauri/crates/madake-core/tests/golden/fine-blocks.json` (written by the test with `MADAKE_UPDATE_GOLDEN=1`)

**Interfaces:**
- Consumes: `BlockSpec.pitch`, `BlockSpec.numbers`, `BlockPitch::mm`, `block_box` from Task 1.
- Produces: `fn axis_shift(size: f64, pitch: BlockPitch) -> f64`, `fn row_offsets(n: usize, size: f64, pitch: BlockPitch) -> Vec<f64>` (offsets in the centred frame, row shift included), golden file format `{"cases":[{"name","attrs","primitives","pins"}]}` (same shape as `connector-blocks.json`).

- [ ] **Step 1: Write the failing tests**

```rust
fn on_grid(v: f64) -> bool {
    let k = v / 2.5;
    (k - k.round()).abs() < 1e-9
}

fn outline_of(def: &SymbolDef) -> (f64, f64, f64, f64) {
    match &def.primitives[0] {
        Primitive::Rect { p1, p2, .. } => (p1.x, p1.y, p2.x, p2.y),
        other => panic!("先頭が外形でない: {other:?}"),
    }
}

fn left_pins(n: usize) -> String {
    let pins: Vec<String> = (1..=n).map(|i| format!(r#"{{"no":"{i}"}}"#)).collect();
    format!(r#"{{"left":[{}]}}"#, pins.join(","))
}

/// The outline and the pins of a 2.5 mm block follow the table of the design: a height that is an odd multiple of 2.5 mm moves the outline down by 1.25 mm, and a row of pins that would miss the grid moves down by 1.25 mm.
/// 2.5mm間隔のブロックの外形とピンは設計の表のとおりになる: 高さが2.5mmの奇数倍なら外形が1.25mm下へ寄り、グリッドから外れるピンの列は1.25mm下へ寄る。
#[test]
fn a_fine_block_shifts_its_outline_and_rows_onto_the_grid() {
    // (高さ, ピン数, 外形の上, 外形の下, ピンのy)
    let table: [(f64, usize, f64, f64, &[f64]); 6] = [
        (5.0, 1, -2.5, 2.5, &[0.0]),
        (7.5, 2, -2.5, 5.0, &[0.0, 2.5]),
        (7.5, 1, -2.5, 5.0, &[2.5]),
        (10.0, 3, -5.0, 5.0, &[-2.5, 0.0, 2.5]),
        (10.0, 2, -5.0, 5.0, &[0.0, 2.5]),
        (12.5, 4, -5.0, 7.5, &[-2.5, 0.0, 2.5, 5.0]),
    ];
    for (h, n, top, bottom, ys) in table {
        let size = format!(r#"{{"w":5,"h":{h}}}"#);
        let def = block_symbol(&fine(&left_pins(n), Some(&size))).unwrap();
        let (x1, y1, x2, y2) = outline_of(&def);
        assert_eq!((x1, y1, x2, y2), (-2.5, top, 2.5, bottom), "h={h} n={n}");
        let got: Vec<f64> = def.pins.iter().map(|p| p.at.y).collect();
        assert_eq!(got, ys, "h={h} n={n}");
        assert!(def.pins.iter().all(|p| p.at.x == -5.0), "接続点は外形から2.5mm: h={h} n={n}");
    }
}

/// Every pin and every outline edge of a 2.5 mm block is on the 2.5 mm grid, for every size from 5 to 20 mm and every pin count that fits, on all four sides, also when the two opposite sides have different pin counts.
/// 2.5mm間隔のブロックのピンと外形の辺は、5〜20mmのどの大きさ・収まるどのピン数・四辺のどれでも、向かい合う辺のピン数が違っても、2.5mmグリッドに乗る。
#[test]
fn every_fine_block_is_on_the_grid() {
    let sizes: Vec<f64> = (2..=8).map(|k| k as f64 * 2.5).collect();
    let side = |name: &str, n: usize, tag: &str| {
        let pins: Vec<String> = (1..=n).map(|i| format!(r#"{{"no":"{tag}{i}"}}"#)).collect();
        format!(r#""{name}":[{}]"#, pins.join(","))
    };
    let mut checked = 0;
    for &w in &sizes {
        for &h in &sizes {
            let max_rows = (h / 2.5) as usize - 1;
            let max_cols = (w / 2.5) as usize - 1;
            for (l, r) in [(0, max_rows), (max_rows, 0), (max_rows, max_rows.saturating_sub(1)), (1, max_rows)] {
                for (t, b) in [(0, max_cols), (max_cols, max_cols.saturating_sub(1))] {
                    let json = format!("{{{},{},{},{}}}", side("left", l, "L"), side("right", r, "R"), side("top", t, "T"), side("bottom", b, "B"));
                    let size = format!(r#"{{"w":{w},"h":{h}}}"#);
                    let mut a = fine(&json, Some(&size));
                    a.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
                    let def = block_symbol(&a).unwrap_or_else(|e| panic!("{w}x{h} {json}: {e}"));
                    let (x1, y1, x2, y2) = outline_of(&def);
                    for v in [x1, y1, x2, y2] {
                        assert!(on_grid(v), "外形 {v}: {w}x{h} {json}");
                    }
                    for p in &def.pins {
                        assert!(on_grid(p.at.x) && on_grid(p.at.y), "ピン {:?}: {w}x{h} {json}", p.at);
                        assert!(p.at.x < x1 || p.at.x > x2 || p.at.y < y1 || p.at.y > y2, "ピンが外形の中: {:?}", p.at);
                    }
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 300, "{checked}");
}

/// With numbers inside, the number of a left or right pin is drawn inside the outline next to its edge at the height of the pin, and pin names are not drawn.
/// 番号を内側に描くブロックでは、左右のピンの番号は外形の内側の辺のそばに、ピンと同じ高さで描かれ、ピン名は描かれない。
#[test]
fn numbers_inside_are_drawn_inside_and_names_are_not_drawn() {
    let mut a = fine(r#"{"left":[{"no":"1","name":"VCC"}],"right":[{"no":"2","name":"GND"}]}"#, Some(r#"{"w":10,"h":5}"#));
    a.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
    let def = block_symbol(&a).unwrap();
    let texts: Vec<(&str, f64, f64)> = def.primitives.iter().filter_map(|p| match p {
        Primitive::Text { at, text, .. } => Some((text.as_str(), at.x, at.y)),
        _ => None,
    }).collect();
    assert_eq!(texts.iter().map(|t| t.0).collect::<Vec<_>>(), ["1", "2"]);
    let (x1, _, x2, _) = outline_of(&def);
    assert!(texts[0].1 > x1 && texts[0].1 < 0.0 && texts[0].2 == 0.0, "{texts:?}");
    assert!(texts[1].1 < x2 && texts[1].1 > 0.0 && texts[1].2 == 0.0, "{texts:?}");
    assert_eq!(def.pins[0].name, "VCC", "名前はデータに残る");
}

/// A 5 mm wide block with long numbers inside on both sides is widened until the numbers fit, and its pins stay on the grid.
/// 幅5mmのブロックの両側に長い番号を内側へ描くと、番号が収まる幅まで広がり、ピンはグリッドに乗ったままになる。
#[test]
fn a_narrow_block_with_numbers_inside_is_widened() {
    let mut a = fine(r#"{"left":[{"no":"1234"}],"right":[{"no":"5678"}]}"#, Some(r#"{"w":5,"h":5}"#));
    a.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
    let def = block_symbol(&a).unwrap();
    let (x1, _, x2, _) = outline_of(&def);
    assert!(x2 - x1 > 5.0);
    assert!(on_grid(x1) && on_grid(x2) && def.pins.iter().all(|p| on_grid(p.at.x)));
}

/// Numbers inside also work on a 5 mm block, whose pins stay where they were.
/// 番号の内側表示は5mm間隔のブロックでも使え、ピンの位置は変わらない。
#[test]
fn numbers_inside_work_on_a_coarse_block_without_moving_pins() {
    let pins = r#"{"left":[{"no":"1"},{"no":"2"}]}"#;
    let mut plain = BTreeMap::new();
    plain.insert(BLOCK_PINS_ATTR.to_string(), pins.to_string());
    plain.insert(BLOCK_SIZE_ATTR.to_string(), r#"{"w":15,"h":10}"#.to_string());
    let mut inside = plain.clone();
    inside.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
    let (a, b) = (block_symbol(&plain).unwrap(), block_symbol(&inside).unwrap());
    assert_eq!(a.pins.iter().map(|p| p.at).collect::<Vec<_>>(), b.pins.iter().map(|p| p.at).collect::<Vec<_>>());
}

/// A 2.5 mm block and a 5 mm block with the same pins get different export names, and the sample block keeps the name "block".
/// 同じピンを持つ2.5mm間隔と5mm間隔のブロックは書き出し名が違い、見本のブロックの名前は "block" のまま。
#[test]
fn fine_and_coarse_blocks_get_different_export_ids() {
    let pins = r#"{"left":[{"no":"1"}]}"#;
    let mut coarse = BTreeMap::new();
    coarse.insert(BLOCK_PINS_ATTR.to_string(), pins.to_string());
    assert_ne!(block_export_id(&block_symbol(&coarse).unwrap()), block_export_id(&block_symbol(&fine(pins, None)).unwrap()));
    assert_eq!(block_export_id(&block_template()), "block");
}
```

Add the golden test next to `connector_blocks_match_the_shared_golden_file`, reusing its `rounded` helper and its file layout (`{"cases":[ ... ]}`, one case per line, fields `name`, `attrs`, `primitives`, `pins`):

```rust
const FINE_GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/fine-blocks.json");

/// (名前, 属性) の例: 表の6例、四辺にピンのある例、左右のピン数の偶奇が違う例、番号内側、5mm間隔で番号内側、名前あり。
fn fine_cases() -> Vec<(String, BTreeMap<String, String>)> {
    let mut cases = Vec::new();
    for (h, n) in [(5.0, 1), (7.5, 2), (7.5, 1), (10.0, 3), (10.0, 2), (12.5, 4)] {
        cases.push((format!("left-{n}-h{h}"), fine(&left_pins(n), Some(&format!(r#"{{"w":5,"h":{h}}}"#)))));
    }
    let four = r#"{"left":[{"no":"L1","name":"IN"},{"no":"L2"}],"right":[{"no":"R1"},{"no":"R2"},{"no":"R3"}],"top":[{"no":"T1"}],"bottom":[{"no":"B1"},{"no":"B2"}]}"#;
    cases.push(("four-sides-auto".into(), fine(four, None)));
    cases.push(("four-sides-sized".into(), fine(four, Some(r#"{"w":17.5,"h":12.5}"#))));
    let mut inside = fine(four, None);
    inside.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
    cases.push(("four-sides-inside".into(), inside));
    let mut narrow = fine(r#"{"left":[{"no":"1234"}],"right":[{"no":"5678"}]}"#, Some(r#"{"w":5,"h":5}"#));
    narrow.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
    cases.push(("inside-widened".into(), narrow));
    let mut coarse_inside = BTreeMap::new();
    coarse_inside.insert(BLOCK_PINS_ATTR.to_string(), four.to_string());
    coarse_inside.insert(BLOCK_NUMBERS_ATTR.to_string(), "inside".to_string());
    cases.push(("coarse-inside".into(), coarse_inside));
    cases
}

/// The 2.5 mm blocks and the blocks with numbers inside are drawn exactly as recorded in the golden file that the TypeScript twin is also checked against (rewrite it with MADAKE_UPDATE_GOLDEN=1).
/// 2.5mm間隔のブロックと番号内側のブロックは、TS 側も突き合わせる golden ファイルのとおりに描かれる (MADAKE_UPDATE_GOLDEN=1 で書き直す)。
#[test]
fn fine_blocks_match_the_shared_golden_file() {
    let lines: Vec<String> = fine_cases().into_iter().map(|(name, attrs)| {
        let def = block_symbol(&attrs).unwrap_or_else(|e| panic!("{name}: {e}"));
        let v = serde_json::json!({ "name": name, "attrs": attrs, "primitives": rounded(serde_json::to_value(&def.primitives).unwrap()), "pins": rounded(serde_json::to_value(&def.pins).unwrap()) });
        serde_json::to_string(&v).unwrap()
    }).collect();
    let fresh = format!("{{\"cases\":[\n{}\n]}}\n", lines.join(",\n"));
    if std::env::var("MADAKE_UPDATE_GOLDEN").as_deref() == Ok("1") {
        std::fs::write(FINE_GOLDEN, &fresh).expect("golden を書く");
    }
    let text = std::fs::read_to_string(FINE_GOLDEN).unwrap_or_else(|_| panic!("{FINE_GOLDEN} が無い: MADAKE_UPDATE_GOLDEN=1 を付けて作る"));
    assert_eq!(text, fresh, "golden と違う。意図した変更なら MADAKE_UPDATE_GOLDEN=1 で書き直す");
}
```

If the existing connector golden test compares parsed JSON instead of text, compare the same way it does.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p madake-core block::tests 2>&1 | tail -20`
Expected: the new geometry tests fail (pins at ±1.25, outline at ±3.75), the golden test fails with "が無い".

- [ ] **Step 3: Implement**

```rust
fn is_grid(v: f64) -> bool {
    let k = v / BLOCK_FINE_PITCH_MM;
    (k - k.round()).abs() < 1e-9
}

/// 外形をその軸でずらす量 (mm)。2.5mm間隔で、大きさが2.5mmの奇数倍のとき1.25、ほかは0。
/// ずらすと、原点がグリッドにあるかぎり外形の両端がグリッドに乗る。
fn axis_shift(size: f64, pitch: BlockPitch) -> f64 {
    if pitch == BlockPitch::Fine && !is_grid(size / 2.0) {
        BLOCK_FINE_PITCH_MM / 2.0
    } else {
        0.0
    }
}

/// 辺のピンn本の、外形の中心から測ったオフセット (mm)。中央揃えにしたあと、
/// 外形のずれを足してもグリッドに乗らない列は、列ごと1.25mm (下または右へ) 寄せる。
/// 5mm間隔のブロックでは寄せることが無く、これまでの `offset` と同じ値になる。
fn row_offsets(n: usize, size: f64, pitch: BlockPitch) -> Vec<f64> {
    if n == 0 {
        return Vec::new();
    }
    let first = -((n - 1) as f64) / 2.0 * pitch.mm();
    let nudge = if is_grid(first + axis_shift(size, pitch)) { 0.0 } else { BLOCK_FINE_PITCH_MM / 2.0 };
    (0..n).map(|i| first + i as f64 * pitch.mm() + nudge).collect()
}
```

In `block_symbol_from_spec`:

- Compute `let (w, h) = block_box(spec);` as today, then `let (sx, sy) = (axis_shift(w, spec.pitch), axis_shift(h, spec.pitch));`.
- Per side, compute `let offs = row_offsets(n, if left/right { h } else { w }, spec.pitch);` once and use `offs[i]` everywhere `offset(i, n)` was used inside the builder, including the `edges` of connector groups and the two uses inside `label_at` (pass `&offs` to `label_at`; replace its `BLOCK_PIN_PITCH_MM` by `spec.pitch.mm()`). Keep `pub(crate) fn offset` for `connector.rs`, which only runs for coarse blocks.
- `pin_layout` gets `numbers: BlockNumbers` and `pitch_mm: f64`. For `Inside` and `dir` Left/Right the number goes where the name goes today, using the number's width: `Point::new(-hw + BLOCK_TEXT_PAD_MM + name_width(no) / 2.0, off)` and the mirrored one for Right. The Up/Down branches use `pitch_mm / 2.0` in place of `BLOCK_PIN_PITCH_MM / 2.0`.
- When `spec.numbers == BlockNumbers::Inside`, do not push the name text primitive (the `PinDef.name` stays).
- At the very end, before building `SymbolDef`, translate when `(sx, sy) != (0, 0)`: every point of every primitive, every `PinDef.at`, and every `TextSlot.at` by `(sx, sy)`. Write `fn translate_primitive(p: &mut Primitive, dx: f64, dy: f64)` covering every `Primitive` variant with an exhaustive `match` (no `_` arm, so a new variant fails to compile here).

Then create the golden file:

Run: `MADAKE_UPDATE_GOLDEN=1 cargo test -p madake-core fine_blocks_match_the_shared_golden_file`

and open it once to check the first case by eye: outline `-2.5,-2.5 .. 2.5,2.5`, pin at `-5,0`.

- [ ] **Step 4: Run** `cargo test --workspace` — all pass; `git diff --stat src-tauri/crates/madake-core/tests/golden/connector-blocks.json` shows no change.

- [ ] **Step 5: Add the end-to-end connection test** in the test module that already places blocks through `Engine` and reads the netlist (search `block_pins` in `netlist.rs` tests; follow that module's helpers):

```rust
/// Wires drawn to the pins of a 2.5 mm block connect: two blocks placed on the grid, one of them turned 90 degrees, joined pin to pin by two wires, give two nets with two pins each and no unconnected pin.
/// 2.5mm間隔のブロックのピンへ引いた配線はつながる: グリッドに置いた2つのブロック (片方は90度回転) をピンどうし2本の配線で結ぶと、2ピンずつのネットが2つでき、未接続のピンは無い。
```

Body: place block A at (50, 50) and block B at (100, 50) rotated 90, both with attrs `block_pins = {"left":[{"no":"1"},{"no":"2"}]}` (B: `{"top":…}` is not needed — rotation does it), `block_pitch = 2.5`, `block_size = {"w":5,"h":7.5}`; read each pin's sheet position with the same function `place_symbol` uses to report pins; add two wires between corresponding pins with an L-shaped route; assert the netlist has two nets of two pins and `verify` reports no unconnected pin for A and B.

- [ ] **Step 6: Commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/audit_confidential.py
git add -A src-tauri/crates/madake-core docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat: 2.5mm間隔のブロックをグリッドに乗せ、番号を外形の内側に描けるようにする"
```

---

### Task 3: TypeScript twin

**Files:**
- Modify: `src/canvas/dynamicSymbol.ts` (constants near line 251, `parseBlockSpec` line 664, `blockBox` line 792, `pinLayout` line 844, `blockSymbol` line 927)
- Test: `src/canvas/dynamicSymbol.test.ts`

**Interfaces:**
- Consumes: `src-tauri/crates/madake-core/tests/golden/fine-blocks.json` from Task 2.
- Produces: exports `BLOCK_PITCH_ATTR`, `BLOCK_NUMBERS_ATTR`, `BLOCK_FINE_PITCH_MM`, `type BlockPitch = "coarse" | "fine"`, `type BlockNumbers = "outside" | "inside"`, `blockPitchMm(p: BlockPitch): number`, `blockSizeStep(p: BlockPitch): number`, `sideNeed(n: number, p: BlockPitch): number`; `parseBlockSpec` returns `{ pins, size, pitch, numbers }`; `blockBox(pins, size, pitch = "coarse", numbers = "outside")`.

- [ ] **Step 1: Write the failing tests**

```ts
import fineGolden from "../../src-tauri/crates/madake-core/tests/golden/fine-blocks.json";

describe("fine pitch blocks (Rust twin)", () => {
  // ja: golden ファイルの全例 (2.5mm間隔・番号内側) で、TS は Rust と同じプリミティブとピンを返す
  it("draws every block of the fine-pitch golden file exactly like Rust", () => {
    expect(fineGolden.cases.length).toBe(11);
    for (const c of fineGolden.cases) {
      const def = blockSymbol(c.attrs as Record<string, string>);
      expect({ name: c.name, primitives: rounded(def.primitives), pins: rounded(def.pins) }).toEqual({
        name: c.name,
        primitives: c.primitives,
        pins: c.pins,
      });
    }
  });

  // ja: block_pitch は "5" と "2.5" だけ、block_numbers は "outside" と "inside" だけを受け付ける
  it('accepts only "5" / "2.5" for block_pitch and "outside" / "inside" for block_numbers', () => {
    expect(() => parseBlockSpec({ block_pitch: "3" })).toThrow(/block_pitch/);
    expect(() => parseBlockSpec({ block_numbers: "in" })).toThrow(/block_numbers/);
    const spec = parseBlockSpec({ block_pitch: " 2.5 ", block_numbers: "inside" });
    expect([spec.pitch, spec.numbers]).toEqual(["fine", "inside"]);
    expect(parseBlockSpec({}).pitch).toBe("coarse");
  });

  // ja: 2.5mm間隔のブロックで、ピンが2本の辺に高さ5mmは小さすぎ、必要な大きさ 5x7.5 をメッセージに出す
  it("refuses a fine block_size that is too small and names the needed size", () => {
    const attrs = { block_pins: '{"left":[{"no":"1"},{"no":"2"}]}', block_pitch: "2.5", block_size: '{"w":5,"h":5}' };
    expect(() => parseBlockSpec(attrs)).toThrow(/5x7\.5/);
  });

  // ja: 2.5mm間隔のブロックは、コネクタの絵を持つピンを断る
  it("refuses a connector picture on a fine block", () => {
    const attrs = { block_pins: '{"left":[{"no":"U1","connector":"usb_a_f"}]}', block_pitch: "2.5" };
    expect(() => parseBlockSpec(attrs)).toThrow(/U1/);
  });
});
```

The case count (11) must equal the number of cases `fine_cases()` produced in Task 2; adjust the literal to the real count.

- [ ] **Step 2: Run** `npx vitest run src/canvas/dynamicSymbol.test.ts` — the new tests fail.

- [ ] **Step 3: Implement** the same rules as Tasks 1 and 2, function by function, with the same names in camelCase: `sideNeed`, `axisShift(size, pitch)`, `rowOffsets(n, size, pitch)`, `maxNumberWidth`, `insideNumbersWidth`, the error messages word for word (`block_pitch must be "5" or "2.5": …`, `block_numbers must be "outside" or "inside": …`, `block pin U1: a connector picture cannot be used with block_pitch 2.5`, and the existing too-small message with the new numbers), and the final translation of primitives, pins and text slots by `(sx, sy)`. The primitive translation switches on `type` with a `never` check in the default branch.

- [ ] **Step 4: Run** `npx vitest run` and `npx vue-tsc --noEmit` — all pass (the connector golden test of 59 cases is unchanged).

- [ ] **Step 5: Commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/audit_confidential.py
git add -A src/canvas docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat: キャンバスでも2.5mm間隔のブロックと番号の内側表示を描く"
```

---

### Task 4: Format version 22

**Files:**
- Modify: `src-tauri/crates/madake-core/src/model.rs` (`FORMAT_VERSION` and its history comment near line 60), `src-tauri/crates/madake-core/src/io.rs` (tests near line 419), any TypeScript constant that mirrors the version (search `21` next to `format_version` / `FORMAT_VERSION` in `src/`).

- [ ] **Step 1: Change the tests first.** Rename `the_current_format_version_is_twenty_one` to `the_current_format_version_is_twenty_two` with its two sentences, assert 22, and add:

```rust
/// A version 21 project file opens in this build and becomes version 22; its blocks keep their attributes and their 5 mm pins.
/// 形式21のプロジェクトファイルはこのビルドで開け、形式22になる。ブロックの属性と5mm間隔のピンはそのまま残る。
#[test]
fn a_version_21_file_opens_as_version_22() { /* build a Project with one block (block_pins with 2 left pins), set format_version = 21, serialise, load with the loader this module's other version tests use, assert format_version == 22 and the block's attrs are equal to the original */ }

/// A file of a newer version than this build is refused.
/// このビルドより新しい形式のファイルは断る。
#[test]
fn a_version_23_file_is_refused() { /* same, with format_version = 23; assert the loader returns an error */ }
```

Write the bodies with the helpers the neighbouring version tests use (`a_version_19_project_migrates_to_the_current_version` is the model). If a "newer version is refused" test already exists and is version-agnostic, do not add the second one.

- [ ] **Step 2: Run** — the renamed test fails (21 ≠ 22).

- [ ] **Step 3: Implement.** `FORMAT_VERSION = 22` and one history line:

```rust
/// - v22: ブロックのピン間隔と番号の位置 (属性 `block_pitch` / `block_numbers`)。形式21のビルドは版番号で断る
///   (2.5mm間隔のブロックを5mmで描くとピンが動き、配線が外れるため)。使っていないファイルは版番号以外これまでと同じ
```

- [ ] **Step 4: Run** `cargo test --workspace`, `npx vitest run` — fix every test that pins the number 21 by making it state the current version.

- [ ] **Step 5: Commit** — `feat: 保存形式を22へ上げる (ブロックのピン間隔)`.

---

### Task 5: Block editor keeps the attributes

**Files:**
- Modify: `src/canvas/blockEditor.ts` (`BlockDraft`, `attrsFromDraft` line 189, `draftFromAttrs` line 200, the size check near line 284), `src/components/BlockPinsEditor.vue` (lines 73-118)
- Test: `src/canvas/blockEditor.test.ts`

**Interfaces:**
- Consumes: `parseBlockSpec(...).pitch/.numbers`, `blockSizeStep`, `sideNeed`, `BLOCK_PITCH_ATTR`, `BLOCK_NUMBERS_ATTR` from Task 3.
- Produces: `BlockDraft.pitch: BlockPitch`, `BlockDraft.numbers: BlockNumbers`.

- [ ] **Step 1: Write the failing tests**

```ts
// ja: 2.5mm間隔・番号内側のブロックを編集して書き戻しても、2つの属性は残る
it("keeps block_pitch and block_numbers through an edit", () => {
  const attrs = { block_pins: '{"left":[{"no":"1"}]}', block_pitch: "2.5", block_numbers: "inside", block_size: '{"w":5,"h":5}' };
  const out = attrsFromDraft(draftFromAttrs(attrs));
  expect(out.block_pitch).toBe("2.5");
  expect(out.block_numbers).toBe("inside");
});

// ja: 属性の無いブロックを編集して書き戻しても、2つの属性は増えない
it("does not add the attributes to a block that has none", () => {
  const out = attrsFromDraft(draftFromAttrs({ block_pins: '{"left":[{"no":"1"}]}' }));
  expect("block_pitch" in out).toBe(false);
  expect("block_numbers" in out).toBe(false);
});

// ja: 2.5mm間隔のブロックでピンが収まらないとき、必要な大きさは2.5mm間隔の規則で出る (左に3本なら高さ10mm)
it("reports the needed size of a fine block with the fine rule", () => {
  const draft = draftFromAttrs({ block_pins: '{"left":[{"no":"1"},{"no":"2"},{"no":"3"}]}', block_pitch: "2.5" });
  draft.size = { w: 5, h: 7.5 };
  const issue = /* call the validation function of this module the way its existing sizeTooSmall test does */;
  expect(issue).toEqual({ code: "sizeTooSmall", w: 5, h: 10 });
});
```

Complete the third test with the validation call used by the existing `sizeTooSmall` test in this file.

- [ ] **Step 2: Run** `npx vitest run src/canvas/blockEditor.test.ts` — fail.

- [ ] **Step 3: Implement.** `draftFromAttrs` copies `pitch` and `numbers` from the parsed spec (when parsing fails, from the raw attributes if valid, else the defaults). `attrsFromDraft` writes `block_pitch = "2.5"` only when `draft.pitch === "fine"` and `block_numbers = "inside"` only when `draft.numbers === "inside"`. The size check uses `sideNeed(cols, draft.pitch)` / `sideNeed(rows, draft.pitch)`. In `BlockPinsEditor.vue`, replace `BLOCK_SIZE_STEP_MM` by `blockSizeStep(editor.pitch)` (expose `pitch` on the editor state object the component already uses for `size`). No template or style change.

- [ ] **Step 4: Run** `npx vitest run`, `npx vue-tsc --noEmit` — pass.

- [ ] **Step 5: Commit** — `feat: ブロックエディタが間隔と番号の属性を保ち、大きさを間隔の刻みで増減する`.

---

### Task 6: Documents, tool descriptions, final verification

**Files:**
- Modify: `src-tauri/crates/madake-mcp/src/lib.rs` (the `block_pins` doc comment near line 476 and the `place_symbol` description near line 1246), `docs/10-automation-api.md` + `.ja.md`, `docs/internal/data-model.md`, `docs/internal/feature-inventory.md`, `docs/internal/specs/reproduction-gaps.md` + `.ja.md`

- [ ] **Step 1: Tool descriptions.** Append to the `place_symbol` description (Japanese, like the rest): 「狭い間隔の機器・小さいコネクタは attrs.block_pitch="2.5" で2.5mm間隔にできる (外形は2.5mm刻み、最小5×5mm、ピンn本の辺は(n+1)×2.5mm以上、connector の絵は使えない)。attrs.block_numbers="inside" でピン番号を外形の内側に描く (ピン名は描かないので、信号名は配線のネットラベルで出す)」. If a test pins the description text or a schema snapshot, update it.

- [ ] **Step 2: Documents.**
  - `data-model.md`: header date line, `format_version: 22`, a row for 22 in the history table (what: the two block attributes; old files: open unchanged; a version-21 app refuses the file), and the block attributes where `block_pins` / `block_size` are described.
  - `feature-inventory.md`: one paragraph "ブロックの細かいピン間隔 (2026-11-15)" stating the rules of the spec and that the editor has no control for it yet.
  - `10-automation-api(.ja).md`: the two attributes next to `block_pins`.
  - Gap ledger, English and Japanese: a new section "G34 — Block pins are 5 mm apart and the outline cannot be smaller than 15 x 10 mm · required", in general terms only (dense harness drawings place conductors closer and use small connector boxes with the pin numbers inside; closed by `block_pitch` and `block_numbers`, format 22), plus a line in the summary table if the ledger has one per gap. No measured values.

- [ ] **Step 3: Full verification**

```bash
export PATH="$HOME/.cargo/bin:$PATH" MADAKE_USER_FRAME_TEMPLATES_PATH="$PWD/.superpowers/sdd/2026-11-15-stroke-font/empty-frames"
(cd src-tauri && cargo test --workspace)
npx vitest run
npx vue-tsc --noEmit
python3 -m unittest discover -s freecad-addon/tests
python3 scripts/gen_spec.py --check
python3 scripts/gen_licenses.py --check
python3 scripts/audit_confidential.py
```

Expected: everything passes; `git diff feature/stroke-font -- src-tauri/crates/madake-core/tests/golden/connector-blocks.json` is empty.

- [ ] **Step 4: Commit** — `docs: ブロックの細かいピン間隔をデータ設計・機能一覧・自動化API・ギャップ台帳へ反映`.

---

## After the plan (outside the repository, not part of any commit)

- `~/.claude/skills/drawing-madakecad-wiring/mdk_draw.py`: `block(..., pitch=5, numbers="outside")` writes the two attributes and computes `PINS` with the same outline shift and row shift (port `axis_shift` and `row_offsets`), and rounds `w`/`h` up to the step of the pitch.
- Check one fine block on the canvas of the development build (port 9320) and in an exported PDF before drawing region 2 with it.
