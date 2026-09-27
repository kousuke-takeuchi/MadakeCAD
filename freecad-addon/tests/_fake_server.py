"""A tiny fake Link API for the client tests (standard library only)."""

from __future__ import annotations

import json
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

PROJECT = {
    "revision": 7,
    "project": {
        "format_version": 2,
        "name": "demo",
        "wire_parts": [],
        "plc_assignments": [],
        "sheets": [
            {
                "id": "s1",
                "name": "Sheet1",
                "size": "A3",
                "orientation": "Landscape",
                "entities": {
                    "e1": {"kind": "symbol", "id": "e1", "reference": "K1"},
                    "e2": {"kind": "wire", "id": "e2"},
                    "e3": {"kind": "wire", "id": "e3"},
                },
            }
        ],
    },
    "can_undo": False,
    "can_redo": False,
}

NETLIST = [
    {
        "name": "101",
        "pins": [{"reference": "K1", "entity_id": "e1", "pin": "A1"}, {"reference": "TB1", "entity_id": "e9", "pin": "3"}],
        "wire_ids": ["e2", "e3"],
        "label": None,
        "wire_no": "101",
    }
]


class Handler(BaseHTTPRequestHandler):
    calls: list = []

    def log_message(self, *args):  # quiet
        pass

    def _json(self, body, status=200):
        data = json.dumps(body).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        Handler.calls.append(("GET", self.path))
        if self.path == "/api/v1":
            return self._json({"name": "MadakeCAD Link API", "version": 1})
        if self.path == "/api/v1/project":
            return self._json(PROJECT)
        if self.path.startswith("/api/v1/netlist"):
            return self._json(NETLIST)
        if self.path.startswith("/api/v1/parts"):
            return self._json([{"part_no": "MY2N", "name": "relay"}])
        if self.path == "/api/v1/events":
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.end_headers()
            self.wfile.write(b": keep-alive\n\n")
            self.wfile.write(b'event: patch\ndata: {"revision": 8, "ops": []}\n\n')
            self.wfile.write(b'event: patch\ndata: {"revision": 9,\ndata:  "ops": [{"op": "entity_removed", "sheet_id": "s1", "id": "e2"}]}\n\n')
            self.wfile.flush()
            return
        if self.path == "/api/v1/broken":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(b"not json")
            return
        return self._json({"error": f"unknown {self.path}"}, 404)

    def do_POST(self):
        length = int(self.headers.get("Content-Length") or 0)
        body = json.loads(self.rfile.read(length) or b"null")
        Handler.calls.append(("POST", self.path, body))
        if self.path == "/api/v1/commands":
            if body and body[0].get("type") == "bad":
                return self._json({"error": "sheet not found"}, 400)
            return self._json([{"revision": 8, "ops": []} for _ in body])
        return self._json({"error": "unknown"}, 404)


def start() -> tuple[ThreadingHTTPServer, int]:
    """Start the fake server on a free port; returns (server, port)."""
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    Handler.calls = []
    return server, server.server_address[1]
