<script setup lang="ts">
// 端子台エディタ (Pencilデザイン「M4デザイン - 端子台チャート/帳票様式」の
// 「端子台エディタ」準拠)。CAD調モーダル(幅広): palette-headヘッダ+端子台切替
// ドロップダウン+ツールバー+白グリッド+フッタ(チェック結果+生成ボタン)。
//
// グリッドの中身は図面から導出した端子台チャートそのもので、ここで編集できるのは
// ジャンパだけ (update_entity コマンド1回。Cmd+Zで戻る)。
// v1で実装しない操作 (並べ替え・自動採番・多段端子・アクセサリ・内外スワップ・図面へ) は
// 無効化したプレースホルダとして並べ、Lv・型番・配置の列は出さない (フェーズ2)。
// 文字列はすべてi18nカタログ経由 (docs/internal/specs/i18n.md)。
import { computed, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowLeftRight, Layers, ListOrdered, Plus, Rows3, ShieldCheck, SquareStack, Unplug, X } from "lucide-vue-next";
import { filterFor, pickSave } from "../composables/fileActions";
import { ipc } from "../ipc";
import { useDocumentStore } from "../stores/document";
import { useTerminalsStore } from "../stores/terminals";
import { useUiStore } from "../stores/ui";

const terminals = useTerminalsStore();
const doc = useDocumentStore();
const ui = useUiStore();
const { t } = useI18n();

// 図面が変われば (undo/redo・AIチャット・CLIからの編集も含む) グリッドを取り直す
watch(
  () => doc.revision,
  () => void terminals.refresh(),
);

/** フェーズ2で実装する操作 (デザインどおり並べるが押せない)。 */
const phase2 = [
  { key: "sort", icon: Rows3 },
  { key: "autoNumber", icon: ListOrdered },
  { key: "spareTerminal", icon: SquareStack },
  { key: "multiLevel", icon: Layers },
  { key: "accessory", icon: Plus },
  { key: "swap", icon: ArrowLeftRight },
  { key: "goto", icon: Unplug },
] as const;

const title = computed(() =>
  terminals.reference
    ? t("terminals.titleWith", { reference: terminals.reference, poles: terminals.terminalCount })
    : t("terminals.title"),
);

/** チェック結果の表示文。 */
const checkText = computed(() => {
  if (terminals.diagnostics === null) return t("terminals.checkPending");
  if (terminals.checkOk) return t("terminals.checkOk");
  return t("terminals.checkCounts", terminals.checkCounts);
});

function close() {
  terminals.close();
}

async function addJumper() {
  const before = terminals.selectedTerminals.join(",");
  if (!(await terminals.addJumper())) {
    if (terminals.error) ui.log(terminals.error);
    return;
  }
  const jumper = before.split(",").join("-");
  ui.log(t("terminals.jumperAddedLog", { reference: terminals.reference, jumper }));
}

async function removeJumper() {
  const terminalList = terminals.selectedTerminals.join(", ");
  if (!(await terminals.removeJumper())) {
    if (terminals.error) ui.log(terminals.error);
    return;
  }
  ui.log(
    t("terminals.jumperRemovedLog", { reference: terminals.reference, terminals: terminalList }),
  );
}

async function runCheck() {
  await terminals.runCheck();
  if (terminals.error) {
    ui.log(terminals.error);
    return;
  }
  ui.log(t("terminals.checkLog", { reference: terminals.reference, result: checkText.value }));
}

/** 端子台チャートをCSVで書き出す (OSの保存ダイアログ経由)。 */
async function generateChart() {
  const id = terminals.entityId;
  if (!id) return;
  const path = await pickSave(`${terminals.reference}-terminal-chart.csv`, filterFor("csv"));
  if (!path) return;
  const count = await ipc.exportReport("terminal-chart", "csv", id, path);
  ui.log(t("terminals.chartLog", { reference: terminals.reference, count, path }));
}

/** 端子接続図を図枠付きのPDFで書き出す。 */
async function generateDiagram() {
  const id = terminals.entityId;
  if (!id) return;
  const path = await pickSave(`${terminals.reference}-terminal-diagram.pdf`, filterFor("pdf"));
  if (!path) return;
  const count = await ipc.exportReport("terminal-diagram", "pdf", id, path);
  ui.log(t("terminals.diagramLog", { reference: terminals.reference, count, path }));
}
</script>

