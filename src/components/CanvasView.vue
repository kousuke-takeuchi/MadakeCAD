<script setup lang="ts">
import { inject, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { renderSheet } from "../canvas/renderer";
import { AgentOverlay } from "../canvas/agentOverlay";
import type { EditorController } from "../tools/controller";
import type { Patch } from "../ipc";
import { useDocumentStore } from "../stores/document";
import { useChatStore } from "../stores/chat";
import ChatPanel from "./chat/ChatPanel.vue";

const store = useDocumentStore();
const chat = useChatStore();
const controller = inject<EditorController>("controller")!;
const canvasRef = ref<HTMLCanvasElement | null>(null);
const wrapRef = ref<HTMLDivElement | null>(null);
let observer: ResizeObserver | null = null;
let raf = 0;

// エージェント編集オーバーレイ。ストリーミング中だけアニメーションを回す。
const overlay = new AgentOverlay();
let overlayRaf = 0;

function draw() {
  raf = 0;
  const canvas = canvasRef.value;
  const sheet = store.activeSheet;
  if (!canvas || !sheet) return;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  const now = Date.now();
  const regions = overlay.activeRegions(now);
  renderSheet(ctx, sheet, store.symbols, controller.vp, {
    selection: store.selection,
    cursor: controller.cursorScreen,
    agent:
      chat.streaming || regions.length > 0
        ? { regions, active: chat.streaming }
        : undefined,
  });
  controller.renderPreview(ctx);
}

function scheduleDraw() {
  if (!raf) raf = requestAnimationFrame(draw);
}

// --- オーバーレイの駆動 -----------------------------------------------------

/** アニメーションを続ける必要があるか(アイドル時はループを止める)。 */
function overlayRunning(): boolean {
  return chat.streaming || overlay.hasActive();
}

function overlayTick() {
  overlayRaf = 0;
  draw();
  if (overlayRunning()) overlayRaf = requestAnimationFrame(overlayTick);
}

function startOverlayAnimation() {
  if (!overlayRaf) overlayRaf = requestAnimationFrame(overlayTick);
}

/** ストリーミング中のターンのツール呼び出しを取り込む。 */
function syncToolCalls() {
  const calls = chat.streamingMessage?.tool_calls ?? [];
  for (const call of calls) {
    if (call.status === "running") overlay.noteToolStart(call.tool, call.input, { id: call.id });
    else overlay.noteToolFinish(call.id || call.tool);
  }
  startOverlayAnimation();
}

watch(
  () => chat.streamingMessage?.tool_calls.map((c) => `${c.id}:${c.status}`).join(",") ?? "",
  () => syncToolCalls(),
);

watch(
  () => chat.streaming,
  (streaming) => {
    if (streaming) syncToolCalls();
    // ターン終了時は進行中の領域を畳む(完了イベントが来なかった場合の保険)
    else overlay.finishAll();
    startOverlayAnimation();
  },
);

// エージェントのターン中に届いたpatchの対象要素も編集箇所として光らせる。
// (document storeは書き換えず、適用済みpatchを覗くだけ)
store.$onAction(({ name, args, after }) => {
  if (name !== "applyPatch") return;
  after(() => {
    if (!chat.streaming) return;
    const patch = args[0] as Patch | undefined;
    if (!patch) return;
    for (const op of patch.ops) {
      if (op.op === "entity_upserted") overlay.noteEntityUpserted(op.entity);
    }
    startOverlayAnimation();
  });
});

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
  if (overlayRaf) cancelAnimationFrame(overlayRaf);
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
    <ChatPanel />
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
