//! APIキーの保管。**OSキーチェーンだけに置き、設定ファイル・ログ・イベントには出さない。**
//!
//! - macOS: キーチェーン / Windows: 資格情報マネージャ / Linux: Secret Service
//!   (いずれも`keyring`クレート経由)
//! - 保管先は`service = "MadakeCAD"`。`account`はプロバイダごとに分ける
//!   (Anthropic=`anthropic_api_key` / OpenAI互換=`openai_compat_api_key` /
//!   Gemini=`gemini_api_key`)
//!
//! テスト・CIでは実キーチェーンが使えない(ヘッドレスLinuxにはSecret Serviceが無い)ため、
//! [`install_store`]で[`MemoryStore`]へ差し替えられるようにしてある。差し替えは
//! プロセス全体に効くので、install_lockで1本ずつ順番に行う。
//!
//! **この模組のコードはキーを一切ログへ出さない。** 値を`Debug`表示しないこと。

use std::sync::{Arc, Mutex, MutexGuard};

use crate::{AgentError, Result};

/// キーチェーンのサービス名(アプリ名)。
pub const KEYCHAIN_SERVICE: &str = "MadakeCAD";
/// Anthropic APIキーの保管名。
pub const ANTHROPIC_ACCOUNT: &str = "anthropic_api_key";
/// キーチェーンに無いときに見る環境変数(CI・開発用の逃げ道)。
pub const ANTHROPIC_KEY_ENV: &str = "ANTHROPIC_API_KEY";
/// OpenAI互換API(OpenAI/xAI/OpenRouter/Ollama…)のキーの保管名。
///
/// Anthropicのキーとは**別の入れ物**なので、両方を保存しておいて設定画面で
/// プロバイダを切り替えるだけで行き来できる。
pub const OPENAI_COMPAT_ACCOUNT: &str = "openai_compat_api_key";
/// キーチェーンに無いときに見る環境変数(CI・開発用の逃げ道)。
pub const OPENAI_KEY_ENV: &str = "OPENAI_API_KEY";
/// Google Gemini APIのキーの保管名。
///
/// Anthropic・OpenAI互換のキーとは**別の入れ物**なので、全部を保存しておいて
/// 設定画面でプロバイダを切り替えるだけで行き来できる。
pub const GEMINI_ACCOUNT: &str = "gemini_api_key";
/// キーチェーンに無いときに見る環境変数(CI・開発用の逃げ道)。
pub const GEMINI_KEY_ENV: &str = "GEMINI_API_KEY";

/// 秘密の保管先。実体はOSキーチェーン([`KeyringStore`])、テストは[`MemoryStore`]。
pub trait SecretStore: Send + Sync + 'static {
    /// 保管された値。無ければ`Ok(None)`。
    fn get(&self, account: &str) -> Result<Option<String>>;
    /// 値を保管する(既存があれば上書き)。
    fn set(&self, account: &str, secret: &str) -> Result<()>;
    /// 値を消す。元から無い場合も成功扱い。
    fn delete(&self, account: &str) -> Result<()>;
}

/// OSキーチェーン(`keyring`クレート)による保管。
#[derive(Debug, Default, Clone, Copy)]
pub struct KeyringStore;

impl KeyringStore {
    fn entry(account: &str) -> Result<keyring::Entry> {
        keyring::Entry::new(KEYCHAIN_SERVICE, account)
            .map_err(|e| AgentError::Keychain(e.to_string()))
    }
}

impl SecretStore for KeyringStore {
    fn get(&self, account: &str) -> Result<Option<String>> {
        match Self::entry(account)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AgentError::Keychain(e.to_string())),
        }
    }

    fn set(&self, account: &str, secret: &str) -> Result<()> {
        Self::entry(account)?
            .set_password(secret)
            .map_err(|e| AgentError::Keychain(e.to_string()))
    }

    fn delete(&self, account: &str) -> Result<()> {
        match Self::entry(account)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AgentError::Keychain(e.to_string())),
        }
    }
}

/// プロセス内だけの保管(テスト用)。ディスクにも環境にも残らない。
#[derive(Debug, Default)]
pub struct MemoryStore {
    entries: Mutex<std::collections::HashMap<String, String>>,
}

impl SecretStore for MemoryStore {
    fn get(&self, account: &str) -> Result<Option<String>> {
        Ok(self.entries.lock().unwrap().get(account).cloned())
    }

