// 図枠テンプレート (プロットフレーム、M4仕様 §9) のレイアウト計算。
// Rust側 `madake-core/src/frame_template.rs` + `svg.rs` と同一ルールであること
// (キャンバスと印刷出力が同じ図枠になる)。
//
// テンプレートの実体はJSON (`src-tauri/resources/frame_templates/*.json`)。
// 一覧はIPC `list_frame_templates` / Link API `GET /frame-templates` で取れる。
// ここに持つ DEFAULT_FRAME_TEMPLATE は、まだ一覧を読んでいないときに使う
// 同梱jis-a3の写し (Rust側 jis-a3.json と一致させること)。

import type { Latin, Project, Revision, Sheet, TitleBlock } from "../ipc";
import { textAdvanceMm, withLatin } from "./strokeFont";

// ---- Rust側 (frame_template.rs / svg.rs) と共有する数値。変えるときは両方を変える ----
/** 大文字の高さ ÷ フォントサイズ (`text_size_is: cap` の換算)。 */
export const CAP_HEIGHT_RATIO = 0.72;
/** 複数行の文字の行送り ÷ フォントサイズ。 */
export const LINE_PITCH = 1.25;
/** 罫線の線幅 (mm)。 */
export const LINE_STROKE = { thin: 0.25, medium: 0.5, heavy: 1 } as const;
/** 裁断マーク・中心マークの線幅 (mm)。 */
export const MARK_STROKE = 0.5;
/** 中心マークの長さの上限 (mm)。 */
export const CENTRE_MARK_MAX = 10;
/** 枠付きセル (`boxed`) の、文字の箱から枠までの距離 (mm)。 */
export const BOX_GAP = 0.8;
/** 改訂記号を囲む三角の既定の形: 高さ ÷ 行高、底辺 ÷ 高さ (正三角形)、列の左端からの距離 (mm)。 */
export const MARK_TRIANGLE_H = 0.8;
export const MARK_TRIANGLE_ASPECT = 2 / Math.sqrt(3);
export const MARK_TRIANGLE_PAD = 2;
/** 三角の中の記号: ベースラインを底辺から上げる量 ÷ 三角の高さ、文字高の上限 ÷ 三角の高さ。 */
export const MARK_TEXT_LIFT = 0.12;
export const MARK_TEXT_MAX = 0.55;
/** 同じセルに続けて出す項目 (`suffix`) と前のものとの間隔、記号の列で右に残す余白 (mm)。 */
export const SUFFIX_GAP = 1;
const SUFFIX_RIGHT_PAD = 0.5;
/** 裁断マークの既定の位置 (用紙の端から mm) と腕の長さ (mm)。 */
export const TRIM_INSET = 4;
export const TRIM_ARM = 15;
/** 中心マークの矢印 (開いたV字) の長さと、片側の幅 (mm)。 */
export const CENTRE_ARROW_LEN = 1.9;
export const CENTRE_ARROW_HALF = 0.5;
/** ベースラインより下に出る分 ÷ フォントサイズ (枠の下端の見積もり)。 */
const DESCENT_RATIO = 0.25;
/** セル内テキストの左右の余白 (mm)。 */
const CELL_PAD = 2;
/** 改訂欄の本文・列見出しの文字高さ (mm)。 */
const REV_FONT = 2.5;
const REV_HEADER_FONT = 2.2;

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
  /** 回転 (0/90/180/270、画面上で時計回り、アンカー点が中心)。 */
  rotation?: number;
  /** 文字を枠で囲む。 */
  boxed?: boolean;
  /** `w`に収まるよう文字を縮める。 */
  fit?: boolean;
  /** 字間 (字の送りに掛ける倍率、既定1)。ストロークフォントのときだけ効く。 */
  spacing?: number | null;
}

/** 罫線の太さ (thin 0.25 / medium 0.5 / heavy 1 mm)。 */
export type LineWeight = "thin" | "medium" | "heavy";

