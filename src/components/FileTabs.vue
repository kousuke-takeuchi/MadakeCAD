<script setup lang="ts">
// 図面ファイルタブ行 (Pencilデザイン準拠)。シートをタブとして表示する。
// M4 §8: シートには種別 (回路図 / 盤レイアウト) があり、追加時に選ぶ。表示中のシートの
// 種別は行の右端に出し、盤レイアウトシートを開くとリボン・左ドックも盤の面へ切り替わる。
import { LayoutPanelTop, Plus, X } from "lucide-vue-next";
import { computed, watch } from "vue";
import { useI18n } from "vue-i18n";
import { addSheetCommand, isPanelSheet, sheetKind, sheetKindLabelKey } from "../canvas/sheets";
import { usePopover } from "../composables/popover";
import type { SheetKind } from "../ipc";
import { useDocumentStore } from "../stores/document";
import { useUiStore } from "../stores/ui";

const store = useDocumentStore();
const ui = useUiStore();
const { t } = useI18n();
const { open: addOpen, toggle: toggleAdd, close: closeAdd } = usePopover();

const kindLabel = computed(() => t(sheetKindLabelKey(store.activeSheet)));

/** 種別を選んでシートを追加する (`add_sheet`コマンド1回 = undo一発)。 */
async function addSheet(kind: SheetKind) {
  closeAdd();
  const command = addSheetCommand(store.project, kind, store.activeSheetId);
  await store.execute(command);
  const sheets = store.project?.sheets ?? [];
  store.activeSheetId = sheets[sheets.length - 1]?.id ?? null;
  ui.log(
    t("sheets.addedLog", {
      name: command.type === "add_sheet" ? command.name : "",
      kind: t(`sheets.kind.${kind}`),
    }),
  );
}

async function closeSheet(id: string) {
  if ((store.project?.sheets.length ?? 0) <= 1) {
    ui.log(t("fileTabs.lastSheetLog"));
    return;
  }
  if (!window.confirm(t("fileTabs.deleteConfirm"))) return;
  await store.execute({ type: "remove_sheet", sheet_id: id });
}

// 表示中のシートの種別に合わせてリボン (パネルタブ) と左ドック (未配置部品) を切り替える
watch(
  () => sheetKind(store.activeSheet),
  (kind) => ui.syncSheetKind(kind),
  { immediate: true },
);
</script>

<template>
  <div class="file-tabs">
    <button
      v-for="s in store.project?.sheets ?? []"
      :key="s.id"
      class="tab"
      :class="{ active: s.id === store.activeSheet?.id }"
      @click="store.activeSheetId = s.id"
    >
      <LayoutPanelTop v-if="isPanelSheet(s)" :size="10" class="kind-icon" />
      {{ s.name }}
      <X
        v-if="s.id === store.activeSheet?.id"
        :size="10"
        class="close"
        @click.stop="closeSheet(s.id)"
      />
    </button>
    <div class="add-wrap">
      <button class="add" :title="t('sheets.addTitle')" @click="toggleAdd()">
        <Plus :size="12" />
      </button>
      <template v-if="addOpen">
        <div class="backdrop" @click="closeAdd()" />
        <div class="add-menu">
          <div class="menu-head">{{ t("sheets.addTitle") }}</div>
          <button class="menu-row" @click="addSheet('schematic')">
            {{ t("sheets.addSchematic") }}
          </button>
          <button class="menu-row" @click="addSheet('panel')">
            <LayoutPanelTop :size="12" class="menu-icon" />
            {{ t("sheets.addPanel") }}
          </button>
          <div class="menu-hint">{{ t("sheets.addHint") }}</div>
        </div>
      </template>
    </div>
    <span class="spacer" />
    <span v-if="store.activeSheet" class="kind-badge" :class="{ panel: isPanelSheet(store.activeSheet) }">
      {{ t("sheets.kindRow") }}: {{ kindLabel }}
    </span>
  </div>
</template>

<style scoped>
.file-tabs {
  display: flex;
  align-items: flex-end;
  gap: 2px;
  height: 26px;
  padding: 0 6px;
  background: var(--ribbon-strip);
  user-select: none;
  flex: none;
}
.tab {
  display: flex;
  align-items: center;
  gap: 6px;
  border: 1px solid var(--ribbon-line);
  border-bottom: none;
  background: #e9ecef;
  color: var(--ui-muted);
  font-size: 11px;
  padding: 4px 10px;
  border-radius: 4px 4px 0 0;
  cursor: pointer;
}
.tab.active {
  background: var(--model-bg);
  color: #e8eaec;
  border-color: var(--model-bg);
}
.kind-icon { color: var(--ribbon-icon); flex: none; }
.close { color: var(--ui-placeholder); }
.close:hover { color: #fff; }
.spacer { flex: 1; }
/* シート種別 (デザイン「シート種別: 盤レイアウト」) */
.kind-badge {
  align-self: center;
  padding: 1px 8px;
  border-radius: 999px;
  font-size: 10px;
  color: var(--ui-muted);
}
.kind-badge.panel {
  background: var(--sel-blue);
  color: var(--acad-blue);
  font-weight: 600;
}
.add-wrap { position: relative; display: flex; }
.add {
  width: 22px;
  height: 22px;
  margin-bottom: 1px;
  border: none;
  background: transparent;
  color: var(--ui-muted);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  border-radius: 4px;
}
.add:hover { background: var(--hover-bg); }
.backdrop { position: fixed; inset: 0; z-index: 20; }
/* 種別の選択ポップオーバー (2階層目のモーダルを作らない) */
.add-menu {
  position: absolute;
  top: calc(100% + 2px);
  left: 0;
  z-index: 21;
  width: 190px;
  padding: 4px 0 6px;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 6px;
  box-shadow: var(--shadow-popup, 0 6px 18px rgba(0, 0, 0, 0.18));
}
.menu-head {
  padding: 4px 10px;
  font-size: 10px;
  font-weight: 600;
  color: var(--ui-placeholder);
}
.menu-row {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  border: none;
  background: transparent;
  text-align: left;
  padding: 5px 10px;
  font-size: 11px;
  color: var(--ui-text);
  cursor: pointer;
}
.menu-row:hover { background: var(--hover-bg); }
.menu-icon { color: var(--ribbon-icon); }
.menu-hint {
  padding: 4px 10px 0;
  font-size: 10px;
  color: var(--ui-muted);
}
</style>
