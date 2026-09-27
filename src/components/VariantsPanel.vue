<script setup lang="ts">
// 整え案の比較パネル (M3フェーズ4)。作図領域下部にドッキングする既存のドックパネル様式
// (検証結果・検索結果と同じ)。案ごとに実行状態と整え指標を並べ、「表示」で図面タブを
// 切り替え、「この案を採用」で元シートへ写し戻す。案の会話が終わるたびに指標を取り直す。
import { X } from "lucide-vue-next";
import { watch } from "vue";
import { useI18n } from "vue-i18n";
import type { TidyMetrics } from "../ipc";
import { useChatStore } from "../stores/chat";
import { useDocumentStore } from "../stores/document";
import { metricsTotal, useVariantsStore, type VariantEntry } from "../stores/variants";

const { t } = useI18n();
const variants = useVariantsStore();
const chat = useChatStore();
const doc = useDocumentStore();

// 案の会話が終わったら (実行中の数が減ったら) 指標を取り直す
watch(
  () => variants.runningCount,
  (now, before) => {
    if (variants.run && now < before) void variants.refreshMetrics();
  },
);

function metricsText(m: TidyMetrics | null): string {
  if (!m) return t("variants.noMetrics");
  return t("variants.metrics", {
    crossings: m.crossings,
    overlaps: m.label_overlaps + m.symbol_overlaps,
    off_grid: m.off_grid,
  });
}

function status(v: VariantEntry): { key: string; cls: string } {
  if (!v.conversationId) return { key: "failed", cls: "failed" };
  if (variants.isRunning(v)) return { key: "running", cls: "running" };
  return { key: "done", cls: "done" };
}

/** 案の合計が元より良い(小さい)ときは強調する。 */
function isBetter(v: VariantEntry): boolean {
  const base = variants.run?.baseline;
  return !!v.metrics && !!base && metricsTotal(v.metrics) < metricsTotal(base);
}
</script>

<template>
  <div v-if="variants.panelOpen && variants.run" class="panel">
    <div class="head">
      <span class="title">{{ t("variants.title") }}</span>
      <span class="mode">{{ t(`chat.tidy.mode.${variants.run.mode}`) }}</span>
      <span v-if="variants.run.baseline" class="baseline">
        {{ t("variants.original") }}: {{ metricsText(variants.run.baseline) }}
        ({{ t("variants.total", { total: metricsTotal(variants.run.baseline) }) }})
      </span>
      <span class="spacer" />
      <span v-if="!variants.allDone" class="waiting">{{ t("variants.waiting") }}</span>
      <button class="link" :disabled="variants.finishing" @click="variants.discard()">
        {{ t("variants.discard") }}
      </button>
      <button class="close" @click="variants.close()"><X :size="12" /></button>
    </div>
    <div class="rows">
      <div
        v-for="v in variants.run.variants"
        :key="v.sheet_id"
        class="row"
        :class="{ active: doc.activeSheetId === v.sheet_id }"
      >
        <span class="dot" :style="{ background: v.conversationId ? chat.conversationColors[v.conversationId] : 'var(--off-fg)' }" />
        <span class="label">{{ v.label }}</span>
        <span class="status" :class="status(v).cls">{{ t(`variants.${status(v).key}`) }}</span>
        <span class="metrics" :class="{ better: isBetter(v) }">
          {{ metricsText(v.metrics) }}
          <template v-if="v.metrics"> ({{ t("variants.total", { total: metricsTotal(v.metrics) }) }})</template>
        </span>
        <span class="spacer" />
        <button class="link" @click="variants.show(v.sheet_id)">{{ t("variants.show") }}</button>
        <button
          class="adopt"
          :disabled="!variants.allDone || variants.finishing"
          @click="variants.adopt(v.sheet_id)"
        >
          {{ t("variants.adopt") }}
        </button>
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
  gap: 10px;
  padding: 6px 10px;
  background: var(--palette-head);
}
.title {
  font-size: 11px;
  font-weight: 600;
  color: var(--ui-text);
}
.mode {
  font-size: 10px;
  font-weight: 600;
  border-radius: 999px;
  padding: 1px 8px;
  color: var(--info-fg);
  background: var(--info-bg);
}
.baseline {
  font-size: 10px;
  color: var(--ui-muted);
}
.waiting {
  font-size: 10px;
  color: var(--ui-placeholder);
}
.spacer { flex: 1; }
.link {
  border: none;
  background: transparent;
  color: var(--acad-blue);
  font: inherit;
  font-size: 10px;
  font-weight: 600;
  cursor: pointer;
}
.link:disabled { color: var(--off-fg); cursor: default; }
.close {
  border: none;
  background: transparent;
  color: var(--ui-muted);
  display: flex;
  cursor: pointer;
}
.rows { overflow-y: auto; padding: 4px 0; }
.row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 10px;
  border-bottom: 1px solid var(--ribbon-line);
}
.row.active { background: var(--sel-blue); }
.dot { width: 8px; height: 8px; border-radius: 999px; flex: none; }
.label { font-size: 11px; font-weight: 600; color: var(--ui-text); width: 40px; }
.status { font-size: 10px; font-weight: 600; border-radius: 999px; padding: 1px 8px; }
.status.running { color: var(--warn-fg); background: var(--warn-bg); }
.status.done { color: var(--ok-fg); background: var(--ok-bg); }
.status.failed { color: var(--err-fg); background: var(--err-bg); }
.metrics { font-family: var(--mono-font); font-size: 10px; color: var(--ui-muted); }
.metrics.better { color: var(--ok-fg); font-weight: 600; }
.adopt {
  border: 1px solid var(--acad-blue);
  border-radius: 4px;
  background: var(--card-bg);
  color: var(--acad-blue);
  font: inherit;
  font-size: 10px;
  font-weight: 600;
  padding: 2px 8px;
  cursor: pointer;
}
.adopt:hover:enabled { background: var(--sel-blue); }
.adopt:disabled { color: var(--off-fg); border-color: var(--ribbon-line); cursor: default; }
</style>
