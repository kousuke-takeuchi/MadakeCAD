<script setup lang="ts">
import { onBeforeUnmount, onMounted, provide, reactive } from "vue";
import { EditorController } from "../tools/controller";
import { useDocumentStore } from "../stores/document";
import CanvasView from "./CanvasView.vue";
import CommandBar from "./CommandBar.vue";
import ProjectPanel from "./ProjectPanel.vue";
import PropertiesPanel from "./PropertiesPanel.vue";
import RibbonBar from "./RibbonBar.vue";
import StatusBar from "./StatusBar.vue";

const store = useDocumentStore();
const controller = reactive(new EditorController(store)) as EditorController;
provide("controller", controller);

function isEditableTarget(ev: KeyboardEvent) {
  const t = ev.target as HTMLElement | null;
  return !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable);
}

async function onKeyDown(ev: KeyboardEvent) {
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
    <RibbonBar />
    <div class="main-row">
      <ProjectPanel />
      <CanvasView />
      <PropertiesPanel />
    </div>
    <CommandBar />
    <StatusBar />
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
