use std::path::PathBuf;
use std::sync::Arc;

use madake_agent::{AgentManager, AppSettings, Conversation, DetectResult};
use madake_core::{builtin_symbols, sheet_symbol_defs, Command, Engine, Patch, Project, SymbolDef};
use madake_mcp::SharedDoc;
use tauri::{Emitter, State};
use uuid::Uuid;

/// MCPサーバーの待受ポート。環境変数MADAKE_MCP_PORTで上書き可能。
const DEFAULT_MCP_PORT: u16 = 9310;

struct AppState {
    doc: SharedDoc,
    agent: Arc<AgentManager>,
    parts: madake_mcp::SharedParts,
}

#[derive(serde::Serialize)]
struct ProjectSnapshot {
    revision: u64,
    project: Project,
    can_undo: bool,
    can_redo: bool,
}

#[tauri::command]
fn get_project(state: State<AppState>) -> ProjectSnapshot {
    let engine = state.doc.engine.lock().unwrap();
    ProjectSnapshot {
        revision: engine.revision(),
        project: engine.project().clone(),
        can_undo: engine.can_undo(),
        can_redo: engine.can_redo(),
    }
}

#[tauri::command]
fn list_symbols() -> Vec<SymbolDef> {
    builtin_symbols()
}

#[tauri::command]
fn execute_command(state: State<AppState>, command: Command) -> Result<Patch, String> {
    state.doc.execute(command).map_err(|e| e.to_string())
}

#[tauri::command]
fn undo(state: State<AppState>) -> Result<Option<Patch>, String> {
    state.doc.undo().map_err(|e| e.to_string())
}

#[tauri::command]
fn redo(state: State<AppState>) -> Result<Option<Patch>, String> {
    state.doc.redo().map_err(|e| e.to_string())
}

#[tauri::command]
fn save_project(state: State<AppState>, path: String) -> Result<(), String> {
    // 実処理はmadake-mcp側の共通関数(Link APIの/api/v1/saveと同一)
    madake_mcp::agent::save_project_with_chat(&state.doc, &state.agent, &PathBuf::from(path))
}

#[tauri::command]
fn load_project(state: State<AppState>, path: String) -> Result<Patch, String> {
    madake_mcp::agent::load_project_with_chat(&state.doc, &state.agent, &PathBuf::from(path))
}

#[tauri::command]
fn new_project(state: State<AppState>, name: String) -> Result<Patch, String> {
    let patch = state
        .doc
        .engine
        .lock()
        .unwrap()
        .replace_project(Project::new(&name));
    let _ = state.doc.patches.send(patch.clone());
    // 無題(未保存)プロジェクトの会話は保存先が無いため、履歴も新規から始める
    state.agent.set_conversations(Vec::new());
    Ok(patch)
}

#[tauri::command]
fn get_netlist(
    state: State<AppState>,
    sheet_id: madake_core::SheetId,
) -> Result<Vec<madake_core::netlist::Net>, String> {
    let engine = state.doc.engine.lock().unwrap();
    let sheet = engine
        .project()
        .sheet(sheet_id)
        .ok_or_else(|| format!("sheet not found: {sheet_id}"))?;
    Ok(madake_core::netlist::extract_netlist(
        sheet,
        &sheet_symbol_defs(sheet),
    ))
}

#[tauri::command]
fn export_svg(
    state: State<AppState>,
    sheet_id: madake_core::SheetId,
    path: String,
) -> Result<(), String> {
    let engine = state.doc.engine.lock().unwrap();
    let sheet = engine
        .project()
        .sheet(sheet_id)
        .ok_or_else(|| format!("sheet not found: {sheet_id}"))?;
    let svg = madake_core::svg::sheet_to_svg(sheet, &sheet_symbol_defs(sheet));
    std::fs::write(&path, svg).map_err(|e| e.to_string())
}

