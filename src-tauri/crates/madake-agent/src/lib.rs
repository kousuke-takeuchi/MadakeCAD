//! madake-agent: MadakeCADのAIエージェント基盤。
//!
//! ローカルのClaude Code CLIをヘッドレス(`--output-format stream-json`)で起動し、
//! 出力を [`events::AgentEvent`] のストリームへ変換する。図面の編集自体はCLIが
//! 内蔵MCPサーバー(9310)のツールを呼ぶことで行われるため、全ての編集は
//! madake-coreのCommandエンジンを通る。

pub mod backend;
pub mod conversation;
pub mod events;
pub mod knowledge;
pub mod manager;
pub mod settings;

pub use backend::{ClaudeCodeCliBackend, DetectResult};
pub use conversation::{
    chat_path_for, load_chat, save_chat, AppliedRevisions, AppliedUndoDepth, ChatMessage,
    Conversation, DocState, Role, ToolCall,
};
pub use events::{parse_stream_events, parse_stream_line, AgentEvent, StreamParser, Usage};
pub use knowledge::{standards_text, system_prompt};
pub use manager::{AgentManager, ConversationEvent, DocBridge, RevertError, RevertReport};
pub use settings::{load_settings, save_settings, settings_path, AppSettings};

/// madake-agentのエラー。
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("claude CLIが見つかりません: {0}")]
    NotFound(String),
    #[error("claude CLIの起動に失敗しました: {0}")]
    Spawn(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error(
        "このビルドが対応していないチャット履歴フォーマット版です: {found} (対応: {supported}以下)"
    )]
    UnsupportedChatFormat { found: u32, supported: u32 },
    #[error("この会話は送信中です")]
    Busy,
    #[error("会話が見つかりません: {0}")]
    NoConversation(uuid::Uuid),
    #[error("ターンが見つかりません: {0}")]
    UnknownTurn(uuid::Uuid),
    #[error("このターンには巻き戻せる編集がありません(既に巻き戻し済みです): {0}")]
    TurnNotApplied(uuid::Uuid),
    #[error(
        "このターンの編集は現在の図面と衝突するため巻き戻せません: {turn_id} \
         (図面は変更していません: {detail})"
    )]
    TurnConflict { turn_id: uuid::Uuid, detail: String },
    #[error("ドキュメント操作に失敗しました: {0}")]
    Doc(String),
    #[error("設定ファイルの場所を特定できません(ホームディレクトリが不明です)")]
    NoSettingsPath,
}

pub type Result<T> = std::result::Result<T, AgentError>;

/// 一時ファイル→renameでアトミックに書き出す。
///
/// 途中で失敗しても、書きかけの内容で既存ファイルを壊さない。renameを同一
/// ファイルシステム内に閉じるため、一時ファイルは保存先と同じ親ディレクトリへ置く。
pub(crate) fn write_atomic(path: &std::path::Path, contents: &str) -> Result<()> {
    let dir = path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let temp = dir.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
    if let Err(e) = std::fs::write(&temp, contents) {
        let _ = std::fs::remove_file(&temp);
        return Err(e.into());
    }
    if let Err(e) = std::fs::rename(&temp, path) {
        let _ = std::fs::remove_file(&temp);
        return Err(e.into());
    }
    Ok(())
}
