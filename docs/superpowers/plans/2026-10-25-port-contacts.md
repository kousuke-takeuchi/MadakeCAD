# Port Contacts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A block pin can be a **port** that carries several named conductors (`contacts`, e.g. `VBUS, D-, D+, GND`). It is drawn as one pin, and the netlist, ERC, cross references, run cables and the agent see one net per contact.

**Architecture:**
- **Data.** `BlockPin` / `PinDef` / `PinInfo` gain `contacts`.
- **Netlist.** `madake-core/src/netlist.rs` builds today's union-find without cable cores ("clusters") and gives each port cluster one slot per contact plus a residual slot. Cores and dotted labels (`L.c`) then join slots, so `extract_netlist` returns one net per contact. `extract_wire_nets` keeps today's one-net-per-drawn-group view for wire numbering, wire-number drawing and the wire list.
- **ERC.** New ERC lives in `madake-core/src/port.rs`.
- **Run cables.** A run cable can land on one port pin, with `CoreEnd::Pin.contact` saying which contact each core is.
- **File format.** 18. A block with contacts is written with the entity kind `port_block`, and only while a file is being written. IPC, MCP and Link API keep `symbol`.

**Base:** `feature/port-contacts` with `origin/master` merged after PR #16 (cable-polish: `CableSide.labels`, `cable::effective_wire`, `TextKind::CableLabel`, `place_mate` pole / `REF:POLE`). Line numbers below are for that tree.

**Tech Stack:** Rust (madake-core, madake-mcp, madake-agent, madake-cli; serde 1, serde_json 1 without `preserve_order`, schemars 1.2), Vue 3 + TypeScript (Vitest, vue-tsc), Tauri 2.

**Spec:** `docs/superpowers/specs/2026-10-25-port-contacts-design.md` (English) / `.ja.md` (Japanese). Read the spec first: it has the problem, the algorithm (§2.1), the names (§2.3) and the decision log.

## Global Constraints

- Work only in `/Users/ktakeuchi/Documents/MadakeCAD/.worktrees/port-contacts` on branch `feature/port-contacts`. Never touch other worktrees (`.worktrees/aogu-draw` runs a dev app) or the main checkout; never switch branches.
- Before trusting any cargo result, run from the worktree root: `touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs`. The shared `CARGO_TARGET_DIR=/Users/ktakeuchi/Documents/MadakeCAD/src-tauri/target` can link another branch's rlib.
- Every Rust test has exactly two `///` lines directly above `#[test]` / `#[tokio::test]`: line 1 English, line 2 Japanese. Every Vitest test has a `// ja: 日本語文` line directly above `it("English sentence", …)`.
- After adding or renaming tests: `python3 scripts/gen_spec.py`, then `python3 scripts/gen_spec.py --check` (Expected: `spec docs up to date; all tests have bilingual spec comments`). Commit the regenerated `docs/13-specification.md` and `docs/13-specification.ja.md` with the task.
- Commits use Conventional Commits with a Japanese subject. **No `Co-Authored-By` or any other AI attribution trailer.** Do not push.
- Frontend tests need `node_modules`. If `/Users/ktakeuchi/Documents/MadakeCAD/.worktrees/port-contacts/node_modules` is missing, run `ln -s /Users/ktakeuchi/Documents/MadakeCAD/node_modules /Users/ktakeuchi/Documents/MadakeCAD/.worktrees/port-contacts/node_modules` (git-ignored). Known flake: `src/canvas/terminals.test.ts` can time out under load. Re-run it alone before treating it as a failure.
- Contacts: trimmed, non-empty, unique within one pin, **at least 2**, at most `BLOCK_CONTACT_MAX` = 32 per pin. Matching is by exact string, never by parsing.
- Pass-through pins are `connector_plug` / `connector_socket` only (`mate::is_mate_half`), not all `connector_*`.
- A plain pin joins a contact by its **pin name** only, never by number.
- Contact nets are named `<label|wire number|Nxxx>.<contact>`. A port cluster takes one `Nxxx`. A residual net keeps the base name without a suffix.
- Sort key, exactly: `(wire_ids.is_empty(), min wire id, min (cable_id, core), contact rank)`. Rank is 0 for unsplit nets, so drawings without ports keep today's order and names.
- ERC codes, exactly:
  - `erc.port_to_plain_pin` (Warning)
  - `erc.port_contact_unmatched` (Info; only clusters with 2+ port pins)
  - `erc.port_label_unsplit` (Warning, project level)
  - `erc.cable_core_unassigned` (existing Error; extended)
  - `erc.label_conflict` (contact nets compare `Net.contact_labels`)
- `connect_cable` port landing:
  - It applies only when a side resolves to exactly one pin that has contacts. `style` must be `single`.
  - Mapping: all core names are contacts → by name (a subset is fine). No core name is a contact and the counts are equal → by order. Anything else is refused.
- Wire list and BOM stay one row per drawn `Wire`. Run-cable core rows print `REF:PIN/CONTACT`.
- `FORMAT_VERSION` = 18. A file without port blocks is written byte for byte as before, apart from the version.
- Mates still do not conduct (decided 2026-10-25: a separate change).

## Review Focus

1. A block edited after a cable landed on it, so that a core's contact no longer exists, must give `erc.cable_core_unassigned` naming the contact, not a panic or a silent short. Test: Task 6 `a_core_on_a_removed_contact_is_reported`.
2. Contact names typed with spaces or Japanese commas (`" VBUS 、GND"`) must be trimmed and split the same way in Rust and TS, so a plain pin named `GND` still joins. Test: Task 1 `contacts_are_trimmed` and Task 10 `splits contacts on ASCII, Japanese and full-width commas and trims them`.
3. A single-line run between two port pins, with no wire at all, must still give one net per core (core-only nets are split too). Test: Task 3 `a_run_between_two_ports_gives_one_net_per_core`.
4. Two ports with different contact sets (USB 2 hub port and a port with an `ID` contact) must join only the shared contacts and give `erc.port_contact_unmatched` for the others, with no residual net. Test: Task 3 `ports_with_different_contacts_join_only_the_shared_ones` and Task 6 `a_contact_on_only_one_of_two_ports_is_reported_as_info`.
5. A block whose contacts are all removed again must be written back with the kind `symbol`, so the file opens in older builds again. Test: Task 7 `a_block_whose_contacts_were_removed_is_saved_as_symbol_again`.

---

## Rulings (this plan)

- **`port_block` mechanism.** Spec §8 rewrites `kind` on a `serde_json::Value` inside `io.rs`. serde_json is built **without** `preserve_order` (checked with `cargo tree -e features -i serde_json`), so a `Value` round trip would sort every object's keys and change every saved file. Instead, `EntityRepr` gains a variant `PortBlock(SymbolInstance)` with `#[schemars(skip)]`. `From<Entity> for EntityRepr` picks it only while a thread-local flag is set, and only `io::save_project` sets that flag (through `FileWriteGuard`). IPC, MCP and Link API JSON is unchanged (`symbol`). Reading accepts `port_block` anywhere and turns it into `Entity::Symbol`. The spec text (§8 and decision log) is updated in the same commit as this plan.
- **Not in this plan:** the MadakeCAD.pen block-table column (Pen MCP writes the active file; the user edits the design), and the `drawing-madakecad-wiring` skill text (it lives outside the repo; the coordinator updates it after merge).
- **SPICE** is unchanged (spec §2.2).
- **Sort key** is taken literally from the spec. Contact nets of one cluster are usually adjacent but can interleave with a net whose single-conductor wire has a smaller id. That is accepted.

## How this interacts with the merged cable-polish code (PR #16)

- **`CableSide.labels` (breakout wires that end in net labels, `plan_label_side`).** Each breakout wire with its label is a single-conductor cluster.
  - **Label named `<port label>.<contact>`** (for example `USB_MIC.VBUS`): the dotted-label rule (Task 3) joins that core to the contact. A labels-side cable can therefore feed a port drawn elsewhere, on the same sheet or across sheets. Test: Task 8 `breakout_labels_named_like_port_contacts_join_them`.
  - **Label named exactly the port label** (`USB_MIC`): the breakout wire becomes part of the port cluster. That is a breakout wire touching a port net, so the core goes to the residual slot and `check_ports` reports `erc.cable_core_unassigned` ("分岐の配線がポートのネットに触れています"). Nothing silently shorts.
  - **`connect_cable` side with `labels` and `style: single`:** cable-polish refuses it already. The port landing (`port_side`) runs only for a `pins`/`part` side, after the `labels` early return in `plan_side`.
- **`REF:POLE` (`reports::pin_display`, `mate::POLE_ATTR`) vs port contacts.**
  - A pole number only applies to `connector_plug` / `connector_socket`. Those are pass-through pins and never have contacts, so a wire-list text never carries both a pole and a contact.
  - The wire list uses the unsplit view (`extract_wire_nets`, Task 4), so its `NetPin`s have no contact and `resolve_label_end` keeps printing `X44A:2`.
  - Only run-cable core rows that land on a port pin get `/contact`. `with_contact` wraps the text that `pin_display` produced (`39:U3/VBUS`).
  - In the split netlist (`get_netlist`, xref table) a mate half is listed in every contact net with `contact` set. The xref table prints `J1:1/VBUS` (pin number, not pole). That is accepted: the xref table never used poles.
- **`cable::effective_wire`** fills a breakout wire's colour, gauge and part number from its core and cable for reports (`panel_wiring`, `terminal_chart`, `wire_legend`). A side landing on a port pin has no breakout wires, so it is unaffected. The netlist does not read wire specs.
- **`TextKind::CableLabel` (tidy)** counts the run label, core count and core names. A port landing adds no text and no bar, so tidy metrics do not change.
- **`place_mate` pole numbering** is unchanged. Mate halves stay pass-through whatever their pole.
- **Regenerated spec files.** If master moves again, re-run `python3 scripts/gen_spec.py` instead of hand-resolving `docs/13-specification*.md`.

## File map

| File | Responsibility in this plan |
|---|---|
| `src-tauri/crates/madake-core/src/block.rs` | `BlockPin.contacts`, validation, `BLOCK_CONTACT_MAX`, `BlockSpec::has_contacts`, test fixtures `port_block`, `pin_at`, `USB` |
| `src-tauri/crates/madake-core/src/symbol.rs` | `PinDef.contacts`, `PinDef::is_port` |
| `src-tauri/crates/madake-core/src/pin_wiring.rs` | `PinInfo.contacts` (feeds `get_pins`, `place_symbol`, `resolve_pin`) |
| `src-tauri/crates/madake-core/src/model.rs` | `CoreEnd::Pin.contact`, `EntityRepr::PortBlock`, `FileWriteGuard`, `FORMAT_VERSION` 18 |
| `src-tauri/crates/madake-core/src/cable.rs` | `validate_run_cable` key, pattern updates |
| `src-tauri/crates/madake-core/src/netlist.rs` | split algorithm, `extract_wire_nets`, `NetPin.contact`, `Net.contact`, `Net.contact_labels`, `PortReport`, `port_report` |
| `src-tauri/crates/madake-core/src/mate.rs` | `is_mate_half` |
| `src-tauri/crates/madake-core/src/wire_no.rs`, `svg.rs`, `dxf.rs`, `kicad.rs`, `tidy.rs`, `reports.rs` | switch to `extract_wire_nets`; `reports.rs` also prints `/contact` |
| `src-tauri/crates/madake-core/src/verify.rs` | `pin_net` as `Vec`, label conflict via `contact_labels`, `is_mate_half`, call `port` checks |
| `src-tauri/crates/madake-core/src/xref.rs` | merge keys from `contact_labels`, `REF:PIN/CONTACT` in the xref table, `labels_on_net` → `pub(crate)` |
| `src-tauri/crates/madake-core/src/port.rs` (new) | `check_ports`, `port_label_diagnostics`, ERC codes |
| `src-tauri/crates/madake-core/src/io.rs` | `save_project` sets `FileWriteGuard`; format tests |
| `src-tauri/crates/madake-core/src/cable_run.rs` | `port_side` (port landing) |
| `src-tauri/crates/madake-cli/src/format.rs` | `REF:PIN/CONTACT` in `madake netlist` |
| `src-tauri/crates/madake-mcp/src/lib.rs`, `tests/port_tools.rs` (new) | tool descriptions, MCP tests |
| `src-tauri/crates/madake-agent/src/knowledge.rs`, `tests/knowledge.rs` | one rule bullet + test |
| `src/ipc.ts`, `src/canvas/dynamicSymbol.ts`, `src/canvas/blockEditor.ts`, `src/components/BlockPinColumn.vue`, `src/components/BlockPinsEditor.vue`, `src/components/PropertiesPanel.vue`, `src/locales/*.json` | contacts in the UI |
| `docs/05-wire-management(.ja).md`, `docs/10-automation-api(.ja).md` | user docs |

---

### Task 1: Contacts on block pins (`BlockPin`, `PinDef`, `PinInfo`)

**Files:**
- Modify: `src-tauri/crates/madake-core/src/block.rs` (`BlockPin` at lines 47-56, `parse_block_spec` at 137-185, `block_symbol_from_spec` pin push at 306-311, test module)
- Modify: `src-tauri/crates/madake-core/src/symbol.rs:84-94` (`PinDef`), `:139-146` (`pin` helper)
- Modify: `src-tauri/crates/madake-core/src/pin_wiring.rs:16-24` (`PinInfo`), `:60-74` (`part_pins`)
- Modify (struct literals only): `src-tauri/crates/madake-core/src/pin_groups.rs:730-731`, `src-tauri/crates/madake-core/src/symbol_library.rs:507-512`, `src-tauri/crates/madake-core/src/netlist.rs:677-688`

**Interfaces:**
- Consumes: nothing new.
- Produces:
  - `pub struct BlockPin { pub no: String, pub name: String, pub contacts: Vec<String> }` (`#[serde(default)]` on `contacts`)
  - `pub const BLOCK_CONTACT_MAX: usize = 32;`
  - `impl BlockSpec { pub fn has_contacts(&self) -> bool }`
  - `pub struct PinDef { …, pub contacts: Vec<String> }` (`#[serde(default, skip_serializing_if = "Vec::is_empty")]`), `impl PinDef { pub fn is_port(&self) -> bool }`
  - `pub struct PinInfo { …, pub contacts: Vec<String> }` (`#[serde(skip_serializing_if = "Vec::is_empty")]`)
  - test fixtures in `block.rs`: `pub(crate) mod port_fixtures { pub(crate) const USB: [&str; 4]; pub(crate) fn port_block(reference: &str, x: f64, y: f64, side: &str, no: &str, contacts: &[&str]) -> SymbolInstance; pub(crate) fn pin_at(sheet: &Sheet, id: EntityId, no: &str) -> Point }`

- [ ] **Step 1: Write the failing tests** (append inside `mod tests` of `block.rs`)

```rust
    /// A block pin may list contacts; they reach the pin definition in order, trimmed, and a pin without contacts has none.
    /// ブロックのピンは接点を並べて持てる。接点は前後の空白を落とした順にピン定義へ届き、接点の無いピンは空のまま。
    #[test]
    fn contacts_are_trimmed() {
        let def = block_symbol(&attrs(&[(
            "block_pins",
            r#"{"right":[{"no":"U3","name":"USB-A メス","contacts":[" VBUS ","D-","D+","GND"]}],"left":[{"no":"1"}]}"#,
        )]))
        .expect("ブロック定義");
        let u3 = def.pins.iter().find(|p| p.number == "U3").unwrap();
        assert_eq!(u3.contacts, vec!["VBUS", "D-", "D+", "GND"]);
        assert!(u3.is_port());
        let one = def.pins.iter().find(|p| p.number == "1").unwrap();
        assert!(one.contacts.is_empty() && !one.is_port());
        let spec = parse_block_spec(&attrs(&[("block_pins", r#"{"left":[{"no":"1","contacts":["A","B"]}]}"#)])).unwrap();
        assert!(spec.has_contacts());
        assert!(!parse_block_spec(&attrs(&[("block_pins", r#"{"left":[{"no":"1"}]}"#)])).unwrap().has_contacts());
    }

    /// Contacts that are empty, repeated on one pin, alone (one conductor is a plain pin) or more than 32 are refused with the pin number in the message.
    /// 空の接点・1本のピンで重なる接点・1つだけの接点 (導体1本は普通のピン)・32を超える接点は、ピン番号入りの文で断る。
    #[test]
    fn bad_contacts_are_refused() {
        let err = |json: &str| block_symbol(&attrs(&[("block_pins", json)])).expect_err("断る");
        assert!(err(r#"{"left":[{"no":"P1","contacts":["A"," "]}]}"#).contains("P1"));
        assert!(err(r#"{"left":[{"no":"P1","contacts":["A","A"]}]}"#).contains("duplicate contact A"));
        assert!(err(r#"{"left":[{"no":"P1","contacts":["A"]}]}"#).contains("at least two contacts"));
        let many: Vec<String> = (0..=BLOCK_CONTACT_MAX).map(|i| format!("C{i}")).collect();
        let json = format!(r#"{{"left":[{{"no":"P1","contacts":{}}}]}}"#, serde_json::to_string(&many).unwrap());
        assert!(err(&json).contains("too many contacts"));
        // 別のピンと同じ接点名は使ってよい (USB ポートはどれも VBUS を持つ)
        block_symbol(&attrs(&[("block_pins", r#"{"left":[{"no":"1","contacts":["A","B"]},{"no":"2","contacts":["A","B"]}]}"#)]))
            .expect("別のピンなら同じ名前でよい");
    }
```

Append inside `mod tests` of `pin_wiring.rs`:

```rust
    /// get_pins shows a port pin's contacts and leaves them out for plain pins.
    /// get_pins はポートのピンの接点を出し、普通のピンには出さない。
    #[test]
    fn part_pins_show_contacts() {
        use crate::block::port_fixtures::{port_block, USB};
        let inst = port_block("39", 100.0, 100.0, "right", "U3", &USB);
        let def = crate::block::block_symbol(&inst.attrs).unwrap();
        let pins = part_pins(&inst, &def);
        assert_eq!(pins.pins[0].contacts, vec!["VBUS", "D-", "D+", "GND"]);
        let json = serde_json::to_value(&pins).unwrap();
        assert_eq!(json["pins"][0]["contacts"][1], "D-");
        let plain = crate::symbol::resolve_symbol("resistor").unwrap();
        let r = SymbolInstance { symbol_id: "resistor".into(), ..inst.clone() };
        let json = serde_json::to_value(part_pins(&r, &plain)).unwrap();
        assert!(json["pins"][0].get("contacts").is_none(), "{json}");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test -p madake-core contacts_are_trimmed bad_contacts_are_refused part_pins_show_contacts 2>&1 | tail -20`
Expected: compile errors: `no field contacts`, `cannot find value BLOCK_CONTACT_MAX`, `no method has_contacts`, `could not find port_fixtures`.

- [ ] **Step 3: Implement**

In `symbol.rs`, replace the `PinDef` struct and the `pin` helper:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PinDef {
    pub number: String,
    #[serde(default)]
    pub name: String,
    /// 接続点(ローカル座標)。
    pub at: Point,
    /// 接続方向(シンボルの外側へ電線が出る向き)。配線の自動接続・引き出しに使う。
    #[serde(default)]
    pub dir: PinDir,
    /// ポートの接点 (この1本のピンが運ぶ導体の名前、順に)。空 = 普通のピン。
    /// 今はブロック ([`crate::block`]) だけが持つ。ネットリストは接点ごとにネットを分ける。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contacts: Vec<String>,
}

