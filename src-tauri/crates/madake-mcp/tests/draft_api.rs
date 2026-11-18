//! 機能シートの下書き (MCPツール `draft_function_sheet` / `check_function_sheets` と
//! Link API `POST /api/v1/draft/function-sheet` / `POST /api/v1/draft/check`) のテスト。

use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use madake_agent::tools::ToolBridge;
use madake_core::draft::DraftRequest;
use madake_core::{Engine, Project};
use madake_mcp::tool_bridge::McpToolBridge;
use madake_mcp::{tool_descriptions, SharedDoc};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

/// The invented source the core tests use: one unit (U2) is divided over two sheets by `pins`.
fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../madake-core/tests/fixtures/draft-source.mdkproj")
}

fn empty_project() -> Project {
    let mut project = Project::new("下書き試験");
    project.sheets.clear();
    project
}

fn fixture() -> (SharedDoc, McpToolBridge, Router) {
    let doc = SharedDoc::new(Engine::new(empty_project()));
    let parts = madake_mcp::open_parts(&std::env::temp_dir().join(format!("madake-draft-{}.sqlite", Uuid::new_v4())))
        .expect("部品DB");
    let bridge = McpToolBridge::new(doc.clone(), parts.clone());
    let agent = madake_mcp::agent::manager(&doc, 9310);
    let router = madake_mcp::link_api::router(doc.clone(), Arc::clone(&agent), parts);
    (doc, bridge, router)
}

async fn call(bridge: &McpToolBridge, name: &str, input: Value) -> Value {
    let outcome = bridge.call(name, input).await;
    assert!(!outcome.is_error, "{name} が失敗した: {}", outcome.content);
    serde_json::from_str(&outcome.content).expect("JSON")
}

async fn post(router: &Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let (status, text) = post_text(router, uri, body).await;
    (status, serde_json::from_str(&text).unwrap_or(Value::Null))
}

/// The reply body as text (an error body of the Link API is plain text).
async fn post_text(router: &Router, uri: &str, body: Value) -> (StatusCode, String) {
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&bytes).into_owned())
}

fn first_request() -> Value {
    json!({
        "source_path": fixture_path(),
        "name": "Sheet A",
        "columns": [["U1", "M1"], ["U2"]],
        "pins": {"U2": ["1", "2", "3", "4"]},
        "power_nets": ["P24X"],
    })
}

fn second_request() -> Value {
    json!({
        "source_path": fixture_path(),
        "name": "Sheet B",
        "columns": [["U2"], ["TB1", "R1"]],
        "pins": {"U2": ["9", "10"]},
        "power_nets": ["P24X"],
    })
}

fn request_of(body: &Value) -> DraftRequest {
    let mut body = body.clone();
    body.as_object_mut().unwrap().remove("source_path");
    serde_json::from_value(body).expect("DraftRequest")
}

fn project_json(doc: &SharedDoc) -> String {
    serde_json::to_string(doc.engine.lock().unwrap().project()).unwrap()
}

fn sheet_count(doc: &SharedDoc) -> usize {
    doc.engine.lock().unwrap().project().sheets.len()
}

fn draft(doc: &SharedDoc, body: &Value) -> Result<Value, String> {
    doc.draft_function_sheet(fixture_path().to_str().unwrap(), &request_of(body), madake_core::EditOrigin::Mcp)
}

fn bump(doc: &SharedDoc) {
    let text: madake_core::TextEntity =
        serde_json::from_value(json!({"id": Uuid::new_v4(), "at": {"x": 300.0, "y": 200.0}, "text": "note", "height": 3.5})).unwrap();
    let sheet_id = doc.engine.lock().unwrap().project().sheets[0].id;
    doc.engine
        .lock()
        .unwrap()
        .execute(madake_core::Command::AddEntity { sheet_id, entity: madake_core::Entity::Text(text) })
        .expect("注記を足せる");
}

/// Both draft tools are advertised, and the draft description says that it is one edit with one undo, and both say that the source file is only read.
/// 下書きの2つのツールが公開され、下書きの説明にはundo一発、両方の説明に元のファイルは読むだけと書いてある。
#[test]
fn the_draft_tools_are_advertised() {
    let descriptions = tool_descriptions();
    for name in ["draft_function_sheet", "check_function_sheets"] {
        let description = &descriptions
            .iter()
            .find(|(tool, _)| tool == name)
            .unwrap_or_else(|| panic!("MCPツール「{name}」が公開されていない"))
            .1;
        if name == "draft_function_sheet" {
            assert!(description.contains("undo"), "{name} の説明に「undo」が無い: {description}");
        }
        assert!(description.contains("読む"), "{name} の説明に「読む」が無い: {description}");
    }
}

