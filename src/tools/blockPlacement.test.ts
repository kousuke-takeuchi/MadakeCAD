// ブロック(汎用機器)の配置ゴーストと、シンボルの選択判定(外形矩形ベース)の仕様テスト。
// 計画: docs/superpowers/plans/2026-08-23-phase5-reproduction-parity.md Task 2。
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
  type SymbolDef,
  type SymbolInstance,
} from "../ipc";
import { deviceBlockBounds } from "../canvas/relayXref";
import { useDocumentStore } from "../stores/document";
import { EditorController } from "./controller";

const emptyPatch: Patch = { revision: 1, ops: [] };

/** 8ピンのブロック (縦長: 外形は高さ45mm・幅15mm)。 */
const TALL_PINS = JSON.stringify({
  left: Array.from({ length: 8 }, (_, i) => ({ no: `IN${i + 1}` })),
});

/** 小さな表示灯 (接続点は左右5mm)。 */
const lamp: SymbolDef = {
  id: "lamp_generic",
  name: "Lamp",
  name_ja: "表示灯",
  category: "output",
  ref_prefix: "L",
  keywords: [],
  primitives: [{ type: "circle", center: { x: 0, y: 0 }, r: 2.5, filled: false }],
  pins: [
    { number: "1", name: "", at: { x: -5, y: 0 }, dir: "left" },
    { number: "2", name: "", at: { x: 5, y: 0 }, dir: "right" },
  ],
};

/** リレーコイル (A1/A2ピンは±7.5mm、外形は高さ6mm)。 */
const relayCoil: SymbolDef = {
  id: "relay_coil",
  name: "Relay coil",
  name_ja: "リレーコイル",
  category: "relay",
  ref_prefix: "K",
  keywords: [],
  primitives: [{ type: "rect", p1: { x: -5, y: -3 }, p2: { x: 5, y: 3 }, filled: false }],
  pins: [
    { number: "A1", name: "", at: { x: -7.5, y: 0 }, dir: "left" },
    { number: "A2", name: "", at: { x: 7.5, y: 0 }, dir: "right" },
  ],
};

/** a接点 (実機ブロックの接点イメージに1行出すためだけに置く)。 */
const relayContactNo: SymbolDef = {
  ...relayCoil,
  id: "relay_contact_no",
  name: "Relay contact (NO)",
  name_ja: "リレー接点(a接点)",
  primitives: [],
};

function symbol(id: string, symbolId: string, attrs: Record<string, string> = {}): Entity {
  return {
    kind: "symbol",
    id,
    symbol_id: symbolId,
    at: { x: 100, y: 100 },
    rotation: 0,
    mirror: false,
    reference: "CTRL1",
    value: "",
    attrs,
  };
}

function seed(entities: Entity[] = []) {
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
    entities: Object.fromEntries(entities.map((e) => [e.id, e])),
  };
  doc.project = { sheets: [sheet] } as unknown as Project;
  doc.activeSheetId = "sheet-1";
  doc.symbols = [lamp, relayCoil, relayContactNo];
  return doc;
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.restoreAllMocks();
});

describe("placing a block with a pin table", () => {
  // ja: ブロックの配置ゴーストは、ピン表どおりの形(接続点の数)で出る
  it("shows a ghost shaped by the pin table that is about to be placed", () => {
    const doc = seed();
    const controller = new EditorController(doc);
    controller.setTool("place", "block", { value: "", attrs: { block_pins: TALL_PINS } });
    const ghost = controller.ghostInstance();
    expect(ghost?.attrs.block_pins).toBe(TALL_PINS);
    expect(doc.resolveInstance(ghost!)?.pins).toHaveLength(8);
  });

  // ja: 配置を確定すると、ピン表の属性を持ったブロックが1回のadd_entityで追加される
  it("adds the block with its pin table in a single add_entity", async () => {
    const doc = seed();
    const sent: Command[] = [];
    vi.spyOn(ipc, "executeCommand").mockImplementation(async (command: Command) => {
      sent.push(command);
      return emptyPatch;
    });
    const controller = new EditorController(doc);
    controller.setTool("place", "block", { value: "", attrs: { block_pins: TALL_PINS } });
    controller.onPointerDown(controller.vp.toScreen({ x: 100, y: 100 }), {
      button: 0,
    } as PointerEvent);
    await vi.waitFor(() => expect(sent).toHaveLength(1));
    expect(sent[0]).toMatchObject({
      type: "add_entity",
      entity: {
        kind: "symbol",
        symbol_id: "block",
        reference: "U1",
        attrs: { block_pins: TALL_PINS },
      },
    });
  });
});

