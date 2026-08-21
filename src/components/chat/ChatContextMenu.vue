<script setup lang="ts">
// 「コンテキストに追加」メニュー (デザイン: 「AIチャット - ポップアップ集」Pコンテキスト)。
// 入力フッタの+ボタンから開く。開閉は親が usePopover で持つ。
//
// A1で動くのは「図面から追加...」だけ。挿し込むのはタグ文字列であって、
// 図面そのものには一切触れない (document storeは読み取りのみ)。
import { BookOpen, Database, FileText, Paperclip } from "lucide-vue-next";
import { appendContextTag, drawingContextTag } from "../../composables/drawingContext";
import { useDocumentStore } from "../../stores/document";
import { useUiStore } from "../../stores/ui";

const emit = defineEmits<{ close: [] }>();

const doc = useDocumentStore();
const ui = useUiStore();

/**
 * メニュー項目。`note`があるものは未実装 (グレー表示のまま、押されたら予定を知らせる)。
 * 並びはデザイン通り: 画像/ファイル → 図面 → 部品DB → 規格。
 *
 * 未実装項目は`disabled`/`aria-disabled`にしない。押せない要素にすると、
 * 「表示は残して操作時に予定をログする」というデザインシステムの方針
 * (docs/design-system.md「未実装機能のUI」) を満たせなくなる。
 */
const items: { icon: typeof Paperclip; label: string; note?: string }[] = [
  {
    icon: Paperclip,
    label: "画像/ファイルを追加... (データシートPDF等)",
    note: "画像/ファイルの添付はフェーズA2で対応予定です",
  },
  { icon: FileText, label: "図面から追加... (シート/選択範囲)" },
  {
    icon: Database,
    label: "部品DBから追加...",
    note: "部品DBの参照はフェーズ2で対応予定です",
  },
  {
    icon: BookOpen,
    label: "規格を選択... (JIS C 0617 ほか)",
    note: "規格の参照はフェーズ2で対応予定です",
  },
];

function onFromDrawing() {
  const tag = drawingContextTag(doc.activeSheet, doc.selection);
  ui.setChatDraft(appendContextTag(ui.chatDraft, tag));
  emit("close");
}

function onPick(note: string | undefined) {
  if (note) ui.log(`AGENT   ${note}`);
  else onFromDrawing();
}
</script>

<template>
  <div class="context-menu">
    <div class="body">
      <div class="head">コンテキストに追加</div>

      <button
        v-for="item in items"
        :key="item.label"
        class="item"
        :class="{ disabled: !!item.note }"
        :title="item.note"
        @click="onPick(item.note)"
      >
        <component :is="item.icon" :size="14" class="icon" />
        <span class="label">{{ item.label }}</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.context-menu {
  width: 300px;
  display: flex;
  flex-direction: column;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 12px;
  box-shadow: var(--shadow-popup);
  overflow: hidden;
}
.body {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 10px 8px;
}
.head {
  padding: 0 2px 2px;
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 1px;
  color: var(--ui-placeholder);
  white-space: nowrap;
}
.item {
  display: flex;
  align-items: center;
  gap: 9px;
  width: 100%;
  padding: 8px 10px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--ui-text);
  font: inherit;
  text-align: left;
  cursor: pointer;
}
.item:hover {
  background: var(--hover-bg);
}
.item.disabled {
  color: var(--off-fg);
  cursor: default;
}
.item.disabled:hover {
  background: var(--off-bg);
}
.icon {
  flex-shrink: 0;
}
.label {
  font-size: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
