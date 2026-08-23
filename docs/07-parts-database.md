# Parts Database

[日本語](07-parts-database.ja.md)

A global, local-first parts master (SQLite) shared by all projects. Drawings reference parts by part number and stay self-contained.

## Available

- **Part master**: part number (unique key), maker, name, category, default symbol (parametric IDs allowed), rated voltage, rated current, contact configuration, PLC module definition, panel footprint size, procurement URL, datasheet URL, price/currency, notes; reserved columns for 3D model reference, mounting info, and SPICE model
- **Contact configuration** (`contact_config`, schema v3): how many contacts a relay or contactor actually has, written the way a datasheet does — `2NO+2NC`, `4NO`, `1CO`. Placing such a part copies the value onto the symbol, so the coil's contact map can list the unused contacts as `—` and ERC can flag a drawing that uses more contacts than the device provides
- **PLC module definition** (`plc_module`, schema v4): a small JSON object — point count, I/O direction (`DI` / `DO`), address prefix and the maker's address scheme (`mitsubishi` octal, `siemens` byte.bit, `ab` word/bit). A part that carries one is a PLC module: it is offered in the I/O module library, it places as the matching parametric symbol (`plc_di_16p`, `plc_do_8p`, …), and it drives address auto-numbering in the [I/O assignment table](03-schematic-editor.md). Parts with an empty `plc_module` are ordinary parts
- **Footprint size** (`footprint_w` / `footprint_h`, schema v6): the width and height in millimetres that the part occupies on a panel, kept as two real columns so they can be filtered on in SQL. Both must be positive to count as a size. The [panel layout](03-schematic-editor.md) looks the size up by part number when you place an unplaced part, and falls back to a 20 × 40 mm rectangle when the part number is empty or unregistered — the unplaced-parts list says which of the two you are getting
- **Wire part master**: color + gauge → wire part number, per-meter price
- **Search & place**: the symbol-insert dialog searches the database; selecting a part places its default symbol with the part number as value, and the rated current and contact configuration as verification-ready attributes
- **Versioned schema** with automatic migrations (v6 today); sample (dummy) parts seeded on first run, including four PLC modules covering the three address schemes. Upgrading an older database backfills the PLC samples missing from it (v5) and fills in footprint sizes for the bundled samples (v6) — only where they are absent, so parts you edited or registered under the same part number are left alone
- **Access everywhere**: CLI (`madake parts`), MCP tools (`search_parts` / `upsert_part` / `delete_part`), REST (`/api/v1/parts`, `/api/v1/wire-parts`, `/api/v1/plc/modules`) — the AI assistant can register and look up parts conversationally
- Default location: OS app-data folder (`MADAKE_PARTS_DB` to override)

## Planned

- BOM enrichment (maker, price, procurement columns pulled from the database) (M4)
- Management UI (list/edit screens) — currently by design CLI/MCP/API-driven
- SPICE model usage for transient/nonlinear simulation (after M4)
- 3D model references consumed by FreeCAD integration (M5)
