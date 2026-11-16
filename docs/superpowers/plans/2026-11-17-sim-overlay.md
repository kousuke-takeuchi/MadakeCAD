# Simulation Results on the Drawing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show the DC operating point results on the drawing (amber/red by load ratio, labels on overloaded items, a hover probe) and slim the results panel to findings + details of the selection.

**Architecture:** The core adds `SimOpResult.loads` (one entry per wire and per symbol: current, voltages, limit, ratio). The front end turns it into an overlay map in a pure module (`src/canvas/simOverlay.ts`), the store decides when it is shown and whether it is stale (document revision), the renderer paints it under the ink and labels on top, and the canvas shows a DOM tooltip on hover.

**Tech Stack:** Rust (madake-core, ngspice deck), Vue 3 + Pinia + TypeScript, Canvas2D, Vitest, cargo test.

**Spec:** `docs/superpowers/specs/2026-11-17-sim-overlay-design.md`

## Global Constraints

- Levels: `none` when ratio < 0.8 or no limit; `near` when 0.8 <= ratio <= 1.0; `over` when ratio > 1.0.
- Label text for `over` only, form `70.9A / 50A` (current with 1 decimal, limit as given).
- Limits are exactly the ones the findings use (wire ampacity from `sq`, fuse `rating_a` / value text, `pin_rating_a`, source `i_max_a`, converter `i_out_a`); a component with several reports the highest ratio.
- Stale = document revision differs from the revision of the run; stale overlay at reduced opacity, no labels, panel note; no automatic re-run.
- Tests are the specification: Rust `///` English + Japanese lines before each `#[test]`; Vitest `it("English")` with a `// ja:` line before it; run `python3 scripts/gen_spec.py` after test changes.
- Colours only from `src/canvas/theme.ts` (canvas) and CSS variables in `src/App.vue` (panel); no raw colours in components.
- UI appearance follows `MadakeCAD.pen` (Task 5 updates the design before Task 6 implements it).
- Nothing read from the confidential reference drawing in tests, commits or docs; run `python3 scripts/audit_confidential.py` before each commit; commit messages in Japanese with a conventional prefix, no attribution trailer.

## Review Focus

- A result from another sheet must not be painted on the active sheet (store test in Task 3).
- An entity deleted after the run must not break drawing or the probe (overlay test in Task 2, renderer test in Task 4).
- A failed run must leave no colours from the previous result (store test in Task 3).
- Wires without `sq` (most hand-drawn wires) have no limit and stay uncoloured, but still show current in the probe (core test in Task 1, overlay test in Task 2).
- A symbol with only feed-through pins (connector without a role) still gets a load entry with its pin limit (core test in Task 1).

## File Structure

- Modify `src-tauri/crates/madake-core/src/spice.rs`: `SpiceDeck.pin_nodes` (pin number -> node per symbol).
- Modify `src-tauri/crates/madake-core/src/sim.rs`: `EntityLoad`, `LoadKind`, `LimitKind`, `loads()`, `SimOpResult.loads`.
- Modify `src/ipc.ts`: `EntityLoad` type, `SimOpResult.loads`.
- Create `src/canvas/simOverlay.ts` (+ test): levels, labels, overlay map.
- Modify `src/stores/simulation.ts` (+ test): revision, toggle, overlay getter, selected load.
- Modify `src/canvas/theme.ts`, `src/canvas/renderer.ts` (+ test): `RenderOptions.sim`, `drawSimUnderlay`, `drawSimLabels`.
- Modify `src/components/CanvasView.vue`: pass `sim`, hover probe tooltip.
- Modify `MadakeCAD.pen` (pencil MCP): panel, probe tooltip, stale note.
- Modify `src/components/SimulationPanel.vue`, `src/locales/*.json` (6).
- Modify docs: `docs/06-verification-simulation(.ja).md`, `docs/internal/feature-inventory.md`, `docs/10-automation-api(.ja).md`, regenerated `docs/13-specification(.ja).md`.

---

### Task 1: Core per-entity loads

**Files:**
- Modify: `src-tauri/crates/madake-core/src/spice.rs` (struct `SpiceDeck`, symbol loop, node naming)
- Modify: `src-tauri/crates/madake-core/src/sim.rs`
- Modify: `src/ipc.ts` (types only)

**Interfaces:**
- Produces (Rust): `pub struct EntityLoad { pub entity_id: EntityId, pub kind: LoadKind, pub amps: f64, pub volts: Vec<f64>, pub limit: Option<f64>, pub limit_kind: Option<LimitKind>, pub ratio: Option<f64> }`, `#[serde(rename_all = "snake_case")] pub enum LoadKind { Wire, Component }`, `pub enum LimitKind { WireAmpacity, FuseRating, PinRating, SourceMax, ConverterOut }`, `SimOpResult.loads: Vec<EntityLoad>` (sorted: wires first then components, each by entity id).
- Produces (TS): `export interface EntityLoad { entity_id: string; kind: "wire" | "component"; amps: number; volts: number[]; limit: number | null; limit_kind: "wire_ampacity" | "fuse_rating" | "pin_rating" | "source_max" | "converter_out" | null; ratio: number | null }`, `SimOpResult.loads?: EntityLoad[]`.

- [ ] **Step 1: Write the failing tests** (append to the `tests` module in `sim.rs`, reusing `role_series`, `role_defs`, `place`, `add_wire`, `run`, `converter_sheet` already there)

