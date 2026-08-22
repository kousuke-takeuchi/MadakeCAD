<script setup lang="ts">
// 入力欄の下のフッタ行 (添付 / トークン / モデル / 送信・停止)。
// 左ドックのエージェントタブと浮き入力カードで共通。
import { ArrowUp, Plus, Square, WandSparkles, Zap } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { useChatComposer } from "../../composables/chatComposer";
import { usePopover } from "../../composables/popover";
import ChatContextMenu from "./ChatContextMenu.vue";
import ChatTidyMenu from "./ChatTidyMenu.vue";
import ModelPicker from "./ModelPicker.vue";

const { t } = useI18n();
const { store, canSend, tokenBadge, submit, onCancel } = useChatComposer();
const { open: contextOpen, toggle: toggleContext, close: closeContext } = usePopover();
const { open: tidyOpen, toggle: toggleTidy, close: closeTidy } = usePopover();
</script>

<template>
  <div class="footer">
    <div class="attach-wrap">
      <button class="attach" title="コンテキストに追加" @click="toggleContext()">
        <Plus :size="13" />
      </button>
      <template v-if="contextOpen">
        <div class="backdrop" @click="closeContext()" />
        <ChatContextMenu class="context-popup" @close="closeContext()" />
      </template>
    </div>
    <div class="attach-wrap">
      <button class="attach" :title="t('chat.tidy.button')" @click="toggleTidy()">
        <WandSparkles :size="13" />
      </button>
      <template v-if="tidyOpen">
        <div class="backdrop" @click="closeTidy()" />
        <ChatTidyMenu class="context-popup" @close="closeTidy()" />
      </template>
    </div>
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
</template>

<style scoped>
.footer {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-shrink: 0;
}
.spacer {
  flex: 1;
}
.attach-wrap {
  position: relative;
  display: flex;
  flex-shrink: 0;
}
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 20;
}
/* フッタの上へせり上げる。入力カードは画面左寄せなので左端合わせ */
.context-popup {
  position: absolute;
  left: 0;
  bottom: calc(100% + 6px);
  z-index: 21;
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
  border-radius: 999px;
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
