//! 線番(ワイヤ番号)の自動採番。
//!
//! 線番は**ネット単位**の属性で、同一ネットの全Wireの [`crate::model::Wire::net`] へ
//! 同じ文字列を書き込む。採番順は「上→下、同じ高さなら左→右」で決定的。

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::model::{Entity, EntityId, Sheet};
use crate::netlist::extract_netlist;
use crate::symbol::sheet_symbol_defs;

/// 自動採番の方式 (IEC 62491 / ACADE慣行: 既定は連番)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RenumberMode {
    /// 追い番: 既存の線番・ネットラベルを保持し、未採番のネットにだけ新しい番号を振る。
    Append,
    /// 振り直し: 数字の線番を全て捨てて振り直す。数字でない手動の名前は保持する。
    Renumber,
}

/// 開始番号の既定値 (JSONで省略されたとき)。
pub fn default_start() -> u32 {
    1
}

/// 自動採番が生成する形 (数字だけ) の線番か。手動で付けた名前と区別するために使う。
pub fn is_generated_number(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// 採番対象のネット1件。
pub struct NumberingNet {
    /// 線番を書き込むWireのid。空 = 書き込み対象外 (ネットラベル付きネット)。
    pub wire_ids: Vec<EntityId>,
    /// 保持する既存の名前・線番。Noneなら新規に番号を振る対象。
    pub keep: Option<String>,
}

/// ネットの代表位置 (最も上、同じ高さなら最も左の点)。採番順の基準。
fn net_anchor(sheet: &Sheet, wire_ids: &[EntityId]) -> (f64, f64) {
    let mut best = (f64::MAX, f64::MAX);
    for id in wire_ids {
        if let Some(Entity::Wire(w)) = sheet.entities.get(id) {
            for p in &w.points {
                if (p.y, p.x) < best {
                    best = (p.y, p.x);
                }
            }
        }
    }
    best
}

/// シートのネットを採番順 (上→下、同じ高さなら左→右) に並べ、
/// 各ネットが既存の名前・線番を保持するかどうかを判定する。
pub fn numbering_nets(sheet: &Sheet, mode: RenumberMode) -> Vec<NumberingNet> {
    let nets = extract_netlist(sheet, &sheet_symbol_defs(sheet));
    let mut ordered: Vec<(f64, f64, EntityId, NumberingNet)> = nets
        .into_iter()
        .map(|net| {
            let (y, x) = net_anchor(sheet, &net.wire_ids);
            let tie = net.wire_ids.iter().min().copied().unwrap_or_default();
            let item = if net.label.is_some() {
                // ネットラベル付きは名前が確定しているので触らない (番号は予約だけする)
                NumberingNet {
                    wire_ids: Vec::new(),
                    keep: net.label.clone(),
                }
            } else {
                let keep = match (&net.wire_no, mode) {
                    (Some(no), RenumberMode::Append) => Some(no.clone()),
                    (Some(no), RenumberMode::Renumber) if !is_generated_number(no) => {
                        Some(no.clone())
                    }
                    _ => None,
                };
                NumberingNet {
                    wire_ids: net.wire_ids.clone(),
                    keep,
                }
            };
            (y, x, tie, item)
        })
        .collect();
    ordered.sort_by(|a, b| {
        (a.0, a.1)
            .partial_cmp(&(b.0, b.1))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.2.cmp(&b.2))
    });
    ordered.into_iter().map(|(_, _, _, n)| n).collect()
}

/// 連番の発行器。既に使われている名前・線番を避けて番号を配る。
pub struct Numberer {
    next: u32,
    used: BTreeSet<String>,
}

impl Numberer {
    pub fn new(start: u32) -> Self {
        Self {
            next: start,
            used: BTreeSet::new(),
        }
    }

    /// 保持する名前・線番を予約し、以後その番号を配らないようにする。
    pub fn reserve(&mut self, name: &str) {
        self.used.insert(name.to_string());
    }

    /// 次の未使用番号を発行する。
    pub fn issue(&mut self) -> String {
        loop {
            let candidate = self.next.to_string();
            self.next = self.next.saturating_add(1);
            if !self.used.contains(&candidate) {
                self.used.insert(candidate.clone());
                return candidate;
            }
        }
    }
}

