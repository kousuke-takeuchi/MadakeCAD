//! OpenAI互換のChat Completions API(`POST {base_url}/chat/completions`)へつなぐバックエンド。
//!
//! 「OpenAI互換」を名乗るサービスは同じ形のリクエストを受け付けるので、**接続先URLと
//! モデル名を変えるだけ**で下記がすべて同じ経路で動く:
//!
//! - OpenAI本体(`https://api.openai.com/v1`)
//! - xAI / OpenRouter / 社内ゲートウェイ(それぞれのURL)
//! - **ローカルのOllama**(`http://localhost:11434/v1`。APIキー不要)
//!
//! [`crate::AnthropicApiBackend`]との違いはリクエストの形だけで、UIから見た振る舞い
//! ([`AgentEvent`])は同じ:
//!
//! - **認証**: `Authorization: Bearer <キー>`。キーはOSキーチェーンから読む
//!   ([`crate::secrets`])。**キーが無ければヘッダごと省く**ので、キーを要求しない
//!   ローカルサーバー(Ollama等)にそのままつながる
//! - **システム指示**: OpenAI互換APIに`system`引数は無いため、`messages`の先頭へ
//!   `role: "system"`のメッセージとして積む
//! - **ツール**: 内蔵MCPのツール定義をOpenAIのfunction形式へ変換して渡し
//!   ([`openai_tool_definitions`])、`tool_calls`が返ったらこちらで実行して
//!   `role: "tool"`のメッセージで返す往復を回す。実行先はMCPサーバーそのものなので、
//!   編集は今までどおりCommandエンジンを通る
//! - **文脈**: APIはステートレスなので、会話の過去のやりとりを毎回送り直す
//!
//! ストリーミングはSSE(`stream: true`)。`data: {...}`の行を受け取ったその場で
//! [`AgentEvent`]へ変換して流すため、UI側の表示コードは無変更で動く。

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{json, Map, Value};
use tokio::sync::mpsc;

use crate::anthropic::ConnectionError;
use crate::backend::{AgentBackend, TurnFuture, TurnRequest};
use crate::conversation::Role;
use crate::events::Usage;
use crate::tools::{ToolBridge, ToolDef};
use crate::{AgentEvent, Result};

/// 既定の接続先(OpenAI本体)。
pub const DEFAULT_OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

/// ローカルOllamaのOpenAI互換エンドポイント(設定画面の「Ollama」プリセット)。
///
/// Ollamaはこのエンドポイントで**APIキーを要求しない**ので、キー未保存のまま使える。
pub const OLLAMA_BASE_URL: &str = "http://localhost:11434/v1";

/// 接続先の既定値を差し替える環境変数(社内ゲートウェイ・テスト用)。
///
/// 設定のURLが空のときだけ効く(設定を明示したらそちらが優先)。
pub const OPENAI_BASE_URL_ENV: &str = "MADAKE_OPENAI_BASE_URL";

/// ツール実行の往復回数の上限(堂々巡りを止める)。
const DEFAULT_MAX_TOOL_ROUNDS: usize = 16;

/// ツールが失敗したことをモデルへ伝える印。
///
/// OpenAIの`role: "tool"`メッセージには成否の欄が無いため、本文の先頭に付けて
/// 「これは失敗の報告だ」と分かるようにする(モデルが直しにいける)。
pub const TOOL_ERROR_PREFIX: &str = "ERROR: ";

/// 接続テストの失敗区分(UIの翻訳キー。Anthropic用と混ざらないよう接頭辞をつける)。
pub const KIND_OPENAI_AUTH: &str = "openai_auth";
pub const KIND_OPENAI_RATE_LIMIT: &str = "openai_rate_limit";
pub const KIND_OPENAI_MODEL_NOT_FOUND: &str = "openai_model_not_found";
pub const KIND_OPENAI_SERVER: &str = "openai_server";
pub const KIND_OPENAI_REQUEST: &str = "openai_request";
pub const KIND_OPENAI_NETWORK: &str = "openai_network";
pub const KIND_OPENAI_UNKNOWN: &str = "openai_unknown";
/// キーが必要なURLなのにキーが保存されていない(通信する前に分かる)。
pub const KIND_OPENAI_NO_KEY: &str = "openai_no_key";
/// モデル名が空のまま(通信する前に分かる)。
pub const KIND_OPENAI_NO_MODEL: &str = "openai_no_model";

