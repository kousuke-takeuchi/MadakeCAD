//! エージェントのターン実行バックエンド。
//!
//! [`AgentBackend`]が「1ターン = プロンプト+システムプロンプト → イベント列」の
//! 共通の形で、実装は2つ:
//!
//! - [`ClaudeCodeCliBackend`]: ローカルのClaude Code CLIをヘッドレス起動する。
//!   APIキーは扱わず、認証はCLI側のOAuthセッション(`claude login`)に委譲する。
//!   図面編集ツールは内蔵MCPサーバー(`http://127.0.0.1:<port>/mcp`)だけを許可し、
//!   `--strict-mcp-config`でユーザー環境のMCP設定を読ませない
//! - [`crate::AnthropicApiBackend`]: Anthropic Messages APIへ直接つなぐ
//!   (APIキーはOSキーチェーンから。ツールは[`crate::tools::ToolBridge`]経由)
//!
//! どちらも流すのは同じ[`AgentEvent`]なので、UIはバックエンドの違いを知らない。

use crate::conversation::Role;
use crate::{AgentError, AgentEvent, Result, StreamParser};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

/// 会話の過去のやりとり1件(API直結バックエンドが文脈として送り直す)。
///
/// CLIバックエンドはCLI側のセッション(`--resume`)が文脈を持つため使わない。
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryMessage {
    pub role: Role,
    pub text: String,
}

/// 1ターンの入力。
#[derive(Debug, Clone, Copy)]
pub struct TurnRequest<'a> {
    /// ユーザーの発言
    pub prompt: &'a str,
    /// CLIセッションID(CLIバックエンドの`--resume`用。API直結では使わない)
    pub session: Option<&'a str>,
    /// 図面コンテキスト+作図ルール+規格知識([`crate::knowledge::system_prompt`])
    pub system_prompt: Option<&'a str>,
    /// この会話の過去のやりとり(古い順)
    pub history: &'a [HistoryMessage],
}

impl<'a> TurnRequest<'a> {
    pub fn new(prompt: &'a str) -> Self {
        Self {
            prompt,
            session: None,
            system_prompt: None,
            history: &[],
        }
    }
}

/// [`AgentBackend::run_turn`]の戻り値(トレイトオブジェクトのままawaitできる形)。
pub type TurnFuture<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

/// 1ターンを実行してイベントを流すもの。
///
/// エラーの扱いは実装をまたいで一本化してある: 失敗は種類を問わず必ず
/// [`AgentEvent::Error`]として`tx`へ流れる。呼び出し側はイベントだけを見て
/// UI表示すればよい。戻り値の`Err`は「そのうえで異常終了した」ことを示す。
pub trait AgentBackend: Send + Sync {
    fn run_turn<'a>(
        &'a self,
        request: TurnRequest<'a>,
        tx: mpsc::Sender<AgentEvent>,
    ) -> TurnFuture<'a>;
}

/// エージェントに開放するMCPツールのパターン(内蔵サーバーのみ)。
/// 自動承認するツール。図面の編集は内蔵MCP経由(Commandエンジン)に限り、
/// ファイル系は**読み取り専用**だけ許す(同梱ドキュメントを出典つきで引用するため)。
const ALLOWED_TOOLS: &str = "mcp__madakecad__*,Read,Glob,Grep";

