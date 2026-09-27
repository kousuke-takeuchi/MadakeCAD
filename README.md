<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/images/lockup-dark.png">
  <img src="docs/images/lockup-light.png" alt="MadakeCAD" width="300">
</picture>

**An AI-first electrical CAD for industrial equipment, built for JIS / IEC / ISO compliance.**

Draw standards-compliant wiring diagrams for robots, machinery, and control panels —
with an AI assistant that edits through the same undoable command engine as you,
and verification grounded in a real circuit solver (ngspice).

[![CI](https://github.com/kousuke-takeuchi/MadakeCAD/actions/workflows/ci.yml/badge.svg)](https://github.com/kousuke-takeuchi/MadakeCAD/actions/workflows/ci.yml)
[![Spec clauses](https://img.shields.io/badge/spec_clauses-570_tested-blue)](docs/13-specification.md)
![Status](https://img.shields.io/badge/status-alpha-orange)
![Platform](https://img.shields.io/badge/platform-macOS_(Win%2FLinux_planned)-lightgrey)
![Built with](https://img.shields.io/badge/built_with-Tauri_2_·_Vue_3_·_Rust-24C8DB)
![Docs](https://img.shields.io/badge/docs-EN_%7C_JA-informational)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-green)](#license)

[Overview](docs/01-overview.md) · [Getting Started](docs/02-getting-started.md) · [Documentation](#documentation) · [Specification](docs/13-specification.md) · [Roadmap](docs/12-roadmap.md) · [日本語](README.ja.md)

<img src="docs/images/sample-drawing.svg" alt="Sample drawing: 48V power and control diagram on a JIS A3 frame" width="820">

*A drawing produced by MadakeCAD: JIS A3 frame with zone references and title block, a revision table, net-based wire numbers, a dashed harness boundary, and cross-sheet references next to the net labels.*

</div>

---

## Why MadakeCAD?

- 🏭 **PCB CADs are the wrong tool for equipment wiring** — no standard drawing frames (JIS/ISO), no wire part management, no terminal-block-centric connections, no wire lists
- 💰 **Industrial CADs (AutoCAD Electrical, EPLAN) are closed and heavyweight** — and have no meaningful AI integration
- 🤝 **Expertise shouldn't be the entry ticket** — with an LLM in the loop, non-experts produce correct drawings while veterans keep a fast, conventional CAD workflow

## Highlights

- ✏️ **Purpose-built schematic editor** — JIS C 0617 symbols, parametric terminal blocks & connectors (1–50 poles), orthogonal wiring with grid/pin snap, auto reference designators, multi-sheet, layers, reusable circuit macros, coil ⇔ contact cross-references, and project-wide search with a device navigator
- 📐 **Standards-true output** — JIS drawing frame today (zones, title block), IEC 60617 / IEC 81346 / ISO 7200-series variants on the roadmap; print-quality SVG/PDF with embedded CJK fonts, and five reports (from–to wire list, terminal chart, terminal connection diagram, BOM, cross-reference) as CSV, as framed drawing sheets, or bound into one PDF with the schematics
- ✅ **Solver-grounded verification** — ERC plus electrical checks (reachability, ampacity, voltage drop, fuse rating) judged from an actual ngspice DC solution
- ⚡ **DC simulation** — net voltages, component currents/power, open-switch what-if analysis
- 🗄️ **Parts database** — local SQLite master with ratings, procurement links, and a wire part master; place parts with ratings applied automatically
- 🤖 **AI assistant built in** — chat drafts and edits your drawing via MCP, knows the JIS/IEC drafting conventions, and verifies its own work (ERC → fix → re-verify) before reporting back; every AI turn is undoable and never swallows your manual edits. One-click tidy-up passes, several conversations working in parallel, and six providers to choose from — Claude Code CLI, Anthropic API, GitHub Copilot CLI, any OpenAI-compatible endpoint, a local Ollama (no key, nothing leaves the machine), or Google Gemini
- 🔌 **Automate everything** — MCP server, REST API + SSE, and a `madake` CLI, all driving the same command engine

## Quick Start

Prerequisites: [Rust](https://rustup.rs), Node.js 20+. Optional: [ngspice](https://ngspice.sourceforge.io/) (circuit-solved verification), and — for the AI chat — **any one** of six providers: [Claude Code](https://claude.com/claude-code) (the default), an Anthropic API key, GitHub Copilot CLI, an OpenAI-compatible endpoint, a local [Ollama](https://ollama.com), or a Gemini key. API keys are kept in your OS keychain; see [AI Assistant → Providers](docs/09-ai-assistant.md#providers).

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
| 04 | [Standards & Output](docs/04-standards-output.md) | JIS frame, PDF/SVG, reports, combined PDF |
| 05 | [Wire Management](docs/05-wire-management.md) | Colors, gauges, part numbers, harnesses, terminal charts |
| 06 | [Verification & Simulation](docs/06-verification-simulation.md) | ERC, electrical checks, DC analysis |
| 07 | [Parts Database](docs/07-parts-database.md) | Part master, ratings, procurement |
| 08 | [Import & Export](docs/08-import-export.md) | KiCad import/export, DXF interop with AutoCAD Electrical / EPLAN, file formats |
| 09 | [AI Assistant](docs/09-ai-assistant.md) | Chat drafting, standards knowledge, verification loop, templates, turn revert, tidy-up passes, parallel conversations, providers |
| 10 | [Automation & APIs](docs/10-automation-api.md) | MCP tools, REST API, CLI |
| 11 | [Mechanical Integration](docs/11-mechanical-integration.md) | FreeCAD add-on "MadakeCAD Link" (netlist view, live follow), linkage roadmap |
| 12 | [Roadmap](docs/12-roadmap.md) | Milestones M1–M6 |
| 13 | [Detailed Specification](docs/13-specification.md) | **Generated from the test suite** — every clause is machine-verified |

Each page has a Japanese twin (`*.ja.md`). Contributor/internal materials (architecture, data model, feature specs, design system) live in [`docs/internal/`](docs/internal/README.md).

## Architecture in one line

```
UI · AI chat · CLI · REST  →  Command(JSON)  →  engine (Rust)  →  patch  →  every client updates live
```

Every edit from every entry point is an undoable command against a single engine — that's what makes AI edits safe. Details: [internal/architecture.md](docs/internal/architecture.md).

## Status & Roadmap

**Alpha.** Milestones M1 (foundation) and M2 (reference-drawing parity: revision table, wire numbers, harness boundaries, cross-sheet references) are essentially complete, and M3 phase 1 (standards knowledge, self-verification loop, safe turn revert, start templates) and M4 phases 1 and 2 (terminal charts + the extended report set; circuit macros, coil ⇔ contact cross-references, search and navigation) have landed; see the [roadmap](docs/12-roadmap.md) for the rest of M3 through M6 (open-source release). Built with Tauri 2, Vue 3, and Rust.

## Contributing

The project follows a docs-first, design-first (Pencil), TDD workflow. Start with [`docs/internal/`](docs/internal/README.md) — architecture, change recipes, and feature specs are all there. English is canonical for documentation; Japanese versions are provided as `*.ja.md`.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
