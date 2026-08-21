//! madake-agent: MadakeCADのAIエージェント基盤。
//!
//! ローカルのClaude Code CLIをヘッドレス(`--output-format stream-json`)で起動し、
//! 出力を [`events::AgentEvent`] のストリームへ変換する。図面の編集自体はCLIが
//! 内蔵MCPサーバー(9310)のツールを呼ぶことで行われるため、全ての編集は
//! madake-coreのCommandエンジンを通る。

pub mod backend;
pub mod conversation;
pub mod events;
pub mod manager;

pub use backend::{ClaudeCodeCliBackend, DetectResult};
pub use conversation::{
    chat_path_for, load_chat, save_chat, AppliedRevisions, AppliedUndoDepth, ChatMessage,
    Conversation, DocState, Role, ToolCall,
};
pub use events::{parse_stream_events, parse_stream_line, AgentEvent, StreamParser, Usage};
pub use manager::{AgentManager, ConversationEvent, DocBridge};

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
    #[error("メッセージが見つかりません: {0}")]
    NoMessage(usize),
    #[error(
        "最新の適用済みターンではないため巻き戻せません: {0} \
         (後続の編集を先に取り消してください)"
    )]
    NotLatestTurn(usize),
    #[error("ドキュメント操作に失敗しました: {0}")]
    Doc(String),
}

pub type Result<T> = std::result::Result<T, AgentError>;
