import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, beforeEach } from "vitest";
import { useUiStore } from "./ui";
import { useChatStore } from "./chat";

describe("ui store: 左ドックのタブとチャット下書き", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  // ja: 既定はプロジェクトタブ+チャット折りたたみ
  it("defaults are the project tab with the chat collapsed", () => {
    const ui = useUiStore();
    const chat = useChatStore();
    expect(ui.leftPanelTab).toBe("project");
    expect(chat.panelOpen).toBe("collapsed");
  });

  // ja: openAgentTabでエージェントタブ切替とチャット展開が同時に行われる
  it("openAgentTab switches to the agent tab and expands the chat together", () => {
    const ui = useUiStore();
    const chat = useChatStore();
    ui.openAgentTab();
    expect(ui.leftPanelTab).toBe("chat");
    expect(chat.panelOpen).toBe("expanded");
  });

  // ja: closeAgentTabでプロジェクトタブへ戻り、チャットも畳まれる
  it("closeAgentTab returns to the project tab and collapses the chat", () => {
    const ui = useUiStore();
    const chat = useChatStore();
    ui.openAgentTab();
    ui.closeAgentTab();
    expect(ui.leftPanelTab).toBe("project");
    expect(chat.panelOpen).toBe("collapsed");
  });

  // ja: チャット下書きはドックと浮きカードで共有される
  it("the chat draft is shared between the dock and the floating card", () => {
    const ui = useUiStore();
    ui.setChatDraft("24V系にヒューズを追加して");
    expect(ui.chatDraft).toBe("24V系にヒューズを追加して");
    ui.setChatDraft("");
    expect(ui.chatDraft).toBe("");
  });
});

describe("表示クラス (レイヤ)", () => {
  // ja: 既定では全表示クラスが表示される
  it("all view classes are visible by default", () => {
    const ui = useUiStore();
    expect(ui.isClassVisible("wires")).toBe(true);
    expect(ui.isClassVisible("grid")).toBe(true);
    expect(ui.isClassVisible("frame")).toBe(true);
  });

  // ja: toggleViewClassで表示クラスの非表示・再表示を切り替える
  it("toggleViewClass hides and re-shows a class", () => {
    const ui = useUiStore();
    ui.toggleViewClass("texts");
    expect(ui.isClassVisible("texts")).toBe(false);
    expect(ui.isClassVisible("wires")).toBe(true);
    ui.toggleViewClass("texts");
    expect(ui.isClassVisible("texts")).toBe(true);
  });
});

// 盤レイアウトシート (M4仕様 §8) を開いたときのリボン・左ドックの切り替え。
// .pen「M4デザイン - リボン パネル/レポート/管理タブ」の注記
// 「パネルタブは盤レイアウトシート選択時に自動アクティブ化」。
describe("シート種別に合わせた画面の切り替え", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  // ja: 起動直後のリボンは回路図タブ
  it("starts on the schematic ribbon tab", () => {
    expect(useUiStore().ribbonTab).toBe("schematic");
  });

  // ja: 盤レイアウトシートを開くとリボンのパネルタブと左ドックの未配置部品が自動で開く
  it("opens the panel ribbon tab and the unplaced parts dock for a panel sheet", () => {
    const ui = useUiStore();
    ui.syncSheetKind("panel");
    expect(ui.ribbonTab).toBe("panel");
    expect(ui.leftPanelTab).toBe("panel");
  });

  // ja: 回路図シートへ戻すと回路図タブとプロジェクトツリーへ戻る
  it("returns to the schematic tab and the project tree on a schematic sheet", () => {
    const ui = useUiStore();
    ui.syncSheetKind("panel");
    ui.syncSheetKind("schematic");
    expect(ui.ribbonTab).toBe("schematic");
    expect(ui.leftPanelTab).toBe("project");
  });

  // ja: 自分で開いた表示タブは盤シートを開いても勝手に切り替わらない
  it("leaves a ribbon tab the user chose after the panel tab is once shown", () => {
    const ui = useUiStore();
    ui.syncSheetKind("panel");
    ui.setRibbonTab("view");
    ui.syncSheetKind("schematic");
    expect(ui.ribbonTab).toBe("view");
  });

  // ja: エージェントと会話中は左ドックのタブを取り上げない
  it("does not steal the left dock from an open agent conversation", () => {
    const ui = useUiStore();
    ui.openAgentTab();
    ui.syncSheetKind("panel");
    expect(ui.ribbonTab).toBe("panel");
    expect(ui.leftPanelTab).toBe("chat");
  });
});
