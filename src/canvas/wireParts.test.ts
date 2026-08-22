// 電線品番の引き当て (G11)。Rust側 wire_parts.rs と同じ結果になることを確かめる。
import { describe, expect, it } from "vitest";

import type { Project, WirePart } from "../ipc";
import { autoWirePartNo, findWirePart, wirePartNo } from "./wireParts";

function part(part_no: string, color: string, sq: number): WirePart {
  return { part_no, color, sq, note: "" };
}

function project(parts: WirePart[]): Project {
  return { format_version: 6, name: "t", sheets: [], wire_parts: parts, plc_assignments: [] } as Project;
}

describe("wirePartNo", () => {
  // ja: ワイヤの品番は線色と線径から電線品番表を引いて決まる
  it("looks a wire's part number up by colour and gauge", () => {
    const p = project([part("SAMPLE0001", "水色", 0.3), part("SAMPLE0002", "白", 0.75)]);
    expect(wirePartNo(p, { color: "白", sq: 0.75 })).toBe("SAMPLE0002");
  });

  // ja: 表に無い線色+線径の組み合わせは空のまま
  it("leaves an unregistered combination empty", () => {
    const p = project([part("SAMPLE0001", "水色", 0.3)]);
    expect(wirePartNo(p, { color: "水色", sq: 0.75 })).toBe("");
    expect(wirePartNo(p, { color: "赤", sq: 0.3 })).toBe("");
    expect(wirePartNo(null, { color: "水色", sq: 0.3 })).toBe("");
  });

  // ja: ワイヤ自身に書かれた品番は表より優先される
  it("prefers a part number written on the wire", () => {
    const p = project([part("SAMPLE0001", "水色", 0.3)]);
    expect(wirePartNo(p, { color: "水色", sq: 0.3, part_no: "MY-WIRE-9" })).toBe("MY-WIRE-9");
    expect(wirePartNo(p, { color: "水色", sq: 0.3, part_no: "  " })).toBe("SAMPLE0001");
  });

  // ja: 引き当ては線色の前後の空白と英字の大小を無視する
  it("ignores spaces and letter case in the colour", () => {
    const p = project([part("SAMPLE0003", " Light_Blue ", 0.5)]);
    expect(autoWirePartNo(p, "light_blue", 0.5)).toBe("SAMPLE0003");
    expect(autoWirePartNo(p, " LIGHT_BLUE", 0.5)).toBe("SAMPLE0003");
  });

  // ja: 線色が空・線径が0のワイヤは引き当てない
  it("does not look up a wire without a colour or a gauge", () => {
    const p = project([part("SAMPLE0001", "水色", 0.3), part("SAMPLE0004", "", 0.3)]);
    expect(autoWirePartNo(p, "", 0.3)).toBe("");
    expect(autoWirePartNo(p, "水色", 0)).toBe("");
  });

  // ja: 同じ線色+線径の行が2つあるときは先に書いた行が勝つ
  it("takes the first of two rows for the same colour and gauge", () => {
    const p = project([part("FIRST", "黒", 1.25), part("SECOND", "黒", 1.25)]);
    expect(findWirePart(p.wire_parts, "黒", 1.25)?.part_no).toBe("FIRST");
  });
});
