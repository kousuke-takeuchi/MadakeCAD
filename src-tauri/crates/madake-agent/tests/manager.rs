//! AgentManagerの統合テスト。本物のclaudeは呼ばず、fixtureのフェイクCLIを使う。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use madake_agent::manager::{AgentManager, ConversationEvent, DocBridge};
use madake_agent::{AgentEvent, AppSettings, DocState};
use tokio::sync::broadcast::Receiver;
use uuid::Uuid;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// テスト用のドキュメント。実エンジンと同じく、編集はrevisionと深さの両方を進め、
/// undoはrevisionを進めつつ深さを1減らす。
///
/// `edits_per_event`を1以上にすると`state()`が読まれるたびに編集が起き、
/// 「エージェントがターン中にMCP経由で編集した」状況を再現できる。
/// `pending_undos`はその編集に混ぜるundoの残り回数(MCPのundoツール/ユーザーのUI undo)。
struct FakeDoc {
    revision: AtomicU64,
    undo_depth: AtomicU64,
    edits_per_event: u64,
    pending_undos: AtomicU64,
    undo_calls: AtomicU64,
    undo_fails: Mutex<Option<String>>,
    /// この回数だけ成功したあとundoを失敗させる(途中失敗の再現)。
    undo_fails_after: AtomicU64,
}

impl FakeDoc {
    fn new(edits_per_event: u64) -> Arc<Self> {
        Arc::new(Self {
            revision: AtomicU64::new(0),
            undo_depth: AtomicU64::new(0),
            edits_per_event,
            pending_undos: AtomicU64::new(0),
            undo_calls: AtomicU64::new(0),
            undo_fails: Mutex::new(None),
            undo_fails_after: AtomicU64::new(u64::MAX),
        })
    }

    /// 1コマンド実行(revision+1・深さ+1)。
    fn record_edit(&self) {
        self.revision.fetch_add(1, Ordering::SeqCst);
        self.undo_depth.fetch_add(1, Ordering::SeqCst);
    }

    /// 1回undo(revision+1・深さ-1)。戻せる編集が無ければ何もしない。
    fn record_undo(&self) -> bool {
        if self
            .undo_depth
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |d| d.checked_sub(1))
            .is_err()
        {
            return false;
        }
        self.revision.fetch_add(1, Ordering::SeqCst);
        true
    }

    fn snapshot(&self) -> DocState {
        DocState::new(
            self.revision.load(Ordering::SeqCst),
            self.undo_depth.load(Ordering::SeqCst),
        )
    }
}

impl DocBridge for FakeDoc {
    fn revision(&self) -> u64 {
        self.revision.load(Ordering::SeqCst)
    }

    fn undo_depth(&self) -> u64 {
        self.undo_depth.load(Ordering::SeqCst)
    }

    fn state(&self) -> DocState {
        for _ in 0..self.edits_per_event {
            self.record_edit();
        }
        if self
            .pending_undos
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
            .is_ok()
        {
            self.record_undo();
        }
        self.snapshot()
    }

    fn undo(&self) -> Result<bool, String> {
        if let Some(e) = self.undo_fails.lock().unwrap().clone() {
            return Err(e);
        }
        if self.undo_calls.load(Ordering::SeqCst) >= self.undo_fails_after.load(Ordering::SeqCst) {
            return Err("engine busy".to_string());
        }
        self.undo_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.record_undo())
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

/// ターンを1本受け切り、`TurnCompleted`の本文を返す
/// (`fake_claude_echo_args.sh`はCLIへ渡った引数をここへ載せる)。
async fn turn_result(rx: &mut Receiver<ConversationEvent>) -> String {
    collect_turn(rx)
        .await
        .iter()
        .filter_map(|e| match e {
            AgentEvent::TurnCompleted { result, .. } => Some(result.clone()),
            _ => None,
        })
        .collect()
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

/// Sending without a conversation id creates a conversation and broadcasts its events.
/// 会話IDなしの送信は会話を新規作成し、そのイベントを配信する。
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

/// A turn that edits the document records the applied revisions and emits a turn-applied event with the undo depth.
/// 図面を編集したターンは適用revisionを記録し、undo深さ付きのターン適用イベントを発行する。
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
        "ターン中の編集がundoスタック深さの増分として残る: {:?}",
        turn.applied_undo_depth
    );

