//! Link API (/api/v1): 素のJSON REST + SSEパッチストリーム。
//!
//! 用途:
//! 1. FreeCADアドオン等の外部ツール連携 (spec §7)
//! 2. ブラウザでのフロントエンド開発・E2E検証 (Tauri外からvite UIを実バックエンドに接続)
//!
//! 書き込みは全て既存のCommandエンジン(SharedDoc::execute)を通るため、
//! undo/redo・patch配信・UI/AI編集と完全に整合する。

use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::sse::{Event, Sse};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{http::StatusCode, Json, Router};
use futures::stream::Stream;
use madake_agent::{AgentManager, AppSettings, Conversation, DetectResult};
use madake_core::{builtin_symbols, sheet_symbol_defs, Command, Patch};
use serde::Deserialize;
use tower_http::cors::{AllowHeaders, AllowMethods, AllowOrigin, CorsLayer};
use uuid::Uuid;

use crate::{SharedDoc, SharedParts};

type ApiError = (StatusCode, String);

fn bad_request(msg: impl std::fmt::Display) -> ApiError {
    (StatusCode::BAD_REQUEST, msg.to_string())
}

async fn get_project(State(doc): State<SharedDoc>) -> Json<serde_json::Value> {
    let engine = doc.engine.lock().unwrap();
    Json(serde_json::json!({
        "revision": engine.revision(),
        "project": engine.project(),
        "can_undo": engine.can_undo(),
        "can_redo": engine.can_redo(),
    }))
}

async fn get_symbols() -> Json<serde_json::Value> {
    Json(serde_json::json!(builtin_symbols()))
}

#[derive(Deserialize)]
struct NetlistQuery {
    sheet_id: Option<Uuid>,
}

async fn get_netlist(
    State(doc): State<SharedDoc>,
    Query(q): Query<NetlistQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let engine = doc.engine.lock().unwrap();
    let sheet = match q.sheet_id {
        Some(id) => engine.project().sheet(id),
        None => engine.project().sheets.first(),
    }
    .ok_or_else(|| bad_request("sheet not found"))?;
    Ok(Json(serde_json::json!(
        madake_core::netlist::extract_netlist(sheet, &sheet_symbol_defs(sheet))
    )))
}

/// 外部クライアント(madake CLI・FreeCADアドオン・ブラウザ検証)からの編集。
/// 由来は`mcp`として履歴に残る(UI操作=`user`・エージェント=`agent`と区別する)。
async fn post_commands(
    State(doc): State<SharedDoc>,
    Json(commands): Json<Vec<Command>>,
) -> Result<Json<Vec<Patch>>, ApiError> {
    let mut patches = Vec::new();
    for cmd in commands {
        patches.push(doc.execute_external(cmd).map_err(bad_request)?);
    }
    Ok(Json(patches))
}

/// 使える開始テンプレートの一覧(同梱+ユーザーの`~/MadakeCAD/templates`)。
async fn get_templates() -> Json<madake_core::templates::TemplateList> {
    Json(madake_core::templates::list())
}

#[derive(Deserialize)]
struct ApplyTemplateBody {
    template_id: String,
    #[serde(default)]
    sheet_id: Option<Uuid>,
}

/// 開始テンプレートをシートへ適用する。**1回の編集**として履歴に乗る(undo一発)。
async fn post_apply_template(
    State(doc): State<SharedDoc>,
    Json(body): Json<ApplyTemplateBody>,
) -> Result<Json<Patch>, ApiError> {
    let sheet_id = match body.sheet_id {
        Some(id) => id,
        None => doc
            .engine
            .lock()
            .unwrap()
            .project()
            .sheets
            .first()
            .map(|s| s.id)
            .ok_or_else(|| bad_request("project has no sheets"))?,
    };
    doc.apply_template(&body.template_id, sheet_id, doc.mcp_origin())
        .map(Json)
        .map_err(bad_request)
}

/// 使える回路マクロの一覧(ユーザーの`~/MadakeCAD/macros`)。
async fn get_macros() -> Json<madake_core::macros::MacroList> {
    Json(madake_core::macros::list())
}

#[derive(Deserialize)]
struct SaveMacroBody {
    #[serde(default)]
    sheet_id: Option<Uuid>,
    /// マクロにするエンティティ(選択範囲)。
    entity_ids: Vec<Uuid>,
    #[serde(flatten)]
    meta: madake_core::macros::MacroMeta,
}

/// 選択したエンティティから回路マクロを**組み立てるだけ**返す(ファイルには書かない)。
/// 保存ダイアログのプレビューと、⌘Cの無名マクロが使う。
async fn post_build_macro(
    State(doc): State<SharedDoc>,
    Json(body): Json<SaveMacroBody>,
) -> Result<Json<madake_core::macros::Macro>, ApiError> {
    let sheet_id = resolve_sheet(&doc, body.sheet_id)?;
    doc.build_macro(sheet_id, &body.entity_ids, &body.meta)
        .map(Json)
        .map_err(bad_request)
}