/// 1シート分の採番結果 (wire id, 新しい線番) を返す。
/// 保持するネットも同値へ揃えるため、ネット内の全Wireが結果に含まれる。
pub fn plan_sheet(sheet: &Sheet, mode: RenumberMode, numberer: &mut Numberer) -> Vec<(EntityId, String)> {
    let nets = numbering_nets(sheet, mode);
    let mut out = Vec::new();
    for net in &nets {
        if let Some(keep) = &net.keep {
            for id in &net.wire_ids {
                out.push((*id, keep.clone()));
            }
        }
    }
    for net in &nets {
        if net.keep.is_none() && !net.wire_ids.is_empty() {
            let number = numberer.issue();
            for id in &net.wire_ids {
                out.push((*id, number.clone()));
            }
        }
    }
    out
}

/// 採番前に、対象シートで既に使われている名前・線番を予約する。
pub fn reserve_sheet(sheet: &Sheet, mode: RenumberMode, numberer: &mut Numberer) {
    for net in numbering_nets(sheet, mode) {
        if let Some(keep) = net.keep {
            numberer.reserve(&keep);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::command::{Command, Engine};
    use crate::geometry::Point;
    use crate::model::*;
    use crate::wire_no::RenumberMode;
    use uuid::Uuid;

    fn wire(points: &[(f64, f64)], net: Option<&str>) -> Entity {
        Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            color: "black".into(),
            sq: 0.3,
            length_m: None,
            part_no: None,
            net: net.map(|s| s.to_string()),
        })
    }

    /// 横一直線のワイヤ (id順に返す) をシートへ入れ、engineとwire idを返す。
    fn engine_with(wires: Vec<Entity>) -> (Engine, SheetId, Vec<EntityId>) {
        let mut project = Project::new("t");
        let sheet_id = project.sheets[0].id;
        let ids: Vec<EntityId> = wires.iter().map(|e| e.id()).collect();
        let sheet = project.sheet_mut(sheet_id).unwrap();
        for e in wires {
            sheet.entities.insert(e.id(), e);
        }
        (Engine::new(project), sheet_id, ids)
    }

    /// 3本の独立した横線 (y=10 / 20 / 30)。真ん中だけ既存線番を持たせられる。
    fn three_rows(middle_net: Option<&str>) -> (Engine, SheetId, Vec<EntityId>) {
        engine_with(vec![
            wire(&[(0.0, 10.0), (20.0, 10.0)], None),
            wire(&[(0.0, 20.0), (20.0, 20.0)], middle_net),
            wire(&[(0.0, 30.0), (20.0, 30.0)], None),
        ])
    }

    fn net_of(engine: &Engine, sheet_id: SheetId, id: EntityId) -> Option<String> {
        let Entity::Wire(w) = &engine.project().sheet(sheet_id).unwrap().entities[&id] else {
            panic!("not a wire")
        };
        w.net.clone()
    }

    fn renumber(engine: &mut Engine, sheet_id: Option<SheetId>, mode: RenumberMode, start: u32) {
        engine
            .execute(Command::RenumberWires {
                sheet_id,
                mode,
                start,
            })
            .unwrap();
    }

    /// Append mode numbers only the nets that have no number yet, leaving already numbered nets untouched.
    /// 追い番モードは線番の無いネットにだけ番号を振り、すでに線番を持つネットはそのまま残す。
    #[test]
    fn append_numbers_only_unnumbered_nets() {
        let (mut engine, sid, ids) = three_rows(Some("5"));
        renumber(&mut engine, Some(sid), RenumberMode::Append, 1);
        assert_eq!(net_of(&engine, sid, ids[0]).as_deref(), Some("1"));
        assert_eq!(net_of(&engine, sid, ids[1]).as_deref(), Some("5"), "既存線番は保持");
        assert_eq!(net_of(&engine, sid, ids[2]).as_deref(), Some("2"));
    }

    /// New numbers never collide with numbers that are already used on the drawing.
    /// 新しく振る番号は、図面ですでに使われている番号とは決して衝突しない。
    #[test]
    fn append_skips_numbers_already_in_use() {
        let (mut engine, sid, ids) = three_rows(Some("1"));
        renumber(&mut engine, Some(sid), RenumberMode::Append, 1);
        assert_eq!(net_of(&engine, sid, ids[0]).as_deref(), Some("2"));
        assert_eq!(net_of(&engine, sid, ids[1]).as_deref(), Some("1"));
        assert_eq!(net_of(&engine, sid, ids[2]).as_deref(), Some("3"));
    }

    /// Renumber mode throws away every numeric wire number and assigns fresh consecutive numbers from the start value.
    /// 振り直しモードは数字の線番を全て捨て、開始番号から順に新しい連番を振り直す。
    #[test]
    fn renumber_reassigns_every_numeric_wire_number() {
        let (mut engine, sid, ids) = three_rows(Some("5"));
        renumber(&mut engine, Some(sid), RenumberMode::Renumber, 1);
        assert_eq!(net_of(&engine, sid, ids[0]).as_deref(), Some("1"));
        assert_eq!(net_of(&engine, sid, ids[1]).as_deref(), Some("2"));
        assert_eq!(net_of(&engine, sid, ids[2]).as_deref(), Some("3"));
    }

    /// A hand-written name that is not a plain number (for example "24V_1") survives a full renumber.
    /// 数字ではない手書きの名前 (例: "24V_1") は、全振り直しをしても消えずに残る。
    #[test]
    fn renumber_keeps_manual_non_numeric_names() {
        let (mut engine, sid, ids) = three_rows(Some("24V_1"));
        renumber(&mut engine, Some(sid), RenumberMode::Renumber, 1);
        assert_eq!(net_of(&engine, sid, ids[0]).as_deref(), Some("1"));
        assert_eq!(net_of(&engine, sid, ids[1]).as_deref(), Some("24V_1"));
        assert_eq!(net_of(&engine, sid, ids[2]).as_deref(), Some("2"));
    }

    /// A net that carries a net label keeps the label as its name and is never given a wire number.
    /// ネットラベルが付いたネットはラベル名がそのまま名前になり、線番は振られない。
    #[test]
    fn nets_with_a_net_label_are_never_numbered() {
        let label = Entity::NetLabel(NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(0.0, 20.0),
            name: "24-P1".into(),
            rotation: 0,
        });
        let (mut engine, sid, ids) = engine_with(vec![
            wire(&[(0.0, 10.0), (20.0, 10.0)], None),
            wire(&[(0.0, 20.0), (20.0, 20.0)], None),
            label,
        ]);
        renumber(&mut engine, Some(sid), RenumberMode::Renumber, 1);
        assert_eq!(net_of(&engine, sid, ids[0]).as_deref(), Some("1"));
        assert_eq!(net_of(&engine, sid, ids[1]), None, "ラベル付きネットは無採番");
    }

    /// All wires of one net receive the very same wire number, even when they were drawn as separate segments.
    /// 1つのネットに属する全ワイヤには、別々に描かれていても同じ線番が入る。
    #[test]
    fn every_wire_of_a_net_gets_the_same_number() {
        let (mut engine, sid, ids) = engine_with(vec![
            wire(&[(0.0, 10.0), (20.0, 10.0)], None),
            wire(&[(20.0, 10.0), (20.0, 40.0)], None),
        ]);
        renumber(&mut engine, Some(sid), RenumberMode::Append, 1);
        assert_eq!(net_of(&engine, sid, ids[0]).as_deref(), Some("1"));
        assert_eq!(net_of(&engine, sid, ids[1]).as_deref(), Some("1"));
    }

    /// Undo after an automatic renumbering restores every previous wire number, including wires that had none.
    /// 自動採番のundoは、線番が無かったワイヤも含めて全ての線番を元の状態へ戻す。
    #[test]
    fn undo_restores_previous_wire_numbers() {
        let (mut engine, sid, ids) = three_rows(Some("5"));
        renumber(&mut engine, Some(sid), RenumberMode::Renumber, 1);
        engine.undo().unwrap().unwrap();
        assert_eq!(net_of(&engine, sid, ids[0]), None);
        assert_eq!(net_of(&engine, sid, ids[1]).as_deref(), Some("5"));
        assert_eq!(net_of(&engine, sid, ids[2]), None);
    }

    /// Numbering runs top to bottom, and left to right within the same height, so the same drawing always yields the same numbers.
    /// 採番は上から下へ、同じ高さなら左から右へ進むため、同じ図面なら常に同じ線番になる。
    #[test]
    fn numbering_order_is_top_to_bottom_then_left_to_right() {
        let (mut engine, sid, ids) = engine_with(vec![
            wire(&[(0.0, 30.0), (20.0, 30.0)], None),
            wire(&[(50.0, 10.0), (70.0, 10.0)], None),
            wire(&[(0.0, 10.0), (20.0, 10.0)], None),
        ]);
        renumber(&mut engine, Some(sid), RenumberMode::Append, 1);
        assert_eq!(net_of(&engine, sid, ids[2]).as_deref(), Some("1"), "最上段の左");
        assert_eq!(net_of(&engine, sid, ids[1]).as_deref(), Some("2"), "最上段の右");
        assert_eq!(net_of(&engine, sid, ids[0]).as_deref(), Some("3"), "下の段");
    }

    /// Numbering can start from any number, for example 100 for a second panel.
    /// 開始番号は自由に指定でき、例えば2面目の盤を100番から始められる。
    #[test]
    fn numbering_starts_at_the_given_start_number() {
        let (mut engine, sid, ids) = three_rows(None);
        renumber(&mut engine, Some(sid), RenumberMode::Append, 100);
        assert_eq!(net_of(&engine, sid, ids[0]).as_deref(), Some("100"));
        assert_eq!(net_of(&engine, sid, ids[2]).as_deref(), Some("102"));
    }

    /// Without a sheet id every sheet of the project is numbered in sheet order with numbers unique across the whole project.
    /// シートを指定しない採番はプロジェクトの全シートをシート順に処理し、番号は図面全体で重複しない。
    #[test]
    fn numbering_without_a_sheet_covers_the_whole_project() {
        let mut project = Project::new("t");
        let s1 = project.sheets[0].id;
        let w1 = wire(&[(0.0, 10.0), (20.0, 10.0)], None);
        let (id1, id2) = (w1.id(), Uuid::new_v4());
        project.sheet_mut(s1).unwrap().entities.insert(w1.id(), w1);
        let mut sheet2 = Sheet::new("S2", PaperSize::A3, Orientation::Landscape);
        let s2 = sheet2.id;
        let mut w2 = wire(&[(0.0, 10.0), (20.0, 10.0)], None);
        if let Entity::Wire(w) = &mut w2 {
            w.id = id2;
        }
        sheet2.entities.insert(id2, w2);
        project.sheets.push(sheet2);

        let mut engine = Engine::new(project);
        renumber(&mut engine, None, RenumberMode::Append, 1);
        assert_eq!(net_of(&engine, s1, id1).as_deref(), Some("1"));
        assert_eq!(net_of(&engine, s2, id2).as_deref(), Some("2"), "シートを跨いで一意");
    }

    /// A single wire number can be edited directly, and undo puts the previous value back.
    /// 線番は1本ずつ直接編集でき、undoで以前の値に戻る。
    #[test]
    fn set_wire_numbers_edits_one_wire_and_is_undoable() {
        let (mut engine, sid, ids) = three_rows(None);
        engine
            .execute(Command::SetWireNumbers {
                sheet_id: sid,
                numbers: vec![WireNumber {
                    wire_id: ids[0],
                    number: Some("W7".into()),
                }],
            })
            .unwrap();
        assert_eq!(net_of(&engine, sid, ids[0]).as_deref(), Some("W7"));
        engine.undo().unwrap().unwrap();
        assert_eq!(net_of(&engine, sid, ids[0]), None);
    }

    /// The renumber command is plain JSON ({"type":"renumber_wires","mode":"append","start":1}), so AI agents and the CLI can send it.
    /// 採番コマンドは素のJSON ({"type":"renumber_wires","mode":"append","start":1}) で表現でき、AIやCLIから送れる。
    #[test]
    fn renumber_command_is_plain_json() {
        let cmd: Command =
            serde_json::from_str(r#"{"type":"renumber_wires","mode":"append","start":1}"#).unwrap();
        let Command::RenumberWires {
            sheet_id,
            mode,
            start,
        } = cmd
        else {
            panic!("renumber_wiresとして解釈されない")
        };
        assert_eq!(sheet_id, None, "省略時は全シート");
        assert_eq!(mode, RenumberMode::Append);
        assert_eq!(start, 1);
        let back = serde_json::to_string(&Command::RenumberWires {
            sheet_id: None,
            mode: RenumberMode::Renumber,
            start: 10,
        })
        .unwrap();
        assert!(back.contains("\"mode\":\"renumber\""), "{back}");
    }
}
