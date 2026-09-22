// Rust側 madake-core/src/pin_groups.rs の parse_pin_groups / pin_group_bracket と
// 同一ルールを検証する (parityテスト)。
import { describe, expect, it } from "vitest";

import type { SymbolInstance } from "../ipc";

import { blockSymbol, captionLineAnchors, dynamicSymbol } from "./dynamicSymbol";
import {
  chartOrderPinDefs,
  chartOrderPinNumbers,
  PIN_GROUP_INVALID,
  PIN_GROUP_OVERLAP,
  PIN_GROUP_UNKNOWN_PIN,
  parsePinGroups,
  pinGroupBracket,
  pinGroupBrackets,
  pinGroupIssues,
  wrapCaption,
} from "./pinGroups";

/** 回転もミラーもしていない、原点に置いたインスタンス (用紙座標=ローカル座標になる)。 */
function unrotated(symbolId: string): SymbolInstance {
  return {
    id: "s1",
    symbol_id: symbolId,
    at: { x: 0, y: 0 },
    rotation: 0,
    mirror: false,
    reference: "",
    value: "",
    attrs: {},
  } as SymbolInstance;
}

function order(id: string): string[] {
  return chartOrderPinNumbers(dynamicSymbol(id)!);
}

describe("parsePinGroups", () => {
  // ja: 「範囲:名前」1項目と、ダッシュの無い単ピン指定は、それぞれ1つのグループとして解析される (Rust側とparity)
  it("parses a range and a dash-less single pin to one group each", () => {
    const parsed = parsePinGroups("1-4:電源,6:予備", order("terminal_block_8p"));
    expect(parsed.issues).toEqual([]);
    expect(parsed.groups).toEqual([
      { rangeText: "1-4", caption: "電源", first: "1", last: "4" },
      { rangeText: "6", caption: "予備", first: "6", last: "6" },
    ]);
  });

  // ja: チャート順で終点が始点より前に書かれた範囲は不正として報告される
  it("reports a backwards range as invalid", () => {
    const parsed = parsePinGroups("4-1:電源", order("terminal_block_8p"));
    expect(parsed.groups).toEqual([]);
    expect(parsed.issues[0].code).toBe(PIN_GROUP_INVALID);
  });

  // ja: そのインスタンスに無いピンを指す範囲は不明ピンの診断コードで報告される
  it("reports a range naming an unknown pin", () => {
    const parsed = parsePinGroups("1-99:電源", order("terminal_block_8p"));
    expect(parsed.groups).toEqual([]);
    expect(parsed.issues[0].code).toBe(PIN_GROUP_UNKNOWN_PIN);
  });

  // ja: コロンが無い・範囲が空の読めない記述はパニックせず不正として報告され、残りの項目は解析される
  it("reports unparseable text without throwing, keeping the rest", () => {
    const parsed = parsePinGroups("nonsense,-4:x,1-2:良い", order("terminal_block_8p"));
    expect(parsed.groups).toHaveLength(1);
    expect(parsed.groups[0].caption).toBe("良い");
    expect(parsed.issues).toHaveLength(2);
    expect(parsed.issues.every((i) => i.code === PIN_GROUP_INVALID)).toBe(true);
  });

  // ja: チャート順の範囲が重なる2つのグループはどちらも報告されグループにならないが、重ならない3つ目は残る
  it("rejects both of two overlapping groups but keeps a third that does not overlap", () => {
    const parsed = parsePinGroups("1-4:A,3-5:B,6-8:C", order("terminal_block_8p"));
    expect(parsed.groups).toEqual([{ rangeText: "6-8", caption: "C", first: "6", last: "8" }]);
    expect(parsed.issues).toHaveLength(2);
    expect(parsed.issues.every((i) => i.code === PIN_GROUP_OVERLAP)).toBe(true);
  });

  // ja: 多段端子台のチャート順は段ごとに並ぶ (1a,2a,…,1b,2b,…) ので、"1a-4a"は1段の中で連続する
  it("accepts a multi-level range that stays within one level", () => {
    const ord = order("terminal_block_8p_2l");
    expect(ord.slice(0, 4)).toEqual(["1a", "2a", "3a", "4a"]);
    const parsed = parsePinGroups("1a-4a:A系統", ord);
    expect(parsed.issues).toEqual([]);
    expect(parsed.groups).toHaveLength(1);
  });

  // ja: ピン番号そのものに「-」が入っている端子はピングループに指定できず、使えない理由がそのまま報告される (Rust側とparity)
  it("refuses a pin whose own number contains a hyphen, with its own reason", () => {
    const parsed = parsePinGroups("A-1:電源", ["A-1", "B"]);
    expect(parsed.groups).toEqual([]);
    expect(parsed.issues[0].code).toBe(PIN_GROUP_INVALID);
    expect(parsed.issues[0].message).toContain("-");
  });
});

