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
