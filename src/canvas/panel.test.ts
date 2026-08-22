// 2D盤レイアウトの幾何計算の仕様 (docs/internal/specs/m4-industrial-core.md §8)。
// Rust側 madake-core/src/panel.rs と同じ数式で、キャンバスとSVG出力の見た目を一致させる。
import { describe, expect, it } from "vitest";

import type { DinRail, Duct, Footprint, PanelOutline } from "../ipc";
import {
  DEFAULT_FOOTPRINT_SIZE,
  DIN_RAIL_H,
  PLATE_INSET,
  ductHatch,
  footprintRect,
  footprintSize,
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
