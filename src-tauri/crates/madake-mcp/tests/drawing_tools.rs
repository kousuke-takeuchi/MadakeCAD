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

fn unconnected_pins(diagnostics: &Value) -> usize {
    diagnostics
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "erc.unconnected_pin")
        .count()
}

const RIGHT_PIN_BLOCK: &str = r#"{"right":[{"no":"1","name":"A"}]}"#;

/// connect_pins wires two pins by name with right-angle segments, and verification then finds no unconnected pin.
/// connect_pins はピン名を指定した2本のピンを直角の線分で結び、その後の検証で未接続ピンが出ない。
#[tokio::test]
async fn connect_pins_wires_two_pins_by_name() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    place_block(&bridge, sheet, "U1", 60.0, 60.0, RIGHT_PIN_BLOCK).await;
    place_block(&bridge, sheet, "U2", 120.0, 90.0, ONE_PIN_BLOCK).await;
    let before = unconnected_pins(&call(&bridge, "run_verification", json!({"sheet_id": sheet})).await);
    assert_eq!(before, 2);
    let wired = call(
        &bridge,
        "connect_pins",
        json!({"sheet_id": sheet, "from": "U1.A", "to": "U2.1", "color": "red", "sq": 0.5}),
    )
    .await;
    let pts = wired["points"].as_array().unwrap();
    assert_eq!(pts.len(), 4, "Z 字になっていない: {pts:?}");
    for w in pts.windows(2) {
        assert!(w[0]["x"] == w[1]["x"] || w[0]["y"] == w[1]["y"], "斜めの線分: {w:?}");
    }
    let after = unconnected_pins(&call(&bridge, "run_verification", json!({"sheet_id": sheet})).await);
    assert_eq!(after, 0);
    let engine = doc.engine.lock().unwrap();
    let wire = engine.project().sheets[0]
        .entities
        .values()
        .find_map(|e| match e {
            madake_core::Entity::Wire(w) => Some(w.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!((wire.color.as_str(), wire.sq), ("red", 0.5));
}

/// connect_pins passes the given via points in order.
/// connect_pins は渡した経由点を順に通る。
#[tokio::test]
async fn connect_pins_passes_via_points() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    place_block(&bridge, sheet, "U1", 60.0, 60.0, RIGHT_PIN_BLOCK).await;
    place_block(&bridge, sheet, "U2", 120.0, 90.0, ONE_PIN_BLOCK).await;
    let wired = call(
        &bridge,
        "connect_pins",
        json!({"sheet_id": sheet, "from": "U1.1", "to": "U2.1", "via": [{"x": 90.0, "y": 40.0}]}),
    )
    .await;
    assert!(wired["points"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["x"] == 90.0 && p["y"] == 40.0));
}

/// connect_pins refuses an unknown part, an unknown pin, an ambiguous reference and a pin wired to itself, and creates nothing.
/// connect_pins は不明な部品・不明なピン・どれか決まらない参照記号・同じピンどうしを断り、何も作らない。
#[tokio::test]
async fn connect_pins_refuses_bad_specifiers_and_creates_nothing() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    place_block(&bridge, sheet, "U1", 60.0, 60.0, TWO_PIN_BLOCK).await;
    place_block(&bridge, sheet, "J1", 20.0, 20.0, ONE_PIN_BLOCK).await;
    place_block(&bridge, sheet, "J1", 20.0, 60.0, ONE_PIN_BLOCK).await;
    let count = |doc: &SharedDoc| doc.engine.lock().unwrap().project().sheets[0].entities.len();
    let before = count(&doc);
    let e = call_err(&bridge, "connect_pins", json!({"sheet_id": sheet, "from": "U9.1", "to": "U1.IN1"})).await;
    assert!(e.contains("U1"), "{e}");
    let e = call_err(&bridge, "connect_pins", json!({"sheet_id": sheet, "from": "U1.IN9", "to": "U1.IN1"})).await;
    assert!(e.contains("OUT1"), "{e}");
    let e = call_err(&bridge, "connect_pins", json!({"sheet_id": sheet, "from": "J1.1", "to": "U1.IN1"})).await;
    assert!(e.contains("entity id"), "{e}");
    let e = call_err(&bridge, "connect_pins", json!({"sheet_id": sheet, "from": "U1.IN1", "to": "U1.入力1"})).await;
    assert!(e.contains("同じピン"), "{e}");
    assert_eq!(count(&doc), before);
}
