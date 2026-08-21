//! アプリ設定(`~/.madakecad/settings.json`)の保存・読込テスト。
//!
//! ユーザーの実ファイルには一切触れない(常に一時ディレクトリを使う)。

use std::path::{Path, PathBuf};

use madake_agent::settings::{
    load_settings, save_settings, settings_path, AppSettings, SETTINGS_PATH_ENV,
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
