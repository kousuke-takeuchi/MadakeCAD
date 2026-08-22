//! GitHub Copilot CLIのJSONL出力をアプリ内イベントへ変換するパーサのテスト。
//!
//! 行の形は**実機のcopilot 1.0.80**(`--output-format json`)から採取したものを
//! そのまま使っている。実機は`{"type":…,"data":{…}}`の封筒に入った行を流し、
//! 最終行の`result`だけは封筒が無い。
//!
//! 公開仕様が無くバージョン差も見込まれるため、パーサは別名にも**寛容**に作ってある。
//! ここではその約束(実形の解釈・別名の許容・未知の行を捨てる)を仕様として固定する。

use madake_agent::copilot_events::{parse_copilot_events, CopilotParser};
use madake_agent::AgentEvent;

fn one(line: &str) -> AgentEvent {
    let events = parse_copilot_events(line);
    assert_eq!(events.len(), 1, "1イベントになるはず: {line} -> {events:?}");
    events.into_iter().next().unwrap()
}

fn texts(events: &[AgentEvent]) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::TextDelta { text } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 実機の行(2026-08-22採取)
// ---------------------------------------------------------------------------

/// The assistant's answer arrives as message deltas and is shown in the chat as it is typed.
/// アシスタントの回答は差分(message_delta)で届き、書かれるそばからチャットに表示される。
#[test]
fn assistant_message_deltas_are_shown_as_they_arrive() {
    let event = one(
        r#"{"type":"assistant.message_delta","data":{"messageId":"m1","deltaContent":"端子台を"},"ephemeral":true,"id":"e1","timestamp":"2026-08-22T18:00:04.441Z","parentId":"p1"}"#,
    );

    assert_eq!(
        event,
        AgentEvent::TextDelta {
            text: "端子台を".to_string()
        }
    );
}

/// The model's private reasoning is not shown in the chat, even though it streams like the answer.
/// モデルの思考(reasoning)は回答と同じ形で流れてくるが、チャットには表示しない。
#[test]
fn reasoning_deltas_are_not_shown_in_the_chat() {
    assert!(parse_copilot_events(
        r#"{"type":"assistant.reasoning_delta","data":{"reasoningId":"r1","deltaContent":"どこに置くか考える"},"ephemeral":true,"id":"e1"}"#
    )
    .is_empty());
    assert!(parse_copilot_events(
        r#"{"type":"assistant.reasoning","data":{"reasoningId":"r1","content":"どこに置くか考える"},"ephemeral":true,"id":"e1"}"#
    )
    .is_empty());
}

/// A completed assistant message carries the tool calls the model asked for, with their arguments.
/// 完成したアシスタントメッセージには、モデルが頼んだツール呼び出しと引数が入っている。
#[test]
fn a_completed_message_carries_the_requested_tool_calls() {
    let event = one(
        r#"{"type":"assistant.message","data":{"messageId":"m1","model":"claude-haiku-4.5","content":"","toolRequests":[{"toolCallId":"toolu_01","name":"madakecad-place_symbol","arguments":{"symbol_id":"TB","x":100},"type":"function","mcpServerName":"madakecad","mcpToolName":"place_symbol"}],"turnId":"0","outputTokens":166},"id":"e1"}"#,
    );

    match event {
        AgentEvent::ToolUseStarted { id, tool, input } => {
            assert_eq!(id, "toolu_01");
            assert_eq!(tool, "madakecad-place_symbol", "MCPツールはサーバー名付き");
            assert_eq!(input["symbol_id"], "TB");
            assert_eq!(input["x"], 100);
        }
        other => panic!("tool_startのはず: {other:?}"),
    }
}

/// Starting a tool reports its name and arguments, so the chat can show what the agent is doing.
/// ツールの実行開始はツール名と引数を伝えるので、チャットには今なにをしているかが出る。
#[test]
fn a_tool_execution_start_reports_the_tool_and_its_arguments() {
    let event = one(
        r#"{"type":"tool.execution_start","data":{"toolCallId":"toolu_01","toolName":"madakecad-place_symbol","arguments":{"symbol_id":"TB"},"turnId":"0","model":"claude-haiku-4.5","mcpServerName":"madakecad","mcpToolName":"place_symbol"},"id":"e1"}"#,
    );

    match event {
        AgentEvent::ToolUseStarted { id, tool, input } => {
            assert_eq!(id, "toolu_01");
            assert_eq!(tool, "madakecad-place_symbol");
            assert_eq!(input["symbol_id"], "TB");
        }
        other => panic!("tool_startのはず: {other:?}"),
    }
}

