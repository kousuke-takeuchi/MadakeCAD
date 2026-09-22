// 注記の引き出し線 (G20)。Rust側 madake-core/src/leader.rs と同一ジオメトリの双子。
//
// 対象がシンボルのときの外接矩形は dynamicSymbol.ts の symbolBounds 1つだけを使う
// (Rust側 leader.rs も svg::symbol_bounds 1つだけを使う)。図面に描かれる外形と、
// 引き出し線が指す先を必ず一致させるため。
//
// TextEntity.leader_toに対象のエンティティIDが入っていれば、注記の外接矩形のうち
// 対象にいちばん近い辺の中点から、対象の外接矩形上のいちばん近い点まで直線を引く
// (対象端には配線の分岐点と同じ大きさの丸印)。位置は保存せず描画のたびに求め直すので、
// 注記か対象のどちらを動かしても線は追従する。

import type { Entity, Point, Sheet, SymbolDef, TextEntity } from "../ipc";
import { symbolBounds, textWidthMm } from "./dynamicSymbol";
import { harnessBounds } from "./harness";
import { shapeBounds } from "./shapes";

/** 軸に平行な外接矩形 (mm)。Rust側 tidy::Rect と同一。 */
export interface Rect {
  min: Point;
  max: Point;
}

/** 引き出し線の線幅 (mm)。Rust側 svg.rs の LEADER_STROKE と同じ。 */
export const LEADER_STROKE_MM = 0.25;

function rectFromCorners(a: Point, b: Point): Rect {
  return {
    min: { x: Math.min(a.x, b.x), y: Math.min(a.y, b.y) },
    max: { x: Math.max(a.x, b.x), y: Math.max(a.y, b.y) },
  };
}

function extend(rect: Rect, p: Point): Rect {
  return {
    min: { x: Math.min(rect.min.x, p.x), y: Math.min(rect.min.y, p.y) },
    max: { x: Math.max(rect.max.x, p.x), y: Math.max(rect.max.y, p.y) },
  };
}

/** 注記の外接矩形 (用紙座標)。基準位置 (anchor) に応じて左揃え・中央揃え・右揃えで求める。 */
export function textBounds(text: TextEntity): Rect {
  const w = textWidthMm(text.text, text.height);
  const anchor = text.anchor ?? "start";
  const x0 = anchor === "middle" ? text.at.x - w / 2 : anchor === "end" ? text.at.x - w : text.at.x;
  return rectFromCorners({ x: x0, y: text.at.y - text.height }, { x: x0 + w, y: text.at.y });
}

function boundsOfPoints(points: readonly Point[]): Rect | null {
  if (points.length === 0) return null;
  let rect = rectFromCorners(points[0], points[0]);
  for (const p of points.slice(1)) rect = extend(rect, p);
  return rect;
}

/**
 * 任意のエンティティの外接矩形 (用紙座標)。求まらない種類 (盤レイアウト専用など) や
 * 頂点の無い図形はnull (Rust側 entity_bounds と同一)。
 */
export function entityBounds(entity: Entity, defs: Map<string, SymbolDef>): Rect | null {
  switch (entity.kind) {
    case "symbol": {
      const def = defs.get(entity.symbol_id);
      return def ? symbolBounds(entity, def) : null;
    }
    case "text":
      return textBounds(entity);
    case "wire":
      return boundsOfPoints(entity.points);
    case "junction":
      return rectFromCorners(entity.at, entity.at);
    case "net_label":
      return rectFromCorners(entity.at, entity.at);
    case "harness":
      return harnessBounds(entity.points);
    case "shape":
      return shapeBounds(entity);
    default:
      // 盤レイアウト専用エンティティは回路図シートには載らないので対象になり得ない
      return null;
  }
}

/** 矩形上で`p`にいちばん近い点 (`p`が内側ならそのもの)。 */
function nearestPointOnRect(rect: Rect, p: Point): Point {
  return {
    x: Math.min(Math.max(p.x, rect.min.x), rect.max.x),
    y: Math.min(Math.max(p.y, rect.min.y), rect.max.y),
  };
}

function edgeMidpoints(rect: Rect): Point[] {
  const cx = (rect.min.x + rect.max.x) / 2;
  const cy = (rect.min.y + rect.max.y) / 2;
  return [
    { x: cx, y: rect.min.y }, // 上辺
    { x: cx, y: rect.max.y }, // 下辺
    { x: rect.min.x, y: cy }, // 左辺
    { x: rect.max.x, y: cy }, // 右辺
  ];
}

function dist(a: Point, b: Point): number {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

/**
 * 注記の外接矩形から対象の外接矩形への引き出し線 (始点, 終点)。
 * 始点は注記の4辺の中点のうち、対象の外接矩形にいちばん近いもの。終点はそこから対象の
 * 外接矩形上のいちばん近い点 (Rust側 leader_line と同一)。
 */
export function leaderLine(textRect: Rect, targetRect: Rect): [Point, Point] {
  let best: Point = edgeMidpoints(textRect)[0];
  let bestD = Infinity;
  for (const mid of edgeMidpoints(textRect)) {
    const end = nearestPointOnRect(targetRect, mid);
    const d = dist(mid, end);
    if (d < bestD) {
      bestD = d;
      best = mid;
    }
  }
  return [best, nearestPointOnRect(targetRect, best)];
}

/**
 * シート上の注記1つぶんの引き出し線ジオメトリ。leader_toが対象を指していない・対象が
 * 求まらないときはnull (Rust側 leader_for と同一)。
 */
export function leaderFor(
  sheet: Sheet,
  text: TextEntity,
  defs: Map<string, SymbolDef>,
): [Point, Point] | null {
  if (!text.leader_to) return null;
  const target = sheet.entities[text.leader_to];
  if (!target) return null;
  const targetRect = entityBounds(target, defs);
  if (!targetRect) return null;
  return leaderLine(textBounds(text), targetRect);
}
