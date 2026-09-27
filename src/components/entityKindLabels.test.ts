// プロパティパネルの「種類」行のラベルの仕様。
// 種別が増えたときにラベルを足し忘れると、選んだ相手の名前が出ない (「 (1)」だけになる)。
import { describe, expect, it } from "vitest";

import en from "../locales/en.json";
import ja from "../locales/ja.json";
import { ENTITY_KINDS, entityKindLabels } from "./entityKindLabels";

/** ドット区切りのi18nキーを辞書から引く (vue-i18nの`t`の代わり)。 */
function lookup(dict: Record<string, unknown>): (key: string) => string {
  return (key: string) => {
    let node: unknown = dict;
    for (const part of key.split(".")) {
      node = (node as Record<string, unknown> | undefined)?.[part];
    }
    return typeof node === "string" ? node : "";
  };
}

describe("entity kind labels", () => {
  // ja: 図面に置ける全てのエンティティ種別に「種類」行のラベルがある (電線品番凡例ブロックも盤の凡例表も含む)。
  it("has a label for every entity kind that can be placed on a sheet", () => {
    const labels = entityKindLabels((key) => key);
    for (const kind of ENTITY_KINDS) {
      expect(labels[kind], kind).toBeTruthy();
    }
    expect(Object.keys(labels).sort()).toEqual([...ENTITY_KINDS].sort());
  });

  // ja: ラベルが引くi18nキーは英語・日本語の両方に実体があり、空文字にならない。
  it("resolves every label through keys that exist in both languages", () => {
    for (const dict of [en, ja]) {
      const labels = entityKindLabels(lookup(dict as unknown as Record<string, unknown>));
      for (const kind of ENTITY_KINDS) {
        expect(labels[kind], `${kind}`).not.toBe("");
      }
    }
  });

  // ja: 電線品番凡例ブロックを選ぶと、種別の名前が出る (英語・日本語それぞれの短い名前)。
  it("names the wire legend block in both languages", () => {
    expect(entityKindLabels(lookup(en as unknown as Record<string, unknown>)).wire_legend).toBe(
      "Wire legend",
    );
    expect(entityKindLabels(lookup(ja as unknown as Record<string, unknown>)).wire_legend).toBe(
      "電線品番凡例",
    );
  });
});
