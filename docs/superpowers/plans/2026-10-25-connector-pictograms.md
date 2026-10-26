# Connector Pictograms Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A block pin can name the physical connector it is (`connector`, 13 fixed kinds, plus `connector_ref` for screw terminals and JST sockets), and the block draws a small monochrome picture of it on the box edge, identically in Rust (SVG / PDF / PNG / `get_sheet_image`, and DXF / KiCad through per-shape block definitions) and in TypeScript (canvas, editor preview).

**Architecture:**
- **Data and validation.** `BlockPin` gains `connector: Option<ConnectorKind>` and `connector_ref: Option<String>`. A new module `madake-core/src/connector.rs` owns the kinds, the grouping, the labels, the pictures (local edge frames) and the gapped outline. `block.rs` calls it from validation, `block_box` and `block_symbol_from_spec`.
- **Twin.** `src/canvas/connector.ts` and `src/canvas/dynamicSymbol.ts` mirror every function in the same order. A shared golden file `src-tauri/crates/madake-core/tests/golden/connector-blocks.json` is written by a Rust test and compared by a Vitest test. It is introduced in Task 3, before any geometry, and every geometry task regenerates it and then makes the TS twin match it.
- **File format 19.** A block with any connector is also saved as the file-only kind `port_block`.
- **Exporters.** DXF and KiCad write one definition per distinct block shape (Tasks 12, 13; spec §3.1) and read our own names back as blocks.
- **UI, MCP, agent, docs** follow the core.

**Tech Stack:** Rust (madake-core, madake-mcp, madake-agent; serde 1, serde_json 1 without `preserve_order`), Vue 3 + TypeScript (Vitest, vue-tsc), Tauri 2.

**Spec:** `docs/superpowers/specs/2026-10-25-connector-pictograms-design.md` (English only, user-approved, binding). Read it first. It has the pictures (§2.1), the pin / label positions (§2.2), the outline (§2.3), the box size (§2.4), the no-overlap rule (§2.5) and the decision log.

## Global Constraints

- Work only in `/Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms` on branch `feature/connector-pictograms`. Never touch other worktrees or the main checkout. Never switch branches.
- Every command in this plan starts with `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms`.
- Before trusting any cargo result, run: `touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs`. The shared `CARGO_TARGET_DIR=/Users/ktakeuchi/Documents/MadakeCAD/src-tauri/target` can link another branch's rlib. Every cargo command below already starts with this `touch`.
- `cargo test` takes **one** name filter per run. Never list several test names in one command.
- Rust tests: exactly two `///` lines directly above `#[test]` / `#[tokio::test]`, English first, then Japanese. Vitest: a `// ja: …` line directly above each `it(…)`.
- After adding or renaming tests: `python3 scripts/gen_spec.py`, then `python3 scripts/gen_spec.py --check`. Expected: `spec docs up to date; all tests have bilingual spec comments`. Commit the regenerated `docs/13-specification.md` and `docs/13-specification.ja.md` with the task.
- A test that already passes before its task is written is labelled **"Regression test … (already passes)"** in its English doc line. Every other new test must fail first, and its step states the exact RED.
- Reuse existing fixtures: `block::tests::{attrs, pin_at, texts, text_at, rect_of, SAMPLE}`, `crate::block::test_block`, `io::tests::{port_project, save_text}`, `verify::tests::{sheet_with, symbol, wire, run}`, MCP `tests/drawing_tools.rs` `{fixture, first_sheet, call, call_err, place_block}`. No new MCP test file.
- Earlier tasks edit the same files. Find code by its content (quoted in each step), never by line number.
- Frontend: if `node_modules` is missing in the worktree, run `ln -s /Users/ktakeuchi/Documents/MadakeCAD/node_modules node_modules` (git-ignored). That `node_modules` lacks `markdown-it`, so `src/chat/markdown.test.ts` cannot load and `npx vue-tsc --noEmit` reports errors in `src/chat/markdown.ts`. Both are pre-existing and not failures of this work. `src/stores/terminals.test.ts` is a known flake: re-run it alone before treating it as a failure.
- Commits: Conventional Commits with a Japanese subject. **No `Co-Authored-By` or any other AI attribution trailer.** Add files by path (never `git add -A` / `git add .`), never commit `.superpowers/`. Do not push. The whole branch becomes one PR.
- Product docs keep EN + `.ja.md`. Plans and specs under `docs/superpowers/` are English only.
- Values copied from the spec, exactly:
  - Kinds, in this order: `usb_a_f usb_a_m usb_c_f usb_c_m micro_b_f micro_b_m jack35_f jack35_m dc_f dc_m screw jst_ph_f jst_xh_f`. Grouped: `screw`, `jst_ph_f`, `jst_xh_f`. Series: `PH` / `XH`.
  - `CONNECTOR_IN_MM` 1.5, `CONNECTOR_OUT_MM` 2.5, `CONNECTOR_HALF_SPAN_MM` 2.0, `CONNECTOR_LEAD_MM` 5.0, `CONNECTOR_NUM_GAP_MM` 0.25, `CONNECTOR_LABEL_TOP_MM` 7.0, `CONNECTOR_LABEL_INSET_MM` 10.0. This plan adds two named constants for literals in spec §2.2: `CONNECTOR_NUM_SIDE_MM` 0.5 and `CONNECTOR_LABEL_GAP_MM` 1.0.
  - Messages, in this check order: (1) serde's `block_pins is not valid JSON: unknown variant …` (TS: `block_pins is not valid JSON: unknown connector {value}`); (2) `block pin {no}: connector_ref must not be empty`; (3) `block pin {no}: connector_ref needs a grouped connector (screw, jst_ph_f, jst_xh_f)`; (4) `block pin {no}: connector_ref is only for screw, jst_ph_f and jst_xh_f, not {kind}`; (5) `block pin {no}: connector_ref {ref} is used by another group; its pins must be next to each other on one side`.
  - Labels: `<ref> PH` / `<ref> XH`, `PH` / `XH` without ref, `<ref>` for screw, nothing for an unnamed screw block.
  - `FORMAT_VERSION` = 19. A block with any contact **or any connector** is written as `port_block`. A file without connectors is written byte for byte as before, apart from the version.
  - Pins without a connector are drawn exactly as today. A block without connectors keeps its single outline `Rect`.

## Review Focus

1. An agent that writes `"connector": null` or `"connector_ref": null` (common in generated JSON) should get a plain pin, in Rust and in TS alike, not an error or a twin mismatch. Tests: Task 1 `a_connector_kind_is_read_and_an_unknown_one_is_refused` (null part), Task 2 `reads a connector kind, treats null as absent …`.
2. A pin that is already wired and then gets a connector moves its point 2.5 mm outward. The user must be told by ERC: the old wire end dangles (`erc.dangling_wire`), or, when the wire left in another direction, the pin is named unconnected (`erc.unconnected_pin`). Nothing may break silently. Test: Task 7 `a_wired_pin_that_gets_a_connector_is_flagged_by_erc`.
3. A rotated or mirrored block must still show a closed USB-C opening: each arc ends where a straight edge ends and bulges outward, on every side and at every rotation and mirror. Test: Task 4 `the_usb_c_opening_stays_closed_at_every_rotation_and_mirror`.
4. A group name typed as spaces only in the editor must not be written, must not raise an issue and must not block the preview. Test: Task 9 `drops a group name made only of spaces without reporting it`.
5. A block whose connectors are all removed again must be saved as `symbol` again, so older builds can open the file. Test: Task 8 `a_block_with_connectors_is_saved_as_port_block` (second half).

---

## Rulings (this plan)

- **DXF and KiCad show each block's real drawing (user decision 2026-10-25, overturning this plan's first ruling).** Both exporters picked one definition per `symbol_id` (`defs.get(inst.symbol_id)`), so every block exported as the empty `block` template (a 1-pin and an 8-pin block gave identical output). Tasks 12 and 13 export one definition per distinct block shape (`MDK_block_<16 hex>`, `MadakeCAD:block_<16 hex>`, key = FNV-1a 64 of the drawn primitives + pins, spec §3.1), keep `block` for a template-equal block and keep library symbols byte-identical. The importers map those names back to `block` and restore `block_pins` / `block_size` from the DXF attributes / KiCad properties; graphics are not read back. Cost if wrong: names in exported files only (key scheme), or two importer branches.
- **A wired pin that gets a connector does not always lose its wire (spec §2.2 says it does).** A pin connects to any point of a wire (`netlist.rs`, "ピン ⇔ Wire上の任意点"). A wire that leaves the pin straight outward still passes through the new pin point, so the net is unchanged and only the old wire end dangles. Review Focus 2 pins the ERC signal. Cost if wrong: none in code; the docs wording changes.
- **Commit grouping.** The spec's Delivery lists four commit groups (core, TS, UI, MCP/docs). This plan pairs Rust and TS inside each geometry task (Tasks 5 and 6), so the golden file never disagrees with the TS twin at any commit. There are 14 commits, still one PR. Cost if wrong: history shape only.
- **Golden file shape.** One case per line (diffable), 13 kinds × 4 sides as the connector pin alone (grouped kinds: two pins with ref `J1`), plus `aogu-hub-39`, `aogu-amp-53`, `mixed-left-right`, `top-bottom` and `plain` (the existing `SAMPLE`, no connectors). About 80 KB. The TS twin of the §2.5 no-overlap test is the golden equality on the same four fixtures: TS has no second copy of the overlap checker.
- **TS-only input checks.** TS cannot rely on serde, so `parseBlockSpec` also refuses a non-string `connector_ref` with `block_pins is not valid JSON: connector_ref must be a string` (Rust refuses it during deserialisation). `null` is treated as absent in both. Cost if wrong: one message.
- **Group-name clearing** lives in a pure `patchRow` in `blockEditor.ts`. The store's `updateRow` calls it, so the store test covers it.
- **Language of agent-facing text.** `knowledge.rs` and the MCP descriptions are Japanese today, so the new sentences are Japanese with the same content as spec §6.
- **Not in this plan** (spec Out of scope / Follow-up): library mate symbols, `MadakeCAD.pen`, colour, filling `contacts` from the kind, any electrical meaning, AOGU drawing changes, the `drawing-madakecad-wiring` skill.

## File map

| File | Responsibility in this plan |
|---|---|
| `src-tauri/crates/madake-core/src/connector.rs` (new) | `ConnectorKind`, constants, `connector_groups`, `group_label`, edge frames, `connector_primitives`, `outline_segments`, test helper `primitive_extent` |
| `src-tauri/crates/madake-core/src/lib.rs` | `pub mod connector;` |
| `src-tauri/crates/madake-core/src/block.rs` | `block_export_id`, `is_block_export_id`, `export_def`, `imported_block_attrs`, `export_blocks_sheet`; `BlockPin` fields, `check_connectors`, `has_connectors`, `side_pad`, `side_text_width`, `caption_inset`, `block_box`, `edge_point`, `pin_layout`, `label_at`, `block_symbol_from_spec`, golden test and fixtures, overlap test |
| `src-tauri/crates/madake-core/tests/golden/connector-blocks.json` (new, generated) | shared Rust ↔ TS golden |
| `src-tauri/crates/madake-core/src/command.rs` | refusal test only |
| `src-tauri/crates/madake-core/src/dxf.rs`, `kicad.rs` | per-shape block definitions on export, `block` + pin table on import (Tasks 12, 13) |
| `src-tauri/crates/madake-core/src/svg.rs`, `verify.rs` | regression tests only |
| `src-tauri/crates/madake-core/src/model.rs`, `io.rs` | format 19, `is_port_block`, doc comments, tests |
| `src/canvas/connector.ts` (new) | TS twin of `connector.rs` |
| `src/canvas/dynamicSymbol.ts`, `dynamicSymbol.test.ts` | TS twin of `block.rs`, golden and validation tests |
| `src/canvas/blockEditor.ts`, `blockEditor.test.ts`, `src/stores/blockEditor.ts`, `src/stores/blockEditor.test.ts` | draft rows, `patchRow`, issues |
| `src/components/BlockPinColumn.vue`, `BlockPinsEditor.vue`, `BlockEditorDialog.vue`, `SymbolPickerDialog.vue` | columns, issue texts, widths |
| `src/locales/{en,ja,zh,fr,de,es}.json`, `src/i18n/i18n.test.ts` | labels |
| `src-tauri/crates/madake-mcp/src/lib.rs`, `tests/drawing_tools.rs` | descriptions, tests |
| `src-tauri/crates/madake-agent/src/knowledge.rs`, `tests/knowledge.rs` | rule bullet, test |
| `docs/03-schematic-editor(.ja).md`, `docs/05-wire-management(.ja).md`, `docs/10-automation-api(.ja).md`, `docs/internal/feature-inventory.md` | docs |

---

### Task 1: Connector kinds, grouping and validation (Rust)

**Files:**
- Create: `src-tauri/crates/madake-core/src/connector.rs`
- Modify: `src-tauri/crates/madake-core/src/lib.rs` (`pub mod command;` line)
- Modify: `src-tauri/crates/madake-core/src/block.rs` (imports, `BlockPin`, `impl BlockSpec`, `parse_block_spec`, new `check_connectors`, test module)
- Modify: `src-tauri/crates/madake-core/src/command.rs` (test module only)

**Interfaces:**
- Consumes: nothing new.
- Produces:
  - `pub enum ConnectorKind { UsbAF, UsbAM, UsbCF, UsbCM, MicroBF, MicroBM, Jack35F, Jack35M, DcF, DcM, Screw, JstPhF, JstXhF }` (`Deserialize`, `rename_all = "snake_case"`, `Copy`, `Eq`)
  - `impl ConnectorKind { pub const ALL: [ConnectorKind; 13]; pub fn name(self) -> &'static str; pub fn is_grouped(self) -> bool; pub fn series(self) -> Option<&'static str> }`
  - `pub struct ConnectorGroup { pub kind: ConnectorKind, pub reference: Option<String>, pub start: usize, pub end: usize }`
  - `pub fn connector_groups(side: &[BlockPin]) -> Vec<ConnectorGroup>`, `pub fn group_label(group: &ConnectorGroup) -> Option<String>`
  - `BlockPin { …, pub connector: Option<ConnectorKind>, pub connector_ref: Option<String> }`, `impl BlockSpec { pub fn has_connectors(&self) -> bool }`

- [ ] **Step 1: Create `connector.rs` with its tests, and register the module**

Create `src-tauri/crates/madake-core/src/connector.rs`:

```rust
//! ブロックのピンのコネクタの絵 (USB・3.5mm・DC・端子台・JST)。
//!
//! ピン表のピンが `connector` (どの形のコネクタか) を持つと、外形の辺にそのコネクタの小さな絵を描く。
//! 絵は既存のプリミティブ (線・円・弧・長方形) だけで作り、ピンが外形と交わる点 E を原点にした
//! ローカル座標 (u = 外向き、v = 辺に沿う向き) で決める。`connector` は絵だけで、接点・ネットは変えない。
//! TS側 `src/canvas/connector.ts` と同じ数・同じ順 (`tests/golden/connector-blocks.json` で突き合わせる)。

use serde::Deserialize;

use crate::block::BlockPin;

/// コネクタの形 (13種、決め打ち)。`_f` = メス (機器側の口)、`_m` = オス (プラグ)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorKind {
    UsbAF,
    UsbAM,
    UsbCF,
    UsbCM,
    MicroBF,
    MicroBM,
    Jack35F,
    Jack35M,
    DcF,
    DcM,
    Screw,
    JstPhF,
    JstXhF,
}

impl ConnectorKind {
    /// 全種類 (仕様の表の順。エディタの選択肢もこの順)。
    pub const ALL: [ConnectorKind; 13] = [
        ConnectorKind::UsbAF,
        ConnectorKind::UsbAM,
        ConnectorKind::UsbCF,
        ConnectorKind::UsbCM,
        ConnectorKind::MicroBF,
        ConnectorKind::MicroBM,
        ConnectorKind::Jack35F,
        ConnectorKind::Jack35M,
        ConnectorKind::DcF,
        ConnectorKind::DcM,
        ConnectorKind::Screw,
        ConnectorKind::JstPhF,
        ConnectorKind::JstXhF,
    ];

    /// JSON での名前 (`usb_a_f` など)。
    pub fn name(self) -> &'static str {
        match self {
            ConnectorKind::UsbAF => "usb_a_f",
            ConnectorKind::UsbAM => "usb_a_m",
            ConnectorKind::UsbCF => "usb_c_f",
            ConnectorKind::UsbCM => "usb_c_m",
            ConnectorKind::MicroBF => "micro_b_f",
            ConnectorKind::MicroBM => "micro_b_m",
            ConnectorKind::Jack35F => "jack35_f",
            ConnectorKind::Jack35M => "jack35_m",
            ConnectorKind::DcF => "dc_f",
            ConnectorKind::DcM => "dc_m",
            ConnectorKind::Screw => "screw",
            ConnectorKind::JstPhF => "jst_ph_f",
            ConnectorKind::JstXhF => "jst_xh_f",
        }
    }

    /// 並んだ数本のピンで1つの本体を描く種類 (端子台・JST)。`connector_ref` はこれだけが持てる。
    pub fn is_grouped(self) -> bool {
        matches!(self, ConnectorKind::Screw | ConnectorKind::JstPhF | ConnectorKind::JstXhF)
    }

    /// JST のシリーズ名 (`PH` / `XH`)。他は `None`。
    pub fn series(self) -> Option<&'static str> {
        match self {
            ConnectorKind::JstPhF => Some("PH"),
            ConnectorKind::JstXhF => Some("XH"),
            _ => None,
        }
    }
}

/// 1辺の中の、1つの絵で描くピンのまとまり (`start..end` は辺のピン表の添字)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorGroup {
    pub kind: ConnectorKind,
    /// 前後の空白を落とした `connector_ref`。無ければ `None`。
    pub reference: Option<String>,
    pub start: usize,
    pub end: usize,
}

/// 1辺のピンをまとまりに分ける。まとめる種類のピンは、すぐ前のピンと種類も `connector_ref` も
/// 同じなら同じまとまりに入る (無い同士も同じ)。まとめない種類は1本で1つ。`connector` の無いピンは入らない。
pub fn connector_groups(side: &[BlockPin]) -> Vec<ConnectorGroup> {
    let mut groups: Vec<ConnectorGroup> = Vec::new();
    for (i, pin) in side.iter().enumerate() {
        let Some(kind) = pin.connector else { continue };
        let reference = pin.connector_ref.as_deref().map(|r| r.trim().to_string());
        if kind.is_grouped() {
            if let Some(last) = groups.last_mut() {
                if last.end == i && last.kind == kind && last.reference == reference {
                    last.end = i + 1;
                    continue;
                }
            }
        }
        groups.push(ConnectorGroup { kind, reference, start: i, end: i + 1 });
    }
    groups
}

/// まとまりの名前: JST は `<ref> PH` / `<ref> XH` (ref が無ければ `PH` / `XH`)、端子台は `<ref>` (無ければ書かない)。
pub fn group_label(group: &ConnectorGroup) -> Option<String> {
    match (group.kind.series(), &group.reference) {
        (Some(series), Some(r)) => Some(format!("{r} {series}")),
        (Some(series), None) => Some(series.to_string()),
        (None, Some(r)) if group.kind.is_grouped() => Some(r.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::BlockPins;

    fn left_side(json: &str) -> Vec<BlockPin> {
        let pins: BlockPins = serde_json::from_str(json).expect("ピン表");
        pins.left
    }

    fn spans(json: &str) -> Vec<(usize, usize)> {
        connector_groups(&left_side(json)).iter().map(|g| (g.start, g.end)).collect()
    }

    fn label(json: &str) -> Option<String> {
        group_label(&connector_groups(&left_side(json))[0])
    }

    /// Every connector kind is read from its snake_case name (usb_a_f … jst_xh_f); only screw terminals and JST sockets are grouped, and only JST has a series.
    /// コネクタの種類はどれも snake_case の名前 (usb_a_f … jst_xh_f) で読め、まとめて描くのは端子台と JST だけで、シリーズ名を持つのは JST だけ。
    #[test]
    fn every_connector_kind_has_its_json_name() {
        let names: Vec<&str> = ConnectorKind::ALL.iter().map(|k| k.name()).collect();
        assert_eq!(
            names,
            [
                "usb_a_f", "usb_a_m", "usb_c_f", "usb_c_m", "micro_b_f", "micro_b_m", "jack35_f",
                "jack35_m", "dc_f", "dc_m", "screw", "jst_ph_f", "jst_xh_f"
            ]
        );
        for kind in ConnectorKind::ALL {
            let read: ConnectorKind = serde_json::from_str(&format!("\"{}\"", kind.name())).expect("読める");
            assert_eq!(read, kind);
        }
        let grouped: Vec<&str> = ConnectorKind::ALL.iter().filter(|k| k.is_grouped()).map(|k| k.name()).collect();
        assert_eq!(grouped, ["screw", "jst_ph_f", "jst_xh_f"]);
        assert_eq!(ConnectorKind::JstPhF.series(), Some("PH"));
        assert_eq!(ConnectorKind::JstXhF.series(), Some("XH"));
        assert_eq!(ConnectorKind::Screw.series(), None);
    }

    /// Consecutive screw or JST pins with the same kind and the same name (or both unnamed) form one connector; a different name, a different kind or a pin in between starts a new one, and USB-like kinds are always one pin each.
    /// 同じ種類で同じ名前 (どちらも名前なしを含む) の端子台・JST のピンが続けば1つのコネクタになり、名前・種類が違うか間に別のピンがあれば別のコネクタになる。USB などはいつも1本で1つ。
    #[test]
    fn consecutive_pins_with_the_same_kind_and_name_form_one_connector() {
        assert_eq!(
            spans(r#"{"left":[{"no":"1","connector":"screw","connector_ref":"TB1"},{"no":"2","connector":"screw","connector_ref":" TB1 "}]}"#),
            [(0, 2)]
        );
        assert_eq!(spans(r#"{"left":[{"no":"1","connector":"screw"},{"no":"2","connector":"screw"}]}"#), [(0, 2)]);
        assert_eq!(
            spans(r#"{"left":[{"no":"1","connector":"screw","connector_ref":"TB1"},{"no":"2","connector":"screw","connector_ref":"TB2"}]}"#),
            [(0, 1), (1, 2)]
        );
        assert_eq!(
            spans(r#"{"left":[{"no":"1","connector":"jst_ph_f"},{"no":"2","connector":"jst_xh_f"}]}"#),
            [(0, 1), (1, 2)]
        );
        assert_eq!(
            spans(r#"{"left":[{"no":"1","connector":"screw"},{"no":"2"},{"no":"3","connector":"screw"}]}"#),
            [(0, 1), (2, 3)]
        );
        assert_eq!(
            spans(r#"{"left":[{"no":"1","connector":"usb_a_f"},{"no":"2","connector":"usb_a_f"}]}"#),
            [(0, 1), (1, 2)]
        );
    }

    /// A JST connector is labelled with its name and series ("J3 PH", or "XH" alone), a screw terminal block with its name, and an unnamed screw block or a USB socket gets no label.
    /// JST のコネクタには名前とシリーズ (「J3 PH」、名前が無ければ「XH」だけ)、端子台には名前を書き、名前の無い端子台と USB の口には何も書かない。
    #[test]
    fn group_labels_name_the_connector_and_its_series() {
        assert_eq!(label(r#"{"left":[{"no":"1","connector":"jst_ph_f","connector_ref":"J3"}]}"#).as_deref(), Some("J3 PH"));
        assert_eq!(label(r#"{"left":[{"no":"1","connector":"jst_xh_f"}]}"#).as_deref(), Some("XH"));
        assert_eq!(label(r#"{"left":[{"no":"1","connector":"screw","connector_ref":"TB1"}]}"#).as_deref(), Some("TB1"));
        assert_eq!(label(r#"{"left":[{"no":"1","connector":"screw"}]}"#), None);
        assert_eq!(label(r#"{"left":[{"no":"1","connector":"usb_a_f"}]}"#), None);
    }
}
```

In `src-tauri/crates/madake-core/src/lib.rs`, replace `pub mod command;` with:

```rust
pub mod command;
pub mod connector;
```

- [ ] **Step 2: Write the failing block tests** (append inside `mod tests` of `block.rs`, after `bad_contacts_are_refused`)

```rust
    // ---- コネクタの絵 (connector / connector_ref) ----

    /// A pin may name its physical connector, also together with contacts, and null means no connector; an unknown kind is refused as invalid JSON.
    /// ピンは自分のコネクタの形を書ける (接点と一緒でもよい)。null は書かなかったのと同じ。知らない種類は壊れた JSON として断る。
    #[test]
    fn a_connector_kind_is_read_and_an_unknown_one_is_refused() {
        let spec = parse_block_spec(&attrs(&[(
            "block_pins",
            r#"{"top":[{"no":"U1","connector":"usb_a_f","contacts":["VBUS","D-","D+","GND"]}]}"#,
        )]))
        .expect("コネクタと接点の両方を持てる");
        assert_eq!(spec.pins.top[0].connector, Some(ConnectorKind::UsbAF));
        assert!(spec.has_connectors() && spec.has_contacts());
        assert!(!parse_block_spec(&attrs(&[("block_pins", SAMPLE)])).unwrap().has_connectors());
        let nulls = parse_block_spec(&attrs(&[("block_pins", r#"{"left":[{"no":"1","connector":null,"connector_ref":null}]}"#)]))
            .expect("null は書かなかったのと同じ");
        assert!(!nulls.has_connectors());
        let err = parse_block_spec(&attrs(&[("block_pins", r#"{"left":[{"no":"1","connector":"usb_z"}]}"#)]))
            .expect_err("知らない種類");
        assert!(err.starts_with("block_pins is not valid JSON") && err.contains("unknown variant"), "{err}");
    }

    /// A connector name (connector_ref) must not be blank, needs a screw or JST connector and is refused on any other kind, with the pin number in the message.
    /// コネクタの名前 (connector_ref) は空ではいけない・端子台か JST のコネクタが要る・ほかの種類には付けられない。断る文にはピン番号が入る。
    #[test]
    fn a_bad_connector_ref_is_refused() {
        let err = |json: &str| parse_block_spec(&attrs(&[("block_pins", json)])).expect_err("断る");
        assert_eq!(
            err(r#"{"left":[{"no":"P1","connector":"screw","connector_ref":"  "}]}"#),
            "block pin P1: connector_ref must not be empty"
        );
        assert_eq!(
            err(r#"{"left":[{"no":"P1","connector_ref":"J3"}]}"#),
            "block pin P1: connector_ref needs a grouped connector (screw, jst_ph_f, jst_xh_f)"
        );
        assert_eq!(
            err(r#"{"left":[{"no":"P1","connector":"usb_a_f","connector_ref":"J3"}]}"#),
            "block pin P1: connector_ref is only for screw, jst_ph_f and jst_xh_f, not usb_a_f"
        );
    }

    /// One connector name may not be used by two separate connectors: on two sides, split by another pin, or with two different kinds.
    /// 1つのコネクタの名前を別々の2つのコネクタに使うことはできない (2つの辺に分かれる・別のピンで途切れる・種類が違う)。
    #[test]
    fn a_connector_ref_used_by_two_groups_is_refused() {
        let msg = "block pin 2: connector_ref J3 is used by another group; its pins must be next to each other on one side";
        for json in [
            r#"{"left":[{"no":"1","connector":"jst_ph_f","connector_ref":"J3"}],"right":[{"no":"2","connector":"jst_ph_f","connector_ref":"J3"}]}"#,
            r#"{"left":[{"no":"1","connector":"jst_ph_f","connector_ref":"J3"},{"no":"X"},{"no":"2","connector":"jst_ph_f","connector_ref":"J3"}]}"#,
            r#"{"left":[{"no":"1","connector":"screw","connector_ref":"J3"},{"no":"2","connector":"jst_ph_f","connector_ref":"J3"}]}"#,
        ] {
            let err = parse_block_spec(&attrs(&[("block_pins", json)])).expect_err("断る");
            assert_eq!(err, msg, "{json}");
        }
    }

    /// Unnamed connectors may repeat anywhere, and a named connector may run over several neighbouring pins.
    /// 名前の無いコネクタはどこで繰り返してもよく、名前のあるコネクタは隣り合う数本のピンにまたがってよい。
    #[test]
    fn unnamed_connectors_may_repeat() {
        parse_block_spec(&attrs(&[(
            "block_pins",
            r#"{"left":[{"no":"1","connector":"screw"},{"no":"2"},{"no":"3","connector":"screw"}],
                "right":[{"no":"4","connector":"jst_ph_f","connector_ref":"J3"},{"no":"5","connector":"jst_ph_f","connector_ref":"J3"}],
                "top":[{"no":"6","connector":"jst_xh_f"},{"no":"7","connector":"usb_a_f"},{"no":"8","connector":"usb_a_f"}]}"#,
        )]))
        .expect("通る");
    }
```

