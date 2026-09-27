//! Link APIのエクスポートエンドポイントのテスト(サーバーは立てずRouterへ直接流す)。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use madake_core::{Engine, Project};
use madake_mcp::SharedDoc;
use serde_json::{json, Value};
use tower::ServiceExt;

fn fake_claude() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../madake-agent/tests/fixtures")
        .join("fake_claude.sh")
}

/// テストごとに一意の部品DBパスを返す。並列テストがファイルを共有すると
/// スキーマ初期化(版チェック→INSERT)が非アトミックなためUNIQUE制約で落ちる。
fn unique_parts_db(tag: &str) -> std::path::PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "madake-parts-{tag}-{}-{seq}.sqlite",
        std::process::id()
    ))
}

/// The REST parts endpoints support searching (sample data included), upserting, category filtering, deleting, and listing wire parts.
/// RESTの部品エンドポイントは検索(サンプル含む)・登録更新・カテゴリ絞り込み・削除・電線品番一覧に対応する。
#[tokio::test]
async fn parts_endpoints_search_upsert_delete() {
    let doc = SharedDoc::new(Engine::new(Project::new("部品テスト")));
    let agent = madake_mcp::agent::manager(&doc, 9310);
    agent.set_executable(Some(fake_claude()));
    let db_path = unique_parts_db("api");
    std::fs::remove_file(&db_path).ok();
    let parts = madake_mcp::open_parts(&db_path).expect("parts db");
    let router = madake_mcp::link_api::router(doc, Arc::clone(&agent), parts);

    let call = |method: &'static str, uri: String, body: Option<Value>| {
        let router = router.clone();
        async move {
            let mut b = Request::builder().method(method).uri(uri);
            if body.is_some() {
                b = b.header("content-type", "application/json");
            }
            let req = b
                .body(match body {
                    Some(v) => Body::from(v.to_string()),
                    None => Body::empty(),
                })
                .unwrap();
            let res = router.oneshot(req).await.unwrap();
            let status = res.status();
            let bytes = res.into_body().collect().await.unwrap().to_bytes();
            (status, serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null))
        }
    };

    // サンプルが検索できる
    let (status, body) = call("GET", "/api/v1/parts?query=MDK-FUSE".into(), None).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body.as_array().unwrap().len(), 1);
    assert_eq!(body[0]["symbol_id"], "fuse");

    // upsert → 検索でヒット
    let (status, _) = call(
        "POST",
        "/api/v1/parts".into(),
        Some(json!({
            "part_no": "TEST-PB-01", "name": "押しボタン", "category": "switch",
            "symbol_id": "pushbutton_no", "rated_current_a": 3.0
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = call("GET", "/api/v1/parts?query=TEST-PB".into(), None).await;
    assert_eq!(body[0]["name"], "押しボタン");

    // カテゴリ絞り込み
    let (_, body) = call("GET", "/api/v1/parts?category=switch".into(), None).await;
    assert!(body.as_array().unwrap().iter().all(|p| p["category"] == "switch"));

    // 削除
    let (status, body) = call("DELETE", "/api/v1/parts/TEST-PB-01".into(), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["deleted"], true);
    let (_, body) = call("GET", "/api/v1/parts?query=TEST-PB".into(), None).await;
    assert!(body.as_array().unwrap().is_empty());

    // 電線品番マスタ
    let (status, body) = call("GET", "/api/v1/wire-parts".into(), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.as_array().unwrap().is_empty(), "電線サンプル");

    std::fs::remove_file(&db_path).ok();
}

/// GET /api/v1/verify returns the drawing's diagnostics as JSON (e.g. empty reference and unconnected pins).
/// GET /api/v1/verify は図面の診断(空参照・未接続ピンなど)をJSONで返す。
#[tokio::test]
async fn verify_returns_diagnostics() {
    let mut project = Project::new("検証テスト");
    // 参照記号なしの抵抗を1個置く → erc.empty_reference と erc.unconnected_pin が出る
    let sheet_id = project.sheets[0].id;
    let entity = madake_core::Entity::Symbol(madake_core::SymbolInstance {
        id: uuid::Uuid::new_v4(),
        symbol_id: "resistor".into(),
        at: madake_core::Point::new(100.0, 50.0),
        rotation: 0,
        mirror: false,
        reference: String::new(),
        value: String::new(),
        attrs: Default::default(),
    });
    project.sheets[0].entities.insert(entity.id(), entity);
    let doc = SharedDoc::new(Engine::new(project));
    let agent = madake_mcp::agent::manager(&doc, 9310);
    agent.set_executable(Some(fake_claude()));
    let parts = madake_mcp::open_parts(
        &unique_parts_db("export"),
    )
    .expect("parts db");
    let router = madake_mcp::link_api::router(doc, Arc::clone(&agent), parts);

    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/verify?sheet_id={sheet_id}"))
        .body(Body::empty())
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let diags = body.as_array().expect("diagnostics array");
    let codes: Vec<&str> = diags.iter().filter_map(|d| d["code"].as_str()).collect();
    assert!(codes.contains(&"erc.empty_reference"), "{codes:?}");
    assert!(codes.contains(&"erc.unconnected_pin"), "{codes:?}");
}

/// POST /api/v1/import/kicad replaces the open project with the converted schematic and returns a patch plus an import report.
/// POST /api/v1/import/kicad は開いているプロジェクトを変換結果で置き換え、patchとインポートレポートを返す。
#[tokio::test]
async fn import_kicad_replaces_project_and_reports() {
    let doc = SharedDoc::new(Engine::new(Project::new("元プロジェクト")));
    let agent = madake_mcp::agent::manager(&doc, 9310);
    agent.set_executable(Some(fake_claude()));
    let parts = madake_mcp::open_parts(
        &unique_parts_db("kicad"),
    )
    .expect("parts db");
    let router = madake_mcp::link_api::router(doc.clone(), Arc::clone(&agent), parts);

    let sch = std::env::temp_dir().join(format!("madake-import-{}.kicad_sch", std::process::id()));
    std::fs::write(
        &sch,
        r#"(kicad_sch (version 20250114) (paper "A4")
  (title_block (title "KiCadから"))
  (wire (pts (xy 10 10) (xy 50 10)))
  (symbol (lib_id "Device:R") (at 60 10 0)
    (property "Reference" "R9") (property "Value" "1k")))"#,
    )
    .unwrap();

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/import/kicad")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "path": sch.to_string_lossy() }).to_string(),
        ))
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["report"]["symbols"], 1);
    assert_eq!(body["report"]["wires"], 1);
    assert!(body["patch"]["revision"].is_number());

    let engine = doc.engine.lock().unwrap();
    assert_eq!(engine.project().sheets[0].title_block.title, "KiCadから");
    std::fs::remove_file(&sch).ok();
}

