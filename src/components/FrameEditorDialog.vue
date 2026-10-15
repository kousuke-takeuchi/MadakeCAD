<script setup lang="ts">
// 図枠エディタ (M4残り⑥、.pen「M4残りデザイン⑥」`dhP9P`)。EPLANのプロットフレームエディタ準拠:
// 図枠を線・矩形・文字で描き、プロパティを差し込む「プレースホルダ文字」を置く (出力時に実際の値へ置換)。
// 保存先は既存の図枠テンプレートJSON (形式は変えない) なので、図枠ダイアログ・一括適用はそのまま使える。
//
// エディタ内の⌘Z/⇧⌘Zはエディタ自身のundo (`stores/frameEditor.ts`)。図面のCommand履歴には積まない。
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Braces, Grid3x3, Image, Minus, MousePointer2, Square, Table2, Type, X } from "lucide-vue-next";
import {
  FRAME_TOOLS,
  bindsIn,
  deleteItem,
  filterPlaceholders,
  fromTb,
  toTb,
  updateCell,
  updateLine,
  type FrameTool,
  type PlaceholderGroup,
} from "../canvas/frameEditor";
import { frameTemplateName, type FrameTemplate } from "../canvas/frameTemplate";
import { useFrameEditorStore } from "../stores/frameEditor";
import { useUiStore } from "../stores/ui";
import FrameEditorCanvas from "./FrameEditorCanvas.vue";

const editor = useFrameEditorStore();
const ui = useUiStore();
const { t, locale } = useI18n();
const pickerQuery = ref("");

const TOOL_ICONS: Record<FrameTool, unknown> = {
  select: MousePointer2,
  line: Minus,
  rect: Square,
  text: Type,
  placeholder: Braces,
  cell: Table2,
  logo: Image,
  zone: Grid3x3,
};

const draft = computed(() => editor.draft);

/** 選択中のセル・罫線 (無ければnull)。 */
const cell = computed(() => {
  const s = editor.selection;
  return s?.kind === "cell" ? (draft.value?.title_block.cells[s.index] ?? null) : null;
});
const gridLine = computed(() => {
  const s = editor.selection;
  return s?.kind === "line" ? (draft.value?.title_block.grid_lines[s.index] ?? null) : null;
});
const cellPos = computed(() => (cell.value && draft.value ? fromTb(draft.value, cell.value) : null));
const linePts = computed(() => {
  const g = gridLine.value;
  const d = draft.value;
  return g && d ? [fromTb(d, { x: g.x1, y: g.y1 }), fromTb(d, { x: g.x2, y: g.y2 })] : null;
});

const propsTitle = computed(() => {
  const s = editor.selection;
  if (!s) return t("frameEditor.props.template");
  if (s.kind === "line") return t("frameEditor.props.line");
  if (s.kind === "logo") return t("frameEditor.props.logo");
  return cell.value?.bind != null ? t("frameEditor.props.placeholder") : t("frameEditor.props.text");
});

/** プレースホルダの書き方 `{名前}` (テンプレートの口ひげ記法と混ざらないよう関数で組む)。 */
function token(name: string): string {
  return "{" + name + "}";
}

/** 罫線の始点・終点。 */
const LINE_ENDS = [0, 1] as const;

const GROUPS: PlaceholderGroup[] = ["project", "sheet", "titleBlock"];
const pickerGroups = computed(() => {
  const all = filterPlaceholders(pickerQuery.value);
  return GROUPS.map((g) => ({ group: g, items: all.filter((p) => p.group === g) })).filter((g) => g.items.length);
});

const title = computed(() => {
  const d = draft.value;
  if (!d) return "";
  const name = `${frameTemplateName(d, locale.value)} (${d.id}.json)`;
  return editor.dirty || editor.isNew ? t("frameEditor.titleDirty", { name }) : t("frameEditor.title", { name });
});

