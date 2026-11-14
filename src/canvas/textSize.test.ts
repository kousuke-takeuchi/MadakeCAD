import { afterEach, describe, expect, it } from "vitest";
import type { StrokeFontData } from "../ipc";
import { setActiveStrokeFont } from "./strokeFont";
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

  afterEach(() => setActiveStrokeFont(null, "half"));

  // 線の記録だけをする作り物の描画先
  function strokeCtx(align: CanvasTextAlign = "left") {
    const log: string[] = [];
    const ctx = {
      font: "10px sans-serif",
      textAlign: align,
      fillStyle: "#123456",
      strokeStyle: "#000",
      lineWidth: 1,
      lineCap: "butt",
      lineJoin: "miter",
      fillText: (t: string) => log.push(`text ${t}`),
      beginPath: () => log.push("begin"),
      moveTo: (x: number, y: number) => log.push(`M ${x.toFixed(2)} ${y.toFixed(2)}`),
      lineTo: (x: number, y: number) => log.push(`L ${x.toFixed(2)} ${y.toFixed(2)}`),
      stroke: () => log.push("stroke"),
    };
    return { ctx: ctx as unknown as CanvasRenderingContext2D, raw: ctx, log };
  }
  const font: StrokeFontData = { id: "t", baseline: 0, cap_top: 25, glyphs: { I: { wide: false, strokes: [[2, 0, 2, 25]] } } };

  // ja: ストロークフォントが有効なら、文字は文字の色・サイズに応じた太さの丸い線で、揃えに合わせた位置に描かれる
  it("strokes the text with the active stroke font, in the text colour, aligned like the text", () => {
    setActiveStrokeFont(font, "half");
    const { ctx, raw, log } = strokeCtx("right");
    setDrawingFont(ctx, 2.5, 4, "sans-serif", "bold ");
    drawingText(ctx, "I", 100, 50);
    // 10px: 単位 = 7.2/25 = 0.288、幅 = 15 × 0.288 = 4.32 → 右揃えで x0 = 95.68、縦線は x = 95.68 + 0.576
    expect(log).toEqual(["begin", "M 96.26 50.00", "L 96.26 42.80", "stroke"]);
    expect(raw.strokeStyle).toBe("#123456");
    expect(raw.lineWidth).toBeCloseTo(1.05, 9);
    expect(raw.lineCap).toBe("round");
  });

  // ja: フォントに無い字を含む文字列と、ストロークフォントが無いときは、従来どおり文字として描く
  it("falls back to filled text for missing glyphs and without a stroke font", () => {
    const { ctx, log } = strokeCtx();
    setDrawingFont(ctx, 2.5, 4, "sans-serif");
    drawingText(ctx, "I", 0, 0);
    setActiveStrokeFont(font, "half");
    drawingText(ctx, "IX", 0, 0);
    expect(log).toEqual(["text I", "text IX"]);
  });
});
