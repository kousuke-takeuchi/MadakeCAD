//! 会話・ターン管理と、チャット履歴の永続化。
//!
//! ターン中にエージェントがMCP経由で実行した編集はCommandエンジンのundo履歴に積まれる。
//! ターン開始/終了時のundoスタック深さを [`ChatMessage::applied_undo_depth`] に記録して
//! おけば、「元に戻す」は増分の回数だけ`undo`を呼べばよい。
//!
//! **revisionではなく深さを使う理由**: エンジンの`revision`はundo/redoでも進むため、
//! ターン中にエージェントがMCPのundo/redoツールを使ったり、ユーザーがUIで編集+undoを
//! 挟んだりすると、revision差分は実際に積まれたコマンド数より大きくなる。
//! [`ChatMessage::applied_revisions`] は表示用に残してあるが、undo回数の正は深さ増分。
//!
//! 履歴はプロジェクトファイルの隣に`<stem>.chat.json`(整形JSON)として保存する。

use crate::{AgentError, AgentEvent, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// チャット履歴ファイルのフォーマット版(構造変更時に上げる)。
pub const CHAT_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
}

/// ターンの前後で挟んだEngineのrevision範囲(表示・デバッグ用)。
///
/// undo/redoでもrevisionは進むため、`end - start`は「積まれたコマンド数」ではない。
/// 巻き戻し回数には [`AppliedUndoDepth`] を使うこと。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AppliedRevisions {
    /// ターン開始時のrevision
    pub start: u64,
    /// ターン終了時のrevision
    pub end: u64,
}

impl AppliedRevisions {
    pub fn at(revision: u64) -> Self {
        Self {
            start: revision,
            end: revision,
        }
    }

    pub fn count(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }
}

/// ターンの前後で挟んだundoスタックの深さ。
///
/// `end - start`がこのターンで新たに積まれた編集コマンド数
/// (= 元に戻すのに必要なundo回数)。ターン中にundoが混ざっても正しい値になる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AppliedUndoDepth {
    /// ターン開始時のundoスタック深さ
    pub start: u64,
    /// ターン終了時のundoスタック深さ
    pub end: u64,
}

impl AppliedUndoDepth {
    pub fn at(depth: u64) -> Self {
        Self {
            start: depth,
            end: depth,
        }
    }

    pub fn count(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }
}

/// ある時点のドキュメント状態(revisionとundoスタック深さ)。
///
/// 両方を1回のロックで取れるよう、[`crate::DocBridge`]がまとめて返す。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DocState {
    pub revision: u64,
    pub undo_depth: u64,
}

impl DocState {
    pub fn new(revision: u64, undo_depth: u64) -> Self {
        Self {
            revision,
            undo_depth,
        }
    }
}

/// 1回のツール呼び出し(UIのツールチップ表示に使う)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    /// CLIのtool_use_id
    pub id: String,
    pub tool: String,
    pub input: Value,
    pub finished: bool,
    pub is_error: bool,
}

/// 会話中の1メッセージ。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: Role,
    pub text: String,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    /// ターン開始/終了時のEngine revision(表示用)
    pub applied_revisions: AppliedRevisions,
    /// ターン開始/終了時のundoスタック深さ(巻き戻し回数の正)
    #[serde(default)]
    pub applied_undo_depth: AppliedUndoDepth,
    #[serde(default)]
    pub error: Option<String>,
}

impl ChatMessage {
    fn new(role: Role, text: String, state: DocState) -> Self {
        Self {
            role,
            text,
            tool_calls: Vec::new(),
            applied_revisions: AppliedRevisions::at(state.revision),
            applied_undo_depth: AppliedUndoDepth::at(state.undo_depth),
            error: None,
        }
    }

    /// このターンで確定した編集コマンド数(= 元に戻すのに必要なundo回数)。
    ///
    /// undo履歴の深さの増分。ターン中にundo/redoが混ざっても過不足なく数えられる。
    pub fn applied_command_count(&self) -> u64 {
        self.applied_undo_depth.count()
    }

    pub fn has_edits(&self) -> bool {
        self.applied_command_count() > 0
    }

    /// ターン終了時点のドキュメント状態を記録する。
    pub fn finish_turn(&mut self, state: DocState) {
        self.applied_revisions.end = state.revision;
        self.applied_undo_depth.end = state.undo_depth;
    }

    /// `count`回分の巻き戻しを記録に反映する(全部戻せば「適用済み」でなくなる)。
    ///
    /// undoが途中で失敗しても、実行できた回数だけ反映してから返すことで
    /// リトライ時に過剰undoにならないようにする。
    pub fn record_undone(&mut self, count: u64) {
        self.applied_undo_depth.end = self.applied_undo_depth.end.saturating_sub(count);
        if self.applied_command_count() == 0 {
            self.applied_revisions = AppliedRevisions::at(self.applied_revisions.start);
        } else {
            self.applied_revisions.end = self.applied_revisions.end.saturating_sub(count);
        }
    }
}

