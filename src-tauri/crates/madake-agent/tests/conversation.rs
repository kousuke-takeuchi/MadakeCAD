//! 会話マネージャ(revision追跡)とチャット履歴永続化のテスト。

use madake_agent::conversation::{chat_path_for, load_chat, save_chat, Conversation, Role};
use madake_agent::AgentEvent;
use serde_json::json;
use std::path::{Path, PathBuf};

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("madake_agent_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 1ターン: プロンプト → セッション開始 → ツール2回 → テキスト → 完了。
/// 図面編集はEngineのrevisionを進めるので、開始/終了revisionを記録する。
fn run_turn(conv: &mut Conversation, start_rev: u64, end_rev: u64) {
    conv.begin_turn("24V系にヒューズF2を追加して", start_rev);
    conv.apply_event(
        &AgentEvent::SessionStarted {
            session_id: "sess-1".to_string(),
        },
        start_rev,
    );
    conv.apply_event(
        &AgentEvent::ToolUseStarted {
            id: "toolu_1".to_string(),
            tool: "mcp__madakecad__place_symbol".to_string(),
            input: json!({ "symbol_id": "fuse", "reference": "F2" }),
        },
        start_rev,
    );
    conv.apply_event(
        &AgentEvent::ToolUseFinished {
            id: "toolu_1".to_string(),
            tool: "mcp__madakecad__place_symbol".to_string(),
            is_error: false,
        },
        start_rev + 1,
    );
    conv.apply_event(
        &AgentEvent::ToolUseStarted {
            id: "toolu_2".to_string(),
            tool: "mcp__madakecad__draw_wire".to_string(),
            input: json!({ "from": "F2:1" }),
        },
        start_rev + 1,
    );
    conv.apply_event(
        &AgentEvent::ToolUseFinished {
            id: "toolu_2".to_string(),
            tool: "mcp__madakecad__draw_wire".to_string(),
            is_error: false,
        },
        end_rev,
    );
    conv.apply_event(
        &AgentEvent::TextDelta {
            text: "F2を".to_string(),
        },
        end_rev,
    );
    conv.apply_event(
        &AgentEvent::TextDelta {
            text: "追加しました".to_string(),
        },
        end_rev,
    );
    conv.apply_event(
        &AgentEvent::TurnCompleted {
            result: "F2を追加しました".to_string(),
            usage: None,
        },
        end_rev,
    );
}

#[test]
fn turn_records_engine_revision_range() {
    let mut conv = Conversation::new();
    run_turn(&mut conv, 7, 10);

    let assistant = conv.messages.last().unwrap();
    assert_eq!(assistant.role, Role::Assistant);
    assert_eq!(assistant.applied_revisions, (7, 10));
    // 「元に戻す」= (end - start)回のundo
    assert_eq!(assistant.applied_command_count(), 3);
    assert!(assistant.has_edits());
}

#[test]
fn turn_without_edits_has_zero_undo_count() {
    let mut conv = Conversation::new();
    conv.begin_turn("この図面の説明をして", 4);
    conv.apply_event(
        &AgentEvent::TextDelta {
            text: "24V系の制御盤です".to_string(),
        },
        4,
    );
    conv.apply_event(
        &AgentEvent::TurnCompleted {
            result: "24V系の制御盤です".to_string(),
            usage: None,
        },
        4,
    );
    let assistant = conv.messages.last().unwrap();
    assert_eq!(assistant.applied_command_count(), 0);
    assert!(!assistant.has_edits());
}

#[test]
fn turn_collects_text_tool_calls_and_session() {
    let mut conv = Conversation::new();
    run_turn(&mut conv, 7, 10);

    assert_eq!(conv.session_id.as_deref(), Some("sess-1"));
    assert_eq!(conv.messages.len(), 2);
    assert_eq!(conv.messages[0].role, Role::User);
    assert_eq!(conv.messages[0].text, "24V系にヒューズF2を追加して");

    let assistant = &conv.messages[1];
    assert_eq!(assistant.text, "F2を追加しました");
    assert_eq!(assistant.tool_calls.len(), 2);
    assert_eq!(assistant.tool_calls[0].tool, "mcp__madakecad__place_symbol");
    assert_eq!(assistant.tool_calls[0].input["reference"], "F2");
    assert!(assistant.tool_calls.iter().all(|t| t.finished));
    assert!(assistant.tool_calls.iter().all(|t| !t.is_error));
    assert!(assistant.error.is_none());
}

#[test]
fn second_turn_appends_messages_and_reuses_session() {
    let mut conv = Conversation::new();
    run_turn(&mut conv, 7, 10);
    run_turn(&mut conv, 10, 12);

    assert_eq!(conv.messages.len(), 4);
    assert_eq!(conv.messages[3].applied_revisions, (10, 12));
    assert_eq!(conv.messages[3].applied_command_count(), 2);
    assert_eq!(conv.session_id.as_deref(), Some("sess-1"));
}

#[test]
fn error_event_is_recorded_on_current_turn() {
    let mut conv = Conversation::new();
    conv.begin_turn("落ちるやつ", 3);
    conv.apply_event(
        &AgentEvent::Error {
            message: "claude CLIが異常終了しました (exit 3)".to_string(),
        },
        3,
    );
    let assistant = conv.messages.last().unwrap();
    assert_eq!(
        assistant.error.as_deref(),
        Some("claude CLIが異常終了しました (exit 3)")
    );
    assert_eq!(assistant.applied_revisions, (3, 3));
}

#[test]
fn chat_file_roundtrip_is_pretty_json() {
    let dir = temp_dir();
    let path = dir.join("plant.chat.json");

    let mut a = Conversation::new();
    a.model = Some("claude-opus-4-6".to_string());
    run_turn(&mut a, 7, 10);
    let mut b = Conversation::new();
    run_turn(&mut b, 0, 1);

    save_chat(&path, &[a.clone(), b.clone()]).unwrap();
    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(raw.contains("\n  "), "整形JSONであること");

    let loaded = load_chat(&path).unwrap();
    assert_eq!(loaded, vec![a, b]);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn load_chat_of_missing_file_is_empty() {
    let dir = temp_dir();
    let loaded = load_chat(&dir.join("none.chat.json")).unwrap();
    assert!(loaded.is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn chat_path_sits_next_to_project_file() {
    assert_eq!(
        chat_path_for(Path::new("/work/plant.mdkproj")),
        PathBuf::from("/work/plant.chat.json")
    );
}
