<script setup lang="ts">
// 端子台エディタ (Pencilデザイン「M4デザイン - 端子台チャート/帳票様式」の
// 「端子台エディタ」準拠)。CAD調モーダル(幅広): palette-headヘッダ+端子台切替
// ドロップダウン+ツールバー+白グリッド+フッタ(チェック結果+生成ボタン)。
//
// グリッドの中身は図面から導出した端子台チャートそのもので、ここで編集できるのは
// ジャンパ・並べ替え順 (row_order)・端子番号 (terminal_numbers) の3つ (どれも
// update_entity/RenumberTerminals/SetTerminalNumbersコマンド1回。Cmd+Zで戻る)。
// 並べ替え・Lv列 (多段端子台だけ)・図面へジャンプ・端子番号の自動採番はここで実装済み。
// v1で実装しない操作 (多段端子の生成・アクセサリ・内外スワップ) は
// 無効化したプレースホルダとして並べ、型番・配置の列は出さない (フェーズ2)。
// 文字列はすべてi18nカタログ経由 (docs/internal/specs/i18n.md)。
import { computed, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowLeftRight, Layers, ListOrdered, Minus, Plus, ShieldCheck, SquareStack, TriangleAlert, Unplug, X } from "lucide-vue-next";
import { clampLevels } from "./symbolPickerDynamic";
import { filterFor, pickSave } from "../composables/fileActions";
import { useReveal } from "../composables/reveal";
import { ipc } from "../ipc";
import { useDocumentStore } from "../stores/document";
import { ROW_ORDERS, rowOrderOf, TERMINAL_LEVEL_MAX, useTerminalsStore } from "../stores/terminals";
import { useUiStore } from "../stores/ui";

const terminals = useTerminalsStore();
const doc = useDocumentStore();
const ui = useUiStore();
const reveal = useReveal();
const { t } = useI18n();

// 図面が変われば (undo/redo・AIチャット・CLIからの編集も含む) グリッドを取り直す
watch(
  () => doc.revision,
  () => void terminals.refresh(),
);

/** フェーズ2で実装する操作 (デザインどおり並べるが押せない。採番・多段は実装済みなので除く)。 */
const phase2 = [
  { key: "spareTerminal", icon: SquareStack },
  { key: "accessory", icon: Plus },
  { key: "swap", icon: ArrowLeftRight },
] as const;

// --- 採番ポップオーバー (デザイン「採番 (既定)」/「採番 (エラー)」準拠) ---
// 開始番号・増分・接頭辞・対象・プレビュー (対応表+ジャンパの追従)・指摘行・適用/キャンセル。
// 適用できない案は**ポップオーバーの中で**理由を出し、適用ボタンを無効にする
// (Rust側のエラー文をログへ流すのではなく、打っている場所で断る)。
const showRenumber = ref(false);
const renumberStart = ref(1);
const renumberStep = ref(1);
const renumberPrefix = ref("");
/**
 * 採番の対象。「選択行」はコマンドに行単位のスコープが無いためフェーズ6送りで、
 * デザインどおりラジオを並べたまま無効にする (読み取り専用の欄に化けさせない)。
 */
const renumberScope = ref<"all" | "selected">("all");
const renumberPopover = ref<HTMLElement | null>(null);
/** プレビュー (適用前に、実際に付く端子番号の一覧を見せる)。 */
const renumberPreview = computed(() =>
  terminals.renumberPreview(renumberStart.value, renumberStep.value, renumberPrefix.value),
);
/** プレビューの対応表 (`1 → 11`)。多段端子台は段記号を保った対応がそのまま出る。 */
const renumberMapping = computed(() =>
  terminals.renumberMapping(renumberStart.value, renumberStep.value, renumberPrefix.value),
);
/** ジャンパ指定の追従 (ジャンパが無ければnull)。 */
const renumberJumperPreview = computed(() =>
  terminals.renumberJumperPreview(renumberStart.value, renumberStep.value, renumberPrefix.value),
);
/** 適用できない理由 (適用できるならnull)。 */
const renumberIssue = computed(() =>
  terminals.renumberIssue(renumberStart.value, renumberStep.value, renumberPrefix.value),
);
/** 指摘行の文。 */
const renumberIssueText = computed(() => {
  const issue = renumberIssue.value;
  if (!issue) return "";
  const key = `terminals.renumber.issue${issue.code[0].toUpperCase()}${issue.code.slice(1)}`;
  return t(key, { ...issue });
});

