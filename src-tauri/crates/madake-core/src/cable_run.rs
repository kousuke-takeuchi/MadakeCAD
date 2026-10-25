//! 1本線のケーブルをピン名で引くための計算 (MCPの connect_cable)。
//!
//! エージェントは芯の名前・両端のピン (または部品)・端ごとの描き方 (`multi` = 芯ごとの配線を
//! ピンから分岐点まで引く / `single` = 1本線をその部品に着ける) だけを渡す。ここが分岐点・芯ごとの
//! 配線・1本線の経路・芯の割当てを決め、ケーブルと分岐の配線を返す。芯はピン順に対応させる。
//! 編集そのものは呼び出し側が1回の編集 (`execute_batch`) で行う。

use serde::Deserialize;
use uuid::Uuid;

use crate::geometry::Point;
use crate::model::{Cable, CableCore, CableShield, CoreEnd, Entity, Sheet, Wire};
use crate::pin_wiring::{find_parts, pins_on_route, resolve_pin, route, ResolvedPin, ROUTE_GRID};
use crate::symbol::{PinDir, SymbolDef};

/// 自動で置く分岐点を、ピンの並びの先からどれだけ離すか (mm)。
pub const BREAKOUT_OFFSET_MM: f64 = 10.0;
/// 芯ごとの配線の分岐側の端を、分岐点からピン側へどれだけ戻すか (mm)。扇の線の長さになる。
pub const FAN_LENGTH_MM: f64 = 2.5;

/// 端の描き方。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SideStyle {
    /// 芯ごとの配線をピンから分岐点まで引き、そこから1本線にする。
    Multi,
    /// 1本線をそのまま部品に着ける (着いたピンを横切る棒で描く)。
    Single,
}

/// ケーブルの片側の指定。
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct CableSide {
    /// 芯が着くピンを芯の順に。"参照記号.ピン" (例 "J45.1"、同じ番号の2つ目は "TB1.1#2")。
    /// `part` を書いたときはピン番号か名前だけでもよい。
    #[serde(default)]
    pub pins: Option<Vec<String>>,
    /// 部品 (参照記号かentity id)。`pins` を省略するとこの部品のピンをピン順に使う。
    #[serde(default)]
    pub part: Option<String>,
    /// multi = 芯ごとの配線を引いて分岐 / single = 1本線をこの部品に着ける。
    pub style: SideStyle,
    /// 分岐点 (multi のとき、mm。2.5mmグリッドに丸める)。省略時はピンの並びの先 10mm。
    #[serde(default)]
    pub breakout_at: Option<Point>,
}

/// 芯1本の指定: 名前だけか、名前と線色。
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum CoreSpec {
    Name(String),
    Full {
        core: String,
        #[serde(default)]
        color: Option<String>,
    },
}

impl CoreSpec {
    fn name(&self) -> &str {
        match self {
            CoreSpec::Name(n) => n,
            CoreSpec::Full { core, .. } => core,
        }
    }

    fn color(&self) -> Option<&str> {
        match self {
            CoreSpec::Name(_) => None,
            CoreSpec::Full { color, .. } => color.as_deref(),
        }
    }
}

/// connect_cable の1本ぶんの指定。
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct CableRunSpec {
    /// 参照記号 (省略時はシートの次の W 番号)。
    #[serde(default)]
    pub reference: Option<String>,
    /// 型番 (部品表に載る)。
    #[serde(default)]
    pub part_no: Option<String>,
    /// 線径 sq (分岐の配線にも使う。省略時はケーブル未設定・配線0.3)。
    #[serde(default)]
    pub sq: Option<f64>,
    /// ケーブル全長 m (分岐の先も含む、買って切る長さ)。
    #[serde(default)]
    pub length_m: Option<f64>,
    /// シールド。
    #[serde(default)]
    pub shield: Option<CableShield>,
    /// 芯の線色の既定 (芯ごとの color が無いとき)。
    #[serde(default)]
    pub color: Option<String>,
    /// 芯 (順に、両端のピンへピン順に対応する)。
    pub cores: Vec<CoreSpec>,
    /// 区間の始点側。
    pub a: CableSide,
    /// 区間の終点側。
    pub b: CableSide,
    /// 1本線の経由点 (mm)。
    #[serde(default)]
    pub via: Option<Vec<Point>>,
}

