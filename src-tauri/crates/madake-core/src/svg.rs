//! 印刷品質のSVGエクスポート。JIS図枠(枠線・ゾーン番号・表題欄)+全エンティティを描く。
//!
//! 図枠・表題欄・改訂欄の寸法と中身は**図枠テンプレート**([`crate::frame_template`])が
//! 持つデータで、ここはそれを描くだけ。テンプレートを差し替えれば全シートの図枠が変わる。

use std::fmt::Write as _;

use crate::frame_template::{BindContext, FrameTemplate, Weight};
use crate::geometry::Point;
use crate::model::{Entity, Revision, Sheet};
use crate::netlist::transform_local;
use crate::symbol::{Primitive, SymbolDef};

/// 図枠の用紙端からのマージン (mm)。**図面シートは図枠テンプレートの`frame_margin`を使う**。
/// この定数は帳票シート ([`crate::report_sheet`]) やゾーン住所の計算 ([`crate::xref`]) が
/// 使う、JIS標準どおりの既定値。
pub const FRAME_MARGIN: f64 = 10.0;
/// 配線の線幅 (mm)。
const WIRE_STROKE: f64 = 0.35;
/// シンボルの線幅 (mm)。
const SYMBOL_STROKE: f64 = 0.3;
/// 表題欄の外形 (幅, 高さ) mm。右下に置く。図面シートの表題欄は図枠テンプレートが決めるので、
/// この定数は帳票シート ([`crate::report_sheet`]) 用の既定値。
pub(crate) const TITLE_W: f64 = 120.0;
/// 表題欄・改訂欄の共通行高 (mm)。同上 (図面シートはテンプレートの`row_h`)。
pub(crate) const ROW_H: f64 = 8.0;
/// 枠線の線幅 (mm)。
pub(crate) const FRAME_STROKE: f64 = 0.5;
/// 罫線の線幅 (mm)。
pub(crate) const RULE_STROKE: f64 = 0.25;
/// 表題欄の文字高さ (mm)。
pub(crate) const TITLE_FONT: f64 = 3.0;
/// 改訂欄の列見出し (ISO 7200 / JIS Z 8311)。列幅は図枠テンプレートの`revision_block.col_w`。
const REV_HEADERS: [&str; 4] = ["記号", "日付", "内容", "承認"];
/// 改訂欄に描く既定の最大行数 (図枠テンプレートの`revision_block.rows_max`が無いとき)。
const REV_MAX_ROWS: usize = 6;
/// 改訂欄の本文・列見出しの文字高さ (mm)。
const REV_FONT: f64 = 2.5;
const REV_HEADER_FONT: f64 = 2.2;
/// セル内テキストの左余白 (mm)。
pub(crate) const CELL_PAD: f64 = 2.0;
/// 線番テキストの文字高さ (mm)。
pub(crate) const WIRE_NO_FONT: f64 = 2.5;
/// 線番テキストと配線の間隔 (mm)。グリッドピッチと同じ。
pub(crate) const WIRE_NO_GAP: f64 = 2.5;
/// 参照記号・型番・ネットラベルの文字高さ (mm)。
pub(crate) const LABEL_FONT: f64 = 2.5;
/// 参照記号のベースラインを、シンボル外形の上端からどれだけ上に置くか (mm)。
pub(crate) const REF_LABEL_DY: f64 = 4.5;
/// 型番・値のベースラインを、シンボル外形の上端からどれだけ上に置くか (mm)。
pub(crate) const VALUE_LABEL_DY: f64 = 1.0;
/// ネットラベルのベースラインを、基準点からどれだけ上に置くか (mm)。
pub(crate) const NET_LABEL_DY: f64 = 1.0;
/// ハーネス境界の破線 (IEC 61082-1 のグループ囲み)。線の長さと間隔 mm。
const HARNESS_DASH: (f64, f64) = (3.0, 2.0);
/// ハーネス境界の線幅 (mm)。図面の主線より細い補助線。
const HARNESS_STROKE: f64 = 0.25;
/// ハーネス名の文字高さ (mm)。
const HARNESS_FONT: f64 = 2.5;
/// ハーネス名の位置: 囲みの左上角から右へ / 上へ (mm)。
const HARNESS_LABEL_DX: f64 = 1.0;
const HARNESS_LABEL_DY: f64 = 1.0;
/// 盤の名前を外形の上端からどれだけ上に置くか (mm)。
const PANEL_LABEL_DY: f64 = 1.5;
/// ダクトの斜線・DINレールの取付線の線幅 (mm)。外形の罫線より細い補助線。
const DUCT_HATCH_STROKE: f64 = 0.15;
/// フットプリントの型番の文字高さ (mm) と、参照記号からの送り (mm)。
const FOOTPRINT_PART_NO_FONT: f64 = 2.0;
const FOOTPRINT_PART_NO_DY: f64 = 3.0;

/// 図面に描く改訂行(古い順)。既定の上限を超えた分は古い行から省く。
pub fn visible_revisions(revisions: &[Revision]) -> &[Revision] {
    visible_revisions_max(revisions, REV_MAX_ROWS)
}

/// 図面に描く改訂行(古い順)。上限は図枠テンプレートが決める。
pub fn visible_revisions_max(revisions: &[Revision], rows_max: usize) -> &[Revision] {
    &revisions[revisions.len().saturating_sub(rows_max)..]
}

/// 表題欄のRev欄に出す改訂記号。改訂があれば最新のmarkを優先する。
pub fn effective_rev(sheet: &Sheet) -> String {
    let latest = sheet
        .revisions
        .last()
        .map(|r| r.mark.trim())
        .filter(|m| !m.is_empty());
    match latest {
        Some(m) => m.to_string(),
        None if sheet.title_block.rev.is_empty() => "-".to_string(),
        None => sheet.title_block.rev.clone(),
    }
}

pub(crate) fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// 数値整形: 無駄な小数を出さない。
pub(crate) fn n(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{:.3}", v)
    }
}

/// 線色名→表示色。白地の紙に見える色を割り当てる。#始まりはそのまま。
pub fn color_hex(color: &str) -> &str {
    match color {
        "red" | "赤" => "#c00000",
        "black" | "黒" => "#000000",
        "white" | "白" => "#a0a0a0",
        "blue" | "青" => "#0000c0",
        "yellow" | "黄" => "#b89000",
        "green" | "緑" => "#008040",
        "orange" | "橙" => "#d07010",
        "purple" | "紫" => "#8020a0",
        "brown" | "茶" => "#805020",
        "gray" | "grey" | "灰" => "#808080",
        "pink" | "桃" => "#d06090",
        "light_blue" | "sky" | "水" | "水色" => "#2090c0",
        c if c.starts_with('#') => color,
        _ => "#000000",
    }
}

fn text_family_el(
    out: &mut String,
    x: f64,
    y: f64,
    size: f64,
    fill: &str,
    anchor: &str,
    family: &str,
    s: &str,
) {
    let _ = write!(
        out,
        "<text x=\"{}\" y=\"{}\" font-size=\"{}\" fill=\"{}\" text-anchor=\"{}\" font-family=\"{}\">{}</text>\n",
        n(x), n(y), n(size), fill, anchor, family, xml_escape(s)
    );
}

pub(crate) fn text_el(
    out: &mut String,
    x: f64,
    y: f64,
    size: f64,
    fill: &str,
    anchor: &str,
    s: &str,
) {
    text_family_el(out, x, y, size, fill, anchor, "sans-serif", s);
}

/// 表題欄のセル用。太字指定のときだけ`font-weight`を足す
/// (通常の太さのセルは[`text_el`]と1バイトも変わらない出力にする)。
fn text_weight_el(
    out: &mut String,
    x: f64,
    y: f64,
    size: f64,
    fill: &str,
    anchor: &str,
    weight: Weight,
    s: &str,
) {
    if weight == Weight::Normal {
        text_el(out, x, y, size, fill, anchor, s);
        return;
    }
    let _ = write!(
        out,
        "<text x=\"{}\" y=\"{}\" font-size=\"{}\" fill=\"{}\" text-anchor=\"{}\" font-family=\"sans-serif\" font-weight=\"bold\">{}</text>\n",
        n(x), n(y), n(size), fill, anchor, xml_escape(s)
    );
}

/// 矩形。属性の並び (x, y, width, height) は図枠のテストが依存しているので変えない。
pub(crate) fn rect_el(
    out: &mut String,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    fill: &str,
    stroke: &str,
    stroke_width: f64,
) {
    let _ = write!(
        out,
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>\n",
        n(x), n(y), n(w), n(h), fill, stroke, n(stroke_width)
    );
}