function toggleRenumber() {
  if (showRenumber.value) {
    showRenumber.value = false;
    return;
  }
  renumberStart.value = 1;
  renumberStep.value = 1;
  renumberPrefix.value = "";
  renumberScope.value = "all";
  showRenumber.value = true;
}

function cancelRenumber() {
  showRenumber.value = false;
}

// Escとポップオーバーの外側クリックで閉じる (このフェーズの残り4つのポップオーバーが
// 真似する型になるので、最初のここで閉じ方まで決めておく)
function onRenumberKeydown(e: KeyboardEvent) {
  if (e.key === "Escape" && showRenumber.value) {
    e.stopPropagation();
    showRenumber.value = false;
  }
}

function onRenumberOutsideClick(e: MouseEvent) {
  if (!showRenumber.value) return;
  const el = renumberPopover.value;
  const target = e.target as Node | null;
  // 採番ボタン自身のクリックは toggleRenumber に任せる (ここで閉じると開き直せない)
  if (el && target && !el.contains(target) && !(target instanceof Node && el.parentElement?.contains(target))) {
    showRenumber.value = false;
  }
}

watch(showRenumber, (open) => {
  if (open) {
    window.addEventListener("keydown", onRenumberKeydown, true);
    window.addEventListener("mousedown", onRenumberOutsideClick, true);
  } else {
    window.removeEventListener("keydown", onRenumberKeydown, true);
    window.removeEventListener("mousedown", onRenumberOutsideClick, true);
  }
});

onUnmounted(() => {
  window.removeEventListener("keydown", onRenumberKeydown, true);
  window.removeEventListener("mousedown", onRenumberOutsideClick, true);
});

async function applyRenumber() {
  const from = terminals.displayTerminalNumbers.join(",");
  if (!(await terminals.renumberTerminals(renumberStart.value, renumberStep.value, renumberPrefix.value))) {
    if (terminals.error) ui.log(terminals.error);
    return;
  }
  showRenumber.value = false;
  ui.log(
    t("terminals.renumberedLog", {
      reference: terminals.reference,
      from,
      to: renumberPreview.value.join(","),
    }),
  );
}

// --- 多段ポップオーバー (Task 3。デザイン「多段」準拠、採番ポップオーバーと同じ開閉の型) ---
// 段数ステッパー (デザインシステムの共通ステッパー = SymbolPickerDialogの挿入ダイアログと
// 同じマークアップ・CSS)。適用できない (段を減らす対象に結線・ジャンパ・ピングループ等の
// 参照が残っている) 場合はRust側が拒否するので、その理由をポップオーバーの中に出す
// (採番ポップオーバーがプレビューで事前に断るのに対し、こちらは適用してから理由を見せる:
// 判定が結線・ジャンパ・ピングループを跨ぐため、双子をもう1つ持たずRust側に任せる)。
const showLevels = ref(false);
const levelsInput = ref(1);
const levelsError = ref("");
const levelsPopover = ref<HTMLElement | null>(null);

function toggleLevels() {
  if (showLevels.value) {
    showLevels.value = false;
    return;
  }
  levelsInput.value = terminals.levels;
  levelsError.value = "";
  showLevels.value = true;
}

function cancelLevels() {
  showLevels.value = false;
}

function stepLevels(n: number) {
  levelsInput.value = clampLevels(n);
}

function onLevelsKeydown(e: KeyboardEvent) {
  if (e.key === "Escape" && showLevels.value) {
    e.stopPropagation();
    showLevels.value = false;
  }
}

