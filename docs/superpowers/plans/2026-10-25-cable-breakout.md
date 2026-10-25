# Cable Breakout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A cable can be drawn as one line on one end (or along its whole length) while every core stays its own conductor end to end: in the netlist, From-To wire list, BOM, cable list, ERC, SVG/canvas drawing and through one MCP call (`connect_cable`).

**Architecture:** The existing `Entity::Cable` gets an optional single-line `run` polyline, and every `CableCore` gets explicit `end_a`/`end_b` (`CoreEnd::Pin` = the line lands on that pin, `CoreEnd::Wire` = the core continues as that breakout wire). The netlist joins each core only through these assignments, never by position. On the JSON wire (file, IPC, MCP) a cable with a run is tagged `run_cable` through a serde proxy enum (`EntityRepr`), so older builds refuse such files. A pure planner (`madake-core/src/cable_run.rs`) turns pin names + per-end style (`multi` | `single`) into the cable plus its breakout wires; the MCP tool applies them as one undoable batch.

**Tech Stack:** Rust (madake-core, madake-mcp, madake-agent; serde 1, schemars 1.2, uuid v4), Vue 3 + TypeScript (Vitest, vue-tsc), Tauri 2.

**Spec:** `docs/superpowers/specs/2026-10-25-cable-breakout-design.md` (English) / `.ja.md` (Japanese). Read the spec first; it has the problem, the options, the decisions and the decision log.

## Global Constraints

