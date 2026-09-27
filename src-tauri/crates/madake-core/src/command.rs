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
    /// FreeCADオブジェクトとの対応付けを登録・更新する (entity_idがキー。M5-2)。
    SetMechLink {
        link: MechLink,
    },
    /// FreeCADオブジェクトとの対応付けを外す。
    RemoveMechLink {
        entity_id: EntityId,
    },
    /// 配線の長さをまとめて書き換える (FreeCADの経路計測の書き戻し。出所も一緒に記録)。
    /// 逆コマンドは書き換え前の長さと出所。
    SetWireLengths {
        sheet_id: SheetId,
        lengths: Vec<WireLength>,
    },
}

/// 1本の配線の長さと出所 (`set_wire_lengths` コマンドの要素)。length_mがnullなら長さを消す。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WireLength {
    pub wire_id: EntityId,
    #[serde(default)]
    pub length_m: Option<f64>,
    #[serde(default)]
    pub source: LengthSource,
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
    /// FreeCAD対応付けの一覧が置き換わった (1件の登録・解除でも全体を送る)。
    MechLinksReplaced { mech_links: Vec<MechLink> },
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
            } => {
                let sheet = Sheet::new(name, *size, *orientation);
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
            Command::AddEntity { sheet_id, entity } => {
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
            Command::SetMechLink { link } => {
                let previous = self
                    .project
                    .mech_links
                    .iter()
                    .position(|l| l.entity_id == link.entity_id)
                    .map(|i| self.project.mech_links.remove(i));
                self.project.mech_links.push(link.clone());
                let inverse = match previous {
                    Some(old) => Command::SetMechLink { link: old },
                    None => Command::RemoveMechLink {
                        entity_id: link.entity_id,
                    },
                };
                Ok((
                    vec![PatchOp::MechLinksReplaced {
                        mech_links: self.project.mech_links.clone(),
                    }],
                    vec![inverse],
                ))
            }
            Command::RemoveMechLink { entity_id } => {
                let index = self
                    .project
                    .mech_links
                    .iter()
                    .position(|l| l.entity_id == *entity_id)
                    .ok_or(CoreError::EntityNotFound(*entity_id))?;
                let old = self.project.mech_links.remove(index);
                Ok((
                    vec![PatchOp::MechLinksReplaced {
                        mech_links: self.project.mech_links.clone(),
                    }],
                    vec![Command::SetMechLink { link: old }],
                ))
            }
            Command::SetWireLengths { sheet_id, lengths } => {
                let sheet = self
                    .project
                    .sheet_mut(*sheet_id)
                    .ok_or(CoreError::SheetNotFound(*sheet_id))?;
                let mut ops = Vec::new();
                let mut old = Vec::new();
                for WireLength { wire_id, length_m, source } in lengths {
                    let Some(Entity::Wire(w)) = sheet.entities.get_mut(wire_id) else {
                        return Err(CoreError::EntityNotFound(*wire_id));
                    };
                    if w.length_m == *length_m && w.length_source == *source {
                        continue;
                    }
                    old.push(WireLength {
                        wire_id: *wire_id,
                        length_m: w.length_m,
                        source: w.length_source,
                    });
                    w.length_m = *length_m;
                    w.length_source = *source;
                    ops.push(PatchOp::EntityUpserted {
                        sheet_id: *sheet_id,
                        entity: Entity::Wire(w.clone()),
                    });
                }
                Ok((
                    ops,
                    vec![Command::SetWireLengths {
                        sheet_id: *sheet_id,
                        lengths: old,
                    }],
                ))
            }
        }
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
            length_source: Default::default(),
            part_no: None,
            net: None,
        })
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

    fn mech_link(entity_id: Uuid, object_name: &str) -> MechLink {
        MechLink {
            entity_id,
            fcstd_path: "/work/panel.FCStd".into(),
            object_name: object_name.into(),
            synced_at: "2026-09-27T10:00:00Z".into(),
        }
    }

    /// Registering a FreeCAD link stores it under the entity id, registering again replaces it, removing it takes it away, and each step undoes back to the previous list.
    /// FreeCAD対応付けを登録するとentity idをキーに保存され、再登録で置き換わり、解除で消える。各操作はundoで直前の一覧に戻る。
    #[test]
    fn mech_links_are_upserted_removed_and_undone() {
        let (mut engine, _) = test_engine();
        let id = Uuid::new_v4();
        let patch = engine.execute(Command::SetMechLink { link: mech_link(id, "Relay001") }).unwrap();
        assert!(matches!(&patch.ops[0], PatchOp::MechLinksReplaced { mech_links } if mech_links.len() == 1));
        engine.execute(Command::SetMechLink { link: mech_link(id, "Relay002") }).unwrap();
        assert_eq!(engine.project().mech_links.len(), 1);
        assert_eq!(engine.project().mech_links[0].object_name, "Relay002");
        engine.execute(Command::RemoveMechLink { entity_id: id }).unwrap();
        assert!(engine.project().mech_links.is_empty());
        engine.undo().unwrap();
        assert_eq!(engine.project().mech_links[0].object_name, "Relay002");
        engine.undo().unwrap();
        assert_eq!(engine.project().mech_links[0].object_name, "Relay001");
        engine.undo().unwrap();
        assert!(engine.project().mech_links.is_empty());
        assert!(matches!(
            engine.execute(Command::RemoveMechLink { entity_id: id }),
            Err(CoreError::EntityNotFound(_))
        ));
    }

    /// Writing measured wire lengths back sets each wire's length and marks it as measured by FreeCAD in one undo step; unchanged wires are skipped and an unknown wire is an error.
    /// 計測した電線長の書き戻しは、各配線の長さを設定して出所を「FreeCAD計測」にする1回の編集になる。変わらない配線は飛ばし、存在しない配線はエラーになる。
    #[test]
    fn wire_lengths_write_back_with_their_source() {
        let (mut engine, sheet_id) = test_engine();
        let wire = sample_wire();
        let wire_id = wire.id();
        engine.execute(Command::AddEntity { sheet_id, entity: wire }).unwrap();
        let patch = engine
            .execute(Command::SetWireLengths {
                sheet_id,
                lengths: vec![WireLength { wire_id, length_m: Some(1.25), source: LengthSource::Freecad }],
            })
            .unwrap();
        assert_eq!(patch.ops.len(), 1);
        let Entity::Wire(w) = &engine.project().sheets[0].entities[&wire_id] else { panic!() };
        assert_eq!((w.length_m, w.length_source), (Some(1.25), LengthSource::Freecad));
        // 同じ値をもう一度: 変更なし (patchは空)
        let patch = engine
            .execute(Command::SetWireLengths {
                sheet_id,
                lengths: vec![WireLength { wire_id, length_m: Some(1.25), source: LengthSource::Freecad }],
            })
            .unwrap();
        assert!(patch.ops.is_empty());
        engine.undo().unwrap();
        engine.undo().unwrap();
        let Entity::Wire(w) = &engine.project().sheets[0].entities[&wire_id] else { panic!() };
        assert_eq!((w.length_m, w.length_source), (Some(0.4), LengthSource::Manual));
        assert!(matches!(
            engine.execute(Command::SetWireLengths {
                sheet_id,
                lengths: vec![WireLength { wire_id: Uuid::new_v4(), length_m: None, source: LengthSource::Manual }],
            }),
            Err(CoreError::EntityNotFound(_))
        ));
    }
}