/// 選択したエンティティを回路マクロとして保存する(座標は基準点からの相対、線番は除去)。
async fn post_save_macro(
    State(doc): State<SharedDoc>,
    Json(body): Json<SaveMacroBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let sheet_id = resolve_sheet(&doc, body.sheet_id)?;
    let (m, path) = doc
        .save_macro(sheet_id, &body.entity_ids, &body.meta)
        .map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "macro": m, "path": path })))
}

#[derive(Deserialize)]
struct ApplyMacroBody {
    /// マクロのid (`GET /macros`で得る)。
    id: String,
    /// バリアントキー ("A"=既定)。省略時は既定。
    #[serde(default)]
    variant: Option<String>,
    /// 値セットid。指定すると定格・型番が一括設定される。省略時は保存時の値のまま。
    #[serde(default)]
    value_set: Option<String>,
    #[serde(default)]
    sheet_id: Option<Uuid>,
    /// 基準点が来る位置 (カーソル位置)。
    at: madake_core::Point,
    /// 0/90/180/270。省略時は0。
    #[serde(default)]
    rotation: u16,
}

/// 回路マクロをシートへ挿入する。**1回の編集**として履歴に乗る(undo一発)。
async fn post_apply_macro(
    State(doc): State<SharedDoc>,
    Json(body): Json<ApplyMacroBody>,
) -> Result<Json<Patch>, ApiError> {
    let sheet_id = resolve_sheet(&doc, body.sheet_id)?;
    doc.apply_macro(
        &body.id,
        body.variant.as_deref(),
        body.value_set.as_deref(),
        sheet_id,
        body.at,
        body.rotation,
        doc.mcp_origin(),
    )
    .map(Json)
    .map_err(bad_request)
}

#[derive(Deserialize)]
struct ApplyMacroInlineBody {
    /// 挿入するマクロそのもの (ライブラリに無い無名マクロ = ⌘C/Vのクリップボード)。
    #[serde(rename = "macro")]
    macro_def: madake_core::macros::Macro,
    #[serde(default)]
    variant: Option<String>,
    /// 値セットid (`POST /macros/apply`と同じ)。
    #[serde(default)]
    value_set: Option<String>,
    #[serde(default)]
    sheet_id: Option<Uuid>,
    at: madake_core::Point,
    #[serde(default)]
    rotation: u16,
}

/// マクロ**そのもの**をシートへ挿入する(idを引かない)。`POST /macros/apply`と同じく
/// **1回の編集**として履歴に乗る(undo一発)。
async fn post_apply_macro_inline(
    State(doc): State<SharedDoc>,
    Json(body): Json<ApplyMacroInlineBody>,
) -> Result<Json<Patch>, ApiError> {
    let sheet_id = resolve_sheet(&doc, body.sheet_id)?;
    doc.insert_macro(
        &body.macro_def,
        body.variant.as_deref(),
        body.value_set.as_deref(),
        sheet_id,
        body.at,
        body.rotation,
        doc.mcp_origin(),
    )
    .map(Json)
    .map_err(bad_request)
}

#[derive(Deserialize)]
struct PlcModuleQuery {
    /// PLCモジュールの参照記号 (例 "PLC1")。省略で全モジュール。
    #[serde(default)]
    module_ref: Option<String>,
}

/// 図面に置かれているPLC I/Oモジュールの一覧 (参照記号・点数・入出力の種別)。
async fn get_plc_modules(State(doc): State<SharedDoc>) -> Json<serde_json::Value> {
    let engine = doc.engine.lock().unwrap();
    Json(serde_json::json!({
        "modules": madake_core::plc::plc_modules(engine.project())
    }))
}

/// PLC I/O割付表 (接続先・線番は図面から読んだ値)。`module_ref`で1モジュールに絞れる。
async fn get_plc_assignments(
    State(doc): State<SharedDoc>,
    Query(q): Query<PlcModuleQuery>,
) -> Json<serde_json::Value> {
    let engine = doc.engine.lock().unwrap();
    let project = engine.project();
    let refs = match q.module_ref {
        Some(r) => vec![r],
        None => madake_core::plc::assigned_module_refs(project),
    };
    let modules: Vec<serde_json::Value> = refs
        .iter()
        .map(|module_ref| {
            serde_json::json!({
                "module_ref": module_ref,
                "points": madake_core::plc::plc_points(project, module_ref),
            })
        })
        .collect();
    Json(serde_json::json!({
        "assignments": project.plc_assignments,
        "modules": modules,
    }))
}

#[derive(Deserialize)]
struct PlcAssignmentsBody {
    /// 置き換え後の割付表 (プロジェクト全体)。
    assignments: Vec<madake_core::PlcAssignment>,
}

/// PLC I/O割付表を丸ごと置き換える。**1回の編集**として履歴に乗る(undo一発)。
async fn put_plc_assignments(
    State(doc): State<SharedDoc>,
    Json(body): Json<PlcAssignmentsBody>,
) -> Result<Json<Patch>, ApiError> {
    doc.set_plc_assignments(body.assignments, doc.mcp_origin())
        .map(Json)
        .map_err(bad_request)
}

