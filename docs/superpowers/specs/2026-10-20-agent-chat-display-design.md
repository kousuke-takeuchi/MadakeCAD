# Agent chat: showing the agent's work (stage 1)

Date: 2026-10-20 · Status: design approved in chat, awaiting spec review · Japanese: [2026-10-20-agent-chat-display-design.ja.md](2026-10-20-agent-chat-display-design.ja.md)

## Goal

The agent chat should look and behave like Claude's and Ollama's chat. This is stage 1 of three:

1. **Response display (this spec).** Show what the agent did, in the order it happened: thinking, text and tool calls. Collapse it by default and let the user open it. Render Markdown in replies.
2. Message actions: copy, edit and resend, retry (tied into the existing turn undo).
3. Conversation management: rename, delete, search, export, per-conversation system prompt.

Stages 2 and 3 get their own specs.

Success for stage 1: after a turn, the user can open any tool call and see what the agent sent and what came back, also for conversations reopened from history. Replies with headings, lists, tables or code read as formatted text.

## What exists today

- `ChatMessage` (`madake-agent/src/conversation.rs`) holds one `text` string and a list of `tool_calls`. Each call keeps its id, tool name, `input`, `finished` and `is_error`.
- `ChatMessage.vue` draws the whole text first and then one `ToolChip` per call. A chip shows the tool name and a one-line summary only.
- Every backend sees the tool result when it emits `AgentEvent::ToolUseFinished`, and then drops it:
  - Claude Code CLI: the `tool_result` block in `parse_user` (`events.rs`).
  - Anthropic API, Gemini and OpenAI-compatible: `ToolOutcome.content` in their tool loops.
  - Copilot CLI: the `tool.execution_complete` line.
- Thinking is dropped everywhere: `thinking_delta` in `anthropic.rs` and `events.rs`, and `thinking` blocks in Claude CLI assistant messages.
- Reply text is shown with `white-space: pre-wrap`; there is no Markdown.

## Design

### 1. Conversation data

`ChatMessage` gains `blocks: Vec<MessageBlock>`, the reply in the order it happened:

```rust
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageBlock {
    Text { text: String },
    Thinking { text: String, duration_ms: Option<u64> },
    Tool { id: String }, // refers to tool_calls[].id
}
```

- A `TextDelta` appends to the last block when it is `Text`; otherwise it starts a new `Text` block. `ThinkingDelta` works the same way for `Thinking`. `ToolUseStarted` adds a `Tool` block.
- `duration_ms` is the time from the first to the last delta of that thinking block. The app measures it, because no provider reports it.
- `text` and `tool_calls` stay as they are and are still filled, so stage 2/3 code and older readers keep working.
- `ToolCall` gains `output: Option<String>` and `output_size: Option<u64>`. `output` keeps the first 8 KB of the result, cut on a UTF-8 character boundary. `output_size` is the full size in bytes. When they differ, the screen shows "… N KB more not shown".
- All new fields use `#[serde(default, skip_serializing_if = ...)]`, so `CHAT_FORMAT_VERSION` stays 2:
  - An older build reading a new file ignores the unknown fields (nothing uses `deny_unknown_fields`). It drops them when it saves again; that is acceptable.
  - A message without `blocks` (any conversation saved before this change) is shown the old way: text first, then the tool steps.

### 2. Events

- `AgentEvent::ToolUseFinished` gains `output: Option<String>`. Each backend fills it with the result it already holds:

  | Backend | Source of the result |
  |---|---|
  | Claude Code CLI | `tool_result.content`: a string, or an array of text blocks joined together |
  | Anthropic API, Gemini, OpenAI-compatible | `ToolOutcome.content` |
  | Copilot CLI | the result text of `tool.execution_complete` if the line carries one (existing `text_of` rules), otherwise `None` |

- New `AgentEvent::ThinkingDelta { text: String }`, emitted where the provider already sends thinking without extra settings:

  | Backend | Source of the thinking |
  |---|---|
  | Claude Code CLI | `thinking_delta` in `stream_event`. The `thinking` blocks in the final assistant message are not re-emitted, the same way text is handled. |
  | Anthropic API | `thinking_delta` |
  | OpenAI-compatible (Ollama and others) | `delta.reasoning_content` or `delta.reasoning`, when the model sends them |
  | Gemini | out of scope: it needs `includeThoughts` turned on |
  | Copilot CLI | out of scope |

- Truncation to 8 KB happens in `Conversation` when the event is recorded. The event itself carries the whole result, because the live turn does not need to hold it beyond the message.

### 3. Screen components

`ChatMessage.vue` lays out `blocks` and does nothing else. New components in `src/components/chat/`:

| Component | Job |
|---|---|
| `ChatMarkdown.vue` | Renders Markdown text; code blocks get the language name and a copy button |
| `ThinkingRow.vue` | Collapsed row "Thought for N s ›". When open, the thinking text is shown inside a left rule. |
| `ToolGroup.vue` | One collapsed row per run of consecutive tool calls: "Used N tools ›". When open, it shows the steps inside a left rule. |
| `ToolStep.vue` | One step (status icon, tool name, summary, ›). When open, it shows the **Input** (pretty JSON) and **Result** panels, each with a copy button, and the truncation note. |

