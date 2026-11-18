import { describe, expect, it } from "vitest";
import type { Point, SimCase, Sheet, SymbolDef, SymbolInstance } from "../ipc";
import { nearestPin, simMenuItems } from "./simMenu";

const wire = { kind: "wire", id: "w1", points: [{ x: 0, y: 0 }, { x: 20, y: 0 }], color: "", sq: 0 };
const sym = {
  kind: "symbol",
  id: "f1",
  symbol_id: "fuse2",
  at: { x: 0, y: 10 },
  rotation: 0,
  mirror: false,
  reference: "F1",
  value: "",
  attrs: {},
} as unknown as SymbolInstance;
const def = {
  id: "fuse2",
  pins: [
    { number: "1", at: { x: 0, y: 0 } },
    { number: "2", at: { x: 10, y: 0 } },
  ],
} as unknown as SymbolDef;
const sheet = {
  id: "s",
  entities: { w1: wire, f1: sym, j1: { kind: "junction", id: "j1", at: { x: 0, y: 0 } } },
} as unknown as Sheet;
const resolve = (i: SymbolInstance) => (i.symbol_id === "fuse2" ? def : undefined);
const caseWith = (faults: SimCase["faults"]): SimCase => ({ id: "c", name: "c", open_switches: [], overrides: {}, faults });
const at = (x: number, y: number): Point => ({ x, y });

describe("simulation right-click menu", () => {
  // ja: 電線の上では「ここで短絡」と「ここで断線」が出る
  it("offers a short and a break on a wire", () => {
    expect(simMenuItems("w1", sheet, resolve, at(5, 0), null)).toEqual([
      { kind: "short_wire", wireId: "w1" },
      { kind: "open_wire", wireId: "w1" },
    ]);
  });

  // ja: ケースに既にその電線の故障があれば「この条件を外す」も出る
  it("also offers to remove a fault the case already has on that wire", () => {
    const short = { kind: "short" as const, target: { wire_id: "w1" } };
    const items = simMenuItems("w1", sheet, resolve, at(5, 0), caseWith([short]));
    expect(items).toContainEqual({ kind: "remove", fault: short });
  });

  // ja: 部品の上では、マウスに一番近いピンで「ピン N で短絡」が出る
  it("offers a short at the pin nearest the pointer on a part", () => {
    expect(nearestPin(sym, def, at(8, 11))).toBe("2");
    expect(simMenuItems("f1", sheet, resolve, at(8, 11), null)).toEqual([{ kind: "short_pin", entityId: "f1", pin: "2" }]);
  });

  // ja: ケースに既にその部品のピンの短絡があれば「この条件を外す」も出る
  it("also offers to remove a short the case already has on that part", () => {
    const short = { kind: "short" as const, target: { entity_id: "f1", pin: "1" } };
    expect(simMenuItems("f1", sheet, resolve, at(1, 10), caseWith([short]))).toContainEqual({ kind: "remove", fault: short });
  });

  // ja: 何も無い所や、電線・部品以外 (接続点など) では何も出さない
  it("offers nothing on empty canvas or on other kinds of entities", () => {
    expect(simMenuItems(null, sheet, resolve, at(50, 50), null)).toEqual([]);
    expect(simMenuItems("j1", sheet, resolve, at(0, 0), null)).toEqual([]);
  });

  // ja: コネクタ接点の上では、ピン番号ではなく極の番号がピンとして選ばれる
  it("picks a connector contact by its pole", () => {
    const contact = { id: "connector_contact", through: true, pins: [{ number: "1", at: { x: -2.5, y: 0 } }, { number: "1", at: { x: 2.5, y: 0 } }] } as unknown as SymbolDef;
    const inst = { ...sym, symbol_id: "connector_contact", value: "5", at: { x: 0, y: 0 } } as unknown as SymbolInstance;
    expect(nearestPin(inst, contact, at(2, 0))).toBe("5");
    expect(nearestPin({ ...inst, value: "" } as SymbolInstance, contact, at(2, 0))).toBe("1");
  });

  // ja: 定義の引けない部品やピンの無い部品では短絡を出さない
  it("offers no pin short for a part without a definition or pins", () => {
    expect(nearestPin(sym, { ...def, pins: [] } as SymbolDef, at(0, 10))).toBeNull();
    expect(simMenuItems("f1", sheet, () => undefined, at(0, 10), null)).toEqual([]);
  });
});
