// 図形 (G10) の幾何計算・当たり判定・コマンド組み立て。
// Rust側 madake-core/src/shape.rs と同一ルール:
//   - 種別ごとの`points`の読み方 (矩形=対角2点 / 線=両端2点 / 円=中心+円周上の1点 / 折れ線=N点)
//   - 色は注記と同じ**トークン名**だけ (生の色コードはコマンドが拒否する)
//   - 線幅は0.1〜2.0mm、破線は3mm/間隔2mm (ハーネス境界と同じ刻み)
//   - 塗りは下の回路が透けて読める濃さ (SHAPE_FILL_OPACITY)
// 図面への反映は必ず Command 経由 (CLAUDE.md「アーキテクチャの絶対原則」)。

import type { Command, Entity, Point, Shape, ShapeKind, Sheet, TextColorToken } from "../ipc";
import { textColors } from "./theme";
import { theme } from "./theme";

/** 図形の種類 (作図ツール・プロパティの選択肢の並び順)。 */
export const SHAPE_KINDS: ShapeKind[] = ["rect", "line", "circle", "polyline"];

/** 作図ツールのid (リボン「回路図」タブ>回路図を編集>図形)。 */
export type ShapeToolId = "shape_rect" | "shape_line" | "shape_circle" | "shape_polyline";

/** 作図ツール → 描く図形の種類。 */
export const SHAPE_TOOLS: Record<ShapeToolId, ShapeKind> = {
  shape_rect: "rect",
  shape_line: "line",
  shape_circle: "circle",
  shape_polyline: "polyline",
};

/** 作図ツールの一覧 (リボンの並び順)。 */
export const SHAPE_TOOL_IDS = Object.keys(SHAPE_TOOLS) as ShapeToolId[];

/** そのツールidが図形の作図ツールか。 */
export function isShapeTool(tool: string): tool is ShapeToolId {
  return tool in SHAPE_TOOLS;
}

/** 破線の線の長さと間隔 (mm)。shape.rsのSHAPE_DASHと一致させること。 */
export const SHAPE_DASH: [number, number] = [3, 2];
/** 線幅の既定値 (mm)。図枠の細線・ハーネス境界と同じ太さ。 */
export const DEFAULT_SHAPE_STROKE_MM = 0.25;
/** 線幅の下限・上限 (mm)。範囲外はコマンドエンジンが拒否する。 */
export const MIN_SHAPE_STROKE_MM = 0.1;
export const MAX_SHAPE_STROKE_MM = 2.0;
/** 塗りの不透明度。塗っても下の配線・記号が透けて読める濃さ。 */
export const SHAPE_FILL_OPACITY = 0.15;
/** 色を指定していない図形が使うトークン。 */
export const DEFAULT_SHAPE_COLOR: TextColorToken = "default";

/** プロパティの線幅の選択肢 (mm)。範囲の両端を含む。 */
export const SHAPE_STROKE_CHOICES = [0.1, 0.25, 0.35, 0.5, 0.7, 1.0, 1.4, 2.0];

/** これより小さい図形は作らない (クリックだけの誤操作を弾く) mm。 */
export const MIN_SHAPE_SIZE = 0.5;

/** その種別が必要とする頂点の数 (null = 2点以上なら何点でもよい = 折れ線)。 */
export function requiredPoints(kind: ShapeKind): number | null {
  return kind === "polyline" ? null : 2;
}

/** 線幅が図面に出せる範囲か (Rust側と同じ判定)。 */
export function isValidShapeStrokeMm(width: number): boolean {
  return (
    Number.isFinite(width) && width >= MIN_SHAPE_STROKE_MM && width <= MAX_SHAPE_STROKE_MM
  );
}

/** シート上の全図形。 */
export function shapes(sheet: Sheet): Array<Extract<Entity, { kind: "shape" }>> {
  return Object.values(sheet.entities).filter(
    (e): e is Extract<Entity, { kind: "shape" }> => e.kind === "shape",
  );
}

/** 矩形の正規化した対角 (左上, 右下)。どの向きにドラッグしても同じ矩形になる。 */
export function shapeRectCorners(points: Point[]): { min: Point; max: Point } | null {
  if (points.length < 2) return null;
  const [a, b] = points;
  return {
    min: { x: Math.min(a.x, b.x), y: Math.min(a.y, b.y) },
    max: { x: Math.max(a.x, b.x), y: Math.max(a.y, b.y) },
  };
}

/** 円の中心と半径 (mm)。1点目が中心・2点目が円周上の点。 */
export function shapeCircle(points: Point[]): { center: Point; r: number } | null {
  if (points.length < 2) return null;
  const [center, edge] = points;
  return { center, r: Math.hypot(edge.x - center.x, edge.y - center.y) };
}

/** 図形の外接矩形 (左上, 右下)。円は中心±半径、他は頂点の最小・最大。 */
export function shapeBounds(shape: Shape): { min: Point; max: Point } | null {
  if (shape.shape === "circle") {
    const c = shapeCircle(shape.points);
    if (!c) return null;
    return {
      min: { x: c.center.x - c.r, y: c.center.y - c.r },
      max: { x: c.center.x + c.r, y: c.center.y + c.r },
    };
  }
  if (shape.points.length === 0) return null;
  const xs = shape.points.map((p) => p.x);
  const ys = shape.points.map((p) => p.y);
  return {
    min: { x: Math.min(...xs), y: Math.min(...ys) },
    max: { x: Math.max(...xs), y: Math.max(...ys) },
  };
}

/** 図形を画面へ描く線の色。選択ハイライトが色トークンより優先する。 */
export function shapeStrokeColor(shape: Shape, selected: boolean): string {
  if (selected) return theme.selection;
  return textColors[shape.stroke] ?? textColors[DEFAULT_SHAPE_COLOR];
}

