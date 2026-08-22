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

/// UI操作の編集。由来は`user`として履歴に残り、AIのターン巻き戻しに巻き込まれない。
#[tauri::command]
fn execute_command(state: State<AppState>, command: Command) -> Result<Patch, String> {
    state.doc.execute_user(command).map_err(|e| e.to_string())
}

/// 使える開始テンプレートの一覧(同梱+ユーザーの`~/MadakeCAD/templates`)。
#[tauri::command]
fn list_templates() -> madake_core::templates::TemplateList {
    madake_core::templates::list()
}

/// ユーザーテンプレートの置き場(`~/MadakeCAD/templates`)をOSのファイラで開く。
/// 無ければ作ってから開く(「ここへJSONを置けば一覧に並ぶ」を実物で示す)。
#[tauri::command]
fn open_templates_folder(app: tauri::AppHandle) -> Result<String, String> {
    use tauri_plugin_opener::OpenerExt;

    let dir = madake_core::templates::user_dir().ok_or("home directory not found")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.display().to_string();
    app.opener()
        .open_path(path.clone(), None::<&str>)
        .map_err(|e| e.to_string())?;
    Ok(path)
}

/// 開始テンプレートをシートへ適用する。UI操作なので由来は`user`、
/// **1回の編集**として履歴に乗るのでundo一発で全体が戻る。
#[tauri::command]
fn apply_template(
    state: State<AppState>,
    template_id: String,
    sheet_id: madake_core::SheetId,
) -> Result<Patch, String> {
    state
        .doc
        .apply_template(&template_id, sheet_id, madake_core::EditOrigin::User)
        .map_err(|e| e.to_string())
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
    let project = engine.project();
    let sheet = project
        .sheet(sheet_id)
        .ok_or_else(|| format!("sheet not found: {sheet_id}"))?;
    // プロジェクト文脈で描くとネットラベルにシート間クロスリファレンスが入る
    let svg = madake_core::svg::project_sheet_to_svg(project, sheet_id, &sheet_symbol_defs(sheet))
        .ok_or_else(|| format!("sheet not found: {sheet_id}"))?;
    std::fs::write(&path, svg).map_err(|e| e.to_string())
}

#[tauri::command]
fn import_kicad(state: State<AppState>, path: String) -> Result<serde_json::Value, String> {
    let (patch, report) = madake_mcp::agent::import_kicad_with_chat(
        &state.doc,
        &state.agent,
        std::path::Path::new(&path),
    )?;
    Ok(serde_json::json!({ "patch": patch, "report": report }))
}

#[tauri::command]
fn simulate_op(
    state: State<AppState>,
    sheet_id: Option<madake_core::SheetId>,
    open_switches: Vec<String>,
) -> Result<madake_core::sim::SimOpResult, String> {
    let engine = state.doc.engine.lock().unwrap();
    let project = engine.project();
    let sheet = match sheet_id {
        Some(id) => project.sheet(id),
        None => project.sheets.first(),
    }
    .ok_or("sheet not found")?;
    madake_core::sim::simulate_op(sheet, &sheet_symbol_defs(sheet), &open_switches)
        .map_err(|e| e.to_string())
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
    // シート指定なし=プロジェクト全体。ネットラベル関連はシートを跨いだ統合ネットで評価する
    let Some(id) = sheet_id else {
        return Ok(madake_core::verify::verify_project(project));
    };
    let sheet = project
        .sheet(id)
        .ok_or_else(|| format!("sheet not found: {id}"))?;
    Ok(madake_core::verify::verify_sheet(
        sheet,
        &sheet_symbol_defs(sheet),
    ))
}

#[tauri::command]
fn export_pdf(
    state: State<AppState>,
    sheet_id: madake_core::SheetId,
    path: String,
) -> Result<(), String> {
    let engine = state.doc.engine.lock().unwrap();
    let project = engine.project();
    let sheet = project
        .sheet(sheet_id)
        .ok_or_else(|| format!("sheet not found: {sheet_id}"))?;
    let pdf = madake_core::pdf::project_sheet_to_pdf(project, sheet_id, &sheet_symbol_defs(sheet))
        .ok_or_else(|| format!("sheet not found: {sheet_id}"))?
        .map_err(|e| e.to_string())?;
    std::fs::write(&path, pdf).map_err(|e| e.to_string())
}

