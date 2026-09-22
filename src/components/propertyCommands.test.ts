// プロパティパネルの編集がCommandへどう変換されるか (docs/internal/specs/m2-drawing-parity.md §2・§3)。
import { describe, expect, it } from "vitest";

import type { Entity, Sheet, SymbolInstance } from "../ipc";
import {
  annotationTextCommand,
  harnessUpdateCommand,
  symbolAttrs,
  wireNumberCommand,
} from "./propertyCommands";

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

const sheetWith = (height?: number | null): Sheet =>
  ({ id: "s1", annotation_text_mm: height, entities: {} }) as unknown as Sheet;

describe("annotationTextCommand", () => {
  // ja: 注記文字高を入力するとシート設定のコマンドになり、注記・線番・タグがまとめて縮む
  it("builds a sheet command for the entered annotation text height", () => {
    const expected = { type: "set_sheet_annotation_text", sheet_id: "s1", height_mm: 1.4 };
    expect(annotationTextCommand(sheetWith(), "1.4")).toEqual(expected);
    // 数値入力欄のv-modelは数値を返すので、そのままでも同じコマンドになる
    expect(annotationTextCommand(sheetWith(), 1.4)).toEqual(expected);
  });

  // ja: 空欄にすると既定 (2.0mm) へ戻すコマンドになる
  it("clears the height back to the default when the field is emptied", () => {
    expect(annotationTextCommand(sheetWith(1.4), "")).toEqual({
      type: "set_sheet_annotation_text",
      sheet_id: "s1",
      height_mm: null,
    });
  });

  // ja: 範囲 (1.0〜3.5mm) を外れた値・数値でない入力は送らない (エンジンも同じ範囲で拒否する)
  it("sends nothing for a height outside the allowed range or a non-number", () => {
    for (const bad of ["0.5", "9", "おおきく"]) {
      expect(annotationTextCommand(sheetWith(), bad), bad).toBeNull();
    }
  });

  // ja: 今と同じ高さならコマンドを送らない (無駄なundo履歴を作らない)
  it("sends nothing when the height did not change", () => {
    expect(annotationTextCommand(sheetWith(1.4), "1.4")).toBeNull();
    expect(annotationTextCommand(sheetWith(), "2")).toBeNull();
  });
});

describe("symbolAttrs", () => {
  const symbol = (symbolId: string, attrs: Record<string, string>): SymbolInstance => ({
    id: "s1",
    symbol_id: symbolId,
    at: { x: 0, y: 0 },
    rotation: 0,
    mirror: false,
    reference: "J1",
    value: "",
    attrs,
  });
  const edited = { functionLabel: "", deviceBlock: false, mate: "" };

  // ja: コネクタでないシンボルを保存しても、パネルに欄の無い嵌合相手の属性は残る (MCP・CLIが書いた値を落とさない)
  it("keeps the mate attribute of a non-connector symbol, which the panel has no field for", () => {
    const attrs = symbolAttrs(symbol("relay_coil", { mates_with: "J12" }), edited);
    expect(attrs.mates_with).toBe("J12");
  });

  // ja: コネクタの嵌合相手欄を空にして保存すると、その属性はキーごと消える
  it("removes the mate attribute of a connector when the field is emptied", () => {
    const attrs = symbolAttrs(symbol("connector_3p", { mates_with: "J12" }), edited);
    expect(attrs.mates_with).toBeUndefined();
  });

  // ja: パネルが持たない属性 (端子台のジャンパなど) は、どのシンボルでもそのまま残る
  it("leaves attributes the panel does not own untouched", () => {
    const attrs = symbolAttrs(symbol("terminal_block_4p", { jumpers: "1-2" }), edited);
    expect(attrs.jumpers).toBe("1-2");
  });
});