```rust
    fn load_of<'a>(r: &'a SimOpResult, id: EntityId) -> &'a EntityLoad {
        r.loads.iter().find(|l| l.entity_id == id).expect("load entry")
    }

    fn id_of(sheet: &Sheet, reference: &str) -> EntityId {
        sheet
            .entities
            .values()
            .find_map(|e| match e {
                Entity::Symbol(s) if s.reference == reference => Some(s.id),
                _ => None,
            })
            .unwrap()
    }

    /// Each wire reports its current, the voltage at both ends and, from its size, the ampacity limit and the load ratio.
    /// 電線ごとに、電流・両端の電圧・太さから決まる許容電流と負荷率が出る。
    #[test]
    fn a_wire_load_has_current_end_voltages_and_ampacity() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        place(&mut sheet, "t_src", "BT1", 0.0, 100.0, &[("voltage", "24")]);
        place(&mut sheet, "t_load", "LD1", 50.0, 100.0, &[("current_a", "6")]);
        let w = add_wire(&mut sheet, &[(0.0, 100.0), (50.0, 100.0)], 0.75, Some(10.0));
        add_wire(&mut sheet, &[(50.0, 110.0), (0.0, 110.0)], 0.75, None);
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        let l = load_of(&r, w);
        assert_eq!(l.kind, LoadKind::Wire);
        assert!((l.amps - 6.0).abs() < 0.01, "{l:?}");
        assert_eq!(l.volts.len(), 2);
        // 0.0175 × 10m / 0.75sq = 0.233Ω → 6A で 1.4V 落ちる
        assert!((l.volts[0] - l.volts[1] - 1.4).abs() < 0.05, "{l:?}");
        assert_eq!(l.limit, Some(12.0));
        assert_eq!(l.limit_kind, Some(LimitKind::WireAmpacity));
        assert!((l.ratio.unwrap() - 0.5).abs() < 0.01);
    }

    /// A wire without a size, or thicker than the ampacity table, still reports its current but has no limit and no ratio.
    /// 太さの無い電線と、許容電流の表より太い電線も電流は出るが、限度と負荷率は無い。
    #[test]
    fn a_wire_without_a_known_ampacity_has_no_limit() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        place(&mut sheet, "t_src", "BT1", 0.0, 100.0, &[("voltage", "24")]);
        place(&mut sheet, "t_load", "LD1", 50.0, 100.0, &[("current_a", "6")]);
        let bare = add_wire(&mut sheet, &[(0.0, 100.0), (50.0, 100.0)], 0.0, None);
        let thick = add_wire(&mut sheet, &[(50.0, 110.0), (0.0, 110.0)], 8.0, None);
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        for id in [bare, thick] {
            let l = load_of(&r, id);
            assert!((l.amps - 6.0).abs() < 0.01 && l.limit.is_none() && l.ratio.is_none(), "{l:?}");
        }
    }

    /// A fuse reports its rating as the limit, and the voltage at each of its pins in pin order.
    /// ヒューズは定格を限度として出し、ピンごとの電圧をピンの順に出す。
    #[test]
    fn a_fuse_load_uses_its_rating() {
        let sheet = role_series(&[("voltage", "24")], &[("rating_a", "5")], &[("current_a", "8")]);
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        let l = load_of(&r, id_of(&sheet, "F1"));
        assert_eq!(l.kind, LoadKind::Component);
        assert_eq!((l.limit, l.limit_kind), (Some(5.0), Some(LimitKind::FuseRating)));
        assert!((l.ratio.unwrap() - 1.6).abs() < 0.01);
        assert_eq!(l.volts.len(), 2);
    }

    /// A connector without a role still gets a load entry, limited by its pin rating, because current flows through its same-numbered pins.
    /// 役割の無いコネクタも、同じ番号のピンを電流が通るので負荷の行が出て、ピン定格が限度になる。
    #[test]
    fn a_connector_load_uses_its_pin_rating() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        place(&mut sheet, "t_src", "BT1", 0.0, 100.0, &[("voltage", "24")]);
        place(&mut sheet, "t_cn", "CN1", 20.0, 100.0, &[("pin_rating_a", "10")]);
        place(&mut sheet, "t_load", "LD1", 50.0, 100.0, &[("current_a", "4")]);
        add_wire(&mut sheet, &[(0.0, 100.0), (20.0, 100.0)], 0.75, None);
        add_wire(&mut sheet, &[(30.0, 100.0), (50.0, 100.0)], 0.75, None);
        add_wire(&mut sheet, &[(50.0, 110.0), (0.0, 110.0)], 0.75, None);
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        let l = load_of(&r, id_of(&sheet, "CN1"));
        assert_eq!((l.limit, l.limit_kind), (Some(10.0), Some(LimitKind::PinRating)));
        assert!((l.ratio.unwrap() - 0.4).abs() < 0.01);
    }

    /// A source reports its maximum current as the limit and a converter its rated output current.
    /// 電源は許容電流を、変換器は定格の出力電流を限度として出す。
    #[test]
    fn source_and_converter_loads_use_their_ratings() {
        let mut sheet = converter_sheet(&[("v_out", "12"), ("eta", "0.8"), ("i_out_a", "5")]);
        for e in sheet.entities.values_mut() {
            if let Entity::Symbol(s) = e {
                if s.reference == "BT1" {
                    s.attrs.insert("i_max_a".into(), "2".into());
                }
            }
        }
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        let bt = load_of(&r, id_of(&sheet, "BT1"));
        assert_eq!((bt.limit, bt.limit_kind), (Some(2.0), Some(LimitKind::SourceMax)));
        assert!((bt.ratio.unwrap() - 1.25).abs() < 0.02, "{bt:?}");
        let u1 = load_of(&r, id_of(&sheet, "U1"));
        assert_eq!((u1.limit, u1.limit_kind), (Some(5.0), Some(LimitKind::ConverterOut)));
        assert!((u1.ratio.unwrap() - 0.8).abs() < 0.01, "{u1:?}");
    }

    /// A part with several limits (a fuse that also has a pin rating) reports the one it is closest to exceeding.
    /// 限度を複数持つ部品 (ピン定格もあるヒューズ) は、一番超えそうな限度を出す。
    #[test]
    fn the_highest_ratio_limit_wins() {
        let sheet = role_series(&[("voltage", "24")], &[("rating_a", "10"), ("pin_rating_a", "5")], &[("current_a", "4")]);
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        let l = load_of(&r, id_of(&sheet, "F1"));
        assert_eq!((l.limit, l.limit_kind), (Some(5.0), Some(LimitKind::PinRating)));
    }

    /// Exactly the parts whose ratio exceeds 1 have a fuse, pin, source or converter finding.
    /// 負荷率が 1 を超えた部品と、ヒューズ・ピン・電源・変換器の指摘が出る部品は一致する。
    #[test]
    fn loads_over_one_match_the_findings() {
        let sheet = role_series(&[("voltage", "24"), ("i_max_a", "3")], &[("rating_a", "5")], &[("current_a", "8")]);
        let Some(r) = run(&sheet, &SimOptions::default()) else { return };
        let mut over: Vec<EntityId> =
            r.loads.iter().filter(|l| l.kind == LoadKind::Component && l.ratio.unwrap_or(0.0) > 1.0).map(|l| l.entity_id).collect();
        let mut found: Vec<EntityId> = r
            .findings
            .iter()
            .filter(|f| !f.code.starts_with("sim.wire") && f.code != "sim.load_voltage")
            .flat_map(|f| f.entity_ids.clone())
            .collect();
        over.sort();
        over.dedup();
        found.sort();
        found.dedup();
        assert_eq!(over, found);
    }
```

- [ ] **Step 2: Run to see them fail**

Run: `cd src-tauri && export PATH="$HOME/.cargo/bin:$PATH" && cargo test -p madake-core --lib -- sim::tests`
Expected: compile errors `EntityLoad`, `LoadKind`, `LimitKind`, field `loads` not found.

- [ ] **Step 3: Record pin nodes in the deck (`spice.rs`)**

Add to `SpiceDeck` (after `load_terminals`):

```rust
    /// 部品ごとのピンのノード (定義のピンの順、同じ番号は最初の点)。回路につながらないピンは入らない。
    pub pin_nodes: Vec<(EntityId, Vec<(String, String)>)>,
```

In the symbol loop of `build_deck_with`, right after `let first_of = ...`, record raw keys:

