//! madake-mcp: MadakeCADに内蔵されるMCPサーバー。
//!
//! Streamable HTTP (127.0.0.1:port/mcp) で公開し、Claude Code / Claude Desktop等の
//! MCPクライアントが起動中のMadakeCADドキュメントを直接読み書きできる。
//! 全ての編集はmadake-coreのCommandエンジンを通るため、UI操作と同じundo/redo履歴に乗り、
//! patchブロードキャスト経由でUIにリアルタイム反映される。

pub mod agent;
pub mod link_api;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use madake_core::{builtin_symbols, resolve_symbol, sheet_symbol_defs, Command, EditOrigin, Engine, Entity, Patch, Point, SymbolInstance, Wire};
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
    /// 実行中のエージェントターン数。`>0`の間にMCP経由で届いた編集は
    /// [`EditOrigin::Agent`]として記録する(入口はUI・外部クライアントと共通なので
    /// 時間で見分ける)。
    agent_turns: Arc<AtomicUsize>,
}

impl SharedDoc {
    pub fn new(engine: Engine) -> Self {
        let (patches, _) = broadcast::channel(256);
        Self {
            engine: Arc::new(Mutex::new(engine)),
            patches,
            agent_turns: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// MCPツール経由の編集の入口。
    ///
    /// 由来はエージェントのターン実行中なら[`EditOrigin::Agent`]、そうでなければ
    /// 外部MCPクライアントとみなして[`EditOrigin::Mcp`]。
    pub fn execute(&self, cmd: Command) -> madake_core::Result<Patch> {
        self.execute_as(cmd, self.mcp_origin())
    }

    /// UI(Tauri IPC)からの編集の入口。由来は[`EditOrigin::User`]。
    pub fn execute_user(&self, cmd: Command) -> madake_core::Result<Patch> {
        self.execute_as(cmd, EditOrigin::User)
    }

    /// 外部クライアント(Link API・madake CLI・FreeCADアドオン)からの編集の入口。
    /// 由来は[`EditOrigin::Mcp`]。
    pub fn execute_external(&self, cmd: Command) -> madake_core::Result<Patch> {
        self.execute_as(cmd, EditOrigin::Mcp)
    }

    /// 由来を明示してコマンドを実行し、patchをブロードキャストする。
    pub fn execute_as(&self, cmd: Command, origin: EditOrigin) -> madake_core::Result<Patch> {
        let patch = self.engine.lock().unwrap().execute_as(cmd, origin)?;
        let _ = self.patches.send(patch.clone());
        Ok(patch)
    }

    /// いまMCP経由で届いた編集に付ける由来。
    pub fn mcp_origin(&self) -> EditOrigin {
        if self.agent_turns.load(Ordering::SeqCst) > 0 {
            EditOrigin::Agent
        } else {
            EditOrigin::Mcp
        }
    }

    /// エージェントのターン実行が始まった([`madake_agent::DocBridge`]から呼ばれる)。
    pub fn begin_agent_turn(&self) {
        self.agent_turns.fetch_add(1, Ordering::SeqCst);
    }

    /// エージェントのターン実行が終わった(中断時も必ず呼ばれる)。
    pub fn end_agent_turn(&self) {
        let _ = self
            .agent_turns
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1));
    }

    /// 開始テンプレートをシートへ適用する(**1回の編集** = undo一発で戻る)。
    ///
    /// 由来は呼び出し経路で決まる: UI=[`EditOrigin::User`]、エージェント/外部=
    /// [`Self::mcp_origin`]。
    pub fn apply_template(
        &self,
        template_id: &str,
        sheet_id: Uuid,
        origin: EditOrigin,
    ) -> madake_core::Result<Patch> {
        let patch = madake_core::templates::apply(
            &mut self.engine.lock().unwrap(),
            template_id,
            sheet_id,
            origin,
        )?;
        let _ = self.patches.send(patch.clone());
        Ok(patch)
    }

    /// 選択したエンティティを回路マクロとして保存する(ユーザー領域へJSONを書き出す)。
    ///
    /// 図面は変更しない(読み取りのみ)ので履歴には乗らない。保存したマクロと
    /// 書き出し先パスを返す。
    pub fn save_macro(
        &self,
        sheet_id: Uuid,
        entity_ids: &[Uuid],
        meta: &madake_core::macros::MacroMeta,
    ) -> madake_core::Result<(madake_core::macros::Macro, String)> {
        let m = {
            let engine = self.engine.lock().unwrap();
            madake_core::macros::save_macro(engine.project(), sheet_id, entity_ids, meta)?
        };
        let path = madake_core::macros::write_macro(&m)?;
        Ok((m, path.display().to_string()))
    }

