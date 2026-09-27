// ファイルメニュー(新規/開く/保存/名前を付けて保存/最近使ったファイル)の仕様テスト。
import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));
import { ipc, type Patch, type Project } from "../ipc";
import { useDocumentStore } from "../stores/document";
import { useProjectFileStore, type RecentStorage } from "../stores/projectFile";
import { useUiStore } from "../stores/ui";
import { useFileActions, type FileDialogs } from "./fileActions";

function project(name = "panel"): Project {
  return { format_version: 2, name, wire_parts: [], plc_assignments: [], sheets: [] };
}

function replaced(revision: number, name = "panel"): Patch {
  return { revision, ops: [{ op: "project_replaced", project: project(name) }] };
}

function edit(revision: number) {
  useDocumentStore().applyPatch({ revision, ops: [{ op: "wire_parts_replaced", wire_parts: [] }] });
}

const memory: RecentStorage = { get: () => [], set() {} };

/** ダイアログの応答を決め打ちにし、呼ばれたかを記録する。 */
function fakeDialogs(answers: { open?: string | null; save?: string | null; discard?: boolean }) {
  const dialogs: FileDialogs = {
    pickOpen: vi.fn(async () => answers.open ?? null),
    pickSave: vi.fn(async () => answers.save ?? null),
    confirmDiscard: vi.fn(() => answers.discard ?? true),
  };
  return dialogs;
}

describe("file menu: 保存", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    setActivePinia(createPinia());
    useDocumentStore().project = project();
    useDocumentStore().revision = 1;
    useProjectFileStore().attach(memory);
    vi.spyOn(ipc, "saveProject").mockResolvedValue();
  });

  // ja: 保存先が決まっていれば「保存」はダイアログを出さずに上書きする
  it("save overwrites the current file without showing a dialog", async () => {
    const file = useProjectFileStore();
    file.path = "/work/panel.mdkproj";
    edit(2);
    const dialogs = fakeDialogs({});
    expect(await useFileActions(dialogs).saveProject()).toBe(true);
    expect(dialogs.pickSave).not.toHaveBeenCalled();
    expect(ipc.saveProject).toHaveBeenCalledWith("/work/panel.mdkproj");
    expect(file.dirty).toBe(false);
    expect(useUiStore().lastMessage).toContain("/work/panel.mdkproj");
  });

  // ja: 保存先が未定なら「保存」は保存ダイアログを出す (名前を付けて保存と同じ)
  it("save asks for a path when the project has none yet", async () => {
    const dialogs = fakeDialogs({ save: "/work/new.mdkproj" });
    expect(await useFileActions(dialogs).saveProject()).toBe(true);
    expect(dialogs.pickSave).toHaveBeenCalledWith("panel.mdkproj", [
      { name: "MadakeCAD project", extensions: ["mdkproj"] },
    ]);
    expect(ipc.saveProject).toHaveBeenCalledWith("/work/new.mdkproj");
    expect(useProjectFileStore().path).toBe("/work/new.mdkproj");
  });

  // ja: 保存ダイアログをキャンセルすると何も書かない
  it("cancelling the save dialog writes nothing", async () => {
    const dialogs = fakeDialogs({ save: null });
    expect(await useFileActions(dialogs).saveProjectAs()).toBe(false);
    expect(ipc.saveProject).not.toHaveBeenCalled();
  });

  // ja: 「名前を付けて保存」は開いているファイルのパスを既定にする
  it("save-as defaults to the currently open file's path", async () => {
    useProjectFileStore().path = "/work/panel.mdkproj";
    const dialogs = fakeDialogs({ save: "/work/copy.mdkproj" });
    await useFileActions(dialogs).saveProjectAs();
    expect(dialogs.pickSave).toHaveBeenCalledWith("/work/panel.mdkproj", expect.anything());
    expect(useProjectFileStore().path).toBe("/work/copy.mdkproj");
  });

  // ja: 保存に失敗したら理由をログに出し、保存先は変えない
  it("a failed save is logged and the save target stays unchanged", async () => {
    vi.spyOn(ipc, "saveProject").mockRejectedValue(new Error("disk full"));
    const dialogs = fakeDialogs({ save: "/work/new.mdkproj" });
    expect(await useFileActions(dialogs).saveProjectAs()).toBe(false);
    expect(useProjectFileStore().path).toBeNull();
    expect(useUiStore().lastMessage).toContain("disk full");
  });
});

