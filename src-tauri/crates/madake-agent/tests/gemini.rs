//! Google Gemini API バックエンド([`GeminiBackend`])のテスト。
//!
//! 本物のGeminiは呼ばない。テスト内でHTTPサーバーを立て、ベースURLをそこへ向けて
//! SSE(`?alt=sse`のストリーミング)応答を台本どおりに返す。確かめるのは4点:
//! 「APIへ何をどこへ送るか」「応答をどのUIイベントへ変換するか」「関数呼び出しの
//! 往復が回るか」「失敗を人が読める言葉にできるか」。

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use madake_agent::backend::{AgentBackend, TurnRequest};
use madake_agent::gemini::{GeminiBackend, DEFAULT_GEMINI_BASE_URL, DEFAULT_GEMINI_MODEL};
use madake_agent::tools::{ToolBridge, ToolDef, ToolFuture, ToolOutcome};
use madake_agent::AgentEvent;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

// ---------------------------------------------------------------- モックAPI

/// 1リクエストに返す台本。
enum Scripted {
    /// 200 + SSEのチャンク列(`data: {...}`。Geminiは終端マーカーを送らず、そのまま切れる)
    Sse(Vec<Value>),
    /// エラー応答(HTTPステータス + JSONボディ)
    Status(u16, Value),
}

/// 届いたリクエスト1件(パス・ヘッダ・ボディ)。
#[derive(Debug, Clone)]
struct Recorded {
    /// クエリ文字列込みのパス(`?alt=sse`まで見る)
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

