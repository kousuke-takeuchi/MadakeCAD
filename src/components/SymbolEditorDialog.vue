<script setup lang="ts">
// シンボルエディタ (.pen「M4デザイン - シンボルエディタ」= ACADE Symbol Builder様式)。
//
// CAD調モーダル幅1180: palette-headヘッダ /
// 左=属性エディタパレット (必須属性 TAG / 任意属性 PART・DESC / 定格 RATING / 接続点表+
//    方向別追加ボタン) /
// ツール列40px (選/線/矩/円/弧/文/属/ピ) /
// 中=白紙2.5mmグリッドキャンバス (SymbolEditorCanvas) /
// 右=シンボル情報 + 回転バリアント4種の実プレビュー + 保存前チェック /
// フッタ=既存シンボルを複製して新規… + 保存先パス + キャンセル + ユーザーライブラリへ保存。
//
// エディタはファイルのドメインで、図面のCommandエンジンとは別。保存は
// `~/MadakeCAD/symbols/{id}.json` への書き込みで、**undoでは戻らない**。
// 「複製から新規」はモーダルの上にモーダルを重ねないようポップオーバーで出す。
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { AlertTriangle, Check, Copy, FolderOpen, X, XCircle } from "lucide-vue-next";
import { PIN_DIRS, SYMBOL_TOOLS, type SymbolCheck, type SlotKey } from "../canvas/symbolEditor";
import { symbolName } from "../canvas/symbolLibrary";
import { useSymbolEditorStore } from "../stores/symbolEditor";
import { useUiStore } from "../stores/ui";
import SymbolEditorCanvas from "./SymbolEditorCanvas.vue";
import SymbolPreview from "./SymbolPreview.vue";

const editor = useSymbolEditorStore();
const ui = useUiStore();
const { t, te, locale } = useI18n();

/** 属性パレットのグループ (必須 / 任意 / 定格)。デザインの3グループに対応する。 */
const SLOT_GROUPS: { name: string; keys: SlotKey[] }[] = [
  { name: "required", keys: ["TAG"] },
  { name: "optional", keys: ["PART", "DESC"] },
  { name: "rating", keys: ["RATING"] },
];

/** 回転バリアント帯の角度 (自動導出なのでプレビューのみ)。 */
const VARIANTS = [0, 90, 180, 270];

const title = computed(() => {
  const def = editor.draft;
  const name = def ? symbolName(def, locale.value) : "";
  return name ? t("symbolEditor.titleWith", { name }) : t("symbolEditor.title");
});

/** 保存前チェック1行の文言 (見出し行は合否ごとに文が変わる)。 */
function checkText(row: SymbolCheck): string {
  const key = row.code.startsWith("check.")
    ? `symbolEditor.check.${row.code.slice("check.".length)}.${row.level}`
    : `symbolEditor.issue.${row.code}`;
  return te(key) ? t(key, row.params) : row.code;
}

function close() {
  if (!editor.requestClose()) return;
  ui.log(t("symbolEditor.closedLog"));
}

function discard() {
  editor.discardAndClose();
  ui.log(t("symbolEditor.discardedLog"));
}

async function save() {
  const path = await editor.save();
  if (!path) {
    const reasons = editor.saveErrors.map((e) => e.message).join(" / ");
    ui.log(t("symbolEditor.saveFailedLog", { reasons }));
    return;
  }
  ui.log(t("symbolEditor.savedLog", { id: editor.draft?.id ?? "", path }));
}

async function pick(id: string) {
  await editor.pickDuplicate(id);
  ui.log(t("symbolEditor.duplicatedLog", { source: id, id: editor.draft?.id ?? "" }));
}

async function openFolder() {
  const { path, opened } = await editor.openUserFolder();
  ui.log(t(opened ? "symbolEditor.folderOpenedLog" : "symbolEditor.folderHintLog", { path }));
}
</script>

