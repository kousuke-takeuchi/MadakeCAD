//! Anthropic Messages API(`POST /v1/messages`)へ直接つなぐバックエンド。
//!
//! Claude Code CLIが入っていない環境でも、APIキーだけでチャット作図が動くようにする。
//! CLIバックエンドとの違いは3点だけで、UIから見た振る舞い([`AgentEvent`])は同じ:
//!
//! - **認証**: APIキー。OSキーチェーンから読む([`crate::secrets`])。ここでは値を
//!   ログにもイベントにも出さない
//! - **ツール**: 内蔵MCPのツール定義をAPIの`tools`へブリッジし([`crate::tools`])、
//!   `tool_use`が返ったらこちらでツールを実行して`tool_result`を返す往復を回す。
//!   実行先はMCPサーバーそのものなので、編集は今までどおりCommandエンジンを通る
//! - **文脈**: APIはステートレスなので、会話の過去のやりとりを毎回送り直す
//!
//! ストリーミングはSSE(`stream: true`)。受け取ったイベントはその場で
//! [`AgentEvent`]へ変換して流すため、UI側の表示コードは無変更で動く。

use std::sync::Arc;

use serde_json::{json, Map, Value};
use tokio::sync::mpsc;

use crate::backend::{AgentBackend, TurnFuture, TurnRequest};
use crate::conversation::Role;
use crate::events::Usage;
use crate::tools::{api_tool_definitions, ToolBridge};
use crate::{AgentEvent, Result};

/// 既定のモデル(設定`api_model`で変更できる)。
pub const DEFAULT_API_MODEL: &str = "claude-sonnet-5";
/// APIの既定の接続先。
pub const ANTHROPIC_API_BASE: &str = "https://api.anthropic.com";
/// `anthropic-version`ヘッダの値。
pub const ANTHROPIC_VERSION: &str = "2023-06-01";

/// 接続先を差し替える環境変数(社内ゲートウェイ・テスト用)。
pub const API_BASE_ENV: &str = "MADAKE_ANTHROPIC_BASE_URL";

/// 1回の応答で許す最大トークン数。
///
/// ストリーミングなのでHTTPタイムアウトの心配は無く、図面編集の説明が途中で
/// 切れないだけの余裕を取る。
const DEFAULT_MAX_TOKENS: u32 = 32_000;

/// ツール実行の往復回数の上限(堂々巡りを止める)。
const DEFAULT_MAX_TOOL_ROUNDS: usize = 16;

/// Anthropic Messages APIを直接叩くバックエンド。
pub struct AnthropicApiBackend {
    /// APIキー。**表示・ログ出力しないこと**([`std::fmt::Debug`]も伏せてある)
    api_key: String,
    /// `model`へ渡すモデルID
    pub model: String,
    /// 接続先のベースURL(テストでモックサーバーへ向けるために公開)
    pub base_url: String,
    /// 1応答あたりの最大トークン数
    pub max_tokens: u32,
    /// ツール実行の往復回数の上限
    pub max_tool_rounds: usize,
    /// 図面編集ツールの窓口(`None`ならツール無しの会話だけ)
    tools: Option<Arc<dyn ToolBridge>>,
    client: reqwest::Client,
}

impl std::fmt::Debug for AnthropicApiBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // APIキーは伏せる(デバッグ出力からの漏洩を防ぐ)
        f.debug_struct("AnthropicApiBackend")
            .field("model", &self.model)
            .field("base_url", &self.base_url)
            .field("max_tokens", &self.max_tokens)
            .field("max_tool_rounds", &self.max_tool_rounds)
            .field("api_key", &"<非表示>")
            .field("tools", &self.tools.is_some())
            .finish()
    }
}

/// 既定の接続先。[`API_BASE_ENV`]があればそれを使う。
fn default_base_url() -> String {
    std::env::var(API_BASE_ENV)
        .ok()
        .map(|url| url.trim().trim_end_matches('/').to_string())
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| ANTHROPIC_API_BASE.to_string())
}

