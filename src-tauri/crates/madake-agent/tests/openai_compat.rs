//! OpenAI互換Chat Completions APIバックエンド([`OpenAiCompatBackend`])のテスト。
//!
//! 本物のOpenAI・Ollamaは呼ばない。テスト内でHTTPサーバーを立て、`base_url`をそこへ
//! 向けてSSE(ストリーミング)応答を台本どおりに返す。確かめるのは4点:
//! 「APIへ何をどこへ送るか」「応答をどのUIイベントへ変換するか」「ツール実行ループが
//! 回るか」「失敗を人が読める言葉にできるか」。

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use madake_agent::backend::{AgentBackend, TurnRequest};
use madake_agent::openai_compat::{OpenAiCompatBackend, DEFAULT_OPENAI_BASE_URL, OLLAMA_BASE_URL};
use madake_agent::tools::{ToolBridge, ToolDef, ToolFuture, ToolOutcome};
use madake_agent::AgentEvent;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

// ---------------------------------------------------------------- モックAPI

/// 1リクエストに返す台本。
enum Scripted {
    /// 200 + SSEのチャンク列(`data: {...}`。末尾に`[DONE]`を足して返す)
    Sse(Vec<Value>),
    /// エラー応答(HTTPステータス + JSONボディ)
    Status(u16, Value),
}

/// 届いたリクエスト1件(パス・ヘッダ・ボディ)。
#[derive(Debug, Clone)]
struct Recorded {
    path: String,
    /// ヘッダ名は小文字化して持つ
    headers: Vec<(String, String)>,
    body: Value,
}

impl Recorded {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

/// テスト用のOpenAI互換サーバーもどき。台本を先頭から1リクエストずつ消費する。
struct MockApi {
    addr: SocketAddr,
    requests: Arc<Mutex<Vec<Recorded>>>,
}

impl MockApi {
    async fn start(script: Vec<Scripted>) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&requests);
        tokio::spawn(async move {
            let mut script = script.into_iter();
            while let Ok((mut socket, _)) = listener.accept().await {
                let Some(request) = read_request(&mut socket).await else {
                    continue;
                };
                recorded.lock().unwrap().push(request);
                let response = match script.next() {
                    Some(Scripted::Sse(chunks)) => sse_response(&chunks),
                    Some(Scripted::Status(code, body)) => status_response(code, &body),
                    None => status_response(
                        500,
                        &json!({"error": {"message": "台本切れ", "type": "test"}}),
                    ),
                };
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.flush().await;
                let _ = socket.shutdown().await;
            }
        });
        Self { addr, requests }
    }

    fn base_url(&self) -> String {
        format!("http://{}/v1", self.addr)
    }

    fn request(&self, index: usize) -> Recorded {
        self.requests
            .lock()
            .unwrap()
            .get(index)
            .cloned()
            .unwrap_or_else(|| panic!("{index}番目のリクエストが届いていない"))
    }

    fn request_count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> Option<Recorded> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        if let Some(head_end) = find_head_end(&buf) {
            let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
            let mut lines = head.lines();
            let path = lines
                .next()
                .and_then(|start| start.split_whitespace().nth(1))
                .unwrap_or_default()
                .to_string();
            let headers: Vec<(String, String)> = lines
                .filter_map(|line| line.split_once(':'))
                .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
                .collect();
            let len = headers
                .iter()
                .find(|(k, _)| k == "content-length")
                .and_then(|(_, v)| v.parse::<usize>().ok())
                .unwrap_or(0);
            if buf.len() >= head_end + 4 + len {
                let body = String::from_utf8_lossy(&buf[head_end + 4..head_end + 4 + len]);
                return Some(Recorded {
                    path,
                    headers,
                    body: serde_json::from_str(&body).unwrap_or(Value::Null),
                });
            }
        }
        let n = socket.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
}

