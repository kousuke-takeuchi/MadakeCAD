//! 会話・ターン管理と、チャット履歴の永続化。
//!
//! ターン中にエージェントがMCP経由で実行した編集はCommandエンジンのrevisionを進める。
//! ターン開始/終了時のrevisionを [`ChatMessage::applied_revisions`] に記録しておくと、
//! 「元に戻す」は差分回数だけ`undo`を呼べばよい。
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

/// ターンの前後で挟んだEngineのrevision範囲。
///
/// `end - start`がこのターンで確定した編集コマンド数(= 元に戻すのに必要なundo回数)。
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
    /// ターン開始/終了時のEngine revision
    pub applied_revisions: AppliedRevisions,
    #[serde(default)]
    pub error: Option<String>,
}

impl ChatMessage {
    fn new(role: Role, text: String, revision: u64) -> Self {
        Self {
            role,
            text,
            tool_calls: Vec::new(),
            applied_revisions: AppliedRevisions::at(revision),
            error: None,
        }
    }

    /// このターンで確定した編集コマンド数(= 元に戻すのに必要なundo回数)。
    pub fn applied_command_count(&self) -> u64 {
        self.applied_revisions.count()
    }

    pub fn has_edits(&self) -> bool {
        self.applied_command_count() > 0
    }
}

/// 1本の会話(CLIセッションと1:1)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Conversation {
    pub id: Uuid,
    /// claude CLIのsession_id(`--resume`で継続する)
    pub session_id: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub model: Option<String>,
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
        }
    }

    /// ターンを開始する。ユーザー発話と、これから埋めるアシスタント応答を積む。
    ///
    /// `revision`は送信直前のEngineのrevision。
    pub fn begin_turn(&mut self, prompt: &str, revision: u64) {
        self.messages
            .push(ChatMessage::new(Role::User, prompt.to_string(), revision));
        self.messages
            .push(ChatMessage::new(Role::Assistant, String::new(), revision));
    }

    /// ストリーム中のイベントを現在のターンへ反映する。
    ///
    /// `revision`はイベント受信時点のEngineのrevision(ターン終了時の記録に使う)。
    pub fn apply_event(&mut self, event: &AgentEvent, revision: u64) {
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
                message.applied_revisions.end = revision;
            }
            AgentEvent::Error { message: err } => {
                message.error = Some(err.clone());
                message.applied_revisions.end = revision;
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
    let json = serde_json::to_string_pretty(&file)?;

    // renameを同一ファイルシステム内に閉じるため、一時ファイルは保存先と同じ親へ置く
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let temp = dir.join(format!(".{}.tmp", Uuid::new_v4()));
    if let Err(e) = std::fs::write(&temp, &json) {
        let _ = std::fs::remove_file(&temp);
        return Err(e.into());
    }
    if let Err(e) = std::fs::rename(&temp, path) {
        let _ = std::fs::remove_file(&temp);
        return Err(e.into());
    }
    Ok(())
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
