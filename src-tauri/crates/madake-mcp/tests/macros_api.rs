//! 回路マクロのLink API (`POST /macros/save` / `GET /macros` / `POST /macros/apply`) のテスト。
//!
//! UI(Tauri IPC)・MCPツール(`save_macro`/`list_macros`/`apply_macro`)・Link APIは
//! 同じ`SharedDoc`のメソッドを通るので、どの入口から挿入しても1回の編集(undo一発)になる。

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

/// このテストプロセス専用のマクロ置き場 (ユーザーの`~/MadakeCAD/macros`を汚さない)。
fn macros_dir() -> PathBuf {
    static DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("madake-macros-api-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var(madake_core::macros::USER_MACROS_PATH_ENV, &dir);
        dir
    })
    .clone()
}

fn setup() -> (SharedDoc, Router) {
    macros_dir();
    let doc = SharedDoc::new(Engine::new(Project::new("テストプロジェクト")));
    let agent = madake_mcp::agent::manager(&doc, 9310);
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let parts = madake_mcp::open_parts(&std::env::temp_dir().join(format!(
        "madake-parts-macros-{}-{}.sqlite",
        std::process::id(),
        seq
    )))
    .expect("parts db");
    let router = madake_mcp::link_api::router(doc.clone(), Arc::clone(&agent), parts);
    (doc, router)
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

/// 2つのシンボル(リレーコイルとランプ)を図面へ置き、そのidを返す。
async fn place_two_symbols(router: &Router, sheet_id: Uuid) -> Vec<Uuid> {
    let ids = vec![Uuid::new_v4(), Uuid::new_v4()];
    let commands = json!([
        { "type": "add_entity", "sheet_id": sheet_id, "entity": {
            "kind": "symbol", "id": ids[0], "symbol_id": "relay_coil",
            "at": { "x": 100.0, "y": 100.0 }, "reference": "K1" } },
        { "type": "add_entity", "sheet_id": sheet_id, "entity": {
            "kind": "symbol", "id": ids[1], "symbol_id": "lamp",
            "at": { "x": 100.0, "y": 130.0 }, "reference": "L1" } }
    ]);
    let (status, _) = post(router, "/api/v1/commands", commands).await;
    assert_eq!(status, StatusCode::OK);
    ids
}

/// POST /macros/save turns the selected entities into a macro file in the user's macros folder, and GET /macros lists it with its base point.
/// POST /macros/save は選択したエンティティをユーザーのマクロフォルダのファイルにし、GET /macros がそれを基準点つきで一覧に返す。
#[tokio::test]
async fn saving_a_selection_over_the_link_api_stores_a_macro_in_the_user_folder() {
    let (doc, router) = setup();
    let sheet_id = doc.engine.lock().unwrap().project().sheets[0].id;
    let ids = place_two_symbols(&router, sheet_id).await;

    let (status, body) = post(
        &router,
        "/api/v1/macros/save",
        json!({
            "sheet_id": sheet_id,
            "entity_ids": ids,
            "id": "api_saved",
            "name": "API saved",
            "name_ja": "API保存",
            "category": "control"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["macro"]["id"], "api_saved");
    assert_eq!(body["macro"]["base_point"]["x"], 92.5);
    assert_eq!(body["macro"]["base_point"]["y"], 130.0);
    assert!(
        body["path"].as_str().is_some_and(|p| p.ends_with("api_saved.json")),
        "保存先のパスを返す {body}"
    );

    let (status, list) = get(&router, "/api/v1/macros").await;
    assert_eq!(status, StatusCode::OK);
    let macros = list["macros"].as_array().expect("macros配列");
    let mine = macros
        .iter()
        .find(|m| m["id"] == "api_saved")
        .expect("保存したマクロが並ぶ");
    assert_eq!(mine["name_ja"], "API保存");
    assert_eq!(mine["category"], "control");
    assert_eq!(mine["commands"].as_array().expect("commands").len(), 2);
    assert!(list["user_dir"].as_str().is_some(), "置き場も返す");
}

/// POST /macros/apply drops the macro at the requested point as a single edit that one undo takes back, renumbering its reference designators so they do not clash.
/// POST /macros/apply はマクロを指定位置へ1回の編集として入れ(undo一発で戻る)、参照記号は衝突しないよう振り直される。
#[tokio::test]
async fn applying_a_macro_over_the_link_api_is_one_undo_step() {
    let (doc, router) = setup();
    let sheet_id = doc.engine.lock().unwrap().project().sheets[0].id;
    let ids = place_two_symbols(&router, sheet_id).await;
    let (status, _) = post(
        &router,
        "/api/v1/macros/save",
        json!({ "sheet_id": sheet_id, "entity_ids": ids, "id": "api_applied", "name": "API applied" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, patch) = post(
        &router,
        "/api/v1/macros/apply",
        json!({ "id": "api_applied", "sheet_id": sheet_id, "at": { "x": 250.0, "y": 130.0 } }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(patch["ops"].as_array().expect("ops").len(), 2);

    let references: Vec<String> = {
        let engine = doc.engine.lock().unwrap();
        engine.project().sheets[0]
            .entities
            .values()
            .filter_map(|e| match e {
                madake_core::Entity::Symbol(s) => Some(s.reference.clone()),
                _ => None,
            })
            .collect()
    };
    assert_eq!(references.len(), 4);
    assert!(references.contains(&"K2".to_string()), "{references:?}");
    assert!(references.contains(&"L2".to_string()), "{references:?}");

    let (status, _) = post(&router, "/api/v1/undo", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        doc.engine.lock().unwrap().project().sheets[0].entities.len(),
        2,
        "undo一発でマクロ分だけが消える"
    );
}

/// A macro saved with placeholders and value sets can be inserted over the Link API with one of them chosen, and the values land on the drawing in the same single edit.
/// プレースホルダと値セットを付けて保存したマクロは、Link APIから値セットを選んで挿入でき、値も同じ1回の編集で図面へ入る。
#[tokio::test]
async fn applying_a_macro_with_a_value_set_over_the_link_api_fills_in_the_values() {
    let (doc, router) = setup();
    let sheet_id = doc.engine.lock().unwrap().project().sheets[0].id;
    let ids = place_two_symbols(&router, sheet_id).await;

    let (status, body) = post(
        &router,
        "/api/v1/macros/save",
        json!({
            "sheet_id": sheet_id,
            "entity_ids": ids,
            "id": "api_value_sets",
            "name": "API value sets",
            "placeholders": [
                { "key": "rating", "label": "Rating", "label_ja": "定格",
                  "targets": [
                    { "entity": ids[0], "field": "value" },
                    { "entity": ids[1], "field": "attrs.rating" }
                  ] }
            ],
            "value_sets": [
                { "id": "0.75kw", "label": "0.75 kW", "label_ja": "0.75kW",
                  "values": { "rating": "0.75kW" } },
                { "id": "1.5kw", "label": "1.5 kW", "label_ja": "1.5kW",
                  "values": { "rating": "1.5kW" } }
            ]
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, list) = get(&router, "/api/v1/macros").await;
    assert_eq!(status, StatusCode::OK);
    let mine = list["macros"]
        .as_array()
        .expect("macros配列")
        .iter()
        .find(|m| m["id"] == "api_value_sets")
        .expect("保存したマクロ");
    assert_eq!(
        mine["value_sets"].as_array().expect("value_sets").len(),
        2,
        "挿入UIのドロップダウン用に値セットも返す"
    );

    let (status, patch) = post(
        &router,
        "/api/v1/macros/apply",
        json!({
            "id": "api_value_sets",
            "value_set": "1.5kw",
            "sheet_id": sheet_id,
            "at": { "x": 250.0, "y": 130.0 }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{patch}");

    let filled: Vec<(String, String, Option<String>)> = {
        let engine = doc.engine.lock().unwrap();
        engine.project().sheets[0]
            .entities
            .values()
            .filter_map(|e| match e {
                madake_core::Entity::Symbol(s) if s.at.x > 200.0 => Some((
                    s.symbol_id.clone(),
                    s.value.clone(),
                    s.attrs.get("rating").cloned(),
                )),
                _ => None,
            })
            .collect()
    };
    assert!(
        filled.contains(&("relay_coil".into(), "1.5kW".into(), None)),
        "{filled:?}"
    );
    assert!(
        filled.contains(&("lamp".into(), String::new(), Some("1.5kW".into()))),
        "{filled:?}"
    );

    let (status, _) = post(&router, "/api/v1/undo", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        doc.engine.lock().unwrap().project().sheets[0].entities.len(),
        2,
        "値セット込みでもundo一発"
    );

    let (status, body) = post(
        &router,
        "/api/v1/macros/apply",
        json!({
            "id": "api_value_sets",
            "value_set": "2.2kw",
            "sheet_id": sheet_id,
            "at": { "x": 250.0, "y": 130.0 }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(
        doc.engine.lock().unwrap().project().sheets[0].entities.len(),
        2,
        "知らない値セットでは何も置かれない"
    );
}

/// POST /macros/build turns the selection into a macro without writing any file, which is what the save dialog previews and what Cmd+C keeps in memory.
/// POST /macros/build は選択範囲をファイルに書かずにマクロへ組み立てる(保存ダイアログのプレビューと⌘Cの無名マクロが使う)。
#[tokio::test]
async fn building_a_macro_does_not_write_a_file() {
    let (doc, router) = setup();
    let sheet_id = doc.engine.lock().unwrap().project().sheets[0].id;
    let ids = place_two_symbols(&router, sheet_id).await;

    let (status, m) = post(
        &router,
        "/api/v1/macros/build",
        json!({ "sheet_id": sheet_id, "entity_ids": ids, "name": "" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{m}");
    assert_eq!(m["commands"].as_array().expect("commands").len(), 2);
    assert_eq!(m["base_point"]["y"], 130.0);

    let (status, list) = get(&router, "/api/v1/macros").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !list["macros"]
            .as_array()
            .expect("macros配列")
            .iter()
            .any(|m| m["name"] == ""),
        "組み立てただけのマクロは一覧に出ない(ファイルを作っていない)"
    );
}

/// POST /macros/apply-inline drops a macro handed over by value (the Cmd+C clipboard) as a single edit, renumbering its reference designators just like a stored macro.
/// POST /macros/apply-inline は値で渡したマクロ(⌘Cのクリップボード)を1回の編集として入れ、参照記号も保存済みマクロと同じように振り直す。
#[tokio::test]
async fn applying_an_inline_macro_behaves_like_a_stored_one() {
    let (doc, router) = setup();
    let sheet_id = doc.engine.lock().unwrap().project().sheets[0].id;
    let ids = place_two_symbols(&router, sheet_id).await;
    let (_, m) = post(
        &router,
        "/api/v1/macros/build",
        json!({ "sheet_id": sheet_id, "entity_ids": ids, "name": "" }),
    )
    .await;

    let (status, patch) = post(
        &router,
        "/api/v1/macros/apply-inline",
        json!({ "macro": m, "sheet_id": sheet_id, "at": { "x": 250.0, "y": 130.0 } }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{patch}");
    assert_eq!(patch["ops"].as_array().expect("ops").len(), 2);

    let references: Vec<String> = {
        let engine = doc.engine.lock().unwrap();
        engine.project().sheets[0]
            .entities
            .values()
            .filter_map(|e| match e {
                madake_core::Entity::Symbol(s) => Some(s.reference.clone()),
                _ => None,
            })
            .collect()
    };
    assert!(references.contains(&"K2".to_string()), "{references:?}");
    assert!(references.contains(&"L2".to_string()), "{references:?}");

    let (status, _) = post(&router, "/api/v1/undo", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        doc.engine.lock().unwrap().project().sheets[0].entities.len(),
        2,
        "undo一発で貼り付けた分だけが消える"
    );
}

/// An unknown macro id, an unknown variant key and an empty selection are all refused with 400 and leave the drawing untouched.
/// 知らないマクロid・知らないバリアントキー・空の選択はいずれも400で拒否され、図面は変わらない。
#[tokio::test]
async fn the_link_api_refuses_unknown_macros_variants_and_empty_selections() {
    let (doc, router) = setup();
    let sheet_id = doc.engine.lock().unwrap().project().sheets[0].id;
    let ids = place_two_symbols(&router, sheet_id).await;
    post(
        &router,
        "/api/v1/macros/save",
        json!({ "sheet_id": sheet_id, "entity_ids": ids, "id": "api_refused", "name": "API refused" }),
    )
    .await;

    let (status, _) = post(
        &router,
        "/api/v1/macros/apply",
        json!({ "id": "no_such_macro", "at": { "x": 0.0, "y": 0.0 } }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = post(
        &router,
        "/api/v1/macros/apply",
        json!({ "id": "api_refused", "variant": "Z", "at": { "x": 0.0, "y": 0.0 } }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = post(
        &router,
        "/api/v1/macros/save",
        json!({ "sheet_id": sheet_id, "entity_ids": [], "name": "empty" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    assert_eq!(
        doc.engine.lock().unwrap().project().sheets[0].entities.len(),
        2,
        "図面は変わらない"
    );
}
