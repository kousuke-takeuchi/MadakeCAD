// 嵌合コネクタ対 (G14) の幾何・対応付けの仕様。Rust側 madake-core/src/mate.rs と同一ルール
// (パリティテスト: 同じ入力に対して同じ組・同じ座標を返すことを確かめる)。
import { describe, expect, it } from "vitest";

import type { Entity, Project, Sheet, SymbolInstance } from "../ipc";
import { resolveSymbolDef } from "./dynamicSymbol";
import {
  MATES_WITH_ATTR,
  declaredMate,
  isConnectorInstance,
  mateLinkSegment,
  mateLinks,
  rectBoundaryPointToward,
} from "./mate";

function connector(id: string, reference: string, x: number, mate: string | null): Entity {
  return {
    kind: "symbol",
    id,
    symbol_id: "connector_3p",
    at: { x, y: 0 },
    rotation: 0,
    mirror: false,
    reference,
    value: "",
    attrs: mate ? { [MATES_WITH_ATTR]: mate } : {},
  } satisfies SymbolInstance & { kind: "symbol" };
}

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

function projectWith(entities: Entity[]): Project {
  const sheet = sheetWith(entities);
  return {
    format_version: 10,
    name: "t",
    sheets: [sheet],
    wire_parts: [],
    plc_assignments: [],
  };
}

const symbolDefs = new Map(
  [resolveSymbolDef("connector_3p", new Map())!, resolveSymbolDef("connector_4p", new Map())!].map((d) => [
    d.id,
    d,
  ]),
);

describe("mate: connector family gate", () => {
  // ja: connector_で始まるsymbol_idだけが嵌合対の対象で、端子台は除外される
  it("only symbol ids starting with connector_ are treated as mating connectors", () => {
    expect(isConnectorInstance("connector_3p")).toBe(true);
    expect(isConnectorInstance("connector_plug")).toBe(true);
    expect(isConnectorInstance("terminal_block_3p")).toBe(false);
    expect(isConnectorInstance("terminal")).toBe(false);
  });
});

describe("mate: declaredMate", () => {
  // ja: 宣言した嵌合相手の参照記号がそのまま読める
  it("reads the mate declared on a connector instance", () => {
    const sheet = sheetWith([connector("a", "J1", 0, "J12")]);
    expect(declaredMate(sheet, "J1")).toBe("J12");
  });

  // ja: 宣言が無い参照記号は空文字を返す
  it("returns empty string when there is no declaration", () => {
    const sheet = sheetWith([connector("a", "J1", 0, null)]);
    expect(declaredMate(sheet, "J1")).toBe("");
    expect(declaredMate(sheet, "J99")).toBe("");
  });
});

describe("mate: mateLinks", () => {
  // ja: 同一シート上で50mm以内にある嵌合対は、描くべきペアがちょうど1組できる
  it("a close pair on one sheet produces exactly one mate link", () => {
    const project = projectWith([connector("a", "J1", 0, "J12"), connector("b", "J12", 30, "J1")]);
    const links = mateLinks(project, symbolDefs);
    expect(links).toHaveLength(1);
    expect(new Set([links[0].a, links[0].b])).toEqual(new Set(["a", "b"]));
  });

  // ja: 50mmを超えて離れた嵌合対は描くべきペアに入らない (帳票だけが伝える)
  it("a pair more than 50mm apart produces no mate link", () => {
    const project = projectWith([connector("a", "J1", 0, "J12"), connector("b", "J12", 200, "J1")]);
    expect(mateLinks(project, symbolDefs)).toHaveLength(0);
  });

  // ja: 宣言していないコネクタはペアに入らない
  it("a connector with no declaration produces no mate link", () => {
    const project = projectWith([connector("a", "J1", 0, null), connector("b", "J12", 30, null)]);
    expect(mateLinks(project, symbolDefs)).toHaveLength(0);
  });
});

describe("mate: rectBoundaryPointToward", () => {
  // ja: 矩形の外側にある点へは、その点へ向かってクランプした周上の点を返す
  it("clamps toward an external point onto the rectangle's boundary", () => {
    const p = rectBoundaryPointToward({ x: 0, y: 0 }, { x: 10, y: 10 }, { x: 50, y: 5 });
    expect(p).toEqual({ x: 10, y: 5 });
  });
});

describe("mate: mateLinkSegment (SVG/canvas parity)", () => {
  // ja: 破線の両端は、双方の外形のうち相手にいちばん近い点になる
  it("the segment endpoints sit on each connector's own outline, nearest the other", () => {
    const project = projectWith([connector("a", "J1", 0, "J12"), connector("b", "J12", 30, "J1")]);
    const sheet = project.sheets[0];
    const link = mateLinks(project, symbolDefs)[0];
    const seg = mateLinkSegment(sheet, link, symbolDefs);
    expect(seg).not.toBeNull();
    // connector_3p の外形は基準点を中心に左右対称なので、線分は2点とも y=0 上
    expect(seg!.a.y).toBe(0);
    expect(seg!.b.y).toBe(0);
    // aの点はbの点よりも常にaの基準点に近い側 (x座標が小さい側) にある
    expect(seg!.a.x).toBeLessThan(seg!.b.x);
  });
});
