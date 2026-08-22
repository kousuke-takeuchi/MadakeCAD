// 回路マクロUI (保存ダイアログ・部品挿入ダイアログの「マクロ」カテゴリ・⌘C/V) の仕様テスト
// (計画: docs/superpowers/plans/2026-08-22-m4-phase2-macros-xref-nav.md Task 2)。
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { ipc, type Command, type Macro, type MacroMeta } from "../ipc";
import { ALL_CATEGORIES, UNCATEGORIZED, useMacrosStore } from "./macros";

const NIL = "00000000-0000-0000-0000-000000000000";

function addSymbol(id: string, reference: string): Command {
  return {
    type: "add_entity",
    sheet_id: NIL,
    entity: {
      kind: "symbol",
      id,
      symbol_id: "relay_coil",
      at: { x: 0, y: 0 },
      rotation: 0,
      mirror: false,
      reference,
      value: "",
      attrs: {},
    },
  };
}

function macro(id: string, category: string, variants = 0): Macro {
  return {
    id,
    name: `${id} (en)`,
    name_ja: `${id} (ja)`,
    description: "",
    description_ja: "",
    category,
    base_point: { x: 0, y: 0 },
    commands: [addSymbol(`${id}-1`, "K1")],
    variants: Array.from({ length: variants }, (_, i) => ({
      key: String.fromCharCode(66 + i),
      name: `v${i}`,
      name_ja: `v${i}`,
      commands: [addSymbol(`${id}-v${i}`, "K1")],
    })),
  };
}

const library = [
  macro("motor_dol", "motor", 2),
  macro("motor_reversing", "motor"),
  macro("power_24v", "power"),
  macro("scratch", ""),
];

function mockList(macros = library) {
  return vi
    .spyOn(ipc, "listMacros")
    .mockResolvedValue({ macros, issues: [], user_dir: "/home/u/MadakeCAD/macros" });
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.restoreAllMocks();
});

describe("macro library listing", () => {
  // ja: 一覧を読み込むとマクロ・読めなかったファイル・置き場のパスがそろう
  it("loads the macros, the unreadable files and the user folder path", async () => {
    mockList();
    const macros = useMacrosStore();
    await macros.load();
    expect(macros.macros).toHaveLength(4);
    expect(macros.user_dir).toBe("/home/u/MadakeCAD/macros");
  });

  // ja: カテゴリツリーは「すべて」が先頭で、分類の無いマクロは「ユーザー」に入る
  it("builds a category tree led by an all-macros entry, with uncategorized macros in their own group", async () => {
    mockList();
    const macros = useMacrosStore();
    await macros.load();
    expect(macros.categories).toEqual([
      { key: ALL_CATEGORIES, count: 4 },
      { key: "motor", count: 2 },
      { key: "power", count: 1 },
      { key: UNCATEGORIZED, count: 1 },
    ]);
  });

  // ja: タイルにはバリアント数(既定のA+持っているバリアント)のバッジが付く
  it("badges every tile with its variant count, counting the default A", async () => {
    mockList();
    const macros = useMacrosStore();
    await macros.load();
    const badges = Object.fromEntries(macros.tiles.map((t) => [t.macro.id, t.variantCount]));
    expect(badges).toEqual({ motor_dol: 3, motor_reversing: 1, power_24v: 1, scratch: 1 });
  });

  // ja: カテゴリを選ぶとそのカテゴリのタイルだけが並ぶ
  it("shows only the tiles of the selected category", async () => {
    mockList();
    const macros = useMacrosStore();
    await macros.load();
    macros.setCategory("motor");
    expect(macros.tiles.map((t) => t.macro.id)).toEqual(["motor_dol", "motor_reversing"]);
    macros.setCategory(UNCATEGORIZED);
    expect(macros.tiles.map((t) => t.macro.id)).toEqual(["scratch"]);
  });

  // ja: 検索語は名前(英語・日本語)とidに部分一致し、大文字小文字は区別しない
  it("filters tiles by a case-insensitive substring of the name or the id", async () => {
    mockList();
    const macros = useMacrosStore();
    await macros.load();
    macros.query = "REVERS";
    expect(macros.tiles.map((t) => t.macro.id)).toEqual(["motor_reversing"]);
  });

  // ja: タイルを選ぶとバリアントは既定の「A」に戻り、右のプレビューが切り替わる
  it("selects a tile and resets the variant back to the default A", async () => {
    mockList();
    const macros = useMacrosStore();
    await macros.load();
    macros.select("motor_dol");
    macros.setVariant("C");
    macros.select("power_24v");
    expect(macros.selected?.id).toBe("power_24v");
    expect(macros.variantKey).toBe("A");
  });

  // ja: 一覧の読み込みに失敗しても画面は壊れず、理由がエラーとして残る
  it("keeps the dialog usable and records the reason when the list cannot be read", async () => {
    vi.spyOn(ipc, "listMacros").mockRejectedValue(new Error("no folder"));
    const macros = useMacrosStore();
    await macros.load();
    expect(macros.macros).toEqual([]);
    expect(macros.error).toContain("no folder");
  });
});