/// POST /api/v1/simulate/op solves the DC operating point and returns net voltages and component currents (requires ngspice; skipped otherwise).
/// POST /api/v1/simulate/op はDC動作点を解き、ネット電圧と部品電流を返す(ngspice必須。未導入時はスキップ)。
#[tokio::test]
async fn simulate_op_returns_result() {
    if madake_core::ngspice::find_ngspice().is_none() {
        eprintln!("ngspice未検出のためスキップ");
        return;
    }
    let mut project = Project::new("simテスト");
    let sheet_id = project.sheets[0].id;
    // battery + lamp(2A) を直結した最小回路
    let bt = madake_core::Entity::Symbol(madake_core::SymbolInstance {
        id: uuid::Uuid::new_v4(),
        symbol_id: "battery".into(),
        at: madake_core::Point::new(60.0, 100.0),
        rotation: 0,
        mirror: false,
        reference: "BT1".into(),
        value: "DC24V".into(),
        attrs: Default::default(),
    });
    let mut attrs = std::collections::BTreeMap::new();
    attrs.insert("current_a".to_string(), "2".to_string());
    let lamp = madake_core::Entity::Symbol(madake_core::SymbolInstance {
        id: uuid::Uuid::new_v4(),
        symbol_id: "lamp".into(),
        at: madake_core::Point::new(110.0, 100.0),
        rotation: 0,
        mirror: false,
        reference: "L1".into(),
        value: String::new(),
        attrs,
    });
    let w1 = madake_core::Entity::Wire(madake_core::Wire {
        id: uuid::Uuid::new_v4(),
        points: vec![madake_core::Point::new(67.5, 100.0), madake_core::Point::new(102.5, 100.0)],
        color: "red".into(),
        sq: 0.75,
        length_m: None,
        part_no: None,
        net: None,
    });
    let w2 = madake_core::Entity::Wire(madake_core::Wire {
        id: uuid::Uuid::new_v4(),
        points: vec![
            madake_core::Point::new(117.5, 100.0),
            madake_core::Point::new(130.0, 100.0),
            madake_core::Point::new(130.0, 130.0),
            madake_core::Point::new(52.5, 130.0),
            madake_core::Point::new(52.5, 100.0),
        ],
        color: "black".into(),
        sq: 0.75,
        length_m: None,
        part_no: None,
        net: None,
    });
    for e in [bt, lamp, w1, w2] {
        project.sheets[0].entities.insert(e.id(), e);
    }
    let _ = sheet_id;
    let doc = SharedDoc::new(Engine::new(project));
    let agent = madake_mcp::agent::manager(&doc, 9310);
    agent.set_executable(Some(fake_claude()));
    let parts = madake_mcp::open_parts(
        &unique_parts_db("sim"),
    )
    .expect("parts db");
    let router = madake_mcp::link_api::router(doc, Arc::clone(&agent), parts);

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/simulate/op")
        .header("content-type", "application/json")
        .body(Body::from(json!({ "open_switches": [] }).to_string()))
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["voltage"], 24.0);
    let l1 = body["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["reference"] == "L1")
        .expect("L1");
    let amps = l1["amps"].as_f64().unwrap();
    assert!((amps - 2.0).abs() < 0.05, "{amps}");
}

