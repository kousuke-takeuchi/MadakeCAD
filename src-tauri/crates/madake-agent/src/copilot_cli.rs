//! GitHub Copilot CLI(`copilot`)をサブプロセスとして駆動するバックエンド。
//!
//! [`ClaudeCodeCliBackend`](crate::ClaudeCodeCliBackend)と同じ形
//! ([`AgentBackend`])で1ターンを回す。違いはCopilot CLI固有の事情:
//!
//! - **認証はCopilot自身のもの**を使う(GitHubのOAuthサインイン、または
//!   `COPILOT_GITHUB_TOKEN`等の環境変数)。MadakeCADはトークンを一切持たず、
//!   設定ファイルにも書かない
//! - **システムプロンプト用のフラグが無い**ため、MadakeCADの作図ルールは
//!   [`CopilotCliBackend::compose_prompt`]でプロンプトの先頭へ前置する
//!   (`--no-custom-instructions`でユーザーのAGENTS.md等は読ませない)
//! - 出力は`--output-format json`(1行1JSON)。行の形は公開仕様が無いため
//!   [`crate::copilot_events`]が寛容に変換する
//! - 会話の継続はCopilotのセッション。初回は`--session-id <uuid>`でIDを決め打ちし、
//!   2ターン目以降は`--resume <uuid>`で同じセッションを再開する
//!
//! 図面の編集は内蔵MCPサーバー(`http://127.0.0.1:<port>/mcp`)のツール経由で、
//! 他のクライアントと同じCommandエンジンを通る。

use crate::anthropic::ConnectionError;
use crate::backend::{AgentBackend, DetectResult, TempMcpConfig, TurnFuture, TurnRequest};
use crate::copilot_events::CopilotParser;
use crate::{AgentError, AgentEvent, Result};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

/// 既定のCopilotモデル。`auto`はCopilotに選ばせる指定。
pub const DEFAULT_COPILOT_MODEL: &str = "auto";

/// 前置するシステム指示の見出し。
const SYSTEM_HEADING: &str = "# システム指示";
/// ユーザーの発言の見出し。
const USER_HEADING: &str = "# ユーザーの依頼";

/// 未認証と判断する手がかり(実機のcopilot 1.0.80が出す文面)。
const AUTH_MARKERS: &[&str] = &[
    "No authentication information found",
    "not authenticated",
    "run the '/login' command",
    "gh auth login",
];

/// 未認証のときにチャットへ出す案内。
pub const NOT_AUTHENTICATED_MESSAGE: &str = "GitHub Copilot CLIが未認証です。\
     ターミナルで`copilot`を起動し`/login`を実行してサインインしてください\
     (または`GITHUB_TOKEN`等の環境変数を設定してください)。";

/// 接続テストの失敗区分: 未認証。
pub const KIND_COPILOT_AUTH: &str = "copilot_auth";
/// 接続テストの失敗区分: CLIが見つからない。
pub const KIND_COPILOT_MISSING: &str = "copilot_missing";

/// ローカルのGitHub Copilot CLIをサブプロセスとして駆動するバックエンド。
#[derive(Debug, Clone)]
pub struct CopilotCliBackend {
    /// `copilot`実行ファイル(PATH上の名前でも絶対パスでも可)
    pub executable: PathBuf,
    /// `--model`へ渡すモデル(`auto`でCopilotが選ぶ。`None`ならフラグ自体を付けない)
    pub model: Option<String>,
    /// 内蔵MCPサーバーのポート
    pub mcp_port: u16,
    /// 既定で前置する図面コンテキスト+作図ルール
    pub system_prompt: Option<String>,
}

impl CopilotCliBackend {
    pub fn new(executable: PathBuf, mcp_port: u16) -> Self {
        Self {
            executable,
            model: Some(DEFAULT_COPILOT_MODEL.to_string()),
            mcp_port,
            system_prompt: None,
        }
    }

    /// `--additional-mcp-config`へ渡す一時ファイルの中身。
    ///
    /// 形は`~/.copilot/mcp-config.json`と同じ(`copilot mcp add --transport http`が
    /// 書き出す形を実機で確認済み)。
    pub fn mcp_config_json(&self) -> String {
        serde_json::json!({
            "mcpServers": {
                "madakecad": {
                    "type": "http",
                    "url": format!("http://127.0.0.1:{}/mcp", self.mcp_port),
                    "tools": ["*"],
                }
            }
        })
        .to_string()
    }

    /// システム指示をユーザーの発言の前へ置いた、CLIへ渡す1本のプロンプトを作る。
    ///
    /// Copilot CLIには`--append-system-prompt`相当のフラグが無いため、図面コンテキスト
    /// と作図ルールはここでプロンプトへ畳み込む。見出しで区切るのは、どこまでが
    /// MadakeCADからの指示でどこからがユーザーの発言かをモデルが取り違えないため。
    pub fn compose_prompt(system_prompt: Option<&str>, prompt: &str) -> String {
        match system_prompt.map(str::trim).filter(|s| !s.is_empty()) {
            Some(system) => {
                format!("{SYSTEM_HEADING}\n\n{system}\n\n{USER_HEADING}\n\n{prompt}")
            }
            None => prompt.to_string(),
        }
    }

