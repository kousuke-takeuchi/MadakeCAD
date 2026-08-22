//! APIキーの保管。**OSキーチェーンだけに置き、設定ファイル・ログ・イベントには出さない。**
//!
//! - macOS: キーチェーン / Windows: 資格情報マネージャ / Linux: Secret Service
//!   (いずれも`keyring`クレート経由)
//! - 保管先は`service = "MadakeCAD"`、`account = "anthropic_api_key"`
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
static KEY_CACHE: Mutex<Option<CachedKey>> = Mutex::new(None);

fn invalidate_cache() {
    *lock(&KEY_CACHE) = None;
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

/// Anthropic APIキーを取り出す。
///
/// キーチェーンに無ければ環境変数[`ANTHROPIC_KEY_ENV`]を見る(CI・開発用の逃げ道)。
/// [`install_store`]で保管先を差し替えている間は環境変数を見ない(テストが
/// 実行環境の環境変数に左右されないように)。
/// キーチェーンが使えない環境でもエラーにせず`None`を返す(チャットは
/// 「キーが未設定です」の案内へ落ちるだけで、アプリは止まらない)。
pub fn anthropic_api_key() -> Option<String> {
    anthropic_api_key_checked().0
}

/// [`anthropic_api_key`]の理由つき版。`(キー, キーチェーンが使えなかった理由)`。
///
/// 理由は設定画面に出す(「保存したのにキー未設定と出る」を黙って放置しないため)。
/// **理由の文言にキーは含まれない。**
pub fn anthropic_api_key_checked() -> (Option<String>, Option<String>) {
    if let Some(cached) = lock(&KEY_CACHE).clone() {
        return cached;
    }
    let resolved = read_anthropic_api_key();
    *lock(&KEY_CACHE) = Some(resolved.clone());
    resolved
}

/// キャッシュを介さずに実際の保管先から読む。
fn read_anthropic_api_key() -> (Option<String>, Option<String>) {
    let overridden = overridden();
    let store = overridden
        .clone()
        .unwrap_or_else(|| Arc::new(KeyringStore));
    let (stored, error) = match store.get(ANTHROPIC_ACCOUNT) {
        Ok(found) => (
            found.map(|k| k.trim().to_string()).filter(|k| !k.is_empty()),
            None,
        ),
        Err(e) => (None, Some(e.to_string())),
    };
    if stored.is_some() || overridden.is_some() {
        return (stored, error);
    }
    let from_env = std::env::var(ANTHROPIC_KEY_ENV)
        .ok()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty());
    (from_env, error)
}

/// Anthropic APIキーを保管する。前後の空白は取り除く。空文字は拒否する。
pub fn set_anthropic_api_key(key: &str) -> Result<()> {
    let key = key.trim();
    if key.is_empty() {
        return Err(AgentError::EmptyApiKey);
    }
    store().set(ANTHROPIC_ACCOUNT, key)?;
    // 書いた値をそのまま覚える(直後の読み出しで許可ダイアログを出さないため)
    *lock(&KEY_CACHE) = Some((Some(key.to_string()), None));
    Ok(())
}

/// Anthropic APIキーを消す(元から無くても成功)。
pub fn clear_anthropic_api_key() -> Result<()> {
    store().delete(ANTHROPIC_ACCOUNT)?;
    *lock(&KEY_CACHE) = Some((None, None));
    Ok(())
}

/// キーが保管済みか(UIの「保存済み」表示用)。**値そのものは返さない。**
pub fn has_anthropic_api_key() -> bool {
    anthropic_api_key().is_some()
}
