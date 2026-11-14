//! 図枠テンプレート一覧のロゴ画像のテスト。ユーザー図枠フォルダを環境変数で選ぶため、
//! 他のテストと並行実行で競合しないよう、このファイル (= 別プロセス) に1本だけ置く。

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

/// The frame template list sent to the canvas carries the logo image of a user template as a data URI.
/// キャンバスへ送る図枠テンプレートの一覧は、ユーザーの図枠のロゴ画像をdata URIで運ぶ。
#[tokio::test]
async fn the_template_list_carries_the_logo_image_of_a_user_template() {
    use base64::Engine as _;
    let dir = std::env::temp_dir().join(format!("madake-logo-api-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = base64::engine::general_purpose::STANDARD
        .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==")
        .unwrap();
    std::fs::write(dir.join("logo.png"), png).unwrap();
    let mut template = madake_core::frame_template::default_template().clone();
    template.id = "with-logo".into();
    template.logo = Some(madake_core::frame_template::LogoFrame {
        x: 300.0,
        y: 260.0,
        w: 40.0,
        h: 10.0,
        path: Some("logo.png".into()),
        data_url: None,
    });
    let mut value = serde_json::to_value(&template).unwrap();
    value.as_object_mut().unwrap().remove("builtin");
    std::fs::write(dir.join("with-logo.json"), serde_json::to_string(&value).unwrap()).unwrap();

    // 図枠フォルダは環境変数で選ぶ。このファイルのテストはこの1本だけなので並行して競合しない。
    std::env::set_var(madake_core::frame_template::USER_FRAME_TEMPLATES_PATH_ENV, &dir);
    let (_doc, router) = setup();
    let (status, body) = get(&router, "/api/v1/frame-templates").await;
    assert_eq!(status, StatusCode::OK);
    let t = body["templates"].as_array().unwrap().iter().find(|t| t["id"] == "with-logo").expect("listed");
    let url = t["logo"]["data_url"].as_str().expect("data_url");
    assert!(url.starts_with("data:image/png;base64,"), "{url}");
}
