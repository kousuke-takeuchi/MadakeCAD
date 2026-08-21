<script setup lang="ts">
// シミュレーション結果パネル (デザイン: MadakeCAD.pen「シミュレーション結果パネル」)。
import { X } from "lucide-vue-next";
import { useDocumentStore } from "../stores/document";
import { useSimulationStore } from "../stores/simulation";

const sim = useSimulationStore();
const store = useDocumentStore();

function volts(n: { volts_min: number; volts_max: number }): string {
  if (Math.abs(n.volts_max - n.volts_min) < 0.005) return `${n.volts_max.toFixed(2)} V`;
  return `${n.volts_min.toFixed(2)} 〜 ${n.volts_max.toFixed(2)} V`;
}
</script>

<template>
  <div v-if="sim.panelOpen" class="panel">
    <div class="head">
      <span class="title">シミュレーション (DC動作点)</span>
      <span v-if="sim.result" class="chip">電源 {{ sim.result.voltage }}V</span>
      <span v-if="sim.openSwitches.length" class="chip open">開路: {{ sim.openSwitches.join(", ") }}</span>
      <span class="spacer" />
      <button class="rerun" :disabled="sim.running" @click="sim.run(store.activeSheetId)">再実行</button>
      <button class="close" @click="sim.close()"><X :size="12" /></button>
    </div>
    <div v-if="sim.error" class="error">{{ sim.error }}</div>
    <div v-else-if="sim.result" class="body">
      <div class="col">
        <div class="col-head">ネット電圧</div>
        <div v-for="n in sim.result.nets" :key="n.name" class="row">
          <span class="mono">{{ n.name }}</span>
          <span class="spacer" />
          <span class="mono strong">{{ volts(n) }}</span>
        </div>
      </div>
      <div class="col left-border">
        <div class="col-head">部品電流</div>
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
  max-height: 200px;
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
  border-radius: 8px;
  padding: 1px 8px;
}
.chip.open { color: var(--warn-fg); background: var(--warn-bg); }
.spacer { flex: 1; }
.rerun {
  border: none;
  background: transparent;
  color: var(--acad-blue);
  font-size: 10px;
  font-weight: 600;
  cursor: pointer;
}
.close { border: none; background: transparent; color: var(--ui-muted); display: flex; cursor: pointer; }
.error { padding: 10px 12px; font-size: 11px; color: var(--err-fg); }
.body { display: flex; overflow-y: auto; }
.col { flex: 1; padding: 6px 10px; display: flex; flex-direction: column; gap: 3px; }
.col.left-border { border-left: 1px solid var(--ribbon-line); }
.col-head { font-size: 10px; font-weight: 600; color: var(--ui-muted); }
.row { display: flex; align-items: center; gap: 8px; }
.mono { font-family: var(--mono-font); font-size: 11px; color: var(--ui-text); }
.mono.strong { font-weight: 600; }
.mono.dim { color: var(--ui-muted); font-size: 10px; }
</style>