#[tauri::command]
fn search_parts(
    state: State<AppState>,
    query: Option<String>,
    category: Option<String>,
) -> Result<Vec<madake_core::parts::Part>, String> {
    let db = state.parts.lock().unwrap();
    db.search_parts(query.as_deref().unwrap_or(""), category.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn run_verification(
    state: State<AppState>,
    sheet_id: Option<madake_core::SheetId>,
) -> Result<Vec<madake_core::verify::Diagnostic>, String> {
    let engine = state.doc.engine.lock().unwrap();
    let project = engine.project();
    let sheets: Vec<_> = match sheet_id {
        Some(id) => vec![project.sheet(id).ok_or_else(|| format!("sheet not found: {id}"))?],
        None => project.sheets.iter().collect(),
    };
    let mut diags = Vec::new();
    for sheet in sheets {
        diags.extend(madake_core::verify::verify_sheet(
            sheet,
            &sheet_symbol_defs(sheet),
        ));
    }
    Ok(diags)
}

#[tauri::command]
fn export_pdf(
    state: State<AppState>,
    sheet_id: madake_core::SheetId,
    path: String,
) -> Result<(), String> {
    let engine = state.doc.engine.lock().unwrap();
    let sheet = engine
        .project()
        .sheet(sheet_id)
        .ok_or_else(|| format!("sheet not found: {sheet_id}"))?;
    let pdf = madake_core::pdf::sheet_to_pdf(sheet, &sheet_symbol_defs(sheet))
        .map_err(|e| e.to_string())?;
    std::fs::write(&path, pdf).map_err(|e| e.to_string())
}

#[tauri::command]
fn export_bom(state: State<AppState>, path: String) -> Result<(), String> {
    let engine = state.doc.engine.lock().unwrap();
    std::fs::write(&path, madake_core::reports::bom_csv(engine.project()))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn export_wire_list(state: State<AppState>, path: String) -> Result<(), String> {
    let engine = state.doc.engine.lock().unwrap();
    std::fs::write(&path, madake_core::reports::wire_list_csv(engine.project()))
        .map_err(|e| e.to_string())
}

/// エージェントへ1ターン送る。`conversation_id`省略で新規会話を作り、そのIDを返す。
///
/// 図面コンテキスト(アクティブシート・シート一覧・ネット数・座標系)は毎回付与する。
#[tauri::command]
async fn agent_send(
    state: State<'_, AppState>,
    conversation_id: Option<Uuid>,
    prompt: String,
    model: Option<String>,
) -> Result<Uuid, String> {
    let context = madake_mcp::agent::drawing_context(&state.doc);
    state
        .agent
        .send(conversation_id, &prompt, model, Some(context))
        .await
        .map_err(|e| e.to_string())
}

/// 実行中のターンを中断する(中断した場合のみtrue)。
#[tauri::command]
fn agent_cancel(state: State<AppState>, conversation_id: Uuid) -> bool {
    state.agent.cancel(conversation_id)
}

#[tauri::command]
fn agent_list_conversations(state: State<AppState>) -> Vec<Conversation> {
    state.agent.conversations()
}

/// 指定ターンの編集を巻き戻す。戻り値は巻き戻し後のrevision。
#[tauri::command]
fn agent_undo_turn(
    state: State<AppState>,
    conversation_id: Uuid,
    message_index: usize,
) -> Result<u64, String> {
    state
        .agent
        .undo_turn(conversation_id, message_index)
        .map_err(|e| e.to_string())
}

/// claude CLIを検出する(パス・バージョン)。
#[tauri::command]
async fn agent_detect(state: State<'_, AppState>) -> Result<DetectResult, String> {
    state.agent.detect().await.map_err(|e| e.to_string())
}

/// アプリ設定(`~/.madakecad/settings.json`)を取得する。
#[tauri::command]
fn get_settings(state: State<AppState>) -> AppSettings {
    state.agent.settings()
}

/// アプリ設定を保存し、エージェントへ反映する(次の送信から有効)。
///
/// 戻り値は正規化後の設定(空パスは未指定に畳まれる)。
#[tauri::command]
fn set_settings(state: State<AppState>, settings: AppSettings) -> Result<AppSettings, String> {
    madake_mcp::agent::update_settings(&state.agent, settings)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let doc = SharedDoc::new(Engine::new(Project::new("無題プロジェクト")));
    let parts = madake_mcp::open_parts(&madake_core::parts::default_db_path())
        .unwrap_or_else(|e| {
            eprintln!("部品DBを開けませんでした ({e})。一時DBで継続します");
            let tmp = std::env::temp_dir().join("madake-parts-fallback.sqlite");
            madake_mcp::open_parts(&tmp).expect("一時部品DB")
        });
    // MCPサーバーの待受ポート。エージェントのCLIもこのポートの/mcpへ自己接続する
    let mcp_port = std::env::var("MADAKE_MCP_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_MCP_PORT);
    let agent = madake_mcp::agent::manager(&doc, mcp_port);
    // 保存済みのアプリ設定(claude実行パス・図面の自動読み取り)を反映してから起動する
    madake_mcp::agent::load_and_apply_settings(&agent);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            parts: parts.clone(),
            doc: doc.clone(),
            agent: Arc::clone(&agent),
        })
        .setup(move |app| {
            // MCPサーバー起動 (127.0.0.1:port/mcp)。Link API(/api/v1)も同じポート
            let mcp_doc = doc.clone();
            let mcp_agent = Arc::clone(&agent);
            let mcp_parts = parts.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = madake_mcp::serve(mcp_doc, mcp_agent, mcp_parts, mcp_port).await {
                    eprintln!("MCP server error: {e}");
                }
            });

            // エージェントイベントをwebviewへ転送 (payload: {conversation_id, event})
            let agent_app = app.handle().clone();
            let mut agent_rx = agent.subscribe();
            tauri::async_runtime::spawn(async move {
                loop {
                    match agent_rx.recv().await {
                        Ok(event) => {
                            let _ = agent_app.emit("agent:event", &event);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });

            // patchブロードキャストをwebviewイベントへ転送
            let app_handle = app.handle().clone();
            let mut rx = doc.patches.subscribe();
            tauri::async_runtime::spawn(async move {
                loop {
                    match rx.recv().await {
                        Ok(patch) => {
                            let _ = app_handle.emit("doc:patch", &patch);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_project,
            list_symbols,
            execute_command,
            undo,
            redo,
            save_project,
            load_project,
            new_project,
            get_netlist,
            run_verification,
            search_parts,
            export_svg,
            export_pdf,
            export_bom,
            export_wire_list,
            agent_send,
            agent_cancel,
            agent_list_conversations,
            agent_undo_turn,
            agent_detect,
            get_settings,
            set_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
