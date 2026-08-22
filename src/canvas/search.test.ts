import { describe, expect, it } from "vitest";
import type { SearchHit } from "../ipc";
import {
  SEARCH_FILTERS,
  cycleIndex,
  functionLabel,
  hitLabel,
  hitLocation,
  kindsForFilter,
  searchJumpTarget,
} from "./search";

function hit(over: Partial<SearchHit> = {}): SearchHit {
  return {
    kind: "reference",
    text: "K1",
    detail: "MY2N",
    function: "coil",
    terminals: "A1-A2",
    sheet_id: "sheet-1",
    sheet_no: 1,
    sheet_name: "シート1",
    zone: "C2",
    entity_id: "entity-1",
    ...over,
  };
}

describe("検索バーのフィルタ", () => {
  // ja: 「すべて」チップは対象を絞らない
  it("the All chip does not narrow the targets", () => {
    expect(kindsForFilter("all")).toEqual([]);
  });

  // ja: 「ネット」チップはネット名と線番の両方を探す(図面上はどちらもネットの名前)
  it("the Net chip searches both net names and wire numbers", () => {
    expect(kindsForFilter("net")).toEqual(["net", "wire_no"]);
  });

  // ja: 参照記号・型番・テキストのチップはそれぞれ1種類だけに絞る
  it("the reference, part-number and text chips each narrow to one target", () => {
    expect(kindsForFilter("reference")).toEqual(["reference"]);
    expect(kindsForFilter("value")).toEqual(["value"]);
    expect(kindsForFilter("text")).toEqual(["text"]);
  });

  // ja: チップはデザインどおり「すべて/参照記号/型番/ネット/テキスト」の順に並ぶ
  it("the chips are ordered All, reference, part number, net, text", () => {
    expect(SEARCH_FILTERS).toEqual(["all", "reference", "value", "net", "text"]);
  });
});

describe("Enter巡回", () => {
  // ja: Enterは次の結果へ進み、末尾まで行くと先頭へ回り込む
  it("Enter steps to the next result and wraps around at the end", () => {
    expect(cycleIndex(3, 0, 1)).toBe(1);
    expect(cycleIndex(3, 2, 1)).toBe(0);
  });

  // ja: Shift+Enterは前の結果へ戻り、先頭からは末尾へ回り込む
  it("Shift+Enter steps back and wraps around at the start", () => {
    expect(cycleIndex(3, 1, -1)).toBe(0);
    expect(cycleIndex(3, 0, -1)).toBe(2);
  });

  // ja: まだ何も選んでいなければ、Enterで先頭、Shift+Enterで末尾を選ぶ
  it("with nothing selected yet, Enter picks the first hit and Shift+Enter the last", () => {
    expect(cycleIndex(3, -1, 1)).toBe(0);
    expect(cycleIndex(3, -1, -1)).toBe(2);
  });

  // ja: 結果が0件なら選択は無いまま(巡回しても何も起きない)
  it("with no results there is nothing to select", () => {
    expect(cycleIndex(0, -1, 1)).toBe(-1);
    expect(cycleIndex(0, 2, -1)).toBe(-1);
  });
});

describe("結果行", () => {
  // ja: 行クリックのジャンプ先は「そのヒットのシート」と「選択する1エンティティ」
  it("clicking a row jumps to that hit's sheet and selects its entity", () => {
    expect(searchJumpTarget(hit())).toEqual({ sheetId: "sheet-1", entityIds: ["entity-1"] });
  });

  // ja: 所在の列はシート名とゾーンを中黒でつないで出す
  it("the location column joins the sheet name and the zone", () => {
    expect(hitLocation(hit())).toBe("シート1 · C2");
  });

  // ja: シンボルのヒットの種別欄は、そのシンボルがデバイスで果たす機能で説明する
  it("a symbol hit is described by the function it plays in its device", () => {
    expect(hitLabel(hit({ function: "coil", terminals: "A1-A2" }))).toEqual({
      key: "search.function.coil",
      params: { terminals: "A1-A2" },
    });
    expect(hitLabel(hit({ function: "contact_no", terminals: "13-14" }))).toEqual({
      key: "search.function.contactNo",
      params: { terminals: "13-14" },
    });
    expect(hitLabel(hit({ function: "contact_nc", terminals: "21-22" }))).toEqual({
      key: "search.function.contactNc",
      params: { terminals: "21-22" },
    });
  });

  // ja: 機能を持たないヒット(ネット名・線番・注記)は検索の対象種別そのもので説明する
  it("hits without a device function are described by the search target itself", () => {
    expect(hitLabel(hit({ kind: "net", function: null, terminals: "" }))).toEqual({
      key: "search.kind.net",
    });
    expect(hitLabel(hit({ kind: "wire_no", function: null, terminals: "" }))).toEqual({
      key: "search.kind.wireNo",
    });
    expect(hitLabel(hit({ kind: "text", function: null, terminals: "" }))).toEqual({
      key: "search.kind.text",
    });
  });

  // ja: ツリーとSurferでは、デバイス名の下に並ぶので種別を略した呼び名を使う
  it("in the tree and the surfer, functions drop the device kind from their label", () => {
    expect(functionLabel("terminal", "1-8")).toEqual({
      key: "devices.function.terminal",
      params: { terminals: "1-8" },
    });
    expect(functionLabel("coil", "A1-A2")).toEqual({
      key: "devices.function.coil",
      params: { terminals: "A1-A2" },
    });
    expect(functionLabel("body", "")).toEqual({
      key: "devices.function.body",
      params: { terminals: "" },
    });
  });
});
