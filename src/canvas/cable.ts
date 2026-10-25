// 多芯ケーブル (M4 §5、.pen「M4残りデザイン④」`SWqFJ`) の作図領域側。
//
// 芯線の割当 (定義線が横切った配線を交差順に) はRust側 (madake-core/src/cable.rs と
// `add_cable` コマンド) が正。ここは「定義線ツールが送るコマンド」「注記の文字列」
// 「シールド楕円の形」「描画」「当たり判定」だけを持ち、注記とシールドの形はSVG出力
// (svg.rs) と同じ規則で作る。

import type { Cable, CableCore, Command, CoreEnd, Entity, Point, Sheet, SymbolDef, Wire } from "../ipc";
import { resolveInstanceDef, transformLocal } from "./dynamicSymbol";
import { theme } from "./theme";
import type { Viewport } from "./viewport";
import { drawingText, setDrawingFont } from "./textSize";

/** ケーブルの参照記号の接頭辞 (ハーネスと同じ W 系)。 */
export const CABLE_PREFIX = "W";
/** 定義線とみなす最短の長さ mm (クリックしただけでは作らない)。 */
const MIN_LENGTH = 2.5;
/** 注記の文字高 mm (SVGと同じ)。 */
const CABLE_FONT = 2.5;
/** 区間の線の太さ mm (Rust `RUN_STROKE_MM`)。 */
const RUN_STROKE = 0.7;
/** 区間の斜線の印の半分の長さ mm (Rust `RUN_SLASH_HALF_MM`)。 */
const RUN_SLASH_HALF = 1.5;
/** 分岐の配線の端に添える芯の呼び名の文字高 mm (Rust `CORE_LABEL_MM`)。 */
const CORE_LABEL = 1.5;

/** 多芯ケーブルのエンティティ (区間の無い "cable" と、区間のある "run_cable")。 */
export type CableEntity = Extract<Entity, { kind: "cable" | "run_cable" }>;

/** ケーブルか (区間の有無を問わない)。 */
export function isCable(e: Entity | null | undefined): e is CableEntity {
  return e?.kind === "cable" || e?.kind === "run_cable";
}

/** 1本線の区間を持つか。 */
export function isRunCable(c: Cable): boolean {
  return (c.run?.length ?? 0) > 0;
}

/** ケーブルの全頂点 (定義線と区間)。範囲選択・外接矩形・移動のプレビューに使う。 */
export function cablePoints(c: Cable): Point[] {
  return [...c.points, ...(c.run ?? [])];
}

/** 芯の行き先がピンなら "USB1.2" (部品が消えていれば "?.2")、分岐の配線ならnull。 */
export function coreEndPin(sheet: Sheet, end: CoreEnd): string | null {
  if (end.kind !== "pin") return null;
  const part = sheet.entities[end.part];
  const ref = part && part.kind === "symbol" && part.reference ? part.reference : "?";
  return `${ref}.${end.pin}`;
}

/** 区間のいちばん長い線分の中点 (同じ長さなら後ろの線分。Rust `run_mid` と同じ)。 */
export function runMid(c: Cable): Point | null {
  const run = c.run ?? [];
  let best: Point | null = null;
  let bestLen = -1;
  for (let i = 0; i + 1 < run.length; i++) {
    const a = run[i];
    const b = run[i + 1];
    const len = Math.hypot(b.x - a.x, b.y - a.y);
    if (len >= bestLen) {
      bestLen = len;
      best = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
    }
  }
  return best;
}

/** 単線図の印: 中点を通る45度の短い斜線 (Rust `run_slash`)。 */
export function runSlash(c: Cable): [Point, Point] | null {
  const m = runMid(c);
  return m ? [{ x: m.x - RUN_SLASH_HALF, y: m.y + RUN_SLASH_HALF }, { x: m.x + RUN_SLASH_HALF, y: m.y - RUN_SLASH_HALF }] : null;
}

/** 芯数の文字の位置 (Rust `run_count_at`)。 */
export function runCountAt(c: Cable): Point | null {
  const m = runMid(c);
  return m ? { x: m.x + 2, y: m.y - 2 } : null;
}

