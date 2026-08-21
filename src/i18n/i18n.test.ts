// UI多言語対応の仕様テスト (docs/internal/specs/i18n.md)。
import { describe, expect, it } from "vitest";
import { SUPPORTED_LOCALES, resolveLocale, i18n } from "./index";
import en from "../locales/en.json";
import ja from "../locales/ja.json";

/** カタログのキーを "a.b.c" 形式で列挙する。 */
function keysOf(obj: Record<string, unknown>, prefix = ""): string[] {
  return Object.entries(obj).flatMap(([key, value]) => {
    const path = prefix ? `${prefix}.${key}` : key;
    return value !== null && typeof value === "object"
      ? keysOf(value as Record<string, unknown>, path)
      : [path];
  });
}

describe("i18n", () => {
  // ja: UIの既定言語は英語で、フォールバックも英語
  it("defaults to English and falls back to English", () => {
    expect(i18n.global.locale.value).toBe("en");
    expect(i18n.global.fallbackLocale.value).toBe("en");
  });

  // ja: 実装済みロケールは英語と日本語
  it("ships English and Japanese catalogs", () => {
    expect(SUPPORTED_LOCALES).toEqual(["en", "ja"]);
  });

  // ja: 英語と日本語のカタログはキーが完全に一致する(訳し漏れをCIで検出)
  it("keeps the English and Japanese catalogs key-identical", () => {
    expect(keysOf(ja).sort()).toEqual(keysOf(en).sort());
  });

  // ja: カタログの文字列は空にできない(キーだけ足して訳し忘れることを防ぐ)
  it("rejects empty strings in either catalog", () => {
    for (const catalog of [en, ja]) {
      for (const key of keysOf(catalog)) {
        const value = key.split(".").reduce<unknown>((o, k) => (o as Record<string, unknown>)[k], catalog);
        expect(value, key).toBeTypeOf("string");
        expect((value as string).length, key).toBeGreaterThan(0);
      }
    }
  });

  // ja: 言語タグは大文字・余白があっても解決でき、未知・空の値は英語になる
  it("resolves language tags leniently and falls back to English for unknown values", () => {
    expect(resolveLocale("ja")).toBe("ja");
    expect(resolveLocale(" JA ")).toBe("ja");
    expect(resolveLocale("fr")).toBe("en");
    expect(resolveLocale("")).toBe("en");
    expect(resolveLocale(null)).toBe("en");
    expect(resolveLocale(undefined)).toBe("en");
  });
});
