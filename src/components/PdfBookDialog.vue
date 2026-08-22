<script setup lang="ts">
// PDF一括出力ダイアログ (リボン「レポート」タブ>出力グループの大ボタン)。
// CAD調モーダル: palette-headヘッダ+グループボックス(含める帳票のチェックボックス・
// 表紙・出力先)+フッタ。ダイアログ内のon/off設定はトグルでなくチェックボックス(CAD慣習)。
// 文字列はすべてi18nカタログ経由 (docs/internal/specs/i18n.md)。
import { useI18n } from "vue-i18n";
import { Check, X } from "lucide-vue-next";
import { filterFor, pickSave } from "../composables/fileActions";
import { REPORT_KINDS, usePdfBookStore } from "../stores/reports";
import { useDocumentStore } from "../stores/document";
import { useUiStore } from "../stores/ui";

const book = usePdfBookStore();
const doc = useDocumentStore();
const ui = useUiStore();
const { t } = useI18n();

function cancel() {
  book.cancel();
}

async function browse() {
  const path = await pickSave(`${doc.project?.name ?? "project"}.pdf`, filterFor("pdf"));
  if (path) book.path = path;
}

async function run() {
  const request = book.request;
  const pages = await book.run();
  if (book.error) {
    ui.log(book.error);
    return;
  }
  if (pages === null || !request) return;
  ui.log(t("pdfBook.doneLog", { count: pages, path: request.path }));
}
</script>

<template>
  <div v-if="book.open" class="overlay" @click.self="cancel">
    <div class="dialog">
      <div class="titlebar">
        <span class="title">{{ t("pdfBook.title") }}</span>
        <button class="close" :title="t('pdfBook.close')" @click="cancel"><X :size="14" /></button>
      </div>

      <div class="body">
        <p class="caption">{{ t("pdfBook.note") }}</p>

        <div class="group">
          <div class="group-head"><span>{{ t("pdfBook.reportsLabel") }}</span><i /></div>
          <label v-for="k in REPORT_KINDS" :key="k" class="check-row">
            <span class="box" :class="{ on: book.reports[k] }">
              <Check v-if="book.reports[k]" :size="10" />
            </span>
            <input v-model="book.reports[k]" type="checkbox" class="native" />
            {{ t(`reports.kind.${k}`) }}
          </label>
        </div>

        <div class="group">
          <div class="group-head"><span>{{ t("pdfBook.coverLabel") }}</span><i /></div>
          <label class="check-row">
            <span class="box" :class="{ on: book.cover }"><Check v-if="book.cover" :size="10" /></span>
            <input v-model="book.cover" type="checkbox" class="native" />
            {{ t("pdfBook.cover") }}
          </label>
          <div class="form-row">
            <label class="label">{{ t("pdfBook.pathLabel") }}</label>
            <input
              v-model="book.path"
              class="input mono"
              spellcheck="false"
              :placeholder="t('pdfBook.pathPlaceholder')"
            />
            <button class="btn secondary small" @click="browse">{{ t("pdfBook.browse") }}</button>
          </div>
        </div>

        <p v-if="book.error" class="error">{{ book.error }}</p>
      </div>

      <div class="footer">
        <button class="btn secondary" @click="cancel">{{ t("pdfBook.cancel") }}</button>
        <button class="btn primary" :disabled="!book.request || book.running" @click="run">
          {{ book.running ? t("pdfBook.running") : t("pdfBook.run") }}
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
  width: min(520px, 94vw);
  max-height: 90vh;
  background: var(--ribbon-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 8px;
  box-shadow: var(--shadow-panel-lg);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
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
.body {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 16px;
  overflow-y: auto;
}
.caption { margin: 0; font-size: 10px; line-height: 15px; color: var(--ui-muted); }
.group { display: flex; flex-direction: column; gap: 9px; }
.group-head { display: flex; align-items: center; gap: 8px; font-size: 11px; font-weight: 600; color: var(--ui-text); }
.group-head i { flex: 1; height: 1px; background: var(--ribbon-line); }
.check-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11px;
  color: var(--ui-text);
  cursor: pointer;
}
.native { position: absolute; opacity: 0; width: 0; height: 0; }
.box {
  width: 14px;
  height: 14px;
  flex: none;
  border-radius: 2px;
  border: 1px solid var(--ribbon-line);
  background: var(--card-bg);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--card-bg);
}
.box.on { background: var(--acad-blue); border-color: var(--acad-blue); }
.form-row { display: flex; align-items: center; gap: 8px; }
.label { width: 130px; flex: none; font-size: 11px; color: var(--ui-text); }
.input {
  flex: 1;
  min-width: 0;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  padding: 5px 8px;
  font-size: 11px;
  color: var(--ui-text);
  outline: none;
}
.input:focus { border-color: var(--acad-blue); }
.mono { font-family: var(--mono-font); }
.error { margin: 0; font-size: 11px; color: var(--err-fg); }
.footer {
  flex: none;
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 16px;
  border-top: 1px solid var(--ribbon-line);
}
.btn {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  min-width: 72px;
  border-radius: 4px;
  padding: 5px 14px;
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
}
.btn.small { min-width: 0; padding: 5px 10px; }
.btn:disabled { opacity: 0.5; cursor: default; }
.btn.primary { background: var(--acad-blue); border: 1px solid var(--acad-blue); color: var(--card-bg); }
.btn.secondary { background: var(--card-bg); border: 1px solid var(--ribbon-line); color: var(--ui-text); }
.btn.secondary:hover:not(:disabled) { background: var(--hover-bg); }
</style>