#[derive(Deserialize)]
struct PlcImportBody {
    /// 取り込み先モジュールの参照記号。
    module_ref: String,
    /// CSV本文 (アドレス,信号名,コメント。見出し行はあってもなくてもよい)。
    csv: String,
}

/// 割付表のCSVを取り込む (対象モジュールの行だけ置き換え)。undo一発で元に戻る。
async fn post_plc_import(
    State(doc): State<SharedDoc>,
    Json(body): Json<PlcImportBody>,
) -> Result<Json<Patch>, ApiError> {
    doc.import_plc_assignments_csv(&body.module_ref, &body.csv, doc.mcp_origin())
        .map(Json)
        .map_err(bad_request)
}

#[derive(Deserialize)]
struct PlcGenerateBody {
    /// モジュールの参照記号 (例 "PLC1")。
    module_ref: String,
    /// モジュール定義 (点数・入出力・アドレス体系)。部品DBの`plc_module`列と同じ形。
    module: madake_core::plc::PlcModuleSpec,
    /// 生成設定 (ラダー形式・ラング間隔・先頭スキップ・配置方針)。省略時は既定。
    #[serde(default)]
    options: madake_core::plc::PlcSheetOptions,
}

/// PLC I/O図面 (ラダーページ) を生成する。**1回の編集**として履歴に乗る(undo一発)。
async fn post_plc_generate(
    State(doc): State<SharedDoc>,
    Json(body): Json<PlcGenerateBody>,
) -> Result<Json<Patch>, ApiError> {
    doc.generate_plc_sheet(
        &body.module_ref,
        &body.module,
        &body.options,
        doc.mcp_origin(),
    )
    .map(Json)
    .map_err(bad_request)
}

/// シートidの解決 (省略時は先頭シート)。
fn resolve_sheet(doc: &SharedDoc, sheet_id: Option<Uuid>) -> Result<Uuid, ApiError> {
    match sheet_id {
        Some(id) => Ok(id),
        None => doc
            .engine
            .lock()
            .unwrap()
            .project()
            .sheets
            .first()
            .map(|s| s.id)
            .ok_or_else(|| bad_request("project has no sheets")),
    }
}

async fn post_undo(State(doc): State<SharedDoc>) -> Result<Json<Option<Patch>>, ApiError> {
    doc.undo().map(Json).map_err(bad_request)
}

async fn post_redo(State(doc): State<SharedDoc>) -> Result<Json<Option<Patch>>, ApiError> {
    doc.redo().map(Json).map_err(bad_request)
}

#[derive(Deserialize)]
struct PathBody {
    path: String,
}

/// 図面とチャット履歴をまとめて保存する(Tauriの`save_project`と同じ処理)。
async fn post_save(
    State(state): State<AgentApi>,
    Json(body): Json<PathBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    crate::agent::save_project_with_chat(
        &state.doc,
        &state.agent,
        std::path::Path::new(&body.path),
    )
    .map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "written": body.path })))
}

/// 図面とチャット履歴をまとめて読み込む(Tauriの`load_project`と同じ処理)。
///
/// 実行中のターンは中断される。
async fn post_load(
    State(state): State<AgentApi>,
    Json(body): Json<PathBody>,
) -> Result<Json<Patch>, ApiError> {
    crate::agent::load_project_with_chat(&state.doc, &state.agent, std::path::Path::new(&body.path))
        .map(Json)
        .map_err(bad_request)
}

/// KiCad回路図を読み込みプロジェクトを置き換える。
async fn post_import_kicad(
    State(state): State<AgentApi>,
    Json(body): Json<PathBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let (patch, report) = crate::agent::import_kicad_with_chat(
        &state.doc,
        &state.agent,
        std::path::Path::new(&body.path),
    )
    .map_err(bad_request)?;
    Ok(Json(
        serde_json::json!({ "patch": patch, "report": report }),
    ))
}

#[derive(Deserialize)]
struct ImportDxfBody {
    path: String,
    /// 配線とみなすレイヤ名 (省略時は名前に WIRE を含むレイヤ)。
    #[serde(default)]
    wire_layers: Vec<String>,
}

/// DXF (AutoCAD Electrical / EPLAN の中間形式) を読み込みプロジェクトを置き換える。
async fn post_import_dxf(
    State(state): State<AgentApi>,
    Json(body): Json<ImportDxfBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let options = madake_core::dxf::DxfImportOptions { wire_layers: body.wire_layers };
    let (patch, report) = crate::agent::import_dxf_with_chat(
        &state.doc,
        &state.agent,
        std::path::Path::new(&body.path),
        &options,
    )
    .map_err(bad_request)?;
    Ok(Json(
        serde_json::json!({ "patch": patch, "report": report }),
    ))
}

#[derive(Deserialize)]
struct ExportSvgBody {
    sheet_id: Option<Uuid>,
    path: String,
}

