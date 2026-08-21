<script setup lang="ts">
import { FilePlus, FolderOpen, Save, Printer, Settings, Undo2, Redo2 } from "lucide-vue-next";
import { inject } from "vue";
import { inTauri } from "../ipc";
import type { EditorController } from "../tools/controller";
import { useDocumentStore } from "../stores/document";
import { useUiStore } from "../stores/ui";

const store = useDocumentStore();
const ui = useUiStore();
const controller = inject<EditorController>("controller")!;

const emit = defineEmits<{ (e: "open"): void; (e: "save"): void }>();

async function undo() {
  await store.undo();
  controller.requestRedraw();
}
async function redo() {
  await store.redo();
  controller.requestRedraw();
}
</script>

<template>
  <div class="titlebar" data-tauri-drag-region>
    <!-- macOSのトラフィックライト分の余白 (Tauri実行時のみ) -->
    <div v-if="inTauri" class="traffic-space" />
    <div class="logo">M</div>
    <div class="qat">
      <button title="新規"><FilePlus :size="14" /></button>
      <button title="開く" @click="emit('open')"><FolderOpen :size="14" /></button>
      <button title="保存" @click="emit('save')"><Save :size="14" /></button>
      <button title="印刷"><Printer :size="14" /></button>
      <span class="qat-sep" />
      <button title="元に戻す (Cmd+Z)" :disabled="!store.canUndo" @click="undo"><Undo2 :size="14" /></button>
      <button title="やり直し (Cmd+Shift+Z)" :disabled="!store.canRedo" @click="redo"><Redo2 :size="14" /></button>
    </div>
    <div class="doc-name" data-tauri-drag-region>
      MadakeCAD - [{{ store.activeSheet?.name ?? "無題" }}.mdkproj]
    </div>
    <div class="right-space">
      <button class="gear" title="設定" @click="ui.settingsOpen = true"><Settings :size="14" /></button>
    </div>
  </div>
</template>

<style scoped>
.titlebar {
  display: flex;
  align-items: center;
  gap: 2px;
  height: 34px;
  padding: 0 8px;
  background: var(--tb-bg);
  user-select: none;
  flex: none;
}
.traffic-space { width: 70px; flex: none; }
.logo {
  width: 26px;
  height: 22px;
  border-radius: 3px;
  background: #c0392b;
  color: #fff;
  font-weight: 800;
  font-size: 13px;
  display: flex;
  align-items: center;
  justify-content: center;
  margin-right: 8px;
}
.qat { display: flex; align-items: center; gap: 2px; }
.qat button {
  width: 26px;
  height: 24px;
  border: none;
  border-radius: 3px;
  background: transparent;
  color: var(--tb-text);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}
.qat button:hover { background: rgba(255, 255, 255, 0.1); }
.qat button:disabled { opacity: 0.35; cursor: default; }
.qat-sep { width: 1px; height: 16px; background: rgba(255, 255, 255, 0.15); margin: 0 4px; }
.doc-name {
  flex: 1;
  text-align: center;
  font-size: 12px;
  color: var(--tb-text);
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
.right-space {
  width: 120px;
  flex: none;
  display: flex;
  justify-content: flex-end;
  align-items: center;
}
.gear {
  width: 26px;
  height: 24px;
  border: none;
  border-radius: 3px;
  background: transparent;
  color: var(--tb-text);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}
.gear:hover { background: rgba(255, 255, 255, 0.1); }
</style>
