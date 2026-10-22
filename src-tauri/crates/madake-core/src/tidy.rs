//! 整えメトリクス: 図面の「整い具合」を数値にする (M3 §3 の自動反復)。
//!
//! エージェントの整えループ (測る → 直す → 測り直す) の目標値。全て0が理想で、
//! 数値が減れば図面は読みやすくなっている。数え方の約束:
//!
//! - **配線交差** ([`wire_crossings`]): 線分どうしが**面で交わる**箇所。端点で出会う
//!   接続 (L字の突き合わせ・T分岐) は交差ではない。同じ配線の連続する線分 (曲がり角) も
//!   除く。同一直線上で二重に引かれた部分重なりは、直す場所1つとして1件に数える。
//!   数えるのは「交差した線分の組」で、3本が1点で交わればペア数の3件になる。
//! - **重なり** ([`label_overlaps`] / [`symbol_overlaps`]): 外接矩形が**面で重なる**組。
//!   辺や角が接しているだけ (食い込みが [`crate::netlist::CONNECT_EPS`] 以下) は数えない。
//!   文字どうし・文字とシンボル外形を[`label_overlaps`]、シンボル外形どうしを
//!   [`symbol_overlaps`]で別々に数える。シンボル自身の参照記号・型番は持ち主とは
//!   重ならない扱いにする (常に外形の上へ描かれるため)。
//! - **グリッド外** ([`off_grid_count`]): [`GRID_PITCH`] の格子に乗っていないシンボル
//!   原点・配線頂点の数。ずれた点1つにつき1件。
//!
//! 全て[`std::collections::BTreeMap`]順の走査と組の数え上げなので結果は決定的で、
//! 同じ図面を何度測っても同じ数値になる。

use serde::{Deserialize, Serialize};

use crate::geometry::Point;
use crate::model::{Entity, EntityId, Sheet, SymbolInstance};
use crate::netlist::{transform_local, CONNECT_EPS};
use crate::symbol::{Primitive, SymbolDef};

/// グリッドピッチ (mm)。シンボル原点・配線頂点はこの格子に乗せる。
pub const GRID_PITCH: f64 = 2.5;

/// 座標一致・食い込みの許容誤差 (mm)。接続判定と同じ基準。
const EPS: f64 = CONNECT_EPS;

/// 平行判定のしきい値 (正規化した外積 = 2線分のなす角のsin)。
const PARALLEL_EPS: f64 = 1e-9;

/// 半角文字の文字送り (文字高さに対する比)。
const NARROW_ADVANCE: f64 = 0.6;
/// 全角文字 (CJK等) の文字送り (文字高さに対する比)。
const WIDE_ADVANCE: f64 = 1.0;

/// 図面の整い具合。全て0が理想で、整えループはこの数値を減らすことを目標にする。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct TidyMetrics {
    /// 配線どうしが交わっている箇所の数 ([`wire_crossings`])。
    pub crossings: usize,
    /// 文字が他の文字・シンボル外形に重なっている組の数 ([`label_overlaps`])。
    pub label_overlaps: usize,
    /// シンボル外形どうしが重なっている組の数 ([`symbol_overlaps`])。
    pub symbol_overlaps: usize,
    /// 2.5mmグリッドから外れている点の数 ([`off_grid_count`])。
    pub off_grid: usize,
}

impl TidyMetrics {
    /// 4つの数値の合計。整えループの「良くなったか」の単純な指標。
    pub fn total(&self) -> usize {
        self.crossings + self.label_overlaps + self.symbol_overlaps + self.off_grid
    }
}

/// シート1枚の整えメトリクスをまとめて測る。決定的 (同じ図面なら常に同じ数値)。
pub fn tidy_metrics(sheet: &Sheet, symbols: &[SymbolDef]) -> TidyMetrics {
    TidyMetrics {
        crossings: wire_crossings(sheet),
        label_overlaps: label_overlaps(sheet, symbols),
        symbol_overlaps: symbol_overlaps(sheet, symbols),
        off_grid: off_grid_count(sheet),
    }
}

/// 軸に平行な外接矩形 (mm)。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Rect {
    pub min: Point,
    pub max: Point,
}

impl Rect {
    /// 対角の2点から作る (どちらの角からでもよい)。
    pub fn from_corners(a: Point, b: Point) -> Self {
        Self {
            min: Point::new(a.x.min(b.x), a.y.min(b.y)),
            max: Point::new(a.x.max(b.x), a.y.max(b.y)),
        }
    }

    /// 点を含むように広げる。
    pub fn extend(&mut self, p: Point) {
        self.min.x = self.min.x.min(p.x);
        self.min.y = self.min.y.min(p.y);
        self.max.x = self.max.x.max(p.x);
        self.max.y = self.max.y.max(p.y);
    }

    /// 面のある重なりがあるか。辺・角が接しているだけ (食い込みが[`EPS`]以下) は重なりとしない。
    pub fn overlaps(&self, other: &Rect) -> bool {
        self.min.x < other.max.x - EPS
            && other.min.x < self.max.x - EPS
            && self.min.y < other.max.y - EPS
            && other.min.y < self.max.y - EPS
    }
}

/// 文字列を高さ`height`で描いたときの見積もり幅 (mm)。全角は半角の約1.7倍で数える。
pub fn text_width(text: &str, height: f64) -> f64 {
    height
        * text
            .chars()
            .map(|c| if is_wide(c) { WIDE_ADVANCE } else { NARROW_ADVANCE })
            .sum::<f64>()
}

/// 全角幅で描かれる文字か (CJK・かな・全角記号)。
fn is_wide(c: char) -> bool {
    matches!(c as u32,
        0x1100..=0x115F
        | 0x2E80..=0x303E
        | 0x3041..=0x33FF
        | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xA000..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE6F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x20000..=0x3FFFD)
}

