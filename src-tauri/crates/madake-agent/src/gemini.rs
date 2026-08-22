//! Google Gemini API(`generativelanguage.googleapis.com`)へつなぐバックエンド。
//!
//! [`crate::AnthropicApiBackend`]・[`crate::OpenAiCompatBackend`]との違いはリクエストの
//! 形だけで、UIから見た振る舞い([`AgentEvent`])は同じ:
//!
//! - **接続先**: `POST {base}/models/{model}:streamGenerateContent?alt=sse`
//!   (ストリーミング。接続テストだけは`:generateContent`)
//! - **認証**: `x-goog-api-key`ヘッダ。**URLのクエリには載せない**(URLは履歴やログへ
//!   残りやすいため)。キーはOSキーチェーンから読む([`crate::secrets`])
//! - **システム指示**: Geminiには専用の`systemInstruction`欄があるのでそこへ入れる
//!   (OpenAI互換のように会話の先頭へ積む必要はない)
//! - **ツール**: 内蔵MCPのツール定義をGeminiのfunctionDeclarations形式へ変換して渡し
//!   ([`gemini_tool_definitions`])、`functionCall`が返ったらこちらで実行して
//!   `functionResponse`で返す往復を回す。実行先はMCPサーバーそのものなので、
//!   編集は今までどおりCommandエンジンを通る
//! - **文脈**: APIはステートレスなので、会話の過去のやりとりを毎回送り直す
//!   (アシスタントの発言は`model`ロール)
//!
//! **JSON Schemaの方言に注意**: Geminiのスキーマは*OpenAPIの部分集合*で、
//! `$schema` / `$ref` / `additionalProperties` / `allOf`のような項目を受け付けず、
//! 送ると400になる。そのため[`gemini_schema`]で「Geminiが読める項目だけ」に削ってから
//! 渡す(削った結果として型が無くなったスキーマには型を補う)。

use std::sync::Arc;

use serde_json::{json, Map, Value};
use tokio::sync::mpsc;

use crate::anthropic::ConnectionError;
use crate::backend::{AgentBackend, TurnFuture, TurnRequest};
use crate::conversation::Role;
use crate::events::Usage;
use crate::tools::{ToolBridge, ToolDef};
use crate::{AgentEvent, Result};

/// 既定の接続先(モデル名の1つ上まで)。
pub const DEFAULT_GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

/// 既定のモデル(速度と価格のつり合いが良い現行の安定版)。
pub const DEFAULT_GEMINI_MODEL: &str = "gemini-2.5-flash";

/// 接続先の既定値を差し替える環境変数(テスト・社内ゲートウェイ用)。
pub const GEMINI_BASE_URL_ENV: &str = "MADAKE_GEMINI_BASE_URL";

/// APIキーを載せるヘッダ名(Geminiの流儀)。
const API_KEY_HEADER: &str = "x-goog-api-key";

/// ツール実行の往復回数の上限(堂々巡りを止める)。
const DEFAULT_MAX_TOOL_ROUNDS: usize = 16;

/// 接続テストの失敗区分(UIの翻訳キー。他プロバイダと混ざらないよう接頭辞をつける)。
pub const KIND_GEMINI_AUTH: &str = "gemini_auth";
pub const KIND_GEMINI_RATE_LIMIT: &str = "gemini_rate_limit";
pub const KIND_GEMINI_MODEL_NOT_FOUND: &str = "gemini_model_not_found";
pub const KIND_GEMINI_SERVER: &str = "gemini_server";
pub const KIND_GEMINI_REQUEST: &str = "gemini_request";
pub const KIND_GEMINI_NETWORK: &str = "gemini_network";
pub const KIND_GEMINI_UNKNOWN: &str = "gemini_unknown";
/// キーが保存されていない(通信する前に分かる)。
pub const KIND_GEMINI_NO_KEY: &str = "gemini_no_key";
/// モデル名が空のまま(通信する前に分かる)。
pub const KIND_GEMINI_NO_MODEL: &str = "gemini_no_model";

/// モデル名が空のときの案内。
pub const NO_MODEL_MESSAGE: &str = "モデル名が設定されていません(設定 > エージェント の\
     「モデル」に、使うGeminiのモデル名を入力してください)。";

