//! 会話の実行管理: CLIターンの起動・イベント配信・undo追跡。
//!
//! UI(Tauri IPC)とLink APIの両方がこのマネージャを共有する。tauri/madake-coreには
//! 依存せず、ドキュメント側とは [`DocBridge`] だけで結ばれる。
//!
//! 配信されるイベントは [`ConversationEvent`](`{conversation_id, event}`)。
//! `event`はCLI由来の [`AgentEvent`] そのままだが、ターン終了時に編集が入っていた場合は
//! マネージャ発の合成イベント [`AgentEvent::TurnApplied`] が最後に1つ追加で流れる。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::backend::{ClaudeCodeCliBackend, DetectResult};
use crate::conversation::{AppliedRevisions, AppliedUndoDepth, Conversation, DocState, Role};
use crate::events::AgentEvent;
use crate::settings::AppSettings;
use crate::{AgentError, Result};

/// ユーザーがターンを中断したときにメッセージへ記録する理由。
pub const CANCELLED_MESSAGE: &str = "キャンセルされました";

/// 配信チャネルのバッファ長(遅い購読者はLaggedで取りこぼす)。
const EVENT_CHANNEL_CAPACITY: usize = 1024;

/// ドキュメント(Commandエンジン)側への最小の窓口。
///
/// madake-agentがmadake-coreへ依存しないためのトレイト。実装はmadake-mcp側にあり、
/// `SharedDoc`のエンジンを触る。**新しい編集経路は作らない**: undoは既存の
/// `SharedDoc::undo`(patch配信込み)をそのまま呼ぶだけ。
pub trait DocBridge: Send + Sync + 'static {
    /// 現在のドキュメントrevision。
    fn revision(&self) -> u64;
    /// 現在のundoスタック深さ(積まれている編集コマンド数)。
    fn undo_depth(&self) -> u64;
    /// undoを1回実行する。戻せる編集が無ければ`false`。
    fn undo(&self) -> std::result::Result<bool, String>;

    /// revisionと深さの組。両方を1回のロックで取れるなら上書きすること。
    fn state(&self) -> DocState {
        DocState::new(self.revision(), self.undo_depth())
    }
}

/// 購読者へ配信する1件。JSONは`{"conversation_id": "...", "event": {"type": ...}}`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationEvent {
    pub conversation_id: Uuid,
    pub event: AgentEvent,
}

#[derive(Default)]
struct ManagerState {
    conversations: Vec<Conversation>,
    /// 送信中の会話 → (ターン通し番号, タスクハンドル)
    running: HashMap<Uuid, (u64, JoinHandle<()>)>,
    /// 検出済み(または設定済み)のclaude実行パス
    executable: Option<PathBuf>,
    /// アプリ設定(`~/.madakecad/settings.json`由来)。次の送信から効く
    settings: AppSettings,
    next_seq: u64,
}

impl ManagerState {
    fn conversation_mut(&mut self, id: Uuid) -> Option<&mut Conversation> {
        self.conversations.iter_mut().find(|c| c.id == id)
    }
}

/// 会話の保持とターン実行を受け持つ。`Arc`で共有して使う。
pub struct AgentManager {
    doc: Arc<dyn DocBridge>,
    state: Arc<Mutex<ManagerState>>,
    events: broadcast::Sender<ConversationEvent>,
    mcp_port: u16,
}

impl AgentManager {
    pub fn new(doc: Arc<dyn DocBridge>, mcp_port: u16) -> Self {
        let (events, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            doc,
            state: Arc::new(Mutex::new(ManagerState::default())),
            events,
            mcp_port,
        }
    }

    /// イベント購読(接続以降のイベントのみ)。
    pub fn subscribe(&self) -> broadcast::Receiver<ConversationEvent> {
        self.events.subscribe()
    }

    /// claude実行パスを固定する(設定・テスト用)。`None`で自動検出へ戻す。
    pub fn set_executable(&self, executable: Option<PathBuf>) {
        self.state.lock().unwrap().executable = executable;
    }

    pub fn executable(&self) -> Option<PathBuf> {
        self.state.lock().unwrap().executable.clone()
    }

    /// 現在のアプリ設定。
    pub fn settings(&self) -> AppSettings {
        self.state.lock().unwrap().settings.clone()
    }

