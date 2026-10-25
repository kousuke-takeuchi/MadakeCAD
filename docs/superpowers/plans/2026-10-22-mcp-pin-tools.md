# MCP pin tools Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add MCP tools `get_pins`, `connect_pins` and `place_mate`, and make `place_symbol` return pins, so the in-app agent wires by pin name instead of computing coordinates.

**Architecture:** The geometry lives in a new pure module `madake-core/src/pin_wiring.rs`: pin positions and directions after rotation and mirror, pin-specifier resolution, right-angle routing, and the plug/socket pair layout. The MCP crate (`madake-mcp/src/lib.rs`) adds thin tools that read the sheet, call the module, and edit only through `Command::AddEntity` (one batch for a mate pair). The agent's system prompt gets one rule that points at the tools.

**Tech Stack:** Rust (madake-core, madake-mcp with rmcp 3.1.4, madake-agent), serde, schemars, tokio tests through `McpToolBridge`.

**Spec:** `docs/superpowers/specs/2026-10-22-mcp-pin-tools-design.md`

## Global Constraints

- Work in worktree `/Users/ktakeuchi/Documents/MadakeCAD/.worktrees/mcp-pin-tools`, branch `feature/mcp-pin-tools`. Never switch branches in the main checkout.
- Every test has a `///` English line and a `///` Japanese line directly above it (the spec generator requires both). Test function names are English.
- Code comments and tool descriptions follow the surrounding code: Japanese.
- Pin specifier: `"<ref>.<pin>"`, split at the **last** dot; `<ref>` may be an entity id (UUID); `<pin>` matches the pin number first, then the pin name.
- Route bends snap to the 2.5 mm grid. No diagonal segment. No obstacle avoidance.
- Mate layout: the two parts sit 3.5 mm apart; `reference` and `value` go on the socket.
- All edits go through `Command::AddEntity` (`SharedDoc::execute` for one entity, `SharedDoc::execute_batch_as(cmds, self.doc.mcp_origin())` for the pair).
- On any resolution error, nothing is created.
- `place_symbol` keeps returning `entity_id` and `revision`; `pins` is added.
- Commit messages: Conventional Commits, Japanese summary, **no `Co-Authored-By` or other AI trailer**.
- After test changes: `python3 scripts/gen_spec.py` (CI runs `--check`).

## Review Focus

- A pin on a part that is rotated **and** mirrored: the reported `dir` must match where the wire actually leaves. Covered in Task 1 (`world_dir` test with rotation 90 + mirror).
- A block whose pins are given only by name (`"CTRL1.入力1"`): name lookup must work when the number differs. Covered in Task 2 (`resolve_pin` name test).
- Two pins that are already aligned but whose directions differ (e.g. Right then Up on the same y): must give one straight segment, not an L with a zero-length leg. Covered in Task 2 (straight test uses Right/Up on the same y).
- A reference with a dot in it (`"X1.A.3"`): must split at the last dot. Covered in Task 2.
- Bends from pins that are off the 2.5 mm grid: the bend snaps, but the end vertices stay exactly on the pins, so the pins stay connected. Covered in Task 2 (off-grid Z test) and Task 3 (`run_verification` after `connect_pins`).

---

### Task 1: Pin positions and `get_pins`

**Files:**
- Create: `src-tauri/crates/madake-core/src/pin_wiring.rs`
- Modify: `src-tauri/crates/madake-core/src/lib.rs` (add `pub mod pin_wiring;` after `pub mod pin_groups;`)
- Modify: `src-tauri/crates/madake-mcp/src/lib.rs` (params struct near `PlaceSymbolParams` ~line 460; tool after `draw_wire` ~line 1064; `place_symbol` return ~line 1060)
- Test: `src-tauri/crates/madake-core/src/pin_wiring.rs` (inline `mod tests`), `src-tauri/crates/madake-mcp/tests/drawing_tools.rs` (new)

**Interfaces:**
- Produces (core, `madake_core::pin_wiring`):
  - `pub struct PinInfo { pub number: String, pub name: String, pub x: f64, pub y: f64, pub dir: PinDir }` (Serialize, JsonSchema, Clone, Debug, PartialEq)
  - `pub struct PartPins { pub entity_id: EntityId, pub reference: String, pub symbol_id: String, pub bbox: Option<Rect>, pub pins: Vec<PinInfo> }`
  - `pub fn world_dir(dir: PinDir, inst: &SymbolInstance) -> PinDir`
  - `pub fn part_pins(inst: &SymbolInstance, def: &SymbolDef) -> PartPins`
  - `pub fn find_parts(sheet: &Sheet, defs: &[SymbolDef], key: &str) -> Vec<PartPins>` (key = reference, or entity id string)
  - `pub fn sheet_part_pins(sheet: &Sheet, entity_id: EntityId) -> Option<PartPins>` (builds defs itself; for `place_symbol`)
- Produces (MCP): tool `get_pins` with `GetPinsParams { sheet_id: Option<Uuid>, reference: Option<String>, entity_id: Option<Uuid> }`, returns `{"parts": [PartPins...]}`. `place_symbol` returns `{"entity_id", "revision", "pins"}`.

- [ ] **Step 1: Write the failing core tests**

Create `src-tauri/crates/madake-core/src/pin_wiring.rs` with only the test module first:

```rust
//! ピン名で配線するための計算 (MCPの get_pins / connect_pins / place_mate)。
//!
//! エージェントがピン座標を手で計算しなくて済むように、配置済み部品のピンの用紙座標・向きと、
//! ピンどうしを結ぶ直角配線の頂点列をここで求める。編集そのものは呼び出し側がCommandで行う。

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Entity, Sheet, SymbolInstance};
    use crate::symbol::{resolve_symbol, sheet_symbol_defs};
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn inst(symbol_id: &str, x: f64, y: f64, rotation: u16, mirror: bool, reference: &str) -> SymbolInstance {
        SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: symbol_id.into(),
            at: Point::new(x, y),
            rotation,
            mirror,
            reference: reference.into(),
            value: String::new(),
            attrs: BTreeMap::new(),
        }
    }

    fn block(reference: &str, x: f64, y: f64) -> SymbolInstance {
        let mut b = inst("block", x, y, 0, false, reference);
        b.attrs.insert(
            "block_pins".into(),
            r#"{"left":[{"no":"IN1","name":"入力1"}],"right":[{"no":"OUT1","name":"出力1"}]}"#.into(),
        );
        b
    }

    pub(super) fn sheet_with(parts: Vec<SymbolInstance>) -> Sheet {
        let mut sheet = Sheet::new("試験");
        for p in parts {
            sheet.entities.insert(p.id, Entity::Symbol(p));
        }
        sheet
    }

    /// A plug's pin is reported at its paper position, and its direction follows rotation and mirror.
    /// プラグのピンは用紙上の位置で返り、向きは回転と反転に合わせて変わる。
    #[test]
    fn pin_direction_follows_rotation_and_mirror() {
        let def = resolve_symbol("connector_plug").unwrap();
        let plain = part_pins(&inst("connector_plug", 50.0, 40.0, 0, false, "J1"), &def);
        assert_eq!((plain.pins[0].x, plain.pins[0].y), (42.5, 40.0));
        assert_eq!(plain.pins[0].dir, PinDir::Left);
        let mirrored = part_pins(&inst("connector_plug", 50.0, 40.0, 0, true, "J1"), &def);
        assert_eq!(mirrored.pins[0].dir, PinDir::Right);
        let turned = part_pins(&inst("connector_plug", 50.0, 40.0, 90, false, "J1"), &def);
        assert_eq!((turned.pins[0].x, turned.pins[0].y), (50.0, 32.5));
        assert_eq!(turned.pins[0].dir, PinDir::Up);
        let both = part_pins(&inst("connector_plug", 50.0, 40.0, 90, true, "J1"), &def);
        assert_eq!((both.pins[0].x, both.pins[0].y), (50.0, 47.5));
        assert_eq!(both.pins[0].dir, PinDir::Down);
    }

    /// A block's named pins are reported with their numbers, names, outside positions and the rounded outline.
    /// ブロックの名前付きピンは番号・名前・外側の位置で返り、外形は丸めた後の大きさで返る。
    #[test]
    fn block_pins_and_outline_are_reported() {
        let b = block("CTRL1", 100.0, 50.0);
        let sheet = sheet_with(vec![b.clone()]);
        let parts = find_parts(&sheet, &sheet_symbol_defs(&sheet), "CTRL1");
        assert_eq!(parts.len(), 1);
        let p = &parts[0];
        assert_eq!(p.entity_id, b.id);
        let in1 = p.pins.iter().find(|x| x.number == "IN1").unwrap();
        assert_eq!(in1.name, "入力1");
        assert_eq!(in1.dir, PinDir::Left);
        let bbox = p.bbox.expect("外形");
        assert!(in1.x <= bbox.min.x + 1e-9 && in1.x < 100.0);
        let out1 = p.pins.iter().find(|x| x.number == "OUT1").unwrap();
        assert_eq!(out1.dir, PinDir::Right);
        assert!(out1.x >= bbox.max.x - 1e-9);
    }

    /// Looking up a reference returns every part that carries it, and an entity id finds exactly that part.
    /// 参照記号で引くとその参照記号の部品を全部返し、entity id で引くとその部品だけを返す。
    #[test]
    fn parts_are_found_by_reference_or_entity_id() {
        let a = inst("connector_socket", 10.0, 10.0, 0, false, "J2");
        let b = inst("connector_socket", 10.0, 30.0, 0, false, "J2");
        let sheet = sheet_with(vec![a.clone(), b.clone()]);
        let defs = sheet_symbol_defs(&sheet);
        assert_eq!(find_parts(&sheet, &defs, "J2").len(), 2);
        let by_id = find_parts(&sheet, &defs, &a.id.to_string());
        assert_eq!(by_id.len(), 1);
        assert_eq!(by_id[0].entity_id, a.id);
        assert!(find_parts(&sheet, &defs, "J9").is_empty());
    }
}
```

