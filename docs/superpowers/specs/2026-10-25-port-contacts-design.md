# Port contacts: one drawn pin that carries several conductors

Date: 2026-10-25 · Status: design approved by the user in chat, awaiting spec review · Japanese: [2026-10-25-port-contacts-design.ja.md](2026-10-25-port-contacts-design.ja.md)

## Goal

Let a block pin stand for a **port**: one drawn pin (a USB-A socket on a hub, the USB-A plug of a dongle) that carries several named conductors (**contacts**, e.g. `VBUS`, `D−`, `D+`, `GND`). The drawing keeps one pin and, where the user wants, one line; the netlist, ERC, cross references and the agent see one net per contact.

Success, with the AOGU drawing:

- Hub block `39` has a pin `U3` named `USB-A メス` with contacts `VBUS, D−, D+, GND`; the LTE dongle block has a pin named `USB-A オス` with the same contacts. One plain wire joins the two pins.
- `get_netlist` returns 4 nets (`N003.VBUS`, `N003.D−`, …) instead of one, each listing `39:U3/VBUS` and the dongle's pin with the same contact.
- The LTE harness 45/46 can be drawn with `connect_cable` as 4 board-side wires (JST XH) merging into one run that **ends on the single port pin** `39.U3`; each core lands on the same-named contact.
- Drawings without contacts produce exactly the same netlist, reports and files as before (apart from the format number).

## The problem today

One pin is one net. `extract_netlist` (`madake-core/src/netlist.rs`) puts a symbol pin into the group of every wire it sits on, and `NetPin` records only `reference`, `entity_id` and `pin`. So there is no correct way to draw a USB port as one pin:

- Drawn as one pin with one wire to the dongle, the netlist has **one** net. VBUS, D−, D+ and GND are indistinguishable; nothing downstream (wire list, cross reference, a future harness check) can follow D+ from the hub to the dongle.
- Landing a run cable (the previous feature) on that pin is refused: `connect_cable` needs one pin per core (`plan_side`: "the pins and cores must be the same number"), and `validate_run_cable` refuses two cores on one pin. If it were allowed, the 4 cores would join one pin node and short.
- The workaround is to draw the port as 4 pins (`U3.VBUS`, `U3.D−`, …). That quadruples the pin count of a hub with several ports, makes the block much taller, and is not how the user (or the AOGU whiteboard) thinks about a USB port.

## What exists today (verified in code)

