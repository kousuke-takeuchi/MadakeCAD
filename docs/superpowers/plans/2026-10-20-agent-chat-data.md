# エージェントチャット段階1 PR A (会話データとイベント) 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 応答を「起きた順のブロック」で会話に残し、ツールの結果 (先頭8 KB) と思考を5つのバックエンドから受け取れるようにする。画面はまだ使わない (PR B)。

**Architecture:** `AgentEvent::ToolUseFinished`に`output`、新しく`AgentEvent::ThinkingDelta`を足す。各バックエンドはすでに持っている結果・思考をイベントに載せるだけ。`Conversation::apply_event`がイベントから`ChatMessage.blocks`を組み立て、結果を8 KBで切って`ToolCall.output`/`output_size`に残す。フィールドは足すだけで`CHAT_FORMAT_VERSION`は2のまま。

**Tech Stack:** Rust (madake-agent crate)、serde / serde_json、tokio (テストの偽サーバー)

**Spec:** `docs/superpowers/specs/2026-10-20-agent-chat-display-design.md` (日本語版 `.ja.md`)

## Global Constraints

- ツールの結果は1件 **8 KB (8192バイト)** までを`ToolCall.output`に残し、UTF-8の文字の切れ目で切る。`output_size`は結果全体のバイト数。
- 足すフィールドはすべて`#[serde(default, skip_serializing_if = ...)]`。`CHAT_FORMAT_VERSION`は**2のまま**。
- 思考を出すのは Claude Code CLI (`stream_event`の`thinking_delta`)・Anthropic API (`thinking_delta`)・OpenAI互換 (`delta.reasoning_content`か`delta.reasoning`) だけ。Gemini・Copilot CLIは出さない。
- `text`と`tool_calls`は今までどおり埋め続ける (古い読み手・段階2/3のため)。
- テストには必ず`///`の2行 (1行目英語、2行目日本語) の仕様文を付け、最後に`python3 scripts/gen_spec.py`で`docs/13-specification(.ja).md`を作り直す (CIが`--check`で確かめる)。
- コミットにAIの共著者行 (`Co-Authored-By`) を付けない。
- PRは`master`向け。PR Bと積み重ねない。

## Review Focus

- **結果が日本語などの多バイト文字で8 KB境界をまたぐ**: 文字の途中で切れて壊れた文字にならず、境界より前の最後の完全な文字で終わる。→ Task 1 のテスト`a_long_tool_result_is_cut_on_a_character_boundary`
- **旧形式の会話ファイル (`blocks`も`output`も無い)**: 読み込めて、そのまま次のターンを続けられる。保存し直しても`blocks`が無いメッセージに空配列を書き出さない。→ Task 1 のテスト`an_old_chat_file_without_blocks_still_loads`
- **思考→ツール→思考→本文のように種類が入れ替わる**: 同じ種類が続く間だけ1ブロックにまとまり、種類が変わるたびに新しいブロックになる。→ Task 1 のテスト`replies_are_recorded_as_blocks_in_the_order_they_happened`
- **idの無いツール完了 (Copilotの一部の行)**: 今と同じく、名前が同じで未完了の呼び出しに結果が付く。→ Task 1 のテスト`a_finish_without_an_id_puts_the_result_on_the_open_call_of_that_tool`
- **空の思考差分 (Anthropic・OpenAI互換が空文字を送る)**: 空のブロックを作らない。→ Task 3・4 の各テストで空差分を混ぜる

---

### Task 1: イベントと会話データ

**Files:**
- Modify: `src-tauri/crates/madake-agent/src/events.rs:38-60` (`AgentEvent`)
- Modify: `src-tauri/crates/madake-agent/src/conversation.rs` (`ToolCall`・`ChatMessage`・`MessageBlock`・`apply_event`)
- Modify (コンパイルを通すだけ): `src/events.rs:162`、`src/anthropic.rs:243`、`src/openai_compat.rs:314`、`src/gemini.rs:281`、`src/copilot_events.rs:216`、`tests/conversation.rs:46,62`、`tests/parser.rs:188`、`tests/anthropic_api.rs:381`、`tests/openai_compat.rs:384`、`tests/gemini.rs:500`
- Test: `src-tauri/crates/madake-agent/tests/conversation.rs`

**Interfaces:**
- Produces:
  - `AgentEvent::ToolUseFinished { id: String, tool: String, is_error: bool, #[serde(default)] output: Option<String> }`
  - `AgentEvent::ThinkingDelta { text: String }` (serdeのタグは`"thinking_delta"`)
  - `pub enum MessageBlock { Text { text: String }, Thinking { text: String, duration_ms: Option<u64> }, Tool { id: String } }` (`#[serde(tag = "type", rename_all = "snake_case")]`)
  - `ChatMessage.blocks: Vec<MessageBlock>`
  - `ToolCall.output: Option<String>`、`ToolCall.output_size: Option<u64>`
  - `pub const MAX_TOOL_OUTPUT_BYTES: usize = 8 * 1024;`
  - `pub fn truncate_output(output: &str) -> String` (8 KB以内の先頭を文字の切れ目で返す)
  - `Conversation::apply_event_at(&mut self, event: &AgentEvent, state: DocState, now_ms: u64)` (`apply_event`はこれを`now_millis()`で呼ぶ)

