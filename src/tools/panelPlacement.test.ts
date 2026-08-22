// 盤レイアウトの作図・配置操作 (リボン「パネル」タブのツール + 未配置部品の配置) の仕様テスト
// (M4仕様 §8、計画 docs/superpowers/plans/2026-08-23-m4-phase4-templates-symbol-editor-panel.md Task 6)。
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import {
  ipc,
  type Command,
  type Entity,
  type Patch,
  type Project,
  type Sheet,
  type SheetKind,
  type UnplacedComponent,
} from "../ipc";
import { useDocumentStore } from "../stores/document";
import { useUiStore } from "../stores/ui";
import { EditorController } from "./controller";

const emptyPatch: Patch = { revision: 1, ops: [] };

const component: UnplacedComponent = {
  reference: "K1",
  part_no: "MDK-RLY-24V",
  symbol_id: "relay_coil",
  sheet_id: "s1",
  w: 25,
  h: 65,
};

function seedDocument(kind: SheetKind = "panel", ...entities: Entity[]) {
  const doc = useDocumentStore();
  const sheet: Sheet = {
    id: "sheet-1",
    name: kind === "panel" ? "Panel1" : "Sheet1",
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

/** 送られたコマンドを記録する。 */
function spyCommands(): Command[] {
  const sent: Command[] = [];
  vi.spyOn(ipc, "executeCommand").mockImplementation(async (command) => {
    sent.push(command);
    return emptyPatch;
  });
  return sent;
}

/** ポインタ押下 (テスト環境にPointerEventが無いので読まれるプロパティだけ渡す)。 */
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

describe("panel drawing tools on a panel sheet", () => {
  // ja: 盤枠ツールは矩形ドラッグで盤の外形を1つ作る (undo一発で消える)。
  it("draws the enclosure with one rectangle drag", async () => {
    const controller = controllerAt(seedDocument());
    const sent = spyCommands();
    controller.setTool("panel_outline");
    controller.onPointerDown({ x: 40, y: 30 }, down());
    controller.onPointerMove({ x: 240, y: 230 });
    await controller.onPointerUp();
    expect(sent).toHaveLength(1);
    expect(sent[0]).toMatchObject({
      type: "add_entity",
      entity: { kind: "panel_outline", at: { x: 40, y: 30 }, w: 200, h: 200 },
    });
  });

  // ja: DINレールツールは水平ドラッグでレールを1本作る。
  it("draws a DIN rail with one horizontal drag", async () => {
    const controller = controllerAt(seedDocument());
    const sent = spyCommands();
    controller.setTool("din_rail");
    controller.onPointerDown({ x: 60, y: 120 }, down());
    controller.onPointerMove({ x: 220, y: 121 });
    await controller.onPointerUp();
    expect(sent[0]).toMatchObject({ entity: { kind: "din_rail", y: 120, x1: 60, x2: 220 } });
  });

  // ja: ダクトツールは矩形ドラッグでダクトを1つ作る。
  it("draws a duct with one rectangle drag", async () => {
    const controller = controllerAt(seedDocument());
    const sent = spyCommands();
    controller.setTool("duct");
    controller.onPointerDown({ x: 50, y: 40 }, down());
    controller.onPointerMove({ x: 230, y: 70 });
    await controller.onPointerUp();
    expect(sent[0]).toMatchObject({ entity: { kind: "duct", at: { x: 50, y: 40 }, w: 180, h: 30 } });
  });

  // ja: 作図をやめる (Esc) と選択ツールへ戻り、コマンドは送られない。
  it("sends nothing when the drawing is abandoned with Escape", async () => {
    const controller = controllerAt(seedDocument());
    const sent = spyCommands();
    controller.setTool("duct");
    expect(await controller.onKeyDown(key("Escape"))).toBe(true);
    expect(controller.tool).toBe("select");
    expect(sent).toHaveLength(0);
  });
});

describe("placing an unplaced part", () => {
  const rail: Entity = { kind: "din_rail", id: "r1", y: 150, x1: 40, x2: 260 };

  // ja: 未配置部品を選ぶとフットプリントの配置モードに入る (ゴーストがカーソルに付く)。
  it("enters footprint placement for the chosen part", () => {
    const controller = controllerAt(seedDocument());
    controller.startFootprintPlacement(component);
    expect(controller.tool).toBe("footprint");
    expect(controller.placeFootprint?.reference).toBe("K1");
    expect(controller.placeRotation).toBe(0);
  });

  // ja: クリックでフットプリントが確定し、レールの近くなら取付線へ吸い付く。
  it("commits the footprint on click and snaps it onto the rail", async () => {
    const controller = controllerAt(seedDocument("panel", rail));
    const sent = spyCommands();
    controller.startFootprintPlacement(component);
    controller.onPointerDown({ x: 100, y: 152.5 }, down());
    await vi.waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toMatchObject({
      type: "add_entity",
      entity: {
        kind: "footprint",
        at: { x: 87.5, y: 117.5 },
        w: 25,
        h: 65,
        rotation: 0,
        reference: "K1",
        part_no: "MDK-RLY-24V",
      },
    });
  });

  // ja: 配置中のRは向きを0度と90度で入れ替える (盤の描画はこの2つだけを持つ)。
  it("toggles the footprint between 0 and 90 degrees on R", async () => {
    const controller = controllerAt(seedDocument());
    controller.startFootprintPlacement(component);
    expect(await controller.onKeyDown(key("r"))).toBe(true);
    expect(controller.placeRotation).toBe(90);
    await controller.onKeyDown(key("r"));
    expect(controller.placeRotation).toBe(0);
  });

  // ja: 置いたあとは選択ツールへ戻る (同じ部品を2台置いてしまわない)。
  it("returns to the select tool once the part is placed", async () => {
    const controller = controllerAt(seedDocument("panel", rail));
    const sent = spyCommands();
    controller.startFootprintPlacement(component);
    controller.onPointerDown({ x: 100, y: 152.5 }, down());
    await vi.waitFor(() => expect(sent).toHaveLength(1));
    await vi.waitFor(() => expect(controller.tool).toBe("select"));
    expect(controller.placeFootprint).toBeNull();
  });

  // ja: 盤の要素 (フットプリント・レール・ダクト・盤枠) はクリックで選べる。
  it("selects panel entities by clicking them", () => {
    const footprint: Entity = {
      kind: "footprint",
      id: "fp1",
      at: { x: 60, y: 40 },
      w: 25,
      h: 65,
      rotation: 0,
      reference: "K1",
      part_no: "",
    };
    const duct: Entity = { kind: "duct", id: "d1", at: { x: 40, y: 200 }, w: 200, h: 30 };
    const outline: Entity = {
      kind: "panel_outline",
      id: "o1",
      at: { x: 20, y: 20 },
      w: 260,
      h: 260,
      name: "CP-1",
    };
    const controller = controllerAt(seedDocument("panel", outline, duct, rail, footprint));
    expect(controller.hitTest({ x: 70, y: 70 })).toBe("fp1");
    expect(controller.hitTest({ x: 100, y: 150 })).toBe("r1");
    expect(controller.hitTest({ x: 100, y: 210 })).toBe("d1");
    expect(controller.hitTest({ x: 25, y: 25 })).toBe("o1");
  });

  // ja: 置いたフットプリントをレールの近くへ動かすと、移動もレールへ吸い付く。
  it("snaps a footprint drag onto the rail", async () => {
    const footprint: Entity = {
      kind: "footprint",
      id: "fp1",
      at: { x: 60, y: 40 },
      w: 25,
      h: 65,
      rotation: 0,
      reference: "K1",
      part_no: "",
    };
    const doc = seedDocument("panel", rail, footprint);
    const controller = controllerAt(doc);
    const sent = spyCommands();
    doc.selection = new Set(["fp1"]);
    controller.setTool("select");
    controller.onPointerDown({ x: 70, y: 70 }, down());
    controller.onPointerMove({ x: 80, y: 145 });
    await controller.onPointerUp();
    expect(sent[0]).toMatchObject({ type: "move_entities", ids: ["fp1"], dx: 10, dy: 77.5 });
  });
});

describe("tools that do not belong to the sheet", () => {
  // ja: 盤レイアウトシートでは回路図のツール(配線・部品挿入)へ切り替わらず、理由をログに出す。
  it("refuses the schematic tools on a panel sheet", () => {
    const controller = controllerAt(seedDocument("panel"));
    const ui = useUiStore();
    controller.setTool("wire");
    expect(controller.tool).toBe("select");
    expect(ui.lastMessage).toContain("panel layout sheet");
  });

  // ja: 回路図シートでは盤のツール(盤枠・レール・ダクト)へ切り替わらない。
  it("refuses the panel tools on a schematic sheet", () => {
    const controller = controllerAt(seedDocument("schematic"));
    controller.setTool("duct");
    expect(controller.tool).toBe("select");
  });

  // ja: 回路図シートでは未配置部品の配置も始まらない (置き場所が無い)。
  it("does not start footprint placement on a schematic sheet", () => {
    const controller = controllerAt(seedDocument("schematic"));
    controller.startFootprintPlacement(component);
    expect(controller.tool).toBe("select");
    expect(controller.placeFootprint).toBeNull();
  });

  // ja: 盤シートでも選択・移動・削除はこれまでどおり使える。
  it("keeps select, move and delete available on a panel sheet", () => {
    const controller = controllerAt(seedDocument("panel"));
    controller.setTool("select");
    expect(controller.tool).toBe("select");
  });
});