/// テキストの寄せ方 (SVGのtext-anchorと同じ)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Anchor {
    Start,
    Middle,
    End,
}

/// ベースライン位置・文字高さ・寄せ方から文字の外接矩形を作る。
fn text_box(x: f64, baseline_y: f64, height: f64, anchor: Anchor, text: &str) -> Rect {
    let w = text_width(text, height);
    let x0 = match anchor {
        Anchor::Start => x,
        Anchor::Middle => x - w / 2.0,
        Anchor::End => x - w,
    };
    Rect::from_corners(
        Point::new(x0, baseline_y - height),
        Point::new(x0 + w, baseline_y),
    )
}

/// 図面上の文字の種別。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextKind {
    /// 参照記号 (R1, K2 …)。
    Reference,
    /// 型番・値。
    Value,
    /// ネットラベル。
    NetLabel,
    /// 自由テキスト注記。
    Note,
    /// 線番の表示。
    WireNumber,
    /// ピングループの括弧のキャプション (G8)。
    PinGroupCaption,
}

/// 図面に印字される文字1つ分。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TextBox {
    pub rect: Rect,
    /// 印字される文字。
    pub text: String,
    pub kind: TextKind,
    /// 持ち主のエンティティ (参照記号・型番ならシンボル、注記なら注記自身)。
    pub owner: Option<EntityId>,
}

/// 配置後のシンボル外形の外接矩形 (用紙座標)。図形プリミティブとピンを全て含む。
/// 円・弧は中心±半径で安全側に評価する。図形の無いシンボルはNone。
pub fn symbol_bounds(inst: &SymbolInstance, def: &SymbolDef) -> Option<Rect> {
    let mut rect: Option<Rect> = None;
    let mut visit = |p: Point| {
        let t = transform_local(p, inst);
        match &mut rect {
            Some(r) => r.extend(t),
            None => rect = Some(Rect::from_corners(t, t)),
        }
    };
    for prim in &def.primitives {
        match prim {
            Primitive::Line { pts } => pts.iter().copied().for_each(&mut visit),
            Primitive::Circle { center, r, .. } | Primitive::Arc { center, r, .. } => {
                visit(Point::new(center.x - r, center.y - r));
                visit(Point::new(center.x + r, center.y + r));
            }
            Primitive::Rect { p1, p2, .. } => {
                visit(*p1);
                visit(Point::new(p2.x, p1.y));
                visit(*p2);
                visit(Point::new(p1.x, p2.y));
            }
            Primitive::Text { at, text, height } => {
                let (hw, hh) = (text_width(text, *height) / 2.0, height / 2.0);
                visit(Point::new(at.x - hw, at.y - hh));
                visit(Point::new(at.x + hw, at.y + hh));
            }
        }
    }
    for pin in &def.pins {
        visit(pin.at);
    }
    rect
}

/// シート上の全シンボルの外形矩形 (entity id付き)。id順で決定的。
pub fn symbol_boxes(sheet: &Sheet, symbols: &[SymbolDef]) -> Vec<(EntityId, Rect)> {
    let defs = crate::symbol::DefIndex::new(symbols);
    sheet
        .entities
        .values()
        .filter_map(|e| match e {
            Entity::Symbol(s) => {
                let def = defs.get(s)?;
                Some((s.id, symbol_bounds(s, &def)?))
            }
            _ => None,
        })
        .collect()
}