Add `pub mod pin_wiring;` to `src-tauri/crates/madake-core/src/lib.rs` after `pub mod pin_groups;`. If `Sheet::new` does not exist with that signature, use the constructor the other inline tests in `madake-core/src/mate.rs` use (check its `mod tests`).

- [ ] **Step 2: Run to verify it fails**

Run: `cd src-tauri && cargo test -p madake-core pin_wiring`
Expected: compile errors (`part_pins`, `find_parts`, `PinDir` not found).

- [ ] **Step 3: Implement**

Insert above `#[cfg(test)]` in `pin_wiring.rs`:

```rust
use serde::Serialize;

use crate::geometry::Point;
use crate::model::{Entity, EntityId, Sheet, SymbolInstance};
use crate::netlist::{rotate_local, transform_local};
use crate::symbol::{instance_symbol_def, sheet_symbol_defs, PinDir, SymbolDef};
use crate::tidy::{symbol_bounds, Rect};

/// 配置済みピン1本 (用紙座標・回転と反転の後の向き)。
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct PinInfo {
    pub number: String,
    pub name: String,
    pub x: f64,
    pub y: f64,
    /// 電線が出ていく向き (回転・反転の後)。
    pub dir: PinDir,
}

/// 配置済み部品1個のピン一覧と外形。
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct PartPins {
    pub entity_id: EntityId,
    pub reference: String,
    pub symbol_id: String,
    /// 用紙上の外接矩形 (ブロックは5mm丸めの後)。図形の無いシンボルはnull。
    pub bbox: Option<Rect>,
    pub pins: Vec<PinInfo>,
}

/// ピンの向き (回転0度の定義) を、配置の回転・反転を通した用紙上の向きにする。
pub fn world_dir(dir: PinDir, inst: &SymbolInstance) -> PinDir {
    let v = match dir {
        PinDir::Up => Point::new(0.0, -1.0),
        PinDir::Down => Point::new(0.0, 1.0),
        PinDir::Left => Point::new(-1.0, 0.0),
        PinDir::Right => Point::new(1.0, 0.0),
    };
    let r = rotate_local(v, inst.rotation, inst.mirror);
    if r.x.abs() > r.y.abs() {
        if r.x > 0.0 { PinDir::Right } else { PinDir::Left }
    } else if r.y > 0.0 {
        PinDir::Down
    } else {
        PinDir::Up
    }
}

/// 部品1個のピン (用紙座標・向き) と外形。
pub fn part_pins(inst: &SymbolInstance, def: &SymbolDef) -> PartPins {
    let pins = def
        .pins
        .iter()
        .map(|pin| {
            let at = transform_local(pin.at, inst);
            PinInfo {
                number: pin.number.clone(),
                name: pin.name.clone(),
                x: at.x,
                y: at.y,
                dir: world_dir(pin.dir, inst),
            }
        })
        .collect();
    PartPins {
        entity_id: inst.id,
        reference: inst.reference.clone(),
        symbol_id: inst.symbol_id.clone(),
        bbox: symbol_bounds(inst, def),
        pins,
    }
}

/// シート上の部品を参照記号 (完全一致) か entity id で探す。見つかった順 (entity id順) に返す。
pub fn find_parts(sheet: &Sheet, defs: &[SymbolDef], key: &str) -> Vec<PartPins> {
    let by_id = key.parse::<EntityId>().ok();
    sheet
        .entities
        .values()
        .filter_map(|e| match e {
            Entity::Symbol(s) if Some(s.id) == by_id || s.reference == key => Some(s),
            _ => None,
        })
        .filter_map(|s| instance_symbol_def(s, defs).map(|def| part_pins(s, &def)))
        .collect()
}

/// entity id の部品1個のピン (定義はシートから集める)。シンボルでなければNone。
pub fn sheet_part_pins(sheet: &Sheet, entity_id: EntityId) -> Option<PartPins> {
    let defs = sheet_symbol_defs(sheet);
    match sheet.entities.get(&entity_id)? {
        Entity::Symbol(s) => instance_symbol_def(s, &defs).map(|def| part_pins(s, &def)),
        _ => None,
    }
}
```

If `rotate_local` / `transform_local` are not `pub` in `netlist.rs`, they are (lines 19 and 45); `PinDir` is `pub` in `symbol.rs`. If `Rect` lacks `schemars::JsonSchema`, it has it (tidy.rs line 75).

- [ ] **Step 4: Run core tests**

Run: `cd src-tauri && cargo test -p madake-core pin_wiring`
Expected: 3 passed. If the rotated+mirrored case fails, print `rotate_local(Point::new(-7.5,0.0),90,true)` and fix the expected numbers only if they disagree with how `transform_local` draws the symbol (the drawing is the truth; `world_dir` must agree with it).

- [ ] **Step 5: Write the failing MCP test**

Create `src-tauri/crates/madake-mcp/tests/drawing_tools.rs`:

