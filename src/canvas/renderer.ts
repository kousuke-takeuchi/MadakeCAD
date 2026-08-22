// Canvas2Dレンダラ本体。純関数でDOM非依存(ctxとviewportを受け取るだけ)。
// 描画順: 背景 → グリッド → JIS図枠 → 配線 → シンボル → ジャンクション → ラベル → 選択

import type { Point, Sheet, SymbolDef, SymbolInstance } from "../ipc";
import type { Region } from "./agentOverlay";
import {
  DEFAULT_FRAME_TEMPLATE,
  REV_HEADERS,
  bindContextForSheet,
  effectiveRev,
  frameLayout,
  paperSizeMm,
  revisionLayout,
  visibleRevisions,
  type BindContext,
  type CellAlign,
  type FrameLayout,
  type FrameTemplate,
  type RevisionLayout,
  type RevisionRowLayout,
} from "./frameTemplate";
import { resolveSymbolDef } from "./dynamicSymbol";
import { HARNESS_DASH, HARNESS_FONT, harnessLabelAt } from "./harness";
import { entityViewClass, type ViewClass } from "./viewClasses";
import { agentRgba, theme, wireColorScreen } from "./theme";
import { WIRE_NO_FONT, wireNumberLabels } from "./wireNumbers";
import { XREF_FONT, xrefTextAt } from "./xref";
import {
  CONTACT_MAP_FONT,
  coilLocationAt,
  contactMapLayout,
  contactMapOrigin,
  type ContactMapRow,
} from "./relayXref";
import { GRID_PITCH, type Viewport } from "./viewport";

/** セル内テキストの左余白 (mm)。 */
const CELL_PAD = 2;
/** 改訂欄の列幅 (記号 / 日付 / 内容 / 承認) mm。既定の図枠テンプレートのもの。 */
export const REV_COL_W = DEFAULT_FRAME_TEMPLATE.revision_block.col_w;
/** 改訂欄に描く最大行数。溢れた分は古い行から省略する (データは保持)。 */
export const REV_MAX_ROWS = DEFAULT_FRAME_TEMPLATE.revision_block.rows_max;
/** 改訂欄の本文・列見出しの文字高さ (mm)。 */
const REV_FONT = 2.5;
const REV_HEADER_FONT = 2.2;

// 図枠の寸法・表題欄のセルは図枠テンプレート (frameTemplate.ts) が持つ。
// 既存の呼び出し元のために、そこから再輸出する。
export {
  DEFAULT_FRAME_TEMPLATE,
  REV_HEADERS,
  effectiveRev,
  frameLayout,
  paperSizeMm,
  revisionLayout,
  visibleRevisions,
};
export type { FrameTemplate, RevisionLayout, RevisionRowLayout };

export interface RenderOptions {
  selection: Set<string>;
  /** 非表示中の表示クラス (レイヤ、spec §4)。省略時は全表示。 */
  hidden?: ReadonlySet<ViewClass>;
  /** クロスヘア位置(スクリーンpx)。nullで非表示。 */
  cursor: { x: number; y: number } | null;
  /** エージェント編集オーバーレイ。省略時は描かない。 */
  agent?: AgentPaint;
  /**
   * シート間クロスリファレンス (ネットラベルのid → 「/2.B3」等の相手先テキスト)。
   * `sheetXrefs()` の戻り。省略時は描かない。表示クラスは「ネットラベル」に含まれる。
   */
  xrefs?: ReadonlyMap<string, string>;
  /**
   * コイル⇔接点クロスリファレンス (M4 §4)。`sheetContactMaps()` / `sheetCoilLocations()`
   * の戻り。省略時は描かない。表示クラスは「参照記号」に含める。
   */
  relay?: {
    /** コイルのid → その下に描く接点マップの行。 */
    contactMaps?: ReadonlyMap<string, ContactMapRow[]>;
    /** 接点のid → 脇に描くコイル所在 (例 "(/1.C2)")。 */
    coilLocations?: ReadonlyMap<string, string>;
  };
  /** 図枠テンプレート (M4 §9)。省略時は同梱のjis-a3。 */
  frame?: FrameTemplate;
  /** 表題欄バインドの文脈。省略時はシート単体 (1枚中1枚目・プロジェクト名は空)。 */
  bind?: BindContext;
}

