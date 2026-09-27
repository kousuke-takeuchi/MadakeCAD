//! 整えバリアント (M3フェーズ4): 同じ整え指示を複数の案で並列に走らせ、1案だけ採用する。
//!
//! 仕組みは「シート複製」。元シートを案の数だけ複製し(エンティティidは振り直し、
//! 元id→複製idの対応表を保持)、案ごとに別の会話で整えを走らせる。比較が済んだら
//! 選んだ案の内容を**元シートへ写し戻し**(対応表で元idを保つ)、複製を全て消す。
//! 開始・採用・破棄はいずれもCommandの列で、`execute_batch`で1回の編集=undo一発。
//!
//! 複製は普通のシート(モデルに「案」フラグは無い)。比較中はクロスリファレンスや
//! 検証に複製分が混ざるが、整えは「増えていないか」の相対比較なので影響しない。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::command::Command;
use crate::model::{Entity, EntityId, Sheet, SheetId};

/// 案の数の上限 (ポップアップのチップ 2/3/4 と一致)。
pub const VARIANT_MAX: usize = 4;

/// 1案の情報。クライアントが開始→終了の間だけ保持し、終了時にそのまま返す。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct VariantInfo {
    /// 複製シートのid。
    pub sheet_id: SheetId,
    /// 表示ラベル ("案A" など)。
    pub label: String,
    /// 元シートのエンティティid → 複製側のid。
    pub id_map: BTreeMap<EntityId, EntityId>,
}

/// 開始結果: 複製を入れるコマンド列と、案ごとの情報。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct VariantRun {
    pub original_sheet_id: SheetId,
    pub variants: Vec<VariantInfo>,
}

/// 案のラベル ("案A", "案B", …)。
pub fn variant_label(index: usize) -> String {
    let letter = (b'A' + (index % 26) as u8) as char;
    format!("案{letter}")
}

/// 元シートの複製を1枚作る。用紙・表題欄・改訂欄・全エンティティは同じ形で、
/// シートidとエンティティidだけ新しい。戻り値の対応表は元id→複製id。
pub fn duplicate_sheet(original: &Sheet, label: &str) -> (Sheet, BTreeMap<EntityId, EntityId>) {
    let mut copy = original.clone();
    copy.id = Uuid::new_v4();
    copy.name = format!("{} · {label}", original.name);
    copy.entities = BTreeMap::new();
    let mut id_map = BTreeMap::new();
    for (old_id, entity) in &original.entities {
        let mut e = entity.clone();
        let new_id = Uuid::new_v4();
        e.set_id(new_id);
        id_map.insert(*old_id, new_id);
        copy.entities.insert(new_id, e);
    }
    (copy, id_map)
}

/// 案の開始: 元シートを`count`枚複製して末尾へ入れるコマンド列と、案の情報。
/// `count`は1..=[`VARIANT_MAX`]。`base_index`は複製を差し込む位置 (通常はシート数)。
pub fn start_commands(
    original: &Sheet,
    count: usize,
    base_index: usize,
) -> Result<(Vec<Command>, VariantRun), String> {
    if !(1..=VARIANT_MAX).contains(&count) {
        return Err(format!("案の数は1〜{VARIANT_MAX}です (指定: {count})"));
    }
    let mut commands = Vec::with_capacity(count);
    let mut variants = Vec::with_capacity(count);
    for i in 0..count {
        let label = variant_label(i);
        let (sheet, id_map) = duplicate_sheet(original, &label);
        variants.push(VariantInfo {
            sheet_id: sheet.id,
            label,
            id_map,
        });
        commands.push(Command::RestoreSheet {
            index: base_index + i,
            sheet: Box::new(sheet),
        });
    }
    Ok((
        commands,
        VariantRun {
            original_sheet_id: original.id,
            variants,
        },
    ))
}

/// エンティティの中身が同じか (idを除く)。
fn same_content(a: &Entity, b: &Entity) -> bool {
    let mut b = b.clone();
    b.set_id(a.id());
    serde_json::to_value(a).ok() == serde_json::to_value(&b).ok()
}