    /// copilot CLIへ渡す引数列を組み立てる。
    ///
    /// - `session`: 継続ターンのセッションID(`Some`なら`--resume`で同じ会話を続ける)
    /// - `new_session_id`: 新規ターンで固定するUUID(`--session-id`)
    ///
    /// プロンプトは`-p`の値として渡す(Copilot CLIはstdinからプロンプトを読まない)。
    pub fn build_args(
        &self,
        prompt: &str,
        session: Option<&str>,
        new_session_id: &str,
        mcp_config_path: &Path,
    ) -> Vec<String> {
        let mut args = vec![
            "-p".to_string(),
            prompt.to_string(),
            "--output-format".to_string(),
            "json".to_string(),
            // 非対話モードではツールの自動許可が必須(無いと確認待ちで固まる)
            "--allow-all-tools".to_string(),
            // 質問されても答える相手がいない
            "--no-ask-user".to_string(),
            // ユーザーのAGENTS.md等でMadakeCADの作図ルールを乱されないようにする
            "--no-custom-instructions".to_string(),
            // 既定のgithub-mcp-serverは図面作成に不要
            "--disable-builtin-mcps".to_string(),
            "--additional-mcp-config".to_string(),
            format!("@{}", mcp_config_path.display()),
            // 進捗ログでJSONLを汚さない
            "--log-level".to_string(),
            "error".to_string(),
        ];
        match session {
            Some(session) => {
                args.push("--resume".to_string());
                args.push(session.to_string());
            }
            None => {
                args.push("--session-id".to_string());
                args.push(new_session_id.to_string());
            }
        }
        if let Some(model) = &self.model {
            args.push("--model".to_string());
            args.push(model.clone());
        }
        // 同梱ドキュメントはCLIの作業ディレクトリの外にあるため、読める場所として明示する
        if let Some(docs) = crate::knowledge::docs_dir() {
            args.push("--add-dir".to_string());
            args.push(docs.display().to_string());
        }
        args
    }

    /// 出力に未認証の手がかりが含まれるか。
    pub fn is_unauthenticated(text: &str) -> bool {
        AUTH_MARKERS.iter().any(|marker| text.contains(marker))
    }

    /// 1ターンを実行し、イベントを`tx`へ流す(プロセス終了まで待つ)。
    pub async fn send(
        &self,
        prompt: &str,
        session: Option<&str>,
        tx: mpsc::Sender<AgentEvent>,
    ) -> Result<()> {
        self.send_with_context(prompt, session, self.system_prompt.as_deref(), tx)
            .await
    }