const statusText = computed(() => {
  const d = draft.value;
  if (!d) return "";
  const c = editor.cursor;
  return t("frameEditor.status", {
    paper: `${d.paper.w}×${d.paper.h}`,
    at: c ? `${c.x.toFixed(0)}, ${c.y.toFixed(0)}` : "—",
  });
});

const checkLine = computed(() => {
  const c = editor.checks;
  if (c.errors.length) return { cls: "error", text: t("frameEditor.checks.errors", { list: c.errors.join(", ") }) };
  const text = t("frameEditor.checks.summary", { unknown: c.unknownBinds.length, outside: c.outside });
  return { cls: c.unknownBinds.length || c.outside ? "warn" : "ok", text };
});

const errorText = computed(() => {
  const e = editor.error;
  if (!e) return "";
  if (e === "id") return t("frameEditor.saveAs.badId");
  if (e === "taken") return t("frameEditor.saveAs.taken");
  return e;
});

function num(v: unknown): number | null {
  const n = Number(v);
  return Number.isFinite(n) ? n : null;
}

/** テンプレート全体の欄を書き換えて1段積む。 */
function patchTemplate(mutate: (t: FrameTemplate) => void) {
  const d = draft.value;
  if (!d) return;
  const next = JSON.parse(JSON.stringify(d)) as FrameTemplate;
  mutate(next);
  editor.commit(next);
}

function setCell(patch: Record<string, unknown>) {
  const s = editor.selection;
  if (s?.kind === "cell" && draft.value) editor.commit(updateCell(draft.value, s.index, patch));
}

function setCellPos(axis: "x" | "y", value: unknown) {
  const n = num(value);
  const d = draft.value;
  const p = cellPos.value;
  if (n == null || !d || !p) return;
  const rel = toTb(d, { ...p, [axis]: n });
  setCell({ [axis]: rel[axis] });
}

function setCellKind(kind: "text" | "bind") {
  const c = cell.value;
  if (!c) return;
  if (kind === "bind" && c.bind == null) setCell({ bind: "{project.name}" });
  if (kind === "text" && c.text == null) setCell({ text: (c.bind ?? "").replace(/[{}]/g, "") || t("frameEditor.newText") });
}

function setLinePoint(i: 0 | 1, axis: "x" | "y", value: unknown) {
  const n = num(value);
  const d = draft.value;
  const s = editor.selection;
  const pts = linePts.value;
  if (n == null || !d || s?.kind !== "line" || !pts) return;
  const rel = toTb(d, { ...pts[i], [axis]: n });
  const key = `${axis}${i + 1}` as "x1" | "y1" | "x2" | "y2";
  editor.commit(updateLine(d, s.index, { [key]: rel[axis] }));
}

function setLogo(key: "x" | "y" | "w" | "h", value: unknown) {
  const n = num(value);
  if (n == null) return;
  patchTemplate((t) => {
    if (t.logo) t.logo[key] = n;
  });
}

/** プレースホルダを選ぶ: セルのバインドをそれ1つにする。 */
function pick(name: string) {
  const c = cell.value;
  if (!c) return;
  const current = c.bind ?? "";
  const single = bindsIn(current).length <= 1 && current.trim().replace(/\{[^{}]*\}/, "") === "";
  setCell({ bind: single ? `{${name}}` : `${current} {${name}}` });
}

function deleteSelected() {
  const s = editor.selection;
  if (!s || !draft.value) return;
  editor.commit(deleteItem(draft.value, s));
  editor.select(null);
}

async function save() {
  if (await editor.save()) ui.log(t("frameEditor.savedLog", { id: editor.draft?.id ?? "" }));
}
async function saveAs() {
  if (await editor.saveAs()) ui.log(t("frameEditor.savedLog", { id: editor.draft?.id ?? "" }));
}

