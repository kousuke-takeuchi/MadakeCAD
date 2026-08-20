<script setup lang="ts">
import { ChevronsRight } from "lucide-vue-next";
import { inject, ref } from "vue";
import type { EditorController } from "../tools/controller";
import { useDocumentStore } from "../stores/document";
import { useUiStore } from "../stores/ui";

const store = useDocumentStore();
const ui = useUiStore();
const controller = inject<EditorController>("controller")!;
const input = ref("");

async function run() {
  const raw = input.value.trim();
  input.value = "";
  if (!raw) return;
  const [cmd] = raw.toUpperCase().split(/\s+/);
  ui.log(`コマンド: ${raw}`);
  switch (cmd) {
    case "L":
    case "LINE":
    case "W":
    case "WIRE":
      controller.setTool("wire");
      ui.log("配線の始点を指定 (ダブルクリックまたはEnterで確定)");
      break;
    case "I":
    case "INSERT":
      ui.symbolPickerOpen = true;
      break;
    case "E":
    case "ERASE":
      await controller.deleteSelection();
      ui.log("削除しました");
      break;
    case "U":
    case "UNDO":
      await store.undo();
      controller.requestRedraw();
      ui.log("元に戻しました");
      break;
    case "REDO":
      await store.redo();
      controller.requestRedraw();
      ui.log("やり直しました");
      break;
    case "ESC":
    case "SELECT":
      controller.setTool("select");
      break;
    default:
      ui.log(`不明なコマンド: ${cmd}`);
  }
}
</script>

<template>
  <div class="cmdline">
    <div class="history">
      <div v-for="(line, i) in ui.commandHistory.slice(-2)" :key="i">{{ line }}</div>
    </div>
    <div class="input-row">
      <ChevronsRight :size="12" class="prompt" />
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
  flex: none;
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
.prompt { color: var(--acad-blue); flex: none; }
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
