# Import & Export

[日本語](08-import-export.ja.md)

MadakeCAD exchanges drawings with three ecosystems. KiCad has an open text format, so it is read and written directly. AutoCAD Electrical (ACADE) and EPLAN keep their projects in proprietary formats (`.dwg`, `.elk`/`.zw1`), so the exchange goes through **DXF**, the interchange format both products import and export natively. Design notes and the mapping tables are in the internal [interop spec](internal/specs/interop.md).

## Available

### KiCad import (`.kicad_sch`)
- Reads KiCad 8/9 schematics (self-contained S-expression parser, no external dependency)
- Converts paper size/orientation, title block, wires, junctions, labels (local/global/hierarchical), text, and mapped symbols (Device:R/C/D/LED/Fuse/Lamp/Battery, switches, relays, motors); `Conn_01xNN` and `Screw_Terminal_01xNN` become parametric connectors/terminal blocks with the right pole count; power symbols (`power:GND`, `power:+24V`, …) become net labels preserving their electrical meaning; `MadakeCAD:<id>` symbols written by the exporter come back as the same symbol
- Unsupported symbols are skipped and itemized in an **import report** (counts, skipped lib_ids, warnings)
- Entry points: app Open dialog or ribbon **Import/Export > KiCad schematic**, `madake open file.kicad_sch`, MCP `import_kicad`, REST `POST /api/v1/import/kicad`
- Known limitation: pin geometries differ between libraries, so some connections need repair after import — the ERC is designed to surface exactly those spots

### KiCad export (`.kicad_sch`)
- One file per sheet in KiCad 9 format. Every symbol used on the sheet is embedded in the file (`lib_symbols`, named `MadakeCAD:<symbol_id>`) with its graphics and pins, so KiCad opens it without any MadakeCAD library installed
- Wires (one KiCad wire per segment), junctions, net labels, text and the title block map 1:1. Wire numbers become a label on the numbered net (KiCad has no wire numbers); harness boundaries become dashed polylines with their name
- Round trip: exporting and re-importing keeps symbol placement, rotation, mirroring, references and values
- Use it for an independent cross-check with `kicad-cli sch erc`, or to hand a drawing to a KiCad user
- **Export report**: the export returns how many of each kind it wrote and which kinds it did not write at all — shapes, the wire part legend and the five panel-layout kinds have no equivalent in a KiCad schematic. They are listed (`panel_outline x1: …`) in the command log, the CLI output and the REST/MCP response, never dropped silently. Export the sheet as DXF to keep them
- Entry points: ribbon **Import/Export > KiCad schematic** (export group), `madake export kicad out.kicad_sch [--sheet ID]`, MCP `export_kicad`, REST `POST /api/v1/export/kicad`

### DXF export (AutoCAD Electrical / EPLAN)
- AutoCAD 2000 (AC1015) ASCII DXF in millimetres; Japanese and other non-ASCII text is written as `\U+XXXX`, which AutoCAD and EPLAN decode
- Layer layout follows ACADE conventions: wires are `LINE`s on `WIRES`, wire numbers on `WIRENO`, references on `TAGS`, part numbers/descriptions on `DESC`, net labels on `LABELS`, notes on `MISC`, harness boundaries as dashed polylines on `HARNESS`
- Symbols are block inserts `MDK_<symbol_id>` with ACADE-style attributes `TAG1` (reference), `CAT` (part number / value), `DESC1`, `RATING1` and `TERMnn` (pin numbers); junctions use the `WDDOT` block. Group 50 carries `-θ` because the export flips Y, so a rotated symbol's pins land exactly where its wires end
- **The frame is written** on `FRAME`: the paper outline, the border, the zone ticks with their letters and numbers, and the title block (cells with their binds resolved — drawing number, title, scale, date, names, revision). The revision table itself is not written; the latest revision appears in the title block
- **Everything else on the sheet is written too**, because DXF is a general 2D drawing format: shapes (G10) as `LINE`/`LWPOLYLINE`/`CIRCLE` on `SHAPES` (dashed shapes use the `DASHED` linetype; fills are not written), the wire part legend as a `LWPOLYLINE` grid plus `TEXT` rows on `ANNOTATION`, and the panel layout (enclosure and mounting plate, DIN rails, ducts with their hatching, footprints with reference/part number/balloon, the legend table) on `PANEL`
- **Export report**: the export returns the count per kind plus anything it could not write (for example a symbol whose definition is missing). It appears in the app's command log, in `madake export dxf` output and in the REST/MCP response
- Entry points: ribbon **Import/Export > DXF**, `madake export dxf out.dxf [--sheet ID]`, MCP `export_dxf`, REST `POST /api/v1/export/dxf`

