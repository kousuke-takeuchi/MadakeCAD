use serde::{Deserialize, Serialize};

use crate::model::*;
use crate::wire_no::{plan_sheet, reserve_sheet, Numberer, RenumberMode};
use crate::{CoreError, Result};

/// シリアライズ可能な編集コマンド。UI(Tauri IPC)とAI(MCP)の両方がこれを発行する。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    AddSheet {
        name: String,
        size: PaperSize,
        orientation: Orientation,
        /// シート種別 (省略時は回路図)。盤レイアウトシートは`"panel"`。
        #[serde(default)]
        kind: SheetKind,
    },
    RemoveSheet {
        sheet_id: SheetId,
    },
    /// 位置(index)を指定して**組み立て済みのシートを丸ごと挿入する**。
    /// 削除の逆操作(復元)であると同時に、生成した図面ページ(PLC I/O図面など)を
    /// 1コマンドで入れる口でもある(エンティティ込みで入るのでundo一発で消える)。
    RestoreSheet {
        index: usize,
        sheet: Box<Sheet>,
    },
    RenameSheet {
        sheet_id: SheetId,
        name: String,
    },
    SetTitleBlock {
        sheet_id: SheetId,
        title_block: TitleBlock,
    },
    SetRevisions {
        sheet_id: SheetId,
        revisions: Vec<Revision>,
    },
    /// シートの注記文字高 (mm) の設定 (G20)。配線注記・線番・ネットタグの大きさが変わる。
    /// `height_mm`がnullなら既定 ([`crate::model::DEFAULT_ANNOTATION_TEXT_MM`]) に戻す。
    /// 範囲は [`crate::model::MIN_ANNOTATION_TEXT_MM`]〜[`crate::model::MAX_ANNOTATION_TEXT_MM`]。
    SetSheetAnnotationText {
        sheet_id: SheetId,
        #[serde(default)]
        height_mm: Option<f64>,
    },
    /// シートの用紙サイズ・向きの変更 (G15)。既存エンティティの座標はそのまま
    /// (動かない・消えない)。ゾーン分割 (`zone_cols`/`zone_rows`) も**変えない**
    /// (シートが正。図枠テンプレートの差し替えと同じで、クロスリファレンスの
    /// ゾーン住所を用紙変更で書き換えないため)。逆コマンドはサイズと向きだけを元へ戻す。
    /// 新しい用紙の外に出たエンティティはERC警告 (`erc.outside_paper`) で報告する
    /// (このコマンドでは検出しない。検証は読み取り専用で行う)。
    SetSheetPaper {
        sheet_id: SheetId,
        size: PaperSize,
        orientation: Orientation,
    },
    AddEntity {
        sheet_id: SheetId,
        entity: Entity,
    },
    /// idで既存エンティティを完全置換。
    UpdateEntity {
        sheet_id: SheetId,
        entity: Entity,
    },
    DeleteEntities {
        sheet_id: SheetId,
        ids: Vec<EntityId>,
    },
    MoveEntities {
        sheet_id: SheetId,
        ids: Vec<EntityId>,
        dx: f64,
        dy: f64,
    },
    SetWireParts {
        wire_parts: Vec<WirePart>,
    },
    /// PLC I/O割付表の置換 (プロジェクト単位)。逆コマンドは置換前のリスト。
    SetPlcAssignments {
        assignments: Vec<PlcAssignment>,
    },
    /// 図枠テンプレートの差し替え (プロジェクト単位。M4仕様 §9)。全シートの図枠・
    /// 表題欄・改訂欄が一括で変わる。nullで既定 (`jis-a3`) へ戻す。
    /// 逆コマンドは差し替え前のid。
    SetFrameTemplate {
        #[serde(default)]
        frame_template: Option<String>,
    },
    /// 線番のネット単位自動採番。sheet_id省略時はプロジェクトの全シートが対象。
    RenumberWires {
        #[serde(default)]
        sheet_id: Option<SheetId>,
        mode: RenumberMode,
        #[serde(default = "crate::wire_no::default_start")]
        start: u32,
    },
    /// 線番の直接指定 (個別編集・自動採番の逆コマンド)。numberがnullなら線番を消す。
    SetWireNumbers {
        sheet_id: SheetId,
        numbers: Vec<WireNumber>,
    },
    /// 盤レイアウトシートのアイテム番号 (バルーン) の自動採番 (M4仕様 §8)。
    /// そのシートのフットプリントへ読み順 ([`crate::panel::item_order`]) で 1..N を振る。
    /// 逆コマンドは採番前の番号 (`SetPanelItemNumbers`)。
    RenumberPanelItems {
        sheet_id: SheetId,
    },
    /// アイテム番号の直接指定 (自動採番の逆コマンド・個別編集)。
    /// item_noがnullなら番号を消す (バルーンも消える)。
    SetPanelItemNumbers {
        sheet_id: SheetId,
        numbers: Vec<PanelItemNumber>,
    },
    /// プロジェクト名・番号と、全シート共通の表題欄の供給元となる項目の一括設定 (G16)。
    /// 各欄は省略 (キーごと書かない) すれば変更しない。`name`/`no`はプロジェクト単位、
    /// `company`/`drawing_no`/`title`は**全シートの表題欄**へ同じ値を書く
    /// (1コマンドでundo一発)。表題欄はシートごとに異なる値を持てるので、逆コマンドは
    /// シートごとの元の値を個別に復元する。
    SetProjectMeta {
        #[serde(default)]
        name: Option<String>,
        /// プロジェクト番号 ([`crate::model::Project::no`]) は**3状態**:
        /// キーを書かなければ変更しない / `null`なら未設定へ戻す (番号を消す) /
        /// 文字列ならその値にする。逆コマンドも同じ3状態で元の状態を正確に復元する。
        #[serde(
            default,
            deserialize_with = "double_option",
            skip_serializing_if = "Option::is_none"
        )]
        #[schemars(with = "Option<String>")]
        no: Option<Option<String>>,
        #[serde(default)]
        company: Option<String>,
        #[serde(default)]
        drawing_no: Option<String>,
        #[serde(default)]
        title: Option<String>,
    },
}

/// 「キーが無い」と「`null`」を区別して読む (`Option<Option<T>>`)。serdeの既定では
/// どちらも`None`になってしまうので、**変更しない**と**未設定へ戻す**を区別する欄に使う。
fn double_option<'de, T, D>(de: D) -> std::result::Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Deserialize::deserialize(de).map(Some)
}

/// エンジンからフロントエンドへ通知する差分。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Patch {
    /// 単調増加のドキュメント世代番号。
    pub revision: u64,
    pub ops: Vec<PatchOp>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum PatchOp {
    /// プロジェクト全体の置換(新規作成・ファイル読込時)。
    ProjectReplaced { project: Project },
    SheetAdded { index: usize, sheet: Sheet },
    SheetRemoved { sheet_id: SheetId },
    /// シートのメタ情報(名前・表題欄・改訂欄)更新。entitiesは含まない。
    SheetMetaUpdated { sheet: Sheet },
    EntityUpserted { sheet_id: SheetId, entity: Entity },
    EntityRemoved { sheet_id: SheetId, id: EntityId },
    WirePartsReplaced { wire_parts: Vec<WirePart> },
    /// PLC I/O割付表が置き換わった。
    PlcAssignmentsReplaced { assignments: Vec<PlcAssignment> },
    /// 図枠テンプレートが差し替わった (全シートの図枠を描き直す)。
    FrameTemplateChanged { frame_template: Option<String> },
    /// プロジェクト名・番号が変わった (G16)。表題欄側の変更は各シートの
    /// [`PatchOp::SheetMetaUpdated`]で別に届く。
    ProjectMetaChanged { name: String, no: Option<String> },
}

/// 編集の由来(誰の操作か)。undo履歴の各エントリに記録する。
///
/// ターン巻き戻し([`Engine::revert_range`])が「エージェントの編集だけ」を戻し、
/// 間に挟まったユーザーの手編集を保持するために使う。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum EditOrigin {
    /// UI操作(Tauri IPC)。既定
    #[default]
    User,
    /// 内蔵AIエージェントがターン実行中に行った編集
    Agent,
    /// 外部MCPクライアント・Link API・madake CLIからの編集
    Mcp,
}

/// [`Engine::revert_range`]の結果。
#[derive(Debug, Clone)]
pub struct Reverted {
    /// 逆適用した履歴エントリ数(= 巻き戻した編集コマンド数)
    pub commands: usize,
    /// 巻き戻しで生じた差分
    pub patch: Patch,
}

struct HistoryEntry {
    /// 適用したコマンド列(通常は1件。巻き戻しのように1操作=複数コマンドの場合もある)
    forward: Vec<Command>,
    inverse: Vec<Command>,
    origin: EditOrigin,
}

/// ドキュメントの単一の真実。全編集はexecute()を通る。
pub struct Engine {
    project: Project,
    revision: u64,
    undo_stack: Vec<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>,
}