/// OpenAI互換のChat Completions APIを叩くバックエンド。
pub struct OpenAiCompatBackend {
    /// APIキー。`None`ならAuthorizationヘッダを送らない(ローカルサーバー用)。
    /// **表示・ログ出力しないこと**([`std::fmt::Debug`]も伏せてある)
    api_key: Option<String>,
    /// `model`へ渡すモデル名(接続先ごとに正解が違うので既定値は置かない)
    pub model: String,
    /// 接続先のベースURL(`/chat/completions`の1つ上)
    pub base_url: String,
    /// ツール実行の往復回数の上限
    pub max_tool_rounds: usize,
    /// 図面編集ツールの窓口(`None`ならツール無しの会話だけ)
    tools: Option<Arc<dyn ToolBridge>>,
    client: reqwest::Client,
}

impl std::fmt::Debug for OpenAiCompatBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // APIキーは伏せる(デバッグ出力からの漏洩を防ぐ)
        f.debug_struct("OpenAiCompatBackend")
            .field("model", &self.model)
            .field("base_url", &self.base_url)
            .field("max_tool_rounds", &self.max_tool_rounds)
            .field("api_key", &self.api_key.as_ref().map(|_| "<非表示>"))
            .field("tools", &self.tools.is_some())
            .finish()
    }
}

/// 接続先URLがこのPCの中を指しているか(=APIキーを要求しない前提で使えるか)。
///
/// Ollamaなどのローカルサーバーはキーを要求しないため、キー未保存でも
/// 「使える」と判定してよい。判定はホスト名だけで行う。
pub fn is_local_url(url: &str) -> bool {
    let rest = url
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(url)
        .trim_start_matches('/');
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        // ユーザー情報(user@host)とポートを落とす
        .rsplit('@')
        .next()
        .unwrap_or_default();
    let host = host.rsplit_once(':').map(|(h, _)| h).unwrap_or(host);
    let host = host.trim_matches(['[', ']']).to_ascii_lowercase();
    host == "localhost"
        || host == "127.0.0.1"
        || host == "::1"
        || host == "0.0.0.0"
        || host.ends_with(".localhost")
        || host.ends_with(".local")
}

/// 空のURLを既定([`OPENAI_BASE_URL_ENV`]があればそれ)へ寄せ、末尾の`/`を落とす。
pub fn resolve_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    std::env::var(OPENAI_BASE_URL_ENV)
        .ok()
        .map(|url| url.trim().trim_end_matches('/').to_string())
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| DEFAULT_OPENAI_BASE_URL.to_string())
}

