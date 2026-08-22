//! 整えメトリクスの公開口 (MCPツール `get_tidy_metrics` と Link API `/api/v1/tidy-metrics`)。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use madake_core::{Engine, Entity, Point, Project, Wire};
use madake_mcp::{tool_descriptions, SharedDoc};
use serde_json::Value;
use tower::ServiceExt;

fn fake_claude() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../madake-agent/tests/fixtures")
        .join("fake_claude.sh")
}

fn unique_parts_db(tag: &str) -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "madake-parts-{tag}-{}-{seq}.sqlite",
        std::process::id()
    ))
}

fn wire(points: &[(f64, f64)]) -> Entity {
    Entity::Wire(Wire {
        id: uuid::Uuid::new_v4(),
        points: points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
        color: "black".into(),
        sq: 0.75,
        length_m: None,
        part_no: None,
        net: None,
    })
}

/// The tidy metrics tool tells the agent it is the target to aim at while tidying, and names the three things it counts.
/// 整えメトリクスのツール説明には「整えループの目標値として使う」ことと、数える3つの対象が書かれている。
#[test]
fn the_tidy_metrics_tool_is_advertised_as_the_tidy_loop_target() {
    let description = tool_descriptions()
        .into_iter()
        .find(|(tool, _)| tool == "get_tidy_metrics")
        .expect("MCPツール「get_tidy_metrics」が公開されていない")
        .1;
    for needle in ["整えループの目標値として使う", "交差", "重なり", "グリッド"] {
        assert!(
            description.contains(needle),
            "get_tidy_metricsの説明に「{needle}」が無い: {description}"
        );
    }
}

/// GET /api/v1/tidy-metrics returns the sheet's crossing, label overlap, symbol overlap and off-grid counts as JSON.
/// GET /api/v1/tidy-metrics はシートの交差数・ラベル重なり数・シンボル重なり数・グリッド外数をJSONで返す。
#[tokio::test]
async fn tidy_metrics_endpoint_returns_the_four_counts() {
    let mut project = Project::new("整えAPI");
    let sheet_id = project.sheets[0].id;
    // X字に交差する2本 + グリッドから外れた頂点1つ
    for e in [
        wire(&[(50.0, 50.0), (100.0, 50.0)]),
        wire(&[(75.0, 25.0), (75.0, 75.1)]),
    ] {
        project.sheets[0].entities.insert(e.id(), e);
    }
    let doc = SharedDoc::new(Engine::new(project));
    let agent = madake_mcp::agent::manager(&doc, 9310);
    agent.set_executable(Some(fake_claude()));
    let parts = madake_mcp::open_parts(&unique_parts_db("tidy")).expect("parts db");
    let router = madake_mcp::link_api::router(doc, Arc::clone(&agent), parts);

    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/tidy-metrics?sheet_id={sheet_id}"))
        .body(Body::empty())
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["crossings"], 1);
    assert_eq!(body["label_overlaps"], 0);
    assert_eq!(body["symbol_overlaps"], 0);
    assert_eq!(body["off_grid"], 1);

    // sheet_id省略でも先頭シートを測る
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/tidy-metrics")
        .body(Body::empty())
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let same: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    assert_eq!(same, body);

    // 存在しないシートは400
    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/tidy-metrics?sheet_id={}", uuid::Uuid::new_v4()))
        .body(Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
