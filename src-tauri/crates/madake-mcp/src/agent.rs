//! AIエージェント連携: [`SharedDoc`]とmadake-agentの接続。
//!
//! - [`SharedDocBridge`][]: マネージャからドキュメントを触るための最小の窓口。
//!   **新しい編集経路は作らない**: undoは既存の[`SharedDoc::undo`](crate::SharedDoc::undo)を
//!   呼ぶだけで、patch配信・undo履歴はUI/MCPと完全に共通。
//! - [`drawing_context`][]: 送信のたびに`--append-system-prompt`へ渡す図面サマリ。

use std::sync::Arc;

use madake_agent::{AgentManager, DocBridge};
use madake_core::{builtin_symbols, Entity};

use crate::SharedDoc;

/// [`SharedDoc`]をmadake-agentの[`DocBridge`]として見せるアダプタ。
pub struct SharedDocBridge(pub SharedDoc);

impl DocBridge for SharedDocBridge {
    fn revision(&self) -> u64 {
        self.0.engine.lock().unwrap().revision()
    }

    fn undo(&self) -> Result<bool, String> {
        self.0
            .undo()
            .map(|patch| patch.is_some())
            .map_err(|e| e.to_string())
    }
}

/// ドキュメントに結びついたエージェントマネージャを作る。
pub fn manager(doc: &SharedDoc, mcp_port: u16) -> Arc<AgentManager> {
    Arc::new(AgentManager::new(
        Arc::new(SharedDocBridge(doc.clone())),
        mcp_port,
    ))
}

/// エージェントへ毎ターン渡す図面コンテキスト(`--append-system-prompt`)。
///
/// アクティブシートの概念はUI側にしか無いため、ここでは先頭シートを既定とする。
pub fn drawing_context(doc: &SharedDoc) -> String {
    let engine = doc.engine.lock().unwrap();
    let project = engine.project();
    let mut out = String::new();
    out.push_str(
        "あなたはMadakeCAD(産業用電気図面CAD)に組み込まれた図面編集エージェントです。\
         ユーザーの指示に従って、開いている図面を読み取り・編集してください。\n\n",
    );

    out.push_str("## 現在の図面\n");
    out.push_str(&format!("- プロジェクト名: {}\n", project.name));
    out.push_str(&format!("- リビジョン: {}\n", engine.revision()));
    match project.sheets.first() {
        Some(active) => {
            out.push_str(&format!(
                "- アクティブシート: {} ({})\n",
                active.id, active.name
            ));
            let nets = madake_core::netlist::extract_netlist(active, &builtin_symbols());
            out.push_str(&format!(
                "- アクティブシートのネット数: {} / エンティティ数: {}\n",
                nets.len(),
                active.entities.len()
            ));
        }
        None => out.push_str("- アクティブシート: なし(シートが1枚もありません)\n"),
    }
    out.push_str("- シート一覧:\n");
    for sheet in &project.sheets {
        let (symbols, wires) = sheet
            .entities
            .values()
            .fold((0usize, 0usize), |(s, w), entity| match entity {
                Entity::Symbol(_) => (s + 1, w),
                Entity::Wire(_) => (s, w + 1),
                _ => (s, w),
            });
        out.push_str(&format!(
            "  - {} \"{}\" ({:?} {:?}, シンボル{}個, 配線{}本)\n",
            sheet.id, sheet.name, sheet.size, sheet.orientation, symbols, wires
        ));
    }

    out.push_str(
        "\n## 座標系\n\
         - 用紙座標系はmm単位・左上原点・Y下向き\n\
         - グリッド/ピンピッチは2.5mm。シンボルのピンは必ず2.5mmグリッド上に載せること\n\
         - 回転は0/90/180/270のみ\n\
         - 線径は「sq」(mm2断面積: 0.3 / 0.75 / 3.5 など)\n",
    );

    out.push_str(
        "\n## ツール\n\
         - 図面の読み書きは必ずMCPサーバー\"madakecad\"のツール(mcp__madakecad__*)で行うこと\n\
         - .mdkprojファイルを直接編集してはならない(全ての編集はCommandエンジンを通す必要がある)\n\
         - ツール経由の編集はundo履歴に乗り、UIへ即座に反映される\n\
         - 位置やシートが不明なときは、まずget_project / get_netlist / list_symbolsで現状を確認すること\n",
    );
    out
}
