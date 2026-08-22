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
use crate::model::{
    DinRail, Duct, Entity, EntityId, Footprint, PanelLegend, PanelOutline, Project, Sheet, SheetId,
};
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

/// バルーン (アイテム番号) の円の半径 (mm)。
pub const BALLOON_R: f64 = 3.5;
/// バルーンの円とフットプリントの角を結ぶ引出線の長さ (mm)。
pub const BALLOON_LEADER: f64 = 3.0;
/// バルーンに入れる番号の文字高さ (mm)。
pub const BALLOON_FONT: f64 = 3.0;

/// 凡例表の列見出し ([`legend_rows`] と同じ並び)。
pub const LEGEND_COLUMNS: [&str; 3] = ["No", "参照記号", "型番"];
/// 凡例表の列幅 (mm)。合計が表の幅。
pub const LEGEND_COL_W: [f64; 3] = [12.0, 22.0, 46.0];
/// 凡例表の行の高さ (mm)。列見出しの行も同じ高さ。
pub const LEGEND_ROW_H: f64 = 5.0;
/// 凡例表の文字高さ (mm)。
pub const LEGEND_FONT: f64 = 2.5;

/// 盤内部品配置表 ([`panel_placement_rows`]) の列見出し。
pub const PANEL_PLACEMENT_COLUMNS: [&str; 6] =
    ["アイテム番号", "参照記号", "型番", "X", "Y", "レール"];

/// 回路図に無い参照記号のフットプリントの診断コード。
pub const ORPHAN_FOOTPRINT: &str = "panel.orphan_footprint";
/// フットプリントどうしが重なっているときの診断コード。
pub const OVERLAP: &str = "panel.overlap";
/// どのDINレールにも乗っていない部品 (吊り配置) の診断コード。
pub const OFF_RAIL: &str = "panel.off_rail";
/// DINレールの充填率の診断コード。
pub const RAIL_FILL: &str = "panel.rail_fill";
/// 充填率がこれを超えたレールは注意にする (0.0〜1.0)。
pub const RAIL_FILL_WARN: f64 = 0.8;

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

/// シート上の凡例表 (id順)。
pub fn legends(sheet: &Sheet) -> impl Iterator<Item = &PanelLegend> {
    sheet.entities.values().filter_map(|e| match e {
        Entity::PanelLegend(l) => Some(l),
        _ => None,
    })
}

/// フットプリントの中心 (用紙座標mm)。回転後の外形の中心。
fn footprint_center(fp: &Footprint) -> Point {
    let (w, h) = footprint_size(fp);
    Point::new(fp.at.x + w / 2.0, fp.at.y + h / 2.0)
}

/// 盤シートのフットプリントを**読み順** (上の段から下の段へ、段の中は左から右へ) に並べる。
///
/// 段は**部品の中心の高さ**で分ける。同じDINレールに吸い付いた部品は中心の高さが完全に
/// 一致するので必ず同じ段になり、レールに乗っていない部品も段の先頭から
/// [`RAIL_SNAP_TOLERANCE`] の範囲にあれば同じ段として数える (背の高い部品と低い部品を
/// 並べても、外形の上端の違いで順番が入れ替わらない)。
pub fn item_order(sheet: &Sheet) -> Vec<&Footprint> {
    let mut all: Vec<&Footprint> = footprints(sheet).collect();
    all.sort_by(|a, b| {
        let (ca, cb) = (footprint_center(a), footprint_center(b));
        ca.y.partial_cmp(&cb.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(ca.x.partial_cmp(&cb.x).unwrap_or(std::cmp::Ordering::Equal))
            .then(a.id.cmp(&b.id))
    });
    // 段 (中心の高さが近いものの集まり) ごとに切り出し、段の中は左から右へ並べ直す
    let mut rows: Vec<Vec<&Footprint>> = Vec::new();
    let mut anchor = 0.0;
    for fp in all {
        let cy = footprint_center(fp).y;
        match rows.last_mut() {
            Some(row) if (cy - anchor).abs() <= RAIL_SNAP_TOLERANCE => row.push(fp),
            _ => {
                anchor = cy;
                rows.push(vec![fp]);
            }
        }
    }
    rows.into_iter()
        .flat_map(|mut row| {
            row.sort_by(|a, b| {
                footprint_center(a)
                    .x
                    .partial_cmp(&footprint_center(b).x)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.id.cmp(&b.id))
            });
            row
        })
        .collect()
}

