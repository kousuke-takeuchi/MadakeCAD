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
    /// 削除の逆操作・復元用。位置(index)を保ってシートを戻す。
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
}

struct HistoryEntry {
    forward: Command,
    inverse: Vec<Command>,
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

    /// コマンドを実行し、undo履歴に積む。
    pub fn execute(&mut self, cmd: Command) -> Result<Patch> {
        let (ops, inverse) = self.apply(&cmd)?;
        self.undo_stack.push(HistoryEntry {
            forward: cmd,
            inverse,
        });
        self.redo_stack.clear();
        self.revision += 1;
        Ok(Patch {
            revision: self.revision,
            ops,
        })
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
        let (ops, inverse) = self.apply(&entry.forward)?;
        self.undo_stack.push(HistoryEntry {
            forward: entry.forward,
            inverse,
        });
        self.revision += 1;
        Ok(Some(Patch {
            revision: self.revision,
            ops,
        }))
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
                let old = sheet
                    .entities
                    .insert(id, entity.clone())
                    .ok_or(CoreError::EntityNotFound(id))?;
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
}
