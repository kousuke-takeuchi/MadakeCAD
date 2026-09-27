"""Live follow: decide from a patch whether the panel must refresh."""

from __future__ import annotations

# Ops that change the sheet list or a project-wide table: always refresh, whatever
# sheet is shown. `mech_links_replaced` is one of them: it carries no sheet id, and it
# is what undoing a link in MadakeCAD sends — the Parts/Wires tabs would otherwise keep
# showing links (and placements) that no longer exist.
STRUCTURAL_OPS = {
    "project_replaced",
    "sheet_added",
    "sheet_removed",
    "sheet_meta_updated",
    "wire_parts_replaced",
    "plc_assignments_replaced",
    "frame_template_changed",
    "project_meta_changed",
    "mech_links_replaced",
}


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


def op_sheet_id(op: dict) -> str | None:
    """The sheet an op belongs to: its ``sheet_id``, or the id of the ``sheet`` it carries.

    ``None`` means the op is not about one sheet but about the whole project (the
    FreeCAD link list, the wire-part table, the project name, ...).
    """
    sheet_id = op.get("sheet_id")
    if sheet_id:
        return sheet_id
    sheet = op.get("sheet")
    if isinstance(sheet, dict) and sheet.get("id"):
        return sheet["id"]
    return None


def refresh_needed(patch: dict, shown_sheet_id: str | None) -> bool:
    """Whether the panel showing ``shown_sheet_id`` must reload after ``patch``.

    Structural ops (project replaced, sheets added/removed/renamed) and anything
    project-wide (FreeCAD links, project-wide tables — ops that carry no sheet) always
    need a reload. Entity ops need one only when they touch the shown sheet; with no
    sheet shown yet, any entity op triggers a reload so the first view is not stale.
    """
    for op in patch.get("ops") or []:
        if op.get("op") in STRUCTURAL_OPS:
            return True
        scope = op_sheet_id(op)
        if scope is None or shown_sheet_id is None or scope == shown_sheet_id:
            return True
    return False
