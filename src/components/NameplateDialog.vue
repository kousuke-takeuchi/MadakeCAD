<script setup lang="ts">
// 銘板の挿入ダイアログ (.pen「M4残りデザイン⑤」`B2gfQZ`の「銘板の挿入 — K1」)。
// CAD調モーダル幅300: 1行目・2行目・説明の出典 (読み取り専用)・大きさ・文字高・位置・型番
// +「回路図の説明が変わったら追従」。挿入はadd_entity 1回 (Cmd+Zで戻る)。
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { X } from "lucide-vue-next";
import { useNameplatesStore } from "../stores/nameplates";
import { useUiStore } from "../stores/ui";

const np = useNameplatesStore();
const ui = useUiStore();
const { t } = useI18n();

const title = computed(() =>
  np.reference ? t("panelMarks.nameplate.titleFor", { reference: np.reference }) : t("panelMarks.nameplate.titleAlone"),
);
const source = computed(() => {
  if (!np.footprintId) return t("panelMarks.nameplate.sourceAlone");
  return np.description
    ? t("panelMarks.nameplate.sourceSchematic", { reference: np.reference ?? "" })
    : t("panelMarks.nameplate.sourceNone");
});

async function insert() {
  if (await np.insert()) {
    ui.log(t("panelMarks.nameplate.insertedLog", { reference: np.reference ?? t("panelMarks.nameplate.alone") }));
  } else if (np.error) {
    ui.log(np.error);
  }
}
</script>

<template>
  <div v-if="np.open" class="overlay" @click.self="np.cancel()">
    <div class="dialog">
      <div class="titlebar">
        <span class="title">{{ title }}</span>
        <button class="close" :title="t('panelMarks.cancel')" @click="np.cancel()"><X :size="14" /></button>
      </div>
      <div class="body">
        <label class="row"><span class="label">{{ t("panelMarks.nameplate.line1") }}</span><input v-model="np.line1" class="input" /></label>
        <label class="row"><span class="label">{{ t("panelMarks.nameplate.line2") }}</span><input v-model="np.line2" class="input" /></label>
        <div class="row"><span class="label">{{ t("panelMarks.nameplate.source") }}</span><span class="input readonly">{{ source }}</span></div>
        <div class="row">
          <span class="label">{{ t("panelMarks.nameplate.size") }}</span>
          <input v-model.number="np.w" class="input num mono" type="number" min="1" step="5" />
          <span class="x">×</span>
          <input v-model.number="np.h" class="input num mono" type="number" min="1" step="1" />
          <span class="unit">mm</span>
        </div>
        <div class="row">
          <span class="label">{{ t("panelMarks.nameplate.textHeight") }}</span>
          <input v-model.number="np.textMm" class="input num mono" type="number" min="1" step="0.5" />
          <span class="unit">mm</span>
        </div>
        <label class="row">
          <span class="label">{{ t("panelMarks.nameplate.position") }}</span>
          <select v-model="np.position" class="input" :disabled="!np.footprintId">
            <option value="below">{{ t("panelMarks.nameplate.below") }}</option>
            <option value="above">{{ t("panelMarks.nameplate.above") }}</option>
          </select>
        </label>
        <label class="row"><span class="label">{{ t("panelMarks.nameplate.partNo") }}</span><input v-model="np.partNo" class="input mono" /></label>
        <label v-if="np.footprintId" class="check">
          <input v-model="np.follow" type="checkbox" />
          {{ t("panelMarks.nameplate.follow") }}
        </label>
        <p class="hint">{{ t("panelMarks.nameplate.hint") }}</p>
      </div>
      <div class="footer">
        <button class="btn primary" :disabled="!np.canInsert" @click="insert">{{ t("panelMarks.nameplate.insert") }}</button>
        <button class="btn secondary" @click="np.cancel()">{{ t("panelMarks.cancel") }}</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.overlay { position: fixed; inset: 0; background: var(--scrim); display: flex; align-items: center; justify-content: center; z-index: 100; }
.dialog { width: min(320px, 94vw); background: var(--ribbon-bg); border: 1px solid var(--ribbon-line); border-radius: 8px; box-shadow: var(--shadow-panel-lg); display: flex; flex-direction: column; overflow: hidden; }
.titlebar { display: flex; align-items: center; justify-content: space-between; padding: 8px 12px; background: var(--palette-head); }
.title { font-size: 12px; font-weight: 600; color: var(--ui-text); }
.close { border: none; background: transparent; color: var(--ui-muted); display: flex; cursor: pointer; padding: 0; }
.body { display: flex; flex-direction: column; gap: 8px; padding: 12px; }
.row { display: flex; align-items: center; gap: 8px; }
.label { flex: none; width: 70px; font-size: 11px; color: var(--ui-text); }
.input { flex: 1; min-width: 0; background: var(--card-bg); border: 1px solid var(--ribbon-line); border-radius: 4px; padding: 4px 8px; font-size: 11px; color: var(--ui-text); }
.input.readonly { background: var(--off-bg); color: var(--ui-muted); }
.input.num { flex: none; width: 56px; }
.mono { font-family: var(--mono-font); }
.x, .unit { font-size: 11px; color: var(--ui-muted); }
.check { display: flex; align-items: center; gap: 6px; font-size: 11px; color: var(--ui-text); }
.check input { accent-color: var(--acad-blue); }
.hint { margin: 0; font-size: 10px; line-height: 1.4; color: var(--ui-muted); }
.footer { display: flex; justify-content: flex-end; gap: 8px; padding: 10px 12px; border-top: 1px solid var(--ribbon-line); }
.btn { border-radius: 4px; padding: 5px 14px; font-size: 11px; font-weight: 600; cursor: pointer; }
.btn:disabled { opacity: 0.5; cursor: default; }
.btn.primary { background: var(--acad-blue); border: 1px solid var(--acad-blue); color: var(--card-bg); }
.btn.secondary { background: var(--card-bg); border: 1px solid var(--ribbon-line); color: var(--ui-text); }
</style>
