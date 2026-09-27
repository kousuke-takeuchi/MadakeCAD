<script setup lang="ts">
// キャンバス左下の浮き入力カード (デザイン: 「AIチャットパネル」折りたたみ状態)。
// 展開状態の会話は左ドックのエージェントタブ (LeftPanel > ChatDock) が持つので、
// このカードは chat.panelOpen==="collapsed" の間だけ CanvasView が描く。
// 最小化はローカル状態。図面の編集は一切行わない。
import { Maximize2, Minus, SquarePen } from "lucide-vue-next";
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useChatComposer } from "../../composables/chatComposer";
import ChatComposerFooter from "./ChatComposerFooter.vue";

const { store, ui, draft, onKeydown } = useChatComposer();
const { t } = useI18n();

const minimized = ref(false);

const title = computed(() => (store.messages.length ? t("chat.panel.titleActive") : t("chat.panel.titleNew")));

/** 左ドックのエージェントタブを開く。 */
function expand() {
  minimized.value = false;
  ui.openAgentTab();
}
</script>

<template>
  <div class="chat-overlay">
    <!-- 最小化: タイトルバーのみ -->
    <div v-if="minimized" class="panel minimized">
      <SquarePen :size="14" class="head-icon" />
      <span class="head-title">{{ title }}</span>
      <span class="spacer" />
      <button class="icon-btn" :title="t('chat.panel.expand')" @click="expand"><Maximize2 :size="12" /></button>
    </div>

    <!-- 折りたたみ -->
    <div v-else class="panel collapsed">
      <div class="head">
        <SquarePen :size="14" class="head-icon" />
        <span class="head-title">{{ title }}</span>
        <span class="spacer" />
        <button class="icon-btn" :title="t('chat.panel.minimize')" @click="minimized = true">
          <Minus :size="13" />
        </button>
        <button class="icon-btn" :title="t('chat.panel.expand')" @click="expand"><Maximize2 :size="12" /></button>
      </div>

      <textarea
        v-model="draft"
        class="input"
        :placeholder="t('chat.panel.placeholder')"
        @keydown="onKeydown"
      />

      <ChatComposerFooter />
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

/* ヘッダ */
.head {
  display: flex;
  align-items: center;
  gap: 7px;
  flex-shrink: 0;
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

/* 入力 */
.input {
  flex: 1;
  min-height: 0;
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
.input::placeholder {
  color: var(--ui-placeholder);
}
</style>
