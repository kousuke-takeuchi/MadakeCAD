<script setup lang="ts">
// 図枠エディタの作図キャンバス (.pen「M4残りデザイン⑥」`dhP9P`の中央)。
//
// 白い用紙 (テンプレートの想定用紙) に図枠・ゾーン目盛・改訂欄の見出し・表題欄の罫線と
// セルを描き、プレースホルダは青い`{...}`で見せる (隠す/実際の値で見せるに切り替え可)。
// クリック位置は1mmグリッドへ吸い付く (Altで外す)。線はShiftで水平/垂直。
//
// - 選択: クリックで掴む / ドラッグで動かす (離したときに1段だけ積む)
// - 線・矩形・表題欄セル・ロゴ枠: ドラッグで1つ描く
// - 文字・プレースホルダ: クリック位置に置いて、そのまま選択する
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  addLine,
  addRect,
  addTextCell,
  addTitleCell,
  constrainHV,
  fromTb,
  hitItem,
  itemBounds,
  moveItem,
  setLogo,
  snapMm,
  tbOrigin,
  type FrameItem,
  type Pt,
} from "../canvas/frameEditor";
import { REV_HEADERS, resolveBinds, zoneLabel, type FrameTemplate } from "../canvas/frameTemplate";
import { frameEditorTheme as ink } from "../canvas/theme";
import { Viewport } from "../canvas/viewport";
import { useDocumentStore } from "../stores/document";
import { useFrameEditorStore } from "../stores/frameEditor";

const editor = useFrameEditorStore();
const doc = useDocumentStore();
const { t } = useI18n();

const wrapRef = ref<HTMLDivElement | null>(null);
const canvasRef = ref<HTMLCanvasElement | null>(null);
const vp = new Viewport();
let observer: ResizeObserver | null = null;

/** ドラッグ中 (選択ツール=移動、作図ツール=ラバーバンド)。 */
interface Drag {
  from: Pt;
  to: Pt;
  item: FrameItem | null;
}
const drag = ref<Drag | null>(null);

function fit() {
  const canvas = canvasRef.value;
  const draft = editor.draft;
  if (!canvas || !draft) return;
  vp.fit(draft.paper.w, draft.paper.h, canvas.width, canvas.height, 28);
}

function resize() {
  const wrap = wrapRef.value;
  const canvas = canvasRef.value;
  if (!wrap || !canvas) return;
  canvas.width = wrap.clientWidth;
  canvas.height = wrap.clientHeight;
  fit();
  paint();
}

function worldOf(ev: MouseEvent): Pt {
  const canvas = canvasRef.value;
  if (!canvas) return { x: 0, y: 0 };
  const r = canvas.getBoundingClientRect();
  return vp.toWorld({ x: ev.clientX - r.left, y: ev.clientY - r.top });
}

/** ドラッグ中に見せる下書き (移動はまだ積まずに描くだけ)。 */
function shownTemplate(): FrameTemplate | null {
  const draft = editor.draft;
  const d = drag.value;
  if (!draft || !d?.item) return draft;
  return moveItem(draft, d.item, d.to.x - d.from.x, d.to.y - d.from.y);
}

/** セルに描く文字。固定文字はそのまま、プレースホルダは表示切替に従う。 */
function cellLabel(text: string | null | undefined, bind: string | null | undefined): { text: string; bound: boolean } | null {
  if (text != null) return { text, bound: false };
  if (bind == null || editor.hidePlaceholders) return null;
  if (editor.showValues) {
    const sheet = doc.activeSheet;
    return sheet ? { text: resolveBinds(bind, doc.bindContextFor(sheet)), bound: false } : { text: bind, bound: true };
  }
  return { text: bind, bound: true };
}

function line(ctx: CanvasRenderingContext2D, a: Pt, b: Pt) {
  const p = vp.toScreen(a);
  const q = vp.toScreen(b);
  ctx.beginPath();
  ctx.moveTo(p.x, p.y);
  ctx.lineTo(q.x, q.y);
  ctx.stroke();
}

function rect(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number) {
  const p = vp.toScreen({ x, y });
  ctx.strokeRect(p.x, p.y, w * vp.scale, h * vp.scale);
}

function text(ctx: CanvasRenderingContext2D, s: string, at: Pt, sizeMm: number, align: CanvasTextAlign = "left", bold = false) {
  const p = vp.toScreen(at);
  ctx.textAlign = align;
  ctx.font = `${bold ? "bold " : ""}${Math.max(7, sizeMm * vp.scale)}px sans-serif`;
  ctx.fillText(s, p.x, p.y);
}

