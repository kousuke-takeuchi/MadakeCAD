# MadakeCAD

**日本語版: [README.ja.md](README.ja.md)**

An industrial electrical schematic CAD (aiming for open source). MadakeCAD lets you create electrical drawings for industrial equipment — robots, factory machinery, control panels — that comply with industrial standards (JIS). The goal is twofold: **LLM integration so that non-experts can produce correct drawings**, and **an industrial-grade CAD experience that veterans find comfortable** (benchmarked against AutoCAD Electrical and EPLAN). Wiring verification, circuit simulation, and mechanical CAD (FreeCAD) integration are part of the design. See the [requirements document](docs/requirements.md) (Japanese) for the full vision.

## Key Features

- **Schematic editing**: JIS C 0617 symbols plus parametric terminal blocks/connectors (1–50 poles), orthogonal wiring, grid/pin snapping, automatic reference designators, multi-sheet, view classes (layers)
- **Standards & output**: JIS drawing frame (zones, title block), print-quality SVG/PDF with embedded CJK fonts, BOM CSV, wire list CSV
- **Verification**: ERC (unconnected pins, duplicate references, dangling wires, label conflicts) and electrical checks (source reachability, wire ampacity, voltage drop, fuse rating) backed by real ngspice DC analysis
- **Simulation**: DC operating point (net voltages, component currents/power) with open-switch what-if analysis
- **Parts management**: global parts database (SQLite; part numbers, ratings, procurement URLs) and wire part master; place parts with ratings applied automatically
- **Import**: KiCad `.kicad_sch`
- **AI & automation**: built-in MCP server, in-app AI chat (Claude Code integration with per-turn undo), `madake` CLI, REST Link API with SSE

Feature documentation by category: [docs/features/](docs/features/README.md). Implementation status: [docs/features.md](docs/features.md) (Japanese).

## Documentation

| Document | Contents |
|---|---|
| [Feature docs](docs/features/README.md) | Public feature descriptions by category (EN/JA) |
| [Requirements](docs/requirements.md) | Vision (5 pillars), milestones, requirements (JA) |
| [Architecture](docs/architecture.md) | System diagrams, file map, data flow, state machines, change recipes (JA) |
| [Tech stack](docs/tech-stack.md) | Technologies, versions, selection rationale (JA) |
| [Data model](docs/data-model.md) | Document model, parts DB ER diagram (JA) |
| [Setup](docs/setup.md) | Prerequisites, setup, troubleshooting (JA) |
| [Feature inventory](docs/features.md) | Implemented/planned checklist (JA) |
| [Feature specs](docs/specs/README.md) | Detailed specs for upcoming milestones M2–M6 (JA) |
| Screen designs | `MadakeCAD.pen` (Pencil) is the source of truth |

Japanese-only documents are being migrated to English-canonical progressively.

## Architecture

Every edit is executed as a serializable Command by a single engine (madake-core). The UI (Tauri IPC) and AI (built-in MCP server) share the same Command API; results are broadcast to all clients as patches.

```
UI action / MCP tool → Command(JSON) → madake-core → patch(JSON) → UI re-render
```

- `src-tauri/crates/madake-core` — document model, Command engine (undo/redo), symbol library, netlist, verification, simulation, file I/O
- `src-tauri/crates/madake-mcp` — built-in MCP server (rmcp / Streamable HTTP) and Link API (/api/v1)
- `src-tauri/crates/madake-agent` — in-app AI chat backends
- `src-tauri/crates/madake-cli` — `madake` command (thin Link API client)
- `src-tauri/src` — Tauri shell (IPC handlers, MCP startup, patch forwarding)
- `src/` — Vue 3 + TypeScript frontend (custom Canvas2D renderer)

## Development

See the [setup guide](docs/setup.md). Prerequisites: Rust (rustup) and Node.js. Optional: [ngspice](https://ngspice.sourceforge.io/) for circuit analysis (verification falls back to a graph approximation without it).

```bash
npm install
npm run tauri dev
```

Tests:

```bash
cd src-tauri && cargo test
npx vitest run
```

## MCP Integration

While the app is running, an MCP server listens at `http://127.0.0.1:9310/mcp` (configurable via `MADAKE_MCP_PORT`). Claude Code connects automatically through this repository's `.mcp.json` and can edit drawings directly via tools such as `get_project`, `place_symbol`, `draw_wire`, `execute_commands`, `run_verification`, `simulate_op`, `search_parts`, `import_kicad`, and `export_svg|pdf|bom|wire_list`. Dynamic symbol IDs `connector_{n}p` / `terminal_block_{n}p` allow variable pole counts.

## madake CLI

A thin client that talks to the running app over the Link API (`http://127.0.0.1:9310/api/v1`). All edits go through the Command engine, so CLI changes are undoable and appear in the UI immediately.

```bash
cd src-tauri && cargo install --path crates/madake-cli   # installs `madake`
```

```bash
madake status                        # connectivity + project summary
madake netlist [--sheet <ID>]
madake verify [--sheet <ID>]         # ERC + electrical checks
madake sim [--open SW1,K1]           # DC operating point (requires ngspice)
madake parts [<query>] [--category <c>]
madake export svg|pdf|bom|wire-list <path> [--sheet <ID>]
madake save|open <path>              # .mdkproj, or .kicad_sch to import
madake exec <commands.json>          # arbitrary Command array
madake undo / madake redo
```

Common options: `--port` (default 9310), `--json` (raw JSON for jq etc.).

## Parts Database

A global SQLite master stored in the OS app-data folder (override with `MADAKE_PARTS_DB`). Holds part numbers, makers, ratings (linked to verification), procurement/datasheet URLs, prices, and a wire part master (color + gauge → part number). Sample data is seeded on first creation.

## File Format

`.mdkproj` is pretty-printed JSON (git-diffable). KiCad `.kicad_sch` files can be imported via `madake open`, the app's Open dialog, or the `import_kicad` MCP tool.

## Roadmap

Milestones (see [requirements](docs/requirements.md) §2):

1. **M1 Foundation** — ✅ mostly complete: Command engine, editor, JIS frame, PDF/BOM/wire list, verification, DC simulation, KiCad import, AI chat
2. **M2 Reference-drawing parity** — revision table, wire numbers, harness boundaries, cross-sheet references
3. **M3 AI-first drafting** — standards knowledge + verification loop, tidy-up automation, multi-provider LLM support
4. **M4 Industrial CAD core** — terminal charts, circuit macros, PLC I/O, report sheets, symbol editor (benchmarked against AutoCAD Electrical / EPLAN)
5. **M5 Mechanical CAD integration** — FreeCAD workbench, part linking, wire-length write-back
6. **M6 Open-source release** — licensing, cross-platform builds, i18n, community docs