```rust
//! ピン名で配線するMCPツール (get_pins / connect_pins / place_mate、place_symbol のピン) のテスト。

use madake_agent::tools::ToolBridge;
use madake_core::{Engine, Project};
use madake_mcp::tool_bridge::McpToolBridge;
use madake_mcp::{tool_descriptions, SharedDoc};
use serde_json::{json, Value};
use uuid::Uuid;

fn fixture() -> (SharedDoc, McpToolBridge) {
    let doc = SharedDoc::new(Engine::new(Project::new("ピン試験")));
    let parts = madake_mcp::open_parts(
        &std::env::temp_dir().join(format!("madake-pins-{}.sqlite", Uuid::new_v4())),
    )
    .expect("部品DB");
    let bridge = McpToolBridge::new(doc.clone(), parts);
    (doc, bridge)
}

fn first_sheet(doc: &SharedDoc) -> Uuid {
    doc.engine.lock().unwrap().project().sheets[0].id
}

async fn call(bridge: &McpToolBridge, name: &str, input: Value) -> Value {
    let outcome = bridge.call(name, input).await;
    assert!(!outcome.is_error, "{name} が失敗した: {}", outcome.content);
    serde_json::from_str(&outcome.content).expect("JSON")
}

async fn call_err(bridge: &McpToolBridge, name: &str, input: Value) -> String {
    let outcome = bridge.call(name, input).await;
    assert!(outcome.is_error, "{name} が成功してしまった: {}", outcome.content);
    outcome.content
}

const ONE_PIN_BLOCK: &str = r#"{"left":[{"no":"1","name":"A"}]}"#;
const TWO_PIN_BLOCK: &str = r#"{"left":[{"no":"IN1","name":"入力1"}],"right":[{"no":"OUT1","name":"出力1"}]}"#;

async fn place_block(bridge: &McpToolBridge, sheet: Uuid, reference: &str, x: f64, y: f64, pins: &str) -> Value {
    call(
        bridge,
        "place_symbol",
        json!({"sheet_id": sheet, "symbol_id": "block", "x": x, "y": y,
               "reference": reference, "attrs": {"block_pins": pins}}),
    )
    .await
}

/// place_symbol returns the new part's pins with paper positions, so the agent can wire it without computing coordinates.
/// place_symbol は置いた部品のピンを用紙上の位置つきで返すので、エージェントは座標を計算せずに配線できる。
#[tokio::test]
async fn place_symbol_returns_the_pins_of_the_new_part() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    let placed = place_block(&bridge, sheet, "CTRL1", 100.0, 50.0, TWO_PIN_BLOCK).await;
    assert!(placed["entity_id"].is_string());
    assert!(placed["revision"].is_number());
    let pins = placed["pins"].as_array().expect("pins");
    let in1 = pins.iter().find(|p| p["number"] == "IN1").expect("IN1");
    assert_eq!(in1["name"], "入力1");
    assert_eq!(in1["dir"], "left");
    assert!(in1["x"].as_f64().unwrap() < 100.0);
}

/// get_pins finds parts by reference or entity id and returns their pins and outline.
/// get_pins は参照記号か entity id で部品を探し、ピンと外形を返す。
#[tokio::test]
async fn get_pins_finds_parts_by_reference_or_entity_id() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    let placed = place_block(&bridge, sheet, "U1", 60.0, 60.0, ONE_PIN_BLOCK).await;
    let by_ref = call(&bridge, "get_pins", json!({"sheet_id": sheet, "reference": "U1"})).await;
    let parts = by_ref["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0]["pins"][0]["number"], "1");
    assert!(parts[0]["bbox"]["min"]["x"].is_number());
    let by_id = call(
        &bridge,
        "get_pins",
        json!({"sheet_id": sheet, "entity_id": placed["entity_id"]}),
    )
    .await;
    assert_eq!(by_id["parts"][0]["reference"], "U1");
    let none = call_err(&bridge, "get_pins", json!({"sheet_id": sheet})).await;
    assert!(none.contains("reference"), "{none}");
}

/// The pin tools tell the agent to wire by pin name instead of computing pin coordinates.
/// ピン操作ツールの説明には、ピン座標を計算せずピン名で配線するよう書かれている。
#[test]
fn the_pin_tools_tell_the_agent_not_to_compute_coordinates() {
    let tools = tool_descriptions();
    let get = |n: &str| tools.iter().find(|(t, _)| t == n).unwrap_or_else(|| panic!("{n} が無い")).1.clone();
    assert!(get("get_pins").contains("用紙座標"));
    assert!(get("place_symbol").contains("pins"));
}
```

- [ ] **Step 6: Run to verify it fails**

Run: `cd src-tauri && cargo test -p madake-mcp --test drawing_tools`
Expected: FAIL (`get_pins` unknown tool; `pins` missing).

- [ ] **Step 7: Implement the MCP side**

In `src-tauri/crates/madake-mcp/src/lib.rs`, add after `DrawWireParams`:

```rust
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetPinsParams {
    /// 対象シートID。省略時は先頭シート。
    pub sheet_id: Option<Uuid>,
    /// 参照記号 (例: "U1")。同じ参照記号の部品が複数あれば全部返す。
    pub reference: Option<String>,
    /// 部品のentity id。referenceの代わりに使う。
    pub entity_id: Option<Uuid>,
}
```

Change the end of `place_symbol` (the `json_ok` after `execute`) to:

```rust
        let pins = {
            let engine = self.doc.engine.lock().unwrap();
            engine
                .project()
                .sheet(sheet_id)
                .and_then(|sheet| madake_core::pin_wiring::sheet_part_pins(sheet, id))
                .map(|p| p.pins)
                .unwrap_or_default()
        };
        json_ok(&serde_json::json!({ "entity_id": id, "revision": patch.revision, "pins": pins }))
```

Change the `place_symbol` description to:

```rust
        description = "シンボルをシートに配置する。作成されたエンティティidと、置いた部品のピン(pins: 番号・名前・用紙座標x/y・向きdir)を返す。配線はこの座標を使うかconnect_pinsでピン名を指定する。多ピンの機器は symbol_id=\"block\" + attrs.block_pins で名前付きピンのブロックとして作る"
```

Add after `draw_wire`:

```rust
    #[tool(
        description = "配置済み部品のピンを返す。reference(参照記号)かentity_idで指定する。部品ごとにentity_id・参照記号・symbol_id・外形bbox(ブロックは5mm丸めの後)と、各ピンの番号・名前・用紙座標x/y・向きdir(回転と反転の後。電線が出ていく向き)を返す。ピン座標は自分で計算せずこのツールで読む"
    )]
    fn get_pins(&self, Parameters(p): Parameters<GetPinsParams>) -> Result<String, ErrorData> {
        let key = match (p.reference, p.entity_id) {
            (Some(r), _) => r,
            (None, Some(id)) => id.to_string(),
            (None, None) => {
                return Err(ErrorData::invalid_params(
                    "reference か entity_id のどちらかを指定する",
                    None,
                ))
            }
        };
        let sheet_id = self.resolve_sheet(p.sheet_id)?;
        let engine = self.doc.engine.lock().unwrap();
        let sheet = engine
            .project()
            .sheet(sheet_id)
            .ok_or_else(|| ErrorData::invalid_params("sheet not found", None))?;
        let parts = madake_core::pin_wiring::find_parts(sheet, &sheet_symbol_defs(sheet), &key);
        json_ok(&serde_json::json!({ "parts": parts }))
    }
```

- [ ] **Step 8: Run tests**

Run: `cd src-tauri && cargo test -p madake-mcp --test drawing_tools && cargo test -p madake-mcp --test tool_bridge --test tools`
Expected: all pass (the bridge test that compares tool lists still passes, because both sides come from the same router).

- [ ] **Step 9: Commit**

```bash
git add src-tauri/crates/madake-core/src/pin_wiring.rs src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-mcp/tests/drawing_tools.rs
git commit -m "feat(mcp): get_pins で配置済み部品のピン座標と向きを返し、place_symbol もピンを返す"
```

---

### Task 2: Pin specifier resolution and routing (core)

**Files:**
- Modify: `src-tauri/crates/madake-core/src/pin_wiring.rs`
- Test: same file, `mod tests`

