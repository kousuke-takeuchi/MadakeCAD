//! 整えバリアント (M3フェーズ4): 同じ整え指示を複数の案で並列に走らせ、1案だけ採用する。
//!
//! 仕組みは「シート複製」。元シートを案の数だけ複製し(エンティティidは振り直し、
//! 元id→複製idの対応表を保持)、案ごとに別の会話で整えを走らせる。比較が済んだら
//! 選んだ案の内容を**元シートへ写し戻し**(対応表で元idを保つ)、複製を全て消す。
//! 開始・採用・破棄はいずれもCommandの列で、`execute_batch`で1回の編集=undo一発。
//!
//! 複製は**印つきの普通のシート**([`Sheet::variant_of`])。印は図面全体を見渡す自動注記
//! から複製を除くために使う: シート間クロスリファレンス (G13) は「複数シートに跨る
//! 参照記号」に住所注記を出すので、印が無いと元シートの全シンボルが多シート扱いになり、
//! 元シートにも各案にも存在しない住所 (`/2.B3`) が生える ([`crate::xref::device_sites`])。
//! 整え指標 ([`crate::tidy`]) はシート内しか見ないので比較そのものは元から汚れない。

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::command::Command;
use crate::model::{Entity, EntityId, Project, Sheet, SheetId};

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
    /// 開始した時点の元シートの中身の指紋 ([`sheet_digest`])。
    ///
    /// 採用は「今の元シート」と案を比べて差分を書き戻すので、比較中にユーザーが元シートを
    /// 手で編集していると、その手編集が**警告なしに案の状態へ戻ってしまう**。採用の直前に
    /// これを照合し、変わっていたら断る ([`validate_run`])。古いクライアントからの
    /// 空文字は照合しない。
    #[serde(default)]
    pub original_digest: String,
}

/// シートの中身 (エンティティ) の指紋。採用が書き戻す対象が開始時と同じかを見るためだけの
/// 値なので、同じプロセスの中で比べられれば十分 (FNV-1a)。
pub fn sheet_digest(sheet: &Sheet) -> String {
    let json = serde_json::to_string(&sheet.entities).unwrap_or_default();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in json.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// 案のラベル ("案A", "案B", …)。
pub fn variant_label(index: usize) -> String {
    let letter = (b'A' + (index % 26) as u8) as char;
    format!("案{letter}")
}

/// 元シートの複製を1枚作る。用紙・表題欄・改訂欄・全エンティティは同じ形で、
/// シートidとエンティティidだけ新しい。戻り値の対応表は元id→複製id。
///
/// **シート内のエンティティ参照も一緒に写す**: 引き出し線つきの注記 (G20) の対象idを
/// 写さないと、複製の注記は元シートのidを指したままになり、引き出し線が描かれず、
/// ERCが複製を不正 (`erc.leader_target_missing`) と判定する ([`Entity::remap_refs`])。
pub fn duplicate_sheet(original: &Sheet, label: &str) -> (Sheet, BTreeMap<EntityId, EntityId>) {
    let mut copy = original.clone();
    copy.id = Uuid::new_v4();
    copy.name = format!("{} · {label}", original.name);
    copy.variant_of = Some(original.id);
    copy.entities = BTreeMap::new();
    let mut id_map = BTreeMap::new();
    for (old_id, entity) in &original.entities {
        let mut e = entity.clone();
        let new_id = Uuid::new_v4();
        e.set_id(new_id);
        id_map.insert(*old_id, new_id);
        copy.entities.insert(new_id, e);
    }
    // idを振り終えてから参照を写す (対応表が揃っていないと写せない)
    for entity in copy.entities.values_mut() {
        entity.remap_refs(&id_map);
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
            original_digest: sheet_digest(original),
        },
    ))
}

