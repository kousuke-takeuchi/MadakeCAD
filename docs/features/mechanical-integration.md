# Mechanical CAD Integration

[日本語](mechanical-integration.ja.md)

Electrical–mechanical round-trip in the spirit of SOLIDWORKS Electrical ⇔ SOLIDWORKS, built on FreeCAD 1.1+. Electrical data (designators, part numbers, pin connectivity) is mastered in MadakeCAD; geometry (3D placement, routes, measured lengths) is mastered in FreeCAD. Synchronization is always explicit — no silent overwrites.

## Available (foundation)

- **Link API** (`/api/v1`, REST + SSE): the integration surface is already live — external tools can read the project/netlist, subscribe to changes, and write through the Command engine
- **Parts database hooks**: reserved `model_3d` (STEP/FCStd path) and `mounting` columns
- Entity UUIDs as the linking key across both worlds

## Planned (M5)

- **M5-1 Workbench skeleton**: a FreeCAD add-on workbench ("MadakeCAD Link") with connection settings, project overview, and a netlist view that follows drawing edits live
- **M5-2 Part linking & wire-length write-back**: insert 3D models from the parts list into a FreeCAD assembly, store `madake_id` on FreeCAD objects and `mech_links` in the project; measure route lengths in FreeCAD and write them back to wire `length_m` (with a source flag so manual edits never silently overwrite measured values) — feeding voltage-drop verification and the wire list
- **M5-3 Route visualization & panel layout**: 3D route sync and 2D panel-layout ⇔ 3D enclosure correspondence

Design record: master spec §7; implementation breakdown: [specs/m5-freecad.md](../specs/m5-freecad.md).