/// POST /api/v1/export/pdf writes a valid PDF file to the requested path.
/// POST /api/v1/export/pdf は指定パスへ正しいPDFファイルを書き出す。
#[tokio::test]
async fn export_pdf_writes_pdf_file() {
    let doc = SharedDoc::new(Engine::new(Project::new("PDFテスト")));
    let agent = madake_mcp::agent::manager(&doc, 9310);
    agent.set_executable(Some(fake_claude()));
    let parts = madake_mcp::open_parts(
        &unique_parts_db("export"),
    )
    .expect("parts db");
    let router = madake_mcp::link_api::router(doc, Arc::clone(&agent), parts);

    let dir = std::env::temp_dir().join(format!("madake-pdf-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("out.pdf");

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/export/pdf")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "path": out.to_string_lossy() }).to_string(),
        ))
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["written"], json!(out.to_string_lossy()));

    let pdf = std::fs::read(&out).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    std::fs::remove_dir_all(&dir).ok();
}

/// POST /api/v1/export/dxf and /export/kicad write the sheet as DXF and .kicad_sch, and POST /api/v1/import/dxf reads the DXF back into a project with an import report.
/// POST /api/v1/export/dxf と /export/kicad はシートをDXFと.kicad_schへ書き出し、POST /api/v1/import/dxf はそのDXFをインポートレポート付きでプロジェクトへ読み戻す。
#[tokio::test]
async fn export_dxf_kicad_and_import_dxf_round_trip() {
    let project = Project::new("DXFテスト");
    let sheet_id = project.sheets[0].id;
    let doc = SharedDoc::new(Engine::new(project));
    doc.engine
        .lock()
        .unwrap()
        .execute(madake_core::Command::AddEntity {
            sheet_id,
            entity: madake_core::Entity::Symbol(madake_core::SymbolInstance {
                id: uuid::Uuid::new_v4(),
                symbol_id: "resistor".into(),
                at: madake_core::Point::new(100.0, 100.0),
                rotation: 0,
                mirror: false,
                reference: "R1".into(),
                value: "1k".into(),
                attrs: Default::default(),
            }),
        })
        .unwrap();
    let agent = madake_mcp::agent::manager(&doc, 9310);
    agent.set_executable(Some(fake_claude()));
    let parts = madake_mcp::open_parts(&unique_parts_db("dxf")).expect("parts db");
    let router = madake_mcp::link_api::router(doc.clone(), Arc::clone(&agent), parts);

    let dir = std::env::temp_dir().join(format!("madake-dxf-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let call = |uri: &'static str, body: Value| {
        let router = router.clone();
        async move {
            let req = Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap();
            let res = router.oneshot(req).await.unwrap();
            let status = res.status();
            let bytes = res.into_body().collect().await.unwrap().to_bytes();
            (status, serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null))
        }
    };

    let dxf_path = dir.join("sheet.dxf");
    let (status, body) = call("/api/v1/export/dxf", json!({ "path": dxf_path.to_string_lossy() })).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let dxf = std::fs::read_to_string(&dxf_path).unwrap();
    assert!(dxf.contains("AC1015") && dxf.contains("MDK_resistor"), "{dxf}");

    let sch_path = dir.join("sheet.kicad_sch");
    let (status, body) = call(
        "/api/v1/export/kicad",
        json!({ "sheet_id": sheet_id, "path": sch_path.to_string_lossy() }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let sch = std::fs::read_to_string(&sch_path).unwrap();
    assert!(sch.starts_with("(kicad_sch") && sch.contains("MadakeCAD:resistor"), "{sch}");

    let (status, body) = call("/api/v1/import/dxf", json!({ "path": dxf_path.to_string_lossy() })).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["report"]["symbols"], 1);
    assert!(body["patch"]["revision"].is_number());
    {
        let engine = doc.engine.lock().unwrap();
        let refs: Vec<String> = engine.project().sheets[0]
            .entities
            .values()
            .filter_map(|e| match e {
                madake_core::Entity::Symbol(s) => Some(s.reference.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(refs, vec!["R1"]);
    }

    let (status, body) = call("/api/v1/export/kicad", json!({ "sheet_id": uuid::Uuid::nil(), "path": sch_path.to_string_lossy() })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body:?}");
    std::fs::remove_dir_all(&dir).ok();
}

/// POST /api/v1/variants/start copies the sheet once per variant (one undo step) and returns each copy's id map; /variants/finish with a chosen sheet writes it back under the original ids and removes the copies, with null it only removes them; a count outside 1..=4 is rejected.
/// POST /api/v1/variants/start はシートを案の数だけ複製し(undo1回)、案ごとのid対応表を返す。/variants/finish は選んだ案を元idのまま元シートへ写し戻して複製を消し、nullなら複製を消すだけ。1〜4以外の数は拒否される。
#[tokio::test]
async fn variants_start_and_finish_round_trip() {
    let mut project = Project::new("案テスト");
    let sheet_id = project.sheets[0].id;
    let symbol_id = uuid::Uuid::new_v4();
    project.sheets[0].entities.insert(
        symbol_id,
        madake_core::Entity::Symbol(madake_core::SymbolInstance {
            id: symbol_id,
            symbol_id: "resistor".into(),
            at: madake_core::Point::new(100.0, 100.0),
            rotation: 0,
            mirror: false,
            reference: "R1".into(),
            value: "1k".into(),
            attrs: Default::default(),
        }),
    );
    let doc = SharedDoc::new(Engine::new(project));
    let agent = madake_mcp::agent::manager(&doc, 9310);
    agent.set_executable(Some(fake_claude()));
    let parts = madake_mcp::open_parts(&unique_parts_db("variants")).expect("parts db");
    let router = madake_mcp::link_api::router(doc.clone(), Arc::clone(&agent), parts);
    let call = |uri: &'static str, body: Value| {
        let router = router.clone();
        async move {
            let req = Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap();
            let res = router.oneshot(req).await.unwrap();
            let status = res.status();
            let bytes = res.into_body().collect().await.unwrap().to_bytes();
            (status, serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null))
        }
    };

    let (status, body) = call("/api/v1/variants/start", json!({ "sheet_id": sheet_id, "count": 5 })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body:?}");

    let (status, body) = call("/api/v1/variants/start", json!({ "sheet_id": sheet_id, "count": 2 })).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let run = body["run"].clone();
    assert_eq!(run["variants"].as_array().unwrap().len(), 2);
    assert_eq!(run["variants"][0]["label"], "案A");
    let variant_sheet: uuid::Uuid = serde_json::from_value(run["variants"][0]["sheet_id"].clone()).unwrap();
    let copy_symbol: uuid::Uuid =
        serde_json::from_value(run["variants"][0]["id_map"][symbol_id.to_string()].clone()).unwrap();
    assert_eq!(doc.engine.lock().unwrap().project().sheets.len(), 3);

    // 案Aでシンボルを動かす
    let mut moved = doc.engine.lock().unwrap().project().sheet(variant_sheet).unwrap().entities[&copy_symbol].clone();
    if let madake_core::Entity::Symbol(s) = &mut moved {
        s.at = madake_core::Point::new(150.0, 100.0);
    }
    let (status, _) = call(
        "/api/v1/commands",
        json!([{ "type": "update_entity", "sheet_id": variant_sheet, "entity": moved }]),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let mut finish = run.clone();
    finish["chosen_sheet_id"] = json!(variant_sheet);
    let (status, body) = call("/api/v1/variants/finish", finish).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    {
        let engine = doc.engine.lock().unwrap();
        assert_eq!(engine.project().sheets.len(), 1);
        let madake_core::Entity::Symbol(s) = &engine.project().sheets[0].entities[&symbol_id] else { panic!() };
        assert_eq!(s.at.x, 150.0, "元idのまま案の位置になる");
    }
    doc.undo().unwrap();
    assert_eq!(doc.engine.lock().unwrap().project().sheets.len(), 3, "採用はundo1回");

    let mut discard = run.clone();
    discard["chosen_sheet_id"] = Value::Null;
    let (status, body) = call("/api/v1/variants/finish", discard).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(doc.engine.lock().unwrap().project().sheets.len(), 1);
}

/// POST /api/v1/export/pdf-book writes one PDF holding the cover, every sheet and the requested reports.
/// POST /api/v1/export/pdf-book は表紙・全シート・指定した帳票を1つのPDFにまとめて書き出す。
#[tokio::test]
async fn export_pdf_book_writes_cover_sheets_and_reports() {
    let doc = SharedDoc::new(Engine::new(Project::new("PDF一括テスト")));
    let agent = madake_mcp::agent::manager(&doc, 9310);
    agent.set_executable(Some(fake_claude()));
    let parts = madake_mcp::open_parts(
        &unique_parts_db("book"),
    )
    .expect("parts db");
    let router = madake_mcp::link_api::router(doc, Arc::clone(&agent), parts);

    let dir = std::env::temp_dir().join(format!("madake-pdf-book-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("book.pdf");

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/export/pdf-book")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "path": out.to_string_lossy(),
                "include_reports": ["wire-list", "bom"],
            })
            .to_string(),
        ))
        .unwrap();
    let response = router.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["written"], json!(out.to_string_lossy()));
    // 新規プロジェクト = 表紙 + シート1枚 + 帳票2ページ
    assert_eq!(body["pages"], json!(4));

    let pdf = std::fs::read(&out).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    std::fs::remove_dir_all(&dir).ok();
}