/// 計画したケーブルと、その分岐の配線。
#[derive(Debug, Clone)]
pub struct PlannedCable {
    pub cable: Cable,
    /// 分岐の配線 (a 側の芯の順、続いて b 側)。
    pub wires: Vec<Wire>,
    /// 芯ごとの両端の表記 ("J45.1" / "TB1.1#2")。
    pub ends: Vec<[String; 2]>,
}

impl PlannedCable {
    /// 図面へ入れる順のエンティティ (配線が先、ケーブルが最後。ケーブルの検証は配線があることを見る)。
    pub fn entities(&self) -> Vec<Entity> {
        let mut out: Vec<Entity> = self.wires.iter().cloned().map(Entity::Wire).collect();
        out.push(Entity::Cable(self.cable.clone()));
        out
    }
}

/// 片側の計画結果。
struct SidePlan {
    /// 1本線の端の点。
    at: Point,
    /// 1本線がこの端から出ていく向き。
    dir: PinDir,
    ends: Vec<CoreEnd>,
    wires: Vec<Wire>,
    labels: Vec<String>,
}

/// 1本線のケーブルを計画する。シートは変えない。
pub fn plan_cable_run(sheet: &Sheet, defs: &[SymbolDef], spec: &CableRunSpec) -> Result<PlannedCable, String> {
    if spec.cores.is_empty() {
        return Err("cores に芯を1本以上書く".into());
    }
    if let Some(name) = crate::cable::duplicate_core_name(spec.cores.iter().map(|c| c.name())) {
        return Err(format!("cores に呼び名 {name} の芯が2本あります。呼び名は芯ごとに変える"));
    }
    let colors: Vec<String> = spec
        .cores
        .iter()
        .map(|c| c.color().or(spec.color.as_deref()).unwrap_or("").to_string())
        .collect();
    let a = plan_side(sheet, defs, spec, "a", &spec.a, &colors)?;
    let b = plan_side(sheet, defs, spec, "b", &spec.b, &colors)?;
    let run = route(a.at, a.dir, b.at, b.dir, spec.via.as_deref().unwrap_or_default());
    if run.len() < 2 {
        return Err("a と b の1本線の端が同じ点です。部品か breakout_at を離す".into());
    }
    let cores = spec
        .cores
        .iter()
        .enumerate()
        .map(|(i, c)| CableCore {
            core: c.name().to_string(),
            color: colors[i].clone(),
            wire: None,
            end_a: Some(a.ends[i].clone()),
            end_b: Some(b.ends[i].clone()),
        })
        .collect();
    let cable = Cable {
        id: Uuid::new_v4(),
        reference: spec
            .reference
            .clone()
            .unwrap_or_else(|| crate::cable::next_cable_reference(sheet)),
        part_no: spec.part_no.clone().unwrap_or_default(),
        core_count: spec.cores.len(),
        sq: spec.sq.unwrap_or(0.0),
        points: Vec::new(),
        cores,
        shield: spec.shield.clone(),
        length_m: spec.length_m,
        run,
    };
    let ends = (0..spec.cores.len()).map(|i| [a.labels[i].clone(), b.labels[i].clone()]).collect();
    let mut wires = a.wires;
    wires.extend(b.wires);
    Ok(PlannedCable { cable, wires, ends })
}

/// 向きの単位ベクトル。
fn unit(dir: PinDir) -> Point {
    match dir {
        PinDir::Right => Point::new(1.0, 0.0),
        PinDir::Left => Point::new(-1.0, 0.0),
        PinDir::Down => Point::new(0.0, 1.0),
        PinDir::Up => Point::new(0.0, -1.0),
    }
}

fn opposite(dir: PinDir) -> PinDir {
    match dir {
        PinDir::Right => PinDir::Left,
        PinDir::Left => PinDir::Right,
        PinDir::Down => PinDir::Up,
        PinDir::Up => PinDir::Down,
    }
}

/// "TB1.1#2" の 2。
fn occurrence(spec: &str) -> Option<u32> {
    spec.rsplit_once('#').and_then(|(_, n)| n.parse().ok()).filter(|n| *n >= 1)
}