describe("file menu: 新規と開く", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    setActivePinia(createPinia());
    useDocumentStore().project = project();
    useDocumentStore().revision = 1;
    useProjectFileStore().attach(memory);
    vi.spyOn(ipc, "newProject").mockResolvedValue(replaced(10, "Untitled"));
    vi.spyOn(ipc, "loadProject").mockResolvedValue(replaced(10, "loaded"));
  });

  // ja: 「新規」は無題のプロジェクトを作り、保存先は空になる
  it("new creates an untitled project with no save target", async () => {
    useProjectFileStore().path = "/work/panel.mdkproj";
    expect(await useFileActions(fakeDialogs({})).newProject()).toBe(true);
    expect(ipc.newProject).toHaveBeenCalledWith("Untitled");
    expect(useProjectFileStore().path).toBeNull();
    expect(useDocumentStore().project?.name).toBe("Untitled");
  });

  // ja: 未保存の編集があるときの「新規」は確認し、キャンセルすると図面はそのまま
  it("new asks before discarding unsaved changes, and cancel keeps the drawing", async () => {
    edit(2);
    const dialogs = fakeDialogs({ discard: false });
    expect(await useFileActions(dialogs).newProject()).toBe(false);
    expect(dialogs.confirmDiscard).toHaveBeenCalled();
    expect(ipc.newProject).not.toHaveBeenCalled();
    expect(useDocumentStore().project?.name).toBe("panel");
  });

  // ja: 新規に失敗したら理由をログに出し、図面はそのまま
  it("a failed new-project is logged and the drawing stays", async () => {
    vi.spyOn(ipc, "newProject").mockRejectedValue(new Error("not supported"));
    expect(await useFileActions(fakeDialogs({})).newProject()).toBe(false);
    expect(useUiStore().lastMessage).toContain("not supported");
    expect(useDocumentStore().project?.name).toBe("panel");
  });

  // ja: 未保存の編集が無ければ確認なしで新規・開くへ進む
  it("new and open skip the confirmation when nothing is unsaved", async () => {
    const dialogs = fakeDialogs({ open: "/work/other.mdkproj" });
    await useFileActions(dialogs).newProject();
    await useFileActions(dialogs).openProject();
    expect(dialogs.confirmDiscard).not.toHaveBeenCalled();
  });

  // ja: 「開く」はダイアログで選んだ.mdkprojを読み込み、そのパスが保存先になる
  it("open loads the chosen .mdkproj and makes it the save target", async () => {
    const dialogs = fakeDialogs({ open: "/work/other.mdkproj" });
    expect(await useFileActions(dialogs).openProject()).toBe(true);
    expect(dialogs.pickOpen).toHaveBeenCalledWith([
      { name: "MadakeCAD project", extensions: ["mdkproj"] },
      { name: "KiCad schematic", extensions: ["kicad_sch"] },
    ]);
    expect(ipc.loadProject).toHaveBeenCalledWith("/work/other.mdkproj");
    expect(useProjectFileStore().path).toBe("/work/other.mdkproj");
    expect(useProjectFileStore().recent[0]).toBe("/work/other.mdkproj");
  });

  // ja: 開くダイアログをキャンセルすると何も読み込まない
  it("cancelling the open dialog loads nothing", async () => {
    expect(await useFileActions(fakeDialogs({ open: null })).openProject()).toBe(false);
    expect(ipc.loadProject).not.toHaveBeenCalled();
  });

  // ja: 未保存の編集があるときの「開く」は確認し、キャンセルするとダイアログも出ない
  it("open asks before discarding unsaved changes; cancel shows no file dialog", async () => {
    edit(2);
    const dialogs = fakeDialogs({ open: "/work/other.mdkproj", discard: false });
    expect(await useFileActions(dialogs).openProject()).toBe(false);
    expect(dialogs.pickOpen).not.toHaveBeenCalled();
  });

  // ja: .kicad_schを選ぶとKiCad回路図として読み込み、件数をログに出す
  it("choosing a .kicad_sch imports it as a KiCad schematic and logs the counts", async () => {
    vi.spyOn(ipc, "importKicad").mockResolvedValue({
      patch: replaced(10, "board"),
      report: { symbols: 4, wires: 6, junctions: 1, labels: 2, texts: 0, skipped: ["x", "y"], warnings: [] },
    });
    const dialogs = fakeDialogs({ open: "/work/board.kicad_sch" });
    expect(await useFileActions(dialogs).openProject()).toBe(true);
    expect(ipc.loadProject).not.toHaveBeenCalled();
    expect(useProjectFileStore().path).toBeNull();
    const log = useUiStore().lastMessage;
    expect(log).toContain("4 symbols");
    expect(log).toContain("2 kinds skipped");
  });

  // ja: 最近使ったファイルはダイアログ無しで開く
  it("a recent file opens without a dialog", async () => {
    const dialogs = fakeDialogs({});
    expect(await useFileActions(dialogs).openRecent("/work/recent.mdkproj")).toBe(true);
    expect(dialogs.pickOpen).not.toHaveBeenCalled();
    expect(ipc.loadProject).toHaveBeenCalledWith("/work/recent.mdkproj");
  });

  // ja: 開けなかった最近使ったファイルは一覧から外し、理由をログに出す
  it("a recent file that fails to open is dropped from the list with the reason logged", async () => {
    vi.spyOn(ipc, "loadProject").mockRejectedValue(new Error("no such file"));
    const file = useProjectFileStore();
    file.addRecent("/work/gone.mdkproj");
    expect(await useFileActions(fakeDialogs({})).openRecent("/work/gone.mdkproj")).toBe(false);
    expect(file.recent).not.toContain("/work/gone.mdkproj");
    expect(useUiStore().lastMessage).toContain("no such file");
    expect(useDocumentStore().project?.name).toBe("panel");
  });
});
