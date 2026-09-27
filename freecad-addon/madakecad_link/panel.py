"""Qt dock panel: connection settings, project overview, netlist view, live follow.

Only importable inside FreeCAD (needs FreeCADGui and PySide). Everything that can be
tested without Qt is delegated to client.py / model.py / events.py / settings.py.
"""

from __future__ import annotations

import math

import FreeCADGui  # type: ignore
from PySide import QtCore, QtWidgets  # type: ignore

from .client import LinkClient, LinkError
from .events import PatchFollower, refresh_needed
from .linking import (
    link_commands_for_routes,
    measurements_from_objects,
    model_kind,
    part_rows,
    placement_of,
    remove_mech_link_command,
    set_mech_link_command,
    wire_length_commands,
    wire_rows,
)
from .model import NETLIST_COLUMNS, netlist_cells, netlist_rows, summarize_project, summary_text
from .routes import (
    format_placement,
    net_highlight_objects,
    placement_commands,
    rotation_deg_about_z,
    route_stubs,
    split_route_stubs,
)
from .settings import load_settings

#: How long to wait (ms) for the event thread to notice it was interrupted.
FOLLOW_STOP_TIMEOUT_MS = 3000

PARTS_COLUMNS = ("Sheet", "Ref", "Part no.", "3D model", "FreeCAD object", "Placement")
WIRES_COLUMNS = ("Sheet", "Net", "Length m", "Source", "Route object")
MADAKE_ID_PROPERTY = "madake_id"


class EventThread(QtCore.QThread):
    """Reads the SSE stream in the background and re-emits each patch on the GUI thread."""

    patch = QtCore.Signal(dict)
    ended = QtCore.Signal(str)

    def __init__(self, client: LinkClient, parent=None):
        super().__init__(parent)
        self._client = client

    def run(self):
        """Read the stream until it ends or is interrupted, always reporting the outcome.

        ``ended`` is emitted exactly once, whatever happens: the panel uses it to drop
        its reference to this thread, so an escaping exception would leave live follow
        permanently off for the rest of the FreeCAD session.
        """
        error = ""
        try:
            for event, payload in self._client.events():
                if self.isInterruptionRequested():
                    break
                if event == "patch" and isinstance(payload, dict):
                    self.patch.emit(payload)
        except LinkError as e:
            error = str(e)
        except Exception as e:  # noqa: BLE001 - FreeCADのコンソールへ例外を漏らさない
            error = f"Live follow stopped ({type(e).__name__}: {e})"
        self.ended.emit(error)


