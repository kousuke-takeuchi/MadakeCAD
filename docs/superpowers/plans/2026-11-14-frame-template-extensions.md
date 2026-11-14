# Frame Template Extensions (G24–G31, data and drawing) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a frame template state per-side margins, hidden zone labels, trim and centre marks, a configurable revision table, rule weights, rotated/boxed/fitted/multi-line cells, free title block fields, capital-height sizes and a logo image, drawn identically by SVG, PDF and the canvas.

**Architecture:** All new items are optional serde fields on the existing `FrameTemplate` types in `madake-core`, with helper methods (`margins`, `frame_rect`, `font_size`, `revision_columns`, `revision_row_h`) that return today's values when the items are absent. `svg.rs` keeps its current code path byte for byte when no new item is used and takes a new path otherwise. `src/canvas/frameTemplate.ts` mirrors the helpers and extends `FrameLayout`; `renderer.ts` draws the extended layout. `Revision` and `TitleBlock` gain fields and the project format goes to version 20.

**Tech Stack:** Rust (serde, schemars, svg2pdf 0.13, base64), TypeScript (Vue 3, Vitest), Canvas2D.

**Spec:** `docs/superpowers/specs/2026-11-14-frame-template-extensions-design.md`

## Global Constraints

- A template or project that uses none of the new items produces exactly the same SVG as before. The golden tests `the_jis_a3_template_reproduces_the_previous_frame_exactly` and `an_empty_sheet_reproduces_the_previous_frame_exactly` and their golden files are never edited.
- Every edit to a document goes through a `Command` in `madake-core`. No code writes to `Project`/`Sheet` directly.
- Every test has its specification sentences: Rust `///` two lines (English, then Japanese) before `#[test]`; Vitest `// ja: …` on the line before `it("English…")`.
- After adding or changing tests: `python3 scripts/gen_spec.py`, and commit the regenerated `docs/13-specification*.md` with the tests.
- Constants shared by both renderers: strokes thin 0.25 / medium 0.5 / heavy 1.0 mm; trim mark inset 4 mm, arm 15 mm, stroke 0.5; centre mark length min(margin, 10 mm), stroke 0.5; cap ratio 0.72; line pitch 1.25 × font size; box gap 0.8 mm; logo image limit 2 MB (2 097 152 bytes).
- Field names: revision fields `mark`, `count`, `description`, `rev_no`, `date`, `by`, `approved`; free field names 1–32 characters of `a-z`, `0-9`, `_`.
- No UI look changes in this plan (sub-project B). The frame editor only has to keep the new items.
- `python3 scripts/audit_confidential.py` prints `no confidential terms found` before every commit.
- Commit messages in Japanese, conventional prefix (`feat:`, `test:`, `docs:`), no `Co-Authored-By` or other AI attribution trailer.
- Rust is in `~/.cargo/bin` (`export PATH="$HOME/.cargo/bin:$PATH"`).

## Review Focus

1. A template with `frame_margins` whose title block origin is not bottom-right (top-left etc.): the title block must sit in that corner of the per-side frame, not of the uniform one.
2. A revision whose description is longer than its column, or has three lines in a 7 mm row: the text must be shrunk to stay inside the column and must not run into the next column.
3. `rows_fixed` smaller than the number of revisions: only the newest `rows_fixed` rows are shown and the table height does not grow.
4. A logo file that is missing, unreadable, larger than 2 MB or of an unknown type: the drawing still exports, with the empty rectangle, and the template is still listed.
5. A project saved by this build and opened by the frame editor's template save path: saving a template never writes `logo.data_url` or `builtin` into the file.

## File Structure

| File | Change |
|---|---|
| `src-tauri/crates/madake-core/src/frame_template.rs` | New types and fields, helpers, validation, logo loading, `{field.*}` bind |
| `src-tauri/crates/madake-core/src/model.rs` | `Revision.rev_no/approved/count`, `TitleBlock.fields`, `FORMAT_VERSION = 20` |
| `src-tauri/crates/madake-core/src/svg.rs` | Frame drawing: margins, zones, marks, weights, cells, revision table, logo |
| `src-tauri/crates/madake-core/src/dxf.rs` | Border uses `frame_rect` (other new items are not exported to DXF) |
| `src-tauri/crates/madake-core/src/pdf.rs` | Test only: embedded images reach the PDF |
| `src-tauri/crates/madake-core/Cargo.toml` | `base64` dependency |
| `src/ipc.ts` | `Revision` and `TitleBlock` types |
| `src/canvas/frameTemplate.ts` | Types, helpers, extended `FrameLayout` and `RevisionLayout` |
| `src/canvas/renderer.ts` | Draw the extended layout |
| `src/canvas/frameEditor.ts` | `frameChecks` accepts `columns`; operations keep unknown items |
| Tests | In-file `mod tests` (Rust); `src/canvas/frameTemplate.test.ts`, `src/canvas/frameEditor.test.ts` |
| `docs/internal/data-model.md`, `docs/internal/feature-inventory.md`, `docs/internal/specs/reproduction-gaps.md` (+ `.ja.md`), `docs/13-specification*.md` | Documentation |

Branch: `feature/frame-template-extensions`, created from `docs/a0-reference-reproduction`.

---

### Task 1: Prove that an embedded image reaches the PDF

**Files:**
- Modify: `src-tauri/crates/madake-core/src/pdf.rs` (tests only)

**Interfaces:**
- Consumes: `svgs_to_pdf(svgs: &[String]) -> Result<Vec<u8>, PdfError>`
- Produces: the decision "PNG, JPEG and SVG" or "PNG and JPEG only" for Task 7, written into the ledger

- [ ] **Step 1: Write the tests**

Add to `mod tests` in `pdf.rs`:

```rust
    /// A 1x1 PNG, base64-encoded (red pixel).
    const PNG_1X1: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==";

    fn svg_with_image(href: &str) -> String {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100mm\" height=\"50mm\" viewBox=\"0 0 100 50\">\
             <image x=\"10\" y=\"10\" width=\"40\" height=\"20\" preserveAspectRatio=\"xMidYMid meet\" href=\"{href}\"/></svg>"
        )
    }

    /// A PNG embedded in the sheet SVG as a data URI is written into the PDF as an image object.
    /// シートのSVGへdata URIで埋め込んだPNGは、PDFに画像オブジェクトとして書き出される。
    #[test]
    fn an_embedded_png_reaches_the_pdf() {
        let svg = svg_with_image(&format!("data:image/png;base64,{PNG_1X1}"));
        let pdf = svgs_to_pdf(&[svg]).expect("pdf");
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("/Subtype /Image"), "PDFに画像が無い");
    }

    /// An SVG image embedded as a data URI is converted too: the PDF is produced and is larger than the same page without the image.
    /// data URIで埋め込んだSVG画像も変換される。PDFができ、画像の無い同じページより大きい。
    #[test]
    fn an_embedded_svg_image_reaches_the_pdf() {
        use base64::Engine as _;
        let inner = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 10 10\"><rect width=\"10\" height=\"10\" fill=\"#c00\"/></svg>";
        let b64 = base64::engine::general_purpose::STANDARD.encode(inner);
        let with = svgs_to_pdf(&[svg_with_image(&format!("data:image/svg+xml;base64,{b64}"))]).expect("pdf");
        let without = svgs_to_pdf(&[svg_with_image("")]).expect("pdf");
        assert!(with.len() > without.len(), "{} <= {}", with.len(), without.len());
    }
```

- [ ] **Step 2: Add the dependency and run**

Find the `base64` version already locked: `grep -n -A1 '^name = "base64"' src-tauri/Cargo.lock`. Add the newest listed version to `src-tauri/crates/madake-core/Cargo.toml` under `[dependencies]` as `base64 = "<major.minor>"`.

Run: `cd src-tauri && cargo test -p madake-core --lib pdf::tests::an_embedded 2>&1 | tail -15`
Expected: both tests PASS (they test an existing capability; a pass is the finding). If `an_embedded_png_reaches_the_pdf` fails, check `svg2pdf`/`usvg` features in `Cargo.toml` and enable the raster image feature, then rerun. If only `an_embedded_svg_image_reaches_the_pdf` fails, delete that test, and record in the ledger: `Task 1: Ruling: logo images limited to PNG and JPEG`; Task 7 then refuses `.svg`, and the spec's G31 section (both languages) is updated in Task 10.

- [ ] **Step 3: Commit**

```bash
git checkout -b feature/frame-template-extensions
git add src-tauri/crates/madake-core/Cargo.toml src-tauri/Cargo.lock src-tauri/crates/madake-core/src/pdf.rs
git commit -m "test: SVGへ埋め込んだ画像がPDFへ出ることを確かめる"
```

---

### Task 2: Template data — new fields, helpers, validation

**Files:**
- Modify: `src-tauri/crates/madake-core/src/frame_template.rs`

**Interfaces:**
- Produces (Rust):
  - `pub struct FrameMargins { pub left: f64, pub right: f64, pub top: f64, pub bottom: f64 }`
  - `pub struct Marks { pub trim: bool, pub centre: bool }`
  - `pub enum LineWeight { Thin, Medium, Heavy }` with `pub fn stroke(&self) -> f64`
  - `pub enum TextSizeIs { Font, Cap }`, `pub enum MarkShape { None, Triangle }`
  - `pub enum RevisionField { Mark, Count, Description, RevNo, Date, By, Approved }`
  - `pub struct RevisionColumn { pub field: RevisionField, pub header: String, pub w: f64, pub align: Align }`
  - New fields: `FrameTemplate.frame_margins: Option<FrameMargins>`, `.marks: Marks`, `.text_size_is: TextSizeIs`; `Zones.show: bool`; `GridLine.weight: LineWeight`; `Cell.rotation: u16`, `.boxed: bool`, `.fit: bool`; `RevisionBlockDef.columns: Option<Vec<RevisionColumn>>`, `.row_h: Option<f64>`, `.rows_fixed: Option<usize>`, `.mark_shape: MarkShape`; `LogoFrame.data_url: Option<String>`
  - Methods on `FrameTemplate`: `margins() -> FrameMargins`, `frame_rect(pw, ph) -> (f64, f64, f64, f64)` (x0, y0, x1, y1), `font_size(size: f64) -> f64`, `revision_columns() -> Vec<RevisionColumn>`, `revision_row_h() -> f64`, `uses_extended_revision_table() -> bool`
  - Constants: `CAP_HEIGHT_RATIO: f64 = 0.72`, `LINE_PITCH: f64 = 1.25`, `LOGO_MAX_BYTES: u64 = 2_097_152`

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `frame_template.rs` (the module already has `sheet()`; `default_template().clone()` gives a valid template to modify):