- [ ] **Step 1: 失敗するテストを書く**

`tests/conversation.rs`の`use madake_agent::conversation::{...}`に`MessageBlock, MAX_TOOL_OUTPUT_BYTES`を足し、ファイルの末尾に追加する:

```rust
fn ev_text(t: &str) -> AgentEvent {
    AgentEvent::TextDelta { text: t.to_string() }
}
fn ev_think(t: &str) -> AgentEvent {
    AgentEvent::ThinkingDelta { text: t.to_string() }
}
fn ev_start(id: &str) -> AgentEvent {
    AgentEvent::ToolUseStarted { id: id.into(), tool: "mcp__madakecad__place_symbol".into(), input: json!({}) }
}
fn ev_end(id: &str, output: Option<&str>) -> AgentEvent {
    AgentEvent::ToolUseFinished {
        id: id.into(),
        tool: "mcp__madakecad__place_symbol".into(),
        is_error: false,
        output: output.map(str::to_string),
    }
}

/// A reply is recorded as blocks in the order it happened: consecutive pieces of the same kind join one block, and a change of kind starts a new block, while the plain text and tool list are still filled as before.
/// 応答は起きた順のブロックとして記録される。同じ種類が続く間は1つのブロックにまとまり、種類が変わると新しいブロックになる。本文とツールの一覧も今までどおり埋まる。
#[test]
fn replies_are_recorded_as_blocks_in_the_order_they_happened() {
    let mut conv = Conversation::new();
    let s = edits(0);
    conv.begin_turn("F2を足して", s);
    for (t, ev) in [
        (1_000, ev_think("24V系を")),
        (1_400, ev_think("確かめる")),
        (1_500, ev_text("確認します。")),
        (1_600, ev_start("t1")),
        (1_700, ev_end("t1", Some("{\"net\":\"N3\"}"))),
        (1_800, ev_start("t2")),
        (1_900, ev_end("t2", None)),
        (2_000, ev_think("配置できた")),
        (2_100, ev_text("F2を")),
        (2_200, ev_text("追加しました")),
    ] {
        conv.apply_event_at(&ev, s, t);
    }
    let m = conv.last_turn().unwrap();
    assert_eq!(
        m.blocks,
        vec![
            MessageBlock::Thinking { text: "24V系を確かめる".into(), duration_ms: Some(400) },
            MessageBlock::Text { text: "確認します。".into() },
            MessageBlock::Tool { id: "t1".into() },
            MessageBlock::Tool { id: "t2".into() },
            MessageBlock::Thinking { text: "配置できた".into(), duration_ms: Some(0) },
            MessageBlock::Text { text: "F2を追加しました".into() },
        ]
    );
    assert_eq!(m.text, "確認します。F2を追加しました");
    assert_eq!(m.tool_calls.len(), 2);
    assert_eq!(m.tool_calls[0].output.as_deref(), Some("{\"net\":\"N3\"}"));
    assert_eq!(m.tool_calls[0].output_size, Some(12));
    assert_eq!(m.tool_calls[1].output, None);
}

/// A tool result longer than 8 KB keeps only its head, cut before a character that would cross the limit, and the full size is kept so the screen can say how much was left out.
/// 8 KBより長いツールの結果は先頭だけを残し、上限をまたぐ文字の手前で切る。省いた量を画面が示せるよう、全体の大きさも残す。
#[test]
fn a_long_tool_result_is_cut_on_a_character_boundary() {
    // 「あ」は3バイト。8192 = 3 * 2730 + 2 なので、2731文字目が上限をまたぐ
    let long = "あ".repeat(3000);
    let mut conv = Conversation::new();
    conv.begin_turn("全体を見せて", edits(0));
    conv.apply_event(&ev_start("t1"), edits(0));
    conv.apply_event(&ev_end("t1", Some(&long)), edits(0));
    let call = &conv.last_turn().unwrap().tool_calls[0];
    let kept = call.output.as_deref().unwrap();
    assert_eq!(kept.len(), 2730 * 3);
    assert!(kept.len() <= MAX_TOOL_OUTPUT_BYTES);
    assert!(kept.chars().all(|c| c == 'あ'));
    assert_eq!(call.output_size, Some(9000));
}

/// A tool finish without an id (some Copilot lines) still puts its result on the open call of the same tool, as before.
/// idの無いツール完了 (Copilotの一部の行) でも、今までどおり同じツールの未完了の呼び出しに結果が付く。
#[test]
fn a_finish_without_an_id_puts_the_result_on_the_open_call_of_that_tool() {
    let mut conv = Conversation::new();
    conv.begin_turn("置いて", edits(0));
    conv.apply_event(&ev_start("t1"), edits(0));
    conv.apply_event(&ev_end("", Some("ok")), edits(1));
    let call = &conv.last_turn().unwrap().tool_calls[0];
    assert!(call.finished);
    assert_eq!(call.output.as_deref(), Some("ok"));
}

/// A chat file saved before blocks and tool results existed still loads, its messages have no blocks, and saving it again does not write empty blocks or results.
/// ブロックとツールの結果が無かった頃に保存した会話ファイルも読み込め、メッセージのブロックは空になる。保存し直しても空のブロックや結果は書き出さない。
#[test]
fn an_old_chat_file_without_blocks_still_loads() {
    let dir = temp_dir();
    let path = dir.join("old.chat.json");
    let mut conv = Conversation::new();
    run_turn(&mut conv, 0, 2);
    save_chat(&path, &[conv.clone()]).unwrap();
    // 今の形式から blocks / output を取り除いて、旧形式のファイルを作る
    let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    for m in v["conversations"][0]["messages"].as_array_mut().unwrap() {
        m.as_object_mut().unwrap().remove("blocks");
        for c in m["tool_calls"].as_array_mut().unwrap() {
            c.as_object_mut().unwrap().remove("output");
            c.as_object_mut().unwrap().remove("output_size");
        }
    }
    std::fs::write(&path, serde_json::to_string_pretty(&v).unwrap()).unwrap();

    let loaded = load_chat(&path).unwrap();
    let m = loaded[0].last_turn().unwrap();
    assert!(m.blocks.is_empty());
    assert_eq!(m.tool_calls.len(), 2);
    assert_eq!(m.tool_calls[0].output, None);

    save_chat(&path, &loaded).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("\"blocks\""), "空のblocksを書き出している");
    assert!(!text.contains("\"output\""), "空のoutputを書き出している");
    std::fs::remove_dir_all(&dir).ok();
}
```

