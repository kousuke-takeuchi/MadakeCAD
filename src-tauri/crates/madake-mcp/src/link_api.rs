//! Link API (/api/v1): 素のJSON REST + SSEパッチストリーム。
//!
//! 用途:
//! 1. FreeCADアドオン等の外部ツール連携 (spec §7)
//! 2. ブラウザでのフロントエンド開発・E2E検証 (Tauri外からvite UIを実バックエンドに接続)
//!
//! 書き込みは全て既存のCommandエンジン(SharedDoc::execute)を通るため、
//! undo/redo・patch配信・UI/AI編集と完全に整合する。

use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::sse::{Event, Sse};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{http::StatusCode, Json, Router};
use futures::stream::Stream;
use madake_agent::{AgentManager, AppSettings, Conversation, DetectResult};
use madake_core::{builtin_symbols, sheet_symbol_defs, Command, Patch};
use serde::Deserialize;
use tower_http::cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer};
use uuid::Uuid;

use crate::{SharedDoc, SharedParts};

type ApiError = (StatusCode, String);

fn bad_request(msg: impl std::fmt::Display) -> ApiError {
    (StatusCode::BAD_REQUEST, msg.to_string())
}

async fn get_project(State(doc): State<SharedDoc>) -> Json<serde_json::Value> {
    let engine = doc.engine.lock().unwrap();
    Json(serde_json::json!({
        "revision": engine.revision(),
        "project": engine.project(),
        "can_undo": engine.can_undo(),
        "can_redo": engine.can_redo(),
    }))
}

async fn get_symbols() -> Json<serde_json::Value> {
    Json(serde_json::json!(builtin_symbols()))
}

#[derive(Deserialize)]
struct NetlistQuery {
    sheet_id: Option<Uuid>,
}

async fn get_netlist(
    State(doc): State<SharedDoc>,
    Query(q): Query<NetlistQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let engine = doc.engine.lock().unwrap();
    let sheet = match q.sheet_id {
        Some(id) => engine.project().sheet(id),
        None => engine.project().sheets.first(),
    }
    .ok_or_else(|| bad_request("sheet not found"))?;
    Ok(Json(serde_json::json!(
        madake_core::netlist::extract_netlist(sheet, &sheet_symbol_defs(sheet))
    )))
}

async fn post_commands(
    State(doc): State<SharedDoc>,
    Json(commands): Json<Vec<Command>>,
) -> Result<Json<Vec<Patch>>, ApiError> {
    let mut patches = Vec::new();
    for cmd in commands {
        patches.push(doc.execute(cmd).map_err(bad_request)?);
    }
    Ok(Json(patches))
}

async fn post_undo(State(doc): State<SharedDoc>) -> Result<Json<Option<Patch>>, ApiError> {
    doc.undo().map(Json).map_err(bad_request)
}

async fn post_redo(State(doc): State<SharedDoc>) -> Result<Json<Option<Patch>>, ApiError> {
    doc.redo().map(Json).map_err(bad_request)
}

#[derive(Deserialize)]
struct PathBody {
    path: String,
}

/// 図面とチャット履歴をまとめて保存する(Tauriの`save_project`と同じ処理)。
async fn post_save(
    State(state): State<AgentApi>,
    Json(body): Json<PathBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    crate::agent::save_project_with_chat(
        &state.doc,
        &state.agent,
        std::path::Path::new(&body.path),
    )
    .map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "written": body.path })))
}

/// 図面とチャット履歴をまとめて読み込む(Tauriの`load_project`と同じ処理)。
///
/// 実行中のターンは中断される。
async fn post_load(
    State(state): State<AgentApi>,
    Json(body): Json<PathBody>,
) -> Result<Json<Patch>, ApiError> {
    crate::agent::load_project_with_chat(&state.doc, &state.agent, std::path::Path::new(&body.path))
        .map(Json)
        .map_err(bad_request)
}

/// KiCad回路図を読み込みプロジェクトを置き換える。
async fn post_import_kicad(
    State(state): State<AgentApi>,
    Json(body): Json<PathBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let (patch, report) = crate::agent::import_kicad_with_chat(
        &state.doc,
        &state.agent,
        std::path::Path::new(&body.path),
    )
    .map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "patch": patch, "report": report })))
}

#[derive(Deserialize)]
struct ExportSvgBody {
    sheet_id: Option<Uuid>,
    path: String,
}

