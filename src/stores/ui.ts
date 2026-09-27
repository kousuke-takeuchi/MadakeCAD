// エディタUIの共有状態: コマンドライン履歴、モーダル表示など。

import { defineStore } from "pinia";
import { i18n } from "../i18n";
import type { ViewClass } from "../canvas/viewClasses";
import { useChatStore } from "./chat";

/**
 * 左ドックのタブ。"chat" は chat store の panelOpen="expanded" と対で同期する。
 * "devices" = デバイスナビゲータ (spec §10)。
 */
export type LeftPanelTab = "project" | "devices" | "chat";

export const useUiStore = defineStore("ui", {
  state: () => ({
    commandHistory: [i18n.global.t("ui.welcome")] as string[],
    symbolPickerOpen: false,
    settingsOpen: false,
    leftPanelTab: "project" as LeftPanelTab,
    /**
     * チャットの入力途中テキスト。左ドックの入力欄とキャンバスの浮き入力カードは
     * 別コンポーネントなので、下書きはここに置いて双方から共有する。
     */
    chatDraft: "",
    /** 非表示中の表示クラス (レイヤ、spec §4)。画面表示のみで出力へは非反映。 */
    hiddenViewClasses: new Set<ViewClass>(),
  }),
  getters: {
    isClassVisible(state): (c: ViewClass) => boolean {
      return (c) => !state.hiddenViewClasses.has(c);
    },
    /** ステータスバーに出す直近メッセージ。 */
    lastMessage(state): string {
      return state.commandHistory[state.commandHistory.length - 1] ?? "";
    },
  },
  actions: {
    setLeftPanelTab(tab: LeftPanelTab) {
      this.leftPanelTab = tab;
    },
    setChatDraft(text: string) {
      this.chatDraft = text;
    },
    /**
     * 左ドックのエージェントタブを開く。「panelOpen="expanded" ⇔ leftPanelTab="chat"」の
     * 不変条件はこの2アクションだけが両状態を書いて維持する(呼び出し側での両建て禁止)。
     */
    openAgentTab() {
      this.leftPanelTab = "chat";
      useChatStore().setPanel("expanded");
    },
    /** エージェントタブを閉じ、浮き入力カードへ戻す。 */
    closeAgentTab() {
      if (this.leftPanelTab === "chat") this.leftPanelTab = "project";
      useChatStore().setPanel("collapsed");
    },
    toggleViewClass(c: ViewClass) {
      if (this.hiddenViewClasses.has(c)) this.hiddenViewClasses.delete(c);
      else this.hiddenViewClasses.add(c);
    },
    log(line: string) {
      this.commandHistory.push(line);
      if (this.commandHistory.length > 200) this.commandHistory.shift();
    },
  },
});
