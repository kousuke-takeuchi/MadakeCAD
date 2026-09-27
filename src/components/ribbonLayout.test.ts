// リボンの小ボタンの列分けの仕様。リボン本体の高さは固定なので、1列に入る数を超えた
// ボタンはリボンの外へはみ出して押せなくなる。はみ出す前に次の列へ送るのがこの規則。
import { describe, expect, it } from "vitest";

import { RIBBON_SMALL_ROWS, smallColumns } from "./ribbonLayout";

describe("ribbon small-button columns", () => {
  // ja: 1列に入る数ちょうどの小ボタンは、1列のままで列が増えない。
  it("keeps one column when the buttons fit in it", () => {
    const items = ["a", "b", "c", "d"].slice(0, RIBBON_SMALL_ROWS);
    expect(smallColumns(items)).toEqual([items]);
  });

  // ja: 1列に入る数を超えた小ボタンは、リボンからはみ出さないよう次の列へ送られる。
  it("moves the buttons that no longer fit into a further column", () => {
    const items = ["a", "b", "c", "d", "e"];
    const columns = smallColumns(items, 4);
    expect(columns).toEqual([
      ["a", "b", "c", "d"],
      ["e"],
    ]);
    expect(columns.every((c) => c.length <= 4)).toBe(true);
  });

  // ja: 小ボタンが1つも無いグループは列も作らない (空の列が幅だけ取らない)。
  it("makes no column at all for a group with no small buttons", () => {
    expect(smallColumns([])).toEqual([]);
  });

  // ja: 2列に収まらない数の小ボタンは、必要なだけ列を増やして全部が押せる位置に並ぶ。
  it("adds as many columns as the buttons need", () => {
    const items = [1, 2, 3, 4, 5, 6, 7, 8, 9];
    const columns = smallColumns(items, 4);
    expect(columns).toEqual([
      [1, 2, 3, 4],
      [5, 6, 7, 8],
      [9],
    ]);
    expect(columns.flat()).toEqual(items);
  });
});
