//! 盤内配線図 (M4仕様 §8、.pen「M4残りデザイン⑤」`B2gfQZ`)。
//!
//! 盤レイアウトシートに置いた部品ごとに、**回路図**でその部品のピンに繋がっている電線を
//! 端子ごとに並べる: 端子 → 線番・行き先 (相手の`参照記号:ピン`かネットラベル)・線色/sq。
//! ACADEのパネル配線注記 (回路図の線番・線色・接続順をフットプリントの端子に注記) に相当する。
//!
//! 中身は毎回図面から導出する派生帳票で、編集はしない (線番・行き先を直すのは回路図)。
//! CSV ([`panel_wiring_csv`]) と図枠付きの図面ページ ([`panel_wiring_svg`]) の2様式。

use crate::model::{Entity, Project, SheetKind};
use crate::netlist::{on_wire, pin_positions};
use crate::report_sheet::{body_bottom, page_open, report_title_block, table_top, ReportKind, ReportMeta, REPORT_PAPER};
use crate::reports::{csv_row, endpoint_label};
use crate::svg::{line_el, rect_el, text_el, FRAME_MARGIN};
use crate::symbol::sheet_symbol_defs;

/// 盤内配線図 (CSV) の列見出し。
pub const PANEL_WIRING_COLUMNS: [&str; 8] =
    ["盤", "機器", "型番", "端子", "線番", "行き先", "線色", "線径sq"];

/// 端子1つに繋がる電線1本。
#[derive(Debug, Clone, PartialEq)]
pub struct WiringRow {
    pub pin: String,
    pub wire_no: String,
    pub dest: String,
    pub color: String,
    pub sq: f64,
}

/// 盤上の部品1つぶん。
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceWiring {
    /// 盤レイアウトシートの名前。
    pub panel: String,
    pub reference: String,
    pub part_no: String,
    pub rows: Vec<WiringRow>,
}

/// ピン番号の並べ方 (数字は数値順、それ以外は文字順で後ろ)。
fn pin_key(pin: &str) -> (u32, String) {
    (pin.parse::<u32>().unwrap_or(u32::MAX), pin.to_string())
}

/// 盤レイアウトシートごと・部品ごと (盤の読み順) の配線。回路図に無い部品は端子の行が無い。
pub fn panel_wiring(project: &Project) -> Vec<DeviceWiring> {
    let mut out = Vec::new();
    for panel in project.sheets.iter().filter(|s| s.kind == SheetKind::Panel) {
        for fp in crate::panel::item_order(panel) {
            let reference = fp.reference.trim();
            if reference.is_empty() {
                continue;
            }
            let mut rows: Vec<WiringRow> = Vec::new();
            for sheet in project.sheets.iter().filter(|s| s.kind != SheetKind::Panel) {
                let defs = sheet_symbol_defs(sheet);
                for e in sheet.entities.values() {
                    let Entity::Symbol(s) = e else { continue };
                    if s.reference.trim() != reference {
                        continue;
                    }
                    let Some(def) = defs.iter().find(|d| d.id == s.symbol_id) else { continue };
                    for (pin, at) in pin_positions(s, def) {
                        for w in sheet.entities.values().filter_map(|e| match e {
                            Entity::Wire(w) if on_wire(w, &at) => Some(w),
                            _ => None,
                        }) {
                            let spec = crate::cable::effective_wire(sheet, w);
                            let far = [w.points.first(), w.points.last()]
                                .into_iter()
                                .flatten()
                                .find(|p| p.distance_to(&at) >= crate::netlist::CONNECT_EPS);
                            rows.push(WiringRow {
                                pin: pin.clone(),
                                wire_no: w.net.clone().unwrap_or_default(),
                                dest: far.map(|p| endpoint_label(sheet, &defs, p)).unwrap_or_default(),
                                color: spec.color,
                                sq: spec.sq,
                            });
                        }
                    }
                }
            }
            rows.sort_by(|a, b| pin_key(&a.pin).cmp(&pin_key(&b.pin)).then(a.wire_no.cmp(&b.wire_no)));
            out.push(DeviceWiring {
                panel: panel.name.clone(),
                reference: reference.to_string(),
                part_no: fp.part_no.trim().to_string(),
                rows,
            });
        }
    }
    out
}

