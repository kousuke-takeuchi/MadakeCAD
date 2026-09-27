/**
 * リボンの小ボタンの並べ方。
 *
 * リボン本体 (`.ribbon-body`) の高さは固定で、1つのグループの1列に入る小ボタンは
 * [`RIBBON_SMALL_ROWS`] 個まで。それを超えて1列に積むと、あふれたボタンはリボンの下端から
 * はみ出し、下に重なる画面 (ツリー・キャンバス) に隠れて**押せなくなる**。
 * グループの小ボタンは必ずこの関数で列に分けてから描く。
 */
export const RIBBON_SMALL_ROWS = 4;

/** 小ボタンの並びを、1列 `rows` 個までの列に前から詰めて分ける (順番は変えない)。 */
export function smallColumns<T>(items: T[], rows: number = RIBBON_SMALL_ROWS): T[][] {
  const columns: T[][] = [];
  for (let i = 0; i < items.length; i += rows) columns.push(items.slice(i, i + rows));
  return columns;
}
