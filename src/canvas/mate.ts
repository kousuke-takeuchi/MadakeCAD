// 嵌合コネクタ対 (G14)。Rust側 madake-core/src/mate.rs と同一ルール:
//   - `attrs["mates_with"]`にもう一方の参照記号を書いて対を宣言する(双方に書いて初めて
//     「両側から宣言された対」)
//   - 対象は symbol_id が "connector_" で始まるシンボルだけ(動的connector_{n}pと
//     静的connector_plug/connector_socketの両方。端子台は対象にしない)
//   - 図面には、両方が同一シート上・50mm以内にあるときだけ短い破線を自動で描く
//     (それ以外は何も描かず、帳票 (`reports.rs`) だけが対を伝える)
// svg.rsのrender_mate_links / rect_boundary_point_towardと同一の幾何。

import { resolveInstanceDef } from "./dynamicSymbol";
import type { Point, Project, Sheet, SymbolDef, SymbolInstance } from "../ipc";
import { symbolBounds } from "./dynamicSymbol";

/** `mates_with`宣言を書き込む属性キー。 */
export const MATES_WITH_ATTR = "mates_with";
/** 嵌合対の破線を描く距離の上限 (mm)。svg.rsのMATE_LINE_MAX_DIST_MMと一致させること。 */
export const MATE_LINE_MAX_DIST_MM = 50;
/** 破線の長さと間隔 (mm)。svg.rsのMATE_LINE_DASHと一致させること。 */
export const MATE_LINE_DASH: [number, number] = [1.5, 1];

/** このシンボルが嵌合対の対象になるコネクタ一族か(静的プラグ/ソケット含む)。 */
export function isConnectorInstance(symbolId: string): boolean {
  return symbolId.startsWith("connector_");
}

/** 図面に破線を描く相手先ペア。 */
export interface MateLink {
  sheetId: string;
  a: string;
  b: string;
}

interface MateEntry {
  sheetId: string;
  entityId: string;
  declared: string | null;
}

function collect(project: Project, defs: ReadonlyMap<string, SymbolDef>): Map<string, MateEntry> {
  const out = new Map<string, MateEntry>();
  for (const sheet of project.sheets) {
    for (const entity of Object.values(sheet.entities)) {
      if (entity.kind !== "symbol") continue;
      const s = entity as SymbolInstance & { kind: "symbol" };
      if (!isConnectorInstance(s.symbol_id)) continue;
      const reference = s.reference.trim();
      if (!reference || out.has(reference)) continue;
      // 定義が引けないシンボルは対象外 (Rust側mate.rsのresolve_symbolでの足切りと揃える。
      // 極数はERCで使うがERCはRust側だけの仕事なので、ここでは持たない)
      if (!resolveInstanceDef(s, defs)) continue;
      const raw = (s.attrs ?? {})[MATES_WITH_ATTR]?.trim() ?? "";
      out.set(reference, {
        sheetId: sheet.id,
        entityId: s.id,
        declared: raw || null,
      });
    }
  }
  return out;
}

/**
 * あるシート上の参照記号が持つ`mates_with`宣言の値。宣言が無い・コネクタ一族でない
 * 参照記号は空文字。プロパティパネルの表示・電線リスト帳票の考え方と揃える。
 */
export function declaredMate(sheet: Sheet, reference: string): string {
  const ref = reference.trim();
  if (!ref) return "";
  for (const entity of Object.values(sheet.entities)) {
    if (entity.kind !== "symbol") continue;
    const s = entity as SymbolInstance & { kind: "symbol" };
    if (!isConnectorInstance(s.symbol_id) || s.reference.trim() !== ref) continue;
    return (s.attrs ?? {})[MATES_WITH_ATTR]?.trim() ?? "";
  }
  return "";
}

/**
 * 図面に破線を描くべき嵌合対。両方が同一シートにあり、配置基準点どうしの距離が
 * [`MATE_LINE_MAX_DIST_MM`]以内のときだけ対象になる。ERC(片側宣言・極数不一致)とは
 * 独立に、宣言さえあれば対を返す。
 */
export function mateLinks(project: Project, defs: ReadonlyMap<string, SymbolDef>): MateLink[] {
  const entries = collect(project, defs);
  const out: MateLink[] = [];
  const seen = new Set<string>();
  for (const [reference, entry] of entries) {
    const targetRef = entry.declared;
    if (!targetRef || targetRef === reference) continue;
    const target = entries.get(targetRef);
    if (!target || target.sheetId !== entry.sheetId) continue;
    const sheet = project.sheets.find((s) => s.id === entry.sheetId);
    if (!sheet) continue;
    const a = sheet.entities[entry.entityId];
    const b = sheet.entities[target.entityId];
    if (!a || a.kind !== "symbol" || !b || b.kind !== "symbol") continue;
    const key = [entry.entityId, target.entityId].sort().join("|");
    if (seen.has(key)) continue;
    seen.add(key);
    const dx = (a as SymbolInstance).at.x - (b as SymbolInstance).at.x;
    const dy = (a as SymbolInstance).at.y - (b as SymbolInstance).at.y;
    if (Math.hypot(dx, dy) > MATE_LINE_MAX_DIST_MM) continue;
    const [x, y] = [entry.entityId, target.entityId].sort();
    out.push({ sheetId: entry.sheetId, a: x, b: y });
  }
  return out;
}

/**
 * 矩形の周上で、外部の点`target`にいちばん近い点(G14の「外形どうしの最短点」)。
 * svg.rsのrect_boundary_point_towardと同一。
 */
export function rectBoundaryPointToward(min: Point, max: Point, target: Point): Point {
  const cx = Math.min(Math.max(target.x, min.x), max.x);
  const cy = Math.min(Math.max(target.y, min.y), max.y);
  const inside = cx > min.x && cx < max.x && cy > min.y && cy < max.y;
  if (!inside) return { x: cx, y: cy };
  const dl = cx - min.x;
  const dr = max.x - cx;
  const dt = cy - min.y;
  const db = max.y - cy;
  const m = Math.min(dl, dr, dt, db);
  if (m === dl) return { x: min.x, y: cy };
  if (m === dr) return { x: max.x, y: cy };
  if (m === dt) return { x: cx, y: min.y };
  return { x: cx, y: max.y };
}

/** 1組の嵌合対を結ぶ線分の両端(用紙座標)。片方でもシンボルが解決できなければnull。 */
export function mateLinkSegment(
  sheet: Sheet,
  link: MateLink,
  defs: ReadonlyMap<string, SymbolDef>,
): { a: Point; b: Point } | null {
  const ea = sheet.entities[link.a];
  const eb = sheet.entities[link.b];
  if (!ea || ea.kind !== "symbol" || !eb || eb.kind !== "symbol") return null;
  const sa = ea as SymbolInstance & { kind: "symbol" };
  const sb = eb as SymbolInstance & { kind: "symbol" };
  const defA = resolveInstanceDef(sa, defs);
  const defB = resolveInstanceDef(sb, defs);
  if (!defA || !defB) return null;
  const boundsA = symbolBounds(sa, defA);
  const boundsB = symbolBounds(sb, defB);
  return {
    a: rectBoundaryPointToward(boundsA.min, boundsA.max, sb.at),
    b: rectBoundaryPointToward(boundsB.min, boundsB.max, sa.at),
  };
}
