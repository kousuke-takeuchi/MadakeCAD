// エディタUIの共有状態: コマンドライン履歴、モーダル表示など。

import { defineStore } from "pinia";
import type { ViewClass } from "../canvas/viewClasses";
import type { SheetKind } from "../ipc";
import { useChatStore } from "./chat";

/**
 * 左ドックのタブ。"chat" は chat store の panelOpen="expanded" と対で同期する。
 * "devices" = デバイスナビゲータ (spec §10)。
 * "panel" = 未配置部品 (盤レイアウトシートを開いているあいだだけ出る、M4 §8)。
 */
export type LeftPanelTab = "project" | "devices" | "panel" | "chat";

/** リボンのタブ (表示名はi18nカタログの`ribbon.tab.*`)。 */
export type RibbonTab =
  | "home"
  | "project"
  | "schematic"
  | "panel"
  | "report"
  | "io"
  | "view"
  | "admin";

export const useUiStore = defineStore("ui", {
  state: () => ({
    commandHistory: ["MadakeCAD コマンドライン (L=配線 E=削除 U=元に戻す)"] as string[],
    symbolPickerOpen: false,
    settingsOpen: false,
    leftPanelTab: "project" as LeftPanelTab,
    /** 表示中のリボンタブ。盤レイアウトシートを開くと「パネル」へ自動で移る。 */
    ribbonTab: "schematic" as RibbonTab,
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
    setRibbonTab(tab: RibbonTab) {
      this.ribbonTab = tab;
    },
    /**
     * 表示中のシートの種別に合わせてリボンと左ドックを切り替える
     * (.pen「M4デザイン - リボン パネル/レポート/管理タブ」:
     * 「パネルタブは盤レイアウトシート選択時に自動アクティブ化」)。
     *
     * 戻すときは**パネル系を開いていたときだけ**回路図・プロジェクトへ戻すので、
     * 自分で開いた「表示」タブなどは取り上げない。エージェントと会話中の左ドックも
     * そのままにする (会話を見失わないため)。
     */
    syncSheetKind(kind: SheetKind) {
      if (kind === "panel") {
        this.ribbonTab = "panel";
        if (this.leftPanelTab === "project" || this.leftPanelTab === "devices") {
          this.leftPanelTab = "panel";
        }
        return;
      }
      if (this.ribbonTab === "panel") this.ribbonTab = "schematic";
      if (this.leftPanelTab === "panel") this.leftPanelTab = "project";
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
