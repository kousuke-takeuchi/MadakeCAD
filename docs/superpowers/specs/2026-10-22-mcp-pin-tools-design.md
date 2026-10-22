# MCP pin tools: wiring by pin name

Date: 2026-10-22 · Status: design approved in chat, awaiting spec review · Japanese: [2026-10-22-mcp-pin-tools-design.ja.md](2026-10-22-mcp-pin-tools-design.ja.md)

## Goal

The in-app agent should draw a correctly connected wiring diagram without computing coordinates itself.

Background: [agent drawing comparison](../../internal/research/2026-10-22-agent-drawing-comparison.md). The agent drew the AOGU CoCo wiring diagram (3 A3 sheets) in about 40 minutes. A script that knows the pin geometry drew the same diagram in 4.6 s. Most of the agent's time went into computing pin positions by hand (block sizes rounded to 5 mm, pins 2.5 mm outside the box, rotation and mirror).

Success: the agent redraws the same sheets much faster than 40 minutes, with fewer tool calls, and no pin is left unconnected because of a coordinate mistake.

## What exists today

- MCP has only low-level drawing tools: `place_symbol` (place at x/y) and `draw_wire` (a list of points). No tool returns where a placed part's pins are.
- The core already computes pin positions: `pin_positions(inst, def)` in `madake-core/src/netlist.rs` applies mirror, rotation and translation. `block_symbol` / `block_box` in `madake-core/src/block.rs` build a block's pins and its rounded size.
- `PinDef` has `number`, `name`, `at` and `dir` (Up/Down/Left/Right at rotation 0).
- The drawing skill outside the repo (`~/.claude/skills/drawing-madakecad-wiring`) has a Python helper that does this geometry, but the in-app agent cannot run shell commands, so it cannot use the helper.

## Design

### Pin specifier

A pin is named as `"<ref>.<pin>"`, e.g. `"U1.SDA"` or `"J1.2"`. The string is split at the **last** dot, so a reference that contains a dot still works. `<ref>` may also be an entity id (UUID). `<pin>` is matched against the pin number first, then against the pin name.

### 1. `get_pins`

- Input: `sheet_id` (optional, as in other tools), and either `reference` or `entity_id`.
- Output: one entry per matching part (several parts may share a reference, e.g. the two halves of a mate pair):
  - `entity_id`, `reference`, `symbol_id`
  - `bbox`: the part's outline on paper (for a block, after rounding to 5 mm)
  - `pins`: `number`, `name`, `x`, `y`, `dir` (after rotation and mirror)

### 2. `connect_pins`

- Input: `sheet_id`, `from`, `to` (pin specifiers), `via` (optional points), `color`, `sq`, `length_m`, `part_no` (same meaning and defaults as `draw_wire`).
- Output: the new wire's `entity_id`, the `points` actually used, and `revision`.
- Every vertex comes from a pin position or a `via` point. No segment is diagonal.
- Without `via`, the route depends on the two pins' directions:

  | Pin directions | Route |
  |---|---|
  | both left/right | Z: horizontal, vertical, horizontal; the bend x is midway |
  | both up/down | Z: vertical, horizontal, vertical; the bend y is midway |
  | one left/right, one up/down | L: each end leaves in its own direction |
  | already aligned | one straight segment |

- Bend positions snap to the 2.5 mm grid.
- With `via`, the wire passes `from`, each `via` point, then `to`, in order. Where two consecutive points are not aligned, one corner is added, horizontal first.
- No obstacle avoidance. Overlaps are found with `get_tidy_metrics`, and the agent fixes them by adding `via` points.

### 3. `place_mate`

- Input: `sheet_id`, `x`, `y`, `orientation` (`horizontal` | `vertical`), `male_side` (`first` | `second`: whether the left/top part is the plug), `reference`, `value`.
- Places `connector_plug` and `connector_socket` facing each other, 3.5 mm apart, the same layout as the skill's `mate` / `vmate`. The socket carries `reference` and `value`.
- Output: both entity ids, and the two outer connection points (`first`, `second`).

### 4. `place_symbol` returns pins

`place_symbol` keeps `entity_id` and `revision` and adds `pins` in the `get_pins` format. Existing callers keep working.

### Commands and undo

All edits go through the existing `Command::AddEntity`, so undo, redo and per-turn undo work as today. `place_mate` runs its two adds as one command batch, so one undo removes the pair.

### Errors

Nothing is created when any of these happen. The message lists candidates so the agent can correct itself:

| Case | Message includes |
|---|---|
| reference not found | similar references on the sheet |
| pin not found | that part's pins (number and name) |
| several parts with that reference have the pin | their entity ids, to retry with `"<entity_id>.<pin>"` |
| `from` and `to` are the same pin | (nothing extra) |

### System prompt and docs

- `WORKFLOW_RULES` in `madake-agent/src/knowledge.rs` gains a short rule: for wiring diagrams, place parts with `place_symbol` / `place_mate`, read positions with `get_pins`, wire with `connect_pins`; do not compute pin coordinates by hand.
- `docs/10-automation-api.md` and `.ja.md` list the three tools.
- The skill outside the repo says the same for the in-app agent (updated separately, not part of the PR).

## Testing

New `madake-mcp/tests/drawing_tools.rs`, with `///` English and Japanese lines as the spec convention requires:

- `get_pins`: a rotated and mirrored symbol; a block's named pins; two parts with the same reference.
- `connect_pins`: Z (horizontal), Z (vertical), L, straight, with `via`; bends on the 2.5 mm grid; after connecting, `run_verification` reports no unconnected pin for the wired pins.
- The four error cases.
- `place_mate`: both orientations and both `male_side` values; one undo removes the pair.
- `place_symbol` returns `pins`.

Then `python3 scripts/gen_spec.py`. Finally, run the dev app, have the agent draw one AOGU sheet, and compare time and tool calls with the earlier run.

## Delivery

One PR from `feature/mcp-pin-tools`: tools, tests, system prompt rule, docs, and the comparison record.

## Decision log

- 2026-10-22: Route shape: right-angle L/Z only, no obstacle avoidance (user choice 1 of 3). Predictable, small; the agent fixes overlaps with `via`.
- 2026-10-22: Approach A (three low-level tools) over B (one declarative "draw the whole diagram" tool) and C (allow the in-app agent to run the Python helper). A keeps the same granularity as existing tools, so it also helps partial edits; C would weaken the shell ban.
- 2026-10-22: Batch forms added after the live run: `connect_pins.connections[]` and `place_mate.mates[]`, each one undoable edit, all-or-nothing with the failing index named. The agent never called the one-wire-per-call tools; it sent `execute_commands` batches of up to 181 commands with hand-computed coordinates instead.
