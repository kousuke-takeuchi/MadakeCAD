<script setup lang="ts">
// 図枠テンプレートのライブプレビュー (図枠選択ダイアログの中央)。
// 図面キャンバスと同じレンダラ (renderSheet) で、いま開いているシートの用紙へ
// 選んだ図枠を当てて描く。図枠そのものが主題なので回路・グリッドは隠す。
// 値が空になる表題欄の欄には、そこへ何が入るかを`{バインド}`の青字で重ねる。
import { onMounted, ref, watch } from "vue";
import { renderSheet } from "../canvas/renderer";
import { bindOverlays, paperSizeMm, type BindContext, type FrameTemplate } from "../canvas/frameTemplate";
import { theme } from "../canvas/theme";
import { VIEW_CLASSES, type ViewClass } from "../canvas/viewClasses";
import { Viewport } from "../canvas/viewport";
import type { Sheet } from "../ipc";

const props = withDefaults(
  defineProps<{
    template: FrameTemplate;
    /** どのシートへ当てるか (用紙サイズ・向き・表題欄・改訂欄の実物)。 */
    sheet: Sheet;
    /** 表題欄バインドの文脈 (プロジェクト名・シート番号)。 */
    bind: BindContext;
    width?: number;
    height?: number;
  }>(),
  { width: 560, height: 340 },
);

const canvasRef = ref<HTMLCanvasElement | null>(null);

/** 図枠以外の表示クラス (プレビューでは隠す)。 */
const HIDDEN: ReadonlySet<ViewClass> = new Set(
  VIEW_CLASSES.map((v) => v.id).filter((id) => id !== "frame"),
);

function draw() {
  const ctx = canvasRef.value?.getContext("2d");
  if (!ctx) return;
  const { w, h } = paperSizeMm(props.sheet);
  const vp = new Viewport();
  vp.fit(w, h, props.width, props.height, 14);
  renderSheet(ctx, props.sheet, [], vp, {
    selection: new Set(),
    cursor: null,
    hidden: HIDDEN,
    frame: props.template,
    bind: props.bind,
  });

  // 空欄に入るバインドを青字で重ねる (値が入っている欄には重ねない)
  const mono =
    getComputedStyle(document.documentElement).getPropertyValue("--mono-font").trim() || "monospace";
  ctx.fillStyle = theme.bindPlaceholder;
  for (const o of bindOverlays(props.sheet, props.template, props.bind)) {
    const p = vp.toScreen({ x: o.x, y: o.y });
    ctx.textAlign = o.align === "middle" ? "center" : o.align === "end" ? "right" : "left";
    ctx.font = `${Math.max(8, o.size * vp.scale)}px ${mono}`;
    ctx.fillText(o.text, p.x, p.y);
  }
  ctx.textAlign = "left";
}

onMounted(draw);
watch(
  () => [props.template, props.sheet, props.bind, props.width, props.height],
  draw,
  { flush: "post", deep: true },
);
</script>

<template>
  <canvas ref="canvasRef" :width="width" :height="height" />
</template>