    /// ヘッダ全体を1本の文字列にする(「どこにも載っていない」の確認用)。
    fn headers_text(&self) -> String {
        self.headers
            .iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// テスト用のGeminiサーバーもどき。台本を先頭から1リクエストずつ消費する。
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
                        &json!({"error": {"code": 500, "message": "台本切れ", "status": "INTERNAL"}}),
                    ),
                };
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.flush().await;
                let _ = socket.shutdown().await;
            }
        });
        Self { addr, requests }
    }

    /// `/v1beta`まで(`models/<model>:streamGenerateContent`の1つ上)。
    fn base_url(&self) -> String {
        format!("http://{}/v1beta", self.addr)
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
        body.push_str(&format!("data: {chunk}\r\n\r\n"));
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

/// 本文のチャンク(Geminiは1チャンク=GenerateContentResponse 1つ)。
fn text_chunk(text: &str) -> Value {
    json!({"candidates": [{"content": {"role": "model", "parts": [{"text": text}]}, "index": 0}]})
}

/// 関数呼び出しのチャンク(引数は文字列ではなくJSONオブジェクトで届く)。
fn function_call_chunk(id: Option<&str>, name: &str, args: Value) -> Value {
    let mut call = serde_json::Map::new();
    if let Some(id) = id {
        call.insert("id".into(), json!(id));
    }
    call.insert("name".into(), json!(name));
    call.insert("args".into(), args);
    json!({"candidates": [{
        "content": {"role": "model", "parts": [{"functionCall": Value::Object(call)}]},
        "finishReason": "STOP",
        "index": 0,
    }]})
}

/// 使用量つきの終端チャンク(Geminiは**その時点までの累計**を毎回載せてくる)。
fn usage_chunk(prompt: u64, candidates: u64) -> Value {
    json!({
        "candidates": [{"content": {"role": "model", "parts": [{"text": ""}]}, "finishReason": "STOP", "index": 0}],
        "usageMetadata": {
            "promptTokenCount": prompt,
            "candidatesTokenCount": candidates,
            "totalTokenCount": prompt + candidates,
        },
    })
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

    /// JSON Schemaの方言(Geminiが受け付けない項目)をわざと混ぜたツール定義。
    fn with_messy_schema() -> Self {
        Self {
            defs: vec![ToolDef {
                name: "place_symbol".into(),
                description: "シンボルをシートに配置する".into(),
                input_schema: json!({
                    "$schema": "https://json-schema.org/draft/2020-12/schema",
                    "title": "PlaceSymbol",
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "symbol_id": {"type": "string", "description": "シンボルID"},
                        // 数値の書式はGeminiが受け付ける綴りだけを残す
                        "rotation": {"type": "integer", "format": "uint32", "minimum": 0},
                        // 参照だけのプロパティ(型が消えても壊れないこと)
                        "point": {"$ref": "#/$defs/Point"},
                        "mode": {"allOf": [{"type": "string"}], "enum": ["new", "copy"]},
                    },
                    "required": ["symbol_id"],
                    "$defs": {"Point": {"type": "object", "properties": {}}},
                }),
            }],
            calls: Arc::new(Mutex::new(Vec::new())),
            result: "{}".to_string(),
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

async fn run(backend: &GeminiBackend, prompt: &str, system: Option<&str>) -> Vec<AgentEvent> {
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
fn backend_with_key(api: &MockApi) -> GeminiBackend {
    GeminiBackend::new("test-key", "gemini-2.5-flash", api.base_url())
}

// ------------------------------------------------------------------- テスト

/// Streamed model text arrives as text deltas and the turn ends with the assembled reply and its token usage.
/// Geminiから届いた本文はそのまま文字の差分として流れ、ターンの終わりに組み立てた返答とトークン使用量が付く。
#[tokio::test]
async fn streamed_text_becomes_deltas_and_a_completed_turn() {
    let api = MockApi::start(vec![Scripted::Sse(vec![
        text_chunk("こん"),
        text_chunk("にちは"),
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

/// The repeated running totals Gemini puts on every chunk are not added up twice.
/// Geminiが毎チャンクに載せてくる累計の使用量を二重に足し込まない。
#[tokio::test]
async fn repeated_running_token_totals_are_not_counted_twice() {
    let api = MockApi::start(vec![Scripted::Sse(vec![
        usage_chunk(100, 5),
        usage_chunk(100, 12),
    ])])
    .await;
    let backend = backend_with_key(&api);

    let events = run(&backend, "こんにちは", None).await;

    let usage = events
        .iter()
        .find_map(|e| match e {
            AgentEvent::TurnCompleted { usage, .. } => usage.clone(),
            _ => None,
        })
        .expect("使用量が無い");
    assert_eq!(usage.input_tokens, 100, "入力トークンを二重に数えている");
    assert_eq!(usage.output_tokens, 12, "最後の累計になっていない");
}

/// The request goes to the streaming endpoint of the configured model and carries the system instruction and the user's message.
/// リクエストは設定したモデルのストリーミング用エンドポイントへ行き、システム指示とユーザーの発言を載せる。
#[tokio::test]
async fn the_request_goes_to_the_streaming_endpoint_with_the_system_instruction_and_message() {
    let api = MockApi::start(vec![Scripted::Sse(vec![text_chunk("ok")])]).await;
    let backend = backend_with_key(&api);

    run(&backend, "R1を置いて", Some("あなたは電気図面のCADです")).await;

    let request = api.request(0);
    assert_eq!(
        request.path, "/v1beta/models/gemini-2.5-flash:streamGenerateContent?alt=sse",
        "送り先が違う"
    );
    let body = &request.body;
    // Geminiのシステム指示は専用の欄(messagesの先頭ではない)
    assert_eq!(
        body["systemInstruction"]["parts"][0]["text"],
        "あなたは電気図面のCADです"
    );
    assert_eq!(body["contents"][0]["role"], "user");
    assert_eq!(body["contents"][0]["parts"][0]["text"], "R1を置いて");
}

/// The bridged MCP tools are offered as Gemini function declarations so the agent can edit the drawing.
/// ブリッジしたMCPツールはGeminiのfunctionDeclarations形式で渡る(エージェントが図面を編集できる)。
#[tokio::test]
async fn the_bridged_mcp_tools_are_offered_as_function_declarations() {
    let api = MockApi::start(vec![Scripted::Sse(vec![text_chunk("ok")])]).await;
    let backend = backend_with_key(&api).with_tools(Arc::new(FakeTools::new("{}")));

    run(&backend, "置いて", None).await;

    let body = api.request(0).body;
    let declaration = &body["tools"][0]["functionDeclarations"][0];
    assert_eq!(declaration["name"], "place_symbol");
    assert_eq!(declaration["description"], "シンボルをシートに配置する");
    assert_eq!(declaration["parameters"]["type"], "object");
    assert_eq!(
        declaration["parameters"]["properties"]["symbol_id"]["type"],
        "string"
    );
}

/// Schema keywords Gemini does not accept are dropped from the tool definitions, and a property left without a type still gets one.
/// Geminiが受け付けないスキーマの項目はツール定義から落とし、型が無くなったプロパティにも型を補う。
#[tokio::test]
async fn schema_keywords_gemini_rejects_are_dropped_from_tool_definitions() {
    let api = MockApi::start(vec![Scripted::Sse(vec![text_chunk("ok")])]).await;
    let backend = backend_with_key(&api).with_tools(Arc::new(FakeTools::with_messy_schema()));

    run(&backend, "置いて", None).await;

    let body = api.request(0).body;
    let parameters = &body["tools"][0]["functionDeclarations"][0]["parameters"];
    let text = parameters.to_string();
    for rejected in [
        "$schema",
        "additionalProperties",
        "$defs",
        "$ref",
        "allOf",
        "uint32",
    ] {
        assert!(
            !text.contains(rejected),
            "Geminiが受け付けない項目({rejected})が残っている: {text}"
        );
    }
    // 受け付ける項目は残す
    assert_eq!(parameters["type"], "object");
    assert_eq!(parameters["required"][0], "symbol_id");
    assert_eq!(
        parameters["properties"]["symbol_id"]["description"],
        "シンボルID"
    );
    assert_eq!(parameters["properties"]["rotation"]["minimum"], 0);
    assert_eq!(parameters["properties"]["mode"]["enum"][0], "new");
    // 型が消えたプロパティにも型を補う(型なしスキーマはGeminiが弾くため)
    assert!(
        parameters["properties"]["point"]["type"].is_string(),
        "型の無いプロパティが残っている: {text}"
    );
}

/// A tool the model asks for is executed locally and its result is sent back as a functionResponse so the model can continue.
/// モデルが要求したツールはその場で実行され、結果をfunctionResponseとして返して会話を続ける(1ターンで完結する)。
#[tokio::test]
async fn a_requested_tool_is_executed_and_its_result_is_sent_back() {
    let tools = Arc::new(FakeTools::new("{\"id\":\"e1\"}"));
    let calls = Arc::clone(&tools.calls);
    let api = MockApi::start(vec![
        Scripted::Sse(vec![
            text_chunk("配置します"),
            function_call_chunk(
                Some("call_1"),
                "place_symbol",
                json!({"symbol_id": "relay"}),
            ),
        ]),
        Scripted::Sse(vec![text_chunk("置きました")]),
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

    // 2回目のリクエストにはモデルの発言と実行結果が積まれている
    assert_eq!(api.request_count(), 2, "リクエスト回数が違う");
    let second = api.request(1).body;
    let contents = second["contents"].as_array().expect("contentsが配列でない");
    let model_turn = contents
        .iter()
        .find(|c| c["role"] == "model")
        .expect("モデルの発言が積まれていない");
    let call_part = model_turn["parts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p.get("functionCall").is_some())
        .expect("関数呼び出しが積み直されていない");
    assert_eq!(call_part["functionCall"]["name"], "place_symbol");
    assert_eq!(call_part["functionCall"]["args"]["symbol_id"], "relay");

    // 結果はuserロールのfunctionResponseで返す(名前とidで呼び出しと対応づく)
    let last = contents.last().expect("contentsが空");
    assert_eq!(last["role"], "user");
    let response = &last["parts"][0]["functionResponse"];
    assert_eq!(response["name"], "place_symbol");
    assert_eq!(response["id"], "call_1");
    assert_eq!(response["response"]["result"], "{\"id\":\"e1\"}");

    // 最終応答は2回目の本文
    assert!(
        events.iter().any(
            |e| matches!(e, AgentEvent::TurnCompleted { result, .. } if result == "置きました")
        ),
        "最終応答が2回目の本文になっていない: {events:?}"
    );
}

/// A model that omits the call id still gets its result back, matched by the function name.
/// 呼び出しIDを付けてこないモデルにも、関数名で対応づけて結果を返す。
#[tokio::test]
async fn a_call_without_an_id_is_answered_by_function_name() {
    let api = MockApi::start(vec![
        Scripted::Sse(vec![function_call_chunk(
            None,
            "place_symbol",
            json!({"symbol_id": "relay"}),
        )]),
        Scripted::Sse(vec![text_chunk("置きました")]),
    ])
    .await;
    let backend = backend_with_key(&api).with_tools(Arc::new(FakeTools::new("{}")));

    run(&backend, "置いて", None).await;

    let contents = api.request(1).body["contents"].clone();
    let last = contents.as_array().unwrap().last().unwrap().clone();
    let response = &last["parts"][0]["functionResponse"];
    assert_eq!(response["name"], "place_symbol");
    // 元の呼び出しにIDが無ければ付けない(APIが知らないIDを返さない)
    assert!(
        response.get("id").is_none(),
        "モデルが送っていないIDを返している: {response}"
    );
}

/// A tool that fails is reported back to the model as an error field instead of aborting the turn.
/// 失敗したツールはターンを中断せず、エラーの欄に理由を入れてモデルへ返す(モデルが直せる)。
#[tokio::test]
async fn a_failing_tool_is_reported_to_the_model_as_an_error_result() {
    let mut tools = FakeTools::new("シートが見つかりません");
    tools.is_error = true;
    let api = MockApi::start(vec![
        Scripted::Sse(vec![function_call_chunk(
            Some("call_9"),
            "place_symbol",
            json!({}),
        )]),
        Scripted::Sse(vec![text_chunk("直します")]),
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
    let contents = api.request(1).body["contents"].clone();
    let last = contents.as_array().unwrap().last().unwrap().clone();
    let response = &last["parts"][0]["functionResponse"]["response"];
    let error = response["error"].as_str().unwrap_or_default();
    assert!(
        error.contains("シートが見つかりません"),
        "失敗の理由が届いていない: {response}"
    );
    assert!(
        response.get("result").is_none(),
        "失敗なのに成功の欄に入っている: {response}"
    );
}

/// The tool loop stops after a bounded number of rounds so a looping model cannot run forever.
/// 関数呼び出しの往復には上限があり、堂々巡りになったモデルが延々と動き続けない。
#[tokio::test]
async fn the_tool_loop_stops_after_a_bounded_number_of_rounds() {
    let script: Vec<Scripted> = (0..10)
        .map(|_| {
            Scripted::Sse(vec![function_call_chunk(
                Some("call_x"),
                "place_symbol",
                json!({}),
            )])
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

/// The API key travels only in the x-goog-api-key header, never in the URL, the body, or anything shown to the user.
/// APIキーはx-goog-api-keyヘッダだけで送られ、URLにも本文にもユーザーに見える文言にも現れない。
#[tokio::test]
async fn the_api_key_travels_only_in_the_header() {
    const KEY: &str = "AIza-secret-0001";
    let api = MockApi::start(vec![Scripted::Sse(vec![text_chunk("ok")])]).await;
    let backend = GeminiBackend::new(KEY, "gemini-2.5-flash", api.base_url());

    let events = run(&backend, "こんにちは", None).await;

    let request = api.request(0);
    assert_eq!(request.header("x-goog-api-key"), Some(KEY));
    assert!(
        !request.path.contains(KEY),
        "URLにキーが載っている(履歴・ログに残る): {}",
        request.path
    );
    assert!(
        !request.body.to_string().contains(KEY),
        "リクエスト本文にキーが載っている"
    );
    // Authorizationヘッダは使わない(Geminiの認証は専用ヘッダ)
    assert_eq!(request.header("authorization"), None);
    let serialized = serde_json::to_string(&events).unwrap();
    assert!(
        !serialized.contains(KEY),
        "イベントにキーが混ざっている: {serialized}"
    );
    // 念のため、ヘッダ以外の場所へ写っていないことも確かめる
    assert!(request.headers_text().contains(KEY));
}

/// A rejected API key produces a message that says the key is the problem, not a raw HTTP code.
/// APIキーが弾かれたときは、生のHTTPコードではなく「キーの問題」と分かる文言を表示する。
#[tokio::test]
async fn a_rejected_api_key_is_explained_as_a_key_problem() {
    let api = MockApi::start(vec![Scripted::Status(
        400,
        json!({"error": {
            "code": 400,
            "message": "API key not valid. Please pass a valid API key.",
            "status": "INVALID_ARGUMENT",
            "details": [{"@type": "type.googleapis.com/google.rpc.ErrorInfo", "reason": "API_KEY_INVALID"}],
        }}),
    )])
    .await;
    let backend = GeminiBackend::new("AIza-bad-key", "gemini-2.5-flash", api.base_url());

    let message = error_message(&run(&backend, "こんにちは", None).await);

    assert!(
        message.contains("APIキー"),
        "キーの問題と分からない: {message}"
    );
    assert!(
        !message.contains("AIza-bad-key"),
        "エラー文にAPIキーが混ざっている: {message}"
    );
}

/// Hitting the Gemini usage limit is explained as a usage limit with a "try again later" hint.
/// Geminiの利用上限に当たったときは「利用上限に達したので時間をおいて」と読める文言を表示する。
#[tokio::test]
async fn a_rate_limited_service_is_explained_as_a_usage_limit() {
    let api = MockApi::start(vec![Scripted::Status(
        429,
        json!({"error": {"code": 429, "message": "Resource has been exhausted", "status": "RESOURCE_EXHAUSTED"}}),
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
        json!({"error": {"code": 404, "message": "models/gemini-nope is not found for API version v1beta", "status": "NOT_FOUND"}}),
    )])
    .await;
    let backend = GeminiBackend::new("test-key", "gemini-nope", api.base_url());

    let message = error_message(&run(&backend, "こんにちは", None).await);
    assert!(
        message.contains("モデル"),
        "モデルの問題と分からない: {message}"
    );
    assert!(
        message.contains("gemini-nope"),
        "どのモデル名で失敗したか分からない: {message}"
    );
}

/// A server that cannot be reached names the URL that was tried instead of failing silently.
/// サーバーへ接続できないときは、黙って失敗せずに試したURLを示す。
#[tokio::test]
async fn an_unreachable_server_names_the_url_that_was_tried() {
    // 誰も待ち受けていないポートへ向ける(接続そのものが失敗する)
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    let base = format!("http://127.0.0.1:{}/v1beta", addr.port());
    let backend = GeminiBackend::new("test-key", "gemini-2.5-flash", base.clone());

    let message = error_message(&run(&backend, "こんにちは", None).await);

    assert!(message.contains(&base), "試したURLが分からない: {message}");
    assert!(
        !message.contains("test-key"),
        "エラー文にAPIキーが混ざっている: {message}"
    );
}

/// Earlier turns of the conversation are replayed so the model remembers what was said before.
/// 会話の前のやりとりも一緒に送るので、モデルは前に話した内容を覚えている。
#[tokio::test]
async fn earlier_turns_of_the_conversation_are_replayed() {
    use madake_agent::backend::HistoryMessage;
    use madake_agent::Role;

    let api = MockApi::start(vec![Scripted::Sse(vec![text_chunk("R1です")])]).await;
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

    let contents = api.request(0).body["contents"].clone();
    let contents = contents.as_array().unwrap();
    assert_eq!(contents.len(), 3, "履歴が送られていない: {contents:?}");
    assert_eq!(contents[0]["parts"][0]["text"], "リレーを置いて");
    // アシスタントの発言はGeminiでは「model」ロール
    assert_eq!(contents[1]["role"], "model");
    assert_eq!(contents[1]["parts"][0]["text"], "R1を置きました");
    assert_eq!(contents[2]["parts"][0]["text"], "さっき置いたのは?");
}

/// The connection test reports success for a working setup and a readable, machine-tagged reason for a broken one.
/// 接続テストは、使える設定なら成功を、駄目な設定なら区分つきの読める理由を返す。
#[tokio::test]
async fn the_connection_test_reports_success_or_a_readable_reason() {
    let api = MockApi::start(vec![
        Scripted::Status(
            200,
            json!({"candidates": [{"content": {"role": "model", "parts": [{"text": "pong"}]}}]}),
        ),
        Scripted::Status(
            400,
            json!({"error": {"code": 400, "message": "API key not valid", "status": "INVALID_ARGUMENT", "details": [{"reason": "API_KEY_INVALID"}]}}),
        ),
    ])
    .await;
    let backend = backend_with_key(&api);

    assert!(backend.check_connection().await.is_ok(), "疎通に失敗した");
    let error = backend.check_connection().await.unwrap_err();
    // 区分はUIが翻訳するため、文言だけでなく機械可読な種類も返す
    assert_eq!(error.kind, "gemini_auth", "エラーの種類が違う: {error:?}");
    assert!(
        error.message.contains("APIキー"),
        "理由が読めない: {error:?}"
    );
}

/// The connection test uses the plain (non-streaming) endpoint of the configured model.
/// 接続テストは設定したモデルの通常(非ストリーミング)エンドポイントを叩く。
#[tokio::test]
async fn the_connection_test_uses_the_non_streaming_endpoint() {
    let api = MockApi::start(vec![Scripted::Status(
        200,
        json!({"candidates": [{"content": {"role": "model", "parts": [{"text": "pong"}]}}]}),
    )])
    .await;
    let backend = backend_with_key(&api);

    backend.check_connection().await.expect("疎通できる");

    assert_eq!(
        api.request(0).path,
        "/v1beta/models/gemini-2.5-flash:generateContent"
    );
}

/// The endpoint and the default model match Google's published Gemini API.
/// 接続先と既定のモデル名はGoogleが公開しているGemini APIのものになっている。
#[test]
fn the_defaults_match_the_published_gemini_api() {
    assert_eq!(
        DEFAULT_GEMINI_BASE_URL,
        "https://generativelanguage.googleapis.com/v1beta"
    );
    assert_eq!(DEFAULT_GEMINI_MODEL, "gemini-2.5-flash");
}

/// A model name pasted with the "models/" prefix still reaches the right endpoint exactly once.
/// 「models/」付きで貼り付けたモデル名でも、正しいエンドポイントへ1回だけつながる。
#[tokio::test]
async fn a_model_name_with_the_models_prefix_still_works() {
    let api = MockApi::start(vec![Scripted::Sse(vec![text_chunk("ok")])]).await;
    let backend = GeminiBackend::new(
        "test-key",
        "models/gemini-2.5-flash",
        format!("{}/", api.base_url()),
    );

    run(&backend, "こんにちは", None).await;

    assert_eq!(
        api.request(0).path,
        "/v1beta/models/gemini-2.5-flash:streamGenerateContent?alt=sse"
    );
}

/// With no model name saved the turn stops before any request, telling the user which box to fill in.
/// モデル名が無いときは通信する前にターンを止め、どこを埋めればよいかを伝える。
#[tokio::test]
async fn an_empty_model_name_stops_before_any_request() {
    let api = MockApi::start(vec![Scripted::Sse(vec![text_chunk("ok")])]).await;
    let backend = GeminiBackend::new("test-key", "  ", api.base_url());

    let message = error_message(&run(&backend, "こんにちは", None).await);

    assert_eq!(api.request_count(), 0, "モデル名が無いのに通信している");
    assert!(message.contains("モデル"), "理由が読めない: {message}");
}