impl AnthropicApiBackend {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        let model = model.into();
        Self {
            api_key: api_key.into(),
            model: if model.trim().is_empty() {
                DEFAULT_API_MODEL.to_string()
            } else {
                model
            },
            base_url: default_base_url(),
            max_tokens: DEFAULT_MAX_TOKENS,
            max_tool_rounds: DEFAULT_MAX_TOOL_ROUNDS,
            tools: None,
            client: reqwest::Client::new(),
        }
    }

    /// 接続先を差し替える(テスト用のモックサーバー・プロキシ)。
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into().trim_end_matches('/').to_string();
        self
    }

    /// 図面編集ツールの窓口をつなぐ。
    pub fn with_tools(mut self, tools: Arc<dyn ToolBridge>) -> Self {
        self.tools = Some(tools);
        self
    }

    fn messages_url(&self) -> String {
        format!("{}/v1/messages", self.base_url.trim_end_matches('/'))
    }

    fn post(&self, body: &Value) -> reqwest::RequestBuilder {
        self.client
            .post(self.messages_url())
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(body)
    }

    /// 設定が使えるかを小さなリクエストで確かめる(設定画面の「接続テスト」)。
    ///
    /// 失敗は[`ConnectionError`]で返す。**APIキーは含めない。**
    pub async fn check_connection(&self) -> std::result::Result<(), ConnectionError> {
        let body = json!({
            "model": self.model,
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "ping"}],
        });
        let response = self.post(&body).send().await.map_err(|e| ConnectionError {
            kind: KIND_NETWORK.to_string(),
            message: format!("Anthropic APIへ接続できませんでした: {e}"),
        })?;
        let status = response.status().as_u16();
        if (200..300).contains(&status) {
            return Ok(());
        }
        let text = response.text().await.unwrap_or_default();
        let parsed = parse_api_error(&text);
        Err(ConnectionError {
            kind: error_kind(Some(status), &parsed.0).to_string(),
            message: friendly_error(Some(status), &parsed),
        })
    }

    /// 1ターン(必要ならツール実行の往復込み)を回す。
    async fn stream_turn(
        &self,
        request: TurnRequest<'_>,
        tx: &mpsc::Sender<AgentEvent>,
    ) -> Result<()> {
        let tool_defs = self
            .tools
            .as_ref()
            .map(|bridge| api_tool_definitions(&bridge.tools()))
            .unwrap_or_default();
        let mut messages = history_messages(request.history);
        messages.push(json!({"role": "user", "content": request.prompt}));

        let mut usage = Usage::default();
        for _round in 0..self.max_tool_rounds {
            let mut body = Map::new();
            body.insert("model".into(), json!(self.model));
            body.insert("max_tokens".into(), json!(self.max_tokens));
            body.insert("stream".into(), json!(true));
            body.insert("messages".into(), Value::Array(messages.clone()));
            if let Some(system) = request.system_prompt.filter(|s| !s.trim().is_empty()) {
                body.insert("system".into(), json!(system));
            }
            if !tool_defs.is_empty() {
                body.insert("tools".into(), Value::Array(tool_defs.clone()));
            }
            let body = Value::Object(body);

            let response = match self.post(&body).send().await {
                Ok(response) => response,
                Err(e) => {
                    send(tx, error_event(format!("Anthropic APIへ接続できませんでした: {e}"))).await;
                    return Ok(());
                }
            };
            let status = response.status().as_u16();
            if !(200..300).contains(&status) {
                let text = response.text().await.unwrap_or_default();
                send(
                    tx,
                    error_event(friendly_error(Some(status), &parse_api_error(&text))),
                )
                .await;
                return Ok(());
            }

            let round = read_stream(response, tx).await?;
            usage.input_tokens += round.usage.input_tokens;
            usage.output_tokens += round.usage.output_tokens;
            usage.cache_creation_input_tokens += round.usage.cache_creation_input_tokens;
            usage.cache_read_input_tokens += round.usage.cache_read_input_tokens;

            if let Some(error) = round.error {
                send(tx, error_event(error)).await;
                return Ok(());
            }
            if round.tool_uses.is_empty() {
                send(
                    tx,
                    AgentEvent::TurnCompleted {
                        result: round.text,
                        usage: Some(usage),
                    },
                )
                .await;
                return Ok(());
            }

            // モデルの発言(本文+ツール要求)をそのまま積み直し、結果を返して続ける
            messages.push(json!({"role": "assistant", "content": round.content_blocks}));
            let mut results = Vec::new();
            for tool_use in round.tool_uses {
                let outcome = match &self.tools {
                    Some(bridge) => bridge.call(&tool_use.name, tool_use.input.clone()).await,
                    // ツール窓口が無いのにツールを要求された(設定の取り違え)
                    None => crate::tools::ToolOutcome::error(
                        "このバックエンドでは図面編集ツールを使えません".to_string(),
                    ),
                };
                send(
                    tx,
                    AgentEvent::ToolUseFinished {
                        id: tool_use.id.clone(),
                        tool: tool_use.name.clone(),
                        is_error: outcome.is_error,
                    },
                )
                .await;
                results.push(json!({
                    "type": "tool_result",
                    "tool_use_id": tool_use.id,
                    "content": outcome.content,
                    "is_error": outcome.is_error,
                }));
            }
            messages.push(json!({"role": "user", "content": results}));
        }

        send(
            tx,
            error_event(format!(
                "ツール実行の往復が上限({}回)に達したので中断しました。\
                 指示を分けて小さく頼むと通ることがあります。",
                self.max_tool_rounds
            )),
        )
        .await;
        Ok(())
    }
}

