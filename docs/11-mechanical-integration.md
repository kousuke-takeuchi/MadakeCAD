# Mechanical CAD Integration

[日本語](11-mechanical-integration.ja.md)

Electrical–mechanical round-trip in the spirit of SOLIDWORKS Electrical ⇔ SOLIDWORKS, built on FreeCAD 1.1+. Electrical data (designators, part numbers, pin connectivity) is mastered in MadakeCAD; geometry (3D placement, routes, measured lengths) is mastered in FreeCAD. Synchronization is always explicit — no silent overwrites.

## Available (foundation)

- **Link API** (`/api/v1`, REST + SSE): the integration surface is already live — external tools can read the project/netlist, subscribe to changes, and write through the Command engine
- **Parts database hooks**: reserved `model_3d` (STEP/FCStd path) and `mounting` columns
- Entity UUIDs as the linking key across both worlds

## Available: the "MadakeCAD Link" workbench (M5-1)

- A FreeCAD 1.0+ add-on in [`freecad-addon/`](../freecad-addon/README.md): copy or symlink the folder into FreeCAD's `Mod` directory, pick the **MadakeCAD Link** workbench, set the port (default 9310) and press **Connect**
- The panel shows the project name, revision and entity counts per sheet, and a netlist table for the selected sheet (net name, wire number, label, pins as `K1:A1, TB1:3`, wire count)
- **Follow live** subscribes to MadakeCAD's event stream and reloads the view when the shown sheet or the sheet list changes, so edits in MadakeCAD (by you or by the AI) appear in FreeCAD as they happen
- The Link API client is standard-library Python and, together with the panel model and live-follow logic, is unit-tested without FreeCAD (`python3 -m unittest discover -s freecad-addon/tests`; the tests are part of the [specification](13-specification.md)). Writes always go through `POST /api/v1/commands`, i.e. the Command engine

## Planned (M5)

- **M5-2 Part linking & wire-length write-back**: insert 3D models from the parts list into a FreeCAD assembly, store `madake_id` on FreeCAD objects and `mech_links` in the project; measure route lengths in FreeCAD and write them back to wire `length_m` (with a source flag so manual edits never silently overwrite measured values) — feeding voltage-drop verification and the wire list
- **M5-3 Route visualization & panel layout**: 3D route sync and 2D panel-layout ⇔ 3D enclosure correspondence

Design record: master spec §7; implementation breakdown: [specs/m5-freecad.md](internal/specs/m5-freecad.md).
