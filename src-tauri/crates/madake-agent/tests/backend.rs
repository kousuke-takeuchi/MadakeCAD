//! ClaudeCodeCliBackendのテスト。本物のclaudeは呼ばず、fixtureを吐くフェイクCLIを使う。

use madake_agent::backend::ClaudeCodeCliBackend;
use madake_agent::AgentEvent;
use std::path::{Path, PathBuf};
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
            AgentEvent::Error { .. } => "error",
        })
        .collect()
}

#[test]
fn args_contain_required_flags() {
    let backend = backend("fake_claude.sh");
    let args = backend.build_args("ヒューズを追加して", None, Path::new("/tmp/mcp.json"), None);

    assert_eq!(args[0], "-p");
    assert_eq!(args[1], "ヒューズを追加して");
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
        "続き",
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
    let (tx, mut rx) = mpsc::channel(64);
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
    let (tx, mut rx) = mpsc::channel(64);
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
