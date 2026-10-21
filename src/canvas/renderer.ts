// Canvas2Dレンダラ本体。純関数でDOM非依存(ctxとviewportを受け取るだけ)。
// 描画順: 背景 → グリッド → JIS図枠 → 配線 → シンボル → ジャンクション → ラベル → 選択

import type { Point, Shape, Sheet, SymbolDef, SymbolInstance, WireLegend as WireLegendEntity, WirePart } from "../ipc";
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
import {
  captionLineAnchors,
  resolveInstanceDef,
  symbolBounds,
  transformLocal,
} from "./dynamicSymbol";
// 循環import防止のため定義はdynamicSymbol.tsへ移した。他モジュールとの互換のため再エクスポートする
export { symbolBounds, transformLocal } from "./dynamicSymbol";
import { LEADER_STROKE_MM, leaderFor } from "./leader";
import { drawCable } from "./cable";
import { DIM_FONT, DIM_TICK, dimensionGeometry, nameplateLines, nameplateRect } from "./panelMarks";
import { HARNESS_DASH, HARNESS_FONT, harnessLabelAt } from "./harness";
import { MATE_LINE_DASH, mateLinkSegment, type MateLink } from "./mate";
import {
  SHAPE_DASH,
  shapeCircle,
  shapeFillColor,
  shapeRectCorners,
  shapeStrokeColor,
} from "./shapes";
import {
  JUMPER_BAR_STROKE_MM,
  jumperBars,
  parseJumperSpec,
  terminalLabels,
  terminalNumbers,
  terminalNumbersOfInstance,
} from "./terminalJumpers";
import { PIN_GROUP_CAPTION_TEXT_MM, PIN_GROUPS_ATTR, pinGroupBrackets } from "./pinGroups";
import { entityViewClass, type ViewClass } from "./viewClasses";
import {
  BALLOON_FONT,
  BALLOON_R,
  balloon,
  ductHatch,
  ducts,
  footprintRect,
  footprints,
  LEGEND_COLUMNS,
  LEGEND_COL_W,
  LEGEND_FONT,
  LEGEND_ROW_H,
  legendRect,
  legendRows,
  panelLegends,
  panelOutlineRect,
  panelOutlines,
  panelPlateRect,
  railRect,
  rails,
  type Rect,
} from "./panel";
import {
  LEGEND_COLUMNS as WIRE_LEGEND_COLUMNS,
  LEGEND_COL_W as WIRE_LEGEND_COL_W,
  LEGEND_FONT as WIRE_LEGEND_FONT,
  LEGEND_ROW_H as WIRE_LEGEND_ROW_H,
  legendRect as wireLegendRect,
  legendRows as wireLegendRows,
} from "./wireLegend";
import { agentRgba, panelTheme, theme, wireColorScreen } from "./theme";
import {
  FUNCTION_LABEL_FONT,
  LABEL_FONT,
  REF_LABEL_DY,
  VALUE_LABEL_DY,
  canvasTextAlign,
  functionLabelBelow,
  functionLabelFallbackAt,
  functionLabelSlot,
  functionLabelText,
  noteColor,
} from "./textStyle";
import { wireNumberFont, wireNumberLabels } from "./wireNumbers";
import { wireNoteFont, wireNoteLabels } from "./wireNotes";
import { TAG_FONT, netTag } from "./netTag";
import { XREF_FONT, deviceXrefAt, xrefTextAt } from "./xref";
import { annotationScale } from "./annotationText";
import {
  CONTACT_MAP_FONT,
  coilLocationAt,
  contactMapLayout,
  contactMapOrigin,
  deviceBlockBounds,
  deviceBlockLayout,
  deviceBlockLeadAxis,
  deviceBlockLeadEnd,
  deviceBlockTexts,
  type ContactMapRow,
  type DeviceBlock,
} from "./relayXref";
import { GRID_PITCH, type Viewport } from "./viewport";
import { textPx } from "./textSize";

/** セル内テキストの左余白 (mm)。 */
const CELL_PAD = 2;
/** シンボル本体・ピングループ括弧の線幅 (mm)。svg.rsのSYMBOL_STROKEと一致。 */
export const SYMBOL_STROKE_MM = 0.3;
/**
 * 丸印の半径 (mm)。svg.rsのJUNCTION_DOT_Rと一致。配線の分岐点 (ジャンクション) と、
 * 引き出し線 (G20) の対象端に同じ大きさで描く。
 */
export const JUNCTION_DOT_R_MM = 0.6;
/** 盤の名前を外形の上端からどれだけ上に置くか (mm)。svg.rsのPANEL_LABEL_DYと一致。 */
const PANEL_LABEL_DY = 1.5;
/** フットプリントの型番の文字高さ (mm) と、参照記号からの送り (mm)。svg.rsと一致。 */
const FOOTPRINT_PART_NO_FONT = 2;
const FOOTPRINT_PART_NO_DY = 3;
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
    /** コイルのid → その下に描く接点マップの行 (分散配置のコイルだけ)。 */
    contactMaps?: ReadonlyMap<string, ContactMapRow[]>;
    /** 接点のid → 脇に描くコイル所在 (例 "(/1.C2)")。 */
    coilLocations?: ReadonlyMap<string, string>;
    /** コイルのid → 実機ブロック表示の中身 (G7、`sheetDeviceBlocks()`の戻り)。 */
    deviceBlocks?: ReadonlyMap<string, DeviceBlock>;
  };
  /**
   * デバイス単位シート間クロスリファレンス (G13)。`sheetDeviceXrefs()` の戻り。
   * 省略時は描かない。表示クラスは「参照記号」に含める。
   */
  deviceXrefs?: ReadonlyMap<string, string>;
  /**
   * このシート上で図面に破線を描くべき嵌合コネクタ対 (G14)。`mateLinks()` の戻りを
   * このシートで絞ったもの。省略時は描かない。表示クラスは「シンボル」に含める
   * (シンボルどうしを結ぶ線なので)。表示クラスは画面だけのもので、SVG/PDF出力は
   * 他のクラスと同じく常に全部を描く。
   */
  mateLinks?: readonly MateLink[];
  /** 図枠テンプレート (M4 §9)。省略時は同梱のjis-a3。 */
  frame?: FrameTemplate;
  /** 表題欄バインドの文脈。省略時はシート単体 (1枚中1枚目・プロジェクト名は空)。 */
  bind?: BindContext;
  /**
   * プロジェクトの電線品番表 (G11)。電線品番凡例ブロック (G9残) の品番引き当てに使う。
   * 省略時は空 (ワイヤ自身に書かれた品番だけで引く)。
   */
  wireParts?: WirePart[];
  /** 参照記号 → 回路図の説明 (盤の銘板の1行目を追従させる。`deviceDescriptions()`の戻り)。 */
  deviceDescriptions?: ReadonlyMap<string, string>;
}

