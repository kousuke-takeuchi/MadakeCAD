// ブロックのピンのコネクタの絵 (Rust側 madake-core/src/connector.rs と対。同じ数・同じ順)。
// 両者が同じ形を返すことは src-tauri/crates/madake-core/tests/golden/connector-blocks.json で突き合わせる
// (dynamicSymbol.test.ts)。型だけを dynamicSymbol.ts から読むので、実行時の循環importは無い。

import type { Point, Primitive } from "../ipc";
import type { BlockPin, BlockPins } from "./dynamicSymbol";

/** 絵の本体が外形の内側に入る深さ mm。 */
export const CONNECTOR_IN_MM = 1.5;
/** 絵の本体が外形の外側へ出る長さ mm。 */
export const CONNECTOR_OUT_MM = 2.5;
/** 本体の辺に沿う長さの半分 mm (本体は4mm)。 */
export const CONNECTOR_HALF_SPAN_MM = 2.0;
/** コネクタのあるピンの、外形から接続点までの長さ mm。 */
export const CONNECTOR_LEAD_MM = 5.0;
/** 本体の外端からピン番号までのすき間 mm。 */
export const CONNECTOR_NUM_GAP_MM = 0.25;
/** 上下辺のピン番号をリードから右へずらす量 mm。 */
export const CONNECTOR_NUM_SIDE_MM = 0.5;
/** 1本だけのまとまりの名前をピン名の後ろに空ける量 mm。 */
export const CONNECTOR_LABEL_GAP_MM = 1.0;
/** 上下辺のまとまりの名前の中心の、外形からの深さ mm。 */
export const CONNECTOR_LABEL_TOP_MM = 7.0;
/** まとまりの名前を書く上下辺が外形の内側で使う高さ mm。 */
export const CONNECTOR_LABEL_INSET_MM = 10.0;

/** コネクタの形 (仕様の表の順。エディタの選択肢もこの順)。`_f` = メス、`_m` = オス。 */
export const CONNECTOR_KINDS = [
  "usb_a_f",
  "usb_a_m",
  "usb_c_f",
  "usb_c_m",
  "micro_b_f",
  "micro_b_m",
  "jack35_f",
  "jack35_m",
  "dc_f",
  "dc_m",
  "screw",
  "jst_ph_f",
  "jst_xh_f",
] as const;

export type ConnectorKind = (typeof CONNECTOR_KINDS)[number];

/** 知っているコネクタの名前か。 */
export function isConnectorKind(value: unknown): value is ConnectorKind {
  return typeof value === "string" && (CONNECTOR_KINDS as readonly string[]).includes(value);
}

/** 並んだ数本のピンで1つの本体を描く種類 (端子台・JST)。`connector_ref` はこれだけが持てる。 */
export function isGroupedConnector(kind: ConnectorKind): boolean {
  return kind === "screw" || kind === "jst_ph_f" || kind === "jst_xh_f";
}

/** JST のシリーズ名 (`PH` / `XH`)。他はnull。 */
export function connectorSeries(kind: ConnectorKind): string | null {
  if (kind === "jst_ph_f") return "PH";
  if (kind === "jst_xh_f") return "XH";
  return null;
}

/** 辺のピン i 番目 (全 n 個) の中央揃えオフセット mm (Rust側 block::offset。他の動的シンボルも同じ規則)。 */
export function sideOffset(i: number, n: number): number {
  return (i - (n - 1) / 2) * 5.0;
}


/** 1辺の中の、1つの絵で描くピンのまとまり (`start`〜`end` の手前までが辺のピン表の添字)。 */
export interface ConnectorGroup {
  kind: ConnectorKind;
  /** 前後の空白を落とした connector_ref。無ければnull。 */
  ref: string | null;
  start: number;
  end: number;
}

