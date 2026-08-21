//! madake-core: MadakeCADのドキュメントモデルとCommandエンジン。
//!
//! UIにもTauriにも依存しない。全ての編集はシリアライズ可能な [`command::Command`] として
//! [`command::Engine`] で実行され、UI(Tauri IPC)とAI(MCPサーバー)が同じAPIを共有する。

pub mod command;
pub mod geometry;
pub mod io;
pub mod kicad;
pub mod model;
pub mod netlist;
pub mod ngspice;
pub mod parts;
pub mod pdf;
pub mod reports;
pub mod sim;
pub mod spice;
pub mod svg;
pub mod symbol;
pub mod verify;
pub mod wire_no;

pub use command::{Command, Engine, Patch, PatchOp};
pub use geometry::Point;
pub use model::*;
pub use wire_no::RenumberMode;
pub use symbol::{
    builtin_symbols, dynamic_symbol, resolve_symbol, sheet_symbol_defs, PinDef, Primitive,
    SymbolDef,
};

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("sheet not found: {0}")]
    SheetNotFound(uuid::Uuid),
    #[error("entity not found: {0}")]
    EntityNotFound(uuid::Uuid),
    #[error("symbol not found in library: {0}")]
    SymbolNotFound(String),
    #[error("invalid command: {0}")]
    InvalidCommand(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CoreError>;
