//! MCPツール定義 → Anthropic Messages APIの`tools`配列へのブリッジのテスト。
//!
//! エージェントがAPI直結で図面を編集できるのは、内蔵MCPサーバーのツールが
//! そのままAPIのツールとして渡るからなので、変換の取りこぼしをここで固定する。

use madake_agent::tools::{api_tool_definitions, ToolDef};
use serde_json::json;

fn def(name: &str, description: &str, schema: serde_json::Value) -> ToolDef {
    ToolDef {
        name: name.to_string(),
        description: description.to_string(),
        input_schema: schema,
    }
}

/// Each MCP tool becomes one API tool keeping its name, description and input schema.
/// MCPツール1つがAPIのツール1つになり、名前・説明・入力スキーマがそのまま引き継がれる。
#[test]
fn every_mcp_tool_becomes_one_api_tool() {
    let defs = vec![
        def(
            "place_symbol",
            "シンボルをシートに配置する",
            json!({"type": "object", "properties": {"symbol_id": {"type": "string"}}, "required": ["symbol_id"]}),
        ),
        def("undo", "直前の編集を取り消す", json!({"type": "object"})),
    ];
    let tools = api_tool_definitions(&defs);
    assert_eq!(tools.len(), 2, "ツール数が一致しない: {tools:?}");
    assert_eq!(tools[0]["name"], "place_symbol");
    assert_eq!(tools[0]["description"], "シンボルをシートに配置する");
    assert_eq!(tools[0]["input_schema"]["properties"]["symbol_id"]["type"], "string");
    assert_eq!(tools[0]["input_schema"]["required"][0], "symbol_id");
    assert_eq!(tools[1]["name"], "undo");
}

/// The JSON Schema `$schema` marker is dropped because the API only wants the shape itself.
/// JSON Schemaの`$schema`宣言は落とす(APIが必要とするのは形そのものだけ)。
#[test]
fn the_schema_marker_is_dropped_from_the_input_schema() {
    let defs = vec![def(
        "get_project",
        "プロジェクト全体を返す",
        json!({"$schema": "https://json-schema.org/draft/2020-12/schema", "type": "object", "properties": {}}),
    )];
    let tools = api_tool_definitions(&defs);
    assert!(
        tools[0]["input_schema"].get("$schema").is_none(),
        "$schemaが残っている: {}",
        tools[0]["input_schema"]
    );
    assert_eq!(tools[0]["input_schema"]["type"], "object");
}

/// A tool without a usable schema still gets an empty object schema, so the API accepts it.
/// スキーマの無いツールにも空のオブジェクトスキーマを与える(APIが受け付ける形にする)。
#[test]
fn a_tool_without_a_schema_gets_an_empty_object_schema() {
    let defs = vec![
        def("no_args", "引数の無いツール", json!(null)),
        def("wrong_type", "スキーマが配列になっている", json!([1, 2, 3])),
    ];
    let tools = api_tool_definitions(&defs);
    for tool in &tools {
        assert_eq!(tool["input_schema"]["type"], "object", "{tool}");
        assert!(
            tool["input_schema"]["properties"].is_object(),
            "propertiesがオブジェクトでない: {tool}"
        );
    }
}

/// A schema that forgot `"type": "object"` is repaired instead of being sent as-is.
/// `"type": "object"`が抜けたスキーマは、そのまま送らずに補って直す。
#[test]
fn a_schema_missing_its_object_type_is_repaired() {
    let defs = vec![def(
        "half_baked",
        "typeの無いスキーマ",
        json!({"properties": {"n": {"type": "integer"}}}),
    )];
    let tools = api_tool_definitions(&defs);
    assert_eq!(tools[0]["input_schema"]["type"], "object");
    assert_eq!(tools[0]["input_schema"]["properties"]["n"]["type"], "integer");
}

/// Every bridged tool name fits the API's allowed name pattern, so no tool is rejected.
/// ブリッジしたツール名は全てAPIが許す文字種・長さに収まる(ツールが弾かれない)。
#[test]
fn bridged_tool_names_fit_the_api_name_rules() {
    let defs = vec![
        def("run_verification", "検証", json!({"type": "object"})),
        def("mcp__madakecad__place_symbol", "配置", json!({"type": "object"})),
    ];
    let tools = api_tool_definitions(&defs);
    for tool in &tools {
        let name = tool["name"].as_str().expect("nameは文字列");
        assert!(!name.is_empty() && name.len() <= 128, "名前の長さが範囲外: {name}");
        assert!(
            name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
            "APIが許さない文字を含む名前: {name}"
        );
    }
}

/// A tool with an empty description keeps an explanatory placeholder rather than nothing.
/// 説明が空のツールには説明の代わりを入れる(説明無しのツールをAPIへ渡さない)。
#[test]
fn a_tool_without_a_description_still_carries_some_text() {
    let defs = vec![def("mystery", "   ", json!({"type": "object"}))];
    let tools = api_tool_definitions(&defs);
    let description = tools[0]["description"].as_str().unwrap_or_default();
    assert!(!description.trim().is_empty(), "説明が空のまま渡っている");
    assert!(description.contains("mystery"), "説明にツール名が入っていない: {description}");
}
