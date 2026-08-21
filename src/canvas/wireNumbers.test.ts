// 線番テキストの配置計算 (IEC 62491)。Rust側 svg.rs の render_wire_numbers と同一ルールであること。
import { describe, expect, it } from "vitest";

import type { Entity, Sheet } from "../ipc";
import { WIRE_NO_FONT, WIRE_NO_GAP, wireNumberLabels } from "./wireNumbers";

let seq = 0;
function wire(points: [number, number][], net: string | null): Entity {
  seq += 1;
  return {
    kind: "wire",
    id: `w${seq}`,
    points: points.map(([x, y]) => ({ x, y })),
    color: "black",
    sq: 0.3,
    length_m: null,
    part_no: null,
    net,
  } as Entity;
}

function sheet(entities: Entity[]): Sheet {
  return {
    id: "s1",
    name: "Sheet1",
    size: "A3",
    orientation: "Landscape",
    zone_cols: 4,
    zone_rows: 6,
    title_block: {},
    revisions: [],
    entities: Object.fromEntries(entities.map((e) => [e.id, e])),
  } as unknown as Sheet;
}

describe("wireNumberLabels", () => {
  // ja: 横向きの配線の線番は、配線の中点の2.5mm上に中央揃えで置かれる
  it("puts the number of a horizontal wire 2.5mm above the midpoint, centred", () => {
    const labels = wireNumberLabels(sheet([wire([[10, 40], [30, 40]], "12")]));
    expect(labels).toEqual([{ number: "12", at: { x: 20, y: 40 - WIRE_NO_GAP }, align: "center" }]);
    expect(WIRE_NO_GAP).toBe(2.5);
    expect(WIRE_NO_FONT).toBe(2.5);
  });

  // ja: 縦向きの配線の線番は、配線の中点の2.5mm左に右揃えで置かれる
  it("puts the number of a vertical wire 2.5mm to the left of the midpoint, right aligned", () => {
    const labels = wireNumberLabels(sheet([wire([[50, 20], [50, 60]], "7")]));
    expect(labels).toEqual([{ number: "7", at: { x: 50 - WIRE_NO_GAP, y: 40 }, align: "right" }]);
  });

  // ja: 1つの線番は、何本のワイヤに分かれていても最も長い線分の中点に1回だけ置かれる
  it("draws one net's number only once, at the midpoint of its longest segment", () => {
    const labels = wireNumberLabels(
      sheet([
        wire([[0, 10], [10, 10]], "3"),
        wire([[10, 10], [10, 90]], "3"),
      ]),
    );
    expect(labels).toEqual([{ number: "3", at: { x: 10 - WIRE_NO_GAP, y: 50 }, align: "right" }]);
  });

  // ja: 線番の無い配線、空白だけの線番は描かない
  it("skips wires without a number and numbers that are only whitespace", () => {
    expect(wireNumberLabels(sheet([wire([[0, 0], [10, 0]], null)]))).toEqual([]);
    expect(wireNumberLabels(sheet([wire([[0, 0], [10, 0]], "  ")]))).toEqual([]);
  });

  // ja: 複数の線番があると線番順に並んで返り、同じ図面なら常に同じ並びになる
  it("returns every number in a stable order", () => {
    const labels = wireNumberLabels(
      sheet([
        wire([[0, 30], [20, 30]], "2"),
        wire([[0, 10], [20, 10]], "1"),
      ]),
    );
    expect(labels.map((l) => l.number)).toEqual(["1", "2"]);
  });

  // ja: 斜めの配線は、横に長ければ上、縦に長ければ左に線番を置く
  it("places a diagonal wire's number above when it runs wide and to the left when it runs tall", () => {
    expect(wireNumberLabels(sheet([wire([[0, 0], [40, 10]], "A")]))[0].align).toBe("center");
    expect(wireNumberLabels(sheet([wire([[0, 0], [10, 40]], "B")]))[0].align).toBe("right");
  });

  // ja: 配線以外のエンティティは線番の計算に影響しない
  it("ignores entities that are not wires", () => {
    const label = { kind: "net_label", id: "l1", at: { x: 0, y: 0 }, name: "24V", rotation: 0 } as Entity;
    expect(wireNumberLabels(sheet([label]))).toEqual([]);
  });
});
