// 盤レイアウト (M4仕様 §8) の作図ツール・未配置部品の配置・プロパティ編集を
// **Command** へ変換する層。幾何の数式は`panel.ts` (Rustの`panel.rs`と同一) が持つ。
//
// 図面を変えるのは必ずCommandエンジン (CLAUDE.md「アーキテクチャの絶対原則」)。
// ここでは「ドラッグ・クリック・入力 → どのコマンドを送るか」だけを決め、モデルには触らない。
//
// **盤エンティティ専用のコマンドは作らない**: 既存の`add_entity`/`update_entity`/
// `move_entities`にそのまま相乗りするので、undo/redoもpatch配信も従来どおり効く (Task 5の決定)。

import type { Command, DinRail, Duct, Entity, Footprint, PanelOutline, Point, Sheet } from "../ipc";
import {
  DEFAULT_FOOTPRINT_SIZE,
  footprints,
  footprintSize,
  numberedRails,
  rails,
  railSnapY,
} from "./panel";
import { isPanelSheet } from "./sheets";

/** 盤の作図ツール (リボン「パネル」タブ)。 */
export type PanelToolId =
  | "panel_outline"
  | "din_rail"
  | "duct"
  | "footprint"
  | "panel_legend";

/** これより小さいドラッグは作図にしない (クリックだけの誤操作を弾く) mm。 */
export const MIN_PANEL_SIZE = 0.5;
/** 盤の名前の接頭辞 (CP-1, CP-2 …)。 */
export const PANEL_NAME_PREFIX = "CP-";

/** 未配置部品パネルの1行の見せ方。 */
export interface UnplacedRow {
  reference: string;
  partNo: string;
  w: number;
  h: number;
  /** 外形寸法の表示 ("25 × 65 mm")。 */
  sizeLabel: string;
  /**
   * 寸法の出どころ:
   * - `sized`        部品DBに外形が登録されている
   * - `default`      型番はあるが外形が未登録 (既定の矩形で置く)
   * - `unregistered` 型番そのものが入っていない
   */
  status: "sized" | "default" | "unregistered";
}

/** 未配置部品の1件 (Rustの`UnplacedComponent`と同じ形。ここでは寸法だけを使う)。 */
interface PlaceableComponent {
  reference: string;
  part_no: string;
  w: number;
  h: number;
}

function rect(a: Point, b: Point) {
  const x = Math.min(a.x, b.x);
  const y = Math.min(a.y, b.y);
  return { x, y, w: Math.abs(b.x - a.x), h: Math.abs(b.y - a.y) };
}

/** 次に使う盤の名前 (既存の CP-番号 の最大+1)。1枚も無ければ "CP-1"。 */
export function nextPanelName(sheet: Sheet): string {
  let max = 0;
  for (const e of Object.values(sheet.entities)) {
    if (e.kind !== "panel_outline") continue;
    const m = /^CP-(\d+)$/.exec(e.name ?? "");
    if (m) max = Math.max(max, Number(m[1]));
  }
  return `${PANEL_NAME_PREFIX}${max + 1}`;
}

/**
 * 盤枠ツール: 矩形ドラッグ → 盤の外形 (取付板つき) を1つ足すコマンド。
 * つぶれた矩形 (クリックしただけ) ではnullを返してコマンドを送らない。
 */
export function panelOutlineAddCommand(sheet: Sheet, a: Point, b: Point): Command | null {
  const r = rect(a, b);
  if (r.w < MIN_PANEL_SIZE || r.h < MIN_PANEL_SIZE) return null;
  return {
    type: "add_entity",
    sheet_id: sheet.id,
    entity: {
      kind: "panel_outline",
      id: crypto.randomUUID(),
      at: { x: r.x, y: r.y },
      w: r.w,
      h: r.h,
      name: nextPanelName(sheet),
    },
  };
}

/**
 * DINレールツール: 水平ドラッグ → レール1本。**取付線の高さは押し始めた点**で決まる
 * (v1は水平レールのみなので、上下にぶれても引いた高さは動かない)。
 */
export function dinRailAddCommand(sheet: Sheet, a: Point, b: Point): Command | null {
  if (Math.abs(b.x - a.x) < MIN_PANEL_SIZE) return null;
  return {
    type: "add_entity",
    sheet_id: sheet.id,
    entity: {
      kind: "din_rail",
      id: crypto.randomUUID(),
      y: a.y,
      x1: Math.min(a.x, b.x),
      x2: Math.max(a.x, b.x),
    },
  };
}

