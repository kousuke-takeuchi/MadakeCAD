<script setup lang="ts">
// AIチャットパネル (デザイン: 「AIチャットパネル」/「AIチャット(展開状態)」)。
// 作図領域の左下オーバーレイ。折りたたみ⇔展開は store.panelOpen、最小化はローカル状態。
// 図面の編集は一切行わない (エージェントがMCP→Commandエンジン経由で編集する)。
import {
  ArrowUp,
  History,
  Maximize2,
  Minimize2,
  Minus,
  Plus,
  Square,
  SquarePen,
  Zap,
} from "lucide-vue-next";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { appliedCommandCount, useChatStore } from "../../stores/chat";
import { useDocumentStore } from "../../stores/document";
import { useUiStore } from "../../stores/ui";
import ChatMessage from "./ChatMessage.vue";
import ModelPicker from "./ModelPicker.vue";

const store = useChatStore();
const doc = useDocumentStore();
const ui = useUiStore();

const input = ref("");
const minimized = ref(false);
const listRef = ref<HTMLDivElement | null>(null);

const expanded = computed(() => store.panelOpen === "expanded");
const connected = computed(() => store.detect !== null);
const canSend = computed(() => input.value.trim().length > 0 && !store.streaming);

const title = computed(() => (store.messages.length ? "回路エージェント" : "新規エージェント"));

/** ⚡バッジ: 会話のトークン累計 (未使用なら並列エージェント数=1x)。 */
const tokenBadge = computed(() => {
  let total = 0;
  for (const m of store.messages) {
    if (m.usage) total += m.usage.input_tokens + m.usage.output_tokens;
  }
  if (total === 0) return "1x";
  return total >= 1000 ? `${(total / 1000).toFixed(1)}k` : String(total);
});

/**
 * 「元に戻す」を出せるのは最新の適用済みターンだけ (古いターンはmessage_indexが
 * ずれて別ターンを巻き戻す可能性があるため)。該当なしは-1。
 */
const lastAppliedIndex = computed(() => {
  const messages = store.messages;
  for (let i = messages.length - 1; i >= 0; i--) {
    if (appliedCommandCount(messages[i]) > 0 && !messages[i].undone) return i;
  }
  return -1;
});

async function submit() {
  if (!canSend.value) return;
  const text = input.value;
  input.value = "";
  const id = await store.send(text);
  if (!id) ui.log("AGENT   送信に失敗しました (詳細はチャットのエラー表示を参照)");
  void scrollToBottom();
}

function onKeydown(ev: KeyboardEvent) {
  if (ev.key !== "Enter" || ev.shiftKey || ev.isComposing) return;
  ev.preventDefault();
  void submit();
}

/** 最新の適用済みターンを巻き戻す。実行前にサーバーの会話状態へ再同期する。 */
async function onUndo() {
  const id = store.activeId;
  if (!id) return;
  try {
    await store.loadConversations();
    const index = lastAppliedIndex.value;
    if (store.activeId !== id || index < 0) return;
    await store.undoTurn(id, index);
  } catch (e) {
    ui.log(`AGENT   元に戻す失敗: ${String(e)}`);
  }
}

/** ストリーミング中断。pending会話などで失敗してもUIを固めない。 */
async function onCancel() {
  try {
    await store.cancel();
  } catch (e) {
    ui.log(`AGENT   キャンセル失敗: ${String(e)}`);
  }
}

function notImplemented(label: string) {
  ui.log(`AGENT   ${label}は未実装です (フェーズA2以降)`);
}

function expand() {
  minimized.value = false;
  store.setPanel("expanded");
}

