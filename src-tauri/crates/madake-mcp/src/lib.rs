//! madake-mcp: MadakeCADに内蔵されるMCPサーバー。
//!
//! Streamable HTTP (127.0.0.1:port/mcp) で公開し、Claude Code / Claude Desktop等の
//! MCPクライアントが起動中のMadakeCADドキュメントを直接読み書きできる。
//! 全ての編集はmadake-coreのCommandエンジンを通るため、UI操作と同じundo/redo履歴に乗り、
//! patchブロードキャスト経由でUIにリアルタイム反映される。

pub mod agent;
pub mod link_api;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use madake_core::{builtin_symbols, resolve_symbol, sheet_symbol_defs, Command, Engine, Entity, Patch, Point, SymbolInstance, Wire};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ErrorData, ServerCapabilities, ServerInfo};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use tokio::sync::broadcast;
use uuid::Uuid;

/// UI(Tauri)とMCPが共有するドキュメント状態。
/// 部品DB (グローバル共有マスタ)。UI/MCP/Link API共通。
pub type SharedParts = Arc<Mutex<madake_core::parts::PartsDb>>;

/// 部品DBを開いて共有ハンドルにする。
pub fn open_parts(path: &std::path::Path) -> Result<SharedParts, madake_core::parts::PartsError> {
    Ok(Arc::new(Mutex::new(madake_core::parts::PartsDb::open(path)?)))
}

#[derive(Clone)]
pub struct SharedDoc {
    pub engine: Arc<Mutex<Engine>>,
    /// 全patchの配信チャネル。Tauri側が購読しwebviewへ転送する。
    pub patches: broadcast::Sender<Patch>,
}

impl SharedDoc {
    pub fn new(engine: Engine) -> Self {
        let (patches, _) = broadcast::channel(256);
        Self {
            engine: Arc::new(Mutex::new(engine)),
            patches,
        }
    }

    /// コマンドを実行しpatchをブロードキャストする。UI・MCP共通の入口。
    pub fn execute(&self, cmd: Command) -> madake_core::Result<Patch> {
        let patch = self.engine.lock().unwrap().execute(cmd)?;
        let _ = self.patches.send(patch.clone());
        Ok(patch)
    }

    pub fn undo(&self) -> madake_core::Result<Option<Patch>> {
        let patch = self.engine.lock().unwrap().undo()?;
        if let Some(p) = &patch {
            let _ = self.patches.send(p.clone());
        }
        Ok(patch)
    }

    pub fn redo(&self) -> madake_core::Result<Option<Patch>> {
        let patch = self.engine.lock().unwrap().redo()?;
        if let Some(p) = &patch {
            let _ = self.patches.send(p.clone());
        }
        Ok(patch)
    }
}

fn internal(e: impl std::fmt::Display) -> ErrorData {
    ErrorData::internal_error(e.to_string(), None)
}