    fn set(&self, account: &str, secret: &str) -> Result<()> {
        self.entries
            .lock()
            .unwrap()
            .insert(account.to_string(), secret.to_string());
        Ok(())
    }

    fn delete(&self, account: &str) -> Result<()> {
        self.entries.lock().unwrap().remove(account);
        Ok(())
    }
}

static OVERRIDE: Mutex<Option<Arc<dyn SecretStore>>> = Mutex::new(None);
static INSTALL_LOCK: Mutex<()> = Mutex::new(());

/// 読み出したキー(と失敗理由)のプロセス内キャッシュ。
///
/// **キーチェーンの読み出しは環境によってOSの許可ダイアログを出す**(macOSでは
/// アプリの署名が変わると再確認になる)。設定画面の状態取得のたびに読みに行くと
/// そのたびにダイアログが出て操作が止まるため、1プロセスにつき1回だけ読む。
/// 保存・削除・保管先の差し替えで無効化する。
type CachedKey = (Option<String>, Option<String>);
static KEY_CACHE: Mutex<Option<std::collections::HashMap<String, CachedKey>>> = Mutex::new(None);

fn invalidate_cache() {
    *lock(&KEY_CACHE) = None;
}

fn cached(account: &str) -> Option<CachedKey> {
    lock(&KEY_CACHE)
        .as_ref()
        .and_then(|cache| cache.get(account).cloned())
}

fn remember(account: &str, value: CachedKey) {
    lock(&KEY_CACHE)
        .get_or_insert_with(Default::default)
        .insert(account.to_string(), value);
}

/// 差し替えを元へ戻すガード。生きている間は他のテストが差し替えられない。
pub struct StoreGuard {
    _lock: MutexGuard<'static, ()>,
}

