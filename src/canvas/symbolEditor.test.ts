// シンボルエディタのキャンバス幾何と保存前チェックの仕様テスト
// (M4仕様 §7、.pen「M4デザイン - シンボルエディタ」)。
//
// 検証のコードはRust側 madake-core/src/symbol_library.rs の `code` と同じ文字列を使う
// (エディタが出す指摘と、保存が拒否される理由がずれないようにするため)。
import { describe, expect, it } from "vitest";

import type { PinDef, Primitive, SymbolDef } from "../ipc";
import {
  hitTest,
  nextPinNumber,
  nextPinSpot,
  snapToGrid,
  symbolCanSave,
  symbolChecks,
  translatePrimitive,
  validateSymbol,
} from "./symbolEditor";

function pin(number: string, x: number, y: number, dir: PinDef["dir"] = "left"): PinDef {
  return { number, name: "", at: { x, y }, dir };
}

function rect(x1: number, y1: number, x2: number, y2: number): Primitive {
  return { type: "rect", p1: { x: x1, y: y1 }, p2: { x: x2, y: y2 }, filled: false };
}

/** 保存できる最小のシンボル (矩形1つ+ピン2本+TAGスロット)。 */
function goodSymbol(overrides: Partial<SymbolDef> = {}): SymbolDef {
  return {
    id: "shop_lamp",
    name: "Shop lamp",
    name_ja: "自社ランプ",
    category: "user",
    ref_prefix: "L",
    keywords: [],
    primitives: [rect(-5, -5, 5, 5)],
    pins: [pin("1", -7.5, 0, "left"), pin("2", 7.5, 0, "right")],
    text_slots: [{ key: "TAG", at: { x: 0, y: -10 }, height: 2.5 }],
    ...overrides,
  };
}

function codes(def: SymbolDef, reserved: string[] = []): string[] {
  return validateSymbol(def, reserved).map((c) => c.code);
}

