//! GitHub Copilot CLIのJSONL出力([`--output-format json`])をアプリ内イベントへ変換する。
//!
//! Copilot CLI 1.0.80の`--output-format json`は「1行1JSONオブジェクト」だが、
//! **行の形は公開仕様が無い**(`copilot --help`にも記述が無く、バージョン差も見込まれる)。
//! そのためここは`type`フィールドを手がかりにした**寛容な**変換にしてある:
//!
//! - 型名は小文字化し`.`/`-`を`_`へ寄せてから、既知の別名の集合と突き合わせる
//! - フィールド名も別名を許す(`text`/`content`/`delta`、`arguments`/`input`/`args` 等)
//! - 解釈できない行(未知の型・JSONでない行・空行)は**黙って捨てる**
//!
//! 実形が確認できしだい、ここの別名表を実際に出る名前へ寄せていくこと
//! (捨てる方向に倒してあるので、未知の行でターンが壊れることはない)。

use crate::events::{AgentEvent, Usage};
use serde_json::Value;
use std::collections::HashMap;

/// セッション開始とみなす型名。
const SESSION_TYPES: &[&str] = &["session", "session_started", "session_start", "init"];
/// アシスタントの本文とみなす型名。
const TEXT_TYPES: &[&str] = &[
    "assistant",
    "assistant_message",
    "agent_message",
    "message",
    "text",
    "text_delta",
    "content",
    "content_block_delta",
    "delta",
];
/// ツール呼び出しの開始とみなす型名。
const TOOL_START_TYPES: &[&str] = &[
    "tool_call",
    "tool_use",
    "tool_start",
    "tool_started",
    "tool_call_start",
    "function_call",
];
/// ツール呼び出しの完了とみなす型名。
const TOOL_END_TYPES: &[&str] = &[
    "tool_result",
    "tool_call_result",
    "tool_use_result",
    "tool_end",
    "tool_finished",
    "tool_completed",
    "function_result",
];
/// ターン完了とみなす型名。
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
const ERROR_TYPES: &[&str] = &["error", "fatal", "failure"];

/// JSONLの1行を0個以上のイベントへ変換する(解釈できない行は空)。
pub fn parse_copilot_events(line: &str) -> Vec<AgentEvent> {
    let line = line.trim();
    if line.is_empty() {
        return Vec::new();
    }
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return Vec::new();
    };
    let Some(kind) = value.get("type").and_then(Value::as_str) else {
        return Vec::new();
    };
    let kind = normalize_type(kind);
    let kind = kind.as_str();

    if SESSION_TYPES.contains(&kind) {
        return session_event(&value);
    }
    if TOOL_START_TYPES.contains(&kind) {
        return tool_start_event(&value);
    }
    if TOOL_END_TYPES.contains(&kind) {
        return tool_end_event(&value);
    }
    if RESULT_TYPES.contains(&kind) {
        return result_event(&value);
    }
    if ERROR_TYPES.contains(&kind) {
        return error_event(&value);
    }
    if TEXT_TYPES.contains(&kind) {
        return text_event(&value);
    }
    Vec::new()
}

/// 型名の表記ゆれ(大文字・`.`・`-`)をならす。
fn normalize_type(kind: &str) -> String {
    kind.trim()
        .to_ascii_lowercase()
        .replace(['.', '-', ' '], "_")
}

