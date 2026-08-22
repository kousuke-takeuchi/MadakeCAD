<script setup lang="ts">
// PLC I/O割付表エディタ (Pencilデザイン「M4デザイン - PLC I/O」の「PLC割付表エディタ」準拠)。
// CAD調モーダル(幅640): palette-headヘッダ+モジュール行(ドロップダウン+開始アドレス)+
// 白グリッド+注記+フッタ(CSV読み込み…/保存/I/O図面ページを生成/閉じる)。
//
// 編集できるのは信号名・コメント・アドレスだけで、接続先・線番は図面から導出した
// 読み取り専用の値。保存は set_plc_assignments コマンド1回 (Cmd+Zで戻る)。
// 文字列はすべてi18nカタログ経由 (docs/internal/specs/i18n.md)。
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ListOrdered, Upload, X } from "lucide-vue-next";
import { useDocumentStore } from "../stores/document";
import { usePlcIoStore } from "../stores/plcIo";
import { useUiStore } from "../stores/ui";

const plc = usePlcIoStore();
const doc = useDocumentStore();
const ui = useUiStore();
const { t } = useI18n();

/** CSV読み込みのファイル選択 (Tauriのwebviewでもブラウザでも同じ経路)。 */
const fileInput = ref<HTMLInputElement | null>(null);
/** 選択中の行 (見た目だけ。行に対する操作は無い)。 */
const selected = ref<number | null>(null);

// 図面が変われば (undo/redo・AIチャット・CLIからの編集も含む) 表を追従させる
watch(
  () => doc.revision,
  () => void plc.refresh(),
);

const title = computed(() =>
  plc.moduleRef ? t("plcIo.titleWith", { reference: plc.moduleRef }) : t("plcIo.title"),
);

/** モジュールのドロップダウンの表示名 ("PLC1 — 型番 (DI 16点)")。 */
function moduleOption(reference: string, value: string, kind: string, points: number): string {
  return t("plcIo.moduleOption", { reference, value: value || "-", kind, points });
}

function close() {
  plc.close();
}

function fillAddresses() {
  plc.fillAddresses();
  ui.log(t("plcIo.autoNumberLog", { reference: plc.moduleRef ?? "", start: plc.rows[0]?.address ?? "" }));
}

async function save() {
  const count = await plc.save();
  if (plc.error) {
    ui.log(plc.error);
    return;
  }
  if (count === null) {
    ui.log(t("plcIo.noChangeLog"));
    return;
  }
  ui.log(t("plcIo.savedLog", { reference: plc.moduleRef ?? "", count }));
}

/** CSV読み込み: ファイルを選び、中身をCommand経由で取り込む。 */
async function onCsvPicked(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = "";
  if (!file) return;
  const csv = await file.text();
  if (!(await plc.importCsv(csv))) {
    ui.log(plc.error ?? t("plcIo.csvFailedLog", { file: file.name }));
    return;
  }
  // 取り込んだ行数 = 割付表に入った行 (グリッドの行数ではない)
  ui.log(t("plcIo.csvLog", { reference: plc.moduleRef ?? "", file: file.name, count: plc.points.length }));
}

function openGenerate() {
  plc.openGenerate();
  ui.log(t("plcIo.generateOpenLog", { reference: plc.moduleRef ?? "" }));
}
</script>

