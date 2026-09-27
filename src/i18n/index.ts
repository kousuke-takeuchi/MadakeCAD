// UI多言語対応の入口 (仕様: docs/internal/specs/i18n.md)。
// 既定=英語、フォールバック=英語。ロケールはアプリ設定の`language`に従い、
// 変更は即時反映される(再起動不要)。
import { createI18n } from "vue-i18n";
import en from "../locales/en.json";
import ja from "../locales/ja.json";
import zh from "../locales/zh.json";
import es from "../locales/es.json";
import fr from "../locales/fr.json";
import de from "../locales/de.json";

/**
 * 実装済みロケール。追加時はsrc/locales/へカタログを置き、ここと`messages`に足す
 * (設定の言語ドロップダウンはこの配列から自動で増える。翻訳者向け手順: docs/internal/specs/i18n.md §6)。
 */
export const SUPPORTED_LOCALES = ["en", "ja", "zh", "es", "fr", "de"] as const;
export type Locale = (typeof SUPPORTED_LOCALES)[number];

/** 設定の言語タグを実装済みロケールへ解決する(未知・空は英語)。 */
export function resolveLocale(language: string | null | undefined): Locale {
  const tag = (language ?? "").trim().toLowerCase();
  return (SUPPORTED_LOCALES as readonly string[]).includes(tag) ? (tag as Locale) : "en";
}

export const i18n = createI18n({
  legacy: false,
  locale: "en",
  fallbackLocale: "en",
  messages: { en, ja, zh, es, fr, de },
});

/** アクティブなUI言語を切り替える(設定の読込時・変更時に呼ぶ)。 */
export function setLocale(language: string | null | undefined): void {
  i18n.global.locale.value = resolveLocale(language);
}