impl Engine {
    pub fn new(project: Project) -> Self {
        Self {
            project,
            revision: 0,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// プロジェクトを丸ごと置換(新規作成・読込)。履歴はクリアされる。
    pub fn replace_project(&mut self, project: Project) -> Patch {
        self.project = project;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.revision += 1;
        Patch {
            revision: self.revision,
            ops: vec![PatchOp::ProjectReplaced {
                project: self.project.clone(),
            }],
        }
    }

    /// コマンドを実行し、undo履歴に積む(ユーザー操作として記録する)。
    pub fn execute(&mut self, cmd: Command) -> Result<Patch> {
        self.execute_as(cmd, EditOrigin::User)
    }

    /// 由来を明示してコマンドを実行する。
    ///
    /// 由来は履歴エントリに残り、[`Self::revert_range`]が対象を絞るのに使う。
    /// 呼び出し経路ごとに: UI(Tauri IPC)=[`EditOrigin::User`]、エージェントの
    /// ターン実行中=[`EditOrigin::Agent`]、外部MCP/Link API/CLI=[`EditOrigin::Mcp`]。
    pub fn execute_as(&mut self, cmd: Command, origin: EditOrigin) -> Result<Patch> {
        let (ops, inverse) = self.apply(&cmd)?;
        self.undo_stack.push(HistoryEntry {
            forward: vec![cmd],
            inverse,
            origin,
        });
        self.redo_stack.clear();
        self.revision += 1;
        Ok(Patch {
            revision: self.revision,
            ops,
        })
    }

    /// コマンド列を**まとめて1回の編集**として実行する(履歴エントリは1件)。
    ///
    /// テンプレートの適用のように「1操作で図面へ一式を入れる」編集に使う。
    /// [`Self::execute`]をN回呼ぶとundoもN回必要になるが、これなら**undo一発**で
    /// 全体が元へ戻る。
    ///
    /// 途中のコマンドが失敗したら適用前の図面へ戻して`Err`(部分適用しない・
    /// 履歴も積まない)。空のコマンド列は何もしない(revisionも進めない)。
    pub fn execute_batch(&mut self, cmds: Vec<Command>, origin: EditOrigin) -> Result<Patch> {
        if cmds.is_empty() {
            return Ok(Patch {
                revision: self.revision,
                ops: Vec::new(),
            });
        }
        let (ops, inverse) = self.apply_all(&cmds)?;
        self.undo_stack.push(HistoryEntry {
            forward: cmds,
            inverse,
            origin,
        });
        self.redo_stack.clear();
        self.revision += 1;
        Ok(Patch {
            revision: self.revision,
            ops,
        })
    }

    /// undo履歴に積まれている編集の由来を古い順に返す(検査・テスト用)。
    pub fn history_origins(&self) -> Vec<EditOrigin> {
        self.undo_stack.iter().map(|e| e.origin).collect()
    }

    /// undo深さの区間`[start_depth, end_depth)`にある`origin`由来の編集だけを
    /// 逆Commandとして適用し、巻き戻す。
    ///
    /// 履歴末尾からn回undoする方式と違い、**区間に挟まった他の由来の編集(ユーザーの
    /// 手編集)は保持する**。AIエージェントのターン単位の巻き戻しが使う。
    ///
    /// 規則:
    /// - 逆適用は新しい編集として通常の履歴に積まれる(由来は[`EditOrigin::User`]。
    ///   「元に戻す」はユーザー操作のため)。したがって**巻き戻し自体をundoで取り消せる**
    ///   一方、redo履歴は通常の編集と同じく破棄される
    /// - 逆適用が現在の図面と衝突した場合(対象が手で削除されている等)は
    ///   [`CoreError::RevertConflict`]を返し、**何も変更しない**(部分適用しない)
    /// - 区間に対象の編集が1件も無ければ`Ok(None)`(図面もrevisionも変わらない)
    pub fn revert_range(
        &mut self,
        start_depth: usize,
        end_depth: usize,
        origin: EditOrigin,
    ) -> Result<Option<Reverted>> {
        let end = end_depth.min(self.undo_stack.len());
        let start = start_depth.min(end);
        // 新しい編集から順に戻す(逆コマンドはLIFOで適用しないと前提が崩れる)
        let mut commands = 0usize;
        let mut inverses: Vec<Command> = Vec::new();
        for entry in self.undo_stack[start..end].iter().rev() {
            if entry.origin != origin {
                continue;
            }
            commands += 1;
            inverses.extend(entry.inverse.iter().cloned());
        }
        if inverses.is_empty() {
            return Ok(None);
        }
        let (ops, inverse) = self
            .apply_all(&inverses)
            .map_err(|e| CoreError::RevertConflict(e.to_string()))?;
        self.undo_stack.push(HistoryEntry {
            forward: inverses,
            inverse,
            origin: EditOrigin::User,
        });
        self.redo_stack.clear();
        self.revision += 1;
        Ok(Some(Reverted {
            commands,
            patch: Patch {
                revision: self.revision,
                ops,
            },
        }))
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// undo履歴に積まれている編集の数。
    ///
    /// `revision`はundo/redoでも進むため「ある区間で何コマンド積まれたか」を測るのに
    /// 使えない。区間の前後でこの深さを比べれば、増分がそのまま巻き戻しに必要な
    /// undo回数になる(AIエージェントのターン単位undoが利用する)。
    pub fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo(&mut self) -> Result<Option<Patch>> {
        let Some(entry) = self.undo_stack.pop() else {
            return Ok(None);
        };
        let mut ops = Vec::new();
        for inv in &entry.inverse {
            let (mut o, _) = self.apply(inv)?;
            ops.append(&mut o);
        }
        self.redo_stack.push(entry);
        self.revision += 1;
        Ok(Some(Patch {
            revision: self.revision,
            ops,
        }))
    }

    pub fn redo(&mut self) -> Result<Option<Patch>> {
        let Some(entry) = self.redo_stack.pop() else {
            return Ok(None);
        };
        let (ops, inverse) = self.apply_all(&entry.forward)?;
        self.undo_stack.push(HistoryEntry {
            forward: entry.forward,
            inverse,
            origin: entry.origin,
        });
        self.revision += 1;
        Ok(Some(Patch {
            revision: self.revision,
            ops,
        }))
    }

    /// コマンド列を順に適用する。**途中で失敗したら適用前の状態へ戻して`Err`**
    /// (中途半端に適用された図面を残さない)。
    ///
    /// 戻り値は (生成patch, 逆コマンド列)。逆コマンドは適用と逆順に並べてあるので、
    /// そのまま順に適用すれば元へ戻る。
    fn apply_all(&mut self, cmds: &[Command]) -> Result<(Vec<PatchOp>, Vec<Command>)> {
        let snapshot = self.project.clone();
        let mut ops = Vec::new();
        let mut groups: Vec<Vec<Command>> = Vec::new();
        for cmd in cmds {
            match self.apply(cmd) {
                Ok((mut o, inv)) => {
                    ops.append(&mut o);
                    groups.push(inv);
                }
                Err(e) => {
                    self.project = snapshot;
                    return Err(e);
                }
            }
        }
        groups.reverse();
        Ok((ops, groups.into_iter().flatten().collect()))
    }

    /// コマンドを適用し、(生成patch, 逆コマンド列) を返す。履歴には触れない。
    fn apply(&mut self, cmd: &Command) -> Result<(Vec<PatchOp>, Vec<Command>)> {
        match cmd {
            Command::AddSheet {
                name,
                size,
                orientation,
                kind,
            } => {
                let sheet = Sheet::new_kind(name, *size, *orientation, *kind);
                let inverse = vec![Command::RemoveSheet { sheet_id: sheet.id }];
                let index = self.project.sheets.len();
                self.project.sheets.push(sheet.clone());
                Ok((vec![PatchOp::SheetAdded { index, sheet }], inverse))
            }
            Command::RemoveSheet { sheet_id } => {
                let index = self
                    .project
                    .sheets
                    .iter()
                    .position(|s| s.id == *sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let sheet = self.project.sheets.remove(index);
                let inverse = vec![Command::RestoreSheet {
                    index,
                    sheet: Box::new(sheet),
                }];
                Ok((vec![PatchOp::SheetRemoved { sheet_id: *sheet_id }], inverse))
            }
            Command::RestoreSheet { index, sheet } => {
                let idx = (*index).min(self.project.sheets.len());
                self.project.sheets.insert(idx, (**sheet).clone());
                let inverse = vec![Command::RemoveSheet { sheet_id: sheet.id }];
                Ok((
                    vec![PatchOp::SheetAdded {
                        index: idx,
                        sheet: (**sheet).clone(),
                    }],
                    inverse,
                ))
            }
            Command::RenameSheet { sheet_id, name } => {
                let sheet = self
                    .project
                    .sheet_mut(*sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let old = sheet.name.clone();
                sheet.name = name.clone();
                let meta = sheet_meta(sheet);
                Ok((
                    vec![PatchOp::SheetMetaUpdated { sheet: meta }],
                    vec![Command::RenameSheet {
                        sheet_id: *sheet_id,
                        name: old,
                    }],
                ))
            }
            Command::SetTitleBlock {
                sheet_id,
                title_block,
            } => {
                let sheet = self
                    .project
                    .sheet_mut(*sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let old = sheet.title_block.clone();
                sheet.title_block = title_block.clone();
                let meta = sheet_meta(sheet);
                Ok((
                    vec![PatchOp::SheetMetaUpdated { sheet: meta }],
                    vec![Command::SetTitleBlock {
                        sheet_id: *sheet_id,
                        title_block: old,
                    }],
                ))
            }
            Command::SetRevisions {
                sheet_id,
                revisions,
            } => {
                let sheet = self
                    .project
                    .sheet_mut(*sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let old = sheet.revisions.clone();
                sheet.revisions = revisions.clone();
                let meta = sheet_meta(sheet);
                Ok((
                    vec![PatchOp::SheetMetaUpdated { sheet: meta }],
                    vec![Command::SetRevisions {
                        sheet_id: *sheet_id,
                        revisions: old,
                    }],
                ))
            }
            Command::SetSheetAnnotationText { sheet_id, height_mm } => {
                if let Some(h) = height_mm {
                    if !h.is_finite()
                        || *h < MIN_ANNOTATION_TEXT_MM
                        || *h > MAX_ANNOTATION_TEXT_MM
                    {
                        return Err(CoreError::InvalidCommand(format!(
                            "annotation text height out of range: {h} (expected {MIN_ANNOTATION_TEXT_MM}..{MAX_ANNOTATION_TEXT_MM} mm)"
                        )));
                    }
                }
                let sheet = self
                    .project
                    .sheet_mut(*sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let old = sheet.annotation_text_mm;
                sheet.annotation_text_mm = *height_mm;
                let meta = sheet_meta(sheet);
                Ok((
                    vec![PatchOp::SheetMetaUpdated { sheet: meta }],
                    vec![Command::SetSheetAnnotationText {
                        sheet_id: *sheet_id,
                        height_mm: old,
                    }],
                ))
            }
            Command::SetSheetPaper {
                sheet_id,
                size,
                orientation,
            } => {
                let sheet = self
                    .project
                    .sheet_mut(*sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let old_size = sheet.size;
                let old_orientation = sheet.orientation;
                sheet.size = *size;
                sheet.orientation = *orientation;
                // ゾーン分割はシートが正のまま触らない (図枠テンプレートの差し替えと同じ扱い。
                // クロスリファレンスの指すゾーン住所が用紙変更で書き換わらないように。docs/04)
                let meta = sheet_meta(sheet);
                Ok((
                    vec![PatchOp::SheetMetaUpdated { sheet: meta }],
                    vec![Command::SetSheetPaper {
                        sheet_id: *sheet_id,
                        size: old_size,
                        orientation: old_orientation,
                    }],
                ))
            }
            Command::AddEntity { sheet_id, entity } => {
                validate_entity(
                    entity,
                    self.project
                        .sheet(*sheet_id)
                        .ok_or(CoreError::SheetNotFound(*sheet_id))?,
                )?;
                let sheet = self
                    .project
                    .sheet_mut(*sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let id = entity.id();
                if sheet.entities.contains_key(&id) {
                    return Err(CoreError::InvalidCommand(format!(
                        "entity already exists: {id}"
                    )));
                }
                sheet.entities.insert(id, entity.clone());
                Ok((
                    vec![PatchOp::EntityUpserted {
                        sheet_id: *sheet_id,
                        entity: entity.clone(),
                    }],
                    vec![Command::DeleteEntities {
                        sheet_id: *sheet_id,
                        ids: vec![id],
                    }],
                ))
            }
            Command::UpdateEntity { sheet_id, entity } => {
                validate_entity(
                    entity,
                    self.project
                        .sheet(*sheet_id)
                        .ok_or(CoreError::SheetNotFound(*sheet_id))?,
                )?;
                let sheet = self
                    .project
                    .sheet_mut(*sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let id = entity.id();
                // 先に存在確認する: insertしてからエラーにすると、失敗したはずの
                // 更新でエンティティがシートへ紛れ込む
                if !sheet.entities.contains_key(&id) {
                    return Err(CoreError::EntityNotFound(id));
                }
                let old = sheet
                    .entities
                    .insert(id, entity.clone())
                    .expect("直前に存在確認済み");
                Ok((
                    vec![PatchOp::EntityUpserted {
                        sheet_id: *sheet_id,
                        entity: entity.clone(),
                    }],
                    vec![Command::UpdateEntity {
                        sheet_id: *sheet_id,
                        entity: old,
                    }],
                ))
            }
            Command::DeleteEntities { sheet_id, ids } => {
                let sheet = self
                    .project
                    .sheet_mut(*sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let mut ops = Vec::new();
                let mut inverse = Vec::new();
                for id in ids {
                    if let Some(old) = sheet.entities.remove(id) {
                        ops.push(PatchOp::EntityRemoved {
                            sheet_id: *sheet_id,
                            id: *id,
                        });
                        inverse.push(Command::AddEntity {
                            sheet_id: *sheet_id,
                            entity: old,
                        });
                    }
                }
                // 復元順は削除の逆順でなくても問題ない(idは独立)が、直感的に逆順にする。
                inverse.reverse();
                Ok((ops, inverse))
            }
            Command::MoveEntities {
                sheet_id,
                ids,
                dx,
                dy,
            } => {
                let sheet = self
                    .project
                    .sheet_mut(*sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let mut ops = Vec::new();
                let mut moved = Vec::new();
                for id in ids {
                    if let Some(e) = sheet.entities.get_mut(id) {
                        e.translate(*dx, *dy);
                        moved.push(*id);
                        ops.push(PatchOp::EntityUpserted {
                            sheet_id: *sheet_id,
                            entity: e.clone(),
                        });
                    }
                }
                Ok((
                    ops,
                    vec![Command::MoveEntities {
                        sheet_id: *sheet_id,
                        ids: moved,
                        dx: -dx,
                        dy: -dy,
                    }],
                ))
            }
            Command::SetWireParts { wire_parts } => {
                let old = self.project.wire_parts.clone();
                self.project.wire_parts = wire_parts.clone();
                Ok((
                    vec![PatchOp::WirePartsReplaced {
                        wire_parts: wire_parts.clone(),
                    }],
                    vec![Command::SetWireParts { wire_parts: old }],
                ))
            }
            Command::SetPlcAssignments { assignments } => {
                let old = self.project.plc_assignments.clone();
                self.project.plc_assignments = assignments.clone();
                Ok((
                    vec![PatchOp::PlcAssignmentsReplaced {
                        assignments: assignments.clone(),
                    }],
                    vec![Command::SetPlcAssignments { assignments: old }],
                ))
            }
            Command::SetFrameTemplate { frame_template } => {
                let old = self.project.frame_template.clone();
                self.project.frame_template = frame_template.clone();
                Ok((
                    vec![PatchOp::FrameTemplateChanged {
                        frame_template: frame_template.clone(),
                    }],
                    vec![Command::SetFrameTemplate {
                        frame_template: old,
                    }],
                ))
            }
            Command::RenumberWires {
                sheet_id,
                mode,
                start,
            } => {
                let targets: Vec<SheetId> = match sheet_id {
                    Some(id) => {
                        if self.project.sheet(*id).is_none() {
                            return Err(CoreError::SheetNotFound(*id));
                        }
                        vec![*id]
                    }
                    None => self.project.sheets.iter().map(|s| s.id).collect(),
                };
                // 1周目: 対象シート全体で保持される名前・線番を予約 (図面全体で番号が重複しない)
                let mut numberer = Numberer::new(*start);
                for id in &targets {
                    let sheet = self.project.sheet(*id).expect("checked above");
                    reserve_sheet(sheet, *mode, &mut numberer);
                }
                // 2周目: シート順に採番して書き込む
                let mut ops = Vec::new();
                let mut inverse = Vec::new();
                for id in &targets {
                    let sheet = self.project.sheet(*id).expect("checked above");
                    let plan = plan_sheet(sheet, *mode, &mut numberer);
                    let numbers: Vec<WireNumber> = plan
                        .into_iter()
                        .map(|(wire_id, number)| WireNumber {
                            wire_id,
                            number: Some(number),
                        })
                        .collect();
                    let (mut o, mut inv) = self.apply_wire_numbers(*id, &numbers)?;
                    ops.append(&mut o);
                    inverse.append(&mut inv);
                }
                Ok((ops, inverse))
            }
            Command::SetWireNumbers { sheet_id, numbers } => {
                self.apply_wire_numbers(*sheet_id, numbers)
            }
            Command::RenumberPanelItems { sheet_id } => {
                let sheet = self
                    .project
                    .sheet(*sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let numbers: Vec<PanelItemNumber> = crate::panel::renumber_plan(sheet)
                    .into_iter()
                    .map(|(footprint_id, item_no)| PanelItemNumber {
                        footprint_id,
                        item_no,
                    })
                    .collect();
                self.apply_panel_item_numbers(*sheet_id, &numbers)
            }
            Command::SetPanelItemNumbers { sheet_id, numbers } => {
                self.apply_panel_item_numbers(*sheet_id, numbers)
            }
            Command::SetProjectMeta {
                name,
                no,
                company,
                drawing_no,
                title,
            } => self.apply_project_meta(name, no, company, drawing_no, title),
        }
    }

    /// [`Command::SetProjectMeta`]の適用: プロジェクト単位の欄(名前・番号)は1つの
    /// [`PatchOp::ProjectMetaChanged`]で、表題欄の欄は**シートごとに**
    /// [`PatchOp::SheetMetaUpdated`]で届く。逆コマンドも同じ形に分かれるので、
    /// undoはプロジェクト単位の値とシートごとの表題欄をそれぞれ正しい元の値へ戻す。
    fn apply_project_meta(
        &mut self,
        name: &Option<String>,
        no: &Option<Option<String>>,
        company: &Option<String>,
        drawing_no: &Option<String>,
        title: &Option<String>,
    ) -> Result<(Vec<PatchOp>, Vec<Command>)> {
        let mut ops = Vec::new();
        let mut inverse = Vec::new();

        let mut inv_name = None;
        let mut inv_no = None;
        if let Some(n) = name {
            inv_name = Some(self.project.name.clone());
            self.project.name = n.clone();
        }
        if let Some(next) = no {
            // 「未設定」も1つの状態なので、そのまま逆コマンドへ持たせる
            // (undoで空文字にすり替わらない・nullで番号を消せる)
            inv_no = Some(self.project.no.clone());
            self.project.no = next.clone();
        }
        if inv_name.is_some() || inv_no.is_some() {
            ops.push(PatchOp::ProjectMetaChanged {
                name: self.project.name.clone(),
                no: self.project.no.clone(),
            });
            inverse.push(Command::SetProjectMeta {
                name: inv_name,
                no: inv_no,
                company: None,
                drawing_no: None,
                title: None,
            });
        }

        if company.is_some() || drawing_no.is_some() || title.is_some() {
            for sheet in &mut self.project.sheets {
                let old_tb = sheet.title_block.clone();
                let mut changed = false;
                if let Some(c) = company {
                    sheet.title_block.company = c.clone();
                    changed = true;
                }
                if let Some(d) = drawing_no {
                    sheet.title_block.drawing_no = d.clone();
                    changed = true;
                }
                if let Some(t) = title {
                    sheet.title_block.title = t.clone();
                    changed = true;
                }
                if changed {
                    ops.push(PatchOp::SheetMetaUpdated { sheet: sheet_meta(sheet) });
                    inverse.push(Command::SetTitleBlock {
                        sheet_id: sheet.id,
                        title_block: old_tb,
                    });
                }
            }
        }

        Ok((ops, inverse))
    }

    /// 指定フットプリントのアイテム番号を書き換える。値が変わるものだけをpatch・
    /// 逆コマンドに含める (同じ番号を振り直しても無駄なundoの段を作らない)。
    fn apply_panel_item_numbers(
        &mut self,
        sheet_id: SheetId,
        numbers: &[PanelItemNumber],
    ) -> Result<(Vec<PatchOp>, Vec<Command>)> {
        let sheet = self
            .project
            .sheet_mut(sheet_id)
            .ok_or(CoreError::SheetNotFound(sheet_id))?;
        let mut ops = Vec::new();
        let mut old = Vec::new();
        for PanelItemNumber {
            footprint_id,
            item_no,
        } in numbers
        {
            let Some(Entity::Footprint(fp)) = sheet.entities.get_mut(footprint_id) else {
                continue;
            };
            if fp.item_no == *item_no {
                continue;
            }
            old.push(PanelItemNumber {
                footprint_id: *footprint_id,
                item_no: fp.item_no,
            });
            fp.item_no = *item_no;
            ops.push(PatchOp::EntityUpserted {
                sheet_id,
                entity: Entity::Footprint(fp.clone()),
            });
        }
        let inverse = if old.is_empty() {
            Vec::new()
        } else {
            vec![Command::SetPanelItemNumbers {
                sheet_id,
                numbers: old,
            }]
        };
        Ok((ops, inverse))
    }

    /// 指定Wireの線番を書き換える。値が変わるものだけをpatch・逆コマンドに含める。
    fn apply_wire_numbers(
        &mut self,
        sheet_id: SheetId,
        numbers: &[WireNumber],
    ) -> Result<(Vec<PatchOp>, Vec<Command>)> {
        let sheet = self
            .project
            .sheet_mut(sheet_id)
            .ok_or(CoreError::SheetNotFound(sheet_id))?;
        let mut ops = Vec::new();
        let mut old = Vec::new();
        for WireNumber { wire_id, number } in numbers {
            let Some(Entity::Wire(w)) = sheet.entities.get_mut(wire_id) else {
                continue;
            };
            if w.net == *number {
                continue;
            }
            old.push(WireNumber {
                wire_id: *wire_id,
                number: w.net.clone(),
            });
            w.net = number.clone();
            ops.push(PatchOp::EntityUpserted {
                sheet_id,
                entity: Entity::Wire(w.clone()),
            });
        }
        let inverse = if old.is_empty() {
            Vec::new()
        } else {
            vec![Command::SetWireNumbers {
                sheet_id,
                numbers: old,
            }]
        };
        Ok((ops, inverse))
    }
}

/// 図面へ入れる前のエンティティ検証。
///
/// 描けないもの・意味が壊れているものは**黙って受け取らず**、
/// [`CoreError::InvalidCommand`]で断る(サイレント失敗の禁止)。今のところ対象は
/// ブロックシンボル ([`crate::block`]) のピン表・外形寸法と、注記の色トークン。
/// 図面へ入れられない`symbol_id`を拒む (G17)。
///
/// **描けないシンボルを黙って受け取らない**: 知らないidや極数が範囲外のidを200 OKで
/// 受け取ると、エンティティは保存されるのに図面には何も描かれず、原因も分からない。
/// 解決先は配置と同じ [`crate::symbol::resolve_symbol`] (同梱→極数可変→ユーザーライブラリ)。
fn validate_symbol_id(id: &str) -> Result<()> {
    if crate::symbol::resolve_symbol(id).is_some() {
        return Ok(());
    }
    let reason = match crate::symbol::dynamic_family(id) {
        Some((prefix, max)) => format!(
            "極数が範囲外です (`{prefix}{{1〜{max}}}p` の形で指定してください)"
        ),
        None => "そのようなシンボルはライブラリにありません".to_string(),
    };
    Err(CoreError::InvalidCommand(format!(
        "unknown symbol_id: {id:?} ({reason})"
    )))
}

fn validate_entity(entity: &Entity, sheet: &Sheet) -> Result<()> {
    match entity {
        Entity::Symbol(s) if s.symbol_id == crate::block::BLOCK_SYMBOL_ID => {
            crate::block::parse_block_spec(&s.attrs).map_err(CoreError::InvalidCommand)?;
        }
        Entity::Symbol(s) => validate_symbol_id(&s.symbol_id)?,
        Entity::Text(t) => {
            // 注記色は決められたトークン名だけ。生の色コードを黙って受け取ると、
            // 紙では読めない色の注記が図面に残ってしまう
            if let Some(color) = &t.color {
                if !crate::text_style::is_text_color_token(color) {
                    let known: Vec<&str> = crate::text_style::text_color_tokens().collect();
                    return Err(CoreError::InvalidCommand(format!(
                        "unknown text color token: {color:?} (expected one of {})",
                        known.join(", ")
                    )));
                }
            }
            // 引き出し線の対象 (G20): 同じシート上に実在するエンティティだけを指せる。
            // 存在しないIDを黙って受け取ると、注記は保存されるのに引き出し線が
            // 永遠に描かれず、原因も分からない (シンボルidの検証と同じ理由)
            if let Some(target) = t.leader_to {
                if target == t.id {
                    return Err(CoreError::InvalidCommand(
                        "leader_to cannot point to the note itself".to_string(),
                    ));
                }
                if !sheet.entities.contains_key(&target) {
                    return Err(CoreError::InvalidCommand(format!(
                        "unknown leader_to target: {target} (no such entity on this sheet)"
                    )));
                }
            }
        }
        // 図形 (G10): 頂点の数・色トークン・線幅を確かめる。生の色コードや
        // 印刷できない線幅を黙って受け取ると、紙に出せない図形が図面に残ってしまう
        Entity::Shape(s) => crate::shape::validate(s).map_err(CoreError::InvalidCommand)?,
        _ => {}
    }
    Ok(())
}

/// entitiesを除いたシートのコピー(メタ更新patch用)。
fn sheet_meta(sheet: &Sheet) -> Sheet {
    let mut meta = sheet.clone();
    meta.entities = Default::default();
    meta
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use uuid::Uuid;

    fn test_engine() -> (Engine, SheetId) {
        let project = Project::new("test");
        let sheet_id = project.sheets[0].id;
        (Engine::new(project), sheet_id)
    }

    fn sample_wire() -> Entity {
        Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(10.0, 10.0), Point::new(50.0, 10.0)],
            color: "red".into(),
            sq: 0.75,
            length_m: Some(0.4),
            part_no: None,
            net: None,
        })
    }

    /// 色トークンだけを変えた注記 (他の欄は同じ)。
    fn styled_note(color: Option<&str>) -> Entity {
        Entity::Text(TextEntity {
            id: Uuid::new_v4(),
            at: Point::new(20.0, 20.0),
            text: "注意".into(),
            height: 2.5,
            rotation: 0,
            anchor: TextAnchor::Start,
            color: color.map(|c| c.to_string()),
            bold: false,
            leader_to: None,
        })
    }

    /// シンボルを1個置くコマンド (idと属性だけを変える)。
    fn place(symbol_id: &str) -> Entity {
        Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: symbol_id.into(),
            at: Point::new(100.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "X1".into(),
            value: String::new(),
            attrs: Default::default(),
        })
    }

    /// A symbol whose id is not in the library is refused instead of being stored as an invisible entity, and the sheet is left untouched.
    /// ライブラリに無いシンボルidは、描かれない幽霊エンティティとして保存されるのではなく拒否され、図面は何も変わらない。
    #[test]
    fn an_unknown_symbol_id_is_refused_and_changes_nothing() {
        let (mut engine, sheet_id) = test_engine();
        let err = engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: place("no_such_symbol"),
            })
            .expect_err("知らないシンボルidは拒否される");
        assert!(
            matches!(&err, CoreError::InvalidCommand(m)
                if m.contains("no_such_symbol") && m.contains("ライブラリ")),
            "{err:?}"
        );
        assert!(engine.project().sheet(sheet_id).unwrap().entities.is_empty());
        assert_eq!(engine.undo_depth(), 0, "履歴も積まれない");
    }

    /// A parametric symbol asked for outside its pole range (0 poles, or more than the limit) is refused with a message that says the range.
    /// 極数可変シンボルを範囲外の極数 (0極や上限超え) で頼むと、使える範囲を書いたメッセージで拒否される。
    #[test]
    fn a_pole_count_outside_the_range_is_refused() {
        for id in [
            "connector_0p",
            "connector_51p",
            "terminal_block_0p",
            "plc_di_65p",
            "plc_do_0p",
        ] {
            let (mut engine, sheet_id) = test_engine();
            let err = engine
                .execute(Command::AddEntity {
                    sheet_id,
                    entity: place(id),
                })
                .expect_err("範囲外の極数は拒否される");
            assert!(
                matches!(&err, CoreError::InvalidCommand(m) if m.contains("極数が範囲外")),
                "{id}: {err:?}"
            );
            assert!(engine.project().sheet(sheet_id).unwrap().entities.is_empty());
        }
    }

    /// Every ordinary placement still goes through: bundled symbols, parametric symbols inside their range, and a block defined by its attributes.
    /// ふつうの配置はこれまでどおり通る: 同梱シンボル・範囲内の極数可変シンボル・属性で形が決まるブロック。
    #[test]
    fn valid_symbol_ids_are_still_accepted() {
        let (mut engine, sheet_id) = test_engine();
        for id in [
            "resistor",
            "relay_coil",
            "connector_3p",
            "terminal_block_8p",
            "plc_di_16p",
            "plc_do_64p",
        ] {
            engine
                .execute(Command::AddEntity {
                    sheet_id,
                    entity: place(id),
                })
                .unwrap_or_else(|e| panic!("{id} が置けない: {e:?}"));
        }
        let block = crate::block::test_block(
            "CTRL1",
            "MainController",
            100.0,
            50.0,
            r#"{"left":[{"no":"IN1","name":"入力1"}]}"#,
        );
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: block,
            })
            .expect("ブロックも置ける");
        assert_eq!(engine.project().sheet(sheet_id).unwrap().entities.len(), 7);
    }

    /// Changing an existing symbol to an unknown id is refused too, so a symbol on the sheet can always be drawn.
    /// 既にある部品を知らないシンボルidへ変更するのも拒否されるので、図面の部品は常に描ける。
    #[test]
    fn changing_a_symbol_to_an_unknown_id_is_refused() {
        let (mut engine, sheet_id) = test_engine();
        let entity = place("resistor");
        let id = entity.id();
        engine
            .execute(Command::AddEntity { sheet_id, entity })
            .expect("抵抗は置ける");
        let Entity::Symbol(mut s) =
            engine.project().sheet(sheet_id).unwrap().entities[&id].clone()
        else {
            unreachable!()
        };
        s.symbol_id = "resistor_typo".into();
        let err = engine
            .execute(Command::UpdateEntity {
                sheet_id,
                entity: Entity::Symbol(s),
            })
            .expect_err("知らないidへの変更は拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");
        let Entity::Symbol(kept) = &engine.project().sheet(sheet_id).unwrap().entities[&id] else {
            unreachable!()
        };
        assert_eq!(kept.symbol_id, "resistor", "元の部品のまま");
    }

    /// A block whose pin table is broken (duplicate numbers) is refused when added, and the sheet is left untouched.
    /// ピン表が壊れている(番号が重複した)ブロックは追加時に拒否され、図面は何も変わらない。
    #[test]
    fn a_block_with_a_broken_pin_table_is_refused_and_changes_nothing() {
        let (mut engine, sheet_id) = test_engine();
        let bad = crate::block::test_block(
            "CTRL1",
            "MainController",
            100.0,
            50.0,
            r#"{"left":[{"no":"IN1"}],"right":[{"no":"IN1"}]}"#,
        );
        let err = engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: bad,
            })
            .expect_err("重複ピン番号は拒否される");
        assert!(
            matches!(&err, CoreError::InvalidCommand(m) if m.contains("duplicate")),
            "{err:?}"
        );
        assert!(
            engine.project().sheet(sheet_id).unwrap().entities.is_empty(),
            "拒否されたのに図面へ入っている"
        );
        assert_eq!(engine.undo_depth(), 0, "undo履歴に積まれている");
    }

    /// Editing a good block into a broken one is refused too, so a block on the sheet always has a drawable pin table.
    /// 正しいブロックを壊れた内容へ編集するのも拒否されるので、図面上のブロックは常に描けるピン表を持つ。
    #[test]
    fn editing_a_block_into_a_broken_one_is_refused() {
        let (mut engine, sheet_id) = test_engine();
        let good = crate::block::test_block(
            "CTRL1",
            "MainController",
            100.0,
            50.0,
            r#"{"left":[{"no":"IN1"}]}"#,
        );
        let id = good.id();
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: good,
            })
            .expect("正しいブロックは追加できる");
        let mut broken = match engine.project().sheet(sheet_id).unwrap().entities[&id].clone() {
            Entity::Symbol(s) => s,
            _ => unreachable!(),
        };
        broken
            .attrs
            .insert("block_pins".into(), "not json".into());
        let err = engine
            .execute(Command::UpdateEntity {
                sheet_id,
                entity: Entity::Symbol(broken),
            })
            .expect_err("壊れた属性への更新は拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");
        let Entity::Symbol(kept) = &engine.project().sheet(sheet_id).unwrap().entities[&id] else {
            unreachable!()
        };
        assert_eq!(
            kept.attrs.get("block_pins").map(String::as_str),
            Some(r#"{"left":[{"no":"IN1"}]}"#),
            "拒否されたのに書き換わっている"
        );
    }

    /// A note whose colour is a name the drawing does not know (a typo, or a raw colour code) is refused instead of being stored and silently drawn in black.
    /// 図面が知らない色名 (打ち間違いや生の色コード) を付けた注記は、黙って黒で描かれるのではなく拒否される。
    #[test]
    fn a_note_with_an_unknown_colour_is_refused() {
        let (mut engine, sheet_id) = test_engine();
        for bad in ["#ff0000", "red", "赤", "", "Accent"] {
            let err = engine
                .execute(Command::AddEntity {
                    sheet_id,
                    entity: styled_note(Some(bad)),
                })
                .expect_err("知らない色は拒否される");
            assert!(matches!(err, CoreError::InvalidCommand(_)), "{bad}: {err:?}");
        }
        assert!(
            engine.project().sheet(sheet_id).unwrap().entities.is_empty(),
            "拒否された注記はシートに残らない"
        );
    }

    /// A note that uses one of the drawing's colour tokens (or no colour at all) is accepted.
    /// 図面の色トークンを使った注記 (色を指定しない注記も) は受け付けられる。
    #[test]
    fn a_note_with_a_known_colour_token_is_accepted() {
        let (mut engine, sheet_id) = test_engine();
        for good in [None, Some("default"), Some("accent"), Some("warning")] {
            engine
                .execute(Command::AddEntity {
                    sheet_id,
                    entity: styled_note(good),
                })
                .unwrap_or_else(|e| panic!("{good:?} は受け付ける: {e:?}"));
        }
        assert_eq!(engine.project().sheet(sheet_id).unwrap().entities.len(), 4);
    }

    /// Changing an existing note to an unknown colour is refused too, and the note keeps the colour it had.
    /// 既にある注記を知らない色へ変える更新も拒否され、注記は元の色のままになる。
    #[test]
    fn recolouring_a_note_to_an_unknown_colour_is_refused() {
        let (mut engine, sheet_id) = test_engine();
        let good = styled_note(Some("accent"));
        let id = good.id();
        engine.execute(Command::AddEntity { sheet_id, entity: good }).unwrap();
        let Entity::Text(mut broken) =
            engine.project().sheet(sheet_id).unwrap().entities[&id].clone()
        else {
            unreachable!()
        };
        broken.color = Some("#ff0000".into());
        let err = engine
            .execute(Command::UpdateEntity {
                sheet_id,
                entity: Entity::Text(broken),
            })
            .expect_err("生の色コードへの更新は拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");
        let Entity::Text(kept) = &engine.project().sheet(sheet_id).unwrap().entities[&id] else {
            unreachable!()
        };
        assert_eq!(kept.color.as_deref(), Some("accent"), "拒否されたのに書き換わっている");
    }

    /// A note whose leader points at an entity id that does not exist on the sheet is refused instead of being stored with a leader line that never draws.
    /// シート上に存在しないIDへ引き出し線を向けた注記は、いつまでも描かれない引き出し線を持ったまま保存されるのではなく拒否される。
    #[test]
    fn a_note_with_an_unknown_leader_target_is_refused() {
        let (mut engine, sheet_id) = test_engine();
        let mut note = TextEntity {
            id: Uuid::new_v4(),
            at: Point::new(20.0, 20.0),
            text: "対象なし".into(),
            height: 2.5,
            rotation: 0,
            anchor: TextAnchor::Start,
            color: None,
            bold: false,
            leader_to: Some(Uuid::new_v4()),
        };
        let err = engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: Entity::Text(note.clone()),
            })
            .expect_err("存在しない対象は拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");
        assert!(engine.project().sheet(sheet_id).unwrap().entities.is_empty());

        // 自分自身を指すのも拒否される (長さ0の引き出し線に意味は無い)
        note.leader_to = Some(note.id);
        let err = engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: Entity::Text(note),
            })
            .expect_err("自分自身を指す引き出し線は拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err:?}");
    }

    /// A note can set its leader to another entity already on the sheet, and clearing the leader (setting it back to None) is undoable like any other update.
    /// 注記はシート上の他のエンティティへ引き出し先を設定でき、引き出し先を外す更新 (Noneへ戻す) も他の更新と同じくundoできる。
    #[test]
    fn a_note_can_target_an_existing_entity_and_the_leader_can_be_cleared() {
        let (mut engine, sheet_id) = test_engine();
        let target = sample_wire();
        let target_id = target.id();
        engine.execute(Command::AddEntity { sheet_id, entity: target }).unwrap();

        let note = Entity::Text(TextEntity {
            id: Uuid::new_v4(),
            at: Point::new(20.0, 20.0),
            text: "対象あり".into(),
            height: 2.5,
            rotation: 0,
            anchor: TextAnchor::Start,
            color: None,
            bold: false,
            leader_to: Some(target_id),
        });
        let note_id = note.id();
        engine.execute(Command::AddEntity { sheet_id, entity: note }).unwrap();
        let Entity::Text(t) = &engine.project().sheet(sheet_id).unwrap().entities[&note_id] else {
            unreachable!()
        };
        assert_eq!(t.leader_to, Some(target_id));

        let mut cleared = t.clone();
        cleared.leader_to = None;
        engine
            .execute(Command::UpdateEntity { sheet_id, entity: Entity::Text(cleared) })
            .unwrap();
        assert_eq!(
            engine.undo().unwrap().unwrap().ops.len(),
            1,
            "更新のundoも1回で戻る"
        );
        let Entity::Text(restored) = &engine.project().sheet(sheet_id).unwrap().entities[&note_id]
        else {
            unreachable!()
        };
        assert_eq!(restored.leader_to, Some(target_id), "undoで引き出し先が復元される");
    }

    /// Adding an entity can be undone (the entity disappears) and redone (it comes back).
    /// エンティティの追加はundoで消え、redoで復活する。
    #[test]
    fn add_undo_redo_entity() {
        let (mut engine, sheet_id) = test_engine();
        let wire = sample_wire();
        let id = wire.id();

        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: wire,
            })
            .unwrap();
        assert!(engine.project().sheets[0].entities.contains_key(&id));

        let patch = engine.undo().unwrap().unwrap();
        assert!(!engine.project().sheets[0].entities.contains_key(&id));
        assert!(matches!(patch.ops[0], PatchOp::EntityRemoved { .. }));

        engine.redo().unwrap().unwrap();
        assert!(engine.project().sheets[0].entities.contains_key(&id));
    }

    /// Moving entities shifts their coordinates; undo restores the original position exactly.
    /// 移動でエンティティの座標が動き、undoで元の位置に正確に戻る。
    #[test]
    fn move_and_undo_restores_position() {
        let (mut engine, sheet_id) = test_engine();
        let wire = sample_wire();
        let id = wire.id();
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: wire,
            })
            .unwrap();
        engine
            .execute(Command::MoveEntities {
                sheet_id,
                ids: vec![id],
                dx: 5.0,
                dy: 2.5,
            })
            .unwrap();
        let Entity::Wire(w) = &engine.project().sheets[0].entities[&id] else {
            panic!()
        };
        assert_eq!(w.points[0].x, 15.0);
        engine.undo().unwrap().unwrap();
        let Entity::Wire(w) = &engine.project().sheets[0].entities[&id] else {
            panic!()
        };
        assert_eq!(w.points[0].x, 10.0);
    }

    /// Deleting several entities at once is a single undo step that restores all of them.
    /// 複数エンティティの一括削除は1回のundoで全て復元される。
    #[test]
    fn delete_multiple_and_undo() {
        let (mut engine, sheet_id) = test_engine();
        let a = sample_wire();
        let b = sample_wire();
        let (ida, idb) = (a.id(), b.id());
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: a,
            })
            .unwrap();
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: b,
            })
            .unwrap();
        engine
            .execute(Command::DeleteEntities {
                sheet_id,
                ids: vec![ida, idb],
            })
            .unwrap();
        assert!(engine.project().sheets[0].entities.is_empty());
        engine.undo().unwrap().unwrap();
        assert_eq!(engine.project().sheets[0].entities.len(), 2);
        assert!(engine.project().sheets[0].entities.contains_key(&ida));
        assert!(engine.project().sheets[0].entities.contains_key(&idb));
    }

    /// Sheets can be added and removed; undoing a removal restores the sheet with its original id and position.
    /// シートは追加・削除でき、削除のundoは元のid・位置のままシートを復元する。
    #[test]
    fn sheet_add_remove_undo() {
        let (mut engine, _) = test_engine();
        engine
            .execute(Command::AddSheet {
                name: "TB2".into(),
                size: PaperSize::A4,
                orientation: Orientation::Portrait,
                kind: SheetKind::Schematic,
            })
            .unwrap();
        assert_eq!(engine.project().sheets.len(), 2);
        let new_id = engine.project().sheets[1].id;
        engine
            .execute(Command::RemoveSheet { sheet_id: new_id })
            .unwrap();
        assert_eq!(engine.project().sheets.len(), 1);
        engine.undo().unwrap().unwrap();
        assert_eq!(engine.project().sheets.len(), 2);
        assert_eq!(engine.project().sheets[1].id, new_id);
    }

    /// Adding a sheet without naming a kind gives an ordinary schematic sheet, and a panel-layout sheet is added by asking for the panel kind (undo removes it either way).
    /// 種別を指定しないシート追加は通常の回路図シートになり、盤レイアウトシートは種別を指定して追加する(どちらもundoで消える)。
    #[test]
    fn add_sheet_defaults_to_schematic_and_can_add_a_panel_sheet() {
        let (mut engine, _) = test_engine();
        engine
            .execute(Command::AddSheet {
                name: "回路".into(),
                size: PaperSize::A3,
                orientation: Orientation::Landscape,
                kind: SheetKind::default(),
            })
            .unwrap();
        assert_eq!(engine.project().sheets[1].kind, SheetKind::Schematic);
        engine
            .execute(Command::AddSheet {
                name: "盤レイアウト".into(),
                size: PaperSize::A3,
                orientation: Orientation::Landscape,
                kind: SheetKind::Panel,
            })
            .unwrap();
        assert_eq!(engine.project().sheets[2].kind, SheetKind::Panel);
        engine.undo().unwrap().unwrap();
        assert_eq!(engine.project().sheets.len(), 2);
    }

    /// A sheet-add command written before panel sheets existed (no kind field) still works and adds a schematic sheet.
    /// 盤シートが無かった頃に書かれたシート追加コマンド(種別の欄が無い)もそのまま動き、回路図シートを追加する。
    #[test]
    fn a_sheet_add_command_without_a_kind_still_adds_a_schematic_sheet() {
        let cmd: Command = serde_json::from_str(
            r#"{"type":"add_sheet","name":"S2","size":"A3","orientation":"Landscape"}"#,
        )
        .expect("旧いコマンドJSONが読める");
        let Command::AddSheet { kind, .. } = cmd else {
            panic!("シート追加コマンドであること")
        };
        assert_eq!(kind, SheetKind::Schematic);
    }

    fn sample_panel_entities() -> Vec<Entity> {
        vec![
            Entity::PanelOutline(PanelOutline {
                id: Uuid::new_v4(),
                at: Point::new(40.0, 40.0),
                w: 200.0,
                h: 150.0,
                name: "制御盤".into(),
            }),
            Entity::DinRail(DinRail {
                id: Uuid::new_v4(),
                y: 100.0,
                x1: 60.0,
                x2: 220.0,
            }),
            Entity::Duct(Duct {
                id: Uuid::new_v4(),
                at: Point::new(60.0, 60.0),
                w: 160.0,
                h: 20.0,
            }),
            Entity::Footprint(Footprint {
                id: Uuid::new_v4(),
                at: Point::new(80.0, 82.5),
                w: 25.0,
                h: 65.0,
                rotation: 0,
                reference: "K1".into(),
                part_no: "MDK-RLY-24V".into(),
                item_no: None,
            }),
        ]
    }

    /// The four panel entities (panel outline, DIN rail, duct, footprint) are added, moved and deleted with the ordinary entity commands, and every step can be undone.
    /// 盤エンティティ4種(盤外形・DINレール・ダクト・フットプリント)は通常のエンティティ用コマンドで追加・移動・削除でき、どの操作もundoで戻せる。
    #[test]
    fn panel_entities_are_added_moved_and_deleted_with_undo() {
        let (mut engine, sheet_id) = test_engine();
        let entities = sample_panel_entities();
        let ids: Vec<EntityId> = entities.iter().map(|e| e.id()).collect();
        for entity in entities {
            engine
                .execute(Command::AddEntity { sheet_id, entity })
                .unwrap();
        }
        assert_eq!(engine.project().sheets[0].entities.len(), 4);

        engine
            .execute(Command::MoveEntities {
                sheet_id,
                ids: ids.clone(),
                dx: 10.0,
                dy: 5.0,
            })
            .unwrap();
        let Entity::DinRail(rail) = &engine.project().sheets[0].entities[&ids[1]] else {
            panic!("DINレールであること")
        };
        assert_eq!((rail.x1, rail.y), (70.0, 105.0));

        engine
            .execute(Command::DeleteEntities {
                sheet_id,
                ids: ids.clone(),
            })
            .unwrap();
        assert!(engine.project().sheets[0].entities.is_empty());
        engine.undo().unwrap().unwrap(); // 削除を戻す
        engine.undo().unwrap().unwrap(); // 移動を戻す
        let Entity::DinRail(rail) = &engine.project().sheets[0].entities[&ids[1]] else {
            panic!("DINレールであること")
        };
        assert_eq!((rail.x1, rail.y), (60.0, 100.0));
    }

    /// Editing a footprint (its reference, part number or rotation) replaces it wholesale, and undo brings back the previous version.
    /// フットプリントの編集(参照記号・型番・回転)は丸ごと置換で、undoで編集前に戻る。
    #[test]
    fn a_footprint_can_be_edited_and_undone() {
        let (mut engine, sheet_id) = test_engine();
        let entity = Entity::Footprint(Footprint {
            id: Uuid::new_v4(),
            at: Point::new(80.0, 80.0),
            w: 25.0,
            h: 65.0,
            rotation: 0,
            reference: "K1".into(),
            part_no: "MDK-RLY-24V".into(),
            item_no: None,
        });
        let id = entity.id();
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: entity.clone(),
            })
            .unwrap();
        let mut edited = entity.clone();
        if let Entity::Footprint(f) = &mut edited {
            f.reference = "K2".into();
            f.rotation = 90;
        }
        engine
            .execute(Command::UpdateEntity {
                sheet_id,
                entity: edited,
            })
            .unwrap();
        let Entity::Footprint(f) = &engine.project().sheets[0].entities[&id] else {
            panic!()
        };
        assert_eq!((f.reference.as_str(), f.rotation), ("K2", 90));
        engine.undo().unwrap().unwrap();
        let Entity::Footprint(f) = &engine.project().sheets[0].entities[&id] else {
            panic!()
        };
        assert_eq!((f.reference.as_str(), f.rotation), ("K1", 0));
    }

    /// Commands serialize to JSON and back without loss, so any client can send them over the wire.
    /// CommandはJSONに往復変換でき、どのクライアントからも送信できる。
    #[test]
    fn command_json_roundtrip() {
        let (_, sheet_id) = test_engine();
        let cmd = Command::AddEntity {
            sheet_id,
            entity: sample_wire(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let back: Command = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, Command::AddEntity { .. }));
    }

    /// UpdateEntity replaces an entity wholesale; undo brings back the previous version.
    /// UpdateEntityはエンティティを丸ごと置換し、undoで置換前の状態に戻る。
    #[test]
    fn update_entity_undo_restores_previous_version() {
        let (mut engine, sheet_id) = test_engine();
        let wire = sample_wire();
        let id = wire.id();
        engine.execute(Command::AddEntity { sheet_id, entity: wire.clone() }).unwrap();
        let mut updated = wire.clone();
        if let Entity::Wire(w) = &mut updated { w.color = "blue".into(); }
        engine.execute(Command::UpdateEntity { sheet_id, entity: updated }).unwrap();
        let Entity::Wire(w) = &engine.project().sheets[0].entities[&id] else { panic!() };
        assert_eq!(w.color, "blue");
        engine.undo().unwrap().unwrap();
        let Entity::Wire(w) = &engine.project().sheets[0].entities[&id] else { panic!() };
        assert_eq!(w.color, "red");
    }

    /// SetTitleBlock updates the sheet's title block; undo restores the previous fields.
    /// SetTitleBlockはシートの表題欄を更新し、undoで以前の内容に戻る。
    #[test]
    fn set_title_block_is_undoable() {
        let (mut engine, sheet_id) = test_engine();
        let mut tb = TitleBlock::default();
        tb.title = "動力系統図".into();
        engine.execute(Command::SetTitleBlock { sheet_id, title_block: tb }).unwrap();
        assert_eq!(engine.project().sheets[0].title_block.title, "動力系統図");
        engine.undo().unwrap().unwrap();
        assert_eq!(engine.project().sheets[0].title_block.title, "");
    }

    /// SetRevisions replaces the revision-table rows; undo restores the previous list.
    /// SetRevisionsは改訂欄の行を置き換え、undoで以前のリストに戻る。
    #[test]
    fn set_revisions_is_undoable() {
        let (mut engine, sheet_id) = test_engine();
        let rev = Revision { mark: "A".into(), date: "2026-08-21".into(), description: "初版".into(), by: "K.T".into() };
        engine.execute(Command::SetRevisions { sheet_id, revisions: vec![rev] }).unwrap();
        assert_eq!(engine.project().sheets[0].revisions.len(), 1);
        engine.undo().unwrap().unwrap();
        assert!(engine.project().sheets[0].revisions.is_empty());
    }

    /// The annotation text height of a sheet is set through a command, and undo puts the previous height back.
    /// シートの注記文字高はコマンドで設定し、undoで以前の高さに戻る。
    #[test]
    fn set_sheet_annotation_text_is_undoable() {
        let (mut engine, sheet_id) = test_engine();
        engine
            .execute(Command::SetSheetAnnotationText {
                sheet_id,
                height_mm: Some(1.4),
            })
            .unwrap();
        assert_eq!(engine.project().sheets[0].annotation_text_mm, Some(1.4));
        assert!((engine.project().sheets[0].annotation_text_mm() - 1.4).abs() < 1e-9);
        engine.undo().unwrap().unwrap();
        assert_eq!(engine.project().sheets[0].annotation_text_mm, None, "設定前へ戻る");
    }

    /// Clearing the annotation text height with null puts the sheet back to the default 2.0mm.
    /// 注記文字高をnullで消すと、シートは既定の2.0mmに戻る。
    #[test]
    fn clearing_the_annotation_text_height_restores_the_default() {
        let (mut engine, sheet_id) = test_engine();
        for height in [Some(3.0), None] {
            engine
                .execute(Command::SetSheetAnnotationText {
                    sheet_id,
                    height_mm: height,
                })
                .unwrap();
        }
        assert_eq!(engine.project().sheets[0].annotation_text_mm, None);
        assert!((engine.project().sheets[0].annotation_text_mm() - 2.0).abs() < 1e-9);
    }

    /// An annotation text height outside 1.0-3.5mm is rejected with a reason, and the sheet is left untouched.
    /// 1.0〜3.5mmの範囲を外れた注記文字高は理由付きで拒否され、シートは書き換わらない。
    #[test]
    fn an_out_of_range_annotation_text_height_is_rejected() {
        let (mut engine, sheet_id) = test_engine();
        for bad in [0.5, 5.0, f64::NAN] {
            let err = engine
                .execute(Command::SetSheetAnnotationText {
                    sheet_id,
                    height_mm: Some(bad),
                })
                .unwrap_err();
            assert!(err.to_string().contains("annotation text height"), "{err}");
        }
        assert_eq!(engine.project().sheets[0].annotation_text_mm, None, "書き換わらない");
    }

    /// The annotation text height command is plain JSON, so AI agents and the CLI can send it.
    /// 注記文字高のコマンドは素のJSONで表現でき、AIやCLIから送れる。
    #[test]
    fn set_sheet_annotation_text_is_plain_json() {
        let (mut engine, sheet_id) = test_engine();
        let json = format!(
            r#"{{"type":"set_sheet_annotation_text","sheet_id":"{sheet_id}","height_mm":1.4}}"#
        );
        let cmd: Command = serde_json::from_str(&json).unwrap();
        engine.execute(cmd).unwrap();
        assert_eq!(engine.project().sheets[0].annotation_text_mm, Some(1.4));
    }

    /// SetPlcAssignments replaces the whole PLC I/O assignment table of the project; undo restores the previous table.
    /// SetPlcAssignmentsはプロジェクトのPLC I/O割付表を丸ごと置き換え、undoで以前の表に戻る。
    #[test]
    fn set_plc_assignments_is_undoable() {
        let (mut engine, _) = test_engine();
        let row = PlcAssignment {
            id: Uuid::new_v4(),
            module_ref: "PLC1".into(),
            address: "X0".into(),
            signal_name: "起動押釦".into(),
            comment: "PB1".into(),
        };
        let patch = engine
            .execute(Command::SetPlcAssignments {
                assignments: vec![row],
            })
            .unwrap();
        assert!(matches!(
            patch.ops[0],
            PatchOp::PlcAssignmentsReplaced { .. }
        ));
        assert_eq!(engine.project().plc_assignments.len(), 1);
        assert_eq!(engine.project().plc_assignments[0].signal_name, "起動押釦");
        engine.undo().unwrap().unwrap();
        assert!(engine.project().plc_assignments.is_empty());
        engine.redo().unwrap().unwrap();
        assert_eq!(engine.project().plc_assignments.len(), 1);
    }

    /// SetFrameTemplate switches the frame template of the whole project in one edit, and undo puts the previous one back (undoing the very first switch returns to the default frame).
    /// SetFrameTemplateはプロジェクト全体の図枠テンプレートを1回の編集で差し替え、undoで元の図枠へ戻る(最初の差し替えをundoすると未指定=既定の図枠へ戻る)。
    #[test]
    fn set_frame_template_is_undoable() {
        let (mut engine, _) = test_engine();
        assert_eq!(engine.project().frame_template, None);
        let patch = engine
            .execute(Command::SetFrameTemplate {
                frame_template: Some("jis-a4-portrait".into()),
            })
            .unwrap();
        assert!(matches!(patch.ops[0], PatchOp::FrameTemplateChanged { .. }));
        assert_eq!(
            engine.project().frame_template.as_deref(),
            Some("jis-a4-portrait")
        );
        engine.undo().unwrap().unwrap();
        assert_eq!(engine.project().frame_template, None, "未指定 (既定の図枠) へ戻る");
        engine.redo().unwrap().unwrap();
        assert_eq!(
            engine.project().frame_template.as_deref(),
            Some("jis-a4-portrait")
        );
    }

    /// SetFrameTemplate accepts an id nobody knows: the drawing keeps printing with the default frame, so a project made elsewhere still opens.
    /// SetFrameTemplateは知らないidも受け付ける(図面は既定の図枠で描かれ続けるので、他所で作ったプロジェクトも開ける)。
    #[test]
    fn set_frame_template_accepts_an_unknown_id() {
        let (mut engine, _) = test_engine();
        engine
            .execute(Command::SetFrameTemplate {
                frame_template: Some("our-own-frame".into()),
            })
            .expect("未知のidでも失敗しない");
        assert_eq!(
            crate::frame_template::for_project(engine.project()).id,
            crate::frame_template::DEFAULT_FRAME_TEMPLATE_ID
        );
    }

    fn sample_harness(name: &str) -> Entity {
        Entity::Harness(Harness {
            id: Uuid::new_v4(),
            points: crate::harness::rect_points(Point::new(50.0, 50.0), Point::new(150.0, 100.0)),
            name: name.into(),
            note: String::new(),
        })
    }

    /// A harness boundary is added, renamed and deleted with the ordinary entity commands, and every step can be undone.
    /// ハーネス境界は通常のエンティティ用コマンドで追加・改名・削除でき、どの操作もundoで戻せる。
    #[test]
    fn harness_add_rename_delete_are_undoable() {
        let (mut engine, sheet_id) = test_engine();
        let harness = sample_harness("W1");
        let id = harness.id();
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: harness.clone(),
            })
            .unwrap();
        assert!(engine.project().sheets[0].entities.contains_key(&id));

        let mut renamed = harness.clone();
        if let Entity::Harness(h) = &mut renamed {
            h.name = "W2".into();
        }
        engine
            .execute(Command::UpdateEntity {
                sheet_id,
                entity: renamed,
            })
            .unwrap();
        let Entity::Harness(h) = &engine.project().sheets[0].entities[&id] else {
            panic!("ハーネスであること")
        };
        assert_eq!(h.name, "W2");

        engine
            .execute(Command::DeleteEntities {
                sheet_id,
                ids: vec![id],
            })
            .unwrap();
        assert!(!engine.project().sheets[0].entities.contains_key(&id));

        engine.undo().unwrap().unwrap(); // 削除を戻す
        engine.undo().unwrap().unwrap(); // 改名を戻す
        let Entity::Harness(h) = &engine.project().sheets[0].entities[&id] else {
            panic!("ハーネスであること")
        };
        assert_eq!(h.name, "W1");
        engine.undo().unwrap().unwrap(); // 追加を戻す
        assert!(engine.project().sheets[0].entities.is_empty());
    }

    /// Moving a harness boundary shifts all of its corners, and undo puts them back exactly.
    /// ハーネス境界を移動すると4隅すべてが動き、undoで元の位置に正確に戻る。
    #[test]
    fn harness_move_and_undo_restores_every_corner() {
        let (mut engine, sheet_id) = test_engine();
        let harness = sample_harness("W1");
        let id = harness.id();
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: harness,
            })
            .unwrap();
        engine
            .execute(Command::MoveEntities {
                sheet_id,
                ids: vec![id],
                dx: 10.0,
                dy: -5.0,
            })
            .unwrap();
        let Entity::Harness(h) = &engine.project().sheets[0].entities[&id] else {
            panic!()
        };
        assert_eq!(h.points[0], Point::new(60.0, 45.0));
        assert_eq!(h.points[2], Point::new(160.0, 95.0));
        engine.undo().unwrap().unwrap();
        let Entity::Harness(h) = &engine.project().sheets[0].entities[&id] else {
            panic!()
        };
        assert_eq!(h.points[0], Point::new(50.0, 50.0));
    }

    /// A harness boundary survives a JSON round trip with the kind tag "harness", so any client can send it.
    /// ハーネス境界は kind="harness" としてJSONに往復変換でき、どのクライアントからも送れる。
    #[test]
    fn harness_command_json_roundtrip_uses_the_harness_kind() {
        let (_, sheet_id) = test_engine();
        let cmd = Command::AddEntity {
            sheet_id,
            entity: sample_harness("W1"),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"kind\":\"harness\""), "{json}");
        let back: Command = serde_json::from_str(&json).unwrap();
        let Command::AddEntity {
            entity: Entity::Harness(h),
            ..
        } = back
        else {
            panic!("ハーネスの追加コマンドに戻ること")
        };
        assert_eq!(h.name, "W1");
        assert_eq!(h.points.len(), 4);
    }

    /// 図形1つ (G10)。既定の見た目 (既定色・細線・塗りなし・実線) で作る。
    fn sample_shape(kind: ShapeKind, points: &[(f64, f64)]) -> Entity {
        Entity::Shape(crate::shape::new_shape(
            kind,
            points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
        ))
    }

    /// All four kinds of shape are added, edited and deleted with the ordinary entity commands, and every step can be undone.
    /// 4種類すべての図形が通常のエンティティ用コマンドで追加・編集・削除でき、どの操作もundoで戻せる。
    #[test]
    fn shape_add_edit_delete_are_undoable_for_every_kind() {
        for (kind, points) in [
            (ShapeKind::Rect, &[(50.0, 50.0), (150.0, 100.0)][..]),
            (ShapeKind::Line, &[(50.0, 50.0), (150.0, 50.0)][..]),
            (ShapeKind::Circle, &[(100.0, 50.0), (110.0, 50.0)][..]),
            (
                ShapeKind::Polyline,
                &[(50.0, 50.0), (100.0, 50.0), (100.0, 90.0)][..],
            ),
        ] {
            let (mut engine, sheet_id) = test_engine();
            let entity = sample_shape(kind, points);
            let id = entity.id();
            engine
                .execute(Command::AddEntity {
                    sheet_id,
                    entity: entity.clone(),
                })
                .unwrap_or_else(|e| panic!("{kind:?}: {e}"));
            assert!(engine.project().sheets[0].entities.contains_key(&id), "{kind:?}");

            let mut styled = entity.clone();
            if let Entity::Shape(s) = &mut styled {
                s.stroke = "accent".into();
                s.stroke_width_mm = 0.5;
                s.dashed = true;
                s.fill = Some("muted".into());
            }
            engine
                .execute(Command::UpdateEntity {
                    sheet_id,
                    entity: styled,
                })
                .unwrap();
            let Entity::Shape(s) = &engine.project().sheets[0].entities[&id] else {
                panic!("図形であること")
            };
            assert_eq!(s.stroke, "accent");
            assert!(s.dashed);

            engine
                .execute(Command::DeleteEntities {
                    sheet_id,
                    ids: vec![id],
                })
                .unwrap();
            assert!(!engine.project().sheets[0].entities.contains_key(&id));

            engine.undo().unwrap().unwrap(); // 削除を戻す
            engine.undo().unwrap().unwrap(); // 見た目の変更を戻す
            let Entity::Shape(s) = &engine.project().sheets[0].entities[&id] else {
                panic!("図形であること")
            };
            assert_eq!(s.stroke, crate::shape::DEFAULT_SHAPE_COLOR, "{kind:?}");
            assert!(!s.dashed);
            engine.undo().unwrap().unwrap(); // 追加を戻す
            assert!(engine.project().sheets[0].entities.is_empty(), "{kind:?}");
        }
    }

    /// Moving a shape shifts every one of its points, and undo puts them back exactly.
    /// 図形を移動すると全ての頂点が動き、undoで元の位置に正確に戻る。
    #[test]
    fn shape_move_and_undo_restores_every_point() {
        let (mut engine, sheet_id) = test_engine();
        let entity = sample_shape(ShapeKind::Polyline, &[(50.0, 50.0), (100.0, 50.0), (100.0, 90.0)]);
        let id = entity.id();
        engine.execute(Command::AddEntity { sheet_id, entity }).unwrap();
        engine
            .execute(Command::MoveEntities {
                sheet_id,
                ids: vec![id],
                dx: 10.0,
                dy: -5.0,
            })
            .unwrap();
        let Entity::Shape(s) = &engine.project().sheets[0].entities[&id] else { panic!() };
        assert_eq!(s.points[0], Point::new(60.0, 45.0));
        assert_eq!(s.points[2], Point::new(110.0, 85.0));
        engine.undo().unwrap().unwrap();
        let Entity::Shape(s) = &engine.project().sheets[0].entities[&id] else { panic!() };
        assert_eq!(s.points[0], Point::new(50.0, 50.0));
        assert_eq!(s.points[2], Point::new(100.0, 90.0));
    }

    /// A shape with a raw colour code instead of a colour token is refused, and the drawing is left untouched.
    /// 色トークンではなく生の色コードを入れた図形は拒否され、図面は何も変わらない。
    #[test]
    fn a_shape_with_a_raw_colour_code_is_refused() {
        let (mut engine, sheet_id) = test_engine();
        let mut entity = sample_shape(ShapeKind::Rect, &[(0.0, 0.0), (10.0, 10.0)]);
        if let Entity::Shape(s) = &mut entity {
            s.stroke = "#ff0000".into();
        }
        let err = engine
            .execute(Command::AddEntity { sheet_id, entity })
            .expect_err("生の色コードは拒否される");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err}");
        assert!(engine.project().sheets[0].entities.is_empty(), "図面は変わらない");
    }

    /// A shape whose stroke is too thin to print or thick enough to swamp the grid is refused.
    /// 印刷できないほど細い線幅・グリッドを潰すほど太い線幅の図形は拒否される。
    #[test]
    fn a_shape_with_an_unprintable_stroke_width_is_refused() {
        let (mut engine, sheet_id) = test_engine();
        for width in [0.0, 0.05, 2.5] {
            let mut entity = sample_shape(ShapeKind::Line, &[(0.0, 0.0), (10.0, 0.0)]);
            if let Entity::Shape(s) = &mut entity {
                s.stroke_width_mm = width;
            }
            let err = engine
                .execute(Command::AddEntity { sheet_id, entity })
                .expect_err("範囲外の線幅は拒否される");
            assert!(matches!(err, CoreError::InvalidCommand(_)), "{width}: {err}");
        }
        assert!(engine.project().sheets[0].entities.is_empty());
    }

    /// A rectangle sent with the wrong number of points is refused, so a half-drawn shape never reaches the drawing.
    /// 頂点の数が合っていない矩形は拒否されるので、描きかけの図形が図面へ入ることはない。
    #[test]
    fn a_shape_with_the_wrong_number_of_points_is_refused() {
        let (mut engine, sheet_id) = test_engine();
        let entity = sample_shape(ShapeKind::Rect, &[(0.0, 0.0)]);
        assert!(engine.execute(Command::AddEntity { sheet_id, entity }).is_err());
        assert!(engine.project().sheets[0].entities.is_empty());
    }

    /// A shape survives a JSON round trip with the kind tag "shape", so any client can send it.
    /// 図形は kind="shape" としてJSONに往復変換でき、どのクライアントからも送れる。
    #[test]
    fn shape_command_json_roundtrip_uses_the_shape_kind() {
        let (_, sheet_id) = test_engine();
        let cmd = Command::AddEntity {
            sheet_id,
            entity: sample_shape(ShapeKind::Rect, &[(0.0, 0.0), (10.0, 10.0)]),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        assert!(json.contains("\"kind\":\"shape\""), "{json}");
        let back: Command = serde_json::from_str(&json).unwrap();
        let Command::AddEntity {
            entity: Entity::Shape(s),
            ..
        } = back
        else {
            panic!("図形の追加コマンドに戻ること")
        };
        assert_eq!(s.shape, ShapeKind::Rect);
        assert_eq!(s.points.len(), 2);
    }

    /// Every execute/undo/redo increases the document revision, so clients can discard stale patches.
    /// execute/undo/redoのたびにドキュメントrevisionが増加し、クライアントは古いpatchを破棄できる。
    #[test]
    fn revision_increases_monotonically() {
        let (mut engine, sheet_id) = test_engine();
        let r0 = engine.revision();
        let p1 = engine.execute(Command::AddEntity { sheet_id, entity: sample_wire() }).unwrap();
        let p2 = engine.undo().unwrap().unwrap();
        let p3 = engine.redo().unwrap().unwrap();
        assert!(r0 < p1.revision && p1.revision < p2.revision && p2.revision < p3.revision);
    }

    /// Commands targeting a non-existent sheet fail with an error and change nothing.
    /// 存在しないシートへのCommandはエラーになり、何も変更されない。
    #[test]
    fn unknown_sheet_is_rejected() {
        let (mut engine, _) = test_engine();
        let missing = Uuid::new_v4();
        let err = engine.execute(Command::AddEntity { sheet_id: missing, entity: sample_wire() });
        assert!(err.is_err());
        assert_eq!(engine.project().sheets[0].entities.len(), 0);
    }

    /// Undo with an empty history returns None instead of an error.
    /// 履歴が空のときのundoはエラーではなくNoneを返す。
    #[test]
    fn undo_on_empty_history_returns_none() {
        let (mut engine, _) = test_engine();
        assert!(engine.undo().unwrap().is_none());
        assert!(engine.redo().unwrap().is_none());
    }

    /// A new edit after undo clears the redo history (standard editor behavior).
    /// undo後に新しい編集をするとredo履歴は消える(一般的なエディタと同じ挙動)。
    #[test]
    fn new_edit_after_undo_clears_redo() {
        let (mut engine, sheet_id) = test_engine();
        engine.execute(Command::AddEntity { sheet_id, entity: sample_wire() }).unwrap();
        engine.undo().unwrap().unwrap();
        engine.execute(Command::AddEntity { sheet_id, entity: sample_wire() }).unwrap();
        assert!(engine.redo().unwrap().is_none());
    }

    /// Updating an entity that does not exist fails and does not sneak the new entity into the sheet.
    /// 存在しないエンティティの更新は失敗し、その要素がシートへ紛れ込むこともない。
    #[test]
    fn updating_a_missing_entity_fails_without_inserting_it() {
        let (mut engine, sheet_id) = test_engine();
        let wire = sample_wire();
        let id = wire.id();
        assert!(engine
            .execute(Command::UpdateEntity {
                sheet_id,
                entity: wire
            })
            .is_err());
        assert!(!engine.project().sheets[0].entities.contains_key(&id));
    }

    /// Every history entry records who made the edit; plain execute() counts as a user edit.
    /// 履歴の各エントリは編集の由来(誰の編集か)を記録し、通常のexecute()はユーザー編集として扱う。
    #[test]
    fn execute_as_records_the_edit_origin() {
        let (mut engine, sheet_id) = test_engine();
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: sample_wire(),
            })
            .unwrap();
        engine
            .execute_as(
                Command::AddEntity {
                    sheet_id,
                    entity: sample_wire(),
                },
                EditOrigin::Agent,
            )
            .unwrap();
        engine
            .execute_as(
                Command::AddEntity {
                    sheet_id,
                    entity: sample_wire(),
                },
                EditOrigin::Mcp,
            )
            .unwrap();
        assert_eq!(
            engine.history_origins(),
            vec![EditOrigin::User, EditOrigin::Agent, EditOrigin::Mcp]
        );
        // redoで積み直しても由来は変わらない
        engine.undo().unwrap().unwrap();
        engine.redo().unwrap().unwrap();
        assert_eq!(
            engine.history_origins(),
            vec![EditOrigin::User, EditOrigin::Agent, EditOrigin::Mcp]
        );
    }

    /// Reverting a range rolls back only the agent's edits in it and keeps the user's own edits, even when they were interleaved.
    /// 区間の巻き戻しはその中のエージェント編集だけを戻し、間に挟まったユーザー編集はそのまま残す。
    #[test]
    fn revert_range_rolls_back_agent_edits_and_keeps_user_edits() {
        let (mut engine, sheet_id) = test_engine();
        let agent_a = sample_wire();
        let user = sample_wire();
        let agent_b = sample_wire();
        let (agent_a_id, user_id, agent_b_id) = (agent_a.id(), user.id(), agent_b.id());

        let start = engine.undo_depth();
        engine
            .execute_as(
                Command::AddEntity {
                    sheet_id,
                    entity: agent_a,
                },
                EditOrigin::Agent,
            )
            .unwrap();
        // ターンの途中でユーザーが手で1本引いた
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: user,
            })
            .unwrap();
        engine
            .execute_as(
                Command::AddEntity {
                    sheet_id,
                    entity: agent_b,
                },
                EditOrigin::Agent,
            )
            .unwrap();
        let end = engine.undo_depth();

        let reverted = engine
            .revert_range(start, end, EditOrigin::Agent)
            .unwrap()
            .expect("エージェント編集が2件戻る");
        assert_eq!(reverted.commands, 2);
        let entities = &engine.project().sheets[0].entities;
        assert!(!entities.contains_key(&agent_a_id), "エージェントの編集は戻る");
        assert!(!entities.contains_key(&agent_b_id));
        assert!(entities.contains_key(&user_id), "ユーザーの手編集は残る");
    }

    /// A revert is a normal edit in the history, so undoing it brings the agent's work back.
    /// 巻き戻しも通常の編集として履歴に乗るため、undoすればエージェントの編集が戻ってくる。
    #[test]
    fn revert_range_is_itself_undoable() {
        let (mut engine, sheet_id) = test_engine();
        let wire = sample_wire();
        let id = wire.id();
        let start = engine.undo_depth();
        engine
            .execute_as(
                Command::AddEntity {
                    sheet_id,
                    entity: wire,
                },
                EditOrigin::Agent,
            )
            .unwrap();
        let end = engine.undo_depth();

        engine.revert_range(start, end, EditOrigin::Agent).unwrap();
        assert!(!engine.project().sheets[0].entities.contains_key(&id));
        assert_eq!(engine.undo_depth(), end + 1, "巻き戻しも1件の履歴になる");
        assert_eq!(
            engine.history_origins().last(),
            Some(&EditOrigin::User),
            "巻き戻しはユーザー操作として積まれる"
        );

        engine.undo().unwrap().unwrap();
        assert!(
            engine.project().sheets[0].entities.contains_key(&id),
            "巻き戻しの取り消しでエージェントの編集が戻る"
        );
        engine.redo().unwrap().unwrap();
        assert!(!engine.project().sheets[0].entities.contains_key(&id));
    }

    /// If the agent's edit cannot be undone against the current drawing (the user deleted the target), the whole revert is refused and nothing changes.
    /// エージェントの編集を現在の図面へ逆適用できない場合(対象をユーザーが消した等)、巻き戻し全体を拒否し何も変更しない。
    #[test]
    fn revert_range_refuses_conflicting_reverts_without_partial_changes() {
        let (mut engine, sheet_id) = test_engine();
        let wire = sample_wire();
        let id = wire.id();
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: wire.clone(),
            })
            .unwrap();

        let start = engine.undo_depth();
        // エージェントが色を変え、さらに別の配線を追加した
        let mut updated = wire.clone();
        if let Entity::Wire(w) = &mut updated {
            w.color = "blue".into();
        }
        engine
            .execute_as(
                Command::UpdateEntity {
                    sheet_id,
                    entity: updated,
                },
                EditOrigin::Agent,
            )
            .unwrap();
        let other = sample_wire();
        let other_id = other.id();
        engine
            .execute_as(
                Command::AddEntity {
                    sheet_id,
                    entity: other,
                },
                EditOrigin::Agent,
            )
            .unwrap();
        let end = engine.undo_depth();

        // ユーザーが更新対象を手で削除 → 色の巻き戻し先が存在しない
        engine
            .execute(Command::DeleteEntities {
                sheet_id,
                ids: vec![id],
            })
            .unwrap();
        let depth_before = engine.undo_depth();

        let err = engine
            .revert_range(start, end, EditOrigin::Agent)
            .unwrap_err();
        assert!(
            matches!(err, CoreError::RevertConflict(_)),
            "衝突は専用エラー: {err}"
        );
        assert!(
            engine.project().sheets[0].entities.contains_key(&other_id),
            "部分適用しない(戻せた分も戻さない)"
        );
        assert_eq!(engine.undo_depth(), depth_before, "履歴も増えない");
    }

    /// Commands run as one batch become a single history entry, so one undo removes all of them at once (and one redo brings them all back).
    /// バッチとしてまとめて実行したコマンド列は履歴1件になり、undo一発で全部消え、redo一発で全部戻る。
    #[test]
    fn execute_batch_is_undone_in_one_step() {
        let (mut engine, sheet_id) = test_engine();
        let cmds = vec![
            Command::AddEntity {
                sheet_id,
                entity: sample_wire(),
            },
            Command::AddEntity {
                sheet_id,
                entity: sample_wire(),
            },
            Command::AddEntity {
                sheet_id,
                entity: sample_wire(),
            },
        ];
        engine.execute_batch(cmds, EditOrigin::User).unwrap();
        assert_eq!(engine.project().sheets[0].entities.len(), 3);
        assert_eq!(engine.undo_depth(), 1, "3コマンドでも履歴は1件");

        engine.undo().unwrap().unwrap();
        assert!(
            engine.project().sheets[0].entities.is_empty(),
            "undo一発で全部戻る"
        );
        engine.redo().unwrap().unwrap();
        assert_eq!(engine.project().sheets[0].entities.len(), 3);
    }

    /// If any command in a batch fails, the whole batch is refused: the drawing is unchanged and nothing lands in the history.
    /// バッチ内の1つでも失敗したらバッチ全体を拒否し、図面は変わらず履歴にも残らない。
    #[test]
    fn execute_batch_refuses_everything_when_one_command_fails() {
        let (mut engine, sheet_id) = test_engine();
        let missing = Uuid::new_v4();
        let err = engine.execute_batch(
            vec![
                Command::AddEntity {
                    sheet_id,
                    entity: sample_wire(),
                },
                Command::AddEntity {
                    sheet_id: missing,
                    entity: sample_wire(),
                },
            ],
            EditOrigin::User,
        );
        assert!(err.is_err());
        assert!(engine.project().sheets[0].entities.is_empty(), "部分適用しない");
        assert_eq!(engine.undo_depth(), 0, "履歴も増えない");
    }

    /// An empty batch changes nothing at all: no history entry and no new document revision.
    /// 空のバッチは何も変えない(履歴も増えず、ドキュメントrevisionも進まない)。
    #[test]
    fn empty_batch_changes_nothing() {
        let (mut engine, _) = test_engine();
        let revision = engine.revision();
        let patch = engine.execute_batch(Vec::new(), EditOrigin::User).unwrap();
        assert!(patch.ops.is_empty());
        assert_eq!(engine.revision(), revision);
        assert_eq!(engine.undo_depth(), 0);
    }

    /// Reverting a range with no edits of that origin reports "nothing to do" instead of touching the drawing.
    /// その由来の編集が1件も無い区間の巻き戻しは、図面に触れず「戻すものが無い」と報告する。
    #[test]
    fn revert_range_without_matching_edits_changes_nothing() {
        let (mut engine, sheet_id) = test_engine();
        let start = engine.undo_depth();
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: sample_wire(),
            })
            .unwrap();
        let end = engine.undo_depth();
        let revision = engine.revision();
        assert!(engine
            .revert_range(start, end, EditOrigin::Agent)
            .unwrap()
            .is_none());
        assert_eq!(engine.project().sheets[0].entities.len(), 1);
        assert_eq!(engine.revision(), revision, "revisionも進まない");
    }

    // --- 盤レイアウトのアイテム番号 (バルーン) ---------------------------------

    /// 盤シート1枚を足し、そこへフットプリントを置いたエンジンを作る。
    fn panel_engine(places: &[(&str, f64, f64)]) -> (Engine, SheetId, Vec<EntityId>) {
        let mut engine = Engine::new(Project::new("panel"));
        engine
            .execute(Command::AddSheet {
                name: "盤".into(),
                size: PaperSize::A3,
                orientation: Orientation::Landscape,
                kind: SheetKind::Panel,
            })
            .unwrap();
        let sheet_id = engine.project().sheets.last().unwrap().id;
        let mut ids = Vec::new();
        for (reference, x, y) in places {
            let id = Uuid::new_v4();
            ids.push(id);
            engine
                .execute(Command::AddEntity {
                    sheet_id,
                    entity: Entity::Footprint(Footprint {
                        id,
                        at: Point::new(*x, *y),
                        w: 20.0,
                        h: 40.0,
                        rotation: 0,
                        reference: (*reference).into(),
                        part_no: String::new(),
                        item_no: None,
                    }),
                })
                .unwrap();
        }
        (engine, sheet_id, ids)
    }

    fn item_no_of(engine: &Engine, sheet_id: SheetId, id: EntityId) -> Option<u32> {
        match engine.project().sheet(sheet_id).unwrap().entities.get(&id) {
            Some(Entity::Footprint(fp)) => fp.item_no,
            other => panic!("フットプリントではない: {other:?}"),
        }
    }

    /// Numbering the panel gives every footprint on that sheet an item number in reading order.
    /// 盤の採番は、そのシートのフットプリント全てに読み順のアイテム番号を振る。
    #[test]
    fn renumbering_a_panel_numbers_every_footprint() {
        let (mut engine, sheet_id, ids) =
            panel_engine(&[("K2", 200.0, 80.0), ("K1", 100.0, 80.0)]);
        engine
            .execute(Command::RenumberPanelItems { sheet_id })
            .unwrap();
        assert_eq!(item_no_of(&engine, sheet_id, ids[1]), Some(1), "左のK1が1番");
        assert_eq!(item_no_of(&engine, sheet_id, ids[0]), Some(2));
    }

    /// Undoing the numbering puts the previous item numbers back, including parts that had no number at all.
    /// 採番のundoは元のアイテム番号を戻す(番号が無かった部品は番号の無い状態へ戻る)。
    #[test]
    fn undoing_the_numbering_restores_the_previous_numbers() {
        let (mut engine, sheet_id, ids) =
            panel_engine(&[("K2", 200.0, 80.0), ("K1", 100.0, 80.0)]);
        engine
            .execute(Command::SetPanelItemNumbers {
                sheet_id,
                numbers: vec![PanelItemNumber {
                    footprint_id: ids[0],
                    item_no: Some(9),
                }],
            })
            .unwrap();
        engine
            .execute(Command::RenumberPanelItems { sheet_id })
            .unwrap();
        assert_eq!(item_no_of(&engine, sheet_id, ids[0]), Some(2));
        engine.undo().unwrap();
        assert_eq!(item_no_of(&engine, sheet_id, ids[0]), Some(9), "元の番号へ戻る");
        assert_eq!(item_no_of(&engine, sheet_id, ids[1]), None, "無かった番号は無いまま");
    }

    /// Numbering the same panel a second time changes nothing on the drawing: the numbers are already the ones the reading order asks for.
    /// 同じ盤をもう一度採番しても図面は何も変わらない(すでに読み順どおりの番号が入っているため)。
    #[test]
    fn numbering_an_already_numbered_panel_changes_nothing() {
        let (mut engine, sheet_id, ids) = panel_engine(&[("K1", 100.0, 80.0)]);
        engine
            .execute(Command::RenumberPanelItems { sheet_id })
            .unwrap();
        let patch = engine
            .execute(Command::RenumberPanelItems { sheet_id })
            .unwrap();
        assert!(patch.ops.is_empty(), "図面は1つも変わらない: {:?}", patch.ops);
        assert_eq!(item_no_of(&engine, sheet_id, ids[0]), Some(1));
    }

    /// The numbering command is plain JSON, so agents and the CLI can send it.
    /// 採番コマンドは素のJSONで表現できるので、AIやCLIから送れる。
    #[test]
    fn the_panel_numbering_command_is_plain_json() {
        let (mut engine, sheet_id, _) = panel_engine(&[("K1", 100.0, 80.0)]);
        let json = format!(r#"{{"type":"renumber_panel_items","sheet_id":"{sheet_id}"}}"#);
        let command: Command = serde_json::from_str(&json).unwrap();
        engine.execute(command).unwrap();
    }

    /// Numbering a sheet that does not exist is refused instead of silently doing nothing.
    /// 存在しないシートの採番は、黙って何もせずに断られる。
    #[test]
    fn numbering_an_unknown_sheet_is_refused() {
        let (mut engine, _, _) = panel_engine(&[("K1", 100.0, 80.0)]);
        let missing = Uuid::new_v4();
        assert!(engine
            .execute(Command::RenumberPanelItems { sheet_id: missing })
            .is_err());
    }

    /// Changing a sheet's paper size and orientation (G15) keeps every existing entity at its original coordinates; nothing is moved or deleted.
    /// シートの用紙サイズ・向きを変えても (G15)、既存のエンティティは元の座標のまま動かず、消えもしない。
    #[test]
    fn changing_sheet_paper_keeps_entity_coordinates() {
        let (mut engine, sheet_id) = test_engine();
        let entity = Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "resistor".into(),
            at: Point::new(100.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "R1".into(),
            value: "10k".into(),
            attrs: Default::default(),
        });
        let id = entity.id();
        engine.execute(Command::AddEntity { sheet_id, entity }).unwrap();
        engine
            .execute(Command::SetSheetPaper {
                sheet_id,
                size: PaperSize::A4,
                orientation: Orientation::Portrait,
            })
            .unwrap();
        let sheet = &engine.project().sheets[0];
        assert_eq!(sheet.size, PaperSize::A4);
        assert_eq!(sheet.orientation, Orientation::Portrait);
        let Entity::Symbol(s) = &sheet.entities[&id] else {
            panic!("シンボルであること")
        };
        assert_eq!(s.at, Point::new(100.0, 50.0), "座標は変わらない");
    }

    /// Undoing a paper size change restores both the previous size and orientation.
    /// 用紙サイズの変更をundoすると、サイズと向きの両方が元へ戻る。
    #[test]
    fn undoing_a_paper_change_restores_size_and_orientation() {
        let (mut engine, sheet_id) = test_engine();
        assert_eq!(engine.project().sheets[0].size, PaperSize::A3);
        assert_eq!(engine.project().sheets[0].orientation, Orientation::Landscape);
        engine
            .execute(Command::SetSheetPaper {
                sheet_id,
                size: PaperSize::A4,
                orientation: Orientation::Portrait,
            })
            .unwrap();
        engine.undo().unwrap().unwrap();
        assert_eq!(engine.project().sheets[0].size, PaperSize::A3);
        assert_eq!(engine.project().sheets[0].orientation, Orientation::Landscape);
    }

    /// Changing the paper leaves the sheet's own zone division alone: a sheet divided into its own number of zones keeps that division.
    /// 用紙を変えてもシート独自のゾーン分割はそのまま: 独自の分割数を持つシートはその分割を保つ。
    #[test]
    fn changing_paper_keeps_the_sheets_own_zone_division() {
        let mut project = Project::new("t");
        project.sheets[0].zone_cols = 8;
        project.sheets[0].zone_rows = 5;
        let sheet_id = project.sheets[0].id;
        let mut engine = Engine::new(project);
        engine
            .execute(Command::SetSheetPaper {
                sheet_id,
                size: PaperSize::A4,
                orientation: Orientation::Portrait,
            })
            .unwrap();
        assert_eq!(engine.project().sheets[0].zone_cols, 8);
        assert_eq!(engine.project().sheets[0].zone_rows, 5);
    }

    /// Undoing a paper change puts the sheet back exactly as it was, so a cross-reference zone address computed on it points at the same zone as before.
    /// 用紙変更をundoするとシートは元どおりになるので、そこで求めたクロスリファレンスのゾーン住所も変更前と同じゾーンを指す。
    #[test]
    fn undoing_a_paper_change_restores_the_zone_addresses() {
        let mut project = Project::new("t");
        project.sheets[0].zone_cols = 8;
        project.sheets[0].zone_rows = 5;
        let sheet_id = project.sheets[0].id;
        let mut engine = Engine::new(project);
        let at = Point::new(200.0, 120.0);
        let before = crate::xref::zone_at(&engine.project().sheets[0], at);
        engine
            .execute(Command::SetSheetPaper {
                sheet_id,
                size: PaperSize::A4,
                orientation: Orientation::Portrait,
            })
            .unwrap();
        engine.undo().unwrap().unwrap();
        assert_eq!(crate::xref::zone_at(&engine.project().sheets[0], at), before);
    }

    /// A paper size change on a sheet that does not exist is refused.
    /// 存在しないシートの用紙変更は断られる。
    #[test]
    fn changing_paper_on_an_unknown_sheet_is_refused() {
        let (mut engine, _sheet_id) = test_engine();
        let missing = Uuid::new_v4();
        assert!(engine
            .execute(Command::SetSheetPaper {
                sheet_id: missing,
                size: PaperSize::A4,
                orientation: Orientation::Portrait,
            })
            .is_err());
    }

    /// `set_project_meta` sets the project name and number in one command, leaving every sheet's title block untouched when those fields are omitted.
    /// `set_project_meta`はプロジェクト名と番号を1コマンドで設定し、その欄を省略すれば全シートの表題欄には触れない。
    #[test]
    fn set_project_meta_sets_name_and_number() {
        let (mut engine, sheet_id) = test_engine();
        let before_company = engine.project().sheets[0].title_block.company.clone();
        engine
            .execute(Command::SetProjectMeta {
                name: Some("まだけ制御盤".into()),
                no: Some(Some("MDK-2026-001".into())),
                company: None,
                drawing_no: None,
                title: None,
            })
            .unwrap();
        assert_eq!(engine.project().name, "まだけ制御盤");
        assert_eq!(engine.project().no.as_deref(), Some("MDK-2026-001"));
        assert_eq!(
            engine.project().sheets[0].title_block.company,
            before_company,
            "表題欄は変わらない"
        );
        let _ = sheet_id;
    }

    /// `set_project_meta` writes the title-block fields (company, drawing number, title) to every sheet in the project with one undoable command.
    /// `set_project_meta`は表題欄の欄(会社名・図番・表題)をプロジェクトの全シートへ1つのundo可能なコマンドで書き込む。
    #[test]
    fn set_project_meta_writes_title_block_to_every_sheet() {
        let mut project = Project::new("t");
        project.sheets.push(Sheet::new("Sheet2", PaperSize::A3, Orientation::Landscape));
        let mut engine = Engine::new(project);
        engine
            .execute(Command::SetProjectMeta {
                name: None,
                no: None,
                company: Some("まだけ電機".into()),
                drawing_no: Some("MDK-100".into()),
                title: Some("主回路".into()),
            })
            .unwrap();
        for sheet in &engine.project().sheets {
            assert_eq!(sheet.title_block.company, "まだけ電機");
            assert_eq!(sheet.title_block.drawing_no, "MDK-100");
            assert_eq!(sheet.title_block.title, "主回路");
        }
    }

    /// Undoing `set_project_meta` restores each sheet's own previous title block, even when the sheets had different values beforehand (not a single shared value).
    /// `set_project_meta`のundoは、各シートがそれぞれ異なる元の値を持っていた場合でも、シートごとの元の表題欄を正しく復元する(共通の1値には戻さない)。
    #[test]
    fn undoing_set_project_meta_restores_each_sheets_own_title_block() {
        let mut project = Project::new("t");
        project.sheets[0].title_block.drawing_no = "OLD-1".into();
        project.sheets.push(Sheet::new("Sheet2", PaperSize::A3, Orientation::Landscape));
        project.sheets[1].title_block.drawing_no = "OLD-2".into();
        let mut engine = Engine::new(project);
        engine
            .execute(Command::SetProjectMeta {
                name: None,
                no: None,
                company: None,
                drawing_no: Some("NEW".into()),
                title: None,
            })
            .unwrap();
        engine.undo().unwrap().unwrap();
        assert_eq!(engine.project().sheets[0].title_block.drawing_no, "OLD-1");
        assert_eq!(engine.project().sheets[1].title_block.drawing_no, "OLD-2");
    }

    /// The project number command is plain JSON, so agents, the CLI and the Link API can all send it.
    /// プロジェクト番号のコマンドは素のJSONで表現できるので、AI・CLI・Link APIのどこからでも送れる。
    #[test]
    fn set_project_meta_command_is_plain_json() {
        let (mut engine, _sheet_id) = test_engine();
        let json = r#"{"type":"set_project_meta","no":"MDK-2026-001"}"#;
        let command: Command = serde_json::from_str(json).unwrap();
        engine.execute(command).unwrap();
        assert_eq!(engine.project().no.as_deref(), Some("MDK-2026-001"));
    }

    /// Undoing the very first project number puts the project back to having no number at all, not to an empty one.
    /// 番号が無かったプロジェクトに初めて番号を付けてundoすると、空の番号ではなく「番号なし」に戻る。
    #[test]
    fn undoing_the_first_project_number_restores_the_unset_state() {
        let (mut engine, _sheet_id) = test_engine();
        assert_eq!(engine.project().no, None, "はじめは番号なし");
        engine
            .execute(Command::SetProjectMeta {
                name: None,
                no: Some(Some("MDK-2026-001".into())),
                company: None,
                drawing_no: None,
                title: None,
            })
            .unwrap();
        engine.undo().unwrap().unwrap();
        assert_eq!(engine.project().no, None, "空文字ではなく未設定へ戻る");
    }

    /// Sending the project number as null clears it back to unset, which is how a number set by mistake is removed.
    /// プロジェクト番号にnullを送ると未設定へ戻る(間違って付けた番号を消す手段)。
    #[test]
    fn a_null_project_number_clears_it() {
        let (mut engine, _sheet_id) = test_engine();
        engine
            .execute(serde_json::from_str(r#"{"type":"set_project_meta","no":"MDK-2026-001"}"#).unwrap())
            .unwrap();
        engine
            .execute(serde_json::from_str(r#"{"type":"set_project_meta","no":null}"#).unwrap())
            .unwrap();
        assert_eq!(engine.project().no, None);
    }

    /// Undoing a cleared project number brings the previous number back exactly as it was.
    /// 消したプロジェクト番号をundoすると、元の番号がそのまま戻る。
    #[test]
    fn undoing_a_cleared_project_number_brings_it_back() {
        let (mut engine, _sheet_id) = test_engine();
        engine
            .execute(Command::SetProjectMeta {
                name: None,
                no: Some(Some("MDK-2026-001".into())),
                company: None,
                drawing_no: None,
                title: None,
            })
            .unwrap();
        engine
            .execute(Command::SetProjectMeta {
                name: None,
                no: Some(None),
                company: None,
                drawing_no: None,
                title: None,
            })
            .unwrap();
        engine.undo().unwrap().unwrap();
        assert_eq!(engine.project().no.as_deref(), Some("MDK-2026-001"));
    }

    /// Leaving the project number out of the command leaves an existing number untouched, even while other fields change.
    /// コマンドからプロジェクト番号の欄を省くと、ほかの欄を変えても既にある番号はそのまま残る。
    #[test]
    fn omitting_the_project_number_leaves_it_untouched() {
        let (mut engine, _sheet_id) = test_engine();
        engine
            .execute(serde_json::from_str(r#"{"type":"set_project_meta","no":"MDK-2026-001"}"#).unwrap())
            .unwrap();
        engine
            .execute(serde_json::from_str(r#"{"type":"set_project_meta","name":"まだけ制御盤"}"#).unwrap())
            .unwrap();
        assert_eq!(engine.project().name, "まだけ制御盤");
        assert_eq!(engine.project().no.as_deref(), Some("MDK-2026-001"));
    }
}
