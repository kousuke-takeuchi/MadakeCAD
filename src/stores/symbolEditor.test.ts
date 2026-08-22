// シンボルエディタ (下書きストア) の仕様テスト
// (M4仕様 §7、.pen「M4デザイン - シンボルエディタ」)。
//
// エディタはファイルのドメインで、図面のCommandエンジンとは別。下書きを触っても図面は
// 変わらず、「ユーザーライブラリへ保存」で初めて `~/MadakeCAD/symbols/{id}.json` に書く。
import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { ipc, type SymbolDef } from "../ipc";
import { useDocumentStore } from "./document";
import { useSymbolEditorStore } from "./symbolEditor";

const builtin: SymbolDef = {
  id: "lamp_generic",
  name: "Lamp",
  name_ja: "表示灯",
  category: "output",
  ref_prefix: "L",
  keywords: [],
  primitives: [{ type: "circle", center: { x: 0, y: 0 }, r: 2.5, filled: false }],
  pins: [
    { number: "1", name: "", at: { x: -5, y: 0 }, dir: "left" },
    { number: "2", name: "", at: { x: 5, y: 0 }, dir: "right" },
  ],
  text_slots: [{ key: "TAG", at: { x: 0, y: -7.5 }, height: 2.5 }],
};

/** 複製元 (`duplicate_symbol` が返す下書き)。ファイルにはまだ書かれていない。 */
const copy: SymbolDef = {
  ...builtin,
  id: "lamp_generic_copy",
  name: "Lamp (copy)",
  name_ja: "表示灯のコピー",
  category: "user",
};

/** 定義は毎回作り直す (テスト同士が同じ配列を書き換え合わないように)。 */
function clone<T>(v: T): T {
  return JSON.parse(JSON.stringify(v)) as T;
}

function setup() {
  const doc = useDocumentStore();
  doc.symbols = [clone(builtin)];
  vi.spyOn(ipc, "listUserSymbols").mockResolvedValue({
    symbols: [],
    issues: [],
    user_dir: "/Users/me/MadakeCAD/symbols",
  });
  vi.spyOn(ipc, "listSymbols").mockImplementation(async () => [clone(builtin)]);
  vi.spyOn(ipc, "duplicateSymbol").mockImplementation(async () => clone(copy));
  return doc;
}

/** 保存された下書きを記録する。 */
function spySave(): SymbolDef[] {
  const saved: SymbolDef[] = [];
  vi.spyOn(ipc, "saveUserSymbol").mockImplementation(async (symbol) => {
    saved.push(symbol);
    return { symbol, path: `/Users/me/MadakeCAD/symbols/${symbol.id}.json` };
  });
  return saved;
}

