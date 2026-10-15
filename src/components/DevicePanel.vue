<script setup lang="ts">
// デバイスナビゲータ (デザイン: .pen「M4デザイン - 検索/デバイスナビゲータ/Surfer」)。
// 左ドックの「デバイス」タブ。参照記号ツリーを機能単位まで展開し、行クリックで
// 図面へジャンプ (シート切替+選択+ズーム)。右クリック=ジャンプ/削除。
//
// 未配置機能 (予約接点。.pen「M4残りデザイン②」`L314JT`): リレーの接点構成にあって図面に無い
// 接点を「◌ … 未配置」行で並べ、クリック/右クリック「配置」で配置モード、作図領域へドラッグ&
// ドロップでその位置へ置く。置くのは普通の add_entity 1回 (Cmd+Zで戻る)。
import { computed, inject, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  deviceHeadline,
  deviceJumpTarget,
  functionAddress,
  functionHeadline,
  unplacedHeadline,
  type DeviceFilter,
  type DeviceRow,
} from "../canvas/deviceTree";
import type { EditorController } from "../tools/controller";
import { useReveal } from "../composables/reveal";
import type { DeviceNode, UnplacedFunction } from "../ipc";
import { rowKey, useDevicesStore } from "../stores/devices";
import { useDocumentStore } from "../stores/document";
import { useUiStore } from "../stores/ui";

const { t, locale } = useI18n();
const devices = useDevicesStore();
const store = useDocumentStore();
const ui = useUiStore();
const reveal = useReveal();
const controller = inject<EditorController>("controller")!;

/** 右クリックメニュー (デバイス行・未配置行)。未配置行なら`unplaced`を持つ。 */
const menu = ref<{ x: number; y: number; device: DeviceNode; unplaced?: UnplacedFunction } | null>(null);

const FILTERS: DeviceFilter[] = ["all", "unplaced", "placed"];

/** 未配置行の名前 (例 「接点 33-34 NO」)。 */
function unplacedName(u: UnplacedFunction): string {
  const spec = unplacedHeadline(u);
  return t(spec.key, spec.params ?? {});
}

/** 未配置の接点を置くモードに入る (行クリック・右クリック「配置」・ドラッグ開始)。 */
function startPlacement(reference: string, u: UnplacedFunction) {
  menu.value = null;
  controller.startContactPlacement(reference, u.symbol_id, `${reference} ${unplacedName(u)}`);
  ui.log(t("devices.placeStartLog", { reference, name: unplacedName(u) }));
}

/** ドラッグ開始: 配置モードに入り、作図領域でのドロップを待つ (CanvasViewが受ける)。 */
function onDragStart(event: DragEvent, reference: string, u: UnplacedFunction) {
  event.dataTransfer?.setData("application/x-madake-unplaced", `${reference} ${u.terminals}`);
  if (event.dataTransfer) event.dataTransfer.effectAllowed = "copy";
  startPlacement(reference, u);
}

function placeAll() {
  menu.value = null;
  ui.log(t("devices.placeAllNotImplemented"));
}

const japanese = computed(() => locale.value === "ja");

function headline(device: DeviceNode): string {
  const spec = deviceHeadline(device, japanese.value);
  return t(spec.key, spec.params ?? {});
}

function functionName(row: Extract<DeviceRow, { type: "function" }>): string {
  const spec = functionHeadline(row.fn);
  return t(spec.key, spec.params ?? {});
}

function onRow(row: DeviceRow) {
  devices.select(rowKey(row));
  if (row.type === "device") {
    devices.toggle(row.reference);
    return;
  }
  if (row.type === "unplaced") {
    startPlacement(row.reference, row.unplaced);
    return;
  }
  reveal(deviceJumpTarget(row.fn));
  ui.log(
    t("devices.revealLog", {
      reference: row.reference,
      function: functionName(row),
      address: functionAddress(row.fn),
    }),
  );
}

function onContextMenu(event: MouseEvent, device: DeviceNode) {
  menu.value = { x: event.clientX, y: event.clientY, device };
}

function jumpToDevice(device: DeviceNode) {
  menu.value = null;
  const first = device.functions[0];
  if (!first) return;
  reveal(deviceJumpTarget(first));
}

async function removeDevice(device: DeviceNode) {
  menu.value = null;
  const count = await devices.deleteDevice(device);
  ui.log(t("devices.deletedLog", { reference: device.reference, count }));
}

onMounted(() => void devices.load());
// 図面が変われば読み直す (部品を置く・消す・undoのたび)
watch(
  () => store.revision,
  () => void devices.load(),
);
</script>

