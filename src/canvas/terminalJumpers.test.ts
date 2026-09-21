// Rust側 madake-core/src/terminal_chart.rs の parse_jumpers / jumper_bar_segments と
// 同一ルールを検証する。
import { describe, expect, it } from "vitest";

import { dynamicSymbol } from "./dynamicSymbol";
import {
  formatJumperSpec,
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

describe("jumperBarSegments", () => {
  // ja: 隣り合う2端子間のジャンパバーは、それぞれの印の中心を結ぶ5mm間隔の線分になる
  it("spans the two terminal centres, 5 mm apart on the standard pitch", () => {
    const def = dynamicSymbol("terminal_block_4p")!;
    const segs = jumperBarSegments(def, [["1", "2"]]);
    expect(segs).toHaveLength(1);
    const [a, b] = segs[0];
    expect(a.x).toBe(0);
    expect(b.x).toBe(0);
    expect(b.y - a.y).toBeCloseTo(5, 9);
  });

  // ja: 段をまたぐジャンパのバーは、同じ行にある2つの段の列を横に結ぶ線分になる
  it("spans the two level columns of one pole for a cross-level jumper", () => {
    const def = dynamicSymbol("terminal_block_4p_2l")!;
    const segs = jumperBarSegments(def, [["1a", "1b"]]);
    expect(segs).toHaveLength(1);
    const [a, b] = segs[0];
    expect(a.y).toBeCloseTo(b.y, 9);
    expect(b.x - a.x).toBeCloseTo(10, 9);
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
