//! ピン名で配線するための計算 (MCPの get_pins / connect_pins / place_mate)。
//!
//! エージェントがピン座標を手で計算しなくて済むように、配置済み部品のピンの用紙座標・向きと、
//! ピンどうしを結ぶ直角配線の頂点列をここで求める。編集そのものは呼び出し側がCommandで行う。

use serde::Serialize;

use crate::geometry::Point;
use crate::model::{Entity, EntityId, Sheet, SymbolInstance};
use crate::netlist::{rotate_local, segment_distance, transform_local, CONNECT_EPS};
use crate::symbol::{instance_symbol_def, resolve_symbol, sheet_symbol_defs, PinDir, SymbolDef};
use crate::tidy::{symbol_bounds, Rect};
use uuid::Uuid;

/// 配置済みピン1本 (用紙座標・回転と反転の後の向き)。
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct PinInfo {
    pub number: String,
    pub name: String,
    pub x: f64,
    pub y: f64,
    /// 電線が出ていく向き (回転・反転の後)。
    pub dir: PinDir,
}

/// 配置済み部品1個のピン一覧と外形。
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct PartPins {
    pub entity_id: EntityId,
    pub reference: String,
    pub symbol_id: String,
    /// 用紙上の外接矩形 (ブロックは5mm丸めの後)。図形の無いシンボルはnull。
    pub bbox: Option<Rect>,
    pub pins: Vec<PinInfo>,
}

/// ピンの向き (回転0度の定義) を、配置の回転・反転を通した用紙上の向きにする。
pub fn world_dir(dir: PinDir, inst: &SymbolInstance) -> PinDir {
    let v = match dir {
        PinDir::Up => Point::new(0.0, -1.0),
        PinDir::Down => Point::new(0.0, 1.0),
        PinDir::Left => Point::new(-1.0, 0.0),
        PinDir::Right => Point::new(1.0, 0.0),
    };
    let r = rotate_local(v, inst.rotation, inst.mirror);
    if r.x.abs() > r.y.abs() {
        if r.x > 0.0 {
            PinDir::Right
        } else {
            PinDir::Left
        }
    } else if r.y > 0.0 {
        PinDir::Down
    } else {
        PinDir::Up
    }
}

/// 部品1個のピン (用紙座標・向き) と外形。
pub fn part_pins(inst: &SymbolInstance, def: &SymbolDef) -> PartPins {
    let pins = def
        .pins
        .iter()
        .map(|pin| {
            let at = transform_local(pin.at, inst);
            PinInfo {
                number: pin.number.clone(),
                name: pin.name.clone(),
                x: at.x,
                y: at.y,
                dir: world_dir(pin.dir, inst),
            }
        })
        .collect();
    PartPins {
        entity_id: inst.id,
        reference: inst.reference.clone(),
        symbol_id: inst.symbol_id.clone(),
        bbox: symbol_bounds(inst, def),
        pins,
    }
}

/// シート上の部品を参照記号 (完全一致) か entity id で探す。entity id順に返す。
pub fn find_parts(sheet: &Sheet, defs: &[SymbolDef], key: &str) -> Vec<PartPins> {
    let by_id = key.parse::<EntityId>().ok();
    sheet
        .entities
        .values()
        .filter_map(|e| match e {
            Entity::Symbol(s) if Some(s.id) == by_id || s.reference == key => Some(s),
            _ => None,
        })
        .filter_map(|s| instance_symbol_def(s, defs).map(|def| part_pins(s, &def)))
        .collect()
}

/// entity id の部品1個のピン (定義はシートから集める)。シンボルでなければNone。
pub fn sheet_part_pins(sheet: &Sheet, entity_id: EntityId) -> Option<PartPins> {
    let defs = sheet_symbol_defs(sheet);
    match sheet.entities.get(&entity_id)? {
        Entity::Symbol(s) => instance_symbol_def(s, &defs).map(|def| part_pins(s, &def)),
        _ => None,
    }
}

/// 配線の折れ点をそろえるグリッド (mm)。
pub const ROUTE_GRID: f64 = 2.5;

const EPS: f64 = 1e-6;