pub(crate) fn line_el(out: &mut String, x1: f64, y1: f64, x2: f64, y2: f64, w: f64, stroke: &str) {
    let _ = write!(
        out,
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{}\"/>\n",
        n(x1), n(y1), n(x2), n(y2), stroke, n(w)
    );
}

fn polyline_points(pts: &[Point]) -> String {
    pts.iter()
        .map(|p| format!("{},{}", n(p.x), n(p.y)))
        .collect::<Vec<_>>()
        .join(" ")
}

/// 図枠テンプレート ([`crate::frame_template`]) のとおりに図枠・ゾーン目盛・改訂欄・
/// 表題欄・ロゴ枠を描く。表題欄のセルはバインドを解決してから描く。
fn frame_and_title_block(out: &mut String, sheet: &Sheet, t: &FrameTemplate, ctx: &BindContext) {
    let (pw, ph) = sheet.paper_mm();
    let m = t.frame_margin;
    let (x0, y0) = (m, m);
    let (x1, y1) = (pw - m, ph - m);
    rect_el(out, x0, y0, x1 - x0, y1 - y0, "none", "#000", FRAME_STROKE);
    // ゾーン目盛。分割数はシートごとの設定 (図枠を替えても図面の住所は変わらない)、
    // 文字種 (数字/英字) はテンプレートが決める
    let cols = sheet.zone_cols.max(1) as f64;
    let rows = sheet.zone_rows.max(1) as f64;
    let zw = (x1 - x0) / cols;
    let zh = (y1 - y0) / rows;
    for i in 0..sheet.zone_cols.max(1) {
        let cx = x0 + zw * (i as f64 + 0.5);
        let label = t.zones.col_labels.label(i);
        text_el(out, cx, y0 - 3.0, 3.0, "#000", "middle", &label);
        text_el(out, cx, y1 + 6.0, 3.0, "#000", "middle", &label);
        if i > 0 {
            let tx = x0 + zw * i as f64;
            line_el(out, tx, y0 - 2.0, tx, y0, 0.25, "#000");
            line_el(out, tx, y1, tx, y1 + 2.0, 0.25, "#000");
        }
    }
    for i in 0..sheet.zone_rows.max(1) {
        let cy = y0 + zh * (i as f64 + 0.5) + 1.0;
        let label = t.zones.row_labels.label(i);
        text_el(out, x0 - 3.0, cy, 3.0, "#000", "middle", &label);
        text_el(out, x1 + 3.0, cy, 3.0, "#000", "middle", &label);
    }
    // 表題欄 (テンプレートの基準隅・寸法・セル)
    let def = &t.title_block;
    let (tx, ty) = t.title_block_origin(pw, ph);
    revision_block(out, sheet, t, tx, ty);
    rect_el(out, tx, ty, def.w, def.h, "#fff", "#000", FRAME_STROKE);
    for g in &def.grid_lines {
        line_el(out, tx + g.x1, ty + g.y1, tx + g.x2, ty + g.y2, RULE_STROKE, "#000");
    }
    for cell in &def.cells {
        let value = cell.resolve(ctx);
        if cell.omit_if_empty && value.is_empty() {
            continue;
        }
        text_weight_el(
            out,
            tx + cell.x,
            ty + cell.y,
            cell.size,
            "#000",
            cell.align.anchor(),
            cell.weight,
            &value,
        );
    }
    // ロゴ枠 (画像の埋め込みは未対応。枠だけを描いて置き場所を示す)
    if let Some(logo) = &t.logo {
        rect_el(out, logo.x, logo.y, logo.w, logo.h, "none", "#000", RULE_STROKE);
    }
}

/// 改訂欄 (ISO 7200 / JIS Z 8311)。表題欄の直上に同じ右端・同じ幅で描き、
/// 最下段を列見出し、その上に古い改訂から順に積む (最新が最上段)。0行なら何も描かない。
/// 行高・列幅・最大行数は図枠テンプレートが決める。
fn revision_block(out: &mut String, sheet: &Sheet, t: &FrameTemplate, tx: f64, ty: f64) {
    let rows = visible_revisions_max(&sheet.revisions, t.revision_block.rows_max);
    if rows.is_empty() {
        return;
    }
    let width = t.title_block.w;
    let row_h = t.title_block.row_h;
    // 高さ = (改訂行 + 列見出し1行) × 行高。下端は表題欄の上端。
    let h = (rows.len() as f64 + 1.0) * row_h;
    let top = ty - h;
    rect_el(out, tx, top, width, h, "#fff", "#000", FRAME_STROKE);
    for r in 1..=rows.len() {
        let ry = top + row_h * r as f64;
        line_el(out, tx, ry, tx + width, ry, RULE_STROKE, "#000");
    }
    // セル左端 (記号 / 日付 / 内容 / 承認) と、その間の仕切り線
    let lefts = t.revision_col_lefts(tx);
    for left in lefts.iter().skip(1) {
        line_el(out, *left, top, *left, ty, RULE_STROKE, "#000");
    }
    let baseline = |row_top: f64, font: f64| row_top + row_h / 2.0 + font / 2.0;
    // 列見出しは最下段 (表題欄側)
    let head_top = ty - row_h;
    for (i, label) in REV_HEADERS.iter().enumerate() {
        let cy = baseline(head_top, REV_HEADER_FONT);
        text_el(out, lefts[i] + CELL_PAD, cy, REV_HEADER_FONT, "#000", "start", label);
    }
    // 改訂行: 古い行 (rows[0]) が見出しの直上、新しい行ほど上へ積む
    for (i, rev) in rows.iter().enumerate() {
        let row_top = ty - (i as f64 + 2.0) * row_h;
        let cy = baseline(row_top, REV_FONT);
        let cells = [&rev.mark, &rev.date, &rev.description, &rev.by];
        for (c, value) in cells.iter().enumerate() {
            if value.is_empty() {
                continue;
            }
            text_el(out, lefts[c] + CELL_PAD, cy, REV_FONT, "#000", "start", value);
        }
    }
}

/// 配置後のシンボル外形の上端Y(用紙座標)。参照記号・型番の重なり回避用。
pub(crate) fn symbol_top_y(inst: &crate::model::SymbolInstance, def: &SymbolDef) -> f64 {
    symbol_bounds(inst, def).0.y
}

/// 配置後のシンボル外形の囲み矩形 (最小点, 最大点) を用紙座標で返す。
/// 円・弧は回転対称なので中心±rで安全側に評価する。配置基準点も必ず含める。
/// 参照記号・接点マップなど、外形の外側に置く注記の位置決めに使う。
pub fn symbol_bounds(inst: &crate::model::SymbolInstance, def: &SymbolDef) -> (Point, Point) {
    let (mut min, mut max) = (inst.at, inst.at);
    let mut visit = |p: Point| {
        let t = transform_local(p, inst);
        min.x = min.x.min(t.x);
        min.y = min.y.min(t.y);
        max.x = max.x.max(t.x);
        max.y = max.y.max(t.y);
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
            Primitive::Text { at, .. } => visit(*at),
        }
    }
    for pin in &def.pins {
        visit(pin.at);
    }
    (min, max)
}

