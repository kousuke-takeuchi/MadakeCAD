// テンプレート選択ダイアログの仕様テスト (docs/internal/specs/m3-ai-first.md §2)。
import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { ipc, type Patch, type Template } from "../ipc";
import { entityBounds, fitPreview, templateEntities, templateSheet } from "../canvas/templatePreview";
import { templateDescription, templateName, useTemplatesStore } from "./templates";

function template(id: string, nameJa: string): Template {
  return {
    id,
    name: `${id} (en)`,
    name_ja: nameJa,
    description: "what it contains",
    description_ja: "入っているもの",
    builtin: true,
    commands: [
      {
        type: "add_entity",
        sheet_id: "00000000-0000-0000-0000-000000000000",
        entity: {
          kind: "symbol",
          id: "e1",
          symbol_id: "battery",
          at: { x: 75, y: 100 },
          rotation: 0,
          mirror: false,
          reference: "BT1",
          value: "DC24V",
          attrs: {},
        },
      },
      {
        type: "add_entity",
        sheet_id: "00000000-0000-0000-0000-000000000000",
        entity: {
          kind: "wire",
          id: "e2",
          points: [
            { x: 67.5, y: 100 },
            { x: 60, y: 100 },
            { x: 60, y: 80 },
          ],
          color: "blue",
          sq: 0.75,
          length_m: null,
          part_no: null,
          net: null,
        },
      },
    ],
  };
}

const twoTemplates = [template("control_24v_basic", "24V制御基本"), template("motor_starter", "モータ起動回路")];

function mockList(templates = twoTemplates, issues: { path: string; message: string }[] = []) {
  vi.spyOn(ipc, "listTemplates").mockResolvedValue({ templates, issues });
}

function mockApply() {
  const patch: Patch = { revision: 7, ops: [] };
  return vi.spyOn(ipc, "applyTemplate").mockResolvedValue(patch);
}