export interface FrameGridLine {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  weight?: LineWeight;
}

/** 辺ごとの図枠の余白 (mm)。 */
export interface FrameMargins {
  left: number;
  right: number;
  top: number;
  bottom: number;
}

/** 改訂欄の列が表示する項目。 */
export type RevisionField = "mark" | "count" | "description" | "rev_no" | "date" | "by" | "approved";

/** 改訂欄の1列。 */
export interface RevisionColumn {
  field: RevisionField;
  /** 見出し行に印字する文字。 */
  header: string;
  /** 列幅 (mm)。 */
  w: number;
  align?: CellAlign;
  /** 同じセルの中で、主の項目のあとに続けて出す項目 (例: 記号の横の個数)。間に罫線は入らない。 */
  suffix?: RevisionField | null;
  /** 見出しの揃え。無ければ`align`と同じ。 */
  header_align?: CellAlign | null;
}

/** 改訂記号を囲む三角の形 (省略した項目は既定値)。 */
export interface MarkStyle {
  /** 三角の高さ ÷ 行高 (既定0.8)。 */
  h?: number;
  /** 底辺 ÷ 高さ (既定は正三角形)。 */
  aspect?: number;
  /** 斜辺の中ほどを空ける割合 (0〜0.9、既定0)。 */
  gap?: number;
  /** 列の左端から三角の左下の角までの距離 (mm、既定2)。 */
  pad?: number;
  /** 改訂の無い行にも三角を描く。 */
  in_empty_rows?: boolean;
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
  /** 辺ごとの余白。あれば`frame_margin`の代わりに使う。 */
  frame_margins?: FrameMargins | null;
  zones: {
    /** 新しいシートの既定値。描画は各シートのzone_cols/zone_rowsを使う。 */
    cols: number;
    rows: number;
    col_labels: LabelStyle;
    row_labels: LabelStyle;
    /** ゾーン目盛を描くか (既定: 描く)。 */
    show?: boolean;
  };
  /** 図枠の外に描くマーク (用紙四隅の裁断マーク / 外枠の各辺中央の中心マーク)。 */
  marks?: {
    trim?: boolean;
    centre?: boolean;
    /** 裁断マークの角の位置 (用紙の各辺からの距離 mm)。無ければ4辺とも4mm。 */
    trim_insets?: FrameMargins | null;
    /** 裁断マークの腕の長さ (mm)。無ければ15mm。 */
    trim_arm?: number | null;
    /** 中心マークを外枠から裁断マークの線まで伸ばす。 */
    centre_to_trim?: boolean;
    /** 中心マークの外側の端に矢印 (開いたV字) を付ける。 */
    centre_arrow?: boolean;
    /** 中心マークの線の太さ。無ければ0.5mm。 */
    centre_weight?: LineWeight | null;
  };
  /** セルの`size`の意味 (font = フォントサイズ / cap = 大文字の高さ)。 */
  text_size_is?: "font" | "cap";
  /** ストロークフォントのとき、図枠の文字の英数字を半角・全角どちらで描くか (無ければプロジェクトの設定)。 */
  text_latin?: Latin | null;
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
    /** 列の定義。無ければ従来の4列 (記号/日付/内容/承認)。 */
    columns?: RevisionColumn[] | null;
    /** 改訂欄の行高。無ければ表題欄の`row_h`。 */
    row_h?: number | null;
    /** 改訂が少なくても必ず描く行数。 */
    rows_fixed?: number | null;
    /** 改訂記号を囲む形。 */
    mark_shape?: "none" | "triangle";
    /** 改訂行・見出し行の文字サイズ (mm)。無ければ2.5 / 2.2。 */
    text_size?: number | null;
    header_size?: number | null;
    /** 記号を囲む三角の形。 */
    mark_style?: MarkStyle | null;
  };
  logo?: {
    x: number;
    y: number;
    w: number;
    h: number;
    path?: string | null;
    /** 読み込んだ画像 (data URI)。テンプレートのファイルには書かれない実行時の項目。 */
    data_url?: string | null;
  } | null;
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
      if (name.startsWith("field.")) return tb.fields?.[name.slice("field.".length)] ?? "";
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