describe("symbol editor store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.restoreAllMocks();
  });

  // ja: 空から始めると、参照記号(TAG)の置き場だけが入った白紙の下書きになる
  it("starts a blank draft that already carries the required TAG attribute slot", async () => {
    setup();
    const editor = useSymbolEditorStore();
    await editor.openBlank();
    expect(editor.open).toBe(true);
    expect(editor.draft?.category).toBe("user");
    expect(editor.draft?.primitives).toEqual([]);
    expect(editor.draft?.pins).toEqual([]);
    expect(editor.draft?.text_slots?.map((s) => s.key)).toEqual(["TAG"]);
    expect(editor.dirty).toBe(false);
  });

  // ja: 既存シンボルを複製して開くと、その図形と接続点がそのまま下書きに入る
  it("opens a duplicate of an existing symbol with its shapes and points copied", async () => {
    setup();
    const editor = useSymbolEditorStore();
    await editor.openDuplicate("lamp_generic");
    expect(ipc.duplicateSymbol).toHaveBeenCalledWith("lamp_generic");
    expect(editor.draft?.id).toBe("lamp_generic_copy");
    expect(editor.draft?.pins).toHaveLength(2);
    expect(editor.userDir).toBe("/Users/me/MadakeCAD/symbols");
    expect(editor.dirty).toBe(false);
  });

  // ja: 図形を描き足すと、その図形が選択された状態になる
  it("selects a shape right after it is drawn", async () => {
    setup();
    const editor = useSymbolEditorStore();
    await editor.openBlank();
    editor.addPrimitive({ type: "rect", p1: { x: -5, y: -5 }, p2: { x: 5, y: 5 }, filled: false });
    expect(editor.draft?.primitives).toHaveLength(1);
    expect(editor.selection).toEqual({ kind: "primitive", index: 0 });
    expect(editor.dirty).toBe(true);
  });

  // ja: 選択した図形はドラッグで動かせる
  it("moves the selected shape by the dragged amount", async () => {
    setup();
    const editor = useSymbolEditorStore();
    await editor.openBlank();
    editor.addPrimitive({ type: "rect", p1: { x: 0, y: 0 }, p2: { x: 5, y: 5 }, filled: false });
    editor.moveSelection(2.5, -2.5);
    expect(editor.draft?.primitives[0]).toEqual({
      type: "rect",
      p1: { x: 2.5, y: -2.5 },
      p2: { x: 7.5, y: 2.5 },
      filled: false,
    });
  });

  // ja: 選択した図形・接続点は削除できる
  it("deletes the selected shape or connection point", async () => {
    setup();
    const editor = useSymbolEditorStore();
    await editor.openDuplicate("lamp_generic");
    editor.select({ kind: "pin", index: 0 });
    editor.deleteSelection();
    expect(editor.draft?.pins.map((p) => p.number)).toEqual(["2"]);
    expect(editor.selection).toBeNull();
    editor.select({ kind: "primitive", index: 0 });
    editor.deleteSelection();
    expect(editor.draft?.primitives).toEqual([]);
  });

  // ja: 方向ボタンで足した接続点には次の番号が付き、2.5mmグリッドに乗る
  it("gives a connection point added from a side button the next number, on the grid", async () => {
    setup();
    const editor = useSymbolEditorStore();
    await editor.openDuplicate("lamp_generic");
    editor.addPin("down");
    const added = editor.draft!.pins[2];
    expect(added.number).toBe("3");
    expect(added.dir).toBe("down");
    expect(Math.abs(added.at.x % 2.5)).toBe(0);
    expect(Math.abs(added.at.y % 2.5)).toBe(0);
  });

  // ja: キャンバスで置いた接続点も、クリック位置に関わらずグリッドの交点に乗る
  it("snaps a connection point dropped on the canvas onto a grid intersection", async () => {
    setup();
    const editor = useSymbolEditorStore();
    await editor.openBlank();
    editor.addPinAt({ x: 6.1, y: -1.4 }, "right");
    expect(editor.draft?.pins[0].at).toEqual({ x: 5, y: -2.5 });
  });

  // ja: 接続点の番号・名前・方向は表から書き換えられる
  it("edits the number, name and direction of a connection point from the table", async () => {
    setup();
    const editor = useSymbolEditorStore();
    await editor.openDuplicate("lamp_generic");
    editor.updatePin(0, { number: "A1", name: "主回路 L1", dir: "up" });
    expect(editor.draft?.pins[0]).toMatchObject({ number: "A1", name: "主回路 L1", dir: "up" });
  });

  // ja: 属性テキストは置いたり外したりでき、外した属性はチェックで注意になる
  it("places and removes an attribute slot, and cautions about the removed ones", async () => {
    setup();
    const editor = useSymbolEditorStore();
    await editor.openDuplicate("lamp_generic");
    editor.toggleSlot("PART");
    expect(editor.draft?.text_slots?.map((s) => s.key)).toEqual(["TAG", "PART"]);
    editor.toggleSlot("PART");
    expect(editor.draft?.text_slots?.map((s) => s.key)).toEqual(["TAG"]);
    expect(editor.checks[3].level).toBe("warn");
  });

  // ja: シンボル情報 (id・名前・接頭辞・検索キー) は右のフォームから書き換えられる
  it("edits the symbol information (id, names, prefix and search keys) from the right panel", async () => {
    setup();
    const editor = useSymbolEditorStore();
    await editor.openBlank();
    editor.setInfo({ id: "shop_lamp", name_ja: "自社ランプ", ref_prefix: "L" });
    editor.setKeywords("lamp, 表示灯, パイロットランプ");
    expect(editor.draft).toMatchObject({ id: "shop_lamp", name_ja: "自社ランプ", ref_prefix: "L" });
    expect(editor.draft?.keywords).toEqual(["lamp", "表示灯", "パイロットランプ"]);
  });

  // ja: 保存先のパスはidに追従する (フッタに出るパス)
  it("keeps the save path in step with the symbol id", async () => {
    setup();
    const editor = useSymbolEditorStore();
    await editor.openBlank();
    expect(editor.filePath).toBe("");
    editor.setInfo({ id: "shop_lamp" });
    expect(editor.filePath).toBe("/Users/me/MadakeCAD/symbols/shop_lamp.json");
  });

  // ja: 同梱シンボルと同じidにすると、保存前チェックがエラーになり保存できない
  it("refuses to save under an id that a bundled symbol already uses", async () => {
    setup();
    const editor = useSymbolEditorStore();
    await editor.openDuplicate("lamp_generic");
    editor.setInfo({ id: "lamp_generic" });
    expect(editor.checks.some((c) => c.code === "symbol.id_reserved")).toBe(true);
    expect(editor.canSave).toBe(false);
  });

  // ja: グリッド外の接続点・番号の重複・必須属性の欠落は保存前チェックに出る
  it("shows off-grid points, duplicate numbers and a missing TAG slot in the checklist", async () => {
    setup();
    const editor = useSymbolEditorStore();
    await editor.openDuplicate("lamp_generic");
    editor.updatePin(1, { at: { x: 1.2, y: 0 } });
    expect(editor.checks[0].level).toBe("error");
    editor.updatePin(1, { at: { x: -5, y: 0 }, number: "1" });
    expect(editor.checks[1].level).toBe("error");
    editor.toggleSlot("TAG");
    expect(editor.checks[2].level).toBe("error");
  });

  // ja: 保存すると下書きがそのままユーザーライブラリへ書かれ、保存先のパスが返る
  it("writes the draft to the user library and reports where it was saved", async () => {
    setup();
    const saved = spySave();
    const editor = useSymbolEditorStore();
    await editor.openDuplicate("lamp_generic");
    const path = await editor.save();
    expect(saved).toHaveLength(1);
    expect(saved[0].id).toBe("lamp_generic_copy");
    expect(path).toBe("/Users/me/MadakeCAD/symbols/lamp_generic_copy.json");
    expect(editor.savedPath).toBe(path);
  });

  // ja: 保存に成功すると未保存の変更が消え、部品挿入の一覧も読み直される
  it("clears the unsaved-changes mark and reloads the insert list after a successful save", async () => {
    const doc = setup();
    spySave();
    const reload = vi.spyOn(doc, "reloadSymbols").mockResolvedValue([builtin, copy]);
    const editor = useSymbolEditorStore();
    await editor.openDuplicate("lamp_generic");
    editor.addPrimitive({ type: "rect", p1: { x: -5, y: -5 }, p2: { x: 5, y: 5 }, filled: false });
    expect(editor.dirty).toBe(true);
    await editor.save();
    expect(editor.dirty).toBe(false);
    expect(reload).toHaveBeenCalled();
  });

  // ja: 保存が拒否されたら何も書かれず、理由が全部そのまま出る
  it("keeps the editor open and shows every reason when the save is refused", async () => {
    setup();
    vi.spyOn(ipc, "saveUserSymbol").mockRejectedValue([
      { code: "symbol.pin_off_grid", message: "接続点2が2.5mmグリッドから外れています" },
      { code: "symbol.slot_missing", message: "参照記号のスロット(TAG)は必須です" },
    ]);
    const editor = useSymbolEditorStore();
    await editor.openDuplicate("lamp_generic");
    expect(await editor.save()).toBeNull();
    expect(editor.open).toBe(true);
    expect(editor.saveErrors.map((e) => e.code)).toEqual([
      "symbol.pin_off_grid",
      "symbol.slot_missing",
    ]);
  });

  // ja: 未保存の変更があるまま閉じようとすると、いったん確認を出して閉じない
  it("asks for confirmation instead of closing while there are unsaved changes", async () => {
    setup();
    const editor = useSymbolEditorStore();
    await editor.openDuplicate("lamp_generic");
    expect(editor.requestClose()).toBe(true);

    await editor.openDuplicate("lamp_generic");
    editor.addPin("down");
    expect(editor.requestClose()).toBe(false);
    expect(editor.open).toBe(true);
    expect(editor.confirmingClose).toBe(true);

    editor.discardAndClose();
    expect(editor.open).toBe(false);
    expect(editor.confirmingClose).toBe(false);
  });

  // ja: 「複製から新規」のピッカーは、同梱もユーザーも合わせた全シンボルから選べる
  it("offers every symbol (bundled and user) in the duplicate-from picker", async () => {
    const doc = setup();
    doc.symbols = [builtin, { ...copy, id: "shop_lamp" }];
    const editor = useSymbolEditorStore();
    await editor.openBlank();
    editor.openPicker();
    expect(editor.pickerOpen).toBe(true);
    expect(editor.pickerRows.map((d) => d.id)).toEqual(["lamp_generic", "shop_lamp"]);
    editor.setPickerQuery("shop");
    expect(editor.pickerRows.map((d) => d.id)).toEqual(["shop_lamp"]);
  });
});
