use std::path::PathBuf;

use madake_core::{builtin_symbols, Command, Engine, Patch, Project, SymbolDef};
use madake_mcp::SharedDoc;
use tauri::{Emitter, State};

/// MCPサーバーの待受ポート。環境変数MADAKE_MCP_PORTで上書き可能。
const DEFAULT_MCP_PORT: u16 = 9310;

struct AppState {
    doc: SharedDoc,
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
    let engine = state.doc.engine.lock().unwrap();
    madake_core::io::save_project(&PathBuf::from(path), engine.project())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn load_project(state: State<AppState>, path: String) -> Result<Patch, String> {
    let project =
        madake_core::io::load_project(&PathBuf::from(path)).map_err(|e| e.to_string())?;
    let patch = state.doc.engine.lock().unwrap().replace_project(project);
    let _ = state.doc.patches.send(patch.clone());
    Ok(patch)
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
        &builtin_symbols(),
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
    let svg = madake_core::svg::sheet_to_svg(sheet, &builtin_symbols());
    std::fs::write(&path, svg).map_err(|e| e.to_string())
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let doc = SharedDoc::new(Engine::new(Project::new("無題プロジェクト")));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { doc: doc.clone() })
        .setup(move |app| {
            // MCPサーバー起動 (127.0.0.1:port/mcp)
            let mcp_port = std::env::var("MADAKE_MCP_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_MCP_PORT);
            let mcp_doc = doc.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = madake_mcp::serve(mcp_doc, mcp_port).await {
                    eprintln!("MCP server error: {e}");
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
            export_svg,
            export_bom,
            export_wire_list
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
