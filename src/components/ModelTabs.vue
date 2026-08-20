<script setup lang="ts">
// モデル/レイアウトタブ行 (Pencilデザイン準拠)。レイアウトはフェーズ1後半で実装予定。
import { Plus } from "lucide-vue-next";
import { ref } from "vue";
import { useUiStore } from "../stores/ui";

const ui = useUiStore();
const active = ref("モデル");
const tabs = ["モデル", "レイアウト1", "レイアウト2"];

function select(tab: string) {
  if (tab !== "モデル") {
    ui.log("レイアウト表示: 未実装 (フェーズ1後半で対応予定)");
    return;
  }
  active.value = tab;
}
</script>

<template>
  <div class="model-tabs">
    <button
      v-for="t in tabs"
      :key="t"
      class="tab"
      :class="{ active: t === active }"
      @click="select(t)"
    >
      {{ t }}
    </button>
    <button class="add" @click="ui.log('レイアウト追加: 未実装')"><Plus :size="11" /></button>
  </div>
</template>

<style scoped>
.model-tabs {
  display: flex;
  align-items: center;
  gap: 2px;
  height: 22px;
  padding: 0 8px;
  background: var(--ribbon-strip);
  border-top: 1px solid var(--ribbon-line);
  user-select: none;
  flex: none;
}
.tab {
  border: none;
  background: transparent;
  font-size: 10px;
  color: var(--ui-muted);
  padding: 2px 10px;
  cursor: pointer;
  border-bottom: 2px solid transparent;
}
.tab.active {
  background: #fff;
  color: var(--acad-blue);
  border-bottom-color: var(--acad-blue);
}
.add {
  border: none;
  background: transparent;
  color: var(--ui-muted);
  display: flex;
  cursor: pointer;
}
</style>
