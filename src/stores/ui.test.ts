import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, beforeEach } from "vitest";
import { useUiStore } from "./ui";
import { useChatStore } from "./chat";

describe("ui store: 左ドックのタブとチャット下書き", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("既定はプロジェクトタブ + チャット折りたたみ", () => {
    const ui = useUiStore();
    const chat = useChatStore();
    expect(ui.leftPanelTab).toBe("project");
    expect(chat.panelOpen).toBe("collapsed");
  });

  it("openAgentTabでエージェントタブとpanelOpen=expandedが同時に立つ", () => {
    const ui = useUiStore();
    const chat = useChatStore();
    ui.openAgentTab();
    expect(ui.leftPanelTab).toBe("chat");
    expect(chat.panelOpen).toBe("expanded");
  });

  it("closeAgentTabでプロジェクトタブへ戻り、panelOpenも畳まれる", () => {
    const ui = useUiStore();
    const chat = useChatStore();
    ui.openAgentTab();
    ui.closeAgentTab();
    expect(ui.leftPanelTab).toBe("project");
    expect(chat.panelOpen).toBe("collapsed");
  });

  it("chatDraftはsetChatDraftで更新され、ドックと浮きカードで共有される", () => {
    const ui = useUiStore();
    ui.setChatDraft("24V系にヒューズを追加して");
    expect(ui.chatDraft).toBe("24V系にヒューズを追加して");
    ui.setChatDraft("");
    expect(ui.chatDraft).toBe("");
  });
});

describe("表示クラス (レイヤ)", () => {
  it("既定は全クラス表示", () => {
    const ui = useUiStore();
    expect(ui.isClassVisible("wires")).toBe(true);
    expect(ui.isClassVisible("grid")).toBe(true);
    expect(ui.isClassVisible("frame")).toBe(true);
  });

  it("toggleViewClassで非表示と再表示を切り替える", () => {
    const ui = useUiStore();
    ui.toggleViewClass("texts");
    expect(ui.isClassVisible("texts")).toBe(false);
    expect(ui.isClassVisible("wires")).toBe(true);
    ui.toggleViewClass("texts");
    expect(ui.isClassVisible("texts")).toBe(true);
  });
});
