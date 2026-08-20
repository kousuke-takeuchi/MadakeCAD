//! AgentManagerの統合テスト。本物のclaudeは呼ばず、fixtureのフェイクCLIを使う。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use madake_agent::manager::{AgentManager, ConversationEvent, DocBridge};
use madake_agent::AgentEvent;
use tokio::sync::broadcast::Receiver;
use uuid::Uuid;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// テスト用のドキュメント。
///
/// `edits_per_event`を1以上にすると`revision()`が呼ばれるたびにrevisionが進み、
/// 「エージェントがターン中にMCP経由で編集した」状況を再現できる。
struct FakeDoc {
    revision: AtomicU64,
    edits_per_event: u64,
    undo_calls: AtomicU64,
    undo_fails: Mutex<Option<String>>,
}

impl FakeDoc {
    fn new(edits_per_event: u64) -> Arc<Self> {
        Arc::new(Self {
            revision: AtomicU64::new(0),
            edits_per_event,
            undo_calls: AtomicU64::new(0),
            undo_fails: Mutex::new(None),
        })
    }
}

impl DocBridge for FakeDoc {
    fn revision(&self) -> u64 {
        self.revision
            .fetch_add(self.edits_per_event, Ordering::SeqCst)
    }

    fn undo(&self) -> Result<bool, String> {
        if let Some(e) = self.undo_fails.lock().unwrap().clone() {
            return Err(e);
        }
        self.undo_calls.fetch_add(1, Ordering::SeqCst);
        // 実エンジンと同じくundoもrevisionを進める
        self.revision.fetch_add(1, Ordering::SeqCst);
        Ok(true)
    }
}

fn manager(doc: Arc<FakeDoc>, script: &str) -> Arc<AgentManager> {
    let manager = Arc::new(AgentManager::new(doc, 9310));
    manager.set_executable(Some(fixtures_dir().join(script)));
    manager
}

/// ターンが終わる(TurnCompleted/Errorが来て、以降のTurnAppliedも受け切る)まで集める。
async fn collect_turn(rx: &mut Receiver<ConversationEvent>) -> Vec<AgentEvent> {
    let mut events = Vec::new();
    let deadline = Duration::from_secs(10);
    loop {
        let next = tokio::time::timeout(deadline, rx.recv())
            .await
            .expect("イベント待ちがタイムアウト")
            .expect("配信チャネルが閉じた");
        let terminal = matches!(
            next.event,
            AgentEvent::TurnCompleted { .. } | AgentEvent::Error { .. }
        );
        events.push(next.event);
        if terminal {
            // 合成イベントTurnAppliedは終了イベントの直後(無い場合もある)
            if let Ok(Ok(extra)) = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await {
                events.push(extra.event);
            }
            return events;
        }
    }
}

fn kinds(events: &[AgentEvent]) -> Vec<&'static str> {
    events
        .iter()
        .map(|e| match e {
            AgentEvent::SessionStarted { .. } => "session",
            AgentEvent::TextDelta { .. } => "text",
            AgentEvent::ToolUseStarted { .. } => "tool_start",
            AgentEvent::ToolUseFinished { .. } => "tool_end",
            AgentEvent::TurnCompleted { .. } => "completed",
            AgentEvent::TurnApplied { .. } => "applied",
            AgentEvent::Error { .. } => "error",
        })
        .collect()
}

#[tokio::test]
async fn send_creates_conversation_and_broadcasts_events() {
    let doc = FakeDoc::new(0);
    let manager = manager(doc, "fake_claude.sh");
    let mut rx = manager.subscribe();

    let id = manager
        .send(None, "hi", None, None)
        .await
        .expect("送信成功");
    let events = collect_turn(&mut rx).await;

    // 編集が無いターンではTurnAppliedは流れない
    assert_eq!(
        kinds(&events),
        vec!["session", "tool_start", "tool_end", "text", "completed"]
    );

    let conversations = manager.conversations();
    assert_eq!(conversations.len(), 1);
    let conversation = &conversations[0];
    assert_eq!(conversation.id, id);
    assert_eq!(
        conversation.session_id.as_deref(),
        Some("39628e1e-925e-42e5-9619-7cda7c2671f1")
    );
    assert_eq!(conversation.messages.len(), 2, "user + assistant");
    assert_eq!(conversation.messages[0].text, "hi");
    let turn = conversation.last_turn().unwrap();
    assert!(!turn.text.is_empty(), "本文が入る");
    assert_eq!(turn.tool_calls.len(), 1);
    assert!(turn.tool_calls[0].finished);
    assert_eq!(turn.applied_command_count(), 0);
}

#[tokio::test]
async fn turn_records_applied_revisions_and_emits_turn_applied() {
    let doc = FakeDoc::new(1);
    let manager = manager(Arc::clone(&doc), "fake_claude.sh");
    let mut rx = manager.subscribe();

    let id = manager
        .send(None, "ヒューズを追加して", None, None)
        .await
        .unwrap();
    let events = collect_turn(&mut rx).await;

    let turn = manager.conversations()[0].last_turn().cloned().unwrap();
    assert!(
        turn.applied_command_count() > 0,
        "ターン中の編集がrevision差として残る: {:?}",
        turn.applied_revisions
    );

    match events.last() {
        Some(AgentEvent::TurnApplied {
            start_revision,
            end_revision,
        }) => {
            assert_eq!(*start_revision, turn.applied_revisions.start);
            assert_eq!(*end_revision, turn.applied_revisions.end);
        }
        other => panic!("最後はTurnAppliedのはず: {other:?}"),
    }

    // 「元に戻す」= このターンのコマンド数だけundo
    let expected_undos = turn.applied_command_count();
    let assistant_index = 1;
    let revision = manager.undo_turn(id, assistant_index).expect("undo成功");
    assert_eq!(doc.undo_calls.load(Ordering::SeqCst), expected_undos);
    assert!(revision > 0);
    assert_eq!(
        manager.conversations()[0]
            .last_turn()
            .unwrap()
            .applied_command_count(),
        0,
        "巻き戻し後は適用済みではなくなる"
    );
}

