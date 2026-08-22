// 図枠テンプレート (プロットフレーム、M4仕様 §9) のレイアウト計算。
// Rust側 `madake-core/src/frame_template.rs` + `svg.rs` と同一ルールであること
// (キャンバスと印刷出力が同じ図枠になる)。
//
// テンプレートの実体はJSON (`src-tauri/resources/frame_templates/*.json`)。
// 一覧はIPC `list_frame_templates` / Link API `GET /frame-templates` で取れる。
// ここに持つ DEFAULT_FRAME_TEMPLATE は、まだ一覧を読んでいないときに使う
// 同梱jis-a3の写し (Rust側 jis-a3.json と一致させること)。

import type { Project, Revision, Sheet, TitleBlock } from "../ipc";

/** ゾーン目盛の文字種。 */
export type LabelStyle = "numeric" | "alpha";
/** 表題欄の基準隅。 */
export type FrameOrigin = "bottom_right" | "bottom_left" | "top_right" | "top_left";
export type CellAlign = "start" | "middle" | "end";
export type CellWeight = "normal" | "bold";

/** 表題欄のセル1つ (座標は表題欄の左上角からの相対mm)。 */
export interface FrameCell {
  x: number;
  y: number;
  w?: number | null;
  h?: number | null;
  /** 固定文字 (バインドは解決しない)。 */
  text?: string | null;
  /** バインド入りの文字列 (例 "{title_block.drawing_no}  Rev {rev.latest}")。 */
  bind?: string | null;
  size: number;
  weight?: CellWeight;
  align?: CellAlign;
  /** 値が空なら描かない (会社名など)。 */
  omit_if_empty?: boolean;
}

export interface FrameGridLine {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
}

export interface FrameTemplate {
  id: string;
  name: string;
  name_ja: string;
  description?: string;
  description_ja?: string;
  /** 想定用紙 (mm)。描画はシートの用紙を使うので、図枠を替えても用紙は変わらない。 */
  paper: { w: number; h: number };
  frame_margin: number;
  zones: {
    /** 新しいシートの既定値。描画は各シートのzone_cols/zone_rowsを使う。 */
    cols: number;
    rows: number;
    col_labels: LabelStyle;
    row_labels: LabelStyle;
  };
  title_block: {
    origin: FrameOrigin;
    w: number;
    h: number;
    row_h: number;
    grid_lines: FrameGridLine[];
    cells: FrameCell[];
  };
  revision_block: {
    rows_max: number;
    col_w: number[];
  };
  logo?: { x: number; y: number; w: number; h: number; path?: string | null } | null;
  builtin?: boolean;
}

/** 未指定・未知idのときに使う図枠テンプレートのid。 */
export const DEFAULT_FRAME_TEMPLATE_ID = "jis-a3";

