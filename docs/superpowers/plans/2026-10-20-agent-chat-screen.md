# エージェントチャット段階1 PR B (画面) 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** PR A で会話に残るようになった「起きた順のブロック・ツールの結果・思考」を、Claude風の折りたたみ表示とMarkdownで画面に出す。

**Architecture:** 状態はPiniaの`chat`ストアが`AgentEvent`を畳み込んで持つ (Rustと同じ規則で`blocks`を組み立てる)。表示の判断 (まとまりへの分け方・開閉・時間や省略の文言・Markdownの安全なHTML化) は、DOMの要らない純粋なTSモジュール`src/chat/`に置いてVitestで仕様を固める (このリポジトリにはコンポーネントのテスト環境が無い)。Vueの部品は、それらを呼んで描くだけにする。

**Tech Stack:** Vue 3 + TypeScript + Pinia、Vitest (node環境)、markdown-it 15.0.2 (型同梱)、`@tauri-apps/plugin-opener`

**Spec:** `docs/superpowers/specs/2026-10-20-agent-chat-display-design.md` (日本語版 `.ja.md`)。見た目は`MadakeCAD.pen`の`AIチャット 作業過程(折りたたみ)`・`AIチャット 作業過程(展開)` (このブランチの`design:`コミット)。

## Global Constraints

- `markdown-it`は**ちょうど`15.0.2`** (`npm install --save-exact markdown-it@15.0.2`)。15.0.3は公開から日が浅いので使わない。型は同梱なので`@types/markdown-it`は入れない (設計書の「`@types/markdown-it`を開発用に入れる」は不要になった。理由: 15系は`dist/markdown-it.d.mts`を同梱)。
- Markdownの安全: `html: false`。リンクは`http:`/`https:`だけを残し、押したら`openUrl` (`@tauri-apps/plugin-opener`) でOSのブラウザで開き、webviewは移動させない。画像`![alt](url)`は画像として読み込まず、altを表示したリンクにする。`javascript:`・`file:`・`data:`はリンクにしない。
- 応答中は今まとまりを受けている行 (思考・ツール) を開き、ターンが終わったら閉じる。利用者が開閉した行はパネルを開いている間その状態を保つ (保存しない)。失敗した手順を含むまとまりは、閉じていても赤い行にする。
- `blocks`の無い古いメッセージは、本文のあとにツールの手順を並べる旧来の順で出す。
- 思考の時間は1秒未満なら秒数を出さない (「考えました」)。PR Aの最終レビューで残した指摘 (1回で届いた思考が「0秒」になる) への対応。
- ツールの結果が切り詰められているとき (`output_size`が`output`のバイト数より大きい) は「…残り N KB は省略」。Nは切り上げのKB。
- 文言は6つのロケール (`src/locales/{ja,en,zh,es,fr,de}.json`) すべてに足す。zh/es/fr/deは英語と同じ文でよい (既存の`chat.attach`と同じ扱い)。
- Vitestのテストは`it("英語の仕様文")`+直前の行に`// ja: 日本語の仕様文`。最後に`python3 scripts/gen_spec.py`で仕様書を作り直す。
- コミットにAIの共著者行を付けない。PRは`master`向け。

## Review Focus

- **応答中に本文・ツール・本文と種類が入れ替わる**: 本文の前後で別のまとまりになり、前のまとまりは閉じ、今のまとまりだけが開く。→ Task 2 `groupBlocks`/`isOpen`のテスト
- **履歴から開いた会話 (`streaming: false`)**: どの行も閉じて始まる。→ Task 2
- **`blocks`の無い古いメッセージ**: 本文→ツールの順で出て、ツールは1つのまとまりになる。→ Task 2 `legacyBlocks`
- **悪意のあるMarkdown** (`<img onerror>`・`[x](javascript:alert(1))`・`![x](https://evil/track.png)`): 生HTMLは文字、危険なリンクは文字、画像は読み込まないリンク。→ Task 3
- **閉じていないコードブロックが届いている途中**: 例外を出さず、届いた分までをコードとして描く。→ Task 3

---

### Task 1: チャットストアがブロック・結果・思考を持つ

