// 図枠エディタ (M4残り⑥、.pen「M4残りデザイン⑥」`dhP9P`) の純粋ロジック。
//
// 編集の対象は既存の図枠テンプレートJSON (`FrameTemplate`) そのもので、形式は変えない:
// - 線・矩形 → 表題欄の罫線 `title_block.grid_lines` (矩形は4本の罫線)
// - 文字 → 固定文字のセル (`text`)、プレースホルダ → バインド入りのセル (`bind`)
// - 表題欄セル → 大きさ (w/h) を持つバインドのセルと、その外周の罫線
// - ロゴ枠 → `logo`、ゾーン → `zones` (テンプレート全体の設定)
//
// セル・罫線の座標は**表題欄の左上角からの相対mm**で保存し、エディタは用紙座標 (左上原点・
// Y下向き、アプリ全体と同じ) で見せる。関数はどれも元のテンプレートを変えず、新しい
// テンプレートを返す (エディタのundoは前の版をそのまま積むだけで済む)。
// 保存可否の最終判断はRust (`frame_template::save` → `FrameTemplate::validate`) が行い、
// ここの`frameChecks`は同じ規則を画面に先に見せるための写し。

import type { FrameCell, FrameGridLine, FrameTemplate } from "./frameTemplate";
import { titleBlockOrigin } from "./frameTemplate";

export interface Pt {
  x: number;
  y: number;
}

/** 左のツール列 (デザインの8項目)。 */
export type FrameTool = "select" | "line" | "rect" | "text" | "placeholder" | "cell" | "logo" | "zone";

export const FRAME_TOOLS: FrameTool[] = ["select", "line", "rect", "text", "placeholder", "cell", "logo", "zone"];

/** 選べる要素。罫線・セルは配列の位置で指す。 */
export type FrameItem = { kind: "line"; index: number } | { kind: "cell"; index: number } | { kind: "logo" };

/** プレースホルダの分類 (選択リストの見出し)。 */
export type PlaceholderGroup = "project" | "sheet" | "titleBlock";

export interface Placeholder {
  /** バインド名 (`{}`の中身)。 */
  name: string;
  group: PlaceholderGroup;
}

/** 選べるプレースホルダ (Rust `bind_value`が知っている名前。別名は除く)。 */
export const PLACEHOLDERS: Placeholder[] = [
  { name: "project.name", group: "project" },
  { name: "project.no", group: "project" },
  { name: "sheet.no", group: "sheet" },
  { name: "sheet.total", group: "sheet" },
  { name: "sheet.name", group: "sheet" },
  { name: "rev.latest", group: "titleBlock" },
  { name: "title_block.company", group: "titleBlock" },
  { name: "title_block.title", group: "titleBlock" },
  { name: "title_block.drawing_no", group: "titleBlock" },
  { name: "title_block.scale", group: "titleBlock" },
  { name: "title_block.date", group: "titleBlock" },
  { name: "title_block.designed", group: "titleBlock" },
  { name: "title_block.drawn", group: "titleBlock" },
  { name: "title_block.checked", group: "titleBlock" },
  { name: "title_block.approved", group: "titleBlock" },
  { name: "title_block.rev", group: "titleBlock" },
];

/** バインドとして通る名前 (選択リスト+Rust側の別名)。 */
const KNOWN_BINDS = new Set([...PLACEHOLDERS.map((p) => p.name), "scale", "date", "designer", "approver"]);

/** 新しく置く文字・プレースホルダの既定の文字高 mm。 */
export const DEFAULT_TEXT_MM = 3.5;
/** 表題欄セルの文字の左余白 mm。 */
export const CELL_PAD = 2;

/** 検索語でプレースホルダを絞る (名前の部分一致、大文字小文字は無視)。 */
export function filterPlaceholders(query: string): Placeholder[] {
  const q = query.trim().toLowerCase();
  return q ? PLACEHOLDERS.filter((p) => p.name.toLowerCase().includes(q)) : PLACEHOLDERS;
}

/** 文字列の中の`{名前}`を出てくる順に返す。 */
export function bindsIn(text: string): string[] {
  const out: string[] = [];
  const re = /\{([^{}]*)\}/g;
  for (let m = re.exec(text); m; m = re.exec(text)) out.push(m[1]);
  return out;
}

/** 1mmグリッドへ丸める (Altで外す)。 */
export function snapMm(p: Pt, snap = true): Pt {
  return snap ? { x: Math.round(p.x), y: Math.round(p.y) } : p;
}

