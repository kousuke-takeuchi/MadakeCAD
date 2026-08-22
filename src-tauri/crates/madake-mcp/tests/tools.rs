//! 公開しているMCPツールの説明文(エージェントがツールを選ぶ手がかり)のテスト。

use madake_mcp::tool_descriptions;

fn description_of(name: &str) -> String {
    tool_descriptions()
        .into_iter()
        .find(|(tool, _)| tool == name)
        .unwrap_or_else(|| panic!("MCPツール「{name}」が公開されていない"))
        .1
}

/// The parts search tool advertises selection, comparison and alternative-part use, so the agent reaches for it when asked "what can replace this?".
/// 部品検索ツールの説明には選定・比較・代替品の用途が書かれており、「これの代替は?」と聞かれたエージェントがこれを使う。
#[test]
fn the_parts_search_tool_advertises_selection_and_comparison() {
    let description = description_of("search_parts");
    for needle in ["選定", "比較", "代替", "価格", "定格"] {
        assert!(
            description.contains(needle),
            "search_partsの説明に「{needle}」が無い: {description}"
        );
    }
}

/// Every published tool carries a description, so no tool is offered to the agent unexplained.
/// 公開ツールには全て説明文が付いており、説明の無いツールをエージェントへ見せない。
#[test]
fn every_published_tool_has_a_description() {
    let tools = tool_descriptions();
    assert!(tools.len() > 10, "ツールが少なすぎる: {tools:?}");
    for (name, description) in tools {
        assert!(!description.trim().is_empty(), "{name}の説明が空");
    }
}
