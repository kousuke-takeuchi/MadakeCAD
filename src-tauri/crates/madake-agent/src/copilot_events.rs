//! GitHub Copilot CLIのJSONL出力([`--output-format json`])をアプリ内イベントへ変換する。
//!
//! # 実機で採取した行の形(copilot 1.0.80、2026-08-22)
//!
//! 1行1JSONで、最終行以外は次の封筒に入っている:
//!
//! ```json
//! {"type":"assistant.message_delta","data":{…},"id":"…","timestamp":"…",
//!  "parentId":"…","ephemeral":true}
//! ```
//!
//! 使う行:
//!
//! - `assistant.message_delta` → `data.deltaContent` が本文の差分(**`ephemeral`だが本文そのもの**なので捨てない)
//! - `assistant.message` → `data.content`(完成した本文)と`data.toolRequests[]`
//!   (`toolCallId`/`name`/`arguments`)、`data.outputTokens`
//! - `tool.execution_start` → `data.toolCallId`/`data.toolName`/`data.arguments`
//! - `tool.execution_complete` → `data.toolCallId`/`data.success`
//! - `result`(最終行。**ここだけ封筒が無く直下にフィールドを持つ**) →
//!   `sessionId`(こちらが`--session-id`で決め打ちした値と一致)・`exitCode`・
//!   `usage.premiumRequests`(Copilotの課金単位)
//!
//! 捨てる行(ターンの表示に不要): `session.mcp_servers_loaded` /
//! `session.mcp_server_status_changed` / `session.skills_loaded` /
//! `session.tools_updated` / `session.auto_mode_resolved` / `session.usage_checkpoint` /
//! `model.call_start` / `assistant.message_start` / `assistant.turn_start` /
//! `assistant.turn_end` / `assistant.idle` / `user.message` / `system.message` など。
//! **`assistant.reasoning_delta`も`data.deltaContent`を持つが本文ではない**ので捨てる。
//!
//! # 寛容さ
//!
//! 行の形は公開仕様が無くバージョン差も見込まれるため、型名は小文字化して`.`/`-`を`_`へ
//! 寄せたうえで**既知の別名**とも突き合わせ、フィールド名も別名を許す。解釈できない行
//! (未知の型・JSONでない行・空行)は**黙って捨てる**ので、未知の行でターンは壊れない。

use crate::events::{AgentEvent, Usage};
use serde_json::Value;
use std::collections::HashMap;

/// セッション開始とみなす型名(実機は`session.start`。ただし`-p`のstdoutには出ない)。
const SESSION_TYPES: &[&str] = &["session_start", "session", "session_started", "init"];
/// アシスタント本文の**差分**とみなす型名(実機は`assistant.message_delta`)。
const TEXT_TYPES: &[&str] = &[
    "assistant_message_delta",
    "text_delta",
    "assistant",
    "text",
    "content_block_delta",
    "delta",
];
/// 完成したアシスタントメッセージとみなす型名(実機は`assistant.message`)。
const ASSISTANT_MESSAGE_TYPES: &[&str] = &["assistant_message", "assistant_message_complete"];
/// ツール呼び出しの開始とみなす型名(実機は`tool.execution_start`)。
const TOOL_START_TYPES: &[&str] = &[
    "tool_execution_start",
    "tool_call",
    "tool_use",
    "tool_start",
    "tool_started",
    "tool_call_start",
    "function_call",
];
/// ツール呼び出しの完了とみなす型名(実機は`tool.execution_complete`)。
const TOOL_END_TYPES: &[&str] = &[
    "tool_execution_complete",
    "tool_result",
    "tool_call_result",
    "tool_use_result",
    "tool_end",
    "tool_finished",
    "tool_completed",
    "function_result",
];
/// ターン完了とみなす型名(実機は最終行の`result`)。
const RESULT_TYPES: &[&str] = &[
    "result",
    "done",
    "completion",
    "complete",
    "completed",
    "turn_completed",
    "final",
    "final_response",
];
/// エラーとみなす型名。
const ERROR_TYPES: &[&str] = &["error", "session_error", "fatal", "failure"];