/** Shiftで水平/垂直に拘束した線の終点 (変化の大きいほうの軸だけを残す)。 */
export function constrainHV(a: Pt, b: Pt): Pt {
  return Math.abs(b.x - a.x) >= Math.abs(b.y - a.y) ? { x: b.x, y: a.y } : { x: a.x, y: b.y };
}

/** 表題欄の左上角 (用紙座標)。テンプレートの想定用紙で計算する。 */
export function tbOrigin(t: FrameTemplate): Pt {
  return titleBlockOrigin(t, t.paper.w, t.paper.h);
}

const round = (v: number) => Math.round(v * 1000) / 1000;

/** 用紙座標 → 表題欄の相対座標。 */
export function toTb(t: FrameTemplate, p: Pt): Pt {
  const o = tbOrigin(t);
  return { x: round(p.x - o.x), y: round(p.y - o.y) };
}

/** 表題欄の相対座標 → 用紙座標。 */
export function fromTb(t: FrameTemplate, p: Pt): Pt {
  const o = tbOrigin(t);
  return { x: round(p.x + o.x), y: round(p.y + o.y) };
}

function clone(t: FrameTemplate): FrameTemplate {
  return JSON.parse(JSON.stringify(t)) as FrameTemplate;
}

/** 線ツール: 2点の罫線を足す (同じ点ならそのまま返す)。 */
export function addLine(t: FrameTemplate, a: Pt, b: Pt): FrameTemplate {
  if (a.x === b.x && a.y === b.y) return t;
  const next = clone(t);
  const p = toTb(t, a);
  const q = toTb(t, b);
  next.title_block.grid_lines.push({ x1: p.x, y1: p.y, x2: q.x, y2: q.y });
  return next;
}

function rectLines(t: FrameTemplate, a: Pt, b: Pt): FrameGridLine[] {
  const p = toTb(t, { x: Math.min(a.x, b.x), y: Math.min(a.y, b.y) });
  const q = toTb(t, { x: Math.max(a.x, b.x), y: Math.max(a.y, b.y) });
  return [
    { x1: p.x, y1: p.y, x2: q.x, y2: p.y },
    { x1: q.x, y1: p.y, x2: q.x, y2: q.y },
    { x1: p.x, y1: q.y, x2: q.x, y2: q.y },
    { x1: p.x, y1: p.y, x2: p.x, y2: q.y },
  ];
}

const flat = (a: Pt, b: Pt) => a.x === b.x || a.y === b.y;

/** 矩形ツール: 対角の2点から4本の罫線を足す (幅か高さが0ならそのまま返す)。 */
export function addRect(t: FrameTemplate, a: Pt, b: Pt): FrameTemplate {
  if (flat(a, b)) return t;
  const next = clone(t);
  next.title_block.grid_lines.push(...rectLines(t, a, b));
  return next;
}

/** 文字・プレースホルダツール: クリック位置 (文字のベースライン) にセルを足し、その番号を返す。 */
export function addTextCell(t: FrameTemplate, p: Pt, content: { text: string } | { bind: string }): { template: FrameTemplate; index: number } {
  const next = clone(t);
  const q = toTb(t, p);
  next.title_block.cells.push({ x: q.x, y: q.y, size: DEFAULT_TEXT_MM, ...content });
  return { template: next, index: next.title_block.cells.length - 1 };
}

/**
 * 表題欄セルツール: 矩形の中に大きさ付きのバインドセルを置き、外周の罫線も引く。
 * 文字は左余白`CELL_PAD`、高さ方向は中央 (ベースライン=中心+文字高の0.35倍)。
 */
export function addTitleCell(t: FrameTemplate, a: Pt, b: Pt, bind = "{project.name}"): { template: FrameTemplate; index: number } {
  if (flat(a, b)) return { template: t, index: -1 };
  const next = clone(t);
  const x0 = Math.min(a.x, b.x);
  const y0 = Math.min(a.y, b.y);
  const w = Math.abs(b.x - a.x);
  const h = Math.abs(b.y - a.y);
  const size = Math.min(DEFAULT_TEXT_MM, Math.max(1, h * 0.6));
  const anchor = toTb(t, { x: x0 + CELL_PAD, y: y0 + h / 2 + size * 0.35 });
  next.title_block.grid_lines.push(...rectLines(t, a, b));
  next.title_block.cells.push({ x: anchor.x, y: round(anchor.y), w: round(w), h: round(h), bind, size: round(size) });
  return { template: next, index: next.title_block.cells.length - 1 };
}