Append inside `mod tests` of `command.rs`, after `editing_a_block_into_a_broken_one_is_refused`:

```rust
    /// A block whose connector table is wrong (a connector name on a USB socket, or a blank name) is refused when added or edited, with the reason.
    /// コネクタの表が誤っているブロック (USB の口にコネクタの名前、または空の名前) は、追加でも編集でも理由付きで拒否される。
    #[test]
    fn a_block_with_a_bad_connector_table_is_refused() {
        let (mut engine, sheet_id) = test_engine();
        let bad = crate::block::test_block("U1", "", 100.0, 50.0, r#"{"left":[{"no":"1","connector":"usb_a_f","connector_ref":"J3"}]}"#);
        let err = engine.execute(Command::AddEntity { sheet_id, entity: bad }).expect_err("断る");
        assert!(matches!(&err, CoreError::InvalidCommand(m) if m.contains("connector_ref is only for")), "{err:?}");
        let good = crate::block::test_block("U1", "", 100.0, 50.0, r#"{"left":[{"no":"1","connector":"usb_a_f"}]}"#);
        let id = good.id();
        engine.execute(Command::AddEntity { sheet_id, entity: good }).expect("正しいコネクタは置ける");
        let Entity::Symbol(mut s) = engine.project().sheet(sheet_id).unwrap().entities[&id].clone() else {
            unreachable!()
        };
        s.attrs.insert("block_pins".into(), r#"{"left":[{"no":"1","connector":"screw","connector_ref":""}]}"#.into());
        let err = engine
            .execute(Command::UpdateEntity { sheet_id, entity: Entity::Symbol(s) })
            .expect_err("空の名前は断る");
        assert!(matches!(&err, CoreError::InvalidCommand(m) if m.contains("connector_ref must not be empty")), "{err:?}");
    }
```

- [ ] **Step 3: Run to verify RED**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core connector 2>&1 | grep -E "^error|cannot find|no field" | head`
Expected: compile errors: `block.rs` cannot find `ConnectorKind` and `has_connectors`, and `connector.rs` finds no field `connector` / `connector_ref` on `BlockPin`.

- [ ] **Step 4: Implement in `block.rs`**

Below `use serde::Deserialize;` add (keep the blank line before `use crate::geometry::Point;`):

```rust
use crate::connector::{connector_groups, ConnectorKind};
```

In `pub struct BlockPin`, replace

```rust
    #[serde(default)]
    pub contacts: Vec<String>,
}
```

with

```rust
    #[serde(default)]
    pub contacts: Vec<String>,
    /// 外形の辺に描くコネクタの形。`None` = 普通のピン (今までどおりに描く)。絵だけで、接点・ネットは変えない。
    #[serde(default)]
    pub connector: Option<ConnectorKind>,
    /// まとめて1つの本体に描くコネクタ (端子台・JST) の名前 (例 `"J3"`, `"TB1"`)。
    #[serde(default)]
    pub connector_ref: Option<String>,
}
```

In `impl BlockSpec`, after `has_contacts`, add:

```rust

    /// どれかのピンがコネクタの絵を持つか (外形を線分で描き、ファイルでは種類 `port_block` で書く)。
    pub fn has_connectors(&self) -> bool {
        self.pins.all().any(|p| p.connector.is_some())
    }
```

In `parse_block_spec`, replace

```rust
        check_contacts(no, &pin.contacts)?;
    }
```

with

```rust
        check_contacts(no, &pin.contacts)?;
    }
    check_connectors(&pins)?;
```

and add to its doc comment, after the `block_size` bullet:

```rust
/// - `connector_ref` が空・まとめない種類に付いている・2つのまとまりに使われている → エラー ([`check_connectors`])
```

Directly above `/// ピンが収まる最小の外形寸法 (mm)。` add:

```rust
/// コネクタの名前 (`connector_ref`) を調べる: 空でない・まとめる種類 (端子台・JST) だけが持つ・
/// 1つの名前は1つのまとまり (1辺の隣り合うピン、同じ種類) だけが使う。
fn check_connectors(pins: &BlockPins) -> Result<(), String> {
    for pin in pins.all() {
        let Some(reference) = &pin.connector_ref else { continue };
        let no = pin.no.trim();
        if reference.trim().is_empty() {
            return Err(format!("block pin {no}: connector_ref must not be empty"));
        }
        match pin.connector {
            None => {
                return Err(format!(
                    "block pin {no}: connector_ref needs a grouped connector (screw, jst_ph_f, jst_xh_f)"
                ))
            }
            Some(kind) if !kind.is_grouped() => {
                return Err(format!(
                    "block pin {no}: connector_ref is only for screw, jst_ph_f and jst_xh_f, not {}",
                    kind.name()
                ))
            }
            Some(_) => {}
        }
    }
    let mut seen: std::collections::BTreeSet<String> = Default::default();
    for side in [&pins.left, &pins.right, &pins.top, &pins.bottom] {
        for group in connector_groups(side) {
            let Some(reference) = group.reference else { continue };
            if !seen.insert(reference.clone()) {
                return Err(format!(
                    "block pin {}: connector_ref {reference} is used by another group; its pins must be next to each other on one side",
                    side[group.start].no.trim()
                ));
            }
        }
    }
    Ok(())
}

```

- [ ] **Step 5: Run to verify GREEN** (one filter per run)

Run each and expect `test result: ok`:
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core connector 2>&1 | grep -E "^test |test result"` — Expected: 9 tests (3 in `connector::tests`, 5 in `block::tests` whose names contain `connector`, 1 in `command::tests`), all `ok`.
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms/src-tauri && cargo test -p madake-core block:: 2>&1 | grep "test result"` — Expected: `ok`, 0 failed (existing block tests unchanged).

- [ ] **Step 6: Whole core suite and spec docs**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core 2>&1 | grep -E "^test result|FAILED|panicked" | head`
Expected: every `test result: ok`, no `FAILED`.
Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check`
Expected: `spec docs up to date; all tests have bilingual spec comments`.

- [ ] **Step 7: Commit**

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms
git add src-tauri/crates/madake-core/src/connector.rs src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-core/src/block.rs src-tauri/crates/madake-core/src/command.rs docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(core): ブロックのピンにコネクタの種類とまとめ名を持たせて検証する"
```

---

### Task 2: Connector kinds and validation in the TS twin

**Files:**
- Create: `src/canvas/connector.ts`
- Modify: `src/canvas/dynamicSymbol.ts` (imports, `BlockPin`, new `checkConnectors`, `parseBlockSpec`)
- Test: `src/canvas/dynamicSymbol.test.ts`

**Interfaces:**
- Consumes: the Rust messages of Task 1 (copied verbatim).
- Produces (`src/canvas/connector.ts`):
  - `export const CONNECTOR_KINDS` (readonly tuple of the 13 names in spec order), `export type ConnectorKind`
  - `export function isConnectorKind(value: unknown): value is ConnectorKind`
  - `export function isGroupedConnector(kind: ConnectorKind): boolean`, `export function connectorSeries(kind: ConnectorKind): string | null`
  - `export interface ConnectorGroup { kind: ConnectorKind; ref: string | null; start: number; end: number }`
  - `export function connectorGroups(side: BlockPin[]): ConnectorGroup[]`, `export function groupLabel(group: ConnectorGroup): string | null`
  - `BlockPin` in `dynamicSymbol.ts` gains `connector?: ConnectorKind; connector_ref?: string`.

- [ ] **Step 1: Write the failing tests** (append at the end of `src/canvas/dynamicSymbol.test.ts`)

```ts
describe("blockSymbol connectors", () => {
  const message = (json: string): string => {
    try {
      blockSymbol({ block_pins: json });
    } catch (e) {
      return (e as Error).message;
    }
    throw new Error(`断らなかった: ${json}`);
  };

  // ja: ピンはコネクタの形を接点と一緒に持て、null は書かなかったのと同じ。知らない種類は Rust と同じく壊れた JSON として断る
  it("reads a connector kind, treats null as absent and refuses an unknown kind as invalid JSON, like Rust", () => {
    expect(() =>
      blockSymbol({ block_pins: '{"top":[{"no":"U1","connector":"usb_a_f","contacts":["VBUS","D-","D+","GND"]}]}' }),
    ).not.toThrow();
    const nulls = blockSymbol({ block_pins: '{"left":[{"no":"1","connector":null,"connector_ref":null}]}' });
    expect(nulls.primitives[0].type).toBe("rect");
    expect(message('{"left":[{"no":"1","connector":"usb_z"}]}')).toMatch(/^block_pins is not valid JSON/);
  });

  // ja: コネクタの名前の誤りは Rust と同じ文で断り (空・コネクタなし・まとめない種類・2つのまとまり)、名前の無いコネクタは繰り返してよい
  it("refuses a bad connector name with the same messages as Rust and lets unnamed connectors repeat", () => {
    expect(message('{"left":[{"no":"P1","connector":"screw","connector_ref":"  "}]}')).toBe(
      "block pin P1: connector_ref must not be empty",
    );
    expect(message('{"left":[{"no":"P1","connector_ref":"J3"}]}')).toBe(
      "block pin P1: connector_ref needs a grouped connector (screw, jst_ph_f, jst_xh_f)",
    );
    expect(message('{"left":[{"no":"P1","connector":"usb_a_f","connector_ref":"J3"}]}')).toBe(
      "block pin P1: connector_ref is only for screw, jst_ph_f and jst_xh_f, not usb_a_f",
    );
    const split = "block pin 2: connector_ref J3 is used by another group; its pins must be next to each other on one side";
    for (const json of [
      '{"left":[{"no":"1","connector":"jst_ph_f","connector_ref":"J3"}],"right":[{"no":"2","connector":"jst_ph_f","connector_ref":"J3"}]}',
      '{"left":[{"no":"1","connector":"jst_ph_f","connector_ref":"J3"},{"no":"X"},{"no":"2","connector":"jst_ph_f","connector_ref":"J3"}]}',
      '{"left":[{"no":"1","connector":"screw","connector_ref":"J3"},{"no":"2","connector":"jst_ph_f","connector_ref":"J3"}]}',
    ]) {
      expect(message(json)).toBe(split);
    }
    expect(() =>
      blockSymbol({
        block_pins:
          '{"left":[{"no":"1","connector":"screw"},{"no":"2"},{"no":"3","connector":"screw"}],"top":[{"no":"4","connector":"jst_xh_f"},{"no":"5","connector":"usb_a_f"},{"no":"6","connector":"usb_a_f"}]}',
      }),
    ).not.toThrow();
  });
});
```

- [ ] **Step 2: Run to verify RED**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && npx vitest run src/canvas/dynamicSymbol.test.ts -t "blockSymbol connectors" 2>&1 | tail -15`
Expected: 2 failed. Both fail with `断らなかった: {"left":[{"no":"1","connector":"usb_z"}]}` / `断らなかった: {"left":[{"no":"P1","connector":"screw","connector_ref":"  "}]}` (TS ignores the new keys today).

- [ ] **Step 3: Create `src/canvas/connector.ts`**

```ts
// ブロックのピンのコネクタの絵 (Rust側 madake-core/src/connector.rs と対。同じ数・同じ順)。
// 両者が同じ形を返すことは src-tauri/crates/madake-core/tests/golden/connector-blocks.json で突き合わせる
// (dynamicSymbol.test.ts)。型だけを dynamicSymbol.ts から読むので、実行時の循環importは無い。

import type { BlockPin } from "./dynamicSymbol";

/** コネクタの形 (仕様の表の順。エディタの選択肢もこの順)。`_f` = メス、`_m` = オス。 */
export const CONNECTOR_KINDS = [
  "usb_a_f",
  "usb_a_m",
  "usb_c_f",
  "usb_c_m",
  "micro_b_f",
  "micro_b_m",
  "jack35_f",
  "jack35_m",
  "dc_f",
  "dc_m",
  "screw",
  "jst_ph_f",
  "jst_xh_f",
] as const;

export type ConnectorKind = (typeof CONNECTOR_KINDS)[number];

/** 知っているコネクタの名前か。 */
export function isConnectorKind(value: unknown): value is ConnectorKind {
  return typeof value === "string" && (CONNECTOR_KINDS as readonly string[]).includes(value);
}

/** 並んだ数本のピンで1つの本体を描く種類 (端子台・JST)。`connector_ref` はこれだけが持てる。 */
export function isGroupedConnector(kind: ConnectorKind): boolean {
  return kind === "screw" || kind === "jst_ph_f" || kind === "jst_xh_f";
}

/** JST のシリーズ名 (`PH` / `XH`)。他はnull。 */
export function connectorSeries(kind: ConnectorKind): string | null {
  if (kind === "jst_ph_f") return "PH";
  if (kind === "jst_xh_f") return "XH";
  return null;
}

/** 1辺の中の、1つの絵で描くピンのまとまり (`start`〜`end` の手前までが辺のピン表の添字)。 */
export interface ConnectorGroup {
  kind: ConnectorKind;
  /** 前後の空白を落とした connector_ref。無ければnull。 */
  ref: string | null;
  start: number;
  end: number;
}

/** 1辺のピンをまとまりに分ける (Rust側 connector_groups と同じ規則)。 */
export function connectorGroups(side: BlockPin[]): ConnectorGroup[] {
  const groups: ConnectorGroup[] = [];
  side.forEach((pin, i) => {
    const kind = pin.connector;
    if (kind == null) return;
    const ref = pin.connector_ref == null ? null : pin.connector_ref.trim();
    const last = groups[groups.length - 1];
    if (isGroupedConnector(kind) && last && last.end === i && last.kind === kind && last.ref === ref) {
      last.end = i + 1;
      return;
    }
    groups.push({ kind, ref, start: i, end: i + 1 });
  });
  return groups;
}

/** まとまりの名前 (JST は `<ref> PH` / `PH`、端子台は `<ref>`、無ければnull)。 */
export function groupLabel(group: ConnectorGroup): string | null {
  const series = connectorSeries(group.kind);
  if (series !== null) return group.ref !== null ? `${group.ref} ${series}` : series;
  if (group.ref !== null && isGroupedConnector(group.kind)) return group.ref;
  return null;
}
```

- [ ] **Step 4: Wire it into `dynamicSymbol.ts`**

After `import type { Point, Primitive, PinDef, SymbolDef, SymbolInstance, TextSlot } from "../ipc";` add:

```ts
import { connectorGroups, isConnectorKind, isGroupedConnector, type ConnectorKind } from "./connector";
```

In `export interface BlockPin`, replace

```ts
  /** ポートの接点 (Rust側 BlockPin::contacts)。 */
  contacts?: string[];
}
```

with

```ts
  /** ポートの接点 (Rust側 BlockPin::contacts)。 */
  contacts?: string[];
  /** 外形の辺に描くコネクタの形 (Rust側 BlockPin::connector)。無ければ普通のピン。 */
  connector?: ConnectorKind;
  /** まとめて1つの本体に描くコネクタ (端子台・JST) の名前 (Rust側 BlockPin::connector_ref)。 */
  connector_ref?: string;
}
```

Directly above `export function parseBlockSpec(attrs: Record<string, string>): {` add:

```ts
/** コネクタの名前を調べる (Rust側 check_connectors と同じ順・同じ文)。 */
function checkConnectors(pins: BlockPins): void {
  for (const side of BLOCK_SIDES) {
    for (const pin of sideOf(pins, side)) {
      if (pin.connector_ref == null) continue;
      const no = (pin.no ?? "").trim();
      if (pin.connector_ref.trim() === "") throw new Error(`block pin ${no}: connector_ref must not be empty`);
      if (pin.connector == null) {
        throw new Error(`block pin ${no}: connector_ref needs a grouped connector (screw, jst_ph_f, jst_xh_f)`);
      }
      if (!isGroupedConnector(pin.connector)) {
        throw new Error(`block pin ${no}: connector_ref is only for screw, jst_ph_f and jst_xh_f, not ${pin.connector}`);
      }
    }
  }
  const seen = new Set<string>();
  for (const side of BLOCK_SIDES) {
    const list = sideOf(pins, side);
    for (const group of connectorGroups(list)) {
      if (group.ref === null) continue;
      if (seen.has(group.ref)) {
        throw new Error(
          `block pin ${(list[group.start].no ?? "").trim()}: connector_ref ${group.ref} is used by another group; its pins must be next to each other on one side`,
        );
      }
      seen.add(group.ref);
    }
  }
}

```

In `parseBlockSpec`, replace

```ts
    pins = parsed as BlockPins;
  }
```

with

```ts
    pins = parsed as BlockPins;
    // Rust では serde が読むときに断る (知らない種類・文字でない名前)
    for (const pin of BLOCK_SIDES.flatMap((s) => sideOf(pins, s))) {
      if (pin.connector != null && !isConnectorKind(pin.connector)) {
        throw new Error(`${BLOCK_PINS_ATTR} is not valid JSON: unknown connector ${String(pin.connector)}`);
      }
      if (pin.connector_ref != null && typeof pin.connector_ref !== "string") {
        throw new Error(`${BLOCK_PINS_ATTR} is not valid JSON: connector_ref must be a string`);
      }
    }
  }
```

and replace

```ts
    checkContacts(no, pin.contacts ?? []);
  }
```

with

```ts
    checkContacts(no, pin.contacts ?? []);
  }
  checkConnectors(pins);
```

- [ ] **Step 5: Run to verify GREEN**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && npx vitest run src/canvas/dynamicSymbol.test.ts src/canvas/blockEditor.test.ts src/stores/blockEditor.test.ts 2>&1 | tail -5`
Expected: all passed (the 2 new tests included).
Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && npx vue-tsc --noEmit 2>&1 | grep -v "src/chat/markdown" | head`
Expected: no output (only the pre-existing `src/chat/markdown.ts` errors are filtered out).

- [ ] **Step 6: Spec docs and commit**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check` — Expected: `spec docs up to date; all tests have bilingual spec comments`.

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms
git add src/canvas/connector.ts src/canvas/dynamicSymbol.ts src/canvas/dynamicSymbol.test.ts docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(ui): ピン表のコネクタの種類とまとめ名を TS でも読んで同じ文で断る"
```

---

### Task 3: Shared golden file (the twin backbone)

**Files:**
- Modify: `src-tauri/crates/madake-core/src/block.rs` (test module: fixtures, golden helpers, golden test)
- Create (generated): `src-tauri/crates/madake-core/tests/golden/connector-blocks.json`
- Modify: `src/canvas/dynamicSymbol.test.ts` (import + golden test)

**Interfaces:**
- Consumes: `ConnectorKind::ALL`, `name()`, `is_grouped()` (Task 1); TS `blockSymbol` (Task 2).
- Produces: test fixtures in `block::tests`: `const HUB_39: &str`, `const AMP_53: &str`, `const MIXED: &str`, `const TOP_BOTTOM: &str`; `fn golden_cases() -> Vec<(String, String)>`; `fn rounded(serde_json::Value) -> serde_json::Value`; the golden file `{"cases":[{"name","attrs":{"block_pins"},"primitives","pins"}, …]}`, one case per line. Regenerate with `MADAKE_UPDATE_GOLDEN=1`.

At this point connector pins still draw as plain pins in both languages, so the golden file records today's drawing for every case. Tasks 5 and 6 regenerate it.

- [ ] **Step 1: Write the Rust golden test** (append inside `mod tests` of `block.rs`)

```rust
    /// AOGU のハブ 39: 上辺に USB-A メス4つ、下辺に micro-B メスの電源、右辺に OTG ケーブルの micro-B オス。
    const HUB_39: &str = r#"{"top":[{"no":"U1","connector":"usb_a_f","contacts":["VBUS","D-","D+","GND"]},{"no":"U2","connector":"usb_a_f","contacts":["VBUS","D-","D+","GND"]},{"no":"U3","connector":"usb_a_f","contacts":["VBUS","D-","D+","GND"]},{"no":"U4","connector":"usb_a_f","contacts":["VBUS","D-","D+","GND"]}],"bottom":[{"no":"PWR","connector":"micro_b_f","contacts":["VBUS","GND"]}],"right":[{"no":"OTG","connector":"micro_b_m","contacts":["VBUS","D-","D+","GND"]}]}"#;
    /// AOGU のアンプ 53: 3.5mm ジャックの入力と、5V・GND の端子台 TB1、スピーカー出力の端子台 TB2。
    const AMP_53: &str = r#"{"left":[{"no":"IN","connector":"jack35_f","contacts":["L","R","GND"]},{"no":"4","name":"5V","connector":"screw","connector_ref":"TB1"},{"no":"5","name":"GND","connector":"screw","connector_ref":"TB1"}],"right":[{"no":"6","name":"SP+","connector":"screw","connector_ref":"TB2"},{"no":"7","name":"SP-","connector":"screw","connector_ref":"TB2"}]}"#;
    /// 左右の辺に全種類と普通のピン (番号は1〜4文字、名前あり・なし)、1本と3本の JST、端子台を並べたブロック。
    const MIXED: &str = r#"{"left":[{"no":"1","name":"VIN"},{"no":"22","name":"USB","connector":"usb_a_f"},{"no":"333","name":"LTE","connector":"usb_a_m"},{"no":"4444","name":"C","connector":"usb_c_f"},{"no":"5","connector":"usb_c_m"},{"no":"6","name":"MIC","connector":"micro_b_f"},{"no":"7","name":"OTG","connector":"micro_b_m"},{"no":"P8","name":"EN"},{"no":"9","name":"SIG","connector":"jst_ph_f","connector_ref":"J9"},{"no":"10","name":"A","connector":"screw","connector_ref":"TB1"},{"no":"11","name":"B","connector":"screw","connector_ref":"TB1"}],"right":[{"no":"20","name":"LINE","connector":"jack35_f"},{"no":"21","connector":"jack35_m"},{"no":"230","name":"DC","connector":"dc_f"},{"no":"24","name":"12V","connector":"dc_m"},{"no":"V+","name":"VCC"},{"no":"31","name":"5V","connector":"jst_xh_f","connector_ref":"J3"},{"no":"32","name":"SDA","connector":"jst_xh_f","connector_ref":"J3"},{"no":"33","name":"GND","connector":"jst_xh_f","connector_ref":"J3"},{"no":"34","name":"L","connector":"screw"},{"no":"35","name":"R","connector":"screw"}]}"#;
    /// 上辺に名前付きの JST と普通のピンと USB-C、下辺に名前付きの端子台と DC ジャックを並べたブロック。
    const TOP_BOTTOM: &str = r#"{"top":[{"no":"1","name":"A","connector":"jst_ph_f","connector_ref":"J5"},{"no":"2","name":"B","connector":"jst_ph_f","connector_ref":"J5"},{"no":"3","name":"C"},{"no":"4","connector":"usb_c_f"}],"bottom":[{"no":"5","connector":"screw","connector_ref":"TB3"},{"no":"6","connector":"screw","connector_ref":"TB3"},{"no":"7","name":"D","connector":"dc_f"}]}"#;

    /// Rust と TS が同じ形を返すことを確かめる共有ファイル (`src/canvas/dynamicSymbol.test.ts` も読む)。
    const CONNECTOR_GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/connector-blocks.json");

    /// golden の例: 13種類×4辺 (1本、まとめる種類は名前付きの2本) と、AOGU の機器・重なりの例・コネクタの無いブロック。
    fn golden_cases() -> Vec<(String, String)> {
        let mut cases = Vec::new();
        for kind in ConnectorKind::ALL {
            let k = kind.name();
            for side in ["left", "right", "top", "bottom"] {
                let json = if kind.is_grouped() {
                    format!(r#"{{"{side}":[{{"no":"1","name":"A","connector":"{k}","connector_ref":"J1"}},{{"no":"2","name":"B","connector":"{k}","connector_ref":"J1"}}]}}"#)
                } else {
                    format!(r#"{{"{side}":[{{"no":"1","name":"A","connector":"{k}"}}]}}"#)
                };
                cases.push((format!("{k}-{side}"), json));
            }
        }
        for (name, json) in [("aogu-hub-39", HUB_39), ("aogu-amp-53", AMP_53), ("mixed-left-right", MIXED), ("top-bottom", TOP_BOTTOM), ("plain", SAMPLE)] {
            cases.push((name.to_string(), json.to_string()));
        }
        cases
    }

    /// 数を 1e-6 に丸め、-0 を 0 にそろえる (TS 側も同じ丸めで比べる)。
    fn rounded(v: serde_json::Value) -> serde_json::Value {
        match v {
            serde_json::Value::Number(n) if n.is_f64() => {
                let r = (n.as_f64().unwrap() * 1e6).round() / 1e6;
                serde_json::json!(if r == 0.0 { 0.0 } else { r })
            }
            serde_json::Value::Array(a) => serde_json::Value::Array(a.into_iter().map(rounded).collect()),
            serde_json::Value::Object(o) => serde_json::Value::Object(o.into_iter().map(|(k, v)| (k, rounded(v))).collect()),
            other => other,
        }
    }

    fn golden_json() -> serde_json::Value {
        let cases: Vec<serde_json::Value> = golden_cases()
            .into_iter()
            .map(|(name, pins)| {
                let def = block_symbol(&attrs(&[("block_pins", &pins)])).unwrap_or_else(|e| panic!("{name}: {e}"));
                serde_json::json!({
                    "name": name,
                    "attrs": { "block_pins": pins },
                    "primitives": rounded(serde_json::to_value(&def.primitives).unwrap()),
                    "pins": rounded(serde_json::to_value(&def.pins).unwrap()),
                })
            })
            .collect();
        serde_json::json!({ "cases": cases })
    }

    /// Every connector kind on every side, and the AOGU hub and amplifier, are drawn exactly as recorded in the golden file that the TypeScript twin is also checked against (rewrite it with MADAKE_UPDATE_GOLDEN=1).
    /// 全種類のコネクタを全辺に置いた例と AOGU のハブ・アンプは、TS 側も突き合わせる golden ファイルのとおりに描かれる (MADAKE_UPDATE_GOLDEN=1 で書き直す)。
    #[test]
    fn connector_blocks_match_the_shared_golden_file() {
        let want = golden_json();
        if std::env::var_os("MADAKE_UPDATE_GOLDEN").is_some() {
            // 1行に1例 (差分が例ごとに読める)
            let lines: Vec<String> = want["cases"].as_array().unwrap().iter().map(|c| serde_json::to_string(c).unwrap()).collect();
            std::fs::write(CONNECTOR_GOLDEN, format!("{{\"cases\":[\n{}\n]}}\n", lines.join(",\n"))).expect("golden を書く");
        }
        let text = std::fs::read_to_string(CONNECTOR_GOLDEN)
            .unwrap_or_else(|_| panic!("{CONNECTOR_GOLDEN} が無い: MADAKE_UPDATE_GOLDEN=1 を付けて作る"));
        let got: serde_json::Value = serde_json::from_str(&text).expect("golden の JSON");
        assert!(got == want, "golden と違う: MADAKE_UPDATE_GOLDEN=1 で書き直し、git diff で変化を確かめる");
    }
```

