# Stroke Font (KST32B bundled) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Draw all text of a project's sheets with a bundled CAD stroke font (KST32B) when the project selects it, in SVG, PDF and on the canvas, without moving any geometry.

**Architecture:** Rust owns the font: `madake-core/src/stroke_font.rs` embeds the original `KST32B.TXT`, parses it once into glyphs keyed by Unicode character, and lays out strings. `svg.rs` routes its two text writers through an active-font scope that project-level entry points open from `Project.text_font`; PDF follows because it is converted from SVG. The frontend receives the parsed glyphs through IPC / Link API and only lays them out (`src/canvas/strokeFont.ts`); `drawingText` strokes them. Text width estimates used for geometry (`tidy::text_width`, `textWidthMm`) are not touched.

**Tech Stack:** Rust (serde, schemars, encoding_rs 0.8 — already in `Cargo.lock`), TypeScript (Vue 3, Vitest), Canvas2D, axum (Link API), Tauri IPC.

**Spec:** `docs/superpowers/specs/2026-11-14-stroke-font-design.md`

## Global Constraints

- A project without `text_font` produces exactly the same SVG as before. The golden tests `the_jis_a3_template_reproduces_the_previous_frame_exactly` and `an_empty_sheet_reproduces_the_previous_frame_exactly` and their golden files are never edited.
- `tidy::text_width` (Rust) and `textWidthMm` (TypeScript) are not changed. Selecting a stroke font never moves a pin, a symbol outline or a wire.
- Every edit to a document goes through a `Command`.
- Bundled font files are copied unmodified from the KST32B 3.0 archive: `KST32B.TXT`, `KST32B.RDM`, `KST32B.MAN`.
- Numbers shared by both renderers: capital height = `0.72 × size`; half-width advance 15 grid units, full-width 30; stroke width `max(0.07 × size, 0.13)` mm, × 1.5 for bold; round caps and joins.
- Every test has its specification sentences: Rust `///` two lines (English, then Japanese) before `#[test]`; Vitest `// ja: …` on the line before `it("English…")`. After adding tests: `python3 scripts/gen_spec.py`, commit the regenerated `docs/13-specification*.md`.
- Tests use invented strings and values only. Nothing read or measured from the confidential reference drawing goes into code, tests, commit messages or committed documents.
- `python3 scripts/audit_confidential.py` prints `no confidential terms found` before every commit.
- Commit messages in Japanese with a conventional prefix; no `Co-Authored-By` or any other attribution or session trailer.
- Superpowers documents (specs, plans) are English only.
- Run Rust tests with `export PATH="$HOME/.cargo/bin:$PATH" MADAKE_USER_FRAME_TEMPLATES_PATH=<an empty folder>`.

## Review Focus

1. A string that mixes characters with and without glyphs (for example an emoji or a CJK character outside JIS X 0208): the whole string falls back to the system font, nothing is dropped silently.
2. Rotated text (frame cells, net labels on vertical wires): strokes rotate with the text because they are written inside the same transform.
3. `text_latin: full` with an ASCII character whose full-width form has no glyph: that character uses its half-width glyph instead of failing the string.
4. Empty strings and strings of spaces: no `<path d="">` is written, and a string of spaces still advances.
5. A second export in the same thread after a project with a stroke font: the active font does not leak into a project without one (the scope restores the previous state on drop, also on early return).

## File Structure

| File | Responsibility |
|---|---|
| `src-tauri/resources/stroke_fonts/kst32b/KST32B.{TXT,RDM,MAN}` | Original font data and documentation (new) |
| `src-tauri/crates/madake-core/src/stroke_font.rs` | Parse CSF/1, registry, Unicode mapping, layout, serialisable glyph data (new) |
| `src-tauri/crates/madake-core/src/model.rs` | `TextFont`, `Latin`, `Project.text_font`, `FORMAT_VERSION = 21` |
| `src-tauri/crates/madake-core/src/command.rs` | `Command::SetTextFont`, `PatchOp::TextFontChanged` |
| `src-tauri/crates/madake-core/src/frame_template.rs` | `FrameTemplate.text_latin`, `Cell.spacing` + validation |
| `src-tauri/crates/madake-core/src/svg.rs` | Active-font scope, stroke text writer, `text_advance` |
| `src-tauri/crates/madake-core/src/pdf.rs`, `report_sheet.rs` | Open the scope at project-level entry points |
| `src-tauri/crates/madake-mcp/src/link_api.rs`, `src-tauri/src/lib.rs` | `GET /api/v1/stroke-fonts/{id}`, IPC `get_stroke_font` |
| `src/ipc.ts` | Types and `getStrokeFont` in both implementations |
| `src/canvas/strokeFont.ts` (+ test) | Active font, layout, `textAdvanceMm` (new) |
| `src/canvas/textSize.ts`, `renderer.ts`, `frameTemplate.ts`, `src/components/CanvasView.vue`, `src/stores/document.ts` | Drawing and wiring |
| `scripts/gen_licenses.py`, `docs/internal/third-party-licenses.md` | Bundled fonts in the licence inventory |

Branch: `feature/stroke-font` (exists, holds the spec).

---

### Task 1: Bundle KST32B and parse it

**Files:**
- Create: `src-tauri/resources/stroke_fonts/kst32b/KST32B.TXT`, `KST32B.RDM`, `KST32B.MAN`
- Create: `src-tauri/crates/madake-core/src/stroke_font.rs`
- Modify: `src-tauri/crates/madake-core/src/lib.rs` (`pub mod stroke_font;`), `src-tauri/crates/madake-core/Cargo.toml` (`encoding_rs`)

