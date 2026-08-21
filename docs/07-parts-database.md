# Parts Database

[日本語](07-parts-database.ja.md)

A global, local-first parts master (SQLite) shared by all projects. Drawings reference parts by part number and stay self-contained.

## Available

- **Part master**: part number (unique key), maker, name, category, default symbol (parametric IDs allowed), rated voltage, rated current, procurement URL, datasheet URL, price/currency, notes; reserved columns for 3D model reference, mounting info, and SPICE model
- **Wire part master**: color + gauge → wire part number, per-meter price
- **Search & place**: the symbol-insert dialog searches the database; selecting a part places its default symbol with the part number as value and the rated current as a verification-ready attribute
- **Versioned schema** with automatic migrations; sample (dummy) parts seeded on first run
- **Access everywhere**: CLI (`madake parts`), MCP tools (`search_parts` / `upsert_part` / `delete_part`), REST (`/api/v1/parts`, `/api/v1/wire-parts`) — the AI assistant can register and look up parts conversationally
- Default location: OS app-data folder (`MADAKE_PARTS_DB` to override)

## Planned

- BOM enrichment (maker, price, procurement columns pulled from the database) (M4)
- PLC module definitions (I/O points, address schemes) (M4)
- Management UI (list/edit screens) — currently by design CLI/MCP/API-driven
- SPICE model usage for transient/nonlinear simulation (after M4)
- 3D model references consumed by FreeCAD integration (M5)
