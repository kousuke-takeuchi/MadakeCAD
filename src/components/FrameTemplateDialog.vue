<script setup lang="ts">
// 図枠テンプレート選択ダイアログ (.pen「M4デザイン - 図枠テンプレート」)。
// CAD調モーダル幅1120: palette-headヘッダ「図枠テンプレート」/
// 左=テンプレ一覧 (名前+ソースmono、選択=sel-blue、「+ JSONを読み込み…」) /
// 中=ライブプレビュー (図面と同じレンダラ。空欄には青い`{バインド}`を重ねる) +
//    情報行 + 適用範囲ラジオ (このプロジェクト / グローバル既定) /
// 右=フィールドバインド表 + ゾーン設定 + 一括適用の説明。
// フッタ=OK / キャンセル / 適用 (適用は閉じずに反映する)。
//
// 「このプロジェクト」は set_frame_template コマンド1回なので、Cmd+Z一発で戻せる。
// 「グローバル既定」は図面を変えず、新しく作る図面の初期値として設定へ保存する。
// 文字列はすべてi18nカタログ経由 (docs/internal/specs/i18n.md)。
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { FolderPlus, X } from "lucide-vue-next";
import { bindNames, frameTemplateName } from "../canvas/frameTemplate";
import { useDocumentStore } from "../stores/document";
import { useFrameTemplatesStore, type FrameApplyResult } from "../stores/frameTemplates";
import { useUiStore } from "../stores/ui";
import FrameTemplatePreview from "./FrameTemplatePreview.vue";

const frames = useFrameTemplatesStore();
const doc = useDocumentStore();
const ui = useUiStore();
const { t, te, locale } = useI18n();

/** プレビューの対象シート (表示中のシートの用紙・表題欄・改訂欄で描く)。 */
const sheet = computed(() => doc.activeSheet);
const bind = computed(() => (sheet.value ? doc.bindContextFor(sheet.value) : null));

/** 右の「フィールドバインド」表 (テンプレートが使っているバインドだけを並べる)。 */
const binds = computed(() =>
  (frames.selected ? bindNames(frames.selected) : []).map((name) => ({
    name,
    // 表題欄に書くのと同じ形 (テンプレートJSONへ写せる)
    token: `{${name}}`,
    label: te(`frameTemplates.bind.${name}`) ? t(`frameTemplates.bind.${name}`) : name,
  })),
);

function cancel() {
  frames.cancel();
}

/** ユーザー図枠の置き場をファイラで開く (開けない環境ではパスを案内する)。 */
async function addUserFrame() {
  const { path, opened } = await frames.openUserFolder();
  ui.log(t(opened ? "frameTemplates.folderOpenedLog" : "frameTemplates.folderHintLog", { path }));
}

/** 適用結果をコマンドログへ出す (何も変わらなかったときもその旨を出す)。 */
function log(applied: FrameApplyResult | null) {
  if (frames.error) {
    ui.log(frames.error);
    return;
  }
  if (!applied) return;
  const frame = frameTemplateName(applied.template, locale.value);
  if (!applied.changed) ui.log(t("frameTemplates.unchangedLog", { frame }));
  else if (frames.scope === "global") ui.log(t("frameTemplates.defaultSavedLog", { frame }));
  else ui.log(t("frameTemplates.appliedLog", { frame }));
}

async function apply() {
  log(await frames.apply());
}

async function ok() {
  log(await frames.ok());
}
</script>