/// シートをDXF (AutoCAD 2000形式、ACADE/EPLANが読める) で書き出す。
async fn post_export_dxf(
    State(doc): State<SharedDoc>,
    Json(body): Json<ExportSvgBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let engine = doc.engine.lock().unwrap();
    let project = engine.project();
    let sheet = match body.sheet_id {
        Some(id) => project.sheet(id),
        None => project.sheets.first(),
    }
    .ok_or_else(|| bad_request("sheet not found"))?;
    let dxf = madake_core::dxf::sheet_to_dxf(sheet, &sheet_symbol_defs(sheet));
    std::fs::write(&body.path, dxf).map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "written": body.path })))
}

/// シートをKiCad回路図 (.kicad_sch) で書き出す。
async fn post_export_kicad(
    State(doc): State<SharedDoc>,
    Json(body): Json<ExportSvgBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let engine = doc.engine.lock().unwrap();
    let project = engine.project();
    let sheet = match body.sheet_id {
        Some(id) => project.sheet(id),
        None => project.sheets.first(),
    }
    .ok_or_else(|| bad_request("sheet not found"))?;
    let sch = madake_core::kicad::export_kicad_sch(sheet, &sheet_symbol_defs(sheet));
    std::fs::write(&body.path, sch).map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "written": body.path })))
}

async fn post_export_svg(
    State(doc): State<SharedDoc>,
    Json(body): Json<ExportSvgBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let engine = doc.engine.lock().unwrap();
    let project = engine.project();
    let sheet = match body.sheet_id {
        Some(id) => project.sheet(id),
        None => project.sheets.first(),
    }
    .ok_or_else(|| bad_request("sheet not found"))?;
    // プロジェクト文脈で描くとネットラベルにシート間クロスリファレンスが入る
    let svg = madake_core::svg::project_sheet_to_svg(project, sheet.id, &sheet_symbol_defs(sheet))
        .ok_or_else(|| bad_request("sheet not found"))?;
    std::fs::write(&body.path, svg).map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "written": body.path })))
}

#[derive(Deserialize)]
struct SimulateBody {
    sheet_id: Option<Uuid>,
    #[serde(default)]
    open_switches: Vec<String>,
}

async fn post_simulate_op(
    State(doc): State<SharedDoc>,
    Json(body): Json<SimulateBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let engine = doc.engine.lock().unwrap();
    let sheet = match body.sheet_id {
        Some(id) => engine.project().sheet(id),
        None => engine.project().sheets.first(),
    }
    .ok_or_else(|| bad_request("sheet not found"))?;
    let result =
        madake_core::sim::simulate_op(sheet, &sheet_symbol_defs(sheet), &body.open_switches)
            .map_err(bad_request)?;
    Ok(Json(serde_json::json!(result)))
}

async fn get_verify(
    State(doc): State<SharedDoc>,
    Query(q): Query<NetlistQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let engine = doc.engine.lock().unwrap();
    let project = engine.project();
    // シート指定なし=プロジェクト全体。ネットラベル関連はシートを跨いだ統合ネットで評価する
    let diags = match q.sheet_id {
        Some(id) => {
            let sheet = project
                .sheet(id)
                .ok_or_else(|| bad_request("sheet not found"))?;
            madake_core::verify::verify_sheet(sheet, &sheet_symbol_defs(sheet))
        }
        None => madake_core::verify::verify_project(project),
    };
    Ok(Json(serde_json::json!(diags)))
}

/// 図面の整い具合 (配線交差数・ラベル/シンボルの重なり数・グリッド外の点数)。
/// 整えループが編集の前後で比べる目標値。`sheet_id`省略で先頭シート。
async fn get_tidy_metrics(
    State(doc): State<SharedDoc>,
    Query(q): Query<NetlistQuery>,
) -> Result<Json<madake_core::tidy::TidyMetrics>, ApiError> {
    let engine = doc.engine.lock().unwrap();
    let sheet = match q.sheet_id {
        Some(id) => engine.project().sheet(id),
        None => engine.project().sheets.first(),
    }
    .ok_or_else(|| bad_request("sheet not found"))?;
    Ok(Json(madake_core::tidy::tidy_metrics(
        sheet,
        &sheet_symbol_defs(sheet),
    )))
}

async fn post_export_pdf(
    State(doc): State<SharedDoc>,
    Json(body): Json<ExportSvgBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let engine = doc.engine.lock().unwrap();
    let project = engine.project();
    let sheet = match body.sheet_id {
        Some(id) => project.sheet(id),
        None => project.sheets.first(),
    }
    .ok_or_else(|| bad_request("sheet not found"))?;
    let pdf = madake_core::pdf::project_sheet_to_pdf(project, sheet.id, &sheet_symbol_defs(sheet))
        .ok_or_else(|| bad_request("sheet not found"))?
        .map_err(bad_request)?;
    std::fs::write(&body.path, pdf).map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "written": body.path })))
}

#[derive(Deserialize)]
struct ExportPdfBookBody {
    path: String,
    /// 回路図の後ろに付ける帳票 (`wire-list` / `terminal-chart` / `terminal-diagram` / `bom` / `xref`)。
    #[serde(default)]
    include_reports: Vec<madake_core::report_sheet::ReportKind>,
    /// 表紙を付けるか (既定: 付ける)。
    #[serde(default = "default_true")]
    cover: bool,
}

