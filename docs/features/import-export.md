# Import & Export

[日本語](import-export.ja.md)

## Available

### KiCad import (`.kicad_sch`)
- Reads KiCad 8/9 schematics (self-contained S-expression parser, no external dependency)
- Converts paper size/orientation, title block, wires, junctions, labels (local/global/hierarchical), text, and mapped symbols (Device:R/C/D/LED/Fuse/Lamp/Battery, switches, relays, motors); `Conn_01xNN` and `Screw_Terminal_01xNN` become parametric connectors/terminal blocks with the right pole count; power symbols (`power:GND`, `power:+24V`, …) become net labels preserving their electrical meaning
- Unsupported symbols are skipped and itemized in an **import report** (counts, skipped lib_ids, warnings)
- Entry points: app Open dialog, `madake open file.kicad_sch`, MCP `import_kicad`, REST `POST /api/v1/import/kicad`
- Known limitation: pin geometries differ between libraries, so some connections need repair after import — the ERC is designed to surface exactly those spots

### Project format (`.mdkproj`)
- Pretty-printed JSON, git-diffable, with `format_version` and migrations; chat history saved alongside as `<name>.chat.json`

### Outputs
- SVG, PDF, BOM CSV, wire list CSV (see [Standards & Output](standards-output.md))

## Planned

- KiCad export, enabling `kicad-cli sch erc` as an independent cross-check (M6 timeframe)
- DXF/DWG export for interop with the AutoCAD ecosystem (backlog, see gap analysis)
- Hierarchical-sheet and bus support for KiCad import (backlog)