```rust
    fn base() -> FrameTemplate {
        default_template().clone()
    }

    /// Without the new items a template reports today's values: one margin on all four sides, thin rules, font sizes as written, the four classic revision columns.
    /// 新しい項目の無いテンプレートは今までどおりの値を返す(4辺同じ余白、細い罫線、書いたとおりの文字サイズ、従来の改訂欄4列)。
    #[test]
    fn a_template_without_the_new_items_keeps_todays_values() {
        let t = base();
        let m = t.margins();
        assert_eq!((m.left, m.right, m.top, m.bottom), (10.0, 10.0, 10.0, 10.0));
        assert_eq!(t.frame_rect(420.0, 297.0), (10.0, 10.0, 410.0, 287.0));
        assert_eq!(t.font_size(3.0), 3.0);
        assert!(t.zones.show);
        assert!(!t.uses_extended_revision_table());
        let cols = t.revision_columns();
        assert_eq!(cols.len(), 4);
        assert_eq!(cols[0].field, RevisionField::Mark);
        assert_eq!(cols[1].field, RevisionField::Date);
        assert_eq!(cols[2].field, RevisionField::Description);
        assert_eq!(cols[3].field, RevisionField::By);
        assert_eq!(cols.iter().map(|c| c.w).sum::<f64>(), t.title_block.w);
        assert_eq!(t.revision_row_h(), t.title_block.row_h);
    }

    /// Margins given per side move the frame and the title block corner; the single margin is then ignored.
    /// 辺ごとの余白を指定すると外枠と表題欄の隅がそれに従い、1値の余白は無視される。
    #[test]
    fn margins_per_side_move_the_frame_and_the_title_block() {
        let mut t = base();
        t.frame_margins = Some(FrameMargins { left: 30.0, right: 15.0, top: 12.0, bottom: 10.0 });
        assert_eq!(t.frame_rect(1189.0, 841.0), (30.0, 12.0, 1174.0, 831.0));
        let (tx, ty) = t.title_block_origin(1189.0, 841.0);
        assert_eq!((tx, ty), (1174.0 - t.title_block.w, 831.0 - t.title_block.h));
        t.title_block.origin = Origin::TopLeft;
        assert_eq!(t.title_block_origin(1189.0, 841.0), (30.0, 12.0));
    }

    /// With capital-height sizes, a size of 3.6 mm is drawn with a 5 mm font.
    /// 文字サイズが大文字高さの指定なら、3.6mmは5mmのフォントで描く。
    #[test]
    fn capital_height_sizes_are_converted_to_font_sizes() {
        let mut t = base();
        t.text_size_is = TextSizeIs::Cap;
        assert!((t.font_size(3.6) - 5.0).abs() < 1e-9);
    }

    /// The new items are optional keys: a template file written before them still loads, and saving a template that does not use them writes none of the new keys.
    /// 新しい項目は省略できるキーである。追加前のテンプレートファイルはそのまま読め、使っていないテンプレートを保存しても新しいキーは書かれない。
    #[test]
    fn old_template_files_load_and_unused_items_are_not_written() {
        let t = parse(BUNDLED[0]).expect("jis-a3");
        let json = serde_json::to_string(&t).unwrap();
        for key in ["frame_margins", "marks", "text_size_is", "\"show\"", "weight\":\"thin", "rotation", "boxed", "\"fit\"", "columns", "rows_fixed", "mark_shape", "data_url"] {
            assert!(!json.contains(key), "{key} が書かれている: {json}");
        }
    }

    /// A template that uses every new item survives a write and a read unchanged.
    /// 新しい項目を全て使うテンプレートは、書いて読み直しても変わらない。
    #[test]
    fn a_template_with_every_new_item_round_trips() {
        let t = extended();
        let back: FrameTemplate = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
        assert_eq!(back, t);
        back.validate().expect("valid");
    }

    fn extended() -> FrameTemplate {
        let mut t = base();
        t.frame_margins = Some(FrameMargins { left: 30.0, right: 15.0, top: 12.0, bottom: 10.0 });
        t.zones.show = false;
        t.marks = Marks { trim: true, centre: true };
        t.text_size_is = TextSizeIs::Cap;
        t.title_block.grid_lines[0].weight = LineWeight::Medium;
        t.title_block.cells[0].rotation = 90;
        t.title_block.cells[0].boxed = true;
        t.title_block.cells[0].fit = true;
        let w = t.title_block.w;
        t.revision_block.columns = Some(vec![
            RevisionColumn { field: RevisionField::Mark, header: "SYMB".into(), w: 14.0, align: Align::Start },
            RevisionColumn { field: RevisionField::Description, header: "CHANGELOG".into(), w: w - 64.0, align: Align::Start },
            RevisionColumn { field: RevisionField::RevNo, header: "ECN NO".into(), w: 16.0, align: Align::Middle },
            RevisionColumn { field: RevisionField::Date, header: "DATE".into(), w: 12.0, align: Align::Middle },
            RevisionColumn { field: RevisionField::By, header: "BY".into(), w: 11.0, align: Align::Middle },
            RevisionColumn { field: RevisionField::Approved, header: "OKAY".into(), w: 11.0, align: Align::Middle },
        ]);
        t.revision_block.row_h = Some(7.0);
        t.revision_block.rows_fixed = Some(7);
        t.revision_block.mark_shape = MarkShape::Triangle;
        t
    }

    /// Broken values are refused with a message that names the item.
    /// 壊れた値は、項目名を示すメッセージで拒否される。
    #[test]
    fn invalid_new_items_are_refused_by_name() {
        let cases: Vec<(&str, Box<dyn Fn(&mut FrameTemplate)>)> = vec![
            ("frame_margins", Box::new(|t| t.frame_margins = Some(FrameMargins { left: -1.0, right: 0.0, top: 0.0, bottom: 0.0 }))),
            ("rotation", Box::new(|t| t.title_block.cells[0].rotation = 45)),
            ("revision_block.columns", Box::new(|t| t.revision_block.columns = Some(vec![]))),
            ("revision_block.columns", Box::new(|t| {
                let w = t.title_block.w;
                t.revision_block.columns = Some(vec![
                    RevisionColumn { field: RevisionField::Mark, header: "A".into(), w: w / 2.0, align: Align::Start },
                    RevisionColumn { field: RevisionField::Mark, header: "B".into(), w: w / 2.0, align: Align::Start },
                ])
            })),
            ("revision_block.columns", Box::new(|t| t.revision_block.columns = Some(vec![
                RevisionColumn { field: RevisionField::Mark, header: "A".into(), w: 1.0, align: Align::Start },
            ]))),
            ("revision_block.row_h", Box::new(|t| t.revision_block.row_h = Some(0.0))),
            ("revision_block.rows_fixed", Box::new(|t| t.revision_block.rows_fixed = Some(0))),
            ("logo.path", Box::new(|t| t.logo = Some(LogoFrame { x: 0.0, y: 0.0, w: 10.0, h: 5.0, path: Some("../x.png".into()), data_url: None }))),
            ("logo.path", Box::new(|t| t.logo = Some(LogoFrame { x: 0.0, y: 0.0, w: 10.0, h: 5.0, path: Some("/etc/x.png".into()), data_url: None }))),
        ];
        for (name, edit) in cases {
            let mut t = base();
            edit(&mut t);
            let err = t.validate().expect_err(name);
            assert!(err.contains(name), "{name}: {err}");
        }
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd src-tauri && cargo test -p madake-core --lib frame_template 2>&1 | tail -20`
Expected: compile errors (`FrameMargins`, `margins`, … not found).

- [ ] **Step 3: Implement**

In `frame_template.rs`:

```rust
/// 大文字の高さ ÷ フォントサイズ (`text_size_is: cap` の換算)。
pub const CAP_HEIGHT_RATIO: f64 = 0.72;
/// 複数行の文字の行送り ÷ フォントサイズ。
pub const LINE_PITCH: f64 = 1.25;
/// ロゴ画像の大きさの上限 (バイト)。
pub const LOGO_MAX_BYTES: u64 = 2_097_152;

fn yes() -> bool { true }
fn is_true(v: &bool) -> bool { *v }
fn is_false(v: &bool) -> bool { !*v }
fn is_zero(v: &u16) -> bool { *v == 0 }
fn is_default<T: Default + PartialEq>(v: &T) -> bool { *v == T::default() }

/// 辺ごとの図枠の余白 (mm)。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FrameMargins { pub left: f64, pub right: f64, pub top: f64, pub bottom: f64 }

/// 図枠の外に描くマーク。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Marks {
    /// 用紙の四隅の裁断マーク (L字)。
    #[serde(default)]
    pub trim: bool,
    /// 外枠の各辺の中央の中心マーク。
    #[serde(default)]
    pub centre: bool,
}

/// 罫線の太さ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LineWeight { #[default] Thin, Medium, Heavy }

impl LineWeight {
    /// 線幅 (mm)。
    pub fn stroke(&self) -> f64 {
        match self { LineWeight::Thin => 0.25, LineWeight::Medium => 0.5, LineWeight::Heavy => 1.0 }
    }
}

/// `Cell.size`の意味。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextSizeIs { #[default] Font, Cap }

/// 改訂記号を囲む形。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarkShape { #[default] None, Triangle }

/// 改訂欄の列が表示する項目。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RevisionField { Mark, Count, Description, RevNo, Date, By, Approved }

impl RevisionField {
    /// この列に出す値。
    pub fn value<'a>(&self, rev: &'a crate::model::Revision) -> &'a str {
        match self {
            RevisionField::Mark => &rev.mark,
            RevisionField::Count => &rev.count,
            RevisionField::Description => &rev.description,
            RevisionField::RevNo => &rev.rev_no,
            RevisionField::Date => &rev.date,
            RevisionField::By => &rev.by,
            RevisionField::Approved => &rev.approved,
        }
    }
}

/// 改訂欄の1列。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RevisionColumn {
    pub field: RevisionField,
    /// 見出し行に印字する文字。
    pub header: String,
    /// 列幅 (mm)。
    pub w: f64,
    #[serde(default)]
    pub align: Align,
}
```

Add the fields (each with the serde attributes shown) to the existing structs:

```rust
// FrameTemplate, after `frame_margin`:
    /// 辺ごとの余白。あれば`frame_margin`の代わりに使う。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_margins: Option<FrameMargins>,
// FrameTemplate, after `revision_block`:
    #[serde(default, skip_serializing_if = "is_default")]
    pub marks: Marks,
    #[serde(default, skip_serializing_if = "is_default")]
    pub text_size_is: TextSizeIs,
// Zones:
    /// ゾーン目盛を描くか (既定: 描く)。
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub show: bool,
// GridLine:
    #[serde(default, skip_serializing_if = "is_default")]
    pub weight: LineWeight,
// Cell:
    /// 回転 (0/90/180/270、画面上で時計回り)。
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rotation: u16,
    /// 文字を枠で囲む。
    #[serde(default, skip_serializing_if = "is_false")]
    pub boxed: bool,
    /// `w`に収まるよう文字を縮める。
    #[serde(default, skip_serializing_if = "is_false")]
    pub fit: bool,
// RevisionBlockDef:
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<RevisionColumn>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_h: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows_fixed: Option<usize>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub mark_shape: MarkShape,
// LogoFrame:
    /// 読み込んだ画像 (data URI)。テンプレートのファイルからは読まず、書きもしない。
    #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub data_url: Option<String>,
```

Methods (replace the body of `title_block_origin` so that it uses `frame_rect`):

```rust
    /// 4辺の余白。`frame_margins`が無ければ`frame_margin`を4辺に使う。
    pub fn margins(&self) -> FrameMargins {
        self.frame_margins.unwrap_or(FrameMargins {
            left: self.frame_margin, right: self.frame_margin,
            top: self.frame_margin, bottom: self.frame_margin,
        })
    }

    /// 外枠の矩形 (x0, y0, x1, y1) 用紙mm。
    pub fn frame_rect(&self, pw: f64, ph: f64) -> (f64, f64, f64, f64) {
        let m = self.margins();
        (m.left, m.top, pw - m.right, ph - m.bottom)
    }

    pub fn title_block_origin(&self, pw: f64, ph: f64) -> (f64, f64) {
        let (x0, y0, x1, y1) = self.frame_rect(pw, ph);
        let tb = &self.title_block;
        match tb.origin {
            Origin::BottomRight => (x1 - tb.w, y1 - tb.h),
            Origin::BottomLeft => (x0, y1 - tb.h),
            Origin::TopRight => (x1 - tb.w, y0),
            Origin::TopLeft => (x0, y0),
        }
    }

    /// 描くフォントサイズ。`text_size_is: cap`なら大文字高さから換算する。
    pub fn font_size(&self, size: f64) -> f64 {
        match self.text_size_is {
            TextSizeIs::Font => size,
            TextSizeIs::Cap => size / CAP_HEIGHT_RATIO,
        }
    }

    /// 改訂欄の列。`columns`が無ければ従来の4列 (記号/日付/内容/承認)。
    pub fn revision_columns(&self) -> Vec<RevisionColumn> {
        if let Some(cols) = &self.revision_block.columns {
            return cols.clone();
        }
        let fields = [RevisionField::Mark, RevisionField::Date, RevisionField::Description, RevisionField::By];
        let headers = ["記号", "日付", "内容", "承認"];
        self.revision_block.col_w.iter().zip(fields).zip(headers)
            .map(|((w, field), header)| RevisionColumn { field, header: header.into(), w: *w, align: Align::Start })
            .collect()
    }

    /// 改訂欄の行高。
    pub fn revision_row_h(&self) -> f64 {
        self.revision_block.row_h.unwrap_or(self.title_block.row_h)
    }

    /// 改訂欄が新しい項目を使っているか (使っていなければ従来の描画を1バイトも変えない)。
    pub fn uses_extended_revision_table(&self) -> bool {
        let rb = &self.revision_block;
        rb.columns.is_some() || rb.row_h.is_some() || rb.rows_fixed.is_some()
            || rb.mark_shape != MarkShape::None || self.text_size_is != TextSizeIs::Font
    }
```

`validate()` additions (keep the existing checks; the `col_w` checks stay as they are, because `col_w` is still required):

```rust
        if let Some(m) = &self.frame_margins {
            if [m.left, m.right, m.top, m.bottom].iter().any(|v| *v < 0.0) {
                return Err(format!("frame_margins が負: {m:?}"));
            }
        }
        for (i, cell) in tb.cells.iter().enumerate() {
            if ![0, 90, 180, 270].contains(&cell.rotation) {
                return Err(format!("cells[{i}]: rotation は 0/90/180/270: {}", cell.rotation));
            }
        }
        if let Some(cols) = &rb.columns {
            if cols.is_empty() || cols.len() > 8 {
                return Err(format!("revision_block.columns は1〜8列: {}列", cols.len()));
            }
            for (i, c) in cols.iter().enumerate() {
                if cols[..i].iter().any(|p| p.field == c.field) {
                    return Err(format!("revision_block.columns: {:?} が2回ある", c.field));
                }
                if c.w <= 0.0 {
                    return Err(format!("revision_block.columns[{i}].w が不正: {}", c.w));
                }
            }
            let sum: f64 = cols.iter().map(|c| c.w).sum();
            if (sum - tb.w).abs() > 0.01 {
                return Err(format!("revision_block.columns の幅の合計 {sum} が表題欄の幅 {} と違う", tb.w));
            }
        }
        if let Some(h) = rb.row_h {
            if h <= 0.0 { return Err(format!("revision_block.row_h が不正: {h}")); }
        }
        if rb.rows_fixed == Some(0) {
            return Err("revision_block.rows_fixed は1以上".into());
        }
        if let Some(path) = self.logo.as_ref().and_then(|l| l.path.as_deref()) {
            let p = std::path::Path::new(path);
            if p.is_absolute() || p.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                return Err(format!("logo.path はテンプレートのフォルダの中だけ: {path}"));
            }
        }
```

