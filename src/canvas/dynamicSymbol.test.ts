// Rust側 madake-core/src/symbol.rs の dynamic_symbol と同一座標を返すことを検証する。
import { describe, expect, it } from "vitest";

import type { SymbolDef } from "../ipc";
import { dynamicSymbol, resolveSymbolDef } from "./dynamicSymbol";

describe("dynamicSymbol", () => {
  it("connector_2p は旧静的定義と同一のピン座標", () => {
    const def = dynamicSymbol("connector_2p");
    expect(def).not.toBeNull();
    expect(def!.ref_prefix).toBe("J");
    expect(def!.pins.map((p) => [p.number, p.at.x, p.at.y])).toEqual([
      ["1", 7.5, -2.5],
      ["2", 7.5, 2.5],
    ]);
  });

  it("terminal_block_3p は3端子x左右2接続点、中央揃え・2.5mmグリッド上", () => {
    const def = dynamicSymbol("terminal_block_3p");
    expect(def).not.toBeNull();
    expect(def!.ref_prefix).toBe("TB");
    expect(def!.pins).toHaveLength(6);
    // 端子2(中央)は y=0、左右 x=±2.5
    const t2 = def!.pins.filter((p) => p.number === "2");
    expect(t2.map((p) => [p.at.x, p.at.y]).sort()).toEqual([
      [-2.5, 0],
      [2.5, 0],
    ]);
    for (const p of def!.pins) {
      expect(Math.abs((p.at.x / 2.5) % 1)).toBeCloseTo(0, 9);
      expect(Math.abs((p.at.y / 2.5) % 1)).toBeCloseTo(0, 9);
    }
  });

  it("不正なIDはnull", () => {
    expect(dynamicSymbol("connector_0p")).toBeNull();
    expect(dynamicSymbol("connector_51p")).toBeNull();
    expect(dynamicSymbol("connector_p")).toBeNull();
    expect(dynamicSymbol("terminal_block_xp")).toBeNull();
    expect(dynamicSymbol("resistor")).toBeNull();
  });
});

describe("resolveSymbolDef", () => {
  it("静的Mapを優先し、無ければ動的生成にフォールバックする", () => {
    const staticDef = { id: "resistor" } as SymbolDef;
    const defs = new Map([["resistor", staticDef]]);
    expect(resolveSymbolDef("resistor", defs)).toBe(staticDef);
    expect(resolveSymbolDef("terminal_block_4p", defs)?.pins).toHaveLength(8);
    expect(resolveSymbolDef("unknown", defs)).toBeUndefined();
  });
});