describe("symbol editor canvas geometry", () => {
  // ja: キャンバスのクリック位置は必ず2.5mmグリッドの交点へ吸い付く
  it("snaps a click position to the nearest 2.5 mm grid intersection", () => {
    expect(snapToGrid({ x: 5.9, y: 1.4 })).toEqual({ x: 5, y: 2.5 });
    expect(snapToGrid({ x: 0.4, y: 3.9 })).toEqual({ x: 0, y: 5 });
  });

  // ja: 原点より左上(マイナス側)をクリックしてもグリッドに吸い付く
  it("snaps negative coordinates (left of and above the origin) to the grid too", () => {
    expect(snapToGrid({ x: -6.2, y: -8.8 })).toEqual({ x: -5, y: -10 });
  });

  // ja: 図形の上に乗っている接続点をクリックすると、図形ではなく接続点が選ばれる
  it("picks the connection point on top of a shape rather than the shape itself", () => {
    // 接続点は矩形の左辺の上に乗っている (どちらも当たる位置)
    const def = goodSymbol({ pins: [pin("1", -5, 0, "left")] });
    expect(hitTest(def, { x: -5, y: 0 }, 1.0)).toEqual({ kind: "pin", index: 0 });
  });

  // ja: 図形が重なっている場所では、あとから描いた図形が選ばれる
  it("picks the most recently drawn shape where two shapes overlap", () => {
    const def = goodSymbol({ primitives: [rect(-5, -5, 5, 5), rect(-5, -5, 5, 5)] });
    expect(hitTest(def, { x: -5, y: 0 }, 0.6)).toEqual({ kind: "primitive", index: 1 });
  });

  // ja: 何も無いところをクリックすると選択は外れる
  it("selects nothing when the click is away from every shape and point", () => {
    expect(hitTest(goodSymbol(), { x: 40, y: 40 }, 1.0)).toBeNull();
  });

  // ja: 属性テキストは配置位置をクリックして選べる
  it("finds an attribute slot by clicking its anchor point", () => {
    expect(hitTest(goodSymbol(), { x: 0, y: -10 }, 1.0)).toEqual({ kind: "slot", index: 0 });
  });

  // ja: 図形をドラッグすると、その図形の全ての点が同じ量だけ動く
  it("moves every point of a shape by the same amount when it is dragged", () => {
    expect(translatePrimitive(rect(-5, -5, 5, 5), 2.5, -2.5)).toEqual(
      rect(-2.5, -7.5, 7.5, 2.5),
    );
    expect(
      translatePrimitive({ type: "line", pts: [{ x: 0, y: 0 }, { x: 5, y: 0 }] }, 0, 2.5),
    ).toEqual({ type: "line", pts: [{ x: 0, y: 2.5 }, { x: 5, y: 2.5 }] });
    expect(
      translatePrimitive({ type: "circle", center: { x: 0, y: 0 }, r: 2.5, filled: false }, 5, 0),
    ).toEqual({ type: "circle", center: { x: 5, y: 0 }, r: 2.5, filled: false });
    expect(
      translatePrimitive(
        { type: "arc", center: { x: 0, y: 0 }, r: 5, start_deg: 0, end_deg: 90 },
        0,
        5,
      ),
    ).toEqual({ type: "arc", center: { x: 0, y: 5 }, r: 5, start_deg: 0, end_deg: 90 });
    expect(
      translatePrimitive({ type: "text", at: { x: 0, y: 0 }, text: "A", height: 2.5 }, 1, 2),
    ).toEqual({ type: "text", at: { x: 1, y: 2 }, text: "A", height: 2.5 });
  });

  // ja: 新しい接続点の番号は、いま一番大きい番号の次になる(最初は1番)
  it("numbers a new connection point one past the largest number in use", () => {
    expect(nextPinNumber(goodSymbol({ pins: [] }))).toBe("1");
    expect(nextPinNumber(goodSymbol())).toBe("3");
    expect(nextPinNumber(goodSymbol({ pins: [pin("A1", 0, 0), pin("7", 0, 5)] }))).toBe("8");
  });

  // ja: 方向ボタンで足した接続点は、図形の外側1グリッドの位置にグリッドに乗って置かれる
  it("places a new connection point one grid step outside the shape on the chosen side", () => {
    const def = goodSymbol({ pins: [] });
    expect(nextPinSpot(def, "left")).toEqual({ x: -7.5, y: 0 });
    expect(nextPinSpot(def, "right")).toEqual({ x: 7.5, y: 0 });
    expect(nextPinSpot(def, "up")).toEqual({ x: 0, y: -7.5 });
    expect(nextPinSpot(def, "down")).toEqual({ x: 0, y: 7.5 });
  });

  // ja: 同じ方向にもう1本足しても、接続点が同じ点に重なることはない
  it("never stacks a second connection point on the same point as an existing one", () => {
    const def = goodSymbol({ pins: [pin("1", -7.5, 0, "left")] });
    const spot = nextPinSpot(def, "left");
    expect(spot).not.toEqual({ x: -7.5, y: 0 });
    expect(Math.abs(spot.x % 2.5)).toBe(0);
    expect(Math.abs(spot.y % 2.5)).toBe(0);
  });
});