/// 図面に印字される全ての文字の外接矩形。位置と大きさはSVG/PDF出力と同じ規則で求める
/// (参照記号・型番はシンボル外形の上、ネットラベルは基準点の上、線番はネットの
/// 代表線分の脇)。
pub fn text_boxes(sheet: &Sheet, symbols: &[SymbolDef]) -> Vec<TextBox> {
    use crate::svg::{
        LABEL_FONT, NET_LABEL_DY, REF_LABEL_DY, VALUE_LABEL_DY, WIRE_NO_FONT, WIRE_NO_GAP,
    };
    let defs = crate::symbol::DefIndex::new(symbols);
    // 注記の層 (ネットラベル・タグ・線番) はシートの注記文字高で伸縮する (G20)
    let annotation_scale = sheet.annotation_scale();
    let mut boxes = Vec::new();
    for entity in sheet.entities.values() {
        match entity {
            Entity::Symbol(s) => {
                let Some(def) = defs.get(s) else {
                    continue;
                };
                let top = crate::svg::symbol_top_y(s, &def);
                if !s.reference.is_empty() {
                    boxes.push(TextBox {
                        rect: text_box(
                            s.at.x,
                            top - REF_LABEL_DY,
                            LABEL_FONT,
                            Anchor::Middle,
                            &s.reference,
                        ),
                        text: s.reference.clone(),
                        kind: TextKind::Reference,
                        owner: Some(s.id),
                    });
                }
                if !s.value.is_empty() {
                    boxes.push(TextBox {
                        rect: text_box(
                            s.at.x,
                            top - VALUE_LABEL_DY,
                            LABEL_FONT,
                            Anchor::Middle,
                            &s.value,
                        ),
                        text: s.value.clone(),
                        kind: TextKind::Value,
                        owner: Some(s.id),
                    });
                }
                // ピングループの括弧のキャプション (G8)。SVG出力 (render_pin_group_brackets)
                // と同じジオメトリで、行ごとに中央寄せの1文字ぶんの矩形を積む
                if let Some(spec) = s.attrs.get(crate::pin_groups::PIN_GROUPS_ATTR) {
                    let labels = crate::terminal_chart::TerminalLabels::of(&def, &s.attrs);
                    for bracket in
                        crate::pin_groups::pin_group_brackets(&def, spec, labels.display_map())
                    {
                        for (text, p) in crate::pin_groups::caption_line_anchors(s, &bracket) {
                            boxes.push(TextBox {
                                rect: text_box(
                                    p.x,
                                    p.y + crate::pin_groups::PIN_GROUP_CAPTION_TEXT_MM / 2.0,
                                    crate::pin_groups::PIN_GROUP_CAPTION_TEXT_MM,
                                    Anchor::Middle,
                                    &text,
                                ),
                                text: text.clone(),
                                kind: TextKind::PinGroupCaption,
                                owner: Some(s.id),
                            });
                        }
                    }
                }
            }
            // タグ付きラベルの文字は旗の本体の中 (基準点の左右どちらに来るかは向き次第)
            Entity::NetLabel(l) if !l.name.is_empty() => {
                let font = LABEL_FONT * annotation_scale;
                let (x, baseline) = match crate::net_tag::net_tag(l, font) {
                    Some(tag) => (tag.text_at.x, tag.text_at.y),
                    None => (l.at.x, l.at.y - NET_LABEL_DY * annotation_scale),
                };
                boxes.push(TextBox {
                    rect: text_box(x, baseline, font, Anchor::Start, &l.name),
                    text: l.name.clone(),
                    kind: TextKind::NetLabel,
                    owner: Some(l.id),
                });
            }
            Entity::Text(t) if !t.text.is_empty() => boxes.push(TextBox {
                rect: text_box(t.at.x, t.at.y, t.height, Anchor::Start, &t.text),
                text: t.text.clone(),
                kind: TextKind::Note,
                owner: Some(t.id),
            }),
            _ => {}
        }
    }
    // 線番はネットごとに代表線分の脇へ1回だけ描かれる (SVG出力と同じ位置)
    for net in crate::netlist::extract_netlist(sheet, symbols) {
        let Some(no) = net.wire_no.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
            continue;
        };
        let Some((a, b)) = crate::svg::longest_segment(sheet, &net.wire_ids) else {
            continue;
        };
        let mid = crate::svg::midpoint(&a, &b);
        let (font, gap) = (WIRE_NO_FONT * annotation_scale, WIRE_NO_GAP * annotation_scale);
        let rect = if (b.x - a.x).abs() >= (b.y - a.y).abs() {
            text_box(mid.x, mid.y - gap, font, Anchor::Middle, no)
        } else {
            text_box(mid.x - gap, mid.y, font, Anchor::End, no)
        };
        boxes.push(TextBox {
            rect,
            text: no.to_string(),
            kind: TextKind::WireNumber,
            owner: None,
        });
    }
    boxes
}

/// 文字が読めなくなっている組の数: 文字どうしの重なり + 文字とシンボル外形の重なり。
///
/// 辺で接しているだけは数えない。シンボル自身の参照記号・型番は、その持ち主の外形とは
/// 重ならない扱いにする (SVG出力では必ず外形の上へ逃がして描かれるため)。
pub fn label_overlaps(sheet: &Sheet, symbols: &[SymbolDef]) -> usize {
    label_overlap_issues(sheet, symbols).len()
}

/// [`label_overlaps`]が数える組を1件ずつ (重なりの中心・関係するエンティティ・文字つき)。
pub fn label_overlap_issues(sheet: &Sheet, symbols: &[SymbolDef]) -> Vec<TidyIssue> {
    let texts = text_boxes(sheet, symbols);
    let symbol_rects = symbol_boxes(sheet, symbols);
    let mut issues = Vec::new();
    for (i, a) in texts.iter().enumerate() {
        for b in texts.iter().skip(i + 1) {
            if a.rect.overlaps(&b.rect) {
                issues.push(TidyIssue {
                    kind: TidyIssueKind::LabelOverlap,
                    entity_ids: a.owner.into_iter().chain(b.owner).collect(),
                    texts: vec![a.text.clone(), b.text.clone()],
                    at: overlap_center(&a.rect, &b.rect),
                });
            }
        }
        for (id, rect) in &symbol_rects {
            if a.owner == Some(*id) {
                continue;
            }
            if a.rect.overlaps(rect) {
                issues.push(TidyIssue {
                    kind: TidyIssueKind::LabelOverlap,
                    entity_ids: a.owner.into_iter().chain(Some(*id)).collect(),
                    texts: vec![a.text.clone()],
                    at: overlap_center(&a.rect, rect),
                });
            }
        }
    }
    issues
}

/// シンボル外形どうしが重なっている組の数。辺で接しているだけは数えない。
pub fn symbol_overlaps(sheet: &Sheet, symbols: &[SymbolDef]) -> usize {
    symbol_overlap_issues(sheet, symbols).len()
}

/// [`symbol_overlaps`]が数える組を1件ずつ。
pub fn symbol_overlap_issues(sheet: &Sheet, symbols: &[SymbolDef]) -> Vec<TidyIssue> {
    let rects = symbol_boxes(sheet, symbols);
    let mut issues = Vec::new();
    for (i, (ia, a)) in rects.iter().enumerate() {
        for (ib, b) in rects.iter().skip(i + 1) {
            if a.overlaps(b) {
                issues.push(TidyIssue {
                    kind: TidyIssueKind::SymbolOverlap,
                    entity_ids: vec![*ia, *ib],
                    texts: Vec::new(),
                    at: overlap_center(a, b),
                });
            }
        }
    }
    issues
}

