//! Claude Code CLIのstream-json出力をアプリ内イベントへ変換する。
//!
//! CLIの出力仕様はバージョン差が大きいため、[`serde_json::Value`]で緩くパースし
//! 必要なフィールドだけを取り出す。未知のイベント型は黙って捨てる。
//!
//! 実測した行の種類(claude 2.1.237, `-p --output-format stream-json --verbose
//! --include-partial-messages`):
//! - `system`/`init`: session_id
//! - `stream_event`: 生APIイベント。`content_block_delta`の`text_delta`のみ採用
//!   (`thinking_delta`/`signature_delta`/`input_json_delta`は無視)
//! - `assistant`: 完成したcontent block。`tool_use`は完全なinputを持つ
//! - `user`: `tool_result`(is_error付き)
//! - `result`: ターン終了(subtype=success、result/usage)
//! - `system`/`status`、`system`/`thinking_tokens`、`rate_limit_event` 等はスキップ

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// 1ターン分のトークン使用量(CLIの`result`イベントより)。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub total_cost_usd: Option<f64>,
}

/// UI/CLIへ配信するエージェントイベント。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    /// CLIセッション開始。`session_id`は`--resume`での再開に使う
    SessionStarted { session_id: String },
    /// アシスタント本文のストリーミング差分
    TextDelta { text: String },
    /// ツール呼び出し開始(inputは完全なJSON)
    ToolUseStarted {
        id: String,
        tool: String,
        input: Value,
    },
    /// ツール呼び出し完了。`tool`は[`StreamParser`]経由の場合のみ補完される
    ToolUseFinished {
        id: String,
        tool: String,
        is_error: bool,
    },
    /// ターン完了(最終テキストと使用量)
    TurnCompleted {
        result: String,
        usage: Option<Usage>,
    },
    /// ターンで確定した編集が図面に適用されたことの通知。
    ///
    /// CLIの出力には存在しない合成イベントで、[`crate::AgentManager`]だけが発行する
    /// (ターン開始/終了時のEngine revision。差が「元に戻す」に必要なundo回数)。
    TurnApplied {
        start_revision: u64,
        end_revision: u64,
    },
    /// CLI側のエラー(プロセス異常終了・result(is_error)など)
    Error { message: String },
}

/// stream-jsonの1行を1イベントへ変換する(未知の行はNone)。
///
/// 1行が複数イベントになり得る(tool_useブロックが複数)場合は先頭のみを返す。
/// 全て取りたい場合は [`parse_stream_events`] か [`StreamParser`] を使う。
pub fn parse_stream_line(line: &str) -> Option<AgentEvent> {
    parse_stream_events(line).into_iter().next()
}

/// stream-jsonの1行を0個以上のイベントへ変換する。
pub fn parse_stream_events(line: &str) -> Vec<AgentEvent> {
    let line = line.trim();
    if line.is_empty() {
        return Vec::new();
    }
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return Vec::new();
    };
    match v.get("type").and_then(Value::as_str) {
        Some("system") => parse_system(&v),
        Some("stream_event") => parse_stream_event(&v),
        Some("assistant") => parse_assistant(&v),
        Some("user") => parse_user(&v),
        Some("result") => parse_result(&v),
        _ => Vec::new(),
    }
}

fn parse_system(v: &Value) -> Vec<AgentEvent> {
    if v.get("subtype").and_then(Value::as_str) != Some("init") {
        return Vec::new();
    }
    match v.get("session_id").and_then(Value::as_str) {
        Some(id) => vec![AgentEvent::SessionStarted {
            session_id: id.to_string(),
        }],
        None => Vec::new(),
    }
}