**Interfaces:**
- Produces: `pub fn parse_csf1(def: &[u8]) -> Vec<Vec<(u8, u8)>>`; `pub fn parse_kst(data: &[u8]) -> BTreeMap<u16, Vec<Vec<(u8, u8)>>>`; `pub fn address_to_char(addr: u16) -> Option<(char, bool)>` (bool = full width); `pub struct Glyph { pub strokes: Vec<Vec<(u8, u8)>>, pub wide: bool }`; `pub struct StrokeFont { pub id: &'static str, pub name: &'static str, pub glyphs: HashMap<char, Glyph>, pub baseline: u8, pub cap_top: u8, pub address_count: usize }`; `pub fn fonts() -> Vec<&'static StrokeFont>`; `pub fn font(id: &str) -> Option<&'static StrokeFont>`.

The archive is at `docs/references/repro/a0/font-trial/KST32Bv3/` (git-ignored copy of `KST32Bv3.zip` from Vector, `https://ftp.vector.co.jp/69/13/114/KST32Bv3.zip`). Copy the three files byte for byte.

CSF/1, from the header of `KST32B.TXT` (lines starting with `*` are comments; a record is a 4-digit hex address, one space, the definition, terminated by a space or end of line; pen starts at (0, 0); y grows upwards; a move starts a new stroke, a stroke with one point is dropped):

| Bytes | Meaning |
|---|---|
| `0x20` | terminator |
| `0x21–0x26`, `0x28–0x3F` | move to x = 0–5, 6–29 |
| `0x40–0x5B`, `0x5E–0x5F` | draw to x = 0–27, 28–29 |
| `0x60–0x7D` | next x = 0–29 (applied by the following y command) |
| `0x7E`, `0xA1–0xBF` | move to y = 0, 1–31 |
| `0xC0–0xDF` | draw to y = 0–31 |
| `0x27`, `0x5C`, `0x5D` | reserved (ends the definition) |

Address → character: `0x0021–0x007E` ASCII (half); `0x00A1–0x00DF` → U+FF61 + (a − 0xA1) (half); `0x1A20–0x1A7F` → ISO 8859-15 byte `a − 0x1A20 + 0xA0` (half); `0x2121–0x7E7E` → EUC-JP bytes `[hi | 0x80, lo | 0x80]` decoded with `encoding_rs::EUC_JP` (full). Anything else, or a byte pair that does not decode to exactly one character, → `None`. Space (U+0020, half) and U+3000 (full) are added with no strokes when the font lacks them.

- [ ] **Step 1: Write the failing tests** (in `stroke_font.rs`, `#[cfg(test)] mod tests`)

```rust
    /// Each kind of CSF/1 command moves or draws the pen as the format describes, and a space ends the definition.
    /// CSF/1の各命令は形式のとおりにペンを動かすか線を引き、空白で定義が終わる。
    #[test]
    fn csf1_commands_move_and_draw_the_pen() {
        // move x5, move y8 → start (5,8); next x11, draw y25 → (11,25)
        assert_eq!(parse_csf1(b"&\xA9k\xD9"), vec![vec![(5, 8), (11, 25)]]);
        // draw x10 then draw x28 along y8
        assert_eq!(parse_csf1(b"&\xA9J^"), vec![vec![(5, 8), (10, 8), (28, 8)]]);
        // move x6 (0x28) at y0; move y0 (0x7E) restarts; draw y31 (0xDF)
        assert_eq!(parse_csf1(b"(~\xDF"), vec![vec![(6, 0), (6, 31)]]);
        // a lone move makes no stroke; the terminator stops parsing
        assert_eq!(parse_csf1(b"&\xA9J \xC0"), vec![vec![(5, 8), (10, 8)]]);
        assert_eq!(parse_csf1(b"!"), Vec::<Vec<(u8, u8)>>::new());
    }

    /// Font records are read by their hexadecimal address, comment lines are skipped, and the bundled font has 7514 records.
    /// フォントの行は16進の番地で読み、注釈の行は飛ばす。同梱フォントの行は7514件。
    #[test]
    fn kst_records_are_read_by_address() {
        let table = parse_kst(b"**** comment\n0041 &\xA9k\xD9 \n* x\n0042 (~\xDF\n");
        assert_eq!(table.len(), 2);
        assert_eq!(table[&0x41], vec![vec![(5, 8), (11, 25)]]);
        assert_eq!(font("kst32b").unwrap().address_count, 7514);
    }

    /// Addresses map to characters: ASCII and half-width kana are half width, ISO 8859-15 is shifted into its own block, and JIS X 0208 cells are full width.
    /// 番地は文字に対応する。ASCIIと半角カナは半角、ISO 8859-15は専用の領域、JIS X 0208の区点は全角。
    #[test]
    fn addresses_map_to_unicode() {
        assert_eq!(address_to_char(0x0041), Some(('A', false)));
        assert_eq!(address_to_char(0x00B1), Some(('ｱ', false)));
        assert_eq!(address_to_char(0x1A24), Some(('€', false)));
        assert_eq!(address_to_char(0x3021), Some(('亜', true)));
        assert_eq!(address_to_char(0x5021), Some(('弌', true)));
        assert_eq!(address_to_char(0x2341), Some(('Ａ', true)));
        assert_eq!(address_to_char(0x0000), None);
        assert_eq!(address_to_char(0x2F21), None);
    }

    /// The bundled font is registered as kst32b, covers JIS level 1 and level 2 kanji, kana and Latin letters, and has a space; capitals stand on row 3 and reach row 28.
    /// 同梱フォントはkst32bとして登録され、第1・第2水準の漢字、かな、英字と空白を持つ。大文字は3行目に立ち28行目に届く。
    #[test]
    fn the_bundled_font_is_registered() {
        assert_eq!(fonts().iter().map(|f| f.id).collect::<Vec<_>>(), vec!["kst32b"]);
        assert!(font("missing").is_none());
        let f = font("kst32b").unwrap();
        for c in ['A', 'z', '0', 'ｱ', 'あ', 'ア', '亜', '弌', 'Ａ', ' ', '\u{3000}'] {
            assert!(f.glyphs.contains_key(&c), "{c}");
        }
        assert!(f.glyphs.len() >= 7400, "{}", f.glyphs.len());
        assert!(!f.glyphs[&'A'].wide && f.glyphs[&'亜'].wide);
        assert_eq!((f.baseline, f.cap_top), (3, 28));
    }
```

