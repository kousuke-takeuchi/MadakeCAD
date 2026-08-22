//! AgentManagerの統合テスト。本物のclaudeは呼ばず、fixtureのフェイクCLIを使う。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use madake_agent::manager::{AgentManager, ConversationEvent, DocBridge, RevertError, RevertReport};
use madake_agent::{AgentEvent, AppSettings, DocState};
use tokio::sync::broadcast::Receiver;
use uuid::Uuid;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// undo履歴の1エントリ(テスト用のミラー)。
#[derive(Debug, Clone, Copy, PartialEq)]
struct Edit {
    /// エージェントのターン実行中に入った編集か(そうでなければユーザーの手編集)
    agent: bool,
    /// 巻き戻し済みか
    reverted: bool,
}

/// テスト用のドキュメント。実エンジンと同じく編集はundo履歴に積まれ、
/// 由来(エージェント/ユーザー)を持つ。巻き戻しは区間内のエージェント編集だけを戻し、
/// 戻した分は新しい編集として1件積む(逆Command適用のミラー)。
///
/// `edits_per_event`を1以上にすると`state()`が読まれるたびに編集が起き、
/// 「エージェントがターン中にMCP経由で編集した」状況を再現できる。
/// `pending_undos`はその編集に混ぜるundoの残り回数(MCPのundoツール/ユーザーのUI undo)。
struct FakeDoc {
    revision: AtomicU64,
    stack: Mutex<Vec<Edit>>,
    edits_per_event: u64,
    pending_undos: AtomicU64,
    /// エージェントのターン実行中(begin/end_agent_turnの入れ子数)
    agent_turns: AtomicU64,
    revert_calls: AtomicU64,
    /// 次の巻き戻しを失敗させる(衝突・エラーの再現)
    revert_error: Mutex<Option<RevertError>>,
}

impl FakeDoc {
    fn new(edits_per_event: u64) -> Arc<Self> {
        Arc::new(Self {
            revision: AtomicU64::new(0),
            stack: Mutex::new(Vec::new()),
            edits_per_event,
            pending_undos: AtomicU64::new(0),
            agent_turns: AtomicU64::new(0),
            revert_calls: AtomicU64::new(0),
            revert_error: Mutex::new(None),
        })
    }

    /// 1コマンド実行(revision+1・履歴+1)。ターン実行中ならエージェント編集になる。
    fn record_edit(&self) {
        let agent = self.agent_turns.load(Ordering::SeqCst) > 0;
        self.stack.lock().unwrap().push(Edit {
            agent,
            reverted: false,
        });
        self.revision.fetch_add(1, Ordering::SeqCst);
    }

    /// ユーザーがUIで1コマンド編集した(ターン中でもエージェント編集にはならない)。
    fn record_user_edit(&self) {
        self.stack.lock().unwrap().push(Edit {
            agent: false,
            reverted: false,
        });
        self.revision.fetch_add(1, Ordering::SeqCst);
    }

    /// 1回undo(revision+1・履歴-1)。戻せる編集が無ければ何もしない。
    fn record_undo(&self) -> bool {
        if self.stack.lock().unwrap().pop().is_none() {
            return false;
        }
        self.revision.fetch_add(1, Ordering::SeqCst);
        true
    }

    fn depth(&self) -> u64 {
        self.stack.lock().unwrap().len() as u64
    }

    /// 生きている(巻き戻されていない)エージェント編集の数。
    fn live_agent_edits(&self) -> usize {
        self.stack
            .lock()
            .unwrap()
            .iter()
            .filter(|e| e.agent && !e.reverted)
            .count()
    }

    /// 生きているユーザー編集の数。
    fn live_user_edits(&self) -> usize {
        self.stack
            .lock()
            .unwrap()
            .iter()
            .filter(|e| !e.agent && !e.reverted)
            .count()
    }

    fn snapshot(&self) -> DocState {
        DocState::new(self.revision.load(Ordering::SeqCst), self.depth())
    }
}

impl DocBridge for FakeDoc {
    fn revision(&self) -> u64 {
        self.revision.load(Ordering::SeqCst)
    }

