// 図形 (G10) の作図ツールの仕様テスト
// (計画 docs/superpowers/plans/2026-08-23-phase6-quality-gaps.md Task 1)。
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { ipc, type Command, type Entity, type Patch, type Project, type Sheet, type SheetKind } from "../ipc";
import { newShape } from "../canvas/shapes";
import { useDocumentStore } from "../stores/document";
import { EditorController } from "./controller";

const emptyPatch: Patch = { revision: 1, ops: [] };

function seedDocument(kind: SheetKind = "schematic", ...entities: Entity[]) {
  const doc = useDocumentStore();
  const sheet: Sheet = {
    id: "sheet-1",
    name: "Sheet1",
    kind,
    size: "A3",
    orientation: "Landscape",
    zone_cols: 4,
    zone_rows: 6,
    title_block: {},
    revisions: [],
    entities: Object.fromEntries(entities.map((e) => [e.id, e])),
  };
  doc.project = { sheets: [sheet] } as unknown as Project;
  doc.activeSheetId = "sheet-1";
  return doc;
}

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

function key(k: string): KeyboardEvent {
  return { key: k, code: k, metaKey: false, ctrlKey: false, shiftKey: false } as KeyboardEvent;
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

describe("drawing shapes", () => {
  // ja: 矩形ツールは1回のドラッグで矩形を1つ作る(undo一発で消える)
  it("draws a rectangle with one drag", async () => {
    const controller = controllerAt(seedDocument());
    const sent = spyCommands();
    controller.setTool("shape_rect");
    controller.onPointerDown({ x: 50, y: 50 }, down());
    controller.onPointerMove({ x: 150, y: 100 });
    await controller.onPointerUp();
    expect(sent).toHaveLength(1);
    const command = sent[0];
    expect(command.type).toBe("add_entity");
    if (command.type !== "add_entity" || command.entity.kind !== "shape") throw new Error("図形の追加");
    expect(command.entity.shape).toBe("rect");
    expect(command.entity.points).toEqual([
      { x: 50, y: 50 },
      { x: 150, y: 100 },
    ]);
  });

  // ja: 線ツールと円ツールも1回のドラッグで1つ作る(円は押した点が中心)
  it("draws a line and a circle with one drag each", async () => {
    for (const [tool, kind] of [
      ["shape_line", "line"],
      ["shape_circle", "circle"],
    ] as const) {
      setActivePinia(createPinia());
      const controller = controllerAt(seedDocument());
      const sent = spyCommands();
      controller.setTool(tool);
      controller.onPointerDown({ x: 100, y: 50 }, down());
      controller.onPointerMove({ x: 150, y: 50 });
      await controller.onPointerUp();
      const command = sent[0];
      if (command?.type !== "add_entity" || command.entity.kind !== "shape") {
        throw new Error(`${tool}: 図形の追加`);
      }
      expect(command.entity.shape).toBe(kind);
      expect(command.entity.points[0]).toEqual({ x: 100, y: 50 });
    }
  });

  // ja: クリックしただけ(ドラッグしていない)ときは何も作らない
  it("creates nothing from a stray click", async () => {
    const controller = controllerAt(seedDocument());
    const sent = spyCommands();
    controller.setTool("shape_rect");
    controller.onPointerDown({ x: 50, y: 50 }, down());
    await controller.onPointerUp();
    expect(sent).toHaveLength(0);
  });

  // ja: 折れ線はクリックを重ねて打ち、ダブルクリックで確定する
  it("draws a polyline by clicking each corner and finishing with a double click", async () => {
    const controller = controllerAt(seedDocument());
    const sent = spyCommands();
    controller.setTool("shape_polyline");
    controller.onPointerDown({ x: 50, y: 50 }, down());
    controller.onPointerDown({ x: 100, y: 50 }, down());
    controller.onPointerDown({ x: 100, y: 90 }, down());
    expect(sent).toHaveLength(0);
    controller.onDoubleClick();
    await Promise.resolve();
    await Promise.resolve();
    const command = sent[0];
    if (command?.type !== "add_entity" || command.entity.kind !== "shape") throw new Error("図形の追加");
    expect(command.entity.shape).toBe("polyline");
    expect(command.entity.points).toEqual([
      { x: 50, y: 50 },
      { x: 100, y: 50 },
      { x: 100, y: 90 },
    ]);
  });

  // ja: 折れ線はEnterでも確定でき、点が1つだけなら何も作らずに捨てる
  it("finishes a polyline with Enter and throws away a single stray point", async () => {
    const controller = controllerAt(seedDocument());
    const sent = spyCommands();
    controller.setTool("shape_polyline");
    controller.onPointerDown({ x: 50, y: 50 }, down());
    await controller.onKeyDown(key("Enter"));
    await Promise.resolve();
    expect(sent).toHaveLength(0);

    controller.onPointerDown({ x: 50, y: 50 }, down());
    controller.onPointerDown({ x: 80, y: 50 }, down());
    await controller.onKeyDown(key("Enter"));
    await Promise.resolve();
    await Promise.resolve();
    expect(sent).toHaveLength(1);
  });

  // ja: 図形ツールは盤レイアウトシートでは使えない(回路図の作図要素なので)
  it("refuses the shape tools on a panel layout sheet", () => {
    const controller = controllerAt(seedDocument("panel"));
    controller.setTool("shape_rect");
    expect(controller.tool).toBe("select");
  });
});

describe("selecting shapes", () => {
  // ja: 図形は輪郭で掴む(塗りの下の配線を選びたいので、面の中では掴まない)
  it("picks a shape by its outline, not by the area inside it", () => {
    const rect = newShape("rect", [
      { x: 50, y: 50 },
      { x: 150, y: 100 },
    ]);
    const controller = controllerAt(seedDocument("schematic", rect));
    expect(controller.hitTest({ x: 100, y: 50 })).toBe(rect.id);
    expect(controller.hitTest({ x: 100, y: 75 })).toBeNull();
  });

  // ja: 図形を移動すると全ての頂点が動く(move_entitiesコマンド1本)
  it("moves every point of a shape with one move_entities command", async () => {
    const rect = newShape("rect", [
      { x: 50, y: 50 },
      { x: 150, y: 100 },
    ]);
    const doc = seedDocument("schematic", rect);
    const controller = controllerAt(doc);
    const sent = spyCommands();
    controller.setTool("select");
    controller.onPointerDown({ x: 100, y: 50 }, down());
    controller.onPointerMove({ x: 110, y: 55 });
    await controller.onPointerUp();
    expect(sent).toEqual([
      { type: "move_entities", sheet_id: "sheet-1", ids: [rect.id], dx: 10, dy: 5 },
    ]);
  });
});
