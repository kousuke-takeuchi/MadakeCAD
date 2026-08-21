import { describe, it, expect } from "vitest";
import { appendContextTag, drawingContextTag } from "./drawingContext";
import type { Entity, Sheet } from "../ipc";

function symbol(id: string, reference: string): Entity {
  return {
    kind: "symbol",
    id,
    symbol_id: "fuse",
    at: { x: 0, y: 0 },
    rotation: 0,
    mirror: false,
    reference,
    value: "",
    attrs: {},
  };
}

function wire(id: string): Entity {
  return {
    kind: "wire",
    id,
    points: [
      { x: 0, y: 0 },
      { x: 10, y: 0 },
    ],
    color: "black",
    sq: 0.75,
    length_m: null,
    part_no: null,
    net: null,
  };
}

function sheet(entities: Entity[], name = "動力系統図"): Sheet {
  return {
    id: "sheet-1",
    name,
    size: "A3",
    orientation: "Landscape",
    zone_cols: 8,
    zone_rows: 4,
    title_block: {} as Sheet["title_block"],
    revisions: [],
    entities: Object.fromEntries(entities.map((e) => [e.id, e])),
  };
}

describe("drawingContextTag", () => {
  // ja: 選択が無ければコンテキストはシート全体になる
  it("with no selection the context covers the whole sheet", () => {
    expect(drawingContextTag(sheet([symbol("a", "F2")]), [])).toBe(
      "[図面コンテキスト: シート 動力系統図 全体]",
    );
  });

  // ja: シートが無くても文字列は壊れない
  it("with no sheet the string stays well-formed", () => {
    expect(drawingContextTag(null, ["a"])).toBe("[図面コンテキスト: シート (シートなし) 全体]");
  });

  // ja: 選択があれば参照記号を列挙する(無ければ種別名)
  it("a selection lists reference designators (falling back to entity kinds)", () => {
    const s = sheet([symbol("a", "F2"), symbol("b", "K1"), wire("c")]);
    expect(drawingContextTag(s, ["a", "b", "c"])).toBe(
      "[図面コンテキスト: 動力系統図 / 選択: F2, K1, 配線]",
    );
  });

  // ja: 10件を超える選択は「+N件」に畳まれる
  it("selections beyond 10 items collapse into +N more", () => {
    const entities = Array.from({ length: 13 }, (_, i) => symbol(`e${i}`, `F${i}`));
    const tag = drawingContextTag(
      sheet(entities),
      entities.map((e) => e.id),
    );
    expect(tag).toBe("[図面コンテキスト: 動力系統図 / 選択: F0, F1, F2, F3, F4, F5, F6, F7, F8, F9 +3件]");
  });

  // ja: アクティブシートに無い選択idは無視される(全て外れれば全体扱い)
  it("selection ids absent from the active sheet are ignored", () => {
    const s = sheet([symbol("a", "F2")]);
    expect(drawingContextTag(s, ["a", "other-sheet-id"])).toBe(
      "[図面コンテキスト: 動力系統図 / 選択: F2]",
    );
    expect(drawingContextTag(s, ["other-sheet-id"])).toBe("[図面コンテキスト: シート 動力系統図 全体]");
  });
});

describe("appendContextTag", () => {
  // ja: 空の下書きにはそのまま挿入される
  it("an empty draft receives the context as-is", () => {
    expect(appendContextTag("", "[タグ]")).toBe("[タグ]");
  });

  // ja: 既存の下書きとは改行で区切る(改行済みなら重ねない)
  it("an existing draft is separated by a newline (without doubling)", () => {
    expect(appendContextTag("F2を追加して", "[タグ]")).toBe("F2を追加して\n[タグ]");
    expect(appendContextTag("F2を追加して\n", "[タグ]")).toBe("F2を追加して\n[タグ]");
  });
});
