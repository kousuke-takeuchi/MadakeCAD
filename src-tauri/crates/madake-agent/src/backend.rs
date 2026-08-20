//! Claude Code CLIをヘッドレス起動してAgentEventを流すバックエンド。
//!
//! APIキーは扱わない。認証はCLI側のOAuthセッション(`claude login`)に委譲する。
//! 図面編集ツールは内蔵MCPサーバー(`http://127.0.0.1:<port>/mcp`)だけを許可し、
//! `--strict-mcp-config`でユーザー環境のMCP設定を読ませない。

use crate::{AgentError, AgentEvent, Result, StreamParser};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

/// エージェントに開放するMCPツールのパターン(内蔵サーバーのみ)。
const ALLOWED_TOOLS: &str = "mcp__madakecad__*";

/// `claude --version`による検出結果。
#[derive(Debug, Clone, PartialEq)]
pub struct DetectResult {
    pub path: PathBuf,
    pub version: String,
}

/// ローカルのClaude Code CLIをサブプロセスとして駆動するバックエンド。
#[derive(Debug, Clone)]
pub struct ClaudeCodeCliBackend {
    /// `claude`実行ファイル(PATH上の名前でも絶対パスでも可)
    pub executable: PathBuf,
    /// `--model`へ渡すモデル(Noneでユーザー既定)
    pub model: Option<String>,
    /// 内蔵MCPサーバーのポート
    pub mcp_port: u16,
    /// 既定で付与する図面コンテキスト(`--append-system-prompt`)
    pub append_system_prompt: Option<String>,
}

impl ClaudeCodeCliBackend {
    pub fn new(executable: PathBuf, mcp_port: u16) -> Self {
        Self {
            executable,
            model: None,
            mcp_port,
            append_system_prompt: None,
        }
    }

    /// `--mcp-config`へ渡す一時ファイルの中身。
    pub fn mcp_config_json(&self) -> String {
        serde_json::json!({
            "mcpServers": {
                "madakecad": {
                    "type": "http",
                    "url": format!("http://127.0.0.1:{}/mcp", self.mcp_port),
                }
            }
        })
        .to_string()
    }

    /// claude CLIへ渡す引数列を組み立てる。
    pub fn build_args(
        &self,
        prompt: &str,
        session: Option<&str>,
        mcp_config_path: &Path,
        append_system_prompt: Option<&str>,
    ) -> Vec<String> {
        let mut args = vec![
            "-p".to_string(),
            prompt.to_string(),
            "--output-format".to_string(),
            "stream-json".to_string(),
            "--verbose".to_string(),
            // 無指定だと完成メッセージ単位でしか出ないため、デルタ配信に必須
            "--include-partial-messages".to_string(),
        ];
        if let Some(session) = session {
            args.push("--resume".to_string());
            args.push(session.to_string());
        }
        if let Some(model) = &self.model {
            args.push("--model".to_string());
            args.push(model.clone());
        }
        args.push("--mcp-config".to_string());
        args.push(mcp_config_path.display().to_string());
        args.push("--strict-mcp-config".to_string());
        args.push("--allowedTools".to_string());
        args.push(ALLOWED_TOOLS.to_string());
        if let Some(ctx) = append_system_prompt {
            args.push("--append-system-prompt".to_string());
            args.push(ctx.to_string());
        }
        args
    }

    /// 1ターンを実行し、イベントを`tx`へ流す(プロセス終了まで待つ)。
    pub async fn send(
        &self,
        prompt: &str,
        session: Option<&str>,
        tx: mpsc::Sender<AgentEvent>,
    ) -> Result<()> {
        self.send_with_context(prompt, session, self.append_system_prompt.as_deref(), tx)
            .await
    }

    /// [`Self::send`]の図面コンテキスト明示版(ターンごとに内容が変わるため)。
    pub async fn send_with_context(
        &self,
        prompt: &str,
        session: Option<&str>,
        append_system_prompt: Option<&str>,
        tx: mpsc::Sender<AgentEvent>,
    ) -> Result<()> {
        let config = TempMcpConfig::create(&self.mcp_config_json())?;
        let args = self.build_args(prompt, session, config.path(), append_system_prompt);

        let mut child = Command::new(&self.executable)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| AgentError::Spawn(format!("{}: {e}", self.executable.display())))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AgentError::Spawn("stdoutを取得できません".to_string()))?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| AgentError::Spawn("stderrを取得できません".to_string()))?;

        // stderrはstdoutと並行に吸い出す(パイプが詰まるとCLIが止まるため)
        let stderr_task = tokio::spawn(async move {
            let mut buf = String::new();
            let _ = stderr.read_to_string(&mut buf).await;
            buf
        });

        let mut parser = StreamParser::new();
        let mut lines = BufReader::new(stdout).lines();
        'read: while let Some(line) = lines.next_line().await? {
            for event in parser.push(&line) {
                if tx.send(event).await.is_err() {
                    // 受信側が閉じた(キャンセル等)。プロセスはdropでkillされる
                    break 'read;
                }
            }
        }

        let status = child.wait().await?;
        let stderr_text = stderr_task.await.unwrap_or_default();
        if !status.success() {
            let code = status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".to_string());
            let detail = stderr_text.trim();
            let message = if detail.is_empty() {
                format!("claude CLIが異常終了しました (exit {code})")
            } else {
                format!("claude CLIが異常終了しました (exit {code}): {detail}")
            };
            let _ = tx.send(AgentEvent::Error { message }).await;
        }
        Ok(())
    }

    /// claude CLIを検出する。パス指定があればそれのみ、無ければPATH上の`claude`。
    pub async fn detect(executable: Option<PathBuf>) -> Result<DetectResult> {
        let path = executable.unwrap_or_else(|| PathBuf::from("claude"));
        let output = Command::new(&path)
            .arg("--version")
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|e| AgentError::NotFound(format!("{}: {e}", path.display())))?;
        if !output.status.success() {
            return Err(AgentError::NotFound(format!(
                "{} --version が失敗しました",
                path.display()
            )));
        }
        Ok(DetectResult {
            path,
            version: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        })
    }
}

/// `--mcp-config`用の一時ファイル(dropで削除)。
struct TempMcpConfig {
    path: PathBuf,
}

impl TempMcpConfig {
    fn create(contents: &str) -> Result<Self> {
        let path = std::env::temp_dir().join(format!("madake-mcp-{}.json", uuid::Uuid::new_v4()));
        std::fs::write(&path, contents)?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempMcpConfig {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
