// ハーネス境界 (IEC 61082-1 のグループ囲み) の幾何計算とコマンド組み立て。
// Rust側 madake-core/src/harness.rs と同一ルール:
//   - 所属判定は幾何学的な内包のみ。配線の全ての点が囲みの内側 (境界線上を含む) にあるときだけ所属
//   - 入れ子の囲みでは最も小さい (内側の) ハーネスを選ぶ
//   - 名前は参照記号と同じ命名規則 (W1, W2 …) で、既存の最大値+1を自動で提案する
// 図面への反映は必ず Command 経由 (CLAUDE.md「アーキテクチャの絶対原則」)。

import type { Command, Entity, Harness, Point, Sheet, Wire } from "../ipc";

/** 境界線上の点を「内側」と扱うための許容誤差 (mm)。svg.rs / harness.rs と同じ考え方。 */
const EPS = 1e-9;
/** ハーネス名の接頭辞 (参照記号と同じ命名規則)。 */
export const HARNESS_PREFIX = "W";
/** 破線の線の長さと間隔 (mm)。svg.rsのHARNESS_DASHと一致させること。 */
export const HARNESS_DASH: [number, number] = [3, 2];
/** ハーネス名の文字高さ (mm)。svg.rsのHARNESS_FONTと一致させること。 */
export const HARNESS_FONT = 2.5;
/** ハーネス名の位置: 囲みの左上角から右へ / 上へ (mm)。svg.rsと一致させること。 */
const HARNESS_LABEL_DX = 1;
const HARNESS_LABEL_DY = 1;
/** これより小さい矩形はハーネスにしない (クリックだけの誤操作を弾く) mm。 */
const MIN_SIZE = 0.5;

/** 囲みの外接矩形 (左上, 右下)。頂点が無ければnull。 */
export function harnessBounds(points: Point[]): { min: Point; max: Point } | null {
  if (points.length === 0) return null;
  const xs = points.map((p) => p.x);
  const ys = points.map((p) => p.y);
  return {
    min: { x: Math.min(...xs), y: Math.min(...ys) },
    max: { x: Math.max(...xs), y: Math.max(...ys) },
  };
}

/** 2点のドラッグから矩形の4頂点 (左上→右上→右下→左下) を作る。 */
export function harnessRectPoints(a: Point, b: Point): Point[] {
  const x0 = Math.min(a.x, b.x);
  const x1 = Math.max(a.x, b.x);
  const y0 = Math.min(a.y, b.y);
  const y1 = Math.max(a.y, b.y);
  return [
    { x: x0, y: y0 },
    { x: x1, y: y0 },
    { x: x1, y: y1 },
    { x: x0, y: y1 },
  ];
}

/** ハーネス名を描く用紙座標 (囲みの左上角の外側)。頂点が無ければnull。 */
export function harnessLabelAt(points: Point[]): Point | null {
  const b = harnessBounds(points);
  if (!b) return null;
  return { x: b.min.x + HARNESS_LABEL_DX, y: b.min.y - HARNESS_LABEL_DY };
}

/** 点が囲みの内側 (境界線上を含む) にあるか。 */
function containsPoint(points: Point[], p: Point): boolean {
  const b = harnessBounds(points);
  if (!b) return false;
  return (
    p.x >= b.min.x - EPS && p.x <= b.max.x + EPS && p.y >= b.min.y - EPS && p.y <= b.max.y + EPS
  );
}

/** 配線が囲みに所属するか。全ての点が内側にあるときだけ所属する。 */
export function harnessContainsWire(harness: Harness, wire: Wire): boolean {
  return wire.points.length > 0 && wire.points.every((p) => containsPoint(harness.points, p));
}

/** シート上の全ハーネス境界。 */
export function harnesses(sheet: Sheet): Harness[] {
  return Object.values(sheet.entities).filter(
    (e): e is Extract<Entity, { kind: "harness" }> => e.kind === "harness",
  );
}

/** 囲みの面積 (mm2)。入れ子の優先順位に使う。 */
function area(harness: Harness): number {
  const b = harnessBounds(harness.points);
  if (!b) return 0;
  return (b.max.x - b.min.x) * (b.max.y - b.min.y);
}

/** 配線が所属するハーネス。入れ子では最も小さい囲みを選ぶ。所属しなければnull。 */
export function harnessOfWire(sheet: Sheet, wire: Wire): Harness | null {
  const hits = harnesses(sheet).filter((h) => harnessContainsWire(h, wire));
  if (hits.length === 0) return null;
  return hits.reduce((best, h) =>
    area(h) < area(best) || (area(h) === area(best) && h.id < best.id) ? h : best,
  );
}

/** ハーネスが囲んでいる配線の本数 (プロパティパネルの「含む電線」)。 */
export function harnessWireCount(sheet: Sheet, harnessId: string): number {
  return Object.values(sheet.entities).filter(
    (e) => e.kind === "wire" && harnessOfWire(sheet, e)?.id === harnessId,
  ).length;
}

/** 次に使うハーネス名 (既存の W番号 の最大+1)。1件も無ければ "W1"。 */
export function nextHarnessName(sheet: Sheet): string {
  let max = 0;
  for (const h of harnesses(sheet)) {
    const m = /^W(\d+)$/.exec(h.name ?? "");
    if (m) max = Math.max(max, Number(m[1]));
  }
  return `${HARNESS_PREFIX}${max + 1}`;
}

/**
 * 矩形ドラッグ → ハーネス追加コマンド。名前は自動採番 (W1, W2 …)。
 * つぶれた矩形 (クリックしただけ) ではnullを返してコマンドを送らない。
 */
export function harnessAddCommand(sheet: Sheet, a: Point, b: Point): Command | null {
  if (Math.abs(b.x - a.x) < MIN_SIZE || Math.abs(b.y - a.y) < MIN_SIZE) return null;
  return {
    type: "add_entity",
    sheet_id: sheet.id,
    entity: {
      kind: "harness",
      id: crypto.randomUUID(),
      points: harnessRectPoints(a, b),
      name: nextHarnessName(sheet),
      note: "",
    },
  };
}
