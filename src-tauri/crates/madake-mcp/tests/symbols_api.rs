//! ユーザーシンボルライブラリのLink API
//! (`GET /symbols` / `GET /symbols/user` / `POST /symbols/user/save` /
//! `POST /symbols/user/delete` / `POST /symbols/duplicate`) のテスト。
//!
//! シンボルライブラリは**ファイルのドメイン**で、図面を書き換えるCommandエンジンの
//! 外にある。保存・削除はundo履歴に乗らない代わりに、図面はシンボル定義の
//! スナップショットを持たず`symbol_id`で参照するだけなので、定義を直すと
//! **すでに置いてある部品の描画・ネットリストも次の描画から変わる**。

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use madake_core::{Engine, Project};
use madake_mcp::SharedDoc;
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

/// このテストプロセス専用のシンボル置き場 (ユーザーの`~/MadakeCAD/symbols`を汚さない)。
fn symbols_dir() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("madake-symbols-api-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var(madake_core::symbol_library::USER_SYMBOLS_PATH_ENV, &dir);
        dir
    })
    .clone()
}

/// シンボルの置き場はプロセス共通なので、テストは1つずつ直列に走らせる。
static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// テストごとにフォルダを空にしてから始める(前のテストのシンボルを持ち越さない)。
fn setup() -> (SharedDoc, Router, std::sync::MutexGuard<'static, ()>) {
    let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let dir = symbols_dir();
    for path in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        std::fs::remove_file(path.path()).ok();
    }
    let doc = SharedDoc::new(Engine::new(Project::new("テストプロジェクト")));
    let agent = madake_mcp::agent::manager(&doc, 9310);
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let parts = madake_mcp::open_parts(&std::env::temp_dir().join(format!(
        "madake-parts-symbols-{}-{}.sqlite",
        std::process::id(),
        seq
    )))
    .expect("parts db");
    let router = madake_mcp::link_api::router(doc.clone(), Arc::clone(&agent), parts);
    (doc, router, guard)
}