    /// [`Self::send`]のシステム指示明示版(ターンごとに図面コンテキストが変わるため)。
    ///
    /// 失敗は種類を問わず必ず[`AgentEvent::Error`]として`tx`へ流れる。戻り値の`Err`は
    /// 「そのうえで異常終了した」ことを示す(CLIの非0終了はイベントのみでOkを返す)。
    pub async fn send_with_context(
        &self,
        prompt: &str,
        session: Option<&str>,
        system_prompt: Option<&str>,
        tx: mpsc::Sender<AgentEvent>,
    ) -> Result<()> {
        let error_tx = tx.clone();
        match self.stream_turn(prompt, session, system_prompt, tx).await {
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
        system_prompt: Option<&str>,
        tx: mpsc::Sender<AgentEvent>,
    ) -> Result<()> {
        let config = TempMcpConfig::create(&self.mcp_config_json())?;
        let new_session_id = uuid::Uuid::new_v4().to_string();
        let composed = Self::compose_prompt(system_prompt, prompt);
        let args = self.build_args(&composed, session, &new_session_id, config.path());

        let mut child = Command::new(&self.executable)
            .args(&args)
            // プロンプトは引数で渡すのでstdinは使わない。開いたままにすると
            // CLIが対話入力を待つ可能性があるため閉じておく
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| AgentError::Spawn(format!("{}: {e}", self.executable.display())))?;

        // セッションIDはこちらで決めているので、CLIの出力を待たずに通知する
        // (これが次のターンの`--resume`になる)
        if session.is_none()
            && tx
                .send(AgentEvent::SessionStarted {
                    session_id: new_session_id,
                })
                .await
                .is_err()
        {
            let _ = child.start_kill();
            let _ = child.wait().await;
            return Ok(());
        }

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

        let mut parser = CopilotParser::new();
        let mut lines = BufReader::new(stdout).lines();
        let mut read_error = None;
        // JSONLとして解釈できなかった最後のstdout行。CLIはクレジット切れなどの理由を
        // 素のテキストで出して非0終了することがあり、異常終了時の説明に使う
        let mut last_plain_line: Option<String> = None;
        'read: loop {
            // 受信側のcloseはsendの失敗だけでは検知できない(イベントにならない行が
            // 続く間はsendが呼ばれない)。行の待ち受けと同時に監視する
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
                            let _ = child.start_kill();
                            break 'read;
                        }
                    }
                }
                Ok(None) => break,
                Err(e) => {
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
        if !status.success() {
            let _ = tx
                .send(AgentEvent::Error {
                    message: failure_message(
                        status.code(),
                        stderr_text.trim(),
                        last_plain_line.as_deref().unwrap_or_default(),
                    ),
                })
                .await;
        } else if !stderr_text.trim().is_empty() {
            // 正常終了時の警告はイベントにせず標準エラーへ転記するだけ(捨てはしない)
            eprintln!("copilot CLI stderr: {}", stderr_text.trim());
        }
        Ok(())
    }

    /// 設定が使えるかを最小のプロンプトで確かめる(設定画面の「接続テスト」)。
    ///
    /// Copilotのサインイン状態はCLIを実際に走らせないと分からない
    /// (`--version`は未認証でも成功する)。そのため**ごく短い1往復**を投げて、
    /// 失敗の文面で「CLIが無い」「未認証」を切り分ける。AIクレジットを消費するので、
    /// 呼ぶのはユーザーがボタンを押したときだけにすること。
    pub async fn check_connection(&self) -> std::result::Result<(), ConnectionError> {
        let output = Command::new(&self.executable)
            .args([
                "-p",
                "Reply with the single word: pong",
                "--output-format",
                "json",
                "--allow-all-tools",
                "--no-ask-user",
                "--no-custom-instructions",
                "--disable-builtin-mcps",
                "--log-level",
                "error",
            ])
            .stdin(Stdio::null())
            .output()
            .await
            .map_err(|e| ConnectionError {
                kind: KIND_COPILOT_MISSING.to_string(),
                message: format!(
                    "GitHub Copilot CLIを起動できませんでした({}): {e}",
                    self.executable.display()
                ),
            })?;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if Self::is_unauthenticated(&text) {
            return Err(ConnectionError {
                kind: KIND_COPILOT_AUTH.to_string(),
                message: NOT_AUTHENTICATED_MESSAGE.to_string(),
            });
        }
        if !output.status.success() {
            return Err(ConnectionError {
                kind: KIND_COPILOT_MISSING.to_string(),
                message: format!(
                    "GitHub Copilot CLIが応答しませんでした({}): {}",
                    self.executable.display(),
                    text.trim()
                ),
            });
        }
        Ok(())
    }

    /// copilot CLIを検出する。パス指定があればそれのみを試す。
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

    /// copilot CLIの検出候補(先頭から順に試す)。
    ///
    /// macOSのGUI起動アプリはPATHが最小構成のため、PATH解決に失敗しても
    /// npm等の既知のインストール先を直接見に行く。
    pub fn default_candidates() -> Vec<PathBuf> {
        let mut candidates = vec![PathBuf::from("copilot")];
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            candidates.push(home.join(".local/bin/copilot"));
            candidates.push(home.join(".npm-global/bin/copilot"));
        }
        candidates.push(PathBuf::from("/usr/local/bin/copilot"));
        candidates.push(PathBuf::from("/opt/homebrew/bin/copilot"));
        candidates
    }

    /// 単一のパスに対して`copilot --version`を実行する。
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
        // 実機は「GitHub Copilot CLI 1.0.80.」+更新案内の複数行を出す。先頭行だけ使う
        let version = String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .to_string();
        Ok(DetectResult { path, version })
    }
}

/// 非0終了の説明文(未認証ならサインイン案内、それ以外は理由をそのまま添える)。
fn failure_message(code: Option<i32>, stderr: &str, last_plain_line: &str) -> String {
    if CopilotCliBackend::is_unauthenticated(stderr)
        || CopilotCliBackend::is_unauthenticated(last_plain_line)
    {
        return NOT_AUTHENTICATED_MESSAGE.to_string();
    }
    let code = code
        .map(|c| c.to_string())
        .unwrap_or_else(|| "signal".to_string());
    // 理由はstderr優先。空ならJSONLでなかった最後のstdout行(クレジット切れ等)
    let detail = if stderr.is_empty() {
        last_plain_line
    } else {
        stderr
    };
    if detail.is_empty() {
        format!("copilot CLIが異常終了しました (exit {code})")
    } else {
        format!("copilot CLIが異常終了しました (exit {code}): {detail}")
    }
}

impl AgentBackend for CopilotCliBackend {
    fn run_turn<'a>(
        &'a self,
        request: TurnRequest<'a>,
        tx: mpsc::Sender<AgentEvent>,
    ) -> TurnFuture<'a> {
        // 会話の文脈はCopilotのセッション(`--resume`)が持つので`history`は使わない
        Box::pin(async move {
            self.send_with_context(request.prompt, request.session, request.system_prompt, tx)
                .await
        })
    }
}
