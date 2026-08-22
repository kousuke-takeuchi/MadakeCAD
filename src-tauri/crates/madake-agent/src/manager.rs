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

use crate::backend::{AgentBackend, ClaudeCodeCliBackend, DetectResult, HistoryMessage, TurnRequest};
use crate::conversation::{AppliedRevisions, AppliedUndoDepth, Conversation, DocState};
use crate::events::AgentEvent;
use crate::settings::{AgentProvider, AppSettings};
use crate::tools::ToolBridge;
use crate::{AgentError, AnthropicApiBackend, Result};

/// ユーザーがターンを中断したときにメッセージへ記録する理由。
pub const CANCELLED_MESSAGE: &str = "キャンセルされました";

/// 配信チャネルのバッファ長(遅い購読者はLaggedで取りこぼす)。
const EVENT_CHANNEL_CAPACITY: usize = 1024;

/// ターン巻き戻しの結果(ドキュメント側からの報告)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RevertReport {
    /// 逆適用した編集コマンド数(0なら戻す編集が無かった)
    pub reverted: u64,
    /// 巻き戻し後のrevision
    pub revision: u64,
}

/// 巻き戻しが実行できなかった理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevertError {
    /// 逆適用が現在の図面と衝突した(対象が手編集で消えている等)。**図面は無変更**
    Conflict(String),
    /// その他の失敗(ドキュメント側のエラー)
    Failed(String),
}

/// ドキュメント(Commandエンジン)側への最小の窓口。
///
/// madake-agentがmadake-coreへ依存しないためのトレイト。実装はmadake-mcp側にあり、
/// `SharedDoc`のエンジンを触る。**新しい編集経路は作らない**: 巻き戻しも
/// Commandエンジンの逆Command適用(patch配信込み)を呼ぶだけ。
pub trait DocBridge: Send + Sync + 'static {
    /// 現在のドキュメントrevision。
    fn revision(&self) -> u64;
    /// 現在のundoスタック深さ(積まれている編集コマンド数)。
    fn undo_depth(&self) -> u64;

    /// undo深さの区間`[start_depth, end_depth)`にある**エージェント由来の編集だけ**を
    /// 逆Commandとして適用し、巻き戻す。
    ///
    /// 区間に挟まったユーザーの手編集は保持する。逆適用が現在の図面と衝突する場合は
    /// [`RevertError::Conflict`]を返し、図面は一切変更しない(部分適用しない)。
    fn revert_agent_edits(
        &self,
        start_depth: u64,
        end_depth: u64,
    ) -> std::result::Result<RevertReport, RevertError>;

    /// エージェントのターン実行が始まったことを知らせる。
    ///
    /// この区間にCommandエンジンへ届いた編集はエージェント由来として記録される
    /// (MCPサーバーの入口はUI・外部クライアントと共通なので、時間で見分ける)。
    fn begin_agent_turn(&self) {}
    /// ターン実行が終わったことを知らせる(中断時も必ず呼ばれる)。
    fn end_agent_turn(&self) {}

    /// revisionと深さの組。両方を1回のロックで取れるなら上書きすること。
    fn state(&self) -> DocState {
        DocState::new(self.revision(), self.undo_depth())
    }
}

/// ターン実行中だけ「エージェント編集中」を立てるRAIIガード。
///
/// タスクがabort(キャンセル)された場合もdropは走るため、フラグが立ちっぱなしに
/// ならない。
struct AgentTurnGuard(Arc<dyn DocBridge>);

impl AgentTurnGuard {
    fn begin(doc: Arc<dyn DocBridge>) -> Self {
        doc.begin_agent_turn();
        Self(doc)
    }
}

impl Drop for AgentTurnGuard {
    fn drop(&mut self) {
        self.0.end_agent_turn();
    }
}

