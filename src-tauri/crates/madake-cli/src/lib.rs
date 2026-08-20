//! `madake` CLI の本体。
//!
//! 起動中のMadakeCADアプリが提供するLink API (`http://127.0.0.1:<port>/api/v1`) を
//! 叩くだけの薄いクライアント。編集系は必ず `POST /api/v1/commands` (Commandエンジン)
//! を経由するため、undo/redo履歴とpatch配信はアプリ内操作と完全に整合する。

pub mod cli;
pub mod client;
pub mod format;

pub use cli::{run, Cli, Commands};
pub use client::{CliError, ExportKind, HttpClient, LinkApi, DEFAULT_PORT};
