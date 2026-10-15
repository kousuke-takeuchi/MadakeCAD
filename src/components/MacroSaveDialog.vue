<script setup lang="ts">
// 回路マクロの保存ダイアログ (.pen「M4デザイン - 回路マクロ」左半分)。
// CAD調モーダル幅520: palette-headヘッダ「回路マクロとして保存」/
// フォーム行 (名前・カテゴリ・基準点・バリアント) + 選択範囲のプレビュー + 保存先の案内 /
// フッタ=キャンセル+保存。
//
// 基準点は選択範囲の左下ピンからRust側が自動で決めるので、ここでは表示だけ。
// 名前欄の候補から既存マクロを選ぶと、そのマクロへバリアントを足す (.pen「M4残りデザイン①」
// `TSvHz`: 候補リスト・バリアントチップ 登録済/選択/空き・比較プレビュー・上書き確認)。
// 文字列はすべてi18nカタログ経由 (docs/internal/specs/i18n.md)。
import { computed, watch } from "vue";
import { useI18n } from "vue-i18n";
import { TriangleAlert, X } from "lucide-vue-next";
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

/** 欄の表示名 (`value`=型番・値、`attrs.<名前>`は属性名そのまま)。 */
function fieldLabel(field: string): string {
  return field === "value" ? t("macros.placeholderFieldValue") : field.replace(/^attrs\./, "");
}

/** 入力欄の現在値 (テンプレートから型付きで読むための小さなヘルパ)。 */
function inputValue(event: Event): string {
  return (event.target as HTMLInputElement).value;
}

// 名前欄の候補は一覧から出すので、開いたときに一覧が無ければ読む
watch(
  () => macros.saveOpen,
  (open) => {
    if (open && macros.macros.length === 0) void macros.load();
  },
  { immediate: true },
);

/** 名前を打ち直したら既存マクロの選択をやめる (新しいマクロとして保存する)。 */
function onNameInput() {
  if (macros.saveTargetId) macros.clearExisting();
}

/** 既存マクロ候補の右側の説明 (登録済みのキーと分類)。 */
function matchNote(m: (typeof macros.macros)[number]): string {
  const keys = ["A", ...m.variants.map((v) => v.key)].join(" ");
  return t("macros.existingNote", { keys, category: m.category || t("macros.uncategorized") });
}

const saveLabel = computed(() => {
  if (macros.saving) return t("macros.saving");
  if (!macros.saveTarget) return t("macros.save");
  return t("macros.saveAsVariant", { key: macros.saveVariant });
});

async function save(confirm = false) {
  const result = await macros.save(confirm);
  if (!result && macros.overwriteAsk) return;
  report(result);
}

async function saveToFree() {
  report(await macros.saveToFreeVariant());
}