/** 同梱jis-a3の写し (resources/frame_templates/jis-a3.json と一致させること)。 */
export const DEFAULT_FRAME_TEMPLATE: FrameTemplate = {
  id: "jis-a3",
  name: "JIS A3 landscape",
  name_ja: "JIS A3 横",
  description:
    "JIS Z 8311 frame with a 120x32 mm title block in the bottom-right corner and the revision table stacked above it.",
  description_ja: "JIS Z 8311の図枠。右下に120×32mmの表題欄、その真上に改訂欄を積む。",
  paper: { w: 420, h: 297 },
  frame_margin: 10,
  zones: { cols: 4, rows: 6, col_labels: "numeric", row_labels: "alpha" },
  title_block: {
    origin: "bottom_right",
    w: 120,
    h: 32,
    row_h: 8,
    grid_lines: [
      { x1: 0, y1: 8, x2: 120, y2: 8 },
      { x1: 0, y1: 16, x2: 120, y2: 16 },
      { x1: 0, y1: 24, x2: 120, y2: 24 },
      { x1: 24, y1: 0, x2: 24, y2: 32 },
    ],
    cells: [
      { x: 2, y: 5.5, w: 22, h: 8, text: "図番", size: 3 },
      { x: 26, y: 5.5, w: 92, h: 8, bind: "{title_block.drawing_no}  Rev {rev.latest}", size: 3 },
      { x: 2, y: 13.5, w: 22, h: 8, text: "品名", size: 3 },
      { x: 26, y: 13.5, w: 92, h: 8, bind: "{title_block.title}", size: 3 },
      { x: 2, y: 21.5, w: 22, h: 8, text: "尺度", size: 3 },
      { x: 26, y: 21.5, w: 92, h: 8, bind: "{scale}    日付 {date}", size: 3 },
      { x: 2, y: 29.5, w: 22, h: 8, text: "設計", size: 3 },
      {
        x: 26,
        y: 29.5,
        w: 92,
        h: 8,
        bind: "{title_block.designed}  製図 {title_block.drawn}  検図 {title_block.checked}  承認 {approver}",
        size: 3,
      },
      { x: -2, y: 30, bind: "{title_block.company}", size: 3, align: "end", omit_if_empty: true },
    ],
  },
  revision_block: { rows_max: 6, col_w: [14, 28, 56, 22] },
  logo: null,
};

/** UI言語に合わせた図枠テンプレート名 (日本語名を持たないユーザー図枠は英語名)。 */
export function frameTemplateName(t: FrameTemplate, locale: string): string {
  return locale === "ja" && t.name_ja ? t.name_ja : t.name;
}

/** UI言語に合わせた図枠テンプレートの説明。 */
export function frameTemplateDescription(t: FrameTemplate, locale: string): string {
  return (locale === "ja" ? t.description_ja || t.description : t.description) ?? "";
}

/** 図枠テンプレートの要約 (選択ダイアログの情報行)。 */
export interface FrameTemplateSummary {
  /** 想定用紙 (mm)。実際の描画はシートの用紙を使う。 */
  paper: string;
  /** そのテンプレートが想定するゾーン分割 (実際の分割はシートが持つ)。 */
  zones: string;
  /** 表題欄の大きさ (mm)。 */
  titleBlock: string;
  /** 改訂欄に積める最大行数。 */
  revisionRows: number;
}

/** 情報行に出す要約を組み立てる (数値の整形だけ。文字はi18nカタログが持つ)。 */
export function frameTemplateSummary(t: FrameTemplate): FrameTemplateSummary {
  return {
    paper: `${t.paper.w}×${t.paper.h}`,
    zones: `${t.zones.cols}×${t.zones.rows}`,
    titleBlock: `${t.title_block.w}×${t.title_block.h}`,
    revisionRows: t.revision_block.rows_max,
  };
}

/**
 * テンプレートの表題欄が使っているバインド名を、出てくる順に重複なく返す
 * (選択ダイアログの「フィールドバインド」表)。
 */
export function bindNames(t: FrameTemplate): string[] {
  const out: string[] = [];
  for (const cell of t.title_block.cells) {
    let rest = cell.bind ?? "";
    for (;;) {
      const open = rest.indexOf("{");
      if (open < 0) break;
      const after = rest.slice(open + 1);
      const close = after.indexOf("}");
      if (close < 0) break;
      const name = after.slice(0, close);
      if (name && !out.includes(name)) out.push(name);
      rest = after.slice(close + 1);
    }
  }
  return out;
}

/** プレビューで空欄へ重ねる「そこへ何が入るか」の表示 (デザインの青い`{...}`)。 */
export interface BindOverlay {
  x: number;
  y: number;
  size: number;
  align: CellAlign;
  /** バインドの記述そのまま (例 "{title_block.title}")。 */
  text: string;
}

/**
 * 図枠プレビュー用に、値が空になる表題欄セルのバインド記述を返す。
 *
 * 実際の値が入っている欄には重ねない(図面の値が読めなくならない)。値が空の欄だけ
 * `{title_block.title}`のように青字で見せて、その欄に何が入るかを分かるようにする。
 * 空欄なら描画されないセル (会社名) もプレビューでは見せる。
 */
