//! プロバイダ切り替え(Claude Code CLI / Anthropic API)のテスト。
//!
//! 狙いは仕様の受け入れ基準そのもの: **claude CLIが入っていない環境でも、
//! APIキーだけでチャットが動く**。本物のAPIは呼ばず、テスト内のHTTPサーバーへ向ける。
//!
//! 接続先の差し替えは環境変数なのでプロセス全体に効く。他のテストへ影響させないよう
//! このファイルだけを別のテストバイナリにしてある。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use madake_agent::manager::{AgentManager, ConversationEvent, DocBridge, RevertError, RevertReport};
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

/// SSEを1本返すだけのモックAPI。
async fn mock_api(events: Vec<Value>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 8192];
            let _ = socket.read(&mut buf).await;
            let mut body = String::new();
            for event in &events {
                body.push_str(&format!(
                    "event: {}\ndata: {event}\n\n",
                    event["type"].as_str().unwrap_or("message")
                ));
            }
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
    });
    format!("http://{addr}")
}

fn hello_stream() -> Vec<Value> {
    vec![
        json!({"type": "message_start", "message": {"id": "msg_1", "usage": {"input_tokens": 10, "output_tokens": 1}}}),
        json!({"type": "content_block_start", "index": 0, "content_block": {"type": "text", "text": ""}}),
        json!({"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "APIから返事です"}}),
        json!({"type": "content_block_stop", "index": 0}),
        json!({"type": "message_delta", "delta": {"stop_reason": "end_turn"}, "usage": {"output_tokens": 5}}),
        json!({"type": "message_stop"}),
    ]
}

fn api_settings() -> AppSettings {
    AppSettings {
        provider: AgentProvider::AnthropicApi,
        api_model: "claude-sonnet-5".into(),
        // 図面コンテキストは今回関係ないので切っておく
        auto_read_drawing: false,
        ..AppSettings::default()
    }
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

/// With the Anthropic API selected, a chat turn runs even though no Claude Code CLI is installed.
/// Anthropic APIを選んでおけば、Claude Code CLIが入っていない環境でもチャットのやりとりができる。
#[tokio::test]
async fn a_turn_runs_on_the_api_without_any_claude_cli() {
    // 差し替えガードを先に取り、握っている間だけ接続先を差し替える
    // (環境変数はプロセス全体に効くので、他のテストと重ならないようにする)
    let _store = secrets::install_store(Arc::new(MemoryStore::default()));
    let base = mock_api(hello_stream()).await;
    std::env::set_var(madake_agent::anthropic::API_BASE_ENV, &base);
    secrets::set_anthropic_api_key("sk-ant-test").unwrap();

    let manager = Arc::new(AgentManager::new(Arc::new(QuietDoc::default()), 9310));
    manager.apply_settings(api_settings());
    // CLIは「入っていない」状態にする(このパスは存在しない)
    manager.set_executable(Some(std::path::PathBuf::from(
        "/nonexistent/claude-not-installed",
    )));

    let mut rx = manager.subscribe();
    manager
        .send(None, "こんにちは", None, None)
        .await
        .expect("APIキーがあれば送信できる");
    let events = collect_turn(&mut rx).await;

    assert!(
        events.iter().any(|e| matches!(e, AgentEvent::TextDelta { text } if text.contains("API"))),
        "APIからの本文が届いていない: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::TurnCompleted { .. })),
        "ターンが完了していない: {events:?}"
    );
}

/// Choosing the API without saving a key refuses the send with a message pointing at the settings screen.
/// APIを選んだのにキーを保存していないと、設定画面へ促す文言で送信を断る。
#[tokio::test]
async fn sending_without_a_saved_key_points_at_the_settings_screen() {
    let _store = secrets::install_store(Arc::new(MemoryStore::default()));
    let manager = Arc::new(AgentManager::new(Arc::new(QuietDoc::default()), 9310));
    manager.apply_settings(api_settings());

    let error = manager
        .send(None, "こんにちは", None, None)
        .await
        .expect_err("キーが無ければ送信できない");
    let message = error.to_string();
    assert!(message.contains("APIキー"), "理由が読めない: {message}");
    assert!(message.contains("設定"), "どこで直すか分からない: {message}");
}

/// The provider badge reports "ready" once a key is saved and "not ready" once it is removed.
/// キーを保存すればプロバイダは「使える」状態になり、消せば「使えない」状態に戻る(バッジ表示用)。
#[tokio::test]
async fn the_provider_is_ready_only_while_a_key_is_saved() {
    let _store = secrets::install_store(Arc::new(MemoryStore::default()));
    let manager = Arc::new(AgentManager::new(Arc::new(QuietDoc::default()), 9310));
    manager.apply_settings(api_settings());

    assert!(!manager.provider_ready().await, "キーが無いのに使える判定");
    secrets::set_anthropic_api_key("sk-ant-test").unwrap();
    assert!(manager.provider_ready().await, "キーがあるのに使えない判定");
    secrets::clear_anthropic_api_key().unwrap();
    assert!(!manager.provider_ready().await, "削除後も使える判定のまま");
}

/// The connection test refuses before a key is saved, naming the missing key as the reason.
/// 接続テストはキーを保存する前は実行せず、「キーが無い」ことを理由として返す。
#[tokio::test]
async fn the_connection_test_refuses_before_a_key_is_saved() {
    let _store = secrets::install_store(Arc::new(MemoryStore::default()));
    let manager = Arc::new(AgentManager::new(Arc::new(QuietDoc::default()), 9310));
    manager.apply_settings(api_settings());

    let error = manager
        .test_anthropic_connection()
        .await
        .expect_err("キーが無ければ接続テストは失敗する");
    assert_eq!(error.kind, "no_key", "理由の種類が違う: {error:?}");
    assert!(error.message.contains("APIキー"), "理由が読めない: {error:?}");
}

/// Nothing about the key is ever broadcast to the UI event stream.
/// APIキーはUIへ流れるイベントに一切載らない。
#[tokio::test]
async fn the_api_key_never_appears_in_the_event_stream() {
    const KEY: &str = "sk-ant-must-never-be-broadcast";
    let _store = secrets::install_store(Arc::new(MemoryStore::default()));
    let base = mock_api(hello_stream()).await;
    std::env::set_var(madake_agent::anthropic::API_BASE_ENV, &base);
    secrets::set_anthropic_api_key(KEY).unwrap();

    let manager = Arc::new(AgentManager::new(Arc::new(QuietDoc::default()), 9310));
    manager.apply_settings(api_settings());
    let mut rx = manager.subscribe();
    manager.send(None, "こんにちは", None, None).await.unwrap();
    let events = collect_turn(&mut rx).await;

    let serialized = serde_json::to_string(&events).unwrap();
    assert!(
        !serialized.contains(KEY),
        "イベントにAPIキーが混ざっている: {serialized}"
    );
    // 会話履歴(保存先はプロジェクトの隣のJSON)にも残らない
    let chat = serde_json::to_string(&manager.conversations()).unwrap();
    assert!(!chat.contains(KEY), "会話履歴にAPIキーが混ざっている: {chat}");
}
