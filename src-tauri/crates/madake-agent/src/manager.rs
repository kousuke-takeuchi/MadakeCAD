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
use crate::conversation::{AppliedRevisions, Conversation, Role};
use crate::events::AgentEvent;
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
    /// undoを1回実行する。戻せる編集が無ければ`false`。
    fn undo(&self) -> std::result::Result<bool, String>;
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

    /// 全会話のスナップショット。
    pub fn conversations(&self) -> Vec<Conversation> {
        self.state.lock().unwrap().conversations.clone()
    }

    /// 会話を丸ごと差し替える(プロジェクト読込・新規作成時)。
    ///
    /// 送信中のターンがあれば中断する(別プロジェクトの会話へイベントが混ざらないように)。
    pub fn set_conversations(&self, conversations: Vec<Conversation>) {
        let running: Vec<Uuid> = {
            let state = self.state.lock().unwrap();
            state.running.keys().copied().collect()
        };
        for id in running {
            self.cancel(id);
        }
        let mut state = self.state.lock().unwrap();
        state.conversations = conversations;
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
    pub async fn send(
        &self,
        conversation_id: Option<Uuid>,
        prompt: &str,
        model: Option<String>,
        context: Option<String>,
    ) -> Result<Uuid> {
        let executable = self.resolve_executable().await?;

        let (id, seq, session, turn_model) = {
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
            let revision = self.doc.revision();
            state.next_seq += 1;
            let seq = state.next_seq;
            let conversation = state
                .conversation_mut(id)
                .expect("直前に存在確認済みの会話が消えることはない");
            if model.is_some() {
                conversation.model = model;
            }
            conversation.begin_turn(prompt, revision);
            let session = conversation.session_id.clone();
            let turn_model = conversation.model.clone();
            (id, seq, session, turn_model)
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
            let revision = self.doc.revision();
            state.conversation_mut(conversation_id).and_then(|c| {
                let message = c.current_turn_mut()?;
                message.error = Some(CANCELLED_MESSAGE.to_string());
                message.applied_revisions.end = revision;
                Some(message.applied_revisions)
            })
        };
        // abortされたタスクは後片付けを実行できないので、終了イベントはここで流す
        self.broadcast(
            conversation_id,
            AgentEvent::Error {
                message: CANCELLED_MESSAGE.to_string(),
            },
        );
        if let Some(applied) = applied {
            self.broadcast_applied(conversation_id, applied);
        }
        true
    }

    /// 指定メッセージのターンで入った編集を巻き戻す。戻り値は巻き戻し後のrevision。
    ///
    /// エンジンのundoはLIFOなので、そのターンより後に別の編集が入っていた場合は
    /// 後の編集から戻る(redoで復帰可能)。UIは直近ターンにのみ「元に戻す」を出す想定。
    pub fn undo_turn(&self, conversation_id: Uuid, message_index: usize) -> Result<u64> {
        let count = {
            let mut state = self.state.lock().unwrap();
            let conversation = state
                .conversation_mut(conversation_id)
                .ok_or(AgentError::NoConversation(conversation_id))?;
            let message = conversation
                .messages
                .get(message_index)
                .ok_or(AgentError::NoMessage(message_index))?;
            if message.role != Role::Assistant {
                return Err(AgentError::NoMessage(message_index));
            }
            message.applied_command_count()
        };

        for _ in 0..count {
            if !self.doc.undo().map_err(AgentError::Doc)? {
                break;
            }
        }

        let revision = self.doc.revision();
        {
            let mut state = self.state.lock().unwrap();
            if let Some(message) = state
                .conversation_mut(conversation_id)
                .and_then(|c| c.messages.get_mut(message_index))
            {
                // 巻き戻し済み。以降このターンは「適用済み」ではない
                message.applied_revisions = AppliedRevisions::at(message.applied_revisions.start);
            }
        }
        Ok(revision)
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

    fn broadcast_applied(&self, conversation_id: Uuid, applied: AppliedRevisions) {
        if applied.count() == 0 {
            return;
        }
        self.broadcast(
            conversation_id,
            AgentEvent::TurnApplied {
                start_revision: applied.start,
                end_revision: applied.end,
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
            let revision = self.doc.revision();
            if matches!(
                event,
                AgentEvent::TurnCompleted { .. } | AgentEvent::Error { .. }
            ) {
                terminated = true;
            }
            {
                let mut state = self.state.lock().unwrap();
                if let Some(conversation) = state.conversation_mut(self.conversation_id) {
                    conversation.apply_event(&event, revision);
                }
            }
            let _ = self.events.send(ConversationEvent {
                conversation_id: self.conversation_id,
                event,
            });
        }
        let _ = backend_task.await;

        let applied = {
            let revision = self.doc.revision();
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
                .and_then(|c| c.current_turn_mut())
                .map(|message| {
                    if !terminated {
                        // result行が来ないまま終わった場合の保険
                        message.applied_revisions.end = revision;
                    }
                    message.applied_revisions
                })
        };
        if let Some(applied) = applied {
            if applied.count() > 0 {
                let _ = self.events.send(ConversationEvent {
                    conversation_id: self.conversation_id,
                    event: AgentEvent::TurnApplied {
                        start_revision: applied.start,
                        end_revision: applied.end,
                    },
                });
            }
        }
    }
}
