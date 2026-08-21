<script setup lang="ts">
// 部品挿入ダイアログ (AutoCAD Electricalの部品選択ダイアログ相当)。
// デザイン: MadakeCAD.pen「部品挿入ダイアログ」(極数バー含む)
import { Minus, Plus, Search, X } from "lucide-vue-next";
import { computed, inject, ref, watch } from "vue";
import { DYNAMIC_PIN_MAX, dynamicSymbol } from "../canvas/dynamicSymbol";
import type { Part } from "../ipc";
import type { EditorController } from "../tools/controller";
import { useDocumentStore } from "../stores/document";
import { usePartsStore } from "../stores/parts";
import { useUiStore } from "../stores/ui";
import SymbolPreview from "./SymbolPreview.vue";

const store = useDocumentStore();
const ui = useUiStore();
const parts = usePartsStore();
const controller = inject<EditorController>("controller")!;
const query = ref("");

const categoryNames: Record<string, string> = {
  power: "電源",
  passive: "受動部品",
  protection: "保護",
  switch: "スイッチ",
  relay: "リレー",
  output: "出力",
  semiconductor: "半導体",
  connector: "コネクタ",
};

/** ピン数可変部品 (選択すると極数バーが出る)。 */
const dynamicBases = [
  { base: "terminal_block", label: "端子台(N極)" },
  { base: "connector", label: "コネクタ(N極)" },
] as const;
type DynBase = (typeof dynamicBases)[number]["base"];

const selectedDyn = ref<DynBase | null>(null);
const pinCount = ref(8);

const dynamicCells = computed(() => {
  const q = query.value.trim().toLowerCase();
  return dynamicBases.filter(
    (d) => !q || d.label.toLowerCase().includes(q) || d.base.includes(q),
  );
});

const categories = computed(() => {
  const q = query.value.trim().toLowerCase();
  const map = new Map<string, typeof store.symbols>();
  for (const s of store.symbols) {
    if (q && !s.name_ja.toLowerCase().includes(q) && !s.name.toLowerCase().includes(q) && !s.id.includes(q)) {
      continue;
    }
    const arr = map.get(s.category) ?? [];
    arr.push(s);
    map.set(s.category, arr);
  }
  // 動的セル(端子台/コネクタ)が検索に一致する場合はコネクタカテゴリを必ず出す
  if (dynamicCells.value.length > 0 && !map.has("connector")) map.set("connector", []);
  return [...map.entries()];
});

/** 選択中の可変部品の現在極数でのプレビュー定義。 */
const dynPreview = (base: DynBase) =>
  dynamicSymbol(`${base}_${base === selectedDyn.value ? pinCount.value : 3}p`)!;

watch(
  () => ui.symbolPickerOpen,
  (open) => {
    if (open) {
      selectedDyn.value = null;
      void parts.search(query.value);
    }
  },
);

// 検索語の変更を部品DB検索にも反映 (デバウンス)
let partsTimer: ReturnType<typeof setTimeout> | undefined;
watch(query, (q) => {
  clearTimeout(partsTimer);
  partsTimer = setTimeout(() => void parts.search(q), 200);
});

function clampPins(n: number) {
  pinCount.value = Math.min(DYNAMIC_PIN_MAX, Math.max(1, Math.round(n) || 1));
}

function pick(symbolId: string) {
  controller.setTool("place", symbolId);
  ui.symbolPickerOpen = false;
  ui.log(`INSERT  シンボル [${symbolId}] を配置: 位置をクリック (Rで回転, Escで解除)`);
}

function placeDynamic() {
  if (!selectedDyn.value) return;
  pick(`${selectedDyn.value}_${pinCount.value}p`);
}

/** 部品DBの部品を配置: 既定シンボルを型番(value)・定格(attrs.current_a)付きで置く。 */
function pickPart(part: Part) {
  if (!part.symbol_id || !store.resolveSymbol(part.symbol_id)) {
    ui.log(`部品 ${part.part_no}: 既定シンボルが未設定または不明です (symbol_id: ${part.symbol_id || "空"})`);
    return;
  }
  const attrs: Record<string, string> = {};
  if (part.rated_current_a != null) attrs.current_a = String(part.rated_current_a);
  controller.setTool("place", part.symbol_id, { value: part.part_no, attrs });
  ui.symbolPickerOpen = false;
  ui.log(`INSERT  部品 [${part.part_no}] を配置: 位置をクリック (Rで回転, Escで解除)`);
}
</script>

<template>
  <div v-if="ui.symbolPickerOpen" class="overlay" @click.self="ui.symbolPickerOpen = false">
    <div class="dialog">
      <div class="dialog-head">
        <span>挿入する部品を選択</span>
        <button class="close" @click="ui.symbolPickerOpen = false"><X :size="14" /></button>
      </div>
      <div class="search-row">
        <Search :size="13" class="search-icon" />
        <input v-model="query" placeholder="検索 (名称・記号)" autofocus />
      </div>
      <div class="body">
        <template v-for="[cat, syms] in categories" :key="cat">
          <div class="cat">{{ categoryNames[cat] ?? cat }}</div>
          <div class="grid">
            <button v-for="sym in syms" :key="sym.id" class="cell" @click="pick(sym.id)">
              <SymbolPreview :def="sym" :width="72" :height="40" />
              <span>{{ sym.name_ja }}</span>
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
          <div class="cat">部品DB (グローバルマスタ)</div>
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
            <div class="parts-hint">選択して配置すると型番・定格が図面に設定されます (登録は madake parts / AIチャットから)</div>
          </div>
        </template>
      </div>
      <div v-if="selectedDyn" class="pins-bar">
        <span class="pins-label">極数</span>
        <div class="stepper">
          <button class="step" @click="clampPins(pinCount - 1)"><Minus :size="12" /></button>
          <input
            class="step-value"
            :value="pinCount"
            @change="clampPins(Number(($event.target as HTMLInputElement).value))"
          />
          <button class="step" @click="clampPins(pinCount + 1)"><Plus :size="12" /></button>
        </div>
        <span class="pins-hint">1〜{{ DYNAMIC_PIN_MAX }} (5mmピッチ・中央揃え)</span>
        <span class="spacer" />
        <button class="place-btn" @click="placeDynamic">配置</button>
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
.search-row {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 8px 12px;
  padding: 5px 8px;
  background: #fff;
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
  background: #fff;
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
  background: #fff;
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
  background: #fff;
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
  color: #fff;
  font-size: 12px;
  font-weight: 600;
  padding: 5px 14px;
  border-radius: 4px;
  cursor: pointer;
}
</style>
