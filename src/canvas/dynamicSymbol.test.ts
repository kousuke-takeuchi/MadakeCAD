// Rust側 madake-core/src/symbol.rs の dynamic_symbol と同一座標を返すことを検証する。
import { describe, expect, it } from "vitest";

import type { SymbolDef, SymbolInstance } from "../ipc";
import {
  blockBox,
  blockSymbol,
  captionAnchor,
  captionBounds,
  CORNER_CAPTION_GAP,
  dynamicSymbol,
  parseBlockSpec,
  resolveInstanceDef,
  pinGroupPaperBoxes,
  resolveSymbolDef,
  symbolBounds,
  transformLocal,
} from "./dynamicSymbol";
import { symbolTopY } from "./renderer";
import { FUNCTION_LABEL_FONT, functionLabelFallbackAt, LABEL_FONT, REF_LABEL_DY, VALUE_LABEL_DY } from "./textStyle";
import { textWidthMm } from "./textWidth";
import golden from "../../src-tauri/crates/madake-core/tests/golden/connector-blocks.json";

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

  // ja: PLCモジュールの分割片 plc_di_{n}p_s{start} は点番号を開始点から振り、形は同じ点数のモジュールと同じ (Rust側と一致)
  it("a split PLC module piece numbers its points from the start point and keeps the shape of a module of that size", () => {
    const piece = dynamicSymbol("plc_di_4p_s13")!;
    expect(piece.pins.map((p) => p.number)).toEqual(["13", "14", "15", "16"]);
    expect(piece.pins.map((p) => p.at)).toEqual(dynamicSymbol("plc_di_4p")!.pins.map((p) => p.at));
    expect(dynamicSymbol("plc_do_8p_s57")).not.toBeNull();
    for (const bad of ["plc_di_8p_s58", "plc_di_4p_s0", "plc_di_4p_sx"]) expect(dynamicSymbol(bad)).toBeNull();
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

  // ja: 多段端子台の接続点はすべて2.5mmグリッド上にあり、別の端子どうしが同じ点を共有しない (段数1は`_1l`という別名idを作らないので対象外)
  it("keeps every connection point on the 2.5 mm grid and never lets two terminals share one", () => {
    for (const levels of [2, 3]) {
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

  // ja: `terminal_block_8p_1l`は`terminal_block_8p`の別名として受け付けずnullになる (Task 3。単段端子台のsymbol_idの綴りは1通りだけ、Rust側resolve_symbolと同じ規則)
  it("rejects the one-level alias spelling of a single-level terminal block", () => {
    expect(dynamicSymbol("terminal_block_8p_1l")).toBeNull();
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

  // ja: 端子番号を採番したあとでも、ピングループの箱は外形に入ったままになる (範囲は表示番号と突き合わせられるため。Rust側 pin_group_paper_boxes とparity)
  it("still takes in a pin group's box after the terminals are renumbered", () => {
    const def = dynamicSymbol("terminal_block_8p")!;
    const plain = symbolBounds(tb(0, false, {}), def);
    const renumbered = symbolBounds(
      tb(0, false, { pin_groups: "11-14:電源系統一式", terminal_numbers: "11,12,13,14,15,16,17,18" }),
      def,
    );
    expect(renumbered.min.x).toBeLessThan(plain.min.x);
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

  // ja: ブロックのピンの接点は Rust と同じく空白を落として PinDef に入り、1つだけの接点や重なる接点は同じ理由で断る
  it("copies block pin contacts into the pin definition and refuses bad contacts like Rust", () => {
    const def = blockSymbol({ block_pins: '{"right":[{"no":"U3","contacts":[" VBUS ","D-","D+","GND"]}],"left":[{"no":"1"}]}' });
    expect(def.pins.find((p) => p.number === "U3")!.contacts).toEqual(["VBUS", "D-", "D+", "GND"]);
    expect(def.pins.find((p) => p.number === "1")!.contacts).toBeUndefined();
    expect(() => blockSymbol({ block_pins: '{"left":[{"no":"P1","contacts":["A"]}]}' })).toThrow("at least two contacts");
    expect(() => blockSymbol({ block_pins: '{"left":[{"no":"P1","contacts":["A","A"]}]}' })).toThrow("duplicate contact A");
  });
});

describe("blockSymbol connectors", () => {
  const message = (json: string): string => {
    try {
      blockSymbol({ block_pins: json });
    } catch (e) {
      return (e as Error).message;
    }
    throw new Error(`断らなかった: ${json}`);
  };

  // ja: ピンはコネクタの形を接点と一緒に持て、null は書かなかったのと同じ。知らない種類は Rust と同じく壊れた JSON として断る
  it("reads a connector kind, treats null as absent and refuses an unknown kind as invalid JSON, like Rust", () => {
    expect(() =>
      blockSymbol({ block_pins: '{"top":[{"no":"U1","connector":"usb_a_f","contacts":["VBUS","D-","D+","GND"]}]}' }),
    ).not.toThrow();
    const nulls = blockSymbol({ block_pins: '{"left":[{"no":"1","connector":null,"connector_ref":null}]}' });
    expect(nulls.primitives[0].type).toBe("rect");
    expect(message('{"left":[{"no":"1","connector":"usb_z"}]}')).toMatch(/^block_pins is not valid JSON/);
  });

  // ja: コネクタの名前の誤りは Rust と同じ文で断り (空・コネクタなし・まとめない種類・2つのまとまり)、名前の無いコネクタは繰り返してよい
  it("refuses a bad connector name with the same messages as Rust and lets unnamed connectors repeat", () => {
    expect(message('{"left":[{"no":"P1","connector":"screw","connector_ref":"  "}]}')).toBe(
      "block pin P1: connector_ref must not be empty",
    );
    expect(message('{"left":[{"no":"P1","connector_ref":"J3"}]}')).toBe(
      "block pin P1: connector_ref needs a grouped connector (screw, jst_ph_f, jst_xh_f)",
    );
    expect(message('{"left":[{"no":"P1","connector":"usb_a_f","connector_ref":"J3"}]}')).toBe(
      "block pin P1: connector_ref is only for screw, jst_ph_f and jst_xh_f, not usb_a_f",
    );
    const split = "block pin 2: connector_ref J3 is used by another group; its pins must be next to each other on one side";
    for (const json of [
      '{"left":[{"no":"1","connector":"jst_ph_f","connector_ref":"J3"}],"right":[{"no":"2","connector":"jst_ph_f","connector_ref":"J3"}]}',
      '{"left":[{"no":"1","connector":"jst_ph_f","connector_ref":"J3"},{"no":"X"},{"no":"2","connector":"jst_ph_f","connector_ref":"J3"}]}',
      '{"left":[{"no":"1","connector":"screw","connector_ref":"J3"},{"no":"2","connector":"jst_ph_f","connector_ref":"J3"}]}',
    ]) {
      expect(message(json)).toBe(split);
    }
    expect(() =>
      blockSymbol({
        block_pins:
          '{"left":[{"no":"1","connector":"screw"},{"no":"2"},{"no":"3","connector":"screw"}],"top":[{"no":"4","connector":"jst_xh_f"},{"no":"5","connector":"usb_a_f"},{"no":"6","connector":"usb_a_f"}]}',
      }),
    ).not.toThrow();
  });
});

/** 数を 1e-6 に丸め、-0 を 0 にそろえる (Rust側 block.rs の rounded と同じ)。 */
function rounded(v: unknown): unknown {
  if (typeof v === "number") {
    const r = (Math.sign(v) * Math.round(Math.abs(v) * 1e6)) / 1e6;
    return r === 0 ? 0 : r;
  }
  if (Array.isArray(v)) return v.map(rounded);
  if (v !== null && typeof v === "object") {
    return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, rounded(x)]));
  }
  return v;
}

describe("connector golden (Rust twin)", () => {
  // ja: golden ファイルの全例 (全種類×全辺・AOGU の機器・コネクタの無いブロック) で、TS は Rust と同じプリミティブとピンを返す
  it("draws every block of the shared golden file exactly like Rust", () => {
    expect(golden.cases.length).toBe(59);
    for (const c of golden.cases) {
      const def = blockSymbol(c.attrs as Record<string, string>);
      expect({ name: c.name, primitives: rounded(def.primitives), pins: rounded(def.pins) }).toEqual({
        name: c.name,
        primitives: c.primitives,
        pins: c.pins,
      });
    }
  });
});

describe("block captions and group names (Rust twin)", () => {
  const BOARD_44 = JSON.stringify({
    left: ["VBUS", "D-", "D+", "GND"].map((name, i) => ({ no: String(i + 1), name, connector: "jst_xh_f", connector_ref: "X44B" })),
    right: ["VBUS", "D-", "D+", "GND"].map((name, i) => ({ no: String(i + 5), name, connector: "jst_xh_f", connector_ref: "X44A" })),
  });
  const textBox = (def: SymbolDef, needle: string): { lo: number; hi: number } => {
    for (const prim of def.primitives) {
      if (prim.type === "text" && prim.text === needle) {
        const half = textWidthMm(prim.text, prim.height) / 2;
        return { lo: prim.at.x - half, hi: prim.at.x + half };
      }
    }
    throw new Error(`文字 ${needle} が描かれていない`);
  };

  // ja: 左右にまとまりの名前があるときは、同じ行の2つの名前が文字の間隔 (1mm) を空けて収まる幅になる (44 の 20 x 20 指定は広がる)
  it("widens the box so group names on both sides keep the text gap", () => {
    const def = blockSymbol({ block_pins: BOARD_44, block_size: '{"w":20,"h":20}' });
    const left = textBox(def, "X44B XH");
    const right = textBox(def, "X44A XH");
    expect(right.lo - left.hi).toBeGreaterThanOrEqual(1 - 1e-9);
    const rect = def.primitives.flatMap((p) => (p.type === "line" ? p.pts.map((q) => q.x) : []));
    expect(Math.max(...rect) - Math.min(...rect)).toBeGreaterThan(20);
  });

  // ja: まとまりの名前が片側だけ、またはもう収まっている箱は指定の大きさのまま
  it("keeps the given size when the group names are on one side only or already fit", () => {
    const oneSide =
      '{"left":[{"no":"1","name":"VBUS","connector":"jst_xh_f","connector_ref":"X44B"},{"no":"2","name":"D-","connector":"jst_xh_f","connector_ref":"X44B"}],"right":[{"no":"5"},{"no":"6"}]}';
    const fixed = blockSymbol({ block_pins: oneSide, block_size: '{"w":20,"h":20}' });
    expect(Math.max(...fixed.pins.map((p) => p.at.x))).toBe(12.5);
    const roomy = blockSymbol({ block_pins: BOARD_44, block_size: '{"w":50,"h":20}' });
    expect(Math.max(...roomy.pins.map((p) => p.at.x))).toBe(30);
  });

  const placed = (json: string, rotation: number, mirror: boolean): [SymbolInstance, SymbolDef] => {
    const inst: SymbolInstance = {
      id: "i",
      symbol_id: "block",
      at: { x: 0, y: 0 },
      rotation,
      mirror,
      reference: "39",
      value: "USBハブ",
      attrs: { block_pins: json },
    };
    return [inst, blockSymbol(inst.attrs)];
  };

  // ja: 参照記号・型番は文字の箱まで含めた上端の上、機能ラベルは下端の下に置く (回転・ミラーでも。Rust の captions_clear_everything_the_pins_draw と同じ)
  it("places captions above and below the whole text boxes, also when rotated or mirrored", () => {
    const longNumbers =
      '{"left":[{"no":"4444","connector":"usb_c_f"},{"no":"333","connector":"usb_a_f"}],"right":[{"no":"2222","connector":"usb_a_f"}],"top":[{"no":"U1","connector":"usb_a_f"},{"no":"U2","connector":"usb_a_f"}],"bottom":[{"no":"PWR","connector":"micro_b_f"}]}';
    for (const rotation of [0, 90, 180, 270]) {
      for (const mirror of [false, true]) {
        const [inst, def] = placed(longNumbers, rotation, mirror);
        const base = symbolBounds(inst, def);
        let lo = base.min.y;
        let hi = base.max.y;
        for (const prim of def.primitives) {
          if (prim.type !== "text") continue;
          const c = transformLocal(prim.at, inst);
          lo = Math.min(lo, c.y - prim.height / 2);
          hi = Math.max(hi, c.y + prim.height / 2);
        }
        // 描画が使う関数そのもの: 型番の基準は symbolTopY、機能ラベルは functionLabelFallbackAt(captionBounds)
        expect(symbolTopY(inst, def)).toBeLessThanOrEqual(lo + 1e-9);
        const below = functionLabelFallbackAt(inst, captionBounds(inst, def));
        expect(below.y - FUNCTION_LABEL_FONT).toBeGreaterThanOrEqual(hi - 1e-9);
      }
    }
  });

  // ja: 外形が自動で上辺にピンの無いブロックと普通のシンボルの参照記号・型番の位置は今までと同じ
  it("leaves the caption bounds of an auto-sized block without top pins and of plain symbols as they were", () => {
    const [inst, def] = placed('{"left":[{"no":"IN","name":"IN"}],"right":[{"no":"OUT","name":"OUT"}]}', 0, false);
    expect(captionBounds(inst, def)).toEqual(symbolBounds(inst, def));
    const terminal = dynamicSymbol("terminal_block_3p")!;
    const plain = { ...inst, symbol_id: "terminal_block_3p", attrs: {} };
    expect(captionBounds(plain, terminal)).toEqual(symbolBounds(plain, terminal));
  });

  // ja: 最小の高さを指定したブロックは最初の左右のピン番号が外形の上端から 0.5 mm はみ出すので、上辺にピンが無くても参照記号・型番は 0.5 mm 上がる (Rust の captions_of_a_minimum_height_block_clear_the_side_pin_numbers と同じ)
  it("moves the captions of a minimum-height block up 0.5 mm to clear the side pin numbers", () => {
    const [inst] = placed('{"left":[{"no":"1"},{"no":"2"}]}', 0, false);
    inst.attrs = { ...inst.attrs, block_size: '{"w":10,"h":10}' };
    const def = blockSymbol(inst.attrs);
    const top = symbolTopY(inst, def);
    expect(top).toBeCloseTo(symbolBounds(inst, def).min.y - 0.5, 9);
    for (const prim of def.primitives) {
      if (prim.type === "text") expect(top).toBeLessThanOrEqual(prim.at.y - prim.height / 2 + 1e-9);
    }
  });

  /** ピン表のどの辺が描かれた上辺になるか (ミラー→時計回りの回転、Y下向き) を手で書いた表 (Rust の side_drawn_on_top と同じ)。 */
  const sideDrawnOnTop = (rotation: number, mirror: boolean): string =>
    rotation === 0
      ? "top"
      : rotation === 180
        ? "bottom"
        : rotation === 90
          ? mirror
            ? "right"
            : "left"
          : mirror
            ? "left"
            : "right";

  // ja: 描かれた上辺にピンのあるブロックは、参照記号・型番を外形の左上の角 (左辺の 1.0 mm 左・型番の箱の下端が上辺の 1.0 mm 以上上) に右揃えで置き、ブロックが描くものと重ならない。それ以外は今まで通り (Rust の captions_of_a_block_with_top_pins_sit_at_the_top_left_corner と同じ)
  it("puts the captions of a block with top pins at the top-left corner, right-aligned, clear of everything drawn", () => {
    const longTop =
      '{"left":[{"no":"4444","connector":"usb_c_f"},{"no":"333","connector":"usb_a_f"}],"top":[{"no":"UUUU","name":"長い名前","connector":"usb_a_f"},{"no":"U2","connector":"usb_a_f"}],"right":[{"no":"2222","connector":"usb_a_f"}]}';
    const hub =
      '{"top":[{"no":"U1","connector":"usb_a_f"},{"no":"U2","connector":"usb_a_f"}],"bottom":[{"no":"PWR","connector":"micro_b_f"}],"right":[{"no":"OTG","connector":"micro_b_m"}]}';
    const flat = '{"left":[{"no":"IN","name":"IN"}],"right":[{"no":"OUT","name":"OUT"}]}';
    let withTop = 0;
    let withoutTop = 0;
    for (const json of [hub, longTop, flat, '{"top":[{"no":"1"}],"left":[{"no":"2"}]}']) {
      for (const rotation of [0, 90, 180, 270]) {
        for (const mirror of [false, true]) {
          const [inst, def] = placed(json, rotation, mirror);
          const a = captionAnchor(inst, def);
          // 外形の用紙座標 (ピン表と寸法から)
          const { pins, size } = parseBlockSpec(inst.attrs);
          const [w, h] = blockBox(pins, size);
          const c1 = transformLocal({ x: -w / 2, y: -h / 2 }, inst);
          const c2 = transformLocal({ x: w / 2, y: h / 2 }, inst);
          const body = {
            minX: Math.min(c1.x, c2.x),
            maxX: Math.max(c1.x, c2.x),
            minY: Math.min(c1.y, c2.y),
          };
          // 描かれた上辺になるピン表の辺は手で書いた表から (実装の判定を写さない)
          const hasTop = (JSON.parse(json)[sideDrawnOnTop(rotation, mirror)] ?? []).length > 0;
          if (!hasTop) {
            withoutTop += 1;
            expect(a).toEqual({ x: inst.at.x, top: symbolTopY(inst, def), end: false });
            continue;
          }
          withTop += 1;
          expect(a.end).toBe(true);
          expect(a.x).toBeCloseTo(body.minX - CORNER_CAPTION_GAP, 9);
          expect(a.top - VALUE_LABEL_DY).toBeLessThanOrEqual(body.minY - 1 + 1e-9);
          // ブロックが描くもの (図形の外接矩形・文字の箱全体) と、2行の文字の箱が重ならない
          const rects: { x0: number; y0: number; x1: number; y1: number }[] = [];
          for (const prim of def.primitives) {
            if (prim.type === "text") {
              const c = transformLocal(prim.at, inst);
              const half = textWidthMm(prim.text, prim.height) / 2;
              rects.push({ x0: c.x - half, y0: c.y - prim.height / 2, x1: c.x + half, y1: c.y + prim.height / 2 });
            } else {
              const pts =
                prim.type === "line"
                  ? prim.pts
                  : prim.type === "rect"
                    ? [prim.p1, prim.p2]
                    : [
                        { x: prim.center.x - prim.r, y: prim.center.y - prim.r },
                        { x: prim.center.x + prim.r, y: prim.center.y + prim.r },
                      ];
              const t = pts.map((q) => transformLocal(q, inst));
              rects.push({
                x0: Math.min(...t.map((q) => q.x)),
                y0: Math.min(...t.map((q) => q.y)),
                x1: Math.max(...t.map((q) => q.x)),
                y1: Math.max(...t.map((q) => q.y)),
              });
            }
          }
          for (const [text, dy] of [
            [inst.reference, REF_LABEL_DY],
            [inst.value, VALUE_LABEL_DY],
          ] as const) {
            const cap = { x0: a.x - textWidthMm(text, LABEL_FONT), x1: a.x, y0: a.top - dy - LABEL_FONT, y1: a.top - dy };
            for (const r of rects) {
              const overlap = cap.x0 < r.x1 - 0.01 && r.x0 < cap.x1 - 0.01 && cap.y0 < r.y1 - 0.01 && r.y0 < cap.y1 - 0.01;
              expect(overlap, `回転${rotation}・ミラー${mirror}: ${text} が描かれたものに重なる`).toBe(false);
            }
          }
        }
      }
    }
    expect(withTop).toBeGreaterThan(0);
    expect(withoutTop).toBeGreaterThan(0);
  });

  // ja: 参照記号・型番が動くのはブロックだけ。普通のシンボルは今まで通り中央揃え (Rust の captions_of_a_plain_symbol_keep_the_centred_anchor と同じ)
  it("keeps the centred caption anchor of a plain symbol", () => {
    const [inst] = placed('{"top":[{"no":"1"}]}', 0, false);
    const terminal = dynamicSymbol("terminal_block_3p")!;
    const plain = { ...inst, symbol_id: "terminal_block_3p", attrs: {} };
    expect(captionAnchor(plain, terminal)).toEqual({ x: plain.at.x, top: symbolTopY(plain, terminal), end: false });
  });

  // ja: 上辺と左辺に1本ずつピンのあるブロックは、手で書いた表のとおりに角へ置く (Rust の corner_placement_follows_the_rotation_table と同じ)
  it("follows the explicit rotation table for a block with one top and one left pin", () => {
    const json = '{"top":[{"no":"1"}],"left":[{"no":"2"}]}';
    const table: [number, boolean, boolean][] = [
      [0, false, true],
      [0, true, true],
      [90, false, true],
      [270, true, true],
      [90, true, false],
      [180, false, false],
      [180, true, false],
      [270, false, false],
    ];
    for (const [rotation, mirror, corner] of table) {
      const [inst, def] = placed(json, rotation, mirror);
      expect(captionAnchor(inst, def).end, `回転${rotation}・ミラー${mirror}`).toBe(corner);
    }
  });

  // ja: 外形の左上の角の外に描かれたもの (手で足した文字) があると、角の参照記号・型番はその上まで上がり、重ならない (Rust の corner_captions_rise_above_a_drawing_at_the_corner と同じ)
  it("raises the corner captions above a drawing placed at the corner", () => {
    const [inst, base] = placed('{"top":[{"no":"1"}],"left":[{"no":"2"}]}', 0, false);
    const { pins, size } = parseBlockSpec(inst.attrs);
    const [w, h] = blockBox(pins, size);
    const bodyMinX = transformLocal({ x: -w / 2, y: -h / 2 }, inst).x;
    const before = captionAnchor(inst, base);
    // 何も無いときの型番の行の高さに、角の左へ文字を置く
    const at = { x: bodyMinX - 4, y: before.top - VALUE_LABEL_DY - 1 };
    const def: SymbolDef = {
      ...base,
      primitives: [...base.primitives, { type: "text", at, text: "OBSTACLE", height: 2.5 }],
    };
    const half = textWidthMm("OBSTACLE", 2.5) / 2;
    const obstacle = { x0: at.x - half, x1: at.x + half, y0: at.y - 1.25, y1: at.y + 1.25 };
    const a = captionAnchor(inst, def);
    expect(a.end).toBe(true);
    expect(a.top - VALUE_LABEL_DY).toBeLessThanOrEqual(obstacle.y0 + 1e-9);
    for (const [text, dy] of [
      [inst.reference, REF_LABEL_DY],
      [inst.value, VALUE_LABEL_DY],
    ] as const) {
      const cap = { x0: a.x - textWidthMm(text, LABEL_FONT), x1: a.x, y0: a.top - dy - LABEL_FONT, y1: a.top - dy };
      for (const prim of def.primitives) {
        if (prim.type !== "text") continue;
        const c = transformLocal(prim.at, inst);
        const hw = textWidthMm(prim.text, prim.height) / 2;
        const r = { x0: c.x - hw, x1: c.x + hw, y0: c.y - prim.height / 2, y1: c.y + prim.height / 2 };
        const overlap = cap.x0 < r.x1 - 0.01 && r.x0 < cap.x1 - 0.01 && cap.y0 < r.y1 - 0.01 && r.y0 < cap.y1 - 0.01;
        expect(overlap, `${text} が ${prim.text} に重なる`).toBe(false);
      }
    }
  });
});