/** ケーブル注記の位置 (Rust `run_label_at`)。 */
export function runLabelAt(c: Cable): Point | null {
  const m = runMid(c);
  return m ? { x: m.x + 3.5, y: m.y + 4 } : null;
}

/** 区間の端の点 (a = 最初の点、b = 最後の点)。 */
export function runEndPoint(c: Cable, end: "a" | "b"): Point | null {
  const run = c.run ?? [];
  return (end === "a" ? run[0] : run[run.length - 1]) ?? null;
}

/**
 * 帳票・凡例に載せる配線の仕様 (線色・線径・品番)。区間ケーブルの分岐の配線は自分では持たないので、
 * 空の項目を芯の線色・ケーブルの線径・型番で補う (Rust `cable::effective_wire`)。
 */
export function effectiveWire(sheet: Sheet, w: Wire): Wire {
  for (const e of Object.values(sheet.entities)) {
    if (e.kind !== "run_cable" || !isRunCable(e)) continue;
    for (const core of e.cores) {
      if (core.end_a?.kind !== "wire" || core.end_a.wire !== w.id) {
        if (core.end_b?.kind !== "wire" || core.end_b.wire !== w.id) continue;
      }
      const own = (w.part_no ?? "").trim();
      return {
        ...w,
        color: (w.color ?? "").trim() ? w.color : core.color,
        sq: (w.sq ?? 0) > 0 ? w.sq : e.sq,
        part_no: own || e.part_no.trim() || null,
      };
    }
  }
  return w;
}

function coreEnd(core: CableCore, end: "a" | "b"): CoreEnd | null | undefined {
  return end === "a" ? core.end_a : core.end_b;
}

/** ピンの行き先の用紙上の位置 (`index`番目、省略=最初の接続点。Rust `pin_end_position`)。 */
export function pinEndPosition(sheet: Sheet, end: CoreEnd, defs: ReadonlyMap<string, SymbolDef>): Point | null {
  if (end.kind !== "pin") return null;
  const part = sheet.entities[end.part];
  if (!part || part.kind !== "symbol") return null;
  const def = resolveInstanceDef(part, defs);
  const pins = (def?.pins ?? []).filter((p) => p.number === end.pin);
  const pin = pins[Math.max(0, (end.index ?? 1) - 1)];
  return pin ? transformLocal(pin.at, part) : null;
}

/** 棒の両端: 上→下、同じ高さなら左→右に並べた最初と最後 (Rust `bar_ends`)。 */
export function barEnds(positions: Point[]): [Point, Point] | null {
  if (positions.length === 0) return null;
  const sorted = [...positions].sort((a, b) => a.y - b.y || a.x - b.x);
  return [sorted[0], sorted[sorted.length - 1]];
}

/** 部品に着く端の棒 (Rust `run_bars`)。 */
export function runBars(sheet: Sheet, c: Cable, defs: ReadonlyMap<string, SymbolDef>): [Point, Point][] {
  const out: [Point, Point][] = [];
  for (const end of ["a", "b"] as const) {
    const positions = c.cores
      .map((k) => coreEnd(k, end))
      .filter((e): e is CoreEnd => !!e)
      .map((e) => pinEndPosition(sheet, e, defs))
      .filter((p): p is Point => p !== null);
    const bar = barEnds(positions);
    if (bar && Math.hypot(bar[1].x - bar[0].x, bar[1].y - bar[0].y) > 1e-6) out.push(bar);
  }
  return out;
}

/** 分岐側の端点: 芯の配線の端点のうち区間の端に近い方 (Rust `breakout_point`)。 */
export function breakoutPoint(sheet: Sheet, c: Cable, end: "a" | "b", wireId: string): Point | null {
  const at = runEndPoint(c, end);
  const w = sheet.entities[wireId];
  if (!at || !w || w.kind !== "wire" || w.points.length === 0) return null;
  const first = w.points[0];
  const last = w.points[w.points.length - 1];
  return Math.hypot(first.x - at.x, first.y - at.y) <= Math.hypot(last.x - at.x, last.y - at.y) ? first : last;
}

