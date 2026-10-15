// 盤レイアウトの銘板と関連付き寸法線 (M4 §8、.pen「M4残りデザイン⑤」`B2gfQZ`)。
//
// 形の計算はRust側 madake-core/src/panel.rs (nameplate_rect / nameplate_lines /
// dimension_geometry) と同じ規則で、SVG出力と画面が同じ位置に描く。編集は必ずCommand経由
// (銘板・寸法線はadd_entity、揃え・等間隔はalign_footprints / distribute_footprints)。

import type { Command, DimAnchor, Dimension, Entity, Nameplate, Point, Project, Sheet } from "../ipc";
import { footprintRect } from "./panel";
import { FUNCTION_LABEL_ATTR } from "./textStyle";

/** 銘板とフットプリントの隙間 mm (Rust NAMEPLATE_GAP)。 */
export const NAMEPLATE_GAP = 2;
/** 寸法の補助線が端点から離れる量 mm (Rust DIM_EXT_GAP)。 */
export const DIM_EXT_GAP = 1;
/** 寸法線の端の斜線の長さ mm (Rust DIM_TICK)。 */
export const DIM_TICK = 1.5;
/** 寸法値の文字高 mm (Rust DIM_FONT)。 */
export const DIM_FONT = 2.5;
/** 新しい銘板の既定の大きさ・文字高 mm。 */
export const NAMEPLATE_DEFAULT = { w: 30, h: 10, text_mm: 3.5 } as const;
/** 寸法線を端点からどれだけ離して置くか mm (寸法ツールの既定)。 */
export const DIM_OFFSET = 10;

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** 銘板の外形。結び付いた銘板はフットプリントの下 (上) に中央揃え。結び付き先が無ければnull。 */
export function nameplateRect(sheet: Sheet, np: Nameplate): Rect | null {
  if (!np.footprint) return { x: np.at.x, y: np.at.y, w: np.w, h: np.h };
  const fp = sheet.entities[np.footprint];
  if (!fp || fp.kind !== "footprint") return null;
  const r = footprintRect(fp);
  const x = r.x + r.w / 2 - np.w / 2;
  const y = np.position === "above" ? r.y - NAMEPLATE_GAP - np.h : r.y + r.h + NAMEPLATE_GAP;
  return { x, y, w: np.w, h: np.h };
}

/** 回路図の参照記号 → 説明 (シンボルの機能ラベル。最初に見つかった空でないもの)。 */
export function deviceDescriptions(project: Project | null): Map<string, string> {
  const out = new Map<string, string>();
  for (const sheet of project?.sheets ?? []) {
    for (const e of Object.values(sheet.entities)) {
      if (e.kind !== "symbol") continue;
      const label = e.attrs?.[FUNCTION_LABEL_ATTR]?.trim();
      const reference = e.reference.trim();
      if (label && !out.has(reference)) out.set(reference, label);
    }
  }
  return out;
}

/** 銘板の文字の行。説明に追従する銘板は1行目を回路図の説明にする (無ければ保存した行のまま)。 */
export function nameplateLines(sheet: Sheet, np: Nameplate, descriptions: ReadonlyMap<string, string>): string[] {
  const lines = [...np.lines];
  if (!np.follow_description || !np.footprint) return lines;
  const fp = sheet.entities[np.footprint];
  const reference = fp && fp.kind === "footprint" ? fp.reference.trim() : "";
  const d = reference ? descriptions.get(reference) : undefined;
  if (d) {
    if (lines.length === 0) lines.push(d);
    else lines[0] = d;
  }
  return lines;
}

/** 寸法線の端点の位置 (フットプリントは中心)。無い部品を指していればnull。 */
export function dimAnchorPoint(sheet: Sheet, anchor: DimAnchor): Point | null {
  if (anchor.kind === "point") return anchor.at;
  const fp = sheet.entities[anchor.id];
  if (!fp || fp.kind !== "footprint") return null;
  const r = footprintRect(fp);
  return { x: r.x + r.w / 2, y: r.y + r.h / 2 };
}

export interface DimGeometry {
  extensions: [Point, Point][];
  line: [Point, Point];
  textAt: Point;
  text: string;
}