**Interfaces:**
- Consumes: `PinInfo`, `find_parts`, `sheet_with` (test helper) from Task 1.
- Produces:
  - `pub const ROUTE_GRID: f64 = 2.5;`
  - `pub struct ResolvedPin { pub entity_id: EntityId, pub reference: String, pub pin: PinInfo }` (Debug, Clone, PartialEq)
  - `pub fn resolve_pin(sheet: &Sheet, defs: &[SymbolDef], spec: &str) -> Result<ResolvedPin, String>`
  - `pub fn route(from: Point, from_dir: PinDir, to: Point, to_dir: PinDir, via: &[Point]) -> Vec<Point>`

- [ ] **Step 1: Write the failing tests**

Append inside `mod tests`:

```rust
    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    /// A pin specifier finds the pin by number first and by name second, splitting at the last dot.
    /// ピン指定は最後の「.」で分け、ピン番号で探してから名前で探す。
    #[test]
    fn a_pin_is_resolved_by_number_then_by_name() {
        let b = block("CTRL1", 100.0, 50.0);
        let dotted = block("X1.A", 40.0, 50.0);
        let sheet = sheet_with(vec![b.clone(), dotted]);
        let defs = sheet_symbol_defs(&sheet);
        assert_eq!(resolve_pin(&sheet, &defs, "CTRL1.IN1").unwrap().pin.number, "IN1");
        let by_name = resolve_pin(&sheet, &defs, "CTRL1.出力1").unwrap();
        assert_eq!(by_name.pin.number, "OUT1");
        assert_eq!(by_name.entity_id, b.id);
        assert_eq!(resolve_pin(&sheet, &defs, "X1.A.IN1").unwrap().reference, "X1.A");
        let by_id = resolve_pin(&sheet, &defs, &format!("{}.IN1", b.id)).unwrap();
        assert_eq!(by_id.entity_id, b.id);
    }

    /// A wrong specifier is refused with what the agent needs to fix it: the format, similar references, the part's pins, or the entity ids to choose from.
    /// 誤ったピン指定は、直すのに要るもの(書き方・似た参照記号・部品のピン・選ぶentity id)を添えて断られる。
    #[test]
    fn a_wrong_specifier_is_refused_with_candidates() {
        let a = inst("connector_socket", 10.0, 10.0, 0, false, "J2");
        let b = inst("connector_socket", 10.0, 30.0, 0, false, "J2");
        let sheet = sheet_with(vec![block("CTRL1", 100.0, 50.0), a.clone(), b.clone()]);
        let defs = sheet_symbol_defs(&sheet);
        let no_dot = resolve_pin(&sheet, &defs, "CTRL1").unwrap_err();
        assert!(no_dot.contains("参照記号.ピン"), "{no_dot}");
        let missing = resolve_pin(&sheet, &defs, "CTRL2.IN1").unwrap_err();
        assert!(missing.contains("CTRL1"), "{missing}");
        let no_pin = resolve_pin(&sheet, &defs, "CTRL1.IN9").unwrap_err();
        assert!(no_pin.contains("IN1") && no_pin.contains("入力1"), "{no_pin}");
        let ambiguous = resolve_pin(&sheet, &defs, "J2.1").unwrap_err();
        assert!(ambiguous.contains(&a.id.to_string()) && ambiguous.contains(&b.id.to_string()), "{ambiguous}");
    }

    /// Two left/right pins are joined with a Z whose vertical leg sits midway on the 2.5 mm grid.
    /// 左右向きのピンどうしは Z 字で結ばれ、縦の区間は2本の中央の2.5mmグリッド上に来る。
    #[test]
    fn horizontal_pins_get_a_z_route() {
        let pts = route(p(10.0, 10.0), PinDir::Right, p(41.0, 30.0), PinDir::Left, &[]);
        assert_eq!(pts, vec![p(10.0, 10.0), p(25.0, 10.0), p(25.0, 30.0), p(41.0, 30.0)]);
    }

    /// Two up/down pins are joined with a Z whose horizontal leg sits midway.
    /// 上下向きのピンどうしは Z 字で結ばれ、横の区間は中央に来る。
    #[test]
    fn vertical_pins_get_a_z_route() {
        let pts = route(p(10.0, 10.0), PinDir::Down, p(30.0, 40.0), PinDir::Up, &[]);
        assert_eq!(pts, vec![p(10.0, 10.0), p(10.0, 25.0), p(30.0, 25.0), p(30.0, 40.0)]);
    }

    /// A left/right pin and an up/down pin are joined with an L, each end leaving in its own direction.
    /// 左右向きと上下向きのピンは L 字で結ばれ、各端は自分の向きへ出る。
    #[test]
    fn mixed_pins_get_an_l_route() {
        let pts = route(p(10.0, 10.0), PinDir::Right, p(30.0, 40.0), PinDir::Up, &[]);
        assert_eq!(pts, vec![p(10.0, 10.0), p(30.0, 10.0), p(30.0, 40.0)]);
        let back = route(p(30.0, 40.0), PinDir::Up, p(10.0, 10.0), PinDir::Right, &[]);
        assert_eq!(back, vec![p(30.0, 40.0), p(30.0, 10.0), p(10.0, 10.0)]);
    }

    /// Pins that are already aligned get one straight segment, whatever their directions.
    /// すでに一直線に並んだピンは、向きにかかわらず直線1本で結ばれる。
    #[test]
    fn aligned_pins_get_a_straight_wire() {
        assert_eq!(
            route(p(10.0, 10.0), PinDir::Right, p(40.0, 10.0), PinDir::Up, &[]),
            vec![p(10.0, 10.0), p(40.0, 10.0)]
        );
        assert_eq!(
            route(p(10.0, 10.0), PinDir::Down, p(10.0, 40.0), PinDir::Up, &[]),
            vec![p(10.0, 10.0), p(10.0, 40.0)]
        );
    }

    /// Pins off the grid keep their exact end points while the bend snaps to the grid.
    /// グリッドから外れたピンでも端点はピンの位置のままで、曲がる位置だけグリッドにそろう。
    #[test]
    fn off_grid_pins_keep_their_ends() {
        let pts = route(p(10.3, 10.0), PinDir::Right, p(41.1, 30.0), PinDir::Left, &[]);
        assert_eq!(pts.first(), Some(&p(10.3, 10.0)));
        assert_eq!(pts.last(), Some(&p(41.1, 30.0)));
        assert_eq!(pts[1].x, 25.0);
    }

    /// With via points the wire passes each one in order, adding a horizontal-first corner where two points are not aligned.
    /// 経由点があると順に通り、そろっていない区間には水平を先にした角を1つ足す。
    #[test]
    fn via_points_are_passed_in_order() {
        let pts = route(p(10.0, 10.0), PinDir::Right, p(50.0, 50.0), PinDir::Left, &[p(20.0, 30.0)]);
        assert_eq!(
            pts,
            vec![p(10.0, 10.0), p(20.0, 10.0), p(20.0, 30.0), p(50.0, 30.0), p(50.0, 50.0)]
        );
        for w in pts.windows(2) {
            assert!(w[0].x == w[1].x || w[0].y == w[1].y, "斜めの線分: {w:?}");
        }
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd src-tauri && cargo test -p madake-core pin_wiring`
Expected: compile errors (`resolve_pin`, `route` not found).

- [ ] **Step 3: Implement**

Add to `pin_wiring.rs` (above `#[cfg(test)]`):