function isEditable(ev: KeyboardEvent): boolean {
  const el = ev.target as HTMLElement | null;
  if (!el) return false;
  // チェックボックス等は文字を打たないので、⌘Z・Deleteはエディタへ渡す
  if (el instanceof HTMLInputElement) return !["checkbox", "radio", "button"].includes(el.type);
  return el.tagName === "TEXTAREA" || el.tagName === "SELECT" || el.isContentEditable;
}

function onKeyDown(ev: KeyboardEvent) {
  if (!editor.open) return;
  const mod = ev.metaKey || ev.ctrlKey;
  if (mod && ev.key.toLowerCase() === "z" && !isEditable(ev)) {
    ev.preventDefault();
    ev.stopPropagation();
    if (ev.shiftKey) editor.redo();
    else editor.undo();
    return;
  }
  if (mod && ev.key.toLowerCase() === "y" && !isEditable(ev)) {
    ev.preventDefault();
    ev.stopPropagation();
    editor.redo();
    return;
  }
  if ((ev.key === "Delete" || ev.key === "Backspace") && !isEditable(ev) && editor.selection) {
    ev.preventDefault();
    ev.stopPropagation();
    deleteSelected();
    return;
  }
  if (ev.key === "Escape") {
    ev.preventDefault();
    ev.stopPropagation();
    if (editor.saveAsOpen) editor.saveAsOpen = false;
    else if (editor.confirmClose) editor.confirmClose = false;
    else if (editor.tool !== "select") editor.setTool("select");
    else if (editor.selection) editor.select(null);
    else editor.requestClose();
  }
}

onMounted(() => window.addEventListener("keydown", onKeyDown, true));
onBeforeUnmount(() => window.removeEventListener("keydown", onKeyDown, true));
watch(
  () => editor.selection,
  () => (pickerQuery.value = ""),
);
</script>