/// Gemini APIを叩くバックエンド。
pub struct GeminiBackend {
    /// APIキー。**表示・ログ出力しないこと**([`std::fmt::Debug`]も伏せてある)
    api_key: String,
    /// 使うモデル名(`models/`接頭辞は付いていてもよい)
    pub model: String,
    /// 接続先のベースURL(`models/...`の1つ上)
    pub base_url: String,
    /// ツール実行の往復回数の上限
    pub max_tool_rounds: usize,
    /// 図面編集ツールの窓口(`None`ならツール無しの会話だけ)
    tools: Option<Arc<dyn ToolBridge>>,
    client: reqwest::Client,
}

impl std::fmt::Debug for GeminiBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // APIキーは伏せる(デバッグ出力からの漏洩を防ぐ)
        f.debug_struct("GeminiBackend")
            .field("model", &self.model)
            .field("base_url", &self.base_url)
            .field("max_tool_rounds", &self.max_tool_rounds)
            .field("api_key", &"<非表示>")
            .field("tools", &self.tools.is_some())
            .finish()
    }
}

/// 空のURLを既定([`GEMINI_BASE_URL_ENV`]があればそれ)へ寄せ、末尾の`/`を落とす。
pub fn resolve_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim().trim_end_matches('/');
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    std::env::var(GEMINI_BASE_URL_ENV)
        .ok()
        .map(|url| url.trim().trim_end_matches('/').to_string())
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| DEFAULT_GEMINI_BASE_URL.to_string())
}

/// 貼り付けた`models/gemini-2.5-flash`のような書き方も受け取れるようにする。
fn model_id(model: &str) -> &str {
    model.trim().trim_start_matches("models/")
}