/// A draft adds exactly one sheet in one edit, and one undo removes it again.
/// 下書きは1回の編集でシートを1枚だけ足し、undo一発でそのシートが消える。
#[tokio::test]
async fn a_draft_adds_one_sheet_and_one_undo_removes_it() {
    let (doc, _, _) = fixture();
    let depth = doc.engine.lock().unwrap().undo_depth();
    let reply = draft(&doc, &first_request()).expect("下書きできる");
    assert_eq!(sheet_count(&doc), 1);
    assert_eq!(doc.engine.lock().unwrap().undo_depth(), depth + 1);
    let sheet_id = reply["sheet_id"].as_str().expect("sheet_id");
    assert_eq!(doc.engine.lock().unwrap().project().sheets[0].id.to_string(), sheet_id);
    assert_eq!(doc.engine.lock().unwrap().project().sheets[0].name, "Sheet A");
    for key in ["units", "pins", "wired", "tagged", "marked", "labelled", "skipped_pins", "metrics"] {
        assert!(reply.get(key).is_some(), "返り値に「{key}」が無い: {reply}");
    }
    assert!(reply["metrics"].get("crossings").is_some(), "{reply}");
    doc.engine.lock().unwrap().undo().expect("元に戻せる");
    assert_eq!(sheet_count(&doc), 0);
    assert_eq!(doc.engine.lock().unwrap().undo_depth(), depth);
}

/// After one undo the project is byte for byte what it was before the draft.
/// 下書きの後にundoを1回すると、プロジェクトは下書きの前とバイト単位で同じに戻る。
#[tokio::test]
async fn undo_after_a_draft_restores_the_project_bytes() {
    let (doc, _, _) = fixture();
    let before = project_json(&doc);
    draft(&doc, &first_request()).expect("下書きできる");
    assert_ne!(project_json(&doc), before);
    doc.engine.lock().unwrap().undo().expect("元に戻せる");
    assert_eq!(project_json(&doc), before);
}

/// Drafting the same request twice is refused the second time (the sheet name is in use) and changes nothing.
/// 同じ依頼を2回続けると、2回目はシート名が使用中で断られ、何も変わらない。
#[tokio::test]
async fn drafting_the_same_request_twice_is_refused() {
    let (doc, _, _) = fixture();
    draft(&doc, &first_request()).expect("1回目は下書きできる");
    let after_first = project_json(&doc);
    let depth = doc.engine.lock().unwrap().undo_depth();
    let err = draft(&doc, &first_request()).expect_err("2回目は断られる");
    assert!(err.contains("Sheet A"), "{err}");
    assert_eq!(project_json(&doc), after_first);
    assert_eq!(doc.engine.lock().unwrap().undo_depth(), depth);
}

/// A source file that does not exist is refused with its path in the message, and nothing changes.
/// 元のファイルが無いと、メッセージに場所を入れて断り、何も変わらない。
#[tokio::test]
async fn a_missing_source_file_is_refused_and_nothing_changes() {
    let (doc, _, _) = fixture();
    let before = project_json(&doc);
    let depth = doc.engine.lock().unwrap().undo_depth();
    let missing = "/nonexistent/madake/source.mdkproj";
    let err = doc
        .draft_function_sheet(missing, &request_of(&first_request()), madake_core::EditOrigin::Mcp)
        .expect_err("断られる");
    assert!(err.contains(missing), "{err}");
    assert_eq!(project_json(&doc), before);
    assert_eq!(doc.engine.lock().unwrap().undo_depth(), depth);
    let err = doc.check_function_sheets(missing, &[]).expect_err("検算も断られる");
    assert!(err.contains(missing), "{err}");
}

