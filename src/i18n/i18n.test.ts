// UI多言語対応の仕様テスト (docs/internal/specs/i18n.md)。
import { describe, expect, it } from "vitest";
import { SUPPORTED_LOCALES, resolveLocale, i18n } from "./index";
import en from "../locales/en.json";
import ja from "../locales/ja.json";
import zh from "../locales/zh.json";
import es from "../locales/es.json";
import fr from "../locales/fr.json";
import de from "../locales/de.json";

/** 実装済みカタログ (ロケール → カタログ)。 */
const CATALOGS: Record<string, Record<string, unknown>> = { en, ja, zh, es, fr, de };

/** カタログのキーを "a.b.c" 形式で列挙する。 */
function keysOf(obj: Record<string, unknown>, prefix = ""): string[] {
  return Object.entries(obj).flatMap(([key, value]) => {
    const path = prefix ? `${prefix}.${key}` : key;
    return value !== null && typeof value === "object"
      ? keysOf(value as Record<string, unknown>, path)
      : [path];
  });
}

function valueAt(catalog: Record<string, unknown>, key: string): unknown {
  return key.split(".").reduce<unknown>((o, k) => (o as Record<string, unknown>)[k], catalog);
}

/** 文字列中の名前付きプレースホルダ (`{count}` 等) を昇順で列挙する。 */
function placeholdersOf(text: string): string[] {
  return (text.match(/\{[a-zA-Z_]+\}/g) ?? []).sort();
}

describe("i18n", () => {
  // ja: UIの既定言語は英語で、フォールバックも英語
  it("defaults to English and falls back to English", () => {
    expect(i18n.global.locale.value).toBe("en");
    expect(i18n.global.fallbackLocale.value).toBe("en");
  });

  // ja: 実装済みロケールは英語・日本語・中国語・スペイン語・フランス語・ドイツ語の6つ
  it("ships English, Japanese, Chinese, Spanish, French and German catalogs", () => {
    expect(SUPPORTED_LOCALES).toEqual(["en", "ja", "zh", "es", "fr", "de"]);
    for (const locale of SUPPORTED_LOCALES) expect(Object.keys(CATALOGS)).toContain(locale);
  });

  // ja: 全ロケールのカタログは英語とキーが完全に一致する(訳し漏れ・余分なキーをCIで検出)
  it("keeps every catalog key-identical to the English one", () => {
    const expected = keysOf(en).sort();
    for (const [locale, catalog] of Object.entries(CATALOGS)) {
      expect(keysOf(catalog).sort(), locale).toEqual(expected);
    }
  });

  // ja: どのカタログでも文字列は空にできない(キーだけ足して訳し忘れることを防ぐ)
  it("rejects empty strings in any catalog", () => {
    for (const [locale, catalog] of Object.entries(CATALOGS)) {
      for (const key of keysOf(catalog)) {
        const value = valueAt(catalog, key);
        expect(value, `${locale}: ${key}`).toBeTypeOf("string");
        expect((value as string).trim().length, `${locale}: ${key}`).toBeGreaterThan(0);
      }
    }
  });

  // ja: 翻訳は英語と同じ名前付きプレースホルダを持つ(`{count}`の欠落・綴り違いを検出)
  it("keeps the same named placeholders as English in every translation", () => {
    for (const [locale, catalog] of Object.entries(CATALOGS)) {
      for (const key of keysOf(en)) {
        expect(placeholdersOf(valueAt(catalog, key) as string), `${locale}: ${key}`).toEqual(
          placeholdersOf(valueAt(en, key) as string),
        );
      }
    }
  });

  // ja: 複数形の文(`|`区切り)は英語と同じ数の形を持つ
  it("keeps the same number of plural forms as English in every translation", () => {
    for (const [locale, catalog] of Object.entries(CATALOGS)) {
      for (const key of keysOf(en)) {
        const enForms = (valueAt(en, key) as string).split("|").length;
        expect((valueAt(catalog, key) as string).split("|").length, `${locale}: ${key}`).toBe(
          enForms,
        );
      }
    }
  });

  // ja: 言語名は各言語の自称で表示される(英語に翻訳しない)
  it("shows each language name as its own endonym", () => {
    for (const [locale, catalog] of Object.entries(CATALOGS)) {
      expect((catalog.language as Record<string, string>).ja, locale).toBe("日本語");
      expect((catalog.language as Record<string, string>).de, locale).toBe("Deutsch");
    }
  });

  // ja: 言語タグは大文字・余白があっても解決でき、未知・空の値は英語になる
  it("resolves language tags leniently and falls back to English for unknown values", () => {
    expect(resolveLocale("ja")).toBe("ja");
    expect(resolveLocale(" JA ")).toBe("ja");
    expect(resolveLocale("de")).toBe("de");
    expect(resolveLocale("pt")).toBe("en");
    expect(resolveLocale("")).toBe("en");
    expect(resolveLocale(null)).toBe("en");
    expect(resolveLocale(undefined)).toBe("en");
  });
});