- `ToolChip.vue` is replaced by `ToolStep.vue`.
- Grouping consecutive `Tool` blocks into runs is a pure function `groupBlocks(blocks)` in `stores/chat.ts`, so it can be tested on its own. A `Thinking` or `Text` block ends a run.

Open and close behaviour (as in Claude):

- While the turn streams, the group or thinking row that is still receiving events is open. When the turn ends, both close.
- A row the user opened or closed keeps that state while the panel is open. It is not saved in the conversation.
- A group containing a failed step shows its collapsed row in the error colour, so a failure is visible without opening it.
- Running steps show the spinner in place of the check, as the chip does today.

The look follows the approved mockups in `MadakeCAD.pen`: `AIチャット 作業過程(折りたたみ)` (collapsed) and `AIチャット 作業過程(展開)` (expanded).

### 4. Markdown

- Library: `markdown-it` pinned to exactly `15.0.2`. 15.0.3 was published on 2026-10-16, four days before this spec, so the release before it is used. `@types/markdown-it` goes into dev dependencies.
- Rendered: headings, bullet and numbered lists, tables, bold and italic, inline code, fenced code blocks, block quotes, horizontal rules.
- Not in stage 1: syntax highlighting (the language name is shown), math, raw HTML.
- Safety: the app runs in Tauri with `"csp": null`, so model output must never become live HTML.
  - `html: false`: raw HTML in the reply is shown as text.
  - Links: only `http:` and `https:` are kept. Clicking one opens it in the OS browser through `@tauri-apps/plugin-opener` and never navigates the webview. Other schemes (`javascript:`, `file:`, `data:`) are rendered as plain text.
  - Images: `![alt](url)` is rendered as a link labelled with the alt text, so a reply cannot make the app load remote content.
- While streaming, the text is re-rendered on each delta. An unclosed code fence is shown as a code block up to the end of the text received so far (markdown-it's default behaviour).

### 5. Error handling

- A tool result that is not valid UTF-8 cannot happen. Every source is a JSON string, so `output` is always text.
- If a result is longer than 8 KB, the stored `output` ends at the last full character before 8 KB and `output_size` keeps the real size.
- A thinking or tool event for a message that no longer exists (for example, the conversation was cleared during the turn) is ignored, as text deltas are today.

## Testing

Tests are the specification in this repository: each test gets an English and a Japanese spec sentence, and `scripts/gen_spec.py` regenerates `docs/13-specification(.ja).md`.

- Rust:
  - Each backend puts the tool result into `ToolUseFinished.output`.
  - Claude CLI and Anthropic emit `ThinkingDelta`; OpenAI-compatible emits it for `reasoning_content` and `reasoning`; Gemini does not.
  - `Conversation` builds `blocks` in order, merges consecutive deltas, measures thinking duration, and truncates output at 8 KB on a character boundary while keeping `output_size`.
  - A conversation file without `blocks` and `output` still loads and keeps working.
- Vitest:
  - `groupBlocks` makes runs of consecutive tools.
  - Groups open while streaming and close at the end; a user's open or close choice is kept.
  - A failed step colours its group.
  - Markdown safety: raw HTML becomes text, `javascript:` links are not links, images become links, links open through the opener.
  - Copy buttons copy the input, the result and code blocks.
  - Old messages without `blocks` still render.
- In the app: run a Claude Code CLI turn that edits a drawing. Open and close the thinking row, a tool group and a step. Then reopen the conversation from history and check that the results are still there.

## Delivery

Two pull requests, both based on `master` (no stacking):

- **PR A: conversation data and events (Rust only).** Covers `MessageBlock`, `ToolCall.output`, `ToolUseFinished.output`, `ThinkingDelta`, the backend changes and the tests. The screen ignores the new fields, so merging A alone changes nothing visible.
- **PR B: screen.** Covers the new components, markdown-it, the locale strings for all six catalogs, and the two mockups already committed on this branch (`05360be`). Opened after A, with "merge #A first" in its description.

## Decision log

| Date | Decision | Why |
|---|---|---|
| 2026-10-20 | Split chat parity into three stages; this spec is stage 1 | Too large for one spec; stage 2's edit and retry depend on stage 1's data shape and on turn undo |
| 2026-10-20 | Store tool results in history, truncated to 8 KB per call | Results can be hundreds of KB (`get_project`, netlists). Keeping a head is enough to review the work and keeps history files small. Rejected: store everything (slow history loading); keep results only during the session (lost on restart). |
| 2026-10-20 | Thinking only from Claude Code CLI, Anthropic API and OpenAI-compatible `reasoning` fields | These send it without extra settings. Gemini needs `includeThoughts`. |
| 2026-10-20 | Additive fields, no `CHAT_FORMAT_VERSION` bump | Older builds ignore unknown fields; bumping would make them refuse new files |
| 2026-10-20 | `markdown-it` pinned to 15.0.2, raw HTML off, links only http(s) through the OS browser, images as links | The webview has no CSP, so model output must not run as HTML or load remote content. 15.0.3 was four days old. |
| 2026-10-20 | Visual design: Claude-style collapsed rows with left rules (`.pen` mockups) | Approved by the user in Pen.app |
| 2026-10-20 | Two PRs (data, then screen), both on `master` | One PR per change; the data PR is safe to merge alone |
