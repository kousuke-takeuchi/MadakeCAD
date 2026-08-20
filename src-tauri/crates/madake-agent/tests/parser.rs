//! 実物のclaude CLI出力(fixtures)を1行ずつ食わせてイベント列を検証する。

use madake_agent::{parse_stream_line, AgentEvent, StreamParser};

const TOOLUSE: &str = include_str!("fixtures/stream_tooluse.jsonl");
const TEXT: &str = include_str!("fixtures/stream_text.jsonl");

fn events(fixture: &str) -> Vec<AgentEvent> {
    fixture.lines().filter_map(parse_stream_line).collect()
}

fn joined_text(events: &[AgentEvent]) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::TextDelta { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn init_line_yields_session_started() {
    let evs = events(TOOLUSE);
    assert_eq!(
        evs.first(),
        Some(&AgentEvent::SessionStarted {
            session_id: "39628e1e-925e-42e5-9619-7cda7c2671f1".to_string()
        })
    );
}

#[test]
fn only_text_deltas_become_text_events() {
    // thinking_delta / signature_delta は無視され、text_deltaのみ連結される
    assert_eq!(joined_text(&events(TOOLUSE)), "done.");
    assert_eq!(joined_text(&events(TEXT)), "hello");
}

#[test]
fn assistant_tool_use_block_yields_tool_use_started() {
    let evs = events(TOOLUSE);
    let started: Vec<_> = evs
        .iter()
        .filter_map(|e| match e {
            AgentEvent::ToolUseStarted { id, tool, input } => Some((id, tool, input)),
            _ => None,
        })
        .collect();
    assert_eq!(started.len(), 1, "tool_useブロックは1個のはず");
    let (id, tool, input) = started[0];
    assert_eq!(id, "toolu_01KLusiJMaKub9DeG7CjencY");
    assert_eq!(tool, "Bash");
    assert_eq!(input["command"], "echo madake-test");
}

#[test]
fn tool_result_yields_tool_use_finished() {
    let evs = events(TOOLUSE);
    let finished: Vec<_> = evs
        .iter()
        .filter_map(|e| match e {
            AgentEvent::ToolUseFinished { id, is_error, .. } => Some((id, *is_error)),
            _ => None,
        })
        .collect();
    assert_eq!(finished.len(), 1);
    assert_eq!(finished[0].0, "toolu_01KLusiJMaKub9DeG7CjencY");
    assert!(!finished[0].1);
}

#[test]
fn result_line_yields_turn_completed_with_usage() {
    let evs = events(TOOLUSE);
    match evs.last() {
        Some(AgentEvent::TurnCompleted { result, usage }) => {
            assert_eq!(result, "done.");
            let usage = usage.as_ref().expect("usageが取れること");
            assert_eq!(usage.input_tokens, 18);
            assert_eq!(usage.output_tokens, 143);
            assert_eq!(usage.cache_creation_input_tokens, 8413);
            assert_eq!(usage.cache_read_input_tokens, 42963);
            assert!(usage.total_cost_usd.unwrap() > 0.0);
        }
        other => panic!("最後はTurnCompletedのはず: {other:?}"),
    }
}

#[test]
fn unknown_and_noise_lines_are_none() {
    for line in [
        r#"{"type":"system","subtype":"status","status":"requesting"}"#,
        r#"{"type":"system","subtype":"thinking_tokens","estimated_tokens":5}"#,
        r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed"}}"#,
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"hm"}}}"#,
        r#"{"type":"stream_event","event":{"type":"message_stop"}}"#,
        r#"{"type":"future_unknown_type"}"#,
        "not json at all",
        "",
        "   ",
    ] {
        assert_eq!(parse_stream_line(line), None, "line={line}");
    }
}

#[test]
fn error_result_yields_error_event() {
    let line = r#"{"type":"result","subtype":"error_during_execution","is_error":true,"result":"boom","session_id":"s"}"#;
    assert_eq!(
        parse_stream_line(line),
        Some(AgentEvent::Error {
            message: "boom".to_string()
        })
    );
}

#[test]
fn full_event_sequence_of_tooluse_fixture() {
    let evs = events(TOOLUSE);
    let kinds: Vec<&str> = evs
        .iter()
        .map(|e| match e {
            AgentEvent::SessionStarted { .. } => "session",
            AgentEvent::TextDelta { .. } => "text",
            AgentEvent::ToolUseStarted { .. } => "tool_start",
            AgentEvent::ToolUseFinished { .. } => "tool_end",
            AgentEvent::TurnCompleted { .. } => "completed",
            AgentEvent::TurnApplied { .. } => "applied",
            AgentEvent::Error { .. } => "error",
        })
        .collect();
    assert_eq!(
        kinds,
        vec!["session", "tool_start", "tool_end", "text", "completed"]
    );
}

#[test]
fn stream_parser_fills_tool_name_on_finish() {
    let mut parser = StreamParser::new();
    let evs: Vec<AgentEvent> = TOOLUSE.lines().flat_map(|l| parser.push(l)).collect();
    let finished = evs
        .iter()
        .find_map(|e| match e {
            AgentEvent::ToolUseFinished { tool, .. } => Some(tool.clone()),
            _ => None,
        })
        .expect("ToolUseFinishedがあること");
    // 単体のparse_stream_lineではtool_resultにツール名が無いが、
    // StreamParserはtool_use_idから補完する
    assert_eq!(finished, "Bash");
    assert_eq!(evs.len(), 5);
}

#[test]
fn stream_parser_drops_duplicate_tool_use_started() {
    // CLIが同じtool_useブロックを複数行(assistant再送等)で出しても、
    // 既知のtool_use_idならToolUseStartedは1回だけ発火する
    let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_dup","name":"Bash","input":{"command":"ls"}}]}}"#;
    let mut parser = StreamParser::new();
    assert_eq!(parser.push(line).len(), 1);
    assert!(parser.push(line).is_empty(), "2回目は重複として捨てる");

    // 補完は引き続き効くこと
    let result = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_dup","is_error":false}]}}"#;
    assert_eq!(
        parser.push(result),
        vec![AgentEvent::ToolUseFinished {
            id: "toolu_dup".to_string(),
            tool: "Bash".to_string(),
            is_error: false,
        }]
    );
}
