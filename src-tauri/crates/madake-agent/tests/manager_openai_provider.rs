//! プロバイダ切り替え(OpenAI互換API / Ollama)のテスト。
//!
//! 狙いは受け入れ基準そのもの: **OpenAI互換のURLとモデルを設定するだけでチャット
//! 作図が動く**(claude CLIもAnthropicのキーも要らない)。ローカルのOllamaは
//! **キー無しのまま**使えることも固定する。本物のAPIは呼ばず、テスト内のHTTPサーバーへ向ける。
//!
//! キー保管の差し替えはプロセス全体に効くため、このファイルだけを別のテストバイナリにしてある。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use madake_agent::manager::{
    AgentManager, ConversationEvent, DocBridge, RevertError, RevertReport,
};
use madake_agent::openai_compat::OLLAMA_BASE_URL;
use madake_agent::secrets::{self, MemoryStore};
use madake_agent::{AgentEvent, AgentProvider, AppSettings};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::broadcast::Receiver;

/// 何も起きないドキュメント(このテストでは編集を見ない)。
#[derive(Default)]
struct QuietDoc {
    revision: AtomicU64,
}

impl DocBridge for QuietDoc {
    fn revision(&self) -> u64 {
        self.revision.load(Ordering::SeqCst)
    }
    fn undo_depth(&self) -> u64 {
        0
    }
    fn revert_agent_edits(&self, _: u64, _: u64) -> Result<RevertReport, RevertError> {
        Ok(RevertReport::default())
    }
}

/// SSEを1本返すだけのモックサーバー。戻り値は`{base_url}`(末尾に`/v1`)。
async fn mock_api(chunks: Vec<Value>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 8192];
            let _ = socket.read(&mut buf).await;
            let mut body = String::new();
            for chunk in &chunks {
                body.push_str(&format!("data: {chunk}\n\n"));
            }
            body.push_str("data: [DONE]\n\n");
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
    });
    format!("http://{addr}/v1")
}

fn hello_stream() -> Vec<Value> {
    vec![
        json!({"choices": [{"index": 0, "delta": {"content": "OpenAI互換サーバーから返事です"}}]}),
        json!({"choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]}),
        json!({"choices": [], "usage": {"prompt_tokens": 10, "completion_tokens": 5}}),
    ]
}

fn openai_settings(base_url: &str) -> AppSettings {
    AppSettings {
        provider: AgentProvider::OpenAiCompat,
        openai_base_url: base_url.to_string(),
        openai_model: "gpt-4o".into(),
        // 図面コンテキストは今回関係ないので切っておく
        auto_read_drawing: false,
        ..AppSettings::default()
    }
}

fn manager(settings: AppSettings) -> Arc<AgentManager> {
    let manager = Arc::new(AgentManager::new(Arc::new(QuietDoc::default()), 9310));
    manager.apply_settings(settings);
    // claude CLIは「入っていない」状態にする(このパスは存在しない)
    manager.set_executable(Some(std::path::PathBuf::from(
        "/nonexistent/claude-not-installed",
    )));
    manager
}

async fn collect_turn(rx: &mut Receiver<ConversationEvent>) -> Vec<AgentEvent> {
    let mut events = Vec::new();
    loop {
        let next = tokio::time::timeout(Duration::from_secs(10), rx.recv())
            .await
            .expect("イベント待ちがタイムアウト")
            .expect("配信チャネルが閉じた");
        let terminal = matches!(
            next.event,
            AgentEvent::TurnCompleted { .. } | AgentEvent::Error { .. }
        );
        events.push(next.event);
        if terminal {
            return events;
        }
    }
}

/// With an OpenAI-compatible endpoint selected, a chat turn runs even though no Claude CLI is installed.
/// OpenAI互換APIを選んでおけば、Claude Code CLIが入っていない環境でもチャットのやりとりができる。
#[tokio::test]
async fn a_turn_runs_on_an_openai_compatible_endpoint() {
    let _store = secrets::install_store(Arc::new(MemoryStore::default()));
    let base = mock_api(hello_stream()).await;
    secrets::set_openai_compat_api_key("sk-openai-test").unwrap();

    let manager = manager(openai_settings(&base));
    let mut rx = manager.subscribe();
    manager
        .send(None, "こんにちは", None, None)
        .await
        .expect("キーがあれば送信できる");
    let events = collect_turn(&mut rx).await;

    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::TextDelta { text } if text.contains("OpenAI互換"))),
        "OpenAI互換サーバーからの本文が届いていない: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::TurnCompleted { .. })),
        "ターンが完了していない: {events:?}"
    );
}

/// A local Ollama URL needs no API key at all, so a turn runs with nothing saved in the keychain.
/// ローカルのOllamaのURLならAPIキーは要らず、キーチェーンに何も保存していなくてもやりとりできる。
#[tokio::test]
async fn a_local_ollama_url_needs_no_api_key() {
    let _store = secrets::install_store(Arc::new(MemoryStore::default()));
    // モックは127.0.0.1で待ち受ける=ローカル扱い(キー不要)
    let base = mock_api(hello_stream()).await;

    let manager = manager(AppSettings {
        openai_model: "llama3.2".into(),
        ..openai_settings(&base)
    });
    let mut rx = manager.subscribe();
    manager
        .send(None, "こんにちは", None, None)
        .await
        .expect("ローカルURLならキー無しでも送信できる");
    let events = collect_turn(&mut rx).await;

    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::TurnCompleted { .. })),
        "ターンが完了していない: {events:?}"
    );
}

