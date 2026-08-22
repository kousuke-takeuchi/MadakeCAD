# Wire Management

[日本語](05-wire-management.ja.md)

Industrial wiring diagrams live and die by wire data: color, gauge, length, and part numbers. MadakeCAD treats these as first-class wire attributes that flow into reports and verification.

## Available

- **Per-wire attributes**: color (named palette incl. Japanese conventions), gauge in sq (mm²), optional length (m), wire part number, optional net name
- **Wire part master**: color + gauge → wire part number lookup, stored in the global parts database (with per-meter price and procurement URL); the project file keeps its own snapshot so drawings stay self-contained
- **Electrical semantics**: wire gauge and length feed ampacity and voltage-drop verification; terminal blocks provide feed-through connectivity in the netlist
- **Wire numbers**: net-scoped numbers stored on every wire of the net. Automatic numbering (Schematic tab → Wiring group → *Wire numbers*) runs in two modes — *top-up*, which keeps existing numbers and net labels and only fills in unnumbered nets, and *renumber*, which reassigns every automatically generated number; manually typed names are always kept. Numbers are issued top-to-bottom, then left-to-right, are unique across the whole project when no sheet is given, and never collide with a net label. They are drawn on the wire (above a horizontal run, left of a vertical one) and can be edited one net at a time in the properties panel. Same command from the CLI: `madake renumber [--sheet <ID>] [--mode append|renumber] [--start N]`
- **Harness boundaries**: a dashed rectangle with a name (`W1`, `W2` …) and a note. Membership is geometric — a wire belongs to the harness that encloses all of its points, and the innermost enclosure wins for nested boundaries. Drawn with the harness tool (Schematic tab → Wiring group → *Harness*, drag a rectangle), edited in the properties panel, which also shows how many wires are enclosed
- **From–to wire list report** (CSV) with sheet, from, to, wire number, color, gauge, length, part number, harness. From and To are the two ends of the wire: a pin is written `reference:pin` (`K1:A1`, `TB1:3`), a net label as its name, an end that touches nothing stays empty; of the two ends the alphabetically smaller one is From, and an unconnected end is always To

## Planned

- **Reference-based numbering** (M4): wire numbers derived from sheet/zone addresses (AutoCAD Electrical style)
- **Multi-core cables** (M4): cable entities grouping cores, cable summary report
- **Wire length write-back from FreeCAD** (M5): measured 3D route lengths applied to `length_m` with a source flag to prevent accidental manual overwrite
