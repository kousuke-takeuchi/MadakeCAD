// プロパティパネルの線番編集がCommandへどう変換されるか (docs/internal/specs/m2-drawing-parity.md §2)。
import { describe, expect, it } from "vitest";

import type { Entity } from "../ipc";
import { wireNumberCommand } from "./propertyCommands";

function wire(net: string | null): Entity {
  return {
    kind: "wire",
    id: "w1",
    points: [
      { x: 0, y: 0 },
      { x: 10, y: 0 },
    ],
    color: "black",
    sq: 0.3,
    length_m: null,
    part_no: null,
    net,
  } as Entity;
}

describe("wireNumberCommand", () => {
  // ja: 線番を書き換えて確定するとset_wire_numbersコマンドが1件だけ組み立てられる(undoで戻せる)
  it("builds a single set_wire_numbers command for an edited wire number", () => {
    expect(wireNumberCommand("s1", wire("3"), "W7")).toEqual({
      type: "set_wire_numbers",
      sheet_id: "s1",
      numbers: [{ wire_id: "w1", number: "W7" }],
    });
  });

  // ja: 線番を空にして確定すると、その配線の線番を消すコマンドになる
  it("clears the wire number when the field is emptied", () => {
    expect(wireNumberCommand("s1", wire("3"), "")).toEqual({
      type: "set_wire_numbers",
      sheet_id: "s1",
      numbers: [{ wire_id: "w1", number: null }],
    });
  });

  // ja: 前後の空白は落とされ、空白だけの入力は線番を消す扱いになる
  it("trims the entered number and treats whitespace as clearing it", () => {
    expect(wireNumberCommand("s1", wire(null), "  12  ")).toMatchObject({
      numbers: [{ wire_id: "w1", number: "12" }],
    });
    expect(wireNumberCommand("s1", wire("12"), "   ")).toMatchObject({
      numbers: [{ wire_id: "w1", number: null }],
    });
  });

  // ja: 線番が変わっていなければコマンドを送らない(無駄なundo履歴を作らない)
  it("sends nothing when the wire number did not change", () => {
    expect(wireNumberCommand("s1", wire("12"), "12")).toBeNull();
    expect(wireNumberCommand("s1", wire("12"), " 12 ")).toBeNull();
    expect(wireNumberCommand("s1", wire(null), "")).toBeNull();
  });

  // ja: 配線以外を選んでいるときは線番コマンドを作らない
  it("builds nothing for entities other than wires", () => {
    const symbol = { kind: "symbol", id: "s", reference: "K1" } as Entity;
    expect(wireNumberCommand("s1", symbol, "12")).toBeNull();
  });
});
