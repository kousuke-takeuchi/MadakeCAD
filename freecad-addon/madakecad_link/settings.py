"""Port preference. Inside FreeCAD it is stored in the user parameters; elsewhere in memory."""

from __future__ import annotations

from .client import DEFAULT_PORT

PREF_PATH = "User parameter:BaseApp/Preferences/Mod/MadakeCADLink"


def normalize_port(value) -> int:
    """A usable TCP port: integers 1..65535, anything else falls back to the default."""
    try:
        port = int(value)
    except (TypeError, ValueError):
        return DEFAULT_PORT
    if 1 <= port <= 65535:
        return port
    return DEFAULT_PORT


class MemorySettings:
    """Settings kept in memory (tests, and FreeCAD without a parameter store)."""

    def __init__(self, port: int = DEFAULT_PORT):
        self._port = normalize_port(port)

    def get_port(self) -> int:
        return self._port

    def set_port(self, port) -> int:
        self._port = normalize_port(port)
        return self._port


class FreeCADSettings:
    """Settings backed by FreeCAD's user parameters (``Mod/MadakeCADLink/Port``)."""

    def __init__(self, app):
        self._params = app.ParamGet(PREF_PATH)

    def get_port(self) -> int:
        return normalize_port(self._params.GetInt("Port", DEFAULT_PORT))

    def set_port(self, port) -> int:
        port = normalize_port(port)
        self._params.SetInt("Port", port)
        return port


def load_settings():
    """FreeCAD-backed settings when running inside FreeCAD, in-memory otherwise."""
    try:
        import FreeCAD  # type: ignore

        return FreeCADSettings(FreeCAD)
    except ImportError:
        return MemorySettings()
