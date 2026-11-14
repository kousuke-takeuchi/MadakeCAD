// ストロークフォントの配置 (Rust側 stroke_font.rs の layout と同じ規則・同じ数値)。
import { afterEach, describe, expect, it } from "vitest";

import type { StrokeFontData } from "../ipc";
import {
  activeStrokeFont,
  layoutStroke,
  setActiveStrokeFont,
  strokeWidth,
  textAdvanceMm,
  withLatin,
} from "./strokeFont";
import { textWidthMm } from "./textWidth";

// 小さな作り物のフォント: "H" は高さ0〜25の縦線2本と横線1本 (baseline 0, cap_top 25)、"亜" は全角の四角
const font: StrokeFontData = {
  id: "t",
  baseline: 0,
  cap_top: 25,
  glyphs: {
    H: { wide: false, strokes: [[2, 0, 2, 25], [12, 0, 12, 25], [2, 12, 12, 12]] },
    A: { wide: false, strokes: [[1, 0, 7, 25, 13, 0]] },
    "亜": { wide: true, strokes: [[1, 1, 29, 1, 29, 29, 1, 29, 1, 1]] },
    "Ｈ": { wide: true, strokes: [[4, 0, 4, 25]] },
    " ": { wide: false, strokes: [] },
    "　": { wide: true, strokes: [] },
  },
};

afterEach(() => setActiveStrokeFont(null, "half"));

describe("stroke font layout", () => {
  // ja: 半角の大文字の高さはフォントサイズの0.72倍、送りは0.432倍。全角の字は2倍送る
  it("scales glyphs so that capitals are 0.72 of the size", () => {
    const t = layoutStroke(font, "H", 10, "half")!;
    const ys = t.polylines.flat().map((p) => p[1]);
    expect(Math.min(...ys)).toBeCloseTo(-7.2, 9);
    expect(Math.max(...ys)).toBeCloseTo(0, 9);
    expect(t.width).toBeCloseTo(4.32, 9);
    expect(layoutStroke(font, "亜", 10, "half")!.width).toBeCloseTo(8.64, 9);
    expect(layoutStroke(font, "HH", 10, "half")!.width).toBeCloseTo(8.64, 9);
    // 2文字目は1文字目の送りの分だけ右から始まる
    expect(layoutStroke(font, "HH", 10, "half")!.polylines[3][0][0]).toBeCloseTo(4.32 + 2 * 0.288, 9);
  });

  // ja: 英数字を全角にするとASCIIは全角の字で描かれ、全角の字が無い文字は半角の字で描く
  it("uses full-width forms for Latin letters and falls back to half width", () => {
    const h = layoutStroke(font, "H", 10, "full")!;
    expect(h.width).toBeCloseTo(8.64, 9);
    expect(h.polylines).toEqual([[[4 * 0.288, 0], [4 * 0.288, -7.2]]].map((l) => l.map(([x, y]) => [expect.closeTo(x, 9), expect.closeTo(y, 9)])));
    expect(layoutStroke(font, "A", 10, "full")!.width).toBeCloseTo(4.32, 9);
    expect(layoutStroke(font, "H H", 10, "full")!.width).toBeCloseTo(25.92, 9);
  });

  // ja: 字間は文字の間だけを広げ、最後の字の後ろは広げない。空白は線なしで送る
  it("widens only the gaps with spacing, and spaces advance without strokes", () => {
    expect(layoutStroke(font, "HH", 10, "half", 1.5)!.width).toBeCloseTo(4.32 * 1.5 + 4.32, 9);
    const sp = layoutStroke(font, "  ", 10, "half")!;
    expect(sp.polylines).toEqual([]);
    expect(sp.width).toBeCloseTo(8.64, 9);
    expect(layoutStroke(font, "", 10, "half")).toEqual({ polylines: [], width: 0 });
  });

  // ja: フォントに無い字を1つでも含む文字列は配置しない
  it("returns null when any character is missing", () => {
    expect(layoutStroke(font, "H😀", 10, "half")).toBeNull();
  });

  // ja: 線の太さはフォントサイズの0.07倍で0.13より細くならず、太字は1.5倍
  it("computes the stroke width from the size", () => {
    expect(strokeWidth(10, false)).toBeCloseTo(0.7, 9);
    expect(strokeWidth(1, false)).toBeCloseTo(0.13, 9);
    expect(strokeWidth(10, true)).toBeCloseTo(1.05, 9);
  });

  // ja: 文字の幅は、フォントが有効な間だけ線の幅になり、外すと見積もりの幅へ戻る
  it("measures with the stroke font only while it is active", () => {
    expect(textAdvanceMm("HH", 10)).toBeCloseTo(textWidthMm("HH", 10), 9);
    setActiveStrokeFont(font, "half");
    expect(activeStrokeFont()?.font.id).toBe("t");
    expect(textAdvanceMm("HH", 10)).toBeCloseTo(8.64, 9);
    expect(textAdvanceMm("HH", 10, 1.5)).toBeCloseTo(10.8, 9);
    expect(textAdvanceMm("H😀", 10)).toBeCloseTo(textWidthMm("H😀", 10), 9);
    setActiveStrokeFont(null, "half");
    expect(textAdvanceMm("HH", 10)).toBeCloseTo(textWidthMm("HH", 10), 9);
  });

  // ja: withLatin は有効なフォントの英数字の字を一時的に上書きし、終わると元へ戻す
  it("overrides the Latin form for a while and restores it", () => {
    setActiveStrokeFont(font, "half");
    const inside = withLatin("full", () => textAdvanceMm("H", 10));
    expect(inside).toBeCloseTo(8.64, 9);
    expect(activeStrokeFont()?.latin).toBe("half");
    expect(withLatin(undefined, () => textAdvanceMm("H", 10))).toBeCloseTo(4.32, 9);
  });
});
