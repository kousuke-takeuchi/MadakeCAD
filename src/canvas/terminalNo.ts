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

/**
 * 端子番号の表記を「前置き・数値・後置き」に分ける(Rust側 split_label_number と一致)。
 * `"A3"` → `["A", 3, ""]`、`"11a"` → `["", 11, "a"]`、`"3"` → `["", 3, ""]`。
 * 最初に現れる数字の並びを数値部と見る。数字が1つも無い表記は`null`。
 *
 * [`parseTerminalNo`]とは役割が違う: あちらは**並べ替えのキー**なので読めない表記を
 * `Number.MAX_SAFE_INTEGER`という番兵にして末尾へ回すが、こちらは**番号の計算**に使うので
 * 読めないことをはっきり`null`で返す。
 */
export function splitLabelNumber(label: string): [string, number, string] | null {
  const m = /^([^0-9]*)([0-9]+)(.*)$/.exec(label);
  if (!m) return null;
  const n = Number(m[2]);
  return Number.isSafeInteger(n) ? [m[1], n, m[3]] : null;
}

/**
 * 端子番号の表記の**数値部だけ**を`delta`増やした表記(Rust側 bump_label_number と一致)。
 * `("A3", 1)` → `"A4"`、`("11a", 1)` → `"12a"`。前置き・後置きはそのまま引き継ぐので、
 * 接頭辞付きの採番や手書きの上書き表記を壊さない。数値部が無い表記は`null`。
 */
export function bumpLabelNumber(label: string, delta: number): string | null {
  const parts = splitLabelNumber(label);
  if (!parts) return null;
  const n = parts[1] + delta;
  return Number.isSafeInteger(n) ? `${parts[0]}${n}${parts[2]}` : null;
}

/** 多段端子台 (`terminal_block_{n}p_{levels}l`、G6) の最大段数(Rust側 TERMINAL_LEVEL_MAX と一致)。 */
export const TERMINAL_LEVEL_MAX = 3;

/** 段番号(0始まり)を段記号('a','b','c',…)にする(Rust側 level_letter と一致)。[`parseTerminalNo`]の逆。 */
export function levelLetter(levelIndex: number): string {
  return String.fromCharCode(97 + levelIndex);
}
