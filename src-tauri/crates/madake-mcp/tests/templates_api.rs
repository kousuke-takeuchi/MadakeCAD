//! 開始テンプレートのLink API (`GET /templates` / `POST /templates/apply`) のテスト。
//!
//! UI(Tauri IPC)・MCPツール・Link APIは同じ`SharedDoc::apply_template`を通るので、
//! どの入口から適用しても1回の編集(undo一発)になる。

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
        "madake-parts-templates-{}-{}.sqlite",
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

async fn post(router: &Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// GET /templates lists the bundled start templates with their names in both languages.
/// GET /templates は同梱の開始テンプレートを英日の名前つきで返す。
#[tokio::test]
async fn the_link_api_lists_the_bundled_templates() {
    let (_doc, router) = setup();
    let (status, body) = get(&router, "/api/v1/templates").await;
    assert_eq!(status, StatusCode::OK);
    let templates = body["templates"].as_array().expect("templates配列");
    assert_eq!(templates.len(), 3);
    assert_eq!(templates[0]["id"], "control_24v_basic");
    assert_eq!(templates[0]["name_ja"], "24V制御基本");
    assert!(templates[0]["name"].as_str().is_some_and(|s| !s.is_empty()));
    assert!(body["issues"].as_array().expect("issues配列").is_empty());
}

/// POST /templates/apply drops the template on the sheet as a single edit that one undo takes back.
/// POST /templates/apply はテンプレートを1回の編集としてシートへ入れ、undo一発で戻せる。
#[tokio::test]
async fn applying_a_template_over_the_link_api_is_one_undo_step() {
    let (doc, router) = setup();
    let sheet_id = doc.engine.lock().unwrap().project().sheets[0].id;

    let (status, patch) = post(
        &router,
        "/api/v1/templates/apply",
        json!({ "template_id": "control_24v_basic", "sheet_id": sheet_id }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(patch["ops"].as_array().expect("ops").len() > 5);

    let placed = doc.engine.lock().unwrap().project().sheets[0]
        .entities
        .len();
    assert!(placed > 5, "{placed}件");

    let (status, _) = post(&router, "/api/v1/undo", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        doc.engine.lock().unwrap().project().sheets[0]
            .entities
            .is_empty(),
        "undo一発で空図面へ戻る"
    );
}

/// Applying without a sheet id targets the first sheet, and an unknown template id is refused with 400.
/// シートidを省くと先頭シートが対象になり、知らないテンプレートidは400で拒否される。
#[tokio::test]
async fn the_link_api_defaults_to_the_first_sheet_and_refuses_unknown_templates() {
    let (doc, router) = setup();
    let (status, _) = post(
        &router,
        "/api/v1/templates/apply",
        json!({ "template_id": "emergency_stop" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!doc.engine.lock().unwrap().project().sheets[0]
        .entities
        .is_empty());

    let (status, _) = post(
        &router,
        "/api/v1/templates/apply",
        json!({ "template_id": "no_such_template" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