/// UNIXエポックからのミリ秒。システム時計が1970より前を指す異常時は`0`(=不明)。
fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 1本の会話(CLIセッションと1:1)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Conversation {
    pub id: Uuid,
    /// claude CLIのsession_id(`--resume`で継続する)
    pub session_id: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub model: Option<String>,
    /// 最終更新時刻(unixミリ秒)。会話履歴の相対時刻表示に使う。
    ///
    /// この項目が無い旧`chat.json`は`0`になる。UI側は`0`を「時刻不明」として
    /// 扱い、相対時刻の表示を省く(1970年と表示させない)。
    #[serde(default)]
    pub updated_at: u64,
}

impl Default for Conversation {
    fn default() -> Self {
        Self::new()
    }
}

impl Conversation {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            session_id: None,
            messages: Vec::new(),
            model: None,
            updated_at: now_millis(),
        }
    }

    /// 最終更新時刻を現在時刻にする。会話の内容が変わる操作は必ずこれを通す。
    pub fn touch(&mut self) {
        self.updated_at = now_millis();
    }

    /// ターンを開始する。ユーザー発話と、これから埋めるアシスタント応答を積む。
    ///
    /// `state`は送信直前のドキュメント状態。
    pub fn begin_turn(&mut self, prompt: &str, state: DocState) {
        self.touch();
        self.messages
            .push(ChatMessage::new(Role::User, prompt.to_string(), state));
        self.messages
            .push(ChatMessage::new(Role::Assistant, String::new(), state));
    }

    /// ストリーム中のイベントを現在のターンへ反映する。
    ///
    /// `state`はイベント受信時点のドキュメント状態(ターン終了時の記録に使う)。
    pub fn apply_event(&mut self, event: &AgentEvent, state: DocState) {
        self.touch();
        if let AgentEvent::SessionStarted { session_id } = event {
            self.session_id = Some(session_id.clone());
            return;
        }
        let Some(message) = self.current_turn_mut() else {
            return;
        };
        match event {
            AgentEvent::SessionStarted { .. } => {}
            AgentEvent::TextDelta { text } => message.text.push_str(text),
            AgentEvent::ToolUseStarted { id, tool, input } => message.tool_calls.push(ToolCall {
                id: id.clone(),
                tool: tool.clone(),
                input: input.clone(),
                finished: false,
                is_error: false,
            }),
            AgentEvent::ToolUseFinished { id, tool, is_error } => {
                if let Some(call) = message
                    .tool_calls
                    .iter_mut()
                    .find(|c| c.id == *id || (id.is_empty() && c.tool == *tool && !c.finished))
                {
                    call.finished = true;
                    call.is_error = *is_error;
                }
            }
            AgentEvent::TurnCompleted { result, .. } => {
                // デルタが来ない構成(--include-partial-messages無し)でも本文を埋める
                if message.text.is_empty() {
                    message.text = result.clone();
                }
                message.finish_turn(state);
            }
            // マネージャがターン確定後に合成するだけの通知。会話状態は既に更新済み
            AgentEvent::TurnApplied { .. } => {}
            AgentEvent::Error { message: err } => {
                message.error = Some(err.clone());
                message.finish_turn(state);
            }
        }
    }

    /// 進行中(または直近)のアシスタントメッセージ。
    pub fn current_turn_mut(&mut self) -> Option<&mut ChatMessage> {
        self.messages
            .iter_mut()
            .rev()
            .find(|m| m.role == Role::Assistant)
    }

    pub fn last_turn(&self) -> Option<&ChatMessage> {
        self.messages
            .iter()
            .rev()
            .find(|m| m.role == Role::Assistant)
    }
}

/// チャット履歴ファイルの中身。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatFile {
    format_version: u32,
    conversations: Vec<Conversation>,
}

/// プロジェクトファイルに対応するチャット履歴のパス(`<stem>.chat.json`)。
pub fn chat_path_for(project_path: &Path) -> PathBuf {
    project_path.with_extension("chat.json")
}

/// チャット履歴を整形JSONで保存する(同ディレクトリの一時ファイル→renameでアトミックに)。
pub fn save_chat(path: &Path, conversations: &[Conversation]) -> Result<()> {
    let file = ChatFile {
        format_version: CHAT_FORMAT_VERSION,
        conversations: conversations.to_vec(),
    };
    crate::write_atomic(path, &serde_json::to_string_pretty(&file)?)
}

/// チャット履歴を読み込む。ファイルが無ければ空(新規プロジェクト)。
///
/// このビルドより新しい`format_version`のファイルは、黙って読み違えるより
/// 明示エラーにする(未知フィールドを落として上書き保存する事故を防ぐ)。
pub fn load_chat(path: &Path) -> Result<Vec<Conversation>> {
    let json = match std::fs::read_to_string(path) {
        Ok(json) => json,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let file: ChatFile = serde_json::from_str(&json)?;
    if file.format_version > CHAT_FORMAT_VERSION {
        return Err(AgentError::UnsupportedChatFormat {
            found: file.format_version,
            supported: CHAT_FORMAT_VERSION,
        });
    }
    Ok(file.conversations)
}
