// 端子台のジャンパ (G5)。attrs["jumpers"] の解析と、シンボル上に描くバーの図形を求める。
// Rust側 madake-core/src/terminal_chart.rs の parse_jumpers / jumper_bars と同一ルール。
// Pinia非依存の純関数だけを置き、端子台エディタのストア (stores/terminals.ts) も
// キャンバス描画 (renderer.ts) も**ここ1か所**の正規化ルールを使う。

import { parseTerminalNo, TERMINAL_MARK_R } from "./dynamicSymbol";
import type { Point, SymbolDef } from "../ipc";

/** ジャンパバーの線幅 (mm)。Rust側 terminal_chart.rs の JUMPER_BAR_STROKE と一致させること。 */
export const JUMPER_BAR_STROKE_MM = 0.4;
/** ジャンパバーの両端に付ける丸の半径 (mm)。Rust側 JUMPER_BAR_DOT_R と一致させること。 */
export const JUMPER_BAR_DOT_R_MM = 0.3;
/** 配線が接続点に載っているとみなす距離 (mm)。Rust側 netlist::CONNECT_EPS と同じ。 */
const CONNECT_EPS = 0.01;

/** 端子番号の並べ替えキー (極番号→段記号→表記)。Rust側 terminal_sort_key と同一。 */
function sortKey(no: string): [number, string, string] {
  const [pole, level] = parseTerminalNo(no);
  return [pole, level ?? "", no];
}

function compareKeys(a: [number, string, string], b: [number, string, string]): number {
  return a[0] - b[0] || a[1].localeCompare(b[1]) || a[2].localeCompare(b[2]);
}

/**
 * 2つの端子が隣り合っているか (ジャンパを掛けられる並び)。
 * 同じ段で極番号が1違い(縦の隣)か、同じ極で段記号が1違い(横の隣=段をまたぐ)の組だけ。
 * 極も段も違う斜めの組("1a-2b")は隣り合っていない(Rust側 terminals_are_adjacent と同一)。
 */
export function terminalsAreAdjacent(a: string, b: string): boolean {
  const [poleA, levelA] = parseTerminalNo(a);
  const [poleB, levelB] = parseTerminalNo(b);
  if (levelA === levelB) return Math.abs(poleA - poleB) === 1;
  if (poleA === poleB && levelA !== null && levelB !== null) {
    return Math.abs(levelA.charCodeAt(0) - levelB.charCodeAt(0)) === 1;
  }
  return false;
}

/** 端子台シンボルが持つ端子番号を、チャートに並ぶ順に列挙する(Rust側 terminal_numbers_of と同一)。 */
export function terminalNumbers(def: SymbolDef): string[] {
  const keys = def.pins.map((p) => sortKey(p.number));
  keys.sort(compareKeys);
  const out: string[] = [];
  for (const k of keys) if (out[out.length - 1] !== k[2]) out.push(k[2]);
  return out;
}

/**
 * ジャンパ指定 (`"1-2,3-4"`、多段端子台なら `"1a-1b"`) を解析して正規化する。
 * 並び順で先に来る端子を前にし、重複を除き、昇順に並べる。隣り合わない端子どうしの
 * 組は捨てる。`terminals` (その端子台が実際に持つ端子番号) を渡すと、そこに無い端子を
 * 指す組と、多段端子台に段記号なしで書かれた組も捨てる(Rust側 parse_jumpers と同一)。
 */
export function parseJumperSpec(spec: string, terminals?: readonly string[]): [string, string][] {
  const multilevel = (terminals ?? []).some((t) => parseTerminalNo(t)[1] !== null);
  const pairs: [string, string][] = [];
  for (const item of spec.split(",")) {
    const parts = item.split("-").map((p) => p.trim());
    if (parts.length !== 2 || parts.some((p) => p === "")) continue;
    if (multilevel && parts.every((p) => /^\d+$/.test(p))) continue;
    if (terminals && parts.some((p) => !terminals.includes(p))) continue;
    if (!terminalsAreAdjacent(parts[0], parts[1])) continue;
    const [a, b] =
      compareKeys(sortKey(parts[0]), sortKey(parts[1])) <= 0
        ? [parts[0], parts[1]]
        : [parts[1], parts[0]];
    if (!pairs.some(([x, y]) => x === a && y === b)) pairs.push([a, b]);
  }
  pairs.sort((p, q) => compareKeys(sortKey(p[0]), sortKey(q[0])) || compareKeys(sortKey(p[1]), sortKey(q[1])));
  return pairs;
}

/** ジャンパ指定の文字列表現 (`attrs["jumpers"]` に入れる形)。 */
export function formatJumperSpec(pairs: readonly (readonly [string, string])[]): string {
  return pairs.map(([a, b]) => `${a}-${b}`).join(",");
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

/** 図面に描くジャンパ1本ぶんの図形(シンボルのローカル座標。Rust側 JumperBar と同一)。 */
export interface JumperBar {
  /** バーの両端。2つの端子の印の縁どうしを結ぶ。 */
  bar: [Point, Point];
  /** 両端の丸の中心。それぞれ自分の端子の印に外接する。 */
  dots: [Point, Point];
}

/**
 * ジャンパ (正規化済みの端子番号の組) を、シンボル定義のローカル座標系の図形に変換する
 * (Rust側 jumper_bars と同一)。見つからない端子番号の組は結果から省く。
 *
 * 形はサドルジャンパの実物どおり、隣り合う2端子の中心を結ぶ軸の上、2つの印の隙間だけに
 * 描く。バーは印の縁から縁まで、丸は自分の端子の印に外接する位置
 * (中心から TERMINAL_MARK_R + JUMPER_BAR_DOT_R_MM) に置く。
 */
export function jumperBars(
  def: SymbolDef,
  pairs: readonly (readonly [string, string])[],
): JumperBar[] {
  const out: JumperBar[] = [];
  for (const [a, b] of pairs) {
    const pa = terminalCenter(def, a);
    const pb = terminalCenter(def, b);
    if (!pa || !pb) continue;
    const [dx, dy] = [pb.x - pa.x, pb.y - pa.y];
    const len = Math.hypot(dx, dy);
    if (len < CONNECT_EPS) continue;
    const [ux, uy] = [dx / len, dy / len];
    const along = (from: Point, d: number, sign: number): Point => ({
      x: from.x + ux * d * sign,
      y: from.y + uy * d * sign,
    });
    const dotD = TERMINAL_MARK_R + JUMPER_BAR_DOT_R_MM;
    out.push({
      bar: [along(pa, TERMINAL_MARK_R, 1), along(pb, TERMINAL_MARK_R, -1)],
      dots: [along(pa, dotD, 1), along(pb, dotD, -1)],
    });
  }
  return out;
}
