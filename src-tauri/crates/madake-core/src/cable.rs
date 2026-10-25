//! 多芯ケーブル (M4仕様 §5「ケーブル一覧」、.pen「M4残りデザイン④」)。
//!
//! - **定義線** ([`crate::model::Cable::points`]): まとめる配線を斜めに横切る線。芯線は定義線が
//!   横切った配線に交差順で割り当てる ([`crossed_wires`])
//! - **注記** ([`cable_label`]): `W1  CVV-S 4C×1.25sq  12.5m`
//! - **シールド** ([`shield_ellipse`]): 定義線を囲む楕円
//! - **ケーブル一覧** ([`cable_list_rows`]) と部品表のケーブル行 ([`cable_bom_rows`])
//! - **ERC** ([`check_cables`]): 型番の芯数を超える割当は Error
//!
//! ベンチマーク: EPLANのケーブル定義 (ケーブル=複数の芯線をまとめた1機器、定義線とシールドで図示)。

use std::collections::BTreeMap;

use crate::geometry::Point;
use crate::model::{Cable, CoreEnd, Entity, EntityId, Project, RunEnd, Sheet};
use crate::netlist::pin_positions;
use crate::reports::{csv_row, endpoint_label};
use crate::symbol::{sheet_symbol_defs, SymbolDef};
use crate::verify::{Diagnostic, Severity};

/// ERC: ケーブルに割り当てた芯が芯数を超えている。
pub const CABLE_CORE_OVERFLOW: &str = "erc.cable_core_overflow";

/// ケーブル一覧の列見出し。
pub const CABLE_LIST_COLUMNS: [&str; 11] = [
    "ケーブル",
    "型番",
    "芯数",
    "線径sq",
    "長さm",
    "From機器",
    "To機器",
    "使用芯",
    "予備芯",
    "シールド",
    "シート",
];

/// 部品表でケーブル行の「シンボル」欄に立てる印。
pub const BOM_CABLE_KIND: &str = "ケーブル";

/// 数値の表記 (末尾の0と小数点を書かない)。
fn num(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// 定義線に添える注記 (参照記号・型番・芯数×線径・長さを2つの空白で区切る。空の項は飛ばす)。
pub fn cable_label(c: &Cable) -> String {
    let spec = if c.sq > 0.0 {
        format!("{}C×{}sq", c.core_count, num(c.sq))
    } else {
        format!("{}C", c.core_count)
    };
    [
        c.reference.clone(),
        c.part_no.trim().to_string(),
        spec,
        c.length_m.map(|m| format!("{}m", num(m))).unwrap_or_default(),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join("  ")
}

/// 線分 `a-b` と `c-d` の交点の、`a-b` 上の位置 (0〜1)。交わらなければNone。
fn segment_hit(a: Point, b: Point, c: Point, d: Point) -> Option<f64> {
    let r = (b.x - a.x, b.y - a.y);
    let s = (d.x - c.x, d.y - c.y);
    let denom = r.0 * s.1 - r.1 * s.0;
    if denom.abs() < 1e-12 {
        return None;
    }
    let t = ((c.x - a.x) * s.1 - (c.y - a.y) * s.0) / denom;
    let u = ((c.x - a.x) * r.1 - (c.y - a.y) * r.0) / denom;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some(t)
}

/// 定義線 `a-b` が横切る配線を、`a`に近い順に返す (配線id, 線色)。1本の配線は1回だけ数える。
pub fn crossed_wires(sheet: &Sheet, a: Point, b: Point) -> Vec<(EntityId, String)> {
    let mut hits: Vec<(f64, EntityId, String)> = Vec::new();
    for e in sheet.entities.values() {
        let Entity::Wire(w) = e else { continue };
        let first = w
            .points
            .windows(2)
            .filter_map(|seg| segment_hit(a, b, seg[0], seg[1]))
            .fold(None::<f64>, |m, t| Some(m.map_or(t, |m| m.min(t))));
        if let Some(t) = first {
            hits.push((t, w.id, w.color.clone()));
        }
    }
    hits.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal).then(x.1.cmp(&y.1)));
    hits.into_iter().map(|(_, id, color)| (id, color)).collect()
}

