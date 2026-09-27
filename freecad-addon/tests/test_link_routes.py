"""Route sync, net highlight and placement write-back specification tests."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from madakecad_link.routes import (  # noqa: E402
    format_placement,
    net_highlight_objects,
    placement_command,
    placement_commands,
    route_stubs,
)

SNAPSHOT = {
    "project": {
        "mech_links": [
            {"entity_id": "k1", "fcstd_path": "/p.FCStd", "object_name": "Relay001", "synced_at": ""},
            {"entity_id": "tb1", "fcstd_path": "/p.FCStd", "object_name": "Terminal001", "synced_at": ""},
            {"entity_id": "pb1", "fcstd_path": "/p.FCStd", "object_name": "Button001", "synced_at": "", "placement": {"x_mm": 10.0, "y_mm": 20.0, "z_mm": 0.0, "rotation_deg": 0.0}},
            {"entity_id": "w9", "fcstd_path": "/p.FCStd", "object_name": "Route_101", "synced_at": ""},
        ],
        "sheets": [],
    }
}
PLACEMENTS = {"Relay001": (100.0, 50.0, 0.0), "Terminal001": (10.0, 50.0, 0.0), "Button001": (10.0, 20.0, 0.0)}


def net(name, pins, wire_ids):
    return {"name": name, "pins": [{"reference": r, "entity_id": e, "pin": "1"} for r, e in pins], "wire_ids": wire_ids}


class RouteStubsTest(unittest.TestCase):
    def test_a_wire_between_two_linked_parts_gets_one_stub_between_their_placements(self):
        """A wire whose net joins two linked parts gets one route stub from one part's placement to the other's, tagged with the wire id and named after the net.
        両端の部品が対応付け済みの配線には、一方の部品の配置から他方の配置へ向かう経路スタブが1本でき、配線idとネット名を持つ。"""
        stubs = route_stubs(SNAPSHOT, [net("101", [("K1", "k1"), ("TB1", "tb1")], ["w1"])], PLACEMENTS)
        self.assertEqual(len(stubs), 1)
        stub = stubs[0]
        self.assertEqual((stub.wire_id, stub.net, stub.name), ("w1", "101", "Route_101"))
        self.assertEqual((stub.from_object, stub.to_object), ("Relay001", "Terminal001"))
        self.assertEqual((stub.start, stub.end), ((100.0, 50.0, 0.0), (10.0, 50.0, 0.0)))

    def test_parts_without_a_link_or_not_in_the_document_produce_no_stub(self):
        """Nets whose parts are not linked, or whose linked objects are missing from the FreeCAD document, produce no stub.
        部品が対応付けられていないネットや、対応付け先のオブジェクトがFreeCADドキュメントに無いネットにはスタブができない。"""
        self.assertEqual(route_stubs(SNAPSHOT, [net("N1", [("K1", "k1"), ("X1", "x1")], ["w1"])], PLACEMENTS), [])
        self.assertEqual(route_stubs(SNAPSHOT, [net("N2", [("K1", "k1"), ("TB1", "tb1")], ["w1"])], {"Relay001": (0, 0, 0)}), [])

    def test_a_net_with_three_parts_is_chained_and_segments_follow_the_wire_ids(self):
        """Three linked parts on one net are chained in name order into two segments, assigned to the net's wires in order; extra wires get no stub and extra segments are dropped when wires run out.
        1ネットに対応付け済み部品が3つあると名前順に2区間の鎖になり、ネットの配線へ順に割り当てる。余った配線にはスタブを作らず、配線が足りなければ余った区間は捨てる。"""
        three = [("K1", "k1"), ("TB1", "tb1"), ("PB1", "pb1")]
        stubs = route_stubs(SNAPSHOT, [net("24V", three, ["w1", "w2", "w3"])], PLACEMENTS)
        self.assertEqual([(s.wire_id, s.from_object, s.to_object) for s in stubs], [("w1", "Button001", "Relay001"), ("w2", "Relay001", "Terminal001")])
        only_one = route_stubs(SNAPSHOT, [net("24V", three, ["w1"])], PLACEMENTS)
        self.assertEqual([s.wire_id for s in only_one], ["w1"])

    def test_rerunning_skips_wires_that_already_have_a_route_object(self):
        """Wires that already have a route object in the document are skipped, so creating stubs again never duplicates them.
        すでに経路オブジェクトのある配線は飛ばすので、スタブ生成を再実行しても重複しない。"""
        nets = [net("101", [("K1", "k1"), ("TB1", "tb1")], ["w1"])]
        self.assertEqual(route_stubs(SNAPSHOT, nets, PLACEMENTS, existing_route_ids={"w1"}), [])
        self.assertEqual(len(route_stubs(SNAPSHOT, nets, PLACEMENTS, existing_route_ids={"w2"})), 1)


class NetHighlightTest(unittest.TestCase):
    def test_highlighting_a_net_selects_its_linked_parts_and_routes_once_each(self):
        """Highlighting a net selects the FreeCAD objects of its linked parts and of its linked route wires, each object once, ignoring unlinked pins.
        ネットのハイライトは、対応付け済み部品と経路の各オブジェクトを1回ずつ選び、未対応付けのピンは無視する。"""
        n = net("101", [("K1", "k1"), ("K1", "k1"), ("X1", "x1"), ("TB1", "tb1")], ["w9", "w1"])
        self.assertEqual(net_highlight_objects(n, SNAPSHOT), ["Relay001", "Terminal001", "Route_101"])
        self.assertEqual(net_highlight_objects(net("N", [], []), SNAPSHOT), [])


class PlacementTest(unittest.TestCase):
    def test_placement_command_carries_the_base_in_mm_and_the_rotation_in_degrees(self):
        """Syncing a placement sends set_mech_link with the object's base point in millimetres (1 µm resolution) and its rotation in degrees, keeping the existing link data.
        配置の同期はset_mech_linkで基点をmm(1µm単位)・回転を度で送り、既存の対応付け情報は保つ。"""
        link = SNAPSHOT["project"]["mech_links"][0]
        cmd = placement_command(link, (100.00049, 50.0, 0.0), 89.999, "2026-09-27T00:00:00Z")
        self.assertEqual(cmd["type"], "set_mech_link")
        self.assertEqual(cmd["link"]["object_name"], "Relay001")
        self.assertEqual(cmd["link"]["fcstd_path"], "/p.FCStd")
        self.assertEqual(cmd["link"]["placement"], {"x_mm": 100.0, "y_mm": 50.0, "z_mm": 0.0, "rotation_deg": 90.0})

    def test_sync_placements_writes_only_linked_objects_whose_placement_changed(self):
        """Sync placements writes one command per linked object present in the document and skips objects whose stored placement is already the same.
        配置の同期はドキュメントにある対応付け済みオブジェクトごとに1コマンド送り、保存済みの配置と同じものは飛ばす。"""
        placements = {"Relay001": ((100.0, 50.0, 0.0), 0.0), "Button001": ((10.0, 20.0, 0.0), 0.0), "Unlinked": ((1, 2, 3), 0.0)}
        cmds = placement_commands(SNAPSHOT, placements, "2026-09-27T00:00:00Z")
        self.assertEqual([c["link"]["object_name"] for c in cmds], ["Relay001"])
        self.assertEqual(format_placement({"x_mm": 120, "y_mm": 45.5, "z_mm": 0, "rotation_deg": 90}), "(120, 45.5, 0) mm / 90°")
        self.assertEqual(format_placement(None), "")


if __name__ == "__main__":
    unittest.main()
