// 多芯ケーブル (M4 §5、.pen「M4残りデザイン④」`SWqFJ`) の作図領域側。
//
// 芯線の割当 (定義線が横切った配線を交差順に) はRust側 (madake-core/src/cable.rs と
// `add_cable` コマンド) が正。ここは「定義線ツールが送るコマンド」「注記の文字列」
// 「シールド楕円の形」「描画」「当たり判定」だけを持ち、注記とシールドの形はSVG出力
// (svg.rs) と同じ規則で作る。

import type { Cable, Command, Point, Sheet } from "../ipc";
import { theme } from "./theme";
import type { Viewport } from "./viewport";
import { drawingText, setDrawingFont } from "./textSize";

/** ケーブルの参照記号の接頭辞 (ハーネスと同じ W 系)。 */
export const CABLE_PREFIX = "W";
/** 定義線とみなす最短の長さ mm (クリックしただけでは作らない)。 */
const MIN_LENGTH = 2.5;
/** 注記の文字高 mm (SVGと同じ)。 */
const CABLE_FONT = 2.5;

/** 数値の表記 (小数第2位まで、末尾の0と小数点は書かない)。 */
function num(v: number): string {
  return v.toFixed(2).replace(/0+$/, "").replace(/\.$/, "");
}

/** 定義線に添える注記 (Rust `cable_label` と同じ: 参照記号・型番・芯数×線径・長さ)。 */
export function cableLabel(c: Cable): string {
  const spec = c.sq > 0 ? `${c.core_count}C×${num(c.sq)}sq` : `${c.core_count}C`;
  return [c.reference, c.part_no.trim(), spec, c.length_m != null ? `${num(c.length_m)}m` : ""]
    .filter((s) => s !== "")
    .join("  ");
}

/** シールドの楕円 (Rust `shield_ellipse` と同じ): 中心・長半径・短半径・向き [rad]。 */
export function shieldEllipse(c: Cable): { center: Point; rx: number; ry: number; angle: number } | null {
  const [a, b] = c.points;
  if (!a || !b) return null;
  return {
    center: { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 },
    rx: Math.hypot(b.x - a.x, b.y - a.y) / 2 + 2.5,
    ry: 3,
    angle: Math.atan2(b.y - a.y, b.x - a.x),
  };
}

/** シート上の次のケーブル参照記号 (ケーブルとハーネスの W 番号の続き)。 */
export function nextCableReference(sheet: Sheet): string {
  let max = 0;
  for (const e of Object.values(sheet.entities)) {
    const name = e.kind === "cable" ? e.reference : e.kind === "harness" ? e.name : "";
    const m = /^W(\d+)$/.exec(name ?? "");
    if (m) max = Math.max(max, Number(m[1]));
  }
  return `${CABLE_PREFIX}${max + 1}`;
}

/**
 * 定義線のドラッグ → ケーブル追加コマンド。芯数0=横切った本数にする (Rust側で決まる)。
 * 短すぎる線 (クリックしただけ) ではnullを返してコマンドを送らない。
 */
export function cableAddCommand(sheet: Sheet, a: Point, b: Point): Command | null {
  if (Math.hypot(b.x - a.x, b.y - a.y) < MIN_LENGTH) return null;
  return {
    type: "add_cable",
    sheet_id: sheet.id,
    cable: {
      id: crypto.randomUUID(),
      reference: nextCableReference(sheet),
      part_no: "",
      core_count: 0,
      sq: 0,
      points: [a, b],
      cores: [],
      shield: null,
      length_m: null,
    },
  };
}

/** 点から線分までの距離。 */
function segmentDistance(p: Point, a: Point, b: Point): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const len2 = dx * dx + dy * dy;
  const t = len2 === 0 ? 0 : Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / len2));
  return Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy));
}

/** 定義線を掴めるか。 */
export function cableHit(c: Cable, world: Point, tolerance: number): boolean {
  const [a, b] = c.points;
  return !!a && !!b && segmentDistance(world, a, b) <= tolerance;
}

/** ケーブルを描く (定義線・シールド楕円・注記)。選択中は選択色。 */
export function drawCable(ctx: CanvasRenderingContext2D, vp: Viewport, c: Cable, selected: boolean): void {
  const [a, b] = c.points;
  if (!a || !b) return;
  const color = selected ? theme.selection : theme.annotation;
  const sa = vp.toScreen(a);
  const sb = vp.toScreen(b);
  ctx.strokeStyle = color;
  ctx.lineWidth = Math.max(1.2, 0.35 * vp.scale) + (selected ? 1 : 0);
  ctx.beginPath();
  ctx.moveTo(sa.x, sa.y);
  ctx.lineTo(sb.x, sb.y);
  ctx.stroke();
  const shield = c.shield ? shieldEllipse(c) : null;
  if (shield) {
    const center = vp.toScreen(shield.center);
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.ellipse(center.x, center.y, shield.rx * vp.scale, shield.ry * vp.scale, shield.angle, 0, Math.PI * 2);
    ctx.stroke();
  }
  const top = a.y <= b.y ? a : b;
  const at = vp.toScreen({ x: top.x + 1.5, y: top.y - 1.5 });
  ctx.fillStyle = color;
  setDrawingFont(ctx, CABLE_FONT, vp.scale, "monospace");
  ctx.textAlign = "left";
  ctx.textBaseline = "alphabetic";
  drawingText(ctx, cableLabel(c), at.x, at.y);
}