/// シールドの楕円 (中心, 長半径, 短半径, 定義線の向きの角度 [度])。定義線を少し長めに囲む。
pub fn shield_ellipse(c: &Cable) -> Option<(Point, f64, f64, f64)> {
    let (a, b) = (c.points.first()?, c.points.get(1)?);
    let center = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
    let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
    let angle = (b.y - a.y).atan2(b.x - a.x).to_degrees();
    Some((center, len / 2.0 + 2.5, 3.0, angle))
}

/// 割り当てた芯のうち、配線が今もシートにあるもの。
fn live_cores<'a>(sheet: &'a Sheet, c: &'a Cable) -> impl Iterator<Item = &'a crate::model::Wire> + 'a {
    c.cores.iter().filter_map(move |core| match core.wire.and_then(|id| sheet.entities.get(&id)) {
        Some(Entity::Wire(w)) => Some(w),
        _ => None,
    })
}

/// 配線の端にある機器の参照記号 (ピンの参照記号部分。ネットラベルならその名前)。
fn end_devices(sheet: &Sheet, wires: &[&crate::model::Wire], first: bool) -> String {
    let defs = sheet_symbol_defs(sheet);
    let mut out: Vec<String> = Vec::new();
    for w in wires {
        let end = if first { w.points.first() } else { w.points.last() };
        let Some(end) = end else { continue };
        let label = endpoint_label(sheet, &defs, end);
        let device = label.split(':').next().unwrap_or("").trim().to_string();
        if !device.is_empty() && !out.contains(&device) {
            out.push(device);
        }
    }
    out.sort();
    out.join(", ")
}

/// ケーブル一覧の行 ([`CABLE_LIST_COLUMNS`] と同じ並び)。全シートのケーブルを参照記号順に。
pub fn cable_list_rows(project: &Project) -> Vec<Vec<String>> {
    let mut rows: Vec<(String, Vec<String>)> = Vec::new();
    for sheet in &project.sheets {
        for e in sheet.entities.values() {
            let Entity::Cable(c) = e else { continue };
            let wires: Vec<&crate::model::Wire> = live_cores(sheet, c).collect();
            let used = wires.len();
            rows.push((
                c.reference.clone(),
                vec![
                    c.reference.clone(),
                    c.part_no.clone(),
                    c.core_count.to_string(),
                    if c.sq > 0.0 { num(c.sq) } else { String::new() },
                    c.length_m.map(num).unwrap_or_default(),
                    end_devices(sheet, &wires, true),
                    end_devices(sheet, &wires, false),
                    used.to_string(),
                    c.core_count.saturating_sub(used).to_string(),
                    match &c.shield {
                        Some(s) if s.grounded => "あり (接地)".to_string(),
                        Some(_) => "あり".to_string(),
                        None => String::new(),
                    },
                    sheet.name.clone(),
                ],
            ));
        }
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    rows.into_iter().map(|(_, r)| r).collect()
}

/// ケーブル一覧CSV。見出しは [`CABLE_LIST_COLUMNS`]。
pub fn cable_list_csv(project: &Project) -> String {
    let mut out = CABLE_LIST_COLUMNS.join(",");
    out.push('\n');
    for row in cable_list_rows(project) {
        out.push_str(&csv_row(&row));
        out.push('\n');
    }
    out
}

/// 部品表のケーブル行 (型番ごと。部品表の先頭5列の形)。数量は長さが全部分かっていれば
/// 合計の長さ (`12.50 m`)、1本でも長さが無ければ本数。型番の無いケーブルは載せない。
pub fn cable_bom_rows(project: &Project) -> Vec<Vec<String>> {
    let mut groups: BTreeMap<String, (Vec<String>, Vec<Option<f64>>)> = BTreeMap::new();
    for sheet in &project.sheets {
        for e in sheet.entities.values() {
            let Entity::Cable(c) = e else { continue };
            let part = c.part_no.trim();
            if part.is_empty() {
                continue;
            }
            let g = groups.entry(part.to_string()).or_default();
            g.0.push(c.reference.clone());
            g.1.push(c.length_m);
        }
    }
    groups
        .into_iter()
        .map(|(part, (mut refs, lengths))| {
            refs.sort();
            let qty = if lengths.iter().all(Option::is_some) {
                format!("{:.2} m", lengths.iter().flatten().sum::<f64>())
            } else {
                lengths.len().to_string()
            };
            vec![String::new(), refs.join(", "), part, BOM_CABLE_KIND.to_string(), qty]
        })
        .collect()
}

/// シート上のケーブルのERC: 割り当てた芯が芯数を超えていれば Error。
pub fn check_cables(sheet: &Sheet) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for e in sheet.entities.values() {
        let Entity::Cable(c) = e else { continue };
        let used = live_cores(sheet, c).count();
        if used > c.core_count {
            out.push(Diagnostic {
                severity: Severity::Error,
                code: CABLE_CORE_OVERFLOW.into(),
                message: format!(
                    "ケーブル {} に {} 本の配線が割り当てられていますが、芯数は {} です",
                    c.reference, used, c.core_count
                ),
                sheet_id: sheet.id,
                entity_ids: vec![c.id],
            });
        }
    }
    out
}