    fn undo_depth(&self) -> u64 {
        self.depth()
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

    fn begin_agent_turn(&self) {
        self.agent_turns.fetch_add(1, Ordering::SeqCst);
    }

    fn end_agent_turn(&self) {
        let _ = self
            .agent_turns
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1));
    }

    fn revert_agent_edits(
        &self,
        start_depth: u64,
        end_depth: u64,
    ) -> Result<RevertReport, RevertError> {
        self.revert_calls.fetch_add(1, Ordering::SeqCst);
        if let Some(e) = self.revert_error.lock().unwrap().clone() {
            return Err(e);
        }
        let mut stack = self.stack.lock().unwrap();
        let end = (end_depth as usize).min(stack.len());
        let start = (start_depth as usize).min(end);
        let mut reverted = 0u64;
        for edit in stack[start..end].iter_mut() {
            if edit.agent && !edit.reverted {
                edit.reverted = true;
                reverted += 1;
            }
        }
        if reverted > 0 {
            // 巻き戻しも新しい編集として履歴に積まれる(ユーザー操作)
            stack.push(Edit {
                agent: false,
                reverted: false,
            });
            self.revision.fetch_add(1, Ordering::SeqCst);
        }
        Ok(RevertReport {
            reverted,
            revision: self.revision.load(Ordering::SeqCst),
        })
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

/// 「エージェントのターン実行中」フラグが下りる(ガードがdropされる)まで待つ。
async fn wait_agent_idle(doc: &FakeDoc) {
    for _ in 0..200 {
        if doc.agent_turns.load(Ordering::SeqCst) == 0 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("ターン実行中フラグが下りない");
}

/// ターンを1本受け切り、各イベントに載っていたターン通し番号を返す。
async fn collect_turn_seqs(rx: &mut Receiver<ConversationEvent>) -> Vec<u64> {
    let mut seqs = Vec::new();
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
        seqs.push(next.turn_seq);
        if terminal {
            if let Ok(Ok(extra)) = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await {
                seqs.push(extra.turn_seq);
            }
            return seqs;
        }
    }
}

/// ターンを1本受け切り、`TurnCompleted`の本文を返す
/// (`fake_claude_probe_prompt.sh`はCLIへ渡った引数の検査結果をここへ載せる)。
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

/// 会話(1本目)のターンID一覧(アシスタント応答の順)。巻き戻し対象の指定に使う。
fn turn_ids(manager: &AgentManager) -> Vec<Uuid> {
    manager.conversations()[0]
        .messages
        .iter()
        .filter(|m| m.role == madake_agent::Role::Assistant)
        .map(|m| m.turn_id)
        .collect()
}

/// 会話(1本目)の最新ターンID。
fn last_turn_id(manager: &AgentManager) -> Uuid {
    manager.conversations()[0].last_turn().unwrap().turn_id
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
            turn_id,
            start_revision,
            end_revision,
            start_undo_depth,
            end_undo_depth,
        }) => {
            assert_eq!(*turn_id, turn.turn_id, "巻き戻し対象のターンIDが載る");
            assert_eq!(*start_revision, turn.applied_revisions.start);
            assert_eq!(*end_revision, turn.applied_revisions.end);
            assert_eq!(*start_undo_depth, turn.applied_undo_depth.start);
            assert_eq!(*end_undo_depth, turn.applied_undo_depth.end);
        }
        other => panic!("最後はTurnAppliedのはず: {other:?}"),
    }

    // 「元に戻す」= このターンのエージェント編集を逆Commandで戻す
    assert!(doc.live_agent_edits() > 0);
    let revision = manager.undo_turn(id, turn.turn_id).expect("undo成功");
    assert_eq!(doc.live_agent_edits(), 0, "ターンの編集が全て戻る");
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

