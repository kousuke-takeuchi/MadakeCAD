import { describe, expect, it } from "vitest";
import { MIN_TEXT_PX, drawingText, setDrawingFont, textPx } from "./textSize";

describe("drawing text size on screen", () => {
  // ja: 図面の文字は図形と同じ割合で拡大・縮小し、画面上で小さすぎて読めない大きさになったら描かない (大きさ 0)
  it("scales drawing text with the zoom like the shapes, and drops it once it is too small to read", () => {
    expect(textPx(2.5, 4)).toBe(10);
    expect(textPx(2.5, 8)).toBe(20);
    expect(textPx(2.5, 2)).toBe(5);
    expect(textPx(2.5, 1)).toBe(0);
    expect(textPx(MIN_TEXT_PX, 1)).toBe(MIN_TEXT_PX);
  });

  // ja: 読めないほど小さい文字はフォントを変えずに描かない。0px を指定すると WebKit は無視して直前の大きさで描くため
  it("skips unreadably small text without setting a 0px font, which WebKit ignores and would draw at the previous size", () => {
    const drawn: string[] = [];
    const ctx = { font: "10px sans-serif", fillText: (t: string) => drawn.push(t) } as unknown as CanvasRenderingContext2D;
    expect(setDrawingFont(ctx, 2.5, 1, "sans-serif")).toBe(0);
    drawingText(ctx, "小さい", 0, 0);
    expect(ctx.font).toBe("10px sans-serif");
    expect(setDrawingFont(ctx, 2.5, 4, "monospace", "600 ")).toBe(10);
    drawingText(ctx, "読める", 0, 0);
    expect(ctx.font).toBe("600 10px monospace");
    expect(drawn).toEqual(["読める"]);
  });
});