/** ロゴ枠ツール: 矩形をロゴ枠にする (テンプレートに1つ。あれば置き換える)。 */
export function setLogo(t: FrameTemplate, a: Pt, b: Pt): FrameTemplate {
  if (flat(a, b)) return t;
  const next = clone(t);
  next.logo = {
    x: Math.min(a.x, b.x),
    y: Math.min(a.y, b.y),
    w: Math.abs(b.x - a.x),
    h: Math.abs(b.y - a.y),
    path: t.logo?.path ?? null,
  };
  return next;
}

/** 要素の外形 (用紙座標)。罫線は2点を囲む矩形、文字だけのセルは文字の大まかな幅。 */
export function itemBounds(t: FrameTemplate, item: FrameItem): { x: number; y: number; w: number; h: number } | null {
  if (item.kind === "logo") return t.logo ? { x: t.logo.x, y: t.logo.y, w: t.logo.w, h: t.logo.h } : null;
  if (item.kind === "line") {
    const g = t.title_block.grid_lines[item.index];
    if (!g) return null;
    const a = fromTb(t, { x: g.x1, y: g.y1 });
    const b = fromTb(t, { x: g.x2, y: g.y2 });
    return { x: Math.min(a.x, b.x), y: Math.min(a.y, b.y), w: Math.abs(b.x - a.x), h: Math.abs(b.y - a.y) };
  }
  const c = t.title_block.cells[item.index];
  if (!c) return null;
  const p = fromTb(t, c);
  const label = c.text ?? c.bind ?? "";
  const w = c.w ?? Math.max(c.size, label.length * c.size * 0.6);
  const left = c.w != null ? p.x - CELL_PAD : c.align === "end" ? p.x - w : c.align === "middle" ? p.x - w / 2 : p.x;
  const top = c.h != null ? p.y - (c.h / 2 + c.size * 0.35) : p.y - c.size;
  return { x: left, y: top, w, h: c.h ?? c.size * 1.3 };
}

function distToSegment(p: Pt, a: Pt, b: Pt): number {
  const len2 = (b.x - a.x) ** 2 + (b.y - a.y) ** 2;
  const k = len2 === 0 ? 0 : Math.max(0, Math.min(1, ((p.x - a.x) * (b.x - a.x) + (p.y - a.y) * (b.y - a.y)) / len2));
  return Math.hypot(p.x - (a.x + k * (b.x - a.x)), p.y - (a.y + k * (b.y - a.y)));
}

/**
 * クリック位置の要素。罫線は線の近く (`tolerance`以内) を先に拾う (セルの枠と重なるため)、
 * 次にセルの外形の中、最後にロゴ枠。無ければnull。
 */
export function hitItem(t: FrameTemplate, p: Pt, tolerance: number): FrameItem | null {
  for (let i = t.title_block.grid_lines.length - 1; i >= 0; i--) {
    const g = t.title_block.grid_lines[i];
    if (distToSegment(p, fromTb(t, { x: g.x1, y: g.y1 }), fromTb(t, { x: g.x2, y: g.y2 })) <= tolerance) {
      return { kind: "line", index: i };
    }
  }
  const inside = (r: { x: number; y: number; w: number; h: number } | null) =>
    !!r && p.x >= r.x && p.x <= r.x + r.w && p.y >= r.y && p.y <= r.y + r.h;
  for (let i = t.title_block.cells.length - 1; i >= 0; i--) {
    if (inside(itemBounds(t, { kind: "cell", index: i }))) return { kind: "cell", index: i };
  }
  return inside(itemBounds(t, { kind: "logo" })) ? { kind: "logo" } : null;
}

/** 要素を(dx, dy) mm動かす。 */
export function moveItem(t: FrameTemplate, item: FrameItem, dx: number, dy: number): FrameTemplate {
  if (dx === 0 && dy === 0) return t;
  const next = clone(t);
  if (item.kind === "logo") {
    if (next.logo) {
      next.logo.x = round(next.logo.x + dx);
      next.logo.y = round(next.logo.y + dy);
    }
  } else if (item.kind === "line") {
    const g = next.title_block.grid_lines[item.index];
    if (g) Object.assign(g, { x1: round(g.x1 + dx), y1: round(g.y1 + dy), x2: round(g.x2 + dx), y2: round(g.y2 + dy) });
  } else {
    const c = next.title_block.cells[item.index];
    if (c) Object.assign(c, { x: round(c.x + dx), y: round(c.y + dy) });
  }
  return next;
}