/** 図形を画面へ塗る色 (半透明)。塗り無しなら null。 */
export function shapeFillColor(shape: Shape): string | null {
  if (!shape.fill) return null;
  const hex = textColors[shape.fill];
  if (!hex) return null;
  const v = Number.parseInt(hex.slice(1), 16);
  return `rgba(${(v >> 16) & 255}, ${(v >> 8) & 255}, ${v & 255}, ${SHAPE_FILL_OPACITY})`;
}

/** 点から線分までの距離 (mm)。当たり判定に使う。 */
function segmentDistance(p: Point, a: Point, b: Point): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const len2 = dx * dx + dy * dy;
  if (len2 < 1e-12) return Math.hypot(p.x - a.x, p.y - a.y);
  const t = Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / len2));
  return Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy));
}

/**
 * 図形を掴めるか (**描かれている線の上**で判定する)。塗りつぶした図形でも、面の中では
 * なく輪郭で掴む: 塗りの下にある配線・記号を選びたいことのほうが多いから。
 */
export function shapeHitTest(shape: Shape, p: Point, tolerance: number): boolean {
  switch (shape.shape) {
    case "rect": {
      const r = shapeRectCorners(shape.points);
      if (!r) return false;
      const corners = [
        r.min,
        { x: r.max.x, y: r.min.y },
        r.max,
        { x: r.min.x, y: r.max.y },
      ];
      return corners.some((a, i) => segmentDistance(p, a, corners[(i + 1) % 4]) <= tolerance);
    }
    case "circle": {
      const c = shapeCircle(shape.points);
      if (!c) return false;
      return Math.abs(Math.hypot(p.x - c.center.x, p.y - c.center.y) - c.r) <= tolerance;
    }
    case "line":
    case "polyline": {
      for (let i = 0; i + 1 < shape.points.length; i += 1) {
        if (segmentDistance(p, shape.points[i], shape.points[i + 1]) <= tolerance) return true;
      }
      return false;
    }
  }
}

/** 図形の既定の見た目 (既定色・細線・塗りなし・実線)。 */
export function newShape(kind: ShapeKind, points: Point[]): Extract<Entity, { kind: "shape" }> {
  return {
    kind: "shape",
    id: crypto.randomUUID(),
    shape: kind,
    points,
    stroke: DEFAULT_SHAPE_COLOR,
    stroke_width_mm: DEFAULT_SHAPE_STROKE_MM,
    fill: null,
    dashed: false,
  };
}

/**
 * 続けて同じ場所に打たれた点をひとつにまとめる。折れ線を**ダブルクリックで確定**すると、
 * 確定のクリックが最後の点をもう一度打つので、そのままだと同じ点が重なって残る。
 */
export function dedupeConsecutive(points: Point[]): Point[] {
  return points.filter(
    (p, i) => i === 0 || p.x !== points[i - 1].x || p.y !== points[i - 1].y,
  );
}

/**
 * 作図の確定 → 図形を1つ足すコマンド。**つぶれた図形 (クリックしただけ・同じ点だけの
 * 折れ線) ではnullを返してコマンドを送らない**。
 */
export function shapeAddCommand(
  sheet: Sheet,
  kind: ShapeKind,
  input: Point[],
  style?: Partial<Pick<Shape, "stroke" | "stroke_width_mm" | "fill" | "dashed">>,
): Command | null {
  // 折れ線だけは重なった点を落とす (ダブルクリック確定で最後の点が2度打たれるため)
  const points = kind === "polyline" ? dedupeConsecutive(input) : input;
  const need = requiredPoints(kind);
  if (need !== null && points.length !== need) return null;
  if (need === null && points.length < 2) return null;
  const bounds = shapeBounds({ ...newShape(kind, points) });
  if (!bounds) return null;
  const w = bounds.max.x - bounds.min.x;
  const h = bounds.max.y - bounds.min.y;
  if (kind === "rect") {
    if (w < MIN_SHAPE_SIZE || h < MIN_SHAPE_SIZE) return null;
  } else if (Math.max(w, h) < MIN_SHAPE_SIZE) {
    return null;
  }
  return {
    type: "add_entity",
    sheet_id: sheet.id,
    entity: { ...newShape(kind, points), ...style },
  };
}

/** プロパティで編集できる図形の見た目。 */
export interface ShapePatch {
  stroke?: TextColorToken;
  stroke_width_mm?: number;
  /** 塗りの色トークン。null で塗りを外す。 */
  fill?: TextColorToken | null;
  dashed?: boolean;
}

/**
 * 図形の見た目の編集コマンド。何も変わっていなければ null (コマンドを送らない)。
 * 範囲外の線幅は送らずに null を返す (エンジンも同じ範囲で拒否する)。
 */
export function shapeUpdateCommand(
  sheetId: string,
  entity: Entity,
  patch: ShapePatch,
): Command | null {
  if (entity.kind !== "shape") return null;
  const width = patch.stroke_width_mm ?? entity.stroke_width_mm;
  if (!isValidShapeStrokeMm(width)) return null;
  const next = {
    ...entity,
    stroke: patch.stroke ?? entity.stroke,
    stroke_width_mm: width,
    fill: patch.fill === undefined ? (entity.fill ?? null) : patch.fill,
    dashed: patch.dashed ?? entity.dashed,
  };
  const unchanged =
    next.stroke === entity.stroke &&
    next.stroke_width_mm === entity.stroke_width_mm &&
    next.fill === (entity.fill ?? null) &&
    next.dashed === entity.dashed;
  if (unchanged) return null;
  return { type: "update_entity", sheet_id: sheetId, entity: next };
}
