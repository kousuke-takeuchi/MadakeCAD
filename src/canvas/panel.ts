// 2D盤レイアウト (M4仕様 §8) の幾何計算。
// Rust側 madake-core/src/panel.rs と同じ数式で、画面とSVG/PDF出力の見た目を一致させる:
//   - 盤外形の内側に PLATE_INSET だけ入ったところが取付板
//   - DINレールは取付線 (y) を中心とした高さ DIN_RAIL_H の帯
//   - ダクトは用紙座標に固定した45度の斜線 (DUCT_HATCH_PITCH間隔) で埋める
//   - フットプリントは部品の外形。90度回すと幅と高さが入れ替わる
// 図面への反映は必ず Command 経由 (CLAUDE.md「アーキテクチャの絶対原則」)。

import type { DinRail, Duct, Entity, Footprint, PanelLegend, PanelOutline, Sheet } from "../ipc";

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
/** バルーン (アイテム番号) の円の半径 (mm)。 */
export const BALLOON_R = 3.5;
/** バルーンの円とフットプリントの角を結ぶ引出線の長さ (mm)。 */
export const BALLOON_LEADER = 3;
/** バルーンに入れる番号の文字高さ (mm)。 */
export const BALLOON_FONT = 3;
/** 凡例表の列見出し (`legendRows`と同じ並び)。 */
export const LEGEND_COLUMNS = ["No", "参照記号", "型番"] as const;
/** 凡例表の列幅 (mm)。合計が表の幅。 */
export const LEGEND_COL_W = [12, 22, 46] as const;
/** 凡例表の行の高さ (mm)。列見出しの行も同じ高さ。 */
export const LEGEND_ROW_H = 5;
/** 凡例表の文字高さ (mm)。 */
export const LEGEND_FONT = 2.5;
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
export const panelLegends = (sheet: Sheet) => ofKind(sheet, "panel_legend");

/** フットプリントの中心 (回転後の外形の中心)。 */
function footprintCenter(fp: Footprint): { x: number; y: number } {
  const { w, h } = footprintSize(fp);
  return { x: fp.at.x + w / 2, y: fp.at.y + h / 2 };
}

/**
 * 盤シートのフットプリントを**読み順** (上の段から下の段へ、段の中は左から右へ) に並べる。
 * Rustの`panel::item_order`と同じ規則: 段は部品の**中心の高さ**で分け、
 * RAIL_SNAP_TOLERANCEの範囲に収まる高さは同じ段として数える
 * (同じレールに乗った部品は外形の高さが違っても同じ段になる)。
 */
export function itemOrder(sheet: Sheet): Footprint[] {
  const all = [...footprints(sheet)].sort((a, b) => {
    const ca = footprintCenter(a);
    const cb = footprintCenter(b);
    return ca.y - cb.y || ca.x - cb.x || a.id.localeCompare(b.id);
  });
  const rows: Footprint[][] = [];
  let anchor = 0;
  for (const fp of all) {
    const cy = footprintCenter(fp).y;
    const row = rows[rows.length - 1];
    if (row && Math.abs(cy - anchor) <= RAIL_SNAP_TOLERANCE) {
      row.push(fp);
    } else {
      anchor = cy;
      rows.push([fp]);
    }
  }
  return rows.flatMap((row) =>
    row.sort((a, b) => footprintCenter(a).x - footprintCenter(b).x || a.id.localeCompare(b.id)),
  );
}

/** バルーン1つ分の幾何 (白丸の中心・引出線の両端・番号)。 */
export interface Balloon {
  center: { x: number; y: number };
  leaderFrom: { x: number; y: number };
  leaderTo: { x: number; y: number };
  itemNo: number;
}

/**
 * フットプリントのバルーン。**アイテム番号が無ければnull** (採番するまで描かない)。
 * 丸は左上角から45度外へ逃がし、角とは長さBALLOON_LEADERの引出線で結ぶ。
 */
export function balloon(fp: Footprint): Balloon | null {
  const itemNo = fp.item_no;
  if (itemNo === null || itemNo === undefined) return null;
  const unit = Math.SQRT1_2;
  const at = (d: number) => ({ x: fp.at.x - unit * d, y: fp.at.y - unit * d });
  return {
    center: at(BALLOON_LEADER + BALLOON_R),
    leaderFrom: at(BALLOON_LEADER),
    leaderTo: { x: fp.at.x, y: fp.at.y },
    itemNo,
  };
}

/**
 * 凡例表に並ぶ行 (`LEGEND_COLUMNS`と同じ並び)。そのシートのフットプリントを
 * アイテム番号順に並べ、まだ採番していない部品は番号欄を空けて末尾に置く。
 */
export function legendRows(sheet: Sheet): string[][] {
  const ordered = itemOrder(sheet);
  const numbered = ordered
    .filter((f) => f.item_no !== null && f.item_no !== undefined)
    .sort((a, b) => (a.item_no ?? 0) - (b.item_no ?? 0));
  const rest = ordered.filter((f) => f.item_no === null || f.item_no === undefined);
  return [...numbered, ...rest].map((fp) => [
    fp.item_no === null || fp.item_no === undefined ? "" : String(fp.item_no),
    (fp.reference ?? "").trim(),
    (fp.part_no ?? "").trim(),
  ]);
}

/** 凡例表の矩形。列見出しの1行＋本文の行数ぶんだけ下へ伸びる。 */
export function legendRect(sheet: Sheet, legend: PanelLegend): Rect {
  const w = LEGEND_COL_W.reduce((a, b) => a + b, 0);
  return {
    x: legend.at.x,
    y: legend.at.y,
    w,
    h: LEGEND_ROW_H * (1 + legendRows(sheet).length),
  };
}

/**
 * 盤の上から順に番号を振ったDINレール (1始まり)。同じ高さなら左のレールが先。
 * Rustの`panel::numbered_rails`と同じ番号で、配置表・プロパティが同じレールを指す。
 */
export function numberedRails(sheet: Sheet): { index: number; rail: DinRail }[] {
  return [...rails(sheet)]
    .sort(
      (a, b) =>
        a.y - b.y ||
        Math.min(a.x1, a.x2) - Math.min(b.x1, b.x2) ||
        a.id.localeCompare(b.id),
    )
    .map((rail, i) => ({ index: i + 1, rail }));
}

/** 部品を落とした高さ`y`に一番近いレールの取付線。近くにレールが無ければnull。 */
export function railSnapY(rails: DinRail[], y: number): number | null {
  let best: number | null = null;
  for (const rail of rails) {
    if (Math.abs(rail.y - y) > RAIL_SNAP_TOLERANCE) continue;
    if (best === null || Math.abs(rail.y - y) < Math.abs(best - y)) best = rail.y;
  }
  return best;
}
