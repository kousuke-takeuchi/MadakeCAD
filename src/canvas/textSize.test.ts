import { describe, expect, it } from "vitest";
import { MIN_TEXT_PX, textPx } from "./textSize";

describe("drawing text size on screen", () => {
  // ja: 図面の文字は図形と同じ割合で拡大・縮小し、画面上で小さすぎて読めない大きさになったら描かない (大きさ 0)
  it("scales drawing text with the zoom like the shapes, and drops it once it is too small to read", () => {
    expect(textPx(2.5, 4)).toBe(10);
    expect(textPx(2.5, 8)).toBe(20);
    expect(textPx(2.5, 2)).toBe(5);
    expect(textPx(2.5, 1)).toBe(0);
    expect(textPx(MIN_TEXT_PX, 1)).toBe(MIN_TEXT_PX);
  });
});
