//! Link APIのエージェントエンドポイント(/api/v1/agent/*)のテスト。
//!
//! サーバーは立てず、axumのRouterへ直接リクエストを流す(ポート競合を避けるため)。
//! claudeは呼ばず、madake-agentのフェイクCLIフィクスチャを使う。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use madake_agent::AgentManager;
use madake_core::{Engine, Project};
use madake_mcp::SharedDoc;
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

fn fake_claude(script: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../madake-agent/tests/fixtures")
        .join(script)
}

fn setup(script: &str) -> (SharedDoc, Arc<AgentManager>, Router) {
    let doc = SharedDoc::new(Engine::new(Project::new("テストプロジェクト")));
    let agent = madake_mcp::agent::manager(&doc, 9310);
    agent.set_executable(Some(fake_claude(script)));
    // 並列実行するテストがDBファイルを共有すると、スキーマ初期化(版チェック→INSERT)が
    // 非アトミックなため UNIQUE constraint で落ちる。テストごとに一意のファイルを使う
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let parts = madake_mcp::open_parts(&std::env::temp_dir().join(format!(
        "madake-parts-agent-{}-{}.sqlite",
        std::process::id(),
        seq
    )))
    .expect("parts db");
    let router = madake_mcp::link_api::router(doc.clone(), Arc::clone(&agent), parts);
    (doc, agent, router)
}

async fn call(
    router: &Router,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    call_with_origin(router, method, path, body, None).await
}

