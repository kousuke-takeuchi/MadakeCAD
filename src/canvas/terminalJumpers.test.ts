// Rust側 madake-core/src/terminal_chart.rs の parse_jumpers / jumper_bar_segments と
// 同一ルールを検証する。
import { describe, expect, it } from "vitest";

import type { Primitive, SymbolDef } from "../ipc";
import { dynamicSymbol } from "./dynamicSymbol";
import {
  formatJumperSpec,
  JUMPER_BAR_DOT_R_MM,
  JUMPER_BAR_OFFSET_MM,
  JUMPER_BAR_STROKE_MM,
  jumperBarSegments,
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
    // madake-core/src/terminal_chart.rs の JUMPER_BAR_STROKE / _DOT_R / _OFFSET
    expect([JUMPER_BAR_STROKE_MM, JUMPER_BAR_DOT_R_MM, JUMPER_BAR_OFFSET_MM]).toEqual([
      0.8, 0.7, 2.7,
    ]);
  });
});

describe("jumperBarSegments", () => {
  // ja: 同じ列の2端子を繋ぐジャンパバーは、端子番号を避けて列の右へずらした5mmの線分になる
  it("runs alongside the column, clear of the numbers, 5 mm long on the standard pitch", () => {
    const def = dynamicSymbol("terminal_block_4p")!;
    const segs = jumperBarSegments(def, [["1", "2"]]);
    expect(segs).toHaveLength(1);
    const [a, b] = segs[0];
    expect(a.x).toBe(JUMPER_BAR_OFFSET_MM);
    expect(b.x).toBe(JUMPER_BAR_OFFSET_MM);
    expect(b.y - a.y).toBeCloseTo(5, 9);
  });

  // ja: 段をまたぐジャンパのバーは、2つの段の列の真下を横に結ぶ線分になる(行の下へずれる)
  it("runs below the row, across the two level columns of one pole", () => {
    const def = dynamicSymbol("terminal_block_4p_2l")!;
    const segs = jumperBarSegments(def, [["1a", "1b"]]);
    expect(segs).toHaveLength(1);
    const [a, b] = segs[0];
    expect(a.y).toBeCloseTo(b.y, 9);
    expect(b.x - a.x).toBeCloseTo(10, 9);
    expect(a.y).toBeCloseTo(-7.5 + JUMPER_BAR_OFFSET_MM, 9);
  });

  // ja: ジャンパバーと両端の丸は、どの端子番号の文字にも重ならない
  it("never covers a terminal number", () => {
    // 文字は中心揃え・ベースラインが端子の中心。幅は文字数x文字高x0.6、高さは文字高x0.7で見る
    const numberBoxes = (def: SymbolDef) =>
      def.primitives
        .filter((p): p is Extract<Primitive, { type: "text" }> => p.type === "text")
        .map((p) => {
          const baseline = p.at.y + p.height / 2;
          const half = (p.text.length * p.height * 0.6) / 2;
          return [p.at.x - half, baseline - p.height * 0.7, p.at.x + half, baseline] as const;
        });
    const overlaps = (a: readonly number[], b: readonly number[]) =>
      a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3];
    for (const [id, spec] of [
      ["terminal_block_4p", "1-2,3-4"],
      ["terminal_block_4p_2l", "1a-1b,2a-3a"],
      ["terminal_block_4p_3l", "1a-1b,1b-1c"],
      ["terminal_block_12p_2l", "10a-10b,11a-12a"],
    ] as const) {
      const def = dynamicSymbol(id)!;
      const pairs = parseJumperSpec(spec, terminalNumbers(def));
      const graphics: (readonly number[])[] = [];
      for (const [a, b] of jumperBarSegments(def, pairs)) {
        const h = JUMPER_BAR_STROKE_MM / 2;
        graphics.push([
          Math.min(a.x, b.x) - h,
          Math.min(a.y, b.y) - h,
          Math.max(a.x, b.x) + h,
          Math.max(a.y, b.y) + h,
        ]);
        for (const p of [a, b]) {
          const r = JUMPER_BAR_DOT_R_MM;
          graphics.push([p.x - r, p.y - r, p.x + r, p.y + r]);
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
      jumperBarSegments(def, [
        ["1", "2"],
        ["3", "4"],
      ]),
    ).toHaveLength(2);
  });

  // ja: 端子台に無い端子番号を指すジャンパは線分を作らない
  it("produces no segment for a jumper to a terminal the block does not have", () => {
    const def = dynamicSymbol("terminal_block_4p")!;
    expect(jumperBarSegments(def, [["4", "5"]])).toEqual([]);
  });
});
