<script setup lang="ts">
// シンボルエディタの作図キャンバス (.pen「M4デザイン - シンボルエディタ」の中央)。
//
// 白紙+2.5mmグリッド+原点マークの上に、下書きの図形・接続点 (赤ドット)・属性テキストの
// 見本を実際の配置位置で描く。クリック位置は必ずグリッドの交点へ吸い付く。
//
// 操作はこのフェーズでは最小限:
// - 選択ツール: クリックで掴む / ドラッグで動かす / Delete で消す (頂点編集は無い)
// - 線・矩形・円・弧: ドラッグで1つ描く
// - 文字: 座標バーの入力欄の文字をクリック位置へ置く
// - 属性・ピン: クリック位置へ置く (属性は未配置のものを順に、ピンは向きを自動で決める)
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { GRID_PITCH, draftBounds, snapToGrid } from "../canvas/symbolEditor";
import { symbolEditorTheme as ink } from "../canvas/theme";
import { Viewport, type Pt } from "../canvas/viewport";
import type { PinDir, Point, Primitive } from "../ipc";
import { useSymbolEditorStore } from "../stores/symbolEditor";

const editor = useSymbolEditorStore();
const { t } = useI18n();

const wrapRef = ref<HTMLDivElement | null>(null);
const canvasRef = ref<HTMLCanvasElement | null>(null);
/** 文字ツールで置く文字列 (座標バーの入力欄)。 */
const textValue = ref("TEXT");

const vp = new Viewport();
vp.scale = 8;
let observer: ResizeObserver | null = null;

/** ドラッグ中の状態 (選択ツール=移動、作図ツール=ラバーバンド)。 */
interface Drag {
  from: Point;
  to: Point;
  moving: boolean;
}
const drag = ref<Drag | null>(null);

/** 座標バーに出す現在位置。 */
const cursorText = computed(() => {
  const c = editor.cursor;
  return c ? t("symbolEditor.cursor", { x: c.x.toFixed(1), y: c.y.toFixed(1) }) : "";
});

function ctxOf(): CanvasRenderingContext2D | null {
  return canvasRef.value?.getContext("2d") ?? null;
}

/** キャンバスの中央に原点が来るように合わせる。 */
function center() {
  const canvas = canvasRef.value;
  if (!canvas) return;
  vp.originX = canvas.width / 2;
  vp.originY = canvas.height / 2;
}

function resize() {
  const wrap = wrapRef.value;
  const canvas = canvasRef.value;
  if (!wrap || !canvas) return;
  canvas.width = wrap.clientWidth;
  canvas.height = wrap.clientHeight;
  center();
  paint();
}

/** マウス位置 → 用紙座標 (mm)。 */
function worldOf(ev: MouseEvent): Point {
  const canvas = canvasRef.value;
  if (!canvas) return { x: 0, y: 0 };
  const rect = canvas.getBoundingClientRect();
  const w = vp.toWorld({ x: ev.clientX - rect.left, y: ev.clientY - rect.top });
  return { x: w.x, y: w.y };
}

/** ドラッグで描いた図形 (プレビューにも確定にも使う)。 */
function primitiveFromDrag(d: Drag): Primitive | null {
  const { from, to } = d;
  switch (editor.tool) {
    case "line":
      if (from.x === to.x && from.y === to.y) return null;
      return { type: "line", pts: [from, to] };
    case "rect":
      if (from.x === to.x && from.y === to.y) return null;
      return { type: "rect", p1: from, p2: to, filled: false };
    case "circle": {
      const r = Math.hypot(to.x - from.x, to.y - from.y);
      return r > 0 ? { type: "circle", center: from, r, filled: false } : null;
    }
    case "arc": {
      const r = Math.hypot(to.x - from.x, to.y - from.y);
      if (r <= 0) return null;
      // ドラッグの向きを弧の始点にして、そこから時計回りに90度
      const start = (Math.atan2(to.y - from.y, to.x - from.x) * 180) / Math.PI;
      return { type: "arc", center: from, r, start_deg: start, end_deg: start + 90 };
    }
    default:
      return null;
  }
}

/** クリック位置から接続点の向きを決める (外形の中心から見て遠いほうの軸)。 */
function pinDirFor(at: Point): PinDir {
  if (!editor.draft) return "right";
  const { min, max } = draftBounds(editor.draft);
  const cx = (min.x + max.x) / 2;
  const cy = (min.y + max.y) / 2;
  const dx = at.x - cx;
  const dy = at.y - cy;
  if (Math.abs(dx) >= Math.abs(dy)) return dx >= 0 ? "right" : "left";
  return dy >= 0 ? "down" : "up";
}