impl AgentBackend for AnthropicApiBackend {
    fn run_turn<'a>(
        &'a self,
        request: TurnRequest<'a>,
        tx: mpsc::Sender<AgentEvent>,
    ) -> TurnFuture<'a> {
        Box::pin(async move {
            let error_tx = tx.clone();
            match self.stream_turn(request, &tx).await {
                Ok(()) => Ok(()),
                Err(e) => {
                    let _ = error_tx
                        .send(AgentEvent::Error {
                            message: e.to_string(),
                        })
                        .await;
                    Err(e)
                }
            }
        })
    }
}

/// 会話履歴 → APIの`messages`配列(本文だけ。過去ターンのツール往復は送り直さない)。
fn history_messages(history: &[crate::backend::HistoryMessage]) -> Vec<Value> {
    history
        .iter()
        .filter(|m| !m.text.trim().is_empty())
        .map(|m| {
            let role = match m.role {
                Role::User => "user",
                Role::Assistant => "assistant",
            };
            json!({"role": role, "content": m.text})
        })
        .collect()
}

async fn send(tx: &mpsc::Sender<AgentEvent>, event: AgentEvent) {
    let _ = tx.send(event).await;
}

fn error_event(message: String) -> AgentEvent {
    AgentEvent::Error { message }
}

/// ストリーム1本(=モデルの1発言)から取り出したもの。
#[derive(Debug, Default)]
struct StreamRound {
    /// 本文(テキストブロックの連結)
    text: String,
    /// 発言をそのまま積み直すためのcontentブロック列
    content_blocks: Vec<Value>,
    /// 実行すべきツール要求
    tool_uses: Vec<ToolUse>,
    usage: Usage,
    /// ストリーム内で届いたエラー
    error: Option<String>,
}

#[derive(Debug, Clone)]
struct ToolUse {
    id: String,
    name: String,
    input: Value,
}

/// 組み立て中のcontentブロック。
#[derive(Debug)]
enum Building {
    Text(String),
    Tool { id: String, name: String, json: String },
    Other,
}

/// SSEを読みながらイベントを流し、1発言分の結果を返す。
async fn read_stream(
    mut response: reqwest::Response,
    tx: &mpsc::Sender<AgentEvent>,
) -> Result<StreamRound> {
    let mut round = StreamRound::default();
    let mut buffer = String::new();
    let mut blocks: std::collections::BTreeMap<u64, Building> = Default::default();

    loop {
        let chunk = match response.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(e) => {
                round.error = Some(format!("Anthropic APIからの応答が途切れました: {e}"));
                break;
            }
        };
        buffer.push_str(&String::from_utf8_lossy(&chunk).replace("\r\n", "\n"));
        while let Some(split) = buffer.find("\n\n") {
            let frame: String = buffer.drain(..split + 2).collect();
            let Some(data) = sse_data(&frame) else {
                continue;
            };
            let Ok(event) = serde_json::from_str::<Value>(&data) else {
                continue;
            };
            if apply_stream_event(&event, &mut round, &mut blocks, tx).await {
                // message_stop / error でこの発言は終わり
                return Ok(finish_round(round, blocks));
            }
        }
    }
    Ok(finish_round(round, blocks))
}