fn default_true() -> bool {
    true
}

/// 表紙+回路図全シート+選択帳票を1つのPDFにまとめて書き出す。
async fn post_export_pdf_book(
    State(doc): State<SharedDoc>,
    Json(body): Json<ExportPdfBookBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let engine = doc.engine.lock().unwrap();
    let options = madake_core::pdf::PdfBookOptions {
        include_reports: body.include_reports,
        cover: body.cover,
    };
    let pages = madake_core::pdf::project_pdf_pages(engine.project(), &options).len();
    let pdf =
        madake_core::pdf::export_project_pdf(engine.project(), &options).map_err(bad_request)?;
    std::fs::write(&body.path, pdf).map_err(bad_request)?;
    Ok(Json(
        serde_json::json!({ "written": body.path, "pages": pages }),
    ))
}

/// 端子台エディタ・帳票の対象選択に出す端子台の一覧。`sheet_id`でシートを絞れる。
async fn get_terminals(
    State(doc): State<SharedDoc>,
    Query(q): Query<NetlistQuery>,
) -> Json<serde_json::Value> {
    let engine = doc.engine.lock().unwrap();
    Json(serde_json::json!(
        madake_core::terminal_chart::terminal_block_infos(engine.project(), q.sheet_id)
    ))
}

#[derive(Deserialize)]
struct EntityQuery {
    entity_id: Uuid,
}

#[derive(Deserialize)]
struct SearchQuery {
    /// 検索語 (部分一致・大文字小文字を区別しない)。空なら結果は0件。
    #[serde(default)]
    q: String,
    /// 対象種別をカンマ区切りで絞る (`reference,value,net,wire_no,text`)。省略で全種別。
    #[serde(default)]
    kinds: Option<String>,
}

/// プロジェクト内検索 (⌘F)。参照記号・型番・ネット名・線番・テキストを横断する。
/// 結果は図面の読み順 (シート→ゾーン→id) で、各ヒットはジャンプ先の所在を持つ。
async fn get_search(
    State(doc): State<SharedDoc>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let kinds = match q.kinds.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => Vec::new(),
        Some(raw) => raw
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|name| {
                madake_core::search::SearchKind::parse(name)
                    .ok_or_else(|| bad_request(format!("unknown search kind: {name}")))
            })
            .collect::<Result<Vec<_>, _>>()?,
    };
    let engine = doc.engine.lock().unwrap();
    let hits = madake_core::search::search_project(engine.project(), &q.q, &kinds);
    Ok(Json(
        serde_json::json!({ "count": hits.len(), "hits": hits }),
    ))
}

/// デバイスナビゲータのツリー (参照記号 → 機能: コイル/接点/端子/本体)。
async fn get_devices(State(doc): State<SharedDoc>) -> Json<serde_json::Value> {
    let engine = doc.engine.lock().unwrap();
    Json(serde_json::json!({
        "devices": madake_core::search::device_tree(engine.project())
    }))
}

/// 端子台1つのチャート (端子ごとの行・ジャンパ)。端子台でなければ400。
async fn get_terminal_chart(
    State(doc): State<SharedDoc>,
    Query(q): Query<EntityQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let engine = doc.engine.lock().unwrap();
    let chart =
        madake_core::terminal_chart::terminal_chart_in_project(engine.project(), q.entity_id)
            .ok_or_else(|| bad_request("terminal block not found"))?;
    Ok(Json(serde_json::json!(chart)))
}

/// 端子台チェック (未結線の端子・不正なジャンパ)。
async fn get_terminal_check(
    State(doc): State<SharedDoc>,
    Query(q): Query<EntityQuery>,
) -> Json<serde_json::Value> {
    let engine = doc.engine.lock().unwrap();
    Json(serde_json::json!(
        madake_core::terminal_chart::check_terminal_block_in_project(engine.project(), q.entity_id)
    ))
}

#[derive(Deserialize)]
struct ExportReportBody {
    path: String,
    /// 帳票の種類 (`wire-list` / `terminal-chart` / `terminal-diagram` / `bom` / `xref`)。
    kind: madake_core::report_sheet::ReportKind,
    /// 出力形式 (`csv` / `pdf`)。
    format: madake_core::report_sheet::ReportFormat,
    /// 端子台チャート・端子接続図の対象を1つの端子台に絞る場合のentity id。
    #[serde(default)]
    entity_id: Option<Uuid>,
}

/// 帳票1種を1ファイルへ書き出す。`count`はCSVなら行数、PDFならページ数。
async fn post_export_report(
    State(doc): State<SharedDoc>,
    Json(body): Json<ExportReportBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let engine = doc.engine.lock().unwrap();
    let (bytes, count) = madake_core::report_sheet::report_bytes(
        engine.project(),
        body.kind,
        body.format,
        body.entity_id,
    )
    .map_err(bad_request)?;
    std::fs::write(&body.path, bytes).map_err(bad_request)?;
    Ok(Json(
        serde_json::json!({ "written": body.path, "count": count }),
    ))
}