    /// アプリ設定を反映する(**次の送信から有効**。再起動は不要)。
    ///
    /// `claude_path`はそのまま検出の明示パスになる(`None`で自動検出へ戻す)。
    /// `auto_read_drawing`が`false`なら[`Self::send`]は図面コンテキストを渡さない。
    pub fn apply_settings(&self, settings: AppSettings) {
        let mut state = self.state.lock().unwrap();
        // 明示パスなしの設定保存(トグル変更等)で検出済みパスのキャッシュを捨てない。
        // 明示パス→自動検出へ戻す場合はresolve_executableが次回送信時に再検出する
        if settings.claude_path.is_some() || state.settings.claude_path.is_some() {
            state.executable = settings.claude_path.clone();
        }
        state.settings = settings;
    }

    /// 全会話のスナップショット。
    pub fn conversations(&self) -> Vec<Conversation> {
        self.state.lock().unwrap().conversations.clone()
    }

    /// 会話を丸ごと差し替える(プロジェクト読込・新規作成時)。
    ///
    /// 送信中のターンがあれば中断する(別プロジェクトの会話へイベントが混ざらないように)。
    pub fn set_conversations(&self, conversations: Vec<Conversation>) {
        self.cancel_all();
        let mut state = self.state.lock().unwrap();
        state.conversations = conversations;
    }

    /// 実行中の全ターンを中断する。戻り値は中断した本数。
    ///
    /// プロジェクトを差し替える前に呼ぶこと(走っているCLIが新しい図面を
    /// 編集し始めるのを防ぐ)。
    pub fn cancel_all(&self) -> usize {
        let running: Vec<Uuid> = {
            let state = self.state.lock().unwrap();
            state.running.keys().copied().collect()
        };
        running.into_iter().filter(|id| self.cancel(*id)).count()
    }

    /// 指定会話がターン実行中か。
    pub fn is_sending(&self, conversation_id: Uuid) -> bool {
        self.state
            .lock()
            .unwrap()
            .running
            .contains_key(&conversation_id)
    }

    /// claude CLIを検出する。成功したパスは以降のターンで再利用する。
    pub async fn detect(&self) -> Result<DetectResult> {
        let configured = self.executable();
        let found = ClaudeCodeCliBackend::detect(configured).await?;
        self.state.lock().unwrap().executable = Some(found.path.clone());
        Ok(found)
    }

    /// 1ターンを開始する。`conversation_id`が`None`なら新規会話を作る。
    ///
    /// 戻り値は対象の会話ID。イベントは購読者へ非同期に流れる。
    ///
    /// `context`(図面コンテキスト)は設定の「図面の自動読み取り」がOFFなら捨てる。
    /// 呼び出し側は設定を気にせず毎回渡してよい。
    pub async fn send(
        &self,
        conversation_id: Option<Uuid>,
        prompt: &str,
        model: Option<String>,
        context: Option<String>,
    ) -> Result<Uuid> {
        let executable = self.resolve_executable().await?;

        let (id, seq, session, turn_model, context) = {
            let mut state = self.state.lock().unwrap();
            let id = match conversation_id {
                Some(id) => {
                    if !state.conversations.iter().any(|c| c.id == id) {
                        return Err(AgentError::NoConversation(id));
                    }
                    id
                }
                None => {
                    let conversation = Conversation::new();
                    let id = conversation.id;
                    state.conversations.push(conversation);
                    id
                }
            };
            if state.running.contains_key(&id) {
                return Err(AgentError::Busy);
            }
            let doc_state = self.doc.state();
            state.next_seq += 1;
            let seq = state.next_seq;
            // 「図面の自動読み取り」OFFなら図面の内容をCLIへ渡さない
            let context = state
                .settings
                .auto_read_drawing
                .then_some(context)
                .flatten();
            let conversation = state
                .conversation_mut(id)
                .expect("直前に存在確認済みの会話が消えることはない");
            if model.is_some() {
                conversation.model = model;
            }
            conversation.begin_turn(prompt, doc_state);
            let session = conversation.session_id.clone();
            let turn_model = conversation.model.clone();
            (id, seq, session, turn_model, context)
        };

        let mut backend = ClaudeCodeCliBackend::new(executable, self.mcp_port);
        backend.model = turn_model;

        // タスクの登録より先にターンが終わると`running`に完了済みハンドルが残るため、
        // 登録が済むまでoneshotで待たせる
        let (start_tx, start_rx) = oneshot::channel::<()>();
        let turn = Turn {
            doc: Arc::clone(&self.doc),
            state: Arc::clone(&self.state),
            events: self.events.clone(),
            conversation_id: id,
            seq,
            prompt: prompt.to_string(),
            session,
            context,
        };
        let handle = tokio::spawn(async move {
            if start_rx.await.is_err() {
                return;
            }
            turn.run(backend).await;
        });
        self.state.lock().unwrap().running.insert(id, (seq, handle));
        let _ = start_tx.send(());
        Ok(id)
    }