- Work only in `/Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout` on branch `feature/cable-breakout`. Never touch `/Users/ktakeuchi/Documents/MadakeCAD/.worktrees/integration` and never switch branches in the main checkout.
- Before trusting any cargo result, run `touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs` from the worktree root (the shared `CARGO_TARGET_DIR=/Users/ktakeuchi/Documents/MadakeCAD/src-tauri/target` can link another branch's rlib).
- Every Rust test has exactly two `///` lines directly above `#[test]` / `#[tokio::test]`: line 1 English, line 2 Japanese. Every Vitest test has a `// ja: 日本語文` line directly above `it("English sentence", …)`.
- After adding or renaming tests: `python3 scripts/gen_spec.py` then `python3 scripts/gen_spec.py --check` (Expected: `spec docs up to date; all tests have bilingual spec comments`), and commit the regenerated `docs/13-specification.md` and `docs/13-specification.ja.md` with the task.
- Commits: Conventional Commits with a Japanese subject. **No `Co-Authored-By` or any other AI attribution trailer.** Do not push.
- `FORMAT_VERSION` = 17. "In memory this is one `Cable` struct. On disk, a cable that has a `run` is written with the entity kind `run_cable` and one without keeps `cable`." `load_project` refuses a file whose `format_version` is newer than the app.
- "Core identity across a single line is set by explicit core-to-pin / core-to-wire assignments, never by touching." The run is never a `Wire`.
- "A group becomes a net if it holds at least one wire or one cable core"; core-only nets are ordered after wire-bearing nets so existing net names do not move.
- ERC codes, exactly: `erc.cable_cores_shorted` (Error), `erc.cable_core_unassigned` (Error), `erc.cable_run_end_detached` (Warning, more than 2.5 mm from the part's outline), `erc.cable_core_overflow` (existing, also counts run-cable cores), `erc.dangling_wire` exempts breakout ends.
- Wire list: one end-to-end row per run-cable core; breakout wires get no rows; new last column `ケーブル芯` with values like `W45/D-`.
- BOM: "a `Wire` that is a core of any cable is left out of the wire roll-up (fixes today's double count)".
- `length_m` is the whole cable. Core-to-pin default mapping: by pin order. Part end: a bar across the landed pins. Same-sheet runs only. No shield-drain core. No `replace_breakout_block` helper.
- Drawing: the run is 0.7 mm thick (wires are 0.35 mm), with a short slash and the core count near its middle.
- Ruling (this plan): the interactive "single-line run" mode of the canvas Cable tool and its creation dialog (spec §5) are **not** in this plan. Run cables are created with `connect_cable` (MCP, in-app agent) or `execute_commands`; the Properties panel shows End A / End B read-only.
- Frontend tests need `node_modules`: if `/Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout/node_modules` is missing, `ln -s /Users/ktakeuchi/Documents/MadakeCAD/node_modules /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout/node_modules` (git-ignored).

## Review Focus

1. Deleting a run cable together with its breakout wires and the part it lands on, then undo, must bring all of them back (and redo must delete them again); today the inverse re-adds entities in selection order, so the cable can come back before its wires and the undo fails. Test: Task 2 `deleting_a_run_cable_with_its_wires_can_be_undone`.
2. An agent that names pins (`CTRL1.TX`) instead of numbering them must get the same result, and a side whose pin count differs from the core count must be refused with the side and both counts named, nothing drawn. Test: Task 6 `pins_by_name_and_a_count_mismatch`.
3. A single line landing on a through terminal block (two connection points per number) must map each core to one terminal number, on the left points by default and on the right points with `#2`. Test: Task 6 `a_single_line_on_a_through_terminal_block`.
4. A hand-edited file with a `run_cable` entry whose `run` is empty must open as a plain definition-line cable and be saved back as `cable`, not fail. Test: Task 1 `a_run_cable_without_a_run_opens_as_a_plain_cable`.
5. A breakout wire that ends on a net label (the continuation is on another sheet) must show the label name and the `none` remark in that core's From-To row, not an empty cell. Test: Task 5 `a_breakout_wire_ending_on_a_label_shows_the_label`.

---

## File map

| File | Responsibility in this plan |
|---|---|
| `src-tauri/crates/madake-core/src/model.rs` | `Cable::run`, `RunEnd`, `CableCore::end_a/end_b`, `CoreEnd`, `EntityRepr` proxy (`run_cable`), `FORMAT_VERSION` 17, refs/remap/translate |
| `src-tauri/crates/madake-core/src/lib.rs` | `CoreError::NewerFormat`, `pub mod cable_run` |
| `src-tauri/crates/madake-core/src/io.rs` | newer-format refusal, format tests |
| `src-tauri/crates/madake-core/src/cable.rs` | test fixtures, run-cable validation, breakout geometry, ERC, cable list, BOM/wire-list helpers, drawing geometry, `next_cable_reference`, `bar_ends` |
| `src-tauri/crates/madake-core/src/command.rs` | validation hook, delete-undo ordering |
| `src-tauri/crates/madake-core/src/netlist.rs` | core nodes, `Net::cable_cores`, `NetCableCore`, ordering |
| `src-tauri/crates/madake-core/src/verify.rs` | dangling-wire exemption |
| `src-tauri/crates/madake-core/src/reports.rs`, `report_sheet.rs` | per-core wire-list rows, `ケーブル芯` column, BOM roll-up |
| `src-tauri/crates/madake-core/src/cable_run.rs` (new) | `plan_cable_run` planner |
| `src-tauri/crates/madake-core/src/pin_wiring.rs` | `pins_on_route` becomes `pub(crate)` |
| `src-tauri/crates/madake-core/src/leader.rs`, `macros.rs` | include the run in bounds / macro mapping |
| `src-tauri/crates/madake-core/src/svg.rs` | run-cable drawing |
| `src-tauri/crates/madake-mcp/src/lib.rs`, `tests/drawing_tools.rs` | `connect_cable` tool |
| `src-tauri/crates/madake-agent/src/knowledge.rs` | one rule line for the in-app agent |
| `src/ipc.ts`, `src/canvas/cable.ts`, `src/canvas/*.ts`, `src/tools/controller.ts`, `src/components/*` , `src/locales/*.json` | `run_cable` kind, helpers, drawing, Properties columns |
| `docs/05-wire-management(.ja).md`, `docs/10-automation-api(.ja).md` | user docs |

---

### Task 1: Data model, `run_cable` on disk, format 17, newer-file refusal

**Files:**
- Modify: `src-tauri/crates/madake-core/src/model.rs` (FORMAT_VERSION doc + const at lines 11-52, `Entity` attrs at 374-377, `refs`/`remap_refs`/`translate` at 455-566, `Cable`/`CableCore` at 656-699)
- Modify: `src-tauri/crates/madake-core/src/lib.rs:68-85` (`CoreError`)
- Modify: `src-tauri/crates/madake-core/src/io.rs:43-49` (`load_project`) and its tests
- Modify: `src-tauri/crates/madake-core/src/leader.rs:67`, `src-tauri/crates/madake-core/src/macros.rs:619,875-879`
- Modify: `src-tauri/crates/madake-core/src/cable.rs` (test fixture module; existing test literal at 273-285, 432), `src-tauri/crates/madake-core/src/io.rs:379-389`, `src-tauri/crates/madake-core/src/command.rs:801-805` (struct literals)

**Interfaces:**
- Consumes: nothing new.
- Produces (later tasks rely on these exact names):
  - `pub struct Cable { …existing…, pub run: Vec<Point> }`, `impl Cable { pub fn is_run(&self) -> bool; pub fn run_end(&self, end: RunEnd) -> Option<Point> }`
  - `#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum RunEnd { A, B }` with `pub const BOTH: [RunEnd; 2]` and `pub fn letter(self) -> &'static str` (`"a"` / `"b"`)
  - `#[derive(Default)] pub struct CableCore { core, color, wire, pub end_a: Option<CoreEnd>, pub end_b: Option<CoreEnd> }`, `impl CableCore { pub fn end(&self, end: RunEnd) -> Option<&CoreEnd> }`
  - `pub enum CoreEnd { Pin { part: EntityId, pin: String, index: Option<u32> }, Wire { wire: EntityId } }` (serde tag `kind`, snake_case), `impl CoreEnd { pub fn target(&self) -> EntityId; pub fn remapped(&self, map: &BTreeMap<EntityId, EntityId>) -> Option<CoreEnd> }`
  - `pub enum EntityRepr` (JSON shape of `Entity`; adds `RunCable(Cable)`)
  - `CoreError::NewerFormat { found: u32, supported: u32 }`
  - Test fixtures `crate::cable::test_support::{connector, wire, insert, lte_sheet, aux_sheet, cable_of, engine_with, project_with, Lte, LTE_CORES}`

- [ ] **Step 1: Add the shared test fixtures to `cable.rs`**

Insert this module directly above `#[cfg(test)]\nmod tests {` in `src-tauri/crates/madake-core/src/cable.rs`:

```rust
#[cfg(test)]
pub(crate) mod test_support {
    //! 区間ケーブルのテスト用の図面 (model・io・command・netlist・verify・reports・svg・cable_run のテストが共有する)。
    use crate::command::Engine;
    use crate::geometry::Point;
    use crate::model::*;
    use uuid::Uuid;

    /// `connector_{n}p` のインスタンス。`mirror`ならピンが左向き (x - 7.5) になる。
    pub(crate) fn connector(n: usize, reference: &str, x: f64, y: f64, mirror: bool) -> SymbolInstance {
        SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: format!("connector_{n}p"),
            at: Point::new(x, y),
            rotation: 0,
            mirror,
            reference: reference.into(),
            value: String::new(),
            attrs: Default::default(),
        }
    }

    /// 黒 0.3sq の配線。
    pub(crate) fn wire(points: &[(f64, f64)]) -> Wire {
        Wire {
            id: Uuid::new_v4(),
            points: points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            color: "black".into(),
            sq: 0.3,
            length_m: None,
            length_source: Default::default(),
            part_no: None,
            net: None,
        }
    }

    /// エンティティをシートへ入れてidを返す。
    pub(crate) fn insert(sheet: &mut Sheet, e: Entity) -> EntityId {
        let id = e.id();
        sheet.entities.insert(id, e);
        id
    }

    /// LTEハーネス45の芯 (呼び名, 線色)。
    pub(crate) const LTE_CORES: [(&str, &str); 4] =
        [("VBUS", "red"), ("D-", "white"), ("D+", "green"), ("GND", "black")];

    /// LTEハーネス45の図面とid。
    pub(crate) struct Lte {
        pub sheet: Sheet,
        pub cable: EntityId,
        pub j45: EntityId,
        pub usb: EntityId,
        pub wires: Vec<EntityId>,
    }

    /// 基板 J45 (connector_4p、(50,100)、ピンは x=57.5 の y=92.5/97.5/102.5/107.5 で右向き) から
    /// 芯ごとの配線4本が x=77.5 まで出て、区間 (80,100)→(192.5,100) が USB1 (connector_4p を
    /// 反転して (200,100)、ピンは x=192.5 で左向き) に着く。芯は VBUS / D- / D+ / GND をピン順に。
    pub(crate) fn lte_sheet() -> Lte {
        let mut sheet = Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape);
        let j45 = insert(&mut sheet, Entity::Symbol(connector(4, "J45", 50.0, 100.0, false)));
        let usb = insert(&mut sheet, Entity::Symbol(connector(4, "USB1", 200.0, 100.0, true)));
        let mut wires = Vec::new();
        let mut cores = Vec::new();
        for (i, (name, color)) in LTE_CORES.into_iter().enumerate() {
            let y = 92.5 + 5.0 * i as f64;
            let mut w = wire(&[(57.5, y), (77.5, y)]);
            w.color = color.into();
            let id = insert(&mut sheet, Entity::Wire(w));
            wires.push(id);
            cores.push(CableCore {
                core: name.into(),
                color: color.into(),
                wire: None,
                end_a: Some(CoreEnd::Wire { wire: id }),
                end_b: Some(CoreEnd::Pin { part: usb, pin: (i + 1).to_string(), index: None }),
            });
        }
        let cable = Cable {
            id: Uuid::new_v4(),
            reference: "W45".into(),
            part_no: String::new(),
            core_count: 4,
            sq: 0.2,
            points: Vec::new(),
            cores,
            shield: None,
            length_m: Some(0.3),
            run: vec![Point::new(80.0, 100.0), Point::new(192.5, 100.0)],
        };
        let cable = insert(&mut sheet, Entity::Cable(cable));
        Lte { sheet, cable, j45, usb, wires }
    }

    /// 3.5mm AUXケーブル: J1 (connector_3p、(50,50)、ピン x=57.5 の y=45/50/55) と J2 (反転、(150,50)、
    /// ピン x=142.5) を全長1本線の区間 (57.5,50)→(142.5,50) で結ぶ。芯 L / R / GND はどちらの端もピン。
    /// 戻り値は (シート, ケーブルid)。
    pub(crate) fn aux_sheet() -> (Sheet, EntityId) {
        let mut sheet = Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape);
        let j1 = insert(&mut sheet, Entity::Symbol(connector(3, "J1", 50.0, 50.0, false)));
        let j2 = insert(&mut sheet, Entity::Symbol(connector(3, "J2", 150.0, 50.0, true)));
        let cores = ["L", "R", "GND"]
            .into_iter()
            .enumerate()
            .map(|(i, name)| CableCore {
                core: name.into(),
                end_a: Some(CoreEnd::Pin { part: j1, pin: (i + 1).to_string(), index: None }),
                end_b: Some(CoreEnd::Pin { part: j2, pin: (i + 1).to_string(), index: None }),
                ..Default::default()
            })
            .collect();
        let cable = Cable {
            id: Uuid::new_v4(),
            reference: "W58".into(),
            part_no: String::new(),
            core_count: 3,
            sq: 0.0,
            points: Vec::new(),
            cores,
            shield: None,
            length_m: None,
            run: vec![Point::new(57.5, 50.0), Point::new(142.5, 50.0)],
        };
        let id = insert(&mut sheet, Entity::Cable(cable));
        (sheet, id)
    }

    /// シート上のケーブル。
    pub(crate) fn cable_of(sheet: &Sheet, id: EntityId) -> &Cable {
        match &sheet.entities[&id] {
            Entity::Cable(c) => c,
            other => panic!("ケーブルではない: {other:?}"),
        }
    }

    /// シートを1枚だけ持つプロジェクト (シートのidはプロジェクトの先頭シートのものにそろえる)。
    pub(crate) fn project_with(mut sheet: Sheet) -> Project {
        let mut project = Project::new("t");
        sheet.id = project.sheets[0].id;
        project.sheets[0] = sheet;
        project
    }

    /// シートを1枚だけ持つエンジンと、そのシートid。
    pub(crate) fn engine_with(sheet: Sheet) -> (Engine, SheetId) {
        let project = project_with(sheet);
        let id = project.sheets[0].id;
        (Engine::new(project), id)
    }
}
```

- [ ] **Step 2: Write the failing tests**

Append to the `mod tests` in `src-tauri/crates/madake-core/src/io.rs` (it already has `use super::*; use crate::command::{Command, Engine}; use crate::geometry::Point; use crate::model::*; use uuid::Uuid;`), and replace the existing `the_current_format_version_is_sixteen` test (io.rs:362-371) with `the_current_format_version_is_seventeen`:

```rust
    /// The current file format version is 17, the version that added single-line cable runs (saved as "run_cable").
    /// 現行のファイル形式バージョンは17 (1本線の区間を持つケーブルを追加し、種類「run_cable」で保存する版) である。
    #[test]
    fn the_current_format_version_is_seventeen() {
        assert_eq!(FORMAT_VERSION, 17);
        let mut old = Project::new("x");
        old.format_version = 16;
        migrate(&mut old);
        assert_eq!(old.format_version, 17);
    }

    /// A cable with a single-line run is saved under the entity kind "run_cable" and comes back with its run and core ends; such a file has no entity of kind "cable".
    /// 1本線の区間を持つケーブルは種類「run_cable」で保存され、読み直すと区間と芯の端が戻る。そのファイルに種類「cable」のエンティティは無い。
    #[test]
    fn a_run_cable_is_saved_as_run_cable_and_round_trips() {
        let lte = crate::cable::test_support::lte_sheet();
        let project = crate::cable::test_support::project_with(lte.sheet);
        let dir = std::env::temp_dir().join(format!("madake-io-run-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("r.mdkproj");
        save_project(&path, &project).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"kind\": \"run_cable\""), "{text}");
        assert!(!text.contains("\"kind\": \"cable\""), "区間のあるケーブルは cable では書かない");
        let loaded = load_project(&path).unwrap();
        let c = crate::cable::test_support::cable_of(&loaded.sheets[0], lte.cable);
        assert_eq!(c.run, vec![Point::new(80.0, 100.0), Point::new(192.5, 100.0)]);
        assert_eq!(c.cores[1].core, "D-");
        assert_eq!(c.cores[1].end_a, Some(CoreEnd::Wire { wire: lte.wires[1] }));
        assert_eq!(
            c.cores[1].end_b,
            Some(CoreEnd::Pin { part: lte.usb, pin: "2".into(), index: None })
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A "run_cable" entry whose run is empty (for example after hand-editing the file) opens as a plain definition-line cable and is written back with the kind "cable".
    /// 区間が空の「run_cable」(ファイルを手で直したときなど) は普通の定義線ケーブルとして開き、書き戻すと種類「cable」になる。
    #[test]
    fn a_run_cable_without_a_run_opens_as_a_plain_cable() {
        let id = Uuid::new_v4();
        let json = format!(
            r#"{{"kind":"run_cable","id":"{id}","reference":"W1","core_count":2,"points":[{{"x":0.0,"y":0.0}},{{"x":10.0,"y":10.0}}]}}"#
        );
        let e: Entity = serde_json::from_str(&json).unwrap();
        let Entity::Cable(c) = &e else { panic!("ケーブル") };
        assert!(!c.is_run());
        assert_eq!(serde_json::to_value(&e).unwrap()["kind"], "cable");
    }

    /// A file saved by a newer MadakeCAD (a higher format version) is refused with a message naming both versions, instead of being opened with parts missing.
    /// 新しいMadakeCADで保存されたファイル (形式の版が大きい) は、一部が欠けたまま開くのではなく、両方の版を示す理由付きで断られる。
    #[test]
    fn a_file_from_a_newer_app_is_refused() {
        let dir = std::env::temp_dir().join(format!("madake-io-newer-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("newer.mdkproj");
        std::fs::write(
            &path,
            format!(r#"{{"format_version":{},"name":"未来","sheets":[]}}"#, FORMAT_VERSION + 1),
        )
        .unwrap();
        let err = load_project(&path).unwrap_err();
        assert!(
            matches!(err, crate::CoreError::NewerFormat { found, supported }
                if found == FORMAT_VERSION + 1 && supported == FORMAT_VERSION),
            "{err}"
        );
        assert!(err.to_string().contains("update MadakeCAD"), "{err}");
        std::fs::remove_dir_all(&dir).ok();
    }
```

Append to the `mod tests` in `src-tauri/crates/madake-core/src/model.rs`:

```rust
    /// A run cable lists the wires and parts its cores land on as in-sheet references, and remapping (sheet duplication) points every core end at the copies; ends with no copy are dropped.
    /// 区間ケーブルは芯が着く配線と部品をシート内参照として挙げ、写し替え (シート複製) で芯の端がすべて複製先を指す。複製先の無い端は落とす。
    #[test]
    fn a_run_cable_remaps_its_core_ends() {
        let lte = crate::cable::test_support::lte_sheet();
        let mut e = lte.sheet.entities[&lte.cable].clone();
        let refs = e.refs();
        assert!(refs.contains(&lte.usb), "{refs:?}");
        assert!(lte.wires.iter().all(|w| refs.contains(w)), "{refs:?}");
        let new_usb = uuid::Uuid::new_v4();
        let map: BTreeMap<EntityId, EntityId> = lte
            .wires
            .iter()
            .map(|w| (*w, uuid::Uuid::new_v4()))
            .chain([(lte.usb, new_usb)])
            .collect();
        e.remap_refs(&map);
        let Entity::Cable(c) = &e else { panic!("ケーブル") };
        assert_eq!(c.cores[0].end_a, Some(CoreEnd::Wire { wire: map[&lte.wires[0]] }));
        assert_eq!(c.cores[0].end_b, Some(CoreEnd::Pin { part: new_usb, pin: "1".into(), index: None }));
        e.remap_refs(&BTreeMap::new());
        let Entity::Cable(c) = &e else { panic!("ケーブル") };
        assert!(c.cores.iter().all(|k| k.end_a.is_none() && k.end_b.is_none()));
    }

    /// Moving a run cable moves every point of its single-line run.
    /// 区間ケーブルを動かすと、1本線の区間の全頂点が動く。
    #[test]
    fn translating_a_run_cable_moves_its_run() {
        let lte = crate::cable::test_support::lte_sheet();
        let mut e = lte.sheet.entities[&lte.cable].clone();
        e.translate(5.0, 2.5);
        let Entity::Cable(c) = &e else { panic!("ケーブル") };
        assert_eq!(c.run, vec![Point::new(85.0, 102.5), Point::new(197.5, 102.5)]);
    }
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core -- io:: model::`
Expected: FAIL to compile (`struct Cable has no field named run`, `cannot find type CoreEnd`, `no variant NewerFormat`).

- [ ] **Step 4: Implement the model**

In `model.rs`, extend the `FORMAT_VERSION` doc list and the constant:

```rust
/// - v16: 盤レイアウトの銘板 ([`Entity::Nameplate`]) と関連付き寸法線 ([`Entity::Dimension`])。
///   どちらも無い旧ファイルはそのまま開ける
/// - v17: 1本線の区間を持つケーブル ([`Cable::run`]) と芯ごとの行き先 ([`CableCore::end_a`] /
///   [`CableCore::end_b`])。区間のあるケーブルは種類`run_cable`で書く ([`EntityRepr`]) ので、
///   この版より前のアプリはそのファイルを開けない (区間を黙って捨てない)。区間の無いファイルは
///   版番号以外これまでと同じに書かれる
pub const FORMAT_VERSION: u32 = 17;
```

(Replace the existing two `v16` doc lines and `pub const FORMAT_VERSION: u32 = 16;` with the block above.)

Replace the `Entity` header

```rust
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Entity {
```

with

```rust
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(from = "EntityRepr", into = "EntityRepr")]
pub enum Entity {
```

Insert after the closing `}` of `impl Entity { … }` (just above the `/// 銘板 (M4仕様 §8。ACADEのNameplate相当)` doc):

```rust
/// 保存・送受信するときのエンティティの形 (JSONの`kind`タグ)。
///
/// メモリ上は区間のあるケーブルも無いケーブルも同じ [`Entity::Cable`] だが、JSONでは区間
/// ([`Cable::run`]) のあるケーブルだけを種類`run_cable`で書く。v16以前のアプリは知らない種類の
/// エンティティを含むファイルを開けないので、区間を黙って捨てて保存し直すことが起こらない
/// (serdeは既存の構造体の知らないフィールドを無視するため、フィールドを足すだけでは守れない)。
/// 読むときはどちらの種類も [`Entity::Cable`] になる。ファイル・Tauri IPC・MCPの全てがこの形。
#[derive(Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EntityRepr {
    Symbol(SymbolInstance),
    Wire(Wire),
    Junction(Junction),
    NetLabel(NetLabel),
    Text(TextEntity),
    Harness(Harness),
    Shape(Shape),
    PanelOutline(PanelOutline),
    DinRail(DinRail),
    Duct(Duct),
    Footprint(Footprint),
    PanelLegend(PanelLegend),
    WireLegend(WireLegend),
    Cable(Cable),
    /// 区間のあるケーブル (v17)。
    RunCable(Cable),
    Nameplate(Nameplate),
    Dimension(Dimension),
}

impl From<EntityRepr> for Entity {
    fn from(r: EntityRepr) -> Self {
        match r {
            EntityRepr::Symbol(e) => Entity::Symbol(e),
            EntityRepr::Wire(e) => Entity::Wire(e),
            EntityRepr::Junction(e) => Entity::Junction(e),
            EntityRepr::NetLabel(e) => Entity::NetLabel(e),
            EntityRepr::Text(e) => Entity::Text(e),
            EntityRepr::Harness(e) => Entity::Harness(e),
            EntityRepr::Shape(e) => Entity::Shape(e),
            EntityRepr::PanelOutline(e) => Entity::PanelOutline(e),
            EntityRepr::DinRail(e) => Entity::DinRail(e),
            EntityRepr::Duct(e) => Entity::Duct(e),
            EntityRepr::Footprint(e) => Entity::Footprint(e),
            EntityRepr::PanelLegend(e) => Entity::PanelLegend(e),
            EntityRepr::WireLegend(e) => Entity::WireLegend(e),
            EntityRepr::Cable(e) | EntityRepr::RunCable(e) => Entity::Cable(e),
            EntityRepr::Nameplate(e) => Entity::Nameplate(e),
            EntityRepr::Dimension(e) => Entity::Dimension(e),
        }
    }
}

impl From<Entity> for EntityRepr {
    fn from(e: Entity) -> Self {
        match e {
            Entity::Symbol(e) => EntityRepr::Symbol(e),
            Entity::Wire(e) => EntityRepr::Wire(e),
            Entity::Junction(e) => EntityRepr::Junction(e),
            Entity::NetLabel(e) => EntityRepr::NetLabel(e),
            Entity::Text(e) => EntityRepr::Text(e),
            Entity::Harness(e) => EntityRepr::Harness(e),
            Entity::Shape(e) => EntityRepr::Shape(e),
            Entity::PanelOutline(e) => EntityRepr::PanelOutline(e),
            Entity::DinRail(e) => EntityRepr::DinRail(e),
            Entity::Duct(e) => EntityRepr::Duct(e),
            Entity::Footprint(e) => EntityRepr::Footprint(e),
            Entity::PanelLegend(e) => EntityRepr::PanelLegend(e),
            Entity::WireLegend(e) => EntityRepr::WireLegend(e),
            Entity::Cable(c) if c.is_run() => EntityRepr::RunCable(c),
            Entity::Cable(c) => EntityRepr::Cable(c),
            Entity::Nameplate(e) => EntityRepr::Nameplate(e),
            Entity::Dimension(e) => EntityRepr::Dimension(e),
        }
    }
}
```

In `Entity::refs`, replace

```rust
            Entity::Cable(e) => e.cores.iter().filter_map(|c| c.wire).collect(),
```

with

```rust
            // ケーブルの芯が割り当てられた配線と、区間ケーブルの芯の行き先 (部品・分岐の配線)
            Entity::Cable(e) => e
                .cores
                .iter()
                .flat_map(|c| {
                    c.wire
                        .into_iter()
                        .chain(c.end_a.as_ref().map(CoreEnd::target))
                        .chain(c.end_b.as_ref().map(CoreEnd::target))
                })
                .collect(),
```

(Also delete the now-duplicated comment line `// ケーブルの芯線が割り当てられた配線` above it.)

In `Entity::remap_refs`, replace the `Entity::Cable(e) => { … }` arm with:

```rust
            Entity::Cable(e) => {
                for core in &mut e.cores {
                    if let Some(w) = core.wire {
                        core.wire = map.get(&w).copied();
                    }
                    core.end_a = core.end_a.as_ref().and_then(|end| end.remapped(map));
                    core.end_b = core.end_b.as_ref().and_then(|end| end.remapped(map));
                }
            }
```

In `Entity::translate`, replace the `Entity::Cable(e) => { for p in &mut e.points { … } }` arm with:

```rust
            Entity::Cable(e) => {
                for p in e.points.iter_mut().chain(e.run.iter_mut()) {
                    *p = p.translated(dx, dy);
                }
            }
```

In `struct Cable`, add after `pub length_m: Option<f64>,`:

```rust
    /// 1本線の区間 (用紙座標mm。配線と同じ直交の折れ線)。空 = 定義線ケーブル (v15の形)。
    /// 区間があると、芯ごとの行き先 ([`CableCore::end_a`] / [`CableCore::end_b`]) でネットが繋がり、
    /// 区間の線そのものには位置では何も繋がらない ([`crate::netlist::extract_netlist`])。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub run: Vec<Point>,
```

Replace the `CableCore` struct (model.rs:688-699) with:

```rust
/// 区間の端。A = [`Cable::run`]の最初の点、B = 最後の点。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunEnd {
    A,
    B,
}

impl RunEnd {
    /// 両端 (A, B の順)。
    pub const BOTH: [RunEnd; 2] = [RunEnd::A, RunEnd::B];

    /// 文字 ("a" / "b")。エラーや診断の文に使う。
    pub fn letter(self) -> &'static str {
        match self {
            RunEnd::A => "a",
            RunEnd::B => "b",
        }
    }
}

impl Cable {
    /// 1本線の区間を持つか (区間ケーブル)。
    pub fn is_run(&self) -> bool {
        !self.run.is_empty()
    }

    /// 区間の端の点。区間が無ければNone。
    pub fn run_end(&self, end: RunEnd) -> Option<Point> {
        match end {
            RunEnd::A => self.run.first().copied(),
            RunEnd::B => self.run.last().copied(),
        }
    }
}

/// ケーブルの芯線1本の割当。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CableCore {
    /// 芯の呼び (例 "1"、"VBUS")。
    pub core: String,
    /// 芯の色 (割り当てた配線の線色)。
    #[serde(default)]
    pub color: String,
    /// 割り当てた配線 (定義線ケーブル)。配線が消えていればNone (帳票では予備芯と同じ扱い)。
    #[serde(default)]
    pub wire: Option<EntityId>,
    /// 区間の始点 (A) 側でこの芯がどこへ出るか (区間ケーブル、v17)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_a: Option<CoreEnd>,
    /// 区間の終点 (B) 側でこの芯がどこへ出るか (区間ケーブル、v17)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_b: Option<CoreEnd>,
}

impl CableCore {
    /// 区間の端`end`での行き先。
    pub fn end(&self, end: RunEnd) -> Option<&CoreEnd> {
        match end {
            RunEnd::A => self.end_a.as_ref(),
            RunEnd::B => self.end_b.as_ref(),
        }
    }
}

/// 区間ケーブルの芯が区間の端からどこへ出るか (v17)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CoreEnd {
    /// 区間が部品に着き、この芯はそのピンになる。`index`は同じ番号の接続点が複数あるとき何番目か
    /// (1始まり。"TB1.1#2"の2)。省略 = 最初の接続点。
    Pin {
        part: EntityId,
        pin: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<u32>,
    },
    /// 分岐: 芯はこの配線として続く (外被を剥いて出した芯そのもの)。配線の端点のうち区間の端に
    /// 近い方が分岐側の端 ([`crate::cable::breakout_point`])。
    Wire { wire: EntityId },
}

impl CoreEnd {
    /// 指している部品か配線のid。
    pub fn target(&self) -> EntityId {
        match self {
            CoreEnd::Pin { part, .. } => *part,
            CoreEnd::Wire { wire } => *wire,
        }
    }

    /// 対応表で写し替える。対応の無い参照はNone (シート複製で相手が複製されていないとき)。
    pub fn remapped(&self, map: &BTreeMap<EntityId, EntityId>) -> Option<CoreEnd> {
        let to = *map.get(&self.target())?;
        Some(match self {
            CoreEnd::Pin { pin, index, .. } => CoreEnd::Pin { part: to, pin: pin.clone(), index: *index },
            CoreEnd::Wire { .. } => CoreEnd::Wire { wire: to },
        })
    }
}
```

- [ ] **Step 5: `CoreError::NewerFormat` and the refusal**

In `src-tauri/crates/madake-core/src/lib.rs`, add inside `pub enum CoreError` after the `RevertConflict` variant:

```rust
    /// プロジェクトファイルがこのアプリより新しい形式で保存されている ([`crate::io::load_project`])。
    #[error("this file uses project format {found}, but this MadakeCAD reads up to format {supported}; update MadakeCAD to open it")]
    NewerFormat { found: u32, supported: u32 },
```

In `src-tauri/crates/madake-core/src/io.rs`, replace `load_project` with:

```rust
/// プロジェクトを読み込む。アプリより新しい形式のファイルは断り ([`check_format_version`])、
/// 古い形式のファイルは現行形式へ移行してから返す ([`migrate`])。
pub fn load_project(path: &Path) -> Result<Project> {
    let json = std::fs::read_to_string(path)?;
    check_format_version(&json)?;
    let mut project: Project = serde_json::from_str(&json)?;
    migrate(&mut project);
    Ok(project)
}

/// ファイルの`format_version`がこのアプリの読める版 ([`crate::model::FORMAT_VERSION`]) より新しければ
/// 断る。中身全体を読む前に版番号だけを見るので、新しい版で増えたエンティティの種類 (例: v17の
/// `run_cable`) に当たって分かりにくいJSONのエラーになる前に、理由を伝えられる。
fn check_format_version(json: &str) -> Result<()> {
    #[derive(serde::Deserialize)]
    struct Head {
        #[serde(default)]
        format_version: u32,
    }
    let head: Head = serde_json::from_str(json)?;
    if head.format_version > crate::model::FORMAT_VERSION {
        return Err(crate::CoreError::NewerFormat {
            found: head.format_version,
            supported: crate::model::FORMAT_VERSION,
        });
    }
    Ok(())
}
```

- [ ] **Step 6: Fix the struct literals and the run in bounds/macros**

- `cable.rs` test helper `fn cable(…) -> Cable` (cable.rs:273-285): add `run: Vec::new(),` after `length_m: Some(12.5),`.
- `cable.rs:432`: `crate::model::CableCore { core: "1".into(), color: String::new(), wire: Some(Uuid::new_v4()), ..Default::default() }`.
- `io.rs:379-389` (`a_cable_round_trips_through_a_saved_file`): add `run: Vec::new(),` after `length_m: Some(12.5),`, and change the core to `crate::model::CableCore { core: "1".into(), color: "black".into(), wire: Some(wire), ..Default::default() }`.
- `command.rs:801-805` (`AddCable`): `crate::model::CableCore { core: (i + 1).to_string(), color, wire: Some(wire), ..Default::default() }`.
- `leader.rs:67`: `Entity::Cable(c) => bounds_of_points(&[c.points.as_slice(), c.run.as_slice()].concat()),`
- `macros.rs:619`: `Entity::Cable(c) => pts.extend(c.points.iter().chain(&c.run).copied()),`
- `macros.rs:875-879`: replace `for p in &mut c.points {` with `for p in c.points.iter_mut().chain(c.run.iter_mut()) {`.

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core`
Expected: PASS, including `the_current_format_version_is_seventeen`, `a_run_cable_is_saved_as_run_cable_and_round_trips`, `a_run_cable_without_a_run_opens_as_a_plain_cable`, `a_file_from_a_newer_app_is_refused`, `a_run_cable_remaps_its_core_ends`, `translating_a_run_cable_moves_its_run`; no existing test fails.

Then: `cd src-tauri && cargo build --workspace` — Expected: builds (madake-mcp/cli use `Entity` through serde only).

- [ ] **Step 8: Regenerate the spec docs and commit**

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout
python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check
git add src-tauri/crates/madake-core/src/{model.rs,lib.rs,io.rs,cable.rs,command.rs,leader.rs,macros.rs} docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(core): ケーブルに1本線の区間と芯の行き先を足し、run_cable で保存する (形式17)"
```

---

### Task 2: Validating run cables and undoing deletes in a safe order

**Files:**
- Modify: `src-tauri/crates/madake-core/src/cable.rs` (imports at top; new `validate_run_cable`, `pin_end_position`)
- Modify: `src-tauri/crates/madake-core/src/command.rs:1789-1808` (`validate_entity` Cable arm), `command.rs:866-868` (`DeleteEntities`)
- Test: `src-tauri/crates/madake-core/src/cable.rs` `mod tests`

**Interfaces:**
- Consumes: Task 1 `Cable::is_run`, `CableCore::end`, `CoreEnd`, `RunEnd::{BOTH, letter}`, `test_support::{lte_sheet, engine_with, cable_of}`.
- Produces:
  - `pub fn validate_run_cable(sheet: &Sheet, c: &Cable) -> Result<(), String>`
  - `pub fn pin_end_position(sheet: &Sheet, defs: &[SymbolDef], end: &CoreEnd) -> Option<Point>`

- [ ] **Step 1: Write the failing tests**

Append to `mod tests` in `cable.rs`:

```rust
    /// A run cable is accepted when every core has both ends on this sheet, and refused when a core lacks an end, points at a missing pin or wire, reuses a wire, still uses the definition-line field, or the run has fewer than two points; a definition-line cable with core ends is refused too.
    /// 区間ケーブルは、どの芯も両端がこのシートにあれば受け付け、端の無い芯・無いピンや配線を指す芯・同じ配線の使い回し・定義線用の欄を使った芯・2点未満の区間があれば断る。芯の行き先を持つ定義線ケーブルも断る。
    #[test]
    fn a_run_cable_is_checked_when_placed() {
        let lte = test_support::lte_sheet();
        let mut sheet = lte.sheet.clone();
        let cable = test_support::cable_of(&sheet, lte.cable).clone();
        sheet.entities.remove(&lte.cable);
        let (mut engine, sheet_id) = test_support::engine_with(sheet);
        let bad = |f: &dyn Fn(&mut Cable)| {
            let mut c = cable.clone();
            c.id = Uuid::new_v4();
            f(&mut c);
            c
        };
        let refused = [
            bad(&|c| c.cores[0].end_a = None),
            bad(&|c| c.cores[1].end_b = Some(CoreEnd::Pin { part: lte.usb, pin: "9".into(), index: None })),
            bad(&|c| c.cores[2].end_a = Some(CoreEnd::Wire { wire: Uuid::new_v4() })),
            bad(&|c| c.cores[3].end_a = Some(CoreEnd::Wire { wire: lte.wires[0] })),
            bad(&|c| c.cores[0].wire = Some(lte.wires[0])),
            bad(&|c| c.run.truncate(1)),
            bad(&|c| {
                c.run.clear();
                c.points = vec![Point::new(0.0, 0.0), Point::new(1.0, 1.0)];
            }),
        ];
        for c in refused {
            assert!(
                engine.execute(Command::AddEntity { sheet_id, entity: Entity::Cable(c) }).is_err(),
                "断るはず"
            );
        }
        engine.execute(Command::AddEntity { sheet_id, entity: Entity::Cable(cable) }).unwrap();
    }

    /// Deleting a run cable together with its breakout wires and the part it lands on, then undoing, brings all of them back, and redo deletes them again (the cable is restored after what it points at).
    /// 区間ケーブルを分岐の配線と着く先の部品と一緒に消してから元に戻すと全部が戻り、やり直すとまた消える (ケーブルは指す相手の後に戻す)。
    #[test]
    fn deleting_a_run_cable_with_its_wires_can_be_undone() {
        let lte = test_support::lte_sheet();
        let (mut engine, sheet_id) = test_support::engine_with(lte.sheet);
        let count = |e: &Engine| e.project().sheet(sheet_id).unwrap().entities.len();
        let before = count(&engine);
        let mut ids = lte.wires.clone();
        ids.push(lte.cable);
        ids.push(lte.usb);
        engine.execute(Command::DeleteEntities { sheet_id, ids }).unwrap();
        assert_eq!(count(&engine), before - 6);
        engine.undo().unwrap();
        assert_eq!(count(&engine), before);
        assert!(engine.project().sheet(sheet_id).unwrap().entities.contains_key(&lte.cable));
        engine.redo().unwrap();
        assert_eq!(count(&engine), before - 6);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core -- cable::tests::a_run_cable_is_checked_when_placed cable::tests::deleting_a_run_cable_with_its_wires_can_be_undone`
Expected: FAIL, both. Today every cable must have exactly two definition-line points, so `a_run_cable_is_checked_when_placed` panics at its last `.unwrap()` with `invalid command: a cable definition line needs exactly two points`, and `deleting_a_run_cable_with_its_wires_can_be_undone` panics at `engine.undo().unwrap()` with the same message (the undo re-adds the run cable through `AddEntity`). After Step 3 the first passes through the new validation, and the second passes only because of the re-add ordering (without it the cable is re-added before its wires and is refused with `points at a wire that is not on this sheet`).

- [ ] **Step 3: Implement validation**

In `cable.rs`, replace the top imports

```rust
use std::collections::BTreeMap;

use crate::geometry::Point;
use crate::model::{Cable, Entity, EntityId, Project, Sheet};
use crate::reports::{csv_row, endpoint_label};
use crate::symbol::sheet_symbol_defs;
use crate::verify::{Diagnostic, Severity};
```

with

```rust
use std::collections::BTreeMap;

use crate::geometry::Point;
use crate::model::{Cable, CoreEnd, Entity, EntityId, Project, RunEnd, Sheet};
use crate::netlist::pin_positions;
use crate::reports::{csv_row, endpoint_label};
use crate::symbol::{sheet_symbol_defs, SymbolDef};
use crate::verify::{Diagnostic, Severity};
```

Add after `check_cables` (before `#[cfg(test)] pub(crate) mod test_support`):

```rust
/// `CoreEnd::Pin`が指すピンの用紙上の位置 (`index`番目、省略 = 最初の接続点)。
/// 部品・ピンが無いとき、ピンでない行き先のときはNone。
pub fn pin_end_position(sheet: &Sheet, defs: &[SymbolDef], end: &CoreEnd) -> Option<Point> {
    let CoreEnd::Pin { part, pin, index } = end else {
        return None;
    };
    let Some(Entity::Symbol(s)) = sheet.entities.get(part) else {
        return None;
    };
    let def = crate::symbol::instance_symbol_def(s, defs)?;
    pin_positions(s, &def)
        .into_iter()
        .filter(|(no, _)| no == pin)
        .nth(index.unwrap_or(1).saturating_sub(1) as usize)
        .map(|(_, p)| p)
}

/// 区間ケーブルの形を調べる ([`crate::command::Engine`]が置く・更新するときに通す)。
///
/// 区間が2点以上・どの芯も両端を持つ・芯の`wire` (定義線ケーブルの欄) は使わない・行き先の
/// 部品にそのピンがあり配線がこのシートにある・1本の配線/1つのピンを2本の芯で使わない。
pub fn validate_run_cable(sheet: &Sheet, c: &Cable) -> Result<(), String> {
    if c.run.len() < 2 {
        return Err(format!("cable {} needs a run of at least two points", c.reference));
    }
    let defs = sheet_symbol_defs(sheet);
    let mut used_wires: Vec<EntityId> = Vec::new();
    let mut used_pins: Vec<(EntityId, String, u32)> = Vec::new();
    for core in &c.cores {
        if core.wire.is_some() {
            return Err(format!(
                "cable {} core {}: a run cable assigns cores with end_a/end_b, not wire",
                c.reference, core.core
            ));
        }
        for end in RunEnd::BOTH {
            let Some(e) = core.end(end) else {
                return Err(format!("cable {} core {} has no end_{}", c.reference, core.core, end.letter()));
            };
            match e {
                CoreEnd::Wire { wire } => {
                    if !matches!(sheet.entities.get(wire), Some(Entity::Wire(_))) {
                        return Err(format!(
                            "cable {} core {} points at a wire that is not on this sheet: {wire}",
                            c.reference, core.core
                        ));
                    }
                    if used_wires.contains(wire) {
                        return Err(format!("cable {}: wire {wire} is used by two cores", c.reference));
                    }
                    used_wires.push(*wire);
                }
                CoreEnd::Pin { part, pin, index } => {
                    if pin_end_position(sheet, &defs, e).is_none() {
                        return Err(format!(
                            "cable {} core {} points at pin {pin} of {part}, which is not on this sheet",
                            c.reference, core.core
                        ));
                    }
                    let key = (*part, pin.clone(), index.unwrap_or(1));
                    if used_pins.contains(&key) {
                        return Err(format!("cable {}: pin {pin} of {part} is used by two cores", c.reference));
                    }
                    used_pins.push(key);
                }
            }
        }
    }
    Ok(())
}
```

In `command.rs` `validate_entity`, replace the whole `Entity::Cable(c) => { … }` arm with:

```rust
        Entity::Cable(c) => {
            if c.core_count == 0 {
                return Err(CoreError::InvalidCommand("a cable needs at least one core".into()));
            }
            if c.is_run() {
                crate::cable::validate_run_cable(sheet, c).map_err(CoreError::InvalidCommand)?;
            } else {
                if c.points.len() != 2 {
                    return Err(CoreError::InvalidCommand(
                        "a cable definition line needs exactly two points".into(),
                    ));
                }
                for core in &c.cores {
                    if core.end_a.is_some() || core.end_b.is_some() {
                        return Err(CoreError::InvalidCommand(format!(
                            "cable core {} has end_a/end_b but the cable has no run",
                            core.core
                        )));
                    }
                    if let Some(w) = core.wire {
                        if !matches!(sheet.entities.get(&w), Some(Entity::Wire(_))) {
                            return Err(CoreError::InvalidCommand(format!(
                                "cable core {} points at a wire that is not on this sheet: {w}",
                                core.core
                            )));
                        }
                    }
                }
            }
        }
```

In `command.rs` `Command::DeleteEntities`, replace

```rust
                // 復元順は削除の逆順でなくても問題ない(idは独立)が、直感的に逆順にする。
                inverse.reverse();
                Ok((ops, inverse))
```

with

```rust
                // 復元順は削除の逆順にする。ただし参照を持つもの (ケーブル・引き出し線付きの注記・
                // 銘板・寸法線) は指す相手より後に戻す: 先に戻すと AddEntity の検証が「相手がシートに
                // 無い」で断り、undo が失敗する
                inverse.reverse();
                inverse.sort_by_key(|c| {
                    matches!(c, Command::AddEntity { entity, .. } if !entity.refs().is_empty())
                });
                Ok((ops, inverse))
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core`
Expected: PASS (all, including `a_malformed_cable_is_refused`).

- [ ] **Step 5: Regenerate the spec docs and commit**

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout
python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check
git add src-tauri/crates/madake-core/src/{cable.rs,command.rs} docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(core): 区間ケーブルの形を検証し、削除の取り消しは参照先を先に戻す"
```

---

### Task 3: Netlist joins cores by assignment

**Files:**
- Modify: `src-tauri/crates/madake-core/src/netlist.rs` (imports line 8, `Net` at 69-82, `extract_netlist` at 187-370)
- Modify: `src/ipc.ts:710-719` (`Net`) — type only, so the frontend keeps compiling against the new field
- Test: `src-tauri/crates/madake-core/src/netlist.rs`, `src-tauri/crates/madake-core/src/wire_no.rs`

**Interfaces:**
- Consumes: Task 1 model and fixtures.
- Produces:
  - `#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)] pub struct NetCableCore { pub cable_id: EntityId, pub reference: String, pub core: String }`
  - `Net::cable_cores: Vec<NetCableCore>` (serde default, omitted when empty)
  - TS: `export interface NetCableCore { cable_id: string; reference: string; core: string }`, `Net.cable_cores?: NetCableCore[]`

- [ ] **Step 1: Write the failing tests**

Append to `mod tests` in `netlist.rs`:

```rust
    /// The LTE harness (four board wires breaking out of a single line that lands on the USB plug) gives four separate nets, each joining one board pin to the matching plug pin through its cable core.
    /// LTEハーネス (基板側の配線4本が1本線に分岐し、その線がUSBプラグに着く) は別々の4ネットになり、各ネットは芯を通して基板のピン1本とプラグの同じ番号のピンを結ぶ。
    #[test]
    fn a_run_cable_keeps_each_core_its_own_net() {
        let lte = crate::cable::test_support::lte_sheet();
        let nets = extract_netlist(&lte.sheet, &crate::symbol::sheet_symbol_defs(&lte.sheet));
        assert_eq!(nets.len(), 4, "{nets:?}");
        for (i, (core, _)) in crate::cable::test_support::LTE_CORES.into_iter().enumerate() {
            let pin = (i + 1).to_string();
            let net = nets
                .iter()
                .find(|n| n.cable_cores.iter().any(|c| c.core == core))
                .expect(core);
            let pins: Vec<(String, String)> =
                net.pins.iter().map(|p| (p.reference.clone(), p.pin.clone())).collect();
            assert_eq!(pins, vec![("J45".to_string(), pin.clone()), ("USB1".to_string(), pin)]);
            assert_eq!(net.wire_ids, vec![lte.wires[i]]);
            assert_eq!(net.cable_cores[0].reference, "W45");
        }
    }

    /// A single line lying across every pin of a connector joins none of them: a cable's run never connects by position.
    /// コネクタの全ピンの上を通る1本線でも、どのピンも繋がない (ケーブルの区間は位置では繋がらない)。
    #[test]
    fn a_run_over_a_pin_column_joins_nothing() {
        let mut lte = crate::cable::test_support::lte_sheet();
        let Some(Entity::Cable(c)) = lte.sheet.entities.get_mut(&lte.cable) else { panic!("ケーブル") };
        // USB1 のピン列 (x=192.5、y=92.5〜107.5) に沿って区間を引き直す
        c.run = vec![
            Point::new(80.0, 100.0),
            Point::new(192.5, 100.0),
            Point::new(192.5, 85.0),
            Point::new(192.5, 115.0),
        ];
        let nets = extract_netlist(&lte.sheet, &crate::symbol::sheet_symbol_defs(&lte.sheet));
        assert_eq!(nets.len(), 4);
        assert!(
            nets.iter().all(|n| n.pins.iter().filter(|p| p.reference == "USB1").count() == 1),
            "{nets:?}"
        );
    }

    /// A single-line cable between two connectors (3.5 mm AUX: L, R, GND) gives one net per core even though no wire is drawn.
    /// 2つのコネクタを全長1本線で結ぶケーブル (3.5mm AUX の L・R・GND) は、配線が1本も無くても芯ごとに1ネットになる。
    #[test]
    fn a_connector_to_connector_run_gives_one_net_per_core() {
        let (sheet, _) = crate::cable::test_support::aux_sheet();
        let nets = extract_netlist(&sheet, &crate::symbol::sheet_symbol_defs(&sheet));
        assert_eq!(nets.len(), 3, "{nets:?}");
        for net in &nets {
            assert!(net.wire_ids.is_empty());
            assert_eq!(net.pins.len(), 2);
            assert_eq!(net.pins[0].pin, net.pins[1].pin);
        }
    }

    /// Nets made only of cable cores are named after the nets that have wires, so adding such a cable does not rename existing nets.
    /// 芯だけのネットは配線のあるネットの後ろに名前が付くので、そういうケーブルを足しても今あるネットの名前は変わらない。
    #[test]
    fn core_only_nets_are_named_after_wire_nets() {
        let (mut sheet, _) = crate::cable::test_support::aux_sheet();
        for e in [
            symbol("resistor", "R1", 100.0, 150.0),
            symbol("resistor", "R2", 150.0, 150.0),
            wire(&[(107.5, 150.0), (142.5, 150.0)]),
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let nets = extract_netlist(&sheet, &crate::symbol::sheet_symbol_defs(&sheet));
        assert_eq!(nets[0].wire_ids.len(), 1);
        assert!(nets[1..].iter().all(|n| n.wire_ids.is_empty()));
        assert_eq!(
            nets.iter().map(|n| n.name.as_str()).collect::<Vec<_>>(),
            ["N001", "N002", "N003", "N004"]
        );
    }

    /// Moving the part a single line lands on keeps every core connected to the same pin: connectivity follows the assignment, not the drawing.
    /// 1本線が着く部品を動かしても、どの芯も同じピンに繋がったまま (接続は割当てで決まり、描いた位置では決まらない)。
    #[test]
    fn moving_the_landing_part_keeps_core_connectivity() {
        let mut lte = crate::cable::test_support::lte_sheet();
        lte.sheet.entities.get_mut(&lte.usb).unwrap().translate(0.0, 40.0);
        let nets = extract_netlist(&lte.sheet, &crate::symbol::sheet_symbol_defs(&lte.sheet));
        assert_eq!(nets.len(), 4);
        assert!(nets.iter().all(|n| n.pins.len() == 2 && n.pins[0].pin == n.pins[1].pin), "{nets:?}");
    }
```

Append to `mod tests` in `wire_no.rs`:

```rust
    /// Renumbering wires on a sheet with a connector-to-connector single-line cable numbers the drawn wires 1, 2 with no gap left by the cable's wire-less core nets.
    /// コネクタ同士を結ぶ1本線ケーブルがあるシートで線番を振り直しても、配線は1, 2と飛ばずに番号が付く (配線の無い芯のネットは番号を使わない)。
    #[test]
    fn core_only_nets_take_no_wire_number() {
        let (aux, _) = crate::cable::test_support::aux_sheet();
        let w1 = wire(&[(0.0, 200.0), (20.0, 200.0)], None);
        let w2 = wire(&[(0.0, 210.0), (20.0, 210.0)], None);
        let (id1, id2) = (w1.id(), w2.id());
        let mut all: Vec<Entity> = aux.entities.into_values().collect();
        all.push(w1);
        all.push(w2);
        let (mut engine, sid, _) = engine_with(all);
        renumber(&mut engine, Some(sid), RenumberMode::Renumber, 1);
        assert_eq!(net_of(&engine, sid, id1).as_deref(), Some("1"));
        assert_eq!(net_of(&engine, sid, id2).as_deref(), Some("2"));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core -- netlist:: wire_no::`
Expected: FAIL to compile (`no field cable_cores on type Net`).

- [ ] **Step 3: Implement**

`netlist.rs` imports: `use crate::model::{CoreEnd, Entity, EntityId, Sheet, SymbolInstance};`

Add to `struct Net` after `pub wire_no: Option<String>,`:

```rust
    /// このネットを通る区間ケーブルの芯 (v17)。ケーブルid・参照記号・芯の呼びの順に並ぶ。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cable_cores: Vec<NetCableCore>,
```

Add after `struct Net`:

```rust
/// ネットを通る区間ケーブルの芯1本。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NetCableCore {
    pub cable_id: EntityId,
    /// ケーブルの参照記号 (例 "W45")。
    pub reference: String,
    /// 芯の呼び (例 "D-")。
    pub core: String,
}
```

In the `extract_netlist` doc list, add a rule line after `/// - 同名NetLabelのネットは統合され、ネット名はラベル名(辞書順最小)になる`:

```rust
/// - 区間ケーブル ([`crate::model::Cable::run`]) の芯は、芯の行き先 ([`CoreEnd`]) が指すピン・配線と
///   だけ繋がる。区間の線は位置では何にも繋がらない (ピン列の上を通っても短絡しない)
/// - 配線か区間ケーブルの芯を1つ以上含むまとまりをネットにする。芯だけのネットは配線のあるネットの後ろ
```

In the entity loop, add `let mut cables = Vec::new();` next to `let mut pins = Vec::new();` and add, before the no-op arm list, the arm:

```rust
            Entity::Cable(c) if c.is_run() => cables.push(c),
```

Replace `let mut ds = DisjointSet::new(li + labels.len());` with:

```rust
    // 区間ケーブルの芯: 1芯1ノード (ノード番号は labels の後ろ)
    let ci = li + labels.len();
    let cores: Vec<(&crate::model::Cable, &crate::model::CableCore)> = cables
        .iter()
        .flat_map(|c| c.cores.iter().map(move |k| (*c, k)))
        .collect();
    let mut ds = DisjointSet::new(ci + cores.len());
```

After the label loop (the `for (l, lab) in labels.iter().enumerate() { … }` block) add:

```rust
    // 芯 ⇔ 行き先 (割当てだけで結ぶ。区間の線はどの位置判定にも出てこない)
    for (k, (_, core)) in cores.iter().enumerate() {
        for end in [&core.end_a, &core.end_b].into_iter().flatten() {
            match end {
                CoreEnd::Wire { wire } => {
                    if let Some(a) = wires.iter().position(|w| w.id == *wire) {
                        ds.union(ci + k, wi + a);
                    }
                }
                // 同じ番号の接続点は上で内部短絡済みなので、番号の一致だけを見る (index は描画用)
                CoreEnd::Pin { part, pin, .. } => {
                    for (p, (eid, _, no, _)) in pins.iter().enumerate() {
                        if eid == part && no == pin {
                            ds.union(ci + k, pi + p);
                        }
                    }
                }
            }
        }
    }
```

Change the group comment and struct:

```rust
    // グループ化 (配線か区間ケーブルの芯を1つ以上含むグループだけをネットにする)
    #[derive(Default)]
    struct Group {
        wire_ids: Vec<EntityId>,
        pins: Vec<NetPin>,
        label: Option<String>,
        /// グループ内のWireに書かれた線番 (重複可)。
        wire_nos: Vec<String>,
        cable_cores: Vec<NetCableCore>,
    }
```

After the `for (a, w) in wires.iter().enumerate() { … }` group loop add:

```rust
    for (k, (c, core)) in cores.iter().enumerate() {
        groups.entry(ds.find(ci + k)).or_default().cable_cores.push(NetCableCore {
            cable_id: c.id,
            reference: c.reference.clone(),
            core: core.core.clone(),
        });
    }
```

Replace `ordered.sort_by_key(|g| g.wire_ids.iter().min().copied());` with:

```rust
    // 配線のあるネットが先 (これまでと同じ順・同じ自動名)、芯だけのネットはその後ろにケーブルid・芯の順
    ordered.sort_by_key(|g| {
        (
            g.wire_ids.is_empty(),
            g.wire_ids.iter().min().copied(),
            g.cable_cores.iter().map(|c| (c.cable_id, c.core.clone())).min(),
        )
    });
```

In the `.map(|mut g| { … })` closure, add after `g.pins.sort_by(…);`:

```rust
            g.cable_cores.sort_by(|a, b| (a.cable_id, &a.core).cmp(&(b.cable_id, &b.core)));
```

and add `cable_cores: g.cable_cores,` to the `Net { … }` literal after `wire_no,`.

In `src/ipc.ts`, add to `interface Net` after `wire_no?: string | null;`:

```ts
  /** このネットを通る区間ケーブルの芯 (v17)。 */
  cable_cores?: NetCableCore[];
```

and after the `Net` interface:

```ts
/** ネットを通る区間ケーブルの芯1本 (madake-coreの`NetCableCore`)。 */
export interface NetCableCore {
  cable_id: string;
  reference: string;
  core: string;
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core`
Expected: PASS (`core_only_nets_take_no_wire_number` is a guard: `plan_sheet` already skips nets without wires, so it may pass before Step 3 too).

- [ ] **Step 5: Regenerate the spec docs and commit**

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout
python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check
git add src-tauri/crates/madake-core/src/{netlist.rs,wire_no.rs} src/ipc.ts docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(core): ネットリストは区間ケーブルの芯を割当てだけで繋ぎ、芯だけのネットを認める"
```

---

### Task 4: ERC for run cables

**Files:**
- Modify: `src-tauri/crates/madake-core/src/cable.rs` (constants near line 20; `check_cables` at 207-227; new `breakout_point`, `breakout_points`, private `end_is_live`, `distance_to_part`)
- Modify: `src-tauri/crates/madake-core/src/verify.rs:624-682` (dangling ends)
- Test: `src-tauri/crates/madake-core/src/cable.rs` `mod tests`

**Interfaces:**
- Consumes: Task 2 `pin_end_position`, Task 3 `Net::cable_cores`.
- Produces:
  - `pub const CABLE_CORES_SHORTED: &str = "erc.cable_cores_shorted"`, `pub const CABLE_CORE_UNASSIGNED: &str = "erc.cable_core_unassigned"`, `pub const CABLE_RUN_END_DETACHED: &str = "erc.cable_run_end_detached"`, `pub const RUN_END_DETACH_MM: f64 = 2.5`
  - `pub fn breakout_point(sheet: &Sheet, c: &Cable, end: RunEnd, wire: EntityId) -> Option<Point>`
  - `pub fn breakout_points(sheet: &Sheet) -> Vec<Point>`

- [ ] **Step 1: Write the failing tests**

Append to `mod tests` in `cable.rs`:

```rust
    /// The LTE harness drawn with a single-line run passes ERC: no short, no unconnected plug pin, no dangling breakout wire and no cable finding.
    /// 1本線の区間で描いたLTEハーネスはERCを通る: 短絡も、未接続のプラグのピンも、宙ぶらりんの分岐の配線も、ケーブルの指摘も出ない。
    #[test]
    fn the_lte_harness_passes_erc() {
        let lte = test_support::lte_sheet();
        let diags = crate::verify::verify_sheet(&lte.sheet, &sheet_symbol_defs(&lte.sheet));
        let erc: Vec<_> = diags.iter().filter(|d| d.code.starts_with("erc.")).collect();
        assert!(erc.is_empty(), "{erc:?}");
    }

    /// Two cores of one cable that end up in the same net are an ERC error naming the cable and both cores, for a run cable and for a definition-line cable alike.
    /// 1本のケーブルの芯2本が同じネットにあるとERCのエラーになり、ケーブルと両方の芯を示す (区間ケーブルでも定義線ケーブルでも)。
    #[test]
    fn two_cores_in_one_net_is_a_short() {
        let mut lte = test_support::lte_sheet();
        // J45 のピン1 (VBUS) とピン2 (D-) を配線で直結する
        test_support::insert(&mut lte.sheet, Entity::Wire(test_support::wire(&[(57.5, 92.5), (57.5, 97.5)])));
        let diags = check_cables(&lte.sheet);
        let short: Vec<_> = diags.iter().filter(|d| d.code == CABLE_CORES_SHORTED).collect();
        assert_eq!(short.len(), 1, "{diags:?}");
        assert_eq!(short[0].severity, Severity::Error);
        assert!(short[0].message.contains("W45") && short[0].message.contains("VBUS, D-"), "{}", short[0].message);

        let (mut engine, sheet_id, _) = three_wires();
        // 黒 (y=10) と白 (y=20) の配線を左端で繋ぐ
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: Entity::Wire(test_support::wire(&[(15.0, 10.0), (15.0, 20.0)])),
            })
            .unwrap();
        engine.execute(Command::AddCable { sheet_id, cable: cable(3, (50.0, 5.0), (50.0, 35.0)) }).unwrap();
        let diags = check_cables(engine.project().sheet(sheet_id).unwrap());
        assert!(diags.iter().any(|d| d.code == CABLE_CORES_SHORTED && d.message.contains("1, 2")), "{diags:?}");
    }

    /// A run-cable core whose wire or part has been deleted is an ERC error naming the cores.
    /// 区間ケーブルの芯が指す配線や部品が消えているとERCのエラーになり、その芯を示す。
    #[test]
    fn a_core_whose_target_is_gone_is_unassigned() {
        let mut lte = test_support::lte_sheet();
        lte.sheet.entities.remove(&lte.wires[2]);
        let diags = check_cables(&lte.sheet);
        let hit: Vec<_> = diags.iter().filter(|d| d.code == CABLE_CORE_UNASSIGNED).collect();
        assert_eq!(hit.len(), 1, "{diags:?}");
        assert_eq!(hit[0].severity, Severity::Error);
        assert!(hit[0].message.contains("D+"), "{}", hit[0].message);
        lte.sheet.entities.remove(&lte.usb);
        let diags = check_cables(&lte.sheet);
        let hit = diags.iter().find(|d| d.code == CABLE_CORE_UNASSIGNED).expect("未割当");
        assert!(hit.message.contains("VBUS, D-, D+, GND"), "{}", hit.message);
    }

    /// A single-line run whose end is far from the part its cores land on is warned about; one that touches the part is not.
    /// 芯が着く部品から1本線の端が離れていると警告になり、部品に触れていれば警告しない。
    #[test]
    fn a_run_end_away_from_its_part_is_warned() {
        let mut lte = test_support::lte_sheet();
        assert!(!check_cables(&lte.sheet).iter().any(|d| d.code == CABLE_RUN_END_DETACHED));
        lte.sheet.entities.get_mut(&lte.usb).unwrap().translate(40.0, 0.0);
        let diags = check_cables(&lte.sheet);
        let hit: Vec<_> = diags.iter().filter(|d| d.code == CABLE_RUN_END_DETACHED).collect();
        assert_eq!(hit.len(), 1, "{diags:?}");
        assert_eq!(hit[0].severity, Severity::Warning);
        assert!(hit[0].message.contains("USB1"), "{}", hit[0].message);
    }

    /// The breakout ends of a run cable's wires are not dangling; without the cable they would be.
    /// 区間ケーブルの芯の配線の分岐側の端は宙ぶらりんにならない (ケーブルが無ければ宙ぶらりん)。
    #[test]
    fn breakout_ends_are_not_dangling() {
        let mut lte = test_support::lte_sheet();
        let dangling = |sheet: &Sheet| {
            crate::verify::verify_sheet(sheet, &sheet_symbol_defs(sheet))
                .into_iter()
                .filter(|d| d.code == "erc.dangling_wire")
                .count()
        };
        assert_eq!(dangling(&lte.sheet), 0);
        lte.sheet.entities.remove(&lte.cable);
        assert_eq!(dangling(&lte.sheet), 4);
    }

    /// A run cable with more cores than its core count is the same core-overflow error as a definition-line cable.
    /// 芯数より多い芯を持つ区間ケーブルは、定義線ケーブルと同じ芯数超過のエラーになる。
    #[test]
    fn a_run_cable_with_too_many_cores_overflows() {
        let mut lte = test_support::lte_sheet();
        let Some(Entity::Cable(c)) = lte.sheet.entities.get_mut(&lte.cable) else { panic!("ケーブル") };
        c.core_count = 3;
        assert!(check_cables(&lte.sheet).iter().any(|d| d.code == CABLE_CORE_OVERFLOW));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core -- cable::tests`
Expected: FAIL to compile (`cannot find value CABLE_CORES_SHORTED`).

- [ ] **Step 3: Implement**

In `cable.rs`, add after `pub const CABLE_CORE_OVERFLOW: &str = "erc.cable_core_overflow";`:

```rust
/// ERC: 1本のケーブルの芯2本が同じネットにある (短絡)。
pub const CABLE_CORES_SHORTED: &str = "erc.cable_cores_shorted";
/// ERC: 区間ケーブルの芯の行き先が無い (端が未設定、または指す部品・ピン・配線が消えた)。
pub const CABLE_CORE_UNASSIGNED: &str = "erc.cable_core_unassigned";
/// ERC (警告): 芯が着く部品から1本線の端が離れている。
pub const CABLE_RUN_END_DETACHED: &str = "erc.cable_run_end_detached";
/// 1本線の端が部品の外形からこれより離れていたら警告する (mm)。
pub const RUN_END_DETACH_MM: f64 = 2.5;
```

Replace the whole `check_cables` function with:

```rust
/// シート上のケーブルのERC:
/// - 割り当てた芯が芯数を超えていれば Error ([`CABLE_CORE_OVERFLOW`]。区間ケーブルは芯の数)
/// - 1本のケーブルの芯2本が同じネットにあれば Error ([`CABLE_CORES_SHORTED`])
/// - 区間ケーブルの芯の行き先が無ければ Error ([`CABLE_CORE_UNASSIGNED`])
/// - 芯が着く部品から1本線の端が [`RUN_END_DETACH_MM`] より離れていれば Warning ([`CABLE_RUN_END_DETACHED`])
pub fn check_cables(sheet: &Sheet) -> Vec<Diagnostic> {
    let defs = sheet_symbol_defs(sheet);
    let nets = crate::netlist::extract_netlist(sheet, &defs);
    let mut out = Vec::new();
    for e in sheet.entities.values() {
        let Entity::Cable(c) = e else { continue };
        let diag = |severity: Severity, code: &str, message: String| Diagnostic {
            severity,
            code: code.into(),
            message,
            sheet_id: sheet.id,
            entity_ids: vec![c.id],
        };
        let used = if c.is_run() { c.cores.len() } else { live_cores(sheet, c).count() };
        if used > c.core_count {
            out.push(diag(
                Severity::Error,
                CABLE_CORE_OVERFLOW,
                format!(
                    "ケーブル {} に {} 本の配線が割り当てられていますが、芯数は {} です",
                    c.reference, used, c.core_count
                ),
            ));
        }
        // 芯2本が同じネット = 短絡 (偽の分岐ブロックや、1本の配線を複数のピンに触れさせた図で起こる)
        let mut by_net: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
        for core in &c.cores {
            let net = nets.iter().position(|n| {
                if c.is_run() {
                    n.cable_cores.iter().any(|k| k.cable_id == c.id && k.core == core.core)
                } else {
                    core.wire.is_some_and(|w| n.wire_ids.contains(&w))
                }
            });
            if let Some(i) = net {
                by_net.entry(i).or_default().push(core.core.as_str());
            }
        }
        for names in by_net.values().filter(|v| v.len() >= 2) {
            out.push(diag(
                Severity::Error,
                CABLE_CORES_SHORTED,
                format!(
                    "ケーブル {} の芯 {} が同じネットに繋がっています (短絡)",
                    c.reference,
                    names.join(", ")
                ),
            ));
        }
        if !c.is_run() {
            continue;
        }
        let missing: Vec<&str> = c
            .cores
            .iter()
            .filter(|k| RunEnd::BOTH.iter().any(|&end| !end_is_live(sheet, &defs, k.end(end))))
            .map(|k| k.core.as_str())
            .collect();
        if !missing.is_empty() {
            out.push(diag(
                Severity::Error,
                CABLE_CORE_UNASSIGNED,
                format!(
                    "ケーブル {} の芯 {} の行き先がありません (端が未設定か、指す部品・ピン・配線が消えています)",
                    c.reference,
                    missing.join(", ")
                ),
            ));
        }
        for end in RunEnd::BOTH {
            let Some(at) = c.run_end(end) else { continue };
            let mut parts: Vec<EntityId> = Vec::new();
            for core in &c.cores {
                if let Some(CoreEnd::Pin { part, .. }) = core.end(end) {
                    if !parts.contains(part) {
                        parts.push(*part);
                    }
                }
            }
            for part in parts {
                let Some(d) = distance_to_part(sheet, &defs, part, at) else { continue };
                if d > RUN_END_DETACH_MM {
                    let reference = match sheet.entities.get(&part) {
                        Some(Entity::Symbol(s)) => s.reference.clone(),
                        _ => part.to_string(),
                    };
                    out.push(diag(
                        Severity::Warning,
                        CABLE_RUN_END_DETACHED,
                        format!(
                            "ケーブル {} の1本線の端 {} が {} から {:.1}mm 離れています (線がどこへ行くか図から読めません)",
                            c.reference,
                            end.letter().to_uppercase(),
                            reference,
                            d
                        ),
                    ));
                }
            }
        }
    }
    out
}

/// 芯の行き先が今もシートにあるか (配線がある・部品にそのピンがある)。
fn end_is_live(sheet: &Sheet, defs: &[SymbolDef], end: Option<&CoreEnd>) -> bool {
    match end {
        None => false,
        Some(CoreEnd::Wire { wire }) => matches!(sheet.entities.get(wire), Some(Entity::Wire(_))),
        Some(pin @ CoreEnd::Pin { .. }) => pin_end_position(sheet, defs, pin).is_some(),
    }
}

/// 点から部品の外形 (図形とピンを含む外接矩形) までの距離 mm。部品が無ければNone。
fn distance_to_part(sheet: &Sheet, defs: &[SymbolDef], part: EntityId, at: Point) -> Option<f64> {
    let Some(Entity::Symbol(s)) = sheet.entities.get(&part) else {
        return None;
    };
    let def = crate::symbol::instance_symbol_def(s, defs)?;
    let pins = crate::pin_wiring::part_pins(s, &def);
    let mut points: Vec<Point> = pins.pins.iter().map(|p| Point::new(p.x, p.y)).collect();
    if let Some(b) = pins.bbox {
        points.push(b.min);
        points.push(b.max);
    }
    let first = *points.first()?;
    let mut rect = crate::tidy::Rect::from_corners(first, first);
    for p in points {
        rect.extend(p);
    }
    let dx = (rect.min.x - at.x).max(0.0).max(at.x - rect.max.x);
    let dy = (rect.min.y - at.y).max(0.0).max(at.y - rect.max.y);
    Some((dx * dx + dy * dy).sqrt())
}

/// 分岐側の端点: 芯の配線`wire`の端点のうち、区間の端`end`に近い方。配線か区間が無ければNone。
pub fn breakout_point(sheet: &Sheet, c: &Cable, end: RunEnd, wire: EntityId) -> Option<Point> {
    let at = c.run_end(end)?;
    let Some(Entity::Wire(w)) = sheet.entities.get(&wire) else {
        return None;
    };
    let (first, last) = (w.points.first()?, w.points.last()?);
    Some(if first.distance_to(&at) <= last.distance_to(&at) { *first } else { *last })
}

/// シート上の区間ケーブルの芯の配線の、分岐側の端点すべて (ERCの宙ぶらりん判定はここを繋がっていると数える)。
pub fn breakout_points(sheet: &Sheet) -> Vec<Point> {
    let mut out = Vec::new();
    for e in sheet.entities.values() {
        let Entity::Cable(c) = e else { continue };
        for core in &c.cores {
            for end in RunEnd::BOTH {
                if let Some(CoreEnd::Wire { wire }) = core.end(end) {
                    out.extend(breakout_point(sheet, c, end, *wire));
                }
            }
        }
    }
    out
}
```

In `verify.rs`, after the entity loop that fills `pin_points`/`junctions`/`labels`/`wires` (just before `for w in &wires {`), add:

```rust
    // 区間ケーブルの芯の配線の分岐側の端は、芯の割当てで繋がっている (位置で繋がる相手は無い)
    let breakouts = crate::cable::breakout_points(sheet);
```

and in the `attached` expression, add a line after `|| labels.iter().any(|p| near(p, ep))`:

```rust
                    || breakouts.iter().any(|p| near(p, ep))
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core`
Expected: PASS (including the existing `more_assigned_wires_than_cores_is_an_erc_error`).

- [ ] **Step 5: Regenerate the spec docs and commit**

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout
python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check
git add src-tauri/crates/madake-core/src/{cable.rs,verify.rs} docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(core): 区間ケーブルのERC (芯の短絡・行き先なし・端の離れ) と分岐の端の宙ぶらりん除外"
```

---

### Task 5: Wire list per core, cable list, BOM roll-up

**Files:**
- Modify: `src-tauri/crates/madake-core/src/reports.rs` (imports 1-6; `WIRE_LIST_COLUMNS` 332-344; `wire_from_to` 454-506; `wire_list_rows` 510-544; `wire_rollup_rows` 275-310; test `wire_list_is_a_from_to_list` 933-944)
- Modify: `src-tauri/crates/madake-core/src/cable.rs` (`cable_list_rows`, new helpers)
- Modify: `src-tauri/crates/madake-core/src/report_sheet.rs:538-540`
- Modify: `src-tauri/crates/madake-mcp/src/lib.rs:1573` (`export_wire_list` description)
- Test: `reports.rs`, `cable.rs`

**Interfaces:**
- Consumes: Task 4 `breakout_point`.
- Produces:
  - `pub const WIRE_LIST_COLUMNS: [&str; 12]` (last = `"ケーブル芯"`)
  - `pub fn breakout_far_point(sheet: &Sheet, c: &Cable, end: RunEnd, wire: EntityId) -> Option<Point>`
  - `pub fn breakout_wire_ids(sheet: &Sheet) -> BTreeSet<EntityId>`, `pub fn core_wire_ids(sheet: &Sheet) -> BTreeSet<EntityId>`, `pub fn definition_core_labels(sheet: &Sheet) -> BTreeMap<EntityId, String>`

- [ ] **Step 1: Write the failing tests**

In `reports.rs` replace the body of `wire_list_is_a_from_to_list` (keep its doc lines):

```rust
    #[test]
    fn wire_list_is_a_from_to_list() {
        let project = one_wire_project(|_| {});
        let csv = wire_list_csv(&project);
        assert_eq!(
            csv.lines().next().unwrap(),
            "シート,From,To,嵌合相手,備考,線番,線色,線径sq,長さm,電線品番,ハーネス,ケーブル芯"
        );
        assert_eq!(WIRE_LIST_COLUMNS.len(), 12);
    }
```

Append to `mod tests` in `reports.rs`:

```rust
    /// A single-line cable gives one From-To row per core, end to end: the board pin reached through the breakout wire, the plug pin the core lands on, the core colour, the cable gauge, length and part, the cable as harness and the "cable/core" column; its breakout wires get no rows of their own.
    /// 1本線のケーブルは芯ごとに端から端まで1行になる: 分岐の配線の先の基板のピン、芯が着くプラグのピン、芯の色、ケーブルの線径・長さ・型番、ハーネス列にケーブル、「ケーブル/芯」の列。分岐の配線だけの行は出ない。
    #[test]
    fn a_run_cable_gives_one_wire_list_row_per_core() {
        let lte = crate::cable::test_support::lte_sheet();
        let project = crate::cable::test_support::project_with(lte.sheet);
        let rows = wire_list_rows(&project);
        assert_eq!(rows.len(), 4, "{rows:?}");
        assert_eq!(
            rows[0],
            ["Sheet1", "J45:1", "USB1:1", "", "", "", "red", "0.2", "0.3", "", "W45", "W45/VBUS"]
                .map(String::from)
                .to_vec()
        );
        assert_eq!(rows[3][11], "W45/GND");
    }

    /// When a breakout wire ends on a net label and nothing else on the sheet carries that label (the continuation is on another sheet), the core's row shows the label name and the "none" remark.
    /// 分岐の配線の先がネットラベルで、シート上にそのラベルを持つ他の相手が無いとき (続きは別シート)、芯の行はラベル名と備考「none」を出す。
    #[test]
    fn a_breakout_wire_ending_on_a_label_shows_the_label() {
        let mut lte = crate::cable::test_support::lte_sheet();
        // VBUS の分岐の配線を J45 から外し、上へ (77.5, 60) まで延ばしてネットラベルで終える
        // (水平のままだと線分が J45 のピン1 (57.5, 92.5) の上を通り、ピンに繋がってしまう)
        if let Some(Entity::Wire(w)) = lte.sheet.entities.get_mut(&lte.wires[0]) {
            w.points = vec![Point::new(77.5, 92.5), Point::new(77.5, 60.0)];
        }
        let label = Entity::NetLabel(NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(77.5, 60.0),
            name: "LTE_VBUS".into(),
            rotation: 0,
            attrs: Default::default(),
        });
        crate::cable::test_support::insert(&mut lte.sheet, label);
        let project = crate::cable::test_support::project_with(lte.sheet);
        let rows = wire_list_rows(&project);
        let vbus = rows.iter().find(|r| r[11] == "W45/VBUS").expect("VBUS の行");
        assert_eq!(
            (vbus[1].as_str(), vbus[2].as_str(), vbus[4].as_str()),
            ("LTE_VBUS", "USB1:1", REMARK_NO_DEVICE)
        );
    }

    /// Wires that are cores of a cable (breakout wires of a single-line cable, and wires crossed by a definition line) are not added again to the BOM wire roll-up, since the cable row already counts them.
    /// ケーブルの芯になっている配線 (1本線のケーブルの分岐の配線と、定義線が横切った配線) は、ケーブル行が数えているので部品表の電線集約に重ねて足さない。
    #[test]
    fn cable_core_wires_are_not_counted_as_loose_wire() {
        let mut lte = crate::cable::test_support::lte_sheet();
        for id in &lte.wires {
            if let Some(Entity::Wire(w)) = lte.sheet.entities.get_mut(id) {
                w.part_no = Some("WIRE-A".into());
                w.length_m = Some(0.5);
            }
        }
        let mut loose = crate::cable::test_support::wire(&[(10.0, 200.0), (30.0, 200.0)]);
        loose.part_no = Some("WIRE-A".into());
        loose.length_m = Some(1.0);
        let loose_id = crate::cable::test_support::insert(&mut lte.sheet, Entity::Wire(loose));
        let mut project = crate::cable::test_support::project_with(lte.sheet);
        let wire_rows = |p: &Project| {
            bom_rows(p)
                .into_iter()
                .filter(|r| r[3] == BOM_WIRE_KIND)
                .map(|r| r[..5].to_vec())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            wire_rows(&project),
            vec![vec![String::new(), String::new(), "WIRE-A".into(), BOM_WIRE_KIND.into(), "1.00 m".into()]]
        );
        // 定義線ケーブルで遊びの配線を芯1にすると、その配線も数えない
        let defline = Cable {
            id: Uuid::new_v4(),
            reference: "W1".into(),
            part_no: String::new(),
            core_count: 1,
            sq: 0.0,
            points: vec![Point::new(20.0, 195.0), Point::new(20.0, 205.0)],
            cores: vec![CableCore { core: "1".into(), wire: Some(loose_id), ..Default::default() }],
            shield: None,
            length_m: None,
            run: Vec::new(),
        };
        crate::cable::test_support::insert(&mut project.sheets[0], Entity::Cable(defline));
        assert!(wire_rows(&project).is_empty(), "{:?}", wire_rows(&project));
    }
```

Append to `mod tests` in `cable.rs`:

```rust
    /// The cable list names the devices at each end of a single-line cable (the board its breakout wires go to, the plug its run lands on) and counts the cores that have both ends as used.
    /// ケーブル一覧は1本線のケーブルの両端の機器 (分岐の配線が行く基板、1本線が着くプラグ) を示し、両端のある芯を使用芯と数える。
    #[test]
    fn the_cable_list_shows_the_ends_of_a_run_cable() {
        let lte = test_support::lte_sheet();
        let project = test_support::project_with(lte.sheet);
        assert_eq!(
            cable_list_rows(&project),
            vec![["W45", "", "4", "0.2", "0.3", "J45", "USB1", "4", "0", "", "Sheet1"].map(String::from).to_vec()]
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core -- reports:: cable::tests::the_cable_list_shows_the_ends_of_a_run_cable`
Expected: FAIL (header has 11 columns; LTE gives 4 breakout-wire rows with empty `To`; BOM shows `3.00 m`; the cable list From/To are empty).

- [ ] **Step 3: Implement the cable helpers (`cable.rs`)**

Change `use std::collections::BTreeMap;` to `use std::collections::{BTreeMap, BTreeSet};`. Add after `breakout_points`:

```rust
/// 分岐の配線の反対側 (部品や端子台へ行く側) の端点。
pub fn breakout_far_point(sheet: &Sheet, c: &Cable, end: RunEnd, wire: EntityId) -> Option<Point> {
    let near = breakout_point(sheet, c, end, wire)?;
    let Some(Entity::Wire(w)) = sheet.entities.get(&wire) else {
        return None;
    };
    let (a, b) = (w.points.first()?, w.points.last()?);
    Some(if a.distance_to(&near) < 1e-9 { *b } else { *a })
}

/// 区間ケーブルの分岐の配線 (芯そのもの。電線リストでは芯の行に畳み込む)。
pub fn breakout_wire_ids(sheet: &Sheet) -> BTreeSet<EntityId> {
    let mut out = BTreeSet::new();
    for e in sheet.entities.values() {
        let Entity::Cable(c) = e else { continue };
        for core in &c.cores {
            for end in RunEnd::BOTH {
                if let Some(CoreEnd::Wire { wire }) = core.end(end) {
                    out.insert(*wire);
                }
            }
        }
    }
    out
}

/// どれかのケーブルの芯になっている配線 (定義線ケーブルの芯の配線と、区間ケーブルの分岐の配線)。
/// 部品表の電線集約から除く (ケーブル行がもう数えている)。
pub fn core_wire_ids(sheet: &Sheet) -> BTreeSet<EntityId> {
    let mut out = breakout_wire_ids(sheet);
    out.extend(definition_core_labels(sheet).into_keys());
    out
}

/// 定義線ケーブルの芯の配線 → "W1/1" (電線リストの「ケーブル芯」列)。
pub fn definition_core_labels(sheet: &Sheet) -> BTreeMap<EntityId, String> {
    let mut out = BTreeMap::new();
    for e in sheet.entities.values() {
        let Entity::Cable(c) = e else { continue };
        for core in &c.cores {
            if let Some(w) = core.wire {
                out.insert(w, format!("{}/{}", c.reference, core.core));
            }
        }
    }
    out
}

/// 使っている芯の数: 定義線ケーブルは配線が今もある芯、区間ケーブルは両端のある芯。
fn used_cores(sheet: &Sheet, c: &Cable) -> usize {
    if c.is_run() {
        c.cores.iter().filter(|k| k.end_a.is_some() && k.end_b.is_some()).count()
    } else {
        live_cores(sheet, c).count()
    }
}

/// 区間ケーブルの端`end`側の機器 (ピンの行き先は部品の参照記号、分岐の配線は反対側の端の機器)。
fn run_end_devices(sheet: &Sheet, c: &Cable, end: RunEnd) -> String {
    let defs = sheet_symbol_defs(sheet);
    let mut out: Vec<String> = Vec::new();
    for core in &c.cores {
        let device = match core.end(end) {
            Some(CoreEnd::Pin { part, .. }) => match sheet.entities.get(part) {
                Some(Entity::Symbol(s)) => s.reference.clone(),
                _ => String::new(),
            },
            Some(CoreEnd::Wire { wire }) => breakout_far_point(sheet, c, end, *wire)
                .map(|far| endpoint_label(sheet, &defs, &far))
                .and_then(|label| label.split(':').next().map(|d| d.trim().to_string()))
                .unwrap_or_default(),
            None => String::new(),
        };
        if !device.is_empty() && !out.contains(&device) {
            out.push(device);
        }
    }
    out.sort();
    out.join(", ")
}
```

In `cable_list_rows`, replace

```rust
            let wires: Vec<&crate::model::Wire> = live_cores(sheet, c).collect();
            let used = wires.len();
```

with

```rust
            let (from, to) = if c.is_run() {
                (run_end_devices(sheet, c, RunEnd::A), run_end_devices(sheet, c, RunEnd::B))
            } else {
                let wires: Vec<&crate::model::Wire> = live_cores(sheet, c).collect();
                (end_devices(sheet, &wires, true), end_devices(sheet, &wires, false))
            };
            let used = used_cores(sheet, c);
```

and in the row `vec![…]` replace `end_devices(sheet, &wires, true),` with `from,` and `end_devices(sheet, &wires, false),` with `to,`.

- [ ] **Step 4: Implement the reports (`reports.rs`)**

Imports: `use crate::model::{Cable, CableCore, CoreEnd, Entity, Project, RunEnd, Sheet, Wire};`

Replace `WIRE_LIST_COLUMNS` with (keep the doc comment above it and append one line to it: `/// 末尾の「ケーブル芯」(v17) はケーブルの芯の配線・区間ケーブルの芯の行で "W45/D-" になる。`):

```rust
pub const WIRE_LIST_COLUMNS: [&str; 12] = [
    "シート",
    "From",
    "To",
    "嵌合相手",
    "備考",
    "線番",
    "線色",
    "線径sq",
    "長さm",
    "電線品番",
    "ハーネス",
    "ケーブル芯",
];
```

Replace `wire_from_to` (from `pub fn wire_from_to(` to its closing `}`) with:

```rust
pub fn wire_from_to(
    sheet: &Sheet,
    defs: &[SymbolDef],
    nets: &[Net],
    wire: &Wire,
) -> (String, String, String, String) {
    let (Some(a), Some(b)) = (wire.points.first(), wire.points.last()) else {
        return (String::new(), String::new(), String::new(), String::new());
    };
    from_to(sheet, nets, [resolve_endpoint(sheet, defs, a), resolve_endpoint(sheet, defs, b)])
}

/// 両端の解決結果から (From, To, 嵌合相手, 備考) を組み立てる ([`wire_from_to`] と区間ケーブルの
/// 芯の行が共有する。規則は [`wire_from_to`] の説明のとおり)。
fn from_to(sheet: &Sheet, nets: &[Net], kinds: [EndKind; 2]) -> (String, String, String, String) {
    let mut multi = false;
    let mut no_device = false;
    let mut resolved: Vec<String> = Vec::with_capacity(2);
    for (i, kind) in kinds.iter().enumerate() {
        let text = match kind {
            EndKind::Pin { reference, pin, .. } => format!("{reference}:{pin}"),
            EndKind::None => String::new(),
            EndKind::Label(name) => {
                let exclude = match &kinds[1 - i] {
                    EndKind::Pin { entity_id, pin, .. } => Some((*entity_id, pin.clone())),
                    _ => None,
                };
                let (text, mark) = resolve_label_end(nets, name, exclude.as_ref());
                multi |= mark == REMARK_MULTI_ENDED;
                no_device |= mark == REMARK_NO_DEVICE;
                text
            }
        };
        resolved.push(text);
    }

    // From/To を決めてから嵌合相手列を組み立てる (描いた向きで並びが変わらないように)
    resolved.sort_by(|x, y| (x.is_empty(), x.as_str()).cmp(&(y.is_empty(), y.as_str())));
    let mut mates: Vec<String> = Vec::new();
    for text in &resolved {
        if let Some(reference) = reference_of(text) {
            let m = crate::mate::declared_mate(sheet, reference);
            if !m.is_empty() && !mates.contains(&m) {
                mates.push(m);
            }
        }
    }

    let [from, to] = [resolved[0].clone(), resolved[1].clone()];
    let remark = if multi {
        REMARK_MULTI_ENDED
    } else if no_device {
        REMARK_NO_DEVICE
    } else {
        ""
    };
    (from, to, mates.join(", "), remark.to_string())
}

/// 区間ケーブルの芯の端`end`の行き先。ピンの行き先はそのピン、分岐の配線はその配線の反対側の端を
/// 配線と同じに解決する (ピンかネットラベル)。
fn core_end_kind(sheet: &Sheet, defs: &[SymbolDef], c: &Cable, core: &CableCore, end: RunEnd) -> EndKind {
    match core.end(end) {
        Some(CoreEnd::Pin { part, pin, .. }) => match sheet.entities.get(part) {
            Some(Entity::Symbol(s)) => EndKind::Pin {
                entity_id: s.id,
                reference: s.reference.clone(),
                pin: pin.clone(),
            },
            _ => EndKind::None,
        },
        Some(CoreEnd::Wire { wire }) => match crate::cable::breakout_far_point(sheet, c, end, *wire) {
            Some(far) => resolve_endpoint(sheet, defs, &far),
            None => EndKind::None,
        },
        None => EndKind::None,
    }
}
```

In `wire_list_rows`, replace the `let mut sheet_rows: Vec<Vec<String>> = sheet … .collect();` statement with:

```rust
        // 区間ケーブルの分岐の配線は芯の行に畳み込む (別の行にしない)
        let breakout = crate::cable::breakout_wire_ids(sheet);
        let core_labels = crate::cable::definition_core_labels(sheet);
        let mut sheet_rows: Vec<Vec<String>> = sheet
            .entities
            .values()
            .filter_map(|e| match e {
                Entity::Wire(w) if !breakout.contains(&w.id) => Some(w),
                _ => None,
            })
            .map(|w| {
                let (from, to, mate, remark) = wire_from_to(sheet, &defs, &nets, w);
                vec![
                    sheet.name.clone(),
                    from,
                    to,
                    mate,
                    remark,
                    w.net.clone().unwrap_or_default(),
                    w.color.clone(),
                    fmt_num(w.sq),
                    w.length_m.map(fmt_num).unwrap_or_default(),
                    crate::wire_parts::wire_part_number(project, w).unwrap_or_default(),
                    crate::harness::harness_name_of_wire(sheet, w),
                    core_labels.get(&w.id).cloned().unwrap_or_default(),
                ]
            })
            .collect();
        // 区間ケーブルは芯1本につき端から端まで1行 (線色=芯、線径・長さ・型番=ケーブル、ハーネス=ケーブル)
        for e in sheet.entities.values() {
            let Entity::Cable(c) = e else { continue };
            if !c.is_run() {
                continue;
            }
            for core in &c.cores {
                let kinds = [
                    core_end_kind(sheet, &defs, c, core, RunEnd::A),
                    core_end_kind(sheet, &defs, c, core, RunEnd::B),
                ];
                let (from, to, mate, remark) = from_to(sheet, &nets, kinds);
                let wire_no = RunEnd::BOTH
                    .iter()
                    .find_map(|&end| match core.end(end) {
                        Some(CoreEnd::Wire { wire }) => match sheet.entities.get(wire) {
                            Some(Entity::Wire(w)) => w.net.clone(),
                            _ => None,
                        },
                        _ => None,
                    })
                    .unwrap_or_default();
                sheet_rows.push(vec![
                    sheet.name.clone(),
                    from,
                    to,
                    mate,
                    remark,
                    wire_no,
                    core.color.clone(),
                    fmt_num(c.sq),
                    c.length_m.map(fmt_num).unwrap_or_default(),
                    c.part_no.clone(),
                    c.reference.clone(),
                    format!("{}/{}", c.reference, core.core),
                ]);
            }
        }
```

In `wire_rollup_rows`, replace

```rust
    for sheet in &project.sheets {
        for entity in sheet.entities.values() {
            let Entity::Wire(w) = entity else { continue };
```

with

```rust
    for sheet in &project.sheets {
        // ケーブルの芯の配線はケーブル行が数えるので、電線として重ねて足さない
        let cores = crate::cable::core_wire_ids(sheet);
        for entity in sheet.entities.values() {
            let Entity::Wire(w) = entity else { continue };
            if cores.contains(&w.id) {
                continue;
            }
```

In `report_sheet.rs:538-540` replace the two comment lines and ratios with:

```rust
        // シート・From・To・電線品番を広めに、線色/sq/長さ/備考を狭く
        // (シート,From,To,嵌合相手,備考,線番,線色,線径sq,長さm,電線品番,ハーネス,ケーブル芯)
        .with_col_ratios(&[8.0, 14.0, 14.0, 10.0, 8.0, 7.0, 6.0, 5.0, 5.0, 12.0, 8.0, 8.0]);
```

In `madake-mcp/src/lib.rs:1573`, change the `export_wire_list` description text `(列: シート/From/To/嵌合相手/備考/線番/線色/線径sq/長さm/電線品番/ハーネス)` to `(列: シート/From/To/嵌合相手/備考/線番/線色/線径sq/長さm/電線品番/ハーネス/ケーブル芯。1本線のケーブルは芯ごとに端から端まで1行)`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core && cargo test -p madake-mcp`
Expected: PASS.

- [ ] **Step 6: Regenerate the spec docs and commit**

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout
python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check
git add src-tauri/crates/madake-core/src/{reports.rs,cable.rs,report_sheet.rs} src-tauri/crates/madake-mcp/src/lib.rs docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(core): 電線リストを芯ごとの行にしケーブル芯の列を足す、ケーブルの芯の配線を部品表で二重に数えない"
```

---

### Task 6: Planner `plan_cable_run`

**Files:**
- Create: `src-tauri/crates/madake-core/src/cable_run.rs`
- Modify: `src-tauri/crates/madake-core/src/lib.rs` (add `pub mod cable_run;` after `pub mod cable;`)
- Modify: `src-tauri/crates/madake-core/src/pin_wiring.rs:325` (`fn pins_on_route` → `pub(crate) fn pins_on_route`)
- Modify: `src-tauri/crates/madake-core/src/cable.rs` (new `next_cable_reference`, `bar_ends`)

**Interfaces:**
- Consumes: Task 2 `validate_run_cable`; `pin_wiring::{find_parts, resolve_pin, route, pins_on_route, ResolvedPin, ROUTE_GRID}`; `netlist::point_is_attached`.
- Produces (used by Task 7 and Task 8):
  - `pub enum SideStyle { Multi, Single }` (serde snake_case)
  - `pub struct CableSide { pub pins: Option<Vec<String>>, pub part: Option<String>, pub style: SideStyle, pub breakout_at: Option<Point> }`
  - `pub enum CoreSpec { Name(String), Full { core: String, color: Option<String> } }` (serde untagged)
  - `pub struct CableRunSpec { pub reference: Option<String>, pub part_no: Option<String>, pub sq: Option<f64>, pub length_m: Option<f64>, pub shield: Option<CableShield>, pub color: Option<String>, pub cores: Vec<CoreSpec>, pub a: CableSide, pub b: CableSide, pub via: Option<Vec<Point>> }`
  - `pub struct PlannedCable { pub cable: Cable, pub wires: Vec<Wire>, pub ends: Vec<[String; 2]> }`, `impl PlannedCable { pub fn entities(&self) -> Vec<Entity> }` (wires first, cable last)
  - `pub fn plan_cable_run(sheet: &Sheet, defs: &[SymbolDef], spec: &CableRunSpec) -> Result<PlannedCable, String>`
  - `pub const BREAKOUT_OFFSET_MM: f64 = 10.0`, `pub const FAN_LENGTH_MM: f64 = 2.5`
  - `crate::cable::next_cable_reference(sheet: &Sheet) -> String`, `crate::cable::bar_ends(positions: &[Point]) -> Option<(Point, Point)>`

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/crates/madake-core/src/cable_run.rs` containing only the test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cable::test_support::{connector, insert};
    use crate::model::{Orientation, PaperSize, SymbolInstance};
    use crate::symbol::sheet_symbol_defs;

    fn spec(json: serde_json::Value) -> CableRunSpec {
        serde_json::from_value(json).unwrap()
    }

    /// J45 (基板、ピンは x=57.5 で右向き) と USB1 (反転して (200,100)、ピンは x=192.5 で左向き)。
    fn board_and_plug() -> Sheet {
        let mut sheet = Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape);
        insert(&mut sheet, Entity::Symbol(connector(4, "J45", 50.0, 100.0, false)));
        insert(&mut sheet, Entity::Symbol(connector(4, "USB1", 200.0, 100.0, true)));
        sheet
    }

    /// Planning the LTE harness draws one wire per core from the board pins to a breakout 10 mm out, runs one line from there to the plug, and maps the cores to the pins in pin order; the result is four nets.
    /// LTEハーネスを計画すると、基板のピンから10mm先の分岐点まで芯ごとに1本ずつ配線を引き、そこからプラグまで1本線を通し、芯をピン順にピンへ対応させる。結果は4ネットになる。
    #[test]
    fn plans_the_lte_harness() {
        let mut sheet = board_and_plug();
        let defs = sheet_symbol_defs(&sheet);
        let planned = plan_cable_run(
            &sheet,
            &defs,
            &spec(serde_json::json!({
                "reference": "W45", "sq": 0.2,
                "cores": ["VBUS", "D-", {"core": "D+", "color": "green"}, "GND"],
                "a": {"part": "J45", "style": "multi"},
                "b": {"part": "USB1", "style": "single"}
            })),
        )
        .unwrap();
        assert_eq!(planned.wires.len(), 4);
        assert_eq!(planned.wires[0].points, vec![Point::new(57.5, 92.5), Point::new(65.0, 92.5)]);
        assert_eq!(planned.cable.run, vec![Point::new(67.5, 100.0), Point::new(192.5, 100.0)]);
        assert_eq!(planned.ends[1], ["J45.2".to_string(), "USB1.2".to_string()]);
        assert_eq!(planned.cable.cores[2].color, "green");
        assert_eq!(planned.wires[2].color, "green");
        for e in planned.entities() {
            insert(&mut sheet, e);
        }
        crate::cable::validate_run_cable(&sheet, &planned.cable).unwrap();
        let nets = crate::netlist::extract_netlist(&sheet, &defs);
        assert_eq!(nets.len(), 4);
        assert!(nets.iter().all(|n| n.pins.len() == 2));
    }

    /// Two connectors joined end to end by one line (3.5 mm AUX) get a run between their pin columns and no wires.
    /// 全長1本線で結ぶ2つのコネクタ (3.5mm AUX) は、ピン列の間に1本線が通り、配線は無い。
    #[test]
    fn plans_a_single_line_between_two_connectors() {
        let mut sheet = Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape);
        let j1 = insert(&mut sheet, Entity::Symbol(connector(3, "J1", 50.0, 50.0, false)));
        insert(&mut sheet, Entity::Symbol(connector(3, "J2", 150.0, 50.0, true)));
        let defs = sheet_symbol_defs(&sheet);
        let planned = plan_cable_run(
            &sheet,
            &defs,
            &spec(serde_json::json!({
                "cores": ["L", "R", "GND"],
                "a": {"part": "J1", "style": "single"},
                "b": {"part": "J2", "style": "single"}
            })),
        )
        .unwrap();
        assert!(planned.wires.is_empty());
        assert_eq!(planned.cable.run, vec![Point::new(57.5, 50.0), Point::new(142.5, 50.0)]);
        assert_eq!(planned.cable.cores[0].end_a, Some(CoreEnd::Pin { part: j1, pin: "1".into(), index: None }));
        assert_eq!(planned.cable.reference, "W1");
    }

    /// Pins may be given by name as well as number, and a side whose pin count differs from the core count is refused, naming the side and both counts.
    /// ピンは番号でも名前でも指定でき、ピンの数が芯の数と違う端は、その端と両方の数を示して断る。
    #[test]
    fn pins_by_name_and_a_count_mismatch() {
        let mut sheet = Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape);
        let block = SymbolInstance {
            id: uuid::Uuid::new_v4(),
            symbol_id: crate::block::BLOCK_SYMBOL_ID.into(),
            at: Point::new(50.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "CTRL1".into(),
            value: String::new(),
            attrs: [(
                crate::block::BLOCK_PINS_ATTR.to_string(),
                r#"{"right":[{"no":"1","name":"TX"},{"no":"2","name":"RX"},{"no":"3","name":"GND"}]}"#.to_string(),
            )]
            .into_iter()
            .collect(),
        };
        insert(&mut sheet, Entity::Symbol(block));
        insert(&mut sheet, Entity::Symbol(connector(3, "J2", 150.0, 50.0, true)));
        let defs = sheet_symbol_defs(&sheet);
        let ok = plan_cable_run(
            &sheet,
            &defs,
            &spec(serde_json::json!({
                "cores": ["TX", "RX", "GND"],
                "a": {"pins": ["CTRL1.TX", "CTRL1.RX", "CTRL1.GND"], "style": "multi"},
                "b": {"part": "J2", "style": "single"}
            })),
        )
        .unwrap();
        assert_eq!(ok.ends[0][0], "CTRL1.1");
        let err = plan_cable_run(
            &sheet,
            &defs,
            &spec(serde_json::json!({
                "cores": ["TX", "RX", "GND", "5V"],
                "a": {"pins": ["CTRL1.TX", "CTRL1.RX", "CTRL1.GND"], "style": "multi"},
                "b": {"part": "J2", "style": "single"}
            })),
        )
        .unwrap_err();
        assert!(err.contains("a 側のピンが 3 本で、芯は 4 本"), "{err}");
    }

    /// A single line landing on a through terminal block maps each core to one terminal number, using the left connection points unless "#2" picks the right ones.
    /// 貫通端子台に着く1本線は、芯を端子番号1つずつに対応させ、ふだんは左の接続点、「#2」を付ければ右の接続点に着く。
    #[test]
    fn a_single_line_on_a_through_terminal_block() {
        let mut sheet = Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape);
        insert(&mut sheet, Entity::Symbol(connector(3, "J1", 20.0, 100.0, false)));
        let mut tb = connector(3, "TB1", 100.0, 100.0, false);
        tb.symbol_id = "terminal_block_3p".into();
        let tb = insert(&mut sheet, Entity::Symbol(tb));
        let defs = sheet_symbol_defs(&sheet);
        let by_part = plan_cable_run(
            &sheet,
            &defs,
            &spec(serde_json::json!({
                "cores": ["1", "2", "3"],
                "a": {"part": "J1", "style": "single"},
                "b": {"part": "TB1", "style": "single"}
            })),
        )
        .unwrap();
        assert_eq!(by_part.cable.cores[2].end_b, Some(CoreEnd::Pin { part: tb, pin: "3".into(), index: None }));
        assert_eq!(by_part.cable.run.last().unwrap().x, 97.5);
        let right = plan_cable_run(
            &sheet,
            &defs,
            &spec(serde_json::json!({
                "cores": ["1", "2", "3"],
                "a": {"part": "J1", "style": "single"},
                "b": {"pins": ["TB1.1#2", "TB1.2#2", "TB1.3#2"], "style": "single"}
            })),
        )
        .unwrap();
        assert_eq!(right.cable.cores[0].end_b, Some(CoreEnd::Pin { part: tb, pin: "1".into(), index: Some(2) }));
        assert_eq!(right.cable.run.last().unwrap().x, 102.5);
        assert_eq!(right.ends[0][1], "TB1.1#2");
    }

    /// A breakout placed behind the pins, or whose wire ends would land on another wire, is refused with a hint to move breakout_at.
    /// ピンより後ろに置いた分岐点や、芯の配線の端が別の配線に乗る分岐点は、breakout_at をずらすよう伝えて断る。
    #[test]
    fn a_bad_breakout_is_refused() {
        let mut sheet = board_and_plug();
        let defs = sheet_symbol_defs(&sheet);
        let lte = |breakout: serde_json::Value| {
            spec(serde_json::json!({
                "cores": ["VBUS", "D-", "D+", "GND"],
                "a": {"part": "J45", "style": "multi", "breakout_at": breakout},
                "b": {"part": "USB1", "style": "single"}
            }))
        };
        let behind = plan_cable_run(&sheet, &defs, &lte(serde_json::json!({"x": 50.0, "y": 100.0}))).unwrap_err();
        assert!(behind.contains("breakout_at"), "{behind}");
        insert(&mut sheet, Entity::Wire(crate::cable::test_support::wire(&[(65.0, 92.5), (65.0, 80.0)])));
        let on_wire = plan_cable_run(&sheet, &defs, &lte(serde_json::Value::Null)).unwrap_err();
        assert!(on_wire.contains("breakout_at"), "{on_wire}");
    }
}
```

Add `pub mod cable_run;` to `src-tauri/crates/madake-core/src/lib.rs` after `pub mod cable;`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core -- cable_run::`
Expected: FAIL to compile (`cannot find function plan_cable_run`).

- [ ] **Step 3: Implement**

In `pin_wiring.rs`, change `fn pins_on_route(` to `pub(crate) fn pins_on_route(`.

In `cable.rs`, add after `cable_label`:

```rust
/// シート上の次のケーブル参照記号 (ケーブルとハーネスの W 番号の続き。作図領域の`nextCableReference`と同じ)。
pub fn next_cable_reference(sheet: &Sheet) -> String {
    let prefix = crate::harness::HARNESS_PREFIX;
    let max = sheet
        .entities
        .values()
        .filter_map(|e| match e {
            Entity::Cable(c) => Some(c.reference.as_str()),
            Entity::Harness(h) => Some(h.name.as_str()),
            _ => None,
        })
        .filter_map(|n| n.strip_prefix(prefix))
        .filter_map(|n| n.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    format!("{prefix}{}", max + 1)
}

/// 着いたピンの位置から、部品に着く端の棒の両端 (上→下、同じ高さなら左→右に並べた最初と最後)。
pub fn bar_ends(positions: &[Point]) -> Option<(Point, Point)> {
    let order = |a: &&Point, b: &&Point| {
        (a.y, a.x).partial_cmp(&(b.y, b.x)).unwrap_or(std::cmp::Ordering::Equal)
    };
    let first = positions.iter().min_by(order)?;
    let last = positions.iter().max_by(order)?;
    Some((*first, *last))
}
```

Write the implementation at the top of `cable_run.rs` (above the test module):

```rust
//! 1本線のケーブルをピン名で引くための計算 (MCPの connect_cable)。
//!
//! エージェントは芯の名前・両端のピン (または部品)・端ごとの描き方 (`multi` = 芯ごとの配線を
//! ピンから分岐点まで引く / `single` = 1本線をその部品に着ける) だけを渡す。ここが分岐点・芯ごとの
//! 配線・1本線の経路・芯の割当てを決め、ケーブルと分岐の配線を返す。芯はピン順に対応させる。
//! 編集そのものは呼び出し側が1回の編集 (`execute_batch`) で行う。

use serde::Deserialize;
use uuid::Uuid;

use crate::geometry::Point;
use crate::model::{Cable, CableCore, CableShield, CoreEnd, Entity, Sheet, Wire};
use crate::pin_wiring::{find_parts, pins_on_route, resolve_pin, route, ResolvedPin, ROUTE_GRID};
use crate::symbol::{PinDir, SymbolDef};

/// 自動で置く分岐点を、ピンの並びの先からどれだけ離すか (mm)。
pub const BREAKOUT_OFFSET_MM: f64 = 10.0;
/// 芯ごとの配線の分岐側の端を、分岐点からピン側へどれだけ戻すか (mm)。扇の線の長さになる。
pub const FAN_LENGTH_MM: f64 = 2.5;

/// 端の描き方。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SideStyle {
    /// 芯ごとの配線をピンから分岐点まで引き、そこから1本線にする。
    Multi,
    /// 1本線をそのまま部品に着ける (着いたピンを横切る棒で描く)。
    Single,
}

/// ケーブルの片側の指定。
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct CableSide {
    /// 芯が着くピンを芯の順に。"参照記号.ピン" (例 "J45.1"、同じ番号の2つ目は "TB1.1#2")。
    /// `part` を書いたときはピン番号か名前だけでもよい。
    #[serde(default)]
    pub pins: Option<Vec<String>>,
    /// 部品 (参照記号かentity id)。`pins` を省略するとこの部品のピンをピン順に使う。
    #[serde(default)]
    pub part: Option<String>,
    /// multi = 芯ごとの配線を引いて分岐 / single = 1本線をこの部品に着ける。
    pub style: SideStyle,
    /// 分岐点 (multi のとき、mm。2.5mmグリッドに丸める)。省略時はピンの並びの先 10mm。
    #[serde(default)]
    pub breakout_at: Option<Point>,
}

/// 芯1本の指定: 名前だけか、名前と線色。
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum CoreSpec {
    Name(String),
    Full {
        core: String,
        #[serde(default)]
        color: Option<String>,
    },
}

impl CoreSpec {
    fn name(&self) -> &str {
        match self {
            CoreSpec::Name(n) => n,
            CoreSpec::Full { core, .. } => core,
        }
    }

    fn color(&self) -> Option<&str> {
        match self {
            CoreSpec::Name(_) => None,
            CoreSpec::Full { color, .. } => color.as_deref(),
        }
    }
}

/// connect_cable の1本ぶんの指定。
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct CableRunSpec {
    /// 参照記号 (省略時はシートの次の W 番号)。
    #[serde(default)]
    pub reference: Option<String>,
    /// 型番 (部品表に載る)。
    #[serde(default)]
    pub part_no: Option<String>,
    /// 線径 sq (分岐の配線にも使う。省略時はケーブル未設定・配線0.3)。
    #[serde(default)]
    pub sq: Option<f64>,
    /// ケーブル全長 m (分岐の先も含む、買って切る長さ)。
    #[serde(default)]
    pub length_m: Option<f64>,
    /// シールド。
    #[serde(default)]
    pub shield: Option<CableShield>,
    /// 芯の線色の既定 (芯ごとの color が無いとき)。
    #[serde(default)]
    pub color: Option<String>,
    /// 芯 (順に、両端のピンへピン順に対応する)。
    pub cores: Vec<CoreSpec>,
    /// 区間の始点側。
    pub a: CableSide,
    /// 区間の終点側。
    pub b: CableSide,
    /// 1本線の経由点 (mm)。
    #[serde(default)]
    pub via: Option<Vec<Point>>,
}

/// 計画したケーブルと、その分岐の配線。
#[derive(Debug, Clone)]
pub struct PlannedCable {
    pub cable: Cable,
    /// 分岐の配線 (a 側の芯の順、続いて b 側)。
    pub wires: Vec<Wire>,
    /// 芯ごとの両端の表記 ("J45.1" / "TB1.1#2")。
    pub ends: Vec<[String; 2]>,
}

impl PlannedCable {
    /// 図面へ入れる順のエンティティ (配線が先、ケーブルが最後。ケーブルの検証は配線があることを見る)。
    pub fn entities(&self) -> Vec<Entity> {
        let mut out: Vec<Entity> = self.wires.iter().cloned().map(Entity::Wire).collect();
        out.push(Entity::Cable(self.cable.clone()));
        out
    }
}

/// 片側の計画結果。
struct SidePlan {
    /// 1本線の端の点。
    at: Point,
    /// 1本線がこの端から出ていく向き。
    dir: PinDir,
    ends: Vec<CoreEnd>,
    wires: Vec<Wire>,
    labels: Vec<String>,
}

/// 1本線のケーブルを計画する。シートは変えない。
pub fn plan_cable_run(sheet: &Sheet, defs: &[SymbolDef], spec: &CableRunSpec) -> Result<PlannedCable, String> {
    if spec.cores.is_empty() {
        return Err("cores に芯を1本以上書く".into());
    }
    let colors: Vec<String> = spec
        .cores
        .iter()
        .map(|c| c.color().or(spec.color.as_deref()).unwrap_or("").to_string())
        .collect();
    let a = plan_side(sheet, defs, spec, "a", &spec.a, &colors)?;
    let b = plan_side(sheet, defs, spec, "b", &spec.b, &colors)?;
    let run = route(a.at, a.dir, b.at, b.dir, spec.via.as_deref().unwrap_or_default());
    if run.len() < 2 {
        return Err("a と b の1本線の端が同じ点です。部品か breakout_at を離す".into());
    }
    let cores = spec
        .cores
        .iter()
        .enumerate()
        .map(|(i, c)| CableCore {
            core: c.name().to_string(),
            color: colors[i].clone(),
            wire: None,
            end_a: Some(a.ends[i].clone()),
            end_b: Some(b.ends[i].clone()),
        })
        .collect();
    let cable = Cable {
        id: Uuid::new_v4(),
        reference: spec
            .reference
            .clone()
            .unwrap_or_else(|| crate::cable::next_cable_reference(sheet)),
        part_no: spec.part_no.clone().unwrap_or_default(),
        core_count: spec.cores.len(),
        sq: spec.sq.unwrap_or(0.0),
        points: Vec::new(),
        cores,
        shield: spec.shield.clone(),
        length_m: spec.length_m,
        run,
    };
    let ends = (0..spec.cores.len()).map(|i| [a.labels[i].clone(), b.labels[i].clone()]).collect();
    let mut wires = a.wires;
    wires.extend(b.wires);
    Ok(PlannedCable { cable, wires, ends })
}

/// 向きの単位ベクトル。
fn unit(dir: PinDir) -> Point {
    match dir {
        PinDir::Right => Point::new(1.0, 0.0),
        PinDir::Left => Point::new(-1.0, 0.0),
        PinDir::Down => Point::new(0.0, 1.0),
        PinDir::Up => Point::new(0.0, -1.0),
    }
}

fn opposite(dir: PinDir) -> PinDir {
    match dir {
        PinDir::Right => PinDir::Left,
        PinDir::Left => PinDir::Right,
        PinDir::Down => PinDir::Up,
        PinDir::Up => PinDir::Down,
    }
}

/// "TB1.1#2" の 2。
fn occurrence(spec: &str) -> Option<u32> {
    spec.rsplit_once('#').and_then(|(_, n)| n.parse().ok()).filter(|n| *n >= 1)
}

/// 端の指定をピンの並び (解決結果, 何番目) にする。
fn side_pins(
    sheet: &Sheet,
    defs: &[SymbolDef],
    name: &str,
    side: &CableSide,
) -> Result<Vec<(ResolvedPin, Option<u32>)>, String> {
    match (&side.pins, &side.part) {
        (Some(list), part) => list
            .iter()
            .map(|spec| {
                let full = match part {
                    Some(p) if !spec.contains('.') => format!("{p}.{spec}"),
                    _ => spec.clone(),
                };
                let r = resolve_pin(sheet, defs, &full).map_err(|e| format!("{name} 側 {full}: {e}"))?;
                Ok((r, occurrence(&full)))
            })
            .collect(),
        (None, Some(part)) => {
            let parts = find_parts(sheet, defs, part);
            let [one] = parts.as_slice() else {
                return Err(format!(
                    "{name} 側の部品「{part}」が{}個あります。1つに決まる参照記号か entity id を書く",
                    parts.len()
                ));
            };
            // ピン順。同じ番号の接続点 (貫通端子の左右) は最初の1つだけ
            let mut seen: Vec<&str> = Vec::new();
            let mut out = Vec::new();
            for pin in &one.pins {
                if seen.contains(&pin.number.as_str()) {
                    continue;
                }
                seen.push(pin.number.as_str());
                out.push((
                    ResolvedPin { entity_id: one.entity_id, reference: one.reference.clone(), pin: pin.clone() },
                    None,
                ));
            }
            Ok(out)
        }
        (None, None) => Err(format!("{name} 側に pins か part を書く")),
    }
}

fn plan_side(
    sheet: &Sheet,
    defs: &[SymbolDef],
    spec: &CableRunSpec,
    name: &str,
    side: &CableSide,
    colors: &[String],
) -> Result<SidePlan, String> {
    let pins = side_pins(sheet, defs, name, side)?;
    if pins.len() != spec.cores.len() {
        return Err(format!(
            "{name} 側のピンが {} 本で、芯は {} 本です。ピンと芯は同じ数にする (芯はピン順に対応する)",
            pins.len(),
            spec.cores.len()
        ));
    }
    let labels: Vec<String> = pins
        .iter()
        .map(|(r, index)| {
            let owner = if r.reference.is_empty() { r.entity_id.to_string() } else { r.reference.clone() };
            match index {
                Some(n) => format!("{owner}.{}#{n}", r.pin.number),
                None => format!("{owner}.{}", r.pin.number),
            }
        })
        .collect();
    let dir = pins[0].0.pin.dir;
    match side.style {
        SideStyle::Single => {
            let part = pins[0].0.entity_id;
            if pins.iter().any(|(r, _)| r.entity_id != part) {
                return Err(format!(
                    "{name} 側は single なので、ピンは1つの部品のものだけにする (1本線はその部品に着く)"
                ));
            }
            let positions: Vec<Point> = pins.iter().map(|(r, _)| Point::new(r.pin.x, r.pin.y)).collect();
            let (lo, hi) = crate::cable::bar_ends(&positions).ok_or("ピンがありません")?;
            let at = Point::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
            let ends = pins
                .iter()
                .map(|(r, index)| CoreEnd::Pin { part, pin: r.pin.number.clone(), index: *index })
                .collect();
            Ok(SidePlan { at, dir, ends, wires: Vec::new(), labels })
        }
        SideStyle::Multi => {
            let d = unit(dir);
            let at = match side.breakout_at {
                Some(p) => p.snapped(ROUTE_GRID),
                None => auto_breakout(&pins, d),
            };
            let mut wires = Vec::new();
            let mut ends = Vec::new();
            let mut taken: Vec<Point> = Vec::new();
            for (i, (r, _)) in pins.iter().enumerate() {
                let core = spec.cores[i].name();
                let pin_at = Point::new(r.pin.x, r.pin.y);
                let ahead = (at.x - pin_at.x) * d.x + (at.y - pin_at.y) * d.y;
                if ahead < FAN_LENGTH_MM + ROUTE_GRID - 1e-6 {
                    return Err(format!(
                        "{name} 側の分岐点 ({:.1}, {:.1}) が {} の先 (向き {:?}) に {}mm 以上離れていません。breakout_at をピンの外側へずらす",
                        at.x, at.y, labels[i], dir, FAN_LENGTH_MM + ROUTE_GRID
                    ));
                }
                // 分岐側の端: 分岐点から FAN_LENGTH_MM だけピン側へ戻り、ピンと同じ並びの位置
                let end = if d.x != 0.0 {
                    Point::new(at.x - d.x * FAN_LENGTH_MM, pin_at.y)
                } else {
                    Point::new(pin_at.x, at.y - d.y * FAN_LENGTH_MM)
                };
                if taken.iter().any(|p| p.distance_to(&end) < 1e-6) {
                    return Err(format!(
                        "{name} 側の芯 {core} の分岐側の端が他の芯と重なります (ピンが一列に並んでいません)。ピンの並ぶ部品を選ぶか single にする"
                    ));
                }
                if crate::netlist::point_is_attached(sheet, defs, &end, None) {
                    return Err(format!(
                        "{name} 側の芯 {core} の分岐側の端 ({:.1}, {:.1}) に別の配線やピンがあります。breakout_at をずらす",
                        end.x, end.y
                    ));
                }
                let points = route(pin_at, r.pin.dir, end, opposite(r.pin.dir), &[]);
                let crossed = pins_on_route(sheet, defs, &points, &[r]);
                if !crossed.is_empty() {
                    return Err(format!(
                        "{name} 側の芯 {core} の配線が別のピンの上を通ります: {}。breakout_at をずらす",
                        crossed.join(", ")
                    ));
                }
                taken.push(end);
                let wire = Wire {
                    id: Uuid::new_v4(),
                    points,
                    color: if colors[i].is_empty() { "black".into() } else { colors[i].clone() },
                    sq: spec.sq.unwrap_or(0.3),
                    length_m: None,
                    length_source: Default::default(),
                    part_no: None,
                    net: None,
                };
                ends.push(CoreEnd::Wire { wire: wire.id });
                wires.push(wire);
            }
            Ok(SidePlan { at, dir, ends, wires, labels })
        }
    }
}

/// 自動の分岐点: ピンの並びのいちばん先 (向き d) から BREAKOUT_OFFSET_MM 先、並びの中央。2.5mmグリッドに丸める。
fn auto_breakout(pins: &[(ResolvedPin, Option<u32>)], d: Point) -> Point {
    let n = pins.len() as f64;
    let (cx, cy) = pins.iter().fold((0.0, 0.0), |(x, y), (r, _)| (x + r.pin.x / n, y + r.pin.y / n));
    let reach = pins
        .iter()
        .map(|(r, _)| r.pin.x * d.x + r.pin.y * d.y)
        .fold(f64::MIN, f64::max)
        + BREAKOUT_OFFSET_MM;
    let p = if d.x != 0.0 { Point::new(reach * d.x, cy) } else { Point::new(cx, reach * d.y) };
    p.snapped(ROUTE_GRID)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core`
Expected: PASS.

- [ ] **Step 5: Regenerate the spec docs and commit**

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout
python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check
git add src-tauri/crates/madake-core/src/{cable_run.rs,lib.rs,pin_wiring.rs,cable.rs} docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(core): ピン名と端ごとの描き方から1本線のケーブルを計画する plan_cable_run"
```

---

### Task 7: MCP tool `connect_cable`

**Files:**
- Modify: `src-tauri/crates/madake-mcp/src/lib.rs` (new `ConnectCableParams` after `PlaceMateParams`/`MateSpec` ~line 630; new tool after `place_mate` ~line 1442)
- Modify: `src-tauri/crates/madake-agent/src/knowledge.rs:60-61`
- Test: `src-tauri/crates/madake-mcp/tests/drawing_tools.rs`

**Interfaces:**
- Consumes: Task 6 `madake_core::cable_run::{plan_cable_run, CableRunSpec, CableSide, CoreSpec}`, `PlannedCable::{cable, wires, ends, entities}`; `madake_core::CoreEnd`, `madake_core::CableShield`.
- Produces: MCP tool `connect_cable` returning `{cable_id, reference, run, cores: [{core, a, b, wire_ids}], revision}` (single) or `{cables: [...], revision}` (list).

- [ ] **Step 1: Write the failing tests**

Append to `src-tauri/crates/madake-mcp/tests/drawing_tools.rs`:

```rust
/// connect_cable draws the LTE harness in one call: four board wires to a breakout, one line to the USB plug, cores mapped by pin order; the netlist has four nets, ERC finds no unconnected pin, and one undo removes it all.
/// connect_cable は LTE ハーネスを1回で描く: 基板側の配線4本と分岐、USBプラグまでの1本線、ピン順の芯の対応。ネットリストは4ネット、ERC は未接続ピンを出さず、undo 1回で全部消える。
#[tokio::test]
async fn connect_cable_draws_a_breakout_harness_in_one_edit() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    call(&bridge, "place_symbol", json!({"sheet_id": sheet, "symbol_id": "connector_4p", "x": 50.0, "y": 100.0, "reference": "J45"})).await;
    call(&bridge, "place_symbol", json!({"sheet_id": sheet, "symbol_id": "connector_4p", "x": 200.0, "y": 100.0, "rotation": 180, "reference": "USB1"})).await;
    let depth = doc.engine.lock().unwrap().undo_depth();
    let out = call(
        &bridge,
        "connect_cable",
        json!({"sheet_id": sheet, "reference": "W45",
               "cores": ["VBUS", "D-", "D+", "GND"],
               "a": {"part": "J45", "style": "multi"},
               "b": {"part": "USB1", "style": "single"}}),
    )
    .await;
    assert_eq!(out["reference"], "W45");
    let cores = out["cores"].as_array().expect("cores");
    assert_eq!(cores.len(), 4);
    assert_eq!(cores[1]["a"], "J45.2");
    assert_eq!(cores[1]["b"], "USB1.2");
    assert_eq!(cores[1]["wire_ids"].as_array().unwrap().len(), 1);
    assert_eq!(doc.engine.lock().unwrap().undo_depth(), depth + 1);
    let nets = call(&bridge, "get_netlist", json!({"sheet_id": sheet})).await;
    assert_eq!(nets.as_array().unwrap().len(), 4, "{nets}");
    let diags = call(&bridge, "run_verification", json!({"sheet_id": sheet})).await;
    assert_eq!(unconnected_pins(&diags), 0);
    doc.undo().expect("元に戻せる");
    assert_eq!(doc.engine.lock().unwrap().project().sheets[0].entities.len(), 2);
}

/// If any cable in the list is wrong, connect_cable names its index and draws none of them; a correct list is drawn in one edit.
/// リストのケーブルのどれか1本でも誤っていれば、connect_cable はその番号を知らせて1本も描かない。正しいリストは1回の編集で描く。
#[tokio::test]
async fn connect_cable_list_is_all_or_nothing() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    call(&bridge, "place_symbol", json!({"sheet_id": sheet, "symbol_id": "connector_3p", "x": 50.0, "y": 50.0, "reference": "J1"})).await;
    call(&bridge, "place_symbol", json!({"sheet_id": sheet, "symbol_id": "connector_3p", "x": 150.0, "y": 50.0, "rotation": 180, "reference": "J2"})).await;
    call(&bridge, "place_symbol", json!({"sheet_id": sheet, "symbol_id": "connector_4p", "x": 50.0, "y": 120.0, "reference": "J45"})).await;
    let count = || doc.engine.lock().unwrap().project().sheets[0].entities.len();
    let before = count();
    let aux = json!({"cores": ["L", "R", "GND"], "a": {"part": "J1", "style": "single"}, "b": {"part": "J2", "style": "single"}});
    let err = call_err(
        &bridge,
        "connect_cable",
        json!({"sheet_id": sheet, "cables": [
            aux.clone(),
            {"cores": ["VBUS", "D-"], "a": {"part": "J45", "style": "multi"}, "b": {"part": "J2", "style": "single"}}
        ]}),
    )
    .await;
    assert!(err.contains("cables[1]") && err.contains("a 側のピンが 4 本で、芯は 2 本"), "{err}");
    assert_eq!(count(), before);
    let out = call(&bridge, "connect_cable", json!({"sheet_id": sheet, "cables": [aux]})).await;
    assert_eq!(out["cables"].as_array().unwrap().len(), 1);
    assert_eq!(out["cables"][0]["reference"], "W1");
    assert_eq!(count(), before + 1);
}

/// connect_cable's description tells the agent that cores follow pin order and to use it instead of a fake breakout block.
/// connect_cable の説明は、芯がピン順に対応することと、偽の分岐ブロックの代わりに使うことをエージェントに伝える。
#[test]
fn connect_cable_is_described_for_the_agent() {
    let tools = tool_descriptions();
    let (_, d) = tools.iter().find(|(n, _)| n == "connect_cable").expect("connect_cable");
    assert!(d.contains("ピン順") && d.contains("分岐"), "{d}");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-mcp --test drawing_tools connect_cable`
Expected: FAIL (`connect_cable が失敗した` — unknown tool; and `expect("connect_cable")` panics).

- [ ] **Step 3: Implement**

In `madake-mcp/src/lib.rs`, add after the `MateSpec` struct:

```rust
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ConnectCableParams {
    /// 配線先シートID。省略時は先頭シート。
    pub sheet_id: Option<Uuid>,
    /// 参照記号 (省略時はシートの次のW番号)。1本だけ引くときに使う (複数はcables)。
    pub reference: Option<String>,
    /// 型番 (部品表に載る)。
    pub part_no: Option<String>,
    /// 線径 sq。
    pub sq: Option<f64>,
    /// ケーブル全長 m (分岐の先も含む)。
    pub length_m: Option<f64>,
    /// シールド。
    pub shield: Option<madake_core::CableShield>,
    /// 芯の線色の既定。
    pub color: Option<String>,
    /// 芯 (名前の並び、または {core, color})。両端のピンへピン順に対応する。
    pub cores: Option<Vec<madake_core::cable_run::CoreSpec>>,
    /// 区間の始点側 (pins か part と style)。
    pub a: Option<madake_core::cable_run::CableSide>,
    /// 区間の終点側。
    pub b: Option<madake_core::cable_run::CableSide>,
    /// 1本線の経由点 (mm)。
    pub via: Option<Vec<Point>>,
    /// 複数のケーブルを1回で引く (上の欄の代わり)。全部引けるときだけ引き、undo一発で全部消える。
    pub cables: Option<Vec<madake_core::cable_run::CableRunSpec>>,
}
```

Add after the `place_mate` tool function:

```rust
    #[tool(
        description = "多芯ケーブルを、片側(または両側)を1本線で描いて1回で引く(例: USBプラグ側は1本線、基板側は芯ごとの配線4本)。cores=芯の名前の並び(または{core,color})。a/b=端ごとに pins(\"J45.1\"の並び。partを書けばピン番号か名前だけでもよい。同じ番号の2つ目は\"TB1.1#2\")か part(参照記号かentity id。pins省略でその部品のピンを順に使う)と style: multi=ピンから分岐点まで芯ごとに配線を引く(breakout_atで分岐点、省略でピンの先10mm) / single=1本線をその部品に着ける。**芯はピン順に対応する**(芯の数=ピンの数)。ネットリスト・電線リスト・ERCは芯ごとに端から端まで繋がり、1本線は位置では何にも繋がらない。偽の「分岐」ブロックや、1本の配線を複数のピンに触れさせる描き方(全ピンが短絡する)の代わりに使う。reference/part_no/sq/length_m(ケーブル全長)/shield/color/via(1本線の経由点)は任意。**複数のケーブルはcablesに並べて1回で引く**(全部引けるときだけ引き、undo一発で全部消える。誤りがあれば何番目かを知らせて1本も引かない)。ケーブルid・参照記号・1本線の点列runと、芯ごとの両端(a/b)と分岐の配線id(wire_ids)を返す"
    )]
    fn connect_cable(&self, Parameters(p): Parameters<ConnectCableParams>) -> Result<String, ErrorData> {
        use madake_core::cable_run::{plan_cable_run, CableRunSpec};
        let sheet_id = self.resolve_sheet(p.sheet_id)?;
        let single = p.cables.is_none();
        let specs = match (p.cables, p.cores, p.a, p.b) {
            (Some(list), None, None, None) if !list.is_empty() => list,
            (None, Some(cores), Some(a), Some(b)) => vec![CableRunSpec {
                reference: p.reference,
                part_no: p.part_no,
                sq: p.sq,
                length_m: p.length_m,
                shield: p.shield,
                color: p.color,
                cores,
                a,
                b,
                via: p.via,
            }],
            _ => {
                return Err(ErrorData::invalid_params(
                    "cores と a と b (1本) か cables (複数) のどちらか一方を指定する",
                    None,
                ))
            }
        };
        // 計画から追加までを同じロックの中で行う (間で部品が動くと端がピンから外れる)
        let (planned, patch) = {
            let mut engine = self.doc.engine.lock().unwrap();
            let sheet = engine
                .project()
                .sheet(sheet_id)
                .ok_or_else(|| ErrorData::invalid_params("sheet not found", None))?;
            let defs = sheet_symbol_defs(sheet);
            // 前のケーブルで引いた配線・付けた参照記号を次の計画が見るよう、写しのシートへ順に足す
            let mut working = sheet.clone();
            let mut planned = Vec::with_capacity(specs.len());
            for (i, spec) in specs.iter().enumerate() {
                let plan = plan_cable_run(&working, &defs, spec).map_err(|e| {
                    let e = if single { e } else { format!("cables[{i}]: {e}。1本も引いていません") };
                    ErrorData::invalid_params(e, None)
                })?;
                for e in plan.entities() {
                    working.entities.insert(e.id(), e);
                }
                planned.push(plan);
            }
            let commands = planned
                .iter()
                .flat_map(|plan| plan.entities())
                .map(|entity| Command::AddEntity { sheet_id, entity })
                .collect();
            let patch = engine
                .execute_batch(commands, self.doc.mcp_origin())
                .map_err(|e| ErrorData::invalid_params(e.to_string(), None))?;
            (planned, patch)
        };
        let _ = self.doc.patches.send(patch.clone());
        let out: Vec<serde_json::Value> = planned
            .iter()
            .map(|plan| {
                let cores: Vec<serde_json::Value> = plan
                    .cable
                    .cores
                    .iter()
                    .zip(&plan.ends)
                    .map(|(core, ends)| {
                        let wire_ids: Vec<Uuid> = [&core.end_a, &core.end_b]
                            .into_iter()
                            .flatten()
                            .filter_map(|e| match e {
                                madake_core::CoreEnd::Wire { wire } => Some(*wire),
                                madake_core::CoreEnd::Pin { .. } => None,
                            })
                            .collect();
                        serde_json::json!({ "core": core.core, "a": ends[0], "b": ends[1], "wire_ids": wire_ids })
                    })
                    .collect();
                serde_json::json!({
                    "cable_id": plan.cable.id,
                    "reference": plan.cable.reference,
                    "run": plan.cable.run,
                    "cores": cores,
                })
            })
            .collect();
        if single {
            let mut one = out.into_iter().next().unwrap_or_default();
            one["revision"] = serde_json::json!(patch.revision);
            return json_ok(&one);
        }
        json_ok(&serde_json::json!({ "cables": out, "revision": patch.revision }))
    }
```

In `madake-agent/src/knowledge.rs`, replace the line

```
(1本ずつ呼ばない。`execute_commands`で座標を書いた配線を送らない)
```

with

```
(1本ずつ呼ばない。`execute_commands`で座標を書いた配線を送らない)
- **片側を1本線・もう片側を芯ごとに描くケーブル(USBプラグ側を1本線、基板側を4本など)は`connect_cable`で引く**\
(cores=芯の名前、a/b=pinsかpartとstyle=multi|single。芯はピン順に対応する)。\
偽の「分岐」ブロックや、1本の配線を複数のピンに触れさせる描き方はしない(全ピンが短絡する)
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-mcp && cargo test -p madake-agent`
Expected: PASS.

- [ ] **Step 5: Regenerate the spec docs and commit**

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout
python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check
git add src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-mcp/tests/drawing_tools.rs src-tauri/crates/madake-agent/src/knowledge.rs docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(mcp): 1本線のケーブルを1回で引く connect_cable"
```

---

### Task 8: SVG drawing of run cables

**Files:**
- Modify: `src-tauri/crates/madake-core/src/cable.rs` (`shield_ellipse` at ~96-103; new drawing geometry)
- Modify: `src-tauri/crates/madake-core/src/svg.rs:1416-1429` (Cable arm) and a new `render_run_cable` fn
- Test: `src-tauri/crates/madake-core/src/cable.rs`

**Interfaces:**
- Consumes: Task 2 `pin_end_position`, Task 4 `breakout_point`, Task 6 `bar_ends`.
- Produces (mirrored exactly in TS in Task 10):
  - `pub const RUN_STROKE_MM: f64 = 0.7`, `pub const RUN_SLASH_HALF_MM: f64 = 1.5`, `pub const CORE_LABEL_MM: f64 = 1.5`
  - `pub fn run_mid(c: &Cable) -> Option<Point>` (midpoint of the longest run segment; on ties the later one)
  - `pub fn run_slash(c) -> Option<(Point, Point)>` = `(m + (-1.5, +1.5), m + (+1.5, -1.5))`
  - `pub fn run_count_at(c) -> Option<Point>` = `m + (2.0, -2.0)`, `pub fn run_label_at(c) -> Option<Point>` = `m + (3.5, 4.0)`
  - `pub fn run_bars(sheet, defs, c) -> Vec<(Point, Point)>`, `pub fn run_fans(sheet, c) -> Vec<(Point, Point, String)>` (run end → breakout point, core name)
  - `shield_ellipse(c)` for a run cable = `(m, 3.0, 2.0, -45.0)`

- [ ] **Step 1: Write the failing test**

Append to `mod tests` in `cable.rs`:

```rust
    /// The printed sheet draws a single-line cable as a thick line with a slash, a bar across the plug pins it lands on, fan lines to the breakout wires with the core names, and the cable label.
    /// 印刷される図面は、1本線のケーブルを太い線と斜線、着いたプラグのピンを横切る棒、分岐の配線への扇と芯の呼び名、注記で描く。
    #[test]
    fn the_printed_sheet_draws_a_run_cable() {
        let lte = test_support::lte_sheet();
        let project = test_support::project_with(lte.sheet);
        let sid = project.sheets[0].id;
        let defs = sheet_symbol_defs(&project.sheets[0]);
        let svg = crate::svg::project_sheet_to_svg(&project, sid, &defs).unwrap();
        assert!(
            svg.contains("<polyline points=\"80,100 192.500,100\" fill=\"none\" stroke=\"#000\" stroke-width=\"0.700\"/>"),
            "区間"
        );
        assert!(svg.contains("x1=\"134.750\" y1=\"101.500\" x2=\"137.750\" y2=\"98.500\""), "斜線");
        assert!(svg.contains("x1=\"192.500\" y1=\"92.500\" x2=\"192.500\" y2=\"107.500\""), "棒");
        assert!(svg.contains("x1=\"80\" y1=\"100\" x2=\"77.500\" y2=\"97.500\""), "扇");
        assert!(svg.contains(">D-</text>"), "芯の呼び名");
        assert!(svg.contains("W45  4C×0.2sq  0.3m"), "注記");
        assert!(!svg.contains("<ellipse"), "シールドが無ければ楕円は描かない");
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core -- cable::tests::the_printed_sheet_draws_a_run_cable`
Expected: FAIL at "区間" (today the SVG arm only draws `points`, which is empty for a run cable).

- [ ] **Step 3: Implement the geometry (`cable.rs`)**

Replace `shield_ellipse` with:

```rust
/// シールドの楕円 (中心, 長半径, 短半径, 向きの角度 [度])。定義線ケーブルは定義線を少し長めに囲み、
/// 区間ケーブルは区間の斜線の印を囲む。
pub fn shield_ellipse(c: &Cable) -> Option<(Point, f64, f64, f64)> {
    if c.is_run() {
        return run_mid(c).map(|m| (m, 3.0, 2.0, -45.0));
    }
    let (a, b) = (c.points.first()?, c.points.get(1)?);
    let center = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
    let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
    let angle = (b.y - a.y).atan2(b.x - a.x).to_degrees();
    Some((center, len / 2.0 + 2.5, 3.0, angle))
}
```

Add after `bar_ends`:

```rust
/// 区間の線の太さ (mm)。配線 (0.35mm) より太く描く。
pub const RUN_STROKE_MM: f64 = 0.7;
/// 区間の斜線の印の半分の長さ (x・y方向それぞれ、mm)。
pub const RUN_SLASH_HALF_MM: f64 = 1.5;
/// 分岐の配線の端に添える芯の呼び名の文字高 (mm)。
pub const CORE_LABEL_MM: f64 = 1.5;

/// 区間のいちばん長い線分の中点 (同じ長さなら後ろの線分)。斜線の印・芯数・注記の基準。
pub fn run_mid(c: &Cable) -> Option<Point> {
    c.run
        .windows(2)
        .max_by(|a, b| {
            a[0].distance_to(&a[1])
                .partial_cmp(&b[0].distance_to(&b[1]))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|s| Point::new((s[0].x + s[1].x) / 2.0, (s[0].y + s[1].y) / 2.0))
}

/// 単線図の印: 区間の中点を通る45度の短い斜線。
pub fn run_slash(c: &Cable) -> Option<(Point, Point)> {
    let m = run_mid(c)?;
    let h = RUN_SLASH_HALF_MM;
    Some((Point::new(m.x - h, m.y + h), Point::new(m.x + h, m.y - h)))
}

/// 芯数の文字の位置 (斜線の右上)。
pub fn run_count_at(c: &Cable) -> Option<Point> {
    run_mid(c).map(|m| Point::new(m.x + 2.0, m.y - 2.0))
}

/// ケーブル注記の位置 (斜線の右下)。
pub fn run_label_at(c: &Cable) -> Option<Point> {
    run_mid(c).map(|m| Point::new(m.x + 3.5, m.y + 4.0))
}

/// 部品に着く端の棒: 端ごとに、着いたピンの先を結ぶ線分 (ピンが1本だけなら描かない)。
pub fn run_bars(sheet: &Sheet, defs: &[SymbolDef], c: &Cable) -> Vec<(Point, Point)> {
    RunEnd::BOTH
        .iter()
        .filter_map(|&end| {
            let positions: Vec<Point> = c
                .cores
                .iter()
                .filter_map(|k| k.end(end))
                .filter_map(|e| pin_end_position(sheet, defs, e))
                .collect();
            let (a, b) = bar_ends(&positions)?;
            (a.distance_to(&b) > 1e-6).then_some((a, b))
        })
        .collect()
}

/// 分岐の扇: 区間の端から各芯の配線の分岐側の端への線と、その芯の呼び名。
pub fn run_fans(sheet: &Sheet, c: &Cable) -> Vec<(Point, Point, String)> {
    let mut out = Vec::new();
    for end in RunEnd::BOTH {
        let Some(at) = c.run_end(end) else { continue };
        for core in &c.cores {
            if let Some(CoreEnd::Wire { wire }) = core.end(end) {
                if let Some(p) = breakout_point(sheet, c, end, *wire) {
                    out.push((at, p, core.core.clone()));
                }
            }
        }
    }
    out
}
```

- [ ] **Step 4: Implement the SVG**

In `svg.rs`, replace

```rust
            // 多芯ケーブル: 定義線 (配線を斜めに横切る線)・シールドの楕円・注記
            Entity::Cable(c) => {
```

with

```rust
            // 1本線の区間を持つケーブル: 太い線・芯数の斜線・部品側の棒・分岐の扇と芯の呼び名・注記
            Entity::Cable(c) if c.is_run() => render_run_cable(&mut out, sheet, symbols, c),
            // 多芯ケーブル: 定義線 (配線を斜めに横切る線)・シールドの楕円・注記
            Entity::Cable(c) => {
```

and add this function directly above `pub fn sheet_to_svg(`:

```rust
/// 区間ケーブルを描く (形は [`crate::cable`] の `run_*` が決め、作図領域 `src/canvas/cable.ts` と同じ)。
fn render_run_cable(out: &mut String, sheet: &Sheet, symbols: &[SymbolDef], c: &crate::model::Cable) {
    let _ = write!(
        out,
        "<polyline points=\"{}\" fill=\"none\" stroke=\"#000\" stroke-width=\"{}\"/>\n",
        polyline_points(&c.run),
        n(crate::cable::RUN_STROKE_MM)
    );
    if let Some((a, b)) = crate::cable::run_slash(c) {
        line_el(out, a.x, a.y, b.x, b.y, CABLE_STROKE, "#000");
    }
    if let Some(at) = crate::cable::run_count_at(c) {
        text_el(out, at.x, at.y, 2.0, "#000", "start", &c.core_count.to_string());
    }
    for (a, b) in crate::cable::run_bars(sheet, symbols, c) {
        line_el(out, a.x, a.y, b.x, b.y, crate::cable::RUN_STROKE_MM, "#000");
    }
    for (a, b, core) in crate::cable::run_fans(sheet, c) {
        line_el(out, a.x, a.y, b.x, b.y, WIRE_STROKE, "#000");
        text_el(out, b.x, b.y - 0.8, crate::cable::CORE_LABEL_MM, "#000", "middle", &core);
    }
    if c.shield.is_some() {
        if let Some((center, rx, ry, angle)) = crate::cable::shield_ellipse(c) {
            let _ = write!(
                out,
                "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" transform=\"rotate({} {} {})\" fill=\"none\" stroke=\"#000\" stroke-width=\"{}\"/>\n",
                n(center.x), n(center.y), n(rx), n(ry), n(angle), n(center.x), n(center.y), n(CABLE_STROKE)
            );
        }
    }
    if let Some(at) = crate::cable::run_label_at(c) {
        text_el(out, at.x, at.y, 2.5, "#000", "start", &crate::cable::cable_label(c));
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout && touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core`
Expected: PASS.

- [ ] **Step 6: Regenerate the spec docs and commit**

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout
python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check
git add src-tauri/crates/madake-core/src/{cable.rs,svg.rs} docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(core): 印刷図面に1本線のケーブル (斜線・棒・分岐の扇) を描く"
```

---

### Task 9: Frontend types, the `run_cable` kind and the Properties columns

**Files:**
- Modify: `src/ipc.ts:157-189` (`Cable`, `CableCore`), `ipc.ts:313-329` (`Entity` union)
- Modify: `src/canvas/cable.ts` (imports, new helpers, `nextCableReference`)
- Modify: `src/tools/controller.ts:4,448,667-673,759-768,1101-1103`, `src/canvas/leader.ts:82-84`, `src/canvas/macroPreview.ts:99-103`, `src/canvas/viewClasses.ts:61-63`, `src/canvas/templatePreview.ts:53-54`, `src/canvas/renderer.ts:1319,1341`
- Modify: `src/components/entityKindLabels.ts`, `src/components/propertyCommands.ts:181`, `src/components/PropertiesPanel.vue:208,385,695-736`
- Modify: `src/locales/{en,ja,zh,es,fr,de}.json` (`cable` section)
- Test: `src/canvas/cable.test.ts`, `src/components/propertyCommands.test.ts`

**Interfaces:**
- Consumes: Task 1 JSON shapes (`kind: "run_cable"`, `run`, `end_a`/`end_b` with `kind: "pin"|"wire"`).
- Produces (used by Task 10):
  - `ipc.ts`: `Cable.run?: Point[]`, `CableCore.end_a?: CoreEnd | null`, `CableCore.end_b?: CoreEnd | null`, `export type CoreEnd = { kind: "pin"; part: string; pin: string; index?: number | null } | { kind: "wire"; wire: string }`, Entity member `({ kind: "run_cable" } & Cable)`
  - `cable.ts`: `export type CableEntity`, `export function isCable(e: Entity | null | undefined): e is CableEntity`, `export function isRunCable(c: Cable): boolean`, `export function cablePoints(c: Cable): Point[]`, `export function coreEndPin(sheet: Sheet, end: CoreEnd): string | null`

- [ ] **Step 1: Make sure `node_modules` exists**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout && [ -e node_modules ] || ln -s /Users/ktakeuchi/Documents/MadakeCAD/node_modules node_modules; ls node_modules/.bin/vitest`
Expected: prints the vitest path.

- [ ] **Step 2: Write the failing tests**

In `src/canvas/cable.test.ts`, change the import line to

```ts
import type { Cable, Entity, Sheet } from "../ipc";
import { cableAddCommand, cableHit, cableLabel, cablePoints, coreEndPin, isCable, nextCableReference, shieldEllipse } from "./cable";
```

and append inside `describe("多芯ケーブル", …)`:

```ts
  // ja: isCable は区間の有無にかかわらずケーブル (cable と run_cable) を見分け、cablePoints は定義線と区間の点を両方返す
  it("recognises both cable kinds and lists the points of the run", () => {
    const run: Entity = { kind: "run_cable", ...cable({ points: [], run: [{ x: 1, y: 2 }, { x: 3, y: 2 }] }) };
    const plain: Entity = { kind: "cable", ...cable() };
    const wire = { kind: "wire", id: "w", points: [], color: "", sq: 0, length_m: null, part_no: null, net: null } as Entity;
    expect([isCable(run), isCable(plain), isCable(wire), isCable(null)]).toEqual([true, true, false, false]);
    expect(cablePoints(run as Cable)).toEqual([{ x: 1, y: 2 }, { x: 3, y: 2 }]);
    expect(cablePoints(plain as Cable)).toEqual([{ x: 0, y: 0 }, { x: 0, y: 30 }]);
  });

  // ja: 芯の行き先がピンなら「参照記号.ピン」(部品が消えていれば「?.ピン」)、分岐の配線ならnull
  it("names a pin end by designator and pin, and a breakout wire end as null", () => {
    const s = sheet({
      usb: { kind: "symbol", id: "usb", symbol_id: "connector_4p", at: { x: 0, y: 0 }, rotation: 0, mirror: false, reference: "USB1", value: "", attrs: {} },
    } as unknown as Sheet["entities"]);
    expect(coreEndPin(s, { kind: "pin", part: "usb", pin: "2" })).toBe("USB1.2");
    expect(coreEndPin(s, { kind: "pin", part: "gone", pin: "1" })).toBe("?.1");
    expect(coreEndPin(s, { kind: "wire", wire: "w1" })).toBeNull();
  });
```

In `src/components/propertyCommands.test.ts`, append inside `describe("cableUpdateCommand", …)`:

```ts
  // ja: 1本線のケーブル (run_cable) のプロパティを書き換えても、種類・区間・芯の行き先はそのまま残る
  it("keeps the kind, run and core ends of a single-line cable", () => {
    const run: Entity = {
      ...(cable() as Extract<Entity, { kind: "cable" }>),
      kind: "run_cable",
      points: [],
      run: [{ x: 0, y: 0 }, { x: 20, y: 0 }],
      cores: [{ core: "VBUS", color: "red", wire: null, end_a: { kind: "wire", wire: "w1" }, end_b: { kind: "pin", part: "p1", pin: "1" } }],
    };
    const command = cableUpdateCommand("s1", run, patch);
    const e = (command as { entity: Entity }).entity;
    expect(e.kind).toBe("run_cable");
    expect(e).toMatchObject({ run: [{ x: 0, y: 0 }, { x: 20, y: 0 }], part_no: "SAMPLE-CBL" });
    expect(e.kind === "run_cable" && e.cores[0].end_b).toEqual({ kind: "pin", part: "p1", pin: "1" });
  });
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout && npx vitest run src/canvas/cable.test.ts src/components/propertyCommands.test.ts`
Expected: FAIL (`isCable`/`cablePoints`/`coreEndPin` are not exported; `cableUpdateCommand` returns null for `run_cable`).

- [ ] **Step 4: Implement the types and helpers**

`src/ipc.ts` — in `interface Cable` add after `length_m: number | null;`:

```ts
  /** 1本線の区間 (format v17)。無ければ定義線ケーブル。区間のあるケーブルは種類 "run_cable" で届く。 */
  run?: Point[];
```

in `interface CableCore` add after `wire: string | null;`:

```ts
  /** 区間の始点 (run[0]) 側の行き先 (v17)。 */
  end_a?: CoreEnd | null;
  /** 区間の終点側の行き先 (v17)。 */
  end_b?: CoreEnd | null;
```

after `interface CableCore` add:

```ts
/** 区間ケーブルの芯の行き先: 1本線が着く部品のピンか、分岐して続く配線 (madake-coreの`CoreEnd`)。 */
export type CoreEnd =
  | { kind: "pin"; part: string; pin: string; index?: number | null }
  | { kind: "wire"; wire: string };
```

and in `export type Entity =` add after `| ({ kind: "cable" } & Cable)`:

```ts
  | ({ kind: "run_cable" } & Cable)
```

`src/canvas/cable.ts` — replace the import `import type { Cable, Command, Point, Sheet } from "../ipc";` with:

```ts
import type { Cable, Command, CoreEnd, Entity, Point, Sheet } from "../ipc";
```

add after the `CABLE_FONT` constant:

```ts
/** 多芯ケーブルのエンティティ (区間の無い "cable" と、区間のある "run_cable")。 */
export type CableEntity = Extract<Entity, { kind: "cable" | "run_cable" }>;

/** ケーブルか (区間の有無を問わない)。 */
export function isCable(e: Entity | null | undefined): e is CableEntity {
  return e?.kind === "cable" || e?.kind === "run_cable";
}

/** 1本線の区間を持つか。 */
export function isRunCable(c: Cable): boolean {
  return (c.run?.length ?? 0) > 0;
}

/** ケーブルの全頂点 (定義線と区間)。範囲選択・外接矩形・移動のプレビューに使う。 */
export function cablePoints(c: Cable): Point[] {
  return [...c.points, ...(c.run ?? [])];
}

/** 芯の行き先がピンなら "USB1.2" (部品が消えていれば "?.2")、分岐の配線ならnull。 */
export function coreEndPin(sheet: Sheet, end: CoreEnd): string | null {
  if (end.kind !== "pin") return null;
  const part = sheet.entities[end.part];
  const ref = part && part.kind === "symbol" && part.reference ? part.reference : "?";
  return `${ref}.${end.pin}`;
}
```

and in `nextCableReference` replace `const name = e.kind === "cable" ? e.reference : e.kind === "harness" ? e.name : "";` with `const name = isCable(e) ? e.reference : e.kind === "harness" ? e.name : "";`.

- [ ] **Step 5: Update every place that checks `kind === "cable"`**

- `src/tools/controller.ts:4`: `import { cableAddCommand, cableHit, cablePoints, isCable, isRunCable } from "../canvas/cable";`
- `controller.ts:448`: `if (isCable(e) && cableHit(e, world, HIT_TOLERANCE)) return e.id;`
- `controller.ts:667-673`, replace the `const pts: Point[] = …;` expression with:

```ts
              const pts: Point[] =
                e.kind === "wire" || e.kind === "harness" || e.kind === "shape"
                  ? e.points
                  : isCable(e)
                    ? cablePoints(e)
                    : "at" in e && e.kind !== "dimension"
                      ? [e.at]
                      : [];
```

- `controller.ts:759-768`, replace the whole `const target: Pt | null = …;` statement with:

```ts
    const firstPoints =
      first.kind === "wire" || first.kind === "harness" || first.kind === "shape"
        ? first.points
        : isCable(first)
          ? cablePoints(first)
          : null;
    const target: Pt | null = firstPoints
      ? firstPoints.length
        ? {
            x: (firstPoints[0].x + firstPoints[firstPoints.length - 1].x) / 2,
            y: (firstPoints[0].y + firstPoints[firstPoints.length - 1].y) / 2,
          }
        : null
      : "at" in first && first.kind !== "dimension"
        ? first.at
        : null;
```

- `controller.ts:1101-1103`, replace

```ts
          if (e.kind === "wire" || e.kind === "harness" || e.kind === "shape" || e.kind === "cable") {
            ctx.beginPath();
            e.points.forEach((p, i) => {
```

with

```ts
          if (e.kind === "wire" || e.kind === "harness" || e.kind === "shape" || isCable(e)) {
            ctx.beginPath();
            (isCable(e) && isRunCable(e) ? (e.run ?? []) : e.points).forEach((p, i) => {
```

- `src/canvas/leader.ts`: add `import { cablePoints } from "./cable";` and replace

```ts
    case "cable":
      // Rust側 leader.rs と同じ: 定義線の2点の外接矩形
      return boundsOfPoints(entity.points);
```

with

```ts
    case "cable":
    case "run_cable":
      // Rust側 leader.rs と同じ: 定義線と区間の全頂点の外接矩形
      return boundsOfPoints(cablePoints(entity));
```

- `src/canvas/macroPreview.ts:99-103`, replace

```ts
      case "wire":
      case "harness":
      case "shape":
      case "cable":
        return { ...e, points: e.points.map(map) };
```

with

```ts
      case "wire":
      case "harness":
      case "shape":
        return { ...e, points: e.points.map(map) };
      case "cable":
      case "run_cable":
        return { ...e, points: e.points.map(map), ...(e.run ? { run: e.run.map(map) } : {}) };
```

- `src/canvas/viewClasses.ts:61-63`: add `case "run_cable":` directly below `case "cable":`.
- `src/canvas/templatePreview.ts`: add `import { cablePoints, isCable } from "./cable";` and replace

```ts
    if (e.kind === "wire" || e.kind === "harness" || e.kind === "shape" || e.kind === "cable")
      e.points.forEach((p) => consider(p.x, p.y));
```

with

```ts
    if (e.kind === "wire" || e.kind === "harness" || e.kind === "shape")
      e.points.forEach((p) => consider(p.x, p.y));
    else if (isCable(e)) cablePoints(e).forEach((p) => consider(p.x, p.y));
```

- `src/canvas/renderer.ts:31`: `import { drawCable, isCable } from "./cable";`; line 1319: `const cables = entities.filter(isCable);`; line 1341: `drawCable(ctx, vp, e, opts.selection.has(e.id));` (drop the `if (e.kind === "cable")` guard; the loop body is just this call).
- `src/components/entityKindLabels.ts`: add `run_cable: t("cable.kindLabel"),` after `cable: t("cable.kindLabel"),` and `"run_cable",` after `"cable",` in `ENTITY_KINDS`.
- `src/components/propertyCommands.ts`: add `import { isCable } from "../canvas/cable";` and change `if (entity.kind !== "cable") return null;` to `if (!isCable(entity)) return null;`.
- `src/components/PropertiesPanel.vue`:
  - imports: add `import { coreEndPin, isCable, isRunCable } from "../canvas/cable";` and change `import type { Entity, TextAnchor, TextColorToken } from "../ipc";` to `import type { CoreEnd, Entity, TextAnchor, TextColorToken } from "../ipc";`
  - after `const selected = computed<Entity | null>(…);` block add:

```ts
/** 選んでいるのが1本線の区間を持つケーブルか (芯の表に端A・端Bの列を出す)。 */
const selectedIsRun = computed(() => isCable(selected.value) && isRunCable(selected.value));

/** 芯の行き先の表記 (ピンは "USB1.2"、分岐の配線は「分岐の配線」、無ければ「—」)。 */
function coreEndText(end: CoreEnd | null | undefined): string {
  if (!end) return "—";
  const sheet = store.activeSheet;
  return (sheet ? coreEndPin(sheet, end) : null) ?? t("cable.endWire");
}
```

  - line 208: `} else if (e.kind === "cable") {` → `} else if (isCable(e)) {`
  - line 385: `} else if (e.kind === "cable") {` → `} else if (isCable(e)) {`
  - line 695: `<template v-else-if="selected?.kind === 'cable'">` → `<template v-else-if="selected?.kind === 'cable' || selected?.kind === 'run_cable'">`
  - replace the core table `<thead>…</tbody>` and the trailing hint (lines 724-736) with:

```vue
        <thead>
          <tr>
            <th>{{ t("cable.colCore") }}</th>
            <th>{{ t("cable.colColor") }}</th>
            <th v-if="!selectedIsRun">{{ t("cable.colWire") }}</th>
            <th v-if="selectedIsRun">{{ t("cable.colEndA") }}</th>
            <th v-if="selectedIsRun">{{ t("cable.colEndB") }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="c in selected.cores" :key="c.core">
            <td class="mono">{{ c.core }}</td>
            <td>{{ c.color }}</td>
            <td v-if="!selectedIsRun" class="mono">{{ c.wire ? t("cable.assigned") : t("cable.spare") }}</td>
            <td v-if="selectedIsRun" class="mono">{{ coreEndText(c.end_a) }}</td>
            <td v-if="selectedIsRun" class="mono">{{ coreEndText(c.end_b) }}</td>
          </tr>
          <tr v-for="n in Math.max(0, selected.core_count - selected.cores.length)" :key="`spare-${n}`" class="spare">
            <td class="mono">{{ selected.cores.length + n }}</td>
            <td>—</td>
            <td>{{ t("cable.spare") }}</td>
            <td v-if="selectedIsRun">—</td>
          </tr>
        </tbody>
      </table>
      <p v-if="selected.cores.length > selected.core_count" class="cable-hint warn">{{ t("cable.overflowHint") }}</p>
      <p class="cable-hint">{{ selectedIsRun ? t("cable.runHint") : t("cable.hint") }}</p>
```

- Locales. In `src/locales/en.json`, `zh.json`, `es.json`, `fr.json`, `de.json` replace `    "colWire": "Wire",` with:

```json
    "colWire": "Wire",
    "colEndA": "End A",
    "colEndB": "End B",
    "endWire": "breakout wire",
    "runHint": "Cores are joined end to end by their assignments: the pin where the single line lands, or the breakout wire the core continues as. Redraw the cable with connect_cable to change them",
```

In `src/locales/ja.json` replace `    "colWire": "配線",` with:

```json
    "colWire": "配線",
    "colEndA": "端A",
    "colEndB": "端B",
    "endWire": "分岐の配線",
    "runHint": "芯は割当てで端から端まで繋がります (1本線が着くピンか、分岐して続く配線)。割当てを変えるには connect_cable で引き直してください",
```

- [ ] **Step 6: Run the tests and the type check**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout && npx vitest run && npx vue-tsc --noEmit`
Expected: all Vitest tests PASS (including `entityKindLabels.test.ts` and `i18n.test.ts`, which now see `run_cable` and the new keys); `vue-tsc` reports no errors. If `vue-tsc` reports a `switch (…kind)` that no longer covers every `Entity` kind, add `case "run_cable":` next to that switch's `case "cable":` and rerun.

- [ ] **Step 7: Regenerate the spec docs and commit**

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout
python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check
git add src/ipc.ts src/canvas/{cable.ts,cable.test.ts,leader.ts,macroPreview.ts,viewClasses.ts,templatePreview.ts,renderer.ts} src/tools/controller.ts src/components/{entityKindLabels.ts,propertyCommands.ts,propertyCommands.test.ts,PropertiesPanel.vue} src/locales/*.json docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(ui): 種類 run_cable を扱い、プロパティの芯の表に端A・端Bを出す"
```

---

### Task 10: Canvas drawing and picking of run cables

**Files:**
- Modify: `src/canvas/cable.ts` (`shieldEllipse`, `cableHit`, `drawCable`; new run geometry)
- Modify: `src/canvas/renderer.ts:1341` (pass `sheet` and `defs`)
- Test: `src/canvas/cable.test.ts`

**Interfaces:**
- Consumes: Task 9 helpers; `resolveInstanceDef`, `transformLocal` from `./dynamicSymbol`; Task 8 numbers (must match exactly).
- Produces: `export function runMid(c: Cable): Point | null`, `runSlash(c): [Point, Point] | null`, `runCountAt(c): Point | null`, `runLabelAt(c): Point | null`, `runEndPoint(c: Cable, end: "a" | "b"): Point | null`, `pinEndPosition(sheet: Sheet, end: CoreEnd, defs: ReadonlyMap<string, SymbolDef>): Point | null`, `barEnds(positions: Point[]): [Point, Point] | null`, `runBars(sheet, c, defs): [Point, Point][]`, `breakoutPoint(sheet, c, end, wireId): Point | null`, `runFans(sheet, c): { from: Point; to: Point; core: string }[]`; `drawCable(ctx, vp, c, selected, sheet: Sheet, defs: ReadonlyMap<string, SymbolDef>)`.

- [ ] **Step 1: Write the failing tests**

In `src/canvas/cable.test.ts`, extend the import:

```ts
import {
  cableAddCommand,
  cableHit,
  cableLabel,
  cablePoints,
  coreEndPin,
  isCable,
  nextCableReference,
  runBars,
  runCountAt,
  runFans,
  runLabelAt,
  runSlash,
  shieldEllipse,
} from "./cable";
```

add this fixture after `function sheet(…)`:

```ts
/** LTEハーネス45 (Rust側 test_support::lte_sheet と同じ座標): J45 のピン x=57.5 から配線4本が x=77.5 まで出て、区間 (80,100)→(192.5,100) が反転した USB1 (ピン x=192.5) に着く。 */
function lteSheet(): Sheet {
  const sym = (id: string, reference: string, x: number, mirror: boolean) => ({
    kind: "symbol", id, symbol_id: "connector_4p", at: { x, y: 100 }, rotation: 0, mirror, reference, value: "", attrs: {},
  });
  const entities: Record<string, unknown> = { j45: sym("j45", "J45", 50, false), usb: sym("usb", "USB1", 200, true) };
  const cores = ["VBUS", "D-", "D+", "GND"].map((core, i) => {
    const y = 92.5 + 5 * i;
    entities[`w${i}`] = { kind: "wire", id: `w${i}`, points: [{ x: 57.5, y }, { x: 77.5, y }], color: "black", sq: 0.3, length_m: null, part_no: null, net: null };
    return { core, color: "", wire: null, end_a: { kind: "wire", wire: `w${i}` }, end_b: { kind: "pin", part: "usb", pin: String(i + 1) } };
  });
  entities.w45 = { kind: "run_cable", ...cable({ id: "w45", reference: "W45", points: [], cores, run: [{ x: 80, y: 100 }, { x: 192.5, y: 100 }] }) };
  return sheet(entities as Sheet["entities"]);
}
```

and append inside `describe("多芯ケーブル", …)`:

```ts
  // ja: 区間ケーブルの印は区間のいちばん長い線分の中点を通る45度の斜線で、芯数の文字・注記・シールドの楕円もそこを基準にする (Rust側と同じ)
  it("marks a run with a slash through the middle of its longest segment, like the printed sheet", () => {
    const c = cable({ points: [], run: [{ x: 80, y: 100 }, { x: 192.5, y: 100 }] });
    expect(runSlash(c)).toEqual([{ x: 134.75, y: 101.5 }, { x: 137.75, y: 98.5 }]);
    expect(runCountAt(c)).toEqual({ x: 138.25, y: 98 });
    expect(runLabelAt(c)).toEqual({ x: 139.75, y: 104 });
    expect(shieldEllipse(c)).toEqual({ center: { x: 136.25, y: 100 }, rx: 3, ry: 2, angle: -Math.PI / 4 });
  });

  // ja: 部品に着く端は着いたピンの先を結ぶ棒、分岐の端は区間の端から各芯の配線の端への扇になる
  it("draws a bar across the landed pins and a fan to each breakout wire", () => {
    const s = lteSheet();
    const c = s.entities.w45 as Cable;
    expect(runBars(s, c, new Map())).toEqual([[{ x: 192.5, y: 92.5 }, { x: 192.5, y: 107.5 }]]);
    const fans = runFans(s, c);
    expect(fans).toHaveLength(4);
    expect(fans[1]).toEqual({ from: { x: 80, y: 100 }, to: { x: 77.5, y: 97.5 }, core: "D-" });
  });

  // ja: 区間ケーブルは区間の線の近くをクリックすると掴める
  it("is picked by clicking near its run", () => {
    const c = cable({ points: [], run: [{ x: 80, y: 100 }, { x: 192.5, y: 100 }] });
    expect(cableHit(c, { x: 120, y: 100.5 }, 1)).toBe(true);
    expect(cableHit(c, { x: 120, y: 105 }, 1)).toBe(false);
  });
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout && npx vitest run src/canvas/cable.test.ts`
Expected: FAIL (`runSlash` is not exported).

- [ ] **Step 3: Implement**

In `src/canvas/cable.ts` change the imports to:

```ts
import type { Cable, CableCore, Command, CoreEnd, Entity, Point, Sheet, SymbolDef } from "../ipc";
import { resolveInstanceDef, transformLocal } from "./dynamicSymbol";
import { theme } from "./theme";
import type { Viewport } from "./viewport";
import { drawingText, setDrawingFont } from "./textSize";
```

Add after `CABLE_FONT`:

```ts
/** 区間の線の太さ mm (Rust `RUN_STROKE_MM`)。 */
const RUN_STROKE = 0.7;
/** 区間の斜線の印の半分の長さ mm (Rust `RUN_SLASH_HALF_MM`)。 */
const RUN_SLASH_HALF = 1.5;
/** 分岐の配線の端に添える芯の呼び名の文字高 mm (Rust `CORE_LABEL_MM`)。 */
const CORE_LABEL = 1.5;
```

Replace `shieldEllipse` with:

```ts
/** シールドの楕円 (Rust `shield_ellipse` と同じ): 中心・長半径・短半径・向き [rad]。区間ケーブルは斜線の印を囲む。 */
export function shieldEllipse(c: Cable): { center: Point; rx: number; ry: number; angle: number } | null {
  if (isRunCable(c)) {
    const m = runMid(c);
    return m ? { center: m, rx: 3, ry: 2, angle: -Math.PI / 4 } : null;
  }
  const [a, b] = c.points;
  if (!a || !b) return null;
  return {
    center: { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 },
    rx: Math.hypot(b.x - a.x, b.y - a.y) / 2 + 2.5,
    ry: 3,
    angle: Math.atan2(b.y - a.y, b.x - a.x),
  };
}
```

Add after `coreEndPin`:

```ts
/** 区間のいちばん長い線分の中点 (同じ長さなら後ろの線分。Rust `run_mid` と同じ)。 */
export function runMid(c: Cable): Point | null {
  const run = c.run ?? [];
  let best: Point | null = null;
  let bestLen = -1;
  for (let i = 0; i + 1 < run.length; i++) {
    const a = run[i];
    const b = run[i + 1];
    const len = Math.hypot(b.x - a.x, b.y - a.y);
    if (len >= bestLen) {
      bestLen = len;
      best = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
    }
  }
  return best;
}

/** 単線図の印: 中点を通る45度の短い斜線 (Rust `run_slash`)。 */
export function runSlash(c: Cable): [Point, Point] | null {
  const m = runMid(c);
  return m ? [{ x: m.x - RUN_SLASH_HALF, y: m.y + RUN_SLASH_HALF }, { x: m.x + RUN_SLASH_HALF, y: m.y - RUN_SLASH_HALF }] : null;
}

/** 芯数の文字の位置 (Rust `run_count_at`)。 */
export function runCountAt(c: Cable): Point | null {
  const m = runMid(c);
  return m ? { x: m.x + 2, y: m.y - 2 } : null;
}

/** ケーブル注記の位置 (Rust `run_label_at`)。 */
export function runLabelAt(c: Cable): Point | null {
  const m = runMid(c);
  return m ? { x: m.x + 3.5, y: m.y + 4 } : null;
}

/** 区間の端の点 (a = 最初の点、b = 最後の点)。 */
export function runEndPoint(c: Cable, end: "a" | "b"): Point | null {
  const run = c.run ?? [];
  return (end === "a" ? run[0] : run[run.length - 1]) ?? null;
}

function coreEnd(core: CableCore, end: "a" | "b"): CoreEnd | null | undefined {
  return end === "a" ? core.end_a : core.end_b;
}

/** ピンの行き先の用紙上の位置 (`index`番目、省略=最初の接続点。Rust `pin_end_position`)。 */
export function pinEndPosition(sheet: Sheet, end: CoreEnd, defs: ReadonlyMap<string, SymbolDef>): Point | null {
  if (end.kind !== "pin") return null;
  const part = sheet.entities[end.part];
  if (!part || part.kind !== "symbol") return null;
  const def = resolveInstanceDef(part, defs);
  const pins = (def?.pins ?? []).filter((p) => p.number === end.pin);
  const pin = pins[Math.max(0, (end.index ?? 1) - 1)];
  return pin ? transformLocal(pin.at, part) : null;
}

/** 棒の両端: 上→下、同じ高さなら左→右に並べた最初と最後 (Rust `bar_ends`)。 */
export function barEnds(positions: Point[]): [Point, Point] | null {
  if (positions.length === 0) return null;
  const sorted = [...positions].sort((a, b) => a.y - b.y || a.x - b.x);
  return [sorted[0], sorted[sorted.length - 1]];
}

/** 部品に着く端の棒 (Rust `run_bars`)。 */
export function runBars(sheet: Sheet, c: Cable, defs: ReadonlyMap<string, SymbolDef>): [Point, Point][] {
  const out: [Point, Point][] = [];
  for (const end of ["a", "b"] as const) {
    const positions = c.cores
      .map((k) => coreEnd(k, end))
      .filter((e): e is CoreEnd => !!e)
      .map((e) => pinEndPosition(sheet, e, defs))
      .filter((p): p is Point => p !== null);
    const bar = barEnds(positions);
    if (bar && Math.hypot(bar[1].x - bar[0].x, bar[1].y - bar[0].y) > 1e-6) out.push(bar);
  }
  return out;
}

/** 分岐側の端点: 芯の配線の端点のうち区間の端に近い方 (Rust `breakout_point`)。 */
export function breakoutPoint(sheet: Sheet, c: Cable, end: "a" | "b", wireId: string): Point | null {
  const at = runEndPoint(c, end);
  const w = sheet.entities[wireId];
  if (!at || !w || w.kind !== "wire" || w.points.length === 0) return null;
  const first = w.points[0];
  const last = w.points[w.points.length - 1];
  return Math.hypot(first.x - at.x, first.y - at.y) <= Math.hypot(last.x - at.x, last.y - at.y) ? first : last;
}

/** 分岐の扇: 区間の端から各芯の配線の分岐側の端へ (Rust `run_fans`)。 */
export function runFans(sheet: Sheet, c: Cable): { from: Point; to: Point; core: string }[] {
  const out: { from: Point; to: Point; core: string }[] = [];
  for (const end of ["a", "b"] as const) {
    const from = runEndPoint(c, end);
    if (!from) continue;
    for (const core of c.cores) {
      const e = coreEnd(core, end);
      if (e?.kind !== "wire") continue;
      const to = breakoutPoint(sheet, c, end, e.wire);
      if (to) out.push({ from, to, core: core.core });
    }
  }
  return out;
}
```

Replace `cableHit` with:

```ts
/** ケーブルを掴めるか (定義線ケーブルは定義線、区間ケーブルは区間の線)。 */
export function cableHit(c: Cable, world: Point, tolerance: number): boolean {
  if (isRunCable(c)) {
    const run = c.run ?? [];
    for (let i = 0; i + 1 < run.length; i++) {
      if (segmentDistance(world, run[i], run[i + 1]) <= tolerance) return true;
    }
    return false;
  }
  const [a, b] = c.points;
  return !!a && !!b && segmentDistance(world, a, b) <= tolerance;
}
```

Replace `drawCable` with:

```ts
/** ケーブルを描く (定義線ケーブルは定義線・シールド楕円・注記、区間ケーブルは drawRun)。選択中は選択色。 */
export function drawCable(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  c: Cable,
  selected: boolean,
  sheet: Sheet,
  defs: ReadonlyMap<string, SymbolDef>,
): void {
  const color = selected ? theme.selection : theme.annotation;
  if (isRunCable(c)) {
    drawRun(ctx, vp, c, color, selected, sheet, defs);
    return;
  }
  const [a, b] = c.points;
  if (!a || !b) return;
  const sa = vp.toScreen(a);
  const sb = vp.toScreen(b);
  ctx.strokeStyle = color;
  ctx.lineWidth = Math.max(1.2, 0.35 * vp.scale) + (selected ? 1 : 0);
  ctx.beginPath();
  ctx.moveTo(sa.x, sa.y);
  ctx.lineTo(sb.x, sb.y);
  ctx.stroke();
  drawShield(ctx, vp, c);
  const top = a.y <= b.y ? a : b;
  const at = vp.toScreen({ x: top.x + 1.5, y: top.y - 1.5 });
  ctx.fillStyle = color;
  setDrawingFont(ctx, CABLE_FONT, vp.scale, "monospace");
  ctx.textAlign = "left";
  ctx.textBaseline = "alphabetic";
  drawingText(ctx, cableLabel(c), at.x, at.y);
}

function drawShield(ctx: CanvasRenderingContext2D, vp: Viewport, c: Cable): void {
  const shield = c.shield ? shieldEllipse(c) : null;
  if (!shield) return;
  const center = vp.toScreen(shield.center);
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.ellipse(center.x, center.y, shield.rx * vp.scale, shield.ry * vp.scale, shield.angle, 0, Math.PI * 2);
  ctx.stroke();
}

/** 区間ケーブル: 太い線・斜線と芯数・部品側の棒・分岐の扇と芯の呼び名・シールド・注記 (SVG の render_run_cable と同じ形)。 */
function drawRun(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  c: Cable,
  color: string,
  selected: boolean,
  sheet: Sheet,
  defs: ReadonlyMap<string, SymbolDef>,
): void {
  const line = (a: Point, b: Point, mm: number) => {
    const sa = vp.toScreen(a);
    const sb = vp.toScreen(b);
    ctx.lineWidth = Math.max(1, mm * vp.scale);
    ctx.beginPath();
    ctx.moveTo(sa.x, sa.y);
    ctx.lineTo(sb.x, sb.y);
    ctx.stroke();
  };
  ctx.strokeStyle = color;
  ctx.fillStyle = color;
  ctx.lineWidth = Math.max(2, RUN_STROKE * vp.scale) + (selected ? 1 : 0);
  ctx.beginPath();
  (c.run ?? []).forEach((p, i) => {
    const s = vp.toScreen(p);
    if (i === 0) ctx.moveTo(s.x, s.y);
    else ctx.lineTo(s.x, s.y);
  });
  ctx.stroke();
  const slash = runSlash(c);
  if (slash) line(slash[0], slash[1], 0.35);
  for (const [a, b] of runBars(sheet, c, defs)) line(a, b, RUN_STROKE);
  ctx.textBaseline = "alphabetic";
  for (const fan of runFans(sheet, c)) {
    line(fan.from, fan.to, 0.35);
    const at = vp.toScreen({ x: fan.to.x, y: fan.to.y - 0.8 });
    setDrawingFont(ctx, CORE_LABEL, vp.scale, "monospace");
    ctx.textAlign = "center";
    drawingText(ctx, fan.core, at.x, at.y);
  }
  const count = runCountAt(c);
  if (count) {
    const at = vp.toScreen(count);
    setDrawingFont(ctx, 2, vp.scale, "monospace");
    ctx.textAlign = "left";
    drawingText(ctx, String(c.core_count), at.x, at.y);
  }
  drawShield(ctx, vp, c);
  const label = runLabelAt(c);
  if (label) {
    const at = vp.toScreen(label);
    setDrawingFont(ctx, CABLE_FONT, vp.scale, "monospace");
    ctx.textAlign = "left";
    drawingText(ctx, cableLabel(c), at.x, at.y);
  }
}
```

In `src/canvas/renderer.ts:1341` change the call to `drawCable(ctx, vp, e, opts.selection.has(e.id), sheet, defs);`.

- [ ] **Step 4: Run the tests and the type check**

Run: `cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout && npx vitest run && npx vue-tsc --noEmit`
Expected: PASS, no type errors.

- [ ] **Step 5: Regenerate the spec docs and commit**

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout
python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check
git add src/canvas/{cable.ts,cable.test.ts,renderer.ts} docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(ui): 作図領域に1本線のケーブルを描き、区間で掴めるようにする"
```

---

### Task 11: User docs and the final full check

**Files:**
- Modify: `docs/05-wire-management.md`, `docs/05-wire-management.ja.md`, `docs/10-automation-api.md`, `docs/10-automation-api.ja.md`

**Interfaces:**
- Consumes: all previous tasks (names, codes, column, tool).
- Produces: user-facing documentation.

- [ ] **Step 1: `docs/05-wire-management.md`**

Replace `` `.mdkproj` format is 15. DXF and KiCad exports do not carry cables`` with:

```markdown
`.mdkproj` format is 15. DXF and KiCad exports do not carry cables
- **Single-line cables (cable breakout)**: a cable can be drawn as **one line** on one end, or along its whole length, while every core keeps its own connection. Each core says where it goes at each end of the line: a **pin** of the part the line lands on (drawn as a bar across those pins), or the **breakout wire** it continues as (drawn as a short fan from the end of the line, with the core name). The netlist joins each core only through these assignments, never because the line touches something, so a line over a connector's pin column shorts nothing, and a cable drawn as one line between two connectors gets one net per core even without wires. The line is drawn thicker than a wire, with a slash and the core count. ERC adds `erc.cable_cores_shorted` (two cores of one cable in the same net, also for definition-line cables), `erc.cable_core_unassigned` (a core whose pin or wire is gone) and `erc.cable_run_end_detached` (a line end away from its part); breakout wire ends are not dangling. The From-To list has one row per core, end to end; wires that are cores of any cable are not counted again as loose wire in the BOM. `length_m` is the whole cable. Draw one with the MCP tool `connect_cable` (cores map to pins in pin order). A cable with a line is saved as the entity kind `run_cable` (`.mdkproj` format 17), so older apps refuse such files, and MadakeCAD refuses a file newer than itself
```

and replace `sheet, from, to, mate, remark, wire number, color, gauge, length, part number, harness` with `sheet, from, to, mate, remark, wire number, color, gauge, length, part number, harness, cable core (`W45/D-` for a wire that is a cable core; a single-line cable gives one row per core)`.

- [ ] **Step 2: `docs/05-wire-management.ja.md`**

Replace `` `.mdkproj`形式は15。DXF・KiCadの書き出しにはケーブルは載らない`` with:

```markdown
`.mdkproj`形式は15。DXF・KiCadの書き出しにはケーブルは載らない
- **1本線のケーブル(束線の分岐)**: ケーブルを片側だけ、または全長を**1本の線**で描いても、芯はそれぞれ自分の接続を保つ。芯ごとに、線の両端でどこへ行くかを持つ: 線が着く部品の**ピン**(着いたピンを横切る棒で描く)か、芯がそのまま続く**分岐の配線**(線の端から短い扇で描き、芯の呼び名を添える)。ネットリストは芯をこの割当てだけで繋ぎ、線が何かに触れていても繋がないので、コネクタのピン列の上に線を通しても短絡しない。2つのコネクタの間を1本線で描いたケーブルは、配線が無くても芯ごとに1ネットになる。線は配線より太く、斜線と芯数を付けて描く。ERCは`erc.cable_cores_shorted`(1本のケーブルの芯2本が同じネット。定義線ケーブルにも効く)・`erc.cable_core_unassigned`(芯の行き先のピンや配線が消えた)・`erc.cable_run_end_detached`(線の端が部品から離れている)を出し、分岐の配線の端は宙ぶらりんにしない。From-To電線リストは芯ごとに端から端まで1行。どれかのケーブルの芯になっている配線は、部品表で電線として重ねて数えない。`length_m`はケーブル全長。描くにはMCPツール`connect_cable`を使う(芯はピン順に対応する)。線を持つケーブルは種類`run_cable`で保存するので(`.mdkproj`形式17)、古いアプリはそのファイルを開けず、MadakeCADは自分より新しいファイルを断る
```

and replace `シート・From・To・嵌合相手・備考・線番・線色・sq・長さ・品番・ハーネス` with `シート・From・To・嵌合相手・備考・線番・線色・sq・長さ・品番・ハーネス・ケーブル芯(ケーブルの芯の配線は`W45/D-`。1本線のケーブルは芯ごとに1行)`.

- [ ] **Step 3: `docs/10-automation-api.md` and `.ja.md`**

English: replace `` `place_mate` (a plug/socket pair in one step; `mates` places many), `` with:

```markdown
`place_mate` (a plug/socket pair in one step; `mates` places many), `connect_cable` (a multi-core cable drawn as one line on one end or both: `cores` names the cores, `a`/`b` give each end's `pins` or `part` and `style: multi` (one wire per core from the pins to a breakout) or `single` (the line lands on the part); cores map to pins in pin order; `cables` draws many in one undoable edit), 
```

Japanese: replace `` `place_mate`(プラグとソケットの組を1回で置く。`mates` で複数)・`` with:

```markdown
`place_mate`(プラグとソケットの組を1回で置く。`mates` で複数)・`connect_cable`(多芯ケーブルを片側または両側を1本線で描いて引く。`cores`=芯の名前、`a`/`b`=端ごとに`pins`か`part`と`style: multi`(ピンから分岐点まで芯ごとに配線)/`single`(1本線を部品に着ける)。芯はピン順に対応する。`cables`で複数をundo一発の1編集で引く)・
```

- [ ] **Step 4: Full verification**

Run:

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout
touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs
(cd src-tauri && cargo test --workspace)
npx vitest run
npx vue-tsc --noEmit
python3 scripts/gen_spec.py --check
```

Expected: every Rust and Vitest test PASS, no type errors, `spec docs up to date; all tests have bilingual spec comments`.

- [ ] **Step 5: Commit**

```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/cable-breakout
git add docs/05-wire-management.md docs/05-wire-management.ja.md docs/10-automation-api.md docs/10-automation-api.ja.md
git commit -m "docs: 1本線のケーブル (束線の分岐) と connect_cable を利用者向け文書に書く"
```

---

## Delivery note

All tasks commit to `feature/cable-breakout`. When splitting into PRs (spec "Delivery"), base every PR on `master` and write "merge #N first" for dependents: PR 1 = Tasks 1-4 (core), PR 2 = Task 5 (reports), PR 3 = Tasks 8-10 (drawing), PR 4 = Tasks 6, 7, 11 (planner, MCP, docs).