function onLevelsOutsideClick(e: MouseEvent) {
  if (!showLevels.value) return;
  const el = levelsPopover.value;
  const target = e.target as Node | null;
  if (el && target && !el.contains(target) && !(target instanceof Node && el.parentElement?.contains(target))) {
    showLevels.value = false;
  }
}

watch(showLevels, (open) => {
  if (open) {
    window.addEventListener("keydown", onLevelsKeydown, true);
    window.addEventListener("mousedown", onLevelsOutsideClick, true);
  } else {
    window.removeEventListener("keydown", onLevelsKeydown, true);
    window.removeEventListener("mousedown", onLevelsOutsideClick, true);
  }
});

onUnmounted(() => {
  window.removeEventListener("keydown", onLevelsKeydown, true);
  window.removeEventListener("mousedown", onLevelsOutsideClick, true);
});

async function applyLevels() {
  levelsError.value = "";
  const before = terminals.levels;
  if (levelsInput.value === before) {
    showLevels.value = false;
    return;
  }
  if (!(await terminals.setLevels(levelsInput.value))) {
    levelsError.value = terminals.error ?? "";
    if (terminals.error) ui.log(terminals.error);
    return;
  }
  showLevels.value = false;
  ui.log(
    t("terminals.levelsChangedLog", { reference: terminals.reference, from: before, to: levelsInput.value }),
  );
}

