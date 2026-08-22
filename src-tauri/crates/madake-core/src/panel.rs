//! 2D盤レイアウト (M4仕様 §8)。
//!
//! 盤レイアウトシート ([`SheetKind::Panel`]) は「部品を盤のどこに取り付けるか」を描く
//! シートで、回路図とは別の道具立てを持つ:
//!
//! - **盤外形** ([`PanelOutline`]): 筐体と、その内側の取付板
//! - **DINレール** ([`DinRail`]) / **ダクト** ([`Duct`]): 部品を留めるレールと配線を通す樋
//! - **フットプリント** ([`Footprint`]): 部品が盤面に占める外形。寸法は部品DBの
//!   `footprint_w`/`footprint_h` から引き、登録の無い型番は [`DEFAULT_FOOTPRINT_SIZE`] で置く
//!
//! 回路図のシンボルとは**参照記号**で結び付く。回路図にあって盤に無い部品は
//! [`unplaced_components`] が「未配置」として返し、逆に回路図に無い参照記号の
//! フットプリントは [`panel_diagnostics`] が注意として報告する。
//!
//! ここは読み取り専用の純関数だけを置く (図面の編集は必ずCommand経由)。描画の数式も
//! ここが正で、SVG出力 ([`crate::svg`]) と画面 (`src/canvas/panel.ts`) が同じ結果を描く。

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::geometry::Point;
use crate::model::{DinRail, Duct, Entity, Footprint, PanelOutline, Project, Sheet, SheetId};
use crate::verify::{Diagnostic, Severity};

/// 盤の外周から取付板までの距離 (mm)。
pub const PLATE_INSET: f64 = 10.0;
/// DINレール (TS35相当) の見かけの高さ (mm)。[`DinRail::y`] はこの帯の中心線。
pub const DIN_RAIL_H: f64 = 35.0;
/// ダクトのハッチ (45°斜線) の間隔 (mm、x方向)。斜線どうしの直交距離はこの 1/√2 倍。
/// 用紙座標に固定した間隔なので、隣り合うダクトでも斜線がそろう。
pub const DUCT_HATCH_PITCH: f64 = 5.0;
/// 部品DBにフットプリント寸法が無い型番を置くときの既定の外形 (幅, 高さ) mm。
/// 置いてから属性で直せる。
pub const DEFAULT_FOOTPRINT_SIZE: (f64, f64) = (20.0, 40.0);
/// レールへ吸い付く距離 (mm)。この範囲にレールがあれば取付線に合わせる。
pub const RAIL_SNAP_TOLERANCE: f64 = DIN_RAIL_H / 2.0;
/// つぶれた図形を弾くための許容誤差 (mm)。
const EPS: f64 = 1e-9;

/// 回路図に無い参照記号のフットプリントの診断コード。
pub const ORPHAN_FOOTPRINT: &str = "panel.orphan_footprint";

/// フットプリントが盤面で占める寸法 (幅, 高さ) mm。90/270度回転で幅と高さが入れ替わる。
pub fn footprint_size(fp: &Footprint) -> (f64, f64) {
    match fp.rotation % 180 {
        90 => (fp.h, fp.w),
        _ => (fp.w, fp.h),
    }
}

/// フットプリントの矩形 (左上, 右下)。
pub fn footprint_rect(fp: &Footprint) -> (Point, Point) {
    let (w, h) = footprint_size(fp);
    (fp.at, Point::new(fp.at.x + w, fp.at.y + h))
}

/// 盤の外形の矩形 (左上, 右下)。
pub fn outline_rect(o: &PanelOutline) -> (Point, Point) {
    (o.at, Point::new(o.at.x + o.w, o.at.y + o.h))
}

