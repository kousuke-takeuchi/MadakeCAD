// 配線注記 (G2) の文言と配置計算。Rust側 madake-core/src/wire_note.rs と同一ルール:
// ワイヤ1本ごとに最長線分の中点へ置き、線番と重ならないよう反対側 (横線=下、縦線=右) へ
// 2.5mm (グリッドピッチ) 逃がす。線番がネット単位なのに対し、注記は電線長が1本ごとに
// 違うのでワイヤ単位で描く。

import type { Point, Sheet, Wire } from "../ipc";
import { annotationScale } from "./annotationText";

/** 配線注記の文字高さ (mm)。wire_note.rsのWIRE_NOTE_FONTと一致させること (既定値)。 */
export const WIRE_NOTE_FONT = 2.0;
/** 配線注記と配線の間隔 (mm)。wire_note.rsのWIRE_NOTE_GAPと一致させること (既定値)。 */
export const WIRE_NOTE_GAP = 2.5;

/** このシートで配線注記を描く文字高さ (mm) = シートの注記文字高そのもの (G20)。 */
export function wireNoteFont(sheet: Sheet | null | undefined): number {
  return WIRE_NOTE_FONT * annotationScale(sheet);
}

/** 線色名 → 図面に書く日本語の色名 (wire_note.rsのcolor_jaと同じ表)。 */
const COLOR_JA: Record<string, string> = {
  black: "黒",
  white: "白",
  red: "赤",
  blue: "青",
  yellow: "黄",
  green: "緑",
  light_blue: "水色",
  sky: "水色",
  orange: "橙",
  brown: "茶",
  gray: "灰",
  grey: "灰",
  purple: "紫",
  violet: "紫",
  pink: "桃",
};

/** 線色名の日本語表記。未知の色名はそのまま返す。 */
export function colorJa(color: string): string {
  return COLOR_JA[color] ?? color;
}

/** 図面用の数値整形。小数の末尾の0を落とす (0.75 → "0.75"、1.0 → "1")。 */
function num(v: number): string {
  return v.toFixed(3).replace(/\.?0+$/, "");
}

/** 描画する配線注記1件。alignはテキストの揃え (横線=中央、縦線=左)。 */
export interface WireNoteLabel {
  wireId: string;
  text: string;
  /** 用紙座標 (mm) のテキスト基準点 (ベースライン)。 */
  at: Point;
  align: "center" | "left";
}

/**
 * ワイヤ属性から注記テキストを作る。書式は `{sq}sq {色} {長さ}m`。
 * 欠けている属性はその場ごと省き、全て欠けていれば空文字 (注記を描かない)。
 */
export function wireNoteText(wire: Wire): string {
  const parts: string[] = [];
  if (wire.sq > 0) parts.push(`${num(wire.sq)}sq`);
  const color = (wire.color ?? "").trim();
  if (color) parts.push(colorJa(color));
  if (wire.length_m != null && wire.length_m > 0) parts.push(`${num(wire.length_m)}m`);
  return parts.join(" ");
}

/** ワイヤの代表線分 (最も長い線分。同長なら上・左)。Rust側 svg::longest_segment と同じ。 */
function longestSegment(points: Point[]): [Point, Point] | null {
  let best: { len: number; a: Point; b: Point } | null = null;
  for (let i = 0; i + 1 < points.length; i++) {
    const a = points[i];
    const b = points[i + 1];
    const len = Math.hypot(b.x - a.x, b.y - a.y);
    if (best) {
      const bm = { x: (best.a.x + best.b.x) / 2, y: (best.a.y + best.b.y) / 2 };
      const m = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
      const better =
        len > best.len + 1e-9 ||
        (Math.abs(len - best.len) <= 1e-9 && (m.y < bm.y || (m.y === bm.y && m.x < bm.x)));
      if (!better) continue;
    }
    best = { len, a, b };
  }
  return best ? [best.a, best.b] : null;
}

/** シートの配線注記を、ワイヤid順 (決定的) に返す。属性の無いワイヤは含まない。 */
export function wireNoteLabels(sheet: Sheet): WireNoteLabel[] {
  const out: WireNoteLabel[] = [];
  // 注記文字高を下げたシートでは、配線からの逃がしも同じ比率で詰める
  const gap = WIRE_NOTE_GAP * annotationScale(sheet);
  for (const entity of Object.values(sheet.entities)) {
    if (entity.kind !== "wire") continue;
    const text = wireNoteText(entity);
    if (!text) continue;
    const seg = longestSegment(entity.points);
    if (!seg) continue;
    const [a, b] = seg;
    const mid = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
    // 線番は横線の上・縦線の左。注記はその反対側 (下・右) へ置いて重ねない
    out.push(
      Math.abs(b.x - a.x) >= Math.abs(b.y - a.y)
        ? { wireId: entity.id, text, at: { x: mid.x, y: mid.y + gap }, align: "center" }
        : { wireId: entity.id, text, at: { x: mid.x + gap, y: mid.y }, align: "left" },
    );
  }
  return out.sort((p, q) => (p.wireId < q.wireId ? -1 : p.wireId > q.wireId ? 1 : 0));
}
