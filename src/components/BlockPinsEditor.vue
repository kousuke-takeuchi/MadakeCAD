<script setup lang="ts">
// ブロック(汎用機器)のピン表エディタ。
// デザイン: MadakeCAD.pen「フェーズ5 - 部品挿入 ブロック(ピン表エディタ)」
// (部品挿入ダイアログの極数バーの発展形: 左右のピン行 + サイズ自動/手動 + 実描画プレビュー)。
//
// 挿入パネル (部品挿入ダイアログ) と編集ダイアログの両方がこの1枚を使う。
// 状態は stores/blockEditor.ts の下書きだけで、図面はここからは触らない。
import { ChevronDown, ChevronRight, Minus, Plus } from "lucide-vue-next";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { BLOCK_EXTRA_SIDES, type BlockIssue, type BlockSide } from "../canvas/blockEditor";
import { BLOCK_SIZE_STEP_MM } from "../canvas/dynamicSymbol";
import { useBlockEditorStore } from "../stores/blockEditor";
import BlockPinColumn from "./BlockPinColumn.vue";
import SymbolPreview from "./SymbolPreview.vue";

const editor = useBlockEditorStore();
const { t } = useI18n();

/** 主辺 (常に出す)。上下辺は折りたたみの中。 */
const mainSides: BlockSide[] = ["left", "right"];

/** 指摘の表示文 (指摘の種類ごとに必要な値を差し込む)。 */
function issueText(issue: BlockIssue): string {
  if (issue.code === "duplicate") return t("blockEditor.issue.duplicate", { no: issue.no });
  if (issue.code === "emptyNo") {
    return t("blockEditor.issue.emptyNo", {
      side: t(`blockEditor.side.${issue.side}`),
      row: issue.index + 1,
    });
  }
  if (issue.code === "tooMany") {
    return t("blockEditor.issue.tooMany", { count: issue.count, max: issue.max });
  }
  if (issue.code === "contactsTooFew") {
    return t("blockEditor.issue.contactsTooFew", { side: t(`blockEditor.side.${issue.side}`), row: issue.index + 1 });
  }
  if (issue.code === "duplicateContact") {
    return t("blockEditor.issue.duplicateContact", {
      side: t(`blockEditor.side.${issue.side}`),
      row: issue.index + 1,
      contact: issue.contact,
    });
  }
  if (issue.code === "tooManyContacts") {
    return t("blockEditor.issue.tooManyContacts", {
      side: t(`blockEditor.side.${issue.side}`),
      row: issue.index + 1,
      count: issue.count,
      max: issue.max,
    });
  }
  if (issue.code === "connectorRefNotGrouped") {
    return t("blockEditor.issue.connectorRefNotGrouped", {
      side: t(`blockEditor.side.${issue.side}`),
      row: issue.index + 1,
    });
  }
  if (issue.code === "connectorRefSplit") return t("blockEditor.issue.connectorRefSplit", { ref: issue.ref });
  return t("blockEditor.issue.sizeTooSmall", { w: issue.w, h: issue.h });
}

/** 同じ文面は1回だけ出す (まとめ名が3か所に割れると同じ警告が2つ出るため)。 */
const issueTexts = computed(() => [...new Set(editor.issues.map(issueText))]);

/** 折りたたみの中の上下辺に入っているピンの本数。 */
const extraCount = computed(() =>
  BLOCK_EXTRA_SIDES.reduce((n, side) => n + (editor.draft?.rows[side].length ?? 0), 0),
);

/** 外形寸法は5mm刻み (2.5mmグリッドに載るように)。 */
function setSize(key: "w" | "h", value: number) {
  const steps = Math.round((Number(value) || 0) / BLOCK_SIZE_STEP_MM);
  editor.setSize({ [key]: Math.max(BLOCK_SIZE_STEP_MM, steps * BLOCK_SIZE_STEP_MM) });
}
</script>