/// 2つの矩形の重なり部分の中心。
fn overlap_center(a: &Rect, b: &Rect) -> Point {
    Point::new(
        (a.min.x.max(b.min.x) + a.max.x.min(b.max.x)) / 2.0,
        (a.min.y.max(b.min.y) + a.max.y.min(b.max.y)) / 2.0,
    )
}

/// 2線分の位置関係。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SegHit {
    /// 交わらない。
    Apart,
    /// 端点で出会っているだけ (接続・T分岐・突き合わせ)。
    Touch,
    /// 面で交わっている (交差、または同一直線上の二重引き)。
    Cross,
}

fn cross2(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    ax * by - ay * bx
}

/// 2線分が交差しているか。端点で出会うだけの接続は[`SegHit::Touch`]として区別する。
fn segment_hit(a1: Point, a2: Point, b1: Point, b2: Point) -> SegHit {
    let (rx, ry) = (a2.x - a1.x, a2.y - a1.y);
    let (sx, sy) = (b2.x - b1.x, b2.y - b1.y);
    let (len_r, len_s) = (a1.distance_to(&a2), b1.distance_to(&b2));
    if len_r < EPS || len_s < EPS {
        return SegHit::Apart;
    }
    let denom = cross2(rx, ry, sx, sy);
    let (qx, qy) = (b1.x - a1.x, b1.y - a1.y);
    if denom.abs() <= PARALLEL_EPS * len_r * len_s {
        // 平行。同一直線上でなければ交わらない
        if cross2(qx, qy, rx, ry).abs() > EPS * len_r {
            return SegHit::Apart;
        }
        // 同一直線: aの向きに射影して重なり長さを見る
        let rr = rx * rx + ry * ry;
        let t0 = (qx * rx + qy * ry) / rr;
        let t1 = t0 + (sx * rx + sy * ry) / rr;
        let (lo, hi) = (t0.min(t1), t0.max(t1));
        let overlap = (hi.min(1.0) - lo.max(0.0)) * len_r;
        return if overlap > EPS {
            SegHit::Cross
        } else if overlap > -EPS {
            SegHit::Touch
        } else {
            SegHit::Apart
        };
    }
    let t = cross2(qx, qy, sx, sy) / denom;
    let u = cross2(qx, qy, rx, ry) / denom;
    let (t_tol, u_tol) = (EPS / len_r, EPS / len_s);
    if t < -t_tol || t > 1.0 + t_tol || u < -u_tol || u > 1.0 + u_tol {
        return SegHit::Apart;
    }
    let p = Point::new(a1.x + t * rx, a1.y + t * ry);
    if [a1, a2, b1, b2].iter().any(|e| p.distance_to(e) <= EPS) {
        SegHit::Touch
    } else {
        SegHit::Cross
    }
}

/// 配線どうしが交わっている箇所の数。
///
/// 端点で出会う接続 (L字の突き合わせ・T分岐) と、同じ配線の連続する線分 (曲がり角) は
/// 交差に数えない。同一直線上に二重に引かれた部分重なりは1件として数える。
pub fn wire_crossings(sheet: &Sheet) -> usize {
    crossing_issues(sheet).len()
}

/// [`wire_crossings`]が数える箇所を1件ずつ (交点と2本の配線)。
pub fn crossing_issues(sheet: &Sheet) -> Vec<TidyIssue> {
    // (配線id, 配線内の線分番号, 始点, 終点)
    let mut segments: Vec<(EntityId, usize, Point, Point)> = Vec::new();
    for entity in sheet.entities.values() {
        let Entity::Wire(w) = entity else {
            continue;
        };
        for (seg_index, seg) in w.points.windows(2).enumerate() {
            segments.push((w.id, seg_index, seg[0], seg[1]));
        }
    }
    let mut issues = Vec::new();
    for (i, a) in segments.iter().enumerate() {
        for b in segments.iter().skip(i + 1) {
            // 同じ配線の連続する線分 (曲がり角) は接続なので数えない
            if a.0 == b.0 && a.1.abs_diff(b.1) == 1 {
                continue;
            }
            if segment_hit(a.2, a.3, b.2, b.3) == SegHit::Cross {
                let ids = if a.0 == b.0 { vec![a.0] } else { vec![a.0, b.0] };
                issues.push(TidyIssue {
                    kind: TidyIssueKind::Crossing,
                    entity_ids: ids,
                    texts: Vec::new(),
                    at: crossing_point(a.2, a.3, b.2, b.3),
                });
            }
        }
    }
    issues
}

/// 交わる2線分の交点。同一直線上の二重引きは、重なっている部分の中心。
fn crossing_point(a1: Point, a2: Point, b1: Point, b2: Point) -> Point {
    let (rx, ry) = (a2.x - a1.x, a2.y - a1.y);
    let (sx, sy) = (b2.x - b1.x, b2.y - b1.y);
    let denom = cross2(rx, ry, sx, sy);
    if denom.abs() > PARALLEL_EPS * a1.distance_to(&a2) * b1.distance_to(&b2) {
        let t = cross2(b1.x - a1.x, b1.y - a1.y, sx, sy) / denom;
        return Point::new(a1.x + t * rx, a1.y + t * ry);
    }
    overlap_center(&Rect::from_corners(a1, a2), &Rect::from_corners(b1, b2))
}

/// 点が2.5mmグリッドに乗っているか (許容誤差は[`CONNECT_EPS`])。
pub fn is_on_grid(p: &Point) -> bool {
    let off = |v: f64| (v - (v / GRID_PITCH).round() * GRID_PITCH).abs() <= EPS;
    off(p.x) && off(p.y)
}

/// 2.5mmグリッドから外れている点の数 (シンボル原点・配線頂点)。ずれた点1つにつき1件。
pub fn off_grid_count(sheet: &Sheet) -> usize {
    off_grid_issues(sheet).len()
}

