//! 前の起動から残ったエージェントCLIの書き込みを断る仕組み (インスタンスidヘッダ) のテスト。

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use madake_core::{Engine, Project};
use madake_mcp::SharedDoc;
use tower::ServiceExt;

fn setup() -> (std::sync::Arc<madake_agent::AgentManager>, Router) {
    let doc = SharedDoc::new(Engine::new(Project::new("インスタンス試験")));
    let agent = madake_mcp::agent::manager(&doc, 9310);
    let parts = madake_mcp::open_parts(&std::env::temp_dir().join(format!(
        "madake-parts-instance-{}.sqlite",
        uuid::Uuid::new_v4()
    )))
    .expect("parts db");
    let router = madake_mcp::router(doc, agent.clone(), parts);
    (agent, router)
}

async fn post_mcp(router: Router, instance: Option<&str>) -> (StatusCode, String) {
    let mut req = Request::post("/mcp")
        .header("host", "127.0.0.1:9310")
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream");
    if let Some(id) = instance {
        req = req.header(madake_agent::INSTANCE_HEADER, id);
    }
    let body = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}"#;
    let res = router
        .oneshot(req.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let bytes = http_body_util::BodyExt::collect(res.into_body())
        .await
        .unwrap()
        .to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

/// A request carrying another app run's instance id is refused before it reaches the drawing, so an agent CLI left over from a crashed or restarted app cannot edit the new session.
/// 別の起動のインスタンスidを持つリクエストは図面に届く前に断られるので、落ちた・再起動したアプリから残ったエージェントCLIは新しいセッションを編集できない。
#[tokio::test]
async fn a_request_from_an_earlier_app_run_is_refused() {
    let (_agent, router) = setup();
    let (status, body) = post_mcp(router, Some("stale-run")).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert!(body.contains("起動"), "{body}");
}

/// The agent's own requests (this run's id) and external clients without the header are served as before.
/// この起動のidを持つエージェント自身のリクエストと、ヘッダの無い外部クライアントは今までどおり通る。
#[tokio::test]
async fn this_run_and_external_clients_are_served() {
    let (agent, router) = setup();
    let id = agent.instance_id().to_string();
    let (status, body) = post_mcp(router.clone(), Some(&id)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = post_mcp(router, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

/// Every app run gets its own instance id.
/// アプリを起動するたびに別のインスタンスidになる。
#[test]
fn each_app_run_gets_its_own_instance_id() {
    let (a, _) = setup();
    let (b, _) = setup();
    assert!(!a.instance_id().is_empty());
    assert_ne!(a.instance_id(), b.instance_id());
}
