"""Part linking and wire-length write-back (M5-2), pure helpers.

Keys: MadakeCAD entity UUIDs. FreeCAD objects carry the same UUID in a ``madake_id``
property; MadakeCAD keeps ``Project.mech_links``. Every write is a Command sent through
``POST /api/v1/commands`` — ``set_mech_link`` / ``remove_mech_link`` / ``set_wire_lengths``.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timezone

#: 3D model files the add-on can insert directly (STEP / IGES / BREP via Part.read).
MODEL_EXTENSIONS = (".step", ".stp", ".iges", ".igs", ".brep", ".brp")
#: FreeCAD documents are merged rather than read as a shape.
FCSTD_EXTENSIONS = (".fcstd",)


def model_kind(path: str) -> str:
    """``"shape"`` for STEP/IGES/BREP, ``"fcstd"`` for FreeCAD documents, ``""`` otherwise."""
    lower = (path or "").lower()
    if lower.endswith(MODEL_EXTENSIONS):
        return "shape"
    if lower.endswith(FCSTD_EXTENSIONS):
        return "fcstd"
    return ""


@dataclass
class PartRow:
    entity_id: str
    sheet_id: str
    sheet_name: str
    reference: str
    part_no: str
    model_3d: str
    linked_object: str


@dataclass
class WireRow:
    entity_id: str
    sheet_id: str
    sheet_name: str
    net: str
    length_m: float | None
    length_source: str
    linked_object: str


def links_by_entity(snapshot: dict) -> dict:
    """``entity_id → mech_link`` from a ``GET /project`` snapshot."""
    project = snapshot.get("project") or {}
    return {link.get("entity_id"): link for link in project.get("mech_links") or []}


def part_rows(snapshot: dict, parts: list) -> list:
    """One row per symbol on every sheet, with the 3D model path of its part (matched by part number = symbol value) and the linked FreeCAD object, if any."""
    models = {p.get("part_no"): p.get("model_3d") or "" for p in parts or []}
    links = links_by_entity(snapshot)
    rows = []
    for sheet in (snapshot.get("project") or {}).get("sheets") or []:
        for entity in (sheet.get("entities") or {}).values():
            if entity.get("kind") != "symbol":
                continue
            part_no = entity.get("value") or ""
            rows.append(
                PartRow(
                    entity_id=entity.get("id", ""),
                    sheet_id=sheet.get("id", ""),
                    sheet_name=sheet.get("name", ""),
                    reference=entity.get("reference") or "",
                    part_no=part_no,
                    model_3d=models.get(part_no, ""),
                    linked_object=(links.get(entity.get("id")) or {}).get("object_name", ""),
                )
            )
    rows.sort(key=lambda r: (r.sheet_name, r.reference))
    return rows


def wire_rows(snapshot: dict) -> list:
    """One row per wire on every sheet: net/wire number, current length and its source, linked route object."""
    links = links_by_entity(snapshot)
    rows = []
    for sheet in (snapshot.get("project") or {}).get("sheets") or []:
        for entity in (sheet.get("entities") or {}).values():
            if entity.get("kind") != "wire":
                continue
            rows.append(
                WireRow(
                    entity_id=entity.get("id", ""),
                    sheet_id=sheet.get("id", ""),
                    sheet_name=sheet.get("name", ""),
                    net=entity.get("net") or "",
                    length_m=entity.get("length_m"),
                    length_source=entity.get("length_source") or "manual",
                    linked_object=(links.get(entity.get("id")) or {}).get("object_name", ""),
                )
            )
    # 採番済み (線番/ネット名あり) を先に、その中は名前順。未採番は後ろにid順
    rows.sort(key=lambda r: (r.sheet_name, r.net == "", r.net, r.entity_id))
    return rows


def now_iso() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def set_mech_link_command(entity_id: str, fcstd_path: str, object_name: str, synced_at: str | None = None) -> dict:
    """The ``set_mech_link`` command registering (or replacing) the link of one entity."""
    return {
        "type": "set_mech_link",
        "link": {
            "entity_id": entity_id,
            "fcstd_path": fcstd_path or "",
            "object_name": object_name,
            "synced_at": synced_at or now_iso(),
        },
    }


def remove_mech_link_command(entity_id: str) -> dict:
    return {"type": "remove_mech_link", "entity_id": entity_id}


def length_m_from_mm(length_mm: float) -> float:
    """FreeCAD shape lengths are millimetres; MadakeCAD stores metres rounded to 1 mm."""
    return round(float(length_mm) / 1000.0, 3)


def measurements_from_objects(objects: list, snapshot: dict) -> list:
    """Match FreeCAD objects to MadakeCAD wires.

    ``objects`` are ``(object_name, madake_id, length_mm)`` triples (what the panel reads
    from ``obj.madake_id`` and ``obj.Shape.Length``). Only ids that are wires in the
    project are kept; the result is ``[(sheet_id, wire_id, length_m, object_name)]``.
    """
    wires = {}
    for sheet in (snapshot.get("project") or {}).get("sheets") or []:
        for entity in (sheet.get("entities") or {}).values():
            if entity.get("kind") == "wire":
                wires[entity.get("id")] = sheet.get("id")
    out = []
    for object_name, madake_id, length_mm in objects:
        sheet_id = wires.get(madake_id)
        if sheet_id is None or length_mm is None:
            continue
        out.append((sheet_id, madake_id, length_m_from_mm(length_mm), object_name))
    return out


def wire_length_commands(measurements: list) -> list:
    """One ``set_wire_lengths`` command per sheet (source ``freecad``), from
    ``[(sheet_id, wire_id, length_m, object_name)]``. Empty input → no commands."""
    per_sheet: dict = {}
    for sheet_id, wire_id, length_m, _name in measurements:
        per_sheet.setdefault(sheet_id, []).append({"wire_id": wire_id, "length_m": length_m, "source": "freecad"})
    return [{"type": "set_wire_lengths", "sheet_id": sheet_id, "lengths": lengths} for sheet_id, lengths in per_sheet.items()]


def link_commands_for_routes(measurements: list, fcstd_path: str, snapshot: dict) -> list:
    """``set_mech_link`` commands for route objects that are not linked yet (or linked to another object)."""
    links = links_by_entity(snapshot)
    out = []
    for _sheet_id, wire_id, _length_m, object_name in measurements:
        current = (links.get(wire_id) or {}).get("object_name")
        if current != object_name:
            out.append(set_mech_link_command(wire_id, fcstd_path, object_name))
    return out