function paint() {
  const canvas = canvasRef.value;
  const ctx = canvas?.getContext("2d");
  const tpl = shownTemplate();
  if (!canvas || !ctx || !tpl) return;
  const { w: pw, h: ph } = tpl.paper;
  const m = tpl.frame_margin;
  ctx.fillStyle = ink.background;
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  const o = vp.toScreen({ x: 0, y: 0 });
  ctx.fillStyle = ink.paper;
  ctx.fillRect(o.x, o.y, pw * vp.scale, ph * vp.scale);

  // 10mmごとの補助グリッド
  ctx.strokeStyle = ink.grid;
  ctx.lineWidth = 1;
  for (let x = 10; x < pw; x += 10) line(ctx, { x, y: 0 }, { x, y: ph });
  for (let y = 10; y < ph; y += 10) line(ctx, { x: 0, y }, { x: pw, y });

  // 図枠とゾーン目盛 (分割数はテンプレートの既定値)
  ctx.strokeStyle = ink.line;
  ctx.lineWidth = 1.5;
  rect(ctx, m, m, pw - 2 * m, ph - 2 * m);
  ctx.lineWidth = 1;
  ctx.fillStyle = ink.dim;
  const cols = Math.max(1, tpl.zones.cols);
  const rows = Math.max(1, tpl.zones.rows);
  const zw = (pw - 2 * m) / cols;
  const zh = (ph - 2 * m) / rows;
  ctx.strokeStyle = ink.dim;
  for (let i = 0; i < cols; i++) {
    if (i > 0) line(ctx, { x: m + i * zw, y: m }, { x: m + i * zw, y: m * 0.4 });
    text(ctx, zoneLabel(tpl.zones.col_labels, i), { x: m + (i + 0.5) * zw, y: m * 0.75 }, Math.min(3, m * 0.5), "center");
  }
  for (let j = 0; j < rows; j++) {
    if (j > 0) line(ctx, { x: m, y: m + j * zh }, { x: m * 0.4, y: m + j * zh });
    text(ctx, zoneLabel(tpl.zones.row_labels, j), { x: m * 0.5, y: m + (j + 0.5) * zh + 1 }, Math.min(3, m * 0.5), "center");
  }

  // 改訂欄の見出し行 (表題欄の直上。中身は図面の改訂が入るので見出しだけ)
  const tb = tbOrigin(tpl);
  const rowH = tpl.title_block.row_h;
  let x = tb.x;
  ctx.strokeStyle = ink.dim;
  rect(ctx, tb.x, tb.y - rowH, tpl.title_block.w, rowH);
  tpl.revision_block.col_w.forEach((w, i) => {
    if (i > 0) line(ctx, { x, y: tb.y - rowH }, { x, y: tb.y });
    text(ctx, REV_HEADERS[i] ?? "", { x: x + w / 2, y: tb.y - rowH / 2 + 1 }, 2.5, "center");
    x += w;
  });

  // 表題欄: 外形・罫線・セル
  ctx.strokeStyle = ink.line;
  ctx.lineWidth = 1.5;
  rect(ctx, tb.x, tb.y, tpl.title_block.w, tpl.title_block.h);
  ctx.lineWidth = 1;
  for (const g of tpl.title_block.grid_lines) line(ctx, fromTb(tpl, { x: g.x1, y: g.y1 }), fromTb(tpl, { x: g.x2, y: g.y2 }));
  for (const c of tpl.title_block.cells) {
    const label = cellLabel(c.text, c.bind);
    if (!label) continue;
    ctx.fillStyle = label.bound ? ink.placeholder : ink.line;
    const align: CanvasTextAlign = c.align === "middle" ? "center" : c.align === "end" ? "right" : "left";
    text(ctx, label.text, fromTb(tpl, c), c.size, align, c.weight === "bold");
  }

  // ロゴ枠
  if (tpl.logo) {
    ctx.strokeStyle = ink.placeholder;
    ctx.setLineDash([4, 3]);
    rect(ctx, tpl.logo.x, tpl.logo.y, tpl.logo.w, tpl.logo.h);
    ctx.setLineDash([]);
    ctx.fillStyle = ink.placeholder;
    text(ctx, "{logo}", { x: tpl.logo.x + 1.5, y: tpl.logo.y + Math.min(tpl.logo.h - 1, 4.5) }, 3);
  }

  // 選択
  const selected = editor.selection ? itemBounds(tpl, drag.value?.item ?? editor.selection) : null;
  if (selected) {
    const p = vp.toScreen(selected);
    const w = Math.max(4, selected.w * vp.scale);
    const h = Math.max(4, selected.h * vp.scale);
    ctx.fillStyle = ink.selectionFill;
    ctx.fillRect(p.x, p.y, w, h);
    ctx.strokeStyle = ink.selection;
    ctx.strokeRect(p.x, p.y, w, h);
    ctx.fillStyle = ink.paper;
    for (const [hx, hy] of [[p.x, p.y], [p.x + w, p.y], [p.x, p.y + h], [p.x + w, p.y + h]]) {
      ctx.fillRect(hx - 3, hy - 3, 6, 6);
      ctx.strokeRect(hx - 3, hy - 3, 6, 6);
    }
  }

  // ドラッグ中のラバーバンド
  const d = drag.value;
  if (d && !d.item) {
    ctx.strokeStyle = ink.selection;
    ctx.setLineDash([4, 3]);
    if (editor.tool === "line") line(ctx, d.from, d.to);
    else rect(ctx, Math.min(d.from.x, d.to.x), Math.min(d.from.y, d.to.y), Math.abs(d.to.x - d.from.x), Math.abs(d.to.y - d.from.y));
    ctx.setLineDash([]);
  }

  // 原点 (用紙の左上、アプリ全体と同じ座標系)
  ctx.strokeStyle = ink.origin;
  ctx.lineWidth = 1.5;
  ctx.beginPath();
  ctx.arc(o.x, o.y, 5, 0, Math.PI * 2);
  ctx.stroke();
  ctx.lineWidth = 1;
  ctx.fillStyle = ink.origin;
  ctx.textAlign = "left";
  ctx.font = "11px sans-serif";
  ctx.fillText(t("frameEditor.origin"), o.x + 8, o.y - 6);
}