<template>
  <div v-if="editor.open && editor.draft" class="overlay" @click.self="close">
    <div class="dialog">
      <div class="titlebar">
        <span class="title">{{ title }}</span>
        <span class="caption">{{ t("symbolEditor.library") }}</span>
        <span class="caption">{{ t("symbolEditor.kind") }}</span>
        <span class="spacer" />
        <button class="close" :title="t('symbolEditor.close')" @click="close">
          <X :size="14" />
        </button>
      </div>

      <div class="body">
        <!-- 左: 属性エディタパレット -->
        <div class="palette">
          <div v-for="group in SLOT_GROUPS" :key="group.name" class="group">
            <div class="group-head">{{ t(`symbolEditor.group.${group.name}`) }}</div>
            <div v-for="key in group.keys" :key="key" class="slot-row">
              <label class="slot-label mono">{{ t(`symbolEditor.slot.${key}`) }}</label>
              <input
                class="input"
                :value="editor.samples[key] ?? ''"
                :placeholder="editor.sampleFor(key)"
                spellcheck="false"
                @input="editor.setSample(key, ($event.target as HTMLInputElement).value)"
              />
              <button
                class="slot-state"
                :class="{ placed: !!editor.slotAt(key) }"
                :title="t('symbolEditor.slotToggle')"
                @click="editor.toggleSlot(key)"
              >
                {{ t(editor.slotAt(key) ? "symbolEditor.placed" : "symbolEditor.unplaced") }}
              </button>
            </div>
          </div>

          <div class="group grow">
            <div class="group-head">
              {{ t("symbolEditor.group.pins") }}
              <span class="spacer" />
              <span class="count">{{ t("symbolEditor.pinCount", { count: editor.pinRows.length }) }}</span>
            </div>
            <div class="grid">
              <div class="row head">
                <div class="cell no">#</div>
                <div class="cell number">{{ t("symbolEditor.col.number") }}</div>
                <div class="cell dir">{{ t("symbolEditor.col.dir") }}</div>
                <div class="cell name">{{ t("symbolEditor.col.name") }}</div>
              </div>
              <div v-if="editor.pinRows.length === 0" class="empty">
                {{ t("symbolEditor.noPins") }}
              </div>
              <div
                v-for="(pin, i) in editor.pinRows"
                :key="i"
                class="row"
                :class="{
                  selected: editor.selection?.kind === 'pin' && editor.selection.index === i,
                }"
                @click="editor.select({ kind: 'pin', index: i })"
              >
                <div class="cell no mono">{{ i + 1 }}</div>
                <input
                  class="cell number mono edit"
                  :value="pin.number"
                  spellcheck="false"
                  @input="editor.updatePin(i, { number: ($event.target as HTMLInputElement).value })"
                />
                <select
                  class="cell dir edit"
                  :value="pin.dir ?? 'right'"
                  @change="editor.updatePin(i, { dir: ($event.target as HTMLSelectElement).value as never })"
                >
                  <option v-for="d in PIN_DIRS" :key="d" :value="d">
                    {{ t(`symbolEditor.dir.${d}`) }}
                  </option>
                </select>
                <input
                  class="cell name edit"
                  :value="pin.name"
                  @input="editor.updatePin(i, { name: ($event.target as HTMLInputElement).value })"
                />
              </div>
            </div>
            <div class="dir-buttons">
              <button v-for="d in PIN_DIRS" :key="d" class="tool" @click="editor.addPin(d)">
                + {{ t(`symbolEditor.dir.${d}`) }}
              </button>
            </div>
          </div>
        </div>

        <!-- ツール列 -->
        <div class="tools">
          <button
            v-for="tool in SYMBOL_TOOLS"
            :key="tool"
            class="tool-btn"
            :class="{ active: editor.tool === tool }"
            :title="t(`symbolEditor.toolName.${tool}`)"
            @click="editor.setTool(tool)"
          >
            {{ t(`symbolEditor.tool.${tool}`) }}
          </button>
        </div>

        <!-- 中央: 白紙2.5mmグリッドキャンバス -->
        <SymbolEditorCanvas />

        <!-- 右: シンボル情報 + 回転バリアント + 保存前チェック -->
        <div class="side">
          <div class="group">
            <div class="group-head">{{ t("symbolEditor.group.info") }}</div>
            <div class="form-row">
              <label class="form-label">{{ t("symbolEditor.field.id") }}</label>
              <input
                class="input mono"
                :value="editor.draft.id"
                spellcheck="false"
                @input="editor.setInfo({ id: ($event.target as HTMLInputElement).value })"
              />
            </div>
            <div class="form-row">
              <label class="form-label">{{ t("symbolEditor.field.nameJa") }}</label>
              <input
                class="input"
                :value="editor.draft.name_ja"
                @input="editor.setInfo({ name_ja: ($event.target as HTMLInputElement).value })"
              />
            </div>
            <div class="form-row">
              <label class="form-label">{{ t("symbolEditor.field.name") }}</label>
              <input
                class="input"
                :value="editor.draft.name"
                @input="editor.setInfo({ name: ($event.target as HTMLInputElement).value })"
              />
            </div>
            <div class="form-row">
              <label class="form-label">{{ t("symbolEditor.field.prefix") }}</label>
              <input
                class="input mono"
                :value="editor.draft.ref_prefix"
                spellcheck="false"
                @input="editor.setInfo({ ref_prefix: ($event.target as HTMLInputElement).value })"
              />
            </div>
            <div class="form-row">
              <label class="form-label">{{ t("symbolEditor.field.category") }}</label>
              <input class="input readonly" :value="t('symbolPicker.category.user')" readonly />
            </div>
            <div class="form-row">
              <label class="form-label">{{ t("symbolEditor.field.keywords") }}</label>
              <input
                class="input"
                :value="(editor.draft.keywords ?? []).join(', ')"
                @input="editor.setKeywords(($event.target as HTMLInputElement).value)"
              />
            </div>
          </div>

          <div class="group">
            <div class="group-head">{{ t("symbolEditor.group.variants") }}</div>
            <div class="variants">
              <div v-for="deg in VARIANTS" :key="deg" class="variant" :class="{ first: deg === 0 }">
                <SymbolPreview
                  :def="editor.draft"
                  :rotation="deg"
                  :width="52"
                  :height="40"
                  :key="`${deg}-${editor.draft.primitives.length}-${editor.draft.pins.length}`"
                />
                <span class="caption">{{ deg }}°</span>
              </div>
            </div>
            <p class="caption">{{ t("symbolEditor.variantsHint") }}</p>
          </div>

          <div class="group grow">
            <div class="group-head">{{ t("symbolEditor.group.checks") }}</div>
            <div v-for="(row, i) in editor.checks" :key="i" class="check" :class="row.level">
              <Check v-if="row.level === 'ok'" :size="12" />
              <AlertTriangle v-else-if="row.level === 'warn'" :size="12" />
              <XCircle v-else :size="12" />
              <span>{{ checkText(row) }}</span>
            </div>
            <p v-for="(e, i) in editor.saveErrors" :key="`e${i}`" class="check error">
              <XCircle :size="12" />
              <span>{{ e.message }}</span>
            </p>
            <p v-if="editor.error" class="check error">
              <XCircle :size="12" />
              <span>{{ editor.error }}</span>
            </p>
          </div>
        </div>
      </div>

      <div class="footer">
        <div class="picker-anchor">
          <button class="btn secondary" @click="editor.pickerOpen ? editor.closePicker() : editor.openPicker()">
            <Copy :size="13" />
            {{ t("symbolEditor.duplicateFrom") }}
          </button>
          <div v-if="editor.pickerOpen" class="picker">
            <input
              class="input"
              :value="editor.pickerQuery"
              :placeholder="t('symbolEditor.pickerSearch')"
              spellcheck="false"
              @input="editor.setPickerQuery(($event.target as HTMLInputElement).value)"
            />
            <div class="picker-list">
              <button
                v-for="def in editor.pickerRows"
                :key="def.id"
                class="picker-row"
                @click="pick(def.id)"
              >
                <SymbolPreview :def="def" :width="40" :height="24" />
                <span class="picker-name">{{ symbolName(def, locale) }}</span>
                <span class="picker-id mono">{{ def.id }}</span>
              </button>
              <p v-if="editor.pickerRows.length === 0" class="caption">
                {{ t("symbolEditor.pickerEmpty") }}
              </p>
            </div>
          </div>
        </div>
        <button class="icon-btn" :title="t('symbolEditor.openFolder')" @click="openFolder">
          <FolderOpen :size="13" />
        </button>
        <span class="caption mono path">
          {{ editor.filePath || t("symbolEditor.noPath") }}
        </span>
        <span class="spacer" />
        <template v-if="editor.confirmingClose">
          <span class="caption warn">{{ t("symbolEditor.unsaved") }}</span>
          <button class="btn secondary" @click="editor.keepEditing()">
            {{ t("symbolEditor.keepEditing") }}
          </button>
          <button class="btn secondary" @click="discard">{{ t("symbolEditor.discard") }}</button>
        </template>
        <template v-else>
          <button class="btn secondary" @click="close">{{ t("symbolEditor.cancel") }}</button>
          <button class="btn primary" :disabled="!editor.canSave" @click="save">
            {{ t(editor.saving ? "symbolEditor.saving" : "symbolEditor.save") }}
          </button>
        </template>
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
  width: min(1180px, 96vw);
  height: min(760px, 92vh);
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
  gap: 14px;
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
  flex: 1;
  min-height: 0;
  display: flex;
  align-items: stretch;
}
.palette {
  width: 260px;
  flex: none;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 10px;
  overflow-y: auto;
  border-right: 1px solid var(--ribbon-line);
}
.side {
  width: 250px;
  flex: none;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 10px;
  overflow-y: auto;
  border-left: 1px solid var(--ribbon-line);
}
.group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.group.grow { flex: 1; min-height: 0; }
.group-head {
  display: flex;
  align-items: center;
  background: var(--palette-head);
  border-radius: 4px;
  padding: 4px 8px;
  font-size: 11px;
  font-weight: 600;
  color: var(--ui-text);
}
.count { font-size: 10px; font-weight: 400; color: var(--ui-muted); }

