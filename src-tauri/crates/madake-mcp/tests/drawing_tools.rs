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
    assert!(
        outcome.is_error,
        "{name} が成功してしまった: {}",
        outcome.content
    );
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
    let by_ref = call(
        &bridge,
        "get_pins",
        json!({"sheet_id": sheet, "reference": "U1"}),
    )
    .await;
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
    let before =
        unconnected_pins(&call(&bridge, "run_verification", json!({"sheet_id": sheet})).await);
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
        assert!(
            w[0]["x"] == w[1]["x"] || w[0]["y"] == w[1]["y"],
            "斜めの線分: {w:?}"
        );
    }
    let after =
        unconnected_pins(&call(&bridge, "run_verification", json!({"sheet_id": sheet})).await);
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
    let count = |doc: &SharedDoc| {
        doc.engine.lock().unwrap().project().sheets[0]
            .entities
            .len()
    };
    let before = count(&doc);
    let e = call_err(
        &bridge,
        "connect_pins",
        json!({"sheet_id": sheet, "from": "U9.1", "to": "U1.IN1"}),
    )
    .await;
    assert!(e.contains("U1"), "{e}");
    let e = call_err(
        &bridge,
        "connect_pins",
        json!({"sheet_id": sheet, "from": "U1.IN9", "to": "U1.IN1"}),
    )
    .await;
    assert!(e.contains("OUT1"), "{e}");
    let e = call_err(
        &bridge,
        "connect_pins",
        json!({"sheet_id": sheet, "from": "J1.1", "to": "U1.IN1"}),
    )
    .await;
    assert!(e.contains("entity id"), "{e}");
    let e = call_err(
        &bridge,
        "connect_pins",
        json!({"sheet_id": sheet, "from": "U1.IN1", "to": "U1.入力1"}),
    )
    .await;
    assert!(e.contains("同じピン"), "{e}");
    assert_eq!(count(&doc), before);
}

/// place_mate puts a plug/socket pair in one step, both carrying the designator without any reference warning, returns both outer connection points, and one undo removes the pair.
/// place_mate はプラグとソケットの組を1回で置き、両方に参照記号が付いて参照記号の警告は出ず、外側の2つの接続点を返し、1回の取り消しで組ごと消える。
#[tokio::test]
async fn place_mate_puts_a_pair_that_one_undo_removes() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    let placed = call(
        &bridge,
        "place_mate",
        json!({"sheet_id": sheet, "x": 100.0, "y": 50.0, "orientation": "vertical",
               "male_side": "first", "reference": "J5", "value": "XH-3P"}),
    )
    .await;
    assert_eq!(
        (placed["first"]["x"].as_f64(), placed["first"]["y"].as_f64()),
        (Some(100.0), Some(42.5))
    );
    assert_eq!(
        (
            placed["second"]["x"].as_f64(),
            placed["second"]["y"].as_f64()
        ),
        (Some(100.0), Some(62.5))
    );
    assert_eq!(
        doc.engine.lock().unwrap().project().sheets[0]
            .entities
            .len(),
        2
    );
    let pins = call(
        &bridge,
        "get_pins",
        json!({"sheet_id": sheet, "reference": "J5"}),
    )
    .await;
    let mut kinds: Vec<&str> = pins["parts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["symbol_id"].as_str().unwrap())
        .collect();
    kinds.sort();
    assert_eq!(kinds, ["connector_plug", "connector_socket"]);
    let diags = call(&bridge, "run_verification", json!({"sheet_id": sheet})).await;
    assert!(
        !diags
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "erc.empty_reference" || d["code"] == "erc.duplicate_reference"),
        "{diags}"
    );
    doc.undo().expect("元に戻せる");
    assert_eq!(
        doc.engine.lock().unwrap().project().sheets[0]
            .entities
            .len(),
        0
    );
}

