"""Route sync, net highlighting and placement write-back (M5-3), pure helpers.

- Route stubs (MadakeCAD → FreeCAD): for every wire whose net joins two linked parts, a
  two-point line between the parts' placements, tagged with the wire's ``madake_id``.
- Net highlight: the FreeCAD object names (parts and routes) belonging to a net.
- Placements (FreeCAD → MadakeCAD): ``set_mech_link`` commands carrying the object's
  placement, stored in ``Project.mech_links[].placement`` for the future panel-layout sheet.
"""

from __future__ import annotations

from dataclasses import dataclass

from .linking import links_by_entity, now_iso


@dataclass
class RouteStub:
    wire_id: str
    net: str
    from_object: str
    to_object: str
    start: tuple
    end: tuple

    @property
    def name(self) -> str:
        """FreeCAD object name for the stub: ``Route_<net>`` (net sanitised)."""
        safe = "".join(c if c.isalnum() else "_" for c in (self.net or "wire"))
        return f"Route_{safe}"


def linked_objects_of_net(net: dict, links: dict) -> list:
    """Object names of the linked parts on a net, in name order, without duplicates."""
    names = []
    for pin in net.get("pins") or []:
        link = links.get(pin.get("entity_id"))
        if link and link.get("object_name") and link["object_name"] not in names:
            names.append(link["object_name"])
    return sorted(names)


def route_stubs(snapshot: dict, nets: list, placements: dict, existing_route_ids: set | None = None) -> list:
    """Stubs to create for ``nets`` (one sheet's netlist).

    ``placements`` maps object name → ``(x, y, z)`` in mm of the objects present in the
    FreeCAD document. The linked parts of a net are chained in name order; segment *i*
    is assigned to the net's *i*-th wire id. Wires that already have a route object
    (``existing_route_ids``) and segments without a wire are skipped, so re-running
    never creates duplicates.
    """
    links = links_by_entity(snapshot)
    existing = existing_route_ids or set()
    stubs = []
    for net in nets:
        objects = [n for n in linked_objects_of_net(net, links) if n in placements]
        wire_ids = list(net.get("wire_ids") or [])
        for i in range(min(len(objects) - 1, len(wire_ids))):
            wire_id = wire_ids[i]
            if wire_id in existing:
                continue
            stubs.append(
                RouteStub(
                    wire_id=wire_id,
                    net=net.get("name", ""),
                    from_object=objects[i],
                    to_object=objects[i + 1],
                    start=tuple(placements[objects[i]]),
                    end=tuple(placements[objects[i + 1]]),
                )
            )
    return stubs


def net_highlight_objects(net: dict, snapshot: dict) -> list:
    """Object names to select in FreeCAD for a net: its linked parts and its linked routes."""
    links = links_by_entity(snapshot)
    names = linked_objects_of_net(net, links)
    for wire_id in net.get("wire_ids") or []:
        link = links.get(wire_id)
        if link and link.get("object_name") and link["object_name"] not in names:
            names.append(link["object_name"])
    return names


def placement_command(link: dict, base: tuple, rotation_deg: float, synced_at: str | None = None) -> dict:
    """``set_mech_link`` carrying the object's placement (mm, 3 decimals; degrees, 2 decimals)."""
    x, y, z = (round(float(v), 3) for v in base)
    return {
        "type": "set_mech_link",
        "link": {
            "entity_id": link.get("entity_id"),
            "fcstd_path": link.get("fcstd_path") or "",
            "object_name": link.get("object_name") or "",
            "synced_at": synced_at or now_iso(),
            "placement": {"x_mm": x, "y_mm": y, "z_mm": z, "rotation_deg": round(float(rotation_deg), 2)},
        },
    }


def placement_commands(snapshot: dict, placements: dict, synced_at: str | None = None) -> list:
    """One ``set_mech_link`` per linked object present in ``placements``
    (object name → ``((x, y, z), rotation_deg)``), skipping links whose stored placement already matches."""
    out = []
    for link in (snapshot.get("project") or {}).get("mech_links") or []:
        name = link.get("object_name")
        if name not in placements:
            continue
        base, rotation = placements[name]
        cmd = placement_command(link, base, rotation, synced_at)
        if link.get("placement") == cmd["link"]["placement"]:
            continue
        out.append(cmd)
    return out


def format_placement(placement: dict | None) -> str:
    """``(120, 45.5, 0) mm / 90°`` for the panel; empty when unplaced."""
    if not placement:
        return ""
    fmt = lambda v: f"{v:g}"  # noqa: E731
    return f"({fmt(placement.get('x_mm', 0))}, {fmt(placement.get('y_mm', 0))}, {fmt(placement.get('z_mm', 0))}) mm / {fmt(placement.get('rotation_deg', 0))}°"
