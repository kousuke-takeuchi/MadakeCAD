import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { useDocumentStore } from "./document";
import { ipc, type FrameTemplate, type Project, type Sheet } from "../ipc";
import { DEFAULT_FRAME_TEMPLATE } from "../canvas/frameTemplate";

function testProject(): Project {
  const sheet: Sheet = {
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
  return { format_version: 2, name: "t", wire_parts: [], plc_assignments: [], sheets: [sheet] };
}

describe("document store", () => {
  beforeEach(() => setActivePinia(createPinia()));

  // ja: 自動注記 (シート間XRef・リレー注記・デバイス単位XRef) はドキュメントが変わったときだけ計算し直す (描画のたびに全プロジェクトを走査しない)
  it("recomputes the sheet annotations only when the document changes", () => {
    const store = useDocumentStore();
    store.project = testProject();
    store.activeSheetId = "s1";
    const first = store.sheetAnnotations;
    expect(first).not.toBeNull();
    // 同じドキュメントなら計算し直さない (getterのキャッシュがそのまま返る)
    expect(store.sheetAnnotations).toBe(first);
    store.applyPatch({
      revision: 1,
      ops: [
        {
          op: "entity_upserted",
          sheet_id: "s1",
          entity: {
            kind: "symbol",
            id: "k1",
            symbol_id: "relay_coil",
            at: { x: 20, y: 20 },
            rotation: 0,
            mirror: false,
            reference: "K1",
            value: "",
            attrs: {},
          },
        },
      ],
    });
    // 図面が変わったら計算し直す
    expect(store.sheetAnnotations).not.toBe(first);
  });

  // ja: entity_upserted / entity_removed パッチがミラーのシートを更新する
  it("entity_upserted / entity_removed patches update the mirrored sheet", () => {
    const store = useDocumentStore();
    store.project = testProject();
    store.applyPatch({
      revision: 1,
      ops: [
        {
          op: "entity_upserted",
          sheet_id: "s1",
          entity: { kind: "junction", id: "e1", at: { x: 1, y: 2 } },
        },
      ],
    });
    expect(store.revision).toBe(1);
    expect(store.project!.sheets[0].entities["e1"]).toBeTruthy();
    store.applyPatch({
      revision: 2,
      ops: [{ op: "entity_removed", sheet_id: "s1", id: "e1" }],
    });
    expect(store.project!.sheets[0].entities["e1"]).toBeUndefined();
  });

  // ja: project_replacedでミラーのプロジェクト全体が置き換わる
  it("project_replaced swaps the whole mirrored project", () => {
    const store = useDocumentStore();
    store.applyPatch({
      revision: 5,
      ops: [
        {
          op: "project_replaced",
          project: { format_version: 2, name: "new", wire_parts: [], plc_assignments: [], sheets: [] },
        },
      ],
    });
    expect(store.project!.name).toBe("new");
    expect(store.revision).toBe(5);
  });

  // ja: シート追加・削除のパッチは並び順を保つ
  it("sheet add/remove patches keep sheet order", () => {
    const store = useDocumentStore();
    store.project = testProject();
    const s2: Sheet = { ...testProject().sheets[0], id: "s2", name: "TB2" };
    store.applyPatch({ revision: 3, ops: [{ op: "sheet_added", index: 1, sheet: s2 }] });
    expect(store.project!.sheets.map((s) => s.id)).toEqual(["s1", "s2"]);
    store.applyPatch({ revision: 4, ops: [{ op: "sheet_removed", sheet_id: "s1" }] });
    expect(store.project!.sheets.map((s) => s.id)).toEqual(["s2"]);
  });

  // ja: シートメタ更新のパッチはエンティティに触れない
  it("sheet-meta patches never touch the entities", () => {
    const store = useDocumentStore();
    store.project = testProject();
    store.project.sheets[0].entities["e1"] = {
      kind: "junction",
      id: "e1",
      at: { x: 0, y: 0 },
    };
    const meta: Sheet = { ...testProject().sheets[0], name: "改名", entities: {} };
    store.applyPatch({ revision: 6, ops: [{ op: "sheet_meta_updated", sheet: meta }] });
    expect(store.project!.sheets[0].name).toBe("改名");
    expect(store.project!.sheets[0].entities["e1"]).toBeTruthy();
  });

  // ja: 古いrevisionのパッチは破棄される(二重配信しても安全)
  it("patches with an older revision are discarded (duplicate delivery is safe)", () => {
    const store = useDocumentStore();
    store.project = testProject();
    store.applyPatch({
      revision: 10,
      ops: [
        {
          op: "entity_upserted",
          sheet_id: "s1",
          entity: { kind: "junction", id: "e1", at: { x: 0, y: 0 } },
        },
      ],
    });
    store.applyPatch({ revision: 9, ops: [{ op: "entity_removed", sheet_id: "s1", id: "e1" }] });
    expect(store.project!.sheets[0].entities["e1"]).toBeTruthy();
    expect(store.revision).toBe(10);
  });

  // ja: mech_links_replaced パッチでFreeCAD対応付けの一覧が置き換わる
  it("a mech_links_replaced patch replaces the FreeCAD link list", () => {
    const store = useDocumentStore();
    store.project = testProject();
    store.applyPatch({
      revision: 1,
      ops: [
        {
          op: "mech_links_replaced",
          mech_links: [
            {
              entity_id: "e1",
              fcstd_path: "/work/panel.FCStd",
              object_name: "Relay001",
              synced_at: "2026-09-27T10:00:00Z",
            },
          ],
        },
      ],
    });
    expect(store.project?.mech_links?.map((l) => l.object_name)).toEqual(["Relay001"]);
  });

  // ja: 図枠テンプレートを差し替えるパッチはミラーへ反映され、undoでも即座に元へ戻る
  it("frame_template_changed patches update the mirror both ways (switch and undo)", () => {
    const store = useDocumentStore();
    store.project = testProject();
    store.applyPatch({
      revision: 1,
      ops: [{ op: "frame_template_changed", frame_template: "jis-a4-portrait" }],
    });
    expect(store.project!.frame_template).toBe("jis-a4-portrait");
    store.applyPatch({
      revision: 2,
      ops: [{ op: "frame_template_changed", frame_template: null }],
    });
    expect(store.project!.frame_template).toBeNull();
  });
});

/** 一覧に並ぶ図枠テンプレート (中身はレイアウトに関係しない最小限)。 */
function frameTemplate(id: string, overrides: Partial<FrameTemplate> = {}): FrameTemplate {
  return { ...DEFAULT_FRAME_TEMPLATE, id, name: id, name_ja: id, builtin: true, ...overrides };
}

describe("document store: 図枠テンプレートの解決", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.restoreAllMocks();
  });

  // ja: 起動時に図枠テンプレートの一覧を読み込み、以降の描画はその定義で行う
  it("loads the frame template catalogue on bootstrap", async () => {
    const store = useDocumentStore();
    vi.spyOn(ipc, "getProject").mockResolvedValue({
      project: testProject(),
      revision: 0,
      can_undo: false,
      can_redo: false,
    });
    vi.spyOn(ipc, "listSymbols").mockResolvedValue([]);
    vi.spyOn(ipc, "onPatch").mockResolvedValue(() => {});
    const list = vi
      .spyOn(ipc, "listFrameTemplates")
      .mockResolvedValue({ templates: [frameTemplate("jis-a3")], issues: [] });
    await store.bootstrap();
    expect(list).toHaveBeenCalledOnce();
    expect(store.frameTemplates.map((t) => t.id)).toEqual(["jis-a3"]);
  });

  // ja: 図枠テンプレートの一覧が読めなくても図面は開ける(同梱のjis-a3で描く)
  it("still opens the drawing when the frame template catalogue cannot be read", async () => {
    const store = useDocumentStore();
    vi.spyOn(ipc, "getProject").mockResolvedValue({
      project: testProject(),
      revision: 0,
      can_undo: false,
      can_redo: false,
    });
    vi.spyOn(ipc, "listSymbols").mockResolvedValue([]);
    vi.spyOn(ipc, "onPatch").mockResolvedValue(() => {});
    vi.spyOn(ipc, "listFrameTemplates").mockRejectedValue(new Error("no backend"));
    await store.bootstrap();
    expect(store.frameTemplates).toEqual([]);
    expect(store.frameTemplate.id).toBe("jis-a3");
  });

  // ja: プロジェクトが選んでいる図枠テンプレートの定義が、図面の描画に使われる
  it("resolves the frame template the project has chosen", () => {
    const store = useDocumentStore();
    store.project = { ...testProject(), frame_template: "jis-a4-portrait" };
    store.frameTemplates = [frameTemplate("jis-a3"), frameTemplate("jis-a4-portrait")];
    expect(store.frameTemplate.id).toBe("jis-a4-portrait");
  });

  // ja: 図枠テンプレートが未指定なら同梱のjis-a3で描く(従来どおりの見た目)
  it("falls back to the bundled JIS A3 frame when the project names none", () => {
    const store = useDocumentStore();
    store.project = testProject();
    store.frameTemplates = [frameTemplate("jis-a3"), frameTemplate("jis-a4-portrait")];
    expect(store.frameTemplate.id).toBe("jis-a3");
  });

  // ja: 他所で作った図面が知らない図枠idを持っていても、jis-a3で開けて図面は失われない
  it("falls back to JIS A3 for a frame id this installation does not know", () => {
    const store = useDocumentStore();
    store.project = { ...testProject(), frame_template: "acme-custom" };
    store.frameTemplates = [frameTemplate("jis-a3")];
    expect(store.frameTemplate.id).toBe("jis-a3");
  });

  // ja: 図枠を差し替えるとキャンバスへ渡す図枠定義もその場で入れ替わる(全シートに効く)
  it("swaps the frame handed to the canvas as soon as the project's frame changes", () => {
    const store = useDocumentStore();
    store.project = testProject();
    store.frameTemplates = [
      frameTemplate("jis-a3"),
      frameTemplate("jis-a4-portrait", { title_block: { ...DEFAULT_FRAME_TEMPLATE.title_block, w: 110 } }),
    ];
    expect(store.frameOptions!.frame.title_block.w).toBe(120);
    store.applyPatch({
      revision: 1,
      ops: [{ op: "frame_template_changed", frame_template: "jis-a4-portrait" }],
    });
    expect(store.frameOptions!.frame.title_block.w).toBe(110);
  });

  // ja: 表題欄のバインドはプロジェクト文脈で解決される(「3枚中2枚目」がシートごとに入る)
  it("binds the title block against the project, so each sheet knows its own number out of the total", () => {
    const store = useDocumentStore();
    const project = testProject();
    project.sheets = [
      project.sheets[0],
      { ...project.sheets[0], id: "s2", name: "動力" },
      { ...project.sheets[0], id: "s3", name: "制御" },
    ];
    store.project = project;
    store.activeSheetId = "s2";
    expect(store.frameOptions!.bind).toMatchObject({
      projectName: "t",
      sheetNo: 2,
      sheetTotal: 3,
      sheetName: "動力",
    });
  });

  // ja: シートが1枚も無ければ図枠の描画情報は無い(空の図面でも落ちない)
  it("has no frame options at all when the project holds no sheet", () => {
    const store = useDocumentStore();
    store.project = { ...testProject(), sheets: [] };
    expect(store.frameOptions).toBeNull();
  });
});
