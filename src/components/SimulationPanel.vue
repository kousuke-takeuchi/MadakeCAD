<script setup lang="ts">
// シミュレーション結果パネル (デザイン: MadakeCAD.pen「シミュレーション結果パネル」)。
// 見出し: 定格超過の件数と「図面に重ねる」。条件バー: 属性の置き換え・短絡のチップと入力。古い結果の注意。
// 本体: 定格超過 (重さで絞り込み、行クリックで選択+ズーム) と選択中の詳細。下端: すべての値 (ネット電圧・部品電流) の折りたたみ。
import { ChevronDown, ChevronRight, Copy, Layers, Pencil, Plus, RefreshCw, Trash2, X } from "lucide-vue-next";
import { computed, inject, ref } from "vue";
import { useI18n } from "vue-i18n";
import type { SimFinding } from "../ipc";
import type { EditorController } from "../tools/controller";
import { useDocumentStore } from "../stores/document";
import { useSimulationStore } from "../stores/simulation";
import { formatProbe } from "../canvas/simOverlay";
import { roleAttributes } from "../canvas/simCaseView";

const sim = useSimulationStore();
const store = useDocumentStore();
const { t } = useI18n();
const controller = inject<EditorController>("controller")!;

const draft = ref("");
const invalid = ref(false);

const marks: Record<SimFinding["severity"], string> = { error: "✗", warning: "⚠", info: "ℹ" };
const severities: SimFinding["severity"][] = ["error", "warning", "info"];

/** 定格超過の絞り込み (表示する重さ)。 */
const shown = ref(new Set<SimFinding["severity"]>(severities));
function toggleSeverity(sv: SimFinding["severity"]) {
  const next = new Set(shown.value);
  if (next.has(sv)) next.delete(sv);
  else next.add(sv);
  shown.value = next;
}
const visibleFindings = computed(() => sim.findings.filter((f) => shown.value.has(f.severity)));

/** ケースの名前の変更中の文字 (null = 変更していない) と、断られたときの印。 */
const renaming = ref<string | null>(null);
const renameRefused = ref(false);
async function commitRename() {
  if (renaming.value === null) return;
  const ok = await sim.renameCase(renaming.value);
  renameRefused.value = !ok;
  if (ok) renaming.value = null;
}
function onCaseSelect(e: Event) {
  const v = (e.target as HTMLSelectElement).value;
  renaming.value = null;
  sim.selectCase(v === "" ? null : v);
}

/** 図面で選んだ部品 (値の編集の対象)。 */
const selectedSymbol = computed(() => {
  const sheet = store.activeSheet;
  if (!sheet) return null;
  for (const id of store.selection) {
    const e = sheet.entities[id];
    if (e && e.kind === "symbol") return e;
  }
  return null;
});
/** 選んだ部品の役割に関係する値 (図面の値と、このケースでの値)。 */
const valueRows = computed(() => {
  const e = selectedSymbol.value;
  if (!e) return [];
  const caseAttrs = sim.selectedCase?.overrides[e.reference] ?? {};
  return roleAttributes(store.resolveInstance(e)).map((attr) => ({
    attr,
    drawing: e.attrs[attr] ?? "—",
    value: caseAttrs[attr] ?? "",
  }));
});
function setValue(attr: string, ev: Event) {
  const e = selectedSymbol.value;
  if (!e || !e.reference) return;
  const v = (ev.target as HTMLInputElement).value;
  const current = sim.selectedCase?.overrides[e.reference]?.[attr] ?? "";
  if (v.trim() === current) return;
  void sim.setOverride(e.reference, attr, v);
}

/** 条件のチップの文字。 */
function chipLabel(c: { kind: "set" | "short" | "open"; label: string; missing: boolean }): string {
  const text =
    c.kind === "short" ? t("simulation.shortChip", { net: c.label }) : c.kind === "open" ? t("simulation.openChip", { label: c.label }) : c.label;
  return c.missing ? `${text} (${t("simulation.missing")})` : text;
}

/** すべての値 (ネット電圧・部品電流) を開いているか。 */
const showAll = ref(false);

/** 選択中の電線・部品の値。 */
const selected = computed(() => {
  const load = sim.selectedLoad;
  if (!load) return null;
  const e = store.activeSheet?.entities[load.entity_id];
  const name = e && e.kind === "symbol" ? e.reference || e.symbol_id : t("simulation.wire");
  return { load, name, text: formatProbe(load) };
});

function volts(n: { volts_min: number; volts_max: number }): string {
  if (Math.abs(n.volts_max - n.volts_min) < 0.005) return `${n.volts_max.toFixed(2)} V`;
  return `${n.volts_min.toFixed(2)} 〜 ${n.volts_max.toFixed(2)} V`;
}