/// 購読者へ配信する1件。
/// JSONは`{"conversation_id": "...", "turn_seq": N, "event": {"type": ...}}`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationEvent {
    pub conversation_id: Uuid,
    /// このイベントを生んだターンの通し番号(送信のたびに単調増加。1始まり)。
    ///
    /// キャンセル後に遅れて届くイベントを受信側が捨てるための鍵。中断したターンの
    /// seqを覚えておき、それ以下のseqのイベントを無視すれば、次のターンの表示へ
    /// 前のターンの残りが混ざらない。`0`は「不明」(旧サーバー由来)。
    #[serde(default)]
    pub turn_seq: u64,
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

    /// 巻き戻した区間`swept`に**すっぽり収まる**ターンを、全会話から探して
    /// 巻き戻し済みにする。
    ///
    /// 会話をまたいでターンが同時に走ると、あとから巻き戻す区間に別会話のターンの
    /// 編集が混ざる(エージェント編集は由来だけで選ぶため、会話別には戻せない)。
    /// 巻き込まれたターンをそのままにすると、UIが「適用済み」と表示し続けたまま
    /// 「元に戻す」が空振りするので、ここで実態に合わせる。
    /// 区間から**はみ出す**ターン(まだ戻っていない編集が残るターン)は触らない。
    fn mark_swept_turns(&mut self, swept: AppliedUndoDepth) {
        for conversation in &mut self.conversations {
            let mut changed = false;
            for message in &mut conversation.messages {
                let depth = message.applied_undo_depth;
                if message.has_edits() && swept.start <= depth.start && depth.end <= swept.end {
                    message.record_reverted();
                    changed = true;
                }
            }
            if changed {
                conversation.touch();
            }
        }
    }
}

/// 会話の保持とターン実行を受け持つ。`Arc`で共有して使う。
pub struct AgentManager {
    doc: Arc<dyn DocBridge>,
    state: Arc<Mutex<ManagerState>>,
    events: broadcast::Sender<ConversationEvent>,
    mcp_port: u16,
    /// API直結バックエンドが図面編集ツールを呼ぶための窓口(madake-mcpが差す)。
    ///
    /// Claude Code CLIバックエンドはCLI自身がMCPサーバーへ接続するため使わない。
    tools: Mutex<Option<Arc<dyn ToolBridge>>>,
}

impl AgentManager {
    pub fn new(doc: Arc<dyn DocBridge>, mcp_port: u16) -> Self {
        let (events, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            doc,
            state: Arc::new(Mutex::new(ManagerState::default())),
            events,
            mcp_port,
            tools: Mutex::new(None),
        }
    }

    /// 図面編集ツールの窓口をつなぐ(API直結バックエンド用)。
    pub fn set_tool_bridge(&self, tools: Arc<dyn ToolBridge>) {
        *self.tools.lock().unwrap() = Some(tools);
    }

    fn tool_bridge(&self) -> Option<Arc<dyn ToolBridge>> {
        self.tools.lock().unwrap().clone()
    }

    /// いま選ばれているプロバイダで実際に送信できるか(UIのバッジ表示用)。
    ///
    /// - [`AgentProvider::ClaudeCli`]: CLIを検出できるか
    /// - [`AgentProvider::AnthropicApi`]: APIキーが保管されているか(値は返さない)
    pub async fn provider_ready(&self) -> bool {
        match self.settings().provider {
            AgentProvider::ClaudeCli => self.detect().await.is_ok(),
            AgentProvider::AnthropicApi => crate::secrets::has_anthropic_api_key(),
        }
    }

