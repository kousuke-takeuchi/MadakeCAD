//! プロバイダ切り替え(Google Gemini)のテスト。
//!
//! 狙いは受け入れ基準そのもの: **Geminiのキーとモデルを設定するだけでチャット作図が
//! 動く**(claude CLIもAnthropicのキーも要らない)。キーはAnthropic・OpenAI互換とは
//! 別の入れ物に入り、イベントにも履歴にも出ないことも固定する。本物のGeminiは呼ばず、
//! テスト内のHTTPサーバーへ`MADAKE_GEMINI_BASE_URL`で向ける。
//!
//! キー保管の差し替えはプロセス全体に効くため、このファイルだけを別のテストバイナリにしてある。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use madake_agent::manager::{
    AgentManager, ConversationEvent, DocBridge, RevertError, RevertReport,
};
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

/// SSEを1本返すだけのモックサーバー。戻り値は`{base_url}`(末尾に`/v1beta`)。
async fn mock_api(chunks: Vec<Value>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 8192];
            let _ = socket.read(&mut buf).await;
            let mut body = String::new();
            for chunk in &chunks {
                body.push_str(&format!("data: {chunk}\r\n\r\n"));
            }
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
    });
    format!("http://{addr}/v1beta")
}

fn hello_stream() -> Vec<Value> {
    vec![
        json!({"candidates": [{"content": {"role": "model", "parts": [{"text": "Geminiから返事です"}]}}]}),
        json!({
            "candidates": [{"content": {"role": "model", "parts": [{"text": ""}]}, "finishReason": "STOP"}],
            "usageMetadata": {"promptTokenCount": 10, "candidatesTokenCount": 5},
        }),
    ]
}

fn gemini_settings() -> AppSettings {
    AppSettings {
        provider: AgentProvider::Gemini,
        gemini_model: "gemini-2.5-flash".into(),
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

/// With Google Gemini selected, a chat turn runs even though no Claude CLI is installed.
/// Google Geminiを選んでおけば、Claude Code CLIが入っていない環境でもチャットのやりとりができる。
#[tokio::test]
async fn a_turn_runs_on_google_gemini() {
    let _store = secrets::install_store(Arc::new(MemoryStore::default()));
    let base = mock_api(hello_stream()).await;
    std::env::set_var(madake_agent::gemini::GEMINI_BASE_URL_ENV, &base);
    secrets::set_gemini_api_key("AIza-test").unwrap();

    let manager = manager(gemini_settings());
    let mut rx = manager.subscribe();
    manager
        .send(None, "こんにちは", None, None)
        .await
        .expect("キーがあれば送信できる");
    let events = collect_turn(&mut rx).await;

    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::TextDelta { text } if text.contains("Gemini"))),
        "Geminiからの本文が届いていない: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::TurnCompleted { .. })),
        "ターンが完了していない: {events:?}"
    );
    std::env::remove_var(madake_agent::gemini::GEMINI_BASE_URL_ENV);
}

/// Choosing Gemini without saving a key refuses the send with a message pointing at the settings screen.
/// Geminiを選んだのにキーを保存していないと、設定画面へ促す文言で送信を断る。
#[tokio::test]
async fn sending_without_a_key_points_at_the_settings_screen() {
    let _store = secrets::install_store(Arc::new(MemoryStore::default()));
    let manager = manager(gemini_settings());

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

/// The Gemini key lives in its own keychain entry, so an Anthropic or OpenAI key does not make Gemini ready.
/// GeminiのキーはAnthropic・OpenAI互換とは別の入れ物に入るため、他社のキーがあってもGeminiは「使える」にならない。
#[tokio::test]
async fn the_gemini_key_is_stored_separately_from_the_other_providers() {
    let _store = secrets::install_store(Arc::new(MemoryStore::default()));
    let manager = manager(gemini_settings());

    secrets::set_anthropic_api_key("sk-ant-test").unwrap();
    secrets::set_openai_compat_api_key("sk-openai-test").unwrap();
    assert!(
        !manager.provider_ready().await,
        "他社のキーでGeminiが使える判定になっている"
    );

    secrets::set_gemini_api_key("AIza-test").unwrap();
    assert!(manager.provider_ready().await, "キーがあるのに使えない判定");
    secrets::clear_gemini_api_key().unwrap();
    assert!(!manager.provider_ready().await, "削除後も使える判定のまま");
    // Geminiのキーを消してもほかのプロバイダのキーは残る
    assert!(secrets::has_anthropic_api_key());
    assert!(secrets::has_openai_compat_api_key());
}

/// Clearing the model box falls back to the recommended default model, so the provider stays usable.
/// モデル欄を空にすると推奨の既定モデルへ戻るため、そのまま使える状態が保たれる。
#[tokio::test]
async fn a_blank_model_box_falls_back_to_the_default_model() {
    let _store = secrets::install_store(Arc::new(MemoryStore::default()));
    secrets::set_gemini_api_key("AIza-test").unwrap();

    let blank = manager(AppSettings {
        gemini_model: "   ".into(),
        ..gemini_settings()
    });
    assert!(
        blank.provider_ready().await,
        "空欄は既定のモデルへ戻るので使える判定になること"
    );
}

/// While Gemini is chosen, the connection test goes to Gemini instead of to the Anthropic API.
/// Geminiを選んでいる間、接続テストはAnthropic APIではなくGeminiへ行く。
#[tokio::test]
async fn the_connection_test_follows_the_chosen_provider() {
    let _store = secrets::install_store(Arc::new(MemoryStore::default()));

    // キーを保存していないうちは通信せずに理由を返す
    let manager = manager(gemini_settings());
    let error = manager
        .test_connection()
        .await
        .expect_err("キーが無ければ接続テストは失敗する");
    assert_eq!(error.kind, "gemini_no_key", "理由の種類が違う: {error:?}");
    assert!(
        error.message.contains("APIキー"),
        "理由が読めない: {error:?}"
    );

    // キーを保存すれば実際に叩く(モックは200を返す)
    let base = mock_api(vec![
        json!({"candidates": [{"content": {"role": "model", "parts": [{"text": "pong"}]}}]}),
    ])
    .await;
    std::env::set_var(madake_agent::gemini::GEMINI_BASE_URL_ENV, &base);
    secrets::set_gemini_api_key("AIza-test").unwrap();
    assert_eq!(
        manager.test_connection().await.expect("疎通できる"),
        "gemini-2.5-flash",
        "確かめたモデル名を返す"
    );
    std::env::remove_var(madake_agent::gemini::GEMINI_BASE_URL_ENV);
}

/// Nothing about the Gemini key is ever broadcast to the UI event stream or written into the chat history.
/// GeminiのAPIキーはUIへ流れるイベントにも会話履歴にも一切載らない。
#[tokio::test]
async fn the_api_key_never_appears_in_the_event_stream() {
    const KEY: &str = "AIza-must-never-be-broadcast";
    let _store = secrets::install_store(Arc::new(MemoryStore::default()));
    let base = mock_api(hello_stream()).await;
    std::env::set_var(madake_agent::gemini::GEMINI_BASE_URL_ENV, &base);
    secrets::set_gemini_api_key(KEY).unwrap();

    let manager = manager(gemini_settings());
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
    std::env::remove_var(madake_agent::gemini::GEMINI_BASE_URL_ENV);
}
