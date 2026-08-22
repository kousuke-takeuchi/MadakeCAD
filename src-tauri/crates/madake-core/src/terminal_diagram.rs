//! 端子接続図 (グラフィカル、spec §1.2)。EPLANの端子図様式で端子台1つを1ページに描く。
//!
//! - 中央に**端子ストリップ** (端子番号入りの箱を縦に積む)。予備端子は薄く塗って残す
//! - **外部側 (盤外) は左、内部側 (盤内) は右**。各端子から引出線を伸ばし、その先に接続先
//!   (`参照記号:ピン番号` またはネットラベル名)、引出線の下に電線の仕様 (線色・sq・品番) を書く
//! - 同じハーネスに属する電線は引出線の外端で**ブラケット**にまとめ、ハーネス名を記す
//! - 隣り合う端子のサドルジャンパは端子箱の**内部側の縁**に縦線で描く
//!
//! 行データは [`crate::terminal_chart::terminal_chart`] がただ1つの情報源で、ここは描画だけを行う。
//! 用紙・図枠・表題欄は帳票ページ ([`crate::report_sheet`]) と共通。

use crate::model::{EntityId, Project};
use crate::report_sheet::{
    body_bottom, fit_text, page_open, report_title_block, table_top, ReportKind, ReportMeta,
    REPORT_PAPER,
};
use crate::svg::{line_el, rect_el, text_el, CELL_PAD, FRAME_MARGIN, FRAME_STROKE};
use crate::terminal_chart::{terminal_block_ids, terminal_chart, TerminalRow};

/// 端子箱の幅 (mm)。
const BOX_W: f64 = 24.0;
/// 端子箱の高さ (mm)。
const BOX_H: f64 = 7.0;
/// 端子の縦ピッチ (mm)。箱の高さ+隙間。
const TERM_PITCH: f64 = 10.0;
/// 端子番号の文字高さ (mm)。
const NUMBER_FONT: f64 = 3.0;
/// 引出線の長さ (端子箱の縁から) mm。
const LEAD_LEN: f64 = 34.0;
/// 接続先表記の文字高さ (mm)。
const LABEL_FONT: f64 = 2.6;
/// 電線仕様・注記の文字高さ (mm)。
const NOTE_FONT: f64 = 2.0;
/// 引出線から電線仕様のベースラインまで (mm、線の下)。
const NOTE_DY: f64 = 2.6;
/// 引出線の外端と接続先表記の間に空ける帯 (mm)。ブラケットとハーネス名がここに入る。
const HARNESS_ZONE: f64 = 20.0;
/// ブラケットの腕 (端子箱側へ伸びる短い横線) の長さ (mm)。
const BRACKET_ARM: f64 = 2.5;
/// ジャンパの縦線を端子箱の内部側の縁からどれだけ離すか (mm)。
const JUMPER_OFFSET: f64 = 3.0;
/// 見出し (外部側/内部側) の文字高さ (mm)。
const CAPTION_FONT: f64 = 2.8;
/// 本文上端から最初の端子の中心まで (mm)。見出しの分を空ける。
const CAPTION_GAP: f64 = 8.0;
/// 予備端子の箱の塗り。
const SPARE_FILL: &str = "#f0f0f0";
/// 結線済みの端子の箱の塗り。
const BOX_FILL: &str = "#ffffff";
/// 補助的な注記 (予備・ジャンパ名・ハーネス名) の文字色。
const NOTE_FILL: &str = "#555555";
/// 外部側 (紙の左) の見出し。
pub const OUTSIDE_CAPTION: &str = "外部側 (盤外)";
/// 内部側 (紙の右) の見出し。
pub const INSIDE_CAPTION: &str = "内部側 (盤内)";
/// 予備端子に添える注記。
pub const SPARE_NOTE: &str = "(予備)";
/// ジャンパに添える注記。
pub const JUMPER_NOTE: &str = "サドルジャンパ";

/// 端子ストリップの中心X (mm)。
fn center_x() -> f64 {
    REPORT_PAPER.dimensions_mm().0 / 2.0
}

/// 端子箱の左端X / 右端X (mm)。
fn box_left() -> f64 {
    center_x() - BOX_W / 2.0
}
fn box_right() -> f64 {
    center_x() + BOX_W / 2.0
}