- [ ] **Step 2: Run to verify they fail** — `cargo test -p madake-core --lib stroke_font` → compile error (module missing).

- [ ] **Step 3: Implement**

```rust
//! ストロークフォント (線で描く字)。同梱は KST32B 3.0 (坂 直純 氏、フリーウェア: 使用・配付・改変・再配付自由、無保証)。
//! 形式 CSF/1 の解析と、文字 (Unicode) をキーにした字形の表、文字列の配置を持つ。
//! 字形の座標は 30x32 の格子で、y は上向き。半角の字は左の15列を使う。

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

const KST32B: &[u8] = include_bytes!("../../../resources/stroke_fonts/kst32b/KST32B.TXT");

/// 1つの字形。
#[derive(Debug, Clone, PartialEq)]
pub struct Glyph {
    pub strokes: Vec<Vec<(u8, u8)>>,
    /// 全角 (送り30) か半角 (送り15) か。
    pub wide: bool,
}

/// 解析済みのストロークフォント。
#[derive(Debug)]
pub struct StrokeFont {
    pub id: &'static str,
    pub name: &'static str,
    pub glyphs: HashMap<char, Glyph>,
    /// 半角の大文字が立つ行と、届く行 (Hの字形から測る)。
    pub baseline: u8,
    pub cap_top: u8,
    /// フォントファイルの字の行数 (番地の数)。
    pub address_count: usize,
}

/// CSF/1 の定義1つを線の並びへ。
pub fn parse_csf1(def: &[u8]) -> Vec<Vec<(u8, u8)>> {
    let (mut x, mut y, mut next_x) = (0u8, 0u8, None::<u8>);
    let mut strokes = Vec::new();
    let mut cur: Vec<(u8, u8)> = vec![(0, 0)];
    let mut flush = |cur: &mut Vec<(u8, u8)>, start: (u8, u8)| {
        if cur.len() > 1 {
            strokes.push(std::mem::take(cur));
        }
        *cur = vec![start];
    };
    for &b in def {
        match b {
            0x21..=0x26 | 0x28..=0x3F => {
                x = if b <= 0x26 { b - 0x21 } else { b - 0x28 + 6 };
                flush(&mut cur, (x, y));
            }
            0x40..=0x5B | 0x5E..=0x5F => {
                x = if b <= 0x5B { b - 0x40 } else { b - 0x5E + 28 };
                cur.push((x, y));
            }
            0x60..=0x7D => next_x = Some(b - 0x60),
            0x7E | 0xA1..=0xBF => {
                y = if b == 0x7E { 0 } else { b - 0xA0 };
                if let Some(nx) = next_x.take() { x = nx; }
                flush(&mut cur, (x, y));
            }
            0xC0..=0xDF => {
                y = b - 0xC0;
                if let Some(nx) = next_x.take() { x = nx; }
                cur.push((x, y));
            }
            _ => break, // 0x20 (終わり) と予約 (0x27, 0x5C, 0x5D)
        }
    }
    flush(&mut cur, (0, 0));
    strokes
}
```

(If the borrow checker rejects the `flush` closure borrowing `strokes` while `cur` is passed separately, make `flush` a small nested `fn flush(strokes: &mut Vec<..>, cur: &mut Vec<..>, start)`.)

`parse_kst`: split on `\n`, strip `\r`, skip lines shorter than 6 bytes or starting with `*`, parse `line[0..4]` as hex `u16` (skip on error), `parse_csf1(&line[5..])`.

`address_to_char` as described above (use `encoding_rs::EUC_JP.decode_without_bom_handling_and_without_replacement(&[hi | 0x80, lo | 0x80])` and `encoding_rs::ISO_8859_15.decode_without_bom_handling(&[b]).0`; accept only a result of exactly one `char`).

`build(id, name, data)`: `parse_kst`, map every address through `address_to_char` into `glyphs` (first one wins), add space and U+3000 if missing, then `baseline`/`cap_top` = min/max `y` over the strokes of `'H'`. `fonts()` / `font(id)` build KST32B once with `OnceLock<StrokeFont>` (`id: "kst32b"`, `name: "KST32B 3.0"`).

Add `encoding_rs = "0.8"` to `madake-core/Cargo.toml`.

- [ ] **Step 4: Run to verify they pass** — `cargo test -p madake-core --lib stroke_font` → 4 passed. If `address_count` or `(baseline, cap_top)` differ, print the real values once and check the font file before changing the test: these numbers were measured from the archive on 2026-11-14.