fn json_ok<T: serde::Serialize>(value: &T) -> Result<String, ErrorData> {
    serde_json::to_string(value).map_err(internal)
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ExecuteCommandsParams {
    /// 実行するコマンド列。スキーマはmadake-coreのCommand型に準拠。
    pub commands: Vec<Command>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct PlaceSymbolParams {
    /// 配置先シートID。省略時は先頭シート。
    pub sheet_id: Option<Uuid>,
    /// シンボルライブラリのキー(list_symbolsで取得)。
    pub symbol_id: String,
    /// 配置位置(mm、用紙左上原点)。
    pub x: f64,
    pub y: f64,
    /// 回転角: 0/90/180/270。
    pub rotation: Option<u16>,
    /// 参照記号(例: "K1")。
    pub reference: Option<String>,
    /// 型番・値(例: "JZX-22F")。
    pub value: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DrawWireParams {
    /// 配線先シートID。省略時は先頭シート。
    pub sheet_id: Option<Uuid>,
    /// 折れ線の頂点列(mm)。直交配線を推奨。
    pub points: Vec<Point>,
    /// 線色(例: "red", "black", "light_blue")。
    pub color: Option<String>,
    /// 線径 sq (mm2)。例: 0.3, 0.75, 3.5。
    pub sq: Option<f64>,
    /// 電線長(m)。
    pub length_m: Option<f64>,
    /// 電線品番。
    pub part_no: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SheetRefParams {
    /// 対象シートID。省略時は先頭シート。
    pub sheet_id: Option<Uuid>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SimulateOpParams {
    /// 対象シートID。省略時は先頭シート。
    pub sheet_id: Option<Uuid>,
    /// 開路として扱うスイッチ/接点の参照記号 (what-if)。
    #[serde(default)]
    pub open_switches: Vec<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchPartsParams {
    /// 型番・名称・メーカの部分一致。空で全件。
    pub query: Option<String>,
    /// カテゴリ完全一致 (例: "relay", "connector")。
    pub category: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct PartNoParams {
    pub part_no: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ExportPathParams {
    /// 出力先ファイルパス(絶対パス)。
    pub path: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ExportSvgParams {
    /// 対象シートID。省略時は先頭シート。
    pub sheet_id: Option<Uuid>,
    /// 出力先ファイルパス(絶対パス)。
    pub path: String,
}

#[derive(Clone)]
pub struct MadakeMcp {
    doc: SharedDoc,
    parts: SharedParts,
}

impl MadakeMcp {
    pub fn new(doc: SharedDoc, parts: SharedParts) -> Self {
        Self { doc, parts }
    }

    fn resolve_sheet(&self, sheet_id: Option<Uuid>) -> Result<Uuid, ErrorData> {
        let engine = self.doc.engine.lock().unwrap();
        match sheet_id {
            Some(id) => {
                if engine.project().sheet(id).is_some() {
                    Ok(id)
                } else {
                    Err(ErrorData::invalid_params(
                        format!("sheet not found: {id}"),
                        None,
                    ))
                }
            }
            None => engine
                .project()
                .sheets
                .first()
                .map(|s| s.id)
                .ok_or_else(|| ErrorData::internal_error("project has no sheets", None)),
        }
    }
}

#[tool_router]
impl MadakeMcp {
    #[tool(
        description = "現在開いているMadakeCADプロジェクト全体(全シート・全エンティティ)をJSONで返す"
    )]
    fn get_project(&self) -> Result<String, ErrorData> {
        let engine = self.doc.engine.lock().unwrap();
        let out = serde_json::json!({
            "revision": engine.revision(),
            "project": engine.project(),
        });
        json_ok(&out)
    }

    #[tool(description = "シンボルライブラリの一覧(id・名称・カテゴリ・ピン定義)を返す。これに加え、動的ID `connector_{n}p` / `terminal_block_{n}p`(n=1..50、例: terminal_block_8p)でピン数可変のコネクタ・端子台を配置できる")]
    fn list_symbols(&self) -> Result<String, ErrorData> {
        json_ok(&builtin_symbols())
    }

    #[tool(
        description = "任意の編集コマンド列を実行する(シート追加、エンティティ追加/更新/削除/移動、表題欄設定など)。各コマンドの完全なスキーマは入力スキーマを参照。実行結果のpatchを返す"
    )]
    fn execute_commands(
        &self,
        Parameters(p): Parameters<ExecuteCommandsParams>,
    ) -> Result<String, ErrorData> {
        let mut patches = Vec::new();
        for cmd in p.commands {
            let patch = self.doc.execute(cmd).map_err(internal)?;
            patches.push(patch);
        }
        json_ok(&patches)
    }

    #[tool(description = "シンボルをシートに配置する。作成されたエンティティidを返す")]
    fn place_symbol(
        &self,
        Parameters(p): Parameters<PlaceSymbolParams>,
    ) -> Result<String, ErrorData> {
        if resolve_symbol(&p.symbol_id).is_none() {
            return Err(ErrorData::invalid_params(
                format!("unknown symbol_id: {}", p.symbol_id),
                None,
            ));
        }
        let sheet_id = self.resolve_sheet(p.sheet_id)?;
        let id = Uuid::new_v4();
        let entity = Entity::Symbol(SymbolInstance {
            id,
            symbol_id: p.symbol_id,
            at: Point::new(p.x, p.y),
            rotation: p.rotation.unwrap_or(0) % 360,
            mirror: false,
            reference: p.reference.unwrap_or_default(),
            value: p.value.unwrap_or_default(),
            attrs: Default::default(),
        });
        let patch = self
            .doc
            .execute(Command::AddEntity { sheet_id, entity })
            .map_err(internal)?;
        json_ok(&serde_json::json!({ "entity_id": id, "revision": patch.revision }))
    }

    #[tool(description = "配線(ポリライン)をシートに描く。作成されたエンティティidを返す")]
    fn draw_wire(&self, Parameters(p): Parameters<DrawWireParams>) -> Result<String, ErrorData> {
        if p.points.len() < 2 {
            return Err(ErrorData::invalid_params(
                "wire needs at least 2 points",
                None,
            ));
        }
        let sheet_id = self.resolve_sheet(p.sheet_id)?;
        let id = Uuid::new_v4();
        let entity = Entity::Wire(Wire {
            id,
            points: p.points,
            color: p.color.unwrap_or_else(|| "black".into()),
            sq: p.sq.unwrap_or(0.3),
            length_m: p.length_m,
            part_no: p.part_no,
            net: None,
        });
        let patch = self
            .doc
            .execute(Command::AddEntity { sheet_id, entity })
            .map_err(internal)?;
        json_ok(&serde_json::json!({ "entity_id": id, "revision": patch.revision }))
    }

    #[tool(
        description = "シートのネットリスト(電気的接続グラフ)を返す。各ネットは名前・所属ピン(参照記号+ピン番号)・配線idを持つ"
    )]
    fn get_netlist(&self, Parameters(p): Parameters<SheetRefParams>) -> Result<String, ErrorData> {
        let sheet_id = self.resolve_sheet(p.sheet_id)?;
        let engine = self.doc.engine.lock().unwrap();
        let sheet = engine
            .project()
            .sheet(sheet_id)
            .ok_or_else(|| ErrorData::invalid_params("sheet not found", None))?;
        json_ok(&madake_core::netlist::extract_netlist(
            sheet,
            &sheet_symbol_defs(sheet),
        ))
    }

    #[tool(
        description = "図面を検証しDiagnostic配列を返す。ERC(未接続ピン・参照記号重複・宙ぶらりん配線・ネットラベル競合)と電気検証(電源到達性・線径許容電流・電圧降下・ヒューズ定格)。負荷電流はシンボル属性current_a、電源電圧はbatteryのvalue(既定24V)。sheet_id省略時は全シート"
    )]
    fn run_verification(
        &self,
        Parameters(p): Parameters<SheetRefParams>,
    ) -> Result<String, ErrorData> {
        let engine = self.doc.engine.lock().unwrap();
        let project = engine.project();
        let sheets: Vec<_> = match p.sheet_id {
            Some(id) => vec![project
                .sheet(id)
                .ok_or_else(|| ErrorData::invalid_params("sheet not found", None))?],
            None => project.sheets.iter().collect(),
        };
        let mut diags = Vec::new();
        for sheet in sheets {
            diags.extend(madake_core::verify::verify_sheet(
                sheet,
                &sheet_symbol_defs(sheet),
            ));
        }
        json_ok(&diags)
    }

    #[tool(description = "部品表(BOM)CSVを指定パスに書き出す")]
    fn export_bom(&self, Parameters(p): Parameters<ExportPathParams>) -> Result<String, ErrorData> {
        let engine = self.doc.engine.lock().unwrap();
        let csv = madake_core::reports::bom_csv(engine.project());
        std::fs::write(&p.path, csv).map_err(internal)?;
        json_ok(&serde_json::json!({ "written": p.path }))
    }

    #[tool(description = "電線リストCSVを指定パスに書き出す")]
    fn export_wire_list(
        &self,
        Parameters(p): Parameters<ExportPathParams>,
    ) -> Result<String, ErrorData> {
        let engine = self.doc.engine.lock().unwrap();
        let csv = madake_core::reports::wire_list_csv(engine.project());
        std::fs::write(&p.path, csv).map_err(internal)?;
        json_ok(&serde_json::json!({ "written": p.path }))
    }

    #[tool(description = "シートをJIS図枠つきSVGとして指定パスに書き出す")]
    fn export_svg(&self, Parameters(p): Parameters<ExportSvgParams>) -> Result<String, ErrorData> {
        let sheet_id = self.resolve_sheet(p.sheet_id)?;
        let engine = self.doc.engine.lock().unwrap();
        let sheet = engine
            .project()
            .sheet(sheet_id)
            .ok_or_else(|| ErrorData::invalid_params("sheet not found", None))?;
        let svg = madake_core::svg::sheet_to_svg(sheet, &sheet_symbol_defs(sheet));
        std::fs::write(&p.path, svg).map_err(internal)?;
        json_ok(&serde_json::json!({ "written": p.path }))
    }

    #[tool(description = "シートをJIS図枠つきPDF(印刷品質、フォント埋め込み)として指定パスに書き出す")]
    fn export_pdf(&self, Parameters(p): Parameters<ExportSvgParams>) -> Result<String, ErrorData> {
        let sheet_id = self.resolve_sheet(p.sheet_id)?;
        let engine = self.doc.engine.lock().unwrap();
        let sheet = engine
            .project()
            .sheet(sheet_id)
            .ok_or_else(|| ErrorData::invalid_params("sheet not found", None))?;
        let pdf = madake_core::pdf::sheet_to_pdf(sheet, &sheet_symbol_defs(sheet))
            .map_err(internal)?;
        std::fs::write(&p.path, pdf).map_err(internal)?;
        json_ok(&serde_json::json!({ "written": p.path }))
    }

    #[tool(
        description = "DC動作点シミュレーション(ngspice)を実行し、各ネットの電圧(min/max)・各部品の電流/電力を返す。open_switchesでスイッチ/接点を開路にしたwhat-if解析ができる。負荷電流はシンボル属性current_a、電源はbattery"
    )]
    fn simulate_op(
        &self,
        Parameters(p): Parameters<SimulateOpParams>,
    ) -> Result<String, ErrorData> {
        let sheet_id = self.resolve_sheet(p.sheet_id)?;
        let engine = self.doc.engine.lock().unwrap();
        let sheet = engine
            .project()
            .sheet(sheet_id)
            .ok_or_else(|| ErrorData::invalid_params("sheet not found", None))?;
        let result =
            madake_core::sim::simulate_op(sheet, &sheet_symbol_defs(sheet), &p.open_switches)
                .map_err(internal)?;
        json_ok(&result)
    }

    #[tool(
        description = "部品DB(グローバル共有マスタ)を検索する。queryは型番・名称・メーカの部分一致、categoryは完全一致。部品のsymbol_id/rated_current_aはplace_symbolやexecute_commandsでの配置に使える"
    )]
    fn search_parts(
        &self,
        Parameters(p): Parameters<SearchPartsParams>,
    ) -> Result<String, ErrorData> {
        let db = self.parts.lock().unwrap();
        json_ok(
            &db.search_parts(p.query.as_deref().unwrap_or(""), p.category.as_deref())
                .map_err(internal)?,
        )
    }

    #[tool(description = "部品DBへ部品を登録・更新する(part_noが一意キー)")]
    fn upsert_part(
        &self,
        Parameters(p): Parameters<madake_core::parts::Part>,
    ) -> Result<String, ErrorData> {
        let db = self.parts.lock().unwrap();
        db.upsert_part(&p).map_err(internal)?;
        json_ok(&serde_json::json!({ "ok": true, "part_no": p.part_no }))
    }

    #[tool(description = "部品DBから部品を削除する")]
    fn delete_part(&self, Parameters(p): Parameters<PartNoParams>) -> Result<String, ErrorData> {
        let db = self.parts.lock().unwrap();
        let deleted = db.delete_part(&p.part_no).map_err(internal)?;
        json_ok(&serde_json::json!({ "deleted": deleted }))
    }

    #[tool(
        description = "KiCad回路図(.kicad_sch)を読み込み、現在のプロジェクトを置き換える。変換結果の要約(スキップしたシンボル等)を返す"
    )]
    fn import_kicad(&self, Parameters(p): Parameters<ExportPathParams>) -> Result<String, ErrorData> {
        let input = std::fs::read_to_string(&p.path).map_err(internal)?;
        let name = std::path::Path::new(&p.path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "KiCadインポート".into());
        let (project, report) =
            madake_core::kicad::import_kicad_sch(&input, &name).map_err(internal)?;
        let patch = self.doc.engine.lock().unwrap().replace_project(project);
        let _ = self.doc.patches.send(patch.clone());
        json_ok(&serde_json::json!({ "patch": patch, "report": report }))
    }

    #[tool(description = "直前の編集を取り消す")]
    fn undo(&self) -> Result<String, ErrorData> {
        let patch = self.doc.undo().map_err(internal)?;
        json_ok(&serde_json::json!({ "undone": patch.is_some() }))
    }

    #[tool(description = "取り消した編集をやり直す")]
    fn redo(&self) -> Result<String, ErrorData> {
        let patch = self.doc.redo().map_err(internal)?;
        json_ok(&serde_json::json!({ "redone": patch.is_some() }))
    }
}