```rust
/// 配線の折れ点をそろえるグリッド (mm)。
pub const ROUTE_GRID: f64 = 2.5;

const EPS: f64 = 1e-6;

/// エラーに並べる候補の上限。
const MAX_CANDIDATES: usize = 20;

/// ピン指定 ("参照記号.ピン") を解決した結果。
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedPin {
    pub entity_id: EntityId,
    pub reference: String,
    pub pin: PinInfo,
}

/// ピン指定を解決する。最後の「.」で参照記号 (またはentity id) とピンに分け、
/// ピンは番号で探してから名前で探す。エラーの文には直すための候補を添える。
pub fn resolve_pin(sheet: &Sheet, defs: &[SymbolDef], spec: &str) -> Result<ResolvedPin, String> {
    let Some((key, pin)) = spec.rsplit_once('.') else {
        return Err(format!(
            "ピン指定「{spec}」は「参照記号.ピン」の形で書く (例: U1.SDA、J1.2)"
        ));
    };
    let parts = find_parts(sheet, defs, key);
    if parts.is_empty() {
        return Err(format!(
            "参照記号「{key}」の部品がシートにありません。シート上の参照記号: {}",
            similar_references(sheet, key).join(", ")
        ));
    }
    let hits: Vec<(&PartPins, &PinInfo)> = parts
        .iter()
        .filter_map(|part| {
            part.pins
                .iter()
                .find(|x| x.number == pin)
                .or_else(|| part.pins.iter().find(|x| x.name == pin))
                .map(|x| (part, x))
        })
        .collect();
    match hits.as_slice() {
        [] => {
            let listed: Vec<String> = parts
                .iter()
                .flat_map(|part| part.pins.iter())
                .map(|x| if x.name.is_empty() { x.number.clone() } else { format!("{}({})", x.number, x.name) })
                .collect();
            Err(format!("{key} にピン「{pin}」がありません。ピン: {}", listed.join(", ")))
        }
        [(part, info)] => Ok(ResolvedPin {
            entity_id: part.entity_id,
            reference: part.reference.clone(),
            pin: (*info).clone(),
        }),
        many => Err(format!(
            "参照記号「{key}」の部品が複数あり、どれにもピン「{pin}」があります。entity idで指定し直す: {}",
            many.iter()
                .map(|(part, _)| format!("{}.{pin}", part.entity_id))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// 見つからない参照記号に似た、シート上の参照記号 (先頭の英字が同じもの。無ければ全部)。
fn similar_references(sheet: &Sheet, key: &str) -> Vec<String> {
    let mut refs: Vec<String> = sheet
        .entities
        .values()
        .filter_map(|e| match e {
            Entity::Symbol(s) if !s.reference.is_empty() => Some(s.reference.clone()),
            _ => None,
        })
        .collect();
    refs.sort();
    refs.dedup();
    let prefix: String = key.chars().take_while(|c| !c.is_ascii_digit()).collect();
    let similar: Vec<String> = refs
        .iter()
        .filter(|r| !prefix.is_empty() && r.starts_with(&prefix))
        .cloned()
        .collect();
    let mut out = if similar.is_empty() { refs } else { similar };
    out.truncate(MAX_CANDIDATES);
    out
}

fn snap(v: f64) -> f64 {
    (v / ROUTE_GRID).round() * ROUTE_GRID
}

fn is_horizontal(dir: PinDir) -> bool {
    matches!(dir, PinDir::Left | PinDir::Right)
}

fn aligned(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < EPS || (a.y - b.y).abs() < EPS
}

/// ピンどうしを結ぶ直角配線の頂点列。経由点が無ければ両端の向きで Z 字・L 字・直線を選び、
/// 経由点があれば順に通る (そろっていない区間は水平を先にした角を1つ足す)。
/// 端点はピンの位置のまま、折れ点だけ [`ROUTE_GRID`] にそろえる。部品は避けない。
pub fn route(from: Point, from_dir: PinDir, to: Point, to_dir: PinDir, via: &[Point]) -> Vec<Point> {
    let mut pts = vec![from];
    if via.is_empty() {
        if !aligned(from, to) {
            match (is_horizontal(from_dir), is_horizontal(to_dir)) {
                (true, true) => {
                    let xm = snap((from.x + to.x) / 2.0);
                    pts.push(Point::new(xm, from.y));
                    pts.push(Point::new(xm, to.y));
                }
                (false, false) => {
                    let ym = snap((from.y + to.y) / 2.0);
                    pts.push(Point::new(from.x, ym));
                    pts.push(Point::new(to.x, ym));
                }
                (true, false) => pts.push(Point::new(to.x, from.y)),
                (false, true) => pts.push(Point::new(from.x, to.y)),
            }
        }
    } else {
        let mut prev = from;
        for &next in via.iter().chain(std::iter::once(&to)) {
            if !aligned(prev, next) {
                pts.push(Point::new(next.x, prev.y));
            }
            if next != to {
                pts.push(next);
            }
            prev = next;
        }
    }
    pts.push(to);
    simplify(pts)
}

/// 重なった頂点と、一直線上の途中の頂点を落とす。
fn simplify(pts: Vec<Point>) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(pts.len());
    for p in pts {
        if out.last().is_some_and(|q| q.distance_to(&p) < EPS) {
            continue;
        }
        if out.len() >= 2 {
            let (a, b) = (out[out.len() - 2], out[out.len() - 1]);
            let same_x = (a.x - b.x).abs() < EPS && (b.x - p.x).abs() < EPS;
            let same_y = (a.y - b.y).abs() < EPS && (b.y - p.y).abs() < EPS;
            if same_x || same_y {
                out.pop();
            }
        }
        out.push(p);
    }
    out
}
```

Note on the `via` loop: `next != to` compares `Point` (PartialEq). A `via` point equal to `to` is harmless because `simplify` drops the duplicate.

- [ ] **Step 4: Run tests**

Run: `cd src-tauri && cargo test -p madake-core pin_wiring`
Expected: 10 passed.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/crates/madake-core/src/pin_wiring.rs
git commit -m "feat(core): ピン指定の解決と直角配線の経路計算を足す"
```

---

### Task 3: `connect_pins` tool

**Files:**
- Modify: `src-tauri/crates/madake-mcp/src/lib.rs` (params after `GetPinsParams`; tool after `get_pins`)
- Test: `src-tauri/crates/madake-mcp/tests/drawing_tools.rs`

**Interfaces:**
- Consumes: `resolve_pin`, `route` (Task 2); `place_block`, `call`, `call_err`, `ONE_PIN_BLOCK`, `TWO_PIN_BLOCK` (Task 1 test helpers).
- Produces: tool `connect_pins` with `ConnectPinsParams { sheet_id, from: String, to: String, via: Option<Vec<Point>>, color, sq, length_m, part_no }`, returns `{"entity_id", "points", "revision"}`.

- [ ] **Step 1: Write the failing tests**

Append to `drawing_tools.rs`:

```rust
fn unconnected_pins(diagnostics: &Value) -> usize {
    diagnostics
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "erc.unconnected_pin")
        .count()
}

