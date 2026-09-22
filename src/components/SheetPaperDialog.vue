<script setup lang="ts">
// 用紙変更ダイアログ (G15)。用紙サイズ・向きを選んで set_sheet_paper コマンドを1回だけ
// 実行する(undo一発で戻る)。既存のエンティティ座標は動かない。新しい用紙の外に出た
// エンティティはERC警告 (erc.outside_paper) で報告されるだけで、動かしたり消したりしない。
// CAD調モーダル: palette-headヘッダ+グループ(WireNumberDialog.vue準拠)+フッタ。
// 文字列はすべてi18nカタログ経由 (docs/internal/specs/i18n.md)。
import { X } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import type { Orientation, PaperSize } from "../ipc";
import { useSheetPaperStore } from "../stores/sheetPaper";
import { useUiStore } from "../stores/ui";

const paper = useSheetPaperStore();
const ui = useUiStore();
const { t } = useI18n();

const sizes: PaperSize[] = ["A4", "A3", "A2", "A1", "A0"];
const orientations: { value: Orientation; labelKey: string }[] = [
  { value: "Landscape", labelKey: "sheetPaper.landscape" },
  { value: "Portrait", labelKey: "sheetPaper.portrait" },
];

function cancel() {
  if (paper.open) paper.cancel();
}

async function save() {
  const sheet = paper.sheetName;
  const dirty = paper.dirty;
  const sent = await paper.save();
  if (paper.error) {
    ui.log(paper.error);
    return;
  }
  ui.log(dirty && sent ? t("sheetPaper.savedLog", { sheet }) : t("sheetPaper.unchangedLog"));
}
</script>

<template>
  <div v-if="paper.open" class="overlay" @click.self="cancel">
    <div class="dialog">
      <div class="titlebar">
        <span class="title">{{ t("sheetPaper.title", { sheet: paper.sheetName }) }}</span>
        <button class="close" :title="t('sheetPaper.close')" @click="cancel"><X :size="14" /></button>
      </div>

      <div class="body">
        <p class="caption">{{ t("sheetPaper.note") }}</p>

        <div class="group">
          <span class="group-label">{{ t("sheetPaper.size") }}</span>
          <select v-model="paper.size" class="field">
            <option v-for="s in sizes" :key="s" :value="s">{{ s }}</option>
          </select>
        </div>
        <div class="group">
          <span class="group-label">{{ t("sheetPaper.orientation") }}</span>
          <label v-for="o in orientations" :key="o.value" class="radio">
            <input v-model="paper.orientation" type="radio" :value="o.value" />
            <span class="radio-label">{{ t(o.labelKey) }}</span>
          </label>
        </div>

        <p class="caption">{{ t("sheetPaper.hint") }}</p>
        <p v-if="paper.error" class="error">{{ paper.error }}</p>
      </div>

      <div class="footer">
        <button class="btn secondary" @click="cancel">{{ t("sheetPaper.cancel") }}</button>
        <button class="btn primary" :disabled="paper.saving" @click="save">
          {{ paper.saving ? t("sheetPaper.saving") : t("sheetPaper.save") }}
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
  width: min(360px, 94vw);
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
.radio {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--ui-text);
  cursor: pointer;
}
.radio input { accent-color: var(--acad-blue); margin: 0; }
.radio-label { line-height: 16px; }

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