```rust
        {
            let mut seen: Vec<&str> = Vec::new();
            let mut list: Vec<(String, String)> = Vec::new();
            for (no, p) in &pins {
                let no = no.trim();
                if seen.contains(&no) {
                    continue;
                }
                seen.push(no);
                list.push((no.to_string(), format!("{:?}", node_key(*p))));
            }
            raw_pin_nodes.push((s.id, list));
        }
```

Declare `let mut raw_pin_nodes: Vec<(EntityId, Vec<(String, String)>)> = Vec::new();` next to `raw_loads`. After the `load_terminals` mapping, resolve them, keeping only keys that are nodes of the deck:

```rust
    let pin_nodes = raw_pin_nodes
        .into_iter()
        .map(|(id, list)| {
            let list = list
                .into_iter()
                .filter_map(|(no, raw)| {
                    let key = parse_key(&raw)?;
                    names.get(&key).map(|n| (no, n.clone()))
                })
                .collect();
            (id, list)
        })
        .collect();
```

and add `pin_nodes,` to the returned `SpiceDeck`.

- [ ] **Step 4: Add the types and `loads()` (`sim.rs`)**

After `SimFinding`:

```rust
/// 電線か部品か。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LoadKind {
    Wire,
    Component,
}

/// 負荷率の限度の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LimitKind {
    WireAmpacity,
    FuseRating,
    PinRating,
    SourceMax,
    ConverterOut,
}

/// 電線・部品1つの電流・電圧・限度 (図面に重ねる表示とプローブに使う)。
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct EntityLoad {
    pub entity_id: EntityId,
    pub kind: LoadKind,
    /// 電線は区間の最大、部品は素子の最大 (A)。
    pub amps: f64,
    /// 電線は [始点, 終点]、部品はピンの順の電圧 (V)。
    pub volts: Vec<f64>,
    pub limit: Option<f64>,
    pub limit_kind: Option<LimitKind>,
    /// amps / limit。
    pub ratio: Option<f64>,
}
```

Add `pub loads: Vec<EntityLoad>,` to `SimOpResult` (after `findings`), compute it with `let loads = loads(sheet, symbols, &deck_opts, &deck, &volt_of, &amps_of);` before `Ok(SimOpResult {..})` and pass `loads`. The function:

```rust
/// 電線・部品ごとの負荷。限度は判定 (findings) と同じ規則で、部品で複数あれば負荷率の一番高いもの。
fn loads(
    sheet: &Sheet,
    symbols: &[SymbolDef],
    opts: &crate::spice::DeckOptions,
    deck: &crate::spice::SpiceDeck,
    volt_of: &dyn Fn(&str) -> f64,
    amps_of: &dyn Fn(&crate::spice::SpiceElement) -> f64,
) -> Vec<EntityLoad> {
    use crate::verify::{ampacity, parse_number, AMPACITY};
    let defs: BTreeMap<&str, &SymbolDef> = symbols.iter().map(|d| (d.id.as_str(), d)).collect();
    let table_max_sq = AMPACITY.last().map(|&(sq, _)| sq).unwrap_or(0.0);
    let mut out = Vec::new();

    // 電線: 区間を弧長の順に並べてあるので、最初の区間の node_a が始点、最後の区間の node_b が終点
    let mut by_wire: BTreeMap<EntityId, Vec<&crate::spice::SpiceElement>> = BTreeMap::new();
    for el in &deck.wire_elements {
        by_wire.entry(el.entity_id).or_default().push(el);
    }
    for (id, els) in by_wire {
        let Some(Entity::Wire(w)) = sheet.entities.get(&id) else { continue };
        let amps = els.iter().map(|e| amps_of(e)).fold(0.0, f64::max);
        let volts = vec![volt_of(&els[0].node_a), volt_of(&els[els.len() - 1].node_b)];
        let limit = (w.sq > 0.0 && w.sq <= table_max_sq + 1e-9).then(|| ampacity(w.sq));
        out.push(EntityLoad {
            entity_id: id,
            kind: LoadKind::Wire,
            amps,
            volts,
            limit,
            limit_kind: limit.map(|_| LimitKind::WireAmpacity),
            ratio: limit.map(|l| amps / l),
        });
    }

    // 部品: ラベル橋・短絡・接地の基準は数えない
    let mut by_symbol: BTreeMap<EntityId, Vec<&crate::spice::SpiceElement>> = BTreeMap::new();
    for el in &deck.component_elements {
        if matches!(el.kind, ElementKind::Label | ElementKind::Short(_) | ElementKind::Tie) {
            continue;
        }
        by_symbol.entry(el.entity_id).or_default().push(el);
    }
    let pin_nodes: BTreeMap<EntityId, &Vec<(String, String)>> = deck.pin_nodes.iter().map(|(id, l)| (*id, l)).collect();
    for (id, els) in by_symbol {
        let Some(Entity::Symbol(s)) = sheet.entities.get(&id) else { continue };
        let Some(def) = defs.get(s.symbol_id.as_str()) else { continue };
        let role = def.sim.as_ref().map(|r| r.role);
        let max_kind = |pred: &dyn Fn(&ElementKind) -> bool| -> f64 {
            els.iter().filter(|e| pred(&e.kind)).map(|e| amps_of(e)).fold(0.0, f64::max)
        };
        let amps = els.iter().map(|e| amps_of(e)).fold(0.0, f64::max);
        let mut candidates: Vec<(LimitKind, f64, f64)> = Vec::new(); // (種類, 限度, その限度と比べる電流)
        if role == Some(SimRole::Fuse) || (role.is_none() && def.id == "fuse") {
            if let Some(r) = opts.number(s, "rating_a").or_else(|| parse_number(&s.value)) {
                candidates.push((LimitKind::FuseRating, r, max_kind(&|k| matches!(k, ElementKind::Bridge))));
            }
        }
        if let Some(r) = opts.number(s, "pin_rating_a") {
            candidates.push((LimitKind::PinRating, r, max_kind(&|k| matches!(k, ElementKind::Pin(_) | ElementKind::Bridge))));
        }
        if role == Some(SimRole::Source) {
            if let Some(r) = opts.number(s, "i_max_a") {
                candidates.push((LimitKind::SourceMax, r, max_kind(&|k| matches!(k, ElementKind::Source))));
            }
        }
        if role == Some(SimRole::Converter) {
            if let Some(r) = opts.number(s, "i_out_a") {
                candidates.push((LimitKind::ConverterOut, r, max_kind(&|k| matches!(k, ElementKind::ConverterOut))));
            }
        }
        let best = candidates
            .into_iter()
            .filter(|(_, limit, _)| *limit > 0.0)
            .max_by(|a, b| (a.2 / a.1).total_cmp(&(b.2 / b.1)));
        let volts = pin_nodes.get(&id).map(|l| l.iter().map(|(_, n)| volt_of(n)).collect()).unwrap_or_default();
        out.push(EntityLoad {
            entity_id: id,
            kind: LoadKind::Component,
            amps,
            volts,
            limit: best.map(|b| b.1),
            limit_kind: best.map(|b| b.0),
            ratio: best.map(|b| b.2 / b.1),
        });
    }
    out
}
```