`save_chat(path, ..)`/`load_chat(path)`は会話ファイル (`*.chat.json`) のパスを直接受け取る。ファイルは`{"format_version": 2, "conversations": [...]}`の形 (`conversation.rs`の`ChatFile`)。`use`には`MessageBlock`と`MAX_TOOL_OUTPUT_BYTES`だけを足す (`ToolCall`は使わない)。

- [ ] **Step 2: 失敗を確かめる**

Run: `cd src-tauri && cargo test -p madake-agent --test conversation 2>&1 | tail -20`
Expected: コンパイルエラー (`ThinkingDelta`・`MessageBlock`・`apply_event_at`・`output`が無い)

- [ ] **Step 3: `AgentEvent`を変える**

`src/events.rs`の`AgentEvent`:

```rust
    /// アシスタント本文のストリーミング差分
    TextDelta { text: String },
    /// 思考 (thinking / reasoning) のストリーミング差分。送ってこないプロバイダもある
    ThinkingDelta { text: String },
    ...
    /// ツール呼び出し完了。`tool`は[`StreamParser`]経由の場合のみ補完される。
    /// `output`はツールが返した結果の全文 (会話へ残すときに[`crate::conversation::truncate_output`]で切る)
    ToolUseFinished {
        id: String,
        tool: String,
        is_error: bool,
        #[serde(default)]
        output: Option<String>,
    },
```

ファイル先頭のドキュメントコメントの`thinking_delta ... は無視`は Task 2 で直すので、ここではそのまま。

- [ ] **Step 4: コンパイルを通す (挙動は変えない)**

結果を作る側は、いったんすべて`output: None`を足す: `src/events.rs:162`、`src/anthropic.rs:243`、`src/openai_compat.rs:314`、`src/gemini.rs:281`、`src/copilot_events.rs:216`、`tests/conversation.rs:46,62`、`tests/parser.rs:188`。

全フィールドを並べてパターン照合しているテストは`..`を足す (例: `AgentEvent::ToolUseFinished { id, tool, is_error, .. }`): `tests/anthropic_api.rs:381`、`tests/openai_compat.rs:384`、`tests/gemini.rs:500`。

Run: `cd src-tauri && cargo build -p madake-agent --tests 2>&1 | grep -E "^error" -A6 | head -40`
ここで残るエラーは`conversation.rs`の`apply_event`だけのはず。

- [ ] **Step 5: 会話データを実装する**

`src/conversation.rs`:

```rust
/// ツールの結果を会話に残す上限 (バイト)。超えた分は[`truncate_output`]で省く。
pub const MAX_TOOL_OUTPUT_BYTES: usize = 8 * 1024;

/// 1回のツール呼び出し(UIのツールチップ表示に使う)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// CLIのtool_use_id
    pub id: String,
    pub tool: String,
    pub input: Value,
    pub finished: bool,
    pub is_error: bool,
    /// ツールが返した結果の先頭 ([`MAX_TOOL_OUTPUT_BYTES`]まで)。結果を返さないプロバイダ・旧履歴では`None`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// 結果全体の大きさ (バイト)。`output`より大きければ切り詰めてある
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_size: Option<u64>,
}

/// 応答を起きた順に並べたときの1区切り (画面はこの順に描く)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageBlock {
    Text { text: String },
    /// 思考。`duration_ms`は最初の差分から最後の差分までの時間 (アプリが測る)
    Thinking {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        duration_ms: Option<u64>,
    },
    /// ツール呼び出し ([`ChatMessage::tool_calls`]の`id`を指す)
    Tool { id: String },
}

/// 結果の先頭を[`MAX_TOOL_OUTPUT_BYTES`]以内で、UTF-8の文字の切れ目に合わせて返す。
pub fn truncate_output(output: &str) -> String {
    if output.len() <= MAX_TOOL_OUTPUT_BYTES {
        return output.to_string();
    }
    let mut end = MAX_TOOL_OUTPUT_BYTES;
    while !output.is_char_boundary(end) {
        end -= 1;
    }
    output[..end].to_string()
}
```

