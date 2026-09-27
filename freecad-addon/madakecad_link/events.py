"""Live follow: decide from a patch whether the panel must refresh."""

from __future__ import annotations

# Ops that change the sheet list or the whole project: always refresh.
STRUCTURAL_OPS = {"project_replaced", "sheet_added", "sheet_removed", "sheet_meta_updated"}


class PatchFollower:
    """Tracks the last seen revision so duplicate or stale patches are ignored.

    Patches arrive both from the SSE stream and, later, from replies to our own
    commands; MadakeCAD's revision is monotonic, so anything at or below the last
    seen revision has already been applied.
    """

    def __init__(self, revision: int = 0):
        self.revision = int(revision)

    def accept(self, patch: dict) -> bool:
        """Record the patch's revision; ``True`` when it is newer than what we have."""
        revision = int(patch.get("revision") or 0)
        if revision <= self.revision:
            return False
        self.revision = revision
        return True


def refresh_needed(patch: dict, shown_sheet_id: str | None) -> bool:
    """Whether the panel showing ``shown_sheet_id`` must reload after ``patch``.

    Structural ops (project replaced, sheets added/removed/renamed) always need a
    reload. Entity ops need one only when they touch the shown sheet; with no sheet
    shown yet, any entity op triggers a reload so the first view is not stale.
    """
    for op in patch.get("ops") or []:
        kind = op.get("op")
        if kind in STRUCTURAL_OPS or kind in ("wire_parts_replaced", "plc_assignments_replaced"):
            return True
        if shown_sheet_id is None or op.get("sheet_id") == shown_sheet_id:
            return True
    return False