async fn post_export_svg(
    State(doc): State<SharedDoc>,
    Json(body): Json<ExportSvgBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let engine = doc.engine.lock().unwrap();
    let project = engine.project();
    let sheet = match body.sheet_id {
        Some(id) => project.sheet(id),
        None => project.sheets.first(),
    }
    .ok_or_else(|| bad_request("sheet not found"))?;
    // プロジェクト文脈で描くとネットラベルにシート間クロスリファレンスが入る
    let svg = madake_core::svg::project_sheet_to_svg(project, sheet.id, &sheet_symbol_defs(sheet))
        .ok_or_else(|| bad_request("sheet not found"))?;
    std::fs::write(&body.path, svg).map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "written": body.path })))
}

#[derive(Deserialize)]
struct SimulateBody {
    sheet_id: Option<Uuid>,
    #[serde(default)]
    open_switches: Vec<String>,
}

async fn post_simulate_op(
    State(doc): State<SharedDoc>,
    Json(body): Json<SimulateBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let engine = doc.engine.lock().unwrap();
    let sheet = match body.sheet_id {
        Some(id) => engine.project().sheet(id),
        None => engine.project().sheets.first(),
    }
    .ok_or_else(|| bad_request("sheet not found"))?;
    let result = madake_core::sim::simulate_op(sheet, &sheet_symbol_defs(sheet), &body.open_switches)
        .map_err(bad_request)?;
    Ok(Json(serde_json::json!(result)))
}

async fn get_verify(
    State(doc): State<SharedDoc>,
    Query(q): Query<NetlistQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let engine = doc.engine.lock().unwrap();
    let project = engine.project();
    // シート指定なし=プロジェクト全体。ネットラベル関連はシートを跨いだ統合ネットで評価する
    let diags = match q.sheet_id {
        Some(id) => {
            let sheet = project.sheet(id).ok_or_else(|| bad_request("sheet not found"))?;
            madake_core::verify::verify_sheet(sheet, &sheet_symbol_defs(sheet))
        }
        None => madake_core::verify::verify_project(project),
    };
    Ok(Json(serde_json::json!(diags)))
}

async fn post_export_pdf(
    State(doc): State<SharedDoc>,
    Json(body): Json<ExportSvgBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let engine = doc.engine.lock().unwrap();
    let project = engine.project();
    let sheet = match body.sheet_id {
        Some(id) => project.sheet(id),
        None => project.sheets.first(),
    }
    .ok_or_else(|| bad_request("sheet not found"))?;
    let pdf = madake_core::pdf::project_sheet_to_pdf(project, sheet.id, &sheet_symbol_defs(sheet))
        .ok_or_else(|| bad_request("sheet not found"))?
        .map_err(bad_request)?;
    std::fs::write(&body.path, pdf).map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "written": body.path })))
}

#[derive(Deserialize)]
struct ExportPdfBookBody {
    path: String,
    /// 回路図の後ろに付ける帳票 (`wire-list` / `terminal-chart` / `terminal-diagram` / `bom` / `xref`)。
    #[serde(default)]
    include_reports: Vec<madake_core::report_sheet::ReportKind>,
    /// 表紙を付けるか (既定: 付ける)。
    #[serde(default = "default_true")]
    cover: bool,
}

fn default_true() -> bool {
    true
}

/// 表紙+回路図全シート+選択帳票を1つのPDFにまとめて書き出す。
async fn post_export_pdf_book(
    State(doc): State<SharedDoc>,
    Json(body): Json<ExportPdfBookBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let engine = doc.engine.lock().unwrap();
    let options = madake_core::pdf::PdfBookOptions {
        include_reports: body.include_reports,
        cover: body.cover,
    };
    let pages = madake_core::pdf::project_pdf_pages(engine.project(), &options).len();
    let pdf = madake_core::pdf::export_project_pdf(engine.project(), &options)
        .map_err(bad_request)?;
    std::fs::write(&body.path, pdf).map_err(bad_request)?;
    Ok(Json(
        serde_json::json!({ "written": body.path, "pages": pages }),
    ))
}

async fn post_export_bom(
    State(doc): State<SharedDoc>,
    Json(body): Json<PathBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let engine = doc.engine.lock().unwrap();
    std::fs::write(&body.path, madake_core::reports::bom_csv(engine.project()))
        .map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "written": body.path })))
}

async fn post_export_wire_list(
    State(doc): State<SharedDoc>,
    Json(body): Json<PathBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let engine = doc.engine.lock().unwrap();
    std::fs::write(
        &body.path,
        madake_core::reports::wire_list_csv(engine.project()),
    )
    .map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "written": body.path })))
}

/// パッチのSSEストリーム。接続時点以降の全patchをJSONで流す。
async fn get_events(
    State(doc): State<SharedDoc>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = doc.patches.subscribe();
    let stream = futures::stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(patch) => {
                    let event = Event::default()
                        .event("patch")
                        .data(serde_json::to_string(&patch).unwrap_or_default());
                    return Some((Ok(event), rx));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
            }
        }
    });
    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}

