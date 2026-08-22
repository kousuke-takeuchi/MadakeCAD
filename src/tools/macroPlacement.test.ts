// 回路マクロの配置モード (ゴースト → R回転 / Tabバリアント切替 → クリック確定) の仕様テスト
// (計画: docs/superpowers/plans/2026-08-22-m4-phase2-macros-xref-nav.md Task 2)。
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { ipc, type Command, type Macro, type Patch, type Project, type Sheet } from "../ipc";
import { useDocumentStore } from "../stores/document";
import { useMacrosStore } from "../stores/macros";
import { EditorController } from "./controller";

const NIL = "00000000-0000-0000-0000-000000000000";

function addSymbol(id: string): Command {
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
      reference: "K1",
      value: "",
      attrs: {},
    },
  };
}

function macro(id = "motor_dol", variantKeys: string[] = ["B", "C"]): Macro {
  return {
    id,
    name: id,
    name_ja: id,
    description: "",
    description_ja: "",
    category: "motor",
    base_point: { x: 0, y: 0 },
    commands: [addSymbol("s1")],
    variants: variantKeys.map((key) => ({
      key,
      name: key,
      name_ja: key,
      commands: [addSymbol(`s-${key}`)],
    })),
    placeholders: [],
    value_sets: [],
  };
}

/** 値セットを2つ持つマクロ (挿入時に定格を一括設定するモータ回路)。 */
function ratedMacro(): Macro {
  return {
    ...macro("motor_rated", []),
    placeholders: [
      { key: "rating", label: "Rating", label_ja: "定格", targets: [{ entity: "s1", field: "value" }] },
    ],
    value_sets: [
      { id: "0_75_kw", label: "0.75 kW", label_ja: "0.75kW", values: { rating: "0.75kW" } },
      { id: "1_5_kw", label: "1.5 kW", label_ja: "1.5kW", values: { rating: "1.5kW" } },
    ],
  };
}

const emptyPatch: Patch = { revision: 1, ops: [] };

function seedDocument(selection: string[] = []) {
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
    entities: {},
  };
  doc.project = { sheets: [sheet] } as unknown as Project;
  doc.activeSheetId = "sheet-1";
  doc.selection = new Set(selection);
  return doc;
}

/** キー入力 (テスト環境にKeyboardEventが無いので、読まれるプロパティだけを渡す)。 */
function key(k: string, mod = false): KeyboardEvent {
  return { key: k, code: k, metaKey: mod, ctrlKey: false, shiftKey: false } as KeyboardEvent;
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.restoreAllMocks();
});

describe("starting macro placement", () => {
  // ja: マクロを配置し始めるとマクロツールになり、バリアントAで回転0のゴーストが出る
  it("switches to the macro tool with variant A and no rotation", () => {
    const controller = new EditorController(seedDocument());
    controller.startMacroPlacement(macro());
    expect(controller.tool).toBe("macro");
    expect(controller.macroVariantKey).toBe("A");
    expect(controller.placeRotation).toBe(0);
  });

  // ja: Escでマクロ配置をやめると選択ツールへ戻り、ゴーストが消える
  it("leaves macro placement on Escape, back to the select tool", async () => {
    const controller = new EditorController(seedDocument());
    controller.startMacroPlacement(macro());
    expect(await controller.onKeyDown(key("Escape"))).toBe(true);
    expect(controller.tool).toBe("select");
    expect(controller.placeMacro).toBeNull();
  });
});

describe("R and Tab during macro placement", () => {
  // ja: 配置中のRは90度ずつ回転し、4回で元の向きへ戻る
  it("rotates the ghost a quarter turn per R, back to the start after four", async () => {
    const controller = new EditorController(seedDocument());
    controller.startMacroPlacement(macro());
    await controller.onKeyDown(key("r"));
    expect(controller.placeRotation).toBe(90);
    await controller.onKeyDown(key("r"));
    await controller.onKeyDown(key("r"));
    expect(controller.placeRotation).toBe(270);
    await controller.onKeyDown(key("r"));
    expect(controller.placeRotation).toBe(0);
  });

  // ja: 配置中のTabはバリアントをA→B→C→Aと巡回し、ゴーストが差し替わる
  it("cycles the variants A, B, C and back to A on Tab", async () => {
    const controller = new EditorController(seedDocument());
    controller.startMacroPlacement(macro());
    expect(await controller.onKeyDown(key("Tab"))).toBe(true);
    expect(controller.macroVariantKey).toBe("B");
    await controller.onKeyDown(key("Tab"));
    expect(controller.macroVariantKey).toBe("C");
    await controller.onKeyDown(key("Tab"));
    expect(controller.macroVariantKey).toBe("A");
  });

  // ja: バリアントが1つしか無いマクロではTabを押しても「A」のまま
  it("keeps the single variant A on Tab when the macro has no other variants", async () => {
    const controller = new EditorController(seedDocument());
    controller.startMacroPlacement(macro("simple", []));
    await controller.onKeyDown(key("Tab"));
    expect(controller.macroVariantKey).toBe("A");
  });

  // ja: マクロを配置していないときのTabは横取りしない(キャンバス外の操作を邪魔しない)
  it("does not swallow Tab when no macro is being placed", async () => {
    const controller = new EditorController(seedDocument());
    expect(await controller.onKeyDown(key("Tab"))).toBe(false);
  });
});

