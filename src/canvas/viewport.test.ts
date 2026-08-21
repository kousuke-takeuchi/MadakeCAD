import { describe, it, expect } from "vitest";
import { Viewport } from "./viewport";

describe("Viewport", () => {
  // ja: ワールド⇔スクリーン変換は往復可能で、ズームはアンカー点を固定する
  it("world<->screen conversion round-trips and zoom keeps the anchor fixed", () => {
    const vp = new Viewport();
    vp.scale = 4;
    vp.originX = 10;
    vp.originY = 20; // 1mm = 4px
    const s = vp.toScreen({ x: 100, y: 50 });
    const w = vp.toWorld(s);
    expect(w.x).toBeCloseTo(100);
    expect(w.y).toBeCloseTo(50);
    const anchor = { x: 300, y: 200 };
    const before = vp.toWorld(anchor);
    vp.zoomAt(anchor, 1.25);
    expect(vp.scale).toBeCloseTo(5);
    const after = vp.toWorld(anchor);
    expect(after.x).toBeCloseTo(before.x);
    expect(after.y).toBeCloseTo(before.y);
  });

  // ja: パンはスクリーンpx単位でビューを動かす
  it("panning moves the view in screen pixels", () => {
    const vp = new Viewport();
    vp.scale = 2;
    const before = vp.toScreen({ x: 0, y: 0 });
    vp.pan(30, -10);
    const after = vp.toScreen({ x: 0, y: 0 });
    expect(after.x - before.x).toBeCloseTo(30);
    expect(after.y - before.y).toBeCloseTo(-10);
  });

  // ja: スナップは既定で2.5mmグリッドへ丸める
  it("snapping rounds to the 2.5 mm grid by default", () => {
    const vp = new Viewport();
    expect(vp.snap({ x: 101.2, y: 48.9 })).toEqual({ x: 100, y: 50 });
    expect(vp.snap({ x: 3.7, y: -1.3 }, 5)).toEqual({ x: 5, y: -0 });
  });

  // ja: ズーム倍率は妥当な範囲に制限される
  it("zoom is clamped to sane bounds", () => {
    const vp = new Viewport();
    vp.zoomAt({ x: 0, y: 0 }, 1e9);
    expect(vp.scale).toBeLessThanOrEqual(200);
    vp.zoomAt({ x: 0, y: 0 }, 1e-12);
    expect(vp.scale).toBeGreaterThanOrEqual(0.05);
  });
});
