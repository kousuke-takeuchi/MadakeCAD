import { describe, it, expect } from "vitest";
import { parseSimCondition } from "./simConditions";

describe("simulation condition input", () => {
  // ja: 「参照記号.属性=値」は部品の属性の置き換えとして読む (空白は無視)
  it("reads REF.ATTR=VALUE as an attribute override, ignoring spaces", () => {
    expect(parseSimCondition("M1.current_a=40")).toEqual({ kind: "set", reference: "M1", attr: "current_a", value: "40" });
    expect(parseSimCondition("  M1 . current_a = 40 ")).toEqual({ kind: "set", reference: "M1", attr: "current_a", value: "40" });
  });

  // ja: 「短絡 ネット名」と「short ネット名」はネットの短絡として読む
  it("reads 短絡 NET and short NET as a short of that net", () => {
    expect(parseSimCondition("短絡 P24")).toEqual({ kind: "short", net: "P24" });
    expect(parseSimCondition("short N048")).toEqual({ kind: "short", net: "N048" });
  });

  // ja: どちらの形でもない入力 (値の無い置き換え・ネット名の無い短絡・ただの文字) は読めないと返す
  it("refuses input that is neither form", () => {
    for (const bad of ["", "M1=40", "M1.current_a", "M1.current_a=", "短絡", "short", "hello"]) {
      expect(parseSimCondition(bad), bad).toEqual({ kind: "error" });
    }
  });

  // ja: 大文字の SHORT、全角の空白、日本語入力の全角の ． ＝ も読める。値には空白や = を含められる
  it("accepts upper-case SHORT, full-width spaces and full-width punctuation, and values with spaces or =", () => {
    expect(parseSimCondition("SHORT P24")).toEqual({ kind: "short", net: "P24" });
    expect(parseSimCondition("短絡　P24")).toEqual({ kind: "short", net: "P24" });
    expect(parseSimCondition("Ｍ１．current_a＝４０")).toEqual({ kind: "set", reference: "M1", attr: "current_a", value: "40" });
    expect(parseSimCondition("M1.note=a = b")).toEqual({ kind: "set", reference: "M1", attr: "note", value: "a = b" });
  });

  // ja: 短絡のネット名の後ろに余計な語があれば読めないと返す
  it("refuses a short with extra words after the net name", () => {
    expect(parseSimCondition("短絡 P24 extra")).toEqual({ kind: "error" });
  });
});
