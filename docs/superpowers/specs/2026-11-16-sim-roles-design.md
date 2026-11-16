# Simulation roles for user symbols (design)

Date: 2026-11-16. Status: approved by the user.

## Problem

The DC operating point simulation (`spice.rs`, `sim.rs`) only knows built-in symbols: the source must be the
library `battery`, loads are `category == "output"` / `relay_coil` / `led`, conductors are `switch` /
`protection` / `*contact*`. A drawing made of user symbols (traced or hand-made, category `user`) gives no
source, no loads and no fuses, so it cannot be simulated. Loads are also modelled as a resistor `V / I` with one
global voltage, so a drawing with two supply voltages (for example 24 V and a 12 V DC/DC output) is wrong.

## Design

1. **`SymbolDef.sim`** (optional, user symbols and built-ins alike): `{ "role": ..., "pins": [...] }`.
   Roles: `source`, `load`, `fuse`, `conductor`, `converter`, `none`.
   - `source`, `load`: `pins = [plus, minus]`.
   - `converter`: `pins = [in_plus, in_minus, out_plus, out_minus]`.
   - `fuse`, `conductor`: `pins` = the pins that conduct to each other; when empty, the built-in pole rule
     applies (pins numbered 1..2n conduct per pole: 1-2, 3-4, ...; otherwise all pins).
   - `none`: the symbol takes no part (no bridge, no load) except same-number feed-through pins.
   Without `sim` the current category rules apply unchanged.
2. **Values are instance attributes** (`attrs`, set with the existing attribute command):
   `voltage`, `r_int_ohm`, `i_max_a` (source); `current_a`, `v_min` (load); `rating_a` (fuse; falls back to the
   value text); `v_out`, `eta`, `i_out_a` (converter); `pin_rating_a` (any symbol: current per pin through its
   bridges, e.g. a connector with feed-through pins).
3. **SPICE model** for symbols with a role:
   - source: `V` + series `r_int_ohm` (default 1 mΩ);
   - load: constant current source `I` (`current_a`) with a 1 mΩ series sense resistor; without `current_a` the
     existing 1 MΩ placeholder;
   - converter: output = `V` of `v_out` + 1 mΩ; input = behavioural current source drawing
     `P_out / (eta * V_in)` (`eta` default 0.9);
   - several sources are allowed; every node gets a 1 GΩ shunt to ground (`.options rshunt`) so isolated
     sub-circuits (for example a converter output that is not tied to the input return) still solve.
   Ground is the minus pin of the first source (built-in battery first).
4. **Scenarios** (`SimOptions`): `open_switches` (existing), `overrides` (`reference -> {attr: value}`, applied to
   every symbol with that reference), `shorts` (net names joined to ground through 1 mΩ). The CLI
   (`madake sim --set REF.attr=value --short NET`), the Link API (`POST /api/v1/simulate/op`) and the MCP tool
   take the same fields; old requests without them keep working.
5. **Findings** (`SimOpResult.findings`, each with code, severity, reference, measured value and limit):
   `sim.fuse_rating` (current above `rating_a`), `sim.pin_rating` (bridge current above `pin_rating_a`),
   `sim.source_overload` (above `i_max_a`), `sim.converter_overload` (output above `i_out_a`),
   `sim.load_voltage` (load terminal voltage below `v_min`), `sim.wire_ampacity` (wire current above the
   ampacity of its `sq`; wires thicker than the table are reported as unchecked). The verifier's existing
   simulated fuse check also covers symbols with the `fuse` role.

## Out of scope

Transients (inrush, fuse melting time), temperature, AC. Joining nets by plain text (drawings that name nets in
free text need net labels or net-name symbols). Format version: the project file does not change (roles live
in symbol files and values in existing `attrs`).