/// SSEフレームから`data:`行の中身を取り出す(複数行は改行で連結)。
fn sse_data(frame: &str) -> Option<String> {
    let mut data = String::new();
    for line in frame.lines() {
        if let Some(rest) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest.trim_start());
        }
    }
    (!data.is_empty()).then_some(data)
}

/// 1イベントを反映する。戻り値`true`でこの発言の終わり。
async fn apply_stream_event(
    event: &Value,
    round: &mut StreamRound,
    blocks: &mut std::collections::BTreeMap<u64, Building>,
    tx: &mpsc::Sender<AgentEvent>,
) -> bool {
    match event.get("type").and_then(Value::as_str) {
        Some("message_start") => {
            if let Some(usage) = event.pointer("/message/usage") {
                round.usage.input_tokens += u64_at(usage, "input_tokens");
                round.usage.cache_creation_input_tokens +=
                    u64_at(usage, "cache_creation_input_tokens");
                round.usage.cache_read_input_tokens += u64_at(usage, "cache_read_input_tokens");
            }
        }
        Some("content_block_start") => {
            let index = u64_at(event, "index");
            let block = event.get("content_block");
            let building = match block.and_then(|b| b.get("type")).and_then(Value::as_str) {
                Some("text") => Building::Text(
                    block
                        .and_then(|b| b.get("text"))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                ),
                Some("tool_use") => Building::Tool {
                    id: str_at(block.unwrap_or(&Value::Null), "id"),
                    name: str_at(block.unwrap_or(&Value::Null), "name"),
                    json: String::new(),
                },
                _ => Building::Other,
            };
            blocks.insert(index, building);
        }
        Some("content_block_delta") => {
            let index = u64_at(event, "index");
            let delta = event.get("delta");
            let kind = delta.and_then(|d| d.get("type")).and_then(Value::as_str);
            match (kind, blocks.get_mut(&index)) {
                (Some("text_delta"), Some(Building::Text(text))) => {
                    let chunk = delta
                        .and_then(|d| d.get("text"))
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if !chunk.is_empty() {
                        text.push_str(chunk);
                        send(
                            tx,
                            AgentEvent::TextDelta {
                                text: chunk.to_string(),
                            },
                        )
                        .await;
                    }
                }
                (Some("input_json_delta"), Some(Building::Tool { json, .. })) => {
                    json.push_str(
                        delta
                            .and_then(|d| d.get("partial_json"))
                            .and_then(Value::as_str)
                            .unwrap_or_default(),
                    );
                }
                // thinking_delta / signature_delta などは表示しない
                _ => {}
            }
        }
        Some("content_block_stop") => {
            let index = u64_at(event, "index");
            // ツール要求は入力が揃った時点でUIへ知らせる(CLI経由と同じ見え方にする)
            if let Some(Building::Tool { id, name, json }) = blocks.get(&index) {
                let input = parse_tool_input(json);
                send(
                    tx,
                    AgentEvent::ToolUseStarted {
                        id: id.clone(),
                        tool: name.clone(),
                        input: input.clone(),
                    },
                )
                .await;
                round.tool_uses.push(ToolUse {
                    id: id.clone(),
                    name: name.clone(),
                    input,
                });
            }
        }
        Some("message_delta") => {
            if let Some(usage) = event.get("usage") {
                round.usage.output_tokens += u64_at(usage, "output_tokens");
            }
        }
        Some("message_stop") => return true,
        Some("error") => {
            round.error = Some(friendly_error(None, &parse_api_error(&event.to_string())));
            return true;
        }
        _ => {}
    }
    false
}

/// 組み立て中のブロックを確定して発言全体をまとめる。
fn finish_round(
    mut round: StreamRound,
    blocks: std::collections::BTreeMap<u64, Building>,
) -> StreamRound {
    for building in blocks.into_values() {
        match building {
            Building::Text(text) => {
                if text.is_empty() {
                    // 空のテキストブロックはAPIが受け付けないので積み直さない
                    continue;
                }
                round.text.push_str(&text);
                round
                    .content_blocks
                    .push(json!({"type": "text", "text": text}));
            }
            Building::Tool { id, name, json } => {
                round.content_blocks.push(json!({
                    "type": "tool_use",
                    "id": id,
                    "name": name,
                    "input": parse_tool_input(&json),
                }));
            }
            Building::Other => {}
        }
    }
    round
}

