<div align="center">

# MadakeCAD

**An AI-first, standards-compliant electrical CAD for industrial equipment.**

Draw JIS-compliant wiring diagrams for robots, machinery, and control panels —
with an AI assistant that edits through the same undoable command engine as you,
and verification grounded in a real circuit solver (ngspice).

[Overview](docs/01-overview.md) · [Getting Started](docs/02-getting-started.md) · [Documentation](#documentation) · [Roadmap](docs/12-roadmap.md) · [日本語](README.ja.md)

<img src="docs/images/sample-drawing.svg" alt="Sample drawing: 48V power distribution on a JIS A3 frame" width="820">

*A drawing produced by MadakeCAD: JIS A3 frame with zone references and title block.*

</div>

---

## Why MadakeCAD?

- 🏭 **PCB CADs are the wrong tool for equipment wiring** — no JIS frames, no wire part management, no terminal-block-centric connections, no wire lists
- 💰 **Industrial CADs (AutoCAD Electrical, EPLAN) are closed and heavyweight** — and have no meaningful AI integration
- 🤝 **Expertise shouldn't be the entry ticket** — with an LLM in the loop, non-experts produce correct drawings while veterans keep a fast, conventional CAD workflow

## Highlights

- ✏️ **Purpose-built schematic editor** — JIS C 0617 symbols, parametric terminal blocks & connectors (1–50 poles), orthogonal wiring with grid/pin snap, auto reference designators, multi-sheet, layers
- 📐 **Standards-true output** — JIS frame (zones, title block), print-quality SVG/PDF with embedded CJK fonts, BOM & wire-list CSV
- ✅ **Solver-grounded verification** — ERC plus electrical checks (reachability, ampacity, voltage drop, fuse rating) judged from an actual ngspice DC solution
- ⚡ **DC simulation** — net voltages, component currents/power, open-switch what-if analysis
- 🗄️ **Parts database** — local SQLite master with ratings, procurement links, and a wire part master; place parts with ratings applied automatically
- 🤖 **AI assistant built in** — chat drafts and edits your drawing via MCP; every AI turn is undoable, with live edit-region highlighting
- 🔌 **Automate everything** — MCP server, REST API + SSE, and a `madake` CLI, all driving the same command engine
- 📥 **KiCad import** — migrate existing `.kicad_sch` schematics with a detailed import report

## Quick Start

Prerequisites: [Rust](https://rustup.rs), Node.js 20+. Optional: [ngspice](https://ngspice.sourceforge.io/) (circuit-solved verification), [Claude Code](https://claude.com/claude-code) (AI chat).

```bash
git clone <this repository>
cd MadakeCAD
npm install
npm run tauri dev
```

Full instructions, optional components, and troubleshooting: **[Getting Started](docs/02-getting-started.md)**.

## Documentation

Read in order — files are numbered:

| # | Document | What you'll learn |
|---|---|---|
| 01 | [Overview](docs/01-overview.md) | What MadakeCAD is, why it exists, design goals |
| 02 | [Getting Started](docs/02-getting-started.md) | Install, build, run, troubleshoot |
| 03 | [Schematic Editor](docs/03-schematic-editor.md) | Canvas, tools, symbols, sheets, layers |
| 04 | [Standards & Output](docs/04-standards-output.md) | JIS frame, PDF/SVG, BOM, wire list |
| 05 | [Wire Management](docs/05-wire-management.md) | Colors, gauges, part numbers, harnesses |
| 06 | [Verification & Simulation](docs/06-verification-simulation.md) | ERC, electrical checks, DC analysis |
| 07 | [Parts Database](docs/07-parts-database.md) | Part master, ratings, procurement |
| 08 | [Import & Export](docs/08-import-export.md) | KiCad import, file formats |
| 09 | [AI Assistant](docs/09-ai-assistant.md) | Chat drafting, per-turn undo, providers |
| 10 | [Automation & APIs](docs/10-automation-api.md) | MCP tools, REST API, CLI |
| 11 | [Mechanical Integration](docs/11-mechanical-integration.md) | FreeCAD linkage |
| 12 | [Roadmap](docs/12-roadmap.md) | Milestones M1–M6 |

Each page has a Japanese twin (`*.ja.md`). Contributor/internal materials (architecture, data model, feature specs, design system) live in [`docs/internal/`](docs/internal/README.md).

## Architecture in one line

```
UI · AI chat · CLI · REST  →  Command(JSON)  →  engine (Rust)  →  patch  →  every client updates live
```

Every edit from every entry point is an undoable command against a single engine — that's what makes AI edits safe. Details: [internal/architecture.md](docs/internal/architecture.md).

## Status & Roadmap

**Alpha.** Milestone M1 (foundation) is essentially complete; see the [roadmap](docs/12-roadmap.md) for M2 (reference-drawing parity) through M6 (open-source release). Built with Tauri 2, Vue 3, and Rust.

## Contributing

The project follows a docs-first, design-first (Pencil), TDD workflow. Start with [`docs/internal/`](docs/internal/README.md) — architecture, change recipes, and feature specs are all there. English is canonical for documentation; Japanese versions are provided as `*.ja.md`.

## License

To be determined ahead of the open-source release (tracked in [M6](docs/internal/specs/m6-oss.md)). All dependencies are permissively licensed.
