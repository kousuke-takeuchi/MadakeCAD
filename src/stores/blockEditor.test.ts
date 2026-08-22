// ブロック(汎用機器)のピン表エディタ(下書きストア)の仕様テスト。
// 部品挿入ダイアログの「ブロック(汎用)」パネルと、配置済みブロックの
// 「ピン表を編集…」ダイアログが同じ下書きを共有する。
// 図面への反映は必ず update_entity コマンド1回 (undo一発)。
// 計画: docs/superpowers/plans/2026-08-23-phase5-reproduction-parity.md Task 2。
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { ipc, type Command, type Entity, type Patch, type Project, type Sheet } from "../ipc";
import { useBlockEditorStore } from "./blockEditor";
import { useDocumentStore } from "./document";

const emptyPatch: Patch = { revision: 1, ops: [] };

/** 左5ピン(IN1..)/右3ピン(OUT1..)のコントローラ相当ブロック。 */
const PINS =
  '{"left":[{"no":"IN1","name":"入力1"},{"no":"IN2","name":"入力2"}],"right":[{"no":"OUT1","name":"出力1"}]}';

function block(attrs: Record<string, string> = { block_pins: PINS }): Entity {
  return {
    kind: "symbol",
    id: "u1",
    symbol_id: "block",
    at: { x: 100, y: 100 },
    rotation: 0,
    mirror: false,
    reference: "CTRL1",
    value: "GENERIC-CTRL",
    attrs,
  };
}

function seed(entities: Entity[] = [block()]) {
  const doc = useDocumentStore();
  const sheet: Sheet = {
    id: "sheet-1",
    name: "制御回路図",
    size: "A3",
    orientation: "Landscape",
    zone_cols: 8,
    zone_rows: 4,
    title_block: {},
    revisions: [],
    entities: Object.fromEntries(entities.map((e) => [e.id, e])),
  };
  doc.project = { sheets: [sheet] } as unknown as Project;
  doc.activeSheetId = "sheet-1";
  return doc;
}

/** 実行されたコマンドを記録する。 */
function spyExecute(): Command[] {
  const sent: Command[] = [];
  vi.spyOn(ipc, "executeCommand").mockImplementation(async (command: Command) => {
    sent.push(command);
    return emptyPatch;
  });
  return sent;
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.restoreAllMocks();
});

describe("starting a new block from the insert dialog", () => {
  // ja: 部品挿入で「ブロック(汎用)」を選ぶと、左右1本ずつのピン表が開く
  it("opens a pin table with one pin per side when the generic block is picked", () => {
    seed([]);
    const editor = useBlockEditorStore();
    editor.startInsert();
    expect(editor.active).toBe(true);
    expect(editor.mode).toBe("insert");
    expect(editor.draft?.rows.left).toEqual([{ no: "1", name: "" }]);
    expect(editor.draft?.rows.right).toEqual([{ no: "2", name: "" }]);
  });

  // ja: 編集したピン表はそのまま配置に使う属性になる(配置後もこの形で描かれる)
  it("hands the edited table to the placement as block attributes", () => {
    seed([]);
    const editor = useBlockEditorStore();
    editor.startInsert();
    editor.updateRow("left", 0, { no: "IN1", name: "入力1" });
    editor.removeRow("right", 0);
    expect(editor.attrs).toEqual({ block_pins: '{"left":[{"no":"IN1","name":"入力1"}]}' });
  });

  // ja: 「手動」に切り替えると、そのときの自動寸法が編集できる初期値として入る
  it("seeds the hand-set size with the automatic one when switching to manual", () => {
    seed([]);
    const editor = useBlockEditorStore();
    editor.startInsert();
    expect(editor.sizeMode).toBe("auto");
    editor.setSizeMode("manual");
    expect(editor.sizeMode).toBe("manual");
    expect(editor.draft?.size).toEqual(editor.autoSize);
    editor.setSizeMode("auto");
    expect(editor.draft?.size).toBeNull();
  });

  // ja: ピン番号が重なっているあいだは配置できない(指摘が出る)
  it("refuses to place while two pins share a number", () => {
    seed([]);
    const editor = useBlockEditorStore();
    editor.startInsert();
    editor.updateRow("right", 0, { no: "1" });
    expect(editor.issues).toEqual([{ code: "duplicate", no: "1" }]);
    expect(editor.canApply).toBe(false);
  });
});

