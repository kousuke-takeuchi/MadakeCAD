// シンボルエディタのキャンバス幾何と保存前チェック (M4仕様 §7)。
//
// キャンバスの操作 (クリック位置の丸め・当たり判定・図形の移動) と、保存前チェックの
// 判定をここに閉じ込める。Vueコンポーネントは描画とイベントだけを持つ。
//
// **検証はRust側 madake-core/src/symbol_library.rs の `validate` と同じ判断・同じコード**。
// 画面で「大丈夫」と出たものが保存で拒否されると直しようが無いので、コード
// (`symbol.pin_off_grid` 等) を揃えてある。最終的な可否は保存時にRustが決める。

import { dynamicSymbol } from "./dynamicSymbol";
import { GRID_PITCH, type Pt } from "./viewport";
import type { PinDef, PinDir, Point, Primitive, SymbolDef } from "../ipc";

export { GRID_PITCH };

/** 属性テキストのスロットキー (Rust側 `SLOT_KEYS` と同じ。順序=パレットの並び)。 */
export const SLOT_KEYS = ["TAG", "PART", "DESC", "RATING"] as const;
export type SlotKey = (typeof SLOT_KEYS)[number];

/** 必須の属性スロット (参照記号)。これが無いと配置しても記号名が出せない。 */
export const REQUIRED_SLOT: SlotKey = "TAG";

/** 任意の属性スロット (型番・説明・定格)。未配置は注意止まりで保存はできる。 */
export const OPTIONAL_SLOTS: SlotKey[] = ["PART", "DESC", "RATING"];

/** 作図ツール (デザインの左ツール列: 選/線/矩/円/弧/文/属/ピ)。 */
export type SymbolTool = "select" | "line" | "rect" | "circle" | "arc" | "text" | "slot" | "pin";

/** ツール列の並び。 */
export const SYMBOL_TOOLS: SymbolTool[] = [
  "select",
  "line",
  "rect",
  "circle",
  "arc",
  "text",
  "slot",
  "pin",
];

/** ピンの方向 (接続点表の「方向」・方向別追加ボタンの並び)。 */
export const PIN_DIRS: PinDir[] = ["up", "down", "left", "right"];

/** キャンバスで掴めるもの。 */
export type EditorHit =
  | { kind: "pin"; index: number }
  | { kind: "slot"; index: number }
  | { kind: "primitive"; index: number };

/** 保存前チェックの1行 (`ok`=合格 / `warn`=注意 / `error`=保存できない)。 */
export interface SymbolCheck {
  level: "ok" | "warn" | "error";
  /** 安定コード。Rustの検証由来は`symbol.*`、チェックリストの見出し行は`check.*`。 */
  code: string;
  /** 文言に差し込む値 (i18nカタログが使う)。 */
  params: Record<string, string | number>;
}

/** ワールド座標を2.5mmグリッドの交点へ丸める。 */
export function snapToGrid(p: Pt, pitch: number = GRID_PITCH): Point {
  return { x: snapValue(p.x, pitch), y: snapValue(p.y, pitch) };
}

/** 数値1つをグリッドへ丸める (`-0`は`0`に正規化する)。 */
export function snapValue(v: number, pitch: number = GRID_PITCH): number {
  const snapped = Math.round(v / pitch) * pitch;
  return snapped === 0 ? 0 : snapped;
}

/** 値がグリッドに乗っているか (Rust側`on_grid`と同じ許容差)。 */
export function onGrid(v: number, pitch: number = GRID_PITCH): boolean {
  return Math.abs(v - Math.round(v / pitch) * pitch) < 1e-6;
}

/** シンボルのローカル外形 (図形+接続点を含む最小矩形)。空なら原点だけの矩形。 */
export function draftBounds(def: SymbolDef): { min: Point; max: Point } {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  const visit = (x: number, y: number) => {
    minX = Math.min(minX, x);
    minY = Math.min(minY, y);
    maxX = Math.max(maxX, x);
    maxY = Math.max(maxY, y);
  };
  for (const prim of def.primitives) {
    switch (prim.type) {
      case "line":
        prim.pts.forEach((p) => visit(p.x, p.y));
        break;
      case "circle":
      case "arc":
        visit(prim.center.x - prim.r, prim.center.y - prim.r);
        visit(prim.center.x + prim.r, prim.center.y + prim.r);
        break;
      case "rect":
        visit(prim.p1.x, prim.p1.y);
        visit(prim.p2.x, prim.p2.y);
        break;
      case "text":
        visit(prim.at.x, prim.at.y);
        break;
    }
  }
  for (const pin of def.pins) visit(pin.at.x, pin.at.y);
  if (minX > maxX) return { min: { x: 0, y: 0 }, max: { x: 0, y: 0 } };
  return { min: { x: minX, y: minY }, max: { x: maxX, y: maxY } };
}