- **Blocks** (`madake-core/src/block.rs`): `symbol_id = "block"`, shape held in `attrs["block_pins"]` as a JSON **string** (`BlockPins { left, right, top, bottom }` of `BlockPin { no, name }`, both structs `#[serde(deny_unknown_fields)]`). `parse_block_spec` validates (empty / duplicate numbers, `BLOCK_PIN_MAX` = 200, size); `block_symbol_from_spec` turns each `BlockPin` into a `PinDef`. The definition is rebuilt from the attributes every time (`symbol::instance_symbol_def`, `DefIndex::get`); a block whose attributes fail to parse resolves to `None` and is **not drawn and not in the netlist**. `Engine` runs the same `parse_block_spec` on add/update (`command.rs` `validate_entity`).
- **`PinDef`** (`symbol.rs`): `number`, `name` (`#[serde(default)]`), `at`, `dir`. No `deny_unknown_fields`. Built with struct literals in 10 places.
- **`get_pins`** (MCP) returns `pin_wiring::PartPins { entity_id, reference, symbol_id, bbox, pins: Vec<PinInfo { number, name, x, y, dir }> }`. `resolve_pin` splits a spec at the last `.` (`"39.U3"` → part `39`, pin `U3`, number first, then name).
- **Netlist** (`netlist.rs`, `extract_netlist`): nodes `[wires][pins][junctions][labels][cable cores]` in one `DisjointSet`. Wires join at end points or junctions; pins and labels join any point of a wire; same-named labels join; same-numbered points of one symbol join (through terminals); run-cable cores join their `CoreEnd` targets only. A group becomes a `Net` if it has a wire or a core. Order: `(wire_ids.is_empty(), min wire id, min (cable_id, core))`; name: label (lexicographic minimum) > wire number (`wire_no::is_generated_number` rule) > `N001…` in that order.
- **Mates are not conducting.** `connector_plug` / `connector_socket` (1 pin each, `symbol.rs`) are placed 5 mm apart by `pin_wiring::mate_pair` with the same reference; their outlines touch but their pins are at the outer ends. Nothing in `netlist.rs` joins the two halves. The pair is only known to ERC (`mate.rs` `mate_diagnostics`; `verify.rs` treats `connector_plug|connector_socket` as `MATE_HALF` to allow the shared reference) and to reports (`嵌合相手` column). So today a hub → socket | plug → dongle chain is **two nets**.
- **Cross-sheet** (`xref.rs`, `extract_netlist_project`): per-sheet nets are merged by the names of labels found **geometrically on the net's wires** (`labels_on_net` scans `wire_ids`). `project_label_conflicts` (`verify.rs`) reports a project net with 2+ label names.
- **Who uses `extract_netlist`** (non-test): ERC `verify::erc` (unconnected pin, label conflict), `verify::electrical` (maps `(entity, pin)` → **one** net index), `cable::check_cables` (core short), `sim.rs` (net voltages via `wire_ids`), `xref.rs`, MCP `get_netlist`, Link API `/netlist`, Tauri `get_netlist`, the agent's context (`agent.rs`, net count), and wire-centric code: `wire_no::numbering_nets` (one number per net, written to its `wire_ids`), wire-number drawing in `svg.rs` `render_wire_numbers`, `dxf.rs`, `kicad.rs` (wire numbers become KiCad labels), `tidy.rs` (wire-number text boxes) and `reports::wire_list_rows` (label-end resolution, G18). There is no KiCad *netlist* export; KiCad export is the schematic (`export_kicad_sch`). SPICE (`spice.rs`) does not use the netlist at all: its nodes are coordinate clusters and every wire is a resistor.
- **Run cables** (`model.rs`): `CoreEnd::Pin { part, pin, index }` | `CoreEnd::Wire { wire }`. `cable_run::plan_cable_run` / `plan_side` map cores to pins by order; `single` style draws a bar across the landed pins (`cable::bar_ends`). `cable::validate_run_cable` refuses a pin key `(part, pin, index)` used by two cores.
- **File format** 17 (`model.rs` `FORMAT_VERSION`). `io::load_project` refuses newer files (`check_format_version`). Entities go through the serde proxy `EntityRepr` (`kind` tag), which is **also the IPC, MCP and Link API shape**; `run_cable` exists in `EntityRepr` and in the frontend types (`src/ipc.ts`, `src/canvas/cable.ts`). Releases: tags up to `v0.4.0` (format 16 or older, no newer-file refusal).
- **Old builds and a `contacts` key.** Because `BlockPin` is `deny_unknown_fields` (also in `v0.4.0`), an old build that opens a block whose `block_pins` contains `contacts` cannot parse it: the block **silently disappears from the drawing and the netlist** (its pins become unconnected for everything wired to them), while the attribute string is kept and saved back unchanged. Edits to that block are refused by `Engine` validation. So the risk is not "contacts silently dropped" but "the whole device silently vanishes", which is still a silently wrong netlist.
- **UI**: the block pin table is edited in `BlockEditorDialog.vue` (opened from the Properties panel, `PropertiesPanel.vue` shows a pin-count summary and an edit button) with one `BlockPinColumn.vue` per side (inputs `no`, `name`). The TS side mirrors Rust: `src/canvas/dynamicSymbol.ts` (`BlockPin { no, name? }`, `parseBlockSpec`, `blockSymbol`) and `src/canvas/blockEditor.ts` (`BlockPinRow`, `pinsOf`, `attrsFromDraft`, `draftFromAttrs`, `draftIssues`). TS `PinDef` is in `src/ipc.ts`. The frontend matches `kind === "symbol"` in many places, and the FreeCAD add-on (`freecad-addon/madakecad_link/linking.py`) filters entities by `kind == "symbol"`.
- **Agent prompt** (`madake-agent/src/knowledge.rs`) already tells the agent to use `connect_cable` for "USB plug side one line, board side 4 wires".

