use serde::{Deserialize, Serialize};

use crate::model::*;
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
        }
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
}
