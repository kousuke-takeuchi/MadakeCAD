// Canvas2Dレンダラ本体。純関数でDOM非依存(ctxとviewportを受け取るだけ)。
// 描画順: 背景 → グリッド → JIS図枠 → 配線 → シンボル → ジャンクション → ラベル → 選択

import type { Point, Sheet, SymbolDef, SymbolInstance } from "../ipc";
import { theme, wireColorScreen } from "./theme";
import { GRID_PITCH, type Viewport } from "./viewport";

/** 図枠の用紙端からのマージン (mm)。svg.rsのFRAME_MARGINと一致させること。 */
const FRAME_MARGIN = 10;

export interface RenderOptions {
  selection: Set<string>;
  /** クロスヘア位置(スクリーンpx)。nullで非表示。 */
  cursor: { x: number; y: number } | null;
}

export function paperSizeMm(sheet: Sheet): { w: number; h: number } {
  const dims: Record<string, [number, number]> = {
    A4: [297, 210],
    A3: [420, 297],
    A2: [594, 420],
    A1: [841, 594],
    A0: [1189, 841],
  };
  const [w, h] = dims[sheet.size] ?? [420, 297];
  return sheet.orientation === "Portrait" ? { w: h, h: w } : { w, h };
}

/** シンボルローカル座標→用紙座標。netlist.rs transform_localと同一の変換。 */
export function transformLocal(p: Point, inst: SymbolInstance): Point {
  const x0 = inst.mirror ? -p.x : p.x;
  const y0 = p.y;
  let rx = x0;
  let ry = y0;
  switch (inst.rotation % 360) {
    case 90:
      rx = -y0;
      ry = x0;
      break;
    case 180:
      rx = -x0;
      ry = -y0;
      break;
    case 270:
      rx = y0;
      ry = -x0;
      break;
  }
  return { x: inst.at.x + rx, y: inst.at.y + ry };
}

function drawGrid(ctx: CanvasRenderingContext2D, vp: Viewport, w: number, h: number, pw: number, ph: number) {
  // ズームが小さいときは間引く
  let pitch = GRID_PITCH;
  while (pitch * vp.scale < 6) pitch *= 2;
  ctx.fillStyle = theme.grid;
  for (let gx = 0; gx <= pw; gx += pitch) {
    for (let gy = 0; gy <= ph; gy += pitch) {
      const s = vp.toScreen({ x: gx, y: gy });
      if (s.x < -2 || s.y < -2 || s.x > w + 2 || s.y > h + 2) continue;
      ctx.fillRect(s.x - 0.5, s.y - 0.5, 1, 1);
    }
  }
}

function line(ctx: CanvasRenderingContext2D, vp: Viewport, a: Point, b: Point) {
  const sa = vp.toScreen(a);
  const sb = vp.toScreen(b);
  ctx.moveTo(sa.x, sa.y);
  ctx.lineTo(sb.x, sb.y);
}