Prior art (from the cable-breakout spec, verified there): KiCad group buses name members `USB1.DP`, `USB1.DM`. This design uses the same `<base>.<member>` naming for contact nets.

## Design

### 1. Data model

**Block pin JSON** (`block.rs`):

```rust
pub struct BlockPin {
    pub no: String,
    #[serde(default)]
    pub name: String,
    /// Port contacts (conductors carried by this one pin), in order. Empty = a plain pin.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contacts: Vec<String>,
}
```

```json
{"right":[{"no":"U3","name":"USB-A メス","contacts":["VBUS","D−","D+","GND"]}]}
```

`parse_block_spec` adds: each contact is trimmed and must be non-empty; no duplicate within one pin; a pin with contacts has **at least 2** (one conductor is a plain pin); at most `BLOCK_CONTACT_MAX` = 32 per pin. The same pin may repeat contact names used by other pins (every USB port has `VBUS`). No character restrictions: nets match contact names by exact string, never by parsing.

**`PinDef`** (`symbol.rs`) gains `#[serde(default, skip_serializing_if = "Vec::is_empty")] pub contacts: Vec<String>`. `block_symbol_from_spec` copies the trimmed contacts. All other symbols (built-in, dynamic, user library) keep it empty; the 10 struct literals get `contacts: Vec::new()`. User symbol files are unchanged (field skipped when empty). Helper: `PinDef::is_port()` = `!contacts.is_empty()`.

A port pin is **drawn exactly like any other block pin** (number outside, name inside). Contacts are not drawn.

**`PinInfo`** (`pin_wiring.rs`, `get_pins`) gains `contacts` (skipped when empty).

**`CoreEnd::Pin`** (`model.rs`) gains `#[serde(default, skip_serializing_if = "Option::is_none")] contact: Option<String>`: the contact of a port pin this core is. `None` on plain pins.

**`NetPin`** (`netlist.rs`) gains `#[serde(default, skip_serializing_if = "Option::is_none")] contact: Option<String>`, shown as `39:U3/VBUS` wherever pins are printed (`xref_table_rows`, `madake-cli` `format::netlist`).

**`Net`** gains two fields, both skipped when empty, so JSON of drawings without ports is byte-identical:

- `contact: Option<String>`: the contact this net is (`"VBUS"`), set only on nets split from a port group.
- `contact_labels: Vec<String>`: the label names this net is known by across sheets (see §2.6). Filled only on split nets; when empty, `xref` reads labels from the sheet as today.

### 2. Netlist rule (`netlist.rs`)

Terms, per sheet:

- **Port pin**: a connection point of a symbol whose `PinDef.contacts` is non-empty.
- **Pass-through pin**: a pin of a mate half, `symbol_id` `connector_plug` or `connector_socket`. Decided by a new `mate::is_mate_half(symbol_id)`, which `verify.rs` also uses for its `MATE_HALF` exemption (today a `matches!` there). Not the whole connector family (`mate::is_connector_instance` = `connector_*`): a `connector_4p` pin is one conductor (JST), and treating it as pass-through would hide a USB-to-JST short.
- **Plain pin**: any other pin.
- **Pass-through elements**: wires, junctions, net labels and pass-through pins. They carry every contact of their group.

#### 2.1 Algorithm