export function bindOverlays(
  sheet: Sheet,
  t: FrameTemplate = DEFAULT_FRAME_TEMPLATE,
  ctx: BindContext = bindContextForSheet(sheet),
): BindOverlay[] {
  const { w: pw, h: ph } = paperSizeMm(sheet);
  const { x: tx, y: ty } = titleBlockOrigin(t, pw, ph);
  const out: BindOverlay[] = [];
  for (const cell of t.title_block.cells) {
    if (cell.bind == null) continue;
    if (resolveBinds(cell.bind, ctx).trim() !== "") continue;
    out.push({
      x: tx + cell.x,
      y: ty + cell.y,
      size: cell.size,
      align: cell.align ?? "start",
      text: cell.bind,
    });
  }
  return out;
}

/** 改訂欄の列見出し (ISO 7200 / JIS Z 8311)。列幅はテンプレートの revision_block.col_w。 */
export const REV_HEADERS = ["記号", "日付", "内容", "承認"];

/** ゾーン目盛の文字 (0基点)。 */
export function zoneLabel(style: LabelStyle, i: number): string {
  return style === "numeric" ? String(i + 1) : String.fromCharCode(65 + (i % 26));
}

/** 用紙サイズ (mm)。向きを反映する。 */
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

/** バインドを解決するための文脈 (どのプロジェクトの何枚目のシートか)。 */
export interface BindContext {
  projectName: string;
  /** プロジェクト番号。モデルにまだ無いので現状は常に空。 */
  projectNo: string;
  sheetNo: number;
  sheetTotal: number;
  sheetName: string;
  titleBlock: Partial<TitleBlock>;
  revLatest: string;
}

/** シート単体の文脈 (1枚中1枚目・プロジェクト名は空)。 */
export function bindContextForSheet(sheet: Sheet): BindContext {
  return {
    projectName: "",
    projectNo: "",
    sheetNo: 1,
    sheetTotal: 1,
    sheetName: sheet.name,
    titleBlock: sheet.title_block ?? {},
    revLatest: effectiveRev(sheet),
  };
}

/** プロジェクトの中の1シートとしての文脈 (シート番号・総数が入る)。 */
export function bindContextForProject(project: Project, sheet: Sheet): BindContext {
  const index = project.sheets.findIndex((s) => s.id === sheet.id);
  return {
    ...bindContextForSheet(sheet),
    projectName: project.name,
    sheetNo: (index < 0 ? 0 : index) + 1,
    sheetTotal: project.sheets.length,
  };
}

/** 1つのバインド名の値。知らない名前はundefined (呼び出し側で空文字になる)。 */
export function bindValue(name: string, ctx: BindContext): string | undefined {
  const tb = ctx.titleBlock ?? {};
  switch (name) {
    case "project.name":
      return ctx.projectName;
    case "project.no":
      return ctx.projectNo;
    case "sheet.no":
      return String(ctx.sheetNo);
    case "sheet.total":
      return String(ctx.sheetTotal);
    case "sheet.name":
      return ctx.sheetName;
    case "rev.latest":
      return ctx.revLatest;
    case "title_block.company":
      return tb.company ?? "";
    case "title_block.title":
      return tb.title ?? "";
    case "title_block.drawing_no":
      return tb.drawing_no ?? "";
    case "title_block.scale":
    case "scale":
      return tb.scale ?? "";
    case "title_block.date":
    case "date":
      return tb.date ?? "";
    case "title_block.designed":
    case "designer":
      return tb.designed ?? "";
    case "title_block.drawn":
      return tb.drawn ?? "";
    case "title_block.checked":
      return tb.checked ?? "";
    case "title_block.approved":
    case "approver":
      return tb.approved ?? "";
    case "title_block.rev":
      return tb.rev ?? "";
    default:
      return undefined;
  }
}

