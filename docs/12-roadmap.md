# Roadmap

**日本語: [12-roadmap.ja.md](12-roadmap.ja.md)** | ← [Mechanical Integration](11-mechanical-integration.md)

Milestones toward the vision (standards-compliant, AI-first, veteran-grade, mechanically integrated, open source). Detailed specs live in the internal [feature specs](internal/specs/README.md); the competitive coverage check against AutoCAD Electrical / EPLAN is in the [gap analysis](internal/specs/gap-analysis.md).

## M1 — Foundation ✅ (mostly complete)

Command engine with full undo/redo · Canvas editor · JIS frame · parametric terminal blocks/connectors · netlist · ERC + ngspice-backed electrical verification · DC operating-point simulation · SVG/PDF/BOM/wire-list output · parts database · KiCad import · AI chat (Claude Code) · MCP/REST/CLI automation

## M2 — Reference-drawing parity

- Revision table rendering and editing
- Wire numbers (auto-numbering, on-drawing labels, wire-list integration)
- Harness boundaries (dashed enclosures feeding the wire list)
- Cross-sheet references (project-wide nets, destination zone labels)

## M3 — AI-first drafting

- Robust turn management (stable IDs, user/agent edit origins)
- Standards knowledge injection + self-verification loop
- Circuit templates; tidy-up automation (placement/wiring/labels)
- Parallel agents; multi-provider LLM support with OS-keychain credentials

## M4 — Industrial CAD core (ACADE/EPLAN benchmark)

- Terminal charts; circuit macros; PLC I/O drawings & reports
- Coil ⇔ contact cross-references; extended report set (from–to, cables, enriched BOM)
- 2D panel layout sheets; customizable frame/title-block templates
- Symbol editor + expanded symbol library; project-wide search

## M5 — Mechanical CAD integration (FreeCAD)

- Workbench with live netlist view
- Part linking (`madake_id`) and wire-length write-back
- Route visualization and panel-layout correspondence

## M6 — Open-source release

- License decision, repository hygiene audit
- Windows/Linux builds + CI, installers
- Remaining i18n sweep + Chinese/Spanish/French/German catalogs (the i18n foundation — English default + Japanese, message catalog, language setting — lands early, before M6), contributor docs and community setup

## Long term

- IEC / ISO standard variants: IEC 60617 symbols, IEC 81346 structure designations, ISO 7200 title blocks, ISO 5457 sheet frames

Deliberately deferred (with rationale recorded in the gap analysis): transient simulation, single-line diagrams, DWG/DXF export, multi-user editing.