/// ページ内k番目 (0始まり) の端子の中心Y (mm)。
fn row_y(k: usize) -> f64 {
    table_top() + CAPTION_GAP + k as f64 * TERM_PITCH
}

/// 1ページに描ける端子の数。用紙の高さと端子ピッチから決まる固定値。
pub fn terminals_per_page() -> usize {
    let usable = body_bottom() - (row_y(0) - BOX_H / 2.0);
    ((usable / TERM_PITCH).floor() as usize).max(1)
}

/// 図の左右。外部側が左、内部側が右。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    /// 外部側 (盤外)。紙の左。
    Outside,
    /// 内部側 (盤内)。紙の右。
    Inside,
}

impl Side {
    /// 左なら-1、右なら+1。
    fn dir(self) -> f64 {
        match self {
            Side::Outside => -1.0,
            Side::Inside => 1.0,
        }
    }

    /// 端子箱のこちら側の縁のX。
    fn box_edge(self) -> f64 {
        match self {
            Side::Outside => box_left(),
            Side::Inside => box_right(),
        }
    }

    /// 引出線の外端X (ブラケットもここに立つ)。
    fn lead_end(self) -> f64 {
        self.box_edge() + self.dir() * LEAD_LEN
    }

    /// 接続先表記の基準X。
    fn label_x(self) -> f64 {
        self.lead_end() + self.dir() * HARNESS_ZONE
    }

    /// 接続先表記・見出しの揃え。
    fn anchor(self) -> &'static str {
        match self {
            Side::Outside => "end",
            Side::Inside => "start",
        }
    }

    /// 接続先表記に使える幅 (mm)。
    fn label_width(self) -> f64 {
        match self {
            Side::Outside => self.label_x() - FRAME_MARGIN - CELL_PAD,
            Side::Inside => REPORT_PAPER.dimensions_mm().0 - FRAME_MARGIN - CELL_PAD - self.label_x(),
        }
    }

    /// この側の接続先表記。
    fn target(self, row: &TerminalRow) -> &str {
        match self {
            Side::Outside => &row.external,
            Side::Inside => &row.internal,
        }
    }

    /// この側の電線の仕様。
    fn wire(self, row: &TerminalRow) -> &str {
        match self {
            Side::Outside => &row.external_wire,
            Side::Inside => &row.internal_wire,
        }
    }

    /// この側の電線が属するハーネス名。
    fn harness(self, row: &TerminalRow) -> &str {
        match self {
            Side::Outside => &row.external_harness,
            Side::Inside => &row.internal_harness,
        }
    }

    /// この側に電線が繋がっているか (引出線を描くか)。
    fn wired(self, row: &TerminalRow) -> bool {
        !self.wire(row).is_empty() || !self.target(row).is_empty()
    }

    fn caption(self) -> &'static str {
        match self {
            Side::Outside => OUTSIDE_CAPTION,
            Side::Inside => INSIDE_CAPTION,
        }
    }
}

/// 端子台1つの端子接続図。**1端子台=1ページ**で、端子が1ページに収まらないときだけ
/// [`terminals_per_page`] ごとに分け、ページ表題に端子の範囲を書く。
/// `tb_id` が端子台シンボルでなければ1ページも返さない。
pub fn terminal_diagram_svg(project: &Project, tb_id: EntityId) -> Vec<String> {
    let Some(chart) = project
        .sheets
        .iter()
        .find_map(|s| terminal_chart(s, tb_id))
    else {
        return Vec::new();
    };
    let meta = ReportMeta::from_project(project);
    let base = format!("{} {}", ReportKind::TerminalDiagram.title(), chart.reference);
    let per_page = terminals_per_page();
    let chunks: Vec<&[TerminalRow]> = if chart.rows.is_empty() {
        vec![&[]]
    } else {
        chart.rows.chunks(per_page).collect()
    };
    let total = chunks.len();
    chunks
        .into_iter()
        .enumerate()
        .map(|(i, rows)| {
            let title = match (total, rows.first(), rows.last()) {
                (1, _, _) | (_, None, _) | (_, _, None) => base.clone(),
                (_, Some(first), Some(last)) => {
                    format!("{base} (端子 {}〜{})", first.terminal, last.terminal)
                }
            };
            let mut out = page_open(&title);
            draw_captions(&mut out);
            draw_strip(&mut out, rows);
            for side in [Side::Outside, Side::Inside] {
                draw_side(&mut out, rows, side);
                draw_brackets(&mut out, rows, side);
            }
            draw_jumpers(&mut out, rows, &chart.jumpers);
            report_title_block(&mut out, &title, &meta, i + 1, total);
            out.push_str("</svg>\n");
            out
        })
        .collect()
}

