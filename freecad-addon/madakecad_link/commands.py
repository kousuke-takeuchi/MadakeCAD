"""FreeCAD GUI commands of the workbench (only importable inside FreeCAD)."""

from __future__ import annotations

import os

import FreeCADGui  # type: ignore

ICON = os.path.join(os.path.dirname(os.path.dirname(__file__)), "resources", "madakecad_link.svg")

_panel = None


def show_panel():
    """Create the dock panel on first use and bring it to front."""
    global _panel
    from .panel import LinkDockWidget

    if _panel is None:
        _panel = LinkDockWidget()
        _panel.attach_to_main_window()
    _panel.show()
    _panel.raise_()
    return _panel


class ShowPanelCommand:
    def GetResources(self):
        return {
            "Pixmap": ICON,
            "MenuText": "MadakeCAD Link panel",
            "ToolTip": "Show the MadakeCAD Link panel (connection, project overview, netlist)",
        }

    def Activated(self):
        show_panel()

    def IsActive(self):
        return True


class RefreshCommand:
    def GetResources(self):
        return {
            "Pixmap": "view-refresh",
            "MenuText": "Refresh from MadakeCAD",
            "ToolTip": "Reload the project overview and the netlist from the running MadakeCAD",
        }

    def Activated(self):
        show_panel().refresh()

    def IsActive(self):
        return True


COMMANDS = {
    "MadakeCAD_ShowPanel": ShowPanelCommand,
    "MadakeCAD_Refresh": RefreshCommand,
}


def register() -> list:
    """Register every command once; returns their names for toolbars/menus."""
    for name, cls in COMMANDS.items():
        if name not in FreeCADGui.listCommands():
            FreeCADGui.addCommand(name, cls())
    return list(COMMANDS)