function dist(a: Pt, b: Pt): number {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

/** 点と線分の距離。 */
function segmentDistance(p: Pt, a: Pt, b: Pt): number {
  const vx = b.x - a.x;
  const vy = b.y - a.y;
  const len2 = vx * vx + vy * vy;
  if (len2 < 1e-12) return dist(p, a);
  const t = Math.max(0, Math.min(1, ((p.x - a.x) * vx + (p.y - a.y) * vy) / len2));
  return dist(p, { x: a.x + t * vx, y: a.y + t * vy });
}

/** 用紙座標系 (Y下向き) での点の角度 (度・0〜360)。 */
function angleDeg(center: Pt, p: Pt): number {
  const deg = (Math.atan2(p.y - center.y, p.x - center.x) * 180) / Math.PI;
  return (deg + 360) % 360;
}

/** 角度が start→end (時計回り) の範囲に入るか。 */
function withinArc(deg: number, startDeg: number, endDeg: number): boolean {
  const span = (((endDeg - startDeg) % 360) + 360) % 360;
  const offset = (((deg - startDeg) % 360) + 360) % 360;
  return span < 1e-9 || offset <= span;
}

/** 点から図形までの距離 (塗り図形の内側は0)。 */
export function primitiveDistance(prim: Primitive, p: Pt): number {
  switch (prim.type) {
    case "line": {
      let best = Infinity;
      for (let i = 1; i < prim.pts.length; i += 1) {
        best = Math.min(best, segmentDistance(p, prim.pts[i - 1], prim.pts[i]));
      }
      return prim.pts.length === 1 ? dist(p, prim.pts[0]) : best;
    }
    case "circle": {
      const d = dist(p, prim.center);
      if (prim.filled && d <= prim.r) return 0;
      return Math.abs(d - prim.r);
    }
    case "arc": {
      const d = Math.abs(dist(p, prim.center) - prim.r);
      return withinArc(angleDeg(prim.center, p), prim.start_deg, prim.end_deg) ? d : Infinity;
    }
    case "rect": {
      const x1 = Math.min(prim.p1.x, prim.p2.x);
      const x2 = Math.max(prim.p1.x, prim.p2.x);
      const y1 = Math.min(prim.p1.y, prim.p2.y);
      const y2 = Math.max(prim.p1.y, prim.p2.y);
      if (prim.filled && p.x >= x1 && p.x <= x2 && p.y >= y1 && p.y <= y2) return 0;
      const corners: Pt[] = [
        { x: x1, y: y1 },
        { x: x2, y: y1 },
        { x: x2, y: y2 },
        { x: x1, y: y2 },
      ];
      let best = Infinity;
      for (let i = 0; i < 4; i += 1) {
        best = Math.min(best, segmentDistance(p, corners[i], corners[(i + 1) % 4]));
      }
      return best;
    }
    case "text":
      return dist(p, prim.at);
  }
}

/**
 * クリック位置で掴めるものを1つ返す。
 *
 * 優先順は **接続点 → 属性テキスト → 図形**。接続点は図形の線上に置くのが普通なので、
 * 重なっている場所では常に接続点が勝つ (でないと接続点を動かせない)。
 * 図形どうしが重なっている場所では**あとから描いた図形**が勝つ。
 */
export function hitTest(def: SymbolDef, at: Pt, tolMm: number): EditorHit | null {
  for (let i = def.pins.length - 1; i >= 0; i -= 1) {
    if (dist(at, def.pins[i].at) <= tolMm) return { kind: "pin", index: i };
  }
  const slots = def.text_slots ?? [];
  for (let i = slots.length - 1; i >= 0; i -= 1) {
    if (dist(at, slots[i].at) <= tolMm) return { kind: "slot", index: i };
  }
  for (let i = def.primitives.length - 1; i >= 0; i -= 1) {
    if (primitiveDistance(def.primitives[i], at) <= tolMm) return { kind: "primitive", index: i };
  }
  return null;
}

/** 図形を平行移動する (全ての点が同じ量だけ動く。半径・角度は変わらない)。 */
export function translatePrimitive(prim: Primitive, dx: number, dy: number): Primitive {
  const move = (p: Point): Point => ({ x: p.x + dx, y: p.y + dy });
  switch (prim.type) {
    case "line":
      return { type: "line", pts: prim.pts.map(move) };
    case "circle":
      return { ...prim, center: move(prim.center) };
    case "arc":
      return { ...prim, center: move(prim.center) };
    case "rect":
      return { ...prim, p1: move(prim.p1), p2: move(prim.p2) };
    case "text":
      return { ...prim, at: move(prim.at) };
  }
}

/** 次の接続点番号 (いま使っている一番大きい数字の次。数字でない番号は数えない)。 */
export function nextPinNumber(def: SymbolDef): string {
  let max = 0;
  for (const pin of def.pins) {
    if (/^\d+$/.test(pin.number.trim())) max = Math.max(max, Number(pin.number.trim()));
  }
  return String(max + 1);
}

function floorGrid(v: number): number {
  return Math.floor(v / GRID_PITCH) * GRID_PITCH;
}
function ceilGrid(v: number): number {
  return Math.ceil(v / GRID_PITCH) * GRID_PITCH;
}

/**
 * 方向別追加ボタンで足す接続点の位置。
 *
 * 図形の外側1グリッドに、グリッドへ乗せて置く。すでに接続点がある点は避けて
 * 1グリッドずつずらす (同じ点に2つ重ねると保存できないため)。
 */
export function nextPinSpot(def: SymbolDef, dir: PinDir): Point {
  const { min, max } = draftBounds(def);
  const midX = snapValue((min.x + max.x) / 2);
  const midY = snapValue((min.y + max.y) / 2);
  let spot: Point;
  switch (dir) {
    case "left":
      spot = { x: floorGrid(min.x) - GRID_PITCH, y: midY };
      break;
    case "right":
      spot = { x: ceilGrid(max.x) + GRID_PITCH, y: midY };
      break;
    case "up":
      spot = { x: midX, y: floorGrid(min.y) - GRID_PITCH };
      break;
    case "down":
      spot = { x: midX, y: ceilGrid(max.y) + GRID_PITCH };
      break;
  }
  const taken = (p: Point) =>
    def.pins.some((pin) => Math.abs(pin.at.x - p.x) < 1e-6 && Math.abs(pin.at.y - p.y) < 1e-6);
  // 横向きのピンは縦へ、縦向きのピンは横へ逃がす
  const step = dir === "left" || dir === "right" ? { x: 0, y: GRID_PITCH } : { x: GRID_PITCH, y: 0 };
  let guard = 0;
  while (taken(spot) && guard < 200) {
    spot = { x: snapValue(spot.x + step.x), y: snapValue(spot.y + step.y) };
    guard += 1;
  }
  return spot;
}

function error(code: string, params: Record<string, string | number> = {}): SymbolCheck {
  return { level: "error", code, params };
}

/**
 * 保存できるシンボルかを確かめる。**問題は全部返す** (空なら保存してよい)。
 *
 * 判断はRust側`madake_core::symbol_library::validate`と同じ:
 * idの体裁と衝突・名前と参照記号接頭辞・図形0個と潰れた図形・接続点 (0個/番号なし/
 * グリッド外/同じ点の重なり)・属性スロット (TAG必須・知らないキー・重複・文字高0)。
 *
 * `reservedIds`=同梱シンボルのid。ここと衝突すると、そのidを使っている既存図面の
 * 記号が黙って変わってしまうので保存を拒否する。
 */
export function validateSymbol(def: SymbolDef, reservedIds: readonly string[] = []): SymbolCheck[] {
  const out: SymbolCheck[] = [];
  const id = def.id.trim();
  if (!id) {
    out.push(error("symbol.id_empty"));
  } else if (!/^[A-Za-z0-9_-]+$/.test(id)) {
    out.push(error("symbol.id_invalid", { id }));
  } else if (reservedIds.includes(id) || dynamicSymbol(id) !== null) {
    out.push(error("symbol.id_reserved", { id }));
  }
  if (!def.name.trim() && !def.name_ja.trim()) out.push(error("symbol.name_empty"));
  if (!def.ref_prefix.trim()) out.push(error("symbol.ref_prefix_empty"));

  if (def.primitives.length === 0) out.push(error("symbol.no_primitives"));
  def.primitives.forEach((prim, i) => {
    let bad: string | null = null;
    switch (prim.type) {
      case "line":
        if (prim.pts.length < 2) bad = "line";
        break;
      case "circle":
        if (prim.r <= 0) bad = "radius";
        break;
      case "arc":
        if (prim.r <= 0) bad = "radius";
        break;
      case "rect":
        if (Math.abs(prim.p1.x - prim.p2.x) < 1e-9 && Math.abs(prim.p1.y - prim.p2.y) < 1e-9) {
          bad = "rect";
        }
        break;
      case "text":
        if (!prim.text.trim()) bad = "text";
        else if (prim.height <= 0) bad = "height";
        break;
    }
    if (bad) out.push(error("symbol.primitive_invalid", { index: i + 1, reason: bad }));
  });

  if (def.pins.length === 0) out.push(error("symbol.no_pins"));
  const seen = new Set<string>();
  const atPoint = new Map<string, string>();
  const key = (pin: PinDef) =>
    `${Math.round(pin.at.x * 1000)},${Math.round(pin.at.y * 1000)}`;
  for (const pin of def.pins) {
    const number = pin.number.trim();
    if (!number) {
      out.push(error("symbol.pin_number_empty", { x: pin.at.x, y: pin.at.y }));
      continue;
    }
    if (!onGrid(pin.at.x) || !onGrid(pin.at.y)) {
      out.push(error("symbol.pin_off_grid", { number, x: pin.at.x, y: pin.at.y }));
    }
    const point = key(pin);
    if (seen.has(`${number}@${point}`)) {
      out.push(error("symbol.pin_duplicate", { number, x: pin.at.x, y: pin.at.y }));
      continue;
    }
    seen.add(`${number}@${point}`);
    const other = atPoint.get(point);
    if (other !== undefined && other !== number) {
      out.push(error("symbol.pin_overlap", { number, other, x: pin.at.x, y: pin.at.y }));
    } else {
      atPoint.set(point, number);
    }
  }

  const slotKeys: string[] = [];
  for (const slot of def.text_slots ?? []) {
    const slotKey = slot.key.trim().toUpperCase();
    if (!(SLOT_KEYS as readonly string[]).includes(slotKey)) {
      out.push(error("symbol.slot_unknown", { key: slotKey, keys: SLOT_KEYS.join(" / ") }));
      continue;
    }
    if (slotKeys.includes(slotKey)) {
      out.push(error("symbol.slot_duplicate", { key: slotKey }));
      continue;
    }
    if (slot.height <= 0) out.push(error("symbol.slot_height", { key: slotKey }));
    slotKeys.push(slotKey);
  }
  if (!slotKeys.includes(REQUIRED_SLOT)) {
    out.push(error("symbol.slot_missing", { key: REQUIRED_SLOT }));
  }
  return out;
}

/** チェックリストの見出し行がどの検証コードを引き受けるか。 */
const ROW_CODES: Record<string, string[]> = {
  "check.pins_grid": ["symbol.no_pins", "symbol.pin_off_grid"],
  "check.pin_numbers": ["symbol.pin_number_empty", "symbol.pin_duplicate", "symbol.pin_overlap"],
  "check.slot_required": ["symbol.slot_missing"],
};

/**
 * 右パネルの「保存前チェック」。
 *
 * 先頭4行は必ず出す (グリッド/番号の重複/必須属性/任意属性)。その4行が引き受けない
 * 指摘は5行目以降にそのまま並べる。任意属性の未配置だけは注意 (`warn`) で、保存できる。
 */
export function symbolChecks(
  def: SymbolDef,
  reservedIds: readonly string[] = [],
): SymbolCheck[] {
  const issues = validateSymbol(def, reservedIds);
  const taken = new Set<SymbolCheck>();
  const rowFor = (code: string, params: Record<string, string | number>): SymbolCheck => {
    const owned = issues.filter((i) => ROW_CODES[code].includes(i.code));
    owned.forEach((i) => taken.add(i));
    if (owned.length === 0) return { level: "ok", code, params };
    return { level: "error", code, params: { ...params, reason: owned[0].code } };
  };

  const placed = new Set((def.text_slots ?? []).map((s) => s.key.trim().toUpperCase()));
  const missing = OPTIONAL_SLOTS.filter((k) => !placed.has(k));
  const rows: SymbolCheck[] = [
    rowFor("check.pins_grid", { count: def.pins.length }),
    rowFor("check.pin_numbers", { count: def.pins.length }),
    rowFor("check.slot_required", { key: REQUIRED_SLOT }),
    missing.length === 0
      ? { level: "ok", code: "check.slot_optional", params: { keys: OPTIONAL_SLOTS.join(" / ") } }
      : { level: "warn", code: "check.slot_optional", params: { keys: missing.join(" / ") } },
  ];
  return [...rows, ...issues.filter((i) => !taken.has(i))];
}

/** エラーが1つも無ければ保存してよい (注意は保存できる)。 */
export function symbolCanSave(rows: readonly SymbolCheck[]): boolean {
  return !rows.some((r) => r.level === "error");
}
