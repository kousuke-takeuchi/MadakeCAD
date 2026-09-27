// 電線品番凡例ブロック (G9残) のキャンバス操作の仕様テスト: 置く・選ぶ・動かす。
// 中身 (行) はシートの配線から導出されるので、操作の対象になるのは置き場所だけ。
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { LEGEND_COL_W, LEGEND_ROW_H } from "../canvas/wireLegend";
import { ipc, type Command, type Entity, type Patch, type Project, type Sheet } from "../ipc";
import { useDocumentStore } from "../stores/document";
import { EditorController } from "./controller";

const emptyPatch: Patch = { revision: 1, ops: [] };

/** 凡例ブロック1つと配線1本を置いたシート (行が1行出るので表は見出し+1行の高さになる)。 */
function seedDocument(...entities: Entity[]) {
  const doc = useDocumentStore();
  const sheet: Sheet = {
    id: "sheet-1",
    name: "Sheet1",
    kind: "schematic",
    size: "A3",
    orientation: "Landscape",
    zone_cols: 4,
    zone_rows: 6,
    title_block: {},
    revisions: [],
    entities: Object.fromEntries(entities.map((e) => [e.id, e])),
  };
  doc.project = { sheets: [sheet], wire_parts: [] } as unknown as Project;
  doc.activeSheetId = "sheet-1";
  return doc;
}

const legend = (id: string, x: number, y: number): Entity =>
  ({ kind: "wire_legend", id, at: { x, y } }) as unknown as Entity;

const wire = (id: string): Entity =>
  ({
    kind: "wire",
    id,
    points: [
      { x: 0, y: 0 },
      { x: 10, y: 0 },
    ],
    color: "red",
    sq: 0.75,
    length_m: 1,
    part_no: null,
    net: null,
  }) as unknown as Entity;

function spyCommands(): Command[] {
  const sent: Command[] = [];
  vi.spyOn(ipc, "executeCommand").mockImplementation(async (command) => {
    sent.push(command);
    return emptyPatch;
  });
  return sent;
}

function down(button = 0): PointerEvent {
  return { button, shiftKey: false, altKey: false } as PointerEvent;
}

/** 1mm = 1px にして、画面座標をそのまま用紙座標として書けるようにする。 */
function controllerAt(doc: ReturnType<typeof useDocumentStore>): EditorController {
  const controller = new EditorController(doc);
  controller.vp.scale = 1;
  return controller;
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.restoreAllMocks();
});

describe("wire legend block on the canvas", () => {
  // ja: 電線品番凡例ツールは、クリックした位置を表の左上にしてブロックを1つ置く (置いたらツールは選択へ戻る)。
  it("places one legend block with the clicked point as its top-left corner", async () => {
    const controller = controllerAt(seedDocument(wire("w1")));
    const sent = spyCommands();
    controller.setTool("wire_legend");
    controller.onPointerDown({ x: 60, y: 40 }, down());
    await vi.waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toMatchObject({
      type: "add_entity",
      entity: { kind: "wire_legend", at: { x: 60, y: 40 } },
    });
    await vi.waitFor(() => expect(controller.tool).toBe("select"));
  });

  // ja: 表の矩形の中をクリックすると凡例ブロックが選べ、外をクリックすると選べない。
  it("selects the legend block by clicking inside its table, and not outside it", () => {
    const width = LEGEND_COL_W.reduce((a, b) => a + b, 0);
    const controller = controllerAt(seedDocument(wire("w1"), legend("l1", 100, 50)));
    // 配線1本 → 列見出し1行 + 本文1行
    expect(controller.hitTest({ x: 100 + width / 2, y: 50 + LEGEND_ROW_H })).toBe("l1");
    expect(controller.hitTest({ x: 100 + width + 5, y: 50 + LEGEND_ROW_H })).toBeNull();
    expect(controller.hitTest({ x: 100 + width / 2, y: 50 + LEGEND_ROW_H * 4 })).toBeNull();
  });

  // ja: 選んだ凡例ブロックはドラッグで動かせる (置き場所だけが動き、中身は動かした先のシートから導出し直される)。
  it("moves a selected legend block by dragging it", async () => {
    const doc = seedDocument(wire("w1"), legend("l1", 100, 50));
    const controller = controllerAt(doc);
    const sent = spyCommands();
    controller.setTool("select");
    controller.onPointerDown({ x: 110, y: 52 }, down());
    expect([...doc.selection]).toEqual(["l1"]);
    controller.onPointerMove({ x: 130, y: 62 });
    await controller.onPointerUp();
    expect(sent[0]).toMatchObject({ type: "move_entities", ids: ["l1"], dx: 20, dy: 10 });
  });
});
