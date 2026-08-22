//! AIエージェント連携: [`SharedDoc`]とmadake-agentの接続。
//!
//! - [`SharedDocBridge`][]: マネージャからドキュメントを触るための最小の窓口。
//!   **新しい編集経路は作らない**: undoは既存の[`SharedDoc::undo`](crate::SharedDoc::undo)を
//!   呼ぶだけで、patch配信・undo履歴はUI/MCPと完全に共通。
//! - [`drawing_context`][]: 送信のたびに`--append-system-prompt`へ渡す図面サマリ。
//! - [`load_project_with_chat`][] / [`save_project_with_chat`][]: 図面とチャット履歴を
//!   セットで読み書きする共通処理(Tauri IPCとLink APIの両方がこれを呼ぶ)。
//! - [`load_and_apply_settings`][] / [`update_settings`][]: アプリ設定
//!   (`~/.madakecad/settings.json`)とマネージャの同期。

use std::path::Path;
use std::sync::Arc;

use madake_agent::{
    AgentManager, AppSettings, Conversation, DocBridge, DocState, RevertError, RevertReport,
};
use madake_core::{sheet_symbol_defs, Entity, Patch};

use crate::SharedDoc;

/// [`SharedDoc`]をmadake-agentの[`DocBridge`]として見せるアダプタ。
pub struct SharedDocBridge(pub SharedDoc);

impl DocBridge for SharedDocBridge {
    fn revision(&self) -> u64 {
        self.0.engine.lock().unwrap().revision()
    }

    fn undo_depth(&self) -> u64 {
        self.0.engine.lock().unwrap().undo_depth() as u64
    }

    fn state(&self) -> DocState {
        let engine = self.0.engine.lock().unwrap();
        DocState::new(engine.revision(), engine.undo_depth() as u64)
    }

    fn begin_agent_turn(&self) {
        self.0.begin_agent_turn();
    }

    fn end_agent_turn(&self) {
        self.0.end_agent_turn();
    }

    fn revert_agent_edits(
        &self,
        start_depth: u64,
        end_depth: u64,
    ) -> Result<RevertReport, RevertError> {
        match self
            .0
            .revert_agent_edits(start_depth as usize, end_depth as usize)
        {
            Ok(reverted) => Ok(RevertReport {
                reverted: reverted.as_ref().map(|r| r.commands as u64).unwrap_or(0),
                revision: reverted
                    .map(|r| r.patch.revision)
                    .unwrap_or_else(|| self.revision()),
            }),
            // 逆適用が現在の図面と衝突した(対象が手編集で消えている等)。図面は無変更
            Err(madake_core::CoreError::RevertConflict(detail)) => {
                Err(RevertError::Conflict(detail))
            }
            Err(e) => Err(RevertError::Failed(e.to_string())),
        }
    }
}

/// ドキュメントに結びついたエージェントマネージャを作る。
pub fn manager(doc: &SharedDoc, mcp_port: u16) -> Arc<AgentManager> {
    Arc::new(AgentManager::new(
        Arc::new(SharedDocBridge(doc.clone())),
        mcp_port,
    ))
}

/// 設定ファイルを読み、エージェントへ反映する(起動時に1回呼ぶ)。
///
/// 読めない設定でアプリが起動できなくなるのは困るため、失敗しても既定値で続行する。
pub fn load_and_apply_settings(agent: &AgentManager) -> AppSettings {
    let settings = madake_agent::settings::load_default_settings().unwrap_or_else(|e| {
        eprintln!("設定の読込に失敗しました(既定値で続行します): {e}");
        AppSettings::default()
    });
    agent.apply_settings(settings.clone());
    settings
}

/// 設定を保存し、エージェントへ反映する(Tauri IPCとLink APIの共通処理)。
///
/// 戻り値は正規化後の設定(UIはこれで表示を更新する)。反映は次の送信から有効。
pub fn update_settings(agent: &AgentManager, settings: AppSettings) -> Result<AppSettings, String> {
    let settings = settings.normalized();
    madake_agent::settings::save_default_settings(&settings).map_err(|e| e.to_string())?;
    agent.apply_settings(settings.clone());
    Ok(settings)
}

/// プロジェクトを読み込み、隣のチャット履歴(`<stem>.chat.json`)へ会話を差し替える。
///
/// 実行中のターンは先に中断する。走っているCLIが「読み込む前の図面」を前提に
/// 新しいプロジェクトを編集してしまうため、プロジェクト置換より前に止める必要がある。
pub fn load_project_with_chat(
    doc: &SharedDoc,
    agent: &AgentManager,
    path: &Path,
) -> Result<Patch, String> {
    let project = madake_core::io::load_project(path).map_err(|e| e.to_string())?;
    agent.cancel_all();
    let patch = doc.engine.lock().unwrap().replace_project(project);
    let _ = doc.patches.send(patch.clone());
    agent.set_conversations(load_chat_beside(path));
    Ok(patch)
}

/// KiCad回路図(.kicad_sch)を読み込み、プロジェクトを置き換える。
/// 実行中のターンは中断し、チャット履歴は空になる(新規プロジェクト扱い)。
pub fn import_kicad_with_chat(
    doc: &SharedDoc,
    agent: &AgentManager,
    path: &Path,
) -> Result<(Patch, madake_core::kicad::ImportReport), String> {
    let input = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "KiCadインポート".into());
    let (project, report) =
        madake_core::kicad::import_kicad_sch(&input, &name).map_err(|e| e.to_string())?;
    agent.cancel_all();
    let patch = doc.engine.lock().unwrap().replace_project(project);
    let _ = doc.patches.send(patch.clone());
    agent.set_conversations(Vec::new());
    Ok((patch, report))
}

/// プロジェクトを保存し、隣へチャット履歴も書き出す。
pub fn save_project_with_chat(
    doc: &SharedDoc,
    agent: &AgentManager,
    path: &Path,
) -> Result<(), String> {
    {
        let engine = doc.engine.lock().unwrap();
        madake_core::io::save_project(path, engine.project()).map_err(|e| e.to_string())?;
    }
    save_chat_beside(agent, path);
    Ok(())
}

/// プロジェクトの隣(`<stem>.chat.json`)へチャット履歴を保存する。
///
/// 会話が1本も無ければファイルを作らない(空ファイルを撒かない)。
pub fn save_chat_beside(agent: &AgentManager, project_path: &Path) {
    let conversations = agent.conversations();
    if conversations.is_empty() {
        return;
    }
    let chat_path = madake_agent::chat_path_for(project_path);
    if let Err(e) = madake_agent::save_chat(&chat_path, &conversations) {
        // 図面本体の保存は成功しているので、失敗しても保存操作自体は失敗させない
        eprintln!(
            "チャット履歴の保存に失敗しました ({}): {e}",
            chat_path.display()
        );
    }
}

/// プロジェクトの隣のチャット履歴を読む(無ければ空)。
pub fn load_chat_beside(project_path: &Path) -> Vec<Conversation> {
    let chat_path = madake_agent::chat_path_for(project_path);
    match madake_agent::load_chat(&chat_path) {
        Ok(conversations) => conversations,
        Err(e) => {
            eprintln!(
                "チャット履歴の読込に失敗しました ({}): {e}",
                chat_path.display()
            );
            Vec::new()
        }
    }
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
            let nets = madake_core::netlist::extract_netlist(active, &sheet_symbol_defs(active));
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
