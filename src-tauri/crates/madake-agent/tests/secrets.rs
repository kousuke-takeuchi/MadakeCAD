//! APIキーの保管([`madake_agent::secrets`])のテスト。
//!
//! **キーは平文でファイルに残さない**のがこの機能の肝なので、保存後に設定ファイルへ
//! キー文字列が現れないことをテストで固定する。OSキーチェーンを実際に叩くテストは
//! 環境(CI・ヘッドレスLinux)によっては使えないため、既定では差し替え可能な
//! メモリ保管で確かめ、実キーチェーンは環境変数で明示的に有効化したときだけ走る。

use std::sync::Arc;

use madake_agent::secrets::{self, MemoryStore, ANTHROPIC_ACCOUNT, KEYCHAIN_SERVICE};
use madake_agent::settings::{save_settings, AppSettings};
use madake_agent::AgentProvider;
use uuid::Uuid;

/// テストの間だけメモリ保管に差し替える。テストは同一プロセスで並行に走るため、
/// 差し替えは1本ずつ順番に行う(`secrets::install_store`が返すガードで元へ戻る)。
fn with_memory_store<T>(body: impl FnOnce() -> T) -> T {
    let _guard = secrets::install_store(Arc::new(MemoryStore::default()));
    body()
}

/// A saved API key can be read back, and deleting it makes it gone.
/// 保存したAPIキーは読み戻せて、削除すると消える。
#[test]
fn a_saved_api_key_can_be_read_back_and_deleted() {
    with_memory_store(|| {
        assert_eq!(secrets::anthropic_api_key(), None, "最初は未設定のはず");
        secrets::set_anthropic_api_key("sk-ant-test-0001").unwrap();
        assert_eq!(
            secrets::anthropic_api_key().as_deref(),
            Some("sk-ant-test-0001")
        );
        assert!(secrets::has_anthropic_api_key());
        secrets::clear_anthropic_api_key().unwrap();
        assert_eq!(secrets::anthropic_api_key(), None);
        assert!(!secrets::has_anthropic_api_key());
    });
}

/// Deleting a key that was never stored is not an error, so the UI can always offer "remove".
/// 保存していないキーを削除してもエラーにしない(UIの「削除」がいつでも押せる)。
#[test]
fn deleting_a_key_that_was_never_stored_is_not_an_error() {
    with_memory_store(|| {
        assert!(secrets::clear_anthropic_api_key().is_ok());
    });
}

/// Blank input is refused instead of storing an empty key that would fail later with a confusing error.
/// 空の入力は保存せずに断る(空のキーを持って後から分かりにくい失敗をしないため)。
#[test]
fn a_blank_api_key_is_refused() {
    with_memory_store(|| {
        assert!(secrets::set_anthropic_api_key("   ").is_err());
        assert_eq!(secrets::anthropic_api_key(), None);
    });
}

/// Surrounding whitespace is trimmed, so a key pasted with a stray newline still works.
/// 前後の空白は取り除いて保存する(改行ごと貼り付けたキーでも使える)。
#[test]
fn a_pasted_key_is_trimmed_before_it_is_stored() {
    with_memory_store(|| {
        secrets::set_anthropic_api_key("  sk-ant-test-0002\n").unwrap();
        assert_eq!(
            secrets::anthropic_api_key().as_deref(),
            Some("sk-ant-test-0002")
        );
    });
}

