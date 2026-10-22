//! madake-mcp: MadakeCADに内蔵されるMCPサーバー。
//!
//! Streamable HTTP (127.0.0.1:port/mcp) で公開し、Claude Code / Claude Desktop等の
//! MCPクライアントが起動中のMadakeCADドキュメントを直接読み書きできる。
//! 全ての編集はmadake-coreのCommandエンジンを通るため、UI操作と同じundo/redo履歴に乗り、
//! patchブロードキャスト経由でUIにリアルタイム反映される。

pub mod agent;
pub mod link_api;
pub mod tool_bridge;

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use madake_core::{resolve_symbol, sheet_symbol_defs, Command, EditOrigin, Engine, Entity, Patch, Point, SymbolInstance, Wire};
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

/// 部品表 (BOM拡張) の引き当て表を部品DBから作る。UI・MCP・Link APIの帳票出力が共通で使う。
/// 部品DBが読めなければ空の表 (部品表のカタログ欄が空欄になるだけで、出力は止めない)。
pub fn bom_catalog(
    project: &madake_core::model::Project,
    parts: &SharedParts,
) -> madake_core::reports::BomCatalog {
    let numbers = madake_core::reports::bom_part_numbers(project);
    let db = parts.lock().unwrap();
    db.bom_catalog(numbers.iter().map(String::as_str)).unwrap_or_else(|e| {
        eprintln!("部品表の引き当てに部品DBを使えませんでした: {e}");
        Default::default()
    })
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

    /// 複数コマンドを**1回の編集**として実行し、patchをブロードキャストする (undo一発)。
    pub fn execute_batch_as(
        &self,
        cmds: Vec<Command>,
        origin: EditOrigin,
    ) -> madake_core::Result<Patch> {
        let patch = self.engine.lock().unwrap().execute_batch(cmds, origin)?;
        let _ = self.patches.send(patch.clone());
        Ok(patch)
    }

    /// 整えバリアントの開始: 元シートを`count`枚複製して入れる (1回の編集)。
    /// 戻り値は適用したpatchと、案ごとの情報 (複製シートid・ラベル・id対応表)。
    ///
    /// コマンドの組み立てと適用は**同じロックの中**で行う: 間でロックを放すと、その隙の
    /// 編集が複製に入らないまま複製が古い姿で入ってしまう。
    pub fn start_variants(
        &self,
        sheet_id: Uuid,
        count: usize,
        origin: EditOrigin,
    ) -> Result<(Patch, madake_core::variants::VariantRun), String> {
        let (patch, run) = {
            let mut engine = self.engine.lock().unwrap();
            let (commands, run) = {
                let project = engine.project();
                let sheet = project
                    .sheet(sheet_id)
                    .ok_or_else(|| format!("sheet not found: {sheet_id}"))?;
                madake_core::variants::start_commands(sheet, count, project.sheets.len())?
            };
            let patch = engine
                .execute_batch(commands, origin)
                .map_err(|e| e.to_string())?;
            (patch, run)
        };
        let _ = self.patches.send(patch.clone());
        Ok((patch, run))
    }

    /// 整えバリアントの終了: `chosen`があればその案を元シートへ写し戻して全複製を消す (採用)、
    /// 無ければ全複製を消すだけ (破棄)。どちらも1回の編集=undo一発。
    ///
    /// 実行情報 (`run`) はクライアントが往復させるので、写し戻す前に
    /// [`madake_core::variants::validate_run`]で今の図面と噛み合うか確かめる。組み立てと
    /// 適用は`start_variants`と同じ理由で同じロックの中で行う。
    pub fn finish_variants(
        &self,
        run: &madake_core::variants::VariantRun,
        chosen: Option<Uuid>,
        origin: EditOrigin,
    ) -> Result<Patch, String> {
        let sheet_ids: Vec<Uuid> = run.variants.iter().map(|v| v.sheet_id).collect();
        let patch = {
            let mut engine = self.engine.lock().unwrap();
            let commands = {
                let project = engine.project();
                madake_core::variants::validate_run(project, run, chosen)?;
                match chosen {
                    Some(chosen_id) => {
                        let info = run
                            .variants
                            .iter()
                            .find(|v| v.sheet_id == chosen_id)
                            .ok_or_else(|| format!("variant not found: {chosen_id}"))?;
                        let original = project
                            .sheet(run.original_sheet_id)
                            .ok_or_else(|| format!("sheet not found: {}", run.original_sheet_id))?;
                        let chosen_sheet = project
                            .sheet(chosen_id)
                            .ok_or_else(|| format!("sheet not found: {chosen_id}"))?;
                        madake_core::variants::adopt_commands(
                            original,
                            chosen_sheet,
                            &info.id_map,
                            &sheet_ids,
                        )
                    }
                    None => madake_core::variants::discard_commands(&sheet_ids),
                }
            };
            engine
                .execute_batch(commands, origin)
                .map_err(|e| e.to_string())?
        };
        let _ = self.patches.send(patch.clone());
        Ok(patch)
    }

    /// 選択したエンティティから回路マクロを**組み立てるだけ**(ファイルには書かない)。
    ///
    /// 保存ダイアログのプレビューと、⌘Cの無名マクロ(メモリ上のクリップボード)が使う。
    pub fn build_macro(
        &self,
        sheet_id: Uuid,
        entity_ids: &[Uuid],
        meta: &madake_core::macros::MacroMeta,
    ) -> madake_core::Result<madake_core::macros::Macro> {
        let engine = self.engine.lock().unwrap();
        madake_core::macros::save_macro(engine.project(), sheet_id, entity_ids, meta)
    }

    /// 選択したエンティティを回路マクロとして保存する(ユーザー領域へJSONを書き出す)。
    ///
    /// 図面は変更しない(読み取りのみ)ので履歴には乗らない。保存したマクロと
    /// 書き出し先パスを返す。`meta.variant`と`meta.id`が既存マクロを指すときは、
    /// そのマクロへバリアントを足す (登録済みのキーなら置き換える)。
    pub fn save_macro(
        &self,
        sheet_id: Uuid,
        entity_ids: &[Uuid],
        meta: &madake_core::macros::MacroMeta,
    ) -> madake_core::Result<(madake_core::macros::Macro, String)> {
        let existing = match (&meta.variant, meta.id.trim()) {
            (Some(_), id) if !id.is_empty() => madake_core::macros::find(id),
            _ => None,
        };
        let m = {
            let engine = self.engine.lock().unwrap();
            madake_core::macros::save_macro_into(
                existing.as_ref(),
                engine.project(),
                sheet_id,
                entity_ids,
                meta,
            )?
        };
        let path = madake_core::macros::write_macro(&m)?;
        Ok((m, path.display().to_string()))
    }

    /// ユーザーシンボルを削除する(ファイルを消す)。
    ///
    /// ライブラリはファイルのドメインなので**undoでは戻らない**。まだ図面で使われて
    /// いるシンボルは`force`なしでは消さず、使用数を添えて拒否する(消すと、置いてある
    /// 部品が定義を失って描けなくなるため)。戻り値は(消したパス, 削除時点の使用数)。
    pub fn delete_user_symbol(
        &self,
        id: &str,
        force: bool,
    ) -> madake_core::Result<(String, usize)> {
        let in_use = {
            let engine = self.engine.lock().unwrap();
            madake_core::symbol_library::usage_count(engine.project(), id)
        };
        if in_use > 0 && !force {
            return Err(madake_core::CoreError::InvalidCommand(format!(
                "シンボル「{id}」は図面で{in_use}個使われています(force=trueで削除できます)"
            )));
        }
        let path = madake_core::symbol_library::delete(id)?;
        Ok((path.display().to_string(), in_use))
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
        value_set: Option<&str>,
        sheet_id: Uuid,
        at: Point,
        rotation: u16,
        origin: EditOrigin,
    ) -> madake_core::Result<Patch> {
        let patch = madake_core::macros::apply(
            &mut self.engine.lock().unwrap(),
            macro_id,
            variant,
            value_set,
            sheet_id,
            at,
            rotation,
            origin,
        )?;
        let _ = self.patches.send(patch.clone());
        Ok(patch)
    }

    /// マクロ**そのもの**をシートへ挿入する(ライブラリのidを引かない)。
    ///
    /// ⌘C/Vの無名マクロのように、ファイルに無いマクロを置くための入口。挿入の中身は
    /// [`Self::apply_macro`]と同じで、**1回の編集** = undo一発で戻る。
    #[allow(clippy::too_many_arguments)]
    pub fn insert_macro(
        &self,
        m: &madake_core::macros::Macro,
        variant: Option<&str>,
        value_set: Option<&str>,
        sheet_id: Uuid,
        at: Point,
        rotation: u16,
        origin: EditOrigin,
    ) -> madake_core::Result<Patch> {
        let patch = madake_core::macros::insert_macro(
            &mut self.engine.lock().unwrap(),
            m,
            variant,
            value_set,
            sheet_id,
            at,
            rotation,
            origin,
        )?;
        let _ = self.patches.send(patch.clone());
        Ok(patch)
    }

    /// PLC I/O割付表を丸ごと置き換える(1回の編集 = undo一発で元の表に戻る)。
    pub fn set_plc_assignments(
        &self,
        assignments: Vec<madake_core::PlcAssignment>,
        origin: EditOrigin,
    ) -> madake_core::Result<Patch> {
        self.execute_as(Command::SetPlcAssignments { assignments }, origin)
    }

    /// CSVからPLC I/O割付表を取り込む(対象モジュールの行だけ置き換え、undo一発で戻る)。
    pub fn import_plc_assignments_csv(
        &self,
        module_ref: &str,
        csv: &str,
        origin: EditOrigin,
    ) -> madake_core::Result<Patch> {
        let patch = madake_core::plc::import_assignments_csv(
            &mut self.engine.lock().unwrap(),
            module_ref,
            csv,
            origin,
        )?;
        let _ = self.patches.send(patch.clone());
        Ok(patch)
    }

    /// 電線品番表 (線色+線径→品番) のCSVを取り込む。**Commandを通す**のでundoで戻る。
    pub fn import_wire_parts_csv(
        &self,
        csv: &str,
        origin: EditOrigin,
    ) -> madake_core::Result<Patch> {
        let patch = madake_core::wire_parts::import_wire_parts_csv(
            &mut self.engine.lock().unwrap(),
            csv,
            origin,
        )?;
        let _ = self.patches.send(patch.clone());
        Ok(patch)
    }

    /// PLC I/O図面 (ラダーページ) を生成する(**1回の編集** = undo一発でページごと消える)。
    pub fn generate_plc_sheet(
        &self,
        module_ref: &str,
        spec: &madake_core::plc::PlcModuleSpec,
        options: &madake_core::plc::PlcSheetOptions,
        origin: EditOrigin,
    ) -> madake_core::Result<Patch> {
        let patch = madake_core::plc::generate_plc_sheet(
            &mut self.engine.lock().unwrap(),
            module_ref,
            spec,
            options,
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
    /// 型番・値(例: "RLY-24V")。
    pub value: Option<String>,
    /// 追加属性。ブロック(`symbol_id="block"`)は
    /// `block_pins` = `{"left":[{"no":"IN1","name":"入力1"}],"right":[…]}`(任意で`top`/`bottom`)、
    /// 任意で `block_size` = `{"w":40,"h":30}` を渡して名前付きピンの機器を1回で作る。
    pub attrs: Option<std::collections::BTreeMap<String, String>>,
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
pub struct GetPinsParams {
    /// 対象シートID。省略時は先頭シート。
    pub sheet_id: Option<Uuid>,
    /// 参照記号 (例: "U1")。同じ参照記号の部品が複数あれば全部返す。
    pub reference: Option<String>,
    /// 部品のentity id。referenceの代わりに使う。
    pub entity_id: Option<Uuid>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ConnectPinsParams {
    /// 配線先シートID。省略時は先頭シート。
    pub sheet_id: Option<Uuid>,
    /// 始点のピン指定 "参照記号.ピン" (例: "U1.SDA")。ピンは番号でも名前でもよい。参照記号の代わりにentity idも書ける。
    pub from: String,
    /// 終点のピン指定 (fromと同じ書き方)。
    pub to: String,
    /// 経由点(mm)。順に通る。省略時は両端のピンの向きでZ字・L字・直線を選ぶ。
    pub via: Option<Vec<Point>>,
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
pub struct SaveUserSymbolParams {
    /// 保存するシンボル定義 (`duplicate_symbol`の戻り値を直して渡すのが近道)。
    pub symbol: madake_core::SymbolDef,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DuplicateSymbolParams {
    /// 複製元のシンボルid (同梱・ピン数可変・ユーザーのどれでもよい)。
    pub source_id: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DeleteUserSymbolParams {
    /// 削除するユーザーシンボルのid。
    pub id: String,
    /// 図面で使われていても消す。省略時false (使用中なら使用数を添えて拒否)。
    #[serde(default)]
    pub force: bool,
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
    /// 可変値のスロット。keyと行き先 (どのエンティティのvalue/attrs.<名前>か) を並べる。省略可。
    #[serde(default)]
    pub placeholders: Vec<madake_core::macros::MacroPlaceholder>,
    /// 値セット (例: 0.75kW / 1.5kW)。プレースホルダのkey→値。省略可。
    #[serde(default)]
    pub value_sets: Vec<madake_core::macros::MacroValueSet>,
    /// 既存のマクロ (`id`) へ足すバリアントのキー (A〜Z)。登録済みのキーなら置き換える。
    /// 省略すると新しいマクロとして保存する。
    #[serde(default)]
    pub variant: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ApplyMacroParams {
    /// 挿入するマクロのid (`list_macros`で得る)。
    pub macro_id: String,
    /// バリアントキー ("A"=既定)。省略時は既定。
    pub variant: Option<String>,
    /// 値セットid (`list_macros`のvalue_setsで得る)。指定すると定格・型番が一括設定される。省略時はマクロ保存時の値のまま。
    pub value_set: Option<String>,
    /// 挿入先シートID。省略時は先頭シート。
    pub sheet_id: Option<Uuid>,
    /// 基準点が来る位置 (mm)。
    pub at: Point,
    /// 回転角。0/90/180/270。省略時は0。
    pub rotation: Option<u16>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct PlcModuleParams {
    /// PLCモジュールの参照記号 (例 "PLC1")。省略時は割付のある全モジュール。
    pub module_ref: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SetPlcAssignmentsParams {
    /// 置き換え後の割付表 (プロジェクト全体)。
    pub assignments: Vec<madake_core::PlcAssignment>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GeneratePlcSheetParams {
    /// モジュールの参照記号 (例 "PLC1")。割付表の`module_ref`と揃える。
    pub module_ref: String,
    /// I/O点数 (1〜64)。
    pub points: usize,
    /// 入出力の種別 ("DI"=入力 / "DO"=出力)。
    pub kind: madake_core::plc::PlcIoKind,
    /// アドレスの接頭辞 ("X" / "Y" / "%I" / "%Q" / "I:" / "O:")。
    pub address_prefix: Option<String>,
    /// アドレス体系 ("mitsubishi" / "siemens" / "ab")。
    pub address_style: madake_core::plc::PlcAddressStyle,
    /// ラング間隔 (mm。2.5mmグリッドの正の倍数)。省略時は10mm。
    pub rung_spacing_mm: Option<f64>,
    /// 先頭で空けるラング位置の数。省略時は0。
    pub start_skip: Option<usize>,
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
pub struct ImportDxfParams {
    /// 読み込むDXFファイルのパス(絶対パス)。
    pub path: String,
    /// 配線とみなすレイヤ名。省略時は名前に WIRE を含むレイヤ(WIRENOは除く)。
    #[serde(default)]
    pub wire_layers: Vec<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ExportPdfBookParams {
    /// 出力先ファイルパス(絶対パス)。
    pub path: String,
    /// 回路図の後ろに付ける帳票。`wire-list` / `terminal-chart` / `terminal-diagram` / `bom` / `xref` / `plc-io` / `panel-placement`。省略時は帳票なし。
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
    /// 帳票の種類。`wire-list` / `terminal-chart` / `terminal-diagram` / `bom` / `xref` / `plc-io` / `panel-placement`。
    pub kind: madake_core::report_sheet::ReportKind,
    /// 出力形式。`csv`(表計算用) / `pdf`(図枠+表題欄付きのA4横ページ)。
    pub format: madake_core::report_sheet::ReportFormat,
    /// 端子台チャート・端子接続図の対象を1つの端子台に絞る場合のentity id。省略時は全端子台。
    #[serde(default)]
    pub entity_id: Option<Uuid>,
}

/// シート1枚を書き出す共通パラメータ (SVG / PDF / DXF / KiCad)。
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ExportSheetParams {
    /// 対象シートID。省略時は先頭シート。
    pub sheet_id: Option<Uuid>,
    /// 出力先ファイルパス(絶対パス)。
    pub path: String,
}

#[derive(Clone)]
pub struct MadakeMcp {
    doc: SharedDoc,
    parts: SharedParts,
    /// 外部の MCP クライアント向けの窓口だけが持つ (open_project / save_project がチャット履歴も扱う)。
    /// アプリ内エージェントのツール窓口 ([`crate::tool_bridge`]) では `None`。
    agent: Option<Arc<madake_agent::AgentManager>>,
}

impl MadakeMcp {
    pub fn new(doc: SharedDoc, parts: SharedParts) -> Self {
        Self { doc, parts, agent: None }
    }

    /// 外部の MCP クライアント向け (エージェント管理つき)。
    pub fn with_agent(doc: SharedDoc, parts: SharedParts, agent: Arc<madake_agent::AgentManager>) -> Self {
        Self { doc, parts, agent: Some(agent) }
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

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ProjectPathParams {
    /// プロジェクトファイルのパス (絶対パス、拡張子 .mdkproj)。
    pub path: String,
}

#[tool_router]
impl MadakeMcp {
    #[tool(
        description = "MadakeCAD のプロジェクトファイル (.mdkproj) を開き、いま開いている図面と置き換える (隣の <名前>.chat.json があればチャット履歴も読む)。置き換えは undo で戻らないので、必要なら先に save_project で保存すること。エージェントの応答中は断る。KiCad 回路図は import_kicad、DXF は import_dxf を使う"
    )]
    fn open_project(&self, Parameters(p): Parameters<ProjectPathParams>) -> Result<String, ErrorData> {
        agent::open_project_tool(&self.doc, self.agent.as_deref(), std::path::Path::new(&p.path))
            .map_err(|e| ErrorData::invalid_params(e, None))?;
        let engine = self.doc.engine.lock().unwrap();
        let project = engine.project();
        json_ok(&serde_json::json!({
            "opened": p.path,
            "name": project.name,
            "sheets": project.sheets.iter().map(|s| s.name.clone()).collect::<Vec<_>>(),
            "revision": engine.revision(),
        }))
    }

    #[tool(
        description = "いま開いている図面をプロジェクトファイル (.mdkproj) として保存する (画面の「保存」と同じ。チャット履歴があれば隣に <名前>.chat.json も書く)。同じパスがあれば上書きする"
    )]
    fn save_project(&self, Parameters(p): Parameters<ProjectPathParams>) -> Result<String, ErrorData> {
        agent::save_project_tool(&self.doc, self.agent.as_deref(), std::path::Path::new(&p.path))
            .map_err(|e| ErrorData::invalid_params(e, None))?;
        json_ok(&serde_json::json!({ "written": p.path }))
    }

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

    #[tool(description = "シンボルライブラリの一覧(id・名称・カテゴリ・ピン定義)を返す。同梱シンボルに続けて、ユーザーが作ったシンボル(~/MadakeCAD/symbols/。category=\"user\")も並ぶ。これに加え、動的ID `connector_{n}p` / `terminal_block_{n}p`(n=1..50、例: terminal_block_8p)でピン数可変のコネクタ・端子台を配置できる。欲しい記号が無いときはduplicate_symbolで似た記号を複製し、直してsave_user_symbolで保存すれば以後は同梱シンボルと同じように配置できる")]
    fn list_symbols(&self) -> Result<String, ErrorData> {
        json_ok(&madake_core::symbol_library::all())
    }

    #[tool(
        description = "既存シンボル(同梱・ピン数可変・ユーザーのどれでも)を複製した下書きを返す。ファイルにはまだ書かない。新しいシンボルは白紙からではなくこれで始めるのが既定の作り方(idは空きへ自動採番、名前は「〜のコピー」、分類はuser)。直したらsave_user_symbolで保存する"
    )]
    fn duplicate_symbol(
        &self,
        Parameters(p): Parameters<DuplicateSymbolParams>,
    ) -> Result<String, ErrorData> {
        let def = madake_core::symbol_library::duplicate_from(&p.source_id).map_err(internal)?;
        json_ok(&def)
    }

    #[tool(
        description = "ユーザーシンボルを~/MadakeCAD/symbols/へ保存する(部品挿入の「ユーザー」分類に並び、以後は同梱シンボルと同じように配置・配線・BOM・SVG出力できる)。保存前に検証し、落ちたら理由を全部返して**何も書かない**: 接続点は2.5mmグリッド上・番号必須・同じ点に2つ置けない(同じ番号を離れた2点に置くのは貫通端子として正しい)、参照記号スロット(TAG)必須、図形と接続点は最低1つずつ、idは同梱シンボル/ピン数可変シンボルと重複禁止。**シンボルライブラリはファイルのドメインでundo履歴に乗らない**(保存はundoで戻らない)。同じidで保存し直すと、すでに図面に置いてある部品もその定義で描き直される"
    )]
    fn save_user_symbol(
        &self,
        Parameters(p): Parameters<SaveUserSymbolParams>,
    ) -> Result<String, ErrorData> {
        match madake_core::symbol_library::save(&p.symbol) {
            Ok((symbol, path)) => json_ok(&serde_json::json!({
                "symbol": symbol,
                "path": path.display().to_string(),
            })),
            Err(errors) => Err(ErrorData::invalid_params(
                errors
                    .iter()
                    .map(|e| format!("[{}] {}", e.code, e.message))
                    .collect::<Vec<_>>()
                    .join(" / "),
                None,
            )),
        }
    }

    #[tool(
        description = "ユーザーシンボルを削除する(ファイルを消す。undoでは戻らない)。まだ図面で使われているシンボルは使用数を添えて拒否するので、force=trueで押し切るか、先に配置を消すこと"
    )]
    fn delete_user_symbol(
        &self,
        Parameters(p): Parameters<DeleteUserSymbolParams>,
    ) -> Result<String, ErrorData> {
        let (path, in_use) = self
            .doc
            .delete_user_symbol(&p.id, p.force)
            .map_err(internal)?;
        json_ok(&serde_json::json!({ "path": path, "in_use": in_use }))
    }

    #[tool(
        description = "任意の編集コマンド列を実行する。シート追加(add_sheet: name/size/orientation。kind=\"panel\"で2D盤レイアウトシート、省略時は回路図)/削除/改名、エンティティ追加(add_entity)/更新/削除/移動、表題欄設定(set_title_block)のほか、改訂欄の書き換え(set_revisions: 記号/日付/内容/承認の行リスト。表題欄のRevは最新行に連動)、シートの注記文字高の設定(set_sheet_annotation_text: height_mm=1.0〜3.5mm。配線注記・線番・ネットタグの大きさが一緒に変わる。nullで既定2.0mmへ戻す)、線番のネット単位自動採番(renumber_wires: mode=append で未採番のネットだけ追い番、mode=renumber で全振り直し、sheet_id省略で図面全体、start=開始番号)、線番の個別指定(set_wire_numbers)、PLC I/O割付表の置換(set_plc_assignments)、シートの用紙サイズ・向きの変更(set_sheet_paper: sheet_id/size/orientation。既存エンティティの座標もゾーン分割もそのまま。新しい用紙の外に出たエンティティはERC警告erc.outside_paperで報告するだけで動かさない)、プロジェクト名・番号・表題欄の一括設定(set_project_meta: name?/no?/company?/drawing_no?/title?。省略した欄は変更しない。noだけは3状態で、null を送るとプロジェクト番号を未設定へ戻す。company/drawing_no/titleは全シートの表題欄へ同じ値を書き込む)が使える。ハーネス境界は専用コマンドではなくadd_entityでkind=\"harness\"のエンティティ(points=矩形4点・name・note)を追加する(内包した配線が電線リストのハーネス列に載る)。**ハーネスは電線をまとめる境界専用**で、筐体・強調枠・注記の枠はadd_entityでkind=\"shape\"の図形(shape=\"rect\"は対角2点・\"line\"は両端2点・\"circle\"は中心と円周上の1点・\"polyline\"はN点。points・stroke=色トークン(default/accent/warning/success/muted)・stroke_width_mm=0.1〜2.0・fill=色トークンかnull・dashed=true/false)を使う。図形は電気的な意味を持たず、ネットリスト・ERC・部品表・電線リストのどれにも出てこないので、筐体をハーネスで囲んだときのように電線リストのハーネス列が汚れない。端子台のサドルジャンパも専用コマンドではなくupdate_entityで端子台シンボルのattrs[\"jumpers\"]を書き換える(値は隣り合う端子の対をカンマ区切りにした\"1-2,3-4\"。端子の並び順に正規化し、全て外すときは属性ごと削除)。**多段端子台 (terminal_block_{n}p_{levels}l) では段記号まで書く**: \"1a-1b\"=同じ極の隣の段、\"1a-2a\"=同じ段の隣の極。段記号なしの数字だけの指定と、存在しない端子・隣り合わない端子を指す指定は拒否され、端子台チェックがエラーとして報告する。ジャンパは端子台エディタ・端子台チャート・端子接続図と図面上のジャンパバーに反映され、undoで戻る。盤レイアウトシートの盤外形・DINレール・ダクト・フットプリントも専用コマンドではなくadd_entityでkind=\"panel_outline\"(at/w/h/name)・\"din_rail\"(y=取付線・x1・x2)・\"duct\"(at/w/h)・\"footprint\"(at/w/h/rotation=0か90/reference=回路図の参照記号/part_no/item_no=バルーンのアイテム番号)・\"panel_legend\"(at=凡例表の左上。中身はそのシートのフットプリントから自動で導出する)を追加する。盤のアイテム番号(バルーン)は renumber_panel_items(sheet_id) で読み順に1..Nを自動採番し、set_panel_item_numbers(sheet_id/numbers=[{footprint_id,item_no}]) で個別に指定できる(item_no=nullで消す)。番号は部品表(BOM)のアイテム番号列と盤内部品配置表(帳票 panel-placement)に連動する。回路図シートには電線品番凡例ブロックもadd_entityでkind=\"wire_legend\"(at=表の左上)として置ける。中身(線色・sq・電線品番・使用長合計)はそのシートの配線から自動で導出し、ネットリスト・ERC・部品表・電線リストのどれにも出てこない。部品表(BOM)には部品行の後ろへ電線品番ごとの集約行(数量欄にプロジェクト全体の使用長合計をm単位・小数第2位まで、品番が引けない配線は\"(未割当)\"の1行にまとめる)が自動で付く。各コマンドの完全なスキーマは入力スキーマを参照。実行結果のpatchを返す"
    )]
    fn execute_commands(
        &self,
        Parameters(p): Parameters<ExecuteCommandsParams>,
    ) -> Result<String, ErrorData> {
        let mut patches = Vec::new();
        for (i, cmd) in p.commands.into_iter().enumerate() {
            // 失敗したコマンドは黙って捨てない (G17): 何番目がなぜ落ちたかまで返す
            let patch = self.doc.execute(cmd).map_err(|e| {
                ErrorData::invalid_params(
                    format!(
                        "commands[{i}] が拒否されました: {e} (ここまでの{}件は適用済み。取り消すにはundoを{}回)",
                        patches.len(),
                        patches.len()
                    ),
                    None,
                )
            })?;
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
        description = "使える回路マクロ(現場で作った回路をそのまま再利用する部品)の一覧を返す。各マクロはid・名称(英/日)・分類・基準点・バリアントキー(A〜。Aが既定)・値セット(定格や型番を一括設定する値の組)を持つ。置き場は~/MadakeCAD/macros/。挿入はapply_macro、新しく作るのはsave_macro"
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
                    "placeholders": m.placeholders,
                    "value_sets": m.value_sets,
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
        description = "図面の一部(選択したエンティティ)を回路マクロとして保存する。座標は基準点(選択範囲の左下のピン)からの相対で保存され、線番は捨てられる(挿入先で振り直すため)。idとvariant(A〜Z)を渡すと既存マクロのバリアントとして足す(登録済みなら置き換え、プレースホルダは参照記号が同じシンボルへ結び付く)。図面は変更しない"
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
            placeholders: p.placeholders,
            value_sets: p.value_sets,
            variant: p.variant,
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
        description = "回路マクロをシートへ挿入する。基準点が指定位置(at)へ来るように置かれ、参照記号は図面で使用済みの次の番号へ自動で振り直される(2回挿入しても重複しない)。バリアントを指定すると代替回路が入り、値セット(value_set)を指定すると定格・型番などが一括設定される。挿入は1回の編集として履歴に乗るためundo一発で全体が戻る。挿入後は必ずrun_verificationで確認する"
    )]
    fn apply_macro(&self, Parameters(p): Parameters<ApplyMacroParams>) -> Result<String, ErrorData> {
        let sheet_id = self.resolve_sheet(p.sheet_id)?;
        let patch = self
            .doc
            .apply_macro(
                &p.macro_id,
                p.variant.as_deref(),
                p.value_set.as_deref(),
                sheet_id,
                p.at,
                p.rotation.unwrap_or(0),
                self.doc.mcp_origin(),
            )
            .map_err(internal)?;
        json_ok(&serde_json::json!({
            "macro_id": p.macro_id,
            "value_set": p.value_set,
            "sheet_id": sheet_id,
            "revision": patch.revision,
            "entities_added": patch.ops.len(),
        }))
    }

    #[tool(
        description = "PLC I/O割付表を読む。図面に置かれているPLCモジュール(参照記号・点数・DI/DO)と、各点のアドレス・信号名・コメント・接続先・線番を返す。信号名とコメントは割付表が正で、接続先と線番は図面の結線から読み取った値(図面を直せば変わる)。編集はset_plc_assignments、I/O図面の生成はgenerate_plc_sheet"
    )]
    fn get_plc_assignments(
        &self,
        Parameters(p): Parameters<PlcModuleParams>,
    ) -> Result<String, ErrorData> {
        let engine = self.doc.engine.lock().unwrap();
        let project = engine.project();
        let refs = match p.module_ref {
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
        json_ok(&serde_json::json!({
            "placed_modules": madake_core::plc::plc_modules(project),
            "modules": modules,
        }))
    }

    #[tool(
        description = "PLC I/O割付表を丸ごと置き換える(プロジェクト全体のリストを渡す)。各行は module_ref(モジュールの参照記号 例 PLC1)・address(例 X0 / %I0.0 / I:0/0)・signal_name・comment と id(UUID)。行の並びがそのままモジュールの点番号(1行目=点1)になる。接続先と線番は図面から導くのでここには書かない。1回の編集として履歴に乗るためundo一発で元の表に戻る"
    )]
    fn set_plc_assignments(
        &self,
        Parameters(p): Parameters<SetPlcAssignmentsParams>,
    ) -> Result<String, ErrorData> {
        let count = p.assignments.len();
        let patch = self
            .doc
            .set_plc_assignments(p.assignments, self.doc.mcp_origin())
            .map_err(internal)?;
        json_ok(&serde_json::json!({ "rows": count, "revision": patch.revision }))
    }

    #[tool(
        description = "PLC I/O図面(ラダーページ)を新しいシートとして生成する。縦バスから各点へ1点=1ラングで横ラングを引き、モジュールのシンボルと信号名のテキストを置く。割付表に行が足りなければアドレスを自動採番して表にも入れる(三菱=8進のX0,X1..X7,X10、Siemens=%I0.0のバイト.ビット、Allen-Bradley=I:0/0のワード/ビット)。生成は1回の編集として履歴に乗るためundo一発でページごと消える。v1が対応するのはラダー形式=縦バス+横ラング・配置方針=モジュールごとに新ラダーのみ"
    )]
    fn generate_plc_sheet(
        &self,
        Parameters(p): Parameters<GeneratePlcSheetParams>,
    ) -> Result<String, ErrorData> {
        let spec = madake_core::plc::PlcModuleSpec {
            points: p.points,
            kind: p.kind,
            address_prefix: p.address_prefix.unwrap_or_default(),
            address_style: p.address_style,
        };
        let options = madake_core::plc::PlcSheetOptions {
            rung_spacing_mm: p
                .rung_spacing_mm
                .unwrap_or(madake_core::plc::DEFAULT_RUNG_SPACING_MM),
            start_skip: p.start_skip.unwrap_or(0),
            ..Default::default()
        };
        let patch = self
            .doc
            .generate_plc_sheet(&p.module_ref, &spec, &options, self.doc.mcp_origin())
            .map_err(internal)?;
        json_ok(&serde_json::json!({
            "module_ref": p.module_ref,
            "points": p.points,
            "symbol_id": spec.symbol_id(),
            "revision": patch.revision,
        }))
    }

    #[tool(
        description = "シンボルをシートに配置する。作成されたエンティティidと、置いた部品のピン(pins: 番号・名前・用紙座標x/y・向きdir)を返す。配線はこの座標を使うかconnect_pinsでピン名を指定する。多ピンの機器は symbol_id=\"block\" + attrs.block_pins で名前付きピンのブロックとして作る"
    )]
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
            attrs: p.attrs.unwrap_or_default(),
        });
        let patch = self
            .doc
            .execute(Command::AddEntity { sheet_id, entity })
            .map_err(internal)?;
        let pins = {
            let engine = self.doc.engine.lock().unwrap();
            engine
                .project()
                .sheet(sheet_id)
                .and_then(|sheet| madake_core::pin_wiring::sheet_part_pins(sheet, id))
                .map(|p| p.pins)
                .unwrap_or_default()
        };
        json_ok(&serde_json::json!({ "entity_id": id, "revision": patch.revision, "pins": pins }))
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
            length_source: Default::default(),
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
        description = "配置済み部品のピンを返す。reference(参照記号)かentity_idで指定する。部品ごとにentity_id・参照記号・symbol_id・外形bbox(ブロックは5mm丸めの後)と、各ピンの番号・名前・用紙座標x/y・向きdir(回転と反転の後。電線が出ていく向き)を返す。ピン座標は自分で計算せずこのツールで読む"
    )]
    fn get_pins(&self, Parameters(p): Parameters<GetPinsParams>) -> Result<String, ErrorData> {
        let key = match (p.reference, p.entity_id) {
            (Some(r), _) => r,
            (None, Some(id)) => id.to_string(),
            (None, None) => {
                return Err(ErrorData::invalid_params(
                    "reference か entity_id のどちらかを指定する",
                    None,
                ))
            }
        };
        let sheet_id = self.resolve_sheet(p.sheet_id)?;
        let engine = self.doc.engine.lock().unwrap();
        let sheet = engine
            .project()
            .sheet(sheet_id)
            .ok_or_else(|| ErrorData::invalid_params("sheet not found", None))?;
        let parts = madake_core::pin_wiring::find_parts(sheet, &sheet_symbol_defs(sheet), &key);
        json_ok(&serde_json::json!({ "parts": parts }))
    }

    #[tool(
        description = "2本のピンをピン名で指定して配線する(座標は計算しなくてよい)。from/toは\"参照記号.ピン\"(例: \"U1.SDA\"、\"J1.2\"。ピンは番号でも名前でも、参照記号の代わりにentity idでもよい)。経由点viaが無ければ両端の向きでZ字・L字・直線の直角配線を引き、折れ点は2.5mmグリッドにそろえる。部品は避けないので、引いた後にget_tidy_metricsで重なりを確かめ、必要ならviaを足して引き直す。color/sq/length_m/part_noはdraw_wireと同じ。作成した配線idと頂点列pointsを返す"
    )]
    fn connect_pins(
        &self,
        Parameters(p): Parameters<ConnectPinsParams>,
    ) -> Result<String, ErrorData> {
        let sheet_id = self.resolve_sheet(p.sheet_id)?;
        let points = {
            let engine = self.doc.engine.lock().unwrap();
            let sheet = engine
                .project()
                .sheet(sheet_id)
                .ok_or_else(|| ErrorData::invalid_params("sheet not found", None))?;
            let defs = sheet_symbol_defs(sheet);
            let invalid = |e: String| ErrorData::invalid_params(e, None);
            let from = madake_core::pin_wiring::resolve_pin(sheet, &defs, &p.from).map_err(invalid)?;
            let to = madake_core::pin_wiring::resolve_pin(sheet, &defs, &p.to).map_err(invalid)?;
            if from.entity_id == to.entity_id && from.pin.number == to.pin.number {
                return Err(ErrorData::invalid_params(
                    format!("from と to が同じピンです ({})", p.from),
                    None,
                ));
            }
            madake_core::pin_wiring::route(
                Point::new(from.pin.x, from.pin.y),
                from.pin.dir,
                Point::new(to.pin.x, to.pin.y),
                to.pin.dir,
                p.via.as_deref().unwrap_or_default(),
            )
        };
        let id = Uuid::new_v4();
        let entity = Entity::Wire(Wire {
            id,
            points: points.clone(),
            color: p.color.unwrap_or_else(|| "black".into()),
            sq: p.sq.unwrap_or(0.3),
            length_m: p.length_m,
            length_source: Default::default(),
            part_no: p.part_no,
            net: None,
        });
        let patch = self
            .doc
            .execute(Command::AddEntity { sheet_id, entity })
            .map_err(internal)?;
        json_ok(&serde_json::json!({ "entity_id": id, "points": points, "revision": patch.revision }))
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
        description = "図面の「整い具合」を数値で返す。crossings=配線の交差数(線分どうしが面で交わっている箇所。端点で出会う接続・T分岐は交差に含まない)、label_overlaps=文字の重なり数(文字が他の文字やシンボル外形に重なっている組の数。辺で接しているだけは数えない)、symbol_overlaps=シンボル外形どうしの重なり数、off_grid=2.5mmグリッドから外れているシンボル原点・配線頂点の数。全て0が理想。整えループの目標値として使う: 整える前に測り、編集したらもう一度測って数値が減ったか確かめ、改善が止まるか3回で終える。**測るシートのsheet_idは必ず指定する** (省略すると先頭シートを測るので、整えバリアントの比較のようにシートが複数あると別のシートの数値を見てしまう)"
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
        description = "帳票1種を1ファイルへ書き出す。kind= wire-list(From-To電線リスト: From/To/嵌合相手/備考/線番/色/sq/長さ/品番/ハーネス。嵌合相手はG14の`mates_with`宣言、備考はネットラベルを1対1で辿れなかった行に付く`multi`の印) / terminal-chart(端子台チャート) / terminal-diagram(端子接続図。外部側=左・内部側=右のグラフィカル図) / bom(部品表。型番を部品DBで引き当てたメーカ・品名・単価・通貨・金額・購入先の列と通貨ごとの合計行付き) / xref(クロスリファレンス表)。format= csv または pdf(図枠+表題欄付きA4横。行が多ければ自動でページ分割)。端子接続図は図面のためCSV不可。entity_idで端子台1台に絞れる。戻り値のcountはCSVなら行数、PDFならページ数"
    )]
    fn export_report(
        &self,
        Parameters(p): Parameters<ExportReportParams>,
    ) -> Result<String, ErrorData> {
        let engine = self.doc.engine.lock().unwrap();
        let catalog = bom_catalog(engine.project(), &self.parts);
        let (bytes, count) = madake_core::report_sheet::report_bytes_with(
            engine.project(),
            p.kind,
            p.format,
            p.entity_id,
            &catalog,
        )
        .map_err(|e| ErrorData::invalid_params(e.to_string(), None))?;
        std::fs::write(&p.path, bytes).map_err(internal)?;
        json_ok(&serde_json::json!({ "written": p.path, "count": count }))
    }

    #[tool(description = "部品表(BOM)CSVを指定パスに書き出す。型番を部品DBで引き当て、メーカ・品名・単価・通貨・金額・購入先の列と通貨ごとの合計行を付ける")]
    fn export_bom(&self, Parameters(p): Parameters<ExportPathParams>) -> Result<String, ErrorData> {
        let engine = self.doc.engine.lock().unwrap();
        let catalog = bom_catalog(engine.project(), &self.parts);
        let csv = madake_core::reports::bom_csv_with(engine.project(), &catalog);
        std::fs::write(&p.path, csv).map_err(internal)?;
        json_ok(&serde_json::json!({ "written": p.path }))
    }

    #[tool(description = "From-To電線リストCSVを指定パスに書き出す(列: シート/From/To/嵌合相手/備考/線番/線色/線径sq/長さm/電線品番/ハーネス)。export_report(kind=wire-list, format=csv)と同じ中身")]
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
    fn export_svg(&self, Parameters(p): Parameters<ExportSheetParams>) -> Result<String, ErrorData> {
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
    fn export_pdf(&self, Parameters(p): Parameters<ExportSheetParams>) -> Result<String, ErrorData> {
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
        let catalog = bom_catalog(engine.project(), &self.parts);
        let svgs = madake_core::pdf::project_pdf_pages_with(engine.project(), &options, &catalog);
        let pages = svgs.len();
        let pdf = madake_core::pdf::svgs_to_pdf(&svgs).map_err(internal)?;
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

    #[tool(
        description = "DXF(AutoCAD Electrical / EPLANが書き出した図面、またはMadakeCADのDXF)を読み込み、現在のプロジェクトを置き換える。配線レイヤの線分→配線、MDK_<symbol_id>ブロック→シンボル、WIRENOレイヤの文字→線番。未対応ブロックはスキップ報告"
    )]
    fn import_dxf(&self, Parameters(p): Parameters<ImportDxfParams>) -> Result<String, ErrorData> {
        let input = std::fs::read_to_string(&p.path).map_err(internal)?;
        let name = std::path::Path::new(&p.path)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "DXFインポート".into());
        let options = madake_core::dxf::DxfImportOptions { wire_layers: p.wire_layers };
        let (project, report) =
            madake_core::dxf::import_dxf(&input, &name, &options).map_err(internal)?;
        let patch = self.doc.engine.lock().unwrap().replace_project(project);
        let _ = self.doc.patches.send(patch.clone());
        json_ok(&serde_json::json!({ "patch": patch, "report": report }))
    }

    #[tool(description = "シートをDXF(AutoCAD 2000形式。AutoCAD Electrical / EPLANで開ける。配線=WIRESレイヤのLINE、シンボル=MDK_<id>ブロック+TAG1/CAT属性、線番=WIRENOレイヤ)として指定パスに書き出す")]
    fn export_dxf(&self, Parameters(p): Parameters<ExportSheetParams>) -> Result<String, ErrorData> {
        let engine = self.doc.engine.lock().unwrap();
        let project = engine.project();
        let sheet = match p.sheet_id {
            Some(id) => project.sheet(id),
            None => project.sheets.first(),
        }
        .ok_or_else(|| internal("sheet not found"))?;
        let sheet_id = sheet.id;
        let (dxf, report) =
            madake_core::dxf::project_sheet_to_dxf(project, sheet_id, &sheet_symbol_defs(sheet))
                .ok_or_else(|| internal("sheet not found"))?;
        std::fs::write(&p.path, dxf).map_err(internal)?;
        json_ok(&serde_json::json!({ "written": p.path, "report": report }))
    }

    #[tool(description = "シートをKiCad回路図(.kicad_sch、KiCad 9形式。シンボル図形はファイルに埋め込むのでKiCad側にライブラリ不要)として指定パスに書き出す。線番はネットのラベルになる")]
    fn export_kicad(&self, Parameters(p): Parameters<ExportSheetParams>) -> Result<String, ErrorData> {
        let engine = self.doc.engine.lock().unwrap();
        let project = engine.project();
        let sheet = match p.sheet_id {
            Some(id) => project.sheet(id),
            None => project.sheets.first(),
        }
        .ok_or_else(|| internal("sheet not found"))?;
        let (sch, report) = madake_core::kicad::export_kicad_sch(sheet, &sheet_symbol_defs(sheet));
        std::fs::write(&p.path, sch).map_err(internal)?;
        json_ok(&serde_json::json!({ "written": p.path, "report": report }))
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
    let mcp_agent = Arc::clone(&agent);
    let service = StreamableHttpService::new(
        move || Ok(MadakeMcp::with_agent(mcp_doc.clone(), mcp_parts.clone(), Arc::clone(&mcp_agent))),
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