/// JSONLの1行を0個以上のイベントへ変換する(解釈できない行は空)。
///
/// 本文は`assistant.message_delta`だけから作る。`assistant.message`の完成本文は
/// 差分と重複するため、ここでは**道具の依頼だけ**を拾う(差分を取りこぼした場合の
/// 補完は[`CopilotParser`]が受け持つ)。
pub fn parse_copilot_events(line: &str) -> Vec<AgentEvent> {
    match parse_line(line) {
        Some(value) => events_from_value(&value),
        None => Vec::new(),
    }
}

/// 1行をJSONとして読む(空行・JSONでない行・`type`の無い行はNone)。
fn parse_line(line: &str) -> Option<Value> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let value = serde_json::from_str::<Value>(line).ok()?;
    value.get("type").and_then(Value::as_str)?;
    Some(value)
}

/// 型名の表記ゆれ(大文字・`.`・`-`)をならして返す。
fn kind_of(v: &Value) -> String {
    normalize_type(v.get("type").and_then(Value::as_str).unwrap_or_default())
}

fn events_from_value(v: &Value) -> Vec<AgentEvent> {
    let kind = kind_of(v);
    let kind = kind.as_str();

    if SESSION_TYPES.contains(&kind) {
        return session_event(v);
    }
    if ASSISTANT_MESSAGE_TYPES.contains(&kind) {
        return assistant_message_event(v);
    }
    if TOOL_START_TYPES.contains(&kind) {
        return tool_start_event(v);
    }
    if TOOL_END_TYPES.contains(&kind) {
        return tool_end_event(v);
    }
    if RESULT_TYPES.contains(&kind) {
        return result_event(v);
    }
    if ERROR_TYPES.contains(&kind) {
        return error_event(v);
    }
    if TEXT_TYPES.contains(&kind) {
        return text_event(v);
    }
    Vec::new()
}

/// 型名の表記ゆれ(大文字・`.`・`-`)をならす。
fn normalize_type(kind: &str) -> String {
    kind.trim()
        .to_ascii_lowercase()
        .replace(['.', '-', ' '], "_")
}

/// イベントのフィールドを取り出す。
///
/// 実機のイベントは`{"type":…,"data":{…}}`の入れ子なので**`data`を先に**見る。
/// 最終行の`result`だけは封筒が無く直下にフィールドを持つため、無ければ直下を見る。
fn field<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    v.get("data")
        .and_then(|data| data.get(key))
        .or_else(|| v.get(key))
}

fn session_event(v: &Value) -> Vec<AgentEvent> {
    let id = str_of(v, &["sessionId", "session_id", "id"])
        .or_else(|| v.get("session").and_then(|s| str_of(s, &["id"])));
    match id {
        Some(session_id) if !session_id.is_empty() => {
            vec![AgentEvent::SessionStarted { session_id }]
        }
        _ => Vec::new(),
    }
}

fn text_event(v: &Value) -> Vec<AgentEvent> {
    match text_of(v) {
        Some(text) if !text.is_empty() => vec![AgentEvent::TextDelta { text }],
        _ => Vec::new(),
    }
}

