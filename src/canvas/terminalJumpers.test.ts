// Rust側 madake-core/src/terminal_chart.rs の parse_jumpers / jumper_bars と
// 同一ルールを検証する。
import { describe, expect, it } from "vitest";

import type { Primitive, SymbolDef } from "../ipc";
import { dynamicSymbol, TERMINAL_MARK_R, TERMINAL_NUMBER_HEIGHT } from "./dynamicSymbol";
import {
  formatJumperSpec,
  JUMPER_BAR_STROKE_MM,
  jumperBars,
  multilevelSymbolId,
  parseJumperSpec,
  polesAndLevelsOf,
  relabelForLevels,
  rewriteJumperLabels,
  terminalCenter,
  terminalLabels,
  terminalNumbers,
  terminalNumbersOfInstance,
  terminalRenumberPlan,
  validTerminalNumbers,
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

describe("validTerminalNumbers", () => {
  // ja: 個数が合い、重複も空要素も無ければ妥当
  it("accepts a list with the right length and no duplicates or empty items", () => {
    expect(validTerminalNumbers(["11", "12", "13", "14"], 4)).toBe(true);
  });

  // ja: ピン数と合わない個数は無効
  it("rejects a list whose length does not match the pin count", () => {
    expect(validTerminalNumbers(["11", "12", "13"], 4)).toBe(false);
  });

  // ja: 重複したラベルは無効
  it("rejects a list with a duplicate label", () => {
    expect(validTerminalNumbers(["1", "1", "2", "3"], 4)).toBe(false);
  });

  // ja: 空要素は無効
  it("rejects a list with an empty item", () => {
    expect(validTerminalNumbers(["1", "", "3", "4"], 4)).toBe(false);
  });

  // ja: 判定はtrim後の値で行う: ラベル前後の空白は無視し、空白しか違わない2つは重複と数え、空白だけのラベルも無効 (Rust側とparity)
  it("judges the list on its trimmed values", () => {
    expect(validTerminalNumbers([" 11", "12 ", " 13 ", "14"], 4)).toBe(true);
    expect(validTerminalNumbers(["1 ", "1", "2", "3"], 4)).toBe(false);
    expect(validTerminalNumbers(["1", "  ", "3", "4"], 4)).toBe(false);
  });

  // ja: 保存形の区切り記号 (カンマ) を含むラベルは無効 (読み戻すと端子の個数が変わってしまう)
  it("rejects a label containing the stored list's own separator", () => {
    expect(validTerminalNumbers(["1,2", "3", "4", "5"], 4)).toBe(false);
  });
});

describe("terminalLabels / rewriteJumperLabels", () => {
  // ja: 対応表は生のピン番号と表示ラベルを相互変換し、知らない番号はそのまま返す (Rust側 TerminalLabels とparity)
  it("converts a raw pin number to the displayed label and back", () => {
    const def = dynamicSymbol("terminal_block_4p")!;
    const labels = terminalLabels(def, { terminal_numbers: "11,12,13,14" });
    expect(labels.isIdentity).toBe(false);
    expect(labels.displayOf("1")).toBe("11");
    expect(labels.rawOf("11")).toBe("1");
    expect(labels.displayOf("99")).toBe("99");
    expect(labels.rawOf("99")).toBe("99");
  });

  // ja: 端子台以外のシンボルには対応表を作らない (上書きは端子台だけの仕組み)
  it("never builds a label map for a symbol that is not a terminal block", () => {
    const def = dynamicSymbol("connector_4p")!;
    expect(terminalLabels(def, { terminal_numbers: "11,12,13,14" }).isIdentity).toBe(true);
  });

  // ja: ジャンパ指定の書き換えは認識できた端子番号だけを置き換え、空白・読めない項目・存在しない端子を含む項目はそのまま残す (逆向きで元の文字列へ戻る。Rust側 rewrite_jumper_labels とparity)
  it("rewrites only the terminal numbers it recognises and round-trips", () => {
    const forward = new Map(
      ["1", "2", "3", "4", "5", "6", "7", "8"].map((n, i) => [n, String(11 + i)] as const),
    );
    const backward = new Map([...forward].map(([a, b]) => [b, a] as const));
    const original = "1-2, abc ,1-99,3 - 4";
    const renamed = rewriteJumperLabels(original, forward);
    expect(renamed).toBe("11-12, abc ,1-99,13 - 14");
    expect(rewriteJumperLabels(renamed, backward)).toBe(original);
  });
});

describe("terminalNumbersOfInstance / terminalLabels", () => {
  // ja: attrs["terminal_numbers"]が無ければシンボル既定の番号のまま
  it("falls back to the symbol's default numbers when there is no override", () => {
    const def = dynamicSymbol("terminal_block_4p")!;
    expect(terminalNumbersOfInstance(def, undefined)).toEqual(["1", "2", "3", "4"]);
    expect(terminalLabels(def, undefined).renameMap).toBeNull();
  });

  // ja: 妥当な上書きはピン順のラベルとして使われ、生の番号→上書きラベルの対応が引ける
  it("uses a valid override and builds a raw-to-display rename map", () => {
    const def = dynamicSymbol("terminal_block_4p")!;
    const attrs = { terminal_numbers: "11,12,13,14" };
    expect(terminalNumbersOfInstance(def, attrs)).toEqual(["11", "12", "13", "14"]);
    expect(terminalLabels(def, attrs).renameMap).toEqual(
      new Map([
        ["1", "11"],
        ["2", "12"],
        ["3", "13"],
        ["4", "14"],
      ]),
    );
  });

  // ja: 個数が合わない上書きは既定の番号へフォールバックする
  it("falls back to the defaults when the override's item count is wrong", () => {
    const def = dynamicSymbol("terminal_block_4p")!;
    expect(terminalNumbersOfInstance(def, { terminal_numbers: "11,12,13" })).toEqual([
      "1",
      "2",
      "3",
      "4",
    ]);
  });
});

describe("terminalRenumberPlan", () => {
  // ja: 開始番号から刻み1でピン順に振る (単段)
  it("numbers every pin from the start value on a single-level block", () => {
    const raw = terminalNumbers(dynamicSymbol("terminal_block_8p")!);
    expect(terminalRenumberPlan(raw, 11, 1, "")).toEqual([
      "11",
      "12",
      "13",
      "14",
      "15",
      "16",
      "17",
      "18",
    ]);
  });

  // ja: 多段端子台は段記号を保ったまま極だけ採番するので、同じ極の全段が同じ番号を共有する
  it("keeps the level letters on a multi-level block and numbers only the pole", () => {
    const raw = terminalNumbers(dynamicSymbol("terminal_block_4p_2l")!);
    expect(terminalRenumberPlan(raw, 11, 1, "")).toEqual([
      "11a",
      "11b",
      "12a",
      "12b",
      "13a",
      "13b",
      "14a",
      "14b",
    ]);
  });

  // ja: 増分と接頭辞はどちらも効く
  it("applies the step and the prefix", () => {
    const raw = terminalNumbers(dynamicSymbol("terminal_block_4p")!);
    expect(terminalRenumberPlan(raw, 1, 2, "P")).toEqual(["P1", "P3", "P5", "P7"]);
  });
});

describe("level change helpers (Rust側 poles_and_levels_of / multilevel_id / relabel_for_levels の双子)", () => {
  // ja: 極数と段数は端子番号から数え、単段の端子台は段数1になる
  it("counts poles and levels from the terminal numbers", () => {
    expect(polesAndLevelsOf(dynamicSymbol("terminal_block_8p"))).toEqual([8, 1]);
    expect(polesAndLevelsOf(dynamicSymbol("terminal_block_8p_2l"))).toEqual([8, 2]);
    expect(polesAndLevelsOf(null)).toEqual([0, 0]);
  });

  // ja: 段数を変えたsymbol_idは、段数1のとき`_1l`の別名ではなく単段のidへ正規化される
  it("normalises one level back to the plain single-level symbol id", () => {
    const two = dynamicSymbol("terminal_block_8p_2l");
    expect(multilevelSymbolId("terminal_block_8p", 2, dynamicSymbol("terminal_block_8p"))).toBe(
      "terminal_block_8p_2l",
    );
    expect(multilevelSymbolId("terminal_block_8p_2l", 1, two)).toBe("terminal_block_8p");
    expect(multilevelSymbolId("terminal_block_8p_2l", 3, two)).toBe("terminal_block_8p_3l");
  });

  // ja: 端子台以外のシンボルと範囲外の段数はnull (プレビューを出さない)
  it("returns null for a symbol that is not a terminal block and for an out-of-range level count", () => {
    expect(multilevelSymbolId("resistor", 2, dynamicSymbol("resistor"))).toBeNull();
    expect(multilevelSymbolId("terminal_block_8p", 0, dynamicSymbol("terminal_block_8p"))).toBeNull();
    expect(multilevelSymbolId("terminal_block_8p", 4, dynamicSymbol("terminal_block_8p"))).toBeNull();
  });

  // ja: 段を増やすと極ごとのラベルに段記号が付き、減らすと残る段のラベルだけになる (段数1では段記号が落ちる)
  it("expands labels into lettered levels and drops the vanishing ones when decreasing", () => {
    expect(relabelForLevels(["11", "12"], 2, 1, 2)).toEqual(["11a", "11b", "12a", "12b"]);
    expect(relabelForLevels(["11a", "11b", "12a", "12b"], 2, 2, 1)).toEqual(["11", "12"]);
    expect(relabelForLevels(["11a", "11b", "12a", "12b"], 2, 2, 3)).toEqual([
      "11a",
      "11b",
      "11c",
      "12a",
      "12b",
      "12c",
    ]);
  });

  // ja: 末尾が英字でない上書きラベル (接頭辞付きの`X11`) は段記号を外す対象にならず、そのまま土台になる
  it("keeps a label that does not end in a level letter intact as the base", () => {
    expect(relabelForLevels(["X11", "X12"], 2, 1, 2)).toEqual(["X11a", "X11b", "X12a", "X12b"]);
  });
});

describe("parseJumperSpec with an overridden display order", () => {
  // ja: 増分2で採番したような表示ラベル(11,13,15,17)でも、隣接判定は生のピン配置で決まるので、物理的に隣り合う極どうしのジャンパは通る (Rust側 adjacency_after_an_override_is_based_on_the_raw_pin_layout_not_the_label_arithmetic とparity)
  it("keeps adjacency based on the raw pin layout, not on the arithmetic difference between displayed labels", () => {
    const def = dynamicSymbol("terminal_block_4p")!;
    const raw = terminalNumbers(def);
    const display = terminalRenumberPlan(raw, 11, 2, "");
    expect(display).toEqual(["11", "13", "15", "17"]);
    expect(parseJumperSpec("11-13", display, raw)).toEqual([["11", "13"]]);
    // 表示ラベルの数値差だけで見ると2つ離れているが、生のピン配置では隣り合っていない
    // (極1と極3) 組は、それでも拒否される
    expect(parseJumperSpec("11-15", display, raw)).toEqual([]);
  });
});
