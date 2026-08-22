<script setup lang="ts">
// シンボル定義をミニキャンバスに描くプレビュー (部品挿入ダイアログ・パレット用)。
import { onMounted, ref, watch } from "vue";
import { drawSymbol, transformLocal } from "../canvas/renderer";
import { Viewport } from "../canvas/viewport";
import type { SymbolDef } from "../ipc";

const props = withDefaults(
  defineProps<{
    def: SymbolDef;
    width?: number;
    height?: number;
    color?: string;
    /** 回転バリアントのプレビュー (0/90/180/270)。既定は0度。 */
    rotation?: number;
  }>(),
  { width: 64, height: 36, color: "#2b2f33", rotation: 0 },
);
const canvasRef = ref<HTMLCanvasElement | null>(null);

/** 定義・回転が変わったら描き直す (バリアント帯は同じ定義を4つの角度で描く)。 */
function draw() {
  const canvas = canvasRef.value;
  if (!canvas) return;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.clearRect(0, 0, canvas.width, canvas.height);
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
  const inst = {
    id: "p",
    symbol_id: props.def.id,
    at: { x: 0, y: 0 },
    rotation: props.rotation,
    mirror: false,
    reference: "",
    value: "",
    attrs: {},
  };
  // 回転後の外形でフィットさせる (90/270度では縦横が入れ替わる)
  const corners = [
    { x: minX, y: minY },
    { x: maxX, y: minY },
    { x: maxX, y: maxY },
    { x: minX, y: maxY },
  ].map((p) => transformLocal(p, inst));
  const rMinX = Math.min(...corners.map((p) => p.x));
  const rMaxX = Math.max(...corners.map((p) => p.x));
  const rMinY = Math.min(...corners.map((p) => p.y));
  const rMaxY = Math.max(...corners.map((p) => p.y));
  const vp = new Viewport();
  const w = rMaxX - rMinX, h = rMaxY - rMinY;
  vp.scale = Math.min((props.width - 8) / w, (props.height - 8) / h);
  vp.originX = props.width / 2 - ((rMinX + rMaxX) / 2) * vp.scale;
  vp.originY = props.height / 2 - ((rMinY + rMaxY) / 2) * vp.scale;
  drawSymbol(ctx, vp, inst, props.def, false, props.color);
}

onMounted(draw);
watch(() => [props.def, props.rotation, props.color], draw, { deep: true });
</script>

<template>
  <canvas ref="canvasRef" :width="width" :height="height" />
</template>
