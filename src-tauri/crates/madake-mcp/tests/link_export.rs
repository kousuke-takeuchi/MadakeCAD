//! Link APIのエクスポートエンドポイントのテスト(サーバーは立てずRouterへ直接流す)。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use madake_core::{Engine, Project};
use madake_mcp::SharedDoc;
use serde_json::{json, Value};
use tower::ServiceExt;

fn fake_claude() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../madake-agent/tests/fixtures")
        .join("fake_claude.sh")
}

#[tokio::test]
async fn parts_endpoints_search_upsert_delete() {
    let doc = SharedDoc::new(Engine::new(Project::new("部品テスト")));
    let agent = madake_mcp::agent::manager(&doc, 9310);
    agent.set_executable(Some(fake_claude()));
    let db_path = std::env::temp_dir().join(format!("madake-parts-api-{}.sqlite", std::process::id()));
    std::fs::remove_file(&db_path).ok();
    let parts = madake_mcp::open_parts(&db_path).expect("parts db");
    let router = madake_mcp::link_api::router(doc, Arc::clone(&agent), parts);

    let call = |method: &'static str, uri: String, body: Option<Value>| {
        let router = router.clone();
        async move {
            let mut b = Request::builder().method(method).uri(uri);
            if body.is_some() {
                b = b.header("content-type", "application/json");
            }
            let req = b
                .body(match body {
                    Some(v) => Body::from(v.to_string()),
                    None => Body::empty(),
                })
                .unwrap();
            let res = router.oneshot(req).await.unwrap();
            let status = res.status();
            let bytes = res.into_body().collect().await.unwrap().to_bytes();
            (status, serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null))
        }
    };

    // サンプルが検索できる
    let (status, body) = call("GET", "/api/v1/parts?query=MDK-FUSE".into(), None).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body.as_array().unwrap().len(), 1);
    assert_eq!(body[0]["symbol_id"], "fuse");

    // upsert → 検索でヒット
    let (status, _) = call(
        "POST",
        "/api/v1/parts".into(),
        Some(json!({
            "part_no": "TEST-PB-01", "name": "押しボタン", "category": "switch",
            "symbol_id": "pushbutton_no", "rated_current_a": 3.0
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = call("GET", "/api/v1/parts?query=TEST-PB".into(), None).await;
    assert_eq!(body[0]["name"], "押しボタン");

    // カテゴリ絞り込み
    let (_, body) = call("GET", "/api/v1/parts?category=switch".into(), None).await;
    assert!(body.as_array().unwrap().iter().all(|p| p["category"] == "switch"));

    // 削除
    let (status, body) = call("DELETE", "/api/v1/parts/TEST-PB-01".into(), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["deleted"], true);
    let (_, body) = call("GET", "/api/v1/parts?query=TEST-PB".into(), None).await;
    assert!(body.as_array().unwrap().is_empty());

    // 電線品番マスタ
    let (status, body) = call("GET", "/api/v1/wire-parts".into(), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.as_array().unwrap().is_empty(), "電線サンプル");

    std::fs::remove_file(&db_path).ok();
}

#[tokio::test]
async fn verify_returns_diagnostics() {
    let mut project = Project::new("検証テスト");
    // 参照記号なしの抵抗を1個置く → erc.empty_reference と erc.unconnected_pin が出る
    let sheet_id = project.sheets[0].id;
    let entity = madake_core::Entity::Symbol(madake_core::SymbolInstance {
        id: uuid::Uuid::new_v4(),
        symbol_id: "resistor".into(),
        at: madake_core::Point::new(100.0, 50.0),
        rotation: 0,
        mirror: false,
        reference: String::new(),
        value: String::new(),
        attrs: Default::default(),
    });
    project.sheets[0].entities.insert(entity.id(), entity);
    let doc = SharedDoc::new(Engine::new(project));
    let agent = madake_mcp::agent::manager(&doc, 9310);
    agent.set_executable(Some(fake_claude()));
    let parts = madake_mcp::open_parts(
        &std::env::temp_dir().join(format!("madake-parts-export-{}.sqlite", std::process::id())),
    )
    .expect("parts db");
    let router = madake_mcp::link_api::router(doc, Arc::clone(&agent), parts);

    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/verify?sheet_id={sheet_id}"))
        .body(Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let diags = body.as_array().expect("diagnostics array");
    let codes: Vec<&str> = diags.iter().filter_map(|d| d["code"].as_str()).collect();
    assert!(codes.contains(&"erc.empty_reference"), "{codes:?}");
    assert!(codes.contains(&"erc.unconnected_pin"), "{codes:?}");
}

#[tokio::test]
async fn import_kicad_replaces_project_and_reports() {
    let doc = SharedDoc::new(Engine::new(Project::new("元プロジェクト")));
    let agent = madake_mcp::agent::manager(&doc, 9310);
    agent.set_executable(Some(fake_claude()));
    let parts = madake_mcp::open_parts(
        &std::env::temp_dir().join(format!("madake-parts-kicad-{}.sqlite", std::process::id())),
    )
    .expect("parts db");
    let router = madake_mcp::link_api::router(doc.clone(), Arc::clone(&agent), parts);

    let sch = std::env::temp_dir().join(format!("madake-import-{}.kicad_sch", std::process::id()));
    std::fs::write(
        &sch,
        r#"(kicad_sch (version 20250114) (paper "A4")
  (title_block (title "KiCadから"))
  (wire (pts (xy 10 10) (xy 50 10)))
  (symbol (lib_id "Device:R") (at 60 10 0)
    (property "Reference" "R9") (property "Value" "1k")))"#,
    )
    .unwrap();

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/import/kicad")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "path": sch.to_string_lossy() }).to_string(),
        ))
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["report"]["symbols"], 1);
    assert_eq!(body["report"]["wires"], 1);
    assert!(body["patch"]["revision"].is_number());

    let engine = doc.engine.lock().unwrap();
    assert_eq!(engine.project().sheets[0].title_block.title, "KiCadから");
    std::fs::remove_file(&sch).ok();
}

#[tokio::test]
async fn export_pdf_writes_pdf_file() {
    let doc = SharedDoc::new(Engine::new(Project::new("PDFテスト")));
    let agent = madake_mcp::agent::manager(&doc, 9310);
    agent.set_executable(Some(fake_claude()));
    let parts = madake_mcp::open_parts(
        &std::env::temp_dir().join(format!("madake-parts-export-{}.sqlite", std::process::id())),
    )
    .expect("parts db");
    let router = madake_mcp::link_api::router(doc, Arc::clone(&agent), parts);

    let dir = std::env::temp_dir().join(format!("madake-pdf-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("out.pdf");

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/export/pdf")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "path": out.to_string_lossy() }).to_string(),
        ))
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["written"], json!(out.to_string_lossy()));

    let pdf = std::fs::read(&out).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    std::fs::remove_dir_all(&dir).ok();
}
