<script setup lang="ts">
// I/O図面の生成設定ダイアログ (Pencilデザイン「M4デザイン - PLC I/O」の
// 「PLC生成設定ダイアログ」準拠)。CAD調モーダル(幅440): グループボックス
// 「ラダー」(形式ラジオ+ラング間隔/先頭スキップ) と「モジュール配置」(方針ラジオ)+
// 注記+フッタ(生成/キャンセル)。割付表エディタのフッタから開く。
//
// v1が対応するのは縦バス+横ラング・モジュールごとに新ラダーだけで、他の選択肢は
// デザインどおり並べるが押せない (disabled + 「未実装」のtitle)。
// ラダー幅はv1の生成では使わないので列ごと出さない。
// 文字列はすべてi18nカタログ経由 (docs/internal/specs/i18n.md)。
import { useI18n } from "vue-i18n";
import { X } from "lucide-vue-next";
import type { LadderStyle, ModulePlacement } from "../ipc";
import { usePlcIoStore } from "../stores/plcIo";
import { useUiStore } from "../stores/ui";

const plc = usePlcIoStore();
const ui = useUiStore();
const { t } = useI18n();

/** ラダー形式 (v1は縦バスのみ実装)。 */
const styles: { id: LadderStyle; ready: boolean }[] = [
  { id: "vertical-bus", ready: true },
  { id: "horizontal-bus", ready: false },
];

/** 配置方針 (v1はモジュールごとに新ラダーのみ実装)。 */
const placements: { id: ModulePlacement; ready: boolean }[] = [
  { id: "new-ladder", ready: true },
  { id: "share-if-fits", ready: false },
  { id: "share-or-split", ready: false },
];

function cancel() {
  plc.cancelGenerate();
}

async function run() {
  const reference = plc.moduleRef ?? "";
  if (!(await plc.generate())) {
    ui.log(plc.error ?? t("plcIo.generateFailedLog", { reference }));
    return;
  }
  ui.log(t("plcIo.generatedLog", { reference, spacing: plc.options.rung_spacing_mm }));
}
</script>

<template>
  <div v-if="plc.open && plc.generateOpen" class="overlay" @click.self="cancel">
    <div class="dialog">
      <div class="titlebar">
        <span class="title">{{ t("plcIo.generateTitle") }}</span>
        <button class="close" :title="t('plcIo.cancel')" @click="cancel"><X :size="14" /></button>
      </div>

      <div class="body">
        <div class="group">
          <div class="group-head"><span>{{ t("plcIo.ladderGroup") }}</span><i /></div>
          <label v-for="s in styles" :key="s.id" class="choice" :title="s.ready ? '' : t('plcIo.notImplemented')">
            <input
              v-model="plc.options.ladder_style"
              type="radio"
              :value="s.id"
              :disabled="!s.ready"
            />
            {{ t(`plcIo.ladderStyle.${s.id}`) }}
            <span class="tag">{{ s.ready ? t("plcIo.jisHint") : t("plcIo.notImplemented") }}</span>
          </label>
          <div class="form-row">
            <label class="label">{{ t("plcIo.rungSpacing") }}</label>
            <input
              v-model.number="plc.options.rung_spacing_mm"
              class="input num mono"
              type="number"
              min="2.5"
              step="2.5"
            />
            <label class="label skip">{{ t("plcIo.startSkip") }}</label>
            <input
              v-model.number="plc.options.start_skip"
              class="input num mono"
              type="number"
              min="0"
              step="1"
            />
          </div>
        </div>

        <div class="group">
          <div class="group-head"><span>{{ t("plcIo.placementGroup") }}</span><i /></div>
          <label
            v-for="p in placements"
            :key="p.id"
            class="choice"
            :title="p.ready ? '' : t('plcIo.notImplemented')"
          >
            <input v-model="plc.options.placement" type="radio" :value="p.id" :disabled="!p.ready" />
            {{ t(`plcIo.placement.${p.id}`) }}
            <span v-if="!p.ready" class="tag">{{ t("plcIo.notImplemented") }}</span>
          </label>
        </div>

        <p class="caption">{{ t("plcIo.generateNote") }}</p>
        <p v-if="plc.error" class="error">{{ plc.error }}</p>
      </div>

      <div class="footer">
        <button class="btn secondary" @click="cancel">{{ t("plcIo.cancel") }}</button>
        <button class="btn primary" :disabled="!plc.canGenerate || plc.saving" @click="run">
          {{ t("plcIo.generate") }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.overlay {
  position: fixed;
  inset: 0;
  background: var(--scrim);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 110;
}
.dialog {
  width: min(440px, 94vw);
  max-height: 90vh;
  background: var(--ribbon-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 8px;
  box-shadow: var(--shadow-panel-lg);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.titlebar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 8px 12px;
  background: var(--palette-head);
  flex: none;
}
.title { font-size: 12px; font-weight: 600; color: var(--ui-text); }
.close {
  border: none;
  background: transparent;
  color: var(--ui-muted);
  display: flex;
  cursor: pointer;
  padding: 0;
}
.body {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 16px;
  overflow-y: auto;
}
.group { display: flex; flex-direction: column; gap: 8px; }
.group-head {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11px;
  font-weight: 600;
  color: var(--ui-text);
}
.group-head i { flex: 1; height: 1px; background: var(--ribbon-line); }
.choice { display: flex; align-items: center; gap: 6px; font-size: 11px; color: var(--ui-text); }
.choice input { accent-color: var(--acad-blue); }
.choice:has(input:disabled) { color: var(--ui-muted); }
.tag { font-size: 10px; color: var(--ui-muted); }
.form-row { display: flex; align-items: center; gap: 8px; }
.label { font-size: 11px; color: var(--ui-text); flex: none; }
.label.skip { margin-left: 6px; }
.input {
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  padding: 5px 8px;
  font-size: 11px;
  color: var(--ui-text);
  outline: none;
}
.input:focus { border-color: var(--acad-blue); }
.input.num { width: 72px; }
.mono { font-family: var(--mono-font); }
.caption { margin: 0; font-size: 10px; line-height: 15px; color: var(--ui-muted); }
.error { margin: 0; font-size: 11px; color: var(--err-fg); }
.footer {
  flex: none;
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 12px 16px;
  border-top: 1px solid var(--ribbon-line);
}
.btn {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  min-width: 72px;
  border-radius: 4px;
  padding: 5px 14px;
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
}
.btn:disabled { opacity: 0.5; cursor: default; }
.btn.primary { background: var(--acad-blue); border: 1px solid var(--acad-blue); color: var(--card-bg); }
.btn.secondary { background: var(--card-bg); border: 1px solid var(--ribbon-line); color: var(--ui-text); }
.btn.secondary:hover:not(:disabled) { background: var(--hover-bg); }
</style>