/// connect_pins refuses a route that would run over a third pin and creates nothing, so no pin is joined to a net by accident.
/// connect_pins は別のピンの上を通る経路を断って何も作らないので、ピンが誤って回路に入ることがない。
#[tokio::test]
async fn connect_pins_refuses_a_route_over_another_pin() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    place_block(&bridge, sheet, "CTRL1", 100.0, 50.0, TWO_PIN_BLOCK).await;
    place_block(&bridge, sheet, "U2", 200.0, 50.0, ONE_PIN_BLOCK).await;
    let count = |doc: &SharedDoc| {
        doc.engine.lock().unwrap().project().sheets[0]
            .entities
            .len()
    };
    let before = count(&doc);
    let e = call_err(
        &bridge,
        "connect_pins",
        json!({"sheet_id": sheet, "from": "CTRL1.IN1", "to": "U2.1"}),
    )
    .await;
    assert!(e.contains("CTRL1.OUT1") && e.contains("via"), "{e}");
    assert_eq!(count(&doc), before);
}

/// connect_pins takes a list of connections and draws them all in one call that one undo removes, so the agent can wire a whole sheet without one call per wire.
/// connect_pins は配線のリストを受け取って1回で全部引き、1回の取り消しで全部消えるので、エージェントは配線1本ごとに呼ばずにシートを配線できる。
#[tokio::test]
async fn connect_pins_draws_a_list_of_connections_in_one_edit() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    place_block(&bridge, sheet, "U1", 60.0, 60.0, RIGHT_PIN_BLOCK).await;
    place_block(&bridge, sheet, "U2", 120.0, 90.0, ONE_PIN_BLOCK).await;
    place_block(&bridge, sheet, "U3", 60.0, 120.0, RIGHT_PIN_BLOCK).await;
    place_block(&bridge, sheet, "U4", 120.0, 150.0, ONE_PIN_BLOCK).await;
    let depth = doc.engine.lock().unwrap().undo_depth();
    let wired = call(
        &bridge,
        "connect_pins",
        json!({"sheet_id": sheet, "color": "red", "connections": [
            {"from": "U1.1", "to": "U2.1"},
            {"from": "U3.1", "to": "U4.1", "color": "black", "sq": 0.75}
        ]}),
    )
    .await;
    let wires = wired["wires"].as_array().expect("wires");
    assert_eq!(wires.len(), 2);
    assert!(wires[0]["points"].as_array().unwrap().len() >= 2);
    assert_eq!(doc.engine.lock().unwrap().undo_depth(), depth + 1);
    let diags = call(&bridge, "run_verification", json!({"sheet_id": sheet})).await;
    assert_eq!(unconnected_pins(&diags), 0);
    let colors: Vec<String> = doc.engine.lock().unwrap().project().sheets[0]
        .entities
        .values()
        .filter_map(|e| match e {
            madake_core::Entity::Wire(w) => Some(format!("{}/{}", w.color, w.sq)),
            _ => None,
        })
        .collect();
    assert!(
        colors.contains(&"red/0.3".to_string()) && colors.contains(&"black/0.75".to_string()),
        "{colors:?}"
    );
    doc.undo().expect("元に戻せる");
    assert_eq!(
        doc.engine.lock().unwrap().project().sheets[0]
            .entities
            .len(),
        4
    );
}

/// If any connection in the list is wrong, connect_pins names its index and creates none of them.
/// リストの配線のどれか1本でも誤っていれば、connect_pins はその番号を知らせて1本も作らない。
#[tokio::test]
async fn connect_pins_list_is_all_or_nothing() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    place_block(&bridge, sheet, "U1", 60.0, 60.0, RIGHT_PIN_BLOCK).await;
    place_block(&bridge, sheet, "U2", 120.0, 90.0, ONE_PIN_BLOCK).await;
    let before = doc.engine.lock().unwrap().project().sheets[0]
        .entities
        .len();
    let e = call_err(
        &bridge,
        "connect_pins",
        json!({"sheet_id": sheet, "connections": [
            {"from": "U1.1", "to": "U2.1"},
            {"from": "U9.1", "to": "U2.1"}
        ]}),
    )
    .await;
    assert!(e.contains("connections[1]") && e.contains("U9"), "{e}");
    assert_eq!(
        doc.engine.lock().unwrap().project().sheets[0]
            .entities
            .len(),
        before
    );
    let both = call_err(
        &bridge,
        "connect_pins",
        json!({"sheet_id": sheet, "from": "U1.1", "to": "U2.1", "connections": [{"from": "U1.1", "to": "U2.1"}]}),
    )
    .await;
    assert!(both.contains("connections"), "{both}");
}