const title = computed(() =>
  terminals.reference
    ? t("terminals.titleWith", { reference: terminals.reference, poles: terminals.poleCount })
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

/** 並べ替え順のドロップダウンを変える (update_entity 1回。Cmd+Zで戻る)。 */
async function changeSort(event: Event) {
  // ドロップダウンが返す生の文字列は双子の純関数で`RowOrder`に落とす (未知語は数値順)
  const order = rowOrderOf({ row_order: (event.target as HTMLSelectElement).value });
  if (!(await terminals.setRowOrder(order))) {
    if (terminals.error) ui.log(terminals.error);
    return;
  }
  ui.log(
    t("terminals.sortChangedLog", {
      reference: terminals.reference,
      order: t(`terminals.sort.${order}`),
    }),
  );
}

/**
 * 「図面へジャンプ」 (ツールバー・行のダブルクリック共通)。既存のreveal動線をそのまま使う。
 * ストア側がエディタを閉じてからrevealするので、ジャンプ先がスクリムの下に隠れない
 * (開き直せば同じ端子台・同じ行選択に戻る)。
 */
function gotoTerminal() {
  const reference = terminals.reference;
  if (!reference) return;
  terminals.gotoTerminal(reveal);
  ui.log(t("terminals.gotoLog", { reference }));
}

/**
 * 行クリックで選択をトグルする。ダブルクリックの2回目 (`detail > 1`) は数えない
 * ——数えると1回のダブルクリックで選択が2回動いてしまう (打ち消し合うのは偶然)。
 */
function clickRow(terminal: string, event: MouseEvent) {
  if (event.detail > 1) return;
  terminals.toggleRow(terminal);
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
            {{ t("terminals.blockOption", { reference: b.reference, poles: b.poles, sheet: b.sheet_name }) }}
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
        <label class="sort-label">
          {{ t("terminals.sort.label") }}
          <select
            class="picker mono sort-picker"
            :value="terminals.rowOrder"
            :disabled="!terminals.entityId"
            @change="changeSort"
          >
            <option v-for="o in ROW_ORDERS" :key="o" :value="o">{{ t(`terminals.sort.${o}`) }}</option>
          </select>
        </label>
        <span class="tool-sep" />
        <div class="popover-anchor">
          <button class="tool" :disabled="!terminals.entityId" @click="toggleRenumber">
            <ListOrdered :size="12" class="tool-icon" />
            {{ t("terminals.autoNumber") }}
          </button>
          <div v-if="showRenumber" ref="renumberPopover" class="popover" @click.stop>
            <div class="popover-title">{{ t("terminals.autoNumber") }}</div>
            <div class="popover-desc">{{ t("terminals.renumber.description") }}</div>
            <label class="popover-field">
              <span>{{ t("terminals.renumber.start") }}</span>
              <input v-model.number="renumberStart" type="number" min="0" class="popover-input mono" />
            </label>
            <label class="popover-field">
              <span>{{ t("terminals.renumber.step") }}</span>
              <input v-model.number="renumberStep" type="number" min="1" class="popover-input mono" />
            </label>
            <label class="popover-field">
              <span>{{ t("terminals.renumber.prefix") }}</span>
              <input v-model="renumberPrefix" type="text" class="popover-input mono" />
            </label>
            <div class="popover-field">
              <span>{{ t("terminals.renumber.target") }}</span>
              <span class="popover-radios">
                <label class="popover-radio">
                  <input v-model="renumberScope" type="radio" value="all" />
                  {{ t("terminals.renumber.targetAll") }}
                </label>
                <label class="popover-radio" :title="t('terminals.renumber.targetSelectedHint')">
                  <input v-model="renumberScope" type="radio" value="selected" disabled />
                  {{ t("terminals.renumber.targetSelected", { count: terminals.selectedTerminals.length }) }}
                </label>
              </span>
            </div>
            <div class="popover-preview">
              <span>{{ t("terminals.renumber.preview") }}</span>
              <div class="popover-box" :class="{ invalid: renumberIssue !== null }">
                <span v-for="m in renumberMapping" :key="m.from" class="mono map">
                  {{ m.from }} → {{ m.to }}
                </span>
              </div>
              <span v-if="renumberJumperPreview" class="mono jumper-follow">
                {{
                  t("terminals.renumber.jumpers", {
                    from: renumberJumperPreview.from,
                    to: renumberJumperPreview.to,
                  })
                }}
              </span>
            </div>
            <div v-if="renumberIssue" class="popover-issue">
              <TriangleAlert :size="12" class="issue-icon" />
              <span>{{ renumberIssueText }}</span>
            </div>
            <div class="popover-note">{{ t("terminals.renumber.note") }}</div>
            <div class="popover-actions">
              <button class="btn secondary" @click="cancelRenumber">
                {{ t("terminals.renumber.cancel") }}
              </button>
              <button
                class="btn primary"
                :disabled="terminals.busy || renumberIssue !== null"
                @click="applyRenumber"
              >
                {{ t("terminals.renumber.apply") }}
              </button>
            </div>
          </div>
        </div>
        <div class="popover-anchor">
          <button class="tool" :disabled="!terminals.entityId" @click="toggleLevels">
            <Layers :size="12" class="tool-icon" />
            {{ t("terminals.multiLevel") }}
          </button>
          <div v-if="showLevels" ref="levelsPopover" class="popover" @click.stop>
            <div class="popover-title">{{ t("terminals.multiLevel") }}</div>
            <div class="popover-desc">{{ t("terminals.levels.description") }}</div>
            <label class="popover-field">
              <span>{{ t("terminals.levels.label") }}</span>
              <div class="stepper">
                <button class="step" :disabled="levelsInput <= 1" @click="stepLevels(levelsInput - 1)">
                  <Minus :size="12" />
                </button>
                <input
                  class="step-value"
                  :value="levelsInput"
                  @change="stepLevels(Number(($event.target as HTMLInputElement).value))"
                />
                <button
                  class="step"
                  :disabled="levelsInput >= TERMINAL_LEVEL_MAX"
                  @click="stepLevels(levelsInput + 1)"
                >
                  <Plus :size="12" />
                </button>
              </div>
            </label>
            <div class="popover-note">{{ t("terminals.levels.hint", { max: TERMINAL_LEVEL_MAX }) }}</div>
            <div v-if="levelsError" class="popover-issue">
              <TriangleAlert :size="12" class="issue-icon" />
              <span>{{ levelsError }}</span>
            </div>
            <div class="popover-actions">
              <button class="btn secondary" @click="cancelLevels">
                {{ t("terminals.levels.cancel") }}
              </button>
              <button class="btn primary" :disabled="terminals.busy" @click="applyLevels">
                {{ t("terminals.levels.apply") }}
              </button>
            </div>
          </div>
        </div>
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
        <button class="tool" :disabled="!terminals.entityId" @click="gotoTerminal">
          <Unplug :size="12" class="tool-icon" />
          {{ t("terminals.goto") }}
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
            <div v-if="terminals.hasLevels" class="cell level">{{ t("terminals.col.level") }}</div>
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
            :title="t('terminals.gotoHint')"
            @click="clickRow(r.terminal, $event)"
            @dblclick="gotoTerminal"
          >
            <div class="cell no mono">{{ r.no }}</div>
            <div class="cell target mono">{{ r.external }}</div>
            <div class="cell cable mono">{{ r.externalCable }}</div>
            <div class="cell jumper mono">{{ r.jumper }}</div>
            <div class="cell terminal mono strong">{{ r.terminal }}</div>
            <div v-if="terminals.hasLevels" class="cell level mono">{{ r.level }}</div>
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
.sort-label {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
  color: var(--ui-muted);
}
.sort-picker { padding: 3px 6px; font-size: 11px; }

/* --- 採番ポップオーバー --- */
.popover-anchor { position: relative; display: flex; }
.popover {
  position: absolute;
  top: calc(100% + 6px);
  left: 0;
  z-index: 10;
  display: flex;
  flex-direction: column;
  gap: 8px;
  width: 248px;
  padding: 10px;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 6px;
  box-shadow: var(--shadow-panel-lg);
}
.popover-title { font-size: 11px; font-weight: 600; color: var(--ui-text); }
.popover-field {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  font-size: 11px;
  color: var(--ui-muted);
}
.popover-input {
  width: 96px;
  background: var(--input-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  padding: 3px 6px;
  font-size: 11px;
  color: var(--ui-text);
}
.popover-desc { font-size: 10px; color: var(--ui-muted); }
.popover-radios { display: flex; gap: 10px; }
.popover-radio { display: flex; align-items: center; gap: 4px; color: var(--ui-text); }
.popover-radio input { margin: 0; }
.popover-radio:has(input:disabled) { color: var(--ui-muted); }
.popover-preview {
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-size: 10px;
  color: var(--ui-muted);
  padding-top: 4px;
  border-top: 1px solid var(--ribbon-line);
}
.popover-preview .mono { color: var(--ui-text); word-break: break-all; }
.popover-box {
  display: flex;
  flex-wrap: wrap;
  gap: 2px 10px;
  max-height: 88px;
  overflow-y: auto;
  padding: 4px 6px;
  background: var(--input-bg);
  border-radius: 4px;
}
.popover-box.invalid { background: var(--err-bg); }
.popover-box.invalid .map { color: var(--err-fg); }
.jumper-follow { padding-top: 2px; }
.popover-issue {
  display: flex;
  align-items: flex-start;
  gap: 4px;
  padding: 5px 6px;
  border-radius: 4px;
  background: var(--err-bg);
  color: var(--err-fg);
  font-size: 10px;
}
.popover-issue .issue-icon { flex: none; margin-top: 1px; }
.popover-note { font-size: 10px; color: var(--ui-muted); }
.popover-actions { display: flex; justify-content: flex-end; gap: 6px; }
.popover-actions .btn { min-width: 0; padding: 4px 12px; }

/* --- 段数ステッパー (デザインシステムの共通ステッパー。SymbolPickerDialogと同じ見た目) --- */
.stepper {
  display: flex;
  align-items: center;
  background: var(--input-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 8px;
  overflow: hidden;
}
.step {
  width: 22px;
  height: 24px;
  border: none;
  background: transparent;
  color: var(--ui-muted);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}
.step:hover:not(:disabled) { background: var(--hover-bg); }
.step:disabled { opacity: 0.4; cursor: default; }
.step-value {
  width: 36px;
  height: 24px;
  border: none;
  border-left: 1px solid var(--ribbon-line);
  border-right: 1px solid var(--ribbon-line);
  background: var(--card-bg);
  text-align: center;
  font-family: var(--mono-font);
  font-size: 12px;
  font-weight: 600;
  color: var(--ui-text);
  outline: none;
}

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
.cell.level { width: 32px; flex: none; }
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
