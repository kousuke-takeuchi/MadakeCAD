//! アプリ設定(`~/.madakecad/settings.json`)の保存・読込テスト。
//!
//! ユーザーの実ファイルには一切触れない(常に一時ディレクトリを使う)。

use std::path::{Path, PathBuf};

use madake_agent::settings::{
    load_settings, save_settings, settings_path, AgentProvider, AppSettings, SETTINGS_PATH_ENV,
};
use uuid::Uuid;

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("madake_settings_test_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Default AI settings enable auto-apply and auto-read-drawing.
/// AI設定の既定は自動適用と図面自動読み取りが有効。
#[test]
fn default_settings_are_auto_apply_and_auto_read() {
    let settings = AppSettings::default();
    assert_eq!(settings.claude_path, None);
    assert!(settings.auto_apply);
    assert!(settings.auto_read_drawing);
}

/// A missing settings file yields the defaults.
/// 設定ファイルが無ければ既定値になる。
#[test]
fn missing_file_yields_defaults() {
    let dir = temp_dir();
    let loaded = load_settings(&dir.join("settings.json")).unwrap();
    assert_eq!(loaded, AppSettings::default());
    std::fs::remove_dir_all(&dir).ok();
}

/// Saved settings load back identically.
/// 保存した設定は同一内容で読み戻せる。
#[test]
fn saved_settings_round_trip() {
    let dir = temp_dir();
    let path = dir.join("settings.json");
    let settings = AppSettings {
        claude_path: Some(PathBuf::from("/opt/homebrew/bin/claude")),
        auto_apply: false,
        auto_read_drawing: false,
        language: "ja".into(),
        knowledge_path: Some(PathBuf::from("/home/me/house-rules.md")),
        provider: AgentProvider::AnthropicApi,
        api_model: "claude-sonnet-5".into(),
    };

    save_settings(&path, &settings).unwrap();
    assert_eq!(load_settings(&path).unwrap(), settings);
    // 整形JSONで書く(手で開いて直せること)
    let json = std::fs::read_to_string(&path).unwrap();
    assert!(json.contains("\n  \"claude_path\""), "{json}");
    // 一時ファイルを残さない
    let leftovers: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.file_name()))
        .filter(|name| name.to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");

    std::fs::remove_dir_all(&dir).ok();
}

