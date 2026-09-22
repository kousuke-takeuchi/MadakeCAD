// プロジェクト情報ダイアログの仕様テスト (G16)。
import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { ipc, type Command, type Project } from "../ipc";
import { useDocumentStore } from "./document";
import { useProjectInfoStore } from "./projectInfo";

function project(): Project {
  return {
    format_version: 10,
    name: "旧プロジェクト",
    no: "OLD-1",
    sheets: [
      {
        id: "s1",
        name: "Sheet1",
        size: "A3",
        orientation: "Landscape",
        zone_cols: 4,
        zone_rows: 6,
        title_block: { company: "旧会社", drawing_no: "OLD-DWG", title: "旧表題" },
        revisions: [],
        entities: {},
      },
    ],
    wire_parts: [],
    plc_assignments: [],
  };
}

function spyCommands(): Command[] {
  const sent: Command[] = [];
  vi.spyOn(ipc, "executeCommand").mockImplementation(async (command) => {
    sent.push(command);
    return { revision: 1, ops: [] };
  });
  return sent;
}

describe("project info dialog store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.restoreAllMocks();
  });

  // ja: 開くと、プロジェクト名・番号と現在のシートの表題欄が下書きへ複製される
  it("opens with the project's name/number and the active sheet's title block", () => {
    const doc = useDocumentStore();
    doc.project = project();
    doc.activeSheetId = "s1";
    const info = useProjectInfoStore();
    info.openDialog();
    expect(info.draft).toEqual({
      name: "旧プロジェクト",
      no: "OLD-1",
      company: "旧会社",
      drawingNo: "OLD-DWG",
      title: "旧表題",
    });
    expect(info.dirty).toBe(false);
  });

  // ja: 何も変えずに保存してもコマンドは送られない
  it("sends no command when nothing changed", async () => {
    const doc = useDocumentStore();
    doc.project = project();
    doc.activeSheetId = "s1";
    const sent = spyCommands();
    const info = useProjectInfoStore();
    info.openDialog();
    const result = await info.save();
    expect(result).toBe(false);
    expect(sent).toHaveLength(0);
    expect(info.open).toBe(false);
  });

  // ja: 変えた欄だけがset_project_metaコマンドに載り、他の欄はundefinedのまま(触っていないシートの値を壊さない)
  it("sends only the changed fields, leaving the rest undefined", async () => {
    const doc = useDocumentStore();
    doc.project = project();
    doc.activeSheetId = "s1";
    const sent = spyCommands();
    const info = useProjectInfoStore();
    info.openDialog();
    info.draft.no = "MDK-2026-001";
    const result = await info.save();
    expect(result).toBe(true);
    expect(sent).toEqual([
      {
        type: "set_project_meta",
        name: undefined,
        no: "MDK-2026-001",
        company: undefined,
        drawing_no: undefined,
        title: undefined,
      },
    ]);
  });

  // ja: 名前・会社名・図番・表題をすべて変えれば、すべての欄がコマンドに載る
  it("sends every field when every field is changed", async () => {
    const doc = useDocumentStore();
    doc.project = project();
    doc.activeSheetId = "s1";
    const sent = spyCommands();
    const info = useProjectInfoStore();
    info.openDialog();
    info.draft.name = "まだけ制御盤";
    info.draft.no = "MDK-2026-001";
    info.draft.company = "まだけ電機";
    info.draft.drawingNo = "MDK-100";
    info.draft.title = "主回路";
    await info.save();
    expect(sent).toEqual([
      {
        type: "set_project_meta",
        name: "まだけ制御盤",
        no: "MDK-2026-001",
        company: "まだけ電機",
        drawing_no: "MDK-100",
        title: "主回路",
      },
    ]);
  });

  // ja: プロジェクト番号を空にして保存すると、空文字ではなくnullを送って番号を未設定へ戻す
  it("clears the project number with null when the field is emptied", async () => {
    const doc = useDocumentStore();
    const p = project();
    p.no = "MDK-2026-001";
    doc.project = p;
    doc.activeSheetId = "s1";
    const sent = spyCommands();
    const info = useProjectInfoStore();
    info.openDialog();
    info.draft.no = "";
    await info.save();
    expect(sent[0]).toMatchObject({ type: "set_project_meta", no: null });
  });

  // ja: キャンセルすると下書きが捨てられ、コマンドは送られない
  it("discards the draft and sends nothing on cancel", () => {
    const doc = useDocumentStore();
    doc.project = project();
    doc.activeSheetId = "s1";
    const sent = spyCommands();
    const info = useProjectInfoStore();
    info.openDialog();
    info.draft.name = "変更";
    info.cancel();
    expect(info.open).toBe(false);
    expect(sent).toHaveLength(0);
  });
});
