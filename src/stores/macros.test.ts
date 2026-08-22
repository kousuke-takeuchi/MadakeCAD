// 回路マクロUI (保存ダイアログ・部品挿入ダイアログの「マクロ」カテゴリ・⌘C/V) の仕様テスト
// (計画: docs/superpowers/plans/2026-08-22-m4-phase2-macros-xref-nav.md Task 2)。
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { ipc, type Command, type Macro, type MacroMeta } from "../ipc";
import { ALL_CATEGORIES, UNCATEGORIZED, useMacrosStore } from "./macros";

const NIL = "00000000-0000-0000-0000-000000000000";

function addSymbol(
  id: string,
  reference: string,
  opts: { symbolId?: string; value?: string; attrs?: Record<string, string> } = {},
): Command {
  return {
    type: "add_entity",
    sheet_id: NIL,
    entity: {
      kind: "symbol",
      id,
      symbol_id: opts.symbolId ?? "relay_coil",
      at: { x: 0, y: 0 },
      rotation: 0,
      mirror: false,
      reference,
      value: opts.value ?? "",
      attrs: opts.attrs ?? {},
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
    placeholders: [],
    value_sets: [],
  };
}

/** モータ回路の選択範囲 (型番欄に値が入ったモータ+定格属性を持つブレーカ)。 */
function motorSelection(): Macro {
  return {
    ...macro("selection", ""),
    commands: [
      addSymbol("m1", "M1", { symbolId: "motor", value: "0.4kW" }),
      addSymbol("cb1", "CB1", { symbolId: "breaker_3p", attrs: { rating: "1-1.6A" } }),
    ],
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

describe("placeholders and value sets in the save dialog", () => {
  // ja: プレースホルダの候補は、選択したシンボルの型番欄と属性の一覧から作られる
  it("lists the value field and every attribute of the selected symbols as a placeholder candidate", async () => {
    vi.spyOn(ipc, "buildMacro").mockResolvedValue(motorSelection());
    const macros = useMacrosStore();
    await macros.openSave("sheet-1", ["m1", "cb1"]);
    expect(macros.placeholderCandidates).toEqual([
      { id: "m1|value", entity: "m1", field: "value", reference: "M1", symbolId: "motor", current: "0.4kW" },
      { id: "cb1|value", entity: "cb1", field: "value", reference: "CB1", symbolId: "breaker_3p", current: "" },
      {
        id: "cb1|attrs.rating",
        entity: "cb1",
        field: "attrs.rating",
        reference: "CB1",
        symbolId: "breaker_3p",
        current: "1-1.6A",
      },
    ]);
  });

  // ja: 同じキー名を付けた複数の欄は、行き先を複数持つ1つのプレースホルダにまとまる
  it("groups every field that was given the same key name into one placeholder with several targets", async () => {
    vi.spyOn(ipc, "buildMacro").mockResolvedValue(motorSelection());
    const macros = useMacrosStore();
    await macros.openSave("sheet-1", ["m1", "cb1"]);
    macros.setPlaceholderKey("m1|value", "rating");
    macros.setPlaceholderKey("cb1|attrs.rating", " rating ");

    expect(macros.savePlaceholders).toEqual([
      {
        key: "rating",
        label: "rating",
        label_ja: "rating",
        targets: [
          { entity: "m1", field: "value" },
          { entity: "cb1", field: "attrs.rating" },
        ],
      },
    ]);
    expect(macros.placeholderKeyList).toEqual(["rating"]);
  });

  // ja: 値セットは行を足して名前とキーごとの値を入れると組み立てられ、idは名前から作られる
  it("builds one value set per row from its name and the value of each key", async () => {
    vi.spyOn(ipc, "buildMacro").mockResolvedValue(motorSelection());
    const macros = useMacrosStore();
    await macros.openSave("sheet-1", ["m1", "cb1"]);
    macros.setPlaceholderKey("m1|value", "rating");

    macros.addValueSet();
    macros.setValueSetLabel(0, "0.75 kW");
    macros.setValueSetValue(0, "rating", "0.75kW");
    macros.addValueSet();
    macros.setValueSetLabel(1, "1.5 kW");
    macros.setValueSetValue(1, "rating", "1.5kW");

    expect(macros.saveValueSets).toEqual([
      { id: "0_75_kw", label: "0.75 kW", label_ja: "0.75 kW", values: { rating: "0.75kW" } },
      { id: "1_5_kw", label: "1.5 kW", label_ja: "1.5 kW", values: { rating: "1.5kW" } },
    ]);

    macros.removeValueSet(0);
    expect(macros.saveValueSets.map((v) => v.id)).toEqual(["1_5_kw"]);
  });

  // ja: 名前を入れていない値セットの行は保存されない(空の値セットは作らない)
  it("drops a value set row that has no name yet", async () => {
    vi.spyOn(ipc, "buildMacro").mockResolvedValue(motorSelection());
    const macros = useMacrosStore();
    await macros.openSave("sheet-1", ["m1"]);
    macros.setPlaceholderKey("m1|value", "rating");
    macros.addValueSet();
    macros.setValueSetValue(0, "rating", "1.5kW");
    expect(macros.saveValueSets).toEqual([]);
  });

  // ja: プレースホルダと値セットは保存時にマクロの情報として一緒に渡される
  it("sends the placeholders and the value sets along with the save", async () => {
    vi.spyOn(ipc, "buildMacro").mockResolvedValue(motorSelection());
    const saved = macro("motor_circuit", "motor");
    const save = vi
      .spyOn(ipc, "saveMacro")
      .mockResolvedValue({ macro: saved, path: "/home/u/MadakeCAD/macros/motor_circuit.json" });
    mockList([...library, saved]);
    const macros = useMacrosStore();
    await macros.openSave("sheet-1", ["m1", "cb1"]);
    macros.saveName = "モータ回路";
    macros.setPlaceholderKey("m1|value", "rating");
    macros.setPlaceholderKey("cb1|attrs.rating", "rating");
    macros.addValueSet();
    macros.setValueSetLabel(0, "1.5 kW");
    macros.setValueSetValue(0, "rating", "1.5kW");

    await macros.save();
    const meta: MacroMeta = {
      name: "モータ回路",
      name_ja: "モータ回路",
      category: "",
      placeholders: [
        {
          key: "rating",
          label: "rating",
          label_ja: "rating",
          targets: [
            { entity: "m1", field: "value" },
            { entity: "cb1", field: "attrs.rating" },
          ],
        },
      ],
      value_sets: [{ id: "1_5_kw", label: "1.5 kW", label_ja: "1.5 kW", values: { rating: "1.5kW" } }],
    };
    expect(save).toHaveBeenCalledWith("sheet-1", ["m1", "cb1"], meta);
  });

  // ja: 何も指定しなければ保存の中身は今までどおり(プレースホルダも値セットも付かない)
  it("saves exactly as before when no placeholder and no value set was entered", async () => {
    vi.spyOn(ipc, "buildMacro").mockResolvedValue(motorSelection());
    const saved = macro("plain", "");
    const save = vi.spyOn(ipc, "saveMacro").mockResolvedValue({ macro: saved, path: "/p.json" });
    mockList([saved]);
    const macros = useMacrosStore();
    await macros.openSave("sheet-1", ["m1"]);
    macros.saveName = "そのまま";
    await macros.save();
    expect(save).toHaveBeenCalledWith("sheet-1", ["m1"], {
      name: "そのまま",
      name_ja: "そのまま",
      category: "",
    });
  });

  // ja: 保存ダイアログを開き直すと、前回のプレースホルダと値セットは残らない
  it("starts every save dialog with no placeholder keys and no value set rows", async () => {
    vi.spyOn(ipc, "buildMacro").mockResolvedValue(motorSelection());
    const macros = useMacrosStore();
    await macros.openSave("sheet-1", ["m1", "cb1"]);
    macros.setPlaceholderKey("m1|value", "rating");
    macros.addValueSet();
    macros.setValueSetLabel(0, "1.5 kW");

    await macros.openSave("sheet-1", ["m1"]);
    expect(macros.savePlaceholders).toEqual([]);
    expect(macros.saveValueSets).toEqual([]);
  });
});

describe("choosing a value set when inserting", () => {
  // ja: 値セットを持たないマクロでは選ぶものが無い(挿入ダイアログのドロップダウンを出さない)
  it("offers no value set for a macro that defines none", async () => {
    mockList();
    const macros = useMacrosStore();
    await macros.load();
    macros.select("motor_dol");
    expect(macros.valueSets).toEqual([]);
    expect(macros.valueSetId).toBeNull();
  });

  // ja: 値セットを持つマクロでは一覧が並び、選んだ値セットを覚える
  it("lists the value sets of the selected macro and remembers the chosen one", async () => {
    const rated: Macro = {
      ...macro("motor_rated", "motor"),
      placeholders: [
        { key: "rating", label: "Rating", label_ja: "定格", targets: [{ entity: "m1", field: "value" }] },
      ],
      value_sets: [
        { id: "0_75_kw", label: "0.75 kW", label_ja: "0.75kW", values: { rating: "0.75kW" } },
        { id: "1_5_kw", label: "1.5 kW", label_ja: "1.5kW", values: { rating: "1.5kW" } },
      ],
    };
    mockList([...library, rated]);
    const macros = useMacrosStore();
    await macros.load();
    macros.select("motor_rated");
    expect(macros.valueSets.map((v) => v.id)).toEqual(["0_75_kw", "1_5_kw"]);
    macros.setValueSet("1_5_kw");
    expect(macros.valueSetId).toBe("1_5_kw");
  });

  // ja: 別のマクロを選び直すと、値セットの選択は外れる(そのマクロには無い値セットのため)
  it("clears the chosen value set when another macro is selected", async () => {
    const rated: Macro = {
      ...macro("motor_rated", "motor"),
      value_sets: [{ id: "1_5_kw", label: "1.5 kW", label_ja: "1.5kW", values: {} }],
    };
    mockList([...library, rated]);
    const macros = useMacrosStore();
    await macros.load();
    macros.select("motor_rated");
    macros.setValueSet("1_5_kw");
    macros.select("power_24v");
    expect(macros.valueSetId).toBeNull();
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