1. **Clusters.** Run today's union-find **without the cable-core nodes**: wires, pins, junctions, labels, same-name labels, same-number points. Each root is a *cluster*.
2. **Port clusters.** A cluster that contains at least one port pin is a port cluster. Its **contact list** is the union of the contacts of its port pins, in a fixed order: port pins sorted by `(reference, pin number, entity id)`, each pin's contacts in declared order, a name added the first time it appears. Every other cluster is a single-conductor cluster.
3. **Slots.** A single-conductor cluster is one slot. A port cluster is one slot per contact plus one **residual slot**.
4. **Members into slots** (port clusters only):
   - port pin → every slot of its own contacts (a `NetPin` with `contact = Some(c)` in each);
   - pass-through pin, wire, junction, label → every contact slot (pass-through pins listed with `contact = Some(c)`; wire ids repeated in each contact net);
   - plain pin → the contact slot whose name equals the pin's `PinDef.name` (trimmed, exact); otherwise the residual slot and ERC `erc.port_to_plain_pin`. The pin number is not used (a JST pin numbered `1` must not join a contact named `1` by accident).
5. **Cores.** Union, over slots, each run-cable core's two ends. An end resolves to: `Wire { wire }` → the slot of that wire's cluster if it is single-conductor (a breakout wire is the core itself); `Pin` on a plain pin → that pin's cluster; `Pin` on a port pin with `contact = Some(c)` → slot `c` of the port cluster; `Pin` on a port pin without `contact` → slot named like the core (`core.core`), else the residual slot (ERC, §3). A `Wire` end whose wire is in a port cluster → the residual slot (ERC `erc.cable_core_unassigned`: a breakout wire must not touch a port net).
6. **Dotted labels.** A single-conductor cluster that carries a label named exactly `<L>.<c>`, where `<L>` is a label of a port cluster that has contact `<c>`, is unioned with that slot. This lets the user tap one conductor by name (`USB_MIC.VBUS` on a plain wire) on the same sheet; across sheets the same name joins in `xref` (§2.6).
7. **Nets.** Each resulting slot group becomes a net if it holds a wire or a core (today's rule). Residual slots with no plain pin and no core are dropped (the shared wires already appear in every contact net).
8. **No ports → no change.** If no cluster is a port cluster, steps 2–6 do nothing and steps 1+5 form exactly today's groups (union-find is order-independent), with today's order and names.

The current behavior is kept as a second public function `extract_wire_nets(sheet, symbols)`: steps 1 and 5 without splitting, one net per physical group. Both functions share the node and union code.

#### 2.2 Which consumers get which view

- **Split (`extract_netlist`)**, electrical meaning: `verify::erc`, `verify::electrical`, `cable::check_cables`, `sim.rs` net voltages, `xref.rs`, MCP `get_netlist`, Link API, Tauri `get_netlist`, agent context.
- **Unsplit (`extract_wire_nets`)**, one entry per drawn wire group: `wire_no::numbering_nets` (otherwise a USB wire would get four numbers and the last would win), `svg.rs` `render_wire_numbers`, `dxf.rs` and `kicad.rs` wire-number text, `tidy.rs` wire-number boxes, `reports::wire_list_rows` (one row per `Wire`, unchanged; G18 label ends resolve on the drawn group).
- Code that indexes nets by `(entity, pin)` must accept a port pin in several nets: `verify::electrical` `pin_net` becomes `(entity, pin) → Vec<net index>`; `verify::erc` `connected` is a set and needs no change.
- SPICE stays as it is: nodes are coordinate clusters, so a port pin is one node and its contacts are not simulated separately. Ports are signal/power interfaces of blocks, which the DC check does not model today; noted, not changed.

#### 2.3 Names

Per contact net, candidates in today's priority:

1. **Labels**: `<L>.<c>` for each label `L` of the port cluster(s) in the net, plus the literal labels of its single-conductor parts (e.g. a dotted label, or `PWR5V` on the JST side). Name = the lexicographic minimum; `Net.label` = that name.
2. **Wire numbers**: `<W>.<c>` where `W` is the port cluster's wire number (today's pick rule over the cluster's wires), plus raw wire numbers of single-conductor parts; picked with today's rule (manual before generated, then lexicographic). `Net.wire_no` keeps the raw number of the net's own wires (today's rule over all its wires), so wire-number readers see real numbers.
3. **Auto**: `<Nxxx>.<c>`. A port cluster takes **one** `Nxxx` number, at the position of its first contact net in the order below; its contact nets share it. A residual net is named `<base>` (the same base without a suffix).