/// Choosing a remote OpenAI-compatible endpoint without saving a key refuses the send with a message pointing at the settings screen.
/// 社外のOpenAI互換APIを選んだのにキーを保存していないと、設定画面へ促す文言で送信を断る。
#[tokio::test]
async fn sending_to_a_remote_endpoint_without_a_key_points_at_the_settings_screen() {
    let _store = secrets::install_store(Arc::new(MemoryStore::default()));
    let manager = manager(openai_settings("https://api.openai.com/v1"));

    let error = manager
        .send(None, "こんにちは", None, None)
        .await
        .expect_err("キーが無ければ送信できない");
    let message = error.to_string();
    assert!(message.contains("APIキー"), "理由が読めない: {message}");
    assert!(
        message.contains("設定"),
        "どこで直すか分からない: {message}"
    );
}

/// Leaving the model box empty refuses the send and says which box to fill in.
/// モデル名を空のままにしていると送信を断り、どこを埋めればよいかを伝える。
#[tokio::test]
async fn sending_without_a_model_name_says_which_box_to_fill_in() {
    let _store = secrets::install_store(Arc::new(MemoryStore::default()));
    let manager = manager(AppSettings {
        openai_model: String::new(),
        ..openai_settings(OLLAMA_BASE_URL)
    });

    let error = manager
        .send(None, "こんにちは", None, None)
        .await
        .expect_err("モデル名が無ければ送信できない");
    let message = error.to_string();
    assert!(message.contains("モデル"), "理由が読めない: {message}");
    assert!(
        message.contains("設定"),
        "どこで直すか分からない: {message}"
    );
}

/// The provider badge reports "ready" once a key is saved (or the URL is local) and a model name is filled in.
/// プロバイダのバッジは、キーを保存済み(またはURLがローカル)でモデル名が入っているときだけ「使える」状態になる。
#[tokio::test]
async fn the_provider_is_ready_with_a_key_or_a_local_url_and_a_model() {
    let _store = secrets::install_store(Arc::new(MemoryStore::default()));

    // 社外のURL + キー無し = まだ使えない
    let remote = manager(openai_settings("https://api.openai.com/v1"));
    assert!(!remote.provider_ready().await, "キーが無いのに使える判定");
    secrets::set_openai_compat_api_key("sk-openai-test").unwrap();
    assert!(remote.provider_ready().await, "キーがあるのに使えない判定");
    secrets::clear_openai_compat_api_key().unwrap();
    assert!(!remote.provider_ready().await, "削除後も使える判定のまま");

    // ローカル(Ollama)はキー無しでも使える。ただしモデル名は要る
    let local = manager(openai_settings(OLLAMA_BASE_URL));
    assert!(
        local.provider_ready().await,
        "ローカルURLなのに使えない判定"
    );
    let no_model = manager(AppSettings {
        openai_model: "   ".into(),
        ..openai_settings(OLLAMA_BASE_URL)
    });
    assert!(
        !no_model.provider_ready().await,
        "モデル名が無いのに使える判定"
    );
}

/// While an OpenAI-compatible endpoint is chosen, the connection test goes there instead of to the Anthropic API.
/// OpenAI互換APIを選んでいる間、接続テストはAnthropic APIではなくそのURLへ行く。
#[tokio::test]
async fn the_connection_test_follows_the_chosen_provider() {
    let _store = secrets::install_store(Arc::new(MemoryStore::default()));

    // キーを保存していないうちは通信せずに理由を返す(社外URL)
    let remote = manager(openai_settings("https://api.openai.com/v1"));
    let error = remote
        .test_connection()
        .await
        .expect_err("キーが無ければ接続テストは失敗する");
    assert_eq!(error.kind, "openai_no_key", "理由の種類が違う: {error:?}");
    assert!(
        error.message.contains("APIキー"),
        "理由が読めない: {error:?}"
    );

    // ローカルのURLならキー無しでも実際に叩く(モックは200を返す)
    let base = mock_api(hello_stream()).await;
    let local = manager(openai_settings(&base));
    assert_eq!(
        local.test_connection().await.expect("疎通できる"),
        "gpt-4o",
        "確かめたモデル名を返す"
    );
}

/// Nothing about the OpenAI key is ever broadcast to the UI event stream or written into the chat history.
/// OpenAI互換APIのキーはUIへ流れるイベントにも会話履歴にも一切載らない。
#[tokio::test]
async fn the_api_key_never_appears_in_the_event_stream() {
    const KEY: &str = "sk-openai-must-never-be-broadcast";
    let _store = secrets::install_store(Arc::new(MemoryStore::default()));
    let base = mock_api(hello_stream()).await;
    secrets::set_openai_compat_api_key(KEY).unwrap();

    let manager = manager(openai_settings(&base));
    let mut rx = manager.subscribe();
    manager.send(None, "こんにちは", None, None).await.unwrap();
    let events = collect_turn(&mut rx).await;

    let serialized = serde_json::to_string(&events).unwrap();
    assert!(
        !serialized.contains(KEY),
        "イベントにAPIキーが混ざっている: {serialized}"
    );
    let chat = serde_json::to_string(&manager.conversations()).unwrap();
    assert!(
        !chat.contains(KEY),
        "会話履歴にAPIキーが混ざっている: {chat}"
    );
}