### DXF import (AutoCAD Electrical / EPLAN)
- Reads any ASCII DXF (R12 or later). Inch drawings are converted to millimetres. The paper size and origin come from the header drawing limits (`$LIMMIN`/`$LIMMAX`, else `$EXTMIN`/`$EXTMAX`); only a file without them falls back to the smallest ISO A size that fits the entity extents
- Lines on wire layers become wires. Wire layers are detected by name (containing `WIRE`, e.g. ACADE's `WIRES` and `_MULTI_WIRE_1`) or given explicitly (`--wire-layer`, `wire_layers`)
  - If the file has no wire layer at all, every line is read as a wire and the report says so, listing the layers it saw
  - If **you named** the layers and none of them holds a line (a typo, or a different convention), no wire is imported and the warning lists the layers the file really has. An explicit narrowing never turns into "every line is a wire"
  - In the app the layers from that warning appear as buttons in the ribbon group **Import/Export > Wire layer**; one click re-imports the same file with that layer as the wire layer
- `WIRENO` texts become the wire number of the nearest wire; `MDK_<symbol_id>` blocks (MadakeCAD's own exports) become symbols with their attributes; `WDDOT` inserts and small circles (r ≤ 1 mm) **on a wire layer** become junctions — never under the all-lines fallback, where pin circles of a foreign drawing would silently merge nets; other text becomes notes. Justified `TEXT` (groups 72/73) is read at its alignment point (groups 11/21), so ACADE's centred wire numbers land where they are drawn
- ACADE's and EPLAN's own symbol blocks have no public mapping table, so they are listed in the import report as skipped (`HCR1 (CR1) x2`) for manual placement
- Entry points: app Open dialog or ribbon **Import/Export > DXF (ACADE/EPLAN)**, `madake open drawing.dxf [--wire-layer WIRES,_MULTI_WIRE_1]`, MCP `import_dxf`, REST `POST /api/v1/import/dxf`

### Getting a DXF out of the other tools
- **AutoCAD Electrical**: `DXFOUT` / Save As → AutoCAD 2000 DXF. Keep the default wire layer names; the importer recognises them
- **EPLAN Electric P8**: Page → Export → DXF/DWG, one file per page; choose DXF
- **`.dwg` files**: convert with AutoCAD, EPLAN, or the free ODA File Converter first — MadakeCAD does not read `.dwg`

### Project format (`.mdkproj`)
- Pretty-printed JSON, git-diffable, with `format_version` and migrations; chat history saved alongside as `<name>.chat.json`

### Outputs
- SVG, PDF, BOM CSV, wire list CSV (see [Standards & Output](04-standards-output.md))

## Planned / open

- Mapping tables for ACADE family blocks and EPLAN symbol numbers, so their inserts become real MadakeCAD symbols (needs sample drawings; see the interop spec §7)
- DXF import of shapes and panel geometry (the export writes them; the import still skips and reports non-wire geometry), and shape fills as `HATCH`
- Native `.dwg` / EPLAN project files: not planned (proprietary formats); use DXF
- Hierarchical-sheet and bus support for KiCad import (backlog)