/// プロジェクト内の全端子台の端子接続図 (シート順→参照記号順)。
/// 端子台が1つも無ければ、見出しだけの空のページを1枚返す。
pub fn terminal_diagrams_svg(project: &Project) -> Vec<String> {
    let ids = terminal_block_ids(project);
    if ids.is_empty() {
        let mut out = page_open(ReportKind::TerminalDiagram.title());
        draw_captions(&mut out);
        report_title_block(
            &mut out,
            ReportKind::TerminalDiagram.title(),
            &ReportMeta::from_project(project),
            1,
            1,
        );
        out.push_str("</svg>\n");
        return vec![out];
    }
    ids.into_iter()
        .flat_map(|id| terminal_diagram_svg(project, id))
        .collect()
}

/// 左右の見出し (外部側 / 内部側)。
fn draw_captions(out: &mut String) {
    for side in [Side::Outside, Side::Inside] {
        text_el(
            out,
            side.label_x(),
            table_top() + CAPTION_FONT,
            CAPTION_FONT,
            "#000",
            side.anchor(),
            side.caption(),
        );
    }
}

/// 端子ストリップ (端子番号入りの箱を縦に積む)。予備端子は薄塗り+注記。
fn draw_strip(out: &mut String, rows: &[TerminalRow]) {
    for (k, row) in rows.iter().enumerate() {
        let y = row_y(k);
        rect_el(
            out,
            box_left(),
            y - BOX_H / 2.0,
            BOX_W,
            BOX_H,
            if row.spare { SPARE_FILL } else { BOX_FILL },
            "#000",
            FRAME_STROKE,
        );
        text_el(
            out,
            center_x(),
            y + NUMBER_FONT * 0.35,
            NUMBER_FONT,
            "#000",
            "middle",
            &row.terminal,
        );
        if row.spare {
            text_el(
                out,
                box_left() - CELL_PAD,
                y + NOTE_FONT * 0.35,
                NOTE_FONT,
                NOTE_FILL,
                "end",
                SPARE_NOTE,
            );
        }
    }
}

/// 片側の引出線・接続先表記・電線仕様。
fn draw_side(out: &mut String, rows: &[TerminalRow], side: Side) {
    for (k, row) in rows.iter().enumerate() {
        if !side.wired(row) {
            continue;
        }
        let y = row_y(k);
        line_el(out, side.box_edge(), y, side.lead_end(), y, FRAME_STROKE, "#000");
        let target = side.target(row);
        if !target.is_empty() {
            text_el(
                out,
                side.label_x(),
                y + LABEL_FONT * 0.35,
                LABEL_FONT,
                "#000",
                side.anchor(),
                &fit_text(target, side.label_width(), LABEL_FONT),
            );
        }
        let wire = side.wire(row);
        if !wire.is_empty() {
            text_el(
                out,
                side.lead_end() + side.dir() * -CELL_PAD,
                y + NOTE_DY,
                NOTE_FONT,
                "#000",
                match side {
                    Side::Outside => "start",
                    Side::Inside => "end",
                },
                &fit_text(wire, LEAD_LEN - CELL_PAD * 2.0, NOTE_FONT),
            );
        }
    }
}

