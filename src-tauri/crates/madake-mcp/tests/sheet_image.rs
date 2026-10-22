//! シートを画像で返すMCPツール (get_sheet_image) のテスト。

use madake_agent::tools::ToolBridge;
use madake_core::{Engine, Project};
use madake_mcp::tool_bridge::McpToolBridge;
use madake_mcp::{tool_descriptions, SharedDoc};
use serde_json::json;
use uuid::Uuid;

fn fixture() -> (SharedDoc, McpToolBridge) {
    let doc = SharedDoc::new(Engine::new(Project::new("画像試験")));
    let parts = madake_mcp::open_parts(
        &std::env::temp_dir().join(format!("madake-image-{}.sqlite", Uuid::new_v4())),
    )
    .expect("部品DB");
    let bridge = McpToolBridge::new(doc.clone(), parts);
    (doc, bridge)
}

/// The sheet image is a PNG of the whole sheet at 4 px/mm by default, and a region in millimetres zooms into part of it.
/// シート画像は既定で 4 px/mm のシート全体のPNGで、mm で範囲を指定すると一部を拡大できる。
#[test]
fn the_sheet_image_is_a_png_of_the_sheet_or_a_region() {
    let (doc, _bridge) = fixture();
    let whole = madake_mcp::sheet_image(&doc, None, None, None).unwrap();
    assert_eq!(&whole.png[..4], b"\x89PNG");
    assert_eq!((whole.width, whole.height), (1680, 1188));
    let region = madake_mcp::ImageRegion {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 50.0,
        max_y: 25.0,
    };
    let part = madake_mcp::sheet_image(&doc, None, Some(10.0), Some(region)).unwrap();
    assert_eq!((part.width, part.height), (500, 250));
}

/// Backends that cannot show tool images (API and Copilot routes through the tool bridge) get a short note instead of a huge base64 string.
/// ツール結果の画像を表示できない経路 (ツール窓口を通るAPI系) には、巨大なbase64の代わりに短い説明を返す。
#[tokio::test]
async fn the_bridge_replaces_the_image_with_a_short_note() {
    let (_doc, bridge) = fixture();
    let outcome = bridge.call("get_sheet_image", json!({})).await;
    assert!(!outcome.is_error, "{}", outcome.content);
    assert!(outcome.content.contains("image/png"), "{}", outcome.content);
    assert!(
        outcome.content.len() < 2000,
        "{} bytes",
        outcome.content.len()
    );
}

/// The tool tells the agent to look at the drawing after editing it.
/// ツールの説明には、編集した後に図面を目で確かめるよう書かれている。
#[test]
fn the_image_tool_tells_the_agent_to_look_after_editing() {
    let description = tool_descriptions()
        .into_iter()
        .find(|(t, _)| t == "get_sheet_image")
        .expect("get_sheet_image が無い")
        .1;
    for needle in ["PNG", "region", "目で"] {
        assert!(description.contains(needle), "{needle}: {description}");
    }
}
