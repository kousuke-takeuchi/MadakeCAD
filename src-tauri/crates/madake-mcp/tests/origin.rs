//! 編集origin(誰の編集か)の配線テスト。
//!
//! UI(Tauri IPC)・AIエージェント・外部クライアント(MCP/Link API/CLI)は同じ
//! Commandエンジンを共有する。どの入口を通ったかがundo履歴に残ることを確かめる
//! (ターン巻き戻しがユーザーの手編集を巻き込まないための土台)。

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use madake_agent::{AgentManager, DocBridge};
use madake_core::{Command, EditOrigin, Engine, Entity, Point, Project, TextEntity};
use madake_mcp::SharedDoc;
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

fn setup() -> (SharedDoc, Arc<AgentManager>, Router) {
    let doc = SharedDoc::new(Engine::new(Project::new("テストプロジェクト")));
    let agent = madake_mcp::agent::manager(&doc, 9310);
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let parts = madake_mcp::open_parts(&std::env::temp_dir().join(format!(
        "madake-parts-origin-{}-{}.sqlite",
        std::process::id(),
        seq
    )))
    .expect("parts db");
    let router = madake_mcp::link_api::router(doc.clone(), Arc::clone(&agent), parts);
    (doc, agent, router)
}

fn add_text(doc: &SharedDoc, label: &str) -> Command {
    let sheet_id = doc.engine.lock().unwrap().project().sheets[0].id;
    Command::AddEntity {
        sheet_id,
        entity: Entity::Text(TextEntity {
            id: Uuid::new_v4(),
            at: Point::new(10.0, 10.0),
            text: label.into(),
            height: 3.5,
            rotation: 0,
        }),
    }
}

fn origins(doc: &SharedDoc) -> Vec<EditOrigin> {
    doc.engine.lock().unwrap().history_origins()
}

/// Each entry point records who made the edit: the UI is a user edit, the agent's turn is an agent edit, and outside clients are mcp edits.
/// 編集は入口ごとに由来が残る: UIはユーザー編集、エージェントのターン中はエージェント編集、外部クライアントはmcp編集。
#[tokio::test]
async fn every_edit_path_records_who_made_the_change() {
    let (doc, _agent, router) = setup();

    // UI (Tauri IPCのexecute_command)
    doc.execute_user(add_text(&doc, "UI")).unwrap();

    // MCPツール: エージェントのターン実行中はエージェント編集
    doc.begin_agent_turn();
    assert_eq!(doc.mcp_origin(), EditOrigin::Agent);
    doc.execute(add_text(&doc, "AI")).unwrap();
    doc.end_agent_turn();

    // MCPツール: ターン外(外部のMCPクライアント)はmcp編集
    assert_eq!(doc.mcp_origin(), EditOrigin::Mcp);
    doc.execute(add_text(&doc, "MCP")).unwrap();

    // Link API (madake CLI・FreeCADアドオン・ブラウザ検証)
    let command = add_text(&doc, "REST");
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/commands")
        .header("content-type", "application/json")
        .body(Body::from(json!([command]).to_string()))
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let patches: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(patches.as_array().map(Vec::len), Some(1));

    assert_eq!(
        origins(&doc),
        vec![
            EditOrigin::User,
            EditOrigin::Agent,
            EditOrigin::Mcp,
            EditOrigin::Mcp,
        ]
    );
}

/// The agent-turn marker nests and always clears, so edits after the turn are user edits again.
/// ターン実行中の印は入れ子でも数えられ、必ず解除されるため、ターン後の編集はまたユーザー編集になる。
#[tokio::test]
async fn the_agent_turn_marker_nests_and_always_clears() {
    let (doc, _agent, _router) = setup();
    // 会話を2本同時に走らせた状況(入れ子)
    doc.begin_agent_turn();
    doc.begin_agent_turn();
    doc.end_agent_turn();
    assert_eq!(
        doc.mcp_origin(),
        EditOrigin::Agent,
        "もう1本走っている間はエージェント編集のまま"
    );
    doc.end_agent_turn();
    assert_eq!(doc.mcp_origin(), EditOrigin::Mcp);
    // 余分な解除で壊れない
    doc.end_agent_turn();
    assert_eq!(doc.mcp_origin(), EditOrigin::Mcp);
}

/// The bridge the agent manager uses reverts only agent edits and reports how many were rolled back.
/// エージェントマネージャが使う窓口はエージェント編集だけを巻き戻し、戻した件数を報告する。
#[tokio::test]
async fn the_agent_bridge_reverts_only_agent_edits() {
    let (doc, _agent, _router) = setup();
    let bridge = madake_mcp::agent::SharedDocBridge(doc.clone());

    let start = bridge.undo_depth();
    doc.begin_agent_turn();
    doc.execute(add_text(&doc, "AI-1")).unwrap();
    doc.execute_user(add_text(&doc, "手編集")).unwrap();
    doc.execute(add_text(&doc, "AI-2")).unwrap();
    doc.end_agent_turn();
    let end = bridge.undo_depth();

    let report = bridge.revert_agent_edits(start, end).expect("巻き戻し成功");
    assert_eq!(report.reverted, 2, "エージェント編集2件だけ");
    let engine = doc.engine.lock().unwrap();
    let entities = &engine.project().sheets[0].entities;
    assert_eq!(entities.len(), 1, "ユーザーの手編集だけが残る");
    assert!(entities
        .values()
        .any(|e| matches!(e, Entity::Text(t) if t.text == "手編集")));
}
