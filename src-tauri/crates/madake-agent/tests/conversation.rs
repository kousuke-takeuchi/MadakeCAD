//! 会話マネージャ(revision追跡)とチャット履歴永続化のテスト。

use madake_agent::conversation::{
    chat_path_for, load_chat, save_chat, AppliedRevisions, AppliedUndoDepth, Conversation,
    DocState, MessageBlock, Role, CHAT_FORMAT_VERSION, MAX_TOOL_OUTPUT_BYTES,
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
            output: None,
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
            output: None,
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

/// Each turn records the engine revision range it spanned, for display and debugging.
/// 各ターンは跨いだエンジンrevision範囲を記録する(表示・デバッグ用)。
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

/// The turn's undo count is the undo-stack depth delta, not the revision delta (undo/redo also advance revisions).
/// ターンのundo回数はrevision差分ではなくundoスタック深さの増分で数える(undo/redoでもrevisionは進むため)。
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

/// A turn whose edits were all undone during the turn counts as having no edits.
/// ターン中に編集がすべてundoされた場合、そのターンは編集なしとして扱われる。
#[test]
fn turn_whose_edits_were_all_undone_has_no_edits() {
    let mut conv = Conversation::new();
    run_turn_between(&mut conv, DocState::new(3, 1), DocState::new(9, 1));

    let assistant = conv.messages.last().unwrap();
    assert_eq!(assistant.applied_command_count(), 0);
    assert!(!assistant.has_edits());
}

/// Recording a rollback clears the turn's applied range, so it is no longer offered for undo.
/// 巻き戻しを記録するとターンの適用範囲が畳まれ、以降は「元に戻す」の対象にならない。
#[test]
fn record_reverted_clears_the_applied_range() {
    let mut conv = Conversation::new();
    run_turn(&mut conv, 7, 10);

    let assistant = conv.current_turn_mut().unwrap();
    assert_eq!(assistant.applied_command_count(), 3);
    assistant.record_reverted();
    assert_eq!(assistant.applied_command_count(), 0);
    assert!(!assistant.has_edits());
    assert_eq!(
        assistant.applied_revisions,
        AppliedRevisions { start: 7, end: 7 },
        "巻き戻したターンは適用済みではなくなる"
    );
}

/// A text-only turn (no document edits) has an undo count of zero.
/// テキストのみのターン(図面編集なし)のundo回数はゼロ。
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

/// A turn collects the streamed text, the tool calls, and the CLI session id.
/// ターンはストリームされたテキスト・ツール実行・CLIセッションIDを収集する。
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

/// A second turn appends to the conversation and resumes the same CLI session.
/// 2回目のターンは会話に追記され、同じCLIセッションを再開する。
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

/// An error event is recorded on the current turn's message.
/// エラーイベントは現在ターンのメッセージに記録される。
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

/// Chat history saves as pretty JSON and loads back identically.
/// チャット履歴は整形JSONで保存され、同一内容で読み戻せる。
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

/// A chat file from a newer format version is rejected rather than silently mangled.
/// 新しいformat_versionのチャットファイルは黙って壊さず拒否する。
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

/// Chat saving writes atomically (temp file + rename) and leaves no temp file behind.
/// チャット保存はアトミック(一時ファイル+リネーム)で、一時ファイルを残さない。
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

/// Loading chat history when no file exists yields an empty history.
/// チャットファイルが無い場合は空の履歴として読み込まれる。
#[test]
fn load_chat_of_missing_file_is_empty() {
    let dir = temp_dir();
    let loaded = load_chat(&dir.join("none.chat.json")).unwrap();
    assert!(loaded.is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

/// Conversations get an updated_at timestamp on creation that advances with each turn.
/// 会話は作成時にupdated_atを持ち、ターンごとに進む。
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

/// Legacy chat files without updated_at load with a sensible default.
/// updated_atの無い旧チャットファイルは妥当な既定値で読み込まれる。
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

/// The user prompt and the agent reply of one turn share a single stable turn id.
/// 1ターンのユーザー発話とエージェント応答には同じターンID(巻き戻しの目印)が付く。
#[test]
fn the_two_messages_of_a_turn_share_one_turn_id() {
    let mut conv = Conversation::new();
    run_turn(&mut conv, 7, 10);

    let turn_id = conv.messages[0].turn_id;
    assert!(!turn_id.is_nil(), "ターンIDが採番されること");
    assert_eq!(
        conv.messages[1].turn_id, turn_id,
        "ユーザー発話とアシスタント応答は同じターン"
    );
    assert_eq!(
        conv.turn(turn_id).map(|m| m.role),
        Some(Role::Assistant),
        "ターンIDからはアシスタント応答が引ける"
    );
}

/// Each turn gets its own turn id, so a turn can be addressed after later turns are appended.
/// ターンごとに別のターンIDが振られ、後続ターンが積まれても対象を指定できる。
#[test]
fn each_turn_gets_its_own_turn_id() {
    let mut conv = Conversation::new();
    run_turn(&mut conv, 7, 10);
    run_turn(&mut conv, 10, 12);

    let first = conv.messages[1].turn_id;
    let second = conv.messages[3].turn_id;
    assert_ne!(first, second, "ターンごとに別ID");
    assert_eq!(conv.turn_index(first), Some(1));
    assert_eq!(conv.turn_index(second), Some(3));
    assert_eq!(
        conv.turn_index(uuid::Uuid::new_v4()),
        None,
        "未知のターンIDは見つからない"
    );
}

/// A legacy chat file (older format version, no turn ids) loads with turn ids assigned per turn boundary.
/// 旧フォーマット(ターンID無し)のチャット履歴は、ターン境界からIDを採番して読み込まれる。
#[test]
fn load_chat_migrates_legacy_files_by_assigning_turn_ids() {
    let dir = temp_dir();
    let path = dir.join("legacy_turns.chat.json");
    std::fs::write(
        &path,
        r#"{"format_version":1,"conversations":[{
            "id":"6f1b7f2e-6a3a-4a1f-9d4e-2b0c9d5a1e77",
            "session_id":"sess-1",
            "model":null,
            "messages":[
                {"role":"user","text":"F2を追加","applied_revisions":{"start":3,"end":3}},
                {"role":"assistant","text":"追加しました","applied_revisions":{"start":3,"end":5},
                 "applied_undo_depth":{"start":3,"end":5}},
                {"role":"user","text":"線番を振って","applied_revisions":{"start":5,"end":5}},
                {"role":"assistant","text":"振りました","applied_revisions":{"start":5,"end":6},
                 "applied_undo_depth":{"start":5,"end":6}}
            ]
        }]}"#,
    )
    .unwrap();

    let loaded = load_chat(&path).expect("旧フォーマットも読める");
    let conv = &loaded[0];
    assert_eq!(conv.messages.len(), 4, "既存の会話が壊れない");
    assert_eq!(conv.messages[1].text, "追加しました");
    assert_eq!(conv.messages[1].applied_command_count(), 2);

    assert!(conv.messages.iter().all(|m| !m.turn_id.is_nil()));
    assert_eq!(
        conv.messages[0].turn_id, conv.messages[1].turn_id,
        "1ターン目の2メッセージは同じID"
    );
    assert_eq!(
        conv.messages[2].turn_id, conv.messages[3].turn_id,
        "2ターン目の2メッセージは同じID"
    );
    assert_ne!(
        conv.messages[1].turn_id, conv.messages[3].turn_id,
        "ターンをまたぐと別ID"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// Saving chat history stamps the current format version so migrated files are not re-migrated.
/// チャット履歴の保存は現在のフォーマット版を記録する(移行済みファイルを再移行しない)。
#[test]
fn save_chat_stamps_the_current_format_version() {
    let dir = temp_dir();
    let path = dir.join("stamped.chat.json");
    let mut conv = Conversation::new();
    run_turn(&mut conv, 0, 1);
    save_chat(&path, &[conv.clone()]).unwrap();

    let raw: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(raw["format_version"], json!(CHAT_FORMAT_VERSION));
    assert_eq!(
        raw["conversations"][0]["messages"][0]["turn_id"],
        json!(conv.messages[0].turn_id.to_string()),
        "ターンIDが保存される"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// The chat file lives next to the project file as <name>.chat.json.
/// チャットファイルはプロジェクトの隣に<名前>.chat.jsonとして置かれる。
#[test]
fn chat_path_sits_next_to_project_file() {
    assert_eq!(
        chat_path_for(Path::new("/work/plant.mdkproj")),
        PathBuf::from("/work/plant.chat.json")
    );
}

fn ev_text(t: &str) -> AgentEvent {
    AgentEvent::TextDelta { text: t.to_string() }
}
fn ev_think(t: &str) -> AgentEvent {
    AgentEvent::ThinkingDelta { text: t.to_string() }
}
fn ev_start(id: &str) -> AgentEvent {
    AgentEvent::ToolUseStarted { id: id.into(), tool: "mcp__madakecad__place_symbol".into(), input: json!({}) }
}
fn ev_end(id: &str, output: Option<&str>) -> AgentEvent {
    AgentEvent::ToolUseFinished {
        id: id.into(),
        tool: "mcp__madakecad__place_symbol".into(),
        is_error: false,
        output: output.map(str::to_string),
    }
}

/// A reply is recorded as blocks in the order it happened: consecutive pieces of the same kind join one block, and a change of kind starts a new block, while the plain text and tool list are still filled as before.
/// 応答は起きた順のブロックとして記録される。同じ種類が続く間は1つのブロックにまとまり、種類が変わると新しいブロックになる。本文とツールの一覧も今までどおり埋まる。
#[test]
fn replies_are_recorded_as_blocks_in_the_order_they_happened() {
    let mut conv = Conversation::new();
    let s = edits(0);
    conv.begin_turn("F2を足して", s);
    for (t, ev) in [
        (1_000, ev_think("24V系を")),
        (1_400, ev_think("確かめる")),
        (1_500, ev_text("確認します。")),
        (1_600, ev_start("t1")),
        (1_700, ev_end("t1", Some("{\"net\":\"N3\"}"))),
        (1_800, ev_start("t2")),
        (1_900, ev_end("t2", None)),
        (2_000, ev_think("配置できた")),
        (2_100, ev_text("F2を")),
        (2_200, ev_text("追加しました")),
    ] {
        conv.apply_event_at(&ev, s, t);
    }
    let m = conv.last_turn().unwrap();
    assert_eq!(
        m.blocks,
        vec![
            MessageBlock::Thinking { text: "24V系を確かめる".into(), duration_ms: Some(400) },
            MessageBlock::Text { text: "確認します。".into() },
            MessageBlock::Tool { id: "t1".into() },
            MessageBlock::Tool { id: "t2".into() },
            MessageBlock::Thinking { text: "配置できた".into(), duration_ms: Some(0) },
            MessageBlock::Text { text: "F2を追加しました".into() },
        ]
    );
    assert_eq!(m.text, "確認します。F2を追加しました");
    assert_eq!(m.tool_calls.len(), 2);
    assert_eq!(m.tool_calls[0].output.as_deref(), Some("{\"net\":\"N3\"}"));
    assert_eq!(m.tool_calls[0].output_size, Some(12));
    assert_eq!(m.tool_calls[1].output, None);
}

/// A tool result longer than 8 KB keeps only its head, cut before a character that would cross the limit, and the full size is kept so the screen can say how much was left out.
/// 8 KBより長いツールの結果は先頭だけを残し、上限をまたぐ文字の手前で切る。省いた量を画面が示せるよう、全体の大きさも残す。
#[test]
fn a_long_tool_result_is_cut_on_a_character_boundary() {
    // 「あ」は3バイト。8192 = 3 * 2730 + 2 なので、2731文字目が上限をまたぐ
    let long = "あ".repeat(3000);
    let mut conv = Conversation::new();
    conv.begin_turn("全体を見せて", edits(0));
    conv.apply_event(&ev_start("t1"), edits(0));
    conv.apply_event(&ev_end("t1", Some(&long)), edits(0));
    let call = &conv.last_turn().unwrap().tool_calls[0];
    let kept = call.output.as_deref().unwrap();
    assert_eq!(kept.len(), 2730 * 3);
    assert!(kept.len() <= MAX_TOOL_OUTPUT_BYTES);
    assert!(kept.chars().all(|c| c == 'あ'));
    assert_eq!(call.output_size, Some(9000));
}

/// A tool finish without an id (some Copilot lines) still puts its result on the open call of the same tool, as before.
/// idの無いツール完了 (Copilotの一部の行) でも、今までどおり同じツールの未完了の呼び出しに結果が付く。
#[test]
fn a_finish_without_an_id_puts_the_result_on_the_open_call_of_that_tool() {
    let mut conv = Conversation::new();
    conv.begin_turn("置いて", edits(0));
    conv.apply_event(&ev_start("t1"), edits(0));
    conv.apply_event(&ev_end("", Some("ok")), edits(1));
    let call = &conv.last_turn().unwrap().tool_calls[0];
    assert!(call.finished);
    assert_eq!(call.output.as_deref(), Some("ok"));
}

/// A chat file saved before blocks and tool results existed still loads, its messages have no blocks, and saving it again does not write empty blocks or results.
/// ブロックとツールの結果が無かった頃に保存した会話ファイルも読み込め、メッセージのブロックは空になる。保存し直しても空のブロックや結果は書き出さない。
#[test]
fn an_old_chat_file_without_blocks_still_loads() {
    let dir = temp_dir();
    let path = dir.join("old.chat.json");
    let mut conv = Conversation::new();
    run_turn(&mut conv, 0, 2);
    save_chat(&path, &[conv.clone()]).unwrap();
    // 今の形式から blocks / output を取り除いて、旧形式のファイルを作る
    let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    for m in v["conversations"][0]["messages"].as_array_mut().unwrap() {
        m.as_object_mut().unwrap().remove("blocks");
        for c in m["tool_calls"].as_array_mut().unwrap() {
            c.as_object_mut().unwrap().remove("output");
            c.as_object_mut().unwrap().remove("output_size");
        }
    }
    std::fs::write(&path, serde_json::to_string_pretty(&v).unwrap()).unwrap();

    let loaded = load_chat(&path).unwrap();
    let m = loaded[0].last_turn().unwrap();
    assert!(m.blocks.is_empty());
    assert_eq!(m.tool_calls.len(), 2);
    assert_eq!(m.tool_calls[0].output, None);

    save_chat(&path, &loaded).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("\"blocks\""), "空のblocksを書き出している");
    assert!(!text.contains("\"output\""), "空のoutputを書き出している");
    std::fs::remove_dir_all(&dir).ok();
}

/// A chat file containing a block kind this build does not know still loads: the unknown block is kept as an unknown placeholder and the rest of the history is intact.
/// このビルドが知らない種類のブロックを含む会話ファイルも読み込める。知らないブロックは「不明」として受け止め、残りの履歴はそのまま残る。
#[test]
fn a_chat_file_with_an_unknown_block_kind_still_loads() {
    let dir = temp_dir();
    let path = dir.join("future.chat.json");
    let mut conv = Conversation::new();
    run_turn(&mut conv, 0, 2);
    save_chat(&path, &[conv.clone()]).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let msgs = v["conversations"][0]["messages"].as_array_mut().unwrap();
    let blocks = msgs.last_mut().unwrap()["blocks"].as_array_mut().unwrap();
    let known = blocks.len();
    assert!(known >= 1, "テストの前提: 既知のブロックがある");
    blocks.insert(1.min(known), serde_json::json!({"type": "future_kind", "foo": 1}));
    std::fs::write(&path, serde_json::to_string_pretty(&v).unwrap()).unwrap();

    let loaded = load_chat(&path).unwrap();
    assert_eq!(loaded.len(), 1);
    let m = loaded[0].last_turn().unwrap();
    assert_eq!(m.blocks.len(), known + 1);
    assert_eq!(m.blocks[1], MessageBlock::Unknown);
    assert_eq!(m.blocks[0], conv.last_turn().unwrap().blocks[0]);
    assert_eq!(m.tool_calls.len(), 2);
    std::fs::remove_dir_all(&dir).ok();
}
