# MadakeCAD Link (FreeCAD add-on)

[日本語](README.ja.md)

A FreeCAD 1.0+ workbench that talks to a **running MadakeCAD** over its Link API
(`http://127.0.0.1:9310/api/v1`). Milestone M5-1 (skeleton): connection settings, project
overview and a netlist view that follows drawing edits live. Part linking and wire-length
write-back come with M5-2 (see `docs/internal/specs/m5-freecad.md`).

## Install

Copy or symlink this folder into FreeCAD's `Mod` directory, then restart FreeCAD:

| OS | `Mod` directory |
|---|---|
| Linux | `~/.local/share/FreeCAD/Mod/` (or `~/.FreeCAD/Mod/`) |
| macOS | `~/Library/Application Support/FreeCAD/Mod/` |
| Windows | `%APPDATA%\FreeCAD\Mod\` |

```bash
ln -s /path/to/MadakeCAD/freecad-addon ~/.local/share/FreeCAD/Mod/MadakeCADLink
```

Registration with the FreeCAD Addon Manager is planned for the open-source release (M6).

## Use

1. Start MadakeCAD (the Link API is live while the app runs).
2. In FreeCAD pick the **MadakeCAD Link** workbench. The panel docks on the right.
3. Set the port (default 9310) and press **Connect**. The status line shows the project name,
   revision and entity counts; the table lists the nets of the selected sheet
   (net name, wire number, label, pins as `K1:A1, TB1:3`, wire count).
4. **Follow live** (on by default) subscribes to MadakeCAD's event stream and reloads the view
   when the shown sheet changes. **Refresh** reloads on demand.

## Layout

- `InitGui.py` — workbench registration (toolbar/menu with *MadakeCAD Link panel* and *Refresh*)
- `madakecad_link/client.py` — Link API client (REST + SSE, standard library only)
- `madakecad_link/model.py`, `events.py`, `settings.py` — pure helpers, unit-tested without FreeCAD
- `madakecad_link/panel.py`, `commands.py` — Qt panel and commands (FreeCAD only)
- `tests/` — `python3 -m unittest discover -s freecad-addon/tests` (also run in CI; every test doubles as a specification clause in `docs/13-specification.md`)

Writes from FreeCAD always go through `POST /api/v1/commands`, i.e. MadakeCAD's Command
engine — the add-on never touches the drawing model directly.
