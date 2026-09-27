# Interop with KiCad / EPLAN / AutoCAD Electrical (import & export)

[日本語](interop.ja.md)

Created: 2026-09-27. Pillar: G3 (veteran-grade, ACADE/EPLAN benchmark) and G5 (open source: no lock-in).
Status: **implemented** (KiCad import/export, DXF import/export). Items marked "open" at the end are not implemented.

## 1. Purpose

Users migrating from KiCad, or exchanging drawings with customers who use AutoCAD Electrical (ACADE) or EPLAN, need to bring drawings in and hand drawings out without redrawing them. This spec fixes which file formats are realistic for each ecosystem and how MadakeCAD's model maps onto them.

## 2. Format boundaries (what is and is not possible)

| Ecosystem | Native project format | Open / documented? | What MadakeCAD uses |
|---|---|---|---|
| KiCad | `.kicad_sch` (S-expression text) | Yes (public format, versioned) | Import **and export** of `.kicad_sch` directly |
| AutoCAD Electrical | `.dwg` drawings + `.wdp` project list + MDB/SQLite catalog | No (`.dwg` is proprietary binary) | **DXF** (ASCII, AutoCAD 2000 = AC1015). ACADE reads and writes DXF natively (`DXFIN`/`DXFOUT`, or "Save as DXF") |
| EPLAN Electric P8 | `.elk` / `.zw1` (proprietary database) | No | **DXF**. EPLAN imports/exports DXF/DWG per page ("Page > Export > DXF/DWG") |

Consequences:

- There is no MadakeCAD-side reader or writer for `.dwg`, `.elk` or `.zw1`, and none is planned: the formats are undocumented and change per release. Converting `.dwg` to DXF is done in AutoCAD, in EPLAN, or with the free ODA File Converter before import.
- DXF carries **geometry plus block attributes**, not the electrical model. Import therefore reconstructs wires from lines on wire layers and symbols from block inserts; everything without a mapping is reported as skipped, exactly like the KiCad importer.

## 3. User stories

1. As a KiCad user, I export a MadakeCAD sheet to `.kicad_sch`, open it in KiCad without installing any library, and run `kicad-cli sch erc` as an independent cross-check.
2. As a designer receiving an ACADE drawing, I ask the customer for a DXF, open it in MadakeCAD, and get the wires, wire numbers and notes, plus a list of the blocks that could not be mapped so I can place them by hand.
3. As a designer handing a drawing to an EPLAN/ACADE reviewer, I export the sheet as DXF and the reviewer opens it with layers (`WIRES`, `WIRENO`, `TAGS`, …) they recognise.
4. As an automation user, I do all of the above from the CLI, the REST Link API and the MCP server.

## 4. Functional specification

### 4.1 KiCad export (`madake_core::kicad::export_kicad_sch`)