If a contact net joins two port clusters through a core, the base comes from the first of them in net order.

**Collisions.** A literal label equal to a generated `<L>.<c>` is not a collision: step 6 joins it. A manual wire number that equals a generated name (`5.VBUS`) is not joined and both nets show the name; names are display text and nets stay distinct by content, so this is accepted (not checked). `Nxxx.c` cannot collide with `Nxxx`.

#### 2.4 Order (stability)

Sort key: `(wire_ids.is_empty(), min wire id, min (cable_id, core), contact rank)`, where contact rank is the contact's index in the port cluster's contact list (residual last) and 0 for unsplit nets. Without ports the last component is always 0, so the order and the `N001…` numbering are exactly today's. With ports, all contact nets of a cluster sit together where the unsplit net used to be, so the numbers of the other nets do not move.

#### 2.5 Mates

Mates still do not conduct (§ What exists). A hub → socket | plug → dongle chain stays two drawn groups; each is a port cluster (one port pin plus a pass-through pin) and is split per contact, with the mate half listed in every contact net. The two sides are not joined by contact, exactly as they are not joined at all today. Making mate halves conduct is a separate change (Open questions).

The user's choice for the common case (option 1): a plain wire, or a chain through pass-through elements, between two port pins **connects same-named contacts**. That is steps 2–4 in one cluster.

#### 2.6 Across sheets (`xref.rs`)

`extract_netlist_project` uses `Net.contact_labels` when it is non-empty instead of `labels_on_net` names: `USB_MIC.VBUS` on sheet 1 merges with `USB_MIC.VBUS` on sheet 2 (split there too, or a plain wire with the dotted label), and never with `USB_MIC.D−`. `NetMember.label_ids` and `sites` still come from the geometric scan, so cross-reference addresses on the drawn labels are unchanged. `ProjectNet.label_names` then holds the qualified names, so `project_label_conflicts` compares the right things.

A sheet whose net carries label `L` **without** a port, while `L` is a port label elsewhere, keeps key `L` and is not joined to any contact (joining it to all would short them). ERC reports it (§3).

### 3. ERC

New codes live in a new `madake-core/src/port.rs` (`check_ports(sheet)`, called from `verify_sheet_with` next to `check_cables`; the cross-sheet check from `verify_project`).

| Code | Severity | When |
|---|---|---|
| `erc.port_to_plain_pin` | Warning | A plain pin is in a port cluster and its name matches no contact. Message lists the contacts, e.g. `J45:1 は 39:U3 (VBUS, D−, D+, GND) につながっていますが、どの接点か分かりません。ピン名を接点名にするか、connect_cable で芯ごとに引く`. The pin goes to the residual net. |
| `erc.port_contact_unmatched` | Info | A port cluster has 2+ port pins and some contact is on only one of them (e.g. a `SHIELD` contact on the hub but not on the dongle). Not raised for a cluster with one port pin (a port wired to a mate half). |
| `erc.port_label_unsplit` | Warning | Project: label `L` is on a port net on one sheet and on a net without a port on another. Those sheets are not joined; the message suggests placing the port there or using labels `L.<contact>`. |
| `erc.cable_core_unassigned` (existing) | Error | Also: a core's `contact` is not among the pin's contacts (the block was edited after), a core on a port pin has no `contact` and its name is no contact, or a breakout wire end touches a port net. |
| `erc.label_conflict` (changed) | Error | On a contact net, compare qualified names (`USB_MIC.VBUS`) with the literal labels, so `USB_MIC` + `USB_MIC.VBUS` is one name, not a conflict. |
| `erc.unconnected_pin` | (unchanged) | Unchanged: a port pin listed in any net is connected. |

### 4. Run cables and `connect_cable` (`cable_run.rs`, `cable.rs`)

A side may land on **one port pin**: `"b": { "pins": ["39.U3"], "style": "single" }` (or `"part": "39", "pins": ["U3"]`).