**Files:**
- Modify: `src/stores/chat.ts` (型`AgentEvent`・`WireToolCall`・`WireChatMessage`・`ChatToolCall`・`ChatMessage`、`newMessage`・`normalizeToolCall`・`normalizeMessage`、`applyAgentEvent`)
- Test: `src/stores/chat.test.ts`

**Interfaces:**
- Produces (`src/stores/chat.ts`からexport):
  ```ts
  export type ChatBlock =
    | { type: "text"; text: string }
    | { type: "thinking"; text: string; duration_ms: number | null }
    | { type: "tool"; id: string };
  // ChatToolCall に追加: output: string | null; output_size: number | null
  // ChatMessage に追加: blocks: ChatBlock[]
  // AgentEvent に追加: { type: "thinking_delta"; text: string }、tool_use_finished に output?: string | null
  ```

- [ ] **Step 1: 失敗するテストを書く** (`src/stores/chat.test.ts`の`describe`の中、既存の「ツール呼び出しはチップになり」テストの後)

```ts
  // ja: 応答は起きた順のブロックとして組み立てられ、同じ種類が続く間は1つにまとまり、思考には最初から最後の差分までの時間が付く
  it("builds the reply as blocks in the order they happened, joining runs of one kind and timing the thinking", () => {
    const store = useChatStore();
    seed(store);
    const now = vi.spyOn(Date, "now");
    now.mockReturnValue(1_000);
    feed(store, [{ type: "thinking_delta", text: "24V系を" }]);
    now.mockReturnValue(1_400);
    feed(store, [
      { type: "thinking_delta", text: "確かめる" },
      { type: "text_delta", text: "確認します。" },
      { type: "tool_use_started", id: "t1", tool: "mcp__madakecad__get_netlist", input: {} },
      { type: "tool_use_finished", id: "t1", tool: "mcp__madakecad__get_netlist", is_error: false, output: '{"nets":3}' },
      { type: "text_delta", text: "完了" },
    ]);
    now.mockRestore();
    const m = store.messages[store.messages.length - 1];
    expect(m.blocks).toEqual([
      { type: "thinking", text: "24V系を確かめる", duration_ms: 400 },
      { type: "text", text: "確認します。" },
      { type: "tool", id: "t1" },
      { type: "text", text: "完了" },
    ]);
    expect(m.text).toBe("確認します。完了");
    expect(m.tool_calls[0].output).toBe('{"nets":3}');
  });

  // ja: 会話一覧から読み込んだメッセージはブロックとツールの結果を持ち、知らない種類のブロックは捨てる
  it("messages loaded from the conversation list keep their blocks and tool results, dropping block kinds it does not know", () => {
    const conv = normalizeConversation({
      id: CONV,
      session_id: null,
      model: null,
      updated_at: 0,
      messages: [
        {
          turn_id: TURN,
          role: "assistant",
          text: "済み",
          tool_calls: [
            { id: "t1", tool: "mcp__madakecad__get_project", input: {}, finished: true, is_error: false, output: "{}", output_size: 9000 },
          ],
          blocks: [
            { type: "thinking", text: "考え", duration_ms: 1500 },
            { type: "unknown" },
            { type: "tool", id: "t1" },
            { type: "text", text: "済み" },
          ],
          applied_revisions: { start: 0, end: 0 },
          applied_undo_depth: { start: 0, end: 0 },
          error: null,
        },
      ],
    } as never);
    const m = conv.messages[0];
    expect(m.blocks.map((b) => b.type)).toEqual(["thinking", "tool", "text"]);
    expect(m.tool_calls[0]).toMatchObject({ output: "{}", output_size: 9000 });
  });
```

`seed(store)`は既存の補助関数 (会話を1つ用意して開く)。`normalizeConversation`の引数の型が合わない場合は既存テスト (`normalizeConversation`を使うもの) の書き方に合わせる。

- [ ] **Step 2: 失敗を確かめる**

Run: `npx vitest run src/stores/chat.test.ts`
Expected: 新しい2件が失敗 (`blocks`が無い・`thinking_delta`が型エラー)

- [ ] **Step 3: 実装する**