/// 終了 (採用・破棄) の前に、クライアントから来た実行情報が今の図面と噛み合うか確かめる。
///
/// 実行の台帳はクライアントが持って往復させる設計なので、古い・作り物の`run`がそのまま
/// 通ると**無関係のシートへ写し戻す**・`RemoveSheet`が失敗してバッチごと落ちる、という
/// 壊れ方をする。噛み合わない理由を文で返して、写し戻す前に断る。
pub fn validate_run(
    project: &Project,
    run: &VariantRun,
    chosen: Option<SheetId>,
) -> Result<(), String> {
    let original = project
        .sheet(run.original_sheet_id)
        .ok_or_else(|| format!("元シートが見つかりません: {}", run.original_sheet_id))?;
    if original.is_variant() {
        return Err(format!(
            "元シートに案の複製が指定されています: {}",
            run.original_sheet_id
        ));
    }
    if run.variants.is_empty() {
        return Err("案が1つもありません".to_string());
    }
    let mut seen = BTreeSet::new();
    let original_ids: BTreeSet<EntityId> = original.entities.keys().copied().collect();
    for info in &run.variants {
        if info.sheet_id == run.original_sheet_id {
            return Err(format!("案に元シートが混ざっています: {}", info.sheet_id));
        }
        if !seen.insert(info.sheet_id) {
            return Err(format!("同じ案シートが2回あります: {}", info.sheet_id));
        }
        let sheet = project.sheet(info.sheet_id).ok_or_else(|| {
            format!(
                "案シートが見つかりません: {} (比較はもう終わっているか、取り消されています)",
                info.sheet_id
            )
        })?;
        if sheet.variant_of != Some(run.original_sheet_id) {
            return Err(format!(
                "この元シートの案ではありません: {} (別の比較のシートです)",
                info.sheet_id
            ));
        }
        // 対応表は「元id→複製id」。値が元シートのidと重なっていたら向きが壊れている
        let mut mapped = BTreeSet::new();
        for (orig_id, copy_id) in &info.id_map {
            if original_ids.contains(copy_id) {
                return Err(format!(
                    "対応表の写し先が元シートのidです: {orig_id} → {copy_id}"
                ));
            }
            if !mapped.insert(*copy_id) {
                return Err(format!("対応表の写し先が重複しています: {copy_id}"));
            }
        }
    }
    let Some(chosen_id) = chosen else {
        // 破棄はシートを消すだけなので、元シートの手編集は関係ない
        return Ok(());
    };
    if !run.variants.iter().any(|v| v.sheet_id == chosen_id) {
        return Err(format!("採用する案が実行に含まれていません: {chosen_id}"));
    }
    if !run.original_digest.is_empty() && sheet_digest(original) != run.original_digest {
        return Err(
            "比較を始めたあとで元シートが編集されています。採用するとその手編集が案の状態へ\
戻ってしまうので断りました (「すべて破棄」で案を捨ててから、もう一度整えてください)"
                .to_string(),
        );
    }
    Ok(())
}

/// エンティティの中身が同じか (idを除く)。
fn same_content(a: &Entity, b: &Entity) -> bool {
    let mut b = b.clone();
    b.set_id(a.id());
    serde_json::to_value(a).ok() == serde_json::to_value(&b).ok()
}

