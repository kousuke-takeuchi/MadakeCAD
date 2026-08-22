//! GitHub Copilot CLIのJSONL出力をアプリ内イベントへ変換するパーサのテスト。
//!
//! Copilot CLI 1.0.80の`--output-format json`は「1行1JSON」だが、行の形は
//! 公開仕様が無くバージョン差も見込まれる。そのためパーサは**寛容**に作ってあり、
//! ここではその約束(別名の許容・未知の行を捨てる)を仕様として固定する。

use madake_agent::copilot_events::{parse_copilot_events, CopilotParser};
use madake_agent::AgentEvent;

fn one(line: &str) -> AgentEvent {
    let events = parse_copilot_events(line);
    assert_eq!(events.len(), 1, "1イベントになるはず: {line} -> {events:?}");
    events.into_iter().next().unwrap()
}

/// A Copilot session id line becomes the session event used to continue the same conversation.
/// Copilotのセッション行は、同じ会話を続けるためのセッションイベントになる。
#[test]
fn session_line_becomes_a_session_event() {
    for line in [
        r#"{"type":"session","session_id":"S1"}"#,
        r#"{"type":"session_started","sessionId":"S1"}"#,
        r#"{"type":"init","session":{"id":"S1"}}"#,
    ] {
        assert_eq!(
            one(line),
            AgentEvent::SessionStarted {
                session_id: "S1".to_string()
            },
            "{line}"
        );
    }
}

/// Assistant text is delivered to the chat whichever field name the CLI uses for it.
/// アシスタントの本文は、CLIがどのフィールド名で出してもチャットへ届く。
#[test]
fn assistant_text_is_delivered_under_any_of_the_known_field_names() {
    for line in [
        r#"{"type":"assistant","text":"こんにちは"}"#,
        r#"{"type":"text","content":"こんにちは"}"#,
        r#"{"type":"assistant_message","message":{"content":"こんにちは"}}"#,
        r#"{"type":"message","content":[{"type":"text","text":"こんにちは"}]}"#,
        r#"{"type":"text_delta","delta":"こんにちは"}"#,
    ] {
        assert_eq!(
            one(line),
            AgentEvent::TextDelta {
                text: "こんにちは".to_string()
            },
            "{line}"
        );
    }
}

/// A tool call line becomes a tool-start event with its name and arguments.
/// ツール呼び出しの行は、ツール名と引数を持つ「ツール開始」イベントになる。
#[test]
fn a_tool_call_line_becomes_a_tool_start_event() {
    let event = one(
        r#"{"type":"tool_call","id":"call_1","name":"madakecad-add_symbol","arguments":{"symbol":"TB"}}"#,
    );
    match event {
        AgentEvent::ToolUseStarted { id, tool, input } => {
            assert_eq!(id, "call_1");
            assert_eq!(tool, "madakecad-add_symbol");
            assert_eq!(input["symbol"], "TB");
        }
        other => panic!("tool_startのはず: {other:?}"),
    }
}

/// Tool calls are recognised under the alternative field names too (tool/input, function/parameters).
/// ツール呼び出しは別のフィールド名(tool/input、function/parameters)でも認識される。
#[test]
fn tool_calls_are_recognised_under_alternative_field_names() {
    for line in [
        r#"{"type":"tool_use","tool_call_id":"call_1","tool":"madakecad-add_symbol","input":{"symbol":"TB"}}"#,
        r#"{"type":"tool.start","call_id":"call_1","tool_name":"madakecad-add_symbol","parameters":{"symbol":"TB"}}"#,
        r#"{"type":"function_call","id":"call_1","function":"madakecad-add_symbol","args":{"symbol":"TB"}}"#,
    ] {
        match one(line) {
            AgentEvent::ToolUseStarted { id, tool, input } => {
                assert_eq!(id, "call_1", "{line}");
                assert_eq!(tool, "madakecad-add_symbol", "{line}");
                assert_eq!(input["symbol"], "TB", "{line}");
            }
            other => panic!("tool_startのはず ({line}): {other:?}"),
        }
    }
}

