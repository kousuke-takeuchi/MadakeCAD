# Wire Management

[日本語](wire-management.ja.md)

Industrial wiring diagrams live and die by wire data: color, gauge, length, and part numbers. MadakeCAD treats these as first-class wire attributes that flow into reports and verification.

## Available

- **Per-wire attributes**: color (named palette incl. Japanese conventions), gauge in sq (mm²), optional length (m), wire part number, optional net name
- **Wire part master**: color + gauge → wire part number lookup, stored in the global parts database (with per-meter price and procurement URL); the project file keeps its own snapshot so drawings stay self-contained
- **Electrical semantics**: wire gauge and length feed ampacity and voltage-drop verification; terminal blocks provide feed-through connectivity in the netlist
- **Wire list report** (CSV) with part number, color, gauge, length

## Planned

- **Wire numbers** (M2): net-based numbering with automatic sequential assignment ("top-up" and full renumber modes), manual overrides preserved, on-drawing labels with their own view class, wire-list integration
- **Harness boundaries** (M2): dashed enclosures naming a harness; enclosed wires are grouped in the wire list
- **Reference-based numbering** (M4): wire numbers derived from sheet/zone addresses (AutoCAD Electrical style)
- **Multi-core cables** (M4): cable entities grouping cores, cable summary report
- **From–to wire list** (M4): wire list extended with from/to terminal references and harness column
- **Wire length write-back from FreeCAD** (M5): measured 3D route lengths applied to `length_m` with a source flag to prevent accidental manual overwrite