describe("wrapCaption", () => {
  // ja: キャプションの折り返しは4文字ごとに順番どおり分割する
  it("splits into chunks of at most 4 characters", () => {
    expect(wrapCaption("電源系統一式")).toEqual(["電源系統", "一式"]);
    expect(wrapCaption("信号")).toEqual(["信号"]);
    expect(wrapCaption("")).toEqual([""]);
  });
});

describe("pinGroupBracket", () => {
  // ja: シンボル左側のピンにかける括弧は、いちばん上から下のピンまで縦棒でつながり、キャプションはさらに左に置かれる
  it("draws a vertical bar further left than left-side pins, with the caption further out still", () => {
    const def = dynamicSymbol("terminal_block_4p")!;
    const pins = chartOrderPinDefs(def).slice(0, 2); // "1","2" (左側の足)
    const bracket = pinGroupBracket(pins, "電源")!;
    expect(bracket.bracket.every((p) => p.x < pins[0].at.x)).toBe(true);
    const anchors = captionLineAnchors(unrotated("terminal_block_4p"), bracket);
    expect(anchors.every(([, p]) => p.x < bracket.bracket[0].x)).toBe(true);
  });

  // ja: ブロックでも括弧は左のピンの足先より外に出て、キャプションはさらに外に置かれる (Rust側の3一族テストとparity)
  it("puts a block's bracket beyond the pin stubs and its caption beyond the bracket", () => {
    const def = blockSymbol({
      block_pins: '{"left":[{"no":"1","name":"IN1"},{"no":"2","name":"IN2"}]}',
    })!;
    const stubEnd = def.pins[0].at.x; // 左の足の先端 (負)
    const brackets = pinGroupBrackets(def, "1-2:電源系統一式");
    expect(brackets).toHaveLength(1);
    expect(brackets[0].bracket.every((p) => p.x < stubEnd)).toBe(true);
    const barX = Math.min(...brackets[0].bracket.map((p) => p.x));
    const anchors = captionLineAnchors(unrotated("block"), brackets[0]);
    expect(anchors.every(([, p]) => p.x < barX)).toBe(true);
  });

  // ja: コネクタは右側にピンが出るので、括弧とキャプションは足先より右に出る (Rust側の3一族テストとparity)
  it("puts a connector's bracket to the right of its pin stubs", () => {
    const def = dynamicSymbol("connector_8p")!;
    const stubEnd = def.pins[0].at.x; // 右の足の先端 (正)
    const brackets = pinGroupBrackets(def, "1-4:電源系統一式");
    expect(brackets).toHaveLength(1);
    expect(brackets[0].bracket.every((p) => p.x > stubEnd)).toBe(true);
    const barX = Math.max(...brackets[0].bracket.map((p) => p.x));
    const anchors = captionLineAnchors(unrotated("connector_8p"), brackets[0]);
    expect(anchors.every(([, p]) => p.x > barX)).toBe(true);
  });

  // ja: 多段端子台で2段目の端子にかけたグループの括弧は、その段の列の外側に出る (列と列の間には入らない、Rust側とparity)
  it("brackets the outside of a multi-level terminal block's own column, not the gap between columns", () => {
    const def = dynamicSymbol("terminal_block_8p_2l")!;
    // 2段の列は中心x=-5と+5、箱はそれぞれ±2.5なので右の列の外縁はx=7.5
    const brackets = pinGroupBrackets(def, "1b-4b:B系統");
    expect(brackets).toHaveLength(1);
    expect(brackets[0].bracket.every((p) => p.x > 7.5)).toBe(true);
    const anchors = captionLineAnchors(unrotated("terminal_block_8p_2l"), brackets[0]);
    expect(anchors.every(([, p]) => p.x > 7.5)).toBe(true);
  });

  // ja: 多段端子台の段をまたぐ範囲は、黙って何も描かれるのではなく報告される (段ごとに外側の辺が逆で1本の括弧では囲めないため、Rust側とparity)
  it("reports a range that spans two levels instead of quietly drawing nothing", () => {
    const def = dynamicSymbol("terminal_block_8p_2l")!;
    expect(pinGroupBrackets(def, "4a-1b:またぎ")).toEqual([]);
    const issues = pinGroupIssues(def, "4a-1b:またぎ");
    expect(issues).toHaveLength(1);
    expect(issues[0].code).toBe(PIN_GROUP_INVALID);
    expect(issues[0].text).toContain("4a-1b");
  });

  // ja: シンボルの2つの辺にまたがるピン (向きが揃わない) への括弧は拒否される
  it("refuses a bracket across mixed pin directions", () => {
    const left = { number: "1", name: "", at: { x: -10, y: 0 }, dir: "left" as const };
    const top = { number: "2", name: "", at: { x: 0, y: -10 }, dir: "up" as const };
    expect(pinGroupBracket([left, top], "x")).toBeNull();
  });
});
