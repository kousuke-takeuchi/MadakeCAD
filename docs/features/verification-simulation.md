# Verification & Simulation

[日本語](verification-simulation.ja.md)

Catch wiring mistakes before they leave the drawing office. MadakeCAD extracts a netlist from the drawing, runs electrical rules checks, and — uniquely — grounds its electrical judgments in a **real circuit solver (ngspice)** rather than heuristics alone.

## Available

### Netlist extraction
- Connectivity from coordinate coincidence (2.5 mm grid), junctions, and same-named net labels; terminal blocks contribute feed-through connections
- Deterministic net names (label name, or N001… sequence)

### ERC (electrical rules check)
- Unconnected pins (terminal blocks judged per terminal — one wired side counts)
- Empty and duplicate reference designators (relay coil/contact sharing is correctly allowed)
- Dangling wire ends, including "crossing without a junction"
- Conflicting net labels on one net (a classic short between potentials)

### Electrical checks (ngspice-backed)
- The drawing is converted to a SPICE deck: wires become resistors (ρ·L/A, split at mid-wire connections), sources become voltage sources, loads become equivalent resistances from their rated current, switches/contacts/fuses become milliohm bridges
- Checks: source reachability, wire ampacity vs. gauge, voltage drop (limit: 3 % of supply), fuse rating — all judged from solved currents/voltages, with messages marked "(simulated)"
- **Graceful fallback**: without ngspice installed, checks run on a graph approximation and an Info diagnostic says so
- Results appear in a dockable panel; clicking a finding selects and centers the offending entity. Also available via CLI (`madake verify`) and MCP (`run_verification`)

### DC operating-point simulation
- Net voltages (min–max across each net, exposing wiring drop), component currents and power
- **What-if switching**: declare switches/contacts open (`--open SW1,K1`) and re-solve
- Ribbon button with results panel; CLI `madake sim`; MCP `simulate_op`. ngspice is located via `MADAKE_NGSPICE` → PATH → OS default paths (macOS/Linux/Windows)

## Planned

- Fuse selectivity/coordination across series fuses (after parts-DB SPICE models)
- Relay contact-count check against the part's actual contact complement (M4)
- Transient analysis (deferred until nonlinear/reactive SPICE models from the parts DB are in real use)
- ERC cross-check against `kicad-cli sch erc` once KiCad export exists