/// `claude --version`による検出結果。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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
    ///
    /// プロンプトはここには含めない。`-` 始まりのプロンプトがフラグとして解釈される
    /// のを避けるため、stdin経由で渡す(`echo <prompt> | claude -p ...`と同じ形)。
    pub fn build_args(
        &self,
        session: Option<&str>,
        mcp_config_path: &Path,
        append_system_prompt: Option<&str>,
    ) -> Vec<String> {
        let mut args = vec![
            "-p".to_string(),
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
        // 同梱ドキュメントはCLIの作業ディレクトリの外にあるため、読める場所として明示する
        if let Some(docs) = crate::knowledge::docs_dir() {
            args.push("--add-dir".to_string());
            args.push(docs.display().to_string());
        }
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
    ///
    /// エラーの扱いは一本化してある: 失敗は種類を問わず必ず
    /// [`AgentEvent::Error`] として`tx`へ流れる。呼び出し側はイベントだけを見て
    /// UI表示すればよい。戻り値の`Err`は「そのうえで異常終了した」ことを示す
    /// (CLIの非0終了はイベントのみでOkを返す)。
    pub async fn send_with_context(
        &self,
        prompt: &str,
        session: Option<&str>,
        append_system_prompt: Option<&str>,
        tx: mpsc::Sender<AgentEvent>,
    ) -> Result<()> {
        let error_tx = tx.clone();
        match self
            .stream_turn(prompt, session, append_system_prompt, tx)
            .await
        {
            Ok(()) => Ok(()),
            Err(e) => {
                let _ = error_tx
                    .send(AgentEvent::Error {
                        message: e.to_string(),
                    })
                    .await;
                Err(e)
            }
        }
    }

    /// 1ターンの実処理。エラーイベント化は[`Self::send_with_context`]が受け持つ。
    async fn stream_turn(
        &self,
        prompt: &str,
        session: Option<&str>,
        append_system_prompt: Option<&str>,
        tx: mpsc::Sender<AgentEvent>,
    ) -> Result<()> {
        let config = TempMcpConfig::create(&self.mcp_config_json())?;
        let args = self.build_args(session, config.path(), append_system_prompt);

        let mut child = Command::new(&self.executable)
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| AgentError::Spawn(format!("{}: {e}", self.executable.display())))?;

        // プロンプトはstdin経由(`-`始まりでもフラグ扱いされない)。書き終えたら閉じる。
        // stdoutの読み出しと並行させないと、長いプロンプトでデッドロックし得る
        let stdin_task = child.stdin.take().map(|mut stdin| {
            let prompt = prompt.to_string();
            tokio::spawn(async move {
                stdin.write_all(prompt.as_bytes()).await?;
                stdin.shutdown().await
            })
        });

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
        let mut read_error = None;
        // stream-jsonとして解釈できなかった最後のstdout行。実物のCLIは利用上限などの
        // 理由を素のテキストで出して非0終了することがあり、異常終了時の説明に使う
        let mut last_plain_line: Option<String> = None;
        'read: loop {
            // 受信側のcloseは`send`の失敗だけでは検知できない。イベントにならない行
            // (thinking_delta・未知の行)が続く間はsendが呼ばれず、キャンセル後も
            // CLIが走り続けて編集を重ねてしまうため、行の待ち受けと同時に監視する。
            // どちらもキャンセル安全(`Lines::next_line` / `Sender::closed`)。
            let line = tokio::select! {
                biased;
                _ = tx.closed() => {
                    let _ = child.start_kill();
                    break 'read;
                }
                line = lines.next_line() => line,
            };
            match line {
                Ok(Some(line)) => {
                    let events = parser.push(&line);
                    if events.is_empty() && !line.trim().is_empty() {
                        last_plain_line = Some(line.trim().to_string());
                    }
                    for event in events {
                        if tx.send(event).await.is_err() {
                            // 受信側が閉じた(キャンセル等)。読み手が居なくなるので
                            // killしないとパイプが詰まってwait()が返らなくなる
                            let _ = child.start_kill();
                            break 'read;
                        }
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    // 不正UTF-8等。以降は読めないのでkillして終わらせる
                    let _ = child.start_kill();
                    read_error = Some(e);
                    break;
                }
            }
        }

        let status = child.wait().await?;
        let stderr_text = stderr_task.await.unwrap_or_default();
        if let Some(e) = read_error {
            return Err(e.into());
        }
        if let Some(task) = stdin_task {
            match task.await {
                // CLIがstdinを読まずに終了した場合(BrokenPipe)は無視してよい
                Ok(Err(e)) if e.kind() != std::io::ErrorKind::BrokenPipe => return Err(e.into()),
                _ => {}
            }
        }
        if !status.success() {
            let code = status
                .code()
                .map(|c| c.to_string())
                .unwrap_or_else(|| "signal".to_string());
            // 理由はstderr優先。空ならstream-jsonでなかった最後のstdout行(利用上限等)
            let stderr_detail = stderr_text.trim();
            let detail = if stderr_detail.is_empty() {
                last_plain_line.as_deref().unwrap_or("")
            } else {
                stderr_detail
            };
            let message = if detail.is_empty() {
                format!("claude CLIが異常終了しました (exit {code})")
            } else {
                format!("claude CLIが異常終了しました (exit {code}): {detail}")
            };
            let _ = tx.send(AgentEvent::Error { message }).await;
        } else if !stderr_text.trim().is_empty() {
            // 正常終了時の警告はイベントにせず標準エラーへ転記するだけ(捨てはしない)
            eprintln!("claude CLI stderr: {}", stderr_text.trim());
        }
        Ok(())
    }

    /// claude CLIを検出する。
    ///
    /// パス指定があればそれのみを試す。無指定なら[`Self::default_candidates`]を順に試す
    /// (PATH上の`claude` → 既知のインストール先)。
    pub async fn detect(executable: Option<PathBuf>) -> Result<DetectResult> {
        match executable {
            Some(path) => Self::detect_one(path).await,
            None => Self::detect_from(&Self::default_candidates()).await,
        }
    }

    /// 検出候補を順に試し、最初に成功したものを返す。
    pub async fn detect_from(candidates: &[PathBuf]) -> Result<DetectResult> {
        let mut last_error = None;
        for candidate in candidates {
            match Self::detect_one(candidate.clone()).await {
                Ok(found) => return Ok(found),
                Err(e) => last_error = Some(e),
            }
        }
        Err(last_error.unwrap_or_else(|| AgentError::NotFound("検出候補がありません".to_string())))
    }

    /// claude CLIの検出候補(先頭から順に試す)。
    ///
    /// macOSのGUI起動アプリはPATHが最小構成(`/usr/bin:/bin:/usr/sbin:/sbin`)のため、
    /// PATH解決に失敗しても既知のインストール先を直接見に行く。
    pub fn default_candidates() -> Vec<PathBuf> {
        let mut candidates = vec![PathBuf::from("claude")];
        if let Some(home) = std::env::var_os("HOME") {
            candidates.push(PathBuf::from(home).join(".local/bin/claude"));
        }
        candidates.push(PathBuf::from("/usr/local/bin/claude"));
        candidates.push(PathBuf::from("/opt/homebrew/bin/claude"));
        candidates
    }

    /// 単一のパスに対して`claude --version`を実行する。
    async fn detect_one(path: PathBuf) -> Result<DetectResult> {
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

impl AgentBackend for ClaudeCodeCliBackend {
    fn run_turn<'a>(
        &'a self,
        request: TurnRequest<'a>,
        tx: mpsc::Sender<AgentEvent>,
    ) -> TurnFuture<'a> {
        // CLIは会話の文脈をセッション(`--resume`)で持つので`history`は使わない
        Box::pin(async move {
            self.send_with_context(request.prompt, request.session, request.system_prompt, tx)
                .await
        })
    }
}

/// MCP設定を渡すための一時ファイル(dropで削除)。
///
/// claude CLIの`--mcp-config`とCopilot CLIの`--additional-mcp-config`で共用する。
pub(crate) struct TempMcpConfig {
    path: PathBuf,
}

impl TempMcpConfig {
    pub(crate) fn create(contents: &str) -> Result<Self> {
        let path = std::env::temp_dir().join(format!("madake-mcp-{}.json", uuid::Uuid::new_v4()));
        std::fs::write(&path, contents)?;
        Ok(Self { path })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempMcpConfig {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
