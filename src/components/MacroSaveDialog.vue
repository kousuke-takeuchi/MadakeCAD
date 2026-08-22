<script setup lang="ts">
// 回路マクロの保存ダイアログ (.pen「M4デザイン - 回路マクロ」左半分)。
// CAD調モーダル幅520: palette-headヘッダ「回路マクロとして保存」/
// フォーム行 (名前・カテゴリ・基準点・バリアント) + 選択範囲のプレビュー + 保存先の案内 /
// フッタ=キャンセル+保存。
//
// 基準点は選択範囲の左下ピンからRust側が自動で決めるので、ここでは表示だけ。
// バリアントの追加はフェーズ3のため、無効チップ+titleでフェーズを示す。
// 文字列はすべてi18nカタログ経由 (docs/internal/specs/i18n.md)。
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { X } from "lucide-vue-next";
import { useMacrosStore } from "../stores/macros";
import { useUiStore } from "../stores/ui";
import MacroPreview from "./MacroPreview.vue";

const macros = useMacrosStore();
const ui = useUiStore();
const { t } = useI18n();

/** 基準点の表示 (mm、小数1桁)。組み立て前は「選択範囲から自動」と出す。 */
const basePoint = computed(() => {
  const p = macros.savePreview?.base_point;
  return p
    ? t("macros.basePointAuto", { x: p.x.toFixed(1), y: p.y.toFixed(1) })
    : t("macros.basePointPending");
});

async function save() {
  const result = await macros.save();
  if (!result) {
    if (macros.error) ui.log(macros.error);
    return;
  }
  ui.log(t("macros.savedLog", { name: result.macro.name, path: result.path }));
}
</script>

<template>
  <div v-if="macros.saveOpen" class="overlay" @click.self="macros.cancelSave()">
    <div class="dialog">
      <div class="titlebar">
        <span class="title">{{ t("macros.saveTitle") }}</span>
        <button class="close" :title="t('macros.close')" @click="macros.cancelSave()">
          <X :size="14" />
        </button>
      </div>

      <div class="body">
        <label class="row">
          <span class="label">{{ t("macros.nameRow") }}</span>
          <input
            v-model="macros.saveName"
            class="input"
            :placeholder="t('macros.namePlaceholder')"
            autofocus
          />
        </label>
        <label class="row">
          <span class="label">{{ t("macros.categoryRow") }}</span>
          <input
            v-model="macros.saveCategory"
            class="input"
            :placeholder="t('macros.categoryPlaceholder')"
            list="macro-categories"
          />
          <datalist id="macro-categories">
            <option v-for="c in macros.categories" :key="c.key" :value="c.key" />
          </datalist>
        </label>
        <div class="row">
          <span class="label">{{ t("macros.basePointRow") }}</span>
          <span class="readonly">{{ basePoint }}</span>
        </div>
        <div class="row">
          <span class="label">{{ t("macros.variantRow") }}</span>
          <span class="variants">
            <span class="chip current">{{ t("macros.variantDefault") }}</span>
            <button class="chip add" disabled :title="t('macros.variantAddHint')">
              {{ t("macros.variantAdd") }}
            </button>
          </span>
        </div>

        <div class="preview-frame">
          <MacroPreview
            v-if="macros.savePreview"
            :macro="macros.savePreview"
            :width="440"
            :height="180"
          />
        </div>
        <p class="caption">{{ t("macros.previewCount", { count: macros.saveEntityCount }) }}</p>
        <p class="hint">
          {{ t("macros.saveHint", { path: macros.user_dir ?? "~/MadakeCAD/macros" }) }}
        </p>
        <p v-if="macros.error" class="error">{{ macros.error }}</p>
      </div>

      <div class="footer">
        <button class="btn secondary" @click="macros.cancelSave()">
          {{ t("macros.cancel") }}
        </button>
        <button class="btn primary" :disabled="!macros.canSave" @click="save">
          {{ macros.saving ? t("macros.saving") : t("macros.save") }}
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
  z-index: 100;
}
.dialog {
  width: min(520px, 94vw);
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
  gap: 8px;
  padding: 16px;
  overflow-y: auto;
}
.row {
  display: flex;
  align-items: center;
  gap: 10px;
}
.label {
  flex: none;
  width: 74px;
  font-size: 11px;
  color: var(--ui-text);
}
.input {
  flex: 1;
  min-width: 0;
  background: var(--input-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  padding: 5px 8px;
  font-size: 11px;
  color: var(--ui-text);
  outline: none;
}
.input:focus { border-color: var(--acad-blue); }
.readonly { font-size: 11px; color: var(--ui-muted); }
.variants { display: flex; align-items: center; gap: 6px; }
.chip {
  border-radius: 4px;
  padding: 3px 9px;
  font-size: 11px;
  font-weight: 600;
}
.chip.current {
  background: var(--sel-blue);
  border: 1px solid var(--acad-blue);
  color: var(--ui-text);
}
.chip.add {
  background: transparent;
  border: 1px dashed var(--ribbon-line);
  color: var(--ui-muted);
  cursor: default;
  opacity: 0.6;
}
.preview-frame {
  height: 180px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  overflow: hidden;
}
.caption, .hint { margin: 0; font-size: 10px; line-height: 15px; color: var(--ui-muted); }
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
