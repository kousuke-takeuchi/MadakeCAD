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

  it("applies entity_upserted and entity_removed patches", () => {
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

  it("replaces whole project on project_replaced", () => {
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

  it("adds and removes sheets, preserving order", () => {
    const store = useDocumentStore();
    store.project = testProject();
    const s2: Sheet = { ...testProject().sheets[0], id: "s2", name: "TB2" };
    store.applyPatch({ revision: 3, ops: [{ op: "sheet_added", index: 1, sheet: s2 }] });
    expect(store.project!.sheets.map((s) => s.id)).toEqual(["s1", "s2"]);
    store.applyPatch({ revision: 4, ops: [{ op: "sheet_removed", sheet_id: "s1" }] });
    expect(store.project!.sheets.map((s) => s.id)).toEqual(["s2"]);
  });

  it("updates sheet meta without touching entities", () => {
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

  it("ignores stale patches with older revision", () => {
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