/// エージェントAPIの状態(ドキュメント + マネージャ)。
#[derive(Clone)]
struct AgentApi {
    doc: SharedDoc,
    agent: Arc<AgentManager>,
}

#[derive(Deserialize)]
struct AgentSendBody {
    /// 継続する会話。省略時は新規会話を作る。
    conversation_id: Option<Uuid>,
    prompt: String,
    model: Option<String>,
}

async fn post_agent_send(
    State(state): State<AgentApi>,
    Json(body): Json<AgentSendBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let context = crate::agent::drawing_context(&state.doc);
    let id = state
        .agent
        .send(
            body.conversation_id,
            &body.prompt,
            body.model,
            Some(context),
        )
        .await
        .map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "conversation_id": id })))
}

#[derive(Deserialize)]
struct ConversationRefBody {
    conversation_id: Uuid,
}

async fn post_agent_cancel(
    State(state): State<AgentApi>,
    Json(body): Json<ConversationRefBody>,
) -> Json<serde_json::Value> {
    let cancelled = state.agent.cancel(body.conversation_id);
    Json(serde_json::json!({ "cancelled": cancelled }))
}

async fn get_agent_conversations(State(state): State<AgentApi>) -> Json<Vec<Conversation>> {
    Json(state.agent.conversations())
}

#[derive(Deserialize)]
struct UndoTurnBody {
    conversation_id: Uuid,
    /// 対象のアシスタントメッセージの位置(`conversations`の`messages`添字)。
    message_index: usize,
}

async fn post_agent_undo_turn(
    State(state): State<AgentApi>,
    Json(body): Json<UndoTurnBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let revision = state
        .agent
        .undo_turn(body.conversation_id, body.message_index)
        .map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "revision": revision })))
}

async fn get_agent_detect(State(state): State<AgentApi>) -> Result<Json<DetectResult>, ApiError> {
    state.agent.detect().await.map(Json).map_err(bad_request)
}

/// 現在のアプリ設定(`~/.madakecad/settings.json`の内容)。
async fn get_settings(State(state): State<AgentApi>) -> Json<AppSettings> {
    Json(state.agent.settings())
}

/// アプリ設定を保存し、エージェントへ反映する。戻り値は正規化後の設定。
async fn put_settings(
    State(state): State<AgentApi>,
    Json(body): Json<AppSettings>,
) -> Result<Json<AppSettings>, ApiError> {
    crate::agent::update_settings(&state.agent, body)
        .map(Json)
        .map_err(bad_request)
}

/// エージェントイベントのSSEストリーム。
///
/// イベント名は`agent`、データはTauriの`agent:event`と同一のJSON
/// (`{"conversation_id": "...", "event": {"type": ...}}`)。
async fn get_agent_events(
    State(state): State<AgentApi>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.agent.subscribe();
    let stream = futures::stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let event = Event::default()
                        .event("agent")
                        .data(serde_json::to_string(&event).unwrap_or_default());
                    return Some((Ok(event), rx));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
            }
        }
    });
    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}

/// エージェントマネージャを必要とするRouter(ブラウザ検証用。Tauri IPCと同じ
/// マネージャを共有)。`/save`・`/load`もチャット履歴を伴うためここに置く。
/// `/settings`はマネージャへ反映するため同様。
fn agent_router(state: AgentApi) -> Router {
    Router::new()
        .route("/api/v1/save", post(post_save))
        .route("/api/v1/load", post(post_load))
        .route("/api/v1/import/kicad", post(post_import_kicad))
        .route("/api/v1/agent/send", post(post_agent_send))
        .route("/api/v1/agent/cancel", post(post_agent_cancel))
        .route("/api/v1/agent/conversations", get(get_agent_conversations))
        .route("/api/v1/agent/undo-turn", post(post_agent_undo_turn))
        .route("/api/v1/agent/detect", get(get_agent_detect))
        .route("/api/v1/agent/events", get(get_agent_events))
        .route("/api/v1/settings", get(get_settings).put(put_settings))
        .with_state(state)
}

/// ローカル由来のOriginか。
///
/// 判定対象は`localhost` / `127.0.0.1` / `::1` / `*.localhost`(Tauri WindowsのWebView2は
/// `http://tauri.localhost`、macOS/Linuxは`tauri://localhost`を送る)。
/// ポート番号は任意(viteは1420、他ツールは任意ポートを使う)。
pub fn is_local_origin(origin: &str) -> bool {
    let Some(rest) = ["http://", "https://", "tauri://"]
        .iter()
        .find_map(|scheme| origin.strip_prefix(scheme))
    else {
        return false;
    };
    if rest.is_empty() || rest.contains('/') || rest.contains('@') {
        return false;
    }
    // `host:port` / `[::1]:port` からホスト部を取り出す
    let host = match rest.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => host,
        _ => rest,
    };
    let host = host.trim_start_matches('[').trim_end_matches(']');
    matches!(host, "localhost" | "127.0.0.1" | "::1") || host.ends_with(".localhost")
}