/** 4辺の余白。`frame_margins`が無ければ`frame_margin`を4辺に使う。 */
export function frameMargins(t: FrameTemplate): FrameMargins {
  const m = t.frame_margin;
  return t.frame_margins ?? { left: m, right: m, top: m, bottom: m };
}

/** 外枠の矩形 (用紙mm)。 */
export function frameRect(t: FrameTemplate, pw: number, ph: number): { x0: number; y0: number; x1: number; y1: number } {
  const m = frameMargins(t);
  return { x0: m.left, y0: m.top, x1: pw - m.right, y1: ph - m.bottom };
}

/** 描くフォントサイズ。`text_size_is: cap`なら大文字高さから換算する。 */
export function fontSize(t: FrameTemplate, size: number): number {
  return t.text_size_is === "cap" ? size / CAP_HEIGHT_RATIO : size;
}

/** `maxW`に収まるフォントサイズ。収まっていれば`font`のまま (大きくはしない)。 */
export function fittedFont(lines: string[], font: number, maxW?: number | null, spacing = 1): number {
  if (maxW == null || !(maxW > 0)) return font;
  // 幅はストロークフォントが有効ならその線の幅 (Rust側 fitted_font_spaced と同じ)
  const widest = lines.reduce((w, l) => Math.max(w, textAdvanceMm(l, font, spacing)), 0);
  return widest <= maxW ? font : (font * maxW) / widest;
}

/** 改訂欄の列。`columns`が無ければ従来の4列 (記号/日付/内容/承認)。 */
export function revisionColumns(t: FrameTemplate): RevisionColumn[] {
  if (t.revision_block.columns) return t.revision_block.columns;
  const fields: RevisionField[] = ["mark", "date", "description", "by"];
  return t.revision_block.col_w.slice(0, fields.length).map((w, i) => ({
    field: fields[i],
    header: REV_HEADERS[i],
    w,
    align: "start" as const,
  }));
}

/** 改訂欄が新しい項目を使っているか (Rust側 uses_extended_revision_table と同じ判定)。 */
export function usesExtendedRevisionTable(t: FrameTemplate): boolean {
  const rb = t.revision_block;
  return (
    rb.columns != null ||
    rb.row_h != null ||
    rb.rows_fixed != null ||
    (rb.mark_shape ?? "none") !== "none" ||
    (t.text_size_is ?? "font") !== "font" ||
    rb.text_size != null ||
    rb.header_size != null ||
    rb.mark_style != null
  );
}

/** 改訂欄のセル1つ (見出しまたは本文) の文字の配置。空のセルは`lines`が空。 */
export interface RevisionText {
  /** アンカーX (揃えに応じて左端・中央・右端)。 */
  x: number;
  align: CellAlign;
  /** 収めたあとのフォントサイズ (mm)。 */
  size: number;
  lines: string[];
  /** 各行のベースラインY。 */
  baselines: number[];
}

/** 改訂欄1行分の配置。topは行上端の用紙Y (mm)。 */
export interface RevisionRowLayout {
  rev: Revision;
  top: number;
  /** 列ごとの文字 (列の並び順)。 */
  cells: RevisionText[];
  /** 記号を囲む三角の3頂点 (頂点・右下・左下)。三角を描かない行はnull。描く線は`markShapes`にある。 */
  triangle: [number, number][] | null;
  /** 同じセルに続けて出す文字 (記号の列の`suffix`。三角の横に置く)。 */
  extras: RevisionText[];
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
  /** 列ごとの左端・幅・揃え・見出しの文字。 */
  columns: { left: number; w: number; align: CellAlign; header: RevisionText }[];
  /** 行の間の横罫線のY (最上段の上は外枠なので含まない)。 */
  ruleYs: number[];
  /** 記号の三角を描く図形: 切れ目の無い三角は多角形、切れ目のある三角は線 (空行のぶんも含む)。 */
  markShapes: { polygons: [number, number][][]; lines: { x1: number; y1: number; x2: number; y2: number }[] };
}

