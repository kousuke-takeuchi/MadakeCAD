<script setup lang="ts">
// シンボル定義をミニキャンバスに描くプレビュー (部品挿入ダイアログ・パレット用)。
import { onMounted, ref } from "vue";
import { drawSymbol } from "../canvas/renderer";
import { Viewport } from "../canvas/viewport";
import type { SymbolDef } from "../ipc";

const props = withDefaults(
  defineProps<{ def: SymbolDef; width?: number; height?: number; color?: string }>(),
  { width: 64, height: 36, color: "#2b2f33" },
);
const canvasRef = ref<HTMLCanvasElement | null>(null);

onMounted(() => {
  const canvas = canvasRef.value;
  if (!canvas) return;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  // シンボルのバウンディングボックスを求めてフィット
  let minX = -8, maxX = 8, minY = -8, maxY = 8;
  const consider = (x: number, y: number) => {
    minX = Math.min(minX, x); maxX = Math.max(maxX, x);
    minY = Math.min(minY, y); maxY = Math.max(maxY, y);
  };
  for (const p of props.def.pins) consider(p.at.x, p.at.y);
  for (const prim of props.def.primitives) {
    if (prim.type === "line") prim.pts.forEach((p) => consider(p.x, p.y));
    else if (prim.type === "circle" || prim.type === "arc") {
      consider(prim.center.x - prim.r, prim.center.y - prim.r);
      consider(prim.center.x + prim.r, prim.center.y + prim.r);
    } else if (prim.type === "rect") {
      consider(prim.p1.x, prim.p1.y);
      consider(prim.p2.x, prim.p2.y);
    }
  }
  const vp = new Viewport();
  const w = maxX - minX, h = maxY - minY;
  vp.scale = Math.min((props.width - 8) / w, (props.height - 8) / h);
  vp.originX = props.width / 2 - ((minX + maxX) / 2) * vp.scale;
  vp.originY = props.height / 2 - ((minY + maxY) / 2) * vp.scale;
  drawSymbol(
    ctx,
    vp,
    { id: "p", symbol_id: props.def.id, at: { x: 0, y: 0 }, rotation: 0, mirror: false, reference: "", value: "", attrs: {} },
    props.def,
    false,
    props.color,
  );
});
</script>

<template>
  <canvas ref="canvasRef" :width="width" :height="height" />
</template>
