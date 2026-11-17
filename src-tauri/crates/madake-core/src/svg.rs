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
/// 配線を太くしたシートで、分岐点の丸が線に埋もれないための倍率 (半径 ÷ 配線の太さ)。
pub const JUNCTION_DOT_PER_WEIGHT: f64 = 0.9;

/// シートの配線の分岐点の丸の半径 (mm)。既定の太さでは [`JUNCTION_DOT_R`]、太い配線では太さの0.9倍。
pub fn junction_dot_r(sheet: &Sheet) -> f64 {
    JUNCTION_DOT_R.max(JUNCTION_DOT_PER_WEIGHT * sheet.wire_weight_mm())
}
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

/// 線色名→鮮やかな表示色 (純色)。[`crate::model::Sheet::vivid_wire_colors`] のシートで配線に使う。
/// 知らない色名と `#` 始まりは [`color_hex`] と同じ。白は紙に見えるよう灰色のまま。
pub fn color_hex_vivid(color: &str) -> &str {
    match color {
        "red" | "赤" => "#ff0000",
        "blue" | "青" => "#0000ff",
        "yellow" | "黄" => "#ffff00",
        "green" | "緑" => "#00ff00",
        "orange" | "橙" => "#ff8000",
        "purple" | "紫" => "#8000ff",
        "brown" | "茶" => "#800000",
        "pink" | "桃" => "#ff00ff",
        "light_blue" | "sky" | "水" | "水色" => "#00ffff",
        other => color_hex(other),
    }
}

/// シートの設定に合わせた配線の表示色。
pub fn wire_color_hex<'a>(sheet: &Sheet, color: &'a str) -> &'a str {
    if sheet.vivid_wire_colors { color_hex_vivid(color) } else { color_hex(color) }
}

/// 破線の配線の線と空きの長さ (mm)。
pub const WIRE_DASH: (f64, f64) = (4.0, 2.5);

/// いま有効なストロークフォント (プロジェクトの書体)。[`text_font_scope`] の範囲だけ有効。
#[derive(Clone, Copy)]
struct ActiveFont {
    font: &'static crate::stroke_font::StrokeFont,
    latin: crate::model::Latin,
    /// 文字のペンの太さ (mm)。あれば大きさによらずこの太さ (太字は1.5倍)。
    pen: Option<f64>,
    /// 字間の倍率 (書体の字間 × 注記などの字間)。セルの字間にはさらにこれを掛ける。
    spacing: f64,
    /// 字の横の伸縮率 (書体 × 注記)。字形の x と送りに掛ける。
    stretch: f64,
}

/// ストロークフォントが有効な間だけ、字間にさらに倍率を掛ける (注記の字間)。`None`・1・範囲外ならそのまま。
fn spacing_scope(spacing: Option<f64>) -> TextFontScope {
    note_text_scope(spacing, None)
}

/// ストロークフォントが有効な間だけ、注記の字間と横の伸縮率を書体の値に掛ける。`None`・範囲外はそのまま。
fn note_text_scope(spacing: Option<f64>, stretch: Option<f64>) -> TextFontScope {
    let previous = TEXT_FONT.with(|c| c.get());
    if let Some(active) = previous {
        let k = spacing.filter(|v| crate::model::is_text_spacing(*v)).unwrap_or(1.0);
        let st = stretch.filter(|v| crate::model::is_text_spacing(*v)).unwrap_or(1.0);
        TEXT_FONT.with(|c| c.set(Some(ActiveFont { spacing: active.spacing * k, stretch: active.stretch * st, ..active })));
    }
    TextFontScope { previous }
}

/// いま有効な書体での文字の線の太さ (mm)。ペンの太さが決まっていればそれ (太字は1.5倍)、無ければ大きさに比例。
fn text_stroke_width(size: f64, bold: bool) -> f64 {
    match TEXT_FONT.with(|c| c.get()).and_then(|f| f.pen) {
        Some(pen) => pen * if bold { crate::stroke_font::BOLD_FACTOR } else { 1.0 },
        None => crate::stroke_font::stroke_width(size, bold),
    }
}

thread_local! {
    static TEXT_FONT: std::cell::Cell<Option<ActiveFont>> = const { std::cell::Cell::new(None) };
}

/// 書体の有効範囲。落とすと (早期returnでも) 前の状態へ戻す。
pub struct TextFontScope {
    previous: Option<ActiveFont>,
}

impl Drop for TextFontScope {
    fn drop(&mut self) {
        TEXT_FONT.with(|c| c.set(self.previous));
    }
}

/// プロジェクトの書体を、戻り値を持っている間だけ有効にする。`None`や知らないidはシステムの書体。
pub fn text_font_scope(text_font: Option<&crate::model::TextFont>) -> TextFontScope {
    let next = text_font.and_then(|tf| crate::stroke_font::font(&tf.id).map(|font| ActiveFont { font, latin: tf.latin, pen: tf.pen(), spacing: tf.spacing(), stretch: tf.stretch() }));
    TextFontScope { previous: TEXT_FONT.with(|c| c.replace(next)) }
}

/// ストロークフォントが有効な間だけ、英数字の字 (半角・全角) を上書きする。`None`ならそのまま。
///
/// 図枠の文字には書体の横の伸縮率を掛けない (図枠は自分のセルの大きさと字間で幅を決めているため)。
fn latin_scope(latin: Option<crate::model::Latin>) -> TextFontScope {
    let previous = TEXT_FONT.with(|c| c.get());
    if let Some(active) = previous {
        TEXT_FONT.with(|c| c.set(Some(ActiveFont { latin: latin.unwrap_or(active.latin), stretch: 1.0, ..active })));
    }
    TextFontScope { previous }
}

/// 文字列の幅 (mm)。ストロークフォントが有効でその字が全てあれば線の幅、そうでなければ見積もり
/// ([`crate::tidy::text_width`]。システムの書体では字間は効かない)。
pub(crate) fn text_advance(s: &str, size: f64, spacing: f64) -> f64 {
    TEXT_FONT
        .with(|c| c.get())
        .and_then(|f| f.font.layout(s, size, f.latin, spacing * f.spacing).map(|t| t.width * f.stretch))
        .unwrap_or_else(|| crate::tidy::text_width(s, size))
}

/// ストロークフォントが有効なら文字列を線の`<path>`で書いて true を返す。空の文字列は何も書かない。
/// 書体が無い・フォントに無い字を含むときは何も書かずに false (呼び出し側が`<text>`を書く)。
fn stroke_text_el(out: &mut String, x: f64, y: f64, size: f64, fill: &str, anchor: &str, bold: bool, spacing: f64, s: &str) -> bool {
    let Some(active) = TEXT_FONT.with(|c| c.get()) else { return false };
    let Some(t) = active.font.layout(s, size, active.latin, spacing * active.spacing) else { return false };
    if t.polylines.is_empty() {
        return true;
    }
    // 横の伸縮率は字形の x と幅の両方に掛ける (基準点は動かない)
    let st = active.stretch;
    let width = t.width * st;
    let dx = match anchor {
        "middle" => -width / 2.0,
        "end" => -width,
        _ => 0.0,
    };
    let mut d = String::new();
    for line in &t.polylines {
        for (i, (px, py)) in line.iter().enumerate() {
            let _ = write!(d, "{}{} {} ", if i == 0 { "M" } else { "L" }, n(x + dx + px * st), n(y + py));
        }
    }
    let _ = writeln!(
        out,
        "<path d=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",
        d.trim_end(),
        fill,
        n(text_stroke_width(size, bold))
    );
    true
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
    if stroke_text_el(out, x, y, size, fill, anchor, false, 1.0, s) {
        return;
    }
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
    text_cell_el(out, x, y, size, fill, anchor, weight, 1.0, s);
}

