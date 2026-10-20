<script setup lang="ts">
// ツールの手順1行 (.pen「AIチャット 作業過程(展開)」手順・入力ボックス・結果ボックス)。
// 行は状態・ツール名・要約・›。開くと、エージェントが送った入力と返ってきた結果を出し、それぞれコピーできる。
import { ChevronDown, ChevronRight, CircleCheck, CircleX, Copy, LoaderCircle } from "lucide-vue-next";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { omittedKb, prettyInput } from "../../chat/blocks";
import { shortToolName, type ChatToolCall } from "../../stores/chat";

const props = defineProps<{ call: ChatToolCall; open: boolean }>();
defineEmits<{ toggle: [] }>();
const { t } = useI18n();

const name = computed(() => shortToolName(props.call.tool));
const input = computed(() => prettyInput(props.call.input));
const omitted = computed(() => omittedKb(props.call));

function copy(text: string) {
  void navigator.clipboard.writeText(text);
}
</script>

<template>
  <div class="step">
    <button class="step-row" :aria-expanded="open" @click="$emit('toggle')">
      <CircleCheck v-if="call.status === 'ok'" :size="12" class="icon ok" />
      <CircleX v-else-if="call.status === 'error'" :size="12" class="icon err" />
      <LoaderCircle v-else :size="12" class="icon running" />
      <span class="name">{{ name }}</span>
      <!-- 要約が作れないツール (MCP以外) はツール名だけ -->
      <span v-if="call.summary" class="summary">{{ call.summary }}</span>
      <span class="spacer" />
      <ChevronDown v-if="open" :size="11" class="chevron" />
      <ChevronRight v-else :size="11" class="chevron" />
    </button>
    <template v-if="open">
      <div class="panel">
        <div class="panel-head">
          <span>{{ t("chat.message.input") }}</span>
          <button class="copy" :title="t('chat.message.copy')" @click="copy(input)"><Copy :size="11" /></button>
        </div>
        <pre class="panel-body">{{ input }}</pre>
      </div>
      <div class="panel" :class="{ failed: call.status === 'error' }">
        <div class="panel-head">
          <span>{{ t("chat.message.result") }}</span>
          <button v-if="call.output !== null" class="copy" :title="t('chat.message.copy')" @click="copy(call.output)">
            <Copy :size="11" />
          </button>
        </div>
        <pre v-if="call.output !== null" class="panel-body">{{ call.output }}</pre>
        <p v-else class="panel-empty">{{ t("chat.message.noResult") }}</p>
        <p v-if="omitted !== null" class="panel-empty">{{ t("chat.message.omitted", { kb: omitted }) }}</p>
      </div>
    </template>
  </div>
</template>

<style scoped>
.step {
  display: flex;
  flex-direction: column;
  gap: 6px;
  width: 100%;
  min-width: 0;
}
.step-row {
  display: flex;
  align-items: center;
  gap: 7px;
  width: 100%;
  min-width: 0;
  padding: 0;
  border: none;
  background: transparent;
  font: inherit;
  text-align: left;
  cursor: pointer;
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
  animation: step-spin 1s linear infinite;
}
@keyframes step-spin {
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
  min-width: 0;
  overflow: hidden;
  font-size: 10px;
  color: var(--ui-muted);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.spacer {
  flex: 1;
}
.chevron {
  flex-shrink: 0;
  color: var(--ui-placeholder);
}
.panel {
  border-radius: 6px;
  background: var(--hover-bg);
  overflow: hidden;
}
.panel.failed {
  background: var(--err-bg);
}
.panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 8px;
  font-size: 10px;
  color: var(--ui-muted);
}
.copy {
  display: inline-flex;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--ui-placeholder);
  cursor: pointer;
}
.copy:hover {
  color: var(--acad-blue);
}
.panel-body {
  max-height: 240px;
  margin: 0;
  padding: 0 8px 8px;
  overflow: auto;
  font-family: var(--mono-font);
  font-size: 10px;
  line-height: 1.5;
  color: var(--ui-text);
  white-space: pre-wrap;
  word-break: break-word;
}
.panel-empty {
  margin: 0;
  padding: 0 8px 8px;
  font-size: 10px;
  color: var(--ui-placeholder);
}
</style>
