//! 印刷品質のSVGエクスポート。JIS図枠(枠線・ゾーン番号・表題欄)+全エンティティを描く。

use std::fmt::Write as _;

use crate::geometry::Point;
use crate::model::{Entity, Revision, Sheet};
use crate::netlist::transform_local;
use crate::symbol::{Primitive, SymbolDef};

/// 図枠の用紙端からのマージン (mm)。
const FRAME_MARGIN: f64 = 10.0;
/// 配線の線幅 (mm)。
const WIRE_STROKE: f64 = 0.35;
/// シンボルの線幅 (mm)。
const SYMBOL_STROKE: f64 = 0.3;
/// 表題欄の外形 (幅, 高さ) mm。右下に置く。
const TITLE_W: f64 = 120.0;
const TITLE_H: f64 = 32.0;
/// 表題欄・改訂欄の共通行高 (mm)。
const ROW_H: f64 = 8.0;
/// 枠線の線幅 (mm)。
const FRAME_STROKE: f64 = 0.5;
/// 罫線の線幅 (mm)。
const RULE_STROKE: f64 = 0.25;
/// 表題欄の文字高さ (mm)。
const TITLE_FONT: f64 = 3.0;
/// 改訂欄の列幅 (記号 / 日付 / 内容 / 承認) mm。合計は表題欄の幅と一致させる。
const REV_COL_W: [f64; 4] = [14.0, 28.0, 56.0, 22.0];
/// 改訂欄の列見出し (ISO 7200 / JIS Z 8311)。
const REV_HEADERS: [&str; 4] = ["記号", "日付", "内容", "承認"];
/// 改訂欄に描く最大行数。溢れた分は古い行から省略する (データは保持)。
const REV_MAX_ROWS: usize = 6;
/// 改訂欄の本文・列見出しの文字高さ (mm)。
const REV_FONT: f64 = 2.5;
const REV_HEADER_FONT: f64 = 2.2;
/// セル内テキストの左余白 (mm)。
const CELL_PAD: f64 = 2.0;
/// 線番テキストの文字高さ (mm)。
const WIRE_NO_FONT: f64 = 2.5;
/// 線番テキストと配線の間隔 (mm)。グリッドピッチと同じ。
const WIRE_NO_GAP: f64 = 2.5;
/// ハーネス境界の破線 (IEC 61082-1 のグループ囲み)。線の長さと間隔 mm。
const HARNESS_DASH: (f64, f64) = (3.0, 2.0);
/// ハーネス境界の線幅 (mm)。図面の主線より細い補助線。
const HARNESS_STROKE: f64 = 0.25;
/// ハーネス名の文字高さ (mm)。
const HARNESS_FONT: f64 = 2.5;
/// ハーネス名の位置: 囲みの左上角から右へ / 上へ (mm)。
const HARNESS_LABEL_DX: f64 = 1.0;
const HARNESS_LABEL_DY: f64 = 1.0;

