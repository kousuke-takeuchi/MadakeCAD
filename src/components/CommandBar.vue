<script setup lang="ts">
// AutoCAD風コマンド入力バー (1行)。履歴表示は持たず、直近メッセージはステータスバーに出す。
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
  switch (cmd) {
    case "L":
    case "LINE":
    case "W":
    case "WIRE":
      controller.setTool("wire");
      ui.log("WIRE: 始点を指定 (ダブルクリックまたはEnterで確定)");
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
    <ChevronsRight :size="12" class="prompt" />
    <input
      v-model="input"
      placeholder="コマンドを入力 (L=配線 I=部品挿入 E=削除 U=元に戻す)"
      spellcheck="false"
      @keydown.enter="run"
      @keydown.esc="input = ''"
    />
  </div>
</template>

<style scoped>
.cmdline {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 30px;
  padding: 0 8px;
  background: #f4f5f6;
  border-top: 1px solid var(--ribbon-line);
  font-family: "SF Mono", Menlo, monospace;
  user-select: none;
  flex: none;
}
.prompt { color: var(--acad-blue); flex: none; }
input {
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
