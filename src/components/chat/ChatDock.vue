<script setup lang="ts">
// 左ドック「エージェント」タブの中身 (デザイン: 「左パネル(タブ式)」)。
// 会話エリア + 入力フッタ。タブ行は親の LeftPanel が描く。
// タブ切替でもアンマウントされない (v-show) ので、エージェントイベントの購読
// (bootstrap/unsubscribe) はここが持つ。図面の編集は一切行わない。
import { nextTick, computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useChatComposer } from "../../composables/chatComposer";
import { appliedCommandCount, useChatStore } from "../../stores/chat";
import { useDocumentStore } from "../../stores/document";
import { useUiStore } from "../../stores/ui";
import ChatComposerFooter from "./ChatComposerFooter.vue";
import ChatMessage from "./ChatMessage.vue";

const store = useChatStore();
const doc = useDocumentStore();
const ui = useUiStore();
const { draft, onKeydown } = useChatComposer();

const listRef = ref<HTMLDivElement | null>(null);
const connected = computed(() => store.detect !== null);

/**
 * 「元に戻す」を出すのは最新の適用済みターンだけ (サーバーは古いターンの巻き戻しも
 * 受け付けるが、UIは直前のターンを取り消す1ボタンに絞る)。該当なしは-1。
 */
const lastAppliedIndex = computed(() => {
  const messages = store.messages;
  for (let i = messages.length - 1; i >= 0; i--) {
    if (appliedCommandCount(messages[i]) > 0 && !messages[i].undone) return i;
  }
  return -1;
});

/**
 * 最新の適用済みターンを巻き戻す。実行前にサーバーの会話状態へ再同期する
 * (再取得でターン安定IDも埋まるため、添字のズレとは無関係に対象を指定できる)。
 */
async function onUndo() {
  const id = store.activeId;
  // ストリーミング中の再同期は進行中ターンの表示を壊すため不可 (サーバー側もBusyで拒否する)
  if (!id || store.streaming) return;
  try {
    await store.loadConversations();
    const index = lastAppliedIndex.value;
    if (store.activeId !== id || index < 0) return;
    const turnId = store.messages[index].turn_id;
    if (!turnId) return;
    await store.undoTurn(id, turnId);
  } catch (e) {
    ui.log(`AGENT   元に戻す失敗: ${String(e)}`);
  }
}

async function scrollToBottom() {
  await nextTick();
  const el = listRef.value;
  if (el) el.scrollTop = el.scrollHeight;
}

watch(
  () => [store.messages.length, store.streamingMessage?.text, store.streaming] as const,
  () => void scrollToBottom(),
);

// 非表示(プロジェクトタブ)の間はscrollHeightが0なので、表示に戻った時に追従し直す。
watch(
  () => ui.leftPanelTab,
  (tab) => {
    if (tab === "chat") void scrollToBottom();
  },
);

// プロジェクトの読込/切替 (patchの project_replaced) 後は会話を読み直す(幽霊会話防止)。
watch(
  () => doc.project,
  () => {
    void store
      .loadConversations()
      .catch((e) => ui.log(`AGENT   会話履歴の再読込に失敗: ${String(e)}`));
  },
);

onMounted(() => {
  void store.bootstrap().catch((e) => ui.log(`AGENT   エージェント接続に失敗: ${String(e)}`));
});

onBeforeUnmount(() => store.unsubscribe());
</script>

<template>
  <div class="dock">
    <div ref="listRef" class="messages">
      <ChatMessage
        v-for="(message, index) in store.messages"
        :key="index"
        :message="message"
        :can-undo="index === lastAppliedIndex && !store.streaming"
        @undo="onUndo()"
      />
      <p v-if="!store.messages.length" class="empty">
        {{
          connected
            ? "図面について指示してください。編集は自動で適用され、あとから元に戻せます。"
            : "Claude Code CLI が見つかりません。`claude` をインストールしてサインインしてください。"
        }}
      </p>
    </div>

    <div class="input-area">
      <textarea
        v-model="draft"
        class="input"
        rows="2"
        placeholder="返信を入力..."
        @keydown="onKeydown"
      />
      <ChatComposerFooter />
    </div>
  </div>
</template>

<style scoped>
.dock {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background: var(--ribbon-bg);
  color: var(--ui-text);
}

/* 会話エリア */
.messages {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 12px;
  padding: 14px;
  overflow-y: auto;
}
.empty {
  margin: 0;
  font-size: 11px;
  line-height: 18px;
  color: var(--ui-placeholder);
}

/* 入力 */
.input-area {
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 10px 12px;
  border-top: 1px solid var(--ribbon-line);
}
.input {
  border: none;
  outline: none;
  background: transparent;
  resize: none;
  font: inherit;
  font-size: 12px;
  line-height: 18px;
  color: var(--ui-text);
  padding: 0;
}
.input::placeholder {
  color: var(--ui-placeholder);
}
</style>
