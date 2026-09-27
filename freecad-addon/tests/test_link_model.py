"""Panel model specification tests."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))

from _fake_server import NETLIST, PROJECT  # noqa: E402
from madakecad_link.model import (  # noqa: E402
    NETLIST_COLUMNS,
    entity_counts,
    format_pins,
    netlist_cells,
    netlist_rows,
    summarize_project,
    summary_text,
)


class ProjectSummaryTest(unittest.TestCase):
    def test_summary_carries_name_revision_and_per_sheet_entity_counts(self):
        """The project summary shows the project name, the revision and, per sheet, how many symbols, wires and other entities it holds.
        プロジェクト概要にはプロジェクト名・revisionと、シートごとのシンボル・配線などの件数が入る。"""
        summary = summarize_project(PROJECT)
        self.assertEqual((summary.name, summary.revision), ("demo", 7))
        self.assertEqual(len(summary.sheets), 1)
        sheet = summary.sheets[0]
        self.assertEqual(sheet.label, "Sheet1 (A3 landscape)")
        self.assertEqual(sheet.counts["symbol"], 1)
        self.assertEqual(sheet.counts["wire"], 2)
        self.assertEqual(sheet.counts["junction"], 0)
        self.assertEqual(summary_text(summary), "demo · rev 7 · 1 sheet · 1 symbols · 2 wires")

    def test_an_empty_snapshot_summarizes_to_zero(self):
        """An empty or partial snapshot summarizes to an unnamed project with no sheets instead of failing.
        空または欠けたスナップショットは、失敗せずにシート無しの無名プロジェクトとして要約される。"""
        summary = summarize_project({})
        self.assertEqual((summary.name, summary.revision, summary.sheets), ("", 0, []))
        self.assertEqual(entity_counts({})["symbol"], 0)


class NetlistRowsTest(unittest.TestCase):
    def test_rows_show_net_name_wire_number_label_pins_and_wire_count(self):
        """Each netlist row shows the net name, wire number, label, the pins as "K1:A1, TB1:3" and how many wires form the net.
        ネットリストの各行には、ネット名・線番・ラベル・「K1:A1, TB1:3」形式のピン一覧・配線本数が並ぶ。"""
        rows = netlist_rows(NETLIST)
        self.assertEqual(len(rows), 1)
        row = rows[0]
        self.assertEqual(row.name, "101")
        self.assertEqual(row.wire_no, "101")
        self.assertEqual(row.label, "")
        self.assertEqual(row.pins, "K1:A1, TB1:3")
        self.assertEqual((row.pin_count, row.wire_count), (2, 2))
        self.assertEqual(netlist_cells(row), ("101", "101", "", "K1:A1, TB1:3", "2"))
        self.assertEqual(len(NETLIST_COLUMNS), len(netlist_cells(row)))

    def test_pins_without_a_designator_show_a_placeholder(self):
        """A pin whose symbol has no designator is shown as "?:<pin>" so the row still reads.
        参照記号の無いシンボルのピンは「?:<ピン番号>」と表示され、行はそのまま読める。"""
        self.assertEqual(format_pins([{"reference": "", "pin": "1"}]), "?:1")
        self.assertEqual(netlist_rows([]), [])


if __name__ == "__main__":
    unittest.main()
