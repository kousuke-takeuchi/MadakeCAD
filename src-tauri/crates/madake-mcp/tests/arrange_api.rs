//! 部品の並べ直し (MCPツール `arrange_sheet` と Link API `POST /api/v1/route/arrange`) のテスト。

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use madake_agent::tools::ToolBridge;
use madake_core::{Engine, Orientation, PaperSize, Project, Sheet};
use madake_mcp::tool_bridge::McpToolBridge;
use madake_mcp::{tool_descriptions, SharedDoc};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

const RIGHT_PIN_BLOCK: &str = r#"{"right":[{"no":"1","name":"A"}]}"#;
const LEFT_PIN_BLOCK: &str = r#"{"left":[{"no":"1","name":"A"}]}"#;

fn fixture() -> (SharedDoc, McpToolBridge, Router) {
    let mut project = Project::new("並べ直し試験");
    // 図枠に余裕のあるA3
    project.sheets = vec![Sheet::new("S1", PaperSize::A3, Orientation::Landscape)];
    let doc = SharedDoc::new(Engine::new(project));
    let parts = madake_mcp::open_parts(
        &std::env::temp_dir().join(format!("madake-arrange-{}.sqlite", Uuid::new_v4())),
    )
    .expect("部品DB");
    let bridge = McpToolBridge::new(doc.clone(), parts.clone());
    let agent = madake_mcp::agent::manager(&doc, 9310);
    let router = madake_mcp::link_api::router(doc.clone(), Arc::clone(&agent), parts);
    (doc, bridge, router)
}