型:
```ts
  | { type: "thinking_delta"; text: string }
  | { type: "tool_use_finished"; id: string; tool: string; is_error: boolean; output?: string | null }
```
`WireToolCall`に`output?: string | null; output_size?: number | null;`、`WireChatMessage`に`blocks?: Array<{ type: string; [key: string]: unknown }>;`。`ChatToolCall`に`output: string | null; output_size: number | null;`、`ChatMessage`に`blocks: ChatBlock[];`と、思考の時間を測るための`thinking_started_at: number | null;` (表示しない作業用)。

`newMessage`に`blocks: [], thinking_started_at: null`。`normalizeToolCall`に`output: call.output ?? null, output_size: call.output_size ?? null`。`normalizeMessage`に:
```ts
    blocks: (message.blocks ?? []).flatMap(toChatBlock),
    thinking_started_at: null,
```
```ts
/** 保存されたブロックを表示用へ。知らない種類 (`unknown`や後の版で足された種類) は捨てる。 */
function toChatBlock(block: { type: string; [key: string]: unknown }): ChatBlock[] {
  if (block.type === "text" && typeof block.text === "string") return [{ type: "text", text: block.text }];
  if (block.type === "thinking" && typeof block.text === "string") {
    return [{ type: "thinking", text: block.text, duration_ms: typeof block.duration_ms === "number" ? block.duration_ms : null }];
  }
  if (block.type === "tool" && typeof block.id === "string") return [{ type: "tool", id: block.id }];
  return [];
}
```

`applyAgentEvent`:
- `text_delta`: `const turn = this.openTurn(conv); turn.text += event.text; pushText(turn, event.text);`
- 新しい`case "thinking_delta": pushThinking(this.openTurn(conv), event.text, Date.now()); break;`
- `tool_use_started`: 重複チェックの後、push する呼び出しに`output: null, output_size: null`を足し、`turn.thinking_started_at = null; turn.blocks.push({ type: "tool", id: event.id });`
- `tool_use_finished`: `if (call) { call.status = ...; if (event.output != null) { call.output = event.output; call.output_size = new TextEncoder().encode(event.output).length; } }` (画面のミラーなので切り詰めはしない。正はRust側で、再取得で置き換わる)
- `turn_completed`: `if (!turn.text) { turn.text = event.result; if (event.result) pushText(turn, event.result); }`、`turn.thinking_started_at = null`

ファイル末尾の補助関数群に:
```ts
/** 本文の差分を最後の本文ブロックへ足す (最後が本文でなければ新しく作る)。Rust `ChatMessage::push_text`と同じ規則。 */
function pushText(turn: ChatMessage, text: string) {
  turn.thinking_started_at = null;
  const last = turn.blocks[turn.blocks.length - 1];
  if (last?.type === "text") last.text += text;
  else turn.blocks.push({ type: "text", text });
}

/** 思考の差分を最後の思考ブロックへ足し、時間を測り直す (空は無視)。Rust `push_thinking`と同じ規則。 */
function pushThinking(turn: ChatMessage, text: string, now: number) {
  if (!text) return;
  if (turn.thinking_started_at === null) turn.thinking_started_at = now;
  const duration = now - turn.thinking_started_at;
  const last = turn.blocks[turn.blocks.length - 1];
  if (last?.type === "thinking") {
    last.text += text;
    last.duration_ms = duration;
  } else {
    turn.blocks.push({ type: "thinking", text, duration_ms: duration });
  }
}
```

`ChatMessage`を作っている他の場所 (テストの補助関数・`sendPrompt`のユーザー発言など) が型エラーになったら、`blocks: []`と`thinking_started_at: null`を足す (`npx vue-tsc --noEmit`で全部洗い出す)。

- [ ] **Step 4: 通ることを確かめる**

Run: `npx vitest run src/stores/chat.test.ts && npx vue-tsc --noEmit`
Expected: すべて通る

- [ ] **Step 5: コミット**

```bash
git add src/stores
git commit -m "feat(chat): ストアが応答のブロック・ツールの結果・思考を持つ"
```

---

### Task 2: 表示の判断 (まとまり・開閉・文言) を純粋な関数にする

**Files:**
- Create: `src/chat/blocks.ts`
- Test: `src/chat/blocks.test.ts`

