<script setup lang="ts">
import { computed, reactive, watch } from "vue";
import type { Entity } from "../ipc";
import { useDocumentStore } from "../stores/document";

const store = useDocumentStore();

const selected = computed<Entity | null>(() => {
  const sheet = store.activeSheet;
  if (!sheet || store.selection.size !== 1) return null;
  const [id] = [...store.selection];
  return sheet.entities[id] ?? null;
});

const wireColors = [
  ["red", "赤"], ["black", "黒"], ["white", "白"], ["blue", "青"],
  ["yellow", "黄"], ["green", "緑"], ["orange", "橙"], ["purple", "紫"],
  ["brown", "茶"], ["gray", "灰"], ["pink", "桃"], ["light_blue", "水色"],
] as const;
const sqValues = [0.2, 0.3, 0.5, 0.75, 1.25, 2.0, 3.5, 5.5];

// 編集バッファ (適用ボタンでCommand発行)
const buf = reactive({
  color: "red",
  sq: 0.75,
  part_no: "",
  length_m: "",
  reference: "",
  value: "",
  name: "",
});

watch(
  selected,
  (e) => {
    if (!e) return;
    if (e.kind === "wire") {
      buf.color = e.color;
      buf.sq = e.sq;
      buf.part_no = e.part_no ?? "";
      buf.length_m = e.length_m?.toString() ?? "";
    } else if (e.kind === "symbol") {
      buf.reference = e.reference;
      buf.value = e.value;
    } else if (e.kind === "net_label") {
      buf.name = e.name;
    }
  },
  { immediate: true },
);

async function apply() {
  const sheet = store.activeSheet;
  const e = selected.value;
  if (!sheet || !e) return;
  let entity: Entity;
  if (e.kind === "wire") {
    entity = {
      ...e,
      color: buf.color,
      sq: buf.sq,
      part_no: buf.part_no || null,
      length_m: buf.length_m === "" ? null : Number(buf.length_m),
    };
  } else if (e.kind === "symbol") {
    entity = { ...e, reference: buf.reference, value: buf.value };
  } else if (e.kind === "net_label") {
    entity = { ...e, name: buf.name };
  } else {
    return;
  }
  await store.execute({ type: "update_entity", sheet_id: sheet.id, entity });
}

const kindLabel: Record<string, string> = {
  wire: "配線",
  symbol: "シンボル",
  junction: "ジャンクション",
  net_label: "ネットラベル",
  text: "テキスト",
};
</script>

<template>
  <aside class="panel">
    <div class="panel-head">プロパティ</div>
    <div class="type-row">
      <span v-if="selected">{{ kindLabel[selected.kind] }} (1)</span>
      <span v-else-if="store.selection.size > 1">{{ store.selection.size }} 個選択</span>
      <span v-else class="muted">選択なし</span>
    </div>

    <template v-if="selected?.kind === 'wire'">
      <div class="sec">電気属性</div>
      <label class="field">
        <span>線色</span>
        <select v-model="buf.color">
          <option v-for="[v, label] in wireColors" :key="v" :value="v">{{ label }} ({{ v }})</option>
        </select>
      </label>
      <label class="field">
        <span>線径 sq</span>
        <select v-model.number="buf.sq">
          <option v-for="v in sqValues" :key="v" :value="v">{{ v }} sq</option>
        </select>
      </label>
      <label class="field">
        <span>電線品番</span>
        <input v-model="buf.part_no" placeholder="例: SAMPLE0001" />
      </label>
      <label class="field">
        <span>長さ m</span>
        <input v-model="buf.length_m" placeholder="例: 0.4" />
      </label>
    </template>

    <template v-else-if="selected?.kind === 'symbol'">
      <div class="sec">識別</div>
      <label class="field">
        <span>参照記号</span>
        <input v-model="buf.reference" />
      </label>
      <label class="field">
        <span>型番/値</span>
        <input v-model="buf.value" placeholder="例: JZX-22F" />
      </label>
    </template>

    <template v-else-if="selected?.kind === 'net_label'">
      <div class="sec">ネット</div>
      <label class="field">
        <span>ネット名</span>
        <input v-model="buf.name" />
      </label>
    </template>

    <div v-if="selected" class="apply-row">
      <button class="apply" @click="apply">適用</button>
    </div>
  </aside>
</template>

<style scoped>
.panel {
  width: 232px;
  flex: none;
  background: var(--palette-bg);
  border-left: 1px solid var(--ribbon-line);
  display: flex;
  flex-direction: column;
  overflow-y: auto;
  user-select: none;
}
.panel-head {
  padding: 6px 10px;
  background: var(--palette-head);
  font-size: 11px;
  font-weight: 600;
  color: var(--ui-text);
}
.type-row {
  padding: 6px 10px;
  font-size: 11px;
  color: var(--ui-text);
  border-bottom: 1px solid var(--ribbon-line);
}
.muted { color: var(--ui-muted); }
.sec {
  padding: 5px 10px 3px;
  font-size: 10px;
  font-weight: 600;
  color: var(--ui-text);
  background: #e7eaed;
}
.field {
  display: grid;
  grid-template-columns: 72px 1fr;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  font-size: 10px;
  color: var(--ui-muted);
}
.field input,
.field select {
  width: 100%;
  font-size: 11px;
  padding: 3px 6px;
  border: 1px solid var(--ribbon-line);
  border-radius: 3px;
  background: #fff;
  color: var(--ui-text);
}
.apply-row {
  display: flex;
  justify-content: flex-end;
  padding: 10px;
}
.apply {
  background: var(--acad-blue);
  color: #fff;
  border: none;
  border-radius: 4px;
  padding: 5px 16px;
  font-size: 12px;
  cursor: pointer;
}
</style>