<template>
  <div v-if="frames.open" class="overlay" @click.self="cancel">
    <div class="dialog">
      <div class="titlebar">
        <span class="title">{{ t("frameTemplates.title") }}</span>
        <button class="close" :title="t('frameTemplates.close')" @click="cancel">
          <X :size="14" />
        </button>
      </div>

      <div class="body">
        <!-- 左: テンプレート一覧 -->
        <div class="list">
          <button
            v-for="row in frames.rows"
            :key="row.template.id"
            class="row"
            :class="{ selected: row.template.id === frames.selectedId }"
            @click="frames.select(row.template.id)"
          >
            <span class="row-name">{{ frameTemplateName(row.template, locale) }}</span>
            <span class="row-source mono">
              {{ t(`frameTemplates.source.${row.source}`) }} · {{ row.template.id }}
              <template v-if="row.current"> · {{ t("frameTemplates.inUse") }}</template>
            </span>
          </button>
          <p v-if="frames.loading" class="caption">{{ t("frameTemplates.loading") }}</p>
          <p v-else-if="!frames.rows.length" class="caption">{{ t("frameTemplates.empty") }}</p>
          <button class="add" @click="addUserFrame">
            <FolderPlus :size="13" />
            {{ t("frameTemplates.addUser") }}
          </button>
        </div>

        <!-- 中央: ライブプレビュー + 情報行 + 適用範囲 -->
        <div class="center">
          <div class="preview-frame">
            <FrameTemplatePreview
              v-if="frames.selected && sheet && bind"
              :key="frames.selected.id"
              :template="frames.selected"
              :sheet="sheet"
              :bind="bind"
              :width="560"
              :height="340"
            />
            <p v-else class="caption">{{ t("frameTemplates.noSheet") }}</p>
          </div>
          <p v-if="frames.summary" class="info mono">
            {{ t("frameTemplates.info", { ...frames.summary }) }}
          </p>
          <div class="form-row">
            <label class="label">{{ t("frameTemplates.scopeLabel") }}</label>
            <div class="choices">
              <label class="choice">
                <input
                  type="radio"
                  value="project"
                  :checked="frames.scope === 'project'"
                  @change="frames.setScope('project')"
                />
                {{ t("frameTemplates.scopeProject") }}
              </label>
              <label class="choice">
                <input
                  type="radio"
                  value="global"
                  :checked="frames.scope === 'global'"
                  @change="frames.setScope('global')"
                />
                {{ t("frameTemplates.scopeGlobal") }}
              </label>
            </div>
          </div>
          <p class="hint">
            {{ frames.scope === "global" ? t("frameTemplates.scopeGlobalHint") : t("frameTemplates.scopeProjectHint") }}
          </p>
          <p v-for="issue in frames.issues" :key="issue.path" class="issue">
            {{ t("frameTemplates.issue", { path: issue.path, message: issue.message }) }}
          </p>
          <p v-if="frames.error" class="error">{{ frames.error }}</p>
        </div>

        <!-- 右: フィールドバインド / ゾーン設定 / 一括適用 -->
        <div class="side">
          <div class="group">
            <div class="group-head"><span>{{ t("frameTemplates.groupBinds") }}</span><i /></div>
            <div v-for="b in binds" :key="b.name" class="bind-row">
              <span class="bind-label">{{ b.label }}</span>
              <span class="bind-value mono">{{ b.token }}</span>
            </div>
            <p v-if="!binds.length" class="caption">{{ t("frameTemplates.noBinds") }}</p>
            <p class="caption">{{ t("frameTemplates.bindsHint") }}</p>
          </div>

          <div class="group">
            <div class="group-head"><span>{{ t("frameTemplates.groupZones") }}</span><i /></div>
            <div class="form-row">
              <label class="label short">{{ t("frameTemplates.zoneCols") }}</label>
              <input
                class="input num mono"
                :value="frames.selected?.zones.cols ?? ''"
                readonly
                :title="t('frameTemplates.zoneReadOnly')"
              />
              <label class="label short">{{ t("frameTemplates.zoneRows") }}</label>
              <input
                class="input num mono"
                :value="frames.selected?.zones.rows ?? ''"
                readonly
                :title="t('frameTemplates.zoneReadOnly')"
              />
            </div>
            <p class="caption">
              {{ t("frameTemplates.zoneLabels", {
                cols: t(`frameTemplates.labelStyle.${frames.selected?.zones.col_labels ?? "numeric"}`),
                rows: t(`frameTemplates.labelStyle.${frames.selected?.zones.row_labels ?? "alpha"}`),
              }) }}
            </p>
            <p class="caption">{{ t("frameTemplates.zoneReadOnly") }}</p>
          </div>

          <div class="group">
            <div class="group-head"><span>{{ t("frameTemplates.groupBulk") }}</span><i /></div>
            <p class="caption">{{ t("frameTemplates.bulkHint") }}</p>
            <p class="caption">{{ t("frameTemplates.undoHint") }}</p>
          </div>
        </div>
      </div>

      <div class="footer">
        <button class="btn primary" :disabled="!frames.selected || frames.running" @click="ok">
          {{ t("frameTemplates.ok") }}
        </button>
        <button class="btn secondary" @click="cancel">{{ t("frameTemplates.cancel") }}</button>
        <button class="btn secondary" :disabled="!frames.selected || frames.running" @click="apply">
          {{ frames.running ? t("frameTemplates.applying") : t("frameTemplates.apply") }}
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
  width: min(1120px, 96vw);
  max-height: 92vh;
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
  gap: 14px;
  padding: 16px;
  overflow-y: auto;
}

