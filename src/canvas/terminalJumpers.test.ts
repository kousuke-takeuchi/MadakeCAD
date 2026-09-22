// Rust側 madake-core/src/terminal_chart.rs の parse_jumpers / jumper_bars と
// 同一ルールを検証する。
import { describe, expect, it } from "vitest";

import type { Primitive, SymbolDef } from "../ipc";
import { dynamicSymbol, TERMINAL_MARK_R, TERMINAL_NUMBER_HEIGHT } from "./dynamicSymbol";
import {
  formatJumperSpec,
  JUMPER_BAR_STROKE_MM,
  jumperBars,
  parseJumperSpec,
  terminalCenter,
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
  // ja: ジャンパバーの寸法(線幅・端子の印の半径・端子番号の文字高さ)はRust側と同じ数値になる
  it("keeps the same millimetre values as the Rust side", () => {
    // madake-core/src/terminal_chart.rs の JUMPER_BAR_STROKE と
    // symbol.rs の TERMINAL_MARK_R / TERMINAL_NUMBER_HEIGHT
    expect([JUMPER_BAR_STROKE_MM, TERMINAL_MARK_R, TERMINAL_NUMBER_HEIGHT]).toEqual([
      0.8, 1.8, 2.0,
    ]);
  });
});

describe("jumperBars", () => {
  // ja: 同じ列の2端子を繋ぐジャンパは、列の中心線上の2つの印の隙間だけに描かれる
  it("is drawn in the gap between the two marks, on the column's centre line", () => {
    const def = dynamicSymbol("terminal_block_4p")!;
    const bars = jumperBars(def, [["1", "2"]]);
    expect(bars).toHaveLength(1);
    const [a, b] = bars[0];
    expect([a.x, b.x]).toEqual([0, 0]);
    expect(a.y).toBeCloseTo(-7.5 + TERMINAL_MARK_R, 9);
    expect(b.y).toBeCloseTo(-2.5 - TERMINAL_MARK_R, 9);
  });

  // ja: ジャンパバーの長さは、その端子台の端子の間隔から両端の印の半径を引いた長さちょうどになる
  it("is exactly as long as the gap between the two marks it joins", () => {
    for (const [id, a, b] of [
      ["terminal_block_4p", "1", "2"],
      ["terminal_block_4p_2l", "1a", "2a"],
      ["terminal_block_4p_2l", "1a", "1b"],
    ] as const) {
      const def = dynamicSymbol(id)!;
      // 端子の間隔はシンボル定義のピンの位置から取る(図面上の実際の間隔)
      const ca = terminalCenter(def, a)!;
      const cb = terminalCenter(def, b)!;
      const pitch = Math.hypot(cb.x - ca.x, cb.y - ca.y);
      const [[p, q]] = jumperBars(def, [[a, b]]);
      expect(Math.hypot(q.x - p.x, q.y - p.y), `${id} ${a}-${b}`).toBeCloseTo(
        pitch - 2 * TERMINAL_MARK_R,
        9,
      );
    }
  });

  // ja: 段をまたぐジャンパは、極の行の上、2つの段の列の印の隙間に描かれる
  it("lies on the pole's row, between the two level columns' marks", () => {
    const def = dynamicSymbol("terminal_block_4p_2l")!;
    const [bar] = jumperBars(def, [["1a", "1b"]]);
    expect(bar[0].y).toBeCloseTo(bar[1].y, 9);
    expect(bar[0].x).toBeCloseTo(-5 + TERMINAL_MARK_R, 9);
    expect(bar[1].x).toBeCloseTo(5 - TERMINAL_MARK_R, 9);
  });

  // ja: バーの両端はちょうど自分の端子の印の縁の上にあり、ほかのどの端子よりも自分の端子に近い
  it("ends exactly on the edge of the mark of the terminal it belongs to", () => {
    for (const [id, spec] of [
      ["terminal_block_4p", "1-2,2-3"],
      ["terminal_block_4p_2l", "1a-1b,2a-3a"],
      ["terminal_block_12p_2l", "10a-10b,11a-12a"],
    ] as const) {
      const def = dynamicSymbol(id)!;
      const numbers = terminalNumbers(def);
      const pairs = parseJumperSpec(spec, numbers);
      const bars = jumperBars(def, pairs);
      expect(bars).toHaveLength(pairs.length);
      bars.forEach((bar, i) => {
        bar.forEach((end, side) => {
          const owner = pairs[i][side];
          const c = terminalCenter(def, owner)!;
          const own = Math.hypot(end.x - c.x, end.y - c.y);
          expect(own, `${id} ${spec}: ${owner}側の端`).toBeCloseTo(TERMINAL_MARK_R, 9);
          for (const no of numbers) {
            if (no === owner) continue;
            const o = terminalCenter(def, no)!;
            const other = Math.hypot(end.x - o.x, end.y - o.y);
            expect(own, `${id} ${spec}: ${owner} vs ${no}`).toBeLessThan(other);
          }
        });
      });
    }
  });

  // ja: ジャンパバーは、どの端子番号の文字にも重ならない
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
      for (const [a, b] of jumperBars(def, pairs)) {
        // 線の端は伸びない(butt cap)ので、太さのぶんは線に直交する向きにだけ広げる
        const h = JUMPER_BAR_STROKE_MM / 2;
        const hx = a.x === b.x ? h : 0;
        const hy = a.y === b.y ? h : 0;
        graphics.push([
          Math.min(a.x, b.x) - hx,
          Math.min(a.y, b.y) - hy,
          Math.max(a.x, b.x) + hx,
          Math.max(a.y, b.y) + hy,
        ]);
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
