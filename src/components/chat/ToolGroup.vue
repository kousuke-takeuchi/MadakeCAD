<script setup lang="ts">
// 続けて使ったツールのまとまり (.pen「AIチャット 作業過程」ツールまとめ行・ツール 展開)。
// たたむと「ツールをN件使いました ›」、開くと左の線の中に手順を並べる。失敗を含むまとまりは行を赤くする。
import { ChevronDown, ChevronRight, Wrench } from "lucide-vue-next";
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import type { ChatToolCall } from "../../stores/chat";
import ToolStep from "./ToolStep.vue";

const props = defineProps<{
  calls: ChatToolCall[];
  failed: boolean;
  running: boolean;
  open: boolean;
}>();
defineEmits<{ toggle: [] }>();
const { t } = useI18n();

const label = computed(() => {
  const n = props.calls.length;
  return props.running ? t("chat.message.usingTools", { n }) : t("chat.message.usedTools", { n }, n);
});

/** 手順ごとの開閉 (既定は閉じる。パネルを開いている間だけ覚える)。 */
const openSteps = ref<Record<string, boolean>>({});
function toggleStep(id: string) {
  openSteps.value = { ...openSteps.value, [id]: !openSteps.value[id] };
}
</script>

<template>
  <div class="group">
    <button class="fold-row" :class="{ failed }" :aria-expanded="open" @click="$emit('toggle')">
      <Wrench :size="12" />
      <span>{{ label }}</span>
      <ChevronDown v-if="open" :size="12" class="chevron" />
      <ChevronRight v-else :size="12" class="chevron" />
    </button>
    <div v-if="open" class="rail">
      <ToolStep
        v-for="call in calls"
        :key="call.id"
        :call="call"
        :open="!!openSteps[call.id]"
        @toggle="toggleStep(call.id)"
      />
    </div>
  </div>
</template>

<style scoped>
.group {
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
.fold-row.failed {
  color: var(--err-fg);
}
.chevron {
  color: var(--ui-placeholder);
}
.rail {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 2px 0 2px 12px;
  border-left: 2px solid var(--ribbon-line);
}
</style>