`ChatMessage`に2つのフィールドを足す (`attachments`の下):

```rust
    /// 起きた順の応答 (本文・思考・ツール)。この変更より前の履歴には無い (画面は旧表示にする)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<MessageBlock>,
    /// いま伸ばしている思考ブロックの開始時刻 (時間を測るためだけ。保存しない)
    #[serde(skip)]
    thinking_started_ms: Option<u64>,
```

`ChatMessage`は`PartialEq`を派生しているので`thinking_started_ms`も比較に入るが、ターンの終わり (`TurnCompleted`・`Error`) で必ず`None`へ戻すので、保存して読み込んだ会話との比較 (`chat_file_roundtrip_is_pretty_json`) は変わらない。`ChatMessage::new`に`blocks: Vec::new(), thinking_started_ms: None`を足す。

`apply_event`を次の形にする:

```rust
    /// ストリーム中のイベントを現在のターンへ反映する。
    ///
    /// `state`はイベント受信時点のドキュメント状態(ターン終了時の記録に使う)。
    pub fn apply_event(&mut self, event: &AgentEvent, state: DocState) {
        self.apply_event_at(event, state, now_millis());
    }

    /// [`Self::apply_event`]の時刻指定版 (思考の時間を測る時刻。テストで時刻を決めるため)。
    pub fn apply_event_at(&mut self, event: &AgentEvent, state: DocState, now_ms: u64) {
        self.touch();
        if let AgentEvent::SessionStarted { session_id } = event {
            self.session_id = Some(session_id.clone());
            return;
        }
        let Some(message) = self.current_turn_mut() else {
            return;
        };
        match event {
            AgentEvent::SessionStarted { .. } => {}
            AgentEvent::TextDelta { text } => {
                message.text.push_str(text);
                message.push_text(text);
            }
            AgentEvent::ThinkingDelta { text } => message.push_thinking(text, now_ms),
            AgentEvent::ToolUseStarted { id, tool, input } => {
                message.tool_calls.push(ToolCall {
                    id: id.clone(),
                    tool: tool.clone(),
                    input: input.clone(),
                    finished: false,
                    is_error: false,
                    output: None,
                    output_size: None,
                });
                message.end_thinking();
                message.blocks.push(MessageBlock::Tool { id: id.clone() });
            }
            AgentEvent::ToolUseFinished { id, tool, is_error, output } => {
                if let Some(call) = message
                    .tool_calls
                    .iter_mut()
                    .find(|c| c.id == *id || (id.is_empty() && c.tool == *tool && !c.finished))
                {
                    call.finished = true;
                    call.is_error = *is_error;
                    if let Some(output) = output {
                        call.output = Some(truncate_output(output));
                        call.output_size = Some(output.len() as u64);
                    }
                }
            }
            AgentEvent::TurnCompleted { result, .. } => {
                // デルタが来ない構成(--include-partial-messages無し)でも本文を埋める
                if message.text.is_empty() {
                    message.text = result.clone();
                    if !result.is_empty() {
                        message.push_text(result);
                    }
                }
                message.end_thinking();
                message.finish_turn(state);
            }
            // マネージャがターン確定後に合成するだけの通知。会話状態は既に更新済み
            AgentEvent::TurnApplied { .. } => {}
            AgentEvent::Error { message: err } => {
                message.error = Some(err.clone());
                message.end_thinking();
                message.finish_turn(state);
            }
        }
    }
```

`impl ChatMessage`に足す:

```rust
    /// 本文の差分を最後の本文ブロックへ足す (最後が本文でなければ新しく作る)。
    fn push_text(&mut self, text: &str) {
        self.end_thinking();
        if let Some(MessageBlock::Text { text: last }) = self.blocks.last_mut() {
            last.push_str(text);
        } else {
            self.blocks.push(MessageBlock::Text { text: text.to_string() });
        }
    }

    /// 思考の差分を最後の思考ブロックへ足し、時間を測り直す (空の差分は無視)。
    fn push_thinking(&mut self, text: &str, now_ms: u64) {
        if text.is_empty() {
            return;
        }
        let started = *self.thinking_started_ms.get_or_insert(now_ms);
        let duration = Some(now_ms.saturating_sub(started));
        if let Some(MessageBlock::Thinking { text: last, duration_ms }) = self.blocks.last_mut() {
            last.push_str(text);
            *duration_ms = duration;
        } else {
            self.blocks.push(MessageBlock::Thinking { text: text.to_string(), duration_ms: duration });
        }
    }

    /// 思考ブロックを閉じる (次の思考は新しいブロックになり、時間も測り直す)。
    fn end_thinking(&mut self) {
        self.thinking_started_ms = None;
    }
```

`push_thinking`は、最後のブロックが思考でない状態から始まるとき`thinking_started_ms`が`None`である必要がある。`push_text`と`ToolUseStarted`が`end_thinking`を呼ぶので満たされる。

- [ ] **Step 6: テストが通ることを確かめる**

Run: `cd src-tauri && cargo test -p madake-agent 2>&1 | grep -E "test result|FAILED|panicked"`
Expected: すべて`ok`。`tests/conversation.rs`の新しい4件も通る。

