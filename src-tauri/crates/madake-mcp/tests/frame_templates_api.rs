//! 図枠テンプレートのLink API (`GET /frame-templates`) と、
//! `set_frame_template`コマンド (`POST /commands`) のテスト。
//!
//! 図枠の差し替えも他の編集と同じくCommandエンジンを通るので、undo一発で戻せる。

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
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// GET /frame-templates lists the bundled frame templates with their names in both languages and the folder where the user can drop their own.
/// GET /frame-templates は同梱の図枠テンプレートを英日の名前つきで返し、ユーザーが自分の図枠を置くフォルダの場所も添える。
#[tokio::test]
async fn the_link_api_lists_the_bundled_frame_templates() {
    let (_doc, router) = setup();
    let (status, body) = get(&router, "/api/v1/frame-templates").await;
    assert_eq!(status, StatusCode::OK);
    let templates = body["templates"].as_array().expect("templates配列");
    assert_eq!(templates.len(), 2);
    assert_eq!(templates[0]["id"], "jis-a3");
    assert_eq!(templates[0]["name_ja"], "JIS A3 横");
    assert_eq!(templates[1]["id"], "jis-a4-portrait");
    assert!(templates[0]["name"].as_str().is_some_and(|s| !s.is_empty()));
    assert!(body["issues"].as_array().expect("issues配列").is_empty());
    assert!(body["user_dir"].as_str().is_some_and(|s| s.ends_with("frames")));
}

/// The listed frame template carries the whole frame definition (paper, margin, zones, title-block cells and revision columns), so a client can preview it without reading any file.
/// 一覧には図枠の定義一式(用紙・マージン・ゾーン・表題欄のセル・改訂欄の列幅)が入っているので、クライアントはファイルを読まずにプレビューできる。
#[tokio::test]
async fn a_listed_frame_template_carries_its_whole_definition() {
    let (_doc, router) = setup();
    let (_status, body) = get(&router, "/api/v1/frame-templates").await;
    let a3 = &body["templates"][0];
    assert_eq!(a3["paper"]["w"], 420.0);
    assert_eq!(a3["frame_margin"], 10.0);
    assert_eq!(a3["zones"]["cols"], 4);
    assert_eq!(a3["zones"]["col_labels"], "numeric");
    assert_eq!(a3["title_block"]["w"], 120.0);
    assert_eq!(a3["title_block"]["origin"], "bottom_right");
    let cells = a3["title_block"]["cells"].as_array().expect("cells配列");
    assert!(cells.iter().any(|c| c["text"] == "図番"));
    assert!(cells
        .iter()
        .any(|c| c["bind"].as_str().is_some_and(|b| b.contains("{rev.latest}"))));
    assert_eq!(a3["revision_block"]["rows_max"], 6);
}

/// POST /commands with set_frame_template switches the whole project's frame, and one undo puts the previous frame back.
/// POST /commands の set_frame_template でプロジェクト全体の図枠が切り替わり、undo一発で元の図枠へ戻る。
#[tokio::test]
async fn setting_the_frame_template_over_the_link_api_is_one_undo_step() {
    let (doc, router) = setup();
    let (status, patches) = post(
        &router,
        "/api/v1/commands",
        json!([{ "type": "set_frame_template", "frame_template": "jis-a4-portrait" }]),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(patches[0]["ops"][0]["op"], "frame_template_changed");
    assert_eq!(
        doc.engine.lock().unwrap().project().frame_template.as_deref(),
        Some("jis-a4-portrait")
    );

    let (status, _) = post(&router, "/api/v1/undo", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        doc.engine.lock().unwrap().project().frame_template,
        None,
        "undo一発で既定の図枠へ戻る"
    );
}

/// The exported SVG follows the project's frame template: after switching to A4 portrait the title block is drawn 110 mm wide instead of 120 mm.
/// 書き出したSVGはプロジェクトの図枠テンプレートに従う: A4縦へ切り替えると表題欄が120mmではなく110mm幅で描かれる。
#[tokio::test]
async fn the_exported_svg_follows_the_projects_frame_template() {
    let (_doc, router) = setup();
    let path = std::env::temp_dir().join(format!(
        "madake-frame-export-{}-{}.svg",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    let export = |router: &Router, path: &std::path::Path| {
        let router = router.clone();
        let path = path.to_path_buf();
        async move {
            let (status, _) = post(
                &router,
                "/api/v1/export/svg",
                json!({ "path": path.display().to_string() }),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
            std::fs::read_to_string(&path).expect("書き出したSVG")
        }
    };

    let before = export(&router, &path).await;
    assert!(before.contains("width=\"120\" height=\"32\""), "{before}");

    post(
        &router,
        "/api/v1/commands",
        json!([{ "type": "set_frame_template", "frame_template": "jis-a4-portrait" }]),
    )
    .await;

    let after = export(&router, &path).await;
    assert!(after.contains("width=\"110\" height=\"32\""), "{after}");
    std::fs::remove_file(&path).ok();
}