/// Finishing a tool closes the matching call and reports whether it succeeded.
/// ツールの実行完了は対応する呼び出しを閉じ、成功したかどうかを伝える。
#[test]
fn a_tool_execution_complete_closes_the_call_with_its_outcome() {
    for (line, expected_error) in [
        (
            r#"{"type":"tool.execution_complete","data":{"toolCallId":"toolu_01","success":true,"result":{"content":"{}"}},"id":"e1"}"#,
            false,
        ),
        (
            r#"{"type":"tool.execution_complete","data":{"toolCallId":"toolu_01","success":false,"result":{"content":"不明なシンボルです"}},"id":"e1"}"#,
            true,
        ),
    ] {
        match one(line) {
            AgentEvent::ToolUseFinished { id, is_error, .. } => {
                assert_eq!(id, "toolu_01", "{line}");
                assert_eq!(is_error, expected_error, "{line}");
            }
            other => panic!("tool_endのはず ({line}): {other:?}"),
        }
    }
}

/// The final line ends the turn and reports how many Copilot premium requests it used.
/// 最終行はターンを終わらせ、そのターンが使ったCopilotのプレミアムリクエスト数を伝える。
#[test]
fn the_final_line_completes_the_turn_with_the_premium_requests_used() {
    let events = parse_copilot_events(
        r#"{"type":"result","timestamp":"2026-08-22T18:00:04.896Z","sessionId":"cf2a4fc4-a6bb-491e-9229-dd7fbf53855d","exitCode":0,"usage":{"premiumRequests":0.33,"totalApiDurationMs":5501,"sessionDurationMs":8905,"codeChanges":{"linesAdded":0,"linesRemoved":0,"filesModified":[]}}}"#,
    );

    // 最終行はセッションIDの確定も兼ねる(次のターンの`--resume`はこのID)
    assert_eq!(
        events[0],
        AgentEvent::SessionStarted {
            session_id: "cf2a4fc4-a6bb-491e-9229-dd7fbf53855d".to_string()
        }
    );
    match &events[1] {
        AgentEvent::TurnCompleted { usage, .. } => {
            let usage = usage.as_ref().expect("usageが読めること");
            assert_eq!(usage.premium_requests, Some(0.33));
            // Copilotの最終行はトークン数を持たない(プレミアムリクエストが課金の単位)
            assert_eq!(usage.input_tokens, 0);
            assert_eq!(usage.total_cost_usd, None);
        }
        other => panic!("completedのはず: {other:?}"),
    }
    assert_eq!(events.len(), 2, "{events:?}");
}

