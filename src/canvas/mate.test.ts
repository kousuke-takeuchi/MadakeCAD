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

function connector(id: string, reference: string, x: number, mate: string | null, y = 0): Entity {
  return {
    kind: "symbol",
    id,
    symbol_id: "connector_3p",
    at: { x, y },
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

  // ja: 距離の上限は50mmちょうどを含む (50.0mmは描き、50mmを超えると描かない。Rust側 the_mate_link_distance_limit_includes_exactly_fifty_millimetres とparity)
  it("includes exactly 50mm in the distance limit", () => {
    const linksAt = (distance: number) =>
      mateLinks(
        projectWith([connector("a", "J1", 0, "J12"), connector("b", "J12", distance, "J1")]),
        symbolDefs,
      );
    expect(linksAt(50)).toHaveLength(1);
    expect(linksAt(50.5)).toHaveLength(0);
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

  // ja: 矩形の内側にある点は、いちばん近い辺の上へ押し出される (Rust側 a_point_inside_the_outline_is_pushed_out_to_the_nearest_edge とparity)
  it("pushes a point inside the rectangle out onto its nearest edge", () => {
    // (3,4) からは左辺が3mmでいちばん近い (右7mm・上4mm・下6mm)
    const p = rectBoundaryPointToward({ x: 0, y: 0 }, { x: 10, y: 10 }, { x: 3, y: 4 });
    expect(p).toEqual({ x: 0, y: 4 });
  });
});

describe("mate: mateLinkSegment (SVG/canvas parity)", () => {
  // ja: 30mm離れた3極コネクタ2つの嵌合線は、双方の外形の向かい合う辺の上、配置基準点の高さを結ぶ (Rust側 the_mate_link_runs_between_the_facing_outline_edges と同じ配置・同じ座標)
  it("runs between the facing outline edges of the two connectors", () => {
    const project = projectWith([
      connector("a", "J1", 100, "J12", 100),
      connector("b", "J12", 130, "J1", 100),
    ]);
    const sheet = project.sheets[0];
    const link = mateLinks(project, symbolDefs)[0];
    const seg = mateLinkSegment(sheet, link, symbolDefs);
    // connector_3pの外形は基準点の周り x[-4,+7.5] (ピンの先まで)・y[-7.5,+7.5]
    expect(seg).toEqual({ a: { x: 107.5, y: 100 }, b: { x: 126, y: 100 } });
  });
});