fn session_event(v: &Value) -> Vec<AgentEvent> {
    let id = str_of(v, &["session_id", "sessionId", "id"])
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

fn tool_start_event(v: &Value) -> Vec<AgentEvent> {
    let tool = str_of(v, &["name", "tool", "tool_name", "toolName"])
        .or_else(|| match v.get("function") {
            Some(Value::String(name)) => Some(name.clone()),
            Some(function) => str_of(function, &["name"]),
            None => None,
        })
        .unwrap_or_default();
    if tool.is_empty() {
        return Vec::new();
    }
    vec![AgentEvent::ToolUseStarted {
        id: call_id(v),
        tool,
        input: tool_input(v),
    }]
}

fn tool_end_event(v: &Value) -> Vec<AgentEvent> {
    vec![AgentEvent::ToolUseFinished {
        id: call_id(v),
        // ツール名は結果行に無いことが多い。[`CopilotParser`]が呼び出しから補完する
        tool: str_of(v, &["name", "tool", "tool_name", "toolName"]).unwrap_or_default(),
        is_error: is_error(v),
    }]
}

fn result_event(v: &Value) -> Vec<AgentEvent> {
    let text = text_of(v).unwrap_or_default();
    if is_error(v) {
        let message = if text.is_empty() {
            "GitHub Copilot CLIがエラーを返しました".to_string()
        } else {
            text
        };
        return vec![AgentEvent::Error { message }];
    }
    vec![AgentEvent::TurnCompleted {
        result: text,
        usage: usage_of(v),
    }]
}

fn error_event(v: &Value) -> Vec<AgentEvent> {
    let message = str_of(v, &["message", "detail", "reason", "text"])
        .or_else(|| match v.get("error") {
            Some(Value::String(text)) => Some(text.clone()),
            Some(error) => str_of(error, &["message", "detail", "reason"]),
            None => None,
        })
        .unwrap_or_else(|| "GitHub Copilot CLIがエラーを返しました".to_string());
    vec![AgentEvent::Error { message }]
}

/// ツール呼び出しの識別子(呼び出しと結果を結びつける鍵)。
fn call_id(v: &Value) -> String {
    str_of(
        v,
        &["tool_call_id", "toolCallId", "call_id", "callId", "id"],
    )
    .unwrap_or_default()
}

/// ツールへ渡された引数(文字列で来た場合はJSONとして読み直す)。
fn tool_input(v: &Value) -> Value {
    for key in ["arguments", "input", "args", "parameters", "params"] {
        match v.get(key) {
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
        "text", "result", "response", "content", "delta", "message", "output",
    ] {
        match v.get(key) {
            Some(Value::String(text)) => return Some(text.clone()),
            Some(Value::Array(blocks)) => {
                let joined = blocks.iter().filter_map(block_text).collect::<String>();
                if !joined.is_empty() {
                    return Some(joined);
                }
            }
            Some(Value::Object(_)) => {
                if let Some(nested) = v.get(key).and_then(nested_text) {
                    return Some(nested);
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

/// 失敗を示す印(`is_error`・`status`・`error`のいずれか)。
fn is_error(v: &Value) -> bool {
    if let Some(flag) = v
        .get("is_error")
        .or_else(|| v.get("isError"))
        .and_then(Value::as_bool)
    {
        return flag;
    }
    if let Some(status) = str_of(v, &["status", "state", "outcome"]) {
        let status = status.to_ascii_lowercase();
        if ["error", "failed", "failure", "denied", "cancelled"].contains(&status.as_str()) {
            return true;
        }
    }
    !matches!(v.get("error"), None | Some(Value::Null))
}

/// トークン使用量(MadakeCAD流とOpenAI流のどちらの名前でも読む)。
fn usage_of(v: &Value) -> Option<Usage> {
    let usage = v.get("usage")?;
    Some(Usage {
        input_tokens: u64_of(usage, &["input_tokens", "prompt_tokens", "inputTokens"]),
        output_tokens: u64_of(
            usage,
            &["output_tokens", "completion_tokens", "outputTokens"],
        ),
        cache_creation_input_tokens: u64_of(usage, &["cache_creation_input_tokens"]),
        cache_read_input_tokens: u64_of(usage, &["cache_read_input_tokens", "cached_tokens"]),
        total_cost_usd: v
            .get("total_cost_usd")
            .or_else(|| v.get("cost"))
            .or_else(|| usage.get("total_cost_usd"))
            .and_then(Value::as_f64),
    })
}

fn str_of(v: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| v.get(*key).and_then(Value::as_str))
        .map(str::to_string)
}

fn u64_of(v: &Value, keys: &[&str]) -> u64 {
    keys.iter()
        .find_map(|key| v.get(*key).and_then(Value::as_u64))
        .unwrap_or(0)
}

/// ツール名を覚えながらJSONLを変換するステートフルパーサ。
///
/// - ツール結果の行にはツール名が無いことが多いので、呼び出し行の名前を補完する
/// - 同じ呼び出しIDが二度流れた場合(再送)は2回目以降を捨てる
#[derive(Debug, Default)]
pub struct CopilotParser {
    tool_names: HashMap<String, String>,
}

impl CopilotParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// 1行を処理し、0個以上のイベントを返す。
    pub fn push(&mut self, line: &str) -> Vec<AgentEvent> {
        let mut out = Vec::new();
        for mut event in parse_copilot_events(line) {
            match &mut event {
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
                _ => {}
            }
            out.push(event);
        }
        out
    }
}