- [ ] **Step 2: Run to verify RED**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core connector_blocks_match_the_shared_golden_file 2>&1 | grep -E "panicked|test result" -A1 | head -4`
Expected: FAIL, panic message ending in `connector-blocks.json が無い: MADAKE_UPDATE_GOLDEN=1 を付けて作る`.

- [ ] **Step 3: Generate the file and verify GREEN**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms/src-tauri && MADAKE_UPDATE_GOLDEN=1 cargo test -p madake-core connector_blocks_match_the_shared_golden_file 2>&1 | grep "test result"`
Expected: `test result: ok. 1 passed`.
Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms/src-tauri && cargo test -p madake-core connector_blocks_match_the_shared_golden_file 2>&1 | grep "test result"`
Expected: `test result: ok. 1 passed` without the variable.
Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && head -c 300 src-tauri/crates/madake-core/tests/golden/connector-blocks.json`
Expected: starts with `{"cases":[`, the next line begins `{"attrs":{"block_pins":"{\"left\":[{\"no\":\"1\",\"name\":\"A\",\"connector\":\"usb_a_f\"}]}"},"name":"usb_a_f-left"`. Count the cases with `grep -c '^{"attrs"' src-tauri/crates/madake-core/tests/golden/connector-blocks.json` → `57` (52 kind × side cases + 5 named cases).

- [ ] **Step 4: Write the TS golden test**

In `src/canvas/dynamicSymbol.test.ts`, after the existing `} from "./dynamicSymbol";` import line add:

```ts
import golden from "../../src-tauri/crates/madake-core/tests/golden/connector-blocks.json";
```

Append at the end of the file:

```ts
/** 数を 1e-6 に丸め、-0 を 0 にそろえる (Rust側 block.rs の rounded と同じ)。 */
function rounded(v: unknown): unknown {
  if (typeof v === "number") {
    const r = (Math.sign(v) * Math.round(Math.abs(v) * 1e6)) / 1e6;
    return r === 0 ? 0 : r;
  }
  if (Array.isArray(v)) return v.map(rounded);
  if (v !== null && typeof v === "object") {
    return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, rounded(x)]));
  }
  return v;
}

describe("connector golden (Rust twin)", () => {
  // ja: golden ファイルの全例 (全種類×全辺・AOGU の機器・コネクタの無いブロック) で、TS は Rust と同じプリミティブとピンを返す
  it("draws every block of the shared golden file exactly like Rust", () => {
    expect(golden.cases.length).toBe(57);
    for (const c of golden.cases) {
      const def = blockSymbol(c.attrs);
      expect({ name: c.name, primitives: rounded(def.primitives), pins: rounded(def.pins) }).toEqual({
        name: c.name,
        primitives: c.primitives,
        pins: c.pins,
      });
    }
  });
});
```

- [ ] **Step 5: Run the TS golden test**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && npx vitest run src/canvas/dynamicSymbol.test.ts -t "connector golden" 2>&1 | tail -5`
Expected: `1 passed`. (Before Step 3 created the file this import failed to resolve: that was the RED of this test.)
Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && npx vue-tsc --noEmit 2>&1 | grep -v "src/chat/markdown" | head` — Expected: no output.

- [ ] **Step 6: Spec docs and commit**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check` — Expected: `spec docs up to date; all tests have bilingual spec comments`.

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms
git add src-tauri/crates/madake-core/src/block.rs src-tauri/crates/madake-core/tests/golden/connector-blocks.json src/canvas/dynamicSymbol.test.ts docs/13-specification.md docs/13-specification.ja.md
git commit -m "test: コネクタのブロックを Rust と TS で突き合わせる golden を足す"
```

---

### Task 4: Pictures in local edge frames (Rust)

**Files:**
- Modify: `src-tauri/crates/madake-core/src/connector.rs` (imports, constants, `Frame`, `frame`, `connector_primitives`, test helper `primitive_extent`, tests)

**Interfaces:**
- Consumes: `ConnectorKind` (Task 1); `crate::netlist::rotate_arc_angles(start_deg, end_deg, rotation: u16, mirror: bool) -> (f64, f64)` and `crate::netlist::rotate_local(p, rotation, mirror) -> Point` (existing).
- Produces:
  - `pub const CONNECTOR_IN_MM: f64 = 1.5; CONNECTOR_OUT_MM = 2.5; CONNECTOR_HALF_SPAN_MM = 2.0; CONNECTOR_LEAD_MM = 5.0; CONNECTOR_NUM_GAP_MM = 0.25; CONNECTOR_NUM_SIDE_MM = 0.5; CONNECTOR_LABEL_GAP_MM = 1.0; CONNECTOR_LABEL_TOP_MM = 7.0; CONNECTOR_LABEL_INSET_MM = 10.0;`
  - `pub fn connector_primitives(kind: ConnectorKind, dir: PinDir, edges: &[Point]) -> Vec<Primitive>`
  - `#[cfg(test)] pub(crate) fn primitive_extent(p: &Primitive) -> (Point, Point)`

- [ ] **Step 1: Write the failing tests** (append inside `mod tests` of `connector.rs`)

```rust
    /// 数を短い文字にする (-0 は 0 にそろえる)。
    fn n(v: f64) -> String {
        format!("{}", v + 0.0)
    }

    /// プリミティブを1行の文字にする (期待値を手で書けるように)。
    fn show(p: &Primitive) -> String {
        match p {
            Primitive::Line { pts } => {
                let pts: Vec<String> = pts.iter().map(|q| format!("{},{}", n(q.x), n(q.y))).collect();
                format!("line {}", pts.join(" "))
            }
            Primitive::Rect { p1, p2, filled } => {
                format!("rect {},{} {},{}{}", n(p1.x), n(p1.y), n(p2.x), n(p2.y), if *filled { " filled" } else { "" })
            }
            Primitive::Circle { center, r, filled } => {
                format!("circle {},{} r{}{}", n(center.x), n(center.y), n(*r), if *filled { " filled" } else { "" })
            }
            Primitive::Arc { center, r, start_deg, end_deg } => {
                format!("arc {},{} r{} {}..{}", n(center.x), n(center.y), n(*r), n(*start_deg), n(*end_deg))
            }
            Primitive::Text { at, text, .. } => format!("text {text} {},{}", n(at.x), n(at.y)),
        }
    }

    /// 左辺の、E = (0, 0) の1本 (まとめる種類は E と (0, 5) の2本) の絵。
    fn left_picture(kind: ConnectorKind) -> Vec<String> {
        let edges = if kind.is_grouped() {
            vec![Point::new(0.0, 0.0), Point::new(0.0, 5.0)]
        } else {
            vec![Point::new(0.0, 0.0)]
        };
        connector_primitives(kind, PinDir::Left, &edges).iter().map(show).collect()
    }

    /// On the left side every connector picture is exactly the drawing of the spec: sockets are an open body across the edge, plugs a housing inside with a black insert outside, and screw and JST bodies span their pins.
    /// 左辺では、どのコネクタの絵も仕様の図そのものになる: メスは辺にまたがる白い口、オスは内側の枠と外へ出る黒い差し込み、端子台と JST の本体はピンをまたいで1つ。
    #[test]
    fn every_picture_matches_the_spec_on_the_left_side() {
        use ConnectorKind::*;
        let open = "rect -2.5,-2 1.5,2";
        let housing = "rect 0,-2 1.5,2";
        assert_eq!(left_picture(UsbAF), [open, "rect -1.8,0.1 0.8,0.9 filled"]);
        assert_eq!(left_picture(UsbAM), [housing, "rect -2.5,-1.2 0,1.2 filled"]);
        assert_eq!(
            left_picture(UsbCF),
            [open, "line 0.2,0.8 -1.2,0.8", "arc -1.2,0 r0.8 90..270", "line -1.2,-0.8 0.2,-0.8", "arc 0.2,0 r0.8 270..450", "line 0,0 -1,0"]
        );
        assert_eq!(left_picture(UsbCM), [housing, "rect -1.7,-0.8 0,0.8 filled", "circle -1.7,0 r0.8 filled"]);
        assert_eq!(
            left_picture(MicroBF),
            ["line 1.5,2 0,2 -2.5,1.4 -2.5,-1.4 0,-2 1.5,-2 1.5,2", "line 0.6,0 -1.6,0"]
        );
        assert_eq!(left_picture(MicroBM), [housing, "rect -2.5,-0.6 0,0.6 filled"]);
        assert_eq!(left_picture(Jack35F), [open, "circle -0.5,0 r1", "circle -0.5,0 r0.35 filled"]);
        assert_eq!(left_picture(Jack35M), [housing, "rect -0.8,-1 0,1 filled", "rect -2.5,-0.4 -0.8,0.4 filled"]);
        assert_eq!(left_picture(DcF), [open, "circle -0.5,0 r1.5", "circle -0.5,0 r0.6 filled"]);
        assert_eq!(left_picture(DcM), [housing, "rect -2.5,0.5 0,1.4 filled", "rect -2.5,-1.4 0,-0.5 filled"]);
        assert_eq!(
            left_picture(Screw),
            ["rect -2.5,-2 1.5,7", "circle -0.5,0 r1.2", "line -1.3,0.8 0.3,-0.8", "circle -0.5,5 r1.2", "line -1.3,5.8 0.3,4.2"]
        );
        let jst = ["rect -2.5,-2 1.5,7", "line -1.5,7 -1.5,-2", "rect -0.6,-0.6 0.6,0.6", "rect -0.6,4.4 0.6,5.6"];
        assert_eq!(left_picture(JstPhF), jst);
        assert_eq!(left_picture(JstXhF), jst, "PH と XH は名前だけが違う");
    }

    /// The picture frame of each side is the right-side frame turned clockwise: a local point lands where the side faces, and an arc keeps its clockwise sweep.
    /// 辺ごとの絵の座標は右辺の座標を時計回りに回したもので、ローカルの点はその辺の向きに写り、弧は時計回りの振れ幅を保つ。
    #[test]
    fn side_frames_are_turns_of_the_right_side_frame() {
        let e = Point::new(10.0, 20.0);
        let cases = [
            (PinDir::Right, (11.0, 20.5), (270.0, 450.0)),
            (PinDir::Down, (9.5, 21.0), (0.0, 180.0)),
            (PinDir::Left, (9.0, 19.5), (90.0, 270.0)),
            (PinDir::Up, (10.5, 19.0), (180.0, 360.0)),
        ];
        for (dir, point, angles) in cases {
            let f = frame(dir, e);
            let at = f.at(1.0, 0.5);
            assert_eq!((at.x, at.y), point, "{dir:?}");
            let Primitive::Arc { start_deg, end_deg, .. } = f.arc((0.0, 0.0), 1.0, -90.0, 90.0) else { unreachable!() };
            assert_eq!((start_deg, end_deg), angles, "{dir:?}");
        }
    }

    /// No picture ever leaves its envelope (1.5 mm inside to 2.5 mm outside the edge, 2 mm beyond its outer pins along the edge), on any side.
    /// どの辺でも、絵は決まった枠 (辺の内側1.5mmから外側2.5mm、辺に沿って両端のピンから2mm) の外へ出ない。
    #[test]
    fn every_picture_stays_inside_its_envelope() {
        for dir in [PinDir::Left, PinDir::Right, PinDir::Up, PinDir::Down] {
            for kind in ConnectorKind::ALL {
                let e = Point::new(30.0, 40.0);
                let f = frame(dir, e);
                let edges = if kind.is_grouped() { vec![e, f.at(0.0, 5.0)] } else { vec![e] };
                let v1 = if kind.is_grouped() { 5.0 } else { 0.0 };
                let (a, b) = (f.at(-CONNECTOR_IN_MM, -2.0), f.at(CONNECTOR_OUT_MM, v1 + 2.0));
                let (lo, hi) = (Point::new(a.x.min(b.x), a.y.min(b.y)), Point::new(a.x.max(b.x), a.y.max(b.y)));
                for prim in connector_primitives(kind, dir, &edges) {
                    let (a, b) = primitive_extent(&prim);
                    let inside = a.x >= lo.x - 1e-9 && a.y >= lo.y - 1e-9 && b.x <= hi.x + 1e-9 && b.y <= hi.y + 1e-9;
                    assert!(inside, "{} {dir:?}: {} が枠の外", kind.name(), show(&prim));
                }
            }
        }
    }

    /// A USB-C socket's opening stays a closed rounded slot on every side and at every rotation and mirror of the block: each arc ends where a straight edge of the opening ends and bulges outward.
    /// USB-C メスの口は、どの辺でも、ブロックをどう回転・ミラーしても閉じた角丸の穴のまま: 弧の端は口の直線の端に合い、弧は外へふくらむ。
    #[test]
    fn the_usb_c_opening_stays_closed_at_every_rotation_and_mirror() {
        use crate::netlist::{rotate_arc_angles, rotate_local};
        for dir in [PinDir::Left, PinDir::Right, PinDir::Up, PinDir::Down] {
            let e = Point::new(7.5, 2.5);
            let prims = connector_primitives(ConnectorKind::UsbCF, dir, &[e]);
            let slot_centre = frame(dir, e).at(0.5, 0.0);
            for rotation in [0u16, 90, 180, 270] {
                for mirror in [false, true] {
                    let line_ends: Vec<Point> = prims
                        .iter()
                        .filter_map(|p| match p {
                            Primitive::Line { pts } => Some(pts.clone()),
                            _ => None,
                        })
                        .flatten()
                        .map(|q| rotate_local(q, rotation, mirror))
                        .collect();
                    let centre = rotate_local(slot_centre, rotation, mirror);
                    for p in &prims {
                        let Primitive::Arc { center, r, start_deg, end_deg } = p else { continue };
                        let c = rotate_local(*center, rotation, mirror);
                        let (a0, a1) = rotate_arc_angles(*start_deg, *end_deg, rotation, mirror);
                        let on_arc = |a: f64| Point::new(c.x + r * a.to_radians().cos(), c.y + r * a.to_radians().sin());
                        for end in [on_arc(a0), on_arc(a1)] {
                            assert!(
                                line_ends.iter().any(|q| (q.x - end.x).abs() < 1e-9 && (q.y - end.y).abs() < 1e-9),
                                "{dir:?} {rotation} {mirror}: 弧の端 {end:?} が口の直線の端に合わない"
                            );
                        }
                        let mid = on_arc((a0 + a1) / 2.0);
                        let bulge = ((mid.x - centre.x).powi(2) + (mid.y - centre.y).powi(2)).sqrt();
                        assert!(bulge > 1.0, "{dir:?} {rotation} {mirror}: 弧が内側へふくらむ ({bulge})");
                    }
                }
            }
        }
    }
```

- [ ] **Step 2: Run to verify RED**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core connector:: 2>&1 | grep -E "^error" | head`
Expected: compile errors: cannot find `Primitive`, `Point`, `PinDir`, `connector_primitives`, `frame`, `primitive_extent`, `CONNECTOR_IN_MM`.

- [ ] **Step 3: Implement in `connector.rs`**

Replace the imports

```rust
use serde::Deserialize;

use crate::block::BlockPin;
```

with

```rust
use serde::Deserialize;

use crate::block::BlockPin;
use crate::geometry::Point;
use crate::symbol::{PinDir, Primitive};

/// 絵の本体が外形の内側に入る深さ (mm)。
pub const CONNECTOR_IN_MM: f64 = 1.5;
/// 絵の本体が外形の外側へ出る長さ (mm)。
pub const CONNECTOR_OUT_MM: f64 = 2.5;
/// 本体の辺に沿う長さの半分 (mm)。本体は4mm。
pub const CONNECTOR_HALF_SPAN_MM: f64 = 2.0;
/// コネクタのあるピンの、外形から接続点までの長さ (mm)。
pub const CONNECTOR_LEAD_MM: f64 = 5.0;
/// 本体の外端からピン番号までのすき間 (mm)。
pub const CONNECTOR_NUM_GAP_MM: f64 = 0.25;
/// 上下辺のピン番号を、リードからこれだけ右へずらして書く (mm)。
pub const CONNECTOR_NUM_SIDE_MM: f64 = 0.5;
/// 1本だけのまとまりの名前を、ピン名の後ろにこれだけ空けて書く (mm)。
pub const CONNECTOR_LABEL_GAP_MM: f64 = 1.0;
/// 上下辺のまとまりの名前の中心の、外形からの深さ (mm)。
pub const CONNECTOR_LABEL_TOP_MM: f64 = 7.0;
/// まとまりの名前を書く上下辺が外形の内側で使う高さ (mm)。
pub const CONNECTOR_LABEL_INSET_MM: f64 = 10.0;
```

Directly above `#[cfg(test)]\nmod tests {` add:

```rust
/// 辺ごとのローカル座標 (右辺の座標を時計回りに回したもの)。
struct Frame {
    e: Point,
    u: Point,
    v: Point,
    theta: u16,
}

fn frame(dir: PinDir, e: Point) -> Frame {
    let (u, v, theta) = match dir {
        PinDir::Right => ((1.0, 0.0), (0.0, 1.0), 0),
        PinDir::Down => ((0.0, 1.0), (-1.0, 0.0), 90),
        PinDir::Left => ((-1.0, 0.0), (0.0, -1.0), 180),
        PinDir::Up => ((0.0, -1.0), (1.0, 0.0), 270),
    };
    Frame { e, u: Point::new(u.0, u.1), v: Point::new(v.0, v.1), theta }
}

impl Frame {
    fn at(&self, u: f64, v: f64) -> Point {
        Point::new(
            self.e.x + u * self.u.x + v * self.v.x,
            self.e.y + u * self.u.y + v * self.v.y,
        )
    }

    fn line(&self, pts: &[(f64, f64)]) -> Primitive {
        Primitive::Line { pts: pts.iter().map(|&(u, v)| self.at(u, v)).collect() }
    }

    /// 2隅を写してから (小さい方, 大きい方) に並べ直す。
    fn rect(&self, (u1, v1): (f64, f64), (u2, v2): (f64, f64), filled: bool) -> Primitive {
        let (a, b) = (self.at(u1, v1), self.at(u2, v2));
        Primitive::Rect {
            p1: Point::new(a.x.min(b.x), a.y.min(b.y)),
            p2: Point::new(a.x.max(b.x), a.y.max(b.y)),
            filled,
        }
    }

    fn circle(&self, (u, v): (f64, f64), r: f64, filled: bool) -> Primitive {
        Primitive::Circle { center: self.at(u, v), r, filled }
    }

    /// 角度は辺の回転ぶん足す (開始は0〜360へ、振れ幅はそのまま。時計回りは時計回りのまま)。
    fn arc(&self, (u, v): (f64, f64), r: f64, start_deg: f64, end_deg: f64) -> Primitive {
        let (start_deg, end_deg) =
            crate::netlist::rotate_arc_angles(start_deg, end_deg, self.theta, false);
        Primitive::Arc { center: self.at(u, v), r, start_deg, end_deg }
    }
}

/// 1つのまとまり (または1本のピン) の絵。`edges` はそのピンたちが外形と交わる点 (辺の並び順)。
/// リード・番号・名前は含まない。
pub fn connector_primitives(kind: ConnectorKind, dir: PinDir, edges: &[Point]) -> Vec<Primitive> {
    let f = frame(dir, edges[0]);
    let vs: Vec<f64> = edges
        .iter()
        .map(|p| (p.x - f.e.x) * f.v.x + (p.y - f.e.y) * f.v.y)
        .collect();
    let v0 = vs.iter().copied().fold(f64::INFINITY, f64::min);
    let v1 = vs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let (i, o, s) = (-CONNECTOR_IN_MM, CONNECTOR_OUT_MM, CONNECTOR_HALF_SPAN_MM);
    // メス = 辺にまたがる白い口、オス = 内側の枠と外へ出る黒い差し込み
    let female = || f.rect((i, -s), (o, s), false);
    let housing = || f.rect((i, -s), (0.0, s), false);
    match kind {
        ConnectorKind::UsbAF => vec![female(), f.rect((-0.8, -0.9), (1.8, -0.1), true)],
        ConnectorKind::UsbAM => vec![housing(), f.rect((0.0, -1.2), (o, 1.2), true)],
        ConnectorKind::UsbCF => vec![
            female(),
            f.line(&[(-0.2, -0.8), (1.2, -0.8)]),
            f.arc((1.2, 0.0), 0.8, -90.0, 90.0),
            f.line(&[(1.2, 0.8), (-0.2, 0.8)]),
            f.arc((-0.2, 0.0), 0.8, 90.0, 270.0),
            f.line(&[(0.0, 0.0), (1.0, 0.0)]),
        ],
        ConnectorKind::UsbCM => vec![
            housing(),
            f.rect((0.0, -0.8), (1.7, 0.8), true),
            f.circle((1.7, 0.0), 0.8, true),
        ],
        ConnectorKind::MicroBF => vec![
            f.line(&[(i, -s), (0.0, -s), (o, -1.4), (o, 1.4), (0.0, s), (i, s), (i, -s)]),
            f.line(&[(-0.6, 0.0), (1.6, 0.0)]),
        ],
        ConnectorKind::MicroBM => vec![housing(), f.rect((0.0, -0.6), (o, 0.6), true)],
        ConnectorKind::Jack35F => vec![
            female(),
            f.circle((0.5, 0.0), 1.0, false),
            f.circle((0.5, 0.0), 0.35, true),
        ],
        ConnectorKind::Jack35M => vec![
            housing(),
            f.rect((0.0, -1.0), (0.8, 1.0), true),
            f.rect((0.8, -0.4), (o, 0.4), true),
        ],
        ConnectorKind::DcF => vec![
            female(),
            f.circle((0.5, 0.0), 1.5, false),
            f.circle((0.5, 0.0), 0.6, true),
        ],
        ConnectorKind::DcM => vec![
            housing(),
            f.rect((0.0, -1.4), (o, -0.5), true),
            f.rect((0.0, 0.5), (o, 1.4), true),
        ],
        ConnectorKind::Screw => {
            let mut out = vec![f.rect((i, v0 - s), (o, v1 + s), false)];
            for &v in &vs {
                out.push(f.circle((0.5, v), 1.2, false));
                out.push(f.line(&[(1.3, v - 0.8), (-0.3, v + 0.8)]));
            }
            out
        }
        ConnectorKind::JstPhF | ConnectorKind::JstXhF => {
            let mut out = vec![
                f.rect((i, v0 - s), (o, v1 + s), false),
                f.line(&[(1.5, v0 - s), (1.5, v1 + s)]),
            ];
            for &v in &vs {
                out.push(f.rect((-0.6, v - 0.6), (0.6, v + 0.6), false));
            }
            out
        }
    }
}

/// テスト用: プリミティブが占める範囲 (最小, 最大)。円・弧は中心±半径。
#[cfg(test)]
pub(crate) fn primitive_extent(p: &Primitive) -> (Point, Point) {
    let pts = match p {
        Primitive::Line { pts } => pts.clone(),
        Primitive::Rect { p1, p2, .. } => vec![*p1, *p2],
        Primitive::Circle { center, r, .. } | Primitive::Arc { center, r, .. } => vec![
            Point::new(center.x - r, center.y - r),
            Point::new(center.x + r, center.y + r),
        ],
        Primitive::Text { at, .. } => vec![*at],
    };
    let lo = pts.iter().fold(Point::new(f64::INFINITY, f64::INFINITY), |a, p| Point::new(a.x.min(p.x), a.y.min(p.y)));
    let hi = pts.iter().fold(Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY), |a, p| Point::new(a.x.max(p.x), a.y.max(p.y)));
    (lo, hi)
}

```

- [ ] **Step 4: Run to verify GREEN** (one filter per run)

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core connector:: 2>&1 | grep -E "^test |test result|warning"`
Expected: 7 tests `ok` (3 from Task 1, 4 new), no warnings.
Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms/src-tauri && cargo test -p madake-core connector_blocks_match_the_shared_golden_file 2>&1 | grep "test result"` — Expected: `ok` (the block drawing is unchanged so far).

- [ ] **Step 5: Spec docs and commit**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check` — Expected: `spec docs up to date; all tests have bilingual spec comments`.

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms
git add src-tauri/crates/madake-core/src/connector.rs docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(core): コネクタの絵を辺ごとの座標で作る"
```

---

### Task 5: Pictures, 5 mm leads and outline gaps on the block (Rust + TS)

**Files:**
- Modify: `src-tauri/crates/madake-core/src/connector.rs` (imports, `outline_segments`, `edge_segments`)
- Modify: `src-tauri/crates/madake-core/src/block.rs` (imports, `offset` visibility, `side_pad`, `block_box`, `edge_point`, `PinLayout`, `pin_layout`, `block_symbol_from_spec`, tests)
- Regenerate: `src-tauri/crates/madake-core/tests/golden/connector-blocks.json`
- Modify: `src/canvas/connector.ts` (frames, `connectorPrimitives`, `sideOffset`, `outlineSegments`, constants)
- Modify: `src/canvas/dynamicSymbol.ts` (imports, local `offset`, `sidePad`, `blockBox`, `edgePoint`, `pinLayout`, `blockSymbol`)

