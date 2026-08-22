// 2D盤レイアウトの幾何計算の仕様 (docs/internal/specs/m4-industrial-core.md §8)。
// Rust側 madake-core/src/panel.rs と同じ数式で、キャンバスとSVG出力の見た目を一致させる。
import { describe, expect, it } from "vitest";

import type { DinRail, Duct, Entity, Footprint, PanelOutline, Sheet } from "../ipc";
import {
  BALLOON_LEADER,
  BALLOON_R,
  DEFAULT_FOOTPRINT_SIZE,
  DIN_RAIL_H,
  LEGEND_COL_W,
  LEGEND_ROW_H,
  PLATE_INSET,
  balloon,
  ductHatch,
  footprintRect,
  footprintSize,
  itemOrder,
  legendRect,
  legendRows,
  numberedRails,
  panelPlateRect,
  railRect,
  railSnapY,
} from "./panel";

function footprint(rotation: number): Footprint {
  return {
    id: "fp-1",
    at: { x: 50, y: 50 },
    w: 25,
    h: 65,
    rotation,
    reference: "K1",
    part_no: "MDK-RLY-24V",
  };
}

const outline: PanelOutline = {
  id: "po-1",
  at: { x: 40, y: 40 },
  w: 200,
  h: 150,
  name: "制御盤",
};

const duct: Duct = { id: "d-1", at: { x: 100, y: 100 }, w: 20, h: 20 };

const rail: DinRail = { id: "r-1", y: 150, x1: 60, x2: 220 };

describe("panel layout geometry", () => {
  // ja: フットプリントは自分の寸法どおりの矩形を占める。
  it("gives a footprint the rectangle of its own size", () => {
    expect(footprintSize(footprint(0))).toEqual({ w: 25, h: 65 });
    expect(footprintRect(footprint(0))).toEqual({ x: 50, y: 50, w: 25, h: 65 });
  });

  // ja: 90度回したフットプリントは幅と高さが入れ替わる。
  it("swaps width and height for a footprint turned 90 degrees", () => {
    expect(footprintSize(footprint(90))).toEqual({ w: 65, h: 25 });
    expect(footprintRect(footprint(90))).toEqual({ x: 50, y: 50, w: 65, h: 25 });
  });

  // ja: 部品DBに寸法が無い型番でも既定の矩形で置ける。
  it("falls back to the default rectangle for a part with no size", () => {
    expect(DEFAULT_FOOTPRINT_SIZE).toEqual({ w: 20, h: 40 });
  });

  // ja: 盤外形の内側には一回り小さい取付板がある。
  it("draws the mounting plate inset inside the panel outline", () => {
    expect(panelPlateRect(outline)).toEqual({
      x: 40 + PLATE_INSET,
      y: 40 + PLATE_INSET,
      w: 200 - PLATE_INSET * 2,
      h: 150 - PLATE_INSET * 2,
    });
  });

  // ja: インセットより小さい盤外形には取付板が無い。
  it("has no mounting plate when the outline is smaller than the inset", () => {
    expect(panelPlateRect({ ...outline, w: PLATE_INSET, h: PLATE_INSET })).toBeNull();
  });

  // ja: DINレールは取付線を中心とした横帯として描かれる。
  it("draws a DIN rail as a band centred on its mounting line", () => {
    expect(railRect(rail)).toEqual({ x: 60, y: 150 - DIN_RAIL_H / 2, w: 160, h: DIN_RAIL_H });
  });

  // ja: 右から左へ引いたレールも同じ範囲を覆う。
  it("covers the same span for a rail drawn backwards", () => {
    expect(railRect({ ...rail, x1: 220, x2: 60 })).toEqual(railRect(rail));
  });

  // ja: ダクトは45度の斜線で埋まり、斜線はダクトの内側に収まる。
  it("fills a duct with 45 degree hatch lines that stay inside it", () => {
    const lines = ductHatch(duct);
    expect(lines.length).toBeGreaterThan(0);
    for (const line of lines) {
      expect(line.x2 - line.x1).toBeCloseTo(line.y2 - line.y1, 9);
      for (const [x, y] of [
        [line.x1, line.y1],
        [line.x2, line.y2],
      ]) {
        expect(x).toBeGreaterThanOrEqual(100);
        expect(x).toBeLessThanOrEqual(120);
        expect(y).toBeGreaterThanOrEqual(100);
        expect(y).toBeLessThanOrEqual(120);
      }
    }
    // 位置に依らず一定間隔なので、対角線がそのまま1本入る (Rust側と同じ結果)
    expect(lines).toContainEqual({ x1: 100, y1: 100, x2: 120, y2: 120 });
  });

  // ja: 幅や高さの無いダクトには描く斜線が無い。
  it("has no hatch lines for a flat duct", () => {
    expect(ductHatch({ ...duct, w: 0 })).toEqual([]);
    expect(ductHatch({ ...duct, h: 0 })).toEqual([]);
  });

  // ja: レールの近くへ置いた部品は、レールの取付線に吸い付く。
  it("snaps a part dropped near a rail onto its mounting line", () => {
    expect(railSnapY([rail], 152)).toBe(150);
    expect(railSnapY([rail], 200)).toBeNull();
    expect(railSnapY([], 150)).toBeNull();
  });
});