/// エラーに並べる候補の上限。
const MAX_CANDIDATES: usize = 20;

/// ピン指定 ("参照記号.ピン") を解決した結果。
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedPin {
    pub entity_id: EntityId,
    pub reference: String,
    pub pin: PinInfo,
}

/// ピン指定を解決する。最後の「.」で参照記号 (またはentity id) とピンに分け、
/// ピンは番号で探してから名前で探す。エラーの文には直すための候補を添える。
pub fn resolve_pin(sheet: &Sheet, defs: &[SymbolDef], spec: &str) -> Result<ResolvedPin, String> {
    let Some((key, pin)) = spec.rsplit_once('.') else {
        return Err(format!(
            "ピン指定「{spec}」は「参照記号.ピン」の形で書く (例: U1.SDA、J1.2)"
        ));
    };
    let parts = find_parts(sheet, defs, key);
    if parts.is_empty() {
        return Err(format!(
            "参照記号「{key}」の部品がシートにありません。シート上の参照記号: {}",
            similar_references(sheet, key).join(", ")
        ));
    }
    // 「1#2」= 同じ番号の2つ目 (貫通端子の右側など)
    let (pin, occurrence) = match pin.rsplit_once('#') {
        Some((base, n)) => match n.parse::<usize>() {
            Ok(n) if n >= 1 => (base, Some(n)),
            _ => (pin, None),
        },
        None => (pin, None),
    };
    let mut hits: Vec<(&PartPins, &PinInfo)> = Vec::new();
    for part in &parts {
        let mut matches: Vec<&PinInfo> = part.pins.iter().filter(|x| x.number == pin).collect();
        if matches.is_empty() {
            matches = part.pins.iter().filter(|x| x.name == pin).collect();
        }
        match occurrence {
            Some(n) => {
                if let Some(x) = matches.get(n - 1) {
                    hits.push((part, x));
                }
            }
            None if matches.len() > 1 => {
                return Err(format!(
                    "{key} にはピン「{pin}」が{}つあります。「#番号」で選ぶ: {}",
                    matches.len(),
                    matches
                        .iter()
                        .enumerate()
                        .map(|(i, x)| format!("{key}.{pin}#{} ({:.1}, {:.1})", i + 1, x.x, x.y))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            None => hits.extend(matches.first().map(|x| (part, *x))),
        }
    }
    match hits.as_slice() {
        [] => {
            let listed: Vec<String> = parts
                .iter()
                .flat_map(|part| part.pins.iter())
                .map(|x| {
                    if x.name.is_empty() {
                        x.number.clone()
                    } else {
                        format!("{}({})", x.number, x.name)
                    }
                })
                .collect();
            Err(format!(
                "{key} にピン「{pin}」がありません。ピン: {}",
                listed.join(", ")
            ))
        }
        [(part, info)] => Ok(ResolvedPin {
            entity_id: part.entity_id,
            reference: part.reference.clone(),
            pin: (*info).clone(),
        }),
        many => Err(format!(
            "参照記号「{key}」の部品が複数あり、どれにもピン「{pin}」があります。entity idで指定し直す: {}",
            many.iter()
                .map(|(part, _)| format!("{}.{pin}", part.entity_id))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// 見つからない参照記号に似た、シート上の参照記号 (先頭の英字が同じもの。無ければ全部)。
fn similar_references(sheet: &Sheet, key: &str) -> Vec<String> {
    let mut refs: Vec<String> = sheet
        .entities
        .values()
        .filter_map(|e| match e {
            Entity::Symbol(s) if !s.reference.is_empty() => Some(s.reference.clone()),
            _ => None,
        })
        .collect();
    refs.sort();
    refs.dedup();
    let prefix: String = key.chars().take_while(|c| !c.is_ascii_digit()).collect();
    let similar: Vec<String> = refs
        .iter()
        .filter(|r| !prefix.is_empty() && r.starts_with(&prefix))
        .cloned()
        .collect();
    let mut out = if similar.is_empty() { refs } else { similar };
    out.truncate(MAX_CANDIDATES);
    out
}

fn snap(v: f64) -> f64 {
    (v / ROUTE_GRID).round() * ROUTE_GRID
}

fn is_horizontal(dir: PinDir) -> bool {
    matches!(dir, PinDir::Left | PinDir::Right)
}

fn aligned(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < EPS || (a.y - b.y).abs() < EPS
}

/// ピンどうしを結ぶ直角配線の頂点列。経由点が無ければ両端の向きで Z 字・L 字・直線を選び、
/// 経由点があれば順に通る (そろっていない区間は水平を先にした角を1つ足す)。
/// 端点はピンの位置のまま、折れ点だけ [`ROUTE_GRID`] にそろえる。部品は避けない。
pub fn route(
    from: Point,
    from_dir: PinDir,
    to: Point,
    to_dir: PinDir,
    via: &[Point],
) -> Vec<Point> {
    let mut pts = vec![from];
    if via.is_empty() {
        if !aligned(from, to) {
            match (is_horizontal(from_dir), is_horizontal(to_dir)) {
                (true, true) => {
                    let xm = snap((from.x + to.x) / 2.0);
                    pts.push(Point::new(xm, from.y));
                    pts.push(Point::new(xm, to.y));
                }
                (false, false) => {
                    let ym = snap((from.y + to.y) / 2.0);
                    pts.push(Point::new(from.x, ym));
                    pts.push(Point::new(to.x, ym));
                }
                (true, false) => pts.push(Point::new(to.x, from.y)),
                (false, true) => pts.push(Point::new(from.x, to.y)),
            }
        }
    } else {
        let mut prev = from;
        for &next in via.iter().chain(std::iter::once(&to)) {
            if !aligned(prev, next) {
                pts.push(Point::new(next.x, prev.y));
            }
            pts.push(next);
            prev = next;
        }
        return simplify(pts);
    }
    pts.push(to);
    simplify(pts)
}

/// ピン指定2つから配線の頂点列を決める (connect_pins の中身)。
pub fn plan_connection(
    sheet: &Sheet,
    defs: &[SymbolDef],
    from: &str,
    to: &str,
    via: &[Point],
) -> Result<Vec<Point>, String> {
    let a = resolve_pin(sheet, defs, from)?;
    let b = resolve_pin(sheet, defs, to)?;
    let (pa, pb) = (Point::new(a.pin.x, a.pin.y), Point::new(b.pin.x, b.pin.y));
    if a.entity_id == b.entity_id && pa.distance_to(&pb) < EPS {
        return Err(format!("from と to が同じピンです ({from})"));
    }
    let pts = route(pa, a.pin.dir, pb, b.pin.dir, via);
    // ネットリストは線分上のどこにピンがあっても接続とみなすので、両端以外のピンを通る経路は断る
    let crossed = pins_on_route(sheet, defs, &pts, &[&a, &b]);
    if !crossed.is_empty() {
        return Err(format!(
            "経路が別のピンの上を通るため、そのピンまでつながってしまいます: {}。viaで経由点を足して避ける",
            crossed.join(", ")
        ));
    }
    Ok(pts)
}

/// 経路の線分上にあるピン (両端のピンと、それと同じ部品の同じ番号のピンは除く)。
fn pins_on_route(
    sheet: &Sheet,
    defs: &[SymbolDef],
    pts: &[Point],
    ends: &[&ResolvedPin],
) -> Vec<String> {
    let mut out = Vec::new();
    for e in sheet.entities.values() {
        let Entity::Symbol(s) = e else { continue };
        let Some(def) = instance_symbol_def(s, defs) else {
            continue;
        };
        for pin in part_pins(s, &def).pins {
            if ends
                .iter()
                .any(|r| r.entity_id == s.id && r.pin.number == pin.number)
            {
                continue;
            }
            let at = Point::new(pin.x, pin.y);
            if pts
                .windows(2)
                .any(|seg| segment_distance(&at, &seg[0], &seg[1]) < CONNECT_EPS)
            {
                let owner = if s.reference.is_empty() {
                    s.id.to_string()
                } else {
                    s.reference.clone()
                };
                out.push(format!("{owner}.{}", pin.number));
            }
        }
    }
    out
}

/// 重なった頂点と、一直線上の途中の頂点を落とす。
fn simplify(pts: Vec<Point>) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(pts.len());
    for p in pts {
        if out.last().is_some_and(|q| q.distance_to(&p) < EPS) {
            continue;
        }
        if out.len() >= 2 {
            let (a, b) = (out[out.len() - 2], out[out.len() - 1]);
            let same_x = (a.x - b.x).abs() < EPS && (b.x - p.x).abs() < EPS;
            let same_y = (a.y - b.y).abs() < EPS && (b.y - p.y).abs() < EPS;
            if same_x || same_y {
                out.pop();
            }
        }
        out.push(p);
    }
    out
}

/// かみ合わせの組の並べ方。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MateOrientation {
    /// 左右に並べる (first=左)。
    Horizontal,
    /// 上下に並べる (first=上)。
    Vertical,
}

/// どちら側をオス (プラグ) にするか。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MaleSide {
    /// 左/上がオス。
    First,
    /// 右/下がオス。
    Second,
}

/// プラグとソケットの中心の間隔 (mm)。
pub const MATE_GAP: f64 = 5.0;

/// 置く前のかみ合わせの組と、外側の2つの接続点。
#[derive(Debug, Clone)]
pub struct MatePair {
    pub first: SymbolInstance,
    pub second: SymbolInstance,
    pub first_point: Point,
    pub second_point: Point,
}

/// プラグとソケットを [`MATE_GAP`] 離して向かい合わせに並べる。参照記号は両方、値はソケットに付ける。
pub fn mate_pair(
    at: Point,
    orientation: MateOrientation,
    male_side: MaleSide,
    reference: &str,
    value: &str,
) -> MatePair {
    let second_at = match orientation {
        MateOrientation::Horizontal => at.translated(MATE_GAP, 0.0),
        MateOrientation::Vertical => at.translated(0.0, MATE_GAP),
    };
    // (symbol_id, 回転, 反転) を first・second の順に
    let (first_spec, second_spec) = match (orientation, male_side) {
        (MateOrientation::Horizontal, MaleSide::First) => {
            (("connector_plug", 0, false), ("connector_socket", 0, false))
        }
        (MateOrientation::Horizontal, MaleSide::Second) => {
            (("connector_socket", 0, true), ("connector_plug", 0, true))
        }
        (MateOrientation::Vertical, MaleSide::First) => (
            ("connector_plug", 90, false),
            ("connector_socket", 90, false),
        ),
        (MateOrientation::Vertical, MaleSide::Second) => (
            ("connector_socket", 270, false),
            ("connector_plug", 270, false),
        ),
    };
    let make = |(symbol_id, rotation, mirror): (&str, u16, bool), at: Point| {
        let is_socket = symbol_id == "connector_socket";
        SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: symbol_id.into(),
            at,
            rotation,
            mirror,
            reference: reference.into(),
            value: if is_socket {
                value.into()
            } else {
                String::new()
            },
            attrs: Default::default(),
        }
    };
    let first = make(first_spec, at);
    let second = make(second_spec, second_at);
    let point = |inst: &SymbolInstance| {
        let def = resolve_symbol(&inst.symbol_id).expect("同梱のプラグ/ソケット");
        let pin = &part_pins(inst, &def).pins[0];
        Point::new(pin.x, pin.y)
    };
    MatePair {
        first_point: point(&first),
        second_point: point(&second),
        first,
        second,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Entity, Orientation, PaperSize, Sheet, SymbolInstance};
    use std::collections::BTreeMap;
    use uuid::Uuid;

    fn inst(
        symbol_id: &str,
        x: f64,
        y: f64,
        rotation: u16,
        mirror: bool,
        reference: &str,
    ) -> SymbolInstance {
        SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: symbol_id.into(),
            at: Point::new(x, y),
            rotation,
            mirror,
            reference: reference.into(),
            value: String::new(),
            attrs: BTreeMap::new(),
        }
    }

    fn block(reference: &str, x: f64, y: f64) -> SymbolInstance {
        let mut b = inst("block", x, y, 0, false, reference);
        b.attrs.insert(
            "block_pins".into(),
            r#"{"left":[{"no":"IN1","name":"入力1"}],"right":[{"no":"OUT1","name":"出力1"}]}"#
                .into(),
        );
        b
    }

    fn sheet_with(parts: Vec<SymbolInstance>) -> Sheet {
        let mut sheet = Sheet::new("試験", PaperSize::A3, Orientation::Landscape);
        for p in parts {
            sheet.entities.insert(p.id, Entity::Symbol(p));
        }
        sheet
    }

    /// A plug's pin is reported at its paper position, and its direction follows rotation and mirror.
    /// プラグのピンは用紙上の位置で返り、向きは回転と反転に合わせて変わる。
    #[test]
    fn pin_direction_follows_rotation_and_mirror() {
        let def = resolve_symbol("connector_plug").unwrap();
        let plain = part_pins(&inst("connector_plug", 50.0, 40.0, 0, false, "J1"), &def);
        assert_eq!((plain.pins[0].x, plain.pins[0].y), (42.5, 40.0));
        assert_eq!(plain.pins[0].dir, PinDir::Left);
        let mirrored = part_pins(&inst("connector_plug", 50.0, 40.0, 0, true, "J1"), &def);
        assert_eq!(mirrored.pins[0].dir, PinDir::Right);
        let turned = part_pins(&inst("connector_plug", 50.0, 40.0, 90, false, "J1"), &def);
        assert_eq!((turned.pins[0].x, turned.pins[0].y), (50.0, 32.5));
        assert_eq!(turned.pins[0].dir, PinDir::Up);
        let both = part_pins(&inst("connector_plug", 50.0, 40.0, 90, true, "J1"), &def);
        assert_eq!((both.pins[0].x, both.pins[0].y), (50.0, 47.5));
        assert_eq!(both.pins[0].dir, PinDir::Down);
    }

    /// A block's named pins are reported with their numbers, names, outside positions and the rounded outline.
    /// ブロックの名前付きピンは番号・名前・外側の位置で返り、外形は丸めた後の大きさで返る。
    #[test]
    fn block_pins_and_outline_are_reported() {
        let b = block("CTRL1", 100.0, 50.0);
        let sheet = sheet_with(vec![b.clone()]);
        let parts = find_parts(&sheet, &sheet_symbol_defs(&sheet), "CTRL1");
        assert_eq!(parts.len(), 1);
        let p = &parts[0];
        assert_eq!(p.entity_id, b.id);
        let in1 = p.pins.iter().find(|x| x.number == "IN1").unwrap();
        assert_eq!(in1.name, "入力1");
        assert_eq!(in1.dir, PinDir::Left);
        let bbox = p.bbox.expect("外形");
        assert!(bbox.min.x <= in1.x && in1.x < 100.0);
        let out1 = p.pins.iter().find(|x| x.number == "OUT1").unwrap();
        assert_eq!(out1.dir, PinDir::Right);
        assert!(out1.x <= bbox.max.x && out1.x > 100.0);
    }

    /// Looking up a reference returns every part that carries it, and an entity id finds exactly that part.
    /// 参照記号で引くとその参照記号の部品を全部返し、entity id で引くとその部品だけを返す。
    #[test]
    fn parts_are_found_by_reference_or_entity_id() {
        let a = inst("connector_socket", 10.0, 10.0, 0, false, "J2");
        let b = inst("connector_socket", 10.0, 30.0, 0, false, "J2");
        let sheet = sheet_with(vec![a.clone(), b.clone()]);
        let defs = sheet_symbol_defs(&sheet);
        assert_eq!(find_parts(&sheet, &defs, "J2").len(), 2);
        let by_id = find_parts(&sheet, &defs, &a.id.to_string());
        assert_eq!(by_id.len(), 1);
        assert_eq!(by_id[0].entity_id, a.id);
        assert!(find_parts(&sheet, &defs, "J9").is_empty());
    }

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    /// A pin specifier finds the pin by number first and by name second, splitting at the last dot.
    /// ピン指定は最後の「.」で分け、ピン番号で探してから名前で探す。
    #[test]
    fn a_pin_is_resolved_by_number_then_by_name() {
        let b = block("CTRL1", 100.0, 50.0);
        let dotted = block("X1.A", 40.0, 50.0);
        let sheet = sheet_with(vec![b.clone(), dotted]);
        let defs = sheet_symbol_defs(&sheet);
        assert_eq!(
            resolve_pin(&sheet, &defs, "CTRL1.IN1").unwrap().pin.number,
            "IN1"
        );
        let by_name = resolve_pin(&sheet, &defs, "CTRL1.出力1").unwrap();
        assert_eq!(by_name.pin.number, "OUT1");
        assert_eq!(by_name.entity_id, b.id);
        assert_eq!(
            resolve_pin(&sheet, &defs, "X1.A.IN1").unwrap().reference,
            "X1.A"
        );
        let by_id = resolve_pin(&sheet, &defs, &format!("{}.IN1", b.id)).unwrap();
        assert_eq!(by_id.entity_id, b.id);
    }

    /// A wrong specifier is refused with what the agent needs to fix it: the format, similar references, the part's pins, or the entity ids to choose from.
    /// 誤ったピン指定は、直すのに要るもの(書き方・似た参照記号・部品のピン・選ぶentity id)を添えて断られる。
    #[test]
    fn a_wrong_specifier_is_refused_with_candidates() {
        let a = inst("connector_socket", 10.0, 10.0, 0, false, "J2");
        let b = inst("connector_socket", 10.0, 30.0, 0, false, "J2");
        let sheet = sheet_with(vec![block("CTRL1", 100.0, 50.0), a.clone(), b.clone()]);
        let defs = sheet_symbol_defs(&sheet);
        let no_dot = resolve_pin(&sheet, &defs, "CTRL1").unwrap_err();
        assert!(no_dot.contains("参照記号.ピン"), "{no_dot}");
        let missing = resolve_pin(&sheet, &defs, "CTRL2.IN1").unwrap_err();
        assert!(missing.contains("CTRL1"), "{missing}");
        let no_pin = resolve_pin(&sheet, &defs, "CTRL1.IN9").unwrap_err();
        assert!(
            no_pin.contains("IN1") && no_pin.contains("入力1"),
            "{no_pin}"
        );
        let ambiguous = resolve_pin(&sheet, &defs, "J2.1").unwrap_err();
        assert!(
            ambiguous.contains(&a.id.to_string()) && ambiguous.contains(&b.id.to_string()),
            "{ambiguous}"
        );
    }

    /// Two left/right pins are joined with a Z whose vertical leg sits midway on the 2.5 mm grid.
    /// 左右向きのピンどうしは Z 字で結ばれ、縦の区間は2本の中央の2.5mmグリッド上に来る。
    #[test]
    fn horizontal_pins_get_a_z_route() {
        let pts = route(
            p(10.0, 10.0),
            PinDir::Right,
            p(41.0, 30.0),
            PinDir::Left,
            &[],
        );
        assert_eq!(
            pts,
            vec![p(10.0, 10.0), p(25.0, 10.0), p(25.0, 30.0), p(41.0, 30.0)]
        );
    }

    /// Two up/down pins are joined with a Z whose horizontal leg sits midway.
    /// 上下向きのピンどうしは Z 字で結ばれ、横の区間は中央に来る。
    #[test]
    fn vertical_pins_get_a_z_route() {
        let pts = route(p(10.0, 10.0), PinDir::Down, p(30.0, 40.0), PinDir::Up, &[]);
        assert_eq!(
            pts,
            vec![p(10.0, 10.0), p(10.0, 25.0), p(30.0, 25.0), p(30.0, 40.0)]
        );
    }

    /// A left/right pin and an up/down pin are joined with an L, each end leaving in its own direction.
    /// 左右向きと上下向きのピンは L 字で結ばれ、各端は自分の向きへ出る。
    #[test]
    fn mixed_pins_get_an_l_route() {
        let pts = route(p(10.0, 10.0), PinDir::Right, p(30.0, 40.0), PinDir::Up, &[]);
        assert_eq!(pts, vec![p(10.0, 10.0), p(30.0, 10.0), p(30.0, 40.0)]);
        let back = route(p(30.0, 40.0), PinDir::Up, p(10.0, 10.0), PinDir::Right, &[]);
        assert_eq!(back, vec![p(30.0, 40.0), p(30.0, 10.0), p(10.0, 10.0)]);
    }

    /// Pins that are already aligned get one straight segment, whatever their directions.
    /// すでに一直線に並んだピンは、向きにかかわらず直線1本で結ばれる。
    #[test]
    fn aligned_pins_get_a_straight_wire() {
        assert_eq!(
            route(p(10.0, 10.0), PinDir::Right, p(40.0, 10.0), PinDir::Up, &[]),
            vec![p(10.0, 10.0), p(40.0, 10.0)]
        );
        assert_eq!(
            route(p(10.0, 10.0), PinDir::Down, p(10.0, 40.0), PinDir::Up, &[]),
            vec![p(10.0, 10.0), p(10.0, 40.0)]
        );
    }

    /// Pins off the grid keep their exact end points while the bend snaps to the grid.
    /// グリッドから外れたピンでも端点はピンの位置のままで、曲がる位置だけグリッドにそろう。
    #[test]
    fn off_grid_pins_keep_their_ends() {
        let pts = route(
            p(10.3, 10.0),
            PinDir::Right,
            p(41.1, 30.0),
            PinDir::Left,
            &[],
        );
        assert_eq!(pts.first(), Some(&p(10.3, 10.0)));
        assert_eq!(pts.last(), Some(&p(41.1, 30.0)));
        assert_eq!(pts[1].x, 25.0);
    }

    /// With via points the wire passes each one in order, adding a horizontal-first corner where two points are not aligned.
    /// 経由点があると順に通り、そろっていない区間には水平を先にした角を1つ足す。
    #[test]
    fn via_points_are_passed_in_order() {
        let pts = route(
            p(10.0, 10.0),
            PinDir::Right,
            p(50.0, 50.0),
            PinDir::Left,
            &[p(20.0, 30.0)],
        );
        assert_eq!(
            pts,
            vec![
                p(10.0, 10.0),
                p(20.0, 10.0),
                p(20.0, 30.0),
                p(50.0, 30.0),
                p(50.0, 50.0)
            ]
        );
        for w in pts.windows(2) {
            assert!(w[0].x == w[1].x || w[0].y == w[1].y, "斜めの線分: {w:?}");
        }
    }

    /// A mate pair puts the plug and the socket 5 mm apart facing each other, in both orientations and with the male part on either side; both carry the designator and the socket carries the value.
    /// かみ合わせの組は、どちらの向きでもオスがどちら側でも、プラグとソケットを5mm離して向かい合わせに置く。参照記号は両方に、値はソケットに付く。
    #[test]
    fn a_mate_pair_faces_each_other_in_every_layout() {
        let at = p(100.0, 50.0);
        let h1 = mate_pair(at, MateOrientation::Horizontal, MaleSide::First, "J1", "XH");
        assert_eq!(
            (h1.first.symbol_id.as_str(), h1.second.symbol_id.as_str()),
            ("connector_plug", "connector_socket")
        );
        assert_eq!(
            (h1.first_point, h1.second_point),
            (p(92.5, 50.0), p(112.5, 50.0))
        );
        assert_eq!(
            (h1.second.reference.as_str(), h1.second.value.as_str()),
            ("J1", "XH")
        );
        assert_eq!(
            (h1.first.reference.as_str(), h1.first.value.as_str()),
            ("J1", "")
        );
        let h2 = mate_pair(at, MateOrientation::Horizontal, MaleSide::Second, "J1", "");
        assert_eq!(h2.first.symbol_id, "connector_socket");
        assert_eq!(
            (h2.first_point, h2.second_point),
            (p(92.5, 50.0), p(112.5, 50.0))
        );
        assert_eq!(h2.first.reference, "J1");
        let v1 = mate_pair(at, MateOrientation::Vertical, MaleSide::First, "J1", "");
        assert_eq!(v1.first.symbol_id, "connector_plug");
        assert_eq!(
            (v1.first_point, v1.second_point),
            (p(100.0, 42.5), p(100.0, 62.5))
        );
        let v2 = mate_pair(at, MateOrientation::Vertical, MaleSide::Second, "J1", "");
        assert_eq!(v2.first.symbol_id, "connector_socket");
        assert_eq!(
            (v2.first_point, v2.second_point),
            (p(100.0, 42.5), p(100.0, 62.5))
        );
    }

    /// A route that would run over a third pin is refused, because the netlist would join that pin to the wire; the message names the pin.
    /// 経路が別のピンの上を通るとネットリスト上そのピンもつながってしまうので断り、どのピンかを知らせる。
    #[test]
    fn a_route_that_passes_over_another_pin_is_refused() {
        let ctrl = block("CTRL1", 100.0, 50.0);
        let mut target = block("U2", 200.0, 50.0);
        target.attrs.insert(
            "block_pins".into(),
            r#"{"left":[{"no":"1","name":"A"}]}"#.into(),
        );
        let sheet = sheet_with(vec![ctrl, target]);
        let defs = sheet_symbol_defs(&sheet);
        let err = plan_connection(&sheet, &defs, "CTRL1.IN1", "U2.1", &[]).unwrap_err();
        assert!(err.contains("CTRL1.OUT1"), "{err}");
        let ok = plan_connection(&sheet, &defs, "CTRL1.OUT1", "U2.1", &[]).unwrap();
        assert_eq!(ok.len(), 2);
    }

    /// Wiring a pin to itself is refused, whether it is named by number or by name.
    /// 同じピンどうしの配線は、番号で書いても名前で書いても断られる。
    #[test]
    fn wiring_a_pin_to_itself_is_refused() {
        let sheet = sheet_with(vec![block("CTRL1", 100.0, 50.0)]);
        let defs = sheet_symbol_defs(&sheet);
        let err = plan_connection(&sheet, &defs, "CTRL1.IN1", "CTRL1.入力1", &[]).unwrap_err();
        assert!(err.contains("同じピン"), "{err}");
    }

    /// When one part has the same pin number twice (a through terminal), the bare number is refused with both choices, and "#n" picks one.
    /// 1つの部品に同じピン番号が2つある(貫通端子)ときは、番号だけの指定を両方の候補つきで断り、「#n」で片方を選べる。
    #[test]
    fn a_through_terminal_side_is_chosen_with_an_occurrence() {
        let tb = inst("terminal_block_2p", 50.0, 50.0, 0, false, "TB1");
        let sheet = sheet_with(vec![tb]);
        let defs = sheet_symbol_defs(&sheet);
        let err = resolve_pin(&sheet, &defs, "TB1.1").unwrap_err();
        assert!(err.contains("TB1.1#1") && err.contains("TB1.1#2"), "{err}");
        let left = resolve_pin(&sheet, &defs, "TB1.1#1").unwrap();
        let right = resolve_pin(&sheet, &defs, "TB1.1#2").unwrap();
        assert!(left.pin.x < right.pin.x);
        assert_eq!(right.pin.dir, PinDir::Right);
        assert!(resolve_pin(&sheet, &defs, "TB1.1#3").is_err());
    }

    /// A mate pair sits on the 2.5 mm grid and its two halves only touch, so the tidy metrics count no overlap and no off-grid point for it.
    /// かみ合わせの組は2.5mmグリッドにのり、2つは接するだけなので、整頓指標は組を重なりにもグリッド外にも数えない。
    #[test]
    fn a_mate_pair_is_tidy() {
        for orientation in [MateOrientation::Horizontal, MateOrientation::Vertical] {
            for side in [MaleSide::First, MaleSide::Second] {
                let pair = mate_pair(p(100.0, 50.0), orientation, side, "J1", "");
                let sheet = sheet_with(vec![pair.first, pair.second]);
                let m = crate::tidy::tidy_metrics(&sheet, &sheet_symbol_defs(&sheet));
                assert_eq!(
                    (m.symbol_overlaps, m.off_grid),
                    (0, 0),
                    "{orientation:?} {side:?}"
                );
                for q in [pair.first_point, pair.second_point] {
                    assert!(crate::tidy::is_on_grid(&q), "{q:?}");
                }
            }
        }
    }
}