async fn post_export_bom(
    State(doc): State<SharedDoc>,
    Json(body): Json<PathBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let engine = doc.engine.lock().unwrap();
    std::fs::write(&body.path, madake_core::reports::bom_csv(engine.project()))
        .map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "written": body.path })))
}

async fn post_export_wire_list(
    State(doc): State<SharedDoc>,
    Json(body): Json<PathBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let engine = doc.engine.lock().unwrap();
    std::fs::write(
        &body.path,
        madake_core::reports::wire_list_csv(engine.project()),
    )
    .map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "written": body.path })))
}

/// パッチのSSEストリーム。接続時点以降の全patchをJSONで流す。
async fn get_events(
    State(doc): State<SharedDoc>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = doc.patches.subscribe();
    let stream = futures::stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(patch) => {
                    let event = Event::default()
                        .event("patch")
                        .data(serde_json::to_string(&patch).unwrap_or_default());
                    return Some((Ok(event), rx));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
            }
        }
    });
    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}

/// エージェントAPIの状態(ドキュメント + マネージャ)。
#[derive(Clone)]
struct AgentApi {
    doc: SharedDoc,
    agent: Arc<AgentManager>,
}

#[derive(Deserialize)]
struct AgentSendBody {
    /// 継続する会話。省略時は新規会話を作る。
    conversation_id: Option<Uuid>,
    prompt: String,
    model: Option<String>,
}

async fn post_agent_send(
    State(state): State<AgentApi>,
    Json(body): Json<AgentSendBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let context = crate::agent::drawing_context(&state.doc);
    let id = state
        .agent
        .send(
            body.conversation_id,
            &body.prompt,
            body.model,
            Some(context),
        )
        .await
        .map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "conversation_id": id })))
}

#[derive(Deserialize)]
struct ConversationRefBody {
    conversation_id: Uuid,
}

async fn post_agent_cancel(
    State(state): State<AgentApi>,
    Json(body): Json<ConversationRefBody>,
) -> Json<serde_json::Value> {
    let cancelled = state.agent.cancel(body.conversation_id);
    Json(serde_json::json!({ "cancelled": cancelled }))
}

async fn get_agent_conversations(State(state): State<AgentApi>) -> Json<Vec<Conversation>> {
    Json(state.agent.conversations())
}

#[derive(Deserialize)]
struct UndoTurnBody {
    conversation_id: Uuid,
    /// 巻き戻す対象のターンID(`conversations`の`messages[].turn_id`)。
    turn_id: Uuid,
}

async fn post_agent_undo_turn(
    State(state): State<AgentApi>,
    Json(body): Json<UndoTurnBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let revision = state
        .agent
        .undo_turn(body.conversation_id, body.turn_id)
        .map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "revision": revision })))
}

async fn get_agent_detect(State(state): State<AgentApi>) -> Result<Json<DetectResult>, ApiError> {
    state.agent.detect().await.map(Json).map_err(bad_request)
}

/// 現在のアプリ設定(`~/.madakecad/settings.json`の内容)。
async fn get_settings(State(state): State<AgentApi>) -> Json<AppSettings> {
    Json(state.agent.settings())
}

/// プロバイダの状態(**APIキーそのものは返さない**。保存済みかどうかだけ)。
///
/// キーチェーンの読み出しはOSの許可ダイアログで止まることがあるため、非同期ランタイムの
/// スレッドを塞がないようブロッキング用スレッドで行う(他のAPIは待たされない)。
async fn get_agent_provider(State(state): State<AgentApi>) -> Json<serde_json::Value> {
    Json(provider_status_blocking(&state.agent).await)
}

async fn provider_status_blocking(agent: &Arc<AgentManager>) -> serde_json::Value {
    crate::agent::provider_status_async(agent).await
}

/// APIキーの保存リクエスト。
#[derive(serde::Deserialize)]
struct ApiKeyBody {
    key: String,
    /// どのプロバイダのキーか(`anthropic_api` / `openai_compat` / `gemini`)。省略時はAnthropic。
    #[serde(default)]
    provider: Option<String>,
}

/// 削除するキーのプロバイダ指定(`?provider=gemini`など)。省略時はAnthropic。
#[derive(serde::Deserialize)]
struct ApiKeyQuery {
    #[serde(default)]
    provider: Option<String>,
}

/// APIキーをOSキーチェーンへ保存する。**設定ファイルには書かない。**
async fn put_agent_api_key(
    State(state): State<AgentApi>,
    Json(body): Json<ApiKeyBody>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let key = body.key;
    let account = crate::agent::key_account_for(body.provider.as_deref());
    tokio::task::spawn_blocking(move || madake_agent::secrets::set_api_key(account, &key))
        .await
        .map_err(|e| bad_request(e.to_string()))?
        .map_err(|e| bad_request(e.to_string()))?;
    Ok(Json(provider_status_blocking(&state.agent).await))
}

