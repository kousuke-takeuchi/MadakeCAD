// プロパティパネルの編集がCommandへどう変換されるか (docs/internal/specs/m2-drawing-parity.md §2・§3)。
import { describe, expect, it } from "vitest";

import type { Entity, Sheet, SymbolInstance } from "../ipc";
import {
  annotationTextCommand,
  harnessUpdateCommand,
  cableUpdateCommand,
  nameplateUpdateCommand,
  symbolAttrs,
  wireNumberCommand,
  wireUpdateCommand,
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

describe("wireUpdateCommand", () => {
  const edit = { color: "black", sq: 0.3, part_no: "", length_m: "" };

  // ja: 色・線径・品番・長さのどれも変わっていなければコマンドを送らない
  it("sends nothing when colour, gauge, part number and length are unchanged", () => {
    expect(wireUpdateCommand("s1", wire(null), edit)).toEqual({ command: null, overwroteMeasured: false });
  });

  // ja: 長さを手で変えると更新コマンドになり、出所は「手入力」になる
  it("a hand-edited length becomes an update whose length source is manual", () => {
    const result = wireUpdateCommand("s1", wire(null), { ...edit, length_m: "1.5" });
    expect(result.command?.type).toBe("update_entity");
    const entity = (result.command as { entity: Entity }).entity as Extract<Entity, { kind: "wire" }>;
    expect(entity.length_m).toBe(1.5);
    expect(entity.length_source).toBe("manual");
    expect(result.overwroteMeasured).toBe(false);
  });

  // ja: FreeCADで計測した長さを手で上書きすると、その旨のフラグが立つ (呼び出し側が警告する)
  it("overwriting a FreeCAD-measured length by hand is flagged so the panel can warn", () => {
    const measured = { ...wire(null), length_m: 0.42, length_source: "freecad" } as Entity;
    const result = wireUpdateCommand("s1", measured, { ...edit, length_m: "0.5" });
    expect(result.overwroteMeasured).toBe(true);
    const entity = (result.command as { entity: Entity }).entity as Extract<Entity, { kind: "wire" }>;
    expect(entity.length_source).toBe("manual");
  });

  // ja: 長さ以外だけを変えたときはFreeCAD計測の出所を保つ
  it("changing only colour or gauge keeps the FreeCAD length source", () => {
    const measured = { ...wire(null), length_m: 0.42, length_source: "freecad" } as Entity;
    const result = wireUpdateCommand("s1", measured, { ...edit, color: "red", length_m: "0.42" });
    const entity = (result.command as { entity: Entity }).entity as Extract<Entity, { kind: "wire" }>;
    expect(entity.color).toBe("red");
    expect(entity.length_source).toBe("freecad");
    expect(result.overwroteMeasured).toBe(false);
  });
});

describe("cableUpdateCommand", () => {
  const cable = (): Entity => ({
    kind: "cable",
    id: "c1",
    reference: "W1",
    part_no: "",
    core_count: 3,
    sq: 0,
    points: [{ x: 0, y: 0 }, { x: 10, y: 10 }],
    cores: [{ core: "1", color: "black", wire: "w1" }],
    shield: null,
    length_m: null,
  });
  const patch = { reference: "W1", part_no: "SAMPLE-CBL", core_count: "4", sq: "1.25", shield: true, grounded: true, length_m: "12.5" };

  // ja: ケーブルのプロパティは型番・芯数・線径・シールド・長さを1回のupdate_entityで書き、芯線割当はそのまま残す
  it("writes part, cores, gauge, shield and length in one update and keeps the core assignments", () => {
    const command = cableUpdateCommand("s1", cable(), patch);
    expect(command?.type).toBe("update_entity");
    const e = (command as { entity: Entity }).entity;
    expect(e).toMatchObject({ part_no: "SAMPLE-CBL", core_count: 4, sq: 1.25, shield: { grounded: true }, length_m: 12.5 });
    expect(e.kind === "cable" && e.cores).toEqual([{ core: "1", color: "black", wire: "w1" }]);
  });

  // ja: 芯数が1以上の整数でない、線径や長さが数値でない入力は図面を変えない
  it("refuses a core count below one or a gauge or length that is not a number", () => {
    for (const bad of [{ core_count: "0" }, { core_count: "2.5" }, { sq: "abc" }, { length_m: "-1" }]) {
      expect(cableUpdateCommand("s1", cable(), { ...patch, ...bad })).toBeNull();
    }
  });

  // ja: 何も変えていなければコマンドを送らない
  it("sends nothing when nothing changed", () => {
    expect(cableUpdateCommand("s1", cable(), { reference: "W1", part_no: "", core_count: "3", sq: "", shield: false, grounded: false, length_m: "" })).toBeNull();
  });
});

describe("nameplateUpdateCommand", () => {
  const plate = (): Entity => ({
    kind: "nameplate",
    id: "n1",
    footprint: "k1",
    at: { x: 0, y: 0 },
    w: 30,
    h: 10,
    lines: ["主回路"],
    text_mm: 3.5,
    position: "below",
    follow_description: false,
    part_no: "",
  });

  // ja: 銘板の文字・文字高・位置・追従・型番を1回のupdate_entityで書き、空の2行目は付けない
  it("writes text, height, position, following and part in one update without an empty second line", () => {
    const c = nameplateUpdateCommand("s1", plate(), { line1: "主回路", line2: "", text_mm: "5", position: "above", follow: true, part_no: "SAMPLE-NP" });
    expect((c as { entity: Entity }).entity).toMatchObject({ lines: ["主回路"], text_mm: 5, position: "above", follow_description: true, part_no: "SAMPLE-NP" });
  });

  // ja: 文字高が正の数でなければ、また何も変えていなければ図面を変えない
  it("sends nothing for a non-positive text height or when nothing changed", () => {
    expect(nameplateUpdateCommand("s1", plate(), { line1: "主回路", line2: "", text_mm: "0", position: "below", follow: false, part_no: "" })).toBeNull();
    expect(nameplateUpdateCommand("s1", plate(), { line1: "主回路", line2: "", text_mm: "3.5", position: "below", follow: false, part_no: "" })).toBeNull();
  });
});
