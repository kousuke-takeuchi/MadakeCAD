"""MadakeCAD Link — FreeCAD add-on workbench talking to a running MadakeCAD over its Link API.

Pure-Python modules (no FreeCAD dependency, unit-tested):
  client.py   Link API client (REST + SSE)
  model.py    project summary / netlist rows for the panel
  events.py   live follow: which patches need a refresh
  settings.py port preference

FreeCAD-only modules (imported lazily inside FreeCAD):
  panel.py    Qt dock widget
  commands.py workbench commands
"""

__version__ = "0.1.0"