/// 保存済みのAPIキーを消す。
async fn delete_agent_api_key(
    State(state): State<AgentApi>,
    Query(query): Query<ApiKeyQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let account = crate::agent::key_account_for(query.provider.as_deref());
    tokio::task::spawn_blocking(move || madake_agent::secrets::clear_api_key(account))
        .await
        .map_err(|e| bad_request(e.to_string()))?
        .map_err(|e| bad_request(e.to_string()))?;
    Ok(Json(provider_status_blocking(&state.agent).await))
}

/// いま選んでいるプロバイダへ小さなリクエストを投げて設定が使えるか確かめる。
///
/// 失敗しても200で`{"ok": false, "error": "..."}`を返す(UIがそのまま表示する)。
async fn post_agent_test_connection(State(state): State<AgentApi>) -> Json<serde_json::Value> {
    Json(match state.agent.test_connection().await {
        Ok(model) => serde_json::json!({ "ok": true, "model": model }),
        // kind=UIが翻訳するための区分、error=翻訳が無いときにそのまま出せる説明
        Err(e) => serde_json::json!({ "ok": false, "error_kind": e.kind, "error": e.message }),
    })
}

/// アプリ設定を保存し、エージェントへ反映する。戻り値は正規化後の設定。
async fn put_settings(
    State(state): State<AgentApi>,
    Json(body): Json<AppSettings>,
) -> Result<Json<AppSettings>, ApiError> {
    crate::agent::update_settings(&state.agent, body)
        .map(Json)
        .map_err(bad_request)
}

/// エージェントイベントのSSEストリーム。
///
/// イベント名は`agent`、データはTauriの`agent:event`と同一のJSON
/// (`{"conversation_id": "...", "event": {"type": ...}}`)。
async fn get_agent_events(
    State(state): State<AgentApi>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.agent.subscribe();
    let stream = futures::stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    let event = Event::default()
                        .event("agent")
                        .data(serde_json::to_string(&event).unwrap_or_default());
                    return Some((Ok(event), rx));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
            }
        }
    });
    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}

/// エージェントマネージャを必要とするRouter(ブラウザ検証用。Tauri IPCと同じ
/// マネージャを共有)。`/save`・`/load`もチャット履歴を伴うためここに置く。
/// `/settings`はマネージャへ反映するため同様。
fn agent_router(state: AgentApi) -> Router {
    Router::new()
        .route("/api/v1/save", post(post_save))
        .route("/api/v1/load", post(post_load))
        .route("/api/v1/import/kicad", post(post_import_kicad))
        .route("/api/v1/import/dxf", post(post_import_dxf))
        .route("/api/v1/agent/send", post(post_agent_send))
        .route("/api/v1/agent/cancel", post(post_agent_cancel))
        .route("/api/v1/agent/conversations", get(get_agent_conversations))
        .route("/api/v1/agent/undo-turn", post(post_agent_undo_turn))
        .route("/api/v1/agent/detect", get(get_agent_detect))
        .route("/api/v1/agent/provider", get(get_agent_provider))
        .route(
            "/api/v1/agent/api-key",
            axum::routing::put(put_agent_api_key).delete(delete_agent_api_key),
        )
        .route(
            "/api/v1/agent/test-connection",
            post(post_agent_test_connection),
        )
        .route("/api/v1/agent/events", get(get_agent_events))
        .route("/api/v1/settings", get(get_settings).put(put_settings))
        .with_state(state)
}

/// ローカル由来のOriginか。
///
/// 判定対象は`localhost` / `127.0.0.1` / `::1` / `*.localhost`(Tauri WindowsのWebView2は
/// `http://tauri.localhost`、macOS/Linuxは`tauri://localhost`を送る)。
/// ポート番号は任意(viteは1420、他ツールは任意ポートを使う)。
pub fn is_local_origin(origin: &str) -> bool {
    let Some(rest) = ["http://", "https://", "tauri://"]
        .iter()
        .find_map(|scheme| origin.strip_prefix(scheme))
    else {
        return false;
    };
    if rest.is_empty() || rest.contains('/') || rest.contains('@') {
        return false;
    }
    // `host:port` / `[::1]:port` からホスト部を取り出す
    let host = match rest.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => host,
        _ => rest,
    };
    let host = host.trim_start_matches('[').trim_end_matches(']');
    matches!(host, "localhost" | "127.0.0.1" | "::1") || host.ends_with(".localhost")
}

