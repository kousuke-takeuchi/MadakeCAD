"""Link API client (REST + SSE) using only the standard library.

MadakeCAD serves the Link API on 127.0.0.1:9310/api/v1 while the app runs. All writes go
through ``POST /commands`` and therefore through MadakeCAD's Command engine (undo/redo,
patch broadcast) — the add-on never edits the drawing model directly.
"""

from __future__ import annotations

import json
import urllib.error
import urllib.parse
import urllib.request
from typing import Any, Iterable, Iterator

DEFAULT_PORT = 9310
DEFAULT_TIMEOUT = 5.0


class LinkError(Exception):
    """A request to MadakeCAD failed (not running, HTTP error, bad JSON)."""


def base_url(port: int = DEFAULT_PORT) -> str:
    """Root of the Link API for a port, e.g. ``http://127.0.0.1:9310/api/v1``."""
    return f"http://127.0.0.1:{int(port)}/api/v1"


def parse_sse(lines: Iterable[str]) -> Iterator[tuple[str, str]]:
    """Turn the lines of a Server-Sent Events stream into ``(event, data)`` pairs.

    Follows the SSE rules the add-on needs: ``event:`` names the event (default
    ``"message"``), several ``data:`` lines are joined with newlines, a blank line
    dispatches, and lines starting with ``:`` (keep-alive comments) are ignored.
    """
    event = "message"
    data: list[str] = []
    for raw in lines:
        line = raw.rstrip("\r\n")
        if line == "":
            if data:
                yield event, "\n".join(data)
            event = "message"
            data = []
            continue
        if line.startswith(":"):
            continue
        field, _, value = line.partition(":")
        value = value[1:] if value.startswith(" ") else value
        if field == "event":
            event = value
        elif field == "data":
            data.append(value)
    if data:
        yield event, "\n".join(data)


class LinkClient:
    """Thin client over the Link API. One instance per port."""

    def __init__(self, port: int = DEFAULT_PORT, timeout: float = DEFAULT_TIMEOUT):
        self.port = int(port)
        self.timeout = timeout

    @property
    def base(self) -> str:
        return base_url(self.port)

    # ---- transport -------------------------------------------------------

    def _request(self, method: str, path: str, body: Any = None) -> Any:
        url = self.base + path
        data = None
        headers = {"Accept": "application/json"}
        if body is not None:
            data = json.dumps(body).encode("utf-8")
            headers["Content-Type"] = "application/json"
        req = urllib.request.Request(url, data=data, method=method, headers=headers)
        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as res:
                text = res.read().decode("utf-8")
        except urllib.error.HTTPError as e:
            detail = e.read().decode("utf-8", errors="replace")
            raise LinkError(f"Link API {e.code} on {method} {path}: {detail}") from e
        except urllib.error.URLError as e:
            raise LinkError(
                f"MadakeCAD is not reachable on port {self.port} ({e.reason}). "
                "Start MadakeCAD, then connect again."
            ) from e
        if not text:
            return None
        try:
            return json.loads(text)
        except json.JSONDecodeError as e:
            raise LinkError(f"Link API returned invalid JSON on {method} {path}") from e

    # ---- reads -------------------------------------------------------------

    def health(self) -> dict:
        """``GET /api/v1`` — name and version of the Link API (connection check)."""
        return self._request("GET", "")

    def project(self) -> dict:
        """``GET /api/v1/project`` — ``{revision, project, can_undo, can_redo}``."""
        return self._request("GET", "/project")

    def netlist(self, sheet_id: str | None = None) -> list:
        """``GET /api/v1/netlist[?sheet_id=]`` — nets of one sheet (default: first sheet)."""
        path = "/netlist"
        if sheet_id:
            path += "?" + urllib.parse.urlencode({"sheet_id": sheet_id})
        return self._request("GET", path)

    def parts(self, query: str = "", category: str | None = None) -> list:
        """``GET /api/v1/parts[?query=&category=]`` — parts database search."""
        params = {}
        if query:
            params["query"] = query
        if category:
            params["category"] = category
        path = "/parts"
        if params:
            path += "?" + urllib.parse.urlencode(params)
        return self._request("GET", path)

    # ---- writes (always through the Command engine) ---------------------

    def post_commands(self, commands: list) -> list:
        """``POST /api/v1/commands`` — run a Command array, returns one patch per command."""
        return self._request("POST", "/commands", commands)

    # ---- live follow -----------------------------------------------------

    def events(self) -> Iterator[tuple[str, Any]]:
        """``GET /api/v1/events`` — yields ``(event, payload)`` until the stream closes.

        Patches arrive as ``("patch", {revision, ops})``. Blocks while waiting; run it
        in a thread. Raises :class:`LinkError` when the connection cannot be opened.
        """
        req = urllib.request.Request(self.base + "/events", headers={"Accept": "text/event-stream"})
        try:
            res = urllib.request.urlopen(req, timeout=None)
        except urllib.error.URLError as e:
            raise LinkError(f"Could not subscribe to MadakeCAD events on port {self.port} ({e.reason})") from e
        with res:
            lines = (line.decode("utf-8", errors="replace") for line in res)
            for event, data in parse_sse(lines):
                try:
                    payload = json.loads(data)
                except json.JSONDecodeError:
                    payload = data
                yield event, payload
