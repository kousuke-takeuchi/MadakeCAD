# Competitive Gap Analysis (AutoCAD Electrical / EPLAN)

[日本語](gap-analysis.ja.md) | Created: 2026-08-21

Purpose: verify that the feature specs (M2–M6) and public feature docs have **no missing essential features** compared with the benchmark products, and check internal consistency across our documents.

Benchmark sources (researched 2026-08-21): [ACADE toolset (Autodesk)](https://www.autodesk.com/jp/products/autocad-plus/included-toolsets/autocad-electrical), [ACADE product overview (Otsuka)](https://www.cadjapan.com/products/items/autocad_electrical/point.html), [EPLAN Electric P8](https://www.eplan-software.com/solutions/eplan-electric-p8/), [EPLAN P8 Performance Description 2024](https://www.eplan.com/content/dam/eplan/corporate/performance-descriptions/2024/en/performance-description-eplan-electric-p8.pdf).

## 1. Coverage matrix

| Essential feature (benchmark) | ACADE | EPLAN | MadakeCAD | Where |
|---|---|---|---|---|
| Schematic capture w/ standard symbol library | ✅ | ✅ | ✅ | shipped |
| Automatic component tagging (designators) | ✅ | ✅ | ✅ | shipped |
| Wire numbering (sequential) | ✅ | ✅ | 🔜 M2 | specs/m2 |
| Wire numbering (reference-based) | ✅ | ✅ | 🔜 M4 | specs/m4 §6 |
| Terminal strip management & jumpers | ✅ | ✅ | 🔶 shipped (feed-through) / 🔜 M4 (charts, jumpers) | specs/m4 §1 |
| Terminal diagrams/charts | ✅ | ✅ | 🔜 M4 | specs/m4 §1 |
| Circuit reuse (macros / circuit builder) | ✅ | ✅ (core workflow) | 🔜 M4 | specs/m4 §2 |
| PLC I/O drawings & reports | ✅ | ✅ | 🔜 M4 | specs/m4 §3 |
| Coil ⇔ contact cross-references | ✅ | ✅ | 🔜 M4 | specs/m4 §4 |
| Cross-sheet (signal) references | ✅ | ✅ | 🔜 M2 | specs/m2 §4 |
| Report set (BOM, from–to, cable, terminal, XRef) | ✅ | ✅ | 🔶 BOM/wire list shipped / 🔜 M4 | specs/m4 §5 |
| Parts catalog / master data | ✅ | ✅ | ✅ | shipped |
| Revision table | ✅ | ✅ (+compare) | 🔜 M2 | specs/m2 §1 |
| ERC / design checks | ✅ | ✅ | ✅ (+ real SPICE-backed checks — differentiator) | shipped |
| Symbol editor | ✅ | ✅ | 🔜 M4 | specs/m4 §7 |
| **2D panel layout drawings** | ✅ (core) | ✅ (Pro Panel) | ⚠ **GAP** (only 3D via FreeCAD M5-3) | → added specs/m4 §8 |
| **Customizable frame/title-block templates** | ✅ | ✅ | ⚠ **GAP** (JIS frame hardcoded) | → added specs/m4 §9 |
| Multi-core cable management | ✅ | ✅ | 🔜 M4 (open item) | specs/m4 |
| Project-wide device search/navigation | ✅ | ✅ | ⚠ minor gap | → added specs/m4 §10 |
| Structure identifiers (IEC 81346 =+-) | 🔶 | ✅ | ⚠ long-term (G1) | → recorded below |
| DWG/DXF interop | ✅ (native) | ✅ | ⚠ backlog candidate | → recorded below |
| Single-line diagrams | 🔶 | ✅ | ❌ not planned | decision recorded below |
| Auto-connecting wires (UX aid) | 🔶 | ✅ | ❌ backlog (AI tidy-up partially covers) | recorded below |
| Multi-user / rights management | ✅ | ✅ | ❌ non-goal (local-first) | requirements §6 |

## 2. Newly identified gaps and dispositions

1. **2D panel layout** — a core ACADE deliverable (enclosure layout with footprints, DIN rails, ducts). Previously only reachable via FreeCAD 3D (M5-3). **Disposition: added to M4 spec as §8 (native 2D panel sheet), with FreeCAD sync deferred to M5-3.**
2. **Frame/title-block templates** — our JIS frame layout is hardcoded; benchmark tools let organizations use their own formats, and this is a prerequisite for broad OSS adoption. **Disposition: added to M4 spec as §9.**
3. **Project-wide search/navigation** (find device by designator, jump between sheets). **Disposition: added to M4 spec as §10 (small).**
4. **IEC 81346 structure identifiers** (`=` function, `+` location, `-` device) — EPLAN's structuring backbone. **Disposition: recorded as long-term G1 roadmap item (post-M4); revisit with IEC symbol variants.**
5. **DWG/DXF export** — useful for hand-off to AutoCAD-based reviewers. **Disposition: backlog candidate for M6+; not essential for the primary workflows (PDF covers review).**
6. **Single-line diagrams** — power-distribution style representation. **Disposition: explicitly out of scope for now (our domain is control wiring diagrams); revisit if demanded.**
7. **Auto-connect wiring aids** — EPLAN auto-connects aligned symbols. **Disposition: backlog UX item; AI tidy-up (M3) covers part of the need.**

## 3. Consistency findings (our documents)

| Finding | Resolution |
|---|---|
| `docs/internal/feature-inventory.md` §10 still proposed A–D directions, superseded by milestones M2–M6 | §10 now points to `docs/internal/specs/` |
| Master spec §6 phase plan described phases 0–3 as future | Annotated as historical; milestones are canonical (requirements §2) |
| View classes: M2 adds wire-number (and possibly harness) classes on top of today's 7 | Noted in m2 spec; UI copy must stay in sync |
| Public feature docs vs. specs | Written from the same source (features inventory + specs); each "Planned" item carries its milestone marker |

## 4. Verdict

With §2 dispositions applied, the M2–M6 specs cover all benchmark-essential features except items consciously deferred (single-line diagrams, DWG interop, multi-user) — each with a recorded rationale. The differentiators not present in the benchmarks are: ngspice-grounded verification, AI-first drafting with per-turn undo, and a fully scriptable Command API.