impl OpenAiCompatBackend {
    /// `api_key`が`None`ならAuthorizationヘッダを付けない(ローカルサーバー向け)。
    pub fn new(
        api_key: Option<String>,
        model: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            api_key: api_key
                .map(|key| key.trim().to_string())
                .filter(|key| !key.is_empty()),
            model: model.into().trim().to_string(),
            base_url: resolve_base_url(&base_url.into()),
            max_tool_rounds: DEFAULT_MAX_TOOL_ROUNDS,
            tools: None,
            client: reqwest::Client::new(),
        }
    }

    /// 図面編集ツールの窓口をつなぐ。
    pub fn with_tools(mut self, tools: Arc<dyn ToolBridge>) -> Self {
        self.tools = Some(tools);
        self
    }

    fn completions_url(&self) -> String {
        format!("{}/chat/completions", self.base_url)
    }

    fn post(&self, body: &Value) -> reqwest::RequestBuilder {
        let request = self
            .client
            .post(self.completions_url())
            .header("content-type", "application/json");
        // キーが無いときはヘッダごと省く(空のBearerを拒むローカルサーバーがあるため)
        match &self.api_key {
            Some(key) => request.bearer_auth(key),
            None => request,
        }
        .json(body)
    }

    /// 設定が使えるかを小さなリクエストで確かめる(設定画面の「接続テスト」)。
    ///
    /// 失敗は[`ConnectionError`]で返す。**APIキーは含めない。**
    pub async fn check_connection(&self) -> std::result::Result<(), ConnectionError> {
        if self.model.is_empty() {
            return Err(ConnectionError {
                kind: KIND_OPENAI_NO_MODEL.to_string(),
                message: NO_MODEL_MESSAGE.to_string(),
            });
        }
        let body = json!({
            "model": self.model,
            "messages": [{"role": "user", "content": "ping"}],
            "max_tokens": 1,
            "stream": false,
        });
        let response = self.post(&body).send().await.map_err(|e| ConnectionError {
            kind: KIND_OPENAI_NETWORK.to_string(),
            message: self.network_error(&e.to_string()),
        })?;
        let status = response.status().as_u16();
        if (200..300).contains(&status) {
            return Ok(());
        }
        let text = response.text().await.unwrap_or_default();
        let parsed = parse_api_error(&text);
        Err(ConnectionError {
            kind: error_kind(status, &parsed.0).to_string(),
            message: self.friendly_error(status, &parsed),
        })
    }

    /// 1ターン(必要ならツール実行の往復込み)を回す。
    async fn stream_turn(
        &self,
        request: TurnRequest<'_>,
        tx: &mpsc::Sender<AgentEvent>,
    ) -> Result<()> {
        if self.model.is_empty() {
            send(tx, error_event(NO_MODEL_MESSAGE.to_string())).await;
            return Ok(());
        }
        let tool_defs = self
            .tools
            .as_ref()
            .map(|bridge| openai_tool_definitions(&bridge.tools()))
            .unwrap_or_default();

        let mut messages = Vec::new();
        // OpenAI互換APIに`system`引数は無いので、先頭のメッセージとして積む
        if let Some(system) = request.system_prompt.filter(|s| !s.trim().is_empty()) {
            messages.push(json!({"role": "system", "content": system}));
        }
        messages.extend(history_messages(request.history));
        messages.push(json!({"role": "user", "content": request.prompt}));

        let mut usage = Usage::default();
        for _round in 0..self.max_tool_rounds {
            let mut body = Map::new();
            body.insert("model".into(), json!(self.model));
            body.insert("stream".into(), json!(true));
            // ストリーミングでも使用量を返してもらう(OpenAIはこの指定が要る)
            body.insert("stream_options".into(), json!({"include_usage": true}));
            body.insert("messages".into(), Value::Array(messages.clone()));
            if !tool_defs.is_empty() {
                body.insert("tools".into(), Value::Array(tool_defs.clone()));
            }
            let body = Value::Object(body);

            let response = match self.post(&body).send().await {
                Ok(response) => response,
                Err(e) => {
                    send(tx, error_event(self.network_error(&e.to_string()))).await;
                    return Ok(());
                }
            };
            let status = response.status().as_u16();
            if !(200..300).contains(&status) {
                let text = response.text().await.unwrap_or_default();
                send(
                    tx,
                    error_event(self.friendly_error(status, &parse_api_error(&text))),
                )
                .await;
                return Ok(());
            }

            let round = read_stream(response, tx).await?;
            usage.input_tokens += round.usage.input_tokens;
            usage.output_tokens += round.usage.output_tokens;
            usage.cache_read_input_tokens += round.usage.cache_read_input_tokens;

            if let Some(error) = round.error {
                send(tx, error_event(error)).await;
                return Ok(());
            }
            if round.tool_calls.is_empty() {
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
            messages.push(assistant_message(&round));
            for call in &round.tool_calls {
                let outcome = match &self.tools {
                    Some(bridge) => bridge.call(&call.name, call.arguments_value()).await,
                    // ツール窓口が無いのにツールを要求された(設定の取り違え)
                    None => crate::tools::ToolOutcome::error(
                        "このバックエンドでは図面編集ツールを使えません".to_string(),
                    ),
                };
                send(
                    tx,
                    AgentEvent::ToolUseFinished {
                        id: call.id.clone(),
                        tool: call.name.clone(),
                        is_error: outcome.is_error,
                    },
                )
                .await;
                let content = if outcome.is_error {
                    format!("{TOOL_ERROR_PREFIX}{}", outcome.content)
                } else {
                    outcome.content
                };
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": call.id,
                    "content": content,
                }));
            }
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

    /// 接続そのものに失敗したときの説明(試したURLを必ず添える)。
    fn network_error(&self, detail: &str) -> String {
        if is_local_url(&self.base_url) {
            format!(
                "{}へ接続できませんでした({detail})。Ollamaなどのローカルサーバーが\
                 起動しているか(ターミナルで`ollama serve`)、設定 > エージェント の\
                 URLが合っているかを確認してください。",
                self.completions_url_for_message()
            )
        } else {
            format!(
                "{}へ接続できませんでした({detail})。設定 > エージェント のURLと\
                 ネットワーク接続を確認してください。",
                self.completions_url_for_message()
            )
        }
    }

    /// エラー文に載せるURL(**キーは含まない**)。
    fn completions_url_for_message(&self) -> String {
        self.base_url.clone()
    }

    /// APIのエラーを画面へ出せる日本語にする。**APIキーは決して含めない。**
    fn friendly_error(&self, status: u16, (kind, message): &(String, String)) -> String {
        match error_kind(status, kind) {
            KIND_OPENAI_AUTH => "APIキーが受け付けられませんでした。設定 > エージェント で\
                 APIキーを入れ直してください(キーはOSのキーチェーンに保存されます)。\
                 ローカルのOllamaを使う場合はキーは不要です。"
                .to_string(),
            KIND_OPENAI_RATE_LIMIT => {
                "APIの利用上限(レート制限)に達しました。少し時間をおいてからお試しください。"
                    .to_string()
            }
            KIND_OPENAI_MODEL_NOT_FOUND => format!(
                "モデル「{}」が見つかりません。設定 > エージェント のモデル名を確認してください\
                 (Ollamaなら`ollama list`にある名前)。({message})",
                self.model
            ),
            KIND_OPENAI_SERVER => {
                format!("接続先({})でエラーが起きました({message})", self.base_url)
            }
            KIND_OPENAI_REQUEST => format!("接続先がリクエストを受け付けませんでした: {message}"),
            _ => format!("接続先からエラーが届きました (HTTP {status}): {message}"),
        }
    }
}