/** 1辺のピンをまとまりに分ける (Rust側 connector_groups と同じ規則)。 */
export function connectorGroups(side: BlockPin[]): ConnectorGroup[] {
  const groups: ConnectorGroup[] = [];
  side.forEach((pin, i) => {
    const kind = pin.connector;
    if (kind == null) return;
    const ref = pin.connector_ref == null ? null : pin.connector_ref.trim();
    const last = groups[groups.length - 1];
    if (isGroupedConnector(kind) && last && last.end === i && last.kind === kind && last.ref === ref) {
      last.end = i + 1;
      return;
    }
    groups.push({ kind, ref, start: i, end: i + 1 });
  });
  return groups;
}

/** まとまりの名前 (JST は `<ref> PH` / `PH`、端子台は `<ref>`、無ければnull)。 */
export function groupLabel(group: ConnectorGroup): string | null {
  const series = connectorSeries(group.kind);
  if (series !== null) return group.ref !== null ? `${group.ref} ${series}` : series;
  if (group.ref !== null && isGroupedConnector(group.kind)) return group.ref;
  return null;
}

type Dir = "left" | "right" | "up" | "down";

interface Frame {
  e: Point;
  u: Point;
  v: Point;
  theta: number;
}

/** 辺ごとのローカル座標 (右辺の座標を時計回りに回したもの)。 */
function frame(dir: Dir, e: Point): Frame {
  switch (dir) {
    case "right":
      return { e, u: { x: 1, y: 0 }, v: { x: 0, y: 1 }, theta: 0 };
    case "down":
      return { e, u: { x: 0, y: 1 }, v: { x: -1, y: 0 }, theta: 90 };
    case "left":
      return { e, u: { x: -1, y: 0 }, v: { x: 0, y: -1 }, theta: 180 };
    case "up":
      return { e, u: { x: 0, y: -1 }, v: { x: 1, y: 0 }, theta: 270 };
  }
}

function at(f: Frame, u: number, v: number): Point {
  return { x: f.e.x + u * f.u.x + v * f.v.x, y: f.e.y + u * f.u.y + v * f.v.y };
}

function line(f: Frame, pts: [number, number][]): Primitive {
  return { type: "line", pts: pts.map(([u, v]) => at(f, u, v)) };
}

function rect(f: Frame, [u1, v1]: [number, number], [u2, v2]: [number, number], filled: boolean): Primitive {
  const a = at(f, u1, v1);
  const b = at(f, u2, v2);
  return {
    type: "rect",
    p1: { x: Math.min(a.x, b.x), y: Math.min(a.y, b.y) },
    p2: { x: Math.max(a.x, b.x), y: Math.max(a.y, b.y) },
    filled,
  };
}

function circle(f: Frame, [u, v]: [number, number], r: number, filled: boolean): Primitive {
  return { type: "circle", center: at(f, u, v), r, filled };
}

/** 角度は辺の回転ぶん足す (Rust側 netlist::rotate_arc_angles のミラーなしと同じ: 開始は0〜360、振れ幅はそのまま)。 */
function arc(f: Frame, [u, v]: [number, number], r: number, startDeg: number, endDeg: number): Primitive {
  let start = (startDeg + f.theta) % 360;
  if (start < 0) start += 360;
  return { type: "arc", center: at(f, u, v), r, start_deg: start, end_deg: start + (endDeg - startDeg) };
}