/// A tool result line closes the matching tool call and carries whether it failed.
/// ツール結果の行は対応するツール呼び出しを閉じ、失敗したかどうかを伝える。
#[test]
fn a_tool_result_line_closes_the_call_and_reports_failure() {
    for (line, expected_error) in [
        (r#"{"type":"tool_result","tool_call_id":"call_1"}"#, false),
        (
            r#"{"type":"tool_result","tool_call_id":"call_1","status":"error"}"#,
            true,
        ),
        (
            r#"{"type":"tool_call_result","id":"call_1","is_error":true}"#,
            true,
        ),
        (
            r#"{"type":"tool.end","call_id":"call_1","error":"boom"}"#,
            true,
        ),
    ] {
        match one(line) {
            AgentEvent::ToolUseFinished { id, is_error, .. } => {
                assert_eq!(id, "call_1", "{line}");
                assert_eq!(is_error, expected_error, "{line}");
            }
            other => panic!("tool_endのはず ({line}): {other:?}"),
        }
    }
}

/// The tool name from the call is filled into the matching result, so the chat shows what finished.
/// ツール名は呼び出しから結果へ補完され、チャットには何が終わったのかが表示される。
#[test]
fn the_tool_name_is_carried_from_the_call_to_its_result() {
    let mut parser = CopilotParser::new();
    parser
        .push(r#"{"type":"tool_call","id":"call_1","name":"madakecad-add_symbol","arguments":{}}"#);
    let events = parser.push(r#"{"type":"tool_result","tool_call_id":"call_1"}"#);

    match events.first() {
        Some(AgentEvent::ToolUseFinished { tool, .. }) => assert_eq!(tool, "madakecad-add_symbol"),
        other => panic!("tool_endのはず: {other:?}"),
    }
}

/// The final line completes the turn with the answer and the token usage.
/// 最後の行は、回答とトークン使用量を伴ってターンを完了させる。
#[test]
fn the_final_line_completes_the_turn_with_usage() {
    let event = one(
        r#"{"type":"result","result":"配置しました。","usage":{"input_tokens":1200,"output_tokens":80},"total_cost_usd":0.5}"#,
    );
    match event {
        AgentEvent::TurnCompleted { result, usage } => {
            assert_eq!(result, "配置しました。");
            let usage = usage.expect("usageが読めること");
            assert_eq!(usage.input_tokens, 1200);
            assert_eq!(usage.output_tokens, 80);
            assert_eq!(usage.total_cost_usd, Some(0.5));
        }
        other => panic!("completedのはず: {other:?}"),
    }
}

/// Token usage is also read from OpenAI-style field names (prompt/completion tokens).
/// トークン使用量はOpenAI流のフィールド名(prompt/completion tokens)でも読み取れる。
#[test]
fn token_usage_is_also_read_from_openai_style_names() {
    let event = one(
        r#"{"type":"done","response":"ok","usage":{"prompt_tokens":10,"completion_tokens":2}}"#,
    );
    match event {
        AgentEvent::TurnCompleted { result, usage } => {
            assert_eq!(result, "ok");
            let usage = usage.expect("usageが読めること");
            assert_eq!(usage.input_tokens, 10);
            assert_eq!(usage.output_tokens, 2);
        }
        other => panic!("completedのはず: {other:?}"),
    }
}

/// An error line from the CLI is shown in the chat as an error.
/// CLIのエラー行は、チャットにエラーとして表示される。
#[test]
fn an_error_line_becomes_an_error_event() {
    for line in [
        r#"{"type":"error","message":"rate limited"}"#,
        r#"{"type":"error","error":"rate limited"}"#,
    ] {
        assert_eq!(
            one(line),
            AgentEvent::Error {
                message: "rate limited".to_string()
            },
            "{line}"
        );
    }
}

/// A result line flagged as an error becomes an error instead of a normal completion.
/// エラー扱いのresult行は、通常の完了ではなくエラーになる。
#[test]
fn a_failed_result_line_becomes_an_error() {
    match one(r#"{"type":"result","is_error":true,"result":"credit exhausted"}"#) {
        AgentEvent::Error { message } => assert_eq!(message, "credit exhausted"),
        other => panic!("errorのはず: {other:?}"),
    }
}

/// Lines the parser does not understand are ignored instead of breaking the turn.
/// 解釈できない行はターンを壊さず黙って無視される。
#[test]
fn unknown_and_broken_lines_are_ignored() {
    for line in [
        "",
        "   ",
        "これはJSONではありません",
        r#"{"type":"progress","note":"未知のイベント"}"#,
        r#"{"no_type_field":1}"#,
        r#"{"type":"assistant","text":""}"#,
        "[1,2,3]",
    ] {
        assert!(
            parse_copilot_events(line).is_empty(),
            "無視されるはず: {line}"
        );
    }
}

/// The same tool call reported twice is only shown once in the chat.
/// 同じツール呼び出しが二度流れても、チャットには一度だけ表示される。
#[test]
fn a_repeated_tool_call_is_shown_only_once() {
    let mut parser = CopilotParser::new();
    let line = r#"{"type":"tool_call","id":"call_1","name":"t","arguments":{}}"#;
    assert_eq!(parser.push(line).len(), 1);
    assert!(parser.push(line).is_empty(), "2回目は捨てられる");
}