fn render_symbol(out: &mut String, inst: &crate::model::SymbolInstance, def: &SymbolDef) {
    for prim in &def.primitives {
        match prim {
            Primitive::Line { pts } => {
                let tp: Vec<Point> = pts.iter().map(|p| transform_local(*p, inst)).collect();
                let _ = write!(
                    out,
                    "<polyline points=\"{}\" fill=\"none\" stroke=\"#000\" stroke-width=\"{}\"/>\n",
                    polyline_points(&tp),
                    n(SYMBOL_STROKE)
                );
            }
            Primitive::Circle { center, r, filled } => {
                let c = transform_local(*center, inst);
                let _ = write!(
                    out,
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" stroke=\"#000\" stroke-width=\"{}\"/>\n",
                    n(c.x), n(c.y), n(*r),
                    if *filled { "#000" } else { "none" },
                    n(SYMBOL_STROKE)
                );
            }
            Primitive::Arc { center, r, start_deg, end_deg } => {
                // 開始→終了を時計回りに描く。回転・ミラーは中心と角度の両方に効く
                let c = transform_local(*center, inst);
                let (start_deg, end_deg) = crate::netlist::rotate_arc_angles(
                    *start_deg,
                    *end_deg,
                    inst.rotation,
                    inst.mirror,
                );
                let (a0, a1) = (start_deg.to_radians(), end_deg.to_radians());
                let (sx, sy) = (c.x + r * a0.cos(), c.y + r * a0.sin());
                let (ex, ey) = (c.x + r * a1.cos(), c.y + r * a1.sin());
                let large = if (end_deg - start_deg).abs() > 180.0 { 1 } else { 0 };
                let _ = write!(
                    out,
                    "<path d=\"M {} {} A {} {} 0 {} 1 {} {}\" fill=\"none\" stroke=\"#000\" stroke-width=\"{}\"/>\n",
                    n(sx), n(sy), n(*r), n(*r), large, n(ex), n(ey), n(SYMBOL_STROKE)
                );
            }
            Primitive::Rect { p1, p2, filled } => {
                let corners = [
                    Point::new(p1.x, p1.y),
                    Point::new(p2.x, p1.y),
                    Point::new(p2.x, p2.y),
                    Point::new(p1.x, p2.y),
                ];
                let tp: Vec<Point> = corners.iter().map(|p| transform_local(*p, inst)).collect();
                let _ = write!(
                    out,
                    "<polygon points=\"{}\" fill=\"{}\" stroke=\"#000\" stroke-width=\"{}\"/>\n",
                    polyline_points(&tp),
                    if *filled { "#000" } else { "none" },
                    n(SYMBOL_STROKE)
                );
            }
            Primitive::Text { at, text, height } => {
                let p = transform_local(*at, inst);
                text_el(out, p.x, p.y + height / 2.0, *height, "#000", "middle", text);
            }
        }
    }
    // 参照記号と型番/値をシンボル外形の上に併記(縦長シンボルでも重ならない)
    let top = symbol_top_y(inst, def);
    if !inst.reference.is_empty() {
        text_el(out, inst.at.x, top - REF_LABEL_DY, LABEL_FONT, "#000", "middle", &inst.reference);
    }
    if !inst.value.is_empty() {
        text_el(out, inst.at.x, top - VALUE_LABEL_DY, LABEL_FONT, "#000", "middle", &inst.value);
    }
}

/// コイルの下に接点マップ (端子対 | 所在) の表を描く (M4 §4)。
/// 位置・列幅は [`crate::relay_xref::contact_map_layout`] が決め、キャンバス描画と共通。
fn render_contact_map(
    out: &mut String,
    inst: &crate::model::SymbolInstance,
    def: &SymbolDef,
    rows: &[crate::relay_xref::ContactMapRow],
) {
    use crate::relay_xref::{contact_map_layout, contact_map_origin, CONTACT_MAP_FONT};
    if rows.is_empty() {
        return;
    }
    let origin = contact_map_origin(inst, def);
    let layout = contact_map_layout(rows, origin.x, origin.y);
    let _ = write!(
        out,
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"#000\" stroke-width=\"{}\"/>\n",
        n(layout.x), n(layout.y), n(layout.width()), n(layout.height()), n(RULE_STROKE)
    );
    // 列の仕切り
    let _ = write!(
        out,
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#000\" stroke-width=\"{}\"/>\n",
        n(layout.col_x(1)), n(layout.y),
        n(layout.col_x(1)), n(layout.y + layout.height()),
        n(RULE_STROKE)
    );
    for (i, row) in rows.iter().enumerate() {
        if i > 0 {
            let _ = write!(
                out,
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#000\" stroke-width=\"{}\"/>\n",
                n(layout.x), n(layout.row_top(i)),
                n(layout.x + layout.width()), n(layout.row_top(i)),
                n(RULE_STROKE)
            );
        }
        let baseline = layout.baseline(i);
        for (col, text) in [(0, &row.terminals), (1, &row.address)] {
            text_family_el(
                out,
                layout.text_x(col),
                baseline,
                CONTACT_MAP_FONT,
                "#000",
                "start",
                "monospace",
                text,
            );
        }
    }
}

/// ネットの代表線分 (最も長い線分)。同長なら上・左の線分を選ぶ。
pub(crate) fn longest_segment(
    sheet: &Sheet,
    wire_ids: &[crate::model::EntityId],
) -> Option<(Point, Point)> {
    let mut best: Option<(f64, Point, Point)> = None;
    for id in wire_ids {
        let Some(Entity::Wire(w)) = sheet.entities.get(id) else {
            continue;
        };
        for seg in w.points.windows(2) {
            let (a, b) = (seg[0], seg[1]);
            let len = a.distance_to(&b);
            let mid = midpoint(&a, &b);
            let better = match &best {
                None => true,
                Some((blen, ba, bb)) => {
                    let bmid = midpoint(ba, bb);
                    len > blen + 1e-9
                        || ((len - blen).abs() <= 1e-9 && (mid.y, mid.x) < (bmid.y, bmid.x))
                }
            };
            if better {
                best = Some((len, a, b));
            }
        }
    }
    best.map(|(_, a, b)| (a, b))
}

pub(crate) fn midpoint(a: &Point, b: &Point) -> Point {
    Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
}

/// 線番 (IEC 62491)。ネットごとに代表線分の中点へ1回だけ描く。
/// 横向きの線分なら上へ、縦向きなら左へ [`WIRE_NO_GAP`] だけ離す。線番の無いネットは描かない。
fn render_wire_numbers(out: &mut String, sheet: &Sheet, symbols: &[SymbolDef]) {
    for net in crate::netlist::extract_netlist(sheet, symbols) {
        let Some(no) = net.wire_no.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
            continue;
        };
        let Some((a, b)) = longest_segment(sheet, &net.wire_ids) else {
            continue;
        };
        let mid = midpoint(&a, &b);
        if (b.x - a.x).abs() >= (b.y - a.y).abs() {
            text_family_el(out, mid.x, mid.y - WIRE_NO_GAP, WIRE_NO_FONT, "#000", "middle", "monospace", no);
        } else {
            text_family_el(out, mid.x - WIRE_NO_GAP, mid.y, WIRE_NO_FONT, "#000", "end", "monospace", no);
        }
    }
}

/// ハーネス境界 (IEC 61082-1 のグループ囲み)。破線の閉じた多角形と、その左上角の外側に
/// 置いたハーネス名を描く。配線より先に描いて背面に置く。名前が空なら囲みだけを描く。
fn render_harnesses(out: &mut String, sheet: &Sheet) {
    for entity in sheet.entities.values() {
        let Entity::Harness(h) = entity else {
            continue;
        };
        if h.points.len() < 2 {
            continue;
        }
        let _ = write!(
            out,
            "<polygon points=\"{}\" fill=\"none\" stroke=\"#000\" stroke-width=\"{}\" stroke-dasharray=\"{} {}\"/>\n",
            polyline_points(&h.points),
            n(HARNESS_STROKE),
            n(HARNESS_DASH.0),
            n(HARNESS_DASH.1)
        );
        let name = h.name.trim();
        if name.is_empty() {
            continue;
        }
        let Some((min, _)) = crate::harness::bounds(h) else {
            continue;
        };
        text_el(
            out,
            min.x + HARNESS_LABEL_DX,
            min.y - HARNESS_LABEL_DY,
            HARNESS_FONT,
            "#000",
            "start",
            name,
        );
    }
}

/// 盤レイアウト (M4仕様 §8) を描く。奥から順に **盤外形+取付板 → ダクト → DINレール →
/// フットプリント**。寸法の数式は [`crate::panel`] が正で、画面 (`src/canvas/panel.ts`) と
/// 同じ結果になる。紙面なので白地・黒線で、面は塗らない (ダクトは斜線で表す)。
fn render_panel(out: &mut String, sheet: &Sheet) {
    for entity in sheet.entities.values() {
        let Entity::PanelOutline(o) = entity else {
            continue;
        };
        let (min, max) = crate::panel::outline_rect(o);
        rect_el(out, min.x, min.y, max.x - min.x, max.y - min.y, "none", "#000", FRAME_STROKE);
        if let Some((pmin, pmax)) = crate::panel::plate_rect(o) {
            rect_el(
                out,
                pmin.x,
                pmin.y,
                pmax.x - pmin.x,
                pmax.y - pmin.y,
                "none",
                "#000",
                RULE_STROKE,
            );
        }
        let name = o.name.trim();
        if !name.is_empty() {
            text_el(out, min.x + CELL_PAD, min.y - PANEL_LABEL_DY, TITLE_FONT, "#000", "start", name);
        }
    }
    for duct in crate::panel::ducts(sheet) {
        rect_el(out, duct.at.x, duct.at.y, duct.w, duct.h, "none", "#000", RULE_STROKE);
        for (a, b) in crate::panel::duct_hatch(duct.at, duct.w, duct.h) {
            line_el(out, a.x, a.y, b.x, b.y, DUCT_HATCH_STROKE, "#000");
        }
    }
    for rail in crate::panel::rails(sheet) {
        let (min, max) = crate::panel::rail_rect(rail);
        rect_el(out, min.x, min.y, max.x - min.x, max.y - min.y, "none", "#000", RULE_STROKE);
        // 取付線 (部品が乗る中心線)
        line_el(out, min.x, rail.y, max.x, rail.y, DUCT_HATCH_STROKE, "#000");
    }
    for fp in crate::panel::footprints(sheet) {
        let (min, max) = crate::panel::footprint_rect(fp);
        rect_el(out, min.x, min.y, max.x - min.x, max.y - min.y, "none", "#000", SYMBOL_STROKE);
        let (cx, cy) = ((min.x + max.x) / 2.0, (min.y + max.y) / 2.0);
        let reference = fp.reference.trim();
        if !reference.is_empty() {
            text_family_el(out, cx, cy, LABEL_FONT, "#000", "middle", "monospace", reference);
        }
        let part_no = fp.part_no.trim();
        if !part_no.is_empty() {
            text_el(out, cx, cy + FOOTPRINT_PART_NO_DY, FOOTPRINT_PART_NO_FONT, "#000", "middle", part_no);
        }
    }
}