describe("editing a block that is already on the drawing", () => {
  // ja: プロパティの「ピン表を編集…」は、そのブロックのピン表を読み込んで開く
  it("loads the pin table of the selected block", () => {
    seed();
    const editor = useBlockEditorStore();
    editor.startEdit(block() as Entity);
    expect(editor.mode).toBe("edit");
    expect(editor.entityId).toBe("u1");
    expect(editor.draft?.rows.left.map((r) => r.no)).toEqual(["IN1", "IN2"]);
    expect(editor.draft?.rows.right.map((r) => r.name)).toEqual(["出力1"]);
  });

  // ja: 適用するとupdate_entityコマンドが1回だけ実行され、参照記号・型番・位置は変わらない
  it("applies the table as a single update_entity, keeping the tag, part number and position", async () => {
    seed();
    const sent = spyExecute();
    const editor = useBlockEditorStore();
    editor.startEdit(block() as Entity);
    editor.addRow("right");
    editor.updateRow("right", 1, { no: "OUT2", name: "出力2" });

    expect(await editor.apply()).toBe(true);
    expect(sent).toHaveLength(1);
    expect(sent[0]).toMatchObject({
      type: "update_entity",
      sheet_id: "sheet-1",
      entity: {
        id: "u1",
        reference: "CTRL1",
        value: "GENERIC-CTRL",
        at: { x: 100, y: 100 },
        attrs: {
          block_pins:
            '{"left":[{"no":"IN1","name":"入力1"},{"no":"IN2","name":"入力2"}],"right":[{"no":"OUT1","name":"出力1"},{"no":"OUT2","name":"出力2"}]}',
        },
      },
    });
    expect(editor.active).toBe(false);
  });

  // ja: ブロック以外の属性(定格など)は編集しても残る
  it("keeps attributes that are not part of the pin table", async () => {
    seed([block({ block_pins: PINS, current_a: "5" })]);
    const sent = spyExecute();
    const editor = useBlockEditorStore();
    editor.startEdit(block({ block_pins: PINS, current_a: "5" }) as Entity);
    editor.addRow("left");
    await editor.apply();
    expect((sent[0] as { entity: { attrs: Record<string, string> } }).entity.attrs.current_a).toBe("5");
  });

  // ja: 外形を自動へ戻して適用すると、手動寸法の属性が消える
  it("drops the hand-set size attribute when the outline goes back to automatic", async () => {
    const withSize = block({ block_pins: PINS, block_size: '{"w":40,"h":30}' });
    seed([withSize]);
    const sent = spyExecute();
    const editor = useBlockEditorStore();
    editor.startEdit(withSize as Entity);
    expect(editor.sizeMode).toBe("manual");
    editor.setSizeMode("auto");
    await editor.apply();
    expect((sent[0] as { entity: { attrs: Record<string, string> } }).entity.attrs.block_size).toBeUndefined();
  });

  // ja: 何も変えずに適用したときはコマンドを送らない(履歴を汚さない)
  it("sends no command when the table was not changed", async () => {
    seed();
    const sent = spyExecute();
    const editor = useBlockEditorStore();
    editor.startEdit(block() as Entity);
    expect(await editor.apply()).toBe(false);
    expect(sent).toEqual([]);
  });

  // ja: 閉じると下書きを捨てる(図面は変わらない)
  it("throws the draft away when the editor is closed", () => {
    seed();
    const sent = spyExecute();
    const editor = useBlockEditorStore();
    editor.startEdit(block() as Entity);
    editor.addRow("left");
    editor.close();
    expect(editor.active).toBe(false);
    expect(editor.draft).toBeNull();
    expect(sent).toEqual([]);
  });
});

describe("warning about wired pins", () => {
  /** IN1 の接続点へ届く配線 (ブロックは (100,100) に置いてある)。 */
  function wiredSheet(): Entity[] {
    return [
      block(),
      {
        kind: "wire",
        id: "w1",
        points: [
          { x: 60, y: 97.5 },
          { x: 92.5, y: 97.5 },
        ],
        color: "red",
        sq: 0.75,
        length_m: null,
        part_no: null,
        net: null,
      },
    ];
  }

  // ja: 配線済みのピンの番号を変えると「接続が外れる」警告が出る
  it("warns that a wired pin loses its connection when its number changes", () => {
    seed(wiredSheet());
    const editor = useBlockEditorStore();
    editor.startEdit(block() as Entity);
    expect(editor.wiredPins).toEqual(["IN1"]);
    expect(editor.warnings).toEqual([]);
    editor.updateRow("left", 0, { no: "A1" });
    expect(editor.warnings).toEqual(["IN1"]);
  });

  // ja: 配線済みのピンを消しても同じ警告が出る
  it("warns the same way when a wired pin is deleted", () => {
    seed(wiredSheet());
    const editor = useBlockEditorStore();
    editor.startEdit(block() as Entity);
    editor.removeRow("left", 0);
    expect(editor.warnings).toEqual(["IN1"]);
  });

  // ja: 配線されていないピンを消しても警告は出ない
  it("stays quiet when the deleted pin had no wire on it", () => {
    seed(wiredSheet());
    const editor = useBlockEditorStore();
    editor.startEdit(block() as Entity);
    editor.removeRow("right", 0);
    expect(editor.warnings).toEqual([]);
  });

  // ja: 警告が出ていても適用はできる(承知のうえで直せる)
  it("still allows applying the edit, warning and all", async () => {
    seed(wiredSheet());
    const sent = spyExecute();
    const editor = useBlockEditorStore();
    editor.startEdit(block() as Entity);
    editor.removeRow("left", 0);
    expect(editor.canApply).toBe(true);
    expect(await editor.apply()).toBe(true);
    expect(sent).toHaveLength(1);
  });
});