fn find_head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn sse_response(chunks: &[Value]) -> String {
    let mut body = String::new();
    for chunk in chunks {
        body.push_str(&format!("data: {chunk}\n\n"));
    }
    body.push_str("data: [DONE]\n\n");
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn status_response(code: u16, body: &Value) -> String {
    let body = body.to_string();
    format!(
        "HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

// ------------------------------------------------------------ 台本の部品

/// 本文の差分チャンク。
fn text_chunk(text: &str) -> Value {
    json!({"choices": [{"index": 0, "delta": {"content": text}, "finish_reason": null}]})
}

/// ツール呼び出しの開始チャンク(id・名前つき)。
fn tool_head(index: u64, id: &str, name: &str) -> Value {
    json!({"choices": [{"index": 0, "delta": {"tool_calls": [
        {"index": index, "id": id, "type": "function", "function": {"name": name, "arguments": ""}}
    ]}, "finish_reason": null}]})
}

/// ツール呼び出しの引数チャンク(JSONは細切れで届く)。
fn tool_args(index: u64, partial: &str) -> Value {
    json!({"choices": [{"index": 0, "delta": {"tool_calls": [
        {"index": index, "function": {"arguments": partial}}
    ]}, "finish_reason": null}]})
}

fn finish(reason: &str) -> Value {
    json!({"choices": [{"index": 0, "delta": {}, "finish_reason": reason}]})
}

fn usage_chunk(prompt: u64, completion: u64) -> Value {
    json!({"choices": [], "usage": {"prompt_tokens": prompt, "completion_tokens": completion, "total_tokens": prompt + completion}})
}

// ------------------------------------------------------------ 偽ツールブリッジ

/// 呼ばれたツールを記録し、決め打ちの結果を返すブリッジ。
struct FakeTools {
    defs: Vec<ToolDef>,
    calls: Arc<Mutex<Vec<(String, Value)>>>,
    result: String,
    is_error: bool,
}

impl FakeTools {
    fn new(result: &str) -> Self {
        Self {
            defs: vec![ToolDef {
                name: "place_symbol".into(),
                description: "シンボルをシートに配置する".into(),
                input_schema: json!({"type": "object", "properties": {"symbol_id": {"type": "string"}}}),
            }],
            calls: Arc::new(Mutex::new(Vec::new())),
            result: result.to_string(),
            is_error: false,
        }
    }
}

impl ToolBridge for FakeTools {
    fn tools(&self) -> Vec<ToolDef> {
        self.defs.clone()
    }

    fn call(&self, name: &str, input: Value) -> ToolFuture<'_> {
        self.calls
            .lock()
            .unwrap()
            .push((name.to_string(), input.clone()));
        let outcome = ToolOutcome {
            content: self.result.clone(),
            is_error: self.is_error,
        };
        Box::pin(async move { outcome })
    }
}

// ------------------------------------------------------------ 実行ヘルパ

async fn run(backend: &OpenAiCompatBackend, prompt: &str, system: Option<&str>) -> Vec<AgentEvent> {
    let (tx, mut rx) = mpsc::channel(64);
    let request = TurnRequest {
        prompt,
        session: None,
        system_prompt: system,
        history: &[],
    };
    let _ = backend.run_turn(request, tx).await;
    let mut events = Vec::new();
    while let Some(event) = rx.recv().await {
        events.push(event);
    }
    events
}

fn texts(events: &[AgentEvent]) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::TextDelta { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn error_message(events: &[AgentEvent]) -> String {
    events
        .iter()
        .find_map(|e| match e {
            AgentEvent::Error { message } => Some(message.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("エラーイベントが出ていない: {events:?}"))
}

/// キー付きのバックエンド(接続先はモック)。
fn backend_with_key(api: &MockApi) -> OpenAiCompatBackend {
    OpenAiCompatBackend::new(Some("sk-test".to_string()), "gpt-4o", api.base_url())
}

// ------------------------------------------------------------------- テスト

/// Streamed assistant text arrives as text deltas and the turn ends with the assembled reply and its token usage.
/// OpenAI互換サーバーから届いた本文はそのまま文字の差分として流れ、ターンの終わりに組み立てた返答とトークン使用量が付く。
#[tokio::test]
async fn streamed_text_becomes_deltas_and_a_completed_turn() {
    let api = MockApi::start(vec![Scripted::Sse(vec![
        text_chunk("こん"),
        text_chunk("にちは"),
        finish("stop"),
        usage_chunk(120, 42),
    ])])
    .await;
    let backend = backend_with_key(&api);

    let events = run(&backend, "こんにちは", None).await;

    assert_eq!(texts(&events), "こんにちは");
    let completed = events
        .iter()
        .find_map(|e| match e {
            AgentEvent::TurnCompleted { result, usage } => Some((result.clone(), usage.clone())),
            _ => None,
        })
        .expect("ターン完了イベントが無い");
    assert_eq!(completed.0, "こんにちは");
    let usage = completed.1.expect("使用量が無い");
    assert_eq!(usage.input_tokens, 120);
    assert_eq!(usage.output_tokens, 42);
}

/// The request goes to /chat/completions under the configured base URL and carries the model, the streaming flag, the system prompt and the user's message.
/// リクエストは設定したベースURL配下の /chat/completions へ行き、モデル・ストリーミング指定・システムプロンプト・ユーザーの発言を載せる。
#[tokio::test]
async fn the_request_goes_to_chat_completions_with_the_model_system_prompt_and_message() {
    let api = MockApi::start(vec![Scripted::Sse(vec![text_chunk("ok"), finish("stop")])]).await;
    let backend = backend_with_key(&api);

    run(&backend, "R1を置いて", Some("あなたは電気図面のCADです")).await;

    let request = api.request(0);
    assert_eq!(request.path, "/v1/chat/completions", "送り先が違う");
    let body = &request.body;
    assert_eq!(body["model"], "gpt-4o");
    assert_eq!(body["stream"], true);
    // システム指示は最初のメッセージ(OpenAI互換APIにsystem引数は無い)
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][0]["content"], "あなたは電気図面のCADです");
    assert_eq!(body["messages"][1]["role"], "user");
    assert_eq!(body["messages"][1]["content"], "R1を置いて");
}

/// The bridged MCP tools are offered in the OpenAI function format so the agent can edit the drawing.
/// ブリッジしたMCPツールはOpenAIのfunction形式で渡る(エージェントが図面を編集できる)。
#[tokio::test]
async fn the_bridged_mcp_tools_are_offered_as_openai_functions() {
    let api = MockApi::start(vec![Scripted::Sse(vec![text_chunk("ok"), finish("stop")])]).await;
    let backend = backend_with_key(&api).with_tools(Arc::new(FakeTools::new("{}")));

    run(&backend, "置いて", None).await;

    let body = api.request(0).body;
    assert_eq!(body["tools"][0]["type"], "function");
    assert_eq!(body["tools"][0]["function"]["name"], "place_symbol");
    assert_eq!(body["tools"][0]["function"]["parameters"]["type"], "object");
}

/// A tool the model asks for is executed locally and its result is sent back as a tool message so the model can continue.
/// モデルが要求したツールはその場で実行され、結果をツールメッセージとして返して会話を続ける(1ターンで完結する)。
#[tokio::test]
async fn a_requested_tool_is_executed_and_its_result_is_sent_back() {
    let tools = Arc::new(FakeTools::new("{\"id\":\"e1\"}"));
    let calls = Arc::clone(&tools.calls);
    let api = MockApi::start(vec![
        Scripted::Sse(vec![
            text_chunk("配置します"),
            tool_head(0, "call_1", "place_symbol"),
            // 引数のJSONは細切れで届く(つなぎ直せること)
            tool_args(0, "{\"symbol_id\":"),
            tool_args(0, "\"relay\"}"),
            finish("tool_calls"),
        ]),
        Scripted::Sse(vec![text_chunk("置きました"), finish("stop")]),
    ])
    .await;
    let backend = backend_with_key(&api).with_tools(tools);

    let events = run(&backend, "リレーを置いて", None).await;

    // ツールは実際に呼ばれた
    let calls = calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 1, "ツール呼び出し回数が違う: {calls:?}");
    assert_eq!(calls[0].0, "place_symbol");
    assert_eq!(calls[0].1["symbol_id"], "relay");

    // UIには「開始」と「完了」が両方流れる
    assert!(
        events.iter().any(
            |e| matches!(e, AgentEvent::ToolUseStarted { id, tool, input }
            if id == "call_1" && tool == "place_symbol" && input["symbol_id"] == "relay")
        ),
        "ツール開始イベントが無い: {events:?}"
    );
    assert!(
        events.iter().any(
            |e| matches!(e, AgentEvent::ToolUseFinished { id, tool, is_error }
            if id == "call_1" && tool == "place_symbol" && !*is_error)
        ),
        "ツール完了イベントが無い: {events:?}"
    );

    // 2回目のリクエストにはツール要求と実行結果が積まれている
    assert_eq!(api.request_count(), 2, "リクエスト回数が違う");
    let second = api.request(1).body;
    let messages = second["messages"].as_array().expect("messagesが配列でない");
    let assistant = messages
        .iter()
        .find(|m| m["role"] == "assistant")
        .expect("アシスタントの発言が積まれていない");
    assert_eq!(assistant["tool_calls"][0]["id"], "call_1");
    assert_eq!(assistant["tool_calls"][0]["type"], "function");
    assert_eq!(
        assistant["tool_calls"][0]["function"]["name"],
        "place_symbol"
    );
    // 引数は文字列のJSON(OpenAIの形)
    assert_eq!(
        assistant["tool_calls"][0]["function"]["arguments"],
        "{\"symbol_id\":\"relay\"}"
    );
    let last = messages.last().expect("メッセージが空");
    assert_eq!(last["role"], "tool");
    assert_eq!(last["tool_call_id"], "call_1");
    assert_eq!(last["content"], "{\"id\":\"e1\"}");

    // 最終応答は2回目の本文
    assert!(
        events.iter().any(
            |e| matches!(e, AgentEvent::TurnCompleted { result, .. } if result == "置きました")
        ),
        "最終応答が2回目の本文になっていない: {events:?}"
    );
}

/// A tool that fails is reported back to the model as a clearly marked error instead of aborting the turn.
/// 失敗したツールはターンを中断せず、エラーと分かる印をつけてモデルへ返す(モデルが直せる)。
#[tokio::test]
async fn a_failing_tool_is_reported_to_the_model_as_an_error_result() {
    let mut tools = FakeTools::new("シートが見つかりません");
    tools.is_error = true;
    let api = MockApi::start(vec![
        Scripted::Sse(vec![
            tool_head(0, "call_9", "place_symbol"),
            tool_args(0, "{}"),
            finish("tool_calls"),
        ]),
        Scripted::Sse(vec![text_chunk("直します"), finish("stop")]),
    ])
    .await;
    let backend = backend_with_key(&api).with_tools(Arc::new(tools));

    let events = run(&backend, "置いて", None).await;

    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::ToolUseFinished { is_error, .. } if *is_error)),
        "ツール失敗が伝わっていない: {events:?}"
    );
    // OpenAIのツールメッセージには成否の欄が無いため、本文に印をつけて失敗と分かるようにする
    let last = api.request(1).body;
    let message = last["messages"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(message["role"], "tool");
    let content = message["content"].as_str().unwrap_or_default();
    assert!(content.contains("ERROR"), "失敗の印が無い: {content}");
    assert!(
        content.contains("シートが見つかりません"),
        "失敗の理由が届いていない: {content}"
    );
}

