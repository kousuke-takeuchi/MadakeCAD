<script setup lang="ts">
// 線番自動採番ダイアログ (Pencilデザイン「M2デザイン - 図面記法(規格準拠)」内の
// 「線番自動採番ダイアログ」準拠)。CAD調モーダル: palette-headヘッダ+グループ
// (採番方式/開始番号/対象/既存の線番)+フッタ(キャンセル/採番実行)。幅420。
// 実行で renumber_wires コマンドを1回だけ送る (undo一発で戻る)。
// 文字列はすべてi18nカタログ経由 (docs/internal/specs/i18n.md)。
import { X } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { useUiStore } from "../stores/ui";
import { useWireNumbersStore } from "../stores/wireNumbers";

const wireNumbers = useWireNumbersStore();
const ui = useUiStore();
const { t } = useI18n();

function cancel() {
  if (wireNumbers.open) wireNumbers.cancel();
}

/** ゾーン基準はM4予定。選べないことをログで知らせる。 */
function zoneUnavailable() {
  ui.log(t("wireNumbers.zoneDisabledLog"));
}

async function run() {
  if (!wireNumbers.startValid) {
    ui.log(t("wireNumbers.invalidLog"));
    return;
  }
  const scope = wireNumbers.scope;
  const sheet = wireNumbers.sheetName;
  const count = await wireNumbers.run();
  if (wireNumbers.error) {
    ui.log(wireNumbers.error);
    return;
  }
  if (count === null) return;
  ui.log(
    scope === "sheet"
      ? t("wireNumbers.doneSheetLog", { count, sheet })
      : t("wireNumbers.doneProjectLog", { count }),
  );
}
</script>

<template>
  <div v-if="wireNumbers.open" class="overlay" @click.self="cancel">
    <div class="dialog">
      <div class="titlebar">
        <span class="title">{{ t("wireNumbers.title") }}</span>
        <button class="close" :title="t('wireNumbers.close')" @click="cancel">
          <X :size="14" />
        </button>
      </div>

      <div class="body">
        <p class="caption">{{ t("wireNumbers.note") }}</p>

        <div class="group">
          <span class="group-label">{{ t("wireNumbers.method") }}</span>
          <label class="radio">
            <input v-model="wireNumbers.method" type="radio" value="sequential" />
            <span class="radio-label">{{ t("wireNumbers.sequential") }}</span>
            <span class="example mono">{{ t("wireNumbers.sequentialExample") }}</span>
          </label>
          <label class="radio disabled" :title="t('wireNumbers.zoneDisabled')" @click="zoneUnavailable">
            <input type="radio" value="zone" :checked="wireNumbers.method === 'zone'" disabled />
            <span class="radio-label">{{ t("wireNumbers.zone") }}</span>
            <span class="example mono">{{ t("wireNumbers.zoneExample") }}</span>
            <span class="example">{{ t("wireNumbers.zoneDisabled") }}</span>
          </label>
        </div>

        <div class="group">
          <span class="group-label">{{ t("wireNumbers.start") }}</span>
          <input
            v-model="wireNumbers.start"
            class="start mono"
            :class="{ invalid: !wireNumbers.startValid }"
            inputmode="numeric"
            spellcheck="false"
          />
          <p v-if="!wireNumbers.startValid" class="error">{{ t("wireNumbers.startInvalid") }}</p>
        </div>

        <div class="group">
          <span class="group-label">{{ t("wireNumbers.scope") }}</span>
          <label class="radio">
            <input v-model="wireNumbers.scope" type="radio" value="sheet" />
            <span class="radio-label">{{ t("wireNumbers.scopeSheet") }}</span>
            <span class="example">{{ wireNumbers.sheetName }}</span>
          </label>
          <label class="radio">
            <input v-model="wireNumbers.scope" type="radio" value="project" />
            <span class="radio-label">{{ t("wireNumbers.scopeProject") }}</span>
          </label>
        </div>

        <div class="group">
          <span class="group-label">{{ t("wireNumbers.existing") }}</span>
          <label class="radio">
            <input v-model="wireNumbers.existing" type="radio" value="keep" />
            <span class="radio-label">{{ t("wireNumbers.existingKeep") }}</span>
          </label>
          <label class="radio">
            <input v-model="wireNumbers.existing" type="radio" value="renumber" />
            <span class="radio-label">{{ t("wireNumbers.existingRenumber") }}</span>
          </label>
        </div>

        <p v-if="wireNumbers.error" class="error">{{ wireNumbers.error }}</p>
      </div>

      <div class="footer">
        <button class="btn secondary" @click="cancel">{{ t("wireNumbers.cancel") }}</button>
        <button
          class="btn primary"
          :disabled="wireNumbers.running || !wireNumbers.startValid"
          @click="run"
        >
          {{ wireNumbers.running ? t("wireNumbers.running") : t("wireNumbers.run") }}
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
  gap: 14px;
  padding: 16px;
  overflow-y: auto;
}
.caption { margin: 0; font-size: 10px; line-height: 15px; color: var(--ui-muted); }

/* --- グループ --- */
.group {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 8px;
}
.group-label { font-size: 11px; font-weight: 600; color: var(--ui-muted); }
.radio {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--ui-text);
  cursor: pointer;
}
.radio input { accent-color: var(--acad-blue); margin: 0; }
.radio.disabled { cursor: default; color: var(--ui-muted); }
.radio-label { line-height: 16px; }
.example { font-size: 10px; color: var(--ui-placeholder); }
.mono { font-family: var(--mono-font); }

/* --- 開始番号 --- */
.start {
  width: 120px;
  background: var(--input-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  padding: 5px 8px;
  font-size: 12px;
  color: var(--ui-text);
  outline: none;
}
.start:focus { border-color: var(--acad-blue); }
.start.invalid { border-color: var(--err-fg); }

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