/// A non-zero exit code in the final line is reported to the chat as an error, not as a normal answer.
/// 最終行の終了コードが0でなければ、通常の回答ではなくエラーとしてチャットに出る。
#[test]
fn a_failed_final_line_becomes_an_error() {
    let events =
        parse_copilot_events(r#"{"type":"result","sessionId":"S1","exitCode":1,"usage":{}}"#);

    match events.last() {
        Some(AgentEvent::Error { message }) => assert!(message.contains("exit 1"), "{message}"),
        other => panic!("errorのはず: {other:?}"),
    }
}

/// The session id the CLI actually used is reported, so the next turn resumes the same conversation.
/// CLIが実際に使ったセッションIDが伝わるので、次のターンは同じ会話を再開できる。
#[test]
fn the_session_line_reports_the_id_to_resume() {
    let event = one(
        r#"{"type":"session.start","data":{"sessionId":"S1","version":1,"producer":"copilot-agent","copilotVersion":"1.0.80"},"id":"e1"}"#,
    );

    assert_eq!(
        event,
        AgentEvent::SessionStarted {
            session_id: "S1".to_string()
        }
    );
}

/// Setup and bookkeeping lines (MCP status, skills, tokens spent so far, turn boundaries) do not appear in the chat.
/// 準備や記録のための行(MCPの状態・スキル一覧・ここまでの消費・ターンの区切り)はチャットに出ない。
#[test]
fn setup_and_bookkeeping_lines_are_ignored() {
    for line in [
        r#"{"type":"session.mcp_servers_loaded","data":{"servers":[{"name":"madakecad","status":"connected","transport":"http"}]},"ephemeral":true,"id":"e1"}"#,
        r#"{"type":"session.mcp_server_status_changed","data":{"serverName":"madakecad","status":"connected"},"ephemeral":true,"id":"e1"}"#,
        r#"{"type":"session.skills_loaded","data":{"skills":[]},"ephemeral":true,"id":"e1"}"#,
        r#"{"type":"session.tools_updated","data":{"model":"claude-haiku-4.5"},"ephemeral":true,"id":"e1"}"#,
        r#"{"type":"session.auto_mode_resolved","data":{"chosenModel":"claude-haiku-4.5"},"id":"e1"}"#,
        r#"{"type":"session.usage_checkpoint","data":{"totalNanoAiu":6528006000,"totalPremiumRequests":0.33},"id":"e1"}"#,
        r#"{"type":"model.call_start","data":{"turnId":"0","model":"claude-haiku-4.5"},"ephemeral":true,"id":"e1"}"#,
        r#"{"type":"assistant.message_start","data":{"messageId":"m1"},"ephemeral":true,"id":"e1"}"#,
        r#"{"type":"assistant.turn_start","data":{"turnId":"0"},"id":"e1"}"#,
        r#"{"type":"assistant.turn_end","data":{"turnId":"0"},"id":"e1"}"#,
        r#"{"type":"assistant.idle","data":{},"ephemeral":true,"id":"e1"}"#,
        r#"{"type":"user.message","data":{"content":"端子台を置いて"},"id":"e1"}"#,
        r#"{"type":"system.message","data":{"role":"system","content":"You are the GitHub Copilot CLI"},"id":"e1"}"#,
    ] {
        assert!(
            parse_copilot_events(line).is_empty(),
            "無視されるはず: {line}"
        );
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
        r#"{"type":"assistant.message_delta","data":{"messageId":"m1","deltaContent":""}}"#,
        "[1,2,3]",
    ] {
        assert!(
            parse_copilot_events(line).is_empty(),
            "無視されるはず: {line}"
        );
    }
}

// ---------------------------------------------------------------------------
// ターンをまたぐ辻褄合わせ (CopilotParser)
// ---------------------------------------------------------------------------

/// The tool name from the call is filled into the matching result, so the chat shows what finished.
/// ツール名は呼び出しから結果へ補完され、チャットには何が終わったのかが表示される。
#[test]
fn the_tool_name_is_carried_from_the_call_to_its_result() {
    let mut parser = CopilotParser::new();
    parser.push(
        r#"{"type":"tool.execution_start","data":{"toolCallId":"toolu_01","toolName":"madakecad-place_symbol","arguments":{}},"id":"e1"}"#,
    );
    let events = parser
        .push(r#"{"type":"tool.execution_complete","data":{"toolCallId":"toolu_01","success":true},"id":"e2"}"#);

    match events.first() {
        Some(AgentEvent::ToolUseFinished { tool, .. }) => {
            assert_eq!(tool, "madakecad-place_symbol")
        }
        other => panic!("tool_endのはず: {other:?}"),
    }
}

/// The same tool call is announced by both the message and the execution line, but the chat shows it once.
/// 同じツール呼び出しはメッセージと実行開始の両方に現れるが、チャットには一度だけ表示される。
#[test]
fn a_tool_call_announced_twice_is_shown_only_once() {
    let mut parser = CopilotParser::new();
    let from_message = r#"{"type":"assistant.message","data":{"messageId":"m1","content":"","toolRequests":[{"toolCallId":"toolu_01","name":"madakecad-place_symbol","arguments":{}}]},"id":"e1"}"#;
    let from_execution = r#"{"type":"tool.execution_start","data":{"toolCallId":"toolu_01","toolName":"madakecad-place_symbol","arguments":{}},"id":"e2"}"#;

    assert_eq!(parser.push(from_message).len(), 1);
    assert!(parser.push(from_execution).is_empty(), "2回目は捨てられる");
}

/// The turn's answer is the assistant's last message, because the final line does not repeat it.
/// ターンの回答はアシスタントの最後のメッセージになる(最終行には回答が載らないため)。
#[test]
fn the_turn_result_comes_from_the_last_assistant_message() {
    let mut parser = CopilotParser::new();
    parser.push(r#"{"type":"assistant.message_delta","data":{"messageId":"m1","deltaContent":"端子台X1を"},"ephemeral":true}"#);
    parser.push(r#"{"type":"assistant.message_delta","data":{"messageId":"m1","deltaContent":"配置しました。"},"ephemeral":true}"#);
    parser.push(r#"{"type":"assistant.message","data":{"messageId":"m1","content":"端子台X1を配置しました。","toolRequests":[],"phase":"final_answer","outputTokens":57}}"#);
    let events =
        parser.push(r#"{"type":"result","sessionId":"S1","exitCode":0,"usage":{"premiumRequests":0.33}}"#);

    match events.last() {
        Some(AgentEvent::TurnCompleted { result, usage }) => {
            assert_eq!(result, "端子台X1を配置しました。");
            // 最終行にトークン数は無いので、メッセージごとの出力トークンを積み上げる
            assert_eq!(usage.as_ref().unwrap().output_tokens, 57);
        }
        other => panic!("completedのはず: {other:?}"),
    }
}

/// Text that only arrived in the completed message (never as a delta) is still shown in the chat.
/// 差分では届かず完成メッセージにだけ載った本文も、取りこぼさずチャットに表示される。
#[test]
fn text_missing_from_the_deltas_is_still_delivered() {
    let mut parser = CopilotParser::new();
    parser.push(r#"{"type":"assistant.message_delta","data":{"messageId":"m1","deltaContent":"端子台X1を"},"ephemeral":true}"#);
    let events = parser.push(
        r#"{"type":"assistant.message","data":{"messageId":"m1","content":"端子台X1を配置しました。","toolRequests":[]}}"#,
    );

    assert_eq!(texts(&events), "配置しました。", "続きだけが届く");
}

/// The whole answer is delivered once when the CLI sends no deltas at all.
/// 差分が一切来ない場合は、完成した回答がまるごと一度だけ届く。
#[test]
fn the_whole_answer_is_delivered_when_no_deltas_arrive() {
    let mut parser = CopilotParser::new();
    let events = parser.push(
        r#"{"type":"assistant.message","data":{"messageId":"m1","content":"配置しました。","toolRequests":[]}}"#,
    );

    assert_eq!(texts(&events), "配置しました。");
}

/// The session id MadakeCAD pinned is not announced twice when the CLI confirms it in the final line.
/// MadakeCADが決めたセッションIDが最終行で返ってきただけのときは、二重に通知しない。
#[test]
fn a_confirmed_session_id_is_not_announced_twice() {
    let mut parser = CopilotParser::for_session("S1");

    let events = parser.push(r#"{"type":"result","sessionId":"S1","exitCode":0,"usage":{}}"#);

    assert!(
        !events
            .iter()
            .any(|e| matches!(e, AgentEvent::SessionStarted { .. })),
        "{events:?}"
    );
}

/// If the CLI answers under a different session id, that id is announced so the next turn resumes the right conversation.
/// CLIが別のセッションIDで答えた場合はそのIDが通知され、次のターンは正しい会話を再開する。
#[test]
fn a_different_session_id_from_the_cli_wins() {
    let mut parser = CopilotParser::for_session("S1");

    let events = parser.push(r#"{"type":"result","sessionId":"S2","exitCode":0,"usage":{}}"#);

    assert_eq!(
        events[0],
        AgentEvent::SessionStarted {
            session_id: "S2".to_string()
        }
    );
}

// ---------------------------------------------------------------------------
// 別名への寛容さ (バージョン差でフィールド名が変わっても壊れないこと)
// ---------------------------------------------------------------------------

/// Assistant text is delivered whichever field name the CLI uses for it.
/// アシスタントの本文は、CLIがどのフィールド名で出してもチャットへ届く。
#[test]
fn assistant_text_is_delivered_under_any_of_the_known_field_names() {
    for line in [
        r#"{"type":"assistant","text":"こんにちは"}"#,
        r#"{"type":"text","content":"こんにちは"}"#,
        r#"{"type":"text_delta","delta":"こんにちは"}"#,
        r#"{"type":"assistant.message_delta","data":{"messageId":"m1","delta_content":"こんにちは"}}"#,
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

/// Tool calls are recognised under the alternative field names too (tool/input, function/parameters).
/// ツール呼び出しは別のフィールド名(tool/input、function/parameters)でも認識される。
#[test]
fn tool_calls_are_recognised_under_alternative_field_names() {
    for line in [
        r#"{"type":"tool_use","tool_call_id":"call_1","tool":"madakecad-add_symbol","input":{"symbol":"TB"}}"#,
        r#"{"type":"tool.start","call_id":"call_1","tool_name":"madakecad-add_symbol","parameters":{"symbol":"TB"}}"#,
        r#"{"type":"function_call","id":"call_1","name":"madakecad-add_symbol","args":{"symbol":"TB"}}"#,
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

/// A tool result is recognised as failed under any of the known ways of saying so.
/// ツールの失敗は、知られているどの言い方でも失敗として認識される。
#[test]
fn a_failed_tool_result_is_recognised_under_any_wording() {
    for line in [
        r#"{"type":"tool_result","tool_call_id":"call_1","status":"error"}"#,
        r#"{"type":"tool_call_result","id":"call_1","is_error":true}"#,
        r#"{"type":"tool.end","call_id":"call_1","error":"boom"}"#,
    ] {
        match one(line) {
            AgentEvent::ToolUseFinished { id, is_error, .. } => {
                assert_eq!(id, "call_1", "{line}");
                assert!(is_error, "{line}");
            }
            other => panic!("tool_endのはず ({line}): {other:?}"),
        }
    }
    // 成否の印が無い完了は成功扱い
    match one(r#"{"type":"tool_result","tool_call_id":"call_1"}"#) {
        AgentEvent::ToolUseFinished { is_error, .. } => assert!(!is_error),
        other => panic!("tool_endのはず: {other:?}"),
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
        r#"{"type":"session.error","data":{"message":"rate limited"}}"#,
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