**Interfaces:**
- Consumes: Task 1 の`ChatBlock`、`ChatMessage`、`ChatToolCall` (`../stores/chat`)
- Produces:
  ```ts
  export type DisplayItem =
    | { kind: "text"; key: string; text: string }
    | { kind: "thinking"; key: string; text: string; duration_ms: number | null }
    | { kind: "tools"; key: string; calls: ChatToolCall[]; failed: boolean; running: boolean };
  export function displayItems(message: ChatMessage): DisplayItem[];
  export function isOpen(key: string, items: DisplayItem[], streaming: boolean, overrides: Record<string, boolean>): boolean;
  export function thinkingSeconds(duration_ms: number | null): number | null; // 1秒未満・不明はnull
  export function omittedKb(call: ChatToolCall): number | null;           // 省略が無ければnull
  export function prettyInput(input: unknown): string;                     // 整形したJSON
  ```

- [ ] **Step 1: 失敗するテストを書く** (`src/chat/blocks.test.ts`)

```ts
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
```

- [ ] **Step 2: 失敗を確かめる**

Run: `npx vitest run src/chat/blocks.test.ts`
Expected: FAIL (`./blocks`が無い)

- [ ] **Step 3: 実装する** (`src/chat/blocks.ts`)

```ts
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
export function isOpen(key: string, items: DisplayItem[], streaming: boolean, overrides: Record<string, boolean>): boolean {
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
```

`thinking-0`のように`key`は位置から作るので、応答中にブロックが足されても前の行の`key`は変わらない (利用者の開閉が保たれる)。

- [ ] **Step 4: 通ることを確かめる**

Run: `npx vitest run src/chat/blocks.test.ts && npx vue-tsc --noEmit`
Expected: 通る

- [ ] **Step 5: コミット**

```bash
git add src/chat
git commit -m "feat(chat): 作業過程の表示の判断 (まとまり・開閉・時間・省略) を関数にする"
```

---

### Task 3: Markdownを安全なHTMLにする

**Files:**
- Modify: `package.json`、`package-lock.json` (markdown-it 15.0.2)
- Create: `src/chat/markdown.ts`
- Test: `src/chat/markdown.test.ts`

**Interfaces:**
- Produces:
  ```ts
  export function renderMarkdown(text: string): string;          // 安全なHTML
  export function safeExternalUrl(href: string | null): string | null; // http(s)だけ通す
  ```
  コードブロックは`<div class="md-code"><div class="md-code-head"><span class="md-lang">LANG</span><button type="button" class="md-copy" data-copy>COPY_LABEL</button></div><pre><code>…</code></pre></div>`の形で出す。`COPY_LABEL`は引数で受ける: `renderMarkdown(text, { copyLabel })`。

- [ ] **Step 1: 依存を入れる**

Run: `npm install --save-exact markdown-it@15.0.2`
Expected: `package.json`の`dependencies`に`"markdown-it": "15.0.2"` (`^`なし)。`node_modules`が本体チェックアウトへのシンボリックリンクの場合は、本体側の`node_modules`に入る (それでよい。コミットするのは`package.json`と`package-lock.json`だけ)。

`node_modules/markdown-it/dist/markdown-it.d.mts`を開き、`validateLink`・`renderer.rules`・`utils.escapeHtml`・`Token.attrGet`の名前が下のコードどおりにあることを確かめる (違えば合わせ、報告に書く)。

- [ ] **Step 2: 失敗するテストを書く** (`src/chat/markdown.test.ts`)

