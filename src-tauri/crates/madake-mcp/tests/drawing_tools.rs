//! ピン名で配線するMCPツール (get_pins / connect_pins / place_mate、place_symbol のピン) のテスト。

use madake_agent::tools::ToolBridge;
use madake_core::{Engine, Project};
use madake_mcp::tool_bridge::McpToolBridge;
use madake_mcp::{tool_descriptions, SharedDoc};
use serde_json::{json, Value};
use uuid::Uuid;

fn fixture() -> (SharedDoc, McpToolBridge) {
    let doc = SharedDoc::new(Engine::new(Project::new("ピン試験")));
    let parts = madake_mcp::open_parts(
        &std::env::temp_dir().join(format!("madake-pins-{}.sqlite", Uuid::new_v4())),
    )
    .expect("部品DB");
    let bridge = McpToolBridge::new(doc.clone(), parts);
    (doc, bridge)
}

fn first_sheet(doc: &SharedDoc) -> Uuid {
    doc.engine.lock().unwrap().project().sheets[0].id
}

async fn call(bridge: &McpToolBridge, name: &str, input: Value) -> Value {
    let outcome = bridge.call(name, input).await;
    assert!(!outcome.is_error, "{name} が失敗した: {}", outcome.content);
    serde_json::from_str(&outcome.content).expect("JSON")
}

async fn call_err(bridge: &McpToolBridge, name: &str, input: Value) -> String {
    let outcome = bridge.call(name, input).await;
    assert!(outcome.is_error, "{name} が成功してしまった: {}", outcome.content);
    outcome.content
}

const ONE_PIN_BLOCK: &str = r#"{"left":[{"no":"1","name":"A"}]}"#;
const TWO_PIN_BLOCK: &str =
    r#"{"left":[{"no":"IN1","name":"入力1"}],"right":[{"no":"OUT1","name":"出力1"}]}"#;

async fn place_block(
    bridge: &McpToolBridge,
    sheet: Uuid,
    reference: &str,
    x: f64,
    y: f64,
    pins: &str,
) -> Value {
    call(
        bridge,
        "place_symbol",
        json!({"sheet_id": sheet, "symbol_id": "block", "x": x, "y": y,
               "reference": reference, "attrs": {"block_pins": pins}}),
    )
    .await
}

/// place_symbol returns the new part's pins with paper positions, so the agent can wire it without computing coordinates.
/// place_symbol は置いた部品のピンを用紙上の位置つきで返すので、エージェントは座標を計算せずに配線できる。
#[tokio::test]
async fn place_symbol_returns_the_pins_of_the_new_part() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    let placed = place_block(&bridge, sheet, "CTRL1", 100.0, 50.0, TWO_PIN_BLOCK).await;
    assert!(placed["entity_id"].is_string());
    assert!(placed["revision"].is_number());
    let pins = placed["pins"].as_array().expect("pins");
    let in1 = pins.iter().find(|p| p["number"] == "IN1").expect("IN1");
    assert_eq!(in1["name"], "入力1");
    assert_eq!(in1["dir"], "left");
    assert!(in1["x"].as_f64().unwrap() < 100.0);
}

/// get_pins finds parts by reference or entity id and returns their pins and outline.
/// get_pins は参照記号か entity id で部品を探し、ピンと外形を返す。
#[tokio::test]
async fn get_pins_finds_parts_by_reference_or_entity_id() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    let placed = place_block(&bridge, sheet, "U1", 60.0, 60.0, ONE_PIN_BLOCK).await;
    let by_ref = call(&bridge, "get_pins", json!({"sheet_id": sheet, "reference": "U1"})).await;
    let parts = by_ref["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0]["pins"][0]["number"], "1");
    assert!(parts[0]["bbox"]["min"]["x"].is_number());
    let by_id = call(
        &bridge,
        "get_pins",
        json!({"sheet_id": sheet, "entity_id": placed["entity_id"]}),
    )
    .await;
    assert_eq!(by_id["parts"][0]["reference"], "U1");
    let none = call_err(&bridge, "get_pins", json!({"sheet_id": sheet})).await;
    assert!(none.contains("reference"), "{none}");
}

/// The pin tools tell the agent to wire by pin name instead of computing pin coordinates.
/// ピン操作ツールの説明には、ピン座標を計算せずピン名で配線するよう書かれている。
#[test]
fn the_pin_tools_tell_the_agent_not_to_compute_coordinates() {
    let tools = tool_descriptions();
    let get = |n: &str| {
        tools
            .iter()
            .find(|(t, _)| t == n)
            .unwrap_or_else(|| panic!("{n} が無い"))
            .1
            .clone()
    };
    assert!(get("get_pins").contains("用紙座標"));
    assert!(get("place_symbol").contains("pins"));
}