/// モデル名が空のときの案内。
pub const NO_MODEL_MESSAGE: &str = "モデル名が設定されていません(設定 > エージェント の\
     「モデル」に、接続先で使えるモデル名を入力してください)。";

impl AgentBackend for OpenAiCompatBackend {
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

/// MCPツール定義 → OpenAIのfunction形式(`tools`配列)。
///
/// 名前・説明・スキーマの整え方はAnthropic版([`crate::tools::api_tool_definitions`])と
/// 同じものを使い、包み方だけOpenAIの形にする。
pub fn openai_tool_definitions(defs: &[ToolDef]) -> Vec<Value> {
    crate::tools::api_tool_definitions(defs)
        .into_iter()
        .map(|def| {
            json!({
                "type": "function",
                "function": {
                    "name": def["name"],
                    "description": def["description"],
                    "parameters": def["input_schema"],
                }
            })
        })
        .collect()
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

/// ツール要求を含むアシスタントの発言を積み直す形にする。
fn assistant_message(round: &StreamRound) -> Value {
    let calls: Vec<Value> = round
        .tool_calls
        .iter()
        .map(|call| {
            json!({
                "id": call.id,
                "type": "function",
                "function": {"name": call.name, "arguments": call.arguments},
            })
        })
        .collect();
    let mut message = Map::new();
    message.insert("role".into(), json!("assistant"));
    // 本文が無いときも`content`は省かずnullで置く(必須とするサーバーがあるため)
    message.insert(
        "content".into(),
        if round.text.is_empty() {
            Value::Null
        } else {
            json!(round.text)
        },
    );
    message.insert("tool_calls".into(), Value::Array(calls));
    Value::Object(message)
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
    /// 本文(deltaの連結)
    text: String,
    /// 実行すべきツール要求(届いた順)
    tool_calls: Vec<ToolCall>,
    usage: Usage,
    /// ストリーム内で届いたエラー
    error: Option<String>,
}

/// 組み立て中/確定したツール要求。
#[derive(Debug, Clone, Default)]
struct ToolCall {
    id: String,
    name: String,
    /// 引数のJSON**文字列**(OpenAIは文字列で渡してくる。細切れで届く)
    arguments: String,
}

impl ToolCall {
    /// 引数のJSON文字列を値へ(空・壊れている場合は空オブジェクト)。
    fn arguments_value(&self) -> Value {
        if self.arguments.trim().is_empty() {
            return Value::Object(Map::new());
        }
        serde_json::from_str(&self.arguments).unwrap_or_else(|_| Value::Object(Map::new()))
    }
}

/// SSEを読みながらイベントを流し、1発言分の結果を返す。
async fn read_stream(
    mut response: reqwest::Response,
    tx: &mpsc::Sender<AgentEvent>,
) -> Result<StreamRound> {
    let mut round = StreamRound::default();
    let mut buffer = String::new();
    // ツール要求は`index`ごとに組み立てる(複数同時に届く)
    let mut calls: BTreeMap<u64, ToolCall> = BTreeMap::new();
    let mut done = false;

    loop {
        let chunk = match response.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(e) => {
                round.error = Some(format!("接続先からの応答が途切れました: {e}"));
                break;
            }
        };
        buffer.push_str(&String::from_utf8_lossy(&chunk).replace("\r\n", "\n"));
        while let Some(split) = buffer.find("\n\n") {
            let frame: String = buffer.drain(..split + 2).collect();
            let Some(data) = sse_data(&frame) else {
                continue;
            };
            if data.trim() == "[DONE]" {
                done = true;
                break;
            }
            let Ok(event) = serde_json::from_str::<Value>(&data) else {
                continue;
            };
            apply_chunk(&event, &mut round, &mut calls, tx).await;
        }
        if done {
            break;
        }
    }

    // 引数が揃った時点でUIへ知らせる(CLI経由と同じ見え方にする)
    for (index, mut call) in calls {
        if call.id.is_empty() {
            // idを返さないサーバー(Ollama等)向けの補い
            call.id = format!("call_{index}");
        }
        send(
            tx,
            AgentEvent::ToolUseStarted {
                id: call.id.clone(),
                tool: call.name.clone(),
                input: call.arguments_value(),
            },
        )
        .await;
        round.tool_calls.push(call);
    }
    Ok(round)
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

/// チャンク1つを反映する。
async fn apply_chunk(
    event: &Value,
    round: &mut StreamRound,
    calls: &mut BTreeMap<u64, ToolCall>,
    tx: &mpsc::Sender<AgentEvent>,
) {
    // エラーはボディがSSEで来ることもある
    if let Some(error) = event.get("error") {
        round.error = Some(
            error
                .get("message")
                .and_then(Value::as_str)
                .map(|m| format!("接続先からエラーが届きました: {m}"))
                .unwrap_or_else(|| "接続先からエラーが届きました".to_string()),
        );
        return;
    }
    if let Some(usage) = event.get("usage").filter(|u| u.is_object()) {
        round.usage.input_tokens += u64_at(usage, "prompt_tokens");
        round.usage.output_tokens += u64_at(usage, "completion_tokens");
        if let Some(details) = usage.get("prompt_tokens_details") {
            round.usage.cache_read_input_tokens += u64_at(details, "cached_tokens");
        }
    }
    let Some(delta) = event.pointer("/choices/0/delta") else {
        return;
    };
    if let Some(text) = delta.get("content").and_then(Value::as_str) {
        if !text.is_empty() {
            round.text.push_str(text);
            send(
                tx,
                AgentEvent::TextDelta {
                    text: text.to_string(),
                },
            )
            .await;
        }
    }
    let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) else {
        return;
    };
    for (position, call) in tool_calls.iter().enumerate() {
        // `index`が無いサーバーもあるので、配列上の位置で代用する
        let index = call
            .get("index")
            .and_then(Value::as_u64)
            .unwrap_or(position as u64);
        let entry = calls.entry(index).or_default();
        if let Some(id) = call.get("id").and_then(Value::as_str) {
            if !id.is_empty() {
                entry.id = id.to_string();
            }
        }
        if let Some(function) = call.get("function") {
            if let Some(name) = function.get("name").and_then(Value::as_str) {
                if !name.is_empty() {
                    entry.name = name.to_string();
                }
            }
            if let Some(args) = function.get("arguments").and_then(Value::as_str) {
                entry.arguments.push_str(args);
            }
        }
    }
}

fn u64_at(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(Value::as_u64).unwrap_or(0)
}

/// APIのエラーボディから`(種類, 本文)`を取り出す。
///
/// OpenAIは`{"error": {"message", "type", "code"}}`、Ollamaは`{"error": "..."}`。
fn parse_api_error(text: &str) -> (String, String) {
    let value: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let kind = value
        .pointer("/error/type")
        .or_else(|| value.pointer("/error/code"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let message = value
        .pointer("/error/message")
        .and_then(Value::as_str)
        .or_else(|| value.get("error").and_then(Value::as_str))
        .map(|m| m.to_string())
        .unwrap_or_else(|| text.trim().chars().take(300).collect());
    (kind, message)
}

/// HTTPステータスとAPIのエラー種別から区分を決める(UIの翻訳キー)。
pub fn error_kind(status: u16, kind: &str) -> &'static str {
    if status == 401 || status == 403 || kind.contains("api_key") || kind == "authentication_error"
    {
        KIND_OPENAI_AUTH
    } else if status == 429 || kind.contains("rate_limit") || kind == "insufficient_quota" {
        KIND_OPENAI_RATE_LIMIT
    } else if status == 404 || kind.contains("model_not_found") {
        KIND_OPENAI_MODEL_NOT_FOUND
    } else if (500..600).contains(&status) || kind == "server_error" {
        KIND_OPENAI_SERVER
    } else if status == 400 || status == 422 || kind == "invalid_request_error" {
        KIND_OPENAI_REQUEST
    } else {
        KIND_OPENAI_UNKNOWN
    }
}