describe("template picker store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.restoreAllMocks();
    mockList();
  });

  // ja: ダイアログを開くとテンプレート一覧を読み込み、先頭を選んだ状態で表示する
  it("opens with the first template selected", async () => {
    const store = useTemplatesStore();
    await store.openDialog();
    expect(store.open).toBe(true);
    expect(store.templates).toHaveLength(2);
    expect(store.selectedId).toBe("control_24v_basic");
    expect(store.selected?.name_ja).toBe("24V制御基本");
  });

  // ja: タイルを選ぶと選択中のテンプレートが切り替わる (右のプレビューが変わる)
  it("switches the selected template when another tile is picked", async () => {
    const store = useTemplatesStore();
    await store.openDialog();
    store.select("motor_starter");
    expect(store.selected?.id).toBe("motor_starter");
  });

  // ja: 「このテンプレートで開始」で選択中のテンプレートが現在のシートへ適用され、ダイアログが閉じる
  it("applies the selected template to the current sheet and closes", async () => {
    const apply = mockApply();
    const store = useTemplatesStore();
    await store.openDialog();
    store.select("motor_starter");
    const applied = await store.apply("sheet-1");
    expect(apply).toHaveBeenCalledWith("motor_starter", "sheet-1");
    expect(applied?.id).toBe("motor_starter");
    expect(store.open).toBe(false);
  });

  // ja: キャンセルではダイアログが閉じるだけで、図面には何も適用されない
  it("applies nothing when the dialog is cancelled", async () => {
    const apply = mockApply();
    const store = useTemplatesStore();
    await store.openDialog();
    store.cancel();
    expect(store.open).toBe(false);
    expect(apply).not.toHaveBeenCalled();
  });

  // ja: 適用に失敗したときは理由を表示し、ダイアログは開いたままにする (選択をやり直せる)
  it("keeps the dialog open and shows why when applying fails", async () => {
    vi.spyOn(ipc, "applyTemplate").mockRejectedValue(new Error("unknown template"));
    const store = useTemplatesStore();
    await store.openDialog();
    const applied = await store.apply("sheet-1");
    expect(applied).toBeNull();
    expect(store.open).toBe(true);
    expect(store.error).toContain("unknown template");
  });

  // ja: 読み込めなかったテンプレートファイルはパスと理由つきで持ち帰り、残りのテンプレートは選べる
  it("reports template files it could not read", async () => {
    mockList(twoTemplates, [{ path: "/home/me/MadakeCAD/templates/broken.json", message: "EOF" }]);
    const store = useTemplatesStore();
    await store.openDialog();
    expect(store.templates).toHaveLength(2);
    expect(store.issues[0].path).toContain("broken.json");
  });

  // ja: テンプレートが1つも無いときは選択なしで開き、適用しても何も起きない
  it("opens with nothing selected when there are no templates", async () => {
    mockList([]);
    const apply = mockApply();
    const store = useTemplatesStore();
    await store.openDialog();
    expect(store.selectedId).toBeNull();
    expect(await store.apply("sheet-1")).toBeNull();
    expect(apply).not.toHaveBeenCalled();
  });

  // ja: 「+ ユーザーテンプレートを追加...」はテンプレートフォルダをファイラで開く
  it("opens the user template folder in the file manager", async () => {
    const open = vi
      .spyOn(ipc, "openTemplatesFolder")
      .mockResolvedValue("/home/me/MadakeCAD/templates");
    const store = useTemplatesStore();
    await store.openDialog();
    expect(await store.openUserFolder()).toEqual({
      path: "/home/me/MadakeCAD/templates",
      opened: true,
    });
    expect(open).toHaveBeenCalled();
  });

  // ja: ファイラを開けない環境では、テンプレートを置くフォルダのパスだけを案内する
  it("falls back to naming the folder when it cannot be opened", async () => {
    mockList(twoTemplates, []);
    vi.spyOn(ipc, "listTemplates").mockResolvedValue({
      templates: twoTemplates,
      issues: [],
      user_dir: "/home/me/MadakeCAD/templates",
    });
    vi.spyOn(ipc, "openTemplatesFolder").mockRejectedValue(new Error("browser mode"));
    const store = useTemplatesStore();
    await store.openDialog();
    expect(await store.openUserFolder()).toEqual({
      path: "/home/me/MadakeCAD/templates",
      opened: false,
    });
  });

  // ja: 名前と説明はUI言語に従い、日本語以外では英語表記になる
  it("shows names and descriptions in the UI language", () => {
    const t = twoTemplates[0];
    expect(templateName(t, "ja")).toBe("24V制御基本");
    expect(templateName(t, "en")).toBe("control_24v_basic (en)");
    expect(templateDescription(t, "ja")).toBe("入っているもの");
    expect(templateDescription(t, "en")).toBe("what it contains");
  });
});

describe("template preview", () => {
  // ja: プレビューはテンプレートが置くエンティティを仮のシートに組み立てて描く
  it("builds a throwaway sheet from the template's commands", () => {
    const sheet = templateSheet(twoTemplates[0]);
    expect(Object.keys(sheet.entities)).toEqual(["e1", "e2"]);
    expect(sheet.size).toBe("A3");
    expect(templateEntities(twoTemplates[0].commands)).toHaveLength(2);
  });

  // ja: プレビューは図面の描画範囲を求め、その中心がキャンバスの中心に来るよう合わせる
  it("centres the template's drawing in the preview canvas", () => {
    const entities = templateEntities(twoTemplates[0].commands);
    expect(entityBounds(entities)).toEqual({ minX: 60, minY: 80, maxX: 75, maxY: 100 });
    const vp = fitPreview(entities, 200, 100);
    const centre = vp.toScreen({ x: 67.5, y: 90 });
    expect(centre.x).toBeCloseTo(100);
    expect(centre.y).toBeCloseTo(50);
  });

  // ja: 空のテンプレートでもプレビューは既定のビューポートを返す (描画で落ちない)
  it("survives a template that draws nothing", () => {
    expect(entityBounds([])).toBeNull();
    expect(fitPreview([], 96, 54).scale).toBeGreaterThan(0);
  });
});