function drawFrame(ctx: CanvasRenderingContext2D, vp: Viewport, sheet: Sheet) {
  const { w: pw, h: ph } = paperSizeMm(sheet);
  const x0 = FRAME_MARGIN;
  const y0 = FRAME_MARGIN;
  const x1 = pw - FRAME_MARGIN;
  const y1 = ph - FRAME_MARGIN;

  // 用紙外形 (点線)
  ctx.strokeStyle = theme.dim;
  ctx.lineWidth = 1;
  ctx.setLineDash([4, 4]);
  const p0 = vp.toScreen({ x: 0, y: 0 });
  const p1 = vp.toScreen({ x: pw, y: ph });
  ctx.strokeRect(p0.x, p0.y, p1.x - p0.x, p1.y - p0.y);
  ctx.setLineDash([]);

  // 図枠
  ctx.strokeStyle = theme.line;
  ctx.lineWidth = Math.max(1, 0.5 * vp.scale);
  const f0 = vp.toScreen({ x: x0, y: y0 });
  const f1 = vp.toScreen({ x: x1, y: y1 });
  ctx.strokeRect(f0.x, f0.y, f1.x - f0.x, f1.y - f0.y);

  // ゾーン番号
  const fontPx = Math.max(8, 3 * vp.scale);
  ctx.fillStyle = theme.dim;
  ctx.font = `${fontPx}px sans-serif`;
  ctx.textAlign = "center";
  const cols = Math.max(1, sheet.zone_cols);
  const rows = Math.max(1, sheet.zone_rows);
  const zw = (x1 - x0) / cols;
  const zh = (y1 - y0) / rows;
  ctx.beginPath();
  for (let i = 0; i < cols; i++) {
    const cx = x0 + zw * (i + 0.5);
    const top = vp.toScreen({ x: cx, y: y0 - 4 });
    const bottom = vp.toScreen({ x: cx, y: y1 + 6.5 });
    ctx.fillText(String(i + 1), top.x, top.y);
    ctx.fillText(String(i + 1), bottom.x, bottom.y);
    if (i > 0) {
      line(ctx, vp, { x: x0 + zw * i, y: y0 - 2 }, { x: x0 + zw * i, y: y0 });
      line(ctx, vp, { x: x0 + zw * i, y: y1 }, { x: x0 + zw * i, y: y1 + 2 });
    }
  }
  for (let i = 0; i < rows; i++) {
    const cy = y0 + zh * (i + 0.5) + 1;
    const letter = String.fromCharCode(65 + (i % 26));
    const left = vp.toScreen({ x: x0 - 4, y: cy });
    const right = vp.toScreen({ x: x1 + 4, y: cy });
    ctx.fillText(letter, left.x, left.y);
    ctx.fillText(letter, right.x, right.y);
  }
  ctx.strokeStyle = theme.dim;
  ctx.lineWidth = 1;
  ctx.stroke();

  // 表題欄 (120x32mm、右下)
  const tw = 120;
  const th = 32;
  const tx = x1 - tw;
  const ty = y1 - th;
  const t0 = vp.toScreen({ x: tx, y: ty });
  const t1 = vp.toScreen({ x: x1, y: y1 });
  ctx.strokeStyle = theme.line;
  ctx.strokeRect(t0.x, t0.y, t1.x - t0.x, t1.y - t0.y);
  ctx.beginPath();
  for (let r = 1; r < 4; r++) {
    line(ctx, vp, { x: tx, y: ty + 8 * r }, { x: x1, y: ty + 8 * r });
  }
  line(ctx, vp, { x: tx + 24, y: ty }, { x: tx + 24, y: y1 });
  ctx.lineWidth = 1;
  ctx.stroke();
  const tb = sheet.title_block;
  const rows4: [string, string][] = [
    ["図番", `${tb.drawing_no ?? ""}  Rev ${tb.rev || "-"}`],
    ["品名", tb.title ?? ""],
    ["尺度", `${tb.scale ?? ""}    日付 ${tb.date ?? ""}`],
    ["設計", `${tb.designed ?? ""}  製図 ${tb.drawn ?? ""}  検図 ${tb.checked ?? ""}  承認 ${tb.approved ?? ""}`],
  ];
  ctx.fillStyle = theme.line;
  ctx.textAlign = "left";
  ctx.font = `${Math.max(8, 2.8 * vp.scale)}px sans-serif`;
  rows4.forEach(([label, value], i) => {
    const ly = ty + 8 * i + 5.5;
    const lp = vp.toScreen({ x: tx + 2, y: ly });
    const vpos = vp.toScreen({ x: tx + 26, y: ly });
    ctx.fillText(label, lp.x, lp.y);
    ctx.fillText(value, vpos.x, vpos.y);
  });
}