function report(result: Awaited<ReturnType<typeof macros.save>>) {
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
            :class="{ target: macros.saveTarget }"
            :placeholder="t('macros.namePlaceholder')"
            autofocus
            @input="onNameInput"
          />
        </label>
        <ul v-if="macros.nameMatches.length" class="matches">
          <li v-for="m in macros.nameMatches" :key="m.id">
            <button type="button" class="match" @mousedown.prevent @click="macros.chooseExisting(m.id)">
              <span class="match-name">{{ m.name_ja || m.name }}</span>
              <span class="match-note">{{ matchNote(m) }}</span>
            </button>
          </li>
        </ul>
        <label class="row">
          <span class="label">{{ t("macros.categoryRow") }}</span>
          <input
            v-model="macros.saveCategory"
            class="input"
            :disabled="!!macros.saveTarget"
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
            <span v-if="!macros.saveTarget" class="chip current">{{ t("macros.variantDefault") }}</span>
            <template v-else>
              <button
                v-for="c in macros.saveVariantChips"
                :key="c.key"
                class="chip variant"
                :class="c.state"
                :title="c.state === 'saved' ? t('macros.variantSavedHint') : ''"
                @click="macros.chooseVariant(c.key)"
              >
                {{ c.state === "saved" || c.overwrite ? `${c.key} ✓` : c.state === "selected" ? t("macros.variantNew", { key: c.key }) : c.key }}
              </button>
            </template>
          </span>
        </div>
        <p v-if="macros.saveTarget" class="hint">{{ t("macros.variantExistingHint", { key: macros.saveVariant }) }}</p>

        <!-- 既存マクロへのバリアント追加では、プレースホルダと値セットはマクロのものを引き継ぐ -->
        <template v-if="!macros.saveTarget">
        <!-- プレースホルダ: 現場ごとに決める欄にキー名を付ける (v1=表形式) -->
        <p class="section">{{ t("macros.placeholderSection") }}</p>
        <p class="hint">{{ t("macros.placeholderHint") }}</p>
        <table v-if="macros.placeholderCandidates.length" class="grid">
          <thead>
            <tr>
              <th>{{ t("macros.placeholderColTarget") }}</th>
              <th>{{ t("macros.placeholderColField") }}</th>
              <th>{{ t("macros.placeholderColCurrent") }}</th>
              <th>{{ t("macros.placeholderColKey") }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="row in macros.placeholderCandidates" :key="row.id">
              <td class="mono">{{ row.reference || row.symbolId }}</td>
              <td>{{ fieldLabel(row.field) }}</td>
              <td class="muted">{{ row.current }}</td>
              <td>
                <input
                  class="input cell"
                  :value="macros.placeholderKeys[row.id] ?? ''"
                  :placeholder="t('macros.placeholderKeyPlaceholder')"
                  @input="macros.setPlaceholderKey(row.id, inputValue($event))"
                />
              </td>
            </tr>
          </tbody>
        </table>
        <p v-else class="hint">{{ t("macros.placeholderEmpty") }}</p>

        <!-- 値セット: キーごとの値の組 (挿入時に選ぶと一括設定される) -->
        <p class="section">{{ t("macros.valueSetSection") }}</p>
        <table v-if="macros.placeholderKeyList.length" class="grid">
          <thead>
            <tr>
              <th>{{ t("macros.valueSetColName") }}</th>
              <th v-for="key in macros.placeholderKeyList" :key="key" class="mono">{{ key }}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            <tr v-for="(draft, i) in macros.valueSetDrafts" :key="i">
              <td>
                <input
                  class="input cell"
                  :value="draft.label"
                  :placeholder="t('macros.valueSetNamePlaceholder')"
                  @input="macros.setValueSetLabel(i, inputValue($event))"
                />
              </td>
              <td v-for="key in macros.placeholderKeyList" :key="key">
                <input
                  class="input cell"
                  :value="draft.values[key] ?? ''"
                  @input="macros.setValueSetValue(i, key, inputValue($event))"
                />
              </td>
              <td>
                <button class="link" :title="t('macros.valueSetRemove')" @click="macros.removeValueSet(i)">
                  <X :size="12" />
                </button>
              </td>
            </tr>
          </tbody>
        </table>
        <p v-else class="hint">{{ t("macros.valueSetNeedsKeys") }}</p>
        <button
          v-if="macros.placeholderKeyList.length"
          class="chip add-set"
          @click="macros.addValueSet()"
        >
          {{ t("macros.valueSetAdd") }}
        </button>

        </template>
        <div v-if="macros.saveTarget" class="compare">
          <div class="compare-col">
            <span class="compare-label">{{ t("macros.compareSaved") }}</span>
            <div class="preview-frame small">
              <MacroPreview :macro="macros.saveTarget" :width="214" :height="110" />
            </div>
          </div>
          <div class="compare-col">
            <span class="compare-label current">{{ t("macros.compareCurrent", { key: macros.saveVariant }) }}</span>
            <div class="preview-frame small current">
              <MacroPreview v-if="macros.savePreview" :macro="macros.savePreview" :width="214" :height="110" />
            </div>
          </div>
        </div>
        <div v-else class="preview-frame">
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
        <button class="btn primary" :disabled="!macros.canSave" @click="save()">
          {{ saveLabel }}
        </button>
      </div>

      <!-- 上書き確認 (登録済みのバリアントを選んで保存したとき) -->
      <div v-if="macros.overwriteAsk" class="confirm-scrim">
        <div class="confirm">
          <div class="titlebar">
            <span class="title">{{ t("macros.overwriteTitle", { key: macros.saveVariant }) }}</span>
          </div>
          <div class="confirm-body">
            <TriangleAlert :size="14" class="warn-icon" />
            <span>{{ t("macros.overwriteBody", { name: macros.saveTarget?.name_ja || macros.saveTarget?.name || "", key: macros.saveVariant, count: macros.saveEntityCount }) }}</span>
          </div>
          <div class="footer">
            <button v-if="macros.saveTarget && macros.saveVariantChips.some((c) => c.state === 'free')" class="btn secondary" @click="saveToFree">
              {{ t("macros.saveToFree", { key: macros.saveVariantChips.find((c) => c.state === "free")?.key ?? "" }) }}
            </button>
            <button class="btn primary" @click="save(true)">{{ t("macros.overwrite") }}</button>
            <button class="btn secondary" @click="macros.overwriteAsk = false">{{ t("macros.cancel") }}</button>
          </div>
        </div>
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
/* DSバリアントチップ 登録済/選択/空き (.pen dfMl4 / h7DZn / UhfCL) */
.chip.variant {
  font-family: var(--mono-font);
  font-size: 10px;
  padding: 3px 8px;
  cursor: pointer;
}
.chip.variant.saved {
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  color: var(--ui-muted);
  font-weight: 400;
}
.chip.variant.selected {
  background: var(--sel-blue);
  border: 1px solid var(--acad-blue);
  color: var(--ui-text);
}
.chip.variant.free {
  background: var(--input-bg);
  border: 1px solid var(--ribbon-line);
  color: var(--ui-placeholder);
  font-weight: 400;
}
.input.target { border-color: var(--acad-blue); }
.matches {
  list-style: none;
  margin: -4px 0 0 84px;
  padding: 4px 0;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
}
.match {
  width: 100%;
  display: flex;
  justify-content: space-between;
  gap: 8px;
  padding: 4px 10px;
  border: none;
  background: transparent;
  text-align: left;
  cursor: pointer;
}
.match:hover, .match:focus-visible { background: var(--sel-blue); outline: none; }
.match-name { font-size: 11px; font-weight: 600; color: var(--ui-text); }
.match-note { font-size: 10px; color: var(--ui-muted); }
.compare { display: flex; gap: 8px; }
.compare-col { flex: 1; display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.compare-label { font-size: 10px; font-weight: 600; color: var(--ui-muted); }
.compare-label.current { color: var(--acad-blue); }
.preview-frame.small { height: 110px; }
.preview-frame.current { border-color: var(--acad-blue); }
.dialog { position: relative; }
.confirm-scrim {
  position: absolute;
  inset: 0;
  background: var(--scrim);
  display: flex;
  align-items: center;
  justify-content: center;
}
.confirm {
  width: min(420px, 90%);
  background: var(--ribbon-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 8px;
  box-shadow: var(--shadow-panel-lg);
  overflow: hidden;
}
.confirm-body {
  display: flex;
  gap: 8px;
  margin: 14px;
  padding: 8px 10px;
  background: var(--warn-bg);
  border: 1px solid var(--warn-fg);
  border-radius: 4px;
  font-size: 11px;
  color: var(--ui-text);
}
.warn-icon { color: var(--warn-fg); flex: none; }
.section {
  margin: 6px 0 0;
  font-size: 11px;
  font-weight: 600;
  color: var(--ui-text);
}
.grid {
  width: 100%;
  border-collapse: collapse;
  font-size: 10px;
  color: var(--ui-text);
}
.grid th {
  text-align: left;
  font-weight: 600;
  color: var(--ui-muted);
  padding: 2px 4px;
  border-bottom: 1px solid var(--ribbon-line);
  white-space: nowrap;
}
.grid td {
  padding: 2px 4px;
  border-bottom: 1px solid var(--ribbon-line);
  vertical-align: middle;
}
.grid .mono { font-family: monospace; }
.grid .muted { color: var(--ui-muted); }
.input.cell { padding: 3px 6px; font-size: 10px; width: 100%; }
.link {
  border: none;
  background: transparent;
  color: var(--ui-muted);
  display: flex;
  padding: 0;
  cursor: pointer;
}
.chip.add-set {
  align-self: flex-start;
  background: transparent;
  border: 1px dashed var(--ribbon-line);
  color: var(--ui-text);
  cursor: pointer;
}
.chip.add-set:hover { background: var(--hover-bg); }
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