```ts
import { describe, expect, it } from "vitest";
import { renderMarkdown, safeExternalUrl } from "./markdown";

const md = (text: string) => renderMarkdown(text, { copyLabel: "コピー" });

describe("reply markdown", () => {
  // ja: 見出し・箇条書き・表・強調・インラインコード・引用・区切り線を描画する
  it("renders headings, lists, tables, emphasis, inline code, quotes and rules", () => {
    const html = md("## 変更\n\n- F2\n- W12\n\n| 電線 | 色 |\n|---|---|\n| W12 | 赤 |\n\n**太字** と `code`\n\n> 注意\n\n---");
    for (const tag of ["<h2>", "<ul>", "<table>", "<strong>", "<code>", "<blockquote>", "<hr>"]) {
      expect(html).toContain(tag);
    }
  });

  // ja: コードブロックには言語名とコピーボタンが付き、閉じていないコードブロックも届いた分までをコードとして描く
  it("gives code blocks a language label and a copy button, and draws an unclosed block up to what has arrived", () => {
    const html = md('```json\n{ "ref": "F2" }\n```');
    expect(html).toContain('<span class="md-lang">json</span>');
    expect(html).toContain("data-copy");
    expect(html).toContain("コピー");
    expect(md("```ts\nconst a = 1;")).toContain("const a = 1;");
  });

  // ja: 返答の生のHTMLは動かさず文字として表示する
  it("shows raw HTML in a reply as text instead of running it", () => {
    const html = md('<img src=x onerror="alert(1)"><script>alert(1)</script>');
    expect(html).not.toContain("<img");
    expect(html).not.toContain("<script");
    expect(html).toContain("&lt;img");
  });

  // ja: リンクはhttp(s)だけをリンクにし、javascript:・file:・data:はただの文字にする
  it("keeps only http(s) links and turns javascript:, file: and data: links into plain text", () => {
    expect(md("[docs](https://example.com/a)")).toContain('href="https://example.com/a"');
    for (const bad of ["javascript:alert(1)", "file:///etc/passwd", "data:text/html,x"]) {
      const html = md(`[x](${bad})`);
      expect(html).not.toContain("<a ");
      expect(html).not.toContain(`href="${bad}`);
    }
  });

  // ja: 画像は読み込まず、代替文字を表示したリンクにする
  it("does not load images; an image becomes a link labelled with its alt text", () => {
    const html = md("![回路図](https://example.com/track.png)");
    expect(html).not.toContain("<img");
    expect(html).toContain('href="https://example.com/track.png"');
    expect(html).toContain("回路図");
  });

  // ja: 外部で開いてよいURLはhttp(s)だけ
  it("treats only http(s) URLs as safe to open externally", () => {
    expect(safeExternalUrl("https://example.com")).toBe("https://example.com");
    expect(safeExternalUrl("HTTP://example.com")).toBe("HTTP://example.com");
    expect(safeExternalUrl("javascript:alert(1)")).toBeNull();
    expect(safeExternalUrl(null)).toBeNull();
  });
});
```

- [ ] **Step 3: 失敗を確かめる**

Run: `npx vitest run src/chat/markdown.test.ts`
Expected: FAIL (`./markdown`が無い)

- [ ] **Step 4: 実装する** (`src/chat/markdown.ts`)

```ts
// 返答のMarkdownを安全なHTMLにする (設計書 4. Markdown)。
// 画面にはCSPが無いので、モデルの出力をHTMLとして動かさない: 生HTMLは文字、リンクはhttp(s)だけ、
// 画像は読み込まずリンクにする。リンクを押したときの扱い (OSのブラウザで開く) は部品側。
import MarkdownIt from "markdown-it";

/** 外部 (OSのブラウザ) で開いてよいURLか。http(s)だけ通す。 */
export function safeExternalUrl(href: string | null): string | null {
  return href && /^https?:\/\//i.test(href.trim()) ? href : null;
}

const md = new MarkdownIt({ html: false, linkify: false, breaks: true });

// http(s)以外のリンクはリンクとして認めない (markdown-itは認めないリンクを文字のまま残す)
md.validateLink = (url: string) => safeExternalUrl(url) !== null;

// 画像は読み込まず、altを表示したリンクにする
md.renderer.rules.image = (tokens, idx) => {
  const token = tokens[idx];
  const src = token.attrGet("src") ?? "";
  const alt = md.utils.escapeHtml(token.content || src);
  return `<a href="${md.utils.escapeHtml(src)}">${alt}</a>`;
};

let copyLabel = "Copy";

// コードブロック: 言語名とコピーボタンの見出しを付ける (中身はmarkdown-itがエスケープ済み)
md.renderer.rules.fence = (tokens, idx) => {
  const token = tokens[idx];
  const lang = md.utils.escapeHtml(token.info.trim().split(/\s+/)[0] ?? "");
  const code = md.utils.escapeHtml(token.content);
  return (
    `<div class="md-code"><div class="md-code-head"><span class="md-lang">${lang}</span>` +
    `<button type="button" class="md-copy" data-copy>${md.utils.escapeHtml(copyLabel)}</button></div>` +
    `<pre><code>${code}</code></pre></div>\n`
  );
};