/// The terminal endpoints list the terminal blocks of the drawing and return one block's chart and check result.
/// 端子台エンドポイントは図面の端子台を一覧し、1台のチャートとチェック結果を返す。
#[tokio::test]
async fn terminal_endpoints_list_chart_and_check() {
    let mut project = Project::new("端子台API");
    let tb_id = uuid::Uuid::new_v4();
    let mut attrs = std::collections::BTreeMap::new();
    attrs.insert("jumpers".to_string(), "1-2".to_string());
    project.sheets[0].entities.insert(
        tb_id,
        madake_core::Entity::Symbol(madake_core::SymbolInstance {
            id: tb_id,
            symbol_id: "terminal_block_4p".into(),
            at: madake_core::Point::new(100.0, 100.0),
            rotation: 0,
            mirror: false,
            reference: "TB1".into(),
            value: String::new(),
            attrs,
        }),
    );
    let doc = SharedDoc::new(Engine::new(project));
    let agent = madake_mcp::agent::manager(&doc, 9310);
    agent.set_executable(Some(fake_claude()));
    let parts = madake_mcp::open_parts(
        &unique_parts_db("tb"),
    )
    .expect("parts db");
    let router = madake_mcp::link_api::router(doc, Arc::clone(&agent), parts);

    let get = |uri: String| {
        let router = router.clone();
        async move {
            let req = Request::builder().method("GET").uri(uri).body(Body::empty()).unwrap();
            let res = router.oneshot(req).await.unwrap();
            let status = res.status();
            let bytes = res.into_body().collect().await.unwrap().to_bytes();
            (status, serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null))
        }
    };

    // 一覧: 参照記号・極数・ジャンパ指定が出る
    let (status, body) = get("/api/v1/terminals".into()).await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body.as_array().unwrap().len(), 1);
    assert_eq!(body[0]["reference"], "TB1");
    assert_eq!(body[0]["terminal_count"], 4);
    assert_eq!(body[0]["jumpers"], "1-2");
    assert_eq!(body[0]["entity_id"], json!(tb_id.to_string()));

    // チャート: 端子4行
    let (status, chart) = get(format!("/api/v1/terminals/chart?entity_id={tb_id}")).await;
    assert_eq!(status, StatusCode::OK, "{chart:?}");
    assert_eq!(chart["rows"].as_array().unwrap().len(), 4);

    // チェック: 未結線なので予備端子の情報が出る
    let (status, diags) = get(format!("/api/v1/terminals/check?entity_id={tb_id}")).await;
    assert_eq!(status, StatusCode::OK, "{diags:?}");
    assert!(!diags.as_array().unwrap().is_empty(), "{diags:?}");

    // 端子台でないidは400
    let (status, _) = get(format!("/api/v1/terminals/chart?entity_id={}", uuid::Uuid::new_v4())).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// POST /api/v1/export/report writes one report as CSV or as framed PDF pages, and refuses CSV for the graphical terminal diagram.