/// 表題欄のセル1行。`spacing`はストロークフォントのときだけ効く字間。
#[allow(clippy::too_many_arguments)]
fn text_cell_el(
    out: &mut String,
    x: f64,
    y: f64,
    size: f64,
    fill: &str,
    anchor: &str,
    weight: Weight,
    spacing: f64,
    s: &str,
) {
    if stroke_text_el(out, x, y, size, fill, anchor, weight == Weight::Bold, spacing, s) {
        return;
    }
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

/// 裁断マーク・中心マークの線幅 (mm)。
pub(crate) const MARK_STROKE: f64 = 0.5;
/// 中心マークの長さの上限 (mm)。裁断線まで伸ばす (`centre_to_trim`) ときは使わない。
pub(crate) const CENTRE_MARK_MAX: f64 = 10.0;
/// 中心マークの矢印 (開いたV字) の長さと、片側の幅 (mm)。
pub(crate) const CENTRE_ARROW_LEN: f64 = 1.9;
pub(crate) const CENTRE_ARROW_HALF: f64 = 0.5;

/// 裁断マーク (裁断線の四隅のL字) と中心マーク (外枠の各辺の中央から外向き)。
fn frame_marks(out: &mut String, t: &FrameTemplate, pw: f64, ph: f64) {
    let ins = t.marks.trim_insets();
    if t.marks.trim {
        let arm = t.marks.trim_arm();
        for (cx, cy, sx, sy) in [
            (ins.left, ins.top, 1.0, 1.0),
            (pw - ins.right, ins.top, -1.0, 1.0),
            (ins.left, ph - ins.bottom, 1.0, -1.0),
            (pw - ins.right, ph - ins.bottom, -1.0, -1.0),
        ] {
            line_el(out, cx, cy, cx + sx * arm, cy, MARK_STROKE, "#000");
            line_el(out, cx, cy, cx, cy + sy * arm, MARK_STROKE, "#000");
        }
    }
    if t.marks.centre {
        let (x0, y0, x1, y1) = t.frame_rect(pw, ph);
        let m = t.margins();
        let (mx, my) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let stroke = t.marks.centre_weight.map(|w| w.stroke()).unwrap_or(MARK_STROKE);
        // 辺ごとに: 外枠上の始点、外向きの単位ベクトル、余白、裁断線までの距離の差し引き分
        for (sx, sy, dx, dy, margin, inset) in [
            (mx, y0, 0.0, -1.0, m.top, ins.top),
            (mx, y1, 0.0, 1.0, m.bottom, ins.bottom),
            (x0, my, -1.0, 0.0, m.left, ins.left),
            (x1, my, 1.0, 0.0, m.right, ins.right),
        ] {
            let len = if t.marks.centre_to_trim { margin - inset } else { margin.min(CENTRE_MARK_MAX) };
            if len <= 0.0 {
                continue;
            }
            let (ex, ey) = (sx + dx * len, sy + dy * len);
            line_el(out, sx, sy, ex, ey, stroke, "#000");
            if t.marks.centre_arrow {
                // 外側の端を先端にした開いたV字。(px, py) は進行方向に直角な単位ベクトル
                let (bx, by) = (ex - dx * CENTRE_ARROW_LEN, ey - dy * CENTRE_ARROW_LEN);
                let (px, py) = (-dy, dx);
                let _ = writeln!(
                    out,
                    "<polyline points=\"{},{} {},{} {},{}\" fill=\"none\" stroke=\"#000\" stroke-width=\"{}\"/>",
                    n(bx + px * CENTRE_ARROW_HALF), n(by + py * CENTRE_ARROW_HALF),
                    n(ex), n(ey),
                    n(bx - px * CENTRE_ARROW_HALF), n(by - py * CENTRE_ARROW_HALF),
                    n(stroke)
                );
            }
        }
    }
}

/// 枠付きセル (`boxed`) の、文字の箱から枠までの距離 (mm)。
pub(crate) const BOX_GAP: f64 = 0.8;
/// ベースラインより下に出る分 (ディセンダ) ÷ フォントサイズ。枠の下端の見積もりに使う。
const DESCENT_RATIO: f64 = 0.25;

/// `max_w`に収まるフォントサイズ。収まっていれば`font`のまま (大きくはしない)。
pub(crate) fn fitted_font(lines: &[&str], font: f64, max_w: Option<f64>) -> f64 {
    fitted_font_spaced(lines, font, max_w, 1.0)
}

/// [`fitted_font`] の字間つき (幅はストロークフォントが有効ならその線の幅)。
pub(crate) fn fitted_font_spaced(lines: &[&str], font: f64, max_w: Option<f64>, spacing: f64) -> f64 {
    let Some(max_w) = max_w.filter(|w| *w > 0.0) else { return font };
    let widest = lines.iter().map(|l| text_advance(l, font, spacing)).fold(0.0, f64::max);
    if widest <= max_w { font } else { font * max_w / widest }
}

/// 表題欄のセル1つ。新しい項目を使わないセルは従来の1要素をそのまま出す。
fn frame_cell(out: &mut String, t: &FrameTemplate, cell: &crate::frame_template::Cell, value: &str, x: f64, y: f64) {
    let lines: Vec<&str> = value.split('\n').collect();
    let base = t.font_size(cell.size);
    let spacing = cell.spacing.unwrap_or(1.0);
    let font = if cell.fit { fitted_font_spaced(&lines, base, cell.w, spacing) } else { base };
    let plain = cell.rotation == 0 && !cell.boxed && lines.len() == 1;
    if plain {
        text_cell_el(out, x, y, font, "#000", cell.align.anchor(), cell.weight, spacing, value);
        return;
    }
    if cell.rotation != 0 {
        let _ = writeln!(out, "<g transform=\"rotate({} {} {})\">", cell.rotation, n(x), n(y));
    }
    let pitch = font * crate::frame_template::LINE_PITCH;
    for (i, line) in lines.iter().enumerate() {
        text_cell_el(out, x, y + pitch * i as f64, font, "#000", cell.align.anchor(), cell.weight, spacing, line);
    }
    // 値が空のセルには囲むものが無いので、枠も描かない
    if cell.boxed && !value.is_empty() {
        let w = lines.iter().map(|l| text_advance(l, font, spacing)).fold(0.0, f64::max);
        let left = match cell.align {
            crate::frame_template::Align::Start => x,
            crate::frame_template::Align::Middle => x - w / 2.0,
            crate::frame_template::Align::End => x - w,
        };
        let top = y - font;
        let bottom = y + pitch * (lines.len() as f64 - 1.0) + font * DESCENT_RATIO;
        rect_el(out, left - BOX_GAP, top - BOX_GAP, w + 2.0 * BOX_GAP, bottom - top + 2.0 * BOX_GAP, "none", "#000", RULE_STROKE);
    }
    if cell.rotation != 0 {
        out.push_str("</g>\n");
    }
}

/// 図枠テンプレート ([`crate::frame_template`]) のとおりに図枠・ゾーン目盛・改訂欄・
/// 表題欄・ロゴ枠を描く。表題欄のセルはバインドを解決してから描く。
fn frame_and_title_block(out: &mut String, sheet: &Sheet, t: &FrameTemplate, ctx: &BindContext) {
    // 図枠の文字は、テンプレートが英数字の字 (半角・全角) を決めていればそれに従う
    let _latin = latin_scope(t.text_latin);
    let (pw, ph) = sheet.paper_mm();
    let (x0, y0, x1, y1) = t.frame_rect(pw, ph);
    rect_el(out, x0, y0, x1 - x0, y1 - y0, "none", "#000", FRAME_STROKE);
    if t.zones.show {
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
    }
    frame_marks(out, t, pw, ph);
    // 表題欄 (テンプレートの基準隅・寸法・セル)
    let def = &t.title_block;
    let (tx, ty) = t.title_block_origin(pw, ph);
    revision_block(out, sheet, t, tx, ty);
    rect_el(out, tx, ty, def.w, def.h, "#fff", "#000", FRAME_STROKE);
    for g in &def.grid_lines {
        line_el(out, tx + g.x1, ty + g.y1, tx + g.x2, ty + g.y2, g.weight.stroke(), "#000");
    }
    for cell in &def.cells {
        let value = cell.resolve(ctx);
        if cell.omit_if_empty && value.is_empty() {
            continue;
        }
        frame_cell(out, t, cell, &value, tx + cell.x, ty + cell.y);
    }
    // ロゴ枠 (画像を読み込めていれば画像、無ければ枠だけで置き場所を示す)
    if let Some(logo) = &t.logo {
        match &logo.data_url {
            Some(url) => {
                let _ = write!(
                    out,
                    "<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"xMidYMid meet\" href=\"{}\"/>\n",
                    n(logo.x), n(logo.y), n(logo.w), n(logo.h), xml_escape(url)
                );
            }
            None => rect_el(out, logo.x, logo.y, logo.w, logo.h, "none", "#000", RULE_STROKE),
        }
    }
}

/// 改訂欄 (ISO 7200 / JIS Z 8311)。表題欄の直上に同じ右端・同じ幅で描き、
/// 最下段を列見出し、その上に古い改訂から順に積む (最新が最上段)。0行なら何も描かない。
/// 行高・列幅・最大行数は図枠テンプレートが決める。
fn revision_block(out: &mut String, sheet: &Sheet, t: &FrameTemplate, tx: f64, ty: f64) {
    if t.uses_extended_revision_table() {
        return revision_table(out, sheet, t, tx, ty);
    }
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

/// 記号の三角の中の文字: ベースラインを底辺からどれだけ上げるか ÷ 三角の高さ、文字高の上限 ÷ 三角の高さ。
pub(crate) const MARK_TEXT_LIFT: f64 = 0.12;
pub(crate) const MARK_TEXT_MAX: f64 = 0.55;
/// 同じセルに続けて出す項目 (`suffix`) と、その前のものとの間隔 (mm)。
pub(crate) const SUFFIX_GAP: f64 = 1.0;
/// 記号の列で、`suffix`の文字の右に残す余白 (mm)。
const SUFFIX_RIGHT_PAD: f64 = 0.5;

/// 改訂記号を囲む三角 (頂点が上)。`gap`が0なら閉じた多角形、0より大きければ斜辺の中ほどを空け、
/// 底辺と合わせて線で描く。
fn mark_triangle(out: &mut String, cx: f64, by: f64, th: f64, hb: f64, gap: f64) {
    if gap <= 0.0 {
        let _ = writeln!(
            out,
            "<polygon points=\"{},{} {},{} {},{}\" fill=\"none\" stroke=\"#000\" stroke-width=\"{}\"/>",
            n(cx), n(by - th), n(cx + hb), n(by), n(cx - hb), n(by), n(RULE_STROKE)
        );
        return;
    }
    let (a, b) = ((1.0 - gap) / 2.0, (1.0 + gap) / 2.0);
    for side in [1.0, -1.0] {
        // 頂点 (cx, by - th) から底辺の角 (cx ± hb, by) へ。割合 0〜a と b〜1 の部分だけを描く
        let at = |f: f64| (cx + side * hb * f, by - th + th * f);
        for (f0, f1) in [(0.0, a), (b, 1.0)] {
            let ((x1, y1), (x2, y2)) = (at(f0), at(f1));
            line_el(out, x1, y1, x2, y2, RULE_STROKE, "#000");
        }
    }
    line_el(out, cx - hb, by, cx + hb, by, RULE_STROKE, "#000");
}

/// 改訂欄 (列・見出し・行高・固定行数・記号の形をテンプレートが決める版)。
fn revision_table(out: &mut String, sheet: &Sheet, t: &FrameTemplate, tx: f64, ty: f64) {
    use crate::frame_template::{Align, MarkShape, RevisionColumn, RevisionField, LINE_PITCH};
    let rb = &t.revision_block;
    let rows = visible_revisions_max(&sheet.revisions, rb.rows_fixed.unwrap_or(rb.rows_max));
    let n_rows = rb.rows_fixed.unwrap_or(rows.len());
    if n_rows == 0 {
        return;
    }
    let cols = t.revision_columns();
    let row_h = t.revision_row_h();
    let width = t.title_block.w;
    let h = (n_rows as f64 + 1.0) * row_h;
    let top = ty - h;
    rect_el(out, tx, top, width, h, "#fff", "#000", FRAME_STROKE);
    for r in 1..=n_rows {
        let ry = top + row_h * r as f64;
        line_el(out, tx, ry, tx + width, ry, RULE_STROKE, "#000");
    }
    let mut lefts = Vec::with_capacity(cols.len());
    let mut x = tx;
    for c in &cols {
        lefts.push(x);
        x += c.w;
    }
    for left in lefts.iter().skip(1) {
        line_el(out, *left, top, *left, ty, RULE_STROKE, "#000");
    }
    // 1セル分の文字 (収めてから、行の中央へ縦に並べる)
    fn cell_text(out: &mut String, row_h: f64, row_top: f64, left: f64, c: &RevisionColumn, align: Align, base: f64, value: &str) {
        let lines: Vec<&str> = value.split('\n').collect();
        let mut font = fitted_font(&lines, base, Some(c.w - 2.0 * CELL_PAD));
        let m = lines.len() as f64;
        if m * font * LINE_PITCH > row_h {
            font *= row_h / (m * font * LINE_PITCH);
        }
        let pitch = font * LINE_PITCH;
        let first = row_top + row_h / 2.0 + font / 2.0 - (m - 1.0) * pitch / 2.0;
        let x = match align {
            Align::Start => left + CELL_PAD,
            Align::Middle => left + c.w / 2.0,
            Align::End => left + c.w - CELL_PAD,
        };
        for (k, line) in lines.iter().enumerate() {
            text_el(out, x, first + pitch * k as f64, font, "#000", align.anchor(), line);
        }
    }
    let head_top = ty - row_h;
    for (c, left) in cols.iter().zip(&lefts) {
        if !c.header.is_empty() {
            cell_text(out, row_h, head_top, *left, c, c.header_align.unwrap_or(c.align), t.font_size(rb.header_size.unwrap_or(REV_HEADER_FONT)), &c.header);
        }
    }
    let body = t.font_size(rb.text_size.unwrap_or(REV_FONT));
    // 記号の三角: 高さ・半分の底辺・中心X (列の左端から`pad`の位置が左下の角)
    let style = rb.mark_style.unwrap_or_default();
    let triangle = rb.mark_shape == MarkShape::Triangle;
    let th = style.h * row_h;
    let hb = th * style.aspect / 2.0;
    let mark_col = cols.iter().zip(&lefts).find(|(c, _)| c.field == RevisionField::Mark);
    for (i, rev) in rows.iter().enumerate() {
        let row_top = ty - (i as f64 + 2.0) * row_h;
        for (c, left) in cols.iter().zip(&lefts) {
            let value = c.field.value(rev);
            let suffix = c.suffix.map(|f| f.value(rev)).unwrap_or("");
            if c.field == RevisionField::Mark && triangle {
                if value.is_empty() {
                    continue;
                }
                let cx = left + style.pad + hb;
                let by = row_top + row_h / 2.0 + th / 2.0;
                mark_triangle(out, cx, by, th, hb, style.gap);
                // 記号は三角の中に収める (三角の底辺の半分より広い文字は縮める)
                let font = fitted_font(&[value], body.min(th * MARK_TEXT_MAX), Some(hb));
                text_el(out, cx, by - th * MARK_TEXT_LIFT, font, "#000", "middle", value);
                if !suffix.is_empty() {
                    // 三角の右の角の横に、ふつうのセルと同じ高さで続ける
                    let sx = cx + hb + SUFFIX_GAP;
                    let font = fitted_font(&[suffix], body, Some(left + c.w - SUFFIX_RIGHT_PAD - sx));
                    text_el(out, sx, row_top + row_h / 2.0 + font / 2.0, font, "#000", "start", suffix);
                }
                continue;
            }
            // ふつうの列: 主の文字のあとに、空白1つを挟んで続ける
            let joined;
            let value = if suffix.is_empty() {
                value
            } else if value.is_empty() {
                suffix
            } else {
                joined = format!("{value} {suffix}");
                &joined
            };
            if value.is_empty() {
                continue;
            }
            cell_text(out, row_h, row_top, *left, c, c.align, body, value);
        }
    }
    // 改訂の無い行にも三角だけを描く (刷り込みの記号枠)
    if triangle && style.in_empty_rows {
        if let Some((_, left)) = mark_col {
            for r in rows.len()..n_rows {
                let row_top = ty - (r as f64 + 2.0) * row_h;
                mark_triangle(out, left + style.pad + hb, row_top + row_h / 2.0 + th / 2.0, th, hb, style.gap);
            }
        }
    }
}

/// 配置後のシンボル外形の上端Y(用紙座標)。参照記号・型番の重なり回避用。
pub(crate) fn symbol_top_y(inst: &crate::model::SymbolInstance, def: &SymbolDef) -> f64 {
    caption_bounds(inst, def).0.y
}

/// 参照記号・型番の置き場所: 文字の寄せ (`end` = 右揃え) の基準X、`top - REF_LABEL_DY` /
/// `top - VALUE_LABEL_DY` が各行のベースラインになる基準Y。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CaptionAnchor {
    pub x: f64,
    pub top: f64,
    pub end: bool,
}

/// 参照記号・型番の置き場所を決める。
///
/// 普通は配置基準点の真上に中央揃え (`symbol_top_y` の上)。ただしブロックの描かれた上辺に
/// ピンがあるときは、そこから出る配線が中央の文字を横切るので、外形の**左上の角の外**
/// (左辺の [`CORNER_CAPTION_GAP`] 左・上辺の上) に右揃えで置く。ブロックが描く他のもの
/// (他の辺のリード・番号・コネクタの絵・まとまりの名前) に文字が重なるときは、その上まで上げる。
///
/// 中央揃えのとき、ブロックは描かれた外形 ([`crate::block::block_outline`]) の中央 (回転・ミラーの後) にそろえる。
/// 2.5mm間隔で幅が2.5mmの奇数倍のブロックは外形が配置点から1.25mmずれるため。5mm間隔のブロックはずれが0なので配置点のまま。
pub(crate) fn caption_anchor(inst: &crate::model::SymbolInstance, def: &SymbolDef) -> CaptionAnchor {
    if let Some(inside) = inside_caption_anchor(inst, def) {
        return inside;
    }
    top_left_corner_anchor(inst, def).unwrap_or_else(|| CaptionAnchor {
        x: centred_caption_x(inst, def),
        top: symbol_top_y(inst, def),
        end: false,
    })
}

/// 大文字の高さ ÷ 文字サイズ (参照記号・型番の行を外形の中央へそろえる計算に使う)。
pub(crate) const CAPTION_CAP_RATIO: f64 = 0.72;

/// `block_caption: inside` のブロックの置き場所: 参照記号・型番を、描かれた外形 (回転・ミラーの後) の中央に置く。
/// 2行あれば2行のまとまりの中心を、1行だけならその行の大文字の中心を、外形の中心の高さにそろえる。
/// ブロックでない・属性が無い・読めないときは None (これまでの置き場所)。
fn inside_caption_anchor(inst: &crate::model::SymbolInstance, def: &SymbolDef) -> Option<CaptionAnchor> {
    if !crate::block::is_block_export_id(&def.id) {
        return None;
    }
    let spec = crate::block::parse_block_spec(&inst.attrs).ok()?;
    if spec.caption != crate::block::BlockCaption::Inside {
        return None;
    }
    let (lo, hi) = crate::block::block_outline(&spec);
    let c = transform_local(Point::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0), inst);
    let cap = CAPTION_CAP_RATIO * LABEL_FONT;
    let has_ref = inst.shows_reference();
    let has_value = !inst.value.is_empty();
    // 各行のベースラインは top - REF_LABEL_DY / top - VALUE_LABEL_DY
    let top = match (has_ref, has_value) {
        (true, false) => c.y + cap / 2.0 + REF_LABEL_DY,
        (false, _) => c.y + cap / 2.0 + VALUE_LABEL_DY,
        (true, true) => c.y + (REF_LABEL_DY + cap + VALUE_LABEL_DY) / 2.0,
    };
    Some(CaptionAnchor { x: c.x, top, end: false })
}

/// 中央揃えの参照記号・型番の基準X。普通は配置点、ブロックは外形の中心 (ローカルの中心を変換するので、ずれが0なら配置点ちょうど)。
fn centred_caption_x(inst: &crate::model::SymbolInstance, def: &SymbolDef) -> f64 {
    if !crate::block::is_block_export_id(&def.id) {
        return inst.at.x;
    }
    let Ok(spec) = crate::block::parse_block_spec(&inst.attrs) else {
        return inst.at.x;
    };
    let (lo, hi) = crate::block::block_outline(&spec);
    transform_local(Point::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0), inst).x
}

/// 外形の左辺と右揃えの文字の右端との間隔 (mm)。上辺との間隔は型番のベースライン
/// ([`VALUE_LABEL_DY`]) と同じ 1.0 mm。
pub(crate) const CORNER_CAPTION_GAP: f64 = 1.0;