/// The stored key never appears in the settings file, which stays free of any secret.
/// 保存したキーは設定ファイルに一切現れない(設定ファイルに秘密は書かない)。
#[test]
fn the_settings_file_never_contains_the_api_key() {
    with_memory_store(|| {
        const KEY: &str = "sk-ant-secret-must-not-be-written";
        secrets::set_anthropic_api_key(KEY).unwrap();

        let dir = std::env::temp_dir().join(format!("madake_secret_test_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let settings = AppSettings {
            provider: AgentProvider::AnthropicApi,
            api_model: "claude-sonnet-5".into(),
            ..AppSettings::default()
        };
        save_settings(&path, &settings).unwrap();

        let written = std::fs::read_to_string(&path).unwrap();
        assert!(
            !written.contains(KEY),
            "設定ファイルにAPIキーが書かれている: {written}"
        );
        assert!(
            !written.contains("api_key"),
            "設定ファイルにAPIキー用の項目がある: {written}"
        );
        std::fs::remove_dir_all(&dir).ok();
    });
}

/// A settings file that somehow carries an api key field loses it on the next save.
/// 何らかの理由でAPIキーが書かれた設定ファイルを読んでも、次の保存でその項目は消える。
#[test]
fn an_api_key_smuggled_into_the_settings_file_is_dropped_on_save() {
    let dir = std::env::temp_dir().join(format!("madake_secret_test_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");
    std::fs::write(
        &path,
        r#"{"language":"ja","anthropic_api_key":"sk-ant-leaked","api_key":"sk-ant-leaked"}"#,
    )
    .unwrap();

    let loaded = madake_agent::settings::load_settings(&path).unwrap();
    assert_eq!(loaded.language, "ja");
    save_settings(&path, &loaded).unwrap();

    let written = std::fs::read_to_string(&path).unwrap();
    assert!(
        !written.contains("sk-ant-leaked"),
        "保存し直してもキーが残っている: {written}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// The keychain entry is addressed by the app name and a fixed account, so the same key is found next launch.
/// キーチェーンの保管先はアプリ名と決まった名前で、次に起動しても同じキーが見つかる。
#[test]
fn the_keychain_entry_is_addressed_by_the_app_name() {
    assert_eq!(KEYCHAIN_SERVICE, "MadakeCAD");
    assert_eq!(ANTHROPIC_ACCOUNT, "anthropic_api_key");
}

/// Against the real OS keychain, a key round-trips and is removed again (opt-in; skipped by default).
/// 実際のOSキーチェーンでも、キーは保存して読み戻して削除できる(環境変数で明示的に有効化したときだけ実行)。
#[test]
fn the_real_os_keychain_round_trips_a_key() {
    if std::env::var_os("MADAKE_KEYCHAIN_TESTS").is_none() {
        eprintln!("MADAKE_KEYCHAIN_TESTS が未設定のため、実キーチェーンのテストは省略します");
        return;
    }
    let store = secrets::keyring_store();
    let account = format!("madake_test_{}", Uuid::new_v4());
    store.set(&account, "sk-ant-roundtrip").unwrap();
    assert_eq!(store.get(&account).unwrap().as_deref(), Some("sk-ant-roundtrip"));
    store.delete(&account).unwrap();
    assert_eq!(store.get(&account).unwrap(), None);
}

/// 保管先を数えるラッパ(読み出し回数を確かめるため)。
#[derive(Default)]
struct CountingStore {
    inner: MemoryStore,
    reads: std::sync::atomic::AtomicUsize,
}

impl secrets::SecretStore for CountingStore {
    fn get(&self, account: &str) -> madake_agent::Result<Option<String>> {
        self.reads
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.inner.get(account)
    }
    fn set(&self, account: &str, secret: &str) -> madake_agent::Result<()> {
        self.inner.set(account, secret)
    }
    fn delete(&self, account: &str) -> madake_agent::Result<()> {
        self.inner.delete(account)
    }
}

/// The keychain is read once per app run, so the OS does not ask for permission again and again.
/// キーチェーンを読むのはアプリ起動につき1回だけで、OSの許可確認が何度も出ることがない。
#[test]
fn the_keychain_is_read_only_once_per_app_run() {
    let store = Arc::new(CountingStore::default());
    let guard = secrets::install_store(Arc::clone(&store) as Arc<dyn secrets::SecretStore>);

    secrets::set_anthropic_api_key("sk-ant-cached").unwrap();
    for _ in 0..5 {
        assert_eq!(
            secrets::anthropic_api_key().as_deref(),
            Some("sk-ant-cached")
        );
    }
    // 保存した値をそのまま覚えているので、読み出しは一度も要らない
    assert_eq!(store.reads.load(std::sync::atomic::Ordering::SeqCst), 0);

    drop(guard);
}

/// When the OS keychain cannot be read, the reason is reported instead of a silent "no key".
/// OSキーチェーンが読めないときは、黙って「キー未設定」にせず理由を伝える。
#[test]
fn a_keychain_that_cannot_be_read_reports_the_reason() {
    struct BrokenStore;
    impl secrets::SecretStore for BrokenStore {
        fn get(&self, _: &str) -> madake_agent::Result<Option<String>> {
            Err(madake_agent::AgentError::Keychain("鍵束が閉じています".into()))
        }
        fn set(&self, _: &str, _: &str) -> madake_agent::Result<()> {
            Ok(())
        }
        fn delete(&self, _: &str) -> madake_agent::Result<()> {
            Ok(())
        }
    }

    let _guard = secrets::install_store(Arc::new(BrokenStore));
    let (key, reason) = secrets::anthropic_api_key_checked();
    assert_eq!(key, None);
    assert!(
        reason.unwrap_or_default().contains("鍵束"),
        "読めなかった理由が伝わっていない"
    );
}