/// 完成したアシスタントメッセージから、モデルが頼んだツール呼び出しを取り出す。
///
/// 本文(`content`)はここでは使わない。差分で配信済みだからで、取りこぼしの補完は
/// [`CopilotParser`]が行う。ツール呼び出しは直後の`tool.execution_start`と重複するが、
/// [`CopilotParser`]が同じ`toolCallId`の2回目を捨てる。
fn assistant_message_event(v: &Value) -> Vec<AgentEvent> {
    let Some(requests) = field(v, "toolRequests")
        .or_else(|| field(v, "tool_requests"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    requests
        .iter()
        .filter_map(|request| {
            let tool = tool_name(request)?;
            Some(AgentEvent::ToolUseStarted {
                id: call_id(request),
                tool,
                input: tool_input(request),
            })
        })
        .collect()
}

fn tool_start_event(v: &Value) -> Vec<AgentEvent> {
    let Some(tool) = tool_name(v) else {
        return Vec::new();
    };
    vec![AgentEvent::ToolUseStarted {
        id: call_id(v),
        tool,
        input: tool_input(v),
    }]
}

fn tool_end_event(v: &Value) -> Vec<AgentEvent> {
    vec![AgentEvent::ToolUseFinished {
        id: call_id(v),
        // ツール名は完了の行に無い。[`CopilotParser`]が呼び出しから補完する
        tool: tool_name(v).unwrap_or_default(),
        is_error: is_error(v),
        output: text_of(v),
    }]
}

/// 最終行`result`(セッションIDの確定・成否・使用量)。
///
/// 回答本文はこの行に載らないため、[`CopilotParser`]が直前の`assistant.message`から補う。
fn result_event(v: &Value) -> Vec<AgentEvent> {
    let mut out = Vec::new();
    // CLIが実際に使ったセッションID。`--resume`はこれを使う
    if let Some(session_id) = str_of(v, &["sessionId", "session_id"]).filter(|s| !s.is_empty()) {
        out.push(AgentEvent::SessionStarted { session_id });
    }
    let text = text_of(v).unwrap_or_default();
    if is_error(v) {
        let message = if text.is_empty() {
            match i64_of(v, &["exitCode", "exit_code"]) {
                Some(code) => format!("GitHub Copilot CLIがエラーを返しました (exit {code})"),
                None => "GitHub Copilot CLIがエラーを返しました".to_string(),
            }
        } else {
            text
        };
        out.push(AgentEvent::Error { message });
        return out;
    }
    out.push(AgentEvent::TurnCompleted {
        result: text,
        usage: usage_of(v),
    });
    out
}

fn error_event(v: &Value) -> Vec<AgentEvent> {
    let message = str_of(v, &["message", "detail", "reason", "text"])
        .or_else(|| match field(v, "error") {
            Some(Value::String(text)) => Some(text.clone()),
            Some(error) => str_of(error, &["message", "detail", "reason"]),
            None => None,
        })
        .unwrap_or_else(|| "GitHub Copilot CLIがエラーを返しました".to_string());
    vec![AgentEvent::Error { message }]
}

/// ツール名(実機は`toolName`。MCPツールは`<サーバー名>-<ツール名>`の形)。
fn tool_name(v: &Value) -> Option<String> {
    str_of(v, &["toolName", "name", "tool", "tool_name"]).filter(|name| !name.is_empty())
}

/// ツール呼び出しの識別子(呼び出しと結果を結びつける鍵)。
fn call_id(v: &Value) -> String {
    str_of(
        v,
        &["toolCallId", "tool_call_id", "call_id", "callId", "id"],
    )
    .unwrap_or_default()
}

/// ツールへ渡された引数(文字列で来た場合はJSONとして読み直す)。
fn tool_input(v: &Value) -> Value {
    for key in ["arguments", "input", "args", "parameters", "params"] {
        match field(v, key) {
            Some(Value::String(raw)) => {
                return serde_json::from_str(raw).unwrap_or(Value::String(raw.clone()))
            }
            Some(value) => return value.clone(),
            None => {}
        }
    }
    Value::Null
}

/// 本文テキストの取り出し(フィールド名の別名と、ブロック配列の両方に対応)。
fn text_of(v: &Value) -> Option<String> {
    for key in [
        "deltaContent",
        "delta_content",
        "text",
        "result",
        "response",
        "content",
        "delta",
        "output",
    ] {
        match field(v, key) {
            Some(Value::String(text)) => return Some(text.clone()),
            Some(Value::Array(blocks)) => {
                let joined = blocks.iter().filter_map(block_text).collect::<String>();
                if !joined.is_empty() {
                    return Some(joined);
                }
            }
            Some(nested @ Value::Object(_)) => {
                if let Some(text) = nested_text(nested) {
                    return Some(text);
                }
            }
            _ => {}
        }
    }
    None
}

/// `{"type":"text","text":"..."}`形式のブロックから本文を取り出す。
fn block_text(block: &Value) -> Option<String> {
    match block {
        Value::String(text) => Some(text.clone()),
        _ => str_of(block, &["text", "content"]),
    }
}

/// `{"content": ...}` / `{"text": ...}`を持つ入れ子から本文を取り出す。
fn nested_text(v: &Value) -> Option<String> {
    match v.get("content") {
        Some(Value::String(text)) => return Some(text.clone()),
        Some(Value::Array(blocks)) => {
            let joined = blocks.iter().filter_map(block_text).collect::<String>();
            if !joined.is_empty() {
                return Some(joined);
            }
        }
        _ => {}
    }
    str_of(v, &["text", "delta"])
}

/// 失敗を示す印(`success`・`exitCode`・`is_error`・`status`・`error`のいずれか)。
fn is_error(v: &Value) -> bool {
    if let Some(flag) = bool_of(v, &["is_error", "isError"]) {
        return flag;
    }
    // 実機の`tool.execution_complete`は成否を`success`で持つ
    if let Some(ok) = bool_of(v, &["success", "ok"]) {
        return !ok;
    }
    // 実機の最終行`result`は終了コードで成否を表す
    if let Some(code) = i64_of(v, &["exitCode", "exit_code"]) {
        return code != 0;
    }
    if let Some(status) = str_of(v, &["status", "state", "outcome"]) {
        let status = status.to_ascii_lowercase();
        if ["error", "failed", "failure", "denied", "cancelled"].contains(&status.as_str()) {
            return true;
        }
    }
    !matches!(field(v, "error"), None | Some(Value::Null))
}

/// 使用量。Copilotの最終行はトークン数を持たず、代わりにプレミアムリクエストを持つ。
fn usage_of(v: &Value) -> Option<Usage> {
    let usage = field(v, "usage")?;
    Some(Usage {
        input_tokens: u64_of(usage, &["input_tokens", "prompt_tokens", "inputTokens"]),
        output_tokens: u64_of(
            usage,
            &["output_tokens", "completion_tokens", "outputTokens"],
        ),
        cache_creation_input_tokens: u64_of(usage, &["cache_creation_input_tokens"]),
        cache_read_input_tokens: u64_of(usage, &["cache_read_input_tokens", "cached_tokens"]),
        total_cost_usd: f64_of(usage, &["total_cost_usd"]).or_else(|| f64_of(v, &["cost"])),
        premium_requests: f64_of(usage, &["premiumRequests", "premium_requests"])
            .or_else(|| f64_of(v, &["totalPremiumRequests"])),
    })
}

fn str_of(v: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| field(v, key).and_then(Value::as_str))
        .map(str::to_string)
}

fn u64_of(v: &Value, keys: &[&str]) -> u64 {
    keys.iter()
        .find_map(|key| field(v, key).and_then(Value::as_u64))
        .unwrap_or(0)
}

fn i64_of(v: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter()
        .find_map(|key| field(v, key).and_then(Value::as_i64))
}

fn f64_of(v: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| field(v, key).and_then(Value::as_f64))
}

