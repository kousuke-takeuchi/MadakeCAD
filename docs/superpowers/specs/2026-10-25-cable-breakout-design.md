# Cable breakout: one line on one end, separate cores on the other

Date: 2026-10-25 · Status: decisions made by the user, awaiting spec review · Japanese: [2026-10-25-cable-breakout-design.ja.md](2026-10-25-cable-breakout-design.ja.md)

## Goal

Let a drawn cable be **one line for part of its run and separate core wires for the rest**, while every core keeps its own connection from end to end. The netlist, From-To wire list, BOM, cable list, ERC and verification must see each core as its own conductor.

The AOGU wiring diagram needs this today:

| Cable | Single-line end | Multi-line end |
|---|---|---|
| LTE harness 45/46 | USB Type-A plug/socket | Board JST XH 4P: VBUS / D− / D+ / GND as 4 wires |
| Debug cable 58 | 3.5 mm TRS (AUDIO) | Dupont to Raspberry Pi header pins 8 / 10 / 6 as 3 wires |
| Amp power 54 | USB | Amp screw terminal: 5V / GND as 2 wires |

A second, smaller case: a cable drawn as **one line along its whole length** between two multi-pin connectors (for example a 3.5 mm AUX cable, L / R / GND), still with per-core connectivity.

Success: the LTE harness can be drawn as 4 board-side wires that merge into one line running to the USB plug. `get_netlist` then returns 4 separate nets (`J45:1–USB1:1`, `J45:2–USB1:2`, …), ERC reports no short, unconnected pin or dangling wire, and the wire list has one row per core. An AI agent can draw the whole thing with one MCP call.

## The problem today

The netlist (`madake-core/src/netlist.rs`, `extract_netlist`) connects things by position:

- wires join only where their **end points** meet (or at a junction dot);
- a symbol pin or a net label joins a wire when it sits on **any point** of the wire (within `CONNECT_EPS` = 0.01 mm), not only at its ends.

So if the USB side of the LTE harness is drawn as one line, there is nowhere for it to go that is correct:

- If the line runs along the pin column of `connector_4p` (all 4 pins sit at x = +7.5 mm, 5 mm apart), it touches all 4 pins and the netlist **shorts VBUS, D−, D+ and GND into one net**.
- If the 4 board-side wires end where the single line starts, they share an end point and are also joined into one net.
- The workaround used in the AOGU drawing is a fake "breakout" block (1 pin in, N pins out). ERC is quiet, but the netlist now stops at the block: there is no link from `J45:2` (D−) to `USB1:2`. The wire list, verification and any future harness check cannot follow a core through the cable.

`ERC` cannot catch the short either: no check looks for two cores of one cable landing in the same net.

## What exists today