/// シート1枚を完全なSVG文書として書き出す。座標系はmm 1:1。図枠は既定の`jis-a3`。
/// シート単体の出力なのでシート間クロスリファレンスは描かない
/// (描くには [`project_sheet_to_svg`] を使う)。
pub fn sheet_to_svg(sheet: &Sheet, symbols: &[SymbolDef]) -> String {
    sheet_to_svg_with_frame(sheet, symbols, crate::frame_template::default_template())
}

/// 図枠テンプレートを指定してシート1枚を書き出す(図枠のプレビュー・テスト用)。
pub fn sheet_to_svg_with_frame(
    sheet: &Sheet,
    symbols: &[SymbolDef],
    frame: &FrameTemplate,
) -> String {
    sheet_to_svg_with_xrefs(
        sheet,
        symbols,
        &std::collections::BTreeMap::new(),
        &RelayAnnotations::default(),
        frame,
        &BindContext::for_sheet(sheet),
    )
}

/// リレーのコイル⇔接点クロスリファレンス注記 ([`crate::relay_xref`])。
/// プロジェクト全体を見ないと決まらないので、シート単体の出力では空になる。
#[derive(Debug, Clone, Default)]
pub struct RelayAnnotations {
    /// コイルのentity id → その下に描く接点マップの行。
    pub contact_maps: std::collections::BTreeMap<
        crate::model::EntityId,
        Vec<crate::relay_xref::ContactMapRow>,
    >,
    /// 接点のentity id → 脇に描くコイル所在 (例 "(/1.C2)")。
    pub coil_locations: std::collections::BTreeMap<crate::model::EntityId, String>,
}

/// プロジェクト内の1シートをSVGとして書き出す。図枠はプロジェクトの図枠テンプレート
/// ([`crate::model::Project::frame_template`])。ネットラベルの脇には、他シートにある
/// 同名ラベルの住所「/シート.ゾーン」([`crate::xref`]) を自動で添える。
/// シートが見つからなければNone。
pub fn project_sheet_to_svg(
    project: &crate::model::Project,
    sheet_id: crate::model::SheetId,
    symbols: &[SymbolDef],
) -> Option<String> {
    let sheet = project.sheet(sheet_id)?;
    let xrefs = crate::xref::sheet_xrefs(project, sheet_id);
    let relay = RelayAnnotations {
        contact_maps: crate::relay_xref::sheet_contact_maps(project, sheet_id),
        coil_locations: crate::relay_xref::sheet_coil_locations(project, sheet_id),
    };
    let frame = crate::frame_template::for_project(project);
    Some(sheet_to_svg_with_xrefs(
        sheet,
        symbols,
        &xrefs,
        &relay,
        &frame,
        &BindContext::for_project(project, sheet),
    ))
}