#[tool_handler]
impl ServerHandler for MadakeMcp {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.instructions = Some(
            "MadakeCAD - 産業用電気図面CAD。開いているプロジェクトの図面(シンボル配置・配線)を \
             読み取り・編集できる。座標系は用紙mm単位・左上原点。編集はundo/redo履歴に乗り、\
             UIへリアルタイム反映される。"
                .into(),
        );
        info
    }
}

/// 内蔵サーバーを起動する(127.0.0.1:port)。/mcp = AI用MCP、/api/v1 = Link API。
/// Tauriのasyncランタイム上でspawnして使う。
///
/// `agent`はUI(Tauri IPC)と共有するエージェントマネージャ。Link APIの
/// `/api/v1/agent/*`は同じマネージャを叩くので、ブラウザ検証でもUIと同じ会話を見る。
pub async fn serve(
    doc: SharedDoc,
    agent: std::sync::Arc<madake_agent::AgentManager>,
    parts: SharedParts,
    port: u16,
) -> std::io::Result<()> {
    let mcp_doc = doc.clone();
    let mcp_parts = parts.clone();
    let service = StreamableHttpService::new(
        move || Ok(MadakeMcp::new(mcp_doc.clone(), mcp_parts.clone())),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    let router = axum::Router::new()
        .nest_service("/mcp", service)
        .merge(link_api::router(doc, agent, parts));
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await
}