/* --- 左: 一覧 --- */
.list {
  flex: none;
  width: 220px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.row {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  text-align: left;
  background: transparent;
  border: 1px solid transparent;
  border-radius: 4px;
  padding: 6px 10px;
  cursor: pointer;
}
.row:hover { background: var(--hover-bg); }
.row.selected {
  background: var(--sel-blue);
  border-color: var(--acad-blue);
}
.row-name { font-size: 12px; font-weight: 600; color: var(--ui-text); }
.row-source { font-size: 10px; color: var(--ui-muted); }
.add {
  display: flex;
  align-items: center;
  gap: 6px;
  background: transparent;
  border: none;
  padding: 6px 10px;
  font-size: 11px;
  font-weight: 600;
  color: var(--acad-blue);
  cursor: pointer;
}
.add:hover { text-decoration: underline; }

/* --- 中央: プレビュー --- */
.center {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.preview-frame {
  height: 340px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  overflow: hidden;
}
.info { margin: 0; font-size: 10px; color: var(--ui-muted); }

/* --- 右: グループボックス --- */
.side {
  flex: none;
  width: 300px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.group { display: flex; flex-direction: column; gap: 6px; }
.group-head {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11px;
  font-weight: 600;
  color: var(--ui-text);
}
.group-head i { flex: 1; height: 1px; background: var(--ribbon-line); }
.bind-row { display: flex; align-items: center; gap: 8px; }
.bind-label { width: 96px; flex: none; font-size: 11px; color: var(--ui-text); }
.bind-value { font-size: 10px; color: var(--acad-blue); }

/* --- 共通のフォーム部品 --- */
.form-row { display: flex; align-items: center; gap: 8px; }
.label { width: 130px; flex: none; font-size: 11px; color: var(--ui-text); }
.label.short { width: auto; }
.input {
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  padding: 5px 8px;
  font-size: 11px;
  color: var(--ui-text);
  outline: none;
}
.input.num { width: 56px; }
.input:read-only { color: var(--ui-muted); background: var(--input-bg); }
.mono { font-family: var(--mono-font); }
.choices { display: flex; gap: 14px; }
.choice { display: flex; align-items: center; gap: 5px; font-size: 11px; color: var(--ui-text); }
.choice input { accent-color: var(--acad-blue); }
.caption, .hint { margin: 0; font-size: 10px; line-height: 15px; color: var(--ui-muted); }
.hint { margin-left: 138px; }
.issue { margin: 0; font-size: 10px; line-height: 15px; color: var(--warn-fg); }
.error { margin: 0; font-size: 11px; color: var(--err-fg); }

/* --- フッタ (OK / キャンセル / 適用) --- */
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
