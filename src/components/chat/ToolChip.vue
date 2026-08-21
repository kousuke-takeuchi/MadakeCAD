<script setup lang="ts">
// エージェントのツール呼び出し1件を表す行チップ (デザイン: 「AIチャット(展開状態)」ツール行)。
import { CircleCheck, CircleX, LoaderCircle } from "lucide-vue-next";
import { computed } from "vue";
import { shortToolName, type ChatToolCall } from "../../stores/chat";

const props = defineProps<{ call: ChatToolCall }>();

const name = computed(() => shortToolName(props.call.tool));
</script>

<template>
  <div class="tool-chip" :class="call.status">
    <CircleCheck v-if="call.status === 'ok'" :size="12" class="icon ok" />
    <CircleX v-else-if="call.status === 'error'" :size="12" class="icon err" />
    <LoaderCircle v-else :size="12" class="icon running" />
    <span class="name">{{ name }}</span>
    <!-- 要約が作れないツール (MCP以外) はツール名だけ。空spanを描くと
         「ToolSearch ToolSearch」のような重複や無駄な余白になる -->
    <span v-if="call.summary" class="summary">{{ call.summary }}</span>
  </div>
</template>

<style scoped>
.tool-chip {
  display: flex;
  align-items: center;
  gap: 7px;
  width: 100%;
  padding: 5px 9px;
  border: 1px solid var(--ribbon-line);
  border-radius: 7px;
  background: var(--input-bg);
  min-width: 0;
}
.tool-chip.error {
  border-color: var(--err-fg);
  background: var(--err-bg);
}
.icon {
  flex-shrink: 0;
}
.icon.ok {
  color: var(--ok-fg);
}
.icon.err {
  color: var(--err-fg);
}
.icon.running {
  color: var(--ui-placeholder);
  animation: chip-spin 1s linear infinite;
}
@keyframes chip-spin {
  to {
    transform: rotate(360deg);
  }
}
.name {
  flex-shrink: 0;
  font-family: var(--mono-font);
  font-size: 10px;
  color: var(--ui-text);
}
.summary {
  font-size: 10px;
  color: var(--ui-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  min-width: 0;
}
</style>
