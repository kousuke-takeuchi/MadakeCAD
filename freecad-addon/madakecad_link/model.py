"""Pure helpers that turn Link API JSON into what the panel shows."""

from __future__ import annotations

from dataclasses import dataclass, field

ENTITY_KINDS = ("symbol", "wire", "junction", "net_label", "text", "harness")


@dataclass
class SheetSummary:
    id: str
    name: str
    size: str
    orientation: str
    counts: dict = field(default_factory=dict)

    @property
    def label(self) -> str:
        """Text for the sheet selector, e.g. ``Sheet1 (A3 landscape)``."""
        return f"{self.name} ({self.size} {self.orientation.lower()})"


@dataclass
class ProjectSummary:
    name: str
    revision: int
    sheets: list


def entity_counts(sheet: dict) -> dict:
    """Number of entities per kind on a sheet (every kind present, zero when absent)."""
    counts = {kind: 0 for kind in ENTITY_KINDS}
    for entity in (sheet.get("entities") or {}).values():
        kind = entity.get("kind", "")
        counts[kind] = counts.get(kind, 0) + 1
    return counts


def summarize_project(snapshot: dict) -> ProjectSummary:
    """Summary of ``GET /project``: name, revision and one entry per sheet with counts."""
    project = snapshot.get("project") or {}
    sheets = [
        SheetSummary(
            id=s.get("id", ""),
            name=s.get("name", ""),
            size=s.get("size", ""),
            orientation=s.get("orientation", ""),
            counts=entity_counts(s),
        )
        for s in project.get("sheets") or []
    ]
    return ProjectSummary(name=project.get("name", ""), revision=int(snapshot.get("revision") or 0), sheets=sheets)


def summary_text(summary: ProjectSummary) -> str:
    """One-line status text, e.g. ``demo · rev 12 · 2 sheets · 14 symbols · 20 wires``."""
    symbols = sum(s.counts.get("symbol", 0) for s in summary.sheets)
    wires = sum(s.counts.get("wire", 0) for s in summary.sheets)
    plural = "s" if len(summary.sheets) != 1 else ""
    return f"{summary.name} · rev {summary.revision} · {len(summary.sheets)} sheet{plural} · {symbols} symbols · {wires} wires"


@dataclass
class NetRow:
    name: str
    wire_no: str
    label: str
    pins: str
    pin_count: int
    wire_count: int


def format_pins(pins: list) -> str:
    """``K1:A1, TB1:3`` — designator and pin number of every pin on the net."""
    return ", ".join(f"{p.get('reference') or '?'}:{p.get('pin', '')}" for p in pins)


def netlist_rows(nets: list) -> list:
    """Rows for the netlist table, in the order the API returns the nets."""
    return [
        NetRow(
            name=n.get("name", ""),
            wire_no=n.get("wire_no") or "",
            label=n.get("label") or "",
            pins=format_pins(n.get("pins") or []),
            pin_count=len(n.get("pins") or []),
            wire_count=len(n.get("wire_ids") or []),
        )
        for n in nets
    ]


NETLIST_COLUMNS = ("Net", "Wire no.", "Label", "Pins", "Wires")


def netlist_cells(row: NetRow) -> tuple:
    """The row's cells in :data:`NETLIST_COLUMNS` order."""
    return (row.name, row.wire_no, row.label, row.pins, str(row.wire_count))