export function renderMarkdown(text: string, options: { copyLabel: string }): string {
  copyLabel = options.copyLabel;
  return md.render(text);
}
```

`breaks: true`は、今の`pre-wrap`表示と同じく1つの改行を改行として出すため。

- [ ] **Step 5: 通ることを確かめる**

Run: `npx vitest run src/chat/markdown.test.ts && npx vue-tsc --noEmit`
Expected: 通る

- [ ] **Step 6: コミット**

```bash
git add package.json package-lock.json src/chat/markdown.ts src/chat/markdown.test.ts
git commit -m "feat(chat): 返答のMarkdownを安全なHTMLにする (markdown-it 15.0.2)"
```

---

### Task 4: 画面の部品と文言

**Files:**
- Create: `src/components/chat/ChatMarkdown.vue`、`ThinkingRow.vue`、`ToolGroup.vue`、`ToolStep.vue`
- Modify: `src/components/chat/ChatMessage.vue`
- Delete: `src/components/chat/ToolChip.vue`
- Modify: `src/locales/{ja,en,zh,es,fr,de}.json` (`chat.message`に追加)

**Interfaces:**
- Consumes: Task 2 の`displayItems`・`isOpen`・`thinkingSeconds`・`omittedKb`・`prettyInput`、Task 3 の`renderMarkdown`・`safeExternalUrl`、`openUrl` (`@tauri-apps/plugin-opener`)、`inTauri` (`../../ipc`)

- [ ] **Step 1: 文言を足す**

`chat.message`に次のキーを足す (ja / en。zh・es・fr・deはenと同じ文):

| キー | ja | en |
|---|---|---|
| `thought` | `{s}秒考えました` | `Thought for {s} s` |
| `thoughtShort` | `考えました` | `Thought` |
| `thinking` | `考えています…` | `Thinking…` |
| `usedTools` | `ツールを{n}件使いました` | `Used {n} tool \| Used {n} tools` |
| `usingTools` | `ツールを使っています ({n}件)` | `Using tools ({n})` |
| `input` | `入力` | `Input` |
| `result` | `結果` | `Result` |
| `noResult` | `結果はありません` | `No result` |
| `omitted` | `…残り {kb} KB は省略` | `… {kb} KB more not shown` |
| `copy` | `コピー` | `Copy` |
| `copied` | `コピーしました` | `Copied` |

- [ ] **Step 2: 部品を作る**

見た目は`.pen`の見本どおり: 折りたたみ行は枠なし・11px・`--ui-muted`の文字+lucideアイコン (思考`Brain`、ツール`Wrench`) +開閉の矢印 (`ChevronRight`/`ChevronDown`)。開いた中身は左に2pxの`--ribbon-line`の線と12pxの字下げ。失敗したまとまりの行は`--err-fg`。入力・結果の欄は`--hover-bg`の地・6pxの角丸・見出し行 (ラベルとコピーのアイコン) +等幅10pxの本文 (`white-space: pre-wrap`、縦に長い結果は`max-height: 240px; overflow: auto`)。

`ChatMarkdown.vue`:
```vue
<script setup lang="ts">
// 返答のMarkdown (.pen「AIチャット 作業過程」Markdown本文)。HTMLは renderMarkdown が安全に作ったものだけを入れる。
// リンクはwebviewを移動させずOSのブラウザで開き、コードブロックのボタンは中のコードをコピーする。
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { openUrl } from "@tauri-apps/plugin-opener";
import { renderMarkdown, safeExternalUrl } from "../../chat/markdown";
import { inTauri } from "../../ipc";

const props = defineProps<{ text: string }>();
const { t } = useI18n();
const html = computed(() => renderMarkdown(props.text, { copyLabel: t("chat.message.copy") }));