function onMouseDown(ev: MouseEvent) {
  if (!editor.draft || ev.button !== 0) return;
  const at = snapToGrid(worldOf(ev));
  if (editor.tool === "select") {
    editor.selectAt(worldOf(ev));
    drag.value = editor.selection ? { from: at, to: at, moving: true } : null;
    return;
  }
  if (editor.tool === "pin") {
    editor.addPinAt(at, pinDirFor(at));
    paint();
    return;
  }
  if (editor.tool === "slot") {
    editor.placeSlotAt(at);
    paint();
    return;
  }
  if (editor.tool === "text") {
    const text = textValue.value.trim();
    if (text) editor.addPrimitive({ type: "text", at, text, height: 2.5 });
    paint();
    return;
  }
  drag.value = { from: at, to: at, moving: false };
}

function onMouseMove(ev: MouseEvent) {
  const at = snapToGrid(worldOf(ev));
  editor.setCursor(at);
  const d = drag.value;
  if (!d) {
    paint();
    return;
  }
  if (d.moving) {
    editor.moveSelection(at.x - d.from.x, at.y - d.from.y);
    d.from = at;
  } else {
    d.to = at;
  }
  paint();
}

function onMouseUp() {
  const d = drag.value;
  drag.value = null;
  if (!d || d.moving) {
    paint();
    return;
  }
  const prim = primitiveFromDrag(d);
  if (prim) editor.addPrimitive(prim);
  paint();
}

function onMouseLeave() {
  editor.setCursor(null);
  drag.value = null;
  paint();
}

function onWheel(ev: WheelEvent) {
  const canvas = canvasRef.value;
  if (!canvas) return;
  const rect = canvas.getBoundingClientRect();
  vp.zoomAt(
    { x: ev.clientX - rect.left, y: ev.clientY - rect.top },
    ev.deltaY < 0 ? 1.1 : 1 / 1.1,
  );
  paint();
}

// --- 描画 ---------------------------------------------------------------

function drawGrid(ctx: CanvasRenderingContext2D, w: number, h: number) {
  const step = GRID_PITCH * vp.scale;
  if (step < 3) return;
  const startX = vp.originX % step;
  const startY = vp.originY % step;
  ctx.lineWidth = 1;
  for (let i = 0; ; i += 1) {
    const x = startX + i * step;
    if (x > w) break;
    const major = Math.abs(Math.round((x - vp.originX) / step) % 5) === 0;
    ctx.strokeStyle = major ? ink.gridMajor : ink.grid;
    ctx.beginPath();
    ctx.moveTo(Math.round(x) + 0.5, 0);
    ctx.lineTo(Math.round(x) + 0.5, h);
    ctx.stroke();
  }
  for (let i = 0; ; i += 1) {
    const y = startY + i * step;
    if (y > h) break;
    const major = Math.abs(Math.round((y - vp.originY) / step) % 5) === 0;
    ctx.strokeStyle = major ? ink.gridMajor : ink.grid;
    ctx.beginPath();
    ctx.moveTo(0, Math.round(y) + 0.5);
    ctx.lineTo(w, Math.round(y) + 0.5);
    ctx.stroke();
  }
}

function drawPrimitive(ctx: CanvasRenderingContext2D, prim: Primitive, color: string) {
  ctx.strokeStyle = color;
  ctx.fillStyle = color;
  ctx.lineWidth = Math.max(1, 0.3 * vp.scale);
  switch (prim.type) {
    case "line": {
      ctx.beginPath();
      prim.pts.forEach((p, i) => {
        const s = vp.toScreen(p);
        if (i === 0) ctx.moveTo(s.x, s.y);
        else ctx.lineTo(s.x, s.y);
      });
      ctx.stroke();
      break;
    }
    case "circle": {
      const c = vp.toScreen(prim.center);
      ctx.beginPath();
      ctx.arc(c.x, c.y, prim.r * vp.scale, 0, Math.PI * 2);
      if (prim.filled) ctx.fill();
      else ctx.stroke();
      break;
    }
    case "arc": {
      const c = vp.toScreen(prim.center);
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
      const a = vp.toScreen(prim.p1);
      const b = vp.toScreen(prim.p2);
      ctx.beginPath();
      ctx.rect(a.x, a.y, b.x - a.x, b.y - a.y);
      if (prim.filled) ctx.fill();
      else ctx.stroke();
      break;
    }
    case "text": {
      const p = vp.toScreen(prim.at);
      ctx.font = `${Math.max(8, prim.height * vp.scale)}px sans-serif`;
      ctx.textAlign = "center";
      ctx.fillText(prim.text, p.x, p.y + (prim.height * vp.scale) / 2.8);
      break;
    }
  }
}

/** 選択枠 (掴んでいるものを青で囲う)。 */
function drawSelectionBox(ctx: CanvasRenderingContext2D, at: Pt, sizePx: number) {
  const s = vp.toScreen(at);
  ctx.strokeStyle = ink.selection;
  ctx.fillStyle = ink.selectionFill;
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.rect(s.x - sizePx, s.y - sizePx, sizePx * 2, sizePx * 2);
  ctx.fill();
  ctx.stroke();
}