/// [`off_grid_count`]が数える点を1件ずつ。
pub fn off_grid_issues(sheet: &Sheet) -> Vec<TidyIssue> {
    let issue = |id: EntityId, at: Point| TidyIssue {
        kind: TidyIssueKind::OffGrid,
        entity_ids: vec![id],
        texts: Vec::new(),
        at,
    };
    let mut issues = Vec::new();
    for entity in sheet.entities.values() {
        match entity {
            Entity::Symbol(s) if !is_on_grid(&s.at) => issues.push(issue(s.id, s.at)),
            Entity::Wire(w) => issues.extend(
                w.points.iter().filter(|p| !is_on_grid(p)).map(|p| issue(w.id, *p)),
            ),
            _ => {}
        }
    }
    issues
}

/// 整えメトリクスが数えた問題の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TidyIssueKind {
    /// 配線の交差 ([`wire_crossings`])。
    Crossing,
    /// 文字の重なり ([`label_overlaps`])。
    LabelOverlap,
    /// シンボル外形の重なり ([`symbol_overlaps`])。
    SymbolOverlap,
    /// グリッド外の点 ([`off_grid_count`])。
    OffGrid,
}

/// 整えメトリクスが数えた問題1件。数値の1件ずつに対応する。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TidyIssue {
    pub kind: TidyIssueKind,
    /// 関係するエンティティ (交差なら2本の配線、重なりなら持ち主どうし)。
    pub entity_ids: Vec<EntityId>,
    /// 重なっている文字 (文字の重なりのときだけ)。
    pub texts: Vec<String>,
    /// 問題の場所 (交点・重なりの中心・ずれた点)。
    pub at: Point,
}

/// シート1枚で数えた問題を全部、種類の順 (交差・文字・シンボル・グリッド外) に並べる。
/// 種類ごとの件数は [`tidy_metrics`] の数値と一致する。
pub fn tidy_issues(sheet: &Sheet, symbols: &[SymbolDef]) -> Vec<TidyIssue> {
    let mut issues = crossing_issues(sheet);
    issues.extend(label_overlap_issues(sheet, symbols));
    issues.extend(symbol_overlap_issues(sheet, symbols));
    issues.extend(off_grid_issues(sheet));
    issues
}

/// [`TidyReport::details`] に載せる件数の上限 (応答を大きくしすぎない)。
pub const TIDY_DETAILS_LIMIT: usize = 50;

/// 4つの数値と、数えた問題の一覧 (先頭から [`TIDY_DETAILS_LIMIT`] 件)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TidyReport {
    #[serde(flatten)]
    pub metrics: TidyMetrics,
    /// 数えた問題 (種類・エンティティ・文字・場所)。種類の順。
    pub details: Vec<TidyIssue>,
    /// 上限を超えて省いた件数。
    pub details_omitted: usize,
}

