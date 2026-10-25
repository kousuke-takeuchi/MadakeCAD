// 多芯ケーブル (M4 §5) の作図領域側の仕様テスト。芯線割当そのものはRust側のテストが正。
import { describe, expect, it } from "vitest";
import type { Cable, CableCore, Entity, Sheet } from "../ipc";
import {
  cableAddCommand,
  cableHit,
  cableLabel,
  cablePoints,
  coreEndPin,
  isCable,
  nextCableReference,
  runBars,
  runCountAt,
  runFans,
  runLabelAt,
  runSlash,
  shieldEllipse,
} from "./cable";

function cable(over: Partial<Cable> = {}): Cable {
  return {
    id: "c1",
    reference: "W1",
    part_no: "SAMPLE-CBL",
    core_count: 4,
    sq: 1.25,
    points: [{ x: 0, y: 0 }, { x: 0, y: 30 }],
    cores: [],
    shield: null,
    length_m: 12.5,
    ...over,
  };
}

function sheet(entities: Sheet["entities"] = {}): Sheet {
  return { id: "s1", name: "Sheet1", entities } as unknown as Sheet;
}

/** LTEハーネス45 (Rust側 test_support::lte_sheet と同じ座標): J45 のピン x=57.5 から配線4本が x=77.5 まで出て、区間 (80,100)→(192.5,100) が反転した USB1 (ピン x=192.5) に着く。 */
function lteSheet(): Sheet {
  const sym = (id: string, reference: string, x: number, mirror: boolean) => ({
    kind: "symbol", id, symbol_id: "connector_4p", at: { x, y: 100 }, rotation: 0, mirror, reference, value: "", attrs: {},
  });
  const entities: Record<string, unknown> = { j45: sym("j45", "J45", 50, false), usb: sym("usb", "USB1", 200, true) };
  const cores: CableCore[] = ["VBUS", "D-", "D+", "GND"].map((core, i) => {
    const y = 92.5 + 5 * i;
    entities[`w${i}`] = { kind: "wire", id: `w${i}`, points: [{ x: 57.5, y }, { x: 77.5, y }], color: "black", sq: 0.3, length_m: null, part_no: null, net: null };
    return { core, color: "", wire: null, end_a: { kind: "wire" as const, wire: `w${i}` }, end_b: { kind: "pin" as const, part: "usb", pin: String(i + 1) } };
  });
  entities.w45 = { kind: "run_cable", ...cable({ id: "w45", reference: "W45", points: [], cores, run: [{ x: 80, y: 100 }, { x: 192.5, y: 100 }] }) };
  return sheet(entities as Sheet["entities"]);
}

