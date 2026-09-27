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

## Available: part linking & wire-length write-back (M5-2)

- **Parts tab**: every symbol of the project with its part number, the 3D model path from the parts database (`model_3d`) and the FreeCAD object it is linked to. **Insert 3D model** reads STEP/IGES/BREP (or merges an FCStd) into the active document, tags the object with a `madake_id` property (the entity UUID) and registers the link in MadakeCAD (`set_mech_link`, stored in `Project.mech_links`). **Link selected object** / **Unlink** manage links for models you placed yourself
- **Wires tab**: every wire with its net or wire number, current length and where it came from, and the linked route object. **Link selected route** ties a Draft Wire (or any object with a shape) to a wire; **Measure routes → write back lengths** measures every linked route (`Shape.Length`, mm → m at 1 mm resolution), asks for confirmation and writes the lengths with `set_wire_lengths` — one undo step per sheet in MadakeCAD, feeding the wire list and voltage-drop verification
- **No silent overwrite**: written-back lengths carry the source `freecad`. MadakeCAD shows a *FreeCAD* badge next to such a length in the wire properties; editing it by hand switches the source back to manual and logs a warning. Sync is always a button press, never automatic
- File format: `.mdkproj` format 3 adds `mech_links` and `length_source`; older files open unchanged with empty links and manual lengths

## Available: route sync & placements (M5-3)

- **Create route stubs** (Wires tab): for every wire whose net joins two linked parts, a straight line between the parts' placements is created in FreeCAD, tagged with the wire's `madake_id` and named after the net. Edit it into the real route (add vertices, replace with a Draft Wire keeping the tag) and use *Measure routes → write back lengths*. Re-running skips wires that already have a route object
- **Net highlight**: selecting a net in the Netlist tab selects its linked parts and routes in the 3D view
- **Sync placements** (Parts tab): writes each linked object's placement (base point in mm, Z rotation in degrees) to MadakeCAD (`mech_links[].placement`, file format 4). MadakeCAD shows it in the *3D link* row of the properties panel; the future 2D panel-layout sheet (M4 §8) will use it as the initial footprint placement. Placement is mastered by FreeCAD and only ever synced by this button

## Planned (M5)

- **2D panel-layout sheet** (M4 §8): the native panel drawing that will consume the synced placements; needs its own UI design first

Design record: master spec §7; implementation breakdown: [specs/m5-freecad.md](internal/specs/m5-freecad.md).
