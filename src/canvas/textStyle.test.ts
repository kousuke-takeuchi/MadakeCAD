// 注記テキストの装飾 (G4)。Rust側 text_style.rs と同じトークン・同じ位置になることを確かめる。
import { describe, expect, it } from "vitest";

import type { SymbolDef, SymbolInstance } from "../ipc";
import { theme } from "./theme";
import {
  DEFAULT_TEXT_COLOR,
  FUNCTION_LABEL_ATTR,
  FUNCTION_LABEL_DY,
  FUNCTION_LABEL_FONT,
  SLOT_FUNCTION,
  TEXT_ANCHORS,
  TEXT_COLOR_TOKENS,
  canvasTextAlign,
  functionLabelFallbackAt,
  functionLabelSlot,
  functionLabelText,
  isTextColorToken,
  noteBounds,
  noteColor,
  textScreenColor,
} from "./textStyle";

function symbol(functionLabel?: string): SymbolInstance {
  return {
    id: "s1",
    symbol_id: "switch_spst",
    at: { x: 100, y: 50 },
    rotation: 0,
    mirror: false,
    reference: "SW1",
    value: "",
    attrs: functionLabel === undefined ? {} : { [FUNCTION_LABEL_ATTR]: functionLabel },
  };
}

function def(slot?: { at: { x: number; y: number }; height: number }): SymbolDef {
  return {
    id: "switch_spst",
    name: "Switch",
    name_ja: "スイッチ",
    category: "switch",
    ref_prefix: "SW",
    primitives: [],
    pins: [],
    text_slots: slot ? [{ key: SLOT_FUNCTION, at: slot.at, height: slot.height }] : [],
  };
}

describe("note styling", () => {
  // ja: 注記の色はどれも名前付きトークンで、既定のトークンも必ずその中にある
  it("offers a small fixed set of colour tokens including the default", () => {
    expect(TEXT_COLOR_TOKENS).toContain(DEFAULT_TEXT_COLOR);
    expect(TEXT_COLOR_TOKENS).toEqual(["default", "accent", "warning", "success", "muted"]);
    for (const token of TEXT_COLOR_TOKENS) expect(isTextColorToken(token)).toBe(true);
  });

  // ja: 生の色コードは色トークンではないので、図面に紛れ込むことはない
  it("does not accept a raw colour code as a colour token", () => {
    for (const value of ["#ff0000", "red", "赤", "", "DEFAULT"]) {
      expect(isTextColorToken(value)).toBe(false);
    }
  });

  // ja: 色を指定していない注記は画面でも既定色で描かれ、トークンを指定すればその色になる
  it("draws a note without a colour in the default screen colour", () => {
    expect(textScreenColor(null)).toBe(textScreenColor("default"));
    expect(textScreenColor(undefined)).toBe(textScreenColor("default"));
    expect(textScreenColor("#ff0000")).toBe(textScreenColor("default"));
    expect(textScreenColor("warning")).not.toBe(textScreenColor("default"));
    expect(textScreenColor("accent")).not.toBe(textScreenColor("warning"));
  });

  // ja: 選択中の注記は色トークンより選択ハイライトが優先されるので、選んだものが分かる
  it("shows the selection highlight instead of the note colour while it is selected", () => {
    expect(noteColor("warning", true)).toBe(theme.selection);
    expect(noteColor("warning", false)).toBe(textScreenColor("warning"));
  });

  // ja: 3つの基準位置はCanvas2Dの3つの寄せ方に対応し、既定は従来どおりの左揃えになる
  it("maps the three anchors to the three canvas text alignments", () => {
    expect(TEXT_ANCHORS).toEqual(["start", "middle", "end"]);
    expect(canvasTextAlign("start")).toBe("left");
    expect(canvasTextAlign("middle")).toBe("center");
    expect(canvasTextAlign("end")).toBe("right");
    expect(canvasTextAlign(undefined)).toBe("left");
  });
});

describe("note hit box", () => {
  const note = (anchor: "start" | "middle" | "end") => ({
    id: "t1",
    at: { x: 100, y: 50 },
    text: "ABCD",
    height: 2.5,
    rotation: 0,
    anchor,
  });

  // ja: 左揃えの注記は基準点から右へ広がる箱で掴め、箱の高さは文字高さぶんある
  it("puts the hit box to the right of the anchor point for a left-aligned note", () => {
    const { min, max } = noteBounds(note("start"));
    expect(min.x).toBeCloseTo(100, 9);
    expect(max.x).toBeGreaterThan(100);
    expect(min.y).toBeCloseTo(50 - 2.5, 9);
    expect(max.y).toBeGreaterThan(50);
  });

  // ja: 中央揃え・右揃えの注記は、基準点の左側にも箱が広がるので同じように掴める
  it("extends the hit box to the left for centred and right-aligned notes", () => {
    const centred = noteBounds(note("middle"));
    const right = noteBounds(note("end"));
    const width = centred.max.x - centred.min.x;
    expect(centred.min.x).toBeCloseTo(100 - width / 2, 9);
    expect(right.max.x).toBeCloseTo(100, 9);
    expect(right.min.x).toBeCloseTo(100 - width, 9);
  });

  // ja: 長い文字の注記ほど掴める箱も広くなるので、書いた文字のどこを押しても選べる
  it("widens the hit box with the length of the text", () => {
    const short = noteBounds({ ...note("start"), text: "A" });
    const long = noteBounds({ ...note("start"), text: "A very long note" });
    expect(long.max.x - long.min.x).toBeGreaterThan(short.max.x - short.min.x);
  });
});

describe("function label", () => {
  // ja: 機能ラベルの属性が無いシンボルには、余分に描くものが無い
  it("draws nothing extra for a symbol with no function label", () => {
    expect(functionLabelText(symbol())).toBeNull();
  });

  // ja: 空白だけの機能ラベルは「無し」と同じ扱いになり、前後の空白は取り除かれる
  it("treats a blank function label as no label and trims the surrounding spaces", () => {
    expect(functionLabelText(symbol("   "))).toBeNull();
    expect(functionLabelText(symbol(" 非常停止 "))).toBe("非常停止");
  });

  // ja: スロットを持たない同梱シンボルでは、機能ラベルは外形のすぐ下・基準点を中心に置かれる
  it("places the label just below the outline when the definition has no slot", () => {
    expect(functionLabelSlot(def())).toBeUndefined();
    const at = functionLabelFallbackAt(symbol("非常停止ボタン"), {
      min: { x: 92, y: 42 },
      max: { x: 108, y: 58 },
    });
    expect(at).toEqual({ x: 100, y: 58 + FUNCTION_LABEL_DY });
    expect(FUNCTION_LABEL_FONT).toBe(2.5);
  });

  // ja: シンボル定義がFUNCTIONスロットを持つ場合は、そのスロットの位置と文字高さに従う
  it("follows the FUNCTION slot when the symbol definition defines one", () => {
    const slot = functionLabelSlot(def({ at: { x: 0, y: -12 }, height: 3 }));
    expect(slot?.at).toEqual({ x: 0, y: -12 });
    expect(slot?.height).toBe(3);
  });
});
