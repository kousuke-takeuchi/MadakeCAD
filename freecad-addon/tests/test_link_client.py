"""Link API client specification tests (run: python3 -m unittest discover -s freecad-addon/tests)."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))

from _fake_server import Handler, start  # noqa: E402
from madakecad_link.client import DEFAULT_PORT, LinkClient, LinkError, base_url, parse_sse  # noqa: E402


class LinkClientTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.server, cls.port = start()

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()

    def setUp(self):
        Handler.calls = []
        self.client = LinkClient(self.port)

    def test_base_url_points_at_the_local_link_api(self):
        """The client talks to MadakeCAD's Link API on localhost, port 9310 unless told otherwise.
        クライアントはローカルのMadakeCAD Link API(既定ポート9310)へ接続する。"""
        self.assertEqual(base_url(), "http://127.0.0.1:9310/api/v1")
        self.assertEqual(DEFAULT_PORT, 9310)
        self.assertEqual(LinkClient(9400).base, "http://127.0.0.1:9400/api/v1")

    def test_health_and_project_are_decoded_from_json(self):
        """The connection check and the project snapshot come back as decoded JSON.
        接続確認とプロジェクトのスナップショットはJSONを解釈した辞書で返る。"""
        self.assertEqual(self.client.health()["name"], "MadakeCAD Link API")
        snapshot = self.client.project()
        self.assertEqual(snapshot["revision"], 7)
        self.assertEqual(snapshot["project"]["name"], "demo")

    def test_netlist_and_parts_pass_their_filters_as_query_parameters(self):
        """Netlist and parts requests pass the sheet id, search text and category as query parameters.
        ネットリストと部品の要求は、シートid・検索語・カテゴリをクエリパラメータで渡す。"""
        self.client.netlist()
        self.client.netlist("s1")
        self.client.parts("relay", category="relay")
        paths = [c[1] for c in Handler.calls]
        self.assertEqual(paths, ["/api/v1/netlist", "/api/v1/netlist?sheet_id=s1", "/api/v1/parts?query=relay&category=relay"])

    def test_commands_are_posted_as_a_json_array_and_return_one_patch_each(self):
        """Commands are posted as a JSON array and one patch comes back per command, so every write goes through MadakeCAD's Command engine.
        コマンドはJSON配列として送られ、コマンドごとにpatchが1つ返る(全ての書き込みがMadakeCADのCommandエンジンを通る)。"""
        patches = self.client.post_commands([{"type": "rename_sheet", "sheet_id": "s1", "name": "Main"}])
        self.assertEqual(len(patches), 1)
        self.assertEqual(Handler.calls[-1][2][0]["name"], "Main")

    def test_errors_are_reported_as_link_errors_with_a_readable_message(self):
        """A refused connection, an HTTP error and invalid JSON all raise LinkError with a message that says what went wrong.
        接続拒否・HTTPエラー・不正なJSONはいずれも、原因が読める文面のLinkErrorになる。"""
        with self.assertRaises(LinkError) as refused:
            LinkClient(1).health()
        self.assertIn("port 1", str(refused.exception))
        with self.assertRaises(LinkError) as http_error:
            self.client.post_commands([{"type": "bad"}])
        self.assertIn("400", str(http_error.exception))
        self.assertIn("sheet not found", str(http_error.exception))
        with self.assertRaises(LinkError):
            self.client._request("GET", "/broken")

    def test_events_yield_patches_from_the_sse_stream(self):
        """The event stream yields each patch as ("patch", {revision, ops}), joining multi-line data and skipping keep-alive comments.
        イベントストリームはpatchごとに("patch", {revision, ops})を返し、複数行のdataは結合し、keep-aliveのコメント行は読み飛ばす。"""
        events = list(self.client.events())
        self.assertEqual([e for e, _ in events], ["patch", "patch"])
        self.assertEqual(events[0][1]["revision"], 8)
        self.assertEqual(events[1][1]["ops"][0]["op"], "entity_removed")

    def test_parse_sse_defaults_the_event_name_and_dispatches_on_blank_lines(self):
        """Lines without an event name are "message" events, data is dispatched at the blank line, and a trailing event without a blank line is still delivered.
        イベント名の無い行は"message"イベントになり、dataは空行で確定し、末尾の空行が無いイベントも届く。"""
        pairs = list(parse_sse(["data: a", "", "event: patch", "data: {}", "", "data: tail"]))
        self.assertEqual(pairs, [("message", "a"), ("patch", "{}"), ("message", "tail")])


if __name__ == "__main__":
    unittest.main()