    /// 実行中のターンを中断する。中断した場合のみ`true`。
    ///
    /// タスクをabortするとバックエンド側の受信端(mpsc::Receiver)が落ち、
    /// `ClaudeCodeCliBackend`が子プロセスをkillして片付ける。
    pub fn cancel(&self, conversation_id: Uuid) -> bool {
        let applied = {
            let mut state = self.state.lock().unwrap();
            let Some((_, handle)) = state.running.remove(&conversation_id) else {
                return false;
            };
            handle.abort();
            let doc_state = self.doc.state();
            state.conversation_mut(conversation_id).and_then(|c| {
                c.touch();
                let message = c.current_turn_mut()?;
                message.error = Some(CANCELLED_MESSAGE.to_string());
                message.finish_turn(doc_state);
                message.has_edits().then_some((
                    message.turn_id,
                    message.applied_revisions,
                    message.applied_undo_depth,
                ))
            })
        };
        // abortされたタスクは後片付けを実行できないので、終了イベントはここで流す
        self.broadcast(
            conversation_id,
            AgentEvent::Error {
                message: CANCELLED_MESSAGE.to_string(),
            },
        );
        if let Some((turn_id, revisions, depth)) = applied {
            self.broadcast_applied(conversation_id, turn_id, revisions, depth);
        }
        true
    }

    /// 指定ターンで入った編集を巻き戻す。戻り値は巻き戻し後のrevision。
    ///
    /// 対象はターンの安定ID([`crate::ChatMessage::turn_id`])で指定する。メッセージ添字と
    /// 違い、後続ターンの追記や履歴移行でズレないため、別ターンを取り違えて戻すことがない。
    ///
    /// エンジンのundoはLIFOなので、**巻き戻せるのは最新の適用済みターンだけ**。
    /// 後続ターンやユーザー操作の編集が上に積まれている状態では拒否する。
    /// 既に巻き戻したターン(戻せる編集が残っていないターン)も明示エラーにする
    /// (黙って成功を返すと、UI側が「戻した」と表示したまま何も起きない)。
    pub fn undo_turn(&self, conversation_id: Uuid, turn_id: Uuid) -> Result<u64> {
        let count = {
            let state = self.state.lock().unwrap();
            if state.running.contains_key(&conversation_id) {
                // 実行中ターンはまだ編集が増える。確定してから戻す
                return Err(AgentError::Busy);
            }
            let conversation = state
                .conversations
                .iter()
                .find(|c| c.id == conversation_id)
                .ok_or(AgentError::NoConversation(conversation_id))?;
            let index = conversation
                .turn_index(turn_id)
                .ok_or(AgentError::UnknownTurn(turn_id))?;
            let message = &conversation.messages[index];
            if !message.has_edits() {
                return Err(AgentError::TurnNotApplied(turn_id));
            }
            // 同じ会話の後続ターンに編集が残っていないこと
            if conversation.messages[index + 1..]
                .iter()
                .any(|m| m.role == Role::Assistant && m.has_edits())
            {
                return Err(AgentError::NotLatestTurn(turn_id));
            }
            // 他の会話やユーザーのUI操作による編集が上に積まれていないこと
            if self.doc.undo_depth() != message.applied_undo_depth.end {
                return Err(AgentError::NotLatestTurn(turn_id));
            }
            message.applied_command_count()
        };

        let mut undone = 0u64;
        let mut failure = None;
        for _ in 0..count {
            match self.doc.undo() {
                Ok(true) => undone += 1,
                Ok(false) => break,
                Err(e) => {
                    failure = Some(AgentError::Doc(e));
                    break;
                }
            }
        }

        // 失敗しても「実行できた回数」だけは必ず記録する(リトライで戻しすぎないため)
        {
            let mut state = self.state.lock().unwrap();
            if let Some(conversation) = state.conversation_mut(conversation_id) {
                conversation.touch();
                if let Some(index) = conversation.turn_index(turn_id) {
                    conversation.messages[index].record_undone(undone);
                }
            }
        }
        match failure {
            Some(e) => Err(e),
            None => Ok(self.doc.revision()),
        }
    }