/// place_mate takes a list of pairs and places them all in one call that one undo removes.
/// place_mate は組のリストを受け取って1回で全部置き、1回の取り消しで全部消える。
#[tokio::test]
async fn place_mate_places_a_list_of_pairs_in_one_edit() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    let placed = call(
        &bridge,
        "place_mate",
        json!({"sheet_id": sheet, "mates": [
            {"x": 100.0, "y": 50.0, "reference": "J5", "value": "XH-3P"},
            {"x": 100.0, "y": 57.5},
            {"x": 100.0, "y": 65.0, "orientation": "vertical", "male_side": "second"}
        ]}),
    )
    .await;
    let pairs = placed["mates"].as_array().expect("mates");
    assert_eq!(pairs.len(), 3);
    assert_eq!(
        (
            pairs[1]["first"]["x"].as_f64(),
            pairs[1]["first"]["y"].as_f64()
        ),
        (Some(92.5), Some(57.5))
    );
    assert_eq!(
        doc.engine.lock().unwrap().project().sheets[0]
            .entities
            .len(),
        6
    );
    doc.undo().expect("元に戻せる");
    assert_eq!(
        doc.engine.lock().unwrap().project().sheets[0]
            .entities
            .len(),
        0
    );
}

/// In a place_mate list, mates repeating the same reference without poles are numbered 1, 2, 3 in list order, an explicit pole wins, a lone reference gets none, and both halves carry the pole attribute.
/// place_mate のリストで、極番号なしで同じ参照記号を繰り返す組は並び順に 1, 2, 3 と番号が付き、明示した極番号が優先され、1組だけの参照記号には付かず、プラグとソケットの両方が pole 属性を持つ。
#[tokio::test]
async fn place_mate_list_numbers_the_poles() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    call(
        &bridge,
        "place_mate",
        json!({"sheet_id": sheet, "mates": [
            {"x": 100.0, "y": 50.0, "reference": "X44A"},
            {"x": 100.0, "y": 57.5, "reference": "X44A"},
            {"x": 100.0, "y": 65.0, "reference": "X44A", "pole": "9"},
            {"x": 100.0, "y": 72.5, "reference": "X45"}
        ]}),
    )
    .await;
    let engine = doc.engine.lock().unwrap();
    let mut seen: Vec<(String, f64, String)> = engine.project().sheets[0]
        .entities
        .values()
        .filter_map(|e| match e {
            madake_core::Entity::Symbol(s) => Some((
                s.reference.clone(),
                s.at.y,
                s.attrs.get("pole").cloned().unwrap_or_default(),
            )),
            _ => None,
        })
        .collect();
    seen.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    let poles: Vec<&str> = seen.iter().map(|s| s.2.as_str()).collect();
    assert_eq!(poles, ["1", "1", "2", "2", "9", "9", "", ""], "{seen:?}");
}

/// place_mate numbering skips poles given explicitly for the same reference and treats a blank pole as none: [X, X pole "1", X] gets poles 2, 1, 3.
/// place_mate の番号振りは、同じ参照記号で明示された極番号を飛ばし、空白だけの極番号は無指定として数える。[X, X 極1, X] は 2, 1, 3 になる。
#[tokio::test]
async fn place_mate_numbering_skips_explicit_poles() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    call(
        &bridge,
        "place_mate",
        json!({"sheet_id": sheet, "mates": [
            {"x": 100.0, "y": 50.0, "reference": "X1"},
            {"x": 100.0, "y": 57.5, "reference": "X1", "pole": "1"},
            {"x": 100.0, "y": 65.0, "reference": "X1", "pole": "  "}
        ]}),
    )
    .await;
    let engine = doc.engine.lock().unwrap();
    let mut seen: Vec<(f64, String)> = engine.project().sheets[0]
        .entities
        .values()
        .filter_map(|e| match e {
            madake_core::Entity::Symbol(s) => Some((s.at.y, s.attrs.get("pole").cloned().unwrap_or_default())),
            _ => None,
        })
        .collect();
    seen.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let poles: Vec<&str> = seen.iter().map(|s| s.1.as_str()).collect();
    assert_eq!(poles, ["2", "2", "1", "1", "3", "3"], "{seen:?}");
}