    /// Anthropic APIへの疎通を試す(設定画面の「接続テスト」)。
    ///
    /// 成功したら確かめたモデルIDを返す。失敗は種類つきの[`ConnectionError`]
    /// (UIはkindで翻訳し、翻訳が無ければmessageをそのまま出す)。
    /// **APIキーは戻り値にも含めない。**
    pub async fn test_anthropic_connection(
        &self,
    ) -> std::result::Result<String, crate::anthropic::ConnectionError> {
        let model = self.settings().normalized().api_model;
        let key = crate::secrets::anthropic_api_key().ok_or_else(|| {
            crate::anthropic::ConnectionError {
                kind: "no_key".to_string(),
                message: AgentError::NoApiKey.to_string(),
            }
        })?;
        let backend = AnthropicApiBackend::new(key, model.clone());
        backend.check_connection().await.map(|()| model)
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
    /// 呼び出し側は設定を気にせず毎回渡してよい。CLIへ渡すシステムプロンプトは
    /// [`crate::knowledge::system_prompt`]が組み立てる(図面コンテキスト+作図ルール+
    /// 規格知識+設定の知識ファイル)。
    pub async fn send(
        &self,
        conversation_id: Option<Uuid>,
        prompt: &str,
        model: Option<String>,
        context: Option<String>,
    ) -> Result<Uuid> {
        let settings = self.settings();
        // プロバイダごとの前提を先に確かめる(claude CLIが無くてもAPIキーがあれば送れる)
        let executable = match settings.provider {
            AgentProvider::ClaudeCli => Some(self.resolve_executable().await?),
            AgentProvider::AnthropicApi => {
                if !crate::secrets::has_anthropic_api_key() {
                    return Err(AgentError::NoApiKey);
                }
                None
            }
        };

        let (id, seq, session, turn_model, context, history) = {
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
            // 「図面の自動読み取り」OFFなら図面の内容をCLIへ渡さない。
            // 規格知識と作図ルールはその場合も渡す(図面を伏せるだけで、規格を忘れさせない)
            let context = state
                .settings
                .auto_read_drawing
                .then_some(context)
                .flatten();
            let context = Some(crate::knowledge::system_prompt(
                &state.settings,
                context.as_deref(),
            ));
            let conversation = state
                .conversation_mut(id)
                .expect("直前に存在確認済みの会話が消えることはない");
            if model.is_some() {
                conversation.model = model;
            }
            // API直結は文脈をこちらで送り直すので、ターン開始より前の履歴を控えておく
            let history = api_history(conversation);
            conversation.begin_turn(prompt, doc_state);
            let session = conversation.session_id.clone();
            let turn_model = conversation.model.clone();
            (id, seq, session, turn_model, context, history)
        };

        let backend: Box<dyn AgentBackend> = match (settings.provider, executable) {
            (AgentProvider::ClaudeCli, Some(executable)) => {
                let mut backend = ClaudeCodeCliBackend::new(executable, self.mcp_port);
                backend.model = turn_model;
                Box::new(backend)
            }
            _ => {
                // 直前にhas_anthropic_api_keyで確かめてあるが、その後に消された場合に備える
                let key = crate::secrets::anthropic_api_key().ok_or(AgentError::NoApiKey)?;
                // 会話ごとのモデル指定はCLI用のIDなのでAPIには使わず、設定のapi_modelを使う
                let mut backend =
                    AnthropicApiBackend::new(key, settings.normalized().api_model.clone());
                if let Some(tools) = self.tool_bridge() {
                    backend = backend.with_tools(tools);
                }
                Box::new(backend)
            }
        };

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
            history,
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
        let (seq, applied) = {
            let mut state = self.state.lock().unwrap();
            let Some((seq, handle)) = state.running.remove(&conversation_id) else {
                return false;
            };
            handle.abort();
            let doc_state = self.doc.state();
            let applied = state.conversation_mut(conversation_id).and_then(|c| {
                c.touch();
                let message = c.current_turn_mut()?;
                message.error = Some(CANCELLED_MESSAGE.to_string());
                message.finish_turn(doc_state);
                message.has_edits().then_some((
                    message.turn_id,
                    message.applied_revisions,
                    message.applied_undo_depth,
                ))
            });
            (seq, applied)
        };
        // abortされたタスクは後片付けを実行できないので、終了イベントはここで流す。
        // seqは中断したターンのもの: 受信側はこれ以下のseqの遅延イベントを捨てられる
        self.broadcast(
            conversation_id,
            seq,
            AgentEvent::Error {
                message: CANCELLED_MESSAGE.to_string(),
            },
        );
        if let Some((turn_id, revisions, depth)) = applied {
            self.broadcast_applied(conversation_id, seq, turn_id, revisions, depth);
        }
        true
    }

    /// 指定ターンで入った編集を巻き戻す。戻り値は巻き戻し後のrevision。
    ///
    /// 対象はターンの安定ID([`crate::ChatMessage::turn_id`])で指定する。メッセージ添字と
    /// 違い、後続ターンの追記や履歴移行でズレないため、別ターンを取り違えて戻すことがない。
    ///
    /// 巻き戻すのは**そのターンのエージェント編集だけ**。ターン中・ターン後にユーザーが
    /// 手で入れた編集は保持されるので、手編集を挟んでも「元に戻す」が巻き込むことはない
    /// (ドキュメント側は逆Commandの適用として実行する。詳細は[`DocBridge::revert_agent_edits`])。
    ///
    /// エラー:
    /// - 実行中のターン: [`AgentError::Busy`](編集がまだ増えるため確定後に戻す)
    /// - 既に巻き戻したターン: [`AgentError::TurnNotApplied`](黙って成功を返すと、
    ///   UIが「戻した」と表示したまま何も起きない)
    /// - 逆適用が現在の図面と衝突: [`AgentError::TurnConflict`](図面は無変更)
    pub fn undo_turn(&self, conversation_id: Uuid, turn_id: Uuid) -> Result<u64> {
        let range = {
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
            message.applied_undo_depth
        };

        let report = match self.doc.revert_agent_edits(range.start, range.end) {
            Ok(report) => report,
            Err(RevertError::Conflict(detail)) => {
                return Err(AgentError::TurnConflict { turn_id, detail })
            }
            Err(RevertError::Failed(detail)) => return Err(AgentError::Doc(detail)),
        };
        // 区間にエージェント編集が1件も無かった(履歴が入れ替わった等)。図面は無変更
        if report.reverted == 0 {
            return Err(AgentError::TurnNotApplied(turn_id));
        }

        let mut state = self.state.lock().unwrap();
        if let Some(conversation) = state.conversation_mut(conversation_id) {
            conversation.touch();
            if let Some(index) = conversation.turn_index(turn_id) {
                conversation.messages[index].record_reverted();
            }
        }
        // 並行して走っていた別会話のターンの編集も、区間に入っていれば一緒に戻っている
        // (エージェント編集は由来だけを見て戻すため、会話ごとには選り分けられない)。
        // まるごと戻ったターンはここで巻き戻し済みにして、UIが「適用済み」と表示したまま
        // 実体の無い巻き戻しを勧めることのないようにする。
        state.mark_swept_turns(range);
        Ok(report.revision)
    }

    /// 設定済みパス、無ければ検出結果(キャッシュ)を返す。
    async fn resolve_executable(&self) -> Result<PathBuf> {
        if let Some(path) = self.executable() {
            return Ok(path);
        }
        Ok(self.detect().await?.path)
    }

    fn broadcast(&self, conversation_id: Uuid, turn_seq: u64, event: AgentEvent) {
        let _ = self.events.send(ConversationEvent {
            conversation_id,
            turn_seq,
            event,
        });
    }

    fn broadcast_applied(
        &self,
        conversation_id: Uuid,
        turn_seq: u64,
        turn_id: Uuid,
        revisions: AppliedRevisions,
        depth: AppliedUndoDepth,
    ) {
        self.broadcast(
            conversation_id,
            turn_seq,
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
    /// この会話の過去のやりとり(API直結バックエンドが文脈として送り直す)
    history: Vec<HistoryMessage>,
}

/// 会話の過去のやりとりを、API直結バックエンドへ渡す形にする。
///
/// 本文のみ(ツール往復は送り直さない)。中断・失敗したターンの空応答は落とす。
fn api_history(conversation: &Conversation) -> Vec<HistoryMessage> {
    conversation
        .messages
        .iter()
        .filter(|m| !m.text.trim().is_empty())
        .map(|m| HistoryMessage {
            role: m.role,
            text: m.text.clone(),
        })
        .collect()
}

impl Turn {
    async fn run(self, backend: Box<dyn AgentBackend>) {
        // この間にCommandエンジンへ届いた編集はエージェント由来として記録される。
        // 中断(abort)されてもdropは走るので、フラグは必ず下りる
        let _agent_turn = AgentTurnGuard::begin(Arc::clone(&self.doc));
        let (tx, mut rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        let prompt = self.prompt.clone();
        let session = self.session.clone();
        let context = self.context.clone();
        let history = self.history.clone();
        // バックエンドの駆動は別タスク。こちらがabortされるとrxが落ち、CLIバックエンドは
        // 子プロセスをkillして、API直結バックエンドは送信をやめて自然に終わる
        let backend_task = tokio::spawn(async move {
            let _ = backend
                .run_turn(
                    TurnRequest {
                        prompt: &prompt,
                        session: session.as_deref(),
                        system_prompt: context.as_deref(),
                        history: &history,
                    },
                    tx,
                )
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
                turn_seq: self.seq,
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
                turn_seq: self.seq,
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
