//! 機能シートの下書きの検算 (function sheet draft check)。
//!
//! 元の図 (`source`) と、今開いているプロジェクト (`open`) の全シートを比べ、
//! 全部のシートを合わせて同じつながりを持っているかを調べる純関数。どちらも書き換えない。
//!
//! 両側を同じ関数 ([`crate::draft::device_nets`]) で減らす: 全シートのプロジェクトネットリストから、
//! ネットの印と貫通のピンを除き、`ignore_references` の機器のピンを除く。元の図のネットのうち
//! 機器のピンが2本に満たないものは数えない。ネットの名前は比べず、ピンの組み分けを比べる。
//!
//! 仕様: `docs/superpowers/specs/2026-11-18-function-sheet-draft-design.md` §4。
use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::draft::{device_nets, natural_cmp};
use crate::model::{Entity, Project};

/// 元の図にあるのに開いたプロジェクトに無いピン (`missing_pins`)、または開いたプロジェクトにだけあるピン (`extra_pins`)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct CheckPin {
    pub reference: String,
    pub pin: String,
    /// 元の図でそのピンがあるネットの名前。`extra_pins` では空。
    pub source_net: String,
}

/// 元の図の1本のネットが、開いたプロジェクトでは2本以上に分かれている。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct CheckSplit {
    pub source_net: String,
    /// 開いたプロジェクトでの組。`"REF:PIN"` の文字列を自然順に並べ、組は先頭の文字列の自然順に並べる。
    pub parts: Vec<Vec<String>>,
}

/// 元の図の別々のネットのピンが、開いたプロジェクトでは1本のネットになっている。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct CheckMerged {
    /// 混ざった元のネットの名前 (自然順)。
    pub source_nets: Vec<String>,
    /// 開いたプロジェクトのそのネットの全ピン (`"REF:PIN"`、自然順)。
    pub pins: Vec<String>,
}

/// 検算の結果。
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct DraftCheck {
    /// 4つの一覧がどれも空なら true。
    pub ok: bool,
    /// 数えた元のネットの数 (機器のピンが2本以上残るもの)。
    pub source_nets: usize,
    /// 抜けも分かれも混ざりも無い元のネットの数。
    pub matched: usize,
    pub missing_pins: Vec<CheckPin>,
    pub extra_pins: Vec<CheckPin>,
    pub split: Vec<CheckSplit>,
    pub merged: Vec<CheckMerged>,
    /// 図枠の外・表題欄の中へはみ出す札の旗。
    pub tags_outside: Vec<CheckTagOutside>,
    /// `tags_outside` が空なら true (`ok` には影響しない)。
    pub layout_ok: bool,
}

/// 開いたプロジェクトで、旗が図枠の内側の外・表題欄の中へはみ出している札 (旗は書き出しと同じ行き先の文字で測る)。
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct CheckTagOutside {
    /// シートの名前。
    pub sheet: String,
    /// 札のネット名。
    pub net: String,
    /// いちばん大きくはみ出した量 (mm、小数1桁)。
    pub excess_mm: f64,
}

type Pin = (String, String);

fn pin_cmp(a: &Pin, b: &Pin) -> std::cmp::Ordering {
    natural_cmp(&a.0, &b.0).then_with(|| natural_cmp(&a.1, &b.1))
}

fn label(p: &Pin) -> String {
    format!("{}:{}", p.0, p.1)
}

fn sorted_pins(pins: impl IntoIterator<Item = Pin>) -> Vec<Pin> {
    let mut v: Vec<Pin> = pins.into_iter().collect();
    v.sort_by(pin_cmp);
    v
}

