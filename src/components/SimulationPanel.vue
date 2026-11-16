<script setup lang="ts">
// シミュレーション結果パネル (デザイン: MadakeCAD.pen「シミュレーション結果パネル」)。
// 見出し: 定格超過の件数。条件バー: 属性の置き換え・短絡のチップと入力。本体: ネット電圧・部品電流・定格超過 (行クリックで選択+ズーム)。
import { X } from "lucide-vue-next";
import { inject, ref } from "vue";
import { useI18n } from "vue-i18n";
import type { SimFinding } from "../ipc";
import type { EditorController } from "../tools/controller";
import { useDocumentStore } from "../stores/document";
import { useSimulationStore } from "../stores/simulation";

const sim = useSimulationStore();
const store = useDocumentStore();
const { t } = useI18n();
const controller = inject<EditorController>("controller")!;

const draft = ref("");
const invalid = ref(false);

const marks: Record<SimFinding["severity"], string> = { error: "✗", warning: "⚠", info: "ℹ" };

function volts(n: { volts_min: number; volts_max: number }): string {
  if (Math.abs(n.volts_max - n.volts_min) < 0.005) return `${n.volts_max.toFixed(2)} V`;
  return `${n.volts_min.toFixed(2)} 〜 ${n.volts_max.toFixed(2)} V`;
}

function add() {
  if (!draft.value.trim()) return;
  invalid.value = !sim.addCondition(draft.value);
  if (!invalid.value) draft.value = "";
}

function pick(f: SimFinding) {
  if (sim.sheetId && sim.sheetId !== store.activeSheetId) store.activeSheetId = sim.sheetId;
  controller.reveal(f.entity_ids);
}
</script>