/// POST /api/v1/export/report は帳票1種をCSVまたは図枠付きPDFで書き出し、図面である端子接続図のCSVは拒否する。
#[tokio::test]
async fn export_report_writes_csv_and_pdf_per_report() {
    let doc = SharedDoc::new(Engine::new(Project::new("帳票API")));
    let agent = madake_mcp::agent::manager(&doc, 9310);
    agent.set_executable(Some(fake_claude()));
    let parts = madake_mcp::open_parts(
        &unique_parts_db("report"),
    )
    .expect("parts db");
    let router = madake_mcp::link_api::router(doc, Arc::clone(&agent), parts);

    let dir = std::env::temp_dir().join(format!("madake-report-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let post = |body: Value| {
        let router = router.clone();
        async move {
            let req = Request::builder()
                .method("POST")
                .uri("/api/v1/export/report")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap();
            let res = router.oneshot(req).await.unwrap();
            let status = res.status();
            let bytes = res.into_body().collect().await.unwrap().to_bytes();
            (status, serde_json::from_slice::<Value>(&bytes).unwrap_or(Value::Null))
        }
    };

    // From-To電線リストのCSV
    let csv_path = dir.join("wire-list.csv");
    let (status, body) = post(json!({
        "path": csv_path.to_string_lossy(), "kind": "wire-list", "format": "csv"
    }))
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    let csv = std::fs::read_to_string(&csv_path).unwrap();
    assert!(csv.starts_with("シート,From,To,"), "{csv}");

    // クロスリファレンス表の図面シートPDF
    let pdf_path = dir.join("xref.pdf");
    let (status, body) = post(json!({
        "path": pdf_path.to_string_lossy(), "kind": "xref", "format": "pdf"
    }))
    .await;
    assert_eq!(status, StatusCode::OK, "{body:?}");
    assert_eq!(body["count"], json!(1));
    assert!(std::fs::read(&pdf_path).unwrap().starts_with(b"%PDF-"));

    // 端子接続図はグラフィカルなのでCSV不可
    let (status, _) = post(json!({
        "path": dir.join("tb.csv").to_string_lossy(), "kind": "terminal-diagram", "format": "csv"
    }))
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    std::fs::remove_dir_all(&dir).ok();
}