/// The tool loop stops after a bounded number of rounds so a looping model cannot run forever.
/// ツール実行の往復には上限があり、堂々巡りになったモデルが延々と動き続けない。
#[tokio::test]
async fn the_tool_loop_stops_after_a_bounded_number_of_rounds() {
    let script: Vec<Scripted> = (0..10)
        .map(|_| {
            Scripted::Sse(vec![
                tool_head(0, "call_x", "place_symbol"),
                tool_args(0, "{}"),
                finish("tool_calls"),
            ])
        })
        .collect();
    let api = MockApi::start(script).await;
    let mut backend = backend_with_key(&api).with_tools(Arc::new(FakeTools::new("{}")));
    backend.max_tool_rounds = 3;

    let events = run(&backend, "ずっと置いて", None).await;

    assert_eq!(api.request_count(), 3, "上限を超えてAPIを呼んでいる");
    assert!(
        error_message(&events).contains("上限"),
        "上限に達したことがユーザーへ伝わらない: {events:?}"
    );
}

/// A saved key is sent as a bearer token, and never appears in any message shown to the user.
/// 保存したキーはBearerトークンとして送られ、ユーザーに見える文言には一切現れない。
#[tokio::test]
async fn a_saved_key_is_sent_as_a_bearer_token() {
    let api = MockApi::start(vec![Scripted::Sse(vec![text_chunk("ok"), finish("stop")])]).await;
    let backend = OpenAiCompatBackend::new(Some("sk-secret-0001".into()), "gpt-4o", api.base_url());

    let events = run(&backend, "こんにちは", None).await;

    assert_eq!(
        api.request(0).header("authorization"),
        Some("Bearer sk-secret-0001")
    );
    let serialized = serde_json::to_string(&events).unwrap();
    assert!(
        !serialized.contains("sk-secret-0001"),
        "イベントにキーが混ざっている: {serialized}"
    );
}