/** 表題欄の左上角の用紙座標 (mm)。 */
export function titleBlockOrigin(
  t: FrameTemplate,
  pw: number,
  ph: number,
): { x: number; y: number } {
  const tb = t.title_block;
  const { x0, y0, x1, y1 } = frameRect(t, pw, ph);
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
  return withLatin(t.text_latin, () => revisionLayoutInner(sheet, t));
}

function revisionLayoutInner(sheet: Sheet, t: FrameTemplate): RevisionLayout | null {
  const rb = t.revision_block;
  const rows = visibleRevisions(sheet.revisions ?? [], rb.rows_fixed ?? rb.rows_max);
  const nRows = rb.rows_fixed ?? rows.length;
  if (nRows === 0) return null;
  const { w: pw, h: ph } = paperSizeMm(sheet);
  const { x: tx, y: ty } = titleBlockOrigin(t, pw, ph);
  const extended = usesExtendedRevisionTable(t);
  const rowH = rb.row_h ?? t.title_block.row_h;
  const h = (nRows + 1) * rowH;
  const top = ty - h;
  const cols = revisionColumns(t);
  const colLefts: number[] = [];
  let x = tx;
  for (const c of cols) {
    colLefts.push(x);
    x += c.w;
  }
  // 1セル分の文字。従来の表は左揃え・収めなし、新しい表は列幅と行高に収める
  // (Rust側 svg.rs の revision_block / revision_table の cell_text と同じ)。
  const cellText = (
    rowTop: number,
    left: number,
    c: RevisionColumn,
    base: number,
    value: string,
    colAlign: CellAlign = c.align ?? "start",
  ): RevisionText => {
    const align = extended ? colAlign : "start";
    const ax = align === "middle" ? left + c.w / 2 : align === "end" ? left + c.w - CELL_PAD : left + CELL_PAD;
    if (value === "") return { x: ax, align, size: base, lines: [], baselines: [] };
    if (!extended) {
      return { x: ax, align, size: base, lines: [value], baselines: [rowTop + rowH / 2 + base / 2] };
    }
    const lines = value.split("\n");
    let font = fittedFont(lines, base, c.w - 2 * CELL_PAD);
    const m = lines.length;
    if (m * font * LINE_PITCH > rowH) font *= rowH / (m * font * LINE_PITCH);
    const pitch = font * LINE_PITCH;
    const first = rowTop + rowH / 2 + font / 2 - ((m - 1) * pitch) / 2;
    return { x: ax, align, size: font, lines, baselines: lines.map((_, k) => first + pitch * k) };
  };
  const emptyRowTriangles = () => {
    // 改訂の無い行にも三角だけを置く (刷り込みの記号枠)。行の三角のあとに足す (Rust側と同じ順)
    const j = cols.findIndex((c) => c.field === "mark");
    if (!triangleMarks || !style.in_empty_rows || j < 0) return;
    for (let r = rows.length; r < nRows; r++) {
      const rowTop = ty - (r + 2) * rowH;
      addTriangle(colLefts[j] + pad + hb, rowTop + rowH / 2 + th / 2);
    }
  };
  const headerFont = extended ? fontSize(t, rb.header_size ?? REV_HEADER_FONT) : REV_HEADER_FONT;
  const body = extended ? fontSize(t, rb.text_size ?? REV_FONT) : REV_FONT;
  const headerTop = ty - rowH;
  const triangleMarks = extended && rb.mark_shape === "triangle";
  // 記号の三角 (Rust側 svg.rs の mark_triangle と同じ): 高さ・半分の底辺・切れ目
  const style = rb.mark_style ?? {};
  const th = (style.h ?? MARK_TRIANGLE_H) * rowH;
  const hb = (th * (style.aspect ?? MARK_TRIANGLE_ASPECT)) / 2;
  const pad = style.pad ?? MARK_TRIANGLE_PAD;
  const gap = style.gap ?? 0;
  const markShapes: RevisionLayout["markShapes"] = { polygons: [], lines: [] };
  const addTriangle = (cx: number, by: number): [number, number][] => {
    const points: [number, number][] = [
      [cx, by - th],
      [cx + hb, by],
      [cx - hb, by],
    ];
    if (gap <= 0) {
      markShapes.polygons.push(points);
    } else {
      const [a, b] = [(1 - gap) / 2, (1 + gap) / 2];
      for (const side of [1, -1]) {
        const at = (f: number) => [cx + side * hb * f, by - th + th * f];
        for (const [f0, f1] of [
          [0, a],
          [b, 1],
        ]) {
          const [x1, y1] = at(f0);
          const [x2, y2] = at(f1);
          markShapes.lines.push({ x1, y1, x2, y2 });
        }
      }
      markShapes.lines.push({ x1: cx - hb, y1: by, x2: cx + hb, y2: by });
    }
    return points;
  };
  // 古い行 (rows[0]) が列見出しの直上、新しい行ほど上へ
  const rowLayouts: RevisionRowLayout[] = rows.map((rev, i) => {
    const rowTop = ty - (i + 2) * rowH;
    let triangle: [number, number][] | null = null;
    const extras: RevisionText[] = [];
    const cells = cols.map((c, j) => {
      const value = (rev[c.field] ?? "") as string;
      const suffix = extended && c.suffix ? ((rev[c.suffix] ?? "") as string) : "";
      if (c.field === "mark" && triangleMarks) {
        if (value === "") return cellText(rowTop, colLefts[j], c, body, "");
        const cx = colLefts[j] + pad + hb;
        const by = rowTop + rowH / 2 + th / 2;
        triangle = addTriangle(cx, by);
        // 記号は三角の中に収める (三角の底辺の半分より広い文字は縮める)
        const mark: RevisionText = {
          x: cx,
          align: "middle",
          size: fittedFont([value], Math.min(body, th * MARK_TEXT_MAX), hb),
          lines: [value],
          baselines: [by - th * MARK_TEXT_LIFT],
        };
        if (suffix !== "") {
          // 三角の右の角の横に、ふつうのセルと同じ高さで続ける
          const sx = cx + hb + SUFFIX_GAP;
          const size = fittedFont([suffix], body, colLefts[j] + c.w - SUFFIX_RIGHT_PAD - sx);
          extras.push({ x: sx, align: "start", size, lines: [suffix], baselines: [rowTop + rowH / 2 + size / 2] });
        }
        return mark;
      }
      // ふつうの列: 主の文字のあとに、空白1つを挟んで続ける
      const joined = suffix === "" ? value : value === "" ? suffix : `${value} ${suffix}`;
      return cellText(rowTop, colLefts[j], c, body, joined);
    });
    return { rev, top: rowTop, cells, triangle, extras };
  });
  emptyRowTriangles();
  return {
    x: tx,
    y: top,
    w: t.title_block.w,
    h,
    rowH,
    colLefts,
    headerTop,
    rows: rowLayouts,
    columns: cols.map((c, j) => ({
      left: colLefts[j],
      w: c.w,
      align: extended ? (c.align ?? "start") : "start",
      header: cellText(headerTop, colLefts[j], c, headerFont, c.header, c.header_align ?? c.align ?? "start"),
    })),
    ruleYs: Array.from({ length: nRows }, (_, r) => top + rowH * (r + 1)),
    markShapes,
  };
}