describe("多芯ケーブル", () => {
  // ja: ケーブルの注記は参照記号・型番・芯数×線径・長さの順に並び、空の項は飛ばす (Rust側と同じ)
  it("labels a cable with designator, part, cores times gauge and length, skipping empty items", () => {
    expect(cableLabel(cable())).toBe("W1  SAMPLE-CBL  4C×1.25sq  12.5m");
    expect(cableLabel(cable({ part_no: "", sq: 0, length_m: null, core_count: 2 }))).toBe("W1  2C");
  });

  // ja: シールドの楕円は定義線の中点を中心に、定義線より少し長く囲む
  it("draws the shield as an ellipse around the definition line, a little longer than it", () => {
    const e = shieldEllipse(cable())!;
    expect(e.center).toEqual({ x: 0, y: 15 });
    expect(e.rx).toBeCloseTo(17.5);
    expect(e.angle).toBeCloseTo(Math.PI / 2);
  });

  // ja: 新しいケーブルの参照記号は、シート上のケーブルとハーネスのW番号の続きになる
  it("numbers a new cable after the W numbers of the cables and harnesses on the sheet", () => {
    const s = sheet({
      c1: { kind: "cable", ...cable({ reference: "W2" }) },
      h1: { kind: "harness", id: "h1", points: [], name: "W5", note: "" },
    });
    expect(nextCableReference(s)).toBe("W6");
    expect(nextCableReference(sheet())).toBe("W1");
  });

  // ja: 定義線のドラッグは、芯数0 (横切った本数にする) のadd_cableコマンドになり、クリックしただけでは何も作らない
  it("turns a drag into an add_cable command with no core count, and a mere click into nothing", () => {
    const command = cableAddCommand(sheet(), { x: 0, y: 0 }, { x: 10, y: 20 });
    expect(command).toMatchObject({ type: "add_cable", sheet_id: "s1", cable: { reference: "W1", core_count: 0, points: [{ x: 0, y: 0 }, { x: 10, y: 20 }] } });
    expect(cableAddCommand(sheet(), { x: 0, y: 0 }, { x: 1, y: 1 })).toBeNull();
  });

  // ja: 定義線の近くをクリックするとケーブルを掴める
  it("is picked by clicking near its definition line", () => {
    expect(cableHit(cable(), { x: 0.5, y: 10 }, 1)).toBe(true);
    expect(cableHit(cable(), { x: 5, y: 10 }, 1)).toBe(false);
  });

  // ja: isCable は区間の有無にかかわらずケーブル (cable と run_cable) を見分け、cablePoints は定義線と区間の点を両方返す
  it("recognises both cable kinds and lists the points of the run", () => {
    const run: Entity = { kind: "run_cable", ...cable({ points: [], run: [{ x: 1, y: 2 }, { x: 3, y: 2 }] }) };
    const plain: Entity = { kind: "cable", ...cable() };
    const wire = { kind: "wire", id: "w", points: [], color: "", sq: 0, length_m: null, part_no: null, net: null } as Entity;
    expect([isCable(run), isCable(plain), isCable(wire), isCable(null)]).toEqual([true, true, false, false]);
    expect(cablePoints(run as Cable)).toEqual([{ x: 1, y: 2 }, { x: 3, y: 2 }]);
    expect(cablePoints(plain as Cable)).toEqual([{ x: 0, y: 0 }, { x: 0, y: 30 }]);
  });

  // ja: 芯の行き先がピンなら「参照記号.ピン」(部品が消えていれば「?.ピン」)、分岐の配線ならnull
  it("names a pin end by designator and pin, and a breakout wire end as null", () => {
    const s = sheet({
      usb: { kind: "symbol", id: "usb", symbol_id: "connector_4p", at: { x: 0, y: 0 }, rotation: 0, mirror: false, reference: "USB1", value: "", attrs: {} },
    } as unknown as Sheet["entities"]);
    expect(coreEndPin(s, { kind: "pin", part: "usb", pin: "2" })).toBe("USB1.2");
    expect(coreEndPin(s, { kind: "pin", part: "gone", pin: "1" })).toBe("?.1");
    expect(coreEndPin(s, { kind: "wire", wire: "w1" })).toBeNull();
  });

  // ja: 区間ケーブルの印は区間のいちばん長い線分の中点を通る45度の斜線で、芯数の文字・注記・シールドの楕円もそこを基準にする (Rust側と同じ)
  it("marks a run with a slash through the middle of its longest segment, like the printed sheet", () => {
    const c = cable({ points: [], run: [{ x: 80, y: 100 }, { x: 192.5, y: 100 }] });
    expect(runSlash(c)).toEqual([{ x: 134.75, y: 101.5 }, { x: 137.75, y: 98.5 }]);
    expect(runCountAt(c)).toEqual({ x: 138.25, y: 98 });
    expect(runLabelAt(c)).toEqual({ x: 139.75, y: 104 });
    expect(shieldEllipse(c)).toEqual({ center: { x: 136.25, y: 100 }, rx: 3, ry: 2, angle: -Math.PI / 4 });
  });

  // ja: 部品に着く端は着いたピンの先を結ぶ棒、分岐の端は区間の端から各芯の配線の端への扇になる
  it("draws a bar across the landed pins and a fan to each breakout wire", () => {
    const s = lteSheet();
    const c = s.entities.w45 as Cable;
    expect(runBars(s, c, new Map())).toEqual([[{ x: 192.5, y: 92.5 }, { x: 192.5, y: 107.5 }]]);
    const fans = runFans(s, c);
    expect(fans).toHaveLength(4);
    expect(fans[1]).toEqual({ from: { x: 80, y: 100 }, to: { x: 77.5, y: 97.5 }, core: "D-" });
  });

  // ja: 区間ケーブルは区間の線の近くをクリックすると掴める
  it("is picked by clicking near its run", () => {
    const c = cable({ points: [], run: [{ x: 80, y: 100 }, { x: 192.5, y: 100 }] });
    expect(cableHit(c, { x: 120, y: 100.5 }, 1)).toBe(true);
    expect(cableHit(c, { x: 120, y: 105 }, 1)).toBe(false);
  });

  // ja: 区間の片側の芯が全部1つのポートのピンに着くと、その側には横棒を描かない (Rust の run_bars と同じ)
  it("draws no bar when every core of a side lands on one port pin", () => {
    const s = lteSheet();
    const c = s.entities.w45 as Cable;
    c.cores = c.cores.map((k) => ({ ...k, end_b: { kind: "pin" as const, part: "usb", pin: "1", contact: k.core } }));
    expect(runBars(s, c, new Map())).toEqual([]);
  });
});
