<script setup lang="ts">
// 検索結果パネル (デザイン: .pen「M4デザイン - 検索/デバイスナビゲータ/Surfer」)。
// 作図領域下部にドッキングする検証結果パネルと同型で、行クリック=シート切替+選択+ズーム。
import { X } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { hitLabel, hitLocation, searchJumpTarget } from "../canvas/search";
import { useReveal } from "../composables/reveal";
import type { SearchHit } from "../ipc";
import { useSearchStore } from "../stores/search";
import { useUiStore } from "../stores/ui";

const { t } = useI18n();
const search = useSearchStore();
const ui = useUiStore();
const reveal = useReveal();

function label(hit: SearchHit): string {
  const spec = hitLabel(hit);
  return t(spec.key, spec.params ?? {});
}

function pick(index: number) {
  const hit = search.select(index);
  if (!hit) return;
  reveal(searchJumpTarget(hit));
  ui.log(
    t("search.revealLog", {
      text: hit.text,
      address: `/${hit.sheet_no}.${hit.zone}`,
      index: index + 1,
      count: search.count,
    }),
  );
}
</script>

<template>
  <div v-if="search.panelOpen" class="panel">
    <div class="head">
      <span class="title">{{ t("search.panelTitle", { query: search.query.trim() }) }}</span>
      <span class="count">{{ t("search.count", { count: search.count }) }}</span>
      <span class="spacer" />
      <button class="close" :title="t('search.close')" @click="search.closePanel()">
        <X :size="12" />
      </button>
    </div>
    <div class="rows">
      <button
        v-for="(hit, i) in search.hits"
        :key="`${hit.entity_id}-${hit.kind}-${i}`"
        class="row"
        :class="{ on: i === search.activeIndex }"
        @click="pick(i)"
      >
        <span class="ref">{{ hit.text }}</span>
        <span class="kind">{{ label(hit) }}</span>
        <span class="loc">{{ hitLocation(hit) }}</span>
        <span class="extra">{{ hit.detail }}</span>
      </button>
      <div v-if="search.hits.length === 0" class="empty">
        {{ t("search.empty", { query: search.query.trim() }) }}
      </div>
    </div>
  </div>
</template>

<style scoped>
.panel {
  flex: none;
  max-height: 180px;
  display: flex;
  flex-direction: column;
  background: var(--palette-bg);
  border-top: 1px solid var(--ribbon-line);
}
.head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 10px;
  background: var(--palette-head);
}
.title {
  font-size: 11px;
  font-weight: 600;
  color: var(--ui-text);
}
.count {
  font-size: 10px;
  color: var(--ui-muted);
}
.spacer {
  flex: 1;
}
.close {
  border: none;
  background: transparent;
  color: var(--ui-muted);
  display: flex;
  cursor: pointer;
}
.rows {
  overflow-y: auto;
}
.row {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  border: none;
  border-bottom: 1px solid var(--ribbon-line);
  background: transparent;
  text-align: left;
  padding: 4px 10px;
  cursor: pointer;
}
.row:hover {
  background: var(--hover-bg);
}
.row.on {
  background: var(--sel-blue);
}
/* 列幅はデザイン準拠 (参照60 / 種別170 / 所在110 / 補足=残り) */
.ref {
  flex: none;
  width: 60px;
  font-family: var(--mono-font);
  font-size: 10px;
  font-weight: 600;
  color: var(--ui-text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.kind {
  flex: none;
  width: 170px;
  font-size: 10px;
  color: var(--ui-text);
}
.loc {
  flex: none;
  width: 110px;
  font-family: var(--mono-font);
  font-size: 10px;
  color: var(--ui-muted);
}
.extra {
  flex: 1;
  min-width: 0;
  font-size: 10px;
  color: var(--ui-placeholder);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.empty {
  padding: 8px 10px;
  font-size: 10px;
  color: var(--ui-muted);
}
</style>