describe("macro save dialog", () => {
  // ja: 選択範囲があるときだけ保存ダイアログが開き、選択範囲のプレビューを組み立てる
  it("opens the save dialog for a selection and builds a preview of it", async () => {
    const built = macro("selection", "");
    const build = vi.spyOn(ipc, "buildMacro").mockResolvedValue(built);
    const macros = useMacrosStore();
    const opened = await macros.openSave("sheet-1", ["e1", "e2"]);
    expect(opened).toBe(true);
    expect(macros.saveOpen).toBe(true);
    expect(build).toHaveBeenCalledWith("sheet-1", ["e1", "e2"], { name: "" });
    expect(macros.savePreview).toEqual(built);
    expect(macros.saveEntityCount).toBe(2);
  });

  // ja: 何も選択していないときは保存ダイアログを開かない(マクロにする回路が無い)
  it("refuses to open the save dialog when nothing is selected", async () => {
    const build = vi.spyOn(ipc, "buildMacro");
    const macros = useMacrosStore();
    expect(await macros.openSave("sheet-1", [])).toBe(false);
    expect(macros.saveOpen).toBe(false);
    expect(build).not.toHaveBeenCalled();
  });

  // ja: 名前が空のあいだは保存できない(名前がマクロのidになるため)
  it("keeps saving disabled until a name has been entered", async () => {
    vi.spyOn(ipc, "buildMacro").mockResolvedValue(macro("selection", ""));
    const macros = useMacrosStore();
    await macros.openSave("sheet-1", ["e1"]);
    expect(macros.canSave).toBe(false);
    macros.saveName = "   ";
    expect(macros.canSave).toBe(false);
    macros.saveName = "テスト回路";
    expect(macros.canSave).toBe(true);
  });

  // ja: 保存は選択範囲・名前・カテゴリをそのまま渡し、保存後は一覧を読み直して閉じる
  it("saves the selection with the entered name and category, then reloads the library", async () => {
    vi.spyOn(ipc, "buildMacro").mockResolvedValue(macro("selection", ""));
    const saved = macro("test_circuit", "motor");
    const save = vi
      .spyOn(ipc, "saveMacro")
      .mockResolvedValue({ macro: saved, path: "/home/u/MadakeCAD/macros/test_circuit.json" });
    const list = mockList([...library, saved]);
    const macros = useMacrosStore();
    await macros.openSave("sheet-1", ["e1", "e2"]);
    macros.saveName = "テスト回路";
    macros.saveCategory = "motor";

    const result = await macros.save();
    const meta: MacroMeta = {
      name: "テスト回路",
      name_ja: "テスト回路",
      category: "motor",
    };
    expect(save).toHaveBeenCalledWith("sheet-1", ["e1", "e2"], meta);
    expect(result?.macro.id).toBe("test_circuit");
    expect(list).toHaveBeenCalled();
    expect(macros.saveOpen).toBe(false);
  });

  // ja: 保存に失敗したらダイアログは開いたまま理由を出す(入力をやり直せる)
  it("leaves the dialog open with the reason when saving fails", async () => {
    vi.spyOn(ipc, "buildMacro").mockResolvedValue(macro("selection", ""));
    vi.spyOn(ipc, "saveMacro").mockRejectedValue(new Error("disk full"));
    const macros = useMacrosStore();
    await macros.openSave("sheet-1", ["e1"]);
    macros.saveName = "テスト回路";
    expect(await macros.save()).toBeNull();
    expect(macros.saveOpen).toBe(true);
    expect(macros.error).toContain("disk full");
  });

  // ja: 基準点は選択範囲から自動で決まり(左下ピン)、保存ダイアログには表示だけする
  it("shows the base point derived from the selection, which the user does not edit", async () => {
    vi.spyOn(ipc, "buildMacro").mockResolvedValue({
      ...macro("selection", ""),
      base_point: { x: 92.5, y: 130 },
    });
    const macros = useMacrosStore();
    await macros.openSave("sheet-1", ["e1"]);
    expect(macros.savePreview?.base_point).toEqual({ x: 92.5, y: 130 });
  });
});

describe("unnamed clipboard macro (Cmd+C / Cmd+V)", () => {
  // ja: ⌘Cは選択範囲を無名マクロとしてメモリに持ち、ファイルには書き出さない
  it("copies the selection into an in-memory macro without writing any file", async () => {
    const built = macro("clipboard", "");
    const build = vi.spyOn(ipc, "buildMacro").mockResolvedValue(built);
    const save = vi.spyOn(ipc, "saveMacro");
    const macros = useMacrosStore();

    const copied = await macros.copy("sheet-1", ["e1", "e2"]);
    expect(copied?.commands).toHaveLength(1);
    expect(build).toHaveBeenCalledWith("sheet-1", ["e1", "e2"], { name: "" });
    expect(save).not.toHaveBeenCalled();
    expect(macros.clipboard).toEqual(built);
  });

  // ja: 何も選択していない状態の⌘Cは何もしない(前のコピー内容も消さない)
  it("does nothing on copy when nothing is selected, keeping the previous clipboard", async () => {
    const built = macro("clipboard", "");
    vi.spyOn(ipc, "buildMacro").mockResolvedValue(built);
    const macros = useMacrosStore();
    await macros.copy("sheet-1", ["e1"]);
    expect(await macros.copy("sheet-1", [])).toBeNull();
    expect(macros.clipboard).toEqual(built);
  });

  // ja: コピーした無名マクロは何度でも貼り付けられる(貼り付けても消えない)
  it("hands the copied macro back on every paste, so it can be pasted repeatedly", async () => {
    const built = macro("clipboard", "");
    vi.spyOn(ipc, "buildMacro").mockResolvedValue(built);
    const macros = useMacrosStore();
    await macros.copy("sheet-1", ["e1"]);
    expect(macros.paste()?.id).toBe("clipboard");
    expect(macros.paste()?.id).toBe("clipboard");
  });

  // ja: 何もコピーしていないときの⌘Vは何も起こさない
  it("pastes nothing when nothing has been copied yet", () => {
    expect(useMacrosStore().paste()).toBeNull();
  });
});