/// 端の指定をピンの並び (解決結果, 何番目) にする。
fn side_pins(
    sheet: &Sheet,
    defs: &[SymbolDef],
    name: &str,
    side: &CableSide,
) -> Result<Vec<(ResolvedPin, Option<u32>)>, String> {
    match (&side.pins, &side.part) {
        (Some(list), part) => list
            .iter()
            .map(|spec| {
                let full = match part {
                    Some(p) if !spec.contains('.') => format!("{p}.{spec}"),
                    _ => spec.clone(),
                };
                let r = resolve_pin(sheet, defs, &full).map_err(|e| format!("{name} 側 {full}: {e}"))?;
                Ok((r, occurrence(&full)))
            })
            .collect(),
        (None, Some(part)) => {
            let parts = find_parts(sheet, defs, part);
            let [one] = parts.as_slice() else {
                return Err(format!(
                    "{name} 側の部品「{part}」が{}個あります。1つに決まる参照記号か entity id を書く",
                    parts.len()
                ));
            };
            // ピン順。同じ番号の接続点 (貫通端子の左右) は最初の1つだけ
            let mut seen: Vec<&str> = Vec::new();
            let mut out = Vec::new();
            for pin in &one.pins {
                if seen.contains(&pin.number.as_str()) {
                    continue;
                }
                seen.push(pin.number.as_str());
                out.push((
                    ResolvedPin { entity_id: one.entity_id, reference: one.reference.clone(), pin: pin.clone() },
                    None,
                ));
            }
            Ok(out)
        }
        (None, None) => Err(format!("{name} 側に pins か part を書く")),
    }
}

fn plan_side(
    sheet: &Sheet,
    defs: &[SymbolDef],
    spec: &CableRunSpec,
    name: &str,
    side: &CableSide,
    colors: &[String],
) -> Result<SidePlan, String> {
    let pins = side_pins(sheet, defs, name, side)?;
    if pins.len() != spec.cores.len() {
        return Err(format!(
            "{name} 側のピンが {} 本で、芯は {} 本です。ピンと芯は同じ数にする (芯はピン順に対応する)",
            pins.len(),
            spec.cores.len()
        ));
    }
    let labels: Vec<String> = pins
        .iter()
        .map(|(r, index)| {
            let owner = if r.reference.is_empty() { r.entity_id.to_string() } else { r.reference.clone() };
            match index {
                Some(n) => format!("{owner}.{}#{n}", r.pin.number),
                None => format!("{owner}.{}", r.pin.number),
            }
        })
        .collect();
    let dir = pins[0].0.pin.dir;
    match side.style {
        SideStyle::Single => {
            let part = pins[0].0.entity_id;
            if pins.iter().any(|(r, _)| r.entity_id != part) {
                return Err(format!(
                    "{name} 側は single なので、ピンは1つの部品のものだけにする (1本線はその部品に着く)"
                ));
            }
            let positions: Vec<Point> = pins.iter().map(|(r, _)| Point::new(r.pin.x, r.pin.y)).collect();
            let (lo, hi) = crate::cable::bar_ends(&positions).ok_or("ピンがありません")?;
            let at = Point::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
            let ends = pins
                .iter()
                .map(|(r, index)| CoreEnd::Pin { part, pin: r.pin.number.clone(), index: *index })
                .collect();
            Ok(SidePlan { at, dir, ends, wires: Vec::new(), labels })
        }
        SideStyle::Multi => {
            let d = unit(dir);
            let at = match side.breakout_at {
                Some(p) => p.snapped(ROUTE_GRID),
                None => auto_breakout(&pins, d),
            };
            let mut wires = Vec::new();
            let mut ends = Vec::new();
            let mut taken: Vec<Point> = Vec::new();
            for (i, (r, _)) in pins.iter().enumerate() {
                let core = spec.cores[i].name();
                let pin_at = Point::new(r.pin.x, r.pin.y);
                let ahead = (at.x - pin_at.x) * d.x + (at.y - pin_at.y) * d.y;
                if ahead < FAN_LENGTH_MM + ROUTE_GRID - 1e-6 {
                    return Err(format!(
                        "{name} 側の分岐点 ({:.1}, {:.1}) が {} の先 (向き {:?}) に {}mm 以上離れていません。breakout_at をピンの外側へずらす",
                        at.x, at.y, labels[i], dir, FAN_LENGTH_MM + ROUTE_GRID
                    ));
                }
                // 分岐側の端: 分岐点から FAN_LENGTH_MM だけピン側へ戻り、ピンと同じ並びの位置
                let end = if d.x != 0.0 {
                    Point::new(at.x - d.x * FAN_LENGTH_MM, pin_at.y)
                } else {
                    Point::new(pin_at.x, at.y - d.y * FAN_LENGTH_MM)
                };
                if taken.iter().any(|p| p.distance_to(&end) < 1e-6) {
                    return Err(format!(
                        "{name} 側の芯 {core} の分岐側の端が他の芯と重なります (ピンが一列に並んでいません)。ピンの並ぶ部品を選ぶか single にする"
                    ));
                }
                if crate::netlist::point_is_attached(sheet, defs, &end, None) {
                    return Err(format!(
                        "{name} 側の芯 {core} の分岐側の端 ({:.1}, {:.1}) に別の配線やピンがあります。breakout_at をずらす",
                        end.x, end.y
                    ));
                }
                let points = route(pin_at, r.pin.dir, end, opposite(r.pin.dir), &[]);
                let crossed = pins_on_route(sheet, defs, &points, &[r]);
                if !crossed.is_empty() {
                    return Err(format!(
                        "{name} 側の芯 {core} の配線が別のピンの上を通ります: {}。breakout_at をずらす",
                        crossed.join(", ")
                    ));
                }
                taken.push(end);
                let wire = Wire {
                    id: Uuid::new_v4(),
                    points,
                    color: if colors[i].is_empty() { "black".into() } else { colors[i].clone() },
                    sq: spec.sq.unwrap_or(0.3),
                    length_m: None,
                    length_source: Default::default(),
                    part_no: None,
                    net: None,
                };
                ends.push(CoreEnd::Wire { wire: wire.id });
                wires.push(wire);
            }
            Ok(SidePlan { at, dir, ends, wires, labels })
        }
    }
}