    /// 設定済みパス、無ければ検出結果(キャッシュ)を返す。
    async fn resolve_executable(&self) -> Result<PathBuf> {
        if let Some(path) = self.executable() {
            return Ok(path);
        }
        Ok(self.detect().await?.path)
    }

    fn broadcast(&self, conversation_id: Uuid, event: AgentEvent) {
        let _ = self.events.send(ConversationEvent {
            conversation_id,
            event,
        });
    }

    fn broadcast_applied(
        &self,
        conversation_id: Uuid,
        turn_id: Uuid,
        revisions: AppliedRevisions,
        depth: AppliedUndoDepth,
    ) {
        self.broadcast(
            conversation_id,
            AgentEvent::TurnApplied {
                turn_id,
                start_revision: revisions.start,
                end_revision: revisions.end,
                start_undo_depth: depth.start,
                end_undo_depth: depth.end,
            },
        );
    }
}

/// 1ターン分の実行コンテキスト(spawnされたタスクが所有する)。
struct Turn {
    doc: Arc<dyn DocBridge>,
    state: Arc<Mutex<ManagerState>>,
    events: broadcast::Sender<ConversationEvent>,
    conversation_id: Uuid,
    seq: u64,
    prompt: String,
    session: Option<String>,
    context: Option<String>,
}

impl Turn {
    async fn run(self, backend: ClaudeCodeCliBackend) {
        let (tx, mut rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        let prompt = self.prompt.clone();
        let session = self.session.clone();
        let context = self.context.clone();
        // CLIの駆動は別タスク。こちらがabortされるとrxが落ち、バックエンドは
        // 子プロセスをkillして自然に終わる
        let backend_task = tokio::spawn(async move {
            let _ = backend
                .send_with_context(&prompt, session.as_deref(), context.as_deref(), tx)
                .await;
        });

        let mut terminated = false;
        while let Some(event) = rx.recv().await {
            let doc_state = self.doc.state();
            if matches!(
                event,
                AgentEvent::TurnCompleted { .. } | AgentEvent::Error { .. }
            ) {
                terminated = true;
            }
            {
                let mut state = self.state.lock().unwrap();
                if let Some(conversation) = state.conversation_mut(self.conversation_id) {
                    conversation.apply_event(&event, doc_state);
                }
            }
            let _ = self.events.send(ConversationEvent {
                conversation_id: self.conversation_id,
                event,
            });
        }
        let _ = backend_task.await;

        let applied = {
            // 正常終了ならTurnCompleted/Error受信時の状態が既に記録済み。
            // result行が来ないまま終わった場合だけ、ここで取り直す
            let final_state = (!terminated).then(|| self.doc.state());
            let mut state = self.state.lock().unwrap();
            if state
                .running
                .get(&self.conversation_id)
                .map(|(seq, _)| *seq)
                == Some(self.seq)
            {
                state.running.remove(&self.conversation_id);
            }
            state
                .conversation_mut(self.conversation_id)
                .and_then(|c| {
                    if final_state.is_some() {
                        c.touch();
                    }
                    c.current_turn_mut()
                })
                .and_then(|message| {
                    if let Some(doc_state) = final_state {
                        message.finish_turn(doc_state);
                    }
                    message.has_edits().then_some((
                        message.turn_id,
                        message.applied_revisions,
                        message.applied_undo_depth,
                    ))
                })
        };
        if let Some((turn_id, revisions, depth)) = applied {
            let _ = self.events.send(ConversationEvent {
                conversation_id: self.conversation_id,
                event: AgentEvent::TurnApplied {
                    turn_id,
                    start_revision: revisions.start,
                    end_revision: revisions.end,
                    start_undo_depth: depth.start,
                    end_undo_depth: depth.end,
                },
            });
        }
    }
}