**Interfaces:**
- Consumes: Task 4 constants and `connector_primitives`.
- Produces:
  - Rust `pub fn outline_segments(pins: &BlockPins, hw: f64, hh: f64) -> Vec<Primitive>`; `pub(crate) fn offset(i: usize, n: usize) -> f64` (was private); `fn edge_point(dir: PinDir, off: f64, hw: f64, hh: f64) -> Point`; `fn pin_layout(bp: &BlockPin, off: f64, dir: PinDir, hw: f64, hh: f64) -> PinLayout` with `struct PinLayout { at, lead_from, num_at, name_at: Point }`; `fn side_pad(side: &[BlockPin]) -> f64`.
  - TS `export function sideOffset(i: number, n: number): number`, `export function connectorPrimitives(kind: ConnectorKind, dir: "left" | "right" | "up" | "down", edges: Point[]): Primitive[]`, `export function outlineSegments(pins: BlockPins, hw: number, hh: number): Primitive[]`, the nine `CONNECTOR_*_MM` constants (same values as Rust); in `dynamicSymbol.ts` `type BlockDir`, `edgePoint`, `pinLayout`, `sidePad`.

- [ ] **Step 1: Write the failing Rust tests** (append inside `mod tests` of `block.rs`)

```rust
    /// 線 (2点以上) の点の並び。線以外は None。
    fn line_pts(p: &Primitive) -> Option<Vec<(f64, f64)>> {
        match p {
            Primitive::Line { pts } => Some(pts.iter().map(|q| (q.x, q.y)).collect()),
            _ => None,
        }
    }

    /// 接続点 `at` で終わるリード (外形側の端, 接続点)。
    fn lead_to(def: &SymbolDef, at: Point) -> (Point, Point) {
        def.primitives
            .iter()
            .find_map(|p| match p {
                Primitive::Line { pts } if pts.len() == 2 && pts[1] == at => Some((pts[0], pts[1])),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{at:?} へのリードが無い"))
    }

    /// A pin with a connector reaches 5 mm out of the box: its lead starts at the outer end of the picture, its number sits 0.25 mm beyond the picture above the lead, and its name starts 3 mm inside; a plain pin in the same block is drawn as before.
    /// コネクタのあるピンは外形から5mm外まで伸びる: リードは絵の外端から始まり、番号はリードの上で絵の0.25mm外、名前は3mm内側から書く。同じブロックの普通のピンは今までどおり。
    #[test]
    fn a_connector_pin_reaches_five_millimetres_out() {
        let def = block_symbol(&attrs(&[(
            "block_pins",
            r#"{"left":[{"no":"1","name":"A","connector":"usb_a_f"},{"no":"2","name":"B"}],"right":[{"no":"33","name":"C","connector":"dc_m"}]}"#,
        )]))
        .expect("ブロック定義");
        // 外形 15×15 (hw = hh = 7.5)
        assert_eq!(pin_at(&def, "1"), Point::new(-12.5, -2.5));
        assert_eq!(lead_to(&def, Point::new(-12.5, -2.5)).0, Point::new(-10.0, -2.5));
        assert_eq!(text_at(&def, "1"), Point::new(-10.85, -4.5), "番号の内端は絵の0.25mm外 (-10.25)");
        assert_eq!(text_at(&def, "A"), Point::new(-3.9, -2.5), "名前は外形の3mm内側から");
        assert_eq!(pin_at(&def, "33"), Point::new(12.5, 0.0));
        assert_eq!(lead_to(&def, Point::new(12.5, 0.0)).0, Point::new(10.0, 0.0));
        assert_eq!(text_at(&def, "33"), Point::new(11.45, -2.0));
        assert_eq!(text_at(&def, "C"), Point::new(3.9, 0.0));
        // 普通のピンは今までどおり (2.5mmのリード、番号はリードの上の中央、名前は1.5mm内側)
        assert_eq!(pin_at(&def, "2"), Point::new(-10.0, 2.5));
        assert_eq!(lead_to(&def, Point::new(-10.0, 2.5)).0, Point::new(-7.5, 2.5));
        assert_eq!(text_at(&def, "2"), Point::new(-8.75, 0.5));
        assert_eq!(text_at(&def, "B"), Point::new(-5.4, 2.5));
    }

    /// On the top and bottom sides a connector pin's number moves outside the box, right of the lead beyond the picture, while its name stays inside like a plain pin's.
    /// 上辺・下辺では、コネクタのあるピンの番号は外形の外 (絵より先のリードの右) へ出て、名前は普通のピンと同じく内側に残る。
    #[test]
    fn top_and_bottom_connector_numbers_sit_beside_the_lead() {
        let def = block_symbol(&attrs(&[(
            "block_pins",
            r#"{"top":[{"no":"U1","name":"A","connector":"usb_a_f"},{"no":"2","name":"B"}],"bottom":[{"no":"P","connector":"micro_b_f"}]}"#,
        )]))
        .expect("ブロック定義");
        // 外形 15×20 (hw = 7.5, hh = 10)
        assert_eq!(pin_at(&def, "U1"), Point::new(-2.5, -15.0));
        assert_eq!(lead_to(&def, Point::new(-2.5, -15.0)).0, Point::new(-2.5, -12.5));
        assert_eq!(text_at(&def, "U1"), Point::new(-0.8, -13.75));
        assert_eq!(text_at(&def, "A"), Point::new(-2.5, -5.5), "名前は普通のピンと同じ4.5mm内側");
        assert_eq!(text_at(&def, "2"), Point::new(2.5, -8.0), "普通のピンの番号は内側のまま");
        assert_eq!(pin_at(&def, "P"), Point::new(0.0, 15.0));
        assert_eq!(text_at(&def, "P"), Point::new(1.1, 13.75));
    }

    /// A block without connectors keeps its single outline rectangle, while a block with connectors draws the outline as lines with a gap exactly where each picture crosses the edge.
    /// コネクタの無いブロックは外形の長方形1つのまま。コネクタのあるブロックは、絵が辺と交わる所だけを空けた線分で外形を描く。
    #[test]
    fn the_outline_has_gaps_exactly_at_the_pictures() {
        let plain = block_symbol(&attrs(&[("block_pins", SAMPLE)])).expect("ブロック定義");
        assert!(matches!(plain.primitives[0], Primitive::Rect { filled: false, .. }));
        assert_eq!(plain.primitives.iter().filter(|p| matches!(p, Primitive::Rect { .. })).count(), 1);

        let def = block_symbol(&attrs(&[(
            "block_pins",
            r#"{"left":[{"no":"1","connector":"usb_a_f"},{"no":"2"},{"no":"3","connector":"screw"},{"no":"4","connector":"screw"}]}"#,
        )]))
        .expect("ブロック定義");
        // 外形 15×25 (hw = 7.5, hh = 12.5)。左辺のピンは y = -7.5, -2.5, 2.5, 7.5
        let outline: Vec<Vec<(f64, f64)>> = def.primitives[..6].iter().map(|p| line_pts(p).expect("線")).collect();
        assert_eq!(
            outline,
            vec![
                vec![(-7.5, -12.5), (7.5, -12.5)],
                vec![(7.5, -12.5), (7.5, 12.5)],
                vec![(-7.5, 12.5), (7.5, 12.5)],
                vec![(-7.5, -12.5), (-7.5, -9.5)],
                vec![(-7.5, -5.5), (-7.5, 0.5)],
                vec![(-7.5, 9.5), (-7.5, 12.5)],
            ]
        );
        assert!(matches!(def.primitives[6], Primitive::Rect { .. }), "外形の次は1本目のピンの絵");
    }

    /// The automatic outline grows by 1.5 mm on each side that has a connector, because the names there start 3 mm inside instead of 1.5 mm.
    /// 外形の自動寸法は、コネクタのある辺ごとに1.5mm広がる (その辺のピン名は1.5mmではなく3mm内側から書くため)。
    #[test]
    fn the_outline_makes_room_for_connector_bodies() {
        let size = |json: &str| block_box(&parse_block_spec(&attrs(&[("block_pins", json)])).unwrap());
        assert_eq!(size(r#"{"left":[{"no":"1","name":"ABCDE"}],"right":[{"no":"2","name":"ABCDE"}]}"#), (20.0, 10.0));
        assert_eq!(
            size(r#"{"left":[{"no":"1","name":"ABCDE","connector":"usb_a_f"}],"right":[{"no":"2","name":"ABCDE"}]}"#),
            (25.0, 10.0),
            "3 + 1.5 + 6 + 6 + 5 = 21.5 → 25"
        );
    }
```

- [ ] **Step 2: Run to verify RED** (one filter per run)

- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core a_connector_pin_reaches_five_millimetres_out 2>&1 | grep -E "panicked|left:|right:" -A1 | head -4` — Expected: FAIL, `left: Point { x: -10.0, y: -2.5 }` vs `right: Point { x: -12.5, y: -2.5 }`.
- `… cargo test -p madake-core top_and_bottom_connector_numbers_sit_beside_the_lead …` — Expected: FAIL, `U1` at `y: -12.5` instead of `-15.0`.
- `… cargo test -p madake-core the_outline_has_gaps_exactly_at_the_pictures …` — Expected: FAIL, panic `線` (primitive 0 is still a `Rect`).
- `… cargo test -p madake-core the_outline_makes_room_for_connector_bodies …` — Expected: FAIL, `(20.0, 10.0)` vs `(25.0, 10.0)`.

(`…` = the same `cd …/src-tauri && ` prefix as the first command.)

- [ ] **Step 3: Implement `outline_segments` in `connector.rs`**

Replace `use crate::block::BlockPin;` with:

```rust
use crate::block::{offset, BlockPin, BlockPins};
```

Directly above `/// テスト用: プリミティブが占める範囲` add:

```rust
/// コネクタのあるブロックの外形: 絵が辺と交わる所を空けた線分 (上→右→下→左、各辺は座標の小さい方から)。
/// 白い口が辺の線で塗りつぶされずに見えるのはこのすき間のおかげ。長さ0の線分は作らない。
pub fn outline_segments(pins: &BlockPins, hw: f64, hh: f64) -> Vec<Primitive> {
    let gaps = |side: &[BlockPin]| -> Vec<(f64, f64)> {
        let n = side.len();
        connector_groups(side)
            .iter()
            .map(|g| {
                (
                    offset(g.start, n) - CONNECTOR_HALF_SPAN_MM,
                    offset(g.end - 1, n) + CONNECTOR_HALF_SPAN_MM,
                )
            })
            .collect()
    };
    let mut out = Vec::new();
    edge_segments(&mut out, hw, &gaps(&pins.top), |x| Point::new(x, -hh));
    edge_segments(&mut out, hh, &gaps(&pins.right), |y| Point::new(hw, y));
    edge_segments(&mut out, hw, &gaps(&pins.bottom), |x| Point::new(x, hh));
    edge_segments(&mut out, hh, &gaps(&pins.left), |y| Point::new(-hw, y));
    out
}

/// 1辺 (`-half`〜`half`) を、すき間 `gaps` (小さい順) を除いた線分にする。
fn edge_segments(out: &mut Vec<Primitive>, half: f64, gaps: &[(f64, f64)], at: impl Fn(f64) -> Point) {
    let mut cursor = -half;
    for &(lo, hi) in gaps {
        if lo > cursor {
            out.push(Primitive::Line { pts: vec![at(cursor), at(lo)] });
        }
        cursor = cursor.max(hi);
    }
    if half > cursor {
        out.push(Primitive::Line { pts: vec![at(cursor), at(half)] });
    }
}

```

In the test module of `connector.rs`, delete the now-redundant line `use crate::block::BlockPins;` (the parent module imports it).

- [ ] **Step 4: Implement the block geometry in `block.rs`**

Replace `use crate::connector::{connector_groups, ConnectorKind};` with:

```rust
use crate::connector::{
    connector_groups, connector_primitives, outline_segments, ConnectorKind, CONNECTOR_IN_MM,
    CONNECTOR_LEAD_MM, CONNECTOR_NUM_GAP_MM, CONNECTOR_NUM_SIDE_MM, CONNECTOR_OUT_MM,
};
```

Replace `fn offset(i: usize, n: usize) -> f64 {` with `pub(crate) fn offset(i: usize, n: usize) -> f64 {`.

Directly above `/// 外形寸法を決める。明示指定があればそれ、無ければピン数とピン名の長さから自動。` add:

```rust
/// 左右の辺の、外形からピン名までの余白 (mm)。コネクタのある辺は絵の内側の深さぶん広い。
fn side_pad(side: &[BlockPin]) -> f64 {
    if side.iter().any(|p| p.connector.is_some()) {
        BLOCK_TEXT_PAD_MM + CONNECTOR_IN_MM
    } else {
        BLOCK_TEXT_PAD_MM
    }
}

```

In `block_box`, replace

```rust
    let w_names = 2.0 * BLOCK_TEXT_PAD_MM
        + max_name_width(&pins.left)
        + max_name_width(&pins.right)
        + BLOCK_PIN_PITCH_MM;
```

with

```rust
    // コネクタの無いブロックでは 1.5 + 1.5 = 3.0 で、今までの 2.0 × 1.5 と同じ値になる
    let w_names = side_pad(&pins.left)
        + side_pad(&pins.right)
        + max_name_width(&pins.left)
        + max_name_width(&pins.right)
        + BLOCK_PIN_PITCH_MM;
```

Replace the whole function `fn block_symbol_from_spec(spec: &BlockSpec) -> SymbolDef {` … up to (not including) its `    SymbolDef {` literal, i.e. this block:

```rust
fn block_symbol_from_spec(spec: &BlockSpec) -> SymbolDef {
    let (w, h) = block_box(spec);
    let (hw, hh) = (w / 2.0, h / 2.0);
    let mut primitives = vec![Primitive::Rect {
        p1: Point::new(-hw, -hh),
        p2: Point::new(hw, hh),
        filled: false,
    }];
```

… through …

```rust
    side(&pins_spec.left, PinDir::Left, &mut primitives, &mut pins);
    side(&pins_spec.right, PinDir::Right, &mut primitives, &mut pins);
    side(&pins_spec.top, PinDir::Up, &mut primitives, &mut pins);
    side(&pins_spec.bottom, PinDir::Down, &mut primitives, &mut pins);

```

with:

```rust
/// ピンが外形と交わる点。
fn edge_point(dir: PinDir, off: f64, hw: f64, hh: f64) -> Point {
    match dir {
        PinDir::Left => Point::new(-hw, off),
        PinDir::Right => Point::new(hw, off),
        PinDir::Up => Point::new(off, -hh),
        PinDir::Down => Point::new(off, hh),
    }
}

/// 1本のピンの接続点・リードの始点・番号の位置・名前の位置。
struct PinLayout {
    at: Point,
    lead_from: Point,
    num_at: Point,
    name_at: Point,
}

/// ピンの描き位置。コネクタの無いピンは今までどおり。コネクタのあるピンは接続点が外形から5mm、
/// リードは絵の外端から、番号は絵の外 (左右はリードの上、上下はリードの右)、左右の名前は3mm内側から。
fn pin_layout(bp: &BlockPin, off: f64, dir: PinDir, hw: f64, hh: f64) -> PinLayout {
    let edge = edge_point(dir, off, hw, hh);
    if bp.connector.is_none() {
        let (at, num_at, name_at) = match dir {
            PinDir::Left => (
                Point::new(-hw - BLOCK_LEAD_MM, off),
                Point::new(-hw - BLOCK_LEAD_MM / 2.0, off - BLOCK_PIN_TEXT_MM),
                Point::new(-hw + BLOCK_TEXT_PAD_MM + name_width(&bp.name) / 2.0, off),
            ),
            PinDir::Right => (
                Point::new(hw + BLOCK_LEAD_MM, off),
                Point::new(hw + BLOCK_LEAD_MM / 2.0, off - BLOCK_PIN_TEXT_MM),
                Point::new(hw - BLOCK_TEXT_PAD_MM - name_width(&bp.name) / 2.0, off),
            ),
            PinDir::Up => (
                Point::new(off, -hh - BLOCK_LEAD_MM),
                Point::new(off, -hh + BLOCK_PIN_TEXT_MM),
                Point::new(off, -hh + BLOCK_PIN_TEXT_MM + BLOCK_PIN_PITCH_MM / 2.0),
            ),
            PinDir::Down => (
                Point::new(off, hh + BLOCK_LEAD_MM),
                Point::new(off, hh - BLOCK_PIN_TEXT_MM),
                Point::new(off, hh - BLOCK_PIN_TEXT_MM - BLOCK_PIN_PITCH_MM / 2.0),
            ),
        };
        return PinLayout { at, lead_from: edge, num_at, name_at };
    }
    let num_w = name_width(bp.no.trim());
    let name_w = name_width(&bp.name);
    let num_out = CONNECTOR_OUT_MM + CONNECTOR_NUM_GAP_MM;
    let name_in = BLOCK_TEXT_PAD_MM + CONNECTOR_IN_MM;
    let num_tb = (CONNECTOR_OUT_MM + CONNECTOR_LEAD_MM) / 2.0;
    let (at, lead_from, num_at, name_at) = match dir {
        PinDir::Left => (
            Point::new(-hw - CONNECTOR_LEAD_MM, off),
            Point::new(-hw - CONNECTOR_OUT_MM, off),
            Point::new(-(hw + num_out + num_w / 2.0), off - BLOCK_PIN_TEXT_MM),
            Point::new(-hw + name_in + name_w / 2.0, off),
        ),
        PinDir::Right => (
            Point::new(hw + CONNECTOR_LEAD_MM, off),
            Point::new(hw + CONNECTOR_OUT_MM, off),
            Point::new(hw + num_out + num_w / 2.0, off - BLOCK_PIN_TEXT_MM),
            Point::new(hw - name_in - name_w / 2.0, off),
        ),
        PinDir::Up => (
            Point::new(off, -hh - CONNECTOR_LEAD_MM),
            Point::new(off, -hh - CONNECTOR_OUT_MM),
            Point::new(off + CONNECTOR_NUM_SIDE_MM + num_w / 2.0, -(hh + num_tb)),
            Point::new(off, -hh + BLOCK_PIN_TEXT_MM + BLOCK_PIN_PITCH_MM / 2.0),
        ),
        PinDir::Down => (
            Point::new(off, hh + CONNECTOR_LEAD_MM),
            Point::new(off, hh + CONNECTOR_OUT_MM),
            Point::new(off + CONNECTOR_NUM_SIDE_MM + num_w / 2.0, hh + num_tb),
            Point::new(off, hh - BLOCK_PIN_TEXT_MM - BLOCK_PIN_PITCH_MM / 2.0),
        ),
    };
    PinLayout { at, lead_from, num_at, name_at }
}

fn block_symbol_from_spec(spec: &BlockSpec) -> SymbolDef {
    let (w, h) = block_box(spec);
    let (hw, hh) = (w / 2.0, h / 2.0);
    // コネクタの無いブロックは今までどおり長方形1つ。あれば絵の所を空けた線分 (白い口に見せる)
    let mut primitives = if spec.has_connectors() {
        outline_segments(&spec.pins, hw, hh)
    } else {
        vec![Primitive::Rect {
            p1: Point::new(-hw, -hh),
            p2: Point::new(hw, hh),
            filled: false,
        }]
    };
    let mut pins: Vec<PinDef> = Vec::new();
    let pins_spec = &spec.pins;

    let side = |list: &[BlockPin], dir: PinDir, primitives: &mut Vec<Primitive>, pins: &mut Vec<PinDef>| {
        let n = list.len();
        let groups = connector_groups(list);
        for (i, bp) in list.iter().enumerate() {
            // まとまりの最初のピンの前に、そのまとまりの絵を描く
            if let Some(group) = groups.iter().find(|g| g.start == i) {
                let edges: Vec<Point> = (group.start..group.end)
                    .map(|j| edge_point(dir, offset(j, n), hw, hh))
                    .collect();
                primitives.extend(connector_primitives(group.kind, dir, &edges));
            }
            let layout = pin_layout(bp, offset(i, n), dir, hw, hh);
            primitives.push(lead(layout.lead_from.x, layout.lead_from.y, layout.at.x, layout.at.y));
            primitives.push(text(layout.num_at.x, layout.num_at.y, bp.no.trim(), BLOCK_PIN_TEXT_MM));
            if !bp.name.is_empty() {
                primitives.push(text(layout.name_at.x, layout.name_at.y, &bp.name, BLOCK_PIN_TEXT_MM));
            }
            pins.push(PinDef {
                number: bp.no.trim().into(),
                name: bp.name.clone(),
                at: layout.at,
                dir,
                contacts: bp.contacts.iter().map(|c| c.trim().to_string()).collect(),
            });
        }
    };
    side(&pins_spec.left, PinDir::Left, &mut primitives, &mut pins);
    side(&pins_spec.right, PinDir::Right, &mut primitives, &mut pins);
    side(&pins_spec.top, PinDir::Up, &mut primitives, &mut pins);
    side(&pins_spec.bottom, PinDir::Down, &mut primitives, &mut pins);

```

Also update the doc comment of `block_symbol` (the `/// 外形は原点中心の長方形。…` paragraph) by adding one line at its end:

```rust
/// ピンに `connector` があると、そのピンは外形から5mmまで伸び、外形の辺にコネクタの絵を描く ([`crate::connector`])。
```

- [ ] **Step 5: Run the Rust tests to verify GREEN**

- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core block:: 2>&1 | grep -E "FAILED|panicked|test result"` — Expected: `ok`, 0 failed, except `connector_blocks_match_the_shared_golden_file`, which now FAILS with `golden と違う` (the drawing changed on purpose).
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms/src-tauri && cargo build -p madake-core 2>&1 | grep -c warning` — Expected: `0`.

- [ ] **Step 6: Regenerate the golden file and see the TS twin go RED**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms/src-tauri && MADAKE_UPDATE_GOLDEN=1 cargo test -p madake-core connector_blocks_match_the_shared_golden_file 2>&1 | grep "test result"` — Expected: `ok`.
Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && git diff --stat src-tauri/crates/madake-core/tests/golden/connector-blocks.json` — Expected: the file changed. Spot-check that the line of `"plain"` is unchanged: `git diff src-tauri/crates/madake-core/tests/golden/connector-blocks.json | grep -c '"name":"plain"'` → `0`.
Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && npx vitest run src/canvas/dynamicSymbol.test.ts -t "connector golden" 2>&1 | grep -E "usb_a_f-left|failed" | head -3`
Expected: FAIL; the diff shows `"name": "usb_a_f-left"` (the first case).

- [ ] **Step 7: Implement the TS twin in `connector.ts`**

Replace `import type { BlockPin } from "./dynamicSymbol";` with:

```ts
import type { Point, Primitive } from "../ipc";
import type { BlockPin, BlockPins } from "./dynamicSymbol";

/** 絵の本体が外形の内側に入る深さ mm。 */
export const CONNECTOR_IN_MM = 1.5;
/** 絵の本体が外形の外側へ出る長さ mm。 */
export const CONNECTOR_OUT_MM = 2.5;
/** 本体の辺に沿う長さの半分 mm (本体は4mm)。 */
export const CONNECTOR_HALF_SPAN_MM = 2.0;
/** コネクタのあるピンの、外形から接続点までの長さ mm。 */
export const CONNECTOR_LEAD_MM = 5.0;
/** 本体の外端からピン番号までのすき間 mm。 */
export const CONNECTOR_NUM_GAP_MM = 0.25;
/** 上下辺のピン番号をリードから右へずらす量 mm。 */
export const CONNECTOR_NUM_SIDE_MM = 0.5;
/** 1本だけのまとまりの名前をピン名の後ろに空ける量 mm。 */
export const CONNECTOR_LABEL_GAP_MM = 1.0;
/** 上下辺のまとまりの名前の中心の、外形からの深さ mm。 */
export const CONNECTOR_LABEL_TOP_MM = 7.0;
/** まとまりの名前を書く上下辺が外形の内側で使う高さ mm。 */
export const CONNECTOR_LABEL_INSET_MM = 10.0;
```

Directly above `/** 1辺の中の、1つの絵で描くピンのまとまり` add:

```ts
/** 辺のピン i 番目 (全 n 個) の中央揃えオフセット mm (Rust側 block::offset。他の動的シンボルも同じ規則)。 */
export function sideOffset(i: number, n: number): number {
  return (i - (n - 1) / 2) * 5.0;
}

```

Append at the end of the file:

```ts
type Dir = "left" | "right" | "up" | "down";

interface Frame {
  e: Point;
  u: Point;
  v: Point;
  theta: number;
}

/** 辺ごとのローカル座標 (右辺の座標を時計回りに回したもの)。 */
function frame(dir: Dir, e: Point): Frame {
  switch (dir) {
    case "right":
      return { e, u: { x: 1, y: 0 }, v: { x: 0, y: 1 }, theta: 0 };
    case "down":
      return { e, u: { x: 0, y: 1 }, v: { x: -1, y: 0 }, theta: 90 };
    case "left":
      return { e, u: { x: -1, y: 0 }, v: { x: 0, y: -1 }, theta: 180 };
    case "up":
      return { e, u: { x: 0, y: -1 }, v: { x: 1, y: 0 }, theta: 270 };
  }
}

function at(f: Frame, u: number, v: number): Point {
  return { x: f.e.x + u * f.u.x + v * f.v.x, y: f.e.y + u * f.u.y + v * f.v.y };
}

function line(f: Frame, pts: [number, number][]): Primitive {
  return { type: "line", pts: pts.map(([u, v]) => at(f, u, v)) };
}

function rect(f: Frame, [u1, v1]: [number, number], [u2, v2]: [number, number], filled: boolean): Primitive {
  const a = at(f, u1, v1);
  const b = at(f, u2, v2);
  return {
    type: "rect",
    p1: { x: Math.min(a.x, b.x), y: Math.min(a.y, b.y) },
    p2: { x: Math.max(a.x, b.x), y: Math.max(a.y, b.y) },
    filled,
  };
}

function circle(f: Frame, [u, v]: [number, number], r: number, filled: boolean): Primitive {
  return { type: "circle", center: at(f, u, v), r, filled };
}

/** 角度は辺の回転ぶん足す (Rust側 netlist::rotate_arc_angles のミラーなしと同じ: 開始は0〜360、振れ幅はそのまま)。 */
function arc(f: Frame, [u, v]: [number, number], r: number, startDeg: number, endDeg: number): Primitive {
  let start = (startDeg + f.theta) % 360;
  if (start < 0) start += 360;
  return { type: "arc", center: at(f, u, v), r, start_deg: start, end_deg: start + (endDeg - startDeg) };
}

