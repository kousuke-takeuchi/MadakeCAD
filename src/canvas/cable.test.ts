// 多芯ケーブル (M4 §5) の作図領域側の仕様テスト。芯線割当そのものはRust側のテストが正。
import { describe, expect, it } from "vitest";
import type { Cable, Sheet } from "../ipc";
import { cableAddCommand, cableHit, cableLabel, nextCableReference, shieldEllipse } from "./cable";

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
});
