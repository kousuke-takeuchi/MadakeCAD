<script setup lang="ts">
// 回路マクロのミニ描画 (挿入ダイアログのタイル/大プレビュー、保存ダイアログの確認欄)。
// 図面キャンバスと同じレンダラで描くので、図面記法を変えるとプレビューにも効く。
import { onMounted, ref, watch } from "vue";
import { fitPreview } from "../canvas/templatePreview";
import { macroEntities, macroSheet } from "../canvas/macroPreview";
import { renderSheet } from "../canvas/renderer";
import type { Macro } from "../ipc";
import { useDocumentStore } from "../stores/document";

const props = withDefaults(
  defineProps<{
    macro: Macro;
    /** 描くバリアント (未指定=既定の"A")。 */
    variant?: string | null;
    width?: number;
    height?: number;
    /** 参照記号・型番の注記を描くか (サムネイルでは潰れるので省く)。 */
    labels?: boolean;
  }>(),
  { variant: null, width: 96, height: 54, labels: true },
);
const canvasRef = ref<HTMLCanvasElement | null>(null);
const doc = useDocumentStore();

function draw() {
  const ctx = canvasRef.value?.getContext("2d");
  if (!ctx) return;
  ctx.clearRect(0, 0, props.width, props.height);
  const sheet = macroSheet(props.macro, props.variant);
  const vp = fitPreview(macroEntities(props.macro, props.variant), props.width, props.height);
  renderSheet(ctx, sheet, doc.symbols, vp, {
    selection: new Set(),
    cursor: null,
    // 図枠・グリッドはミニ描画では邪魔なので回路だけ描く
    hidden: new Set(props.labels ? ["grid", "frame"] : ["grid", "frame", "refs", "net_labels"]),
  });
}

onMounted(draw);
watch(
  () => [props.macro.id, props.variant, props.width, props.height, props.labels, doc.symbols.length],
  draw,
  { flush: "post" },
);
</script>

<template>
  <canvas ref="canvasRef" :width="width" :height="height" />
</template>
