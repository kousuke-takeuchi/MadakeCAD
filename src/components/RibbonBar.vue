<script setup lang="ts">
import { inject, ref } from "vue";
import { open, save } from "@tauri-apps/plugin-dialog";
import { ipc } from "../ipc";
import type { EditorController } from "../tools/controller";
import { useDocumentStore } from "../stores/document";

const store = useDocumentStore();
const controller = inject<EditorController>("controller")!;
const activeTab = ref("回路図");
const tabs = ["ホーム", "プロジェクト", "回路図", "パネル", "レポート", "表示", "管理"];

async function exportSvg() {
  const sheet = store.activeSheet;
  if (!sheet) return;
  const path = await save({
    defaultPath: `${sheet.name}.svg`,
    filters: [{ name: "SVG", extensions: ["svg"] }],
  });
  if (path) await ipc.exportSvg(sheet.id, path);
}

async function exportBom() {
  const path = await save({
    defaultPath: "部品表.csv",
    filters: [{ name: "CSV", extensions: ["csv"] }],
  });
  if (path) await ipc.exportBom(path);
}

async function exportWireList() {
  const path = await save({
    defaultPath: "電線リスト.csv",
    filters: [{ name: "CSV", extensions: ["csv"] }],
  });
  if (path) await ipc.exportWireList(path);
}

async function openProject() {
  const path = await open({
    multiple: false,
    filters: [{ name: "MadakeCADプロジェクト", extensions: ["mdkproj"] }],
  });
  if (typeof path === "string") {
    const patch = await ipc.loadProject(path);
    store.applyPatch(patch);
    controller.requestRedraw();
  }
}

async function saveProject() {
  const path = await save({
    defaultPath: `${store.project?.name ?? "project"}.mdkproj`,
    filters: [{ name: "MadakeCADプロジェクト", extensions: ["mdkproj"] }],
  });
  if (path) await ipc.saveProject(path);
}

const groups = [
  {
    name: "ファイル",
    big: { label: "保存", icon: "💾", action: saveProject, toolId: "" },
    small: [
      { label: "開く", action: openProject },
    ],
  },
  {
    name: "配線",
    big: { label: "配線", icon: "〜", action: () => controller.setTool("wire"), toolId: "wire" },
    small: [
      { label: "選択", action: () => controller.setTool("select") },
      { label: "削除", action: () => controller.deleteSelection() },
    ],
  },
  {
    name: "編集",
    big: { label: "元に戻す", icon: "↩", action: () => store.undo().then(() => controller.requestRedraw()), toolId: "" },
    small: [
      { label: "やり直し", action: () => store.redo().then(() => controller.requestRedraw()) },
    ],
  },
  {
    name: "出力",
    big: { label: "SVG出力", icon: "▤", action: exportSvg, toolId: "" },
    small: [
      { label: "部品表", action: exportBom },
      { label: "電線リスト", action: exportWireList },
    ],
  },
];
</script>

<template>
  <div class="ribbon">
    <div class="ribbon-tabs">
      <button
        v-for="t in tabs"
        :key="t"
        class="ribbon-tab"
        :class="{ active: t === activeTab }"
        @click="activeTab = t"
      >
        {{ t }}
      </button>
    </div>
    <div class="ribbon-body">
      <template v-for="(g, i) in groups" :key="g.name">
        <div v-if="i > 0" class="ribbon-sep" />
        <div class="ribbon-group">
          <div class="ribbon-group-body">
            <button
              class="ribbon-big"
              :class="{ active: g.big.toolId && controller.tool === g.big.toolId }"
              @click="g.big.action()"
            >
              <span class="ribbon-big-icon">{{ g.big.icon }}</span>
              <span>{{ g.big.label }}</span>
            </button>
            <div class="ribbon-smalls">
              <button v-for="s in g.small" :key="s.label" class="ribbon-small" @click="s.action()">
                {{ s.label }}
              </button>
            </div>
          </div>
          <div class="ribbon-group-label">{{ g.name }}</div>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.ribbon {
  background: var(--ribbon-bg);
  border-bottom: 1px solid var(--ribbon-line);
  user-select: none;
}
.ribbon-tabs {
  display: flex;
  gap: 2px;
  padding: 2px 10px 0;
  background: var(--ribbon-strip);
}
.ribbon-tab {
  border: none;
  background: transparent;
  padding: 5px 13px;
  font-size: 12px;
  color: var(--ui-text);
  border-radius: 4px 4px 0 0;
  cursor: pointer;
}
.ribbon-tab.active {
  background: var(--ribbon-bg);
  color: var(--acad-blue);
  font-weight: 700;
}
.ribbon-body {
  display: flex;
  align-items: stretch;
  padding: 4px 6px 2px;
  min-height: 84px;
}
.ribbon-group {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 2px 6px 0;
}
.ribbon-group-body {
  display: flex;
  gap: 4px;
  align-items: flex-start;
  flex: 1;
}
.ribbon-group-label {
  font-size: 9px;
  color: var(--ribbon-label);
  padding: 1px 4px;
}
.ribbon-sep {
  width: 1px;
  background: var(--ribbon-line);
  margin: 4px 2px;
}
.ribbon-big {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  border: none;
  background: transparent;
  padding: 6px 8px;
  font-size: 10px;
  color: var(--ui-text);
  border-radius: 3px;
  cursor: pointer;
}
.ribbon-big:hover { background: var(--hover-bg); }
.ribbon-big.active { background: var(--sel-blue); }
.ribbon-big-icon { font-size: 22px; line-height: 1; color: var(--acad-blue); }
.ribbon-smalls {
  display: flex;
  flex-direction: column;
  gap: 1px;
  padding-top: 3px;
}
.ribbon-small {
  border: none;
  background: transparent;
  text-align: left;
  font-size: 10px;
  color: var(--ui-text);
  padding: 3px 6px;
  border-radius: 2px;
  cursor: pointer;
}
.ribbon-small:hover { background: var(--hover-bg); }
</style>