/// 外部WebページからのDNSリバインディング/CSRF的な呼び出しを弾む。
///
/// このサーバーは127.0.0.1バインドだが、任意のWebページのJSからは
/// `http://127.0.0.1:9310/api/v1/agent/send`を叩けてしまう(それだけでAIに図面を
/// 編集させられる)。Originが付かないリクエスト(curl・CLI・Tauri webview)は許可し、
/// ローカル以外のOriginが付いていれば403で拒否する。
async fn guard_origin(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    match request.headers().get(axum::http::header::ORIGIN) {
        None => next.run(request).await,
        Some(origin) if origin.to_str().map(is_local_origin).unwrap_or(false) => {
            next.run(request).await
        }
        Some(_) => (
            StatusCode::FORBIDDEN,
            "MadakeCAD Link APIはローカルからのみ利用できます",
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct PartsQuery {
    query: Option<String>,
    category: Option<String>,
}

async fn get_parts(
    State(parts): State<SharedParts>,
    Query(q): Query<PartsQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = parts.lock().unwrap();
    let hits = db
        .search_parts(q.query.as_deref().unwrap_or(""), q.category.as_deref())
        .map_err(bad_request)?;
    Ok(Json(serde_json::json!(hits)))
}

async fn post_part(
    State(parts): State<SharedParts>,
    Json(part): Json<madake_core::parts::Part>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = parts.lock().unwrap();
    db.upsert_part(&part).map_err(bad_request)?;
    Ok(Json(
        serde_json::json!({ "ok": true, "part_no": part.part_no }),
    ))
}

async fn delete_part(
    State(parts): State<SharedParts>,
    axum::extract::Path(part_no): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = parts.lock().unwrap();
    let deleted = db.delete_part(&part_no).map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "deleted": deleted })))
}

async fn get_wire_parts(
    State(parts): State<SharedParts>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = parts.lock().unwrap();
    Ok(Json(serde_json::json!(db
        .list_wire_parts()
        .map_err(bad_request)?)))
}

async fn post_wire_part(
    State(parts): State<SharedParts>,
    Json(row): Json<madake_core::parts::WirePartRow>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let db = parts.lock().unwrap();
    db.upsert_wire_part(&row).map_err(bad_request)?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "name": "MadakeCAD Link API", "version": 1 }))
}

/// /api/v1 のRouterを構築する。
///
/// アクセスはローカルオリジンに限定する(サーバーは127.0.0.1バインドだが、
/// 外部Webページのブラウザからは到達できてしまうため)。ブラウザ以外
/// (curl・madake CLI・FreeCADアドオン)はOriginを付けないので影響を受けない。
pub fn router(doc: SharedDoc, agent: Arc<AgentManager>, parts: SharedParts) -> Router {
    let agent_routes = agent_router(AgentApi {
        doc: doc.clone(),
        agent,
    });
    let parts_routes = Router::new()
        .route("/api/v1/parts", get(get_parts).post(post_part))
        .route(
            "/api/v1/parts/{part_no}",
            axum::routing::delete(delete_part),
        )
        .route(
            "/api/v1/wire-parts",
            get(get_wire_parts).post(post_wire_part),
        )
        .with_state(parts);
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin, _| {
            origin.to_str().map(is_local_origin).unwrap_or(false)
        }))
        .allow_methods(AllowMethods::mirror_request())
        .allow_headers(AllowHeaders::mirror_request());
    Router::new()
        .route("/api/v1", get(health))
        .route("/api/v1/project", get(get_project))
        .route("/api/v1/symbols", get(get_symbols))
        .route("/api/v1/netlist", get(get_netlist))
        .route("/api/v1/verify", get(get_verify))
        .route("/api/v1/tidy-metrics", get(get_tidy_metrics))
        .route("/api/v1/search", get(get_search))
        .route("/api/v1/devices", get(get_devices))
        .route("/api/v1/terminals", get(get_terminals))
        .route("/api/v1/terminals/chart", get(get_terminal_chart))
        .route("/api/v1/terminals/check", get(get_terminal_check))
        .route("/api/v1/plc/modules", get(get_plc_modules))
        .route(
            "/api/v1/plc/assignments",
            get(get_plc_assignments).put(put_plc_assignments),
        )
        .route("/api/v1/plc/assignments/import", post(post_plc_import))
        .route("/api/v1/plc/generate", post(post_plc_generate))
        .route("/api/v1/simulate/op", post(post_simulate_op))
        .route("/api/v1/templates", get(get_templates))
        .route("/api/v1/templates/apply", post(post_apply_template))
        .route("/api/v1/macros", get(get_macros))
        .route("/api/v1/macros/build", post(post_build_macro))
        .route("/api/v1/macros/save", post(post_save_macro))
        .route("/api/v1/macros/apply", post(post_apply_macro))
        .route("/api/v1/macros/apply-inline", post(post_apply_macro_inline))
        .route("/api/v1/commands", post(post_commands))
        .route("/api/v1/undo", post(post_undo))
        .route("/api/v1/redo", post(post_redo))
        .route("/api/v1/export/svg", post(post_export_svg))
        .route("/api/v1/export/pdf", post(post_export_pdf))
        .route("/api/v1/export/dxf", post(post_export_dxf))
        .route("/api/v1/export/kicad", post(post_export_kicad))
        .route("/api/v1/export/pdf-book", post(post_export_pdf_book))
        .route("/api/v1/export/report", post(post_export_report))
        .route("/api/v1/export/bom", post(post_export_bom))
        .route("/api/v1/export/wire-list", post(post_export_wire_list))
        .route("/api/v1/events", get(get_events))
        .with_state(doc)
        .merge(parts_routes)
        .merge(agent_routes)
        .layer(cors)
        // CORSより外側。プリフライトもここを通す(外部オリジンはここで403)
        .layer(axum::middleware::from_fn(guard_origin))
}