<template>
  <div v-if="editor.open && draft" class="overlay">
    <div class="dialog">
      <div class="titlebar">
        <span class="title">{{ title }}</span>
        <span class="spacer" />
        <button class="close" :title="t('frameEditor.close')" @click="editor.requestClose()"><X :size="14" /></button>
      </div>

      <div class="body">
        <!-- 左: ツール -->
        <div class="tools">
          <button
            v-for="tool in FRAME_TOOLS"
            :key="tool"
            class="tool-btn"
            :class="{ active: editor.tool === tool }"
            :title="t(`frameEditor.toolHint.${tool}`)"
            @click="editor.setTool(tool)"
          >
            <component :is="TOOL_ICONS[tool]" :size="14" />
            <span>{{ t(`frameEditor.tool.${tool}`) }}</span>
          </button>
          <span class="spacer" />
          <p class="caption tools-hint">{{ t("frameEditor.keysHint") }}</p>
        </div>

        <!-- 中央: 用紙 -->
        <div class="center">
          <FrameEditorCanvas />
          <div class="statusbar">
            <span class="mono">{{ statusText }}</span>
            <span class="spacer" />
            <label class="toggle"><input v-model="editor.hidePlaceholders" type="checkbox" /> {{ t("frameEditor.hidePlaceholders") }}</label>
            <label class="toggle"><input v-model="editor.showValues" type="checkbox" /> {{ t("frameEditor.showValues") }}</label>
          </div>
        </div>

        <!-- 右: プロパティ -->
        <div class="side">
          <div class="group-head">{{ t("frameEditor.props.head", { what: propsTitle }) }}</div>

          <!-- セル (固定文字 / プレースホルダ) -->
          <template v-if="cell && cellPos">
            <div class="form-row">
              <span class="form-label">{{ t("frameEditor.props.kind") }}</span>
              <select class="input" :value="cell.bind != null ? 'bind' : 'text'" @change="setCellKind(($event.target as HTMLSelectElement).value as 'text' | 'bind')">
                <option value="text">{{ t("frameEditor.props.kindText") }}</option>
                <option value="bind">{{ t("frameEditor.props.kindBind") }}</option>
              </select>
            </div>
            <div class="form-row">
              <span class="form-label">{{ cell.bind != null ? t("frameEditor.props.property") : t("frameEditor.props.content") }}</span>
              <input
                v-if="cell.bind != null"
                class="input mono"
                :value="cell.bind"
                @change="setCell({ bind: ($event.target as HTMLInputElement).value })"
              />
              <input v-else class="input" :value="cell.text ?? ''" @change="setCell({ text: ($event.target as HTMLInputElement).value })" />
            </div>
            <div class="form-row">
              <span class="form-label">{{ t("frameEditor.props.size") }}</span>
              <input class="input mono" type="number" min="0.5" step="0.5" :value="cell.size" @change="setCell({ size: num(($event.target as HTMLInputElement).value) ?? cell.size })" />
              <span class="unit">mm</span>
            </div>
            <div class="form-row">
              <span class="form-label">{{ t("frameEditor.props.align") }}</span>
              <select class="input" :value="cell.align ?? 'start'" @change="setCell({ align: ($event.target as HTMLSelectElement).value })">
                <option value="start">{{ t("frameEditor.props.alignStart") }}</option>
                <option value="middle">{{ t("frameEditor.props.alignMiddle") }}</option>
                <option value="end">{{ t("frameEditor.props.alignEnd") }}</option>
              </select>
              <select class="input" :value="cell.weight ?? 'normal'" @change="setCell({ weight: ($event.target as HTMLSelectElement).value })">
                <option value="normal">{{ t("frameEditor.props.weightNormal") }}</option>
                <option value="bold">{{ t("frameEditor.props.weightBold") }}</option>
              </select>
            </div>
            <div class="form-row">
              <span class="form-label">{{ t("frameEditor.props.position") }}</span>
              <input class="input mono" type="number" step="0.5" :value="cellPos.x" @change="setCellPos('x', ($event.target as HTMLInputElement).value)" />
              <input class="input mono" type="number" step="0.5" :value="cellPos.y" @change="setCellPos('y', ($event.target as HTMLInputElement).value)" />
            </div>
            <div class="form-row">
              <span class="form-label" />
              <label class="toggle"><input type="checkbox" :checked="!!cell.omit_if_empty" @change="setCell({ omit_if_empty: ($event.target as HTMLInputElement).checked })" /> {{ t("frameEditor.props.omitIfEmpty") }}</label>
            </div>

            <!-- プレースホルダを選ぶ -->
            <template v-if="cell.bind != null">
              <div class="group-head">{{ t("frameEditor.picker.head") }}</div>
              <input v-model="pickerQuery" class="input" :placeholder="t('frameEditor.picker.search')" />
              <div class="picker">
                <template v-for="g in pickerGroups" :key="g.group">
                  <div class="picker-group">{{ t(`frameEditor.picker.group.${g.group}`) }}</div>
                  <button
                    v-for="p in g.items"
                    :key="p.name"
                    class="picker-row"
                    :class="{ on: bindsIn(cell.bind ?? '').includes(p.name) }"
                    @click="pick(p.name)"
                  >
                    <span class="mono">{{ token(p.name) }}</span>
                    <span class="picker-label">{{ t(`frameEditor.picker.label.${p.name.replace('.', '_')}`) }}</span>
                  </button>
                </template>
              </div>
            </template>
          </template>

          <!-- 罫線 -->
          <template v-else-if="gridLine && linePts">
            <div v-for="i in LINE_ENDS" :key="i" class="form-row">
              <span class="form-label">{{ t(i === 0 ? "frameEditor.props.from" : "frameEditor.props.to") }}</span>
              <input class="input mono" type="number" step="0.5" :value="linePts[i].x" @change="setLinePoint(i, 'x', ($event.target as HTMLInputElement).value)" />
              <input class="input mono" type="number" step="0.5" :value="linePts[i].y" @change="setLinePoint(i, 'y', ($event.target as HTMLInputElement).value)" />
            </div>
          </template>

          <!-- ロゴ枠 -->
          <template v-else-if="editor.selection?.kind === 'logo' && draft.logo">
            <div class="form-row">
              <span class="form-label">{{ t("frameEditor.props.position") }}</span>
              <input class="input mono" type="number" :value="draft.logo.x" @change="setLogo('x', ($event.target as HTMLInputElement).value)" />
              <input class="input mono" type="number" :value="draft.logo.y" @change="setLogo('y', ($event.target as HTMLInputElement).value)" />
            </div>
            <div class="form-row">
              <span class="form-label">{{ t("frameEditor.props.sizeWH") }}</span>
              <input class="input mono" type="number" :value="draft.logo.w" @change="setLogo('w', ($event.target as HTMLInputElement).value)" />
              <input class="input mono" type="number" :value="draft.logo.h" @change="setLogo('h', ($event.target as HTMLInputElement).value)" />
            </div>
            <p class="caption">{{ t("frameEditor.props.logoHint") }}</p>
          </template>

          <!-- 何も選んでいない / ゾーンツール: テンプレート全体の設定 -->
          <template v-else>
            <div class="form-row">
              <span class="form-label">{{ t("frameEditor.props.nameJa") }}</span>
              <input class="input" :value="draft.name_ja" @change="patchTemplate((t) => (t.name_ja = ($event.target as HTMLInputElement).value))" />
            </div>
            <div class="form-row">
              <span class="form-label">{{ t("frameEditor.props.nameEn") }}</span>
              <input class="input" :value="draft.name" @change="patchTemplate((t) => (t.name = ($event.target as HTMLInputElement).value))" />
            </div>
            <div class="form-row">
              <span class="form-label">{{ t("frameEditor.props.paper") }}</span>
              <input class="input mono" type="number" :value="draft.paper.w" @change="patchTemplate((t) => (t.paper.w = num(($event.target as HTMLInputElement).value) ?? t.paper.w))" />
              <input class="input mono" type="number" :value="draft.paper.h" @change="patchTemplate((t) => (t.paper.h = num(($event.target as HTMLInputElement).value) ?? t.paper.h))" />
            </div>
            <div class="form-row">
              <span class="form-label">{{ t("frameEditor.props.margin") }}</span>
              <input class="input mono" type="number" min="0" :value="draft.frame_margin" @change="patchTemplate((t) => (t.frame_margin = num(($event.target as HTMLInputElement).value) ?? t.frame_margin))" />
              <span class="unit">mm</span>
            </div>

            <div class="group-head" :class="{ focus: editor.tool === 'zone' }">{{ t("frameEditor.props.zones") }}</div>
            <div class="form-row">
              <span class="form-label">{{ t("frameEditor.props.zoneCountsColsRows") }}</span>
              <input class="input mono" type="number" min="1" :value="draft.zones.cols" @change="patchTemplate((t) => (t.zones.cols = Math.max(1, Math.round(num(($event.target as HTMLInputElement).value) ?? 1))))" />
              <input class="input mono" type="number" min="1" :value="draft.zones.rows" @change="patchTemplate((t) => (t.zones.rows = Math.max(1, Math.round(num(($event.target as HTMLInputElement).value) ?? 1))))" />
            </div>
            <div class="form-row">
              <span class="form-label">{{ t("frameEditor.props.zoneLabels") }}</span>
              <select class="input" :value="draft.zones.col_labels" @change="patchTemplate((t) => (t.zones.col_labels = ($event.target as HTMLSelectElement).value as 'numeric' | 'alpha'))">
                <option value="numeric">1, 2, 3</option>
                <option value="alpha">A, B, C</option>
              </select>
              <select class="input" :value="draft.zones.row_labels" @change="patchTemplate((t) => (t.zones.row_labels = ($event.target as HTMLSelectElement).value as 'numeric' | 'alpha'))">
                <option value="numeric">1, 2, 3</option>
                <option value="alpha">A, B, C</option>
              </select>
            </div>
            <p class="caption">{{ t("frameEditor.props.zoneHint") }}</p>

            <div class="group-head">{{ t("frameEditor.props.titleBlock") }}</div>
            <div class="form-row">
              <span class="form-label">{{ t("frameEditor.props.corner") }}</span>
              <select class="input" :value="draft.title_block.origin" @change="patchTemplate((t) => (t.title_block.origin = ($event.target as HTMLSelectElement).value as FrameTemplate['title_block']['origin']))">
                <option v-for="o in ['bottom_right', 'bottom_left', 'top_right', 'top_left']" :key="o" :value="o">{{ t(`frameEditor.props.cornerName.${o}`) }}</option>
              </select>
            </div>
            <div class="form-row">
              <span class="form-label">{{ t("frameEditor.props.sizeWH") }}</span>
              <input class="input mono" type="number" :value="draft.title_block.w" @change="patchTemplate((t) => (t.title_block.w = num(($event.target as HTMLInputElement).value) ?? t.title_block.w))" />
              <input class="input mono" type="number" :value="draft.title_block.h" @change="patchTemplate((t) => (t.title_block.h = num(($event.target as HTMLInputElement).value) ?? t.title_block.h))" />
            </div>
            <div class="form-row">
              <span class="form-label">{{ t("frameEditor.props.rowH") }}</span>
              <input class="input mono" type="number" :value="draft.title_block.row_h" @change="patchTemplate((t) => (t.title_block.row_h = num(($event.target as HTMLInputElement).value) ?? t.title_block.row_h))" />
              <span class="unit">mm</span>
            </div>

            <div class="group-head">{{ t("frameEditor.props.revision") }}</div>
            <div class="form-row">
              <span class="form-label">{{ t("frameEditor.props.revRows") }}</span>
              <input class="input mono" type="number" min="1" :value="draft.revision_block.rows_max" @change="patchTemplate((t) => (t.revision_block.rows_max = Math.max(1, Math.round(num(($event.target as HTMLInputElement).value) ?? 1))))" />
            </div>
            <div class="form-row">
              <span class="form-label">{{ t("frameEditor.props.revCols") }}</span>
              <input
                v-for="(w, i) in draft.revision_block.col_w"
                :key="i"
                class="input mono narrow"
                type="number"
                :value="w"
                @change="patchTemplate((t) => (t.revision_block.col_w[i] = num(($event.target as HTMLInputElement).value) ?? w))"
              />
            </div>
            <p class="caption">{{ t("frameEditor.props.revHint") }}</p>
          </template>
        </div>
      </div>

      <!-- 別名で保存 / 閉じる確認 -->
      <div v-if="editor.saveAsOpen" class="bar">
        <span class="form-label">{{ t("frameEditor.saveAs.id") }}</span>
        <input v-model="editor.saveAsId" class="input mono" @keyup.enter="saveAs" />
        <span class="form-label">{{ t("frameEditor.saveAs.name") }}</span>
        <input v-model="editor.saveAsName" class="input" @keyup.enter="saveAs" />
        <button class="btn primary" :disabled="editor.saving" @click="saveAs">{{ t("frameEditor.saveAs.confirm") }}</button>
        <button class="btn secondary" @click="editor.saveAsOpen = false">{{ t("frameEditor.cancel") }}</button>
      </div>
      <div v-else-if="editor.confirmClose" class="bar warn">
        <span>{{ t("frameEditor.confirmClose") }}</span>
        <span class="spacer" />
        <button class="btn secondary" @click="editor.requestClose(true)">{{ t("frameEditor.discard") }}</button>
        <button class="btn primary" @click="editor.confirmClose = false">{{ t("frameEditor.keepEditing") }}</button>
      </div>

      <div class="footer">
        <span class="check" :class="checkLine.cls">{{ checkLine.text }}</span>
        <span v-if="errorText" class="check error">{{ errorText }}</span>
        <span class="spacer" />
        <button class="btn secondary" :disabled="editor.saving" @click="editor.openSaveAs()">{{ t("frameEditor.saveAsButton") }}</button>
        <button class="btn primary" :disabled="editor.saving || editor.checks.errors.length > 0" @click="save">{{ t("frameEditor.save") }}</button>
        <button class="btn secondary" @click="editor.requestClose()">{{ t("frameEditor.cancel") }}</button>
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
  z-index: 120;
}
.dialog {
  width: min(1240px, 96vw);
  height: min(800px, 94vh);
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
  padding: 8px 12px;
  background: var(--palette-head);
  flex: none;
}
.title { font-size: 12px; font-weight: 600; color: var(--ui-text); }
.spacer { flex: 1; }
.close { border: none; background: transparent; color: var(--ui-muted); display: flex; cursor: pointer; padding: 0; }