- [ ] **Step 5: Run the tests**

Run: `cd src-tauri && cargo test -p madake-core --lib -- sim:: spice::`
Expected: all pass (ngspice at `/opt/homebrew/bin`).

- [ ] **Step 6: TS types (`src/ipc.ts`)** — add after `SimFinding`:

```ts
/** 電線・部品1つの電流・電圧・限度 (図面に重ねる表示とプローブ)。volts は電線なら [始点, 終点]、部品ならピンの順。 */
export interface EntityLoad {
  entity_id: string;
  kind: "wire" | "component";
  amps: number;
  volts: number[];
  limit: number | null;
  limit_kind: "wire_ampacity" | "fuse_rating" | "pin_rating" | "source_max" | "converter_out" | null;
  ratio: number | null;
}
```

and `loads?: EntityLoad[];` in `SimOpResult`. Run `npx vue-tsc --noEmit` (expect clean).

- [ ] **Step 7: Commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/audit_confidential.py
git add src-tauri/crates/madake-core/src/spice.rs src-tauri/crates/madake-core/src/sim.rs src/ipc.ts docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat: シミュレーション結果に電線・部品ごとの電流・電圧・限度・負荷率 (loads) を足す"
```

---

### Task 2: Overlay model (`src/canvas/simOverlay.ts`)

**Files:**
- Create: `src/canvas/simOverlay.ts`, `src/canvas/simOverlay.test.ts`

**Interfaces:**
- Consumes: `EntityLoad`, `SimOpResult` from `src/ipc.ts`; `Sheet`.
- Produces: `export type OverlayLevel = "none" | "near" | "over"`, `export const NEAR_RATIO = 0.8`, `export function loadLevel(ratio: number | null): OverlayLevel`, `export function overLabel(l: EntityLoad): string`, `export interface OverlayItem { entityId: string; kind: "wire" | "component"; level: OverlayLevel; label: string | null }`, `export function buildOverlay(result: SimOpResult, sheet: Sheet): Map<string, OverlayItem>` (only `near`/`over` items of entities that still exist on `sheet`), `export function formatProbe(l: EntityLoad): { amps: string; volts: string; limit: string | null; ratio: string | null }`.

- [ ] **Step 1: Write the failing tests** (`src/canvas/simOverlay.test.ts`)

```ts
import { describe, expect, it } from "vitest";
import type { EntityLoad, Sheet, SimOpResult } from "../ipc";
import { buildOverlay, formatProbe, loadLevel, overLabel } from "./simOverlay";

const load = (id: string, amps: number, limit: number | null, kind: "wire" | "component" = "wire"): EntityLoad => ({
  entity_id: id,
  kind,
  amps,
  volts: [24, 23.5],
  limit,
  limit_kind: limit === null ? null : kind === "wire" ? "wire_ampacity" : "fuse_rating",
  ratio: limit === null ? null : amps / limit,
});
const result = (loads: EntityLoad[]): SimOpResult => ({ voltage: 24, nets: [], components: [], warnings: [], findings: [], loads });
const sheetWith = (ids: string[]): Sheet =>
  ({ id: "s1", entities: Object.fromEntries(ids.map((id) => [id, { kind: "wire", id, points: [] }])) }) as unknown as Sheet;

describe("simulation overlay", () => {
  // ja: 負荷率 0.8 未満と限度の無いものは色なし、0.8〜1.0 は注意 (橙)、1.0 を超えると超過 (赤)
  it("levels: below 0.8 or no limit is none, 0.8 to 1.0 is near, above 1.0 is over", () => {
    expect(loadLevel(null)).toBe("none");
    expect(loadLevel(0.79)).toBe("none");
    expect(loadLevel(0.8)).toBe("near");
    expect(loadLevel(1.0)).toBe("near");
    expect(loadLevel(1.01)).toBe("over");
  });

  // ja: 超過の札は「電流 (小数1桁) A / 限度 A」
  it("an over label reads current with one decimal over the limit", () => {
    expect(overLabel(load("w", 70.96, 50))).toBe("71.0A / 50A");
    expect(overLabel(load("w", 31, 30))).toBe("31.0A / 30A");
  });

  // ja: 重ねる表示には注意と超過だけが入り、超過にだけ札が付く
  it("the overlay holds only near and over items, and only over items get a label", () => {
    const m = buildOverlay(result([load("a", 1, 10), load("b", 9, 10), load("c", 12, 10), load("d", 5, null)]), sheetWith(["a", "b", "c", "d"]));
    expect([...m.keys()].sort()).toEqual(["b", "c"]);
    expect(m.get("b")).toEqual({ entityId: "b", kind: "wire", level: "near", label: null });
    expect(m.get("c")?.label).toBe("12.0A / 10A");
  });

  // ja: 計算の後で消された電線・部品は重ねる表示から外れる
  it("entities deleted after the run are left out", () => {
    const m = buildOverlay(result([load("gone", 12, 10)]), sheetWith([]));
    expect(m.size).toBe(0);
  });

  // ja: 古い結果 (loads の無い結果) でも空の表示になり、落ちない
  it("a result without loads gives an empty overlay", () => {
    const r = result([]);
    delete (r as { loads?: unknown }).loads;
    expect(buildOverlay(r, sheetWith(["a"])).size).toBe(0);
  });

  // ja: プローブは電流・電圧・限度・負荷率を読める文字にし、限度が無ければ限度と負荷率は出さない
  it("the probe text shows current, voltages, limit and ratio, without limit and ratio when there is none", () => {
    expect(formatProbe(load("w", 6, 12))).toEqual({ amps: "6.00 A", volts: "24.00 V → 23.50 V", limit: "12 A", ratio: "50%" });
    expect(formatProbe(load("w", 6, null))).toEqual({ amps: "6.00 A", volts: "24.00 V → 23.50 V", limit: null, ratio: null });
    expect(formatProbe({ ...load("p", 2, 5, "component"), volts: [24, 23.9, 0] }).volts).toBe("24.00 / 23.90 / 0.00 V");
  });
});
```

- [ ] **Step 2: Run to see them fail**

Run: `npx vitest run src/canvas/simOverlay.test.ts`
Expected: FAIL, module `./simOverlay` not found.

- [ ] **Step 3: Implement (`src/canvas/simOverlay.ts`)**

```ts
// シミュレーション結果を図面に重ねる表示の中身 (spec 2026-11-17-sim-overlay-design §2)。
// 結果 (SimOpResult.loads) から「エンティティ id → 色の段階と札」を作る純ロジック。描画は renderer.ts。

import type { EntityLoad, Sheet, SimOpResult } from "../ipc";

export type OverlayLevel = "none" | "near" | "over";