fn top_left_corner_anchor(inst: &crate::model::SymbolInstance, def: &SymbolDef) -> Option<CaptionAnchor> {
    use crate::tidy::Rect;
    if !crate::block::is_block_export_id(&def.id) {
        return None;
    }
    let spec = crate::block::parse_block_spec(&inst.attrs).ok()?;
    let (lo, hi) = crate::block::block_outline(&spec);
    let a = transform_local(lo, inst);
    let b = transform_local(hi, inst);
    let body = Rect::from_corners(a, b);
    let eps = 1e-6;
    let has_top_pin = def.pins.iter().any(|pin| {
        let t = transform_local(pin.at, inst);
        t.y < body.min.y - eps && t.x > body.min.x - eps && t.x < body.max.x + eps
    });
    if !has_top_pin {
        return None;
    }
    let x = body.min.x - CORNER_CAPTION_GAP;
    // 描かれるもの (図形の外接矩形・文字の箱全体・ピングループの括弧)
    let mut drawn: Vec<Rect> = Vec::new();
    for prim in &def.primitives {
        match prim {
            Primitive::Text { at, text, height, .. } => {
                let c = transform_local(*at, inst);
                let half = crate::tidy::text_width(text, *height) / 2.0;
                drawn.push(Rect::from_corners(
                    Point::new(c.x - half, c.y - height / 2.0),
                    Point::new(c.x + half, c.y + height / 2.0),
                ));
            }
            other => {
                let (lo, hi) = crate::connector::primitive_extent(other);
                drawn.push(Rect::from_corners(transform_local(lo, inst), transform_local(hi, inst)));
            }
        }
    }
    for (lo, hi) in crate::pin_groups::pin_group_paper_boxes(inst, def) {
        drawn.push(Rect::from_corners(lo, hi));
    }
    // 参照記号 (先) と型番の文字の箱 (広いほうの幅)。非表示・空の行は幅に入れない
    let mut width: f64 = 0.0;
    if inst.shows_reference() {
        width = width.max(crate::tidy::text_width(&inst.reference, LABEL_FONT));
    }
    if !inst.value.is_empty() {
        width = width.max(crate::tidy::text_width(&inst.value, LABEL_FONT));
    }
    // 上辺の 1.0 mm 上から始め、重なるものの上端が見つかるたびにその上まで上げる
    let mut top = body.min.y;
    for _ in 0..=drawn.len() {
        let caption = Rect::from_corners(
            Point::new(x - width, top - REF_LABEL_DY - LABEL_FONT),
            Point::new(x, top - VALUE_LABEL_DY),
        );
        match drawn.iter().filter(|r| caption.overlaps(r)).map(|r| r.min.y).reduce(f64::min) {
            // 型番のベースラインは top - VALUE_LABEL_DY (= 1.0 mm) なので、top = y で文字の箱の下端が y の 1.0 mm 上
            Some(y) => top = top.min(y),
            None => break,
        }
    }
    Some(CaptionAnchor { x, top, end: true })
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

fn render_symbol(out: &mut String, inst: &crate::model::SymbolInstance, def: &SymbolDef, weight: f64) {
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
                    n(weight)
                );
            }
            Primitive::Circle { center, r, filled } => {
                let c = transform_local(*center, inst);
                let _ = write!(
                    out,
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{}\" stroke=\"#000\" stroke-width=\"{}\"/>\n",
                    n(c.x), n(c.y), n(*r),
                    if *filled { "#000" } else { "none" },
                    n(weight)
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
                    n(sx), n(sy), n(*r), n(*r), large, n(ex), n(ey), n(weight)
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
                    n(weight)
                );
            }
            Primitive::Text { at, text, height, bold } => {
                // 文字は常に水平に書くので、ベースラインへのずらし (文字高の半分) は
                // シンボルと一緒に回さず、用紙の座標系で下へ取る
                let p = transform_local(*at, inst);
                let label = terminal_rename
                    .and_then(|m| m.get(text.as_str()))
                    .map(String::as_str)
                    .unwrap_or(text.as_str());
                if *bold {
                    text_weight_el(out, p.x, p.y + height / 2.0, *height, "#000", "middle", Weight::Bold, label);
                } else {
                    text_el(out, p.x, p.y + height / 2.0, *height, "#000", "middle", label);
                }
            }
        }
    }
    // 参照記号と型番/値をシンボル外形の上に併記(縦長シンボルでも重ならない)。
    // 上辺にピンのあるブロックは外形の左上の角 (右揃え) へ逃がす
    let caption = caption_anchor(inst, def);
    let anchor = if caption.end { "end" } else { "middle" };
    // 部品ごとの置き場所 (属性 caption_layout) があれば、その行はそこへ書く。無い行はこれまでの場所
    let layout = crate::caption::layout_of(&inst.attrs).unwrap_or_default();
    let mut line = |custom: Option<crate::caption::CaptionLine>, default_y: f64, text: &str| match custom {
        Some(l) if l.hide => {}
        Some(l) => text_el(
            out,
            inst.at.x + l.x,
            inst.at.y + l.y,
            l.h.unwrap_or(LABEL_FONT),
            "#000",
            crate::text_style::svg_anchor(l.anchor),
            text,
        ),
        None => text_el(out, caption.x, default_y, LABEL_FONT, "#000", anchor, text),
    };
    if inst.shows_reference() {
        line(layout.reference, caption.top - REF_LABEL_DY, &inst.reference);
    }
    if !inst.value.is_empty() {
        line(layout.value, caption.top - VALUE_LABEL_DY, &inst.value);
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
    if sheet.hide_wire_numbers {
        return;
    }
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
                let (a, b) = (shape.points[0], arrow_base(shape).unwrap_or(shape.points[1]));
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
                    polyline_points(&match arrow_base(shape) {
                        Some(base) => {
                            let mut pts = shape.points.clone();
                            *pts.last_mut().expect("折れ線は2点以上") = base;
                            pts
                        }
                        None => shape.points.clone(),
                    })
                );
            }
        }
        if let Some(head) = crate::shape::arrow_head(shape) {
            let _ = write!(
                out,
                "<polygon points=\"{}\" fill=\"{stroke}\" stroke=\"none\"/>\n",
                polyline_points(&head)
            );
        }
    }
}

