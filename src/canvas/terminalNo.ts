// 端子番号の読み取りと端子の印の寸法。Rust側 madake-core/src/symbol.rs の
// parse_terminal_no / TERMINAL_MARK_R / TERMINAL_NUMBER_HEIGHT と同一。
//
// **何もimportしない葉モジュール**にしてあるのは循環importを防ぐため
// (textWidth.tsと同じ流儀): 端子番号の対応表 (terminalJumpers.ts の `terminalLabels`) は
// ピングループの箱 (dynamicSymbol.ts) からも使われるので、この2つの定義が
// dynamicSymbol.ts に置かれていると terminalJumpers ↔ dynamicSymbol の循環になる。
// 既存のimport元のために dynamicSymbol.ts が再エクスポートしている。

/** 端子の印(貫通端子を表す丸)の半径 mm(Rust側 TERMINAL_MARK_R と一致)。 */
export const TERMINAL_MARK_R = 1.8;

/**
 * 端子番号の文字高さ mm(Rust側 TERMINAL_NUMBER_HEIGHT と一致)。
 * 文字の基準点は端子の印の中心そのもの。描画側がベースラインを基準点から
 * 文字高の半分だけ画面の下へ取るので、端子台を回しても番号は印の中に留まる。
 */
export const TERMINAL_NUMBER_HEIGHT = 2.0;

/**
 * 端子番号の表記から(極番号, 段記号)を取り出す(Rust側 parse_terminal_no と一致)。
 * 単段は数字だけ("8" → [8, null])、多段端子台 (G6) は末尾に段記号が付く("8a" → [8,"a"])。
 * 読み取れない番号は極番号を Number.MAX_SAFE_INTEGER として末尾に回す。
 */
export function parseTerminalNo(no: string): [number, string | null] {
  const m = /^([0-9]+)([a-zA-Z])$/.exec(no);
  if (m) return [Number(m[1]), m[2]];
  return [/^[0-9]+$/.test(no) ? Number(no) : Number.MAX_SAFE_INTEGER, null];
}

/** 多段端子台 (`terminal_block_{n}p_{levels}l`、G6) の最大段数(Rust側 TERMINAL_LEVEL_MAX と一致)。 */
export const TERMINAL_LEVEL_MAX = 3;

/** 段番号(0始まり)を段記号('a','b','c',…)にする(Rust側 level_letter と一致)。[`parseTerminalNo`]の逆。 */
export function levelLetter(levelIndex: number): string {
  return String.fromCharCode(97 + levelIndex);
}
