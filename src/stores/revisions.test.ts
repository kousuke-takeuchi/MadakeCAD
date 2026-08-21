// 改訂欄編集ダイアログの仕様テスト (docs/internal/specs/m2-drawing-parity.md §1)。
import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { ipc, type Command, type Revision, type Sheet } from "../ipc";
import { nextRevisionMark, todayIso, useRevisionsStore } from "./revisions";

function sheet(revisions: Revision[] = []): Sheet {
  return {
    id: "s1",
    name: "Sheet1",
    size: "A3",
    orientation: "Landscape",
    zone_cols: 4,
    zone_rows: 6,
    title_block: {},
    revisions,
    entities: {},
  };
}

const rev = (mark: string, description = "", by = "", date = "2026-01-01"): Revision => ({
  mark,
  date,
  description,
  by,
});

/** ダイアログが送ったコマンドを記録する。 */
function spyCommands(): Command[] {
  const sent: Command[] = [];
  vi.spyOn(ipc, "executeCommand").mockImplementation(async (command) => {
    sent.push(command);
    return { revision: 1, ops: [] };
  });
  return sent;
}

describe("revisions dialog store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.restoreAllMocks();
  });

  // ja: 改訂が1件も無いシートで行を追加すると、記号Aと今日の日付が入る
  it("adds the first row with mark A and today's date", () => {
    const store = useRevisionsStore();
    store.openFor(sheet());
    store.addRow();
    expect(store.draft).toEqual([{ mark: "A", date: todayIso(), description: "", by: "" }]);
  });

  // ja: 既に改訂がある場合、追加行の記号は既存の最大記号の次のアルファベットになる
  it("numbers a new row with the alphabet letter after the highest existing mark", () => {
    const store = useRevisionsStore();
    store.openFor(sheet([rev("A"), rev("B")]));
    store.addRow();
    expect(store.draft[2].mark).toBe("C");
  });

  // ja: 記号がZまで進んだらAAへ繰り上がる
  it("carries the mark from Z to AA", () => {
    expect(nextRevisionMark([])).toBe("A");
    expect(nextRevisionMark([rev("Z")])).toBe("AA");
    expect(nextRevisionMark([rev("AA"), rev("B")])).toBe("AB");
  });

  // ja: 記号が空欄や数字だけの行があっても採番は壊れず、アルファベットの続きが入る
  it("ignores blank or non-alphabetic marks when picking the next one", () => {
    expect(nextRevisionMark([rev(""), rev("1"), rev("C")])).toBe("D");
  });

  // ja: 今日の日付はYYYY-MM-DD形式で入る
  it("formats today's date as YYYY-MM-DD", () => {
    expect(todayIso(new Date(2026, 7, 22))).toBe("2026-08-22");
    expect(todayIso()).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  // ja: 表は最新の改訂が一番上に並ぶ(図枠の改訂欄と同じ並び)
  it("lists the newest revision at the top of the table", () => {
    const store = useRevisionsStore();
    store.openFor(sheet([rev("A"), rev("B")]));
    expect(store.rows.map((r) => r.rev.mark)).toEqual(["B", "A"]);
    expect(store.rows.map((r) => r.index)).toEqual([1, 0]);
  });

  // ja: ダイアログを開いてもシートの改訂は書き換わらない(編集は下書きの上だけ)
  it("edits a private draft and never touches the sheet itself", () => {
    const target = sheet([rev("A", "初版")]);
    const store = useRevisionsStore();
    store.openFor(target);
    store.updateRow(0, { description: "書き換え" });
    store.addRow();
    expect(target.revisions).toEqual([rev("A", "初版")]);
  });

  // ja: 保存すると編集後の一覧を積んだset_revisionsコマンドが1回だけ送られる
  it("saves the edited list with a single set_revisions command", async () => {
    const sent = spyCommands();
    const store = useRevisionsStore();
    store.openFor(sheet([rev("A", "初版", "竹内")]));
    store.addRow();
    store.updateRow(1, { description: "端子台TB2を8極へ変更", by: "竹内" });
    await store.save();
    expect(sent).toEqual([
      {
        type: "set_revisions",
        sheet_id: "s1",
        revisions: [
          rev("A", "初版", "竹内"),
          { mark: "B", date: todayIso(), description: "端子台TB2を8極へ変更", by: "竹内" },
        ],
      },
    ]);
    expect(store.open).toBe(false);
  });

  // ja: キャンセルするとコマンドは送られず、編集内容は捨てられる
  it("sends nothing when the dialog is cancelled", async () => {
    const sent = spyCommands();
    const store = useRevisionsStore();
    store.openFor(sheet([rev("A", "初版")]));
    store.addRow();
    store.cancel();
    expect(sent).toEqual([]);
    expect(store.open).toBe(false);
    expect(store.draft).toEqual([]);
  });

  // ja: 行の削除と内容の書き換えは保存する一覧に反映される
  it("reflects row edits and deletions in the saved list", async () => {
    const sent = spyCommands();
    const store = useRevisionsStore();
    store.openFor(sheet([rev("A", "初版"), rev("B", "誤記"), rev("C", "追加")]));
    store.removeRow(1);
    store.updateRow(1, { description: "モータ回路追加", by: "竹内" });
    await store.save();
    expect(sent[0]).toMatchObject({
      type: "set_revisions",
      revisions: [rev("A", "初版"), rev("C", "モータ回路追加", "竹内")],
    });
  });

  // ja: 何も編集せずに保存したときはコマンドを送らない(無駄なundo履歴を作らない)
  it("skips the command when nothing was edited", async () => {
    const sent = spyCommands();
    const store = useRevisionsStore();
    store.openFor(sheet([rev("A", "初版")]));
    await store.save();
    expect(sent).toEqual([]);
    expect(store.open).toBe(false);
  });

  // ja: 全欄が空のまま残った行は保存時に取り除かれ、前後の空白も落とされる
  it("drops rows left completely blank and trims the remaining text", async () => {
    const sent = spyCommands();
    const store = useRevisionsStore();
    store.openFor(sheet());
    store.addRow();
    store.updateRow(0, { description: "  初版  ", by: " 竹内 " });
    store.addRow();
    store.updateRow(1, { mark: "", date: "", description: "  ", by: "" });
    await store.save();
    expect(sent[0]).toMatchObject({
      type: "set_revisions",
      revisions: [{ mark: "A", date: todayIso(), description: "初版", by: "竹内" }],
    });
  });

  // ja: 保存に失敗したらダイアログは開いたままエラーを表示する
  it("keeps the dialog open and reports the error when saving fails", async () => {
    vi.spyOn(ipc, "executeCommand").mockRejectedValue(new Error("boom"));
    const store = useRevisionsStore();
    store.openFor(sheet());
    store.addRow();
    await store.save();
    expect(store.open).toBe(true);
    expect(store.error).toContain("boom");
  });
});
