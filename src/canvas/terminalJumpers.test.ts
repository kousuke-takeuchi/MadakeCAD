// Rust側 madake-core/src/terminal_chart.rs の parse_jumpers / jumper_bar_segments と
// 同一ルールを検証する。
import { describe, expect, it } from "vitest";

import type { Primitive, SymbolDef } from "../ipc";
import { dynamicSymbol, TERMINAL_MARK_R } from "./dynamicSymbol";
import {
  formatJumperSpec,
  JUMPER_BAR_DOT_R_MM,
  JUMPER_BAR_STROKE_MM,
  jumperBars,
  parseJumperSpec,
  terminalNumbers,
} from "./terminalJumpers";

describe("parseJumperSpec", () => {
  // ja: 隣り合う端子どうしのジャンパは正規化され、隣り合わない組は捨てられる
  it("keeps jumpers between adjacent terminals and drops non-adjacent ones", () => {
    expect(parseJumperSpec("2-1")).toEqual([["1", "2"]]);
    expect(parseJumperSpec("1-3")).toEqual([]);
  });

  // ja: 複数のジャンパはそれぞれ組として残り、端子番号順に並ぶ
  it("keeps every jumper when there are several, in terminal order", () => {
    expect(parseJumperSpec("3-4,1-2")).toEqual([
      ["1", "2"],
      ["3", "4"],
    ]);
    expect(formatJumperSpec(parseJumperSpec("3-4,1-2"))).toBe("1-2,3-4");
  });

  // ja: 空文字や読めない記述はジャンパ無しとして扱う
  it("treats empty or unreadable text as no jumpers", () => {
    expect(parseJumperSpec("")).toEqual([]);
    expect(parseJumperSpec("abc,1-,1-2-3")).toEqual([]);
  });

  // ja: 多段端子台では段記号付きのジャンパ(同じ極の段をまたぐ組・同じ段の隣の極)だけが通る
  it("accepts level-aware jumpers on a multi-level block", () => {
    const terminals = terminalNumbers(dynamicSymbol("terminal_block_4p_2l")!);
    expect(parseJumperSpec("1b-1a", terminals)).toEqual([["1a", "1b"]]);
    expect(parseJumperSpec("1a-2a", terminals)).toEqual([["1a", "2a"]]);
  });

  // ja: 多段端子台に段記号なしの指定("1-2")を書いても、どの段か決まらないので捨てられる
  it("drops a jumper written without its level letter on a multi-level block", () => {
    const terminals = terminalNumbers(dynamicSymbol("terminal_block_4p_2l")!);
    expect(parseJumperSpec("1-2", terminals)).toEqual([]);
  });

  // ja: 端子台に無い段を指すジャンパ("1a-1c")と斜めの組("1a-2b")は捨てられる
  it("drops a jumper to a level the block does not have and a diagonal one", () => {
    const terminals = terminalNumbers(dynamicSymbol("terminal_block_4p_2l")!);
    expect(parseJumperSpec("1a-1c", terminals)).toEqual([]);
    expect(parseJumperSpec("1a-2b", terminals)).toEqual([]);
  });
});

describe("jumper bar dimensions", () => {
  // ja: ジャンパバーの寸法(線幅・両端の丸の半径・中心線からのずらし量)はRust側と同じ数値になる
  it("keeps the same millimetre values as the Rust side", () => {
    // madake-core/src/terminal_chart.rs の JUMPER_BAR_STROKE / _DOT_R と symbol.rs の TERMINAL_MARK_R
    expect([JUMPER_BAR_STROKE_MM, JUMPER_BAR_DOT_R_MM, TERMINAL_MARK_R]).toEqual([0.4, 0.3, 1.8]);
  });
});

