# Agent drawing comparison: script over MCP vs in-app agent

Date: 2026-10-22 · Japanese: [2026-10-22-agent-drawing-comparison.ja.md](2026-10-22-agent-drawing-comparison.ja.md)

## Setup

The same drawing was made twice: the AOGU CoCo wiring diagram from a whiteboard photo and a BOM spreadsheet. It has 3 A3 sheets (power, USB and control, audio). USB cables are drawn single-line. Power, audio, LTE, I2C and UART are drawn multi-line. Connectors are drawn as plug/socket pairs.

1. **Script over MCP.** A Python helper (the `drawing-madakecad-wiring` skill's `mdk_draw.py`) drives the MCP tools. It knows the block and pin geometry. The sheet scripts were already written.
2. **In-app agent.** The provider is Claude Code CLI 2.1.285. It got the same prompt with the photo and the BOM attached.

## Result

| | Script over MCP | In-app agent |
|---|---|---|
| Time | 4.6 s | about 40 min |
| Tool calls | n/a | 44 (Grep 12, Read 8, execute_commands 11, run_verification 4, get_tidy_metrics 4, other 5), 0 errors |
| Verification | only intentionally unconnected pins | 0 errors; 2 warnings (speaker "power unreachable", a false positive) |
| Tidy metrics | n/a | all 0 |

The agent did these things well:

- It followed the sheet split, BOM numbers, single-line and multi-line rules, enclosures, parts tables and title blocks.
- It used multi-pin connector symbols and dashed mating lines.
- It checked the tidy metrics itself before reporting.
- It reported mismatches between sources, such as different amplifier part numbers.

## Problems found, by impact

1. **The agent computes pin positions by hand.** It does block rounding and pin offsets itself. It went about 10 minutes without a tool call while computing. This explains most of the 40 minutes.
2. **The skill's helper script cannot run in the app.** The agent loaded the skill. Then it said the script cannot run because shell commands are banned.
3. **Reference material was not found.** The old multi-line SVGs and the restart-board design notes were not found. Because of this, the Pi GPIO pin numbers are missing. The LTE board's J1 is one wire instead of EN# and GND.
4. **The agent decided against the instructions without asking.** It powered the Pi from the hub's OTG port into the Pi's data port, although the instructions said to follow the whiteboard.
5. **The final reply runs the intermediate texts together.** The texts between tool calls are joined without line breaks.
6. **Link API cannot tell whether a turn is still running.** `/agent/conversations` has no "in progress" state.
7. **The power-reachability check gives a false positive.** A block such as a battery or an amplifier is not treated as a source.
8. **The agent did not save.** It stopped to ask for a path.

## Follow-up

- 1 and 2: [MCP pin tools spec](../../superpowers/specs/2026-10-22-mcp-pin-tools-design.md).
- 6: next.
- 3, 4 and 7: these need design discussion first.

## Rerun with the pin tools (2026-10-22)

Same prompt and attachments, dev build of `feature/mcp-pin-tools` (batch forms included, mate gap still 3.5 mm, plug designator allowed).

| | First run | Rerun |
|---|---|---|
| Time | about 40 min | 22.5 min (1351 s) |
| Tool calls | 44 | 62 |
| Pin tools | none | `place_mate` 3 calls (70 pairs), `connect_pins` 7 calls (about 110 wires), `get_pins` 2 |
| Verification | 0 errors | 0 errors, 0 unconnected pins |

- `execute_commands` was still used 15 times: symbols, text, shapes and 34 wires.
- 3 `connect_pins` calls failed because the agent sent broken JSON in a 5200-byte input; it split the list and went on.
- Earlier attempts in the same session showed that the agent never called one-wire-per-call tools. It preferred `execute_commands` batches of up to 181 commands. This is why `connections[]` and `mates[]` were added.

Remaining problems, by impact:

1. **The agent left plug designators empty (102 warnings).** With the same designator on both halves, the text is drawn twice on top of itself. One half of a pair needs to hide its designator.
2. **The tidy metrics counted every pair as an overlap and two off-grid points.** Fixed in the same branch: the mate gap is now 5 mm.
3. **The agent cannot look at the sheet.** It said it could not render a PNG without a shell, so it checked text overlaps by numbers only.
4. **The tidy metrics do not say what overlaps.** The agent could not find its 5 and 21 text overlaps.
5. **Still present:** the agent reads the source with Grep, and the reply runs intermediate texts together.
6. **New bug: a CLI process survives an app restart.** When the dev app restarted, the agent CLI of the aborted run kept running. It reconnected to the new app's MCP endpoint and wrote 334 edits into the next run's drawing. Those edits were recorded as the next conversation's turn.
