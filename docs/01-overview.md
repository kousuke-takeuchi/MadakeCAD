# Overview

**日本語: [01-overview.ja.md](01-overview.ja.md)** | Next: [Getting Started →](02-getting-started.md)

## What is MadakeCAD?

MadakeCAD is an electrical CAD for **industrial equipment wiring diagrams** — robots, factory machinery, control panels. It produces drawings that comply with industrial standards (JIS drawing frames, reference designators, wire management) and treats an **AI assistant as a first-class user**: the assistant edits drawings through exactly the same command engine as you do, so every AI edit is undoable and rendered live.

## Why another CAD?

- **PCB CADs are the wrong tool.** Teams often draw equipment wiring in PCB tools like KiCad, but industrial drawings need JIS frames (title blocks, revision tables, zone references), wire part management (color/gauge/length → part number), terminal-block-centric connections, and wire lists — none of which PCB CADs provide
- **Industrial CADs are proprietary and heavyweight.** AutoCAD Electrical and EPLAN define the professional feature bar, but they are closed, expensive, and have no meaningful AI integration
- **Expertise shouldn't be the entry ticket.** With LLM integration, someone who doesn't know the drafting conventions should still be able to produce a correct, standards-compliant drawing — while veterans get a fast, conventional CAD experience

## Design goals (the five pillars)

1. **Standards compliance** — JIS today, IEC variants long-term
2. **AI-first for non-experts** — conversational drafting with built-in verification
3. **Veteran-grade CAD** — benchmarked against AutoCAD Electrical / EPLAN
4. **Mechanical CAD integration** — FreeCAD round-trip (part linking, wire lengths)
5. **Open source** — local-first, cross-platform, community-driven

## What makes it different

- **Verification grounded in a real solver**: electrical checks (voltage drop, ampacity, fuse ratings) are judged from an actual ngspice DC solution of your drawing, not just heuristics
- **One command engine, many entry points**: UI, AI chat, CLI, and REST API all perform the same undoable commands with live updates everywhere
- **Local-first**: your drawings are pretty-printed JSON on your disk; the parts database is a local SQLite file; nothing leaves your machine

## Current status

Milestone M1 (foundation) is essentially complete: editor, JIS frame, PDF/BOM/wire-list output, ERC + electrical verification, DC simulation, KiCad import, and AI chat all work. See the [roadmap](12-roadmap.md) for what's next.