/**
 * 文字列中の`{バインド名}`を実際の値へ置き換える。知らないバインドは空文字。
 * `{`に対応する`}`が無いところはそのまま残す (Rust側 resolve_binds と同一)。
 */
export function resolveBinds(text: string, ctx: BindContext): string {
  let out = "";
  let rest = text;
  for (;;) {
    const open = rest.indexOf("{");
    if (open < 0) break;
    out += rest.slice(0, open);
    const after = rest.slice(open + 1);
    const close = after.indexOf("}");
    if (close < 0) return out + rest.slice(open);
    out += bindValue(after.slice(0, close), ctx) ?? "";
    rest = after.slice(close + 1);
  }
  return out + rest;
}

/** セルに実際に描く文字列。textは固定、bindはバインドを解決する。 */
export function cellText(cell: FrameCell, ctx: BindContext): string {
  if (cell.text != null) return cell.text;
  if (cell.bind != null) return resolveBinds(cell.bind, ctx);
  return "";
}

/** 図面に描く改訂行(古い順)。上限を超えた分は古い行から省く。 */
export function visibleRevisions(revisions: Revision[], rowsMax = DEFAULT_FRAME_TEMPLATE.revision_block.rows_max): Revision[] {
  return revisions.slice(Math.max(0, revisions.length - rowsMax));
}

/** 表題欄のRev欄に出す改訂記号。改訂があれば最新のmarkを優先する。 */
export function effectiveRev(sheet: Sheet): string {
  const list = sheet.revisions ?? [];
  const latest = list[list.length - 1]?.mark?.trim();
  if (latest) return latest;
  return sheet.title_block?.rev || "-";
}

/** 改訂欄1行分の配置。topは行上端の用紙Y (mm)。 */
export interface RevisionRowLayout {
  rev: Revision;
  top: number;
}

/** 改訂欄の配置 (用紙mm)。表題欄の直上・同じ右端・同じ幅。 */
export interface RevisionLayout {
  x: number;
  y: number;
  w: number;
  h: number;
  /** 行高 (mm)。 */
  rowH: number;
  /** 各列の左端X (記号 / 日付 / 内容 / 承認)。 */
  colLefts: number[];
  /** 列見出し行の上端Y (最下段=表題欄側)。 */
  headerTop: number;
  /** 改訂行(古い順)。古い行ほど下、新しい行ほど上。 */
  rows: RevisionRowLayout[];
}

/** 表題欄の左上角の用紙座標 (mm)。 */
export function titleBlockOrigin(
  t: FrameTemplate,
  pw: number,
  ph: number,
): { x: number; y: number } {
  const m = t.frame_margin;
  const tb = t.title_block;
  const [x0, y0] = [m, m];
  const [x1, y1] = [pw - m, ph - m];
  switch (tb.origin) {
    case "bottom_left":
      return { x: x0, y: y1 - tb.h };
    case "top_right":
      return { x: x1 - tb.w, y: y0 };
    case "top_left":
      return { x: x0, y: y0 };
    default:
      return { x: x1 - tb.w, y: y1 - tb.h };
  }
}

/**
 * 改訂欄の配置を計算する (Rust側 svg.rs の revision_block と同一ルール)。
 * 改訂が0件なら描画しないのでnullを返す。
 */
export function revisionLayout(
  sheet: Sheet,
  t: FrameTemplate = DEFAULT_FRAME_TEMPLATE,
): RevisionLayout | null {
  const rows = visibleRevisions(sheet.revisions ?? [], t.revision_block.rows_max);
  if (rows.length === 0) return null;
  const { w: pw, h: ph } = paperSizeMm(sheet);
  const { x: tx, y: ty } = titleBlockOrigin(t, pw, ph);
  const rowH = t.title_block.row_h;
  const h = (rows.length + 1) * rowH;
  const colLefts: number[] = [];
  let x = tx;
  for (const w of t.revision_block.col_w) {
    colLefts.push(x);
    x += w;
  }
  return {
    x: tx,
    y: ty - h,
    w: t.title_block.w,
    h,
    rowH,
    colLefts,
    headerTop: ty - rowH,
    // 古い行 (rows[0]) が列見出しの直上、新しい行ほど上へ
    rows: rows.map((rev, i) => ({ rev, top: ty - (i + 2) * rowH })),
  };
}