/** この負荷率から「注意」(橙)。 */
export const NEAR_RATIO = 0.8;

export interface OverlayItem {
  entityId: string;
  kind: "wire" | "component";
  level: OverlayLevel;
  /** 超過だけに付く札 (例 "71.0A / 50A")。 */
  label: string | null;
}

export function loadLevel(ratio: number | null): OverlayLevel {
  if (ratio === null || ratio < NEAR_RATIO) return "none";
  return ratio > 1 ? "over" : "near";
}

export function overLabel(l: EntityLoad): string {
  return `${l.amps.toFixed(1)}A / ${l.limit}A`;
}

/** 注意・超過の電線と部品 (計算の後で消えたものは除く)。 */
export function buildOverlay(result: SimOpResult, sheet: Sheet): Map<string, OverlayItem> {
  const out = new Map<string, OverlayItem>();
  for (const l of result.loads ?? []) {
    if (!(l.entity_id in sheet.entities)) continue;
    const level = loadLevel(l.ratio);
    if (level === "none") continue;
    out.set(l.entity_id, { entityId: l.entity_id, kind: l.kind, level, label: level === "over" ? overLabel(l) : null });
  }
  return out;
}

/** プローブ (吹き出し) の文字。電線の電圧は「始点 → 終点」、部品はピンの順に「/」で区切る。 */
export function formatProbe(l: EntityLoad): { amps: string; volts: string; limit: string | null; ratio: string | null } {
  const v = (x: number) => x.toFixed(2);
  const volts = l.kind === "wire" && l.volts.length === 2 ? `${v(l.volts[0])} V → ${v(l.volts[1])} V` : `${l.volts.map(v).join(" / ")} V`;
  return {
    amps: `${l.amps.toFixed(2)} A`,
    volts,
    limit: l.limit === null ? null : `${l.limit} A`,
    ratio: l.ratio === null ? null : `${Math.round(l.ratio * 100)}%`,
  };
}
```

- [ ] **Step 4: Run the tests** — `npx vitest run src/canvas/simOverlay.test.ts`, expect PASS.

- [ ] **Step 5: Commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/audit_confidential.py
git add src/canvas/simOverlay.ts src/canvas/simOverlay.test.ts docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat: シミュレーション結果を図面に重ねる表示の中身 (色の段階・札・プローブの文字) を作る"
```

---

### Task 3: Store: revision, toggle, overlay, selection

**Files:**
- Modify: `src/stores/simulation.ts`, `src/stores/simulation.test.ts`

**Interfaces:**
- Consumes: `buildOverlay`, `OverlayItem` (Task 2); `useDocumentStore()` fields `revision`, `activeSheetId`, `selection: Set<string>`, `activeSheet`.
- Produces (state): `resultRevision: number | null`, `showOnDrawing: boolean` (default `true`). Getters: `stale: boolean`, `overlay: { items: Map<string, OverlayItem>; stale: boolean } | null` (null when panel closed, toggle off, no result, or `sheetId !== activeSheetId`), `loadOf: (id: string) => EntityLoad | null`, `selectedLoad: EntityLoad | null` (first selected entity that has a load). Action: `toggleShowOnDrawing()`.

- [ ] **Step 1: Write the failing tests** (append inside the `describe` of `simulation.test.ts`; `result` there gains `findings: [], loads: [...]` locally)

```ts
  const withLoads = (ids: string[]): SimOpResult => ({
    ...result,
    findings: [],
    loads: ids.map((id) => ({ entity_id: id, kind: "wire" as const, amps: 12, volts: [24, 23], limit: 10, limit_kind: "wire_ampacity" as const, ratio: 1.2 })),
  });
  const docWith = (ids: string[], sheetId = "s1") => {
    const doc = useDocumentStore();
    doc.project = { sheets: [{ id: sheetId, entities: Object.fromEntries(ids.map((id) => [id, { kind: "wire", id, points: [] }])) }] } as unknown as typeof doc.project;
    doc.activeSheetId = sheetId;
    doc.revision = 5;
    return doc;
  };

  // ja: 結果は計算したシートにだけ重ね、ほかのシートを開いているときは描かない
  it("the overlay is shown only on the sheet that was simulated", async () => {
    const doc = docWith(["w1"]);
    const store = useSimulationStore();
    vi.spyOn(ipc, "simulateOp").mockResolvedValue(withLoads(["w1"]));
    await store.run("s1");
    expect(store.overlay?.items.get("w1")?.level).toBe("over");
    doc.activeSheetId = "other";
    expect(store.overlay).toBeNull();
  });

  // ja: 計算の後で図面を編集すると結果は古いとみなされ、重ねる表示は「古い」印付きになる
  it("editing the drawing after a run marks the result stale", async () => {
    const doc = docWith(["w1"]);
    const store = useSimulationStore();
    vi.spyOn(ipc, "simulateOp").mockResolvedValue(withLoads(["w1"]));
    await store.run("s1");
    expect(store.stale).toBe(false);
    doc.revision = 6;
    expect(store.stale).toBe(true);
    expect(store.overlay?.stale).toBe(true);
  });

  // ja: 計算に失敗すると、前の結果の色は残らない
  it("a failed run leaves no overlay from the previous result", async () => {
    docWith(["w1"]);
    const store = useSimulationStore();
    const spy = vi.spyOn(ipc, "simulateOp").mockResolvedValue(withLoads(["w1"]));
    await store.run("s1");
    spy.mockRejectedValue(new Error("ngspice"));
    await store.run("s1");
    expect(store.overlay).toBeNull();
  });

  // ja: 「図面に重ねる」を切ると表示が消え、パネルを閉じても消える
  it("turning off the drawing toggle or closing the panel hides the overlay", async () => {
    docWith(["w1"]);
    const store = useSimulationStore();
    vi.spyOn(ipc, "simulateOp").mockResolvedValue(withLoads(["w1"]));
    await store.run("s1");
    store.toggleShowOnDrawing();
    expect(store.overlay).toBeNull();
    store.toggleShowOnDrawing();
    expect(store.overlay).not.toBeNull();
    store.close();
    expect(store.overlay).toBeNull();
  });

  // ja: 図面で選んだ電線・部品の値を引ける (値の無い物を選んでいれば空)
  it("the selected wire or part's values can be looked up", async () => {
    const doc = docWith(["w1", "x"]);
    const store = useSimulationStore();
    vi.spyOn(ipc, "simulateOp").mockResolvedValue(withLoads(["w1"]));
    await store.run("s1");
    doc.selection = new Set(["x"]);
    expect(store.selectedLoad).toBeNull();
    doc.selection = new Set(["w1"]);
    expect(store.selectedLoad?.amps).toBe(12);
    expect(store.loadOf("w1")?.limit).toBe(10);
  });
```

- [ ] **Step 2: Run to see them fail** — `npx vitest run src/stores/simulation.test.ts`, expect FAIL (`overlay`, `stale`, `toggleShowOnDrawing` undefined).