/// Drafting and checking only read the source file: its bytes and its modification time stay the same.
/// 下書きと検算は元のファイルを読むだけで、中身も更新時刻も変わらない。
#[tokio::test]
async fn the_source_file_is_not_modified() {
    let (doc, _, _) = fixture();
    let copy = std::env::temp_dir().join(format!("madake-draft-source-{}.mdkproj", Uuid::new_v4()));
    std::fs::copy(fixture_path(), &copy).unwrap();
    let bytes = std::fs::read(&copy).unwrap();
    let modified = std::fs::metadata(&copy).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    doc.draft_function_sheet(copy.to_str().unwrap(), &request_of(&first_request()), madake_core::EditOrigin::Mcp)
        .expect("下書きできる");
    doc.check_function_sheets(copy.to_str().unwrap(), &[]).expect("検算できる");
    assert_eq!(std::fs::read(&copy).unwrap(), bytes);
    assert_eq!(std::fs::metadata(&copy).unwrap().modified().unwrap(), modified);
    std::fs::remove_file(&copy).ok();
}

/// A request the planner refuses (a unit the source does not have) is an error and changes nothing.
/// 計画が断る依頼 (元の図にない機器) はエラーになり、何も変わらない。
#[tokio::test]
async fn a_refused_plan_changes_nothing() {
    let (doc, _, _) = fixture();
    let before = project_json(&doc);
    let mut body = first_request();
    body["columns"] = json!([["NOPE9"]]);
    let err = draft(&doc, &body).expect_err("断られる");
    assert!(err.contains("NOPE9"), "{err}");
    assert_eq!(project_json(&doc), before);
}

/// When the drawing changes once between planning and applying, the draft plans again and still adds one sheet in one edit.
/// 計画と適用の間に図面が1度変わると、下書きは計画を作り直し、1回の編集でシートを1枚足す。
#[tokio::test]
async fn a_draft_plans_again_after_one_concurrent_edit() {
    let (doc, _, _) = fixture();
    // 変更を入れるシートが要る
    doc.engine
        .lock()
        .unwrap()
        .execute(madake_core::Command::AddSheet {
            name: "Existing".into(),
            size: madake_core::PaperSize::A3,
            orientation: madake_core::Orientation::Landscape,
            kind: Default::default(),
        })
        .expect("シートを足せる");
    let depth = doc.engine.lock().unwrap().undo_depth();
    let mut calls = 0;
    let reply = doc
        .draft_function_sheet_with(
            fixture_path().to_str().unwrap(),
            &request_of(&first_request()),
            madake_core::EditOrigin::Mcp,
            || {
                calls += 1;
                if calls == 1 {
                    bump(&doc);
                }
            },
        )
        .expect("下書きできる");
    assert_eq!(calls, 2);
    assert!(reply["sheet_id"].is_string(), "{reply}");
    assert_eq!(sheet_count(&doc), 2);
    // 注記を足した編集と、下書きの編集の2つ
    assert_eq!(doc.engine.lock().unwrap().undo_depth(), depth + 2);
}

/// When the drawing keeps changing, the draft gives up after three tries with a message and adds no sheet.
/// 図面が変わり続けると、下書きは3回で諦めてメッセージを返し、シートを足さない。
#[tokio::test]
async fn a_draft_gives_up_when_the_drawing_keeps_changing() {
    let (doc, _, _) = fixture();
    doc.engine
        .lock()
        .unwrap()
        .execute(madake_core::Command::AddSheet {
            name: "Existing".into(),
            size: madake_core::PaperSize::A3,
            orientation: madake_core::Orientation::Landscape,
            kind: Default::default(),
        })
        .expect("シートを足せる");
    let depth = doc.engine.lock().unwrap().undo_depth();
    let err = doc
        .draft_function_sheet_with(
            fixture_path().to_str().unwrap(),
            &request_of(&first_request()),
            madake_core::EditOrigin::Mcp,
            || bump(&doc),
        )
        .expect_err("諦めるはず");
    assert!(err.contains("何度も変わった"), "{err}");
    assert_eq!(sheet_count(&doc), 1);
    assert_eq!(doc.engine.lock().unwrap().undo_depth(), depth + 3);
}