/** エージェント編集オーバーレイの描画入力。 */
export interface AgentPaint {
  /** パルス表示する領域(用紙mm)。AgentOverlay.activeRegions()の戻り。 */
  regions: Region[];
  /** ターン進行中か。領域が無くてもラベルチップだけ出す。 */
  active: boolean;
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
    ctx.font = `${bold}${textPx(cell.size, vp.scale)}px sans-serif`;
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
  ctx.font = `${textPx(REV_HEADER_FONT, vp.scale)}px sans-serif`;
  REV_HEADERS.forEach((label, i) => {
    const p = vp.toScreen({
      x: l.colLefts[i] + CELL_PAD,
      y: l.headerTop + l.rowH / 2 + REV_HEADER_FONT / 2,
    });
    ctx.fillText(label, p.x, p.y);
  });
  // 改訂行
  ctx.fillStyle = theme.line;
  ctx.font = `${textPx(REV_FONT, vp.scale)}px sans-serif`;
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
  ctx.lineWidth = Math.max(1, SYMBOL_STROKE_MM * vp.scale);
  // 端子番号の上書き (Task 2, spec §1.1): シンボル定義に焼き込まれた生のピン番号のテキストを
  // このインスタンスの表示ラベルへ差し替える。上書きが無ければnullで、差し替えは起きない
  // (SVG出力 svg.rs::render_symbol の双子)。
  const labels = terminalLabels(def, inst.attrs);
  const terminalRename = labels.renameMap;
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
        // 文字は常に水平に書くので、ベースラインへのずらしはシンボルと一緒に回さず
        // 画面の下向きに取る(Rust側svg.rsと同じ規則)
        const p = vp.toScreen(transformLocal(prim.at, inst));
        ctx.font = `${textPx(prim.height, vp.scale)}px sans-serif`;
        ctx.textAlign = "center";
        const label = terminalRename?.get(prim.text) ?? prim.text;
        ctx.fillText(label, p.x, p.y + (prim.height * vp.scale) / 2.8);
        break;
      }
    }
  }
  // ジャンパバー (G5): 端子台のattrs.jumpersを、端子の印の隙間を埋める太い線として描く
  // (SVG出力のrender_jumper_barsと同じジオメトリ。表示クラスは"symbols"に含まれる)
  if (inst.symbol_id.startsWith("terminal_block_")) {
    const rawTerminals = terminalNumbers(def);
    const displayTerminals = terminalNumbersOfInstance(def, inst.attrs);
    const pairs = parseJumperSpec(inst.attrs.jumpers ?? "", displayTerminals, rawTerminals);
    // ジオメトリ(jumperBars)はシンボル定義の生のピン番号でしか端子を探せないので、
    // 表示ラベルの組を生の番号の組へ戻してから渡す(端子番号の上書き = Task 2)
    const dispToRaw = new Map(displayTerminals.map((d, i) => [d, rawTerminals[i]] as const));
    const rawPairs: [string, string][] = pairs.map(([a, b]) => [
      dispToRaw.get(a) ?? a,
      dispToRaw.get(b) ?? b,
    ]);
    const bars = jumperBars(def, rawPairs);
    if (bars.length > 0) {
      const lineWidth = ctx.lineWidth;
      ctx.lineWidth = Math.max(1, JUMPER_BAR_STROKE_MM * vp.scale);
      for (const [a, b] of bars) {
        const sa = vp.toScreen(transformLocal(a, inst));
        const sb = vp.toScreen(transformLocal(b, inst));
        ctx.beginPath();
        ctx.moveTo(sa.x, sa.y);
        ctx.lineTo(sb.x, sb.y);
        ctx.stroke();
      }
      ctx.lineWidth = lineWidth;
    }
  }
  // ピングループ括弧 (G8): attrs.pin_groupsが指す範囲ごとに角括弧とキャプションを描く
  // (SVG出力のrender_pin_group_bracketsと同じジオメトリ)。読めない・重なる項目は
  // pinGroupBrackets側で既に除いているので、ここでは求まったものをそのまま描くだけ
  const pinGroupsSpec = inst.attrs[PIN_GROUPS_ATTR];
  if (pinGroupsSpec) {
    for (const bracket of pinGroupBrackets(def, pinGroupsSpec, terminalRename)) {
      const lineWidth = ctx.lineWidth;
      ctx.lineWidth = Math.max(1, SYMBOL_STROKE_MM * vp.scale);
      ctx.beginPath();
      bracket.bracket.forEach((p, i) => {
        const s = vp.toScreen(transformLocal(p, inst));
        if (i === 0) ctx.moveTo(s.x, s.y);
        else ctx.lineTo(s.x, s.y);
      });
      ctx.stroke();
      ctx.lineWidth = lineWidth;
      ctx.font = `${textPx(PIN_GROUP_CAPTION_TEXT_MM, vp.scale)}px sans-serif`;
      ctx.textAlign = "center";
      // キャプションの行は用紙座標で積む (文字は回転しても横書きなので、ローカル座標で
      // 積むと回転90/270度で行が横に並んで重なる。Rust側 caption_line_anchors と同じ)
      for (const [text, at] of captionLineAnchors(inst, bracket)) {
        const p = vp.toScreen(at);
        ctx.fillText(text, p.x, p.y + (PIN_GROUP_CAPTION_TEXT_MM * vp.scale) / 2.8);
      }
      ctx.textAlign = "left";
    }
  }
  if (!showLabels) return;
  // 参照記号・型番: シンボル外形の上に併記(縦長シンボルでも重ならない。Rust側svg.rsと同ルール)
  const top = symbolTopY(inst, def);
  ctx.fillStyle = colorOverride ?? (selected ? theme.selection : theme.annotation);
  ctx.font = `${textPx(LABEL_FONT, vp.scale)}px monospace`;
  ctx.textAlign = "center";
  if (inst.reference) {
    const p = vp.toScreen({ x: inst.at.x, y: top - REF_LABEL_DY });
    ctx.fillText(inst.reference, p.x, p.y);
  }
  if (inst.value) {
    const p = vp.toScreen({ x: inst.at.x, y: top - VALUE_LABEL_DY });
    ctx.fillText(inst.value, p.x, p.y);
  }
  // 機能ラベル (G4の第3テキスト欄): 参照記号・型番の反対側=外形の下 (svg.rsと同ルール)
  const functionLabel = functionLabelText(inst);
  if (functionLabel) {
    const slot = functionLabelSlot(def);
    const at = slot
      ? transformLocal(slot.at, inst)
      : functionLabelFallbackAt(inst, symbolBounds(inst, def));
    const height = slot?.height ?? FUNCTION_LABEL_FONT;
    ctx.font = `${textPx(height, vp.scale)}px monospace`;
    const p = vp.toScreen(at);
    ctx.fillText(functionLabel, p.x, p.y);
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
  ctx.font = `${textPx(CONTACT_MAP_FONT, vp.scale)}px monospace`;
  ctx.textAlign = "left";
  rows.forEach((row, i) => {
    const y = vp.toScreen({ x: layout.x, y: layout.baseline(i) }).y;
    ctx.fillText(row.terminals, vp.toScreen({ x: layout.textX(0), y: 0 }).x, y);
    ctx.fillText(row.address, vp.toScreen({ x: layout.textX(1), y: 0 }).x, y);
  });
}

