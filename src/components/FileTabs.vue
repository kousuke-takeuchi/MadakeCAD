<script setup lang="ts">
// 図面ファイルタブ行 (Pencilデザイン準拠)。シートをタブとして表示する。
import { Plus, X } from "lucide-vue-next";
import { useDocumentStore } from "../stores/document";
import { useUiStore } from "../stores/ui";

const store = useDocumentStore();
const ui = useUiStore();

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

async function closeSheet(id: string) {
  if ((store.project?.sheets.length ?? 0) <= 1) {
    ui.log("最後のシートは削除できません");
    return;
  }
  if (!window.confirm("このシートを削除しますか?(undoで戻せます)")) return;
  await store.execute({ type: "remove_sheet", sheet_id: id });
}
</script>

<template>
  <div class="file-tabs">
    <button
      v-for="s in store.project?.sheets ?? []"
      :key="s.id"
      class="tab"
      :class="{ active: s.id === store.activeSheet?.id }"
      @click="store.activeSheetId = s.id"
    >
      {{ s.name }}
      <X
        v-if="s.id === store.activeSheet?.id"
        :size="10"
        class="close"
        @click.stop="closeSheet(s.id)"
      />
    </button>
    <button class="add" title="シートを追加" @click="addSheet"><Plus :size="12" /></button>
  </div>
</template>

<style scoped>
.file-tabs {
  display: flex;
  align-items: flex-end;
  gap: 2px;
  height: 26px;
  padding: 0 6px;
  background: var(--ribbon-strip);
  user-select: none;
  flex: none;
}
.tab {
  display: flex;
  align-items: center;
  gap: 6px;
  border: 1px solid var(--ribbon-line);
  border-bottom: none;
  background: #e9ecef;
  color: var(--ui-muted);
  font-size: 11px;
  padding: 4px 10px;
  border-radius: 4px 4px 0 0;
  cursor: pointer;
}
.tab.active {
  background: var(--model-bg);
  color: #e8eaec;
  border-color: var(--model-bg);
}
.close { color: var(--ui-placeholder); }
.close:hover { color: #fff; }
.add {
  width: 22px;
  height: 22px;
  margin-bottom: 1px;
  border: none;
  background: transparent;
  color: var(--ui-muted);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  border-radius: 4px;
}
.add:hover { background: var(--hover-bg); }
</style>
