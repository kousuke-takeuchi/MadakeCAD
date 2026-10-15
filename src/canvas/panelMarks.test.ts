// 盤の銘板・関連付き寸法線 (M4 §8) の画面側の仕様テスト。形の規則はRust側 panel.rs と同じ。
import { describe, expect, it } from "vitest";
import type { Dimension, Entity, Nameplate, Project, Sheet } from "../ipc";
import {
  deviceDescriptions,
  dimensionAddCommand,
  dimensionGeometry,
  footprintAt,
  nameplateLines,
  nameplateRect,
  panelMarkHit,
} from "./panelMarks";

const k1: Entity = { kind: "footprint", id: "k1", at: { x: 100, y: 50 }, w: 40, h: 60, rotation: 0, reference: "K1", part_no: "", item_no: null };
const k2: Entity = { kind: "footprint", id: "k2", at: { x: 185, y: 50 }, w: 40, h: 60, rotation: 0, reference: "K2", part_no: "", item_no: null };

function sheet(extra: Entity[] = []): Sheet {
  const entities: Record<string, Entity> = { k1, k2 };
  for (const e of extra) entities[e.id] = e;
  return { id: "p1", name: "盤", kind: "panel", entities } as unknown as Sheet;
}

function plate(over: Partial<Nameplate> = {}): Nameplate {
  return { id: "n1", footprint: "k1", at: { x: 300, y: 200 }, w: 30, h: 10, lines: ["主回路"], text_mm: 3.5, position: "below", follow_description: false, part_no: "", ...over };
}

describe("盤の銘板", () => {
  // ja: 部品に結び付いた銘板は部品の下に中央揃えで付き、指定すれば上に付く。単独の銘板は置いた位置のまま (Rust側と同じ)
  it("sits centred under its footprint, above it when asked, and a standalone one stays put", () => {
    expect(nameplateRect(sheet(), plate())).toEqual({ x: 105, y: 112, w: 30, h: 10 });
    expect(nameplateRect(sheet(), plate({ position: "above" }))!.y).toBe(38);
    expect(nameplateRect(sheet(), plate({ footprint: null }))).toEqual({ x: 300, y: 200, w: 30, h: 10 });
    expect(nameplateRect(sheet(), plate({ footprint: "missing" }))).toBeNull();
  });

  // ja: 説明に追従する銘板は1行目が回路図の説明になる (説明は回路図シンボルの機能ラベルから集める)
  it("takes the schematic description as its first line when it follows it", () => {
    const project = {
      sheets: [{ id: "s1", entities: { r: { kind: "symbol", id: "r", reference: "K1", attrs: { function_label: "主回路接触器" } } } }],
    } as unknown as Project;
    const d = deviceDescriptions(project);
    expect(d.get("K1")).toBe("主回路接触器");
    expect(nameplateLines(sheet(), plate({ lines: ["旧", "200V"], follow_description: true }), d)).toEqual(["主回路接触器", "200V"]);
    expect(nameplateLines(sheet(), plate({ lines: ["旧"] }), d)).toEqual(["旧"]);
  });
});

describe("盤の寸法線", () => {
  const dim: Dimension = { id: "d1", a: { kind: "footprint", id: "k1" }, b: { kind: "footprint", id: "k2" }, axis: "horizontal", at: 40 };

  // ja: 2つの部品の間の寸法は中心間の水平距離を測り、線は指定の高さに引く
  it("measures the horizontal distance between two footprint centres", () => {
    const g = dimensionGeometry(sheet(), dim)!;
    expect(g.text).toBe("85");
    expect(g.line).toEqual([{ x: 120, y: 40 }, { x: 205, y: 40 }]);
  });

  // ja: 寸法ツールは2点の距離が大きい向きの寸法を、端点の外側 (水平なら上) 10mmに置き、同じ点なら何も作らない
  it("places a dimension along the larger distance, 10mm outside, and nothing for the same point", () => {
    const command = dimensionAddCommand(sheet(), { kind: "footprint", id: "k1" }, { kind: "footprint", id: "k2" });
    expect(command).toMatchObject({ type: "add_entity", entity: { kind: "dimension", axis: "horizontal", at: 70 } });
    expect(dimensionAddCommand(sheet(), { kind: "point", at: { x: 0, y: 0 } }, { kind: "point", at: { x: 0, y: 0 } })).toBeNull();
  });

  // ja: クリックした位置の部品を見つけ、寸法線は線の近く、銘板は外形の中で掴める
  it("finds the footprint under a click and picks dimensions and nameplates", () => {
    expect(footprintAt(sheet(), { x: 110, y: 60 })?.id).toBe("k1");
    expect(footprintAt(sheet(), { x: 0, y: 0 })).toBeNull();
    const s = sheet([{ kind: "dimension", ...dim }, { kind: "nameplate", ...plate() }]);
    expect(panelMarkHit(s, s.entities.d1, { x: 150, y: 40.5 }, 1)).toBe(true);
    expect(panelMarkHit(s, s.entities.n1, { x: 120, y: 115 }, 1)).toBe(true);
    expect(panelMarkHit(s, s.entities.n1, { x: 0, y: 0 }, 1)).toBe(false);
  });
});