/// 取付板の矩形 (左上, 右下)。外周から [`PLATE_INSET`] だけ内側。
/// 外形が小さすぎて板が残らないときはNone (外周だけを描く)。
pub fn plate_rect(o: &PanelOutline) -> Option<(Point, Point)> {
    let w = o.w - PLATE_INSET * 2.0;
    let h = o.h - PLATE_INSET * 2.0;
    if w <= EPS || h <= EPS {
        return None;
    }
    let min = Point::new(o.at.x + PLATE_INSET, o.at.y + PLATE_INSET);
    Some((min, Point::new(min.x + w, min.y + h)))
}

/// DINレールの帯の矩形 (左上, 右下)。取付線 (`y`) を中心とした高さ [`DIN_RAIL_H`] の帯。
/// 左右はどちら向きに引かれていても同じ範囲になる。
pub fn rail_rect(r: &DinRail) -> (Point, Point) {
    let (x0, x1) = (r.x1.min(r.x2), r.x1.max(r.x2));
    (
        Point::new(x0, r.y - DIN_RAIL_H / 2.0),
        Point::new(x1, r.y + DIN_RAIL_H / 2.0),
    )
}

/// ダクトを埋める45°の斜線 (始点, 終点) の並び。ダクトの内側だけを返す。
///
/// 線は `y = x + b` (b は [`DUCT_HATCH_PITCH`] の倍数) で、用紙座標に固定してあるので
/// ダクトを動かしても斜線の向き・間隔は変わらない。
pub fn duct_hatch(at: Point, w: f64, h: f64) -> Vec<(Point, Point)> {
    if w <= EPS || h <= EPS {
        return Vec::new();
    }
    let (x0, y0) = (at.x, at.y);
    let (x1, y1) = (at.x + w, at.y + h);
    let k0 = ((y0 - x1) / DUCT_HATCH_PITCH).ceil() as i64;
    let k1 = ((y1 - x0) / DUCT_HATCH_PITCH).floor() as i64;
    let mut out = Vec::new();
    for k in k0..=k1 {
        let b = k as f64 * DUCT_HATCH_PITCH;
        let xs = x0.max(y0 - b);
        let xe = x1.min(y1 - b);
        if xe - xs <= EPS {
            continue;
        }
        out.push((Point::new(xs, xs + b), Point::new(xe, xe + b)));
    }
    out
}

/// シート上のDINレール (id順)。
pub fn rails(sheet: &Sheet) -> impl Iterator<Item = &DinRail> {
    sheet.entities.values().filter_map(|e| match e {
        Entity::DinRail(r) => Some(r),
        _ => None,
    })
}

/// シート上のダクト (id順)。
pub fn ducts(sheet: &Sheet) -> impl Iterator<Item = &Duct> {
    sheet.entities.values().filter_map(|e| match e {
        Entity::Duct(d) => Some(d),
        _ => None,
    })
}

/// シート上のフットプリント (id順)。
pub fn footprints(sheet: &Sheet) -> impl Iterator<Item = &Footprint> {
    sheet.entities.values().filter_map(|e| match e {
        Entity::Footprint(f) => Some(f),
        _ => None,
    })
}

