<script setup lang="ts">
// プロジェクト情報ダイアログ (G16)。プロジェクト名・番号と、現在のシートの表題欄
// (会社名・図番・表題)をまとめて編集する。保存で set_project_meta コマンドを1回だけ
// 実行する(触った欄だけを送る。会社名・図番・表題は全シートへ同じ値が書かれる)。
// CAD調モーダル: palette-headヘッダ+グループ(WireNumberDialog.vue準拠)+フッタ。
// 文字列はすべてi18nカタログ経由 (docs/internal/specs/i18n.md)。
import { X } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { useProjectInfoStore } from "../stores/projectInfo";
import { useUiStore } from "../stores/ui";

const info = useProjectInfoStore();
const ui = useUiStore();
const { t } = useI18n();

function cancel() {
  if (info.open) info.cancel();
}

async function save() {
  const dirty = info.dirty;
  const sent = await info.save();
  if (info.error) {
    ui.log(info.error);
    return;
  }
  ui.log(dirty && sent ? t("projectInfo.savedLog") : t("projectInfo.unchangedLog"));
}
</script>

<template>
  <div v-if="info.open" class="overlay" @click.self="cancel">
    <div class="dialog">
      <div class="titlebar">
        <span class="title">{{ t("projectInfo.title") }}</span>
        <button class="close" :title="t('projectInfo.close')" @click="cancel"><X :size="14" /></button>
      </div>

      <div class="body">
        <p class="caption">{{ t("projectInfo.note") }}</p>

        <div class="group">
          <span class="group-label">{{ t("projectInfo.name") }}</span>
          <input v-model="info.draft.name" class="field" spellcheck="false" />
        </div>
        <div class="group">
          <span class="group-label">{{ t("projectInfo.no") }}</span>
          <input v-model="info.draft.no" class="field mono" spellcheck="false" placeholder="MDK-2026-001" />
        </div>
        <div class="group">
          <span class="group-label">{{ t("projectInfo.company") }}</span>
          <input v-model="info.draft.company" class="field" spellcheck="false" />
        </div>
        <div class="group">
          <span class="group-label">{{ t("projectInfo.drawingNo") }}</span>
          <input v-model="info.draft.drawingNo" class="field mono" spellcheck="false" />
        </div>
        <div class="group">
          <span class="group-label">{{ t("projectInfo.titleBlockTitle") }}</span>
          <input v-model="info.draft.title" class="field" spellcheck="false" />
        </div>

        <p class="caption">{{ t("projectInfo.hint") }}</p>
        <p v-if="info.error" class="error">{{ info.error }}</p>
      </div>

      <div class="footer">
        <button class="btn secondary" @click="cancel">{{ t("projectInfo.cancel") }}</button>
        <button class="btn primary" :disabled="info.saving" @click="save">
          {{ info.saving ? t("projectInfo.saving") : t("projectInfo.save") }}
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
  width: min(420px, 94vw);
  max-height: 90vh;
  background: var(--ribbon-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 8px;
  box-shadow: var(--shadow-panel-lg);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* --- タイトルバー --- */
.titlebar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  background: var(--palette-head);
  flex: none;
}
.title { font-size: 12px; font-weight: 600; color: var(--ui-text); }
.close {
  border: none;
  background: transparent;
  color: var(--ui-muted);
  display: flex;
  cursor: pointer;
  padding: 0;
}

/* --- 本体 --- */
.body {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 12px;
  padding: 16px;
  overflow-y: auto;
}
.caption { margin: 0; font-size: 10px; line-height: 15px; color: var(--ui-muted); }

/* --- グループ --- */
.group {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
}
.group-label { font-size: 11px; font-weight: 600; color: var(--ui-muted); }
.field {
  width: 100%;
  background: var(--input-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  padding: 5px 8px;
  font-size: 12px;
  color: var(--ui-text);
  outline: none;
}
.field:focus { border-color: var(--acad-blue); }
.mono { font-family: var(--mono-font); }

/* --- ボタン --- */
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
.btn.secondary:hover:not(:disabled) { background: var(--hover-bg); }

/* --- フッタ --- */
.footer {
  flex: none;
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 16px;
  border-top: 1px solid var(--ribbon-line);
}
.error { margin: 0; font-size: 10px; color: var(--err-fg); }
</style>