/// 盤シートの採番計画: 読み順に 1..N のアイテム番号を割り当てる
/// ([`crate::command::Command::RenumberPanelItems`] が使う)。
pub fn renumber_plan(sheet: &Sheet) -> Vec<(EntityId, Option<u32>)> {
    item_order(sheet)
        .into_iter()
        .enumerate()
        .map(|(i, fp)| (fp.id, Some(i as u32 + 1)))
        .collect()
}

/// バルーン1つ分の幾何 (白丸の中心・引出線の両端)。
#[derive(Debug, Clone, PartialEq)]
pub struct Balloon {
    /// 丸の中心 (用紙座標mm)。
    pub center: Point,
    /// 引出線の始点 (丸の周上)。
    pub leader_from: Point,
    /// 引出線の終点 (フットプリントの左上角)。
    pub leader_to: Point,
    /// 丸の中に書く番号。
    pub item_no: u32,
}

/// フットプリントのバルーン。**アイテム番号が無ければNone** (採番するまで描かない)。
///
/// 丸はフットプリントの左上角から45度外側 (左上) へ逃がし、角とは
/// [`BALLOON_LEADER`] の長さの引出線で結ぶ。部品の外形の上には重ならない。
pub fn balloon(fp: &Footprint) -> Option<Balloon> {
    let item_no = fp.item_no?;
    let corner = fp.at;
    let unit = std::f64::consts::FRAC_1_SQRT_2;
    let at = |d: f64| Point::new(corner.x - unit * d, corner.y - unit * d);
    Some(Balloon {
        center: at(BALLOON_LEADER + BALLOON_R),
        leader_from: at(BALLOON_LEADER),
        leader_to: corner,
        item_no,
    })
}

/// 参照記号 → アイテム番号。盤シートに置かれたフットプリントから作る
/// (同じ参照記号が複数の盤にあるときは一番小さい番号)。部品表との連動に使う。
pub fn item_numbers(project: &Project) -> BTreeMap<String, u32> {
    let mut map: BTreeMap<String, u32> = BTreeMap::new();
    for sheet in project.sheets.iter().filter(|s| s.is_panel()) {
        for fp in footprints(sheet) {
            let (Some(no), reference) = (fp.item_no, fp.reference.trim()) else {
                continue;
            };
            if reference.is_empty() {
                continue;
            }
            map.entry(reference.to_string())
                .and_modify(|cur| *cur = (*cur).min(no))
                .or_insert(no);
        }
    }
    map
}

/// 凡例表に並ぶ行 ([`LEGEND_COLUMNS`] と同じ並び)。
///
/// そのシートのフットプリントを**アイテム番号順**に並べる。まだ採番していない部品も
/// 番号欄を空けたまま末尾に読み順で並ぶ (盤の上の部品が黙って抜け落ちないため)。
pub fn legend_rows(sheet: &Sheet) -> Vec<Vec<String>> {
    let ordered = item_order(sheet);
    let mut numbered: Vec<&Footprint> = ordered.iter().copied().filter(|f| f.item_no.is_some()).collect();
    numbered.sort_by_key(|f| f.item_no.unwrap_or(u32::MAX));
    let rest = ordered.iter().copied().filter(|f| f.item_no.is_none());
    numbered
        .into_iter()
        .chain(rest)
        .map(|fp| {
            vec![
                fp.item_no.map(|n| n.to_string()).unwrap_or_default(),
                fp.reference.trim().to_string(),
                fp.part_no.trim().to_string(),
            ]
        })
        .collect()
}