fn sq_text(sq: f64) -> String {
    if sq > 0.0 {
        let s = format!("{sq:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        String::new()
    }
}

/// 盤内配線図のCSV (見出しは [`PANEL_WIRING_COLUMNS`]、端子1本の電線=1行)。
pub fn panel_wiring_csv(project: &Project) -> String {
    let mut out = PANEL_WIRING_COLUMNS.join(",");
    out.push('\n');
    for d in panel_wiring(project) {
        for r in &d.rows {
            out.push_str(&csv_row(&[
                d.panel.clone(),
                d.reference.clone(),
                d.part_no.clone(),
                r.pin.clone(),
                r.wire_no.clone(),
                r.dest.clone(),
                r.color.clone(),
                sq_text(r.sq),
            ]));
            out.push('\n');
        }
    }
    out
}

/// 部品の箱の幅 (mm)。
const BOX_W: f64 = 24.0;
/// 端子1行の高さ (mm)。
const ROW_H: f64 = 6.0;
/// 引出線の長さ (mm)。
const LEAD: f64 = 10.0;
/// 文字の高さ (mm)。
const FONT: f64 = 2.5;
/// 部品の見出しの文字高さ (mm)。
const HEAD_FONT: f64 = 3.0;
/// 部品どうしの縦の隙間 (mm)。
const GAP: f64 = 6.0;
/// 列の数 (A4横に2列)。
const COLUMNS: usize = 2;

/// 盤内配線図の図面ページ。部品を2列に流し込み、入りきらなければ次のページへ送る。
/// 盤に部品が無ければ見出しだけの空のページを1枚返す。
pub fn panel_wiring_svg(project: &Project) -> Vec<String> {
    let meta = ReportMeta::from_project(project);
    let title = ReportKind::PanelWiring.title();
    let devices = panel_wiring(project);
    let (pw, _) = REPORT_PAPER.dimensions_mm();
    let col_w = (pw - 2.0 * FRAME_MARGIN) / COLUMNS as f64;
    let (top, bottom) = (table_top(), body_bottom());
    // ページ割り: (ページ番号, 列, 上端y) を部品ごとに決める
    let mut placed: Vec<(usize, usize, f64)> = Vec::new();
    let (mut page, mut col, mut y) = (0usize, 0usize, top);
    for d in &devices {
        let h = HEAD_FONT + 2.0 + (d.rows.len().max(1) as f64) * ROW_H;
        if y + h > bottom && y > top {
            col += 1;
            y = top;
            if col == COLUMNS {
                col = 0;
                page += 1;
            }
        }
        placed.push((page, col, y));
        y += h + GAP;
    }
    let pages = placed.last().map_or(1, |p| p.0 + 1);
    (0..pages)
        .map(|p| {
            let mut out = page_open(title);
            for (d, (_, c, y)) in devices.iter().zip(&placed).filter(|(_, (pg, _, _))| *pg == p) {
                let x = FRAME_MARGIN + 6.0 + *c as f64 * col_w;
                let head = if d.part_no.is_empty() {
                    d.reference.clone()
                } else {
                    format!("{}  {}", d.reference, d.part_no)
                };
                text_el(&mut out, x, y + HEAD_FONT, HEAD_FONT, "#000", "start", &head);
                let box_top = y + HEAD_FONT + 2.0;
                let rows = d.rows.len().max(1) as f64;
                rect_el(&mut out, x, box_top, BOX_W, rows * ROW_H, "#fff", "#000", 0.35);
                for (i, r) in d.rows.iter().enumerate() {
                    let cy = box_top + (i as f64 + 0.5) * ROW_H;
                    let edge = x + BOX_W;
                    text_el(&mut out, edge - 1.5, cy + FONT * 0.35, FONT, "#000", "end", &r.pin);
                    line_el(&mut out, edge, cy, edge + LEAD, cy, 0.25, "#000");
                    let spec = [r.color.as_str(), &sq_text(r.sq)].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
                    let label = [r.wire_no.as_str(), &format!("→{}", r.dest), spec.as_str()]
                        .into_iter()
                        .filter(|s| !s.is_empty() && *s != "→")
                        .collect::<Vec<_>>()
                        .join("  ");
                    text_el(&mut out, edge + LEAD + 1.5, cy + FONT * 0.35, FONT, "#000", "start", &label);
                }
            }
            report_title_block(&mut out, title, &meta, p + 1, pages);
            out.push_str("</svg>\n");
            out
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::*;
    use uuid::Uuid;

    /// 回路図にK1 (抵抗: ピン1=(92.5,50)、ピン2=(107.5,50)) と F1 を置き、K1:2→F1:1 を線番101・黒1.25sqで結ぶ。
    /// 盤シートにはK1 (型番 SAMPLE-K) のフットプリントだけを置く。
    fn project() -> Project {
        let mut p = Project::new("盤内配線");
        let sch = &mut p.sheets[0];
        for (r, x) in [("K1", 100.0), ("F1", 130.0)] {
            let s = Entity::Symbol(SymbolInstance {
                id: Uuid::new_v4(),
                symbol_id: "resistor".into(),
                at: Point::new(x, 50.0),
                rotation: 0,
                mirror: false,
                reference: r.into(),
                value: String::new(),
                attrs: Default::default(),
            });
            sch.entities.insert(s.id(), s);
        }
        let w = Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(107.5, 50.0), Point::new(122.5, 50.0)],
            color: "black".into(),
            sq: 1.25,
            length_m: None,
            length_source: Default::default(),
            part_no: None,
            net: Some("101".into()),
        });
        sch.entities.insert(w.id(), w);
        let mut panel = Sheet::new_kind("盤 CP-1", PaperSize::A3, Orientation::Landscape, SheetKind::Panel);
        let fp = Entity::Footprint(Footprint {
            id: Uuid::new_v4(),
            at: Point::new(100.0, 100.0),
            w: 20.0,
            h: 40.0,
            rotation: 0,
            reference: "K1".into(),
            part_no: "SAMPLE-K".into(),
            item_no: None,
        });
        panel.entities.insert(fp.id(), fp);
        p.sheets.push(panel);
        p
    }

    /// The panel wiring lists, for each part on a panel sheet, every wire on its pins in the schematic: pin, wire number, the other end, colour and gauge.
    /// 盤内配線図は、盤に置いた部品ごとに、回路図でそのピンに繋がる電線を端子・線番・行き先・線色・線径で並べる。
    #[test]
    fn the_panel_wiring_lists_each_pin_wire_with_its_destination() {
        let wiring = panel_wiring(&project());
        assert_eq!(wiring.len(), 1);
        let d = &wiring[0];
        assert_eq!((d.panel.as_str(), d.reference.as_str(), d.part_no.as_str()), ("盤 CP-1", "K1", "SAMPLE-K"));
        assert_eq!(
            d.rows,
            vec![WiringRow { pin: "2".into(), wire_no: "101".into(), dest: "F1:1".into(), color: "black".into(), sq: 1.25 }]
        );
        let csv = panel_wiring_csv(&project());
        assert!(csv.starts_with(&PANEL_WIRING_COLUMNS.join(",")));
        assert!(csv.contains("盤 CP-1,K1,SAMPLE-K,2,101,F1:1,black,1.25\n"), "{csv}");
    }

    /// A run-cable breakout wire on a pin is listed with its core's colour and the cable's gauge, not blank.
    /// ピンに着く区間ケーブルの分岐の配線は、空欄ではなく芯の線色とケーブルの線径で並ぶ。
    #[test]
    fn a_breakout_wire_is_listed_with_the_core_colour_and_cable_gauge() {
        let mut p = project();
        let sch = &mut p.sheets[0];
        let wire = sch.entities.values().find_map(|e| if let Entity::Wire(w) = e { Some(w.id) } else { None }).unwrap();
        if let Some(Entity::Wire(w)) = sch.entities.get_mut(&wire) {
            w.color = String::new();
            w.sq = 0.0;
        }
        crate::cable::test_support::attach_breakout_core(sch, wire, "green", 0.2, "");
        let rows = &panel_wiring(&p)[0].rows;
        assert_eq!((rows[0].color.as_str(), rows[0].sq), ("green", 0.2));
    }

    /// The panel wiring sheet draws each part with its terminal rows, and is an empty heading page when no panel has parts.
    /// 盤内配線図の図面ページは部品ごとに端子の行を描き、盤に部品が無ければ見出しだけのページになる。
    #[test]
    fn the_panel_wiring_sheet_draws_parts_or_an_empty_page() {
        let pages = panel_wiring_svg(&project());
        assert_eq!(pages.len(), 1);
        assert!(pages[0].contains("K1  SAMPLE-K"));
        assert!(pages[0].contains("101  →F1:1  black 1.25"), "{}", pages[0]);
        let empty = panel_wiring_svg(&Project::new("空"));
        assert_eq!(empty.len(), 1);
    }
}