/** 要素を消す。 */
export function deleteItem(t: FrameTemplate, item: FrameItem): FrameTemplate {
  const next = clone(t);
  if (item.kind === "logo") next.logo = null;
  else if (item.kind === "line") next.title_block.grid_lines.splice(item.index, 1);
  else next.title_block.cells.splice(item.index, 1);
  return next;
}

/** セルの欄を書き換える (固定文字とバインドはどちらか一方だけを残す)。 */
export function updateCell(t: FrameTemplate, index: number, patch: Partial<FrameCell>): FrameTemplate {
  const next = clone(t);
  const c = next.title_block.cells[index];
  if (!c) return t;
  Object.assign(c, patch);
  if (patch.text != null) c.bind = null;
  if (patch.bind != null) c.text = null;
  return next;
}

/** 罫線の端点を書き換える (表題欄の相対座標)。 */
export function updateLine(t: FrameTemplate, index: number, patch: Partial<FrameGridLine>): FrameTemplate {
  const next = clone(t);
  const g = next.title_block.grid_lines[index];
  if (!g) return t;
  Object.assign(g, patch);
  return next;
}

/** 改訂欄の列数 (記号/日付/内容/承認)。Rust `REVISION_COLUMNS`。 */
export const REVISION_COLUMNS = 4;

/** 保存前チェックの結果。`errors`があると保存できない (Rust validateと同じ規則)。 */
export interface FrameChecks {
  errors: string[];
  /** 知らないプレースホルダ (保存はできるが、図面では空欄になる)。 */
  unknownBinds: string[];
  /** 図枠の外枠より外にある要素の数 (保存はできる)。 */
  outside: number;
}

/** 保存前チェック (画面下部の1行)。 */
export function frameChecks(t: FrameTemplate): FrameChecks {
  const errors: string[] = [];
  if (!t.id.trim()) errors.push("id");
  if (!(t.paper.w > 0 && t.paper.h > 0)) errors.push("paper");
  if (!(t.frame_margin >= 0)) errors.push("frame_margin");
  if (!(t.zones.cols >= 1 && t.zones.rows >= 1)) errors.push("zones");
  const tb = t.title_block;
  if (!(tb.w > 0 && tb.h > 0)) errors.push("title_block");
  if (!(tb.row_h > 0)) errors.push("title_block.row_h");
  tb.cells.forEach((c, i) => {
    const hasText = c.text != null;
    const hasBind = c.bind != null;
    if (hasText === hasBind) errors.push(`cells[${i}]`);
    if (!(c.size > 0)) errors.push(`cells[${i}].size`);
  });
  const cols = t.revision_block.col_w;
  if (cols.length !== REVISION_COLUMNS) errors.push("revision_block.col_w");
  else if (Math.abs(cols.reduce((a, b) => a + b, 0) - tb.w) > 1e-6) errors.push("revision_block.col_w");

  const unknownBinds: string[] = [];
  for (const c of tb.cells) {
    for (const name of bindsIn(c.bind ?? "")) {
      if (!KNOWN_BINDS.has(name) && !unknownBinds.includes(name)) unknownBinds.push(name);
    }
  }

  const m = t.frame_margin;
  const within = (p: Pt) => p.x >= m - 1e-6 && p.x <= t.paper.w - m + 1e-6 && p.y >= m - 1e-6 && p.y <= t.paper.h - m + 1e-6;
  let outside = 0;
  for (const g of tb.grid_lines) {
    if (!within(fromTb(t, { x: g.x1, y: g.y1 })) || !within(fromTb(t, { x: g.x2, y: g.y2 }))) outside++;
  }
  for (const c of tb.cells) if (!within(fromTb(t, c))) outside++;
  if (t.logo && (!within(t.logo) || !within({ x: t.logo.x + t.logo.w, y: t.logo.y + t.logo.h }))) outside++;
  return { errors, unknownBinds, outside };
}

/** 別名で保存するときの既定のid (元のid+`-copy`、空きが出るまで番号を足す)。 */
export function copyId(base: string, taken: readonly string[]): string {
  const stem = `${base || "frame"}-copy`;
  if (!taken.includes(stem)) return stem;
  for (let n = 2; ; n++) if (!taken.includes(`${stem}${n}`)) return `${stem}${n}`;
}

/** idとしてファイル名にできるか (英数字・`-`・`_`。Rustの保存と同じ)。 */
export function isValidFrameId(id: string): boolean {
  return /^[A-Za-z0-9_-]+$/.test(id.trim());
}
