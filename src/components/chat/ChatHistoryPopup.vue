<script setup lang="ts">
// 会話履歴ポップアップ (デザイン: 「AIチャット - ポップアップ集」P会話履歴)。
// 左ドックのタブ行の履歴ボタンから開く。開閉は親が usePopover で持ち、
// ここは中身と「選んだ / 新規」の通知だけを担当する。
import { Plus } from "lucide-vue-next";
import { computed } from "vue";
import {
  conversationMeta,
  conversationTitle,
  sortedConversations,
  useChatStore,
} from "../../stores/chat";

const emit = defineEmits<{ close: [] }>();

const store = useChatStore();

// 「N分前」の基準時刻はポップアップを開いた瞬間で固定する
// (再描画のたびに Date.now() を読むと、リストの表示が理由もなく揺れる)。
const now = Date.now();

const rows = computed(() =>
  sortedConversations(store.conversations).map((conversation) => ({
    id: conversation.id,
    title: conversationTitle(conversation),
    meta: conversationMeta(conversation, now),
    active: conversation.id === store.activeId,
  })),
);

function onNew() {
  store.newConversation();
  emit("close");
}

function onPick(id: string) {
  store.setActive(id);
  emit("close");
}
</script>

<template>
  <div class="history-popup">
    <div class="body">
      <div class="head-row">
        <span class="head">会話履歴</span>
        <span class="spacer" />
        <button class="new" @click="onNew">
          <Plus :size="11" />
          <span>新規会話</span>
        </button>
      </div>

      <button
        v-for="row in rows"
        :key="row.id"
        class="row"
        :class="{ active: row.active }"
        @click="onPick(row.id)"
      >
        <span class="row-title">{{ row.title }}</span>
        <span class="row-meta">{{ row.meta }}</span>
      </button>
      <div v-if="!rows.length" class="empty">会話はまだありません</div>
    </div>
  </div>
</template>

<style scoped>
.history-popup {
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
  max-height: 360px;
  overflow-y: auto;
}
.head-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 2px;
}
.head {
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 1px;
  color: var(--ui-placeholder);
  white-space: nowrap;
}
.spacer {
  flex: 1;
}
.new {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
  padding: 3px 8px;
  border: none;
  border-radius: 6px;
  background: var(--hover-bg);
  color: var(--ui-text);
  font: inherit;
  font-size: 11px;
  font-weight: 600;
  white-space: nowrap;
  cursor: pointer;
}
.new:hover {
  background: var(--palette-head);
}
.row {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  width: 100%;
  padding: 8px 10px;
  border: none;
  border-radius: 8px;
  background: transparent;
  font: inherit;
  text-align: left;
  cursor: pointer;
}
.row:hover {
  background: var(--off-bg);
}
.row.active {
  background: var(--hover-bg);
}
.row-title {
  width: 100%;
  font-size: 12px;
  color: var(--ui-text);
  overflow-wrap: anywhere;
}
.row.active .row-title {
  font-weight: 600;
}
.row-meta {
  font-size: 10px;
  color: var(--ui-placeholder);
  white-space: nowrap;
}
.empty {
  padding: 8px 10px;
  font-size: 11px;
  color: var(--ui-muted);
}
</style>
