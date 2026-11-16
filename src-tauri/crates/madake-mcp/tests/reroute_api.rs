//! 配線の自動の引き直し (MCPツール `reroute_wires` と Link API `POST /api/v1/route/reroute`) のテスト。

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use madake_agent::tools::ToolBridge;
use madake_core::{Engine, Project};
use madake_mcp::tool_bridge::McpToolBridge;
use madake_mcp::{tool_descriptions, SharedDoc};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

const RIGHT_PIN_BLOCK: &str = r#"{"right":[{"no":"1","name":"A"}]}"#;
const LEFT_PIN_BLOCK: &str = r#"{"left":[{"no":"1","name":"A"}]}"#;

fn fixture() -> (SharedDoc, McpToolBridge, Router) {
    let doc = SharedDoc::new(Engine::new(Project::new("引き直し試験")));
    let parts = madake_mcp::open_parts(
        &std::env::temp_dir().join(format!("madake-reroute-{}.sqlite", Uuid::new_v4())),
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

/// Two parts joined by one wire; returns the sheet id.
async fn two_parts_one_wire(doc: &SharedDoc, bridge: &McpToolBridge) -> Uuid {
    let sheet = first_sheet(doc);
    for (reference, x, y, pins) in [("U1", 60.0, 60.0, RIGHT_PIN_BLOCK), ("U2", 140.0, 100.0, LEFT_PIN_BLOCK)] {
        call(
            bridge,
            "place_symbol",
            json!({"sheet_id": sheet, "symbol_id": "block", "x": x, "y": y,
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

fn entities_json(doc: &SharedDoc) -> Value {
    serde_json::to_value(&doc.engine.lock().unwrap().project().sheets[0].entities).unwrap()
}

/// The reroute tool says in Japanese that connections stay, that it is one undo step, and what it returns.
/// 引き直しのツール説明には、接続が変わらないこと・undo一発で戻ること・返り値の項目が書かれている。
#[test]
fn the_reroute_tool_is_advertised_with_its_promises() {
    let description = tool_descriptions()
        .into_iter()
        .find(|(tool, _)| tool == "reroute_wires")
        .expect("MCPツール「reroute_wires」が公開されていない")
        .1;
    for needle in ["接続は変えず", "undo一発", "unchanged", "wire_length_mm", "nets"] {
        assert!(description.contains(needle), "説明に「{needle}」が無い: {description}");
    }
}

/// The reroute tool applies its change as one edit, and one undo brings back exactly the original entities with their ids.
/// 引き直しのツールは変更を1回の編集として適用し、undo一発で元のエンティティがidまで含めてそのまま戻る。
#[tokio::test]
async fn the_tool_applies_one_batch_and_one_undo_restores_the_original() {
    let (doc, bridge, _) = fixture();
    let sheet = two_parts_one_wire(&doc, &bridge).await;
    let original = entities_json(&doc);
    let depth = doc.engine.lock().unwrap().undo_depth();
    let reply = call(&bridge, "reroute_wires", json!({"sheet_id": sheet})).await;
    assert_eq!(reply["rerouted"].as_array().map(Vec::len), Some(1), "{reply}");
    assert!(reply["before"]["crossings"].is_number() && reply["after"]["crossings"].is_number());
    assert!(reply["wire_length_mm"]["before"].as_f64().unwrap() > 0.0);
    assert!(reply["wire_length_mm"]["after"].as_f64().unwrap() > 0.0);
    assert_eq!(doc.engine.lock().unwrap().undo_depth(), depth + 1);
    assert_ne!(entities_json(&doc), original);
    doc.engine.lock().unwrap().undo().expect("元に戻せる");
    assert_eq!(entities_json(&doc), original);
    assert_eq!(doc.engine.lock().unwrap().undo_depth(), depth);
}

/// Asking for a net that does not exist changes nothing, adds no undo step, and the reply says so.
/// 存在しないネットを指定しても何も変わらず、undoの履歴も増えず、返事でそのことが分かる。
#[tokio::test]
async fn naming_an_unknown_net_changes_nothing() {
    let (doc, bridge, _) = fixture();
    let sheet = two_parts_one_wire(&doc, &bridge).await;
    let original = entities_json(&doc);
    let depth = doc.engine.lock().unwrap().undo_depth();
    let reply = call(&bridge, "reroute_wires", json!({"sheet_id": sheet, "nets": ["NOPE"]})).await;
    assert_eq!(reply["rerouted"], json!([]));
    let unchanged = reply["unchanged"].as_array().unwrap();
    assert!(unchanged.iter().any(|u| u["net"] == "NOPE"), "{reply}");
    assert_eq!(entities_json(&doc), original);
    assert_eq!(doc.engine.lock().unwrap().undo_depth(), depth);
}

/// A sheet with no wires gives an empty reply and no edit, not an empty batch.
/// 配線の無いシートには空の返事を返し、空のバッチも実行しない。
#[tokio::test]
async fn a_sheet_without_wires_gives_an_empty_reply_and_no_edit() {
    let (doc, bridge, _) = fixture();
    let depth = doc.engine.lock().unwrap().undo_depth();
    let reply = call(&bridge, "reroute_wires", json!({})).await;
    assert_eq!(reply["rerouted"], json!([]));
    assert_eq!(reply["unchanged"], json!([]));
    assert_eq!(doc.engine.lock().unwrap().undo_depth(), depth);
}

/// POST /api/v1/route/reroute gives the same reply shape as the tool, applies one batch, and one undo restores the original entities.
/// POST /api/v1/route/reroute はツールと同じ形の返事を返し、1回の編集として適用し、undo一発で元のエンティティに戻る。
#[tokio::test]
async fn the_endpoint_applies_one_batch_and_one_undo_restores_the_original() {
    let (doc, bridge, router) = fixture();
    let sheet = two_parts_one_wire(&doc, &bridge).await;
    let original = entities_json(&doc);
    let depth = doc.engine.lock().unwrap().undo_depth();
    let (status, reply) = post(&router, "/api/v1/route/reroute", json!({"sheet_id": sheet})).await;
    assert_eq!(status, StatusCode::OK, "{reply}");
    assert_eq!(reply["rerouted"].as_array().map(Vec::len), Some(1), "{reply}");
    for key in ["unchanged", "before", "after", "wire_length_mm"] {
        assert!(!reply[key].is_null(), "{key} が無い: {reply}");
    }
    assert_eq!(doc.engine.lock().unwrap().undo_depth(), depth + 1);
    doc.engine.lock().unwrap().undo().expect("元に戻せる");
    assert_eq!(entities_json(&doc), original);
}

/// The endpoint answers 400 for a sheet id that does not exist.
/// 存在しないシートidを渡すと、エンドポイントは400を返す。
#[tokio::test]
async fn the_endpoint_rejects_an_unknown_sheet() {
    let (_doc, _bridge, router) = fixture();
    let (status, _) = post(&router, "/api/v1/route/reroute", json!({"sheet_id": Uuid::new_v4()})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// The reroute tool refuses a grid spacing of zero with an error and changes nothing.
/// 引き直しのツールは格子の間隔0をエラーで断り、何も変えない。
#[tokio::test]
async fn the_tool_refuses_a_zero_grid_and_changes_nothing() {
    let (doc, bridge, _) = fixture();
    let sheet = two_parts_one_wire(&doc, &bridge).await;
    let original = entities_json(&doc);
    let outcome = bridge.call("reroute_wires", json!({"sheet_id": sheet, "options": {"grid_mm": 0.0}})).await;
    assert!(outcome.is_error, "{}", outcome.content);
    assert!(outcome.content.contains("grid_mm"), "{}", outcome.content);
    assert_eq!(entities_json(&doc), original);
}

/// POST /api/v1/route/reroute answers 400 with a message naming the setting when a cost is negative.
/// POST /api/v1/route/reroute は、費用が負のとき、その設定の名前を挙げて400を返す。
#[tokio::test]
async fn the_endpoint_refuses_a_negative_cost() {
    let (doc, bridge, router) = fixture();
    let sheet = two_parts_one_wire(&doc, &bridge).await;
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/route/reroute")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&json!({"sheet_id": sheet, "options": {"cross_cost": -2.0}})).unwrap()))
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("cross_cost"), "{text}");
}

/// A wire that is linked to a FreeCAD object is kept by the reroute, so the link keeps its wire.
/// FreeCADのオブジェクトと対応付けた配線は引き直さずに残し、対応付けが配線を失わない。
#[tokio::test]
async fn a_wire_linked_to_freecad_is_kept() {
    let (doc, bridge, _) = fixture();
    let sheet = two_parts_one_wire(&doc, &bridge).await;
    let wire_id = doc.engine.lock().unwrap().project().sheets[0]
        .entities
        .values()
        .find_map(|e| if let madake_core::Entity::Wire(w) = e { Some(w.id) } else { None })
        .expect("a wire");
    let link = madake_core::MechLink {
        entity_id: wire_id,
        fcstd_path: "panel.FCStd".into(),
        object_name: "Wire001".into(),
        synced_at: String::new(),
        placement: None,
    };
    doc.engine
        .lock()
        .unwrap()
        .execute(madake_core::Command::SetMechLink { link })
        .expect("link the wire");
    let reply = call(&bridge, "reroute_wires", json!({"sheet_id": sheet})).await;
    assert_eq!(reply["rerouted"], json!([]), "{reply}");
    assert!(reply["unchanged"].as_array().unwrap().iter().any(|u| u["reason"].as_str().unwrap_or("").contains("FreeCAD")), "{reply}");
    let still_there = doc.engine.lock().unwrap().project().sheets[0].entities.contains_key(&wire_id);
    assert!(still_there);
}
