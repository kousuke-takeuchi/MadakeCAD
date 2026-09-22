// 用紙変更ダイアログの仕様テスト (G15)。
import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { ipc, type Command, type Sheet } from "../ipc";
import { useSheetPaperStore } from "./sheetPaper";

function sheet(): Sheet {
  return {
    id: "s1",
    name: "Sheet1",
    size: "A3",
    orientation: "Landscape",
    zone_cols: 4,
    zone_rows: 6,
    title_block: {},
    revisions: [],
    entities: {},
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

describe("sheet paper dialog store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.restoreAllMocks();
  });

  // ja: 開くと、シートの現在の用紙サイズ・向きが下書きへ複製される
  it("opens with the sheet's current size and orientation", () => {
    const paper = useSheetPaperStore();
    paper.openFor(sheet());
    expect(paper.size).toBe("A3");
    expect(paper.orientation).toBe("Landscape");
    expect(paper.dirty).toBe(false);
  });

  // ja: 何も変えずに保存してもコマンドは送られない
  it("sends no command when nothing changed", async () => {
    const sent = spyCommands();
    const paper = useSheetPaperStore();
    paper.openFor(sheet());
    const result = await paper.save();
    expect(result).toBe(false);
    expect(sent).toHaveLength(0);
  });

  // ja: サイズ・向きを変えて保存すると、set_sheet_paperコマンドが1回送られる
  it("sends one set_sheet_paper command with the new size and orientation", async () => {
    const sent = spyCommands();
    const paper = useSheetPaperStore();
    paper.openFor(sheet());
    paper.size = "A4";
    paper.orientation = "Portrait";
    const result = await paper.save();
    expect(result).toBe(true);
    expect(sent).toEqual([
      { type: "set_sheet_paper", sheet_id: "s1", size: "A4", orientation: "Portrait" },
    ]);
    expect(paper.open).toBe(false);
  });

  // ja: キャンセルすると下書きが捨てられ、コマンドは送られない
  it("discards the draft and sends nothing on cancel", () => {
    const sent = spyCommands();
    const paper = useSheetPaperStore();
    paper.openFor(sheet());
    paper.size = "A0";
    paper.cancel();
    expect(paper.open).toBe(false);
    expect(sent).toHaveLength(0);
  });
});