<template>
  <div class="panel">
    <div class="panel-head">
      <span>{{ t("devices.tab") }}</span>
      <span class="count">{{ t("devices.count", { count: devices.count }) }}</span>
    </div>
    <div class="filters">
      <button
        v-for="f in FILTERS"
        :key="f"
        class="filter"
        :class="{ on: devices.filter === f }"
        @click="devices.filter = f"
      >
        {{ t(`devices.filter.${f}`) }}
      </button>
    </div>
    <div class="tree">
      <template v-for="row in devices.rows" :key="rowKey(row)">
        <button
          v-if="row.type === 'device'"
          class="row device"
          :class="{ on: devices.selectedKey === rowKey(row) }"
          @click="onRow(row)"
          @contextmenu.prevent="onContextMenu($event, row.device)"
        >
          <span class="glyph">{{ row.expanded ? "▾" : "▸" }}</span>
          <span class="ref">{{ row.reference }}</span>
          <span class="head-text">{{ headline(row.device) }}</span>
        </button>
        <button
          v-else-if="row.type === 'unplaced'"
          class="row fn unplaced"
          :class="{ on: devices.selectedKey === rowKey(row) }"
          draggable="true"
          :title="t('devices.unplacedHint')"
          @click="onRow(row)"
          @dragstart="onDragStart($event, row.reference, row.unplaced)"
          @contextmenu.prevent="menu = { x: $event.clientX, y: $event.clientY, device: row.device, unplaced: row.unplaced }"
        >
          <span class="mark">◌</span>
          <span class="fn-name">{{ unplacedName(row.unplaced) }}</span>
          <span class="spacer" />
          <span class="unplaced-badge">{{ t("devices.unplacedBadge") }}</span>
        </button>
        <button
          v-else
          class="row fn"
          :class="{ on: devices.selectedKey === rowKey(row) }"
          @click="onRow(row)"
          @contextmenu.prevent="onContextMenu($event, row.device)"
        >
          <span class="fn-name">{{ functionName(row) }}</span>
          <span class="spacer" />
          <span class="addr">{{ functionAddress(row.fn) }}</span>
        </button>
      </template>
      <div v-if="devices.count === 0" class="empty">{{ t("devices.empty") }}</div>
    </div>
    <div class="spacer" />
    <div class="hint">{{ t("devices.hint") }}</div>

    <template v-if="menu">
      <div class="backdrop" @click="menu = null" @contextmenu.prevent="menu = null" />
      <div v-if="menu.unplaced" class="menu" :style="{ left: `${menu.x}px`, top: `${menu.y}px` }">
        <button @click="startPlacement(menu.device.reference, menu.unplaced)">{{ t("devices.menu.place") }}</button>
        <button @click="placeAll">{{ t("devices.menu.placeAll") }}</button>
      </div>
      <div v-else class="menu" :style="{ left: `${menu.x}px`, top: `${menu.y}px` }">
        <button @click="jumpToDevice(menu.device)">{{ t("devices.menu.reveal") }}</button>
        <button class="danger" @click="removeDevice(menu.device)">
          {{ t("devices.menu.delete") }}
        </button>
      </div>
    </template>
  </div>
</template>

<style scoped>
/* 左ドック (LeftPanel) のタブ内容。幅と右境界はドック側が持つ */
.panel {
  width: 100%;
  flex: 1;
  min-height: 0;
  background: var(--palette-bg);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  user-select: none;
}
.panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 5px 9px;
  background: var(--palette-head);
  font-size: 11px;
  font-weight: 600;
  color: var(--ui-text);
}
.count {
  font-size: 10px;
  font-weight: 400;
  color: var(--ui-muted);
}
.tree {
  overflow-y: auto;
  padding: 6px 0;
}
.row {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  border: none;
  background: transparent;
  text-align: left;
  cursor: pointer;
}
.row:hover {
  background: var(--hover-bg);
}
.row.on {
  background: var(--sel-blue);
}
.row.device {
  padding: 3px 8px;
  font-family: var(--mono-font);
  font-size: 10px;
  font-weight: 600;
  color: var(--ui-text);
}
.glyph {
  flex: none;
  width: 8px;
  color: var(--ui-muted);
}
.head-text {
  font-family: inherit;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
/* 機能行はデバイス行より14px深いインデント (デザイン準拠) */
.row.fn {
  padding: 3px 8px 3px 22px;
}
.fn-name {
  font-size: 10px;
  color: var(--ui-text);
}
.addr {
  flex: none;
  font-family: var(--mono-font);
  font-size: 10px;
  color: var(--ui-placeholder);
}
.spacer {
  flex: 1;
}
.empty {
  padding: 6px 10px;
  font-size: 10px;
  color: var(--ui-muted);
}
.hint {
  padding: 4px 10px;
  font-size: 10px;
  color: var(--ui-muted);
}
.filters {
  display: flex;
  gap: 6px;
  padding: 6px 10px 2px;
}
.filter {
  border: 1px solid var(--ribbon-line);
  background: var(--card-bg);
  border-radius: 4px;
  padding: 2px 8px;
  font-size: 10px;
  color: var(--ui-text);
  cursor: pointer;
}
.filter.on {
  background: var(--sel-blue);
  border-color: var(--acad-blue);
  font-weight: 600;
}
/* DS未配置ラベル (.pen p3f12h): ◌+「未配置」を warn-fg で */
.row.unplaced { cursor: grab; }
.mark { font-size: 10px; color: var(--warn-fg); }
.unplaced-badge {
  flex: none;
  font-size: 10px;
  color: var(--warn-fg);
}
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 30;
}
.menu {
  position: fixed;
  z-index: 31;
  min-width: 140px;
  padding: 4px 0;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  box-shadow: var(--shadow-popup);
  display: flex;
  flex-direction: column;
}
.menu button {
  border: none;
  background: transparent;
  text-align: left;
  padding: 5px 12px;
  font-size: 11px;
  color: var(--ui-text);
  cursor: pointer;
}
.menu button:hover {
  background: var(--hover-bg);
}
.menu button.danger {
  color: var(--err-fg);
}
</style>
