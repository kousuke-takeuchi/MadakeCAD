// 端子台のジャンパ (G5) と行の並べ替え順。attrs["jumpers"] / attrs["row_order"] の解析と、
// シンボル上に描くバーの図形を求める。
// Rust側 madake-core/src/terminal_chart.rs の parse_jumpers / jumper_bars と同一ルール。
// Pinia非依存の純関数だけを置き、端子台エディタのストア (stores/terminals.ts) も
// キャンバス描画 (renderer.ts) も**ここ1か所**の正規化ルールを使う。

import { parseTerminalNo, TERMINAL_MARK_R } from "./dynamicSymbol";
import type { Point, RowOrder, SymbolDef } from "../ipc";

/**
 * 端子台チャートの並べ替え順の一覧 (`terminals.sort.*`のドロップダウンが列挙する)。
 * Rust側 `terminal_chart::RowOrder` の5つのバリアントと同じ並び (parityテストで確認)。
 */
export const ROW_ORDERS: readonly RowOrder[] = ["number", "alnum", "page", "cable", "jumper"];

/**
 * `attrs["row_order"]`を解析する。未設定・未知語は`"number"`に落ちる
 * (Rust側 `terminal_chart::row_order_of` と同一ルール)。実際の並べ替えは常にRust側
 * (`sort_terminal_rows`) が行い、ドロップダウンの選択状態もRustが返したチャートの
 * `row_order`が正なので、ここが使われるのは**未検証の文字列を`RowOrder`にする1点だけ**
 * (並べ替えドロップダウンが返す生の`value`を`update_entity`へ渡す前に通す)。
 */
export function rowOrderOf(attrs: Readonly<Record<string, string>> | undefined): RowOrder {
  const raw = attrs?.row_order;
  return raw && (ROW_ORDERS as readonly string[]).includes(raw) ? (raw as RowOrder) : "number";
}

/**
 * ジャンパバーの線幅 (mm)。Rust側 terminal_chart.rs の JUMPER_BAR_STROKE と一致させること。
 * シンボルの線 (0.3mm) の2倍以上にして、印刷してもショートバーだと分かる太さにする。
 * 端子の印どうしの隙間は1.4mmしかないので、両端の丸は描かずバーだけを描く。
 */
export const JUMPER_BAR_STROKE_MM = 0.8;
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

/** `attrs["terminal_numbers"]` のキー名 (spec §1.1、Rust側 TERMINAL_NUMBERS_ATTR と同一)。 */
export const TERMINAL_NUMBERS_ATTR = "terminal_numbers";

/**
 * `numbers` が `count` 個の端子番号の指定として妥当か: 個数が一致し、空要素も重複も無い
 * (Rust側 valid_terminal_numbers と同一)。
 */
export function validTerminalNumbers(numbers: readonly string[], count: number): boolean {
  if (numbers.length !== count || numbers.some((s) => s.trim() === "")) return false;
  return new Set(numbers).size === numbers.length;
}

/**
 * シンボルインスタンスの端子番号を、チャート順の表示ラベルとして列挙する
 * (Rust側 terminal_numbers_of_instance と同一)。`attrs["terminal_numbers"]`があればそれを
 * 使い、[`validTerminalNumbers`]を満たさない無効な値はシンボル定義の既定へフォールバックする。
 */
export function terminalNumbersOfInstance(
  def: SymbolDef,
  attrs: Readonly<Record<string, string>> | undefined,
): string[] {
  const raw = terminalNumbers(def);
  const spec = attrs?.[TERMINAL_NUMBERS_ATTR];
  if (spec === undefined) return raw;
  const numbers = spec.split(",").map((s) => s.trim());
  return validTerminalNumbers(numbers, raw.length) ? numbers : raw;
}

/**
 * 生の端子番号 ([`terminalNumbers`]) → 表示ラベル ([`terminalNumbersOfInstance`]) の対応
 * (Rust側 terminal_number_rename と同一)。上書きが無ければ`null`。キャンバス描画
 * (renderer.ts) が、シンボル定義の生のピン番号テキストをこのインスタンスの表示ラベルへ
 * 差し替えるのに使う (SVG出力 svg.rs::render_symbol の双子)。
 */
export function terminalNumberRename(
  def: SymbolDef,
  attrs: Readonly<Record<string, string>> | undefined,
): Map<string, string> | null {
  const raw = terminalNumbers(def);
  const display = terminalNumbersOfInstance(def, attrs);
  if (raw.length === display.length && raw.every((r, i) => r === display[i])) return null;
  const map = new Map<string, string>();
  raw.forEach((r, i) => map.set(r, display[i]));
  return map;
}

