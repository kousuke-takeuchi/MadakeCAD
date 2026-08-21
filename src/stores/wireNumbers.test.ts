// 線番自動採番ダイアログの仕様テスト (docs/internal/specs/m2-drawing-parity.md §2)。
import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { ipc, type Command, type Patch, type Sheet } from "../ipc";
import { useWireNumbersStore } from "./wireNumbers";

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
  } as unknown as Sheet;
}

/** ダイアログが送ったコマンドを記録する。patchは採番後のワイヤをそのまま返す。 */
function spyCommands(patch: Patch = { revision: 1, ops: [] }): Command[] {
  const sent: Command[] = [];
  vi.spyOn(ipc, "executeCommand").mockImplementation(async (command) => {
    sent.push(command);
    return patch;
  });
  return sent;
}

/** 採番結果のpatch (線番付きワイヤのupsert)。 */
function numberedPatch(...nets: string[]): Patch {
  return {
    revision: 2,
    ops: nets.map((net, i) => ({
      op: "entity_upserted",
      sheet_id: "s1",
      entity: {
        kind: "wire",
        id: `w${i}`,
        points: [],
        color: "black",
        sq: 0.3,
        length_m: null,
        part_no: null,
        net,
      },
    })) as Patch["ops"],
  };
}

describe("wire numbering dialog store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.restoreAllMocks();
  });

  // ja: 既定は「連番・開始1・現在のシート・既存の線番は保持」で開く
  it("opens with sequential numbering from 1 over the current sheet, keeping existing numbers", () => {
    const store = useWireNumbersStore();
    store.openFor(sheet());
    expect(store.open).toBe(true);
    expect(store.method).toBe("sequential");
    expect(store.start).toBe("1");
    expect(store.scope).toBe("sheet");
    expect(store.existing).toBe("keep");
  });

  // ja: 「現在のシート」+「保持する」は、そのシートだけを追い番で採番するコマンドになる
  it("numbers only the current sheet in append mode when keeping existing numbers", () => {
    const store = useWireNumbersStore();
    store.openFor(sheet());
    expect(store.command).toEqual({
      type: "renumber_wires",
      sheet_id: "s1",
      mode: "append",
      start: 1,
    });
  });

  // ja: 「現在のシート」+「すべて振り直す」は、そのシートを振り直すコマンドになる
  it("renumbers only the current sheet when all numbers are reassigned", () => {
    const store = useWireNumbersStore();
    store.openFor(sheet());
    store.existing = "renumber";
    expect(store.command).toEqual({
      type: "renumber_wires",
      sheet_id: "s1",
      mode: "renumber",
      start: 1,
    });
  });

  // ja: 「プロジェクト全体」はシート指定を省いたコマンドになり、図面全体で一意の線番が振られる
  it("omits the sheet id for the whole project so numbers stay unique across sheets", () => {
    const store = useWireNumbersStore();
    store.openFor(sheet());
    store.scope = "project";
    store.start = "100";
    expect(store.command).toEqual({
      type: "renumber_wires",
      sheet_id: null,
      mode: "append",
      start: 100,
    });
    store.existing = "renumber";
    expect(store.command).toEqual({
      type: "renumber_wires",
      sheet_id: null,
      mode: "renumber",
      start: 100,
    });
  });

  // ja: 開始番号は1以上の整数だけを受け付ける(0・負数・小数・文字は入力エラー)
  it("accepts only whole numbers of 1 or more as the start number", () => {
    const store = useWireNumbersStore();
    store.openFor(sheet());
    for (const ok of ["1", "2", "100", " 7 "]) {
      store.start = ok;
      expect(store.startValid, ok).toBe(true);
    }
    for (const bad of ["0", "-3", "1.5", "abc", "", "  "]) {
      store.start = bad;
      expect(store.startValid, bad).toBe(false);
      expect(store.command, bad).toBeNull();
    }
  });

  // ja: 参照ベースの「ゾーン基準」はまだ選べない(M4予定)
  it("does not offer zone-based numbering yet", () => {
    const store = useWireNumbersStore();
    store.openFor(sheet());
    expect(store.zoneAvailable).toBe(false);
  });

  // ja: 採番実行はコマンドを1回だけ送り、採番したネット数を返して閉じる
  it("runs one command, reports how many nets were numbered and closes", async () => {
    const sent = spyCommands(numberedPatch("1", "1", "2"));
    const store = useWireNumbersStore();
    store.openFor(sheet());
    const count = await store.run();
    expect(sent).toEqual([{ type: "renumber_wires", sheet_id: "s1", mode: "append", start: 1 }]);
    expect(count).toBe(2);
    expect(store.open).toBe(false);
  });

  // ja: 開始番号が不正なままでは採番せず、ダイアログは開いたまま残る
  it("refuses to run while the start number is invalid", async () => {
    const sent = spyCommands();
    const store = useWireNumbersStore();
    store.openFor(sheet());
    store.start = "0";
    expect(await store.run()).toBeNull();
    expect(sent).toEqual([]);
    expect(store.open).toBe(true);
  });

  // ja: キャンセルするとコマンドは送られずに閉じる
  it("sends nothing when the dialog is cancelled", () => {
    const sent = spyCommands();
    const store = useWireNumbersStore();
    store.openFor(sheet());
    store.cancel();
    expect(sent).toEqual([]);
    expect(store.open).toBe(false);
  });

  // ja: 採番に失敗したらダイアログは開いたままエラーを表示する
  it("keeps the dialog open and reports the error when numbering fails", async () => {
    vi.spyOn(ipc, "executeCommand").mockRejectedValue(new Error("boom"));
    const store = useWireNumbersStore();
    store.openFor(sheet());
    expect(await store.run()).toBeNull();
    expect(store.open).toBe(true);
    expect(store.error).toContain("boom");
  });
});
