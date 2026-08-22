//! PLC I/OのLink API (`GET/PUT /plc/assignments`・`POST /plc/assignments/import`・
//! `POST /plc/generate`・`GET /plc/modules`) とMCPツールのテスト。
//!
//! UI(Tauri IPC)・MCPツール・Link APIは同じ`SharedDoc`のメソッドを通るので、
//! どの入口から編集しても1回の編集(undo一発)になる。

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
    let doc = SharedDoc::new(Engine::new(Project::new("PLCテスト")));
    let agent = madake_mcp::agent::manager(&doc, 9310);
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let parts = madake_mcp::open_parts(&std::env::temp_dir().join(format!(
        "madake-parts-plc-{}-{}.sqlite",
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

async fn send(router: &Router, method: &str, uri: &str, body: Value) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// 16点の入力モジュール定義 (三菱式)。
fn di16() -> Value {
    json!({
        "points": 16,
        "kind": "DI",
        "address_prefix": "X",
        "address_style": "mitsubishi"
    })
}

/// PUT /plc/assignments replaces the whole I/O assignment table, and GET reads it back per module.
/// PUT /plc/assignments はI/O割付表を丸ごと置き換え、GETはモジュールごとの表として読み出す。
#[tokio::test]
async fn the_assignment_table_can_be_written_and_read_over_the_link_api() {
    let (_doc, router) = setup();
    let (status, _) = send(
        &router,
        "PUT",
        "/api/v1/plc/assignments",
        json!({ "assignments": [
            { "id": Uuid::new_v4(), "module_ref": "PLC1", "address": "X0",
              "signal_name": "起動押釦", "comment": "PB1" },
            { "id": Uuid::new_v4(), "module_ref": "PLC1", "address": "X1",
              "signal_name": "停止押釦", "comment": "" }
        ]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = get(&router, "/api/v1/plc/assignments?module_ref=PLC1").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let points = body["modules"][0]["points"].as_array().expect("points");
    assert_eq!(points.len(), 2);
    assert_eq!(points[0]["point"], 1);
    assert_eq!(points[0]["address"], "X0");
    assert_eq!(points[0]["signal_name"], "起動押釦");
    assert_eq!(points[0]["target"], "", "図面が空なら接続先も空");
}

/// POST /plc/assignments/import loads a CSV of address, signal name and comment for one module, and one undo takes the whole import back.
/// POST /plc/assignments/import はアドレス・信号名・コメントのCSVを1モジュールぶん取り込み、undo一発で取り込み前へ戻る。
#[tokio::test]
async fn a_csv_of_signal_names_can_be_imported_and_undone() {
    let (doc, router) = setup();
    let (status, _) = send(
        &router,
        "POST",
        "/api/v1/plc/assignments/import",
        json!({
            "module_ref": "PLC1",
            "csv": "アドレス,信号名,コメント\nX0,起動押釦,PB1\nX1,停止押釦,PB2\n"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(doc.engine.lock().unwrap().project().plc_assignments.len(), 2);

    doc.undo().unwrap().unwrap();
    assert!(doc
        .engine
        .lock()
        .unwrap()
        .project()
        .plc_assignments
        .is_empty());
}

/// A CSV whose rows do not have the three columns is refused with 400 and leaves the assignment table untouched.
/// 3列に足りない行のあるCSVは400で断られ、割付表は元のまま変わらない。
#[tokio::test]
async fn a_malformed_csv_import_is_refused_with_a_bad_request() {
    let (doc, router) = setup();
    let (status, _) = send(
        &router,
        "POST",
        "/api/v1/plc/assignments/import",
        json!({ "module_ref": "PLC1", "csv": "X0\nX1\n" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(doc
        .engine
        .lock()
        .unwrap()
        .project()
        .plc_assignments
        .is_empty());
}

/// POST /plc/generate builds the I/O ladder page as a single edit: the new sheet carries the module symbol and one rung per point, and GET /plc/modules then lists the placed module.
/// POST /plc/generate はI/Oラダーページを1回の編集で作り、新しいシートにはモジュールのシンボルと点数ぶんのラングが載る。GET /plc/modules はその置かれたモジュールを一覧に返す。
#[tokio::test]
async fn generating_an_io_page_over_the_link_api_is_one_undo_step() {
    let (doc, router) = setup();
    let (status, patch) = send(
        &router,
        "POST",
        "/api/v1/plc/generate",
        json!({
            "module_ref": "PLC1",
            "module": di16(),
            "options": { "rung_spacing_mm": 10.0, "start_skip": 0,
                         "ladder_style": "vertical-bus", "placement": "new-ladder" }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{patch}");
    {
        let engine = doc.engine.lock().unwrap();
        assert_eq!(engine.project().sheets.len(), 2, "ページが1枚増える");
        assert_eq!(
            engine.project().plc_assignments.len(),
            16,
            "割付表も16点ぶん自動採番される"
        );
    }

    let (status, body) = get(&router, "/api/v1/plc/modules").await;
    assert_eq!(status, StatusCode::OK);
    let modules = body["modules"].as_array().expect("modules");
    assert_eq!(modules.len(), 1);
    assert_eq!(modules[0]["reference"], "PLC1");
    assert_eq!(modules[0]["points"], 16);
    assert_eq!(modules[0]["kind"], "DI");

    doc.undo().unwrap().unwrap();
    let engine = doc.engine.lock().unwrap();
    assert_eq!(engine.project().sheets.len(), 1, "undo一発でページごと消える");
    assert!(engine.project().plc_assignments.is_empty());
}

/// Generation settings that are not implemented yet (sharing a ladder between modules) are refused with 400 and change nothing.
/// まだ実装していない生成設定 (モジュールの同居) は400で断られ、図面には何も起きない。
#[tokio::test]
async fn an_unimplemented_generation_setting_is_refused_with_a_bad_request() {
    let (doc, router) = setup();
    let (status, _) = send(
        &router,
        "POST",
        "/api/v1/plc/generate",
        json!({
            "module_ref": "PLC1",
            "module": di16(),
            "options": { "rung_spacing_mm": 10.0, "start_skip": 0,
                         "ladder_style": "vertical-bus", "placement": "share-or-split" }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(doc.engine.lock().unwrap().project().sheets.len(), 1);
}

/// The PLC I/O report is exported through the ordinary report endpoint as "plc-io", listing one row per assigned point.
/// PLC I/Oレポートは他の帳票と同じ書き出し口から`plc-io`として出力され、割付済みの点ごとに1行が並ぶ。
#[tokio::test]
async fn the_plc_io_report_is_exported_like_any_other_report() {
    let (_doc, router) = setup();
    send(
        &router,
        "POST",
        "/api/v1/plc/assignments/import",
        json!({ "module_ref": "PLC1", "csv": "X0,起動押釦,PB1\n" }),
    )
    .await;
    let path = std::env::temp_dir().join(format!("madake-plc-io-{}.csv", std::process::id()));
    let (status, body) = send(
        &router,
        "POST",
        "/api/v1/export/report",
        json!({ "path": path.display().to_string(), "kind": "plc-io", "format": "csv" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["count"], 1, "本文1行");
    let csv = std::fs::read_to_string(&path).expect("書き出したCSV");
    assert_eq!(
        csv.lines().next().unwrap(),
        "モジュール,アドレス,信号名,接続先,線番,コメント"
    );
    assert!(csv.contains("PLC1,X0,起動押釦"), "{csv}");
    std::fs::remove_file(&path).ok();
}

/// The PLC tools tell the agent what the assignment table is for and that generating a page is one undo step.
/// PLCのMCPツールの説明には、割付表が何のためのものか・ページ生成がundo一発であることが書かれている。
#[test]
fn the_plc_tools_explain_the_assignment_table_and_the_generated_page() {
    let tools = madake_mcp::tool_descriptions();
    let description_of = |name: &str| {
        tools
            .iter()
            .find(|(tool, _)| tool == name)
            .unwrap_or_else(|| panic!("MCPツール「{name}」が公開されていない"))
            .1
            .clone()
    };
    let read = description_of("get_plc_assignments");
    for needle in ["信号名", "接続先", "線番"] {
        assert!(read.contains(needle), "get_plc_assignmentsの説明に「{needle}」が無い: {read}");
    }
    let generate = description_of("generate_plc_sheet");
    for needle in ["ラング", "undo一発", "自動採番"] {
        assert!(
            generate.contains(needle),
            "generate_plc_sheetの説明に「{needle}」が無い: {generate}"
        );
    }
    assert!(description_of("set_plc_assignments").contains("undo一発"));
}
