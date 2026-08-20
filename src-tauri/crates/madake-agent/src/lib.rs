//! madake-agent: MadakeCADのAIエージェント基盤。
//!
//! ローカルのClaude Code CLIをヘッドレス(`--output-format stream-json`)で起動し、
//! 出力を [`events::AgentEvent`] のストリームへ変換する。図面の編集自体はCLIが
//! 内蔵MCPサーバー(9310)のツールを呼ぶことで行われるため、全ての編集は
//! madake-coreのCommandエンジンを通る。

pub mod backend;
pub mod conversation;
pub mod events;

pub use backend::{ClaudeCodeCliBackend, DetectResult};
pub use conversation::{AppliedRevisions, ChatMessage, Conversation, Role, ToolCall};
pub use events::{parse_stream_events, parse_stream_line, AgentEvent, StreamParser, Usage};

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
}

pub type Result<T> = std::result::Result<T, AgentError>;
