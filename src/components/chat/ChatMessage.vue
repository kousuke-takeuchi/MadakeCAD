<script setup lang="ts">
// 会話1件の描画。ユーザー発話は右寄せ吹き出し、アシスタントは起きた順の行
// (本文のMarkdown・思考・ツールのまとまり。.pen「AIチャット 作業過程」) +適用済み行。
import { CircleX, LoaderCircle } from "lucide-vue-next";
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { displayItems, isOpen } from "../../chat/blocks";
import { appliedCommandCount, type ChatMessage } from "../../stores/chat";
import AttachmentChip from "./AttachmentChip.vue";
import ChatMarkdown from "./ChatMarkdown.vue";
import ThinkingRow from "./ThinkingRow.vue";
import ToolGroup from "./ToolGroup.vue";

const { t } = useI18n();

const props = defineProps<{
  message: ChatMessage;
  /** 「元に戻す」を出せるターンか (最新の適用済みターンのみtrue)。 */
  canUndo?: boolean;
}>();
const emit = defineEmits<{ undo: [] }>();

/** 「✓ 図面に適用済み (rev N)」を出すか。 */
const applied = computed(() => appliedCommandCount(props.message) > 0 && !props.message.undone);

const items = computed(() => displayItems(props.message));
/** 利用者が開閉した行 (パネルを開いている間だけ覚える)。 */
const overrides = ref<Record<string, boolean>>({});
const opened = (key: string) => isOpen(key, items.value, props.message.streaming, overrides.value);
function toggle(key: string) {
  overrides.value = { ...overrides.value, [key]: !opened(key) };
}

/** 最後の行が思考・ツールなら、その行が進行を示すので進行行は出さない。 */
const showProgress = computed(() => {
  if (!props.message.streaming) return false;
  const last = items.value[items.value.length - 1];
  return !last || last.kind === "text";
});
</script>

<template>
  <div v-if="message.role === 'user'" class="user-row">
    <!-- 送信した添付 (.pen「添付デザイン」④。×の無いチップ)。本文はpre-wrapなので余白を入れずに置く -->
    <div class="bubble"><div v-if="message.attachments?.length" class="attachments"><AttachmentChip v-for="(a, i) in message.attachments" :key="i" :name="a.name" :mime="a.mime" :size="a.size" /></div>{{ message.text }}</div>
  </div>

  <div v-else class="assistant">
    <template v-for="(item, index) in items" :key="item.key">
      <ChatMarkdown v-if="item.kind === 'text'" :text="item.text" />
      <ThinkingRow
        v-else-if="item.kind === 'thinking'"
        :text="item.text"
        :duration-ms="item.duration_ms"
        :open="opened(item.key)"
        :live="message.streaming && index === items.length - 1"
        @toggle="toggle(item.key)"
      />
      <ToolGroup
        v-else
        :calls="item.calls"
        :failed="item.failed"
        :running="item.running"
        :open="opened(item.key)"
        @toggle="toggle(item.key)"
      />
    </template>

    <div v-if="showProgress" class="progress">
      <LoaderCircle :size="12" class="spinner" />
      <span>{{ t("chat.message.working") }}</span>
    </div>

    <div v-if="message.error" class="error">
      <CircleX :size="12" />
      <span>{{ message.error }}</span>
    </div>

    <div v-if="applied || message.undone" class="action-row">
      <span v-if="applied" class="applied">{{ t("chat.message.applied", { rev: message.applied_revisions.end }) }}</span>
      <span v-else class="undone-label">{{ t("chat.message.undone") }}</span>
      <button v-if="applied && canUndo" class="undo" @click="emit('undo')">{{ t("chat.message.undo") }}</button>
    </div>
  </div>
</template>

<style scoped>
.attachments {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 4px;
  margin-bottom: 6px;
}
.user-row {
  display: flex;
  justify-content: flex-end;
  width: 100%;
}
.bubble {
  max-width: 76%;
  padding: 8px 12px;
  background: var(--hover-bg);
  border-radius: 12px 12px 4px 12px;
  font-size: 12px;
  line-height: 18px;
  color: var(--ui-text);
  white-space: pre-wrap;
  word-break: break-word;
}
.assistant {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 8px;
  width: 100%;
}
.progress {
  display: flex;
  align-items: center;
  gap: 7px;
  font-size: 11px;
  color: var(--ui-placeholder);
}
.spinner {
  animation: msg-spin 1s linear infinite;
}
@keyframes msg-spin {
  to {
    transform: rotate(360deg);
  }
}
.error {
  display: flex;
  align-items: center;
  gap: 7px;
  width: 100%;
  padding: 5px 9px;
  border: 1px solid var(--err-fg);
  border-radius: 8px;
  background: var(--err-bg);
  font-size: 10px;
  color: var(--err-fg);
  word-break: break-word;
}
.action-row {
  display: flex;
  align-items: center;
  gap: 12px;
}
.applied {
  font-size: 10px;
  color: var(--ok-fg);
}
.undone-label {
  font-size: 10px;
  color: var(--ui-muted);
}
.undo {
  padding: 0;
  border: none;
  background: transparent;
  font: inherit;
  font-size: 10px;
  color: var(--ui-muted);
  cursor: pointer;
}
.undo:hover {
  color: var(--acad-blue);
  text-decoration: underline;
}
</style>