/** 表題欄のセル1つの描画位置 (用紙mm) と、解決済みの文字。 */
export interface CellLayout {
  x: number;
  y: number;
  size: number;
  align: CellAlign;
  weight: CellWeight;
  text: string;
}

/** 図枠1枚分の配置 (すべて用紙mm)。SVG出力・キャンバス描画が共有する。 */
export interface FrameLayout {
  paper: { w: number; h: number };
  /** 図枠の外枠。 */
  frame: { x0: number; y0: number; x1: number; y1: number };
  zones: {
    cols: number;
    rows: number;
    /** 1ゾーンの幅・高さ。 */
    zw: number;
    zh: number;
    colLabels: string[];
    rowLabels: string[];
  };
  titleBlock: {
    x: number;
    y: number;
    w: number;
    h: number;
    rowH: number;
    /** 用紙座標へ移した罫線。 */
    gridLines: FrameGridLine[];
    /** 空欄で省くセルを除いた、描くセルだけ。 */
    cells: CellLayout[];
  };
  revision: RevisionLayout | null;
  logo: { x: number; y: number; w: number; h: number } | null;
}

/**
 * 図枠テンプレートをシートへ当てはめた配置を返す。
 * ゾーンの分割数はシートごとの設定 (図枠を替えても図面の住所は変わらない)、
 * 目盛の文字種・表題欄・改訂欄はテンプレートが決める。
 */
export function frameLayout(
  sheet: Sheet,
  t: FrameTemplate = DEFAULT_FRAME_TEMPLATE,
  ctx: BindContext = bindContextForSheet(sheet),
): FrameLayout {
  const { w: pw, h: ph } = paperSizeMm(sheet);
  const m = t.frame_margin;
  const frame = { x0: m, y0: m, x1: pw - m, y1: ph - m };
  const cols = Math.max(1, sheet.zone_cols);
  const rows = Math.max(1, sheet.zone_rows);
  const { x: tx, y: ty } = titleBlockOrigin(t, pw, ph);
  const cells: CellLayout[] = [];
  for (const cell of t.title_block.cells) {
    const text = cellText(cell, ctx);
    if (cell.omit_if_empty && text === "") continue;
    cells.push({
      x: tx + cell.x,
      y: ty + cell.y,
      size: cell.size,
      align: cell.align ?? "start",
      weight: cell.weight ?? "normal",
      text,
    });
  }
  return {
    paper: { w: pw, h: ph },
    frame,
    zones: {
      cols,
      rows,
      zw: (frame.x1 - frame.x0) / cols,
      zh: (frame.y1 - frame.y0) / rows,
      colLabels: Array.from({ length: cols }, (_, i) => zoneLabel(t.zones.col_labels, i)),
      rowLabels: Array.from({ length: rows }, (_, i) => zoneLabel(t.zones.row_labels, i)),
    },
    titleBlock: {
      x: tx,
      y: ty,
      w: t.title_block.w,
      h: t.title_block.h,
      rowH: t.title_block.row_h,
      gridLines: t.title_block.grid_lines.map((g) => ({
        x1: tx + g.x1,
        y1: ty + g.y1,
        x2: tx + g.x2,
        y2: ty + g.y2,
      })),
      cells,
    },
    revision: revisionLayout(sheet, t),
    logo: t.logo ? { x: t.logo.x, y: t.logo.y, w: t.logo.w, h: t.logo.h } : null,
  };
}