/** 1つのまとまり (または1本のピン) の絵 (Rust側 connector_primitives と同じ順)。 */
export function connectorPrimitives(kind: ConnectorKind, dir: Dir, edges: Point[]): Primitive[] {
  const f = frame(dir, edges[0]);
  const vs = edges.map((p) => (p.x - f.e.x) * f.v.x + (p.y - f.e.y) * f.v.y);
  const v0 = Math.min(...vs);
  const v1 = Math.max(...vs);
  const i = -CONNECTOR_IN_MM;
  const o = CONNECTOR_OUT_MM;
  const s = CONNECTOR_HALF_SPAN_MM;
  const female = () => rect(f, [i, -s], [o, s], false);
  const housing = () => rect(f, [i, -s], [0, s], false);
  switch (kind) {
    case "usb_a_f":
      return [female(), rect(f, [-0.8, -0.9], [1.8, -0.1], true)];
    case "usb_a_m":
      return [housing(), rect(f, [0, -1.2], [o, 1.2], true)];
    case "usb_c_f":
      return [
        female(),
        line(f, [
          [-0.2, -0.8],
          [1.2, -0.8],
        ]),
        arc(f, [1.2, 0], 0.8, -90, 90),
        line(f, [
          [1.2, 0.8],
          [-0.2, 0.8],
        ]),
        arc(f, [-0.2, 0], 0.8, 90, 270),
        line(f, [
          [0, 0],
          [1.0, 0],
        ]),
      ];
    case "usb_c_m":
      return [housing(), rect(f, [0, -0.8], [1.7, 0.8], true), circle(f, [1.7, 0], 0.8, true)];
    case "micro_b_f":
      return [
        line(f, [
          [i, -s],
          [0, -s],
          [o, -1.4],
          [o, 1.4],
          [0, s],
          [i, s],
          [i, -s],
        ]),
        line(f, [
          [-0.6, 0],
          [1.6, 0],
        ]),
      ];
    case "micro_b_m":
      return [housing(), rect(f, [0, -0.6], [o, 0.6], true)];
    case "jack35_f":
      return [female(), circle(f, [0.5, 0], 1.0, false), circle(f, [0.5, 0], 0.35, true)];
    case "jack35_m":
      return [housing(), rect(f, [0, -1.0], [0.8, 1.0], true), rect(f, [0.8, -0.4], [o, 0.4], true)];
    case "dc_f":
      return [female(), circle(f, [0.5, 0], 1.5, false), circle(f, [0.5, 0], 0.6, true)];
    case "dc_m":
      return [housing(), rect(f, [0, -1.4], [o, -0.5], true), rect(f, [0, 0.5], [o, 1.4], true)];
    case "screw": {
      const out = [rect(f, [i, v0 - s], [o, v1 + s], false)];
      for (const v of vs) {
        out.push(circle(f, [0.5, v], 1.2, false));
        out.push(
          line(f, [
            [1.3, v - 0.8],
            [-0.3, v + 0.8],
          ]),
        );
      }
      return out;
    }
    case "jst_ph_f":
    case "jst_xh_f": {
      const out = [
        rect(f, [i, v0 - s], [o, v1 + s], false),
        line(f, [
          [1.5, v0 - s],
          [1.5, v1 + s],
        ]),
      ];
      for (const v of vs) out.push(rect(f, [-0.6, v - 0.6], [0.6, v + 0.6], false));
      return out;
    }
  }
}

/** 1辺 (`-half`〜`half`) を、すき間 `gaps` (小さい順) を除いた線分にする。 */
function edgeSegments(out: Primitive[], half: number, gaps: [number, number][], pointAt: (t: number) => Point) {
  let cursor = -half;
  for (const [lo, hi] of gaps) {
    if (lo > cursor) out.push({ type: "line", pts: [pointAt(cursor), pointAt(lo)] });
    cursor = Math.max(cursor, hi);
  }
  if (half > cursor) out.push({ type: "line", pts: [pointAt(cursor), pointAt(half)] });
}

/** コネクタのあるブロックの外形: 絵が辺と交わる所を空けた線分 (Rust側 outline_segments と同じ順)。 */
export function outlineSegments(pins: BlockPins, hw: number, hh: number): Primitive[] {
  const gaps = (side: BlockPin[] = []): [number, number][] =>
    connectorGroups(side).map((g) => [
      sideOffset(g.start, side.length) - CONNECTOR_HALF_SPAN_MM,
      sideOffset(g.end - 1, side.length) + CONNECTOR_HALF_SPAN_MM,
    ]);
  const out: Primitive[] = [];
  edgeSegments(out, hw, gaps(pins.top), (x) => ({ x, y: -hh }));
  edgeSegments(out, hh, gaps(pins.right), (y) => ({ x: hw, y }));
  edgeSegments(out, hw, gaps(pins.bottom), (x) => ({ x, y: hh }));
  edgeSegments(out, hh, gaps(pins.left), (y) => ({ x: -hw, y }));
  return out;
}
```

- [ ] **Step 8: Implement the TS twin in `dynamicSymbol.ts`**

Replace `import { connectorGroups, isConnectorKind, isGroupedConnector, type ConnectorKind } from "./connector";` with:

```ts
import {
  CONNECTOR_IN_MM,
  CONNECTOR_LEAD_MM,
  CONNECTOR_NUM_GAP_MM,
  CONNECTOR_NUM_SIDE_MM,
  CONNECTOR_OUT_MM,
  connectorGroups,
  connectorPrimitives,
  isConnectorKind,
  isGroupedConnector,
  outlineSegments,
  sideOffset as offset,
  type ConnectorKind,
} from "./connector";
```

Delete the line `const offset = (i: number, n: number) => (i - (n - 1) / 2) * 5.0;` (the import above replaces it; same formula, also used by `connector_{n}p` and `terminal_block_{n}p`).

Below `function captionInset(side: BlockPin[]): number { … }` add:

```ts

/** 左右の辺の、外形からピン名までの余白 mm (Rust側 side_pad)。 */
function sidePad(side: BlockPin[]): number {
  return side.some((p) => p.connector != null) ? BLOCK_TEXT_PAD_MM + CONNECTOR_IN_MM : BLOCK_TEXT_PAD_MM;
}
```

In `blockBox`, replace

```ts
  const wNames =
    2 * BLOCK_TEXT_PAD_MM + maxNameWidth(left) + maxNameWidth(right) + BLOCK_PIN_PITCH_MM;
```

with

```ts
  const wNames = sidePad(left) + sidePad(right) + maxNameWidth(left) + maxNameWidth(right) + BLOCK_PIN_PITCH_MM;
```

Directly above the doc comment `/**\n * 属性からブロックのシンボル定義を作る(Rust側 \`block_symbol\` と同一座標)。` add:

```ts
type BlockDir = "left" | "right" | "up" | "down";

/** ピンが外形と交わる点 (Rust側 edge_point)。 */
function edgePoint(dir: BlockDir, off: number, hw: number, hh: number): Point {
  if (dir === "left") return p(-hw, off);
  if (dir === "right") return p(hw, off);
  if (dir === "up") return p(off, -hh);
  return p(off, hh);
}

/** 1本のピンの描き位置 (Rust側 pin_layout と同じ式)。 */
function pinLayout(
  bp: BlockPin,
  off: number,
  dir: BlockDir,
  hw: number,
  hh: number,
): { at: Point; leadFrom: Point; numAt: Point; nameAt: Point } {
  const leadFrom = edgePoint(dir, off, hw, hh);
  const name = bp.name ?? "";
  if (bp.connector == null) {
    if (dir === "left") {
      return {
        at: p(-hw - BLOCK_LEAD_MM, off),
        leadFrom,
        numAt: p(-hw - BLOCK_LEAD_MM / 2, off - BLOCK_PIN_TEXT_MM),
        nameAt: p(-hw + BLOCK_TEXT_PAD_MM + nameWidth(name) / 2, off),
      };
    }
    if (dir === "right") {
      return {
        at: p(hw + BLOCK_LEAD_MM, off),
        leadFrom,
        numAt: p(hw + BLOCK_LEAD_MM / 2, off - BLOCK_PIN_TEXT_MM),
        nameAt: p(hw - BLOCK_TEXT_PAD_MM - nameWidth(name) / 2, off),
      };
    }
    if (dir === "up") {
      return {
        at: p(off, -hh - BLOCK_LEAD_MM),
        leadFrom,
        numAt: p(off, -hh + BLOCK_PIN_TEXT_MM),
        nameAt: p(off, -hh + BLOCK_PIN_TEXT_MM + BLOCK_PIN_PITCH_MM / 2),
      };
    }
    return {
      at: p(off, hh + BLOCK_LEAD_MM),
      leadFrom,
      numAt: p(off, hh - BLOCK_PIN_TEXT_MM),
      nameAt: p(off, hh - BLOCK_PIN_TEXT_MM - BLOCK_PIN_PITCH_MM / 2),
    };
  }
  const numW = nameWidth((bp.no ?? "").trim());
  const nameW = nameWidth(name);
  const numOut = CONNECTOR_OUT_MM + CONNECTOR_NUM_GAP_MM;
  const nameIn = BLOCK_TEXT_PAD_MM + CONNECTOR_IN_MM;
  const numTb = (CONNECTOR_OUT_MM + CONNECTOR_LEAD_MM) / 2;
  if (dir === "left") {
    return {
      at: p(-hw - CONNECTOR_LEAD_MM, off),
      leadFrom: p(-hw - CONNECTOR_OUT_MM, off),
      numAt: p(-(hw + numOut + numW / 2), off - BLOCK_PIN_TEXT_MM),
      nameAt: p(-hw + nameIn + nameW / 2, off),
    };
  }
  if (dir === "right") {
    return {
      at: p(hw + CONNECTOR_LEAD_MM, off),
      leadFrom: p(hw + CONNECTOR_OUT_MM, off),
      numAt: p(hw + numOut + numW / 2, off - BLOCK_PIN_TEXT_MM),
      nameAt: p(hw - nameIn - nameW / 2, off),
    };
  }
  if (dir === "up") {
    return {
      at: p(off, -hh - CONNECTOR_LEAD_MM),
      leadFrom: p(off, -hh - CONNECTOR_OUT_MM),
      numAt: p(off + CONNECTOR_NUM_SIDE_MM + numW / 2, -(hh + numTb)),
      nameAt: p(off, -hh + BLOCK_PIN_TEXT_MM + BLOCK_PIN_PITCH_MM / 2),
    };
  }
  return {
    at: p(off, hh + CONNECTOR_LEAD_MM),
    leadFrom: p(off, hh + CONNECTOR_OUT_MM),
    numAt: p(off + CONNECTOR_NUM_SIDE_MM + numW / 2, hh + numTb),
    nameAt: p(off, hh - BLOCK_PIN_TEXT_MM - BLOCK_PIN_PITCH_MM / 2),
  };
}

```

In `blockSymbol`, replace everything from

```ts
  const primitives: Primitive[] = [
    { type: "rect", p1: p(-hw, -hh), p2: p(hw, hh), filled: false },
  ];
```

through

```ts
  push(sideOf(table, "bottom"), "down");
```

with:

```ts
  // コネクタの無いブロックは今までどおり長方形1つ。あれば絵の所を空けた線分 (Rust側と同じ)
  const hasConnectors = BLOCK_SIDES.some((s) => sideOf(table, s).some((pin) => pin.connector != null));
  const primitives: Primitive[] = hasConnectors
    ? outlineSegments(table, hw, hh)
    : [{ type: "rect", p1: p(-hw, -hh), p2: p(hw, hh), filled: false }];
  const pins: PinDef[] = [];
  const push = (side: BlockPin[], dir: BlockDir) => {
    const n = side.length;
    const groups = connectorGroups(side);
    for (let i = 0; i < n; i++) {
      const bp = side[i];
      const no = (bp.no ?? "").trim();
      const name = bp.name ?? "";
      // まとまりの最初のピンの前に、そのまとまりの絵を描く
      const group = groups.find((g) => g.start === i);
      if (group) {
        const edges: Point[] = [];
        for (let j = group.start; j < group.end; j++) edges.push(edgePoint(dir, offset(j, n), hw, hh));
        primitives.push(...connectorPrimitives(group.kind, dir, edges));
      }
      const { at, leadFrom, numAt, nameAt } = pinLayout(bp, offset(i, n), dir, hw, hh);
      primitives.push({ type: "line", pts: [leadFrom, at] });
      primitives.push({ type: "text", at: numAt, text: no, height: BLOCK_PIN_TEXT_MM });
      if (name) {
        primitives.push({ type: "text", at: nameAt, text: name, height: BLOCK_PIN_TEXT_MM });
      }
      const contacts = (bp.contacts ?? []).map((c) => c.trim());
      pins.push(contacts.length ? { number: no, name, at, dir, contacts } : { number: no, name, at, dir });
    }
  };
  push(sideOf(table, "left"), "left");
  push(sideOf(table, "right"), "right");
  push(sideOf(table, "top"), "up");
  push(sideOf(table, "bottom"), "down");
```

- [ ] **Step 9: Run the TS side to verify GREEN**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && npx vitest run src/canvas 2>&1 | tail -5` — Expected: all passed (`connector golden` included). If `src/canvas/terminals.test.ts` times out, re-run it alone.
Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && npx vue-tsc --noEmit 2>&1 | grep -v "src/chat/markdown" | head` — Expected: no output.
Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core 2>&1 | grep -E "^test result|FAILED|panicked" | head` — Expected: all `ok`.

- [ ] **Step 10: Spec docs and commit**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check` — Expected: `spec docs up to date; all tests have bilingual spec comments`.

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms
git add src-tauri/crates/madake-core/src/connector.rs src-tauri/crates/madake-core/src/block.rs src-tauri/crates/madake-core/tests/golden/connector-blocks.json src/canvas/connector.ts src/canvas/dynamicSymbol.ts docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat: コネクタのあるピンに絵と5mmのリードを描き、外形に口を空ける"
```

---

### Task 6: Group labels and the box size for them (Rust + TS)

**Files:**
- Modify: `src-tauri/crates/madake-core/src/block.rs` (imports, `caption_inset`, new `side_text_width`, `block_box`, new `label_at`, label lines in `block_symbol_from_spec`, tests)
- Regenerate: `src-tauri/crates/madake-core/tests/golden/connector-blocks.json`
- Modify: `src/canvas/dynamicSymbol.ts` (imports, `captionInset`, `sideTextWidth`, `blockBox`, `labelAt`, label push in `blockSymbol`)

**Interfaces:**
- Consumes: `group_label`, `ConnectorGroup` (Task 1); `CONNECTOR_LABEL_GAP_MM`, `CONNECTOR_LABEL_TOP_MM`, `CONNECTOR_LABEL_INSET_MM` (Task 4); TS `groupLabel`, `ConnectorGroup` (Task 2), constants (Task 5).
- Produces: Rust `fn side_text_width(side: &[BlockPin]) -> f64`, `fn label_at(list: &[BlockPin], group: &ConnectorGroup, label: &str, dir: PinDir, hw: f64, hh: f64) -> Point`; TS `sideTextWidth`, `labelAt` with the same parameters.

- [ ] **Step 1: Write the failing Rust tests** (append inside `mod tests` of `block.rs`)

```rust
    /// 点が (x, y) にほぼ等しいことを確かめる (文字幅の足し算の丸め誤差を許す)。
    fn assert_near(p: Point, x: f64, y: f64) {
        assert!((p.x - x).abs() < 1e-9 && (p.y - y).abs() < 1e-9, "{p:?} は ({x}, {y}) ではない");
    }

    /// Group names are drawn where nothing else is: between the middle pins of a 2- or 3-pin group, after the pin name for a 1-pin group, 7 mm inside on the top; JST prints its series and an unnamed screw block prints nothing.
    /// まとまりの名前は空いた所に書く: 2〜3本のまとまりは中ほどのピンの間、1本だけならピン名の後ろ、上辺は7mm内側。JST はシリーズを書き、名前の無い端子台は何も書かない。
    #[test]
    fn group_labels_sit_in_the_free_places() {
        let def = block_symbol(&attrs(&[(
            "block_pins",
            r#"{"left":[{"no":"1","name":"SIG","connector":"jst_ph_f","connector_ref":"J9"},
                {"no":"2","name":"A","connector":"screw","connector_ref":"TB1"},{"no":"3","name":"B","connector":"screw","connector_ref":"TB1"},
                {"no":"4","name":"X","connector":"jst_xh_f"},{"no":"5","name":"Y","connector":"jst_xh_f"},{"no":"6","name":"Z","connector":"jst_xh_f"},
                {"no":"7","connector":"screw"},{"no":"8","connector":"screw"}],
               "right":[{"no":"9","connector":"jst_ph_f"}],
               "top":[{"no":"T1","connector":"jst_ph_f","connector_ref":"J3"},{"no":"T2","connector":"jst_ph_f","connector_ref":"J3"}]}"#,
        )]))
        .expect("ブロック定義");
        // 外形 25×45 (hw = 12.5, hh = 22.5)。左辺のピンは y = -17.5, -12.5, …, 17.5
        assert_near(text_at(&def, "J9 PH"), -1.9, -17.5); // 1本: 名前 SIG (3.6mm) の1mm後ろ
        assert_near(text_at(&def, "TB1"), -7.7, -10.0); // 2本: 1本目と2本目の間
        assert_near(text_at(&def, "XH"), -8.3, 5.0); // 3本: 2本目と3本目の間
        assert_near(text_at(&def, "PH"), 8.3, 0.0); // 右辺の1本・名前なし: 3mm内側から
        assert_near(text_at(&def, "J3 PH"), 0.0, -15.5); // 上辺: まとまりの中央、7mm内側
        let labels = texts(&def).into_iter().filter(|(t, _)| t.contains("PH") || t.contains("XH") || t.starts_with("TB")).count();
        assert_eq!(labels, 5, "名前の無い端子台 (7・8) には何も書かない");
    }

    /// The automatic outline also makes room for group names: a name wider than the pin names widens the box, and a top side that prints a group name keeps 10 mm inside the box.
    /// 外形の自動寸法はまとまりの名前の場所も取る: ピン名より広い名前で箱が広がり、まとまりの名前を書く上辺は箱の内側に10mmを取る。
    #[test]
    fn the_outline_makes_room_for_group_names() {
        let size = |json: &str| block_box(&parse_block_spec(&attrs(&[("block_pins", json)])).unwrap());
        assert_eq!(size(r#"{"left":[{"no":"1","name":"SIG","connector":"jst_ph_f","connector_ref":"J9"}]}"#), (25.0, 10.0), "3 + 1.5 + (3.6 + 1 + 6) + 5 = 20.1 → 25");
        assert_eq!(size(r#"{"left":[{"no":"1","name":"SIG","connector":"jst_ph_f"}]}"#), (20.0, 10.0), "PH だけなら 16.5 → 20");
        assert_eq!(size(r#"{"top":[{"no":"1","connector":"jst_ph_f"},{"no":"2","connector":"jst_ph_f"}]}"#), (15.0, 15.0));
        assert_eq!(size(r#"{"top":[{"no":"1"},{"no":"2"}]}"#), (15.0, 10.0));
    }
```

- [ ] **Step 2: Run to verify RED**

- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core group_labels_sit_in_the_free_places 2>&1 | grep panicked -A1 | head -2` — Expected: FAIL, `文字 J9 PH が描かれていない`.
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms/src-tauri && cargo test -p madake-core the_outline_makes_room_for_group_names 2>&1 | grep -E "left:|right:" | head -2` — Expected: FAIL, `left: (15.0, 10.0)` vs `right: (25.0, 10.0)`.

- [ ] **Step 3: Implement in `block.rs`**

Replace the `use crate::connector::{ … };` block with:

```rust
use crate::connector::{
    connector_groups, connector_primitives, group_label, outline_segments, ConnectorGroup, ConnectorKind,
    CONNECTOR_IN_MM, CONNECTOR_LABEL_GAP_MM, CONNECTOR_LABEL_INSET_MM, CONNECTOR_LABEL_TOP_MM,
    CONNECTOR_LEAD_MM, CONNECTOR_NUM_GAP_MM, CONNECTOR_NUM_SIDE_MM, CONNECTOR_OUT_MM,
};
```

Replace

```rust
/// 上辺・下辺のピンが外形の内側で使う高さ (mm)。番号だけなら1行、名前もあれば2行。
fn caption_inset(side: &[BlockPin]) -> f64 {
    if side.is_empty() {
        0.0
    } else if side.iter().any(|p| !p.name.is_empty()) {
```

with

```rust
/// 上辺・下辺のピンが外形の内側で使う高さ (mm)。番号だけなら1行、名前もあれば2行、
/// コネクタのまとまりの名前を書く辺はその下にもう1行。
fn caption_inset(side: &[BlockPin]) -> f64 {
    if side.is_empty() {
        0.0
    } else if connector_groups(side).iter().any(|g| group_label(g).is_some()) {
        CONNECTOR_LABEL_INSET_MM
    } else if side.iter().any(|p| !p.name.is_empty()) {
```

Below `fn side_pad … }` add:

```rust

/// 左右の辺の内側に書く文字の最大幅 (mm): ピン名と、コネクタのまとまりの名前
/// (1本だけのまとまりはピン名の後ろに続けて書くので、その合計)。
fn side_text_width(side: &[BlockPin]) -> f64 {
    let mut width = max_name_width(side);
    for group in connector_groups(side) {
        let Some(label) = group_label(&group) else { continue };
        let label_w = name_width(&label);
        let name = &side[group.start].name;
        let need = if group.end - group.start >= 2 || name.is_empty() {
            label_w
        } else {
            name_width(name) + CONNECTOR_LABEL_GAP_MM + label_w
        };
        width = width.max(need);
    }
    width
}
```

In `block_box`, replace

```rust
        + max_name_width(&pins.left)
        + max_name_width(&pins.right)
```

with

```rust
        + side_text_width(&pins.left)
        + side_text_width(&pins.right)
```

Directly above `fn block_symbol_from_spec(spec: &BlockSpec) -> SymbolDef {` add:

```rust
/// コネクタのまとまりの名前の位置: 左右の2本以上はまとまりの中ほどのピンの間の空いた行、
/// 1本だけはピン名の後ろ、上下はまとまりの中央で外形から7mm内側。
fn label_at(list: &[BlockPin], group: &ConnectorGroup, label: &str, dir: PinDir, hw: f64, hh: f64) -> Point {
    let n = list.len();
    let first = offset(group.start, n);
    let label_w = name_width(label);
    let name_in = BLOCK_TEXT_PAD_MM + CONNECTOR_IN_MM;
    match dir {
        PinDir::Left | PinDir::Right => {
            let count = group.end - group.start;
            let (y, start) = if count >= 2 {
                let k = ((count - 1) / 2) as f64;
                (first + BLOCK_PIN_PITCH_MM / 2.0 + BLOCK_PIN_PITCH_MM * k, name_in)
            } else {
                let name = &list[group.start].name;
                let start = if name.is_empty() {
                    name_in
                } else {
                    name_in + name_width(name) + CONNECTOR_LABEL_GAP_MM
                };
                (first, start)
            };
            let x = if dir == PinDir::Left {
                -hw + start + label_w / 2.0
            } else {
                hw - start - label_w / 2.0
            };
            Point::new(x, y)
        }
        PinDir::Up | PinDir::Down => {
            let mid = (first + offset(group.end - 1, n)) / 2.0;
            let y = if dir == PinDir::Up {
                -hh + CONNECTOR_LABEL_TOP_MM
            } else {
                hh - CONNECTOR_LABEL_TOP_MM
            };
            Point::new(mid, y)
        }
    }
}

```

In `block_symbol_from_spec`, replace

```rust
            // まとまりの最初のピンの前に、そのまとまりの絵を描く
            if let Some(group) = groups.iter().find(|g| g.start == i) {
                let edges: Vec<Point> = (group.start..group.end)
                    .map(|j| edge_point(dir, offset(j, n), hw, hh))
                    .collect();
                primitives.extend(connector_primitives(group.kind, dir, &edges));
            }
```

with

```rust
            // まとまりの最初のピンの前に、そのまとまりの絵と名前を描く
            if let Some(group) = groups.iter().find(|g| g.start == i) {
                let edges: Vec<Point> = (group.start..group.end)
                    .map(|j| edge_point(dir, offset(j, n), hw, hh))
                    .collect();
                primitives.extend(connector_primitives(group.kind, dir, &edges));
                if let Some(label) = group_label(group) {
                    let at = label_at(list, group, &label, dir, hw, hh);
                    primitives.push(text(at.x, at.y, &label, BLOCK_PIN_TEXT_MM));
                }
            }
```

- [ ] **Step 4: Rust GREEN, golden regeneration, TS RED**

- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core group_labels_sit_in_the_free_places 2>&1 | grep "test result"` — Expected: `ok`.
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms/src-tauri && cargo test -p madake-core the_outline_makes_room 2>&1 | grep "test result"` — Expected: `ok. 2 passed` (this task's and Task 5's box tests).
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms/src-tauri && MADAKE_UPDATE_GOLDEN=1 cargo test -p madake-core connector_blocks_match_the_shared_golden_file 2>&1 | grep "test result"` — Expected: `ok`.
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && npx vitest run src/canvas/dynamicSymbol.test.ts -t "connector golden" 2>&1 | grep -E "screw-left|failed" | head -3` — Expected: FAIL; the first differing case is `"name": "screw-left"` (the first case with a label; the 40 non-grouped cases are unchanged).

- [ ] **Step 5: Implement the TS twin in `dynamicSymbol.ts`**

In the `from "./connector"` import list, add `CONNECTOR_LABEL_GAP_MM`, `CONNECTOR_LABEL_INSET_MM`, `CONNECTOR_LABEL_TOP_MM`, `groupLabel` and `type ConnectorGroup`, so that it reads:

```ts
import {
  CONNECTOR_IN_MM,
  CONNECTOR_LABEL_GAP_MM,
  CONNECTOR_LABEL_INSET_MM,
  CONNECTOR_LABEL_TOP_MM,
  CONNECTOR_LEAD_MM,
  CONNECTOR_NUM_GAP_MM,
  CONNECTOR_NUM_SIDE_MM,
  CONNECTOR_OUT_MM,
  connectorGroups,
  connectorPrimitives,
  groupLabel,
  isConnectorKind,
  isGroupedConnector,
  outlineSegments,
  sideOffset as offset,
  type ConnectorGroup,
  type ConnectorKind,
} from "./connector";
```

Replace

```ts
function captionInset(side: BlockPin[]): number {
  if (side.length === 0) return 0;
  return side.some((p) => (p.name ?? "") !== "") ? 7.5 : 5.0;
}
```

with

```ts
function captionInset(side: BlockPin[]): number {
  if (side.length === 0) return 0;
  if (connectorGroups(side).some((g) => groupLabel(g) !== null)) return CONNECTOR_LABEL_INSET_MM;
  return side.some((p) => (p.name ?? "") !== "") ? 7.5 : 5.0;
}
```

Below `function sidePad … }` add:

```ts

/** 左右の辺の内側に書く文字の最大幅 mm (Rust側 side_text_width)。 */
function sideTextWidth(side: BlockPin[]): number {
  let width = maxNameWidth(side);
  for (const group of connectorGroups(side)) {
    const label = groupLabel(group);
    if (label === null) continue;
    const labelW = nameWidth(label);
    const name = side[group.start].name ?? "";
    const need =
      group.end - group.start >= 2 || name === "" ? labelW : nameWidth(name) + CONNECTOR_LABEL_GAP_MM + labelW;
    width = Math.max(width, need);
  }
  return width;
}
```

In `blockBox`, replace `maxNameWidth(left) + maxNameWidth(right)` with `sideTextWidth(left) + sideTextWidth(right)`.