impl PinDef {
    /// 接点を持つピン (ポート) か。
    pub fn is_port(&self) -> bool {
        !self.contacts.is_empty()
    }
}
```

```rust
fn pin(number: &str, name: &str, x: f64, y: f64, dir: PinDir) -> PinDef {
    PinDef {
        number: number.into(),
        name: name.into(),
        at: p(x, y),
        dir,
        contacts: Vec::new(),
    }
}
```

Add `contacts: Vec::new(),` to the struct literals in `pin_groups.rs:730-731` (both lines, inside the braces before `}`), `symbol_library.rs:507-512` and the two literals in `netlist.rs:677-688`.

In `block.rs`, replace `BlockPin`, add the constant after `BLOCK_PIN_MAX`, add `has_contacts`, and extend the validation loop in `parse_block_spec`:

```rust
/// 1本のピンが持てる接点の最大数。
pub const BLOCK_CONTACT_MAX: usize = 32;

/// ブロックの1ピン。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockPin {
    /// ピン番号(文字列。数字に限らず "A0" "VIN" など)。シンボル内で一意。
    pub no: String,
    /// ピン名(空可)。外形の内側に書く。
    #[serde(default)]
    pub name: String,
    /// ポートの接点 (例 `["VBUS","D-","D+","GND"]`)。空 = 普通のピン。描き方は変わらない。
    #[serde(default)]
    pub contacts: Vec<String>,
}
```

```rust
impl BlockSpec {
    /// どれかのピンが接点を持つか (ファイルでは種類 `port_block` で書く。[`crate::model::EntityRepr`])。
    pub fn has_contacts(&self) -> bool {
        self.pins.all().any(|p| !p.contacts.is_empty())
    }
}
```

In `parse_block_spec`, replace the loop `for pin in pins.all() { … }` with:

```rust
    for pin in pins.all() {
        let no = pin.no.trim();
        if no.is_empty() {
            return Err("block pin number must not be empty".into());
        }
        if !seen.insert(no) {
            return Err(format!("duplicate block pin number: {no}"));
        }
        check_contacts(no, &pin.contacts)?;
    }
```

and add below `parse_block_spec`:

```rust
/// ピン`no`の接点を調べる: 空でない・ピンの中で重ならない・2つ以上・[`BLOCK_CONTACT_MAX`]まで。
/// 接点が無いのは普通のピンなので通す。
fn check_contacts(no: &str, contacts: &[String]) -> Result<(), String> {
    if contacts.is_empty() {
        return Ok(());
    }
    if contacts.len() > BLOCK_CONTACT_MAX {
        return Err(format!(
            "block pin {no}: too many contacts: {} (max {BLOCK_CONTACT_MAX})",
            contacts.len()
        ));
    }
    if contacts.len() < 2 {
        return Err(format!(
            "block pin {no}: a port needs at least two contacts (one conductor is a plain pin)"
        ));
    }
    let mut seen: std::collections::BTreeSet<&str> = Default::default();
    for c in contacts {
        let c = c.trim();
        if c.is_empty() {
            return Err(format!("block pin {no}: contact names must not be empty"));
        }
        if !seen.insert(c) {
            return Err(format!("block pin {no}: duplicate contact {c}"));
        }
    }
    Ok(())
}
```

In `block_symbol_from_spec`, replace the `pins.push(PinDef { … })` with:

```rust
            pins.push(PinDef {
                number: bp.no.trim().into(),
                name: bp.name.clone(),
                at,
                dir,
                contacts: bp.contacts.iter().map(|c| c.trim().to_string()).collect(),
            });
```

Append after `test_block` in `block.rs`:

```rust
/// テスト用: ポート (接点を持つピン) のブロックと、ピンの位置 (他モジュールのテストが共有する)。
#[cfg(test)]
pub(crate) mod port_fixtures {
    use crate::geometry::Point;
    use crate::model::{Entity, EntityId, Sheet, SymbolInstance};

    /// USB 2.0 の接点。
    pub(crate) const USB: [&str; 4] = ["VBUS", "D-", "D+", "GND"];

    /// 辺 `side` ("left" / "right") にピン `no` を1本だけ持ち、そのピンが接点 `contacts` を持つブロック。
    pub(crate) fn port_block(reference: &str, x: f64, y: f64, side: &str, no: &str, contacts: &[&str]) -> SymbolInstance {
        let json = format!(
            r#"{{"{side}":[{{"no":"{no}","contacts":{}}}]}}"#,
            serde_json::to_string(contacts).unwrap()
        );
        match super::test_block(reference, "", x, y, &json) {
            Entity::Symbol(s) => s,
            _ => unreachable!(),
        }
    }

    /// 部品 `id` のピン `no` の用紙上の位置 (最初の接続点)。
    pub(crate) fn pin_at(sheet: &Sheet, id: EntityId, no: &str) -> Point {
        let Some(Entity::Symbol(s)) = sheet.entities.get(&id) else { panic!("部品 {id} が無い") };
        let defs = crate::symbol::sheet_symbol_defs(sheet);
        let def = crate::symbol::instance_symbol_def(s, &defs).expect("定義");
        crate::netlist::pin_positions(s, &def)
            .into_iter()
            .find(|(n, _)| n == no)
            .unwrap_or_else(|| panic!("ピン {no} が無い"))
            .1
    }
}
```

In `pin_wiring.rs`, add to `PinInfo`:

```rust
    /// ポートの接点 (空なら出さない)。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub contacts: Vec<String>,
```

and in `part_pins` set `contacts: pin.contacts.clone(),` in the `PinInfo { … }` literal.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-core 2>&1 | grep -E "^test result|FAILED|panicked" | head`
Expected: every `test result: ok.` line, no `FAILED`.

- [ ] **Step 5: Regenerate the spec and commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check
git add src-tauri/crates/madake-core/src docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(block): ブロックのピンに接点 (contacts) を持たせる"
```

---

### Task 2: `CoreEnd::Pin.contact`, validation key, `/contact` in the wire list

**Files:**
- Modify: `src-tauri/crates/madake-core/src/model.rs:849-864` (`CoreEnd`), and the test literal at `:1580`
- Modify: `src-tauri/crates/madake-core/src/cable.rs:599` (`pin_end_position`), `:621-680` (`validate_run_cable`), literals at `:656`, `:781`, `:812-813`, `:1115-1135`
- Modify: `src-tauri/crates/madake-core/src/io.rs:414`, `src-tauri/crates/madake-core/src/cable_run.rs:331,585,675,687` (literals)
- Modify: `src-tauri/crates/madake-core/src/reports.rs:355-359` (`EndKind`), `:374` (`resolve_endpoint`), `:419` (`endpoint_label`), `:491` (`from_to`), `:540` (`core_end_kind`)

**Interfaces:**
- Consumes: Task 1 `port_fixtures::{port_block, USB}`.
- Produces: `CoreEnd::Pin { part: EntityId, pin: String, index: Option<u32>, contact: Option<String> }` (`#[serde(default, skip_serializing_if = "Option::is_none")]` on `contact`); `fn with_contact(text: String, contact: Option<&str>) -> String` (private, reports.rs).

- [ ] **Step 1: Write the failing tests**

Append inside `mod tests` of `cable.rs`:

```rust
    /// Several cores may land on one port pin as long as each names a different contact; two cores on the same contact are refused.
    /// 1本のポートのピンには、接点が違えば芯を何本でも着けられる。同じ接点に芯2本は断る。
    #[test]
    fn cores_on_one_port_pin_need_different_contacts() {
        let mut lte = test_support::lte_sheet();
        let on_port = |c: &mut Cable, contacts: [&str; 4]| {
            for (core, contact) in c.cores.iter_mut().zip(contacts) {
                core.end_b = Some(CoreEnd::Pin { part: lte.usb, pin: "1".into(), index: None, contact: Some(contact.into()) });
            }
        };
        let Entity::Cable(c) = lte.sheet.entities.get_mut(&lte.cable).unwrap() else { panic!() };
        on_port(c, ["VBUS", "D-", "D+", "GND"]);
        let c = test_support::cable_of(&lte.sheet, lte.cable).clone();
        validate_run_cable(&lte.sheet, &c).expect("接点が違えば同じピンでよい");
        let mut twice = c.clone();
        on_port(&mut twice, ["VBUS", "VBUS", "D+", "GND"]);
        let err = validate_run_cable(&lte.sheet, &twice).expect_err("同じ接点に2本");
        assert!(err.contains("used by two cores"), "{err}");
    }

    /// A core end without a contact is written without a "contact" key, so files without ports do not change.
    /// 接点の無い芯の端は「contact」キーを書かないので、ポートの無いファイルは変わらない。
    #[test]
    fn a_pin_end_without_contact_has_no_contact_key() {
        let end = CoreEnd::Pin { part: uuid::Uuid::nil(), pin: "1".into(), index: None, contact: None };
        let json = serde_json::to_string(&end).unwrap();
        assert!(!json.contains("contact"), "{json}");
        let back: CoreEnd = serde_json::from_str(r#"{"kind":"pin","part":"00000000-0000-0000-0000-000000000000","pin":"U3","contact":"D+"}"#).unwrap();
        assert!(matches!(back, CoreEnd::Pin { contact: Some(c), .. } if c == "D+"));
    }
```

Append inside `mod tests` of `reports.rs`:

```rust
    /// A run-cable core that lands on a port pin shows its contact in the From-To row (39:U3/VBUS).
    /// ポートのピンに着く区間ケーブルの芯は、From-To の行に接点を書く (39:U3/VBUS)。
    #[test]
    fn a_core_on_a_port_pin_shows_its_contact() {
        use crate::block::port_fixtures::{port_block, USB};
        let mut lte = crate::cable::test_support::lte_sheet();
        let mut hub = port_block("39", 200.0, 100.0, "left", "U3", &USB);
        hub.id = lte.usb;
        lte.sheet.entities.insert(lte.usb, Entity::Symbol(hub));
        let Entity::Cable(c) = lte.sheet.entities.get_mut(&lte.cable).unwrap() else { panic!() };
        for core in &mut c.cores {
            core.end_b = Some(CoreEnd::Pin { part: lte.usb, pin: "U3".into(), index: None, contact: Some(core.core.clone()) });
        }
        let project = crate::cable::test_support::project_with(lte.sheet);
        let rows = wire_list_rows(&project);
        let ends: Vec<String> = rows.iter().map(|r| format!("{} {}", r[1], r[2])).collect();
        assert!(ends.contains(&"39:U3/VBUS J45:1".to_string()) || ends.contains(&"J45:1 39:U3/VBUS".to_string()), "{ends:?}");
    }
```

(`reports.rs` tests already `use crate::model::*;`; if `CoreEnd` is not in scope there, add `use crate::model::CoreEnd;` at the top of the test.)

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test -p madake-core cores_on_one_port_pin a_pin_end_without_contact a_core_on_a_port_pin 2>&1 | tail -15`
Expected: compile error `variant CoreEnd::Pin does not have a field named contact`.

- [ ] **Step 3: Implement**

`model.rs`, replace the `Pin` variant:

```rust
    Pin {
        part: EntityId,
        pin: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<u32>,
        /// ポートのピン ([`crate::symbol::PinDef::contacts`]) のどの接点か (v18)。普通のピンでは None。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        contact: Option<String>,
    },
