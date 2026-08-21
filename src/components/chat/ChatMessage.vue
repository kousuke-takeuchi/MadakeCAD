<script setup lang="ts">
// 会話1件の描画。ユーザー発話は右寄せ吹き出し、アシスタントは地の文+ツールチップ+適用済み行。
import { CircleX, LoaderCircle } from "lucide-vue-next";
import { computed } from "vue";
import { appliedCommandCount, type ChatMessage } from "../../stores/chat";
import ToolChip from "./ToolChip.vue";

const props = defineProps<{
  message: ChatMessage;
  /** 「元に戻す」を出せるターンか (最新の適用済みターンのみtrue)。 */
  canUndo?: boolean;
}>();
const emit = defineEmits<{ undo: [] }>();

/** 「✓ 図面に適用済み (rev N)」を出すか。 */
const applied = computed(() => appliedCommandCount(props.message) > 0 && !props.message.undone);

/** 実行中ツールがある間はそのチップが進行を示すので、進行行は出さない。 */
const showProgress = computed(
  () => props.message.streaming && !props.message.tool_calls.some((c) => c.status === "running"),
);
</script>

<template>
  <div v-if="message.role === 'user'" class="user-row">
    <div class="bubble">{{ message.text }}</div>
  </div>

  <div v-else class="assistant">
    <p v-if="message.text" class="text">{{ message.text }}</p>

    <ToolChip v-for="call in message.tool_calls" :key="call.id" :call="call" />

    <div v-if="showProgress" class="progress">
      <LoaderCircle :size="12" class="spinner" />
      <span>エージェントが作業しています...</span>
    </div>

    <div v-if="message.error" class="error">
      <CircleX :size="12" />
      <span>{{ message.error }}</span>
    </div>

    <div v-if="applied || message.undone" class="action-row">
      <span v-if="applied" class="applied">✓ 図面に適用済み (rev {{ message.applied_revisions.end }})</span>
      <span v-else class="undone-label">元に戻しました</span>
      <button v-if="applied && canUndo" class="undo" @click="emit('undo')">元に戻す</button>
    </div>
  </div>
</template>

<style scoped>
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
.text {
  margin: 0;
  font-size: 12px;
  line-height: 19px;
  color: var(--ui-text);
  white-space: pre-wrap;
  word-break: break-word;
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