/// connect_cable draws the LTE harness in one call: four board wires to a breakout, one line to the USB plug, cores mapped by pin order; the netlist has four nets, ERC finds no unconnected pin, and one undo removes it all.
/// connect_cable は LTE ハーネスを1回で描く: 基板側の配線4本と分岐、USBプラグまでの1本線、ピン順の芯の対応。ネットリストは4ネット、ERC は未接続ピンを出さず、undo 1回で全部消える。
#[tokio::test]
async fn connect_cable_draws_a_breakout_harness_in_one_edit() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    call(&bridge, "place_symbol", json!({"sheet_id": sheet, "symbol_id": "connector_4p", "x": 50.0, "y": 100.0, "reference": "J45"})).await;
    call(&bridge, "place_symbol", json!({"sheet_id": sheet, "symbol_id": "connector_4p", "x": 200.0, "y": 100.0, "rotation": 180, "reference": "USB1"})).await;
    let depth = doc.engine.lock().unwrap().undo_depth();
    let out = call(
        &bridge,
        "connect_cable",
        json!({"sheet_id": sheet, "reference": "W45",
               "cores": ["VBUS", "D-", "D+", "GND"],
               "a": {"part": "J45", "style": "multi"},
               "b": {"part": "USB1", "style": "single"}}),
    )
    .await;
    assert_eq!(out["reference"], "W45");
    let cores = out["cores"].as_array().expect("cores");
    assert_eq!(cores.len(), 4);
    assert_eq!(cores[1]["a"], "J45.2");
    assert_eq!(cores[1]["b"], "USB1.2");
    assert_eq!(cores[1]["wire_ids"].as_array().unwrap().len(), 1);
    assert_eq!(doc.engine.lock().unwrap().undo_depth(), depth + 1);
    let nets = call(&bridge, "get_netlist", json!({"sheet_id": sheet})).await;
    assert_eq!(nets.as_array().unwrap().len(), 4, "{nets}");
    let diags = call(&bridge, "run_verification", json!({"sheet_id": sheet})).await;
    assert_eq!(unconnected_pins(&diags), 0);
    doc.undo().expect("元に戻せる");
    assert_eq!(doc.engine.lock().unwrap().project().sheets[0].entities.len(), 2);
}

/// connect_cable with labels on the board side draws a labelled wire per core in one edit, answers the label names as the core ends, joins each core to its label net, refuses labels with single or a wrong count, and its description mentions labels.
/// 基板側にlabelsを指定した connect_cable は、芯ごとにラベル付きの配線を1回の編集で描き、芯の端としてラベル名を返し、芯ごとにそのラベルのネットへ繋ぎ、single との併用や数違いは断り、説明にlabelsを載せる。
#[tokio::test]
async fn connect_cable_labels_end_the_breakout_wires_at_net_labels() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    call(&bridge, "place_symbol", json!({"sheet_id": sheet, "symbol_id": "connector_4p", "x": 200.0, "y": 100.0, "rotation": 180, "reference": "USB1"})).await;
    let depth = doc.engine.lock().unwrap().undo_depth();
    let out = call(
        &bridge,
        "connect_cable",
        json!({"sheet_id": sheet, "reference": "W46",
               "cores": ["VBUS", "D-", "D+", "GND"],
               "a": {"labels": ["VBUS", "D-", "D+", "GND"], "style": "multi", "breakout_at": {"x": 80.0, "y": 100.0}},
               "b": {"part": "USB1", "style": "single"}}),
    )
    .await;
    let cores = out["cores"].as_array().expect("cores");
    assert_eq!(cores[2]["a"], "D+");
    assert_eq!(cores[2]["b"], "USB1.3");
    assert_eq!(cores[2]["wire_ids"].as_array().unwrap().len(), 1);
    assert_eq!(doc.engine.lock().unwrap().undo_depth(), depth + 1);
    let labels = doc.engine.lock().unwrap().project().sheets[0]
        .entities
        .values()
        .filter(|e| matches!(e, madake_core::Entity::NetLabel(_)))
        .count();
    assert_eq!(labels, 4);
    let nets = call(&bridge, "get_netlist", json!({"sheet_id": sheet})).await;
    assert_eq!(nets.as_array().unwrap().len(), 4, "{nets}");
    let single = call_err(
        &bridge,
        "connect_cable",
        json!({"sheet_id": sheet, "cores": ["A", "B", "C", "D"],
               "a": {"labels": ["A", "B", "C", "D"], "style": "single", "breakout_at": {"x": 80.0, "y": 150.0}},
               "b": {"part": "USB1", "style": "single"}}),
    )
    .await;
    assert!(single.contains("labels") && single.contains("single"), "{single}");
    let count = call_err(
        &bridge,
        "connect_cable",
        json!({"sheet_id": sheet, "cores": ["A", "B", "C", "D"],
               "a": {"labels": ["A"], "style": "multi", "breakout_at": {"x": 80.0, "y": 150.0}},
               "b": {"part": "USB1", "style": "single"}}),
    )
    .await;
    assert!(count.contains("ラベル"), "{count}");
    let tools = tool_descriptions();
    let (_, d) = tools.iter().find(|(n, _)| n == "connect_cable").expect("connect_cable");
    assert!(d.contains("labels"), "{d}");
    doc.undo().expect("元に戻せる");
    assert_eq!(doc.engine.lock().unwrap().project().sheets[0].entities.len(), 1);
}

