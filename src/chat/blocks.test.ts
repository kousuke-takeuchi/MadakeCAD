import { describe, expect, it } from "vitest";
import { displayItems, isOpen, omittedKb, prettyInput, thinkingSeconds } from "./blocks";
import type { ChatMessage, ChatToolCall } from "../stores/chat";

const call = (id: string, status: ChatToolCall["status"] = "ok", extra: Partial<ChatToolCall> = {}): ChatToolCall => ({
  id, tool: "mcp__madakecad__place_symbol", input: {}, status, summary: "", output: null, output_size: null, ...extra,
});
const msg = (over: Partial<ChatMessage>): ChatMessage => ({
  turn_id: "", role: "assistant", text: "", tool_calls: [], blocks: [], thinking_started_at: null,
  applied_revisions: { start: 0, end: 0 }, applied_undo_depth: { start: 0, end: 0 },
  error: null, usage: null, streaming: false, undone: false, attachments: [], ...over,
});

describe("agent reply display", () => {
  // ja: 続けて並んだツールは1つのまとまりになり、本文か思考が来るとまとまりが切れる。失敗や実行中の手順があればまとまりにその印が付く
  it("groups consecutive tool calls into one run that text or thinking ends, flagging failures and running steps", () => {
    const m = msg({
      tool_calls: [call("a"), call("b", "error"), call("c", "running")],
      blocks: [
        { type: "thinking", text: "t", duration_ms: 2000 },
        { type: "tool", id: "a" },
        { type: "tool", id: "b" },
        { type: "text", text: "途中" },
        { type: "tool", id: "c" },
      ],
    });
    const items = displayItems(m);
    expect(items.map((i) => i.kind)).toEqual(["thinking", "tools", "text", "tools"]);
    const [, first, , second] = items;
    expect(first.kind === "tools" && first.calls.map((c) => c.id)).toEqual(["a", "b"]);
    expect(first.kind === "tools" && first.failed).toBe(true);
    expect(second.kind === "tools" && second.running).toBe(true);
  });

  // ja: ブロックの無い古いメッセージは、本文のあとにツールを1つのまとまりで並べる
  it("shows a message without blocks the old way: the text, then its tools as one run", () => {
    const items = displayItems(msg({ text: "済み", tool_calls: [call("a"), call("b")] }));
    expect(items.map((i) => i.kind)).toEqual(["text", "tools"]);
  });

  // ja: 応答中は最後の行だけが開き、履歴の会話や終わったターンではどの行も閉じている。利用者が開閉した行はその状態を保つ
  it("opens only the last row while streaming, keeps finished and loaded replies closed, and honours the user's choice", () => {
    const items = displayItems(msg({
      tool_calls: [call("a"), call("b", "running")],
      blocks: [{ type: "tool", id: "a" }, { type: "text", text: "x" }, { type: "tool", id: "b" }],
    }));
    const [firstTools, , lastTools] = items;
    expect(isOpen(lastTools.key, items, true, {})).toBe(true);
    expect(isOpen(firstTools.key, items, true, {})).toBe(false);
    expect(isOpen(lastTools.key, items, false, {})).toBe(false);
    expect(isOpen(firstTools.key, items, false, { [firstTools.key]: true })).toBe(true);
    expect(isOpen(lastTools.key, items, true, { [lastTools.key]: false })).toBe(false);
  });

  // ja: 思考の時間は秒に丸め、1秒未満や不明なら秒数を出さない
  it("rounds thinking time to seconds and hides it below one second or when unknown", () => {
    expect(thinkingSeconds(4300)).toBe(4);
    expect(thinkingSeconds(999)).toBeNull();
    expect(thinkingSeconds(null)).toBeNull();
  });

  // ja: 切り詰めた結果は省いた量をKB (切り上げ) で示し、全部あるときは何も示さない
  it("reports how many KB of a truncated result were left out, and nothing when it is complete", () => {
    expect(omittedKb(call("a", "ok", { output: "x".repeat(8192), output_size: 8192 + 1500 }))).toBe(2);
    expect(omittedKb(call("a", "ok", { output: "あ", output_size: 3 }))).toBeNull();
    expect(omittedKb(call("a"))).toBeNull();
  });

  // ja: ツールの入力は2字下げのJSONで表示し、文字列はそのまま出す
  it("pretty-prints tool input as two-space JSON and shows strings as they are", () => {
    expect(prettyInput({ ref: "F2" })).toBe('{\n  "ref": "F2"\n}');
    expect(prettyInput("echo hi")).toBe("echo hi");
  });
});
