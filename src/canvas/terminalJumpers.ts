// 端子台のジャンパバー (G5)。attrs["jumpers"]からシンボル上に描くバーの端点を求める。
// Rust側 madake-core/src/terminal_chart.rs の parse_jumpers / jumper_bar_segments と
// 同一ルール(隣り合う端子どうしだけを認め、それ以外は捨てる)。SVG出力(svg.rs)と
// このキャンバス描画は、ここの正規化ルールと terminalCenter の座標計算を共有する。

import type { Point, SymbolDef } from "../ipc";

/** ジャンパバーの線幅 (mm)。svg.rsのJUMPER_BAR_STROKEと一致させること。 */
export const JUMPER_BAR_STROKE_MM = 1.0;
/** ジャンパバーの両端に付ける丸の半径 (mm)。svg.rsのJUMPER_BAR_DOT_Rと一致させること。 */
export const JUMPER_BAR_DOT_R_MM = 1.0;

/**
 * ジャンパ指定 (`"1-2,3-4"`) を解析する。小さい番号が先・昇順・重複なしに正規化し、
 * 隣り合わない端子どうし(例 "1-3")は捨てる(Rust側 parse_jumpers と同じ規則)。
 * 端子台エディタの stores/terminals.ts の parseJumperSpec とも同一の正規化。
 */
export function parseJumperPairs(spec: string): [number, number][] {
  const pairs: [number, number][] = [];
  for (const item of spec.split(",")) {
    const parts = item.split("-").map((p) => p.trim());
    if (parts.length !== 2) continue;
    if (!/^\d+$/.test(parts[0]) || !/^\d+$/.test(parts[1])) continue;
    const a = Math.min(Number(parts[0]), Number(parts[1]));
    const b = Math.max(Number(parts[0]), Number(parts[1]));
    if (b - a !== 1) continue;
    if (!pairs.some(([x, y]) => x === a && y === b)) pairs.push([a, b]);
  }
  pairs.sort((p, q) => p[0] - q[0]);
  return pairs;
}

/**
 * シンボル定義の中で、端子番号(文字列)の中心点を求める(同じ番号のピンの中点)。
 * 貫通端子は左右2つの接続点が同じ番号を持つので、その中点が端子の印(丸)の位置になる
 * (Rust側 terminal_center と同一)。
 */
export function terminalCenter(def: SymbolDef, number: string): Point | null {
  const pts = def.pins.filter((p) => p.number === number).map((p) => p.at);
  if (pts.length === 0) return null;
  const sx = pts.reduce((s, p) => s + p.x, 0);
  const sy = pts.reduce((s, p) => s + p.y, 0);
  return { x: sx / pts.length, y: sy / pts.length };
}

/**
 * ジャンパ (正規化済みの端子番号の組) を、シンボル定義のローカル座標系でのバーの
 * 端点に変換する。見つからない端子番号の組は結果から省く(Rust側 jumper_bar_segments
 * と同一)。
 */
export function jumperBarSegments(
  def: SymbolDef,
  pairs: readonly [number, number][],
): [Point, Point][] {
  const out: [Point, Point][] = [];
  for (const [a, b] of pairs) {
    const pa = terminalCenter(def, String(a));
    const pb = terminalCenter(def, String(b));
    if (pa && pb) out.push([pa, pb]);
  }
  return out;
}
