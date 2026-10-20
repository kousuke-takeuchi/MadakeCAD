// チャット入力フッタの使用量表示(⚡バッジとその内訳)の仕様テスト。
import { describe, it, expect } from "vitest";
import { isSendShortcut, sumUsage } from "./chatComposer";
import type { ChatMessage, Usage } from "../stores/chat";

function message(usage: Usage | null): ChatMessage {
  return {
    turn_id: "",
    role: "assistant",
    text: "",
    tool_calls: [],
    applied_revisions: { start: 0, end: 0 },
    applied_undo_depth: { start: 0, end: 0 },
    error: null,
    usage,
    streaming: false,
    undone: false,
    attachments: [],
    blocks: [],
    thinking_started_at: null,
  } as ChatMessage;
}

function usage(fields: Partial<Usage>): Usage {
  return {
    input_tokens: 0,
    output_tokens: 0,
    cache_creation_input_tokens: 0,
    cache_read_input_tokens: 0,
    total_cost_usd: null,
    ...fields,
  };
}

describe("sumUsage", () => {
  // ja: 会話のトークンはターンをまたいで足し合わされる
  it("tokens are added up across the turns of a conversation", () => {
    const total = sumUsage([
      message(usage({ input_tokens: 100, output_tokens: 20 })),
      message(usage({ input_tokens: 200, output_tokens: 30 })),
      message(null),
    ]);

    expect(total).toEqual({ input: 300, output: 50, tokens: 350, premium: 0 });
  });

  // ja: GitHub Copilotのプレミアムリクエストは小数のままトークンと別枠で数える
  it("Copilot's premium requests are counted separately from tokens, as a fraction", () => {
    const total = sumUsage([
      message(usage({ output_tokens: 166, premium_requests: 0.33 })),
      message(usage({ output_tokens: 57, premium_requests: 0.33 })),
    ]);

    expect(total.premium).toBeCloseTo(0.66, 5);
    expect(total.tokens).toBe(223);
  });

  // ja: 使用量がまだ無い会話は全て0(バッジは「1x」表示になる)
  it("a conversation with no usage yet counts as zero", () => {
    expect(sumUsage([])).toEqual({ input: 0, output: 0, tokens: 0, premium: 0 });
  });
});

describe("isSendShortcut (チャット入力の送信キー)", () => {
  const key = (over: Partial<KeyboardEvent>) =>
    ({ key: "Enter", metaKey: false, ctrlKey: false, isComposing: false, keyCode: 13, ...over }) as KeyboardEvent;

  // ja: Cmd+Enter (macOS) で送信する
  it("sends on Cmd+Enter", () => {
    expect(isSendShortcut(key({ metaKey: true }))).toBe(true);
  });

  // ja: Ctrl+Enter で送信する
  it("sends on Ctrl+Enter", () => {
    expect(isSendShortcut(key({ ctrlKey: true }))).toBe(true);
  });

  // ja: ただのEnterは送信せず、入力欄の改行になる (日本語変換の確定で誤送信しない)
  it("does not send on a plain Enter, which stays a line break", () => {
    expect(isSendShortcut(key({}))).toBe(false);
  });

  // ja: Shift+Enterも送信しない (改行)
  it("does not send on Shift+Enter", () => {
    expect(isSendShortcut(key({ shiftKey: true }))).toBe(false);
  });

  // ja: IME変換中 (isComposing) は、Cmd/Ctrlを押していても送信しない
  it("does not send while an IME composition is in progress, even with Cmd or Ctrl", () => {
    expect(isSendShortcut(key({ metaKey: true, isComposing: true }))).toBe(false);
    expect(isSendShortcut(key({ ctrlKey: true, isComposing: true }))).toBe(false);
  });

  // ja: 変換確定のEnterがkeyCode 229で届く環境 (WebKit) でも送信しない
  it("does not send when the Enter is reported as keyCode 229 by the IME", () => {
    expect(isSendShortcut(key({ metaKey: true, keyCode: 229 }))).toBe(false);
  });

  // ja: Enter以外のキーはCmd/Ctrlと一緒でも送信しない
  it("does not send on keys other than Enter", () => {
    expect(isSendShortcut(key({ key: "a", metaKey: true, keyCode: 65 }))).toBe(false);
  });
});
