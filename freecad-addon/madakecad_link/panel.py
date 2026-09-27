"""Qt dock panel: connection settings, project overview, netlist view, live follow.

Only importable inside FreeCAD (needs FreeCADGui and PySide). Everything that can be
tested without Qt is delegated to client.py / model.py / events.py / settings.py.
"""

from __future__ import annotations

import FreeCADGui  # type: ignore
from PySide import QtCore, QtWidgets  # type: ignore

from .client import LinkClient, LinkError
from .events import PatchFollower, refresh_needed
from .model import NETLIST_COLUMNS, netlist_cells, netlist_rows, summarize_project, summary_text
from .settings import load_settings


class EventThread(QtCore.QThread):
    """Reads the SSE stream in the background and re-emits each patch on the GUI thread."""

    patch = QtCore.Signal(dict)
    ended = QtCore.Signal(str)

    def __init__(self, client: LinkClient, parent=None):
        super().__init__(parent)
        self._client = client

    def run(self):
        try:
            for event, payload in self._client.events():
                if self.isInterruptionRequested():
                    break
                if event == "patch" and isinstance(payload, dict):
                    self.patch.emit(payload)
            self.ended.emit("")
        except LinkError as e:
            self.ended.emit(str(e))


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

        self._table = QtWidgets.QTableWidget(0, len(NETLIST_COLUMNS))
        self._table.setHorizontalHeaderLabels(list(NETLIST_COLUMNS))
        self._table.horizontalHeader().setStretchLastSection(False)
        self._table.horizontalHeader().setSectionResizeMode(3, QtWidgets.QHeaderView.Stretch)
        self._table.setEditTriggers(QtWidgets.QAbstractItemView.NoEditTriggers)
        self._table.setSelectionBehavior(QtWidgets.QAbstractItemView.SelectRows)
        layout.addWidget(self._table, 1)

        self.setWidget(body)

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
        self._summary = summarize_project(snapshot)
        self._follower.accept({"revision": self._summary.revision})
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
        rows = netlist_rows(nets)
        self._table.setRowCount(len(rows))
        for r, row in enumerate(rows):
            for c, cell in enumerate(netlist_cells(row)):
                self._table.setItem(r, c, QtWidgets.QTableWidgetItem(cell))

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
        if self._thread is None:
            return
        self._thread.requestInterruption()
        self._thread = None

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
