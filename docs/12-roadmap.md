# Roadmap

**日本語: [12-roadmap.ja.md](12-roadmap.ja.md)** | ← [Mechanical Integration](11-mechanical-integration.md)

Milestones toward the vision (standards-compliant, AI-first, veteran-grade, mechanically integrated, open source). Detailed specs live in the internal [feature specs](internal/specs/README.md); the competitive coverage check against AutoCAD Electrical / EPLAN is in the [gap analysis](internal/specs/gap-analysis.md).

## M1 — Foundation ✅ (mostly complete)

Command engine with full undo/redo · Canvas editor · JIS frame · parametric terminal blocks/connectors · netlist · ERC + ngspice-backed electrical verification · DC operating-point simulation · SVG/PDF/BOM/wire-list output · parts database · KiCad import/export · DXF interop (ACADE/EPLAN) · AI chat (Claude Code) · MCP/REST/CLI automation

## M2 — Reference-drawing parity ✅ (mostly complete)

- Revision table rendering (canvas, SVG, PDF) and the editing dialog, with the title-block revision mark following the newest row
- Wire numbers: net-scoped numbering (top-up / renumber), on-drawing labels with their own view class, per-net editing, wire-list column, `madake renumber` on the CLI
- Harness boundaries: dashed enclosures with a name; enclosed wires get a harness column in the wire list
- Cross-sheet references: project-wide nets from same-named labels, destination `/sheet.zone` addresses on the drawing, click-through in the properties panel, one net in reports and ERC

## M3 — AI-first drafting ✅ (phases 1-4 done)

- ✅ **Robust turn management** (phase 1): stable turn IDs, user/agent/mcp edit origins, turn-revert that undoes agent edits only (manual edits survive; conflicts are reported instead of half-reverted), turn sequence numbers that discard events from a cancelled turn
- ✅ **Standards knowledge + verification loop** (phase 1): a bundled editable standards note injected every turn, a drawing context carrying the verification summary, and a mandatory verify → fix → re-verify loop (up to three rounds) reported as error/warning counts
- ✅ **Knowledge answers, design review, part selection** (phase 1): documentation answers with sources (roadmap answers for unsupported features), a severity-ranked review combining ERC with a human-convention checklist, test-procedure tables, and part comparison tables from the parts database
- ✅ **Start templates** (phase 1): three bundled circuits (24 V control basics, motor starter, emergency stop) applied as a single undo step, plus user templates from `~/MadakeCAD/templates`
- ✅ **Tidy-up passes** (phase 2): deterministic tidy metrics in the core (wire crossings, label and symbol overlaps, off-grid points) exposed as a tool, and a Layout / Wiring / Labels popup that sends a measure → fix → re-measure loop as a **single turn**, so one undo takes the whole pass back
- ✅ **Parallel conversations** (phase 2): conversations run at the same time, each with its own colour for the edit overlay, the conversation list and the "N running" badge. Reverting a turn whose edits interleaved with another conversation's turn takes both back, and marks the swept turn reverted
- ✅ **Anthropic API provider + OS keychain** (phase 2): a backend trait with a Claude Code CLI and a Messages API implementation sharing one tool bridge, the API key kept only in the OS keychain, and a provider settings page with a masked key field and a connection test
- ✅ **Six provider routes** (phase 3): GitHub Copilot CLI (the CLI's own GitHub sign-in, no token stored), any OpenAI-compatible endpoint (OpenAI / xAI / OpenRouter) with a one-click **Ollama (local)** preset that needs no key, and Google Gemini — all on the same tool bridge and the same keychain rule. A full drafting turn has been run through Ollama; Copilot and Gemini need your own credentials to finish confirming. See [AI Assistant → Providers](09-ai-assistant.md#providers)
- ✅ **Tidy variants + comparison panel** (phase 4): pick 2–4 in the tidy popup and the sheet is copied once per variant (one undo step), each copy tidied by its own conversation in parallel; the *Tidy variants* dock panel lists every variant's status and tidy metrics against the original, switches sheets with *Show*, and *Adopt* writes the chosen copy back onto the original sheet under the original entity ids and removes the copies — one undo step, as is *Discard all*. See [AI Assistant](09-ai-assistant.md)
- Macro value sets and the rest of the drafting features are tracked under M4

## M4 — Industrial CAD core (ACADE/EPLAN benchmark) 🔶 (phases 1 and 2 done)

- ✅ **Terminal charts** (phase 1): terminal strip editor with saddle jumpers and terminal checks, terminal chart (CSV / drawing sheet), graphical terminal connection diagram. Sorting, multi-level terminals and accessories need a model extension and come later
- ✅ **Extended report set** (phase 1): from–to wire list, cross-reference table, every report as a framed drawing sheet, and a combined PDF (cover → schematic sheets → reports)
- ✅ **Circuit macros** (phase 2): save a selection as a macro (base point = its bottom-left pin) in `~/MadakeCAD/macros` and insert it from the Macros tab with a ghost preview, `R` to rotate and `Tab` to switch variants. Designators are renumbered above the ones already in use and wire numbers cleared, so repeated inserts never collide; `⌘C` / `⌘V` share the mechanism, and one insert is one undo step
- ✅ **Coil ⇔ contact cross-references** (phase 2): symbols sharing a designator form one device, with an IEC 60947-1 contact map under the coil, the coil's address beside each contact, and ERC rules for overusing a contact configuration, orphan contacts and coils with no contact. The parts database carries the contact configuration (schema v3)
- ✅ **Search and navigation** (phase 2): project-wide ⌘F search (designator, part no., net name, wire number, text) with a docked result panel, a device navigator tree in the left panel, and Alt-click reference surfing — all four paths reveal by switching sheet, selecting and zooming
- Remaining for later phases: PLC I/O drawings & reports; macro value sets and reserved contacts placed from the navigator; cable summary; BOM enriched from the parts database; 2D panel layout sheets; customizable frame/title-block templates; symbol editor + expanded symbol library

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

Deliberately deferred (with rationale recorded in the gap analysis): transient simulation, single-line diagrams, native DWG/EPLAN project files (DXF interop is done, see [Import & Export](08-import-export.md)), multi-user editing.