/// 図面一式を1つのPDFへ (表紙+回路図全シート+選択帳票)。書き出したページ数を返す。
#[tauri::command]
fn export_pdf_book(
    state: State<AppState>,
    path: String,
    include_reports: Vec<madake_core::report_sheet::ReportKind>,
    cover: Option<bool>,
) -> Result<usize, String> {
    let engine = state.doc.engine.lock().unwrap();
    let options = madake_core::pdf::PdfBookOptions {
        include_reports,
        cover: cover.unwrap_or(true),
    };
    let pages = madake_core::pdf::project_pdf_pages(engine.project(), &options).len();
    let pdf = madake_core::pdf::export_project_pdf(engine.project(), &options)
        .map_err(|e| e.to_string())?;
    std::fs::write(&path, pdf).map_err(|e| e.to_string())?;
    Ok(pages)
}

/// 端子台エディタ・帳票の対象選択に出す端子台の一覧 (sheet_id省略でプロジェクト全体)。
#[tauri::command]
fn list_terminal_blocks(
    state: State<AppState>,
    sheet_id: Option<madake_core::SheetId>,
) -> Vec<madake_core::terminal_chart::TerminalBlockInfo> {
    let engine = state.doc.engine.lock().unwrap();
    madake_core::terminal_chart::terminal_block_infos(engine.project(), sheet_id)
}

/// 端子台1つのチャート (端子ごとの行・ジャンパ)。端子台でなければNone。
#[tauri::command]
fn get_terminal_chart(
    state: State<AppState>,
    entity_id: madake_core::EntityId,
) -> Option<madake_core::terminal_chart::TerminalChart> {
    let engine = state.doc.engine.lock().unwrap();
    madake_core::terminal_chart::terminal_chart_in_project(engine.project(), entity_id)
}

/// 端子台チェック (未結線の端子・不正なジャンパ)。
#[tauri::command]
fn check_terminal_block(
    state: State<AppState>,
    entity_id: madake_core::EntityId,
) -> Vec<madake_core::verify::Diagnostic> {
    let engine = state.doc.engine.lock().unwrap();
    madake_core::terminal_chart::check_terminal_block_in_project(engine.project(), entity_id)
}