/// [`sheet_to_svg`] の本体。`xrefs`はネットラベルのentity id → 脇に描くXRefテキスト、
/// `relay`はコイル⇔接点のクロスリファレンス注記、`frame`/`ctx`は図枠テンプレートと
/// 表題欄バインドの文脈。
fn sheet_to_svg_with_xrefs(
    sheet: &Sheet,
    symbols: &[SymbolDef],
    xrefs: &std::collections::BTreeMap<crate::model::EntityId, String>,
    relay: &RelayAnnotations,
    frame: &FrameTemplate,
    ctx: &BindContext,
) -> String {
    let (pw, ph) = sheet.paper_mm();
    let mut out = String::new();
    let _ = write!(
        out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}mm\" height=\"{}mm\" viewBox=\"0 0 {} {}\">\n",
        n(pw), n(ph), n(pw), n(ph)
    );
    let _ = write!(
        out,
        "<rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"#ffffff\"/>\n",
        n(pw), n(ph)
    );
    frame_and_title_block(&mut out, sheet, frame, ctx);
    // 盤レイアウト (盤外形・ダクト・レール・フットプリント) は最背面
    render_panel(&mut out, sheet);
    // ハーネス境界は配線・シンボルの背面に置く
    render_harnesses(&mut out, sheet);
    let defs = crate::symbol::DefIndex::new(symbols);
    for entity in sheet.entities.values() {
        match entity {
            Entity::Wire(w) => {
                let _ = write!(
                    out,
                    "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"/>\n",
                    polyline_points(&w.points),
                    color_hex(&w.color),
                    n(WIRE_STROKE)
                );
            }
            Entity::Symbol(s) => {
                if let Some(def) = defs.get(s) {
                    render_symbol(&mut out, s, &def);
                    // コイル⇔接点クロスリファレンス (M4 §4)
                    if let Some(rows) = relay.contact_maps.get(&s.id) {
                        render_contact_map(&mut out, s, &def, rows);
                    }
                    if let Some(location) = relay.coil_locations.get(&s.id) {
                        let at = crate::relay_xref::coil_location_at(s, &def);
                        text_family_el(
                            &mut out,
                            at.x,
                            at.y,
                            crate::relay_xref::CONTACT_MAP_FONT,
                            "#000",
                            "start",
                            "monospace",
                            location,
                        );
                    }
                }
            }
            Entity::Junction(j) => {
                let _ = write!(
                    out,
                    "<circle cx=\"{}\" cy=\"{}\" r=\"0.6\" fill=\"#000\"/>\n",
                    n(j.at.x), n(j.at.y)
                );
            }
            Entity::NetLabel(l) => {
                text_el(&mut out, l.at.x, l.at.y - NET_LABEL_DY, LABEL_FONT, "#000", "start", &l.name);
                // シート間クロスリファレンス (IEC 61082-1)。相手がいるラベルだけ脇に添える
                if let Some(text) = xrefs.get(&l.id) {
                    let at = crate::xref::xref_text_at(l);
                    text_family_el(
                        &mut out,
                        at.x,
                        at.y,
                        crate::xref::XREF_FONT,
                        "#000",
                        "start",
                        "monospace",
                        text,
                    );
                }
            }
            Entity::Text(t) => {
                text_el(&mut out, t.at.x, t.at.y, t.height, "#000", "start", &t.text);
            }
            // ハーネス境界は背面のrender_harnesses、盤レイアウトはrender_panelで描き済み
            Entity::Harness(_)
            | Entity::PanelOutline(_)
            | Entity::DinRail(_)
            | Entity::Duct(_)
            | Entity::Footprint(_) => {}
        }
    }
    render_wire_numbers(&mut out, sheet, symbols);
    out.push_str("</svg>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::*;
    use crate::symbol::builtin_symbols;
    use uuid::Uuid;

    /// The exported SVG contains the JIS frame, the title block text, wires and reference designators.
    /// 出力SVGにはJIS図枠・表題欄の文字・配線・参照記号が含まれる。
    #[test]
    fn svg_contains_frame_wire_and_symbol() {
        let mut sheet = Sheet::new("TB1", PaperSize::A3, Orientation::Landscape);
        sheet.title_block.drawing_no = "MDK-001".into();
        let w = Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(50.0, 50.0), Point::new(100.0, 50.0)],
            color: "red".into(),
            sq: 0.3,
            length_m: None,
            part_no: None,
            net: None,
        });
        let s = Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "fuse".into(),
            at: Point::new(120.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "F1".into(),
            value: "5A".into(),
            attrs: Default::default(),
        });
        for e in [w, s] {
            sheet.entities.insert(e.id(), e);
        }
        let svg = sheet_to_svg(&sheet, &builtin_symbols());
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("viewBox=\"0 0 420 297\""));
        assert!(svg.contains("MDK-001"));
        assert!(svg.contains("polyline"));
        assert!(svg.contains(">F1<"));
        assert!(svg.matches("<text").count() >= 3);
        assert!(svg.trim_end().ends_with("</svg>"));
    }

    /// A block symbol is drawn with its outline, its pin numbers and its pin names, taken from the instance attributes.
    /// ブロックシンボルは、インスタンスの属性から外形・ピン番号・ピン名まで描かれる。
    #[test]
    fn svg_draws_block_outline_with_pin_numbers_and_names() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let block = crate::block::test_block(
            "CTRL1",
            "MainController",
            100.0,
            50.0,
            r#"{"left":[{"no":"IN1","name":"入力1"}],"right":[{"no":"OUT1","name":"出力1"}]}"#,
        );
        sheet.entities.insert(block.id(), block);
        let svg = sheet_to_svg(&sheet, &crate::symbol::sheet_symbol_defs(&sheet));
        assert!(svg.contains(">IN1<"), "ピン番号が描かれていない");
        assert!(svg.contains(">入力1<"), "ピン名が描かれていない");
        assert!(svg.contains(">OUT1<") && svg.contains(">出力1<"));
        assert!(svg.contains(">CTRL1<"), "参照記号が描かれていない");
        assert!(svg.contains("polygon"), "外形の長方形が描かれていない");
    }

    /// 図枠部分だけを取り出す (SVGの外殻と用紙の下地を除いた、図枠・表題欄・改訂欄の行)。
    fn frame_only(svg: &str) -> String {
        let mut out = String::new();
        for line in svg.lines() {
            if line.starts_with("<svg")
                || line.starts_with("</svg")
                || line.starts_with("<rect x=\"0\" y=\"0\"")
            {
                continue;
            }
            out.push_str(line);
            out.push('\n');
        }
        out
    }

    /// 表題欄を全欄埋めた改訂2件のA3シート (ゴールデンの素)。
    fn golden_sheet() -> Sheet {
        let mut sheet = Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape);
        sheet.title_block = TitleBlock {
            company: "まだけ電機".into(),
            title: "制御盤 主回路".into(),
            drawing_no: "MDK-001".into(),
            scale: "1:1".into(),
            date: "2026-08-23".into(),
            designed: "K.T".into(),
            drawn: "S.M".into(),
            checked: "Y.N".into(),
            approved: "H.I".into(),
            rev: "A".into(),
        };
        sheet.revisions = vec![
            Revision { mark: "A".into(), date: "26-08-01".into(), description: "初版".into(), by: "K.T".into() },
            Revision { mark: "B".into(), date: "26-08-20".into(), description: "端子台追加".into(), by: "K.T".into() },
        ];
        sheet
    }

    /// Drawing the jis-a3 frame template produces exactly the JIS frame this program has always printed, down to the last character of the SVG (frame, zone ticks, revision table and every title-block cell).
    /// jis-a3図枠テンプレートで描いた図枠は、これまで出力してきたJIS図枠とSVGの1文字まで同一である(外枠・ゾーン目盛・改訂欄・表題欄の全セル)。
    #[test]
    fn the_jis_a3_template_reproduces_the_previous_frame_exactly() {
        let golden = include_str!("../tests/golden/frame-jis-a3.svg");
        let got = frame_only(&sheet_to_svg(&golden_sheet(), &builtin_symbols()));
        assert_eq!(got, golden, "図枠の出力が変わっている");
    }

    /// An empty A4 sheet with no revisions also prints exactly the frame it always did: no revision table, and the title-block cells left blank rather than dropped.
    /// 改訂の無い空のA4シートも従来どおりの図枠を出す(改訂欄は描かれず、表題欄のセルは空欄のまま残る)。
    #[test]
    fn an_empty_sheet_reproduces_the_previous_frame_exactly() {
        let golden = include_str!("../tests/golden/frame-jis-a3-empty-a4.svg");
        let sheet = Sheet::new("Sheet1", PaperSize::A4, Orientation::Landscape);
        let got = frame_only(&sheet_to_svg(&sheet, &builtin_symbols()));
        assert_eq!(got, golden, "図枠の出力が変わっている");
    }

    /// The A4 portrait template prints a 190x277 frame with its 110 mm title block in the bottom-right corner, and the same 4x6 zone ticks.
    /// A4縦テンプレートでは190×277の図枠が描かれ、110mm幅の表題欄が右下に、ゾーン目盛は同じ4×6で出る。
    #[test]
    fn the_a4_portrait_template_draws_a_narrower_frame_and_title_block() {
        let mut sheet = golden_sheet();
        sheet.size = PaperSize::A4;
        sheet.orientation = Orientation::Portrait;
        let frame = crate::frame_template::find("jis-a4-portrait").expect("A4縦");
        let svg = sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &frame);
        assert!(svg.contains("viewBox=\"0 0 210 297\""), "{svg}");
        // 外枠 (10mmマージン)
        assert!(
            svg.contains("<rect x=\"10\" y=\"10\" width=\"190\" height=\"277\""),
            "{svg}"
        );
        // 表題欄 = 右下 (90, 255) の110x32
        assert!(
            svg.contains("<rect x=\"90\" y=\"255\" width=\"110\" height=\"32\""),
            "{svg}"
        );
        // 改訂欄 = その真上、同じ幅
        assert!(
            svg.contains("<rect x=\"90\" y=\"231\" width=\"110\" height=\"24\""),
            "{svg}"
        );
        // ゾーンは4列6行のまま (列=数字、行=英字)
        for label in ["1", "2", "3", "4", "A", "F"] {
            assert!(
                texts(&svg).iter().any(|(_, _, s)| s == label),
                "ゾーン{label}が無い: {svg}"
            );
        }
        // 表題欄の中身はjis-a3と同じ (ラベル+バインドの解決結果)
        assert!(svg.contains("MDK-001  Rev B"), "{svg}");
        assert!(svg.contains(">制御盤 主回路<"), "{svg}");
        assert!(svg.contains(">まだけ電機<"), "会社名は表題欄の左外: {svg}");
    }

    /// The frame template is what the drawing follows: switching the project to A4 portrait changes every sheet's frame, and it goes back on undo.
    /// 図面は図枠テンプレートに従う: プロジェクトをA4縦へ切り替えると全シートの図枠が変わり、undoで元へ戻る。
    #[test]
    fn switching_the_project_frame_template_changes_the_output() {
        use crate::command::{Command, Engine};
        let mut project = Project::new("t");
        project.sheets[0] = golden_sheet();
        let mut engine = Engine::new(project);
        let sheet_id = engine.project().sheets[0].id;
        let before =
            project_sheet_to_svg(engine.project(), sheet_id, &builtin_symbols()).unwrap();
        assert!(before.contains("<rect x=\"290\" y=\"255\" width=\"120\""), "{before}");

        engine
            .execute(Command::SetFrameTemplate {
                frame_template: Some("jis-a4-portrait".into()),
            })
            .unwrap();
        let after = project_sheet_to_svg(engine.project(), sheet_id, &builtin_symbols()).unwrap();
        assert!(
            after.contains("<rect x=\"300\" y=\"255\" width=\"110\""),
            "A3の用紙にA4縦の表題欄 (110mm幅) が入る: {after}"
        );

        engine.undo().unwrap().unwrap();
        let undone = project_sheet_to_svg(engine.project(), sheet_id, &builtin_symbols()).unwrap();
        assert_eq!(undone, before, "undoで元の図枠へ戻る");
    }

    /// A project pointing at a frame template nobody knows still prints, using the default JIS A3 frame.
    /// 誰も知らない図枠テンプレートを指しているプロジェクトも、既定のJIS A3図枠で問題なく出力できる。
    #[test]
    fn an_unknown_frame_template_still_prints_with_the_default_frame() {
        let mut project = Project::new("t");
        project.sheets[0] = golden_sheet();
        project.frame_template = Some("no-such-frame".into());
        let sheet_id = project.sheets[0].id;
        let svg = project_sheet_to_svg(&project, sheet_id, &builtin_symbols()).unwrap();
        assert!(svg.contains("<rect x=\"290\" y=\"255\" width=\"120\" height=\"32\""), "{svg}");
    }

    /// Title-block cells print the resolved binds: the sheet number out of the total, the project name and the latest revision all land in the block.
    /// 表題欄のセルにはバインドの解決結果が出る: 「何枚中の何枚目」・プロジェクト名・最新改訂がそのまま欄に入る。
    #[test]
    fn title_block_cells_print_resolved_binds() {
        use crate::frame_template::{Align, Cell, Weight};
        let mut project = Project::new("まだけ制御盤");
        project.sheets[0] = golden_sheet();
        project
            .sheets
            .push(Sheet::new("Sheet2", PaperSize::A3, Orientation::Landscape));
        let mut frame = crate::frame_template::default_template().clone();
        frame.title_block.cells.push(Cell {
            x: 60.0,
            y: 4.0,
            w: None,
            h: None,
            text: None,
            bind: Some("{project.name} {sheet.no}/{sheet.total} Rev {rev.latest}".into()),
            size: 2.5,
            weight: Weight::Bold,
            align: Align::Middle,
            omit_if_empty: false,
        });
        let svg = sheet_to_svg_with_xrefs(
            &project.sheets[0],
            &builtin_symbols(),
            &Default::default(),
            &RelayAnnotations::default(),
            &frame,
            &BindContext::for_project(&project, &project.sheets[0]),
        );
        assert!(svg.contains(">まだけ制御盤 1/2 Rev B<"), "{svg}");
        assert!(svg.contains("font-weight=\"bold\""), "太字指定が効く: {svg}");
    }

    /// A frame template with a logo frame draws an empty rectangle where the logo goes, so the space is reserved even before an image is set.
    /// ロゴ枠を持つ図枠テンプレートは、ロゴの位置に空の矩形を描く(画像を入れる前でも場所が確保される)。
    #[test]
    fn a_logo_frame_is_drawn_as_an_empty_rectangle() {
        use crate::frame_template::LogoFrame;
        let mut frame = crate::frame_template::default_template().clone();
        frame.logo = Some(LogoFrame { x: 15.0, y: 15.0, w: 40.0, h: 20.0, path: None });
        let svg = sheet_to_svg_with_frame(&golden_sheet(), &builtin_symbols(), &frame);
        assert!(
            svg.contains("<rect x=\"15\" y=\"15\" width=\"40\" height=\"20\" fill=\"none\""),
            "{svg}"
        );
    }

    /// Special characters in titles (<, >, &, quotes) are XML-escaped in the SVG.
    /// 品名などの特殊文字(<, >, &, 引用符)はSVG内でXMLエスケープされる。
    #[test]
    fn svg_escapes_xml_special_chars() {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        sheet.title_block.title = "A<B> & \"C\"".into();
        let svg = sheet_to_svg(&sheet, &builtin_symbols());
        assert!(svg.contains("A&lt;B&gt; &amp; &quot;C&quot;"));
        assert!(!svg.contains("A<B>"));
    }

    /// 属性値を1つ読む(テスト用の素朴なパーサ)。
    fn attr(line: &str, name: &str) -> Option<f64> {
        let pat = format!("{name}=\"");
        let i = line.find(&pat)? + pat.len();
        let rest = &line[i..];
        let j = rest.find('"')?;
        rest[..j].parse().ok()
    }

    /// SVG中の`<text>`を (x, y, 文字列) で列挙する。
    fn texts(svg: &str) -> Vec<(f64, f64, String)> {
        svg.lines()
            .filter(|l| l.trim_start().starts_with("<text "))
            .filter_map(|l| {
                let x = attr(l, "x")?;
                let y = attr(l, "y")?;
                let body = l.split_once('>')?.1.rsplit_once("</text>")?.0.to_string();
                Some((x, y, body))
            })
            .collect()
    }

    /// 改訂欄の領域 (表題欄と同じ右下ブロック) にある指定文字列のY座標。
    /// ゾーン記号の "A"/"B" と紛れないようX範囲で絞る。
    fn text_y(svg: &str, content: &str) -> Option<f64> {
        texts(svg)
            .into_iter()
            .find(|(x, _, s)| s == content && (290.0..=410.0).contains(x))
            .map(|(_, y, _)| y)
    }

    fn sheet_with_revisions(marks: &[&str]) -> Sheet {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        sheet.revisions = marks
            .iter()
            .map(|m| Revision {
                mark: (*m).into(),
                date: format!("26-08-{m}"),
                description: format!("変更{m}"),
                by: "K.T".into(),
            })
            .collect();
        sheet
    }

    /// Two revisions draw a table right above the title block, oldest at the bottom and newest on top, with a column header row.
    /// 改訂が2件あると表題欄の真上に改訂表が描かれ、古い行が下・新しい行が上に積まれ、最下段に列見出し(記号/日付/内容/承認)が出る。
    #[test]
    fn svg_draws_revision_table_above_title_block() {
        let sheet = sheet_with_revisions(&["A", "B"]);
        let svg = sheet_to_svg(&sheet, &builtin_symbols());
        // 列見出しと2行分の内容が出る
        assert!(svg.contains("記号"), "{svg}");
        assert!(svg.contains("変更A") && svg.contains("変更B"), "{svg}");
        let y_a = text_y(&svg, "A").expect("改訂Aの記号");
        let y_b = text_y(&svg, "B").expect("改訂Bの記号");
        let y_head = text_y(&svg, "記号").expect("列見出し");
        // 新しい改訂ほど上 (Yが小さい)、見出しは最下段
        assert!(y_b < y_a, "B({y_b}) は A({y_a}) より上");
        assert!(y_a < y_head, "見出し({y_head}) は改訂行より下");
        // 表題欄(右下120x32mm)の真上・同じ右端・同じ幅・行高8mm×(2行+見出し)
        assert!(
            svg.contains("<rect x=\"290\" y=\"231\" width=\"120\" height=\"24\""),
            "{svg}"
        );
    }

    /// A sheet with no revisions draws no revision table at all, not even an empty frame.
    /// 改訂が0件のシートには改訂欄をまったく描かない(空の枠だけも描かない)。
    #[test]
    fn svg_omits_revision_table_when_no_revisions() {
        let sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let svg = sheet_to_svg(&sheet, &builtin_symbols());
        assert!(!svg.contains("記号"), "{svg}");
        // 表題欄より上に120mm幅の枠が増えていない
        assert!(!svg.contains("<rect x=\"290\" y=\"231\""), "{svg}");
    }

    /// The Rev field of the title block shows the mark of the newest revision, and falls back to the stored value when there are no revisions.
    /// 表題欄のRev欄には最新改訂の記号が出る。改訂が無いときは表題欄に保存された値がそのまま出る。
    #[test]
    fn svg_title_block_rev_follows_latest_revision() {
        let mut sheet = sheet_with_revisions(&["A", "B", "C"]);
        sheet.title_block.rev = "A".into();
        let svg = sheet_to_svg(&sheet, &builtin_symbols());
        assert!(svg.contains("Rev C"), "{svg}");
        assert!(!svg.contains("Rev A"), "{svg}");

        let mut plain = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        plain.title_block.rev = "A".into();
        let svg2 = sheet_to_svg(&plain, &builtin_symbols());
        assert!(svg2.contains("Rev A"), "{svg2}");
    }

    /// With seven revisions only the newest six rows are drawn; the oldest row is dropped from the drawing while the data keeps it.
    /// 改訂が7件あると新しい6行だけが描かれ、最も古い行は図面から省かれる(データとしては残る)。
    #[test]
    fn svg_revision_table_shows_only_newest_six_rows() {
        let sheet = sheet_with_revisions(&["A", "B", "C", "D", "E", "F", "G"]);
        assert_eq!(sheet.revisions.len(), 7);
        let svg = sheet_to_svg(&sheet, &builtin_symbols());
        assert!(!svg.contains("変更A"), "最も古い行は省略される: {svg}");
        for m in ["B", "C", "D", "E", "F", "G"] {
            assert!(svg.contains(&format!("変更{m}")), "{m}行が無い: {svg}");
        }
        // 6行+見出し=7行分の高さ (8mm×7=56mm)
        assert!(
            svg.contains("<rect x=\"290\" y=\"199\" width=\"120\" height=\"56\""),
            "{svg}"
        );
    }

    /// テスト用のワイヤ (線番付き)。
    fn numbered_wire(points: &[(f64, f64)], net: Option<&str>) -> Entity {
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

    fn sheet_with(entities: Vec<Entity>) -> Sheet {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        for e in entities {
            sheet.entities.insert(e.id(), e);
        }
        sheet
    }

    /// The wire number of a horizontal wire is printed in a monospaced font 2.5 mm above the middle of the wire.
    /// 横向きの配線の線番は、配線の中点の2.5mm上に等幅フォントで描かれる。
    #[test]
    fn svg_draws_wire_number_above_a_horizontal_wire() {
        let sheet = sheet_with(vec![numbered_wire(&[(50.0, 100.0), (90.0, 100.0)], Some("12"))]);
        let svg = sheet_to_svg(&sheet, &builtin_symbols());
        let line = svg
            .lines()
            .find(|l| l.contains("monospace") && l.contains(">12<"))
            .unwrap_or_else(|| panic!("線番テキストが無い: {svg}"));
        assert_eq!(attr(line, "x"), Some(70.0), "中点のx: {line}");
        assert_eq!(attr(line, "y"), Some(97.5), "配線の2.5mm上: {line}");
        assert!(line.contains("text-anchor=\"middle\""), "{line}");
    }

    /// The wire number of a vertical wire is printed 2.5 mm to the left of the middle of the wire.
    /// 縦向きの配線の線番は、配線の中点の2.5mm左に描かれる。
    #[test]
    fn svg_draws_wire_number_left_of_a_vertical_wire() {
        let sheet = sheet_with(vec![numbered_wire(&[(80.0, 40.0), (80.0, 80.0)], Some("7"))]);
        let svg = sheet_to_svg(&sheet, &builtin_symbols());
        let line = svg
            .lines()
            .find(|l| l.contains("monospace") && l.contains(">7<"))
            .unwrap_or_else(|| panic!("線番テキストが無い: {svg}"));
        assert_eq!(attr(line, "x"), Some(77.5), "中点の2.5mm左: {line}");
        assert_eq!(attr(line, "y"), Some(60.0), "中点のy: {line}");
        assert!(line.contains("text-anchor=\"end\""), "{line}");
    }

    /// One net is labelled once, at the middle of its longest segment, however many wires it is drawn with.
    /// 1つのネットの線番は、何本のワイヤで描かれていても、最も長い線分の中点に1回だけ描かれる。
    #[test]
    fn svg_draws_the_wire_number_once_on_the_longest_segment() {
        // 短い縦線 (10mm) と長い横線 (60mm) が端点でつながった1ネット
        let sheet = sheet_with(vec![
            numbered_wire(&[(100.0, 50.0), (100.0, 60.0)], Some("3")),
            numbered_wire(&[(100.0, 60.0), (160.0, 60.0)], Some("3")),
        ]);
        let svg = sheet_to_svg(&sheet, &builtin_symbols());
        let hits: Vec<_> = svg
            .lines()
            .filter(|l| l.contains("monospace") && l.contains(">3<"))
            .collect();
        assert_eq!(hits.len(), 1, "ネットにつき1つ: {svg}");
        assert_eq!(attr(hits[0], "x"), Some(130.0), "長い横線の中点: {}", hits[0]);
        assert_eq!(attr(hits[0], "y"), Some(57.5), "その2.5mm上: {}", hits[0]);
    }

    /// A net without a wire number gets no number text at all.
    /// 線番の無いネットには線番テキストを一切描かない。
    #[test]
    fn svg_omits_wire_number_for_unnumbered_nets() {
        let sheet = sheet_with(vec![numbered_wire(&[(50.0, 100.0), (90.0, 100.0)], None)]);
        let svg = sheet_to_svg(&sheet, &builtin_symbols());
        assert!(!svg.contains("monospace"), "{svg}");
    }

    /// テスト用のハーネス境界 (矩形)。
    fn harness_entity(name: &str, x0: f64, y0: f64, x1: f64, y1: f64) -> Entity {
        Entity::Harness(Harness {
            id: Uuid::new_v4(),
            points: crate::harness::rect_points(Point::new(x0, y0), Point::new(x1, y1)),
            name: name.into(),
            note: String::new(),
        })
    }

    /// A harness boundary is drawn as a dashed rectangle (IEC 61082-1 group enclosure) around the wires it holds.
    /// ハーネス境界は、囲んだ配線のまわりに破線の矩形 (IEC 61082-1のグループ囲み) として描かれる。
    #[test]
    fn svg_draws_a_harness_as_a_dashed_rectangle() {
        let sheet = sheet_with(vec![harness_entity("W1", 50.0, 50.0, 150.0, 100.0)]);
        let svg = sheet_to_svg(&sheet, &builtin_symbols());
        let line = svg
            .lines()
            .find(|l| l.starts_with("<polygon") && l.contains("stroke-dasharray"))
            .unwrap_or_else(|| panic!("ハーネスの破線囲みが無い: {svg}"));
        assert!(line.contains("points=\"50,50 150,50 150,100 50,100\""), "{line}");
        assert!(line.contains("fill=\"none\""), "{line}");
        assert!(line.contains("stroke-dasharray=\"3 2\""), "{line}");
    }

    /// The harness name is printed just outside the top-left corner of the boundary.
    /// ハーネス名は囲みの左上角のすぐ外側に描かれる。
    #[test]
    fn svg_labels_the_harness_at_its_top_left_corner() {
        let sheet = sheet_with(vec![harness_entity("W1", 50.0, 50.0, 150.0, 100.0)]);
        let svg = sheet_to_svg(&sheet, &builtin_symbols());
        let label = texts(&svg)
            .into_iter()
            .find(|(_, _, s)| s == "W1")
            .unwrap_or_else(|| panic!("ハーネス名が無い: {svg}"));
        assert_eq!((label.0, label.1), (51.0, 49.0), "左上角の外側: {label:?}");
    }

    /// A harness with no name draws only its dashed boundary, without a label.
    /// 名前の無いハーネスは破線の囲みだけを描き、ラベルは出さない。
    #[test]
    fn svg_omits_the_label_of_an_unnamed_harness() {
        let sheet = sheet_with(vec![harness_entity("", 50.0, 50.0, 150.0, 100.0)]);
        let svg = sheet_to_svg(&sheet, &builtin_symbols());
        assert!(svg.contains("stroke-dasharray"), "囲みは描く: {svg}");
        assert!(
            !texts(&svg).iter().any(|&(x, y, _)| x == 51.0 && y == 49.0),
            "囲みの左上角にラベルは描かない: {svg}"
        );
    }

    /// Rotated symbols are drawn with their shapes actually rotated (90 deg makes a resistor body vertical).
    /// 回転したシンボルは形状ごと回転して描かれる(90度で抵抗の本体が縦長になる)。
    #[test]
    fn svg_renders_rotated_symbol_primitives() {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        let s = Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "resistor".into(),
            at: Point::new(100.0, 100.0),
            rotation: 90,
            mirror: false,
            reference: "R1".into(),
            value: String::new(),
            attrs: Default::default(),
        });
        sheet.entities.insert(s.id(), s);
        let svg = sheet_to_svg(&sheet, &builtin_symbols());
        // 90度回転で本体矩形は縦長になる: 頂点(±5,±2)→(100∓2, 100±5) を含む
        assert!(svg.contains("98,95") || svg.contains("98,105"), "{svg}");
    }

    /// 2枚のA3シートに同名ネットラベル付きの配線を1本ずつ置いたプロジェクト。
    fn two_sheet_project() -> Project {
        let mut project = Project::new("t");
        project
            .sheets
            .push(Sheet::new("Sheet2", PaperSize::A3, Orientation::Landscape));
        let put = |sheet: &mut Sheet, x: f64, y: f64| {
            let w = Entity::Wire(Wire {
                id: Uuid::new_v4(),
                points: vec![Point::new(x, y), Point::new(x + 40.0, y)],
                color: "black".into(),
                sq: 0.75,
                length_m: None,
                part_no: None,
                net: None,
            });
            let l = Entity::NetLabel(NetLabel {
                id: Uuid::new_v4(),
                at: Point::new(x, y),
                name: "24V_1".into(),
                rotation: 0,
            });
            for e in [w, l] {
                sheet.entities.insert(e.id(), e);
            }
        };
        let (s1, s2) = project.sheets.split_at_mut(1);
        put(&mut s1[0], 20.0, 20.0);
        put(&mut s2[0], 250.0, 70.0);
        project
    }

    /// A sheet exported as part of its project shows, next to each net label, the "/sheet.zone" address of the same-named label on the other sheet.
    /// プロジェクトの一部として書き出したシートには、各ネットラベルの脇に他シートの同名ラベルの住所「/シート.ゾーン」が出る。
    #[test]
    fn svg_draws_cross_reference_address_next_to_net_label() {
        let project = two_sheet_project();
        let svg = project_sheet_to_svg(&project, project.sheets[0].id, &builtin_symbols()).unwrap();
        assert!(svg.contains(">/2.B3<"), "相手先の住所が出る: {svg}");
        assert!(svg.contains(">24V_1<"), "ラベル本文も残る");
        // XRefは等幅・ラベル本文より小さい文字で、ラベルの右脇に置く
        assert!(
            svg.contains(&format!("font-size=\"{}\"", n(crate::xref::XREF_FONT))),
            "小さめの文字: {svg}"
        );
        let (x, y, _) = texts(&svg)
            .into_iter()
            .find(|(_, _, t)| t == "/2.B3")
            .expect("XRefテキスト");
        assert!(x > 20.0, "ラベル(x=20)の右: {x}");
        assert!((y - 19.0).abs() < 1e-9, "ラベル本文と同じベースライン: {y}");
    }

    /// A net label with no counterpart on another sheet gets no cross-reference text.
    /// 他のシートに相手がいないネットラベルには、クロスリファレンスの文字が出ない。
    #[test]
    fn svg_omits_cross_reference_when_there_is_no_counterpart() {
        let mut project = two_sheet_project();
        project.sheets.truncate(1);
        let svg = project_sheet_to_svg(&project, project.sheets[0].id, &builtin_symbols()).unwrap();
        assert!(svg.contains(">24V_1<"));
        assert!(!svg.contains(">/"), "相手先が無ければ何も描かない: {svg}");
    }

    /// Exporting a single sheet on its own (no project context) never draws cross-references.
    /// プロジェクトの文脈なしにシート単体を書き出したときは、クロスリファレンスを描かない。
    #[test]
    fn svg_of_a_lone_sheet_has_no_cross_reference() {
        let project = two_sheet_project();
        let svg = sheet_to_svg(&project.sheets[0], &builtin_symbols());
        assert!(svg.contains(">24V_1<"));
        assert!(!svg.contains("/2.B3"), "{svg}");
    }

    /// コイル(シート1)+a接点(シート2)のリレーK1を置いた2枚組プロジェクト。
    fn relay_project() -> Project {
        let mut project = Project::new("t");
        project
            .sheets
            .push(Sheet::new("Sheet2", PaperSize::A3, Orientation::Landscape));
        let put = |sheet: &mut Sheet, symbol_id: &str, x: f64, y: f64| {
            let e = Entity::Symbol(SymbolInstance {
                id: Uuid::new_v4(),
                symbol_id: symbol_id.into(),
                at: Point::new(x, y),
                rotation: 0,
                mirror: false,
                reference: "K1".into(),
                value: String::new(),
                attrs: Default::default(),
            });
            sheet.entities.insert(e.id(), e);
        };
        let (s1, s2) = project.sheets.split_at_mut(1);
        put(&mut s1[0], "relay_coil", 150.0, 120.0);
        put(&mut s2[0], "relay_contact_no", 250.0, 70.0);
        project
    }

    /// The sheet holding the coil shows a contact map under it: one row per contact with its terminal pair and its address.
    /// コイルのあるシートには、コイルの下に接点マップ(端子対と所在の行)が描かれる。
    #[test]
    fn svg_draws_the_contact_map_under_the_coil() {
        let project = relay_project();
        let svg = project_sheet_to_svg(&project, project.sheets[0].id, &builtin_symbols()).unwrap();
        assert!(svg.contains(">13-14<"), "端子対の行: {svg}");
        assert!(svg.contains(">/2.B3<"), "接点の所在");
        let (_, y, _) = texts(&svg)
            .into_iter()
            .find(|(_, _, t)| t == "13-14")
            .expect("端子対テキスト");
        assert!(y > 120.0, "コイル(y=120)より下に置く: {y}");
    }

    /// The sheet holding the contact shows the coil's address in parentheses beside it.
    /// 接点のあるシートには、その脇にコイルの住所が丸括弧付きで描かれる。
    #[test]
    fn svg_draws_the_coil_location_beside_the_contact() {
        let project = relay_project();
        let svg = project_sheet_to_svg(&project, project.sheets[1].id, &builtin_symbols()).unwrap();
        assert!(svg.contains(">(/1.C2)<"), "コイル所在: {svg}");
        let (x, _, _) = texts(&svg)
            .into_iter()
            .find(|(_, _, t)| t == "(/1.C2)")
            .expect("コイル所在テキスト");
        assert!(x > 250.0, "接点(x=250)の右脇: {x}");
    }

    /// Exporting a single sheet on its own draws no contact map, because the counterpart sheets are unknown.
    /// シート単体の書き出しでは相手のシートが分からないので、接点マップは描かれない。
    #[test]
    fn svg_of_a_lone_sheet_has_no_contact_map() {
        let project = relay_project();
        let svg = sheet_to_svg(&project.sheets[0], &builtin_symbols());
        assert!(!svg.contains(">13-14<"), "{svg}");
    }
    /// 盤レイアウトシート (盤外形200x150・ダクト・DINレール・K1のフットプリント)。
    fn panel_sheet() -> Sheet {
        let mut sheet =
            Sheet::new_kind("盤", PaperSize::A3, Orientation::Landscape, SheetKind::Panel);
        let entities = vec![
            Entity::PanelOutline(PanelOutline {
                id: Uuid::new_v4(),
                at: Point::new(40.0, 40.0),
                w: 200.0,
                h: 150.0,
                name: "制御盤".into(),
            }),
            Entity::Duct(Duct {
                id: Uuid::new_v4(),
                at: Point::new(100.0, 100.0),
                w: 20.0,
                h: 20.0,
            }),
            Entity::DinRail(DinRail {
                id: Uuid::new_v4(),
                y: 150.0,
                x1: 60.0,
                x2: 220.0,
            }),
            Entity::Footprint(Footprint {
                id: Uuid::new_v4(),
                at: Point::new(60.0, 132.5),
                w: 25.0,
                h: 65.0,
                rotation: 0,
                reference: "K1".into(),
                part_no: "MDK-RLY-24V".into(),
            }),
        ];
        for e in entities {
            sheet.entities.insert(e.id(), e);
        }
        sheet
    }

    /// A panel-layout sheet is exported with the same drawing frame as a schematic, plus the enclosure and its mounting plate.
    /// 盤レイアウトシートは回路図と同じ図枠つきで書き出され、そこに盤外形と取付板が描かれる。
    #[test]
    fn svg_of_a_panel_sheet_draws_the_enclosure_and_its_mounting_plate() {
        let svg = sheet_to_svg(&panel_sheet(), &builtin_symbols());
        assert!(svg.contains("viewBox=\"0 0 420 297\""));
        assert!(
            svg.contains("<rect x=\"40\" y=\"40\" width=\"200\" height=\"150\""),
            "盤外形: {svg}"
        );
        assert!(
            svg.contains("<rect x=\"50\" y=\"50\" width=\"180\" height=\"130\""),
            "取付板 (10mm内側): {svg}"
        );
        assert!(svg.contains(">制御盤<"), "盤の名前: {svg}");
    }

    /// A duct is drawn as a rectangle filled with 45-degree hatch lines.
    /// ダクトは45度のハッチ(斜線)で埋めた矩形として描かれる。
    #[test]
    fn svg_of_a_panel_sheet_hatches_the_duct() {
        let svg = sheet_to_svg(&panel_sheet(), &builtin_symbols());
        assert!(
            svg.contains("<rect x=\"100\" y=\"100\" width=\"20\" height=\"20\""),
            "ダクトの外形: {svg}"
        );
        assert!(
            svg.contains("<line x1=\"100\" y1=\"100\" x2=\"120\" y2=\"120\""),
            "ダクトの斜線: {svg}"
        );
    }

    /// A DIN rail is drawn as a horizontal band centred on its mounting line.
    /// DINレールは取付線を中心にした横帯として描かれる。
    #[test]
    fn svg_of_a_panel_sheet_draws_the_din_rail() {
        let svg = sheet_to_svg(&panel_sheet(), &builtin_symbols());
        assert!(
            svg.contains("<rect x=\"60\" y=\"132.500\" width=\"160\" height=\"35\""),
            "DINレール: {svg}"
        );
    }

    /// A footprint is drawn as its outline rectangle with the reference designator and the type number inside.
    /// フットプリントは外形の矩形として描かれ、その中に参照記号と型番が入る。
    #[test]
    fn svg_of_a_panel_sheet_draws_the_footprint_with_its_reference() {
        let svg = sheet_to_svg(&panel_sheet(), &builtin_symbols());
        assert!(
            svg.contains("<rect x=\"60\" y=\"132.500\" width=\"25\" height=\"65\""),
            "フットプリントの外形: {svg}"
        );
        assert!(svg.contains(">K1<"), "参照記号: {svg}");
        assert!(svg.contains(">MDK-RLY-24V<"), "型番: {svg}");
    }

    /// A footprint turned 90 degrees is drawn lying down (its width and height swap).
    /// 90度回したフットプリントは横倒しに描かれる(幅と高さが入れ替わる)。
    #[test]
    fn svg_draws_a_rotated_footprint_lying_down() {
        let mut sheet =
            Sheet::new_kind("盤", PaperSize::A3, Orientation::Landscape, SheetKind::Panel);
        let fp = Entity::Footprint(Footprint {
            id: Uuid::new_v4(),
            at: Point::new(60.0, 100.0),
            w: 25.0,
            h: 65.0,
            rotation: 90,
            reference: "K2".into(),
            part_no: String::new(),
        });
        sheet.entities.insert(fp.id(), fp);
        let svg = sheet_to_svg(&sheet, &builtin_symbols());
        assert!(
            svg.contains("<rect x=\"60\" y=\"100\" width=\"65\" height=\"25\""),
            "{svg}"
        );
    }
}
