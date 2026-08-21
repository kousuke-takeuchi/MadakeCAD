import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { useDocumentStore } from "./document";
import type { Project, Sheet } from "../ipc";

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
  return { format_version: 1, name: "t", wire_parts: [], sheets: [sheet] };
}

describe("document store", () => {
  beforeEach(() => setActivePinia(createPinia()));

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
          project: { format_version: 1, name: "new", wire_parts: [], sheets: [] },
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
});