- One `.kicad_sch` (KiCad 9 format, version `20250114`) per sheet: paper size/orientation, title block (title, date, effective rev, company).
- Every symbol used on the sheet is embedded once in `lib_symbols` as `MadakeCAD:<symbol_id>` with its graphics (polyline/circle/arc/rectangle/text, Y flipped to KiCad's symbol-library convention) and pins (length 0, connection point = pin position). KiCad opens the file with no external library.
- Symbol instances carry `Reference`/`Value` properties, extra attributes as hidden properties, rotation and `(mirror y)` written with the same convention the importer reads, so an export/import round trip keeps placement.
- Wires: one two-point `wire` per polyline segment. Junctions, net labels, texts map 1:1.
- Wire numbers: KiCad has no wire numbers; the number becomes a **label on the longest segment of the net** (only when the net has no label already), so it survives as the net name in KiCad and on re-import.
- Harness boundaries: dashed closed `polyline` plus a `text` with the name (KiCad has no equivalent object).
- Importer change: `lib_id` values starting with `MadakeCAD:` resolve directly to the symbol id (static and parametric ids), so MadakeCAD's own exports round-trip without the lib_id mapping table.

### 4.2 DXF export (`madake_core::dxf::sheet_to_dxf`, `project_sheet_to_dxf`)

ASCII DXF, `$ACADVER` = AC1015, `$INSUNITS` = 4 (mm), non-ASCII text written as `\U+XXXX` escapes (portable across code pages). Y axis flipped (`y_dxf = paper height − y`), paper outline on layer `FRAME` so extents match the paper. Because the Y flip mirrors the plane, **every angle is written negated** (group 50 = `-θ`, arc sweeps swapped): otherwise a 90°/270° symbol lands 180° off and its pins leave the wires. Returns `(String, io::ExportReport)` — the count per entity kind plus what could not be written; `project_sheet_to_dxf` is the project-level entry point (frame template of the project, sheet number/total in the title block binds, `Project::wire_parts` for the wire legend).

| MadakeCAD | DXF | Layer |
|---|---|---|
| Wire (each segment) | `LINE` (ACADE recognises LINE entities on a wire layer as wires) | `WIRES` |
| Wire number | `TEXT` above a horizontal / left of a vertical longest segment | `WIRENO` |
| Symbol | `INSERT` of block `MDK_<symbol_id>` (rotation = group 50, mirror = X scale −1) with attributes `TAG1` (reference), `CAT` (value / part no.), `DESC1`, `RATING1`, other attrs uppercased, `TERMnn` (pin numbers, invisible) | `SYMS` (block), `TAGS`, `DESC` |
| Symbol block graphics | `LINE`/`LWPOLYLINE`/`CIRCLE`/`ARC`/`SOLID`/`TEXT` in local coordinates, Y flipped; `ATTDEF`s for the attributes above | `SYMS` |
| Junction | `INSERT` of block `WDDOT` (ACADE's wire-dot block name) | `WIRES` |
| Net label | `TEXT` | `LABELS` |
| Text | one `TEXT` per line | `MISC` |
| Harness | closed `LWPOLYLINE` with linetype `DASHED` + name `TEXT` | `HARNESS` |
| Frame (border, zone ticks + labels, title block cells with binds resolved, logo box) | `LWPOLYLINE`/`LINE`/`TEXT` from the frame template the SVG exporter uses | `FRAME` |
| Shape (G10) | `LINE`/`LWPOLYLINE`/`CIRCLE`, linetype `DASHED` when dashed (fills not written) | `SHAPES` |
| Wire part legend | `LWPOLYLINE` frame + `LINE` grid + `TEXT` rows (rows from `wire_legend::legend_rows_with`) | `ANNOTATION` |
| Panel layout (`PanelOutline` incl. mounting plate, `DinRail`, `Duct` + hatching, `Footprint` + reference / part no. / balloon, `PanelLegend` table) | `LWPOLYLINE`/`LINE`/`CIRCLE`/`TEXT` with the same geometry helpers as the SVG exporter (`crate::panel`) | `PANEL` |

The revision table is the one frame part not written (the latest revision reaches the title block through the `rev.latest` bind). Everything else that is on the sheet is written: DXF is a general 2D drawing format, so a shape, a legend or a panel layout does have an equivalent there, and dropping it silently would break the very user story of §3.3. Anything that still cannot be written (a symbol whose definition is missing) is listed in the export report.

Block naming does **not** try to imitate ACADE's family codes (`HCR1`, `VCR1`, …) or EPLAN's symbol numbers: those tables are product-specific and undocumented. The attribute tags (`TAG1`, `CAT`, `DESC1`, `RATING1`, `TERMnn`) follow ACADE's attribute conventions so ACADE tools that read attributes (tag/catalog reports) see the values.

### 4.3 DXF import (`madake_core::dxf::import_dxf`)

- Accepts any ASCII DXF with an `ENTITIES` section (R12 and later). `$INSUNITS` = 1 (inches) is scaled ×25.4; everything else is read as mm.
- Paper: the header drawing limits decide it — `$LIMMIN`/`$LIMMAX`, else `$EXTMIN`/`$EXTMAX`, else (only when both are absent) the extents of all entities. Size = the smallest ISO A size (A4…A0, portrait when taller than wide) that contains that rectangle; the drawing is placed with the rectangle's top-left at the paper origin. Without the header rule an ACADE A3 page whose content covers 200×100 mm would import as A4 with everything in a corner. Files exported by MadakeCAD round-trip exactly (their header and the `FRAME` outline both give the paper).
- Wires: `LINE`/`LWPOLYLINE`/`POLYLINE` on **wire layers**. Wire layers are (a) those given in `wire_layers`, else (b) layers whose name contains `WIRE` but not `WIRENO`/`WIRE_NO`/`WIRENUM` (matches ACADE's `WIRES`, `_MULTI_WIRE_n`). If no line sits on such a layer the two cases differ: with **no** `wire_layers` given, all lines become wires and a warning names the layers seen; with `wire_layers` **given**, no wire is imported and the warning names both the layers asked for and the layers the file has. An explicit narrowing must never widen into "every line is a wire" (the widest possible reading), because those wires feed the netlist, ERC, wire list and BOM. Either way the layer names also come back machine-readable in `ImportReport::layers`, which the ribbon turns into one "re-import with this wire layer" button per layer.
- Junctions: `INSERT` of `WDDOT`, or a `CIRCLE` of radius ≤ 1 mm **on a genuine wire layer** — never under the all-lines fallback, where the pin circles and terminal dots of a foreign drawing would become junctions and merge nets silently.
- Symbols: `INSERT` of `MDK_<symbol_id>` (static or parametric id). Attributes `TAG1`/`TAG` → reference, `CAT` → value, `DESC1`/`RATING1` → the DESC/RATING attribute slots, other tags → attrs, `TERMnn` ignored. Rotation must be a multiple of 90° (otherwise 0 with a warning); negative X scale → mirror. Any other block name is skipped and listed as `NAME (TAG1) xN`.
- Text position: group 10/20, except a `TEXT` with a non-zero justification (group 72 or 73), whose insertion point is group 11/21 (ACADE's centred wire numbers). The extents scan reads the same point.
- Text: layer containing `WIRENO` → wire number of the nearest wire within 5 mm (else a note plus warning); layer `LABELS` → net label; layer `HARNESS` → name of the harness whose top-left corner is within 5 mm; anything else → note (height from group 40). `MTEXT` paragraph breaks (`\P`) and formatting codes are decoded, `\U+XXXX` is decoded.
- The `FRAME` layer (lines and texts: the frame belongs to the sheet's frame template, not to the drawing) and closed polylines on wire layers are ignored; unsupported entity kinds are counted in the skipped list.
- Result: a one-sheet project plus the same `ImportReport` structure as the KiCad importer (counts, skipped, warnings).

### 4.4 Exposure

| Surface | Import DXF | Export DXF | Export KiCad |
|---|---|---|---|
| App | Open dialog (`.dxf` filter) and ribbon **Import/Export** tab (KiCad / DXF import, DXF / KiCad / SVG / PDF export, report CSVs). Imports confirm unsaved changes first and leave the project without a save path, like the KiCad import | same tab (active sheet) | same tab |
| REST | `POST /api/v1/import/dxf` `{path, wire_layers?}` | `POST /api/v1/export/dxf` `{sheet_id?, path}` → `{written, report}` | `POST /api/v1/export/kicad` `{sheet_id?, path}` → `{written, report}` |
| MCP | `import_dxf` | `export_dxf` | `export_kicad` |
| CLI | `madake open x.dxf [--wire-layer WIRES,_MULTI_WIRE_1]` | `madake export dxf out.dxf [--sheet ID]` | `madake export kicad out.kicad_sch [--sheet ID]` |

Both exports return the export report (§4.2) next to the written path; the app logs the counts and the not-written kinds in the command log, the CLI prints them under the "written" line. The app can also pass `wire_layers`: when an import could not identify a wire layer, the layers from `ImportReport::layers` become buttons in the ribbon group **Wire layer**, and one click re-imports the same file with that layer.

All imports go through `Engine::replace_project` (undo history cleared, agent turns cancelled, chat history reset), the same path as KiCad import and file open.

## 5. Design targets

- Ribbon tab "Import/Export": groups Import (Open / KiCad schematic / DXF), Export (DXF big button / KiCad / SVG / PDF), Reports (BOM CSV / wire list CSV), plus **Wire layer** — one small button per candidate layer, shown only after a DXF import that could not identify a wire layer. Reuses the existing ribbon components; no new component was added to the design system.
- Import report presentation: currently the command-line log (counts, skipped list, warnings). A dialog listing skipped blocks with a "place by hand" checklist is a design-phase item.

## 6. Acceptance criteria (all covered by tests; see `docs/13-specification.md` sections "KiCad import / export", "DXF interop", "REST Link API", "Argument parsing & dispatch", "File menu")

- Export → import round trip of a sheet keeps paper, symbols (id, position, rotation, mirror, reference, value, attributes), wires, junctions, wire numbers, labels, notes and harness names, for both `.kicad_sch` and DXF.
- An ACADE-style DXF (wires on `WIRES`/`_MULTI_WIRE_*`, `WIRENO` texts, unknown `HCR1` blocks with `TAG1`) imports its wires and wire numbers and reports the unknown blocks.
- Explicit `wire_layers` restricts which lines become wires; a file with no wire layer imports all lines with a warning; explicit layers that match no line import no wire and warn with the layers the file has.
- A rotated or mirrored symbol's pins land on the paper positions `netlist::pin_positions` gives them, so they meet the exported wires (0/90/180/270 × mirror).
- Shapes, the wire legend, the panel layout and the frame reach the DXF on their own layers, and both exports return a report with the count per kind and the kinds they could not write.
- Header drawing limits decide the imported paper; justified `TEXT` is read at group 11/21; small circles become junctions only on a wire layer.
- Inch files are scaled; without header limits the paper size is the smallest A size that fits.
- Non-ASCII text survives export/import through `\U+XXXX`.
- REST, MCP and CLI expose the three operations with the same JSON shapes as the existing KiCad import / SVG export.

## 7. Open items (not implemented; decide before investing)

1. **ACADE / EPLAN block-name mapping tables** — mapping ACADE family codes (e.g. `HCR1` → relay coil) and EPLAN symbol numbers to MadakeCAD symbol ids would turn skipped inserts into real symbols. Needs sample DXFs from real projects (confidential references) to build and validate the tables. Proposed as an editable JSON table under `~/MadakeCAD/interop/`.
2. **`.dwg` directly** — only via an external converter (ODA File Converter, AutoCAD, EPLAN). A "convert with ODA if installed" convenience could wrap that, but it is a distribution/licensing question (ODA's converter is free but not redistributable).
3. **EPLAN PXF / project XML** — EPLAN's own exchange format is undocumented; not planned.
4. **Multi-sheet KiCad export** — currently one file per sheet. A hierarchical root sheet referencing per-sheet files is possible but KiCad's instance paths make round-tripping fragile; deferred until there is a concrete need.
5. **Wire colour / gauge in DXF** — not carried (no ACADE convention beyond layer colour). Could be written as XDATA if a consumer needs it.