<template>
  <div v-if="sim.panelOpen" class="panel">
    <div class="head">
      <span class="title">{{ t("simulation.title") }}</span>
      <span v-if="sim.result" class="chip">{{ t("simulation.source", { volts: sim.result.voltage }) }}</span>
      <span v-if="sim.counts.error" class="badge error">✗ {{ sim.counts.error }}</span>
      <span v-if="sim.counts.warning" class="badge warning">⚠ {{ sim.counts.warning }}</span>
      <span v-if="sim.counts.info" class="badge info">ℹ {{ sim.counts.info }}</span>
      <span v-if="sim.openSwitches.length" class="chip open">{{ t("simulation.open", { switches: sim.openSwitches.join(", ") }) }}</span>
      <span class="spacer" />
      <button class="rerun" :disabled="sim.running" @click="sim.run(store.activeSheetId)">{{ t("simulation.rerun") }}</button>
      <button class="close" @click="sim.close()"><X :size="12" /></button>
    </div>
    <div class="conditions">
      <span class="cond-head">{{ t("simulation.conditions") }}</span>
      <span v-for="c in sim.conditions" :key="c.key" class="cond-chip" :class="c.kind">
        {{ c.kind === "short" ? t("simulation.shortChip", { net: c.label }) : c.label }}
        <button class="cond-remove" :title="t('simulation.removeCondition')" @click="sim.removeCondition(c.key)"><X :size="10" /></button>
      </span>
      <input
        v-model="draft"
        class="cond-input"
        :class="{ invalid }"
        :placeholder="t('simulation.conditionHint')"
        @input="invalid = false"
        @keydown.enter="add()"
      />
      <button class="link" @click="add()">{{ t("simulation.addCondition") }}</button>
      <span v-if="invalid" class="cond-error">{{ t("simulation.conditionInvalid") }}</span>
      <span class="spacer" />
      <button v-if="sim.conditions.length" class="link muted" @click="sim.clearConditions()">{{ t("simulation.clearConditions") }}</button>
    </div>
    <div v-if="sim.error" class="error">{{ sim.error }}</div>
    <div v-else-if="sim.result" class="body">
      <div class="col nets">
        <div class="col-head">{{ t("simulation.netVoltages") }}</div>
        <div v-for="n in sim.result.nets" :key="n.name" class="row">
          <span class="mono">{{ n.name }}</span>
          <span class="spacer" />
          <span class="mono strong">{{ volts(n) }}</span>
        </div>
      </div>
      <div class="col parts left-border">
        <div class="col-head">{{ t("simulation.componentCurrents") }}</div>
        <div v-for="c in sim.result.components" :key="c.entity_id" class="row">
          <span class="mono">{{ c.reference }}</span>
          <span class="spacer" />
          <span class="mono strong">{{ c.amps.toFixed(3) }} A</span>
          <span class="mono dim">{{ c.watts.toFixed(2) }} W</span>
        </div>
      </div>
      <div class="col findings left-border">
        <div class="col-head pad">{{ t("simulation.findings") }}</div>
        <div v-if="sim.findings.length === 0" class="ok-note pad">{{ t("simulation.noFindings") }}</div>
        <button v-for="(f, i) in sim.findings" :key="f.code + ':' + f.reference + ':' + f.entity_ids.join(',') + ':' + i" class="finding" @click="pick(f)">
          <span class="mark" :class="f.severity">{{ marks[f.severity] }}</span>
          <span class="code">{{ f.code }}</span>
          <span class="msg">{{ f.message }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.panel {
  flex: none;
  max-height: 240px;
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
.title { font-size: 11px; font-weight: 600; color: var(--ui-text); }
.chip {
  font-family: var(--mono-font);
  font-size: 10px;
  font-weight: 600;
  color: var(--ui-muted);
  background: var(--hover-bg);
  border-radius: 999px;
  padding: 1px 8px;
}
.chip.open { color: var(--warn-fg); background: var(--warn-bg); }
.badge { font-size: 10px; font-weight: 600; border-radius: 999px; padding: 1px 8px; }
.badge.error { color: var(--err-fg); background: var(--err-bg); }
.badge.warning { color: var(--warn-fg); background: var(--warn-bg); }
.badge.info { color: var(--info-fg); background: var(--info-bg); }
.spacer { flex: 1; }
.rerun, .link {
  border: none;
  background: transparent;
  color: var(--acad-blue);
  font-size: 10px;
  font-weight: 600;
  cursor: pointer;
}
.link.muted { color: var(--ui-muted); font-weight: normal; }
.close { border: none; background: transparent; color: var(--ui-muted); display: flex; cursor: pointer; }
.conditions {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 10px;
  border-bottom: 1px solid var(--ribbon-line);
  flex-wrap: wrap;
}
.cond-head { font-size: 10px; font-weight: 600; color: var(--ui-muted); }
.cond-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-family: var(--mono-font);
  font-size: 10px;
  font-weight: 600;
  color: var(--ui-text);
  background: var(--hover-bg);
  border-radius: 999px;
  padding: 1px 6px 1px 8px;
}
.cond-chip.short { color: var(--err-fg); background: var(--err-bg); }
.cond-remove { border: none; background: transparent; color: var(--ui-muted); display: flex; padding: 0; cursor: pointer; }
.cond-input {
  width: 240px;
  font-size: 10px;
  padding: 2px 6px;
  border: 1px solid var(--ribbon-line);
  border-radius: 3px;
  background: var(--input-bg);
  color: var(--ui-text);
}
.cond-input.invalid { border-color: var(--err-fg); }
.cond-error { font-size: 10px; color: var(--err-fg); }
.error { padding: 10px 12px; font-size: 11px; color: var(--err-fg); }
.body { display: flex; overflow-y: auto; }
.col { padding: 6px 10px; display: flex; flex-direction: column; gap: 3px; }
.col.nets { flex: none; width: 210px; }
.col.parts { flex: none; width: 230px; }
.col.findings { flex: 1; padding: 6px 0; gap: 0; min-width: 0; }
.col.left-border { border-left: 1px solid var(--ribbon-line); }
.col-head { font-size: 10px; font-weight: 600; color: var(--ui-muted); }
.pad { padding: 0 10px 3px; }
.ok-note { font-size: 10px; color: var(--ok-fg); }
.row { display: flex; align-items: center; gap: 8px; }
.mono { font-family: var(--mono-font); font-size: 11px; color: var(--ui-text); }
.mono.strong { font-weight: 600; }
.mono.dim { color: var(--ui-muted); font-size: 10px; }
.finding {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  border: none;
  background: transparent;
  text-align: left;
  padding: 3px 10px;
  cursor: pointer;
}
.finding:hover { background: var(--hover-bg); }
.mark { font-size: 11px; font-weight: 600; }
.mark.error { color: var(--err-fg); }
.mark.warning { color: var(--warn-fg); }
.mark.info { color: var(--info-fg); }
.code { font-family: var(--mono-font); font-size: 10px; color: var(--ui-muted); flex: none; }
.msg { font-size: 11px; color: var(--ui-text); }
</style>