async function add() {
  if (!draft.value.trim()) return;
  invalid.value = !(await sim.addCondition(draft.value));
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
      <input
        v-if="renaming !== null"
        v-model="renaming"
        class="case-rename"
        :class="{ invalid: renameRefused }"
        :title="renameRefused ? t('simulation.caseNameTaken') : ''"
        @input="renameRefused = false"
        @keydown.enter="commitRename()"
        @keydown.esc="renaming = null"
      />
      <select v-else class="case-select" :value="sim.selectedCase?.id ?? ''" @change="onCaseSelect">
        <option value="">{{ t("simulation.normalCase") }}</option>
        <option v-for="c in sim.cases" :key="c.id" :value="c.id">{{ c.name }}</option>
      </select>
      <button class="icon" :title="t('simulation.newCase')" @click="sim.newCase()"><Plus :size="12" /></button>
      <button class="icon" :title="t('simulation.renameCase')" :disabled="!sim.selectedCase" @click="renaming = sim.selectedCase?.name ?? null"><Pencil :size="12" /></button>
      <button class="icon" :title="t('simulation.duplicateCase')" :disabled="!sim.selectedCase" @click="sim.duplicateCase()"><Copy :size="12" /></button>
      <button class="icon" :title="t('simulation.deleteCase')" :disabled="!sim.selectedCase" @click="sim.deleteCase()"><Trash2 :size="12" /></button>
      <span v-if="renameRefused" class="cond-error">{{ t("simulation.caseNameTaken") }}</span>
      <span v-if="sim.counts.error" class="badge error">✗ {{ sim.counts.error }}</span>
      <span v-if="sim.counts.warning" class="badge warning">⚠ {{ sim.counts.warning }}</span>
      <span v-if="sim.counts.info" class="badge info">ℹ {{ sim.counts.info }}</span>
      <button class="toggle" :class="{ on: sim.showOnDrawing }" @click="sim.toggleShowOnDrawing()">
        <Layers :size="11" />{{ t("simulation.showOnDrawing") }}
      </button>
      <span v-if="sim.openSwitches.length" class="chip open">{{ t("simulation.open", { switches: sim.openSwitches.join(", ") }) }}</span>
      <span class="spacer" />
      <button class="rerun" :disabled="sim.running" @click="sim.run(store.activeSheetId)">{{ t("simulation.rerun") }}</button>
      <button class="close" @click="sim.close()"><X :size="12" /></button>
    </div>
    <div class="conditions">
      <span class="cond-head">{{ t("simulation.conditions") }}</span>
      <span v-for="c in sim.conditions" :key="c.key" class="cond-chip" :class="[c.kind, { missing: c.missing }]">
        {{ chipLabel(c) }}
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
    <div v-if="sim.stale && !sim.error" class="stale"><RefreshCw :size="11" />{{ t("simulation.stale") }}</div>
    <div v-if="sim.error" class="error">{{ sim.error }}</div>
    <div v-else-if="sim.result" class="body">
      <div class="col findings">
        <div class="col-head pad">
          {{ t("simulation.findings") }}
          <button
            v-for="sv in severities"
            :key="sv"
            class="filter"
            :class="[sv, { off: !shown.has(sv) }]"
            @click="toggleSeverity(sv)"
          >{{ marks[sv] }} {{ sim.counts[sv] }}</button>
        </div>
        <div v-if="sim.findings.length === 0" class="ok-note pad">{{ t("simulation.noFindings") }}</div>
        <button v-for="(f, i) in visibleFindings" :key="f.code + ':' + f.reference + ':' + f.entity_ids.join(',') + ':' + i" class="finding" @click="pick(f)">
          <span class="mark" :class="f.severity">{{ marks[f.severity] }}</span>
          <span class="code">{{ f.code }}</span>
          <span class="msg">{{ f.message }}</span>
        </button>
      </div>
      <div class="col details left-border">
        <div class="col-head">{{ t("simulation.details") }}</div>
        <template v-if="selected">
          <div class="row">
            <span class="mono name">{{ selected.name }}</span>
            <span v-if="(selected.load.ratio ?? 0) > 1" class="badge error">{{ t("simulation.overBadge", { ratio: selected.text.ratio }) }}</span>
          </div>
          <div class="row"><span class="label">{{ t("simulation.current") }}</span><span class="spacer" /><span class="mono strong">{{ selected.text.amps }}</span></div>
          <div class="row"><span class="label">{{ t("simulation.voltages") }}</span><span class="spacer" /><span class="mono strong">{{ selected.text.volts }}</span></div>
          <div v-if="selected.text.limit" class="row">
            <span class="label">{{ t("simulation.limit") }}</span><span class="spacer" />
            <span class="mono strong">{{ selected.text.limit }} ({{ t("simulation.limitKinds." + selected.load.limit_kind) }})</span>
          </div>
          <div v-if="selected.text.ratio" class="row"><span class="label">{{ t("simulation.ratio") }}</span><span class="spacer" /><span class="mono strong">{{ selected.text.ratio }}</span></div>
        </template>
        <template v-if="valueRows.length">
          <div class="row value-head">
            <span class="attr">{{ t("simulation.values") }}</span>
            <span class="drawing">{{ t("simulation.drawingValue") }}</span>
            <span class="case-col">{{ t("simulation.caseValue") }}</span>
          </div>
          <div v-for="r in valueRows" :key="(selectedSymbol?.id ?? '') + r.attr" class="row">
            <span class="mono attr">{{ r.attr }}</span>
            <span class="mono drawing">{{ r.drawing }}</span>
            <input
              class="case-col case-value"
              :class="{ set: r.value !== '' }"
              :value="r.value"
              placeholder="—"
              @keydown.enter="setValue(r.attr, $event)"
              @blur="setValue(r.attr, $event)"
            />
          </div>
        </template>
        <div v-if="!selected && !valueRows.length" class="hint">{{ t("simulation.selectHint") }}</div>
      </div>
    </div>
    <button v-if="sim.result && !sim.error" class="all-bar" @click="showAll = !showAll">
      <component :is="showAll ? ChevronDown : ChevronRight" :size="11" />
      {{ showAll ? t("simulation.hideAll") : t("simulation.showAll", { nets: sim.result.nets.length, parts: sim.result.components.length }) }}
    </button>
    <div v-if="showAll && sim.result && !sim.error" class="body all">
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
    </div>
  </div>
</template>

<style scoped>
.panel {
  flex: none;
  max-height: 320px;
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
.cond-chip.open { color: var(--warn-fg); background: var(--warn-bg); }
.cond-chip.missing { color: var(--ui-muted); background: var(--hover-bg); opacity: 0.7; }
.case-select, .case-rename {
  font-size: 10px;
  font-weight: 600;
  padding: 1px 6px;
  border: 1px solid var(--ribbon-line);
  border-radius: 3px;
  background: var(--input-bg);
  color: var(--ui-text);
  max-width: 160px;
}
.case-rename.invalid { border-color: var(--err-fg); }
.icon { border: none; background: transparent; color: var(--ui-muted); display: flex; padding: 2px; cursor: pointer; }
.icon:disabled { opacity: 0.35; cursor: default; }
.value-head { padding-top: 6px; font-size: 9px; color: var(--ui-muted); }
.attr { flex: none; width: 90px; font-size: 10px; }
.value-head .attr { font-size: 10px; font-weight: 600; }
.drawing { flex: 1; font-size: 10px; color: var(--ui-muted); }
.case-col { flex: none; width: 90px; }
.case-value {
  font-family: var(--mono-font);
  font-size: 10px;
  padding: 1px 6px;
  border: 1px solid var(--ribbon-line);
  border-radius: 3px;
  background: var(--input-bg);
  color: var(--ui-text);
}
.case-value.set { border-color: var(--acad-blue); }
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
.col.details { flex: none; width: 300px; }
.toggle {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  border: none;
  border-radius: 999px;
  padding: 1px 8px;
  font-size: 10px;
  font-weight: 600;
  cursor: pointer;
  color: var(--ui-muted);
  background: var(--hover-bg);
}
.toggle.on { color: var(--acad-blue); background: var(--info-bg); }
.stale {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 3px 10px;
  font-size: 10px;
  font-weight: 600;
  color: var(--warn-fg);
  background: var(--warn-bg);
}
.filter { border: none; border-radius: 999px; padding: 0 6px; margin-left: 6px; font-size: 9px; font-weight: 600; cursor: pointer; }
.filter.error { color: var(--err-fg); background: var(--err-bg); }
.filter.warning { color: var(--warn-fg); background: var(--warn-bg); }
.filter.info { color: var(--info-fg); background: var(--info-bg); }
.filter.off { opacity: 0.4; }
.name { font-size: 12px; font-weight: 600; }
.label { font-size: 10px; color: var(--ui-muted); }
.hint { font-size: 10px; color: var(--ui-muted); }
.all-bar {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  border: none;
  border-top: 1px solid var(--ribbon-line);
  background: transparent;
  padding: 4px 10px;
  font-size: 10px;
  font-weight: 600;
  color: var(--acad-blue);
  cursor: pointer;
  text-align: left;
}
.body.all { border-top: 1px solid var(--ribbon-line); }
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