/// 帳票1種を1ファイルへ書き出す。戻り値はCSVなら行数、PDFならページ数。
#[tauri::command]
fn export_report(
    state: State<AppState>,
    kind: madake_core::report_sheet::ReportKind,
    format: madake_core::report_sheet::ReportFormat,
    entity_id: Option<madake_core::EntityId>,
    path: String,
) -> Result<usize, String> {
    let engine = state.doc.engine.lock().unwrap();
    let (bytes, count) =
        madake_core::report_sheet::report_bytes(engine.project(), kind, format, entity_id)
            .map_err(|e| e.to_string())?;
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok(count)
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

/// 指定ターン(ターン安定ID)の編集を巻き戻す。戻り値は巻き戻し後のrevision。
#[tauri::command]
fn agent_undo_turn(
    state: State<AppState>,
    conversation_id: Uuid,
    turn_id: Uuid,
) -> Result<u64, String> {
    state
        .agent
        .undo_turn(conversation_id, turn_id)
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

/// 同梱の規格ノート(`resources/knowledge/standards.md`)の実体パスを
/// エージェントへ渡す(`MADAKE_STANDARDS_PATH`)。
///
/// 配布時はアプリバンドル内のリソース、開発時(`tauri dev`)はリポジトリの
/// `src-tauri/resources/`を見る。どちらも見つからなければ何もしない
/// (madake-agentがビルド時に埋め込んだ同内容へフォールバックする)。
fn resolve_standards_resource(app: &tauri::AppHandle) {
    use tauri::path::BaseDirectory;
    use tauri::Manager;

    const RELATIVE: &str = "resources/knowledge/standards.md";
    let candidates = [
        app.path().resolve(RELATIVE, BaseDirectory::Resource).ok(),
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(RELATIVE)),
    ];
    for path in candidates.into_iter().flatten() {
        if path.is_file() {
            std::env::set_var(madake_agent::knowledge::STANDARDS_PATH_ENV, &path);
            return;
        }
    }
    eprintln!("規格ノート({RELATIVE})が見つかりません。埋め込みの内容で続行します");
}

/// 同梱の開始テンプレート(`resources/templates/*.json`)のディレクトリを
/// madake-coreへ渡す(`MADAKE_TEMPLATES_PATH`)。
///
/// 配布時はアプリバンドル内のリソース、開発時(`tauri dev`)はリポジトリの
/// `src-tauri/resources/`。どちらも見つからなければ何もしない
/// (madake-coreがビルド時に埋め込んだ同内容へフォールバックする)。
fn resolve_templates_resource(app: &tauri::AppHandle) {
    use tauri::path::BaseDirectory;
    use tauri::Manager;

    const RELATIVE: &str = "resources/templates";
    let candidates = [
        app.path().resolve(RELATIVE, BaseDirectory::Resource).ok(),
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(RELATIVE)),
    ];
    for path in candidates.into_iter().flatten() {
        if path.is_dir() {
            std::env::set_var(madake_core::templates::TEMPLATES_PATH_ENV, &path);
            return;
        }
    }
    eprintln!("開始テンプレート({RELATIVE})が見つかりません。埋め込みの内容で続行します");
}

/// 同梱ドキュメント(`docs/`の公開マニュアル01〜13)の場所をエージェントへ渡す
/// (`MADAKE_DOCS_PATH`)。
///
/// 配布時はアプリバンドル内のリソース(`../docs/*.md`は`_up_/docs`へ入る)、
/// 開発時(`tauri dev`)はリポジトリの`docs/`。見つからなければ何もしない
/// (madake-agentがドキュメント案内をプロンプトから省き、読めないファイルを出典にさせない)。
fn resolve_docs_resource(app: &tauri::AppHandle) {
    use tauri::path::BaseDirectory;
    use tauri::Manager;

    let candidates = [
        app.path().resolve("_up_/docs", BaseDirectory::Resource).ok(),
        app.path().resolve("docs", BaseDirectory::Resource).ok(),
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../docs")),
    ];
    for path in candidates.into_iter().flatten() {
        if path.join("01-overview.md").is_file() {
            std::env::set_var(madake_agent::knowledge::DOCS_PATH_ENV, &path);
            return;
        }
    }
    eprintln!("ドキュメント(docs/)が見つかりません。ドキュメント案内なしで続行します");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let doc = SharedDoc::new(Engine::new(Project::new("無題プロジェクト")));
    let parts =
        madake_mcp::open_parts(&madake_core::parts::default_db_path()).unwrap_or_else(|e| {
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
            // 同梱の規格ノート(編集可能なMarkdown)の実体をエージェントへ教える。
            // 見つからなくてもビルドへ埋め込んだ同内容で動くので、失敗は警告だけ
            resolve_standards_resource(app.handle());
            // 同梱ドキュメント(操作方法・規格の出典)の場所も教える
            resolve_docs_resource(app.handle());
            // 同梱の開始テンプレート(編集可能なCommand列JSON)の場所も教える
            resolve_templates_resource(app.handle());

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
            list_templates,
            apply_template,
            open_templates_folder,
            undo,
            redo,
            save_project,
            load_project,
            new_project,
            get_netlist,
            run_verification,
            search_parts,
            simulate_op,
            import_kicad,
            export_svg,
            export_pdf,
            export_pdf_book,
            export_report,
            list_terminal_blocks,
            get_terminal_chart,
            check_terminal_block,
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
