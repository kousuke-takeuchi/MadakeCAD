<script setup lang="ts">
// モデルピッカー (デザイン: 「AIチャット - ポップアップ集」モデルピッカー)。
// A1はAnthropicグループのみ。選択値はそのまま Claude Code CLI の `--model` へ渡る
// (nullはCLI側の既定モデル)。
import { ChevronDown, Plus, Search } from "lucide-vue-next";
import { computed, nextTick, ref } from "vue";
import { useI18n } from "vue-i18n";
import { usePopover } from "../../composables/popover";
import { anthropicModels, type ModelOption } from "./models";

const props = defineProps<{ modelValue: string | null }>();
const { t } = useI18n();
const models = computed(() => anthropicModels(t));
const emit = defineEmits<{ "update:modelValue": [string | null] }>();

const query = ref("");
const searchRef = ref<HTMLInputElement | null>(null);

const { open, toggle, close } = usePopover({
  onOpen: async () => {
    await nextTick();
    searchRef.value?.focus();
  },
  onClose: () => {
    query.value = "";
  },
});

const current = computed(
  () => models.value.find((m) => m.id === props.modelValue) ?? models.value[0],
);

const filtered = computed(() => {
  const q = query.value.trim().toLowerCase();
  if (!q) return models.value;
  return models.value.filter(
    (m) => m.label.toLowerCase().includes(q) || (m.id ?? "").toLowerCase().includes(q),
  );
});

function pick(option: ModelOption) {
  emit("update:modelValue", option.id);
  close();
}
</script>

<template>
  <div class="model-picker">
    <button class="trigger" :title="current.id ?? t('chat.modelPicker.defaultTitle')" @click="toggle()">
      <span class="label">{{ current.label }}</span>
      <ChevronDown :size="11" class="chevron" />
    </button>

    <template v-if="open">
      <div class="backdrop" @click="close()" />
      <div class="popup">
        <div class="search-row">
          <Search :size="13" class="search-icon" />
          <input ref="searchRef" v-model="query" :placeholder="t('chat.modelPicker.search')" />
        </div>
        <div class="group">ANTHROPIC</div>
        <button
          v-for="option in filtered"
          :key="option.id ?? 'default'"
          class="option"
          :class="{ selected: option.id === modelValue }"
          @click="pick(option)"
        >
          <span class="option-name">{{ option.label }}</span>
          <span v-if="option.sub" class="option-sub">{{ option.sub }}</span>
        </button>
        <div v-if="!filtered.length" class="empty">{{ t("chat.modelPicker.empty") }}</div>
        <div class="add-row">
          <Plus :size="12" />
          <span>{{ t("chat.modelPicker.morePlanned") }}</span>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.model-picker {
  position: relative;
  flex-shrink: 0;
}
.trigger {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 3px 8px;
  border: none;
  border-radius: 8px;
  background: transparent;
  cursor: pointer;
  color: var(--ui-text);
  font: inherit;
}
.trigger:hover {
  background: var(--hover-bg);
}
.label {
  font-size: 11px;
  white-space: nowrap;
}
.chevron {
  color: var(--ui-muted);
}
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 20;
}
.popup {
  position: absolute;
  right: 0;
  bottom: calc(100% + 6px);
  z-index: 21;
  width: 240px;
  display: flex;
  flex-direction: column;
  background: var(--card-bg);
  border: 1px solid var(--ribbon-line);
  border-radius: 12px;
  box-shadow: var(--shadow-popup);
  overflow: hidden;
}
.search-row {
  display: flex;
  align-items: center;
  gap: 7px;
  padding: 9px 12px;
  border-bottom: 1px solid var(--ribbon-line);
}
.search-icon {
  color: var(--ui-placeholder);
  flex-shrink: 0;
}
.search-row input {
  flex: 1;
  min-width: 0;
  border: none;
  outline: none;
  background: transparent;
  font: inherit;
  font-size: 12px;
  color: var(--ui-text);
}
.search-row input::placeholder {
  color: var(--ui-placeholder);
}
.group {
  padding: 7px 12px 3px;
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 1px;
  color: var(--ui-placeholder);
}
.option {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  padding: 6px 12px;
  border: none;
  background: transparent;
  font: inherit;
  text-align: left;
  cursor: pointer;
}
.option:hover {
  background: var(--hover-bg);
}
.option.selected {
  background: var(--sel-blue);
}
.option-name {
  flex: 1;
  min-width: 0;
  font-size: 12px;
  color: var(--ui-text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.option-sub {
  flex-shrink: 0;
  font-size: 10px;
  color: var(--ui-placeholder);
}
.empty {
  padding: 8px 12px;
  font-size: 11px;
  color: var(--ui-muted);
}
.add-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 12px;
  border-top: 1px solid var(--ribbon-line);
  font-size: 11px;
  color: var(--off-fg);
}
</style>