In `save_to`, after `obj.remove("builtin")`, also drop the runtime image:

```rust
        if let Some(logo) = obj.get_mut("logo").and_then(|l| l.as_object_mut()) {
            logo.remove("data_url");
        }
```

Then fix every struct literal that no longer compiles (`GridLine { .. }`, `Cell { .. }`, `LogoFrame { .. }`, `Zones { .. }`, `RevisionBlockDef { .. }`, `FrameTemplate { .. }` in `madake-core`, `madake-mcp`, `src-tauri/src`): add the new fields with their defaults (`weight: LineWeight::Thin`, `rotation: 0, boxed: false, fit: false`, `data_url: None`, `show: true`, `columns: None, row_h: None, rows_fixed: None, mark_shape: MarkShape::None`, `frame_margins: None, marks: Marks::default(), text_size_is: TextSizeIs::Font`). Find them with `cargo build --workspace --tests 2>&1 | grep -E "^error|-->" | head -60`.

- [ ] **Step 4: Run to verify they pass**

Run: `cd src-tauri && cargo test -p madake-core --lib frame_template 2>&1 | tail -6` → all pass.
Run: `cd src-tauri && cargo test --workspace 2>&1 | grep -E "^test result|FAILED|failed" | head -20` → every `test result: ok`.

- [ ] **Step 5: Commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/audit_confidential.py
git add -A src-tauri docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat: 図枠テンプレートに辺別余白・マーク・罫線の太さ・回転文字・改訂欄の列などの項目を足す"
```

---

### Task 3: Project model — revision fields, free title block fields, format 20

**Files:**
- Modify: `src-tauri/crates/madake-core/src/model.rs`, `src-tauri/crates/madake-core/src/frame_template.rs` (`bind_value`), `src-tauri/crates/madake-core/src/io.rs` (tests), `src/ipc.ts`, `src/canvas/frameTemplate.ts` (`bindValue`), `src/canvas/frameTemplate.test.ts`

**Interfaces:**
- Produces: `Revision { mark, date, description, by, rev_no, approved, count }` (Rust, all `String`; TS: the three new ones optional); `TitleBlock.fields: BTreeMap<String, String>` (TS: `fields?: Record<string, string>`); bind `{field.<name>}`; `FORMAT_VERSION = 20`; `pub fn valid_field_name(name: &str) -> bool` in `model.rs`

- [ ] **Step 1: Write the failing tests**

`frame_template.rs` tests:

```rust
    /// A free title block field is printed where the template binds `{field.name}`; a name that is not set prints nothing.
    /// 表題欄の自由な欄は、テンプレートが`{field.名前}`でバインドした場所に印字される。設定の無い名前は何も出ない。
    #[test]
    fn free_fields_resolve_through_field_binds() {
        let mut s = sheet();
        s.title_block.fields.insert("title_en".into(), "MAIN CIRCUIT".into());
        let ctx = BindContext::for_sheet(&s);
        assert_eq!(resolve_binds("{field.title_en}/{field.missing}", &ctx), "MAIN CIRCUIT/");
    }
