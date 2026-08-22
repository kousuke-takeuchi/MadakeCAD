# Automation & APIs

[日本語](10-automation-api.ja.md)

Everything the UI can do is scriptable. Three surfaces expose the same Command engine, so automated edits share undo history and update the UI live.

## Available

### MCP server (for AI clients)
Streamable HTTP at `127.0.0.1:9310/mcp` (`MADAKE_MCP_PORT` to change). Tools:
`get_project`, `list_symbols`, `place_symbol`, `draw_wire`, `execute_commands` (full Command schema exposed), `get_netlist`, `run_verification`, `simulate_op`, `search_parts`, `upsert_part`, `delete_part`, `import_kicad`, `export_svg`, `export_pdf`, `export_pdf_book` (cover + all sheets + selected reports in one PDF), `export_bom`, `export_wire_list`, `undo`, `redo`.
Drawing edits that have no dedicated tool go through `execute_commands`, including the M2 additions: `set_revisions` (revision table), `renumber_wires` (`mode: append | renumber`, optional `sheet_id`, `start`) and `set_wire_numbers` for wire numbers, and `add_entity` with a `harness` entity for harness boundaries.
Claude Code connects automatically via this repository's `.mcp.json`.

### Link API (REST + SSE, for external tools)
`127.0.0.1:9310/api/v1` — plain JSON REST used by the CLI, browser-based UI verification, and (future) the FreeCAD add-on:
- Read: `/project`, `/symbols`, `/netlist`, `/verify`, `/parts`, `/wire-parts`
- Write: `/commands` (Command array), `/undo`, `/redo`, `/save`, `/load`, `/import/kicad`, `/simulate/op`, `/export/{svg,pdf,pdf-book,bom,wire-list}`, parts CRUD
- Live updates: `GET /events` (SSE patch stream)
- Local-origin guard: requests from non-local web origins are rejected

### madake CLI
Thin terminal client over the Link API: `status`, `project`, `netlist`, `verify`, `sim`, `parts`, `export`, `save`/`open` (incl. `.kicad_sch`), `renumber` (wire numbering: `--sheet`, `--mode append|renumber`, `--start`), `exec` (Command array from JSON), `undo`/`redo`. `--json` for machine-readable output.

## Planned

- FreeCAD-specific endpoints as needed by the M5 workbench (e.g., parts filtered by 3D model)
- API stability/versioning policy for the open-source release (M6)