fn bool_of(v: &Value, keys: &[&str]) -> Option<bool> {
    keys.iter()
        .find_map(|key| field(v, key).and_then(Value::as_bool))
}

/// JSONLを1行ずつ食べ、ターンをまたぐ辻褄合わせをするパーサ。
///
/// Copilotの行だけでは足りない分をここで補う:
///
/// - ツール完了の行にはツール名が無いので、呼び出し行の名前を補う
/// - 同じ`toolCallId`が二度流れた場合(`assistant.message`と`tool.execution_start`)は
///   2回目以降を捨てる
/// - 最終行`result`には回答本文が無いので、最後の`assistant.message`の本文を入れる
/// - 差分が届かなかった本文(`assistant.message`にだけ載った分)を追いかけて配信する
/// - 最終行にトークン数が無いので、`assistant.message`の`outputTokens`を積み上げる
/// - こちらが`--session-id`で決めたIDが最終行で確認されただけのときは二重に通知しない
#[derive(Debug, Default)]
pub struct CopilotParser {
    tool_names: HashMap<String, String>,
    /// 通知済みのセッションID(呼び出し側が決め打ちした分を含む)
    session_id: Option<String>,
    /// 差分として配信済みの本文(メッセージIDごと)
    streamed: HashMap<String, String>,
    /// 直近の完成した本文(最終行`result`の回答として使う)
    final_text: String,
    /// 積み上げた出力トークン数
    output_tokens: u64,
}

