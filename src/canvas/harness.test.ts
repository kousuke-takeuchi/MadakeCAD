// ハーネス境界の幾何計算とコマンド組み立ての仕様
// (docs/internal/specs/m2-drawing-parity.md §3)。Rust側 madake-core/src/harness.rs と同一ルール。
import { describe, expect, it } from "vitest";

import type { Entity, Sheet } from "../ipc";
import {
  harnessAddCommand,
  harnessBounds,
  harnessLabelAt,
  harnessOfWire,
  harnessRectPoints,
  harnessWireCount,
  nextHarnessName,
} from "./harness";

function sheetWith(entities: Entity[]): Sheet {
  return {
    id: "sheet-1",
    name: "Sheet1",
    size: "A3",
    orientation: "Landscape",
    zone_cols: 4,
    zone_rows: 6,
    title_block: {},
    revisions: [],
    entities: Object.fromEntries(entities.map((e) => [e.id, e])),
  };
}

function harness(id: string, name: string, x0: number, y0: number, x1: number, y1: number): Entity {
  return {
    kind: "harness",
    id,
    points: harnessRectPoints({ x: x0, y: y0 }, { x: x1, y: y1 }),
    name,
    note: "",
  };
}

function wire(id: string, points: [number, number][]): Extract<Entity, { kind: "wire" }> {
  return {
    kind: "wire",
    id,
    points: points.map(([x, y]) => ({ x, y })),
    color: "red",
    sq: 0.75,
    length_m: null,
    part_no: null,
    net: null,
  };
}

describe("harness geometry", () => {
  // ja: 矩形ドラッグはどの向きに引いても同じ4隅の頂点になる
  it("normalizes a rectangle drag into the same four corners in any direction", () => {
    const forward = harnessRectPoints({ x: 10, y: 20 }, { x: 50, y: 40 });
    const backward = harnessRectPoints({ x: 50, y: 40 }, { x: 10, y: 20 });
    expect(forward).toEqual(backward);
    expect(forward).toEqual([
      { x: 10, y: 20 },
      { x: 50, y: 20 },
      { x: 50, y: 40 },
      { x: 10, y: 40 },
    ]);
  });

  // ja: 全ての点が囲みの内側にある配線だけがハーネスに所属する(境界線上は内側)
  it("assigns a wire to a harness only when all of its points are inside (the boundary counts as inside)", () => {
    const h = harness("h1", "W1", 50, 50, 150, 100);
    const inside = wire("w1", [[60, 60], [140, 60]]);
    const onBoundary = wire("w2", [[50, 50], [150, 100]]);
    const partly = wire("w3", [[60, 60], [200, 60]]);
    const outside = wire("w4", [[200, 200], [250, 200]]);
    const sheet = sheetWith([h, inside, onBoundary, partly, outside]);
    expect(harnessOfWire(sheet, inside)?.name).toBe("W1");
    expect(harnessOfWire(sheet, onBoundary)?.name).toBe("W1");
    expect(harnessOfWire(sheet, partly)).toBeNull();
    expect(harnessOfWire(sheet, outside)).toBeNull();
  });

  // ja: 入れ子の囲みでは、その配線を囲む最も小さいハーネスに所属する
  it("prefers the smallest enclosing harness when boundaries are nested", () => {
    const outer = harness("h1", "W1", 50, 50, 200, 150);
    const inner = harness("h2", "W2", 55, 55, 80, 70);
    const w = wire("w1", [[60, 60], [70, 60]]);
    expect(harnessOfWire(sheetWith([outer, inner, w]), w)?.name).toBe("W2");
  });

  // ja: ハーネスの「含む電線」本数は、その囲みが今いくつの配線を囲んでいるかを表す
  it("counts how many wires a harness currently encloses", () => {
    const h = harness("h1", "W1", 50, 50, 150, 100);
    const sheet = sheetWith([
      h,
      wire("w1", [[60, 60], [140, 60]]),
      wire("w2", [[60, 80], [140, 80]]),
      wire("w3", [[200, 200], [250, 200]]),
    ]);
    expect(harnessWireCount(sheet, "h1")).toBe(2);
  });

  // ja: 囲みの外接矩形は左上と右下の点を返し、頂点が無ければnullになる
  it("returns the bounding box of a harness and null when it has no points", () => {
    expect(harnessBounds(harnessRectPoints({ x: 50, y: 100 }, { x: 150, y: 60 }))).toEqual({
      min: { x: 50, y: 60 },
      max: { x: 150, y: 100 },
    });
    expect(harnessBounds([])).toBeNull();
  });

  // ja: 名前ラベルは囲みの左上角の外側(右へ1mm・上へ1mm)に置く
  it("places the name label just outside the top-left corner of the boundary", () => {
    expect(harnessLabelAt(harnessRectPoints({ x: 50, y: 50 }, { x: 150, y: 100 }))).toEqual({
      x: 51,
      y: 49,
    });
  });
});

describe("harness naming", () => {
  // ja: 新しいハーネスの名前は、何も無いシートではW1、既にあるときはその次の番号になる
  it("suggests W1 on an empty sheet and continues the W series afterwards", () => {
    expect(nextHarnessName(sheetWith([]))).toBe("W1");
    expect(
      nextHarnessName(sheetWith([harness("h1", "W1", 0, 0, 10, 10), harness("h2", "W3", 20, 0, 30, 10)])),
    ).toBe("W4");
  });

  // ja: ハーネス以外のエンティティ(参照記号など)は名前の採番に影響しない
  it("ignores non-harness entities when suggesting the next name", () => {
    const symbol: Entity = {
      kind: "symbol",
      id: "s1",
      symbol_id: "relay_coil",
      at: { x: 0, y: 0 },
      rotation: 0,
      mirror: false,
      reference: "W9",
      value: "",
      attrs: {},
    };
    expect(nextHarnessName(sheetWith([symbol]))).toBe("W1");
  });
});

describe("harnessAddCommand", () => {
  // ja: 矩形ドラッグはadd_entityコマンド1回(kind=harness・自動採番した名前)になる
  it("builds one add_entity command with the auto-suggested name", () => {
    const sheet = sheetWith([harness("h1", "W1", 0, 0, 10, 10)]);
    const command = harnessAddCommand(sheet, { x: 50, y: 100 }, { x: 150, y: 60 });
    expect(command).not.toBeNull();
    expect(command?.type).toBe("add_entity");
    if (command?.type !== "add_entity") throw new Error("add_entity");
    expect(command.sheet_id).toBe("sheet-1");
    expect(command.entity.kind).toBe("harness");
    if (command.entity.kind !== "harness") throw new Error("harness");
    expect(command.entity.name).toBe("W2");
    expect(command.entity.note).toBe("");
    expect(command.entity.id).toMatch(/[0-9a-f-]{36}/);
    expect(command.entity.points).toEqual([
      { x: 50, y: 60 },
      { x: 150, y: 60 },
      { x: 150, y: 100 },
      { x: 50, y: 100 },
    ]);
  });

  // ja: つぶれた矩形(クリックしただけ)ではハーネスを作らない
  it("creates nothing for a degenerate rectangle (a plain click)", () => {
    const sheet = sheetWith([]);
    expect(harnessAddCommand(sheet, { x: 50, y: 50 }, { x: 50, y: 50 })).toBeNull();
    expect(harnessAddCommand(sheet, { x: 50, y: 50 }, { x: 50.2, y: 60 })).toBeNull();
  });
});
