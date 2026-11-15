//! 同梱ストロークフォントの字形を返すLink APIのテスト。

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use madake_core::{Engine, Project};
use madake_mcp::SharedDoc;
use serde_json::{json, Value};
use tower::ServiceExt;

fn setup() -> (SharedDoc, Router) {
    let doc = SharedDoc::new(Engine::new(Project::new("テストプロジェクト")));
    let agent = madake_mcp::agent::manager(&doc, 9310);
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let parts = madake_mcp::open_parts(&std::env::temp_dir().join(format!(
        "madake-parts-frames-{}-{}.sqlite",
        std::process::id(),
        seq
    )))
    .expect("parts db");
    let router = madake_mcp::link_api::router(doc.clone(), Arc::clone(&agent), parts);
    (doc, router)
}

async fn post(router: &Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn get(router: &Router, uri: &str) -> (StatusCode, Value) {
    let request = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// The Link API serves the bundled stroke font's glyphs by id, and answers 404 for a font that is not bundled.
/// Link APIは同梱ストロークフォントの字形をidで返し、同梱していないフォントには404を返す。
#[tokio::test]
async fn stroke_fonts_are_served_by_id() {
    let (_doc, router) = setup();
    let (status, body) = get(&router, "/api/v1/stroke-fonts/kst32b").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], "kst32b");
    assert!(body["glyphs"]["A"]["strokes"].is_array());
    assert_eq!(body["glyphs"]["亜"]["wide"], true);
    let (status, _) = get(&router, "/api/v1/stroke-fonts/none").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// Through POST /commands, set_text_font selects a bundled stroke font for the project and answers with the text_font_changed patch.
/// POST /commands の set_text_font で同梱のストロークフォントをプロジェクトの書体に選べ、応答に text_font_changed のパッチが返る。
#[tokio::test]
async fn set_text_font_runs_through_the_commands_endpoint() {
    let (doc, router) = setup();
    let (status, patches) = post(
        &router,
        "/api/v1/commands",
        json!([{ "type": "set_text_font", "text_font": { "id": "kst32b", "latin": "full" } }]),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{patches}");
    assert_eq!(patches[0]["ops"][0]["op"], "text_font_changed");
    assert_eq!(patches[0]["ops"][0]["text_font"]["id"], "kst32b");
    let tf = doc.engine.lock().unwrap().project().text_font.clone().expect("書体");
    assert_eq!((tf.id.as_str(), tf.latin), ("kst32b", madake_core::model::Latin::Full));
}

/// Through POST /commands, set_text_font with a font that is not bundled is refused and the project keeps the system font.
/// POST /commands の set_text_font に同梱していない書体を渡すと拒否され、プロジェクトはシステムの書体のまま。
#[tokio::test]
async fn set_text_font_with_an_unknown_id_is_refused_through_the_commands_endpoint() {
    let (doc, router) = setup();
    let (status, _) = post(
        &router,
        "/api/v1/commands",
        json!([{ "type": "set_text_font", "text_font": { "id": "no-such-font", "latin": "half" } }]),
    )
    .await;
    assert!(status.is_client_error(), "{status}");
    assert!(doc.engine.lock().unwrap().project().text_font.is_none());
}
