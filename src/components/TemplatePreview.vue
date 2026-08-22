<script setup lang="ts">
// テンプレートのミニ描画 (テンプレート選択ダイアログのサムネイル・大プレビュー)。
// 図面キャンバスと同じレンダラで描くので、図面記法を変えるとプレビューにも効く。
import { onMounted, ref, watch } from "vue";
import { renderSheet } from "../canvas/renderer";
import { fitPreview, templateEntities, templateSheet } from "../canvas/templatePreview";
import type { Template } from "../ipc";
import { useDocumentStore } from "../stores/document";

const props = withDefaults(defineProps<{ template: Template; width?: number; height?: number }>(), {
  width: 96,
  height: 54,
});
const canvasRef = ref<HTMLCanvasElement | null>(null);
const doc = useDocumentStore();

function draw() {
  const ctx = canvasRef.value?.getContext("2d");
  if (!ctx) return;
  const sheet = templateSheet(props.template);
  const vp = fitPreview(templateEntities(props.template.commands), props.width, props.height);
  renderSheet(ctx, sheet, doc.symbols, vp, {
    selection: new Set(),
    cursor: null,
    // 図枠・グリッドはミニ描画では邪魔なので回路だけ描く
    hidden: new Set(["grid", "frame"]),
  });
}

onMounted(draw);
watch(
  () => [props.template.id, props.width, props.height, doc.symbols.length],
  draw,
  { flush: "post" },
);
</script>

<template>
  <canvas ref="canvasRef" :width="width" :height="height" />
</template>
