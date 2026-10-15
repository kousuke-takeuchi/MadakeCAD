<script setup lang="ts">
// 整列・等間隔ポップオーバー (.pen「M4残りデザイン⑤」`B2gfQZ`の「整列 (選択3個・基準=最初に選んだK1)」)。
// DSアイコンボタン (26×24) で6方向の揃えと水平/垂直の等間隔を選ぶ。どれもCommand 1回 (undo一発)。
// レール上の部品は上下方向には動かさない (Rust側の規則)。
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  AlignCenterHorizontal,
  AlignCenterVertical,
  AlignEndHorizontal,
  AlignEndVertical,
  AlignHorizontalDistributeCenter,
  AlignStartHorizontal,
  AlignStartVertical,
  AlignVerticalDistributeCenter,
} from "lucide-vue-next";
import type { AlignMode } from "../ipc";
import { useDocumentStore } from "../stores/document";
import { useUiStore } from "../stores/ui";

const emit = defineEmits<{ close: [] }>();
const store = useDocumentStore();
const ui = useUiStore();
const { t } = useI18n();
const pitch = ref<string | number>("");

/** 選択中のフットプリント (選んだ順。基準=先頭)。 */
const picked = computed(() => {
  const sheet = store.activeSheet;
  if (!sheet) return [];
  return [...store.selection].filter((id) => sheet.entities[id]?.kind === "footprint");
});
const base = computed(() => {
  const id = picked.value[0];
  const e = id ? store.activeSheet?.entities[id] : undefined;
  return e && e.kind === "footprint" ? e.reference || "?" : "";
});

const aligns: { mode: AlignMode; icon: unknown }[] = [
  { mode: "left", icon: AlignStartVertical },
  { mode: "center_x", icon: AlignCenterVertical },
  { mode: "right", icon: AlignEndVertical },
  { mode: "top", icon: AlignStartHorizontal },
  { mode: "center_y", icon: AlignCenterHorizontal },
  { mode: "bottom", icon: AlignEndHorizontal },
];

async function align(mode: AlignMode) {
  const sheet = store.activeSheet;
  if (!sheet || picked.value.length < 2) return;
  try {
    await store.execute({ type: "align_footprints", sheet_id: sheet.id, ids: picked.value, mode });
    ui.log(t("panelMarks.align.doneLog", { count: picked.value.length, mode: t(`panelMarks.align.mode.${mode}`) }));
  } catch (e) {
    ui.log(String(e));
  }
}

async function distribute(axis: "horizontal" | "vertical") {
  const sheet = store.activeSheet;
  if (!sheet) return;
  const p = String(pitch.value).trim() === "" ? null : Number(pitch.value);
  try {
    await store.execute({ type: "distribute_footprints", sheet_id: sheet.id, ids: picked.value, axis, pitch: p });
    ui.log(t("panelMarks.align.distributedLog", { count: picked.value.length }));
  } catch (e) {
    ui.log(String(e));
  }
}
</script>

<template>
  <div class="backdrop" @click="emit('close')" />
  <div class="pop">
    <div class="head">{{ picked.length >= 2 ? t("panelMarks.align.title", { count: picked.length, base }) : t("panelMarks.align.needTwo") }}</div>
    <div class="buttons">
      <button
        v-for="a in aligns"
        :key="a.mode"
        class="icon-btn"
        :title="t(`panelMarks.align.mode.${a.mode}`)"
        :disabled="picked.length < 2"
        @click="align(a.mode)"
      >
        <component :is="a.icon" :size="14" />
      </button>
    </div>
    <div class="buttons">
      <button class="icon-btn" :title="t('panelMarks.align.distributeH')" :disabled="picked.length < 2" @click="distribute('horizontal')">
        <AlignHorizontalDistributeCenter :size="14" />
      </button>
      <button class="icon-btn" :title="t('panelMarks.align.distributeV')" :disabled="picked.length < 2" @click="distribute('vertical')">
        <AlignVerticalDistributeCenter :size="14" />
      </button>
      <span class="label">{{ t("panelMarks.align.pitch") }}</span>
      <input v-model="pitch" class="pitch mono" type="number" min="0" step="5" :placeholder="t('panelMarks.align.pitchAuto')" />
    </div>
    <p class="hint">{{ t("panelMarks.align.hint") }}</p>
  </div>
</template>

<style scoped>
.backdrop { position: fixed; inset: 0; z-index: 40; }
.pop { position: fixed; top: 132px; left: 50%; transform: translateX(-50%); z-index: 41; width: 220px; padding: 8px; display: flex; flex-direction: column; gap: 6px; background: var(--card-bg); border: 1px solid var(--ribbon-line); border-radius: 6px; box-shadow: var(--shadow-popup); }
.head { font-size: 10px; font-weight: 600; color: var(--ui-text); }
.buttons { display: flex; align-items: center; gap: 4px; }
/* DSアイコンボタン (.pen mgvgv) */
.icon-btn { width: 26px; height: 24px; display: flex; align-items: center; justify-content: center; background: var(--card-bg); border: 1px solid var(--ribbon-line); border-radius: 4px; color: var(--ribbon-icon); cursor: pointer; }
.icon-btn:hover:not(:disabled) { background: var(--sel-blue); border-color: var(--acad-blue); color: var(--acad-blue); }
.icon-btn:disabled { opacity: 0.4; cursor: default; }
.label { font-size: 10px; color: var(--ui-muted); margin-left: 4px; }
.pitch { width: 56px; padding: 2px 6px; font-size: 10px; border: 1px solid var(--ribbon-line); border-radius: 4px; background: var(--input-bg); color: var(--ui-text); }
.mono { font-family: var(--mono-font); }
.hint { margin: 0; font-size: 10px; line-height: 1.4; color: var(--ui-muted); }
</style>