/// Unknown/missing fields in the settings file fall back to defaults (forward compatible).
/// 設定ファイルに無い項目は既定値へフォールバックする(前方互換)。
#[test]
fn missing_fields_fall_back_to_defaults() {
    let dir = temp_dir();
    let path = dir.join("settings.json");
    // 古い(または手書きの)設定: auto_apply / auto_read_drawing が無い
    std::fs::write(&path, r#"{"claude_path": "/usr/local/bin/claude"}"#).unwrap();

    let loaded = load_settings(&path).unwrap();
    assert_eq!(
        loaded.claude_path.as_deref(),
        Some(Path::new("/usr/local/bin/claude"))
    );
    assert!(loaded.auto_apply);
    assert!(loaded.auto_read_drawing);

    // 未知フィールドがあっても読める(前方互換)
    std::fs::write(&path, r#"{"future_flag": true, "auto_apply": false}"#).unwrap();
    let loaded = load_settings(&path).unwrap();
    assert!(!loaded.auto_apply);
    assert!(loaded.auto_read_drawing);
    assert_eq!(loaded.claude_path, None);

    std::fs::remove_dir_all(&dir).ok();
}

/// Saving creates the settings directory if needed.
/// 保存時に設定ディレクトリが無ければ作成される。
#[test]
fn save_creates_the_settings_directory() {
    let dir = temp_dir();
    let path = dir.join("nested/.madakecad/settings.json");
    save_settings(&path, &AppSettings::default()).unwrap();
    assert!(path.exists());
    std::fs::remove_dir_all(&dir).ok();
}

/// Blank executable paths are normalized away instead of being stored.
/// 空白の実行ファイルパスは保存されず正規化で除去される。
#[test]
fn normalized_drops_blank_paths() {
    let blank = AppSettings {
        claude_path: Some(PathBuf::from("   ")),
        ..AppSettings::default()
    };
    assert_eq!(blank.normalized().claude_path, None);

    let padded = AppSettings {
        claude_path: Some(PathBuf::from(" /usr/local/bin/claude ")),
        ..AppSettings::default()
    };
    assert_eq!(
        padded.normalized().claude_path.as_deref(),
        Some(Path::new("/usr/local/bin/claude"))
    );
}

/// The settings path honors its environment-variable override.
/// 設定ファイルパスは環境変数の上書きに従う。
#[test]
fn settings_path_honors_the_env_override() {
    let dir = temp_dir();
    let path = dir.join("settings.json");
    std::env::set_var(SETTINGS_PATH_ENV, &path);
    assert_eq!(settings_path(), Some(path));
    std::env::remove_var(SETTINGS_PATH_ENV);

    // 上書きが無ければ ~/.madakecad/settings.json (HOMEがある環境のみ)
    if let Some(home) = std::env::var_os("HOME") {
        assert_eq!(
            settings_path(),
            Some(PathBuf::from(home).join(".madakecad/settings.json"))
        );
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// The default UI language is English.
/// UIの既定言語は英語 (en)。
#[test]
fn default_language_is_english() {
    assert_eq!(AppSettings::default().language, "en");
}

/// A settings file saved before the language field existed loads with English.
/// 言語フィールド追加前に保存された設定ファイルは英語 (en) として読み込まれる。
#[test]
fn old_settings_file_without_language_loads_as_english() {
    let dir = temp_dir();
    let path = dir.join("settings.json");
    std::fs::write(&path, r#"{ "claude_path": null, "auto_apply": false }"#).unwrap();
    let settings = load_settings(&path).unwrap();
    assert_eq!(settings.language, "en");
    assert!(!settings.auto_apply);
    std::fs::remove_dir_all(&dir).ok();
}

/// Normalization lowercases the language tag and turns blank input into English.
/// 言語タグは正規化で小文字になり、空白だけの入力は英語 (en) に戻る。
#[test]
fn language_is_normalized_to_lowercase_and_blank_becomes_english() {
    let upper = AppSettings { language: " JA ".into(), ..AppSettings::default() };
    assert_eq!(upper.normalized().language, "ja");
    let blank = AppSettings { language: "   ".into(), ..AppSettings::default() };
    assert_eq!(blank.normalized().language, "en");
}

/// The language choice survives a save/load round trip.
/// 言語の選択は保存して読み直しても保持される。
#[test]
fn language_round_trips_through_save_and_load() {
    let dir = temp_dir();
    let path = dir.join("settings.json");
    let settings = AppSettings { language: "ja".into(), ..AppSettings::default() };
    save_settings(&path, &settings).unwrap();
    assert_eq!(load_settings(&path).unwrap().language, "ja");
    std::fs::remove_dir_all(&dir).ok();
}

/// By default no extra knowledge file is configured (only the bundled standards note is used).
/// 既定では追加の知識ファイルは未設定(同梱の規格ノートだけを使う)。
#[test]
fn default_knowledge_path_is_unset() {
    assert_eq!(AppSettings::default().knowledge_path, None);
}

/// A settings file saved before the knowledge-file field existed loads with it unset.
/// 知識ファイルの項目が無い旧い設定ファイルは、未設定として読み込まれる。
#[test]
fn old_settings_file_without_knowledge_path_loads_unset() {
    let dir = temp_dir();
    let path = dir.join("settings.json");
    std::fs::write(&path, r#"{ "claude_path": null, "language": "ja" }"#).unwrap();
    let settings = load_settings(&path).unwrap();
    assert_eq!(settings.knowledge_path, None);
    assert_eq!(settings.language, "ja");
    std::fs::remove_dir_all(&dir).ok();
}

/// A blank knowledge-file path is normalized away, and padding is trimmed.
/// 空白だけの知識ファイルパスは正規化で未設定になり、前後の空白は取り除かれる。
#[test]
fn normalized_drops_a_blank_knowledge_path() {
    let blank = AppSettings {
        knowledge_path: Some(PathBuf::from("   ")),
        ..AppSettings::default()
    };
    assert_eq!(blank.normalized().knowledge_path, None);

    let padded = AppSettings {
        knowledge_path: Some(PathBuf::from(" /home/me/house-rules.md ")),
        ..AppSettings::default()
    };
    assert_eq!(
        padded.normalized().knowledge_path.as_deref(),
        Some(Path::new("/home/me/house-rules.md"))
    );
}

// ------------------------------------------------ プロバイダ (Claude CLI / Anthropic API)

/// Out of the box the agent runs through the Claude Code CLI, with Claude Sonnet 5 ready for the API route.
/// 既定のエージェントはClaude Code CLI経由で、API経由に切り替えたときのモデルはClaude Sonnet 5。
#[test]
fn the_default_provider_is_the_claude_code_cli() {
    let settings = AppSettings::default();
    assert_eq!(settings.provider, AgentProvider::ClaudeCli);
    assert_eq!(settings.api_model, "claude-sonnet-5");
}

/// Choosing the Anthropic API survives a save/load round trip together with the model name.
/// Anthropic APIを選んだ設定は、モデル名と一緒に保存して読み直しても保持される。
#[test]
fn the_anthropic_api_choice_round_trips_through_save_and_load() {
    let dir = temp_dir();
    let path = dir.join("settings.json");
    let settings = AppSettings {
        provider: AgentProvider::AnthropicApi,
        api_model: "claude-opus-4-6".into(),
        ..AppSettings::default()
    };
    save_settings(&path, &settings).unwrap();
    let loaded = load_settings(&path).unwrap();
    assert_eq!(loaded.provider, AgentProvider::AnthropicApi);
    assert_eq!(loaded.api_model, "claude-opus-4-6");
    std::fs::remove_dir_all(&dir).ok();
}

/// The provider is stored under readable names, so the settings file stays hand-editable.
/// プロバイダは読める名前で保存される(設定ファイルを手で書き換えられる)。
#[test]
fn the_provider_is_stored_under_a_readable_name() {
    let dir = temp_dir();
    let path = dir.join("settings.json");
    save_settings(
        &path,
        &AppSettings {
            provider: AgentProvider::AnthropicApi,
            ..AppSettings::default()
        },
    )
    .unwrap();
    let written = std::fs::read_to_string(&path).unwrap();
    assert!(
        written.contains("\"provider\": \"anthropic_api\""),
        "読める名前で保存されていない: {written}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// A settings file written before providers existed keeps working and stays on the CLI.
/// プロバイダの項目が無い旧い設定ファイルもそのまま動き、Claude Code CLIのままになる。
#[test]
fn an_old_settings_file_without_a_provider_stays_on_the_cli() {
    let dir = temp_dir();
    let path = dir.join("settings.json");
    std::fs::write(&path, r#"{ "language": "ja", "auto_apply": true }"#).unwrap();
    let settings = load_settings(&path).unwrap();
    assert_eq!(settings.provider, AgentProvider::ClaudeCli);
    assert_eq!(settings.api_model, "claude-sonnet-5");
    std::fs::remove_dir_all(&dir).ok();
}

/// A provider name this build does not know falls back to the CLI instead of breaking the whole file.
/// このビルドが知らないプロバイダ名は、設定ファイル全体を読めなくせずにCLIへ戻す。
#[test]
fn an_unknown_provider_name_falls_back_to_the_cli() {
    let dir = temp_dir();
    let path = dir.join("settings.json");
    std::fs::write(
        &path,
        r#"{ "provider": "gemini", "language": "ja", "knowledge_path": "/tmp/notes.md" }"#,
    )
    .unwrap();
    let settings = load_settings(&path).unwrap();
    assert_eq!(settings.provider, AgentProvider::ClaudeCli);
    // 他の項目は失われない
    assert_eq!(settings.language, "ja");
    assert_eq!(
        settings.knowledge_path.as_deref(),
        Some(Path::new("/tmp/notes.md"))
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// A blank model box falls back to the default model, and padding is trimmed.
/// モデル名を空にすると既定のモデルへ戻り、前後の空白は取り除かれる。
#[test]
fn a_blank_api_model_falls_back_to_the_default() {
    let blank = AppSettings {
        api_model: "  ".into(),
        ..AppSettings::default()
    };
    assert_eq!(blank.normalized().api_model, "claude-sonnet-5");

    let padded = AppSettings {
        api_model: " claude-opus-4-6 ".into(),
        ..AppSettings::default()
    };
    assert_eq!(padded.normalized().api_model, "claude-opus-4-6");
}