/** 寸法線を描く形 (Rust dimension_geometry と同じ)。端点が引けなければnull。 */
export function dimensionGeometry(sheet: Sheet, d: Dimension): DimGeometry | null {
  const a = dimAnchorPoint(sheet, d.a);
  const b = dimAnchorPoint(sheet, d.b);
  if (!a || !b) return null;
  const horizontal = d.axis === "horizontal";
  const pa = horizontal ? { x: a.x, y: d.at } : { x: d.at, y: a.y };
  const pb = horizontal ? { x: b.x, y: d.at } : { x: d.at, y: b.y };
  const value = horizontal ? Math.abs(b.x - a.x) : Math.abs(b.y - a.y);
  const ext = (from: Point, to: Point): [Point, Point] => {
    const len = Math.hypot(to.x - from.x, to.y - from.y);
    if (len < 1e-9) return [to, to];
    const ux = (to.x - from.x) / len;
    const uy = (to.y - from.y) / len;
    return [
      { x: from.x + ux * DIM_EXT_GAP, y: from.y + uy * DIM_EXT_GAP },
      { x: to.x + ux * DIM_EXT_GAP, y: to.y + uy * DIM_EXT_GAP },
    ];
  };
  const mid = { x: (pa.x + pb.x) / 2, y: (pa.y + pb.y) / 2 };
  return {
    extensions: [ext(a, pa), ext(b, pb)],
    line: [pa, pb],
    textAt: horizontal ? { x: mid.x, y: mid.y - 1 } : { x: mid.x - 1, y: mid.y },
    text: value.toFixed(1).replace(/0+$/, "").replace(/\.$/, ""),
  };
}

/**
 * 寸法ツールの確定: 2つの端点から寸法線を作る。向きは水平・垂直の距離の大きいほう、
 * 線の位置は端点から`DIM_OFFSET`だけ外側 (水平なら上、垂直なら左)。同じ点ならnull。
 */
export function dimensionAddCommand(sheet: Sheet, a: DimAnchor, b: DimAnchor): Command | null {
  const pa = dimAnchorPoint(sheet, a);
  const pb = dimAnchorPoint(sheet, b);
  if (!pa || !pb) return null;
  const dx = Math.abs(pb.x - pa.x);
  const dy = Math.abs(pb.y - pa.y);
  if (dx < 1e-9 && dy < 1e-9) return null;
  const axis = dx >= dy ? "horizontal" : "vertical";
  const at = axis === "horizontal" ? Math.min(pa.y, pb.y) - DIM_OFFSET : Math.min(pa.x, pb.x) - DIM_OFFSET;
  return {
    type: "add_entity",
    sheet_id: sheet.id,
    entity: { kind: "dimension", id: crypto.randomUUID(), a, b, axis, at },
  };
}

/** 銘板の挿入コマンド (フットプリントに結び付けるか、`at`に単独で置く)。 */
export function nameplateAddCommand(
  sheet: Sheet,
  fields: Omit<Nameplate, "id">,
): Command {
  return { type: "add_entity", sheet_id: sheet.id, entity: { kind: "nameplate", id: crypto.randomUUID(), ...fields } };
}

/** クリック位置にあるフットプリント (寸法・銘板ツールの端点選び)。無ければnull。 */
export function footprintAt(sheet: Sheet, p: Point): Extract<Entity, { kind: "footprint" }> | null {
  for (const e of Object.values(sheet.entities)) {
    if (e.kind !== "footprint") continue;
    const r = footprintRect(e);
    if (p.x >= r.x && p.x <= r.x + r.w && p.y >= r.y && p.y <= r.y + r.h) return e;
  }
  return null;
}

/** 銘板・寸法線を掴めるか (銘板は外形の中、寸法線は線の近く)。 */
export function panelMarkHit(sheet: Sheet, e: Entity, p: Point, tolerance: number): boolean {
  if (e.kind === "nameplate") {
    const r = nameplateRect(sheet, e);
    return !!r && p.x >= r.x && p.x <= r.x + r.w && p.y >= r.y && p.y <= r.y + r.h;
  }
  if (e.kind === "dimension") {
    const g = dimensionGeometry(sheet, e);
    if (!g) return false;
    const [a, b] = g.line;
    const len2 = (b.x - a.x) ** 2 + (b.y - a.y) ** 2;
    const t = len2 === 0 ? 0 : Math.max(0, Math.min(1, ((p.x - a.x) * (b.x - a.x) + (p.y - a.y) * (b.y - a.y)) / len2));
    return Math.hypot(p.x - (a.x + t * (b.x - a.x)), p.y - (a.y + t * (b.y - a.y))) <= tolerance;
  }
  return false;
}