- [ ] **Step 3: Implement** in `src/stores/simulation.ts`:
  - state: `resultRevision: null as number | null`, `showOnDrawing: true`.
  - in `run`, on success after setting `sheetId`: `this.resultRevision = useDocumentStore().revision;`; in `catch`: `this.resultRevision = null;` (result is already cleared there).
  - getters:

```ts
    stale(state): boolean {
      return state.result !== null && state.resultRevision !== useDocumentStore().revision;
    },
    overlay(state): { items: Map<string, OverlayItem>; stale: boolean } | null {
      const doc = useDocumentStore();
      const sheet = doc.activeSheet;
      if (!state.panelOpen || !state.showOnDrawing || !state.result || !sheet || state.sheetId !== doc.activeSheetId) return null;
      return { items: buildOverlay(state.result, sheet), stale: this.stale };
    },
    loadOf(state): (id: string) => EntityLoad | null {
      const byId = new Map((state.result?.loads ?? []).map((l) => [l.entity_id, l]));
      return (id: string) => byId.get(id) ?? null;
    },
    selectedLoad(): EntityLoad | null {
      for (const id of useDocumentStore().selection) {
        const l = this.loadOf(id);
        if (l) return l;
      }
      return null;
    },
```

  - action `toggleShowOnDrawing() { this.showOnDrawing = !this.showOnDrawing; }`.
  - imports: `import { buildOverlay, type OverlayItem } from "../canvas/simOverlay";` and `type EntityLoad` from `../ipc`.

- [ ] **Step 4: Run the tests** — `npx vitest run src/stores/simulation.test.ts`, expect PASS; `npx vue-tsc --noEmit` clean.

- [ ] **Step 5: Commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/audit_confidential.py
git add src/stores/simulation.ts src/stores/simulation.test.ts docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat: シミュレーション結果を図面に重ねる条件 (計算したシート・古さ・表示の切り替え) と選択中の値をストアに持つ"
```

---

### Task 4: Paint the overlay and the probe

**Files:**
- Modify: `src/canvas/theme.ts`, `src/canvas/renderer.ts`, `src/canvas/renderer.test.ts`, `src/components/CanvasView.vue`

**Interfaces:**
- Consumes: `OverlayItem` (Task 2), store getters `overlay`, `loadOf` (Task 3), `formatProbe` (Task 2), `controller.hitTest(world)`, `controller.vp.toWorld`, `symbolBounds` (`src/canvas/dynamicSymbol.ts`).
- Produces: `RenderOptions.sim?: SimPaint`, `export interface SimPaint { items: ReadonlyMap<string, OverlayItem>; stale: boolean }`, `export function drawSimUnderlay(ctx, vp, sheet, defs: Map<string, SymbolDef>, paint: SimPaint): void`, `export function drawSimLabels(ctx, vp, sheet, defs, paint): void`; theme tokens `simNear`, `simOver`, `simLabelBg`, `simLabelText`.

- [ ] **Step 1: Theme tokens** (`theme.ts`, inside `theme`):

```ts
  /** シミュレーション結果の重ね表示: 注意 (負荷率 0.8〜1.0) と超過 (1.0 超)。暗い背景で読める橙・赤。 */
  simNear: "#f5a524",
  simOver: "#ff5a4f",
  /** 超過の札の地と文字。 */
  simLabelBg: "rgba(120, 20, 16, 0.9)",
  simLabelText: "#ffe9e7",
```

- [ ] **Step 2: Write the failing renderer test** (new `describe` at the end of `renderer.test.ts`; reuse the file's helpers to build a sheet with one wire `w1` and one symbol `s1`, or build them inline):

```ts
describe("simulation overlay painting", () => {
  function rec() {
    const strokes: { style: string; width: number; alpha: number }[] = [];
    const rects: { style: string }[] = [];
    const texts: string[] = [];
    const ctx = {
      strokeStyle: "", fillStyle: "", lineWidth: 0, globalAlpha: 1, font: "", textAlign: "left", textBaseline: "alphabetic",
      lineCap: "butt", lineJoin: "miter",
      save() {}, restore() {}, beginPath() {}, moveTo() {}, lineTo() {}, closePath() {}, fill() {},
      stroke() { strokes.push({ style: String(ctx.strokeStyle), width: ctx.lineWidth, alpha: ctx.globalAlpha }); },
      strokeRect() { rects.push({ style: String(ctx.strokeStyle) }); },
      fillRect() {},
      fillText(t: string) { texts.push(t); },
      measureText(t: string) { return { width: t.length * 6 }; },
    };
    return { ctx: ctx as unknown as CanvasRenderingContext2D, strokes, rects, texts };
  }
  const wire = { kind: "wire", id: "w1", points: [{ x: 0, y: 0 }, { x: 20, y: 0 }], color: "", sq: 0.75 };
  const sheetOf = (entities: Record<string, unknown>) => ({ id: "s", entities }) as unknown as Sheet;

  // ja: 超過の電線は赤、注意の電線は橙の太い下敷き線で塗り、超過にだけ札を描く
  it("paints over wires red and near wires amber under the ink, and labels only over items", () => {
    const vp = new Viewport();
    const { ctx, strokes, texts } = rec();
    const sheet = sheetOf({ w1: wire, w2: { ...wire, id: "w2" } });
    const items = new Map([
      ["w1", { entityId: "w1", kind: "wire" as const, level: "over" as const, label: "12.0A / 10A" }],
      ["w2", { entityId: "w2", kind: "wire" as const, level: "near" as const, label: null }],
    ]);
    drawSimUnderlay(ctx, vp, sheet, new Map(), { items, stale: false });
    expect(strokes.map((s) => s.style)).toEqual([theme.simOver, theme.simNear]);
    drawSimLabels(ctx, vp, sheet, new Map(), { items, stale: false });
    expect(texts).toEqual(["12.0A / 10A"]);
  });

  // ja: 古い結果は薄く塗り、札は描かない
  it("a stale result is painted faintly and without labels", () => {
    const vp = new Viewport();
    const { ctx, strokes, texts } = rec();
    const sheet = sheetOf({ w1: wire });
    const items = new Map([["w1", { entityId: "w1", kind: "wire" as const, level: "over" as const, label: "12.0A / 10A" }]]);
    drawSimUnderlay(ctx, vp, sheet, new Map(), { items, stale: true });
    drawSimLabels(ctx, vp, sheet, new Map(), { items, stale: true });
    expect(strokes[0].alpha).toBeLessThan(0.5);
    expect(texts).toEqual([]);
  });

  // ja: 結果にあっても図面から消えた物は描かずに飛ばす
  it("items no longer on the sheet are skipped", () => {
    const { ctx, strokes } = rec();
    const items = new Map([["gone", { entityId: "gone", kind: "wire" as const, level: "over" as const, label: "x" }]]);
    drawSimUnderlay(ctx, new Viewport(), sheetOf({}), new Map(), { items, stale: false });
    expect(strokes).toEqual([]);
  });
});
```

(add `drawSimUnderlay, drawSimLabels` to the import from `./renderer`.)

- [ ] **Step 3: Run to see it fail** — `npx vitest run src/canvas/renderer.test.ts -t "simulation overlay"`, expect FAIL (not exported).

- [ ] **Step 4: Implement in `renderer.ts`**

```ts
/** シミュレーション結果の重ね表示 (spec 2026-11-17-sim-overlay-design §2)。 */
export interface SimPaint {
  items: ReadonlyMap<string, OverlayItem>;
  /** 図面が計算の後に変わった (薄く塗り、札は出さない)。 */
  stale: boolean;
}

