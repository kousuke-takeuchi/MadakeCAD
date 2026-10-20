<script setup lang="ts">
// 思考の行 (.pen「AIチャット 作業過程」思考行・思考 展開)。たたむと「N秒考えました ›」、開くと左の線の中に思考の文。
import { Brain, ChevronDown, ChevronRight } from "lucide-vue-next";
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { thinkingSeconds } from "../../chat/blocks";

const props = defineProps<{
  text: string;
  durationMs: number | null;
  open: boolean;
  /** いま思考が届いている (応答中の最後の行) */
  live: boolean;
}>();
defineEmits<{ toggle: [] }>();
const { t } = useI18n();

const label = computed(() => {
  if (props.live) return t("chat.message.thinking");
  const s = thinkingSeconds(props.durationMs);
  return s === null ? t("chat.message.thoughtShort") : t("chat.message.thought", { s });
});
</script>

<template>
  <div class="thinking">
    <button class="fold-row" :aria-expanded="open" @click="$emit('toggle')">
      <Brain :size="12" />
      <span>{{ label }}</span>
      <ChevronDown v-if="open" :size="12" class="chevron" />
      <ChevronRight v-else :size="12" class="chevron" />
    </button>
    <div v-if="open" class="rail">
      <p class="thought">{{ text }}</p>
    </div>
  </div>
</template>

<style scoped>
.thinking {
  display: flex;
  flex-direction: column;
  gap: 6px;
  width: 100%;
}
.fold-row {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  align-self: flex-start;
  padding: 0;
  border: none;
  background: transparent;
  font: inherit;
  font-size: 11px;
  color: var(--ui-muted);
  cursor: pointer;
}
.fold-row:hover {
  color: var(--ui-text);
}
.chevron {
  color: var(--ui-placeholder);
}
.rail {
  padding: 2px 0 2px 12px;
  border-left: 2px solid var(--ribbon-line);
}
.thought {
  margin: 0;
  font-size: 11px;
  line-height: 1.6;
  color: var(--ui-muted);
  white-space: pre-wrap;
  word-break: break-word;
}
</style>
