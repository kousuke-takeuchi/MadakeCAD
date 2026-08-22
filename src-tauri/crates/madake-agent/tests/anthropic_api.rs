//! Anthropic Messages API直結バックエンド([`AnthropicApiBackend`])のテスト。
//!
//! 本物のAPIは呼ばない。テスト内でHTTPサーバーを立て、`base_url`をそこへ向けて
//! SSE(ストリーミング)応答を台本どおりに返す。確かめるのは3点:
//! 「APIへ何を送るか」「APIの応答をどのUIイベントへ変換するか」「ツール実行ループが回るか」。

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use madake_agent::backend::{AgentBackend, TurnRequest};
use madake_agent::tools::{ToolBridge, ToolDef, ToolFuture, ToolOutcome};
use madake_agent::{AgentEvent, AnthropicApiBackend};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

// ---------------------------------------------------------------- モックAPI

/// 1リクエストに返す台本。
enum Scripted {
    /// 200 + SSEイベント列(`(event名, data JSON)`)
    Sse(Vec<(&'static str, Value)>),
    /// エラー応答(HTTPステータス + JSONボディ)
    Status(u16, Value),
}

/// テスト用のAnthropic APIもどき。台本を先頭から1リクエストずつ消費する。
struct MockApi {
    addr: SocketAddr,
    requests: Arc<Mutex<Vec<Value>>>,
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
                let Some(body) = read_request_body(&mut socket).await else {
                    continue;
                };
                recorded
                    .lock()
                    .unwrap()
                    .push(serde_json::from_str(&body).unwrap_or(Value::Null));
                let response = match script.next() {
                    Some(Scripted::Sse(events)) => sse_response(&events),
                    Some(Scripted::Status(code, body)) => status_response(code, &body),
                    None => status_response(
                        500,
                        &json!({"type": "error", "error": {"type": "test", "message": "台本切れ"}}),
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
        format!("http://{}", self.addr)
    }

    fn request(&self, index: usize) -> Value {
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

async fn read_request_body(socket: &mut tokio::net::TcpStream) -> Option<String> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let head_end = find_head_end(&buf);
        if let Some(head_end) = head_end {
            let head = String::from_utf8_lossy(&buf[..head_end]).to_ascii_lowercase();
            let len = head
                .lines()
                .find_map(|line| line.strip_prefix("content-length:"))
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(0);
            if buf.len() >= head_end + 4 + len {
                return Some(String::from_utf8_lossy(&buf[head_end + 4..head_end + 4 + len]).into());
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

fn sse_response(events: &[(&str, Value)]) -> String {
    let mut body = String::new();
    for (name, data) in events {
        body.push_str(&format!("event: {name}\ndata: {data}\n\n"));
    }
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

fn message_start() -> (&'static str, Value) {
    (
        "message_start",
        json!({"type": "message_start", "message": {"id": "msg_1", "type": "message", "role": "assistant", "content": [], "model": "claude-sonnet-5", "usage": {"input_tokens": 120, "output_tokens": 1}}}),
    )
}

fn text_block(index: u64, chunks: &[&str]) -> Vec<(&'static str, Value)> {
    let mut out = vec![(
        "content_block_start",
        json!({"type": "content_block_start", "index": index, "content_block": {"type": "text", "text": ""}}),
    )];
    for chunk in chunks {
        out.push((
            "content_block_delta",
            json!({"type": "content_block_delta", "index": index, "delta": {"type": "text_delta", "text": chunk}}),
        ));
    }
    out.push((
        "content_block_stop",
        json!({"type": "content_block_stop", "index": index}),
    ));
    out
}

fn tool_block(index: u64, id: &str, name: &str, input_json: &str) -> Vec<(&'static str, Value)> {
    vec![
        (
            "content_block_start",
            json!({"type": "content_block_start", "index": index, "content_block": {"type": "tool_use", "id": id, "name": name, "input": {}}}),
        ),
        (
            "content_block_delta",
            json!({"type": "content_block_delta", "index": index, "delta": {"type": "input_json_delta", "partial_json": input_json}}),
        ),
        (
            "content_block_stop",
            json!({"type": "content_block_stop", "index": index}),
        ),
    ]
}

fn message_end(stop_reason: &str, output_tokens: u64) -> Vec<(&'static str, Value)> {
    vec![
        (
            "message_delta",
            json!({"type": "message_delta", "delta": {"stop_reason": stop_reason}, "usage": {"output_tokens": output_tokens}}),
        ),
        ("message_stop", json!({"type": "message_stop"})),
    ]
}

fn turn(blocks: Vec<Vec<(&'static str, Value)>>, stop_reason: &str) -> Scripted {
    let mut events = vec![message_start()];
    for block in blocks {
        events.extend(block);
    }
    events.extend(message_end(stop_reason, 42));
    Scripted::Sse(events)
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

async fn run(backend: &AnthropicApiBackend, prompt: &str, system: Option<&str>) -> Vec<AgentEvent> {
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

// ------------------------------------------------------------------- テスト

/// Streamed assistant text arrives as text deltas and the turn ends with the assembled reply and its token usage.
/// APIから届いた本文はそのまま文字の差分として流れ、ターンの終わりに組み立てた返答とトークン使用量が付く。
#[tokio::test]
async fn streamed_text_becomes_deltas_and_a_completed_turn() {
    let api = MockApi::start(vec![turn(
        vec![text_block(0, &["こん", "にちは"])],
        "end_turn",
    )])
    .await;
    let backend =
        AnthropicApiBackend::new("sk-test", "claude-sonnet-5").with_base_url(api.base_url());

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

/// The request carries the configured model, a streaming flag, the system prompt and the user's message.
/// APIへ送るリクエストには設定したモデル・ストリーミング指定・システムプロンプト・ユーザーの発言が載る。
#[tokio::test]
async fn the_request_carries_the_model_system_prompt_and_user_message() {
    let api = MockApi::start(vec![turn(vec![text_block(0, &["ok"])], "end_turn")]).await;
    let backend =
        AnthropicApiBackend::new("sk-test", "claude-sonnet-5").with_base_url(api.base_url());

    run(&backend, "R1を置いて", Some("あなたは電気図面のCADです")).await;

    let body = api.request(0);
    assert_eq!(body["model"], "claude-sonnet-5");
    assert_eq!(body["stream"], true);
    assert!(
        body["max_tokens"].as_u64().unwrap_or(0) >= 4096,
        "max_tokensが小さすぎる: {}",
        body["max_tokens"]
    );
    assert_eq!(body["system"], "あなたは電気図面のCADです");
    assert_eq!(body["messages"][0]["role"], "user");
    assert_eq!(body["messages"][0]["content"], "R1を置いて");
}

/// The bridged MCP tools are offered to the API on every request, so the agent can edit the drawing.
/// ブリッジしたMCPツールは毎回のリクエストでAPIへ渡る(エージェントが図面を編集できる)。
#[tokio::test]
async fn the_bridged_mcp_tools_are_offered_to_the_api() {
    let api = MockApi::start(vec![turn(vec![text_block(0, &["ok"])], "end_turn")]).await;
    let backend = AnthropicApiBackend::new("sk-test", "claude-sonnet-5")
        .with_base_url(api.base_url())
        .with_tools(Arc::new(FakeTools::new("{}")));

    run(&backend, "置いて", None).await;

    let body = api.request(0);
    assert_eq!(body["tools"][0]["name"], "place_symbol");
    assert_eq!(body["tools"][0]["input_schema"]["type"], "object");
}

/// A tool the model asks for is executed locally and its result is sent back so the model can continue.
/// モデルが要求したツールはその場で実行され、結果を返して会話を続ける(1ターンで完結する)。
#[tokio::test]
async fn a_requested_tool_is_executed_and_its_result_is_sent_back() {
    let tools = Arc::new(FakeTools::new("{\"id\":\"e1\"}"));
    let calls = Arc::clone(&tools.calls);
    let api = MockApi::start(vec![
        turn(
            vec![
                text_block(0, &["配置します"]),
                tool_block(1, "toolu_1", "place_symbol", "{\"symbol_id\":\"relay\"}"),
            ],
            "tool_use",
        ),
        turn(vec![text_block(0, &["置きました"])], "end_turn"),
    ])
    .await;
    let backend = AnthropicApiBackend::new("sk-test", "claude-sonnet-5")
        .with_base_url(api.base_url())
        .with_tools(tools);

    let events = run(&backend, "リレーを置いて", None).await;

    // ツールは実際に呼ばれた
    let calls = calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 1, "ツール呼び出し回数が違う: {calls:?}");
    assert_eq!(calls[0].0, "place_symbol");
    assert_eq!(calls[0].1["symbol_id"], "relay");

    // UIには「開始」と「完了」が両方流れる
    assert!(
        events.iter().any(|e| matches!(e, AgentEvent::ToolUseStarted { id, tool, input }
            if id == "toolu_1" && tool == "place_symbol" && input["symbol_id"] == "relay")),
        "ツール開始イベントが無い: {events:?}"
    );
    assert!(
        events.iter().any(|e| matches!(e, AgentEvent::ToolUseFinished { id, tool, is_error }
            if id == "toolu_1" && tool == "place_symbol" && !*is_error)),
        "ツール完了イベントが無い: {events:?}"
    );

    // 2回目のリクエストにはツール要求と実行結果が積まれている
    assert_eq!(api.request_count(), 2, "リクエスト回数が違う");
    let second = api.request(1);
    let messages = second["messages"].as_array().expect("messagesが配列でない");
    let assistant = messages
        .iter()
        .find(|m| m["role"] == "assistant")
        .expect("アシスタントの発言が積まれていない");
    assert!(
        assistant["content"]
            .as_array()
            .unwrap()
            .iter()
            .any(|b| b["type"] == "tool_use" && b["id"] == "toolu_1"),
        "ツール要求が積まれていない: {assistant}"
    );
    let last = messages.last().expect("メッセージが空");
    assert_eq!(last["role"], "user");
    assert_eq!(last["content"][0]["type"], "tool_result");
    assert_eq!(last["content"][0]["tool_use_id"], "toolu_1");
    assert_eq!(last["content"][0]["content"], "{\"id\":\"e1\"}");

    // 最終応答は2回目の本文
    assert!(
        events.iter().any(
            |e| matches!(e, AgentEvent::TurnCompleted { result, .. } if result == "置きました")
        ),
        "最終応答が2回目の本文になっていない: {events:?}"
    );
}

/// A tool that fails is reported back to the model as an error result instead of aborting the turn.
/// 失敗したツールはターンを中断せず、エラーとしてモデルへ返す(モデルが直せる)。
#[tokio::test]
async fn a_failing_tool_is_reported_to_the_model_as_an_error_result() {
    let mut tools = FakeTools::new("シートが見つかりません");
    tools.is_error = true;
    let api = MockApi::start(vec![
        turn(
            vec![tool_block(0, "toolu_9", "place_symbol", "{}")],
            "tool_use",
        ),
        turn(vec![text_block(0, &["直します"])], "end_turn"),
    ])
    .await;
    let backend = AnthropicApiBackend::new("sk-test", "claude-sonnet-5")
        .with_base_url(api.base_url())
        .with_tools(Arc::new(tools));

    let events = run(&backend, "置いて", None).await;

    assert!(
        events.iter().any(
            |e| matches!(e, AgentEvent::ToolUseFinished { is_error, .. } if *is_error)
        ),
        "ツール失敗が伝わっていない: {events:?}"
    );
    let last = api.request(1);
    let result = &last["messages"].as_array().unwrap().last().unwrap()["content"][0];
    assert_eq!(result["is_error"], true);
    assert_eq!(result["content"], "シートが見つかりません");
}

/// The tool loop stops after a bounded number of rounds so a looping model cannot run forever.
/// ツール実行の往復には上限があり、堂々巡りになったモデルが延々と動き続けない。
#[tokio::test]
async fn the_tool_loop_stops_after_a_bounded_number_of_rounds() {
    let script: Vec<Scripted> = (0..10)
        .map(|_| {
            turn(
                vec![tool_block(0, "toolu_x", "place_symbol", "{}")],
                "tool_use",
            )
        })
        .collect();
    let api = MockApi::start(script).await;
    let mut backend = AnthropicApiBackend::new("sk-test", "claude-sonnet-5")
        .with_base_url(api.base_url())
        .with_tools(Arc::new(FakeTools::new("{}")));
    backend.max_tool_rounds = 3;

    let events = run(&backend, "ずっと置いて", None).await;

    assert_eq!(api.request_count(), 3, "上限を超えてAPIを呼んでいる");
    assert!(
        error_message(&events).contains("上限"),
        "上限に達したことがユーザーへ伝わらない: {events:?}"
    );
}

/// A rejected API key produces a message that says the key is the problem, not a raw HTTP code.
/// APIキーが弾かれたときは、生のHTTPコードではなく「キーの問題」と分かる文言を表示する。
#[tokio::test]
async fn a_rejected_api_key_is_explained_as_a_key_problem() {
    let api = MockApi::start(vec![Scripted::Status(
        401,
        json!({"type": "error", "error": {"type": "authentication_error", "message": "invalid x-api-key"}}),
    )])
    .await;
    let backend =
        AnthropicApiBackend::new("sk-bad", "claude-sonnet-5").with_base_url(api.base_url());

    let events = run(&backend, "こんにちは", None).await;

    let message = error_message(&events);
    assert!(message.contains("APIキー"), "キーの問題と分からない: {message}");
    assert!(
        !message.contains("sk-bad"),
        "エラー文にAPIキーが混ざっている: {message}"
    );
}

/// An overloaded API produces a "busy, try again" message rather than a bare error code.
/// APIが混み合っているときは「混雑しているので時間をおいて」と読める文言を表示する。
#[tokio::test]
async fn an_overloaded_api_is_explained_as_a_busy_service() {
    let api = MockApi::start(vec![Scripted::Status(
        529,
        json!({"type": "error", "error": {"type": "overloaded_error", "message": "Overloaded"}}),
    )])
    .await;
    let backend =
        AnthropicApiBackend::new("sk-test", "claude-sonnet-5").with_base_url(api.base_url());

    let message = error_message(&run(&backend, "こんにちは", None).await);
    assert!(message.contains("混雑"), "混雑と分からない: {message}");
}

/// An error event that arrives mid-stream is surfaced to the user too.
/// ストリームの途中で届いたエラーもユーザーへ伝える(黙って途切れさせない)。
#[tokio::test]
async fn an_error_event_inside_the_stream_is_surfaced() {
    let api = MockApi::start(vec![Scripted::Sse(vec![
        message_start(),
        (
            "error",
            json!({"type": "error", "error": {"type": "overloaded_error", "message": "Overloaded"}}),
        ),
    ])])
    .await;
    let backend =
        AnthropicApiBackend::new("sk-test", "claude-sonnet-5").with_base_url(api.base_url());

    let message = error_message(&run(&backend, "こんにちは", None).await);
    assert!(message.contains("混雑"), "混雑と分からない: {message}");
}

/// Earlier turns of the conversation are replayed so the model remembers what was said before.
/// 会話の前のやりとりも一緒に送るので、モデルは前に話した内容を覚えている。
#[tokio::test]
async fn earlier_turns_of_the_conversation_are_replayed() {
    use madake_agent::backend::HistoryMessage;
    use madake_agent::Role;

    let api = MockApi::start(vec![turn(vec![text_block(0, &["R1です"])], "end_turn")]).await;
    let backend =
        AnthropicApiBackend::new("sk-test", "claude-sonnet-5").with_base_url(api.base_url());

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

    let messages = api.request(0);
    let messages = messages["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 3, "履歴が送られていない: {messages:?}");
    assert_eq!(messages[0]["content"], "リレーを置いて");
    assert_eq!(messages[1]["role"], "assistant");
    assert_eq!(messages[1]["content"], "R1を置きました");
    assert_eq!(messages[2]["content"], "さっき置いたのは?");
}

/// The connection test reports success for a working key and a readable reason for a bad one.
/// 接続テストは、使えるキーなら成功を、駄目なキーなら読める理由を返す。
#[tokio::test]
async fn the_connection_test_reports_success_or_a_readable_reason() {
    let api = MockApi::start(vec![
        Scripted::Status(
            200,
            json!({"id": "msg_1", "type": "message", "role": "assistant", "content": [{"type": "text", "text": "hi"}], "usage": {"input_tokens": 1, "output_tokens": 1}}),
        ),
        Scripted::Status(
            401,
            json!({"type": "error", "error": {"type": "authentication_error", "message": "invalid x-api-key"}}),
        ),
    ])
    .await;
    let backend =
        AnthropicApiBackend::new("sk-test", "claude-sonnet-5").with_base_url(api.base_url());

    assert!(backend.check_connection().await.is_ok(), "疎通に失敗した");
    let error = backend.check_connection().await.unwrap_err();
    // 区分はUIが翻訳するため、文言だけでなく機械可読な種類も返す
    assert_eq!(error.kind, "auth", "エラーの種類が違う: {error:?}");
    assert!(error.message.contains("APIキー"), "理由が読めない: {error:?}");
}
