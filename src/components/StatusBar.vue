<script setup lang="ts">
// ステータスバー (Pencilデザイン準拠): 座標 + トグルアイコン群 + 尺度/モード/MCP。
import { Compass, Crosshair, Grid3x3, Magnet, MoveVertical, PenLine, Settings } from "lucide-vue-next";
import { inject } from "vue";
import { useI18n } from "vue-i18n";
import type { EditorController } from "../tools/controller";
import { useDocumentStore } from "../stores/document";
import { useUiStore } from "../stores/ui";

const store = useDocumentStore();
const ui = useUiStore();
const { t } = useI18n();
const controller = inject<EditorController>("controller")!;
</script>

<template>
  <div class="statusbar">
    <span class="coords">
      {{ controller.cursorWorld.x.toFixed(4) }}, {{ controller.cursorWorld.y.toFixed(4) }}, 0.0000
    </span>
    <span class="sep" />
    <button
      class="toggle"
      :class="{ on: ui.isClassVisible('grid') }"
      :title="t('statusBar.grid')"
      @click="ui.toggleViewClass('grid')"
    >
      <Grid3x3 :size="13" />
    </button>
    <button
      class="toggle"
      :class="{ on: controller.snapEnabled }"
      :title="t('statusBar.snap')"
      @click="controller.snapEnabled = !controller.snapEnabled"
    >
      <Magnet :size="13" />
    </button>
    <button
      class="toggle"
      :class="{ on: controller.orthoEnabled }"
      :title="t('statusBar.ortho')"
      @click="controller.orthoEnabled = !controller.orthoEnabled"
    >
      <MoveVertical :size="13" />
    </button>
    <button class="toggle" :title="t('statusBar.polar')" @click="ui.log(t('statusBar.polarLog'))">
      <Compass :size="13" />
    </button>
    <button class="toggle on" :title="t('statusBar.osnap')"><Crosshair :size="13" /></button>
    <button class="toggle" :title="t('statusBar.lineweight')" @click="ui.log(t('statusBar.lineweightLog'))">
      <PenLine :size="13" />
    </button>
    <span class="message">{{ ui.lastMessage }}</span>
    <span class="grow" />
    <span class="info">{{ store.activeSheet?.title_block.scale || "1:1" }} ▾</span>
    <span class="info">{{ t("statusBar.zoom", { percent: Math.round((controller.vp.scale / 4) * 100) }) }}</span>
    <span class="info">{{ t("statusBar.model") }}</span>
    <span class="info">{{ store.activeSheet?.name ?? "-" }}</span>
    <span class="mcp"><span class="dot" />MCP 9310</span>
    <Settings :size="12" class="gear" />
  </div>
</template>

<style scoped>
.statusbar {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 26px;
  padding: 0 10px;
  background: var(--status-bg);
  border-top: 1px solid var(--ribbon-line);
  font-size: 10px;
  color: var(--ui-text);
  user-select: none;
  flex: none;
}
.coords { font-family: "SF Mono", Menlo, monospace; min-width: 168px; }
.sep { width: 1px; height: 16px; background: var(--ribbon-line); }
.toggle {
  width: 26px;
  height: 20px;
  border: none;
  background: transparent;
  border-radius: 4px;
  color: var(--ui-muted);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}
.toggle.on {
  background: var(--sel-blue);
  color: var(--acad-blue);
}
.grow { flex: 1; }
.message {
  color: var(--ui-muted);
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  max-width: 380px;
  margin-left: 8px;
}
.info { color: var(--ui-muted); }
.mcp {
  display: flex;
  align-items: center;
  gap: 4px;
  font-family: "SF Mono", Menlo, monospace;
}
.dot { width: 6px; height: 6px; border-radius: 50%; background: var(--ok-fg); }
.gear { color: var(--ui-muted); }
</style>