/** 表題欄のセル1つの描画位置 (用紙mm) と、解決済みの文字。 */
export interface CellLayout {
  x: number;
  y: number;
  /** 描くフォントサイズ (大文字高さの換算と、幅への収めを済ませた値)。 */
  size: number;
  align: CellAlign;
  weight: CellWeight;
  text: string;
  /** 改行で分けた行。2行目以降はベースラインから `size × LINE_PITCH` ずつ下へ。 */
  lines: string[];
  /** 回転 (度、時計回り、(x, y) が中心)。 */
  rotation: number;
  /** 字間 (ストロークフォントのときだけ効く)。 */
  spacing: number;
  /** 文字を囲む枠 (回転前の用紙mm)。枠の無いセルはnull。 */
  box: { x: number; y: number; w: number; h: number } | null;
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
    /** ゾーン目盛 (文字と目盛線) を描くか。 */
    show: boolean;
  };
  /** 裁断マーク・中心マークの線 (用紙mm) と、その線幅 (mm)。 */
  marks: FrameMarkLine[];
  /** 中心マークの端の矢印 (開いたV字の折れ線) と、その線幅 (mm)。 */
  markHeads: { points: [number, number][]; stroke: number }[];
  titleBlock: {
    x: number;
    y: number;
    w: number;
    h: number;
    rowH: number;
    /** 用紙座標へ移した罫線と、その線幅 (mm)。 */
    gridLines: (FrameGridLine & { stroke: number })[];
    /** 空欄で省くセルを除いた、描くセルだけ。 */
    cells: CellLayout[];
  };
  revision: RevisionLayout | null;
  /** ロゴ枠。`dataUrl`があれば画像を描き、無ければ置き場所を示す枠だけを描く。 */
  logo: { x: number; y: number; w: number; h: number; dataUrl: string | null } | null;
}