const SIM_STALE_ALPHA = 0.35;
const SIM_ALPHA = 0.55;
/** 下敷き線の太さ (画面 px)。 */
const SIM_UNDERLAY_PX = 7;

function simColor(level: OverlayItem["level"]): string {
  return level === "over" ? theme.simOver : theme.simNear;
}

/** 注意・超過の電線と部品を図面の下に塗る (電線は太い下敷き線、部品は外形の枠)。 */
export function drawSimUnderlay(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  sheet: Sheet,
  defs: Map<string, SymbolDef>,
  paint: SimPaint,
): void {
  ctx.save();
  ctx.globalAlpha = paint.stale ? SIM_STALE_ALPHA * 0.5 : SIM_ALPHA;
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  for (const item of paint.items.values()) {
    const e = sheet.entities[item.entityId];
    if (!e) continue;
    ctx.strokeStyle = simColor(item.level);
    if (e.kind === "wire" && e.points.length >= 2) {
      ctx.lineWidth = SIM_UNDERLAY_PX;
      ctx.beginPath();
      const p0 = vp.toScreen(e.points[0]);
      ctx.moveTo(p0.x, p0.y);
      for (const p of e.points.slice(1)) {
        const s = vp.toScreen(p);
        ctx.lineTo(s.x, s.y);
      }
      ctx.stroke();
    } else if (e.kind === "symbol") {
      const def = defs.get(e.symbol_id);
      if (!def) continue;
      const { min, max } = symbolBounds(e, def);
      const a = vp.toScreen({ x: min.x - 1, y: min.y - 1 });
      const b = vp.toScreen({ x: max.x + 1, y: max.y + 1 });
      ctx.lineWidth = 3;
      ctx.strokeRect(a.x, a.y, b.x - a.x, b.y - a.y);
    }
  }
  ctx.restore();
}

/** 超過の札 (画面上で一定の大きさ)。古い結果では描かない。 */
export function drawSimLabels(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  sheet: Sheet,
  defs: Map<string, SymbolDef>,
  paint: SimPaint,
): void {
  if (paint.stale) return;
  ctx.save();
  ctx.font = "600 11px var(--mono-font), monospace";
  ctx.textBaseline = "middle";
  for (const item of paint.items.values()) {
    if (!item.label) continue;
    const e = sheet.entities[item.entityId];
    if (!e) continue;
    let anchor: { x: number; y: number } | null = null;
    if (e.kind === "wire" && e.points.length >= 2) {
      const mid = Math.floor((e.points.length - 1) / 2);
      const a = e.points[mid];
      const b = e.points[mid + 1];
      anchor = vp.toScreen({ x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 });
    } else if (e.kind === "symbol") {
      const def = defs.get(e.symbol_id);
      if (def) {
        const { max, min } = symbolBounds(e, def);
        anchor = vp.toScreen({ x: max.x + 1, y: min.y });
      }
    }
    if (!anchor) continue;
    const w = ctx.measureText(item.label).width + 8;
    ctx.fillStyle = theme.simLabelBg;
    ctx.fillRect(anchor.x + 4, anchor.y - 8, w, 16);
    ctx.fillStyle = theme.simLabelText;
    ctx.fillText(item.label, anchor.x + 8, anchor.y);
  }
  ctx.restore();
}
```

Add `sim?: SimPaint;` to `RenderOptions` (with a comment), import `type OverlayItem` from `./simOverlay` and `symbolBounds` from `./dynamicSymbol` (if not already imported). In `renderSheet`, call `if (opts.sim) drawSimUnderlay(ctx, vp, sheet, defs, opts.sim);` right after the harness loop (before cables, wires and symbols are drawn), and `if (opts.sim) drawSimLabels(ctx, vp, sheet, defs, opts.sim);` right before the agent overlay.

- [ ] **Step 5: Run the tests** — `npx vitest run src/canvas/renderer.test.ts`, expect PASS.

- [ ] **Step 6: Canvas wiring and probe (`CanvasView.vue`)**
  - `import { useSimulationStore } from "../stores/simulation";` `import { formatProbe } from "../canvas/simOverlay";` `const sim = useSimulationStore();`
  - in `draw()` pass `sim: sim.overlay ?? undefined,`.
  - redraw when results change: `watch(() => [sim.overlay, sim.showOnDrawing], scheduleDraw);` (next to the other watchers).
  - hover state: `const probe = ref<{ x: number; y: number; text: ReturnType<typeof formatProbe>; name: string } | null>(null);` In the existing `@pointermove` handler add a call `updateProbe(toLocal(e))`:

```ts
function updateProbe(p: { x: number; y: number }) {
  if (!sim.overlay && !(sim.panelOpen && sim.result && sim.sheetId === store.activeSheetId)) {
    probe.value = null;
    return;
  }
  const id = controller.hitTest(controller.vp.toWorld(p));
  const load = id ? sim.loadOf(id) : null;
  if (!id || !load) {
    probe.value = null;
    return;
  }
  const e = store.activeSheet?.entities[id];
  const name = e && e.kind === "symbol" ? e.reference || e.symbol_id : t("simulation.wire");
  probe.value = { x: p.x + 14, y: p.y + 14, text: formatProbe(load), name };
}
```

  - template, inside the canvas container (after the canvas element), styled per the Pencil design of Task 5 (class names `sim-probe`, rows for current, voltages, limit with ratio):

```vue
<div v-if="probe" class="sim-probe" :style="{ left: probe.x + 'px', top: probe.y + 'px' }">
  <div class="sim-probe-name">{{ probe.name }}</div>
  <div>{{ t("simulation.current") }} <b>{{ probe.text.amps }}</b></div>
  <div>{{ t("simulation.voltages") }} {{ probe.text.volts }}</div>
  <div v-if="probe.text.limit">{{ t("simulation.limit") }} {{ probe.text.limit }} ({{ probe.text.ratio }})</div>