/// If any cable in the list is wrong, connect_cable names its index and draws none of them; a correct list is drawn in one edit.
/// リストのケーブルのどれか1本でも誤っていれば、connect_cable はその番号を知らせて1本も描かない。正しいリストは1回の編集で描く。
#[tokio::test]
async fn connect_cable_list_is_all_or_nothing() {
    let (doc, bridge) = fixture();
    let sheet = first_sheet(&doc);
    call(&bridge, "place_symbol", json!({"sheet_id": sheet, "symbol_id": "connector_3p", "x": 50.0, "y": 50.0, "reference": "J1"})).await;
    call(&bridge, "place_symbol", json!({"sheet_id": sheet, "symbol_id": "connector_3p", "x": 150.0, "y": 50.0, "rotation": 180, "reference": "J2"})).await;
    call(&bridge, "place_symbol", json!({"sheet_id": sheet, "symbol_id": "connector_4p", "x": 50.0, "y": 120.0, "reference": "J45"})).await;
    let count = || doc.engine.lock().unwrap().project().sheets[0].entities.len();
    let before = count();
    let aux = json!({"cores": ["L", "R", "GND"], "a": {"part": "J1", "style": "single"}, "b": {"part": "J2", "style": "single"}});
    let err = call_err(
        &bridge,
        "connect_cable",
        json!({"sheet_id": sheet, "cables": [
            aux.clone(),
            {"cores": ["VBUS", "D-"], "a": {"part": "J45", "style": "multi"}, "b": {"part": "J2", "style": "single"}}
        ]}),
    )
    .await;
    assert!(err.contains("cables[1]") && err.contains("a 側のピンが 4 本で、芯は 2 本"), "{err}");
    assert_eq!(count(), before);
    let out = call(&bridge, "connect_cable", json!({"sheet_id": sheet, "cables": [aux]})).await;
    assert_eq!(out["cables"].as_array().unwrap().len(), 1);
    assert_eq!(out["cables"][0]["reference"], "W1");
    assert_eq!(count(), before + 1);
}

/// connect_cable's description tells the agent that cores follow pin order and to use it instead of a fake breakout block.
/// connect_cable の説明は、芯がピン順に対応することと、偽の分岐ブロックの代わりに使うことをエージェントに伝える。
#[test]
fn connect_cable_is_described_for_the_agent() {
    let tools = tool_descriptions();
    let (_, d) = tools.iter().find(|(n, _)| n == "connect_cable").expect("connect_cable");
    assert!(d.contains("ピン順") && d.contains("分岐"), "{d}");
}