/// ツール入力のJSON文字列を値へ(空・壊れている場合は空オブジェクト)。
fn parse_tool_input(json: &str) -> Value {
    if json.trim().is_empty() {
        return Value::Object(Map::new());
    }
    serde_json::from_str(json).unwrap_or_else(|_| Value::Object(Map::new()))
}

fn u64_at(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn str_at(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// APIのエラーボディから`(種類, 本文)`を取り出す。
fn parse_api_error(text: &str) -> (String, String) {
    let value: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let kind = value
        .pointer("/error/type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let message = value
        .pointer("/error/message")
        .and_then(Value::as_str)
        .map(|m| m.to_string())
        .unwrap_or_else(|| text.trim().chars().take(300).collect());
    (kind, message)
}

/// 接続テストの失敗。`kind`はUIが翻訳するための機械可読な区分、`message`は
/// 翻訳が無い場合にそのまま出せる説明。**どちらにもAPIキーは含めない。**
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ConnectionError {
    pub kind: String,
    pub message: String,
}

/// 接続できなかった理由の区分(UIの翻訳キーに使う)。
pub const KIND_AUTH: &str = "auth";
pub const KIND_OVERLOADED: &str = "overloaded";
pub const KIND_RATE_LIMIT: &str = "rate_limit";
pub const KIND_MODEL_NOT_FOUND: &str = "model_not_found";
pub const KIND_SERVER: &str = "server";
pub const KIND_REQUEST: &str = "request";
pub const KIND_NETWORK: &str = "network";
pub const KIND_UNKNOWN: &str = "unknown";

/// HTTPステータスとAPIのエラー種別から区分を決める。
pub fn error_kind(status: Option<u16>, kind: &str) -> &'static str {
    let status = status.unwrap_or(0);
    if status == 401 || status == 403 || kind == "authentication_error" || kind == "permission_error"
    {
        KIND_AUTH
    } else if status == 529 || kind == "overloaded_error" {
        KIND_OVERLOADED
    } else if status == 429 || kind == "rate_limit_error" {
        KIND_RATE_LIMIT
    } else if status == 404 || kind == "not_found_error" {
        KIND_MODEL_NOT_FOUND
    } else if (500..600).contains(&status) || kind == "api_error" {
        KIND_SERVER
    } else if status == 400 || kind == "invalid_request_error" {
        KIND_REQUEST
    } else {
        KIND_UNKNOWN
    }
}

/// APIのエラーを画面へ出せる日本語にする。**APIキーは決して含めない。**
fn friendly_error(status: Option<u16>, (kind, message): &(String, String)) -> String {
    let status = status.unwrap_or(0);
    if status == 401 || status == 403 || kind == "authentication_error" || kind == "permission_error"
    {
        return "APIキーが受け付けられませんでした。設定 > エージェント でAPIキーを\
                入れ直してください(キーはOSのキーチェーンに保存されます)。"
            .to_string();
    }
    if status == 529 || kind == "overloaded_error" {
        return "Anthropic APIが混雑しています。少し時間をおいてもう一度お試しください。"
            .to_string();
    }
    if status == 429 || kind == "rate_limit_error" {
        return "Anthropic APIの利用上限に達しました。少し時間をおいてからお試しください。"
            .to_string();
    }
    if status == 404 || kind == "not_found_error" {
        return format!(
            "指定したモデルが見つかりません。設定 > エージェント のモデル名を確認してください({message})"
        );
    }
    if (500..600).contains(&status) || kind == "api_error" {
        return format!("Anthropic API側でエラーが起きました({message})");
    }
    if status == 400 || kind == "invalid_request_error" {
        return format!("Anthropic APIがリクエストを受け付けませんでした: {message}");
    }
    if status == 0 {
        format!("Anthropic APIからエラーが届きました: {message}")
    } else {
        format!("Anthropic APIからエラーが届きました (HTTP {status}): {message}")
    }
}