/// 採用: 選んだ複製の内容を元シートへ写し戻し、全複製を消すコマンド列。
///
/// - 対応表にある要素: 複製側で変わっていれば**元idのまま**`UpdateEntity`、複製側で
///   消えていれば`DeleteEntities`、変わっていなければ何もしない
/// - 複製側で増えた要素 (対応表に無いid): 新しいidで`AddEntity`
/// - 最後に全複製の`RemoveSheet`
pub fn adopt_commands(
    original: &Sheet,
    chosen: &Sheet,
    chosen_map: &BTreeMap<EntityId, EntityId>,
    variant_sheet_ids: &[SheetId],
) -> Vec<Command> {
    let mut commands = Vec::new();
    let mut deleted = Vec::new();
    for (orig_id, copy_id) in chosen_map {
        let Some(orig) = original.entities.get(orig_id) else {
            continue;
        };
        match chosen.entities.get(copy_id) {
            Some(copy) => {
                if !same_content(orig, copy) {
                    let mut entity = copy.clone();
                    entity.set_id(*orig_id);
                    commands.push(Command::UpdateEntity {
                        sheet_id: original.id,
                        entity,
                    });
                }
            }
            None => deleted.push(*orig_id),
        }
    }
    if !deleted.is_empty() {
        commands.push(Command::DeleteEntities {
            sheet_id: original.id,
            ids: deleted,
        });
    }
    let mapped: std::collections::BTreeSet<EntityId> = chosen_map.values().copied().collect();
    for (copy_id, entity) in &chosen.entities {
        if mapped.contains(copy_id) {
            continue;
        }
        let mut e = entity.clone();
        e.set_id(Uuid::new_v4());
        commands.push(Command::AddEntity {
            sheet_id: original.id,
            entity: e,
        });
    }
    commands.extend(discard_commands(variant_sheet_ids));
    commands
}

