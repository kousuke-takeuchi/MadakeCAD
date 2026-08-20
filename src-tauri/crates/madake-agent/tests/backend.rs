//! ClaudeCodeCliBackendのテスト。本物のclaudeは呼ばず、fixtureを吐くフェイクCLIを使う。

use madake_agent::backend::ClaudeCodeCliBackend;
use madake_agent::AgentEvent;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::mpsc;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn backend(script: &str) -> ClaudeCodeCliBackend {
    ClaudeCodeCliBackend::new(fixtures_dir().join(script), 9310)
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

#[test]
fn args_contain_required_flags() {
    let backend = backend("fake_claude.sh");
    let args = backend.build_args(None, Path::new("/tmp/mcp.json"), None);

    assert_eq!(args[0], "-p");
    for pair in [
        ("--output-format", Some("stream-json")),
        ("--mcp-config", Some("/tmp/mcp.json")),
        ("--allowedTools", Some("mcp__madakecad__*")),
        ("--verbose", None),
        ("--include-partial-messages", None),
        ("--strict-mcp-config", None),
    ] {
        let idx = args
            .iter()
            .position(|a| a == pair.0)
            .unwrap_or_else(|| panic!("{} が無い: {args:?}", pair.0));
        if let Some(value) = pair.1 {
            assert_eq!(args[idx + 1], value, "{} の値", pair.0);
        }
    }
    assert!(!args.iter().any(|a| a == "--resume"), "{args:?}");
    assert!(!args.iter().any(|a| a == "--model"), "{args:?}");
    assert!(
        !args.iter().any(|a| a == "--append-system-prompt"),
        "{args:?}"
    );
}

#[test]
fn args_include_resume_model_and_system_prompt_when_given() {
    let mut backend = backend("fake_claude.sh");
    backend.model = Some("claude-opus-4-6".to_string());
    let args = backend.build_args(
        Some("39628e1e-925e-42e5-9619-7cda7c2671f1"),
        Path::new("/tmp/mcp.json"),
        Some("アクティブシート: S1"),
    );

    let value_of = |flag: &str| -> String {
        let idx = args
            .iter()
            .position(|a| a == flag)
            .unwrap_or_else(|| panic!("{flag} が無い: {args:?}"));
        args[idx + 1].clone()
    };
    assert_eq!(value_of("--resume"), "39628e1e-925e-42e5-9619-7cda7c2671f1");
    assert_eq!(value_of("--model"), "claude-opus-4-6");
    assert_eq!(value_of("--append-system-prompt"), "アクティブシート: S1");
}

#[test]
fn mcp_config_points_at_local_mcp_server() {
    let backend = backend("fake_claude.sh");
    let cfg: serde_json::Value = serde_json::from_str(&backend.mcp_config_json()).unwrap();
    assert_eq!(cfg["mcpServers"]["madakecad"]["type"], "http");
    assert_eq!(
        cfg["mcpServers"]["madakecad"]["url"],
        "http://127.0.0.1:9310/mcp"
    );
}

#[tokio::test]
async fn send_streams_events_from_fake_cli() {
    let backend = backend("fake_claude.sh");
    let (tx, mut rx) = mpsc::channel(1024);
    backend.send("hi", None, tx).await.expect("send成功");

    let mut events = Vec::new();
    while let Some(ev) = rx.recv().await {
        events.push(ev);
    }
    assert_eq!(
        kinds(&events),
        vec!["session", "tool_start", "tool_end", "text", "completed"]
    );
    match &events[2] {
        AgentEvent::ToolUseFinished { tool, is_error, .. } => {
            assert_eq!(tool, "Bash", "tool_use_idからツール名が補完されること");
            assert!(!is_error);
        }
        other => panic!("tool_endのはず: {other:?}"),
    }
}

#[tokio::test]
async fn send_reports_nonzero_exit_as_error_event() {
    let backend = backend("fake_claude_fail.sh");
    let (tx, mut rx) = mpsc::channel(1024);
    backend.send("hi", None, tx).await.expect("send自体は成功");

    let mut events = Vec::new();
    while let Some(ev) = rx.recv().await {
        events.push(ev);
    }
    match events.last() {
        Some(AgentEvent::Error { message }) => {
            assert!(
                message.contains("not logged in"),
                "stderrが含まれる: {message}"
            );
        }
        other => panic!("最後はErrorのはず: {other:?}"),
    }
}

/// プロンプトはargvではなくstdinで渡す(先頭が`-`でもフラグ扱いされないため)。
#[tokio::test]
async fn prompt_is_passed_through_stdin_not_argv() {
    let backend = backend("fake_claude_echo_prompt.sh");
    let prompt = "--dangerously-skip-permissions について説明して";
    let (tx, mut rx) = mpsc::channel(1024);
    backend.send(prompt, None, tx).await.expect("send成功");

    let mut events = Vec::new();
    while let Some(ev) = rx.recv().await {
        events.push(ev);
    }
    match events.last() {
        Some(AgentEvent::TurnCompleted { result, .. }) => assert_eq!(result, prompt),
        other => panic!("最後はTurnCompletedのはず(引数解釈エラーの可能性): {other:?}"),
    }
}

/// 受信側がdrop(キャンセル)したら、書き続ける子プロセスをkillしてすぐ抜けること。
#[tokio::test]
async fn send_returns_promptly_when_receiver_is_dropped() {
    let backend = backend("fake_claude_flood.sh");
    let (tx, rx) = mpsc::channel(1);
    drop(rx);

    let result = tokio::time::timeout(Duration::from_secs(10), backend.send("hi", None, tx)).await;
    assert!(result.is_ok(), "キャンセル後にsendが返らない(パイプ詰まり)");
    result.unwrap().expect("send自体は成功");
}

/// 行読みのIOエラー(不正UTF-8等)もErrorイベントとして流してからErrを返す。
#[tokio::test]
async fn io_error_while_reading_is_reported_as_error_event() {
    let backend = backend("fake_claude_badutf8.sh");
    let (tx, mut rx) = mpsc::channel(1024);
    let result = backend.send("hi", None, tx).await;
    assert!(result.is_err(), "IOエラーはErrで返る");

    let mut events = Vec::new();
    while let Some(ev) = rx.recv().await {
        events.push(ev);
    }
    assert!(
        matches!(events.last(), Some(AgentEvent::Error { .. })),
        "Errorイベントが流れること: {events:?}"
    );
}

#[tokio::test]
async fn detect_reads_version_from_configured_executable() {
    let path = fixtures_dir().join("fake_claude.sh");
    let found = ClaudeCodeCliBackend::detect(Some(path.clone()))
        .await
        .expect("検出成功");
    assert_eq!(found.path, path);
    assert!(found.version.contains("2.1.237"), "{}", found.version);
}

#[tokio::test]
async fn detect_fails_for_missing_executable() {
    let path = fixtures_dir().join("no_such_claude");
    assert!(ClaudeCodeCliBackend::detect(Some(path)).await.is_err());
}

/// GUI起動時のPATHは最小構成なので、PATHで見つからなくても候補を順に試す。
#[tokio::test]
async fn detect_falls_back_to_later_candidates() {
    let candidates = vec![
        PathBuf::from("madake-no-such-command-xyz"),
        fixtures_dir().join("no_such_claude"),
        fixtures_dir().join("fake_claude.sh"),
    ];
    let found = ClaudeCodeCliBackend::detect_from(&candidates)
        .await
        .expect("後ろの候補で検出できる");
    assert_eq!(found.path, candidates[2]);
}

#[tokio::test]
async fn detect_from_reports_error_when_no_candidate_works() {
    let candidates = vec![PathBuf::from("madake-no-such-command-xyz")];
    assert!(ClaudeCodeCliBackend::detect_from(&candidates)
        .await
        .is_err());
}

/// 既定候補はPATH上の`claude`を先頭に、既知のインストール先を含む。
#[test]
fn default_candidates_include_known_install_paths() {
    let candidates = ClaudeCodeCliBackend::default_candidates();
    assert_eq!(candidates[0], PathBuf::from("claude"));
    for expected in ["/usr/local/bin/claude", "/opt/homebrew/bin/claude"] {
        assert!(
            candidates.iter().any(|c| c == Path::new(expected)),
            "{expected} が候補に無い: {candidates:?}"
        );
    }
    if std::env::var_os("HOME").is_some() {
        assert!(
            candidates.iter().any(|c| c.ends_with(".local/bin/claude")),
            "{candidates:?}"
        );
    }
}