    /// 回路マクロをシートへ挿入する(**1回の編集** = undo一発で戻る)。
    ///
    /// 由来は呼び出し経路で決まる: UI=[`EditOrigin::User`]、エージェント/外部=
    /// [`Self::mcp_origin`]。
    #[allow(clippy::too_many_arguments)]
    pub fn apply_macro(
        &self,
        macro_id: &str,
        variant: Option<&str>,
        sheet_id: Uuid,
        at: Point,
        rotation: u16,
        origin: EditOrigin,
    ) -> madake_core::Result<Patch> {
        let patch = madake_core::macros::apply(
            &mut self.engine.lock().unwrap(),
            macro_id,
            variant,
            sheet_id,
            at,
            rotation,
            origin,
        )?;
        let _ = self.patches.send(patch.clone());
        Ok(patch)
    }

    /// undo深さの区間`[start, end)`にあるエージェント編集だけを巻き戻す。
    ///
    /// 逆Commandの適用として実行されるためpatchも配信され、UIへそのまま反映される。
    pub fn revert_agent_edits(
        &self,
        start_depth: usize,
        end_depth: usize,
    ) -> madake_core::Result<Option<madake_core::Reverted>> {
        let reverted =
            self.engine
                .lock()
                .unwrap()
                .revert_range(start_depth, end_depth, EditOrigin::Agent)?;
        if let Some(r) = &reverted {
            let _ = self.patches.send(r.patch.clone());
        }
        Ok(reverted)
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
pub struct ApplyTemplateParams {
    /// 適用するテンプレートのid (`list_templates`で得る)。
    pub template_id: String,
    /// 適用先シートID。省略時は先頭シート。
    pub sheet_id: Option<Uuid>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SaveMacroParams {
    /// マクロにするエンティティid(選択範囲)。
    pub entity_ids: Vec<Uuid>,
    /// 英語名(必須)。
    pub name: String,
    /// 安定id。省略時は名前から作る。
    pub sheet_id: Option<Uuid>,
    pub id: Option<String>,
    pub name_ja: Option<String>,
    pub description: Option<String>,
    pub description_ja: Option<String>,
    /// 挿入ダイアログでの分類 (例: "motor")。
    pub category: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ApplyMacroParams {
    /// 挿入するマクロのid (`list_macros`で得る)。
    pub macro_id: String,
    /// バリアントキー ("A"=既定)。省略時は既定。
    pub variant: Option<String>,
    /// 挿入先シートID。省略時は先頭シート。
    pub sheet_id: Option<Uuid>,
    /// 基準点が来る位置 (mm)。
    pub at: Point,
    /// 回転角。0/90/180/270。省略時は0。
    pub rotation: Option<u16>,
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
pub struct ExportPdfBookParams {
    /// 出力先ファイルパス(絶対パス)。
    pub path: String,
    /// 回路図の後ろに付ける帳票。`wire-list` / `terminal-chart` / `terminal-diagram` / `bom` / `xref`。省略時は帳票なし。
    #[serde(default)]
    pub include_reports: Vec<madake_core::report_sheet::ReportKind>,
    /// 表紙を付けるか。省略時は付ける。
    #[serde(default)]
    pub cover: Option<bool>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TerminalListParams {
    /// 対象シートID。省略時はプロジェクト全体の端子台。
    pub sheet_id: Option<Uuid>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TerminalRefParams {
    /// 端子台シンボルのentity id (list_terminal_blocksで取得)。
    pub entity_id: Uuid,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ExportReportParams {
    /// 出力先ファイルパス(絶対パス)。
    pub path: String,
    /// 帳票の種類。`wire-list` / `terminal-chart` / `terminal-diagram` / `bom` / `xref`。
    pub kind: madake_core::report_sheet::ReportKind,
    /// 出力形式。`csv`(表計算用) / `pdf`(図枠+表題欄付きのA4横ページ)。
    pub format: madake_core::report_sheet::ReportFormat,
    /// 端子台チャート・端子接続図の対象を1つの端子台に絞る場合のentity id。省略時は全端子台。
    #[serde(default)]
    pub entity_id: Option<Uuid>,
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

/// 公開しているMCPツールの一覧(ツール名 → 説明文)。
///
/// 説明文はエージェントがツールを選ぶ唯一の手がかりなので、用途を書き漏らしていないか
/// テストから確かめられるようにここで公開する。
pub fn tool_descriptions() -> Vec<(String, String)> {
    MadakeMcp::tool_router()
        .list_all()
        .into_iter()
        .map(|tool| {
            (
                tool.name.to_string(),
                tool.description.map(|d| d.to_string()).unwrap_or_default(),
            )
        })
        .collect()
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
        description = "任意の編集コマンド列を実行する。シート追加/削除/改名、エンティティ追加(add_entity)/更新/削除/移動、表題欄設定(set_title_block)のほか、改訂欄の書き換え(set_revisions: 記号/日付/内容/承認の行リスト。表題欄のRevは最新行に連動)、線番のネット単位自動採番(renumber_wires: mode=append で未採番のネットだけ追い番、mode=renumber で全振り直し、sheet_id省略で図面全体、start=開始番号)、線番の個別指定(set_wire_numbers)が使える。ハーネス境界は専用コマンドではなくadd_entityでkind=\"harness\"のエンティティ(points=矩形4点・name・note)を追加する(内包した配線が電線リストのハーネス列に載る)。端子台のサドルジャンパも専用コマンドではなくupdate_entityで端子台シンボルのattrs[\"jumpers\"]を書き換える(値は隣接端子の対をカンマ区切りにした\"1-2,3-4\"。小-大順に正規化し、全て外すときは属性ごと削除)。ジャンパは端子台エディタ・端子台チャート・端子接続図の3か所に反映され、undoで戻る。各コマンドの完全なスキーマは入力スキーマを参照。実行結果のpatchを返す"
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

    #[tool(
        description = "開始テンプレート(白紙から作図を始めるための回路の雛形)の一覧を返す。各テンプレートはid・名称(英/日)・説明・入っているコマンド数を持つ。同梱は24V制御基本・モータ起動回路・非常停止回路で、ユーザーが~/MadakeCAD/templates/へ置いたテンプレートも含む。適用はapply_template"
    )]
    fn list_templates(&self) -> Result<String, ErrorData> {
        let list = madake_core::templates::list();
        let templates: Vec<serde_json::Value> = list
            .templates
            .iter()
            .map(|t| {
                serde_json::json!({
                    "id": t.id,
                    "name": t.name,
                    "name_ja": t.name_ja,
                    "description": t.description,
                    "description_ja": t.description_ja,
                    "builtin": t.builtin,
                    "command_count": t.commands.len(),
                })
            })
            .collect();
        json_ok(&serde_json::json!({ "templates": templates, "issues": list.issues }))
    }

    #[tool(
        description = "開始テンプレートをシートへ適用する(部品・配線・ネットラベル一式が入る)。テンプレートはERC指摘ゼロの状態で入るので、そこから編集を足していく。適用は1回の編集として履歴に乗るためundo一発で全体が戻る。適用後は必ずrun_verificationで確認する"
    )]
    fn apply_template(
        &self,
        Parameters(p): Parameters<ApplyTemplateParams>,
    ) -> Result<String, ErrorData> {
        let sheet_id = self.resolve_sheet(p.sheet_id)?;
        let patch = self
            .doc
            .apply_template(&p.template_id, sheet_id, self.doc.mcp_origin())
            .map_err(internal)?;
        json_ok(&serde_json::json!({
            "template_id": p.template_id,
            "sheet_id": sheet_id,
            "revision": patch.revision,
            "entities_added": patch.ops.len(),
        }))
    }

    #[tool(
        description = "使える回路マクロ(現場で作った回路をそのまま再利用する部品)の一覧を返す。各マクロはid・名称(英/日)・分類・基準点・バリアントキー(A〜。Aが既定)を持つ。置き場は~/MadakeCAD/macros/。挿入はapply_macro、新しく作るのはsave_macro"
    )]
    fn list_macros(&self) -> Result<String, ErrorData> {
        let list = madake_core::macros::list();
        let macros: Vec<serde_json::Value> = list
            .macros
            .iter()
            .map(|m| {
                serde_json::json!({
                    "id": m.id,
                    "name": m.name,
                    "name_ja": m.name_ja,
                    "description": m.description,
                    "description_ja": m.description_ja,
                    "category": m.category,
                    "base_point": m.base_point,
                    "variants": m.variant_keys(),
                    "command_count": m.commands.len(),
                })
            })
            .collect();
        json_ok(&serde_json::json!({
            "macros": macros,
            "issues": list.issues,
            "user_dir": list.user_dir,
        }))
    }

    #[tool(
        description = "図面の一部(選択したエンティティ)を回路マクロとして保存する。座標は基準点(選択範囲の左下のピン)からの相対で保存され、線番は捨てられる(挿入先で振り直すため)。図面は変更しない"
    )]
    fn save_macro(&self, Parameters(p): Parameters<SaveMacroParams>) -> Result<String, ErrorData> {
        let sheet_id = self.resolve_sheet(p.sheet_id)?;
        let meta = madake_core::macros::MacroMeta {
            id: p.id.unwrap_or_default(),
            name: p.name,
            name_ja: p.name_ja.unwrap_or_default(),
            description: p.description.unwrap_or_default(),
            description_ja: p.description_ja.unwrap_or_default(),
            category: p.category.unwrap_or_default(),
        };
        let (m, path) = self
            .doc
            .save_macro(sheet_id, &p.entity_ids, &meta)
            .map_err(internal)?;
        json_ok(&serde_json::json!({
            "id": m.id,
            "base_point": m.base_point,
            "command_count": m.commands.len(),
            "path": path,
        }))
    }

    #[tool(
        description = "回路マクロをシートへ挿入する。基準点が指定位置(at)へ来るように置かれ、参照記号は図面で使用済みの次の番号へ自動で振り直される(2回挿入しても重複しない)。バリアントを指定すると代替回路が入る。挿入は1回の編集として履歴に乗るためundo一発で全体が戻る。挿入後は必ずrun_verificationで確認する"
    )]
    fn apply_macro(&self, Parameters(p): Parameters<ApplyMacroParams>) -> Result<String, ErrorData> {
        let sheet_id = self.resolve_sheet(p.sheet_id)?;
        let patch = self
            .doc
            .apply_macro(
                &p.macro_id,
                p.variant.as_deref(),
                sheet_id,
                p.at,
                p.rotation.unwrap_or(0),
                self.doc.mcp_origin(),
            )
            .map_err(internal)?;
        json_ok(&serde_json::json!({
            "macro_id": p.macro_id,
            "sheet_id": sheet_id,
            "revision": patch.revision,
            "entities_added": patch.ops.len(),
        }))
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
        // シート指定なし=プロジェクト全体。ネットラベル関連はシートを跨いだ統合ネットで評価する
        let Some(id) = p.sheet_id else {
            return json_ok(&madake_core::verify::verify_project(project));
        };
        let sheet = project
            .sheet(id)
            .ok_or_else(|| ErrorData::invalid_params("sheet not found", None))?;
        json_ok(&madake_core::verify::verify_sheet(
            sheet,
            &sheet_symbol_defs(sheet),
        ))
    }

    #[tool(
        description = "図面の「整い具合」を数値で返す。crossings=配線の交差数(線分どうしが面で交わっている箇所。端点で出会う接続・T分岐は交差に含まない)、label_overlaps=文字の重なり数(文字が他の文字やシンボル外形に重なっている組の数。辺で接しているだけは数えない)、symbol_overlaps=シンボル外形どうしの重なり数、off_grid=2.5mmグリッドから外れているシンボル原点・配線頂点の数。全て0が理想。整えループの目標値として使う: 整える前に測り、編集したらもう一度測って数値が減ったか確かめ、改善が止まるか3回で終える。sheet_id省略時は先頭シート"
    )]
    fn get_tidy_metrics(
        &self,
        Parameters(p): Parameters<SheetRefParams>,
    ) -> Result<String, ErrorData> {
        let sheet_id = self.resolve_sheet(p.sheet_id)?;
        let engine = self.doc.engine.lock().unwrap();
        let sheet = engine
            .project()
            .sheet(sheet_id)
            .ok_or_else(|| ErrorData::invalid_params("sheet not found", None))?;
        json_ok(&madake_core::tidy::tidy_metrics(
            sheet,
            &sheet_symbol_defs(sheet),
        ))
    }

    #[tool(
        description = "プロジェクト内の端子台の一覧(entity id・所在シート・参照記号・型番・極数・ジャンパ指定)を返す。sheet_id省略で全シート。get_terminal_chart / check_terminal_block / export_reportのentity_idはここで得る"
    )]
    fn list_terminal_blocks(
        &self,
        Parameters(p): Parameters<TerminalListParams>,
    ) -> Result<String, ErrorData> {
        let engine = self.doc.engine.lock().unwrap();
        json_ok(&madake_core::terminal_chart::terminal_block_infos(
            engine.project(),
            p.sheet_id,
        ))
    }

    #[tool(
        description = "端子台1台のチャート(端子番号順の行: 内部側=盤内・外部側=盤外の接続先、線番、電線の色/sq/品番、ハーネス、未結線=予備端子、ジャンパ)を返す。内部/外部は端子台の左ピン接続=内部側・右ピン接続=外部側(回転後の左右)"
    )]
    fn get_terminal_chart(
        &self,
        Parameters(p): Parameters<TerminalRefParams>,
    ) -> Result<String, ErrorData> {
        let engine = self.doc.engine.lock().unwrap();
        let chart =
            madake_core::terminal_chart::terminal_chart_in_project(engine.project(), p.entity_id)
                .ok_or_else(|| ErrorData::invalid_params("terminal block not found", None))?;
        json_ok(&chart)
    }

    #[tool(
        description = "端子台チェックを実行しDiagnostic配列を返す(未結線の端子=予備端子の情報、存在しない端子番号を指すジャンパなどの不整合)"
    )]
    fn check_terminal_block(
        &self,
        Parameters(p): Parameters<TerminalRefParams>,
    ) -> Result<String, ErrorData> {
        let engine = self.doc.engine.lock().unwrap();
        json_ok(&madake_core::terminal_chart::check_terminal_block_in_project(
            engine.project(),
            p.entity_id,
        ))
    }

    #[tool(
        description = "帳票1種を1ファイルへ書き出す。kind= wire-list(From-To電線リスト: From/To/線番/色/sq/長さ/品番/ハーネス) / terminal-chart(端子台チャート) / terminal-diagram(端子接続図。外部側=左・内部側=右のグラフィカル図) / bom(部品表) / xref(クロスリファレンス表)。format= csv または pdf(図枠+表題欄付きA4横。行が多ければ自動でページ分割)。端子接続図は図面のためCSV不可。entity_idで端子台1台に絞れる。戻り値のcountはCSVなら行数、PDFならページ数"
    )]
    fn export_report(
        &self,
        Parameters(p): Parameters<ExportReportParams>,
    ) -> Result<String, ErrorData> {
        let engine = self.doc.engine.lock().unwrap();
        let (bytes, count) = madake_core::report_sheet::report_bytes(
            engine.project(),
            p.kind,
            p.format,
            p.entity_id,
        )
        .map_err(|e| ErrorData::invalid_params(e.to_string(), None))?;
        std::fs::write(&p.path, bytes).map_err(internal)?;
        json_ok(&serde_json::json!({ "written": p.path, "count": count }))
    }

    #[tool(description = "部品表(BOM)CSVを指定パスに書き出す")]
    fn export_bom(&self, Parameters(p): Parameters<ExportPathParams>) -> Result<String, ErrorData> {
        let engine = self.doc.engine.lock().unwrap();
        let csv = madake_core::reports::bom_csv(engine.project());
        std::fs::write(&p.path, csv).map_err(internal)?;
        json_ok(&serde_json::json!({ "written": p.path }))
    }

    #[tool(description = "From-To電線リストCSVを指定パスに書き出す(列: シート/From/To/線番/線色/線径sq/長さm/電線品番/ハーネス)。export_report(kind=wire-list, format=csv)と同じ中身")]
    fn export_wire_list(
        &self,
        Parameters(p): Parameters<ExportPathParams>,
    ) -> Result<String, ErrorData> {
        let engine = self.doc.engine.lock().unwrap();
        let csv = madake_core::reports::wire_list_csv(engine.project());
        std::fs::write(&p.path, csv).map_err(internal)?;
        json_ok(&serde_json::json!({ "written": p.path }))
    }

    #[tool(description = "シートをJIS図枠つきSVGとして指定パスに書き出す(改訂欄・線番・ハーネス破線囲み・ネットラベルのシート間クロスリファレンス「/2.B3」を含む)")]
    fn export_svg(&self, Parameters(p): Parameters<ExportSvgParams>) -> Result<String, ErrorData> {
        let sheet_id = self.resolve_sheet(p.sheet_id)?;
        let engine = self.doc.engine.lock().unwrap();
        let project = engine.project();
        let sheet = project
            .sheet(sheet_id)
            .ok_or_else(|| ErrorData::invalid_params("sheet not found", None))?;
        // プロジェクト文脈で描くとネットラベルにシート間クロスリファレンスが入る
        let svg = madake_core::svg::project_sheet_to_svg(project, sheet_id, &sheet_symbol_defs(sheet))
            .ok_or_else(|| ErrorData::invalid_params("sheet not found", None))?;
        std::fs::write(&p.path, svg).map_err(internal)?;
        json_ok(&serde_json::json!({ "written": p.path }))
    }

    #[tool(description = "シートをJIS図枠つきPDF(印刷品質、フォント埋め込み)として指定パスに書き出す。内容はSVG出力と同一(改訂欄・線番・ハーネス・シート間クロスリファレンス込み)")]
    fn export_pdf(&self, Parameters(p): Parameters<ExportSvgParams>) -> Result<String, ErrorData> {
        let sheet_id = self.resolve_sheet(p.sheet_id)?;
        let engine = self.doc.engine.lock().unwrap();
        let project = engine.project();
        let sheet = project
            .sheet(sheet_id)
            .ok_or_else(|| ErrorData::invalid_params("sheet not found", None))?;
        let pdf =
            madake_core::pdf::project_sheet_to_pdf(project, sheet_id, &sheet_symbol_defs(sheet))
                .ok_or_else(|| ErrorData::invalid_params("sheet not found", None))?
                .map_err(internal)?;
        std::fs::write(&p.path, pdf).map_err(internal)?;
        json_ok(&serde_json::json!({ "written": p.path }))
    }

    #[tool(
        description = "図面一式を1つのPDFにまとめて指定パスに書き出す。ページ順は 表紙(プロジェクト名・図面一覧・最新改訂) → 回路図の全シート → 選択した帳票。include_reportsに wire-list(From-To電線リスト) / terminal-chart(端子台チャート) / terminal-diagram(端子接続図。端子台1つにつき1ページのグラフィカル図) / bom(部品表) / xref(クロスリファレンス表) を並べた順に帳票ページが付く。帳票はA4横の図枠付きページで、行が多ければ自動でページ分割される"
    )]
    fn export_pdf_book(
        &self,
        Parameters(p): Parameters<ExportPdfBookParams>,
    ) -> Result<String, ErrorData> {
        let engine = self.doc.engine.lock().unwrap();
        let options = madake_core::pdf::PdfBookOptions {
            include_reports: p.include_reports,
            cover: p.cover.unwrap_or(true),
        };
        let pages = madake_core::pdf::project_pdf_pages(engine.project(), &options).len();
        let pdf =
            madake_core::pdf::export_project_pdf(engine.project(), &options).map_err(internal)?;
        std::fs::write(&p.path, pdf).map_err(internal)?;
        json_ok(&serde_json::json!({ "written": p.path, "pages": pages }))
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
        description = "部品DB(グローバル共有マスタ)を検索する。部品の選定・比較・代替品の提案はこのツールで実在する部品を調べてから行う(型番を記憶で書かない)。queryは型番・名称・メーカの部分一致、categoryは完全一致。各部品は型番・メーカ・カテゴリ・定格電圧/定格電流・参考価格・購入先URL・データシートURL・既定シンボルidを持ち、比較表(定格・価格・購入先の差分)にそのまま使える。symbol_id/rated_current_aはplace_symbolやexecute_commandsでの配置に使える"
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
             UIへリアルタイム反映される。帳票はFrom-To電線リスト・端子台チャート・端子接続図・\
             部品表・クロスリファレンス表の5種で、export_report(CSV/図枠付きPDF)または\
             export_pdf_book(表紙+回路図全シート+帳票を1PDFへ)で出力する。"
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
