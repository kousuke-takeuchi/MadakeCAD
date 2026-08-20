<script setup lang="ts">
import { computed, inject } from "vue";
import type { EditorController } from "../tools/controller";
import { useDocumentStore } from "../stores/document";

const store = useDocumentStore();
const controller = inject<EditorController>("controller")!;

const categories = computed(() => {
  const map = new Map<string, typeof store.symbols>();
  for (const s of store.symbols) {
    const arr = map.get(s.category) ?? [];
    arr.push(s);
    map.set(s.category, arr);
  }
  return [...map.entries()];
});

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

async function addSheet() {
  await store.execute({
    type: "add_sheet",
    name: `Sheet${(store.project?.sheets.length ?? 0) + 1}`,
    size: "A3",
    orientation: "Landscape",
  });
  const sheets = store.project?.sheets ?? [];
  store.activeSheetId = sheets[sheets.length - 1]?.id ?? null;
}
</script>

<template>
  <aside class="panel">
    <div class="panel-head">プロジェクト マネージャー</div>
    <div class="section-head">
      <span>シート</span>
      <button class="mini-btn" title="シートを追加" @click="addSheet">+</button>
    </div>
    <div class="sheet-list">
      <button
        v-for="s in store.project?.sheets ?? []"
        :key="s.id"
        class="row"
        :class="{ active: s.id === store.activeSheet?.id }"
        @click="store.activeSheetId = s.id"
      >
        <span class="row-icon">▤</span>{{ s.name }}
      </button>
    </div>
    <div class="section-head"><span>シンボル</span></div>
    <div class="symbol-list">
      <template v-for="[cat, syms] in categories" :key="cat">
        <div class="cat">{{ categoryNames[cat] ?? cat }}</div>
        <button
          v-for="sym in syms"
          :key="sym.id"
          class="row indent"
          :class="{ active: controller.tool === 'place' && controller.placeSymbolId === sym.id }"
          @click="controller.setTool('place', sym.id)"
        >
          <span class="row-icon">⌁</span>{{ sym.name_ja }}
        </button>
      </template>
    </div>
  </aside>
</template>

<style scoped>
.panel {
  width: 224px;
  flex: none;
  background: var(--palette-bg);
  border-right: 1px solid var(--ribbon-line);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  user-select: none;
}
.panel-head {
  padding: 6px 10px;
  background: var(--palette-head);
  font-size: 11px;
  font-weight: 600;
  color: var(--ui-text);
}
.section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 5px 10px;
  font-size: 11px;
  font-weight: 600;
  color: var(--ui-text);
  border-bottom: 1px solid var(--ribbon-line);
}
.mini-btn {
  border: 1px solid var(--ribbon-line);
  background: #fff;
  width: 18px;
  height: 18px;
  border-radius: 3px;
  cursor: pointer;
  line-height: 1;
}
.sheet-list { max-height: 30%; overflow-y: auto; }
.symbol-list { flex: 1; overflow-y: auto; padding-bottom: 8px; }
.cat {
  padding: 5px 10px 2px;
  font-size: 10px;
  font-weight: 600;
  color: var(--ui-muted);
}
.row {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  border: none;
  background: transparent;
  text-align: left;
  padding: 3px 10px;
  font-size: 11px;
  color: var(--ui-text);
  cursor: pointer;
}
.row.indent { padding-left: 22px; }
.row:hover { background: var(--hover-bg); }
.row.active { background: var(--sel-blue); }
.row-icon { color: var(--acad-blue); font-size: 10px; }
</style>