- Applies when the side resolves to exactly one pin and that pin has contacts. `style` must be `single` (`multi` is refused: a port is one connection point).
- Mapping: if every core name is a contact of the pin → by name (cores may be a subset, e.g. 4 cores on a 9-contact USB 3 port). Else if no core name is a contact and the counts are equal → by order (core i → contact i). Otherwise refused, listing the contacts (a partial match is most likely a typo).
- Each core gets `CoreEnd::Pin { part, pin, index, contact: Some(c) }`. Labels in the result: `"39.U3/VBUS"`.
- Geometry: the run ends at the pin's connection point and leaves in the pin's direction. **No bar** is drawn when all `Pin` ends of a side are one connection point (SVG `render_run_cable` and `src/canvas/cable.ts` skip the bar when `bar_ends` gives one point). `erc.cable_run_end_detached` works as today.
- `validate_run_cable`: the pin key becomes `(part, pin, index, contact)`; two cores on the same contact are refused, cores on different contacts of one pin are fine. Contacts are not checked against the block here (structure only, per the cable-breakout decision); ERC reports a stale contact.
- Wire list rows of run-cable cores: the pin end prints `39:U3/VBUS`. Cable list "used cores" unchanged. BOM unchanged.

Other sides keep today's rule (one pin per core, by order).

### 5. Wire list and BOM

Unchanged. A plain wire between two ports is one `Wire`, one wire-list row, one roll-up entry, numbered once (unsplit view, §2.2). A run cable stays one cable row.

### 6. MCP and agent

- `place_symbol`: the `block_pins` description gains `contacts` (`{"no":"U3","name":"USB-A メス","contacts":["VBUS","D−","D+","GND"]}`), with the rule "draw a USB / audio / DC-jack port as one pin with contacts, not one pin per conductor".
- `get_pins`: pins show `contacts`.
- `connect_cable`: description adds port landing (§4).
- `get_netlist`: description says ports are split into `<name>.<contact>` nets and pins may carry `contact`.
- `connect_pins` is unchanged: wiring two port pins with one wire is the normal way to connect same-named contacts.
- `knowledge.rs`: one bullet under the wiring rules: "A connector that carries several conductors (USB, 3.5 mm audio, DC jack) is **one block pin with `contacts`**. Join two ports with one `connect_pins` wire (same-named contacts connect); land a cable on a port with `connect_cable` and `pins: [\"<ref>.<port pin>\"]`; name single-conductor pins after the contact (`VBUS`, `GND`) so they join it."
- Docs: `docs/10-automation-api(.ja).md` (block_pins, get_pins, connect_cable, get_netlist), `docs/05-wire-management(.ja).md` (port pins), the `drawing-madakecad-wiring` skill.

### 7. UI

- `BlockPinColumn.vue`: a third input per row, **接点 / Contacts**, comma separated (`,` `、` `，` accepted), placeholder `VBUS, D−, D+, GND`. Empty = plain pin.
- `blockEditor.ts`: `BlockPinRow` gains `contacts: string` (the text); `pinsOf` splits, trims and drops empties, and writes `contacts` only when non-empty; `draftFromAttrs` joins with `, `; `draftIssues` gains `contactsTooFew`, `duplicateContact`, `tooManyContacts` (same rules as Rust).
- `dynamicSymbol.ts`: `BlockPin.contacts?: string[]`; `parseBlockSpec` gets the same checks; `blockSymbol` copies contacts into `PinDef`. `src/ipc.ts` `PinDef.contacts?: string[]`, `NetPin.contact?`, `Net.contact?`, `Net.contact_labels?`, `CoreEnd` pin `contact?`.
- `PropertiesPanel.vue`: the block summary adds the port count when non-zero (`ポート 2`). The drawing does not change.
- i18n keys in both locales; MadakeCAD.pen gets the extra column in the block pin table frame.

### 8. File format and compatibility