<template>
  <div v-if="terminals.open" class="overlay" @click.self="close">
    <div class="dialog">
      <div class="titlebar">
        <span class="title">{{ title }}</span>
        <select
          v-if="terminals.blocks.length > 0"
          class="picker mono"
          :value="terminals.entityId ?? ''"
          @change="terminals.selectBlock(($event.target as HTMLSelectElement).value)"
        >
          <option v-for="b in terminals.blocks" :key="b.entity_id" :value="b.entity_id">
            {{ t("terminals.blockOption", { reference: b.reference, poles: b.terminal_count, sheet: b.sheet_name }) }}
          </option>
        </select>
        <span class="spacer" />
        <button class="close" :title="t('terminals.close')" @click="close"><X :size="14" /></button>
      </div>

      <div class="toolbar">
        <button class="tool" :disabled="!terminals.canAddJumper" @click="addJumper">
          {{ t("terminals.addJumper") }}
        </button>
        <button class="tool" :disabled="!terminals.canRemoveJumper" @click="removeJumper">
          {{ t("terminals.removeJumper") }}
        </button>
        <span class="tool-sep" />
        <button
          v-for="p in phase2"
          :key="p.key"
          class="tool"
          disabled
          :title="t('terminals.phase2Hint')"
        >
          <component :is="p.icon" :size="12" class="tool-icon" />
          {{ t(`terminals.phase2.${p.key}`) }}
        </button>
        <span class="tool-hint">{{ t("terminals.jumperHint") }}</span>
      </div>

      <div class="body">
        <div class="grid">
          <div class="row head">
            <div class="cell no">{{ t("terminals.col.no") }}</div>
            <div class="cell target">{{ t("terminals.col.external") }}</div>
            <div class="cell cable">{{ t("terminals.col.externalCable") }}</div>
            <div class="cell jumper">{{ t("terminals.col.jumper") }}</div>
            <div class="cell terminal">{{ t("terminals.col.terminal") }}</div>
            <div class="cell target">{{ t("terminals.col.internal") }}</div>
            <div class="cell wire-no">{{ t("terminals.col.wireNo") }}</div>
            <div class="cell wire">{{ t("terminals.col.wire") }}</div>
            <div class="cell status">{{ t("terminals.col.status") }}</div>
          </div>
          <div v-if="terminals.rows.length === 0" class="empty">{{ t("terminals.noBlocks") }}</div>
          <div
            v-for="r in terminals.rows"
            :key="r.terminal"
            class="row"
            :class="{ selected: r.selected }"
            @click="terminals.toggleRow(r.terminal)"
          >
            <div class="cell no mono">{{ r.no }}</div>
            <div class="cell target mono">{{ r.external }}</div>
            <div class="cell cable mono">{{ r.externalCable }}</div>
            <div class="cell jumper mono">{{ r.jumper }}</div>
            <div class="cell terminal mono strong">{{ r.terminal }}</div>
            <div class="cell target mono">{{ r.internal }}</div>
            <div class="cell wire-no mono">{{ r.wireNo }}</div>
            <div class="cell wire">{{ r.wire }}</div>
            <div class="cell status">
              <span v-if="r.spare" class="badge">{{ t("terminals.spare") }}</span>
            </div>
          </div>
        </div>
        <p class="caption">{{ t("terminals.hint") }}</p>
        <p v-if="terminals.error" class="error">{{ terminals.error }}</p>
      </div>

      <div class="footer">
        <button class="btn secondary" :disabled="!terminals.entityId" @click="runCheck">
          <ShieldCheck :size="13" />
          {{ t("terminals.check") }}
        </button>
        <span class="check-result" :class="{ ok: terminals.diagnostics !== null && terminals.checkOk }">
          {{ checkText }}
        </span>
        <span class="spacer" />
        <button class="btn secondary" :disabled="!terminals.entityId" @click="generateDiagram">
          {{ t("terminals.generateDiagram") }}
        </button>
        <button class="btn primary" :disabled="!terminals.entityId" @click="generateChart">
          {{ t("terminals.generateChart") }}
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
  width: min(1040px, 96vw);
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
.picker {
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  padding: 4px 8px;
  font-size: 11px;
  color: var(--ui-text);
}
.close {
  border: none;
  background: transparent;
  color: var(--ui-muted);
  display: flex;
  cursor: pointer;
  padding: 0;
}

/* --- ツールバー --- */
.toolbar {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
  padding: 8px 12px;
  border-bottom: 1px solid var(--ribbon-line);
  flex: none;
}
.tool {
  display: flex;
  align-items: center;
  gap: 5px;
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
.tool-sep { width: 1px; align-self: stretch; background: var(--ribbon-line); margin: 0 2px; }
.tool-hint { font-size: 10px; color: var(--ui-muted); }

/* --- 本体グリッド --- */
.body {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 12px;
  overflow-y: auto;
}
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
  cursor: default;
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
.cell.no { width: 40px; flex: none; }
.cell.target { width: 128px; flex: none; }
.cell.cable { width: 104px; flex: none; }
.cell.jumper { width: 68px; flex: none; }
.cell.terminal { width: 56px; flex: none; }
.cell.wire-no { width: 60px; flex: none; }
.cell.wire { flex: 1; min-width: 0; }
.cell.status { width: 64px; flex: none; }
.mono { font-family: var(--mono-font); }
.strong { font-weight: 600; }
.badge {
  font-size: 10px;
  color: var(--ui-muted);
  background: var(--hover-bg);
  border-radius: 999px;
  padding: 1px 7px;
}
.empty {
  padding: 12px 8px;
  font-size: 11px;
  color: var(--ui-muted);
}
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
.check-result { font-size: 10px; color: var(--ui-muted); }
.check-result.ok { color: var(--ok-fg); }
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
