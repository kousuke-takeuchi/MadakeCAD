// 部品挿入ダイアログの極数バー (極数ステッパー + 端子台の段数ステッパー) が
// どのシンボルidを配置するか (G6 多段端子台)。
import { describe, expect, it } from "vitest";

import en from "../locales/en.json";
import ja from "../locales/ja.json";
import { clampLevels, clampPins, dynamicSymbolId, supportsLevels } from "./symbolPickerDynamic";

describe("clampLevels", () => {
  // ja: 段数ステッパーは1〜3段に収める(1より小さい値は1段、3より大きい値は3段になる)
  it("keeps the level count inside the supported range of 1 to 3 levels", () => {
    expect(clampLevels(1)).toBe(1);
    expect(clampLevels(2)).toBe(2);
    expect(clampLevels(3)).toBe(3);
    expect(clampLevels(0)).toBe(1);
    expect(clampLevels(-5)).toBe(1);
    expect(clampLevels(4)).toBe(3);
    expect(clampLevels(99)).toBe(3);
  });

  // ja: 段数の入力欄に数字以外や小数を入れても、1〜3段の整数に直される
  it("rounds a fractional level count and falls back to one level for a non-number", () => {
    expect(clampLevels(2.4)).toBe(2);
    expect(clampLevels(1.6)).toBe(2);
    expect(clampLevels(Number.NaN)).toBe(1);
  });
});

describe("clampPins", () => {
  // ja: 極数ステッパーは1極から50極までに収める
  it("keeps the pole count inside the supported range of 1 to 50 poles", () => {
    expect(clampPins(1)).toBe(1);
    expect(clampPins(0)).toBe(1);
    expect(clampPins(50)).toBe(50);
    expect(clampPins(51)).toBe(50);
    expect(clampPins(Number.NaN)).toBe(1);
  });
});

describe("supportsLevels", () => {
  // ja: 段数ステッパーは端子台を選んだときだけ出る(コネクタには段という概念が無い)
  it("offers the levels stepper for a terminal block only, not for a connector", () => {
    expect(supportsLevels("terminal_block")).toBe(true);
    expect(supportsLevels("connector")).toBe(false);
    expect(supportsLevels(null)).toBe(false);
  });
});

describe("dynamicSymbolId", () => {
  // ja: 段数が既定の1段のままなら、これまでどおりの単段の端子台 (terminal_block_8p) が置かれる
  it("places the plain single-level terminal block id when the level count is left at one", () => {
    expect(dynamicSymbolId("terminal_block", 8, 1)).toBe("terminal_block_8p");
    expect(dynamicSymbolId("terminal_block", 8, 1)).not.toContain("_1l");
  });

  // ja: 段数を2段にすると2段の端子台 (terminal_block_8p_2l) が置かれる
  it("places the multi-level terminal block id when the level count is raised", () => {
    expect(dynamicSymbolId("terminal_block", 8, 2)).toBe("terminal_block_8p_2l");
    expect(dynamicSymbolId("terminal_block", 4, 3)).toBe("terminal_block_4p_3l");
  });

  // ja: コネクタは段数を上げても段は付かない (コネクタに段は無い)
  it("never adds a level suffix to a connector, whatever the level count says", () => {
    expect(dynamicSymbolId("connector", 6, 2)).toBe("connector_6p");
    expect(dynamicSymbolId("connector", 6, 1)).toBe("connector_6p");
  });
});

describe("段数ステッパーの表示文字列", () => {
  // ja: 段数ステッパーのラベルと範囲ヒントは英語カタログにも日本語カタログにも入っている
  it("has the levels stepper label and hint in both the English and the Japanese catalogue", () => {
    for (const catalogue of [en, ja]) {
      expect(catalogue.symbolPicker.levelsLabel).toBeTruthy();
      expect(catalogue.symbolPicker.levelsHint).toContain("{max}");
    }
  });
});