describe("jumperBars", () => {
  // ja: 同じ列の2端子を繋ぐジャンパは、列の中心線上の2つの印の隙間だけに描かれる
  it("is drawn in the gap between the two marks, on the column's centre line", () => {
    const def = dynamicSymbol("terminal_block_4p")!;
    const bars = jumperBars(def, [["1", "2"]]);
    expect(bars).toHaveLength(1);
    const [a, b] = bars[0].bar;
    expect([a.x, b.x]).toEqual([0, 0]);
    expect(a.y).toBeCloseTo(-7.5 + TERMINAL_MARK_R, 9);
    expect(b.y).toBeCloseTo(-2.5 - TERMINAL_MARK_R, 9);
  });

  // ja: 両端の丸はそれぞれ自分の端子の印に外接する位置に付く
  it("puts each end mark just outside its own terminal's mark", () => {
    const def = dynamicSymbol("terminal_block_4p")!;
    const [{ dots }] = jumperBars(def, [["1", "2"]]);
    expect(dots[0].y).toBeCloseTo(-7.5 + TERMINAL_MARK_R + JUMPER_BAR_DOT_R_MM, 9);
    expect(dots[1].y).toBeCloseTo(-2.5 - TERMINAL_MARK_R - JUMPER_BAR_DOT_R_MM, 9);
  });

  // ja: 段をまたぐジャンパは、極の行の上、2つの段の列の印の隙間に描かれる
  it("lies on the pole's row, between the two level columns' marks", () => {
    const def = dynamicSymbol("terminal_block_4p_2l")!;
    const [{ bar }] = jumperBars(def, [["1a", "1b"]]);
    expect(bar[0].y).toBeCloseTo(bar[1].y, 9);
    expect(bar[0].x).toBeCloseTo(-5 + TERMINAL_MARK_R, 9);
    expect(bar[1].x).toBeCloseTo(5 - TERMINAL_MARK_R, 9);
  });

  // ja: 両端の丸は自分の端子の印に一番近く、ほかの端子の印には触れない
  it("gives each end mark to the terminal it belongs to", () => {
    for (const [id, spec] of [
      ["terminal_block_4p", "1-2,2-3"],
      ["terminal_block_4p_2l", "1a-1b,2a-3a"],
      ["terminal_block_12p_2l", "10a-10b,11a-12a"],
    ] as const) {
      const def = dynamicSymbol(id)!;
      const numbers = terminalNumbers(def);
      const pairs = parseJumperSpec(spec, numbers);
      const centre = (no: string) => {
        const pts = def.pins.filter((q) => q.number === no).map((q) => q.at);
        return {
          x: pts.reduce((t, q) => t + q.x, 0) / pts.length,
          y: pts.reduce((t, q) => t + q.y, 0) / pts.length,
        };
      };
      const bars = jumperBars(def, pairs);
      expect(bars).toHaveLength(pairs.length);
      bars.forEach(({ dots }, i) => {
        dots.forEach((dot, side) => {
          const owner = pairs[i][side];
          const c = centre(owner);
          const own = Math.hypot(dot.x - c.x, dot.y - c.y);
          for (const no of numbers) {
            if (no === owner) continue;
            const o = centre(no);
            const other = Math.hypot(dot.x - o.x, dot.y - o.y);
            expect(own, `${id} ${spec}: ${owner} vs ${no}`).toBeLessThan(other);
            expect(Math.abs(other - TERMINAL_MARK_R)).toBeGreaterThan(JUMPER_BAR_DOT_R_MM);
          }
        });
      });
    }
  });

  // ja: ジャンパバーと両端の丸は、どの端子番号の文字にも重ならない
  it("never covers a terminal number", () => {
    // 文字は中心揃え・ベースラインが端子の中心。幅は文字数x文字高x0.6、高さは文字高x0.7で見る
    const numberBoxes = (def: SymbolDef) =>
      def.primitives
        .filter((q): q is Extract<Primitive, { type: "text" }> => q.type === "text")
        .map((q) => {
          const baseline = q.at.y + q.height / 2;
          const half = (q.text.length * q.height * 0.6) / 2;
          return [q.at.x - half, baseline - q.height * 0.7, q.at.x + half, baseline] as const;
        });
    const overlaps = (a: readonly number[], b: readonly number[]) =>
      a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3];
    for (const [id, spec] of [
      ["terminal_block_4p", "1-2,3-4"],
      ["terminal_block_4p", "1-2,2-3"],
      ["terminal_block_4p_2l", "1a-1b,2a-3a"],
      ["terminal_block_4p_3l", "1a-1b,1b-1c"],
      ["terminal_block_12p_2l", "10a-10b,11a-12a"],
    ] as const) {
      const def = dynamicSymbol(id)!;
      const pairs = parseJumperSpec(spec, terminalNumbers(def));
      const graphics: (readonly number[])[] = [];
      for (const { bar, dots } of jumperBars(def, pairs)) {
        const h = JUMPER_BAR_STROKE_MM / 2;
        const [a, b] = bar;
        graphics.push([
          Math.min(a.x, b.x) - h,
          Math.min(a.y, b.y) - h,
          Math.max(a.x, b.x) + h,
          Math.max(a.y, b.y) + h,
        ]);
        for (const q of dots) {
          const r = JUMPER_BAR_DOT_R_MM;
          graphics.push([q.x - r, q.y - r, q.x + r, q.y + r]);
        }
      }
      expect(graphics.length).toBeGreaterThan(0);
      for (const g of graphics) {
        for (const box of numberBoxes(def)) {
          expect(overlaps(g, box), `${id} ${spec}: ${g} vs ${box}`).toBe(false);
        }
      }
    }
  });

  // ja: 複数のジャンパはそれぞれ独立した線分になる
  it("produces one segment per jumper when there are several", () => {
    const def = dynamicSymbol("terminal_block_4p")!;
    expect(
      jumperBars(def, [
        ["1", "2"],
        ["3", "4"],
      ]),
    ).toHaveLength(2);
  });

  // ja: 端子台に無い端子番号を指すジャンパは線分を作らない
  it("produces no segment for a jumper to a terminal the block does not have", () => {
    const def = dynamicSymbol("terminal_block_4p")!;
    expect(jumperBars(def, [["4", "5"]])).toEqual([]);
  });
});