    match events.last() {
        Some(AgentEvent::TurnApplied {
            start_revision,
            end_revision,
            start_undo_depth,
            end_undo_depth,
        }) => {
            assert_eq!(*start_revision, turn.applied_revisions.start);
            assert_eq!(*end_revision, turn.applied_revisions.end);
            assert_eq!(*start_undo_depth, turn.applied_undo_depth.start);
            assert_eq!(*end_undo_depth, turn.applied_undo_depth.end);
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

/// Turn undo counts are based on undo-stack growth, so user undos during the turn don't corrupt the count.
/// ターンのundo回数はundoスタックの増分基準で、ターン中のユーザーundoが数を狂わせない。
#[tokio::test]
async fn undo_turn_counts_stack_growth_not_revision_delta() {
    let doc = FakeDoc::new(1);
    // ターン開始前に「以前の編集」を2つ積んでおく(戻しすぎたらこれが消える)
    doc.record_edit();
    doc.record_edit();
    // ターン中に2回undoが混ざる
    doc.pending_undos.store(2, Ordering::SeqCst);

    let manager = manager(Arc::clone(&doc), "fake_claude.sh");
    let mut rx = manager.subscribe();
    let id = manager
        .send(None, "置いてから1つ戻して", None, None)
        .await
        .unwrap();
    collect_turn(&mut rx).await;

    let turn = manager.conversations()[0].last_turn().cloned().unwrap();
    let stack_growth = turn.applied_undo_depth.end - turn.applied_undo_depth.start;
    assert!(
        turn.applied_revisions.count() > stack_growth,
        "undoが混ざるとrevision差の方が大きくなる: {:?} / {:?}",
        turn.applied_revisions,
        turn.applied_undo_depth
    );
    assert_eq!(
        turn.applied_command_count(),
        stack_growth,
        "undo回数は深さ増分"
    );

    let depth_before = doc.undo_depth.load(Ordering::SeqCst);
    manager.undo_turn(id, 1).expect("undo成功");
    assert_eq!(
        doc.undo_calls.load(Ordering::SeqCst),
        stack_growth,
        "深さ増分ぶんだけundoされる"
    );
    assert_eq!(
        doc.undo_depth.load(Ordering::SeqCst),
        depth_before - stack_growth,
        "ターン開始前の編集は残る"
    );
    assert!(
        doc.undo_depth.load(Ordering::SeqCst) >= 2,
        "ターン外(開始前)の編集まで戻していない"
    );
}

/// Only the latest applied turn may be reverted; older targets are rejected (safety guard).
/// 巻き戻せるのは最新の適用済みターンのみで、古い対象は拒否される(安全ガード)。
#[tokio::test]
async fn undo_turn_rejects_targets_that_are_not_the_latest_applied_turn() {
    let doc = FakeDoc::new(1);
    let manager = manager(Arc::clone(&doc), "fake_claude.sh");
    let mut rx = manager.subscribe();
    let id = manager.send(None, "1回目", None, None).await.unwrap();
    collect_turn(&mut rx).await;
    let first_turn_index = 1;

    // 同じ会話で2ターン目(後続ターンに編集がある)
    manager.send(Some(id), "2回目", None, None).await.unwrap();
    collect_turn(&mut rx).await;
    let err = manager.undo_turn(id, first_turn_index).unwrap_err();
    assert!(
        matches!(err, madake_agent::AgentError::NotLatestTurn(_)),
        "{err}"
    );
    assert_eq!(doc.undo_calls.load(Ordering::SeqCst), 0, "undoは呼ばれない");

    // 最新ターンは戻せる
    let latest = manager.conversations()[0].messages.len() - 1;
    manager.undo_turn(id, latest).expect("最新ターンは戻せる");

    // ユーザーがUIで編集したあとは、その最新ターンも戻せない
    let id2 = manager.send(Some(id), "3回目", None, None).await.unwrap();
    collect_turn(&mut rx).await;
    let latest = manager.conversations()[0].messages.len() - 1;
    doc.record_edit();
    let err = manager.undo_turn(id2, latest).unwrap_err();
    assert!(
        matches!(err, madake_agent::AgentError::NotLatestTurn(_)),
        "ターン後のユーザー編集が積まれていたら拒否: {err}"
    );
}

/// A running turn cannot be reverted.
/// 実行中のターンは巻き戻せない。
#[tokio::test]
async fn undo_turn_rejects_a_running_turn() {
    let doc = FakeDoc::new(0);
    let manager = manager(Arc::clone(&doc), "fake_claude_flood.sh");
    let mut rx = manager.subscribe();
    let id = manager.send(None, "hi", None, None).await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), rx.recv())
        .await
        .expect("イベントが来ない")
        .unwrap();

    let err = manager.undo_turn(id, 1).unwrap_err();
    assert!(matches!(err, madake_agent::AgentError::Busy), "{err}");
    manager.cancel(id);
}

/// If undo fails midway, the partial progress is recorded so the state stays truthful.
/// undoが途中で失敗した場合も進んだ分だけ記録し、状態を正直に保つ。
#[tokio::test]
async fn undo_turn_records_partial_progress_when_undo_fails_midway() {
    let doc = FakeDoc::new(1);
    let manager = manager(Arc::clone(&doc), "fake_claude.sh");
    let mut rx = manager.subscribe();
    let id = manager
        .send(None, "いくつか置いて", None, None)
        .await
        .unwrap();
    collect_turn(&mut rx).await;

    let total = manager.conversations()[0]
        .last_turn()
        .unwrap()
        .applied_command_count();
    assert!(total >= 2, "複数コマンド積まれている前提: {total}");

    // 1回成功したところで失敗させる
    doc.undo_fails_after.store(1, Ordering::SeqCst);
    let err = manager.undo_turn(id, 1).unwrap_err().to_string();
    assert!(err.contains("engine busy"), "{err}");
    assert_eq!(doc.undo_calls.load(Ordering::SeqCst), 1, "成功したのは1回");
    assert_eq!(
        manager.conversations()[0]
            .last_turn()
            .unwrap()
            .applied_command_count(),
        total - 1,
        "残り回数だけが記録に残る(リトライしても戻しすぎない)"
    );

    // 復旧後のリトライは残り回数だけ実行する
    doc.undo_fails_after.store(u64::MAX, Ordering::SeqCst);
    manager.undo_turn(id, 1).expect("リトライ成功");
    assert_eq!(
        doc.undo_calls.load(Ordering::SeqCst),
        total,
        "合計で積まれた分だけ"
    );
    assert_eq!(
        manager.conversations()[0]
            .last_turn()
            .unwrap()
            .applied_command_count(),
        0
    );
}

/// Document errors and unknown turn targets are reported as distinct errors.
/// ドキュメントエラーと不明なターン指定は区別されたエラーとして報告される。
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

/// Sending to a conversation that is already running a turn is rejected.
/// ターン実行中の会話への追加送信は拒否される。
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

/// Cancel kills the CLI process and marks the message as cancelled.
/// キャンセルはCLIプロセスを停止し、メッセージをキャンセル済みにする。
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

/// Sending to an unknown conversation id fails cleanly.
/// 不明な会話IDへの送信は明確に失敗する。
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

/// The drawing context and selected model are forwarded to the CLI invocation.
/// 図面コンテキストと選択モデルはCLI起動へ引き渡される。
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
    let text = turn_result(&mut rx).await;
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

/// Turning off auto-read-drawing suppresses the drawing context.
/// 図面自動読み取りをオフにすると図面コンテキストは付かない。
#[tokio::test]
async fn auto_read_drawing_off_suppresses_the_drawing_context() {
    let doc = FakeDoc::new(0);
    let manager = manager(doc, "fake_claude_echo_args.sh");
    let mut rx = manager.subscribe();

    manager.apply_settings(AppSettings {
        claude_path: Some(fixtures_dir().join("fake_claude_echo_args.sh")),
        auto_apply: true,
        auto_read_drawing: false,
        language: "en".into(),
    });
    manager
        .send(None, "hi", None, Some("アクティブシート: S1".to_string()))
        .await
        .unwrap();
    assert!(
        !turn_result(&mut rx)
            .await
            .contains("--append-system-prompt"),
        "自動読み取りOFFでは図面コンテキストを渡さない"
    );

    // 設定を戻せば、そのまま次の送信から復活する
    manager.apply_settings(AppSettings {
        claude_path: Some(fixtures_dir().join("fake_claude_echo_args.sh")),
        auto_apply: true,
        auto_read_drawing: true,
        language: "en".into(),
    });
    manager
        .send(None, "hi", None, Some("アクティブシート: S1".to_string()))
        .await
        .unwrap();
    assert!(turn_result(&mut rx)
        .await
        .contains("--append-system-prompt=アクティブシート: S1"));
}

/// The claude-path setting overrides which executable the backend runs.
/// claude実行ファイルパス設定がバックエンドの実行ファイルを上書きする。
#[tokio::test]
async fn claude_path_setting_becomes_the_backend_executable() {
    let doc = FakeDoc::new(0);
    // set_executableは使わず、設定だけで実行パスを与える
    let manager = Arc::new(AgentManager::new(doc, 9310));
    let mut rx = manager.subscribe();
    manager.apply_settings(AppSettings {
        claude_path: Some(fixtures_dir().join("fake_claude_echo_args.sh")),
        ..AppSettings::default()
    });
    assert_eq!(
        manager.executable(),
        Some(fixtures_dir().join("fake_claude_echo_args.sh"))
    );

    manager
        .send(None, "hi", Some("claude-opus-4-6".to_string()), None)
        .await
        .unwrap();
    assert!(turn_result(&mut rx)
        .await
        .contains("--model=claude-opus-4-6"));

    // パスを消すと自動検出へ戻る
    manager.apply_settings(AppSettings::default());
    assert_eq!(manager.executable(), None);
    assert_eq!(manager.settings(), AppSettings::default());

    // claude_pathなしの設定保存(トグル変更等)は検出済みパスのキャッシュを消さない
    let cached = fixtures_dir().join("fake_claude.sh");
    manager.set_executable(Some(cached.clone()));
    manager.apply_settings(AppSettings {
        auto_read_drawing: false,
        ..AppSettings::default()
    });
    assert_eq!(manager.executable(), Some(cached));
}

/// Replacing the conversation history (project load) cancels any running turn first.
/// 会話履歴の置き換え(プロジェクト読込)は実行中ターンを先にキャンセルする。
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