function paint() {
  const canvas = canvasRef.value;
  const ctx = ctxOf();
  if (!canvas || !ctx) return;
  const { width: w, height: h } = canvas;
  ctx.fillStyle = ink.paper;
  ctx.fillRect(0, 0, w, h);
  drawGrid(ctx, w, h);

  // 原点 (0,0) の十字
  const o = vp.toScreen({ x: 0, y: 0 });
  ctx.strokeStyle = ink.origin;
  ctx.lineWidth = 1;
  ctx.beginPath();
  ctx.moveTo(o.x - 8, o.y);
  ctx.lineTo(o.x + 8, o.y);
  ctx.moveTo(o.x, o.y - 8);
  ctx.lineTo(o.x, o.y + 8);
  ctx.stroke();
  ctx.fillStyle = ink.origin;
  ctx.font = "10px sans-serif";
  ctx.textAlign = "left";
  ctx.fillText(t("symbolEditor.originMark"), o.x + 10, o.y - 6);

  const def = editor.draft;
  if (!def) return;

  const sel = editor.selection;
  def.primitives.forEach((prim, i) => {
    const on = sel?.kind === "primitive" && sel.index === i;
    drawPrimitive(ctx, prim, on ? ink.selection : ink.line);
  });

  // 作図中のラバーバンド
  const d = drag.value;
  if (d && !d.moving) {
    const preview = primitiveFromDrag(d);
    if (preview) drawPrimitive(ctx, preview, ink.selection);
  }

  // 属性テキストの見本 (配置時に実値へ置き換わる)
  ctx.fillStyle = ink.attr;
  ctx.textAlign = "center";
  (def.text_slots ?? []).forEach((slot, i) => {
    const p = vp.toScreen(slot.at);
    ctx.font = `${Math.max(9, slot.height * vp.scale)}px monospace`;
    ctx.fillStyle = ink.attr;
    ctx.fillText(editor.sampleFor(slot.key), p.x, p.y + (slot.height * vp.scale) / 2.8);
    if (sel?.kind === "slot" && sel.index === i) drawSelectionBox(ctx, slot.at, 8);
  });

  // 接続点 (赤ドット+番号)
  def.pins.forEach((pin, i) => {
    const p = vp.toScreen(pin.at);
    ctx.fillStyle = ink.pin;
    ctx.beginPath();
    ctx.arc(p.x, p.y, 3.5, 0, Math.PI * 2);
    ctx.fill();
    ctx.fillStyle = ink.pinLabel;
    ctx.font = "10px monospace";
    ctx.textAlign = "left";
    ctx.fillText(pin.number, p.x + 6, p.y + 3);
    if (sel?.kind === "pin" && sel.index === i) drawSelectionBox(ctx, pin.at, 6);
  });
}

watch(() => editor.$state, paint, { deep: true });
watch(
  () => editor.open,
  (open) => {
    if (open) requestAnimationFrame(resize);
  },
);

onMounted(() => {
  const wrap = wrapRef.value;
  if (wrap) {
    observer = new ResizeObserver(resize);
    observer.observe(wrap);
  }
  resize();
});
onBeforeUnmount(() => observer?.disconnect());

defineExpose({ resize });
</script>

<template>
  <div class="canvas-col">
    <div ref="wrapRef" class="canvas-wrap">
      <canvas
        ref="canvasRef"
        @mousedown="onMouseDown"
        @mousemove="onMouseMove"
        @mouseup="onMouseUp"
        @mouseleave="onMouseLeave"
        @wheel.prevent="onWheel"
      />
    </div>
    <div class="coord-bar">
      <span class="mono coord">{{ cursorText }}</span>
      <span class="caption">{{ t("symbolEditor.gridHint") }}</span>
      <span class="caption">{{ t("symbolEditor.pinHint") }}</span>
      <span class="spacer" />
      <label v-if="editor.tool === 'text'" class="text-field">
        <span class="caption">{{ t("symbolEditor.textValue") }}</span>
        <input v-model="textValue" class="text-input" spellcheck="false" />
      </label>
    </div>
  </div>
</template>

<style scoped>
.canvas-col {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  background: var(--card-bg);
}
.canvas-wrap {
  flex: 1;
  min-height: 0;
  position: relative;
  overflow: hidden;
}
.canvas-wrap canvas {
  display: block;
  width: 100%;
  height: 100%;
  cursor: crosshair;
}
.coord-bar {
  flex: none;
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 6px 12px;
  border-top: 1px solid var(--ribbon-line);
  background: var(--card-bg);
}
.coord {
  font-size: 11px;
  color: var(--ui-text);
  min-width: 130px;
}
.caption {
  font-size: 10px;
  color: var(--ui-muted);
}
.spacer {
  flex: 1;
}
.text-field {
  display: flex;
  align-items: center;
  gap: 6px;
}
.text-input {
  width: 110px;
  background: var(--input-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  padding: 3px 6px;
  font-size: 11px;
  color: var(--ui-text);
  outline: none;
}
.text-input:focus {
  border-color: var(--acad-blue);
}
.mono {
  font-family: var(--mono-font);
}
</style>
