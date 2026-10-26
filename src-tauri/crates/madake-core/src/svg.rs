//! 印刷品質のSVGエクスポート。JIS図枠(枠線・ゾーン番号・表題欄)+全エンティティを描く。
//!
//! 図枠・表題欄・改訂欄の寸法と中身は**図枠テンプレート**([`crate::frame_template`])が
//! 持つデータで、ここはそれを描くだけ。テンプレートを差し替えれば全シートの図枠が変わる。

use std::fmt::Write as _;

use crate::frame_template::{BindContext, FrameTemplate, Weight};
use crate::geometry::Point;
use crate::model::{Entity, Revision, ShapeKind, Sheet};
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
// ジャンパバー (G5) の見た目はキャンバス描画と共通なので、端子台チャート側に置いてある。
pub(crate) use crate::terminal_chart::JUMPER_BAR_STROKE;
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
/// 嵌合コネクタ対 (G14) の破線。ハーネス境界より短い刻みにして、
/// 「短い連絡線」であることを見た目でも区別する。
const MATE_LINE_DASH: (f64, f64) = (1.5, 1.0);
/// 嵌合コネクタ対の破線の線幅 (mm)。ハーネス境界と同じ細さ。
const MATE_LINE_STROKE: f64 = 0.25;
/// 引き出し線 (G20) の線幅 (mm)。ハーネス境界の細線と同じ太さ。
const LEADER_STROKE: f64 = 0.25;
/// ケーブル定義線とシールド楕円の線幅 mm。
const CABLE_STROKE: f64 = 0.35;
/// 丸印の半径 (mm)。配線の分岐点 (ジャンクション) と、引き出し線 (G20) の対象端に
/// 同じ大きさで描く。図面上で「ここに繋がっている」を表す印は1種類だけにする。
pub const JUNCTION_DOT_R: f64 = 0.6;
/// ピングループ括弧 (G8) の線幅 (mm)。シンボル本体の線と同じ太さ。
const PIN_GROUP_BRACKET_STROKE: f64 = SYMBOL_STROKE;
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
    caption_bounds(inst, def).0.y
}

/// 参照記号・型番を上に、機能ラベル・説明・定格を下に置くための外形 (最小点, 最大点)。
///
/// 普通のシンボルは [`symbol_bounds`] そのもの。ブロック ([`crate::block::is_block_export_id`]) は、
/// ピン番号などの文字が基準点だけでなく文字の高さのぶん上下へはみ出す (文字は回転しても横書きで、
/// 回転すると左右辺のピン番号が上下辺に来る) ので、文字の高さも含める。
/// 文字は常に横書きなので、上下の範囲に効くのは文字の高さだけ (幅は効かない)。
pub(crate) fn caption_bounds(inst: &crate::model::SymbolInstance, def: &SymbolDef) -> (Point, Point) {
    let (mut min, mut max) = symbol_bounds(inst, def);
    if !crate::block::is_block_export_id(&def.id) {
        return (min, max);
    }
    for prim in &def.primitives {
        if let Primitive::Text { at, height, .. } = prim {
            let c = transform_local(*at, inst);
            min.y = min.y.min(c.y - height / 2.0);
            max.y = max.y.max(c.y + height / 2.0);
        }
    }
    (min, max)
}

/// 配置後のシンボル外形の囲み矩形 (最小点, 最大点) を用紙座標で返す。
/// 円・弧は回転対称なので中心±rで安全側に評価する。配置基準点も必ず含める。
/// 参照記号・接点マップなど、外形の外側に置く注記の位置決めに使う。
///
/// ピングループ括弧 (G8) もシンボルと一緒に描かれる外形の一部なので、括弧とキャプションの
/// 箱を含める。こうしないと、回転したシンボルで参照記号・型番が括弧の文字に重なる。
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
    for (a, b) in crate::pin_groups::pin_group_paper_boxes(inst, def) {
        min.x = min.x.min(a.x);
        min.y = min.y.min(a.y);
        max.x = max.x.max(b.x);
        max.y = max.y.max(b.y);
    }
    (min, max)
}

fn render_symbol(out: &mut String, inst: &crate::model::SymbolInstance, def: &SymbolDef) {
    // 端子番号の上書き (Task 2, spec §1.1): シンボル定義に焼き込まれた生のピン番号の
    // テキストを、このインスタンスの表示ラベルへ差し替える。上書きが無ければNoneで、
    // 差し替えは起きない (端子台以外のシンボルも含めて、いつもどおりの描画になる)。
    let terminal_labels = crate::terminal_chart::TerminalLabels::of(def, &inst.attrs);
    let terminal_rename = terminal_labels.display_map();
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
                // 文字は常に水平に書くので、ベースラインへのずらし (文字高の半分) は
                // シンボルと一緒に回さず、用紙の座標系で下へ取る
                let p = transform_local(*at, inst);
                let label = terminal_rename
                    .and_then(|m| m.get(text.as_str()))
                    .map(String::as_str)
                    .unwrap_or(text.as_str());
                text_el(out, p.x, p.y + height / 2.0, *height, "#000", "middle", label);
            }
        }
    }
    // 参照記号と型番/値をシンボル外形の上に併記(縦長シンボルでも重ならない)
    let top = symbol_top_y(inst, def);
    if inst.shows_reference() {
        text_el(out, inst.at.x, top - REF_LABEL_DY, LABEL_FONT, "#000", "middle", &inst.reference);
    }
    if !inst.value.is_empty() {
        text_el(out, inst.at.x, top - VALUE_LABEL_DY, LABEL_FONT, "#000", "middle", &inst.value);
    }
    // 機能ラベル (G4の第3テキスト欄)。参照記号・型番の反対側=外形の下に置く
    if let Some(label) = crate::text_style::function_label_placement(inst, def) {
        let text = crate::text_style::function_label(inst).unwrap_or_default();
        text_el(out, label.at.x, label.at.y, label.height, "#000", "middle", text);
    }
}

/// 端子台のジャンパバー (G5): `attrs["jumpers"]` を、隣り合う端子の印の隙間を埋める
/// 太い線(ショートバー)としてシンボルの上に描く。パース・妥当性検証は端子台チャートと同じ
/// [`crate::terminal_chart::parse_jumpers`] を再利用するので、隣り合わない端子どうしの
/// 指定は(チャート同様)描かれない。
fn render_jumper_bars(out: &mut String, inst: &crate::model::SymbolInstance, def: &SymbolDef) {
    if !crate::terminal_chart::is_terminal_block(&inst.symbol_id) {
        return;
    }
    let raw_terminals = crate::terminal_chart::terminal_numbers_of(def);
    let display_terminals = crate::terminal_chart::terminal_numbers_of_instance(def, &inst.attrs);
    let jumpers = crate::terminal_chart::parse_jumpers(
        inst.attrs.get("jumpers").map(String::as_str).unwrap_or(""),
        &display_terminals,
        &raw_terminals,
    );
    // ジオメトリ (jumper_bars) はシンボル定義の生のピン番号でしか端子を探せないので、
    // 表示ラベルの組を生の番号の組へ戻してから渡す (端子番号の上書き = Task 2)
    let disp_to_raw: std::collections::BTreeMap<&str, &str> = display_terminals
        .iter()
        .map(String::as_str)
        .zip(raw_terminals.iter().map(String::as_str))
        .collect();
    let raw_pairs: Vec<(String, String)> = jumpers
        .pairs
        .iter()
        .map(|(a, b)| {
            (
                disp_to_raw.get(a.as_str()).map(|s| s.to_string()).unwrap_or_else(|| a.clone()),
                disp_to_raw.get(b.as_str()).map(|s| s.to_string()).unwrap_or_else(|| b.clone()),
            )
        })
        .collect();
    for (a, b) in crate::terminal_chart::jumper_bars(def, &raw_pairs) {
        let (pa, pb) = (transform_local(a, inst), transform_local(b, inst));
        line_el(out, pa.x, pa.y, pb.x, pb.y, JUMPER_BAR_STROKE, "#000");
    }
}