Below `function pinLayout( … ) { … }` add:

```ts

/** コネクタのまとまりの名前の位置 (Rust側 label_at と同じ式)。 */
function labelAt(side: BlockPin[], group: ConnectorGroup, label: string, dir: BlockDir, hw: number, hh: number): Point {
  const n = side.length;
  const first = offset(group.start, n);
  const labelW = nameWidth(label);
  const nameIn = BLOCK_TEXT_PAD_MM + CONNECTOR_IN_MM;
  if (dir === "left" || dir === "right") {
    const count = group.end - group.start;
    let y = first;
    let start = nameIn;
    if (count >= 2) {
      y = first + BLOCK_PIN_PITCH_MM / 2 + BLOCK_PIN_PITCH_MM * Math.floor((count - 1) / 2);
    } else {
      const name = side[group.start].name ?? "";
      if (name !== "") start = nameIn + nameWidth(name) + CONNECTOR_LABEL_GAP_MM;
    }
    return p(dir === "left" ? -hw + start + labelW / 2 : hw - start - labelW / 2, y);
  }
  const mid = (first + offset(group.end - 1, n)) / 2;
  return p(mid, dir === "up" ? -hh + CONNECTOR_LABEL_TOP_MM : hh - CONNECTOR_LABEL_TOP_MM);
}
```

In `blockSymbol`, replace

```ts
      // まとまりの最初のピンの前に、そのまとまりの絵を描く
      const group = groups.find((g) => g.start === i);
      if (group) {
        const edges: Point[] = [];
        for (let j = group.start; j < group.end; j++) edges.push(edgePoint(dir, offset(j, n), hw, hh));
        primitives.push(...connectorPrimitives(group.kind, dir, edges));
      }
```

with

```ts
      // まとまりの最初のピンの前に、そのまとまりの絵と名前を描く
      const group = groups.find((g) => g.start === i);
      if (group) {
        const edges: Point[] = [];
        for (let j = group.start; j < group.end; j++) edges.push(edgePoint(dir, offset(j, n), hw, hh));
        primitives.push(...connectorPrimitives(group.kind, dir, edges));
        const label = groupLabel(group);
        if (label !== null) {
          primitives.push({ type: "text", at: labelAt(side, group, label, dir, hw, hh), text: label, height: BLOCK_PIN_TEXT_MM });
        }
      }
```

- [ ] **Step 6: TS GREEN and full checks**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && npx vitest run src/canvas 2>&1 | tail -5` — Expected: all passed.
Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && npx vue-tsc --noEmit 2>&1 | grep -v "src/chat/markdown" | head` — Expected: no output.
Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core 2>&1 | grep -E "^test result|FAILED|panicked" | head` — Expected: all `ok`.

- [ ] **Step 7: Spec docs and commit**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check` — Expected: `spec docs up to date; all tests have bilingual spec comments`.

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms
git add src-tauri/crates/madake-core/src/block.rs src-tauri/crates/madake-core/tests/golden/connector-blocks.json src/canvas/dynamicSymbol.ts docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat: 端子台・JST のまとめ名を空いた所に書き、外形をその分広げる"
```

---

### Task 7: Guards — no overlap, the SVG fill, the ERC signal

**Files:**
- Modify: `src-tauri/crates/madake-core/src/block.rs` (test module: overlap checker and test)
- Modify: `src-tauri/crates/madake-core/src/svg.rs` (test module)
- Modify: `src-tauri/crates/madake-core/src/verify.rs` (test module)

**Interfaces:**
- Consumes: fixtures `HUB_39`, `AMP_53`, `MIXED`, `TOP_BOTTOM` (Task 3), `lead_to` (Task 5), `primitive_extent` (Task 4), `edge_point`, `offset`, `outline_segments`, `connector_groups`, `connector_primitives`.
- Produces: tests only.

All three tests are regression tests: the behaviour already exists after Tasks 5–6. Step 3 proves the overlap test can fail.

- [ ] **Step 1: Write the tests**

Append inside `mod tests` of `block.rs`:

```rust
    /// 正の面積で重なるか (辺が触れるだけは重ならない)。幅0の線分の箱にもそのまま使える。
    fn overlaps(a: (Point, Point), b: (Point, Point)) -> bool {
        a.0.x < b.1.x - 1e-9 && b.0.x < a.1.x - 1e-9 && a.0.y < b.1.y - 1e-9 && b.0.y < a.1.y - 1e-9
    }

    /// 仕様 §2.5 の重なりの決まりを1つのブロックで確かめる。
    fn assert_nothing_overlaps(json: &str) {
        use crate::connector::primitive_extent;
        let a = attrs(&[("block_pins", json)]);
        let spec = parse_block_spec(&a).expect("ピン表");
        let def = block_symbol(&a).expect("ブロック定義");
        let (w, h) = block_box(&spec);
        let (hw, hh) = (w / 2.0, h / 2.0);
        // 普通のピンの番号は今でも外形をまたぐことがある (仕様 §2.5 (c) の対象外)
        let plain_numbers: Vec<&str> = spec.pins.all().filter(|p| p.connector.is_none()).map(|p| p.no.trim()).collect();
        let text_boxes: Vec<(String, (Point, Point))> = def
            .primitives
            .iter()
            .filter_map(|p| match p {
                Primitive::Text { at, text, height } => {
                    let half = crate::tidy::text_width(text, *height) / 2.0;
                    let lo = Point::new(at.x - half, at.y - height / 2.0);
                    let hi = Point::new(at.x + half, at.y + height / 2.0);
                    Some((text.clone(), (lo, hi)))
                }
                _ => None,
            })
            .collect();
        let mut pictures = Vec::new();
        for (list, dir) in [
            (&spec.pins.left, PinDir::Left),
            (&spec.pins.right, PinDir::Right),
            (&spec.pins.top, PinDir::Up),
            (&spec.pins.bottom, PinDir::Down),
        ] {
            let n = list.len();
            for g in connector_groups(list) {
                let edges: Vec<Point> = (g.start..g.end).map(|j| edge_point(dir, offset(j, n), hw, hh)).collect();
                let prims = connector_primitives(g.kind, dir, &edges);
                let (lo, hi) = prims.iter().map(primitive_extent).fold(
                    (Point::new(f64::INFINITY, f64::INFINITY), Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY)),
                    |(lo, hi), (a, b)| (Point::new(lo.x.min(a.x), lo.y.min(a.y)), Point::new(hi.x.max(b.x), hi.y.max(b.y))),
                );
                pictures.push((lo, hi));
            }
        }
        let leads: Vec<(Point, Point)> = def
            .pins
            .iter()
            .map(|p| {
                let (from, to) = lead_to(&def, p.at);
                primitive_extent(&lead(from.x, from.y, to.x, to.y))
            })
            .collect();
        let outline: Vec<(Point, Point)> = outline_segments(&spec.pins, hw, hh).iter().map(primitive_extent).collect();
        for (i, (ta, a)) in text_boxes.iter().enumerate() {
            for (tb, b) in &text_boxes[i + 1..] {
                assert!(!overlaps(*a, *b), "文字 {ta} と {tb} が重なる");
            }
            for p in &pictures {
                assert!(!overlaps(*a, *p), "文字 {ta} が絵 {p:?} に重なる");
            }
            for l in &leads {
                assert!(!overlaps(*a, *l), "文字 {ta} がリード {l:?} に重なる");
            }
            if !plain_numbers.contains(&ta.as_str()) {
                for o in &outline {
                    assert!(!overlaps(*a, *o), "文字 {ta} が外形 {o:?} に重なる");
                }
            }
        }
    }

    /// Regression test for Tasks 5-6 (already passes): with connectors of every kind, plain pins, pin numbers of one to four characters, names and group names, no two texts overlap and no text covers a picture, a lead or the box outline (the AOGU hub and amplifier included).
    /// Task 5〜6 の挙動の回帰テスト (最初から通る): 全種類のコネクタ・普通のピン・1〜4文字のピン番号・ピン名・まとまりの名前を並べても、文字同士は重ならず、文字が絵・リード・外形に重ならない (AOGU のハブとアンプも含む)。
    #[test]
    fn connector_texts_never_overlap() {
        for json in [MIXED, HUB_39, AMP_53, TOP_BOTTOM] {
            assert_nothing_overlaps(json);
        }
    }
```

Append inside `mod tests` of `svg.rs`, after `svg_draws_block_outline_with_pin_numbers_and_names`:

```rust
    /// Regression test for Task 5 (already passes): a plug's black insert is drawn as one more black-filled polygon than the same block without the connector, so PDF and PNG, which are made from this SVG, show it too.
    /// Task 5 の挙動の回帰テスト (最初から通る): プラグの黒い差し込みは、コネクタの無い同じブロックより黒く塗った多角形が1つ多い形で描かれるので、この SVG から作る PDF・PNG にも出る。
    #[test]
    fn svg_draws_a_plug_insert_as_a_black_polygon() {
        let filled = |pins: &str| {
            let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
            let block = crate::block::test_block("LTE1", "", 100.0, 50.0, pins);
            sheet.entities.insert(block.id(), block);
            let svg = sheet_to_svg(&sheet, &crate::symbol::sheet_symbol_defs(&sheet), &[]);
            svg.split("<polygon")
                .skip(1)
                .filter(|el| el.split("/>").next().unwrap_or("").contains("fill=\"#000\""))
                .count()
        };
        let plain = filled(r#"{"left":[{"no":"P1"}]}"#);
        assert_eq!(filled(r#"{"left":[{"no":"P1","connector":"usb_a_m"}]}"#), plain + 1);
    }
```

Append inside `mod tests` of `verify.rs`, after `unconnected_block_pins_are_listed_by_pin_name`:

```rust
    /// Regression test for Task 5 (already passes): a wired pin that gets a connector moves 2.5 mm outward and ERC says so: a straight wire still reaches the pin but leaves a dangling end, and a wire that left sideways leaves the pin named as unconnected.
    /// Task 5 の挙動の回帰テスト (最初から通る): 配線済みのピンにコネクタを付けると接続点が2.5mm外へ動き、ERC がそれを知らせる: まっすぐの配線はピンに届いたまま端が浮き、横へ出ていた配線ではピンが未接続として名前で挙がる。
    #[test]
    fn a_wired_pin_that_gets_a_connector_is_flagged_by_erc() {
        // 幅15mmのブロック: 右ピン OUT1 は普通なら (110, 50)、コネクタ付きなら (112.5, 50)
        let sheet_of = |pins: &str, points: &[(f64, f64)]| {
            sheet_with(vec![
                crate::block::test_block("U1", "", 100.0, 50.0, pins),
                symbol("resistor", "R1", 150.0, 50.0),
                wire(points),
            ])
        };
        let has = |sheet: &Sheet, code: &str, needle: &str| run(sheet).iter().any(|d| d.code == code && d.message.contains(needle));
        let plain = r#"{"right":[{"no":"OUT1"}]}"#;
        let usb = r#"{"right":[{"no":"OUT1","connector":"usb_a_f"}]}"#;
        let straight = [(110.0, 50.0), (142.5, 50.0)];
        let sideways = [(110.0, 50.0), (110.0, 80.0)];
        assert!(!has(&sheet_of(plain, &straight), "erc.dangling_wire", ""));
        assert!(!has(&sheet_of(plain, &sideways), "erc.unconnected_pin", "U1: "));
        assert!(has(&sheet_of(usb, &straight), "erc.dangling_wire", ""));
        assert!(!has(&sheet_of(usb, &straight), "erc.unconnected_pin", "U1: "), "まっすぐの配線はまだピンに届く");
        assert!(has(&sheet_of(usb, &sideways), "erc.unconnected_pin", "U1: 未接続のピン: OUT1"));
    }
```

- [ ] **Step 2: Run them** (one filter per run; Expected for each: `test result: ok. 1 passed`)

- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core connector_texts_never_overlap 2>&1 | grep "test result"`
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms/src-tauri && cargo test -p madake-core svg_draws_a_plug_insert_as_a_black_polygon 2>&1 | grep "test result"`
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms/src-tauri && cargo test -p madake-core a_wired_pin_that_gets_a_connector_is_flagged_by_erc 2>&1 | grep "test result"`

If the ERC test fails on the `sideways` plain case because the resistor's right pin or `R1` also reports, read the failing assertion: every check filters by `"U1: "`, so only `U1`'s own findings count.

- [ ] **Step 3: Prove the overlap test can fail, then revert**

In `connector.rs` temporarily change `pub const CONNECTOR_LABEL_TOP_MM: f64 = 7.0;` to `5.0`.
Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && touch src-tauri/crates/madake-core/src/lib.rs && cd src-tauri && cargo test -p madake-core connector_texts_never_overlap 2>&1 | grep panicked -A1 | head -2`
Expected: FAIL with `文字 J5 PH と A が重なる`.
Restore `7.0`, then run `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && git diff --stat src-tauri/crates/madake-core/src/connector.rs` — Expected: no output (file unchanged).

- [ ] **Step 4: Spec docs and commit**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check` — Expected: `spec docs up to date; all tests have bilingual spec comments`.

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms
git add src-tauri/crates/madake-core/src/block.rs src-tauri/crates/madake-core/src/svg.rs src-tauri/crates/madake-core/src/verify.rs docs/13-specification.md docs/13-specification.ja.md
git commit -m "test: コネクタの文字が重ならないこと・SVG の塗り・ERC の知らせを確かめる"
```

---

### Task 8: File format 19 and `port_block` for connector blocks

**Files:**
- Modify: `src-tauri/crates/madake-core/src/model.rs` (`FORMAT_VERSION` doc + value, `FileWriteGuard` doc, `is_port_block`, `EntityRepr` docs)
- Modify: `src-tauri/crates/madake-core/src/io.rs` (`save_project` doc, tests)

**Interfaces:**
- Consumes: `BlockSpec::has_connectors` (Task 1).
- Produces: `FORMAT_VERSION == 19`; `is_port_block(s)` true for any contact or connector.

- [ ] **Step 1: Write the failing tests** (in `mod tests` of `io.rs`)

Replace the whole test

```rust
    /// The current file format version is 18, the version that added port contacts on block pins (blocks with contacts are saved as "port_block").
    /// 現行のファイル形式バージョンは18 (ブロックのピンにポートの接点を追加し、接点のあるブロックを種類「port_block」で保存する版) である。
    #[test]
    fn the_current_format_version_is_eighteen() {
        assert_eq!(FORMAT_VERSION, 18);
        let mut old = Project::new("x");
        old.format_version = 17;
        migrate(&mut old);
        assert_eq!(old.format_version, 18);
    }
```

with

```rust
    /// The current file format version is 19, the version that added connector pictures on block pins (blocks with contacts or connectors are saved as "port_block").
    /// 現行のファイル形式バージョンは19 (ブロックのピンにコネクタの絵を追加し、接点かコネクタのあるブロックを種類「port_block」で保存する版) である。
    #[test]
    fn the_current_format_version_is_nineteen() {
        assert_eq!(FORMAT_VERSION, 19);
        let mut old = Project::new("x");
        old.format_version = 18;
        migrate(&mut old);
        assert_eq!(old.format_version, 19);
    }
```

Append at the end of `mod tests` (after `a_block_whose_contacts_were_removed_is_saved_as_symbol_again`):

```rust
    /// Regression test (already passes): a version 18 file with a port block opens unchanged apart from being moved up to the current version.
    /// 回帰テスト (最初から通る): ポートのブロックを含む形式18のファイルは、現行版へ上がる以外そのまま開ける。
    #[test]
    fn a_version_18_file_loads_unchanged() {
        let (mut project, id) = port_project();
        project.format_version = 18;
        let (_, path) = save_text(&project, "v18");
        let loaded = load_project(&path).unwrap();
        assert_eq!(loaded.format_version, FORMAT_VERSION);
        let Entity::Symbol(s) = &loaded.sheets[0].entities[&id] else { panic!("シンボルに戻る") };
        assert!(crate::block::parse_block_spec(&s.attrs).unwrap().has_contacts());
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    /// A block whose pins have only connectors (no contacts) is also saved under the kind "port_block" and comes back with its connectors, IPC/MCP JSON still says "symbol", and once its connectors are removed it is saved as "symbol" again.
    /// 接点が無くコネクタだけを持つブロックも種類「port_block」で保存され、読み直すとコネクタ付きで戻り、IPC・MCP の JSON は「symbol」のまま。コネクタを消せばまた「symbol」で保存される。
    #[test]
    fn a_block_with_connectors_is_saved_as_port_block() {
        let mut project = Project::new("connector");
        let block = crate::block::test_block("39", "", 100.0, 100.0, r#"{"top":[{"no":"U1","connector":"usb_a_f"}]}"#);
        let id = block.id();
        project.sheets[0].entities.insert(id, block);
        let (text, path) = save_text(&project, "connector");
        assert!(text.contains("\"kind\": \"port_block\""), "{text}");
        let loaded = load_project(&path).unwrap();
        let Entity::Symbol(s) = &loaded.sheets[0].entities[&id] else { panic!("シンボルに戻る") };
        let spec = crate::block::parse_block_spec(&s.attrs).unwrap();
        assert!(spec.has_connectors() && !spec.has_contacts());
        assert_eq!(serde_json::to_value(&project.sheets[0].entities[&id]).unwrap()["kind"], "symbol");
        std::fs::remove_dir_all(path.parent().unwrap()).ok();

        let Entity::Symbol(s) = project.sheets[0].entities.get_mut(&id).unwrap() else { panic!() };
        s.attrs.insert("block_pins".into(), r#"{"top":[{"no":"U1"}]}"#.into());
        let (text, path) = save_text(&project, "connector-removed");
        assert!(text.contains("\"kind\": \"symbol\"") && !text.contains("port_block"), "{text}");
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }
```

- [ ] **Step 2: Run to verify RED** (one filter per run)

- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core the_current_format_version_is_nineteen 2>&1 | grep -E "left:|right:" | head -2` — Expected: FAIL, `left: 18` / `right: 19`.
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms/src-tauri && cargo test -p madake-core a_block_with_connectors_is_saved_as_port_block 2>&1 | grep panicked -A1 | head -2` — Expected: FAIL at the `port_block` assertion (the file says `"kind": "symbol"`).
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms/src-tauri && cargo test -p madake-core a_version_18_file_loads_unchanged 2>&1 | grep "test result"` — Expected: `ok` (regression).

- [ ] **Step 3: Implement in `model.rs`**

Replace

```rust
///   (ブロックを黙って消さない)。形式17のビルドは版番号で断る。接点の無いファイルは版番号以外これまでと同じ
pub const FORMAT_VERSION: u32 = 18;
```

with

```rust
///   (ブロックを黙って消さない)。形式17のビルドは版番号で断る。接点の無いファイルは版番号以外これまでと同じ
/// - v19: ブロックのピンのコネクタの絵 ([`crate::block::BlockPin::connector`] / `connector_ref`)。コネクタのある
///   ブロックも種類`port_block`で書くので、v0.4.0 以前はそのファイルを開けない (知らないキーのブロックを黙って
///   消さない)。形式17・18のビルドは版番号で断る。コネクタの無いファイルは版番号以外これまでと同じ
pub const FORMAT_VERSION: u32 = 19;
```

Replace

```rust
/// 生きている間、エンティティを「ファイルの形」で書く: 接点のあるブロックを種類`port_block`にする。
```

with

```rust
/// 生きている間、エンティティを「ファイルの形」で書く: 接点かコネクタのあるブロックを種類`port_block`にする。
```

Replace

```rust
/// 接点を持つピンがあるブロックか (ファイルでは種類`port_block`)。
fn is_port_block(s: &SymbolInstance) -> bool {
    s.symbol_id == crate::block::BLOCK_SYMBOL_ID
        && crate::block::parse_block_spec(&s.attrs).is_ok_and(|spec| spec.has_contacts())
}
```

with

```rust
/// 接点かコネクタを持つピンがあるブロックか (ファイルでは種類`port_block`)。
fn is_port_block(s: &SymbolInstance) -> bool {
    s.symbol_id == crate::block::BLOCK_SYMBOL_ID
        && crate::block::parse_block_spec(&s.attrs).is_ok_and(|spec| spec.has_contacts() || spec.has_connectors())
}
```

Replace

```rust
/// 接点のあるブロックは、ファイルへ書くとき ([`FileWriteGuard`]) だけ種類`port_block`で書く (v18)。
```

with

```rust
/// 接点のあるブロック (v18) とコネクタのあるブロック (v19) は、ファイルへ書くとき ([`FileWriteGuard`]) だけ種類`port_block`で書く。
```

Replace

```rust
    /// 接点を持つピンがあるブロック (v18)。ファイルへ書くときだけ使う種類で、読むと [`Entity::Symbol`]。
```

with

```rust
    /// 接点 (v18) かコネクタ (v19) を持つピンがあるブロック。ファイルへ書くときだけ使う種類で、読むと [`Entity::Symbol`]。
```

In `io.rs`, replace

```rust
/// プロジェクトを整形JSONで保存する(git差分が読める形式)。接点のあるブロックは種類`port_block`で書く
```

with

```rust
/// プロジェクトを整形JSONで保存する(git差分が読める形式)。接点かコネクタのあるブロックは種類`port_block`で書く
```

- [ ] **Step 4: Run to verify GREEN**

- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core io:: 2>&1 | grep -E "FAILED|test result"` — Expected: `ok`, 0 failed (includes `a_project_without_ports_is_written_unchanged`, the byte-identical guard).
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms/src-tauri && cargo test -p madake-core 2>&1 | grep -E "^test result|FAILED|panicked" | head` — Expected: all `ok`.

- [ ] **Step 5: Spec docs and commit**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check` — Expected: `spec docs up to date; all tests have bilingual spec comments`.

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms
git add src-tauri/crates/madake-core/src/model.rs src-tauri/crates/madake-core/src/io.rs docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(core): コネクタのあるブロックも port_block で保存し、形式を19にする"
```

---

### Task 9: Draft rows, group-name clearing and issues (editor logic)

**Files:**
- Modify: `src/canvas/blockEditor.ts` (imports, `BlockPinRow`, `BlockIssue`, `isFilled`, new `patchRow`, `pinsOf`, `draftFromAttrs`, `draftIssues`)
- Modify: `src/stores/blockEditor.ts` (imports, `updateRow`)
- Test: `src/canvas/blockEditor.test.ts`, `src/stores/blockEditor.test.ts`

**Interfaces:**
- Consumes: TS `connectorGroups`, `isGroupedConnector`, `ConnectorKind` (Task 2).
- Produces:
  - `BlockPinRow { no; name; contacts?; connector?: ConnectorKind | ""; connectorRef?: string }`
  - `BlockIssue` gains `{ code: "connectorRefNotGrouped"; side: BlockSide; index: number }` and `{ code: "connectorRefSplit"; ref: string }`
  - `export function patchRow(row: BlockPinRow, patch: Partial<BlockPinRow>): void`

- [ ] **Step 1: Write the failing tests**

Append at the end of `src/canvas/blockEditor.test.ts`:

```ts
describe("connector columns", () => {
  // ja: コネクタとまとめ名を書いた行は block_pins に connector / connector_ref として入り、読み戻すと同じ属性に戻る
  it("writes the connector and group name into block_pins and reads them back unchanged", () => {
    const d = draft();
    d.rows.left[0].connector = "screw";
    d.rows.left[0].connectorRef = " TB1 ";
    d.rows.left[1].connector = "screw";
    d.rows.left[1].connectorRef = "TB1";
    d.rows.right[0].connector = "usb_a_f";
    const attrs = attrsFromDraft(d);
    const pins = JSON.parse(attrs.block_pins);
    expect(pins.left[0]).toEqual({ no: "IN1", name: "入力1", connector: "screw", connector_ref: "TB1" });
    expect(pins.right[0]).toEqual({ no: "OUT1", name: "出力1", connector: "usb_a_f" });
    expect(draftFromAttrs(attrs).rows.left[0]).toEqual({ no: "IN1", name: "入力1", connector: "screw", connectorRef: "TB1" });
    expect(attrsFromDraft(draftFromAttrs(attrs))).toEqual(attrs);
  });

  // ja: コネクタだけを選んだ行も書き出す行として数える (番号が空なら指摘が出る)
  it("counts a row with only a connector as filled, so a missing number is reported", () => {
    const d = draft();
    d.rows.right.push({ no: "", name: "", connector: "dc_f" });
    expect(draftIssues(d)).toEqual([{ code: "emptyNo", side: "right", index: 1 }]);
  });

  // ja: 端子台・JST 以外のまとめ名と、2つのコネクタに分かれたまとめ名は指摘になる
  it("flags a group name on a kind that cannot have one and a group name split over two connectors", () => {
    const d = draft();
    d.rows.left[0].connector = "usb_a_f";
    d.rows.left[0].connectorRef = "J3";
    d.rows.left[1].connector = "jst_ph_f";
    d.rows.left[1].connectorRef = "J5";
    d.rows.right[0].connector = "jst_ph_f";
    d.rows.right[0].connectorRef = "J5";
    expect(draftIssues(d)).toEqual([
      { code: "connectorRefNotGrouped", side: "left", index: 0 },
      { code: "connectorRefSplit", ref: "J5" },
    ]);
  });

  // ja: 空白だけのまとめ名は書き出さず、指摘にもならない
  it("drops a group name made only of spaces without reporting it", () => {
    const d = draft();
    d.rows.left[0].connector = "jst_ph_f";
    d.rows.left[0].connectorRef = "   ";
    expect(JSON.parse(attrsFromDraft(d).block_pins).left[0]).toEqual({ no: "IN1", name: "入力1", connector: "jst_ph_f" });
    expect(draftIssues(d)).toEqual([]);
    expect(draftSymbol(d)).not.toBeNull();
  });
});
```

Append at the end of `src/stores/blockEditor.test.ts`:

```ts
describe("connector columns in the pin table", () => {
  // ja: コネクタを「なし」かまとめない種類に変えるとまとめ名は消え、端子台・JST のあいだで変えるだけなら残る
  it("clears the group name when the connector becomes none or a kind that cannot be grouped", () => {
    seed([]);
    const editor = useBlockEditorStore();
    editor.startInsert();
    editor.updateRow("left", 0, { connector: "jst_ph_f", connectorRef: "J3" });
    editor.updateRow("left", 0, { connector: "screw" });
    expect(editor.draft?.rows.left[0].connectorRef).toBe("J3");
    editor.updateRow("left", 0, { connector: "usb_a_f" });
    expect(editor.draft?.rows.left[0].connectorRef).toBeUndefined();
    editor.updateRow("left", 0, { connector: "screw", connectorRef: "TB1" });
    editor.updateRow("left", 0, { connector: "" });
    expect(editor.draft?.rows.left[0]).toEqual({ no: "1", name: "", connector: "" });
  });
});
```

- [ ] **Step 2: Run to verify RED**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && npx vitest run src/canvas/blockEditor.test.ts src/stores/blockEditor.test.ts 2>&1 | grep -E "✗|×|FAIL|failed" | head`
Expected: 5 failed: the round trip (`connector` missing from `block_pins`), the connector-only row (no `emptyNo`, the row is not filled), the two issues (empty list), the spaces test (`connector` missing), and the store test (`connectorRef` still `"J3"` after `usb_a_f`).

- [ ] **Step 3: Implement in `blockEditor.ts`**

Replace `import { transformLocal } from "./renderer";` with:

```ts
import { connectorGroups, isGroupedConnector, type ConnectorKind } from "./connector";
import { transformLocal } from "./renderer";
```

Replace

```ts
/** ピン表の1行。`contacts`はカンマ区切りの接点 (空・省略 = 普通のピン)。 */
export interface BlockPinRow {
  no: string;
  name: string;
  contacts?: string;
}
```

with

```ts
/**
 * ピン表の1行。`contacts`はカンマ区切りの接点 (空・省略 = 普通のピン)。
 * `connector`は外形に描くコネクタの形 (空・省略 = 絵なし)、`connectorRef`は端子台・JST のまとめ名。
 */
export interface BlockPinRow {
  no: string;
  name: string;
  contacts?: string;
  connector?: ConnectorKind | "";
  connectorRef?: string;
}
```

Replace

```ts
  /** 接点が多すぎる。 */
  | { code: "tooManyContacts"; side: BlockSide; index: number; count: number; max: number };
```

with

```ts
  /** 接点が多すぎる。 */
  | { code: "tooManyContacts"; side: BlockSide; index: number; count: number; max: number }
  /** まとめ名は端子台・JST にしか付けられない。 */
  | { code: "connectorRefNotGrouped"; side: BlockSide; index: number }
  /** 1つのまとめ名が2つのコネクタに分かれている。 */
  | { code: "connectorRefSplit"; ref: string };
```

Replace

```ts
function isFilled(row: BlockPinRow): boolean {
  return row.no.trim() !== "" || row.name.trim() !== "" || (row.contacts ?? "").trim() !== "";
}
```

with

```ts
function isFilled(row: BlockPinRow): boolean {
  return (
    row.no.trim() !== "" ||
    row.name.trim() !== "" ||
    (row.contacts ?? "").trim() !== "" ||
    (row.connector ?? "") !== "" ||
    (row.connectorRef ?? "").trim() !== ""
  );
}

/** 行を書き換える。コネクタを「なし」かまとめない種類へ変えたら、まとめ名は消す (付けられないため)。 */
export function patchRow(row: BlockPinRow, patch: Partial<BlockPinRow>): void {
  Object.assign(row, patch);
  if ("connector" in patch && !(row.connector && isGroupedConnector(row.connector))) {
    delete row.connectorRef;
  }
}
```

In `pinsOf`, replace

```ts
        const contacts = splitContacts(row.contacts ?? "");
        if (contacts.length) pin.contacts = contacts;
        return pin;
```

with

```ts
        const contacts = splitContacts(row.contacts ?? "");
        if (contacts.length) pin.contacts = contacts;
        if (row.connector) pin.connector = row.connector;
        const ref = (row.connectorRef ?? "").trim();
        if (ref) pin.connector_ref = ref;
        return pin;
```

In `draftFromAttrs`, replace

```ts
      ...(p.contacts?.length ? { contacts: p.contacts.join(", ") } : {}),
    }));
```

with

```ts
      ...(p.contacts?.length ? { contacts: p.contacts.join(", ") } : {}),
      ...(p.connector ? { connector: p.connector } : {}),
      ...(p.connector_ref ? { connectorRef: p.connector_ref } : {}),
    }));
