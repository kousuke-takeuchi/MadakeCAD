// 図面の文字の画面上の大きさ。
//
// 以前は各描画箇所で `Math.max(8, 高さmm × 拡大率)` px としていたため、ズームアウトしても文字だけが
// 8〜9px より小さくならず、縮小するほど図形に対して文字が大きくなって画面が文字で埋まっていた。
// 文字も図形と同じ割合で縮め、読めない大きさになったら描かない (印刷物を縮小したときと同じ見え方)。
//
// 「描かない」はフォントを 0px にするのでは実現できない: WebKit (Tauri の画面) は 0px の指定を
// 受け付けず直前のフォントのまま描くため、文字が大きいまま残る。そこで setDrawingFont が
// 「この後の文字は描かない」を覚え、drawingText がそれを見て描くかどうかを決める。

/** これより小さい文字は描かない (画面 px)。読めず、図面を埋めるだけになるため。 */
export const MIN_TEXT_PX = 4;

/** 図面上の文字の高さ (mm) を、いまの拡大率での画面 px にする。`MIN_TEXT_PX` 未満は 0 (描かない)。 */
export function textPx(mm: number, scale: number): number {
  const px = mm * scale;
  return px < MIN_TEXT_PX ? 0 : px;
}

const hidden = new WeakMap<object, boolean>();

/**
 * 図面の文字のフォントを設定する。読めないほど小さいときはフォントを変えず、
 * 続く `drawingText` を描かない状態にする。戻り値は画面 px (描かないときは 0)。
 */
export function setDrawingFont(
  ctx: CanvasRenderingContext2D,
  mm: number,
  scale: number,
  family: string,
  prefix = "",
): number {
  const px = textPx(mm, scale);
  hidden.set(ctx, px === 0);
  if (px > 0) ctx.font = `${prefix}${px}px ${family}`;
  return px;
}

/** 図面の文字を描く (直前の `setDrawingFont` が小さすぎると判断していれば描かない)。 */
export function drawingText(ctx: CanvasRenderingContext2D, text: string, x: number, y: number): void {
  if (hidden.get(ctx)) return;
  ctx.fillText(text, x, y);
}