/// 外部WebページからのDNSリバインディング/CSRF的な呼び出しを弾む。
///
/// このサーバーは127.0.0.1バインドだが、任意のWebページのJSからは
/// `http://127.0.0.1:9310/api/v1/agent/send`を叩けてしまう(それだけでAIに図面を
/// 編集させられる)。Originが付かないリクエスト(curl・CLI・Tauri webview)は許可し、
/// ローカル以外のOriginが付いていれば403で拒否する。
async fn guard_origin(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    match request.headers().get(axum::http::header::ORIGIN) {
        None => next.run(request).await,
        Some(origin) if origin.to_str().map(is_local_origin).unwrap_or(false) => {
            next.run(request).await
        }
        Some(_) => (
            StatusCode::FORBIDDEN,
            "MadakeCAD Link APIはローカルからのみ利用できます",
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct PartsQuery {
    query: Option<String>,
    category: Option<String>,
}

async fn get_parts(
    State(parts): State<SharedParts>,
    Query(q): Query<PartsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = parts.lock().unwrap();
    let hits = db
        .search_parts(q.query.as_deref().unwrap_or(""), q.category.as_deref())
        .map_err(bad_request)?;
    Ok(Json(serde_json::json!(hits)))
}

async fn post_part(
    State(parts): State<SharedParts>,
    Json(part): Json<madake_core::parts::Part>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = parts.lock().unwrap();
    db.upsert_part(&part).map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "ok": true, "part_no": part.part_no })))
}

async fn delete_part(
    State(parts): State<SharedParts>,
    axum::extract::Path(part_no): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = parts.lock().unwrap();
    let deleted = db.delete_part(&part_no).map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "deleted": deleted })))
}

async fn get_wire_parts(
    State(parts): State<SharedParts>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = parts.lock().unwrap();
    Ok(Json(serde_json::json!(db.list_wire_parts().map_err(bad_request)?)))
}

async fn post_wire_part(
    State(parts): State<SharedParts>,
    Json(row): Json<madake_core::parts::WirePartRow>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = parts.lock().unwrap();
    db.upsert_wire_part(&row).map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "name": "MadakeCAD Link API", "version": 1 }))
}

/// /api/v1 のRouterを構築する。
///
/// アクセスはローカルオリジンに限定する(サーバーは127.0.0.1バインドだが、
/// 外部Webページのブラウザからは到達できてしまうため)。ブラウザ以外
/// (curl・madake CLI・FreeCADアドオン)はOriginを付けないので影響を受けない。
pub fn router(doc: SharedDoc, agent: Arc<AgentManager>, parts: SharedParts) -> Router {
    let agent_routes = agent_router(AgentApi {
        doc: doc.clone(),
        agent,
    });
    let parts_routes = Router::new()
        .route("/api/v1/parts", get(get_parts).post(post_part))
        .route("/api/v1/parts/{part_no}", axum::routing::delete(delete_part))
        .route("/api/v1/wire-parts", get(get_wire_parts).post(post_wire_part))
        .with_state(parts);
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin, _| {
            origin.to_str().map(is_local_origin).unwrap_or(false)
        }))
        .allow_methods(AllowMethods::mirror_request())
        .allow_headers(AllowHeaders::mirror_request());
    Router::new()
        .route("/api/v1", get(health))
        .route("/api/v1/project", get(get_project))
        .route("/api/v1/symbols", get(get_symbols))
        .route("/api/v1/netlist", get(get_netlist))
        .route("/api/v1/verify", get(get_verify))
        .route("/api/v1/simulate/op", post(post_simulate_op))
        .route("/api/v1/commands", post(post_commands))
        .route("/api/v1/undo", post(post_undo))
        .route("/api/v1/redo", post(post_redo))
        .route("/api/v1/export/svg", post(post_export_svg))
        .route("/api/v1/export/pdf", post(post_export_pdf))
        .route("/api/v1/export/pdf-book", post(post_export_pdf_book))
        .route("/api/v1/export/bom", post(post_export_bom))
        .route("/api/v1/export/wire-list", post(post_export_wire_list))
        .route("/api/v1/events", get(get_events))
        .with_state(doc)
        .merge(parts_routes)
        .merge(agent_routes)
        .layer(cors)
        // CORSより外側。プリフライトもここを通す(外部オリジンはここで403)
        .layer(axum::middleware::from_fn(guard_origin))
}