async fn get(router: &Router, uri: &str) -> (StatusCode, Value) {
    let request = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

async fn post(router: &Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// 保存できる最小のユーザーシンボル (2接続点の箱)。`pins`で接続点を差し替えられる。
fn symbol(id: &str, pins: Value) -> Value {
    json!({
        "id": id,
        "name": "My contactor",
        "name_ja": "自社コンタクタ",
        "category": "relay",
        "ref_prefix": "K",
        "keywords": ["contactor", "電磁接触器"],
        "primitives": [{
            "type": "rect",
            "p1": { "x": -5.0, "y": -5.0 },
            "p2": { "x": 5.0, "y": 5.0 },
            "filled": false
        }],
        "pins": pins,
        "text_slots": [{ "key": "TAG", "at": { "x": 0.0, "y": -7.5 }, "height": 2.5 }]
    })
}

fn pin(number: &str, x: f64, y: f64, dir: &str) -> Value {
    json!({ "number": number, "name": "", "at": { "x": x, "y": y }, "dir": dir })
}

fn two_pins() -> Value {
    json!([pin("1", -5.0, -2.5, "left"), pin("2", 5.0, -2.5, "right")])
}

/// シートに部品を1つ置き、そのエンティティidを返す。
async fn place(router: &Router, sheet_id: Uuid, symbol_id: &str, reference: &str) -> Uuid {
    let entity_id = Uuid::new_v4();
    let (status, _) = post(
        router,
        "/api/v1/commands",
        json!([{
            "type": "add_entity",
            "sheet_id": sheet_id,
            "entity": {
                "kind": "symbol",
                "id": entity_id,
                "symbol_id": symbol_id,
                "at": { "x": 100.0, "y": 100.0 },
                "reference": reference
            }
        }]),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    entity_id
}

fn first_sheet(doc: &SharedDoc) -> Uuid {
    doc.engine.lock().unwrap().project().sheets[0].id
}

/// 未接続ピンの指摘文 (「K1: 未接続のピン: 1, 2」)。図面が今どの定義で描かれているかが分かる。
async fn unconnected_pins(router: &Router) -> String {
    let (_status, diags) = get(router, "/api/v1/verify").await;
    diags
        .as_array()
        .expect("diagnostics")
        .iter()
        .find(|d| d["code"] == "erc.unconnected_pin")
        .map(|d| d["message"].as_str().unwrap_or_default().to_string())
        .unwrap_or_default()
}

/// GET /symbols lists the bundled library and the user's own symbols together, with the user symbols last and always in the "user" category.
/// GET /symbols は同梱ライブラリとユーザーが作ったシンボルをまとめて返す。ユーザーのシンボルは最後に並び、分類は常に「user」になる。
#[tokio::test]
async fn the_link_api_lists_bundled_and_user_symbols_together() {
    let (_doc, router, _guard) = setup();
    let (status, before) = get(&router, "/api/v1/symbols").await;
    assert_eq!(status, StatusCode::OK);
    let bundled = before.as_array().expect("symbols配列").len();

    let (status, _) = post(
        &router,
        "/api/v1/symbols/user/save",
        symbol("my_contactor", two_pins()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_status, after) = get(&router, "/api/v1/symbols").await;
    let symbols = after.as_array().expect("symbols配列");
    assert_eq!(symbols.len(), bundled + 1);
    let last = symbols.last().expect("最後");
    assert_eq!(last["id"], "my_contactor");
    assert_eq!(last["category"], "user", "定義にrelayと書いても分類はuser");
}

/// GET /symbols/user lists only the user's symbols and says where the folder is, so a client can offer "open the symbols folder".
/// GET /symbols/user はユーザーのシンボルだけを返し、置き場のパスも添える(クライアントが「シンボルのフォルダを開く」を出せるように)。
#[tokio::test]
async fn the_link_api_lists_the_user_symbols_with_their_folder() {
    let (_doc, router, _guard) = setup();
    post(
        &router,
        "/api/v1/symbols/user/save",
        symbol("my_contactor", two_pins()),
    )
    .await;
    let (status, body) = get(&router, "/api/v1/symbols/user").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["symbols"].as_array().expect("symbols").len(), 1);
    assert_eq!(body["symbols"][0]["id"], "my_contactor");
    assert!(body["issues"].as_array().expect("issues").is_empty());
    assert!(body["user_dir"].as_str().is_some_and(|s| s.contains("symbols")));
}

/// A user symbol saved over the Link API is immediately placeable on a sheet and its pins take part in the checks, exactly like a bundled symbol.
/// Link APIで保存したユーザーシンボルは、すぐに図面へ配置でき、そのピンも同梱シンボルと同じように検証の対象になる。
#[tokio::test]
async fn a_saved_user_symbol_can_be_placed_like_a_bundled_one() {
    let (doc, router, _guard) = setup();
    post(
        &router,
        "/api/v1/symbols/user/save",
        symbol("my_contactor", two_pins()),
    )
    .await;
    let sheet_id = first_sheet(&doc);
    place(&router, sheet_id, "my_contactor", "K1").await;
    assert_eq!(unconnected_pins(&router).await.as_str(), "K1: 未接続のピン: 1, 2");
}

/// Editing a user symbol changes the parts already placed in the drawing: the drawing keeps no snapshot of the definition, it only stores the symbol id.
/// ユーザーシンボルを編集すると、すでに図面に置いてある部品も変わる。図面はシンボル定義のスナップショットを持たず、シンボルidだけを持っているため。
#[tokio::test]
async fn editing_a_user_symbol_changes_the_parts_already_placed() {
    let (doc, router, _guard) = setup();
    post(
        &router,
        "/api/v1/symbols/user/save",
        symbol("my_contactor", two_pins()),
    )
    .await;
    let sheet_id = first_sheet(&doc);
    place(&router, sheet_id, "my_contactor", "K1").await;
    assert_eq!(unconnected_pins(&router).await.as_str(), "K1: 未接続のピン: 1, 2");

    // 同じidで3接続点へ作り替えて保存し直す
    let (status, _) = post(
        &router,
        "/api/v1/symbols/user/save",
        symbol(
            "my_contactor",
            json!([
                pin("1", -5.0, -2.5, "left"),
                pin("2", 5.0, -2.5, "right"),
                pin("3", 5.0, 2.5, "right")
            ]),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        unconnected_pins(&router).await.as_str(),
        "K1: 未接続のピン: 1, 2, 3",
        "置いてある部品が新しい定義で読み直される"
    );
}

/// Saving an invalid symbol answers with every reason at once and writes no file, so the library never holds a half-fixed definition.
/// おかしなシンボルの保存は理由を一度に全部返し、ファイルは書かない。直しかけの定義がライブラリに残らないようにするため。
#[tokio::test]
async fn saving_an_invalid_user_symbol_returns_every_reason_and_writes_nothing() {
    let (_doc, router, _guard) = setup();
    let mut def = symbol("bad_symbol", json!([pin("1", -5.1, -2.5, "left")]));
    def["primitives"] = json!([]);
    def["text_slots"] = json!([]);
    let (status, body) = post(&router, "/api/v1/symbols/user/save", def).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let codes: Vec<&str> = body["errors"]
        .as_array()
        .expect("errors配列")
        .iter()
        .map(|e| e["code"].as_str().unwrap_or_default())
        .collect();
    assert!(codes.contains(&"symbol.no_primitives"), "{codes:?}");
    assert!(codes.contains(&"symbol.pin_off_grid"), "{codes:?}");
    assert!(codes.contains(&"symbol.slot_missing"), "{codes:?}");

    let (_status, library) = get(&router, "/api/v1/symbols/user").await;
    assert!(library["symbols"].as_array().expect("symbols").is_empty());
}

/// A symbol whose id is already taken by a bundled symbol is refused, so dropping a file can never silently change the drawings that use that symbol.
/// 同梱シンボルと同じidのシンボルは保存を拒否する。ファイルを置いただけで、そのシンボルを使っている図面が黙って変わることが起きないようにするため。
#[tokio::test]
async fn a_user_symbol_may_not_take_a_bundled_symbol_id() {
    let (_doc, router, _guard) = setup();
    let (status, body) = post(
        &router,
        "/api/v1/symbols/user/save",
        symbol("relay_coil", two_pins()),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["errors"][0]["code"], "symbol.id_reserved");
}

/// The symbol library lives outside the undo history: undoing after a save takes back the last drawing edit, not the saved symbol.
/// シンボルライブラリはundo履歴の外にある。保存したあとにundoすると、戻るのは直前の図面の編集で、保存したシンボルは消えない。
#[tokio::test]
async fn the_symbol_library_is_outside_the_undo_history() {
    let (doc, router, _guard) = setup();
    post(
        &router,
        "/api/v1/symbols/user/save",
        symbol("my_contactor", two_pins()),
    )
    .await;
    let sheet_id = first_sheet(&doc);
    place(&router, sheet_id, "my_contactor", "K1").await;

    let (status, _) = post(&router, "/api/v1/undo", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        doc.engine.lock().unwrap().project().sheets[0].entities.is_empty(),
        "undoで戻るのは配置のほう"
    );
    let (_status, library) = get(&router, "/api/v1/symbols/user").await;
    assert_eq!(
        library["symbols"].as_array().expect("symbols").len(),
        1,
        "保存したシンボルはundoでは消えない"
    );
}

/// Deleting a user symbol that parts in the drawing still use is refused (and says how many), unless the caller insists with force.
/// まだ図面で使われているユーザーシンボルの削除は、使用数を添えて拒否される(強制指定したときだけ消える)。
#[tokio::test]
async fn deleting_a_user_symbol_in_use_is_refused_unless_forced() {
    let (doc, router, _guard) = setup();
    post(
        &router,
        "/api/v1/symbols/user/save",
        symbol("my_contactor", two_pins()),
    )
    .await;
    let sheet_id = first_sheet(&doc);
    place(&router, sheet_id, "my_contactor", "K1").await;

    let (status, _) = post(
        &router,
        "/api/v1/symbols/user/delete",
        json!({ "id": "my_contactor" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_status, library) = get(&router, "/api/v1/symbols/user").await;
    assert_eq!(library["symbols"].as_array().expect("symbols").len(), 1);

    let (status, body) = post(
        &router,
        "/api/v1/symbols/user/delete",
        json!({ "id": "my_contactor", "force": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["in_use"], 1);
    let (_status, library) = get(&router, "/api/v1/symbols/user").await;
    assert!(library["symbols"].as_array().expect("symbols").is_empty());
}

/// An unused user symbol is deleted right away.
/// 使われていないユーザーシンボルはそのまま削除できる。
#[tokio::test]
async fn deleting_an_unused_user_symbol_succeeds() {
    let (_doc, router, _guard) = setup();
    post(
        &router,
        "/api/v1/symbols/user/save",
        symbol("my_contactor", two_pins()),
    )
    .await;
    let (status, body) = post(
        &router,
        "/api/v1/symbols/user/delete",
        json!({ "id": "my_contactor" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["in_use"], 0);
    assert!(body["path"].as_str().is_some_and(|p| p.ends_with(".json")));
}

/// Duplicating a symbol returns a draft with a free id and a "copy" name; nothing is written until the draft is saved.
/// シンボルの複製は、空いているidと「コピー」の名前を持った下書きを返すだけで、保存するまでファイルは書かれない。
#[tokio::test]
async fn duplicating_a_symbol_returns_a_draft_that_is_not_saved_yet() {
    let (_doc, router, _guard) = setup();
    let (status, draft) = post(
        &router,
        "/api/v1/symbols/duplicate",
        json!({ "source_id": "relay_coil" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(draft["id"], "relay_coil_copy");
    assert_eq!(draft["category"], "user");
    assert!(draft["name_ja"].as_str().is_some_and(|n| n.ends_with("のコピー")));

    let (_status, library) = get(&router, "/api/v1/symbols/user").await;
    assert!(library["symbols"].as_array().expect("symbols").is_empty());

    let (status, _) = post(&router, "/api/v1/symbols/user/save", draft).await;
    assert_eq!(status, StatusCode::OK, "複製した下書きはそのまま保存できる");
}

/// Duplicating a symbol that does not exist answers with an error instead of an empty symbol.
/// 存在しないシンボルの複製は、空のシンボルではなくエラーを返す。
#[tokio::test]
async fn duplicating_an_unknown_symbol_is_an_error() {
    let (_doc, router, _guard) = setup();
    let (status, _) = post(
        &router,
        "/api/v1/symbols/duplicate",
        json!({ "source_id": "no_such_symbol" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// A broken file in the symbols folder never stops the listing: the good symbols still load and the broken file is reported with its path and reason.
/// シンボルのフォルダに壊れたファイルがあっても一覧は止まらない。正しいシンボルは読み込まれ、壊れたファイルはパスと理由つきで報告される。
#[tokio::test]
async fn a_broken_file_in_the_symbols_folder_is_reported_by_the_api() {
    let (_doc, router, _guard) = setup();
    post(
        &router,
        "/api/v1/symbols/user/save",
        symbol("my_contactor", two_pins()),
    )
    .await;
    std::fs::write(symbols_dir().join("aaa_broken.json"), "{ 壊れている").unwrap();

    let (_status, library) = get(&router, "/api/v1/symbols/user").await;
    assert_eq!(library["symbols"].as_array().expect("symbols").len(), 1);
    let issues = library["issues"].as_array().expect("issues");
    assert_eq!(issues.len(), 1);
    assert!(issues[0]["path"]
        .as_str()
        .is_some_and(|p| p.ends_with("aaa_broken.json")));
    assert!(!issues[0]["message"].as_str().unwrap_or_default().is_empty());
}
