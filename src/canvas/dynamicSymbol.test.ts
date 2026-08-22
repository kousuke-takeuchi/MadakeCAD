// Rust側 madake-core/src/symbol.rs の dynamic_symbol と同一座標を返すことを検証する。
import { describe, expect, it } from "vitest";

import type { SymbolDef } from "../ipc";
import { dynamicSymbol, resolveSymbolDef } from "./dynamicSymbol";

describe("dynamicSymbol", () => {
  // ja: connector_2pは旧静的定義と同一のピン座標を持つ
  it("connector_2p has the same pin coordinates as the legacy static definition", () => {
    const def = dynamicSymbol("connector_2p");
    expect(def).not.toBeNull();
    expect(def!.ref_prefix).toBe("J");
    expect(def!.pins.map((p) => [p.number, p.at.x, p.at.y])).toEqual([
      ["1", 7.5, -2.5],
      ["2", 7.5, 2.5],
    ]);
  });

  // ja: terminal_block_3pは3端子×左右2接続点で、中央揃え・2.5mmグリッド上にある
  it("terminal_block_3p has 3 terminals with left/right points, centered on the 2.5 mm grid", () => {
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

  // ja: PLC入力モジュールは点数ぶんの接続点が左側に5mmピッチで並ぶ縦長の箱になる
  it("a PLC input module has one connection point per I/O point on its left side", () => {
    const def = dynamicSymbol("plc_di_16p");
    expect(def).not.toBeNull();
    expect(def!.ref_prefix).toBe("PLC");
    expect(def!.category).toBe("plc");
    expect(def!.pins).toHaveLength(16);
    expect(def!.pins.every((p) => p.dir === "left" && p.at.x === -7.5)).toBe(true);
    expect(def!.pins[1].at.y - def!.pins[0].at.y).toBeCloseTo(5, 9);
    // 上下中央揃えで、全ピンが2.5mmグリッド上
    expect(def!.pins.reduce((s, p) => s + p.at.y, 0)).toBeCloseTo(0, 9);
    for (const p of def!.pins) expect(Math.abs((p.at.y / 2.5) % 1)).toBeCloseTo(0, 9);
  });

  // ja: PLCモジュールには入力用と出力用があり、名前で見分けられる
  it("PLC modules come in an input and an output flavour", () => {
    const di = dynamicSymbol("plc_di_8p");
    const dout = dynamicSymbol("plc_do_8p");
    expect(di!.pins).toHaveLength(8);
    expect(dout!.pins).toHaveLength(8);
    expect(dout!.ref_prefix).toBe("PLC");
    expect(di!.name).not.toBe(dout!.name);
    expect(dout!.name_ja).toContain("出力");
  });

  // ja: 不正な動的IDはnullになる
  it("malformed dynamic ids return null", () => {
    expect(dynamicSymbol("connector_0p")).toBeNull();
    expect(dynamicSymbol("connector_51p")).toBeNull();
    expect(dynamicSymbol("connector_p")).toBeNull();
    expect(dynamicSymbol("terminal_block_xp")).toBeNull();
    expect(dynamicSymbol("resistor")).toBeNull();
  });

  // ja: PLCモジュールのシンボルは1〜64点までで、その外の点数は存在しない
  it("PLC module symbols exist from 1 to 64 points only", () => {
    expect(dynamicSymbol("plc_di_1p")).not.toBeNull();
    expect(dynamicSymbol("plc_di_64p")).not.toBeNull();
    expect(dynamicSymbol("plc_di_0p")).toBeNull();
    expect(dynamicSymbol("plc_di_65p")).toBeNull();
    expect(dynamicSymbol("plc_do_xp")).toBeNull();
  });
});

describe("resolveSymbolDef", () => {
  // ja: 静的定義を優先し、無ければ動的生成にフォールバックする
  it("static definitions win; unknown ids fall back to dynamic generation", () => {
    const staticDef = { id: "resistor" } as SymbolDef;
    const defs = new Map([["resistor", staticDef]]);
    expect(resolveSymbolDef("resistor", defs)).toBe(staticDef);
    expect(resolveSymbolDef("terminal_block_4p", defs)?.pins).toHaveLength(8);
    expect(resolveSymbolDef("unknown", defs)).toBeUndefined();
  });
});