class LinkDockWidget(QtWidgets.QDockWidget):
    def __init__(self, parent=None):
        super().__init__("MadakeCAD Link", parent)
        self.setObjectName("MadakeCADLinkPanel")
        self._settings = load_settings()
        self._client = LinkClient(self._settings.get_port())
        self._follower = PatchFollower()
        self._thread = None
        self._summary = None
        self._build()

    # ---- UI ------------------------------------------------------------

    def _build(self):
        body = QtWidgets.QWidget()
        layout = QtWidgets.QVBoxLayout(body)

        row = QtWidgets.QHBoxLayout()
        row.addWidget(QtWidgets.QLabel("Port"))
        self._port = QtWidgets.QSpinBox()
        self._port.setRange(1, 65535)
        self._port.setValue(self._settings.get_port())
        row.addWidget(self._port)
        self._connect = QtWidgets.QPushButton("Connect")
        self._connect.clicked.connect(self.connect_now)
        row.addWidget(self._connect)
        self._follow = QtWidgets.QCheckBox("Follow live")
        self._follow.setChecked(True)
        self._follow.toggled.connect(self._on_follow_toggled)
        row.addWidget(self._follow)
        row.addStretch(1)
        layout.addLayout(row)

        self._status = QtWidgets.QLabel("Not connected")
        self._status.setWordWrap(True)
        layout.addWidget(self._status)

        sheet_row = QtWidgets.QHBoxLayout()
        sheet_row.addWidget(QtWidgets.QLabel("Sheet"))
        self._sheets = QtWidgets.QComboBox()
        self._sheets.currentIndexChanged.connect(lambda _i: self.load_netlist())
        sheet_row.addWidget(self._sheets, 1)
        refresh = QtWidgets.QPushButton("Refresh")
        refresh.clicked.connect(self.refresh)
        sheet_row.addWidget(refresh)
        layout.addLayout(sheet_row)

        self._tabs = QtWidgets.QTabWidget()
        layout.addWidget(self._tabs, 1)

        # --- Netlist tab ---
        self._table = self._make_table(NETLIST_COLUMNS, stretch=3)
        self._table.itemSelectionChanged.connect(self.highlight_net)
        self._nets = []
        self._tabs.addTab(self._table, "Netlist")

        # --- Parts tab (M5-2: 3D model insertion + linking) ---
        parts_page = QtWidgets.QWidget()
        parts_layout = QtWidgets.QVBoxLayout(parts_page)
        self._parts = self._make_table(PARTS_COLUMNS, stretch=3)
        parts_layout.addWidget(self._parts, 1)
        parts_buttons = QtWidgets.QHBoxLayout()
        insert = QtWidgets.QPushButton("Insert 3D model")
        insert.setToolTip("Insert the part's 3D model (STEP/IGES/BREP or FCStd) into the active document and link it")
        insert.clicked.connect(self.insert_model)
        parts_buttons.addWidget(insert)
        link = QtWidgets.QPushButton("Link selected object")
        link.setToolTip("Link the FreeCAD object selected in the 3D view to the selected part")
        link.clicked.connect(lambda: self.link_selected(self._parts, self._part_rows))
        parts_buttons.addWidget(link)
        unlink = QtWidgets.QPushButton("Unlink")
        unlink.clicked.connect(lambda: self.unlink_selected(self._parts, self._part_rows))
        parts_buttons.addWidget(unlink)
        sync = QtWidgets.QPushButton("Sync placements")
        sync.setToolTip("Write the placement (mm, Z rotation) of every linked object to MadakeCAD for the panel layout (one undo step)")
        sync.clicked.connect(self.sync_placements)
        parts_buttons.addWidget(sync)
        parts_buttons.addStretch(1)
        parts_layout.addLayout(parts_buttons)
        self._tabs.addTab(parts_page, "Parts")

        # --- Wires tab (M5-2: route linking + length write-back) ---
        wires_page = QtWidgets.QWidget()
        wires_layout = QtWidgets.QVBoxLayout(wires_page)
        self._wires = self._make_table(WIRES_COLUMNS, stretch=4)
        wires_layout.addWidget(self._wires, 1)
        wires_buttons = QtWidgets.QHBoxLayout()
        link_route = QtWidgets.QPushButton("Link selected route")
        link_route.setToolTip("Link the route object (Draft Wire, sketch, ...) selected in the 3D view to the selected wire")
        link_route.clicked.connect(lambda: self.link_selected(self._wires, self._wire_rows))
        wires_buttons.addWidget(link_route)
        unlink_route = QtWidgets.QPushButton("Unlink")
        unlink_route.clicked.connect(lambda: self.unlink_selected(self._wires, self._wire_rows))
        wires_buttons.addWidget(unlink_route)
        stubs = QtWidgets.QPushButton("Create route stubs")
        stubs.setToolTip("For every wire joining two linked parts, create a straight route line between them (tagged with the wire's madake_id) and link it to the wire; edit it into the real route, then measure")
        stubs.clicked.connect(self.create_route_stubs)
        wires_buttons.addWidget(stubs)
        measure = QtWidgets.QPushButton("Measure routes → write back lengths")
        measure.setToolTip("Measure every linked route object and write the lengths to MadakeCAD (one undo step)")
        measure.clicked.connect(self.write_back_lengths)
        wires_buttons.addWidget(measure)
        wires_buttons.addStretch(1)
        wires_layout.addLayout(wires_buttons)
        self._tabs.addTab(wires_page, "Wires")

        self._part_rows = []
        self._wire_rows = []
        self._snapshot = None
        self.setWidget(body)

    def _make_table(self, columns, stretch: int) -> QtWidgets.QTableWidget:
        table = QtWidgets.QTableWidget(0, len(columns))
        table.setHorizontalHeaderLabels(list(columns))
        table.horizontalHeader().setStretchLastSection(False)
        table.horizontalHeader().setSectionResizeMode(stretch, QtWidgets.QHeaderView.Stretch)
        table.setEditTriggers(QtWidgets.QAbstractItemView.NoEditTriggers)
        table.setSelectionBehavior(QtWidgets.QAbstractItemView.SelectRows)
        table.setSelectionMode(QtWidgets.QAbstractItemView.SingleSelection)
        return table

    @staticmethod
    def _fill(table: QtWidgets.QTableWidget, rows: list):
        table.setRowCount(len(rows))
        for r, cells in enumerate(rows):
            for c, cell in enumerate(cells):
                table.setItem(r, c, QtWidgets.QTableWidgetItem("" if cell is None else str(cell)))

    def attach_to_main_window(self):
        FreeCADGui.getMainWindow().addDockWidget(QtCore.Qt.RightDockWidgetArea, self)

    # ---- actions ---------------------------------------------------------

    def connect_now(self):
        port = self._settings.set_port(self._port.value())
        self._port.setValue(port)
        self._client = LinkClient(port)
        self._stop_follow()
        try:
            self._client.health()
        except LinkError as e:
            self._status.setText(str(e))
            return
        self.refresh()
        if self._follow.isChecked():
            self._start_follow()

    def refresh(self):
        """Reload the project overview and the netlist of the selected sheet."""
        try:
            snapshot = self._client.project()
        except LinkError as e:
            self._status.setText(str(e))
            return
        self._snapshot = snapshot
        self._summary = summarize_project(snapshot)
        self._follower.accept({"revision": self._summary.revision})
        self.load_links()
        self._status.setText(summary_text(self._summary))
        current = self.current_sheet_id()
        self._sheets.blockSignals(True)
        self._sheets.clear()
        for sheet in self._summary.sheets:
            self._sheets.addItem(sheet.label, sheet.id)
        index = self._sheets.findData(current) if current else -1
        self._sheets.setCurrentIndex(index if index >= 0 else 0)
        self._sheets.blockSignals(False)
        self.load_netlist()

    def current_sheet_id(self):
        data = self._sheets.currentData()
        return data if data else None

    def load_netlist(self):
        sheet_id = self.current_sheet_id()
        if not sheet_id:
            self._table.setRowCount(0)
            return
        try:
            nets = self._client.netlist(sheet_id)
        except LinkError as e:
            self._status.setText(str(e))
            return
        self._nets = nets
        rows = netlist_rows(nets)
        self._table.setRowCount(len(rows))
        for r, row in enumerate(rows):
            for c, cell in enumerate(netlist_cells(row)):
                self._table.setItem(r, c, QtWidgets.QTableWidgetItem(cell))

    # ---- M5-2: parts / wires / write-back --------------------------------

    def load_links(self):
        """Fill the Parts and Wires tabs from the current snapshot and the parts database."""
        if self._snapshot is None:
            return
        try:
            parts = self._client.parts()
        except LinkError:
            parts = []
        self._part_rows = part_rows(self._snapshot, parts)
        links = {l.get("entity_id"): l for l in (self._snapshot.get("project") or {}).get("mech_links") or []}
        self._fill(
            self._parts,
            [
                (r.sheet_name, r.reference, r.part_no, r.model_3d, r.linked_object, format_placement((links.get(r.entity_id) or {}).get("placement")))
                for r in self._part_rows
            ],
        )
        self._wire_rows = wire_rows(self._snapshot)
        self._fill(
            self._wires,
            [(r.sheet_name, r.net, r.length_m, r.length_source, r.linked_object) for r in self._wire_rows],
        )

    @staticmethod
    def _selected_row(table: QtWidgets.QTableWidget, rows: list):
        index = table.currentRow()
        return rows[index] if 0 <= index < len(rows) else None

    @staticmethod
    def _active_document():
        import FreeCAD  # type: ignore

        doc = FreeCAD.ActiveDocument
        if doc is None:
            doc = FreeCAD.newDocument("MadakeCAD")
        return doc

    @staticmethod
    def _tag(obj, entity_id: str):
        """Store the MadakeCAD entity id on a FreeCAD object (custom property ``madake_id``)."""
        if MADAKE_ID_PROPERTY not in obj.PropertiesList:
            obj.addProperty("App::PropertyString", MADAKE_ID_PROPERTY, "MadakeCAD", "MadakeCAD entity id")
        setattr(obj, MADAKE_ID_PROPERTY, entity_id)

    def _send(self, commands: list) -> bool:
        """Post commands as **one** edit (one undo step in MadakeCAD); False on failure."""
        if not commands:
            return True
        try:
            self._client.post_commands(commands, batch=True)
        except LinkError as e:
            self._status.setText(str(e))
            return False
        return True

    def insert_model(self):
        """Insert the selected part's 3D model into the active document, tag it and register the link."""
        row = self._selected_row(self._parts, self._part_rows)
        if row is None:
            self._status.setText("Select a part first")
            return
        kind = model_kind(row.model_3d)
        if not kind:
            self._status.setText(f"{row.reference}: no insertable 3D model ({row.model_3d or 'no model_3d in the parts database'})")
            return
        doc = self._active_document()
        if kind == "shape":
            import Part  # type: ignore

            shape = Part.read(row.model_3d)
            obj = doc.addObject("Part::Feature", row.reference or "Part")
            obj.Shape = shape
            obj.Label = f"{row.reference} {row.part_no}".strip()
        else:
            before = set(o.Name for o in doc.Objects)
            doc.mergeProject(row.model_3d)
            added = [o for o in doc.Objects if o.Name not in before]
            if not added:
                self._status.setText(f"{row.reference}: nothing was imported from {row.model_3d}")
                return
            obj = added[0]
        self._tag(obj, row.entity_id)
        doc.recompute()
        command = set_mech_link_command(
            row.entity_id,
            doc.FileName or "",
            obj.Name,
            placement=placement_of(self._snapshot or {}, row.entity_id),
        )
        if self._send([command]):
            self._status.setText(f"Inserted {obj.Label} and linked it to {row.reference}")

    def link_selected(self, table, rows):
        """Link the FreeCAD object selected in the 3D view to the selected table row."""
        row = self._selected_row(table, rows)
        selection = FreeCADGui.Selection.getSelection()
        if row is None or not selection:
            self._status.setText("Select a row and one FreeCAD object")
            return
        obj = selection[0]
        self._tag(obj, row.entity_id)
        doc = obj.Document
        command = set_mech_link_command(
            row.entity_id,
            doc.FileName or "",
            obj.Name,
            placement=placement_of(self._snapshot or {}, row.entity_id),
        )
        if self._send([command]):
            self._status.setText(f"Linked {obj.Label} to {getattr(row, 'reference', None) or row.entity_id}")

    def unlink_selected(self, table, rows):
        row = self._selected_row(table, rows)
        if row is None or not row.linked_object:
            self._status.setText("Select a linked row")
            return
        if self._send([remove_mech_link_command(row.entity_id)]):
            self._status.setText(f"Unlinked {row.linked_object}")

    def write_back_lengths(self):
        """Measure every route object tagged with a wire's madake_id and write the lengths to MadakeCAD."""
        if self._snapshot is None:
            self._status.setText("Connect first")
            return
        doc = self._active_document()
        objects = []
        for obj in doc.Objects:
            madake_id = getattr(obj, MADAKE_ID_PROPERTY, None)
            shape = getattr(obj, "Shape", None)
            if madake_id and shape is not None and not shape.isNull():
                objects.append((obj.Name, madake_id, shape.Length))
        measurements = measurements_from_objects(objects, self._snapshot)
        if not measurements:
            self._status.setText("No route objects linked to wires (tag Draft Wires with a wire's madake_id via 'Link selected route')")
            return
        answer = QtWidgets.QMessageBox.question(
            self,
            "Write back wire lengths",
            f"Write {len(measurements)} measured length(s) to MadakeCAD? Hand-entered lengths on those wires will be replaced (undo is possible in MadakeCAD).",
        )
        if answer != QtWidgets.QMessageBox.Yes:
            return
        commands = link_commands_for_routes(measurements, doc.FileName or "", self._snapshot) + wire_length_commands(measurements)
        if self._send(commands):
            self.refresh()  # 先に読み直す (refreshはステータス行を概要で上書きする)
            self._status.setText(f"Wrote {len(measurements)} wire length(s) back to MadakeCAD (one undo step)")

    # ---- M5-3: route stubs / net highlight / placements -------------------

    def _document_placements(self, doc) -> dict:
        """object name → (x, y, z) mm for every object with a Placement."""
        out = {}
        for obj in doc.Objects:
            placement = getattr(obj, "Placement", None)
            if placement is not None:
                base = placement.Base
                out[obj.Name] = (base.x, base.y, base.z)
        return out

    def create_route_stubs(self):
        """Create a straight route line for every wire joining two linked parts, register each as the wire's link, and report the ones that could not be drawn."""
        if self._snapshot is None:
            self._status.setText("Connect first")
            return
        import FreeCAD  # type: ignore
        import Part  # type: ignore

        doc = self._active_document()
        existing = {getattr(o, MADAKE_ID_PROPERTY) for o in doc.Objects if getattr(o, MADAKE_ID_PROPERTY, None)}
        try:
            nets = self._client.netlist(self.current_sheet_id())
        except LinkError as e:
            self._status.setText(str(e))
            return
        stubs, degenerate = split_route_stubs(route_stubs(self._snapshot, nets, self._document_placements(doc), existing))
        if not stubs and not degenerate:
            self._status.setText("No new route stubs: every wire between linked parts already has a route, or the parts are not linked/placed")
            return
        # 退化区間 (両端が同じ位置 = 挿入直後の既定配置) は線にできないので作らず報告する。
        notes = [
            f"{s.from_object} and {s.to_object} are at the same position — move one of them, then try again"
            for s in degenerate
        ]
        created, commands = [], []
        for stub in stubs:
            # 1本の失敗でバッチ全体を落とさない (残りのスタブは作る)。
            try:
                line = Part.makePolygon([FreeCAD.Vector(*stub.start), FreeCAD.Vector(*stub.end)])
                obj = doc.addObject("Part::Feature", stub.name)
                obj.Shape = line
                obj.Label = f"{stub.name} ({stub.from_object} → {stub.to_object})"
                self._tag(obj, stub.wire_id)
            except Exception as e:  # noqa: BLE001 - OCCの例外もステータス行へ
                notes.append(f"{stub.name}: could not be created ({type(e).__name__}: {e})")
                continue
            created.append(obj)
            # ネットハイライトとWiresタブの「Route object」列は対応付けを見るので、
            # 作ったスタブをその場で配線の対応付けとして登録する (配置は保つ)。
            commands.append(
                set_mech_link_command(
                    stub.wire_id,
                    doc.FileName or "",
                    obj.Name,
                    placement=placement_of(self._snapshot, stub.wire_id),
                )
            )
        doc.recompute()
        message = f"Created {len(created)} route stub(s)"
        if created:
            if self._send(commands):
                message += " and linked them to their wires; edit them into real routes, then 'Measure routes → write back lengths'"
                self.refresh()
            else:
                message += f", but registering them in MadakeCAD failed: {self._status.text()}"
        if notes:
            message += f" · {len(notes)} skipped: " + "; ".join(notes)
        self._status.setText(message)

    def highlight_net(self):
        """Select the linked parts and routes of the selected net in the 3D view."""
        index = self._table.currentRow()
        if self._snapshot is None or not (0 <= index < len(self._nets)):
            return
        names = net_highlight_objects(self._nets[index], self._snapshot)
        doc = FreeCADGui.ActiveDocument.Document if FreeCADGui.ActiveDocument else None
        if doc is None:
            return
        FreeCADGui.Selection.clearSelection()
        for name in names:
            if doc.getObject(name) is not None:
                FreeCADGui.Selection.addSelection(doc, name)

    def sync_placements(self):
        """Write the placement of every linked object in the document to MadakeCAD as one edit."""
        if self._snapshot is None:
            self._status.setText("Connect first")
            return
        doc = self._active_document()
        placements = {}
        tilted = []
        for obj in doc.Objects:
            placement = getattr(obj, "Placement", None)
            if placement is None:
                continue
            base = placement.Base
            axis = placement.Rotation.Axis
            angle = rotation_deg_about_z((axis.x, axis.y, axis.z), math.degrees(placement.Rotation.Angle))
            if angle is None:
                # 盤レイアウトは平面なのでZ回転しか意味がない。傾いた部品は0°で記録して報告する。
                tilted.append(obj.Name)
                angle = 0.0
            placements[obj.Name] = ((base.x, base.y, base.z), angle)
        commands = placement_commands(self._snapshot, placements)
        warning = f" · {len(tilted)} object(s) are rotated about an axis other than Z; stored as 0°: " + ", ".join(tilted) if tilted else ""
        if not commands:
            self._status.setText("Placements are already in sync (or no linked object is in this document)" + warning)
            return
        if self._send(commands):
            self.refresh()  # 先に読み直す (refreshはステータス行を概要で上書きする)
            self._status.setText(f"Synced {len(commands)} placement(s) to MadakeCAD (one undo step)" + warning)

    # ---- live follow -----------------------------------------------------

    def _on_follow_toggled(self, on: bool):
        if on:
            self._start_follow()
        else:
            self._stop_follow()

    def _start_follow(self):
        if self._thread is not None:
            return
        self._thread = EventThread(self._client, self)
        self._thread.patch.connect(self._on_patch)
        self._thread.ended.connect(self._on_follow_ended)
        self._thread.start()

    def _stop_follow(self):
        """Stop live follow and wait for the thread, so reconnecting cannot leave two streams open."""
        thread = self._thread
        if thread is None:
            return
        self._thread = None
        for signal, slot in ((thread.patch, self._on_patch), (thread.ended, self._on_follow_ended)):
            try:
                signal.disconnect(slot)
            except (RuntimeError, TypeError):  # 既に切れている
                pass
        thread.requestInterruption()
        thread.wait(FOLLOW_STOP_TIMEOUT_MS)

    def _on_patch(self, patch: dict):
        if not self._follower.accept(patch):
            return
        if refresh_needed(patch, self.current_sheet_id()):
            self.refresh()
        elif self._summary is not None:
            self._summary.revision = self._follower.revision
            self._status.setText(summary_text(self._summary))

    def _on_follow_ended(self, error: str):
        self._thread = None
        if error:
            self._status.setText(error)
        elif self._follow.isChecked():
            self._status.setText(self._status.text() + " · live follow ended (MadakeCAD closed?)")

    def closeEvent(self, event):
        self._stop_follow()
        super().closeEvent(event)