/* --- 属性パレット --- */
.slot-row { display: flex; align-items: center; gap: 6px; }
.slot-label { width: 78px; flex: none; font-size: 10px; color: var(--ui-text); }
.slot-state {
  flex: none;
  border: none;
  background: transparent;
  font-size: 10px;
  color: var(--ui-muted);
  cursor: pointer;
  padding: 0;
  width: 42px;
  text-align: left;
}
.slot-state.placed { color: var(--ok-fg); }

.input {
  flex: 1;
  min-width: 0;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  padding: 4px 7px;
  font-size: 11px;
  color: var(--ui-text);
  outline: none;
}
.input:focus { border-color: var(--acad-blue); }
.input.readonly { color: var(--ui-muted); background: var(--input-bg); }

/* --- 接続点表 --- */
.grid {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
}
.row {
  display: flex;
  align-items: stretch;
  border-bottom: 1px solid var(--ribbon-line);
}
.row:last-child { border-bottom: none; }
.row:not(.head):hover { background: var(--hover-bg); }
.row.head { background: var(--input-bg); position: sticky; top: 0; }
.row.head .cell {
  font-size: 10px;
  font-weight: 600;
  color: var(--ui-placeholder);
  padding: 5px 6px;
}
.row.selected { background: var(--sel-blue); }
.cell {
  display: flex;
  align-items: center;
  padding: 4px 6px;
  font-size: 11px;
  color: var(--ui-text);
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
}
.cell.no { width: 24px; flex: none; color: var(--ui-muted); }
.cell.number { width: 46px; flex: none; }
.cell.dir { width: 56px; flex: none; }
.cell.name { flex: 1; min-width: 0; }
.cell.edit {
  background: transparent;
  border: none;
  outline: none;
  border-radius: 4px;
}
.cell.edit:focus { background: var(--card-bg); box-shadow: inset 0 0 0 1px var(--acad-blue); }
.empty { padding: 10px 8px; font-size: 11px; color: var(--ui-muted); }
.dir-buttons { display: flex; gap: 4px; }
.tool {
  flex: 1;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  padding: 4px 0;
  font-size: 10px;
  color: var(--acad-blue);
  cursor: pointer;
}
.tool:hover { background: var(--hover-bg); }

