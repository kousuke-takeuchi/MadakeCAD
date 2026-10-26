<script setup lang="ts">
// ブロックのピン表の1辺ぶん (行の追加・並べ替え・削除)。
// デザイン: MadakeCAD.pen「フェーズ5 - 部品挿入 ブロック(ピン表エディタ)」のBP列。
import { ChevronDown, ChevronUp, Plus, X } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import type { BlockPinRow, BlockSide } from "../canvas/blockEditor";
import { CONNECTOR_KINDS, isGroupedConnector, type ConnectorKind } from "../canvas/connector";
import { useBlockEditorStore } from "../stores/blockEditor";

const props = defineProps<{ side: BlockSide }>();
const editor = useBlockEditorStore();
const { t } = useI18n();

function edit(index: number, patch: Partial<BlockPinRow>) {
  editor.updateRow(props.side, index, patch);
}

/** まとめ名を書けるのは端子台・JST の行だけ。 */
function refEnabled(row: BlockPinRow): boolean {
  return !!row.connector && isGroupedConnector(row.connector);
}
</script>

<template>
  <div v-if="editor.draft" class="col">
    <div class="col-head">
      <span class="col-name">{{ t(`blockEditor.side.${side}`) }}</span>
      <span class="col-count">({{ editor.draft.rows[side].length }})</span>
    </div>
    <div class="col-labels">
      <span class="lbl no">{{ t("blockEditor.colNo") }}</span>
      <span class="lbl">{{ t("blockEditor.colName") }}</span>
      <span class="lbl">{{ t("blockEditor.colContacts") }}</span>
      <span class="lbl">{{ t("blockEditor.colConnector") }}</span>
      <span class="lbl ref">{{ t("blockEditor.colConnectorRef") }}</span>
    </div>
    <div v-for="(row, i) in editor.draft.rows[side]" :key="`${side}-${i}`" class="row">
      <span class="row-no">{{ i + 1 }}</span>
      <input
        class="cell mono"
        :value="row.no"
        :aria-label="t('blockEditor.colNo')"
        spellcheck="false"
        @input="edit(i, { no: ($event.target as HTMLInputElement).value })"
      />
      <input
        class="cell"
        :value="row.name"
        :aria-label="t('blockEditor.colName')"
        :placeholder="t('blockEditor.namePlaceholder')"
        @input="edit(i, { name: ($event.target as HTMLInputElement).value })"
      />
      <input
        class="cell"
        :value="row.contacts ?? ''"
        :aria-label="t('blockEditor.colContacts')"
        :placeholder="t('blockEditor.contactsPlaceholder')"
        spellcheck="false"
        @input="edit(i, { contacts: ($event.target as HTMLInputElement).value })"
      />
      <select
        class="cell"
        :value="row.connector ?? ''"
        :aria-label="t('blockEditor.colConnector')"
        @change="edit(i, { connector: ($event.target as HTMLSelectElement).value as ConnectorKind | '' })"
      >
        <option value="">{{ t("blockEditor.connectorNone") }}</option>
        <option v-for="kind in CONNECTOR_KINDS" :key="kind" :value="kind">
          {{ t(`blockEditor.connector.${kind}`) }}
        </option>
      </select>
      <input
        class="cell ref mono"
        :value="row.connectorRef ?? ''"
        :aria-label="t('blockEditor.colConnectorRef')"
        :placeholder="t('blockEditor.connectorRefPlaceholder')"
        :disabled="!refEnabled(row)"
        spellcheck="false"
        @input="edit(i, { connectorRef: ($event.target as HTMLInputElement).value })"
      />
      <button class="icon-btn" :title="t('blockEditor.moveUp')" @click="editor.moveRow(side, i, -1)">
        <ChevronUp :size="11" />
      </button>
      <button class="icon-btn" :title="t('blockEditor.moveDown')" @click="editor.moveRow(side, i, 1)">
        <ChevronDown :size="11" />
      </button>
      <button class="icon-btn" :title="t('blockEditor.removePin')" @click="editor.removeRow(side, i)">
        <X :size="11" />
      </button>
    </div>
    <button class="add" @click="editor.addRow(side)">
      <Plus :size="11" class="add-icon" />
      {{ t("blockEditor.addPin") }}
    </button>
  </div>
</template>

<style scoped>
.col {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.col-head { display: flex; align-items: center; gap: 6px; }
.col-name { font-size: 10px; font-weight: 600; color: var(--ui-text); }
.col-count { font-size: 10px; color: var(--ui-muted); }
.col-labels { display: flex; gap: 4px; padding-left: 18px; }
.lbl { flex: 1; font-size: 10px; color: var(--ui-placeholder); }
.lbl.no { flex: none; width: 58px; }
.lbl.ref { flex: none; width: 52px; }
.row { display: flex; align-items: center; gap: 4px; }
.row-no { width: 14px; flex: none; font-size: 10px; color: var(--ui-muted); }
.cell {
  flex: 1;
  min-width: 0;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  padding: 4px 6px;
  font-size: 11px;
  color: var(--ui-text);
  outline: none;
}
.cell.mono { flex: none; width: 58px; font-family: var(--mono-font); }
.cell.ref { width: 52px; }
.cell:disabled { opacity: 0.5; }
.cell:focus { border-color: var(--acad-blue); }
.icon-btn {
  width: 20px;
  height: 20px;
  flex: none;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  color: var(--ui-muted);
  cursor: pointer;
}
.icon-btn:hover { background: var(--hover-bg); }
.add {
  align-self: flex-start;
  display: flex;
  align-items: center;
  gap: 4px;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  padding: 4px 9px;
  font-size: 10px;
  color: var(--ui-text);
  cursor: pointer;
}
.add:hover { background: var(--hover-bg); }
.add-icon { color: var(--acad-blue); }
</style>
