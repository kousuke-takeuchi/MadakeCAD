<script setup lang="ts">
// 「自動で整える」ポップアップ (デザイン: 「AIチャット - ポップアップ集」P自動反復)。
// 入力フッタの杖ボタンから開く。開閉は親が usePopover で持つ。
//
// 3つの反復モードは押した瞬間に定型プロンプトを組み立てて**普通のチャット送信1回**として
// 実行する (1整え=1ターン=undo一発)。図面には一切触らない。
import { useI18n } from "vue-i18n";
import { TIDY_MODES, runTidy, type TidyMode } from "../../composables/tidy";
import { useChatStore } from "../../stores/chat";
import { useUiStore } from "../../stores/ui";

const emit = defineEmits<{ close: [] }>();

const { t } = useI18n();
const chat = useChatStore();
const ui = useUiStore();

/**
 * バリアント数 (複数案を並行で作って比べる) はフェーズ3の予定。
 * デザインどおり表示は残し、押されたら予定をログする
 * (docs/internal/design-system.md「未実装機能のUI」)。
 */
const VARIANT_COUNTS = [2, 3, 4];

async function onMode(mode: TidyMode) {
  emit("close");
  if (chat.streaming) {
    ui.log(t("chat.tidy.busyLog"));
    return;
  }
  ui.log(t("chat.tidy.startedLog", { mode: t(`chat.tidy.mode.${mode}`) }));
  await runTidy(mode);
}

function onVariantPlanned() {
  ui.log(t("chat.tidy.variantPlannedLog"));
}
</script>

<template>
  <div class="tidy-menu">
    <div class="body">
      <div class="title">{{ t("chat.tidy.title") }}</div>
      <div class="description">{{ t("chat.tidy.description") }}</div>

      <div class="head">{{ t("chat.tidy.modeHead") }}</div>
      <div class="row">
        <button
          v-for="mode in TIDY_MODES"
          :key="mode"
          class="chip"
          :disabled="chat.streaming"
          @click="onMode(mode)"
        >
          {{ t(`chat.tidy.mode.${mode}`) }}
        </button>
      </div>

      <div class="head">{{ t("chat.tidy.variantHead") }}</div>
      <div class="row">
        <button class="count selected">{{ t("chat.tidy.variantNone") }}</button>
        <button
          v-for="count in VARIANT_COUNTS"
          :key="count"
          class="count disabled"
          @click="onVariantPlanned()"
        >
          {{ count }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.tidy-menu {
  width: 320px;
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
  gap: 10px;
  padding: 12px 14px;
}
.title {
  font-size: 13px;
  font-weight: 700;
  color: var(--ui-text);
}
.description {
  font-size: 11px;
  color: var(--ui-muted);
}
.head {
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 1px;
  color: var(--ui-placeholder);
}
.row {
  display: flex;
  gap: 8px;
}
.chip {
  flex: 1;
  padding: 8px 0;
  border: 1px solid var(--ribbon-line);
  border-radius: 9px;
  background: var(--card-bg);
  color: var(--ui-muted);
  font: inherit;
  font-size: 11px;
  cursor: pointer;
}
.chip:hover {
  background: var(--hover-bg);
  color: var(--ui-text);
}
.chip:disabled {
  color: var(--off-fg);
  background: var(--off-bg);
  cursor: default;
}
.count {
  flex: 1;
  padding: 9px 0;
  border: 1px solid var(--ribbon-line);
  border-radius: 9px;
  background: var(--card-bg);
  color: var(--ui-text);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}
.count.selected {
  border-color: transparent;
  background: var(--hover-bg);
  font-weight: 600;
}
.count.disabled {
  color: var(--off-fg);
  cursor: default;
}
.count.disabled:hover {
  background: var(--off-bg);
}
</style>