/* --- ツール列 --- */
.tools {
  width: 40px;
  flex: none;
  display: flex;
  flex-direction: column;
  background: var(--ribbon-bg);
  border-right: 1px solid var(--ribbon-line);
  padding-top: 6px;
}
.tool-btn {
  height: 34px;
  border: none;
  background: transparent;
  font-size: 12px;
  color: var(--ui-text);
  cursor: pointer;
}
.tool-btn:hover { background: var(--hover-bg); }
.tool-btn.active { background: var(--sel-blue); font-weight: 600; }

/* --- 右パネル --- */
.form-row { display: flex; align-items: center; gap: 6px; }
.form-label { width: 62px; flex: none; font-size: 10px; color: var(--ui-muted); }
.variants { display: flex; gap: 6px; }
.variant {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 2px;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  padding: 4px 0;
}
.variant.first { border-color: var(--acad-blue); }
.check {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  margin: 0;
  font-size: 10px;
  line-height: 15px;
  color: var(--ui-text);
}
.check.ok { color: var(--ok-fg); }
.check.warn { color: var(--warn-fg); }
.check.error { color: var(--err-fg); }
.caption { font-size: 10px; line-height: 15px; color: var(--ui-muted); margin: 0; }
.caption.warn { color: var(--warn-fg); }

/* --- フッタ --- */
.footer {
  flex: none;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  border-top: 1px solid var(--ribbon-line);
}
.path { max-width: 380px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.icon-btn {
  display: flex;
  border: 1px solid var(--ribbon-line);
  background: var(--card-bg);
  border-radius: 4px;
  padding: 5px;
  color: var(--ribbon-icon);
  cursor: pointer;
}
.icon-btn:hover { background: var(--hover-bg); }
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

/* --- 複製から新規 (ポップオーバー。2階層目のモーダルは作らない) --- */
.picker-anchor { position: relative; }
.picker {
  position: absolute;
  bottom: calc(100% + 6px);
  left: 0;
  width: 300px;
  max-height: 320px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 8px;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 6px;
  box-shadow: var(--shadow-panel-lg);
  z-index: 110;
}
.picker-list { overflow-y: auto; display: flex; flex-direction: column; }
.picker-row {
  display: flex;
  align-items: center;
  gap: 8px;
  border: none;
  background: transparent;
  padding: 4px 6px;
  cursor: pointer;
  text-align: left;
}
.picker-row:hover { background: var(--hover-bg); }
.picker-name { flex: 1; min-width: 0; font-size: 11px; color: var(--ui-text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.picker-id { font-size: 10px; color: var(--ui-muted); }
.mono { font-family: var(--mono-font); }
</style>