/// 図面に描く改訂行(古い順)。上限を超えた分は古い行から省く。
pub fn visible_revisions(revisions: &[Revision]) -> &[Revision] {
    &revisions[revisions.len().saturating_sub(REV_MAX_ROWS)..]
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

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// 数値整形: 無駄な小数を出さない。
fn n(v: f64) -> String {
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

fn text_el(out: &mut String, x: f64, y: f64, size: f64, fill: &str, anchor: &str, s: &str) {
    text_family_el(out, x, y, size, fill, anchor, "sans-serif", s);
}

fn line_el(out: &mut String, x1: f64, y1: f64, x2: f64, y2: f64, w: f64, stroke: &str) {
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

fn frame_and_title_block(out: &mut String, sheet: &Sheet) {
    let (pw, ph) = sheet.paper_mm();
    let (x0, y0) = (FRAME_MARGIN, FRAME_MARGIN);
    let (x1, y1) = (pw - FRAME_MARGIN, ph - FRAME_MARGIN);
    let _ = write!(
        out,
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"#000\" stroke-width=\"0.5\"/>\n",
        n(x0), n(y0), n(x1 - x0), n(y1 - y0)
    );
    // ゾーン番号: 横=数字、縦=英字
    let cols = sheet.zone_cols.max(1) as f64;
    let rows = sheet.zone_rows.max(1) as f64;
    let zw = (x1 - x0) / cols;
    let zh = (y1 - y0) / rows;
    for i in 0..sheet.zone_cols.max(1) {
        let cx = x0 + zw * (i as f64 + 0.5);
        text_el(out, cx, y0 - 3.0, 3.0, "#000", "middle", &(i + 1).to_string());
        text_el(out, cx, y1 + 6.0, 3.0, "#000", "middle", &(i + 1).to_string());
        if i > 0 {
            let tx = x0 + zw * i as f64;
            line_el(out, tx, y0 - 2.0, tx, y0, 0.25, "#000");
            line_el(out, tx, y1, tx, y1 + 2.0, 0.25, "#000");
        }
    }
    for i in 0..sheet.zone_rows.max(1) {
        let cy = y0 + zh * (i as f64 + 0.5) + 1.0;
        let letter = char::from(b'A' + (i % 26) as u8).to_string();
        text_el(out, x0 - 3.0, cy, 3.0, "#000", "middle", &letter);
        text_el(out, x1 + 3.0, cy, 3.0, "#000", "middle", &letter);
    }
    // 表題欄 (右下、120x32mm、4行)
    let (tw, th) = (TITLE_W, TITLE_H);
    let (tx, ty) = (x1 - tw, y1 - th);
    let tb = &sheet.title_block;
    revision_block(out, sheet, tx, ty);
    let _ = write!(
        out,
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#fff\" stroke=\"#000\" stroke-width=\"{}\"/>\n",
        n(tx), n(ty), n(tw), n(th), n(FRAME_STROKE)
    );
    for r in 1..4 {
        let ry = ty + ROW_H * r as f64;
        line_el(out, tx, ry, tx + tw, ry, RULE_STROKE, "#000");
    }
    line_el(out, tx + 24.0, ty, tx + 24.0, ty + th, RULE_STROKE, "#000");
    let rows4: [(&str, String); 4] = [
        ("図番", format!("{}  Rev {}", tb.drawing_no, effective_rev(sheet))),
        ("品名", tb.title.clone()),
        ("尺度", format!("{}    日付 {}", tb.scale, tb.date)),
        (
            "設計",
            format!(
                "{}  製図 {}  検図 {}  承認 {}",
                tb.designed, tb.drawn, tb.checked, tb.approved
            ),
        ),
    ];
    for (i, (label, value)) in rows4.iter().enumerate() {
        let cy = ty + ROW_H * i as f64 + 5.5;
        text_el(out, tx + CELL_PAD, cy, TITLE_FONT, "#000", "start", label);
        text_el(out, tx + 26.0, cy, TITLE_FONT, "#000", "start", value);
    }
    if !tb.company.is_empty() {
        text_el(out, tx - 2.0, y1 - 2.0, TITLE_FONT, "#000", "end", &tb.company);
    }
}

/// 改訂欄 (ISO 7200 / JIS Z 8311)。表題欄の直上に同じ右端・同じ幅で描き、
/// 最下段を列見出し、その上に古い改訂から順に積む (最新が最上段)。0行なら何も描かない。
fn revision_block(out: &mut String, sheet: &Sheet, tx: f64, ty: f64) {
    let rows = visible_revisions(&sheet.revisions);
    if rows.is_empty() {
        return;
    }
    // 高さ = (改訂行 + 列見出し1行) × 行高。下端は表題欄の上端。
    let h = (rows.len() as f64 + 1.0) * ROW_H;
    let top = ty - h;
    let _ = write!(
        out,
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#fff\" stroke=\"#000\" stroke-width=\"{}\"/>\n",
        n(tx), n(top), n(TITLE_W), n(h), n(FRAME_STROKE)
    );
    for r in 1..=rows.len() {
        let ry = top + ROW_H * r as f64;
        line_el(out, tx, ry, tx + TITLE_W, ry, RULE_STROKE, "#000");
    }
    let mut cx = tx;
    for w in REV_COL_W.iter().take(REV_COL_W.len() - 1) {
        cx += w;
        line_el(out, cx, top, cx, ty, RULE_STROKE, "#000");
    }
    // セル左端 (記号 / 日付 / 内容 / 承認)
    let mut lefts = [0.0f64; 4];
    let mut x = tx;
    for (i, w) in REV_COL_W.iter().enumerate() {
        lefts[i] = x;
        x += w;
    }
    let baseline = |row_top: f64, font: f64| row_top + ROW_H / 2.0 + font / 2.0;
    // 列見出しは最下段 (表題欄側)
    let head_top = ty - ROW_H;
    for (i, label) in REV_HEADERS.iter().enumerate() {
        let cy = baseline(head_top, REV_HEADER_FONT);
        text_el(out, lefts[i] + CELL_PAD, cy, REV_HEADER_FONT, "#000", "start", label);
    }
    // 改訂行: 古い行 (rows[0]) が見出しの直上、新しい行ほど上へ積む
    for (i, rev) in rows.iter().enumerate() {
        let row_top = ty - (i as f64 + 2.0) * ROW_H;
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
fn symbol_top_y(inst: &crate::model::SymbolInstance, def: &SymbolDef) -> f64 {
    let mut top = inst.at.y;
    let mut visit = |p: Point| {
        let t = transform_local(p, inst);
        if t.y < top {
            top = t.y;
        }
    };
    for prim in &def.primitives {
        match prim {
            Primitive::Line { pts } => pts.iter().copied().for_each(&mut visit),
            // 円・弧は回転対称なので中心±rで安全側に評価
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
    top
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
                // 単純化: 開始/終了角の弧をパスで描く(回転はtransform_localで中心のみ反映)
                let c = transform_local(*center, inst);
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
        text_el(out, inst.at.x, top - 4.5, 2.5, "#000", "middle", &inst.reference);
    }
    if !inst.value.is_empty() {
        text_el(out, inst.at.x, top - 1.0, 2.5, "#000", "middle", &inst.value);
    }
}

/// ネットの代表線分 (最も長い線分)。同長なら上・左の線分を選ぶ。
fn longest_segment(sheet: &Sheet, wire_ids: &[crate::model::EntityId]) -> Option<(Point, Point)> {
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

fn midpoint(a: &Point, b: &Point) -> Point {
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

/// シート1枚を完全なSVG文書として書き出す。座標系はmm 1:1。
pub fn sheet_to_svg(sheet: &Sheet, symbols: &[SymbolDef]) -> String {
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
    frame_and_title_block(&mut out, sheet);
    // ハーネス境界は配線・シンボルの背面に置く
    render_harnesses(&mut out, sheet);
    let defs: std::collections::BTreeMap<&str, &SymbolDef> =
        symbols.iter().map(|d| (d.id.as_str(), d)).collect();
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
                if let Some(def) = defs.get(s.symbol_id.as_str()) {
                    render_symbol(&mut out, s, def);
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
                text_el(&mut out, l.at.x, l.at.y - 1.0, 2.5, "#000", "start", &l.name);
            }
            Entity::Text(t) => {
                text_el(&mut out, t.at.x, t.at.y, t.height, "#000", "start", &t.text);
            }
            // ハーネス境界は背面のrender_harnessesで描き済み
            Entity::Harness(_) => {}
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
}