/// A local server such as Ollama is called without any Authorization header when no key is saved.
/// Ollamaのようなローカルサーバーは、キーを保存していなくてもAuthorizationヘッダ無しでそのまま呼べる。
#[tokio::test]
async fn a_local_server_is_called_without_an_authorization_header() {
    let api = MockApi::start(vec![Scripted::Sse(vec![text_chunk("ok"), finish("stop")])]).await;
    let backend = OpenAiCompatBackend::new(None, "llama3.2", api.base_url());

    let events = run(&backend, "こんにちは", None).await;

    assert_eq!(
        api.request(0).header("authorization"),
        None,
        "キー無しなのにAuthorizationヘッダを送っている"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::TurnCompleted { .. })),
        "キー無しでもターンが完了すること: {events:?}"
    );
}

/// A rejected API key produces a message that says the key is the problem, not a raw HTTP code.
/// APIキーが弾かれたときは、生のHTTPコードではなく「キーの問題」と分かる文言を表示する。
#[tokio::test]
async fn a_rejected_api_key_is_explained_as_a_key_problem() {
    let api = MockApi::start(vec![Scripted::Status(
        401,
        json!({"error": {"message": "Incorrect API key provided: sk-bad", "type": "invalid_request_error", "code": "invalid_api_key"}}),
    )])
    .await;
    let backend = OpenAiCompatBackend::new(Some("sk-bad".into()), "gpt-4o", api.base_url());

    let message = error_message(&run(&backend, "こんにちは", None).await);

    assert!(
        message.contains("APIキー"),
        "キーの問題と分からない: {message}"
    );
    assert!(
        !message.contains("sk-bad"),
        "エラー文にAPIキーが混ざっている: {message}"
    );
}