- **`Entity::Cable`** (`model.rs`, format 15) is the EPLAN-style *cable definition*: a slanted **definition line** (`points`, exactly 2) drawn across the wires of the cable. `add_cable` assigns each crossed wire to a core in order (`CableCore { core, color, wire }`). It has `reference` (W1…), `part_no`, `core_count`, `sq`, `shield`, `length_m`. It feeds the cable list (`cable.rs`, `cable_list_rows`), the cable BOM rows (`cable_bom_rows`) and one ERC (`erc.cable_core_overflow`). **It is invisible to the netlist** (`Entity::Cable(_) => {}` in `extract_netlist`): every core is still drawn as its own `Wire`.
- **`Entity::Harness`** only draws a dashed box and names the wires inside it.
- **Netlist** is per sheet; `xref::extract_netlist_project` merges sheets by net label. About 20 call sites use `extract_netlist` (ERC, SPICE/simulation, KiCad netlist export, wire numbers, cross references, PLC, MCP `get_netlist`, Link API). A change at the netlist level reaches all of them.
- **Nets need a wire.** `extract_netlist` only turns a group into a net when it holds at least one `Wire`. A pin-to-pin connection without a wire does not exist today.
- **`erc.unconnected_pin`** reads the netlist; **`erc.dangling_wire`** is a separate geometric check on wire end points.
- **Wire list** (`reports.rs`, `wire_list_rows`) is one row per `Wire` entity; `wire_from_to` resolves each end to a pin or follows a net label (G18).
- **BOM wire roll-up** (`wire_rollup_rows`) adds up every `Wire`'s length by wire part number, including wires that are cores of a cable (so a cable's cores are counted once as the cable row and again as loose wire).
- **MCP**: `connect_pins` draws one or many pin-to-pin wires in one undo step (`connections`); `place_mate` places plug/socket pairs; `execute_commands` runs raw `Command`s (including `add_cable`). Pin positions and routes come from `pin_wiring.rs` (`resolve_pin`, `plan_connection`, `route`).
- **File format** is 16. `io::migrate` only raises the version number. `load_project` does **not** refuse a file newer than the app, and serde ignores unknown fields on existing structs. A new field added to `Cable` would be **silently dropped** by an older build, and lost on its next save.
- `Entity::refs` / `remap_refs` list in-sheet references (today: leader target, cable core wires, nameplate, dimension). Sheet duplication (variants) and macros depend on them.

## How other tools do it

Only what could be checked is stated; the rest is marked.

