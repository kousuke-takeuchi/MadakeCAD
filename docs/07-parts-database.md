# Parts Database

[日本語](07-parts-database.ja.md)

A global, local-first parts master (SQLite) shared by all projects. Drawings reference parts by part number and stay self-contained.

## Available

- **Part master**: part number (unique key), maker, name, category, default symbol (parametric IDs allowed), rated voltage, rated current, contact configuration, procurement URL, datasheet URL, price/currency, notes; reserved columns for 3D model reference, mounting info, and SPICE model
- **Contact configuration** (`contact_config`, schema v3): how many contacts a relay or contactor actually has, written the way a datasheet does — `2NO+2NC`, `4NO`, `1CO`. Placing such a part copies the value onto the symbol, so the coil's contact map can list the unused contacts as `—` and ERC can flag a drawing that uses more contacts than the device provides
- **Wire part master**: color + gauge → wire part number, per-meter price
- **Search & place**: the symbol-insert dialog searches the database; selecting a part places its default symbol with the part number as value, and the rated current and contact configuration as verification-ready attributes
- **Versioned schema** with automatic migrations (v3 today); sample (dummy) parts seeded on first run
- **Access everywhere**: CLI (`madake parts`), MCP tools (`search_parts` / `upsert_part` / `delete_part`), REST (`/api/v1/parts`, `/api/v1/wire-parts`) — the AI assistant can register and look up parts conversationally
- Default location: OS app-data folder (`MADAKE_PARTS_DB` to override)

## Planned

- BOM enrichment (maker, price, procurement columns pulled from the database) (M4)
- PLC module definitions (I/O points, address schemes) (M4)
- Management UI (list/edit screens) — currently by design CLI/MCP/API-driven
- SPICE model usage for transient/nonlinear simulation (after M4)
- 3D model references consumed by FreeCAD integration (M5)