/// Hitting the provider's rate limit is explained as a usage limit with a "try again later" hint.
/// レート制限に当たったときは「利用上限に達したので時間をおいて」と読める文言を表示する。
#[tokio::test]
async fn a_rate_limited_service_is_explained_as_a_usage_limit() {
    let api = MockApi::start(vec![Scripted::Status(
        429,
        json!({"error": {"message": "Rate limit reached", "type": "rate_limit_error"}}),
    )])
    .await;
    let backend = backend_with_key(&api);

    let message = error_message(&run(&backend, "こんにちは", None).await);
    assert!(message.contains("上限"), "利用上限と分からない: {message}");
}

/// An unknown model name is explained as a model-name problem, naming the model that was tried.
/// モデル名が見つからないときは「モデル名の問題」と分かる文言で、試したモデル名を添えて表示する。
#[tokio::test]
async fn an_unknown_model_name_is_explained_as_a_model_problem() {
    let api = MockApi::start(vec![Scripted::Status(
        404,
        json!({"error": {"message": "model \"llama-nope\" not found", "type": "invalid_request_error"}}),
    )])
    .await;
    let backend = OpenAiCompatBackend::new(None, "llama-nope", api.base_url());

    let message = error_message(&run(&backend, "こんにちは", None).await);
    assert!(
        message.contains("モデル"),
        "モデルの問題と分からない: {message}"
    );
    assert!(
        message.contains("llama-nope"),
        "どのモデル名で失敗したか分からない: {message}"
    );
}

/// A server that cannot be reached names the URL that was tried and points at starting Ollama for a local URL.
/// サーバーへ接続できないときは試したURLを示し、ローカルURLならOllamaの起動を促す。
#[tokio::test]
async fn an_unreachable_server_names_the_url_and_suggests_starting_ollama() {
    // 誰も待ち受けていないポートへ向ける(接続そのものが失敗する)
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let base = format!("http://127.0.0.1:{}/v1", addr.port());
    let backend = OpenAiCompatBackend::new(None, "llama3.2", base.clone());

    let message = error_message(&run(&backend, "こんにちは", None).await);

    assert!(message.contains(&base), "試したURLが分からない: {message}");
    assert!(
        message.contains("Ollama"),
        "ローカルURLなのにOllamaの起動案内が無い: {message}"
    );
}