- [ ] **Step 5: Commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/audit_confidential.py
git add src-tauri/resources/stroke_fonts src-tauri/crates/madake-core/src/stroke_font.rs src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-core/Cargo.toml src-tauri/Cargo.lock docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat: ストロークフォントKST32Bを同梱し、CSF/1形式を文字ごとの字形へ読む"
```

---

### Task 2: Layout and serialisable glyph data

**Files:**
- Modify: `src-tauri/crates/madake-core/src/stroke_font.rs`, `src-tauri/crates/madake-core/src/model.rs` (`Latin`)

**Interfaces:**
- Consumes: Task 1.
- Produces: in `model.rs`: `#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)] #[serde(rename_all = "snake_case")] pub enum Latin { #[default] Half, Full }`. In `stroke_font.rs`: `pub const CAP_HEIGHT_RATIO: f64 = 0.72; pub const HALF_ADVANCE: f64 = 15.0; pub const FULL_ADVANCE: f64 = 30.0;` `pub struct StrokeText { pub polylines: Vec<Vec<(f64, f64)>>, pub width: f64 }`; `impl StrokeFont { pub fn layout(&self, text: &str, size: f64, latin: Latin, spacing: f64) -> Option<StrokeText>; pub fn data(&self) -> StrokeFontData }`; `pub fn stroke_width(size: f64, bold: bool) -> f64`; `#[derive(Serialize, schemars::JsonSchema)] pub struct StrokeFontData { pub id: String, pub baseline: u8, pub cap_top: u8, pub glyphs: BTreeMap<String, GlyphData> }`, `pub struct GlyphData { pub wide: bool, pub strokes: Vec<Vec<u8>> }` (strokes flattened `x0, y0, x1, y1, …`).

Layout rules: unit `u = 0.72 × size / (cap_top − baseline)`; each character: with `Latin::Full`, ASCII `'!'..='~'` → U+FF01.. and `' '` → U+3000, falling back to the half-width glyph when the full-width one is missing; a character without any glyph → `None` for the whole string. A glyph point `(gx, gy)` → `(pen + gx·u, −(gy − baseline)·u)` (origin at the baseline start, y down). Advance = `15·u` or `30·u`, times `spacing`. `width` = sum of advances, minus `(spacing − 1)` × the last advance (spacing is between letters). Stroke width `max(0.07 × size, 0.13)`, × 1.5 when bold.

- [ ] **Step 1: Write the failing tests**

```rust
    /// A half-width capital is 0.72 of the font size high and advances 0.432 of it; a full-width character advances twice as far.
    /// 半角の大文字の高さはフォントサイズの0.72倍、送りは0.432倍。全角の字は2倍送る。
    #[test]
    fn layout_scales_glyphs_to_the_font_size() {
        let f = font("kst32b").unwrap();
        let t = f.layout("H", 10.0, Latin::Half, 1.0).unwrap();
        let ys: Vec<f64> = t.polylines.iter().flatten().map(|p| p.1).collect();
        let (lo, hi) = ys.iter().fold((f64::MAX, f64::MIN), |(a, b), y| (a.min(*y), b.max(*y)));
        assert!((lo + 7.2).abs() < 1e-9 && hi.abs() < 1e-9, "{lo} {hi}");
        assert!((t.width - 4.32).abs() < 1e-9);
        assert!((f.layout("亜", 10.0, Latin::Half, 1.0).unwrap().width - 8.64).abs() < 1e-9);
        assert!((f.layout("AB", 10.0, Latin::Half, 1.0).unwrap().width - 8.64).abs() < 1e-9);
    }

    /// With full-width Latin letters ASCII is drawn with the font's full-width forms; spacing widens the gaps between letters but not after the last one.
    /// 英数字を全角にするとASCIIはフォントの全角の字で描かれる。字間は文字の間だけを広げ、最後の字の後ろは広げない。
    #[test]
    fn full_width_latin_and_spacing() {
        let f = font("kst32b").unwrap();
        assert!((f.layout("AB", 10.0, Latin::Full, 1.0).unwrap().width - 17.28).abs() < 1e-9);
        assert!((f.layout("A B", 10.0, Latin::Full, 1.0).unwrap().width - 25.92).abs() < 1e-9);
        assert!((f.layout("AB", 10.0, Latin::Half, 1.5).unwrap().width - (4.32 * 1.5 + 4.32)).abs() < 1e-9);
    }

    /// A string with any character the font does not have is not laid out at all; an empty string and spaces are laid out without strokes.
    /// フォントに無い字を1つでも含む文字列は配置しない。空の文字列と空白は線なしで配置する。
    #[test]
    fn missing_characters_and_empty_strings() {
        let f = font("kst32b").unwrap();
        assert!(f.layout("A😀", 10.0, Latin::Half, 1.0).is_none());
        let e = f.layout("", 10.0, Latin::Half, 1.0).unwrap();
        assert!(e.polylines.is_empty() && e.width == 0.0);
        let s = f.layout("  ", 10.0, Latin::Half, 1.0).unwrap();
        assert!(s.polylines.is_empty() && (s.width - 8.64).abs() < 1e-9);
    }

    /// The stroke is 0.07 of the font size thick but never thinner than 0.13 mm, and bold is half as thick again.
    /// 線の太さはフォントサイズの0.07倍で、0.13mmより細くならない。太字はその1.5倍。
    #[test]
    fn stroke_width_follows_the_size() {
        assert!((stroke_width(10.0, false) - 0.7).abs() < 1e-9);
        assert!((stroke_width(1.0, false) - 0.13).abs() < 1e-9);
        assert!((stroke_width(10.0, true) - 1.05).abs() < 1e-9);
    }

    /// The glyph data sent to the canvas holds every glyph keyed by its character, with the strokes flattened to x, y pairs.
    /// キャンバスへ送る字形データは、全ての字形を文字をキーに持ち、線は x, y の並びに平らにしてある。
    #[test]
    fn glyph_data_for_the_canvas() {
        let f = font("kst32b").unwrap();
        let d = f.data();
        assert_eq!((d.id.as_str(), d.baseline, d.cap_top, d.glyphs.len()), ("kst32b", f.baseline, f.cap_top, f.glyphs.len()));
        let a = &d.glyphs["A"];
        assert_eq!(a.strokes.len(), f.glyphs[&'A'].strokes.len());
        assert_eq!(a.strokes[0].len(), f.glyphs[&'A'].strokes[0].len() * 2);
    }
```