/// Edits made while a turn is running count as agent edits, and edits outside a turn stay user edits.
/// ターン実行中に入った編集はエージェント編集として記録され、ターン外の編集はユーザー編集のままになる。
#[tokio::test]
async fn edits_during_a_turn_are_recorded_as_agent_edits() {
    let doc = FakeDoc::new(1);
    // ターン開始前のユーザー編集
    doc.record_edit();
    assert_eq!(doc.live_agent_edits(), 0, "ターン外はユーザー編集");

    let manager = manager(Arc::clone(&doc), "fake_claude.sh");
    let mut rx = manager.subscribe();
    manager.send(None, "置いて", None, None).await.unwrap();
    collect_turn(&mut rx).await;
    assert!(doc.live_agent_edits() > 0, "ターン中の編集はエージェント編集");

    // ターンが終わればフラグは下りる(次の手編集はユーザー編集)
    let agent_before = doc.live_agent_edits();
    wait_agent_idle(&doc).await;
    doc.record_edit();
    assert_eq!(
        doc.live_agent_edits(),
        agent_before,
        "ターン終了後の編集はエージェント編集に数えない"
    );
}

/// Rolling back a turn reverts only the agent's edits and keeps the user's manual edits, even those made during the turn.
/// ターンの巻き戻しはエージェントの編集だけを戻し、ターン中に入れたものも含めてユーザーの手編集は残す。
#[tokio::test]
async fn undo_turn_reverts_only_the_agent_edits_of_the_turn() {
    let doc = FakeDoc::new(1);
    // ターン開始前に「以前の編集」を2つ積んでおく(戻しすぎたらこれが消える)
    doc.record_user_edit();
    doc.record_user_edit();
    // ターン中に2回undoが混ざる(エージェントのundoツール/ユーザーのUI undo)
    doc.pending_undos.store(2, Ordering::SeqCst);

    let manager = manager(Arc::clone(&doc), "fake_claude.sh");
    let mut rx = manager.subscribe();
    let id = manager
        .send(None, "置いてから1つ戻して", None, None)
        .await
        .unwrap();
    collect_turn(&mut rx).await;
    // ターンの最中にユーザーが手で1コマンド編集した
    doc.record_user_edit();

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
        "ターンの区間は深さ増分"
    );

    let user_before = doc.live_user_edits();
    manager
        .undo_turn(id, last_turn_id(&manager))
        .expect("undo成功");
    assert_eq!(doc.live_agent_edits(), 0, "エージェント編集は全て戻る");
    assert_eq!(
        doc.live_user_edits(),
        user_before + 1,
        "ユーザーの手編集は1件も戻さない(+1は巻き戻し操作そのもの)"
    );
}

/// An older turn can still be rolled back later; edits made after it are kept.
/// 古いターンも後から巻き戻せて、そのあとに入った編集は保持される。
#[tokio::test]
async fn undo_turn_can_revert_an_older_turn_and_keeps_later_edits() {
    let doc = FakeDoc::new(1);
    let manager = manager(Arc::clone(&doc), "fake_claude.sh");
    let mut rx = manager.subscribe();
    let id = manager.send(None, "1回目", None, None).await.unwrap();
    collect_turn(&mut rx).await;
    let first_turn = last_turn_id(&manager);
    let first_count = manager.conversations()[0]
        .turn(first_turn)
        .unwrap()
        .applied_command_count() as usize;

    // 同じ会話で2ターン目 + ユーザーの手編集
    manager.send(Some(id), "2回目", None, None).await.unwrap();
    collect_turn(&mut rx).await;
    doc.record_user_edit();
    let second_turn = last_turn_id(&manager);
    let agent_before = doc.live_agent_edits();
    let user_before = doc.live_user_edits();

    // 1ターン目(古いターン)を戻す
    manager.undo_turn(id, first_turn).expect("古いターンも戻せる");
    assert_eq!(
        doc.live_agent_edits(),
        agent_before - first_count,
        "1ターン目の編集だけが戻る(2ターン目は残る)"
    );
    assert_eq!(
        doc.live_user_edits(),
        user_before + 1,
        "ユーザーの手編集は残る(+1は巻き戻し操作)"
    );

    // 2ターン目もそのまま戻せる
    manager.undo_turn(id, second_turn).expect("2ターン目も戻せる");
    assert_eq!(doc.live_agent_edits(), 0);
}

