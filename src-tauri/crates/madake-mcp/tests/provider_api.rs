//! Link APIのプロバイダ設定エンドポイント(`/api/v1/agent/provider` ほか)のテスト。
//!
//! 設定画面がAPIキーを扱う唯一の経路なので、**キーが画面へ返らない**ことと、
//! 「保存済み」表示・削除・接続テストの応答の形をここで固定する。
//!
//! キーチェーンは差し替えたメモリ保管を使う(CIやヘッドレス環境でも走るように)。

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use madake_agent::secrets::{self, MemoryStore};
use madake_core::{Engine, Project};
use madake_mcp::SharedDoc;
use serde_json::{json, Value};
use tower::ServiceExt;

/// 設定の保存先を一時ファイルへ逃がす(ユーザーの`~/.madakecad/settings.json`を壊さない)。
fn redirect_settings_file() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let path = std::env::temp_dir().join(format!(
            "madake-provider-settings-{}.json",
            std::process::id()
        ));
        std::env::set_var(madake_agent::settings::SETTINGS_PATH_ENV, path);
    });
}

fn setup() -> Router {
    redirect_settings_file();
    let doc = SharedDoc::new(Engine::new(Project::new("プロバイダ設定テスト")));
    let agent = madake_mcp::agent::manager(&doc, 9310);
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let parts = madake_mcp::open_parts(&std::env::temp_dir().join(format!(
        "madake-parts-provider-{}-{}.sqlite",
        std::process::id(),
        seq
    )))
    .expect("parts db");
    madake_mcp::link_api::router(doc, agent, parts)
}

async fn call(router: &Router, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .body(match &body {
            Some(v) => Body::from(v.to_string()),
            None => Body::empty(),
        })
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// The settings screen learns which provider is selected and whether a key is saved, but never the key itself.
/// 設定画面は「どのプロバイダか」「キーが保存済みか」を知るが、キーそのものは受け取らない。
#[tokio::test]
async fn the_settings_screen_never_receives_the_api_key() {
    const KEY: &str = "sk-ant-never-leaves-the-keychain";
    let _store = secrets::install_store(Arc::new(MemoryStore::default()));
    let router = setup();

    let (status, before) = call(&router, "GET", "/api/v1/agent/provider", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(before["provider"], "claude_cli");
    assert_eq!(before["api_model"], "claude-sonnet-5");
    assert_eq!(before["api_key_saved"], false);

    let (status, after) = call(
        &router,
        "PUT",
        "/api/v1/agent/api-key",
        Some(json!({ "key": KEY })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(after["api_key_saved"], true);
    assert!(
        !after.to_string().contains(KEY),
        "応答にAPIキーが載っている: {after}"
    );

    let (_, status_now) = call(&router, "GET", "/api/v1/agent/provider", None).await;
    assert_eq!(status_now["api_key_saved"], true);
    assert!(
        !status_now.to_string().contains(KEY),
        "状態の取得でAPIキーが漏れている: {status_now}"
    );

    secrets::clear_anthropic_api_key().unwrap();
}

/// Removing the saved key flips the "saved" flag back, so the settings screen shows it is gone.
/// 保存済みのキーを削除すると「保存済み」表示が戻る(設定画面から消えたことが分かる)。
#[tokio::test]
async fn removing_the_saved_key_flips_the_saved_flag_back() {
    let _store = secrets::install_store(Arc::new(MemoryStore::default()));
    let router = setup();

    call(
        &router,
        "PUT",
        "/api/v1/agent/api-key",
        Some(json!({ "key": "sk-ant-temporary" })),
    )
    .await;
    let (status, after) = call(&router, "DELETE", "/api/v1/agent/api-key", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(after["api_key_saved"], false);
}

/// An empty key box is refused with an error instead of storing a useless entry.
/// キー欄が空のまま保存しようとするとエラーになる(役に立たない空の登録を作らない)。
#[tokio::test]
async fn an_empty_key_box_is_refused() {
    let _store = secrets::install_store(Arc::new(MemoryStore::default()));
    let router = setup();

    let (status, _) = call(
        &router,
        "PUT",
        "/api/v1/agent/api-key",
        Some(json!({ "key": "  " })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, provider) = call(&router, "GET", "/api/v1/agent/provider", None).await;
    assert_eq!(provider["api_key_saved"], false);
}

/// The connection test answers with a readable reason instead of failing the request itself.
/// 接続テストはリクエスト自体を失敗させず、読める理由を答えとして返す。
#[tokio::test]
async fn the_connection_test_answers_with_a_readable_reason() {
    let _store = secrets::install_store(Arc::new(MemoryStore::default()));
    let router = setup();

    let (status, result) = call(&router, "POST", "/api/v1/agent/test-connection", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["ok"], false);
    assert_eq!(result["error_kind"], "no_key", "理由の種類が違う: {result}");
    let error = result["error"].as_str().unwrap_or_default();
    assert!(error.contains("APIキー"), "理由が読めない: {error}");
}

/// Choosing the Anthropic API through the settings endpoint is reflected in the provider status.
/// 設定エンドポイントでAnthropic APIを選ぶと、プロバイダの状態にも反映される。
#[tokio::test]
async fn choosing_the_anthropic_api_is_reflected_in_the_provider_status() {
    let _store = secrets::install_store(Arc::new(MemoryStore::default()));
    let router = setup();

    let (_, settings) = call(&router, "GET", "/api/v1/settings", None).await;
    let mut settings = settings;
    settings["provider"] = json!("anthropic_api");
    settings["api_model"] = json!("claude-opus-4-6");
    let (status, saved) = call(&router, "PUT", "/api/v1/settings", Some(settings)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved["provider"], "anthropic_api");

    let (_, provider) = call(&router, "GET", "/api/v1/agent/provider", None).await;
    assert_eq!(provider["provider"], "anthropic_api");
    assert_eq!(provider["api_model"], "claude-opus-4-6");
}

/// 遅い保管先(OSの許可待ちで返ってこないキーチェーンの再現)。
struct SlowStore;

impl secrets::SecretStore for SlowStore {
    fn get(&self, _: &str) -> madake_agent::Result<Option<String>> {
        std::thread::sleep(madake_mcp::agent::KEYCHAIN_TIMEOUT * 3);
        Ok(Some("sk-ant-too-late".into()))
    }
    fn set(&self, _: &str, _: &str) -> madake_agent::Result<()> {
        Ok(())
    }
    fn delete(&self, _: &str) -> madake_agent::Result<()> {
        Ok(())
    }
}

/// If the OS keychain does not answer, the settings screen still opens and says why.
/// OSキーチェーンが返事をしないときも設定画面は開き、理由を表示する(固まらない)。
#[tokio::test]
async fn a_keychain_that_never_answers_does_not_freeze_the_settings_screen() {
    let _store = secrets::install_store(Arc::new(SlowStore));
    let router = setup();

    let started = std::time::Instant::now();
    let (status, provider) = call(&router, "GET", "/api/v1/agent/provider", None).await;

    assert_eq!(status, StatusCode::OK);
    assert!(
        started.elapsed() < madake_mcp::agent::KEYCHAIN_TIMEOUT * 2,
        "待ち続けている(設定画面が固まる)"
    );
    assert_eq!(provider["api_key_saved"], false);
    let reason = provider["keychain_error"].as_str().unwrap_or_default();
    assert!(
        reason.contains("キーチェーン"),
        "理由が伝わっていない: {reason}"
    );
}
