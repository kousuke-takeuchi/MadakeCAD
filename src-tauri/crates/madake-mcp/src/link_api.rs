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
use madake_agent::{AgentManager, Conversation, DetectResult};
use madake_core::{builtin_symbols, Command, Patch};
use serde::Deserialize;
use tower_http::cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer};
use uuid::Uuid;

use crate::SharedDoc;

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
        madake_core::netlist::extract_netlist(sheet, &builtin_symbols())
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
    let sheet = match body.sheet_id {
        Some(id) => engine.project().sheet(id),
        None => engine.project().sheets.first(),
    }
    .ok_or_else(|| bad_request("sheet not found"))?;
    let svg = madake_core::svg::sheet_to_svg(sheet, &builtin_symbols());
    std::fs::write(&body.path, svg).map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "written": body.path })))
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
fn agent_router(state: AgentApi) -> Router {
    Router::new()
        .route("/api/v1/save", post(post_save))
        .route("/api/v1/load", post(post_load))
        .route("/api/v1/agent/send", post(post_agent_send))
        .route("/api/v1/agent/cancel", post(post_agent_cancel))
        .route("/api/v1/agent/conversations", get(get_agent_conversations))
        .route("/api/v1/agent/undo-turn", post(post_agent_undo_turn))
        .route("/api/v1/agent/detect", get(get_agent_detect))
        .route("/api/v1/agent/events", get(get_agent_events))
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

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "name": "MadakeCAD Link API", "version": 1 }))
}

/// /api/v1 のRouterを構築する。
///
/// アクセスはローカルオリジンに限定する(サーバーは127.0.0.1バインドだが、
/// 外部Webページのブラウザからは到達できてしまうため)。ブラウザ以外
/// (curl・madake CLI・FreeCADアドオン)はOriginを付けないので影響を受けない。
pub fn router(doc: SharedDoc, agent: Arc<AgentManager>) -> Router {
    let agent_routes = agent_router(AgentApi {
        doc: doc.clone(),
        agent,
    });
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
        .route("/api/v1/commands", post(post_commands))
        .route("/api/v1/undo", post(post_undo))
        .route("/api/v1/redo", post(post_redo))
        .route("/api/v1/export/svg", post(post_export_svg))
        .route("/api/v1/export/bom", post(post_export_bom))
        .route("/api/v1/export/wire-list", post(post_export_wire_list))
        .route("/api/v1/events", get(get_events))
        .with_state(doc)
        .merge(agent_routes)
        .layer(cors)
        // CORSより外側。プリフライトもここを通す(外部オリジンはここで403)
        .layer(axum::middleware::from_fn(guard_origin))
}
