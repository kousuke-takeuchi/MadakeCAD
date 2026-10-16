# Roadmap

**日本語: [12-roadmap.ja.md](12-roadmap.ja.md)** | ← [Mechanical Integration](11-mechanical-integration.md)

Milestones toward the vision (standards-compliant, AI-first, veteran-grade, mechanically integrated, open source). Detailed specs live in the internal [feature specs](internal/specs/README.md); the competitive coverage check against AutoCAD Electrical / EPLAN is in the [gap analysis](internal/specs/gap-analysis.md).

## M1 — Foundation ✅ (mostly complete)

Command engine with full undo/redo · Canvas editor · JIS frame · parametric terminal blocks/connectors · netlist · ERC + ngspice-backed electrical verification · DC operating-point simulation · SVG/PDF/BOM/wire-list output · parts database · KiCad import/export · DXF interop (ACADE/EPLAN) · AI chat (Claude Code) · MCP/REST/CLI automation

## M2 — Industrial drawing parity ✅ (mostly complete)

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
- ✅ **Six provider routes** (phase 3): GitHub Copilot CLI (the CLI's own GitHub sign-in, no token stored), any OpenAI-compatible endpoint (OpenAI / xAI / OpenRouter) with a one-click **Ollama (local)** preset that needs no key, and Google Gemini — all on the same tool bridge and the same keychain rule. Full drafting turns have been run through Ollama and the GitHub Copilot CLI (including a follow-up turn that kept its context); Gemini needs your own key to finish confirming. See [AI Assistant → Providers](09-ai-assistant.md#providers)
- ✅ **Tidy variants + comparison panel** (phase 4): pick 2–4 in the tidy popup and the sheet is copied once per variant (one undo step), each copy tidied by its own conversation in parallel; the *Tidy variants* dock panel lists every variant's status and tidy metrics against the original, switches sheets with *Show*, and *Adopt* writes the chosen copy back onto the original sheet under the original entity ids and removes the copies — one undo step, as is *Discard all*. See [AI Assistant](09-ai-assistant.md)
- Macro value sets and the rest of the drafting features are tracked under M4

## M4 — Industrial CAD core (ACADE/EPLAN benchmark) ✅ (phases 1-4 done)

- ✅ **Terminal charts** (phase 1): terminal strip editor with saddle jumpers and terminal checks, terminal chart (CSV / drawing sheet), graphical terminal connection diagram. Phase 5 (2026-09-28) completed the editor: sorting, renumbering, multi-level terminals added and removed in place, spare insertion, part assignment, accessories (counted in the BOM), inside/outside swap and go-to-drawing, each one undoable command
- ✅ **Extended report set** (phase 1): from–to wire list, cross-reference table, every report as a framed drawing sheet, and a combined PDF (cover → schematic sheets → reports)
- ✅ **Circuit macros** (phase 2): save a selection as a macro (base point = its bottom-left pin) in `~/MadakeCAD/macros` and insert it from the Macros tab with a ghost preview, `R` to rotate and `Tab` to switch variants. Designators are renumbered above the ones already in use and wire numbers cleared, so repeated inserts never collide; `⌘C` / `⌘V` share the mechanism, and one insert is one undo step
- ✅ **Coil ⇔ contact cross-references** (phase 2): symbols sharing a designator form one device, with an IEC 60947-1 contact map under the coil, the coil's address beside each contact, and ERC rules for overusing a contact configuration, orphan contacts and coils with no contact. The parts database carries the contact configuration (schema v3)
- ✅ **Search and navigation** (phase 2): project-wide ⌘F search (designator, part no., net name, wire number, text) with a docked result panel, a device navigator tree in the left panel, and Alt-click reference surfing — all four paths reveal by switching sheet, selecting and zooming
- ✅ **Symbol library** (phase 3): 47 bundled JIS C 0617 / IEC 60617 symbols (32 added) across nine categories, plus the parametric `connector_{n}p` / `terminal_block_{n}p` / `plc_di_{n}p` / `plc_do_{n}p` families. Every symbol carries pin connection directions, attribute slots and bilingual search keywords; multi-pole devices are numbered and conduct pole by pole. Shapes whose established form could not be confirmed (thermal overload relay, proximity/photoelectric sensors, timer contacts, solenoids, SPD, …) were deliberately left out for the symbol editor
- ✅ **Macro value sets** (phase 3): placeholders bound to the value or an attribute of specific entities in a macro, and named value sets that fill them all at once from the insert dialog — still one undo step. Older macro files without the fields load unchanged
- ✅ **PLC I/O** (phase 3): PLC module definitions in the parts database (schema v4, point count / direction / address scheme, samples for Mitsubishi, Siemens and Allen-Bradley), an I/O assignment table saved with the project with CSV in and out, address auto-numbering per maker scheme, ladder-page generation as a single undo step, target and wire number read back from the drawing, and a PLC I/O report. Exposed through the UI, IPC, the Link API, MCP tools and the CLI
- ✅ **Frame / title-block templates** (phase 4): the frame, title block and revision table are drawn from a JSON template with property binds (`{sheet.no}/{sheet.total}`, `{rev.latest}`, `{title_block.*}`, …). Two templates ship (`jis-a3` — byte-identical to the previous hard-coded output — and `jis-a4-portrait`), user templates live in `~/MadakeCAD/templates/frames/`, and the picker previews the sheet you are on. Switching is one command across every sheet, in the canvas and in SVG/PDF alike, undone with one `⌘Z`; paper size and zone divisions deliberately stay with the sheet
- ✅ **Symbol editor** (phase 4): a 2.5 mm-grid editor with an attribute palette, a connection-point table, derived 0/90/180/270 previews and pre-save checks that share their verdict with the core. "New from a copy of…" is the default route; symbols are saved as `SymbolDef` JSON in `~/MadakeCAD/symbols/` under a **User** category, ids colliding with bundled symbols are refused, and deleting one that is in use needs a force. Reachable from IPC, the Link API and MCP tools, so the AI assistant can build symbols too
- ✅ **2D panel layout** (phase 4): a `panel` sheet kind (`.mdkproj` format version 4; balloons took it to 6) with enclosure, DIN rail, duct, footprint and legend entities on the ordinary command path. Unplaced parts are derived from the schematic (one device per designator) and placed by click with rail snapping and `R` to rotate; balloons number the footprints in reading order and feed the BOM's item-number column; the panel layout table matches it row for row; and `panel.overlap` / `panel.off_rail` / `panel.rail_fill` / `panel.orphan_footprint` report into the verification panel. Footprint sizes come from the parts database (schema v6)
- ✅ **BOM enrichment** (2026-10-15): every BOM row looks its part number up in the parts database and adds maker, name, unit price, currency, amount and purchase link, with one total row per currency
- ✅ **Macro variants from the save dialog** (2026-10-15): save a selection as variant B–Z of an existing macro, with an overwrite prompt; placeholders follow symbols with the same designator
- ✅ **Reserved contacts** (2026-10-15): the device navigator lists the contacts of a relay's contact configuration that are not drawn yet; click or drag one onto the drawing to place it
- ✅ **PLC ladder styles and placement** (2026-10-15): horizontal-bus ladders, sharing the last I/O page and splitting modules across pages in split units, with the settings saved in the project (format 14)
- ✅ **Multi-core cables and cable list** (2026-10-15): cable definition lines that assign the crossed wires as cores, shield, length, core-overflow ERC, the cable list report and cable rows in the BOM (format 15)
- ✅ **Panel nameplates, dimensions, align and in-panel wiring** (2026-10-15): nameplates attached to footprints that can follow the schematic description, associative dimensions, align/distribute as one undo step, and the in-panel wiring diagram report (format 16)
- ✅ **Graphical frame editor** (2026-10-15): draw frames and title blocks on screen — rules, text, placeholders picked from a list, title cells, logo frame and zones — with editor-local undo and a pre-save check, saved as the same template JSON
- M4 has no deferred items left

## M5 — Mechanical CAD integration (FreeCAD) ✅ (M5-1 to M5-3 done; the 2D panel sheet stays under M4 §8)

- ✅ **Workbench skeleton** (M5-1): the `freecad-addon/` workbench "MadakeCAD Link" — connection settings, project overview and a netlist view that follows drawing edits live over the Link API's event stream. See [Mechanical Integration](11-mechanical-integration.md)
- ✅ **Part linking & wire-length write-back** (M5-2): Parts/Wires tabs in the workbench insert 3D models and link objects by `madake_id` (`Project.mech_links`), and measured route lengths are written back to the wires with a `freecad` source so hand edits never overwrite them silently. File format 3
- ✅ **Route sync & placements** (M5-3): route stubs generated from the netlist between linked parts, net highlighting in the 3D view, and placements synced back into `mech_links` (file format 4) for the future panel-layout sheet

## M6 — Open-source release ✅ (preparation done; publishing itself is a project decision)

- ✅ License: MIT OR Apache-2.0 dual. Repository hygiene: `scripts/audit_confidential.py` scans the tree and the whole history against a git-ignored term list before publishing
- ✅ Windows/Linux builds + CI: CI runs the Rust suites, the frontend tests and the add-on tests on Ubuntu, macOS and Windows and packages the bundles on all three (unsigned), PDF fonts follow the OS (Hiragino / Yu Gothic UI / Noto Sans CJK), and `release.yml` builds the `.dmg` / `.msi` / `.exe` / `.AppImage` / `.deb` / `.rpm` installers and attaches them to a GitHub release as soon as it is published (the `vX.Y.Z` tag sets the version)
- ✅ i18n: the remaining UI literals moved into the catalogs and six languages ship (English, Japanese, Chinese, Spanish, French, German) with key, placeholder and plural parity enforced by test; translator guide in the i18n spec. **English and Japanese are complete; zh/es/fr/de are roughly 60% translated** and the untranslated keys render the English string
- ✅ Community: `CONTRIBUTING.md`, Contributor Covenant code of conduct, `SECURITY.md`, issue and pull-request templates
- ✅ Release runbook: `docs/internal/release-runbook.md` lists the steps the user performs by hand (hygiene audit, visibility, topics, tag, bundles, first response). **Nothing in it has been executed**
- Open: how to publish (this repository vs. an export without history), the project name, and the timing

## Long term

- IEC / ISO standard variants: IEC 60617 symbols, IEC 81346 structure designations, ISO 7200 title blocks, ISO 5457 sheet frames

Deliberately deferred (with rationale recorded in the gap analysis): transient simulation, single-line diagrams, native DWG/EPLAN project files (DXF interop is done, see [Import & Export](08-import-export.md)), multi-user editing.