/// 部品を落とした高さ`y`に一番近いレールの取付線。近くにレールが無ければNone。
pub fn rail_snap_y(sheet: &Sheet, y: f64) -> Option<f64> {
    rails(sheet)
        .map(|r| r.y)
        .filter(|ry| (ry - y).abs() <= RAIL_SNAP_TOLERANCE)
        .min_by(|a, b| {
            (a - y)
                .abs()
                .partial_cmp(&(b - y).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

/// 型番 → フットプリント寸法 (幅, 高さ) mm の対応表。部品DBから作る
/// ([`crate::parts::PartsDb::footprint_sizes`])。
pub type FootprintSizes = BTreeMap<String, (f64, f64)>;

/// 回路図にあって盤シートにまだ置かれていない部品 (未配置部品パネルの1行)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UnplacedComponent {
    /// 参照記号 (例 "K1")。これが盤のフットプリントとの結び付きになる。
    pub reference: String,
    /// 型番 (回路図のシンボルの値)。空なら未指定。
    pub part_no: String,
    /// 代表シンボルのid (一覧のアイコン用)。
    pub symbol_id: String,
    /// その部品が最初に出てくる回路図シート。
    pub sheet_id: SheetId,
    /// 置くときの外形の幅 mm (部品DBの寸法。無ければ [`DEFAULT_FOOTPRINT_SIZE`])。
    pub w: f64,
    /// 置くときの外形の高さ mm。
    pub h: f64,
}

/// 回路図に出てくる参照記号 → (型番, シンボルid, 最初のシート)。
/// 同じ参照記号が何度出てきても1台として数え、型番は最初に見つかった空でない値を採る
/// (リレーのコイルと接点のように、1台の部品が図面に何度も現れるため)。
fn schematic_devices(project: &Project) -> BTreeMap<String, (String, String, SheetId)> {
    let mut devices: BTreeMap<String, (String, String, SheetId)> = BTreeMap::new();
    for sheet in project.sheets.iter().filter(|s| !s.is_panel()) {
        for entity in sheet.entities.values() {
            let Entity::Symbol(s) = entity else { continue };
            let reference = s.reference.trim();
            if reference.is_empty() {
                continue;
            }
            let value = s.value.trim().to_string();
            match devices.get_mut(reference) {
                Some(found) => {
                    if found.0.is_empty() {
                        found.0 = value;
                    }
                }
                None => {
                    devices.insert(
                        reference.to_string(),
                        (value, s.symbol_id.clone(), sheet.id),
                    );
                }
            }
        }
    }
    devices
}

/// 盤シートに置かれているフットプリントの参照記号。
fn placed_references(project: &Project) -> BTreeSet<String> {
    project
        .sheets
        .iter()
        .filter(|s| s.is_panel())
        .flat_map(footprints)
        .map(|f| f.reference.trim().to_string())
        .filter(|r| !r.is_empty())
        .collect()
}

/// 未配置部品 = 回路図にあって、どの盤シートにも同じ参照記号のフットプリントが無い部品。
///
/// 盤シートが1枚も無ければ回路図の全部品が返る。参照記号の付いていないシンボルは
/// 「盤へ置くものが決まっていない」ので数えない。並び順は参照記号順。
pub fn unplaced_components(project: &Project, sizes: &FootprintSizes) -> Vec<UnplacedComponent> {
    let placed = placed_references(project);
    schematic_devices(project)
        .into_iter()
        .filter(|(reference, _)| !placed.contains(reference))
        .map(|(reference, (part_no, symbol_id, sheet_id))| {
            let (w, h) = sizes
                .get(&part_no)
                .copied()
                .unwrap_or(DEFAULT_FOOTPRINT_SIZE);
            UnplacedComponent {
                reference,
                part_no,
                symbol_id,
                sheet_id,
                w,
                h,
            }
        })
        .collect()
}

/// 盤レイアウトの検証 (M4仕様 §8)。
///
/// 盤シートには配線が無いので回路図のERCは行わない。代わりに、**回路図のどこにも無い
/// 参照記号のフットプリント**を注意として報告する (参照記号を変えた・部品を消したときに
/// 盤へ置き去りになるため)。参照記号を結び付けていないフットプリント (盤だけの部品) は
/// 報告しない。
pub fn panel_diagnostics(project: &Project) -> Vec<Diagnostic> {
    let known: BTreeSet<String> = schematic_devices(project).into_keys().collect();
    let mut diags = Vec::new();
    for sheet in project.sheets.iter().filter(|s| s.is_panel()) {
        for fp in footprints(sheet) {
            let reference = fp.reference.trim();
            if reference.is_empty() || known.contains(reference) {
                continue;
            }
            diags.push(Diagnostic {
                severity: Severity::Warning,
                code: ORPHAN_FOOTPRINT.into(),
                message: format!(
                    "盤に置かれている {reference} が回路図にありません (参照記号を直すか、盤から取り除いてください)"
                ),
                sheet_id: sheet.id,
                entity_ids: vec![fp.id],
            });
        }
    }
    diags
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::*;
    use crate::verify::Severity;
    use uuid::Uuid;

    fn footprint(reference: &str, at: (f64, f64), size: (f64, f64), rotation: u16) -> Entity {
        Entity::Footprint(Footprint {
            id: Uuid::new_v4(),
            at: Point::new(at.0, at.1),
            w: size.0,
            h: size.1,
            rotation,
            reference: reference.into(),
            part_no: String::new(),
        })
    }

    fn symbol(reference: &str, part_no: &str) -> Entity {
        Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "relay_coil".into(),
            at: Point::new(50.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: reference.into(),
            value: part_no.into(),
            attrs: Default::default(),
        })
    }

    /// 回路図1枚+盤1枚のプロジェクトを作る (盤シートは2枚目)。
    fn project_with(schematic: Vec<Entity>, panel: Vec<Entity>) -> Project {
        let mut project = Project::new("panel-test");
        for e in schematic {
            project.sheets[0].entities.insert(e.id(), e);
        }
        let mut sheet = Sheet::new_kind("盤", PaperSize::A3, Orientation::Landscape, SheetKind::Panel);
        for e in panel {
            sheet.entities.insert(e.id(), e);
        }
        project.sheets.push(sheet);
        project
    }

    /// A footprint occupies the rectangle of its own size, and turning it 90 degrees swaps width and height.
    /// フットプリントは自分の寸法どおりの矩形を占め、90度回すと幅と高さが入れ替わる。
    #[test]
    fn a_rotated_footprint_swaps_width_and_height() {
        let Entity::Footprint(upright) = footprint("K1", (50.0, 50.0), (25.0, 65.0), 0) else {
            panic!()
        };
        assert_eq!(footprint_size(&upright), (25.0, 65.0));
        let (min, max) = footprint_rect(&upright);
        assert_eq!((min, max), (Point::new(50.0, 50.0), Point::new(75.0, 115.0)));

        let Entity::Footprint(turned) = footprint("K1", (50.0, 50.0), (25.0, 65.0), 90) else {
            panic!()
        };
        assert_eq!(footprint_size(&turned), (65.0, 25.0));
        let (_, max) = footprint_rect(&turned);
        assert_eq!(max, Point::new(115.0, 75.0));
    }

    /// The panel outline draws the enclosure and, inset inside it, the mounting plate that parts are screwed onto.
    /// 盤外形は筐体と、その内側に一回り小さい取付板(部品を留める板)を描く。
    #[test]
    fn the_panel_outline_carries_a_mounting_plate_inside_it() {
        let outline = PanelOutline {
            id: Uuid::new_v4(),
            at: Point::new(40.0, 40.0),
            w: 200.0,
            h: 150.0,
            name: "制御盤".into(),
        };
        let (min, max) = plate_rect(&outline).expect("取付板が描ける");
        assert_eq!(min, Point::new(40.0 + PLATE_INSET, 40.0 + PLATE_INSET));
        assert_eq!(max, Point::new(240.0 - PLATE_INSET, 190.0 - PLATE_INSET));
    }

    /// A panel outline smaller than the inset on both sides has no mounting plate (there is nothing left to draw).
    /// インセットより小さい盤外形には取付板が無い(描く余地が残らない)。
    #[test]
    fn a_tiny_panel_outline_has_no_mounting_plate() {
        let outline = PanelOutline {
            id: Uuid::new_v4(),
            at: Point::new(0.0, 0.0),
            w: PLATE_INSET,
            h: PLATE_INSET,
            name: String::new(),
        };
        assert!(plate_rect(&outline).is_none());
    }

    /// A DIN rail is drawn as a horizontal band centred on its mounting line, so parts snapped to the line sit on the rail.
    /// DINレールは取付線を中心とした横帯として描かれるので、取付線に合わせた部品はレールに乗る。
    #[test]
    fn a_din_rail_is_a_band_centred_on_its_mounting_line() {
        let rail = DinRail {
            id: Uuid::new_v4(),
            y: 100.0,
            x1: 60.0,
            x2: 220.0,
        };
        let (min, max) = rail_rect(&rail);
        assert_eq!(min, Point::new(60.0, 100.0 - DIN_RAIL_H / 2.0));
        assert_eq!(max, Point::new(220.0, 100.0 + DIN_RAIL_H / 2.0));
    }

    /// A rail drawn from right to left covers the same span as one drawn from left to right.
    /// 右から左へ引いたレールも、左から右へ引いたものと同じ範囲を覆う。
    #[test]
    fn a_rail_drawn_backwards_covers_the_same_span() {
        let rail = DinRail {
            id: Uuid::new_v4(),
            y: 100.0,
            x1: 220.0,
            x2: 60.0,
        };
        let (min, max) = rail_rect(&rail);
        assert_eq!(min.x, 60.0);
        assert_eq!(max.x, 220.0);
    }

    /// A duct is filled with 45-degree hatch lines that stay inside it, spaced on a fixed pitch so neighbouring ducts line up.
    /// ダクトは45度のハッチ(斜線)で埋まる。斜線はダクトの内側に収まり、一定間隔なので隣り合うダクトでも線がそろう。
    #[test]
    fn a_duct_is_filled_with_45_degree_hatch_lines() {
        let lines = duct_hatch(Point::new(100.0, 100.0), 20.0, 20.0);
        assert!(!lines.is_empty());
        for (a, b) in &lines {
            assert!((b.x - a.x - (b.y - a.y)).abs() < 1e-9, "45度である");
            for p in [a, b] {
                assert!((100.0..=120.0).contains(&p.x), "{p:?}");
                assert!((100.0..=120.0).contains(&p.y), "{p:?}");
            }
        }
        // 位置に依らず一定間隔なので、対角線 (100,100)-(120,120) がそのまま1本入る
        assert!(lines.contains(&(Point::new(100.0, 100.0), Point::new(120.0, 120.0))));
    }

    /// A duct with no width or height has nothing to hatch.
    /// 幅や高さの無いダクトには描く斜線が無い。
    #[test]
    fn a_flat_duct_has_no_hatch_lines() {
        assert!(duct_hatch(Point::new(0.0, 0.0), 0.0, 20.0).is_empty());
        assert!(duct_hatch(Point::new(0.0, 0.0), 20.0, 0.0).is_empty());
    }

    /// Every part on the schematic that has no footprint on a panel sheet yet is listed as unplaced, with its part number and outline size.
    /// 回路図にあって盤シートにまだフットプリントが無い部品は、型番と外形寸法つきで「未配置」として並ぶ。
    #[test]
    fn parts_without_a_footprint_are_listed_as_unplaced() {
        let project = project_with(
            vec![symbol("K1", "MDK-RLY-24V"), symbol("K2", "MDK-RLY-24V")],
            vec![footprint("K1", (80.0, 80.0), (25.0, 65.0), 0)],
        );
        let mut sizes = FootprintSizes::new();
        sizes.insert("MDK-RLY-24V".into(), (25.0, 65.0));
        let unplaced = unplaced_components(&project, &sizes);
        assert_eq!(unplaced.len(), 1, "{unplaced:?}");
        assert_eq!(unplaced[0].reference, "K2");
        assert_eq!(unplaced[0].part_no, "MDK-RLY-24V");
        assert_eq!((unplaced[0].w, unplaced[0].h), (25.0, 65.0));
    }

    /// A part whose type number has no size in the parts database is still placeable: it comes with the default rectangle, which can be adjusted afterwards.
    /// 型番の寸法が部品DBに無い部品も配置できる。既定の矩形の大きさで並び、置いたあとで直せる。
    #[test]
    fn an_unknown_type_number_falls_back_to_the_default_footprint_size() {
        let project = project_with(vec![symbol("K1", "NOT-IN-DB")], vec![]);
        let unplaced = unplaced_components(&project, &FootprintSizes::new());
        assert_eq!(unplaced.len(), 1);
        assert_eq!(
            (unplaced[0].w, unplaced[0].h),
            DEFAULT_FOOTPRINT_SIZE,
            "既定の矩形で置ける"
        );
    }

    /// With no panel sheet in the project, every part of the schematic is unplaced.
    /// 盤シートがまだ1枚も無いプロジェクトでは、回路図の部品は全て未配置になる。
    #[test]
    fn without_a_panel_sheet_every_part_is_unplaced() {
        let mut project = Project::new("no-panel");
        for e in [symbol("K1", "A"), symbol("K2", "B")] {
            project.sheets[0].entities.insert(e.id(), e);
        }
        let unplaced = unplaced_components(&project, &FootprintSizes::new());
        assert_eq!(unplaced.len(), 2);
    }

    /// One device is listed once even when it appears on the schematic several times (a relay coil and its contacts share one reference).
    /// 同じ部品が回路図に何度も出てきても未配置の一覧には1件だけ載る(リレーのコイルと接点は同じ参照記号の1台)。
    #[test]
    fn a_device_drawn_several_times_is_listed_once() {
        let mut coil = symbol("K1", "MDK-RLY-24V");
        if let Entity::Symbol(s) = &mut coil {
            s.symbol_id = "relay_coil".into();
        }
        let mut contact = symbol("K1", "");
        if let Entity::Symbol(s) = &mut contact {
            s.symbol_id = "contact_no".into();
        }
        let project = project_with(vec![coil, contact], vec![]);
        let unplaced = unplaced_components(&project, &FootprintSizes::new());
        assert_eq!(unplaced.len(), 1);
        assert_eq!(unplaced[0].reference, "K1");
        assert_eq!(unplaced[0].part_no, "MDK-RLY-24V", "型番の入っている方を採る");
    }

    /// Symbols with no reference designator yet (nothing to place on the panel) are left out of the unplaced list.
    /// まだ参照記号が付いていないシンボル(盤へ置くものが決まっていない)は未配置の一覧に出ない。
    #[test]
    fn symbols_without_a_reference_are_not_listed() {
        let project = project_with(vec![symbol("", "MDK-RLY-24V")], vec![]);
        assert!(unplaced_components(&project, &FootprintSizes::new()).is_empty());
    }

    /// A footprint whose reference does not exist on the schematic is reported as an orphan, so a renamed or deleted part is not forgotten on the panel.
    /// 回路図に無い参照記号のフットプリントは「みなしご」として報告される(参照記号を変えた・部品を消したときに盤へ置き去りにしないため)。
    #[test]
    fn a_footprint_with_no_matching_part_is_reported() {
        let project = project_with(
            vec![symbol("K1", "MDK-RLY-24V")],
            vec![
                footprint("K1", (80.0, 80.0), (25.0, 65.0), 0),
                footprint("K9", (120.0, 80.0), (25.0, 65.0), 0),
            ],
        );
        let diags = panel_diagnostics(&project);
        assert_eq!(diags.len(), 1, "{diags:?}");
        assert_eq!(diags[0].code, "panel.orphan_footprint");
        assert_eq!(diags[0].severity, Severity::Warning);
        assert!(diags[0].message.contains("K9"));
    }

    /// A footprint that is not linked to a reference designator (a panel-only part) is not reported as an orphan.
    /// 参照記号を結び付けていないフットプリント(盤だけの部品)はみなしごとして報告しない。
    #[test]
    fn a_footprint_without_a_reference_is_not_reported() {
        let project = project_with(
            vec![symbol("K1", "MDK-RLY-24V")],
            vec![footprint("", (80.0, 80.0), (25.0, 65.0), 0)],
        );
        assert!(panel_diagnostics(&project).is_empty());
    }
}
