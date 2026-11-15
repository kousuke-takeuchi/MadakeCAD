import { afterEach, describe, expect, it, vi } from "vitest";
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

  afterEach(() => {
    setActiveStrokeFont(null, "half");
    vi.unstubAllGlobals();
  });

  // 線の記録だけをする作り物の描画先。save/restore と translate/scale を追い、線の点を画面の座標で記録する
  function strokeCtx(align: CanvasTextAlign = "left", baseline: CanvasTextBaseline = "alphabetic") {
    const log: string[] = [];
    type State = { strokeStyle: string; lineWidth: number; lineCap: string; lineJoin: string; m: [number, number, number] };
    const stack: State[] = [];
    let m: [number, number, number] = [1, 0, 0]; // [倍率, 平行移動x, 平行移動y]
    const at = (x: number, y: number) => `${(m[1] + m[0] * x).toFixed(2)} ${(m[2] + m[0] * y).toFixed(2)}`;
    const ctx = {
      font: "10px sans-serif",
      textAlign: align,
      textBaseline: baseline,
      fillStyle: "#123456",
      strokeStyle: "#000",
      lineWidth: 1,
      lineCap: "butt",
      lineJoin: "miter",
      fillText: (t: string) => log.push(`text ${t}`),
      save: () => stack.push({ strokeStyle: ctx.strokeStyle, lineWidth: ctx.lineWidth, lineCap: ctx.lineCap, lineJoin: ctx.lineJoin, m }),
      restore: () => {
        const st = stack.pop()!;
        Object.assign(ctx, { strokeStyle: st.strokeStyle, lineWidth: st.lineWidth, lineCap: st.lineCap, lineJoin: st.lineJoin });
        m = st.m;
      },
      translate: (x: number, y: number) => {
        m = [m[0], m[1] + m[0] * x, m[2] + m[0] * y];
      },
      scale: (k: number) => {
        m = [m[0] * k, m[1], m[2]];
      },
      beginPath: () => log.push("begin"),
      moveTo: (x: number, y: number) => log.push(`M ${at(x, y)}`),
      lineTo: (x: number, y: number) => log.push(`L ${at(x, y)}`),
      stroke: (path?: unknown) => {
        log.push(path ? "stroke path" : "stroke");
        widths.push(ctx.lineWidth * m[0]);
        styles.push(`${ctx.strokeStyle} ${ctx.lineCap} ${ctx.lineJoin}`);
      },
    };
    const widths: number[] = [];
    const styles: string[] = [];
    return { ctx: ctx as unknown as CanvasRenderingContext2D, raw: ctx, log, widths, styles, depth: () => stack.length };
  }
  const font: StrokeFontData = { id: "t", baseline: 0, cap_top: 25, glyphs: { I: { wide: false, strokes: [[2, 0, 2, 25]] } } };
  // 字の下に格子の行が3つある作り物 (下揃えの確認用)
  const deep: StrokeFontData = { id: "d", baseline: 3, cap_top: 28, glyphs: { I: { wide: false, strokes: [[2, 3, 2, 28]] } } };

  // ja: ストロークフォントが有効なら、文字は文字の色・サイズに応じた太さの丸い線で、揃えに合わせた位置に描かれる
  it("strokes the text with the active stroke font, in the text colour, aligned like the text", () => {
    setActiveStrokeFont(font, "half");
    const { ctx, log, widths, styles } = strokeCtx("right");
    setDrawingFont(ctx, 2.5, 4, "sans-serif", "bold ");
    drawingText(ctx, "I", 100, 50);
    // 10px: 単位 = 7.2/25 = 0.288、幅 = 15 × 0.288 = 4.32 → 右揃えで x0 = 95.68、縦線は x = 95.68 + 0.576
    expect(log).toEqual(["begin", "M 96.26 50.00", "L 96.26 42.80", "stroke"]);
    expect(styles).toEqual(["#123456 round round"]);
    // 太さは mm で求めてから画面へ: max(0.07 × 2.5, 0.13) × 1.5 × 4 = 1.05 px
    expect(widths[0]).toBeCloseTo(1.05, 9);
  });

  // ja: ストロークフォントで描いた後も、線の色・太さ・端の形は描く前のまま残る (後に描く記号や配線の線が文字の線にならない)
  it("leaves the line style of the context as it was after stroking text", () => {
    setActiveStrokeFont(font, "half");
    const { ctx, raw, depth } = strokeCtx();
    setDrawingFont(ctx, 2.5, 4, "sans-serif");
    drawingText(ctx, "I", 10, 10);
    expect([raw.strokeStyle, raw.lineWidth, raw.lineCap, raw.lineJoin]).toEqual(["#000", 1, "butt", "miter"]);
    expect(depth()).toBe(0);
  });

  // ja: 線の太さは図面の mm で決まり (0.07 × 文字の高さ、0.13 mm 以上)、画面ではそれを拡大率倍した太さ (1px 未満にはしない)。SVG と同じ太さになる
  it("computes the stroke width in mm like the SVG and then scales it to the screen", () => {
    setActiveStrokeFont(font, "half");
    const { ctx, widths } = strokeCtx();
    // 1.5 mm × 8 px/mm: 0.07 × 1.5 = 0.105 < 0.13 → 0.13 mm × 8 = 1.04 px (px で下限を掛けると 0.84 px になってしまう)
    setDrawingFont(ctx, 1.5, 8, "sans-serif");
    drawingText(ctx, "I", 0, 0);
    // 10 mm × 4 px/mm: 0.7 mm × 4 = 2.8 px
    setDrawingFont(ctx, 10, 4, "sans-serif");
    drawingText(ctx, "I", 0, 0);
    // 2.5 mm × 2 px/mm: 0.175 mm × 2 = 0.35 px → 1 px (他の線と同じく 1px 未満にしない)
    setDrawingFont(ctx, 2.5, 2, "sans-serif");
    drawingText(ctx, "I", 0, 0);
    expect(widths.map((w) => Number(w.toFixed(6)))).toEqual([1.04, 2.8, 1]);
  });

  // ja: 線を1.5倍の太さにするのは bold の指定だけ。画面の文字だけを少し太くする 600 は、SVG と同じく線の太さを変えない
  it("thickens strokes only for bold, as the SVG does, not for the screen-only weight 600", () => {
    setActiveStrokeFont(font, "half");
    const { ctx, widths } = strokeCtx();
    for (const prefix of ["", "bold ", "600 "]) {
      setDrawingFont(ctx, 10, 4, "sans-serif", prefix);
      drawingText(ctx, "I", 0, 0);
    }
    expect(widths.map((w) => Number(w.toFixed(6)))).toEqual([2.8, 4.2, 2.8]);
  });

  // ja: 文字の縦の基準 (textBaseline) に合わせて線の字の位置を変える: middle は大文字の高さの中央、top は大文字の上端、bottom は字の格子の下端が指定の y に来る
  it("places stroked text by the text baseline of the context", () => {
    const ys = (f: StrokeFontData, b: CanvasTextBaseline) => {
      setActiveStrokeFont(f, "half");
      const { ctx, log } = strokeCtx("left", b);
      setDrawingFont(ctx, 2.5, 4, "sans-serif");
      drawingText(ctx, "I", 0, 50);
      return log.filter((l) => l.startsWith("M") || l.startsWith("L")).map((l) => l.split(" ")[2]);
    };
    // 10px: 大文字の高さ 7.2px
    expect(ys(font, "alphabetic")).toEqual(["50.00", "42.80"]);
    expect(ys(font, "middle")).toEqual(["53.60", "46.40"]);
    expect(ys(font, "top")).toEqual(["57.20", "50.00"]);
    expect(ys(font, "hanging")).toEqual(["57.20", "50.00"]);
    expect(ys(font, "bottom")).toEqual(["50.00", "42.80"]);
    // ベースラインの下に3行 (3 × 0.288 = 0.864px) ある字は、下揃えでその分だけ上がる
    expect(ys(deep, "bottom")).toEqual(["49.14", "41.94"]);
    expect(ys(deep, "ideographic")).toEqual(["49.14", "41.94"]);
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

  // ja: Path2D が使える画面では、同じ文字列の線の形を一度だけ作り、大きさや位置が違っても使い回す
  it("builds the outline of a string once and reuses it at other sizes and places when Path2D exists", () => {
    const built: string[] = [];
    class FakePath2D {
      moveTo(x: number, y: number) {
        built.push(`M ${x.toFixed(3)} ${y.toFixed(3)}`);
      }
      lineTo(x: number, y: number) {
        built.push(`L ${x.toFixed(3)} ${y.toFixed(3)}`);
      }
    }
    vi.stubGlobal("Path2D", FakePath2D);
    setActiveStrokeFont(font, "half");
    const { ctx, log } = strokeCtx();
    setDrawingFont(ctx, 2.5, 4, "sans-serif");
    drawingText(ctx, "I", 0, 0);
    setDrawingFont(ctx, 5, 4, "sans-serif");
    drawingText(ctx, "I", 30, 40);
    // 大きさ1で作った線 (単位 0.72/25 = 0.0288)
    expect(built).toEqual(["M 0.058 0.000", "L 0.058 -0.720"]);
    expect(log).toEqual(["stroke path", "stroke path"]);
  });
});