/// Earlier turns of the conversation are replayed so the model remembers what was said before.
/// 会話の前のやりとりも一緒に送るので、モデルは前に話した内容を覚えている。
#[tokio::test]
async fn earlier_turns_of_the_conversation_are_replayed() {
    use madake_agent::backend::HistoryMessage;
    use madake_agent::Role;

    let api = MockApi::start(vec![Scripted::Sse(vec![
        text_chunk("R1です"),
        finish("stop"),
    ])])
    .await;
    let backend = backend_with_key(&api);

    let history = vec![
        HistoryMessage {
            role: Role::User,
            text: "リレーを置いて".into(),
        },
        HistoryMessage {
            role: Role::Assistant,
            text: "R1を置きました".into(),
        },
    ];
    let (tx, mut rx) = mpsc::channel(64);
    let _ = backend
        .run_turn(
            TurnRequest {
                prompt: "さっき置いたのは?",
                session: None,
                system_prompt: None,
                history: &history,
            },
            tx,
        )
        .await;
    while rx.recv().await.is_some() {}

    let body = api.request(0).body;
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 3, "履歴が送られていない: {messages:?}");
    assert_eq!(messages[0]["content"], "リレーを置いて");
    assert_eq!(messages[1]["role"], "assistant");
    assert_eq!(messages[1]["content"], "R1を置きました");
    assert_eq!(messages[2]["content"], "さっき置いたのは?");
}

/// The connection test reports success for a working setup and a readable, machine-tagged reason for a broken one.
/// 接続テストは、使える設定なら成功を、駄目な設定なら区分つきの読める理由を返す。
#[tokio::test]
async fn the_connection_test_reports_success_or_a_readable_reason() {
    let api = MockApi::start(vec![
        Scripted::Status(
            200,
            json!({"id": "chatcmpl-1", "object": "chat.completion", "choices": [{"index": 0, "message": {"role": "assistant", "content": "pong"}, "finish_reason": "stop"}]}),
        ),
        Scripted::Status(
            401,
            json!({"error": {"message": "Incorrect API key provided", "type": "invalid_request_error"}}),
        ),
    ])
    .await;
    let backend = backend_with_key(&api);

    assert!(backend.check_connection().await.is_ok(), "疎通に失敗した");
    let error = backend.check_connection().await.unwrap_err();
    // 区分はUIが翻訳するため、文言だけでなく機械可読な種類も返す
    assert_eq!(error.kind, "openai_auth", "エラーの種類が違う: {error:?}");
    assert!(
        error.message.contains("APIキー"),
        "理由が読めない: {error:?}"
    );
}

/// The Ollama preset points at the local Ollama server's OpenAI-compatible endpoint, while the default is OpenAI itself.
/// Ollamaプリセットはローカルのollamaが持つOpenAI互換エンドポイントを指し、既定の接続先はOpenAI本体である。
#[test]
fn the_ollama_preset_points_at_the_local_ollama_server() {
    assert_eq!(OLLAMA_BASE_URL, "http://localhost:11434/v1");
    assert_eq!(DEFAULT_OPENAI_BASE_URL, "https://api.openai.com/v1");
}

/// A base URL pasted with a trailing slash still reaches /chat/completions exactly once.
/// 末尾に「/」を付けて貼り付けたベースURLでも、/chat/completions へ正しく1回だけつながる。
#[tokio::test]
async fn a_base_url_with_a_trailing_slash_still_works() {
    let api = MockApi::start(vec![Scripted::Sse(vec![text_chunk("ok"), finish("stop")])]).await;
    let backend = OpenAiCompatBackend::new(
        Some("sk-test".into()),
        "gpt-4o",
        format!("{}/", api.base_url()),
    );

    run(&backend, "こんにちは", None).await;

    assert_eq!(api.request(0).path, "/v1/chat/completions");
}
