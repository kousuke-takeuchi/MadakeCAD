// 部品挿入ダイアログのシンボル絞り込み・カテゴリ分けの仕様テスト。
import { describe, expect, it } from "vitest";

import type { SymbolDef } from "../ipc";
import { groupSymbolsByCategory, symbolMatchesQuery } from "./symbolLibrary";

function def(id: string, category: string, name: string, nameJa: string, keywords: string[] = []): SymbolDef {
  return {
    id,
    name,
    name_ja: nameJa,
    category,
    ref_prefix: "X",
    keywords,
    primitives: [],
    pins: [],
  };
}

const library: SymbolDef[] = [
  def("breaker_3p", "protection", "Circuit breaker 3P (MCB/MCCB)", "配線用遮断器(3極)", [
    "breaker",
    "mccb",
    "nfb",
    "遮断器",
    "ブレーカ",
  ]),
  def("contactor_3p", "relay", "Contactor main contacts 3P", "電磁接触器 主接点(3極)", [
    "contactor",
    "電磁接触器",
    "マグネットスイッチ",
  ]),
  def("relay_coil", "relay", "Relay coil", "リレーコイル", ["relay", "コイル"]),
  def("motor_3ph", "output", "Motor (3-phase)", "三相電動機", ["motor", "三相"]),
];

describe("symbolMatchesQuery", () => {
  // ja: 検索語は英語名・日本語名・シンボルidのどれに当たっても一致する
  it("matches the English name, the Japanese name or the symbol id", () => {
    const breaker = library[0];
    expect(symbolMatchesQuery(breaker, "circuit")).toBe(true);
    expect(symbolMatchesQuery(breaker, "遮断器")).toBe(true);
    expect(symbolMatchesQuery(breaker, "breaker_3p")).toBe(true);
    expect(symbolMatchesQuery(breaker, "モータ")).toBe(false);
  });

  // ja: 名称に無い現場の呼び方 (NFB・マグネットスイッチ等) も検索キーワードで見つかる
  it("finds symbols by the shop-floor words kept in the search keywords", () => {
    expect(symbolMatchesQuery(library[0], "NFB")).toBe(true);
    expect(symbolMatchesQuery(library[1], "マグネットスイッチ")).toBe(true);
  });

  // ja: 空の検索語はすべてのシンボルに一致する
  it("treats an empty query as matching everything", () => {
    expect(library.every((s) => symbolMatchesQuery(s, "   "))).toBe(true);
  });
});

describe("groupSymbolsByCategory", () => {
  // ja: シンボルはライブラリの並び順のままカテゴリごとにまとまる
  it("groups symbols by category keeping the library order", () => {
    expect(groupSymbolsByCategory(library).map(([cat, syms]) => [cat, syms.length])).toEqual([
      ["protection", 1],
      ["relay", 2],
      ["output", 1],
    ]);
  });

  // ja: 検索語を渡すと一致するシンボルだけが残り、空になったカテゴリは消える
  it("keeps only matching symbols and drops the categories left empty", () => {
    expect(groupSymbolsByCategory(library, "relay")).toEqual([["relay", [library[2]]]]);
    expect(groupSymbolsByCategory(library, "3極").map(([cat]) => cat)).toEqual([
      "protection",
      "relay",
    ]);
  });
});