/// 元の図 `source` と開いたプロジェクト `open` の全シートが、同じピンのつながりを持つかを調べる。何も書き換えない。
///
/// - 両側とも、ネットの印・貫通のピンと `ignore_references` の機器のピンを除いて比べる。ネット名は比べない。
/// - 元のネットは機器のピンが2本以上残るものだけを数える (`source_nets`)。1本だけのネットのピンは「無い」とは言わない。
/// - `missing_pins`: 数えた元のネットにあるピンのうち、開いたプロジェクトのどのシートにも描かれていないもの。
///   描いてあるのにどこにもつながっていないピンは「ある、ただし1本だけ」と数え、`missing_pins` ではなく `split` に出る。
/// - `extra_pins`: 開いたプロジェクトのネットにあり、元のどのネット (数えないネットも含む) にも無いピン。
/// - `split`: 数えた元のネットの、ある方のピンが開いたプロジェクトで2本以上のネットに分かれている。
/// - `merged`: 開いたプロジェクトの1本のネットが、元の2本以上のネットのピンを持っている。
/// - `matched`: 抜けず、分かれず、混ざりもしない元のネットの数。
///
/// 一覧はどれも自然順 (`U2` は `U10` より前、ピン `9` は `10` より前)。
pub fn check_function_sheets(source: &Project, open: &Project, ignore_references: &[String]) -> DraftCheck {
    let ignored: BTreeSet<&str> = ignore_references.iter().map(|r| r.trim()).collect();
    let keep = |p: &Pin| !ignored.contains(p.0.as_str());

    // 元: 名前と、残ったピン。機器のピンが2本以上のものを数える。
    let src = device_nets(source);
    let mut all_source_pins: BTreeSet<Pin> = BTreeSet::new();
    let mut counted: Vec<(String, Vec<Pin>)> = Vec::new();
    for (name, pins) in &src.nets {
        let pins: Vec<Pin> = sorted_pins(pins.iter().filter(|p| keep(p)).cloned());
        all_source_pins.extend(pins.iter().cloned());
        if pins.len() >= 2 {
            counted.push((name.clone(), pins));
        }
    }
    counted.sort_by(|a, b| natural_cmp(&a.0, &b.0));

    // 開いた側: ピン → 載っているネットの番号 (複数に載るピンは最初のもの)。
    let dst = device_nets(open);
    let open_nets: Vec<Vec<Pin>> = dst
        .nets
        .iter()
        .map(|(_, pins)| sorted_pins(pins.iter().filter(|p| keep(p)).cloned()))
        .collect();
    let mut open_net_of: BTreeMap<Pin, usize> = BTreeMap::new();
    for (i, pins) in open_nets.iter().enumerate() {
        for p in pins {
            open_net_of.entry(p.clone()).or_insert(i);
        }
    }

    // 数えた元のネットの番号 (ピン → 元のネット、複数に載るピンは最初のもの)
    let mut source_net_of: BTreeMap<Pin, usize> = BTreeMap::new();
    for (i, (_, pins)) in counted.iter().enumerate() {
        for p in pins {
            source_net_of.entry(p.clone()).or_insert(i);
        }
    }

    let mut missing_pins: Vec<CheckPin> = Vec::new();
    let mut split: Vec<CheckSplit> = Vec::new();
    let mut bad: BTreeSet<usize> = BTreeSet::new();
    for (i, (name, pins)) in counted.iter().enumerate() {
        // 開いた側にあるピンを、載っているネットで組にする。ネットに載らないピンは1本ずつの組。
        let mut groups: BTreeMap<(u8, usize, Pin), Vec<Pin>> = BTreeMap::new();
        for p in pins {
            match open_net_of.get(p) {
                Some(&n) => groups.entry((0, n, (String::new(), String::new()))).or_default().push(p.clone()),
                None if dst.drawn.contains(p) => groups.entry((1, 0, p.clone())).or_default().push(p.clone()),
                None => {
                    missing_pins.push(CheckPin { reference: p.0.clone(), pin: p.1.clone(), source_net: name.clone() });
                    bad.insert(i);
                }
            }
        }
        if groups.len() >= 2 {
            let mut parts: Vec<Vec<String>> =
                groups.into_values().map(|g| sorted_pins(g).iter().map(label).collect()).collect();
            parts.sort_by(|a, b| natural_cmp(&a[0], &b[0]));
            split.push(CheckSplit { source_net: name.clone(), parts });
            bad.insert(i);
        }
    }
    missing_pins.sort_by(|a, b| {
        natural_cmp(&a.reference, &b.reference)
            .then_with(|| natural_cmp(&a.pin, &b.pin))
            .then_with(|| natural_cmp(&a.source_net, &b.source_net))
    });
    split.sort_by(|a, b| natural_cmp(&a.source_net, &b.source_net));

    // 開いた側の1本のネットが元の2本以上のネットのピンを持つ
    let mut merged: Vec<CheckMerged> = Vec::new();
    for pins in &open_nets {
        let from: BTreeSet<usize> = pins.iter().filter_map(|p| source_net_of.get(p).copied()).collect();
        if from.len() >= 2 {
            let mut names: Vec<String> = from.iter().map(|&i| counted[i].0.clone()).collect();
            names.sort_by(|a, b| natural_cmp(a, b));
            merged.push(CheckMerged { source_nets: names, pins: pins.iter().map(label).collect() });
            bad.extend(from);
        }
    }
    merged.sort_by(|a, b| natural_cmp(&a.source_nets[0], &b.source_nets[0]).then_with(|| natural_cmp(&a.pins[0], &b.pins[0])));

    // 元のどのネットにも無いピン
    let extra_pins: Vec<CheckPin> = sorted_pins(open_net_of.keys().filter(|p| !all_source_pins.contains(*p)).cloned())
        .into_iter()
        .map(|p| CheckPin { reference: p.0, pin: p.1, source_net: String::new() })
        .collect();

    let tags_outside = tags_outside_the_frame(open);
    DraftCheck {
        ok: missing_pins.is_empty() && extra_pins.is_empty() && split.is_empty() && merged.is_empty(),
        source_nets: counted.len(),
        matched: counted.len() - bad.len(),
        missing_pins,
        extra_pins,
        split,
        merged,
        layout_ok: tags_outside.is_empty(),
        tags_outside,
    }
}