/// 凡例表の矩形 (左上, 右下)。列見出しの1行＋本文の行数ぶんだけ下へ伸びる。
pub fn legend_rect(sheet: &Sheet, legend: &PanelLegend) -> (Point, Point) {
    let w: f64 = LEGEND_COL_W.iter().sum();
    let h = LEGEND_ROW_H * (1 + legend_rows(sheet).len()) as f64;
    (
        legend.at,
        Point::new(legend.at.x + w, legend.at.y + h),
    )
}

/// 盤の上から順に番号を振ったDINレール (1始まり)。同じ高さなら左のレールが先。
/// この番号が配置表の「レール」欄・プロパティの「所属レール」に出る。
pub fn numbered_rails(sheet: &Sheet) -> Vec<(usize, &DinRail)> {
    let mut all: Vec<&DinRail> = rails(sheet).collect();
    all.sort_by(|a, b| {
        a.y.partial_cmp(&b.y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(
                a.x1
                    .min(a.x2)
                    .partial_cmp(&b.x1.min(b.x2))
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(a.id.cmp(&b.id))
    });
    all.into_iter().enumerate().map(|(i, r)| (i + 1, r)).collect()
}

/// 部品が乗っているDINレールと、その番号 ([`numbered_rails`] の番号)。
/// 取付線が外形を縦に横切っていて、左右の範囲も重なっているものを採る。
/// 複数あれば部品の中心に一番近いもの。乗っていなければNone。
pub fn rail_of_footprint<'a>(sheet: &'a Sheet, fp: &Footprint) -> Option<(usize, &'a DinRail)> {
    let (w, h) = footprint_size(fp);
    let (top, bottom) = (fp.at.y, fp.at.y + h);
    let (left, right) = (fp.at.x, fp.at.x + w);
    let center = fp.at.y + h / 2.0;
    let mut best: Option<(usize, &DinRail)> = None;
    for (index, rail) in numbered_rails(sheet) {
        if rail.y < top || rail.y > bottom {
            continue;
        }
        if rail.x1.max(rail.x2) < left || rail.x1.min(rail.x2) > right {
            continue;
        }
        if best.is_none_or(|(_, cur)| (rail.y - center).abs() < (cur.y - center).abs()) {
            best = Some((index, rail));
        }
    }
    best
}

/// レールの充填率 (0.0〜1.0)。そのレールに乗っている部品の幅の合計 ÷ レールの長さ。
/// 長さの無いレールは0を返す。
pub fn rail_fill_ratio(sheet: &Sheet, index: usize, rail: &DinRail) -> f64 {
    let length = (rail.x2 - rail.x1).abs();
    if length <= EPS {
        return 0.0;
    }
    let used: f64 = footprints(sheet)
        .filter(|fp| rail_of_footprint(sheet, fp).is_some_and(|(i, _)| i == index))
        .map(|fp| footprint_size(fp).0)
        .sum();
    used / length
}

/// 座標をCSV・帳票向けに整形する (整数なら小数点を出さない)。0mmも「0」と書く。
fn fmt_mm(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v}")
    }
}

/// 盤内部品配置表の行 ([`PANEL_PLACEMENT_COLUMNS`] と同じ並び)。
///
/// 盤シートごとに、そのシートの読み順 ([`item_order`]) で並ぶ。X/Yは**描かれる外形の
/// 左上角** (モデルの`at`と同じ) で、レール欄は乗っているレールの番号 (乗っていなければ空欄)。
pub fn panel_placement_rows(project: &Project) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for sheet in project.sheets.iter().filter(|s| s.is_panel()) {
        for fp in item_order(sheet) {
            rows.push(vec![
                fp.item_no.map(|n| n.to_string()).unwrap_or_default(),
                fp.reference.trim().to_string(),
                fp.part_no.trim().to_string(),
                fmt_mm(fp.at.x),
                fmt_mm(fp.at.y),
                rail_of_footprint(sheet, fp)
                    .map(|(i, _)| format!("レール{i}"))
                    .unwrap_or_default(),
            ]);
        }
    }
    rows
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