- `FORMAT_VERSION` 18. The `model.rs` history gains the v18 line. Old files open unchanged; a file without port pins is written byte-for-byte as before apart from the version.
- **On-disk entity kind `port_block`.** A symbol with `symbol_id == "block"` whose `block_pins` has any pin with contacts is written with `"kind": "port_block"` (same `SymbolInstance` fields) so that `v0.4.0` and older fail to load the file (unknown entity kind) instead of silently dropping the block from the netlist. Format-17 builds already refuse a format-18 file by number.
- **Only in the file, not in `EntityRepr`.** `EntityRepr` is also the IPC / MCP / Link API shape, and `kind == "symbol"` is matched across the frontend and in the FreeCAD add-on (`linking.py`). Unlike `run_cable` (a new kind the frontend had to learn), a port block must stay `symbol` there. So the rename is done in `io.rs`: `save_project` serializes to a `serde_json::Value`, rewrites `kind` for qualifying entries in `sheets[].entities`, then pretty-prints; `load_project` (after `check_format_version`) parses to a `Value`, rewrites `port_block` back to `symbol`, then deserializes. A `port_block` entry without contacts (hand-edited) loads as a plain symbol and is written back as `symbol`, like `run_cable` without a run.
- Every file write goes through `io::save_project` (Tauri `save_project`, MCP and Link API via `agent.rs`). Macro files (`macros.rs` `write_macro_to`) are not rewritten: an old build refuses to insert such a macro because `Engine` validation fails on the unknown `contacts` key, which is loud, not silent.
- `CoreEnd::Pin.contact` needs no kind trick: run cables are already `run_cable`, unknown to `v0.4.0`, and format 18 stops format-17 builds.

## Testing

Rust tests carry the two doc lines (English, then Japanese) above `#[test]`; Vitest tests carry a `// ja:` line above `it(...)`. Regenerate `docs/13-specification*.md` with `python3 scripts/gen_spec.py` in each PR (`--check` in CI).

- `block.rs`: contacts parse and reach `PinDef`; empty / duplicate / single / too many contacts are refused with clear messages; a pin without contacts is unchanged.
- `netlist.rs`:
  - two port pins joined by one wire give one net per contact, each with both pins and `contact` set, named `Nxxx.<c>`, in contact order;
  - port → wire → socket | plug → wire → label → wire → port: two drawn groups, each split per contact, mate halves listed in every contact net, no join across the mate;
  - a plain pin named `GND` in a port cluster joins `.GND`; an unnamed plain pin goes to the residual net;
  - the LTE harness: 4 JST pins → breakout wires → run → port pin `39.U3` gives 4 nets, each one JST pin + `39:U3/<c>`;
  - a dotted label `USB_MIC.VBUS` on a plain wire joins `USB_MIC.VBUS`;
  - a drawing without ports gives the same `Vec<Net>` (names, order, JSON) as `extract_wire_nets` and as before (the existing `netlist.rs` tests keep passing);
  - `extract_wire_nets` keeps one net for a USB wire.
- `xref.rs`: split nets merge across sheets per contact and never across contacts; a dotted label on another sheet joins its contact.
- `port.rs` / `verify.rs`: `erc.port_to_plain_pin`, `erc.port_contact_unmatched` (and not for a single port pin), `erc.port_label_unsplit`, stale contact as `erc.cable_core_unassigned`, `USB_MIC` + `USB_MIC.VBUS` is no `erc.label_conflict`, port pins are not unconnected.
- `wire_no.rs` / `reports.rs`: a USB wire gets one wire number; the wire list has one row for it; run-cable rows print `/contact`.
- `cable_run.rs` / `cable.rs`: port landing by name, by order, partial match refused, `multi` on a port refused, subset of contacts accepted, two cores on one contact refused, no bar for a single landing point.
- `io.rs`: format 18; a port block is saved as `port_block` and comes back as `Entity::Symbol` with contacts; a file without ports has no `port_block` and is unchanged apart from the version; a `port_block` without contacts loads as a symbol; a v17 file opens unchanged; IPC/MCP JSON of a port block has `kind: "symbol"`.
- MCP (`madake-mcp/tests`): `place_symbol` with contacts, `get_pins` returns them, `connect_cable` lands on `39.U3`, `get_netlist` shows split nets.
- Vitest: `blockEditor.test.ts` (contacts text round trip, separators, issues), `dynamicSymbol.test.ts` (same validation as Rust, `PinDef.contacts`), `cable.test.ts` (no bar for one landing point).

