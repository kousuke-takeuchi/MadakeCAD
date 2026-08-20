//! madake-mcp: MadakeCADに内蔵されるMCPサーバー。
//!
//! Streamable HTTP (127.0.0.1:port/mcp) で公開し、Claude Code / Claude Desktop等の
//! MCPクライアントが起動中のMadakeCADドキュメントを直接読み書きできる。
//! 全ての編集はmadake-coreのCommandエンジンを通るため、UI操作と同じundo/redo履歴に乗り、
//! patchブロードキャスト経由でUIにリアルタイム反映される。

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use madake_core::{builtin_symbols, Command, Engine, Entity, Patch, Point, SymbolInstance, Wire};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{ErrorData, ServerCapabilities, ServerInfo};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use rmcp::{tool, tool_handler, tool_router, ServerHandler};
use tokio::sync::broadcast;
use uuid::Uuid;

/// UI(Tauri)とMCPが共有するドキュメント状態。
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
}

impl MadakeMcp {
    pub fn new(doc: SharedDoc) -> Self {
        Self { doc }
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

    #[tool(description = "シンボルライブラリの一覧(id・名称・カテゴリ・ピン定義)を返す")]
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
        if !builtin_symbols().iter().any(|s| s.id == p.symbol_id) {
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
    fn get_netlist(
        &self,
        Parameters(p): Parameters<SheetRefParams>,
    ) -> Result<String, ErrorData> {
        let sheet_id = self.resolve_sheet(p.sheet_id)?;
        let engine = self.doc.engine.lock().unwrap();
        let sheet = engine
            .project()
            .sheet(sheet_id)
            .ok_or_else(|| ErrorData::invalid_params("sheet not found", None))?;
        json_ok(&madake_core::netlist::extract_netlist(sheet, &builtin_symbols()))
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
    fn export_svg(
        &self,
        Parameters(p): Parameters<ExportSvgParams>,
    ) -> Result<String, ErrorData> {
        let sheet_id = self.resolve_sheet(p.sheet_id)?;
        let engine = self.doc.engine.lock().unwrap();
        let sheet = engine
            .project()
            .sheet(sheet_id)
            .ok_or_else(|| ErrorData::invalid_params("sheet not found", None))?;
        let svg = madake_core::svg::sheet_to_svg(sheet, &builtin_symbols());
        std::fs::write(&p.path, svg).map_err(internal)?;
        json_ok(&serde_json::json!({ "written": p.path }))
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

/// MCPサーバーを起動する(127.0.0.1:port/mcp)。Tauriのasyncランタイム上でspawnして使う。
pub async fn serve(doc: SharedDoc, port: u16) -> std::io::Result<()> {
    let service = StreamableHttpService::new(
        move || Ok(MadakeMcp::new(doc.clone())),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    let router = axum::Router::new().nest_service("/mcp", service);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await
}