impl Drop for StoreGuard {
    fn drop(&mut self) {
        *lock(&OVERRIDE) = None;
        invalidate_cache();
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// 保管先を差し替える(テスト・代替実装用)。戻り値のガードを落とすとOSキーチェーンへ戻る。
pub fn install_store(store: Arc<dyn SecretStore>) -> StoreGuard {
    let guard = lock(&INSTALL_LOCK);
    *lock(&OVERRIDE) = Some(store);
    invalidate_cache();
    StoreGuard { _lock: guard }
}

/// 実体のOSキーチェーン保管(差し替えを無視して直接使う)。
pub fn keyring_store() -> Arc<dyn SecretStore> {
    Arc::new(KeyringStore)
}

/// 差し替え中の保管先(差し替えていなければ`None`)。
fn overridden() -> Option<Arc<dyn SecretStore>> {
    lock(&OVERRIDE).clone()
}

/// いま使う保管先(差し替えがあればそれ、無ければOSキーチェーン)。
pub fn store() -> Arc<dyn SecretStore> {
    overridden().unwrap_or_else(|| Arc::new(KeyringStore))
}

/// 保管名を指定してAPIキーを取り出す(理由つき)。`(キー, 読めなかった理由)`。
///
/// キーチェーンに無ければ環境変数`env`を見る(CI・開発用の逃げ道)。
/// [`install_store`]で保管先を差し替えている間は環境変数を見ない(テストが
/// 実行環境の環境変数に左右されないように)。
/// キーチェーンが使えない環境でもエラーにせず`None`を返す(チャットは
/// 「キーが未設定です」の案内へ落ちるだけで、アプリは止まらない)。
///
/// 読み出しは保管名ごとに1回だけ行い、以降はプロセス内キャッシュを返す。
pub fn api_key_checked(account: &str, env: Option<&str>) -> (Option<String>, Option<String>) {
    if let Some(cached) = cached(account) {
        return cached;
    }
    let resolved = read_api_key(account, env);
    remember(account, resolved.clone());
    resolved
}

/// 保管名を指定してAPIキーを取り出す。
pub fn api_key(account: &str, env: Option<&str>) -> Option<String> {
    api_key_checked(account, env).0
}

/// キャッシュを介さずに実際の保管先から読む。
fn read_api_key(account: &str, env: Option<&str>) -> (Option<String>, Option<String>) {
    let overridden = overridden();
    let store = overridden.clone().unwrap_or_else(|| Arc::new(KeyringStore));
    let (stored, error) = match store.get(account) {
        Ok(found) => (
            found
                .map(|k| k.trim().to_string())
                .filter(|k| !k.is_empty()),
            None,
        ),
        Err(e) => (None, Some(e.to_string())),
    };
    if stored.is_some() || overridden.is_some() {
        return (stored, error);
    }
    let from_env = env
        .and_then(|env| std::env::var(env).ok())
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty());
    (from_env, error)
}

/// 保管名を指定してAPIキーを保管する。前後の空白は取り除く。空文字は拒否する。
pub fn set_api_key(account: &str, key: &str) -> Result<()> {
    let key = key.trim();
    if key.is_empty() {
        return Err(AgentError::EmptyApiKey);
    }
    store().set(account, key)?;
    // 書いた値をそのまま覚える(直後の読み出しで許可ダイアログを出さないため)
    remember(account, (Some(key.to_string()), None));
    Ok(())
}

/// 保管名を指定してAPIキーを消す(元から無くても成功)。
pub fn clear_api_key(account: &str) -> Result<()> {
    store().delete(account)?;
    remember(account, (None, None));
    Ok(())
}

/// Anthropic APIキーを取り出す。
pub fn anthropic_api_key() -> Option<String> {
    anthropic_api_key_checked().0
}

/// [`anthropic_api_key`]の理由つき版。`(キー, キーチェーンが使えなかった理由)`。
///
/// 理由は設定画面に出す(「保存したのにキー未設定と出る」を黙って放置しないため)。
/// **理由の文言にキーは含まれない。**
pub fn anthropic_api_key_checked() -> (Option<String>, Option<String>) {
    api_key_checked(ANTHROPIC_ACCOUNT, Some(ANTHROPIC_KEY_ENV))
}

/// Anthropic APIキーを保管する。前後の空白は取り除く。空文字は拒否する。
pub fn set_anthropic_api_key(key: &str) -> Result<()> {
    set_api_key(ANTHROPIC_ACCOUNT, key)
}

/// Anthropic APIキーを消す(元から無くても成功)。
pub fn clear_anthropic_api_key() -> Result<()> {
    clear_api_key(ANTHROPIC_ACCOUNT)
}

/// キーが保管済みか(UIの「保存済み」表示用)。**値そのものは返さない。**
pub fn has_anthropic_api_key() -> bool {
    anthropic_api_key().is_some()
}

/// OpenAI互換APIのキーを取り出す。
///
/// ローカルのOllama等はキー不要なので、`None`でも送信は成立しうる
/// (呼び出し側が接続先URLで判断する)。
pub fn openai_compat_api_key() -> Option<String> {
    openai_compat_api_key_checked().0
}

/// [`openai_compat_api_key`]の理由つき版。`(キー, キーチェーンが使えなかった理由)`。
pub fn openai_compat_api_key_checked() -> (Option<String>, Option<String>) {
    api_key_checked(OPENAI_COMPAT_ACCOUNT, Some(OPENAI_KEY_ENV))
}

/// OpenAI互換APIのキーを保管する。前後の空白は取り除く。空文字は拒否する。
pub fn set_openai_compat_api_key(key: &str) -> Result<()> {
    set_api_key(OPENAI_COMPAT_ACCOUNT, key)
}

/// OpenAI互換APIのキーを消す(元から無くても成功)。
pub fn clear_openai_compat_api_key() -> Result<()> {
    clear_api_key(OPENAI_COMPAT_ACCOUNT)
}

/// OpenAI互換APIのキーが保管済みか。**値そのものは返さない。**
pub fn has_openai_compat_api_key() -> bool {
    openai_compat_api_key().is_some()
}

/// Gemini APIのキーを取り出す。
pub fn gemini_api_key() -> Option<String> {
    gemini_api_key_checked().0
}

/// [`gemini_api_key`]の理由つき版。`(キー, キーチェーンが使えなかった理由)`。
pub fn gemini_api_key_checked() -> (Option<String>, Option<String>) {
    api_key_checked(GEMINI_ACCOUNT, Some(GEMINI_KEY_ENV))
}

/// Gemini APIのキーを保管する。前後の空白は取り除く。空文字は拒否する。
pub fn set_gemini_api_key(key: &str) -> Result<()> {
    set_api_key(GEMINI_ACCOUNT, key)
}

/// Gemini APIのキーを消す(元から無くても成功)。
pub fn clear_gemini_api_key() -> Result<()> {
    clear_api_key(GEMINI_ACCOUNT)
}

/// Gemini APIのキーが保管済みか。**値そのものは返さない。**
pub fn has_gemini_api_key() -> bool {
    gemini_api_key().is_some()
}
