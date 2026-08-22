//! GitHub Copilot CLIバックエンドのテスト。
//!
//! 本物のcopilot(GitHub認証とAIクレジットが要る)は呼ばず、JSONLを吐く
//! フェイクCLIを使う。実機で確かめてあるのは「起動引数が受け付けられること」と
//! 「未認証時の文面」で、JSONLの中身はもっともらしい形をフィクスチャで固定している。

use madake_agent::copilot_cli::{CopilotCliBackend, DEFAULT_COPILOT_MODEL};
use madake_agent::AgentEvent;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::mpsc;

const SESSION: &str = "39628e1e-925e-42e5-9619-7cda7c2671f1";

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn backend(script: &str) -> CopilotCliBackend {
    CopilotCliBackend::new(fixtures_dir().join(script), 9310)
}

fn kinds(events: &[AgentEvent]) -> Vec<&str> {
    events
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
        .collect()
}

fn value_of(args: &[String], flag: &str) -> String {
    let idx = args
        .iter()
        .position(|a| a == flag)
        .unwrap_or_else(|| panic!("{flag} が無い: {args:?}"));
    args[idx + 1].clone()
}

async fn collect(mut rx: mpsc::Receiver<AgentEvent>) -> Vec<AgentEvent> {
    let mut events = Vec::new();
    while let Some(event) = rx.recv().await {
        events.push(event);
    }
    events
}

/// The Copilot CLI is launched non-interactively with the flags needed to work unattended (JSON lines, auto-approved tools, no questions).
/// Copilot CLIは、人が見ていなくても働ける設定(JSONL出力・ツール自動許可・質問しない)で非対話起動される。
#[test]
fn args_contain_required_flags() {
    let args = backend("fake_copilot.sh").build_args("hi", None, SESSION, Path::new("/tmp/m.json"));

    assert_eq!(args[0], "-p", "プロンプトは先頭の-pで渡す: {args:?}");
    assert_eq!(args[1], "hi");
    assert_eq!(value_of(&args, "--output-format"), "json");
    assert_eq!(value_of(&args, "--additional-mcp-config"), "@/tmp/m.json");
    assert_eq!(value_of(&args, "--log-level"), "error");
    for flag in [
        // 非対話モードではツールの自動許可が必須(無いとCLIが確認待ちで止まる)
        "--allow-all-tools",
        // 質問されても答える相手がいない
        "--no-ask-user",
        // ユーザーのAGENTS.mdでMadakeCADの作図ルールを乱されないようにする
        "--no-custom-instructions",
        // 既定のgithub-mcp-serverは図面作成に不要
        "--disable-builtin-mcps",
    ] {
        assert!(args.iter().any(|a| a == flag), "{flag} が無い: {args:?}");
    }
    // 応答だけに切り詰める-s(--silent)は使わない(ツール実行や使用量の行が消えるため)
    assert!(
        !args.iter().any(|a| a == "-s" || a == "--silent"),
        "{args:?}"
    );
}

/// The first turn of a conversation pins a new session id, so later turns can continue the same session.
/// 会話の最初のターンは新しいセッションIDを固定し、以降のターンが同じセッションを続けられるようにする。
#[test]
fn the_first_turn_pins_a_new_session_id() {
    let args = backend("fake_copilot.sh").build_args("hi", None, SESSION, Path::new("/tmp/m.json"));

    assert_eq!(value_of(&args, "--session-id"), SESSION);
    assert!(!args.iter().any(|a| a == "--resume"), "{args:?}");
}

/// A follow-up turn resumes the same Copilot session instead of starting a new one, so the agent remembers the conversation.
/// 2ターン目以降は新しいセッションを作らず同じCopilotセッションを再開するので、エージェントは会話を覚えている。
#[test]
fn a_follow_up_turn_resumes_the_same_session() {
    let args = backend("fake_copilot.sh").build_args(
        "つづき",
        Some(SESSION),
        "unused-new-id",
        Path::new("/tmp/m.json"),
    );

    assert_eq!(value_of(&args, "--resume"), SESSION);
    assert!(!args.iter().any(|a| a == "--session-id"), "{args:?}");
}

/// The configured Copilot model is passed to the CLI ("auto" lets Copilot pick).
/// 設定したCopilotのモデルはCLIへ渡される(`auto`ならCopilotが自動で選ぶ)。
#[test]
fn the_configured_model_is_passed_to_the_cli() {
    let mut backend = backend("fake_copilot.sh");
    assert_eq!(
        backend.model.as_deref(),
        Some(DEFAULT_COPILOT_MODEL),
        "既定はauto"
    );
    backend.model = Some("claude-sonnet-4.5".to_string());
    let args = backend.build_args("hi", None, SESSION, Path::new("/tmp/m.json"));
    assert_eq!(value_of(&args, "--model"), "claude-sonnet-4.5");

    backend.model = None;
    let args = backend.build_args("hi", None, SESSION, Path::new("/tmp/m.json"));
    assert!(!args.iter().any(|a| a == "--model"), "{args:?}");
}