/** 盤レイアウトシート1枚 (エンティティはidをキーに並べる)。 */
function panelSheet(entities: Entity[]): Sheet {
  return {
    id: "sheet-panel",
    name: "Panel1",
    kind: "panel",
    size: "A3",
    orientation: "Landscape",
    zone_cols: 4,
    zone_rows: 6,
    title_block: {},
    revisions: [],
    entities: Object.fromEntries(entities.map((e) => [e.id, e])),
  };
}

function fp(
  id: string,
  reference: string,
  at: { x: number; y: number },
  opts: Partial<Footprint> = {},
): Entity {
  return {
    kind: "footprint",
    id,
    at,
    w: 20,
    h: 40,
    rotation: 0,
    reference,
    part_no: "",
    item_no: null,
    ...opts,
  };
}

describe("panel item numbers and legend", () => {
  // ja: アイテム番号は読み順(上の段から先に、段の中は左から右へ)に並ぶ。
  it("orders the parts for numbering top row first, then left to right", () => {
    const sheet = panelSheet([
      fp("b", "K2", { x: 200, y: 80 }),
      fp("a", "K1", { x: 100, y: 80 }),
      fp("c", "K3", { x: 150, y: 180 }),
    ]);
    expect(itemOrder(sheet).map((f) => f.reference)).toEqual(["K1", "K2", "K3"]);
  });

  // ja: 同じレールに乗った部品は、外形の高さが違っても同じ段として左から右へ数える。
  it("keeps parts on the same rail in one row even with different heights", () => {
    const sheet = panelSheet([
      fp("b", "F1", { x: 200, y: 90 }, { w: 15, h: 20 }),
      fp("a", "K1", { x: 100, y: 67.5 }, { w: 25, h: 65 }),
    ]);
    expect(itemOrder(sheet).map((f) => f.reference)).toEqual(["K1", "F1"]);
  });

  // ja: アイテム番号がまだ無いフットプリントにはバルーンが出ない。
  it("draws no balloon before the part is numbered", () => {
    const [entity] = Object.values(panelSheet([fp("a", "K1", { x: 100, y: 100 })]).entities);
    expect(entity.kind === "footprint" && balloon(entity)).toBeNull();
  });

  // ja: バルーンは左上角のすぐ外の丸で、短い引出線で角と結ばれる。
  it("puts the balloon just outside the top-left corner with a short leader", () => {
    const sheet = panelSheet([fp("a", "K1", { x: 100, y: 100 }, { item_no: 3 })]);
    const b = balloon(itemOrder(sheet)[0])!;
    expect(b.itemNo).toBe(3);
    expect(b.leaderTo).toEqual({ x: 100, y: 100 });
    const dist = (p: { x: number; y: number }) => Math.hypot(100 - p.x, 100 - p.y);
    expect(dist(b.leaderFrom)).toBeCloseTo(BALLOON_LEADER, 9);
    expect(dist(b.center)).toBeCloseTo(BALLOON_LEADER + BALLOON_R, 9);
  });

  // ja: 凡例表は採番済みの部品を番号順に並べ、未採番の部品は番号欄を空けて末尾に置く。
  it("lists numbered parts in order and unnumbered ones last", () => {
    const sheet = panelSheet([
      fp("b", "K2", { x: 200, y: 100 }, { item_no: 2, part_no: "MDK-RLY-24V" }),
      fp("a", "K1", { x: 100, y: 100 }, { item_no: 1, part_no: "MDK-CB-3P" }),
      fp("c", "F9", { x: 300, y: 100 }),
    ]);
    expect(legendRows(sheet)).toEqual([
      ["1", "K1", "MDK-CB-3P"],
      ["2", "K2", "MDK-RLY-24V"],
      ["", "F9", ""],
    ]);
  });

  // ja: 凡例表は行が増えるぶんだけ下へ伸び、幅は列幅の合計になる。
  it("grows the legend table one row at a time", () => {
    const legend = { id: "lg-1", at: { x: 300, y: 200 } };
    const empty = legendRect(panelSheet([]), legend);
    expect(empty).toEqual({
      x: 300,
      y: 200,
      w: LEGEND_COL_W.reduce((a, b) => a + b, 0),
      h: LEGEND_ROW_H,
    });
    const one = legendRect(panelSheet([fp("a", "K1", { x: 100, y: 100 }, { item_no: 1 })]), legend);
    expect(one.h - empty.h).toBe(LEGEND_ROW_H);
  });

  // ja: レールは盤の上から順に番号を振るので、配置表とプロパティが同じレールを指す。
  it("numbers the rails from the top of the panel down", () => {
    const sheet = panelSheet([
      { kind: "din_rail", id: "r2", y: 300, x1: 50, x2: 400 },
      { kind: "din_rail", id: "r1", y: 120, x1: 50, x2: 400 },
    ]);
    expect(numberedRails(sheet).map((r) => [r.index, r.rail.id])).toEqual([
      [1, "r1"],
      [2, "r2"],
    ]);
  });
});