/// Over MCP the draft tool adds the sheet and the check tool reports a half-drawn set as not ok, then ok once both sheets exist.
/// MCPで下書きすると、検算は1枚だけの間は「合っていない」、2枚そろうと「合っている」と返す。
#[tokio::test]
async fn the_mcp_tools_draft_and_check() {
    let (doc, bridge, _) = fixture();
    let reply = call(&bridge, "draft_function_sheet", first_request()).await;
    assert!(reply["sheet_id"].is_string(), "{reply}");
    assert_eq!(sheet_count(&doc), 1);
    let check = call(&bridge, "check_function_sheets", json!({"source_path": fixture_path()})).await;
    assert_eq!(check["ok"], json!(false), "{check}");
    assert!(!check["missing_pins"].as_array().unwrap().is_empty(), "{check}");
    call(&bridge, "draft_function_sheet", second_request()).await;
    let check = call(&bridge, "check_function_sheets", json!({"source_path": fixture_path()})).await;
    assert_eq!(check["ok"], json!(true), "{check}");
    // 外す参照記号は両側から外れるので、全部そろった図は引き続き合っている
    let check = call(
        &bridge,
        "check_function_sheets",
        json!({"source_path": fixture_path(), "ignore_references": ["U1"]}),
    )
    .await;
    assert_eq!(check["ok"], json!(true), "{check}");
    // 知らない項目のある依頼は、MCPでも断られる
    let mut unknown = second_request();
    unknown["name"] = json!("Sheet C");
    unknown["bogus"] = json!(1);
    let outcome = bridge.call("draft_function_sheet", unknown).await;
    assert!(outcome.is_error, "知らない項目は断られる: {}", outcome.content);
    assert_eq!(sheet_count(&doc), 2);
    // 断られた依頼はエラーになる
    let outcome = bridge.call("draft_function_sheet", first_request()).await;
    assert!(outcome.is_error, "同じ名前の2回目は断られる: {}", outcome.content);
}

