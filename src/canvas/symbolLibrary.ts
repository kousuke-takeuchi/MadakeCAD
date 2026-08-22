// 同梱シンボルライブラリの絞り込み・カテゴリ分け (部品挿入ダイアログが使う)。
// カテゴリの並びと所属はRust側 madake-core/src/symbol.rs が正 (CATEGORY_ORDER)。
// ここではバックエンドから来た並びを保ったままグループ化する。

import type { SymbolDef } from "../ipc";

/**
 * 検索語がシンボルに一致するか。名称 (英語・日本語)・シンボルid・検索キーワードの
 * いずれかを部分一致で見る (大文字小文字は無視)。空の検索語はすべてに一致する。
 */
export function symbolMatchesQuery(def: SymbolDef, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  const fields = [def.name, def.name_ja, def.id, ...(def.keywords ?? [])];
  return fields.some((f) => f.toLowerCase().includes(q));
}

/**
 * カテゴリごとにまとめる。カテゴリの出現順・カテゴリ内の並びはライブラリの並び順のまま
 * (Rust側でカテゴリごとに連続して並んでいる)。
 */
export function groupSymbolsByCategory(
  defs: readonly SymbolDef[],
  query = "",
): [string, SymbolDef[]][] {
  const map = new Map<string, SymbolDef[]>();
  for (const def of defs) {
    if (!symbolMatchesQuery(def, query)) continue;
    const arr = map.get(def.category) ?? [];
    arr.push(def);
    map.set(def.category, arr);
  }
  return [...map.entries()];
}
