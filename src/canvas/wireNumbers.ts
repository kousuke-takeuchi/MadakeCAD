// 線番テキストの配置計算 (IEC 62491)。Rust側 madake-core/src/svg.rs の
// render_wire_numbers と同一ルール: ネットごとに最も長い線分の中点へ1回だけ置き、
// 横向きの線分なら上へ、縦向きなら左へ 2.5mm (グリッドピッチ) 離す。
//
// 線番は Wire::net に入る文字列で、同じネットの全ワイヤが同値を持つ。画面側は
// ネットリストを持たないため、同じ文字列のワイヤを1つの線番として束ねる
// (採番は図面全体で一意なので、別ネットが同じ線番を持つことはない)。

import type { Point, Sheet } from "../ipc";

/** 線番テキストの文字高さ (mm)。svg.rsのWIRE_NO_FONTと一致させること。 */
export const WIRE_NO_FONT = 2.5;
/** 線番テキストと配線の間隔 (mm)。svg.rsのWIRE_NO_GAPと一致させること。 */
export const WIRE_NO_GAP = 2.5;

/** 描画する線番1件。alignはテキストの揃え (横線=中央、縦線=右)。 */
export interface WireNumberLabel {
  number: string;
  /** 用紙座標 (mm) のテキスト基準点。 */
  at: Point;
  align: "center" | "right";
}

/** 線分の長さの2乗 (平方根を取らずに比較するため)。 */
function lengthSq(a: Point, b: Point): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  return dx * dx + dy * dy;
}

/**
 * シートの線番テキストを、線番の昇順 (辞書順) で返す。
 * 線番の無いワイヤ・空白だけの線番は含まない。
 */
export function wireNumberLabels(sheet: Sheet): WireNumberLabel[] {
  /** 線番 → その線番を持つ全ワイヤの中で最も長い線分。 */
  const longest = new Map<string, { a: Point; b: Point; len: number }>();
  for (const entity of Object.values(sheet.entities)) {
    if (entity.kind !== "wire") continue;
    const number = (entity.net ?? "").trim();
    if (!number) continue;
    for (let i = 0; i + 1 < entity.points.length; i++) {
      const a = entity.points[i];
      const b = entity.points[i + 1];
      const len = lengthSq(a, b);
      const best = longest.get(number);
      if (!best || len > best.len) longest.set(number, { a, b, len });
    }
  }
  return [...longest.entries()]
    .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
    .map(([number, { a, b }]) => {
      const mid = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
      // 横に長い線分は上へ、縦に長い線分は左へ逃がす (配線と重ねない)
      return Math.abs(b.x - a.x) >= Math.abs(b.y - a.y)
        ? { number, at: { x: mid.x, y: mid.y - WIRE_NO_GAP }, align: "center" as const }
        : { number, at: { x: mid.x - WIRE_NO_GAP, y: mid.y }, align: "right" as const };
    });
}