/// 表示用の部品名 (参照記号。付いていなければ「(参照記号なし)」)。
fn label_of(fp: &Footprint) -> String {
    let reference = fp.reference.trim();
    if reference.is_empty() {
        "(参照記号なし)".to_string()
    } else {
        reference.to_string()
    }
}

/// 2つの矩形が面で重なっているか。辺が触れているだけ (幅0の重なり) は重なりとしない。
fn rects_overlap(a: &Footprint, b: &Footprint) -> bool {
    let (amin, amax) = footprint_rect(a);
    let (bmin, bmax) = footprint_rect(b);
    amin.x < bmax.x - EPS
        && bmin.x < amax.x - EPS
        && amin.y < bmax.y - EPS
        && bmin.y < amax.y - EPS
}

/// 盤レイアウトの検証 (M4仕様 §8)。
///
/// 盤シートには配線が無いので回路図のERCは行わない。代わりに盤ならではの4つを見る:
///
/// - [`ORPHAN_FOOTPRINT`] (注意): **回路図のどこにも無い参照記号のフットプリント**
///   (参照記号を変えた・部品を消したときに盤へ置き去りになるため)。参照記号を
///   結び付けていないフットプリント (盤だけの部品) は報告しない
/// - [`OVERLAP`] (注意): フットプリントどうしが面で重なっている (両方は留められない)。
///   辺が触れているだけの並びはレールに並べた普通の姿なので報告しない
/// - [`OFF_RAIL`] (お知らせ): どのDINレールにも乗っていない部品。
///   **レールを1本も描いていない盤では報告しない** (取付板へ直付けする設計のため)
/// - [`RAIL_FILL`]: レールごとの充填率。[`RAIL_FILL_WARN`] を超えると注意になる
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

        // フットプリントの重なり (読み順の組み合わせなので出力は決定的)
        let ordered = item_order(sheet);
        for (i, a) in ordered.iter().enumerate() {
            for b in ordered.iter().skip(i + 1) {
                if !rects_overlap(a, b) {
                    continue;
                }
                diags.push(Diagnostic {
                    severity: Severity::Warning,
                    code: OVERLAP.into(),
                    message: format!(
                        "{} と {} が盤面で重なっています (どちらかをずらしてください)",
                        label_of(a),
                        label_of(b)
                    ),
                    sheet_id: sheet.id,
                    entity_ids: vec![a.id, b.id],
                });
            }
        }

        let numbered = numbered_rails(sheet);
        if !numbered.is_empty() {
            for fp in &ordered {
                if rail_of_footprint(sheet, fp).is_some() {
                    continue;
                }
                diags.push(Diagnostic {
                    severity: Severity::Info,
                    code: OFF_RAIL.into(),
                    message: format!(
                        "{} はDINレールに乗っていません (取付板へ直付けするなら問題ありません)",
                        label_of(fp)
                    ),
                    sheet_id: sheet.id,
                    entity_ids: vec![fp.id],
                });
            }
        }
        for (index, rail) in numbered {
            let ratio = rail_fill_ratio(sheet, index, rail);
            let percent = (ratio * 100.0).round() as i64;
            let over = ratio > RAIL_FILL_WARN;
            diags.push(Diagnostic {
                severity: if over { Severity::Warning } else { Severity::Info },
                code: RAIL_FILL.into(),
                message: if over {
                    format!("レール{index}: 充填率 {percent}% (部品を足す余地がほとんどありません)")
                } else {
                    format!("レール{index}: 充填率 {percent}%")
                },
                sheet_id: sheet.id,
                entity_ids: vec![rail.id],
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
            item_no: None,
        })
    }

    /// アイテム番号と型番を付けたフットプリント。
    fn numbered(reference: &str, part_no: &str, at: (f64, f64), item_no: u32) -> Entity {
        let mut e = footprint(reference, at, (20.0, 40.0), 0);
        if let Entity::Footprint(f) = &mut e {
            f.part_no = part_no.into();
            f.item_no = Some(item_no);
        }
        e
    }

    fn rail(y: f64, x1: f64, x2: f64) -> Entity {
        Entity::DinRail(DinRail {
            id: Uuid::new_v4(),
            y,
            x1,
            x2,
        })
    }

    /// 盤シート1枚だけのプロジェクト (回路図は空)。
    fn panel_sheet(entities: Vec<Entity>) -> Sheet {
        let mut sheet = Sheet::new_kind(
            "盤",
            PaperSize::A3,
            Orientation::Landscape,
            SheetKind::Panel,
        );
        for e in entities {
            sheet.entities.insert(e.id(), e);
        }
        sheet
    }

    /// 読み順に並べたフットプリントの参照記号。
    fn order_of(sheet: &Sheet) -> Vec<String> {
        item_order(sheet)
            .into_iter()
            .map(|fp| fp.reference.clone())
            .collect()
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

    // --- アイテム番号 (バルーン) ------------------------------------------------

    /// Item numbers are handed out in reading order: the upper row first, and within a row from left to right.
    /// アイテム番号は読み順(上の段から先に、段の中は左から右へ)に振られる。
    #[test]
    fn item_numbers_follow_reading_order() {
        // 上段 y=100 に K2(x=200)・K1(x=100)、下段 y=200 に K3(x=150)
        let sheet = panel_sheet(vec![
            footprint("K2", (200.0, 80.0), (20.0, 40.0), 0),
            footprint("K1", (100.0, 80.0), (20.0, 40.0), 0),
            footprint("K3", (150.0, 180.0), (20.0, 40.0), 0),
        ]);
        assert_eq!(order_of(&sheet), vec!["K1", "K2", "K3"]);
    }

    /// Parts mounted on the same DIN rail form one row even when their outlines have different heights.
    /// 同じDINレールに乗った部品は、外形の高さが違っても同じ段として左から右へ数える。
    #[test]
    fn parts_on_the_same_rail_form_one_row() {
        // レールの取付線 y=100。背の高い K1 と低い F1 は上端が違っても中心は同じ高さ
        let sheet = panel_sheet(vec![
            footprint("F1", (200.0, 90.0), (15.0, 20.0), 0),
            footprint("K1", (100.0, 67.5), (25.0, 65.0), 0),
        ]);
        assert_eq!(order_of(&sheet), vec!["K1", "F1"]);
    }

    /// Renumbering gives 1..N in reading order, so the numbers on the drawing read from the top left down.
    /// 採番は読み順で 1..N を振るので、図面のバルーンは左上から順に読める。
    #[test]
    fn renumbering_hands_out_one_to_n() {
        let sheet = panel_sheet(vec![
            footprint("K2", (200.0, 80.0), (20.0, 40.0), 0),
            footprint("K1", (100.0, 80.0), (20.0, 40.0), 0),
        ]);
        let plan = renumber_plan(&sheet);
        let numbers: Vec<(String, Option<u32>)> = plan
            .iter()
            .map(|(id, no)| {
                let Some(Entity::Footprint(fp)) = sheet.entities.get(id) else {
                    panic!()
                };
                (fp.reference.clone(), *no)
            })
            .collect();
        assert_eq!(
            numbers,
            vec![("K1".to_string(), Some(1)), ("K2".to_string(), Some(2))]
        );
    }

    /// A footprint with no item number yet has no balloon (nothing is drawn until the parts are numbered).
    /// アイテム番号がまだ無いフットプリントにはバルーンが出ない(採番するまで何も描かない)。
    #[test]
    fn an_unnumbered_footprint_has_no_balloon() {
        let Entity::Footprint(fp) = footprint("K1", (100.0, 100.0), (20.0, 40.0), 0) else {
            panic!()
        };
        assert!(balloon(&fp).is_none());
    }

    /// The balloon is a small circle just outside the top-left corner of the footprint, joined to that corner by a short leader line.
    /// バルーンはフットプリントの左上角のすぐ外の小さな円で、短い引出線で角と結ばれる。
    #[test]
    fn the_balloon_sits_outside_the_top_left_corner() {
        let Entity::Footprint(fp) = numbered("K1", "", (100.0, 100.0), 3) else {
            panic!()
        };
        let b = balloon(&fp).expect("採番済みならバルーンがある");
        assert_eq!(b.item_no, 3);
        assert_eq!(b.leader_to, Point::new(100.0, 100.0), "引出線は角へ届く");
        assert!(b.center.x < 100.0 && b.center.y < 100.0, "円は左上の外: {b:?}");
        // 引出線の長さ=BALLOON_LEADER、円の中心までは さらに半径ぶん離れる
        assert!((b.leader_from.distance_to(&b.leader_to) - BALLOON_LEADER).abs() < 1e-9);
        assert!(
            (b.center.distance_to(&b.leader_to) - (BALLOON_LEADER + BALLOON_R)).abs() < 1e-9
        );
    }

    /// Every reference designator placed on a panel carries its item number, so the parts list can show the same numbers.
    /// 盤に置いた参照記号ごとのアイテム番号が引けるので、部品表にも同じ番号を出せる。
    #[test]
    fn item_numbers_can_be_looked_up_by_reference() {
        let mut project = Project::new("t");
        project.sheets.push(panel_sheet(vec![
            numbered("K1", "MDK-RLY-24V", (100.0, 100.0), 1),
            footprint("K2", (200.0, 100.0), (20.0, 40.0), 0),
        ]));
        let map = item_numbers(&project);
        assert_eq!(map.get("K1"), Some(&1));
        assert_eq!(map.get("K2"), None, "未採番の部品は番号を持たない");
    }

    // --- 凡例表 ----------------------------------------------------------------

    /// The legend lists the numbered parts of its own sheet in item-number order, as item number, reference and part number.
    /// 凡例表は同じシートの採番済み部品を番号順に「アイテム番号・参照記号・型番」で並べる。
    #[test]
    fn the_legend_lists_the_numbered_parts_in_order() {
        let sheet = panel_sheet(vec![
            numbered("K2", "MDK-RLY-24V", (200.0, 100.0), 2),
            numbered("K1", "MDK-CB-3P", (100.0, 100.0), 1),
        ]);
        assert_eq!(
            legend_rows(&sheet),
            vec![
                vec!["1".to_string(), "K1".into(), "MDK-CB-3P".into()],
                vec!["2".to_string(), "K2".into(), "MDK-RLY-24V".into()],
            ]
        );
    }

    /// Parts that have not been numbered yet still appear at the end of the legend with an empty number column, so nothing on the panel is silently left out.
    /// まだ採番していない部品も番号欄を空けて末尾に並ぶので、盤の上の部品が黙って抜け落ちない。
    #[test]
    fn unnumbered_parts_come_last_in_the_legend() {
        let sheet = panel_sheet(vec![
            footprint("F9", (300.0, 100.0), (20.0, 40.0), 0),
            numbered("K1", "MDK-CB-3P", (100.0, 100.0), 1),
        ]);
        let rows = legend_rows(&sheet);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1][0], "", "番号欄は空");
        assert_eq!(rows[1][1], "F9");
    }

    /// The legend table grows downwards one row at a time, and its width is fixed by the column widths.
    /// 凡例表は行が増えるぶんだけ下へ伸び、幅は列幅で決まった固定値になる。
    #[test]
    fn the_legend_table_grows_with_its_rows() {
        let legend = PanelLegend {
            id: Uuid::new_v4(),
            at: Point::new(300.0, 200.0),
        };
        let empty = panel_sheet(vec![]);
        let (min, max) = legend_rect(&empty, &legend);
        assert_eq!(min, Point::new(300.0, 200.0));
        assert_eq!(max.x - min.x, LEGEND_COL_W.iter().sum::<f64>());
        assert_eq!(max.y - min.y, LEGEND_ROW_H, "行が無くても列見出しの1行");

        let one = panel_sheet(vec![numbered("K1", "", (100.0, 100.0), 1)]);
        let (_, max1) = legend_rect(&one, &legend);
        assert_eq!(max1.y - max.y, LEGEND_ROW_H);
    }

    // --- 盤内部品配置表 --------------------------------------------------------

    /// The panel placement table lists every footprint with its item number, reference, part number, position and rail.
    /// 盤内部品配置表は、フットプリントをアイテム番号・参照記号・型番・位置・レールで並べる。
    #[test]
    fn the_placement_table_lists_position_and_rail() {
        let mut project = Project::new("t");
        project.sheets.push(panel_sheet(vec![
            rail(120.0, 50.0, 400.0),
            numbered("K1", "MDK-RLY-24V", (100.0, 100.0), 1),
        ]));
        let rows = panel_placement_rows(&project);
        assert_eq!(PANEL_PLACEMENT_COLUMNS.len(), 6);
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0][0], "1");
        assert_eq!(rows[0][1], "K1");
        assert_eq!(rows[0][2], "MDK-RLY-24V");
        assert_eq!((&rows[0][3], &rows[0][4]), (&"100".to_string(), &"100".to_string()));
        assert_eq!(rows[0][5], "レール1", "取付線が外形を横切るのでレール1に乗る");
    }

    /// A part hung between the rails leaves the rail column empty in the placement table.
    /// レールに乗せずに吊った部品は、配置表のレール欄が空欄になる。
    #[test]
    fn a_part_off_the_rail_has_an_empty_rail_column() {
        let mut project = Project::new("t");
        project.sheets.push(panel_sheet(vec![
            rail(120.0, 50.0, 400.0),
            numbered("K1", "", (100.0, 300.0), 1),
        ]));
        assert_eq!(panel_placement_rows(&project)[0][5], "");
    }

    /// Rails are numbered from the top of the panel down, so "rail 1" means the same thing in the placement table and in the properties panel.
    /// レールは盤の上から順に番号を振るので、「レール1」は配置表でもプロパティでも同じレールを指す。
    #[test]
    fn rails_are_numbered_from_the_top_down() {
        let sheet = panel_sheet(vec![rail(300.0, 50.0, 400.0), rail(120.0, 50.0, 400.0)]);
        let Entity::Footprint(low) = numbered("K1", "", (100.0, 280.0), 1) else {
            panic!()
        };
        let (index, _) = rail_of_footprint(&sheet, &low).expect("下のレールに乗る");
        assert_eq!(index, 2, "上のレールが1、下が2");
    }

    // --- 盤レイアウトの検査 ----------------------------------------------------

    /// Two footprints that overlap on the mounting plate are reported, because the parts cannot both be screwed there.
    /// 取付板の上で重なっている2つのフットプリントは、両方を留められないので指摘される。
    #[test]
    fn overlapping_footprints_are_reported() {
        let mut project = Project::new("t");
        project.sheets.push(panel_sheet(vec![
            footprint("K1", (100.0, 100.0), (30.0, 40.0), 0),
            footprint("K2", (120.0, 110.0), (30.0, 40.0), 0),
        ]));
        let diags: Vec<_> = panel_diagnostics(&project)
            .into_iter()
            .filter(|d| d.code == OVERLAP)
            .collect();
        assert_eq!(diags.len(), 1, "{diags:?}");
        assert_eq!(diags[0].severity, Severity::Warning);
        assert_eq!(diags[0].entity_ids.len(), 2, "重なっている2つを選べる");
    }

    /// Footprints that only touch along an edge are not reported: parts standing side by side on a rail are normal.
    /// 辺が触れているだけのフットプリントは指摘しない(レールに並べた部品どうしは普通に接する)。
    #[test]
    fn footprints_that_only_touch_are_not_reported() {
        let mut project = Project::new("t");
        project.sheets.push(panel_sheet(vec![
            footprint("K1", (100.0, 100.0), (30.0, 40.0), 0),
            footprint("K2", (130.0, 100.0), (30.0, 40.0), 0),
        ]));
        assert!(!panel_diagnostics(&project).iter().any(|d| d.code == OVERLAP));
    }

    /// A part that sits on no DIN rail is reported as a note, so a part left hanging between the rails is noticed.
    /// どのDINレールにも乗っていない部品は「お知らせ」として出るので、レールの間に浮いた部品に気づける。
    #[test]
    fn a_part_hanging_off_the_rails_is_noted() {
        let mut project = Project::new("t");
        project.sheets.push(panel_sheet(vec![
            rail(120.0, 50.0, 400.0),
            footprint("K1", (100.0, 300.0), (20.0, 40.0), 0),
        ]));
        let diags: Vec<_> = panel_diagnostics(&project)
            .into_iter()
            .filter(|d| d.code == OFF_RAIL)
            .collect();
        assert_eq!(diags.len(), 1, "{diags:?}");
        assert_eq!(diags[0].severity, Severity::Info);
    }

    /// On a panel drawn without any DIN rail nothing is reported as hanging: everything on that panel is mounted directly on the plate.
    /// DINレールを1本も描いていない盤では吊り配置を報告しない(その盤は取付板へ直付けする設計のため)。
    #[test]
    fn a_panel_without_rails_reports_nothing_as_hanging() {
        let mut project = Project::new("t");
        project
            .sheets
            .push(panel_sheet(vec![footprint("K1", (100.0, 300.0), (20.0, 40.0), 0)]));
        assert!(!panel_diagnostics(&project).iter().any(|d| d.code == OFF_RAIL));
    }

    /// Every DIN rail reports how much of its length is taken up by parts.
    /// DINレールごとに、長さのどれだけが部品で埋まっているか(充填率)が出る。
    #[test]
    fn each_rail_reports_how_full_it_is() {
        let mut project = Project::new("t");
        project.sheets.push(panel_sheet(vec![
            rail(120.0, 100.0, 200.0), // 長さ100mm
            footprint("K1", (100.0, 100.0), (25.0, 40.0), 0),
        ]));
        let diags: Vec<_> = panel_diagnostics(&project)
            .into_iter()
            .filter(|d| d.code == RAIL_FILL)
            .collect();
        assert_eq!(diags.len(), 1, "{diags:?}");
        assert_eq!(diags[0].severity, Severity::Info);
        assert!(diags[0].message.contains("25%"), "{}", diags[0].message);
    }

    /// A rail packed beyond 80 percent is raised to a warning: there is no room left to fit or to service the parts.
    /// 充填率が80%を超えたレールは注意になる(部品を足す・触る余地が残っていないため)。
    #[test]
    fn a_rail_packed_over_eighty_percent_becomes_a_warning() {
        let mut project = Project::new("t");
        project.sheets.push(panel_sheet(vec![
            rail(120.0, 100.0, 200.0),
            footprint("K1", (100.0, 100.0), (85.0, 40.0), 0),
        ]));
        let diag = panel_diagnostics(&project)
            .into_iter()
            .find(|d| d.code == RAIL_FILL)
            .expect("充填率の指摘");
        assert_eq!(diag.severity, Severity::Warning);
        assert!(diag.message.contains("85%"), "{}", diag.message);
    }

    /// The checks only look at panel sheets, so a schematic never produces panel diagnostics.
    /// これらの検査は盤シートだけを見るので、回路図シートから盤の指摘は出ない。
    #[test]
    fn the_panel_checks_ignore_schematic_sheets() {
        let project = Project::new("t");
        assert!(panel_diagnostics(&project).is_empty());
    }
}