/// シート1枚の [`TidyReport`]。数値は [`tidy_metrics`] と同じ。
pub fn tidy_report(sheet: &Sheet, symbols: &[SymbolDef]) -> TidyReport {
    let mut details = tidy_issues(sheet, symbols);
    let count = |k: TidyIssueKind| details.iter().filter(|i| i.kind == k).count();
    let metrics = TidyMetrics {
        crossings: count(TidyIssueKind::Crossing),
        label_overlaps: count(TidyIssueKind::LabelOverlap),
        symbol_overlaps: count(TidyIssueKind::SymbolOverlap),
        off_grid: count(TidyIssueKind::OffGrid),
    };
    let details_omitted = details.len().saturating_sub(TIDY_DETAILS_LIMIT);
    details.truncate(TIDY_DETAILS_LIMIT);
    TidyReport { metrics, details, details_omitted }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::*;
    use crate::symbol::builtin_symbols;
    use uuid::Uuid;

    fn sheet_with(entities: Vec<Entity>) -> Sheet {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        for e in entities {
            sheet.entities.insert(e.id(), e);
        }
        sheet
    }

    fn wire(points: &[(f64, f64)]) -> Entity {
        Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            color: "black".into(),
            sq: 0.75,
            length_m: None,
            length_source: Default::default(),
            part_no: None,
            net: None,
        })
    }

    fn note(at: (f64, f64), text: &str, height: f64) -> Entity {
        Entity::Text(TextEntity {
            id: Uuid::new_v4(),
            at: Point::new(at.0, at.1),
            text: text.into(),
            height,
            rotation: 0,
            anchor: crate::model::TextAnchor::Start,
            color: None,
            bold: false,
            leader_to: None,
        })
    }

    fn symbol(symbol_id: &str, at: (f64, f64), reference: &str) -> Entity {
        Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: symbol_id.into(),
            at: Point::new(at.0, at.1),
            rotation: 0,
            mirror: false,
            reference: reference.into(),
            value: String::new(),
            attrs: Default::default(),
        })
    }

    /// Two wires laid across each other in an X count as one crossing.
    /// 2本の配線がX字に横切っていると、配線交差数は1になる。
    #[test]
    fn two_wires_laid_across_each_other_count_as_one_crossing() {
        let sheet = sheet_with(vec![
            wire(&[(50.0, 50.0), (100.0, 50.0)]),
            wire(&[(75.0, 25.0), (75.0, 75.0)]),
        ]);
        assert_eq!(wire_crossings(&sheet), 1);
    }

    /// Wires that never meet have no crossings.
    /// 触れ合わない配線どうしには交差が無い。
    #[test]
    fn wires_that_never_meet_have_no_crossings() {
        let sheet = sheet_with(vec![
            wire(&[(50.0, 50.0), (100.0, 50.0)]),
            wire(&[(50.0, 70.0), (100.0, 70.0)]),
        ]);
        assert_eq!(wire_crossings(&sheet), 0);
    }

    /// Two wires joined at a shared endpoint are a connection, not a crossing.
    /// 端点を共有してつながっている2本の配線は接続であって交差ではない。
    #[test]
    fn wires_joined_at_a_shared_endpoint_are_not_a_crossing() {
        let sheet = sheet_with(vec![
            wire(&[(50.0, 50.0), (100.0, 50.0)]),
            wire(&[(100.0, 50.0), (100.0, 100.0)]),
        ]);
        assert_eq!(wire_crossings(&sheet), 0);
    }

    /// A T branch, where one wire ends on the middle of another, is a connection point and is not a crossing.
    /// 片方の端点が相手の途中に乗るT分岐は接続点であり、交差には数えない。
    #[test]
    fn a_t_branch_landing_on_another_wire_is_not_a_crossing() {
        let sheet = sheet_with(vec![
            wire(&[(50.0, 50.0), (100.0, 50.0)]),
            wire(&[(75.0, 50.0), (75.0, 100.0)]),
        ]);
        assert_eq!(wire_crossings(&sheet), 0);
    }

    /// The corner of one polyline wire is not a crossing: consecutive segments of the same wire are skipped.
    /// 1本の折れ線配線の曲がり角は交差ではない: 同じ配線の連続する線分どうしは数えない。
    #[test]
    fn the_corner_of_a_polyline_wire_is_not_a_crossing() {
        let sheet = sheet_with(vec![wire(&[(50.0, 50.0), (100.0, 50.0), (100.0, 100.0)])]);
        assert_eq!(wire_crossings(&sheet), 0);
    }

    /// A wire routed back over itself so that two of its own separate segments cross does count as a crossing.
    /// 自分自身の上を横切るように引き回した配線は、離れた線分どうしが交差するので交差に数える。
    #[test]
    fn a_wire_crossing_its_own_route_counts_as_a_crossing() {
        let sheet = sheet_with(vec![wire(&[
            (50.0, 50.0),
            (100.0, 50.0),
            (100.0, 100.0),
            (75.0, 100.0),
            (75.0, 25.0),
        ])]);
        assert_eq!(wire_crossings(&sheet), 1);
    }

    /// Two wires drawn on top of each other along part of their length count as one crossing (the overlap is one place to fix).
    /// 一部が重なって二重に引かれた配線は交差1件として数える (直すべき箇所が1つだから)。
    #[test]
    fn wires_drawn_on_top_of_each_other_count_as_one_crossing() {
        let sheet = sheet_with(vec![
            wire(&[(50.0, 50.0), (100.0, 50.0)]),
            wire(&[(75.0, 50.0), (125.0, 50.0)]),
        ]);
        assert_eq!(wire_crossings(&sheet), 1);
    }

    /// Two wires that lie on the same line but only meet end to end are a connection, not an overlap.
    /// 同一直線上で端どうしが突き合わさっているだけの配線は接続であり、重なりではない。
    #[test]
    fn wires_meeting_end_to_end_on_one_line_are_not_a_crossing() {
        let sheet = sheet_with(vec![
            wire(&[(50.0, 50.0), (100.0, 50.0)]),
            wire(&[(100.0, 50.0), (150.0, 50.0)]),
        ]);
        assert_eq!(wire_crossings(&sheet), 0);
    }

    /// Two notes printed over each other count as one label overlap.
    /// 重なって印字される2つの注記は、重なり1件として数える。
    #[test]
    fn two_notes_printed_over_each_other_overlap() {
        let sheet = sheet_with(vec![
            note((50.0, 50.0), "AAA", 2.5),
            note((51.0, 50.0), "AAA", 2.5),
        ]);
        assert_eq!(label_overlaps(&sheet, &builtin_symbols()), 1);
    }

    /// Two notes whose boxes only touch along an edge are not counted as overlapping.
    /// 外枠が辺で接しているだけの2つの注記は、重なりに数えない。
    #[test]
    fn notes_that_only_touch_along_an_edge_do_not_overlap() {
        // "AAA" 高さ2.5 の幅 = 3文字 x 0.6 x 2.5 = 4.5mm
        let sheet = sheet_with(vec![
            note((50.0, 50.0), "AAA", 2.5),
            note((54.5, 50.0), "AAA", 2.5),
        ]);
        assert_eq!(label_overlaps(&sheet, &builtin_symbols()), 0);
    }

    /// Notes placed well apart do not overlap.
    /// 十分に離して置かれた注記は重ならない。
    #[test]
    fn notes_placed_well_apart_do_not_overlap() {
        let sheet = sheet_with(vec![
            note((50.0, 50.0), "AAA", 2.5),
            note((100.0, 50.0), "AAA", 2.5),
        ]);
        assert_eq!(label_overlaps(&sheet, &builtin_symbols()), 0);
    }

    /// A note printed on top of a symbol's outline counts as a label overlap.
    /// シンボルの外形の上に印字された注記は、重なりとして数える。
    #[test]
    fn a_note_printed_over_a_symbol_outline_overlaps() {
        // 抵抗器を(100,100)に置くと外形は x 92.5..107.5 / y 98..102
        let sheet = sheet_with(vec![
            symbol("resistor", (100.0, 100.0), "R1"),
            note((95.0, 101.0), "注記", 2.5),
        ]);
        assert_eq!(label_overlaps(&sheet, &builtin_symbols()), 1);
        // シンボルは1個なのでシンボル同士の重なりは無い
        assert_eq!(symbol_overlaps(&sheet, &builtin_symbols()), 0);
    }

    /// A note that stops exactly at the top edge of a symbol is touching, not overlapping.
    /// シンボルの外形の上端でぴたりと止まる注記は、接しているだけで重なっていない。
    #[test]
    fn a_note_stopping_at_the_symbol_edge_does_not_overlap() {
        let sheet = sheet_with(vec![
            symbol("resistor", (100.0, 100.0), "R1"),
            note((95.0, 98.0), "注記", 2.5),
        ]);
        assert_eq!(label_overlaps(&sheet, &builtin_symbols()), 0);
    }

    /// Symbols placed on top of each other are counted separately from label overlaps.
    /// 重ねて置かれたシンボルどうしの重なりは、ラベルの重なりとは別に数える。
    #[test]
    fn symbols_placed_on_top_of_each_other_are_counted_separately() {
        let overlapping = sheet_with(vec![
            symbol("resistor", (100.0, 100.0), ""),
            symbol("resistor", (105.0, 100.0), ""),
        ]);
        assert_eq!(symbol_overlaps(&overlapping, &builtin_symbols()), 1);
        assert_eq!(label_overlaps(&overlapping, &builtin_symbols()), 0);

        let apart = sheet_with(vec![
            symbol("resistor", (100.0, 100.0), ""),
            symbol("resistor", (130.0, 100.0), ""),
        ]);
        assert_eq!(symbol_overlaps(&apart, &builtin_symbols()), 0);
    }

    /// A symbol's own reference designator never counts as overlapping the symbol it belongs to.
    /// シンボル自身の参照記号は、その持ち主のシンボルとの重なりには数えない。
    #[test]
    fn a_reference_designator_never_overlaps_its_own_symbol() {
        let sheet = sheet_with(vec![symbol("resistor", (100.0, 100.0), "R1")]);
        assert_eq!(label_overlaps(&sheet, &builtin_symbols()), 0);
    }

    /// A pin group's caption (G8) is one of the text boxes measured on a sheet, so a caption that overlaps another label is caught by the overlap metric.
    /// ピングループのキャプション(G8)もシートの文字として測られるので、他の文字と重なっていれば重なりの指標が拾う。
    #[test]
    fn a_pin_group_caption_is_measured_as_a_text_box() {
        let mut block = SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "terminal_block_4p".into(),
            at: Point::new(100.0, 100.0),
            rotation: 0,
            mirror: false,
            reference: "TB1".into(),
            value: String::new(),
            attrs: Default::default(),
        };
        block.attrs.insert(crate::pin_groups::PIN_GROUPS_ATTR.into(), "1-2:電源".into());
        let sheet = sheet_with(vec![Entity::Symbol(block)]);
        let defs = crate::symbol::sheet_symbol_defs(&sheet);
        let boxes = text_boxes(&sheet, &defs);
        assert!(
            boxes.iter().any(|b| b.kind == TextKind::PinGroupCaption),
            "キャプションが文字として測られる: {boxes:?}"
        );

        // 重なりの指標にも反映される: キャプションとほぼ同じ場所・大きさの注記を重ねると増える
        let before = label_overlaps(&sheet, &defs);
        let caption_box = boxes
            .iter()
            .find(|b| b.kind == TextKind::PinGroupCaption)
            .expect("キャプションの矩形");
        let center_x = (caption_box.rect.min.x + caption_box.rect.max.x) / 2.0;
        let mut overlapping_note = note((center_x, caption_box.rect.max.y), "点", 2.0);
        if let Entity::Text(t) = &mut overlapping_note {
            t.anchor = crate::model::TextAnchor::Middle;
        }
        let mut with_note = sheet.clone();
        with_note.entities.insert(overlapping_note.id(), overlapping_note);
        assert!(
            label_overlaps(&with_note, &defs) > before,
            "キャプションとの重なりも数える"
        );
    }

    /// The measured text of a tagged net label sits inside its flag, so the overlap count follows what is actually printed.
    /// タグ付きネットラベルの文字は旗の中として測られるので、重なりの数は実際に印字される見た目と一致する。
    #[test]
    fn a_tagged_net_label_is_measured_inside_its_flag() {
        let mut label = NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(100.0, 50.0),
            name: "24V_1".into(),
            rotation: 0,
            attrs: Default::default(),
        };
        label.attrs.insert("tag".into(), "right".into());
        let sheet = sheet_with(vec![Entity::NetLabel(label)]);
        let boxes = text_boxes(&sheet, &builtin_symbols());
        assert_eq!(boxes.len(), 1);
        assert!(boxes[0].rect.max.x <= 100.0, "本体は基準点の左: {:?}", boxes[0].rect);
    }

    /// A smaller annotation text height makes the measured net label and wire number smaller too.
    /// 注記文字高を小さくすると、測られるネットラベル・線番の大きさも一緒に小さくなる。
    #[test]
    fn measured_annotation_text_follows_the_sheet_setting() {
        let label = Entity::NetLabel(NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(100.0, 50.0),
            name: "24V_1".into(),
            rotation: 0,
            attrs: Default::default(),
        });
        let mut sheet = sheet_with(vec![label]);
        let width = |s: &Sheet| {
            let r = text_boxes(s, &builtin_symbols())[0].rect;
            r.max.x - r.min.x
        };
        let before = width(&sheet);
        sheet.annotation_text_mm = Some(1.4);
        assert!((width(&sheet) / before - 0.7).abs() < 1e-9);
    }

    /// Symbol origins and wire vertices sitting on the 2.5 mm grid report no off-grid points.
    /// シンボル原点も配線頂点も2.5mmグリッドに乗っていれば、グリッド外は0件になる。
    #[test]
    fn entities_on_the_grid_report_no_off_grid_points() {
        let sheet = sheet_with(vec![
            symbol("resistor", (100.0, 50.0), "R1"),
            wire(&[(50.0, 50.0), (92.5, 50.0)]),
        ]);
        assert_eq!(off_grid_count(&sheet), 0);
    }

    /// A point shifted by 0.1 mm off the grid is counted, and each stray vertex counts once.
    /// グリッドから0.1mmずれた点は数えられ、ずれた頂点1つにつき1件になる。
    #[test]
    fn a_point_shifted_a_tenth_of_a_millimetre_is_off_grid() {
        let sheet = sheet_with(vec![
            symbol("resistor", (100.1, 50.0), "R1"),
            wire(&[(50.0, 50.1), (92.6, 50.0)]),
        ]);
        assert_eq!(off_grid_count(&sheet), 3);
    }

    /// An empty sheet is perfectly tidy: every metric is zero.
    /// 何も置かれていないシートは完全に整っており、全てのメトリクスが0になる。
    #[test]
    fn an_empty_sheet_scores_zero_on_every_metric() {
        let sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let m = tidy_metrics(&sheet, &builtin_symbols());
        assert_eq!(m, TidyMetrics::default());
        assert_eq!(m.total(), 0);
    }

    /// tidy_metrics gathers the four counts of one sheet in a single value.
    /// tidy_metricsは1枚のシートの4つの数値をまとめて1つの値で返す。
    #[test]
    fn tidy_metrics_gathers_the_four_counts() {
        let sheet = sheet_with(vec![
            wire(&[(50.0, 50.0), (100.0, 50.0)]),
            wire(&[(75.0, 25.0), (75.0, 75.1)]),
            symbol("resistor", (100.0, 100.0), ""),
            symbol("resistor", (105.0, 100.0), ""),
            note((95.0, 101.0), "注記", 2.5),
        ]);
        let m = tidy_metrics(&sheet, &builtin_symbols());
        assert_eq!(m.crossings, 1);
        assert_eq!(m.symbol_overlaps, 1);
        assert!(m.label_overlaps >= 1, "{m:?}");
        assert_eq!(m.off_grid, 1);
        assert_eq!(
            m.total(),
            m.crossings + m.label_overlaps + m.symbol_overlaps + m.off_grid
        );
    }

    /// Measuring the same sheet twice gives exactly the same numbers, so the agent can compare before and after.
    /// 同じシートを2回測ると全く同じ数値になるので、エージェントは編集の前後を比べられる。
    #[test]
    fn measuring_the_same_sheet_twice_gives_the_same_numbers() {
        let sheet = sheet_with(vec![
            wire(&[(50.0, 50.0), (100.0, 50.0)]),
            wire(&[(75.0, 25.0), (75.0, 75.0)]),
            wire(&[(60.0, 30.1), (60.0, 80.0)]),
            symbol("resistor", (100.0, 100.0), "R1"),
            symbol("resistor", (105.0, 100.0), "R2"),
            note((95.0, 101.0), "注記", 2.5),
        ]);
        let symbols = builtin_symbols();
        assert_eq!(tidy_metrics(&sheet, &symbols), tidy_metrics(&sheet, &symbols));
    }

    /// tidy_issues lists every counted problem with its kind, the entities involved, the text and where it is, one entry per count, so the agent can find and fix each one.
    /// tidy_issuesは数えた問題を1件ずつ、種類・関係するエンティティ・文字・場所つきで並べ、件数と一致するので、エージェントは1件ずつ見つけて直せる。
    #[test]
    fn tidy_issues_name_each_counted_problem() {
        let a = wire(&[(50.0, 50.0), (100.0, 50.0)]);
        let b = wire(&[(75.0, 25.0), (75.0, 75.1)]);
        let r1 = symbol("resistor", (100.0, 100.0), "");
        let r2 = symbol("resistor", (105.0, 100.0), "");
        let n = note((95.0, 101.0), "注記", 2.5);
        let ids = (a.id(), b.id(), r1.id(), r2.id(), n.id());
        let sheet = sheet_with(vec![a, b, r1, r2, n]);
        let symbols = builtin_symbols();
        let m = tidy_metrics(&sheet, &symbols);
        let issues = tidy_issues(&sheet, &symbols);
        let count = |k: TidyIssueKind| issues.iter().filter(|i| i.kind == k).count();
        assert_eq!(count(TidyIssueKind::Crossing), m.crossings);
        assert_eq!(count(TidyIssueKind::LabelOverlap), m.label_overlaps);
        assert_eq!(count(TidyIssueKind::SymbolOverlap), m.symbol_overlaps);
        assert_eq!(count(TidyIssueKind::OffGrid), m.off_grid);

        let crossing = issues.iter().find(|i| i.kind == TidyIssueKind::Crossing).unwrap();
        assert!(crossing.entity_ids.contains(&ids.0) && crossing.entity_ids.contains(&ids.1));
        assert_eq!((crossing.at.x, crossing.at.y), (75.0, 50.0));
        let overlap = issues.iter().find(|i| i.kind == TidyIssueKind::SymbolOverlap).unwrap();
        assert!(overlap.entity_ids.contains(&ids.2) && overlap.entity_ids.contains(&ids.3));
        let label = issues.iter().find(|i| i.kind == TidyIssueKind::LabelOverlap).unwrap();
        assert!(label.entity_ids.contains(&ids.4), "{label:?}");
        assert!(label.texts.iter().any(|t| t == "注記"), "{label:?}");
        let off = issues.iter().find(|i| i.kind == TidyIssueKind::OffGrid).unwrap();
        assert_eq!(off.entity_ids, vec![ids.1]);
        assert_eq!((off.at.x, off.at.y), (75.0, 75.1));
    }
}
