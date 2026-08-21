<script setup lang="ts">
// 部品挿入ダイアログ (AutoCAD Electricalの部品選択ダイアログ相当)。
import { Search, X } from "lucide-vue-next";
import { computed, inject, ref } from "vue";
import type { EditorController } from "../tools/controller";
import { useDocumentStore } from "../stores/document";
import { useUiStore } from "../stores/ui";
import SymbolPreview from "./SymbolPreview.vue";

const store = useDocumentStore();
const ui = useUiStore();
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
  return [...map.entries()];
});

function pick(symbolId: string) {
  controller.setTool("place", symbolId);
  ui.symbolPickerOpen = false;
  ui.log(`INSERT  シンボル [${symbolId}] を配置: 位置をクリック (Rで回転, Escで解除)`);
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
          </div>
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
  width: 640px;
  max-height: 70vh;
  background: var(--ribbon-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 6px;
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
</style>
