<script setup lang="ts">
// 左ドック (デザイン: 「左パネル(タブ式)」)。幅340px・全高。
// タブ行 + 「プロジェクト」= ProjectPanel / 「デバイス」= DevicePanel / 「エージェント」= ChatDock。
// プロジェクトとエージェントは v-show で常時マウントし、タブ切替で会話やストリーミングを
// 壊さない (デバイスタブは状態を持たないので v-if でよい)。
//
// chat.panelOpen="expanded" の意味は「左ドックのエージェントタブ表示」。
// 浮き入力カード (CanvasView の ChatPanel) の送信/展開もこの状態を立てるので、
// ここで panelOpen ⇔ leftPanelTab を同期する。
import { History, LoaderCircle, Minus } from "lucide-vue-next";
import { computed, watch } from "vue";
import { useI18n } from "vue-i18n";
import { usePopover } from "../composables/popover";
import { useChatStore } from "../stores/chat";
import { useSettingsStore } from "../stores/settings";
import { useUiStore, type LeftPanelTab } from "../stores/ui";
import ChatDock from "./chat/ChatDock.vue";
import ChatHistoryPopup from "./chat/ChatHistoryPopup.vue";
import DevicePanel from "./DevicePanel.vue";
import ProjectPanel from "./ProjectPanel.vue";

const { t } = useI18n();
const chat = useChatStore();
const ui = useUiStore();
const settings = useSettingsStore();

const { open: historyOpen, toggle: toggleHistory, close: closeHistory } = usePopover();

const tabs = computed<{ id: LeftPanelTab; label: string }[]>(() => [
  { id: "project", label: "プロジェクト" },
  // デバイスナビゲータ (spec §10): 参照記号ツリーで機能単位まで展開する
  { id: "devices", label: t("devices.tab") },
  { id: "chat", label: "エージェント" },
]);

// 接続バッジ: Claude Code CLIならCLIの検出、Anthropic APIならキーの保存状況を見る
const connected = computed(() => settings.agentReady(chat.detect !== null));
const chatTab = computed(() => ui.leftPanelTab === "chat");
// 並列エージェント: 開いていない会話も動くので、実行中の本数をタブ行に出す
const running = computed(() => chat.runningCount);

function selectTab(tab: LeftPanelTab) {
  // エージェント以外のタブへ移ったら浮き入力カードを出す (入力欄の二重表示を避ける)
  if (tab === "chat") {
    ui.openAgentTab();
    return;
  }
  ui.closeAgentTab();
  ui.setLeftPanelTab(tab);
}

function minimize() {
  selectTab("project");
}

// 浮きカードからの送信/展開など、外から panelOpen が変わった場合もタブを合わせる。
watch(
  () => chat.panelOpen,
  (state) => {
    if (state === "expanded") ui.setLeftPanelTab("chat");
    else if (ui.leftPanelTab === "chat") ui.setLeftPanelTab("project");
  },
);
</script>

<template>
  <aside class="left-panel">
    <div class="tabs" role="tablist" aria-label="左パネル">
      <button
        v-for="t in tabs"
        :key="t.id"
        class="tab"
        role="tab"
        :aria-selected="ui.leftPanelTab === t.id"
        :class="{ active: ui.leftPanelTab === t.id }"
        @click="selectTab(t.id)"
      >
        <span class="tab-label">{{ t.label }}</span>
        <span class="underline" />
      </button>
      <span class="spacer" />

      <template v-if="chatTab">
        <span class="badge" :class="{ off: !connected }">
          <span class="dot" />
          {{ connected ? "接続中" : "未接続" }}
        </span>
        <span
          v-if="running > 0"
          class="badge running"
          :title="t('chat.parallel.runningBadgeTitle', { count: running })"
        >
          <LoaderCircle class="spin" :size="10" />
          {{ t("chat.parallel.runningBadge", { count: running }) }}
        </span>
        <button
          class="icon-btn"
          title="履歴"
          :aria-expanded="historyOpen"
          @click="toggleHistory()"
        >
          <History :size="13" />
        </button>
        <button class="icon-btn" title="最小化" @click="minimize">
          <Minus :size="13" />
        </button>
      </template>

      <template v-if="historyOpen">
        <div class="backdrop" @click="closeHistory()" />
        <ChatHistoryPopup class="history-popup" @close="closeHistory()" />
      </template>
    </div>

    <ProjectPanel v-show="ui.leftPanelTab === 'project'" />
    <DevicePanel v-if="ui.leftPanelTab === 'devices'" />
    <ChatDock v-show="chatTab" />
  </aside>
</template>

<style scoped>
.left-panel {
  width: 340px;
  flex: none;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--ribbon-bg);
  border-right: 1px solid var(--ribbon-line);
  color: var(--ui-text);
  user-select: none;
}

/* タブ行 */
.tabs {
  position: relative;
  display: flex;
  align-items: center;
  gap: 2px;
  flex-shrink: 0;
  padding: 0 6px;
  background: var(--palette-head);
}
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 20;
}
/* タブ行の直下、履歴ボタン側 (右端) にぶら下げる */
.history-popup {
  position: absolute;
  top: calc(100% + 4px);
  right: 6px;
  z-index: 21;
}
.tab {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 3px;
  flex-shrink: 0;
  padding: 7px 10px 0;
  border: none;
  background: transparent;
  cursor: pointer;
}
.tab-label {
  font-size: 11px;
  font-weight: 600;
  color: var(--ui-muted);
  white-space: nowrap;
}
.tab.active .tab-label {
  font-weight: 700;
  color: var(--acad-blue);
}
.underline {
  width: 100%;
  height: 2px;
  background: transparent;
}
.tab.active .underline {
  background: var(--acad-blue);
}
.tab:hover .tab-label {
  color: var(--ui-text);
}
.tab.active:hover .tab-label {
  color: var(--acad-blue);
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
.badge {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  border-radius: 999px;
  background: var(--ok-bg);
  font-size: 10px;
  color: var(--ok-fg);
  white-space: nowrap;
}
.badge .dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--ok-fg);
}
.badge.off {
  background: var(--off-bg);
  color: var(--off-fg);
}
.badge.running {
  background: var(--sel-blue);
  color: var(--acad-blue);
  font-weight: 600;
}
.spin {
  animation: badge-spin 1s linear infinite;
}
@keyframes badge-spin {
  to {
    transform: rotate(360deg);
  }
}
.badge.off .dot {
  background: var(--off-fg);
}
</style>