describe("symbol editor validation", () => {
  // ja: 過不足のないシンボルは指摘ゼロで保存できる
  it("reports no problem for a well-formed symbol", () => {
    expect(validateSymbol(goodSymbol(), ["relay_coil"])).toEqual([]);
  });

  // ja: 2.5mmグリッドから外れた接続点は保存前に指摘される
  it("reports a connection point that is off the 2.5 mm grid", () => {
    const def = goodSymbol({ pins: [pin("1", -7.5, 0), pin("2", 7.5, 1.2)] });
    expect(codes(def)).toContain("symbol.pin_off_grid");
  });

  // ja: 同じ位置に同じ番号の接続点が2つあると保存前に指摘される
  it("reports two connection points with the same number at the same position", () => {
    const def = goodSymbol({ pins: [pin("1", -7.5, 0), pin("1", -7.5, 0)] });
    expect(codes(def)).toContain("symbol.pin_duplicate");
  });

  // ja: 同じ位置に別番号の接続点が重なっていると保存前に指摘される
  it("reports two different connection point numbers stacked on one position", () => {
    const def = goodSymbol({ pins: [pin("1", -7.5, 0), pin("2", -7.5, 0)] });
    expect(codes(def)).toContain("symbol.pin_overlap");
  });

  // ja: 離れた2点に同じ番号を置くのは正しい(端子台の貫通端子)ので指摘しない
  it("accepts the same connection point number at two separate positions", () => {
    const def = goodSymbol({ pins: [pin("1", -7.5, 0), pin("1", 7.5, 0)] });
    expect(codes(def)).toEqual([]);
  });

  // ja: 参照記号(TAG)の属性スロットが無いと保存できない
  it("reports a missing reference designator (TAG) attribute slot", () => {
    expect(codes(goodSymbol({ text_slots: [] }))).toContain("symbol.slot_missing");
  });

  // ja: 同梱シンボルと同じidは、既存図面の記号が変わってしまうので使えない
  it("rejects an id that is already used by a bundled symbol", () => {
    expect(codes(goodSymbol({ id: "relay_coil" }), ["relay_coil"])).toContain(
      "symbol.id_reserved",
    );
  });

  // ja: idが空・英数字以外・名前や接頭辞が空・図形0個・接続点0個はいずれも保存できない
  it("rejects an empty id, an id with unusable characters, and an empty symbol", () => {
    expect(codes(goodSymbol({ id: "" }))).toContain("symbol.id_empty");
    expect(codes(goodSymbol({ id: "my symbol" }))).toContain("symbol.id_invalid");
    expect(codes(goodSymbol({ name: "", name_ja: "" }))).toContain("symbol.name_empty");
    expect(codes(goodSymbol({ ref_prefix: "" }))).toContain("symbol.ref_prefix_empty");
    expect(codes(goodSymbol({ primitives: [] }))).toContain("symbol.no_primitives");
    expect(codes(goodSymbol({ pins: [] }))).toContain("symbol.no_pins");
  });

  // ja: 指摘は見つかった分を全部返すので、直しては保存し直す回数が減る
  it("returns every problem at once instead of stopping at the first", () => {
    const def = goodSymbol({ id: "", pins: [], text_slots: [] });
    expect(codes(def).sort()).toEqual(
      ["symbol.id_empty", "symbol.no_pins", "symbol.slot_missing"].sort(),
    );
  });
});

describe("symbol editor pre-save checklist", () => {
  // ja: 保存前チェックは「グリッド」「番号の重複」「必須属性」「任意属性」の4行を必ず出す
  it("always shows the four pre-save rows (grid, numbers, required and optional attributes)", () => {
    const rows = symbolChecks(goodSymbol(), []);
    expect(rows.map((r) => r.code).slice(0, 4)).toEqual([
      "check.pins_grid",
      "check.pin_numbers",
      "check.slot_required",
      "check.slot_optional",
    ]);
  });

  // ja: 過不足のないシンボルでは、任意属性が未配置なら注意、それ以外は合格になる
  it("marks the optional-attribute row as a caution while some optional slots are unplaced", () => {
    const rows = symbolChecks(goodSymbol(), []);
    expect(rows.slice(0, 3).map((r) => r.level)).toEqual(["ok", "ok", "ok"]);
    expect(rows[0].params.count).toBe(2);
    expect(rows[3].level).toBe("warn");
    expect(rows[3].params.keys).toBe("PART / DESC / RATING");
  });

  // ja: 任意属性をすべて配置すると4行すべてが合格になる
  it("marks all four rows as passed once every optional attribute is placed", () => {
    const def = goodSymbol({
      text_slots: [
        { key: "TAG", at: { x: 0, y: -10 }, height: 2.5 },
        { key: "PART", at: { x: 0, y: -7.5 }, height: 2.5 },
        { key: "DESC", at: { x: 0, y: 10 }, height: 2.5 },
        { key: "RATING", at: { x: 0, y: 12.5 }, height: 2.5 },
      ],
    });
    expect(symbolChecks(def, []).map((r) => r.level)).toEqual(["ok", "ok", "ok", "ok"]);
  });

  // ja: 問題のある行はエラーになり、残りの理由は5行目以降に並ぶ
  it("turns the affected row into an error and lists the remaining reasons after it", () => {
    const rows = symbolChecks(goodSymbol({ id: "", pins: [pin("1", 1.2, 0)] }), []);
    expect(rows[0].level).toBe("error");
    expect(rows.map((r) => r.code)).toContain("symbol.id_empty");
  });

  // ja: エラーが1つでもあるあいだは保存ボタンを押せない
  it("keeps the save button disabled while any row is an error", () => {
    expect(symbolCanSave(symbolChecks(goodSymbol(), []))).toBe(true);
    expect(symbolCanSave(symbolChecks(goodSymbol({ text_slots: [] }), []))).toBe(false);
  });
});