/// connect_pins wires two pins by name with right-angle segments, and verification then finds no unconnected pin.
/// connect_pins はピン名を指定した2本のピンを直角の線分で結び、その後の検証で未接続ピンが出ない。
#[tokio::test]
async fn connect_pins_wires_two_pins_by_name() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    place_block(&bridge, sheet, "U1", 60.0, 60.0, r#"{"right":[{"no":"1","name":"A"}]}"#).await;
    place_block(&bridge, sheet, "U2", 120.0, 90.0, ONE_PIN_BLOCK).await;
    let before = unconnected_pins(&call(&bridge, "run_verification", json!({"sheet_id": sheet})).await);
    assert_eq!(before, 2);
    let wired = call(
        &bridge,
        "connect_pins",
        json!({"sheet_id": sheet, "from": "U1.A", "to": "U2.1", "color": "red", "sq": 0.5}),
    )
    .await;
    let pts = wired["points"].as_array().unwrap();
    assert_eq!(pts.len(), 4, "Z 字になっていない: {pts:?}");
    for w in pts.windows(2) {
        assert!(w[0]["x"] == w[1]["x"] || w[0]["y"] == w[1]["y"], "斜めの線分: {w:?}");
    }
    let after = unconnected_pins(&call(&bridge, "run_verification", json!({"sheet_id": sheet})).await);
    assert_eq!(after, 0);
    let engine = doc.engine.lock().unwrap();
    let wire = engine.project().sheets[0]
        .entities
        .values()
        .find_map(|e| match e {
            madake_core::Entity::Wire(w) => Some(w.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!((wire.color.as_str(), wire.sq), ("red", 0.5));
}

/// connect_pins passes the given via points in order.
/// connect_pins は渡した経由点を順に通る。
#[tokio::test]
async fn connect_pins_passes_via_points() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    place_block(&bridge, sheet, "U1", 60.0, 60.0, r#"{"right":[{"no":"1","name":"A"}]}"#).await;
    place_block(&bridge, sheet, "U2", 120.0, 90.0, ONE_PIN_BLOCK).await;
    let wired = call(
        &bridge,
        "connect_pins",
        json!({"sheet_id": sheet, "from": "U1.1", "to": "U2.1", "via": [{"x": 90.0, "y": 40.0}]}),
    )
    .await;
    assert!(wired["points"].as_array().unwrap().iter().any(|p| p["x"] == 90.0 && p["y"] == 40.0));
}

/// connect_pins refuses an unknown part, an unknown pin, an ambiguous reference and a pin wired to itself, and creates nothing.
/// connect_pins は不明な部品・不明なピン・どれか決まらない参照記号・同じピンどうしを断り、何も作らない。
#[tokio::test]
async fn connect_pins_refuses_bad_specifiers_and_creates_nothing() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    place_block(&bridge, sheet, "U1", 60.0, 60.0, TWO_PIN_BLOCK).await;
    place_block(&bridge, sheet, "J1", 20.0, 20.0, ONE_PIN_BLOCK).await;
    place_block(&bridge, sheet, "J1", 20.0, 60.0, ONE_PIN_BLOCK).await;
    let count = |doc: &SharedDoc| doc.engine.lock().unwrap().project().sheets[0].entities.len();
    let before = count(&doc);
    let e = call_err(&bridge, "connect_pins", json!({"sheet_id": sheet, "from": "U9.1", "to": "U1.IN1"})).await;
    assert!(e.contains("U1"), "{e}");
    let e = call_err(&bridge, "connect_pins", json!({"sheet_id": sheet, "from": "U1.IN9", "to": "U1.IN1"})).await;
    assert!(e.contains("OUT1"), "{e}");
    let e = call_err(&bridge, "connect_pins", json!({"sheet_id": sheet, "from": "J1.1", "to": "U1.IN1"})).await;
    assert!(e.contains("entity id"), "{e}");
    let e = call_err(&bridge, "connect_pins", json!({"sheet_id": sheet, "from": "U1.IN1", "to": "U1.入力1"})).await;
    assert!(e.contains("同じピン"), "{e}");
    assert_eq!(count(&doc), before);
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd src-tauri && cargo test -p madake-mcp --test drawing_tools connect_pins`
Expected: FAIL (unknown tool `connect_pins`).

- [ ] **Step 3: Implement**

Params, after `GetPinsParams`:

```rust
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ConnectPinsParams {
    /// 配線先シートID。省略時は先頭シート。
    pub sheet_id: Option<Uuid>,
    /// 始点のピン指定 "参照記号.ピン" (例: "U1.SDA")。ピンは番号でも名前でもよい。参照記号の代わりにentity idも書ける。
    pub from: String,
    /// 終点のピン指定 (fromと同じ書き方)。
    pub to: String,
    /// 経由点(mm)。順に通る。省略時は両端のピンの向きでZ字・L字・直線を選ぶ。
    pub via: Option<Vec<Point>>,
    /// 線色(例: "red", "black", "light_blue")。
    pub color: Option<String>,
    /// 線径 sq (mm2)。例: 0.3, 0.75, 3.5。
    pub sq: Option<f64>,
    /// 電線長(m)。
    pub length_m: Option<f64>,
    /// 電線品番。
    pub part_no: Option<String>,
}
```

Tool, after `get_pins`:

```rust
    #[tool(
        description = "2本のピンをピン名で指定して配線する(座標は計算しなくてよい)。from/toは\"参照記号.ピン\"(例: \"U1.SDA\"、\"J1.2\"。ピンは番号でも名前でも、参照記号の代わりにentity idでもよい)。経由点viaが無ければ両端の向きでZ字・L字・直線の直角配線を引き、折れ点は2.5mmグリッドにそろえる。部品は避けないので、引いた後にget_tidy_metricsで重なりを確かめ、必要ならviaを足して引き直す。color/sq/length_m/part_noはdraw_wireと同じ。作成した配線idと頂点列pointsを返す"
    )]
    fn connect_pins(&self, Parameters(p): Parameters<ConnectPinsParams>) -> Result<String, ErrorData> {
        let sheet_id = self.resolve_sheet(p.sheet_id)?;
        let points = {
            let engine = self.doc.engine.lock().unwrap();
            let sheet = engine
                .project()
                .sheet(sheet_id)
                .ok_or_else(|| ErrorData::invalid_params("sheet not found", None))?;
            let defs = sheet_symbol_defs(sheet);
            let invalid = |e: String| ErrorData::invalid_params(e, None);
            let from = madake_core::pin_wiring::resolve_pin(sheet, &defs, &p.from).map_err(invalid)?;
            let to = madake_core::pin_wiring::resolve_pin(sheet, &defs, &p.to).map_err(invalid)?;
            if from.entity_id == to.entity_id && from.pin.number == to.pin.number {
                return Err(ErrorData::invalid_params(
                    format!("from と to が同じピンです ({})", p.from),
                    None,
                ));
            }
            madake_core::pin_wiring::route(
                Point::new(from.pin.x, from.pin.y),
                from.pin.dir,
                Point::new(to.pin.x, to.pin.y),
                to.pin.dir,
                p.via.as_deref().unwrap_or_default(),
            )
        };
        let id = Uuid::new_v4();
        let entity = Entity::Wire(Wire {
            id,
            points: points.clone(),
            color: p.color.unwrap_or_else(|| "black".into()),
            sq: p.sq.unwrap_or(0.3),
            length_m: p.length_m,
            length_source: Default::default(),
            part_no: p.part_no,
            net: None,
        });
        let patch = self
            .doc
            .execute(Command::AddEntity { sheet_id, entity })
            .map_err(internal)?;
        json_ok(&serde_json::json!({ "entity_id": id, "points": points, "revision": patch.revision }))
    }
```

The engine lock is released before `execute` (the block ends), so `execute` does not deadlock.

- [ ] **Step 4: Run tests**

Run: `cd src-tauri && cargo test -p madake-mcp --test drawing_tools`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-mcp/tests/drawing_tools.rs
git commit -m "feat(mcp): connect_pins でピン名どうしを直角配線でつなぐ"
```

---

### Task 4: Mate pair layout and `place_mate`

**Files:**
- Modify: `src-tauri/crates/madake-core/src/pin_wiring.rs`
- Modify: `src-tauri/crates/madake-mcp/src/lib.rs`
- Test: `pin_wiring.rs` `mod tests`, `drawing_tools.rs`

**Interfaces:**
- Consumes: `part_pins` (Task 1).
- Produces:
  - `pub enum MateOrientation { Horizontal, Vertical }` and `pub enum MaleSide { First, Second }` (Deserialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq; `#[serde(rename_all = "snake_case")]`)
  - `pub struct MatePair { pub first: SymbolInstance, pub second: SymbolInstance, pub first_point: Point, pub second_point: Point }`
  - `pub fn mate_pair(at: Point, orientation: MateOrientation, male_side: MaleSide, reference: &str, value: &str) -> MatePair`
  - MCP tool `place_mate` with `PlaceMateParams { sheet_id, x, y, orientation: Option<MateOrientation>, male_side: Option<MaleSide>, reference: Option<String>, value: Option<String> }` (defaults Horizontal, First); returns `{"first_entity_id", "second_entity_id", "first": {x,y}, "second": {x,y}, "revision"}`.

- [ ] **Step 1: Write the failing core test**

Append inside `mod tests` in `pin_wiring.rs`:

```rust
    /// A mate pair puts the plug and the socket 3.5 mm apart facing each other, in both orientations and with the male part on either side.
    /// かみ合わせの組は、どちらの向きでもオスがどちら側でも、プラグとソケットを3.5mm離して向かい合わせに置く。
    #[test]
    fn a_mate_pair_faces_each_other_in_every_layout() {
        let at = p(100.0, 50.0);
        let h1 = mate_pair(at, MateOrientation::Horizontal, MaleSide::First, "J1", "XH");
        assert_eq!((h1.first.symbol_id.as_str(), h1.second.symbol_id.as_str()), ("connector_plug", "connector_socket"));
        assert_eq!((h1.first_point, h1.second_point), (p(92.5, 50.0), p(111.0, 50.0)));
        assert_eq!((h1.second.reference.as_str(), h1.second.value.as_str()), ("J1", "XH"));
        assert!(h1.first.reference.is_empty());
        let h2 = mate_pair(at, MateOrientation::Horizontal, MaleSide::Second, "J1", "");
        assert_eq!(h2.first.symbol_id, "connector_socket");
        assert_eq!((h2.first_point, h2.second_point), (p(92.5, 50.0), p(111.0, 50.0)));
        assert_eq!(h2.first.reference, "J1");
        let v1 = mate_pair(at, MateOrientation::Vertical, MaleSide::First, "J1", "");
        assert_eq!(v1.first.symbol_id, "connector_plug");
        assert_eq!((v1.first_point, v1.second_point), (p(100.0, 42.5), p(100.0, 61.0)));
        let v2 = mate_pair(at, MateOrientation::Vertical, MaleSide::Second, "J1", "");
        assert_eq!(v2.first.symbol_id, "connector_socket");
        assert_eq!((v2.first_point, v2.second_point), (p(100.0, 42.5), p(100.0, 61.0)));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd src-tauri && cargo test -p madake-core pin_wiring`
Expected: compile error (`mate_pair` not found).

- [ ] **Step 3: Implement core**

Add to `pin_wiring.rs` (and add `use crate::symbol::resolve_symbol;` plus `use uuid::Uuid;` to the imports at the top of the file):

```rust
/// かみ合わせの組の並べ方。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MateOrientation {
    /// 左右に並べる (first=左)。
    Horizontal,
    /// 上下に並べる (first=上)。
    Vertical,
}

/// どちら側をオス (プラグ) にするか。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MaleSide {
    /// 左/上がオス。
    First,
    /// 右/下がオス。
    Second,
}

/// プラグとソケットの中心の間隔 (mm)。
pub const MATE_GAP: f64 = 3.5;

/// 置く前のかみ合わせの組と、外側の2つの接続点。
#[derive(Debug, Clone, PartialEq)]
pub struct MatePair {
    pub first: SymbolInstance,
    pub second: SymbolInstance,
    pub first_point: Point,
    pub second_point: Point,
}

/// プラグとソケットを [`MATE_GAP`] 離して向かい合わせに並べる。参照記号と値はソケットに付ける。
pub fn mate_pair(
    at: Point,
    orientation: MateOrientation,
    male_side: MaleSide,
    reference: &str,
    value: &str,
) -> MatePair {
    let second_at = match orientation {
        MateOrientation::Horizontal => at.translated(MATE_GAP, 0.0),
        MateOrientation::Vertical => at.translated(0.0, MATE_GAP),
    };
    // (first の symbol_id, 回転, 反転) と second の同じもの
    let (first_spec, second_spec) = match (orientation, male_side) {
        (MateOrientation::Horizontal, MaleSide::First) => {
            (("connector_plug", 0, false), ("connector_socket", 0, false))
        }
        (MateOrientation::Horizontal, MaleSide::Second) => {
            (("connector_socket", 0, true), ("connector_plug", 0, true))
        }
        (MateOrientation::Vertical, MaleSide::First) => {
            (("connector_plug", 90, false), ("connector_socket", 90, false))
        }
        (MateOrientation::Vertical, MaleSide::Second) => {
            (("connector_socket", 270, false), ("connector_plug", 270, false))
        }
    };
    let make = |(symbol_id, rotation, mirror): (&str, u16, bool), at: Point| {
        let is_socket = symbol_id == "connector_socket";
        SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: symbol_id.into(),
            at,
            rotation,
            mirror,
            reference: if is_socket { reference.into() } else { String::new() },
            value: if is_socket { value.into() } else { String::new() },
            attrs: Default::default(),
        }
    };
    let first = make(first_spec, at);
    let second = make(second_spec, second_at);
    let point = |inst: &SymbolInstance| {
        let def = resolve_symbol(&inst.symbol_id).expect("同梱のプラグ/ソケット");
        let pin = &part_pins(inst, &def).pins[0];
        Point::new(pin.x, pin.y)
    };
    MatePair {
        first_point: point(&first),
        second_point: point(&second),
        first,
        second,
    }
}
```

- [ ] **Step 4: Run core tests**

Run: `cd src-tauri && cargo test -p madake-core pin_wiring`
Expected: 11 passed.

- [ ] **Step 5: Write the failing MCP test**

Append to `drawing_tools.rs`:

```rust
/// place_mate puts a plug/socket pair in one step, returns both outer connection points, and one undo removes the pair.
/// place_mate はプラグとソケットの組を1回で置き、外側の2つの接続点を返し、1回の取り消しで組ごと消える。
#[tokio::test]
async fn place_mate_puts_a_pair_that_one_undo_removes() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    let placed = call(
        &bridge,
        "place_mate",
        json!({"sheet_id": sheet, "x": 100.0, "y": 50.0, "orientation": "vertical",
               "male_side": "first", "reference": "J5", "value": "XH-3P"}),
    )
    .await;
    assert_eq!((placed["first"]["x"].as_f64(), placed["first"]["y"].as_f64()), (Some(100.0), Some(42.5)));
    assert_eq!((placed["second"]["x"].as_f64(), placed["second"]["y"].as_f64()), (Some(100.0), Some(61.0)));
    assert_eq!(doc.engine.lock().unwrap().project().sheets[0].entities.len(), 2);
    let pins = call(&bridge, "get_pins", json!({"sheet_id": sheet, "reference": "J5"})).await;
    assert_eq!(pins["parts"][0]["symbol_id"], "connector_socket");
    doc.undo().expect("元に戻せる");
    assert_eq!(doc.engine.lock().unwrap().project().sheets[0].entities.len(), 0);
}
```

- [ ] **Step 6: Run to verify it fails**

Run: `cd src-tauri && cargo test -p madake-mcp --test drawing_tools place_mate`
Expected: FAIL (unknown tool `place_mate`).

- [ ] **Step 7: Implement the MCP side**

Params:

```rust
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct PlaceMateParams {
    /// 配置先シートID。省略時は先頭シート。
    pub sheet_id: Option<Uuid>,
    /// 左(縦向きなら上)の部品の位置(mm)。
    pub x: f64,
    pub y: f64,
    /// horizontal(左右に並べる、既定) / vertical(上下に並べる)。
    pub orientation: Option<madake_core::pin_wiring::MateOrientation>,
    /// first(左/上がオス、既定) / second(右/下がオス)。
    pub male_side: Option<madake_core::pin_wiring::MaleSide>,
    /// 参照記号(ソケット側に付く)。
    pub reference: Option<String>,
    /// 型番・値(ソケット側に付く)。
    pub value: Option<String>,
}
```

Tool, after `connect_pins`:

```rust
    #[tool(
        description = "差し込み1極のかみ合わせ(connector_plug=オスとconnector_socket=メス)を3.5mm離して向かい合わせに1回で置く(undo一発で組ごと消える)。orientation=horizontal(左右、既定)/vertical(上下)、male_side=first(左/上がオス、既定)/second。参照記号と値はソケット側に付く。外側の2つの接続点first/second(用紙座標)と2つのentity idを返す。多極は極ごとに置き、配線はconnect_pins(\"<entity_id>.1\")か返した座標で行う"
    )]
    fn place_mate(&self, Parameters(p): Parameters<PlaceMateParams>) -> Result<String, ErrorData> {
        use madake_core::pin_wiring::{mate_pair, MaleSide, MateOrientation};
        let sheet_id = self.resolve_sheet(p.sheet_id)?;
        let pair = mate_pair(
            Point::new(p.x, p.y),
            p.orientation.unwrap_or(MateOrientation::Horizontal),
            p.male_side.unwrap_or(MaleSide::First),
            p.reference.as_deref().unwrap_or_default(),
            p.value.as_deref().unwrap_or_default(),
        );
        let (first_id, second_id) = (pair.first.id, pair.second.id);
        let commands = vec![
            Command::AddEntity { sheet_id, entity: Entity::Symbol(pair.first) },
            Command::AddEntity { sheet_id, entity: Entity::Symbol(pair.second) },
        ];
        let patch = self
            .doc
            .execute_batch_as(commands, self.doc.mcp_origin())
            .map_err(internal)?;
        json_ok(&serde_json::json!({
            "first_entity_id": first_id,
            "second_entity_id": second_id,
            "first": pair.first_point,
            "second": pair.second_point,
            "revision": patch.revision,
        }))
    }
```

- [ ] **Step 8: Run tests**

Run: `cd src-tauri && cargo test -p madake-mcp --test drawing_tools && cargo test -p madake-core pin_wiring`
Expected: all pass.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/crates/madake-core/src/pin_wiring.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-mcp/tests/drawing_tools.rs
git commit -m "feat(mcp): place_mate でプラグとソケットの組を1回で置く"
```

---

### Task 5: System prompt rule, docs, generated spec

**Files:**
- Modify: `src-tauri/crates/madake-agent/src/knowledge.rs` (`WORKFLOW_RULES`, after the `add_entity` id bullet, before the blank line that ends the section)
- Modify: `src-tauri/crates/madake-agent/tests/knowledge.rs` (new test)
- Modify: `docs/10-automation-api.md`, `docs/10-automation-api.ja.md` (tool list line 11)
- Regenerate: spec docs via `python3 scripts/gen_spec.py`

**Interfaces:**
- Consumes: tool names `get_pins`, `connect_pins`, `place_mate` (Tasks 1, 3, 4).

- [ ] **Step 1: Write the failing test**

Append to `src-tauri/crates/madake-agent/tests/knowledge.rs`:

```rust
/// The system prompt tells the agent to wire diagrams by pin name with the pin tools instead of computing pin coordinates.
/// システムプロンプトは、配線図はピン操作ツールでピン名を指定して描き、ピン座標を手で計算しないよう指示する。
#[test]
fn system_prompt_tells_the_agent_to_wire_by_pin_name() {
    let prompt = system_prompt(&AppSettings::default(), None);
    for needle in ["get_pins", "connect_pins", "place_mate", "手で計算しない"] {
        assert!(prompt.contains(needle), "「{needle}」が無い: {prompt}");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd src-tauri && cargo test -p madake-agent --test knowledge system_prompt_tells_the_agent_to_wire_by_pin_name`
Expected: FAIL.

- [ ] **Step 3: Add the rule**

In `WORKFLOW_RULES`, after the line `図面内で重複しなければよい` add:

```
- **配線図は部品を`place_symbol`/`place_mate`で置き、ピンの位置は返り値か`get_pins`で読み、\
配線は`connect_pins`でピン名を指定して引く**(例: from=\"U1.SDA\" to=\"J1.2\")。\
ピン座標(ブロックの5mm丸め・回転・反転)を手で計算しない
```

Keep the trailing `\` line-continuation style of the neighbouring bullets.

- [ ] **Step 4: Update docs**

In `docs/10-automation-api.md` line 11, after `` `draw_wire`, `` insert:

```
`get_pins` (pins of a placed part: number, name, paper position, direction), `connect_pins` (wire two pins by name, e.g. `U1.SDA` to `J1.2`, with a right-angle route), `place_mate` (a plug/socket pair in one step),
```

and change `` `place_symbol`, `` to `` `place_symbol` (returns the new part's pins), ``.

In `docs/10-automation-api.ja.md` line 11, after `` `draw_wire`・ `` insert:

```
`get_pins`(配置済み部品のピン: 番号・名前・用紙座標・向き)・`connect_pins`(ピン名どうしを直角配線でつなぐ。例 `U1.SDA` から `J1.2`)・`place_mate`(プラグとソケットの組を1回で置く)・
```

and change `` `place_symbol`・ `` to `` `place_symbol`(置いた部品のピンを返す)・ ``.

- [ ] **Step 5: Run the whole suite and regenerate the spec**

Run:
```bash
cd src-tauri && cargo test -p madake-core -p madake-mcp -p madake-agent 2>&1 | tail -20
cd .. && python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check
cd src-tauri && cargo clippy -p madake-core -p madake-mcp -p madake-agent --all-targets -- -D warnings
```
Expected: all tests pass; `--check` prints "spec docs up to date; all tests have bilingual spec comments"; clippy clean. If clippy flags something in new code, fix it; do not touch unrelated warnings.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/crates/madake-agent docs
git commit -m "docs: ピン操作ツールをシステムプロンプトと自動化APIの説明に書く"
```

---

### Task 6: Live check in the app and PR

**Files:** none in the repo (outside: `~/.claude/skills/drawing-madakecad-wiring/SKILL.md`).

- [ ] **Step 1: Run the dev app from this worktree**

Stop any other dev app that holds port 9310 first (ask the user if it is not ours). Then:
```bash
cd /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/mcp-pin-tools
npm ci
npx tauri dev --config '{"build":{"devUrl":"http://localhost:1430","beforeDevCommand":"npx vite --port 1430 --strictPort"}}'
```
Expected: app window opens; `curl -s http://127.0.0.1:9310/api/v1/project` answers.

- [ ] **Step 2: Have the in-app agent draw one AOGU sheet**

Send the 給電系 sheet part of the earlier prompt (with the whiteboard photo and BOM attachments) through `POST /api/v1/agent/send`. Record the duration and tool calls from `/api/v1/agent/conversations`.
Expected: the agent uses `place_symbol`/`place_mate`/`connect_pins`; no `erc.unconnected_pin` caused by coordinate mistakes; time and tool calls clearly below the earlier run's share for one sheet (about 13 min).

- [ ] **Step 3: Record the result**

Add a "Rerun with pin tools" section to `docs/internal/research/2026-10-22-agent-drawing-comparison.md` and `.ja.md` with time, tool-call counts and verification counts. Commit:
```bash
git add docs/internal/research
git commit -m "docs: ピン操作ツールでのエージェント作図の再測定を記録する"
```

- [ ] **Step 4: Update the skill (outside the repo)**

In `~/.claude/skills/drawing-madakecad-wiring/SKILL.md`, add under the workflow: inside the MadakeCAD app (no shell), draw with `place_symbol` / `place_mate` / `get_pins` / `connect_pins`; the helper script is for external Claude Code only.

- [ ] **Step 5: Push and open the PR**

```bash
git push origin feature/mcp-pin-tools:feature/mcp-pin-tools
gh pr create --base master --head feature/mcp-pin-tools --title "feat(mcp): ピン名で配線するツール (get_pins / connect_pins / place_mate)" --body-file <body>
```
The body explains the problem (agent spent most of 40 min computing coordinates), the tools, tests, and the live result, and ends with:
```
🤖 Generated with [Claude Code](https://claude.com/claude-code)
```
No `Co-Authored-By` trailer anywhere.
