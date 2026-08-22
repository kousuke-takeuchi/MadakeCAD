// 2D盤レイアウト (M4仕様 §8) の幾何計算。
// Rust側 madake-core/src/panel.rs と同じ数式で、画面とSVG/PDF出力の見た目を一致させる:
//   - 盤外形の内側に PLATE_INSET だけ入ったところが取付板
//   - DINレールは取付線 (y) を中心とした高さ DIN_RAIL_H の帯
//   - ダクトは用紙座標に固定した45度の斜線 (DUCT_HATCH_PITCH間隔) で埋める
//   - フットプリントは部品の外形。90度回すと幅と高さが入れ替わる
// 図面への反映は必ず Command 経由 (CLAUDE.md「アーキテクチャの絶対原則」)。

import type { DinRail, Duct, Entity, Footprint, PanelOutline, Sheet } from "../ipc";

/** 盤の外周から取付板までの距離 (mm)。panel.rsのPLATE_INSETと一致させること。 */
export const PLATE_INSET = 10;
/** DINレール (TS35相当) の見かけの高さ (mm)。 */
export const DIN_RAIL_H = 35;
/** ダクトのハッチ (45°斜線) のx方向の間隔 (mm)。 */
export const DUCT_HATCH_PITCH = 5;
/** 部品DBに寸法が無い型番を置くときの既定の外形 (mm)。 */
export const DEFAULT_FOOTPRINT_SIZE = { w: 20, h: 40 } as const;
/** レールへ吸い付く距離 (mm)。 */
export const RAIL_SNAP_TOLERANCE = DIN_RAIL_H / 2;
/** つぶれた図形を弾くための許容誤差 (mm)。 */
const EPS = 1e-9;

/** 用紙座標の矩形 (左上+寸法) mm。 */
export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** 斜線1本 (用紙座標mm)。 */
export interface HatchLine {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
}

/** フットプリントが盤面で占める寸法。90/270度回転で幅と高さが入れ替わる。 */
export function footprintSize(fp: Pick<Footprint, "w" | "h" | "rotation">): {
  w: number;
  h: number;
} {
  return fp.rotation % 180 === 90 ? { w: fp.h, h: fp.w } : { w: fp.w, h: fp.h };
}

/** フットプリントの矩形。 */
export function footprintRect(fp: Footprint): Rect {
  const { w, h } = footprintSize(fp);
  return { x: fp.at.x, y: fp.at.y, w, h };
}

/** 盤の外形の矩形。 */
export function panelOutlineRect(o: PanelOutline): Rect {
  return { x: o.at.x, y: o.at.y, w: o.w, h: o.h };
}

/** 取付板の矩形 (外周から PLATE_INSET 内側)。板が残らない小さな盤ではnull。 */
export function panelPlateRect(o: PanelOutline): Rect | null {
  const w = o.w - PLATE_INSET * 2;
  const h = o.h - PLATE_INSET * 2;
  if (w <= EPS || h <= EPS) return null;
  return { x: o.at.x + PLATE_INSET, y: o.at.y + PLATE_INSET, w, h };
}

/** DINレールの帯の矩形。左右はどちら向きに引かれていても同じ範囲になる。 */
export function railRect(r: DinRail): Rect {
  const x0 = Math.min(r.x1, r.x2);
  const x1 = Math.max(r.x1, r.x2);
  return { x: x0, y: r.y - DIN_RAIL_H / 2, w: x1 - x0, h: DIN_RAIL_H };
}

/**
 * ダクトを埋める45°の斜線。ダクトの内側だけを返す。
 * 線は `y = x + b` (bはDUCT_HATCH_PITCHの倍数) で用紙座標に固定してあるので、
 * ダクトを動かしても向き・間隔は変わらず、隣り合うダクトでも線がそろう。
 */
export function ductHatch(duct: Duct): HatchLine[] {
  if (duct.w <= EPS || duct.h <= EPS) return [];
  const x0 = duct.at.x;
  const y0 = duct.at.y;
  const x1 = x0 + duct.w;
  const y1 = y0 + duct.h;
  const out: HatchLine[] = [];
  for (
    let k = Math.ceil((y0 - x1) / DUCT_HATCH_PITCH);
    k <= Math.floor((y1 - x0) / DUCT_HATCH_PITCH);
    k += 1
  ) {
    const b = k * DUCT_HATCH_PITCH;
    const xs = Math.max(x0, y0 - b);
    const xe = Math.min(x1, y1 - b);
    if (xe - xs <= EPS) continue;
    out.push({ x1: xs, y1: xs + b, x2: xe, y2: xe + b });
  }
  return out;
}

/** シート上の盤エンティティを種別ごとに取り出す。 */
function ofKind<K extends Entity["kind"]>(sheet: Sheet, kind: K): Extract<Entity, { kind: K }>[] {
  return Object.values(sheet.entities).filter(
    (e): e is Extract<Entity, { kind: K }> => e.kind === kind,
  );
}

export const panelOutlines = (sheet: Sheet) => ofKind(sheet, "panel_outline");
export const rails = (sheet: Sheet) => ofKind(sheet, "din_rail");
export const ducts = (sheet: Sheet) => ofKind(sheet, "duct");
export const footprints = (sheet: Sheet) => ofKind(sheet, "footprint");

/** 部品を落とした高さ`y`に一番近いレールの取付線。近くにレールが無ければnull。 */
export function railSnapY(rails: DinRail[], y: number): number | null {
  let best: number | null = null;
  for (const rail of rails) {
    if (Math.abs(rail.y - y) > RAIL_SNAP_TOLERANCE) continue;
    if (best === null || Math.abs(rail.y - y) < Math.abs(best - y)) best = rail.y;
  }
  return best;
}
