<script setup lang="ts">
// 検証結果パネル (デザイン: MadakeCAD.pen「検証結果パネル」)。
// 作図領域下部にドッキングし、行クリックで該当エンティティを選択+ズームする。
import { X } from "lucide-vue-next";
import { inject } from "vue";
import type { Diagnostic } from "../ipc";
import type { EditorController } from "../tools/controller";
import { useDocumentStore } from "../stores/document";
import { useVerificationStore } from "../stores/verification";

const verification = useVerificationStore();
const store = useDocumentStore();
const controller = inject<EditorController>("controller")!;

const marks: Record<Diagnostic["severity"], string> = {
  error: "✗",
  warning: "⚠",
  info: "ℹ",
};

function pick(d: Diagnostic) {
  if (d.sheet_id !== store.activeSheetId) store.activeSheetId = d.sheet_id;
  controller.reveal(d.entity_ids);
}
</script>

<template>
  <div v-if="verification.panelOpen" class="panel">
    <div class="head">
      <span class="title">検証結果</span>
      <span class="badge error" v-if="verification.counts.error">✗ {{ verification.counts.error }}</span>
      <span class="badge warning" v-if="verification.counts.warning">⚠ {{ verification.counts.warning }}</span>
      <span class="badge info" v-if="verification.counts.info">ℹ {{ verification.counts.info }}</span>
      <span v-if="verification.diagnostics.length === 0" class="ok-note">問題は見つかりませんでした</span>
      <span class="spacer" />
      <button class="rerun" @click="verification.run(null)">再検証</button>
      <button class="close" @click="verification.close()"><X :size="12" /></button>
    </div>
    <div class="rows">
      <button v-for="(d, i) in verification.diagnostics" :key="i" class="row" @click="pick(d)">
        <span class="mark" :class="d.severity">{{ marks[d.severity] }}</span>
        <span class="code">{{ d.code }}</span>
        <span class="msg">{{ d.message }}</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.panel {
  flex: none;
  max-height: 180px;
  display: flex;
  flex-direction: column;
  background: var(--palette-bg);
  border-top: 1px solid var(--ribbon-line);
}
.head {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 10px;
  background: var(--palette-head);
}
.title {
  font-size: 11px;
  font-weight: 600;
  color: var(--ui-text);
}
.badge {
  font-size: 10px;
  font-weight: 600;
  border-radius: 999px;
  padding: 1px 8px;
}
.badge.error { color: var(--err-fg); background: var(--err-bg); }
.badge.warning { color: var(--warn-fg); background: var(--warn-bg); }
.badge.info { color: var(--info-fg); background: var(--info-bg); }
.ok-note { font-size: 10px; color: var(--ok-fg); }
.spacer { flex: 1; }
.rerun {
  border: none;
  background: transparent;
  color: var(--acad-blue);
  font-size: 10px;
  font-weight: 600;
  cursor: pointer;
}
.close {
  border: none;
  background: transparent;
  color: var(--ui-muted);
  display: flex;
  cursor: pointer;
}
.rows { overflow-y: auto; padding: 4px 0; }
.row {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  border: none;
  background: transparent;
  text-align: left;
  padding: 4px 10px;
  cursor: pointer;
}
.row:hover { background: var(--hover-bg); }
.mark { font-size: 11px; font-weight: 600; }
.mark.error { color: var(--err-fg); }
.mark.warning { color: var(--warn-fg); }
.mark.info { color: var(--info-fg); }
.code {
  font-family: var(--mono-font);
  font-size: 10px;
  color: var(--ui-muted);
  flex: none;
}
.msg { font-size: 11px; color: var(--ui-text); }
</style>