/** 1つのまとまり (または1本のピン) の絵 (Rust側 connector_primitives と同じ順)。 */
export function connectorPrimitives(kind: ConnectorKind, dir: Dir, edges: Point[]): Primitive[] {
  const f = frame(dir, edges[0]);
  const vs = edges.map((p) => (p.x - f.e.x) * f.v.x + (p.y - f.e.y) * f.v.y);
  const v0 = Math.min(...vs);
  const v1 = Math.max(...vs);
  const i = -CONNECTOR_IN_MM;
  const o = CONNECTOR_OUT_MM;
  const s = CONNECTOR_HALF_SPAN_MM;
  const female = () => rect(f, [i, -s], [o, s], false);
  const housing = () => rect(f, [i, -s], [0, s], false);
  switch (kind) {
    case "usb_a_f":
      return [female(), rect(f, [-0.8, -0.9], [1.8, -0.1], true)];
    case "usb_a_m":
      return [housing(), rect(f, [0, -1.2], [o, 1.2], true)];
    case "usb_c_f":
      return [
        female(),
        line(f, [
          [-0.2, -0.8],
          [1.2, -0.8],
        ]),
        arc(f, [1.2, 0], 0.8, -90, 90),
        line(f, [
          [1.2, 0.8],
          [-0.2, 0.8],
        ]),
        arc(f, [-0.2, 0], 0.8, 90, 270),
        line(f, [
          [0, 0],
          [1.0, 0],
        ]),
      ];
    case "usb_c_m":
      return [housing(), rect(f, [0, -0.8], [1.7, 0.8], true), circle(f, [1.7, 0], 0.8, true)];
    case "micro_b_f":
      return [
        line(f, [
          [i, -s],
          [0, -s],
          [o, -1.4],
          [o, 1.4],
          [0, s],
          [i, s],
          [i, -s],
        ]),
        line(f, [
          [-0.6, 0],
          [1.6, 0],
        ]),
      ];
    case "micro_b_m":
      return [housing(), rect(f, [0, -0.6], [o, 0.6], true)];
    case "jack35_f":
      return [female(), circle(f, [0.5, 0], 1.0, false), circle(f, [0.5, 0], 0.35, true)];
    case "jack35_m":
      return [housing(), rect(f, [0, -1.0], [0.8, 1.0], true), rect(f, [0.8, -0.4], [o, 0.4], true)];
    case "dc_f":
      return [female(), circle(f, [0.5, 0], 1.5, false), circle(f, [0.5, 0], 0.6, true)];
    case "dc_m":
      return [housing(), rect(f, [0, -1.4], [o, -0.5], true), rect(f, [0, 0.5], [o, 1.4], true)];
    case "screw": {
      const out = [rect(f, [i, v0 - s], [o, v1 + s], false)];
      for (const v of vs) {
        out.push(circle(f, [0.5, v], 1.2, false));
        out.push(
          line(f, [
            [1.3, v - 0.8],
            [-0.3, v + 0.8],
          ]),
        );
      }
      return out;
    }
    case "jst_ph_f":
    case "jst_xh_f": {
      const out = [
        rect(f, [i, v0 - s], [o, v1 + s], false),
        line(f, [
          [1.5, v0 - s],
          [1.5, v1 + s],
        ]),
      ];
      for (const v of vs) out.push(rect(f, [-0.6, v - 0.6], [0.6, v + 0.6], false));
      return out;
    }
  }
}

/** 1辺 (`-half`〜`half`) を、すき間 `gaps` (小さい順) を除いた線分にする。 */
function edgeSegments(out: Primitive[], half: number, gaps: [number, number][], pointAt: (t: number) => Point) {
  let cursor = -half;
  for (const [lo, hi] of gaps) {
    if (lo > cursor) out.push({ type: "line", pts: [pointAt(cursor), pointAt(lo)] });
    cursor = Math.max(cursor, hi);
  }
  if (half > cursor) out.push({ type: "line", pts: [pointAt(cursor), pointAt(half)] });
}

/** コネクタのあるブロックの外形: 絵が辺と交わる所を空けた線分 (Rust側 outline_segments と同じ順)。 */
export function outlineSegments(pins: BlockPins, hw: number, hh: number): Primitive[] {
  const gaps = (side: BlockPin[] = []): [number, number][] =>
    connectorGroups(side).map((g) => [
      sideOffset(g.start, side.length) - CONNECTOR_HALF_SPAN_MM,
      sideOffset(g.end - 1, side.length) + CONNECTOR_HALF_SPAN_MM,
    ]);
  const out: Primitive[] = [];
  edgeSegments(out, hw, gaps(pins.top), (x) => ({ x, y: -hh }));
  edgeSegments(out, hh, gaps(pins.right), (y) => ({ x: hw, y }));
  edgeSegments(out, hw, gaps(pins.bottom), (x) => ({ x, y: hh }));
  edgeSegments(out, hh, gaps(pins.left), (y) => ({ x: -hw, y }));
  return out;
}