function pointOf(ev: MouseEvent, from?: Pt): Pt {
  const p = snapMm(worldOf(ev), !ev.altKey);
  return from && ev.shiftKey && editor.tool === "line" ? constrainHV(from, p) : p;
}

function onMouseDown(ev: MouseEvent) {
  const draft = editor.draft;
  if (!draft || ev.button !== 0) return;
  const at = pointOf(ev);
  switch (editor.tool) {
    case "select": {
      const hit = hitItem(draft, worldOf(ev), 4 / vp.scale);
      editor.select(hit);
      drag.value = hit ? { from: at, to: at, item: hit } : null;
      break;
    }
    case "text":
    case "placeholder": {
      const content = editor.tool === "text" ? { text: t("frameEditor.newText") } : { bind: "{project.name}" };
      const { template, index } = addTextCell(draft, at, content);
      editor.commit(template);
      editor.setTool("select");
      editor.select({ kind: "cell", index });
      break;
    }
    case "zone":
      break;
    default:
      drag.value = { from: at, to: at, item: null };
  }
  paint();
}

function onMouseMove(ev: MouseEvent) {
  editor.cursor = snapMm(worldOf(ev), !ev.altKey);
  if (!drag.value) return;
  drag.value = { ...drag.value, to: pointOf(ev, drag.value.from) };
  paint();
}

function onMouseUp() {
  const d = drag.value;
  const draft = editor.draft;
  drag.value = null;
  if (!d || !draft) return;
  if (d.item) {
    editor.commit(moveItem(draft, d.item, d.to.x - d.from.x, d.to.y - d.from.y));
  } else if (editor.tool === "line") {
    editor.commit(addLine(draft, d.from, d.to));
  } else if (editor.tool === "rect") {
    editor.commit(addRect(draft, d.from, d.to));
  } else if (editor.tool === "logo") {
    editor.commit(setLogo(draft, d.from, d.to));
    editor.select({ kind: "logo" });
  } else if (editor.tool === "cell") {
    const { template, index } = addTitleCell(draft, d.from, d.to);
    if (index >= 0) {
      editor.commit(template);
      editor.setTool("select");
      editor.select({ kind: "cell", index });
    }
  }
  paint();
}

function onWheel(ev: WheelEvent) {
  const canvas = canvasRef.value;
  if (!canvas) return;
  const r = canvas.getBoundingClientRect();
  vp.zoomAt({ x: ev.clientX - r.left, y: ev.clientY - r.top }, ev.deltaY < 0 ? 1.15 : 1 / 1.15);
  paint();
}

onMounted(() => {
  observer = new ResizeObserver(resize);
  if (wrapRef.value) observer.observe(wrapRef.value);
  resize();
});
onBeforeUnmount(() => observer?.disconnect());

watch(
  () => [editor.draft, editor.selection, editor.hidePlaceholders, editor.showValues, editor.tool],
  () => paint(),
  { deep: true, flush: "post" },
);
watch(
  () => editor.draft?.paper,
  () => {
    fit();
    paint();
  },
  { deep: true },
);

defineExpose({ fit: () => { fit(); paint(); } });
</script>

<template>
  <div ref="wrapRef" class="wrap">
    <canvas
      ref="canvasRef"
      :class="{ crosshair: editor.tool !== 'select' }"
      @mousedown="onMouseDown"
      @mousemove="onMouseMove"
      @mouseup="onMouseUp"
      @mouseleave="onMouseUp"
      @wheel.prevent="onWheel"
    />
  </div>
</template>

<style scoped>
.wrap {
  position: relative;
  flex: 1;
  min-height: 0;
  overflow: hidden;
}
canvas {
  position: absolute;
  inset: 0;
  display: block;
}
canvas.crosshair {
  cursor: crosshair;
}
</style>
