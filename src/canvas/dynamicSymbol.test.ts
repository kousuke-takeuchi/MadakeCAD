// Rust側 madake-core/src/symbol.rs の dynamic_symbol と同一座標を返すことを検証する。
import { describe, expect, it } from "vitest";

import type { SymbolDef, SymbolInstance } from "../ipc";
import {
  blockSymbol,
  dynamicSymbol,
  resolveInstanceDef,
  pinGroupPaperBoxes,
  resolveSymbolDef,
  symbolBounds,
} from "./dynamicSymbol";

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

  // ja: 多段端子台(4極2段)は段記号付きのピン番号(1a,1b,…4b)を持ち、1端子=左右2接続点になる
  it("a multi-level terminal block (4 poles, 2 levels) numbers its pins with a level letter", () => {
    const def = dynamicSymbol("terminal_block_4p_2l");
    expect(def).not.toBeNull();
    expect(def!.ref_prefix).toBe("TB");
    expect(def!.pins).toHaveLength(4 * 2 * 2);
    const numbers = new Set(def!.pins.map((p) => p.number));
    expect([...numbers].sort()).toEqual(["1a", "1b", "2a", "2b", "3a", "3b", "4a", "4b"]);
    for (const no of numbers) {
      const same = def!.pins.filter((p) => p.number === no);
      expect(same).toHaveLength(2);
      expect(same[1].at.x - same[0].at.x).toBeCloseTo(5, 9);
      expect(same[0].at.y).toBeCloseTo(same[1].at.y, 9);
    }
  });

  // ja: 多段端子台は段ごとに1列の端子が横に並び、同じ極の端子(1aと1b)は同じ行に載る(Rust側と同じ座標)
  it("draws one terminal column per level, with the same pole of every level on one row", () => {
    const def = dynamicSymbol("terminal_block_4p_2l")!;
    expect(def.primitives.filter((p) => p.type === "rect")).toHaveLength(2);
    const at = (no: string) => def.pins.filter((p) => p.number === no).map((p) => p.at);
    // 段aの列は中心x=-5 (接続点 -7.5/-2.5)、段bの列は中心x=+5 (接続点 +2.5/+7.5)
    expect(at("1a").map((p) => p.x)).toEqual([-7.5, -2.5]);
    expect(at("1b").map((p) => p.x)).toEqual([2.5, 7.5]);
    expect(at("1a")[0].y).toBeCloseTo(at("1b")[0].y, 9);
    expect(at("2a")[0].y - at("1a")[0].y).toBeCloseTo(5, 9);
  });

  // ja: 多段端子台の接続点はすべて2.5mmグリッド上にあり、別の端子どうしが同じ点を共有しない
  it("keeps every connection point on the 2.5 mm grid and never lets two terminals share one", () => {
    for (const levels of [1, 2, 3]) {
      const def = dynamicSymbol(`terminal_block_4p_${levels}l`)!;
      const seen = new Map<string, string>();
      for (const pin of def.pins) {
        expect(Math.abs((pin.at.x / 2.5) % 1)).toBeCloseTo(0, 9);
        expect(Math.abs((pin.at.y / 2.5) % 1)).toBeCloseTo(0, 9);
        const key = `${pin.at.x},${pin.at.y}`;
        expect(seen.get(key) ?? pin.number).toBe(pin.number);
        seen.set(key, pin.number);
      }
    }
  });

  // ja: 段数が0や4(1〜3の範囲外)の端子台は、極数が範囲外のときと同様にnullになる
  it("a terminal block with a level count outside 1-3 returns null", () => {
    expect(dynamicSymbol("terminal_block_8p_0l")).toBeNull();
    expect(dynamicSymbol("terminal_block_8p_4l")).toBeNull();
  });

  // ja: 単段の端子台id(terminal_block_{n}p)は多段対応の追加後も数字だけのピン番号のままで、既存プロジェクトとの互換を保つ
  it("the plain single-level terminal block id keeps its numeric-only pin numbers", () => {
    const def = dynamicSymbol("terminal_block_8p");
    expect(def).not.toBeNull();
    expect(def!.pins.every((p) => /^[0-9]+$/.test(p.number))).toBe(true);
    expect(new Set(def!.pins.map((p) => p.number)).size).toBe(8);
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

// Rust側 madake-core/src/block.rs と同一座標を返すことを検証する。
describe("blockSymbol", () => {
  const SAMPLE =
    '{"left":[{"no":"IN1","name":"入力1"},{"no":"IN2","name":"入力2"}],"right":[{"no":"OUT1","name":"出力1"}]}';

  const rectOf = (def: SymbolDef) => {
    const r = def.primitives.find((p) => p.type === "rect");
    if (!r || r.type !== "rect") throw new Error("外形の長方形が無い");
    return r;
  };
  const textAt = (def: SymbolDef, needle: string) => {
    const t = def.primitives.find((p) => p.type === "text" && p.text === needle);
    if (!t || t.type !== "text") throw new Error(`文字 ${needle} が描かれていない`);
    return t.at;
  };

  // ja: ブロックのピンは箱の左右に5mmピッチ・2.5mmグリッド上で中央揃えに並ぶ(Rust側と同じ座標)
  it("puts named pins on both sides of the box, 5 mm apart on the 2.5 mm grid", () => {
    const def = blockSymbol({ block_pins: SAMPLE });
    const rect = rectOf(def);
    expect([rect.p1.x, rect.p1.y, rect.p2.x, rect.p2.y]).toEqual([-10, -7.5, 10, 7.5]);
    expect(def.pins.map((p) => [p.number, p.at.x, p.at.y, p.dir])).toEqual([
      ["IN1", -12.5, -2.5, "left"],
      ["IN2", -12.5, 2.5, "left"],
      ["OUT1", 12.5, 0, "right"],
    ]);
    expect(def.ref_prefix).toBe("U");
    expect(def.category).toBe("block");
  });

  // ja: ピン番号は外形の外側に、ピン名は内側に描かれる
  it("draws pin numbers outside the outline and pin names inside it", () => {
    const def = blockSymbol({ block_pins: SAMPLE });
    const rect = rectOf(def);
    expect(textAt(def, "IN1").x).toBeLessThan(rect.p1.x);
    expect(textAt(def, "入力1").x).toBeGreaterThan(rect.p1.x);
    expect(textAt(def, "入力1").x).toBeLessThan(rect.p2.x);
    expect(textAt(def, "入力1").y).toBe(-2.5);
    expect(textAt(def, "OUT1").x).toBeGreaterThan(rect.p2.x);
  });

  // ja: 外形はピン数とピン名の長さから自動で決まり、必ず5mm刻みになる
  it("sizes the box automatically from the pin count and the name length, in 5 mm steps", () => {
    const many = blockSymbol({
      block_pins: '{"left":[{"no":"1"},{"no":"2"},{"no":"3"},{"no":"4"},{"no":"5"},{"no":"6"}]}',
    });
    const r = rectOf(many);
    expect([r.p2.x - r.p1.x, r.p2.y - r.p1.y]).toEqual([15, 35]);

    const wide = rectOf(blockSymbol({ block_pins: '{"left":[{"no":"1","name":"MOTOR-DRIVE-ENABLE"}]}' }));
    expect(wide.p2.x - wide.p1.x).toBe(30);
  });

  // ja: 外形寸法を明示すると5mm刻みへ切り上げられる
  it("rounds an explicit size up to the 5 mm step", () => {
    const def = blockSymbol({ block_pins: '{"left":[{"no":"1"}]}', block_size: '{"w":18,"h":11}' });
    const r = rectOf(def);
    expect([r.p2.x - r.p1.x, r.p2.y - r.p1.y]).toEqual([20, 15]);
  });

  // ja: 番号の重複・空番号・小さすぎる寸法・壊れたピン表はエラーになる(黙って描かない)
  it("rejects duplicate or empty pin numbers, a size too small, and a malformed pin table", () => {
    expect(() => blockSymbol({ block_pins: '{"left":[{"no":"A"}],"right":[{"no":"A"}]}' })).toThrow(
      /duplicate/,
    );
    expect(() => blockSymbol({ block_pins: '{"left":[{"no":" "}]}' })).toThrow(/empty/);
    expect(() =>
      blockSymbol({
        block_pins: '{"left":[{"no":"1"},{"no":"2"},{"no":"3"},{"no":"4"}]}',
        block_size: '{"w":20,"h":15}',
      }),
    ).toThrow(/too small/);
    expect(() => blockSymbol({ block_pins: "not json" })).toThrow(/not valid JSON/);
    expect(() => blockSymbol({ block_pins: '{"lef":[{"no":"1"}]}' })).toThrow(/valid JSON/);
  });

  // ja: ピン表の無いブロックは最小サイズの空の箱になる
  it("makes an empty minimum-sized box when there is no pin table", () => {
    const def = blockSymbol({});
    expect(def.pins).toHaveLength(0);
    const r = rectOf(def);
    expect([r.p2.x - r.p1.x, r.p2.y - r.p1.y]).toEqual([15, 10]);
  });
});

describe("resolveInstanceDef", () => {
  const inst = (attrs: Record<string, string>): SymbolInstance => ({
    id: "e1",
    symbol_id: "block",
    at: { x: 100, y: 50 },
    rotation: 0,
    mirror: false,
    reference: "CTRL1",
    value: "MainController",
    attrs,
  });

  // ja: 配置済みブロックの定義はインスタンスの属性から作られる(同じidでも中身が違う)
  it("builds a block's definition from the instance attributes, not from its id", () => {
    const defs = new Map<string, SymbolDef>();
    const a = resolveInstanceDef(inst({ block_pins: '{"left":[{"no":"IN1"}]}' }), defs);
    const b = resolveInstanceDef(inst({ block_pins: '{"left":[{"no":"A"},{"no":"B"}]}' }), defs);
    expect(a?.pins.map((p) => p.number)).toEqual(["IN1"]);
    expect(b?.pins.map((p) => p.number)).toEqual(["A", "B"]);
  });

  // ja: 属性が壊れているブロックは描かれない(undefinedになる)
  it("returns nothing for a block whose attributes are broken", () => {
    expect(resolveInstanceDef(inst({ block_pins: "not json" }), new Map())).toBeUndefined();
  });
});

describe("symbolBounds", () => {
  function tb(rotation: number, mirror: boolean, attrs: Record<string, string>): SymbolInstance {
    return {
      id: "tb1",
      symbol_id: "terminal_block_8p",
      at: { x: 150, y: 100 },
      rotation,
      mirror,
      reference: "TB1",
      value: "",
      attrs,
    } as SymbolInstance;
  }

  // ja: ピングループ括弧はシンボルと一緒に描かれるので、参照記号・型番の位置決めに使う外形にはその箱も入る (Rust側 svg::symbol_bounds とparity)
  it("grows to take in a pin group's bracket and caption, so labels are placed clear of them", () => {
    const def = dynamicSymbol("terminal_block_8p")!;
    const plain = symbolBounds(tb(0, false, {}), def);
    const grouped = symbolBounds(tb(0, false, { pin_groups: "1-4:電源系統一式" }), def);
    // 左側のピンに掛かるので、外形は左へだけ広がる
    expect(grouped.min.x).toBeLessThan(plain.min.x);
    expect(grouped.max.x).toBe(plain.max.x);
  });

  // ja: 実機ブロック表示のコイルは、ピングループの属性を持っていても括弧のための余白を取らない (そこには括弧を描かないため。Rust側 a_device_block_coil_reserves_no_room_for_brackets_it_never_draws とparity)
  it("reserves no room on a device-block coil for brackets it never draws", () => {
    const def = dynamicSymbol("terminal_block_8p")!;
    const coil = (attrs: Record<string, string>): SymbolInstance =>
      ({
        id: "k1",
        symbol_id: "terminal_block_8p",
        at: { x: 150, y: 100 },
        rotation: 0,
        mirror: false,
        reference: "K1",
        value: "",
        attrs,
      }) as SymbolInstance;
    const without = symbolBounds(coil({ device_block: "true" }), def);
    const plain = symbolBounds(coil({ pin_groups: "1-4:電源系統一式" }), def);
    const asBlock = symbolBounds(coil({ device_block: "true", pin_groups: "1-4:電源系統一式" }), def);
    expect(plain).not.toEqual(without); // 普通なら括弧のぶん広がる
    expect(asBlock).toEqual(without);
  });

  // ja: 回転しても同じで、90度回すと括弧は上に出るので外形は上へ広がる (回転したシンボルで参照記号が括弧の文字に重ならない)
  it("grows upward instead when the symbol is turned 90 degrees, keeping labels off the caption", () => {
    const def = dynamicSymbol("terminal_block_8p")!;
    const plain = symbolBounds(tb(90, false, {}), def);
    const grouped = symbolBounds(tb(90, false, { pin_groups: "1-4:電源系統一式" }), def);
    expect(grouped.min.y).toBeLessThan(plain.min.y);
    expect(grouped.min.x).toBe(plain.min.x);
  });
});

describe("captionLineAnchors", () => {
  const SPEC = "1-4:電源系統一式予備回路"; // 4+4+2文字 → 3行

  function boxes(rotation: number, mirror: boolean): [number, number, number, number][] {
    const inst = {
      id: "tb1",
      symbol_id: "terminal_block_8p",
      at: { x: 150, y: 100 },
      rotation,
      mirror,
      reference: "TB1",
      value: "",
      attrs: { pin_groups: SPEC },
    } as SymbolInstance;
    return pinGroupPaperBoxes(inst, dynamicSymbol("terminal_block_8p")!).map(([a, b]) => [
      a.x,
      a.y,
      b.x,
      b.y,
    ]);
  }

  const overlaps = (
    a: [number, number, number, number],
    b: [number, number, number, number],
  ) => a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3];

  // ja: ピングループの部品どうし (括弧と、折り返したキャプションの各行) は全ての回転・ミラーで決して重ならない (Rust側 svg_pin_group_pieces_never_overlap_each_other_at_any_rotation とparity)
  it("never lets a bracket and its caption lines overlap each other, at any rotation", () => {
    for (const rotation of [0, 90, 180, 270]) {
      for (const mirror of [false, true]) {
        const bs = boxes(rotation, mirror);
        expect(bs).toHaveLength(4); // 括弧1つ+キャプション3行
        for (let i = 0; i < bs.length; i++) {
          for (let j = i + 1; j < bs.length; j++) {
            expect(overlaps(bs[i], bs[j])).toBe(false);
          }
        }
      }
    }
  });

  // ja: 文字はシンボルを回しても横書きのままなので、キャプションの行は用紙の上で必ず縦に積まれる (同じ中心線に並び、順に下へ下がる。Rust側 svg_pin_group_caption_lines_stack_downwards_on_paper とparity)
  it("stacks caption lines downwards on the paper, sharing one centre line, at any rotation", () => {
    for (const rotation of [0, 90, 180, 270]) {
      for (const mirror of [false, true]) {
        const captions = boxes(rotation, mirror).slice(1);
        const cx = (b: [number, number, number, number]) => (b[0] + b[2]) / 2;
        for (const c of captions) expect(cx(c)).toBeCloseTo(cx(captions[0]), 9);
        for (let i = 0; i + 1 < captions.length; i++) {
          expect(captions[i][3]).toBeLessThanOrEqual(captions[i + 1][1] + 1e-9);
        }
      }
    }
  });
});