</div>
```

  - `pointerleave` on the canvas clears `probe.value = null`.
  - Add the i18n keys used here in Task 6 (`simulation.wire`, `simulation.current`, `simulation.voltages`, `simulation.limit`); until then vue-i18n shows the key, which is acceptable inside this task.

- [ ] **Step 7: Run** `npx vue-tsc --noEmit` and `npx vitest run src/canvas` (expect clean / PASS).

- [ ] **Step 8: Commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/audit_confidential.py
git add src/canvas/theme.ts src/canvas/renderer.ts src/canvas/renderer.test.ts src/components/CanvasView.vue docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat: シミュレーション結果を図面に重ねて塗り、超過に札を付け、マウスを乗せると値を出す"
```

---

### Task 5: Design in Pencil (`MadakeCAD.pen`)

**Files:**
- Modify: `MadakeCAD.pen` (pencil MCP only; Pen.app must have the worktree's file open)

- [ ] **Step 1:** Read the frames "シミュレーション結果パネル" (`nSeR6`) and "検証結果パネル" (`GUQyd`) and the design-system board; reuse badge, chip and row styles.
- [ ] **Step 2:** Update `nSeR6`: header adds a toggle chip "図面に重ねる" (on/off like `DSトグルON`/`OFF` or a chip); a stale note row "図面が変わりました。再実行で更新" (warn colours) shown under the header; body: left column "定格超過" (as today, with severity filter chips ✗ ⚠ ℹ), right column "選択中" with a name row and rows 電流 / 電圧 / 限度 (種類) / 負荷率, plus an empty-state text "図面で電線か部品を選ぶと値が出ます"; a collapsed bar "すべての値を表示 ▸" at the bottom.
- [ ] **Step 3:** Add a frame "シミュレーション プローブ" next to the panel: small dark tooltip (chip colours of the canvas: `chipBg` background, `chipText` text) with name, 電流, 電圧, 限度 (負荷率).
- [ ] **Step 4:** Add a frame "シミュレーション 重ね表示の例": a dark canvas piece with one wire with a red underlay and a label "71.0A / 50A", one with an amber underlay, and a symbol box with a red frame.
- [ ] **Step 5:** Screenshot, check layout problems, save; ask the user to approve the design before Task 6. Commit `MadakeCAD.pen`:

```bash
git add MadakeCAD.pen && git commit -m "design: シミュレーション結果パネル (選択中の詳細・古い結果の表示・図面に重ねる切り替え) とプローブ・重ね表示の例"
```

---

### Task 6: Panel implementation

**Files:**
- Modify: `src/components/SimulationPanel.vue`, `src/locales/{ja,en,de,es,fr,zh}.json`

**Interfaces:**
- Consumes: store `stale`, `showOnDrawing`, `toggleShowOnDrawing`, `selectedLoad`, `findings`, `counts`, `conditions` (Task 3 and earlier); `formatProbe` (Task 2).

- [ ] **Step 1:** Implement the layout approved in Task 5:
  - header: existing title, source chip, severity badges, open chip, conditions bar unchanged; add a toggle button bound to `sim.showOnDrawing` / `sim.toggleShowOnDrawing()` labelled `t("simulation.showOnDrawing")`.
  - stale note: `<div v-if="sim.stale" class="stale">{{ t("simulation.stale") }}</div>` (colours `--warn-fg` / `--warn-bg`).
  - body: left = findings list with a severity filter (`const filter = ref<Set<SimFinding["severity"]>>(new Set(["error", "warning", "info"]))`, clicking a badge toggles it; rows unchanged); right = details: when `sim.selectedLoad` show name (reference from the active sheet's symbol or `t("simulation.wire")`), `formatProbe` fields and `t("simulation.limitKinds." + limit_kind)`; otherwise `t("simulation.selectHint")`.
  - bottom: `<button class="link" @click="showAll = !showAll">{{ showAll ? t("simulation.hideAll") : t("simulation.showAll") }}</button>` and, when open, the existing net voltage and component current columns.
- [ ] **Step 2:** Locale keys in all six files under `simulation`: `showOnDrawing`, `stale`, `details`, `selectHint`, `showAll`, `hideAll`, `wire`, `current`, `voltages`, `limit`, `ratio`, `limitKinds.wire_ampacity|fuse_rating|pin_rating|source_max|converter_out`. Japanese: 図面に重ねる / 図面が変わりました。再実行で更新してください / 選択中 / 図面で電線か部品を選ぶと値が出ます / すべての値を表示 / 値の一覧を閉じる / 電線 / 電流 / 電圧 / 限度 / 負荷率 / 電線の許容電流・ヒューズ定格・ピン定格・電源の許容電流・変換器の定格出力. English: Show on drawing / The drawing changed. Run again to update / Selected / Select a wire or part on the drawing to see its values / Show all values / Hide all values / Wire / Current / Voltages / Limit / Load / Wire ampacity, Fuse rating, Pin rating, Source maximum, Converter rated output; translate the same keys for de, es, fr, zh.
- [ ] **Step 3:** Add a store-level test only if new logic lands in the store; the filter state is component-local. Run `npx vitest run` (i18n parity test must pass) and `npx vue-tsc --noEmit`.
- [ ] **Step 4: Commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/audit_confidential.py
git add src/components/SimulationPanel.vue src/locales docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat: シミュレーション結果パネルを定格超過と選択中の詳細の2列にし、古い結果の表示と重ね表示の切り替えを足す"
```

---

### Task 7: Docs and full verification

**Files:**
- Modify: `docs/06-verification-simulation.md`, `docs/06-verification-simulation.ja.md`, `docs/internal/feature-inventory.md`, `docs/10-automation-api.md`, `docs/10-automation-api.ja.md`

- [ ] **Step 1:** 06 (en/ja): results on the drawing (amber 0.8–1.0, red above 1.0, labels on overloaded items, hover probe, stale results faint, toggle) and the two-column panel. 10 (en/ja): `/simulate/op` result also has `loads` (per wire and part: current, voltages, limit, limit kind, ratio). feature-inventory: update the simulation rows.
- [ ] **Step 2:** Full verification: `cd src-tauri && cargo test --workspace` (the known environment failure `frame_templates_api` from a user frame in `~/MadakeCAD/templates/frames` is acceptable; note it), `npx vitest run`, `npx vue-tsc --noEmit`, `python3 scripts/gen_spec.py --check`, `python3 scripts/audit_confidential.py`.
- [ ] **Step 3:** Commit docs: `git commit -m "docs: シミュレーション結果を図面に重ねる表示の説明を足す"`.
- [ ] **Step 4:** Manual check: dev build on port 9330 (`npx vite --port 1430 --strictPort` and `MADAKE_MCP_PORT=9330 MADAKE_USER_SYMBOLS_PATH=<repo>/docs/references/repro/a0/sim/symbols npx tauri dev --config '{"build":{"devUrl":"http://localhost:1430","beforeDevCommand":""}}'`), open `docs/references/repro/a0/sim/a0-sim.mdkproj` with `cargo run -p madake-cli -- --port 9330 open <path>`, run the simulation with `DRV01.current_a=31` and `DRV02.current_a=31`, confirm red wires and parts with labels, the probe, the details column, stale fading after moving a part. Then one opus review of the whole branch.
