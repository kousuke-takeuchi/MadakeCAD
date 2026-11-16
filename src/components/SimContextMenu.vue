<script setup lang="ts">
// シミュレーションの右クリックメニュー (デザイン: MadakeCAD.pen「シミュレーション 右クリックメニュー」)。
// 図面の電線・部品の上で開き、短絡・断線を選んでいる計算ケースに足す・外す。
import { useI18n } from "vue-i18n";
import type { SimMenuItem } from "../canvas/simMenu";

defineProps<{ x: number; y: number; items: SimMenuItem[] }>();
const emit = defineEmits<{ pick: [item: SimMenuItem]; close: [] }>();
const { t } = useI18n();

function label(item: SimMenuItem): string {
  switch (item.kind) {
    case "short_wire":
      return t("simulation.menu.shortWire");
    case "open_wire":
      return t("simulation.menu.openWire");
    case "short_pin":
      return t("simulation.menu.shortPin", { pin: item.pin });
    case "remove":
      return t("simulation.menu.remove");
  }
}
</script>

<template>
  <div class="backdrop" @click="emit('close')" @contextmenu.prevent="emit('close')" />
  <div class="menu" :style="{ left: x + 'px', top: y + 'px' }">
    <button
      v-for="(item, i) in items"
      :key="i"
      class="item"
      :class="item.kind"
      @click="emit('pick', item)"
    >{{ label(item) }}</button>
  </div>
</template>

<style scoped>
.backdrop {
  position: absolute;
  inset: 0;
  z-index: 9;
}
.menu {
  position: absolute;
  z-index: 10;
  display: flex;
  flex-direction: column;
  min-width: 150px;
  padding: 3px 0;
  border-radius: 4px;
  background: var(--canvas-chip-bg);
  border: 1px solid var(--canvas-chip-border);
}
.item {
  border: none;
  background: transparent;
  text-align: left;
  padding: 4px 12px;
  font-size: 11px;
  color: var(--canvas-chip-text);
  cursor: pointer;
}
.item:hover {
  background: var(--canvas-chip-border);
}
.item.remove {
  border-top: 1px solid var(--canvas-chip-border);
}
</style>
