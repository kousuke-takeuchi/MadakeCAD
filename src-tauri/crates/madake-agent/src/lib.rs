//! madake-agent: MadakeCADのAIエージェント基盤。
//!
//! ローカルのClaude Code CLIをヘッドレス(`--output-format stream-json`)で起動し、
//! 出力を [`events::AgentEvent`] のストリームへ変換する。図面の編集自体はCLIが
//! 内蔵MCPサーバー(9310)のツールを呼ぶことで行われるため、全ての編集は
//! madake-coreのCommandエンジンを通る。

pub mod events;

pub use events::{parse_stream_events, parse_stream_line, AgentEvent, StreamParser, Usage};
