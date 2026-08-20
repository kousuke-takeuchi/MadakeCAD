<script setup lang="ts">
import { inject, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { renderSheet } from "../canvas/renderer";
import type { EditorController } from "../tools/controller";
import { useDocumentStore } from "../stores/document";

const store = useDocumentStore();
const controller = inject<EditorController>("controller")!;
const canvasRef = ref<HTMLCanvasElement | null>(null);
const wrapRef = ref<HTMLDivElement | null>(null);
let observer: ResizeObserver | null = null;
let raf = 0;

function draw() {
  raf = 0;
  const canvas = canvasRef.value;
  const sheet = store.activeSheet;
  if (!canvas || !sheet) return;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  renderSheet(ctx, sheet, store.symbols, controller.vp, {
    selection: store.selection,
    cursor: controller.cursorScreen,
  });
  controller.renderPreview(ctx);
}

function scheduleDraw() {
  if (!raf) raf = requestAnimationFrame(draw);
}

let fitted = false;

function resize() {
  const canvas = canvasRef.value;
  const wrap = wrapRef.value;
  if (!canvas || !wrap) return;
  canvas.width = wrap.clientWidth;
  canvas.height = wrap.clientHeight;
  if (!fitted && canvas.width > 0 && store.activeSheet) {
    const { w, h } = paperSize();
    controller.vp.fit(w, h, canvas.width, canvas.height);
    fitted = true;
  }
  scheduleDraw();
}

function paperSize() {
  const sheet = store.activeSheet!;
  const dims: Record<string, [number, number]> = {
    A4: [297, 210], A3: [420, 297], A2: [594, 420], A1: [841, 594], A0: [1189, 841],
  };
  const [w, h] = dims[sheet.size] ?? [420, 297];
  return sheet.orientation === "Portrait" ? { w: h, h: w } : { w, h };
}

function toLocal(ev: PointerEvent | WheelEvent | MouseEvent) {
  const rect = canvasRef.value!.getBoundingClientRect();
  return { x: ev.clientX - rect.left, y: ev.clientY - rect.top };
}

onMounted(() => {
  controller.requestRedraw = scheduleDraw;
  observer = new ResizeObserver(resize);
  if (wrapRef.value) observer.observe(wrapRef.value);
  resize();
});

onBeforeUnmount(() => {
  observer?.disconnect();
  if (raf) cancelAnimationFrame(raf);
});

store.$subscribe(() => scheduleDraw());
watch(() => store.activeSheetId, () => {
  const canvas = canvasRef.value;
  if (canvas && store.activeSheet) {
    const { w, h } = paperSize();
    controller.vp.fit(w, h, canvas.width, canvas.height);
  }
  scheduleDraw();
});
</script>

<template>
  <div ref="wrapRef" class="canvas-wrap">
    <canvas
      ref="canvasRef"
      tabindex="0"
      @pointerdown="(e) => { canvasRef?.focus(); canvasRef?.setPointerCapture(e.pointerId); controller.onPointerDown(toLocal(e), e); }"
      @pointermove="(e) => controller.onPointerMove(toLocal(e))"
      @pointerup="() => controller.onPointerUp()"
      @pointerleave="() => { controller.cursorScreen = null; controller.requestRedraw(); }"
      @wheel.prevent="(e) => controller.onWheel(toLocal(e), e.deltaY)"
      @dblclick="() => controller.onDoubleClick()"
      @contextmenu.prevent
    />
  </div>
</template>

<style scoped>
.canvas-wrap {
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  position: relative;
}
canvas {
  display: block;
  outline: none;
  cursor: none;
}
</style>