/// `CoreEnd::Pin`が指すピンの用紙上の位置 (`index`番目、省略 = 最初の接続点)。
/// 部品・ピンが無いとき、ピンでない行き先のときはNone。
pub fn pin_end_position(sheet: &Sheet, defs: &[SymbolDef], end: &CoreEnd) -> Option<Point> {
    let CoreEnd::Pin { part, pin, index } = end else {
        return None;
    };
    let Some(Entity::Symbol(s)) = sheet.entities.get(part) else {
        return None;
    };
    let def = crate::symbol::instance_symbol_def(s, defs)?;
    pin_positions(s, &def)
        .into_iter()
        .filter(|(no, _)| no == pin)
        .nth(index.unwrap_or(1).saturating_sub(1) as usize)
        .map(|(_, p)| p)
}

/// 区間ケーブルの形を調べる ([`crate::command::Engine`]が置く・更新するときに通す)。
///
/// 区間が2点以上・どの芯も両端を持つ・芯の`wire` (定義線ケーブルの欄) は使わない・行き先の
/// 部品にそのピンがあり配線がこのシートにある・1本の配線/1つのピンを2本の芯で使わない。
pub fn validate_run_cable(sheet: &Sheet, c: &Cable) -> Result<(), String> {
    if c.run.len() < 2 {
        return Err(format!("cable {} needs a run of at least two points", c.reference));
    }
    let defs = sheet_symbol_defs(sheet);
    let mut used_wires: Vec<EntityId> = Vec::new();
    let mut used_pins: Vec<(EntityId, String, u32)> = Vec::new();
    for core in &c.cores {
        if core.wire.is_some() {
            return Err(format!(
                "cable {} core {}: a run cable assigns cores with end_a/end_b, not wire",
                c.reference, core.core
            ));
        }
        for end in RunEnd::BOTH {
            let Some(e) = core.end(end) else {
                return Err(format!("cable {} core {} has no end_{}", c.reference, core.core, end.letter()));
            };
            match e {
                CoreEnd::Wire { wire } => {
                    if !matches!(sheet.entities.get(wire), Some(Entity::Wire(_))) {
                        return Err(format!(
                            "cable {} core {} points at a wire that is not on this sheet: {wire}",
                            c.reference, core.core
                        ));
                    }
                    if used_wires.contains(wire) {
                        return Err(format!("cable {}: wire {wire} is used by two cores", c.reference));
                    }
                    used_wires.push(*wire);
                }
                CoreEnd::Pin { part, pin, index } => {
                    if pin_end_position(sheet, &defs, e).is_none() {
                        return Err(format!(
                            "cable {} core {} points at pin {pin} of {part}, which is not on this sheet",
                            c.reference, core.core
                        ));
                    }
                    let key = (*part, pin.clone(), index.unwrap_or(1));
                    if used_pins.contains(&key) {
                        return Err(format!("cable {}: pin {pin} of {part} is used by two cores", c.reference));
                    }
                    used_pins.push(key);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod test_support {
    //! 区間ケーブルのテスト用の図面 (model・io・command・netlist・verify・reports・svg・cable_run のテストが共有する)。
    use crate::command::Engine;
    use crate::geometry::Point;
    use crate::model::*;
    use uuid::Uuid;

    /// `connector_{n}p` のインスタンス。`mirror`ならピンが左向き (x - 7.5) になる。
    pub(crate) fn connector(n: usize, reference: &str, x: f64, y: f64, mirror: bool) -> SymbolInstance {
        SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: format!("connector_{n}p"),
            at: Point::new(x, y),
            rotation: 0,
            mirror,
            reference: reference.into(),
            value: String::new(),
            attrs: Default::default(),
        }
    }

    /// 黒 0.3sq の配線。
    pub(crate) fn wire(points: &[(f64, f64)]) -> Wire {
        Wire {
            id: Uuid::new_v4(),
            points: points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            color: "black".into(),
            sq: 0.3,
            length_m: None,
            length_source: Default::default(),
            part_no: None,
            net: None,
        }
    }

    /// エンティティをシートへ入れてidを返す。
    pub(crate) fn insert(sheet: &mut Sheet, e: Entity) -> EntityId {
        let id = e.id();
        sheet.entities.insert(id, e);
        id
    }

    /// LTEハーネス45の芯 (呼び名, 線色)。
    pub(crate) const LTE_CORES: [(&str, &str); 4] =
        [("VBUS", "red"), ("D-", "white"), ("D+", "green"), ("GND", "black")];

    /// LTEハーネス45の図面とid。
    pub(crate) struct Lte {
        pub sheet: Sheet,
        pub cable: EntityId,
        pub j45: EntityId,
        pub usb: EntityId,
        pub wires: Vec<EntityId>,
    }

    /// 基板 J45 (connector_4p、(50,100)、ピンは x=57.5 の y=92.5/97.5/102.5/107.5 で右向き) から
    /// 芯ごとの配線4本が x=77.5 まで出て、区間 (80,100)→(192.5,100) が USB1 (connector_4p を
    /// 反転して (200,100)、ピンは x=192.5 で左向き) に着く。芯は VBUS / D- / D+ / GND をピン順に。
    pub(crate) fn lte_sheet() -> Lte {
        let mut sheet = Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape);
        let j45 = insert(&mut sheet, Entity::Symbol(connector(4, "J45", 50.0, 100.0, false)));
        let usb = insert(&mut sheet, Entity::Symbol(connector(4, "USB1", 200.0, 100.0, true)));
        let mut wires = Vec::new();
        let mut cores = Vec::new();
        for (i, (name, color)) in LTE_CORES.into_iter().enumerate() {
            let y = 92.5 + 5.0 * i as f64;
            let mut w = wire(&[(57.5, y), (77.5, y)]);
            w.color = color.into();
            let id = insert(&mut sheet, Entity::Wire(w));
            wires.push(id);
            cores.push(CableCore {
                core: name.into(),
                color: color.into(),
                wire: None,
                end_a: Some(CoreEnd::Wire { wire: id }),
                end_b: Some(CoreEnd::Pin { part: usb, pin: (i + 1).to_string(), index: None }),
            });
        }
        let cable = Cable {
            id: Uuid::new_v4(),
            reference: "W45".into(),
            part_no: String::new(),
            core_count: 4,
            sq: 0.2,
            points: Vec::new(),
            cores,
            shield: None,
            length_m: Some(0.3),
            run: vec![Point::new(80.0, 100.0), Point::new(192.5, 100.0)],
        };
        let cable = insert(&mut sheet, Entity::Cable(cable));
        Lte { sheet, cable, j45, usb, wires }
    }

    /// 3.5mm AUXケーブル: J1 (connector_3p、(50,50)、ピン x=57.5 の y=45/50/55) と J2 (反転、(150,50)、
    /// ピン x=142.5) を全長1本線の区間 (57.5,50)→(142.5,50) で結ぶ。芯 L / R / GND はどちらの端もピン。
    /// 戻り値は (シート, ケーブルid)。
    pub(crate) fn aux_sheet() -> (Sheet, EntityId) {
        let mut sheet = Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape);
        let j1 = insert(&mut sheet, Entity::Symbol(connector(3, "J1", 50.0, 50.0, false)));
        let j2 = insert(&mut sheet, Entity::Symbol(connector(3, "J2", 150.0, 50.0, true)));
        let cores = ["L", "R", "GND"]
            .into_iter()
            .enumerate()
            .map(|(i, name)| CableCore {
                core: name.into(),
                end_a: Some(CoreEnd::Pin { part: j1, pin: (i + 1).to_string(), index: None }),
                end_b: Some(CoreEnd::Pin { part: j2, pin: (i + 1).to_string(), index: None }),
                ..Default::default()
            })
            .collect();
        let cable = Cable {
            id: Uuid::new_v4(),
            reference: "W58".into(),
            part_no: String::new(),
            core_count: 3,
            sq: 0.0,
            points: Vec::new(),
            cores,
            shield: None,
            length_m: None,
            run: vec![Point::new(57.5, 50.0), Point::new(142.5, 50.0)],
        };
        let id = insert(&mut sheet, Entity::Cable(cable));
        (sheet, id)
    }

    /// シート上のケーブル。
    pub(crate) fn cable_of(sheet: &Sheet, id: EntityId) -> &Cable {
        match &sheet.entities[&id] {
            Entity::Cable(c) => c,
            other => panic!("ケーブルではない: {other:?}"),
        }
    }

    /// シートを1枚だけ持つプロジェクト (シートのidはプロジェクトの先頭シートのものにそろえる)。
    pub(crate) fn project_with(mut sheet: Sheet) -> Project {
        let mut project = Project::new("t");
        sheet.id = project.sheets[0].id;
        project.sheets[0] = sheet;
        project
    }

    /// シートを1枚だけ持つエンジンと、そのシートid。
    pub(crate) fn engine_with(sheet: Sheet) -> (Engine, SheetId) {
        let project = project_with(sheet);
        let id = project.sheets[0].id;
        (Engine::new(project), id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{Command, Engine};
    use crate::model::{CableShield, CoreEnd, SymbolInstance, Wire};
    use crate::report_sheet::{report_bytes, ReportFormat, ReportKind};
    use uuid::Uuid;

    /// 横3本の配線 (y=10,20,30)。各配線は左の抵抗 L{i} と右の抵抗 R{i} を繋ぐ。
    /// 線色は黒・白・赤。戻り値は (エンジン, シートid, 配線id 3本)。
    fn three_wires() -> (Engine, crate::model::SheetId, Vec<EntityId>) {
        let mut engine = Engine::new(Project::new("ケーブル"));
        let sheet_id = engine.project().sheets[0].id;
        let mut wires = Vec::new();
        for (i, color) in ["black", "white", "red"].into_iter().enumerate() {
            let y = 10.0 * (i + 1) as f64;
            for (r, x) in [(format!("L{}", i + 1), 7.5), (format!("R{}", i + 1), 92.5)] {
                let s = Entity::Symbol(SymbolInstance {
                    id: Uuid::new_v4(),
                    symbol_id: "resistor".into(),
                    at: Point::new(x, y),
                    rotation: 0,
                    mirror: false,
                    reference: r,
                    value: String::new(),
                    attrs: Default::default(),
                });
                engine.execute(Command::AddEntity { sheet_id, entity: s }).unwrap();
            }
            let w = Wire {
                id: Uuid::new_v4(),
                points: vec![Point::new(15.0, y), Point::new(85.0, y)],
                color: color.into(),
                sq: 1.25,
                length_m: None,
                length_source: Default::default(),
                part_no: None,
                net: None,
            };
            wires.push(w.id);
            engine.execute(Command::AddEntity { sheet_id, entity: Entity::Wire(w) }).unwrap();
        }
        (engine, sheet_id, wires)
    }

    fn cable(core_count: usize, a: (f64, f64), b: (f64, f64)) -> Cable {
        Cable {
            id: Uuid::new_v4(),
            reference: "W1".into(),
            part_no: "SAMPLE-CBL".into(),
            core_count,
            sq: 1.25,
            points: vec![Point::new(a.0, a.1), Point::new(b.0, b.1)],
            cores: Vec::new(),
            shield: None,
            length_m: Some(12.5),
            run: Vec::new(),
        }
    }

    fn placed(engine: &Engine, sheet_id: crate::model::SheetId) -> Cable {
        engine
            .project()
            .sheet(sheet_id)
            .unwrap()
            .entities
            .values()
            .find_map(|e| match e {
                Entity::Cable(c) => Some(c.clone()),
                _ => None,
            })
            .expect("ケーブル")
    }

    /// Placing a cable assigns the wires its definition line crosses as cores 1, 2, 3 in order from the line's start, each with the wire's colour; one undo removes the cable.
    /// ケーブルを置くと、定義線が横切った配線が始点に近い順に芯1・2・3として割り当たり、芯の色は配線の線色になる。undo一発でケーブルが消える。
    #[test]
    fn placing_a_cable_assigns_the_crossed_wires_as_cores_in_order() {
        let (mut engine, sheet_id, wires) = three_wires();
        // 下 (y=35) から上 (y=5) へ引く: 始点に近いのは赤 (y=30)
        engine
            .execute(Command::AddCable { sheet_id, cable: cable(4, (55.0, 35.0), (45.0, 5.0)) })
            .unwrap();
        let c = placed(&engine, sheet_id);
        let cores: Vec<(&str, &str, Option<EntityId>)> =
            c.cores.iter().map(|k| (k.core.as_str(), k.color.as_str(), k.wire)).collect();
        assert_eq!(
            cores,
            [("1", "red", Some(wires[2])), ("2", "white", Some(wires[1])), ("3", "black", Some(wires[0]))]
        );
        engine.undo().unwrap();
        assert!(engine.project().sheet(sheet_id).unwrap().entities.values().all(|e| !matches!(e, Entity::Cable(_))));
    }

    /// A cable placed with no core count takes as many cores as wires it crosses; wires the line does not reach are left out.
    /// 芯数を指定せずに置いたケーブルは、横切った配線の本数を芯数にする。定義線が届かない配線は入らない。
    #[test]
    fn a_cable_without_a_core_count_takes_the_number_of_crossed_wires() {
        let (mut engine, sheet_id, _) = three_wires();
        engine
            .execute(Command::AddCable { sheet_id, cable: cable(0, (50.0, 5.0), (50.0, 25.0)) })
            .unwrap();
        let c = placed(&engine, sheet_id);
        assert_eq!(c.cores.len(), 2, "y=30の配線には届かない");
        assert_eq!(c.core_count, 2);
    }

    /// The label next to a cable reads designator, part number, cores times gauge and length.
    /// ケーブルの注記は参照記号・型番・芯数×線径・長さの順に並ぶ。
    #[test]
    fn the_cable_label_reads_designator_part_cores_gauge_and_length() {
        assert_eq!(cable_label(&cable(4, (0.0, 0.0), (1.0, 1.0))), "W1  SAMPLE-CBL  4C×1.25sq  12.5m");
        let bare = Cable { part_no: String::new(), sq: 0.0, length_m: None, ..cable(2, (0.0, 0.0), (1.0, 1.0)) };
        assert_eq!(cable_label(&bare), "W1  2C");
    }

    /// Assigning more wires than the cable has cores is an ERC error naming the cable.
    /// ケーブルの芯数より多い配線を割り当てるとERCのErrorになり、そのケーブルを指す。
    #[test]
    fn more_assigned_wires_than_cores_is_an_erc_error() {
        let (mut engine, sheet_id, _) = three_wires();
        engine
            .execute(Command::AddCable { sheet_id, cable: cable(2, (50.0, 5.0), (50.0, 35.0)) })
            .unwrap();
        let sheet = engine.project().sheet(sheet_id).unwrap();
        let diags = check_cables(sheet);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].code, CABLE_CORE_OVERFLOW);
        assert_eq!(diags[0].severity, Severity::Error);
        assert!(diags[0].message.contains("W1"));
    }

    /// The cable list shows each cable with its part, cores, gauge, length, the devices at both ends of its wires, used and spare cores, shield and sheet.
    /// ケーブル一覧は、ケーブルごとに型番・芯数・線径・長さ・配線両端の機器・使用芯/予備芯・シールド・シートを並べる。
    #[test]
    fn the_cable_list_shows_ends_used_and_spare_cores() {
        let (mut engine, sheet_id, _) = three_wires();
        let mut c = cable(4, (50.0, 5.0), (50.0, 35.0));
        c.shield = Some(CableShield { grounded: true });
        engine.execute(Command::AddCable { sheet_id, cable: c }).unwrap();
        let rows = cable_list_rows(engine.project());
        assert_eq!(
            rows,
            vec![vec![
                "W1".to_string(),
                "SAMPLE-CBL".into(),
                "4".into(),
                "1.25".into(),
                "12.5".into(),
                "L1, L2, L3".into(),
                "R1, R2, R3".into(),
                "3".into(),
                "1".into(),
                "あり (接地)".into(),
                "Sheet1".into(),
            ]]
        );
        let (csv, count) =
            report_bytes(engine.project(), ReportKind::CableList, ReportFormat::Csv, None).unwrap();
        assert!(String::from_utf8(csv).unwrap().starts_with(&CABLE_LIST_COLUMNS.join(",")));
        assert_eq!(count, 1);
    }

    /// In the BOM, cables are counted per part number: by total length when every cable has a length, otherwise by the number of cables; a cable without a part number is left out.
    /// 部品表ではケーブルを型番ごとに数え、全部に長さがあれば合計の長さ、1本でも無ければ本数にする。型番の無いケーブルは載せない。
    #[test]
    fn the_bom_counts_cables_per_part_number() {
        let (mut engine, sheet_id, _) = three_wires();
        for (r, len, part) in [("W1", Some(12.5), "SAMPLE-CBL"), ("W2", Some(3.0), "SAMPLE-CBL"), ("W3", None, "SAMPLE-X"), ("W4", None, "")] {
            let c = Cable { reference: r.into(), length_m: len, part_no: part.into(), ..cable(2, (50.0, 5.0), (50.0, 25.0)) };
            engine.execute(Command::AddCable { sheet_id, cable: c }).unwrap();
        }
        assert_eq!(
            cable_bom_rows(engine.project()),
            vec![
                vec![String::new(), "W1, W2".into(), "SAMPLE-CBL".into(), BOM_CABLE_KIND.into(), "15.50 m".into()],
                vec![String::new(), "W3".into(), "SAMPLE-X".into(), BOM_CABLE_KIND.into(), "1".into()],
            ]
        );
        assert!(crate::reports::bom_rows(engine.project()).iter().any(|r| r[2] == "SAMPLE-CBL"), "部品表に載る");
    }

    /// The printed sheet draws the cable's definition line, the shield ellipse when it has a shield, and the label.
    /// 印刷される図面には、ケーブルの定義線と、シールドがあればシールドの楕円、注記が描かれる。
    #[test]
    fn the_printed_sheet_draws_the_definition_line_shield_and_label() {
        let (mut engine, sheet_id, _) = three_wires();
        let mut c = cable(4, (50.0, 5.0), (50.0, 35.0));
        c.shield = Some(CableShield { grounded: false });
        engine.execute(Command::AddCable { sheet_id, cable: c }).unwrap();
        let sheet = engine.project().sheet(sheet_id).unwrap();
        let svg = crate::svg::project_sheet_to_svg(engine.project(), sheet_id, &sheet_symbol_defs(sheet)).unwrap();
        assert!(svg.contains("<ellipse"), "シールド");
        assert!(svg.contains("W1  SAMPLE-CBL  4C×1.25sq  12.5m"), "注記");
    }

    /// A cable whose definition line does not have two points, that has no cores, or whose core points at a wire not on the sheet is refused.
    /// 定義線が2点でない・芯数が0・シートに無い配線を指す芯を持つケーブルは拒否される。
    #[test]
    fn a_malformed_cable_is_refused() {
        let (mut engine, sheet_id, _) = three_wires();
        let mut one_point = cable(2, (0.0, 0.0), (1.0, 1.0));
        one_point.points.pop();
        let zero = cable(0, (0.0, 0.0), (1.0, 1.0));
        let mut stray = cable(2, (0.0, 0.0), (1.0, 1.0));
        stray.cores = vec![crate::model::CableCore { core: "1".into(), color: String::new(), wire: Some(Uuid::new_v4()), ..Default::default() }];
        for c in [one_point, zero, stray] {
            assert!(engine.execute(Command::AddEntity { sheet_id, entity: Entity::Cable(c) }).is_err());
        }
    }

    /// A run cable is accepted when every core has both ends on this sheet, and refused when a core lacks an end, points at a missing pin or wire, reuses a wire, still uses the definition-line field, or the run has fewer than two points; a definition-line cable with core ends is refused too.
    /// 区間ケーブルは、どの芯も両端がこのシートにあれば受け付け、端の無い芯・無いピンや配線を指す芯・同じ配線の使い回し・定義線用の欄を使った芯・2点未満の区間があれば断る。芯の行き先を持つ定義線ケーブルも断る。
    #[test]
    fn a_run_cable_is_checked_when_placed() {
        let lte = test_support::lte_sheet();
        let mut sheet = lte.sheet.clone();
        let cable = test_support::cable_of(&sheet, lte.cable).clone();
        sheet.entities.remove(&lte.cable);
        let (mut engine, sheet_id) = test_support::engine_with(sheet);
        let bad = |f: &dyn Fn(&mut Cable)| {
            let mut c = cable.clone();
            c.id = Uuid::new_v4();
            f(&mut c);
            c
        };
        let refused = [
            bad(&|c| c.cores[0].end_a = None),
            bad(&|c| c.cores[1].end_b = Some(CoreEnd::Pin { part: lte.usb, pin: "9".into(), index: None })),
            bad(&|c| c.cores[2].end_a = Some(CoreEnd::Wire { wire: Uuid::new_v4() })),
            bad(&|c| c.cores[3].end_a = Some(CoreEnd::Wire { wire: lte.wires[0] })),
            bad(&|c| c.cores[0].wire = Some(lte.wires[0])),
            bad(&|c| c.run.truncate(1)),
            bad(&|c| {
                c.run.clear();
                c.points = vec![Point::new(0.0, 0.0), Point::new(1.0, 1.0)];
            }),
        ];
        for c in refused {
            assert!(
                engine.execute(Command::AddEntity { sheet_id, entity: Entity::Cable(c) }).is_err(),
                "断るはず"
            );
        }
        engine.execute(Command::AddEntity { sheet_id, entity: Entity::Cable(cable) }).unwrap();
    }

    /// Deleting a run cable together with its breakout wires and the part it lands on, then undoing, brings all of them back, and redo deletes them again (the cable is restored after what it points at).
    /// 区間ケーブルを分岐の配線と着く先の部品と一緒に消してから元に戻すと全部が戻り、やり直すとまた消える (ケーブルは指す相手の後に戻す)。
    #[test]
    fn deleting_a_run_cable_with_its_wires_can_be_undone() {
        let lte = test_support::lte_sheet();
        let (mut engine, sheet_id) = test_support::engine_with(lte.sheet);
        let count = |e: &Engine| e.project().sheet(sheet_id).unwrap().entities.len();
        let before = count(&engine);
        let mut ids = lte.wires.clone();
        ids.push(lte.cable);
        ids.push(lte.usb);
        engine.execute(Command::DeleteEntities { sheet_id, ids }).unwrap();
        assert_eq!(count(&engine), before - 6);
        engine.undo().unwrap();
        assert_eq!(count(&engine), before);
        assert!(engine.project().sheet(sheet_id).unwrap().entities.contains_key(&lte.cable));
        engine.redo().unwrap();
        assert_eq!(count(&engine), before - 6);
    }
}