async fn call_with_origin(
    router: &Router,
    method: &str,
    path: &str,
    body: Option<Value>,
    origin: Option<&str>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(origin) = origin {
        builder = builder.header("origin", origin);
    }
    let request = builder
        .body(match &body {
            Some(v) => Body::from(v.to_string()),
            None => Body::empty(),
        })
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("madake_link_api_test_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// ターンが終わる(送信中フラグが下りる)まで待つ。
async fn wait_idle(agent: &AgentManager, id: Uuid) {
    for _ in 0..200 {
        if !agent.is_sending(id) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("ターンが終わらない");
}

/// POST /agent/send runs an assistant turn and the conversation list reflects the new messages.
/// POST /agent/send はアシスタントのターンを実行し、会話一覧に新しいメッセージが反映される。
#[tokio::test]
async fn send_runs_a_turn_and_conversations_reflects_it() {
    let (_doc, agent, router) = setup("fake_claude.sh");

    let (status, body) = call(
        &router,
        "POST",
        "/api/v1/agent/send",
        Some(json!({ "prompt": "ヒューズを追加して" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let id: Uuid = serde_json::from_value(body["conversation_id"].clone()).expect("会話IDが返る");
    wait_idle(&agent, id).await;

    let (status, body) = call(&router, "GET", "/api/v1/agent/conversations", None).await;
    assert_eq!(status, StatusCode::OK);
    let conversations = body.as_array().expect("配列");
    assert_eq!(conversations.len(), 1);
    assert_eq!(conversations[0]["id"], json!(id));
    let messages = conversations[0]["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2, "user + assistant: {body}");
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[0]["text"], "ヒューズを追加して");
    assert_eq!(messages[1]["role"], "assistant");
    assert!(!messages[1]["tool_calls"].as_array().unwrap().is_empty());
}

/// Sending to a non-existent conversation id returns 400 instead of creating garbage.
/// 存在しない会話IDへの送信は400を返し、不正なデータを作らない。
#[tokio::test]
async fn send_to_unknown_conversation_is_400() {
    let (_doc, _agent, router) = setup("fake_claude.sh");
    let (status, _) = call(
        &router,
        "POST",
        "/api/v1/agent/send",
        Some(json!({ "conversation_id": Uuid::new_v4(), "prompt": "hi" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// Cancel and undo-turn endpoints take a stable turn id and reject turns with nothing to roll back.
/// キャンセルとターン巻き戻しのエンドポイントはターン安定IDを受け取り、戻せる編集が無いターンは拒否する。
#[tokio::test]
async fn cancel_and_undo_turn_respond() {
    let (_doc, agent, router) = setup("fake_claude.sh");

    let (_, body) = call(
        &router,
        "POST",
        "/api/v1/agent/send",
        Some(json!({ "prompt": "hi" })),
    )
    .await;
    let id: Uuid = serde_json::from_value(body["conversation_id"].clone()).unwrap();
    wait_idle(&agent, id).await;

    // 実行中でない会話のcancelは何もしない
    let (status, body) = call(
        &router,
        "POST",
        "/api/v1/agent/cancel",
        Some(json!({ "conversation_id": id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["cancelled"], json!(false));

    // ターンの指定はメッセージ添字ではなくターン安定ID
    let turn_id = agent.conversations()[0].last_turn().unwrap().turn_id;
    let (status, body) = call(
        &router,
        "POST",
        "/api/v1/agent/undo-turn",
        Some(json!({ "conversation_id": id, "turn_id": turn_id })),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "編集の無いターンは戻すものが無い: {body}"
    );

    let (status, _) = call(
        &router,
        "POST",
        "/api/v1/agent/undo-turn",
        Some(json!({ "conversation_id": id, "turn_id": Uuid::new_v4() })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "未知のターンIDは拒否");
}

/// ドキュメント状態(revision + undo深さ)のスナップショット。
fn doc_state(doc: &SharedDoc) -> madake_agent::DocState {
    let engine = doc.engine.lock().unwrap();
    madake_agent::DocState::new(engine.revision(), engine.undo_depth() as u64)
}

/// テキスト要素を1つ追加するコマンド。
fn add_text(doc: &SharedDoc, id: Uuid, label: &str) -> madake_core::Command {
    let sheet_id = doc.engine.lock().unwrap().project().sheets[0].id;
    madake_core::Command::AddEntity {
        sheet_id,
        entity: madake_core::Entity::Text(madake_core::TextEntity {
            id,
            at: madake_core::Point::new(10.0, 10.0),
            text: label.into(),
            height: 3.5,
            rotation: 0,
        }),
    }
}

/// 会話を1本用意し、ターンを開始する(戻り値は会話ID)。
fn begin_turn(agent: &AgentManager, doc: &SharedDoc, prompt: &str) -> Uuid {
    let mut conversation = madake_agent::Conversation::new();
    conversation.begin_turn(prompt, doc_state(doc));
    let id = conversation.id;
    agent.set_conversations(vec![conversation]);
    id
}

/// 進行中のターンを終了状態にする(マネージャが記録する処理と同じ)。
fn finish_turn(agent: &AgentManager, doc: &SharedDoc) {
    let mut conversations = agent.conversations();
    conversations[0]
        .current_turn_mut()
        .unwrap()
        .finish_turn(doc_state(doc));
    agent.set_conversations(conversations);
}

/// Undoing a turn rolls back exactly the agent's edits through the command engine, keeping a manual edit the user made during the turn.
/// ターンの巻き戻しはCommandエンジン経由でエージェントの編集だけを戻し、ターン中にユーザーが手で入れた編集は残す。
#[tokio::test]
async fn undo_turn_rolls_back_agent_edits_and_keeps_the_manual_edit() {
    let (doc, agent, _router) = setup("fake_claude.sh");
    let mut patches = doc.patches.subscribe();

    // ターン中にエージェントがMCP経由で2コマンド実行し、間にユーザーが手で1つ描いた状況
    let id = begin_turn(&agent, &doc, "2つ置いて");
    doc.begin_agent_turn();
    doc.execute(add_text(&doc, Uuid::new_v4(), "AI-1")).unwrap();
    let manual = Uuid::new_v4();
    doc.execute_user(add_text(&doc, manual, "手編集")).unwrap();
    doc.execute(add_text(&doc, Uuid::new_v4(), "AI-2")).unwrap();
    doc.end_agent_turn();
    finish_turn(&agent, &doc);
    let end = doc_state(&doc);
    assert_eq!(
        doc.engine.lock().unwrap().project().sheets[0]
            .entities
            .len(),
        3
    );

    let turn_id = agent.conversations()[0].last_turn().unwrap().turn_id;
    let revision = agent.undo_turn(id, turn_id).expect("巻き戻し成功");
    {
        let engine = doc.engine.lock().unwrap();
        let entities = &engine.project().sheets[0].entities;
        assert_eq!(entities.len(), 1, "エージェントの編集だけが戻る");
        assert!(entities.contains_key(&manual), "ユーザーの手編集は残る");
    }
    assert_eq!(revision, end.revision + 1, "巻き戻しもrevisionを進める");

    // patchが配信されている(3件の追加 + 巻き戻し1件)
    let mut count = 0;
    while patches.try_recv().is_ok() {
        count += 1;
    }
    assert_eq!(count, 4);

    // 巻き戻しは通常の編集として履歴に乗るので、undoで取り消せる
    doc.undo().unwrap().unwrap();
    assert_eq!(
        doc.engine.lock().unwrap().project().sheets[0]
            .entities
            .len(),
        3,
        "巻き戻しの取り消しでエージェントの編集が戻る"
    );
}

/// If a manual edit removed what the turn touched, the rollback is refused with a conflict error and the drawing is left untouched.
/// ターンが触った要素を手編集で消していた場合、巻き戻しは衝突エラーで拒否され、図面は一切変更されない。
#[tokio::test]
async fn undo_turn_refuses_a_conflicting_rollback() {
    let (doc, agent, _router) = setup("fake_claude.sh");

    // エージェントが置いた要素を編集し、そのあとユーザーが手で削除した
    let target = Uuid::new_v4();
    doc.execute_user(add_text(&doc, target, "元の文字")).unwrap();
    let id = begin_turn(&agent, &doc, "文字を直して");
    doc.begin_agent_turn();
    let sheet_id = doc.engine.lock().unwrap().project().sheets[0].id;
    doc.execute(madake_core::Command::UpdateEntity {
        sheet_id,
        entity: madake_core::Entity::Text(madake_core::TextEntity {
            id: target,
            at: madake_core::Point::new(10.0, 10.0),
            text: "AIが直した文字".into(),
            height: 3.5,
            rotation: 0,
        }),
    })
    .unwrap();
    doc.execute(add_text(&doc, Uuid::new_v4(), "AI-2")).unwrap();
    doc.end_agent_turn();
    finish_turn(&agent, &doc);
    doc.execute_user(madake_core::Command::DeleteEntities {
        sheet_id,
        ids: vec![target],
    })
    .unwrap();
    let before = doc_state(&doc);

    let turn_id = agent.conversations()[0].last_turn().unwrap().turn_id;
    let err = agent.undo_turn(id, turn_id).unwrap_err();
    assert!(
        matches!(err, madake_agent::AgentError::TurnConflict { .. }),
        "{err}"
    );
    assert_eq!(doc_state(&doc), before, "図面も履歴も変わらない");
    assert_eq!(
        doc.engine.lock().unwrap().project().sheets[0]
            .entities
            .len(),
        1,
        "戻せた分も戻さない(部分適用しない)"
    );
    assert!(
        agent.conversations()[0]
            .turn(turn_id)
            .unwrap()
            .has_edits(),
        "適用済みのまま残るので、衝突を解消してから再試行できる"
    );
}

/// GET /agent/events streams conversation events over SSE, in the same shape as the Tauri agent:event.
/// GET /agent/events は会話イベントをTauriのagent:eventと同じ形でSSE配信する。
#[tokio::test]
async fn events_endpoint_streams_agent_events() {
    let (_doc, _agent, router) = setup("fake_claude.sh");
    let request = Request::builder()
        .uri("/api/v1/agent/events")
        .body(Body::empty())
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("text/event-stream")
    );

    let (_, body) = call(
        &router,
        "POST",
        "/api/v1/agent/send",
        Some(json!({ "prompt": "hi" })),
    )
    .await;
    let id: Uuid = serde_json::from_value(body["conversation_id"].clone()).unwrap();

    // 最初のイベント(SessionStarted)がSSEフレームとして届く
    let mut stream = response.into_body().into_data_stream();
    let frame = tokio::time::timeout(Duration::from_secs(10), stream.frame())
        .await
        .expect("SSEが届かない")
        .expect("ストリームが閉じた")
        .expect("フレーム取得失敗");
    let text = String::from_utf8(frame.into_data().unwrap().to_vec()).unwrap();
    assert!(text.starts_with("event: agent\n"), "{text}");
    let data = text
        .lines()
        .find_map(|l| l.strip_prefix("data: "))
        .expect("dataが無い");
    let payload: Value = serde_json::from_str(data).unwrap();
    assert_eq!(payload["conversation_id"], json!(id));
    assert_eq!(payload["event"]["type"], "session_started");
}

/// Saving and loading a project carries the chat history alongside (.chat.json).
/// プロジェクトの保存・読込はチャット履歴(.chat.json)を一緒に運ぶ。
#[tokio::test]
async fn save_and_load_carry_the_chat_history() {
    let dir = temp_dir();
    let project_path = dir.join("plant.mdkproj");
    let (_doc, agent, router) = setup("fake_claude.sh");

    // 会話を1本作ってから保存 → 図面とチャット履歴の両方が書かれる
    let (_, body) = call(
        &router,
        "POST",
        "/api/v1/agent/send",
        Some(json!({ "prompt": "ヒューズを追加して" })),
    )
    .await;
    let id: Uuid = serde_json::from_value(body["conversation_id"].clone()).unwrap();
    wait_idle(&agent, id).await;

    let (status, body) = call(
        &router,
        "POST",
        "/api/v1/save",
        Some(json!({ "path": project_path.to_str().unwrap() })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let chat_path = madake_agent::chat_path_for(&project_path);
    assert!(chat_path.exists(), "チャット履歴も保存される");

    // 別の会話に差し替えてから読み込むと、保存時の会話へ戻る
    agent.set_conversations(vec![madake_agent::Conversation::new()]);
    assert_ne!(agent.conversations()[0].id, id);

    let (status, body) = call(
        &router,
        "POST",
        "/api/v1/load",
        Some(json!({ "path": project_path.to_str().unwrap() })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let conversations = agent.conversations();
    assert_eq!(conversations.len(), 1);
    assert_eq!(conversations[0].id, id, "chat.jsonの会話が復元される");
    assert_eq!(conversations[0].messages[0].text, "ヒューズを追加して");

    std::fs::remove_dir_all(&dir).ok();
}

/// Loading a project cancels any running turn first, so the agent never edits the wrong document.
/// プロジェクト読込は実行中のターンを先に中断し、エージェントが古い図面を編集し続けないようにする。
#[tokio::test]
async fn load_cancels_a_running_turn() {
    let dir = temp_dir();
    let project_path = dir.join("plant.mdkproj");
    {
        let (_doc, _agent, router) = setup("fake_claude.sh");
        let (status, _) = call(
            &router,
            "POST",
            "/api/v1/save",
            Some(json!({ "path": project_path.to_str().unwrap() })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    let (_doc, agent, router) = setup("fake_claude_flood.sh");
    let (_, body) = call(
        &router,
        "POST",
        "/api/v1/agent/send",
        Some(json!({ "prompt": "延々と出力するやつ" })),
    )
    .await;
    let id: Uuid = serde_json::from_value(body["conversation_id"].clone()).unwrap();
    for _ in 0..200 {
        if agent.is_sending(id) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(agent.is_sending(id), "ターンが走っている");

    let (status, body) = call(
        &router,
        "POST",
        "/api/v1/load",
        Some(json!({ "path": project_path.to_str().unwrap() })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!agent.is_sending(id), "読込で実行中ターンが中断される");

    std::fs::remove_dir_all(&dir).ok();
}

/// Requests from external web origins are rejected; local origins and non-browser clients (no Origin header) pass.
/// 外部Webオリジンからのリクエストは拒否され、ローカルオリジンとブラウザ以外(Originヘッダなし)は通る。
#[tokio::test]
async fn external_origins_are_rejected_but_local_and_originless_pass() {
    let (_doc, _agent, router) = setup("fake_claude.sh");

    for origin in [
        "https://evil.example",
        "http://evil.example:1420",
        "http://localhost.evil.example",
        "http://127.0.0.1.evil.example",
    ] {
        let (status, _) = call_with_origin(
            &router,
            "POST",
            "/api/v1/agent/send",
            Some(json!({ "prompt": "全部消して" })),
            Some(origin),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{origin} は拒否されること");
    }

    // Origin無し(curl・madake CLI・FreeCADアドオン)は従来どおり通る
    let (status, _) = call(&router, "GET", "/api/v1/project", None).await;
    assert_eq!(status, StatusCode::OK);

    // ローカルオリジン(vite開発サーバー・Tauri webview)は通る
    for origin in [
        "http://localhost:1420",
        "http://127.0.0.1:9310",
        "http://[::1]:1420",
        "tauri://localhost",
        "http://tauri.localhost",
    ] {
        let (status, _) =
            call_with_origin(&router, "GET", "/api/v1/project", None, Some(origin)).await;
        assert_eq!(status, StatusCode::OK, "{origin} は許可されること");
    }
}

/// The origin guard treats only loopback hosts (localhost/127.0.0.1) as local.
/// オリジンガードはループバックホスト(localhost/127.0.0.1)のみをローカル扱いする。
#[test]
fn local_origin_predicate_matches_only_loopback_hosts() {
    use madake_mcp::link_api::is_local_origin;

    for origin in [
        "http://localhost",
        "http://localhost:1420",
        "https://localhost:8443",
        "http://127.0.0.1:9310",
        "http://[::1]",
        "http://[::1]:1420",
        "tauri://localhost",
        "http://tauri.localhost",
    ] {
        assert!(is_local_origin(origin), "{origin}");
    }
    for origin in [
        "https://evil.example",
        "http://localhost.evil.example",
        "http://127.0.0.1.evil.example",
        "http://127.0.0.2:9310",
        "http://user@localhost",
        "http://localhost:1420/path",
        "null",
        "",
    ] {
        assert!(!is_local_origin(origin), "{origin}");
    }
}

/// The drawing context given to the agent summarizes the active sheet (name, nets, entity count).
/// エージェントへ渡す図面コンテキストはアクティブシートの要約(名前・ネット数・要素数)を含む。
#[test]
fn drawing_context_summarizes_the_active_sheet() {
    let doc = SharedDoc::new(Engine::new(Project::new("盤A")));
    let sheet = doc.engine.lock().unwrap().project().sheets[0].clone();
    let context = madake_mcp::agent::drawing_context(&doc);

    assert!(context.contains("盤A"), "{context}");
    assert!(
        context.contains(&format!("アクティブシート: {}", sheet.id)),
        "{context}"
    );
    assert!(context.contains("Sheet1"), "{context}");
    assert!(context.contains("ネット数"), "{context}");
    assert!(context.contains("2.5mm"), "{context}");
    assert!(context.contains("mcp__madakecad__*"), "{context}");
}

/// AI settings endpoints persist changes and apply them to the agent manager.
/// AI設定のエンドポイントは変更を永続化し、エージェントマネージャへ適用する。
#[test]
fn settings_endpoints_persist_and_apply() {
    let dir = temp_dir();
    let path = dir.join("settings.json");
    std::env::set_var(madake_agent::settings::SETTINGS_PATH_ENV, &path);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let (_doc, agent, router) = setup("fake_claude.sh");

        let (status, body) = call(&router, "GET", "/api/v1/settings", None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["auto_apply"], json!(true));
        assert_eq!(body["auto_read_drawing"], json!(true));
        assert_eq!(body["claude_path"], Value::Null);

        let (status, body) = call(
            &router,
            "PUT",
            "/api/v1/settings",
            Some(json!({
                // 空白は落として保存される(UIのテキスト欄対策)
                "claude_path": "  /opt/homebrew/bin/claude  ",
                "auto_apply": false,
                "auto_read_drawing": false,
            })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["claude_path"], json!("/opt/homebrew/bin/claude"));

        // マネージャへ反映済み(次の送信から有効)
        assert_eq!(
            agent.executable(),
            Some(std::path::PathBuf::from("/opt/homebrew/bin/claude"))
        );
        assert!(!agent.settings().auto_read_drawing);

        // ファイルにも書かれている
        let saved: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(saved["claude_path"], json!("/opt/homebrew/bin/claude"));
        assert_eq!(saved["auto_read_drawing"], json!(false));

        // GETは保存後の値を返す
        let (_, body) = call(&router, "GET", "/api/v1/settings", None).await;
        assert_eq!(body["auto_apply"], json!(false));
    });

    std::env::remove_var(madake_agent::settings::SETTINGS_PATH_ENV);
    std::fs::remove_dir_all(&dir).ok();
}