/// ピングループ括弧 (G8): `attrs["pin_groups"]`が指す範囲ごとに角括弧とキャプションを描く。
/// 読めない・重なる項目はジオメトリ側 ([`crate::pin_groups::pin_group_brackets`]) が既に
/// 除いているので、ここでは求まったものをそのまま描くだけ (パニックしない)。
fn render_pin_group_brackets(out: &mut String, inst: &crate::model::SymbolInstance, def: &SymbolDef) {
    let Some(spec) = inst.attrs.get(crate::pin_groups::PIN_GROUPS_ATTR) else {
        return;
    };
    let labels = crate::terminal_chart::TerminalLabels::of(def, &inst.attrs);
    for bracket in crate::pin_groups::pin_group_brackets(def, spec, labels.display_map()) {
        let pts: Vec<Point> = bracket.bracket.iter().map(|p| transform_local(*p, inst)).collect();
        let _ = write!(
            out,
            "<polyline points=\"{}\" fill=\"none\" stroke=\"#000\" stroke-width=\"{}\"/>\n",
            polyline_points(&pts),
            n(PIN_GROUP_BRACKET_STROKE)
        );
        // キャプションの行は用紙座標で積む (文字は回転しても横書きなので、ローカル座標で
        // 積むと回転90/270度で行が横に並んで重なる)
        for (text, p) in crate::pin_groups::caption_line_anchors(inst, &bracket) {
            text_el(
                &mut *out,
                p.x,
                p.y + crate::pin_groups::PIN_GROUP_CAPTION_TEXT_MM / 2.0,
                crate::pin_groups::PIN_GROUP_CAPTION_TEXT_MM,
                "#000",
                "middle",
                &text,
            );
        }
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

/// コイルを実機ブロック (G7) として描く: 参照記号+型番のヘッダと、接点の「機器イメージ」
/// (端子対+所在) を1つの外形にまとめる。コイル自身の小さな図形 (rect+リード) の代わりに
/// これを描く。
///
/// 表 (外形・仕切り線・文字) は**常に正立**で、インスタンスの回転・ミラーでは向きが変わらない
/// (回すと複数行の文字が互いに重なって読めなくなるため)。回るのはピンの位置だけで、
/// 引き出し線はシンボル定義の実際のピン (A1/A2) を回した先から、**ピン自身の軸**
/// (配線が来る向き) に沿って外形まで引く ([`crate::relay_xref::device_block_lead_end`])。
/// どの向きでも配線は繋がったまま。
fn render_device_block(
    out: &mut String,
    inst: &crate::model::SymbolInstance,
    def: &SymbolDef,
    block: &crate::relay_xref::DeviceBlock,
) {
    use crate::relay_xref::{
        device_block_bounds, device_block_layout, device_block_lead_axis, device_block_lead_end,
        device_block_texts, CONTACT_MAP_FONT,
    };
    use crate::wire_note::NoteAlign;
    let layout = device_block_layout(block);
    let (min, max) = device_block_bounds(inst, block);
    let _ = write!(
        out,
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"#000\" stroke-width=\"{}\"/>\n",
        n(min.x), n(min.y), n(max.x - min.x), n(max.y - min.y), n(SYMBOL_STROKE)
    );
    // ピン (A1/A2) から外形の縁までの引き出し線。ピン自身の軸 (配線と同じ向き) に沿って引く
    for pin in &def.pins {
        let at = transform_local(pin.at, inst);
        let tip = transform_local(device_block_lead_axis(pin.at), inst);
        let axis = Point::new(tip.x - inst.at.x, tip.y - inst.at.y);
        if let Some(end) = device_block_lead_end(at, axis, (min, max)) {
            line_el(out, at.x, at.y, end.x, end.y, SYMBOL_STROKE, "#000");
        }
    }
    // ヘッダ行 (参照記号、型番があればもう1行) と接点の行を仕切る線
    let rule = |out: &mut String, x0: f64, y0: f64, x1: f64, y1: f64| {
        line_el(out, x0, y0, x1, y1, RULE_STROKE, "#000");
    };
    let header_bottom = min.y + layout.row_h * layout.header_rows as f64;
    for i in 1..layout.header_rows {
        let y = min.y + layout.row_h * i as f64;
        rule(out, min.x, y, max.x, y);
    }
    if layout.contact_rows > 0 {
        rule(out, min.x, header_bottom, max.x, header_bottom);
        // 接点の列の仕切り
        let col1_x = inst.at.x + layout.col_x(1);
        rule(out, col1_x, header_bottom, col1_x, max.y);
        for i in 1..layout.contact_rows {
            let y = inst.at.y + layout.contact_row_top(i);
            rule(out, min.x, y, max.x, y);
        }
    }
    // 文字 (ヘッダは中央揃え、接点の行は左揃えの等幅)
    for t in device_block_texts(inst, block) {
        match t.align {
            NoteAlign::Center => {
                text_el(out, t.at.x, t.at.y, CONTACT_MAP_FONT, "#000", "middle", &t.text)
            }
            NoteAlign::Left => text_family_el(
                out,
                t.at.x,
                t.at.y,
                CONTACT_MAP_FONT,
                "#000",
                "start",
                "monospace",
                &t.text,
            ),
        }
    }
    // 機能ラベル (G4): 分散配置と同じく外形の下に置く (ここではブロックの外形の下)
    if let Some(label) =
        crate::text_style::function_label_below(inst, (min.x + max.x) / 2.0, max.y)
    {
        let text = crate::text_style::function_label(inst).unwrap_or_default();
        text_el(out, label.at.x, label.at.y, label.height, "#000", "middle", text);
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
    // 文字も配線からの逃がしも、シートの注記文字高 (G20) に合わせて一緒に伸縮する
    let scale = sheet.annotation_scale();
    let (font, gap) = (WIRE_NO_FONT * scale, WIRE_NO_GAP * scale);
    for net in crate::netlist::extract_wire_nets(sheet, symbols) {
        let Some(no) = net.wire_no.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
            continue;
        };
        let Some((a, b)) = longest_segment(sheet, &net.wire_ids) else {
            continue;
        };
        let mid = midpoint(&a, &b);
        if (b.x - a.x).abs() >= (b.y - a.y).abs() {
            text_family_el(out, mid.x, mid.y - gap, font, "#000", "middle", "monospace", no);
        } else {
            text_family_el(out, mid.x - gap, mid.y, font, "#000", "end", "monospace", no);
        }
    }
}

/// 配線注記 (G2)。ワイヤ1本ごとに線径・線色・電線長を、線番の反対側 (横線=下、縦線=右) へ描く。
/// 位置と文字列は [`crate::wire_note`] が決める (キャンバスと同じ数式)。
/// **画面の表示クラスとは無関係に常に出力する**: 製造用のハーネス図面ではこれが主情報だから。
fn render_wire_notes(out: &mut String, sheet: &Sheet) {
    for note in crate::wire_note::wire_notes(sheet) {
        let anchor = match note.align {
            crate::wire_note::NoteAlign::Center => "middle",
            crate::wire_note::NoteAlign::Left => "start",
        };
        text_family_el(
            out,
            note.at.x,
            note.at.y,
            crate::wire_note::wire_note_font(sheet),
            "#000",
            anchor,
            "monospace",
            &note.text,
        );
    }
}

/// 図形 (G10) を描く。矩形・線・円・折れ線を、指定された色トークンの印刷色・線幅・
/// 破線・塗りで。配線・シンボルより先に描いて**背面**へ置くので、塗りつぶした強調枠が
/// その上の回路を隠すことはない (塗りは下が透ける濃さ = [`crate::shape::SHAPE_FILL_OPACITY`])。
///
/// 電気的な意味は一切持たない要素なので、ここは純粋に「言われたとおりに描く」だけ。
fn render_shapes(out: &mut String, sheet: &Sheet) {
    for shape in crate::shape::shapes(sheet) {
        if crate::shape::validate(shape).is_err() {
            // 壊れた図形 (頂点不足など) は描かない。図面の他の部分は今までどおり出す
            continue;
        }
        let stroke = crate::shape::stroke_print_color(shape);
        let width = n(shape.stroke_width_mm);
        let fill = shape_fill_attrs(shape);
        let dash = if shape.dashed {
            format!(
                " stroke-dasharray=\"{} {}\"",
                n(crate::shape::SHAPE_DASH.0),
                n(crate::shape::SHAPE_DASH.1)
            )
        } else {
            String::new()
        };
        match shape.shape {
            ShapeKind::Rect => {
                let Some((min, max)) = crate::shape::rect_corners(shape) else {
                    continue;
                };
                let _ = write!(
                    out,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" {fill} stroke=\"{stroke}\" stroke-width=\"{width}\"{dash}/>\n",
                    n(min.x), n(min.y), n(max.x - min.x), n(max.y - min.y)
                );
            }
            ShapeKind::Line => {
                let (a, b) = (shape.points[0], shape.points[1]);
                let _ = write!(
                    out,
                    "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{stroke}\" stroke-width=\"{width}\"{dash}/>\n",
                    n(a.x), n(a.y), n(b.x), n(b.y)
                );
            }
            ShapeKind::Circle => {
                let Some((center, r)) = crate::shape::circle(shape) else {
                    continue;
                };
                let _ = write!(
                    out,
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" {fill} stroke=\"{stroke}\" stroke-width=\"{width}\"{dash}/>\n",
                    n(center.x), n(center.y), n(r)
                );
            }
            ShapeKind::Polyline => {
                let _ = write!(
                    out,
                    "<polyline points=\"{}\" {fill} stroke=\"{stroke}\" stroke-width=\"{width}\"{dash}/>\n",
                    polyline_points(&shape.points)
                );
            }
        }
    }
}

/// 図形の塗りのSVG属性。塗り無しは`fill="none"` (面を持たない線・折れ線もこれ)。
fn shape_fill_attrs(shape: &crate::model::Shape) -> String {
    match crate::shape::fill_print_color(shape) {
        Some(hex) => format!(
            "fill=\"{hex}\" fill-opacity=\"{}\"",
            n(crate::shape::SHAPE_FILL_OPACITY)
        ),
        None => "fill=\"none\"".to_string(),
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

/// 矩形の周上で、外部の点`target`にいちばん近い点 (G14の「外形どうしの最短点」)。
/// `target`が矩形の外にあれば単純にクランプ、矩形の内側 (通常は起きないが保険) なら
/// いちばん近い辺へ押し出す。
fn rect_boundary_point_toward(min: Point, max: Point, target: Point) -> Point {
    let cx = target.x.clamp(min.x, max.x);
    let cy = target.y.clamp(min.y, max.y);
    let inside = cx > min.x && cx < max.x && cy > min.y && cy < max.y;
    if !inside {
        return Point::new(cx, cy);
    }
    let (dl, dr, dt, db) = (cx - min.x, max.x - cx, cy - min.y, max.y - cy);
    let m = dl.min(dr).min(dt).min(db);
    if m == dl {
        Point::new(min.x, cy)
    } else if m == dr {
        Point::new(max.x, cy)
    } else if m == dt {
        Point::new(cx, min.y)
    } else {
        Point::new(cx, max.y)
    }
}

/// 嵌合コネクタ対 (G14)。両方が同一シート上で50mm以内にある対だけが
/// [`crate::mate::mate_links`] に入っているので、ここでは渡された分をそのまま描く:
/// 双方の外形 ([`symbol_bounds`]) のうち、相手の配置基準点にいちばん近い点どうしを
/// 短い破線で結ぶ。配線・シンボルより先に描いて背面に置く (ハーネス境界と同じ扱い)。
fn render_mate_links(
    out: &mut String,
    sheet: &Sheet,
    links: &[crate::mate::MateLink],
    defs: &crate::symbol::DefIndex,
) {
    for link in links {
        let (Some(Entity::Symbol(a)), Some(Entity::Symbol(b))) =
            (sheet.entities.get(&link.a), sheet.entities.get(&link.b))
        else {
            continue;
        };
        let (Some(def_a), Some(def_b)) = (defs.get(a), defs.get(b)) else {
            continue;
        };
        let (a_min, a_max) = symbol_bounds(a, &def_a);
        let (b_min, b_max) = symbol_bounds(b, &def_b);
        let pa = rect_boundary_point_toward(a_min, a_max, b.at);
        let pb = rect_boundary_point_toward(b_min, b_max, a.at);
        let _ = write!(
            out,
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#000\" stroke-width=\"{}\" stroke-dasharray=\"{} {}\"/>\n",
            n(pa.x), n(pa.y), n(pb.x), n(pb.y),
            n(MATE_LINE_STROKE), n(MATE_LINE_DASH.0), n(MATE_LINE_DASH.1)
        );
    }
}

/// 盤レイアウト (M4仕様 §8) を描く。奥から順に **盤外形+取付板 → ダクト → DINレール →
/// フットプリント**。寸法の数式は [`crate::panel`] が正で、画面 (`src/canvas/panel.ts`) と
/// 同じ結果になる。紙面なので白地・黒線で、面は塗らない (ダクトは斜線で表す)。
fn render_panel(
    out: &mut String,
    sheet: &Sheet,
    descriptions: &std::collections::BTreeMap<String, String>,
) {
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
    render_balloons(out, sheet);
    render_legends(out, sheet);
    render_nameplates_and_dimensions(out, sheet, descriptions);
}

/// 銘板 (枠+中央揃えの文字) と関連付き寸法線 (補助線・寸法線・端の斜線・値) を描く。
fn render_nameplates_and_dimensions(
    out: &mut String,
    sheet: &Sheet,
    descriptions: &std::collections::BTreeMap<String, String>,
) {
    for np in crate::panel::nameplates(sheet) {
        let Some((min, max)) = crate::panel::nameplate_rect(sheet, np) else {
            continue;
        };
        rect_el(out, min.x, min.y, max.x - min.x, max.y - min.y, "#fff", "#000", RULE_STROKE);
        let lines = crate::panel::nameplate_lines(sheet, np, descriptions);
        let n = lines.len().max(1) as f64;
        let pitch = np.text_mm * 1.3;
        let top = (min.y + max.y) / 2.0 - pitch * (n - 1.0) / 2.0 + np.text_mm * 0.35;
        for (i, line) in lines.iter().enumerate() {
            text_el(out, (min.x + max.x) / 2.0, top + i as f64 * pitch, np.text_mm, "#000", "middle", line);
        }
    }
    for d in crate::panel::dimensions(sheet) {
        let Some(g) = crate::panel::dimension_geometry(sheet, d) else {
            continue;
        };
        for (a, b) in g.extensions {
            line_el(out, a.x, a.y, b.x, b.y, DUCT_HATCH_STROKE, "#000");
        }
        let (a, b) = g.line;
        line_el(out, a.x, a.y, b.x, b.y, DUCT_HATCH_STROKE, "#000");
        let t = crate::panel::DIM_TICK / 2.0;
        for p in [a, b] {
            line_el(out, p.x - t, p.y + t, p.x + t, p.y - t, DUCT_HATCH_STROKE, "#000");
        }
        text_el(out, g.text_at.x, g.text_at.y, crate::panel::DIM_FONT, "#000", "middle", &g.text);
    }
}

/// バルーン (アイテム番号) を描く。採番済みのフットプリントだけに付き、
/// **白丸の下は塗りつぶす** (下の図面が透けて番号が読めなくならないため)。
fn render_balloons(out: &mut String, sheet: &Sheet) {
    for fp in crate::panel::footprints(sheet) {
        let Some(b) = crate::panel::balloon(fp) else {
            continue;
        };
        line_el(
            out,
            b.leader_from.x,
            b.leader_from.y,
            b.leader_to.x,
            b.leader_to.y,
            RULE_STROKE,
            "#000",
        );
        let _ = write!(
            out,
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"#fff\" stroke=\"#000\" stroke-width=\"{}\"/>\n",
            n(b.center.x),
            n(b.center.y),
            n(crate::panel::BALLOON_R),
            n(SYMBOL_STROKE)
        );
        text_family_el(
            out,
            b.center.x,
            b.center.y + crate::panel::BALLOON_FONT / 2.0,
            crate::panel::BALLOON_FONT,
            "#000",
            "middle",
            "monospace",
            &b.item_no.to_string(),
        );
    }
}

/// 凡例表 (アイテム番号↔参照記号・型番) を描く。中身は同じシートのフットプリントから
/// 導出する ([`crate::panel::legend_rows`]) ので、図面と食い違わない。
fn render_legends(out: &mut String, sheet: &Sheet) {
    let rows = crate::panel::legend_rows(sheet);
    for legend in crate::panel::legends(sheet) {
        let (min, max) = crate::panel::legend_rect(sheet, legend);
        let (w, h) = (max.x - min.x, max.y - min.y);
        rect_el(out, min.x, min.y, w, h, "#fff", "#000", FRAME_STROKE);
        let row_h = crate::panel::LEGEND_ROW_H;
        let font = crate::panel::LEGEND_FONT;
        // 列の区切り線
        let mut x = min.x;
        for cw in crate::panel::LEGEND_COL_W.iter().take(crate::panel::LEGEND_COL_W.len() - 1) {
            x += cw;
            line_el(out, x, min.y, x, max.y, RULE_STROKE, "#000");
        }
        // 列見出し + 本文 (見出しの下は太めの罫線で区切る)
        line_el(out, min.x, min.y + row_h, max.x, min.y + row_h, FRAME_STROKE, "#000");
        let cells = std::iter::once(
            crate::panel::LEGEND_COLUMNS
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
        )
        .chain(rows.iter().cloned());
        for (r, row) in cells.enumerate() {
            let top = min.y + row_h * r as f64;
            if r > 1 {
                line_el(out, min.x, top, max.x, top, RULE_STROKE, "#000");
            }
            let base = top + row_h / 2.0 + font / 2.0;
            let mut cx = min.x;
            for (i, cell) in row.iter().enumerate() {
                if !cell.is_empty() {
                    text_el(out, cx + CELL_PAD, base, font, "#000", "start", cell);
                }
                cx += crate::panel::LEGEND_COL_W[i];
            }
        }
    }
}

/// 電線品番凡例ブロック (色・sq・電線品番・使用長のテーブル、G9残) を描く。中身は同じ
/// シートの配線から導出する ([`crate::wire_legend::legend_rows_with`]) ので、図面と
/// 食い違わない。`wire_parts`はプロジェクトの電線品番表 (呼び出し元から渡す)。
fn render_wire_legends(out: &mut String, sheet: &Sheet, wire_parts: &[crate::model::WirePart]) {
    let rows = crate::wire_legend::legend_rows_with(sheet, wire_parts);
    for legend in crate::wire_legend::legends(sheet) {
        let (min, max) = crate::wire_legend::legend_rect_with(sheet, wire_parts, legend);
        let (w, h) = (max.x - min.x, max.y - min.y);
        // 回路図の表 (接点マップ等) と同じく塗りは無し。下の配線・記号が透けて見える
        rect_el(out, min.x, min.y, w, h, "none", "#000", FRAME_STROKE);
        let row_h = crate::wire_legend::LEGEND_ROW_H;
        let font = crate::wire_legend::LEGEND_FONT;
        let mut x = min.x;
        for cw in crate::wire_legend::LEGEND_COL_W
            .iter()
            .take(crate::wire_legend::LEGEND_COL_W.len() - 1)
        {
            x += cw;
            line_el(out, x, min.y, x, max.y, RULE_STROKE, "#000");
        }
        line_el(out, min.x, min.y + row_h, max.x, min.y + row_h, FRAME_STROKE, "#000");
        let cells = std::iter::once(
            crate::wire_legend::LEGEND_COLUMNS
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
        )
        .chain(rows.iter().cloned());
        for (r, row) in cells.enumerate() {
            let top = min.y + row_h * r as f64;
            if r > 1 {
                line_el(out, min.x, top, max.x, top, RULE_STROKE, "#000");
            }
            let base = top + row_h / 2.0 + font / 2.0;
            let mut cx = min.x;
            for (i, cell) in row.iter().enumerate() {
                if !cell.is_empty() {
                    text_el(out, cx + CELL_PAD, base, font, "#000", "start", cell);
                }
                cx += crate::wire_legend::LEGEND_COL_W[i];
            }
        }
    }
}

/// 区間ケーブルを描く (形は [`crate::cable`] の `run_*` が決め、作図領域 `src/canvas/cable.ts` と同じ)。
fn render_run_cable(out: &mut String, sheet: &Sheet, symbols: &[SymbolDef], c: &crate::model::Cable) {
    let _ = write!(
        out,
        "<polyline points=\"{}\" fill=\"none\" stroke=\"#000\" stroke-width=\"{}\"/>\n",
        polyline_points(&c.run),
        n(crate::cable::RUN_STROKE_MM)
    );
    if let Some((a, b)) = crate::cable::run_slash(c) {
        line_el(out, a.x, a.y, b.x, b.y, CABLE_STROKE, "#000");
    }
    if let Some(at) = crate::cable::run_count_at(c) {
        text_el(out, at.x, at.y, 2.0, "#000", "start", &c.core_count.to_string());
    }
    for (a, b) in crate::cable::run_bars(sheet, symbols, c) {
        line_el(out, a.x, a.y, b.x, b.y, crate::cable::RUN_STROKE_MM, "#000");
    }
    for (a, b, core) in crate::cable::run_fans(sheet, c) {
        line_el(out, a.x, a.y, b.x, b.y, WIRE_STROKE, "#000");
        text_el(out, b.x, b.y - 0.8, crate::cable::CORE_LABEL_MM, "#000", "middle", &core);
    }
    if c.shield.is_some() {
        if let Some((center, rx, ry, angle)) = crate::cable::shield_ellipse(c) {
            let _ = write!(
                out,
                "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" transform=\"rotate({} {} {})\" fill=\"none\" stroke=\"#000\" stroke-width=\"{}\"/>\n",
                n(center.x), n(center.y), n(rx), n(ry), n(angle), n(center.x), n(center.y), n(CABLE_STROKE)
            );
        }
    }
    if let Some(at) = crate::cable::run_label_at(c) {
        text_el(out, at.x, at.y, 2.5, "#000", "start", &crate::cable::cable_label(c));
    }
}

/// シート1枚を完全なSVG文書として書き出す。座標系はmm 1:1。図枠は既定の`jis-a3`。
/// シート単体の出力なのでシート間クロスリファレンスは描かない
/// (描くには [`project_sheet_to_svg`] を使う)。
///
/// `wire_parts`はプロジェクトの電線品番表 ([`crate::model::Project::wire_parts`])。
/// 電線品番凡例ブロック (G9) の品番欄はこの表から引くので、**プロジェクトを持っている
/// 呼び出し元は必ず渡すこと**(空スライスを渡すと、ワイヤ自身が品番を持たない限り
/// 凡例の品番欄が空欄になる)。
pub fn sheet_to_svg(sheet: &Sheet, symbols: &[SymbolDef], wire_parts: &[crate::model::WirePart]) -> String {
    sheet_to_svg_with_frame(
        sheet,
        symbols,
        crate::frame_template::default_template(),
        wire_parts,
    )
}

/// 図枠テンプレートを指定してシート1枚を書き出す(図枠のプレビュー・テスト用)。
/// `wire_parts`は[`sheet_to_svg`]と同じ。
pub fn sheet_to_svg_with_frame(
    sheet: &Sheet,
    symbols: &[SymbolDef],
    frame: &FrameTemplate,
    wire_parts: &[crate::model::WirePart],
) -> String {
    sheet_to_svg_with_xrefs(
        sheet,
        symbols,
        &std::collections::BTreeMap::new(),
        &std::collections::BTreeMap::new(),
        &RelayAnnotations::default(),
        &[],
        frame,
        &BindContext::for_sheet(sheet),
        wire_parts,
    )
}

/// リレーのコイル⇔接点クロスリファレンス注記 ([`crate::relay_xref`])。
/// プロジェクト全体を見ないと決まらないので、シート単体の出力では空になる。
#[derive(Debug, Clone, Default)]
pub struct RelayAnnotations {
    /// コイルのentity id → その下に描く接点マップの行 (分散配置のコイルだけ)。
    pub contact_maps: std::collections::BTreeMap<
        crate::model::EntityId,
        Vec<crate::relay_xref::ContactMapRow>,
    >,
    /// 接点のentity id → 脇に描くコイル所在 (例 "(/1.C2)")。
    pub coil_locations: std::collections::BTreeMap<crate::model::EntityId, String>,
    /// コイルのentity id → 実機ブロック表示の中身 (G7、`attrs["device_block"]`のコイルだけ)。
    pub device_blocks:
        std::collections::BTreeMap<crate::model::EntityId, crate::relay_xref::DeviceBlock>,
    /// 参照記号 → 回路図の説明 (盤レイアウトの銘板の1行目を追従させる。M4 §8)。
    pub device_descriptions: std::collections::BTreeMap<String, String>,
}

/// プロジェクト内の1シートをSVGとして書き出す。図枠はプロジェクトの図枠テンプレート
/// ([`crate::model::Project::frame_template`])。ネットラベルの脇には、他シートにある
/// 同名ラベルの住所「/シート.ゾーン」([`crate::xref`]) を自動で添える。デバイス単位の
/// クロスリファレンス (G13、`crate::xref::sheet_device_xrefs`) も同様に添える。
/// シートが見つからなければNone。
pub fn project_sheet_to_svg(
    project: &crate::model::Project,
    sheet_id: crate::model::SheetId,
    symbols: &[SymbolDef],
) -> Option<String> {
    let sheet = project.sheet(sheet_id)?;
    let xrefs = crate::xref::sheet_xrefs(project, sheet_id);
    let device_xrefs = crate::xref::sheet_device_xrefs(project, sheet_id);
    let relay = RelayAnnotations {
        contact_maps: crate::relay_xref::sheet_contact_maps(project, sheet_id),
        coil_locations: crate::relay_xref::sheet_coil_locations(project, sheet_id),
        device_blocks: crate::relay_xref::sheet_device_blocks(project, sheet_id),
        device_descriptions: crate::panel::device_descriptions(project),
    };
    let frame = crate::frame_template::for_project(project);
    let mate_links: Vec<crate::mate::MateLink> = crate::mate::mate_links(project)
        .into_iter()
        .filter(|l| l.sheet_id == sheet_id)
        .collect();
    Some(sheet_to_svg_with_xrefs(
        sheet,
        symbols,
        &xrefs,
        &device_xrefs,
        &relay,
        &mate_links,
        &frame,
        &BindContext::for_project(project, sheet),
        &project.wire_parts,
    ))
}

/// [`sheet_to_svg`] の本体。`xrefs`はネットラベルのentity id → 脇に描くXRefテキスト、
/// `device_xrefs`はシンボルのentity id → 脇に描くデバイス単位XRefテキスト (G13)、
/// `relay`はコイル⇔接点のクロスリファレンス注記、`frame`/`ctx`は図枠テンプレートと
/// 表題欄バインドの文脈。
fn sheet_to_svg_with_xrefs(
    sheet: &Sheet,
    symbols: &[SymbolDef],
    xrefs: &std::collections::BTreeMap<crate::model::EntityId, String>,
    device_xrefs: &std::collections::BTreeMap<crate::model::EntityId, String>,
    relay: &RelayAnnotations,
    mate_links: &[crate::mate::MateLink],
    frame: &FrameTemplate,
    ctx: &BindContext,
    wire_parts: &[crate::model::WirePart],
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
    render_panel(&mut out, sheet, &relay.device_descriptions);
    // 図形 (G10) は配線・シンボルの背面に置く (塗りつぶしの強調枠が回路を隠さない)
    render_shapes(&mut out, sheet);
    // ハーネス境界は配線・シンボルの背面に置く
    render_harnesses(&mut out, sheet);
    // 注記の層 (ネットラベル・タグ・線番・配線注記) にかける倍率 (G20)
    let annotation_scale = sheet.annotation_scale();
    let defs = crate::symbol::DefIndex::new(symbols);
    // 嵌合コネクタ対 (G14) の破線もハーネス境界と同じく背面に置く
    render_mate_links(&mut out, sheet, mate_links, &defs);
    // コイル所在 (G22) の逃がし判定に使う、このシートの配線注記と、その文字高 (G20で伸縮)
    let wire_notes = crate::wire_note::wire_notes(sheet);
    let wire_note_font = crate::wire_note::wire_note_font(sheet);
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
                    if let Some(block) = relay.device_blocks.get(&s.id) {
                        // 実機ブロック表示 (G7): コイル自身の小さな図形の代わりにこれを描く
                        render_device_block(&mut out, s, &def, block);
                    } else {
                        render_symbol(&mut out, s, &def);
                        render_jumper_bars(&mut out, s, &def);
                        render_pin_group_brackets(&mut out, s, &def);
                        // コイル⇔接点クロスリファレンス (M4 §4、分散配置のコイルだけ)
                        if let Some(rows) = relay.contact_maps.get(&s.id) {
                            render_contact_map(&mut out, s, &def, rows);
                        }
                    }
                    if let Some(location) = relay.coil_locations.get(&s.id) {
                        let at = crate::relay_xref::coil_location_at(s, &def, location, &wire_notes, wire_note_font);
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
                    // デバイス単位シート間クロスリファレンス (G13)。コイル・接点は
                    // relay_xrefが既に自動注記するので、sheet_device_xrefsの対象から外れている
                    if let Some(text) = device_xrefs.get(&s.id) {
                        let at = crate::xref::device_xref_at(s, &def);
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
            }
            Entity::Junction(j) => {
                let _ = write!(
                    out,
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"#000\"/>\n",
                    n(j.at.x), n(j.at.y), n(JUNCTION_DOT_R)
                );
            }
            Entity::NetLabel(l) => {
                // ネットラベルの文字はシートの注記文字高 (G20) に従って伸縮する
                let label_font = LABEL_FONT * annotation_scale;
                // 方向付きタグ (G3)。attrs["tag"]がある時だけ旗の枠で囲む
                let tag = crate::net_tag::net_tag(l, label_font);
                match &tag {
                    Some(t) => {
                        let _ = write!(
                            out,
                            "<polygon points=\"{}\" fill=\"none\" stroke=\"#000\" stroke-width=\"{}\"/>\n",
                            polyline_points(&t.points),
                            n(crate::net_tag::TAG_STROKE)
                        );
                        text_el(&mut out, t.text_at.x, t.text_at.y, label_font, "#000", "start", &l.name);
                    }
                    None => {
                        let dy = NET_LABEL_DY * annotation_scale;
                        text_el(&mut out, l.at.x, l.at.y - dy, label_font, "#000", "start", &l.name);
                    }
                }
                // シート間クロスリファレンス (IEC 61082-1)。相手がいるラベルだけ脇に添える
                if let Some(text) = xrefs.get(&l.id) {
                    // タグ付きは旗の本体側の外へ逃がす (枠にも自分の配線にも重ねない)
                    let (at, anchor) = match &tag {
                        Some(t) => (t.xref_at, crate::text_style::svg_anchor(t.xref_anchor)),
                        None => (crate::xref::xref_text_at(l, label_font), "start"),
                    };
                    text_family_el(
                        &mut out,
                        at.x,
                        at.y,
                        crate::xref::XREF_FONT * annotation_scale,
                        "#000",
                        anchor,
                        "monospace",
                        text,
                    );
                }
            }
            Entity::Text(t) => {
                // 注記の装飾 (G4): 基準位置・色トークン・太字。既定の注記は従来と同じ出力
                text_weight_el(
                    &mut out,
                    t.at.x,
                    t.at.y,
                    t.height,
                    crate::text_style::text_fill(t),
                    crate::text_style::svg_anchor(t.anchor),
                    if t.bold { Weight::Bold } else { Weight::Normal },
                    &t.text,
                );
                // 引き出し線 (G20): 対象がシート上に実在すれば、注記の辺から対象の外形まで
                // 毎回求め直して引く (位置は保存しないので、どちらを動かしても追従する)
                if let Some((start, end)) = crate::leader::leader_for(sheet, t, &defs) {
                    line_el(&mut out, start.x, start.y, end.x, end.y, LEADER_STROKE, "#000");
                    let _ = write!(
                        out,
                        "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"#000\"/>\n",
                        n(end.x), n(end.y), n(JUNCTION_DOT_R)
                    );
                }
            }
            // 図形は背面のrender_shapes、ハーネス境界は背面のrender_harnesses、
            // 盤レイアウトはrender_panelで描き済み、電線品番凡例ブロックは
            // 末尾のrender_wire_legendsで描き済み
            Entity::Shape(_)
            | Entity::Harness(_)
            | Entity::PanelOutline(_)
            | Entity::DinRail(_)
            | Entity::Duct(_)
            | Entity::Footprint(_)
            | Entity::PanelLegend(_)
            | Entity::Nameplate(_)
            | Entity::Dimension(_)
            | Entity::WireLegend(_) => {}
            // 1本線の区間を持つケーブル: 太い線・芯数の斜線・部品側の棒・分岐の扇と芯の呼び名・注記
            Entity::Cable(c) if c.is_run() => render_run_cable(&mut out, sheet, symbols, c),
            // 多芯ケーブル: 定義線 (配線を斜めに横切る線)・シールドの楕円・注記
            Entity::Cable(c) => {
                if let [a, b, ..] = c.points.as_slice() {
                    line_el(&mut out, a.x, a.y, b.x, b.y, CABLE_STROKE, "#000");
                    if let Some((center, rx, ry, angle)) = crate::cable::shield_ellipse(c) {
                        let _ = write!(
                            out,
                            "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" transform=\"rotate({} {} {})\" fill=\"none\" stroke=\"#000\" stroke-width=\"{}\"/>\n",
                            n(center.x), n(center.y), n(rx), n(ry), n(angle), n(center.x), n(center.y), n(CABLE_STROKE)
                        );
                    }
                    let top = if a.y <= b.y { a } else { b };
                    text_el(&mut out, top.x + 1.5, top.y - 1.5, 2.5, "#000", "start", &crate::cable::cable_label(c));
                }
            }
        }
    }
    render_wire_numbers(&mut out, sheet, symbols);
    render_wire_notes(&mut out, sheet);
    render_wire_legends(&mut out, sheet, wire_parts);
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
            length_source: Default::default(),
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
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
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
        let svg = sheet_to_svg(&sheet, &crate::symbol::sheet_symbol_defs(&sheet), &[]);
        assert!(svg.contains(">IN1<"), "ピン番号が描かれていない");
        assert!(svg.contains(">入力1<"), "ピン名が描かれていない");
        assert!(svg.contains(">OUT1<") && svg.contains(">出力1<"));
        assert!(svg.contains(">CTRL1<"), "参照記号が描かれていない");
        assert!(svg.contains("polygon"), "外形の長方形が描かれていない");
    }

    /// Regression test for Task 5 (already passes): a plug's black insert is drawn as one more black-filled polygon than the same block without the connector, so PDF and PNG, which are made from this SVG, show it too.
    /// Task 5 の挙動の回帰テスト (最初から通る): プラグの黒い差し込みは、コネクタの無い同じブロックより黒く塗った多角形が1つ多い形で描かれるので、この SVG から作る PDF・PNG にも出る。
    #[test]
    fn svg_draws_a_plug_insert_as_a_black_polygon() {
        let filled = |pins: &str| {
            let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
            let block = crate::block::test_block("LTE1", "", 100.0, 50.0, pins);
            sheet.entities.insert(block.id(), block);
            let svg = sheet_to_svg(&sheet, &crate::symbol::sheet_symbol_defs(&sheet), &[]);
            svg.split("<polygon")
                .skip(1)
                .filter(|el| el.split("/>").next().unwrap_or("").contains("fill=\"#000\""))
                .count()
        };
        let plain = filled(r#"{"left":[{"no":"P1"}]}"#);
        assert_eq!(filled(r#"{"left":[{"no":"P1","connector":"usb_a_m"}]}"#), plain + 1);
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
        let got = frame_only(&sheet_to_svg(&golden_sheet(), &builtin_symbols(), &[]));
        assert_eq!(got, golden, "図枠の出力が変わっている");
    }

    /// An empty A4 sheet with no revisions also prints exactly the frame it always did: no revision table, and the title-block cells left blank rather than dropped.
    /// 改訂の無い空のA4シートも従来どおりの図枠を出す(改訂欄は描かれず、表題欄のセルは空欄のまま残る)。
    #[test]
    fn an_empty_sheet_reproduces_the_previous_frame_exactly() {
        let golden = include_str!("../tests/golden/frame-jis-a3-empty-a4.svg");
        let sheet = Sheet::new("Sheet1", PaperSize::A4, Orientation::Landscape);
        let got = frame_only(&sheet_to_svg(&sheet, &builtin_symbols(), &[]));
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
        let svg = sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &frame, &[]);
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
            &Default::default(),
            &RelayAnnotations::default(),
            &[],
            &frame,
            &BindContext::for_project(&project, &project.sheets[0]),
            &[],
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
        let svg = sheet_to_svg_with_frame(&golden_sheet(), &builtin_symbols(), &frame, &[]);
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
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
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

    /// SVG中の`<line>`を ((x1,y1), (x2,y2)) で列挙する。
    fn line_segments(svg: &str) -> Vec<(Point, Point)> {
        svg.lines()
            .filter(|l| l.trim_start().starts_with("<line "))
            .filter_map(|l| {
                Some((
                    Point::new(attr(l, "x1")?, attr(l, "y1")?),
                    Point::new(attr(l, "x2")?, attr(l, "y2")?),
                ))
            })
            .collect()
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
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
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
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
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
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
        assert!(svg.contains("Rev C"), "{svg}");
        assert!(!svg.contains("Rev A"), "{svg}");

        let mut plain = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        plain.title_block.rev = "A".into();
        let svg2 = sheet_to_svg(&plain, &builtin_symbols(), &[]);
        assert!(svg2.contains("Rev A"), "{svg2}");
    }

    /// With seven revisions only the newest six rows are drawn; the oldest row is dropped from the drawing while the data keeps it.
    /// 改訂が7件あると新しい6行だけが描かれ、最も古い行は図面から省かれる(データとしては残る)。
    #[test]
    fn svg_revision_table_shows_only_newest_six_rows() {
        let sheet = sheet_with_revisions(&["A", "B", "C", "D", "E", "F", "G"]);
        assert_eq!(sheet.revisions.len(), 7);
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
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
            length_source: Default::default(),
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

    /// 注記1つだけのシートと、その注記のSVG行を返す。
    fn note_line(text: &TextEntity) -> String {
        let sheet = sheet_with(vec![Entity::Text(text.clone())]);
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
        svg.lines()
            .find(|l| l.contains(&format!(">{}<", text.text)))
            .unwrap_or_else(|| panic!("注記が描かれていない: {svg}"))
            .to_string()
    }

    fn plain_note(text: &str) -> TextEntity {
        TextEntity {
            id: Uuid::new_v4(),
            at: Point::new(40.0, 60.0),
            text: text.into(),
            height: 2.5,
            rotation: 0,
            anchor: TextAnchor::Start,
            color: None,
            bold: false,
            leader_to: None,
        }
    }

    /// A note with no styling is exported exactly as notes have always been exported: left-aligned, black, normal weight.
    /// 装飾を付けていない注記は、これまでと寸分違わない形で出力される (左揃え・黒・普通の太さ)。
    #[test]
    fn an_unstyled_note_is_exported_exactly_as_before() {
        let line = note_line(&plain_note("注意"));
        assert_eq!(
            line,
            "<text x=\"40\" y=\"60\" font-size=\"2.500\" fill=\"#000\" text-anchor=\"start\" font-family=\"sans-serif\">注意</text>",
            "従来の出力と1文字も変えないこと"
        );
    }

    /// Choosing the default colour token explicitly changes nothing in the exported drawing.
    /// 既定の色トークンを明示的に選んでも、出力される図面は何も変わらない。
    #[test]
    fn the_default_colour_token_changes_nothing_in_the_export() {
        let mut explicit = plain_note("注意");
        explicit.color = Some("default".into());
        assert_eq!(note_line(&explicit), note_line(&plain_note("注意")));
    }

    /// A note's anchor, colour token and bold flag all reach the exported drawing.
    /// 注記の基準位置・色トークン・太字は、そのまま出力図面に反映される。
    #[test]
    fn a_styled_note_is_exported_with_its_anchor_colour_and_weight() {
        let mut note = plain_note("非常停止");
        note.anchor = TextAnchor::Middle;
        note.color = Some("warning".into());
        note.bold = true;
        let line = note_line(&note);
        assert!(line.contains("text-anchor=\"middle\""), "{line}");
        assert!(line.contains("fill=\"#c00000\""), "赤系の印刷色: {line}");
        assert!(line.contains("font-weight=\"bold\""), "{line}");

        let mut right = plain_note("右寄せ");
        right.anchor = TextAnchor::End;
        right.color = Some("accent".into());
        let line = note_line(&right);
        assert!(line.contains("text-anchor=\"end\""), "{line}");
        assert!(line.contains("fill=\"#0000c0\""), "青系の印刷色: {line}");
        assert!(!line.contains("font-weight"), "太字でない注記に太さ指定は付かない: {line}");
    }

    /// A note with no leader target draws exactly as many lines as the frame and title block alone (no leader line is added), exactly like drawings made before the feature existed.
    /// 引き出し先の無い注記は、図枠・表題欄だけが描く線の本数と変わらない (引き出し線が足されない)。この機能が無かった頃の図面と同じ。
    #[test]
    fn a_note_without_a_leader_target_draws_no_leader_line() {
        let count_lines = |sheet: &Sheet| {
            sheet_to_svg(sheet, &builtin_symbols(), &[])
                .lines()
                .filter(|l| l.starts_with("<line"))
                .count()
        };
        let with_note = sheet_with(vec![Entity::Text(plain_note("注意"))]);
        let without_note = sheet_with(vec![]);
        assert_eq!(count_lines(&with_note), count_lines(&without_note));
    }

    /// A note placed directly above its target draws a straight line from the midpoint of its bottom edge down to the top edge of the target's bounding box, with a filled dot at the target end, the same dot the wire junction uses.
    /// 対象の真上に置いた注記は、下辺の中点から対象の外接矩形の上辺まで直線を引き、対象側の端には配線の分岐点と同じ丸印を描く。
    #[test]
    fn a_note_with_a_leader_target_draws_a_leader_line_to_it() {
        let mut shape =
            crate::shape::new_shape(ShapeKind::Rect, vec![Point::new(90.0, 120.0), Point::new(130.0, 140.0)]);
        shape.id = Uuid::new_v4();
        let target_id = shape.id;
        let mut note = plain_note("ここに接続"); // Start anchor, at (100,50), height 2.5 → box x:[100,112.5] y:[47.5,50]
        note.at = Point::new(100.0, 50.0);
        note.leader_to = Some(target_id);
        let sheet = sheet_with(vec![Entity::Shape(shape), Entity::Text(note)]);
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
        let line = svg
            .lines()
            .find(|l| {
                l.starts_with("<line")
                    && attr(l, "y1") == Some(50.0)
                    && attr(l, "y2") == Some(120.0)
            })
            .unwrap_or_else(|| panic!("注記の下辺から対象の上辺への線が無い: {svg}"));
        assert_eq!(attr(line, "x1"), Some(106.25), "始点は注記の下辺の中点: {line}");
        assert_eq!(attr(line, "x2"), Some(106.25), "対象の上辺で真下に着地: {line}");
        let dot = svg
            .lines()
            .find(|l| l.starts_with("<circle") && l.contains("r=\"0.600\""))
            .unwrap_or_else(|| panic!("対象端の丸印 (半径0.6mm、ジャンクションと同じ) が無い: {svg}"));
        assert_eq!(attr(dot, "cx"), Some(106.25));
        assert_eq!(attr(dot, "cy"), Some(120.0));
    }

    /// A symbol's function label (its third text field) is printed under the symbol outline, centred, alongside the designator and the type number above it.
    /// シンボルの機能ラベル (第3のテキスト欄) は、上に並ぶ参照記号・型番とは反対に、シンボル外形の下へ中央揃えで描かれる。
    #[test]
    fn the_function_label_is_printed_under_the_symbol() {
        let mut inst = SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "switch_spst".into(),
            at: Point::new(100.0, 80.0),
            rotation: 0,
            mirror: false,
            reference: "SW1".into(),
            value: String::new(),
            attrs: Default::default(),
        };
        inst.attrs
            .insert(crate::text_style::FUNCTION_LABEL_ATTR.into(), "非常停止ボタン".into());
        let sheet = sheet_with(vec![Entity::Symbol(inst.clone())]);
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
        let line = svg
            .lines()
            .find(|l| l.contains(">非常停止ボタン<"))
            .unwrap_or_else(|| panic!("機能ラベルが描かれていない: {svg}"));
        assert_eq!(attr(line, "x"), Some(100.0), "配置基準点を中心に: {line}");
        assert!(line.contains("text-anchor=\"middle\""), "{line}");
        let def = builtin_symbols().into_iter().find(|d| d.id == "switch_spst").unwrap();
        let (_, max) = symbol_bounds(&inst, &def);
        assert!(attr(line, "y").unwrap() > max.y, "外形より下: {line}");
        let reference_y = texts(&svg)
            .into_iter()
            .find(|(_, _, s)| s == "SW1")
            .map(|(_, y, _)| y)
            .expect("参照記号");
        assert!(attr(line, "y").unwrap() > reference_y, "参照記号より下: {line}");
    }

    /// A symbol with no function label is drawn exactly as before, with only its designator and type number.
    /// 機能ラベルの無いシンボルは従来どおり、参照記号と型番だけを添えて描かれる。
    #[test]
    fn a_symbol_without_a_function_label_is_drawn_as_before() {
        let inst = SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "switch_spst".into(),
            at: Point::new(100.0, 80.0),
            rotation: 0,
            mirror: false,
            reference: "SW1".into(),
            value: "ABC-1".into(),
            attrs: Default::default(),
        };
        let svg = sheet_to_svg(&sheet_with(vec![Entity::Symbol(inst)]), &builtin_symbols(), &[]);
        let labels: Vec<String> = texts(&svg)
            .into_iter()
            .filter(|(x, _, _)| (*x - 100.0).abs() < 1e-9)
            .map(|(_, _, s)| s)
            .collect();
        assert_eq!(labels, vec!["SW1".to_string(), "ABC-1".to_string()]);
    }

    /// The wire number of a horizontal wire is printed in a monospaced font 2.5 mm above the middle of the wire.
    /// 横向きの配線の線番は、配線の中点の2.5mm上に等幅フォントで描かれる。
    #[test]
    fn svg_draws_wire_number_above_a_horizontal_wire() {
        let sheet = sheet_with(vec![numbered_wire(&[(50.0, 100.0), (90.0, 100.0)], Some("12"))]);
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
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
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
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
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
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
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
        // 配線注記 (線径・線色) は別の文字なので、線番の文字サイズで絞って探す
        let size = format!("font-size=\"{}\"", n(WIRE_NO_FONT));
        let numbers: Vec<&str> = svg
            .lines()
            .filter(|l| l.contains("monospace") && l.contains(&size))
            .collect();
        assert!(numbers.is_empty(), "{numbers:?}");
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
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
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
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
        let label = texts(&svg)
            .into_iter()
            .find(|(_, _, s)| s == "W1")
            .unwrap_or_else(|| panic!("ハーネス名が無い: {svg}"));
        assert_eq!((label.0, label.1), (51.0, 49.0), "左上角の外側: {label:?}");
    }

    /// 動的な3極コネクタ (`src/canvas/mate.test.ts`のparityテストと同じ形)。
    fn connector_3p(reference: &str, mate: Option<&str>, x: f64, y: f64) -> Entity {
        let Entity::Symbol(mut s) = connector_entity(reference, mate, x, y) else {
            unreachable!()
        };
        s.symbol_id = "connector_3p".into();
        Entity::Symbol(s)
    }

    fn connector_entity(reference: &str, mate: Option<&str>, x: f64, y: f64) -> Entity {
        let mut attrs = std::collections::BTreeMap::new();
        if let Some(m) = mate {
            attrs.insert(crate::mate::MATES_WITH_ATTR.to_string(), m.to_string());
        }
        Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "connector_plug".into(),
            at: Point::new(x, y),
            rotation: 0,
            mirror: false,
            reference: reference.into(),
            value: String::new(),
            attrs,
        })
    }

    /// Two mated connectors on the same sheet within 50mm are joined by a short dashed line between their outlines (G14).
    /// 同一シート上で50mm以内にある嵌合対は、外形どうしを結ぶ短い破線で結ばれる (G14)。
    #[test]
    fn svg_draws_a_dashed_line_between_a_mated_pair_on_one_sheet() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        for e in [
            connector_entity("J1", Some("J12"), 100.0, 100.0),
            connector_entity("J12", Some("J1"), 130.0, 100.0),
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let svg = project_sheet_to_svg(&project, sid, &builtin_symbols()).unwrap();
        let line = svg
            .lines()
            .find(|l| l.starts_with("<line") && l.contains("stroke-dasharray"))
            .unwrap_or_else(|| panic!("嵌合対の破線が無い: {svg}"));
        assert!(line.contains("stroke-dasharray=\"1.500 1\""), "{line}");
    }

    /// The mate link of two 3-pole connectors 30mm apart runs between the facing edges of their outlines, at the height of their placement points (G14).
    /// 30mm離れた3極コネクタ2つの嵌合線は、双方の外形の向かい合う辺の上、配置基準点の高さを結ぶ (G14)。
    #[test]
    fn the_mate_link_runs_between_the_facing_outline_edges() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        for e in [
            connector_3p("J1", Some("J12"), 100.0, 100.0),
            connector_3p("J12", Some("J1"), 130.0, 100.0),
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let defs = crate::symbol::sheet_symbol_defs(&project.sheets[0]);
        let svg = project_sheet_to_svg(&project, sid, &defs).unwrap();
        let line = svg
            .lines()
            .find(|l| l.starts_with("<line") && l.contains("stroke-dasharray=\"1.500 1\""))
            .unwrap_or_else(|| panic!("嵌合対の破線が無い: {svg}"));
        // connector_3pの外形は基準点の周り x[-4,+7.5] (ピンの先まで)・y[-7.5,+7.5]。
        // どちらの端から描くかはentity idの順で決まる (図面ごとには一定) ので両向きを許す
        assert!(
            line.contains("x1=\"107.500\" y1=\"100\" x2=\"126\" y2=\"100\"")
                || line.contains("x1=\"126\" y1=\"100\" x2=\"107.500\" y2=\"100\""),
            "{line}"
        );
    }

    /// A mated pair exactly 50mm apart is still drawn: the distance limit includes 50mm itself.
    /// ちょうど50mm離れた嵌合対は描かれる: 距離の上限は50mmちょうどを含む。
    #[test]
    fn the_mate_link_distance_limit_includes_exactly_fifty_millimetres() {
        let pair_at = |distance: f64| {
            let mut project = Project::new("t");
            let sid = project.sheets[0].id;
            let sheet = project.sheet_mut(sid).unwrap();
            for e in [
                connector_3p("J1", Some("J12"), 100.0, 100.0),
                connector_3p("J12", Some("J1"), 100.0 + distance, 100.0),
            ] {
                sheet.entities.insert(e.id(), e);
            }
            let defs = crate::symbol::sheet_symbol_defs(&project.sheets[0]);
            let svg = project_sheet_to_svg(&project, sid, &defs).unwrap();
            svg.lines().any(|l| l.starts_with("<line") && l.contains("stroke-dasharray=\"1.500 1\""))
        };
        assert!(pair_at(50.0), "50mmちょうどは描く");
        assert!(!pair_at(50.5), "50mmを超えたら描かない");
    }

    /// A point inside a symbol's outline is pushed out onto the nearest edge of that outline, so the mate link never starts inside the symbol.
    /// 外形の内側にある点は、その外形のいちばん近い辺の上へ押し出される (嵌合線がシンボルの内側から始まらない)。
    #[test]
    fn a_point_inside_the_outline_is_pushed_out_to_the_nearest_edge() {
        let (min, max) = (Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        // (3,4)からは左辺が3mmでいちばん近い (右7mm・上4mm・下6mm)
        assert_eq!(
            rect_boundary_point_toward(min, max, Point::new(3.0, 4.0)),
            Point::new(0.0, 4.0)
        );
    }

    /// A mated pair more than 50mm apart on the same sheet draws no line at all (the reports carry the pair instead).
    /// 50mmを超えて離れた嵌合対は線を一切描かない (対は帳票が伝える)。
    #[test]
    fn svg_draws_no_line_for_a_mated_pair_more_than_50mm_apart() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        for e in [
            connector_entity("J1", Some("J12"), 0.0, 0.0),
            connector_entity("J12", Some("J1"), 300.0, 0.0),
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let svg = project_sheet_to_svg(&project, sid, &builtin_symbols()).unwrap();
        assert!(
            !svg.lines().any(|l| l.starts_with("<line") && l.contains("stroke-dasharray=\"1.500 1\"")),
            "{svg}"
        );
    }

    /// A harness with no name draws only its dashed boundary, without a label.
    /// 名前の無いハーネスは破線の囲みだけを描き、ラベルは出さない。
    #[test]
    fn svg_omits_the_label_of_an_unnamed_harness() {
        let sheet = sheet_with(vec![harness_entity("", 50.0, 50.0, 150.0, 100.0)]);
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
        assert!(svg.contains("stroke-dasharray"), "囲みは描く: {svg}");
        assert!(
            !texts(&svg).iter().any(|&(x, y, _)| x == 51.0 && y == 49.0),
            "囲みの左上角にラベルは描かない: {svg}"
        );
    }

    /// テスト用の図形 (G10)。見た目は既定 (既定色・細線・塗りなし・実線)。
    fn shape_entity(kind: crate::model::ShapeKind, points: &[(f64, f64)]) -> Entity {
        Entity::Shape(crate::shape::new_shape(
            kind,
            points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
        ))
    }

    /// A rectangle shape is drawn as a plain rectangle at the two corners it was given.
    /// 矩形の図形は、与えた2つの角の位置にそのまま矩形として描かれる。
    #[test]
    fn svg_draws_a_rectangle_shape() {
        let sheet = sheet_with(vec![shape_entity(
            crate::model::ShapeKind::Rect,
            &[(150.0, 100.0), (50.0, 50.0)],
        )]);
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
        let line = svg
            .lines()
            .find(|l| l.starts_with("<rect") && l.contains("x=\"50\"") && l.contains("y=\"50\""))
            .unwrap_or_else(|| panic!("図形の矩形が無い: {svg}"));
        assert!(line.contains("width=\"100\""), "{line}");
        assert!(line.contains("height=\"50\""), "{line}");
        assert!(line.contains("fill=\"none\""), "塗り無しは塗らない: {line}");
        assert!(line.contains("stroke=\"#000\""), "既定色は黒: {line}");
        assert!(line.contains("stroke-width=\"0.250\""), "{line}");
        assert!(!line.contains("stroke-dasharray"), "既定は実線: {line}");
    }

    /// A line shape is drawn as a straight line between its two points, and a polyline shape follows every point in order.
    /// 線の図形は2点を結ぶ直線として、折れ線の図形は与えた点を順にたどる線として描かれる。
    #[test]
    fn svg_draws_line_and_polyline_shapes() {
        let sheet = sheet_with(vec![shape_entity(
            crate::model::ShapeKind::Line,
            &[(50.0, 60.0), (150.0, 60.0)],
        )]);
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
        assert!(
            svg.lines().any(|l| l.starts_with("<line")
                && l.contains("x1=\"50\"")
                && l.contains("y1=\"60\"")
                && l.contains("x2=\"150\"")
                && l.contains("y2=\"60\"")),
            "図形の線が無い: {svg}"
        );

        let sheet = sheet_with(vec![shape_entity(
            crate::model::ShapeKind::Polyline,
            &[(50.0, 50.0), (100.0, 50.0), (100.0, 90.0)],
        )]);
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
        assert!(
            svg.contains("points=\"50,50 100,50 100,90\""),
            "折れ線が順にたどられていない: {svg}"
        );
    }

    /// A circle shape is drawn at its centre with the radius reaching the point on its edge.
    /// 円の図形は中心の位置に、円周上の点までの距離を半径にして描かれる。
    #[test]
    fn svg_draws_a_circle_shape() {
        let sheet = sheet_with(vec![shape_entity(
            crate::model::ShapeKind::Circle,
            &[(100.0, 50.0), (100.0, 60.0)],
        )]);
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
        let line = svg
            .lines()
            .find(|l| l.starts_with("<circle") && l.contains("r=\"10\""))
            .unwrap_or_else(|| panic!("図形の円が無い: {svg}"));
        assert!(line.contains("cx=\"100\"") && line.contains("cy=\"50\""), "{line}");
        assert!(line.contains("fill=\"none\""), "{line}");
    }

    /// A shape prints in the colour its token stands for, at the stroke width it was given, dashed when it is set to dashed and shaded when it has a fill.
    /// 図形はトークンが表す色・指定した線幅で刷られ、破線に設定すれば破線に、塗りを指定すれば薄く塗られる。
    #[test]
    fn svg_honours_the_colour_width_dash_and_fill_of_a_shape() {
        let mut entity = shape_entity(crate::model::ShapeKind::Rect, &[(50.0, 50.0), (150.0, 100.0)]);
        if let Entity::Shape(s) = &mut entity {
            s.stroke = "accent".into();
            s.stroke_width_mm = 0.5;
            s.dashed = true;
            s.fill = Some("warning".into());
        }
        let sheet = sheet_with(vec![entity]);
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
        let line = svg
            .lines()
            .find(|l| l.starts_with("<rect") && l.contains("x=\"50\""))
            .unwrap_or_else(|| panic!("図形の矩形が無い: {svg}"));
        assert!(line.contains("stroke=\"#0000c0\""), "強調色の印刷色: {line}");
        assert!(line.contains("stroke-width=\"0.500\""), "{line}");
        assert!(line.contains("stroke-dasharray=\"3 2\""), "{line}");
        assert!(line.contains("fill=\"#c00000\""), "塗りの印刷色: {line}");
        assert!(
            line.contains(&format!("fill-opacity=\"{}\"", n(crate::shape::SHAPE_FILL_OPACITY))),
            "塗りは下の回路が透けて読める濃さ: {line}"
        );
    }

    /// Shapes are drawn behind the wires, so a filled highlight box never hides the circuit on top of it.
    /// 図形は配線より背面に描かれるので、塗りつぶした強調枠がその上の回路を隠すことはない。
    #[test]
    fn svg_draws_shapes_behind_the_wires() {
        let wire = Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(60.0, 60.0), Point::new(140.0, 60.0)],
            color: "red".into(),
            sq: 0.75,
            length_m: None,
            part_no: None,
            net: None,
            length_source: crate::model::LengthSource::Manual,
        });
        let sheet = sheet_with(vec![
            wire,
            shape_entity(crate::model::ShapeKind::Rect, &[(50.0, 50.0), (150.0, 100.0)]),
        ]);
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
        let shape_at = svg.find("<rect x=\"50\"").expect("図形の矩形");
        let wire_at = svg.find("<polyline points=\"60,60").expect("配線");
        assert!(shape_at < wire_at, "図形が配線より後に描かれている: {svg}");
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
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
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
                length_source: Default::default(),
                part_no: None,
                net: None,
            });
            let l = Entity::NetLabel(NetLabel {
                id: Uuid::new_v4(),
                at: Point::new(x, y),
                name: "24V_1".into(),
                rotation: 0,
                attrs: Default::default(),
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
        let svg = sheet_to_svg(&project.sheets[0], &builtin_symbols(), &[]);
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
        let svg = sheet_to_svg(&project.sheets[0], &builtin_symbols(), &[]);
        assert!(!svg.contains(">13-14<"), "{svg}");
    }

    /// A symbol whose "hide_reference" attribute is "true" keeps its designator in the data but does not print it, so one half of a mated plug/socket pair does not draw the same designator twice.
    /// `hide_reference`属性が`"true"`のシンボルは、参照記号をデータには持ったまま印字しないので、嵌合するプラグ/ソケットの片方が同じ記号を二重に描かない。
    #[test]
    fn svg_does_not_print_a_hidden_designator() {
        let shown = connector_entity("J5", None, 100.0, 50.0);
        let Entity::Symbol(mut hidden) = connector_entity("J6", None, 150.0, 50.0) else {
            unreachable!()
        };
        hidden.attrs.insert(crate::model::HIDE_REFERENCE_ATTR.into(), "true".into());
        let sheet = sheet_with(vec![shown, Entity::Symbol(hidden)]);
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
        assert!(svg.contains(">J5<"), "{svg}");
        assert!(!svg.contains(">J6<"), "{svg}");
    }

    /// A coil whose "device_block" attribute is "true" is drawn as one outline with its designator, part number and contact image, instead of the small coil symbol and the separate hanging contact map.
    /// `device_block`属性が`"true"`のコイルは、小さなコイル図形と別表の接点マップの代わりに、参照記号・型番・接点イメージをまとめた1つの外形として描かれる (G7)。
    #[test]
    fn svg_draws_a_device_block_for_a_flagged_coil() {
        let mut project = relay_project();
        let Entity::Symbol(coil) = project.sheets[0].entities.values_mut().next().unwrap() else {
            unreachable!()
        };
        coil.attrs.insert("device_block".into(), "true".into());
        coil.value = "SAMPLE0001".into();
        let svg = project_sheet_to_svg(&project, project.sheets[0].id, &builtin_symbols()).unwrap();
        assert!(svg.contains(">K1<"), "参照記号: {svg}");
        assert!(svg.contains(">SAMPLE0001<"), "型番: {svg}");
        assert!(svg.contains(">13-14<"), "接点イメージの端子対: {svg}");
        assert!(svg.contains(">/2.B3<"), "接点イメージの所在: {svg}");
    }

    /// 実機ブロック表示のコイルを回転・反転させたシートのSVGと、そのコイルのインスタンス・
    /// ブロックの中身。型番を入れてブロックをピン間隔より広げ、引き出し線が必ず描かれる形にする。
    fn turned_device_block_svg(
        rotation: u16,
        mirror: bool,
    ) -> (String, SymbolInstance, crate::relay_xref::DeviceBlock) {
        let mut project = relay_project();
        let Entity::Symbol(coil) = project.sheets[0].entities.values_mut().next().unwrap() else {
            unreachable!()
        };
        coil.attrs.insert("device_block".into(), "true".into());
        coil.value = "SAMPLE0001".into();
        coil.rotation = rotation;
        coil.mirror = mirror;
        let coil = coil.clone();
        let block = crate::relay_xref::sheet_device_blocks(&project, project.sheets[0].id)
            .remove(&coil.id)
            .expect("実機ブロック");
        let svg = project_sheet_to_svg(&project, project.sheets[0].id, &builtin_symbols()).unwrap();
        (svg, coil, block)
    }

    /// [`turned_device_block_svg`] のブロックが描く文字を (x, ベースラインy, 寄せ, 文字列) で拾う。
    fn block_texts(svg: &str) -> Vec<(f64, f64, String, String)> {
        let wanted = ["K1", "SAMPLE0001", "13-14", "/2.B3"];
        svg.lines()
            .filter(|l| l.trim_start().starts_with("<text "))
            .filter_map(|l| {
                let body = l.split_once('>')?.1.rsplit_once("</text>")?.0.to_string();
                if !wanted.contains(&body.as_str()) {
                    return None;
                }
                let anchor = l
                    .split_once("text-anchor=\"")
                    .map(|(_, rest)| rest.split('"').next().unwrap_or("start").to_string())
                    .unwrap_or_else(|| "start".into());
                Some((attr(l, "x")?, attr(l, "y")?, anchor, body))
            })
            .collect()
    }

    /// However a device-block coil is turned or mirrored, each lead line still starts exactly on one of the coil's A1/A2 pin points, so the wires attached there stay connected.
    /// 実機ブロックのコイルをどの向きに回しても反転しても、引き出し線は必ず実際のA1/A2ピンの点から出るので、そこに繋いだ配線は繋がったまま (G7)。
    #[test]
    fn a_turned_device_block_keeps_its_leads_on_the_coil_pins() {
        for (rotation, mirror) in [(0, false), (90, false), (180, false), (270, false), (0, true)] {
            let (svg, coil, _) = turned_device_block_svg(rotation, mirror);
            let def = crate::symbol::resolve_symbol(&coil.symbol_id).unwrap();
            let segments = line_segments(&svg);
            for (number, at) in crate::netlist::pin_positions(&coil, &def) {
                let on_pin = |p: &Point| (p.x - at.x).abs() < 1e-6 && (p.y - at.y).abs() < 1e-6;
                assert!(
                    segments.iter().any(|(a, b)| on_pin(a) || on_pin(b)),
                    "rotation={rotation} mirror={mirror}: ピン{number} {at:?} から出る引き出し線が無い"
                );
            }
        }
    }

    /// However a device-block coil is turned or mirrored, its table is drawn upright: every line of text stays inside the outline and no two lines are drawn on top of each other.
    /// 実機ブロックのコイルをどの向きに回しても反転しても、表は正立したまま描かれる: どの行の文字も外形の中に収まり、行どうしが重なることはない (G7)。
    #[test]
    fn a_turned_device_block_keeps_its_lettering_upright_and_inside_the_outline() {
        for (rotation, mirror) in [(0, false), (90, false), (180, false), (270, false), (0, true)] {
            let (svg, coil, block) = turned_device_block_svg(rotation, mirror);
            let (min, max) = crate::relay_xref::device_block_bounds(&coil, &block);
            let boxes: Vec<(String, (f64, f64, f64, f64))> = block_texts(&svg)
                .into_iter()
                .map(|(x, y, anchor, text)| {
                    let w = 2.0 * 0.6 * text.chars().count() as f64;
                    let x0 = if anchor == "middle" { x - w / 2.0 } else { x };
                    (text, (x0, y - 2.0, x0 + w, y))
                })
                .collect();
            for (text, b) in &boxes {
                assert!(
                    b.0 >= min.x - 1e-6 && b.2 <= max.x + 1e-6 && b.1 >= min.y - 1e-6 && b.3 <= max.y + 1e-6,
                    "rotation={rotation} mirror={mirror}: 「{text}」が外形({min:?}..{max:?})から出ている: {b:?}"
                );
            }
            for (i, (ta, a)) in boxes.iter().enumerate() {
                for (tb, b) in boxes.iter().skip(i + 1) {
                    assert!(
                        a.0 >= b.2 - 1e-6 || b.0 >= a.2 - 1e-6 || a.1 >= b.3 - 1e-6 || b.1 >= a.3 - 1e-6,
                        "rotation={rotation} mirror={mirror}: 「{ta}」と「{tb}」が重なっている: {a:?} {b:?}"
                    );
                }
            }
        }
    }

    /// A device-block coil still prints its function label under the outline, the same as a coil in the ordinary distributed placement.
    /// 実機ブロックで表示するコイルでも、機能ラベルは従来の分散配置と同じく外形の下に描かれる (G4)。
    #[test]
    fn a_device_block_keeps_the_function_label_under_its_outline() {
        let mut project = relay_project();
        let Entity::Symbol(coil) = project.sheets[0].entities.values_mut().next().unwrap() else {
            unreachable!()
        };
        coil.attrs.insert("device_block".into(), "true".into());
        coil.attrs
            .insert(crate::text_style::FUNCTION_LABEL_ATTR.into(), "運転".into());
        let coil = coil.clone();
        let block = crate::relay_xref::sheet_device_blocks(&project, project.sheets[0].id)
            .remove(&coil.id)
            .expect("実機ブロック");
        let (_, max) = crate::relay_xref::device_block_bounds(&coil, &block);
        let svg = project_sheet_to_svg(&project, project.sheets[0].id, &builtin_symbols()).unwrap();
        let (_, y, _) = texts(&svg)
            .into_iter()
            .find(|(_, _, t)| t == "運転")
            .expect("機能ラベルが描かれていない");
        assert!(y > max.y, "ブロックの外形(下端{})より下に置く: {y}", max.y);
    }

    /// Toggling "device_block" off keeps the previous distributed placement: the small coil symbol with its own separate contact map underneath.
    /// `device_block`を付けなければ従来どおりの分散配置のまま: 小さなコイル図形の下に別表の接点マップが付く。
    #[test]
    fn svg_keeps_the_distributed_placement_without_the_attribute() {
        let project = relay_project();
        let svg = project_sheet_to_svg(&project, project.sheets[0].id, &builtin_symbols()).unwrap();
        assert!(svg.contains(">13-14<"), "接点マップは従来どおり別表で出る: {svg}");
        assert!(svg.contains(">K1<"));
    }

    /// A non-relay designator drawn on two sheets gets an automatic "(/2.B3)"-style cross-reference beside it, on both sheets (G13).
    /// リレー以外の参照記号が2枚のシートに描かれていると、両方のシートに「/2.B3」形式のクロスリファレンスが自動で添えられる (G13)。
    #[test]
    fn svg_draws_a_device_cross_reference_for_a_designator_on_two_sheets() {
        let mut project = Project::new("t");
        project
            .sheets
            .push(Sheet::new("Sheet2", PaperSize::A3, Orientation::Landscape));
        let put = |sheet: &mut Sheet, x: f64, y: f64| {
            let e = Entity::Symbol(SymbolInstance {
                id: Uuid::new_v4(),
                symbol_id: "motor".into(),
                at: Point::new(x, y),
                rotation: 0,
                mirror: false,
                reference: "M1".into(),
                value: String::new(),
                attrs: Default::default(),
            });
            sheet.entities.insert(e.id(), e);
        };
        let (s1, s2) = project.sheets.split_at_mut(1);
        put(&mut s1[0], 150.0, 120.0);
        put(&mut s2[0], 250.0, 70.0);
        let svg1 = project_sheet_to_svg(&project, project.sheets[0].id, &builtin_symbols()).unwrap();
        assert!(svg1.contains(">/2.B3<"), "シート1に相手先の住所: {svg1}");
        let svg2 = project_sheet_to_svg(&project, project.sheets[1].id, &builtin_symbols()).unwrap();
        assert!(svg2.contains(">/1.C2<"), "シート2に相手先の住所: {svg2}");
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
                item_no: None,
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
        let svg = sheet_to_svg(&panel_sheet(), &builtin_symbols(), &[]);
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
        let svg = sheet_to_svg(&panel_sheet(), &builtin_symbols(), &[]);
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
        let svg = sheet_to_svg(&panel_sheet(), &builtin_symbols(), &[]);
        assert!(
            svg.contains("<rect x=\"60\" y=\"132.500\" width=\"160\" height=\"35\""),
            "DINレール: {svg}"
        );
    }

    /// A footprint is drawn as its outline rectangle with the reference designator and the type number inside.
    /// フットプリントは外形の矩形として描かれ、その中に参照記号と型番が入る。
    #[test]
    fn svg_of_a_panel_sheet_draws_the_footprint_with_its_reference() {
        let svg = sheet_to_svg(&panel_sheet(), &builtin_symbols(), &[]);
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
            item_no: None,
        });
        sheet.entities.insert(fp.id(), fp);
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
        assert!(
            svg.contains("<rect x=\"60\" y=\"100\" width=\"65\" height=\"25\""),
            "{svg}"
        );
    }

    /// 盤シートに、指定したアイテム番号のフットプリントと凡例表を置く。
    fn panel_sheet_with(item_no: Option<u32>, legend_at: Option<Point>) -> Sheet {
        let mut sheet =
            Sheet::new_kind("盤", PaperSize::A3, Orientation::Landscape, SheetKind::Panel);
        let fp = Entity::Footprint(Footprint {
            id: Uuid::new_v4(),
            at: Point::new(100.0, 100.0),
            w: 20.0,
            h: 40.0,
            rotation: 0,
            reference: "K1".into(),
            part_no: "MDK-RLY-24V".into(),
            item_no,
        });
        sheet.entities.insert(fp.id(), fp);
        if let Some(at) = legend_at {
            let l = Entity::PanelLegend(PanelLegend {
                id: Uuid::new_v4(),
                at,
            });
            sheet.entities.insert(l.id(), l);
        }
        sheet
    }

    /// A numbered footprint carries a balloon: a white circle holding the item number, joined to the part by a short leader line.
    /// 採番したフットプリントにはバルーンが付く(アイテム番号を入れた白丸と、部品へ伸びる短い引出線)。
    #[test]
    fn svg_draws_a_balloon_on_a_numbered_footprint() {
        let svg = sheet_to_svg(&panel_sheet_with(Some(7), None), &builtin_symbols(), &[]);
        assert!(svg.contains(">7<"), "バルーンの番号: {svg}");
        assert!(
            svg.contains(&format!("r=\"{}\"", n(crate::panel::BALLOON_R))),
            "白丸: {svg}"
        );
        assert!(svg.contains("fill=\"#fff\""), "丸の中は白で塗る (図面が透けない): {svg}");
    }

    /// A footprint that has not been numbered yet has no balloon on the drawing.
    /// まだ採番していないフットプリントには図面にバルーンが出ない。
    #[test]
    fn svg_draws_no_balloon_before_numbering() {
        let svg = sheet_to_svg(&panel_sheet_with(None, None), &builtin_symbols(), &[]);
        assert!(!svg.contains(&format!("r=\"{}\"", n(crate::panel::BALLOON_R))), "{svg}");
    }

    /// The legend table is drawn where it was placed, listing the item number, the reference and the type number of every part on the sheet.
    /// 凡例表は置いた場所に描かれ、そのシートの部品のアイテム番号・参照記号・型番が並ぶ。
    #[test]
    fn svg_draws_the_legend_table() {
        let svg = sheet_to_svg(
            &panel_sheet_with(Some(1), Some(Point::new(300.0, 200.0))),
            &builtin_symbols(),
            &[],
        );
        assert!(svg.contains(">参照記号<"), "列見出し: {svg}");
        assert!(svg.contains(">K1<"), "参照記号: {svg}");
        assert!(svg.contains(">MDK-RLY-24V<"), "型番: {svg}");
        assert!(
            svg.contains("<rect x=\"300\" y=\"200\""),
            "置いた場所に表がある: {svg}"
        );
    }

    /// シート1枚に配線1本と電線品番凡例ブロックを置く。
    fn sheet_with_wire_legend(legend_at: Point) -> Sheet {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let w = Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(50.0, 60.0), Point::new(90.0, 60.0)],
            color: "水色".into(),
            sq: 0.3,
            length_m: Some(1.5),
            part_no: Some("SAMPLE0001".into()),
            net: None,
            length_source: crate::model::LengthSource::Manual,
        });
        sheet.entities.insert(w.id(), w);
        let legend = Entity::WireLegend(WireLegend { id: Uuid::new_v4(), at: legend_at });
        sheet.entities.insert(legend.id(), legend);
        sheet
    }

    /// The wire part-number legend block is drawn where it was placed, listing the colour, gauge, part number and summed length of the sheet's wires.
    /// 電線品番凡例ブロックは置いた場所に描かれ、そのシートの配線の線色・線径・品番・使用長合計が並ぶ。
    #[test]
    fn svg_draws_the_wire_legend_block() {
        let svg = sheet_to_svg(&sheet_with_wire_legend(Point::new(300.0, 200.0)), &builtin_symbols(), &[]);
        assert!(svg.contains(">電線品番<"), "列見出し: {svg}");
        assert!(svg.contains(">水色<"), "線色: {svg}");
        assert!(svg.contains(">SAMPLE0001<"), "ワイヤ自身に書かれた品番: {svg}");
        assert!(
            svg.contains("<rect x=\"300\" y=\"200\""),
            "置いた場所に表がある: {svg}"
        );
    }

    /// A sheet with no wires at all still draws the wire legend block's header row.
    /// 配線の無いシートでも、電線品番凡例ブロックの列見出しは描かれる。
    #[test]
    fn svg_draws_the_wire_legend_header_even_with_no_wires() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let legend = Entity::WireLegend(WireLegend { id: Uuid::new_v4(), at: Point::new(10.0, 10.0) });
        sheet.entities.insert(legend.id(), legend);
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
        assert!(svg.contains(">使用長 m<"), "{svg}");
    }

    /// The wire legend block resolves a blank part number from the project's wire-part table, the same way the wire list does.
    /// 電線品番凡例ブロックは、電線リストと同じ規則でプロジェクトの電線品番表から品番を引く。
    #[test]
    fn svg_wire_legend_resolves_part_number_from_the_project_table() {
        let mut project = Project::new("t");
        project.wire_parts = vec![WirePart {
            part_no: "SAMPLE0002".into(),
            color: "水色".into(),
            sq: 0.3,
            note: String::new(),
        }];
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let w = Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)],
            color: "水色".into(),
            sq: 0.3,
            length_m: None,
            part_no: None,
            net: None,
            length_source: crate::model::LengthSource::Manual,
        });
        sheet.entities.insert(w.id(), w);
        let legend = Entity::WireLegend(WireLegend { id: Uuid::new_v4(), at: Point::new(10.0, 10.0) });
        sheet.entities.insert(legend.id(), legend);
        let sheet_id = sheet.id;
        project.sheets[0] = sheet;
        let svg = project_sheet_to_svg(&project, sheet_id, &builtin_symbols()).unwrap();
        assert!(svg.contains(">SAMPLE0002<"), "{svg}");
    }

    /// Exporting one sheet on its own also fills the legend's part numbers, as long as the caller hands over the project's wire-part table.
    /// シート単体の書き出しでも、呼び出し元が電線品番表を渡せば凡例の品番欄は埋まる。
    #[test]
    fn svg_wire_legend_resolves_part_number_on_the_sheet_only_export() {
        let parts = vec![WirePart {
            part_no: "SAMPLE0002".into(),
            color: "水色".into(),
            sq: 0.3,
            note: String::new(),
        }];
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let w = Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)],
            color: "水色".into(),
            sq: 0.3,
            length_m: None,
            part_no: None,
            net: None,
            length_source: crate::model::LengthSource::Manual,
        });
        sheet.entities.insert(w.id(), w);
        let legend = Entity::WireLegend(WireLegend { id: Uuid::new_v4(), at: Point::new(10.0, 10.0) });
        sheet.entities.insert(legend.id(), legend);
        let with_parts = sheet_to_svg(&sheet, &builtin_symbols(), &parts);
        assert!(with_parts.contains(">SAMPLE0002<"), "{with_parts}");
        let without = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
        assert!(
            !without.contains(">SAMPLE0002<"),
            "表を渡さなければ引きようがない (品番欄は空欄のまま)"
        );
    }

    /// シート1枚に、配線注記の付いたワイヤ1本を置く (横向き)。
    fn sheet_with_annotated_wire() -> Sheet {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let w = Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(50.0, 60.0), Point::new(90.0, 60.0)],
            color: "white".into(),
            sq: 0.75,
            length_m: Some(0.4),
            part_no: None,
            net: None,
            length_source: crate::model::LengthSource::Manual,
        });
        sheet.entities.insert(w.id(), w);
        sheet
    }

    /// Every wire prints its gauge, colour and cut length under the wire, which is what the shop needs to make the harness.
    /// 全ての配線は線径・線色・電線長を配線の下に印字する(現場が電線を作るのに必要な情報)。
    #[test]
    fn svg_annotates_each_wire_with_gauge_colour_and_length() {
        let svg = sheet_to_svg(&sheet_with_annotated_wire(), &builtin_symbols(), &[]);
        let (x, y, _) = texts(&svg)
            .into_iter()
            .find(|(_, _, t)| t == "0.75sq 白 0.4m")
            .expect("配線注記");
        assert!((x - 70.0).abs() < 1e-9, "代表線分の中点の下: {x}");
        assert!((y - (60.0 + crate::wire_note::WIRE_NOTE_GAP)).abs() < 1e-9, "配線の下: {y}");
    }

    /// The wire note is part of the exported drawing itself, so it comes out in SVG and PDF without turning anything on.
    /// 配線注記は図面そのものの一部なので、何も設定しなくてもSVG・PDFに出る。
    #[test]
    fn wire_notes_are_exported_by_default() {
        let svg = sheet_to_svg(&sheet_with_annotated_wire(), &builtin_symbols(), &[]);
        assert!(svg.contains(">0.75sq 白 0.4m<"), "既定で出力に含まれる: {svg}");
    }

    /// Hiding a class on screen never changes the exported file: the export always draws the whole drawing.
    /// 画面で表示クラスを消しても出力ファイルは変わらない(出力は常に図面の全部を描く)。
    #[test]
    fn export_ignores_screen_only_view_classes() {
        let a = sheet_to_svg(&sheet_with_annotated_wire(), &builtin_symbols(), &[]);
        let b = sheet_to_svg(&sheet_with_annotated_wire(), &builtin_symbols(), &[]);
        assert_eq!(a, b, "出力に画面側の表示設定は入り込まない");
        assert!(a.contains(">0.75sq 白 0.4m<"));
    }

    /// A wire with no gauge, colour or length is drawn without any note next to it.
    /// 線径も色も長さも無い配線には、注記が何も付かない。
    #[test]
    fn svg_omits_the_note_of_a_bare_wire() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let w = Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(50.0, 60.0), Point::new(90.0, 60.0)],
            color: String::new(),
            sq: 0.0,
            length_m: None,
            part_no: None,
            net: None,
            length_source: crate::model::LengthSource::Manual,
        });
        sheet.entities.insert(w.id(), w);
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
        assert!(!svg.contains("sq "), "注記なし: {svg}");
    }

    /// ネットラベル1枚のシート (tagを指定できる)。
    fn sheet_with_net_label(tag: Option<&str>) -> Sheet {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let mut attrs = std::collections::BTreeMap::new();
        if let Some(t) = tag {
            attrs.insert("tag".to_string(), t.to_string());
        }
        let l = Entity::NetLabel(NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(100.0, 50.0),
            name: "24V_1".into(),
            rotation: 0,
            attrs,
        });
        sheet.entities.insert(l.id(), l);
        sheet
    }

    /// A net label tagged "right" is drawn as a flag outline pointing right, with the net name inside the body that runs to the left of the label position.
    /// タグ「右」のネットラベルは右を向いた旗の枠として描かれ、ネット名は置いた点の左へ伸びる本体の中に入る。
    #[test]
    fn svg_draws_a_directional_flag_around_a_tagged_net_label() {
        let svg = sheet_to_svg(&sheet_with_net_label(Some("right")), &builtin_symbols(), &[]);
        let polygon = svg
            .lines()
            .find(|l| l.trim_start().starts_with("<polygon") && l.contains("100,"))
            .expect("タグの多角形");
        assert_eq!(polygon.matches(',').count(), 5, "五角形: {polygon}");
        let (x, _, _) = texts(&svg)
            .into_iter()
            .find(|(_, _, t)| t == "24V_1")
            .expect("ネット名");
        assert!(x < 100.0, "文字は基準点の左の本体の中: {x}");
        assert!(svg.contains("fill=\"none\""), "枠は塗らない: {svg}");
    }

    /// A sheet with a smaller annotation text height exports smaller wire numbers, wire notes and net tags, and nothing else changes size.
    /// 注記文字高を小さくしたシートは、線番・配線注記・ネットタグだけが小さくなって出力される。
    #[test]
    fn svg_follows_the_sheet_annotation_text_height() {
        let mut sheet = sheet_with_net_label(Some("right"));
        let w = Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(20.0, 100.0), Point::new(80.0, 100.0)],
            color: "red".into(),
            sq: 0.75,
            length_m: Some(0.4),
            part_no: None,
            net: Some("12".into()),
            length_source: crate::model::LengthSource::Manual,
        });
        sheet.entities.insert(w.id(), w);
        let default_svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
        sheet.annotation_text_mm = Some(1.4);
        let small_svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);

        let size_of = |svg: &str, text: &str| {
            svg.lines()
                .find(|l| l.contains(&format!(">{text}<")))
                .and_then(|l| {
                    let i = l.find("font-size=\"")? + 11;
                    l[i..].split('"').next()?.parse::<f64>().ok()
                })
                .unwrap_or_else(|| panic!("{text} が見つからない: {svg}"))
        };
        assert!((size_of(&default_svg, "12") - 2.5).abs() < 1e-9, "既定の線番は2.5mm");
        assert!((size_of(&small_svg, "12") - 1.75).abs() < 1e-9, "線番も同じ比率で縮む");
        assert!((size_of(&default_svg, "0.75sq 赤 0.4m") - 2.0).abs() < 1e-9);
        assert!((size_of(&small_svg, "0.75sq 赤 0.4m") - 1.4).abs() < 1e-9, "配線注記=設定値");
        assert!((size_of(&small_svg, "24V_1") - 1.75).abs() < 1e-9, "タグ文字も縮む");
        // 表題欄は図面規格の一部なので注記文字高では変わらない
        assert_eq!(
            default_svg.matches(&format!("font-size=\"{}\"", n(TITLE_FONT))).count(),
            small_svg.matches(&format!("font-size=\"{}\"", n(TITLE_FONT))).count(),
        );
    }

    /// A net label with no tag keeps the plain text look, with no outline drawn around it.
    /// タグの無いネットラベルは従来どおり文字だけで、周りに枠は描かれない。
    #[test]
    fn svg_draws_no_outline_for_an_untagged_net_label() {
        let svg = sheet_to_svg(&sheet_with_net_label(None), &builtin_symbols(), &[]);
        assert!(svg.contains(">24V_1<"), "文字は出る");
        assert!(!svg.contains("<polygon"), "枠は描かない: {svg}");
    }

    /// An unknown tag value is ignored and the label falls back to plain text rather than breaking the export.
    /// 知らないタグの値は無視され、出力が壊れるのではなく文字だけの表示に戻る。
    #[test]
    fn svg_ignores_an_unknown_tag_value() {
        let svg = sheet_to_svg(&sheet_with_net_label(Some("diagonal")), &builtin_symbols(), &[]);
        assert!(svg.contains(">24V_1<"));
        assert!(!svg.contains("<polygon"), "{svg}");
    }

    // ---- ジャンパバー (G5) ----

    /// SVG中の`<line>`を (x1, y1, x2, y2, 線幅) で列挙する。
    fn svg_lines(svg: &str) -> Vec<(f64, f64, f64, f64, f64)> {
        svg.lines()
            .filter(|l| l.trim_start().starts_with("<line "))
            .filter_map(|l| {
                Some((attr(l, "x1")?, attr(l, "y1")?, attr(l, "x2")?, attr(l, "y2")?, attr(l, "stroke-width")?))
            })
            .collect()
    }

    /// SVG中の`<text>`を (x, y, 文字高, 中身) で列挙する。
    fn svg_texts(svg: &str) -> Vec<(f64, f64, f64, String)> {
        svg.lines()
            .filter(|l| l.trim_start().starts_with("<text "))
            .filter_map(|l| {
                let body = l.split_once('>')?.1.rsplit_once("</text>")?.0;
                Some((attr(l, "x")?, attr(l, "y")?, attr(l, "font-size")?, body.to_string()))
            })
            .collect()
    }

    /// SVG中の`<circle>`を (cx, cy, r) で列挙する。
    fn svg_circles(svg: &str) -> Vec<(f64, f64, f64)> {
        svg.lines()
            .filter(|l| l.trim_start().starts_with("<circle "))
            .filter_map(|l| Some((attr(l, "cx")?, attr(l, "cy")?, attr(l, "r")?)))
            .collect()
    }

    fn terminal_block(reference: &str, symbol_id: &str, x: f64, y: f64, jumpers: &str) -> Entity {
        let mut attrs = std::collections::BTreeMap::new();
        if !jumpers.is_empty() {
            attrs.insert("jumpers".to_string(), jumpers.to_string());
        }
        Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: symbol_id.into(),
            at: Point::new(x, y),
            rotation: 0,
            mirror: false,
            reference: reference.into(),
            value: String::new(),
            attrs,
        })
    }

    /// 矩形 (x最小, y最小, x最大, y最大) が重なっているか (縁が触れるだけは重なりとしない)。
    fn overlaps(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
        a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
    }

    /// 端子番号の文字が占める矩形。文字は中心揃え・ベースライン基準で描かれるので、
    /// 幅は「文字数×文字高×0.6」(サンセリフの送り幅を多めに見た概算)、
    /// 高さはベースラインの上に「文字高×0.7」(数字・小文字を含む大文字高さ)。
    fn number_boxes(svg: &str) -> Vec<(f64, f64, f64, f64)> {
        svg.lines()
            .filter(|l| l.trim_start().starts_with("<text "))
            .filter_map(|l| {
                let size = attr(l, "font-size")?;
                if (size - crate::symbol::TERMINAL_NUMBER_HEIGHT).abs() > 1e-9 {
                    return None;
                }
                let (x, y) = (attr(l, "x")?, attr(l, "y")?);
                let body = l.split_once('>')?.1.rsplit_once("</text>")?.0;
                let half = body.chars().count() as f64 * size * 0.6 / 2.0;
                Some((x - half, y - size * 0.7, x + half, y))
            })
            .collect()
    }

    /// ジャンパバー (太い線) が実際に塗る矩形。線の端は伸びない (SVG既定の butt cap) ので、
    /// 太さのぶんは線に**直交する向き**にだけ広げる。
    fn jumper_boxes(svg: &str) -> Vec<(f64, f64, f64, f64)> {
        svg_lines(svg)
            .into_iter()
            .filter(|l| (l.4 - JUMPER_BAR_STROKE).abs() < 1e-9)
            .map(|l| {
                let h = JUMPER_BAR_STROKE / 2.0;
                // 端子は2.5mmグリッド上・回転は0/90/180/270だけなので、バーは必ず縦か横になる
                let (hx, hy) = match ((l.0 - l.2).abs() < 1e-9, (l.1 - l.3).abs() < 1e-9) {
                    (true, false) => (h, 0.0), // 縦のバー
                    (false, true) => (0.0, h), // 横のバー
                    _ => panic!("ジャンパバーが縦にも横にもなっていない: {l:?}"),
                };
                (
                    l.0.min(l.2) - hx,
                    l.1.min(l.3) - hy,
                    l.0.max(l.2) + hx,
                    l.1.max(l.3) + hy,
                )
            })
            .collect()
    }

    /// 端子台1台だけのシートをSVGにする (向き・反転つき)。
    fn jumper_sheet(symbol_id: &str, jumpers: &str, rotation: u16, mirror: bool) -> String {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let mut tb = terminal_block("TB1", symbol_id, 200.0, 150.0, jumpers);
        if let Entity::Symbol(s) = &mut tb {
            s.rotation = rotation;
            s.mirror = mirror;
        }
        sheet.entities.insert(tb.id(), tb);
        sheet_to_svg(&sheet, &crate::symbol::sheet_symbol_defs(&sheet), &[])
    }

    /// A jumper bar is always drawn clear of every terminal number, whatever jumpers the block carries and however it is turned or mirrored, so the numbers stay readable.
    /// ジャンパバーは、ジャンパの掛け方や端子台の向き・反転にかかわらず、どの端子番号にも重ならない場所に描かれるので、番号はいつでも読める。
    #[test]
    fn a_jumper_bar_never_covers_a_terminal_number() {
        for (symbol_id, jumpers, rotation, mirror) in [
            ("terminal_block_4p", "1-2", 0u16, false),
            ("terminal_block_4p", "1-3", 0, false),
            ("terminal_block_4p", "1-2,3-4", 0, false),
            ("terminal_block_4p", "1-2,2-3", 0, false),
            ("terminal_block_4p_2l", "1a-1b,2a-3a", 0, false),
            ("terminal_block_4p_3l", "1a-1b,1b-1c", 0, false),
            ("terminal_block_4p", "1-2,2-3", 90, false),
            ("terminal_block_4p", "1-2,2-3", 180, false),
            ("terminal_block_4p", "1-2,2-3", 270, false),
            ("terminal_block_4p", "1-2,2-3", 0, true),
            // 一番幅の広い端子番号 (3文字) を持つ端子台
            ("terminal_block_12p_2l", "10a-10b,11a-12a", 0, false),
        ] {
            let svg = jumper_sheet(symbol_id, jumpers, rotation, mirror);
            let numbers = number_boxes(&svg);
            assert!(!numbers.is_empty(), "{symbol_id}: 端子番号が1つも見つからない");
            for graphic in jumper_boxes(&svg) {
                for number in &numbers {
                    assert!(
                        !overlaps(graphic, *number),
                        "{symbol_id} {jumpers} rot={rotation} mirror={mirror}: \
                         ジャンパの図形{graphic:?}が端子番号{number:?}に重なっている"
                    );
                }
            }
        }
    }

    /// A jumper between two adjacent terminals is drawn on the schematic as one short thick bar filling the gap between their two round marks, and nothing else is drawn around it.
    /// 隣り合う2端子間のジャンパは、図面上でその2つの印(丸)の隙間を埋める短い太いバー1本として描かれ、ほかの図形は足されない。
    #[test]
    fn svg_draws_a_jumper_bar_between_adjacent_terminals() {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        let tb = terminal_block("TB1", "terminal_block_4p", 100.0, 50.0, "1-2");
        sheet.entities.insert(tb.id(), tb);
        let svg = sheet_to_svg(&sheet, &crate::symbol::sheet_symbol_defs(&sheet), &[]);
        // 端子1中心 (100, 42.5)・端子2中心 (100, 47.5)。バーは2つの印の縁の間 (44.3〜45.7)
        let bars: Vec<_> = svg_lines(&svg)
            .into_iter()
            .filter(|l| (l.4 - JUMPER_BAR_STROKE).abs() < 1e-9)
            .collect();
        assert_eq!(bars.len(), 1, "太いバーは1本: {bars:?}");
        assert_eq!((bars[0].0, bars[0].1, bars[0].2, bars[0].3), (100.0, 44.3, 100.0, 45.7));
        // 端子の印以外の丸は描かれない (バーの端は印の縁そのものが示す)
        let extra: Vec<_> = svg_circles(&svg)
            .into_iter()
            .filter(|c| (c.2 - crate::symbol::TERMINAL_MARK_R).abs() > 1e-9)
            .collect();
        assert!(extra.is_empty(), "端子の印以外の丸が描かれている: {extra:?}");
    }

    /// On a multi-level terminal block, a jumper written with level letters ("1a-1b") is drawn on that pole's row as a bar in the gap between the two level columns' marks.
    /// 多段端子台で段記号付きに書かれたジャンパ ("1a-1b") は、その極の行の上、2つの段の列の印の隙間にバーとして描かれる。
    #[test]
    fn svg_draws_a_jumper_bar_across_the_levels_of_a_multilevel_block() {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        let tb = terminal_block("TB1", "terminal_block_4p_2l", 100.0, 50.0, "1a-1b");
        sheet.entities.insert(tb.id(), tb);
        let svg = sheet_to_svg(&sheet, &crate::symbol::sheet_symbol_defs(&sheet), &[]);
        // 極1の行は y=42.5、段aの列は x=95、段bの列は x=105。バーは2つの印の縁の間 (96.8〜103.2)
        let bars: Vec<_> = svg_lines(&svg)
            .into_iter()
            .filter(|l| (l.4 - JUMPER_BAR_STROKE).abs() < 1e-9)
            .collect();
        assert_eq!(bars.len(), 1, "太いバーは1本: {bars:?}");
        assert_eq!((bars[0].0, bars[0].1, bars[0].2, bars[0].3), (96.8, 42.5, 103.2, 42.5));
    }

    /// A jumper bar is part of its terminal block's symbol, so it is drawn or hidden with the symbol and never on its own: the export, which has no per-view-class visibility at all, always writes it out, however the block is turned.
    /// ジャンパバーは端子台シンボルの一部なので、シンボルと一緒に出たり隠れたりし、単独で消えることはない: 表示クラスの概念を持たない書き出しでは、端子台の向きにかかわらず常に描かれる。
    #[test]
    fn svg_export_always_draws_the_jumper_bar_with_its_symbol() {
        for (symbol_id, jumpers, rotation, mirror) in [
            ("terminal_block_4p", "1-2", 0u16, false),
            ("terminal_block_4p", "1-2", 90, false),
            ("terminal_block_4p_2l", "1a-1b", 0, true),
            ("terminal_block_4p_3l", "1a-1b,1b-1c", 270, false),
        ] {
            let svg = jumper_sheet(symbol_id, jumpers, rotation, mirror);
            let bars: Vec<_> = svg_lines(&svg)
                .into_iter()
                .filter(|l| (l.4 - JUMPER_BAR_STROKE).abs() < 1e-9)
                .collect();
            assert_eq!(
                bars.len(),
                jumpers.split(',').count(),
                "{symbol_id} {jumpers} rot={rotation} mirror={mirror}: \
                 書き出しにジャンパバーが全部載っていない: {bars:?}"
            );
            // シンボル本体 (端子の印) も同じ書き出しに載っている = バーだけが単独で出ることはない
            assert!(
                !svg_circles(&svg).is_empty(),
                "{symbol_id}: 端子の印が1つも無い (バーだけが単独で描かれている)"
            );
        }
    }

    /// A jumper bar turns with its terminal block, so on a block rotated a quarter turn the bar lies between the rotated terminals instead of staying upright.
    /// ジャンパバーは端子台と一緒に回るので、90度回した端子台では、バーも回転後の端子の間に横向きに描かれる。
    #[test]
    fn svg_draws_the_jumper_bar_turned_with_a_rotated_terminal_block() {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        let mut tb = terminal_block("TB1", "terminal_block_4p", 100.0, 50.0, "1-2");
        if let Entity::Symbol(s) = &mut tb {
            s.rotation = 90;
        }
        sheet.entities.insert(tb.id(), tb);
        let svg = sheet_to_svg(&sheet, &crate::symbol::sheet_symbol_defs(&sheet), &[]);
        // 90度回すと端子1は(107.5,50)、端子2は(102.5,50)へ移り、バーも2つの印の縁の間で横向きになる
        let bars: Vec<_> = svg_lines(&svg)
            .into_iter()
            .filter(|l| (l.4 - JUMPER_BAR_STROKE).abs() < 1e-9)
            .collect();
        assert_eq!(bars.len(), 1, "{bars:?}");
        assert_eq!((bars[0].0, bars[0].1, bars[0].2, bars[0].3), (105.7, 50.0, 104.3, 50.0));
    }

    /// The lettering inside a symbol (a motor's "M", for instance) is drawn at the place the symbol definition gives it and stays there whatever the symbol's rotation or mirroring, because the letters themselves are always written horizontally.
    /// シンボルの中の文字 (モータの「M」など) はシンボル定義が決めた場所に描かれ、シンボルを回しても反転してもその場所から動かない (文字そのものは常に水平に書くため)。
    #[test]
    fn the_lettering_inside_a_symbol_stays_put_when_the_symbol_is_turned() {
        for (rotation, mirror) in [(0u16, false), (90, false), (180, false), (270, false), (0, true)]
        {
            let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
            let mut motor = terminal_block("M1", "motor", 100.0, 50.0, "");
            if let Entity::Symbol(s) = &mut motor {
                s.rotation = rotation;
                s.mirror = mirror;
            }
            sheet.entities.insert(motor.id(), motor);
            let svg = sheet_to_svg(&sheet, &crate::symbol::sheet_symbol_defs(&sheet), &[]);
            let letters: Vec<_> = svg_texts(&svg).into_iter().filter(|t| t.3 == "M").collect();
            assert_eq!(letters.len(), 1, "rot={rotation} mirror={mirror}: {letters:?}");
            // 定義ではモータの円の中心 (0,0) に高さ4mmの「M」→ ベースラインは中心の2mm下
            assert_eq!(
                (letters[0].0, letters[0].1),
                (100.0, 52.0),
                "rot={rotation} mirror={mirror}"
            );
        }
    }

    /// 端子番号1つぶんの矩形と、その番号が載っている印 (一番近い印) の中心を返す。
    fn number_on_its_mark(
        b: (f64, f64, f64, f64),
        marks: &[(f64, f64, f64)],
    ) -> ((f64, f64), (f64, f64)) {
        let centre = ((b.0 + b.2) / 2.0, (b.1 + b.3) / 2.0);
        let mark = marks
            .iter()
            .min_by(|p, q| {
                let d = |m: &&(f64, f64, f64)| (centre.0 - m.0).hypot(centre.1 - m.1);
                d(p).total_cmp(&d(q))
            })
            .expect("印が1つも無い");
        (centre, (mark.0, mark.1))
    }

    /// A jumper bar between two renumbered terminals is attributed to the terminals whose printed labels it actually connects, matched by the label text rather than by mere geometric proximity — closing the phase-6 follow-up where a nearest-mark test could not detect swapped terminal numbers.
    /// 採番後のジャンパバーは、単に幾何的に近いというだけでなく、実際に繋ぐ端子の印刷ラベルのテキストと紐付けて帰属確認する。最寄りマークのテストでは端子番号の入れ替わりを検出できなかったフェーズ6の積み残しを閉じる。
    #[test]
    fn a_jumper_bar_is_attributed_to_its_terminals_by_label_text_after_renumbering() {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        let mut tb = terminal_block("TB1", "terminal_block_4p", 100.0, 50.0, "P11-P12");
        if let Entity::Symbol(s) = &mut tb {
            s.attrs.insert("terminal_numbers".to_string(), "P11,P12,P13,P14".to_string());
        }
        sheet.entities.insert(tb.id(), tb);
        let svg = sheet_to_svg(&sheet, &crate::symbol::sheet_symbol_defs(&sheet), &[]);

        let mark_r = crate::symbol::TERMINAL_MARK_R;
        let marks: Vec<_> =
            svg_circles(&svg).into_iter().filter(|c| (c.2 - mark_r).abs() < 1e-9).collect();
        let texts = svg_texts(&svg);
        let mark_for = |label: &str| -> (f64, f64) {
            let (x, y, size, _) = texts
                .iter()
                .find(|t| t.3 == label)
                .unwrap_or_else(|| panic!("ラベル{label}が描かれていない: {texts:?}"));
            let half = label.chars().count() as f64 * size * 0.6 / 2.0;
            number_on_its_mark((x - half, y - size * 0.7, x + half, *y), &marks).1
        };
        let (p11, p12) = (mark_for("P11"), mark_for("P12"));

        let bars: Vec<_> = svg_lines(&svg)
            .into_iter()
            .filter(|l| (l.4 - JUMPER_BAR_STROKE).abs() < 1e-9)
            .collect();
        assert_eq!(bars.len(), 1, "{bars:?}");
        let (bx1, by1, bx2, by2, _) = bars[0];
        let near = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).hypot(a.1 - b.1) < mark_r;
        // バーの両端は、ラベル"P11"の印の縁と"P12"の印の縁それぞれの近くにある
        // (印が入れ替わっていたら、あるいはラベルが取り違えられていたら失敗する)
        assert!(
            (near((bx1, by1), p11) && near((bx2, by2), p12))
                || (near((bx1, by1), p12) && near((bx2, by2), p11)),
            "バー({bx1},{by1})-({bx2},{by2}) がP11の印{p11:?}とP12の印{p12:?}に付いていない"
        );
    }

    /// A terminal number of one or two characters sits wholly inside its own round mark, centred on it, however the block is turned or mirrored, so a rotated terminal block still reads terminal by terminal.
    /// 1〜2文字の端子番号は、端子台をどの向きに回しても反転しても、自分の印(丸)の中に丸ごと収まり、その中央に載る。だから回した端子台でも端子ごとに番号が読める。
    #[test]
    fn a_turned_terminal_block_keeps_a_short_number_inside_its_own_mark() {
        let mark_r = crate::symbol::TERMINAL_MARK_R;
        // 1文字 (`1`〜`4`) と2文字 (`10`〜`12`) の端子番号を持つ端子台
        for (symbol_id, count) in [("terminal_block_4p", 4), ("terminal_block_12p", 12)] {
            for (rotation, mirror) in
                [(0u16, false), (90, false), (180, false), (270, false), (0, true)]
            {
                let svg = jumper_sheet(symbol_id, "1-2", rotation, mirror);
                let marks: Vec<_> = svg_circles(&svg)
                    .into_iter()
                    .filter(|c| (c.2 - mark_r).abs() < 1e-9)
                    .collect();
                let numbers = number_boxes(&svg);
                assert_eq!(numbers.len(), count, "{symbol_id} rot={rotation}: {numbers:?}");
                for b in numbers {
                    let (centre, mark) = number_on_its_mark(b, &marks);
                    let off = (centre.0 - mark.0).hypot(centre.1 - mark.1);
                    assert!(
                        off <= mark_r / 2.0,
                        "{symbol_id} rot={rotation} mirror={mirror}: 番号{b:?}が印の中央から{off}mmずれている"
                    );
                    for (x, y) in [(b.0, b.1), (b.2, b.1), (b.0, b.3), (b.2, b.3)] {
                        let d = (x - mark.0).hypot(y - mark.1);
                        assert!(
                            d <= mark_r,
                            "{symbol_id} rot={rotation} mirror={mirror}: 番号{b:?}の角が印から{}mm出る",
                            d - mark_r
                        );
                    }
                }
            }
        }
    }

    /// In the exported drawing (SVG/PDF) an unturned terminal block writes each terminal number with its baseline exactly half the character height (1.0 mm at the 2.0 mm terminal-number height) below the centre of its own mark, single-level and multi-level blocks alike; the on-screen canvas uses a smaller offset of its own.
    /// 出力図面(SVG/PDF)では、回していない端子台のどの端子番号も、自分の印の中心のちょうど文字高の半分(端子番号の文字高2.0mmに対して1.0mm)だけ下にベースラインが来る。単段でも多段でも同じ(画面のキャンバスはこれより小さい独自の下げ幅を使う)。
    #[test]
    fn a_terminal_number_sits_half_a_character_below_its_mark_centre() {
        let mark_r = crate::symbol::TERMINAL_MARK_R;
        let height = crate::symbol::TERMINAL_NUMBER_HEIGHT;
        for (symbol_id, count) in [("terminal_block_4p", 4), ("terminal_block_4p_2l", 8)] {
            let svg = jumper_sheet(symbol_id, "", 0, false);
            let marks: Vec<_> = svg_circles(&svg)
                .into_iter()
                .filter(|c| (c.2 - mark_r).abs() < 1e-9)
                .collect();
            let numbers: Vec<_> = svg_texts(&svg)
                .into_iter()
                .filter(|t| (t.2 - height).abs() < 1e-9)
                .collect();
            assert_eq!(marks.len(), count, "{symbol_id}: {marks:?}");
            assert_eq!(numbers.len(), count, "{symbol_id}: {numbers:?}");
            for m in &marks {
                let want = (m.0, m.1 + height / 2.0);
                assert_eq!(
                    numbers.iter().filter(|t| (t.0, t.1) == want).count(),
                    1,
                    "{symbol_id}: 印{m:?}の番号が{want:?}に無い ({numbers:?})"
                );
            }
        }
    }

    /// A three-character terminal number (a multi-level block's "10a") is as wide as its own mark, so it is centred on the mark and hangs at most three tenths of a millimetre over it, while still never running into a jumper bar or into a neighbouring terminal's mark.
    /// 3文字の端子番号 (多段端子台の`10a`など) は自分の印と同じ幅なので、印の中央に載せたうえで、印から出るのは最大0.3mmまでに収まる。そのうえでジャンパバーにも隣の端子の印にも掛からない。
    #[test]
    fn a_three_character_terminal_number_hangs_at_most_a_little_over_its_own_mark() {
        let mark_r = crate::symbol::TERMINAL_MARK_R;
        /// 印からはみ出してよい量 (mm)。
        const OVERHANG_MAX: f64 = 0.3;
        for (symbol_id, jumpers, count) in [
            ("terminal_block_12p_2l", "10a-10b,11a-12a", 24),
            ("terminal_block_12p_3l", "10a-10b,10b-10c", 36),
        ] {
            for rotation in [0u16, 90] {
                let svg = jumper_sheet(symbol_id, jumpers, rotation, false);
                let marks: Vec<_> = svg_circles(&svg)
                    .into_iter()
                    .filter(|c| (c.2 - mark_r).abs() < 1e-9)
                    .collect();
                let bars = jumper_boxes(&svg);
                assert!(!bars.is_empty(), "{symbol_id} rot={rotation}: バーが無い");
                let numbers = number_boxes(&svg);
                assert_eq!(numbers.len(), count, "{symbol_id} rot={rotation}: {numbers:?}");
                for b in numbers {
                    let (centre, mark) = number_on_its_mark(b, &marks);
                    let off = (centre.0 - mark.0).hypot(centre.1 - mark.1);
                    assert!(
                        off <= mark_r / 2.0,
                        "{symbol_id} rot={rotation}: 番号{b:?}が印の中央から{off}mmずれている"
                    );
                    for (x, y) in [(b.0, b.1), (b.2, b.1), (b.0, b.3), (b.2, b.3)] {
                        let over = (x - mark.0).hypot(y - mark.1) - mark_r;
                        assert!(
                            over <= OVERHANG_MAX,
                            "{symbol_id} rot={rotation}: 番号{b:?}の角が印から{over}mm出る"
                        );
                    }
                    for bar in &bars {
                        assert!(
                            !overlaps(*bar, b),
                            "{symbol_id} rot={rotation}: 番号{b:?}がジャンパバー{bar:?}に掛かる"
                        );
                    }
                    // 隣の端子の印には入らない (矩形の一番近い点が印の外)
                    for m in &marks {
                        if (m.0 - mark.0).abs() < 1e-9 && (m.1 - mark.1).abs() < 1e-9 {
                            continue;
                        }
                        let nx = m.0.clamp(b.0, b.2);
                        let ny = m.1.clamp(b.1, b.3);
                        let d = (nx - m.0).hypot(ny - m.1);
                        assert!(
                            d > mark_r,
                            "{symbol_id} rot={rotation}: 番号{b:?}が隣の印{m:?}に入る"
                        );
                    }
                }
            }
        }
    }

    /// `attrs["terminal_numbers"]` overrides which label is drawn on each mark: the symbol definition's raw pin number text never appears, and every overridden label appears exactly once.
    /// `attrs["terminal_numbers"]`は各印に描かれるラベルを上書きする: シンボル定義の生のピン番号は一切現れず、上書きラベルはそれぞれちょうど1回だけ現れる。
    #[test]
    fn svg_draws_the_overridden_terminal_number_instead_of_the_raw_pin_number() {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        let mut tb = terminal_block("TB1", "terminal_block_4p", 100.0, 50.0, "");
        if let Entity::Symbol(s) = &mut tb {
            s.attrs.insert("terminal_numbers".to_string(), "11,12,13,14".to_string());
        }
        sheet.entities.insert(tb.id(), tb);
        let svg = sheet_to_svg(&sheet, &crate::symbol::sheet_symbol_defs(&sheet), &[]);
        let numbers: Vec<String> = svg_texts(&svg)
            .into_iter()
            .filter(|t| (t.2 - crate::symbol::TERMINAL_NUMBER_HEIGHT).abs() < 1e-9)
            .map(|t| t.3)
            .collect();
        let mut sorted = numbers.clone();
        sorted.sort();
        assert_eq!(sorted, ["11", "12", "13", "14"], "{numbers:?}");
        for raw in ["1", "2", "3", "4"] {
            assert!(!numbers.iter().any(|n| n == raw), "生のピン番号{raw}が残っている: {numbers:?}");
        }
    }

    /// A three-character overridden label (a renumbered multi-level pole like "11a", or a prefixed pole like "P12") stays inside the legibility bounds already proven for a symbol-default three-character number: centred on its own mark, hanging over it by at most the same tolerance, and clear of the jumper bar and neighbouring marks.
    /// 採番などで生じた3文字の上書きラベル (多段の"11a"や接頭辞付き"P12") も、シンボル既定の3文字番号で確かめた可読性の範囲に収まる: 自分の印の中央に載り、はみ出しは同じ許容量まで、ジャンパバーにも隣の印にも掛からない。
    #[test]
    fn an_overridden_three_character_label_hangs_at_most_a_little_over_its_own_mark() {
        let mark_r = crate::symbol::TERMINAL_MARK_R;
        const OVERHANG_MAX: f64 = 0.3;
        for (symbol_id, numbers, jumpers, count) in [
            ("terminal_block_4p_2l", "11a,11b,12a,12b,13a,13b,14a,14b", "11a-11b,11a-12a", 8),
            ("terminal_block_4p", "P11,P12,P13,P14", "P11-P12", 4),
        ] {
            let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
            let mut tb = terminal_block("TB1", symbol_id, 200.0, 150.0, jumpers);
            if let Entity::Symbol(s) = &mut tb {
                s.attrs.insert("terminal_numbers".to_string(), numbers.to_string());
            }
            sheet.entities.insert(tb.id(), tb);
            let svg = sheet_to_svg(&sheet, &crate::symbol::sheet_symbol_defs(&sheet), &[]);
            let marks: Vec<_> =
                svg_circles(&svg).into_iter().filter(|c| (c.2 - mark_r).abs() < 1e-9).collect();
            let bars = jumper_boxes(&svg);
            assert!(!bars.is_empty(), "{symbol_id}: バーが無い");
            let numbers = number_boxes(&svg);
            assert_eq!(numbers.len(), count, "{symbol_id}: {numbers:?}");
            for b in numbers {
                let (centre, mark) = number_on_its_mark(b, &marks);
                let off = (centre.0 - mark.0).hypot(centre.1 - mark.1);
                assert!(off <= mark_r / 2.0, "{symbol_id}: 番号{b:?}が印の中央から{off}mmずれている");
                for (x, y) in [(b.0, b.1), (b.2, b.1), (b.0, b.3), (b.2, b.3)] {
                    let over = (x - mark.0).hypot(y - mark.1) - mark_r;
                    assert!(over <= OVERHANG_MAX, "{symbol_id}: 番号{b:?}の角が印から{over}mm出る");
                }
                for bar in &bars {
                    assert!(!overlaps(*bar, b), "{symbol_id}: 番号{b:?}がジャンパバー{bar:?}に掛かる");
                }
            }
        }
    }

    /// A jumper bar is drawn at least twice as thick as an ordinary symbol line, so that the short bar in the narrow gap between two terminal marks still reads as a shorting bar when the sheet is printed.
    /// ジャンパバーはシンボルの普通の線の2倍以上の太さで描かれるので、端子の印どうしの狭い隙間に入る短いバーでも、印刷したときにショートバーだと分かる。
    #[test]
    fn the_jumper_bar_is_at_least_twice_as_thick_as_a_symbol_line() {
        assert_eq!(JUMPER_BAR_STROKE, 0.8, "ジャンパバーの太さは0.8mm");
        assert!(
            JUMPER_BAR_STROKE >= 2.0 * SYMBOL_STROKE,
            "バー{JUMPER_BAR_STROKE}mmはシンボルの線{SYMBOL_STROKE}mmの2倍以上"
        );
    }

    /// A jumper written between two terminals that are not adjacent is invalid (the same rule the terminal chart enforces), so no bar is drawn for it at all.
    /// 隣り合わない2端子間のジャンパは(端子台チャートと同じ規則で)不正なので、バーはまったく描かれない。
    #[test]
    fn svg_draws_no_bar_for_a_jumper_between_non_adjacent_terminals() {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        let tb = terminal_block("TB1", "terminal_block_4p", 100.0, 50.0, "1-3");
        sheet.entities.insert(tb.id(), tb);
        let svg = sheet_to_svg(&sheet, &crate::symbol::sheet_symbol_defs(&sheet), &[]);
        let bars: Vec<_> = svg_lines(&svg)
            .into_iter()
            .filter(|l| (l.4 - JUMPER_BAR_STROKE).abs() < 1e-9)
            .collect();
        assert!(bars.is_empty(), "離れた端子には描かない: {bars:?}");
    }

    /// A terminal block with several jumpers gets one bar per jumper.
    /// 複数のジャンパを持つ端子台には、ジャンパの数だけバーが描かれる。
    #[test]
    fn svg_draws_one_bar_per_jumper_when_there_are_several() {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        let tb = terminal_block("TB1", "terminal_block_4p", 100.0, 50.0, "1-2,3-4");
        sheet.entities.insert(tb.id(), tb);
        let svg = sheet_to_svg(&sheet, &crate::symbol::sheet_symbol_defs(&sheet), &[]);
        let bars: Vec<_> = svg_lines(&svg)
            .into_iter()
            .filter(|l| (l.4 - JUMPER_BAR_STROKE).abs() < 1e-9)
            .collect();
        assert_eq!(bars.len(), 2, "{bars:?}");
    }

    /// A terminal block with no jumpers draws no bar, and a symbol that is not a terminal block never gets one even if it happened to carry a "jumpers" attribute.
    /// ジャンパの無い端子台にはバーが描かれず、端子台以外のシンボルは"jumpers"属性を持っていてもバーが描かれない。
    #[test]
    fn svg_draws_no_bar_without_jumpers_or_on_a_non_terminal_block() {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        let plain_tb = terminal_block("TB1", "terminal_block_4p", 100.0, 50.0, "");
        let mut not_tb = terminal_block("R1", "resistor", 150.0, 50.0, "");
        if let Entity::Symbol(s) = &mut not_tb {
            s.attrs.insert("jumpers".into(), "1-2".into());
        }
        sheet.entities.insert(plain_tb.id(), plain_tb);
        sheet.entities.insert(not_tb.id(), not_tb);
        let svg = sheet_to_svg(&sheet, &crate::symbol::sheet_symbol_defs(&sheet), &[]);
        let bars: Vec<_> = svg_lines(&svg)
            .into_iter()
            .filter(|l| (l.4 - JUMPER_BAR_STROKE).abs() < 1e-9)
            .collect();
        assert!(bars.is_empty(), "{bars:?}");
    }

    fn with_pin_groups(entity: Entity, spec: &str) -> Entity {
        let Entity::Symbol(mut s) = entity else { panic!("シンボルであること") };
        s.attrs.insert(crate::pin_groups::PIN_GROUPS_ATTR.into(), spec.into());
        Entity::Symbol(s)
    }

    /// ピングループ括弧の本数 (4頂点のポリラインだけを数える。端子台自身が描く2頂点の
    /// 足の短い引き線と区別するため)。
    fn bracket_polylines(svg: &str) -> usize {
        svg.lines()
            .filter(|l| {
                l.starts_with("<polyline")
                    && l.split("points=\"")
                        .nth(1)
                        .and_then(|s| s.split('"').next())
                        .map(|pts| pts.split(' ').count() == 4)
                        .unwrap_or(false)
            })
            .count()
    }

    /// A pin_groups range draws one square-bracket polyline and its caption text on the sheet.
    /// pin_groups の範囲は、角括弧のポリライン1本とそのキャプション文字をシートに描く。
    #[test]
    fn svg_draws_a_bracket_and_caption_for_a_pin_group() {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        let tb = with_pin_groups(
            terminal_block("TB1", "terminal_block_4p", 100.0, 50.0, ""),
            "1-4:電源",
        );
        sheet.entities.insert(tb.id(), tb);
        let svg = sheet_to_svg(&sheet, &crate::symbol::sheet_symbol_defs(&sheet), &[]);
        assert_eq!(bracket_polylines(&svg), 1, "括弧のポリラインが無い: {svg}");
        assert!(svg.contains(">電源<"), "キャプションが無い: {svg}");
    }

    /// After the terminal numbers are renumbered, the pin-group bracket keeps being drawn: the range is matched against the block's displayed numbers, which is what the renumbering rewrote it to.
    /// 端子番号を採番したあとでもピングループの括弧は描かれ続ける: 範囲はその端子台の表示番号と突き合わせられ、採番はその表示番号へ範囲を書き換えているため。
    #[test]
    fn svg_still_draws_the_bracket_after_the_terminals_are_renumbered() {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        let tb = terminal_block("TB1", "terminal_block_4p", 100.0, 50.0, "");
        let Entity::Symbol(mut s) = with_pin_groups(tb, "11-14:電源") else { panic!() };
        // 採番後の状態 (RenumberTerminals が書く2つの属性)
        s.attrs.insert(crate::terminal_chart::TERMINAL_NUMBERS_ATTR.into(), "11,12,13,14".into());
        let tb = Entity::Symbol(s);
        sheet.entities.insert(tb.id(), tb);
        let svg = sheet_to_svg(&sheet, &crate::symbol::sheet_symbol_defs(&sheet), &[]);
        assert_eq!(bracket_polylines(&svg), 1, "採番後に括弧が消えている: {svg}");
        assert!(svg.contains(">電源<"), "キャプションが無い: {svg}");
        assert!(svg.contains(">11<"), "端子番号が表示ラベルで描かれる: {svg}");
    }

    /// A symbol with no pin_groups attribute, or an empty one, draws no bracket polyline.
    /// pin_groups属性が無い・空のシンボルには括弧のポリラインが描かれない。
    #[test]
    fn svg_draws_no_bracket_without_pin_groups() {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        let plain = terminal_block("TB1", "terminal_block_4p", 100.0, 50.0, "");
        let empty = with_pin_groups(terminal_block("TB2", "terminal_block_4p", 200.0, 50.0, ""), "");
        sheet.entities.insert(plain.id(), plain);
        sheet.entities.insert(empty.id(), empty);
        let svg = sheet_to_svg(&sheet, &crate::symbol::sheet_symbol_defs(&sheet), &[]);
        assert_eq!(bracket_polylines(&svg), 0, "{svg}");
    }

    /// Overlapping pin_groups items are reported by verify (tested in verify.rs) and draw no bracket at all, while an unrelated valid item on the same instance still draws.
    /// 重なるpin_groups項目は (verify.rsで検証済みのとおり) 括弧を一切描かず、同じインスタンスの関係ない有効な項目は変わらず描かれる。
    #[test]
    fn svg_draws_nothing_for_overlapping_pin_groups_but_keeps_the_rest() {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        let tb = with_pin_groups(
            terminal_block("TB1", "terminal_block_8p", 100.0, 50.0, ""),
            "1-4:A,3-6:B,7-8:C",
        );
        sheet.entities.insert(tb.id(), tb);
        let svg = sheet_to_svg(&sheet, &crate::symbol::sheet_symbol_defs(&sheet), &[]);
        // キャプション文字はfont-size="2" (図枠のゾーン記号A〜Fはfont-size="3"なので混同しない)
        let captioned = |letter: &str| svg.contains(&format!("font-size=\"2\" fill=\"#000\" text-anchor=\"middle\" font-family=\"sans-serif\">{letter}<"));
        assert!(!captioned("A") && !captioned("B"), "重なる2件は描かれない: {svg}");
        assert!(captioned("C"), "重ならない項目は描かれる: {svg}");
        assert_eq!(bracket_polylines(&svg), 1, "重ならないCの分だけ: {svg}");
    }

    /// 4ピンのブロック (左辺に1〜4)。ピン番号はブロック自身が足の脇に書く。
    fn four_pin_block(reference: &str, x: f64, y: f64) -> Entity {
        let mut attrs = std::collections::BTreeMap::new();
        attrs.insert(
            crate::block::BLOCK_PINS_ATTR.to_string(),
            r#"{"left":[{"no":"1","name":"IN1"},{"no":"2","name":"IN2"},
                {"no":"3","name":"IN3"},{"no":"4","name":"IN4"}]}"#
                .to_string(),
        );
        Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: crate::block::BLOCK_SYMBOL_ID.into(),
            at: Point::new(x, y),
            rotation: 0,
            mirror: false,
            reference: reference.into(),
            value: String::new(),
            attrs,
        })
    }

    /// SVGの全ての`<text>`の (本文, 占める矩形)。文字は常に水平なので、幅は`text_width`、
    /// 高さはベースラインから文字高ぶん上に取る (描画側と同じ約束)。
    fn all_text_boxes(svg: &str) -> Vec<(String, (f64, f64, f64, f64))> {
        svg.lines()
            .filter(|l| l.trim_start().starts_with("<text "))
            .filter_map(|l| {
                let size = attr(l, "font-size")?;
                let (x, y) = (attr(l, "x")?, attr(l, "y")?);
                let body = l.split_once('>')?.1.rsplit_once("</text>")?.0.to_string();
                let w = crate::tidy::text_width(&body, size);
                let x0 = match l.split("text-anchor=\"").nth(1).and_then(|s| s.split('"').next()) {
                    Some("middle") => x - w / 2.0,
                    Some("end") => x - w,
                    _ => x,
                };
                Some((body, (x0, y - size, x0 + w, y)))
            })
            .collect()
    }

    /// 括弧+キャプションを持つシンボル1個だけのシートを描いて、そのSVGと、ピングループの
    /// 箱 (括弧1つ+キャプション各行) を返す。キャプションは3行に割れる長さにする。
    fn pin_group_case(
        id: &str,
        spec: &str,
        rotation: u16,
        mirror: bool,
    ) -> (String, Vec<(f64, f64, f64, f64)>) {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        let base = if id == "block" {
            four_pin_block("B1", 150.0, 100.0)
        } else {
            terminal_block("TB1", id, 150.0, 100.0, "")
        };
        let mut inst = with_pin_groups(base, spec);
        if let Entity::Symbol(s) = &mut inst {
            s.rotation = rotation;
            s.mirror = mirror;
            s.value = "AB-1234".into();
            s.attrs
                .insert(crate::text_style::FUNCTION_LABEL_ATTR.into(), "非常停止".into());
        }
        let (entity_id, symbol) = match &inst {
            Entity::Symbol(s) => (s.id, s.clone()),
            _ => unreachable!(),
        };
        sheet.entities.insert(entity_id, inst);
        let defs = crate::symbol::sheet_symbol_defs(&sheet);
        let def = crate::symbol::DefIndex::new(&defs).get(&symbol).unwrap();
        let boxes = crate::pin_groups::pin_group_paper_boxes(&symbol, &def)
            .into_iter()
            .map(|(min, max)| (min.x, min.y, max.x, max.y))
            .collect();
        (sheet_to_svg(&sheet, &defs, &[]), boxes)
    }

    /// 3行に割れるキャプション (4文字×2+2文字)。
    const THREE_LINE_SPEC_CAPTION: &str = "電源系統一式予備回路";

    /// Pin group brackets and their captions never land on a pin number, a pin name or the symbol's designator, type or function label, for a block, a terminal block and a connector, at every rotation and mirrored.
    /// ピングループの括弧とキャプションは、ブロック・端子台・コネクタのいずれでも、全ての回転とミラーで、ピン番号・ピン名・参照記号・型番・機能ラベルのどれにも重ならない。
    #[test]
    fn svg_pin_group_boxes_never_land_on_any_label_at_any_rotation() {
        for (id, range) in [("block", "1-2"), ("terminal_block_8p", "1-4"), ("connector_8p", "1-4")]
        {
            let spec = format!("{range}:{THREE_LINE_SPEC_CAPTION}");
            for rotation in [0u16, 90, 180, 270] {
                for mirror in [false, true] {
                    let (svg, group_boxes) = pin_group_case(id, &spec, rotation, mirror);
                    let where_ = format!("{id} rot={rotation} mirror={mirror}");
                    assert!(!group_boxes.is_empty(), "括弧が描かれていない ({where_})");
                    for g in &group_boxes {
                        for (body, t) in all_text_boxes(&svg) {
                            // キャプションの行そのものは比べない。文字列ではなく箱の一致で
                            // 除く (同じ文字の参照記号・機能ラベルを取りこぼさないため)
                            if group_boxes.iter().any(|b| *b == t) {
                                continue;
                            }
                            assert!(!overlaps(*g, t), "「{body}」に重なった {g:?} vs {t:?} ({where_})");
                        }
                    }
                }
            }
        }
    }

    /// No two pieces of a pin group - the bracket and each caption line - ever overlap each other, so a caption split over several lines stays readable at every rotation and mirrored.
    /// ピングループの部品どうし (括弧と、折り返したキャプションの各行) は決して重ならないので、複数行に割れたキャプションは全ての回転・ミラーで読める。
    #[test]
    fn svg_pin_group_pieces_never_overlap_each_other_at_any_rotation() {
        for (id, range) in [("block", "1-2"), ("terminal_block_8p", "1-4"), ("connector_8p", "1-4")]
        {
            let spec = format!("{range}:{THREE_LINE_SPEC_CAPTION}");
            for rotation in [0u16, 90, 180, 270] {
                for mirror in [false, true] {
                    let (_, boxes) = pin_group_case(id, &spec, rotation, mirror);
                    let where_ = format!("{id} rot={rotation} mirror={mirror}");
                    assert_eq!(boxes.len(), 4, "括弧1つ+キャプション3行 ({where_})");
                    for i in 0..boxes.len() {
                        for j in (i + 1)..boxes.len() {
                            assert!(
                                !overlaps(boxes[i], boxes[j]),
                                "{:?} と {:?} が重なった ({where_})",
                                boxes[i],
                                boxes[j]
                            );
                        }
                    }
                }
            }
        }
    }

    /// A caption split over several lines is always stacked downwards on the paper - the lines share one centre line and step down one after another - because lettering stays horizontal however the symbol is turned.
    /// 複数行に割れたキャプションは、シンボルをどう回しても文字は横書きのままなので、用紙の上では必ず縦に積まれる (各行は同じ中心線に並び、順に下へ下がる)。
    #[test]
    fn svg_pin_group_caption_lines_stack_downwards_on_paper() {
        let spec = format!("1-4:{THREE_LINE_SPEC_CAPTION}");
        for rotation in [0u16, 90, 180, 270] {
            for mirror in [false, true] {
                let (_, boxes) = pin_group_case("terminal_block_8p", &spec, rotation, mirror);
                let where_ = format!("rot={rotation} mirror={mirror}");
                // 先頭が括弧、残りがキャプションの行 (pin_group_paper_boxes の並び)
                let captions = &boxes[1..];
                let cx = |b: &(f64, f64, f64, f64)| (b.0 + b.2) / 2.0;
                for c in captions {
                    assert!((cx(c) - cx(&captions[0])).abs() < 1e-9, "行の中心がずれた ({where_})");
                }
                for pair in captions.windows(2) {
                    assert!(pair[0].3 <= pair[1].1 + 1e-9, "行が下へ積まれていない ({where_})");
                }
            }
        }
    }

    /// A pin group's caption block sits wholly beyond its bracket: no caption line touches the bracket, and every line is further from the symbol than the bracket bar, at every rotation and mirrored.
    /// ピングループのキャプションは塊ごと括弧の外側に出る: どの行も括弧に触れず、シンボルから見て括弧の棒より遠くにある。回転・ミラーでも変わらない。
    #[test]
    fn svg_pin_group_caption_block_sits_wholly_beyond_its_bracket() {
        let spec = format!("1-4:{THREE_LINE_SPEC_CAPTION}");
        for rotation in [0u16, 90, 180, 270] {
            for mirror in [false, true] {
                let (_, boxes) = pin_group_case("terminal_block_8p", &spec, rotation, mirror);
                let where_ = format!("rot={rotation} mirror={mirror}");
                let bracket = boxes[0];
                // 括弧の棒は用紙の縦か横に揃うので、外向きは「シンボルから遠ざかる軸」1本
                let at = Point::new(150.0, 100.0);
                let horizontal = (bracket.2 - bracket.0) < (bracket.3 - bracket.1);
                for c in &boxes[1..] {
                    assert!(!overlaps(bracket, *c), "キャプションが括弧に重なった ({where_})");
                    let (cb, bb) = if horizontal {
                        // 縦棒 → 外向きは左右
                        (
                            ((c.0 + c.2) / 2.0 - at.x).abs(),
                            ((bracket.0 + bracket.2) / 2.0 - at.x).abs(),
                        )
                    } else {
                        (
                            ((c.1 + c.3) / 2.0 - at.y).abs(),
                            ((bracket.1 + bracket.3) / 2.0 - at.y).abs(),
                        )
                    };
                    assert!(cb > bb, "キャプションが括弧より内側 ({where_}): {c:?} vs {bracket:?}");
                }
            }
        }
    }

    /// A coil shown as a device block reserves no room for pin-group brackets even if it carries the attribute, because none are drawn there: its outline, and so its designator and contact map, sit exactly where they would without the attribute.
    /// 実機ブロック表示のコイルは、ピングループの属性を持っていても括弧のための余白を取らない (そこには括弧を描かないため)。外形も、それに従う参照記号・接点マップの位置も、属性が無いときと寸分違わない。
    #[test]
    fn a_device_block_coil_reserves_no_room_for_brackets_it_never_draws() {
        let mut coil = crate::model::SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: crate::relay_xref::COIL_SYMBOL_ID.into(),
            at: Point::new(150.0, 100.0),
            rotation: 0,
            mirror: false,
            reference: "K1".into(),
            value: String::new(),
            attrs: Default::default(),
        };
        coil.attrs
            .insert(crate::relay_xref::DEVICE_BLOCK_ATTR.into(), "true".into());
        let def = crate::symbol::resolve_symbol(crate::relay_xref::COIL_SYMBOL_ID).unwrap();
        let without = symbol_bounds(&coil, &def);
        // 単ピンの指定なら普通のシンボルでは括弧が描かれる (この対比が効くように選ぶ)
        coil.attrs
            .insert(crate::pin_groups::PIN_GROUPS_ATTR.into(), "A1:コイル電源".into());
        let mut plain = coil.clone();
        plain.attrs.remove(crate::relay_xref::DEVICE_BLOCK_ATTR);
        assert_ne!(symbol_bounds(&plain, &def), without, "普通のコイルなら括弧のぶん広がる");
        assert_eq!(symbol_bounds(&coil, &def), without, "実機ブロック表示は余白を取らない");
    }

    /// The dot at the target end of a note's leader line is drawn exactly the size of a wire junction dot, so the two read as the same mark.
    /// 注記の引き出し線の対象端に描く丸印は、配線の分岐点の丸印とぴったり同じ大きさで描かれる (図面上で同じ印に見える)。
    #[test]
    fn svg_leader_dot_is_the_size_of_a_wire_junction_dot() {
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        let target = Entity::Junction(crate::model::Junction {
            id: Uuid::new_v4(),
            at: Point::new(150.0, 100.0),
        });
        let target_id = target.id();
        let mut note = TextEntity {
            id: Uuid::new_v4(),
            at: Point::new(120.0, 80.0),
            text: "注意".into(),
            height: 2.5,
            rotation: 0,
            anchor: TextAnchor::Start,
            color: None,
            bold: false,
            leader_to: None,
        };
        note.leader_to = Some(target_id);
        sheet.entities.insert(target_id, target);
        sheet.entities.insert(note.id, Entity::Text(note));
        let svg = sheet_to_svg(&sheet, &crate::symbol::sheet_symbol_defs(&sheet), &[]);
        let radii: Vec<f64> = svg
            .lines()
            .filter(|l| l.trim_start().starts_with("<circle "))
            .filter_map(|l| attr(l, "r"))
            .collect();
        assert_eq!(radii.len(), 2, "分岐点と引き出し線の丸印が1つずつ: {svg}");
        assert_eq!(radii[0], radii[1], "同じ大きさ: {svg}");
        assert_eq!(radii[0], JUNCTION_DOT_R);
    }
}