<template>
  <div v-if="plc.open" class="overlay" @click.self="close">
    <div class="dialog">
      <div class="titlebar">
        <span class="title">{{ title }}</span>
        <span class="spacer" />
        <button class="close" :title="t('plcIo.close')" @click="close"><X :size="14" /></button>
      </div>

      <div class="body">
        <div class="module-row">
          <label class="label">{{ t("plcIo.moduleLabel") }}</label>
          <select
            class="input"
            :disabled="plc.modules.length === 0"
            :value="plc.moduleRef ?? ''"
            @change="plc.selectModule(($event.target as HTMLSelectElement).value)"
          >
            <option v-for="m in plc.modules" :key="m.entity_id" :value="m.reference">
              {{ moduleOption(m.reference, m.value, m.kind, m.points) }}
            </option>
          </select>
          <label class="label start">{{ t("plcIo.startAddress") }}</label>
          <input
            v-model="plc.startAddress"
            class="input address mono"
            spellcheck="false"
            :disabled="!plc.moduleRef"
            :placeholder="plc.rows[0]?.autoAddress ?? ''"
          />
          <button class="tool" :disabled="!plc.moduleRef" @click="fillAddresses">
            <ListOrdered :size="12" class="tool-icon" />
            {{ t("plcIo.autoNumber") }}
          </button>
        </div>

        <div class="grid">
          <div class="row head">
            <div class="cell address">{{ t("plcIo.col.address") }}</div>
            <div class="cell signal">{{ t("plcIo.col.signal") }}</div>
            <div class="cell target">{{ t("plcIo.col.target") }}</div>
            <div class="cell wire-no">{{ t("plcIo.col.wireNo") }}</div>
            <div class="cell comment">{{ t("plcIo.col.comment") }}</div>
          </div>
          <div v-if="plc.rows.length === 0" class="empty">{{ t("plcIo.noModules") }}</div>
          <div
            v-for="r in plc.rows"
            :key="r.point"
            class="row"
            :class="{ selected: selected === r.point }"
            @click="selected = r.point"
          >
            <input
              class="cell address mono edit"
              :value="r.address"
              :placeholder="r.autoAddress"
              spellcheck="false"
              @input="plc.updateRow(r.point - 1, { address: ($event.target as HTMLInputElement).value })"
            />
            <input
              class="cell signal edit"
              :value="r.signalName"
              :placeholder="t('plcIo.unassigned')"
              @input="plc.updateRow(r.point - 1, { signalName: ($event.target as HTMLInputElement).value })"
            />
            <div class="cell target mono readonly">{{ r.target }}</div>
            <div class="cell wire-no mono readonly">{{ r.wireNo }}</div>
            <input
              class="cell comment edit"
              :value="r.comment"
              @input="plc.updateRow(r.point - 1, { comment: ($event.target as HTMLInputElement).value })"
            />
          </div>
        </div>

        <p class="caption">{{ t("plcIo.note") }}</p>
        <p v-if="plc.error" class="error">{{ plc.error }}</p>
      </div>

      <div class="footer">
        <button class="btn secondary" :disabled="!plc.moduleRef" @click="fileInput?.click()">
          <Upload :size="13" />
          {{ t("plcIo.importCsv") }}
        </button>
        <input
          ref="fileInput"
          type="file"
          accept=".csv,text/csv"
          class="file-input"
          @change="onCsvPicked"
        />
        <span class="spacer" />
        <button class="btn secondary" :disabled="!plc.dirty || plc.saving" @click="save">
          {{ t("plcIo.save") }}
        </button>
        <button class="btn primary" :disabled="!plc.moduleRef" @click="openGenerate">
          {{ t("plcIo.generateSheet") }}
        </button>
        <button class="btn secondary" @click="close">{{ t("plcIo.close") }}</button>
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
  width: min(640px, 94vw);
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

/* --- 本体 --- */
.body {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px;
  overflow-y: auto;
}
.module-row { display: flex; align-items: center; gap: 8px; }
.label { font-size: 11px; color: var(--ui-text); flex: none; }
.label.start { margin-left: 4px; }
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
.input.address { flex: none; width: 76px; }
.tool {
  display: flex;
  align-items: center;
  gap: 5px;
  flex: none;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  padding: 4px 10px;
  font-size: 11px;
  color: var(--ui-text);
  cursor: pointer;
}
.tool:hover:not(:disabled) { background: var(--hover-bg); }
.tool:disabled { opacity: 0.45; cursor: default; }
.tool-icon { color: var(--ribbon-icon); }

/* --- グリッド --- */
.grid {
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
.row:not(.head):hover { background: var(--hover-bg); }
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
  padding: 5px 8px;
  font-size: 11px;
  color: var(--ui-text);
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
.cell.address { width: 84px; flex: none; }
.cell.signal { flex: 1; min-width: 0; }
.cell.target { width: 96px; flex: none; }
.cell.wire-no { width: 56px; flex: none; }
.cell.comment { width: 130px; flex: none; }
.cell.edit {
  background: transparent;
  border: none;
  outline: none;
  border-radius: 4px;
}
.cell.edit:focus { background: var(--card-bg); box-shadow: inset 0 0 0 1px var(--acad-blue); }
.cell.edit::placeholder { color: var(--ui-placeholder); }
.cell.readonly { color: var(--ui-muted); }
.mono { font-family: var(--mono-font); }
.empty { padding: 12px 8px; font-size: 11px; color: var(--ui-muted); }
.caption { margin: 0; font-size: 10px; line-height: 15px; color: var(--ui-muted); }
.error { margin: 0; font-size: 11px; color: var(--err-fg); }

/* --- フッタ --- */
.footer {
  flex: none;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px;
  border-top: 1px solid var(--ribbon-line);
}
.file-input { display: none; }
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
.btn.primary { background: var(--acad-blue); border: 1px solid var(--acad-blue); color: var(--card-bg); }
.btn.secondary { background: var(--card-bg); border: 1px solid var(--ribbon-line); color: var(--ui-text); }
.btn.secondary:hover:not(:disabled) { background: var(--hover-bg); }
</style>
