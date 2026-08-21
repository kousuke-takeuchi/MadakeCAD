// エディタUIの共有状態: コマンドライン履歴、モーダル表示など。

import { defineStore } from "pinia";

/** 左ドックのタブ。"chat" は chat store の panelOpen="expanded" と対で同期する。 */
export type LeftPanelTab = "project" | "chat";

export const useUiStore = defineStore("ui", {
  state: () => ({
    commandHistory: ["MadakeCAD コマンドライン (L=配線 E=削除 U=元に戻す)"] as string[],
    symbolPickerOpen: false,
    settingsOpen: false,
    leftPanelTab: "project" as LeftPanelTab,
    /**
     * チャットの入力途中テキスト。左ドックの入力欄とキャンバスの浮き入力カードは
     * 別コンポーネントなので、下書きはここに置いて双方から共有する。
     */
    chatDraft: "",
  }),
  getters: {
    /** ステータスバーに出す直近メッセージ。 */
    lastMessage(state): string {
      return state.commandHistory[state.commandHistory.length - 1] ?? "";
    },
  },
  actions: {
    setLeftPanelTab(tab: LeftPanelTab) {
      this.leftPanelTab = tab;
    },
    log(line: string) {
      this.commandHistory.push(line);
      if (this.commandHistory.length > 200) this.commandHistory.shift();
    },
  },
});
