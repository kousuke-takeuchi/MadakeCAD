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
///
/// これだけではAnthropic API直結バックエンドが図面を編集できない
/// (ツールの窓口が無い)。アプリ本体は[`manager_with_tools`]を使うこと。
pub fn manager(doc: &SharedDoc, mcp_port: u16) -> Arc<AgentManager> {
    Arc::new(AgentManager::new(
        Arc::new(SharedDocBridge(doc.clone())),
        mcp_port,
    ))
}

/// 図面編集ツールの窓口までつないだエージェントマネージャを作る(アプリ本体用)。
///
/// 窓口([`crate::tool_bridge::McpToolBridge`])は内蔵MCPサーバーをプロセス内で呼ぶので、
/// Anthropic API直結バックエンドの編集もCommandエンジン・undo履歴・patch配信を通る。
/// Claude Code CLIバックエンドはCLI自身が`/mcp`へ接続するため窓口は使わない。
pub fn manager_with_tools(
    doc: &SharedDoc,
    parts: &crate::SharedParts,
    mcp_port: u16,
) -> Arc<AgentManager> {
    let manager = manager(doc, mcp_port);
    manager.set_tool_bridge(crate::tool_bridge::McpToolBridge::shared(doc, parts));
    manager
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

/// プロバイダの状態(設定画面のバッジ・「保存済み」表示用)。
///
/// **APIキーそのものは絶対に含めない**。返すのは「保存されているか」だけ。
/// UI・Link API・Tauri IPCが同じ形を見るように、組み立てはここに1本化する。
pub fn provider_status(agent: &AgentManager) -> serde_json::Value {
    // 同期版はCLIを起動しない(Copilotの検出は非同期版だけが行う)
    provider_status_with(agent, read_saved_keys(), None)
}

/// キーチェーンから「保存済みか」だけを読む。**値は持ち出さない。**
///
/// 戻り値はプロバイダごとの「保存済みか」と、読めなかった理由。
pub fn read_saved_keys() -> SavedKeys {
    let (anthropic, anthropic_error) = madake_agent::secrets::anthropic_api_key_checked();
    let (openai, openai_error) = madake_agent::secrets::openai_compat_api_key_checked();
    let (gemini, gemini_error) = madake_agent::secrets::gemini_api_key_checked();
    SavedKeys {
        api_key_saved: anthropic.is_some(),
        openai_key_saved: openai.is_some(),
        gemini_key_saved: gemini.is_some(),
        keychain_error: anthropic_error.or(openai_error).or(gemini_error),
    }
}

/// プロバイダごとの「キーが保存済みか」。**キーの値は含まない。**
#[derive(Debug, Clone, Default)]
pub struct SavedKeys {
    pub api_key_saved: bool,
    pub openai_key_saved: bool,
    pub gemini_key_saved: bool,
    /// OSキーチェーンが読めなかった理由(読めたときはNone)
    pub keychain_error: Option<String>,
}

/// キーチェーンを読まずに状態を組み立てる(読み出し結果は呼び出し側が渡す)。
///
/// `copilot`はcopilot CLIの検出結果で3状態:
/// `None`=まだ調べていない(`copilot_detected`はnull。「無い」と言い切らない)、
/// `Some(None)`=調べたが見つからない、`Some(Some(_))`=見つかった。
fn provider_status_with(
    agent: &AgentManager,
    keys: SavedKeys,
    copilot: Option<Option<madake_agent::DetectResult>>,
) -> serde_json::Value {
    let settings = agent.settings().normalized();
    serde_json::json!({
        "provider": settings.provider,
        "api_model": settings.api_model,
        "api_key_saved": keys.api_key_saved,
        // キーチェーンが読めなかった理由(読めたときはnull)。保存したのに「未設定」と
        // 出る状況を黙って放置しないため、設定画面へそのまま出す
        "keychain_error": keys.keychain_error,
        // GitHub Copilot CLI。**資格情報は含めない**(Copilot自身のサインインを使う)
        "copilot_model": settings.copilot_model,
        "copilot_path": settings.copilot_path,
        "copilot_detected": copilot.as_ref().map(Option::is_some),
        "copilot_version": copilot.flatten().map(|found| found.version),
        // OpenAI互換API。**キーの値は返さない**(保存済みかどうかだけ)
        "openai_base_url": settings.openai_base_url,
        "openai_model": settings.openai_model,
        "openai_key_saved": keys.openai_key_saved,
        // Google Gemini。**キーの値は返さない**(保存済みかどうかだけ)
        "gemini_model": settings.gemini_model,
        "gemini_key_saved": keys.gemini_key_saved,
    })
}

/// キーチェーンの許可待ちで設定画面が固まらないよう、待つのをやめるまでの時間。
pub const KEYCHAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// [`provider_status`]の非同期版(Link API・Tauri IPCが使う)。
///
/// キーチェーンの読み出しは**OSの許可ダイアログで止まることがある**ため、
/// ブロッキング用スレッドで行い、返事が来なければ理由つきで諦める
/// (待ち続けると設定画面が開いたまま固まる)。読み出しはプロセスに1回だけなので、
/// 一度許可すれば以降は待ち時間なしで返る。
pub async fn provider_status_async(agent: &Arc<AgentManager>) -> serde_json::Value {
    let job = tokio::task::spawn_blocking(read_saved_keys);
    // キーチェーンの読み出し(別スレッドで進行中)と並行にCopilot CLIを検出する
    let copilot = Some(agent.detect_copilot().await.ok());
    let keys = match tokio::time::timeout(KEYCHAIN_TIMEOUT, job).await {
        Ok(Ok(keys)) => keys,
        Ok(Err(e)) => SavedKeys {
            keychain_error: Some(e.to_string()),
            ..SavedKeys::default()
        },
        Err(_) => SavedKeys {
            keychain_error: Some(format!(
                "OSキーチェーンの読み出しが{}秒たっても終わりませんでした(OSの許可を求める\
                 ダイアログが出ていないか確認してください)",
                KEYCHAIN_TIMEOUT.as_secs()
            )),
            ..SavedKeys::default()
        },
    };
    provider_status_with(agent, keys, copilot)
}

/// 設定画面の「APIキー」欄がどのプロバイダのものかを表す名前 → キーチェーンの保管名。
///
/// 対応する名前は`anthropic_api` / `openai_compat` / `gemini`。
///
/// 未知の名前(古いUI・省略時)はAnthropicとして扱う(これまでの動作のまま)。
pub fn key_account_for(provider: Option<&str>) -> &'static str {
    match provider.map(str::trim) {
        Some("openai_compat") => madake_agent::secrets::OPENAI_COMPAT_ACCOUNT,
        Some("gemini") => madake_agent::secrets::GEMINI_ACCOUNT,
        _ => madake_agent::secrets::ANTHROPIC_ACCOUNT,
    }
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

/// DXF (AutoCAD Electrical / EPLAN の中間形式) を読み込み、プロジェクトを置き換える。
/// 実行中のターンは中断し、チャット履歴は空になる(新規プロジェクト扱い)。
pub fn import_dxf_with_chat(
    doc: &SharedDoc,
    agent: &AgentManager,
    path: &Path,
    options: &madake_core::dxf::DxfImportOptions,
) -> Result<(Patch, madake_core::kicad::ImportReport), String> {
    let input = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "DXFインポート".into());
    let (project, report) =
        madake_core::dxf::import_dxf(&input, &name, options).map_err(|e| e.to_string())?;
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

    out.push_str(&verification_summary(project));

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

/// 図面コンテキストに載せる検証サマリの先頭何件を列挙するか。
const CONTEXT_DIAGNOSTICS: usize = 3;

/// ターン開始時点の検証結果の要約(重要度ごとの件数+先頭数件)。
///
/// エージェントが「今どこが壊れているか」を最初から知っている状態にするための行。
/// 全件は`run_verification`で取り直させる(ここに全部並べるとプロンプトが膨らむ)。
fn verification_summary(project: &madake_core::model::Project) -> String {
    use madake_core::verify::Severity;

    let diags = madake_core::verify::verify_project(project);
    let count = |severity: Severity| diags.iter().filter(|d| d.severity == severity).count();
    let (errors, warnings, infos) = (
        count(Severity::Error),
        count(Severity::Warning),
        count(Severity::Info),
    );

    let mut out = format!(
        "\n## 現在の検証結果(ターン開始時点)\n\
         - エラー {errors}件 / 警告 {warnings}件 / 情報 {infos}件\n"
    );
    if diags.is_empty() {
        out.push_str("- 指摘なし\n");
        return out;
    }
    // 重要度の高い順(エラー→警告→情報)に先頭数件だけ
    let mut ordered: Vec<_> = diags.iter().collect();
    ordered.sort_by_key(|d| match d.severity {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::Info => 2,
    });
    for diag in ordered.iter().take(CONTEXT_DIAGNOSTICS) {
        out.push_str(&format!(
            "  - [{}] {}: {}\n",
            match diag.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info => "info",
            },
            diag.code,
            diag.message
        ));
    }
    if diags.len() > CONTEXT_DIAGNOSTICS {
        out.push_str(&format!(
            "  - (ほか{}件。全件はrun_verificationで取得すること)\n",
            diags.len() - CONTEXT_DIAGNOSTICS
        ));
    } else {
        out.push_str("- 詳細(該当エンティティid)はrun_verificationで取得すること\n");
    }
    out
}
