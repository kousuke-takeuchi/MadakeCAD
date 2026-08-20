<script setup lang="ts">
import { onBeforeUnmount, onMounted, provide, reactive } from "vue";
import { useFileActions } from "../composables/fileActions";
import { EditorController } from "../tools/controller";
import { useDocumentStore } from "../stores/document";
import { useUiStore } from "../stores/ui";
import CanvasView from "./CanvasView.vue";
import FileTabs from "./FileTabs.vue";
import ProjectPanel from "./ProjectPanel.vue";
import PropertiesPanel from "./PropertiesPanel.vue";
import RibbonBar from "./RibbonBar.vue";
import StatusBar from "./StatusBar.vue";
import SymbolPickerDialog from "./SymbolPickerDialog.vue";
import TitleBar from "./TitleBar.vue";

const store = useDocumentStore();
const ui = useUiStore();
const controller = reactive(new EditorController(store)) as EditorController;
provide("controller", controller);
const files = useFileActions();

function isEditableTarget(ev: KeyboardEvent) {
  const t = ev.target as HTMLElement | null;
  return !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable);
}

async function onKeyDown(ev: KeyboardEvent) {
  if (ev.key === "Escape" && ui.symbolPickerOpen) {
    ui.symbolPickerOpen = false;
    ev.preventDefault();
    return;
  }
  if (isEditableTarget(ev)) return;
  if (await controller.onKeyDown(ev)) ev.preventDefault();
}
function onKeyUp(ev: KeyboardEvent) {
  if (isEditableTarget(ev)) return;
  controller.onKeyUp(ev);
}

onMounted(() => {
  window.addEventListener("keydown", onKeyDown);
  window.addEventListener("keyup", onKeyUp);
});
onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKeyDown);
  window.removeEventListener("keyup", onKeyUp);
});
</script>

<template>
  <div class="editor">
    <TitleBar @open="files.openProject()" @save="files.saveProject()" />
    <RibbonBar />
    <FileTabs />
    <div class="main-row">
      <ProjectPanel />
      <CanvasView />
      <PropertiesPanel />
    </div>
    <StatusBar />
    <SymbolPickerDialog />
  </div>
</template>

<style scoped>
.editor {
  display: flex;
  flex-direction: column;
  height: 100vh;
  overflow: hidden;
}
.main-row {
  display: flex;
  flex: 1;
  min-height: 0;
}
</style>
