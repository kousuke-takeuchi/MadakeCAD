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
async fn export_pdf_writes_pdf_file() {
    let doc = SharedDoc::new(Engine::new(Project::new("PDFテスト")));
    let agent = madake_mcp::agent::manager(&doc, 9310);
    agent.set_executable(Some(fake_claude()));
    let router = madake_mcp::link_api::router(doc, Arc::clone(&agent));

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