/// 矢印のある線の、線を止める点 (三角の付け根の中央)。太い線の角が先端からはみ出さないよう、線はここまで描く。
fn arrow_base(shape: &crate::model::Shape) -> Option<Point> {
    crate::shape::arrow_head(shape).map(|[_, a, b]| Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0))
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
        line_el(out, a.x, a.y, b.x, b.y, sheet.wire_weight_mm(), "#000");
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
    // 図面の文字はプロジェクトの書体で描く (書き出しの間だけ有効)
    let _font = text_font_scope(project.text_font.as_ref());
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
                let dash = if w.dashed {
                    format!(" stroke-dasharray=\"{} {}\"", n(WIRE_DASH.0), n(WIRE_DASH.1))
                } else {
                    String::new()
                };
                let _ = write!(
                    out,
                    "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"{}/>\n",
                    polyline_points(&w.points),
                    wire_color_hex(sheet, &w.color),
                    n(sheet.weight_of(w)),
                    dash
                );
            }
            Entity::Symbol(s) => {
                if let Some(def) = defs.get(s) {
                    if let Some(block) = relay.device_blocks.get(&s.id) {
                        // 実機ブロック表示 (G7): コイル自身の小さな図形の代わりにこれを描く
                        render_device_block(&mut out, s, &def, block);
                    } else {
                        render_symbol(&mut out, s, &def, sheet.symbol_weight_mm());
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
                    n(j.at.x), n(j.at.y), n(junction_dot_r(sheet))
                );
            }
            Entity::NetLabel(l) => {
                // ネットラベルの文字はシートの注記文字高 (G20) に従って伸縮する
                let label_font = LABEL_FONT * annotation_scale;
                // 方向付きタグ (G3)。attrs["tag"]がある時だけ旗の枠で囲む。
                // 脇に添える文字: 相手のいるラベルのXRefテキスト (自分の`tag_dest`はそれより優先され、シート単体の出力でも出る)
                let tagged = crate::net_tag::tag_dir(l).is_some();
                let note: Option<&str> = xrefs
                    .get(&l.id)
                    .map(String::as_str)
                    .or_else(|| if tagged { crate::net_tag::tag_dest(l) } else { None });
                let tag = crate::net_tag::net_tag_with(l, label_font, sheet.xref_style, note);
                match &tag {
                    Some(t) => {
                        let _ = write!(
                            out,
                            "<polygon points=\"{}\" fill=\"none\" stroke=\"#000\" stroke-width=\"{}\"/>\n",
                            polyline_points(&t.points),
                            n(crate::net_tag::TAG_STROKE)
                        );
                        if t.bold {
                            text_weight_el(&mut out, t.text_at.x, t.text_at.y, label_font, "#000", "start", Weight::Bold, &l.name);
                        } else {
                            text_el(&mut out, t.text_at.x, t.text_at.y, label_font, "#000", "start", &l.name);
                        }
                        if let (Some(at), Some(text)) = (t.dest_at, note) {
                            text_el(&mut out, at.x, at.y, t.dest_font, "#000", "start", text);
                        }
                    }
                    None => {
                        let dy = NET_LABEL_DY * annotation_scale;
                        text_el(&mut out, l.at.x, l.at.y - dy, label_font, "#000", "start", &l.name);
                    }
                }
                // シート間クロスリファレンス (IEC 61082-1)。相手がいるラベルだけ脇に添える。
                // 行き先を旗の中に書いたタグには、旗の外へは何も書かない
                if let Some(text) = note.filter(|_| tag.as_ref().is_none_or(|t| t.dest_at.is_none() && !t.bold)) {
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
                // 注記の装飾 (G4): 基準位置・色トークン・太字。既定の注記は従来と同じ出力。
                // 回転 (90・180・270、時計回り) は基準点を中心に回す。0 のときは何も足さない
                let _note_spacing = note_text_scope(t.spacing, t.stretch);
                let turned = matches!(t.rotation % 360, 90 | 180 | 270);
                if turned {
                    let _ = write!(
                        out,
                        "<g transform=\"rotate({} {} {})\">\n",
                        t.rotation % 360,
                        n(t.at.x),
                        n(t.at.y)
                    );
                }
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
                if turned {
                    out.push_str("</g>\n");
                }
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
            dashed: false, weight_mm: None,
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

    fn a0_sheet() -> Sheet {
        let mut s = golden_sheet();
        s.size = PaperSize::A0;
        s.revisions.clear();
        s
    }

    fn frame_with(edit: impl Fn(&mut FrameTemplate)) -> FrameTemplate {
        let mut t = crate::frame_template::default_template().clone();
        edit(&mut t);
        t
    }

    fn six_columns() -> FrameTemplate {
        use crate::frame_template::{Align, RevisionColumn, RevisionField as F};
        frame_with(|t| {
            // jis-a3: 表題欄の幅120、左上 (290, 255)
            t.revision_block.columns = Some(vec![
                RevisionColumn { field: F::Mark, header: "SYMB".into(), w: 14.0, align: Align::Start, suffix: None, header_align: None },
                RevisionColumn { field: F::Description, header: "CHANGELOG".into(), w: 56.0, align: Align::Start, suffix: None, header_align: None },
                RevisionColumn { field: F::RevNo, header: "ECN NO".into(), w: 16.0, align: Align::Middle, suffix: None, header_align: None },
                RevisionColumn { field: F::Date, header: "DATE".into(), w: 12.0, align: Align::Middle, suffix: None, header_align: None },
                RevisionColumn { field: F::By, header: "BY".into(), w: 11.0, align: Align::Middle, suffix: None, header_align: None },
                RevisionColumn { field: F::Approved, header: "OKAY".into(), w: 11.0, align: Align::End, suffix: None, header_align: None },
            ]);
            t.revision_block.row_h = Some(7.0);
        })
    }

    fn rev6() -> Revision {
        Revision { mark: "1".into(), date: "24/01".into(), description: "変更".into(), by: "KT".into(),
                   rev_no: "E-204".into(), approved: "MU".into(), count: "x3".into() }
    }

    const TXT_TAIL: &str = "fill=\"#000\" text-anchor=\"start\" font-family=\"sans-serif\">";

    /// A revision table with its own columns prints the template's header words and each revision field in its column, with the row height the table states.
    /// 独自の列を持つ改訂欄は、テンプレートの見出し語と、各列に対応する改訂の項目を、改訂欄が指定する行高で印字する。
    #[test]
    fn a_configured_revision_table_prints_its_own_columns() {
        let mut sheet = golden_sheet();
        sheet.revisions = vec![rev6()];
        let svg = sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &six_columns(), &[]);
        // 1行 + 見出し = 高さ14、上端 255 - 14 = 241
        assert!(svg.contains("<rect x=\"290\" y=\"241\" width=\"120\" height=\"14\" fill=\"#fff\""), "{svg}");
        // 見出し行 (上端 248): 基線 248 + 3.5 + 2.2/2 = 252.6、見出しは縮められない (2.2)
        for (x, anchor, word) in [
            ("292", "start", "SYMB"), ("306", "start", "CHANGELOG"), ("368", "middle", "ECN NO"),
            ("382", "middle", "DATE"), ("393.500", "middle", "BY"), ("408", "end", "OKAY"),
        ] {
            let el = format!("<text x=\"{x}\" y=\"252.600\" font-size=\"2.200\" fill=\"#000\" text-anchor=\"{anchor}\" font-family=\"sans-serif\">{word}</text>");
            assert!(svg.contains(&el), "{el}: {svg}");
        }
        assert!(!svg.contains(">記号</text>"), "{svg}");
        // 改訂行 (上端 241): 基線 241 + 3.5 + 1.25 = 245.75。列の左端は 290, 304, 360, 376, 388, 399
        for (x, anchor, word) in [
            ("292", "start", "1"), ("306", "start", "変更"), ("368", "middle", "E-204"),
            ("382", "middle", "24/01"), ("393.500", "middle", "KT"), ("408", "end", "MU"),
        ] {
            let el = format!("<text x=\"{x}\" y=\"245.750\" font-size=\"2.500\" fill=\"#000\" text-anchor=\"{anchor}\" font-family=\"sans-serif\">{word}</text>");
            assert!(svg.contains(&el), "{el}: {svg}");
        }
        // 数量 (count) は列に無いので出ない
        assert!(!svg.contains(">x3</text>"), "{svg}");
    }

    /// With a fixed number of rows the table is ruled at full height even without any revision, and shows only the newest rows when there are more.
    /// 行数を固定すると、改訂が無くても全行の罫線を描き、改訂のほうが多いときは新しい行だけを表示する。
    #[test]
    fn fixed_rows_are_ruled_even_when_empty_and_show_the_newest() {
        let mut t = six_columns();
        t.revision_block.rows_fixed = Some(3);
        let mut sheet = golden_sheet();
        sheet.revisions.clear();
        let empty = sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &t, &[]);
        // 3行 + 見出し = 28、上端 227
        assert!(empty.contains("<rect x=\"290\" y=\"227\" width=\"120\" height=\"28\" fill=\"#fff\""), "{empty}");
        assert!(empty.contains("<text x=\"292\" y=\"252.600\" font-size=\"2.200\" fill=\"#000\" text-anchor=\"start\" font-family=\"sans-serif\">SYMB</text>"), "{empty}");
        // 罫線: 行の境界 234, 241, 248 (上端の227は外枠)
        for y in ["234", "241", "248"] {
            assert!(empty.contains(&format!("<line x1=\"290\" y1=\"{y}\" x2=\"410\" y2=\"{y}\" stroke=\"#000\" stroke-width=\"0.250\"/>")), "{y}: {empty}");
        }
        sheet.revisions = (1..=5).map(|i| Revision { mark: format!("R{i}"), ..rev6() }).collect();
        let full = sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &t, &[]);
        assert!(full.contains("<rect x=\"290\" y=\"227\" width=\"120\" height=\"28\" fill=\"#fff\""), "{full}");
        // 新しい3件 (R3, R4, R5) が古い順に下から: 基線 245.75 / 238.75 / 231.75
        for (y, mark) in [("245.750", "R3"), ("238.750", "R4"), ("231.750", "R5")] {
            let el = format!("<text x=\"292\" y=\"{y}\" font-size=\"2.500\" {TXT_TAIL}{mark}</text>");
            assert!(full.contains(&el), "{el}: {full}");
        }
        assert!(!full.contains(">R1</text>") && !full.contains(">R2</text>"), "{full}");
    }

    /// When fewer rows are fixed than there are revisions, the table is exactly that many rows plus the header tall and the oldest revisions are left out.
    /// 固定行数が改訂の数より少ないとき、改訂欄の高さは「固定行数+見出し1行」ちょうどになり、古い改訂は表示されない。
    #[test]
    fn fewer_fixed_rows_than_revisions_cut_the_oldest() {
        let mut t = six_columns();
        t.revision_block.rows_fixed = Some(2);
        let mut sheet = golden_sheet();
        sheet.revisions = (1..=4).map(|i| Revision { mark: format!("R{i}"), ..rev6() }).collect();
        let svg = sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &t, &[]);
        // (2 + 1) × 7 = 21、上端 234
        assert!(svg.contains("<rect x=\"290\" y=\"234\" width=\"120\" height=\"21\" fill=\"#fff\""), "{svg}");
        for (y, mark) in [("245.750", "R3"), ("238.750", "R4")] {
            let el = format!("<text x=\"292\" y=\"{y}\" font-size=\"2.500\" {TXT_TAIL}{mark}</text>");
            assert!(svg.contains(&el), "{el}: {svg}");
        }
        assert!(!svg.contains(">R1</text>") && !svg.contains(">R2</text>"), "{svg}");
    }

    /// A description longer than its column is shrunk to the column, and a two-line description is drawn as two lines inside the row.
    /// 列より長い内容は列幅まで縮められ、2行の内容は行の中に2行で描かれる。
    #[test]
    fn long_and_two_line_descriptions_stay_inside_the_cell() {
        let mut sheet = golden_sheet();
        sheet.revisions = vec![Revision { description: "あ".repeat(40), ..rev6() }];
        let svg = sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &six_columns(), &[]);
        // 40全角 × 2.5 = 100mm → 列の内側 56 - 4 = 52mm へ: 2.5 × 52 / 100 = 1.3、基線 241 + 3.5 + 0.65
        let el = format!("<text x=\"306\" y=\"245.150\" font-size=\"1.300\" {TXT_TAIL}{}</text>", "あ".repeat(40));
        assert!(svg.contains(&el), "{el}: {svg}");
        sheet.revisions = vec![Revision { description: "一行目\n二行目".into(), ..rev6() }];
        let svg = sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &six_columns(), &[]);
        // 2行: ピッチ 3.125、2行で 6.25 ≤ 7 なので縮めない。最初の基線 241 + 3.5 + 1.25 - 1.5625
        for (y, line) in [("244.188", "一行目"), ("247.312", "二行目")] {
            let el = format!("<text x=\"306\" y=\"{y}\" font-size=\"2.500\" {TXT_TAIL}{line}</text>");
            assert!(svg.contains(&el), "{el}: {svg}");
        }
    }

    /// A three-line description that would not fit a 7 mm row is reduced by row height / (lines x font x 1.25), so that the three lines fill exactly the row.
    /// 7mmの行に収まらない3行の内容は、行高÷(行数×フォント×1.25)の倍率で縮められ、3行がちょうど行の中に収まる。
    #[test]
    fn a_three_line_description_is_reduced_to_fit_the_row() {
        let mut sheet = golden_sheet();
        sheet.revisions = vec![Revision { description: "a\nb\nc".into(), ..rev6() }];
        let svg = sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &six_columns(), &[]);
        // 3 × 2.5 × 1.25 = 9.375 > 7 → フォント 2.5 × 7 / 9.375 = 1.8667、ピッチ 2.3333
        // 最初の基線 241 + 3.5 + 0.9333 - 2.3333 = 243.1
        for (y, line) in [("243.100", "a"), ("245.433", "b"), ("247.767", "c")] {
            let el = format!("<text x=\"306\" y=\"{y}\" font-size=\"1.867\" {TXT_TAIL}{line}</text>");
            assert!(svg.contains(&el), "{el}: {svg}");
        }
    }

    /// With the triangle mark shape, each revision mark is drawn centred inside an upright triangle.
    /// 記号の形が三角のとき、改訂記号は上向きの三角の中に中央揃えで描かれる。
    #[test]
    fn a_triangle_is_drawn_around_each_revision_mark() {
        let mut t = six_columns();
        t.revision_block.mark_shape = crate::frame_template::MarkShape::Triangle;
        let mut sheet = golden_sheet();
        sheet.revisions = vec![rev6()];
        let svg = sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &t, &[]);
        assert_eq!(svg.matches("<polygon").count() - sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &six_columns(), &[]).matches("<polygon").count(), 1);
        // th = 5.6、hb = 3.233、cx = 290 + 2 + 3.233 = 295.233、底辺 by = 241 + 3.5 + 2.8 = 247.3
        assert!(svg.contains("<polygon points=\"295.233,241.700 298.466,247.300 292,247.300\" fill=\"none\" stroke=\"#000\" stroke-width=\"0.250\"/>"), "{svg}");
        // 記号は cx に中央揃え、基線 247.3 - 5.6 × 0.12 = 246.628
        assert!(svg.contains("<text x=\"295.233\" y=\"246.628\" font-size=\"2.500\" fill=\"#000\" text-anchor=\"middle\" font-family=\"sans-serif\">1</text>"), "{svg}");
    }

    /// A template that only says text sizes are cap heights keeps the classic four columns with their Japanese headers, drawn with every font divided by 0.72.
    /// 文字の大きさを大文字の高さで指定するだけのテンプレートは、従来の4列と日本語の見出しのまま、すべてのフォントを0.72で割って描く。
    #[test]
    fn cap_text_size_alone_draws_the_classic_columns_with_scaled_fonts() {
        let t = frame_with(|t| t.text_size_is = crate::frame_template::TextSizeIs::Cap);
        let mut sheet = golden_sheet();
        sheet.revisions = vec![Revision { mark: "R1".into(), ..rev6() }];
        let svg = sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &t, &[]);
        // 行高 8 (表題欄と同じ)、1行 + 見出し = 16、上端 239。列の左端は 290, 304, 332, 388
        assert!(svg.contains("<rect x=\"290\" y=\"239\" width=\"120\" height=\"16\" fill=\"#fff\""), "{svg}");
        // 見出し (上端 247): 2.2 / 0.72 = 3.0556、基線 247 + 4 + 1.5278
        for (x, word) in [("292", "記号"), ("306", "日付"), ("334", "内容"), ("390", "承認")] {
            let el = format!("<text x=\"{x}\" y=\"252.528\" font-size=\"3.056\" {TXT_TAIL}{word}</text>");
            assert!(svg.contains(&el), "{el}: {svg}");
        }
        // 本文 (上端 239): 2.5 / 0.72 = 3.4722、基線 239 + 4 + 1.7361
        for (x, word) in [("292", "R1"), ("306", "24/01"), ("334", "変更"), ("390", "KT")] {
            let el = format!("<text x=\"{x}\" y=\"244.736\" font-size=\"3.472\" {TXT_TAIL}{word}</text>");
            assert!(svg.contains(&el), "{el}: {svg}");
        }
    }

    /// With margins per side the border is drawn from (left, top) to (paper − right, paper − bottom).
    /// 辺ごとの余白があるとき、外枠は(左, 上)から(用紙−右, 用紙−下)まで描かれる。
    #[test]
    fn margins_per_side_set_the_border_rectangle() {
        let t = frame_with(|t| t.frame_margins = Some(crate::frame_template::FrameMargins { left: 30.0, right: 15.0, top: 12.0, bottom: 10.0 }));
        let svg = sheet_to_svg_with_frame(&a0_sheet(), &builtin_symbols(), &t, &[]);
        assert!(svg.contains("<rect x=\"30\" y=\"12\" width=\"1144\" height=\"819\" fill=\"none\""), "{svg}");
    }

    /// With zone labels hidden, the frame has no zone letters or digits and no zone ticks.
    /// ゾーン目盛を隠すと、図枠にゾーンの英字・数字も目盛線も出ない。
    #[test]
    fn hidden_zones_draw_no_labels_and_no_ticks() {
        let shown = sheet_to_svg_with_frame(&a0_sheet(), &builtin_symbols(), &frame_with(|_| {}), &[]);
        assert!(shown.contains(">A</text>") && shown.contains("stroke-width=\"0.250\""));
        let t = frame_with(|t| { t.zones.show = false; t.title_block.grid_lines.clear(); });
        let svg = frame_only(&sheet_to_svg_with_frame(&a0_sheet(), &builtin_symbols(), &t, &[]));
        assert!(!svg.contains(">A</text>") && !svg.contains(">1</text>"), "{svg}");
        assert!(!svg.contains("stroke-width=\"0.250\""), "{svg}");
    }

    /// Trim marks are an L at each paper corner, 4 mm in from the edge with 15 mm arms: eight lines.
    /// 裁断マークは用紙の四隅のL字で、端から4mm内側、腕15mm。線は8本。
    #[test]
    fn trim_marks_are_an_l_at_each_paper_corner() {
        let t = frame_with(|t| t.marks.trim = true);
        let svg = sheet_to_svg_with_frame(&a0_sheet(), &builtin_symbols(), &t, &[]);
        assert!(svg.contains("<line x1=\"4\" y1=\"4\" x2=\"19\" y2=\"4\" stroke=\"#000\" stroke-width=\"0.500\"/>"), "{svg}");
        assert!(svg.contains("<line x1=\"4\" y1=\"4\" x2=\"4\" y2=\"19\" stroke=\"#000\" stroke-width=\"0.500\"/>"), "{svg}");
        assert!(svg.contains("<line x1=\"1185\" y1=\"837\" x2=\"1170\" y2=\"837\" stroke=\"#000\" stroke-width=\"0.500\"/>"), "{svg}");
        let off = sheet_to_svg_with_frame(&a0_sheet(), &builtin_symbols(), &frame_with(|_| {}), &[]);
        assert_eq!(svg.matches("<line").count() - off.matches("<line").count(), 8);
    }

    /// Centre marks run outwards from the middle of each border side, as long as that side's margin but at most 10 mm.
    /// 中心マークは外枠の各辺の中央から外向きに伸び、長さはその辺の余白(最大10mm)。
    #[test]
    fn centre_marks_run_outwards_from_the_middle_of_each_side() {
        let t = frame_with(|t| {
            t.frame_margins = Some(crate::frame_template::FrameMargins { left: 30.0, right: 15.0, top: 12.0, bottom: 6.0 });
            t.marks.centre = true;
        });
        let svg = sheet_to_svg_with_frame(&a0_sheet(), &builtin_symbols(), &t, &[]);
        assert!(svg.contains("<line x1=\"602\" y1=\"12\" x2=\"602\" y2=\"2\" stroke=\"#000\" stroke-width=\"0.500\"/>"), "{svg}");
        assert!(svg.contains("<line x1=\"602\" y1=\"835\" x2=\"602\" y2=\"841\" stroke=\"#000\" stroke-width=\"0.500\"/>"), "{svg}");
        assert!(svg.contains("<line x1=\"30\" y1=\"423.500\" x2=\"20\" y2=\"423.500\" stroke=\"#000\" stroke-width=\"0.500\"/>"), "{svg}");
    }

    /// A rule marked medium is drawn 0.5 mm wide and one marked heavy 1 mm wide; unmarked rules stay 0.25 mm.
    /// mediumの罫線は0.5mm、heavyは1mmで描かれる。指定の無い罫線は0.25mmのまま。
    #[test]
    fn rule_weights_set_the_stroke_width() {
        let t = frame_with(|t| {
            t.title_block.grid_lines[0].weight = crate::frame_template::LineWeight::Medium;
            t.title_block.grid_lines[1].weight = crate::frame_template::LineWeight::Heavy;
        });
        let svg = sheet_to_svg_with_frame(&golden_sheet(), &builtin_symbols(), &t, &[]);
        assert!(svg.contains("<line x1=\"290\" y1=\"263\" x2=\"410\" y2=\"263\" stroke=\"#000\" stroke-width=\"0.500\"/>"), "{svg}");
        assert!(svg.contains("<line x1=\"290\" y1=\"271\" x2=\"410\" y2=\"271\" stroke=\"#000\" stroke-width=\"1\"/>"), "{svg}");
        assert!(svg.contains("<line x1=\"290\" y1=\"279\" x2=\"410\" y2=\"279\" stroke=\"#000\" stroke-width=\"0.250\"/>"), "{svg}");
    }

    fn one_cell(edit: impl Fn(&mut crate::frame_template::Cell)) -> FrameTemplate {
        frame_with(|t| {
            let mut c = t.title_block.cells[0].clone();
            c.text = Some("ABCDEFGHIJ".into());
            c.bind = None;
            c.x = 10.0; c.y = 20.0; c.size = 3.0; c.w = Some(9.0);
            edit(&mut c);
            t.title_block.cells = vec![c];
            t.title_block.grid_lines.clear();
        })
    }

    fn cell_svg(t: &FrameTemplate) -> String {
        frame_only(&sheet_to_svg_with_frame(&golden_sheet(), &builtin_symbols(), t, &[]))
    }

    /// A plain cell is written exactly as before: one text element at the cell position with the cell size.
    /// ふつうのセルはこれまでと全く同じに書かれる(セルの位置に、セルのサイズで、text要素が1つ)。
    #[test]
    fn a_plain_cell_is_written_as_before() {
        let svg = cell_svg(&one_cell(|_| {}));
        assert!(svg.contains("<text x=\"300\" y=\"275\" font-size=\"3\" fill=\"#000\" text-anchor=\"start\" font-family=\"sans-serif\">ABCDEFGHIJ</text>"), "{svg}");
    }

    /// With capital-height sizes the font is the size divided by 0.72.
    /// 大文字高さの指定では、フォントはサイズを0.72で割った値になる。
    #[test]
    fn a_capital_height_cell_uses_a_larger_font() {
        let mut t = one_cell(|c| c.size = 3.6);
        t.text_size_is = crate::frame_template::TextSizeIs::Cap;
        assert!(cell_svg(&t).contains("font-size=\"5\""));
    }

    /// A fitted cell shrinks its font so that the estimated text width equals the cell width; a text that already fits keeps its size.
    /// fitのセルは、見積もった文字幅がセル幅になるまでフォントを縮める。もともと収まる文字はそのまま。
    #[test]
    fn a_fitted_cell_shrinks_long_text_only() {
        // 10文字 × 0.6 × 3mm = 18mm → 幅9mmへ収めると 1.5mm
        assert!(cell_svg(&one_cell(|c| c.fit = true)).contains("font-size=\"1.500\""));
        assert!(cell_svg(&one_cell(|c| { c.fit = true; c.w = Some(40.0); })).contains("<text x=\"300\" y=\"275\" font-size=\"3\" fill=\"#000\" text-anchor=\"start\" font-family=\"sans-serif\">ABCDEFGHIJ</text>"));
        assert!(cell_svg(&one_cell(|c| { c.fit = true; c.w = None; })).contains("<text x=\"300\" y=\"275\" font-size=\"3\" fill=\"#000\" text-anchor=\"start\" font-family=\"sans-serif\">ABCDEFGHIJ</text>"));
    }

    /// A fitted two-line cell is shrunk by its widest line, and both lines use the shrunk size.
    /// fitの2行セルは、いちばん長い行に合わせて縮み、2行とも縮めたサイズで描かれる。
    #[test]
    fn a_fitted_two_line_cell_shrinks_by_its_widest_line() {
        let svg = cell_svg(&one_cell(|c| { c.fit = true; c.text = Some("ABCDEFGHIJ\nAB".into()); }));
        assert!(svg.contains("<text x=\"300\" y=\"275\" font-size=\"1.500\""), "{svg}");
        assert!(svg.contains("<text x=\"300\" y=\"276.875\" font-size=\"1.500\""), "{svg}");
    }

    /// A value with a line break is drawn as two text elements, the second 1.25 font sizes below the first.
    /// 改行を含む値はtext要素2つで描かれ、2行目は1行目からフォントサイズの1.25倍下になる。
    #[test]
    fn a_two_line_cell_is_drawn_as_two_lines() {
        let svg = cell_svg(&one_cell(|c| c.text = Some("AB\nCD".into())));
        assert!(svg.contains("<text x=\"300\" y=\"275\" font-size=\"3\""), "{svg}");
        assert!(svg.contains("<text x=\"300\" y=\"278.750\" font-size=\"3\""), "{svg}");
    }

    /// A cell rotated by 90 is turned clockwise about its anchor point.
    /// 90回転のセルは、アンカー点を中心に時計回りに回る。
    #[test]
    fn a_rotated_cell_turns_about_its_anchor() {
        let svg = cell_svg(&one_cell(|c| c.rotation = 90));
        assert!(svg.contains("<g transform=\"rotate(90 300 275)\">"), "{svg}");
        assert!(svg.contains("<text x=\"300\" y=\"275\" font-size=\"3\""), "{svg}");
    }

    /// A boxed cell gets a thin rectangle 0.8 mm outside its estimated text box.
    /// boxedのセルは、見積もった文字の箱の0.8mm外側に細い矩形が付く。
    #[test]
    fn a_boxed_cell_gets_a_rectangle_around_the_text() {
        let svg = cell_svg(&one_cell(|c| c.boxed = true));
        // 幅18、ベースライン275、フォント3 → 箱は x 300..318 / y 272..275.75(下へ0.25×フォント)
        assert!(svg.contains("<rect x=\"299.200\" y=\"271.200\" width=\"19.600\" height=\"5.350\" fill=\"none\" stroke=\"#000\" stroke-width=\"0.250\"/>"), "{svg}");
    }

    /// A boxed cell with middle or end alignment gets its box around the text, not around the anchor point.
    /// boxedのセルを中央揃え・右揃えにしても、枠はアンカー点ではなく文字のまわりに付く。
    #[test]
    fn a_boxed_cell_follows_the_alignment() {
        use crate::frame_template::Align;
        let mid = cell_svg(&one_cell(|c| { c.boxed = true; c.align = Align::Middle; }));
        assert!(mid.contains("<rect x=\"290.200\" y=\"271.200\" width=\"19.600\""), "{mid}");
        let end = cell_svg(&one_cell(|c| { c.boxed = true; c.align = Align::End; }));
        assert!(end.contains("<rect x=\"281.200\" y=\"271.200\" width=\"19.600\""), "{end}");
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
            ..Default::default()
        };
        sheet.revisions = vec![
            Revision { mark: "A".into(), date: "26-08-01".into(), description: "初版".into(), by: "K.T".into(), ..Default::default() },
            Revision { mark: "B".into(), date: "26-08-20".into(), description: "端子台追加".into(), by: "K.T".into(), ..Default::default() },
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
            rotation: 0, boxed: false, fit: false, spacing: None,
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

    /// A logo with a loaded image is drawn as that image inside the logo box, without the placeholder rectangle.
    /// 画像を読み込めたロゴは、ロゴ枠の中にその画像として描かれ、置き場所を示す矩形は描かれない。
    #[test]
    fn a_loaded_logo_is_drawn_as_an_image() {
        let t = frame_with(|t| t.logo = Some(crate::frame_template::LogoFrame { x: 15.0, y: 15.0, w: 40.0, h: 20.0, path: Some("l.png".into()), data_url: Some("data:image/png;base64,AAAA".into()) }));
        let svg = sheet_to_svg_with_frame(&golden_sheet(), &builtin_symbols(), &t, &[]);
        assert!(svg.contains("<image x=\"15\" y=\"15\" width=\"40\" height=\"20\" preserveAspectRatio=\"xMidYMid meet\" href=\"data:image/png;base64,AAAA\"/>"), "{svg}");
        assert!(!svg.contains("<rect x=\"15\" y=\"15\" width=\"40\" height=\"20\""), "{svg}");
    }

    /// A frame template with a logo frame draws an empty rectangle where the logo goes, so the space is reserved even before an image is set.
    /// ロゴ枠を持つ図枠テンプレートは、ロゴの位置に空の矩形を描く(画像を入れる前でも場所が確保される)。
    #[test]
    fn a_logo_frame_is_drawn_as_an_empty_rectangle() {
        use crate::frame_template::LogoFrame;
        let mut frame = crate::frame_template::default_template().clone();
        frame.logo = Some(LogoFrame { x: 15.0, y: 15.0, w: 40.0, h: 20.0, path: None, data_url: None });
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
                ..Default::default()
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
            dashed: false, weight_mm: None,
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
            spacing: None,
            stretch: None,
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
            dashed: false, weight_mm: None,
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
                dashed: false, weight_mm: None,
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
            dashed: false, weight_mm: None,
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
            dashed: false, weight_mm: None,
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
            dashed: false, weight_mm: None,
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
            dashed: false, weight_mm: None,
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
            dashed: false, weight_mm: None,
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
            dashed: false, weight_mm: None,
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
            spacing: None,
            stretch: None,
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

    /// A block with top pins draws its reference and value right-aligned (text-anchor end) at the box's top-left corner; a block without top pins keeps the centred caption.
    /// 上辺にピンのあるブロックは参照記号・型番を外形の左上の角に右揃え (text-anchor end) で描く。上辺にピンの無いブロックは今まで通り中央揃え。
    #[test]
    fn svg_puts_top_pin_block_captions_in_the_top_left_corner() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let top = crate::block::test_block("TOP1", "V1", 100.0, 100.0, r#"{"top":[{"no":"1"}],"left":[{"no":"2"}]}"#);
        let flat = crate::block::test_block("FLAT1", "V2", 200.0, 100.0, r#"{"left":[{"no":"1"}],"right":[{"no":"2"}]}"#);
        let (top_id, flat_id) = (top.id(), flat.id());
        sheet.entities.insert(top_id, top);
        sheet.entities.insert(flat_id, flat);
        let svg = sheet_to_svg(&sheet, &crate::symbol::sheet_symbol_defs(&sheet), &[]);
        let line_of = |needle: &str| svg.lines().find(|l| l.contains(needle)).unwrap_or_default().to_string();
        for needle in [">TOP1<", ">V1<"] {
            let l = line_of(needle);
            assert!(l.contains("text-anchor=\"end\""), "{needle} は右揃え: {l}");
        }
        for needle in [">FLAT1<", ">V2<"] {
            let l = line_of(needle);
            assert!(l.contains("text-anchor=\"middle\""), "{needle} は中央揃え: {l}");
        }
    }

    fn binding_edge_marks() -> FrameTemplate {
        use crate::frame_template::{FrameMargins, LineWeight, Marks};
        frame_with(|t| {
            t.frame_margins = Some(FrameMargins { left: 30.0, right: 20.0, top: 18.0, bottom: 16.0 });
            t.marks = Marks {
                trim: true, centre: true,
                trim_insets: Some(FrameMargins { left: 4.0, right: 6.0, top: 5.0, bottom: 3.0 }),
                trim_arm: None, centre_to_trim: true, centre_arrow: true, centre_weight: Some(LineWeight::Thin),
            };
        })
    }

    /// Trim marks follow the insets given per side: each corner of the trim rectangle gets its two arms.
    /// 裁断マークは辺ごとに指定した位置に従う。裁断線の四隅それぞれに腕が2本描かれる。
    #[test]
    fn trim_marks_follow_insets_per_side() {
        let svg = sheet_to_svg_with_frame(&a0_sheet(), &builtin_symbols(), &binding_edge_marks(), &[]);
        for (x1, y1, x2, y2) in [
            ("4", "5", "19", "5"), ("4", "5", "4", "20"),
            ("1183", "5", "1168", "5"), ("1183", "5", "1183", "20"),
            ("4", "838", "19", "838"), ("4", "838", "4", "823"),
            ("1183", "838", "1168", "838"), ("1183", "838", "1183", "823"),
        ] {
            let line = format!("<line x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" stroke=\"#000\" stroke-width=\"0.500\"/>");
            assert!(svg.contains(&line), "{line}");
        }
    }

    /// Centre marks that reach the trim line run from the middle of each border side to the trim line, in the stated weight, and end in an open arrow head there.
    /// 裁断線まで伸ばす中心マークは、外枠の各辺の中央から裁断線まで指定の太さで引かれ、その端に開いた矢印が付く。
    #[test]
    fn centre_marks_reach_the_trim_line_with_an_arrow_head() {
        let svg = sheet_to_svg_with_frame(&a0_sheet(), &builtin_symbols(), &binding_edge_marks(), &[]);
        for (x1, y1, x2, y2) in [
            ("599.500", "18", "599.500", "5"),
            ("599.500", "825", "599.500", "838"),
            ("30", "421.500", "4", "421.500"),
            ("1169", "421.500", "1183", "421.500"),
        ] {
            let line = format!("<line x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" stroke=\"#000\" stroke-width=\"0.250\"/>");
            assert!(svg.contains(&line), "{line}");
        }
        for points in [
            "600,6.900 599.500,5 599,6.900",
            "599,836.100 599.500,838 600,836.100",
            "5.900,421 4,421.500 5.900,422",
            "1181.100,422 1183,421.500 1181.100,421",
        ] {
            let head = format!("<polyline points=\"{points}\" fill=\"none\" stroke=\"#000\" stroke-width=\"0.250\"/>");
            assert!(svg.contains(&head), "{head}");
        }
    }

    /// A centre mark on a side without any margin is not drawn at all.
    /// 余白の無い辺には、中心マークを描かない。
    #[test]
    fn a_centre_mark_without_room_is_not_drawn() {
        let off = sheet_to_svg_with_frame(&a0_sheet(), &builtin_symbols(), &frame_with(|_| {}), &[]);
        let t = frame_with(|t| {
            t.frame_margins = Some(crate::frame_template::FrameMargins { left: 10.0, right: 10.0, top: 10.0, bottom: 0.0 });
            t.marks.centre = true;
        });
        let svg = sheet_to_svg_with_frame(&a0_sheet(), &builtin_symbols(), &t, &[]);
        assert_eq!(svg.matches("<line").count() - off.matches("<line").count(), 3);
    }

    /// The revision table's own text sizes are used for the rows and the header, and every column boundary is ruled from the top of the table to the title block.
    /// 改訂欄に指定した文字サイズが改訂行と見出しに使われ、列の境には表の上端から表題欄まで罫線が引かれる。
    #[test]
    fn revision_text_sizes_and_column_rules() {
        let mut t = six_columns();
        t.revision_block.text_size = Some(2.0);
        t.revision_block.header_size = Some(1.5);
        let mut sheet = golden_sheet();
        sheet.revisions = vec![rev6()];
        let svg = sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &t, &[]);
        assert!(svg.contains("<text x=\"368\" y=\"245.500\" font-size=\"2\" fill=\"#000\" text-anchor=\"middle\" font-family=\"sans-serif\">E-204</text>"), "{svg}");
        assert!(svg.contains("<text x=\"292\" y=\"252.250\" font-size=\"1.500\" fill=\"#000\" text-anchor=\"start\" font-family=\"sans-serif\">SYMB</text>"), "{svg}");
        for x in ["304", "360", "376", "388", "399"] {
            let rule = format!("<line x1=\"{x}\" y1=\"241\" x2=\"{x}\" y2=\"255\" stroke=\"#000\" stroke-width=\"0.250\"/>");
            assert!(svg.contains(&rule), "{rule}");
        }
    }

    fn styled_marks() -> FrameTemplate {
        use crate::frame_template::{MarkShape, MarkStyle, RevisionField};
        let mut t = six_columns();
        t.revision_block.rows_fixed = Some(2);
        t.revision_block.mark_shape = MarkShape::Triangle;
        t.revision_block.mark_style = Some(MarkStyle { h: 1.0, aspect: 0.8, gap: 0.5, pad: 0.0, in_empty_rows: true });
        t.revision_block.columns.as_mut().unwrap()[0].suffix = Some(RevisionField::Count);
        t
    }

    /// A styled mark triangle is as high as the row, as wide as its aspect says, starts at the column edge, leaves the middle of its two sides open, and is also drawn in rows without a revision.
    /// 形を指定した記号の三角は、行と同じ高さ・指定の縦横比で列の左端から始まり、斜辺の中ほどを空け、改訂の無い行にも描かれる。
    #[test]
    fn a_styled_mark_triangle_is_drawn_in_every_row() {
        let mut sheet = golden_sheet();
        sheet.revisions = vec![Revision { mark: "1".into(), ..rev6() }];
        let svg = sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &styled_marks(), &[]);
        // 改訂行 (241〜248): 頂点 (292.8, 241)、底辺 290〜295.6。斜辺は前後の1/4ずつだけ描く
        for (x1, y1, x2, y2) in [
            ("292.800", "241", "293.500", "242.750"), ("294.900", "246.250", "295.600", "248"),
            ("292.800", "241", "292.100", "242.750"), ("290.700", "246.250", "290", "248"),
            ("290", "248", "295.600", "248"),
            // 空行 (234〜241)
            ("292.800", "234", "293.500", "235.750"), ("290", "241", "295.600", "241"),
        ] {
            let line = format!("<line x1=\"{x1}\" y1=\"{y1}\" x2=\"{x2}\" y2=\"{y2}\" stroke=\"#000\" stroke-width=\"0.250\"/>");
            assert!(svg.contains(&line), "{line}\n{svg}");
        }
        assert!(svg.contains("<text x=\"292.800\" y=\"247.160\" font-size=\"2.500\" fill=\"#000\" text-anchor=\"middle\" font-family=\"sans-serif\">1</text>"), "{svg}");
        let plain = sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &six_columns(), &[]);
        assert_eq!(svg.matches("<polygon").count(), plain.matches("<polygon").count(), "切れ目のある三角は線で描く");
    }

    /// A column's suffix field is printed in the same cell, with no rule between: beside the triangle in the mark column, and after the main text elsewhere.
    /// 列のsuffixの項目は、罫線を挟まず同じセルに印字される。記号の列では三角の横に、その他の列では主の文字のあとに続く。
    #[test]
    fn a_suffix_field_shares_the_cell() {
        use crate::frame_template::RevisionField;
        let mut t = styled_marks();
        {
            let cols = t.revision_block.columns.as_mut().unwrap();
            cols.pop();
            cols[4].w = 22.0;
            cols[4].suffix = Some(RevisionField::Approved);
        }
        let mut sheet = golden_sheet();
        sheet.revisions = vec![Revision { mark: "1".into(), ..rev6() }];
        let svg = sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &t, &[]);
        assert!(svg.contains("<text x=\"296.600\" y=\"245.750\" font-size=\"2.500\" fill=\"#000\" text-anchor=\"start\" font-family=\"sans-serif\">x3</text>"), "{svg}");
        assert!(svg.contains("<text x=\"399\" y=\"245.750\" font-size=\"2.500\" fill=\"#000\" text-anchor=\"middle\" font-family=\"sans-serif\">KT MU</text>"), "{svg}");
        // 個数が空なら何も足さない
        sheet.revisions = vec![Revision { mark: "1".into(), count: String::new(), approved: String::new(), ..rev6() }];
        let svg = sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &t, &[]);
        assert!(!svg.contains("x=\"296.600\""), "{svg}");
        assert!(svg.contains("text-anchor=\"middle\" font-family=\"sans-serif\">KT</text>"), "{svg}");
    }

    /// A mark longer than the triangle is wide is shrunk to fit inside it.
    /// 三角より長い記号は、三角に収まるまで縮められる。
    #[test]
    fn a_long_mark_is_fitted_to_the_triangle() {
        let mut t = six_columns();
        t.revision_block.mark_shape = crate::frame_template::MarkShape::Triangle;
        let mut sheet = golden_sheet();
        sheet.revisions = vec![Revision { mark: "R100".into(), ..rev6() }];
        let svg = sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &t, &[]);
        // th = 5.6、hb = 3.233。"R100" は 6mm → 2.5 × 3.233 / 6 = 1.347
        assert!(svg.contains("<text x=\"295.233\" y=\"246.628\" font-size=\"1.347\" fill=\"#000\" text-anchor=\"middle\" font-family=\"sans-serif\">R100</text>"), "{svg}");
    }

    /// A boxed cell whose value is empty draws no box.
    /// 値が空の枠付きセルは、枠を描かない。
    #[test]
    fn a_boxed_cell_without_text_draws_no_box() {
        let svg = cell_svg(&one_cell(|c| { c.boxed = true; c.text = Some(String::new()); }));
        assert!(!svg.contains("width=\"1.600\""), "{svg}");
    }

    /// A column whose header alignment is given separately prints its header with that alignment and its rows with the column's own.
    /// 見出しの揃えを別に指定した列は、見出しをその揃えで、改訂行を列の揃えで印字する。
    #[test]
    fn a_header_can_be_aligned_apart_from_its_rows() {
        let mut t = six_columns();
        t.revision_block.columns.as_mut().unwrap()[1].header_align = Some(crate::frame_template::Align::Middle);
        let mut sheet = golden_sheet();
        sheet.revisions = vec![rev6()];
        let svg = sheet_to_svg_with_frame(&sheet, &builtin_symbols(), &t, &[]);
        // 内容の列: 左端304、幅56 → 見出しは中央 332、内容は左 306
        assert!(svg.contains("<text x=\"332\" y=\"252.600\" font-size=\"2.200\" fill=\"#000\" text-anchor=\"middle\" font-family=\"sans-serif\">CHANGELOG</text>"), "{svg}");
        assert!(svg.contains("<text x=\"306\" y=\"245.750\" font-size=\"2.500\" fill=\"#000\" text-anchor=\"start\" font-family=\"sans-serif\">変更</text>"), "{svg}");
    }

    fn kst(latin: crate::model::Latin) -> crate::model::TextFont {
        crate::model::TextFont { id: "kst32b".into(), latin, pen_mm: None, spacing: None, stretch: None }
    }

    /// 文字を持つ要素をひととおり置いたプロジェクト: リレー2枚組 + 注記付き配線 + ネットラベル + 表題欄 +
    /// 改訂表 + 注記 (通常・太字) + ハーネス + ケーブル、と盤シート (バルーン付きの部品・凡例表・寸法・銘板)。
    fn text_rich_project() -> Project {
        let mut p = relay_project();
        p.sheets[0].title_block = golden_sheet().title_block;
        p.sheets[0].revisions = sheet_with_revisions(&["A", "B"]).revisions;
        let mut bold = plain_note("太字の注記 BOLD");
        bold.bold = true;
        bold.at = Point::new(40.0, 70.0);
        let cable = Entity::Cable(Cable {
            id: Uuid::new_v4(),
            reference: "W1".into(),
            part_no: "MDK-CBL-3C".into(),
            core_count: 3,
            sq: 0.75,
            points: vec![Point::new(200.0, 150.0), Point::new(210.0, 170.0)],
            cores: Vec::new(),
            shield: None,
            length_m: Some(2.5),
            run: Vec::new(),
        });
        let extra = [
            Entity::Text(plain_note("注記 NOTE-1")),
            Entity::Text(bold),
            harness_entity("HN-1", 180.0, 120.0, 230.0, 180.0),
            cable,
        ];
        for e in sheet_with_annotated_wire()
            .entities
            .into_values()
            .chain(sheet_with_net_label(Some("right")).entities.into_values())
            .chain(extra)
        {
            p.sheets[0].entities.insert(e.id(), e);
        }
        let mut panel = panel_sheet_with(Some(1), Some(Point::new(300.0, 200.0)));
        let fp = *panel.entities.keys().next().expect("部品");
        for e in [
            Entity::Dimension(Dimension {
                id: Uuid::new_v4(),
                a: DimAnchor::Footprint { id: fp },
                b: DimAnchor::Point { at: Point::new(180.0, 100.0) },
                axis: DimAxis::Horizontal,
                at: 70.0,
            }),
            Entity::Nameplate(Nameplate {
                id: Uuid::new_v4(),
                footprint: None,
                at: Point::new(250.0, 60.0),
                w: 30.0,
                h: 10.0,
                lines: vec!["主電源 MAIN".into()],
                text_mm: 3.0,
                position: Default::default(),
                follow_description: false,
                part_no: String::new(),
            }),
        ] {
            panel.entities.insert(e.id(), e);
        }
        p.sheets.push(panel);
        p
    }

    /// With a stroke font every text of the sheet, frame and title block included, becomes a stroked path and no text element is left.
    /// ストロークフォントでは、図枠と表題欄を含むシートの全ての文字が線のパスになり、text要素は残らない。
    #[test]
    fn a_stroke_font_turns_all_text_into_paths() {
        let mut p = text_rich_project();
        let ids: Vec<SheetId> = p.sheets.iter().map(|s| s.id).collect();
        for id in &ids {
            let plain = project_sheet_to_svg(&p, *id, &builtin_symbols()).unwrap();
            assert!(plain.contains("<text"));
        }
        let plain_main = project_sheet_to_svg(&p, ids[0], &builtin_symbols()).unwrap();
        for s in ["NOTE-1", "BOLD", "HN-1", "W1", "変更A"] {
            assert!(plain_main.contains(s), "固定の図面に{s}がある");
        }
        let plain_panel = project_sheet_to_svg(&p, *ids.last().unwrap(), &builtin_symbols()).unwrap();
        for s in ["参照記号", "MDK-RLY-24V", "MAIN"] {
            assert!(plain_panel.contains(s), "盤シートに{s}がある");
        }
        p.text_font = Some(kst(crate::model::Latin::Half));
        for id in &ids {
            let svg = project_sheet_to_svg(&p, *id, &builtin_symbols()).unwrap();
            let left: Vec<&str> = svg.lines().filter(|l| l.contains("<text")).collect();
            assert!(left.is_empty(), "{left:?}");
            assert!(svg.contains("stroke-linecap=\"round\" stroke-linejoin=\"round\"/>"));
        }
    }

    /// Selecting a stroke font changes nothing but the text: every other element of the exported sheet is identical.
    /// ストロークフォントを選んでも変わるのは文字だけで、書き出したシートの他の要素は全て同じである。
    #[test]
    fn a_stroke_font_moves_no_geometry() {
        let mut p = text_rich_project();
        let not_text = |svg: &str| -> Vec<String> {
            svg.lines()
                .filter(|l| !l.contains("<text") && !l.contains("stroke-linejoin=\"round\"/>"))
                .map(str::to_string)
                .collect()
        };
        let ids: Vec<SheetId> = p.sheets.iter().map(|s| s.id).collect();
        let plain: Vec<Vec<String>> = ids.iter().map(|id| not_text(&project_sheet_to_svg(&p, *id, &builtin_symbols()).unwrap())).collect();
        p.text_font = Some(kst(crate::model::Latin::Half));
        let stroked: Vec<Vec<String>> = ids.iter().map(|id| not_text(&project_sheet_to_svg(&p, *id, &builtin_symbols()).unwrap())).collect();
        assert_eq!(plain, stroked);
    }

    /// A stroked text starts at its anchor, scaled so that capitals are 0.72 of the font size, in the text colour and with the stroke width of the size.
    /// 線にした文字はアンカーから始まり、大文字がフォントサイズの0.72倍になる縮尺で、文字の色と、サイズに応じた線幅で描かれる。
    #[test]
    fn a_stroked_text_sits_on_its_anchor() {
        let _f = text_font_scope(Some(&kst(crate::model::Latin::Half)));
        let mut out = String::new();
        text_el(&mut out, 100.0, 50.0, 10.0, "#c00000", "end", "H");
        let nums: Vec<f64> = out
            .split("d=\"").nth(1).unwrap()
            .split('"').next().unwrap()
            .split(|c: char| c == ' ' || c == 'M' || c == 'L')
            .filter_map(|t| t.parse().ok())
            .collect();
        let xs: Vec<f64> = nums.iter().step_by(2).copied().collect();
        let ys: Vec<f64> = nums.iter().skip(1).step_by(2).copied().collect();
        let (x0, x1) = xs.iter().fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
        let (y0, y1) = ys.iter().fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
        assert!(x0 >= 95.68 - 1e-9 && x1 <= 100.0 + 1e-9, "{x0} {x1}");
        assert!((y0 - 42.8).abs() < 1e-3 && (y1 - 50.0).abs() < 1e-3, "{y0} {y1}");
        assert!(out.contains("fill=\"none\" stroke=\"#c00000\" stroke-width=\"0.700\""), "{out}");
    }

    /// Bold text is stroked half as thick again.
    /// 太字は1.5倍の太さの線で描かれる。
    #[test]
    fn bold_stroked_text_is_thicker() {
        let _f = text_font_scope(Some(&kst(crate::model::Latin::Half)));
        let mut out = String::new();
        text_weight_el(&mut out, 0.0, 0.0, 10.0, "#000", "start", Weight::Bold, "A");
        assert!(out.contains("stroke-width=\"1.050\""), "{out}");
    }

    /// Text with a character the font lacks keeps its text element, an empty text writes nothing, and the active font does not outlive its scope.
    /// フォントに無い字を含む文字列はtext要素のまま残り、空の文字列は何も書かず、有効な書体はその範囲の外へ持ち越されない。
    #[test]
    fn missing_glyphs_fall_back_and_the_scope_ends() {
        {
            let _f = text_font_scope(Some(&kst(crate::model::Latin::Half)));
            let mut out = String::new();
            text_el(&mut out, 0.0, 0.0, 3.0, "#000", "start", "A😀");
            assert!(out.starts_with("<text"), "{out}");
            let mut empty = String::new();
            text_el(&mut empty, 0.0, 0.0, 3.0, "#000", "start", "");
            assert!(empty.is_empty(), "{empty}");
            {
                let _none = text_font_scope(None);
                let mut inner = String::new();
                text_el(&mut inner, 0.0, 0.0, 3.0, "#000", "start", "A");
                assert!(inner.starts_with("<text"), "{inner}");
            }
            let mut back = String::new();
            text_el(&mut back, 0.0, 0.0, 3.0, "#000", "start", "A");
            assert!(back.starts_with("<path"), "{back}");
        }
        let mut out = String::new();
        text_el(&mut out, 0.0, 0.0, 3.0, "#000", "start", "A");
        assert!(out.starts_with("<text"), "{out}");
    }

    /// The width used to fit text is the stroke width while a stroke font is active, and the estimate otherwise.
    /// 文字を収めるときの幅は、ストロークフォントが有効ならその線の幅、そうでなければ見積もりの幅になる。
    #[test]
    fn text_advance_follows_the_active_font() {
        assert!((text_advance("AB", 10.0, 1.0) - crate::tidy::text_width("AB", 10.0)).abs() < 1e-9);
        let _f = text_font_scope(Some(&kst(crate::model::Latin::Half)));
        assert!((text_advance("AB", 10.0, 1.0) - 8.64).abs() < 1e-9);
        assert!((text_advance("AB", 10.0, 1.5) - 10.8).abs() < 1e-9);
    }

    /// A frame template that asks for full-width Latin letters draws its cells with them, while the rest of the sheet keeps the project's setting.
    /// 英数字を全角にする図枠テンプレートは、セルを全角の字で描き、シートの他の文字はプロジェクトの設定のまま。
    #[test]
    fn a_frame_template_can_use_full_width_latin() {
        let cell_span = |latin: Option<crate::model::Latin>| {
            let mut t = one_cell(|c| { c.text = Some("AB".into()); c.size = 10.0; c.w = None; });
            t.text_latin = latin;
            let _f = text_font_scope(Some(&kst(crate::model::Latin::Half)));
            let svg = frame_only(&sheet_to_svg_with_frame(&golden_sheet(), &builtin_symbols(), &t, &[]));
            let line = svg.lines().find(|l| l.contains("stroke-width=\"0.700\"")).expect("セルの線").to_string();
            let xs: Vec<f64> = line.split("d=\"").nth(1).unwrap().split('"').next().unwrap()
                .split(|c: char| c == ' ' || c == 'M' || c == 'L').filter_map(|v| v.parse().ok()).step_by(2).collect();
            xs.iter().fold(f64::MIN, |a, v| a.max(*v)) - 300.0
        };
        assert!(cell_span(None) <= 8.64 + 1e-9);
        let full = cell_span(Some(crate::model::Latin::Full));
        assert!(full > 8.64 && full <= 17.28 + 1e-9, "{full}");
    }

    /// Letter spacing on a cell widens the gaps between its letters, and fitting uses the stroke width with that spacing.
    /// セルの字間は文字の間を広げ、幅に収める処理はその字間を含めたストロークの幅を使う。
    #[test]
    fn cell_spacing_and_fitting_use_the_stroke_width() {
        let _f = text_font_scope(Some(&kst(crate::model::Latin::Half)));
        // "AB" size 10, spacing 1.5 → width 10.8; fit to 5.4 → size 5 (stroke 0.35)
        let t = one_cell(|c| { c.text = Some("AB".into()); c.size = 10.0; c.w = Some(5.4); c.fit = true; c.spacing = Some(1.5); });
        let svg = frame_only(&sheet_to_svg_with_frame(&golden_sheet(), &builtin_symbols(), &t, &[]));
        assert!(svg.contains("stroke-width=\"0.350\""), "{svg}");
    }

    fn coloured_wire() -> Entity {
        Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(50.0, 60.0), Point::new(90.0, 60.0)],
            color: "yellow".into(),
            sq: 0.0,
            length_m: None,
            part_no: None,
            net: None,
            length_source: crate::model::LengthSource::Manual,
            dashed: false, weight_mm: None,
        })
    }

    fn wire_line(svg: &str) -> String {
        svg.lines()
            .find(|l| l.starts_with("<polyline") && l.contains("50,60 90,60"))
            .unwrap_or_else(|| panic!("配線が描かれていない: {svg}"))
            .to_string()
    }

    /// Wires are drawn 0.35 mm wide on a sheet that set no wire weight, and at the sheet's weight once it is set.
    /// 配線は、太さを設定していないシートでは0.35mm、設定したシートではその太さで描かれる。
    #[test]
    fn wires_are_drawn_at_the_weight_of_the_sheet() {
        let mut sheet = sheet_with(vec![coloured_wire()]);
        let line = wire_line(&sheet_to_svg(&sheet, &builtin_symbols(), &[]));
        assert_eq!(attr(&line, "stroke-width"), Some(0.35), "{line}");
        sheet.wire_weight_mm = Some(0.7);
        let line = wire_line(&sheet_to_svg(&sheet, &builtin_symbols(), &[]));
        assert_eq!(attr(&line, "stroke-width"), Some(0.7), "{line}");
    }

    /// A sheet that hides wire notes draws a coloured wire in its colour but without the colour word; the same sheet without the switch writes the word.
    /// 配線注記を描かないシートは、色付きの配線を色のまま描くが色名は書かない。同じシートでこの設定を外すと色名が出る。
    #[test]
    fn a_sheet_can_hide_the_wire_notes() {
        let mut sheet = sheet_with(vec![coloured_wire()]);
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
        assert!(svg.contains(">黄<"), "既定では色名が出る: {svg}");
        sheet.hide_wire_notes = true;
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
        assert!(!svg.contains(">黄<"), "色名が残っている: {svg}");
        assert!(wire_line(&svg).contains(color_hex("yellow")), "線の色は変わらない");
        assert!(crate::wire_note::wire_notes(&sheet).is_empty());
    }

    /// A note turned 90, 180 or 270 degrees is drawn inside a rotation about its own anchor point; a note that is not turned is written exactly as before, with no rotation around it.
    /// 90・180・270度回した注記は、自分の基準点を中心にした回転の中に描かれる。回していない注記は回転を付けず、これまでと同じに書かれる。
    #[test]
    fn a_turned_note_is_rotated_about_its_anchor() {
        let plain = plain_note("ABC");
        let before = sheet_to_svg(&sheet_with(vec![Entity::Text(plain.clone())]), &builtin_symbols(), &[]);
        assert!(!before.contains("rotate("), "{before}");
        for deg in [90u16, 180, 270] {
            let mut note = plain.clone();
            note.rotation = deg;
            let svg = sheet_to_svg(&sheet_with(vec![Entity::Text(note)]), &builtin_symbols(), &[]);
            let open = format!("<g transform=\"rotate({deg} 40 60)\">");
            let at = svg.find(&open).unwrap_or_else(|| panic!("{deg}度の回転が無い: {svg}"));
            let rest = &svg[at + open.len()..];
            let close = rest.find("</g>").expect("回転を閉じる");
            assert!(rest[..close].contains(">ABC<"), "回転の中に文字が無い: {}", &rest[..close]);
            assert_eq!(svg.replace(&open, "").replacen("</g>\n", "", 1).len() + open.len() + 5, svg.len());
        }
        let mut odd = plain.clone();
        odd.rotation = 45;
        let svg = sheet_to_svg(&sheet_with(vec![Entity::Text(odd)]), &builtin_symbols(), &[]);
        assert!(!svg.contains("rotate("), "90度刻みでない回転は描かない: {svg}");
    }

    fn captioned_block(caption: Option<&str>, reference: &str, value: &str) -> crate::model::SymbolInstance {
        let Entity::Symbol(mut inst) = crate::block::test_block(reference, value, 100.0, 80.0, r#"{"left":[{"no":"1"}],"right":[{"no":"2"}]}"#) else {
            unreachable!()
        };
        inst.attrs.insert(crate::block::BLOCK_SIZE_ATTR.into(), r#"{"w":20,"h":10}"#.into());
        if let Some(c) = caption {
            inst.attrs.insert(crate::block::BLOCK_CAPTION_ATTR.into(), c.into());
        }
        inst
    }

    fn caption_y(svg: &str, text: &str) -> (f64, f64) {
        let line = svg
            .lines()
            .find(|l| l.contains(&format!(">{text}<")))
            .unwrap_or_else(|| panic!("「{text}」が描かれていない: {svg}"));
        assert!(line.contains("text-anchor=\"middle\""), "{line}");
        (attr(line, "x").unwrap(), attr(line, "y").unwrap())
    }

    /// A block with block_caption "inside" writes its reference and its value centred inside the outline, the two lines centred on the middle of the box; without the attribute they stay above the box.
    /// block_caption が "inside" のブロックは、参照記号と型番を外形の中に中央揃えで書き、2行のまとまりを箱の中央の高さにそろえる。属性が無ければ箱の上のまま。
    #[test]
    fn a_block_can_write_its_caption_inside_the_outline() {
        let above = sheet_to_svg(&sheet_with(vec![Entity::Symbol(captioned_block(None, "U1", "AMP"))]), &builtin_symbols(), &[]);
        let (_, y_ref) = caption_y(&above, "U1");
        assert!(y_ref < 75.0, "属性が無ければ箱 (上端75) の上: {y_ref}");

        let inside = sheet_to_svg(&sheet_with(vec![Entity::Symbol(captioned_block(Some("inside"), "U1", "AMP"))]), &builtin_symbols(), &[]);
        let (x_ref, y_ref) = caption_y(&inside, "U1");
        let (x_val, y_val) = caption_y(&inside, "AMP");
        assert_eq!((x_ref, x_val), (100.0, 100.0));
        assert!((y_val - y_ref - (REF_LABEL_DY - VALUE_LABEL_DY)).abs() < 1e-9, "行の間隔は変わらない");
        let cap = CAPTION_CAP_RATIO * LABEL_FONT;
        let middle = (y_ref - cap + y_val) / 2.0;
        assert!((middle - 80.0).abs() < 1e-6, "2行のまとまりの中心が箱の中心 (80): {middle}");
        assert!(y_ref - cap > 75.0 && y_val < 85.0, "箱 (75..85) に収まる: {y_ref} {y_val}");
    }

    /// With only a reference, or only a value, the single line of an inside caption is centred on the middle of the box.
    /// 参照記号だけ、または型番だけのとき、中に書く1行は箱の中央の高さにそろう。
    #[test]
    fn a_single_inside_caption_line_is_centred() {
        let cap = CAPTION_CAP_RATIO * LABEL_FONT;
        let svg = sheet_to_svg(&sheet_with(vec![Entity::Symbol(captioned_block(Some("inside"), "U1", ""))]), &builtin_symbols(), &[]);
        let (_, y) = caption_y(&svg, "U1");
        assert!((y - cap / 2.0 - 80.0).abs() < 1e-6, "{y}");
        let svg = sheet_to_svg(&sheet_with(vec![Entity::Symbol(captioned_block(Some("inside"), "", "AMP"))]), &builtin_symbols(), &[]);
        let (_, y) = caption_y(&svg, "AMP");
        assert!((y - cap / 2.0 - 80.0).abs() < 1e-6, "{y}");
    }

    /// The junction dot keeps its 0.6 mm radius at the default wire weight and grows to 0.9 times the weight on a sheet with thick wires, so it stays visible.
    /// 分岐点の丸は、既定の配線の太さでは半径0.6mmのまま、太い配線のシートでは太さの0.9倍になり、線に埋もれない。
    #[test]
    fn the_junction_dot_grows_with_a_thick_wire() {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        assert_eq!(junction_dot_r(&sheet), 0.6);
        sheet.wire_weight_mm = Some(0.5);
        assert_eq!(junction_dot_r(&sheet), 0.6);
        sheet.wire_weight_mm = Some(1.0);
        assert!((junction_dot_r(&sheet) - 0.9).abs() < 1e-9);
        let j = Entity::Junction(crate::model::Junction { id: Uuid::new_v4(), at: Point::new(70.0, 60.0) });
        sheet.entities.insert(j.id(), j);
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
        let dot = svg.lines().find(|l| l.starts_with("<circle") && l.contains("cx=\"70\"")).expect("丸が描かれる");
        assert_eq!(attr(dot, "r"), Some(0.9), "{dot}");
    }

    /// The outline of a symbol is drawn 0.3 mm wide on a sheet that set no outline weight, and at the sheet's weight once it is set.
    /// シンボルの外形は、太さを設定していないシートでは0.3mm、設定したシートではその太さで描かれる。
    #[test]
    fn symbol_outlines_are_drawn_at_the_weight_of_the_sheet() {
        let mut sheet = sheet_with(vec![Entity::Symbol(captioned_block(None, "U1", ""))]);
        let outline = |svg: &str| -> String {
            svg.lines().find(|l| l.starts_with("<polygon") && l.contains("90,75")).unwrap_or_else(|| panic!("外形が無い: {svg}")).to_string()
        };
        assert_eq!(attr(&outline(&sheet_to_svg(&sheet, &builtin_symbols(), &[])), "stroke-width"), Some(0.3));
        sheet.symbol_weight_mm = Some(1.0);
        assert_eq!(attr(&outline(&sheet_to_svg(&sheet, &builtin_symbols(), &[])), "stroke-width"), Some(1.0));
    }

    /// A sheet with vivid wire colours draws a yellow wire in pure yellow; without the switch it uses the darker paper colour. The colour name of the wire is not changed.
    /// 鮮やかな配線の色のシートは、黄の配線を純色の黄で描く。設定が無ければ紙で見やすい濃い色で描く。配線の色名は変わらない。
    #[test]
    fn a_sheet_can_draw_wires_in_vivid_colours() {
        let mut sheet = sheet_with(vec![coloured_wire()]);
        assert!(wire_line(&sheet_to_svg(&sheet, &builtin_symbols(), &[])).contains("stroke=\"#b89000\""));
        sheet.vivid_wire_colors = true;
        assert!(wire_line(&sheet_to_svg(&sheet, &builtin_symbols(), &[])).contains("stroke=\"#ffff00\""));
        assert_eq!(color_hex_vivid("light_blue"), "#00ffff");
        assert_eq!(color_hex_vivid("桃"), "#ff00ff");
        assert_eq!(color_hex_vivid("black"), "#000000");
        assert_eq!(color_hex_vivid("#123456"), "#123456");
        let Entity::Wire(w) = sheet.entities.values().next().unwrap() else { unreachable!() };
        assert_eq!(w.color, "yellow");
    }

    /// A wire can be dashed and can have its own weight: such a wire is drawn with a dash pattern and at its own weight, while a plain wire on the same sheet stays solid at the sheet's weight.
    /// 配線は破線にでき、その線だけの太さを持てる: その配線は破線・自分の太さで描かれ、同じシートの普通の配線は実線・シートの太さのまま。
    #[test]
    fn a_wire_can_be_dashed_with_its_own_weight() {
        let Entity::Wire(mut bus) = coloured_wire() else { unreachable!() };
        bus.points = vec![Point::new(50.0, 80.0), Point::new(90.0, 80.0)];
        bus.dashed = true;
        bus.weight_mm = Some(2.0);
        let mut sheet = sheet_with(vec![coloured_wire(), Entity::Wire(bus)]);
        sheet.wire_weight_mm = Some(0.7);
        let svg = sheet_to_svg(&sheet, &builtin_symbols(), &[]);
        let plain = wire_line(&svg);
        assert!(!plain.contains("stroke-dasharray") && attr(&plain, "stroke-width") == Some(0.7), "{plain}");
        let dashed = svg.lines().find(|l| l.starts_with("<polyline") && l.contains("50,80 90,80")).unwrap();
        assert!(dashed.contains("stroke-dasharray=\"4 2.500\""), "{dashed}");
        assert_eq!(attr(dashed, "stroke-width"), Some(2.0));
    }

    /// A line shape with an arrow at its end is drawn with a filled triangle in the line's colour; the same line without the arrow has none.
    /// 終点に矢印のある線の図形は、線と同じ色で塗った三角と一緒に描かれる。矢印の無い同じ線には三角は無い。
    #[test]
    fn a_line_shape_can_end_in_an_arrow() {
        let mut line = crate::shape::new_shape(ShapeKind::Line, vec![Point::new(10.0, 20.0), Point::new(30.0, 20.0)]);
        let svg = sheet_to_svg(&sheet_with(vec![Entity::Shape(line.clone())]), &builtin_symbols(), &[]);
        assert!(!svg.lines().any(|l| l.starts_with("<polygon") && l.contains("30,20")), "{svg}");
        line.arrow_end = true;
        let svg = sheet_to_svg(&sheet_with(vec![Entity::Shape(line)]), &builtin_symbols(), &[]);
        let head = svg.lines().find(|l| l.starts_with("<polygon") && l.contains("30,20")).unwrap_or_else(|| panic!("矢印が無い: {svg}"));
        assert!(head.contains("stroke=\"none\"") && !head.contains("fill=\"none\""), "{head}");
    }

    /// With a text pen set on the font, every text is stroked at that width whatever its size, and bold text at 1.5 times it; without a pen the width follows the size.
    /// 書体にペンの太さがあると、文字は大きさによらずその太さで描かれ、太字は1.5倍になる。ペンの太さが無ければ、太さは文字の大きさに比例する。
    #[test]
    fn a_text_pen_fixes_the_stroke_width() {
        let font = |pen| crate::model::TextFont { id: "kst32b".into(), latin: crate::model::Latin::Half, pen_mm: pen, spacing: None, stretch: None, };
        {
            let _scope = text_font_scope(Some(&font(None)));
            assert!((text_stroke_width(10.0, false) - 0.7).abs() < 1e-9);
            assert!((text_stroke_width(2.0, false) - 0.14).abs() < 1e-9);
        }
        {
            let _scope = text_font_scope(Some(&font(Some(0.35))));
            assert_eq!(text_stroke_width(10.0, false), 0.35);
            assert_eq!(text_stroke_width(2.0, false), 0.35);
            assert!((text_stroke_width(2.0, true) - 0.525).abs() < 1e-9);
            let mut out = String::new();
            assert!(stroke_text_el(&mut out, 10.0, 10.0, 5.0, "#000", "start", false, 1.0, "A"));
            assert!(out.contains("stroke-width=\"0.350\""), "{out}");
        }
        {
            // 範囲外のペンは無いものとして扱う
            let _scope = text_font_scope(Some(&font(Some(5.0))));
            assert!((text_stroke_width(10.0, false) - 0.7).abs() < 1e-9);
        }
        assert!((text_stroke_width(10.0, false) - 0.7).abs() < 1e-9, "範囲の外へ漏れない");
    }

    /// A symbol with a caption layout writes its reference at the given offset from the placement point, with the given size and alignment; the value line, which the layout leaves out, stays where it was; a hidden line is not written.
    /// caption_layout を持つシンボルは、参照記号を配置点からの指定のずれ・大きさ・寄せで書く。指定の無い型番の行はこれまでの場所のまま。非表示にした行は書かない。
    #[test]
    fn a_caption_layout_places_each_line() {
        let plain = captioned_block(None, "U1", "AMP");
        let base = sheet_to_svg(&sheet_with(vec![Entity::Symbol(plain.clone())]), &builtin_symbols(), &[]);
        let value_line = |svg: &str| svg.lines().find(|l| l.contains(">AMP<")).map(str::to_string);
        let mut inst = plain.clone();
        inst.attrs.insert(crate::caption::CAPTION_LAYOUT_ATTR.into(), r#"{"ref":{"x":-12,"y":-6,"h":2.0,"anchor":"start"}}"#.into());
        let svg = sheet_to_svg(&sheet_with(vec![Entity::Symbol(inst.clone())]), &builtin_symbols(), &[]);
        let line = svg.lines().find(|l| l.contains(">U1<")).unwrap();
        assert_eq!((attr(line, "x"), attr(line, "y"), attr(line, "font-size")), (Some(88.0), Some(74.0), Some(2.0)), "{line}");
        assert!(!line.contains("text-anchor=\"middle\""), "{line}");
        assert_eq!(value_line(&svg), value_line(&base), "指定の無い行は動かない");
        inst.attrs.insert(crate::caption::CAPTION_LAYOUT_ATTR.into(), r#"{"ref":{"hide":true},"value":{"x":0,"y":8,"anchor":"end"}}"#.into());
        let svg = sheet_to_svg(&sheet_with(vec![Entity::Symbol(inst.clone())]), &builtin_symbols(), &[]);
        assert!(!svg.contains(">U1<"), "非表示の行が出ている");
        let line = value_line(&svg).unwrap();
        assert_eq!((attr(&line, "x"), attr(&line, "y")), (Some(100.0), Some(88.0)));
        assert!(line.contains("text-anchor=\"end\""), "{line}");
        // 壊れた属性は無いものとして、これまでの場所に書く
        inst.attrs.insert(crate::caption::CAPTION_LAYOUT_ATTR.into(), "{".into());
        let svg = sheet_to_svg(&sheet_with(vec![Entity::Symbol(inst)]), &builtin_symbols(), &[]);
        assert_eq!(svg.lines().find(|l| l.contains(">U1<")), base.lines().find(|l| l.contains(">U1<")));
    }

    /// The letter spacing of the font tightens every text, and a note's own spacing multiplies it: with the font at 0.5 a note at 2.0 is as wide as plain text.
    /// 書体の字間は全ての文字を詰め、注記の字間はそれに掛かる: 書体が0.5で注記が2.0なら、字間なしの文字と同じ幅になる。
    #[test]
    fn font_and_note_spacing_multiply() {
        let font = |spacing| crate::model::TextFont { id: "kst32b".into(), latin: crate::model::Latin::Half, pen_mm: None, spacing, stretch: None };
        let plain = {
            let _scope = text_font_scope(Some(&font(None)));
            text_advance("ABCDE", 5.0, 1.0)
        };
        let _scope = text_font_scope(Some(&font(Some(0.5))));
        let tight = text_advance("ABCDE", 5.0, 1.0);
        assert!(tight < plain, "{tight} < {plain}");
        {
            let _note = spacing_scope(Some(2.0));
            assert!((text_advance("ABCDE", 5.0, 1.0) - plain).abs() < 1e-9);
        }
        assert!((text_advance("ABCDE", 5.0, 1.0) - tight).abs() < 1e-9, "注記の範囲を出ると戻る");
        {
            let _bad = spacing_scope(Some(10.0));
            assert!((text_advance("ABCDE", 5.0, 1.0) - tight).abs() < 1e-9, "範囲外の字間は使わない");
        }
    }

    /// The line of an arrow stops at the base of the triangle, not at its tip, so a thick line does not stick out beside the tip.
    /// 矢印のある線は、三角の先端ではなく付け根で止まる。太い線の角が先端の横からはみ出さない。
    #[test]
    fn the_line_of_an_arrow_stops_at_the_base_of_the_head() {
        let mut line = crate::shape::new_shape(ShapeKind::Line, vec![Point::new(10.0, 20.0), Point::new(30.0, 20.0)]);
        line.arrow_end = true;
        line.stroke_width_mm = 1.0;
        let svg = sheet_to_svg(&sheet_with(vec![Entity::Shape(line)]), &builtin_symbols(), &[]);
        let drawn = svg.lines().find(|l| l.starts_with("<line") && l.contains("x1=\"10\"") && l.contains("y1=\"20\"")).unwrap_or_else(|| panic!("{svg}"));
        let len = crate::shape::ARROW_LEN_MM + 2.0;
        assert_eq!(attr(drawn, "x2"), Some(30.0 - len), "{drawn}");
        assert!(svg.lines().any(|l| l.starts_with("<polygon") && l.contains("30,20")), "三角の先端は終点のまま");
    }

    /// A wire with an explicit net name shows that name as its wire number; a sheet that hides wire numbers does not write it, and the name stays on the wire.
    /// ネット名を明示した配線は、その名前を線番として表示する。線番を描かないシートは名前を書かず、名前は配線に残る。
    #[test]
    fn a_sheet_can_hide_the_wire_numbers() {
        let Entity::Wire(mut w) = coloured_wire() else { unreachable!() };
        w.color = String::new();
        w.net = Some("BUS_L".into());
        let mut sheet = sheet_with(vec![Entity::Wire(w)]);
        assert!(sheet_to_svg(&sheet, &builtin_symbols(), &[]).contains(">BUS_L<"));
        sheet.hide_wire_numbers = true;
        assert!(!sheet_to_svg(&sheet, &builtin_symbols(), &[]).contains(">BUS_L<"));
        let Entity::Wire(w) = sheet.entities.values().next().unwrap() else { unreachable!() };
        assert_eq!(w.net.as_deref(), Some("BUS_L"));
    }

    /// The horizontal stretch of the font narrows the glyphs and the width together and leaves the height alone; a note's own stretch multiplies it.
    /// 書体の横の伸縮率は、字形と幅を一緒に縮め、高さは変えない。注記の伸縮率はそれに掛かる。
    #[test]
    fn the_text_stretch_scales_glyphs_and_width() {
        let font = |stretch| crate::model::TextFont { id: "kst32b".into(), latin: crate::model::Latin::Half, pen_mm: None, spacing: None, stretch };
        let path_xs = |out: &str| -> (f64, f64, f64, f64) {
            let d = out.split("d=\"").nth(1).unwrap().split('"').next().unwrap();
            let nums: Vec<f64> = d.split_whitespace().filter_map(|t| t.trim_start_matches(['M', 'L']).parse().ok()).collect();
            let xs: Vec<f64> = nums.iter().step_by(2).copied().collect();
            let ys: Vec<f64> = nums.iter().skip(1).step_by(2).copied().collect();
            (xs.iter().cloned().fold(f64::MAX, f64::min), xs.iter().cloned().fold(f64::MIN, f64::max), ys.iter().cloned().fold(f64::MAX, f64::min), ys.iter().cloned().fold(f64::MIN, f64::max))
        };
        let draw = |f: &crate::model::TextFont, note: Option<f64>| {
            let _scope = text_font_scope(Some(f));
            let _note = note_text_scope(None, note);
            let mut out = String::new();
            assert!(stroke_text_el(&mut out, 0.0, 10.0, 5.0, "#000", "start", false, 1.0, "HH"));
            (path_xs(&out), text_advance("HH", 5.0, 1.0))
        };
        let ((x0, x1, y0, y1), w) = draw(&font(None), None);
        let ((nx0, nx1, ny0, ny1), nw) = draw(&font(Some(0.5)), None);
        assert!((nw - w / 2.0).abs() < 1e-9, "幅は半分: {nw} {w}");
        assert!((nx0 - x0 / 2.0).abs() < 1e-3 && (nx1 - x1 / 2.0).abs() < 1e-3, "字形の x も半分: {nx0} {nx1} / {x0} {x1}");
        assert_eq!((ny0, ny1), (y0, y1), "高さは変わらない");
        let (_, back) = draw(&font(Some(0.5)), Some(2.0));
        assert!((back - w).abs() < 1e-9, "注記の2倍で元の幅");
    }

    /// The horizontal stretch of the font does not reach the frame: inside the frame's text scope a text is as wide as without any stretch, and after the scope the stretch applies again.
    /// 書体の横の伸縮率は図枠には効かない: 図枠の文字の範囲の中では、文字は伸縮率なしと同じ幅になり、範囲を出ると伸縮率が戻る。
    #[test]
    fn the_frame_text_ignores_the_font_stretch() {
        let font = |stretch| crate::model::TextFont { id: "kst32b".into(), latin: crate::model::Latin::Half, pen_mm: None, spacing: None, stretch };
        let plain = {
            let _scope = text_font_scope(Some(&font(None)));
            text_advance("ABC", 5.0, 1.0)
        };
        let _scope = text_font_scope(Some(&font(Some(0.5))));
        assert!((text_advance("ABC", 5.0, 1.0) - plain / 2.0).abs() < 1e-9);
        {
            let _frame = latin_scope(None);
            assert!((text_advance("ABC", 5.0, 1.0) - plain).abs() < 1e-9);
        }
        assert!((text_advance("ABC", 5.0, 1.0) - plain / 2.0).abs() < 1e-9);
    }

    /// Builds a two-sheet project whose sheets both carry the label PUMP; the first label is tagged.
    fn tag_project(style: crate::model::XrefStyle, extra: &[(&str, &str)]) -> (crate::model::Project, crate::model::SheetId) {
        use crate::model::*;
        let mut p = Project::new("t");
        p.sheets[0].name = "Power".into();
        p.sheets[0].xref_style = style;
        let mut second = Sheet::new("Motor", PaperSize::A3, Orientation::Landscape);
        let far = Entity::NetLabel(NetLabel { id: uuid::Uuid::new_v4(), at: Point::new(250.0, 70.0), name: "PUMP".into(), rotation: 0, attrs: Default::default() });
        second.entities.insert(far.id(), far);
        p.sheets.push(second);
        let mut attrs = std::collections::BTreeMap::new();
        attrs.insert("tag".to_string(), "left".to_string());
        for (k, v) in extra {
            attrs.insert((*k).to_string(), (*v).to_string());
        }
        let near = Entity::NetLabel(NetLabel { id: uuid::Uuid::new_v4(), at: Point::new(100.0, 50.0), name: "PUMP".into(), rotation: 0, attrs });
        let sid = p.sheets[0].id;
        p.sheets[0].entities.insert(near.id(), near);
        (p, sid)
    }

    /// In the sheet-name style a tagged label draws the name in bold and the destination sheet name inside the flag at 0.8 of the size, and nothing monospace outside the flag.
    /// シート名の書き方では、タグ付きラベルは太字のネット名と、行き先のシート名 (文字高0.8倍) を旗の中に描き、旗の外には何も描かない。
    #[test]
    fn the_sheet_name_style_draws_the_destination_inside_the_flag() {
        let (p, sid) = tag_project(crate::model::XrefStyle::SheetName, &[]);
        let svg = project_sheet_to_svg(&p, sid, &crate::symbol::builtin_symbols()).unwrap();
        assert!(svg.contains(">Motor</text>"), "{svg}");
        assert!(svg.contains("font-weight=\"bold\"") && svg.contains(">PUMP</text>"));
        assert!(!svg.contains("/2."), "ゾーン住所は描かない");
        assert!(!svg.contains("font-family=\"monospace\""), "旗の外の文字は無い");
    }

    /// In the zone style the same tagged label draws its zone address outside the flag, as it always has.
    /// ゾーン住所の書き方では、同じタグ付きラベルが今までどおりゾーン住所を旗の外に描く。
    #[test]
    fn the_zone_style_keeps_the_address_outside_the_flag() {
        let (p, sid) = tag_project(crate::model::XrefStyle::Zone, &[]);
        let svg = project_sheet_to_svg(&p, sid, &crate::symbol::builtin_symbols()).unwrap();
        assert!(svg.contains(">/2.B3</text>"), "{svg}");
        assert!(!svg.contains(">Motor</text>"));
    }

    /// A tag_dest replaces the outside text in the zone style and the inside text in the sheet-name style, also when drawing the sheet alone.
    /// tag_dest は、ゾーン住所の書き方では旗の外の文字を、シート名の書き方では旗の中の文字を置き換える。シート単体の出力でも同じ。
    #[test]
    fn tag_dest_replaces_the_text_in_both_styles() {
        for style in [crate::model::XrefStyle::Zone, crate::model::XrefStyle::SheetName] {
            let (p, sid) = tag_project(style, &[("tag_dest", "Field box")]);
            let svg = project_sheet_to_svg(&p, sid, &crate::symbol::builtin_symbols()).unwrap();
            assert!(svg.contains(">Field box</text>"), "{style:?}");
            assert!(!svg.contains(">Motor</text>") && !svg.contains(">/2.B3</text>"), "{style:?}");
        }
        let (p, sid) = tag_project(crate::model::XrefStyle::Zone, &[("tag_dest", "Field box")]);
        let svg = sheet_to_svg(p.sheet(sid).unwrap(), &crate::symbol::builtin_symbols(), &[]);
        assert!(svg.contains(">Field box</text>"));
    }

    /// With no other sheet carrying the name, the sheet-name flag shows only the name, and an untagged label keeps its outside zone address in that style.
    /// 他のシートに同名が無いと、シート名の旗には名前だけが出る。タグの無いラベルは、この書き方でも旗の外でなくラベルの脇にゾーン住所を描く。
    #[test]
    fn a_flag_without_a_destination_shows_only_the_name() {
        let (mut p, sid) = tag_project(crate::model::XrefStyle::SheetName, &[]);
        p.sheets.pop();
        let svg = project_sheet_to_svg(&p, sid, &crate::symbol::builtin_symbols()).unwrap();
        assert!(svg.contains(">PUMP</text>") && !svg.contains("Motor"));
        let (mut p, sid) = tag_project(crate::model::XrefStyle::SheetName, &[]);
        for e in p.sheets[0].entities.values_mut() {
            if let crate::model::Entity::NetLabel(l) = e {
                l.attrs.clear();
            }
        }
        let svg = project_sheet_to_svg(&p, sid, &crate::symbol::builtin_symbols()).unwrap();
        assert!(svg.contains(">/2.B3</text>"), "{svg}");
    }

    /// A sheet with no tags and the default style renders byte-identical SVG to the sheet rendered with the zone style set explicitly, and the flow-out flag moves only the polygon.
    /// 既定の書き方のシートは、ゾーン住所の書き方を明示した場合とバイトまで同じSVGになる。tag_flow out は旗の多角形だけを変える。
    #[test]
    fn the_default_style_renders_as_before_and_flow_out_only_moves_the_polygon() {
        let (p, sid) = tag_project(crate::model::XrefStyle::Zone, &[]);
        let base = project_sheet_to_svg(&p, sid, &crate::symbol::builtin_symbols()).unwrap();
        assert_eq!(p.sheets[0].xref_style, crate::model::XrefStyle::default());
        let (q, qid) = tag_project(crate::model::XrefStyle::Zone, &[("tag_flow", "out")]);
        let out = project_sheet_to_svg(&q, qid, &crate::symbol::builtin_symbols()).unwrap();
        let strip = |s: &str| s.lines().filter(|l| !l.starts_with("<polygon")).collect::<Vec<_>>().join("\n");
        assert_ne!(base, out);
        let norm = |s: String| s.lines().filter(|l| !l.contains("PUMP") && !l.contains("/2.B3")).collect::<Vec<_>>().join("\n");
        assert_eq!(norm(strip(&base)), norm(strip(&out)));
    }

    fn block_svg(pins: &str, extra: &[(&str, &str)]) -> String {
        let mut sheet = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        let Entity::Symbol(mut inst) = crate::block::test_block("U1", "", 100.0, 50.0, pins) else { unreachable!() };
        for (k, v) in extra {
            inst.attrs.insert((*k).into(), (*v).into());
        }
        let block = Entity::Symbol(inst);
        sheet.entities.insert(block.id(), block);
        sheet_to_svg(&sheet, &crate::symbol::sheet_symbol_defs(&sheet), &[])
    }

    /// A block's caption row is drawn in bold inside the box, an unnumbered pin prints its name once and no number, and a spacer row draws nothing.
    /// ブロックの見出しの行は箱の中に太字で描かれる。番号のないピンは名前を1回だけ書いて番号は書かず、すき間の行は何も描かない。
    #[test]
    fn svg_draws_block_captions_in_bold_and_unnumbered_pins_by_name() {
        let svg = block_svg(r#"{"left":[{"caption":"POWER"},{"name":"VIN"},{"gap":2},{"no":"7","name":"GND"}]}"#, &[]);
        assert!(svg.contains("font-weight=\"bold\">POWER<"), "{svg}");
        assert_eq!(svg.matches(">VIN<").count(), 1, "名前だけ: {svg}");
        assert_eq!(svg.matches(">GND<").count(), 1);
        assert_eq!(svg.matches(">7<").count(), 1);
        // 見出し以外の文字は太字にならない
        assert_eq!(svg.matches("font-weight=\"bold\"").count(), 1);
    }

    /// block_pin_text sets the font size of the pin names, captions and numbers in the drawing, and writing the default size explicitly changes nothing.
    /// block_pin_text は図面のピン名・見出し・番号の文字の大きさを決める。既定の大きさを明示しても何も変わらない。
    #[test]
    fn svg_draws_block_texts_at_the_block_pin_text_size() {
        let pins = r#"{"left":[{"caption":"POWER"},{"no":"1","name":"VIN"}]}"#;
        let big = block_svg(pins, &[("block_pin_text", "3.5")]);
        for t in ["POWER", "VIN", "1"] {
            assert!(big.lines().any(|l| l.contains(&format!(">{t}</text>")) && l.contains("font-size=\"3.500\"")), "{t}");
        }
        assert_eq!(block_svg(pins, &[]), block_svg(pins, &[("block_pin_text", "2")]));
        assert_ne!(block_svg(pins, &[]), big);
    }
}