function collapse() {
  minimized.value = false;
  store.setPanel("collapsed");
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
  <div class="chat-overlay">
    <!-- 最小化: タイトルバーのみ -->
    <div v-if="minimized" class="panel minimized">
      <SquarePen :size="14" class="head-icon" />
      <span class="head-title">{{ title }}</span>
      <span class="spacer" />
      <button class="icon-btn" title="展開" @click="expand"><Maximize2 :size="12" /></button>
    </div>

    <!-- 展開 -->
    <div v-else-if="expanded" class="panel expanded">
      <div class="head expanded-head">
        <SquarePen :size="14" class="head-icon" />
        <span class="head-title">{{ title }}</span>
        <span class="badge" :class="{ off: !connected }">
          <span class="dot" />
          {{ connected ? "接続中" : "未接続" }}
        </span>
        <span class="spacer" />
        <button class="icon-btn" title="履歴" @click="notImplemented('会話履歴')">
          <History :size="13" />
        </button>
        <button class="icon-btn" title="最小化" @click="minimized = true">
          <Minus :size="13" />
        </button>
        <button class="icon-btn" title="縮小" @click="collapse"><Minimize2 :size="12" /></button>
      </div>

      <div ref="listRef" class="messages">
        <ChatMessage
          v-for="(message, index) in store.messages"
          :key="index"
          :message="message"
          :can-undo="index === lastAppliedIndex"
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
          v-model="input"
          class="input"
          rows="2"
          placeholder="返信を入力..."
          @keydown="onKeydown"
        />
        <div class="footer">
          <button class="attach" title="コンテキストに追加" @click="notImplemented('コンテキスト添付')">
            <Plus :size="13" />
          </button>
          <span class="spacer" />
          <span class="token"><Zap :size="11" class="zap" />{{ tokenBadge }}</span>
          <ModelPicker :model-value="store.model" @update:model-value="store.setModel($event)" />
          <button
            v-if="store.streaming"
            class="send stop"
            title="停止"
            @click="onCancel"
          >
            <Square :size="11" />
          </button>
          <button v-else class="send" :class="{ ready: canSend }" title="送信" @click="submit">
            <ArrowUp :size="14" />
          </button>
        </div>
      </div>
    </div>

    <!-- 折りたたみ -->
    <div v-else class="panel collapsed">
      <div class="head">
        <SquarePen :size="14" class="head-icon" />
        <span class="head-title">{{ title }}</span>
        <span class="spacer" />
        <button class="icon-btn" title="最小化" @click="minimized = true">
          <Minus :size="13" />
        </button>
        <button class="icon-btn" title="展開" @click="expand"><Maximize2 :size="12" /></button>
      </div>

      <textarea
        v-model="input"
        class="input grow"
        placeholder="何でも指示できます... (例: 24V系にヒューズを追加して)"
        @keydown="onKeydown"
      />

      <div class="footer">
        <button class="attach" title="コンテキストに追加" @click="notImplemented('コンテキスト添付')">
          <Plus :size="13" />
        </button>
        <span class="spacer" />
        <span class="token"><Zap :size="11" class="zap" />{{ tokenBadge }}</span>
        <ModelPicker :model-value="store.model" @update:model-value="store.setModel($event)" />
        <button v-if="store.streaming" class="send stop" title="停止" @click="onCancel">
          <Square :size="11" />
        </button>
        <button v-else class="send" :class="{ ready: canSend }" title="送信" @click="submit">
          <ArrowUp :size="14" />
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 作図領域の左下オーバーレイ。キャンバス操作を邪魔しないよう枠はpointer-events:none */
.chat-overlay {
  position: absolute;
  left: 14px;
  top: 14px;
  bottom: 14px;
  width: 380px;
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  pointer-events: none;
  z-index: 5;
}
.panel {
  pointer-events: auto;
  display: flex;
  flex-direction: column;
  background: var(--ribbon-bg);
  border: 1px solid var(--ribbon-line);
  color: var(--ui-text);
}
.panel.minimized {
  flex-direction: row;
  align-items: center;
  gap: 7px;
  width: 330px;
  padding: 8px 12px;
  border-radius: 12px;
  box-shadow: var(--shadow-panel);
}
.panel.collapsed {
  width: 330px;
  height: 148px;
  gap: 8px;
  padding: 10px 12px;
  border-radius: 12px;
  box-shadow: var(--shadow-panel);
}
.panel.expanded {
  width: 380px;
  height: 640px;
  max-height: 100%;
  border-radius: 14px;
  box-shadow: var(--shadow-panel-lg);
  overflow: hidden;
}

/* ヘッダ */
.head {
  display: flex;
  align-items: center;
  gap: 7px;
  flex-shrink: 0;
}
.expanded-head {
  padding: 10px 14px;
  border-bottom: 1px solid var(--ribbon-line);
}
.head-icon {
  flex-shrink: 0;
  color: var(--ui-text);
}
.head-title {
  font-size: 12px;
  font-weight: 600;
  white-space: nowrap;
}
.spacer {
  flex: 1;
}
.icon-btn {
  display: flex;
  align-items: center;
  padding: 2px;
  border: none;
  background: transparent;
  color: var(--ui-muted);
  cursor: pointer;
}
.icon-btn:hover {
  color: var(--ui-text);
}
.badge {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  border-radius: 9px;
  background: var(--ok-bg);
  font-size: 10px;
  color: var(--ok-fg);
  white-space: nowrap;
}
.badge .dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--ok-fg);
}
.badge.off {
  background: var(--off-bg);
  color: var(--off-fg);
}
.badge.off .dot {
  background: var(--off-fg);
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
.input.grow {
  flex: 1;
  min-height: 0;
}
.input::placeholder {
  color: var(--ui-placeholder);
}

/* フッタ */
.footer {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}
.attach {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: 1px solid var(--ribbon-line);
  border-radius: 12px;
  background: transparent;
  color: var(--ui-muted);
  cursor: pointer;
}
.attach:hover {
  background: var(--hover-bg);
}
.token {
  display: flex;
  align-items: center;
  gap: 3px;
  font-size: 11px;
  color: var(--ui-muted);
  white-space: nowrap;
}
.zap {
  color: var(--warn-fg);
}
.send {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  flex-shrink: 0;
  border: none;
  border-radius: 13px;
  background: var(--hover-bg);
  color: var(--ui-placeholder);
  cursor: pointer;
}
.send.ready,
.send.stop {
  background: var(--acad-blue);
  color: var(--card-bg);
}
</style>
