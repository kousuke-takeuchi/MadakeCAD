<script setup lang="ts">
import { inject, ref } from "vue";
import type { EditorController } from "../tools/controller";
import { useDocumentStore } from "../stores/document";

const store = useDocumentStore();
const controller = inject<EditorController>("controller")!;
const input = ref("");
const history = ref<string[]>(["MadakeCAD コマンドライン (L=配線 E=削除 U=元に戻す)"]);

function log(line: string) {
  history.value.push(line);
  if (history.value.length > 100) history.value.shift();
}

async function run() {
  const raw = input.value.trim();
  input.value = "";
  if (!raw) return;
  const [cmd] = raw.toUpperCase().split(/\s+/);
  log(`コマンド: ${raw}`);
  switch (cmd) {
    case "L":
    case "LINE":
    case "W":
    case "WIRE":
      controller.setTool("wire");
      log("配線の始点を指定 (ダブルクリックまたはEnterで確定)");
      break;
    case "E":
    case "ERASE":
      await controller.deleteSelection();
      log("削除しました");
      break;
    case "U":
    case "UNDO":
      await store.undo();
      controller.requestRedraw();
      log("元に戻しました");
      break;
    case "REDO":
      await store.redo();
      controller.requestRedraw();
      log("やり直しました");
      break;
    case "ESC":
    case "SELECT":
      controller.setTool("select");
      break;
    default:
      log(`不明なコマンド: ${cmd}`);
  }
}
</script>

<template>
  <div class="cmdline">
    <div class="history">
      <div v-for="(line, i) in history.slice(-2)" :key="i">{{ line }}</div>
    </div>
    <div class="input-row">
      <span class="prompt">&raquo;</span>
      <input
        v-model="input"
        placeholder="コマンドを入力"
        spellcheck="false"
        @keydown.enter="run"
        @keydown.esc="input = ''"
      />
    </div>
  </div>
</template>

<style scoped>
.cmdline {
  background: #f4f5f6;
  border-top: 1px solid var(--ribbon-line);
  padding: 3px 8px;
  font-family: "SF Mono", Menlo, monospace;
  user-select: none;
}
.history {
  font-size: 10px;
  color: var(--ui-muted);
  line-height: 1.5;
  min-height: 30px;
}
.input-row {
  display: flex;
  align-items: center;
  gap: 6px;
}
.prompt { color: var(--acad-blue); font-size: 12px; }
.input-row input {
  flex: 1;
  border: none;
  background: transparent;
  outline: none;
  font-family: inherit;
  font-size: 11px;
  color: var(--ui-text);
  padding: 2px 0;
}
</style>