- **EPLAN** separates the *cable* from its *drawing*. A cable is defined with a cable definition line over its connections or with connection definition points, and its representation type is single-line or multi-line, taken from the page type ([cable definition](https://www.eplan.help/en-us/infoportal/content/Plattform/2.9/Content/htm/cablegui_k_arbeitsweise.htm)). On single-line pages the number of conductors is a property shown at the connection definition point, "usually shown as a forward slash", and values like `3+PE` are allowed. The change between multi-line and single-line is done by **bundle connection points**: connections are "fed into a bundle consisting of a single thick connection line" and split back before the target ([special features of cables](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/singlepole_k_besonderheitenkabel.htm)). Each connection in a bundle is identified by a **bundle connection point designation**, stored in pairs on the incoming and outgoing bundle points, and the designations can be overwritten with the cable's core numbers or colours ([bundles in single-line representation](https://www.eplan.help/en-us/infoportal/content/Plattform/2026/Content/htm/singlepole_k_straenge_in_einpoligerdarstellung.htm)). So in EPLAN the core identity across the single line comes from **matching designations**, not from geometry. Not verified: how EPLAN assigns designations automatically when a bundle is first drawn.
- **KiCad** buses: a vector bus `D[0..7]` or a group bus `USB1{DP DM}` (members become nets `USB1.DP`, `USB1.DM`), plus bus aliases. Bus entries "are graphical only, and are not necessary to form logical connections"; connections "are made by the labels placed on wires connected to the pins", and a pin cannot connect to a bus directly ([KiCad 8 Schematic Editor manual](https://docs.kicad.org/8.0/en/eeschema/eeschema.html)). Core identity comes from **net-label names**.
- **Harness tools** (Zuken E3.cable, Siemens Capital, EPLAN Harness proD): known to model bundles and cable cores in harness drawings, but not checked for this spec. Not used as evidence.

Common idea: the single line is a **carrier with a list of named members**, and each member is tied to its wire or pin by an **explicit name or assignment**, never by touching. MadakeCAD should do the same: explicit core-to-pin / core-to-wire assignments, owned by the cable.

## Options

### A. A new `Bundle` entity (EPLAN bundle)

A new entity: a polyline with an ordered list of members and two ends. Each end is either a breakout (members continue as wires) or lands on a part (members go to pins). A bundle may carry cores of one cable, of several cables, or plain wires.

- Good: closest to EPLAN; a bundle is pure drawing, the cable stays a part. A new `Entity` variant makes **older builds refuse the file** (unknown tag), so nothing is silently lost.
- Bad: two entities to describe one physical cable (the cable row in the BOM still needs `Cable`, the drawing needs `Bundle`); the user and the agent must keep them in sync. One more entity in the ~15 `match Entity` blocks across the code base. Members of several cables in one line is not needed by any AOGU case.

### B. Extend `Cable` with a single-line run and per-core end assignments (recommended)

The existing cable gets an optional **run** (the single line) and each core gets explicit **ends**: a pin of a part, or one of the core's own wires (breakout). The netlist joins each core's ends explicitly.

- Good: one thing per physical cable. Reference, part number, core count, gauge, shield and length already exist and already flow into the cable list and BOM. Matches EPLAN's "same cable, single-line representation". The agent creates one object.
- Bad: older builds silently drop the new fields (see "File format"). A run cannot carry cores of two different cables (not needed now; Option A could be added later for that).

### C. A netlist-level "bus wire" (KiCad bus)

A `Wire` gets a member list; wires touching it pick their member by their net label.

- Good: smallest model change.
- Bad: every core wire needs a label; a pin cannot sit on the bus, so the USB side (single line straight into the plug) cannot be drawn; a wrong or missing label silently splits or merges nets; no link to the cable part, BOM or cable list. Hard for an agent to get right.

### Recommendation

**Option B.** It fixes the real problem (per-core connectivity through a single line) with explicit assignments, keeps one object per physical cable, and reuses the cable list, BOM rows and Properties panel that already exist. Option A stays possible later if a bundle ever has to carry several cables.

## Design (Option B)

### 1. Data model (`model.rs`, format 17)

In memory this is one `Cable` struct. On disk, a cable that has a `run` is written with the entity kind `run_cable` and one without keeps `cable` (see §6: old builds must fail to load run cables rather than drop them).

```rust
pub struct Cable {
    // ... existing fields unchanged: id, reference, part_no, core_count, sq,
    //     points (definition line), cores, shield, length_m
    /// Single-line run (the drawn "one line"), in paper mm. Empty = a classic
    /// definition-line cable. Orthogonal polyline like a wire.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub run: Vec<Point>,
}

pub struct CableCore {
    pub core: String,          // name: "1", "VBUS", "D-", "L" ...
    pub color: String,
    pub wire: Option<EntityId>, // existing: wire crossed by the definition line
    /// Where the core leaves the run at its first point (run[0]) and last point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_a: Option<CoreEnd>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_b: Option<CoreEnd>,
}

#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CoreEnd {
    /// The run lands on a part and this core is this pin
    /// (pin number; `index` picks the n-th connection point for duplicate numbers, like "TB1.1#2").
    Pin { part: EntityId, pin: String, #[serde(default)] index: Option<u32> },
    /// Breakout: the core continues as this wire. The wire's end nearer the run end
    /// is the breakout end; its far end is wired as usual.
    Wire { wire: EntityId },
}
```

Rules (checked by `Engine::execute`, like today's cable validation):

- A cable is either a **definition-line cable** (`points` has 2 points, `run` empty, no `end_a`/`end_b`) or a **run cable** (`run` has 2+ points, `points` may be empty). Today's check "exactly two points" becomes "two points, or a run".
- In a run cable every core has both ends set; each `Pin` points at a symbol on the sheet with that pin; each `Wire` points at a wire on the sheet; one wire or pin is used by at most one core.
- `Entity::refs` / `remap_refs` include `CoreEnd` targets, so sheet duplication, variants and macros keep the assignments.
- Deleting a part or wire that a core uses leaves the end pointing at nothing; ERC reports it (below), like a cable core whose wire was deleted today.

The run's points move with the cable (`translate`). The run is drawn and hit-tested, but it is **never a `Wire`**, so no pin, label or wire end ever connects to it by position.

### 2. Netlist rule (`netlist.rs`)

Add one node per core of each run cable. Then:

- core node ↔ every connection point of `Pin { part, pin }` (all points with that pin number, or only the `index`-th one);
- core node ↔ the wire of `Wire { wire }`;
- nothing touches the run geometrically.

A group becomes a net if it holds **at least one wire or one cable core** (today: at least one wire), so a connector-to-connector single-line cable (AUX L / R / GND) gets 3 nets with no `Wire` at all.

`Net` gains an additive field `cable_cores: Vec<NetCableCore { cable_id, reference, core }>` (serde default, so the IPC and MCP JSON only grow). Net naming is unchanged (label, then wire number, then `N001…`); a core-only net is ordered by its cable id after wire-bearing nets, so existing net names do not move.

Because every netlist consumer goes through `extract_netlist`, ERC `erc.unconnected_pin`, SPICE, cross references, KiCad netlist export and `get_netlist` all see the cores without further change.

### 3. ERC (`cable.rs` `check_cables`, `verify.rs`)

| Code | Severity | When |
|---|---|---|
| `erc.cable_cores_shorted` (new) | Error | Two cores of one cable end up in the same net. Also checked for definition-line cables (their core wires). This catches the short that the fake breakout block hides today. |
| `erc.cable_core_unassigned` (new) | Error | A run cable core has no `end_a`/`end_b`, or the end points at a deleted part, missing pin or deleted wire. |
| `erc.cable_run_end_detached` (new) | Warning | A run end with `Pin` cores is not on or next to its part (more than 2.5 mm from the part's outline), so the drawing does not show where the line goes. |
| `erc.cable_core_overflow` (existing) | Error | Also counts run cable cores. |
| `erc.dangling_wire` (changed) | — | A wire end that is a breakout end of a core counts as attached. |

### 4. Wire list, cable list, BOM (`reports.rs`, `cable.rs`)

- **Wire list (From-To)**: one row per core of a run cable, end to end. From/To follow each end: a `Pin` end is `USB1:2`; a `Wire` end is the far end of that wire, resolved as today (pin or net label, G18). Colour from the core, gauge from the cable, harness column = cable reference. The breakout wires do not get their own rows (they are part of the core). New last column `ケーブル芯` (`W45/D-`), appended so scripts that read by column position keep working.
- **Cable list**: From/To devices also come from `Pin` ends; used cores = cores with both ends set.
- **BOM**: the cable row is unchanged (per part number). Breakout wires of a run cable are **not** added to the wire roll-up (they are the cable's cores). The same rule now applies to definition-line cables: a `Wire` that is a core of any cable is left out of the wire roll-up (fixes today's double count).

### 5. Drawing (`svg.rs`, `src/canvas/cable.ts`)

- The run is drawn as a thicker line than a wire (0.7 mm against the wire's 0.35 mm) with the usual single-line mark near its middle: a short slash and the core count (`/4`), as EPLAN shows the number of connections. The cable label (`W45  4C×0.2sq  0.3m`) sits next to the run instead of the definition line; the shield ellipse goes around the slash mark.
- **Part end** (`Pin` cores): a short bar across the pins the cores land on, joined to the run end, so it is clear that all those pins are fed by the line (the bar is drawing only).
- **Breakout end** (`Wire` cores): short fan lines from the run end to each core wire's breakout end, with the core name at each wire end (small text, the sheet's annotation size). Breakout ends must be distinct points; the router places them on the 2.5 mm grid.
- Canvas (`cable.ts`): the same geometry as SVG (shared rules, kept in step by tests on both sides), hit test on the run, and a **single-line run** mode in the Cable tool: click the first end (a part, or a point for a breakout), route, click the second end. A dialog fills the core table: core name, colour, end A, end B, defaulting to pin order. The Properties panel core table gains End A / End B columns.
- DXF and KiCad schematic export still do not draw cables; KiCad **netlist** export gets correct per-core nets for free.

### 6. File format and compatibility

- `FORMAT_VERSION` 17. Old files open unchanged (all new fields default to empty); a file without run cables saves byte-for-byte as before apart from the version number.
- Add to `load_project`: refuse a file whose `format_version` is **newer than the app** with a clear message. This protects every later format change. A cable that has a run is stored under a new entity kind `run_cable` (same struct as `cable`), so builds released before this change fail to load the file (unknown entity kind) instead of silently dropping the runs. Cables without a run keep the `cable` kind.
- Converting the fake breakout blocks in the AOGU drawing: redraw them with `connect_cable` once it exists; no `replace_breakout_block` helper.

### 7. Agent-facing MCP API

One new tool, built like `connect_pins` (pin names, no coordinates, everything in one undo step, nothing drawn if any part fails):

```jsonc
connect_cable({
  "sheet_id": "…",                         // optional, first sheet by default
  "reference": "W45",                      // optional, next W number by default
  "part_no": "", "sq": 0.2, "length_m": 0.3, "shield": null,
  "cores": ["VBUS", "D-", "D+", "GND"],    // or [{ "core": "VBUS", "color": "red" }, …]
  "a": { "pins": ["J45.1", "J45.2", "J45.3", "J45.4"], "style": "multi",
         "breakout_at": { "x": 120, "y": 80 } },          // optional; auto when omitted
  "b": { "part": "USB1", "style": "single" },             // pins default to the part's pins in order
  "via": [{ "x": 160, "y": 80 }]                          // optional run waypoints
})
// → { "cable_id": "…", "run": [points], "cores": [
//      { "core": "VBUS", "a": "J45.1", "b": "USB1.1", "wire_id": "…" }, … ],
//     "revision": 42 }
```

- `style: "multi"` draws one wire per core from each pin to its breakout end (routed like `connect_pins`, refused if it would cross another pin) and a `Wire` end; `"single"` lands the run on the part with `Pin` ends. Both ends may be `"multi"` or both `"single"` (the AUX case: `a: { "part": "J1", "style": "single" }, b: { "part": "J2", "style": "single" }`).
- `pins` may be given as `"J45.1"` strings or as `part` + pin list; the count must equal the core count, otherwise the tool refuses and says which side is wrong.
- `cables: [ … ]` draws several cables in one call (like `connections`).
- `execute_commands` gets the same validation through `add_entity` with a run cable, for agents that build geometry themselves.
- `get_netlist` shows `cable_cores` on each net; `get_project` shows the new fields. The `drawing-madakecad-wiring` skill and `docs/10-automation-api.md` get a short "single-line cable" section.

## Testing

Rust tests carry the two doc lines (English, then Japanese) above `#[test]`; Vitest tests carry a `// ja:` line above `it(...)`. `python3 scripts/gen_spec.py --check` must pass (regenerate `docs/13-specification*.md` in the same PR).

- `netlist.rs`: the LTE harness (4 wires → run → `connector_4p`) gives 4 separate nets, each with one board pin and one plug pin; a run lying along a connector's pin column joins nothing; a connector-to-connector run gives one net per core with no wire; `index` picks one side of a through terminal.
- `cable.rs` / `verify.rs`: two cores in one net is `erc.cable_cores_shorted` (run cable and definition-line cable); a core without ends or pointing at a deleted wire is `erc.cable_core_unassigned`; a run end far from its part is `erc.cable_run_end_detached`; a breakout wire end is not dangling; pins fed only by a core are not unconnected.
- `command.rs`: a run cable with a malformed shape (a wire used by two cores, a pin used twice, duplicate core names, an end of the wrong kind) is refused; a missing target is accepted and reported by ERC, so undo and Properties edits keep working after a target was deleted; undo removes the cable and its wires in one step; `remap_refs` keeps core ends in a duplicated sheet.
- `reports.rs`: one From-To row per core, end to end, with the `ケーブル芯` column; breakout wires are not in the BOM wire roll-up.
- `io.rs`: a run cable round-trips; a v16 file opens and keeps its cables; a file newer than the app is refused.
- `svg.rs`: the run, slash mark with core count, part bar and breakout fan are drawn.
- MCP (`madake-mcp`): `connect_cable` draws the LTE harness in one call and one undo; a wrong pin count is refused with nothing drawn; `cables` draws several.
- Vitest (`src/canvas/cable.test.ts`): run label and slash mark match the Rust rules; the run is picked by a click near it; the run tool turns two clicks into one command; the core table defaults to pin order.

## Delivery

Each PR targets `master`; dependent PRs say "merge #N first" instead of stacking.

1. Core: model, format 17, newer-file refusal, netlist rule, ERC, command validation, `refs`. Safe alone: nothing creates run cables yet except `execute_commands`.
2. Reports: wire list rows per core, cable list, BOM roll-up rule.
3. Drawing: SVG and canvas rendering, run tool, Properties core table.
4. MCP `connect_cable`, docs (`05-wire-management`, `10-automation-api`), skill update. Then redraw LTE 45/46, debug 58 and amp power 54 in the AOGU drawing.

## Decisions on the open questions (2026-10-25)

1. Approach: B, extend `Cable`.
2. Wire list: one end-to-end row per core, breakout wires folded in, new last column `ケーブル芯`.
3. Breakout wires are the cable's own cores (jacket stripped back, e.g. LTE 45/46 crimped into JST XH). They are not counted as loose wire in the BOM, and the existing double count of definition-line cable cores is fixed too.
4. `length_m` is the whole cable (the length bought and cut), including the breakout tails.
5. Shield drain as a core: not in this change.
6. Core-to-pin mapping default: by pin order.
7. Old builds must not open a file with run cables: run cables use the new entity kind `run_cable`; `load_project` also refuses files newer than the app.
8. Existing fake breakout blocks: redrawn with `connect_cable`; no helper tool.
9. Part end: a bar across the landed pins.
10. Scope: both ends of a run on the same sheet; across sheets continue through net labels on breakout wires.

## Decision log

| Date | Decision | Why |
|---|---|---|
| 2026-10-25 | Core identity across a single line is set by explicit core-to-pin / core-to-wire assignments, never by touching | The netlist joins a pin to any point of a wire, so a single line over a connector's pin column shorts every pin. EPLAN (bundle designations) and KiCad (bus member labels) also avoid geometric joins. |
| 2026-10-25 | Recommend extending `Cable` (B) over a new `Bundle` entity (A) or a bus wire (C) | One object per physical cable; reuses the cable list, BOM rows and Properties. C cannot land a single line on a plug and depends on labels. The user chose B. |
| 2026-10-25 | Nets may exist without a wire (cable core only) | Needed for a connector-to-connector single-line cable; core-only nets are named after wire nets so existing names do not move. |
| 2026-10-25 | Add a newer-format refusal to `load_project` with format 17 | serde ignores unknown fields, so without it older apps silently drop new data. |
| 2026-10-25 | The spec is a draft only; no product code yet | The user asked for research and a design first. |
| 2026-10-25 | User decisions: B; per-core wire-list rows with `ケーブル芯`; breakout wires are the cable's own cores and are not counted as loose wire (definition-line double count fixed too); `length_m` = whole cable; no shield-drain core; map by pin order; bar at part ends; same-sheet runs only; no `replace_breakout_block` helper | Answers to the open questions |
| 2026-10-25 | Run cables are stored as entity kind `run_cable` | The user chose that old builds must refuse such files; serde ignores unknown fields, so only an unknown entity kind makes v0.4.0 and earlier fail to load instead of silently dropping data |
| 2026-10-25 | Placing or updating a cable checks its structure only; a missing wire/part target is accepted and reported by ERC | Final review: checking target existence made undo lose a cable after separate deletes and blocked Properties edits. Matches §1 ("the end points at nothing; ERC reports it") |