fn parse_stream_event(v: &Value) -> Vec<AgentEvent> {
    let event = match v.get("event") {
        Some(e) => e,
        None => return Vec::new(),
    };
    if event.get("type").and_then(Value::as_str) != Some("content_block_delta") {
        return Vec::new();
    }
    let delta = match event.get("delta") {
        Some(d) => d,
        None => return Vec::new(),
    };
    if delta.get("type").and_then(Value::as_str) != Some("text_delta") {
        return Vec::new();
    }
    match delta.get("text").and_then(Value::as_str) {
        Some(text) if !text.is_empty() => vec![AgentEvent::TextDelta {
            text: text.to_string(),
        }],
        _ => Vec::new(),
    }
}

fn parse_assistant(v: &Value) -> Vec<AgentEvent> {
    // 本文テキストはstream_eventのtext_deltaで既に配信済みなので、
    // ここではtool_useブロック(inputが完全に揃っている)だけを拾う。
    content_blocks(v)
        .iter()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("tool_use"))
        .map(|b| AgentEvent::ToolUseStarted {
            id: str_field(b, "id"),
            tool: str_field(b, "name"),
            input: b.get("input").cloned().unwrap_or(Value::Null),
        })
        .collect()
}

fn parse_user(v: &Value) -> Vec<AgentEvent> {
    content_blocks(v)
        .iter()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("tool_result"))
        .map(|b| AgentEvent::ToolUseFinished {
            id: str_field(b, "tool_use_id"),
            // tool_resultにツール名は含まれない。StreamParserがidから補完する
            tool: String::new(),
            is_error: b.get("is_error").and_then(Value::as_bool).unwrap_or(false),
        })
        .collect()
}

fn parse_result(v: &Value) -> Vec<AgentEvent> {
    let result = v.get("result").and_then(Value::as_str).unwrap_or_default();
    let is_error = v.get("is_error").and_then(Value::as_bool).unwrap_or(false)
        || v.get("subtype").and_then(Value::as_str) != Some("success");
    if is_error {
        let message = if result.is_empty() {
            v.get("subtype")
                .and_then(Value::as_str)
                .unwrap_or("unknown error")
                .to_string()
        } else {
            result.to_string()
        };
        return vec![AgentEvent::Error { message }];
    }
    vec![AgentEvent::TurnCompleted {
        result: result.to_string(),
        usage: parse_usage(v),
    }]
}

fn parse_usage(v: &Value) -> Option<Usage> {
    let usage = v.get("usage")?;
    Some(Usage {
        input_tokens: u64_field(usage, "input_tokens"),
        output_tokens: u64_field(usage, "output_tokens"),
        cache_creation_input_tokens: u64_field(usage, "cache_creation_input_tokens"),
        cache_read_input_tokens: u64_field(usage, "cache_read_input_tokens"),
        total_cost_usd: v.get("total_cost_usd").and_then(Value::as_f64),
    })
}

/// `message.content`の配列を参照で返す(無ければ空)。
fn content_blocks(v: &Value) -> &[Value] {
    v.get("message")
        .and_then(|m| m.get("content"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn str_field(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn u64_field(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(Value::as_u64).unwrap_or(0)
}

/// tool_use_id → ツール名を覚えながら行を変換するステートフルパーサ。
///
/// `tool_result`行にはツール名が含まれないため、直前の`tool_use`の名前を
/// [`AgentEvent::ToolUseFinished`]へ補完する。
/// あわせて、同じtool_use_idの[`AgentEvent::ToolUseStarted`]が複数行にまたがって
/// 出た場合(CLIの再送等)は2回目以降を捨てる。
#[derive(Debug, Default)]
pub struct StreamParser {
    tool_names: HashMap<String, String>,
}

impl StreamParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// 1行を処理し、0個以上のイベントを返す。
    pub fn push(&mut self, line: &str) -> Vec<AgentEvent> {
        let mut out = Vec::new();
        for mut ev in parse_stream_events(line) {
            match &mut ev {
                AgentEvent::ToolUseStarted { id, tool, .. } => {
                    // 既知idの再送は捨てる(UIに同じツール呼び出しが二重に並ばないように)
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
            out.push(ev);
        }
        out
    }
}