/// The Link API draft endpoint returns the plan summary with the new sheet id, and refuses bad bodies with 400 and no change.
/// Link APIの下書きは、計画の要約と新しいシートのidを返し、使えない本文は400で断って何も変えない。
#[tokio::test]
async fn the_link_api_drafts_and_refuses_bad_bodies() {
    let (doc, _, router) = fixture();
    let before = project_json(&doc);
    let revision = doc.engine.lock().unwrap().revision();
    // 知らない項目
    let mut unknown = first_request();
    unknown["bogus"] = json!(1);
    let (status, bad) = post(&router, "/api/v1/draft/function-sheet", unknown).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
    // 元のファイルが無い
    let mut missing = first_request();
    missing["source_path"] = json!("/nonexistent/madake/source.mdkproj");
    let (status, bad) = post(&router, "/api/v1/draft/function-sheet", missing).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
    assert_eq!(project_json(&doc), before);
    assert_eq!(doc.engine.lock().unwrap().revision(), revision);
    let (status, reply) = post(&router, "/api/v1/draft/function-sheet", first_request()).await;
    assert_eq!(status, StatusCode::OK, "{reply}");
    for key in ["sheet_id", "units", "pins", "wired", "tagged", "marked", "labelled", "skipped_pins", "metrics"] {
        assert!(reply.get(key).is_some(), "返り値に「{key}」が無い: {reply}");
    }
    let (status, check) = post(&router, "/api/v1/draft/check", json!({"source_path": "/nonexistent/madake/source.mdkproj"})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{check}");
}

/// Two draft requests that together cover the source, then the check, all through the Link API: the check says ok.
/// 元の図を2枚で覆う下書きの依頼を2つ送り、続けて検算を送ると、すべてLink API経由で「合っている」と返る。
#[tokio::test]
async fn two_drafts_and_a_check_through_the_link_api() {
    let (doc, _, router) = fixture();
    let (status, first) = post(&router, "/api/v1/draft/function-sheet", first_request()).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let (status, second) = post(&router, "/api/v1/draft/function-sheet", second_request()).await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(sheet_count(&doc), 2);
    let (status, check) = post(&router, "/api/v1/draft/check", json!({"source_path": fixture_path()})).await;
    assert_eq!(status, StatusCode::OK, "{check}");
    assert_eq!(check["ok"], json!(true), "{check}");
    assert_eq!(check["missing_pins"], json!([]));
    assert_eq!(check["extra_pins"], json!([]));
    assert_eq!(check["split"], json!([]));
    assert_eq!(check["merged"], json!([]));
    assert_eq!(check["source_nets"], check["matched"], "{check}");
}

/// Writes both drafted sheets as SVG for a look with a renderer (run by hand: `cargo test -p madake-mcp --test draft_api -- --ignored`).
/// 下書きした2枚をSVGに書き出す (目で見るための手動の試験。--ignored を付けて実行する)。
#[tokio::test]
#[ignore]
async fn export_the_drafted_sheets_as_svg() {
    let (doc, bridge, router) = fixture();
    post(&router, "/api/v1/draft/function-sheet", first_request()).await;
    post(&router, "/api/v1/draft/function-sheet", second_request()).await;
    let dir = std::env::var("MADAKE_DRAFT_SVG_DIR").unwrap_or_else(|_| std::env::temp_dir().display().to_string());
    let ids: Vec<Uuid> = doc.engine.lock().unwrap().project().sheets.iter().map(|s| s.id).collect();
    for (n, id) in ids.iter().enumerate() {
        let path = format!("{dir}/task-7-sheet-{}.svg", n + 1);
        call(&bridge, "export_svg", json!({"sheet_id": id, "path": path})).await;
    }
}

const LATER_A: &str = "Cabinet B terminal side";
const LATER_B: &str = "Spare relay panel number two";

/// The three sheets of the scenario in which every sheet carries the net SIG_A; `planned` is sent to every request when given.
fn three_requests(planned: bool) -> Vec<Value> {
    let mut requests = vec![
        json!({"source_path": fixture_path(), "name": "Main", "columns": [["U1"], ["TB1"]], "power_nets": ["P24X"]}),
        json!({"source_path": fixture_path(), "name": LATER_A, "columns": [["R1"]]}),
        json!({"source_path": fixture_path(), "name": LATER_B, "columns": [["U2"]], "pins": {"U2": ["3"]}}),
    ];
    if planned {
        for r in &mut requests {
            r["planned_sheets"] = json!(["Main", LATER_A, LATER_B]);
        }
    }
    requests
}

/// planned_sheets is read by the MCP parameter type, including its hand-written reader, and a wrong type is refused.
/// planned_sheets はMCPの引数の型 (手書きの読み取りを含む) で読まれ、型が違えば断られる。
#[test]
fn planned_sheets_round_trips_through_the_mcp_params() {
    let params: madake_mcp::DraftFunctionSheetParams =
        serde_json::from_value(json!({"source_path": "/x.mdkproj", "name": "A", "columns": [["U1"]], "planned_sheets": ["A", "B"]})).unwrap();
    let names: Vec<&str> = params.request.planned_sheets.iter().map(|p| p.name()).collect();
    assert_eq!(names, vec!["A", "B"]);
    let none: madake_mcp::DraftFunctionSheetParams =
        serde_json::from_value(json!({"source_path": "/x.mdkproj", "name": "A", "columns": [["U1"]]})).unwrap();
    assert!(none.request.planned_sheets.is_empty());
    assert!(serde_json::from_value::<madake_mcp::DraftFunctionSheetParams>(
        json!({"source_path": "/x.mdkproj", "name": "A", "columns": [["U1"]], "planned_sheets": "A"})
    )
    .is_err());
}

/// The draft tool description tells the caller to pass the names of all sheets so that tags keep room for them.
/// 下書きツールの説明は、札が場所を空けるように全てのシートの名前を渡すよう伝えている。
#[test]
fn the_draft_tool_description_mentions_planned_sheets() {
    let descriptions = tool_descriptions();
    let description = &descriptions.iter().find(|(tool, _)| tool == "draft_function_sheet").unwrap().1;
    assert!(description.contains("planned_sheets"), "{description}");
}

/// Through the Link API, planned_sheets reaches the planner: a blank entry is refused (400, and the planner's message names the field), and nothing changes.
/// Link API経由で planned_sheets は計画まで届く。空白の項目は、項目名を挙げた400で断られ、何も変わらない。
#[tokio::test]
async fn the_link_api_reads_planned_sheets() {
    let (doc, _, router) = fixture();
    let before = project_json(&doc);
    let mut bad = three_requests(true).remove(0);
    bad["planned_sheets"] = json!(["Main", " "]);
    let err = draft(&doc, &bad).expect_err("a blank planned name is refused");
    assert!(err.contains("planned_sheets"), "{err}");
    let (status, reply) = post(&router, "/api/v1/draft/function-sheet", bad).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{reply}");
    assert_eq!(project_json(&doc), before);
    let (status, reply) = post(&router, "/api/v1/draft/function-sheet", three_requests(true).remove(0)).await;
    assert_eq!(status, StatusCode::OK, "{reply}");
}

/// Through the Link API, drafting three sheets with planned_sheets leaves no tag outside the frame, and without it the check lists the tag and says layout_ok is false while ok stays true.
/// Link API経由で、planned_sheets を付けて3枚を下書きすると枠からはみ出す札は無く、付けないと検算がその札を挙げて layout_ok が false になるが ok は true のまま。
#[tokio::test]
async fn the_check_reports_tags_outside_the_frame_through_the_link_api() {
    for planned in [true, false] {
        let (_, _, router) = fixture();
        for body in three_requests(planned) {
            let (status, reply) = post(&router, "/api/v1/draft/function-sheet", body).await;
            assert_eq!(status, StatusCode::OK, "{reply}");
        }
        let (status, check) = post(&router, "/api/v1/draft/check", json!({"source_path": fixture_path()})).await;
        assert_eq!(status, StatusCode::OK, "{check}");
        let outside = check["tags_outside"].as_array().expect("tags_outside");
        assert_eq!(check["layout_ok"], json!(outside.is_empty()), "{check}");
        assert_eq!(outside.is_empty(), planned, "{check}");
        if !planned {
            assert_eq!(outside[0]["sheet"], json!("Main"), "{check}");
            assert!(outside[0]["excess_mm"].as_f64().unwrap() > 0.0, "{check}");
        }
    }
}

/// The object form of a planned sheet (name, units, pins) and the string form both go through the MCP parameter type, and the planner reads the units of an object.
/// 計画のシートの機器つきの形 (name・units・pins) も文字列の形も、MCPの引数の型を通り、計画は機器つきの項目の機器を読む。
#[test]
fn planned_objects_round_trip_through_the_mcp_params() {
    let params: madake_mcp::DraftFunctionSheetParams = serde_json::from_value(json!({
        "source_path": "/x.mdkproj", "name": "A", "columns": [["U1"]],
        "planned_sheets": ["B", {"name": "C", "units": ["U2"], "pins": {"U2": ["3"]}}]
    }))
    .unwrap();
    assert_eq!(params.request.planned_sheets.len(), 2);
    assert_eq!(params.request.planned_sheets[0].name(), "B");
    match &params.request.planned_sheets[1] {
        madake_core::draft::PlannedSheet::Detailed(d) => {
            assert_eq!(d.name, "C");
            assert_eq!(d.units, vec!["U2".to_string()]);
            assert_eq!(d.pins["U2"], vec!["3".to_string()]);
        }
        other => panic!("{other:?}"),
    }
}

/// A malformed planned_sheets entry is refused through the Link API (400) and through the MCP tool with a message that names planned_sheets and the entry's position, and nothing changes.
/// planned_sheets の形の違う項目は、Link API (400) でも MCP のツールでも、planned_sheets と何番目の項目かを挙げた文で断られ、何も変わらない。
#[tokio::test]
async fn a_malformed_planned_entry_is_refused_naming_its_position() {
    let (doc, bridge, router) = fixture();
    let mut bad = first_request();
    bad["planned_sheets"] = json!(["Sheet B", {"name": "Sheet C"}]);
    let (status, text) = post_text(&router, "/api/v1/draft/function-sheet", bad.clone()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
    assert!(text.contains("planned_sheets[1]") && text.contains("entry 2"), "{text}");
    let outcome = bridge.call("draft_function_sheet", bad).await;
    assert!(outcome.is_error, "{}", outcome.content);
    assert!(outcome.content.contains("planned_sheets[1]") && outcome.content.contains("entry 2"), "{}", outcome.content);
    assert_eq!(sheet_count(&doc), 0);
}

/// Through the Link API an object entry reaches the planner: a unit the source does not have is refused naming the entry, a good entry drafts the sheet.
/// Link API経由で、機器つきの項目は計画まで届く。元の図に無い機器は項目を挙げて断られ、正しい項目ならシートが作られる。
#[tokio::test]
async fn the_link_api_reads_planned_objects() {
    let (doc, bridge, router) = fixture();
    let mut bad = first_request();
    bad["planned_sheets"] = json!([{"name": "Later", "units": ["NOPE9"]}]);
    let err = draft(&doc, &bad).expect_err("an unknown unit is refused");
    assert!(err.contains("Later") && err.contains("NOPE9"), "{err}");
    let (status, _) = post(&router, "/api/v1/draft/function-sheet", bad).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(sheet_count(&doc), 0);
    let mut good = first_request();
    good["planned_sheets"] = json!(["Sheet A", {"name": "Sheet B", "units": ["U2"], "pins": {"U2": ["9", "10"]}}]);
    let (status, reply) = post(&router, "/api/v1/draft/function-sheet", good.clone()).await;
    assert_eq!(status, StatusCode::OK, "{reply}");
    good["name"] = json!("Sheet B");
    good["columns"] = json!([["U2"], ["TB1", "R1"]]);
    good["pins"] = json!({"U2": ["9", "10"]});
    good["planned_sheets"] = json!(["Sheet A", {"name": "Sheet B", "units": ["U2"], "pins": {"U2": ["9", "10"]}}]);
    let reply = call(&bridge, "draft_function_sheet", good).await;
    assert!(reply["sheet_id"].is_string(), "{reply}");
    assert_eq!(sheet_count(&doc), 2);
}

fn title_block_of_sheet(doc: &SharedDoc, name: &str) -> Value {
    let engine = doc.engine.lock().unwrap();
    let sheet = engine.project().sheets.iter().find(|s| s.name == name).expect("the sheet exists");
    serde_json::to_value(&sheet.title_block).unwrap()
}

/// Through the MCP tool, a draft without title_block gets the sheet name as its drawing title, and one with planned_sheets and title_block fields gets the planned number and the given fields.
/// MCPツール経由で、title_block の無い下書きはシート名が品名になり、planned_sheets と title_block の欄を付けると計画の位置の図番と書いた欄が入る。
#[tokio::test]
async fn the_mcp_tool_fills_the_title_block() {
    let (doc, bridge, _) = fixture();
    call(&bridge, "draft_function_sheet", first_request()).await;
    assert_eq!(title_block_of_sheet(&doc, "Sheet A")["title"], "Sheet A");
    let mut second = second_request();
    second["planned_sheets"] = json!(["Sheet A", "Sheet B"]);
    second["title_block"] = json!({"company": "Example Works", "rev": "A"});
    call(&bridge, "draft_function_sheet", second).await;
    let tb = title_block_of_sheet(&doc, "Sheet B");
    assert_eq!((tb["title"].as_str(), tb["drawing_no"].as_str(), tb["company"].as_str(), tb["rev"].as_str()), (Some("Sheet B"), Some("2/2"), Some("Example Works"), Some("A")));
}

/// The MCP parameter type reads title_block and refuses an unknown title_block field, naming it.
/// MCPの引数の型は title_block を読み、知らない欄を書けばその欄名を挙げて断る。
#[test]
fn the_mcp_params_read_title_block_and_refuse_unknown_fields() {
    let params: madake_mcp::DraftFunctionSheetParams =
        serde_json::from_value(json!({"source_path": "/x.mdkproj", "name": "A", "columns": [["U1"]], "title_block": {"title": "T", "fields": {"k": "v"}}})).unwrap();
    assert_eq!(params.request.title_block.unwrap().title.as_deref(), Some("T"));
    let err = serde_json::from_value::<madake_mcp::DraftFunctionSheetParams>(
        json!({"source_path": "/x.mdkproj", "name": "A", "columns": [["U1"]], "title_block": {"titel": "T"}}),
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("titel"), "{err}");
}

/// Through the Link API, title_block overrides the default title, and an unknown title_block field is refused with 400 and changes nothing.
/// Link API経由で、title_block は既定の品名を置き換え、知らない欄は400で断られて何も変わらない。
#[tokio::test]
async fn the_link_api_reads_title_block() {
    let (doc, _, router) = fixture();
    let mut bad = first_request();
    bad["title_block"] = json!({"titel": "x"});
    let (status, text) = post_text(&router, "/api/v1/draft/function-sheet", bad).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(text.contains("titel"), "{text}");
    assert_eq!(sheet_count(&doc), 0);
    let mut good = first_request();
    good["title_block"] = json!({"title": "Cooling line", "drawing_no": "EX-1"});
    let (status, _) = post(&router, "/api/v1/draft/function-sheet", good).await;
    assert_eq!(status, StatusCode::OK);
    let tb = title_block_of_sheet(&doc, "Sheet A");
    assert_eq!((tb["title"].as_str(), tb["drawing_no"].as_str()), (Some("Cooling line"), Some("EX-1")));
}