impl GeminiBackend {
    pub fn new(
        api_key: impl Into<String>,
        model: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            api_key: api_key.into().trim().to_string(),
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

    /// `{base}/models/{model}:{method}`。**キーはURLに載せない。**
    fn endpoint(&self, method: &str) -> String {
        format!(
            "{}/models/{}:{method}",
            self.base_url,
            model_id(&self.model)
        )
    }

    fn post(&self, url: String, body: &Value) -> reqwest::RequestBuilder {
        self.client
            .post(url)
            .header("content-type", "application/json")
            .header(API_KEY_HEADER, &self.api_key)
            .json(body)
    }

    /// 設定が使えるかを小さなリクエストで確かめる(設定画面の「接続テスト」)。
    ///
    /// 失敗は[`ConnectionError`]で返す。**APIキーは含めない。**
    pub async fn check_connection(&self) -> std::result::Result<(), ConnectionError> {
        if model_id(&self.model).is_empty() {
            return Err(ConnectionError {
                kind: KIND_GEMINI_NO_MODEL.to_string(),
                message: NO_MODEL_MESSAGE.to_string(),
            });
        }
        let body = json!({
            "contents": [{"role": "user", "parts": [{"text": "ping"}]}],
            "generationConfig": {"maxOutputTokens": 1},
        });
        let response = self
            .post(self.endpoint("generateContent"), &body)
            .send()
            .await
            .map_err(|e| ConnectionError {
                kind: KIND_GEMINI_NETWORK.to_string(),
                message: self.network_error(&e.to_string()),
            })?;
        let status = response.status().as_u16();
        if (200..300).contains(&status) {
            return Ok(());
        }
        let text = response.text().await.unwrap_or_default();
        let parsed = ApiError::parse(&text);
        Err(ConnectionError {
            kind: error_kind(status, &parsed).to_string(),
            message: self.friendly_error(status, &parsed),
        })
    }

    /// 1ターン(必要ならツール実行の往復込み)を回す。
    async fn stream_turn(
        &self,
        request: TurnRequest<'_>,
        tx: &mpsc::Sender<AgentEvent>,
    ) -> Result<()> {
        if model_id(&self.model).is_empty() {
            send(tx, error_event(NO_MODEL_MESSAGE.to_string())).await;
            return Ok(());
        }
        let tool_defs = self
            .tools
            .as_ref()
            .map(|bridge| gemini_tool_definitions(&bridge.tools()))
            .unwrap_or_default();

        let mut contents = history_contents(request.history);
        contents.push(json!({"role": "user", "parts": [{"text": request.prompt}]}));

        let mut usage = Usage::default();
        for _round in 0..self.max_tool_rounds {
            let mut body = Map::new();
            // Geminiのシステム指示は専用の欄(contentsの先頭ではない)
            if let Some(system) = request.system_prompt.filter(|s| !s.trim().is_empty()) {
                body.insert(
                    "systemInstruction".into(),
                    json!({"parts": [{"text": system}]}),
                );
            }
            body.insert("contents".into(), Value::Array(contents.clone()));
            if !tool_defs.is_empty() {
                body.insert("tools".into(), Value::Array(tool_defs.clone()));
            }
            let body = Value::Object(body);

            let url = format!("{}?alt=sse", self.endpoint("streamGenerateContent"));
            let response = match self.post(url, &body).send().await {
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
                    error_event(self.friendly_error(status, &ApiError::parse(&text))),
                )
                .await;
                return Ok(());
            }

            let round = read_stream(response, tx).await?;
            // Geminiの使用量は「その時点までの累計」が毎チャンクに載るので、
            // 足し込まずに最後の値で置き換える(往復ごとの合計だけを足す)
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

            // モデルの発言(本文+関数呼び出し)をそのまま積み直し、結果を返して続ける
            contents.push(model_content(&round));
            let mut responses = Vec::new();
            for call in &round.tool_calls {
                let outcome = match &self.tools {
                    Some(bridge) => bridge.call(&call.name, call.args.clone()).await,
                    // ツール窓口が無いのにツールを要求された(設定の取り違え)
                    None => crate::tools::ToolOutcome::error(
                        "このバックエンドでは図面編集ツールを使えません".to_string(),
                    ),
                };
                send(
                    tx,
                    AgentEvent::ToolUseFinished {
                        id: call.event_id(),
                        tool: call.name.clone(),
                        is_error: outcome.is_error,
                    },
                )
                .await;
                responses.push(call.response_part(&outcome));
            }
            // 関数の結果はuserロールでまとめて返す(並行呼び出しも1つのcontentに入る)
            contents.push(json!({"role": "user", "parts": responses}));
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

    /// 接続そのものに失敗したときの説明(試したURLを必ず添える)。**キーは含めない。**
    fn network_error(&self, detail: &str) -> String {
        format!(
            "{}へ接続できませんでした({detail})。ネットワーク接続と、設定 > エージェント の\
             モデル名を確認してください。",
            self.base_url
        )
    }

    /// APIのエラーを画面へ出せる日本語にする。**APIキーは決して含めない。**
    fn friendly_error(&self, status: u16, error: &ApiError) -> String {
        let message = &error.message;
        match error_kind(status, error) {
            KIND_GEMINI_AUTH => "APIキーが受け付けられませんでした。設定 > エージェント で\
                 Gemini APIキーを入れ直してください(キーはOSのキーチェーンに保存されます)。\
                 キーはGoogle AI Studioで発行できます。"
                .to_string(),
            KIND_GEMINI_RATE_LIMIT => {
                "Gemini APIの利用上限(レート制限)に達しました。少し時間をおいてから\
                 お試しください(無料枠は1分あたりの回数が決まっています)。"
                    .to_string()
            }
            KIND_GEMINI_MODEL_NOT_FOUND => format!(
                "モデル「{}」が見つかりません。設定 > エージェント のモデル名を確認してください\
                 (例: gemini-2.5-flash)。({message})",
                model_id(&self.model)
            ),
            KIND_GEMINI_SERVER => format!("Gemini API側でエラーが起きました({message})"),
            KIND_GEMINI_REQUEST => {
                format!("Gemini APIがリクエストを受け付けませんでした: {message}")
            }
            _ => format!("Gemini APIからエラーが届きました (HTTP {status}): {message}"),
        }
    }
}

impl AgentBackend for GeminiBackend {
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

// ------------------------------------------------------------------ ツール定義

/// Geminiのスキーマが受け付ける項目(OpenAPIの部分集合)。これ以外は落とす。
const ALLOWED_SCHEMA_KEYS: &[&str] = &[
    "type",
    "format",
    "title",
    "description",
    "nullable",
    "enum",
    "maxItems",
    "minItems",
    "properties",
    "required",
    "minProperties",
    "maxProperties",
    "minLength",
    "maxLength",
    "pattern",
    "example",
    "anyOf",
    "propertyOrdering",
    "default",
    "items",
    "minimum",
    "maximum",
];

/// Geminiが受け付ける`format`の綴り。ほかの綴り(`uint32`等)は落とす。
const ALLOWED_FORMATS: &[&str] = &["date-time", "enum", "float", "double", "int32", "int64"];

/// MCPツール定義 → Geminiの`tools`配列(`functionDeclarations`ひとまとめ)。
///
/// 名前・説明の整え方はAnthropic版([`crate::tools::api_tool_definitions`])と同じものを
/// 使い、スキーマだけGeminiが読める形へ削る。引数を取らないツールは`parameters`ごと
/// 省く(空のオブジェクトを嫌うため)。
pub fn gemini_tool_definitions(defs: &[ToolDef]) -> Vec<Value> {
    let declarations: Vec<Value> = crate::tools::api_tool_definitions(defs)
        .into_iter()
        .map(|def| {
            let mut declaration = Map::new();
            declaration.insert("name".into(), def["name"].clone());
            declaration.insert("description".into(), def["description"].clone());
            let parameters = gemini_schema(&def["input_schema"]);
            if has_properties(&parameters) {
                declaration.insert("parameters".into(), parameters);
            }
            Value::Object(declaration)
        })
        .collect();
    if declarations.is_empty() {
        return Vec::new();
    }
    vec![json!({"functionDeclarations": declarations})]
}

fn has_properties(schema: &Value) -> bool {
    schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|properties| !properties.is_empty())
        .unwrap_or(false)
}

/// JSON Schema → Geminiが受け付けるスキーマ(読めない項目を落とす)。
///
/// 落とすのは`$schema`・`$ref`・`$defs`・`additionalProperties`・`allOf`/`oneOf`など
/// (送ると400になる)。削った結果として型が無くなったスキーマには`string`を補う
/// (型の無いスキーマもGeminiは受け付けないため)。
pub fn gemini_schema(schema: &Value) -> Value {
    let Value::Object(map) = schema else {
        return json!({"type": "string"});
    };
    let mut out = Map::new();
    for (key, value) in map {
        if !ALLOWED_SCHEMA_KEYS.contains(&key.as_str()) {
            continue;
        }
        let converted = match key.as_str() {
            "properties" => {
                let Some(properties) = value.as_object() else {
                    continue;
                };
                Value::Object(
                    properties
                        .iter()
                        .map(|(name, sub)| (name.clone(), gemini_schema(sub)))
                        .collect(),
                )
            }
            "items" => gemini_schema(value),
            "anyOf" => {
                let Some(list) = value.as_array() else {
                    continue;
                };
                Value::Array(list.iter().map(gemini_schema).collect())
            }
            // Geminiが知らない書式名は落とす(知らない綴りは400になる)
            "format" => match value.as_str().filter(|f| ALLOWED_FORMATS.contains(f)) {
                Some(format) => json!(format),
                None => continue,
            },
            // Geminiのenumは文字列の並びだけ
            "enum" => match value.as_array() {
                Some(list) if list.iter().all(Value::is_string) => Value::Array(list.clone()),
                _ => continue,
            },
            _ => value.clone(),
        };
        out.insert(key.clone(), converted);
    }
    if !out.contains_key("type") && !out.contains_key("anyOf") {
        out.insert("type".into(), json!("string"));
    }
    Value::Object(out)
}

// ------------------------------------------------------------------ 会話の組み立て

/// 会話履歴 → APIの`contents`配列(本文だけ。過去ターンのツール往復は送り直さない)。
fn history_contents(history: &[crate::backend::HistoryMessage]) -> Vec<Value> {
    history
        .iter()
        .filter(|m| !m.text.trim().is_empty())
        .map(|m| {
            // Geminiではアシスタントの発言は「model」ロール
            let role = match m.role {
                Role::User => "user",
                Role::Assistant => "model",
            };
            json!({"role": role, "parts": [{"text": m.text}]})
        })
        .collect()
}

/// 関数呼び出しを含むモデルの発言を積み直す形にする。
fn model_content(round: &StreamRound) -> Value {
    let mut parts = Vec::new();
    if !round.text.is_empty() {
        parts.push(json!({"text": round.text}));
    }
    for call in &round.tool_calls {
        parts.push(json!({"functionCall": call.call_part()}));
    }
    json!({"role": "model", "parts": parts})
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
    /// 本文(チャンクの連結)
    text: String,
    /// 実行すべき関数呼び出し(届いた順)
    tool_calls: Vec<ToolCall>,
    usage: Usage,
    /// ストリーム内で届いたエラー
    error: Option<String>,
}

/// モデルが要求した関数呼び出し1件。
#[derive(Debug, Clone)]
struct ToolCall {
    /// 呼び出しID。**モデルが付けてきたときだけ**入る(返すときもそのまま返す)
    id: Option<String>,
    name: String,
    /// 引数(Geminiは文字列ではなくJSONオブジェクトで送ってくる)
    args: Value,
    /// 同じターン内での通し番号(IDが無いモデル向けのUI表示用)
    index: usize,
}

impl ToolCall {
    /// UIイベントに載せるID(モデルがIDを付けない場合の代わりも用意する)。
    fn event_id(&self) -> String {
        self.id
            .clone()
            .unwrap_or_else(|| format!("call_{}", self.index))
    }

    /// 積み直す`functionCall`の中身。
    fn call_part(&self) -> Value {
        let mut part = Map::new();
        if let Some(id) = &self.id {
            part.insert("id".into(), json!(id));
        }
        part.insert("name".into(), json!(self.name));
        part.insert("args".into(), self.args.clone());
        Value::Object(part)
    }

    /// 実行結果を返す`functionResponse`のパート。
    ///
    /// `response`はオブジェクトでなければならないので、成功は`result`、失敗は`error`に
    /// 入れて「どちらなのか」がモデルに分かるようにする(Geminiの関数応答には
    /// 成否の欄が無いため)。
    fn response_part(&self, outcome: &crate::tools::ToolOutcome) -> Value {
        let mut response = Map::new();
        if outcome.is_error {
            response.insert("error".into(), json!(outcome.content));
        } else {
            response.insert("result".into(), json!(outcome.content));
        }
        let mut function_response = Map::new();
        // IDはモデルが付けてきたときだけ返す(APIが知らないIDを作らない)
        if let Some(id) = &self.id {
            function_response.insert("id".into(), json!(id));
        }
        function_response.insert("name".into(), json!(self.name));
        function_response.insert("response".into(), Value::Object(response));
        json!({"functionResponse": Value::Object(function_response)})
    }
}

/// SSEを読みながらイベントを流し、1発言分の結果を返す。
async fn read_stream(
    mut response: reqwest::Response,
    tx: &mpsc::Sender<AgentEvent>,
) -> Result<StreamRound> {
    let mut round = StreamRound::default();
    let mut buffer = String::new();

    loop {
        let chunk = match response.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(e) => {
                round.error = Some(format!("Gemini APIからの応答が途切れました: {e}"));
                break;
            }
        };
        buffer.push_str(&String::from_utf8_lossy(&chunk).replace("\r\n", "\n"));
        while let Some(split) = buffer.find("\n\n") {
            let frame: String = buffer.drain(..split + 2).collect();
            apply_frame(&frame, &mut round, tx).await;
        }
    }
    // 終端の空行が無いまま切れたストリームでも、最後のフレームを取りこぼさない
    if !buffer.trim().is_empty() {
        let frame = std::mem::take(&mut buffer);
        apply_frame(&frame, &mut round, tx).await;
    }
    Ok(round)
}

/// SSEフレーム1つ(`data: {...}`)を反映する。
async fn apply_frame(frame: &str, round: &mut StreamRound, tx: &mpsc::Sender<AgentEvent>) {
    let Some(data) = sse_data(frame) else {
        return;
    };
    let Ok(event) = serde_json::from_str::<Value>(&data) else {
        return;
    };
    apply_chunk(&event, round, tx).await;
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

/// チャンク1つ(GenerateContentResponse)を反映する。
async fn apply_chunk(event: &Value, round: &mut StreamRound, tx: &mpsc::Sender<AgentEvent>) {
    // エラーがSSEの本文として届くこともある
    if let Some(error) = event.get("error") {
        round.error = Some(
            error
                .get("message")
                .and_then(Value::as_str)
                .map(|m| format!("Gemini APIからエラーが届きました: {m}"))
                .unwrap_or_else(|| "Gemini APIからエラーが届きました".to_string()),
        );
        return;
    }
    // 入力そのものが安全フィルタで止められた場合
    if let Some(reason) = event
        .pointer("/promptFeedback/blockReason")
        .and_then(Value::as_str)
    {
        round.error = Some(format!(
            "Geminiが入力を受け付けませんでした(理由: {reason})。言い回しを変えて\
             お試しください。"
        ));
        return;
    }
    if let Some(usage) = event.get("usageMetadata").filter(|u| u.is_object()) {
        // 累計が毎回届くので「足す」ではなく「置き換える」
        round.usage.input_tokens = u64_at(usage, "promptTokenCount");
        round.usage.output_tokens = u64_at(usage, "candidatesTokenCount");
        round.usage.cache_read_input_tokens = u64_at(usage, "cachedContentTokenCount");
    }
    let Some(parts) = event
        .pointer("/candidates/0/content/parts")
        .and_then(Value::as_array)
    else {
        return;
    };
    for part in parts {
        if let Some(text) = part.get("text").and_then(Value::as_str) {
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
        let Some(call) = part.get("functionCall") else {
            continue;
        };
        let name = call
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if name.is_empty() {
            continue;
        }
        let call = ToolCall {
            id: call
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .map(str::to_string),
            name,
            args: call
                .get("args")
                .cloned()
                .unwrap_or(Value::Object(Map::new())),
            index: round.tool_calls.len(),
        };
        send(
            tx,
            AgentEvent::ToolUseStarted {
                id: call.event_id(),
                tool: call.name.clone(),
                input: call.args.clone(),
            },
        )
        .await;
        round.tool_calls.push(call);
    }
}

fn u64_at(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(Value::as_u64).unwrap_or(0)
}

// ------------------------------------------------------------------ エラー解釈

/// Gemini APIのエラーボディ(`{"error": {"code", "message", "status", "details"}}`)。
#[derive(Debug, Default)]
pub struct ApiError {
    /// `status`(`INVALID_ARGUMENT` / `RESOURCE_EXHAUSTED` …)
    pub status: String,
    /// `details[].reason`(`API_KEY_INVALID` …)を連結したもの
    pub reason: String,
    /// 表示できる本文
    pub message: String,
}

impl ApiError {
    pub fn parse(text: &str) -> Self {
        let value: Value = serde_json::from_str(text).unwrap_or(Value::Null);
        let status = value
            .pointer("/error/status")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let reason = value
            .pointer("/error/details")
            .and_then(Value::as_array)
            .map(|details| {
                details
                    .iter()
                    .filter_map(|d| d.get("reason").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();
        let message = value
            .pointer("/error/message")
            .and_then(Value::as_str)
            .map(|m| m.to_string())
            .unwrap_or_else(|| text.trim().chars().take(300).collect());
        Self {
            status,
            reason,
            message,
        }
    }
}

/// HTTPステータスとAPIのエラー種別から区分を決める(UIの翻訳キー)。
///
/// Geminiは**キーが無効でも400**(`INVALID_ARGUMENT` + `API_KEY_INVALID`)を返すため、
/// ステータスコードだけでは「リクエストの誤り」と見分けられない。理由の欄を先に見る。
pub fn error_kind(status: u16, error: &ApiError) -> &'static str {
    let key_problem = error.reason.contains("API_KEY")
        || error.message.contains("API key")
        || error.message.contains("API_KEY");
    if status == 401
        || status == 403
        || error.status == "UNAUTHENTICATED"
        || error.status == "PERMISSION_DENIED"
        || key_problem
    {
        KIND_GEMINI_AUTH
    } else if status == 429 || error.status == "RESOURCE_EXHAUSTED" {
        KIND_GEMINI_RATE_LIMIT
    } else if status == 404 || error.status == "NOT_FOUND" {
        KIND_GEMINI_MODEL_NOT_FOUND
    } else if (500..600).contains(&status)
        || error.status == "INTERNAL"
        || error.status == "UNAVAILABLE"
    {
        KIND_GEMINI_SERVER
    } else if status == 400 || status == 422 || error.status == "INVALID_ARGUMENT" {
        KIND_GEMINI_REQUEST
    } else {
        KIND_GEMINI_UNKNOWN
    }
}