- [ ] **Step 7: コミット**

```bash
git add src-tauri/crates/madake-agent
git commit -m "feat(agent): 応答を起きた順のブロックで残し、ツールの結果 (先頭8KB) を会話に保存する"
```

---

### Task 2: Claude Code CLI (stream-json) の結果と思考

**Files:**
- Modify: `src-tauri/crates/madake-agent/src/events.rs` (先頭のドキュメントコメント、`parse_stream_event`、`parse_user`)
- Test: `src-tauri/crates/madake-agent/tests/parser.rs`

**Interfaces:**
- Consumes: Task 1 の`AgentEvent::ThinkingDelta`、`ToolUseFinished.output`

- [ ] **Step 1: 失敗するテストを書く**

`tests/parser.rs`の`only_text_deltas_become_text_events`を、次の2つのテストに置き換える (実物のfixture `stream_tooluse.jsonl`には`thinking_delta`が5行、`tool_result`の`content`が`"madake-test"`で入っている):

```rust
/// Text deltas become text events and thinking deltas become thinking events, each joined in order; signature deltas are ignored.
/// テキスト差分はテキストイベント、thinking差分は思考イベントになり、それぞれ順につながる。signature差分は無視される。
#[test]
fn text_and_thinking_deltas_become_their_own_events() {
    assert_eq!(joined_text(&events(TOOLUSE)), "done.");
    assert_eq!(joined_text(&events(TEXT)), "hello");
    let thinking: String = events(TOOLUSE)
        .iter()
        .filter_map(|e| match e {
            AgentEvent::ThinkingDelta { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert!(thinking.starts_with("The user is asking me to run a specific bash command"), "{thinking}");
    assert!(thinking.contains("The command executed successfully"), "{thinking}");
}

/// A tool result carries what the tool returned, whether the CLI sends it as a string or as an array of text blocks.
/// ツールの完了には、ツールが返した内容が載る。CLIが文字列で送っても、テキストブロックの配列で送っても同じ。
#[test]
fn a_tool_result_carries_what_the_tool_returned() {
    let output = events(TOOLUSE).into_iter().find_map(|e| match e {
        AgentEvent::ToolUseFinished { output, .. } => output,
        _ => None,
    });
    assert_eq!(output.as_deref(), Some("madake-test"));
    let line = r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":[{"type":"text","text":"{\"id\":"},{"type":"text","text":"\"e1\"}"}],"is_error":false}]}}"#;
    match parse_stream_line(line) {
        Some(AgentEvent::ToolUseFinished { output, .. }) => assert_eq!(output.as_deref(), Some("{\"id\":\"e1\"}")),
        other => panic!("tool_endのはず: {other:?}"),
    }
}
```

`parse_stream_line`が1行から最初のイベントだけを返す関数であることを`src/events.rs`で確かめる (違えば`parse_stream_events(line).into_iter().next()`を使う。`lib.rs`の`pub use`も確かめる)。

- [ ] **Step 2: 失敗を確かめる**

Run: `cd src-tauri && cargo test -p madake-agent --test parser 2>&1 | grep -E "test result|FAILED|panicked"`
Expected: 新しい2件が失敗 (思考が空、`output`が`None`)

- [ ] **Step 3: 実装する**

`parse_stream_event`の`text_delta`だけを見ている部分を次に替える:

```rust
    match delta.get("type").and_then(Value::as_str) {
        Some("text_delta") => match delta.get("text").and_then(Value::as_str) {
            Some(text) if !text.is_empty() => vec![AgentEvent::TextDelta { text: text.to_string() }],
            _ => Vec::new(),
        },
        Some("thinking_delta") => match delta.get("thinking").and_then(Value::as_str) {
            Some(text) if !text.is_empty() => vec![AgentEvent::ThinkingDelta { text: text.to_string() }],
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
```

`parse_user`の`ToolUseFinished`に`output: tool_result_text(b)`を入れ、関数を足す:

```rust
/// `tool_result.content`の文字 (文字列か、テキストブロックの配列をつないだもの)。
fn tool_result_text(block: &Value) -> Option<String> {
    match block.get("content")? {
        Value::String(text) => Some(text.clone()),
        Value::Array(parts) => Some(
            parts
                .iter()
                .filter_map(|p| p.get("text").and_then(Value::as_str))
                .collect(),
        ),
        _ => None,
    }
}
```

ファイル先頭のコメントを直す: `` `content_block_delta`の`text_delta`と`thinking_delta`を採用 (`signature_delta`/`input_json_delta`は無視) ``、`` `user`: `tool_result`(is_error・結果の文字付き) ``。

- [ ] **Step 4: 通ることを確かめる**

Run: `cd src-tauri && cargo test -p madake-agent 2>&1 | grep -E "test result|FAILED|panicked"`
Expected: すべて`ok`

- [ ] **Step 5: コミット**

```bash
git add src-tauri/crates/madake-agent
git commit -m "feat(agent): Claude Code CLI の思考とツールの結果をイベントに載せる"
```

---

### Task 3: Anthropic API の結果と思考

**Files:**
- Modify: `src-tauri/crates/madake-agent/src/anthropic.rs:243` (結果)、`:463` (思考)
- Test: `src-tauri/crates/madake-agent/tests/anthropic_api.rs`