/// 破棄: 全複製を消すコマンド列。
pub fn discard_commands(variant_sheet_ids: &[SheetId]) -> Vec<Command> {
    variant_sheet_ids
        .iter()
        .map(|id| Command::RemoveSheet { sheet_id: *id })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{EditOrigin, Engine};
    use crate::geometry::Point;
    use crate::model::*;

    fn sheet_with_entities() -> Sheet {
        let mut sheet = Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape);
        sheet.title_block.title = "制御盤".into();
        let symbol = Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "resistor".into(),
            at: Point::new(100.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "R1".into(),
            value: "1k".into(),
            attrs: Default::default(),
        });
        let wire = Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(10.0, 10.0), Point::new(50.0, 10.0)],
            color: "red".into(),
            sq: 0.75,
            length_m: None,
            length_source: Default::default(),
            part_no: None,
            net: Some("101".into()),
        });
        for e in [symbol, wire] {
            sheet.entities.insert(e.id(), e);
        }
        sheet
    }

    fn symbol_id_of(sheet: &Sheet) -> EntityId {
        sheet
            .entities
            .values()
            .find_map(|e| match e {
                Entity::Symbol(s) => Some(s.id),
                _ => None,
            })
            .unwrap()
    }

    /// A variant copy keeps the paper, title block and every entity of the original, with a new sheet id, new entity ids, the label in its name, and a complete old-to-new id map.
    /// 案の複製は用紙・表題欄・全エンティティを元のまま持ち、シートidとエンティティidだけ新しく、名前にラベルが付き、元id→新idの対応表が全要素分できる。
    #[test]
    fn duplicate_keeps_content_with_fresh_ids_and_a_full_map() {
        let original = sheet_with_entities();
        let (copy, id_map) = duplicate_sheet(&original, "案A");
        assert_ne!(copy.id, original.id);
        assert_eq!(copy.name, "Sheet1 · 案A");
        assert_eq!(copy.size, original.size);
        assert_eq!(copy.title_block.title, "制御盤");
        assert_eq!(copy.entities.len(), original.entities.len());
        assert_eq!(id_map.len(), original.entities.len());
        for (old_id, new_id) in &id_map {
            assert_ne!(old_id, new_id);
            assert!(original.entities.contains_key(old_id));
            let copied = copy.entities.get(new_id).expect("copied entity");
            assert_eq!(copied.id(), *new_id);
            assert!(same_content(&original.entities[old_id], copied));
        }
    }

    /// Starting a run with N variants yields N RestoreSheet commands appended after the existing sheets, each with its own label and id map; N outside 1..=4 is rejected.
    /// N案で開始すると既存シートの後ろへ入るRestoreSheetがN個でき、案ごとにラベルと対応表が付く。1〜4以外の数は拒否される。
    #[test]
    fn start_builds_one_restore_sheet_per_variant() {
        let original = sheet_with_entities();
        let (commands, run) = start_commands(&original, 3, 1).unwrap();
        assert_eq!(commands.len(), 3);
        assert_eq!(run.original_sheet_id, original.id);
        let labels: Vec<&str> = run.variants.iter().map(|v| v.label.as_str()).collect();
        assert_eq!(labels, vec!["案A", "案B", "案C"]);
        for (i, cmd) in commands.iter().enumerate() {
            let Command::RestoreSheet { index, sheet } = cmd else {
                panic!("RestoreSheet")
            };
            assert_eq!(*index, 1 + i);
            assert_eq!(sheet.id, run.variants[i].sheet_id);
        }
        // 案ごとに複製idは独立
        let a: Vec<_> = run.variants[0].id_map.values().collect();
        let b: Vec<_> = run.variants[1].id_map.values().collect();
        assert!(a.iter().all(|id| !b.contains(id)));
        assert!(start_commands(&original, 0, 1).is_err());
        assert!(start_commands(&original, 5, 1).is_err());
    }

    /// Adopting a variant writes moved entities back under their original ids, deletes what the variant removed, adds what it created with fresh ids, leaves untouched entities alone, and finally removes every variant sheet.
    /// 採用すると、動かした要素は元idのまま更新、案で消えた要素は削除、案で増えた要素は新idで追加、変わらない要素は触らず、最後に全ての案シートが消える。
    #[test]
    fn adopt_maps_changes_back_to_original_ids() {
        let original = sheet_with_entities();
        let (mut chosen, id_map) = duplicate_sheet(&original, "案A");
        let (other, _) = duplicate_sheet(&original, "案B");
        let orig_symbol = symbol_id_of(&original);
        let copy_symbol = id_map[&orig_symbol];
        // 動かす
        if let Some(Entity::Symbol(s)) = chosen.entities.get_mut(&copy_symbol) {
            s.at = Point::new(120.0, 50.0);
        }
        // 消す (配線)
        let orig_wire = *id_map.keys().find(|id| **id != orig_symbol).unwrap();
        chosen.entities.remove(&id_map[&orig_wire]);
        // 増やす
        let added = Entity::Junction(Junction {
            id: Uuid::new_v4(),
            at: Point::new(30.0, 10.0),
        });
        chosen.entities.insert(added.id(), added);

        let commands = adopt_commands(&original, &chosen, &id_map, &[chosen.id, other.id]);
        let updates: Vec<&Entity> = commands
            .iter()
            .filter_map(|c| match c {
                Command::UpdateEntity { sheet_id, entity } if *sheet_id == original.id => {
                    Some(entity)
                }
                _ => None,
            })
            .collect();
        assert_eq!(updates.len(), 1, "動かした要素だけ更新: {commands:?}");
        assert_eq!(updates[0].id(), orig_symbol, "元idのまま");
        assert!(matches!(updates[0], Entity::Symbol(s) if s.at.x == 120.0));
        assert!(commands
            .iter()
            .any(|c| matches!(c, Command::DeleteEntities { ids, .. } if ids == &vec![orig_wire])));
        let adds: Vec<&Entity> = commands
            .iter()
            .filter_map(|c| match c {
                Command::AddEntity { entity, .. } => Some(entity),
                _ => None,
            })
            .collect();
        assert_eq!(adds.len(), 1);
        assert!(matches!(adds[0], Entity::Junction(_)));
        assert!(
            !chosen.entities.contains_key(&adds[0].id()),
            "増えた要素は新id"
        );
        let removed: Vec<SheetId> = commands
            .iter()
            .filter_map(|c| match c {
                Command::RemoveSheet { sheet_id } => Some(*sheet_id),
                _ => None,
            })
            .collect();
        assert_eq!(removed, vec![chosen.id, other.id]);
        assert!(matches!(commands.last(), Some(Command::RemoveSheet { .. })));
    }

    /// Adopting an unchanged variant only removes the variant sheets.
    /// 何も変わっていない案を採用すると、案シートを消すだけになる。
    #[test]
    fn adopting_an_unchanged_variant_only_removes_sheets() {
        let original = sheet_with_entities();
        let (chosen, id_map) = duplicate_sheet(&original, "案A");
        let commands = adopt_commands(&original, &chosen, &id_map, &[chosen.id]);
        assert_eq!(commands.len(), 1);
        assert!(matches!(commands[0], Command::RemoveSheet { .. }));
        assert_eq!(discard_commands(&[chosen.id, original.id]).len(), 2);
    }

    /// Through the engine, start → edit a variant → adopt leaves the original sheet with the variant's placement and no variant sheets, and each of the three steps is one undo.
    /// エンジンで開始→案を編集→採用すると、元シートが案の配置になり案シートは残らない。開始・編集・採用はそれぞれundo1回で戻る。
    #[test]
    fn engine_round_trip_is_one_undo_per_step() {
        let mut project = Project::new("t");
        project.sheets[0] = sheet_with_entities();
        let original_id = project.sheets[0].id;
        let orig_symbol = symbol_id_of(&project.sheets[0]);
        let mut engine = Engine::new(project);

        let (commands, run) =
            start_commands(engine.project().sheet(original_id).unwrap(), 2, 1).unwrap();
        engine.execute_batch(commands, EditOrigin::User).unwrap();
        assert_eq!(engine.project().sheets.len(), 3);

        // 案Aでシンボルを動かす (エージェントの編集に相当)
        let a = &run.variants[0];
        let copy_symbol = a.id_map[&orig_symbol];
        let mut moved = engine.project().sheet(a.sheet_id).unwrap().entities[&copy_symbol].clone();
        if let Entity::Symbol(s) = &mut moved {
            s.at = Point::new(200.0, 75.0);
        }
        engine
            .execute_as(
                Command::UpdateEntity {
                    sheet_id: a.sheet_id,
                    entity: moved,
                },
                EditOrigin::Agent,
            )
            .unwrap();

        let sheet_ids: Vec<SheetId> = run.variants.iter().map(|v| v.sheet_id).collect();
        let commands = {
            let project = engine.project();
            adopt_commands(
                project.sheet(original_id).unwrap(),
                project.sheet(a.sheet_id).unwrap(),
                &a.id_map,
                &sheet_ids,
            )
        };
        engine.execute_batch(commands, EditOrigin::User).unwrap();
        assert_eq!(engine.project().sheets.len(), 1);
        let Entity::Symbol(s) = &engine.project().sheets[0].entities[&orig_symbol] else {
            panic!()
        };
        assert_eq!((s.at.x, s.at.y), (200.0, 75.0));

        engine.undo().unwrap();
        assert_eq!(engine.project().sheets.len(), 3, "採用はundo1回");
        let Entity::Symbol(s) = &engine.project().sheets[0].entities[&orig_symbol] else {
            panic!()
        };
        assert_eq!(s.at.x, 100.0);
        engine.undo().unwrap();
        engine.undo().unwrap();
        assert_eq!(engine.project().sheets.len(), 1, "開始もundo1回");
    }
}