/// 片側のハーネスのブラケット。**同じハーネス名の電線を1つの角括弧にまとめ**、
/// ページ内の最初の端子から最後の端子までを括ってハーネス名を書く。
/// 1本だけのハーネスは括弧を描かず名前だけを添え、ハーネスに属さない電線には何も付けない。
fn draw_brackets(out: &mut String, rows: &[TerminalRow], side: Side) {
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
    for (k, row) in rows.iter().enumerate() {
        let name = side.harness(row);
        if name.is_empty() || !side.wired(row) {
            continue;
        }
        match groups.iter_mut().find(|(n, _)| n == name) {
            Some((_, members)) => members.push(k),
            None => groups.push((name.to_string(), vec![k])),
        }
    }
    let x = side.lead_end();
    for (name, members) in groups {
        let (top, bottom) = (row_y(members[0]), row_y(members[members.len() - 1]));
        if members.len() > 1 {
            line_el(out, x, top, x, bottom, FRAME_STROKE, "#000");
            for y in [top, bottom] {
                line_el(out, x, y, x + side.dir() * -BRACKET_ARM, y, FRAME_STROKE, "#000");
            }
        }
        text_el(
            out,
            x + side.dir() * CELL_PAD,
            (top + bottom) / 2.0 + NOTE_FONT * 0.35,
            NOTE_FONT,
            NOTE_FILL,
            side.anchor(),
            &fit_text(&name, HARNESS_ZONE - CELL_PAD * 2.0, NOTE_FONT),
        );
    }
}