/// 採用: 選んだ複製の内容を元シートへ写し戻し、全複製を消すコマンド列。
///
/// - 複製側で増えた要素 (対応表に無いid): 新しいidで`AddEntity`
/// - 対応表にある要素: 複製側で変わっていれば**元idのまま**`UpdateEntity`、複製側で
///   消えていれば`DeleteEntities`、変わっていなければ何もしない
/// - 最後に全複製の`RemoveSheet`
///
/// 引き出し線 (G20) のようなシート内参照は**複製id→元シート側のid**へ写し戻す
/// ([`Entity::remap_refs`])。写さないと元シートに無いidを載せた`UpdateEntity`/`AddEntity`に
/// なり、`validate_entity`が`unknown leader_to target`で断って採用がまるごと失敗する。
/// 採用後の元シートに残らない対象への参照は落とす (宙ぶらりんな引き出し線を作らない)。
/// 増えた要素は、同じ「増えた要素」を指す参照が解決できるよう**対象を先に**入れる。
pub fn adopt_commands(
    original: &Sheet,
    chosen: &Sheet,
    chosen_map: &BTreeMap<EntityId, EntityId>,
    variant_sheet_ids: &[SheetId],
) -> Vec<Command> {
    // 複製id → 元シート側のid。対応表の逆写像に、案で増えた要素の新しいidを足す
    let mut back: BTreeMap<EntityId, EntityId> = chosen_map
        .iter()
        .filter(|(orig_id, _)| original.entities.contains_key(orig_id))
        .map(|(orig_id, copy_id)| (*copy_id, *orig_id))
        .collect();
    let mut added: Vec<(EntityId, &Entity)> = Vec::new();
    for (copy_id, entity) in &chosen.entities {
        if back.contains_key(copy_id) {
            continue;
        }
        let new_id = Uuid::new_v4();
        back.insert(*copy_id, new_id);
        added.push((new_id, entity));
    }
    // 採用後の元シートに残るid = 引き出し線の対象に指せるものだけ
    let mut surviving: BTreeSet<EntityId> = original.entities.keys().copied().collect();
    for (orig_id, copy_id) in chosen_map {
        if !chosen.entities.contains_key(copy_id) {
            surviving.remove(orig_id);
        }
    }
    surviving.extend(added.iter().map(|(id, _)| *id));
    let refs: BTreeMap<EntityId, EntityId> = back
        .iter()
        .filter(|(_, orig_id)| surviving.contains(orig_id))
        .map(|(copy_id, orig_id)| (*copy_id, *orig_id))
        .collect();
    let to_original = |entity: &Entity, id: EntityId| -> Entity {
        let mut e = entity.clone();
        e.set_id(id);
        e.remap_refs(&refs);
        e
    };

    let added_ids: BTreeSet<EntityId> = added.iter().map(|(id, _)| *id).collect();
    let new_entities: Vec<Entity> = added
        .iter()
        .map(|(new_id, entity)| to_original(entity, *new_id))
        .collect();
    let mut commands: Vec<Command> = order_by_refs(new_entities, &added_ids)
        .into_iter()
        .map(|entity| Command::AddEntity {
            sheet_id: original.id,
            entity,
        })
        .collect();

    let mut deleted = Vec::new();
    for (orig_id, copy_id) in chosen_map {
        let Some(orig) = original.entities.get(orig_id) else {
            continue;
        };
        match chosen.entities.get(copy_id) {
            Some(copy) => {
                let entity = to_original(copy, *orig_id);
                if !same_content(orig, &entity) {
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
    commands.extend(discard_commands(variant_sheet_ids));
    commands
}

/// 増えた要素を「参照される側が先」の順に並べる。`AddEntity`の検証は引き出し線の対象が
/// **その時点でシート上に実在すること**を求めるため、順番が要る。参照が輪になっていたら
/// (通常あり得ない) 残りは参照を落として入れる。
fn order_by_refs(entities: Vec<Entity>, pending_ids: &BTreeSet<EntityId>) -> Vec<Entity> {
    let mut out = Vec::with_capacity(entities.len());
    let mut placed: BTreeSet<EntityId> = BTreeSet::new();
    let mut rest = entities;
    while !rest.is_empty() {
        let before = rest.len();
        let mut next = Vec::new();
        for entity in rest {
            let ready = entity
                .refs()
                .iter()
                .all(|target| !pending_ids.contains(target) || placed.contains(target));
            if ready {
                placed.insert(entity.id());
                out.push(entity);
            } else {
                next.push(entity);
            }
        }
        rest = next;
        if rest.len() == before {
            for mut entity in rest {
                entity.remap_refs(&BTreeMap::new());
                out.push(entity);
            }
            break;
        }
    }
    out
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

    /// 参考図面が実際に使う取り合わせのフィクスチャ: シンボル・配線に加えて、
    /// **引き出し線つきの注記** (シート内参照を持つ唯一のエンティティ)・図形・電線品番凡例・
    /// 多段端子台を1枚に並べる。
    fn sheet_with_entities() -> Sheet {
        let mut sheet = Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape);
        sheet.title_block.title = "制御盤".into();
        sheet.revisions = vec![Revision {
            mark: "A".into(),
            date: "2026-09-27".into(),
            description: "初版".into(),
            by: "設計".into(),
        }];
        sheet.annotation_text_mm = Some(1.4);
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
        let symbol_id = symbol.id();
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
        // 引き出し線つき注記 (G20): 対象はシンボル。複製で写し替えが要る唯一のフィールド
        let note = Entity::Text(TextEntity {
            id: Uuid::new_v4(),
            at: Point::new(140.0, 40.0),
            text: "絶縁距離を確保".into(),
            height: 2.0,
            rotation: 0,
            anchor: Default::default(),
            color: None,
            bold: false,
            leader_to: Some(symbol_id),
        });
        let shape = Entity::Shape(Shape {
            id: Uuid::new_v4(),
            shape: ShapeKind::Rect,
            points: vec![Point::new(5.0, 5.0), Point::new(60.0, 30.0)],
            stroke: "default".into(),
            stroke_width_mm: 0.25,
            fill: None,
            dashed: true,
        });
        let legend = Entity::WireLegend(WireLegend {
            id: Uuid::new_v4(),
            at: Point::new(300.0, 200.0),
        });
        // 多段端子台 (G6): 極数と段数が symbol_id に入る
        let terminal = Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "terminal_block_4p_2l".into(),
            at: Point::new(200.0, 100.0),
            rotation: 0,
            mirror: false,
            reference: "TB1".into(),
            value: "SL4".into(),
            attrs: Default::default(),
        });
        for e in [symbol, wire, note, shape, legend, terminal] {
            sheet.entities.insert(e.id(), e);
        }
        sheet
    }

    fn symbol_id_of(sheet: &Sheet) -> EntityId {
        sheet
            .entities
            .values()
            .find_map(|e| match e {
                Entity::Symbol(s) if s.reference == "R1" => Some(s.id),
                _ => None,
            })
            .unwrap()
    }

    fn note_id_of(sheet: &Sheet) -> EntityId {
        sheet
            .entities
            .values()
            .find_map(|e| match e {
                Entity::Text(t) => Some(t.id),
                _ => None,
            })
            .unwrap()
    }

    fn leader_of(sheet: &Sheet, id: EntityId) -> Option<EntityId> {
        match &sheet.entities[&id] {
            Entity::Text(t) => t.leader_to,
            other => panic!("注記ではない: {other:?}"),
        }
    }

    fn wire_id_of(sheet: &Sheet) -> EntityId {
        sheet
            .entities
            .values()
            .find_map(|e| match e {
                Entity::Wire(w) => Some(w.id),
                _ => None,
            })
            .unwrap()
    }

    fn project_with(original: Sheet) -> Project {
        let mut project = Project::new("t");
        project.sheets[0] = original;
        project
    }

    /// A variant copy keeps the paper, title block, revision rows, sheet kind and note height of the original and every entity, with a new sheet id, new entity ids, the label in its name, a mark saying which sheet it copies, and a complete old-to-new id map.
    /// 案の複製は用紙・表題欄・改訂欄・シート種別・注記文字高と全エンティティを元のまま持ち、シートidとエンティティidだけ新しく、名前にラベルが付き、どのシートの複製かの印が付き、元id→新idの対応表が全要素分できる。
    #[test]
    fn duplicate_keeps_content_with_fresh_ids_and_a_full_map() {
        let original = sheet_with_entities();
        let (copy, id_map) = duplicate_sheet(&original, "案A");
        assert_ne!(copy.id, original.id);
        assert_eq!(copy.name, "Sheet1 · 案A");
        assert_eq!(copy.size, original.size);
        assert_eq!(copy.orientation, original.orientation);
        assert_eq!(copy.title_block.title, "制御盤");
        // 計画が謳う「改訂欄」と、シート単位の図面規格まで引き継ぐ
        assert_eq!(copy.revisions.len(), 1);
        assert_eq!(copy.revisions[0].mark, "A");
        assert_eq!(copy.kind, original.kind);
        assert_eq!(copy.annotation_text_mm, Some(1.4));
        assert_eq!(copy.zone_cols, original.zone_cols);
        assert_eq!(copy.zone_rows, original.zone_rows);
        assert_eq!(copy.variant_of, Some(original.id), "元シートの印が付く");
        assert!(copy.is_variant());
        assert!(!original.is_variant());
        assert_eq!(copy.entities.len(), original.entities.len());
        assert_eq!(id_map.len(), original.entities.len());
        for (old_id, new_id) in &id_map {
            assert_ne!(old_id, new_id);
            assert!(original.entities.contains_key(old_id));
            let copied = copy.entities.get(new_id).expect("copied entity");
            assert_eq!(copied.id(), *new_id);
            if copied.refs().is_empty() {
                assert!(same_content(&original.entities[old_id], copied));
            }
        }
    }

    /// A leader note in the copy points at the copy of its target, so the copy is a valid sheet on its own: the target exists, nothing points outside the sheet, and the project's own verification finds no missing leader target.
    /// 複製の引き出し線つき注記は「複製側の対象」を指すので、複製だけで成り立つ正しいシートになる。対象は複製の中に実在し、シートの外を指す参照は残らず、図面検証も引き出し線の対象欠落を出さない。
    #[test]
    fn duplicate_repoints_leader_lines_inside_the_copy() {
        let original = sheet_with_entities();
        let orig_symbol = symbol_id_of(&original);
        let orig_note = note_id_of(&original);
        assert_eq!(leader_of(&original, orig_note), Some(orig_symbol));

        let (copy, id_map) = duplicate_sheet(&original, "案A");
        let copy_note = id_map[&orig_note];
        assert_eq!(
            leader_of(&copy, copy_note),
            Some(id_map[&orig_symbol]),
            "引き出し線の対象が複製側のidへ写る"
        );
        for entity in copy.entities.values() {
            for target in entity.refs() {
                assert!(
                    copy.entities.contains_key(&target),
                    "複製の中に対象が実在する: {target}"
                );
                assert!(
                    !original.entities.contains_key(&target),
                    "元シートのidが残っていない: {target}"
                );
            }
        }
        let defs = crate::symbol::sheet_symbol_defs(&copy);
        let diags = crate::verify::verify_sheet(&copy, &defs);
        assert!(
            !diags.iter().any(|d| d.code == "erc.leader_target_missing"),
            "複製に引き出し線の対象欠落が出ない: {diags:?}"
        );
    }

    /// While variant copies exist, the cross-reference addresses (`/2.B3`) of neither the original nor the copies change, because the copies do not count as another sheet carrying the same reference.
    /// 案の複製がある間も、元シートと各案のシート間クロスリファレンス住所 (`/2.B3`) は増えない。複製は「同じ参照記号を持つ別シート」として数えないため。
    #[test]
    fn variant_copies_do_not_grow_cross_reference_addresses() {
        let original = sheet_with_entities();
        let mut project = project_with(original.clone());
        assert!(crate::xref::sheet_device_xrefs(&project, original.id).is_empty());

        let (commands, run) = start_commands(&original, 2, 1).unwrap();
        let mut engine = Engine::new(project);
        engine.execute_batch(commands, EditOrigin::User).unwrap();
        project = engine.project().clone();
        assert_eq!(project.sheets.len(), 3);
        assert!(
            crate::xref::sheet_device_xrefs(&project, original.id).is_empty(),
            "元シートに幽霊の住所注記が生えない"
        );
        for variant in &run.variants {
            assert!(
                crate::xref::sheet_device_xrefs(&project, variant.sheet_id).is_empty(),
                "案シートにも生えない"
            );
        }
        assert!(crate::xref::device_xref_addresses(&project, original.id, "R1").is_empty());
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
        let orig_wire = wire_id_of(&original);
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

    /// Adopting an unchanged variant only removes the variant sheets: the leader note counts as unchanged although its target id differs between the copy and the original.
    /// 何も変わっていない案を採用すると、案シートを消すだけになる。引き出し線つき注記は複製側と元で対象idが違っても「変わっていない」と見なす。
    #[test]
    fn adopting_an_unchanged_variant_only_removes_sheets() {
        let original = sheet_with_entities();
        let (chosen, id_map) = duplicate_sheet(&original, "案A");
        let commands = adopt_commands(&original, &chosen, &id_map, &[chosen.id]);
        assert_eq!(commands.len(), 1, "引き出し線ぶんの空更新が出ない: {commands:?}");
        assert!(matches!(commands[0], Command::RemoveSheet { .. }));
        assert_eq!(discard_commands(&[chosen.id, original.id]).len(), 2);
    }

    /// Adopting a variant that repointed a leader to another entity, and one that added a new note with a leader, writes both back with original-side targets and succeeds through the engine; the target of an entity the variant deleted is dropped instead of left dangling.
    /// 案が引き出し線を張り替えた場合と、引き出し線つき注記を増やした場合の採用は、対象を元シート側のidへ写してエンジンを通る。案が消した要素を指していた引き出し線は宙ぶらりんにせず外す。
    #[test]
    fn adopt_repoints_leaders_to_original_side_ids() {
        let original = sheet_with_entities();
        let original_id = original.id;
        let orig_symbol = symbol_id_of(&original);
        let orig_note = note_id_of(&original);
        let orig_wire = wire_id_of(&original);
        let mut engine = Engine::new(project_with(original));

        let (commands, run) =
            start_commands(engine.project().sheet(original_id).unwrap(), 1, 1).unwrap();
        engine.execute_batch(commands, EditOrigin::User).unwrap();
        let variant = &run.variants[0];

        // 案: 既存の注記の引き出しを配線へ張り替え、引き出しつきの注記を1つ増やす
        let mut note = engine.project().sheet(variant.sheet_id).unwrap().entities
            [&variant.id_map[&orig_note]]
            .clone();
        if let Entity::Text(t) = &mut note {
            t.leader_to = Some(variant.id_map[&orig_wire]);
        }
        let extra = Entity::Text(TextEntity {
            id: Uuid::new_v4(),
            at: Point::new(150.0, 120.0),
            text: "端子台は工具レス".into(),
            height: 2.0,
            rotation: 0,
            anchor: Default::default(),
            color: None,
            bold: false,
            leader_to: Some(variant.id_map[&orig_symbol]),
        });
        engine
            .execute_batch(
                vec![
                    Command::UpdateEntity {
                        sheet_id: variant.sheet_id,
                        entity: note,
                    },
                    Command::AddEntity {
                        sheet_id: variant.sheet_id,
                        entity: extra,
                    },
                ],
                EditOrigin::Agent,
            )
            .expect("案シートの編集は通る (複製の対象が実在するため)");

        let commands = {
            let project = engine.project();
            adopt_commands(
                project.sheet(original_id).unwrap(),
                project.sheet(variant.sheet_id).unwrap(),
                &variant.id_map,
                &[variant.sheet_id],
            )
        };
        engine
            .execute_batch(commands, EditOrigin::User)
            .expect("採用が検証に弾かれない");

        let adopted = engine.project().sheet(original_id).unwrap();
        assert_eq!(adopted.entities.len(), 7, "注記が1つ増える");
        assert_eq!(
            leader_of(adopted, orig_note),
            Some(orig_wire),
            "張り替えた対象が元シート側のidになる"
        );
        let added_note = adopted
            .entities
            .values()
            .find(|e| matches!(e, Entity::Text(t) if t.text == "端子台は工具レス"))
            .expect("増えた注記");
        assert_eq!(added_note.refs(), vec![orig_symbol], "増えた注記の対象も元id");
        let defs = crate::symbol::sheet_symbol_defs(adopted);
        assert!(
            !crate::verify::verify_sheet(adopted, &defs)
                .iter()
                .any(|d| d.code == "erc.leader_target_missing"),
            "採用後の元シートに宙ぶらりんな引き出し線が無い"
        );
    }

    /// When the variant deletes the entity a leader pointed at, adopting drops that leader instead of writing back a target that will not exist.
    /// 案が引き出し線の対象を消した場合、採用は「残らない対象」を書き戻さずに引き出し線を外す。
    #[test]
    fn adopt_drops_a_leader_whose_target_the_variant_deleted() {
        let original = sheet_with_entities();
        let orig_symbol = symbol_id_of(&original);
        let orig_note = note_id_of(&original);
        let (mut chosen, id_map) = duplicate_sheet(&original, "案A");
        // 案: 引き出しの対象 (シンボル) だけを消す。注記は対象を指したまま残る
        chosen.entities.remove(&id_map[&orig_symbol]);

        let commands = adopt_commands(&original, &chosen, &id_map, &[chosen.id]);
        let note = commands
            .iter()
            .find_map(|c| match c {
                Command::UpdateEntity { entity, .. } if entity.id() == orig_note => Some(entity),
                _ => None,
            })
            .expect("注記の更新");
        assert_eq!(note.refs(), Vec::<EntityId>::new(), "引き出し線が外れる");

        // 元シートだけを持つエンジンへ通す (案シートは消す対象に入れない)
        let mut engine = Engine::new(project_with(original.clone()));
        let commands = adopt_commands(&original, &chosen, &id_map, &[]);
        engine
            .execute_batch(commands, EditOrigin::User)
            .expect("採用が検証に弾かれない");
        let adopted = engine.project().sheet(original.id).unwrap();
        assert_eq!(leader_of(adopted, orig_note), None);
        assert!(!adopted.entities.contains_key(&orig_symbol), "対象は消える");
    }

    /// finish rejects a run whose original or variant sheets are gone, whose variant sheets are not copies of that original, whose id map points back at the original's own ids, whose adopt target is not one of the copies, or whose original sheet was hand-edited since the comparison started; discarding after a hand edit is still allowed.
    /// 終了は、元シートや案シートが無い実行・その元シートの案ではないシート・写し先が元シートのidになっている対応表・案に含まれない採用先・比較開始後に元シートが手編集された採用を、理由つきで断る。手編集後でも破棄はできる。
    #[test]
    fn finish_validates_the_run_against_the_current_drawing() {
        let original = sheet_with_entities();
        let original_id = original.id;
        let orig_symbol = symbol_id_of(&original);
        let mut engine = Engine::new(project_with(original));
        let (commands, run) =
            start_commands(engine.project().sheet(original_id).unwrap(), 2, 1).unwrap();
        engine.execute_batch(commands, EditOrigin::User).unwrap();
        let chosen = run.variants[0].sheet_id;
        assert!(validate_run(engine.project(), &run, Some(chosen)).is_ok());
        assert!(validate_run(engine.project(), &run, None).is_ok());

        // 元シートが無い
        let mut gone = run.clone();
        gone.original_sheet_id = Uuid::new_v4();
        assert!(validate_run(engine.project(), &gone, None)
            .unwrap_err()
            .contains("元シートが見つかりません"));
        // 案シートが無い (開始をundoした後など)
        let mut stale = run.clone();
        stale.variants[1].sheet_id = Uuid::new_v4();
        assert!(validate_run(engine.project(), &stale, None)
            .unwrap_err()
            .contains("案シートが見つかりません"));
        // 案に元シートを混ぜる
        let mut mixed = run.clone();
        mixed.variants[1].sheet_id = original_id;
        assert!(validate_run(engine.project(), &mixed, None).is_err());
        // 対応表の写し先が元シートのid
        let mut crossed = run.clone();
        crossed.variants[0]
            .id_map
            .insert(orig_symbol, orig_symbol);
        assert!(validate_run(engine.project(), &crossed, Some(chosen))
            .unwrap_err()
            .contains("対応表の写し先が元シートのidです"));
        // 採用先が案に含まれない
        assert!(
            validate_run(engine.project(), &run, Some(Uuid::new_v4()))
                .unwrap_err()
                .contains("採用する案が実行に含まれていません")
        );

        // 比較中に元シートを手編集 → 採用は断り、破棄は通る
        let mut moved = engine.project().sheet(original_id).unwrap().entities[&orig_symbol].clone();
        if let Entity::Symbol(s) = &mut moved {
            s.at = Point::new(300.0, 200.0);
        }
        engine
            .execute_as(
                Command::UpdateEntity {
                    sheet_id: original_id,
                    entity: moved,
                },
                EditOrigin::User,
            )
            .unwrap();
        let err = validate_run(engine.project(), &run, Some(chosen)).unwrap_err();
        assert!(err.contains("元シートが編集されています"), "{err}");
        assert!(err.contains("すべて破棄"), "破棄の逃げ道を示す: {err}");
        assert!(validate_run(engine.project(), &run, None).is_ok(), "破棄は通る");
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
