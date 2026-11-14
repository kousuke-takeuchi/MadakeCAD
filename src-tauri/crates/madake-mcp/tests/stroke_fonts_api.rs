//! 同梱ストロークフォントの字形を返すLink APIのテスト。

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use madake_core::{Engine, Project};
use madake_mcp::SharedDoc;
use serde_json::Value;
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
