// Rust側 madake-core/src/terminal_chart.rs の parse_jumpers / jumper_bar_segments と
// 同一ルールを検証する。
import { describe, expect, it } from "vitest";

import { dynamicSymbol } from "./dynamicSymbol";
import { jumperBarSegments, parseJumperPairs } from "./terminalJumpers";

describe("parseJumperPairs", () => {
  // ja: 隣り合う端子どうしのジャンパは正規化され、隣り合わない組は捨てられる
  it("keeps jumpers between adjacent terminals and drops non-adjacent ones", () => {
    expect(parseJumperPairs("2-1")).toEqual([[1, 2]]);
    expect(parseJumperPairs("1-3")).toEqual([]);
  });

  // ja: 複数のジャンパはそれぞれ組として残る
  it("keeps every jumper when there are several", () => {
    expect(parseJumperPairs("1-2,3-4")).toEqual([
      [1, 2],
      [3, 4],
    ]);
  });

  // ja: 空文字や読めない記述はジャンパ無しとして扱う
  it("treats empty or unreadable text as no jumpers", () => {
    expect(parseJumperPairs("")).toEqual([]);
    expect(parseJumperPairs("abc,1-,1-2-3")).toEqual([]);
  });
});

describe("jumperBarSegments", () => {
  // ja: 隣り合う2端子間のジャンパバーは、それぞれの印の中心を結ぶ5mm間隔の線分になる
  it("spans the two terminal centres, 5 mm apart on the standard pitch", () => {
    const def = dynamicSymbol("terminal_block_4p")!;
    const segs = jumperBarSegments(def, [[1, 2]]);
    expect(segs).toHaveLength(1);
    const [a, b] = segs[0];
    expect(a.x).toBe(0);
    expect(b.x).toBe(0);
    expect(b.y - a.y).toBeCloseTo(5, 9);
  });

  // ja: 複数のジャンパはそれぞれ独立した線分になる
  it("produces one segment per jumper when there are several", () => {
    const def = dynamicSymbol("terminal_block_4p")!;
    expect(jumperBarSegments(def, [[1, 2], [3, 4]])).toHaveLength(2);
  });

  // ja: 端子台に無い端子番号を指すジャンパは線分を作らない
  it("produces no segment for a jumper to a terminal the block does not have", () => {
    const def = dynamicSymbol("terminal_block_4p")!;
    expect(jumperBarSegments(def, [[4, 5]])).toEqual([]);
  });
});
