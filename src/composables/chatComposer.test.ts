// チャット入力フッタの使用量表示(⚡バッジとその内訳)の仕様テスト。
import { describe, it, expect } from "vitest";
import { sumUsage } from "./chatComposer";
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
