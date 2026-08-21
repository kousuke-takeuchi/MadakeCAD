<script setup lang="ts">
import { onMounted, ref } from "vue";
import EditorLayout from "./components/EditorLayout.vue";
import { useDocumentStore } from "./stores/document";

const store = useDocumentStore();
const ready = ref(false);
const error = ref<string | null>(null);

onMounted(async () => {
  try {
    await store.bootstrap();
    ready.value = true;
  } catch (e) {
    error.value = String(e);
  }
});
</script>

<template>
  <EditorLayout v-if="ready" />
  <div v-else class="boot">
    <p v-if="error" class="boot-error">起動エラー: {{ error }}</p>
    <p v-else>読み込み中...</p>
  </div>
</template>

<style>
/* AutoCAD Electrical風ライトテーマのUIトークン (Pencilデザイン準拠) */
:root {
  --tb-bg: #2a2e34;
  --tb-text: #d8dbde;
  --model-bg: #212830;
  --ribbon-strip: #dfe3e7;
  --ribbon-bg: #f2f4f5;
  --ribbon-line: #c6cbd1;
  --ribbon-label: #5a6068;
  --ui-text: #2b2f33;
  --ui-muted: #6b7178;
  --palette-bg: #f7f8f9;
  --palette-head: #d9dde1;
  --sel-blue: #cce4f7;
  --acad-blue: #1f6fbf;
  --status-bg: #d8dce0;
  --hover-bg: #e4e8ec;
  --card-bg: #ffffff;
  --input-bg: #fafafa;
  --ui-placeholder: #8a9099;
  --ok-fg: #1f8a4c;
  --ok-bg: #f0fdf4;
  --warn-fg: #d97706;
  --warn-bg: #fff7ed;
  --err-fg: #dc2626;
  --err-bg: #fef2f2;
  --off-fg: #9ca3af;
  --off-bg: #f3f4f6;
  --mono-font: "JetBrains Mono", "SF Mono", Menlo, monospace;
  --shadow-popup: 0 6px 18px #00000028;
  --shadow-panel: 0 6px 22px #00000030;
  --shadow-panel-lg: 0 8px 28px #00000038;
  --scrim: #00000059;
}
html, body, #app {
  margin: 0;
  padding: 0;
  height: 100%;
  font-family: "Hiragino Kaku Gothic ProN", "Noto Sans JP", sans-serif;
  background: var(--ribbon-bg);
  color: var(--ui-text);
}
* { box-sizing: border-box; }
</style>

<style scoped>
.boot {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100vh;
  font-size: 13px;
  color: var(--ui-muted);
}
.boot-error { color: #c0392b; }
</style>
