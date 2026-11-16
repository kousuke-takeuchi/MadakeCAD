# Verification & Simulation

[日本語](06-verification-simulation.ja.md)

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
- **Simulation roles for any symbol**: a symbol definition may carry `sim` (`source`, `load`, `fuse`, `conductor`, `converter`, `none`, with the pins each role uses), so user symbols take part. Values come from the placed part's attributes: `voltage` / `r_int_ohm` / `i_max_a` (source), `current_a` / `v_min` (load, a constant current), `rating_a` (fuse), `v_out` / `eta` / `i_out_a` / `i_limit_a` / `v_in_min` (DC/DC converter: a second supply voltage whose input draws output power ÷ efficiency; optional current limit and input undervoltage lockout), `pin_rating_a` (current per pin, e.g. a connector)
- **Results on the drawing**: wires and parts are coloured by load (current ÷ limit): amber from 80 % to 100 %, red above 100 % with a label such as `71.0A / 50A`; pointing at a wire or part shows its current, voltages, limit and load. After an edit the result is marked stale (faint colours, no labels, a note in the panel) until the next run. The panel lists the findings next to the details of the selected wire or part; all net voltages and component currents are one click away. The result also carries `loads` (per wire and part: current, voltages, limit, limit kind, ratio)
- **Scenarios**: override attributes for one run (`--set M1.current_a=40`, e.g. a stalled motor) and short nets to ground (`--short P24`) to see fault currents
- **Rating findings**: fuse, connector pin, source, converter, load terminal voltage and wire ampacity are compared with the solved values and listed most severe first (a wire thicker than the ampacity table is noted as unchecked). Built-in loads keep the resistor model; only loads with a role are constant current. An isolated converter output is referenced to ground at its minus
- Ribbon button with results panel (net voltages, component currents and the rating findings column with counts; click a finding to select and center the part; a conditions bar takes `REF.ATTR=VALUE` and `short NET`); CLI `madake sim`; MCP `simulate_op`. ngspice is located via `MADAKE_NGSPICE` → PATH → OS default paths (macOS/Linux/Windows)

## Planned

- Fuse selectivity/coordination across series fuses (after parts-DB SPICE models)
- Relay contact-count check against the part's actual contact complement (M4)
- Transient analysis (deferred until nonlinear/reactive SPICE models from the parts DB are in real use)
- ERC cross-check against `kicad-cli sch erc` once KiCad export exists
