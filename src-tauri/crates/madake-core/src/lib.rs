//! madake-core: MadakeCADのドキュメントモデルとCommandエンジン。
//!
//! UIにもTauriにも依存しない。全ての編集はシリアライズ可能な [`command::Command`] として
//! [`command::Engine`] で実行され、UI(Tauri IPC)とAI(MCPサーバー)が同じAPIを共有する。

pub mod block;
pub mod cable;
pub mod cable_run;
pub mod command;
pub mod dxf;
pub mod frame_template;
pub mod geometry;
pub mod harness;
pub mod io;
pub mod kicad;
pub mod leader;
pub mod macros;
pub mod mate;
pub mod model;
pub mod net_tag;
pub mod netlist;
pub mod ngspice;
pub mod panel;
pub mod panel_wiring;
pub mod parts;
pub mod pdf;
pub mod pin_groups;
pub mod pin_wiring;
pub mod raster;
pub mod plc;
pub mod relay_xref;
pub mod report_sheet;
pub mod reports;
pub mod search;
pub mod shape;
pub mod sim;
pub mod spice;
pub mod svg;
pub mod symbol;
pub mod symbol_library;
pub mod templates;
pub mod terminal_chart;
pub mod terminal_diagram;
pub mod text_style;
pub mod tidy;
pub mod variants;
pub mod verify;
pub mod wire_legend;
pub mod wire_no;
pub mod wire_note;
pub mod wire_parts;
pub mod xref;

pub use command::{Command, EditOrigin, Engine, Patch, PatchOp, Reverted};
pub use geometry::Point;
pub use io::ExportReport;
pub use model::*;
pub use wire_no::RenumberMode;
pub use net_tag::{net_tag, NetTag, TagDir};
pub use wire_note::{wire_note_text, wire_notes, WireNote};
pub use wire_parts::{wire_part_number, WIRE_PARTS_COLUMNS};
pub use block::{block_symbol, BLOCK_SYMBOL_ID};
pub use symbol::{
    builtin_symbols, dynamic_symbol, instance_symbol_def, resolve_symbol, sheet_symbol_defs, DefIndex,
    PinDef, Primitive, SymbolDef,
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
    /// 巻き戻し([`command::Engine::revert_range`])の逆適用が現在の図面と衝突した。
    /// 図面は変更されていない(部分適用しない)。
    #[error("revert conflict: {0}")]
    RevertConflict(String),
    /// プロジェクトファイルがこのアプリより新しい形式で保存されている ([`crate::io::load_project`])。
    #[error("this file uses project format {found}, but this MadakeCAD reads up to format {supported}; update MadakeCAD to open it")]
    NewerFormat { found: u32, supported: u32 },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CoreError>;