.body { flex: 1; min-height: 0; display: flex; }

/* --- ツール列 --- */
.tools {
  width: 132px;
  flex: none;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 8px 6px;
  border-right: 1px solid var(--ribbon-line);
}
.tool-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 30px;
  padding: 0 8px;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: var(--ui-text);
  font-size: 12px;
  cursor: pointer;
  text-align: left;
}
.tool-btn:hover { background: var(--hover-bg); }
.tool-btn.active { background: var(--sel-blue); color: var(--acad-blue); font-weight: 600; }
.tools-hint { padding: 0 4px; white-space: pre-line; }

/* --- 中央 --- */
.center { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.statusbar {
  flex: none;
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 6px 10px;
  border-top: 1px solid var(--ribbon-line);
  font-size: 11px;
  color: var(--ui-muted);
}
.toggle { display: flex; align-items: center; gap: 5px; font-size: 11px; color: var(--ui-text); }

/* --- 右 --- */
.side {
  width: 300px;
  flex: none;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px;
  overflow-y: auto;
  border-left: 1px solid var(--ribbon-line);
}
.group-head {
  background: var(--palette-head);
  border-radius: 4px;
  padding: 4px 8px;
  font-size: 11px;
  font-weight: 600;
  color: var(--ui-text);
}
.group-head.focus { box-shadow: inset 0 0 0 1px var(--acad-blue); }
.form-row { display: flex; align-items: center; gap: 6px; }
.form-label { width: 72px; flex: none; font-size: 10px; color: var(--ui-muted); }
.unit { font-size: 10px; color: var(--ui-muted); }
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
.input.narrow { padding: 4px 4px; }
.caption { font-size: 10px; line-height: 15px; color: var(--ui-muted); margin: 0; }

.picker { display: flex; flex-direction: column; border: 1px solid var(--ribbon-line); border-radius: 4px; background: var(--card-bg); }
.picker-group { padding: 5px 8px 2px; font-size: 10px; color: var(--ui-muted); }
.picker-row {
  display: flex;
  align-items: center;
  gap: 8px;
  border: none;
  background: transparent;
  padding: 4px 8px;
  font-size: 11px;
  color: var(--ui-text);
  cursor: pointer;
  text-align: left;
}
.picker-row:hover { background: var(--hover-bg); }
.picker-row.on { background: var(--sel-blue); }
.picker-label { margin-left: auto; font-size: 10px; color: var(--ui-muted); }

/* --- 別名で保存 / 確認 / フッタ --- */
.bar {
  flex: none;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  border-top: 1px solid var(--ribbon-line);
  background: var(--card-bg);
  font-size: 11px;
  color: var(--ui-text);
}
.bar.warn { background: var(--warn-bg); }
.footer {
  flex: none;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px;
  border-top: 1px solid var(--ribbon-line);
}
.check { font-size: 11px; }
.check.ok { color: var(--ok-fg); }
.check.warn { color: var(--warn-fg); }
.check.error { color: var(--err-fg); }
.btn {
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
.mono { font-family: var(--mono-font); }
</style>