/** ダクトツール: 矩形ドラッグ → 配線ダクト1つ。 */
export function ductAddCommand(sheet: Sheet, a: Point, b: Point): Command | null {
  const r = rect(a, b);
  if (r.w < MIN_PANEL_SIZE || r.h < MIN_PANEL_SIZE) return null;
  return {
    type: "add_entity",
    sheet_id: sheet.id,
    entity: { kind: "duct", id: crypto.randomUUID(), at: { x: r.x, y: r.y }, w: r.w, h: r.h },
  };
}

/** 未配置部品の1行 (参照記号・型番・外形寸法と、その寸法の出どころ)。 */
export function unplacedRow(component: PlaceableComponent): UnplacedRow {
  const partNo = component.part_no.trim();
  const isDefault =
    component.w === DEFAULT_FOOTPRINT_SIZE.w && component.h === DEFAULT_FOOTPRINT_SIZE.h;
  return {
    reference: component.reference,
    partNo,
    w: component.w,
    h: component.h,
    sizeLabel: `${component.w} × ${component.h} mm`,
    status: partNo === "" ? "unregistered" : isDefault ? "default" : "sized",
  };
}

/**
 * フットプリントを置く位置 (描かれる矩形の左上)。
 *
 * カーソルは**部品の中心**を持つ (外形の左上を持つと、90度回したときに手元がずれる)。
 * DINレールの近く (帯の中) へ落とすと、部品の中心がレールの取付線へ吸い付く。
 */
export function footprintPlaceAt(
  sheet: Sheet,
  component: PlaceableComponent,
  cursor: Point,
  rotation: number,
): Point {
  const size = footprintSize({ w: component.w, h: component.h, rotation });
  const snapped = railSnapY(rails(sheet), cursor.y);
  const centerY = snapped ?? cursor.y;
  return { x: cursor.x - size.w / 2, y: centerY - size.h / 2 };
}

/** 未配置部品 → フットプリントを1つ足すコマンド (参照記号と型番を引き継ぐ)。 */
export function footprintAddCommand(
  sheet: Sheet,
  component: PlaceableComponent,
  cursor: Point,
  rotation: number,
): Command {
  const at = footprintPlaceAt(sheet, component, cursor, rotation);
  return {
    type: "add_entity",
    sheet_id: sheet.id,
    entity: {
      kind: "footprint",
      id: crypto.randomUUID(),
      at,
      w: component.w,
      h: component.h,
      rotation: normalizeRotation(rotation),
      reference: component.reference,
      part_no: component.part_no,
    },
  };
}

/**
 * フットプリントの移動量をレールへ吸い付かせる (置いたあとに動かすときのスナップ)。
 * 近くにレールが無ければ移動量はそのまま。
 */
export function railSnapDelta(sheet: Sheet, fp: Footprint, delta: Point): Point {
  const size = footprintSize(fp);
  const center = fp.at.y + size.h / 2 + delta.y;
  const snapped = railSnapY(rails(sheet), center);
  return snapped === null ? delta : { x: delta.x, y: delta.y + (snapped - center) };
}

/**
 * フットプリントが乗っているレール (プロパティの「所属レール」)。
 * 取付線が部品の外形を横切っていて、左右の範囲も重なっているものを採る。
 * 複数あれば部品の中心に一番近いもの。乗っていなければnull。
 *
 * レール番号は`numberedRails` (盤の上から順) なので、盤内部品配置表の
 * 「レール」欄と同じレールを指す。
 */
export function railOfFootprint(
  sheet: Sheet,
  fp: Footprint,
): { rail: DinRail; index: number } | null {
  const size = footprintSize(fp);
  const top = fp.at.y;
  const bottom = fp.at.y + size.h;
  const left = fp.at.x;
  const right = fp.at.x + size.w;
  const center = fp.at.y + size.h / 2;
  let best: { rail: DinRail; index: number } | null = null;
  for (const { index, rail } of numberedRails(sheet)) {
    if (rail.y < top || rail.y > bottom) continue;
    if (Math.max(rail.x1, rail.x2) < left || Math.min(rail.x1, rail.x2) > right) continue;
    if (!best || Math.abs(rail.y - center) < Math.abs(best.rail.y - center)) {
      best = { rail, index };
    }
  }
  return best;
}