/** エージェント編集オーバーレイの描画入力。 */
export interface AgentPaint {
  /** パルス表示する領域(用紙mm)。AgentOverlay.activeRegions()の戻り。 */
  regions: Region[];
  /** ターン進行中か。領域が無くてもラベルチップだけ出す。 */
  active: boolean;
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

/**
 * 弧の開始・終了角をシンボルの回転・ミラーに追従させる
 * (netlist.rs rotate_arc_anglesと同一。描画は常に開始→終了を時計回り)。
 */
export function rotateArcAngles(
  startDeg: number,
  endDeg: number,
  inst: SymbolInstance,
): [number, number] {
  const [s, e] = inst.mirror ? [180 - endDeg, 180 - startDeg] : [startDeg, endDeg];
  const span = e - s;
  const start = (((s + (inst.rotation % 360)) % 360) + 360) % 360;
  return [start, start + span];
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

/** CellAlign → Canvas2Dの textAlign。 */
function canvasAlign(align: CellAlign): CanvasTextAlign {
  return align === "middle" ? "center" : align === "end" ? "right" : "left";
}

/**
 * 図枠・ゾーン目盛・改訂欄・表題欄を描く。寸法と中身は図枠テンプレート
 * (`frameTemplate.ts` = Rust側 frame_template.rs と同じデータ) が決める。
 */
function drawFrame(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  sheet: Sheet,
  frame: FrameTemplate = DEFAULT_FRAME_TEMPLATE,
  bind: BindContext = bindContextForSheet(sheet),
) {
  const l: FrameLayout = frameLayout(sheet, frame, bind);
  const { w: pw, h: ph } = l.paper;
  const { x0, y0, x1, y1 } = l.frame;

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

  // ゾーン目盛 (分割数はシート、文字種はテンプレート)
  const fontPx = Math.max(8, 3 * vp.scale);
  ctx.fillStyle = theme.dim;
  ctx.font = `${fontPx}px sans-serif`;
  ctx.textAlign = "center";
  const { zw, zh, colLabels, rowLabels } = l.zones;
  ctx.beginPath();
  colLabels.forEach((label, i) => {
    const cx = x0 + zw * (i + 0.5);
    const top = vp.toScreen({ x: cx, y: y0 - 4 });
    const bottom = vp.toScreen({ x: cx, y: y1 + 6.5 });
    ctx.fillText(label, top.x, top.y);
    ctx.fillText(label, bottom.x, bottom.y);
    if (i > 0) {
      line(ctx, vp, { x: x0 + zw * i, y: y0 - 2 }, { x: x0 + zw * i, y: y0 });
      line(ctx, vp, { x: x0 + zw * i, y: y1 }, { x: x0 + zw * i, y: y1 + 2 });
    }
  });
  rowLabels.forEach((label, i) => {
    const cy = y0 + zh * (i + 0.5) + 1;
    const left = vp.toScreen({ x: x0 - 4, y: cy });
    const right = vp.toScreen({ x: x1 + 4, y: cy });
    ctx.fillText(label, left.x, left.y);
    ctx.fillText(label, right.x, right.y);
  });
  ctx.strokeStyle = theme.dim;
  ctx.lineWidth = 1;
  ctx.stroke();

  // 改訂欄 (表題欄の直上)
  drawRevisionBlock(ctx, vp, l.revision);

  // 表題欄 (テンプレートの基準隅・寸法・罫線・セル)
  const tb = l.titleBlock;
  const t0 = vp.toScreen({ x: tb.x, y: tb.y });
  const t1 = vp.toScreen({ x: tb.x + tb.w, y: tb.y + tb.h });
  ctx.strokeStyle = theme.line;
  ctx.strokeRect(t0.x, t0.y, t1.x - t0.x, t1.y - t0.y);
  ctx.beginPath();
  for (const g of tb.gridLines) {
    line(ctx, vp, { x: g.x1, y: g.y1 }, { x: g.x2, y: g.y2 });
  }
  ctx.lineWidth = 1;
  ctx.stroke();
  ctx.fillStyle = theme.line;
  for (const cell of tb.cells) {
    ctx.textAlign = canvasAlign(cell.align);
    const bold = cell.weight === "bold" ? "bold " : "";
    ctx.font = `${bold}${Math.max(8, cell.size * vp.scale)}px sans-serif`;
    const p = vp.toScreen({ x: cell.x, y: cell.y });
    ctx.fillText(cell.text, p.x, p.y);
  }
  ctx.textAlign = "left";

  // ロゴ枠 (画像は未対応。場所を示す枠だけ)
  if (l.logo) {
    const a = vp.toScreen({ x: l.logo.x, y: l.logo.y });
    const b = vp.toScreen({ x: l.logo.x + l.logo.w, y: l.logo.y + l.logo.h });
    ctx.strokeStyle = theme.dim;
    ctx.lineWidth = 1;
    ctx.strokeRect(a.x, a.y, b.x - a.x, b.y - a.y);
  }
}

/**
 * 改訂欄 (ISO 7200 / JIS Z 8311) を描く。表題欄の直上に同じ右端・同じ幅で置き、
 * 最下段が列見出し、その上に古い改訂から順に積む (最新が最上段)。0行なら何も描かない。
 * Rust側 svg.rs の revision_block と同一ルール。
 */
function drawRevisionBlock(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  l: RevisionLayout | null,
) {
  if (!l) return;
  const bottom = l.y + l.h;
  const right = l.x + l.w;

  // 外枠
  ctx.strokeStyle = theme.line;
  ctx.lineWidth = Math.max(1, 0.5 * vp.scale);
  const a = vp.toScreen({ x: l.x, y: l.y });
  const b = vp.toScreen({ x: right, y: bottom });
  ctx.strokeRect(a.x, a.y, b.x - a.x, b.y - a.y);

  // 罫線 (行境界+列境界)
  ctx.beginPath();
  for (let r = 1; r <= l.rows.length; r++) {
    const ry = l.y + l.rowH * r;
    line(ctx, vp, { x: l.x, y: ry }, { x: right, y: ry });
  }
  for (const cx of l.colLefts.slice(1)) {
    line(ctx, vp, { x: cx, y: l.y }, { x: cx, y: bottom });
  }
  ctx.lineWidth = 1;
  ctx.stroke();

  ctx.textAlign = "left";
  // 列見出し (最下段、表題欄側)
  ctx.fillStyle = theme.dim;
  ctx.font = `${Math.max(8, REV_HEADER_FONT * vp.scale)}px sans-serif`;
  REV_HEADERS.forEach((label, i) => {
    const p = vp.toScreen({
      x: l.colLefts[i] + CELL_PAD,
      y: l.headerTop + l.rowH / 2 + REV_HEADER_FONT / 2,
    });
    ctx.fillText(label, p.x, p.y);
  });
  // 改訂行
  ctx.fillStyle = theme.line;
  ctx.font = `${Math.max(8, REV_FONT * vp.scale)}px sans-serif`;
  for (const row of l.rows) {
    const cells = [row.rev.mark, row.rev.date, row.rev.description, row.rev.by];
    cells.forEach((value, c) => {
      if (!value) return;
      const p = vp.toScreen({
        x: l.colLefts[c] + CELL_PAD,
        y: row.top + l.rowH / 2 + REV_FONT / 2,
      });
      ctx.fillText(value, p.x, p.y);
    });
  }
}

/** シンボル1個を描画する。colorOverride指定時は配置プレビュー等のゴースト描画用。 */
export function drawSymbol(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  inst: SymbolInstance,
  def: SymbolDef,
  selected: boolean,
  colorOverride?: string,
  /** falseで参照記号・型番の注記を描かない (表示クラス "refs")。 */
  showLabels = true,
) {
  ctx.strokeStyle = colorOverride ?? (selected ? theme.selection : theme.line);
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
        const [a0, a1] = rotateArcAngles(prim.start_deg, prim.end_deg, inst);
        ctx.beginPath();
        ctx.arc(c.x, c.y, prim.r * vp.scale, (a0 * Math.PI) / 180, (a1 * Math.PI) / 180);
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
  if (!showLabels) return;
  // 参照記号・型番: シンボル外形の上に併記(縦長シンボルでも重ならない。Rust側svg.rsと同ルール)
  const top = symbolTopY(inst, def);
  ctx.fillStyle = colorOverride ?? (selected ? theme.selection : theme.annotation);
  ctx.font = `${Math.max(9, 2.5 * vp.scale)}px monospace`;
  ctx.textAlign = "center";
  if (inst.reference) {
    const p = vp.toScreen({ x: inst.at.x, y: top - 4.5 });
    ctx.fillText(inst.reference, p.x, p.y);
  }
  if (inst.value) {
    const p = vp.toScreen({ x: inst.at.x, y: top - 1 });
    ctx.fillText(inst.value, p.x, p.y);
  }
}

/**
 * コイルの下に接点マップ (端子対 | 所在) の表を描く (M4 §4)。
 * 位置・列幅は relayXref.contactMapLayout が決め、SVG出力 (svg.rs) と同じ数式。
 */
export function drawContactMap(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  inst: SymbolInstance,
  def: SymbolDef,
  rows: ContactMapRow[],
) {
  if (!rows.length) return;
  const origin = contactMapOrigin(symbolBounds(inst, def), inst.at.x);
  const layout = contactMapLayout(rows, origin.x, origin.y);
  const topLeft = vp.toScreen({ x: layout.x, y: layout.y });
  const bottomRight = vp.toScreen({
    x: layout.x + layout.width,
    y: layout.y + layout.height,
  });
  ctx.strokeStyle = theme.xref;
  ctx.lineWidth = 1;
  ctx.strokeRect(
    topLeft.x,
    topLeft.y,
    bottomRight.x - topLeft.x,
    bottomRight.y - topLeft.y,
  );
  ctx.beginPath();
  const divider = vp.toScreen({ x: layout.colX(1), y: layout.y });
  ctx.moveTo(divider.x, topLeft.y);
  ctx.lineTo(divider.x, bottomRight.y);
  for (let i = 1; i < rows.length; i += 1) {
    const y = vp.toScreen({ x: layout.x, y: layout.rowTop(i) }).y;
    ctx.moveTo(topLeft.x, y);
    ctx.lineTo(bottomRight.x, y);
  }
  ctx.stroke();
  ctx.fillStyle = theme.xref;
  ctx.font = `${Math.max(8, CONTACT_MAP_FONT * vp.scale)}px monospace`;
  ctx.textAlign = "left";
  rows.forEach((row, i) => {
    const y = vp.toScreen({ x: layout.x, y: layout.baseline(i) }).y;
    ctx.fillText(row.terminals, vp.toScreen({ x: layout.textX(0), y: 0 }).x, y);
    ctx.fillText(row.address, vp.toScreen({ x: layout.textX(1), y: 0 }).x, y);
  });
}

/** 配置後のシンボル外形の上端Y(用紙座標)。 */
export function symbolTopY(inst: SymbolInstance, def: SymbolDef): number {
  return symbolBounds(inst, def).min.y;
}

/**
 * 配置後のシンボル外形の囲み矩形(用紙座標)。円・弧は回転対称なので中心±rで安全側に
 * 評価し、配置基準点も必ず含める。Rust側 svg.rs の symbol_bounds と同一。
 * 参照記号・接点マップなど、外形の外側に置く注記の位置決めに使う。
 */
export function symbolBounds(
  inst: SymbolInstance,
  def: SymbolDef,
): { min: Point; max: Point } {
  const min = { ...inst.at };
  const max = { ...inst.at };
  const visit = (p: Point) => {
    const t = transformLocal(p, inst);
    min.x = Math.min(min.x, t.x);
    min.y = Math.min(min.y, t.y);
    max.x = Math.max(max.x, t.x);
    max.y = Math.max(max.y, t.y);
  };
  for (const prim of def.primitives) {
    switch (prim.type) {
      case "line":
        prim.pts.forEach(visit);
        break;
      case "circle":
      case "arc":
        visit({ x: prim.center.x - prim.r, y: prim.center.y - prim.r });
        visit({ x: prim.center.x + prim.r, y: prim.center.y + prim.r });
        break;
      case "rect":
        visit(prim.p1);
        visit({ x: prim.p2.x, y: prim.p1.y });
        visit(prim.p2);
        visit({ x: prim.p1.x, y: prim.p2.y });
        break;
      case "text":
        visit(prim.at);
        break;
    }
  }
  for (const pin of def.pins) visit(pin.at);
  return { min, max };
}

// --- エージェント編集オーバーレイ -------------------------------------------

/** 領域の角丸 (px)。デザイン: border-radius 8px。 */
const OVERLAY_RADIUS = 8;
const CHIP_LABEL = "⚡ エージェントが編集中...";
const CHIP_FONT = '600 11px "Noto Sans JP", system-ui, sans-serif';
const CHIP_PAD_X = 9;
const CHIP_PAD_Y = 5;
const CHIP_HEIGHT = 11 + CHIP_PAD_Y * 2;
const CHIP_INSET = 4;

function roundRectPath(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
) {
  const rr = Math.max(0, Math.min(r, w / 2, h / 2));
  ctx.beginPath();
  ctx.moveTo(x + rr, y);
  ctx.lineTo(x + w - rr, y);
  ctx.arcTo(x + w, y, x + w, y + rr, rr);
  ctx.lineTo(x + w, y + h - rr);
  ctx.arcTo(x + w, y + h, x + w - rr, y + h, rr);
  ctx.lineTo(x + rr, y + h);
  ctx.arcTo(x, y + h, x, y + h - rr, rr);
  ctx.lineTo(x, y + rr);
  ctx.arcTo(x, y, x + rr, y, rr);
  ctx.closePath();
}

function drawAgentChip(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  alpha: number,
  color: string,
) {
  ctx.font = CHIP_FONT;
  ctx.textAlign = "left";
  const textW = ctx.measureText(CHIP_LABEL).width;
  const w = textW + CHIP_PAD_X * 2;
  ctx.fillStyle = agentRgba(alpha, color);
  roundRectPath(ctx, x, y, w, CHIP_HEIGHT, CHIP_HEIGHT / 2);
  ctx.fill();
  ctx.fillStyle = theme.agentInk;
  ctx.fillText(CHIP_LABEL, x + CHIP_PAD_X, y + CHIP_HEIGHT / 2 + 4);
}

/** チップの幅 (px)。fontを設定した状態で呼ぶこと。 */
function chipWidth(ctx: CanvasRenderingContext2D): number {
  ctx.font = CHIP_FONT;
  return ctx.measureText(CHIP_LABEL).width + CHIP_PAD_X * 2;
}

/** 編集中領域のパルスとラベルチップを、既存の描画の上に重ねる。 */
export function drawAgentOverlay(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  paint: AgentPaint,
): void {
  const { regions, active } = paint;
  if (!active && regions.length === 0) return;
  const w = ctx.canvas.width;
  const h = ctx.canvas.height;

  // 会話ごとに色が違う (並列エージェント)。チップは代表領域の色に合わせる
  let primary: { x: number; y: number; w: number; h: number; color: string } | null = null;
  for (const region of regions) {
    const a = vp.toScreen(region.min);
    const b = vp.toScreen(region.max);
    const color = region.color || theme.agent;
    const rect = { x: a.x, y: a.y, w: b.x - a.x, h: b.y - a.y, color };
    ctx.fillStyle = agentRgba(region.opacity * region.strength, color);
    roundRectPath(ctx, rect.x, rect.y, rect.w, rect.h, OVERLAY_RADIUS);
    ctx.fill();
    ctx.strokeStyle = agentRgba(region.strength, color);
    ctx.lineWidth = 1.5;
    ctx.stroke();
    // ラベルは最も上(同率なら左)の領域に添える
    if (!primary || rect.y < primary.y || (rect.y === primary.y && rect.x < primary.x)) {
      primary = rect;
    }
  }

  if (!active) return;
  const cw = chipWidth(ctx);
  let cx: number;
  let cy: number;
  if (primary && primary.w >= cw + CHIP_INSET * 2 && primary.h >= CHIP_HEIGHT + CHIP_INSET * 2) {
    cx = primary.x + CHIP_INSET;
    cy = primary.y + CHIP_INSET;
  } else if (primary) {
    cx = primary.x;
    cy = primary.y - CHIP_HEIGHT - CHIP_INSET;
  } else {
    cx = (w - cw) / 2;
    cy = 16;
  }
  // 画面外へはみ出す場合はキャンバス上部中央へ逃がす
  if (cx < 4 || cy < 4 || cx + cw > w - 4 || cy + CHIP_HEIGHT > h - 4) {
    cx = Math.max(4, (w - cw) / 2);
    cy = 16;
  }
  drawAgentChip(ctx, cx, cy, 0.92, primary?.color ?? theme.agent);
}

/**
 * 線番 (IEC 62491) を描く。位置は wireNumberLabels が決める
 * (ネットごとに最長線分の中点、横線は上・縦線は左へ2.5mm)。Rust側svg.rsと同一ルール。
 */
function drawWireNumbers(ctx: CanvasRenderingContext2D, vp: Viewport, sheet: Sheet) {
  const labels = wireNumberLabels(sheet);
  if (labels.length === 0) return;
  ctx.fillStyle = theme.wireNumber;
  ctx.font = `${Math.max(9, WIRE_NO_FONT * vp.scale)}px monospace`;
  for (const label of labels) {
    const s = vp.toScreen(label.at);
    ctx.textAlign = label.align === "center" ? "center" : "right";
    ctx.fillText(label.number, s.x, s.y);
  }
  ctx.textAlign = "left";
}

/**
 * ハーネス境界 (IEC 61082-1) を描く。破線の閉じた多角形+左上角の外側にハーネス名。
 * Rust側 svg.rs の render_harnesses と同一ルール (破線3mm/間隔2mm、名前は右へ1mm・上へ1mm)。
 * 配線より先に描いて背面に置く。
 */
export function drawHarness(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  points: Point[],
  name: string,
  selected: boolean,
): void {
  if (points.length < 2) return;
  ctx.strokeStyle = selected ? theme.selection : theme.harness;
  ctx.lineWidth = selected ? 2 : 1;
  ctx.setLineDash(HARNESS_DASH.map((d) => Math.max(2, d * vp.scale)));
  ctx.beginPath();
  points.forEach((p, i) => {
    const s = vp.toScreen(p);
    if (i === 0) ctx.moveTo(s.x, s.y);
    else ctx.lineTo(s.x, s.y);
  });
  ctx.closePath();
  ctx.stroke();
  ctx.setLineDash([]);
  const at = name ? harnessLabelAt(points) : null;
  if (!at) return;
  const s = vp.toScreen(at);
  ctx.fillStyle = selected ? theme.selection : theme.harness;
  ctx.font = `${Math.max(9, HARNESS_FONT * vp.scale)}px monospace`;
  ctx.textAlign = "left";
  ctx.fillText(name, s.x, s.y);
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
  const hidden = opts.hidden ?? new Set<ViewClass>();
  if (!hidden.has("grid")) drawGrid(ctx, vp, w, h, pw, ph);
  if (!hidden.has("frame")) drawFrame(ctx, vp, sheet, opts.frame, opts.bind);

  const defs = new Map(symbols.map((d) => [d.id, d]));
  const resolve = (id: string) => resolveSymbolDef(id, defs);
  const entities = Object.values(sheet.entities).filter((e) => !hidden.has(entityViewClass(e)));

  // ハーネス境界は配線・シンボルの背面
  for (const e of entities) {
    if (e.kind !== "harness") continue;
    drawHarness(ctx, vp, e.points, e.name, opts.selection.has(e.id));
  }
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
    const def = resolve(e.symbol_id);
    if (!def) continue;
    drawSymbol(ctx, vp, e, def, opts.selection.has(e.id), undefined, !hidden.has("refs"));
    // コイル⇔接点クロスリファレンス (表示クラスは「参照記号」に含める)
    if (hidden.has("refs")) continue;
    const rows = opts.relay?.contactMaps?.get(e.id);
    if (rows?.length) drawContactMap(ctx, vp, e, def, rows);
    const location = opts.relay?.coilLocations?.get(e.id);
    if (location) {
      const at = vp.toScreen(coilLocationAt(symbolBounds(e, def), e.at.y));
      ctx.fillStyle = theme.xref;
      ctx.font = `${Math.max(8, CONTACT_MAP_FONT * vp.scale)}px monospace`;
      ctx.textAlign = "left";
      ctx.fillText(location, at.x, at.y);
    }
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
      // シート間クロスリファレンス (IEC 61082-1)。相手のいるラベルだけ右脇に添える
      const xref = opts.xrefs?.get(e.id);
      if (xref) {
        const at = vp.toScreen(xrefTextAt(e));
        ctx.fillStyle = theme.xref;
        ctx.font = `${Math.max(8, XREF_FONT * vp.scale)}px monospace`;
        ctx.fillText(xref, at.x, at.y);
      }
    } else if (e.kind === "text") {
      const s = vp.toScreen(e.at);
      ctx.fillStyle = theme.line;
      ctx.font = `${Math.max(9, e.height * vp.scale)}px sans-serif`;
      ctx.textAlign = "left";
      ctx.fillText(e.text, s.x, s.y);
    }
  }

  // 線番 (表示クラス "wire_numbers")。SVG出力と同じ位置・等幅フォントで描く
  if (!hidden.has("wire_numbers")) drawWireNumbers(ctx, vp, sheet);

  // エージェント編集オーバーレイ(図面の上、クロスヘアの下)
  if (opts.agent) drawAgentOverlay(ctx, vp, opts.agent);

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