/// A rollback that conflicts with a manual edit is refused, the turn stays applied, and it can be retried later.
/// 手編集と衝突する巻き戻しは拒否され、ターンは適用済みのまま残るので、後から再試行できる。
#[tokio::test]
async fn undo_turn_reports_a_conflict_and_keeps_the_turn_rollbackable() {
    let doc = FakeDoc::new(1);
    let manager = manager(Arc::clone(&doc), "fake_claude.sh");
    let mut rx = manager.subscribe();
    let id = manager.send(None, "置いて", None, None).await.unwrap();
    collect_turn(&mut rx).await;
    let turn = last_turn_id(&manager);
    let applied = doc.live_agent_edits();

    *doc.revert_error.lock().unwrap() =
        Some(RevertError::Conflict("entity not found: ...".into()));
    let err = manager.undo_turn(id, turn).unwrap_err();
    assert!(
        matches!(err, madake_agent::AgentError::TurnConflict { turn_id, .. } if turn_id == turn),
        "{err}"
    );
    assert!(
        err.to_string().contains("図面は変更していません"),
        "何も変わっていないことが伝わる: {err}"
    );
    assert_eq!(doc.live_agent_edits(), applied, "図面は無変更");
    assert!(
        manager.conversations()[0].turn(turn).unwrap().has_edits(),
        "適用済みのまま(再試行できる)"
    );

    // 衝突が解消すればそのまま戻せる
    *doc.revert_error.lock().unwrap() = None;
    manager.undo_turn(id, turn).expect("リトライ成功");
    assert_eq!(doc.live_agent_edits(), 0);
    assert!(!manager.conversations()[0].turn(turn).unwrap().has_edits());
}

/// A turn id keeps pointing at the same turn even after later turns are appended.
/// ターンIDは後続ターンが積まれても同じターンを指し続ける(添字と違いズレない)。
#[tokio::test]
async fn a_turn_id_keeps_addressing_the_same_turn_after_more_turns() {
    let doc = FakeDoc::new(1);
    let manager = manager(Arc::clone(&doc), "fake_claude.sh");
    let mut rx = manager.subscribe();
    let id = manager.send(None, "1回目", None, None).await.unwrap();
    collect_turn(&mut rx).await;
    let first_turn = last_turn_id(&manager);

    manager.send(Some(id), "2回目", None, None).await.unwrap();
    collect_turn(&mut rx).await;
    let ids = turn_ids(&manager);
    assert_eq!(ids.len(), 2, "2ターン分");
    assert_eq!(ids[0], first_turn, "1ターン目のIDは変わらない");
    assert_ne!(ids[0], ids[1]);

    // 2ターン目のIDで戻すと、その2ターン目の編集だけが戻る(1ターン目の編集は残る)
    let second_count = manager.conversations()[0]
        .turn(ids[1])
        .unwrap()
        .applied_command_count() as usize;
    let agent_before = doc.live_agent_edits();
    manager.undo_turn(id, ids[1]).expect("2ターン目を戻す");
    assert_eq!(
        doc.live_agent_edits(),
        agent_before - second_count,
        "1ターン目の編集は残る"
    );
    assert!(
        manager.conversations()[0].turn(ids[0]).unwrap().has_edits(),
        "1ターン目は適用済みのまま"
    );
    assert_eq!(turn_ids(&manager), ids, "ターンIDは巻き戻し後も変わらない");
}