```

If `CoreEnd::remapped` builds `CoreEnd::Pin { part, pin, index }`, make it carry `contact: contact.clone()`. Check with `grep -n "CoreEnd::Pin" src-tauri/crates/madake-core/src/model.rs`.

Add `contact: None` to every literal listed under Files. Use `grep -rn "CoreEnd::Pin {" src-tauri --include=*.rs | grep -v "\.\. }"` until nothing is left without `contact` or `..`. Patterns: `cable.rs:599` becomes `let CoreEnd::Pin { part, pin, index, .. } = end else {`.

`cable.rs` `validate_run_cable`: change the pin key to include the contact.

```rust
    let mut used_pins: Vec<(EntityId, String, u32, Option<String>)> = Vec::new();
```

```rust
                CoreEnd::Pin { part, pin, index, contact } => {
                    if matches!(sheet.entities.get(part), Some(other) if !matches!(other, Entity::Symbol(_))) {
                        return Err(format!(
                            "cable {} core {} end_{} points at {part}, which is not a part",
                            c.reference,
                            core.core,
                            end.letter()
                        ));
                    }
                    // 1本のポートのピンには接点が違えば何本でも着く (接点がブロックにあるかは ERC が見る)
                    let key = (*part, pin.clone(), index.unwrap_or(1), contact.clone());
                    if used_pins.contains(&key) {
                        return Err(format!("cable {}: pin {pin} of {part} is used by two cores", c.reference));
                    }
                    used_pins.push(key);
                }
```

`reports.rs`:

```rust
enum EndKind {
    Pin { entity_id: crate::EntityId, reference: String, pin: String, contact: Option<String> },
    Label(String),
    None,
}

/// ピンの表記にポートの接点を添える (`39:U3` → `39:U3/VBUS`)。接点が無ければそのまま。
fn with_contact(text: String, contact: Option<&str>) -> String {
    match contact {
        Some(c) => format!("{text}/{c}"),
        None => text,
    }
}
```

In `resolve_endpoint`, the line `(Some((_, entity_id, reference, pin)), _) => EndKind::Pin { entity_id, reference, pin },` becomes `… => EndKind::Pin { entity_id, reference, pin, contact: None },`. In `endpoint_label`:

```rust
        EndKind::Pin { entity_id, reference, pin, contact } => {
            with_contact(format!("{reference}:{}", pin_display(sheet, entity_id, &pin)), contact.as_deref())
        }
```

and in `from_to`:

```rust
            EndKind::Pin { entity_id, reference, pin, contact } => {
                with_contact(format!("{reference}:{}", pin_display(sheet, *entity_id, pin)), contact.as_deref())
            }
```

(Keep the other arms unchanged. Leave the `EndKind::Pin { entity_id, pin, .. }` exclusion pattern as it is. `pin_display` (REF:POLE) only rewrites plug/socket pins, which never carry a contact.) In `core_end_kind`:

```rust
        Some(CoreEnd::Pin { part, pin, contact, .. }) => match sheet.entities.get(part) {
            Some(Entity::Symbol(s)) => EndKind::Pin {
                entity_id: s.id,
                reference: s.reference.clone(),
                pin: pin.clone(),
                contact: contact.clone(),
            },
            _ => EndKind::None,
        },
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test --workspace 2>&1 | grep -E "^test result|FAILED|panicked|error\[" | head -20`
Expected: all `test result: ok.`, no `FAILED`, no `error[`.

- [ ] **Step 5: Regenerate the spec and commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check
git add src-tauri docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(cable): 芯の端にポートの接点 (contact) を持たせ、電線リストに接点を書く"
```

---

### Task 3: Netlist split per contact (`extract_netlist`, `extract_wire_nets`, `port_report`)

**Files:**
- Modify: `src-tauri/crates/madake-core/src/netlist.rs` (whole of `extract_netlist` at lines 195-431, `NetPin` at 58-67, `Net` at 69-85, test module)
- Modify: `src-tauri/crates/madake-core/src/mate.rs:35-38` (add `is_mate_half` after `is_connector_instance`)

**Interfaces:**
- Consumes: Task 1 `PinDef.contacts`, `port_fixtures`; Task 2 `CoreEnd::Pin.contact`.
- Produces:
  - `pub fn extract_netlist(sheet: &Sheet, symbols: &[SymbolDef]) -> Vec<Net>` (split)
  - `pub fn extract_wire_nets(sheet: &Sheet, symbols: &[SymbolDef]) -> Vec<Net>` (today's behaviour)
  - `pub(crate) fn port_report(sheet: &Sheet, symbols: &[SymbolDef]) -> PortReport`
  - `pub struct NetPin { reference, entity_id, pin, pub contact: Option<String> }`
  - `pub struct Net { …, pub contact: Option<String>, pub contact_labels: Vec<String> }`
  - `#[derive(Debug, Clone, Default, PartialEq)] pub struct PortReport { pub port_labels: BTreeSet<String>, pub plain_pins: Vec<PlainOnPort>, pub lonely_contacts: Vec<LonelyContact>, pub stray_cores: Vec<StrayCore> }`
  - `pub struct PlainOnPort { pub entity_id: EntityId, pub pin: String, pub ports: Vec<String>, pub contacts: Vec<String> }`
  - `pub struct LonelyContact { pub contact: String, pub port: String, pub entity_ids: Vec<EntityId> }`
  - `pub struct StrayCore { pub cable_id: EntityId, pub reference: String, pub core: String, pub why: String }`
  - `pub fn mate::is_mate_half(symbol_id: &str) -> bool`

- [ ] **Step 1: Write the failing tests** (append inside `mod tests` of `netlist.rs`)

```rust
    use crate::block::port_fixtures::{pin_at, port_block, USB};

    fn put(sheet: &mut Sheet, e: Entity) -> EntityId {
        let id = e.id();
        sheet.entities.insert(id, e);
        id
    }

    /// Hub port 39.U3 and dongle port LTE1.P1 joined by one wire (A3 sheet).
    fn usb_link() -> (Sheet, EntityId, EntityId, EntityId) {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let hub = put(&mut sheet, Entity::Symbol(port_block("39", 100.0, 100.0, "right", "U3", &USB)));
        let dongle = put(&mut sheet, Entity::Symbol(port_block("LTE1", 200.0, 100.0, "left", "P1", &USB)));
        let (a, b) = (pin_at(&sheet, hub, "U3"), pin_at(&sheet, dongle, "P1"));
        let w = put(&mut sheet, wire(&[(a.x, a.y), (b.x, b.y)]));
        (sheet, hub, dongle, w)
    }

    fn pins_of(net: &Net) -> Vec<String> {
        net.pins
            .iter()
            .map(|p| match &p.contact {
                Some(c) => format!("{}:{}/{c}", p.reference, p.pin),
                None => format!("{}:{}", p.reference, p.pin),
            })
            .collect()
    }

    /// Two port pins joined by one wire give one net per contact, named N001.<contact> in contact order, each holding both pins with that contact and the shared wire.
    /// 2本のポートのピンを配線1本で結ぶと接点ごとに1ネットになり、名前は接点の順に N001.<接点>、どれにも両方のピンがその接点付きで、共有の配線と一緒に載る。
    #[test]
    fn two_ports_joined_by_one_wire_split_into_one_net_per_contact() {
        let (sheet, _, _, w) = usb_link();
        let nets = extract_netlist(&sheet, &crate::symbol::sheet_symbol_defs(&sheet));
        let names: Vec<&str> = nets.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, vec!["N001.VBUS", "N001.D-", "N001.D+", "N001.GND"]);
        assert_eq!(pins_of(&nets[2]), vec!["39:U3/D+", "LTE1:P1/D+"]);
        assert_eq!(nets[2].contact.as_deref(), Some("D+"));
        assert!(nets.iter().all(|n| n.wire_ids == vec![w]));
    }

    /// The unsplit view keeps one net for the USB wire, and a drawing without ports gives exactly the same nets (and JSON, with no contact keys) from both functions.
    /// 分けない方では USB の配線は1ネットのまま。ポートの無い図面は、どちらの関数でも全く同じネット (JSON も同じで、contact のキーは無い) になる。
    #[test]
    fn without_ports_nothing_changes() {
        let (sheet, ..) = usb_link();
        let defs = crate::symbol::sheet_symbol_defs(&sheet);
        let one = extract_wire_nets(&sheet, &defs);
        assert_eq!(one.len(), 1);
        assert!(one[0].pins.iter().all(|p| p.contact.is_none()));
        let lte = crate::cable::test_support::lte_sheet().sheet;
        let defs = crate::symbol::sheet_symbol_defs(&lte);
        let split = serde_json::to_string(&extract_netlist(&lte, &defs)).unwrap();
        let wire = serde_json::to_string(&extract_wire_nets(&lte, &defs)).unwrap();
        assert_eq!(split, wire);
        assert!(!split.contains("contact"), "{split}");
    }

    /// A mate half passes every contact through inside its own net, but plug and socket are still not joined: port → socket | plug → label … port gives two groups of contact nets.
    /// かみ合わせの片側は自分のネットの中で全部の接点を素通しにするが、プラグとソケットは今も繋がらない: ポート → ソケット | プラグ → ラベル … ポートは接点のネットの組2つになる。
    #[test]
    fn a_mate_half_passes_contacts_through_but_does_not_conduct() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let hub = put(&mut sheet, Entity::Symbol(port_block("39", 50.0, 100.0, "right", "U3", &USB)));
        let socket = put(&mut sheet, symbol("connector_socket", "J1", 100.0, 100.0));
        let plug = put(&mut sheet, symbol("connector_plug", "J1", 140.0, 100.0));
        let dongle = put(&mut sheet, Entity::Symbol(port_block("LTE1", 250.0, 100.0, "left", "P1", &USB)));
        let (h, s) = (pin_at(&sheet, hub, "U3"), pin_at(&sheet, socket, "1"));
        let (p, d) = (pin_at(&sheet, plug, "1"), pin_at(&sheet, dongle, "P1"));
        put(&mut sheet, wire(&[(h.x, h.y), (s.x, s.y)]));
        put(&mut sheet, wire(&[(p.x, p.y), (p.x, 120.0)]));
        put(&mut sheet, wire(&[(d.x, d.y), (d.x, 120.0)]));
        for (x, y) in [(p.x, 120.0), (d.x, 120.0)] {
            put(&mut sheet, Entity::NetLabel(NetLabel { id: Uuid::new_v4(), at: Point::new(x, y), name: "USB_MIC".into(), rotation: 0, attrs: Default::default() }));
        }
        let nets = extract_netlist(&sheet, &crate::symbol::sheet_symbol_defs(&sheet));
        assert_eq!(nets.len(), 8, "{:?}", nets.iter().map(|n| &n.name).collect::<Vec<_>>());
        let vbus = nets.iter().find(|n| n.name == "USB_MIC.VBUS").expect("USB_MIC.VBUS");
        assert!(vbus.pins.iter().any(|x| x.entity_id == plug && x.contact.as_deref() == Some("VBUS")));
        assert!(vbus.pins.iter().any(|x| x.entity_id == dongle));
        assert!(!vbus.pins.iter().any(|x| x.entity_id == socket || x.entity_id == hub));
        let hub_side = nets.iter().find(|n| n.pins.iter().any(|x| x.entity_id == hub) && n.contact.as_deref() == Some("GND")).unwrap();
        assert!(hub_side.pins.iter().any(|x| x.entity_id == socket));
    }

    /// In a port net a plain pin named like a contact joins that contact; a plain pin with another name goes to a residual net named like the base and is listed in the port report.
    /// ポートのネットの中で、接点と同じ名前の普通のピンはその接点に入る。名前の違う普通のピンは元の名前の残りのネットに入り、ポートの報告に載る。
    #[test]
    fn a_plain_pin_joins_the_contact_with_its_name() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let hub = put(&mut sheet, Entity::Symbol(port_block("39", 100.0, 100.0, "right", "U3", &USB)));
        let pwr = put(&mut sheet, crate::block::test_block("PWR", "", 200.0, 100.0, r#"{"left":[{"no":"3","name":"GND"}]}"#));
        let r1 = put(&mut sheet, symbol("resistor", "R1", 150.0, 140.0));
        let (h, g, r) = (pin_at(&sheet, hub, "U3"), pin_at(&sheet, pwr, "3"), pin_at(&sheet, r1, "1"));
        put(&mut sheet, wire(&[(h.x, h.y), (g.x, g.y)]));
        put(&mut sheet, wire(&[(h.x, h.y), (h.x, r.y), (r.x, r.y)]));
        let defs = crate::symbol::sheet_symbol_defs(&sheet);
        let nets = extract_netlist(&sheet, &defs);
        let gnd = nets.iter().find(|n| n.name == "N001.GND").unwrap();
        assert_eq!(pins_of(gnd), vec!["39:U3/GND", "PWR:3"]);
        let residual = nets.iter().find(|n| n.name == "N001").expect("残りのネット");
        assert_eq!(pins_of(residual), vec!["R1:1"]);
        assert_eq!(residual.contact, None);
        let report = port_report(&sheet, &defs);
        assert_eq!(report.plain_pins.len(), 1);
        assert_eq!(report.plain_pins[0].pin, "R1:1");
        assert_eq!(report.plain_pins[0].ports, vec!["39:U3"]);
    }

    /// The LTE harness drawn as four board wires into one run that ends on the single port pin 39.U3 gives four nets, each one board pin and one contact of the port.
    /// LTE ハーネスを、基板側4本の配線から1本線になってポートのピン 39.U3 1本で終わる形で描くと4ネットになり、どれも基板のピン1本とポートの接点1つを持つ。
    #[test]
    fn the_lte_harness_lands_on_one_port_pin() {
        let mut lte = crate::cable::test_support::lte_sheet();
        let mut hub = port_block("39", 200.0, 100.0, "left", "U3", &USB);
        hub.id = lte.usb;
        lte.sheet.entities.insert(lte.usb, Entity::Symbol(hub));
        let Entity::Cable(c) = lte.sheet.entities.get_mut(&lte.cable).unwrap() else { panic!() };
        for core in &mut c.cores {
            core.end_b = Some(CoreEnd::Pin { part: lte.usb, pin: "U3".into(), index: None, contact: Some(core.core.clone()) });
        }
        let nets = extract_netlist(&lte.sheet, &crate::symbol::sheet_symbol_defs(&lte.sheet));
        let mut got: Vec<Vec<String>> = nets.iter().map(pins_of).collect();
        got.sort();
        assert_eq!(
            got,
            vec![
                vec!["39:U3/D+".to_string(), "J45:3".to_string()],
                vec!["39:U3/D-".to_string(), "J45:2".to_string()],
                vec!["39:U3/GND".to_string(), "J45:4".to_string()],
                vec!["39:U3/VBUS".to_string(), "J45:1".to_string()],
            ]
        );
    }

    /// A label named <port label>.<contact> on a plain wire joins that one contact of the port net.
    /// 普通の配線に「<ポートのラベル>.<接点>」という名前のラベルを置くと、ポートのネットのその接点1つに繋がる。
    #[test]
    fn a_dotted_label_taps_one_contact() {
        let (mut sheet, hub, ..) = usb_link();
        let h = pin_at(&sheet, hub, "U3");
        put(&mut sheet, Entity::NetLabel(NetLabel { id: Uuid::new_v4(), at: Point::new(h.x + 10.0, h.y), name: "USB_MIC".into(), rotation: 0, attrs: Default::default() }));
        let r1 = put(&mut sheet, symbol("resistor", "R1", 150.0, 150.0));
        let r = pin_at(&sheet, r1, "1");
        put(&mut sheet, wire(&[(r.x, r.y), (r.x - 20.0, r.y)]));
        put(&mut sheet, Entity::NetLabel(NetLabel { id: Uuid::new_v4(), at: Point::new(r.x - 20.0, r.y), name: "USB_MIC.VBUS".into(), rotation: 0, attrs: Default::default() }));
        let nets = extract_netlist(&sheet, &crate::symbol::sheet_symbol_defs(&sheet));
        let vbus = nets.iter().find(|n| n.name == "USB_MIC.VBUS").unwrap();
        assert!(pins_of(vbus).contains(&"R1:1".to_string()), "{:?}", pins_of(vbus));
        assert_eq!(vbus.contact_labels, vec!["USB_MIC.VBUS"]);
        assert_eq!(vbus.label.as_deref(), Some("USB_MIC.VBUS"));
    }

    /// A single-line run between two port pins with no wire at all still gives one net per core, each with both ports.
    /// 配線が1本も無く、2本のポートのピンの間を1本線で結んでも、芯ごとに1ネットになり、どれにも両方のポートが載る。
    #[test]
    fn a_run_between_two_ports_gives_one_net_per_core() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let hub = put(&mut sheet, Entity::Symbol(port_block("39", 100.0, 100.0, "right", "U3", &USB)));
        let dongle = put(&mut sheet, Entity::Symbol(port_block("LTE1", 200.0, 100.0, "left", "P1", &USB)));
        let (a, b) = (pin_at(&sheet, hub, "U3"), pin_at(&sheet, dongle, "P1"));
        let cores = USB
            .iter()
            .map(|c| CableCore {
                core: (*c).into(),
                end_a: Some(CoreEnd::Pin { part: hub, pin: "U3".into(), index: None, contact: Some((*c).into()) }),
                end_b: Some(CoreEnd::Pin { part: dongle, pin: "P1".into(), index: None, contact: Some((*c).into()) }),
                ..Default::default()
            })
            .collect();
        put(&mut sheet, Entity::Cable(Cable {
            id: Uuid::new_v4(), reference: "W1".into(), part_no: String::new(), core_count: 4, sq: 0.0,
            points: Vec::new(), cores, shield: None, length_m: None, run: vec![a, b],
        }));
        let nets = extract_netlist(&sheet, &crate::symbol::sheet_symbol_defs(&sheet));
        assert_eq!(nets.len(), 4);
        for n in &nets {
            let c = n.contact.clone().expect("接点のネット");
            assert_eq!(pins_of(n), vec![format!("39:U3/{c}"), format!("LTE1:P1/{c}")]);
            assert!(n.wire_ids.is_empty() && n.cable_cores.len() == 1);
        }
    }

    /// Two ports with different contact sets join only the shared contacts; the others stay alone on their port, with no residual net, and the report lists them.
    /// 接点の組が違う2つのポートは共通の接点だけが繋がる。それ以外は自分のポートだけのネットになり、残りのネットは出ず、報告に載る。
    #[test]
    fn ports_with_different_contacts_join_only_the_shared_ones() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let hub = put(&mut sheet, Entity::Symbol(port_block("39", 100.0, 100.0, "right", "U3", &USB)));
        let otg = put(&mut sheet, Entity::Symbol(port_block("OTG1", 200.0, 100.0, "left", "P1", &["VBUS", "GND", "ID"])));
        let (a, b) = (pin_at(&sheet, hub, "U3"), pin_at(&sheet, otg, "P1"));
        put(&mut sheet, wire(&[(a.x, a.y), (b.x, b.y)]));
        let defs = crate::symbol::sheet_symbol_defs(&sheet);
        let nets = extract_netlist(&sheet, &defs);
        let names: Vec<&str> = nets.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, vec!["N001.VBUS", "N001.D-", "N001.D+", "N001.GND", "N001.ID"]);
        assert_eq!(pins_of(&nets[0]), vec!["39:U3/VBUS", "OTG1:P1/VBUS"]);
        assert_eq!(pins_of(&nets[1]), vec!["39:U3/D-"]);
        let lonely: Vec<(String, String)> = port_report(&sheet, &defs).lonely_contacts.into_iter().map(|l| (l.contact, l.port)).collect();
        assert_eq!(
            lonely,
            vec![("D-".into(), "39:U3".into()), ("D+".into(), "39:U3".into()), ("ID".into(), "OTG1:P1".into())]
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test -p madake-core netlist:: 2>&1 | tail -15`
Expected: compile errors `cannot find function extract_wire_nets`, `cannot find function port_report`, `no field contact on NetPin`.

- [ ] **Step 3: Implement**

`mate.rs`, after `is_connector_instance`:

```rust
/// かみ合わせの片側 (1極のプラグ/ソケット)。ネットリストでは、ポートの接点を全部素通しにするピン
/// ([`crate::netlist`])。参照記号の重複の例外 ([`crate::verify`]) も同じ判定を使う。
pub fn is_mate_half(symbol_id: &str) -> bool {
    matches!(symbol_id, "connector_plug" | "connector_socket")
}
```

`netlist.rs`: change the imports to

```rust
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::geometry::Point;
use crate::model::{Cable, CableCore, CoreEnd, Entity, EntityId, Sheet, SymbolInstance};
use crate::symbol::SymbolDef;
```

Replace `NetPin` and `Net`:

```rust
/// ネットに属するシンボルピン。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NetPin {
    /// 参照記号 (例: "K1")。
    pub reference: String,
    /// シンボルインスタンスのentity id。
    pub entity_id: EntityId,
    /// ピン番号。
    pub pin: String,
    /// このネットで運ぶポートの接点 (ポートのピン・かみ合わせの片側だけ。例 "VBUS")。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact: Option<String>,
}

/// 電気的に接続された1ネット。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Net {
    /// 表示名。優先順は ネットラベル > 線番 > 自動名 (N001…)。ポートの接点のネットは「<名前>.<接点>」。
    pub name: String,
    pub pins: Vec<NetPin>,
    pub wire_ids: Vec<EntityId>,
    /// ネットラベル由来の名前 (手動指定)。接点のネットでは「<ラベル>.<接点>」。
    #[serde(default)]
    pub label: Option<String>,
    /// 線番 (`Wire::net`)。ネット内の全Wireで同値になる。
    #[serde(default)]
    pub wire_no: Option<String>,
    /// このネットを通る区間ケーブルの芯 (v17)。ケーブルid・参照記号・芯の呼びの順に並ぶ。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cable_cores: Vec<NetCableCore>,
    /// ポートから分けたネットなら、その接点 (v18。例 "VBUS")。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact: Option<String>,
    /// ポートから分けたネットが、シートを跨いで呼ばれる名前 (ポートのラベルは「L.接点」、導体1本の部分の
    /// ラベルはそのまま)。空なら [`crate::xref`] は今までどおりシートの上でラベルを探す。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contact_labels: Vec<String>,
}
```

Add the report types after `NetCableCore`:

```rust
/// ポート (接点を持つピン) のまわりで ERC が知らせること ([`crate::port::check_ports`])。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PortReport {
    /// ポートのかたまりに載っているネットラベルの名前。
    pub port_labels: BTreeSet<String>,
    /// ポートのかたまりの中で、どの接点とも名前が合わない普通のピン。
    pub plain_pins: Vec<PlainOnPort>,
    /// ポートのピンが2本以上あるかたまりで、1本にしか無い接点。
    pub lonely_contacts: Vec<LonelyContact>,
    /// 接点を決められない区間ケーブルの芯の端。
    pub stray_cores: Vec<StrayCore>,
}

/// ポートのネットにいる、接点の分からない普通のピン。
#[derive(Debug, Clone, PartialEq)]
pub struct PlainOnPort {
    pub entity_id: EntityId,
    /// "J45:1"
    pub pin: String,
    /// 同じかたまりのポートのピン ("39:U3")。
    pub ports: Vec<String>,
    /// そのかたまりの接点の並び。
    pub contacts: Vec<String>,
}

/// 1本のポートのピンにしか無い接点。
#[derive(Debug, Clone, PartialEq)]
pub struct LonelyContact {
    pub contact: String,
    /// "39:U3"
    pub port: String,
    pub entity_ids: Vec<EntityId>,
}

/// 接点を決められない区間ケーブルの芯。
#[derive(Debug, Clone, PartialEq)]
pub struct StrayCore {
    pub cable_id: EntityId,
    pub reference: String,
    pub core: String,
    pub why: String,
}

/// 配置済みピン1本 (ネットリストのノード)。
struct PinNode {
    entity_id: EntityId,
    reference: String,
    no: String,
    /// ピン名 (前後の空白を落としたもの)。ポートの接点と名前で合わせる。
    name: String,
    /// ポートの接点。空 = 普通のピン。
    contacts: Vec<String>,
    /// かみ合わせの片側のピン (ポートの接点を全部素通しにする)。
    pass: bool,
    pos: Point,
}

impl PinNode {
    fn label(&self) -> String {
        format!("{}:{}", self.reference, self.no)
    }
}

/// かたまり (芯を入れずに作った接続のまとまり) ごとの枠の割り振り。導体1本のかたまりは枠1つ、
/// ポートのかたまりは接点ごとに1つと残りの枠1つ (最後)。
struct Slots {
    first: BTreeMap<usize, usize>,
    contacts: BTreeMap<usize, Vec<String>>,
    count: usize,
}

impl Slots {
    fn new(roots: &[usize], contacts: BTreeMap<usize, Vec<String>>) -> Self {
        let mut first = BTreeMap::new();
        let mut count = 0;
        for &r in roots {
            if let std::collections::btree_map::Entry::Vacant(v) = first.entry(r) {
                v.insert(count);
                count += contacts.get(&r).map_or(1, |l| l.len() + 1);
            }
        }
        Self { first, contacts, count }
    }

    fn is_port(&self, root: usize) -> bool {
        self.contacts.contains_key(&root)
    }

    /// 導体1本のかたまりならその枠、ポートのかたまりなら残りの枠。
    fn single_or_residual(&self, root: usize) -> usize {
        self.first[&root] + self.contacts.get(&root).map_or(0, Vec::len)
    }

    fn contact(&self, root: usize, name: &str) -> Option<usize> {
        let i = self.contacts.get(&root)?.iter().position(|c| c == name)?;
        Some(self.first[&root] + i)
    }

    /// 配線・ラベルが入る枠 (導体1本ならその枠、ポートなら接点の枠全部と残りの枠)。
    fn every(&self, root: usize) -> std::ops::RangeInclusive<usize> {
        self.first[&root]..=self.single_or_residual(root)
    }
}

/// 線番の選び方: 手動の名前 (数字以外) を自動採番より先に、その次は辞書順。
fn pick_wire_no(nos: &[String]) -> Option<String> {
    nos.iter()
        .min_by(|a, b| {
            (crate::wire_no::is_generated_number(a), *a).cmp(&(crate::wire_no::is_generated_number(b), *b))
        })
        .cloned()
}

fn stray(report: &mut PortReport, c: &Cable, core: &CableCore, why: String) {
    report.stray_cores.push(StrayCore { cable_id: c.id, reference: c.reference.clone(), core: core.core.clone(), why });
}
```

Replace the doc comment and body of `extract_netlist` (everything from `/// シートからネットリストを導出する。` to the end of the function) with:

```rust
/// シートからネットリストを導出する (ポートは接点ごとに分ける)。
///
/// 接続ルール:
/// - Wire同士は「端点同士の一致」または「一致点上のJunction経由」でのみ接続(交差だけでは非接続)
/// - シンボルピン・NetLabelはWireの任意の頂点との一致で接続
/// - 同名NetLabelのネットは統合され、ネット名はラベル名(辞書順最小)になる
/// - 区間ケーブル ([`crate::model::Cable::run`]) の芯は、芯の行き先 ([`CoreEnd`]) が指すピン・配線と
///   だけ繋がる。区間の線は位置では何にも繋がらない (ピン列の上を通っても短絡しない)
/// - 配線か区間ケーブルの芯を1つ以上含むまとまりをネットにする。芯だけのネットは配線のあるネットの後ろ
/// - 無名ネットは決定的な順序で "N001" から連番
/// - **ポート** (接点を持つピン、[`crate::symbol::PinDef::contacts`]) を含むまとまりは接点ごとのネット
///   「<名前>.<接点>」に分かれる。配線・ラベル・かみ合わせの片側は全部の接点を運び、普通のピンは
///   ピン名と同じ接点に入る (仕様 2026-10-25-port-contacts-design §2)。ポートが無ければ
///   [`extract_wire_nets`] と全く同じ
pub fn extract_netlist(sheet: &Sheet, symbols: &[SymbolDef]) -> Vec<Net> {
    build(sheet, symbols, true).0
}

/// 描いた配線のまとまりごとに1ネットのネットリスト (ポートを分けない、v17までの [`extract_netlist`])。
/// 線番の採番・線番の描画・電線リストのように、配線1本を1件として扱う所が使う。
pub fn extract_wire_nets(sheet: &Sheet, symbols: &[SymbolDef]) -> Vec<Net> {
    build(sheet, symbols, false).0
}

/// ポートのまわりの ERC の材料 ([`crate::port::check_ports`])。
pub(crate) fn port_report(sheet: &Sheet, symbols: &[SymbolDef]) -> PortReport {
    build(sheet, symbols, true).1
}

fn build(sheet: &Sheet, symbols: &[SymbolDef], split: bool) -> (Vec<Net>, PortReport) {
    let defs = crate::symbol::DefIndex::new(symbols);

    // ノード列挙 (BTreeMap順なので決定的)
    let mut wires = Vec::new();
    let mut junctions = Vec::new();
    let mut labels = Vec::new();
    let mut cables = Vec::new();
    let mut pins: Vec<PinNode> = Vec::new();
    for entity in sheet.entities.values() {
        match entity {
            Entity::Wire(w) => wires.push(w),
            Entity::Junction(j) => junctions.push(j),
            Entity::NetLabel(l) => labels.push(l),
            Entity::Cable(c) if c.is_run() => cables.push(c),
            Entity::Symbol(s) => {
                if let Some(def) = defs.get(s) {
                    let pass = crate::mate::is_mate_half(&s.symbol_id);
                    for pin in &def.pins {
                        pins.push(PinNode {
                            entity_id: s.id,
                            reference: s.reference.clone(),
                            no: pin.number.clone(),
                            name: pin.name.trim().to_string(),
                            contacts: pin.contacts.clone(),
                            pass,
                            pos: transform_local(pin.at, s),
                        });
                    }
                }
            }
            // 注記・図形・ハーネス境界・盤レイアウトの要素は電気的な接続を持たない
            Entity::Text(_)
            | Entity::Shape(_)
            | Entity::Harness(_)
            | Entity::PanelOutline(_)
            | Entity::DinRail(_)
            | Entity::Duct(_)
            | Entity::Footprint(_)
            | Entity::PanelLegend(_)
            | Entity::WireLegend(_)
            | Entity::Cable(_)
            | Entity::Nameplate(_)
            | Entity::Dimension(_) => {}
        }
    }

    // ノード番号: [wires][pins][junctions][labels]。芯はかたまりを作った後で枠どうしを結ぶ
    let wi = 0;
    let pi = wi + wires.len();
    let ji = pi + pins.len();
    let li = ji + junctions.len();
    let ci = li + labels.len();
    let cores: Vec<(&Cable, &CableCore)> = cables
        .iter()
        .flat_map(|c| c.cores.iter().map(move |k| (*c, k)))
        .collect();
    let mut ds = DisjointSet::new(ci);

    let endpoints = |w: &crate::model::Wire| -> Vec<Point> {
        match (w.points.first(), w.points.last()) {
            (Some(a), Some(b)) => vec![*a, *b],
            _ => vec![],
        }
    };

    // Wire端点同士
    for (a, wa) in wires.iter().enumerate() {
        for (b, wb) in wires.iter().enumerate().skip(a + 1) {
            let touch = endpoints(wa)
                .iter()
                .any(|ea| endpoints(wb).iter().any(|eb| near(ea, eb)));
            if touch {
                ds.union(wi + a, wi + b);
            }
        }
    }
    // Junction ⇔ Wire上の任意点(線分途中も可)
    for (j, junc) in junctions.iter().enumerate() {
        for (a, w) in wires.iter().enumerate() {
            if on_wire(w, &junc.at) {
                ds.union(ji + j, wi + a);
            }
        }
    }
    // ピン ⇔ Wire上の任意点
    for (p, node) in pins.iter().enumerate() {
        for (a, w) in wires.iter().enumerate() {
            if on_wire(w, &node.pos) {
                ds.union(pi + p, wi + a);
            }
        }
    }
    // 同一シンボル内の同一ピン番号の接続点は内部短絡(端子台の左右貫通)
    for (p, a) in pins.iter().enumerate() {
        for (p2, b) in pins.iter().enumerate().skip(p + 1) {
            if a.entity_id == b.entity_id && a.no == b.no {
                ds.union(pi + p, pi + p2);
            }
        }
    }
    // ラベル ⇔ Wire上の任意点 / シンボルピン、同名ラベル同士
    // (載り判定は[`label_is_attached`]と同じ2つの述語。ERCの浮きラベル判定と揃える)
    for (l, lab) in labels.iter().enumerate() {
        for (a, w) in wires.iter().enumerate() {
            if on_wire(w, &lab.at) {
                ds.union(li + l, wi + a);
            }
        }
        for (p, node) in pins.iter().enumerate() {
            if near(&node.pos, &lab.at) {
                ds.union(li + l, pi + p);
            }
        }
        for (l2, lab2) in labels.iter().enumerate().skip(l + 1) {
            if lab.name == lab2.name {
                ds.union(li + l, li + l2);
            }
        }
    }

    // かたまり (芯を入れない) と、ポートのかたまりの接点の並び
    let roots: Vec<usize> = (0..ci).map(|i| ds.find(i)).collect();
    let mut port_contacts: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    if split {
        let mut order: Vec<usize> = (0..pins.len()).filter(|&p| !pins[p].contacts.is_empty()).collect();
        order.sort_by(|&a, &b| {
            (&pins[a].reference, &pins[a].no, pins[a].entity_id).cmp(&(&pins[b].reference, &pins[b].no, pins[b].entity_id))
        });
        for p in order {
            let list = port_contacts.entry(roots[pi + p]).or_default();
            for c in &pins[p].contacts {
                if !list.contains(c) {
                    list.push(c.clone());
                }
            }
        }
    }
    let slots = Slots::new(&roots, port_contacts);
    // 枠どうしを結ぶ。枠番号 slots.count + k が芯 k
    let mut sd = DisjointSet::new(slots.count + cores.len());
    let mut report = PortReport::default();

    // ピン p が入る枠と、そこで運ぶ接点
    let pin_slots = |p: usize| -> Vec<(usize, Option<String>)> {
        let node = &pins[p];
        let r = roots[pi + p];
        let Some(list) = slots.contacts.get(&r) else {
            return vec![(slots.first[&r], None)];
        };
        if !node.contacts.is_empty() {
            node.contacts.iter().filter_map(|c| slots.contact(r, c).map(|s| (s, Some(c.clone())))).collect()
        } else if node.pass {
            list.iter().enumerate().map(|(i, c)| (slots.first[&r] + i, Some(c.clone()))).collect()
        } else {
            vec![(slots.contact(r, &node.name).unwrap_or_else(|| slots.single_or_residual(r)), None)]
        }
    };

    // 芯 ⇔ 行き先 (割当てだけで結ぶ。区間の線はどの位置判定にも出てこない)
    for (k, (c, core)) in cores.iter().enumerate() {
        let me = slots.count + k;
        for end in [&core.end_a, &core.end_b].into_iter().flatten() {
            let target = match end {
                CoreEnd::Wire { wire } => match wires.iter().position(|w| w.id == *wire) {
                    Some(a) => {
                        let r = roots[wi + a];
                        if slots.is_port(r) {
                            stray(&mut report, c, core, "分岐の配線がポートのネットに触れています".into());
                        }
                        Some(slots.single_or_residual(r))
                    }
                    None => None,
                },
                // 同じ番号の接続点は内部短絡済みなので、最初の1つを見れば足りる (index は描画用)
                CoreEnd::Pin { part, pin, contact, .. } => match pins.iter().position(|n| n.entity_id == *part && n.no == *pin) {
                    None => None,
                    Some(p) => {
                        let node = &pins[p];
                        let r = roots[pi + p];
                        if !slots.is_port(r) {
                            Some(slots.first[&r])
                        } else if node.contacts.is_empty() && node.pass {
                            stray(&mut report, c, core, format!("{} はかみ合わせの片側なので、どの接点か決められません", node.label()));
                            Some(slots.single_or_residual(r))
                        } else if node.contacts.is_empty() {
                            Some(pin_slots(p)[0].0)
                        } else {
                            let want = contact.clone().unwrap_or_else(|| core.core.trim().to_string());
                            let found = if node.contacts.contains(&want) { slots.contact(r, &want) } else { None };
                            match found {
                                Some(s) => Some(s),
                                None => {
                                    let why = match contact {
                                        Some(x) => format!("接点 {x} は {} にありません (接点: {})", node.label(), node.contacts.join(", ")),
                                        None => format!(
                                            "{} はポート (接点: {}) ですが、芯に接点の指定が無く、芯の名前もどの接点でもありません",
                                            node.label(),
                                            node.contacts.join(", ")
                                        ),
                                    };
                                    stray(&mut report, c, core, why);
                                    Some(slots.single_or_residual(r))
                                }
                            }
                        }
                    }
                },
            };
            if let Some(s) = target {
                sd.union(me, s);
            }
        }
    }

    // 点付きラベル: 導体1本のかたまりの「L.接点」は、ラベル L のポートのかたまりのその接点の枠へ
    if !slots.contacts.is_empty() {
        let port_labels: Vec<(&str, usize)> = labels
            .iter()
            .enumerate()
            .filter(|(l, _)| slots.is_port(roots[li + l]))
            .map(|(l, lab)| (lab.name.as_str(), roots[li + l]))
            .collect();
        for (l, lab) in labels.iter().enumerate() {
            let r = roots[li + l];
            if slots.is_port(r) {
                continue;
            }
            for (name, pr) in &port_labels {
                for c in &slots.contacts[pr] {
                    if lab.name == format!("{name}.{c}") {
                        if let Some(s) = slots.contact(*pr, c) {
                            sd.union(slots.first[&r], s);
                        }
                    }
                }
            }
        }
    }

    // グループ化 (配線か区間ケーブルの芯を1つ以上含むグループだけをネットにする)
    #[derive(Default)]
    struct Group {
        wire_ids: Vec<EntityId>,
        pins: Vec<NetPin>,
        /// 名前の候補になるラベル (ポートの接点の枠では「L.接点」)。
        labels: BTreeSet<String>,
        /// グループ内のWireに書かれた生の線番 (重複可)。
        wire_nos: Vec<String>,
        /// 名前の候補になる線番 (ポートの接点の枠では「W.接点」)。
        name_wire_nos: Vec<String>,
        cable_cores: Vec<NetCableCore>,
        /// このまとまりのポートの枠: (かたまりの根, 接点の順位, 接点名。残りの枠は None)。
        ports: Vec<(usize, usize, Option<String>)>,
    }
    let mut groups: BTreeMap<usize, Group> = BTreeMap::new();
    let mut cluster_nos: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for (a, w) in wires.iter().enumerate() {
        let r = roots[wi + a];
        let no = w.net.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty());
        if let Some(no) = no {
            cluster_nos.entry(r).or_default().push(no.to_string());
        }
        for s in slots.every(r) {
            let g = groups.entry(sd.find(s)).or_default();
            g.wire_ids.push(w.id);
            if let Some(no) = no {
                g.wire_nos.push(no.to_string());
                if !slots.is_port(r) {
                    g.name_wire_nos.push(no.to_string());
                }
            }
        }
    }
    for (k, (c, core)) in cores.iter().enumerate() {
        groups.entry(sd.find(slots.count + k)).or_default().cable_cores.push(NetCableCore {
            cable_id: c.id,
            reference: c.reference.clone(),
            core: core.core.clone(),
        });
    }
    for p in 0..pins.len() {
        let node = &pins[p];
        for (s, contact) in pin_slots(p) {
            if let Some(g) = groups.get_mut(&sd.find(s)) {
                // 同一シンボル同一番号の接続点(内部短絡済み)は1エントリに集約
                if !g.pins.iter().any(|np| np.entity_id == node.entity_id && np.pin == node.no && np.contact == contact) {
                    g.pins.push(NetPin {
                        reference: node.reference.clone(),
                        entity_id: node.entity_id,
                        pin: node.no.clone(),
                        contact,
                    });
                }
            }
        }
    }
    for (l, lab) in labels.iter().enumerate() {
        let r = roots[li + l];
        match slots.contacts.get(&r) {
            None => {
                if let Some(g) = groups.get_mut(&sd.find(slots.first[&r])) {
                    g.labels.insert(lab.name.clone());
                }
            }
            Some(list) => {
                for (i, c) in list.iter().enumerate() {
                    if let Some(g) = groups.get_mut(&sd.find(slots.first[&r] + i)) {
                        g.labels.insert(format!("{}.{c}", lab.name));
                    }
                }
                if let Some(g) = groups.get_mut(&sd.find(slots.single_or_residual(r))) {
                    g.labels.insert(lab.name.clone());
                }
            }
        }
    }
    for (&r, list) in &slots.contacts {
        let base_no = pick_wire_no(cluster_nos.get(&r).map(Vec::as_slice).unwrap_or_default());
        for (i, c) in list.iter().enumerate() {
            if let Some(g) = groups.get_mut(&sd.find(slots.first[&r] + i)) {
                g.ports.push((r, i, Some(c.clone())));
                if let Some(w) = &base_no {
                    g.name_wire_nos.push(format!("{w}.{c}"));
                }
            }
        }
        if let Some(g) = groups.get_mut(&sd.find(slots.single_or_residual(r))) {
            g.ports.push((r, list.len(), None));
            if let Some(w) = &base_no {
                g.name_wire_nos.push(w.clone());
            }
        }
    }
    // 普通のピンも芯も無い残りの枠は出さない (共有の配線は接点のネット全部に入っている)
    groups.retain(|_, g| {
        let residual_only = !g.ports.is_empty() && g.ports.iter().all(|(_, _, c)| c.is_none());
        !(residual_only && g.pins.is_empty() && g.cable_cores.is_empty())
    });

    // ポートの報告
    if !slots.contacts.is_empty() {
        let mut port_pins_of: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (p, node) in pins.iter().enumerate() {
            let r = roots[pi + p];
            if slots.is_port(r) && !node.contacts.is_empty() {
                let list = port_pins_of.entry(r).or_default();
                if !list.iter().any(|&q| pins[q].entity_id == node.entity_id && pins[q].no == node.no) {
                    list.push(p);
                }
            }
        }
        for (l, lab) in labels.iter().enumerate() {
            if slots.is_port(roots[li + l]) {
                report.port_labels.insert(lab.name.clone());
            }
        }
        for (p, node) in pins.iter().enumerate() {
            let r = roots[pi + p];
            let Some(list) = slots.contacts.get(&r) else { continue };
            let label = node.label();
            if node.contacts.is_empty()
                && !node.pass
                && !list.contains(&node.name)
                && !report.plain_pins.iter().any(|x| x.entity_id == node.entity_id && x.pin == label)
            {
                report.plain_pins.push(PlainOnPort {
                    entity_id: node.entity_id,
                    pin: label,
                    ports: port_pins_of.get(&r).map(|v| v.iter().map(|&q| pins[q].label()).collect()).unwrap_or_default(),
                    contacts: list.clone(),
                });
            }
        }
        for (&r, list) in &port_pins_of {
            if list.len() < 2 {
                continue;
            }
            for c in &slots.contacts[&r] {
                let holders: Vec<usize> = list.iter().copied().filter(|&q| pins[q].contacts.contains(c)).collect();
                if let [only] = holders.as_slice() {
                    report.lonely_contacts.push(LonelyContact {
                        contact: c.clone(),
                        port: pins[*only].label(),
                        entity_ids: vec![pins[*only].entity_id],
                    });
                }
            }
        }
    }

    // 決定的順序: 配線のあるネットが先 (最小wire id)、芯だけのネットはその後ろ、同じかたまりの接点は接点の順
    let mut ordered: Vec<Group> = groups.into_values().collect();
    for g in &mut ordered {
        g.ports.sort();
    }
    ordered.sort_by_key(|g| {
        (
            g.wire_ids.is_empty(),
            g.wire_ids.iter().min().copied(),
            g.cable_cores.iter().map(|c| (c.cable_id, c.core.clone())).min(),
            g.ports.first().map_or(0, |p| p.1),
        )
    });
    let mut seq = 0;
    let mut port_base: BTreeMap<usize, String> = BTreeMap::new();
    let mut nets = Vec::with_capacity(ordered.len());
    for mut g in ordered {
        g.wire_ids.sort();
        g.pins.sort_by(|a, b| (&a.reference, &a.pin, &a.contact).cmp(&(&b.reference, &b.pin, &b.contact)));
        g.cable_cores.sort_by(|a, b| (a.cable_id, &a.core).cmp(&(b.cable_id, &b.core)));
        let wire_no = pick_wire_no(&g.wire_nos);
        let label = g.labels.iter().next().cloned();
        let name = match label.clone().or_else(|| pick_wire_no(&g.name_wire_nos)) {
            Some(n) => n,
            None => match g.ports.first() {
                Some((r, _, c)) => {
                    let base = match port_base.get(r) {
                        Some(b) => b.clone(),
                        None => {
                            seq += 1;
                            let b = format!("N{seq:03}");
                            port_base.insert(*r, b.clone());
                            b
                        }
                    };
                    match c {
                        Some(c) => format!("{base}.{c}"),
                        None => base,
                    }
                }
                None => {
                    seq += 1;
                    format!("N{seq:03}")
                }
            },
        };
        let contact = g.ports.iter().find_map(|(_, _, c)| c.clone());
        let contact_labels = if g.ports.is_empty() { Vec::new() } else { g.labels.iter().cloned().collect() };
        nets.push(Net {
            name,
            pins: g.pins,
            wire_ids: g.wire_ids,
            label,
            wire_no,
            cable_cores: g.cable_cores,
            contact,
            contact_labels,
        });
    }
    (nets, report)
}
```

Notes for the implementer:
- `pin_positions` stays as is (other modules use it). `build` uses `transform_local(pin.at, s)`, which is the same computation.
- For drawings without ports, the result must equal the old function. If an existing `netlist.rs` test fails, the new code is wrong, not the test.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test --workspace 2>&1 | grep -E "^test result|FAILED|panicked|error\[" | head -20`
Expected: all `test result: ok.`; the 8 new tests pass; no existing test changes its result.

- [ ] **Step 5: Regenerate the spec and commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check
git add src-tauri docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(netlist): ポートのネットを接点ごとに分け、分けない extract_wire_nets を残す"
```

---

### Task 4: Wire-centric consumers use `extract_wire_nets`; electrical checks accept a pin in several nets

**Files:**
- Modify: `src-tauri/crates/madake-core/src/wire_no.rs:11,60`
- Modify: `src-tauri/crates/madake-core/src/svg.rs:719`, `src-tauri/crates/madake-core/src/dxf.rs:1252`, `src-tauri/crates/madake-core/src/kicad.rs:779`, `src-tauri/crates/madake-core/src/tidy.rs:356`
- Modify: `src-tauri/crates/madake-core/src/reports.rs:7,475,565`
- Modify: `src-tauri/crates/madake-core/src/verify.rs:205-221,279-283` (`pin_net`), `:604` (`MATE_HALF` check)
- Test: `wire_no.rs`, `reports.rs`, `verify.rs` test modules

**Interfaces:**
- Consumes: Task 3 `extract_wire_nets`, `mate::is_mate_half`; Task 1 `port_fixtures`.
- Produces: no new API.

- [ ] **Step 1: Write the failing tests**

`wire_no.rs` `mod tests`:

```rust
    /// A wire between two USB ports carries four contact nets but gets one wire number, and the next wire gets the next number.
    /// 2つの USB ポートの間の配線は接点のネットを4つ運ぶが線番は1つで、次の配線には次の番号が付く。
    #[test]
    fn a_usb_wire_between_two_ports_gets_one_wire_number() {
        use crate::block::port_fixtures::{port_block, USB};
        let hub = Entity::Symbol(port_block("39", 100.0, 100.0, "right", "U3", &USB));
        let dongle = Entity::Symbol(port_block("LTE1", 200.0, 100.0, "left", "P1", &USB));
        // ブロックのピンは外形から 2.5mm 外 (右ピン = x+10、左ピン = x-10)
        let usb = wire(&[(110.0, 100.0), (190.0, 100.0)], None);
        let other = wire(&[(0.0, 150.0), (20.0, 150.0)], None);
        let (mut engine, sheet_id, ids) = engine_with(vec![hub, dongle, usb, other]);
        renumber(&mut engine, Some(sheet_id), RenumberMode::Renumber, 1);
        assert_eq!(net_of(&engine, sheet_id, ids[2]).as_deref(), Some("1"));
        assert_eq!(net_of(&engine, sheet_id, ids[3]).as_deref(), Some("2"));
    }
```

(First check the pin x positions with the Task 1 fixture: a one-pin block without a name is 15 mm wide, so its pin is 10 mm from the centre. If `pin_at` gives other values, use those numbers in the test.)

`reports.rs` `mod tests`:

```rust
    /// The wire list keeps one row for a wire between two USB ports, from port pin to port pin, with no contact in the text.
    /// 電線リストでは、2つの USB ポートの間の配線はポートのピンからポートのピンへの1行のまま (接点は書かない)。
    #[test]
    fn a_usb_wire_is_one_wire_list_row() {
        use crate::block::port_fixtures::{pin_at, port_block, USB};
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        let hub = Entity::Symbol(port_block("39", 100.0, 100.0, "right", "U3", &USB));
        let dongle = Entity::Symbol(port_block("LTE1", 200.0, 100.0, "left", "P1", &USB));
        let (h, d) = (hub.id(), dongle.id());
        sheet.entities.insert(h, hub);
        sheet.entities.insert(d, dongle);
        let (a, b) = (pin_at(sheet, h, "U3"), pin_at(sheet, d, "P1"));
        let w = wire_between((a.x, a.y), (b.x, b.y));
        sheet.entities.insert(w.id(), w);
        let rows = wire_list_rows(&project);
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0][1].as_str(), rows[0][2].as_str()), ("39:U3", "LTE1:P1"));
    }
```

`verify.rs` `mod tests`:

```rust
    /// Two USB ports joined by one wire have no unconnected pin and no other ERC finding.
    /// 配線1本で結んだ2つの USB ポートには、未接続ピンもほかの ERC の指摘も出ない。
    #[test]
    fn a_wired_usb_link_has_no_erc_findings() {
        use crate::block::port_fixtures::{pin_at, port_block, USB};
        let hub = Entity::Symbol(port_block("39", 100.0, 100.0, "right", "U3", &USB));
        let dongle = Entity::Symbol(port_block("LTE1", 200.0, 100.0, "left", "P1", &USB));
        let (h, d) = (hub.id(), dongle.id());
        let mut sheet = sheet_with(vec![hub, dongle]);
        let (a, b) = (pin_at(&sheet, h, "U3"), pin_at(&sheet, d, "P1"));
        let w = wire(&[(a.x, a.y), (b.x, b.y)]);
        sheet.entities.insert(w.id(), w);
        let erc: Vec<_> = run(&sheet).into_iter().filter(|d| d.code.starts_with("erc.")).collect();
        assert!(erc.is_empty(), "{erc:?}");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test -p madake-core a_usb_wire_between_two_ports a_usb_wire_is_one_wire_list_row a_wired_usb_link 2>&1 | grep -E "test |panicked" | head`
Expected: `a_usb_wire_between_two_ports_gets_one_wire_number` FAILS (the other wire gets `5`, not `2`). `a_usb_wire_is_one_wire_list_row` fails or passes depending on label resolution; it must pass after Step 3. `a_wired_usb_link_has_no_erc_findings` may pass already (it pins the behaviour).

- [ ] **Step 3: Implement**

- `wire_no.rs`: `use crate::netlist::extract_wire_nets;` and `let nets = extract_wire_nets(sheet, &sheet_symbol_defs(sheet));`.
- `svg.rs:719`, `dxf.rs:1252`, `kicad.rs:779`, `tidy.rs:356`: `crate::netlist::extract_netlist(` → `crate::netlist::extract_wire_nets(`.
- `reports.rs`: in the `use crate::netlist::{…}` line, replace `extract_netlist` with `extract_wire_nets`; line 565 `let nets = extract_wire_nets(sheet, &defs);`; doc line 475 `/// \`nets\`はこのシートの配線ごとのネットリスト ([\`crate::netlist::extract_wire_nets\`]) を呼び出し側から渡す`.
- `verify.rs` `electrical`:

```rust
    // (シンボルid, ピン番号) → ネット番号 (ポートのピンは接点ごとに複数のネットに入る)
    let mut pin_net: BTreeMap<(EntityId, &str), Vec<usize>> = BTreeMap::new();
    for (ni, net) in nets.iter().enumerate() {
        for p in &net.pins {
            pin_net.entry((p.entity_id, p.pin.as_str())).or_default().push(ni);
        }
    }
    // シンボルの接続先ネット一覧 (重複除去)
    let nets_of = |s: &crate::model::SymbolInstance, def: &SymbolDef| -> Vec<usize> {
        let mut out = BTreeSet::new();
        for pin in &def.pins {
            if let Some(v) = pin_net.get(&(s.id, pin.number.as_str())) {
                out.extend(v.iter().copied());
            }
        }
        out.into_iter().collect()
    };
```

and at the bridge loop:

```rust
                for no in group {
                    if let Some(v) = pin_net.get(&(s.id, no)) {
                        ns.extend(v.iter().copied());
                    }
                }
```

- `verify.rs` duplicate-reference kind: `let kind = if crate::mate::is_mate_half(&s.symbol_id) {`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test --workspace 2>&1 | grep -E "^test result|FAILED|panicked|error\[" | head -20`
Expected: all `test result: ok.`.

- [ ] **Step 5: Regenerate the spec and commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check
git add src-tauri docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(netlist): 線番・線番の描画・電線リストは配線ごとのネットを使い、電気検証はポートのピンを複数ネットで扱う"
```

---

### Task 5: Across sheets, label conflicts and `REF:PIN/CONTACT` display

**Files:**
- Modify: `src-tauri/crates/madake-core/src/xref.rs:371-390` (`extract_netlist_project`), `:481` (`xref_table_rows`), `:518` (`labels_on_net` → `pub(crate)`)
- Modify: `src-tauri/crates/madake-core/src/verify.rs:705-735` (label conflict loop, `let mut names = BTreeSet::new();` at 716)
- Modify: `src-tauri/crates/madake-cli/src/format.rs:363`
- Test: `xref.rs`, `verify.rs`, `format.rs` test modules

**Interfaces:**
- Consumes: Task 3 `Net.contact`, `Net.contact_labels`, `NetPin.contact`.
- Produces: `pub(crate) fn xref::labels_on_net(sheet: &Sheet, net: &Net) -> (Vec<EntityId>, BTreeSet<String>)` (Task 6 uses it).

- [ ] **Step 1: Write the failing tests**

`xref.rs` `mod tests`:

```rust
    /// A port net labelled USB_MIC on two sheets merges contact by contact across the sheets, never across contacts, and a dotted label USB_MIC.D+ on a third wire joins only D+.
    /// 2枚のシートでラベル USB_MIC が付いたポートのネットは、シートを跨いで接点ごとに統合され、別の接点とは統合されない。3本目の配線の点付きラベル USB_MIC.D+ は D+ にだけ繋がる。
    #[test]
    fn port_nets_merge_contact_by_contact_across_sheets() {
        use crate::block::port_fixtures::{pin_at, port_block, USB};
        let mut project = Project::new("t");
        project.sheets.push(Sheet::new("Sheet2", PaperSize::A3, Orientation::Landscape));
        let (s1, s2) = (project.sheets[0].id, project.sheets[1].id);
        for (sid, reference, side, no) in [(s1, "39", "right", "U3"), (s2, "LTE1", "left", "P1")] {
            let sheet = project.sheet_mut(sid).unwrap();
            let port = Entity::Symbol(port_block(reference, 100.0, 100.0, side, no, &USB));
            let id = port.id();
            sheet.entities.insert(id, port);
            let p = pin_at(sheet, id, no);
            let end = if side == "right" { p.x + 20.0 } else { p.x - 20.0 };
            for e in [wire(&[(p.x, p.y), (end, p.y)]), label("USB_MIC", end, p.y)] {
                sheet.entities.insert(e.id(), e);
            }
        }
        let sheet2 = project.sheet_mut(s2).unwrap();
        for e in [symbol("resistor", "R9", 150.0, 150.0), wire(&[(157.5, 150.0), (170.0, 150.0)]), label("USB_MIC.D+", 170.0, 150.0)] {
            sheet2.entities.insert(e.id(), e);
        }
        let nets = extract_netlist_project(&project).nets;
        let names: Vec<&str> = nets.iter().map(|n| n.name.as_str()).collect();
        for c in USB {
            let n = nets.iter().find(|n| n.name == format!("USB_MIC.{c}")).unwrap_or_else(|| panic!("{names:?}"));
            assert_eq!(n.members.len(), 2, "{c}");
        }
        let dplus = nets.iter().find(|n| n.name == "USB_MIC.D+").unwrap();
        assert!(dplus.members.iter().any(|m| m.net.pins.iter().any(|p| p.reference == "R9")));
        let vbus = nets.iter().find(|n| n.name == "USB_MIC.VBUS").unwrap();
        assert!(!vbus.members.iter().any(|m| m.net.pins.iter().any(|p| p.reference == "R9")));
        let rows = xref_table_rows(&project);
        assert!(rows.iter().any(|r| r[2].contains("39:U3/VBUS")), "{rows:?}");
    }
```

(The resistor's pin 2 is at `x + 7.5` for a resistor placed at `x`; `wire(&[(157.5, …)…])` starts on it. If it does not, use `crate::block::port_fixtures::pin_at` on the resistor.)

`verify.rs` `mod tests`:

```rust
    /// On a port net the label USB_MIC and a dotted label USB_MIC.VBUS on a tapped wire are one name, not a label conflict.
    /// ポートのネットでは、ラベル USB_MIC と、取り出した配線の点付きラベル USB_MIC.VBUS は同じ名前で、ラベルの競合にならない。
    #[test]
    fn a_dotted_label_is_not_a_label_conflict() {
        use crate::block::port_fixtures::{pin_at, port_block, USB};
        let hub = Entity::Symbol(port_block("39", 100.0, 100.0, "right", "U3", &USB));
        let h = hub.id();
        let mut sheet = sheet_with(vec![hub, symbol("resistor", "R1", 150.0, 150.0)]);
        let p = pin_at(&sheet, h, "U3");
        for e in [
            wire(&[(p.x, p.y), (p.x + 20.0, p.y)]),
            net_label("USB_MIC", p.x + 20.0, p.y),
            wire(&[(157.5, 150.0), (170.0, 150.0)]),
            net_label("USB_MIC.VBUS", 170.0, 150.0),
        ] {
            sheet.entities.insert(e.id(), e);
        }
        assert!(!run(&sheet).iter().any(|d| d.code == "erc.label_conflict"), "{:?}", run(&sheet));
    }
```

`madake-cli/src/format.rs` `mod tests`:

```rust
    /// madake netlist writes a port pin with its contact (39:U3/VBUS).
    /// madake netlist はポートのピンを接点付き (39:U3/VBUS) で書く。
    #[test]
    fn netlist_shows_port_contacts() {
        let nets = json!([{ "name": "N001.VBUS", "pins": [{ "reference": "39", "pin": "U3", "contact": "VBUS" }, { "reference": "R1", "pin": "1" }], "wire_ids": ["w1"] }]);
        let out = netlist(&nets);
        assert!(out.contains("39:U3/VBUS") && out.contains("R1:1"), "{out}");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test -p madake-core port_nets_merge a_dotted_label_is_not 2>&1 | grep -E "test |panicked" | head; cargo test -p madake-cli netlist_shows_port_contacts 2>&1 | grep -E "test |panicked"`
Expected: `port_nets_merge_contact_by_contact_across_sheets` FAILS (all contacts merge into one project net). `a_dotted_label_is_not_a_label_conflict` FAILS (`erc.label_conflict`). `netlist_shows_port_contacts` FAILS.

- [ ] **Step 3: Implement**

`xref.rs` `extract_netlist_project`, inside the per-net loop:

```rust
        for net in extract_netlist(sheet, &defs) {
            let (label_ids, names) = labels_on_net(sheet, &net);
            // ポートから分けたネットは「L.接点」の名前で統合する (配線を共有するので、位置で探すと全接点が1つになる)
            let names = if net.contact_labels.is_empty() { names } else { net.contact_labels.iter().cloned().collect() };
            members.push(NetMember {
```

`xref_table_rows`:

```rust
                for p in &m.net.pins {
                    pins.insert(match &p.contact {
                        Some(c) => format!("{}:{}/{c}", p.reference, p.pin),
                        None => format!("{}:{}", p.reference, p.pin),
                    });
                }
```

Change `fn labels_on_net` to `pub(crate) fn labels_on_net`.

`verify.rs` label-conflict loop, after the `for e in sheet.entities.values() { … }` that fills `names`:

```rust
        // ポートの接点のネットは、シートを跨ぐときと同じ名前 (「L.接点」) で比べる
        if !net.contact_labels.is_empty() {
            names = net.contact_labels.iter().map(String::as_str).collect();
        }
```

`format.rs` line 363:

```rust
                .map(|p| match p["contact"].as_str() {
                    Some(c) => format!("{}:{}/{c}", text(p, "reference"), text(p, "pin")),
                    None => format!("{}:{}", text(p, "reference"), text(p, "pin")),
                })
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test --workspace 2>&1 | grep -E "^test result|FAILED|panicked|error\[" | head -20`
Expected: all `test result: ok.`.

- [ ] **Step 5: Regenerate the spec and commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check
git add src-tauri docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(xref): ポートの接点のネットをシートを跨いで接点ごとに統合し、REF:PIN/接点 と表記する"
```

---

### Task 6: Port ERC (`port.rs`)

**Files:**
- Create: `src-tauri/crates/madake-core/src/port.rs`
- Modify: `src-tauri/crates/madake-core/src/lib.rs` (add `pub mod port;` after `pub mod pin_wiring;`)
- Modify: `src-tauri/crates/madake-core/src/verify.rs` (`verify_sheet_with` at 110-119, `verify_project` at 52-70)

**Interfaces:**
- Consumes: Task 3 `netlist::port_report`, `PortReport`; Task 5 `xref::labels_on_net`; `cable::CABLE_CORE_UNASSIGNED`.
- Produces: `pub const PORT_TO_PLAIN_PIN: &str = "erc.port_to_plain_pin"; pub const PORT_CONTACT_UNMATCHED: &str = "erc.port_contact_unmatched"; pub const PORT_LABEL_UNSPLIT: &str = "erc.port_label_unsplit"; pub fn check_ports(sheet: &Sheet) -> Vec<Diagnostic>; pub fn port_label_diagnostics(project: &Project) -> Vec<Diagnostic>`.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/crates/madake-core/src/port.rs` with only the test module first (the functions are added in Step 3):

```rust
//! ポート (接点を持つピン、[`crate::symbol::PinDef::contacts`]) の ERC。

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::port_fixtures::{pin_at, port_block, USB};
    use crate::geometry::Point;
    use crate::model::*;
    use uuid::Uuid;

    fn put(sheet: &mut Sheet, e: Entity) -> EntityId {
        let id = e.id();
        sheet.entities.insert(id, e);
        id
    }

    fn wire(a: Point, b: Point) -> Entity {
        Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![a, b],
            color: "black".into(),
            sq: 0.3,
            length_m: None,
            length_source: Default::default(),
            part_no: None,
            net: None,
        })
    }

    fn label(name: &str, at: Point) -> Entity {
        Entity::NetLabel(NetLabel { id: Uuid::new_v4(), at, name: name.into(), rotation: 0, attrs: Default::default() })
    }

    fn codes(diags: &[Diagnostic]) -> Vec<&str> {
        diags.iter().map(|d| d.code.as_str()).collect()
    }

    /// A plain pin wired to a port whose name matches no contact is a warning that names the pin, the port and its contacts; a pin named like a contact is not.
    /// ポートに繋がった普通のピンで、名前がどの接点とも合わないものは、ピン・ポート・接点を書いた警告になる。接点と同じ名前のピンは警告にならない。
    #[test]
    fn a_plain_pin_on_a_port_is_a_warning() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let hub = put(&mut sheet, Entity::Symbol(port_block("39", 100.0, 100.0, "right", "U3", &USB)));
        let jst = put(&mut sheet, crate::block::test_block("J45", "", 200.0, 100.0, r#"{"left":[{"no":"1"},{"no":"4","name":"GND"}]}"#));
        let h = pin_at(&sheet, hub, "U3");
        put(&mut sheet, wire(h, pin_at(&sheet, jst, "1")));
        put(&mut sheet, wire(h, pin_at(&sheet, jst, "4")));
        let diags = check_ports(&sheet);
        assert_eq!(codes(&diags), vec![PORT_TO_PLAIN_PIN]);
        assert_eq!(diags[0].severity, Severity::Warning);
        assert!(diags[0].message.contains("J45:1") && diags[0].message.contains("39:U3") && diags[0].message.contains("VBUS, D-, D+, GND"), "{}", diags[0].message);
    }

    /// A contact on only one of two joined ports is reported as info; a port wired only to a mate half gives no such info.
    /// 繋がった2つのポートのうち1つにしか無い接点は情報として知らせる。かみ合わせの片側にだけ繋いだポートでは出さない。
    #[test]
    fn a_contact_on_only_one_of_two_ports_is_reported_as_info() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let hub = put(&mut sheet, Entity::Symbol(port_block("39", 100.0, 100.0, "right", "U3", &USB)));
        let otg = put(&mut sheet, Entity::Symbol(port_block("OTG1", 200.0, 100.0, "left", "P1", &["VBUS", "D-", "D+", "GND", "ID"])));
        put(&mut sheet, wire(pin_at(&sheet, hub, "U3"), pin_at(&sheet, otg, "P1")));
        let diags = check_ports(&sheet);
        assert_eq!(codes(&diags), vec![PORT_CONTACT_UNMATCHED]);
        assert_eq!(diags[0].severity, Severity::Info);
        assert!(diags[0].message.contains("ID") && diags[0].message.contains("OTG1:P1"), "{}", diags[0].message);

        let mut alone = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let hub = put(&mut alone, Entity::Symbol(port_block("39", 100.0, 100.0, "right", "U3", &USB)));
        let socket = put(&mut alone, Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(), symbol_id: "connector_socket".into(), at: Point::new(150.0, 100.0),
            rotation: 0, mirror: false, reference: "J1".into(), value: String::new(), attrs: Default::default(),
        }));
        put(&mut alone, wire(pin_at(&alone, hub, "U3"), pin_at(&alone, socket, "1")));
        assert!(check_ports(&alone).is_empty(), "{:?}", check_ports(&alone));
    }

    /// A run-cable core whose contact is no longer on the port pin (the block was edited after) is reported as an unassigned core naming the contact.
    /// 区間ケーブルの芯の接点がポートのピンからなくなった (後でブロックを直した) ときは、その接点を書いた「芯の行き先なし」として知らせる。
    #[test]
    fn a_core_on_a_removed_contact_is_reported() {
        let mut lte = crate::cable::test_support::lte_sheet();
        let mut hub = port_block("39", 200.0, 100.0, "left", "U3", &["VBUS", "GND"]);
        hub.id = lte.usb;
        lte.sheet.entities.insert(lte.usb, Entity::Symbol(hub));
        let Entity::Cable(c) = lte.sheet.entities.get_mut(&lte.cable).unwrap() else { panic!() };
        for core in &mut c.cores {
            core.end_b = Some(CoreEnd::Pin { part: lte.usb, pin: "U3".into(), index: None, contact: Some(core.core.clone()) });
        }
        let diags = check_ports(&lte.sheet);
        let stray: Vec<&Diagnostic> = diags.iter().filter(|d| d.code == crate::cable::CABLE_CORE_UNASSIGNED).collect();
        assert_eq!(stray.len(), 2, "{diags:?}");
        assert!(stray.iter().any(|d| d.message.contains("D-") && d.message.contains("39:U3")), "{stray:?}");
        assert_eq!(stray[0].severity, Severity::Error);
    }

    /// A label that is a port label on one sheet but sits on a net without a port on another sheet is a project warning, because those sheets are not joined.
    /// あるシートではポートのラベルなのに、別のシートではポートの無いネットに載っているラベルは、シートが繋がらないのでプロジェクトの警告になる。
    #[test]
    fn a_port_label_without_a_port_on_another_sheet_is_a_warning() {
        let mut project = Project::new("t");
        project.sheets.push(Sheet::new("Sheet2", PaperSize::A3, Orientation::Landscape));
        let (s1, s2) = (project.sheets[0].id, project.sheets[1].id);
        let sheet1 = project.sheet_mut(s1).unwrap();
        let hub = put(sheet1, Entity::Symbol(port_block("39", 100.0, 100.0, "right", "U3", &USB)));
        let h = pin_at(sheet1, hub, "U3");
        put(sheet1, wire(h, Point::new(h.x + 20.0, h.y)));
        put(sheet1, label("USB_MIC", Point::new(h.x + 20.0, h.y)));
        let sheet2 = project.sheet_mut(s2).unwrap();
        put(sheet2, wire(Point::new(50.0, 50.0), Point::new(80.0, 50.0)));
        let l = put(sheet2, label("USB_MIC", Point::new(80.0, 50.0)));
        let diags = port_label_diagnostics(&project);
        assert_eq!(codes(&diags), vec![PORT_LABEL_UNSPLIT]);
        assert_eq!((diags[0].sheet_id, diags[0].entity_ids.clone()), (s2, vec![l]));
        assert!(crate::verify::verify_project(&project).iter().any(|d| d.code == PORT_LABEL_UNSPLIT));
    }
}
```

Add `pub mod port;` to `lib.rs`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test -p madake-core port:: 2>&1 | tail -10`
Expected: compile errors `cannot find function check_ports`, `cannot find value PORT_TO_PLAIN_PIN`.

- [ ] **Step 3: Implement** (above the test module in `port.rs`)

```rust
//! ポート (接点を持つピン、[`crate::symbol::PinDef::contacts`]) の ERC。
//!
//! 材料はネットリストが作る ([`crate::netlist::port_report`])。ここは文と重さを決めるだけ。

use std::collections::{BTreeMap, BTreeSet};

use crate::model::{Project, Sheet, SheetId};
use crate::symbol::sheet_symbol_defs;
use crate::verify::{Diagnostic, Severity};

/// ポートのネットにいる、接点の分からない普通のピン (警告)。
pub const PORT_TO_PLAIN_PIN: &str = "erc.port_to_plain_pin";
/// 繋がったポートのうち1つにしか無い接点 (情報)。
pub const PORT_CONTACT_UNMATCHED: &str = "erc.port_contact_unmatched";
/// あるシートではポートのラベルなのに、別のシートではポートの無いネットに載っているラベル (警告)。
pub const PORT_LABEL_UNSPLIT: &str = "erc.port_label_unsplit";

/// シート1枚のポートの ERC。
pub fn check_ports(sheet: &Sheet) -> Vec<Diagnostic> {
    let defs = sheet_symbol_defs(sheet);
    let report = crate::netlist::port_report(sheet, &defs);
    let diag = |severity: Severity, code: &str, message: String, ids: Vec<crate::model::EntityId>| Diagnostic {
        severity,
        code: code.into(),
        message,
        sheet_id: sheet.id,
        entity_ids: ids,
    };
    let mut out = Vec::new();
    for p in &report.plain_pins {
        out.push(diag(
            Severity::Warning,
            PORT_TO_PLAIN_PIN,
            format!(
                "{} は {} ({}) につながっていますが、どの接点か分かりません。ピン名を接点名にするか、connect_cable で芯ごとに引く",
                p.pin,
                p.ports.join(", "),
                p.contacts.join(", ")
            ),
            vec![p.entity_id],
        ));
    }
    for l in &report.lonely_contacts {
        out.push(diag(
            Severity::Info,
            PORT_CONTACT_UNMATCHED,
            format!("接点 {} は {} にしかありません (同じネットのほかのポートには無い)", l.contact, l.port),
            l.entity_ids.clone(),
        ));
    }
    for s in &report.stray_cores {
        out.push(diag(
            Severity::Error,
            crate::cable::CABLE_CORE_UNASSIGNED,
            format!("ケーブル {} の芯 {}: {}", s.reference, s.core, s.why),
            vec![s.cable_id],
        ));
    }
    out
}

/// プロジェクトのポートの ERC: ポートのラベルが、別のシートでポートの無いネットに載っていないか。
pub fn port_label_diagnostics(project: &Project) -> Vec<Diagnostic> {
    let mut port_sheets: BTreeMap<String, BTreeSet<SheetId>> = BTreeMap::new();
    for sheet in &project.sheets {
        let defs = sheet_symbol_defs(sheet);
        for name in crate::netlist::port_report(sheet, &defs).port_labels {
            port_sheets.entry(name).or_default().insert(sheet.id);
        }
    }
    let mut out = Vec::new();
    if port_sheets.is_empty() {
        return out;
    }
    for sheet in &project.sheets {
        let defs = sheet_symbol_defs(sheet);
        for net in crate::netlist::extract_netlist(sheet, &defs) {
            if net.contact.is_some() || !net.contact_labels.is_empty() {
                continue;
            }
            let (label_ids, names) = crate::xref::labels_on_net(sheet, &net);
            for name in names {
                let Some(on) = port_sheets.get(&name) else { continue };
                if on.iter().all(|s| *s == sheet.id) {
                    continue;
                }
                let ids: Vec<_> = label_ids
                    .iter()
                    .copied()
                    .filter(|id| matches!(sheet.entities.get(id), Some(crate::model::Entity::NetLabel(l)) if l.name.trim() == name))
                    .collect();
                out.push(Diagnostic {
                    severity: Severity::Warning,
                    code: PORT_LABEL_UNSPLIT.into(),
                    message: format!(
                        "ラベル {name} は別のシートではポートのネットですが、このシートのネットにはポートがありません。シートは繋がりません。ここにもポートを置くか、ラベル {name}.<接点> を使う"
                    ),
                    sheet_id: sheet.id,
                    entity_ids: ids,
                });
            }
        }
    }
    out
}
```

`verify.rs`: in `verify_sheet_with`, after `diags.extend(crate::cable::check_cables(sheet));`, add `diags.extend(crate::port::check_ports(sheet));`. In `verify_project`, after `diags.extend(crate::mate::mate_diagnostics(project));`, add:

```rust
    // ポートのラベルはシートを跨ぐので、プロジェクト全体で評価する
    diags.extend(crate::port::port_label_diagnostics(project));
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test --workspace 2>&1 | grep -E "^test result|FAILED|panicked|error\[" | head -20`
Expected: all `test result: ok.`.

- [ ] **Step 5: Regenerate the spec and commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check
git add src-tauri docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(verify): ポートの ERC (普通のピン・片側だけの接点・シートを跨ぐラベル・接点の無い芯)"
```

---

### Task 7: File format 18 and the `port_block` kind in files

**Files:**
- Modify: `src-tauri/crates/madake-core/src/model.rs` (`FORMAT_VERSION` doc + const at 11-56, `EntityRepr` at 586-660)
- Modify: `src-tauri/crates/madake-core/src/io.rs:36-41` (`save_project`), tests at `:383-391`

**Interfaces:**
- Consumes: Task 1 `BlockSpec::has_contacts`, `port_fixtures`.
- Produces: `pub const FORMAT_VERSION: u32 = 18;`, `EntityRepr::PortBlock(SymbolInstance)` (`#[schemars(skip)]`), `pub(crate) struct FileWriteGuard` with `FileWriteGuard::new()`.

- [ ] **Step 1: Write the failing tests** (`io.rs` `mod tests`; rename the old version test)

Replace `the_current_format_version_is_seventeen` with:

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

Append:

```rust
    fn port_project() -> (Project, EntityId) {
        use crate::block::port_fixtures::{port_block, USB};
        let mut project = Project::new("p");
        let hub = port_block("39", 100.0, 100.0, "right", "U3", &USB);
        let id = hub.id;
        project.sheets[0].entities.insert(id, Entity::Symbol(hub));
        (project, id)
    }

    fn save_text(project: &Project, tag: &str) -> (String, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("madake-io-port-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p.mdkproj");
        save_project(&path, project).unwrap();
        (std::fs::read_to_string(&path).unwrap(), path)
    }

    /// A block with port contacts is saved under the kind "port_block" and comes back as a symbol with its contacts, while the same project sent over IPC/MCP still says "symbol".
    /// ポートの接点を持つブロックは種類「port_block」で保存され、読み直すと接点付きのシンボルに戻る。同じプロジェクトを IPC・MCP で送るときは「symbol」のまま。
    #[test]
    fn a_port_block_is_saved_as_port_block() {
        let (project, id) = port_project();
        let (text, path) = save_text(&project, "kind");
        assert!(text.contains("\"kind\": \"port_block\""), "{text}");
        assert!(!text.contains("\"kind\": \"symbol\""));
        let loaded = load_project(&path).unwrap();
        let Entity::Symbol(s) = &loaded.sheets[0].entities[&id] else { panic!("シンボルに戻る") };
        assert!(crate::block::parse_block_spec(&s.attrs).unwrap().has_contacts());
        assert_eq!(serde_json::to_value(&project.sheets[0].entities[&id]).unwrap()["kind"], "symbol");
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    /// A project without port contacts is written byte for byte as serde writes it (no key reordering), with plain blocks still "symbol".
    /// ポートの接点の無いプロジェクトは、serde がそのまま書いたものと1バイトも違わない (キーの並びも変わらない)。普通のブロックは「symbol」のまま。
    #[test]
    fn a_project_without_ports_is_written_unchanged() {
        let mut project = Project::new("plain");
        let block = crate::block::test_block("U1", "", 50.0, 50.0, r#"{"left":[{"no":"1","name":"A"}]}"#);
        project.sheets[0].entities.insert(block.id(), block);
        let (text, path) = save_text(&project, "plain");
        assert_eq!(text, serde_json::to_string_pretty(&project).unwrap());
        assert!(!text.contains("port_block"));
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    /// A "port_block" entry without contacts (hand-edited) opens as a plain symbol.
    /// 接点の無い「port_block」(手で直したファイル) は普通のシンボルとして開く。
    #[test]
    fn a_port_block_without_contacts_opens_as_a_symbol() {
        let id = Uuid::new_v4();
        let json = format!(r#"{{"kind":"port_block","id":"{id}","symbol_id":"block","at":{{"x":0.0,"y":0.0}},"attrs":{{"block_pins":"{{\"left\":[{{\"no\":\"1\"}}]}}"}}}}"#);
        let e: Entity = serde_json::from_str(&json).unwrap();
        assert!(matches!(&e, Entity::Symbol(s) if s.symbol_id == "block"));
        assert_eq!(serde_json::to_value(&e).unwrap()["kind"], "symbol");
    }

    /// A block whose contacts were all removed again is saved with the kind "symbol", so older builds can open the file again.
    /// 接点をすべて消したブロックは種類「symbol」で保存されるので、古いビルドでもまたそのファイルを開ける。
    #[test]
    fn a_block_whose_contacts_were_removed_is_saved_as_symbol_again() {
        let (mut project, id) = port_project();
        let Entity::Symbol(s) = project.sheets[0].entities.get_mut(&id).unwrap() else { panic!() };
        s.attrs.insert("block_pins".into(), r#"{"right":[{"no":"U3"}]}"#.into());
        let (text, path) = save_text(&project, "removed");
        assert!(text.contains("\"kind\": \"symbol\"") && !text.contains("port_block"), "{text}");
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }
```

(If `EntityId` is not in scope in `io.rs` tests, it comes from `use crate::model::*;`, which is already there.)

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test -p madake-core io:: 2>&1 | grep -E "test |panicked" | head -20`
Expected: `the_current_format_version_is_eighteen` and `a_port_block_is_saved_as_port_block` FAIL. `a_port_block_without_contacts_opens_as_a_symbol` FAILS (`unknown variant port_block`).

- [ ] **Step 3: Implement**

`model.rs`, append to the version history doc (after the v17 bullet) and bump the constant:

```rust
/// - v18: ブロックのピンのポートの接点 ([`crate::symbol::PinDef::contacts`]) と、区間ケーブルの芯の
///   接点 (`CoreEnd::Pin::contact`)。接点のあるブロックはファイルの中だけ種類`port_block`で書く
///   ([`EntityRepr::PortBlock`]、IPC・MCP は`symbol`のまま) ので、v0.4.0 以前はそのファイルを開けない
///   (ブロックを黙って消さない)。形式17のビルドは版番号で断る。接点の無いファイルは版番号以外これまでと同じ
pub const FORMAT_VERSION: u32 = 18;
```

Add, above `EntityRepr`:

```rust
thread_local! {
    /// ファイルへ書いている最中か ([`crate::io::save_project`] だけが [`FileWriteGuard`] で立てる)。
    static WRITING_FILE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// 生きている間、エンティティを「ファイルの形」で書く: 接点のあるブロックを種類`port_block`にする。
/// IPC・MCP・Link API の JSON はこれを立てないので`symbol`のまま (フロントエンドと FreeCAD アドオンは
/// `kind == "symbol"`を見ている)。serde_json は preserve_order 無しなので、Value を経由して書き換えると
/// キーの並びが変わる。だから書き出しの途中でこの印を見る。
pub(crate) struct FileWriteGuard;

impl FileWriteGuard {
    pub(crate) fn new() -> Self {
        WRITING_FILE.with(|f| f.set(true));
        Self
    }
}

impl Drop for FileWriteGuard {
    fn drop(&mut self) {
        WRITING_FILE.with(|f| f.set(false));
    }
}

/// 接点を持つピンがあるブロックか (ファイルでは種類`port_block`)。
fn is_port_block(s: &SymbolInstance) -> bool {
    s.symbol_id == crate::block::BLOCK_SYMBOL_ID
        && crate::block::parse_block_spec(&s.attrs).is_ok_and(|spec| spec.has_contacts())
}
```

In `EntityRepr`, after `Dimension(Dimension),`:

```rust
    /// 接点を持つピンがあるブロック (v18)。ファイルへ書くときだけ使う種類で、読むと [`Entity::Symbol`]。
    #[schemars(skip)]
    PortBlock(SymbolInstance),
```

In `From<EntityRepr> for Entity`: `EntityRepr::Symbol(e) | EntityRepr::PortBlock(e) => Entity::Symbol(e),` (replace the `Symbol` arm). In `From<Entity> for EntityRepr`, replace the `Symbol` arm with:

```rust
            Entity::Symbol(e) if WRITING_FILE.with(|f| f.get()) && is_port_block(&e) => EntityRepr::PortBlock(e),
            Entity::Symbol(e) => EntityRepr::Symbol(e),
```

Update the doc comment of `EntityRepr` with one line: `/// 接点のあるブロックは、ファイルへ書くとき ([\`FileWriteGuard\`]) だけ種類\`port_block\`で書く (v18)。`

`io.rs`:

```rust
/// プロジェクトを整形JSONで保存する(git差分が読める形式)。接点のあるブロックは種類`port_block`で書く
/// ([`crate::model::FileWriteGuard`])。
pub fn save_project(path: &Path, project: &Project) -> Result<()> {
    let json = {
        let _file = crate::model::FileWriteGuard::new();
        serde_json::to_string_pretty(project)?
    };
    std::fs::write(path, json)?;
    Ok(())
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test --workspace 2>&1 | grep -E "^test result|FAILED|panicked|error\[" | head -20`
Expected: all `test result: ok.`. If a schemars test snapshots `EntityRepr`, it must not list `port_block`.

- [ ] **Step 5: Regenerate the spec and commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check
git add src-tauri docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(io): ファイル形式18。接点のあるブロックはファイルの中だけ種類 port_block で書く"
```

---

### Task 8: `connect_cable` lands a side on one port pin

**Files:**
- Modify: `src-tauri/crates/madake-core/src/cable_run.rs` (hook in `plan_side` after `let pins = side_pins(…)?;` at line 299, below the `labels` early return; new fn `port_side` after `plan_side`; tests)
- Test: `cable_run.rs`, `cable.rs` (`run_bars`)

**Interfaces:**
- Consumes: Task 1 `PinInfo.contacts`, `port_fixtures`; Task 2 `CoreEnd::Pin.contact`; Task 3 netlist.
- Produces: `fn port_side(spec: &CableRunSpec, name: &str, side: &CableSide, pins: &[(ResolvedPin, Option<u32>)]) -> Result<Option<SidePlan>, String>` (private).

- [ ] **Step 1: Write the failing tests** (`cable_run.rs` `mod tests`)

```rust
    use crate::block::port_fixtures::{port_block, USB};

    /// J45 (board, pins at x=57.5 facing right) and the hub port 39.U3 (one left pin).
    fn board_and_port() -> Sheet {
        let mut sheet = Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape);
        insert(&mut sheet, Entity::Symbol(connector(4, "J45", 50.0, 100.0, false)));
        insert(&mut sheet, Entity::Symbol(port_block("39", 210.0, 100.0, "left", "U3", &USB)));
        sheet
    }

    /// A side may land on one port pin: cores named like the contacts map by name, the line ends at the pin with no bar, and the netlist gives one net per core.
    /// 片側をポートのピン1本に着けられる。接点と同じ名前の芯は名前で対応し、1本線はそのピンで終わり (横棒なし)、ネットリストは芯ごとに1ネットになる。
    #[test]
    fn a_side_lands_on_one_port_pin_by_contact_name() {
        let mut sheet = board_and_port();
        let defs = sheet_symbol_defs(&sheet);
        let planned = plan_cable_run(&sheet, &defs, &spec(serde_json::json!({
            "cores": ["GND", "D-", "D+", "VBUS"],
            "a": {"part": "J45", "style": "multi"},
            "b": {"pins": ["39.U3"], "style": "single"}
        })))
        .unwrap();
        assert_eq!(planned.ends[0], ["J45.1".to_string(), "39.U3/GND".to_string()]);
        let port = crate::block::port_fixtures::pin_at(&sheet, planned.cable.cores[0].end_b.as_ref().unwrap().target(), "U3");
        assert_eq!(planned.cable.run.last().copied(), Some(port));
        for e in planned.entities() {
            insert(&mut sheet, e);
        }
        crate::cable::validate_run_cable(&sheet, &planned.cable).unwrap();
        assert!(crate::cable::run_bars(&sheet, &defs, &planned.cable).is_empty(), "1点に着くので横棒なし");
        let nets = crate::netlist::extract_netlist(&sheet, &defs);
        assert_eq!(nets.len(), 4);
        let gnd = nets.iter().find(|n| n.contact.as_deref() == Some("GND")).unwrap();
        assert!(gnd.pins.iter().any(|p| p.reference == "J45" && p.pin == "1"));
    }

    /// Breakout labels named <port label>.<contact> join each core to that contact of the port net, so a labels-side cable can feed a port drawn elsewhere on the sheet with no ERC finding.
    /// 「<ポートのラベル>.<接点>」という名前の分岐のラベルは、芯をポートのネットのその接点に繋ぐ。labels 側のケーブルで、シートの別の場所に描いたポートへ ERC の指摘なしに給電できる。
    #[test]
    fn breakout_labels_named_like_port_contacts_join_them() {
        let mut sheet = board_and_plug();
        let hub = insert(&mut sheet, Entity::Symbol(port_block("39", 100.0, 160.0, "right", "U3", &USB)));
        let h = crate::block::port_fixtures::pin_at(&sheet, hub, "U3");
        insert(&mut sheet, Entity::Wire(crate::cable::test_support::wire(&[(h.x, h.y), (h.x + 20.0, h.y)])));
        insert(&mut sheet, Entity::NetLabel(crate::model::NetLabel {
            id: Uuid::new_v4(), at: Point::new(h.x + 20.0, h.y), name: "USB_MIC".into(), rotation: 0, attrs: Default::default(),
        }));
        let defs = sheet_symbol_defs(&sheet);
        let dotted: Vec<String> = USB.iter().map(|c| format!("USB_MIC.{c}")).collect();
        let planned = plan_cable_run(&sheet, &defs, &label_spec(serde_json::json!({"labels": dotted}))).unwrap();
        for e in planned.entities() {
            insert(&mut sheet, e);
        }
        let nets = crate::netlist::extract_netlist(&sheet, &defs);
        let vbus = nets.iter().find(|n| n.name == "USB_MIC.VBUS").expect("USB_MIC.VBUS");
        let pins: Vec<String> = vbus.pins.iter().map(|p| format!("{}:{}", p.reference, p.pin)).collect();
        assert!(pins.contains(&"USB1:1".to_string()) && pins.contains(&"39:U3".to_string()), "{pins:?}");
        assert!(crate::port::check_ports(&sheet).is_empty(), "{:?}", crate::port::check_ports(&sheet));
    }

    /// Cores whose names are none of the contacts map by order when the counts match; a partial name match, a count mismatch or style multi on a port is refused, listing the contacts.
    /// 芯の名前がどれも接点でなく数が同じなら並び順で対応する。名前の一部だけの一致・数の違い・ポートへの style multi は、接点を並べて断る。
    #[test]
    fn port_landing_by_order_and_refusals() {
        let sheet = board_and_port();
        let defs = sheet_symbol_defs(&sheet);
        let plan = |cores: serde_json::Value, style: &str| {
            plan_cable_run(&sheet, &defs, &spec(serde_json::json!({
                "cores": cores, "a": {"part": "J45", "style": "multi"}, "b": {"pins": ["39.U3"], "style": style}
            })))
        };
        let by_order = plan(serde_json::json!(["1", "2", "3", "4"]), "single").unwrap();
        assert_eq!(by_order.ends[1][1], "39.U3/D-");
        let partial = plan(serde_json::json!(["VBUS", "DM", "DP", "GND"]), "single").unwrap_err();
        assert!(partial.contains("VBUS, D-, D+, GND"), "{partial}");
        let multi = plan(serde_json::json!(["VBUS", "D-", "D+", "GND"]), "multi").unwrap_err();
        assert!(multi.contains("single"), "{multi}");
        let subset = plan_cable_run(&sheet, &defs, &spec(serde_json::json!({
            "cores": ["VBUS", "GND"], "a": {"pins": ["J45.1", "J45.4"], "style": "multi"}, "b": {"pins": ["39.U3"], "style": "single"}
        })))
        .unwrap();
        assert_eq!(subset.ends[1][1], "39.U3/GND");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd src-tauri && cargo test -p madake-core a_side_lands_on_one_port_pin port_landing_by_order breakout_labels_named_like 2>&1 | grep -E "test |panicked" | head`
Expected: `a_side_lands_on_one_port_pin_by_contact_name` and `port_landing_by_order_and_refusals` FAIL with `b 側のピンが 1 本で、芯は 4 本です`. `breakout_labels_named_like_port_contacts_join_them` already PASSES (Task 3 dotted labels), which pins the interaction with `CableSide.labels`.

- [ ] **Step 3: Implement**

In `plan_side`, right after `let pins = side_pins(sheet, defs, name, side)?;`:

```rust
    // ポートのピン1本 (接点を持つピン) なら、芯を接点へ対応させてそのピンに着ける
    if let Some(port) = port_side(spec, name, side, &pins)? {
        return Ok(port);
    }
```

After `plan_side`:

```rust
/// 片側がポートのピン1本 (接点を持つピン) なら、芯を接点へ対応させて1本線をそのピンに着ける。
/// 芯の名前が全部接点なら名前で (一部の接点だけでもよい)、どれも接点でなく数が同じなら並び順で。
/// ポートでなければ None (いつもの決まりで計画する)。
fn port_side(
    spec: &CableRunSpec,
    name: &str,
    side: &CableSide,
    pins: &[(ResolvedPin, Option<u32>)],
) -> Result<Option<SidePlan>, String> {
    let [(r, index)] = pins else { return Ok(None) };
    let contacts = &r.pin.contacts;
    if contacts.is_empty() {
        return Ok(None);
    }
    let owner = if r.reference.is_empty() { r.entity_id.to_string() } else { r.reference.clone() };
    let pin_label = match index {
        Some(n) => format!("{owner}.{}#{n}", r.pin.number),
        None => format!("{owner}.{}", r.pin.number),
    };
    if side.style != SideStyle::Single {
        return Err(format!("{name} 側の {pin_label} はポート (接続点1つ) なので、style は single にする"));
    }
    let names: Vec<&str> = spec.cores.iter().map(|c| c.name().trim()).collect();
    let is_contact = |n: &&str| contacts.iter().any(|c| c == n);
    let chosen: Vec<String> = if names.iter().all(is_contact) {
        names.iter().map(|n| n.to_string()).collect()
    } else if !names.iter().any(is_contact) && names.len() == contacts.len() {
        contacts.clone()
    } else {
        return Err(format!(
            "{name} 側の {pin_label} はポート (接点 {}) です。芯の名前を接点の名前にそろえる (全部一致なら名前で対応) か、どれも一致させずに芯を接点と同じ {} 本にする (並び順に対応)",
            contacts.join(", "),
            contacts.len()
        ));
    };
    let ends = chosen
        .iter()
        .map(|c| CoreEnd::Pin { part: r.entity_id, pin: r.pin.number.clone(), index: *index, contact: Some(c.clone()) })
        .collect();
    let labels = chosen.iter().map(|c| format!("{pin_label}/{c}")).collect();
    Ok(Some(SidePlan {
        at: Point::new(r.pin.x, r.pin.y),
        dir: r.pin.dir,
        ends,
        wires: Vec::new(),
        net_labels: Vec::new(),
        labels,
    }))
}
```

`run_bars` already drops a bar whose two ends are the same point (`a.distance_to(&b) > 1e-6`), so no drawing change is needed. The test pins that.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test --workspace 2>&1 | grep -E "^test result|FAILED|panicked|error\[" | head -20`
Expected: all `test result: ok.`.

- [ ] **Step 5: Regenerate the spec and commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check
git add src-tauri docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(cable): connect_cable でケーブルの片側をポートのピン1本に着ける"
```

---

### Task 9: MCP descriptions, MCP tests, agent rule

**Files:**
- Modify: `src-tauri/crates/madake-mcp/src/lib.rs` (doc of `attrs` at 475-479, descriptions of `place_symbol` at 1240, `get_pins` at 1308, `connect_cable` at 1521, `get_netlist` at 1617)
- Create: `src-tauri/crates/madake-mcp/tests/port_tools.rs`
- Modify: `src-tauri/crates/madake-agent/src/knowledge.rs:62-64`, `src-tauri/crates/madake-agent/tests/knowledge.rs`

**Interfaces:**
- Consumes: Tasks 1-8 (all behaviour exists).
- Produces: nothing new.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/crates/madake-mcp/tests/port_tools.rs`:

```rust
//! 多接点ポート (ブロックのピンの contacts) の MCP ツールのテスト。

use madake_agent::tools::ToolBridge;
use madake_core::{Engine, Project};
use madake_mcp::tool_bridge::McpToolBridge;
use madake_mcp::{tool_descriptions, SharedDoc};
use serde_json::{json, Value};
use uuid::Uuid;

fn fixture() -> (SharedDoc, McpToolBridge) {
    let doc = SharedDoc::new(Engine::new(Project::new("ポート試験")));
    let parts = madake_mcp::open_parts(&std::env::temp_dir().join(format!("madake-port-{}.sqlite", Uuid::new_v4())))
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

const HUB: &str = r#"{"right":[{"no":"U3","name":"USB-A メス","contacts":["VBUS","D-","D+","GND"]}]}"#;
const DONGLE: &str = r#"{"left":[{"no":"P1","name":"USB-A オス","contacts":["VBUS","D-","D+","GND"]}]}"#;

async fn place(bridge: &McpToolBridge, sheet: Uuid, reference: &str, x: f64, pins: &str) -> Value {
    call(bridge, "place_symbol", json!({"sheet_id": sheet, "symbol_id": "block", "x": x, "y": 100.0, "reference": reference, "attrs": {"block_pins": pins}})).await
}

/// place_symbol and get_pins show a port pin's contacts, and connect_pins between two ports gives get_netlist one net per contact.
/// place_symbol と get_pins はポートのピンの接点を返し、2つのポートを connect_pins で結ぶと get_netlist は接点ごとに1ネットを返す。
#[tokio::test]
async fn two_ports_wired_by_connect_pins_give_one_net_per_contact() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    let hub = place(&bridge, sheet, "39", 100.0, HUB).await;
    assert_eq!(hub["pins"][0]["contacts"], json!(["VBUS", "D-", "D+", "GND"]));
    place(&bridge, sheet, "LTE1", 200.0, DONGLE).await;
    let pins = call(&bridge, "get_pins", json!({"sheet_id": sheet, "reference": "LTE1"})).await;
    assert_eq!(pins["parts"][0]["pins"][0]["contacts"][3], "GND");
    call(&bridge, "connect_pins", json!({"sheet_id": sheet, "from": "39.U3", "to": "LTE1.P1"})).await;
    let nets = call(&bridge, "get_netlist", json!({"sheet_id": sheet})).await;
    let nets = nets.as_array().unwrap();
    assert_eq!(nets.len(), 4, "{nets:?}");
    assert_eq!(nets[0]["contact"], "VBUS");
    assert_eq!(nets[0]["pins"][0]["contact"], "VBUS");
}

/// connect_cable lands a harness on one port pin ("39.U3"): cores map to the same-named contacts and each core is its own net.
/// connect_cable はハーネスをポートのピン1本 ("39.U3") に着ける。芯は同じ名前の接点に対応し、芯ごとに別のネットになる。
#[tokio::test]
async fn connect_cable_lands_on_a_port_pin() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    call(&bridge, "place_symbol", json!({"sheet_id": sheet, "symbol_id": "connector_4p", "x": 50.0, "y": 100.0, "reference": "J45"})).await;
    place(&bridge, sheet, "39", 210.0, r#"{"left":[{"no":"U3","contacts":["VBUS","D-","D+","GND"]}]}"#).await;
    let out = call(&bridge, "connect_cable", json!({"sheet_id": sheet, "cores": ["VBUS", "D-", "D+", "GND"],
        "a": {"part": "J45", "style": "multi"}, "b": {"pins": ["39.U3"], "style": "single"}})).await;
    assert_eq!(out["cores"][1]["b"], "39.U3/D-");
    let nets = call(&bridge, "get_netlist", json!({"sheet_id": sheet})).await;
    assert_eq!(nets.as_array().unwrap().len(), 4, "{nets}");
}

/// The tool descriptions tell the agent to draw a USB-like connector as one block pin with contacts and how to land a cable on it.
/// ツールの説明は、USB のようなコネクタを接点付きのブロックのピン1本で描くことと、そこへのケーブルの着け方をエージェントに伝える。
#[test]
fn the_tools_describe_port_contacts() {
    let tools = tool_descriptions();
    let get = |n: &str| tools.iter().find(|(t, _)| t == n).unwrap_or_else(|| panic!("{n}")).1.clone();
    assert!(get("place_symbol").contains("contacts"));
    assert!(get("get_pins").contains("contacts"));
    assert!(get("connect_cable").contains("ポート"));
    assert!(get("get_netlist").contains("接点"));
}
```

`madake-agent/tests/knowledge.rs`, append:

```rust
/// The system prompt tells the agent to draw a multi-conductor connector (USB) as one block pin with contacts.
/// システムプロンプトは、複数の導体を運ぶコネクタ (USB) を接点付きのブロックのピン1本で描くようエージェントに伝える。
#[test]
fn system_prompt_explains_port_contacts() {
    let prompt = system_prompt(&AppSettings::default(), None);
    assert!(prompt.contains("contacts") && prompt.contains("ポート"), "{prompt}");
}
```

(Use the imports already at the top of `tests/knowledge.rs`: `system_prompt` and `AppSettings` are used by the neighbouring tests.)

- [ ] **Step 2: Run the tests to verify they fail**

Run: `touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test -p madake-mcp --test port_tools 2>&1 | grep -E "test |panicked" ; cargo test -p madake-agent --test knowledge system_prompt_explains_port_contacts 2>&1 | grep -E "test |panicked"`
Expected: the two behaviour tests PASS (they pin behaviour from Tasks 1-8). `the_tools_describe_port_contacts` and `system_prompt_explains_port_contacts` FAIL.

- [ ] **Step 3: Implement**

In `madake-mcp/src/lib.rs`, append these exact sentences to the **end** of each description string (inside the quotes):

- `place_symbol`: `。USB・3.5mmオーディオ・DCジャックのように複数の導体を運ぶコネクタは、導体ごとのピンにせず block_pins のピンに contacts (例 {\"no\":\"U3\",\"name\":\"USB-A メス\",\"contacts\":[\"VBUS\",\"D-\",\"D+\",\"GND\"]}) を付けたポートのピン1本で描く`
- `get_pins`: `。ポートのピンは contacts (接点の並び) も返す`
- `connect_cable`: `。**ポートのピン1本に着けるとき**は pins にそのピンだけを書き (\"39.U3\"、style=single)、芯は同じ名前の接点に対応する (名前がどれも接点でなく数が同じなら並び順)`
- `get_netlist`: `。ポート (接点を持つピン) を含むネットは接点ごとに「<名前>.<接点>」のネットに分かれ、ピンに contact が付く`

Append to the `attrs` doc comment (after the `block_size` line):

```rust
    /// ピンに `contacts` (例 `["VBUS","D-","D+","GND"]`) を書くと、1本のピンで複数の導体を運ぶポートになる
    /// (ネットリストは接点ごとに分かれる)。
```

`knowledge.rs`, insert after the `connect_cable` bullet (after line 64, keep the trailing `\` style):

```
- **複数の導体を運ぶコネクタ(USB・3.5mmオーディオ・DCジャック)は、`contacts`を持つブロックのピン1本(ポート)にする**\
(例 `{\"no\":\"U3\",\"name\":\"USB-A メス\",\"contacts\":[\"VBUS\",\"D-\",\"D+\",\"GND\"]}`)。導体ごとにピンを分けない。\
2つのポートは`connect_pins`の配線1本で結ぶ(同じ名前の接点同士が繋がる)。ケーブルをポートに着けるときは`connect_cable`で\
`pins: [\"<参照記号>.<ポートのピン>\"]`と`style: single`。導体1本のピンは接点と同じ名前(`VBUS`・`GND`)にすればその接点に繋がる
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs && cd src-tauri && cargo test --workspace 2>&1 | grep -E "^test result|FAILED|panicked|error\[" | head -20`
Expected: all `test result: ok.`.

- [ ] **Step 5: Regenerate the spec and commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check
git add src-tauri docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(mcp): ポートの接点をツールの説明とエージェントの決まりに書く"
```

---

### Task 10: Frontend: contacts in the block editor, types, Properties summary

**Files:**
- Modify: `src/ipc.ts:192-194` (`CoreEnd`), `:582-587` (`PinDef`), `:716-733` (`NetPin`, `Net`)
- Modify: `src/canvas/dynamicSymbol.ts` (`BlockPin` near 251, constants near 245, `parseBlockSpec` loop at 474-480, `blockSymbol` push at 580)
- Modify: `src/canvas/blockEditor.ts` (`BlockPinRow` at 39-42, `BlockIssue` at 51-60, `isFilled` at 82-84, `pinsOf` at 120-132, `draftFromAttrs` at 160-162, `draftIssues` at 183-212; add `splitContacts`, `portPinCount`)
- Modify: `src/components/BlockPinColumn.vue`, `src/components/BlockPinsEditor.vue:24-36`, `src/components/PropertiesPanel.vue:468-475`
- Modify: `src/locales/{en,ja,zh,es,fr,de}.json` (`blockEditor` block)
- Test: `src/canvas/blockEditor.test.ts`, `src/canvas/dynamicSymbol.test.ts`, `src/canvas/cable.test.ts`

**Interfaces:**
- Consumes: the Rust JSON shapes from Tasks 1-3 (`PinDef.contacts`, `NetPin.contact`, `Net.contact`, `Net.contact_labels`, `CoreEnd` pin `contact`).
- Produces: `export const BLOCK_CONTACT_MAX = 32;` (dynamicSymbol.ts); `export function splitContacts(text: string): string[]`, `export function portPinCount(def: SymbolDef): number` (blockEditor.ts); `BlockPinRow.contacts?: string`; new `BlockIssue` codes `contactsTooFew { side, index }`, `duplicateContact { side, index, contact }`, `tooManyContacts { side, index, count, max }`.

- [ ] **Step 1: Write the failing tests**

`src/canvas/blockEditor.test.ts`, add to the imports `splitContacts, portPinCount` and append inside the top-level `describe`:

```ts
  // ja: 接点はカンマ (半角・読点・全角) で区切り、前後の空白を落として空のものを捨てる
  it("splits contacts on ASCII, Japanese and full-width commas and trims them", () => {
    expect(splitContacts(" VBUS 、D-，D+, GND ,")).toEqual(["VBUS", "D-", "D+", "GND"]);
    expect(splitContacts("")).toEqual([]);
  });

  // ja: 接点を書いた行は block_pins に contacts として入り、読み戻すとカンマ区切りの文字に戻る。接点の無い行の書き出しは今までと同じ
  it("writes contacts into block_pins and reads them back as comma-separated text", () => {
    const d = draft();
    d.rows.right[0].contacts = "VBUS, D-, D+, GND";
    const attrs = attrsFromDraft(d);
    const pins = JSON.parse(attrs.block_pins);
    expect(pins.right[0]).toEqual({ no: "OUT1", name: "出力1", contacts: ["VBUS", "D-", "D+", "GND"] });
    expect(pins.left[0]).toEqual({ no: "IN1", name: "入力1" });
    expect(draftFromAttrs(attrs).rows.right[0].contacts).toBe("VBUS, D-, D+, GND");
    expect(draftFromAttrs(attrs).rows.left[0]).toEqual({ no: "IN1", name: "入力1" });
    expect(portPinCount(draftSymbol(d)!)).toBe(1);
  });

  // ja: 接点が1つだけ・同じ接点が2つ・32を超える行は指摘になる
  it("flags a single contact, a repeated contact and too many contacts", () => {
    const d = draft();
    d.rows.left[0].contacts = "VBUS";
    d.rows.left[1].contacts = "A, A";
    d.rows.right[0].contacts = Array.from({ length: 33 }, (_, i) => `C${i}`).join(",");
    expect(draftIssues(d)).toEqual([
      { code: "contactsTooFew", side: "left", index: 0 },
      { code: "duplicateContact", side: "left", index: 1, contact: "A" },
      { code: "tooManyContacts", side: "right", index: 0, count: 33, max: 32 },
    ]);
  });
```

`src/canvas/dynamicSymbol.test.ts`, append inside `describe("dynamicSymbol", …)`:

```ts
  // ja: ブロックのピンの接点は Rust と同じく空白を落として PinDef に入り、1つだけの接点や重なる接点は同じ理由で断る
  it("copies block pin contacts into the pin definition and refuses bad contacts like Rust", () => {
    const def = blockSymbol({ block_pins: '{"right":[{"no":"U3","contacts":[" VBUS ","D-","D+","GND"]}],"left":[{"no":"1"}]}' });
    expect(def.pins.find((p) => p.number === "U3")!.contacts).toEqual(["VBUS", "D-", "D+", "GND"]);
    expect(def.pins.find((p) => p.number === "1")!.contacts).toBeUndefined();
    expect(() => blockSymbol({ block_pins: '{"left":[{"no":"P1","contacts":["A"]}]}' })).toThrow("at least two contacts");
    expect(() => blockSymbol({ block_pins: '{"left":[{"no":"P1","contacts":["A","A"]}]}' })).toThrow("duplicate contact A");
  });
```

`src/canvas/cable.test.ts`, append inside `describe("多芯ケーブル", …)`:

```ts
  // ja: 区間の片側の芯が全部1つのポートのピンに着くと、その側には横棒を描かない (Rust の run_bars と同じ)
  it("draws no bar when every core of a side lands on one port pin", () => {
    const s = lteSheet();
    const c = s.entities.w45 as Cable;
    c.cores = c.cores.map((k) => ({ ...k, end_b: { kind: "pin" as const, part: "usb", pin: "1", contact: k.core } }));
    expect(runBars(s, c, new Map())).toEqual([]);
  });
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npx vitest run src/canvas/blockEditor.test.ts src/canvas/dynamicSymbol.test.ts src/canvas/cable.test.ts 2>&1 | tail -20`
Expected: FAIL. `splitContacts` and `portPinCount` are not exported, there are no `contacts` on pins, and `CoreEnd` has no `contact` (type error in `vue-tsc`; Vitest may still run the cable test and pass it, which is fine because it pins existing behaviour).

- [ ] **Step 3: Implement**

`src/ipc.ts`:

```ts
export type CoreEnd =
  | { kind: "pin"; part: string; pin: string; index?: number | null; contact?: string | null }
  | { kind: "wire"; wire: string };
```

```ts
export interface PinDef {
  number: string;
  name: string;
  at: Point;
  dir?: PinDir;
  /** ポートの接点 (この1本のピンが運ぶ導体の名前)。普通のピンには無い。 */
  contacts?: string[];
}
```

```ts
export interface NetPin {
  reference: string;
  entity_id: string;
  pin: string;
  /** このネットで運ぶポートの接点 (例 "VBUS")。 */
  contact?: string | null;
}
```

and add to `Net`:

```ts
  /** ポートから分けたネットなら、その接点 (v18)。 */
  contact?: string | null;
  /** 分けたネットがシートを跨いで呼ばれる名前 ("USB_MIC.VBUS")。 */
  contact_labels?: string[];
```

`src/canvas/dynamicSymbol.ts`: after `BLOCK_PIN_MAX`:

```ts
/** 1本のピンが持てる接点の最大数 (Rust側 BLOCK_CONTACT_MAX)。 */
export const BLOCK_CONTACT_MAX = 32;
```

`BlockPin`:

```ts
export interface BlockPin {
  no: string;
  name?: string;
  /** ポートの接点 (Rust側 BlockPin::contacts)。 */
  contacts?: string[];
}
```

In `parseBlockSpec`, replace the pin loop with:

```ts
  const seen = new Set<string>();
  for (const pin of all) {
    const no = (pin.no ?? "").trim();
    if (!no) throw new Error("block pin number must not be empty");
    if (seen.has(no)) throw new Error(`duplicate block pin number: ${no}`);
    seen.add(no);
    checkContacts(no, pin.contacts ?? []);
  }
```

and add above `parseBlockSpec`:

```ts
/** ピンの接点を調べる (Rust側 check_contacts と同じ順・同じ文)。 */
function checkContacts(no: string, contacts: string[]): void {
  if (contacts.length === 0) return;
  if (contacts.length > BLOCK_CONTACT_MAX) {
    throw new Error(`block pin ${no}: too many contacts: ${contacts.length} (max ${BLOCK_CONTACT_MAX})`);
  }
  if (contacts.length < 2) {
    throw new Error(`block pin ${no}: a port needs at least two contacts (one conductor is a plain pin)`);
  }
  const seen = new Set<string>();
  for (const raw of contacts) {
    const c = raw.trim();
    if (!c) throw new Error(`block pin ${no}: contact names must not be empty`);
    if (seen.has(c)) throw new Error(`block pin ${no}: duplicate contact ${c}`);
    seen.add(c);
  }
}
```

In `blockSymbol`, replace `pins.push({ number: no, name, at, dir });` with:

```ts
      const contacts = (bp.contacts ?? []).map((c) => c.trim());
      pins.push(contacts.length ? { number: no, name, at, dir, contacts } : { number: no, name, at, dir });
```

`src/canvas/blockEditor.ts`: import `BLOCK_CONTACT_MAX` from `./dynamicSymbol` (add to the import list), then:

```ts
/** ピン表の1行。`contacts`はカンマ区切りの接点 (空・省略 = 普通のピン)。 */
export interface BlockPinRow {
  no: string;
  name: string;
  contacts?: string;
}
```

Add to `BlockIssue`:

```ts
  /** 接点が1つだけ (導体1本なら普通のピン)。 */
  | { code: "contactsTooFew"; side: BlockSide; index: number }
  /** 1本のピンで同じ接点が2回。 */
  | { code: "duplicateContact"; side: BlockSide; index: number; contact: string }
  /** 接点が多すぎる。 */
  | { code: "tooManyContacts"; side: BlockSide; index: number; count: number; max: number }
```

```ts
/** 接点の文字をカンマ (半角「,」・読点「、」・全角「，」) で区切り、前後の空白を落として空のものを捨てる。 */
export function splitContacts(text: string): string[] {
  return text
    .split(/[,、，]/)
    .map((s) => s.trim())
    .filter((s) => s !== "");
}

/** 定義のうち接点を持つピン (ポート) の本数。 */
export function portPinCount(def: SymbolDef): number {
  return def.pins.filter((p) => (p.contacts?.length ?? 0) > 0).length;
}
```

`isFilled`:

```ts
function isFilled(row: BlockPinRow): boolean {
  return row.no.trim() !== "" || row.name.trim() !== "" || (row.contacts ?? "").trim() !== "";
}
```

`pinsOf` map body:

```ts
      .map((row): BlockPin => {
        const name = row.name.trim();
        const pin: BlockPin = name ? { no: row.no.trim(), name } : { no: row.no.trim() };
        const contacts = splitContacts(row.contacts ?? "");
        if (contacts.length) pin.contacts = contacts;
        return pin;
      });
```

`draftFromAttrs` row mapping:

```ts
    rows[side] = (pins[side] ?? []).map((p) => ({
      no: p.no ?? "",
      name: p.name ?? "",
      ...(p.contacts?.length ? { contacts: p.contacts.join(", ") } : {}),
    }));
```

In `draftIssues`, inside the `forEach` after the `seen.add(no);` line:

```ts
      const contacts = splitContacts(row.contacts ?? "");
      if (contacts.length > BLOCK_CONTACT_MAX) {
        issues.push({ code: "tooManyContacts", side, index, count: contacts.length, max: BLOCK_CONTACT_MAX });
      } else if (contacts.length === 1) {
        issues.push({ code: "contactsTooFew", side, index });
      } else {
        const dup = contacts.find((c, i) => contacts.indexOf(c) !== i);
        if (dup) issues.push({ code: "duplicateContact", side, index, contact: dup });
      }
```

`BlockPinColumn.vue`: widen the `edit` patch type to `{ no?: string; name?: string; contacts?: string }`. Add a label `<span class="lbl">{{ t("blockEditor.colContacts") }}</span>` after the name label, and after the name `<input>` add:

```vue
      <input
        class="cell"
        :value="row.contacts ?? ''"
        :aria-label="t('blockEditor.colContacts')"
        :placeholder="t('blockEditor.contactsPlaceholder')"
        spellcheck="false"
        @input="edit(i, { contacts: ($event.target as HTMLInputElement).value })"
      />
```

`BlockPinsEditor.vue` `issueText`, before the final `return`:

```ts
  if (issue.code === "contactsTooFew") {
    return t("blockEditor.issue.contactsTooFew", { side: t(`blockEditor.side.${issue.side}`), row: issue.index + 1 });
  }
  if (issue.code === "duplicateContact") {
    return t("blockEditor.issue.duplicateContact", {
      side: t(`blockEditor.side.${issue.side}`),
      row: issue.index + 1,
      contact: issue.contact,
    });
  }
  if (issue.code === "tooManyContacts") {
    return t("blockEditor.issue.tooManyContacts", {
      side: t(`blockEditor.side.${issue.side}`),
      row: issue.index + 1,
      count: issue.count,
      max: issue.max,
    });
  }
```

`PropertiesPanel.vue` `blockPinCounts` (import `portPinCount` from `../canvas/blockEditor`):

```ts
  const counts = t("blockEditor.pinCounts", { left: count("left"), right: count("right") });
  const ports = portPinCount(def);
  return ports ? `${counts} · ${t("blockEditor.portCount", { count: ports })}` : counts;
```

Locales: add under `blockEditor` (next to `colName` / `namePlaceholder`, and under `issue`) in all six files:

| key | en | ja | zh | es | fr | de |
|---|---|---|---|---|---|---|
| `colContacts` | `Contacts` | `接点` | `触点` | `Contactos` | `Contacts` | `Kontakte` |
| `contactsPlaceholder` | `e.g. VBUS, D-, D+, GND` | `例: VBUS, D-, D+, GND` | `例如 VBUS, D-, D+, GND` | `p. ej. VBUS, D-, D+, GND` | `p. ex. VBUS, D-, D+, GND` | `z. B. VBUS, D-, D+, GND` |
| `portCount` | `{count} port(s)` | `ポート {count}` | `端口 {count}` | `{count} puerto(s)` | `{count} port(s)` | `{count} Port(s)` |
| `issue.contactsTooFew` | `Row {row} of the {side} has one contact: a port needs at least two` | `{side}の{row}行目は接点が1つです。ポートには2つ以上要ります` | `{side}第{row}行只有一个触点：端口至少需要两个` | `La fila {row} del {side} tiene un solo contacto: un puerto necesita al menos dos` | `La ligne {row} du {side} n'a qu'un contact : un port en demande au moins deux` | `Zeile {row} der {side} hat nur einen Kontakt: ein Port braucht mindestens zwei` |
| `issue.duplicateContact` | `Row {row} of the {side} lists contact {contact} twice` | `{side}の{row}行目で接点 {contact} が2回あります` | `{side}第{row}行的触点 {contact} 出现了两次` | `La fila {row} del {side} repite el contacto {contact}` | `La ligne {row} du {side} répète le contact {contact}` | `Zeile {row} der {side} nennt den Kontakt {contact} zweimal` |
| `issue.tooManyContacts` | `Row {row} of the {side} has too many contacts: {count} (at most {max})` | `{side}の{row}行目の接点が多すぎます: {count} (最大 {max})` | `{side}第{row}行触点过多：{count}（最多 {max}）` | `La fila {row} del {side} tiene demasiados contactos: {count} (máximo {max})` | `La ligne {row} du {side} a trop de contacts : {count} (au plus {max})` | `Zeile {row} der {side} hat zu viele Kontakte: {count} (höchstens {max})` |

(Do not use `|` in these strings: the i18n test treats `|` as plural forms.)

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npx vitest run 2>&1 | tail -8 && npx vue-tsc --noEmit 2>&1 | tail -5`
Expected: Vitest `Test Files … passed`, including `src/i18n/i18n.test.ts` (key parity and placeholders). `vue-tsc` prints nothing. If `src/canvas/terminals.test.ts` times out, re-run it alone: `npx vitest run src/canvas/terminals.test.ts`.

- [ ] **Step 5: Regenerate the spec and commit**

```bash
python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check
git add src docs/13-specification.md docs/13-specification.ja.md
git commit -m "feat(ui): ブロックのピン表に接点の列を足し、プロパティにポート数を出す"
```

---

### Task 11: User docs and final verification

**Files:**
- Modify: `docs/05-wire-management.md` and `.ja.md` (new bullet after the "Single-line cables" bullet, line 18)
- Modify: `docs/10-automation-api.md` and `.ja.md` (new line after the "Drawing edits that have no dedicated tool…" / 「専用ツールが無い編集は…」 line, line 12)

**Interfaces:**
- Consumes: everything above.
- Produces: docs only.

- [ ] **Step 1: Write the docs**

`docs/05-wire-management.md`, after line 18:

```markdown
- **Port pins (multi-contact pins)**: a block pin can carry several named conductors, its **contacts** (for example a USB-A socket `U3` with `VBUS, D-, D+, GND`). It is drawn as one pin, and the netlist splits every net that holds a port into one net per contact, named `<net>.<contact>` (`USB_MIC.VBUS`, `N003.D+`). One wire between two ports connects same-named contacts. A plain pin joins the contact with its pin name (`GND`). A mate half (plug or socket) passes every contact through, but plug and socket are still not joined to each other. A label `<label>.<contact>` on another wire taps one contact, also across sheets. Wire numbers, the wire list and the BOM still count the drawn wire once. ERC adds `erc.port_to_plain_pin` (a plain pin on a port whose name matches no contact), `erc.port_contact_unmatched` (a contact on only one of the joined ports) and `erc.port_label_unsplit` (a port label used on another sheet without a port). A run cable can end on one port pin (`connect_cable` with `pins: ["39.U3"]`). Edit contacts in the block pin table (comma separated). A file with port pins is `.mdkproj` format 18; blocks with contacts are saved as the entity kind `port_block`, so older apps refuse the file
```

`docs/05-wire-management.ja.md`, after line 18:

```markdown
- **ポートのピン(多接点のピン)**: ブロックのピンは、名前の付いた複数の導体(**接点**)を運べる(例: USB-A ソケット `U3` に `VBUS, D-, D+, GND`)。図面ではピン1本で描き、ネットリストはポートを含むネットを接点ごとのネット `<ネット名>.<接点>`(`USB_MIC.VBUS`・`N003.D+`)に分ける。2つのポートの間の配線1本は、同じ名前の接点同士を繋ぐ。普通のピンはピン名と同じ接点(`GND`)に入る。かみ合わせの片側(プラグ・ソケット)は全部の接点を素通しにするが、プラグとソケットは今も互いには繋がらない。別の配線に `<ラベル>.<接点>` のラベルを置くと接点1つを取り出せる(シートを跨いでも)。線番・電線リスト・部品表は、描いた配線を今までどおり1本と数える。ERC は `erc.port_to_plain_pin`(名前がどの接点とも合わない、ポートの上の普通のピン)・`erc.port_contact_unmatched`(繋がったポートの1つにしか無い接点)・`erc.port_label_unsplit`(別のシートでポートの無いネットに使われたポートのラベル)を出す。区間ケーブルはポートのピン1本で終われる(`connect_cable` の `pins: ["39.U3"]`)。接点はブロックのピン表で編集する(カンマ区切り)。ポートのピンを含むファイルは `.mdkproj` 形式18で、接点のあるブロックは種類 `port_block` で保存するので、古いアプリはそのファイルを開けない
```

`docs/10-automation-api.md`, after line 12:

```markdown
Port pins: `place_symbol` with `attrs.block_pins` pins that carry `contacts` (`{"no":"U3","name":"USB-A メス","contacts":["VBUS","D-","D+","GND"]}`) makes one pin carry several conductors; `get_pins` returns `contacts`; `get_netlist` splits such nets into `<name>.<contact>` with `contact` on the net and on port pins; `connect_cable` lands a side on one port pin with `pins: ["39.U3"]` and `style: single` (cores map to same-named contacts, or by order when no name matches).
```

`docs/10-automation-api.ja.md`, after line 12:

```markdown
ポートのピン: `place_symbol` の `attrs.block_pins` でピンに `contacts`(`{"no":"U3","name":"USB-A メス","contacts":["VBUS","D-","D+","GND"]}`)を付けると、1本のピンが複数の導体を運ぶ。`get_pins` は `contacts` を返し、`get_netlist` はそのネットを `<名前>.<接点>` に分け、ネットとポートのピンに `contact` を付ける。`connect_cable` は `pins: ["39.U3"]` と `style: single` で片側をポートのピン1本に着ける(芯は同じ名前の接点に、名前がどれも合わなければ並び順に対応する)。
```

- [ ] **Step 2: Run the full verification**

Run, from the worktree root:

```bash
touch src-tauri/crates/madake-core/src/lib.rs src-tauri/crates/madake-mcp/src/lib.rs src-tauri/crates/madake-agent/src/lib.rs
(cd src-tauri && cargo test --workspace 2>&1 | grep -E "^test result|FAILED|panicked|error\[")
npx vitest run 2>&1 | tail -5
npx vue-tsc --noEmit
python3 scripts/gen_spec.py --check
```

Expected: every `test result: ok.`; Vitest all files passed; `vue-tsc` silent; `spec docs up to date; all tests have bilingual spec comments`.

- [ ] **Step 3: Commit**

```bash
git add docs/05-wire-management.md docs/05-wire-management.ja.md docs/10-automation-api.md docs/10-automation-api.ja.md
git commit -m "docs: ポートのピン (多接点) を配線と自動化の文書に書く"
```
