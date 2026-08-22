// プロジェクト内検索 (⌘F) の表示ロジック。検索そのものはRust側 (madake-core/src/search.rs)
// の純関数が行い、ここは「結果をどう並べ・どう見せ・どこへ飛ばすか」だけを持つ。
//
// - フィルタチップ (すべて/参照記号/型番/ネット/テキスト) → 検索対象種別への読み替え
// - 行クリック・Enter巡回のジャンプ先 (シートid+選択するエンティティ)
// - 「種別」列と「所在」列の文字列の作り方
//
// 文言そのもの (「リレー」「コイル」等) はi18nカタログが持つ。ここが返すのはキーと引数。

import type { DeviceFunctionKind, SearchHit, SearchKind } from "../ipc";

/** 検索バーのフィルタチップ (デザイン: .pen「M4デザイン - 検索/デバイスナビゲータ/Surfer」)。 */
export type SearchFilter = "all" | "reference" | "value" | "net" | "text";

/** チップの並び (デザイン準拠)。 */
export const SEARCH_FILTERS: SearchFilter[] = ["all", "reference", "value", "net", "text"];

/**
 * フィルタチップが対象にする検索種別。
 *
 * 「すべて」は空配列 = Rust側で全種別。「ネット」はネット名と線番の両方を指す
 * (図面を読む人にとってはどちらも「ネットの名前」なので、チップは1つにまとめている)。
 */
export function kindsForFilter(filter: SearchFilter): SearchKind[] {
  switch (filter) {
    case "all":
      return [];
    case "reference":
      return ["reference"];
    case "value":
      return ["value"];
    case "net":
      return ["net", "wire_no"];
    case "text":
      return ["text"];
  }
}

/**
 * 検索結果の行クリック・Enter巡回のジャンプ先。
 * 呼び出し側はこのシートへ切り替えてから `controller.reveal(entityIds)` を呼ぶ
 * (検証結果パネルの行クリックと同じ機構)。
 */
export function searchJumpTarget(hit: SearchHit): { sheetId: string; entityIds: string[] } {
  return { sheetId: hit.sheet_id, entityIds: [hit.entity_id] };
}

/**
 * Enter=次へ / Shift+Enter=前へ の巡回。端まで行ったら反対の端へ回り込む。
 * 結果が空なら -1 (選択なし) のまま。
 */
export function cycleIndex(count: number, current: number, step: number): number {
  if (count <= 0) return -1;
  if (current < 0) return step >= 0 ? 0 : count - 1;
  return (((current + step) % count) + count) % count;
}

/** 結果パネルの「所在」列 (デザイン: 「シート1 · C2」)。 */
export function hitLocation(hit: SearchHit): string {
  return `${hit.sheet_name} · ${hit.zone}`;
}

/** i18nキーと引数の組 (呼び出し側が `t(key, params)` する)。 */
export interface LabelSpec {
  key: string;
  params?: Record<string, string>;
}

/**
 * デバイスの中での機能の呼び名 (例 「接点 13-14」)。
 * デバイスナビゲータのツリーとSurferポップアップが使う (デバイス名の下に並ぶので種別は略す)。
 */
export function functionLabel(kind: DeviceFunctionKind, terminals: string): LabelSpec {
  return { key: `devices.function.${camelFunction(kind)}`, params: { terminals } };
}

/**
 * 結果パネルの「種別」列。
 *
 * シンボルのヒットは、そのシンボルがデバイスで果たしている機能で説明する
 * (参照記号「K1」のヒットなら「リレー 接点 13-14」)。ツリーと違って前後の文脈が
 * 無いので、デバイスの種別 (リレー/端子台) から書く。機能の分からないヒットは
 * 検索の対象種別そのもの (「ネット名」「線番」「注記テキスト」) で説明する。
 */
export function hitLabel(hit: SearchHit): LabelSpec {
  if (hit.function) {
    return {
      key: `search.function.${camelFunction(hit.function)}`,
      params: { terminals: hit.terminals },
    };
  }
  return { key: `search.kind.${camelKind(hit.kind)}` };
}

function camelFunction(kind: DeviceFunctionKind): string {
  return kind === "contact_no" ? "contactNo" : kind === "contact_nc" ? "contactNc" : kind;
}

function camelKind(kind: SearchKind): string {
  return kind === "wire_no" ? "wireNo" : kind;
}
