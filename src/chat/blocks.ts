// エージェント応答の表示の判断 (.pen「AIチャット 作業過程(折りたたみ)/(展開)」)。
// DOMを持たない純粋な関数にして、まとまりの分け方・開閉・文言の数字をテストで固める。
import type { ChatMessage, ChatToolCall } from "../stores/chat";

export type DisplayItem =
  | { kind: "text"; key: string; text: string }
  | { kind: "thinking"; key: string; text: string; duration_ms: number | null }
  | { kind: "tools"; key: string; calls: ChatToolCall[]; failed: boolean; running: boolean };

/** 応答を表示の行に分ける。続けて並んだツールは1つのまとまりにする。ブロックの無い古い応答は本文→ツールの順。 */
export function displayItems(message: ChatMessage): DisplayItem[] {
  const blocks = message.blocks.length
    ? message.blocks
    : [
        ...(message.text ? [{ type: "text" as const, text: message.text }] : []),
        ...message.tool_calls.map((c) => ({ type: "tool" as const, id: c.id })),
      ];
  const items: DisplayItem[] = [];
  blocks.forEach((block, index) => {
    // 位置から作るので、応答中にブロックが足されても前の行のkeyは変わらない (利用者の開閉が保たれる)
    const key = `${block.type}-${index}`;
    if (block.type === "text") {
      items.push({ kind: "text", key, text: block.text });
    } else if (block.type === "thinking") {
      items.push({ kind: "thinking", key, text: block.text, duration_ms: block.duration_ms });
    } else {
      const found = message.tool_calls.find((c) => c.id === block.id);
      if (!found) return;
      const last = items[items.length - 1];
      if (last?.kind === "tools") last.calls.push(found);
      else items.push({ kind: "tools", key, calls: [found], failed: false, running: false });
    }
  });
  for (const item of items) {
    if (item.kind !== "tools") continue;
    item.failed = item.calls.some((c) => c.status === "error");
    item.running = item.calls.some((c) => c.status === "running");
  }
  return items;
}

/** 行が開いているか。利用者の選択が最優先。無ければ、応答中の最後の行 (思考・ツール) だけを開く。 */
export function isOpen(
  key: string,
  items: DisplayItem[],
  streaming: boolean,
  overrides: Record<string, boolean>,
): boolean {
  if (key in overrides) return overrides[key];
  return streaming && items[items.length - 1]?.key === key;
}

/** 思考の秒数 (四捨五入)。1秒未満・不明は秒数を出さないのでnull。 */
export function thinkingSeconds(duration_ms: number | null): number | null {
  if (duration_ms === null || duration_ms < 1000) return null;
  return Math.round(duration_ms / 1000);
}

/** 切り詰めて省いた量 (KB、切り上げ)。省いていなければnull。 */
export function omittedKb(call: ChatToolCall): number | null {
  if (call.output === null || call.output_size === null) return null;
  const kept = new TextEncoder().encode(call.output).length;
  const omitted = call.output_size - kept;
  return omitted > 0 ? Math.ceil(omitted / 1024) : null;
}

/** ツールの入力の表示 (文字列はそのまま、それ以外は2字下げのJSON)。 */
export function prettyInput(input: unknown): string {
  return typeof input === "string" ? input : JSON.stringify(input, null, 2);
}
