<script setup lang="ts">
// ⌘F 浮き検索バー (デザイン: .pen「M4デザイン - 検索/デバイスナビゲータ/Surfer」)。
// 作図領域の右上に浮くカード。入力欄+件数+フィルタチップ行で、結果は下部ドックの
// SearchResultsPanel に出る。Enter=次へ / Shift+Enter=前へ / Esc=閉じる。
import { Search } from "lucide-vue-next";
import { nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { SEARCH_FILTERS, searchJumpTarget, type SearchFilter } from "../canvas/search";
import { useReveal } from "../composables/reveal";
import { useSearchStore } from "../stores/search";
import { useUiStore } from "../stores/ui";

const { t } = useI18n();
const search = useSearchStore();
const ui = useUiStore();
const reveal = useReveal();
const inputRef = ref<HTMLInputElement | null>(null);

/** Enter=次へ / Shift+Enter=前へ。巡回位置のヒットを図面に表示する。 */
function step(delta: number) {
  const hit = search.step(delta);
  if (!hit) return;
  reveal(searchJumpTarget(hit));
  ui.log(
    t("search.revealLog", {
      text: hit.text,
      address: `/${hit.sheet_no}.${hit.zone}`,
      index: search.activeIndex + 1,
      count: search.count,
    }),
  );
}

/** 入力のたびに検索し直す (Rust側の純関数なので十分速い)。 */
let timer: ReturnType<typeof setTimeout> | null = null;
function onInput(event: Event) {
  search.setQuery((event.target as HTMLInputElement).value);
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => void search.run(), 120);
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    event.preventDefault();
    event.stopPropagation();
    search.close();
    return;
  }
  if (event.key !== "Enter") return;
  event.preventDefault();
  step(event.shiftKey ? -1 : 1);
}

async function pickFilter(filter: SearchFilter) {
  await search.setFilter(filter);
  inputRef.value?.focus();
}

// ⌘Fで開いたら入力欄へフォーカスし、開き直しでは既存の検索語を選択状態にする
watch(
  () => search.barOpen,
  async (open) => {
    if (!open) return;
    await nextTick();
    inputRef.value?.focus();
    inputRef.value?.select();
  },
  { immediate: true },
);
</script>

<template>
  <div v-if="search.barOpen" class="search-bar">
    <div class="input-row">
      <label class="field">
        <Search :size="13" class="icon" />
        <input
          ref="inputRef"
          type="text"
          :value="search.query"
          :placeholder="t('search.placeholder')"
          @input="onInput"
          @keydown="onKeydown"
        />
      </label>
      <span class="count">{{ t("search.count", { count: search.count }) }}</span>
    </div>
    <div class="filter-row">
      <button
        v-for="f in SEARCH_FILTERS"
        :key="f"
        class="chip"
        :class="{ on: search.filter === f }"
        @click="pickFilter(f)"
      >
        {{ t(`search.filter.${f}`) }}
      </button>
    </div>
    <div class="hint">{{ t("search.hint") }}</div>
  </div>
</template>

<style scoped>
/* 作図領域の右上に浮くカード (デザイン: 幅340・角丸lg・白・枠) */
.search-bar {
  position: absolute;
  top: 12px;
  right: 12px;
  z-index: 6;
  width: 340px;
  display: flex;
  flex-direction: column;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 12px;
  box-shadow: var(--shadow-popup);
  overflow: hidden;
}
.input-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
}
.field {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 8px;
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  background: var(--input-bg);
}
/* focus=acad-blue枠 (デザインシステム §3 の状態規約) */
.field:focus-within {
  border-color: var(--acad-blue);
  border-width: 1.5px;
  padding: 4.5px 7.5px;
}
.icon {
  flex: none;
  color: var(--ui-muted);
}
.field input {
  flex: 1;
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  font-family: var(--mono-font);
  font-size: 12px;
  color: var(--ui-text);
}
.field input::placeholder {
  font-family: inherit;
  color: var(--ui-placeholder);
}
.count {
  flex: none;
  font-size: 10px;
  color: var(--ui-muted);
}
.filter-row {
  display: flex;
  gap: 4px;
  padding: 0 10px 8px;
}
.chip {
  padding: 2px 8px;
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  background: var(--input-bg);
  font-size: 10px;
  color: var(--ui-muted);
  cursor: pointer;
  white-space: nowrap;
}
.chip:hover {
  background: var(--hover-bg);
}
.chip.on {
  background: var(--sel-blue);
  border-color: var(--acad-blue);
  color: var(--ui-text);
  font-weight: 600;
}
.hint {
  padding: 0 10px 8px;
  font-size: 10px;
  color: var(--ui-muted);
}
</style>