async fn call(bridge: &McpToolBridge, name: &str, input: Value) -> Value {
    let outcome = bridge.call(name, input).await;
    assert!(!outcome.is_error, "{name} が失敗した: {}", outcome.content);
    serde_json::from_str(&outcome.content).expect("JSON")
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

fn first_sheet(doc: &SharedDoc) -> Uuid {
    doc.engine.lock().unwrap().project().sheets[0].id
}

fn entities_json(doc: &SharedDoc) -> Value {
    serde_json::to_value(&doc.engine.lock().unwrap().project().sheets[0].entities).unwrap()
}

/// Two parts closer than the channel width joined by one wire, well inside the frame; returns the sheet id.
async fn two_squeezed_parts(doc: &SharedDoc, bridge: &McpToolBridge) -> Uuid {
    let sheet = first_sheet(doc);
    for (reference, x, pins) in [("U1", 100.0, RIGHT_PIN_BLOCK), ("U2", 122.0, LEFT_PIN_BLOCK)] {
        call(
            bridge,
            "place_symbol",
            json!({"sheet_id": sheet, "symbol_id": "block", "x": x, "y": 100.0,
                   "reference": reference, "attrs": {"block_pins": pins}}),
        )
        .await;
    }
    call(
        bridge,
        "connect_pins",
        json!({"sheet_id": sheet, "connections": [{"from": "U1.1", "to": "U2.1"}]}),
    )
    .await;
    sheet
}

/// The arrange tool is advertised and its description says that one undo restores the drawing.
/// 並べ直しのツールが公開され、説明にundo一発で戻ることが書いてある。
#[test]
fn the_arrange_tool_is_advertised() {
    let description = tool_descriptions()
        .into_iter()
        .find(|(tool, _)| tool == "arrange_sheet")
        .expect("MCPツール「arrange_sheet」が公開されていない")
        .1;
    assert!(description.contains("undo"), "説明に「undo」が無い: {description}");
}

/// Arranging two squeezed parts over MCP moves them apart, reroutes the wire and one undo restores every entity.
/// MCPで詰まった2つの部品を並べ直すと離れて配線が引き直され、undo一発で全てのエンティティが元に戻る。
#[tokio::test]
async fn arranging_over_mcp_moves_reroutes_and_undoes_in_one_step() {
    let (doc, bridge, _) = fixture();
    let sheet = two_squeezed_parts(&doc, &bridge).await;
    let original = entities_json(&doc);
    let depth = doc.engine.lock().unwrap().undo_depth();
    let reply = call(&bridge, "arrange_sheet", json!({"sheet_id": sheet})).await;
    assert!(reply["moved"].as_u64().unwrap() >= 1, "{reply}");
    assert!(!reply["routed"].as_array().unwrap().is_empty(), "{reply}");
    assert_eq!(doc.engine.lock().unwrap().undo_depth(), depth + 1);
    assert_ne!(entities_json(&doc), original);
    doc.engine.lock().unwrap().undo().expect("元に戻せる");
    assert_eq!(entities_json(&doc), original);
    assert_eq!(doc.engine.lock().unwrap().undo_depth(), depth);
}

/// The Link API arrange endpoint gives the same reply shape and refuses bad options with 400 and no change.
/// Link APIの並べ直しは同じ形の返り値を返し、使えない設定は400で断って何も変えない。
#[tokio::test]
async fn the_link_api_arranges_and_refuses_bad_options() {
    let (doc, bridge, router) = fixture();
    two_squeezed_parts(&doc, &bridge).await;
    let revision = doc.engine.lock().unwrap().revision();
    let (status, bad) = post(&router, "/api/v1/route/arrange", json!({"options": {"channel_mm": 1}})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
    assert_eq!(doc.engine.lock().unwrap().revision(), revision);
    let (status, reply) = post(&router, "/api/v1/route/arrange", json!({})).await;
    assert_eq!(status, StatusCode::OK, "{reply}");
    for key in ["moved", "columns", "rows", "routed", "labelled", "before", "after", "wire_length_mm", "size_mm"] {
        assert!(reply.get(key).is_some(), "返り値に「{key}」が無い: {reply}");
    }
}

/// Adds a cable entity to the first sheet, which the arrange refuses.
fn add_cable(doc: &SharedDoc) {
    let sheet_id = first_sheet(doc);
    let id = Uuid::new_v4();
    let cable: madake_core::Cable = serde_json::from_value(json!({
        "id": id, "reference": "W1", "part_no": "SAMPLE-CBL", "core_count": 2, "sq": 1.25,
        "points": [{"x": 50.0, "y": 200.0}, {"x": 90.0, "y": 200.0}],
    }))
    .expect("cable");
    doc.engine
        .lock()
        .unwrap()
        .execute(madake_core::Command::AddEntity { sheet_id, entity: madake_core::Entity::Cable(cable) })
        .expect("ケーブルを足せる");
}

/// A sheet with a cable is refused over MCP and over the Link API, and the drawing and its revision stay as they were.
/// ケーブルのあるシートはMCPでもLink APIでも断られ、図面も版番号も変わらない。
#[tokio::test]
async fn a_refused_arrange_changes_nothing() {
    let (doc, bridge, router) = fixture();
    let sheet = two_squeezed_parts(&doc, &bridge).await;
    add_cable(&doc);
    let revision = doc.engine.lock().unwrap().revision();
    let original = entities_json(&doc);
    let outcome = bridge.call("arrange_sheet", json!({"sheet_id": sheet})).await;
    assert!(outcome.is_error, "断られるはず: {}", outcome.content);
    assert!(outcome.content.contains("ケーブル"), "{}", outcome.content);
    let (status, body) = post(&router, "/api/v1/route/arrange", json!({"sheet_id": sheet})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(doc.engine.lock().unwrap().revision(), revision);
    assert_eq!(entities_json(&doc), original);
}

/// Arranging a sheet that is already arranged applies no edit at all: no new undo step and no change of revision.
/// 並べ直し済みのシートをもう一度並べ直しても編集は1つも当てず、undoの履歴も版番号も増えない。
#[tokio::test]
async fn arranging_an_arranged_sheet_applies_no_edit() {
    let (doc, bridge, _) = fixture();
    let sheet = two_squeezed_parts(&doc, &bridge).await;
    call(&bridge, "arrange_sheet", json!({"sheet_id": sheet})).await;
    let revision = doc.engine.lock().unwrap().revision();
    let depth = doc.engine.lock().unwrap().undo_depth();
    let reply = call(&bridge, "arrange_sheet", json!({"sheet_id": sheet})).await;
    assert_eq!(reply["moved"], json!(0), "{reply}");
    assert_eq!(reply["labelled"], json!([]), "{reply}");
    assert_eq!(doc.engine.lock().unwrap().revision(), revision);
    assert_eq!(doc.engine.lock().unwrap().undo_depth(), depth);
}
