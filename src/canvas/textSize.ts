// 図面の文字の画面上の大きさ。
//
// 以前は各描画箇所で `Math.max(8, 高さmm × 拡大率)` px としていたため、ズームアウトしても文字だけが
// 8〜9px より小さくならず、縮小するほど図形に対して文字が大きくなって画面が文字で埋まっていた。
// 文字も図形と同じ割合で縮め、読めない大きさになったら描かない (印刷物を縮小したときと同じ見え方)。

/** これより小さい文字は描かない (画面 px)。読めず、図面を灰色に埋めるだけになるため。 */
export const MIN_TEXT_PX = 4;

/**
 * 図面上の文字の高さ (mm) を、いまの拡大率での画面 px にする。
 * `MIN_TEXT_PX` 未満は 0 を返す。フォントの大きさ 0 で `fillText` しても何も描かれない。
 */
export function textPx(mm: number, scale: number): number {
  const px = mm * scale;
  return px < MIN_TEXT_PX ? 0 : px;
}