/// サドルジャンパ。端子箱の**内部側の縁**に縦線で描く (両端に短い横線)。
/// このページに両方の端子が載っているジャンパだけを描き、注記はページに1回だけ添える。
fn draw_jumpers(out: &mut String, rows: &[TerminalRow], jumpers: &[(u32, u32)]) {
    let index = |no: u32| {
        rows.iter()
            .position(|r| r.terminal.parse::<u32>().ok() == Some(no))
    };
    let x = box_right() + JUMPER_OFFSET;
    let mut noted = false;
    for (a, b) in jumpers {
        let (Some(ka), Some(kb)) = (index(*a), index(*b)) else {
            continue;
        };
        let (ya, yb) = (row_y(ka), row_y(kb));
        line_el(out, x, ya, x, yb, FRAME_STROKE, "#000");
        for y in [ya, yb] {
            line_el(out, box_right(), y, x, y, FRAME_STROKE, "#000");
        }
        if !noted {
            text_el(
                out,
                x + CELL_PAD,
                (ya + yb) / 2.0 + NOTE_FONT * 0.35,
                NOTE_FONT,
                NOTE_FILL,
                "start",
                JUMPER_NOTE,
            );
            noted = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{Command, Engine};
    use crate::geometry::Point;
    use crate::model::*;
    use uuid::Uuid;

    /// タグの属性値。
    fn attr(tag: &str, name: &str) -> String {
        let key = format!("{name}=\"");
        let i = tag.find(&key).unwrap_or_else(|| panic!("{name} in {tag}")) + key.len();
        let rest = &tag[i..];
        rest[..rest.find('"').expect("属性の終端")].to_string()
    }

    fn num(tag: &str, name: &str) -> f64 {
        attr(tag, name).parse().expect("数値")
    }

    /// 描画テキストを (x, y, 内容) で取り出す。
    fn texts(svg: &str) -> Vec<(f64, f64, String)> {
        svg.split("<text")
            .skip(1)
            .filter_map(|s| {
                let (tag, rest) = s.split_once('>')?;
                let body = rest.split_once("</text>")?.0.to_string();
                Some((num(tag, "x"), num(tag, "y"), body))
            })
            .collect()
    }

    /// 直線を (x1, y1, x2, y2) で取り出す。
    fn lines(svg: &str) -> Vec<(f64, f64, f64, f64)> {
        svg.split("<line ")
            .skip(1)
            .filter_map(|s| {
                let tag = s.split_once("/>")?.0;
                Some((num(tag, "x1"), num(tag, "y1"), num(tag, "x2"), num(tag, "y2")))
            })
            .collect()
    }

    /// 矩形を (x, y, 幅, 高さ, 塗り) で取り出す。
    fn rects(svg: &str) -> Vec<(f64, f64, f64, f64, String)> {
        svg.split("<rect ")
            .skip(1)
            .filter_map(|s| {
                let tag = s.split_once("/>")?.0;
                Some((
                    num(tag, "x"),
                    num(tag, "y"),
                    num(tag, "width"),
                    num(tag, "height"),
                    attr(tag, "fill"),
                ))
            })
            .collect()
    }

    /// 端子箱 (幅がBOX_Wの矩形) だけを上から順に。
    fn boxes(svg: &str) -> Vec<(f64, f64, f64, f64, String)> {
        let mut v: Vec<_> = rects(svg)
            .into_iter()
            .filter(|r| (r.2 - BOX_W).abs() < 1e-9)
            .collect();
        v.sort_by(|a, b| a.1.partial_cmp(&b.1).expect("y"));
        v
    }

    /// ストリップの範囲にある縦線 (ブラケット・ジャンパ)。
    fn vertical_lines(svg: &str) -> Vec<(f64, f64, f64, f64)> {
        lines(svg)
            .into_iter()
            .filter(|l| (l.0 - l.2).abs() < 1e-9 && l.1 < body_bottom() && l.3 < body_bottom())
            .collect()
    }

    fn text_at(svg: &str, body: &str) -> (f64, f64) {
        let hit: Vec<_> = texts(svg).into_iter().filter(|t| t.2 == body).collect();
        assert_eq!(hit.len(), 1, "「{body}」が1つだけあるはず: {hit:?}");
        (hit[0].0, hit[0].1)
    }

    fn symbol(symbol_id: &str, reference: &str, x: f64, y: f64) -> Entity {
        Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: symbol_id.into(),
            at: Point::new(x, y),
            rotation: 0,
            mirror: false,
            reference: reference.into(),
            value: String::new(),
            attrs: Default::default(),
        })
    }

    fn wire(a: (f64, f64), b: (f64, f64), color: &str, sq: f64, part: Option<&str>) -> Entity {
        Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(a.0, a.1), Point::new(b.0, b.1)],
            color: color.into(),
            sq,
            length_m: None,
            part_no: part.map(str::to_string),
            net: None,
        })
    }

    /// 6極端子台TB1 (150,100・ジャンパ1-2) を1枚のシートに置いたプロジェクト。
    ///
    /// 端子1・2・3・5の**外部側 (紙の右)** に RE1/RE2/RE3/RE5、端子1・2・5の
    /// **内部側 (紙の左)** に RI1/RI2/RI5 を繋ぐ。端子4・6は予備。
    /// 外部側の端子1〜3の電線だけをハーネスW10の囲みに入れる。
    fn demo() -> (Project, EntityId) {
        let mut engine = Engine::new(Project::new("端子接続図デモ"));
        let sheet_id = engine.project().sheets[0].id;
        let mut tb = SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "terminal_block_6p".into(),
            at: Point::new(150.0, 100.0),
            rotation: 0,
            mirror: false,
            reference: "TB1".into(),
            value: "BN 6P".into(),
            attrs: Default::default(),
        };
        tb.attrs.insert("jumpers".into(), "1-2".into());
        let tb_id = tb.id;
        let mut entities = vec![Entity::Symbol(tb)];
        // 端子i (1..=6) の中心Y
        let ty = |i: usize| 100.0 + (i as f64 - 1.0 - 2.5) * 5.0;
        for i in [1usize, 2, 3, 5] {
            entities.push(symbol("resistor", &format!("RE{i}"), 177.5, ty(i)));
            let (color, sq, part) = match i {
                1 => ("red", 0.75, Some("KIV0.75R")),
                2 => ("black", 0.75, None),
                3 => ("white", 1.25, None),
                _ => ("blue", 2.0, None),
            };
            entities.push(wire((152.5, ty(i)), (170.0, ty(i)), color, sq, part));
        }
        for i in [1usize, 2, 5] {
            entities.push(symbol("resistor", &format!("RI{i}"), 132.5, ty(i)));
            entities.push(wire((140.0, ty(i)), (147.5, ty(i)), "green", 0.5, None));
        }
        entities.push(Entity::Harness(Harness {
            id: Uuid::new_v4(),
            points: crate::harness::rect_points(
                Point::new(151.0, 85.0),
                Point::new(171.0, 99.0),
            ),
            name: "W10".into(),
            note: String::new(),
        }));
        for entity in entities {
            engine
                .execute(Command::AddEntity { sheet_id, entity })
                .expect("追加");
        }
        (engine.project().clone(), tb_id)
    }

    /// The terminal strip is drawn as one numbered box per terminal, stacked from top to bottom in terminal order.
    /// 端子ストリップは端子1個につき1つの番号入りの箱として、端子番号の順に上から下へ縦に積まれる。
    #[test]
    fn the_strip_stacks_one_numbered_box_per_terminal() {
        let (project, tb) = demo();
        let pages = terminal_diagram_svg(&project, tb);
        assert_eq!(pages.len(), 1, "6極は1ページ");
        let boxes = boxes(&pages[0]);
        assert_eq!(boxes.len(), 6, "端子6個");
        for (k, b) in boxes.iter().enumerate() {
            assert_eq!(b.0, box_left(), "箱は同じ左端に揃う");
            assert!((b.1 + BOX_H / 2.0 - row_y(k)).abs() < 1e-9, "{k}番目のY");
        }
        // 番号は箱の中央に、上から 1,2,…,6
        let numbers: Vec<String> = texts(&pages[0])
            .into_iter()
            .filter(|t| (t.0 - center_x()).abs() < 1e-9)
            .map(|t| t.2)
            .collect();
        assert_eq!(numbers, ["1", "2", "3", "4", "5", "6"]);
    }

    /// The outside of the panel is drawn to the left of the strip and the inside to the right, each under its own caption.
    /// 盤外 (外部側) はストリップの左、盤内 (内部側) は右に描かれ、それぞれ見出しが付く。
    #[test]
    fn the_outside_is_on_the_left_and_the_inside_on_the_right() {
        let (project, tb) = demo();
        let svg = &terminal_diagram_svg(&project, tb)[0];
        let (ext_x, _) = text_at(svg, "RE1:1");
        let (int_x, _) = text_at(svg, "RI1:2");
        assert!(ext_x < box_left(), "外部側の接続先は左: {ext_x}");
        assert!(int_x > box_right(), "内部側の接続先は右: {int_x}");
        assert!(text_at(svg, OUTSIDE_CAPTION).0 < center_x(), "見出しも左");
        assert!(text_at(svg, INSIDE_CAPTION).0 > center_x(), "見出しも右");
        // 引出線は端子箱の縁から外へ伸びる
        let leads = lines(svg);
        assert!(
            leads.iter().any(|l| l.0 == box_left() && l.2 == box_left() - LEAD_LEN),
            "外部側の引出線: {leads:?}"
        );
        assert!(
            leads.iter().any(|l| l.0 == box_right() && l.2 == box_right() + LEAD_LEN),
            "内部側の引出線: {leads:?}"
        );
    }

    /// A terminal with nothing wired to it stays in the strip as a lightly filled box marked as a spare.
    /// 何も繋がっていない端子は、薄く塗った箱に予備の注記を付けてストリップに残る。
    #[test]
    fn a_spare_terminal_stays_in_the_strip_lightly_filled() {
        let (project, tb) = demo();
        let svg = &terminal_diagram_svg(&project, tb)[0];
        let boxes = boxes(svg);
        assert_eq!(boxes[3].4, SPARE_FILL, "端子4は予備");
        assert_eq!(boxes[5].4, SPARE_FILL, "端子6は予備");
        assert_eq!(boxes[0].4, BOX_FILL, "端子1は結線済み");
        let spares: Vec<_> = texts(svg).into_iter().filter(|t| t.2 == SPARE_NOTE).collect();
        assert_eq!(spares.len(), 2, "予備の注記は2つ: {spares:?}");
        assert!(spares.iter().all(|t| t.0 < box_left()));
    }

    /// Wires of the same harness are gathered into one bracket at the outer end of their lead lines, labelled with the harness name.
    /// 同じハーネスに属する電線は引出線の外端で1つのブラケットにまとめられ、ハーネス名が添えられる。
    #[test]
    fn wires_of_one_harness_are_gathered_into_a_bracket() {
        let (project, tb) = demo();
        let svg = &terminal_diagram_svg(&project, tb)[0];
        let x = box_left() - LEAD_LEN;
        let bracket: Vec<_> = vertical_lines(svg)
            .into_iter()
            .filter(|l| (l.0 - x).abs() < 1e-9)
            .collect();
        assert_eq!(bracket.len(), 1, "ブラケットは1つ: {bracket:?}");
        assert_eq!((bracket[0].1, bracket[0].3), (row_y(0), row_y(2)), "端子1〜3を括る");
        let (nx, ny) = text_at(svg, "W10");
        assert!(nx < box_left(), "ハーネス名は外部側");
        assert!((ny - (row_y(0) + row_y(2)) / 2.0).abs() < 1.0, "ブラケットの中ほど");
    }

    /// A wire that belongs to no harness gets no bracket at all.
    /// どのハーネスにも属さない電線にはブラケットが付かない。
    #[test]
    fn a_wire_without_a_harness_gets_no_bracket() {
        let (project, tb) = demo();
        let svg = &terminal_diagram_svg(&project, tb)[0];
        // 外部側 (左) の縦線はW10のブラケット1本だけ。端子5の電線には何も付かない
        let left: Vec<_> = vertical_lines(svg)
            .into_iter()
            .filter(|l| l.0 < box_left())
            .collect();
        assert_eq!(left.len(), 1, "左の縦線: {left:?}");
        assert!(left[0].3 < row_y(4), "端子5までは括らない");
        // 内部側 (右) にはハーネスが無いのでブラケットも出ない
        let right_brackets: Vec<_> = vertical_lines(svg)
            .into_iter()
            .filter(|l| (l.0 - (box_right() + LEAD_LEN)).abs() < 1e-9)
            .collect();
        assert!(right_brackets.is_empty(), "内部側: {right_brackets:?}");
    }

    /// A saddle jumper between neighbouring terminals is drawn as a vertical link on the inside edge of the terminal boxes.
    /// 隣り合う端子のサドルジャンパは、端子箱の内部側の縁に縦の連結線として描かれる。
    #[test]
    fn a_jumper_is_drawn_on_the_inside_edge_of_the_boxes() {
        let (project, tb) = demo();
        let svg = &terminal_diagram_svg(&project, tb)[0];
        let x = box_right() + JUMPER_OFFSET;
        let jumper: Vec<_> = vertical_lines(svg)
            .into_iter()
            .filter(|l| (l.0 - x).abs() < 1e-9)
            .collect();
        assert_eq!(jumper.len(), 1, "ジャンパは1本: {jumper:?}");
        assert_eq!((jumper[0].1, jumper[0].3), (row_y(0), row_y(1)), "端子1-2を繋ぐ");
        assert!(x > box_right() && x < box_right() + LEAD_LEN, "内部側の縁に沿う");
        let (nx, ny) = text_at(svg, JUMPER_NOTE);
        assert!(nx > box_right(), "注記も内部側");
        assert!((ny - (row_y(0) + row_y(1)) / 2.0).abs() < 1.0);
    }

    /// Each lead line carries the wire it stands for: colour, gauge in sq and part number, written small under the line.
    /// 各引出線には、その電線の線色・線径sq・品番が線の下に小さく書かれる。
    #[test]
    fn each_lead_line_carries_the_wire_specification() {
        let (project, tb) = demo();
        let svg = &terminal_diagram_svg(&project, tb)[0];
        let (x, y) = text_at(svg, "red 0.75sq KIV0.75R");
        assert!(x < box_left(), "端子1の外部側の引出線に添う");
        assert!(y > row_y(0) && y < row_y(0) + TERM_PITCH / 2.0, "線の下に書く");
        assert_eq!(text_at(svg, "black 0.75sq").1, y + TERM_PITCH, "端子2は1つ下");
        // 内部側の電線も同じ書式で、内部側の引出線に添って載る
        let internal: Vec<_> = texts(svg).into_iter().filter(|t| t.2 == "green 0.5sq").collect();
        assert_eq!(internal.len(), 3, "内部側の3本: {internal:?}");
        assert!(internal.iter().all(|t| t.0 > box_right()), "{internal:?}");
    }

    /// Terminals that do not fit on one page continue on the next, and each page title states the range of terminals it holds.
    /// 1ページに収まらない端子は次のページへ続き、各ページの表題にそのページの端子の範囲が入る。
    #[test]
    fn terminals_that_do_not_fit_continue_on_the_next_page() {
        let per = terminals_per_page();
        let mut project = Project::new("大きい端子台");
        let tb = Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: format!("terminal_block_{}p", per + 5),
            at: Point::new(150.0, 100.0),
            rotation: 0,
            mirror: false,
            reference: "TB9".into(),
            value: String::new(),
            attrs: Default::default(),
        });
        let tb_id = tb.id();
        project.sheets[0].entities.insert(tb_id, tb);
        let pages = terminal_diagram_svg(&project, tb_id);
        assert_eq!(pages.len(), 2, "{}極 / 1ページ{per}端子", per + 5);
        assert_eq!(boxes(&pages[0]).len(), per, "1ページ目は満杯");
        assert_eq!(boxes(&pages[1]).len(), 5);
        let title = |svg: &str| {
            texts(svg)
                .into_iter()
                .find(|t| t.2.starts_with("端子接続図"))
                .expect("表題")
                .2
        };
        assert_eq!(title(&pages[0]), format!("端子接続図 TB9 (端子 1〜{per})"));
        assert_eq!(
            title(&pages[1]),
            format!("端子接続図 TB9 (端子 {}〜{})", per + 1, per + 5)
        );
        // 端子は全ページ通して1回ずつ描かれる
        let total: usize = pages.iter().map(|p| boxes(p).len()).sum();
        assert_eq!(total, per + 5);
    }

    /// Every terminal block gets exactly one page; anything that is not a terminal block gets none.
    /// 端子台1つにつき1ページが出て、端子台でないものには1ページも出ない。
    #[test]
    fn one_page_per_terminal_block_and_none_for_anything_else() {
        let (project, _) = demo();
        assert!(terminal_diagram_svg(&project, Uuid::new_v4()).is_empty(), "存在しないID");
        let resistor = project.sheets[0]
            .entities
            .values()
            .find_map(|e| match e {
                Entity::Symbol(s) if s.reference == "RE1" => Some(s.id),
                _ => None,
            })
            .expect("抵抗");
        assert!(terminal_diagram_svg(&project, resistor).is_empty(), "端子台以外");
        assert_eq!(terminal_diagrams_svg(&project).len(), 1, "端子台は1つ");
    }

    /// A terminal diagram page is an A4 landscape sheet with the JIS frame and a title block naming the terminal block.
    /// 端子接続図のページはA4横で、JIS図枠と、端子台の参照記号が入った表題欄を持つ。
    #[test]
    fn the_page_has_the_frame_and_a_title_block() {
        let (project, tb) = demo();
        let svg = &terminal_diagram_svg(&project, tb)[0];
        assert!(svg.contains("width=\"297mm\" height=\"210mm\""), "{svg}");
        assert!(
            svg.contains("<rect x=\"10\" y=\"10\" width=\"277\" height=\"190\""),
            "図枠"
        );
        let t: Vec<String> = texts(svg).into_iter().map(|t| t.2).collect();
        assert!(t.contains(&"端子接続図 TB1".to_string()), "{t:?}");
        assert!(t.contains(&"端子接続図デモ".to_string()), "プロジェクト名: {t:?}");
        assert!(
            t.iter().any(|s| s.contains("端子接続図 TB1") && s.contains("1/1")),
            "表題欄のページ番号: {t:?}"
        );
    }

    /// The terminal diagram is offered as a report named terminal-diagram, spelled the same way in the CLI, Link API and MCP.
    /// 端子接続図はterminal-diagramという名前の帳票として選べ、CLI・Link API・MCPで同じ綴りになる。
    #[test]
    fn the_terminal_diagram_is_a_report_named_terminal_diagram() {
        assert_eq!(
            serde_json::to_string(&ReportKind::TerminalDiagram).expect("JSON"),
            "\"terminal-diagram\""
        );
        let (project, _) = demo();
        let pages = ReportKind::TerminalDiagram.pages(&project);
        assert_eq!(pages.len(), 1);
        assert!(pages[0].contains("TB1"));
    }

    /// A project without any terminal block still yields one page, so the report is never empty.
    /// 端子台が1つも無いプロジェクトでも1ページは出るので、帳票が空になることはない。
    #[test]
    fn a_project_without_terminal_blocks_still_yields_one_page() {
        let project = Project::new("端子台なし");
        let pages = terminal_diagrams_svg(&project);
        assert_eq!(pages.len(), 1);
        let t: Vec<String> = texts(&pages[0]).into_iter().map(|t| t.2).collect();
        assert!(t.contains(&OUTSIDE_CAPTION.to_string()), "{t:?}");
    }
}