/** 分岐の扇: 区間の端から各芯の配線の分岐側の端へ (Rust `run_fans`)。 */
export function runFans(sheet: Sheet, c: Cable): { from: Point; to: Point; core: string }[] {
  const out: { from: Point; to: Point; core: string }[] = [];
  for (const end of ["a", "b"] as const) {
    const from = runEndPoint(c, end);
    if (!from) continue;
    for (const core of c.cores) {
      const e = coreEnd(core, end);
      if (e?.kind !== "wire") continue;
      const to = breakoutPoint(sheet, c, end, e.wire);
      if (to) out.push({ from, to, core: core.core });
    }
  }
  return out;
}

/** 数値の表記 (小数第2位まで、末尾の0と小数点は書かない)。 */
function num(v: number): string {
  return v.toFixed(2).replace(/0+$/, "").replace(/\.$/, "");
}

/** 定義線に添える注記 (Rust `cable_label` と同じ: 参照記号・型番・芯数×線径・長さ)。 */
export function cableLabel(c: Cable): string {
  const spec = c.sq > 0 ? `${c.core_count}C×${num(c.sq)}sq` : `${c.core_count}C`;
  return [c.reference, c.part_no.trim(), spec, c.length_m != null ? `${num(c.length_m)}m` : ""]
    .filter((s) => s !== "")
    .join("  ");
}

/** シールドの楕円 (Rust `shield_ellipse` と同じ): 中心・長半径・短半径・向き [rad]。区間ケーブルは斜線の印を囲む。 */
export function shieldEllipse(c: Cable): { center: Point; rx: number; ry: number; angle: number } | null {
  if (isRunCable(c)) {
    const m = runMid(c);
    return m ? { center: m, rx: 3, ry: 2, angle: -Math.PI / 4 } : null;
  }
  const [a, b] = c.points;
  if (!a || !b) return null;
  return {
    center: { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 },
    rx: Math.hypot(b.x - a.x, b.y - a.y) / 2 + 2.5,
    ry: 3,
    angle: Math.atan2(b.y - a.y, b.x - a.x),
  };
}

/** シート上の次のケーブル参照記号 (ケーブルとハーネスの W 番号の続き)。 */
export function nextCableReference(sheet: Sheet): string {
  let max = 0;
  for (const e of Object.values(sheet.entities)) {
    const name = isCable(e) ? e.reference : e.kind === "harness" ? e.name : "";
    const m = /^W(\d+)$/.exec(name ?? "");
    if (m) max = Math.max(max, Number(m[1]));
  }
  return `${CABLE_PREFIX}${max + 1}`;
}

/**
 * 定義線のドラッグ → ケーブル追加コマンド。芯数0=横切った本数にする (Rust側で決まる)。
 * 短すぎる線 (クリックしただけ) ではnullを返してコマンドを送らない。
 */
export function cableAddCommand(sheet: Sheet, a: Point, b: Point): Command | null {
  if (Math.hypot(b.x - a.x, b.y - a.y) < MIN_LENGTH) return null;
  return {
    type: "add_cable",
    sheet_id: sheet.id,
    cable: {
      id: crypto.randomUUID(),
      reference: nextCableReference(sheet),
      part_no: "",
      core_count: 0,
      sq: 0,
      points: [a, b],
      cores: [],
      shield: null,
      length_m: null,
    },
  };
}

/** 点から線分までの距離。 */
function segmentDistance(p: Point, a: Point, b: Point): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const len2 = dx * dx + dy * dy;
  const t = len2 === 0 ? 0 : Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / len2));
  return Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy));
}

/** ケーブルを掴めるか (定義線ケーブルは定義線、区間ケーブルは区間の線)。 */
export function cableHit(c: Cable, world: Point, tolerance: number): boolean {
  if (isRunCable(c)) {
    const run = c.run ?? [];
    for (let i = 0; i + 1 < run.length; i++) {
      if (segmentDistance(world, run[i], run[i + 1]) <= tolerance) return true;
    }
    return false;
  }
  const [a, b] = c.points;
  return !!a && !!b && segmentDistance(world, a, b) <= tolerance;
}

