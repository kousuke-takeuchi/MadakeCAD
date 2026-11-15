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

import { CAP_HEIGHT_RATIO, activeUnitStroke, strokeWidth, type UnitStroke } from "./strokeFont";

const hidden = new WeakMap<object, boolean>();
/** 直前の `setDrawingFont` の大きさ (図面 mm と拡大率) と太字かどうか (ストロークフォントで描くときに使う)。 */
const sized = new WeakMap<object, { mm: number; scale: number; bold: boolean }>();

/**
 * 線を太字 (×1.5) にする指定か。SVG と同じく `bold` の指定 (表題欄の太字セル・太字の注記) だけ。
 * 吹き出しや表の見出しの `600` は画面の文字だけの太さで、SVG では太字にならないので線も太くしない。
 */
function isBold(prefix: string): boolean {
  return /\bbold\b/.test(prefix);
}

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
  sized.set(ctx, { mm, scale, bold: isBold(prefix) });
  if (px > 0) ctx.font = `${prefix}${px}px ${family}`;
  return px;
}

/** 大きさ1の字の縦の位置をずらす量 (`textBaseline` を線の字に当てはめる。既定はベースライン)。 */
function baselineShift(baseline: CanvasTextBaseline | undefined, t: UnitStroke, cap: number): number {
  switch (baseline) {
    case "middle":
      return cap / 2;
    case "top":
    case "hanging":
      return cap;
    case "bottom":
    case "ideographic":
      return -t.descent;
    default:
      return 0;
  }
}

/**
 * 図面の文字を描く (直前の `setDrawingFont` が小さすぎると判断していれば描かない)。
 * プロジェクトの書体 (ストロークフォント) が有効で字が全てあれば、文字の色の丸い線で描く。
 * 揃え (`textAlign`) と縦の基準 (`textBaseline`) は文字と同じに効き、描画先の線の設定は描く前のまま残す。
 * `spacing` はストロークフォントのときだけ効く字間。
 */
export function drawingText(ctx: CanvasRenderingContext2D, text: string, x: number, y: number, spacing = 1): void {
  if (hidden.get(ctx)) return;
  const size = sized.get(ctx);
  const t = size ? activeUnitStroke(text, spacing) : null;
  if (!t || !size) {
    ctx.fillText(text, x, y);
    return;
  }
  if (t.lines.length === 0) return;
  const px = size.mm * size.scale;
  const align = ctx.textAlign;
  const dx = align === "center" ? -t.width / 2 : align === "right" || align === "end" ? -t.width : 0;
  const dy = baselineShift(ctx.textBaseline, t, CAP_HEIGHT_RATIO);
  ctx.save();
  try {
    ctx.translate(x, y);
    ctx.scale(px, px);
    ctx.translate(dx, dy);
    ctx.strokeStyle = ctx.fillStyle;
    // 太さは図面の mm で決めて (SVG と同じ) 画面へ拡大し、他の線と同じく 1px 未満にはしない
    ctx.lineWidth = Math.max(1, strokeWidth(size.mm, size.bold) * size.scale) / px;
    ctx.lineCap = "round";
    ctx.lineJoin = "round";
    if (t.path) {
      ctx.stroke(t.path);
      return;
    }
    ctx.beginPath();
    for (const l of t.lines) {
      for (let i = 0; i + 1 < l.length; i += 2) {
        if (i === 0) ctx.moveTo(l[i], l[i + 1]);
        else ctx.lineTo(l[i], l[i + 1]);
      }
    }
    ctx.stroke();
  } finally {
    ctx.restore();
  }
}