impl CopilotParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// 呼び出し側が`--session-id`/`--resume`で決めたセッションIDを既知として始める。
    ///
    /// 最終行`result`はそのIDをそのまま返してくるので、同じなら通知しない
    /// (違うIDが返ってきたときだけ「そちらが正」として通知する)。
    pub fn for_session(session_id: impl Into<String>) -> Self {
        Self {
            session_id: Some(session_id.into()),
            ..Self::default()
        }
    }

    /// 1行を処理し、0個以上のイベントを返す。
    pub fn push(&mut self, line: &str) -> Vec<AgentEvent> {
        let Some(value) = parse_line(line) else {
            return Vec::new();
        };
        let kind = kind_of(&value);
        let mut out = Vec::new();

        if kind == "assistant_message_delta" {
            self.note_delta(&value);
        } else if ASSISTANT_MESSAGE_TYPES.contains(&kind.as_str()) {
            if let Some(text) = self.missing_text(&value) {
                out.push(AgentEvent::TextDelta { text });
            }
            self.output_tokens += u64_of(&value, &["outputTokens", "output_tokens"]);
        }

        for mut event in events_from_value(&value) {
            match &mut event {
                AgentEvent::SessionStarted { session_id } => {
                    if self.session_id.as_deref() == Some(session_id.as_str()) {
                        continue;
                    }
                    self.session_id = Some(session_id.clone());
                }
                AgentEvent::ToolUseStarted { id, tool, .. } => {
                    if !id.is_empty() && self.tool_names.contains_key(id.as_str()) {
                        continue;
                    }
                    self.tool_names.insert(id.clone(), tool.clone());
                }
                AgentEvent::ToolUseFinished { id, tool, .. } if tool.is_empty() => {
                    if let Some(name) = self.tool_names.get(id.as_str()) {
                        *tool = name.clone();
                    }
                }
                AgentEvent::TurnCompleted { result, usage } => {
                    if result.is_empty() {
                        *result = self.final_text.clone();
                    }
                    if let Some(usage) = usage {
                        if usage.output_tokens == 0 {
                            usage.output_tokens = self.output_tokens;
                        }
                    }
                }
                _ => {}
            }
            out.push(event);
        }
        out
    }

    /// 差分を「どのメッセージの分か」と一緒に覚える。
    fn note_delta(&mut self, v: &Value) {
        let (Some(id), Some(text)) = (message_id(v), text_of(v)) else {
            return;
        };
        self.streamed.entry(id).or_default().push_str(&text);
    }

    /// 完成した本文のうち、差分でまだ配信していない部分を返す。
    fn missing_text(&mut self, v: &Value) -> Option<String> {
        let content = str_of(v, &["content"]).filter(|text| !text.is_empty())?;
        let sent = self
            .streamed
            .remove(&message_id(v).unwrap_or_default())
            .unwrap_or_default();
        self.final_text = content.clone();
        // 差分と食い違う(前方一致しない)ときは、取りこぼしを避けて全文を出す
        let rest = content.strip_prefix(sent.as_str()).unwrap_or(&content);
        (!rest.is_empty()).then(|| rest.to_string())
    }
}

fn message_id(v: &Value) -> Option<String> {
    str_of(v, &["messageId", "message_id"])
}