/// 開いたプロジェクトの全ての札について、書き出しと同じ旗 (行き先は [`crate::xref::sheet_tag_dests`] か札自身の
/// `tag_dest`、形は [`crate::net_tag::net_tag_with`]) が、図枠の内側の外・表題欄 (改訂欄を含む) の中へはみ出すかを調べる。
/// シートの並び順、ネット名の自然順に並べる。
fn tags_outside_the_frame(open: &Project) -> Vec<CheckTagOutside> {
    let template = crate::frame_template::for_project(open);
    let mut found: Vec<(usize, CheckTagOutside, (f64, f64))> = Vec::new();
    for (index, sheet) in open.sheets.iter().enumerate() {
        let (frame, title) = crate::arrange::frame_areas(&template, sheet);
        let dests = crate::xref::sheet_tag_dests(open, sheet.id);
        let font = crate::svg::LABEL_FONT * sheet.annotation_scale();
        for entity in sheet.entities.values() {
            let Entity::NetLabel(l) = entity else { continue };
            let dest = dests.get(l.name.trim()).map(String::as_str).or_else(|| crate::net_tag::tag_dest(l));
            let Some(tag) = crate::net_tag::net_tag_with(l, font, sheet.xref_style, dest) else { continue };
            let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
            for p in &tag.points {
                x0 = x0.min(p.x);
                y0 = y0.min(p.y);
                x1 = x1.max(p.x);
                y1 = y1.max(p.y);
            }
            let mut excess = (frame.0 - x0).max(x1 - frame.2).max(frame.1 - y0).max(y1 - frame.3);
            // 表題欄の中へ入り込む量: 旗を表題欄の外へ押し出すのに要る最小の距離
            let (dx, dy) = ((x1.min(title.2) - x0.max(title.0)), (y1.min(title.3) - y0.max(title.1)));
            if dx > 1e-9 && dy > 1e-9 {
                let out = (x1 - title.0).min(title.2 - x0).min(y1 - title.1).min(title.3 - y0);
                excess = excess.max(out);
            }
            if excess > 1e-6 {
                let excess_mm = ((excess * 10.0) - 1e-9).ceil() / 10.0;
                found.push((index, CheckTagOutside { sheet: sheet.name.clone(), net: l.name.trim().to_string(), excess_mm }, (l.at.x, l.at.y)));
            }
        }
    }
    found.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| natural_cmp(&a.1.net, &b.1.net))
            .then_with(|| b.1.excess_mm.total_cmp(&a.1.excess_mm))
            .then_with(|| a.2 .0.total_cmp(&b.2 .0))
            .then_with(|| a.2 .1.total_cmp(&b.2 .1))
    });
    found.into_iter().map(|(_, t, _)| t).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft::{fixture_source, plan_function_sheet, DraftPaper, DraftPlan, DraftRequest};
    use crate::model::Entity;

    fn request(name: &str, columns: &[&[&str]], power: &[&str]) -> DraftRequest {
        DraftRequest {
            name: name.into(),
            columns: columns.iter().map(|c| c.iter().map(|r| r.to_string()).collect()).collect(),
            pins: BTreeMap::new(),
            power_nets: power.iter().map(|s| s.to_string()).collect(),
            ground_nets: None,
            destinations: BTreeMap::new(),
            paper: DraftPaper::A3,
            pitch_mm: 5.0,
            planned_sheets: Vec::new(),
            title_block: None,
        }
    }

    fn with_pins(mut req: DraftRequest, reference: &str, pins: &[&str]) -> DraftRequest {
        req.pins.insert(reference.into(), pins.iter().map(|p| p.to_string()).collect());
        req
    }

    /// An open project made only of the sheets the requests draft, one after the other.
    fn open_of(requests: &[DraftRequest]) -> (Project, Vec<DraftPlan>) {
        let source = fixture_source();
        let mut open = Project::new("open");
        open.sheets.clear();
        let mut plans = Vec::new();
        for req in requests {
            let plan = plan_function_sheet(&source, &open, req).expect("the request is valid");
            open.sheets.push(plan.sheet.clone());
            plans.push(plan);
        }
        (open, plans)
    }

    /// The two requests that cover the whole fixture: U2 is divided by `pins`.
    fn full_set() -> Vec<DraftRequest> {
        vec![
            with_pins(request("Sheet A", &[&["U1", "M1"], &["U2"]], &["P24X"]), "U2", &["1", "2", "3", "4"]),
            with_pins(request("Sheet B", &[&["U2"], &["TB1", "R1"]], &["P24X"]), "U2", &["9", "10"]),
        ]
    }

    const LATER_A: &str = "Cabinet B terminal side";
    const LATER_B: &str = "Spare relay panel number two";

    /// Three sheets drafted one after the other; every one of them carries the net SIG_A. The first has SIG_A as a left tag in its first column.
    fn three_sheets(planned: bool) -> Project {
        let mut main = request("Main", &[&["U1"], &["TB1"]], &["P24X"]);
        let mut later_a = request(LATER_A, &[&["R1"]], &[]);
        let mut later_b = with_pins(request(LATER_B, &[&["U2"]], &[]), "U2", &["3"]);
        if planned {
            let names: Vec<crate::draft::PlannedSheet> = ["Main", LATER_A, LATER_B].iter().map(|s| (*s).into()).collect();
            for r in [&mut main, &mut later_a, &mut later_b] {
                r.planned_sheets = names.clone();
            }
        }
        open_of(&[main, later_a, later_b]).0
    }

    /// When the caller names every sheet it will draft, the left tag of the first column keeps room for the names the later sheets add, and no flag leaves the frame.
    /// 呼び出し側がこれから下書きする全てのシートの名前を渡すと、1列目の左の札は後から足される名前の分の場所を空けておき、どの旗も図枠からはみ出さない。
    #[test]
    fn planned_sheets_keep_every_tag_inside_the_frame() {
        let open = three_sheets(true);
        let c = check(&open);
        assert!(c.tags_outside.is_empty(), "{:?}", c.tags_outside);
        assert!(c.layout_ok);
    }

    /// Without the names of the later sheets, the flag of the same left tag grows past the left edge of the frame and the check reports it, while the connections still match.
    /// 後のシートの名前を渡さないと、同じ左の札の旗は図枠の左端を越えて伸び、検算がそれを知らせる。つながりは合ったままである。
    #[test]
    fn without_planned_sheets_the_check_reports_the_flag_that_leaves_the_frame() {
        let open = three_sheets(false);
        let c = check(&open);
        assert!(!c.layout_ok);
        let hit = c.tags_outside.iter().find(|t| t.sheet == "Main" && t.net == "SIG_A").unwrap_or_else(|| panic!("{:?}", c.tags_outside));
        assert!(hit.excess_mm > 1.0, "{hit:?}");
        assert_eq!((hit.excess_mm * 10.0).round() / 10.0, hit.excess_mm, "one decimal");
    }

    /// A layout problem does not change `ok`, which keeps meaning that the connections are equal.
    /// 旗のはみ出しは `ok` を変えない。`ok` はつながりが同じかどうかだけを表す。
    #[test]
    fn layout_ok_false_does_not_change_ok() {
        let mut two = three_sheets(false);
        two.sheets.pop();
        let c = check(&two);
        assert_eq!(c.ok, check_function_sheets(&fixture_source(), &two, &[]).ok);
        let full = three_sheets(false);
        let c = check(&full);
        assert!(!c.layout_ok);
        assert_eq!(c.ok, c.missing_pins.is_empty() && c.extra_pins.is_empty() && c.split.is_empty() && c.merged.is_empty());
    }

    /// The existing two-sheet scenario has no tag outside the frame, so `layout_ok` is true.
    /// 既存の2枚の下書きでは、図枠の外へはみ出す札は無く、`layout_ok` は true になる。
    #[test]
    fn the_two_sheet_scenario_has_no_tag_outside_the_frame() {
        let (open, _) = open_of(&full_set());
        let c = check(&open);
        assert!(c.ok && c.layout_ok && c.tags_outside.is_empty(), "{c:?}");
    }

    /// A tag whose flag pokes past the left, right, top or bottom edge of the inner frame, or lies in the title block, is listed with the overhang in mm; a tag inside the drawing area is not; the list follows sheet order, then net names in natural order (N2 before N10).
    /// 旗が図枠の内側の左・右・上・下の縁を越える札、表題欄の中にある札は、はみ出した量 (mm) と一緒に挙がる。描画範囲の中の札は挙がらない。並びはシート順、次にネット名の自然順 (N2 は N10 より先)。
    #[test]
    fn tags_outside_are_found_on_every_side_and_in_the_title_block() {
        use crate::geometry::Point;
        use crate::model::{NetLabel, Orientation, PaperSize, Sheet};
        let mut open = Project::new("open");
        open.sheets.clear();
        open.sheets.push(Sheet::new("First", PaperSize::A3, Orientation::Landscape));
        open.sheets.push(Sheet::new("Second", PaperSize::A3, Orientation::Landscape));
        let template = crate::frame_template::for_project(&open);
        let (frame, title) = crate::arrange::frame_areas(&template, &open.sheets[0]);
        let font = crate::svg::LABEL_FONT * open.sheets[0].annotation_scale();
        let style = open.sheets[0].xref_style;
        let tag = |name: &str, dir: &str, at: Point| {
            let mut attrs = std::collections::BTreeMap::new();
            attrs.insert("tag".to_string(), dir.to_string());
            NetLabel { id: uuid::Uuid::new_v4(), at, name: name.into(), rotation: 0, attrs }
        };
        // the flag's extent around its point, measured on a probe at the origin
        let extent = |name: &str, dir: &str| {
            let t = crate::net_tag::net_tag_with(&tag(name, dir, Point::new(0.0, 0.0)), font, style, None).unwrap();
            let xs = t.points.iter().map(|p| p.x);
            let ys = t.points.iter().map(|p| p.y);
            (xs.clone().fold(f64::MAX, f64::min), ys.clone().fold(f64::MAX, f64::min), xs.fold(f64::MIN, f64::max), ys.fold(f64::MIN, f64::max))
        };
        let (mid_x, mid_y) = ((frame.0 + frame.2) / 2.0, (frame.1 + title.1) / 2.0);
        let mut place = |sheet: usize, label: NetLabel| {
            open.sheets[sheet].entities.insert(label.id, Entity::NetLabel(label));
        };
        let e = extent("N10", "left");
        place(1, tag("N10", "left", Point::new(frame.0 - e.0 - 3.0, mid_y)));
        let e = extent("N2", "right");
        place(1, tag("N2", "right", Point::new(frame.2 - e.2 + 2.0, mid_y)));
        let e = extent("N3", "left");
        place(1, tag("N3", "left", Point::new(mid_x, frame.1 - e.1 - 1.5)));
        let e = extent("Z1", "left");
        place(0, tag("Z1", "left", Point::new(frame.0 + 10.0, frame.3 - e.3 + 4.0)));
        place(0, tag("TB", "left", Point::new((title.0 + title.2) / 2.0, (title.1 + title.3) / 2.0)));
        place(0, tag("OK", "left", Point::new(mid_x, mid_y)));
        let c = check_function_sheets(&fixture_source(), &open, &[]);
        let got: Vec<(&str, &str)> = c.tags_outside.iter().map(|t| (t.sheet.as_str(), t.net.as_str())).collect();
        assert_eq!(got, [("First", "TB"), ("First", "Z1"), ("Second", "N2"), ("Second", "N3"), ("Second", "N10")]);
        let excess = |net: &str| c.tags_outside.iter().find(|t| t.net == net).unwrap().excess_mm;
        for (net, want) in [("Z1", 4.0), ("N2", 2.0), ("N3", 1.5), ("N10", 3.0)] {
            assert!((excess(net) - want).abs() <= 0.1 + 1e-9, "{net}: {} vs {want}", excess(net));
        }
        assert!(excess("TB") > 0.0);
        assert!(!c.layout_ok);
    }

    /// The tags outside the frame are listed by sheet order, then by net name in natural order.
    /// 枠からはみ出した札は、シートの並び順、次にネット名の自然順で並ぶ。
    #[test]
    fn tags_outside_are_sorted_by_sheet_then_net() {
        let open = three_sheets(false);
        let c = check(&open);
        let order = |name: &str| open.sheets.iter().position(|s| s.name == name).unwrap();
        for w in c.tags_outside.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            assert!(
                order(&a.sheet) < order(&b.sheet) || (order(&a.sheet) == order(&b.sheet) && natural_cmp(&a.net, &b.net) != std::cmp::Ordering::Greater),
                "{a:?} before {b:?}"
            );
        }
    }

    fn check(open: &Project) -> DraftCheck {
        check_function_sheets(&fixture_source(), open, &[])
    }

    fn rename_tag(open: &mut Project, sheet: usize, from: &str, to: &str) {
        let mut hit = 0;
        for e in open.sheets[sheet].entities.values_mut() {
            if let Entity::NetLabel(l) = e {
                if l.name == from {
                    l.name = to.into();
                    hit += 1;
                }
            }
        }
        assert!(hit > 0, "sheet {sheet} has a label {from}");
    }

    /// Two drafted sheets that together cover the source pass the check, with every counted net matched.
    /// 元の図を全部合わせて写した2枚の下書きは、検算に通り、数えたネットがすべて一致する。
    #[test]
    fn a_full_set_of_drafted_sheets_passes() {
        let (open, _) = open_of(&full_set());
        let c = check(&open);
        assert!(c.ok, "{c:?}");
        assert_eq!(c.source_nets, 6, "P24X, GND, SIG_A, SIG_B and the two unlabelled nets");
        assert_eq!(c.matched, 6);
    }

    /// A pin that is on no sheet is reported as missing, with the name of its source net.
    /// どのシートにも描かれていないピンは、元のネットの名前つきで「足りない」と報告される。
    #[test]
    fn a_pin_on_no_sheet_is_missing() {
        let mut reqs = full_set();
        reqs[1] = with_pins(reqs[1].clone(), "TB1", &["1"]);
        let (open, _) = open_of(&reqs);
        let c = check(&open);
        assert!(!c.ok);
        assert_eq!(c.missing_pins, vec![CheckPin { reference: "TB1".into(), pin: "2".into(), source_net: "SIG_B".into() }]);
        assert!(c.split.is_empty() && c.merged.is_empty() && c.extra_pins.is_empty(), "{c:?}");
        assert_eq!(c.matched, 5, "SIG_B is not matched");
    }

    /// A pin on a sheet that the source does not have is reported as extra.
    /// 開いた図にだけあって元の図に無いピンは、「余分」と報告される。
    #[test]
    fn a_pin_the_source_does_not_have_is_extra() {
        let (open, _) = open_of(&full_set());
        let mut source = fixture_source();
        let r1: Vec<_> = source.sheets[0]
            .entities
            .iter()
            .filter(|(_, e)| matches!(e, Entity::Symbol(s) if s.reference == "R1"))
            .map(|(id, _)| *id)
            .collect();
        assert_eq!(r1.len(), 1);
        source.sheets[0].entities.remove(&r1[0]);
        let c = check_function_sheets(&source, &open, &[]);
        let extra: Vec<(&str, &str)> = c.extra_pins.iter().map(|p| (p.reference.as_str(), p.pin.as_str())).collect();
        assert_eq!(extra, vec![("R1", "1"), ("R1", "2")]);
        assert!(c.extra_pins.iter().all(|p| p.source_net.is_empty()));
        assert!(!c.ok);
    }

    /// A tag renamed on one sheet cuts the net in two: the check lists the source net and the two groups.
    /// 片方のシートでタグの名前を変えると、ネットが2つに割れる。検算は元のネットの名前と2つの組を示す。
    #[test]
    fn a_tag_renamed_on_one_sheet_splits_the_net() {
        let (mut open, _) = open_of(&full_set());
        rename_tag(&mut open, 1, "SIG_A", "SIG_Z");
        let c = check(&open);
        assert!(!c.ok);
        assert_eq!(
            c.split,
            vec![CheckSplit { source_net: "SIG_A".into(), parts: vec![vec!["R1:1".into()], vec!["U1:5".into(), "U2:3".into()]] }]
        );
        assert!(c.missing_pins.is_empty() && c.merged.is_empty() && c.extra_pins.is_empty(), "{c:?}");
        assert_eq!(c.matched, 5);
    }

    /// Two source nets that are one net in the open project (here a tag takes the name of another net) are reported as merged.
    /// 元では別のネットだった2本が開いた図で1本につながると、「混ざった」と報告される。
    #[test]
    fn two_source_nets_joined_by_a_wire_are_merged() {
        let (mut open, _) = open_of(&full_set());
        rename_tag(&mut open, 1, "SIG_A", "SIG_B");
        let c = check(&open);
        assert!(!c.ok);
        assert_eq!(c.merged.len(), 1, "{c:?}");
        assert_eq!(c.merged[0].source_nets, vec!["SIG_A".to_string(), "SIG_B".to_string()]);
        assert!(c.merged[0].pins.contains(&"R1:1".to_string()) && c.merged[0].pins.contains(&"U1:4".to_string()), "{c:?}");
        assert_eq!(c.split.len(), 1, "SIG_A also falls apart: U1:5 and U2:3 stay together");
        assert_eq!(c.matched, 4);
    }

    /// References named in ignore_references are left out on both sides, so a unit that is not drawn is not missing.
    /// 無視する参照記号に挙げた機器は両側から除かれるので、描いていない機器でも「足りない」にならない。
    #[test]
    fn ignored_references_are_left_out() {
        let mut reqs = full_set();
        reqs[1] = request("Sheet B", &[&["U2"], &["R1"]], &["P24X"]);
        reqs[1] = with_pins(reqs[1].clone(), "U2", &["9", "10"]);
        let (open, _) = open_of(&reqs);
        let source = fixture_source();
        let without = check_function_sheets(&source, &open, &[]);
        assert!(!without.ok);
        assert_eq!(without.missing_pins.len(), 2, "TB1:1 and TB1:2 are on no sheet");
        let with = check_function_sheets(&source, &open, &["TB1".to_string()]);
        assert!(with.ok, "{with:?}");
        assert_eq!(with.source_nets, 6);
    }

    /// A net with pins on three sheets has a tag on every sheet and counts as one net.
    /// 3枚のシートにピンがあるネットは、どのシートにもタグがあり、1本のネットとして数えられる。
    #[test]
    fn a_net_over_three_sheets_is_one_net() {
        let reqs = vec![
            request("Sheet A", &[&["U1", "M1", "TB1"]], &["P24X"]),
            request("Sheet B", &[&["U2"]], &["P24X"]),
            request("Sheet C", &[&["R1"]], &["P24X"]),
        ];
        let (open, plans) = open_of(&reqs);
        for (i, plan) in plans.iter().enumerate() {
            assert!(plan.tagged.iter().any(|t| t.net == "SIG_A"), "sheet {i} has a tag for SIG_A: {:?}", plan.tagged);
        }
        let c = check(&open);
        assert!(c.ok, "{c:?}");
        assert_eq!(c.matched, c.source_nets);
    }

    /// A power net whose name is also an ordinary tag on another sheet: marks and tags of one name are one net.
    /// 電源のネットの名前が別のシートでは普通のタグになっていても、同じ名前の印とタグは1本のネットになる。
    #[test]
    fn power_marks_and_tags_of_one_name_are_one_net() {
        let mut second = request("Sheet B", &[&["TB1", "R1"]], &[]);
        second.ground_nets = Some(vec!["GND".into()]);
        let reqs = vec![request("Sheet A", &[&["U1", "M1"], &["U2"]], &["P24X"]), second];
        let (open, plans) = open_of(&reqs);
        assert_eq!(plans[1].marked.power, 0, "P24X is not a power net on the second sheet");
        assert!(plans[1].tagged.iter().any(|t| t.net == "P24X"), "{:?}", plans[1].tagged);
        let c = check(&open);
        assert!(c.ok, "{c:?}");
    }

    /// Checking changes neither the source nor the open project.
    /// 検算は、元の図も開いたプロジェクトも書き換えない。
    #[test]
    fn the_check_changes_nothing() {
        let (open, _) = open_of(&full_set());
        let source = fixture_source();
        let before = (serde_json::to_string(&source).unwrap(), serde_json::to_string(&open).unwrap());
        let _ = check_function_sheets(&source, &open, &["U1".to_string()]);
        let after = (serde_json::to_string(&source).unwrap(), serde_json::to_string(&open).unwrap());
        assert_eq!(before, after);
    }

    /// Lists are in natural order: U2 comes before U10, and pin 9 before pin 10.
    /// 一覧は自然順に並ぶ。U2 は U10 より前、ピン 9 は 10 より前。
    #[test]
    fn lists_are_in_natural_order() {
        let mut v = vec![("U10".to_string(), "9".to_string()), ("U2".to_string(), "10".to_string()), ("U2".to_string(), "9".to_string())];
        v.sort_by(pin_cmp);
        let s: Vec<String> = v.iter().map(label).collect();
        assert_eq!(s, ["U2:9", "U2:10", "U10:9"]);
    }

    /// A pin that is drawn but connected to nothing is reported under split, not as missing.
    /// 描いてあるのにどこにもつながっていないピンは、「足りない」ではなく「割れた」に出る。
    #[test]
    fn an_unconnected_view_pin_is_split_not_missing() {
        let (mut open, _) = open_of(&full_set());
        // cut every wire of the sheet that holds R1: its pins stay drawn but join nothing
        let ids: Vec<_> = open.sheets[1].entities.iter().filter(|(_, e)| matches!(e, Entity::Wire(_))).map(|(id, _)| *id).collect();
        for id in ids {
            open.sheets[1].entities.remove(&id);
        }
        let c = check(&open);
        assert!(c.missing_pins.is_empty(), "{c:?}");
        assert!(!c.split.is_empty(), "{c:?}");
    }
}
