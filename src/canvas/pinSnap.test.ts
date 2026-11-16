// 配線ツールのグリッド外のピンへの吸着
import { describe, expect, it } from "vitest";

import type { Sheet, SymbolDef, SymbolInstance } from "../ipc";
import { offGridPinNear } from "./pinSnap";

const def: SymbolDef = {
  id: "user_free",
  name: "Free",
  name_ja: "自由ピン",
  category: "user",
  ref_prefix: "CN",
  primitives: [],
  pins: [
    { number: "1", name: "", at: { x: -1.95, y: 3.3 }, dir: "left" },
    { number: "2", name: "", at: { x: 5, y: 0 }, dir: "right" },
  ],
  free_pins: true,
};
const inst = { kind: "symbol", id: "i1", symbol_id: "user_free", at: { x: 120, y: 60 }, rotation: 0, mirror: false, reference: "CN1", value: "", attrs: {} };
const sheet = { entities: { i1: inst } } as unknown as Sheet;
const resolve = (_: SymbolInstance) => def;

describe("snapping the wire tool to off-grid pins", () => {
  // ja: カーソルの近く (1.25mm 以内) にグリッド外のピンがあれば、そのピンちょうどの位置を返す。配線はピンにつながる
  it("returns the exact place of an off-grid pin near the cursor", () => {
    const hit = offGridPinNear(sheet, resolve, { x: 117.5, y: 63.0 });
    expect(hit?.x).toBeCloseTo(118.05, 9);
    expect(hit?.y).toBeCloseTo(63.3, 9);
  });

  // ja: 1.25mm より遠いとき、グリッド上のピンの近くのとき、シートが無いときは何も返さない (グリッドへ吸着する)
  it("returns nothing when no off-grid pin is near, for a pin on the grid, or without a sheet", () => {
    expect(offGridPinNear(sheet, resolve, { x: 115, y: 63.3 })).toBeNull();
    expect(offGridPinNear(sheet, resolve, { x: 125.2, y: 60.1 })).toBeNull();
    expect(offGridPinNear(null, resolve, { x: 118, y: 63 })).toBeNull();
  });

  // ja: グリッド外のピンが2つ近くにあれば、近いほうを返す
  it("picks the nearer of two off-grid pins", () => {
    const two = { entities: { i1: inst, i2: { ...inst, id: "i2", at: { x: 120, y: 61 } } } } as unknown as Sheet;
    const hit = offGridPinNear(two, resolve, { x: 118.0, y: 63.4 });
    expect(hit?.y).toBeCloseTo(63.3, 9);
  });
});
