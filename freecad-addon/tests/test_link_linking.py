"""Part linking and wire-length write-back specification tests."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from madakecad_link.linking import (  # noqa: E402
    length_m_from_mm,
    link_commands_for_routes,
    measurements_from_objects,
    model_kind,
    part_rows,
    remove_mech_link_command,
    set_mech_link_command,
    wire_length_commands,
    wire_rows,
)

SNAPSHOT = {
    "revision": 3,
    "project": {
        "name": "demo",
        "mech_links": [
            {"entity_id": "k1", "fcstd_path": "/work/panel.FCStd", "object_name": "Relay001", "synced_at": "2026-09-27T00:00:00Z"}
        ],
        "sheets": [
            {
                "id": "s1",
                "name": "Sheet1",
                "entities": {
                    "k1": {"kind": "symbol", "id": "k1", "reference": "K1", "value": "MY2N"},
                    "tb1": {"kind": "symbol", "id": "tb1", "reference": "TB1", "value": ""},
                    "w1": {"kind": "wire", "id": "w1", "net": "101", "length_m": 0.4, "length_source": "manual"},
                    "w2": {"kind": "wire", "id": "w2", "net": None, "length_m": None},
                },
            }
        ],
    },
}
PARTS = [{"part_no": "MY2N", "model_3d": "/models/MY2N.step"}, {"part_no": "OTHER", "model_3d": ""}]


class PartRowsTest(unittest.TestCase):
    def test_part_rows_list_symbols_with_their_3d_model_and_linked_object(self):
        """The parts list has one row per symbol, sorted by sheet and designator, with the 3D model of its part number and the FreeCAD object it is linked to.
        部品リストはシンボルごとに1行(シート・参照記号順)で、型番の3Dモデルと対応付け済みのFreeCADオブジェクト名を持つ。"""
        rows = part_rows(SNAPSHOT, PARTS)
        self.assertEqual([r.reference for r in rows], ["K1", "TB1"])
        k1 = rows[0]
        self.assertEqual((k1.entity_id, k1.part_no, k1.model_3d, k1.linked_object), ("k1", "MY2N", "/models/MY2N.step", "Relay001"))
        self.assertEqual((rows[1].model_3d, rows[1].linked_object), ("", ""))

    def test_wire_rows_show_net_length_source_and_linked_route(self):
        """The wire list has one row per wire with its net or wire number, current length, where the length came from, and the linked route object.
        配線リストは配線ごとに1行で、ネット/線番・現在の長さ・長さの出所・対応付け済みの経路オブジェクトを持つ。"""
        rows = wire_rows(SNAPSHOT)
        self.assertEqual([(r.entity_id, r.net, r.length_m, r.length_source) for r in rows], [("w1", "101", 0.4, "manual"), ("w2", "", None, "manual")])
        self.assertEqual(rows[0].linked_object, "")

    def test_model_kind_recognises_shape_files_and_freecad_documents(self):
        """STEP/IGES/BREP files are inserted as shapes, .FCStd files are merged as documents, and anything else cannot be inserted.
        STEP/IGES/BREPはシェイプとして挿入、.FCStdはドキュメントとして取り込み、それ以外は挿入できない。"""
        self.assertEqual(model_kind("/m/a.STEP"), "shape")
        self.assertEqual(model_kind("/m/a.igs"), "shape")
        self.assertEqual(model_kind("/m/a.FCStd"), "fcstd")
        self.assertEqual(model_kind("/m/a.stl"), "")
        self.assertEqual(model_kind(""), "")


class LinkCommandsTest(unittest.TestCase):
    def test_link_commands_carry_the_entity_id_document_path_object_name_and_time(self):
        """Linking sends a set_mech_link command with the entity id, the FreeCAD document path, the object name and the sync time; unlinking sends remove_mech_link.
        対応付けはentity id・FreeCADドキュメントのパス・オブジェクト名・同期時刻を持つset_mech_linkコマンドを送り、解除はremove_mech_linkを送る。"""
        cmd = set_mech_link_command("k1", "/work/panel.FCStd", "Relay001", "2026-09-27T10:00:00Z")
        self.assertEqual(cmd, {"type": "set_mech_link", "link": {"entity_id": "k1", "fcstd_path": "/work/panel.FCStd", "object_name": "Relay001", "synced_at": "2026-09-27T10:00:00Z"}})
        self.assertTrue(set_mech_link_command("k1", "", "X")["link"]["synced_at"].endswith("Z"))
        self.assertEqual(remove_mech_link_command("k1"), {"type": "remove_mech_link", "entity_id": "k1"})


class WriteBackTest(unittest.TestCase):
    def test_measured_route_lengths_are_converted_to_metres_and_matched_to_wires(self):
        """Route objects are matched to wires by their madake_id; lengths are converted from millimetres to metres (1 mm resolution) and objects that are not wires are ignored.
        経路オブジェクトはmadake_idで配線と突き合わせ、長さはmmからm(1mm単位)へ換算し、配線でないオブジェクトは無視する。"""
        objects = [("Route001", "w1", 1234.56), ("Relay001", "k1", 50.0), ("Loose", "nope", 10.0), ("NoShape", "w2", None)]
        measurements = measurements_from_objects(objects, SNAPSHOT)
        self.assertEqual(measurements, [("s1", "w1", 1.235, "Route001")])
        self.assertEqual(length_m_from_mm(400), 0.4)

    def test_write_back_is_one_command_per_sheet_with_the_freecad_source(self):
        """The write-back is one set_wire_lengths command per sheet whose entries carry the metre length and the source "freecad", so MadakeCAD can warn before a manual overwrite.
        書き戻しはシートごとに1つのset_wire_lengthsコマンドで、各項目はm単位の長さと出所"freecad"を持つ(MadakeCADが手入力の上書きを警告できる)。"""
        commands = wire_length_commands([("s1", "w1", 1.235, "Route001"), ("s1", "w2", 0.5, "Route002"), ("s2", "w9", 2.0, "R")])
        self.assertEqual(len(commands), 2)
        self.assertEqual(commands[0]["type"], "set_wire_lengths")
        self.assertEqual(commands[0]["sheet_id"], "s1")
        self.assertEqual(commands[0]["lengths"], [{"wire_id": "w1", "length_m": 1.235, "source": "freecad"}, {"wire_id": "w2", "length_m": 0.5, "source": "freecad"}])
        self.assertEqual(wire_length_commands([]), [])

    def test_routes_are_linked_only_when_not_already_linked_to_that_object(self):
        """Measuring also registers the route object as the wire's link, but only when the wire is not already linked to that same object.
        計測時に経路オブジェクトを配線の対応付けとして登録するが、すでに同じオブジェクトに対応付いている配線は登録し直さない。"""
        snapshot = {"project": {"mech_links": [{"entity_id": "w1", "object_name": "Route001"}], "sheets": []}}
        measurements = [("s1", "w1", 1.0, "Route001"), ("s1", "w2", 2.0, "Route002")]
        commands = link_commands_for_routes(measurements, "/work/panel.FCStd", snapshot)
        self.assertEqual([c["link"]["entity_id"] for c in commands], ["w2"])
        self.assertEqual(commands[0]["link"]["object_name"], "Route002")


if __name__ == "__main__":
    unittest.main()