/** ケーブルを描く (定義線ケーブルは定義線・シールド楕円・注記、区間ケーブルは drawRun)。選択中は選択色。 */
export function drawCable(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  c: Cable,
  selected: boolean,
  sheet: Sheet,
  defs: ReadonlyMap<string, SymbolDef>,
): void {
  const color = selected ? theme.selection : theme.annotation;
  if (isRunCable(c)) {
    drawRun(ctx, vp, c, color, selected, sheet, defs);
    return;
  }
  const [a, b] = c.points;
  if (!a || !b) return;
  const sa = vp.toScreen(a);
  const sb = vp.toScreen(b);
  ctx.strokeStyle = color;
  ctx.lineWidth = Math.max(1.2, 0.35 * vp.scale) + (selected ? 1 : 0);
  ctx.beginPath();
  ctx.moveTo(sa.x, sa.y);
  ctx.lineTo(sb.x, sb.y);
  ctx.stroke();
  drawShield(ctx, vp, c);
  const top = a.y <= b.y ? a : b;
  const at = vp.toScreen({ x: top.x + 1.5, y: top.y - 1.5 });
  ctx.fillStyle = color;
  setDrawingFont(ctx, CABLE_FONT, vp.scale, "monospace");
  ctx.textAlign = "left";
  ctx.textBaseline = "alphabetic";
  drawingText(ctx, cableLabel(c), at.x, at.y);
}

function drawShield(ctx: CanvasRenderingContext2D, vp: Viewport, c: Cable): void {
  const shield = c.shield ? shieldEllipse(c) : null;
  if (!shield) return;
  const center = vp.toScreen(shield.center);
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.ellipse(center.x, center.y, shield.rx * vp.scale, shield.ry * vp.scale, shield.angle, 0, Math.PI * 2);
  ctx.stroke();
}

/** 区間ケーブル: 太い線・斜線と芯数・部品側の棒・分岐の扇と芯の呼び名・シールド・注記 (SVG の render_run_cable と同じ形)。 */
function drawRun(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  c: Cable,
  color: string,
  selected: boolean,
  sheet: Sheet,
  defs: ReadonlyMap<string, SymbolDef>,
): void {
  const line = (a: Point, b: Point, mm: number) => {
    const sa = vp.toScreen(a);
    const sb = vp.toScreen(b);
    ctx.lineWidth = Math.max(1, mm * vp.scale);
    ctx.beginPath();
    ctx.moveTo(sa.x, sa.y);
    ctx.lineTo(sb.x, sb.y);
    ctx.stroke();
  };
  ctx.strokeStyle = color;
  ctx.fillStyle = color;
  ctx.lineWidth = Math.max(2, RUN_STROKE * vp.scale) + (selected ? 1 : 0);
  ctx.beginPath();
  (c.run ?? []).forEach((p, i) => {
    const s = vp.toScreen(p);
    if (i === 0) ctx.moveTo(s.x, s.y);
    else ctx.lineTo(s.x, s.y);
  });
  ctx.stroke();
  const slash = runSlash(c);
  if (slash) line(slash[0], slash[1], 0.35);
  for (const [a, b] of runBars(sheet, c, defs)) line(a, b, RUN_STROKE);
  ctx.textBaseline = "alphabetic";
  for (const fan of runFans(sheet, c)) {
    line(fan.from, fan.to, 0.35);
    const at = vp.toScreen({ x: fan.to.x, y: fan.to.y - 0.8 });
    setDrawingFont(ctx, CORE_LABEL, vp.scale, "monospace");
    ctx.textAlign = "center";
    drawingText(ctx, fan.core, at.x, at.y);
  }
  const count = runCountAt(c);
  if (count) {
    const at = vp.toScreen(count);
    setDrawingFont(ctx, 2, vp.scale, "monospace");
    ctx.textAlign = "left";
    drawingText(ctx, String(c.core_count), at.x, at.y);
  }
  drawShield(ctx, vp, c);
  const label = runLabelAt(c);
  if (label) {
    const at = vp.toScreen(label);
    setDrawingFont(ctx, CABLE_FONT, vp.scale, "monospace");
    ctx.textAlign = "left";
    drawingText(ctx, cableLabel(c), at.x, at.y);
  }
}
