//! プロジェクト内検索とデバイスツリーのLink API (`GET /search` / `GET /devices`) のテスト。
//!
//! UI (Tauri IPC) とLink APIは同じ`madake_core::search`の純関数を呼ぶので、
//! どちらの入口から引いても同じ結果・同じ並びになる。

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use madake_core::{Engine, Project};
use madake_mcp::SharedDoc;
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

fn setup() -> (SharedDoc, Router) {
    let doc = SharedDoc::new(Engine::new(Project::new("テストプロジェクト")));
    let agent = madake_mcp::agent::manager(&doc, 9310);
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let parts = madake_mcp::open_parts(&std::env::temp_dir().join(format!(
        "madake-parts-search-{}-{}.sqlite",
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

/// K1のコイルと接点、注記テキストを1枚のシートに置く。
async fn place_relay_and_note(router: &Router, sheet_id: Uuid) {
    let commands = json!([
        { "type": "add_entity", "sheet_id": sheet_id, "entity": {
            "kind": "symbol", "id": Uuid::new_v4(), "symbol_id": "relay_coil",
            "at": { "x": 100.0, "y": 100.0 }, "reference": "K1", "value": "MY2N" } },
        { "type": "add_entity", "sheet_id": sheet_id, "entity": {
            "kind": "symbol", "id": Uuid::new_v4(), "symbol_id": "relay_contact_no",
            "at": { "x": 250.0, "y": 100.0 }, "reference": "K1" } },
        { "type": "add_entity", "sheet_id": sheet_id, "entity": {
            "kind": "text", "id": Uuid::new_v4(),
            "at": { "x": 100.0, "y": 200.0 }, "text": "K1動作時に点灯", "height": 2.5 } }
    ]);
    let (status, body) = post(router, "/api/v1/commands", commands).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

/// GET /search finds every kind of target across the project and returns each hit with the sheet and zone to jump to.
/// GET /search はプロジェクト全体から全種別を探し、ジャンプ先のシートとゾーンつきで各ヒットを返す。
#[tokio::test]
async fn searching_over_the_link_api_returns_located_hits() {
    let (doc, router) = setup();
    let sheet_id = doc.engine.lock().unwrap().project().sheets[0].id;
    place_relay_and_note(&router, sheet_id).await;

    let (status, body) = get(&router, "/api/v1/search?q=K1").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let hits = body["hits"].as_array().expect("hits配列");
    assert_eq!(body["count"], 3, "コイル・接点・注記の3件 {body}");
    assert_eq!(hits.len(), 3);
    assert_eq!(
        hits.iter().map(|h| h["kind"].as_str().unwrap()).collect::<Vec<_>>(),
        vec!["reference", "reference", "text"]
    );
    for hit in hits {
        assert_eq!(hit["sheet_id"], serde_json::json!(sheet_id));
        assert_eq!(hit["sheet_no"], 1);
        assert!(hit["zone"].as_str().is_some_and(|z| !z.is_empty()), "{hit}");
        assert!(hit["entity_id"].as_str().is_some());
    }
    // 型番でも引ける (デバイスの機能つき)
    let (_, body) = get(&router, "/api/v1/search?q=my2n").await;
    assert_eq!(body["count"], 1);
    assert_eq!(body["hits"][0]["kind"], "value");
    assert_eq!(body["hits"][0]["detail"], "K1");
}

/// The kinds parameter narrows the search to the chosen filter chips, and an unknown filter name is rejected instead of guessed.
/// kindsパラメータは選んだフィルタチップだけに絞る。未知のフィルタ名は推測せずエラーにする。
#[tokio::test]
async fn the_kinds_parameter_narrows_the_search() {
    let (doc, router) = setup();
    let sheet_id = doc.engine.lock().unwrap().project().sheets[0].id;
    place_relay_and_note(&router, sheet_id).await;

    let (status, body) = get(&router, "/api/v1/search?q=K1&kinds=text").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["count"], 1);
    assert_eq!(body["hits"][0]["kind"], "text");

    let (_, body) = get(&router, "/api/v1/search?q=K1&kinds=reference,text").await;
    assert_eq!(body["count"], 3, "複数指定は和集合");

    let (status, _) = get(&router, "/api/v1/search?q=K1&kinds=colour").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "未知のフィルタ名は400");
}

/// An empty query returns no hits at all, so the search bar never dumps the whole drawing.
/// 空のクエリは1件も返さないので、検索バーが図面全体を並べてしまうことはない。
#[tokio::test]
async fn an_empty_query_returns_nothing() {
    let (doc, router) = setup();
    let sheet_id = doc.engine.lock().unwrap().project().sheets[0].id;
    place_relay_and_note(&router, sheet_id).await;

    let (status, body) = get(&router, "/api/v1/search?q=").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["count"], 0);
    assert!(body["hits"].as_array().unwrap().is_empty());

    // qを省略しても同じ (全件は返さない)
    let (status, body) = get(&router, "/api/v1/search").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["count"], 0);
}

/// GET /devices returns the reference-designator tree the device navigator draws, with each function's terminals and location.
/// GET /devices はデバイスナビゲータが描く参照記号ツリーを、機能ごとの端子と所在つきで返す。
#[tokio::test]
async fn the_device_tree_comes_back_over_the_link_api() {
    let (doc, router) = setup();
    let sheet_id = doc.engine.lock().unwrap().project().sheets[0].id;
    place_relay_and_note(&router, sheet_id).await;

    let (status, body) = get(&router, "/api/v1/devices").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let devices = body["devices"].as_array().expect("devices配列");
    assert_eq!(devices.len(), 1, "参照記号K1の1デバイス {body}");
    let k1 = &devices[0];
    assert_eq!(k1["reference"], "K1");
    assert_eq!(k1["kind"], "relay");
    assert_eq!(k1["value"], "MY2N");
    let functions = k1["functions"].as_array().expect("functions配列");
    assert_eq!(
        functions
            .iter()
            .map(|f| (f["kind"].as_str().unwrap(), f["terminals"].as_str().unwrap()))
            .collect::<Vec<_>>(),
        vec![("coil", "A1-A2"), ("contact_no", "13-14")]
    );
    assert_eq!(functions[0]["sheet_id"], serde_json::json!(sheet_id));
    assert!(functions[0]["zone"].as_str().is_some_and(|z| !z.is_empty()));
}

/// A drawing with nothing on it has an empty device tree rather than an error.
/// 何も置いていない図面のデバイスツリーは、エラーではなく空になる。
#[tokio::test]
async fn an_empty_drawing_has_an_empty_device_tree() {
    let (_doc, router) = setup();
    let (status, body) = get(&router, "/api/v1/devices").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["devices"].as_array().unwrap().is_empty());
}