- [ ] **Step 2: Run** → compile errors.
- [ ] **Step 3: Implement** per the rules above.
- [ ] **Step 4: Run** → pass; whole workspace `cargo test --workspace` green.
- [ ] **Step 5: Commit** — `feat: ストロークフォントで文字列を配置し、キャンバスへ渡す字形データを作る` (regenerate spec, audit, stage only changed files).

---

### Task 3: Project setting, command, format 21

**Files:**
- Modify: `src-tauri/crates/madake-core/src/model.rs`, `command.rs`, `io.rs` (tests), `src/ipc.ts`, `src/stores/document.ts`, `src/stores/document.test.ts`

**Interfaces:**
- Consumes: `stroke_font::font`, `model::Latin`.
- Produces: `pub struct TextFont { pub id: String, #[serde(default, skip_serializing_if = "is_half")] pub latin: Latin }`; `Project.text_font: Option<TextFont>` (`#[serde(default, skip_serializing_if = "Option::is_none")]`); `Command::SetTextFont { #[serde(default)] text_font: Option<TextFont> }`; `PatchOp::TextFontChanged { text_font: Option<TextFont> }`; `FORMAT_VERSION = 21`. TypeScript: `export type Latin = "half" | "full"; export interface TextFont { id: string; latin?: Latin }`; `Project.text_font?: TextFont | null`; patch `{ op: "text_font_changed"; text_font: TextFont | null }`.

- [ ] **Step 1: Failing tests**

`command.rs` (next to the `SetFrameTemplate` tests; read them for the engine helpers):

```rust
    /// Selecting the bundled stroke font sets it on the project and reports it in the patch; undo restores the system font.
    /// 同梱のストロークフォントを選ぶとプロジェクトに設定され、パッチで知らされる。undoでシステムの書体へ戻る。
    #[test]
    fn set_text_font_and_undo() {
        let mut engine = Engine::new(Project::new("p"));
        let tf = TextFont { id: "kst32b".into(), latin: Latin::Full };
        let patch = engine.execute(Command::SetTextFont { text_font: Some(tf.clone()) }).unwrap();
        assert!(matches!(&patch.ops[0], PatchOp::TextFontChanged { text_font: Some(t) } if *t == tf));
        assert_eq!(engine.project().text_font.as_ref(), Some(&tf));
        engine.undo().unwrap();
        assert!(engine.project().text_font.is_none());
    }

    /// A font id that is not bundled is refused and nothing changes.
    /// 同梱していないフォントのidは拒否され、何も変わらない。
    #[test]
    fn set_text_font_refuses_an_unknown_id() {
        let mut engine = Engine::new(Project::new("p"));
        let bad = TextFont { id: "nope".into(), latin: Latin::Half };
        assert!(engine.execute(Command::SetTextFont { text_font: Some(bad) }).is_err());
        assert!(engine.project().text_font.is_none());
    }
```

(Use the real undo API of `Engine`; read an existing undo test first.)

`io.rs`: extend the version test to 21 (rename it to `a_version_20_project_migrates_to_21` only if its subject is "the previous version migrates"; keep the existing one that pins the current version) and add:

```rust
    /// A project with a stroke font round-trips through the file; a project without one writes no text_font key.
    /// ストロークフォントを持つプロジェクトはファイルを往復でき、持たないプロジェクトはtext_fontのキーを書かない。
    #[test]
    fn text_font_round_trips_and_is_omitted_when_unset() {
        let mut p = Project::new("p");
        assert!(!serde_json::to_string(&p).unwrap().contains("text_font"));
        p.text_font = Some(TextFont { id: "kst32b".into(), latin: Latin::Full });
        let back: Project = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
        assert_eq!(back.text_font, p.text_font);
    }
```

`src/stores/document.test.ts` (mirror the `frame_template_changed` test at line 168):

```ts
  // ja: text_font_changed のパッチで、プロジェクトの書体の設定が切り替わり、undoで元へ戻る
  it("text_font_changed patches update the mirror both ways", () => {
    // build the store as the frame_template_changed test does, then:
    // apply { op: "text_font_changed", text_font: { id: "kst32b", latin: "full" } } → project.text_font equals it
    // apply { op: "text_font_changed", text_font: null } → project.text_font is null
  });
```

(write the body by copying the setup of the neighbouring test; the two expectations are given in the comments.)

- [ ] **Step 2: Run** → fail.
- [ ] **Step 3: Implement** — model types; `FORMAT_VERSION = 21` with a doc line `- v21: プロジェクトの書体 (Project::text_font、同梱ストロークフォント)。形式20のビルドは版番号で断る`; command arm like `SetFrameTemplate` plus the id check (`crate::stroke_font::font(&tf.id).is_none()` → `CoreError::InvalidCommand(format!("知らない書体: {}", tf.id))`); TypeScript types; store `case "text_font_changed": if (this.project) this.project.text_font = op.text_font; break;`. Update tests that pin version 20 as the current version.
- [ ] **Step 4: Run** `cargo test --workspace`, `npx vitest run`, `npx vue-tsc --noEmit`, `python3 -m unittest discover -s freecad-addon/tests` → all green.
- [ ] **Step 5: Commit** — `feat: プロジェクトの書体を選ぶコマンドを足す (形式21)`.

