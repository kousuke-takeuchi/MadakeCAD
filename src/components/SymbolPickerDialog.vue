<script setup lang="ts">
// 部品挿入ダイアログ (AutoCAD Electricalの部品選択ダイアログ相当)。
// デザイン: MadakeCAD.pen「部品挿入ダイアログ」(極数バー) +「M4デザイン - 回路マクロ」右半分
// (「マクロ」カテゴリ: 左カテゴリツリー + タイル(バリアント数バッジ) + 右プレビュー/バリアント行)。
// 文字列はすべてi18nカタログ経由 (docs/internal/specs/i18n.md)。
import { Minus, Plus, Search, X } from "lucide-vue-next";
import { computed, inject, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { DYNAMIC_PIN_MAX, dynamicSymbol } from "../canvas/dynamicSymbol";
import {
  macroDescription,
  macroName,
  macroValueSetLabel,
  macroVariantLabel,
} from "../canvas/macroPreview";
import { groupSymbolsByCategory } from "../canvas/symbolLibrary";
import { CONTACT_CONFIG_ATTR } from "../canvas/relayXref";
import type { Part } from "../ipc";
import type { EditorController } from "../tools/controller";
import { useDocumentStore } from "../stores/document";
import { ALL_CATEGORIES, UNCATEGORIZED, useMacrosStore } from "../stores/macros";
import { usePartsStore } from "../stores/parts";
import { useUiStore } from "../stores/ui";
import MacroPreview from "./MacroPreview.vue";
import SymbolPreview from "./SymbolPreview.vue";

const store = useDocumentStore();
const ui = useUiStore();
const parts = usePartsStore();
const macros = useMacrosStore();
const controller = inject<EditorController>("controller")!;
const { t, locale } = useI18n();
const query = ref("");

/** ダイアログ上部のカテゴリタブ。 */
type PickerTab = "parts" | "macros";
const tab = ref<PickerTab>("parts");

/** ピン数可変部品 (選択すると極数バーが出る)。 */
const dynamicBases = [
  { base: "terminal_block", key: "terminalBlock" },
  { base: "connector", key: "connector" },
] as const;
type DynBase = (typeof dynamicBases)[number]["base"];

const selectedDyn = ref<DynBase | null>(null);
const pinCount = ref(8);

const dynamicCells = computed(() => {
  const q = query.value.trim().toLowerCase();
  return dynamicBases
    .map((d) => ({ ...d, label: t(`symbolPicker.${d.key}`) }))
    .filter((d) => !q || d.label.toLowerCase().includes(q) || d.base.includes(q));
});

// カテゴリの並び・所属はバックエンドのライブラリ順 (symbol.rs CATEGORY_ORDER) に従う
const categories = computed(() => {
  const groups = groupSymbolsByCategory(store.symbols, query.value);
  // 動的セル(端子台/コネクタ)が検索に一致する場合はコネクタカテゴリを必ず出す
  if (dynamicCells.value.length > 0 && !groups.some(([cat]) => cat === "connector")) {
    groups.push(["connector", []]);
  }
  return groups;
});

/** カテゴリの表示名 (未知の分類はキーをそのまま出す)。 */
function categoryLabel(key: string): string {
  const path = `symbolPicker.category.${key}`;
  const label = t(path);
  return label === path ? key : label;
}

/** マクロのカテゴリツリーの表示名 (「すべて」「ユーザー」+ ユーザーが付けた分類名)。 */
function macroCategoryLabel(key: string): string {
  if (key === ALL_CATEGORIES) return t("macros.categoryAll");
  if (key === UNCATEGORIZED) return t("macros.categoryNone");
  return key;
}

/** 選択中の可変部品の現在極数でのプレビュー定義。 */
const dynPreview = (base: DynBase) =>
  dynamicSymbol(`${base}_${base === selectedDyn.value ? pinCount.value : 3}p`)!;

watch(
  () => ui.symbolPickerOpen,
  (open) => {
    if (open) {
      selectedDyn.value = null;
      macros.query = query.value;
      void parts.search(query.value);
      void macros.load();
    }
  },
);

// 検索語の変更を部品DB検索・マクロの絞り込みにも反映 (デバウンス)
let partsTimer: ReturnType<typeof setTimeout> | undefined;
watch(query, (q) => {
  macros.query = q;
  clearTimeout(partsTimer);
  partsTimer = setTimeout(() => void parts.search(q), 200);
});

function clampPins(n: number) {
  pinCount.value = Math.min(DYNAMIC_PIN_MAX, Math.max(1, Math.round(n) || 1));
}

function pick(symbolId: string) {
  controller.setTool("place", symbolId);
  ui.symbolPickerOpen = false;
  ui.log(t("symbolPicker.placeLog", { symbol: symbolId }));
}

function placeDynamic() {
  if (!selectedDyn.value) return;
  pick(`${selectedDyn.value}_${pinCount.value}p`);
}

/** 部品DBの部品を配置: 既定シンボルを型番(value)・定格(attrs.current_a)付きで置く。 */
function pickPart(part: Part) {
  if (!part.symbol_id || !store.resolveSymbol(part.symbol_id)) {
    ui.log(
      t("symbolPicker.noSymbolLog", { part: part.part_no, symbol: part.symbol_id || "-" }),
    );
    return;
  }
  const attrs: Record<string, string> = {};
  if (part.rated_current_a != null) attrs.current_a = String(part.rated_current_a);
  // 接点構成はコイル⇔接点XRefの接点数検証に使う (図面はDBのスナップショット)
  if (part.contact_config) attrs[CONTACT_CONFIG_ATTR] = part.contact_config;
  controller.setTool("place", part.symbol_id, { value: part.part_no, attrs });
  ui.symbolPickerOpen = false;
  ui.log(t("symbolPicker.placePartLog", { part: part.part_no }));
}

/** 値セットのドロップダウン (「保存時のまま」=未選択)。 */
function chooseValueSet(event: Event) {
  macros.setValueSet((event.target as HTMLSelectElement).value || null);
}

/** 選んだマクロの配置モードへ入る (ゴースト表示 → クリックで確定)。 */
function placeMacro() {
  const macro = macros.selected;
  if (!macro) return;
  controller.startMacroPlacement(macro, { fromLibrary: true, valueSet: macros.valueSetId });
  controller.macroVariantIndex = Math.max(0, macros.variantKeys.indexOf(macros.variantKey));
  ui.symbolPickerOpen = false;
  ui.log(t("macros.placeLog", { name: macroName(macro, locale.value) }));
}
</script>

<template>
  <div v-if="ui.symbolPickerOpen" class="overlay" @click.self="ui.symbolPickerOpen = false">
    <div class="dialog" :class="{ wide: tab === 'macros' }">
      <div class="dialog-head">
        <span>{{ t("symbolPicker.title") }}</span>
        <button class="close" :title="t('symbolPicker.close')" @click="ui.symbolPickerOpen = false">
          <X :size="14" />
        </button>
      </div>
      <div class="tabs">
        <button class="tab" :class="{ active: tab === 'parts' }" @click="tab = 'parts'">
          {{ t("macros.partsTab") }}
        </button>
        <button class="tab" :class="{ active: tab === 'macros' }" @click="tab = 'macros'">
          {{ t("macros.pickerTab") }}
        </button>
      </div>
      <div class="search-row">
        <Search :size="13" class="search-icon" />
        <input
          v-model="query"
          :placeholder="tab === 'macros' ? t('macros.searchPlaceholder') : t('symbolPicker.search')"
          autofocus
        />
      </div>

      <!-- 部品カテゴリ -->
      <div v-if="tab === 'parts'" class="body">
        <template v-for="[cat, syms] in categories" :key="cat">
          <div class="cat">{{ categoryLabel(cat) }}</div>
          <div class="grid">
            <button v-for="sym in syms" :key="sym.id" class="cell" @click="pick(sym.id)">
              <SymbolPreview :def="sym" :width="72" :height="40" />
              <span>{{ locale === "ja" ? sym.name_ja : sym.name }}</span>
            </button>
            <template v-if="cat === 'connector'">
              <button
                v-for="d in dynamicCells"
                :key="d.base"
                class="cell"
                :class="{ selected: selectedDyn === d.base }"
                @click="selectedDyn = d.base"
              >
                <SymbolPreview :def="dynPreview(d.base)" :width="72" :height="40" />
                <span>{{ d.label }}</span>
              </button>
            </template>
          </div>
        </template>
        <template v-if="parts.results.length > 0">
          <div class="cat">{{ t("symbolPicker.partsDb") }}</div>
          <div class="parts-list">
            <button v-for="p in parts.results" :key="p.part_no" class="part-row" @click="pickPart(p)">
              <span class="part-no">{{ p.part_no }}</span>
              <span class="part-name">{{ p.name }}</span>
              <span class="part-maker">{{ p.maker }}</span>
              <span class="part-spacer" />
              <span v-if="p.rated_voltage || p.rated_current_a != null" class="part-rating">
                {{ p.rated_voltage }}{{ p.rated_current_a != null ? ` ${p.rated_current_a}A` : "" }}
              </span>
            </button>
            <div class="parts-hint">{{ t("symbolPicker.partsHint") }}</div>
          </div>
        </template>
      </div>

      <!-- マクロカテゴリ (回路マクロのギャラリー) -->
      <div v-else class="macro-body">
        <div class="tree">
          <button
            v-for="c in macros.categories"
            :key="c.key"
            class="tree-row"
            :class="{ active: macros.category === c.key }"
            @click="macros.setCategory(c.key)"
          >
            <span class="tree-name">{{ macroCategoryLabel(c.key) }}</span>
            <span class="tree-count">{{ c.count }}</span>
          </button>
        </div>

        <div class="macro-grid">
          <button
            v-for="tile in macros.tiles"
            :key="tile.macro.id"
            class="macro-tile"
            :class="{ selected: tile.macro.id === macros.selectedId }"
            @click="macros.select(tile.macro.id)"
          >
            <span class="thumb-frame">
              <MacroPreview :macro="tile.macro" :width="180" :height="96" :labels="false" />
              <span class="badge">{{ t("macros.variantCount", { count: tile.variantCount }) }}</span>
            </span>
            <span class="macro-tile-name">{{ macroName(tile.macro, locale) }}</span>
          </button>
          <p v-if="macros.loading" class="caption">{{ t("macros.loading") }}</p>
          <p v-else-if="!macros.tiles.length" class="caption">{{ t("macros.empty") }}</p>
        </div>

        <div class="macro-side">
          <p class="side-title">
            {{ macros.selected ? macroName(macros.selected, locale) : t("macros.none") }}
          </p>
          <div v-if="macros.selected" class="variant-row">
            <button
              v-for="key in macros.variantKeys"
              :key="key"
              class="chip"
              :class="{ current: key === macros.variantKey }"
              @click="macros.setVariant(key)"
            >
              {{ macroVariantLabel(macros.selected, key, locale) }}
            </button>
            <span class="caption">{{ t("macros.variantSwitchHint") }}</span>
          </div>
          <div class="side-preview">
            <MacroPreview
              v-if="macros.selected"
              :key="`${macros.selectedId}-${macros.variantKey}`"
              :macro="macros.selected"
              :variant="macros.variantKey"
              :width="300"
              :height="180"
            />
          </div>
          <p v-if="macros.selected" class="side-desc">
            {{ macroDescription(macros.selected, locale) }}
          </p>
          <template v-if="macros.selected && macros.valueSets.length">
            <div class="row">
              <span class="label">{{ t("macros.valueSetRow") }}</span>
              <select
                class="select"
                :value="macros.valueSetId ?? ''"
                :title="t('macros.valueSetHint')"
                @change="chooseValueSet"
              >
                <option value="">{{ t("macros.valueSetNone") }}</option>
                <option v-for="v in macros.valueSets" :key="v.id" :value="v.id">
                  {{ macroValueSetLabel(v, locale) }}
                </option>
              </select>
            </div>
            <p class="caption">{{ t("macros.valueSetHint") }}</p>
          </template>
          <p v-for="issue in macros.issues" :key="issue.path" class="issue">
            {{ t("macros.issue", { path: issue.path, message: issue.message }) }}
          </p>
          <p v-if="macros.error" class="error">{{ macros.error }}</p>
          <span class="side-spacer" />
          <button class="place-btn" :disabled="!macros.selected" @click="placeMacro">
            {{ t("macros.place") }}
          </button>
        </div>
      </div>

      <div v-if="tab === 'parts' && selectedDyn" class="pins-bar">
        <span class="pins-label">{{ t("symbolPicker.pinsLabel") }}</span>
        <div class="stepper">
          <button class="step" @click="clampPins(pinCount - 1)"><Minus :size="12" /></button>
          <input
            class="step-value"
            :value="pinCount"
            @change="clampPins(Number(($event.target as HTMLInputElement).value))"
          />
          <button class="step" @click="clampPins(pinCount + 1)"><Plus :size="12" /></button>
        </div>
        <span class="pins-hint">{{ t("symbolPicker.pinsHint", { max: DYNAMIC_PIN_MAX }) }}</span>
        <span class="spacer" />
        <button class="place-btn" @click="placeDynamic">{{ t("symbolPicker.place") }}</button>
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
  width: 640px;
  max-height: 70vh;
  background: var(--ribbon-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  box-shadow: 0 12px 40px rgba(0, 0, 0, 0.3);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.dialog.wide {
  width: min(940px, 94vw);
  max-height: 82vh;
}
.dialog-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  background: var(--palette-head);
  font-size: 12px;
  font-weight: 600;
  color: var(--ui-text);
}
.close {
  border: none;
  background: transparent;
  cursor: pointer;
  color: var(--ui-muted);
  display: flex;
}
.tabs {
  display: flex;
  gap: 6px;
  padding: 8px 12px 0;
}
.tab {
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  background: var(--card-bg);
  padding: 4px 12px;
  font-size: 11px;
  font-weight: 600;
  color: var(--ui-muted);
  cursor: pointer;
}
.tab:hover { background: var(--hover-bg); }
.tab.active {
  background: var(--sel-blue);
  border-color: var(--acad-blue);
  color: var(--ui-text);
}
.search-row {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 8px 12px;
  padding: 5px 8px;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
}
.search-icon { color: var(--ui-muted); }
.search-row input {
  flex: 1;
  border: none;
  outline: none;
  font-size: 12px;
  background: transparent;
  color: var(--ui-text);
}
.body { overflow-y: auto; padding: 0 12px 12px; }
.cat {
  font-size: 10px;
  font-weight: 600;
  color: var(--ui-muted);
  padding: 8px 0 4px;
}
.grid {
  display: grid;
  grid-template-columns: repeat(6, 1fr);
  gap: 6px;
}
.cell {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 3px;
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  background: var(--card-bg);
  padding: 6px 2px 4px;
  font-size: 10px;
  color: var(--ui-text);
  cursor: pointer;
}
.cell:hover { border-color: var(--acad-blue); background: var(--sel-blue); }
.cell.selected { border-color: var(--acad-blue); background: var(--sel-blue); }
.parts-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.part-row {
  display: flex;
  align-items: center;
  gap: 8px;
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  background: var(--card-bg);
  padding: 5px 8px;
  cursor: pointer;
  text-align: left;
}
.part-row:hover { border-color: var(--acad-blue); background: var(--sel-blue); }
.part-no {
  font-family: var(--mono-font);
  font-size: 11px;
  font-weight: 600;
  color: var(--ui-text);
}
.part-name { font-size: 11px; color: var(--ui-text); }
.part-maker { font-size: 10px; color: var(--ui-muted); }
.part-spacer { flex: 1; }
.part-rating {
  font-family: var(--mono-font);
  font-size: 10px;
  color: var(--ui-muted);
  background: var(--hover-bg);
  border-radius: 4px;
  padding: 1px 7px;
}
.parts-hint {
  font-size: 10px;
  color: var(--ui-muted);
  padding: 2px 0;
}

/* --- マクロギャラリー --- */
.macro-body {
  display: flex;
  gap: 10px;
  padding: 0 12px 12px;
  min-height: 0;
  overflow: hidden;
}
.tree {
  flex: none;
  width: 140px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  padding: 4px;
  overflow-y: auto;
}
.tree-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 6px;
  border: none;
  border-radius: 4px;
  background: transparent;
  padding: 5px 8px;
  font-size: 11px;
  color: var(--ui-text);
  cursor: pointer;
  text-align: left;
}
.tree-row:hover { background: var(--hover-bg); }
.tree-row.active { background: var(--sel-blue); font-weight: 600; }
.tree-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.tree-count { flex: none; font-size: 10px; color: var(--ui-muted); }
.macro-grid {
  flex: 1;
  min-width: 0;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(184px, 1fr));
  grid-auto-rows: min-content;
  gap: 8px;
  overflow-y: auto;
  align-content: start;
}
.macro-tile {
  display: flex;
  flex-direction: column;
  gap: 4px;
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  background: var(--card-bg);
  padding: 6px;
  cursor: pointer;
  text-align: left;
}
.macro-tile:hover { border-color: var(--acad-blue); }
.macro-tile.selected {
  border: 1.5px solid var(--acad-blue);
  background: var(--sel-blue);
  padding: 5.5px;
}
.thumb-frame {
  position: relative;
  display: block;
  border: 1px solid var(--ribbon-line);
  border-radius: 3px;
  overflow: hidden;
  background: var(--card-bg);
}
.badge {
  position: absolute;
  top: 4px;
  right: 4px;
  background: var(--acad-blue);
  color: var(--card-bg);
  border-radius: 4px;
  padding: 1px 6px;
  font-size: 10px;
  font-weight: 600;
}
.macro-tile-name { font-size: 10px; color: var(--ui-text); }
.macro-side {
  flex: none;
  width: 300px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  overflow-y: auto;
}
.side-title { margin: 0; font-size: 12px; font-weight: 600; color: var(--ui-text); }
.variant-row { display: flex; align-items: center; flex-wrap: wrap; gap: 4px; }
.chip {
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  background: var(--card-bg);
  padding: 3px 9px;
  font-size: 11px;
  font-weight: 600;
  color: var(--ui-muted);
  cursor: pointer;
}
.chip:hover { background: var(--hover-bg); }
.chip.current {
  background: var(--sel-blue);
  border-color: var(--acad-blue);
  color: var(--ui-text);
}
.side-preview {
  height: 180px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  overflow: hidden;
}
.side-desc { margin: 0; font-size: 11px; line-height: 16px; color: var(--ui-text); }
.row { display: flex; align-items: center; gap: 10px; }
.label { flex: none; font-size: 11px; color: var(--ui-text); }
.select {
  flex: 1;
  min-width: 0;
  background: var(--input-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  padding: 4px 8px;
  font-size: 11px;
  color: var(--ui-muted);
}
.select:disabled { opacity: 0.6; }
.caption { margin: 0; font-size: 10px; line-height: 15px; color: var(--ui-muted); }
.issue { margin: 0; font-size: 10px; line-height: 15px; color: var(--warn-fg); }
.error { margin: 0; font-size: 11px; color: var(--err-fg); }
.side-spacer { flex: 1; }

.pins-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: var(--palette-bg);
  border-top: 1px solid var(--ribbon-line);
}
.pins-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--ui-text);
}
.stepper {
  display: flex;
  align-items: center;
  background: var(--input-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 8px;
  overflow: hidden;
}
.step {
  width: 26px;
  height: 28px;
  border: none;
  background: transparent;
  color: var(--ui-muted);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}
.step:hover { background: var(--hover-bg); }
.step-value {
  width: 40px;
  height: 28px;
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
.pins-hint {
  font-size: 10px;
  color: var(--ui-muted);
}
.spacer { flex: 1; }
.place-btn {
  border: none;
  background: var(--acad-blue);
  color: var(--card-bg);
  font-size: 12px;
  font-weight: 600;
  padding: 5px 14px;
  border-radius: 4px;
  cursor: pointer;
}
.place-btn:disabled { opacity: 0.5; cursor: default; }
</style>
