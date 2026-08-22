//! プロバイダ切り替え(GitHub Copilot CLI)のテスト。
//!
//! 狙いは受け入れ基準そのもの: **プロバイダで「GitHub Copilot CLI」を選ぶだけで
//! チャットのやりとりが動く**(Anthropicのキーもclaude CLIも要らない)。
//! 本物のcopilotは呼ばず、JSONLを吐くフェイクCLIへ向ける。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use madake_agent::manager::{
    AgentManager, ConversationEvent, DocBridge, RevertError, RevertReport,
};
use madake_agent::{AgentEvent, AgentProvider, AppSettings};
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

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn copilot_settings(script: &str) -> AppSettings {
    AppSettings {
        provider: AgentProvider::CopilotCli,
        copilot_path: Some(fixture(script)),
        // 図面コンテキストは今回関係ないので切っておく
        auto_read_drawing: false,
        ..AppSettings::default()
    }
}

fn manager(script: &str) -> Arc<AgentManager> {
    let manager = Arc::new(AgentManager::new(Arc::new(QuietDoc::default()), 9310));
    manager.apply_settings(copilot_settings(script));
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

/// With GitHub Copilot selected, a chat turn runs through the Copilot CLI without any Anthropic key.
/// GitHub Copilotを選んでおけば、Anthropicのキーが無くてもCopilot CLI経由でチャットのやりとりができる。
#[tokio::test]
async fn a_turn_runs_through_the_copilot_cli() {
    let manager = manager("fake_copilot.sh");
    let mut rx = manager.subscribe();

    manager
        .send(None, "端子台を置いて", None, None)
        .await
        .expect("Copilotが入っていれば送信できる");
    let events = collect_turn(&mut rx).await;

    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::TextDelta { text } if text.contains("端子台"))),
        "Copilotからの本文が届いていない: {events:?}"
    );
    assert!(
        events.iter().any(
            |e| matches!(e, AgentEvent::ToolUseStarted { tool, .. } if tool.contains("madakecad"))
        ),
        "MadakeCADのMCPツール呼び出しが届いていない: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::TurnCompleted { .. })),
        "ターンが完了していない: {events:?}"
    );
}

/// The session id chosen for the first turn is remembered, so the next turn continues the same Copilot session.
/// 最初のターンで決めたセッションIDは記憶され、次のターンは同じCopilotセッションの続きになる。
#[tokio::test]
async fn the_session_is_remembered_for_the_next_turn() {
    let manager = manager("fake_copilot.sh");
    let mut rx = manager.subscribe();

    let conversation = manager
        .send(None, "端子台を置いて", None, None)
        .await
        .expect("送信できる");
    collect_turn(&mut rx).await;

    let session = manager
        .conversations()
        .into_iter()
        .find(|c| c.id == conversation)
        .and_then(|c| c.session_id);
    let session = session.expect("セッションIDが記録されていない");
    uuid::Uuid::parse_str(&session).expect("セッションIDはUUID");
}

/// The provider badge reports "ready" only while the Copilot CLI can actually be found.
/// プロバイダのバッジは、Copilot CLIが実際に見つかるときだけ「使える」状態になる。
#[tokio::test]
async fn the_provider_is_ready_only_while_copilot_is_found() {
    assert!(
        manager("fake_copilot.sh").provider_ready().await,
        "見つかるのに使えない判定"
    );
    assert!(
        !manager("no_such_copilot").provider_ready().await,
        "見つからないのに使える判定"
    );
}

/// If the configured Copilot executable does not exist, the chat shows why instead of failing silently.
/// 設定したCopilotの実行ファイルが無い場合、黙って失敗せず理由がチャットに表示される。
#[tokio::test]
async fn a_missing_copilot_executable_is_explained_in_the_chat() {
    let manager = manager("no_such_copilot");
    let mut rx = manager.subscribe();

    manager
        .send(None, "こんにちは", None, None)
        .await
        .expect("送信自体は始まる");
    let events = collect_turn(&mut rx).await;

    match events.last() {
        Some(AgentEvent::Error { message }) => {
            assert!(
                message.contains("no_such_copilot"),
                "どのパスか分かること: {message}"
            );
        }
        other => panic!("最後はErrorのはず: {other:?}"),
    }
}

/// The connection test goes to Copilot (not to the Anthropic API) while Copilot is the chosen provider.
/// Copilotを選んでいる間、接続テストはAnthropic APIではなくCopilotへ行く。
#[tokio::test]
async fn the_connection_test_follows_the_chosen_provider() {
    let ok = manager("fake_copilot.sh")
        .test_connection()
        .await
        .expect("応答すれば成功");
    assert_eq!(ok, "auto", "確かめたモデルを返す");

    let error = manager("fake_copilot_unauth.sh")
        .test_connection()
        .await
        .expect_err("未認証は失敗になる");
    assert_eq!(error.kind, "copilot_auth");
    assert!(error.message.contains("/login"), "{}", error.message);
}