---

### Task 4: SVG and PDF draw text with the stroke font

**Files:**
- Modify: `src-tauri/crates/madake-core/src/svg.rs`, `pdf.rs`, `report_sheet.rs`, `frame_template.rs`

**Interfaces:**
- Consumes: Tasks 2–3.
- Produces: `pub struct TextFontScope` (restores the previous active font on drop); `pub fn text_font_scope(tf: Option<&TextFont>) -> TextFontScope`; `pub(crate) fn text_advance(s: &str, size: f64, spacing: f64) -> f64`; `FrameTemplate.text_latin: Option<Latin>` and `Cell.spacing: Option<f64>` (both `skip_serializing_if = "Option::is_none"`; validation: `cells[i].spacing` finite and > 0).

Implementation outline:
- A `thread_local!` `Cell<Option<ActiveFont>>` with `ActiveFont { font: &'static StrokeFont, latin: Latin }` (Copy). `text_font_scope` sets it from `tf` (unknown id → `None`), the guard restores the previous value. A second guard `latin_scope(Option<Latin>)` overrides only `latin` while a font is active.
- A private `stroke_text_el(out, x, y, size, fill, anchor, bold, spacing, s) -> bool`: if a font is active and `layout` succeeds, write one `<path d="M … L …" fill="none" stroke="{fill}" stroke-width="{w}" stroke-linecap="round" stroke-linejoin="round"/>` translated by the anchor (`middle` −width/2, `end` −width); write nothing for a string without strokes; return `true`. Otherwise `false`.
- `text_family_el` and `text_weight_el` call `stroke_text_el(…, 1.0, …)` first and return when it wrote; their existing code stays byte for byte. `frame_cell` passes `cell.spacing.unwrap_or(1.0)` through a variant of the writer that takes spacing.
- `text_advance`: the stroke width when a font is active and the string lays out, else `tidy::text_width(s, size)`. `fitted_font`, the box of a boxed cell and the triangle/suffix fitting in `revision_table` use `text_advance` (with the cell's spacing where there is one).
- `frame_and_title_block` opens `latin_scope(t.text_latin)` at its start.
- Open `let _font = text_font_scope(project.text_font.as_ref());` at the start of `svg::project_sheet_to_svg`, `pdf::project_pdf_pages_with` and `report_sheet::report_bytes_with`.

- [ ] **Step 1: Failing tests** (in `svg.rs` tests unless noted)

```rust
    fn stroke_project() -> crate::model::Project {
        let mut p = crate::model::Project::new("stroke");
        p.sheets[0] = golden_sheet();
        p.text_font = Some(crate::model::TextFont { id: "kst32b".into(), latin: crate::model::Latin::Half });
        p
    }

    /// With a stroke font every text of the sheet, frame included, becomes a stroked path and no text element is left.
    /// ストロークフォントでは、図枠を含むシートの全ての文字が線のパスになり、text要素は残らない。
    #[test]
    fn a_stroke_font_turns_all_text_into_paths() {
        let p = stroke_project();
        let svg = project_sheet_to_svg(&p, p.sheets[0].id, &builtin_symbols()).unwrap();
        assert!(!svg.contains("<text"), "{svg}");
        assert!(svg.contains("stroke-linecap=\"round\" stroke-linejoin=\"round\"/>"));
    }

    /// A stroked text starts at its anchor, scaled so that capitals are 0.72 of the font size, in the text colour and with the stroke width of the size.
    /// 線にした文字はアンカーから始まり、大文字がフォントサイズの0.72倍になる縮尺で、文字の色と、サイズに応じた線幅で描かれる。
    #[test]
    fn a_stroked_text_sits_on_its_anchor() {
        let _f = text_font_scope(Some(&crate::model::TextFont { id: "kst32b".into(), latin: crate::model::Latin::Half }));
        let mut out = String::new();
        text_el(&mut out, 100.0, 50.0, 10.0, "#c00000", "end", "H");
        // "H" is 4.32 wide → starts at 95.68; the bottom of its stems is on the baseline y=50, the top at 42.8
        assert!(out.starts_with("<path d=\"M 95."), "{out}");
        assert!(out.contains(" 50 ") || out.contains(" 50\""), "{out}");
        assert!(out.contains(" 42.800"), "{out}");
        assert!(out.contains("stroke=\"#c00000\" stroke-width=\"0.700\""), "{out}");
    }

    /// Text with a character the font lacks keeps its text element, and the active font does not outlive its scope.
    /// フォントに無い字を含む文字列はtext要素のまま残り、有効な書体はその範囲の外へ持ち越されない。
    #[test]
    fn missing_glyphs_fall_back_and_the_scope_ends() {
        {
            let _f = text_font_scope(Some(&crate::model::TextFont { id: "kst32b".into(), latin: crate::model::Latin::Half }));
            let mut out = String::new();
            text_el(&mut out, 0.0, 0.0, 3.0, "#000", "start", "A😀");
            assert!(out.starts_with("<text"), "{out}");
        }
        let mut out = String::new();
        text_el(&mut out, 0.0, 0.0, 3.0, "#000", "start", "A");
        assert!(out.starts_with("<text"), "{out}");
    }

    /// Selecting a stroke font changes no pin, wire or netlist: the drawing's connectivity and its verification stay the same.
    /// ストロークフォントを選んでも、ピン・配線・ネットリストは変わらない。図面の接続と検証結果は同じままである。
    #[test]
    fn a_stroke_font_moves_no_geometry() {
        // build a project with blocks and wires (reuse the richest fixture in this file or in netlist tests),
        // export it with and without text_font; compare: crate::netlist::extract_netlist(...) equal,
        // crate::verify::verify(...) codes equal, and every <line>/<polyline> of wires identical in both SVGs.
    }
```

(The last test's body is to be written from the fixture you choose; its three comparisons are the requirement. `golden_sheet()` has no wires; pick a fixture that has blocks, wires and wire notes.)

Frame template tests in `svg.rs`:

```rust
    /// A frame template that asks for full-width Latin letters draws its cells with them, while the rest of the sheet keeps the project's setting.
    /// 英数字を全角にする図枠テンプレートは、セルを全角の字で描き、シートの他の文字はプロジェクトの設定のまま。
    #[test]
    fn a_frame_template_can_use_full_width_latin() {
        // with text_font Half: one cell "AB" size 10 at x=300 start → path spans 17.28 mm with Full, 8.64 with Half.
        // assert on the max x of the cell's path relative to its start.
    }

    /// Letter spacing on a cell widens the gaps between its letters, and fitting uses the stroke width with that spacing.
    /// セルの字間は文字の間を広げ、幅に収める処理はその字間を含めたストロークの幅を使う。
    #[test]
    fn cell_spacing_and_fitting_use_the_stroke_width() { /* spacing 1.5 on "AB" size 10 → width 10.8; fit to w=5.4 → size 5 */ }
```

`frame_template.rs`: add `("cells[0].spacing", …spacing = Some(0.0))` and a NaN case to `invalid_new_items_are_refused_by_name`; add `text_latin` and `spacing` to the round-trip test and to the "not written when unused" key list.

- [ ] **Step 2: Run** → fail.
- [ ] **Step 3: Implement** per the outline.
- [ ] **Step 4: Run** `cargo test --workspace` → green, golden tests included.
- [ ] **Step 5: Commit** — `feat: プロジェクトの書体がストロークフォントなら、SVG・PDFの文字を線で描く`.

---

### Task 5: Serve the glyphs to the frontend

**Files:**
- Modify: `src-tauri/crates/madake-mcp/src/link_api.rs`, `src-tauri/src/lib.rs`, `src/ipc.ts`
- Test: `src-tauri/crates/madake-mcp/tests/stroke_fonts_api.rs` (new; copy the server start-up of `frame_templates_api.rs`)

**Interfaces:**
- Produces: `GET /api/v1/stroke-fonts/{id}` → `StrokeFontData` JSON (404 for an unknown id); Tauri command `get_stroke_font(id: String) -> Result<StrokeFontData, String>`; TypeScript `StrokeFontData { id: string; baseline: number; cap_top: number; glyphs: Record<string, { wide: boolean; strokes: number[][] }> }` and `getStrokeFont(id: string): Promise<StrokeFontData>` in the interface and both implementations.

- [ ] **Step 1: Failing test**

```rust
/// The Link API serves the bundled stroke font's glyphs by id, and answers 404 for a font that is not bundled.
/// Link APIは同梱ストロークフォントの字形をidで返し、同梱していないフォントには404を返す。
#[tokio::test]
async fn stroke_fonts_are_served_by_id() {
    // start the API as frame_templates_api.rs does; GET /api/v1/stroke-fonts/kst32b → 200, JSON id "kst32b",
    // glyphs has "A" and "亜"; GET /api/v1/stroke-fonts/none → 404.
}
```

- [ ] **Step 2–4:** route + handler (`Json(font.data())`), Tauri command registered in the `invoke_handler` list next to `list_frame_templates`, `ipc.ts` both sides (`invoke<StrokeFontData>("get_stroke_font", { id })` and `http<StrokeFontData>(\`/stroke-fonts/${encodeURIComponent(id)}\`)`). Run the workspace tests, `npx vue-tsc --noEmit`.
- [ ] **Step 5: Commit** — `feat: ストロークフォントの字形をIPCとLink APIでフロントエンドへ渡す`.

---

### Task 6: Canvas draws the stroke font

**Files:**
- Create: `src/canvas/strokeFont.ts`, `src/canvas/strokeFont.test.ts`
- Modify: `src/canvas/textSize.ts`, `src/canvas/frameTemplate.ts`, `src/canvas/renderer.ts`, `src/components/CanvasView.vue`

**Interfaces:**
- Consumes: Task 5 data; Rust layout rules (Task 2).
- Produces (`strokeFont.ts`): `CAP_HEIGHT_RATIO`, `HALF_ADVANCE = 15`, `FULL_ADVANCE = 30`; `layoutStroke(font: StrokeFontData, text: string, size: number, latin: Latin, spacing = 1): { polylines: [number, number][][]; width: number } | null`; `strokeWidth(size: number, bold: boolean): number`; active state `setActiveStrokeFont(font: StrokeFontData | null, latin: Latin)`, `activeStrokeFont()`, `withLatin<T>(latin: Latin | null | undefined, fn: () => T): T`; `textAdvanceMm(text: string, size: number, spacing = 1): number` (stroke width when active, else `textWidthMm(text, size)`).

- [ ] **Step 1: Failing tests** (`strokeFont.test.ts`, with a tiny hand-made `StrokeFontData` — not the real font — so the numbers are easy):

```ts
// 小さな作り物のフォント: "H" は高さ0〜25の縦線2本と横線1本 (baseline 0, cap_top 25)、"亜" は全角の四角
const font: StrokeFontData = {
  id: "t", baseline: 0, cap_top: 25,
  glyphs: {
    H: { wide: false, strokes: [[2, 0, 2, 25], [12, 0, 12, 25], [2, 12, 12, 12]] },
    "亜": { wide: true, strokes: [[1, 1, 29, 1, 29, 29, 1, 29, 1, 1]] },
    "Ｈ": { wide: true, strokes: [[4, 0, 4, 25]] },
    " ": { wide: false, strokes: [] },
  },
};
```

Tests (each with a `// ja:` line): capitals are `0.72 × size` high and `H` advances `0.432 × size`; full-width advances twice; `Latin.full` maps `H` → `Ｈ` and falls back to the half-width glyph for a character without a full-width form; spacing widens gaps but not after the last letter; a missing character returns `null`; `textAdvanceMm` uses the stroke width only while a font is active and returns to `textWidthMm` after `setActiveStrokeFont(null, "half")`; `withLatin` restores the previous latin.

Parity: the frontend never parses the font file; it receives Rust's parsed glyphs (Task 5). The layout formulas are kept equal by the numeric tests above, which use the same numbers as the Rust tests of Task 2.

- [ ] **Step 2: Run** → fail.
- [ ] **Step 3: Implement**
  - `strokeFont.ts` per the interface (same rules as Rust Task 2).
  - `textSize.ts`: `setDrawingFont` remembers `px` and whether the prefix contains `bold` in its `WeakMap`; `drawingText(ctx, text, x, y, spacing = 1)` — when a stroke font is active and `layoutStroke(font, text, px, latin, spacing)` succeeds, draw the polylines with `ctx.strokeStyle = ctx.fillStyle`, `ctx.lineWidth = strokeWidth(px, bold)`, round caps/joins, offset by `ctx.textAlign` (`center` −w/2, `right`/`end` −w); otherwise `fillText` as today. Hidden (too small) text stays hidden.
  - `frameTemplate.ts`: `fittedFont`, the cell box and the revision table fitting use `textAdvanceMm` (with the cell's spacing); `frameLayout` and `revisionLayout` run inside `withLatin(t.text_latin, …)`; `CellLayout.spacing`.
  - `renderer.ts` `drawFrame`: wrap in `withLatin(frame.text_latin, …)`; pass `cell.spacing` to `drawingText`.
  - `CanvasView.vue`: watch `store.project?.text_font`; when set, `ipc.getStrokeFont(id)` once per id (cache), `setActiveStrokeFont(data, latin)`, `scheduleDraw()`; when unset, `setActiveStrokeFont(null, "half")`, `scheduleDraw()`.
- [ ] **Step 4: Run** `npx vitest run`, `npx vue-tsc --noEmit` → green.
- [ ] **Step 5: Commit** — `feat: キャンバスでもプロジェクトの書体のストロークフォントで文字を描く`.

---

### Task 7: Licence inventory

**Files:**
- Modify: `scripts/gen_licenses.py`, `docs/internal/third-party-licenses.md` (generated)

- [ ] **Step 1:** In `render()`, replace the "Fonts | none bundled" row with "Fonts | KST32B 3.0 bundled for stroke text (see Bundled fonts)" and add a section:

```
## Bundled fonts

| Font | Author | Version | Licence (quoted from its documentation) | Files |
|---|---|---|---|---|
| KST32B | Saka.N | 3.0 (2017-07-01) | Freeware: "free to copy, change …; no responsibility". 「使用,配付改変,再配付等は自由ですが,使用した結果のいかなる影響についても,責任及び賠償責任は,一切負いかねます」 | `src-tauri/resources/stroke_fonts/kst32b/` (`KST32B.TXT`, `KST32B.RDM`, `KST32B.MAN`, unmodified) |
```

The script asserts that the three files exist and fails otherwise.
- [ ] **Step 2:** `python3 scripts/gen_licenses.py` then `python3 scripts/gen_licenses.py --check` → exit 0.
- [ ] **Step 3: Commit** — `docs: 同梱フォントKST32Bをサードパーティのライセンス一覧へ載せる`.

---

### Task 8: Documents and the reproduction check

**Files:**
- Modify: `docs/internal/data-model.md` (format 21 row, `Project.text_font`, `text_latin`, `spacing`), `docs/internal/feature-inventory.md`, `docs/internal/specs/reproduction-gaps.md` + `.ja.md` (lettering row of the fifth pass)
- Modify (git-ignored): `docs/references/repro/a0/post.py` (`set_text_font`), `make_frame.py` (`text_latin: "full"`, `spacing` where the reference's lettering is wider), `NOTES.md`

- [ ] **Step 1:** Update the documents (internal docs keep their English + Japanese pairs).
- [ ] **Step 2:** Restart the development build on port 9320, set the font in `post.py` with `{"type": "set_text_font", "text_font": {"id": "kst32b", "latin": "half"}}`, add `"text_latin": "full"` to the template, run `./run.sh r1_frame`, view the title block crop of the diff. Record the coverage numbers and the remaining differences in `NOTES.md` only.
- [ ] **Step 3:** Full verification: `cargo test --workspace`, `npx vitest run`, `npx vue-tsc --noEmit`, `python3 -m unittest discover -s freecad-addon/tests`, `python3 scripts/gen_spec.py --check`, `python3 scripts/gen_licenses.py --check`, `python3 scripts/audit_confidential.py`.
- [ ] **Step 4: Commit** — `docs: ストロークフォントをデータ設計・機能一覧・ギャップ台帳へ反映`.
