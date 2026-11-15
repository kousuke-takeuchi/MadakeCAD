// ストロークフォントの配置 (Rust側 stroke_font.rs の layout と同じ規則・同じ数値)。
import { afterEach, describe, expect, it } from "vitest";

import shared from "../../src-tauri/crates/madake-core/tests/fixtures/stroke_layout_cases.json";
import type { Latin, StrokeFontData } from "../ipc";
import { textPenOf, textSpacingOf, textStrokeWidth, withTextSpacing } from "./strokeFont";
import {
  UNIT_CACHE_MAX,
  activeStrokeFont,
  activeUnitStroke,
  layoutStroke,
  setActiveStrokeFont,
  strokeFontLoader,
  strokeWidth,
  textAdvanceMm,
  unitStrokeCacheSize,
  withLatin,
  withSystemFont,
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

describe("canvas layout cache", () => {
  // ja: キャンバス用の配置は大きさ1で一度だけ求めて覚え、同じ文字列・字間なら同じものを返す。フォントに無い字を含む文字列も「無い」と覚える
  it("lays a string out once at size 1 and returns the same entry again", () => {
    expect(activeUnitStroke("H")).toBeNull();
    setActiveStrokeFont(font, "half");
    const a = activeUnitStroke("HH")!;
    expect(a.width).toBeCloseTo(0.864, 9);
    expect(a.lines[0]).toEqual([expect.closeTo(2 * 0.0288, 9), expect.closeTo(0, 9), expect.closeTo(2 * 0.0288, 9), expect.closeTo(-0.72, 9)]);
    expect(activeUnitStroke("HH")).toBe(a);
    expect(activeUnitStroke("HH", 1.5)).not.toBe(a);
    expect(activeUnitStroke("H😀")).toBeNull();
    expect(unitStrokeCacheSize()).toBe(3);
  });

  // ja: 英数字の字 (半角・全角) が違えば別の配置になる
  it("keeps half- and full-width Latin layouts apart", () => {
    setActiveStrokeFont(font, "half");
    const half = activeUnitStroke("H")!;
    const full = withLatin("full", () => activeUnitStroke("H"))!;
    expect(half.width).toBeCloseTo(0.432, 9);
    expect(full.width).toBeCloseTo(0.864, 9);
  });

  // ja: 書体を切り替えると覚えた配置を捨てる (同じidで中身の違うフォントでも古い線を描かない)
  it("forgets the cached layouts when the active font changes", () => {
    setActiveStrokeFont(font, "half");
    activeUnitStroke("H");
    expect(unitStrokeCacheSize()).toBe(1);
    const narrow: StrokeFontData = { ...font, glyphs: { H: { wide: false, strokes: [[1, 0, 1, 25]] } } };
    setActiveStrokeFont(narrow, "half");
    expect(unitStrokeCacheSize()).toBe(0);
    expect(activeUnitStroke("H")!.lines[0][0]).toBeCloseTo(0.0288, 9);
  });

  // ja: 覚える文字列の数には上限があり、超えると古いものから捨てる
  it("bounds the cache and drops the oldest entries first", () => {
    setActiveStrokeFont(font, "half");
    const first = activeUnitStroke("H");
    for (let i = 0; i < UNIT_CACHE_MAX + 10; i++) activeUnitStroke("H".repeat((i % 50) + 2) + " ".repeat(Math.floor(i / 50)));
    expect(unitStrokeCacheSize()).toBe(UNIT_CACHE_MAX);
    expect(activeUnitStroke("H")).not.toBe(first);
  });
});

describe("system font for previews", () => {
  // ja: プレビューの描画の間だけはシステムの書体で描き (線の字にしない)、終わると (例外でも) プロジェクトの書体へ戻す
  it("draws with the system font inside withSystemFont and restores the font afterwards", () => {
    setActiveStrokeFont(font, "half");
    expect(withSystemFont(() => [activeStrokeFont(), activeUnitStroke("H"), textAdvanceMm("HH", 10)])).toEqual([
      null,
      null,
      textWidthMm("HH", 10),
    ]);
    expect(activeStrokeFont()?.font.id).toBe("t");
    expect(() =>
      withSystemFont(() => {
        throw new Error("x");
      }),
    ).toThrow("x");
    expect(activeStrokeFont()?.font.id).toBe("t");
  });
});

describe("glyph loading", () => {
  // ja: 字形は1つのidにつき1回だけ取りに行き、成功した結果を使い回す
  it("fetches the glyphs of a font once and reuses them", async () => {
    let calls = 0;
    const load = strokeFontLoader(async () => {
      calls++;
      return font;
    });
    expect(await load("t")).toBe(font);
    expect(await load("t")).toBe(font);
    expect(calls).toBe(1);
  });

  // ja: 取得に失敗したら null を返し、その失敗は覚えない (次に読み込むときにまた取りに行く)
  it("returns null for a failed fetch and tries again next time", async () => {
    let calls = 0;
    const load = strokeFontLoader(async () => {
      calls++;
      if (calls === 1) throw new Error("offline");
      return font;
    });
    expect(await load("t")).toBeNull();
    expect(await load("t")).toBe(font);
    expect(calls).toBe(2);
  });
});

describe("shared layout cases", () => {
  // ja: 配置は Rust 側と共有のケース表のとおりの幅と最初の点になる (二つの描画で字の位置が食い違わない)
  it("gives the numbers of the case file shared with the Rust layout", () => {
    const f = shared.font as StrokeFontData;
    expect(shared.cases.length).toBeGreaterThanOrEqual(10);
    for (const c of shared.cases) {
      const t = layoutStroke(f, c.text, c.size, c.latin as Latin, c.spacing);
      if (c.width === null) {
        expect(t, c.text).toBeNull();
        continue;
      }
      expect(t, c.text).not.toBeNull();
      expect(t!.width, c.text).toBeCloseTo(c.width, 9);
      const first = t!.polylines[0]?.[0] ?? null;
      if (c.first === null) {
        expect(first, c.text).toBeNull();
      } else {
        expect(first![0], c.text).toBeCloseTo(c.first[0], 9);
        expect(first![1], c.text).toBeCloseTo(c.first[1], 9);
      }
    }
  });
});

describe("text pen", () => {
  const tiny: StrokeFontData = { id: "t", baseline: 0, cap_top: 25, glyphs: { I: { wide: false, strokes: [[2, 0, 2, 25]] } } };

  // ja: 書体にペンの太さがあると、文字は大きさによらずその太さで描かれ、太字は1.5倍になる。無ければ太さは大きさに比例する
  it("strokes every text at the pen width when the font has one", () => {
    setActiveStrokeFont(tiny, "half");
    expect(textStrokeWidth(10, false)).toBeCloseTo(0.7, 9);
    setActiveStrokeFont(tiny, "half", 0.35);
    expect(textStrokeWidth(10, false)).toBe(0.35);
    expect(textStrokeWidth(2, false)).toBe(0.35);
    expect(textStrokeWidth(2, true)).toBeCloseTo(0.525, 9);
    setActiveStrokeFont(null, "half");
    expect(textStrokeWidth(10, false)).toBeCloseTo(0.7, 9);
  });

  // ja: 書体の設定のペンの太さは0.1〜1.0mmだけを使う。範囲外・未設定は無いものとして扱う
  it("reads a pen width only inside 0.1 to 1.0 mm", () => {
    expect(textPenOf({ pen_mm: 0.35 })).toBe(0.35);
    expect(textPenOf({ pen_mm: 5 })).toBeNull();
    expect(textPenOf({ pen_mm: null })).toBeNull();
    expect(textPenOf({})).toBeNull();
    expect(textPenOf(null)).toBeNull();
  });
});

describe("letter spacing of the font and of a note", () => {
  const tiny: StrokeFontData = { id: "t", baseline: 0, cap_top: 25, glyphs: { I: { wide: false, strokes: [[2, 0, 2, 25]] } } };

  // ja: 書体の字間は全ての文字の送りに掛かり、注記の字間はさらにそれに掛かる。字間1のときの幅を基準に、送りの分だけ幅が変わる
  it("multiplies the advance by the font spacing and then by the note spacing", () => {
    setActiveStrokeFont(tiny, "half");
    const base = textAdvanceMm("III", 10);
    setActiveStrokeFont(tiny, "half", null, 0.5);
    const tight = textAdvanceMm("III", 10);
    expect(tight).toBeLessThan(base);
    const noteWide = withTextSpacing(2, () => textAdvanceMm("III", 10));
    expect(noteWide).toBeCloseTo(base, 9);
    expect(textAdvanceMm("III", 10)).toBeCloseTo(tight, 9);
    expect(withTextSpacing(1, () => textAdvanceMm("III", 10))).toBeCloseTo(tight, 9);
  });

  // ja: 字間は0.3〜3だけを使う。範囲外・未設定は1として扱う
  it("reads a spacing only inside 0.3 to 3", () => {
    expect(textSpacingOf({ spacing: 0.65 })).toBe(0.65);
    expect(textSpacingOf({ spacing: 0.1 })).toBe(1);
    expect(textSpacingOf({ spacing: null })).toBe(1);
    expect(textSpacingOf(null)).toBe(1);
  });
});

