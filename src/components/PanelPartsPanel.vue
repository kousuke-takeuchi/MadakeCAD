<script setup lang="ts">
// 未配置部品パネル (左ドックの「未配置部品」タブ。.pen「M4デザイン - 盤レイアウト」左列)。
// 回路図にあって盤へ置かれていない部品を並べ、行をクリックすると配置モードに入る
// (盤面をクリックで確定・Rで向き・DINレールへ吸着)。図面を変えるのはCommandだけ。
import { AlertTriangle } from "lucide-vue-next";
import { inject, onMounted, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { UnplacedRow } from "../canvas/panelTools";
import { useDocumentStore } from "../stores/document";
import { usePanelStore } from "../stores/panel";
import { useUiStore } from "../stores/ui";
import type { EditorController } from "../tools/controller";

const { t } = useI18n();
const panel = usePanelStore();
const store = useDocumentStore();
const ui = useUiStore();
const controller = inject<EditorController>("controller")!;

/** 行クリック: その部品の配置モードに入る (盤面をクリックで確定)。 */
function place(row: UnplacedRow) {
  const component = panel.find(row.reference);
  if (!component) return;
  panel.select(row.reference);
  controller.startFootprintPlacement(component);
  if (controller.tool === "footprint") {
    ui.log(t("panel.placeLog", { reference: row.reference }));
  }
}

onMounted(() => void panel.load());
// 図面が変われば読み直す (置く・消す・undoのたびに未配置が変わる)
watch(
  () => store.revision,
  () => void panel.load(),
);
</script>

<template>
  <div class="panel">
    <div class="panel-head">
      <span>{{ t("panel.head") }}</span>
      <span class="count">{{ t("panel.count", { count: panel.count }) }}</span>
    </div>
    <div class="rows">
      <button
        v-for="row in panel.rows"
        :key="row.reference"
        class="row"
        :class="{ on: panel.selected === row.reference }"
        :title="t(`panel.statusHint.${row.status}`)"
        @click="place(row)"
      >
        <span class="ref">{{ row.reference }}</span>
        <span class="part">{{ row.partNo || "—" }}</span>
        <span class="spacer" />
        <span class="size">{{ row.sizeLabel }}</span>
        <span class="status" :class="row.status">
          <AlertTriangle v-if="row.status === 'unregistered'" :size="10" />
          {{ t(`panel.status.${row.status}`) }}
        </span>
      </button>
      <div v-if="panel.count === 0 && !panel.error" class="empty">{{ t("panel.empty") }}</div>
      <div v-if="panel.error" class="error">{{ t("panel.errorHint", { error: panel.error }) }}</div>
    </div>
    <div class="spacer" />
    <div class="hint">{{ t("panel.hint") }}</div>
  </div>
</template>

<style scoped>
.panel {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background: var(--palette-bg);
  overflow: hidden;
}
.panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex: none;
  padding: 5px 9px;
  background: var(--palette-head);
  font-size: 11px;
  font-weight: 600;
  color: var(--ui-text);
}
.count { font-size: 10px; font-weight: 400; color: var(--ui-muted); }
.rows { overflow-y: auto; }
.row {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  border: none;
  border-bottom: 1px solid var(--ribbon-line);
  background: transparent;
  text-align: left;
  padding: 5px 9px;
  cursor: pointer;
}
.row:hover { background: var(--hover-bg); }
.row.on { background: var(--sel-blue); }
.ref {
  font: 600 11px var(--mono-font);
  color: var(--ui-text);
  min-width: 34px;
}
.part { font-size: 10px; color: var(--ui-muted); }
.spacer { flex: 1; }
.size { font: 10px var(--mono-font); color: var(--ui-placeholder); }
.status {
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 1px 6px;
  border-radius: 999px;
  font-size: 10px;
  background: var(--off-bg);
  color: var(--off-fg);
  white-space: nowrap;
}
.status.sized { background: var(--ok-bg); color: var(--ok-fg); }
.status.unregistered { background: var(--warn-bg); color: var(--warn-fg); }
.empty,
.error {
  padding: 10px 9px;
  font-size: 10px;
  color: var(--ui-muted);
}
.error { color: var(--err-fg); }
.hint {
  flex: none;
  padding: 6px 9px;
  border-top: 1px solid var(--ribbon-line);
  font-size: 10px;
  color: var(--ui-muted);
}
</style>
