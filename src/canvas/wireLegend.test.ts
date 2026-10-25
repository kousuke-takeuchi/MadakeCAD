// 電線品番凡例ブロック (G9残) の幾何計算の仕様。
// Rust側 madake-core/src/wire_legend.rs と同じ数式で、キャンバスとSVG出力の見た目を一致させる。
import { describe, expect, it } from "vitest";

import type { Entity, Sheet, Wire, WireLegend, WirePart } from "../ipc";
import { LEGEND_COL_W, LEGEND_ROW_H, legendRect, legendRows, wireLegendAddCommand, wireLegends } from "./wireLegend";
import { wireNoteText } from "./wireNotes";

function wire(color: string, sq: number, length_m: number | null, part_no: string | null): Wire {
  return {
    id: `w-${color}-${sq}-${part_no ?? "x"}-${Math.random()}`,
    points: [
      { x: 0, y: 0 },
      { x: 10, y: 0 },
    ],
    color,
    sq,
    length_m,
    part_no,
    net: null,
  };
}

function sheetWith(entities: Entity[]): Sheet {
  return {
    id: "s1",
    name: "S1",
    size: "A3",
    orientation: "Landscape",
    zone_cols: 4,
    zone_rows: 6,
    title_block: {},
    revisions: [],
    entities: Object.fromEntries(entities.map((e) => [e.id, e])),
  } as unknown as Sheet;
}

describe("wire legend with run-cable breakout wires", () => {
  // ja: 区間ケーブルの分岐の配線は線色も線径も持たないので、芯の線色とケーブルの線径で数え、空欄の行を増やさない。
  it("counts a breakout wire under its core colour and cable gauge instead of a blank row", () => {
    const w = wire("", 0, null, null);
    const cable = {
      kind: "run_cable",
      id: "c1",
      reference: "W1",
      part_no: "",
      core_count: 1,
      sq: 0.2,
      points: [],
      cores: [{ core: "C1", color: "green", end_a: { kind: "wire", wire: w.id }, end_b: null }],
      shield: null,
      length_m: null,
      run: [
        { x: 0, y: 0 },
        { x: 10, y: 0 },
      ],
    } as unknown as Entity;
    const rows = legendRows(sheetWith([{ kind: "wire", ...w } as unknown as Entity, cable]), []);
    expect(rows).toEqual([["緑", "0.2", "", "0.00"]]);
  });
});

describe("wire legend block geometry", () => {
  // ja: 配線が1本も無いシートでも、凡例ブロックは列見出しだけの表として出る。
  it("has only the header row for a sheet with no wires", () => {
    const sheet = sheetWith([]);
    expect(legendRows(sheet, [])).toEqual([]);
    const legend: WireLegend = { id: "l1", at: { x: 10, y: 10 } };
    const rect = legendRect(sheet, [], legend);
    expect(rect).toEqual({ x: 10, y: 10, w: LEGEND_COL_W.reduce((a, b) => a + b, 0), h: LEGEND_ROW_H });
  });

  // ja: 線色・線径・引き当てた品番が同じ配線は1行にまとまり、使用長は合算される。
  it("sums the length of wires sharing the same colour, gauge and part number into one row", () => {
    const sheet = sheetWith([
      { ...wire("水色", 0.3, 1.5, "SAMPLE0001"), kind: "wire" },
      { ...wire("水色", 0.3, 2.5, "SAMPLE0001"), kind: "wire" },
    ]);
    expect(legendRows(sheet, [])).toEqual([["水色", "0.3", "SAMPLE0001", "4.00"]]);
  });

  // ja: 行は線色順、同じ線色の中では線径順に並ぶ。
  it("sorts rows by colour then by gauge", () => {
    const sheet = sheetWith([
      { ...wire("赤", 0.5, 1.0, null), kind: "wire" },
      { ...wire("赤", 0.3, 1.0, null), kind: "wire" },
      { ...wire("水色", 0.3, 1.0, null), kind: "wire" },
    ]);
    const rows = legendRows(sheet, []);
    expect(rows.map((r) => [r[0], r[1]])).toEqual([
      ["水色", "0.3"],
      ["赤", "0.3"],
      ["赤", "0.5"],
    ]);
  });

  // ja: 足し合わせると二進小数で誠実に表せない使用長も、桁の並んだ長い数字ではなく短い小数で表示される。
  it("rounds a summed length instead of showing floating point noise", () => {
    // 0.4 を8本足すと、二進小数では 3.1999999999999997 になる
    const sheet = sheetWith(
      Array.from({ length: 8 }, () => ({ ...wire("水色", 0.3, 0.4, null), kind: "wire" }) as Entity),
    );
    const rows = legendRows(sheet, []);
    expect(rows).toHaveLength(1);
    expect(rows[0][3]).toBe("3.20");
  });

  // ja: 英語名と日本語名で書かれた同じ線色は同じ電線なので、1行にまとまり使用長も合算される。
  it("merges a colour written in English and in Japanese into one row", () => {
    const sheet = sheetWith([
      { ...wire("red", 0.75, 1.2, null), kind: "wire" },
      { ...wire("赤", 0.75, 2.0, null), kind: "wire" },
      { ...wire("RED", 0.75, 0.8, null), kind: "wire" },
    ]);
    const rows = legendRows(sheet, []);
    expect(rows).toHaveLength(1);
    expect(rows[0][0]).toBe("赤");
    expect(rows[0][3]).toBe("4.00");
  });

  // ja: 凡例に出る色名は、同じシートの配線注記が印字する色名と同じになる。
  it("prints the same colour name as the wire annotation on the same sheet", () => {
    const w = wire("light_blue", 0.5, 1.0, null);
    const sheet = sheetWith([{ ...w, kind: "wire" }]);
    const rows = legendRows(sheet, []);
    expect(rows[0][0]).toBe("水色");
    expect(wireNoteText(w)).toContain(rows[0][0]);
  });

  // ja: 電線品番表に無い組み合わせの配線は品番欄が空欄のままで、勝手に品番が作られることはない。
  it("leaves the part number blank when the colour and gauge are not in the wire-part table", () => {
    const parts: WirePart[] = [{ part_no: "SAMPLE0001", color: "水色", sq: 0.3, note: "" }];
    const sheet = sheetWith([{ ...wire("赤", 0.5, 1.0, null), kind: "wire" }]);
    const rows = legendRows(sheet, parts);
    expect(rows).toHaveLength(1);
    expect(rows[0][2]).toBe("");
  });

  // ja: 凡例ブロックを置くコマンドはadd_entityでkind="wire_legend"、置き場所だけを決める。
  it("builds an add_entity command that only sets the placement", () => {
    const sheet = sheetWith([]);
    const cmd = wireLegendAddCommand(sheet, { x: 50, y: 60 });
    expect(cmd.type).toBe("add_entity");
    if (cmd.type === "add_entity") {
      expect(cmd.entity.kind).toBe("wire_legend");
      expect(cmd.entity).toMatchObject({ at: { x: 50, y: 60 } });
    }
  });

  // ja: シート上の凡例ブロックはentityのまま取り出せる。
  it("lists the wire legend entities placed on a sheet", () => {
    const legend: WireLegend & { kind: "wire_legend" } = { kind: "wire_legend", id: "l1", at: { x: 0, y: 0 } };
    const sheet = sheetWith([legend]);
    expect(wireLegends(sheet)).toHaveLength(1);
  });
});