```

In `draftIssues`, replace

```ts
        const dup = contacts.find((c, i) => contacts.indexOf(c) !== i);
        if (dup) issues.push({ code: "duplicateContact", side, index, contact: dup });
      }
    });
  }
```

with

```ts
        const dup = contacts.find((c, i) => contacts.indexOf(c) !== i);
        if (dup) issues.push({ code: "duplicateContact", side, index, contact: dup });
      }
      if ((row.connectorRef ?? "").trim() !== "" && !(row.connector && isGroupedConnector(row.connector))) {
        issues.push({ code: "connectorRefNotGrouped", side, index });
      }
    });
  }
  // 1つのまとめ名は1つのまとまり (同じ辺の隣り合うピン、同じ種類) だけ (Rust側 check_connectors の最後の決まり)
  const pins = pinsOf(draft);
  const seenRefs = new Set<string>();
  for (const side of BLOCK_SIDES) {
    for (const group of connectorGroups(pins[side] ?? [])) {
      if (group.ref === null || !isGroupedConnector(group.kind)) continue;
      if (seenRefs.has(group.ref)) issues.push({ code: "connectorRefSplit", ref: group.ref });
      seenRefs.add(group.ref);
    }
  }
```

and, further down in the same function, replace

```ts
  if (draft.size) {
    const pins = pinsOf(draft);
    const rows
```

with

```ts
  if (draft.size) {
    const rows
```

(the `pins` computed above is reused).

In `src/stores/blockEditor.ts`, add `patchRow,` to the import list from `"../canvas/blockEditor"` (between `moveRow,` and `removeRow,`), and replace

```ts
      if (row) Object.assign(row, patch);
```

with

```ts
      if (row) patchRow(row, patch);
```

- [ ] **Step 4: Run to verify GREEN**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && npx vitest run src/canvas/blockEditor.test.ts src/stores/blockEditor.test.ts src/canvas/dynamicSymbol.test.ts 2>&1 | tail -5` — Expected: all passed.
Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && npx vue-tsc --noEmit 2>&1 | grep -v "src/chat/markdown" | head`
Expected: no output (`issueText` ends with a fall-through `return`, so the new issue codes type-check before Task 10 adds their texts).

- [ ] **Step 5: Spec docs and commit**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check` — Expected: `spec docs up to date; all tests have bilingual spec comments`.

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms
git add src/canvas/blockEditor.ts src/canvas/blockEditor.test.ts src/stores/blockEditor.ts src/stores/blockEditor.test.ts docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(ui): ピン表の下書きでコネクタとまとめ名を扱い、誤りを指摘する"
```

---

### Task 10: Editor columns, issue texts and six locales

**Files:**
- Modify: `src/components/BlockPinColumn.vue`, `src/components/BlockPinsEditor.vue` (`issueText`), `src/components/BlockEditorDialog.vue` (`.dialog` width), `src/components/SymbolPickerDialog.vue` (`.dialog.with-pins` width)
- Modify: `src/locales/{en,ja,zh,fr,de,es}.json`
- Test: `src/i18n/i18n.test.ts`

**Interfaces:**
- Consumes: `CONNECTOR_KINDS`, `isGroupedConnector`, `ConnectorKind` (Task 2); `BlockPinRow`, issue codes (Task 9).
- Produces: locale keys `blockEditor.colConnector`, `blockEditor.colConnectorRef`, `blockEditor.connectorRefPlaceholder`, `blockEditor.connectorNone`, `blockEditor.connector.<kind>` × 13, `blockEditor.issue.connectorRefNotGrouped` (`{side}`, `{row}`), `blockEditor.issue.connectorRefSplit` (`{ref}`).

- [ ] **Step 1: Write the failing test** (in `src/i18n/i18n.test.ts`)

After `import de from "../locales/de.json";` add:

```ts
import { CONNECTOR_KINDS } from "../canvas/connector";
```

Directly above `  // ja: 言語名は各言語の自称で表示される(英語に翻訳しない)` add:

```ts
  // ja: コネクタの13種類はどれも全ロケールに表示名がある (ピン表エディタの選択肢)
  it("names every connector kind in every locale", () => {
    for (const [locale, catalog] of Object.entries(CATALOGS)) {
      const names = ((catalog.blockEditor as Record<string, unknown>).connector ?? {}) as Record<string, unknown>;
      for (const kind of CONNECTOR_KINDS) {
        expect(typeof names[kind], `${locale}: ${kind}`).toBe("string");
      }
    }
  });

```

- [ ] **Step 2: Run to verify RED**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && npx vitest run src/i18n/i18n.test.ts 2>&1 | grep -E "en: usb_a_f|failed" | head -3`
Expected: 1 failed, message `en: usb_a_f` (expected `"undefined"` to be `"string"`).

- [ ] **Step 3: Add the locale keys**

The locale files round-trip exactly through `json.dumps(…, ensure_ascii=False, indent=2) + "\n"` (checked), so a script keeps their formatting. Run:

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && python3 - <<'EOF'
import json

KINDS = ["usb_a_f", "usb_a_m", "usb_c_f", "usb_c_m", "micro_b_f", "micro_b_m", "jack35_f",
         "jack35_m", "dc_f", "dc_m", "screw", "jst_ph_f", "jst_xh_f"]
T = {
    "en": {
        "colConnector": "Connector", "colConnectorRef": "Group name",
        "connectorRefPlaceholder": "e.g. J3, TB1", "connectorNone": "None",
        "kinds": ["USB-A socket", "USB-A plug", "USB-C socket", "USB-C plug", "micro-B socket",
                  "micro-B plug", "3.5 mm jack", "3.5 mm plug", "DC jack", "DC plug",
                  "Screw terminals", "JST PH socket", "JST XH socket"],
        "connectorRefNotGrouped": "Row {row} of the {side} has a group name: only screw terminals and JST sockets can have one",
        "connectorRefSplit": "Group name {ref} is used by two connectors: put its pins next to each other on one side, all of one kind",
    },
    "ja": {
        "colConnector": "コネクタ", "colConnectorRef": "まとめ名",
        "connectorRefPlaceholder": "例: J3, TB1", "connectorNone": "なし",
        "kinds": ["USB-A メス", "USB-A オス", "USB-C メス", "USB-C オス", "micro-B メス",
                  "micro-B オス", "3.5mm ジャック", "3.5mm プラグ", "DC ジャック", "DC プラグ",
                  "端子台 (ネジ)", "JST PH メス", "JST XH メス"],
        "connectorRefNotGrouped": "{side}の{row}行目にまとめ名があります。まとめ名を付けられるのは端子台と JST だけです",
        "connectorRefSplit": "まとめ名 {ref} が2つのコネクタに使われています。同じ辺に隣り合わせて並べ、種類をそろえてください",
    },
    "zh": {
        "colConnector": "连接器", "colConnectorRef": "组名",
        "connectorRefPlaceholder": "例如 J3, TB1", "connectorNone": "无",
        "kinds": ["USB-A 母座", "USB-A 公头", "USB-C 母座", "USB-C 公头", "micro-B 母座",
                  "micro-B 公头", "3.5 mm 插孔", "3.5 mm 插头", "DC 插孔", "DC 插头",
                  "螺钉端子", "JST PH 插座", "JST XH 插座"],
        "connectorRefNotGrouped": "{side}第{row}行有组名：只有螺钉端子和 JST 插座可以有组名",
        "connectorRefSplit": "组名 {ref} 被两个连接器使用：请把它的引脚放在同一边并相邻，且类型相同",
    },
    "fr": {
        "colConnector": "Connecteur", "colConnectorRef": "Nom du groupe",
        "connectorRefPlaceholder": "p. ex. J3, TB1", "connectorNone": "Aucun",
        "kinds": ["Prise USB-A", "Fiche USB-A", "Prise USB-C", "Fiche USB-C", "Prise micro-B",
                  "Fiche micro-B", "Jack 3,5 mm", "Fiche 3,5 mm", "Prise DC", "Fiche DC",
                  "Bornier à vis", "Embase JST PH", "Embase JST XH"],
        "connectorRefNotGrouped": "La ligne {row} du {side} a un nom de groupe : seuls les borniers à vis et les embases JST peuvent en avoir",
        "connectorRefSplit": "Le nom de groupe {ref} sert à deux connecteurs : placez ses broches côte à côte sur un même côté, toutes du même type",
    },
    "de": {
        "colConnector": "Steckverbinder", "colConnectorRef": "Gruppenname",
        "connectorRefPlaceholder": "z. B. J3, TB1", "connectorNone": "Keiner",
        "kinds": ["USB-A-Buchse", "USB-A-Stecker", "USB-C-Buchse", "USB-C-Stecker", "Micro-B-Buchse",
                  "Micro-B-Stecker", "3,5-mm-Buchse", "3,5-mm-Stecker", "DC-Buchse", "DC-Stecker",
                  "Schraubklemmen", "JST-PH-Buchse", "JST-XH-Buchse"],
        "connectorRefNotGrouped": "Zeile {row} der {side} hat einen Gruppennamen: nur Schraubklemmen und JST-Buchsen können einen haben",
        "connectorRefSplit": "Der Gruppenname {ref} wird von zwei Steckverbindern verwendet: die Pins müssen nebeneinander auf einer Seite liegen und von einer Art sein",
    },
    "es": {
        "colConnector": "Conector", "colConnectorRef": "Nombre del grupo",
        "connectorRefPlaceholder": "p. ej. J3, TB1", "connectorNone": "Ninguno",
        "kinds": ["USB-A hembra", "USB-A macho", "USB-C hembra", "USB-C macho", "Micro-B hembra",
                  "Micro-B macho", "Jack de 3,5 mm", "Clavija de 3,5 mm", "Jack DC", "Clavija DC",
                  "Bornes de tornillo", "Zócalo JST PH", "Zócalo JST XH"],
        "connectorRefNotGrouped": "La fila {row} del {side} tiene un nombre de grupo: solo los bornes de tornillo y los zócalos JST pueden tenerlo",
        "connectorRefSplit": "El nombre de grupo {ref} lo usan dos conectores: coloque sus pines juntos en un mismo lado y del mismo tipo",
    },
}

for loc, t in T.items():
    path = f"src/locales/{loc}.json"
    data = json.load(open(path, encoding="utf-8"))
    new = {}
    for key, value in data["blockEditor"].items():
        new[key] = value
        if key == "colContacts":
            new["colConnector"] = t["colConnector"]
            new["colConnectorRef"] = t["colConnectorRef"]
        if key == "contactsPlaceholder":
            new["connectorRefPlaceholder"] = t["connectorRefPlaceholder"]
            new["connectorNone"] = t["connectorNone"]
            new["connector"] = dict(zip(KINDS, t["kinds"]))
    new["issue"]["connectorRefNotGrouped"] = t["connectorRefNotGrouped"]
    new["issue"]["connectorRefSplit"] = t["connectorRefSplit"]
    data["blockEditor"] = new
    with open(path, "w", encoding="utf-8") as f:
        f.write(json.dumps(data, ensure_ascii=False, indent=2) + "\n")
    print(f"{loc}: blockEditor has {len(new)} keys")
EOF
```

Expected: six lines `…: blockEditor has 42 keys`. Then `git diff --stat src/locales` shows only additions in the six files.

- [ ] **Step 4: Columns in `BlockPinColumn.vue`**

Replace

```ts
import type { BlockSide } from "../canvas/blockEditor";
import { useBlockEditorStore } from "../stores/blockEditor";
```

with

```ts
import type { BlockPinRow, BlockSide } from "../canvas/blockEditor";
import { CONNECTOR_KINDS, isGroupedConnector, type ConnectorKind } from "../canvas/connector";
import { useBlockEditorStore } from "../stores/blockEditor";
```

Replace

```ts
function edit(index: number, patch: { no?: string; name?: string; contacts?: string }) {
  editor.updateRow(props.side, index, patch);
}
```

with

```ts
function edit(index: number, patch: Partial<BlockPinRow>) {
  editor.updateRow(props.side, index, patch);
}

/** まとめ名を書けるのは端子台・JST の行だけ。 */
function refEnabled(row: BlockPinRow): boolean {
  return !!row.connector && isGroupedConnector(row.connector);
}
```

Replace

```html
      <span class="lbl">{{ t("blockEditor.colContacts") }}</span>
    </div>
```

with

```html
      <span class="lbl">{{ t("blockEditor.colContacts") }}</span>
      <span class="lbl">{{ t("blockEditor.colConnector") }}</span>
      <span class="lbl ref">{{ t("blockEditor.colConnectorRef") }}</span>
    </div>
```

Replace

```html
        @input="edit(i, { contacts: ($event.target as HTMLInputElement).value })"
      />
```

with

```html
        @input="edit(i, { contacts: ($event.target as HTMLInputElement).value })"
      />
      <select
        class="cell"
        :value="row.connector ?? ''"
        :aria-label="t('blockEditor.colConnector')"
        @change="edit(i, { connector: ($event.target as HTMLSelectElement).value as ConnectorKind | '' })"
      >
        <option value="">{{ t("blockEditor.connectorNone") }}</option>
        <option v-for="kind in CONNECTOR_KINDS" :key="kind" :value="kind">
          {{ t(`blockEditor.connector.${kind}`) }}
        </option>
      </select>
      <input
        class="cell ref mono"
        :value="row.connectorRef ?? ''"
        :aria-label="t('blockEditor.colConnectorRef')"
        :placeholder="t('blockEditor.connectorRefPlaceholder')"
        :disabled="!refEnabled(row)"
        spellcheck="false"
        @input="edit(i, { connectorRef: ($event.target as HTMLInputElement).value })"
      />
```

In `<style scoped>`, replace `.lbl.no { flex: none; width: 58px; }` with

```css
.lbl.no { flex: none; width: 58px; }
.lbl.ref { flex: none; width: 52px; }
```

and replace `.cell.mono { flex: none; width: 58px; font-family: var(--mono-font); }` with

```css
.cell.mono { flex: none; width: 58px; font-family: var(--mono-font); }
.cell.ref { width: 52px; }
.cell:disabled { opacity: 0.5; }
```

- [ ] **Step 5: Issue texts and dialog widths**

In `BlockPinsEditor.vue` `issueText`, replace

```ts
  return t("blockEditor.issue.sizeTooSmall", { w: issue.w, h: issue.h });
```

with

```ts
  if (issue.code === "connectorRefNotGrouped") {
    return t("blockEditor.issue.connectorRefNotGrouped", {
      side: t(`blockEditor.side.${issue.side}`),
      row: issue.index + 1,
    });
  }
  if (issue.code === "connectorRefSplit") return t("blockEditor.issue.connectorRefSplit", { ref: issue.ref });
  return t("blockEditor.issue.sizeTooSmall", { w: issue.w, h: issue.h });
```

In `BlockEditorDialog.vue` `<style>`, inside `.dialog {`, replace `width: min(760px, 96vw);` with `width: min(1020px, 96vw);`.
In `SymbolPickerDialog.vue` `<style>`, inside `.dialog.with-pins {`, replace `width: min(820px, 96vw);` with `width: min(1080px, 96vw);` (each side column gains a select and a 52 px cell).

- [ ] **Step 6: Run to verify GREEN**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && npx vitest run src/i18n src/canvas src/stores 2>&1 | tail -5` — Expected: all passed (re-run `src/stores/terminals.test.ts` alone if it times out).
Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && npx vue-tsc --noEmit 2>&1 | grep -v "src/chat/markdown" | head` — Expected: no output.

- [ ] **Step 7: Spec docs and commit**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check` — Expected: `spec docs up to date; all tests have bilingual spec comments`.

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms
git add src/components/BlockPinColumn.vue src/components/BlockPinsEditor.vue src/components/BlockEditorDialog.vue src/components/SymbolPickerDialog.vue src/locales/en.json src/locales/ja.json src/locales/zh.json src/locales/fr.json src/locales/de.json src/locales/es.json src/i18n/i18n.test.ts docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(ui): ピン表エディタにコネクタとまとめ名の列を足す"
```

---

### Task 11: MCP descriptions and the agent rule

**Files:**
- Modify: `src-tauri/crates/madake-mcp/src/lib.rs` (`PlaceSymbolParams::attrs` doc, `place_symbol` description)
- Modify: `src-tauri/crates/madake-mcp/tests/drawing_tools.rs`
- Modify: `src-tauri/crates/madake-agent/src/knowledge.rs`, `src-tauri/crates/madake-agent/tests/knowledge.rs`

**Interfaces:**
- Consumes: the core behaviour of Tasks 1–8.
- Produces: text only.

- [ ] **Step 1: Write the tests**

Append at the end of `src-tauri/crates/madake-mcp/tests/drawing_tools.rs`:

```rust
// ---- コネクタの絵 (ブロックのピンの connector) ----

/// Regression test for Tasks 1-8 (already passes): place_symbol with a connector on a block pin returns that pin 5 mm from the box edge, and the sheet image of such a block renders.
/// Tasks 1-8 の挙動の回帰テスト (最初から通る): ブロックのピンに connector を付けた place_symbol は、そのピンを外形から5mmの位置で返し、そのブロックのシート画像も描ける。
#[tokio::test]
async fn a_connector_pin_is_placed_five_millimetres_from_the_box() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    // 右辺1本・名前なし: 外形15×10、右辺は x = 107.5、接続点は 112.5
    let placed = place_block(&bridge, sheet, "39", 100.0, 100.0, r#"{"right":[{"no":"U1","connector":"usb_a_f","contacts":["VBUS","D-","D+","GND"]}]}"#).await;
    assert_eq!(placed["pins"][0]["x"], 112.5);
    let image = madake_mcp::sheet_image(&doc, None, None, None).expect("画像");
    assert_eq!(&image.png[..4], b"\x89PNG");
}

/// Regression test for Tasks 1-8 (already passes): place_symbol refuses an unknown connector kind and a connector name on a USB socket, with the reason.
/// Tasks 1-8 の挙動の回帰テスト (最初から通る): place_symbol は知らないコネクタの種類と、USB の口に付けたコネクタの名前を理由付きで断る。
#[tokio::test]
async fn place_symbol_refuses_a_bad_connector() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    let place = |pins: &str| json!({"sheet_id": sheet, "symbol_id": "block", "x": 50.0, "y": 50.0, "reference": "U1", "attrs": {"block_pins": pins}});
    let err = call_err(&bridge, "place_symbol", place(r#"{"left":[{"no":"1","connector":"usb_z"}]}"#)).await;
    assert!(err.contains("not valid JSON") && err.contains("unknown variant"), "{err}");
    let err = call_err(&bridge, "place_symbol", place(r#"{"left":[{"no":"1","connector":"usb_a_f","connector_ref":"J3"}]}"#)).await;
    assert!(err.contains("connector_ref is only for screw, jst_ph_f and jst_xh_f, not usb_a_f"), "{err}");
}

/// place_symbol tells the agent which connector kinds exist, how to group terminal blocks and JST sockets with connector_ref, and to read pin positions with get_pins, both in its description and in the attrs schema.
/// place_symbol は説明と attrs のスキーマの両方で、コネクタの種類、端子台・JST を connector_ref でまとめること、ピンの位置を get_pins で読むことをエージェントに伝える。
#[test]
fn place_symbol_describes_connectors() {
    let tools = tool_descriptions();
    let (_, d) = tools.iter().find(|(n, _)| n == "place_symbol").expect("place_symbol");
    for needle in ["connector_ref", "usb_a_f", "jst_xh_f", "get_pins", "5mm"] {
        assert!(d.contains(needle), "説明に「{needle}」が無い: {d}");
    }
    let schema = serde_json::to_string(&schemars::schema_for!(madake_mcp::PlaceSymbolParams)).unwrap();
    assert!(schema.contains("connector_ref") && schema.contains("jst_xh_f"), "attrs の説明にコネクタが無い");
}
```

Append at the end of `src-tauri/crates/madake-agent/tests/knowledge.rs`:

```rust
/// The system prompt tells the agent to give real connector pins their connector kind, to group terminal blocks and JST sockets with one connector_ref, to keep conductors in contacts and to re-read pin positions afterwards.
/// システムプロンプトは、実物のコネクタのピンに connector の種類を付け、端子台・JST は1つの connector_ref でまとめ、導体は contacts に書き、付けた後にピンの位置を読み直すようエージェントに伝える。
#[test]
fn system_prompt_explains_connector_pictures() {
    let prompt = system_prompt(&AppSettings::default(), None);
    for needle in ["`connector`", "`connector_ref`", "`jst_ph_f`", "`_f`(メス)", "`get_pins`で読み直す"] {
        assert!(prompt.contains(needle), "「{needle}」が無い: {prompt}");
    }
}
```

- [ ] **Step 2: Run them** (one filter per run)

- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-mcp --test drawing_tools a_connector_pin_is_placed_five_millimetres_from_the_box 2>&1 | grep "test result"` — Expected: `ok` (regression).
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms/src-tauri && cargo test -p madake-mcp --test drawing_tools place_symbol_refuses_a_bad_connector 2>&1 | grep "test result"` — Expected: `ok` (regression).
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms/src-tauri && cargo test -p madake-mcp --test drawing_tools place_symbol_describes_connectors 2>&1 | grep panicked -A1 | head -2` — Expected: FAIL, `説明に「connector_ref」が無い`.
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms/src-tauri && cargo test -p madake-agent --test knowledge system_prompt_explains_connector_pictures 2>&1 | grep panicked -A1 | head -2` — Expected: FAIL, `「`connector`」が無い`.

- [ ] **Step 3: Edit the MCP text** (`madake-mcp/src/lib.rs`)

In `PlaceSymbolParams`, replace

```rust
    /// (ネットリストは接点ごとに分かれる)。接点は1ピンに2〜32個、前後の空白は除き、ピンの中で重ならない。
```

with

```rust
    /// (ネットリストは接点ごとに分かれる)。接点は1ピンに2〜32個、前後の空白は除き、ピンの中で重ならない。
    /// ピンに `connector` (`usb_a_f` `usb_a_m` `usb_c_f` `usb_c_m` `micro_b_f` `micro_b_m` `jack35_f` `jack35_m`
    /// `dc_f` `dc_m` `screw` `jst_ph_f` `jst_xh_f`。`_f` = メス、`_m` = オス) を書くと外形の辺にそのコネクタの絵を描く
    /// (絵だけで、導体は `contacts` に書く)。続けて並ぶ `screw` / `jst_*` のピンに同じ `connector_ref` (例 `"J3"`) を
    /// 書くと1つの本体になる。コネクタのあるピンの接続点は外形から5mm (位置は `get_pins` で読む)。
```

In the `place_symbol` tool attribute, replace the end of the description

```rust
(接点は1ピンに2〜32個、前後の空白は除き、ピンの中で重ならない)"
    )]
    fn place_symbol(
```

with

```rust
(接点は1ピンに2〜32個、前後の空白は除き、ピンの中で重ならない)。ピンに connector (usb_a_f usb_a_m usb_c_f usb_c_m micro_b_f micro_b_m jack35_f jack35_m dc_f dc_m screw jst_ph_f jst_xh_f。_f=メス、_m=オス) を付けると外形の辺にそのコネクタの絵を描く (絵だけで、導体は contacts に書く)。続けて並ぶ screw / jst_ph_f / jst_xh_f のピンに同じ connector_ref (例 J3) を付けると1つの本体になる。コネクタのあるピンの接続点は外形から5mmなので、位置は get_pins で読む"
    )]
    fn place_symbol(
```

(The description is one string on one line; only its tail changes.)

- [ ] **Step 4: Edit the agent rule** (`madake-agent/src/knowledge.rs`)

Replace

```text
(芯は同じ名前の接点に対応。どれも一致しなければ芯と接点が同数のときだけ並び順。名前の一部だけの一致と`style: multi`は断られる)。導体1本のピンは接点と同じ名前(`VBUS`・`GND`)にすればその接点に繋がる
```

with

```text
(芯は同じ名前の接点に対応。どれも一致しなければ芯と接点が同数のときだけ並び順。名前の一部だけの一致と`style: multi`は断られる)。導体1本のピンは接点と同じ名前(`VBUS`・`GND`)にすればその接点に繋がる
- **実物のコネクタであるブロックのピンには`connector`の種類を付ける**\
(USB・3.5mm・DC: 機器側の口は`_f`(メス)、プラグは`_m`(オス)。例 `{\"no\":\"U1\",\"connector\":\"usb_a_f\",\"contacts\":[\"VBUS\",\"D-\",\"D+\",\"GND\"]}`)。\
端子台・JSTのソケットは隣り合うピンに`screw`/`jst_ph_f`/`jst_xh_f`と同じ`connector_ref`(例 `J3`)を付けて並べる。\
`connector`は絵だけなので導体は`contacts`に書く。コネクタのあるピンの接続点は外形から5mmなので、付けたらピンの位置は`get_pins`で読み直す
```

(This text is inside a normal `"…"` Rust string: keep the `\"` escapes and the trailing `\` line continuations exactly.)

- [ ] **Step 5: Run to verify GREEN**

- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-mcp --test drawing_tools 2>&1 | grep -E "FAILED|test result"` — Expected: `ok`, 0 failed.
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms/src-tauri && cargo test -p madake-agent --test knowledge 2>&1 | grep -E "FAILED|test result"` — Expected: `ok`, 0 failed.
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms/src-tauri && cargo test -p madake-mcp 2>&1 | grep -E "^test result|FAILED|panicked" | head` — Expected: all `ok`.

- [ ] **Step 6: Spec docs and commit**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check` — Expected: `spec docs up to date; all tests have bilingual spec comments`.

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms
git add src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-mcp/tests/drawing_tools.rs src-tauri/crates/madake-agent/src/knowledge.rs src-tauri/crates/madake-agent/tests/knowledge.rs docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(mcp): place_symbol とエージェントにコネクタの絵の書き方を伝える"
```

---

### Task 12: Per-shape block definitions in DXF (export key, export, import)

**Files:**
- Modify: `src-tauri/crates/madake-core/src/block.rs` (`block_export_id`, `is_block_export_id`, `export_def`, `imported_block_attrs`, test fixture `export_blocks_sheet`, one test)
- Modify: `src-tauri/crates/madake-core/src/dxf.rs` (`sheet_to_dxf_with_frame`, `symbol_id_of_block`, `import_dxf` INSERT branch, three tests)
- Modify: `docs/13-specification.md`, `docs/13-specification.ja.md` (generated)

**Interfaces:**
- Produces (`block.rs`, used by Task 13):
  - `pub fn block_export_id(def: &SymbolDef) -> String` — `"block"` when primitives + pins equal `block_template()`'s, else `"block_" + {:016x}` of FNV-1a 64 over `serde_json::to_string(&(&def.primitives, &def.pins))` (spec §3.1).
  - `pub fn is_block_export_id(id: &str) -> bool` — `block`, or `block_` + exactly 16 of `0-9a-f`.
  - `pub fn export_def<'a>(inst: &SymbolInstance, index: &crate::symbol::DefIndex<'a>) -> Option<Cow<'a, SymbolDef>>` — `index.get(inst)`; for `symbol_id == BLOCK_SYMBOL_ID` an owned clone whose `id` is `block_export_id`; library symbols borrowed unchanged.
  - `pub fn imported_block_attrs(pins: Option<&str>, size: Option<&str>) -> Option<BTreeMap<String, String>>` — the non-empty values under `block_pins` / `block_size`; `None` when `block_symbol` refuses them.
  - `#[cfg(test)] pub(crate) fn export_blocks_sheet() -> Sheet` — next to `test_block`.
- Consumes: `symbol::DefIndex` (exists), `block::test_block` (exists).

- [ ] **Step 1: Write the failing tests**

In `block.rs`, next to `test_block` (same `#[cfg(test)]`):

```rust
#[cfg(test)]
pub(crate) fn export_blocks_sheet() -> crate::model::Sheet {
    use crate::model::{Orientation, PaperSize, Sheet};
    let mut sheet = Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape);
    for e in [
        test_block("B0", "", 40.0, 40.0, "{}"),
        test_block("B1", "", 80.0, 40.0, r#"{"left":[{"no":"1","name":"IN"}]}"#),
        test_block("B2", "", 120.0, 40.0, r#"{ "left": [ {"no": "1", "name": "IN"} ] }"#),
        test_block(
            "B8", "", 160.0, 40.0,
            r#"{"left":[{"no":"1"},{"no":"2"},{"no":"3"},{"no":"4"}],"right":[{"no":"5"},{"no":"6"},{"no":"7"},{"no":"8","name":"OUT"}]}"#,
        ),
        test_block("BU", "", 200.0, 40.0, r#"{"left":[{"no":"1","name":"USB","connector":"usb_a_m"}]}"#),
    ] {
        sheet.entities.insert(e.id(), e);
    }
    sheet
}
```

(If `"{}"` is refused by `parse_block_spec`, give `B0` no attributes instead; the test only needs one block equal to the template.)

In `block::tests`, test `block_export_id_follows_the_drawn_shape_only`: (a) `block_export_id(&block_template()) == "block"`; (b) `B1`'s and `B2`'s tables (different whitespace) give the same id; (c) these each give a different id from `B1`'s and from each other: name `IN` → `IN2`, an extra pin `2`, `"connector":"usb_a_f"`, and `B1`'s table plus `block_size` `{"w":30,"h":20}`; (d) every id from (b)/(c) passes `is_block_export_id`; (e) `is_block_export_id` is false for `block_x`, `block_` + 15 hex, `block_` + 17 hex, `block_` + 16 hex with an upper-case `A`, and `relay_coil`.
RED: does not compile (`cannot find function block_export_id`).

In `dxf::tests`:
- `each_block_shape_is_its_own_dxf_block`: `sheet_to_dxf(&sheet, &defs(&sheet), &[])` of `crate::block::export_blocks_sheet()`. BLOCK names (as in `symbols_become_block_inserts_with_attributes`) contain `MDK_block` once and exactly three names `MDK_block_<16 hex>`; with `entity_records`, the INSERTs of `B1` and `B2` (find by the following `TAG1` ATTRIB) name the same block, `B8` and `BU` two others, `B0` names `MDK_block`; the records between the `BLOCK` of `BU`'s name and its `ENDBLK` include a `SOLID`; `B8`'s ATTRIBs include `TERM01` … `TERM08`.
  RED: `assert` on the three `MDK_block_` names fails (today only `MDK_block`).
- `our_block_shapes_import_back_as_blocks_with_their_pin_table`: export as above, `import_dxf(&dxf, "往復", &DxfImportOptions::default())`; `report.skipped` empty, `report.symbols == 5`, every symbol has `symbol_id == "block"`, and `B8`'s `attrs["block_pins"]` equals the fixture's text; no key `BLOCK_PINS` remains.
  RED: `report.skipped` lists `MDK_block_… x4` (`symbol_id_of_block` cannot resolve them).
- `an_unreadable_block_pin_table_imports_as_the_template_with_a_warning`: export, then replace the encoded value of `B1`'s table in the text (`encode_text(r#"{"left":[{"no":"1","name":"IN"}]}"#)`, unique in the file) by `not json`; import; `B1` is `block` with no `block_pins` attribute and `report.warnings` has one entry containing `B1` and `見本の箱`.
  RED: `B1` is missing (skipped).

Each test has the two `///` lines (EN, JA) above `#[test]`.

- [ ] **Step 2: Run to verify RED**

Run (one at a time):
`cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core each_block_shape_is_its_own_dxf_block 2>&1 | grep -E "error\[|panicked|test result"` — Expected: `error[E0425]` for `block_export_id` / `export_blocks_sheet` until Step 3 adds them. After adding only the `block.rs` functions (Step 3 part 1), re-run: Expected `panicked` on the `MDK_block_` count. Same for the other two DXF test names (panicked on `skipped` / missing `B1`).

- [ ] **Step 3: Implement**

Part 1, `block.rs` (non-obvious, so in full):

```rust
fn shape_key(def: &SymbolDef) -> String {
    serde_json::to_string(&(&def.primitives, &def.pins)).expect("SymbolDef serialises")
}

/// Name of a block's drawn shape in DXF / KiCad export (spec §3.1).
pub fn block_export_id(def: &SymbolDef) -> String {
    let key = shape_key(def);
    if key == shape_key(&block_template()) {
        return BLOCK_SYMBOL_ID.to_string();
    }
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in key.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{BLOCK_SYMBOL_ID}_{h:016x}")
}

pub fn is_block_export_id(id: &str) -> bool {
    id == BLOCK_SYMBOL_ID
        || id.strip_prefix("block_").is_some_and(|h| {
            h.len() == 16 && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        })
}

pub fn export_def<'a>(
    inst: &crate::model::SymbolInstance,
    index: &crate::symbol::DefIndex<'a>,
) -> Option<std::borrow::Cow<'a, SymbolDef>> {
    let def = index.get(inst)?;
    if inst.symbol_id != BLOCK_SYMBOL_ID {
        return Some(def);
    }
    let mut def = def.into_owned();
    def.id = block_export_id(&def);
    Some(std::borrow::Cow::Owned(def))
}

pub fn imported_block_attrs(pins: Option<&str>, size: Option<&str>) -> Option<BTreeMap<String, String>> {
    let mut attrs = BTreeMap::new();
    for (k, v) in [(BLOCK_PINS_ATTR, pins), (BLOCK_SIZE_ATTR, size)] {
        if let Some(v) = v.filter(|v| !v.is_empty()) {
            attrs.insert(k.to_string(), v.to_string());
        }
    }
    block_symbol(&attrs).ok().map(|_| attrs)
}
```

Give each `pub fn` a short doc comment in the file's language (Japanese), as the neighbours have.

Part 2, `dxf.rs` `sheet_to_dxf_with_frame`:
- Replace the `defs` / `used` collection (`let defs: BTreeMap<&str, &SymbolDef> = …` and the loop that fills `used`) with `let index = crate::symbol::DefIndex::new(symbols);` and `let mut used: BTreeMap<String, std::borrow::Cow<SymbolDef>> = BTreeMap::new();`, filled by `if let Some(def) = crate::block::export_def(inst, &index) { used.entry(def.id.clone()).or_insert(def); }`.
- The BLOCKS loop `for (id, def) in &used` keeps its body (`def` is now a `Cow`; pass `&**def` / `def` where a `&SymbolDef` is needed).
- The INSERT loop: `let Some(def) = crate::block::export_def(inst, &index) else { … unresolved … };` and the block name `format!("{BLOCK_PREFIX}{}", def.id)` instead of `inst.symbol_id`. Pass `&def` on to `symbol_top_y` / `instance_attributes`.
- Any other `defs.get(` in the function: same replacement (`grep -n "defs.get" src-tauri/crates/madake-core/src/dxf.rs` must print nothing afterwards).

Part 3, `dxf.rs` import:
- `symbol_id_of_block`: after `strip_prefix`, `if crate::block::is_block_export_id(id) { return Some(crate::block::BLOCK_SYMBOL_ID.to_string()); }`.
- In the `INSERT` branch, after the attribute loop and before `push(… Entity::Symbol …)`: when `symbol_id == BLOCK_SYMBOL_ID`, `let pins = attrs.remove("BLOCK_PINS"); let size = attrs.remove("BLOCK_SIZE");` then `match crate::block::imported_block_attrs(pins.as_deref(), size.as_deref())` — `Some(a)` → `attrs.extend(a)`; `None` → `report.warnings.push(format!("{reference}: ブロックのピン表を読めないので見本の箱にしました"))`.

- [ ] **Step 4: Run to verify GREEN**

- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core block_export_id_follows_the_drawn_shape_only 2>&1 | grep "test result"` — Expected: `ok. 1 passed`.
- Same command with `each_block_shape_is_its_own_dxf_block`, then `our_block_shapes_import_back_as_blocks_with_their_pin_table`, then `an_unreadable_block_pin_table_imports_as_the_template_with_a_warning` — Expected: `ok. 1 passed` each.
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms/src-tauri && cargo test -p madake-core dxf:: 2>&1 | grep -E "FAILED|test result"` — Expected: `ok`, 0 failed (existing DXF tests, incl. `export_then_import_round_trips_the_sheet`, unchanged).

- [ ] **Step 5: Spec docs and commit**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check` — Expected: `spec docs up to date; all tests have bilingual spec comments`.

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms
git add src-tauri/crates/madake-core/src/block.rs src-tauri/crates/madake-core/src/dxf.rs docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(dxf): ブロックを形ごとの定義で書き出し、読み戻す"
```

---

### Task 13: Per-shape block symbols in KiCad (export, import)

**Files:**
- Modify: `src-tauri/crates/madake-core/src/kicad.rs` (`export_kicad_sch`, `import_kicad_sch`, two tests)
- Modify: `docs/13-specification.md`, `docs/13-specification.ja.md` (generated)

**Interfaces:**
- Consumes: `block::{export_def, is_block_export_id, imported_block_attrs, export_blocks_sheet, BLOCK_SYMBOL_ID}` (Task 12), `symbol::DefIndex`.
- Produces: no new API. `write_lib_symbol` is unchanged (it already names the entry and its units from `def.id` and writes pins and fills).

- [ ] **Step 1: Write the failing tests**

In `kicad::tests`:
- `each_block_shape_is_its_own_kicad_lib_symbol`: `export_kicad_sch(&sheet, &defs(&sheet))` of `crate::block::export_blocks_sheet()`; parse with `parse_sexpr`. `lib_symbols` names (as in `used_symbols_are_embedded_in_lib_symbols`) are `MadakeCAD:block` plus exactly three `MadakeCAD:block_<16 hex>`; the entry used by `B8` has 8 `pin`s in its units, numbers `1`…`8`, one with name `OUT`; the entry used by `BU` has a `rectangle` whose `fill` `type` is `outline`; top-level `symbol`s: `B1` and `B2` share one `lib_id`, `B8` has 8 `(pin …)` children, `B0` has `lib_id` `MadakeCAD:block`.
  RED: names are only `["MadakeCAD:block"]`.
- `our_block_lib_symbols_import_back_as_blocks_with_their_pin_table`: export, `import_kicad_sch(&text, "往復")`; `report.skipped` empty, `report.symbols == 5`, all `symbol_id == "block"`, `B8`'s `attrs["block_pins"]` equals the fixture's text.
  RED: `report.skipped` lists `MadakeCAD:block_… x1` entries.

Two `///` lines (EN, JA) above each `#[test]`.

- [ ] **Step 2: Run to verify RED**

`cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core each_block_shape_is_its_own_kicad_lib_symbol 2>&1 | grep -E "panicked|test result"` — Expected: `panicked` (left `["MadakeCAD:block"]`). Then the same with `our_block_lib_symbols_import_back_as_blocks_with_their_pin_table` — Expected: `panicked` on `skipped`.

- [ ] **Step 3: Implement**

`export_kicad_sch`: the same three replacements as in DXF (Task 12 Part 2): `DefIndex::new(symbols)`, `used: BTreeMap<String, Cow<SymbolDef>>` filled from `crate::block::export_def`, and per instance `let Some(def) = crate::block::export_def(inst, &index) else { … unresolved … }` with `lib_id` `format!("{EXPORT_LIB}{}", def.id)` instead of `inst.symbol_id`. `for def in used.values() { write_lib_symbol(&mut out, def); }` stays. Afterwards `grep -n "defs.get" src-tauri/crates/madake-core/src/kicad.rs` prints nothing.

`import_kicad_sch`:

```rust
let own_id = lib_id.strip_prefix(EXPORT_LIB).and_then(|id| {
    if crate::block::is_block_export_id(id) {
        Some(crate::block::BLOCK_SYMBOL_ID.to_string())
    } else {
        crate::symbol::resolve_symbol(id).is_some().then(|| id.to_string())
    }
});
```

and before `push(… Entity::Symbol …)`:

```rust
let mut attrs = BTreeMap::new();
if symbol_id == crate::block::BLOCK_SYMBOL_ID {
    let opt = |n: &str| Some(prop(n)).filter(|v| !v.is_empty());
    match crate::block::imported_block_attrs(opt("block_pins").as_deref(), opt("block_size").as_deref()) {
        Some(a) => attrs = a,
        None => report.warnings.push(format!(
            "{}: ブロックのピン表を読めないので見本の箱にしました",
            prop("Reference")
        )),
    }
}
```

with `attrs` instead of `attrs: Default::default()` (use the `BTreeMap` path already imported in the file, or `std::collections::BTreeMap`).

- [ ] **Step 4: Run to verify GREEN**

- The two test names, one per run, with the Step 2 command — Expected: `ok. 1 passed` each.
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms/src-tauri && cargo test -p madake-core kicad:: 2>&1 | grep -E "FAILED|test result"` — Expected: `ok`, 0 failed (`used_symbols_are_embedded_in_lib_symbols`, `export_then_import_round_trips_the_sheet` unchanged).
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms/src-tauri && cargo test -p madake-mcp --test link_export 2>&1 | grep -E "FAILED|test result"` — Expected: `ok`.

- [ ] **Step 5: Spec docs and commit**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check` — Expected: `spec docs up to date; all tests have bilingual spec comments`.

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms
git add src-tauri/crates/madake-core/src/kicad.rs docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(kicad): ブロックを形ごとのシンボルで書き出し、読み戻す"
```

---

### Task 14: Docs (EN + JA) and the final check

**Files:**
- Modify: `docs/03-schematic-editor.md`, `docs/03-schematic-editor.ja.md`, `docs/05-wire-management.md`, `docs/05-wire-management.ja.md`, `docs/10-automation-api.md`, `docs/10-automation-api.ja.md`, `docs/internal/feature-inventory.md`

**Interfaces:**
- Consumes: everything above. Produces: docs only.

- [ ] **Step 1: Schematic editor (block bullet)**

In `docs/03-schematic-editor.md`, replace `and confirming it is one undo step` (end of the *Device blocks with named pins* bullet) with:

```markdown
and confirming it is one undo step. A pin can also say which **physical connector** it is (*Connector* column: USB-A, USB-C or micro-B socket or plug, 3.5 mm jack or plug, DC jack or plug, screw terminals, JST PH or XH socket): a small black-and-white picture is drawn on the box edge — a socket as an open body set into the edge, a plug as a black insert sticking out — and that pin's connection point moves 5 mm out from the box (ERC then reports the wire end to tidy up, or the pin if the wire no longer reaches it). Consecutive screw-terminal or JST pins with the same *Group name* (`J3`, `TB1`) share one body labelled `J3 PH` / `TB1`. The picture is drawing-only: contacts, nets and reports do not change. It appears on the canvas and in SVG, PDF, PNG, DXF and KiCad output (DXF and KiCad write one block definition per distinct block shape, and read it back as a block with its pin table)
```

In `docs/03-schematic-editor.ja.md`, replace `確定は**undo一発**` with:

```markdown
確定は**undo一発**。ピンには**実物のコネクタの形**も書ける(「コネクタ」列: USB-A・USB-C・micro-B のメス/オス、3.5mm ジャック/プラグ、DC ジャック/プラグ、端子台(ネジ)、JST PH/XH メス)。外形の辺に小さな白黒の絵を描き(メスは辺にはめ込んだ白い口、オスは外へ出る黒い差し込み)、そのピンの接続点は外形から5mmへ伸びる(ERC が付け直す配線の端か、配線が届かなくなったピンを知らせる)。同じ「まとめ名」(`J3`・`TB1`)の端子台・JST のピンが続けば本体は1つになり、`J3 PH`・`TB1` と書く。絵は描くだけで、接点・ネット・帳票は変わらない。キャンバスと SVG・PDF・PNG・DXF・KiCad 出力に出る(DXF・KiCad はブロックの形ごとに定義を書き、読み戻すとピン表付きのブロックになる)
```

- [ ] **Step 2: Wire management (port paragraph)**

In `docs/05-wire-management.md`, replace

```markdown
A file with port pins is `.mdkproj` format 18; blocks with contacts are saved as the entity kind `port_block`, so older apps refuse the file
```

with

```markdown
A pin's `connector` picture (see [Schematic editor](03-schematic-editor.md)) is independent of its contacts: a USB socket pin normally has both. A file with port pins or connectors is `.mdkproj` format 19; blocks with contacts or connectors are saved as the entity kind `port_block`, so older apps refuse the file
```

In `docs/05-wire-management.ja.md`, replace

```markdown
ポートのピンを含むファイルは `.mdkproj` 形式18で、接点のあるブロックは種類 `port_block` で保存するので、古いアプリはそのファイルを開けない
```

with

```markdown
ピンの `connector`(外形に描くコネクタの絵、[回路図エディタ](03-schematic-editor.ja.md))は接点とは別のもので、USB の口のピンはふつう両方を持つ。ポートのピンかコネクタを含むファイルは `.mdkproj` 形式19で、接点かコネクタのあるブロックは種類 `port_block` で保存するので、古いアプリはそのファイルを開けない
```

- [ ] **Step 3: Automation API**

In `docs/10-automation-api.md`, replace

```markdown
Claude Code connects automatically via this repository's `.mcp.json`.
```

with

```markdown
Connector pictures: a `block_pins` pin may also carry `connector` (one of `usb_a_f usb_a_m usb_c_f usb_c_m micro_b_f micro_b_m jack35_f jack35_m dc_f dc_m screw jst_ph_f jst_xh_f`; `_f` socket, `_m` plug) and, for `screw` / `jst_ph_f` / `jst_xh_f`, `connector_ref` (consecutive pins with the same ref form one body, labelled `J3 PH` / `TB1`). An unknown kind, a blank ref, a ref on another kind and one ref used by two groups are refused. A connector pin's point is 5 mm from the box, so read positions with `get_pins`; `get_pins` and the netlist do not report the kind.
Claude Code connects automatically via this repository's `.mcp.json`.
```

In `docs/10-automation-api.ja.md`, replace

```markdown
Claude Codeはリポジトリの`.mcp.json`で自動接続。
```

with

```markdown
コネクタの絵: `block_pins` のピンには `connector`(`usb_a_f usb_a_m usb_c_f usb_c_m micro_b_f micro_b_m jack35_f jack35_m dc_f dc_m screw jst_ph_f jst_xh_f` のどれか。`_f` はメス、`_m` はオス)も書け、`screw`・`jst_ph_f`・`jst_xh_f` には `connector_ref` も書ける(同じ ref の続くピンは1つの本体になり、`J3 PH`・`TB1` と書く)。知らない種類・空の ref・ほかの種類の ref・2つのまとまりに使う ref は断る。コネクタのあるピンの接続点は外形から5mmなので、位置は `get_pins` で読む。`get_pins` とネットリストは種類を返さない。
Claude Codeはリポジトリの`.mcp.json`で自動接続。
```

- [ ] **Step 4: Feature inventory**

In `docs/internal/feature-inventory.md`, replace `ピン名は外形の内側・ピン番号は外側、5mmピッチ)。` with:

```markdown
ピン名は外形の内側・ピン番号は外側、5mmピッチ)。ピンの`connector`(13種: USB-A/USB-C/micro-B のメス・オス、3.5mm・DC のジャック・プラグ、端子台、JST PH/XH メス)で外形の辺に白黒のコネクタの絵を描き、続く端子台・JST のピンは同じ`connector_ref`で1つの本体と名前(`J3 PH`・`TB1`)になる。コネクタのあるピンの接続点は外形から5mm。絵だけで接点・ネットは変えない(キャンバスと SVG/PDF/PNG/DXF/KiCad。DXF・KiCad はブロックの形ごとに`MDK_block_<16桁>`・`MadakeCAD:block_<16桁>`の定義を書き、読み戻すと`block`とピン表に戻る)。Rust↔TS の一致は`madake-core/tests/golden/connector-blocks.json`で確かめる。形式19、コネクタのあるブロックはファイルでは種類`port_block`。
```

- [ ] **Step 5: Final verification**

Run each; Expected as stated:
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core -p madake-mcp -p madake-agent -p madake-cli 2>&1 | grep -E "^test result|FAILED|panicked|error\[" | head -40` — Expected: every line `test result: ok`, no `FAILED` (this is the CI command).
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && npx vitest run 2>&1 | tail -8` — Expected: everything passes except the pre-existing load failure of `src/chat/markdown.test.ts` (missing `markdown-it`); re-run `src/stores/terminals.test.ts` alone if it timed out.
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && npx vue-tsc --noEmit 2>&1 | grep -v "src/chat/markdown" | head` — Expected: no output.
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && python3 scripts/gen_spec.py --check` — Expected: `spec docs up to date; all tests have bilingual spec comments`.
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms/src-tauri && cargo test -p madake-core connector_blocks_match_the_shared_golden_file 2>&1 | grep "test result"` without `MADAKE_UPDATE_GOLDEN` — Expected: `ok` (the committed golden file is current).
- `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms && git status --short` — Expected: only the seven doc files of this task are modified.

- [ ] **Step 6: Commit**

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/connector-pictograms
git add docs/03-schematic-editor.md docs/03-schematic-editor.ja.md docs/05-wire-management.md docs/05-wire-management.ja.md docs/10-automation-api.md docs/10-automation-api.ja.md docs/internal/feature-inventory.md
git commit -m "docs: コネクタの絵の付け方と形式19を説明に書く"
```

---

## Spec coverage

| Spec item | Task |
|---|---|
| §1 `BlockPin` fields, `ConnectorKind`, `is_grouped`, `series` | 1 (Rust), 2 (TS) |
| §1 validation rules 1–5, English messages, order | 1, 2 |
| §1 grouping, labels | 1, 2 (logic), 6 (drawing) |
| §1 independence (no `PinDef` change, `get_pins` unchanged) | 1 (no `PinDef` edit), 11 (regression) |
| §2 frames and arc mapping | 4 |
| §2.1 pictures | 4 (hand-written left side), 5 (all sides via golden) |
| §2.2 pin point, lead, number, name | 5 |
| §2.2 group labels | 6 |
| §2.3 outline gaps / single `Rect` without connectors | 5 |
| §2.4 box size | 5 (pad), 6 (labels, caption inset) |
| §2.5 no-overlap | 7 (Rust), golden fixtures in 3 (TS twin) |
| §3 rendering path, no primitive-drawing changes | 5, 6, 7 (SVG) |
| §3.1 per-shape block definitions, DXF / KiCad import back | 12 (key + DXF), 13 (KiCad) |
| §4 format 19, `port_block` | 8 |
| §5 editor, store, issues, locales, preview | 9, 10 |
| §6 MCP, agent, docs | 11, 14 |
| Testing: twin golden | 3, regenerated in 5 and 6 |
