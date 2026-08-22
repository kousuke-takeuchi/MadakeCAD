<script setup lang="ts">
// 配置済みブロックのピン表を編集するダイアログ (プロパティの「ピン表を編集…」から開く)。
// デザイン: MadakeCAD.pen「フェーズ5 - ブロックのピン表を編集(プロパティから)」。
// 適用は update_entity コマンド1回 (⌘Zで戻せる)。
import { X } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { useBlockEditorStore } from "../stores/blockEditor";
import { useUiStore } from "../stores/ui";
import BlockPinsEditor from "./BlockPinsEditor.vue";

const editor = useBlockEditorStore();
const ui = useUiStore();
const { t } = useI18n();

async function apply() {
  const reference = editor.reference;
  const pins = editor.previewDef?.pins.length ?? 0;
  const dropped = editor.warnings.join(", ");
  if (!(await editor.apply())) {
    if (editor.error) ui.log(editor.error);
    return;
  }
  ui.log(t("blockEditor.updatedLog", { reference, pins }));
  if (dropped) ui.log(t("blockEditor.droppedLog", { pins: dropped }));
}
</script>

<template>
  <div v-if="editor.active && editor.mode === 'edit'" class="overlay" @click.self="editor.close()">
    <div class="dialog">
      <div class="titlebar">
        <span class="title">{{ t("blockEditor.title", { reference: editor.reference }) }}</span>
        <span class="spacer" />
        <button class="close" :title="t('blockEditor.close')" @click="editor.close()">
          <X :size="14" />
        </button>
      </div>

      <BlockPinsEditor />

      <div class="footer">
        <span class="hint">{{ t("blockEditor.applyHint") }}</span>
        <span class="spacer" />
        <button class="btn secondary" @click="editor.close()">{{ t("blockEditor.cancel") }}</button>
        <button class="btn primary" :disabled="!editor.canApply" @click="apply">
          {{ t("blockEditor.apply") }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  background: var(--scrim);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}
.dialog {
  width: min(760px, 96vw);
  max-height: 90vh;
  background: var(--ribbon-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 8px;
  box-shadow: var(--shadow-panel-lg);
  display: flex;
  flex-direction: column;
  overflow-y: auto;
}
.titlebar {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  background: var(--palette-head);
  flex: none;
}
.title { font-size: 12px; font-weight: 600; color: var(--ui-text); }
.spacer { flex: 1; }
.close {
  border: none;
  background: transparent;
  color: var(--ui-muted);
  display: flex;
  cursor: pointer;
  padding: 0;
}
.footer {
  flex: none;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px;
  border-top: 1px solid var(--ribbon-line);
}
.hint { font-size: 10px; color: var(--ui-muted); }
.btn {
  display: flex;
  align-items: center;
  justify-content: center;
  min-width: 72px;
  border-radius: 4px;
  padding: 5px 14px;
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
}
.btn:disabled { opacity: 0.5; cursor: default; }
.btn.primary { background: var(--acad-blue); border: 1px solid var(--acad-blue); color: var(--card-bg); }
.btn.secondary { background: var(--card-bg); border: 1px solid var(--ribbon-line); color: var(--ui-text); }
.btn.secondary:hover { background: var(--hover-bg); }
</style>