describe("confirming a macro placement", () => {
  // ja: ライブラリのマクロはid・バリアント・クリック位置・回転を渡して挿入される
  it("applies a library macro by id at the clicked point with the current variant and rotation", async () => {
    const apply = vi.spyOn(ipc, "applyMacro").mockResolvedValue(emptyPatch);
    const controller = new EditorController(seedDocument());
    controller.startMacroPlacement(macro());
    await controller.onKeyDown(key("Tab"));
    await controller.onKeyDown(key("r"));

    expect(controller.macroApplyArgs({ x: 150, y: 120 })).toEqual({
      kind: "library",
      macroId: "motor_dol",
      variant: "B",
      valueSet: null,
      sheetId: "sheet-1",
      at: { x: 150, y: 120 },
      rotation: 90,
    });
    await controller.commitMacro({ x: 150, y: 120 });
    expect(apply).toHaveBeenCalledWith("motor_dol", "B", null, "sheet-1", { x: 150, y: 120 }, 90);
  });

  // ja: 貼り付けた無名マクロはidではなくマクロそのものを渡して挿入される(ファイルが無いため)
  it("applies a pasted macro by value instead of by id, since it has no file", async () => {
    const inline = vi.spyOn(ipc, "applyMacroInline").mockResolvedValue(emptyPatch);
    const controller = new EditorController(seedDocument());
    const m = macro("clipboard", []);
    controller.startMacroPlacement(m, { fromLibrary: false });

    expect(controller.macroApplyArgs({ x: 10, y: 20 })).toMatchObject({ kind: "inline" });
    await controller.commitMacro({ x: 10, y: 20 });
    expect(inline).toHaveBeenCalledWith(m, "A", null, "sheet-1", { x: 10, y: 20 }, 0);
  });

  // ja: 確定してもマクロツールのままなので、同じマクロを続けて何個でも置ける
  it("stays in macro placement after a click, so the same macro can be placed again", async () => {
    vi.spyOn(ipc, "applyMacro").mockResolvedValue(emptyPatch);
    const controller = new EditorController(seedDocument());
    controller.startMacroPlacement(macro());
    await controller.commitMacro({ x: 10, y: 20 });
    expect(controller.tool).toBe("macro");
    expect(controller.placeMacro?.macro.id).toBe("motor_dol");
  });
});

describe("placing a macro with a value set", () => {
  // ja: 挿入ダイアログで選んだ値セットは、挿入の引数にそのまま乗る(定格が一括で入る)
  it("passes the value set chosen in the dialog to the insert", async () => {
    const apply = vi.spyOn(ipc, "applyMacro").mockResolvedValue(emptyPatch);
    const controller = new EditorController(seedDocument());
    controller.startMacroPlacement(ratedMacro(), { fromLibrary: true, valueSet: "1_5_kw" });

    expect(controller.macroApplyArgs({ x: 40, y: 60 })).toEqual({
      kind: "library",
      macroId: "motor_rated",
      variant: "A",
      valueSet: "1_5_kw",
      sheetId: "sheet-1",
      at: { x: 40, y: 60 },
      rotation: 0,
    });
    await controller.commitMacro({ x: 40, y: 60 });
    expect(apply).toHaveBeenCalledWith(
      "motor_rated",
      "A",
      "1_5_kw",
      "sheet-1",
      { x: 40, y: 60 },
      0,
    );
  });

  // ja: 値セットを選ばずに置いたマクロは、保存時の値のまま入る
  it("inserts a macro with no value set chosen just as it was saved", async () => {
    const apply = vi.spyOn(ipc, "applyMacro").mockResolvedValue(emptyPatch);
    const controller = new EditorController(seedDocument());
    controller.startMacroPlacement(ratedMacro());
    await controller.commitMacro({ x: 40, y: 60 });
    expect(apply).toHaveBeenCalledWith("motor_rated", "A", null, "sheet-1", { x: 40, y: 60 }, 0);
  });
});

describe("Cmd+C / Cmd+V", () => {
  // ja: 選択範囲を⌘Cすると無名マクロとして覚え、⌘Vでそのまま配置モードに入る
  it("copies the selection on Cmd+C and starts placing it on Cmd+V", async () => {
    const copied = macro("clipboard", []);
    vi.spyOn(ipc, "buildMacro").mockResolvedValue(copied);
    const controller = new EditorController(seedDocument(["e1", "e2"]));

    expect(await controller.onKeyDown(key("c", true))).toBe(true);
    expect(useMacrosStore().clipboard?.id).toBe("clipboard");

    expect(await controller.onKeyDown(key("v", true))).toBe(true);
    expect(controller.tool).toBe("macro");
    expect(controller.placeMacro).toEqual({ macro: copied, fromLibrary: false, valueSet: null });
  });

  // ja: 何も選択していない⌘C・何もコピーしていない⌘Vはキー入力を横取りしない
  it("does not swallow Cmd+C with an empty selection nor Cmd+V with an empty clipboard", async () => {
    const controller = new EditorController(seedDocument());
    expect(await controller.onKeyDown(key("c", true))).toBe(false);
    expect(await controller.onKeyDown(key("v", true))).toBe(false);
  });
});