/**
 * コイルを実機ブロック (G7) として描く: 参照記号+型番のヘッダと、接点の「機器イメージ」
 * (端子対+所在) を1つの外形にまとめる。コイル自身の小さな図形 (rect+リード) の代わりに
 * これを描く。
 *
 * 外形・仕切り線・引き出し線は、シンボルの図形と同じようにインスタンスの回転・ミラーを
 * 掛けて描く (transformLocal)。引き出し線はシンボル定義の実際のピン (A1/A2) から外形の縁まで
 * 引くので、コイルをどの向きに置いても配線は繋がったまま。文字はシンボル内の文字と同じく
 * 常に水平に書き、回した後のセルの中へ置く。SVG出力 (svg.rs render_device_block) と同じ数式。
 *
 * 接点の行 (機器イメージ) は表示クラス「参照記号」に属するので、分散配置の接点マップと
 * 同じく showContacts=false で消える。外形とヘッダはシンボルそのものなので常に描く。
 */
export function drawDeviceBlock(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  inst: SymbolInstance,
  def: SymbolDef,
  block: DeviceBlock,
  selected: boolean,
  /** falseで接点の「機器イメージ」を描かない (表示クラス "refs")。 */
  showContacts = true,
) {
  const layout = deviceBlockLayout(block);
  const bounds = deviceBlockBounds(inst, block);
  const topLeft = vp.toScreen(bounds.min);
  const bottomRight = vp.toScreen(bounds.max);
  const rule = (x0: number, y0: number, x1: number, y1: number) => {
    const a = vp.toScreen({ x: x0, y: y0 });
    const b = vp.toScreen({ x: x1, y: y1 });
    ctx.moveTo(a.x, a.y);
    ctx.lineTo(b.x, b.y);
  };
  ctx.strokeStyle = selected ? theme.selection : theme.line;
  ctx.lineWidth = Math.max(1, SYMBOL_STROKE_MM * vp.scale);
  ctx.strokeRect(topLeft.x, topLeft.y, bottomRight.x - topLeft.x, bottomRight.y - topLeft.y);
  // ピン (A1/A2) から外形の縁までの引き出し線。ピン自身の軸 (配線と同じ向き) に沿って引く
  ctx.beginPath();
  for (const pin of def.pins) {
    const at = transformLocal(pin.at, inst);
    const tip = transformLocal(deviceBlockLeadAxis(pin.at), inst);
    const axis = { x: tip.x - inst.at.x, y: tip.y - inst.at.y };
    const end = deviceBlockLeadEnd(at, axis, bounds);
    if (end) rule(at.x, at.y, end.x, end.y);
  }
  ctx.stroke();
  // ヘッダ行・接点行の仕切り線 (表は常に正立)
  const [left, right] = [bounds.min.x, bounds.max.x];
  ctx.strokeStyle = selected ? theme.selection : theme.xref;
  ctx.lineWidth = 1;
  ctx.beginPath();
  for (let i = 1; i < layout.headerRows; i += 1) {
    const y = bounds.min.y + layout.rowH * i;
    rule(left, y, right, y);
  }
  if (layout.contactRows > 0 && showContacts) {
    const headerBottom = bounds.min.y + layout.rowH * layout.headerRows;
    const col1 = inst.at.x + layout.colX(1);
    rule(left, headerBottom, right, headerBottom);
    rule(col1, headerBottom, col1, bounds.max.y);
    for (let i = 1; i < layout.contactRows; i += 1) {
      const y = inst.at.y + layout.contactRowTop(i);
      rule(left, y, right, y);
    }
  }
  ctx.stroke();
  // 文字 (ヘッダは中央揃え、接点の行は左揃え)。接点の行は表示クラス「参照記号」で消える
  ctx.fillStyle = selected ? theme.selection : theme.xref;
  ctx.font = `${textPx(CONTACT_MAP_FONT, vp.scale)}px monospace`;
  for (const t of deviceBlockTexts(inst, block)) {
    if (!showContacts && t.align === "left") continue;
    ctx.textAlign = t.align === "center" ? "center" : "left";
    const at = vp.toScreen(t.at);
    ctx.fillText(t.text, at.x, at.y);
  }
  // 機能ラベル (G4): 分散配置と同じく外形の下に置く (ここではブロックの外形の下)
  const functionLabel = functionLabelText(inst);
  if (functionLabel) {
    ctx.font = `${textPx(FUNCTION_LABEL_FONT, vp.scale)}px monospace`;
    ctx.textAlign = "center";
    const at = vp.toScreen(functionLabelBelow((bounds.min.x + bounds.max.x) / 2, bounds.max.y));
    ctx.fillText(functionLabel, at.x, at.y);
  }
}