/** 裁断マーク・中心マークの線1本 (用紙mm) と、その線幅 (mm)。 */
export interface FrameMarkLine {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  stroke: number;
}

/** 裁断マーク (裁断線の四隅のL字) と中心マーク (外枠の各辺の中央から外向き)。Rust側 frame_marks と同じ順。 */
function frameMarks(
  t: FrameTemplate,
  pw: number,
  ph: number,
): { lines: FrameMarkLine[]; heads: FrameLayout["markHeads"] } {
  const lines: FrameMarkLine[] = [];
  const heads: FrameLayout["markHeads"] = [];
  const marks = t.marks ?? {};
  const ins = marks.trim_insets ?? { left: TRIM_INSET, right: TRIM_INSET, top: TRIM_INSET, bottom: TRIM_INSET };
  if (marks.trim) {
    const arm = marks.trim_arm ?? TRIM_ARM;
    const corners: [number, number, number, number][] = [
      [ins.left, ins.top, 1, 1],
      [pw - ins.right, ins.top, -1, 1],
      [ins.left, ph - ins.bottom, 1, -1],
      [pw - ins.right, ph - ins.bottom, -1, -1],
    ];
    for (const [cx, cy, sx, sy] of corners) {
      lines.push({ x1: cx, y1: cy, x2: cx + sx * arm, y2: cy, stroke: MARK_STROKE });
      lines.push({ x1: cx, y1: cy, x2: cx, y2: cy + sy * arm, stroke: MARK_STROKE });
    }
  }
  if (marks.centre) {
    const { x0, y0, x1, y1 } = frameRect(t, pw, ph);
    const m = frameMargins(t);
    const mx = (x0 + x1) / 2;
    const my = (y0 + y1) / 2;
    const stroke = marks.centre_weight ? LINE_STROKE[marks.centre_weight] : MARK_STROKE;
    // 辺ごとに: 外枠上の始点、外向きの単位ベクトル、余白、裁断線までの距離の差し引き分
    const sides: [number, number, number, number, number, number][] = [
      [mx, y0, 0, -1, m.top, ins.top],
      [mx, y1, 0, 1, m.bottom, ins.bottom],
      [x0, my, -1, 0, m.left, ins.left],
      [x1, my, 1, 0, m.right, ins.right],
    ];
    for (const [sx, sy, dx, dy, margin, inset] of sides) {
      const len = marks.centre_to_trim ? margin - inset : Math.min(margin, CENTRE_MARK_MAX);
      if (len <= 0) continue;
      const [ex, ey] = [sx + dx * len, sy + dy * len];
      lines.push({ x1: sx, y1: sy, x2: ex, y2: ey, stroke });
      if (marks.centre_arrow) {
        // 外側の端を先端にした開いたV字。(px, py) は進行方向に直角な単位ベクトル
        const [bx, by] = [ex - dx * CENTRE_ARROW_LEN, ey - dy * CENTRE_ARROW_LEN];
        const [px, py] = [-dy, dx];
        heads.push({
          points: [
            [bx + px * CENTRE_ARROW_HALF, by + py * CENTRE_ARROW_HALF],
            [ex, ey],
            [bx - px * CENTRE_ARROW_HALF, by - py * CENTRE_ARROW_HALF],
          ],
          stroke,
        });
      }
    }
  }
  return { lines, heads };
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
  // 図枠の文字は、テンプレートが英数字の字 (半角・全角) を決めていればそれで測る (Rust側 latin_scope と同じ)
  return withLatin(t.text_latin, () => frameLayoutInner(sheet, t, ctx));
}