<template>
  <div v-if="editor.draft" class="pins-editor">
    <div class="head">
      <span class="title">{{ t("blockEditor.pinTable") }}</span>
      <span class="hint">{{ t("blockEditor.pinTableHint") }}</span>
      <span class="spacer" />
      <span class="size-row">
        <span class="size-label">{{ t("blockEditor.size") }}</span>
        <label class="radio">
          <input type="radio" :checked="editor.sizeMode === 'auto'" @change="editor.setSizeMode('auto')" />
          {{ t("blockEditor.sizeAuto", { w: editor.autoSize.w, h: editor.autoSize.h }) }}
        </label>
        <label class="radio">
          <input type="radio" :checked="editor.sizeMode === 'manual'" @change="editor.setSizeMode('manual')" />
          {{ t("blockEditor.sizeManual") }}
        </label>
        <span class="size-steppers">
          <span
            v-for="key in (['w', 'h'] as const)"
            :key="key"
            class="stepper"
            :class="{ off: editor.sizeMode === 'auto' }"
            :title="t(key === 'w' ? 'blockEditor.width' : 'blockEditor.height')"
          >
            <button
              class="step"
              :disabled="editor.sizeMode === 'auto'"
              @click="setSize(key, editor.size[key] - BLOCK_SIZE_STEP_MM)"
            >
              <Minus :size="11" />
            </button>
            <input
              class="step-value"
              :value="editor.size[key]"
              :disabled="editor.sizeMode === 'auto'"
              @change="setSize(key, Number(($event.target as HTMLInputElement).value))"
            />
            <button
              class="step"
              :disabled="editor.sizeMode === 'auto'"
              @click="setSize(key, editor.size[key] + BLOCK_SIZE_STEP_MM)"
            >
              <Plus :size="11" />
            </button>
          </span>
        </span>
      </span>
    </div>

    <div class="body">
      <BlockPinColumn v-for="side in mainSides" :key="side" :side="side" />
      <div class="preview">
        <SymbolPreview v-if="editor.previewDef" :def="editor.previewDef" :width="180" :height="160" />
        <span v-else class="hint">{{ t("blockEditor.previewUnavailable") }}</span>
      </div>
    </div>

    <button class="extra-toggle" @click="editor.toggleExtraSides()">
      <ChevronDown v-if="editor.extraSidesOpen" :size="12" />
      <ChevronRight v-else :size="12" />
      {{ t("blockEditor.extraSides", { count: extraCount }) }}
      <span class="hint">{{ t("blockEditor.extraSidesHint") }}</span>
    </button>
    <div v-if="editor.extraSidesOpen" class="body">
      <BlockPinColumn v-for="side in BLOCK_EXTRA_SIDES" :key="side" :side="side" />
      <span class="extra-spacer" />
    </div>

    <p v-for="text in issueTexts" :key="text" class="issue">{{ text }}</p>
    <p v-if="editor.warnings.length" class="warning">
      {{ t("blockEditor.wiredWarning", { pins: editor.warnings.join(", ") }) }}
    </p>
  </div>
</template>

<style scoped>
.pins-editor {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 12px;
  background: var(--palette-bg);
  border-top: 1px solid var(--ribbon-line);
}
.head {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.title { font-size: 12px; font-weight: 600; color: var(--ui-text); }
.hint { font-size: 10px; color: var(--ui-muted); }
.spacer { flex: 1; }
.size-label { font-size: 10px; color: var(--ui-muted); }
.size-row { display: flex; align-items: center; gap: 8px; margin-left: auto; }
.size-steppers { display: flex; align-items: center; gap: 6px; }
.radio {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: var(--ui-text);
  cursor: pointer;
}
.radio input { accent-color: var(--acad-blue); margin: 0; }
.stepper {
  display: flex;
  align-items: center;
  background: var(--input-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 8px;
  overflow: hidden;
}
.stepper.off { opacity: 0.5; }
.step {
  width: 22px;
  height: 24px;
  border: none;
  background: transparent;
  color: var(--ui-muted);
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}
.step:hover:not(:disabled) { background: var(--hover-bg); }
.step-value {
  width: 38px;
  height: 24px;
  border: none;
  border-left: 1px solid var(--ribbon-line);
  border-right: 1px solid var(--ribbon-line);
  background: var(--card-bg);
  text-align: center;
  font-family: var(--mono-font);
  font-size: 11px;
  font-weight: 600;
  color: var(--ui-text);
  outline: none;
}
.body {
  display: flex;
  align-items: flex-start;
  gap: 10px;
}
.preview {
  flex: none;
  width: 188px;
  height: 168px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  overflow: hidden;
}
.extra-spacer { flex: none; width: 188px; }
.extra-toggle {
  align-self: flex-start;
  display: flex;
  align-items: center;
  gap: 6px;
  border: none;
  background: transparent;
  padding: 0;
  font-size: 10px;
  font-weight: 600;
  color: var(--ui-text);
  cursor: pointer;
}
.issue { margin: 0; font-size: 10px; line-height: 15px; color: var(--err-fg); }
.warning { margin: 0; font-size: 10px; line-height: 15px; color: var(--warn-fg); }
</style>
