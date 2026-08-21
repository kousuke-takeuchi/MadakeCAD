//! 会話マネージャ(revision追跡)とチャット履歴永続化のテスト。

use madake_agent::conversation::{
    chat_path_for, load_chat, save_chat, AppliedRevisions, AppliedUndoDepth, Conversation,
    DocState, Role, CHAT_FORMAT_VERSION,
};
use madake_agent::AgentEvent;
use serde_json::json;
use std::path::{Path, PathBuf};

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("madake_agent_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 編集のみが起きた状態(revisionと深さが同じだけ進む素直なケース)。
fn edits(n: u64) -> DocState {
    DocState::new(n, n)
}

/// 1ターン: プロンプト → セッション開始 → ツール2回 → テキスト → 完了。
/// 図面編集はEngineのrevisionを進め、undo履歴も同じだけ深くなる。
fn run_turn(conv: &mut Conversation, start_rev: u64, end_rev: u64) {
    run_turn_between(conv, edits(start_rev), edits(end_rev));
}

/// 開始/終了のドキュメント状態を明示する版(ターン中にundoが混ざる検証用)。
fn run_turn_between(conv: &mut Conversation, start: DocState, end: DocState) {
    conv.begin_turn("24V系にヒューズF2を追加して", start);
    conv.apply_event(
        &AgentEvent::SessionStarted {
            session_id: "sess-1".to_string(),
        },
        start,
    );
    conv.apply_event(
        &AgentEvent::ToolUseStarted {
            id: "toolu_1".to_string(),
            tool: "mcp__madakecad__place_symbol".to_string(),
            input: json!({ "symbol_id": "fuse", "reference": "F2" }),
        },
        start,
    );
    conv.apply_event(
        &AgentEvent::ToolUseFinished {
            id: "toolu_1".to_string(),
            tool: "mcp__madakecad__place_symbol".to_string(),
            is_error: false,
        },
        DocState::new(start.revision + 1, start.undo_depth + 1),
    );
    conv.apply_event(
        &AgentEvent::ToolUseStarted {
            id: "toolu_2".to_string(),
            tool: "mcp__madakecad__draw_wire".to_string(),
            input: json!({ "from": "F2:1" }),
        },
        DocState::new(start.revision + 1, start.undo_depth + 1),
    );
    conv.apply_event(
        &AgentEvent::ToolUseFinished {
            id: "toolu_2".to_string(),
            tool: "mcp__madakecad__draw_wire".to_string(),
            is_error: false,
        },
        end,
    );
    conv.apply_event(
        &AgentEvent::TextDelta {
            text: "F2を".to_string(),
        },
        end,
    );
    conv.apply_event(
        &AgentEvent::TextDelta {
            text: "追加しました".to_string(),
        },
        end,
    );
    conv.apply_event(
        &AgentEvent::TurnCompleted {
            result: "F2を追加しました".to_string(),
            usage: None,
        },
        end,
    );
}

#[test]
fn turn_records_engine_revision_range() {
    let mut conv = Conversation::new();
    run_turn(&mut conv, 7, 10);

    let assistant = conv.messages.last().unwrap();
    assert_eq!(assistant.role, Role::Assistant);
    assert_eq!(
        assistant.applied_revisions,
        AppliedRevisions { start: 7, end: 10 }
    );
    assert_eq!(
        assistant.applied_undo_depth,
        AppliedUndoDepth { start: 7, end: 10 }
    );
    // 「元に戻す」= undoスタック深さの増分だけundo
    assert_eq!(assistant.applied_command_count(), 3);
    assert!(assistant.has_edits());
}

/// ターン中にundoが混ざるとrevisionは進むが深さは戻る。undo回数は深さ増分が正。
///
/// 例: エージェントが3コマンド実行し、うち1回をMCPのundoツールで戻した
/// (あるいはユーザーがUIで1編集+1undoを挟んだ)ターン。
/// revision差は4だが、実際に積まれているのは2コマンドしかない。
#[test]
fn undo_count_uses_undo_stack_depth_not_revision_delta() {
    let mut conv = Conversation::new();
    run_turn_between(&mut conv, DocState::new(7, 7), DocState::new(11, 9));

    let assistant = conv.messages.last().unwrap();
    assert_eq!(
        assistant.applied_revisions,
        AppliedRevisions { start: 7, end: 11 },
        "revision範囲は表示用にそのまま残る"
    );
    assert_eq!(
        assistant.applied_command_count(),
        2,
        "revision差(4)ではなく深さ増分(2)がundo回数"
    );
}

/// ターン中の編集が全てundoされていれば「編集なし」扱いになる。
#[test]
fn turn_whose_edits_were_all_undone_has_no_edits() {
    let mut conv = Conversation::new();
    run_turn_between(&mut conv, DocState::new(3, 1), DocState::new(9, 1));

    let assistant = conv.messages.last().unwrap();
    assert_eq!(assistant.applied_command_count(), 0);
    assert!(!assistant.has_edits());
}

/// 巻き戻しの記録は実行できた回数だけ反映される(途中失敗時の過剰undo防止)。
#[test]
fn record_undone_applies_only_the_completed_count() {
    let mut conv = Conversation::new();
    run_turn(&mut conv, 7, 10);

    let assistant = conv.current_turn_mut().unwrap();
    assistant.record_undone(1);
    assert_eq!(assistant.applied_command_count(), 2, "残り2回");
    assistant.record_undone(2);
    assert_eq!(assistant.applied_command_count(), 0);
    assert_eq!(
        assistant.applied_revisions,
        AppliedRevisions { start: 7, end: 7 },
        "全部戻したら適用済みではなくなる"
    );
}

#[test]
fn turn_without_edits_has_zero_undo_count() {
    let mut conv = Conversation::new();
    conv.begin_turn("この図面の説明をして", edits(4));
    conv.apply_event(
        &AgentEvent::TextDelta {
            text: "24V系の制御盤です".to_string(),
        },
        edits(4),
    );
    conv.apply_event(
        &AgentEvent::TurnCompleted {
            result: "24V系の制御盤です".to_string(),
            usage: None,
        },
        edits(4),
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
    assert_eq!(
        conv.messages[3].applied_revisions,
        AppliedRevisions { start: 10, end: 12 }
    );
    assert_eq!(conv.messages[3].applied_command_count(), 2);
    assert_eq!(conv.session_id.as_deref(), Some("sess-1"));
}

#[test]
fn error_event_is_recorded_on_current_turn() {
    let mut conv = Conversation::new();
    conv.begin_turn("落ちるやつ", edits(3));
    conv.apply_event(
        &AgentEvent::Error {
            message: "claude CLIが異常終了しました (exit 3)".to_string(),
        },
        edits(3),
    );
    let assistant = conv.messages.last().unwrap();
    assert_eq!(
        assistant.error.as_deref(),
        Some("claude CLIが異常終了しました (exit 3)")
    );
    assert_eq!(
        assistant.applied_revisions,
        AppliedRevisions { start: 3, end: 3 }
    );
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
fn load_chat_rejects_newer_format_version() {
    let dir = temp_dir();
    let path = dir.join("future.chat.json");
    let future = CHAT_FORMAT_VERSION + 1;
    std::fs::write(
        &path,
        format!(r#"{{"format_version":{future},"conversations":[]}}"#),
    )
    .unwrap();

    let err = load_chat(&path).expect_err("新しいフォーマット版は明示エラー");
    let message = err.to_string();
    assert!(
        message.contains(&future.to_string()),
        "版番号が伝わること: {message}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// 保存中にクラッシュしても既存ファイルが壊れないよう、一時ファイル→renameで書く。
#[test]
fn save_chat_writes_atomically_and_leaves_no_temp_file() {
    let dir = temp_dir();
    let path = dir.join("plant.chat.json");

    let mut a = Conversation::new();
    run_turn(&mut a, 0, 1);
    save_chat(&path, &[a.clone()]).unwrap();
    save_chat(&path, &[a.clone(), Conversation::new()]).unwrap();

    let files: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(files, vec!["plant.chat.json".to_string()], "{files:?}");
    assert_eq!(load_chat(&path).unwrap().len(), 2);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn load_chat_of_missing_file_is_empty() {
    let dir = temp_dir();
    let loaded = load_chat(&dir.join("none.chat.json")).unwrap();
    assert!(loaded.is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

/// 会話履歴の相対時刻表示("8分前"等)の元になる値。作成時に入り、
/// ターンの開始・イベント反映のたびに進む。
#[test]
fn updated_at_is_set_on_creation_and_advances_with_the_turn() {
    let mut conv = Conversation::new();
    assert!(conv.updated_at > 0, "作成時に時刻が入ること");

    // ミリ秒分解能なので「進むこと」は0へ戻してから確かめる
    conv.updated_at = 0;
    conv.begin_turn("24V系にヒューズF2を追加して", edits(0));
    assert!(conv.updated_at > 0, "ターン開始で更新されること");

    conv.updated_at = 0;
    conv.apply_event(
        &AgentEvent::TextDelta {
            text: "配置しました".to_string(),
        },
        edits(1),
    );
    assert!(conv.updated_at > 0, "イベント反映で更新されること");
}

/// `updated_at`を持たない旧`chat.json`も読めること(時刻不明の`0`になる)。
#[test]
fn load_chat_defaults_updated_at_for_legacy_files() {
    let dir = temp_dir();
    let path = dir.join("legacy.chat.json");
    std::fs::write(
        &path,
        format!(
            r#"{{"format_version":{CHAT_FORMAT_VERSION},"conversations":[{{
                "id":"6f1b7f2e-6a3a-4a1f-9d4e-2b0c9d5a1e77",
                "session_id":null,
                "messages":[],
                "model":null
            }}]}}"#
        ),
    )
    .unwrap();

    let loaded = load_chat(&path).unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].updated_at, 0);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn chat_path_sits_next_to_project_file() {
    assert_eq!(
        chat_path_for(Path::new("/work/plant.mdkproj")),
        PathBuf::from("/work/plant.chat.json")
    );
}