/// Copilot is pointed at MadakeCAD's own MCP server, so it edits drawings through the same commands as every other client.
/// CopilotにはMadakeCAD自身のMCPサーバーを渡すので、他のクライアントと同じコマンド経由で図面を編集する。
#[test]
fn the_mcp_config_points_at_the_local_madakecad_server() {
    let config: serde_json::Value =
        serde_json::from_str(&backend("fake_copilot.sh").mcp_config_json()).unwrap();
    let server = &config["mcpServers"]["madakecad"];

    assert_eq!(server["type"], "http");
    assert_eq!(server["url"], "http://127.0.0.1:9310/mcp");
    assert_eq!(server["tools"][0], "*", "MadakeCADのツールは全て使わせる");
}

/// Because the Copilot CLI has no system-prompt flag, MadakeCAD's drawing rules are prepended to the prompt under a clear heading.
/// Copilot CLIにはシステムプロンプト用のフラグが無いため、MadakeCADの作図ルールは見出し付きでプロンプトの先頭へ前置される。
#[test]
fn the_system_prompt_is_prepended_to_the_user_prompt() {
    let composed =
        CopilotCliBackend::compose_prompt(Some("アクティブシート: S1"), "端子台を置いて");

    assert!(composed.starts_with("# システム指示"), "{composed}");
    let system_at = composed.find("アクティブシート: S1").expect("システム指示");
    let user_at = composed.find("端子台を置いて").expect("ユーザーの依頼");
    assert!(system_at < user_at, "システム指示が先: {composed}");
    assert!(composed.contains("# ユーザーの依頼"), "{composed}");

    // システム指示が無いターンはユーザーの発言だけをそのまま渡す
    assert_eq!(
        CopilotCliBackend::compose_prompt(None, "端子台を置いて"),
        "端子台を置いて"
    );
}

/// A turn streams the events parsed from Copilot's JSON lines (session, text, tool use, completion).
/// ターンはCopilotのJSONLから解釈したイベント(セッション・本文・ツール実行・完了)を流す。
#[tokio::test]
async fn a_turn_streams_events_from_the_fake_cli() {
    let (tx, rx) = mpsc::channel(1024);
    backend("fake_copilot.sh")
        .send("hi", None, tx)
        .await
        .expect("send成功");

    let events = collect(rx).await;
    assert_eq!(
        kinds(&events),
        vec!["session", "text", "tool_start", "tool_end", "completed"]
    );
    match &events[0] {
        AgentEvent::SessionStarted { session_id } => {
            uuid::Uuid::parse_str(session_id).expect("セッションIDはUUID");
        }
        other => panic!("最初はsessionのはず: {other:?}"),
    }
    match &events[3] {
        AgentEvent::ToolUseFinished { tool, is_error, .. } => {
            assert_eq!(tool, "madakecad-add_symbol", "ツール名が補完されること");
            assert!(!is_error);
        }
        other => panic!("tool_endのはず: {other:?}"),
    }
    // JSONでない行・未知の型の行はフィクスチャに混ぜてある(捨てられること)
    assert_eq!(events.len(), 5, "{events:?}");
}

/// The composed prompt actually reaches the CLI as the -p argument.
/// 合成したプロンプトは実際に-p引数としてCLIへ届く。
#[tokio::test]
async fn the_composed_prompt_reaches_the_cli() {
    let (tx, rx) = mpsc::channel(1024);
    backend("fake_copilot_echo_prompt.sh")
        .send_with_context("端子台を置いて", None, Some("アクティブシート: S1"), tx)
        .await
        .expect("send成功");

    match collect(rx).await.last() {
        Some(AgentEvent::TurnCompleted { result, .. }) => {
            assert!(result.starts_with("# システム指示"), "{result}");
            assert!(result.contains("アクティブシート: S1"), "{result}");
            assert!(result.contains("端子台を置いて"), "{result}");
        }
        other => panic!("最後はcompletedのはず: {other:?}"),
    }
}

/// The MCP settings file handed to Copilot really exists while the turn runs, and it names MadakeCAD's server.
/// Copilotへ渡すMCP設定ファイルはターン実行中に実在し、MadakeCADのサーバーを指している。
#[tokio::test]
async fn the_mcp_config_file_exists_while_the_turn_runs() {
    let (tx, rx) = mpsc::channel(1024);
    backend("fake_copilot_dump_config.sh")
        .send("hi", None, tx)
        .await
        .expect("send成功");

    match collect(rx).await.last() {
        Some(AgentEvent::TurnCompleted { result, .. }) => {
            let config: serde_json::Value = serde_json::from_str(result).expect("JSONで書かれる");
            assert_eq!(
                config["mcpServers"]["madakecad"]["url"],
                "http://127.0.0.1:9310/mcp"
            );
        }
        other => panic!("最後はcompletedのはず(設定ファイルが無い?): {other:?}"),
    }
}

