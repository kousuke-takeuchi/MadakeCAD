//! アプリ設定(`~/.madakecad/settings.json`)の読み書き。
//!
//! UI非依存。設定はエージェントの動作に効く:
//! - [`AppSettings::claude_path`][]: claude CLIの明示パス(検出候補より優先)
//! - [`AppSettings::auto_read_drawing`][]: 送信時に図面コンテキストを付けるか
//! - [`AppSettings::auto_apply`][]: A1では常にON扱い(保存のみ。確認モードはA2以降)
//!
//! 未知フィールドは無視し、欠けたフィールドは`serde(default)`で既定値になる
//! (古い設定ファイルをそのまま読めるようにするため)。

use crate::{write_atomic, AgentError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 設定ファイルの置き場所を上書きする環境変数(テスト・検証用)。
pub const SETTINGS_PATH_ENV: &str = "MADAKE_SETTINGS_PATH";

/// ホーム直下の設定ディレクトリ名。
const SETTINGS_DIR: &str = ".madakecad";
/// 設定ファイル名。
const SETTINGS_FILE: &str = "settings.json";

/// アプリ全体の設定(A1範囲)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    /// claude実行ファイルの明示パス。`None`なら自動検出(PATH→既知のインストール先)。
    pub claude_path: Option<PathBuf>,
    /// エージェントの編集を確認なしで適用する(A1は常にON。トグルは保存のみ)。
    pub auto_apply: bool,
    /// 送信のたびに図面コンテキストを`--append-system-prompt`で渡す。
    pub auto_read_drawing: bool,
    /// UI表示言語(BCP 47の言語タグ小文字。既定は`"en"`)。
    /// 未知の値はフロントエンド側で`en`へフォールバックする。
    pub language: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            claude_path: None,
            auto_apply: true,
            auto_read_drawing: true,
            language: "en".into(),
        }
    }
}

impl AppSettings {
    /// 入力の揺れを吸収する(空文字・空白だけのパスは「未指定」とみなす)。
    ///
    /// UIのテキスト欄は空文字を送ってくるため、保存前に必ず通すこと。
    pub fn normalized(mut self) -> Self {
        self.claude_path = self.claude_path.and_then(|path| match path.to_str() {
            Some(s) => {
                let trimmed = s.trim();
                (!trimmed.is_empty()).then(|| PathBuf::from(trimmed))
            }
            // 非UTF-8パスはそのまま使う(trimできないだけで有効なパス)
            None => Some(path),
        });
        let language = self.language.trim().to_ascii_lowercase();
        self.language = if language.is_empty() {
            "en".into()
        } else {
            language
        };
        self
    }
}

/// 設定ファイルのパス。`MADAKE_SETTINGS_PATH`があればそれを優先する。
///
/// ホームディレクトリが分からない環境では`None`(設定は既定値のまま動く)。
pub fn settings_path() -> Option<PathBuf> {
    if let Some(overridden) = std::env::var_os(SETTINGS_PATH_ENV) {
        if !overridden.is_empty() {
            return Some(PathBuf::from(overridden));
        }
    }
    home_dir().map(|home| home.join(SETTINGS_DIR).join(SETTINGS_FILE))
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

/// 設定を読み込む。ファイルが無ければ既定値(初回起動)。
pub fn load_settings(path: &Path) -> Result<AppSettings> {
    let json = match std::fs::read_to_string(path) {
        Ok(json) => json,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(AppSettings::default()),
        Err(e) => return Err(e.into()),
    };
    Ok(serde_json::from_str(&json)?)
}

/// 設定を整形JSONで保存する(一時ファイル→renameでアトミックに)。
///
/// 親ディレクトリ(`~/.madakecad`)が無ければ作る。
pub fn save_settings(path: &Path, settings: &AppSettings) -> Result<()> {
    if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    write_atomic(path, &serde_json::to_string_pretty(settings)?)
}

/// 既定の場所から設定を読む。
pub fn load_default_settings() -> Result<AppSettings> {
    let path = settings_path().ok_or(AgentError::NoSettingsPath)?;
    load_settings(&path)
}

/// 既定の場所へ設定を保存する。
pub fn save_default_settings(settings: &AppSettings) -> Result<()> {
    let path = settings_path().ok_or(AgentError::NoSettingsPath)?;
    save_settings(&path, settings)
}
