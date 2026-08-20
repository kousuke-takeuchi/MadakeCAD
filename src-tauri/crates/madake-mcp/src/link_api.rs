//! Link API (/api/v1): 素のJSON REST + SSEパッチストリーム。
//!
//! 用途:
//! 1. FreeCADアドオン等の外部ツール連携 (spec §7)
//! 2. ブラウザでのフロントエンド開発・E2E検証 (Tauri外からvite UIを実バックエンドに接続)
//!
//! 書き込みは全て既存のCommandエンジン(SharedDoc::execute)を通るため、
//! undo/redo・patch配信・UI/AI編集と完全に整合する。

use std::convert::Infallible;

use axum::extract::{Query, State};
use axum::response::sse::{Event, Sse};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{http::StatusCode, Json, Router};
use futures::stream::Stream;
use madake_core::{builtin_symbols, Command, Patch};
use serde::Deserialize;
use tower_http::cors::CorsLayer;
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
    Ok(Json(serde_json::json!(madake_core::netlist::extract_netlist(
        sheet,
        &builtin_symbols()
    ))))
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

async fn post_save(
    State(doc): State<SharedDoc>,
    Json(body): Json<PathBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let engine = doc.engine.lock().unwrap();
    madake_core::io::save_project(std::path::Path::new(&body.path), engine.project())
        .map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "written": body.path })))
}

async fn post_load(
    State(doc): State<SharedDoc>,
    Json(body): Json<PathBody>,
) -> Result<Json<Patch>, ApiError> {
    let project =
        madake_core::io::load_project(std::path::Path::new(&body.path)).map_err(bad_request)?;
    let patch = doc.engine.lock().unwrap().replace_project(project);
    let _ = doc.patches.send(patch.clone());
    Ok(Json(patch))
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
    std::fs::write(&body.path, madake_core::reports::wire_list_csv(engine.project()))
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

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "name": "MadakeCAD Link API", "version": 1 }))
}

/// /api/v1 のRouterを構築する。CORSはローカル開発・ツール連携用に全許可
/// (サーバーは127.0.0.1バインドのため外部公開はされない)。
pub fn router(doc: SharedDoc) -> Router {
    Router::new()
        .route("/api/v1", get(health))
        .route("/api/v1/project", get(get_project))
        .route("/api/v1/symbols", get(get_symbols))
        .route("/api/v1/netlist", get(get_netlist))
        .route("/api/v1/commands", post(post_commands))
        .route("/api/v1/undo", post(post_undo))
        .route("/api/v1/redo", post(post_redo))
        .route("/api/v1/save", post(post_save))
        .route("/api/v1/load", post(post_load))
        .route("/api/v1/export/svg", post(post_export_svg))
        .route("/api/v1/export/bom", post(post_export_bom))
        .route("/api/v1/export/wire-list", post(post_export_wire_list))
        .route("/api/v1/events", get(get_events))
        .layer(CorsLayer::permissive())
        .with_state(doc)
}