## Delivery

Each PR targets `master`; dependent PRs say "merge #N first" instead of stacking.

1. Core: `BlockPin` / `PinDef` contacts, netlist split + `extract_wire_nets` + consumer switch, `NetPin` / `Net` fields, xref keys, `port.rs` ERC, format 18 with `port_block`. Safe alone: only `execute_commands` / `place_symbol` attrs can create contacts yet.
2. Run cables: `CoreEnd::Pin.contact`, `connect_cable` port landing, validation key, no-bar drawing (SVG + canvas), wire-list `/contact`.
3. UI: block editor contacts column, Properties summary, TS types, i18n, .pen.
4. MCP descriptions, `knowledge.rs`, docs, skill. Then redraw the AOGU hub/dongle ports and LTE 45/46.

## Open questions

1. **Should mate halves conduct?** Today a plug/socket pair does not join its two nets in the netlist at all, so contacts cannot be followed through a mate either. Recommendation: leave it for a separate change (it would change nets of every existing drawing with mates); this spec only makes mate halves pass-through inside their own net.

## Decision log

| Date | Decision | Why |
|---|---|---|
| 2026-10-25 | A pin may carry named contacts; it is drawn as one pin and split into one net per contact | The user wants one pin per USB port but per-conductor connectivity. |
| 2026-10-25 | User choice, option A: only block pins get contacts now (library / user symbols later) | Blocks are where ports occur (hub, dongle); `PinDef.contacts` is general so symbols can follow without a format change. |
| 2026-10-25 | User choice, option 1: a plain wire (or a chain through pass-through elements) between two ports connects same-named contacts | Matches how a USB cable is drawn as one line; no per-contact wiring needed. |
| 2026-10-25 | Split nets are named `<base>.<contact>`; a port cluster consumes one auto number | Follows KiCad group-bus naming; keeps other nets' names and order stable. |
| 2026-10-25 | Pass-through = `connector_plug` / `connector_socket` only (`mate::is_mate_half`), not all `connector_*` | A `connector_{n}p` pin is one conductor; treating it as pass-through would hide shorts. |
| 2026-10-25 | Plain pins join a contact by pin **name** only, not number | Avoids accidental joins such as JST pin `1` with a contact named `1`. |
| 2026-10-25 | Keep the unsplit netlist as `extract_wire_nets` for wire numbering, wire-number drawing and the wire list | Split nets share wires; per-net wire numbering would give one wire four numbers. The user asked that wire list and BOM stay one row. |
| 2026-10-25 | Splitting runs on clusters built without cable cores, then cores and dotted labels join slots | A breakout wire carries one core; built after the cores it would sit in the port group and look multi-contact. |
| 2026-10-25 | Cross-sheet merge uses qualified names (`USB_MIC.VBUS`) via `Net.contact_labels` | Split nets share their wires, so the geometric label scan would merge all contacts again. |
| 2026-10-25 | Dotted labels `L.c` tap one contact (same sheet and across sheets) | Needed for cross-sheet consistency; gives a by-name way to reach one conductor. Recommended in this spec; not yet discussed with the user. |
| 2026-10-25 | File format 18; port blocks are written as `port_block` in the file only, `symbol` in IPC/MCP | User chose that old builds must refuse such files. Old builds would make the block vanish (deny_unknown_fields), not drop the contacts; a file-only kind avoids touching the many `kind == "symbol"` checks in the frontend and the FreeCAD add-on. |
| 2026-10-25 | Contacts need at least 2 names, at most 32, unique per pin | One conductor is a plain pin; limits keep the editor and nets sane. |
| 2026-10-25 | SPICE unchanged | Its nodes are coordinate clusters, not nets; ports are not simulated today. |
| 2026-10-25 | The spec is a draft only; no product code yet | The user asked for research and a design first. |