/**
 * 開始番号・増分・接頭辞から採番案 ([`terminalNumbers`]と同じ並び) を作る
 * (Rust側 terminal_renumber_plan と同一)。多段端子台は段記号を保ったまま極だけ採番する
 * (`11a`,`11b`,… — 同じ極の全段が同じ番号を共有する)。ツールバーの採番ポップオーバーの
 * プレビュー表示に使う (実際の適用は`RenumberTerminals`コマンドが行う)。
 */
export function terminalRenumberPlan(
  rawTerminals: readonly string[],
  start: number,
  step: number,
  prefix: string,
): string[] {
  const out: string[] = [];
  let lastPole: number | null = null;
  let poleIndex = 0;
  let n = start;
  for (const t of rawTerminals) {
    const [pole, level] = parseTerminalNo(t);
    if (lastPole !== pole) {
      if (lastPole !== null) {
        poleIndex += 1;
        n = start + step * poleIndex;
      }
      lastPole = pole;
    }
    out.push(level !== null ? `${prefix}${n}${level}` : `${prefix}${n}`);
  }
  return out;
}

/**
 * ジャンパ指定 (`"1-2,3-4"`、多段端子台なら `"1a-1b"`) を解析して正規化する。
 *
 * `terminals`はその端子台の**表示ラベル**(端子番号の上書きがあればそれ、無ければ生の番号と
 * 同じ)をチャート順に並べたもの。`spec`はこのラベルで書かれている。`rawTerminals`は同じ並び
 * の**生の端子番号**([`terminalNumbers`])で、隣接判定・多段判定はここから取る(上書きの
 * 表記がどうであれ、物理的な隣接関係は生の番号の並びだけで決まるため。Rust側 parse_jumpers
 * と同一)。省略時は`terminals`をそのまま使う(上書きの無い既存呼び出しと後方互換)。
 */
export function parseJumperSpec(
  spec: string,
  terminals?: readonly string[],
  rawTerminals: readonly string[] = terminals ?? [],
): [string, string][] {
  const multilevel = rawTerminals.some((t) => parseTerminalNo(t)[1] !== null);
  const pairs: [string, string][] = [];
  for (const item of spec.split(",")) {
    const parts = item.split("-").map((p) => p.trim());
    if (parts.length !== 2 || parts.some((p) => p === "")) continue;
    if (multilevel && parts.every((p) => /^\d+$/.test(p))) continue;
    if (terminals && parts.some((p) => !terminals.includes(p))) continue;
    const i0 = terminals ? terminals.indexOf(parts[0]) : -1;
    const i1 = terminals ? terminals.indexOf(parts[1]) : -1;
    if (terminals && (i0 === -1 || i1 === -1)) continue;
    const rawA = terminals ? rawTerminals[i0] : parts[0];
    const rawB = terminals ? rawTerminals[i1] : parts[1];
    if (!terminalsAreAdjacent(rawA, rawB)) continue;
    const inOrder = terminals ? i0 <= i1 : compareKeys(sortKey(parts[0]), sortKey(parts[1])) <= 0;
    const [a, b] = inOrder ? [parts[0], parts[1]] : [parts[1], parts[0]];
    if (!pairs.some(([x, y]) => x === a && y === b)) pairs.push([a, b]);
  }
  const rank = (t: string) => (terminals ? terminals.indexOf(t) : -1);
  pairs.sort((p, q) => {
    const byFirst = terminals ? rank(p[0]) - rank(q[0]) : compareKeys(sortKey(p[0]), sortKey(q[0]));
    if (byFirst !== 0) return byFirst;
    return terminals ? rank(p[1]) - rank(q[1]) : compareKeys(sortKey(p[1]), sortKey(q[1]));
  });
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

/**
 * ジャンパ (正規化済みの端子番号の組) を、シンボル定義のローカル座標系の線分に変換する
 * (Rust側 jumper_bars と同一)。見つからない端子番号の組は結果から省く。
 *
 * 形はサドルジャンパの実物どおり、隣り合う2端子の中心を結ぶ軸の上、2つの印の隙間だけに
 * 描く。バーは印の縁から縁まで引き、端がどの端子のものかは印の縁が示すので、
 * 両端に別の印は描かない。
 */
export function jumperBars(
  def: SymbolDef,
  pairs: readonly (readonly [string, string])[],
): [Point, Point][] {
  const out: [Point, Point][] = [];
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
    out.push([along(pa, TERMINAL_MARK_R, 1), along(pb, TERMINAL_MARK_R, -1)]);
  }
  return out;
}