function frameLayoutInner(sheet: Sheet, t: FrameTemplate, ctx: BindContext): FrameLayout {
  const { w: pw, h: ph } = paperSizeMm(sheet);
  const frame = frameRect(t, pw, ph);
  const marks = frameMarks(t, pw, ph);
  const cols = Math.max(1, sheet.zone_cols);
  const rows = Math.max(1, sheet.zone_rows);
  const { x: tx, y: ty } = titleBlockOrigin(t, pw, ph);
  const cells: CellLayout[] = [];
  for (const cell of t.title_block.cells) {
    const text = cellText(cell, ctx);
    if (cell.omit_if_empty && text === "") continue;
    // Rust側 svg.rs の frame_cell と同じ: 大文字高さの換算 → 幅への収め → 行分け → 枠
    const lines = text.split("\n");
    const base = fontSize(t, cell.size);
    const spacing = cell.spacing ?? 1;
    const size = cell.fit ? fittedFont(lines, base, cell.w, spacing) : base;
    const align = cell.align ?? "start";
    const cx = tx + cell.x;
    const cy = ty + cell.y;
    let box: CellLayout["box"] = null;
    // 値が空のセルには囲むものが無いので、枠も作らない
    if (cell.boxed && text !== "") {
      const w = lines.reduce((m, l) => Math.max(m, textAdvanceMm(l, size, spacing)), 0);
      const left = align === "middle" ? cx - w / 2 : align === "end" ? cx - w : cx;
      const top = cy - size;
      const bottom = cy + size * LINE_PITCH * (lines.length - 1) + size * DESCENT_RATIO;
      box = { x: left - BOX_GAP, y: top - BOX_GAP, w: w + 2 * BOX_GAP, h: bottom - top + 2 * BOX_GAP };
    }
    cells.push({
      x: cx,
      y: cy,
      size,
      align,
      weight: cell.weight ?? "normal",
      text,
      lines,
      rotation: cell.rotation ?? 0,
      box,
      spacing,
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
      show: t.zones.show ?? true,
    },
    marks: marks.lines,
    markHeads: marks.heads,
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
        stroke: LINE_STROKE[g.weight ?? "thin"],
      })),
      cells,
    },
    revision: revisionLayout(sheet, t),
    logo: t.logo
      ? { x: t.logo.x, y: t.logo.y, w: t.logo.w, h: t.logo.h, dataUrl: t.logo.data_url ?? null }
      : null,
  };
}