```

`model.rs` tests (next to the existing format tests):

```rust
    /// Field names are short lower-case identifiers; anything else is refused.
    /// 欄の名前は短い小文字の識別子で、それ以外は拒否される。
    #[test]
    fn free_field_names_are_lower_case_identifiers() {
        for ok in ["mass", "title_en", "a1"] { assert!(valid_field_name(ok), "{ok}"); }
        for bad in ["", "Title", "a-b", "名前", &"x".repeat(33)] { assert!(!valid_field_name(bad), "{bad}"); }
    }

    /// A revision row written before the new fields loads with them empty, and empty ones are not written back.
    /// 新しい項目より前に書かれた改訂行は、その項目が空で読み込まれ、空のまま書き戻されない。
    #[test]
    fn old_revision_rows_load_with_empty_new_fields() {
        let r: Revision = serde_json::from_str(r#"{"mark":"A","date":"d","description":"x","by":"k"}"#).unwrap();
        assert_eq!((r.rev_no.as_str(), r.approved.as_str(), r.count.as_str()), ("", "", ""));
        let json = serde_json::to_string(&r).unwrap();
        assert!(!json.contains("rev_no") && !json.contains("approved") && !json.contains("count"), "{json}");
    }
```

`io.rs` tests:

```rust
    /// A version 19 project file opens in this build and becomes version 20, with no revision or title block data lost.
    /// 形式19のプロジェクトファイルはこのビルドで開け、形式20になる。改訂や表題欄のデータは失われない。
    #[test]
    fn a_version_19_project_migrates_to_20() {
        let mut project = Project::new("old");
        project.format_version = 19;
        project.sheets[0].revisions.push(Revision { mark: "A".into(), ..Default::default() });
        let mut loaded: Project = serde_json::from_str(&serde_json::to_string(&project).unwrap()).unwrap();
        migrate(&mut loaded);
        assert_eq!(loaded.format_version, 20);
        assert_eq!(loaded.sheets[0].revisions[0].mark, "A");
    }
```

`src/canvas/frameTemplate.test.ts`:

```ts
  // ja: 表題欄の自由な欄は {field.名前} で解決され、無い名前は空文字になる
  it("resolves a free title block field through {field.name}, and a missing name to empty text", () => {
    const s = sheet([], { title_block: { fields: { title_en: "MAIN CIRCUIT" } } });
    const ctx = bindContextForSheet(s);
    expect(resolveBinds("{field.title_en}/{field.missing}", ctx)).toBe("MAIN CIRCUIT/");
  });
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd src-tauri && cargo test -p madake-core --lib 2>&1 | grep -E "^error" | head -5` → compile errors (`fields`, `rev_no`, `valid_field_name`).
Run: `npx vitest run src/canvas/frameTemplate.test.ts 2>&1 | tail -8` → the new test fails.

- [ ] **Step 3: Implement**

`model.rs`:

```rust
// Revision: after `by`
    /// 改訂番号 (設計変更通知の番号など)。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub rev_no: String,
    /// 承認者。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub approved: String,
    /// 記号の横に出す個数 (例 "x7")。保存した文字をそのまま印字する。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub count: String,

// TitleBlock: after `rev`
    /// 自由な欄 (名前 → 値)。図枠テンプレートは`{field.名前}`でバインドする。
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub fields: std::collections::BTreeMap<String, String>,

/// 表題欄の自由な欄の名前として使えるか (a-z・0-9・_ の1〜32文字)。
pub fn valid_field_name(name: &str) -> bool {
    (1..=32).contains(&name.len())
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}
```

Set `pub const FORMAT_VERSION: u32 = 20;` and add to its doc comment: `/// - v20: 改訂行の改訂番号・承認・個数 ([`Revision::rev_no`] ほか) と、表題欄の自由な欄 ([`TitleBlock::fields`])。形式19のビルドは版番号で断る。新しい項目を使わないファイルは版番号以外これまでと同じ`.

In `command.rs`, in the `SetTitleBlock` arm before the title block is stored, refuse bad names:

```rust
                if let Some(bad) = title_block.fields.keys().find(|k| !crate::model::valid_field_name(k)) {
                    return Err(CommandError::Invalid(format!("表題欄の欄の名前「{bad}」は a-z・0-9・_ の1〜32文字で書いてください")));
                }
```

(Use the error constructor the neighbouring arms use: read the `SetTitleBlock` arm at `command.rs:669` and the error enum, and match them.) Add a test beside the existing `SetTitleBlock` test:

```rust
    /// Setting a title block with a free field whose name has an upper-case letter is refused and nothing changes.
    /// 名前に大文字を含む自由な欄を持つ表題欄の設定は拒否され、何も変わらない。
    #[test]
    fn set_title_block_refuses_a_bad_field_name() {
        let mut engine = Engine::new(Project::new("p"));
        let sheet_id = engine.project().sheets[0].id;
        let mut tb = TitleBlock::default();
        tb.fields.insert("Title".into(), "x".into());
        assert!(engine.execute(Command::SetTitleBlock { sheet_id, title_block: tb }).is_err());
        assert!(engine.project().sheets[0].title_block.fields.is_empty());
    }
```

`frame_template.rs` `bind_value`: before the final `_ => return None`, add

```rust
        _ if name.starts_with("field.") => tb.fields.get(&name["field.".len()..]).cloned().unwrap_or_default(),
```

`src/ipc.ts`: `Revision` gains `rev_no?: string; approved?: string; count?: string;` and `TitleBlock` gains `fields?: Record<string, string>;`.

`src/canvas/frameTemplate.ts` `bindValue`: before the function's final `return undefined`, add

```ts
  if (name.startsWith("field.")) return ctx.titleBlock?.fields?.[name.slice("field.".length)] ?? "";
```

(read `bindValue` at line 274 first and use the property name its `BindContext` uses for the title block).

Fix `Revision { .. }` and `TitleBlock { .. }` literals that no longer compile by adding `..Default::default()`; update any test that asserts `format_version == 19` to 20 (`grep -rn "format_version.*19\|19.*format_version" src-tauri src freecad-addon | grep -v target`).

- [ ] **Step 4: Run to verify they pass**

Run: `cd src-tauri && cargo test --workspace 2>&1 | grep -E "^test result|FAILED|failed" | head -20` → all ok.
Run: `npx vitest run 2>&1 | tail -6 && npx vue-tsc --noEmit` → all pass, no type errors.
Run: `python3 -m unittest discover -s freecad-addon/tests 2>&1 | tail -3` → OK.

- [ ] **Step 5: Commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/audit_confidential.py
git add -A src-tauri src docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat: 改訂行に改訂番号・承認・個数、表題欄に自由な欄を足す (形式20)"
```

---

### Task 4: SVG — margins, zone labels, marks, rule weights

**Files:**
- Modify: `src-tauri/crates/madake-core/src/svg.rs` (`frame_and_title_block`), `src-tauri/crates/madake-core/src/dxf.rs` (frame border)

**Interfaces:**
- Consumes: `FrameTemplate::frame_rect`, `margins`, `Zones.show`, `Marks`, `GridLine.weight`, `LineWeight::stroke`
- Produces: constants in `svg.rs`: `pub(crate) const TRIM_INSET: f64 = 4.0; pub(crate) const TRIM_ARM: f64 = 15.0; pub(crate) const MARK_STROKE: f64 = 0.5; pub(crate) const CENTRE_MARK_MAX: f64 = 10.0;`

- [ ] **Step 1: Write the failing tests**

In `svg.rs` `mod tests` (uses the existing `golden_sheet()`, `builtin_symbols()`, `sheet_to_svg_with_frame`):

```rust
    fn a0_sheet() -> Sheet {
        let mut s = golden_sheet();
        s.size = PaperSize::A0;
        s.revisions.clear();
        s
    }

    fn frame_with(edit: impl Fn(&mut FrameTemplate)) -> FrameTemplate {
        let mut t = crate::frame_template::default_template().clone();
        edit(&mut t);
        t
    }

    /// With margins per side the border is drawn from (left, top) to (paper − right, paper − bottom).
    /// 辺ごとの余白があるとき、外枠は(左, 上)から(用紙−右, 用紙−下)まで描かれる。
    #[test]
    fn margins_per_side_set_the_border_rectangle() {
        let t = frame_with(|t| t.frame_margins = Some(crate::frame_template::FrameMargins { left: 30.0, right: 15.0, top: 12.0, bottom: 10.0 }));
        let svg = sheet_to_svg_with_frame(&a0_sheet(), &builtin_symbols(), &t, &[]);
        assert!(svg.contains("<rect x=\"30\" y=\"12\" width=\"1144\" height=\"819\" fill=\"none\""), "{svg}");
    }

    /// With zone labels hidden, the frame has no zone letters or digits and no zone ticks.
    /// ゾーン目盛を隠すと、図枠にゾーンの英字・数字も目盛線も出ない。
    #[test]
    fn hidden_zones_draw_no_labels_and_no_ticks() {
        let shown = sheet_to_svg_with_frame(&a0_sheet(), &builtin_symbols(), &frame_with(|_| {}), &[]);
        assert!(shown.contains(">A</text>") && shown.contains("stroke-width=\"0.25\""));
        let t = frame_with(|t| { t.zones.show = false; t.title_block.grid_lines.clear(); });
        let svg = frame_only(&sheet_to_svg_with_frame(&a0_sheet(), &builtin_symbols(), &t, &[]));
        assert!(!svg.contains(">A</text>") && !svg.contains(">1</text>"), "{svg}");
        assert!(!svg.contains("stroke-width=\"0.25\""), "{svg}");
    }

    /// Trim marks are an L at each paper corner, 4 mm in from the edge with 15 mm arms: eight lines.
    /// 裁断マークは用紙の四隅のL字で、端から4mm内側、腕15mm。線は8本。
    #[test]
    fn trim_marks_are_an_l_at_each_paper_corner() {
        let t = frame_with(|t| t.marks.trim = true);
        let svg = sheet_to_svg_with_frame(&a0_sheet(), &builtin_symbols(), &t, &[]);
        assert!(svg.contains("<line x1=\"4\" y1=\"4\" x2=\"19\" y2=\"4\" stroke=\"#000\" stroke-width=\"0.5\"/>"), "{svg}");
        assert!(svg.contains("<line x1=\"4\" y1=\"4\" x2=\"4\" y2=\"19\" stroke=\"#000\" stroke-width=\"0.5\"/>"), "{svg}");
        assert!(svg.contains("<line x1=\"1185\" y1=\"837\" x2=\"1170\" y2=\"837\" stroke=\"#000\" stroke-width=\"0.5\"/>"), "{svg}");
        let off = sheet_to_svg_with_frame(&a0_sheet(), &builtin_symbols(), &frame_with(|_| {}), &[]);
        assert_eq!(svg.matches("<line").count() - off.matches("<line").count(), 8);
    }

    /// Centre marks run outwards from the middle of each border side, as long as that side's margin but at most 10 mm.
    /// 中心マークは外枠の各辺の中央から外向きに伸び、長さはその辺の余白(最大10mm)。
    #[test]
    fn centre_marks_run_outwards_from_the_middle_of_each_side() {
        let t = frame_with(|t| {
            t.frame_margins = Some(crate::frame_template::FrameMargins { left: 30.0, right: 15.0, top: 12.0, bottom: 6.0 });
            t.marks.centre = true;
        });
        let svg = sheet_to_svg_with_frame(&a0_sheet(), &builtin_symbols(), &t, &[]);
        // 上辺: x = (30 + 1174) / 2 = 602、y 12 → 2 (余白12だが最大10)
        assert!(svg.contains("<line x1=\"602\" y1=\"12\" x2=\"602\" y2=\"2\" stroke=\"#000\" stroke-width=\"0.5\"/>"), "{svg}");
        // 下辺: 余白6なので 835 → 841
        assert!(svg.contains("<line x1=\"602\" y1=\"835\" x2=\"602\" y2=\"841\" stroke=\"#000\" stroke-width=\"0.5\"/>"), "{svg}");
        // 左辺: y = (12 + 835) / 2 = 423.5、x 30 → 20
        assert!(svg.contains("<line x1=\"30\" y1=\"423.500\" x2=\"20\" y2=\"423.500\" stroke=\"#000\" stroke-width=\"0.5\"/>"), "{svg}");
    }

    /// A rule marked medium is drawn 0.5 mm wide and one marked heavy 1 mm wide; unmarked rules stay 0.25 mm.
    /// mediumの罫線は0.5mm、heavyは1mmで描かれる。指定の無い罫線は0.25mmのまま。
    #[test]
    fn rule_weights_set_the_stroke_width() {
        let t = frame_with(|t| {
            t.title_block.grid_lines[0].weight = crate::frame_template::LineWeight::Medium;
            t.title_block.grid_lines[1].weight = crate::frame_template::LineWeight::Heavy;
        });
        let svg = sheet_to_svg_with_frame(&golden_sheet(), &builtin_symbols(), &t, &[]);
        // jis-a3 の罫線0,1 は y=8, y=16 の横線。表題欄の左上は (290, 255)
        assert!(svg.contains("<line x1=\"290\" y1=\"263\" x2=\"410\" y2=\"263\" stroke=\"#000\" stroke-width=\"0.5\"/>"), "{svg}");
        assert!(svg.contains("<line x1=\"290\" y1=\"271\" x2=\"410\" y2=\"271\" stroke=\"#000\" stroke-width=\"1\"/>"), "{svg}");
        assert!(svg.contains("<line x1=\"290\" y1=\"279\" x2=\"410\" y2=\"279\" stroke=\"#000\" stroke-width=\"0.25\"/>"), "{svg}");
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd src-tauri && cargo test -p madake-core --lib svg::tests 2>&1 | grep -E "test result|FAILED|panicked" | head` → the five new tests fail, the golden tests pass.

- [ ] **Step 3: Implement**

In `frame_and_title_block`, replace the margin lines and wrap the zone block:

```rust
    let (pw, ph) = sheet.paper_mm();
    let (x0, y0, x1, y1) = t.frame_rect(pw, ph);
    rect_el(out, x0, y0, x1 - x0, y1 - y0, "none", "#000", FRAME_STROKE);
    if t.zones.show {
        // …既存のゾーン目盛のコードをそのまま (変数 x0,y0,x1,y1 は同じ名前)
    }
    frame_marks(out, t, pw, ph);
```

and draw each rule with its weight:

```rust
    for g in &def.grid_lines {
        line_el(out, tx + g.x1, ty + g.y1, tx + g.x2, ty + g.y2, g.weight.stroke(), "#000");
    }
```

(`LineWeight::Thin.stroke()` is 0.25 = `RULE_STROKE`, so the default output is unchanged.) New function and constants:

```rust
/// 裁断マーク: 用紙の端からの距離と腕の長さ (mm)。
pub(crate) const TRIM_INSET: f64 = 4.0;
pub(crate) const TRIM_ARM: f64 = 15.0;
/// 裁断マーク・中心マークの線幅 (mm)。
pub(crate) const MARK_STROKE: f64 = 0.5;
/// 中心マークの長さの上限 (mm)。
pub(crate) const CENTRE_MARK_MAX: f64 = 10.0;

/// 裁断マーク (用紙四隅のL字) と中心マーク (外枠の各辺の中央から外向き)。
fn frame_marks(out: &mut String, t: &FrameTemplate, pw: f64, ph: f64) {
    if t.marks.trim {
        for (cx, cy, sx, sy) in [
            (TRIM_INSET, TRIM_INSET, 1.0, 1.0),
            (pw - TRIM_INSET, TRIM_INSET, -1.0, 1.0),
            (TRIM_INSET, ph - TRIM_INSET, 1.0, -1.0),
            (pw - TRIM_INSET, ph - TRIM_INSET, -1.0, -1.0),
        ] {
            line_el(out, cx, cy, cx + sx * TRIM_ARM, cy, MARK_STROKE, "#000");
            line_el(out, cx, cy, cx, cy + sy * TRIM_ARM, MARK_STROKE, "#000");
        }
    }
    if t.marks.centre {
        let (x0, y0, x1, y1) = t.frame_rect(pw, ph);
        let m = t.margins();
        let (mx, my) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        line_el(out, mx, y0, mx, y0 - m.top.min(CENTRE_MARK_MAX), MARK_STROKE, "#000");
        line_el(out, mx, y1, mx, y1 + m.bottom.min(CENTRE_MARK_MAX), MARK_STROKE, "#000");
        line_el(out, x0, my, x0 - m.left.min(CENTRE_MARK_MAX), my, MARK_STROKE, "#000");
        line_el(out, x1, my, x1 + m.right.min(CENTRE_MARK_MAX), my, MARK_STROKE, "#000");
    }
}
```

`dxf.rs`: at the frame export near line 971, read how the border rectangle is computed; replace its use of `frame.frame_margin` for the border and title block corner with `frame.frame_rect(pw, ph)` / `frame.title_block_origin(pw, ph)`. Add a comment: `// 図枠の新しい項目のうちDXFへ出すのは辺別の余白だけ (マーク・罫線の太さ・回転文字などは出さない)`. Add a test beside the existing DXF frame test (find it with `grep -n "frame" src-tauri/crates/madake-core/src/dxf.rs | grep "fn "`):

```rust
    /// The DXF frame border follows margins given per side.
    /// DXFの図枠の外枠は、辺ごとの余白に従う。
    #[test]
    fn the_dxf_border_follows_margins_per_side() {
        let mut t = crate::frame_template::default_template().clone();
        t.frame_margins = Some(crate::frame_template::FrameMargins { left: 30.0, right: 15.0, top: 12.0, bottom: 10.0 });
        assert_eq!(t.frame_rect(1189.0, 841.0), (30.0, 12.0, 1174.0, 831.0));
    }
```

(If the DXF module has a helper that returns the frame's line list for a template, assert on that list instead: the border's first corner is `(30, 12)` in sheet coordinates after the module's Y flip.)

- [ ] **Step 4: Run to verify they pass**

Run: `cd src-tauri && cargo test -p madake-core --lib 2>&1 | grep -E "^test result|FAILED"` → ok, including both golden tests.

- [ ] **Step 5: Commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/audit_confidential.py
git add -A src-tauri docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat: 図枠の辺別余白・ゾーン目盛の非表示・裁断マーク・中心マーク・罫線の太さをSVGへ描く"
```

---

### Task 5: SVG — cells: capital height, fit, multi-line, rotation, box

**Files:**
- Modify: `src-tauri/crates/madake-core/src/svg.rs`

**Interfaces:**
- Consumes: `FrameTemplate::font_size`, `Cell.rotation/boxed/fit/w`, `tidy::text_width`, `LINE_PITCH`
- Produces: `pub(crate) fn fitted_font(lines: &[&str], font: f64, max_w: Option<f64>) -> f64`; `pub(crate) const BOX_GAP: f64 = 0.8;`; `fn frame_cell(out, t, cell, value, x, y)`

- [ ] **Step 1: Write the failing tests**

```rust
    fn one_cell(edit: impl Fn(&mut crate::frame_template::Cell)) -> FrameTemplate {
        frame_with(|t| {
            let mut c = t.title_block.cells[0].clone();
            c.text = Some("ABCDEFGHIJ".into());
            c.bind = None;
            c.x = 10.0; c.y = 20.0; c.size = 3.0; c.w = Some(9.0);
            edit(&mut c);
            t.title_block.cells = vec![c];
            t.title_block.grid_lines.clear();
        })
    }

    fn cell_svg(t: &FrameTemplate) -> String {
        frame_only(&sheet_to_svg_with_frame(&golden_sheet(), &builtin_symbols(), t, &[]))
    }

    /// A plain cell is written exactly as before: one text element at the cell position with the cell size.
    /// ふつうのセルはこれまでと全く同じに書かれる(セルの位置に、セルのサイズで、text要素が1つ)。
    #[test]
    fn a_plain_cell_is_written_as_before() {
        let svg = cell_svg(&one_cell(|_| {}));
        assert!(svg.contains("<text x=\"300\" y=\"275\" font-size=\"3\" fill=\"#000\" text-anchor=\"start\" font-family=\"sans-serif\">ABCDEFGHIJ</text>"), "{svg}");
    }

    /// With capital-height sizes the font is the size divided by 0.72.
    /// 大文字高さの指定では、フォントはサイズを0.72で割った値になる。
    #[test]
    fn a_capital_height_cell_uses_a_larger_font() {
        let mut t = one_cell(|c| c.size = 3.6);
        t.text_size_is = crate::frame_template::TextSizeIs::Cap;
        assert!(cell_svg(&t).contains("font-size=\"5\""));
    }

    /// A fitted cell shrinks its font so that the estimated text width equals the cell width; a text that already fits keeps its size.
    /// fitのセルは、見積もった文字幅がセル幅になるまでフォントを縮める。もともと収まる文字はそのまま。
    #[test]
    fn a_fitted_cell_shrinks_long_text_only() {
        // 10文字 × 0.6 × 3mm = 18mm → 幅9mmへ収めると 1.5mm
        assert!(cell_svg(&one_cell(|c| c.fit = true)).contains("font-size=\"1.500\""));
        assert!(cell_svg(&one_cell(|c| { c.fit = true; c.w = Some(40.0); })).contains("font-size=\"3\""));
        assert!(cell_svg(&one_cell(|c| { c.fit = true; c.w = None; })).contains("font-size=\"3\""));
    }

    /// A value with a line break is drawn as two text elements, the second 1.25 font sizes below the first.
    /// 改行を含む値はtext要素2つで描かれ、2行目は1行目からフォントサイズの1.25倍下になる。
    #[test]
    fn a_two_line_cell_is_drawn_as_two_lines() {
        let svg = cell_svg(&one_cell(|c| c.text = Some("AB\nCD".into())));
        assert!(svg.contains("<text x=\"300\" y=\"275\" font-size=\"3\""), "{svg}");
        assert!(svg.contains("<text x=\"300\" y=\"278.750\" font-size=\"3\""), "{svg}");
    }

    /// A cell rotated by 90 is turned clockwise about its anchor point.
    /// 90回転のセルは、アンカー点を中心に時計回りに回る。
    #[test]
    fn a_rotated_cell_turns_about_its_anchor() {
        let svg = cell_svg(&one_cell(|c| c.rotation = 90));
        assert!(svg.contains("<g transform=\"rotate(90 300 275)\">"), "{svg}");
        assert!(svg.contains("<text x=\"300\" y=\"275\" font-size=\"3\""), "{svg}");
    }

    /// A boxed cell gets a thin rectangle 0.8 mm outside its estimated text box.
    /// boxedのセルは、見積もった文字の箱の0.8mm外側に細い矩形が付く。
    #[test]
    fn a_boxed_cell_gets_a_rectangle_around_the_text() {
        let svg = cell_svg(&one_cell(|c| c.boxed = true));
        // 幅18、ベースライン275、フォント3 → 箱は x 300..318 / y 272..275.75(下へ0.25×フォント)
        assert!(svg.contains("<rect x=\"299.200\" y=\"271.200\" width=\"19.600\" height=\"5.350\" fill=\"none\" stroke=\"#000\" stroke-width=\"0.25\"/>"), "{svg}");
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd src-tauri && cargo test -p madake-core --lib svg::tests 2>&1 | grep -E "test result|FAILED|panicked" | head`
Expected: `a_plain_cell_is_written_as_before` passes; the other six fail.

- [ ] **Step 3: Implement**

```rust
/// 枠付きセル (`boxed`) の、文字の箱から枠までの距離 (mm)。
pub(crate) const BOX_GAP: f64 = 0.8;
/// ベースラインより下に出る分 (ディセンダ) ÷ フォントサイズ。枠の下端の見積もりに使う。
const DESCENT_RATIO: f64 = 0.25;

/// `max_w`に収まるフォントサイズ。収まっていれば`font`のまま (大きくはしない)。
pub(crate) fn fitted_font(lines: &[&str], font: f64, max_w: Option<f64>) -> f64 {
    let Some(max_w) = max_w.filter(|w| *w > 0.0) else { return font };
    let widest = lines.iter().map(|l| crate::tidy::text_width(l, font)).fold(0.0, f64::max);
    if widest <= max_w { font } else { font * max_w / widest }
}

/// 表題欄のセル1つ。新しい項目を使わないセルは従来の1要素をそのまま出す。
fn frame_cell(out: &mut String, t: &FrameTemplate, cell: &crate::frame_template::Cell, value: &str, x: f64, y: f64) {
    let lines: Vec<&str> = value.split('\n').collect();
    let base = t.font_size(cell.size);
    let font = if cell.fit { fitted_font(&lines, base, cell.w) } else { base };
    let plain = cell.rotation == 0 && !cell.boxed && lines.len() == 1;
    if plain {
        text_weight_el(out, x, y, font, "#000", cell.align.anchor(), cell.weight, value);
        return;
    }
    if cell.rotation != 0 {
        let _ = write!(out, "<g transform=\"rotate({} {} {})\">\n", cell.rotation, n(x), n(y));
    }
    let pitch = font * crate::frame_template::LINE_PITCH;
    for (i, line) in lines.iter().enumerate() {
        text_weight_el(out, x, y + pitch * i as f64, font, "#000", cell.align.anchor(), cell.weight, line);
    }
    if cell.boxed {
        let w = lines.iter().map(|l| crate::tidy::text_width(l, font)).fold(0.0, f64::max);
        let left = match cell.align {
            crate::frame_template::Align::Start => x,
            crate::frame_template::Align::Middle => x - w / 2.0,
            crate::frame_template::Align::End => x - w,
        };
        let top = y - font;
        let bottom = y + pitch * (lines.len() as f64 - 1.0) + font * DESCENT_RATIO;
        rect_el(out, left - BOX_GAP, top - BOX_GAP, w + 2.0 * BOX_GAP, bottom - top + 2.0 * BOX_GAP, "none", "#000", RULE_STROKE);
    }
    if cell.rotation != 0 {
        out.push_str("</g>\n");
    }
}
```

In `frame_and_title_block`, replace the `text_weight_el(...)` call inside the cell loop with `frame_cell(out, t, cell, &value, tx + cell.x, ty + cell.y);`.

Check `frame_only` (svg.rs:1678): it filters lines of the SVG; make sure `<g transform` and `</g>` lines of the frame are kept by it (read the function; if it drops unknown lines, the rotation test asserts on the full SVG instead of `cell_svg`).

- [ ] **Step 4: Run to verify they pass**

Run: `cd src-tauri && cargo test -p madake-core --lib 2>&1 | grep -E "^test result|FAILED"` → ok.

- [ ] **Step 5: Commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/audit_confidential.py
git add -A src-tauri docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat: 表題欄のセルで大文字高さ・幅への収め・複数行・回転・枠をSVGへ描く"
```

---

### Task 6: SVG — configurable revision table

**Files:**
- Modify: `src-tauri/crates/madake-core/src/svg.rs`

**Interfaces:**
- Consumes: `FrameTemplate::revision_columns`, `revision_row_h`, `uses_extended_revision_table`, `RevisionField::value`, `MarkShape`, `fitted_font`
- Produces: `fn revision_table(out, sheet, t, tx, ty)`; constants `pub(crate) const MARK_TRIANGLE_H: f64 = 0.8;` (triangle height ÷ row height)

Layout rules (used by the tests and by the canvas twin in Task 8):

- `n_rows = rows_fixed.unwrap_or(visible.len())`, where `visible` = the newest `rows_fixed.unwrap_or(rows_max)` revisions, oldest first. `n_rows == 0` → nothing drawn.
- Table: `x = tx`, `w = title_block.w`, `h = (n_rows + 1) * row_h`, `top = ty − h`. Outer rectangle `FRAME_STROKE`; a `RULE_STROKE` line above every row except the top one; a `RULE_STROKE` vertical line at the left of every column except the first, from `top` to `ty`.
- Header row is the bottom row (`ty − row_h … ty`); revision `i` (0 = oldest shown) is the row `ty − (i + 2) * row_h`.
- Text x by `align`: start `left + CELL_PAD`, middle `left + w/2`, end `left + w − CELL_PAD`.
- Fonts: header `t.font_size(REV_HEADER_FONT)`, body `t.font_size(REV_FONT)`, each fitted to `w − 2 * CELL_PAD` (widest line).
- Baseline of line `k` of `m` lines in a row with top `r`: `r + row_h/2 + font/2 − (m − 1) * pitch/2 + k * pitch`, `pitch = font * LINE_PITCH`. If `m * pitch > row_h`, the font is first reduced by the factor `row_h / (m * pitch)`.
- `mark_shape: triangle`, in the `mark` column, for a non-empty mark: triangle height `th = MARK_TRIANGLE_H * row_h`, half base `hb = th / 3f64.sqrt()`, centre x `cx = left + CELL_PAD + hb`, base y `by = r + row_h/2 + th/2`, apex `(cx, by − th)`. Polygon `apex, (cx + hb, by), (cx − hb, by)` with `RULE_STROKE`. The mark text is drawn middle-anchored at `cx`, baseline `by − th * 0.12`, font `min(body font, th * 0.55)`.

- [ ] **Step 1: Write the failing tests**

```rust
    fn six_columns() -> FrameTemplate {
        use crate::frame_template::{Align, RevisionColumn, RevisionField as F};
        frame_with(|t| {
            // jis-a3: 表題欄の幅120、左上 (290, 255)
            t.revision_block.columns = Some(vec![
                RevisionColumn { field: F::Mark, header: "SYMB".into(), w: 14.0, align: Align::Start },
                RevisionColumn { field: F::Description, header: "CHANGELOG".into(), w: 56.0, align: Align::Start },
                RevisionColumn { field: F::RevNo, header: "ECN NO".into(), w: 16.0, align: Align::Middle },
                RevisionColumn { field: F::Date, header: "DATE".into(), w: 12.0, align: Align::Middle },
                RevisionColumn { field: F::By, header: "BY".into(), w: 11.0, align: Align::Middle },
                RevisionColumn { field: F::Approved, header: "OKAY".into(), w: 11.0, align: Align::End },
            ]);
            t.revision_block.row_h = Some(7.0);
        })
    }

    fn rev6() -> Revision {
        Revision { mark: "1".into(), date: "24/01".into(), description: "変更".into(), by: "KT".into(),
                   rev_no: "E-204".into(), approved: "MU".into(), count: "x3".into() }
    }

    /// A revision table with its own columns prints the template's header words and each revision field in its column, with the row height the table states.
    /// 独自の列を持つ改訂欄は、テンプレートの見出し語と、各列に対応する改訂の項目を、改訂欄が指定する行高で印字する。
    #[test]
    fn a_configured_revision_table_prints_its_own_columns() {
        let mut sheet = golden_sheet();
        sheet.revisions = vec![rev6()];
        let svg = sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &six_columns(), &[]);
        // 1行 + 見出し = 高さ14、上端 255 - 14 = 241
        assert!(svg.contains("<rect x=\"290\" y=\"241\" width=\"120\" height=\"14\" fill=\"#fff\""), "{svg}");
        for word in ["SYMB", "CHANGELOG", "ECN NO", "DATE", "BY", "OKAY", "E-204", "24/01", "KT", "MU", "変更"] {
            assert!(svg.contains(&format!(">{word}</text>")), "{word}: {svg}");
        }
        assert!(!svg.contains(">記号</text>"), "{svg}");
        // ECN NO 列 (左端 290+14+56=360、幅16) は中央揃え → x=368
        assert!(svg.contains("<text x=\"368\" y=\"245.750\" font-size=\"2.500\" fill=\"#000\" text-anchor=\"middle\" font-family=\"sans-serif\">E-204</text>"), "{svg}");
        // OKAY 列は右揃え → x = 410 - 2
        assert!(svg.contains("text-anchor=\"end\" font-family=\"sans-serif\">MU</text>"), "{svg}");
    }

    /// With a fixed number of rows the table is ruled at full height even without any revision, and shows only the newest rows when there are more.
    /// 行数を固定すると、改訂が無くても全行の罫線を描き、改訂のほうが多いときは新しい行だけを表示する。
    #[test]
    fn fixed_rows_are_ruled_even_when_empty_and_show_the_newest() {
        let mut t = six_columns();
        t.revision_block.rows_fixed = Some(3);
        let mut sheet = golden_sheet();
        sheet.revisions.clear();
        let empty = sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &t, &[]);
        // 3行 + 見出し = 28、上端 227
        assert!(empty.contains("<rect x=\"290\" y=\"227\" width=\"120\" height=\"28\" fill=\"#fff\""), "{empty}");
        assert!(empty.contains(">SYMB</text>"));
        sheet.revisions = (1..=5).map(|i| Revision { mark: i.to_string(), ..rev6() }).collect();
        let full = sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &t, &[]);
        assert!(full.contains("<rect x=\"290\" y=\"227\" width=\"120\" height=\"28\" fill=\"#fff\""), "{full}");
        assert!(full.contains(">5</text>") && full.contains(">3</text>") && !full.contains(">2</text>"), "{full}");
    }

    /// A description longer than its column is shrunk to the column, and a two-line description is drawn as two lines inside the row.
    /// 列より長い内容は列幅まで縮められ、2行の内容は行の中に2行で描かれる。
    #[test]
    fn long_and_two_line_descriptions_stay_inside_the_cell() {
        let mut sheet = golden_sheet();
        sheet.revisions = vec![Revision { description: "あ".repeat(40), ..rev6() }];
        let svg = sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &six_columns(), &[]);
        // 40全角 × 2.5 = 100mm → 列の内側 56 - 4 = 52mm へ: 2.5 × 52 / 100 = 1.3
        assert!(svg.contains("font-size=\"1.300\""), "{svg}");
        sheet.revisions = vec![Revision { description: "一行目\n二行目".into(), ..rev6() }];
        let svg = sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &six_columns(), &[]);
        assert!(svg.contains(">一行目</text>") && svg.contains(">二行目</text>"), "{svg}");
    }

    /// With the triangle mark shape, each revision mark is drawn centred inside an upright triangle.
    /// 記号の形が三角のとき、改訂記号は上向きの三角の中に中央揃えで描かれる。
    #[test]
    fn a_triangle_is_drawn_around_each_revision_mark() {
        let mut t = six_columns();
        t.revision_block.mark_shape = crate::frame_template::MarkShape::Triangle;
        let mut sheet = golden_sheet();
        sheet.revisions = vec![rev6()];
        let svg = sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &t, &[]);
        assert_eq!(svg.matches("<polygon").count() - sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &six_columns(), &[]).matches("<polygon").count(), 1);
        // th = 5.6、hb = 3.233、cx = 290 + 2 + 3.233 = 295.233
        assert!(svg.contains("text-anchor=\"middle\" font-family=\"sans-serif\">1</text>"), "{svg}");
        assert!(svg.contains("<polygon points=\"295.233,"), "{svg}");
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd src-tauri && cargo test -p madake-core --lib svg::tests 2>&1 | grep -E "test result|FAILED|panicked" | head` → the four new tests fail.

- [ ] **Step 3: Implement**

At the top of `revision_block`, branch:

```rust
    if t.uses_extended_revision_table() {
        return revision_table(out, sheet, t, tx, ty);
    }
```

New code:

```rust
/// 改訂記号を囲む三角の高さ ÷ 行高。
pub(crate) const MARK_TRIANGLE_H: f64 = 0.8;

/// 改訂欄 (列・見出し・行高・固定行数・記号の形をテンプレートが決める版)。
fn revision_table(out: &mut String, sheet: &Sheet, t: &FrameTemplate, tx: f64, ty: f64) {
    use crate::frame_template::{Align, MarkShape, RevisionField, LINE_PITCH};
    let rb = &t.revision_block;
    let rows = visible_revisions_max(&sheet.revisions, rb.rows_fixed.unwrap_or(rb.rows_max));
    let n_rows = rb.rows_fixed.unwrap_or(rows.len());
    if n_rows == 0 {
        return;
    }
    let cols = t.revision_columns();
    let row_h = t.revision_row_h();
    let width = t.title_block.w;
    let h = (n_rows as f64 + 1.0) * row_h;
    let top = ty - h;
    rect_el(out, tx, top, width, h, "#fff", "#000", FRAME_STROKE);
    for r in 1..=n_rows {
        let ry = top + row_h * r as f64;
        line_el(out, tx, ry, tx + width, ry, RULE_STROKE, "#000");
    }
    let mut lefts = Vec::with_capacity(cols.len());
    let mut x = tx;
    for c in &cols {
        lefts.push(x);
        x += c.w;
    }
    for left in lefts.iter().skip(1) {
        line_el(out, *left, top, *left, ty, RULE_STROKE, "#000");
    }
    let anchor_x = |left: f64, w: f64, align: Align| match align {
        Align::Start => left + CELL_PAD,
        Align::Middle => left + w / 2.0,
        Align::End => left + w - CELL_PAD,
    };
    // 1セル分の文字 (収めてから、行の中央へ縦に並べる)
    let mut cell_text = |out: &mut String, row_top: f64, left: f64, c: &crate::frame_template::RevisionColumn, base: f64, value: &str| {
        let lines: Vec<&str> = value.split('\n').collect();
        let mut font = fitted_font(&lines, base, Some(c.w - 2.0 * CELL_PAD));
        let m = lines.len() as f64;
        if m * font * LINE_PITCH > row_h {
            font *= row_h / (m * font * LINE_PITCH);
        }
        let pitch = font * LINE_PITCH;
        let first = row_top + row_h / 2.0 + font / 2.0 - (m - 1.0) * pitch / 2.0;
        for (k, line) in lines.iter().enumerate() {
            text_el(out, anchor_x(left, c.w, c.align), first + pitch * k as f64, font, "#000", c.align.anchor(), line);
        }
    };
    let head_top = ty - row_h;
    for (c, left) in cols.iter().zip(&lefts) {
        if !c.header.is_empty() {
            cell_text(out, head_top, *left, c, t.font_size(REV_HEADER_FONT), &c.header);
        }
    }
    let body = t.font_size(REV_FONT);
    for (i, rev) in rows.iter().enumerate() {
        let row_top = ty - (i as f64 + 2.0) * row_h;
        for (c, left) in cols.iter().zip(&lefts) {
            let value = c.field.value(rev);
            if value.is_empty() {
                continue;
            }
            if c.field == RevisionField::Mark && rb.mark_shape == MarkShape::Triangle {
                let th = MARK_TRIANGLE_H * row_h;
                let hb = th / 3f64.sqrt();
                let cx = left + CELL_PAD + hb;
                let by = row_top + row_h / 2.0 + th / 2.0;
                let _ = write!(
                    out,
                    "<polygon points=\"{},{} {},{} {},{}\" fill=\"none\" stroke=\"#000\" stroke-width=\"{}\"/>\n",
                    n(cx), n(by - th), n(cx + hb), n(by), n(cx - hb), n(by), n(RULE_STROKE)
                );
                text_el(out, cx, by - th * 0.12, body.min(th * 0.55), "#000", "middle", value);
                continue;
            }
            cell_text(out, row_top, *left, c, body, value);
        }
    }
}
```

(If the borrow checker rejects the `cell_text` closure capturing `out` mutably while it is also passed in, make `cell_text` a nested `fn` that takes `row_h` and the anchor computation as parameters.)

- [ ] **Step 4: Run to verify they pass**

Run: `cd src-tauri && cargo test -p madake-core --lib 2>&1 | grep -E "^test result|FAILED"` → ok, golden tests included.

- [ ] **Step 5: Commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/audit_confidential.py
git add -A src-tauri docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat: 改訂欄の列・見出し・行高・固定行数・三角の記号をテンプレートで決められるようにする"
```

---

### Task 7: Logo image — load, SVG, PDF, template list

**Files:**
- Modify: `src-tauri/crates/madake-core/src/frame_template.rs` (`read_dir_frames`, new `load_logo`), `src-tauri/crates/madake-core/src/svg.rs` (logo drawing)
- Test: in-file tests; `src-tauri/crates/madake-mcp/tests/frame_templates_api.rs`

**Interfaces:**
- Consumes: `LogoFrame.data_url`, `LOGO_MAX_BYTES`, the Task 1 ruling on SVG images
- Produces: `pub fn load_logo(template: &mut FrameTemplate, dir: &Path)`; `pub fn logo_mime(path: &str) -> Option<&'static str>`

- [ ] **Step 1: Write the failing tests**

`frame_template.rs`:

```rust
    const PNG_1X1: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
        0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0xDA, 0x63, 0xFC, 0xCF, 0xC0, 0xF0,
        0x1F, 0x00, 0x05, 0x05, 0x02, 0x00, 0x5F, 0xC8, 0xF1, 0xD2, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
        0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    fn user_dir_with_logo(file: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("madake-logo-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(file), bytes).unwrap();
        let mut t = base();
        t.id = "with-logo".into();
        t.logo = Some(LogoFrame { x: 300.0, y: 260.0, w: 40.0, h: 10.0, path: Some(file.into()), data_url: None });
        let mut v = serde_json::to_value(&t).unwrap();
        v.as_object_mut().unwrap().remove("builtin");
        std::fs::write(dir.join("with-logo.json"), serde_json::to_string(&v).unwrap()).unwrap();
        dir
    }

    /// A template whose logo names a PNG next to it is listed with the image loaded as a data URI.
    /// ロゴが隣のPNGを指すテンプレートは、画像がdata URIとして読み込まれた状態で一覧に出る。
    #[test]
    fn a_logo_png_next_to_the_template_is_loaded() {
        let dir = user_dir_with_logo("logo.png", PNG_1X1);
        let list = list_from(None, Some(&dir));
        let t = list.templates.iter().find(|t| t.id == "with-logo").expect("listed");
        let url = t.logo.as_ref().unwrap().data_url.as_deref().expect("loaded");
        assert!(url.starts_with("data:image/png;base64,iVBOR"), "{url}");
    }

    /// A logo that is missing, too large or of an unknown type leaves the template listed without an image.
    /// ロゴが無い・大きすぎる・知らない種類のときも、テンプレートは画像無しで一覧に出る。
    #[test]
    fn an_unusable_logo_leaves_the_template_listed_without_an_image() {
        for (file, bytes) in [("logo.bmp", PNG_1X1.to_vec()), ("logo.png", vec![0u8; (LOGO_MAX_BYTES + 1) as usize])] {
            let dir = user_dir_with_logo(file, &bytes);
            let list = list_from(None, Some(&dir));
            let t = list.templates.iter().find(|t| t.id == "with-logo").expect("listed");
            assert!(t.logo.as_ref().unwrap().data_url.is_none(), "{file}");
        }
        let dir = user_dir_with_logo("logo.png", PNG_1X1);
        std::fs::remove_file(dir.join("logo.png")).unwrap();
        let list = list_from(None, Some(&dir));
        assert!(list.templates.iter().find(|t| t.id == "with-logo").unwrap().logo.as_ref().unwrap().data_url.is_none());
    }

    /// Saving a template never writes the loaded image or the built-in flag into the file.
    /// テンプレートの保存では、読み込んだ画像も同梱フラグもファイルへ書かない。
    #[test]
    fn saving_does_not_write_the_loaded_image() {
        let dir = user_dir_with_logo("logo.png", PNG_1X1);
        let t = list_from(None, Some(&dir)).templates.into_iter().find(|t| t.id == "with-logo").unwrap();
        let (_, path) = save_to(&dir, &t, &[]).expect("saved");
        let text = std::fs::read_to_string(path).unwrap();
        assert!(!text.contains("data_url") && !text.contains("builtin"), "{text}");
        assert!(text.contains("\"path\": \"logo.png\""), "{text}");
    }
```

`svg.rs`:

```rust
    /// A logo with a loaded image is drawn as that image inside the logo box, without the placeholder rectangle.
    /// 画像を読み込めたロゴは、ロゴ枠の中にその画像として描かれ、置き場所を示す矩形は描かれない。
    #[test]
    fn a_loaded_logo_is_drawn_as_an_image() {
        let t = frame_with(|t| t.logo = Some(LogoFrame { x: 15.0, y: 15.0, w: 40.0, h: 20.0, path: Some("l.png".into()), data_url: Some("data:image/png;base64,AAAA".into()) }));
        let svg = sheet_to_svg_with_frame(&golden_sheet(), &builtin_symbols(), &t, &[]);
        assert!(svg.contains("<image x=\"15\" y=\"15\" width=\"40\" height=\"20\" preserveAspectRatio=\"xMidYMid meet\" href=\"data:image/png;base64,AAAA\"/>"), "{svg}");
        assert!(!svg.contains("<rect x=\"15\" y=\"15\" width=\"40\" height=\"20\""), "{svg}");
    }
```

`madake-mcp/tests/frame_templates_api.rs`: read the file's first existing test for how it starts the API with a user frame directory, then add a test that puts `with-logo.json` and `logo.png` in that directory and asserts that `GET /api/v1/frame-templates` returns `logo.data_url` starting with `data:image/png;base64,` for it, with the two specification lines:

```rust
/// The frame template list sent to the canvas carries the logo image of a user template as a data URI.
/// キャンバスへ送る図枠テンプレートの一覧は、ユーザーの図枠のロゴ画像をdata URIで運ぶ。
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd src-tauri && cargo test -p madake-core --lib 2>&1 | grep -E "test result|FAILED|panicked" | head` → new tests fail.

- [ ] **Step 3: Implement**

`frame_template.rs`:

```rust
/// ロゴ画像のMIME型 (拡張子で決める)。知らない拡張子はNone。
pub fn logo_mime(path: &str) -> Option<&'static str> {
    let ext = std::path::Path::new(path).extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

/// ロゴ画像を読んで`logo.data_url`へ入れる。読めない・大きすぎる・知らない種類なら何もしない
/// (図面は枠だけで出る)。
pub fn load_logo(template: &mut FrameTemplate, dir: &Path) {
    use base64::Engine as _;
    let Some(logo) = template.logo.as_mut() else { return };
    logo.data_url = None;
    let Some(path) = logo.path.as_deref() else { return };
    let Some(mime) = logo_mime(path) else { return };
    let file = dir.join(path);
    match std::fs::metadata(&file) {
        Ok(meta) if meta.len() <= LOGO_MAX_BYTES => {}
        _ => return,
    }
    if let Ok(bytes) = std::fs::read(&file) {
        let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
        logo.data_url = Some(format!("data:{mime};base64,{b64}"));
    }
}
```

(If Task 1 ruled "PNG and JPEG only", drop the `"svg"` arm.) In `read_dir_frames`, after `template.builtin = builtin;` add `load_logo(&mut template, dir);`.

`svg.rs`, replace the logo block:

```rust
    if let Some(logo) = &t.logo {
        match &logo.data_url {
            Some(url) => {
                let _ = write!(
                    out,
                    "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"xMidYMid meet\" href=\"{}\"/>\n",
                    n(logo.x), n(logo.y), n(logo.w), n(logo.h), xml_escape(url)
                );
            }
            None => rect_el(out, logo.x, logo.y, logo.w, logo.h, "none", "#000", RULE_STROKE),
        }
    }
```

- [ ] **Step 4: Run to verify they pass**

Run: `cd src-tauri && cargo test --workspace 2>&1 | grep -E "^test result|FAILED|failed" | head -20` → all ok.

- [ ] **Step 5: Commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/audit_confidential.py
git add -A src-tauri docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat: 図枠のロゴ画像を読み込み、SVG・PDF・テンプレート一覧へ出す"
```

---

### Task 8: Canvas twin — types, helpers, layout

**Files:**
- Modify: `src/canvas/frameTemplate.ts`
- Test: `src/canvas/frameTemplate.test.ts`

**Interfaces:**
- Consumes: `textWidthMm(text, height)` from `src/canvas/textWidth.ts`
- Produces (TypeScript):
  - Types: `FrameMargins`, `LineWeight = "thin" | "medium" | "heavy"`, `RevisionField`, `RevisionColumn { field; header; w; align? }`; optional fields on `FrameTemplate` (`frame_margins?`, `marks?: { trim?: boolean; centre?: boolean }`, `text_size_is?: "font" | "cap"`), `zones.show?`, `FrameGridLine.weight?`, `FrameCell.rotation?/boxed?/fit?`, `revision_block.columns?/row_h?/rows_fixed?/mark_shape?`, `logo.data_url?`
  - Constants: `CAP_HEIGHT_RATIO = 0.72`, `LINE_PITCH = 1.25`, `LINE_STROKE = { thin: 0.25, medium: 0.5, heavy: 1 }`, `TRIM_INSET = 4`, `TRIM_ARM = 15`, `MARK_STROKE = 0.5`, `CENTRE_MARK_MAX = 10`, `BOX_GAP = 0.8`, `MARK_TRIANGLE_H = 0.8`
  - Functions: `frameMargins(t)`, `frameRect(t, pw, ph)`, `fontSize(t, size)`, `revisionColumns(t)`, `fittedFont(lines, font, maxW?)`, `usesExtendedRevisionTable(t)`
  - `FrameLayout` additions: `zones.show: boolean`; `marks: { x1; y1; x2; y2 }[]`; `titleBlock.gridLines[i].stroke: number`; `CellLayout.lines: string[]`, `.rotation: number`, `.box: { x; y; w; h } | null` (`size` is the fitted font size; `text` stays the joined value); `logo: { x; y; w; h; dataUrl: string | null } | null`
  - `RevisionLayout` additions: `columns: { left: number; w: number; align: CellAlign; header: RevisionText }[]`; `rows[i].cells: RevisionText[]` and `rows[i].triangle: [number, number][] | null`; `ruleYs: number[]`; with `interface RevisionText { x: number; align: CellAlign; size: number; lines: string[]; baselines: number[] }`. For a legacy table (`usesExtendedRevisionTable` false) the existing fields keep their current values and the new arrays are filled by the same rules, so the renderer can draw both from one structure.

- [ ] **Step 1: Write the failing tests**

Add to `src/canvas/frameTemplate.test.ts` (it already has `sheet()` and `rev()` and imports `jisA3`):

```ts
const base = (): FrameTemplate => structuredClone(jisA3) as FrameTemplate;
const a0 = (): Sheet => sheet([], { size: "A0", orientation: "Landscape" });

describe("frame template extensions", () => {
  // ja: 新しい項目の無いテンプレートは今までどおりの値を返す
  it("keeps today's values for a template without the new items", () => {
    const t = base();
    expect(frameMargins(t)).toEqual({ left: 10, right: 10, top: 10, bottom: 10 });
    expect(frameRect(t, 420, 297)).toEqual({ x0: 10, y0: 10, x1: 410, y1: 287 });
    expect(fontSize(t, 3)).toBe(3);
    expect(usesExtendedRevisionTable(t)).toBe(false);
    expect(revisionColumns(t).map((c) => c.field)).toEqual(["mark", "date", "description", "by"]);
    const l = frameLayout(sheet(["A"]), t);
    expect(l.zones.show).toBe(true);
    expect(l.marks).toEqual([]);
    expect(l.titleBlock.gridLines.every((g) => g.stroke === 0.25)).toBe(true);
    expect(l.titleBlock.cells.every((c) => c.rotation === 0 && c.box === null && c.lines.length === 1)).toBe(true);
  });

  // ja: 辺ごとの余白を指定すると外枠と表題欄の隅がそれに従う
  it("moves the frame and the title block corner with margins per side", () => {
    const t = base();
    t.frame_margins = { left: 30, right: 15, top: 12, bottom: 10 };
    const l = frameLayout(a0(), t);
    expect(l.frame).toEqual({ x0: 30, y0: 12, x1: 1174, y1: 831 });
    expect(l.titleBlock.x).toBe(1174 - t.title_block.w);
    expect(l.titleBlock.y).toBe(831 - t.title_block.h);
  });

  // ja: ゾーン目盛を隠すとレイアウトのshowがfalseになる
  it("reports hidden zone labels", () => {
    const t = base();
    t.zones.show = false;
    expect(frameLayout(a0(), t).zones.show).toBe(false);
  });

  // ja: 裁断マークは四隅のL字で線8本、中心マークは各辺の中央から外向きに4本
  it("lays out eight trim mark lines and four centre mark lines", () => {
    const t = base();
    t.frame_margins = { left: 30, right: 15, top: 12, bottom: 6 };
    t.marks = { trim: true, centre: true };
    const m = frameLayout(a0(), t).marks;
    expect(m).toHaveLength(12);
    expect(m).toContainEqual({ x1: 4, y1: 4, x2: 19, y2: 4 });
    expect(m).toContainEqual({ x1: 1185, y1: 837, x2: 1170, y2: 837 });
    expect(m).toContainEqual({ x1: 602, y1: 12, x2: 602, y2: 2 });
    expect(m).toContainEqual({ x1: 602, y1: 835, x2: 602, y2: 841 });
    expect(m).toContainEqual({ x1: 30, y1: 423.5, x2: 20, y2: 423.5 });
  });

  // ja: 罫線の太さは thin 0.25 / medium 0.5 / heavy 1 mm
  it("gives each rule its stroke width", () => {
    const t = base();
    t.title_block.grid_lines[0].weight = "medium";
    t.title_block.grid_lines[1].weight = "heavy";
    const g = frameLayout(sheet(), t).titleBlock.gridLines;
    expect([g[0].stroke, g[1].stroke, g[2].stroke]).toEqual([0.5, 1, 0.25]);
  });

  // ja: 大文字高さの指定ではフォントはサイズ÷0.72、fitのセルは幅に収まるまで縮む
  it("converts capital heights and shrinks fitted cells", () => {
    const t = base();
    t.title_block.cells = [{ x: 10, y: 20, w: 9, text: "ABCDEFGHIJ", size: 3, fit: true }];
    expect(frameLayout(sheet(), t).titleBlock.cells[0].size).toBeCloseTo(1.5, 6);
    t.text_size_is = "cap";
    t.title_block.cells = [{ x: 10, y: 20, text: "A", size: 3.6 }];
    expect(frameLayout(sheet(), t).titleBlock.cells[0].size).toBeCloseTo(5, 6);
  });

  // ja: 改行を含むセルは2行になり、回転と枠がレイアウトに入る
  it("splits lines and carries rotation and the box", () => {
    const t = base();
    t.title_block.cells = [{ x: 10, y: 20, text: "ABCDEFGHIJ", size: 3, rotation: 90, boxed: true }];
    const c = frameLayout(sheet(), t).titleBlock.cells[0];
    expect(c.rotation).toBe(90);
    expect(c.box).toEqual({ x: c.x - 0.8, y: c.y - 3 - 0.8, w: 18 + 1.6, h: 3.75 + 1.6 });
    t.title_block.cells = [{ x: 10, y: 20, text: "AB\nCD", size: 3 }];
    expect(frameLayout(sheet(), t).titleBlock.cells[0].lines).toEqual(["AB", "CD"]);
  });

  // ja: 独自の列を持つ改訂欄は、テンプレートの見出しと各列の項目を、指定の行高で配置する
  it("lays out a configured revision table", () => {
    const t = base();
    t.revision_block.columns = [
      { field: "mark", header: "SYMB", w: 14 },
      { field: "description", header: "CHANGELOG", w: 56 },
      { field: "rev_no", header: "ECN NO", w: 16, align: "middle" },
      { field: "date", header: "DATE", w: 12, align: "middle" },
      { field: "by", header: "BY", w: 11, align: "middle" },
      { field: "approved", header: "OKAY", w: 11, align: "end" },
    ];
    t.revision_block.row_h = 7;
    const s = sheet([]);
    s.revisions = [{ mark: "1", date: "24/01", description: "変更", by: "KT", rev_no: "E-204", approved: "MU", count: "x3" }];
    const l = revisionLayout(s, t)!;
    expect([l.x, l.y, l.w, l.h, l.rowH]).toEqual([290, 241, 120, 14, 7]);
    expect(l.columns.map((c) => c.header.lines[0])).toEqual(["SYMB", "CHANGELOG", "ECN NO", "DATE", "BY", "OKAY"]);
    const revNo = l.rows[0].cells[2];
    expect(revNo.lines).toEqual(["E-204"]);
    expect(revNo.x).toBe(368);
    expect(revNo.baselines[0]).toBeCloseTo(245.75, 6);
    expect(l.rows[0].cells[5].x).toBe(408);
  });

  // ja: 行数を固定すると、改訂が無くても表が配置され、多いときは新しい行だけになる
  it("keeps a fixed number of rows", () => {
    const t = base();
    t.revision_block.rows_fixed = 3;
    const empty = revisionLayout(sheet([]), t)!;
    expect(empty.h).toBe(4 * t.title_block.row_h);
    expect(empty.rows).toHaveLength(0);
    const full = revisionLayout(sheet(["1", "2", "3", "4", "5"]), t)!;
    expect(full.h).toBe(4 * t.title_block.row_h);
    expect(full.rows.map((r) => r.rev.mark)).toEqual(["3", "4", "5"]);
  });

  // ja: 記号の形が三角なら、記号の行に三角の頂点3つが入る
  it("adds a triangle around each mark when the mark shape is a triangle", () => {
    const t = base();
    t.revision_block.mark_shape = "triangle";
    t.revision_block.row_h = 7;
    const l = revisionLayout(sheet(["1"]), t)!;
    expect(l.rows[0].triangle).toHaveLength(3);
    expect(l.rows[0].triangle![0][0]).toBeCloseTo(290 + 2 + 5.6 / Math.sqrt(3), 6);
  });

  // ja: 読み込んだロゴ画像はレイアウトのdataUrlに入る
  it("carries the loaded logo image", () => {
    const t = base();
    t.logo = { x: 15, y: 15, w: 40, h: 20, path: "l.png", data_url: "data:image/png;base64,AAAA" };
    expect(frameLayout(sheet(), t).logo).toEqual({ x: 15, y: 15, w: 40, h: 20, dataUrl: "data:image/png;base64,AAAA" });
  });
});
```

Add the new names to the import from `./frameTemplate`.

- [ ] **Step 2: Run to verify they fail**

Run: `npx vitest run src/canvas/frameTemplate.test.ts 2>&1 | tail -12` → the new tests fail (functions not exported).

- [ ] **Step 3: Implement**

In `frameTemplate.ts`, add the types, constants and helpers named in **Interfaces**, each mirroring the Rust function of the same purpose in `frame_template.rs` / `svg.rs` (Tasks 2, 4, 5, 6): same formulas, same constants. Concretely:

```ts
export const CAP_HEIGHT_RATIO = 0.72;
export const LINE_PITCH = 1.25;
export const LINE_STROKE = { thin: 0.25, medium: 0.5, heavy: 1 } as const;
export const TRIM_INSET = 4;
export const TRIM_ARM = 15;
export const MARK_STROKE = 0.5;
export const CENTRE_MARK_MAX = 10;
export const BOX_GAP = 0.8;
export const MARK_TRIANGLE_H = 0.8;
const DESCENT_RATIO = 0.25;
const CELL_PAD = 2;
const REV_FONT = 2.5;
const REV_HEADER_FONT = 2.2;

export function frameMargins(t: FrameTemplate): FrameMargins {
  const m = t.frame_margin;
  return t.frame_margins ?? { left: m, right: m, top: m, bottom: m };
}

export function frameRect(t: FrameTemplate, pw: number, ph: number) {
  const m = frameMargins(t);
  return { x0: m.left, y0: m.top, x1: pw - m.right, y1: ph - m.bottom };
}

export function fontSize(t: FrameTemplate, size: number): number {
  return t.text_size_is === "cap" ? size / CAP_HEIGHT_RATIO : size;
}

export function fittedFont(lines: string[], font: number, maxW?: number | null): number {
  if (!maxW || maxW <= 0) return font;
  const widest = Math.max(0, ...lines.map((l) => textWidthMm(l, font)));
  return widest <= maxW ? font : (font * maxW) / widest;
}

export function revisionColumns(t: FrameTemplate): RevisionColumn[] {
  if (t.revision_block.columns) return t.revision_block.columns;
  const fields: RevisionField[] = ["mark", "date", "description", "by"];
  return t.revision_block.col_w.map((w, i) => ({ field: fields[i], header: REV_HEADERS[i], w, align: "start" as const }));
}

export function usesExtendedRevisionTable(t: FrameTemplate): boolean {
  const rb = t.revision_block;
  return rb.columns != null || rb.row_h != null || rb.rows_fixed != null
    || (rb.mark_shape ?? "none") !== "none" || (t.text_size_is ?? "font") !== "font";
}
```

`titleBlockOrigin` uses `frameRect`. `frameLayout` builds `frame` from `frameRect`, fills `zones.show = t.zones.show ?? true`, `marks` (trim then centre, in the order of `frame_marks` in `svg.rs`), `gridLines[i].stroke = LINE_STROKE[g.weight ?? "thin"]`, and for each cell: `lines = text.split("\n")`, `base = fontSize(t, cell.size)`, `size = cell.fit ? fittedFont(lines, base, cell.w) : base`, `rotation = cell.rotation ?? 0`, and `box` computed exactly as in `frame_cell` (left by alignment, `top = y − size`, `bottom = y + pitch × (lines − 1) + size × DESCENT_RATIO`, grown by `BOX_GAP`). `logo` carries `dataUrl: t.logo.data_url ?? null`.

`revisionLayout` follows the layout rules listed at the top of Task 6 for every table (legacy and extended): `rows` from `visibleRevisions(revs, rb.rows_fixed ?? rb.rows_max)`, `nRows = rb.rows_fixed ?? rows.length`, return `null` when `nRows === 0`; `rowH = rb.row_h ?? t.title_block.row_h`; `ruleYs` = the Y of the rule above every row except the top one; `columns[i].header` and `rows[i].cells[j]` are `RevisionText` built by one helper that applies the alignment x, `fittedFont` to `w − 2 × CELL_PAD`, the row-height reduction and the baseline formula; for a legacy table the fonts are `REV_HEADER_FONT` and `REV_FONT` unfitted, the alignment is `start`, and the baseline is `top + rowH / 2 + font / 2` (as today). `rows[i].triangle` holds the three points when `mark_shape === "triangle"` and the mark is not empty; the mark cell then has `align: "middle"`, `x = cx`, `baselines = [by − th × 0.12]`, `size = Math.min(bodyFont, th × 0.55)`. Keep `colLefts` and `headerTop` filled as before.

Add the three new fields to the `revision_block` object of `DEFAULT_FRAME_TEMPLATE` only if TypeScript requires them (they are optional, so it should not).

- [ ] **Step 4: Run to verify they pass**

Run: `npx vitest run src/canvas 2>&1 | tail -6 && npx vue-tsc --noEmit` → all pass, no type errors. (Existing `revisionLayout` tests must stay green without edits.)

- [ ] **Step 5: Commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/audit_confidential.py
git add -A src docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat: キャンバス側の図枠レイアウトに新しい項目を足す (Rust側と同じ規則)"
```

---

### Task 9: Canvas drawing and the frame editor keeping the new items

**Files:**
- Modify: `src/canvas/renderer.ts` (`drawFrame`, `drawRevisionBlock`), `src/canvas/frameEditor.ts` (`frameChecks`)
- Test: `src/canvas/frameEditor.test.ts`

**Interfaces:**
- Consumes: the extended `FrameLayout` / `RevisionLayout` of Task 8
- Produces: no new exports. `frameChecks` reports `revision_block.columns` instead of `revision_block.col_w` problems for templates with `columns`.

- [ ] **Step 1: Write the failing tests**

`src/canvas/frameEditor.test.ts` (read its imports and its template fixture first; `extended()` below builds on the same fixture the file already uses for a valid template — called `tpl()` here):

```ts
function extended(): FrameTemplate {
  const t = tpl();
  t.frame_margins = { left: 30, right: 15, top: 12, bottom: 10 };
  t.zones.show = false;
  t.marks = { trim: true, centre: true };
  t.text_size_is = "cap";
  t.title_block.grid_lines[0].weight = "medium";
  t.title_block.cells[0] = { ...t.title_block.cells[0], rotation: 90, boxed: true, fit: true };
  t.revision_block.columns = [
    { field: "mark", header: "SYMB", w: 14 },
    { field: "description", header: "CHANGELOG", w: t.title_block.w - 14 },
  ];
  t.revision_block.row_h = 7;
  t.revision_block.rows_fixed = 7;
  t.revision_block.mark_shape = "triangle";
  return t;
}

// ja: 新しい項目を持つテンプレートに無関係な編集(線の追加・セルの移動・削除)をしても、新しい項目は失われない
it("keeps the new items through unrelated edits", () => {
  const before = extended();
  let t = addLine(before, { x: 20, y: 20 }, { x: 60, y: 20 });
  t = moveItem(t, { kind: "cell", index: 0 }, 5, 0);
  t = deleteItem(t, { kind: "line", index: t.title_block.grid_lines.length - 1 });
  expect(t.frame_margins).toEqual(before.frame_margins);
  expect(t.zones.show).toBe(false);
  expect(t.marks).toEqual(before.marks);
  expect(t.text_size_is).toBe("cap");
  expect(t.title_block.grid_lines[0].weight).toBe("medium");
  expect(t.title_block.cells[0]).toMatchObject({ rotation: 90, boxed: true, fit: true });
  expect(t.revision_block).toMatchObject({ row_h: 7, rows_fixed: 7, mark_shape: "triangle" });
  expect(t.revision_block.columns).toEqual(before.revision_block.columns);
});

// ja: 列を自分で持つ改訂欄は、従来の4列の検査ではなく、列の幅の合計で検査される
it("checks a table with its own columns by the sum of their widths", () => {
  const good = extended();
  expect(frameChecks(good).errors).not.toContain("revision_block.col_w");
  expect(frameChecks(good).errors).not.toContain("revision_block.columns");
  const bad = extended();
  bad.revision_block.columns![1].w -= 5;
  expect(frameChecks(bad).errors).toContain("revision_block.columns");
});
```

(Use the real property name of the error list in `FrameChecks` — read the interface at `frameEditor.ts:283`.)

- [ ] **Step 2: Run to verify they fail**

Run: `npx vitest run src/canvas/frameEditor.test.ts 2>&1 | tail -10` → the second test fails; the first may already pass (the editor spreads objects). If the first passes, it stays as the regression test that fixes the behaviour.

- [ ] **Step 3: Implement**

`frameEditor.ts` `frameChecks`: when `t.revision_block.columns` is present, skip the `col_w` checks only if `col_w` is still valid for the legacy readers (it is required by the Rust side; keep the existing check for it), and add: columns count 1–8, no field twice, each `w > 0`, and `Math.abs(sum − tb.w) <= 0.01`, pushing `"revision_block.columns"` on failure. The margin check becomes: `frame_margins` present → all four ≥ 0, else `frame_margin >= 0`.

`renderer.ts` `drawFrame`:

- wrap the zone label block in `if (l.zones.show) { … }`;
- after the border, draw `l.marks` with `ctx.lineWidth = Math.max(1, MARK_STROKE * vp.scale)` and `theme.line`;
- draw grid lines grouped by stroke: for each `g`, `ctx.lineWidth = Math.max(1, g.stroke * vp.scale)` then one `beginPath / line / stroke` per line (replaces the single path);
- cells: for each cell, `ctx.save()`; if `cell.rotation`, `const p = vp.toScreen({ x: cell.x, y: cell.y }); ctx.translate(p.x, p.y); ctx.rotate((cell.rotation * Math.PI) / 180); ctx.translate(-p.x, -p.y);`; draw each line of `cell.lines` at `y + i * cell.size * LINE_PITCH`; if `cell.box`, `strokeRect` it with line width 1; `ctx.restore()`;
- logo: when `l.logo.dataUrl` is set, draw the image centred with its aspect ratio kept (`drawImage`) using a module-level cache `Map<string, HTMLImageElement>`; the first time, create the `Image`, set `onload` to request a redraw through the same callback the renderer already uses for asynchronous redraws (search `renderer.ts` for an existing `onload` or redraw hook; if none exists, export `setFrameImageLoaded(cb: () => void)` and call it from `CanvasView.vue` with its redraw function); while the image is not loaded, draw the placeholder rectangle.

`drawRevisionBlock`: draw the outer rectangle, the lines at `l.ruleYs`, the column dividers at `l.columns[i].left` (skip the first), then every `RevisionText` (header of each column, cells of each row) with `ctx.textAlign = canvasAlign(text.align)` and one `drawingText` per line at its baseline, and `l.rows[i].triangle` as a closed path when present.

Verify in the running app: with the development build on port 9320 running and the project of the reproduction open, open http://localhost:1420 in the browser tool and take a screenshot of the title block corner; rotated text, the revision table and the trim marks must be visible. (This check is repeated properly in Task 10 after the template uses the new items.)

- [ ] **Step 4: Run to verify they pass**

Run: `npx vitest run 2>&1 | tail -6 && npx vue-tsc --noEmit` → all pass, no type errors.

- [ ] **Step 5: Commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/audit_confidential.py
git add -A src docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat: キャンバスに図枠の新しい項目を描き、図枠エディタが新しい項目を保持する"
```

---

### Task 10: Documentation, reproduction check, ledger

**Files:**
- Modify: `docs/internal/data-model.md`, `docs/internal/feature-inventory.md`, `docs/internal/specs/reproduction-gaps.md`, `docs/internal/specs/reproduction-gaps.ja.md`, the spec pair if Task 1 limited the image types
- Modify (git-ignored): `docs/references/repro/a0/make_frame.py`, `revisions.json`, `title_block.json`, `NOTES.md`

**Interfaces:**
- Consumes: everything above

- [ ] **Step 1: Update the reproduction's template**

In the git-ignored `make_frame.py`, replace every hand-made workaround by the new template items: capital-height sizes instead of a multiplied size, margins per side, hidden zone labels, trim and centre marks, medium weight on the main rules, the revision table's own columns, row height, fixed rows and mark shape, a free field for the second title line, rotated cells for the note printed along the edge, a boxed rotated label, and the logo image cropped from the reference into the user frame folder. Update the git-ignored `revisions.json` and `title_block.json` with the new revision fields and the free field. The measured values and the strings stay in those git-ignored files and in `NOTES.md`; none of them is written into a committed file.

- [ ] **Step 2: Rebuild, draw and compare**

Restart the development build so that it runs the new code (stop the running `tauri dev`, start it again with `MADAKE_MCP_PORT=9320`), then:

Run: `cd docs/references/repro/a0 && python3 make_frame.py && A0_REF_PDF="<path>" ./run.sh r1_frame`
Then crop the title block corner and the areas outside the frame from `out/r1_frame-diff.png` and open them with Read.
Expected: no red or blue **rule** remains in the title block, the revision table or the frame border (text glyph differences are expected); the trim marks and centre marks lie on the reference's. Differences that remain are listed in `NOTES.md` and, if they are a MadakeCAD limitation, become new ledger entries.

Also open http://localhost:1420 with the browser tool and screenshot the title block corner: the canvas shows the same frame as the PDF.

- [ ] **Step 3: Update the documents**

- `docs/internal/data-model.md`: find the frame template and revision sections (`grep -n "frame_margin\|Revision\|TitleBlock" docs/internal/data-model.md`) and add the new fields with one line each; note format version 20.
- `docs/internal/feature-inventory.md`: in the frame template row, add the new items; add a line to the history paragraph at the end in the style of the existing entries.
- `docs/internal/specs/reproduction-gaps.md` and `.ja.md`: in the fifth-pass table change G24–G31 rows to ✅ (or 🔶 with the remaining difference), and add at the top of each of the entries G24–G31 a line `**Closed 2026-11-14** (frame template extensions).` / `**解消 2026-11-14**(図枠テンプレートの拡張)。`.
- If Task 1 limited the image types: edit the G31 section of both spec files to say PNG and JPEG.

- [ ] **Step 4: Full verification**

Run: `cd src-tauri && cargo test --workspace 2>&1 | grep -E "^test result|FAILED|failed"` → all ok.
Run: `npx vitest run 2>&1 | tail -4 && npx vue-tsc --noEmit` → pass.
Run: `python3 -m unittest discover -s freecad-addon/tests 2>&1 | tail -2` → OK.
Run: `python3 scripts/gen_spec.py --check && python3 scripts/audit_confidential.py` → up to date; no confidential terms found.

- [ ] **Step 5: Commit**

```bash
git add docs/internal docs/superpowers/specs docs/13-specification.md docs/13-specification.ja.md
git commit -m "docs: 図枠テンプレートの拡張をデータ設計・機能一覧・ギャップ台帳へ反映"
```
