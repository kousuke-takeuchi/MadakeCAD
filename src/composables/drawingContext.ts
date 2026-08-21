// 「コンテキストに追加 → 図面から追加...」で入力欄へ挿し込むタグ文字列の組み立て。
//
// ここは読み取り専用の純関数。図面(document store)は一切書き換えない
// (ドキュメントへの編集は全てmadake-coreのCommandを通す規約)。

import type { Entity, Sheet } from "../ipc";
import { entityKindLabel } from "../stores/chat";

/** タグに列挙する選択エンティティの最大数(超過分は「+N件」に畳む)。 */
const MAX_LISTED = 10;

/** 選択エンティティの表示名。参照記号があればそれ、無ければ種別名。 */
function entityLabel(entity: Entity): string {
  const reference = "reference" in entity ? entity.reference : "";
  return reference || entityKindLabel(entity.kind);
}

/**
 * 図面コンテキストのタグ文字列を作る。
 *
 * - 選択あり: `[図面コンテキスト: <シート名> / 選択: K1, 配線, ... +3件]`
 * - 選択なし: `[図面コンテキスト: シート <シート名> 全体]`
 *
 * `selection`にはアクティブシートに無いidが混ざりうる(別シートの選択が残っている等)
 * ので、シート内に見つかったものだけを列挙する。1件も残らなければ「全体」扱い。
 */
export function drawingContextTag(sheet: Sheet | null, selection: Iterable<string>): string {
  const name = sheet?.name ?? "(シートなし)";
  const entities: Entity[] = [];
  if (sheet) {
    for (const id of selection) {
      const entity = sheet.entities[id];
      if (entity) entities.push(entity);
    }
  }
  if (!entities.length) return `[図面コンテキスト: シート ${name} 全体]`;

  const labels = entities.slice(0, MAX_LISTED).map(entityLabel);
  const rest = entities.length - labels.length;
  const listed = rest > 0 ? `${labels.join(", ")} +${rest}件` : labels.join(", ");
  return `[図面コンテキスト: ${name} / 選択: ${listed}]`;
}

/**
 * 入力欄の下書きへタグを追記する。
 *
 * 既存の下書きがあれば改行で区切る(文中に紛れて読みにくくならないように)。
 */
export function appendContextTag(draft: string, tag: string): string {
  if (!draft) return tag;
  return draft.endsWith("\n") ? `${draft}${tag}` : `${draft}\n${tag}`;
}
