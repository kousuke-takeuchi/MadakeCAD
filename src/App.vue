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
