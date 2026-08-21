<script setup lang="ts">
// 改訂欄編集ダイアログ (Pencilデザイン「M2デザイン - 図面記法(規格準拠)」内の
// 「改訂欄編集ダイアログ」準拠)。CAD調モーダル: palette-headヘッダ+白テーブル+
// 「+ 改訂を追加」+フッタ(キャンセル/保存)。
// 表は最新の改訂が最上段 (図枠の改訂欄と同じ並び)。編集は下書きの上だけで行い、
// 保存で set_revisions コマンドを1回実行する (undo一発で戻る)。
// 文字列はすべてi18nカタログ経由 (docs/internal/specs/i18n.md)。
import { Plus, Trash2, X } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { useRevisionsStore } from "../stores/revisions";
import { useUiStore } from "../stores/ui";

const revisions = useRevisionsStore();
const ui = useUiStore();
const { t } = useI18n();

function cancel() {
  if (revisions.open) revisions.cancel();
}

function addRow() {
  revisions.addRow();
}

async function save() {
  const dirty = revisions.dirty;
  const count = await revisions.save();
  if (revisions.error) {
    ui.log(revisions.error);
    return;
  }
  if (!dirty || count === null) ui.log(t("revisions.unchangedLog"));
  else ui.log(t("revisions.savedLog", { count }));
}
</script>

<template>
  <div v-if="revisions.open" class="overlay" @click.self="cancel">
    <div class="dialog">
      <div class="titlebar">
        <span class="title">{{ t("revisions.title") }}</span>
        <button class="close" :title="t('revisions.close')" @click="cancel"><X :size="14" /></button>
      </div>

      <div class="body">
        <p class="caption">{{ t("revisions.note") }}</p>

        <div class="table">
          <div class="row head">
            <div class="cell mark">{{ t("revisions.col.mark") }}</div>
            <div class="cell date">{{ t("revisions.col.date") }}</div>
            <div class="cell desc">{{ t("revisions.col.description") }}</div>
            <div class="cell by">{{ t("revisions.col.by") }}</div>
            <div class="cell op" />
          </div>
          <div v-if="revisions.rows.length === 0" class="empty">{{ t("revisions.empty") }}</div>
          <div
            v-for="r in revisions.rows"
            :key="r.index"
            class="row"
            :class="{ selected: revisions.selected === r.index }"
            @click="revisions.select(r.index)"
          >
            <div class="cell mark">
              <input
                class="cell-input mono"
                :value="r.rev.mark"
                spellcheck="false"
                @input="revisions.updateRow(r.index, { mark: ($event.target as HTMLInputElement).value })"
              />
            </div>
            <div class="cell date">
              <input
                class="cell-input mono"
                :value="r.rev.date"
                placeholder="YYYY-MM-DD"
                spellcheck="false"
                @input="revisions.updateRow(r.index, { date: ($event.target as HTMLInputElement).value })"
              />
            </div>
            <div class="cell desc">
              <input
                class="cell-input"
                :value="r.rev.description"
                :placeholder="t('revisions.descriptionPlaceholder')"
                @input="revisions.updateRow(r.index, { description: ($event.target as HTMLInputElement).value })"
              />
            </div>
            <div class="cell by">
              <input
                class="cell-input"
                :value="r.rev.by"
                :placeholder="t('revisions.byPlaceholder')"
                @input="revisions.updateRow(r.index, { by: ($event.target as HTMLInputElement).value })"
              />
            </div>
            <div class="cell op">
              <button class="icon-btn" :title="t('revisions.deleteRow')" @click.stop="revisions.removeRow(r.index)">
                <Trash2 :size="12" />
              </button>
            </div>
          </div>
        </div>

        <button class="btn add" @click="addRow">
          <Plus :size="12" />
          {{ t("revisions.add") }}
        </button>

        <p class="caption">{{ t("revisions.hint") }}</p>
        <p v-if="revisions.error" class="error">{{ revisions.error }}</p>
      </div>

      <div class="footer">
        <button class="btn secondary" @click="cancel">{{ t("revisions.cancel") }}</button>
        <button class="btn primary" :disabled="revisions.saving" @click="save">
          {{ revisions.saving ? t("revisions.saving") : t("revisions.save") }}
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
  width: min(560px, 94vw);
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
  align-items: flex-start;
  gap: 12px;
  padding: 16px;
  overflow-y: auto;
}
.caption { margin: 0; font-size: 10px; line-height: 15px; color: var(--ui-muted); }

/* --- 白テーブル --- */
.table {
  width: 100%;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  overflow: hidden;
}
.row {
  display: flex;
  align-items: stretch;
  border-bottom: 1px solid var(--ribbon-line);
}
.row:last-child { border-bottom: none; }
.row.head { background: var(--input-bg); }
.row.head .cell {
  font-size: 10px;
  font-weight: 600;
  color: var(--ui-placeholder);
  padding: 6px 8px;
}
.row.selected { background: var(--sel-blue); }
.cell {
  display: flex;
  align-items: center;
  padding: 3px 8px;
  min-width: 0;
}
.cell.mark { width: 56px; flex: none; }
.cell.date { width: 100px; flex: none; }
.cell.desc { flex: 1; min-width: 0; }
.cell.by { width: 84px; flex: none; }
.cell.op { width: 32px; flex: none; justify-content: center; padding: 3px 4px; }
.cell-input {
  width: 100%;
  min-width: 0;
  border: 1px solid transparent;
  border-radius: 4px;
  background: transparent;
  padding: 3px 4px;
  font-size: 11px;
  color: var(--ui-text);
  outline: none;
}
.cell-input:hover { border-color: var(--ribbon-line); }
.cell-input:focus { border-color: var(--acad-blue); background: var(--card-bg); }
.cell-input::placeholder { color: var(--ui-placeholder); }
.mono { font-family: var(--mono-font); }
.empty {
  padding: 10px 8px;
  font-size: 11px;
  color: var(--ui-muted);
  border-bottom: 1px solid var(--ribbon-line);
}
.icon-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: transparent;
  color: var(--ui-muted);
  border-radius: 4px;
  padding: 3px;
  cursor: pointer;
}
.icon-btn:hover { background: var(--hover-bg); color: var(--err-fg); }

/* --- ボタン --- */
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
.btn:disabled { opacity: 0.5; cursor: default; }
.btn.add {
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  color: var(--acad-blue);
  padding: 5px 10px;
}
.btn.add:hover { background: var(--hover-bg); }
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
.error { margin: 0; font-size: 11px; color: var(--err-fg); }
</style>