/// A turn that was already rolled back cannot be rolled back twice.
/// 既に巻き戻したターンは二重に巻き戻せない(明示エラー)。
#[tokio::test]
async fn undo_turn_rejects_an_already_undone_turn() {
    let doc = FakeDoc::new(1);
    let manager = manager(Arc::clone(&doc), "fake_claude.sh");
    let mut rx = manager.subscribe();
    let id = manager.send(None, "置いて", None, None).await.unwrap();
    collect_turn(&mut rx).await;

    let turn = last_turn_id(&manager);
    manager.undo_turn(id, turn).expect("1回目は成功");
    let calls = doc.revert_calls.load(Ordering::SeqCst);

    let err = manager.undo_turn(id, turn).unwrap_err();
    assert!(
        matches!(err, madake_agent::AgentError::TurnNotApplied(t) if t == turn),
        "{err}"
    );
    assert_eq!(
        doc.revert_calls.load(Ordering::SeqCst),
        calls,
        "2回目はドキュメントに触らない"
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

    let err = manager.undo_turn(id, last_turn_id(&manager)).unwrap_err();
    assert!(matches!(err, madake_agent::AgentError::Busy), "{err}");
    manager.cancel(id);
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
    let turn = last_turn_id(&manager);

    assert!(
        matches!(
            manager.undo_turn(Uuid::new_v4(), turn),
            Err(madake_agent::AgentError::NoConversation(_))
        ),
        "未知の会話"
    );
    let unknown = Uuid::new_v4();
    let err = manager.undo_turn(id, unknown).unwrap_err();
    assert!(
        matches!(err, madake_agent::AgentError::UnknownTurn(t) if t == unknown),
        "未知のターンIDは明示エラー: {err}"
    );
    assert!(
        err.to_string().contains(&unknown.to_string()),
        "どのターンかがメッセージに出る: {err}"
    );

    *doc.revert_error.lock().unwrap() = Some(RevertError::Failed("engine busy".to_string()));
    let err = manager.undo_turn(id, turn).unwrap_err().to_string();
    assert!(err.contains("engine busy"), "{err}");
}

/// Every broadcast event carries the sequence number of the turn that produced it, increasing with each send.
/// 配信される全イベントに、それを生んだターンの通し番号が付き、送信のたびに増える。
#[tokio::test]
async fn turn_events_carry_a_monotonic_turn_seq() {
    let doc = FakeDoc::new(0);
    let manager = manager(doc, "fake_claude.sh");
    let mut rx = manager.subscribe();

    let id = manager.send(None, "1回目", None, None).await.unwrap();
    let first = collect_turn_seqs(&mut rx).await;
    assert!(first.iter().all(|s| *s == first[0]), "1ターン内は同じ番号");
    assert!(first[0] >= 1, "1始まりの通し番号: {first:?}");

    manager.send(Some(id), "2回目", None, None).await.unwrap();
    let second = collect_turn_seqs(&mut rx).await;
    assert!(
        second[0] > first[0],
        "次のターンは大きい番号: {first:?} → {second:?}"
    );
}

/// Cancelling reports the cancelled turn's sequence number, and the next turn gets a higher one, so late events can be told apart.
/// キャンセルは中断したターンの通し番号を伝え、次のターンはより大きい番号になるため、遅れて届くイベントを見分けられる。
#[tokio::test]
async fn cancel_reports_the_cancelled_turn_seq() {
    let doc = FakeDoc::new(0);
    let manager = manager(doc, "fake_claude_flood.sh");
    let mut rx = manager.subscribe();

    let id = manager.send(None, "hi", None, None).await.unwrap();
    let first = tokio::time::timeout(Duration::from_secs(10), rx.recv())
        .await
        .expect("イベントが来ない")
        .unwrap();
    let running_seq = first.turn_seq;
    assert!(running_seq >= 1);

    assert!(manager.cancel(id), "中断できる");
    let mut cancelled_seq = None;
    for _ in 0..2000 {
        match tokio::time::timeout(Duration::from_secs(5), rx.recv()).await {
            Ok(Ok(ConversationEvent {
                turn_seq,
                event: AgentEvent::Error { .. },
                ..
            })) => {
                cancelled_seq = Some(turn_seq);
                break;
            }
            Ok(Ok(_)) => continue,
            _ => break,
        }
    }
    assert_eq!(
        cancelled_seq,
        Some(running_seq),
        "中断したターンの番号が載る"
    );

    // 次のターンは大きい番号 = 中断済み番号以下のイベントは捨ててよい
    manager.set_executable(Some(fixtures_dir().join("fake_claude.sh")));
    manager.send(Some(id), "やり直し", None, None).await.unwrap();
    let next = collect_turn_seqs(&mut rx).await;
    assert!(next[0] > running_seq, "{next:?} > {running_seq}");
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
    let manager = manager(doc, "fake_claude_probe_prompt.sh");
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
    assert!(text.contains("model=claude-opus-4-6"), "{text}");
    assert!(text.contains("drawing"), "{text}");
    assert_eq!(
        manager.conversations()[0].model.as_deref(),
        Some("claude-opus-4-6")
    );
}

/// Every turn injects the bundled standards knowledge and the verification-loop rule.
/// 送信のたびに、同梱の規格知識と検証ループの指示がシステムプロンプトへ載る。
#[tokio::test]
async fn every_turn_injects_the_standards_knowledge() {
    let doc = FakeDoc::new(0);
    let manager = manager(doc, "fake_claude_probe_prompt.sh");
    let mut rx = manager.subscribe();

    // 図面コンテキストが無いターン(質問だけ)でも規格知識は載る
    manager.send(None, "線番の付け方は?", None, None).await.unwrap();
    let text = turn_result(&mut rx).await;
    assert!(text.contains("standards"), "{text}");
    assert!(text.contains("verify-loop"), "{text}");
}

/// The knowledge file from the settings reaches the CLI too.
/// 設定の知識ファイルの内容もCLIへ渡る。
#[tokio::test]
async fn the_knowledge_file_setting_reaches_the_cli() {
    let dir = std::env::temp_dir().join(format!("madake_knowledge_manager_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let knowledge = dir.join("house-rules.md");
    std::fs::write(&knowledge, "## 社内ルール\n- 制御電源は24VDCのみ\n").unwrap();

    let doc = FakeDoc::new(0);
    let manager = manager(doc, "fake_claude_probe_prompt.sh");
    let mut rx = manager.subscribe();
    manager.apply_settings(AppSettings {
        claude_path: Some(fixtures_dir().join("fake_claude_probe_prompt.sh")),
        knowledge_path: Some(knowledge),
        ..AppSettings::default()
    });

    manager.send(None, "hi", None, None).await.unwrap();
    let text = turn_result(&mut rx).await;
    assert!(text.contains("user-knowledge"), "{text}");
    assert!(text.contains("standards"), "同梱ノートも残る: {text}");

    std::fs::remove_dir_all(&dir).ok();
}

/// Turning off auto-read-drawing suppresses the drawing context but keeps the standards knowledge.
/// 図面自動読み取りをオフにすると図面コンテキストは付かないが、規格知識は残る。
#[tokio::test]
async fn auto_read_drawing_off_suppresses_the_drawing_context() {
    let doc = FakeDoc::new(0);
    let manager = manager(doc, "fake_claude_probe_prompt.sh");
    let mut rx = manager.subscribe();

    manager.apply_settings(AppSettings {
        claude_path: Some(fixtures_dir().join("fake_claude_probe_prompt.sh")),
        auto_apply: true,
        auto_read_drawing: false,
        ..AppSettings::default()
    });
    manager
        .send(None, "hi", None, Some("アクティブシート: S1".to_string()))
        .await
        .unwrap();
    let text = turn_result(&mut rx).await;
    assert!(
        !text.contains("drawing"),
        "自動読み取りOFFでは図面コンテキストを渡さない: {text}"
    );
    assert!(
        text.contains("standards"),
        "図面を渡さなくても規格知識は渡す: {text}"
    );

    // 設定を戻せば、そのまま次の送信から復活する
    manager.apply_settings(AppSettings {
        claude_path: Some(fixtures_dir().join("fake_claude_probe_prompt.sh")),
        auto_apply: true,
        auto_read_drawing: true,
        ..AppSettings::default()
    });
    manager
        .send(None, "hi", None, Some("アクティブシート: S1".to_string()))
        .await
        .unwrap();
    assert!(turn_result(&mut rx).await.contains("drawing"));
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
        claude_path: Some(fixtures_dir().join("fake_claude_probe_prompt.sh")),
        ..AppSettings::default()
    });
    assert_eq!(
        manager.executable(),
        Some(fixtures_dir().join("fake_claude_probe_prompt.sh"))
    );

    manager
        .send(None, "hi", Some("claude-opus-4-6".to_string()), None)
        .await
        .unwrap();
    assert!(turn_result(&mut rx).await.contains("model=claude-opus-4-6"));

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