describe("picking a symbol on the canvas", () => {
  // ja: 縦長のブロックは外形の中ならどこをクリックしても選べる(固定半径では届かない下端も)
  it("selects a tall block anywhere inside its outline, far from its origin", () => {
    const doc = seed([symbol("u1", "block", { block_pins: TALL_PINS })]);
    const controller = new EditorController(doc);
    expect(controller.hitTest({ x: 100, y: 118 })).toBe("u1");
    expect(controller.hitTest({ x: 105, y: 80 })).toBe("u1");
  });

  // ja: 外形の外をクリックしてもブロックは選ばれない
  it("does not select the block when the click is outside its outline", () => {
    const doc = seed([symbol("u1", "block", { block_pins: TALL_PINS })]);
    const controller = new EditorController(doc);
    expect(controller.hitTest({ x: 100, y: 130 })).toBeNull();
    expect(controller.hitTest({ x: 130, y: 100 })).toBeNull();
  });

  // ja: 小さなシンボルもその外形の中で選べる
  it("selects a small symbol inside its own outline", () => {
    const doc = seed([symbol("s1", "lamp_generic")]);
    const controller = new EditorController(doc);
    expect(controller.hitTest({ x: 100, y: 100 })).toBe("s1");
    expect(controller.hitTest({ x: 104, y: 100 })).toBe("s1");
  });

  // ja: 小さなシンボルから離れたところをクリックしても選ばれない(外形で判定するため)
  it("leaves a small symbol alone when the click is well outside it", () => {
    const doc = seed([symbol("s1", "lamp_generic")]);
    const controller = new EditorController(doc);
    expect(controller.hitTest({ x: 100, y: 108 })).toBeNull();
  });

  // ja: 実機ブロックで表示したコイルは、描かれているブロックの外形の中ならどこをクリックしても選べる (G7)
  it("selects a device-block coil anywhere inside the block it draws, not just the small coil", () => {
    const doc = seed([
      { ...(symbol("k1", "relay_coil") as SymbolInstance & { kind: "symbol" }), reference: "K1", value: "SAMPLE0001" },
      { ...(symbol("k2", "relay_contact_no") as SymbolInstance & { kind: "symbol" }), reference: "K1", at: { x: 200, y: 100 } },
    ]);
    const plain = new EditorController(doc);
    // 分散配置のコイルは小さな外形 (高さ6mm) の中でしか掴めない
    expect(plain.hitTest({ x: 100, y: 106 })).toBeNull();
    // 実機ブロックにすると、ヘッダ2行+接点1行ぶん下まで掴める
    const coil = doc.activeSheet!.entities.k1 as SymbolInstance & { kind: "symbol" };
    coil.attrs = { device_block: "true" };
    const controller = new EditorController(doc);
    const bounds = deviceBlockBounds(coil, doc.sheetAnnotations!.relay.deviceBlocks.get("k1")!);
    expect(bounds.max.y).toBeGreaterThan(106);
    expect(controller.hitTest({ x: 100, y: 106 })).toBe("k1");
    expect(controller.hitTest({ x: 100, y: bounds.max.y - 0.5 })).toBe("k1");
  });

  // ja: 属性が壊れていて描けないブロックも、置いた位置の近くをクリックすれば選べる(直せる)
  it("still lets a block with broken attributes be picked near where it was placed", () => {
    const doc = seed([symbol("u1", "block", { block_pins: "not json" })]);
    const controller = new EditorController(doc);
    expect(doc.resolveInstance(doc.activeSheet!.entities.u1 as never)).toBeUndefined();
    expect(controller.hitTest({ x: 103, y: 103 })).toBe("u1");
  });
});
