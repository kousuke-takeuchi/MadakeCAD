<script setup lang="ts">
import { inject } from "vue";
import type { EditorController } from "../tools/controller";
import { useDocumentStore } from "../stores/document";

const store = useDocumentStore();
const controller = inject<EditorController>("controller")!;

function toggleSnap() {
  controller.snapEnabled = !controller.snapEnabled;
}
function toggleOrtho() {
  controller.orthoEnabled = !controller.orthoEnabled;
}
</script>

<template>
  <div class="statusbar">
    <span class="coords">
      {{ controller.cursorWorld.x.toFixed(1) }}, {{ controller.cursorWorld.y.toFixed(1) }}
    </span>
    <span class="sep" />
    <button class="toggle" :class="{ on: controller.snapEnabled }" title="グリッドスナップ" @click="toggleSnap">
      スナップ
    </button>
    <button class="toggle" :class="{ on: controller.orthoEnabled }" title="直交モード" @click="toggleOrtho">
      直交
    </button>
    <span class="grow" />
    <span class="info">ズーム {{ Math.round((controller.vp.scale / 4) * 100) }}%</span>
    <span class="info">シート: {{ store.activeSheet?.name ?? "-" }}</span>
    <span class="mcp"><span class="dot" />MCP 9310</span>
  </div>
</template>

<style scoped>
.statusbar {
  display: flex;
  align-items: center;
  gap: 10px;
  height: 26px;
  padding: 0 10px;
  background: var(--status-bg);
  border-top: 1px solid var(--ribbon-line);
  font-size: 10px;
  color: var(--ui-text);
  user-select: none;
}
.coords { font-family: "SF Mono", Menlo, monospace; min-width: 110px; }
.sep { width: 1px; height: 16px; background: var(--ribbon-line); }
.toggle {
  border: none;
  background: transparent;
  font-size: 10px;
  color: var(--ui-muted);
  padding: 2px 8px;
  border-radius: 2px;
  cursor: pointer;
}
.toggle.on {
  background: var(--sel-blue);
  color: var(--acad-blue);
}
.grow { flex: 1; }
.info { color: var(--ui-muted); }
.mcp { display: flex; align-items: center; gap: 4px; font-family: "SF Mono", Menlo, monospace; }
.dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #1f8a4c;
}
</style>
