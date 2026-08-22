<script setup lang="ts">
// テンプレート選択ダイアログ (.pen「M3デザイン - テンプレート選択/検証ループ」)。
// CAD調モーダル幅720: palette-headヘッダ「テンプレートから開始」/
// 左=タイル列 (サムネイル96x54+名前+説明、選択=sel-blue+acad-blue枠) と
// 「+ ユーザーテンプレートを追加...」/ 右=大プレビュー+説明+フッタ。
// 適用はCommand列の一括実行1回なので、Cmd+Z一発で図面が元へ戻る。
// 文字列はすべてi18nカタログ経由 (docs/internal/specs/i18n.md)。
import { useI18n } from "vue-i18n";
import { FolderPlus, X } from "lucide-vue-next";
import { useDocumentStore } from "../stores/document";
import { templateDescription, templateName, useTemplatesStore } from "../stores/templates";
import { useUiStore } from "../stores/ui";
import TemplatePreview from "./TemplatePreview.vue";

const templates = useTemplatesStore();
const doc = useDocumentStore();
const ui = useUiStore();
const { t, locale } = useI18n();

function cancel() {
  templates.cancel();
}

/** ユーザーテンプレートの置き場をファイラで開く (開けない環境ではパスを案内する)。 */
async function addUserTemplate() {
  const { path, opened } = await templates.openUserFolder();
  ui.log(t(opened ? "templates.folderOpenedLog" : "templates.folderHintLog", { path }));
}

async function apply() {
  const sheet = doc.activeSheet;
  if (!sheet) {
    ui.log(t("templates.noSheetLog"));
    return;
  }
  const applied = await templates.apply(sheet.id);
  if (templates.error) {
    ui.log(templates.error);
    return;
  }
  if (!applied) return;
  ui.log(
    t("templates.appliedLog", {
      template: templateName(applied, locale.value),
      sheet: sheet.name,
    }),
  );
}
</script>

<template>
  <div v-if="templates.open" class="overlay" @click.self="cancel">
    <div class="dialog">
      <div class="titlebar">
        <span class="title">{{ t("templates.title") }}</span>
        <button class="close" :title="t('templates.close')" @click="cancel">
          <X :size="14" />
        </button>
      </div>

      <div class="body">
        <div class="tiles">
          <button
            v-for="template in templates.templates"
            :key="template.id"
            class="tile"
            :class="{ selected: template.id === templates.selectedId }"
            @click="templates.select(template.id)"
          >
            <TemplatePreview class="thumb" :template="template" :width="96" :height="54" />
            <span class="tile-text">
              <span class="tile-name">{{ templateName(template, locale) }}</span>
              <span class="tile-desc">{{ templateDescription(template, locale) }}</span>
            </span>
          </button>
          <p v-if="templates.loading" class="caption">{{ t("templates.loading") }}</p>
          <p v-else-if="!templates.templates.length" class="caption">{{ t("templates.empty") }}</p>
          <button class="add" @click="addUserTemplate">
            <FolderPlus :size="13" />
            {{ t("templates.addUser") }}
          </button>
        </div>

        <div class="preview">
          <div class="preview-frame">
            <TemplatePreview
              v-if="templates.selected"
              :key="templates.selected.id"
              :template="templates.selected"
              :width="352"
              :height="198"
            />
          </div>
          <p class="preview-name">
            {{ templates.selected ? templateName(templates.selected, locale) : t("templates.none") }}
          </p>
          <p class="preview-desc">
            {{ templates.selected ? templateDescription(templates.selected, locale) : "" }}
          </p>
          <p class="hint">{{ t("templates.undoHint") }}</p>
          <p v-for="issue in templates.issues" :key="issue.path" class="issue">
            {{ t("templates.issue", { path: issue.path, message: issue.message }) }}
          </p>
          <p v-if="templates.error" class="error">{{ templates.error }}</p>
        </div>
      </div>

      <div class="footer">
        <button class="btn secondary" @click="cancel">{{ t("templates.cancel") }}</button>
        <button
          class="btn primary"
          :disabled="!templates.selected || templates.running"
          @click="apply"
        >
          {{ templates.running ? t("templates.applying") : t("templates.apply") }}
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
  width: min(720px, 94vw);
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
  gap: 14px;
  padding: 16px;
  overflow-y: auto;
}
.tiles {
  flex: none;
  width: 300px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.tile {
  display: flex;
  align-items: center;
  gap: 10px;
  text-align: left;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  padding: 6px;
  cursor: pointer;
}
.tile:hover { background: var(--hover-bg); }
.tile.selected {
  background: var(--sel-blue);
  border: 1.5px solid var(--acad-blue);
  padding: 5.5px;
}
.thumb {
  flex: none;
  width: 96px;
  height: 54px;
  border-radius: 3px;
}
.tile-text { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
.tile-name { font-size: 12px; font-weight: 600; color: var(--ui-text); }
.tile-desc {
  font-size: 10px;
  line-height: 14px;
  color: var(--ui-muted);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.add {
  display: flex;
  align-items: center;
  gap: 6px;
  background: transparent;
  border: 1px dashed var(--ribbon-line);
  border-radius: 4px;
  padding: 6px 8px;
  font-size: 11px;
  color: var(--ui-muted);
  cursor: pointer;
}
.add:hover { background: var(--hover-bg); color: var(--ui-text); }
.preview {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.preview-frame {
  height: 198px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 4px;
  overflow: hidden;
}
.preview-name { margin: 4px 0 0; font-size: 12px; font-weight: 600; color: var(--ui-text); }
.preview-desc { margin: 0; font-size: 11px; line-height: 16px; color: var(--ui-text); }
.caption, .hint { margin: 0; font-size: 10px; line-height: 15px; color: var(--ui-muted); }
.issue { margin: 0; font-size: 10px; line-height: 15px; color: var(--warn-fg); }
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