/**
 * バルーン採番コマンド: 盤シートのフットプリントへ読み順で 1..N を振る。
 * 置く部品が1つも無ければnull (送っても何も起きないコマンドを積まない)。
 */
export function renumberPanelItemsCommand(sheet: Sheet): Command | null {
  if (footprints(sheet).length === 0) return null;
  return { type: "renumber_panel_items", sheet_id: sheet.id };
}

/**
 * 凡例表を1つ置くコマンド。中身 (アイテム番号↔参照記号・型番) は描画のたびに
 * 同じシートのフットプリントから導出するので、ここでは置き場所だけを決める。
 */
export function panelLegendAddCommand(sheet: Sheet, at: Point): Command {
  return {
    type: "add_entity",
    sheet_id: sheet.id,
    entity: { kind: "panel_legend", id: crypto.randomUUID(), at },
  };
}

/** プロパティで編集できる値 (種別ごとに使える欄だけが効く)。 */
export interface PanelPatch {
  name?: string;
  part_no?: string;
  w?: number;
  h?: number;
  rotation?: number;
  y?: number;
  x1?: number;
  x2?: number;
}

/** v1の盤の描画が持つ向きは0度と90度だけ。 */
function normalizeRotation(rotation: number): number {
  return Math.round(rotation / 90) % 2 === 0 ? 0 : 90;
}

/** 正の寸法として使える数値か (0・負・NaNは無視して元の値を残す)。 */
function positive(value: number | undefined): number | null {
  return typeof value === "number" && Number.isFinite(value) && value > 0 ? value : null;
}

function finite(value: number | undefined): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

/**
 * 盤の要素のプロパティ編集 → `update_entity`コマンド。
 * 変わっていなければnull (無駄なundo段を作らない)。盤以外の要素にはnullを返す。
 */
export function panelUpdateCommand(
  sheetId: string,
  entity: Entity,
  patch: PanelPatch,
): Command | null {
  let next: Entity;
  switch (entity.kind) {
    case "footprint": {
      const fp: Footprint & { kind: "footprint" } = { ...entity };
      fp.w = positive(patch.w) ?? fp.w;
      fp.h = positive(patch.h) ?? fp.h;
      if (patch.rotation !== undefined && Number.isFinite(patch.rotation)) {
        fp.rotation = normalizeRotation(patch.rotation);
      }
      if (patch.part_no !== undefined) fp.part_no = patch.part_no.trim();
      next = fp;
      break;
    }
    case "panel_outline": {
      const o: PanelOutline & { kind: "panel_outline" } = { ...entity };
      o.w = positive(patch.w) ?? o.w;
      o.h = positive(patch.h) ?? o.h;
      if (patch.name !== undefined) o.name = patch.name.trim();
      next = o;
      break;
    }
    case "din_rail": {
      const r: DinRail & { kind: "din_rail" } = { ...entity };
      r.y = finite(patch.y) ?? r.y;
      r.x1 = finite(patch.x1) ?? r.x1;
      r.x2 = finite(patch.x2) ?? r.x2;
      next = r;
      break;
    }
    case "duct": {
      const d: Duct & { kind: "duct" } = { ...entity };
      d.w = positive(patch.w) ?? d.w;
      d.h = positive(patch.h) ?? d.h;
      next = d;
      break;
    }
    default:
      return null;
  }
  if (JSON.stringify(next) === JSON.stringify(entity)) return null;
  return { type: "update_entity", sheet_id: sheetId, entity: next };
}

/**
 * 回路図のツール (配線・部品挿入・ハーネス・マクロ) を使えるシートか。
 * **盤レイアウトシートでは使えない**: 盤面に配線やシンボルを置いても検証も
 * ネットリストも見ないので、置けてしまうほうが分かりにくいため。
 * Commandエンジン側にシート種別のガードは無く、この判断はUIだけが持つ。
 */
export function schematicToolsAllowed(sheet: Sheet | null | undefined): boolean {
  return !!sheet && !isPanelSheet(sheet);
}

/** 盤のツール (盤枠・DINレール・ダクト・フットプリント) を使えるシートか。 */
export function panelToolsAllowed(sheet: Sheet | null | undefined): boolean {
  return isPanelSheet(sheet);
}
