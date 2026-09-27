"""Live-follow and settings specification tests."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from madakecad_link.events import PatchFollower, refresh_needed  # noqa: E402
from madakecad_link.settings import MemorySettings, normalize_port  # noqa: E402


class PatchFollowerTest(unittest.TestCase):
    def test_only_newer_revisions_are_accepted(self):
        """A patch is applied only when its revision is newer than the last one seen; duplicates and older patches are ignored.
        patchは前回より新しいrevisionのときだけ受け入れられ、重複や古いpatchは無視される。"""
        follower = PatchFollower(revision=5)
        self.assertFalse(follower.accept({"revision": 5, "ops": []}))
        self.assertFalse(follower.accept({"revision": 3, "ops": []}))
        self.assertTrue(follower.accept({"revision": 6, "ops": []}))
        self.assertEqual(follower.revision, 6)
        self.assertFalse(follower.accept({"revision": 6, "ops": []}))

    def test_refresh_is_needed_for_the_shown_sheet_and_structural_changes(self):
        """The panel reloads when the shown sheet's entities change or when sheets are added, removed, renamed or the project is replaced, but not for edits on another sheet.
        表示中シートの要素が変わったとき、またはシートの追加・削除・改名やプロジェクト置換のときにパネルは再読込し、別シートの編集では再読込しない。"""
        self.assertTrue(refresh_needed({"ops": [{"op": "entity_upserted", "sheet_id": "s1"}]}, "s1"))
        self.assertFalse(refresh_needed({"ops": [{"op": "entity_upserted", "sheet_id": "s2"}]}, "s1"))
        self.assertTrue(refresh_needed({"ops": [{"op": "sheet_added", "index": 1}]}, "s1"))
        self.assertTrue(refresh_needed({"ops": [{"op": "project_replaced"}]}, "s1"))
        self.assertTrue(refresh_needed({"ops": [{"op": "sheet_meta_updated", "sheet": {"id": "s2"}}]}, "s1"))
        self.assertFalse(refresh_needed({"ops": []}, "s1"))

    def test_with_no_sheet_shown_yet_any_entity_change_reloads(self):
        """Before a sheet has been chosen, any entity change reloads the panel so the first view is current.
        シートをまだ選んでいない間は、どの要素の変更でも再読込して最初の表示が最新になるようにする。"""
        self.assertTrue(refresh_needed({"ops": [{"op": "entity_upserted", "sheet_id": "s2"}]}, None))


class SettingsTest(unittest.TestCase):
    def test_port_setting_accepts_valid_ports_and_falls_back_to_the_default(self):
        """The port setting keeps integers from 1 to 65535 and falls back to 9310 for anything else (text, 0, too large).
        ポート設定は1〜65535の整数を保持し、それ以外(文字列・0・大きすぎる値)は既定の9310へ戻る。"""
        self.assertEqual(normalize_port(9400), 9400)
        self.assertEqual(normalize_port("9400"), 9400)
        for bad in ("abc", None, 0, 70000):
            self.assertEqual(normalize_port(bad), 9310)
        settings = MemorySettings()
        self.assertEqual(settings.get_port(), 9310)
        self.assertEqual(settings.set_port(9500), 9500)
        self.assertEqual(settings.get_port(), 9500)
        self.assertEqual(settings.set_port("x"), 9310)


if __name__ == "__main__":
    unittest.main()
