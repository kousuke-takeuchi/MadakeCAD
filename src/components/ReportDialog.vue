<script setup lang="ts">
// 帳票生成ダイアログ (リボン「レポート」タブの各帳票ボタンの共通ダイアログ)。
// CAD調モーダル: palette-headヘッダ+グループボックス(帳票/対象/出力)+フッタ。
// 「対象」は端子台チャート・端子接続図だけ端子台1つに絞れる。出力先はOSの保存
// ダイアログで選ぶ (モーダルの上のモーダルは作らない)。
// 文字列はすべてi18nカタログ経由 (docs/internal/specs/i18n.md)。
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { X } from "lucide-vue-next";
import { filterFor, pickSave } from "../composables/fileActions";
import type { ReportKind } from "../ipc";
import { REPORT_KINDS, useReportDialogStore } from "../stores/reports";
import { useUiStore } from "../stores/ui";

const dialog = useReportDialogStore();
const ui = useUiStore();
const { t } = useI18n();

const kindLabel = computed(() => t(`reports.kind.${dialog.kind}`));

function cancel() {
  dialog.cancel();
}

async function browse() {
  const path = await pickSave(dialog.defaultFileName, filterFor(dialog.format));
  if (path) dialog.path = path;
}

async function run() {
  const request = dialog.request;
  const count = await dialog.run();
  if (dialog.error) {
    ui.log(dialog.error);
    return;
  }
  if (count === null || !request) return;
  const key = request.format === "csv" ? "reports.doneCsvLog" : "reports.donePdfLog";
  ui.log(t(key, { report: kindLabel.value, count, path: request.path }));
}
</script>

<template>
  <div v-if="dialog.open" class="overlay" @click.self="cancel">
    <div class="dialog">
      <div class="titlebar">
        <span class="title">{{ t("reports.title") }}</span>
        <button class="close" :title="t('reports.close')" @click="cancel"><X :size="14" /></button>
      </div>

      <div class="body">
        <p class="caption">{{ t("reports.note") }}</p>

        <div class="group">
          <div class="group-head"><span>{{ t("reports.groupReport") }}</span><i /></div>
          <div class="form-row">
            <label class="label">{{ t("reports.kindLabel") }}</label>
            <select
              class="input"
              :value="dialog.kind"
              @change="dialog.setKind(($event.target as HTMLSelectElement).value as ReportKind)"
            >
              <option v-for="k in REPORT_KINDS" :key="k" :value="k">{{ t(`reports.kind.${k}`) }}</option>
            </select>
          </div>
          <div class="form-row">
            <label class="label">{{ t("reports.targetLabel") }}</label>
            <select
              class="input"
              :disabled="dialog.targets.length < 2"
              :value="dialog.entityId ?? ''"
              @change="dialog.entityId = ($event.target as HTMLSelectElement).value || null"
            >
              <option
                v-for="target in dialog.targets"
                :key="target.id ?? 'project'"
                :value="target.id ?? ''"
              >
                {{ target.id
                  ? t("reports.targetBlock", { reference: target.reference, sheet: target.sheetName })
                  : t("reports.targetProject") }}
              </option>
            </select>
          </div>
          <p class="hint">{{ t("reports.targetHint") }}</p>
        </div>

        <div class="group">
          <div class="group-head"><span>{{ t("reports.groupOutput") }}</span><i /></div>
          <div class="form-row">
            <label class="label">{{ t("reports.formatLabel") }}</label>
            <div class="choices">
              <label v-for="f in dialog.formats" :key="f" class="choice">
                <input v-model="dialog.format" type="radio" :value="f" />
                {{ f === "csv" ? t("reports.formatCsv") : t("reports.formatPdf") }}
              </label>
            </div>
          </div>
          <p class="hint">
            {{ dialog.formats.length === 1 ? t("reports.diagramPdfOnly") : t("reports.formatHint") }}
          </p>
          <div class="form-row">
            <label class="label">{{ t("reports.pathLabel") }}</label>
            <input
              v-model="dialog.path"
              class="input mono"
              spellcheck="false"
              :placeholder="t('reports.pathPlaceholder')"
            />
            <button class="btn secondary small" @click="browse">{{ t("reports.browse") }}</button>
          </div>
        </div>

        <p v-if="dialog.error" class="error">{{ dialog.error }}</p>
      </div>

      <div class="footer">
        <button class="btn secondary" @click="cancel">{{ t("reports.cancel") }}</button>
        <button class="btn primary" :disabled="!dialog.request || dialog.running" @click="run">
          {{ dialog.running ? t("reports.running") : t("reports.run") }}
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
.input:disabled { opacity: 0.6; }
.mono { font-family: var(--mono-font); }
.choices { display: flex; gap: 14px; }
.choice { display: flex; align-items: center; gap: 5px; font-size: 11px; color: var(--ui-text); }
.choice input { accent-color: var(--acad-blue); }
.hint { margin: 0 0 0 138px; font-size: 10px; line-height: 15px; color: var(--ui-muted); }
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