function drawSymbol(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  inst: SymbolInstance,
  def: SymbolDef,
  selected: boolean,
) {
  ctx.strokeStyle = selected ? theme.selection : theme.line;
  ctx.fillStyle = ctx.strokeStyle;
  ctx.lineWidth = Math.max(1, 0.3 * vp.scale);
  for (const prim of def.primitives) {
    switch (prim.type) {
      case "line": {
        ctx.beginPath();
        prim.pts.forEach((p, i) => {
          const s = vp.toScreen(transformLocal(p, inst));
          if (i === 0) ctx.moveTo(s.x, s.y);
          else ctx.lineTo(s.x, s.y);
        });
        ctx.stroke();
        break;
      }
      case "circle": {
        const c = vp.toScreen(transformLocal(prim.center, inst));
        ctx.beginPath();
        ctx.arc(c.x, c.y, prim.r * vp.scale, 0, Math.PI * 2);
        if (prim.filled) ctx.fill();
        else ctx.stroke();
        break;
      }
      case "arc": {
        const c = vp.toScreen(transformLocal(prim.center, inst));
        ctx.beginPath();
        ctx.arc(
          c.x,
          c.y,
          prim.r * vp.scale,
          (prim.start_deg * Math.PI) / 180,
          (prim.end_deg * Math.PI) / 180,
        );
        ctx.stroke();
        break;
      }
      case "rect": {
        const corners: Point[] = [
          { x: prim.p1.x, y: prim.p1.y },
          { x: prim.p2.x, y: prim.p1.y },
          { x: prim.p2.x, y: prim.p2.y },
          { x: prim.p1.x, y: prim.p2.y },
        ];
        ctx.beginPath();
        corners.forEach((p, i) => {
          const s = vp.toScreen(transformLocal(p, inst));
          if (i === 0) ctx.moveTo(s.x, s.y);
          else ctx.lineTo(s.x, s.y);
        });
        ctx.closePath();
        if (prim.filled) ctx.fill();
        else ctx.stroke();
        break;
      }
      case "text": {
        const p = vp.toScreen(transformLocal(prim.at, inst));
        ctx.font = `${Math.max(8, prim.height * vp.scale)}px sans-serif`;
        ctx.textAlign = "center";
        ctx.fillText(prim.text, p.x, p.y + (prim.height * vp.scale) / 2.8);
        break;
      }
    }
  }
  // 参照記号・型番
  ctx.fillStyle = selected ? theme.selection : theme.annotation;
  ctx.font = `${Math.max(9, 2.5 * vp.scale)}px monospace`;
  ctx.textAlign = "center";
  if (inst.reference) {
    const p = vp.toScreen({ x: inst.at.x, y: inst.at.y - 9 });
    ctx.fillText(inst.reference, p.x, p.y);
  }
  if (inst.value) {
    const p = vp.toScreen({ x: inst.at.x, y: inst.at.y - 5.5 });
    ctx.fillText(inst.value, p.x, p.y);
  }
}

export function renderSheet(
  ctx: CanvasRenderingContext2D,
  sheet: Sheet,
  symbols: SymbolDef[],
  vp: Viewport,
  opts: RenderOptions,
): void {
  const w = ctx.canvas.width;
  const h = ctx.canvas.height;
  ctx.fillStyle = theme.background;
  ctx.fillRect(0, 0, w, h);

  const { w: pw, h: ph } = paperSizeMm(sheet);
  drawGrid(ctx, vp, w, h, pw, ph);
  drawFrame(ctx, vp, sheet);

  const defs = new Map(symbols.map((d) => [d.id, d]));
  const entities = Object.values(sheet.entities);

  for (const e of entities) {
    if (e.kind !== "wire") continue;
    const selected = opts.selection.has(e.id);
    ctx.strokeStyle = selected ? theme.selection : wireColorScreen(e.color);
    ctx.lineWidth = Math.max(1.2, 0.35 * vp.scale) + (selected ? 1.5 : 0);
    ctx.beginPath();
    e.points.forEach((p, i) => {
      const s = vp.toScreen(p);
      if (i === 0) ctx.moveTo(s.x, s.y);
      else ctx.lineTo(s.x, s.y);
    });
    ctx.stroke();
  }
  for (const e of entities) {
    if (e.kind !== "symbol") continue;
    const def = defs.get(e.symbol_id);
    if (def) drawSymbol(ctx, vp, e, def, opts.selection.has(e.id));
  }
  for (const e of entities) {
    if (e.kind === "junction") {
      const s = vp.toScreen(e.at);
      ctx.fillStyle = theme.line;
      ctx.beginPath();
      ctx.arc(s.x, s.y, Math.max(2, 0.6 * vp.scale), 0, Math.PI * 2);
      ctx.fill();
    } else if (e.kind === "net_label") {
      const s = vp.toScreen(e.at);
      ctx.fillStyle = theme.annotation;
      ctx.font = `${Math.max(9, 2.5 * vp.scale)}px monospace`;
      ctx.textAlign = "left";
      ctx.fillText(e.name, s.x, s.y - 2);
    } else if (e.kind === "text") {
      const s = vp.toScreen(e.at);
      ctx.fillStyle = theme.line;
      ctx.font = `${Math.max(9, e.height * vp.scale)}px sans-serif`;
      ctx.textAlign = "left";
      ctx.fillText(e.text, s.x, s.y);
    }
  }

  // クロスヘア
  if (opts.cursor) {
    ctx.strokeStyle = theme.crosshair;
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, opts.cursor.y);
    ctx.lineTo(w, opts.cursor.y);
    ctx.moveTo(opts.cursor.x, 0);
    ctx.lineTo(opts.cursor.x, h);
    ctx.stroke();
    ctx.strokeStyle = theme.line;
    ctx.strokeRect(opts.cursor.x - 5, opts.cursor.y - 5, 10, 10);
  }
}
