// プロパティパネルの編集がCommandへどう変換されるか (docs/internal/specs/m2-drawing-parity.md §2・§3)。
import { describe, expect, it } from "vitest";

import type { Entity } from "../ipc";
import { harnessUpdateCommand, wireNumberCommand } from "./propertyCommands";

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

function harness(name: string, note = ""): Entity {
  return {
    kind: "harness",
    id: "h1",
    points: [
      { x: 50, y: 50 },
      { x: 150, y: 50 },
      { x: 150, y: 100 },
      { x: 50, y: 100 },
    ],
    name,
    note,
  } as Entity;
}

describe("harnessUpdateCommand", () => {
  // ja: ハーネス名を書き換えるとupdate_entityコマンドになり、囲みの形はそのまま残る
  it("builds an update_entity command for a renamed harness and keeps its shape", () => {
    const command = harnessUpdateCommand("s1", harness("W1"), "W7", "");
    expect(command).toMatchObject({ type: "update_entity", sheet_id: "s1" });
    if (command?.type !== "update_entity") throw new Error("update_entity");
    expect(command.entity).toMatchObject({ kind: "harness", id: "h1", name: "W7" });
    if (command.entity.kind !== "harness") throw new Error("harness");
    expect(command.entity.points).toHaveLength(4);
  });

  // ja: 備考だけを変えたときもコマンドになる
  it("builds a command when only the note changed", () => {
    expect(harnessUpdateCommand("s1", harness("W1"), "W1", "現地配線")).toMatchObject({
      entity: { name: "W1", note: "現地配線" },
    });
  });

  // ja: 名前の前後の空白は落とされる
  it("trims the entered harness name", () => {
    expect(harnessUpdateCommand("s1", harness("W1"), "  W2  ", "")).toMatchObject({
      entity: { name: "W2" },
    });
  });

  // ja: 名前も備考も変わっていなければコマンドを送らない(無駄なundo履歴を作らない)
  it("sends nothing when neither the name nor the note changed", () => {
    expect(harnessUpdateCommand("s1", harness("W1"), "W1", "")).toBeNull();
    expect(harnessUpdateCommand("s1", harness("W1", "現地"), " W1 ", " 現地 ")).toBeNull();
  });

  // ja: ハーネス以外を選んでいるときはハーネスコマンドを作らない
  it("builds nothing for entities other than harnesses", () => {
    expect(harnessUpdateCommand("s1", wire("3"), "W1", "")).toBeNull();
  });
});