/** 配置後のシンボル外形の上端Y(用紙座標)。 */
export function symbolTopY(inst: SymbolInstance, def: SymbolDef): number {
  return symbolBounds(inst, def).min.y;
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
  ctx.font = `${Math.max(9, wireNumberFont(sheet) * vp.scale)}px monospace`;
  for (const label of labels) {
    const s = vp.toScreen(label.at);
    ctx.textAlign = label.align === "center" ? "center" : "right";
    ctx.fillText(label.number, s.x, s.y);
  }
  ctx.textAlign = "left";
}

/**
 * 配線注記 (G2) を描く。位置は wireNoteLabels が決める (ワイヤごとに最長線分の中点、
 * 線番の反対側=横線は下・縦線は右へ2.5mm)。Rust側 wire_note.rs / svg.rs と同一ルール。
 */
function drawWireNotes(ctx: CanvasRenderingContext2D, vp: Viewport, sheet: Sheet) {
  const labels = wireNoteLabels(sheet);
  if (labels.length === 0) return;
  ctx.fillStyle = theme.wireNote;
  ctx.font = `${Math.max(8, wireNoteFont(sheet) * vp.scale)}px monospace`;
  for (const label of labels) {
    const s = vp.toScreen(label.at);
    ctx.textAlign = label.align === "center" ? "center" : "left";
    ctx.fillText(label.text, s.x, s.y);
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
  ctx.font = `${textPx(HARNESS_FONT, vp.scale)}px monospace`;
  ctx.textAlign = "left";
  ctx.fillText(name, s.x, s.y);
}

/**
 * 嵌合コネクタ対 (G14) の破線。svg.rsのrender_mate_linksと同一ルール:
 * 両方の外形のうち、相手の配置基準点にいちばん近い点どうしを短い破線で結ぶ。
 * 配線・シンボルより先に描いて背面に置く (ハーネス境界と同じ扱い)。
 */
export function drawMateLink(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  sheet: Sheet,
  link: MateLink,
  defs: ReadonlyMap<string, SymbolDef>,
): void {
  const seg = mateLinkSegment(sheet, link, defs);
  if (!seg) return;
  ctx.strokeStyle = theme.harness;
  ctx.lineWidth = 1;
  ctx.setLineDash(MATE_LINE_DASH.map((d) => Math.max(2, d * vp.scale)));
  ctx.beginPath();
  const a = vp.toScreen(seg.a);
  const b = vp.toScreen(seg.b);
  ctx.moveTo(a.x, a.y);
  ctx.lineTo(b.x, b.y);
  ctx.stroke();
  ctx.setLineDash([]);
}

/**
 * 図形 (G10) を1つ描く。矩形・線・円・折れ線を、色トークン・線幅・破線・塗りのとおりに。
 * Rust側 svg.rs の render_shapes と同一ルール (破線3mm/間隔2mm、塗りは下が透ける濃さ)。
 * 配線・シンボルより先に描いて背面に置く。
 */
export function drawShape(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  shape: Shape,
  selected: boolean,
): void {
  if (shape.points.length < 2) return;
  const fill = shapeFillColor(shape);
  ctx.strokeStyle = shapeStrokeColor(shape, selected);
  // 紙の線幅をそのまま拡大すると細すぎて見えないので、画面では最低1pxを保つ
  ctx.lineWidth = Math.max(1, shape.stroke_width_mm * vp.scale) + (selected ? 1 : 0);
  ctx.setLineDash(shape.dashed ? SHAPE_DASH.map((d) => Math.max(2, d * vp.scale)) : []);
  ctx.beginPath();
  switch (shape.shape) {
    case "rect": {
      const r = shapeRectCorners(shape.points);
      if (!r) break;
      const a = vp.toScreen(r.min);
      const b = vp.toScreen(r.max);
      ctx.rect(a.x, a.y, b.x - a.x, b.y - a.y);
      break;
    }
    case "circle": {
      const c = shapeCircle(shape.points);
      if (!c) break;
      const s = vp.toScreen(c.center);
      ctx.arc(s.x, s.y, c.r * vp.scale, 0, Math.PI * 2);
      break;
    }
    case "line":
    case "polyline": {
      shape.points.forEach((p, i) => {
        const s = vp.toScreen(p);
        if (i === 0) ctx.moveTo(s.x, s.y);
        else ctx.lineTo(s.x, s.y);
      });
      break;
    }
  }
  if (fill && shape.shape !== "line") {
    ctx.fillStyle = fill;
    ctx.fill();
  }
  ctx.stroke();
  ctx.setLineDash([]);
}

/** 用紙座標の矩形をスクリーンへ変換して塗る/描く (盤レイアウト用の共通処理)。 */
function paintRect(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  rect: Rect,
  fill: string | null,
  stroke: string,
  lineWidth: number,
): void {
  const a = vp.toScreen({ x: rect.x, y: rect.y });
  const b = vp.toScreen({ x: rect.x + rect.w, y: rect.y + rect.h });
  if (fill) {
    ctx.fillStyle = fill;
    ctx.fillRect(a.x, a.y, b.x - a.x, b.y - a.y);
  }
  ctx.strokeStyle = stroke;
  ctx.lineWidth = lineWidth;
  ctx.strokeRect(a.x, a.y, b.x - a.x, b.y - a.y);
}

/**
 * 盤レイアウト (M4 §8) を描く。奥から順に **盤外形+取付板 → ダクト → DINレール →
 * フットプリント**。寸法の数式は `panel.ts` (Rustの`panel.rs`と同一) が持ち、
 * 配色は `panelTheme` (.pen「M4デザイン - 盤レイアウト」)。選択中は sel-blue の枠で囲む。
 */
export function drawPanel(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  sheet: Sheet,
  selection: Set<string>,
  descriptions: ReadonlyMap<string, string> = new Map(),
): void {
  const edge = (id: string, normal: string) => (selection.has(id) ? theme.selection : normal);
  const width = (id: string, normal = 1) => (selection.has(id) ? 2 : normal);

  for (const o of panelOutlines(sheet)) {
    paintRect(
      ctx,
      vp,
      panelOutlineRect(o),
      panelTheme.enclosure,
      edge(o.id, panelTheme.enclosureEdge),
      width(o.id, 1.5),
    );
    const plate = panelPlateRect(o);
    if (plate) paintRect(ctx, vp, plate, panelTheme.plate, panelTheme.plateEdge, 1);
    const name = o.name?.trim();
    if (name) {
      const s = vp.toScreen({ x: o.at.x + 2, y: o.at.y - PANEL_LABEL_DY });
      ctx.fillStyle = edge(o.id, panelTheme.label);
      ctx.font = `${textPx(3, vp.scale)}px sans-serif`;
      ctx.textAlign = "left";
      ctx.fillText(name, s.x, s.y);
    }
  }

  for (const duct of ducts(sheet)) {
    paintRect(
      ctx,
      vp,
      { x: duct.at.x, y: duct.at.y, w: duct.w, h: duct.h },
      panelTheme.duct,
      edge(duct.id, panelTheme.ductEdge),
      width(duct.id),
    );
    ctx.strokeStyle = panelTheme.ductHatch;
    ctx.lineWidth = 1;
    ctx.beginPath();
    for (const line of ductHatch(duct)) {
      const a = vp.toScreen({ x: line.x1, y: line.y1 });
      const b = vp.toScreen({ x: line.x2, y: line.y2 });
      ctx.moveTo(a.x, a.y);
      ctx.lineTo(b.x, b.y);
    }
    ctx.stroke();
  }

  for (const rail of rails(sheet)) {
    paintRect(ctx, vp, railRect(rail), panelTheme.rail, edge(rail.id, panelTheme.railEdge), width(rail.id));
    // 取付線 (部品が乗る中心線)
    const a = vp.toScreen({ x: Math.min(rail.x1, rail.x2), y: rail.y });
    const b = vp.toScreen({ x: Math.max(rail.x1, rail.x2), y: rail.y });
    ctx.strokeStyle = panelTheme.railEdge;
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(a.x, a.y);
    ctx.lineTo(b.x, b.y);
    ctx.stroke();
  }

  for (const fp of footprints(sheet)) {
    const rect = footprintRect(fp);
    paintRect(
      ctx,
      vp,
      rect,
      panelTheme.footprint,
      edge(fp.id, panelTheme.footprintEdge),
      width(fp.id),
    );
    const center = vp.toScreen({ x: rect.x + rect.w / 2, y: rect.y + rect.h / 2 });
    ctx.textAlign = "center";
    const reference = fp.reference?.trim();
    if (reference) {
      ctx.fillStyle = panelTheme.reference;
      ctx.font = `${textPx(LABEL_FONT, vp.scale)}px monospace`;
      ctx.fillText(reference, center.x, center.y);
    }
    const partNo = fp.part_no?.trim();
    if (partNo) {
      ctx.fillStyle = panelTheme.partNo;
      ctx.font = `${textPx(FOOTPRINT_PART_NO_FONT, vp.scale)}px sans-serif`;
      ctx.fillText(partNo, center.x, center.y + Math.max(9, FOOTPRINT_PART_NO_DY * vp.scale));
    }
    ctx.textAlign = "left";
  }

  drawBalloons(ctx, vp, sheet);
  drawLegends(ctx, vp, sheet, selection);
  drawPanelMarks(ctx, vp, sheet, selection, descriptions);
}

/** 銘板 (枠+中央揃えの文字) と関連付き寸法線を描く (SVG出力と同じ形)。 */
function drawPanelMarks(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  sheet: Sheet,
  selection: Set<string>,
  descriptions: ReadonlyMap<string, string>,
): void {
  for (const e of Object.values(sheet.entities)) {
    if (e.kind !== "nameplate") continue;
    const r = nameplateRect(sheet, e);
    if (!r) continue;
    const selected = selection.has(e.id);
    paintRect(ctx, vp, r, panelTheme.nameplate, selected ? theme.selection : panelTheme.nameplateEdge, selected ? 2 : 1);
    const lines = nameplateLines(sheet, e, descriptions);
    const pitch = e.text_mm * 1.3;
    const top = r.y + r.h / 2 - (pitch * (Math.max(lines.length, 1) - 1)) / 2 + e.text_mm * 0.35;
    ctx.fillStyle = panelTheme.nameplateText;
    ctx.font = `${textPx(e.text_mm, vp.scale)}px sans-serif`;
    ctx.textAlign = "center";
    lines.forEach((line, i) => {
      const s = vp.toScreen({ x: r.x + r.w / 2, y: top + i * pitch });
      ctx.fillText(line, s.x, s.y);
    });
    ctx.textAlign = "left";
  }
  for (const e of Object.values(sheet.entities)) {
    if (e.kind !== "dimension") continue;
    const g = dimensionGeometry(sheet, e);
    if (!g) continue;
    const color = selection.has(e.id) ? theme.selection : panelTheme.dimension;
    ctx.strokeStyle = color;
    ctx.lineWidth = selection.has(e.id) ? 2 : 1;
    ctx.beginPath();
    for (const [a, b] of [...g.extensions, g.line]) {
      const sa = vp.toScreen(a);
      const sb = vp.toScreen(b);
      ctx.moveTo(sa.x, sa.y);
      ctx.lineTo(sb.x, sb.y);
    }
    const t = DIM_TICK / 2;
    for (const p of g.line) {
      const sa = vp.toScreen({ x: p.x - t, y: p.y + t });
      const sb = vp.toScreen({ x: p.x + t, y: p.y - t });
      ctx.moveTo(sa.x, sa.y);
      ctx.lineTo(sb.x, sb.y);
    }
    ctx.stroke();
    const s = vp.toScreen(g.textAt);
    ctx.fillStyle = color;
    ctx.font = `${textPx(DIM_FONT, vp.scale)}px sans-serif`;
    ctx.textAlign = "center";
    ctx.fillText(g.text, s.x, s.y);
    ctx.textAlign = "left";
  }
}

/**
 * バルーン (アイテム番号) を描く。採番済みのフットプリントだけに付き、
 * 白丸の下は塗りつぶす (盤面が透けて番号が読めなくならないため)。
 */
function drawBalloons(ctx: CanvasRenderingContext2D, vp: Viewport, sheet: Sheet): void {
  for (const fp of footprints(sheet)) {
    const b = balloon(fp);
    if (!b) continue;
    const from = vp.toScreen(b.leaderFrom);
    const to = vp.toScreen(b.leaderTo);
    ctx.strokeStyle = panelTheme.balloonEdge;
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(from.x, from.y);
    ctx.lineTo(to.x, to.y);
    ctx.stroke();

    const center = vp.toScreen(b.center);
    const r = Math.max(6, BALLOON_R * vp.scale);
    ctx.beginPath();
    ctx.arc(center.x, center.y, r, 0, Math.PI * 2);
    ctx.fillStyle = panelTheme.balloon;
    ctx.fill();
    ctx.stroke();

    ctx.fillStyle = panelTheme.balloonText;
    ctx.font = `600 ${textPx(BALLOON_FONT, vp.scale)}px monospace`;
    ctx.textAlign = "center";
    ctx.fillText(String(b.itemNo), center.x, center.y + Math.max(3, (BALLOON_FONT / 2) * vp.scale));
    ctx.textAlign = "left";
  }
}

/**
 * 凡例表 (アイテム番号↔参照記号・型番) を描く。中身は同じシートのフットプリントから
 * 導出する (`legendRows`) ので、部品を足しても採番し直しても図面と食い違わない。
 */
function drawLegends(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  sheet: Sheet,
  selection: Set<string>,
): void {
  const legends = panelLegends(sheet);
  if (legends.length === 0) return;
  const rows: string[][] = [LEGEND_COLUMNS.map((c) => c), ...legendRows(sheet)];
  for (const legend of legends) {
    const rect = legendRect(sheet, legend);
    const edge = selection.has(legend.id) ? theme.selection : panelTheme.legendEdge;
    paintRect(ctx, vp, rect, panelTheme.legend, edge, selection.has(legend.id) ? 2 : 1);
    ctx.strokeStyle = panelTheme.legendRule;
    ctx.lineWidth = 1;
    ctx.beginPath();
    let x = rect.x;
    for (const cw of LEGEND_COL_W.slice(0, -1)) {
      x += cw;
      const a = vp.toScreen({ x, y: rect.y });
      const b = vp.toScreen({ x, y: rect.y + rect.h });
      ctx.moveTo(a.x, a.y);
      ctx.lineTo(b.x, b.y);
    }
    for (let r = 1; r < rows.length; r += 1) {
      const y = rect.y + LEGEND_ROW_H * r;
      const a = vp.toScreen({ x: rect.x, y });
      const b = vp.toScreen({ x: rect.x + rect.w, y });
      ctx.moveTo(a.x, a.y);
      ctx.lineTo(b.x, b.y);
    }
    ctx.stroke();

    const font = Math.max(8, LEGEND_FONT * vp.scale);
    rows.forEach((row, r) => {
      const baseline = rect.y + LEGEND_ROW_H * r + LEGEND_ROW_H / 2 + LEGEND_FONT / 2;
      let cx = rect.x;
      row.forEach((cell, i) => {
        if (cell) {
          const p = vp.toScreen({ x: cx + 1, y: baseline });
          ctx.fillStyle = r === 0 ? panelTheme.legendHead : panelTheme.legendText;
          ctx.font = `${r === 0 ? "600 " : ""}${font}px ${i === 2 ? "sans-serif" : "monospace"}`;
          ctx.fillText(cell, p.x, p.y);
        }
        cx += LEGEND_COL_W[i];
      });
    });
  }
}

/**
 * 電線品番凡例ブロック (色・sq・電線品番・使用長のテーブル、G9残) を描く。中身は
 * 同じシートの配線から導出する (`wireLegendRows`) ので、配線を足しても図面と食い違わない。
 * 回路図の他の表 (接点マップ等) と同じく塗りは無し。
 */
function drawWireLegend(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  sheet: Sheet,
  legend: WireLegendEntity,
  wireParts: WirePart[] | undefined,
  selected: boolean,
): void {
  const rows: string[][] = [WIRE_LEGEND_COLUMNS.map((c) => c), ...wireLegendRows(sheet, wireParts)];
  const rect = wireLegendRect(sheet, wireParts, legend);
  const edge = selected ? theme.selection : theme.line;
  const topLeft = vp.toScreen({ x: rect.x, y: rect.y });
  const bottomRight = vp.toScreen({ x: rect.x + rect.w, y: rect.y + rect.h });
  ctx.strokeStyle = edge;
  ctx.lineWidth = selected ? 2 : 1;
  ctx.strokeRect(topLeft.x, topLeft.y, bottomRight.x - topLeft.x, bottomRight.y - topLeft.y);

  ctx.strokeStyle = theme.dim;
  ctx.lineWidth = 1;
  ctx.beginPath();
  let x = rect.x;
  for (const cw of WIRE_LEGEND_COL_W.slice(0, -1)) {
    x += cw;
    const a = vp.toScreen({ x, y: rect.y });
    const b = vp.toScreen({ x, y: rect.y + rect.h });
    ctx.moveTo(a.x, a.y);
    ctx.lineTo(b.x, b.y);
  }
  for (let r = 1; r < rows.length; r += 1) {
    const y = rect.y + WIRE_LEGEND_ROW_H * r;
    const a = vp.toScreen({ x: rect.x, y });
    const b = vp.toScreen({ x: rect.x + rect.w, y });
    ctx.moveTo(a.x, a.y);
    ctx.lineTo(b.x, b.y);
  }
  ctx.stroke();

  const font = Math.max(8, WIRE_LEGEND_FONT * vp.scale);
  rows.forEach((row, r) => {
    const baseline = rect.y + WIRE_LEGEND_ROW_H * r + WIRE_LEGEND_ROW_H / 2 + WIRE_LEGEND_FONT / 2;
    let cx = rect.x;
    row.forEach((cell, i) => {
      if (cell) {
        const p = vp.toScreen({ x: cx + 1, y: baseline });
        ctx.fillStyle = r === 0 ? theme.dim : theme.line;
        ctx.font = `${r === 0 ? "600 " : ""}${font}px monospace`;
        ctx.fillText(cell, p.x, p.y);
      }
      cx += WIRE_LEGEND_COL_W[i];
    });
  });
  ctx.textAlign = "left";
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
  const entities = Object.values(sheet.entities).filter((e) => !hidden.has(entityViewClass(e)));
  // コイル所在 (G22) の逃がし判定に使う、このシートの配線注記と、その文字高 (G20で伸縮)
  const wireNotes = wireNoteLabels(sheet);
  const noteFont = wireNoteFont(sheet);

  // 盤レイアウト (盤外形・ダクト・レール・フットプリント) は最背面
  if (!hidden.has("panel")) drawPanel(ctx, vp, sheet, opts.selection, opts.deviceDescriptions);

  // 図形 (G10) は配線・シンボルの背面 (塗りつぶした強調枠が回路を隠さない)
  for (const e of entities) {
    if (e.kind !== "shape") continue;
    drawShape(ctx, vp, e, opts.selection.has(e.id));
  }
  // ハーネス境界は配線・シンボルの背面
  for (const e of entities) {
    if (e.kind !== "harness") continue;
    drawHarness(ctx, vp, e.points, e.name, opts.selection.has(e.id));
  }
  // 多芯ケーブルの定義線は配線の上に重ねて描く (配線を横切る斜線なので後で描く)
  const cables = entities.filter((e) => e.kind === "cable");
  // 嵌合コネクタ対 (G14) の破線も配線・シンボルの背面。シンボルどうしを結ぶ線なので
  // 表示クラスは「シンボル」に属する (シンボルを隠せば線も消える)
  if (!hidden.has("symbols")) {
    for (const link of opts.mateLinks ?? []) {
      drawMateLink(ctx, vp, sheet, link, defs);
    }
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
  for (const e of cables) {
    if (e.kind === "cable") drawCable(ctx, vp, e, opts.selection.has(e.id));
  }
  for (const e of entities) {
    if (e.kind !== "symbol") continue;
    const def = resolveInstanceDef(e, defs);
    if (!def) continue;
    const block = opts.relay?.deviceBlocks?.get(e.id);
    if (block) {
      // 実機ブロック表示 (G7): コイル自身の小さな図形の代わりにこれを描く。接点の
      // 「機器イメージ」は分散配置の接点マップと同じ表示クラス「参照記号」に属する
      drawDeviceBlock(ctx, vp, e, def, block, opts.selection.has(e.id), !hidden.has("refs"));
    } else {
      drawSymbol(ctx, vp, e, def, opts.selection.has(e.id), undefined, !hidden.has("refs"));
    }
    // コイル⇔接点クロスリファレンス・デバイス単位XRef (表示クラスは「参照記号」に含める)
    if (hidden.has("refs")) continue;
    if (!block) {
      const rows = opts.relay?.contactMaps?.get(e.id);
      if (rows?.length) drawContactMap(ctx, vp, e, def, rows);
    }
    const location = opts.relay?.coilLocations?.get(e.id);
    if (location) {
      const at = vp.toScreen(
        coilLocationAt(e, symbolBounds(e, def), location, wireNotes, noteFont),
      );
      ctx.fillStyle = theme.xref;
      ctx.font = `${textPx(CONTACT_MAP_FONT, vp.scale)}px monospace`;
      ctx.textAlign = "left";
      ctx.fillText(location, at.x, at.y);
    }
    const deviceXref = opts.deviceXrefs?.get(e.id);
    if (deviceXref) {
      const at = vp.toScreen(deviceXrefAt(e, symbolBounds(e, def), symbolTopY(e, def)));
      ctx.fillStyle = theme.xref;
      ctx.font = `${textPx(XREF_FONT, vp.scale)}px monospace`;
      ctx.textAlign = "left";
      ctx.fillText(deviceXref, at.x, at.y);
    }
  }
  for (const e of entities) {
    if (e.kind === "junction") {
      const s = vp.toScreen(e.at);
      ctx.fillStyle = theme.line;
      ctx.beginPath();
      ctx.arc(s.x, s.y, Math.max(2, JUNCTION_DOT_R_MM * vp.scale), 0, Math.PI * 2);
      ctx.fill();
    } else if (e.kind === "net_label") {
      // 方向付きタグ (G3/G19)。attrs.tagがある時だけ旗の枠で囲む (Rust側 net_tag.rs と同形)
      const labelFont = TAG_FONT * annotationScale(sheet);
      const tag = netTag(e, labelFont);
      ctx.fillStyle = theme.annotation;
      ctx.font = `${textPx(labelFont, vp.scale)}px monospace`;
      ctx.textAlign = "left";
      if (tag) {
        ctx.strokeStyle = theme.annotation;
        ctx.lineWidth = 1;
        ctx.beginPath();
        tag.points.forEach((p, i) => {
          const t = vp.toScreen(p);
          if (i === 0) ctx.moveTo(t.x, t.y);
          else ctx.lineTo(t.x, t.y);
        });
        ctx.closePath();
        ctx.stroke();
        const t = vp.toScreen(tag.textAt);
        ctx.fillText(e.name, t.x, t.y);
      } else {
        const s = vp.toScreen(e.at);
        ctx.fillText(e.name, s.x, s.y - 2);
      }
      // シート間クロスリファレンス (IEC 61082-1)。相手のいるラベルだけ右脇に添える
      const xref = opts.xrefs?.get(e.id);
      if (xref) {
        // タグ付きは旗の本体側の外へ逃がす (枠にも自分の配線にも重ねない)
        const at = vp.toScreen(tag ? tag.xrefAt : xrefTextAt(e, labelFont));
        ctx.fillStyle = theme.xref;
        ctx.font = `${Math.max(8, XREF_FONT * annotationScale(sheet) * vp.scale)}px monospace`;
        ctx.textAlign = tag ? canvasTextAlign(tag.xrefAnchor) : "left";
        ctx.fillText(xref, at.x, at.y);
        ctx.textAlign = "left";
      }
    } else if (e.kind === "text") {
      // 注記の装飾 (G4): 色トークン・基準位置・太字 (紙の色は text_style.rs が持つ)
      const s = vp.toScreen(e.at);
      ctx.fillStyle = noteColor(e.color, opts.selection.has(e.id));
      const weight = e.bold ? "600 " : "";
      ctx.font = `${weight}${textPx(e.height, vp.scale)}px sans-serif`;
      ctx.textAlign = canvasTextAlign(e.anchor);
      ctx.fillText(e.text, s.x, s.y);
      ctx.textAlign = "left";
      // 引き出し線 (G20): 対象がシート上に実在すれば、注記の辺から対象の外形まで
      // 毎回求め直して引く (位置は保存しないので、どちらを動かしても追従する)
      const leader = leaderFor(sheet, e, defs);
      if (leader) {
        const [start, end] = leader;
        const sa = vp.toScreen(start);
        const sb = vp.toScreen(end);
        ctx.strokeStyle = theme.line;
        ctx.lineWidth = Math.max(1, LEADER_STROKE_MM * vp.scale);
        ctx.beginPath();
        ctx.moveTo(sa.x, sa.y);
        ctx.lineTo(sb.x, sb.y);
        ctx.stroke();
        ctx.fillStyle = theme.line;
        ctx.beginPath();
        ctx.arc(sb.x, sb.y, Math.max(2, JUNCTION_DOT_R_MM * vp.scale), 0, Math.PI * 2);
        ctx.fill();
      }
    }
  }

  // 電線品番凡例ブロック (G9残)。表示クラスは「注記」に含める (entitiesが既に絞り込み済み)
  for (const e of entities) {
    if (e.kind !== "wire_legend") continue;
    drawWireLegend(ctx, vp, sheet, e, opts.wireParts, opts.selection.has(e.id));
  }

  // 線番 (表示クラス "wire_numbers")。SVG出力と同じ位置・等幅フォントで描く
  if (!hidden.has("wire_numbers")) drawWireNumbers(ctx, vp, sheet);
  // 配線注記 (表示クラス "wire_notes")。画面では消せるが、SVG/PDF出力には必ず出る
  if (!hidden.has("wire_notes")) drawWireNotes(ctx, vp, sheet);

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