#[tokio::test]
async fn undo_turn_reports_doc_errors_and_unknown_targets() {
    let doc = FakeDoc::new(1);
    let manager = manager(Arc::clone(&doc), "fake_claude.sh");
    let mut rx = manager.subscribe();
    let id = manager.send(None, "hi", None, None).await.unwrap();
    collect_turn(&mut rx).await;

    assert!(manager.undo_turn(Uuid::new_v4(), 1).is_err(), "未知の会話");
    assert!(manager.undo_turn(id, 99).is_err(), "範囲外のメッセージ");
    assert!(manager.undo_turn(id, 0).is_err(), "ユーザー発話は対象外");

    *doc.undo_fails.lock().unwrap() = Some("engine busy".to_string());
    let err = manager.undo_turn(id, 1).unwrap_err().to_string();
    assert!(err.contains("engine busy"), "{err}");
}

#[tokio::test]
async fn second_send_while_running_is_rejected() {
    let doc = FakeDoc::new(0);
    let manager = manager(doc, "fake_claude_flood.sh");
    let mut rx = manager.subscribe();

    let id = manager.send(None, "hi", None, None).await.unwrap();
    // ストリームが始まるまで待つ
    tokio::time::timeout(Duration::from_secs(10), rx.recv())
        .await
        .expect("イベントが来ない")
        .unwrap();

    let err = manager
        .send(Some(id), "もう1回", None, None)
        .await
        .unwrap_err();
    assert!(matches!(err, madake_agent::AgentError::Busy), "{err}");

    manager.cancel(id);
}

#[tokio::test]
async fn cancel_stops_the_turn_and_marks_the_message() {
    let doc = FakeDoc::new(0);
    let manager = manager(doc, "fake_claude_flood.sh");
    let mut rx = manager.subscribe();

    let id = manager.send(None, "hi", None, None).await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), rx.recv())
        .await
        .expect("イベントが来ない")
        .unwrap();
    assert!(manager.is_sending(id));

    assert!(manager.cancel(id), "中断できる");
    assert!(!manager.is_sending(id), "送信中フラグが下りる");
    assert!(!manager.cancel(id), "2回目は何もしない");

    let turn = manager.conversations()[0].last_turn().cloned().unwrap();
    assert_eq!(
        turn.error.as_deref(),
        Some(madake_agent::manager::CANCELLED_MESSAGE)
    );

    // 中断イベントが購読者にも流れる(受信済みのデルタを読み飛ばして探す)
    let mut saw_cancel = false;
    for _ in 0..2000 {
        match tokio::time::timeout(Duration::from_secs(5), rx.recv()).await {
            Ok(Ok(ConversationEvent {
                event: AgentEvent::Error { message },
                ..
            })) => {
                assert!(message.contains("キャンセル"), "{message}");
                saw_cancel = true;
                break;
            }
            Ok(Ok(_)) => continue,
            _ => break,
        }
    }
    assert!(saw_cancel, "キャンセルのErrorイベントが流れること");
}

#[tokio::test]
async fn send_to_unknown_conversation_fails() {
    let doc = FakeDoc::new(0);
    let manager = manager(doc, "fake_claude.sh");
    let err = manager
        .send(Some(Uuid::new_v4()), "hi", None, None)
        .await
        .unwrap_err();
    assert!(matches!(err, madake_agent::AgentError::NoConversation(_)));
    assert!(manager.conversations().is_empty(), "会話は作られない");
}

/// 図面コンテキストは`--append-system-prompt`としてCLIへ渡る。
#[tokio::test]
async fn context_and_model_are_forwarded_to_the_cli() {
    let doc = FakeDoc::new(0);
    let manager = manager(doc, "fake_claude_echo_args.sh");
    let mut rx = manager.subscribe();

    manager
        .send(
            None,
            "hi",
            Some("claude-opus-4-6".to_string()),
            Some("アクティブシート: S1".to_string()),
        )
        .await
        .unwrap();
    let events = collect_turn(&mut rx).await;

    let text: String = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::TurnCompleted { result, .. } => Some(result.clone()),
            _ => None,
        })
        .collect();
    assert!(text.contains("--model=claude-opus-4-6"), "{text}");
    assert!(
        text.contains("--append-system-prompt=アクティブシート: S1"),
        "{text}"
    );
    assert_eq!(
        manager.conversations()[0].model.as_deref(),
        Some("claude-opus-4-6")
    );
}

/// 会話の差し替え(プロジェクト読込)で履歴が復元され、送信中ターンは中断される。
#[tokio::test]
async fn set_conversations_replaces_history_and_cancels_running_turn() {
    let doc = FakeDoc::new(0);
    let manager = manager(doc, "fake_claude_flood.sh");
    let mut rx = manager.subscribe();
    let id = manager.send(None, "hi", None, None).await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), rx.recv())
        .await
        .expect("イベントが来ない")
        .unwrap();

    let restored = madake_agent::Conversation::new();
    let restored_id = restored.id;
    manager.set_conversations(vec![restored]);

    assert!(!manager.is_sending(id));
    let conversations = manager.conversations();
    assert_eq!(conversations.len(), 1);
    assert_eq!(conversations[0].id, restored_id);
}