async function onClick(event: MouseEvent) {
  const target = event.target as HTMLElement;
  const copy = target.closest<HTMLButtonElement>("[data-copy]");
  if (copy) {
    const code = copy.closest(".md-code")?.querySelector("code")?.textContent ?? "";
    await navigator.clipboard.writeText(code);
    copy.textContent = t("chat.message.copied");
    setTimeout(() => (copy.textContent = t("chat.message.copy")), 1500);
    return;
  }
  const link = target.closest("a");
  if (!link) return;
  event.preventDefault();
  const url = safeExternalUrl(link.getAttribute("href"));
  if (!url) return;
  if (inTauri) await openUrl(url);
  else window.open(url, "_blank", "noopener");
}
</script>

<template>
  <!-- eslint-disable-next-line vue/no-v-html -- renderMarkdown が生HTMLを文字にし、リンクと画像を制限している -->
  <div class="md" @click="onClick" v-html="html" />
</template>
```
スタイルは`:deep()`で`h1`〜`h3` (13px太字)・`p` (12px, 行間1.6, 余白0 0 6px)・`ul/ol` (左余白16px)・`table` (`--ribbon-line`の枠、見出し行`--hover-bg`、セル5px 9px、11px)・`code` (等幅、`--hover-bg`)・`.md-code` (地`--hover-bg`、角丸8px、見出し行に下線、言語名と`.md-copy`は10px `--ui-muted`)・`blockquote` (左線)・`a` (`--acad-blue`)。

`ThinkingRow.vue` (props: `text: string`, `durationMs: number | null`, `open: boolean`, `live: boolean`; emits `toggle`): 行の文言は`live`なら`chat.message.thinking`、そうでなければ`thinkingSeconds(durationMs)`があれば`chat.message.thought`、無ければ`chat.message.thoughtShort`。開いていれば線の中に思考の文 (11px `--ui-muted`、`pre-wrap`)。

`ToolGroup.vue` (props: `calls: ChatToolCall[]`, `failed`, `running`, `open`; emits `toggle`): 行の文言は`running`なら`chat.message.usingTools`、そうでなければ`chat.message.usedTools` (`n`=件数、enは複数形の`|`)。`failed`なら行を`--err-fg`。開いていれば`ToolStep`を並べる。手順ごとの開閉はこの部品の中の`ref<Record<string, boolean>>`で持つ (既定は閉じる)。

`ToolStep.vue` (props: `call: ChatToolCall`, `open`; emits `toggle`): 行は状態アイコン (今の`ToolChip.vue`と同じ`CircleCheck`/`CircleX`/回転する`LoaderCircle`)・`shortToolName(call.tool)` (等幅10px)・`call.summary` (10px `--ui-muted`、省略記号)・余白・矢印。開いていれば「入力」欄 (`prettyInput(call.input)`) と「結果」欄 (`call.output`。`null`なら`chat.message.noResult`、`omittedKb(call)`があれば下に`chat.message.omitted`)。各欄のコピーアイコンは`navigator.clipboard.writeText`。

`ChatMessage.vue`のアシスタント部分を置き換える:
```vue
<script setup lang="ts">
// (既存のimportに追加)
import { computed, ref } from "vue";
import { displayItems, isOpen } from "../../chat/blocks";
import ChatMarkdown from "./ChatMarkdown.vue";
import ThinkingRow from "./ThinkingRow.vue";
import ToolGroup from "./ToolGroup.vue";
// ToolChip の import は削除

const items = computed(() => displayItems(props.message));
/** 利用者が開閉した行 (パネルを開いている間だけ覚える)。 */
const overrides = ref<Record<string, boolean>>({});
const opened = (key: string) => isOpen(key, items.value, props.message.streaming, overrides.value);
function toggle(key: string) {
  overrides.value = { ...overrides.value, [key]: !opened(key) };
}
/** 進行行は、最後の行が開いた思考・ツールで進行を示しているときは出さない。 */
const showProgress = computed(() => {
  if (!props.message.streaming) return false;
  const last = items.value[items.value.length - 1];
  return !last || last.kind === "text";
});
</script>
```
```vue
  <div v-else class="assistant">
    <template v-for="(item, index) in items" :key="item.key">
      <ChatMarkdown v-if="item.kind === 'text'" :text="item.text" />
      <ThinkingRow
        v-else-if="item.kind === 'thinking'"
        :text="item.text"
        :duration-ms="item.duration_ms"
        :open="opened(item.key)"
        :live="message.streaming && index === items.length - 1"
        @toggle="toggle(item.key)"
      />
      <ToolGroup
        v-else
        :calls="item.calls"
        :failed="item.failed"
        :running="item.running"
        :open="opened(item.key)"
        @toggle="toggle(item.key)"
      />
    </template>
    <!-- 進行行・エラー・適用済み行は今のまま -->