/// When Copilot is not signed in to GitHub, the chat says so and explains how to sign in.
/// GitHubへサインインしていない場合、チャットにその旨とサインイン手順が表示される。
#[tokio::test]
async fn a_signed_out_cli_is_reported_with_login_guidance() {
    let (tx, rx) = mpsc::channel(1024);
    backend("fake_copilot_unauth.sh")
        .send("hi", None, tx)
        .await
        .expect("send自体は成功");

    match collect(rx).await.last() {
        Some(AgentEvent::Error { message }) => {
            assert!(message.contains("未認証"), "{message}");
            assert!(message.contains("copilot"), "{message}");
            assert!(message.contains("/login"), "サインイン手順: {message}");
        }
        other => panic!("最後はErrorのはず: {other:?}"),
    }
}

/// The unauthenticated wording of the real CLI is recognised wherever it is printed.
/// 実機CLIの未認証メッセージは、どこに出ても未認証として認識される。
#[test]
fn the_real_unauthenticated_message_is_recognised() {
    assert!(CopilotCliBackend::is_unauthenticated(
        "Error: No authentication information found."
    ));
    assert!(CopilotCliBackend::is_unauthenticated(
        "Start 'copilot' and run the '/login' command"
    ));
    assert!(!CopilotCliBackend::is_unauthenticated(
        "You have run out of AI credits for this month."
    ));
}

/// When Copilot dies with the reason printed as plain text (e.g. credits exhausted), that reason reaches the chat error.
/// Copilotが理由(クレジット切れなど)を素のテキストで出して異常終了したとき、その理由がチャットのエラーに載る。
#[tokio::test]
async fn a_plain_stdout_reason_reaches_the_error_message() {
    let (tx, rx) = mpsc::channel(1024);
    backend("fake_copilot_fail_stdout.sh")
        .send("hi", None, tx)
        .await
        .expect("send自体は成功");

    match collect(rx).await.last() {
        Some(AgentEvent::Error { message }) => {
            assert!(message.contains("AI credits"), "stdoutの理由: {message}");
            assert!(
                message.contains("copilot"),
                "どのCLIか分かること: {message}"
            );
        }
        other => panic!("最後はErrorのはず: {other:?}"),
    }
}

/// If the event receiver goes away (the user cancelled), the turn stops promptly instead of blocking forever.
/// イベント受信側が消えた(ユーザーが中断した)場合、ターンは永久にブロックせず速やかに終わる。
#[tokio::test]
async fn a_cancelled_turn_stops_promptly() {
    let (tx, rx) = mpsc::channel(1);
    drop(rx);

    let result = tokio::time::timeout(
        Duration::from_secs(10),
        backend("fake_copilot_flood.sh").send("hi", None, tx),
    )
    .await;
    assert!(result.is_ok(), "キャンセル後にsendが返らない(パイプ詰まり)");
    result.unwrap().expect("send自体は成功");
}

/// Copilot CLI detection reads the version from the configured executable.
/// Copilot CLIの検出は、設定された実行ファイルからバージョンを読む。
#[tokio::test]
async fn detect_reads_the_version_from_the_configured_executable() {
    let path = fixtures_dir().join("fake_copilot.sh");
    let found = CopilotCliBackend::detect(Some(path.clone()))
        .await
        .expect("検出成功");

    assert_eq!(found.path, path);
    assert!(found.version.contains("1.0.80"), "{}", found.version);
}

/// Detection fails cleanly when the Copilot executable is not installed.
/// Copilotの実行ファイルが入っていない場合、検出は明確に失敗する。
#[tokio::test]
async fn detect_fails_when_copilot_is_not_installed() {
    assert!(
        CopilotCliBackend::detect(Some(fixtures_dir().join("no_such_copilot")))
            .await
            .is_err()
    );
}

/// Detection looks for copilot on PATH and in the usual npm install locations.
/// 検出はPATHと、npmの一般的なインストール先からcopilotを探す。
#[test]
fn default_candidates_include_the_known_install_paths() {
    let candidates = CopilotCliBackend::default_candidates();

    assert_eq!(candidates[0], PathBuf::from("copilot"));
    for expected in ["/usr/local/bin/copilot", "/opt/homebrew/bin/copilot"] {
        assert!(
            candidates.iter().any(|c| c == Path::new(expected)),
            "{expected} が候補に無い: {candidates:?}"
        );
    }
}

/// The connection test reports success when the CLI answers, and the sign-in guidance when it is signed out.
/// 接続テストは、CLIが応答すれば成功を、サインインしていなければサインイン案内を返す。
#[tokio::test]
async fn the_connection_test_distinguishes_success_from_being_signed_out() {
    assert!(backend("fake_copilot.sh").check_connection().await.is_ok());

    let error = backend("fake_copilot_unauth.sh")
        .check_connection()
        .await
        .expect_err("未認証は失敗になる");
    assert_eq!(error.kind, "copilot_auth");
    assert!(error.message.contains("/login"), "{}", error.message);

    let missing = CopilotCliBackend::new(fixtures_dir().join("no_such_copilot"), 9310)
        .check_connection()
        .await
        .expect_err("実行ファイルが無ければ失敗");
    assert_eq!(missing.kind, "copilot_missing");
}
