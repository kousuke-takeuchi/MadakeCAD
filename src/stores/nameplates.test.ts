// 銘板の挿入ダイアログ (M4 §8) の仕様テスト。
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { ipc, type Command, type Project } from "../ipc";
import { useDocumentStore } from "./document";
import { useNameplatesStore } from "./nameplates";

function project(): Project {
  return {
    format_version: 16,
    name: "P",
    wire_parts: [],
    sheets: [
      { id: "s1", name: "回路", kind: "schematic", entities: { r: { kind: "symbol", id: "r", symbol_id: "relay_coil", at: { x: 0, y: 0 }, rotation: 0, mirror: false, reference: "K1", value: "", attrs: { function_label: "主回路接触器" } } } },
      { id: "p1", name: "盤", kind: "panel", entities: { k1: { kind: "footprint", id: "k1", at: { x: 100, y: 50 }, w: 40, h: 60, rotation: 0, reference: "K1", part_no: "", item_no: null } } },
    ],
  } as unknown as Project;
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.restoreAllMocks();
  const doc = useDocumentStore();
  doc.project = project();
  doc.activeSheetId = "p1";
});

describe("銘板の挿入", () => {
  // ja: 部品をクリックして開くと、1行目は回路図の説明から始まり、説明への追従が付く
  it("starts the first line from the schematic description when opened on a part", () => {
    const np = useNameplatesStore();
    np.openFor("k1", { x: 0, y: 0 });
    expect(np.reference).toBe("K1");
    expect(np.line1).toBe("主回路接触器");
    expect(np.follow).toBe(true);
  });

  // ja: 挿入は部品に結び付いた銘板をadd_entity 1回で置き、何も無い所で開けば単独の銘板になる
  it("inserts a nameplate tied to the part with one add_entity, or a standalone one", async () => {
    const sent: Command[] = [];
    vi.spyOn(ipc, "executeCommand").mockImplementation(async (c) => {
      sent.push(c);
      return { revision: 2, ops: [] };
    });
    const np = useNameplatesStore();
    np.openFor("k1", { x: 0, y: 0 });
    np.line2 = "200V 9A";
    np.partNo = "SAMPLE-NP";
    expect(await np.insert()).toBe(true);
    expect(sent[0]).toMatchObject({
      type: "add_entity",
      sheet_id: "p1",
      entity: { kind: "nameplate", footprint: "k1", lines: ["主回路接触器", "200V 9A"], follow_description: true, part_no: "SAMPLE-NP" },
    });
    np.openFor(null, { x: 300, y: 200 });
    expect(np.follow).toBe(false);
    await np.insert();
    expect(sent[1]).toMatchObject({ entity: { kind: "nameplate", footprint: null, at: { x: 300, y: 200 }, follow_description: false } });
  });
});
