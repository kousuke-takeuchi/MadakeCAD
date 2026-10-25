# Automation & APIs

[日本語](10-automation-api.ja.md)

Everything the UI can do is scriptable. Three surfaces expose the same Command engine, so automated edits share undo history and update the UI live.

## Available

### MCP server (for AI clients)
Streamable HTTP at `127.0.0.1:9310/mcp` (`MADAKE_MCP_PORT` to change). Tools:
`open_project` / `save_project` (open or save a `.mdkproj`, with its chat history; opening replaces the drawing and is refused while an agent turn is running, and the in-app agent cannot open), `get_project`, `list_symbols` (bundled **and** user symbols), `place_symbol` (returns the new part's pins; attribute `hide_reference: "true"` keeps the designator but does not print it), `draw_wire`, `get_pins` (pins of a placed part: number, name, paper position, direction), `connect_pins` (wire two pins by name, e.g. `U1.SDA` to `J1.2`, with a right-angle route; `connections` wires many in one undoable edit), `place_mate` (a plug/socket pair in one step; `mates` places many), `execute_commands` (full Command schema exposed), `get_netlist`, `run_verification`, `get_tidy_metrics` (the four counts plus `details`: each counted problem with its kind, entities, text and place, up to 50), `simulate_op`, `search_parts`, `upsert_part`, `delete_part`, `import_kicad`, `import_dxf` (DXF from AutoCAD Electrical / EPLAN; optional `wire_layers`), `list_terminal_blocks`, `get_terminal_chart`, `check_terminal_block`, `list_templates`, `apply_template`, `list_macros`, `save_macro`, `apply_macro`, `duplicate_symbol`, `save_user_symbol`, `delete_user_symbol`, the PLC I/O tools, `get_sheet_image` (the sheet as a PNG image, same look as the SVG export; `px_per_mm` and a `region` in mm to zoom in), `export_svg`, `export_pdf`, `export_dxf` (AutoCAD 2000 DXF), `export_kicad` (`.kicad_sch`), `export_report` (one of the seven reports as CSV or framed PDF), `export_pdf_book` (cover + all sheets + selected reports in one PDF), `export_bom`, `export_wire_list`, `undo`, `redo`.
Drawing edits that have no dedicated tool go through `execute_commands`: `set_revisions` (revision table), `set_sheet_annotation_text` (`height_mm` 1.0-3.5mm, or null for the 2.0mm default: the height of the wire notes, wire numbers and net tags on that sheet), `renumber_wires` (`mode: append | renumber`, optional `sheet_id`, `start`) and `set_wire_numbers` for wire numbers, `add_entity` with a `harness` entity for harness boundaries, `update_entity` writing `attrs["jumpers"] = "1-2,3-4"` on a terminal block symbol for saddle jumpers, `set_frame_template` for the frame template, `add_sheet` with `kind: "panel"` plus the `panel_outline` / `din_rail` / `duct` / `footprint` / `panel_legend` entities for panel layout, `renumber_panel_items` for balloon numbers, and `set_mech_link` / `remove_mech_link` / `set_wire_lengths` for the FreeCAD link (mechanical links and measured wire lengths).
Claude Code connects automatically via this repository's `.mcp.json`.

### Link API (REST + SSE, for external tools)
`127.0.0.1:9310/api/v1` — plain JSON REST used by the CLI, browser-based UI verification, and (future) the FreeCAD add-on:
- Read: `/project`, `/symbols` (+`/symbols/user`), `/frame-templates`, `/panel/unplaced`, `/netlist`, `/verify`, `/search`, `/devices`, `/parts`, `/wire-parts`, `/terminals`, `/terminals/chart`, `/terminals/check`, `/plc/modules`
- Write: `/commands` (Command array; `?batch=true` runs the whole array as **one** edit = one undo step, and applies nothing at all if a command is refused), `/undo`, `/redo`, `/save`, `/load`, `/import/kicad`, `/import/dxf`, `/simulate/op`, `/symbols/user/{save,delete}`, `/symbols/duplicate`, `/macros/{build,save,apply,apply-inline}`, `/templates/apply`, `/variants/{start,finish}`, `/export/{svg,pdf,dxf,kicad,pdf-book,report,bom,wire-list}`, parts CRUD
- Live updates: `GET /events` (SSE patch stream)
- Local-origin guard: requests from non-local web origins are rejected

### madake CLI
Thin terminal client over the Link API: `status`, `project`, `netlist`, `verify`, `sim`, `parts`, `terminals` (terminal block list), `export`, `save`/`open` (incl. `.kicad_sch` and `.dxf`, the latter with `--wire-layer`), `renumber` (wire numbering: `--sheet`, `--mode append|renumber`, `--start`), `exec` (Command array from JSON), `undo`/`redo`. `--json` for machine-readable output.

`madake export <kind> <path>` covers both schematics and reports:

| Kind | Options |
|---|---|
| `svg`, `pdf`, `dxf`, `kicad` | `--sheet <ID>` (default: first sheet). `dxf` = AutoCAD 2000 DXF for ACADE/EPLAN, `kicad` = `.kicad_sch` |
| `pdf-book` | `--reports wire-list,terminal-chart,terminal-diagram,bom,xref,panel-placement` (in that order), `--no-cover` |
| `bom`, `wire-list`, `terminal-chart`, `terminal-diagram`, `xref-table`, `plc-io`, `panel-placement` | `--format csv\|pdf` (default: from the file extension — `.pdf` writes framed drawing sheets), `--terminal <ref\|id>` for the two terminal reports |

Reports always cover the whole project; the only narrowing is `--terminal TB1` (add `--sheet <ID>` when the same designator exists on several sheets). `madake terminals` lists the candidates.

## Planned

- FreeCAD-specific endpoints as needed by the M5 workbench (e.g., parts filtered by 3D model)
- API stability/versioning policy for the open-source release (M6)