**Interfaces:**
- Consumes: Task 1 の`AgentEvent::ThinkingDelta`、`ToolUseFinished.output`

- [ ] **Step 1: 失敗するテストを書く**

`tests/anthropic_api.rs`の台本の部品 (`text_block`の下) に足す:

```rust
fn thinking_block(index: u64, chunks: &[&str]) -> Vec<(&'static str, Value)> {
    let mut out = vec![(
        "content_block_start",
        json!({"type": "content_block_start", "index": index, "content_block": {"type": "thinking", "thinking": ""}}),
    )];
    for chunk in chunks {
        out.push((
            "content_block_delta",
            json!({"type": "content_block_delta", "index": index, "delta": {"type": "thinking_delta", "thinking": chunk}}),
        ));
    }
    out.push((
        "content_block_delta",
        json!({"type": "content_block_delta", "index": index, "delta": {"type": "signature_delta", "signature": "sig"}}),
    ));
    out.push(("content_block_stop", json!({"type": "content_block_stop", "index": index})));
    out
}
```

テストを足す:

```rust
/// Thinking the API streams arrives as thinking events in order (empty pieces and signatures are skipped), separate from the reply text.
/// APIが送ってくる思考は、返答の本文とは別の思考イベントとして順に届く (空の差分と署名は飛ばす)。
#[tokio::test]
async fn streamed_thinking_becomes_thinking_events() {
    let api = MockApi::start(vec![turn(
        vec![thinking_block(0, &["24V系を", "", "確かめる"]), text_block(1, &["はい"])],
        "end_turn",
    )])
    .await;
    let backend = AnthropicApiBackend::new("sk-test", "claude-sonnet-5").with_base_url(api.base_url());

    let events = run(&backend, "考えて", None).await;

    let thinking: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::ThinkingDelta { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(thinking, ["24V系を", "確かめる"]);
    assert_eq!(texts(&events), "はい");
}
```

既存の`a_requested_tool_is_executed_and_its_result_is_sent_back`の「ツール完了イベントが無い」の`assert!`を、結果も確かめる形に替える (`FakeTools::new`に渡した結果の文字列がそのまま`output`に載る。テスト冒頭の`FakeTools::new(...)`の引数を読み、その文字列を期待値にする):

```rust
    assert!(
        events.iter().any(|e| matches!(e, AgentEvent::ToolUseFinished { id, tool, is_error, output }
            if id == "toolu_1" && tool == "place_symbol" && !*is_error
                && output.as_deref() == Some(EXPECTED_RESULT))),
        "ツール完了イベント (結果つき) が無い: {events:?}"
    );
```

`EXPECTED_RESULT`は、そのテストの`FakeTools::new(...)`に渡している文字列リテラルに置き換える。仕様文の2行の末尾に英語` The finish event carries the tool's result.`、日本語`完了イベントにはツールの結果が載る。`を足す。

- [ ] **Step 2: 失敗を確かめる**

Run: `cd src-tauri && cargo test -p madake-agent --test anthropic_api 2>&1 | grep -E "test result|FAILED|panicked"`
Expected: 2件失敗

- [ ] **Step 3: 実装する**

`src/anthropic.rs`のツール完了 (`:243`付近) で`output: Some(outcome.content.clone())`にする (直後に`outcome.content`を`results`へ入れているので`clone`する)。

`content_block_delta`の`match`の`// thinking_delta / signature_delta などは表示しない`の手前に足す:

```rust
                (Some("thinking_delta"), _) => {
                    let chunk = delta
                        .and_then(|d| d.get("thinking"))
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if !chunk.is_empty() {
                        send(tx, AgentEvent::ThinkingDelta { text: chunk.to_string() }).await;
                    }
                }
```

コメントは`// signature_delta などは表示しない`に直す。思考ブロックは`content_block_start`で`Building::Other`のままでよい (次の往復へ積み直すのは今の`round.content_blocks`の扱いに任せ、ここでは変えない)。

- [ ] **Step 4: 通ることを確かめる**

Run: `cd src-tauri && cargo test -p madake-agent 2>&1 | grep -E "test result|FAILED|panicked"`
Expected: すべて`ok`

- [ ] **Step 5: コミット**

```bash
git add src-tauri/crates/madake-agent
git commit -m "feat(agent): Anthropic API の思考とツールの結果をイベントに載せる"
```

---

### Task 4: OpenAI互換 (Ollamaなど) の結果と思考

**Files:**
- Modify: `src-tauri/crates/madake-agent/src/openai_compat.rs:314` (結果)、`:620-635` (思考)
- Test: `src-tauri/crates/madake-agent/tests/openai_compat.rs`

**Interfaces:**
- Consumes: Task 1 の`AgentEvent::ThinkingDelta`、`ToolUseFinished.output`

- [ ] **Step 1: 失敗するテストを書く**

台本の部品 (`text_chunk`の下) に足す:

```rust
/// 思考の差分チャンク (`field`は`reasoning_content`か`reasoning`)。
fn reasoning_chunk(field: &str, text: &str) -> Value {
    json!({"choices": [{"index": 0, "delta": {field: text}, "finish_reason": null}]})
}
```

テストを足す:

```rust
/// Reasoning that a model streams as reasoning_content or reasoning (Ollama, DeepSeek and others) arrives as thinking events, separate from the reply text; empty pieces are skipped.
/// モデルがreasoning_contentかreasoningで送ってくる思考 (Ollama・DeepSeekなど) は、返答の本文とは別の思考イベントとして届く。空の差分は飛ばす。
#[tokio::test]
async fn streamed_reasoning_becomes_thinking_events() {
    let api = MockApi::start(vec![Scripted::Sse(vec![
        reasoning_chunk("reasoning_content", "24V系を"),
        reasoning_chunk("reasoning", ""),
        reasoning_chunk("reasoning", "確かめる"),
        text_chunk("はい"),
        finish("stop"),
    ])])
    .await;
    let backend = backend_with_key(&api);

    let events = run(&backend, "考えて", None).await;

    let thinking: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::ThinkingDelta { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(thinking, ["24V系を", "確かめる"]);
    assert_eq!(texts(&events), "はい");
}
```

既存の`a_requested_tool_is_executed_and_its_result_is_sent_back` (`FakeTools::new("{\"id\":\"e1\"}")`) の完了イベントの`assert!`を替える:

```rust
    assert!(
        events.iter().any(
            |e| matches!(e, AgentEvent::ToolUseFinished { id, tool, is_error, output }
            if id == "call_1" && tool == "place_symbol" && !*is_error
                && output.as_deref() == Some("{\"id\":\"e1\"}"))
        ),
        "ツール完了イベント (結果つき) が無い: {events:?}"
    );
```

仕様文の2行の末尾に英語` The finish event carries the tool's result.`、日本語`完了イベントにはツールの結果が載る。`を足す。

- [ ] **Step 2: 失敗を確かめる**

Run: `cd src-tauri && cargo test -p madake-agent --test openai_compat 2>&1 | grep -E "test result|FAILED|panicked"`
Expected: 2件失敗

- [ ] **Step 3: 実装する**

ツール完了 (`:314`付近) で`output: Some(outcome.content.clone())`にする (直後で`outcome.content`を使うため)。

`let Some(delta) = event.pointer("/choices/0/delta")`の直後、本文 (`content`) の処理の前に足す:

```rust
    // 思考 (Ollama・DeepSeekなど)。サーバーによって名前が違う
    for key in ["reasoning_content", "reasoning"] {
        if let Some(text) = delta.get(key).and_then(Value::as_str) {
            if !text.is_empty() {
                send(tx, AgentEvent::ThinkingDelta { text: text.to_string() }).await;
            }
        }
    }
```

思考は`round.text`に入れない (次の往復へ積み直す発言に混ぜない)。

- [ ] **Step 4: 通ることを確かめる**

Run: `cd src-tauri && cargo test -p madake-agent 2>&1 | grep -E "test result|FAILED|panicked"`
Expected: すべて`ok`

- [ ] **Step 5: コミット**

```bash
git add src-tauri/crates/madake-agent
git commit -m "feat(agent): OpenAI互換の思考 (reasoning) とツールの結果をイベントに載せる"
```

---

### Task 5: Gemini と Copilot CLI の結果

**Files:**
- Modify: `src-tauri/crates/madake-agent/src/gemini.rs:281`、`src-tauri/crates/madake-agent/src/copilot_events.rs:216`
- Test: `src-tauri/crates/madake-agent/tests/gemini.rs`、`src-tauri/crates/madake-agent/tests/copilot_events.rs`

**Interfaces:**
- Consumes: Task 1 の`ToolUseFinished.output`

- [ ] **Step 1: 失敗するテストを書く**

`tests/gemini.rs`の`a_requested_tool_is_executed_and_its_result_is_sent_back`の完了イベントの`assert!`を、`output.as_deref() == Some(<そのテストのFakeTools::newに渡した文字列>)`まで確かめる形にする (Task 4 と同じ形。パターンは`{ id, tool, is_error, output }`)。仕様文の末尾に Task 3 と同じ英日の一文を足す。

さらに、Gemini が思考を出さないことを確かめるテストを足す:

```rust
/// Gemini is not asked for its thoughts, so a reply never produces thinking events.
/// Geminiには思考を求めないので、返答から思考イベントは出ない。
#[tokio::test]
async fn gemini_replies_produce_no_thinking_events() {
    let api = MockApi::start(vec![Scripted::Sse(vec![text_chunk("はい")])]).await;
    let backend = backend_with_key(&api);
    let events = run(&backend, "考えて", None).await;
    assert!(!events.iter().any(|e| matches!(e, AgentEvent::ThinkingDelta { .. })), "{events:?}");
    assert_eq!(texts(&events), "はい");
}
```

`tests/gemini.rs`に`backend_with_key`・`texts`・`run`が無い場合は、既存の`a_requested_tool_is_executed_and_its_result_is_sent_back`と同じ作り方 (ファイル内の補助関数) に合わせる。

`tests/copilot_events.rs`の`a_tool_execution_complete_closes_the_call_with_its_outcome`を、結果まで確かめる形に替える:

```rust
/// Finishing a tool closes the matching call, reports whether it succeeded and carries the result text the CLI printed.
/// ツールの実行完了は対応する呼び出しを閉じ、成功したかどうかと、CLIが出した結果の文字を伝える。
#[test]
fn a_tool_execution_complete_closes_the_call_with_its_outcome() {
    for (line, expected_error, expected_output) in [
        (
            r#"{"type":"tool.execution_complete","data":{"toolCallId":"toolu_01","success":true,"result":{"content":"{}"}},"id":"e1"}"#,
            false,
            "{}",
        ),
        (
            r#"{"type":"tool.execution_complete","data":{"toolCallId":"toolu_01","success":false,"result":{"content":"不明なシンボルです"}},"id":"e1"}"#,
            true,
            "不明なシンボルです",
        ),
    ] {
        match one(line) {
            AgentEvent::ToolUseFinished { id, is_error, output, .. } => {
                assert_eq!(id, "toolu_01", "{line}");
                assert_eq!(is_error, expected_error, "{line}");
                assert_eq!(output.as_deref(), Some(expected_output), "{line}");
            }
            other => panic!("tool_endのはず ({line}): {other:?}"),
        }
    }
    // 結果の文字が無い完了は結果なし
    match one(r#"{"type":"tool_result","tool_call_id":"call_1"}"#) {
        AgentEvent::ToolUseFinished { output, .. } => assert_eq!(output, None),
        other => panic!("tool_endのはず: {other:?}"),
    }
}
```

- [ ] **Step 2: 失敗を確かめる**

Run: `cd src-tauri && cargo test -p madake-agent --test gemini --test copilot_events 2>&1 | grep -E "test result|FAILED|panicked"`
Expected: 結果を確かめる2件が失敗 (思考なしのテストは最初から通る)

- [ ] **Step 3: 実装する**

`src/gemini.rs:281`: `output: Some(outcome.content.clone())`。

`src/copilot_events.rs`の`tool_end_event`: `output: text_of(v)`。`text_of`は`data.result.content`の文字を取り出す (`field`は`data`の下から探し、`result`がオブジェクトなら`nested_text`が`content`を返す)。`tool_call_id`だけの行は`None`になる。

- [ ] **Step 4: 通ることを確かめる**

Run: `cd src-tauri && cargo test -p madake-agent 2>&1 | grep -E "test result|FAILED|panicked"`
Expected: すべて`ok`

- [ ] **Step 5: コミット**

```bash
git add src-tauri/crates/madake-agent
git commit -m "feat(agent): Gemini と Copilot CLI のツールの結果をイベントに載せる"
```

---

### Task 6: 仕様書の再生成・全体の確認・PR

**Files:**
- Modify: `docs/13-specification.md`、`docs/13-specification.ja.md` (生成物)
- Modify: `docs/internal/feature-inventory.md` (AIチャット(A1)の行)

- [ ] **Step 1: 仕様書を作り直す**

Run: `python3 scripts/gen_spec.py && python3 scripts/gen_spec.py --check`
Expected: `spec docs up to date; all tests have bilingual spec comments`

- [ ] **Step 2: 機能一覧に1文足す**

`docs/internal/feature-inventory.md`の「AIチャット(A1)」行の末尾 ` |` の手前に足す:

```
。**作業過程のデータ**(2026-10-20、段階1 PR A): 応答を起きた順の`blocks`(本文・思考・ツール)で残し、ツールの結果を先頭8KBまで`ToolCall.output`に保存。思考はClaude CLI・Anthropic API・OpenAI互換のreasoningから。画面での表示は段階1 PR B
```

- [ ] **Step 3: 全体の確認**

Run:
```bash
cd src-tauri && cargo test -p madake-agent 2>&1 | grep -E "test result|FAILED" ; cargo clippy -q -p madake-agent --all-targets 2>&1 | grep -E "^(warning|error)" -A3 | grep -- "-->" | grep -v "manager.rs"
cd .. && npx vitest run 2>&1 | grep -E "Test Files|Tests " && npx vue-tsc --noEmit
```
Expected: Rustはすべて`ok`、clippyの出力なし (`manager.rs`の既存の警告は除く)、Vitestと型チェックも通る (画面は変えていないので結果は変わらない)

- [ ] **Step 4: アプリで確かめる**

`npm run tauri dev`で起動し、Claude Code CLIで図面を少し編集させる (例: 「24V系にヒューズを1つ置いて」)。終わったらプロジェクトを保存し、隣の`<名前>.chat.json`を開いて次を確かめる:
- 最後のアシスタントメッセージに`"blocks"`があり、`thinking`・`text`・`tool`が起きた順に並ぶ
- `tool_calls[*].output`にツールの結果のJSONの文字が入り、`output_size`がある
- 画面の見た目は今までと同じ

- [ ] **Step 5: コミットとPR**

```bash
git add docs
git commit -m "docs: 作業過程のデータを仕様書と機能一覧に反映"
git push origin feature/agent-chat-data:feature/agent-chat-data
gh pr create --base master --head feature/agent-chat-data --title "feat(agent): record the agent's work in order with tool results and thinking" --body-file <本文ファイル>
```

PR本文には、目的 (段階1のデータ側。画面はPR B)、変えたこと (イベント・会話データ・5つのバックエンド)、`CHAT_FORMAT_VERSION`を上げない理由、テスト、アプリでの確認結果を書き、末尾に`🤖 Generated with [Claude Code](https://claude.com/claude-code)`を付ける。
