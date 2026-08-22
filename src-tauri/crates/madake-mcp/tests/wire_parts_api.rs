//! 電線品番表のLink API (`GET/POST /project/wire-parts`) と、
//! 拒否されたコマンドがきちんと400で返ること (G17) のテスト。
//!
//! どちらも編集はCommandエンジンを通るので、undo一発で元に戻る。

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
        "madake-parts-wireparts-{}-{}.sqlite",
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

fn sheet_id(doc: &SharedDoc) -> uuid::Uuid {
    doc.engine.lock().unwrap().project().sheets[0].id
}

/// 水色0.3sqのワイヤを1本置くコマンド。
fn add_wire(sheet: uuid::Uuid) -> Value {
    json!({
        "type": "add_entity", "sheet_id": sheet,
        "entity": { "kind": "wire", "id": uuid::Uuid::new_v4(),
            "points": [{"x": 10.0, "y": 10.0}, {"x": 50.0, "y": 10.0}],
            "color": "水色", "sq": 0.3 }
    })
}

/// A command the engine refuses comes back as HTTP 400 saying what went wrong and which command in the list failed, instead of a silent 200.
/// エンジンが拒否したコマンドは、黙って200で返るのではなく、理由と何番目のコマンドが落ちたかを添えたHTTP 400で返る。
#[tokio::test]
async fn a_refused_command_returns_400_with_the_reason_and_the_command_index() {
    let (doc, router) = setup();
    let sheet = sheet_id(&doc);
    let commands = json!([
        add_wire(sheet),
        { "type": "add_entity", "sheet_id": sheet,
          "entity": { "kind": "symbol", "id": uuid::Uuid::new_v4(),
              "symbol_id": "no_such_symbol", "at": {"x": 20.0, "y": 20.0}, "reference": "X1" } },
    ]);
    let (status, body) = post(&router, "/api/v1/commands", commands).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["failed_command_index"], 1, "2番目のコマンドで落ちた");
    assert_eq!(body["applied"], 1, "その前の1件は適用済み");
    assert!(
        body["error"].as_str().is_some_and(|e| e.contains("no_such_symbol")),
        "理由にシンボルidが入る: {body}"
    );
    // 拒否された部品はシートに入っていない (配線1本だけ)
    let entities = doc.engine.lock().unwrap().project().sheets[0].entities.len();
    assert_eq!(entities, 1);
}

/// A parametric symbol asked for beyond its pole limit is refused the same way, with a message that names the usable range.
/// 極数の上限を超えた極数可変シンボルも同じように拒否され、使える範囲がメッセージに書かれる。
#[tokio::test]
async fn a_pole_count_beyond_the_limit_is_refused_over_the_api() {
    let (doc, router) = setup();
    let sheet = sheet_id(&doc);
    let commands = json!([
        { "type": "add_entity", "sheet_id": sheet,
          "entity": { "kind": "symbol", "id": uuid::Uuid::new_v4(),
              "symbol_id": "terminal_block_99p", "at": {"x": 20.0, "y": 20.0}, "reference": "TB1" } },
    ]);
    let (status, body) = post(&router, "/api/v1/commands", commands).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["failed_command_index"], 0);
    assert!(
        body["error"].as_str().is_some_and(|e| e.contains("極数が範囲外")),
        "{body}"
    );
    assert!(doc.engine.lock().unwrap().project().sheets[0].entities.is_empty());
}

/// A valid command list still returns 200 with one patch per command.
/// 正しいコマンド列はこれまでどおり200で、コマンドごとのpatchが返る。
#[tokio::test]
async fn a_valid_command_list_still_succeeds() {
    let (doc, router) = setup();
    let sheet = sheet_id(&doc);
    let (status, body) = post(&router, "/api/v1/commands", json!([add_wire(sheet)])).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().expect("patch配列").len(), 1);
    assert_eq!(doc.engine.lock().unwrap().project().sheets[0].entities.len(), 1);
}

/// Posting a wire-part CSV fills the project's colour+gauge table, and the wire list then prints the part number of a wire that has none of its own.
/// 電線品番表のCSVを投げるとプロジェクトの線色+線径の表が埋まり、品番を持たないワイヤでも電線リストに品番が出る。
#[tokio::test]
async fn importing_a_wire_part_csv_fills_the_part_number_column_of_the_wire_list() {
    let (doc, router) = setup();
    let sheet = sheet_id(&doc);
    post(&router, "/api/v1/commands", json!([add_wire(sheet)])).await;

    let (status, _) = post(
        &router,
        "/api/v1/project/wire-parts/csv",
        json!({ "csv": "線色,線径sq,電線品番,備考\n水色,0.3,SAMPLE0001,\n" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = get(&router, "/api/v1/project/wire-parts").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["wire_parts"][0]["part_no"], "SAMPLE0001");
    assert!(body["csv"].as_str().is_some_and(|c| c.contains("水色,0.3,SAMPLE0001")));

    let path = std::env::temp_dir().join(format!("madake-wire-list-{}.csv", std::process::id()));
    let (status, _) = post(
        &router,
        "/api/v1/export/wire-list",
        json!({ "path": path.to_string_lossy() }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let csv = std::fs::read_to_string(&path).expect("電線リストが書かれる");
    assert!(csv.contains("SAMPLE0001"), "品番列が埋まっていない: {csv}");
    let _ = std::fs::remove_file(&path);

    // 表の差し替えもCommandなので、undo一発で元の (空の) 表に戻る
    let (status, _) = post(&router, "/api/v1/undo", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert!(doc.engine.lock().unwrap().project().wire_parts.is_empty());
}

/// A CSV that cannot be read is refused with 400 and the table is left exactly as it was.
/// 読めないCSVは400で拒否され、表は元のままになる。
#[tokio::test]
async fn a_malformed_wire_part_csv_is_refused_and_changes_nothing() {
    let (doc, router) = setup();
    post(
        &router,
        "/api/v1/project/wire-parts/csv",
        json!({ "csv": "水色,0.3,SAMPLE0001\n" }),
    )
    .await;
    let (status, _) = post(
        &router,
        "/api/v1/project/wire-parts/csv",
        json!({ "csv": "水色,0.3\n" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let parts = doc.engine.lock().unwrap().project().wire_parts.clone();
    assert_eq!(parts.len(), 1, "取り込みに失敗しても前の表が残る");
    assert_eq!(parts[0].part_no, "SAMPLE0001");
}
