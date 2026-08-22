//! 内蔵MCPツールの窓口([`McpToolBridge`])のテスト。
//!
//! Anthropic API直結バックエンドはここを通して図面を編集するので、
//! 「CLI経由と同じツールが同じだけ見えること」「編集が本当にCommandエンジンを
//! 通って(=undoできる形で)入ること」を確かめる。

use madake_agent::tools::ToolBridge;
use madake_core::{Engine, Project};
use madake_mcp::tool_bridge::McpToolBridge;
use madake_mcp::{tool_descriptions, SharedDoc, SharedParts};
use serde_json::{json, Value};
use uuid::Uuid;

fn fixture() -> (SharedDoc, SharedParts, McpToolBridge) {
    let doc = SharedDoc::new(Engine::new(Project::new("ブリッジ試験")));
    let parts = madake_mcp::open_parts(
        &std::env::temp_dir().join(format!("madake-bridge-{}.sqlite", Uuid::new_v4())),
    )
    .expect("部品DB");
    let bridge = McpToolBridge::new(doc.clone(), parts.clone());
    (doc, parts, bridge)
}

fn first_sheet(doc: &SharedDoc) -> Uuid {
    doc.engine.lock().unwrap().project().sheets[0].id
}

/// The API route sees exactly the same tools as the Claude Code CLI route, so neither is missing a feature.
/// API経由で見えるツールはCLI経由とまったく同じ顔ぶれで、片方だけ機能が欠けることがない。
#[test]
fn the_api_route_sees_the_same_tools_as_the_cli_route() {
    let (_doc, _parts, bridge) = fixture();
    let mut bridged: Vec<String> = bridge.tools().into_iter().map(|t| t.name).collect();
    let mut published: Vec<String> = tool_descriptions().into_iter().map(|(name, _)| name).collect();
    bridged.sort();
    published.sort();
    assert_eq!(bridged, published, "公開ツールと窓口のツールが食い違っている");
    assert!(bridged.len() > 10, "ツールが少なすぎる: {bridged:?}");
}

/// Every bridged tool carries a description and an object-shaped input schema.
/// 窓口から見えるツールには全て説明が付き、入力スキーマはオブジェクト型になっている。
#[test]
fn every_bridged_tool_has_a_description_and_an_object_schema() {
    let (_doc, _parts, bridge) = fixture();
    for tool in bridge.tools() {
        assert!(!tool.description.trim().is_empty(), "{}の説明が空", tool.name);
        assert_eq!(
            tool.input_schema.get("type").and_then(Value::as_str),
            Some("object"),
            "{}の入力スキーマがオブジェクト型でない",
            tool.name
        );
    }
}

/// An edit made through the bridge lands in the document and can be undone like any other edit.
/// 窓口経由の編集は図面に入り、他の編集と同じように元に戻せる(Commandエンジンを通っている)。
#[tokio::test]
async fn an_edit_through_the_bridge_lands_in_the_document_and_can_be_undone() {
    let (doc, _parts, bridge) = fixture();
    let sheet_id = first_sheet(&doc);
    let before = doc.engine.lock().unwrap().undo_depth();

    let outcome = bridge
        .call(
            "place_symbol",
            json!({"sheet_id": sheet_id, "symbol_id": "relay_coil", "x": 50.0, "y": 50.0}),
        )
        .await;

    assert!(!outcome.is_error, "ツールが失敗した: {}", outcome.content);
    {
        let engine = doc.engine.lock().unwrap();
        assert_eq!(
            engine.undo_depth(),
            before + 1,
            "undo履歴に積まれていない(Commandエンジンを通っていない)"
        );
        assert_eq!(engine.project().sheets[0].entities.len(), 1);
    }

    doc.undo().expect("元に戻せる");
    assert_eq!(
        doc.engine.lock().unwrap().project().sheets[0].entities.len(),
        0,
        "元に戻せていない"
    );
}

/// Reading tools work through the bridge too, so the agent can look at the drawing before editing.
/// 読み取り系のツールも窓口経由で動く(エージェントは編集の前に図面を見られる)。
#[tokio::test]
async fn reading_tools_work_through_the_bridge() {
    let (_doc, _parts, bridge) = fixture();
    let outcome = bridge.call("get_project", Value::Null).await;
    assert!(!outcome.is_error, "読み取りに失敗した: {}", outcome.content);
    let value: Value = serde_json::from_str(&outcome.content).expect("JSONで返る");
    assert_eq!(value["project"]["name"], "ブリッジ試験");
}

/// A tool name that does not exist comes back as an error result instead of killing the turn.
/// 存在しないツール名は、ターンを落とさずにエラーの結果として返る(モデルが言い直せる)。
#[tokio::test]
async fn an_unknown_tool_name_comes_back_as_an_error_result() {
    let (_doc, _parts, bridge) = fixture();
    let outcome = bridge.call("no_such_tool", json!({})).await;
    assert!(outcome.is_error, "エラーになっていない: {}", outcome.content);
    assert!(
        outcome.content.contains("no_such_tool"),
        "どのツールが駄目だったか分からない: {}",
        outcome.content
    );
}

/// Bad arguments come back as an error result carrying the reason, so the model can correct itself.
/// 引数が間違っているときも理由つきのエラーの結果として返り、モデルが自分で直せる。
#[tokio::test]
async fn bad_arguments_come_back_with_a_reason() {
    let (_doc, _parts, bridge) = fixture();
    let outcome = bridge
        .call("place_symbol", json!({"symbol_id": 12345}))
        .await;
    assert!(outcome.is_error, "エラーになっていない: {}", outcome.content);
    assert!(
        !outcome.content.trim().is_empty(),
        "理由が空: {}",
        outcome.content
    );
}