```
`.text`のスタイルは使わなくなるので消す。`ToolChip.vue`を削除する。

- [ ] **Step 3: 確かめる**

Run: `npx vue-tsc --noEmit && npx vitest run`
Expected: 型エラーなし、すべて通る

- [ ] **Step 4: コミット**

```bash
git add src/components/chat src/locales
git commit -m "feat(chat): 作業過程を折りたたみで表示し、返答をMarkdownで描く"
```

---

### Task 5: 文書・仕様書・アプリでの確認・PR

**Files:**
- Modify: `docs/09-ai-assistant.md`、`docs/09-ai-assistant.ja.md`、`docs/internal/feature-inventory.md`、`docs/13-specification(.ja).md` (生成)

- [ ] **Step 1: 文書**

`docs/09-ai-assistant.ja.md`のチャットの説明に1項目足す (英語版にも同じ内容):

```
- **作業過程**: 返答は、思考・本文・ツールの呼び出しを起きた順に並べる。思考は「N秒考えました」、続けて使ったツールは「ツールをN件使いました」の1行にたたまれ、押すと開く。手順を開くと、エージェントが送った入力と、返ってきた結果 (8 KBまで。超えた分は省略と表示) を見られ、コピーできる。応答中は今の行が開き、終わると閉じる。履歴から開いた会話でも見られる。返答の見出し・箇条書き・表・コードはMarkdownとして描き、コードブロックにはコピーボタンが付く。リンクはOSのブラウザで開き、画像は読み込まずリンクとして表示する
```
```
- **Showing the agent's work**: a reply lists its thinking, text and tool calls in the order they happened. Thinking collapses to "Thought for N s" and consecutive tool calls to "Used N tools"; click to open. Opening a step shows the input the agent sent and the result that came back (up to 8 KB, with a note when more was left out), each with a copy button. While the agent works, the current row is open; it closes when the turn ends. Conversations reopened from history show the same. Headings, lists, tables and code in replies render as Markdown, code blocks get a copy button, links open in your browser, and images are shown as links rather than loaded
```

`docs/internal/feature-inventory.md`の「AIチャット(A1)」行のPR Aの文の後に: `。画面(段階1 PR B): 思考・ツールのまとまりを折りたたみ表示 (応答中は開き、終わると閉じる)、手順の入力・結果とコピー、返答のMarkdown (markdown-it 15.0.2、生HTML無効・http(s)リンクだけOSのブラウザで・画像はリンク)`

- [ ] **Step 2: 仕様書と全体の確認**

Run: `python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check && npx vitest run && npx vue-tsc --noEmit`
Expected: すべて通る

- [ ] **Step 3: コミット**

```bash
git add docs
git commit -m "docs: 作業過程の表示とMarkdownをAIアシスタントの説明と機能一覧に書く"
```

- [ ] **Step 4: アプリで確かめる** (コントローラが行う)

開発モードで起動し、Claude Code CLIで「図面の状態を確認してから、ヒューズを1つ配置して。最後に表で報告して」のように頼む。次を見る:
- 応答中、今のまとまりが開き、終わると閉じる
- まとまり・手順を開くと入力と結果が出る、コピーできる
- 表と見出しがMarkdownで描かれる
- 会話を履歴から開き直しても、作業過程が同じように出る
- 古い会話 (この変更より前のもの) も今までどおり読める

画面の写真を撮り、PRに付ける。

- [ ] **Step 5: PR** (コントローラが行う)

`master`向け。本文に目的・変えたこと・テスト・画面の写真・アプリでの確認結果を書き、末尾に`🤖 Generated with [Claude Code](https://claude.com/claude-code)`。