/// 自動の分岐点: ピンの並びのいちばん先 (向き d) から BREAKOUT_OFFSET_MM 先、並びの中央。2.5mmグリッドに丸める。
fn auto_breakout(pins: &[(ResolvedPin, Option<u32>)], d: Point) -> Point {
    let n = pins.len() as f64;
    let (cx, cy) = pins.iter().fold((0.0, 0.0), |(x, y), (r, _)| (x + r.pin.x / n, y + r.pin.y / n));
    let reach = pins
        .iter()
        .map(|(r, _)| r.pin.x * d.x + r.pin.y * d.y)
        .fold(f64::MIN, f64::max)
        + BREAKOUT_OFFSET_MM;
    let p = if d.x != 0.0 { Point::new(reach * d.x, cy) } else { Point::new(cx, reach * d.y) };
    p.snapped(ROUTE_GRID)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cable::test_support::{connector, insert};
    use crate::model::{Orientation, PaperSize, SymbolInstance};
    use crate::symbol::sheet_symbol_defs;

    fn spec(json: serde_json::Value) -> CableRunSpec {
        serde_json::from_value(json).unwrap()
    }

    /// J45 (基板、ピンは x=57.5 で右向き) と USB1 (反転して (200,100)、ピンは x=192.5 で左向き)。
    fn board_and_plug() -> Sheet {
        let mut sheet = Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape);
        insert(&mut sheet, Entity::Symbol(connector(4, "J45", 50.0, 100.0, false)));
        insert(&mut sheet, Entity::Symbol(connector(4, "USB1", 200.0, 100.0, true)));
        sheet
    }

    /// Planning the LTE harness draws one wire per core from the board pins to a breakout 10 mm out, runs one line from there to the plug, and maps the cores to the pins in pin order; the result is four nets.
    /// LTEハーネスを計画すると、基板のピンから10mm先の分岐点まで芯ごとに1本ずつ配線を引き、そこからプラグまで1本線を通し、芯をピン順にピンへ対応させる。結果は4ネットになる。
    #[test]
    fn plans_the_lte_harness() {
        let mut sheet = board_and_plug();
        let defs = sheet_symbol_defs(&sheet);
        let planned = plan_cable_run(
            &sheet,
            &defs,
            &spec(serde_json::json!({
                "reference": "W45", "sq": 0.2,
                "cores": ["VBUS", "D-", {"core": "D+", "color": "green"}, "GND"],
                "a": {"part": "J45", "style": "multi"},
                "b": {"part": "USB1", "style": "single"}
            })),
        )
        .unwrap();
        assert_eq!(planned.wires.len(), 4);
        assert_eq!(planned.wires[0].points, vec![Point::new(57.5, 92.5), Point::new(65.0, 92.5)]);
        assert_eq!(planned.cable.run, vec![Point::new(67.5, 100.0), Point::new(192.5, 100.0)]);
        assert_eq!(planned.ends[1], ["J45.2".to_string(), "USB1.2".to_string()]);
        assert_eq!(planned.cable.cores[2].color, "green");
        assert_eq!(planned.wires[2].color, "green");
        for e in planned.entities() {
            insert(&mut sheet, e);
        }
        crate::cable::validate_run_cable(&sheet, &planned.cable).unwrap();
        let nets = crate::netlist::extract_netlist(&sheet, &defs);
        assert_eq!(nets.len(), 4);
        assert!(nets.iter().all(|n| n.pins.len() == 2));
    }

    /// Two connectors joined end to end by one line (3.5 mm AUX) get a run between their pin columns and no wires.
    /// 全長1本線で結ぶ2つのコネクタ (3.5mm AUX) は、ピン列の間に1本線が通り、配線は無い。
    #[test]
    fn plans_a_single_line_between_two_connectors() {
        let mut sheet = Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape);
        let j1 = insert(&mut sheet, Entity::Symbol(connector(3, "J1", 50.0, 50.0, false)));
        insert(&mut sheet, Entity::Symbol(connector(3, "J2", 150.0, 50.0, true)));
        let defs = sheet_symbol_defs(&sheet);
        let planned = plan_cable_run(
            &sheet,
            &defs,
            &spec(serde_json::json!({
                "cores": ["L", "R", "GND"],
                "a": {"part": "J1", "style": "single"},
                "b": {"part": "J2", "style": "single"}
            })),
        )
        .unwrap();
        assert!(planned.wires.is_empty());
        assert_eq!(planned.cable.run, vec![Point::new(57.5, 50.0), Point::new(142.5, 50.0)]);
        assert_eq!(planned.cable.cores[0].end_a, Some(CoreEnd::Pin { part: j1, pin: "1".into(), index: None }));
        assert_eq!(planned.cable.reference, "W1");
    }

    /// Pins may be given by name as well as number, and a side whose pin count differs from the core count is refused, naming the side and both counts.
    /// ピンは番号でも名前でも指定でき、ピンの数が芯の数と違う端は、その端と両方の数を示して断る。
    #[test]
    fn pins_by_name_and_a_count_mismatch() {
        let mut sheet = Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape);
        let block = SymbolInstance {
            id: uuid::Uuid::new_v4(),
            symbol_id: crate::block::BLOCK_SYMBOL_ID.into(),
            at: Point::new(50.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "CTRL1".into(),
            value: String::new(),
            attrs: [(
                crate::block::BLOCK_PINS_ATTR.to_string(),
                r#"{"right":[{"no":"1","name":"TX"},{"no":"2","name":"RX"},{"no":"3","name":"GND"}]}"#.to_string(),
            )]
            .into_iter()
            .collect(),
        };
        insert(&mut sheet, Entity::Symbol(block));
        insert(&mut sheet, Entity::Symbol(connector(3, "J2", 150.0, 50.0, true)));
        let defs = sheet_symbol_defs(&sheet);
        let ok = plan_cable_run(
            &sheet,
            &defs,
            &spec(serde_json::json!({
                "cores": ["TX", "RX", "GND"],
                "a": {"pins": ["CTRL1.TX", "CTRL1.RX", "CTRL1.GND"], "style": "multi"},
                "b": {"part": "J2", "style": "single"}
            })),
        )
        .unwrap();
        assert_eq!(ok.ends[0][0], "CTRL1.1");
        let err = plan_cable_run(
            &sheet,
            &defs,
            &spec(serde_json::json!({
                "cores": ["TX", "RX", "GND", "5V"],
                "a": {"pins": ["CTRL1.TX", "CTRL1.RX", "CTRL1.GND"], "style": "multi"},
                "b": {"part": "J2", "style": "single"}
            })),
        )
        .unwrap_err();
        assert!(err.contains("a 側のピンが 3 本で、芯は 4 本"), "{err}");
    }

    /// Planning a cable with two cores of the same name (after trimming spaces) is refused, naming the duplicate.
    /// 同じ呼び名 (前後の空白を除いて) の芯が2本あるケーブルの計画は断り、重なった呼び名を示す。
    #[test]
    fn duplicate_core_names_are_refused() {
        let sheet = board_and_plug();
        let defs = sheet_symbol_defs(&sheet);
        let err = plan_cable_run(
            &sheet,
            &defs,
            &spec(serde_json::json!({
                "cores": ["GND", " GND", "D+", "D-"],
                "a": {"part": "J45", "style": "multi"},
                "b": {"part": "USB1", "style": "single"}
            })),
        )
        .unwrap_err();
        assert!(err.contains("GND") && err.contains("呼び名"), "{err}");
    }

    /// A single line landing on a through terminal block maps each core to one terminal number, using the left connection points unless "#2" picks the right ones.
    /// 貫通端子台に着く1本線は、芯を端子番号1つずつに対応させ、ふだんは左の接続点、「#2」を付ければ右の接続点に着く。
    #[test]
    fn a_single_line_on_a_through_terminal_block() {
        let mut sheet = Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape);
        insert(&mut sheet, Entity::Symbol(connector(3, "J1", 20.0, 100.0, false)));
        let mut tb = connector(3, "TB1", 100.0, 100.0, false);
        tb.symbol_id = "terminal_block_3p".into();
        let tb = insert(&mut sheet, Entity::Symbol(tb));
        let defs = sheet_symbol_defs(&sheet);
        let by_part = plan_cable_run(
            &sheet,
            &defs,
            &spec(serde_json::json!({
                "cores": ["1", "2", "3"],
                "a": {"part": "J1", "style": "single"},
                "b": {"part": "TB1", "style": "single"}
            })),
        )
        .unwrap();
        assert_eq!(by_part.cable.cores[2].end_b, Some(CoreEnd::Pin { part: tb, pin: "3".into(), index: None }));
        assert_eq!(by_part.cable.run.last().unwrap().x, 97.5);
        let right = plan_cable_run(
            &sheet,
            &defs,
            &spec(serde_json::json!({
                "cores": ["1", "2", "3"],
                "a": {"part": "J1", "style": "single"},
                "b": {"pins": ["TB1.1#2", "TB1.2#2", "TB1.3#2"], "style": "single"}
            })),
        )
        .unwrap();
        assert_eq!(right.cable.cores[0].end_b, Some(CoreEnd::Pin { part: tb, pin: "1".into(), index: Some(2) }));
        assert_eq!(right.cable.run.last().unwrap().x, 102.5);
        assert_eq!(right.ends[0][1], "TB1.1#2");
    }

    /// A breakout placed behind the pins, or whose wire ends would land on another wire, is refused with a hint to move breakout_at.
    /// ピンより後ろに置いた分岐点や、芯の配線の端が別の配線に乗る分岐点は、breakout_at をずらすよう伝えて断る。
    #[test]
    fn a_bad_breakout_is_refused() {
        let mut sheet = board_and_plug();
        let defs = sheet_symbol_defs(&sheet);
        let lte = |breakout: serde_json::Value| {
            spec(serde_json::json!({
                "cores": ["VBUS", "D-", "D+", "GND"],
                "a": {"part": "J45", "style": "multi", "breakout_at": breakout},
                "b": {"part": "USB1", "style": "single"}
            }))
        };
        let behind = plan_cable_run(&sheet, &defs, &lte(serde_json::json!({"x": 50.0, "y": 100.0}))).unwrap_err();
        assert!(behind.contains("breakout_at"), "{behind}");
        insert(&mut sheet, Entity::Wire(crate::cable::test_support::wire(&[(65.0, 92.5), (65.0, 80.0)])));
        let on_wire = plan_cable_run(&sheet, &defs, &lte(serde_json::Value::Null)).unwrap_err();
        assert!(on_wire.contains("breakout_at"), "{on_wire}");
    }
}
