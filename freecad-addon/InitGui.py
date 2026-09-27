# MadakeCAD Link add-on: workbench registration (FreeCAD 1.0+).
#
# The workbench is a thin shell: it registers the commands from
# madakecad_link/commands.py and puts them on a toolbar and a menu. All logic that can
# run without FreeCAD lives in madakecad_link/ and is unit-tested outside FreeCAD.

import os

import FreeCADGui  # type: ignore

ADDON_DIR = os.path.dirname(__file__)


class MadakeCADLinkWorkbench(FreeCADGui.Workbench):
    MenuText = "MadakeCAD Link"
    ToolTip = "Electrical–mechanical link to a running MadakeCAD (netlist view, live follow)"
    Icon = os.path.join(ADDON_DIR, "resources", "madakecad_link.svg")

    def Initialize(self):
        import sys

        if ADDON_DIR not in sys.path:
            sys.path.insert(0, ADDON_DIR)
        from madakecad_link import commands

        names = commands.register()
        self.appendToolbar("MadakeCAD Link", names)
        self.appendMenu("MadakeCAD Link", names)

    def Activated(self):
        from madakecad_link import commands

        commands.show_panel()

    def Deactivated(self):
        pass

    def GetClassName(self):
        return "Gui::PythonWorkbench"


FreeCADGui.addWorkbench(MadakeCADLinkWorkbench())
