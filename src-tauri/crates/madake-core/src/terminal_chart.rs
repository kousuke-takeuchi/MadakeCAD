//! 端子台チャート (spec §1)。1つの端子台について「端子番号ごとに1行」の表を図面から導出する。
//!
//! 行は端子番号順で、各行は内部側 (盤内) と外部側 (盤外) の接続先・線番・電線・ジャンパを持つ。
//! 未結線の端子も予備端子として行が残る。読み取り専用の純関数で、モデルは変更しない
//! (ジャンパの編集は `update_entity` コマンドで `attrs["jumpers"]` を書き換える)。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::geometry::Point;
use crate::model::{Entity, EntityId, Project, Sheet, SheetId, SymbolInstance, Wire};
use crate::netlist::{transform_local, CONNECT_EPS};
use crate::reports::{csv_row, endpoint_label, fmt_num};
use crate::symbol::{resolve_symbol, sheet_symbol_defs, SymbolDef};
use crate::verify::{Diagnostic, Severity};

/// 端子台チャートの列見出し (デザイン「紙 端子台チャート」準拠)。
pub const TERMINAL_CHART_COLUMNS: [&str; 6] =
    ["端子", "内部側", "線番", "電線", "外部側", "ジャンパ"];

/// プロジェクト全体の端子台チャート (全端子台を1つのCSVにまとめる) の列見出し。
/// [`TERMINAL_CHART_COLUMNS`] の先頭に端子台の参照記号列を足したもの。
pub const TERMINAL_CHART_PROJECT_COLUMNS: [&str; 7] = [
    "端子台", "端子", "内部側", "線番", "電線", "外部側", "ジャンパ",
];

/// ジャンパの記述が読み取れない・隣り合わない端子に掛かっている。
pub const JUMPER_INVALID: &str = "terminal.jumper_invalid";
/// ジャンパが端子台に存在しない端子番号を指している。
pub const JUMPER_UNKNOWN_TERMINAL: &str = "terminal.jumper_unknown_terminal";
/// 端子に電線が1本も繋がっていない (予備端子)。
pub const TERMINAL_UNCONNECTED: &str = "terminal.unconnected";

/// `symbol_id` が端子台シンボル (`terminal_block_{n}p`) かどうか。
pub fn is_terminal_block(symbol_id: &str) -> bool {
    symbol_id.starts_with("terminal_block_")
}

/// シート上の端子台シンボルを参照記号順に列挙する。
pub fn terminal_blocks(sheet: &Sheet) -> Vec<&SymbolInstance> {
    let mut list: Vec<&SymbolInstance> = sheet
        .entities
        .values()
        .filter_map(|e| match e {
            Entity::Symbol(s) if is_terminal_block(&s.symbol_id) => Some(s),
            _ => None,
        })
        .collect();
    list.sort_by(|a, b| (&a.reference, a.id).cmp(&(&b.reference, b.id)));
    list
}

/// プロジェクト内の全端子台のID (シート順→シート内は参照記号順)。
/// 端子台チャート・端子接続図をプロジェクト全体へ広げるときの並び順の正。
pub fn terminal_block_ids(project: &Project) -> Vec<EntityId> {
    project
        .sheets
        .iter()
        .flat_map(terminal_blocks)
        .map(|tb| tb.id)
        .collect()
}

/// 端子台1つの概要 (端子台エディタの切替ドロップダウン・帳票の対象選択に使う)。
///
/// チャート本体 ([`TerminalChart`]) より軽く、図面を開かずに一覧を出すためのもの。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TerminalBlockInfo {
    pub entity_id: EntityId,
    /// この端子台が載っているシート。
    pub sheet_id: SheetId,
    pub sheet_name: String,
    /// 参照記号 (例 "TB1")。
    pub reference: String,
    /// 型番・値。
    pub value: String,
    /// 端子数 (極数)。
    pub terminal_count: usize,
    /// ジャンパ指定の生の値 (`attrs["jumpers"]`。未設定なら空)。
    pub jumpers: String,
}

/// 端子台シンボルの端子数 (ピン番号の種類数)。端子台以外・未知のシンボルは0。
pub fn terminal_count_of(symbol_id: &str) -> usize {
    resolve_symbol(symbol_id)
        .map(|def| {
            def.pins
                .iter()
                .map(|p| p.number.clone())
                .collect::<std::collections::BTreeSet<_>>()
                .len()
        })
        .unwrap_or(0)
}

/// プロジェクト内の端子台の概要一覧。`sheet_id`を渡すとそのシートだけに絞る。
///
/// 並びは [`terminal_block_ids`] と同じ (シート順→シート内は参照記号順)。
pub fn terminal_block_infos(project: &Project, sheet_id: Option<SheetId>) -> Vec<TerminalBlockInfo> {
    project
        .sheets
        .iter()
        .filter(|s| sheet_id.is_none_or(|id| s.id == id))
        .flat_map(|sheet| {
            terminal_blocks(sheet).into_iter().map(move |tb| TerminalBlockInfo {
                entity_id: tb.id,
                sheet_id: sheet.id,
                sheet_name: sheet.name.clone(),
                reference: tb.reference.clone(),
                value: tb.value.clone(),
                terminal_count: terminal_count_of(&tb.symbol_id),
                jumpers: tb.attrs.get("jumpers").cloned().unwrap_or_default(),
            })
        })
        .collect()
}

/// 端子台チャートをプロジェクト全体から探す (シートを跨いでentity idで引く)。
pub fn terminal_chart_in_project(project: &Project, tb_id: EntityId) -> Option<TerminalChart> {
    project.sheets.iter().find_map(|s| terminal_chart(s, tb_id))
}

/// 端子台チェックをプロジェクト全体から探した端子台に対して行う。見つからなければ空。
pub fn check_terminal_block_in_project(project: &Project, tb_id: EntityId) -> Vec<Diagnostic> {
    project
        .sheets
        .iter()
        .find(|s| s.entities.contains_key(&tb_id))
        .map(|s| check_terminal_block(s, tb_id))
        .unwrap_or_default()
}

/// 端子台チャートの1行 = 端子1個。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TerminalRow {
    /// 端子番号 (例 "1")。
    pub terminal: String,
    /// 内部側 (盤内) の接続先。`"参照記号:ピン番号"` またはネットラベル名。複数あれば ", " 区切り。
    pub internal: String,
    /// 外部側 (盤外) の接続先。
    pub external: String,
    /// 線番 (この端子に繋がる電線に振られた番号)。
    pub wire_no: String,
    /// 電線の仕様 (線色・線径sq・品番)。内部側と外部側で違えば " / " で両方並べる。
    pub wire: String,
    /// 内部側の電線の仕様だけ (端子接続図の引出線の添え書きに使う)。
    pub internal_wire: String,
    /// 外部側の電線の仕様だけ。
    pub external_wire: String,
    /// 内部側の電線が属するハーネス名 (属さなければ空)。
    pub internal_harness: String,
    /// 外部側の電線が属するハーネス名。
    pub external_harness: String,
    /// この端子に掛かっているジャンパ (例 "1-2")。
    pub jumper: String,
    /// 予備端子 (内部側・外部側とも電線が繋がっていない)。
    pub spare: bool,
}

/// 1つの端子台から導いたチャート。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TerminalChart {
    pub entity_id: EntityId,
    /// 端子台の参照記号 (例 "TB1")。
    pub reference: String,
    /// 端子台の型番・値。
    pub value: String,
    pub sheet_name: String,
    /// 端子数 (極数)。
    pub terminal_count: usize,
    pub rows: Vec<TerminalRow>,
    /// 正規化済みのジャンパ (小さい端子番号が先、昇順、重複なし)。
    pub jumpers: Vec<(u32, u32)>,
    /// 読み取れなかった・不正なジャンパ指定。
    pub jumper_issues: Vec<JumperIssue>,
}

impl TerminalChart {
    /// [`TERMINAL_CHART_COLUMNS`] と同じ並びのセル行。CSV・図面シート化から使う。
    pub fn cells(&self) -> Vec<Vec<String>> {
        self.rows
            .iter()
            .map(|r| {
                vec![
                    r.terminal.clone(),
                    r.internal.clone(),
                    r.wire_no.clone(),
                    r.wire.clone(),
                    r.external.clone(),
                    r.jumper.clone(),
                ]
            })
            .collect()
    }
}

/// 読み取れなかったジャンパ指定1件。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct JumperIssue {
    /// 元の記述 (例 "1-3")。
    pub text: String,
    /// 診断コード ([`JUMPER_INVALID`] / [`JUMPER_UNKNOWN_TERMINAL`])。
    pub code: String,
    /// 何が問題かの説明。
    pub message: String,
}

/// `attrs["jumpers"]` の解析結果。
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Jumpers {
    /// 正規化済みのジャンパ。
    pub pairs: Vec<(u32, u32)>,
    /// 不正な記述 (該当分だけ捨てられ、残りは有効)。
    pub issues: Vec<JumperIssue>,
}

/// ジャンパ指定 (`"1-2,3-4"`) を解析して正規化する。
///
/// - 各項は「端子番号-端子番号」。小さい番号が先になるよう入れ替え、重複を除き、昇順に並べる
/// - 掛けられるのは**隣り合う端子どうし**だけ (`"1-3"` は不正)
/// - `terminal_count` を超える番号・0番は「存在しない端子」として報告する
/// - 空白と空の項は無視する。不正な項があってもパニックせず、問題は [`Jumpers::issues`] に並ぶ
pub fn parse_jumpers(spec: &str, terminal_count: usize) -> Jumpers {
    let mut pairs: Vec<(u32, u32)> = Vec::new();
    let mut issues: Vec<JumperIssue> = Vec::new();
    let issue = |text: &str, code: &str, message: String| JumperIssue {
        text: text.to_string(),
        code: code.to_string(),
        message,
    };
    for raw in spec.split(',') {
        let text = raw.trim();
        if text.is_empty() {
            continue;
        }
        let parts: Vec<&str> = text.split('-').map(|p| p.trim()).collect();
        let nums: Option<Vec<u32>> = (parts.len() == 2)
            .then(|| parts.iter().map(|p| p.parse::<u32>().ok()).collect())
            .flatten();
        let Some(nums) = nums else {
            issues.push(issue(
                text,
                JUMPER_INVALID,
                "「端子番号-端子番号」の形式で書いてください".into(),
            ));
            continue;
        };
        let (a, b) = (nums[0].min(nums[1]), nums[0].max(nums[1]));
        let outside: Vec<u32> = [a, b]
            .into_iter()
            .filter(|n| *n < 1 || *n as usize > terminal_count)
            .collect();
        if !outside.is_empty() {
            let list = outside
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            issues.push(issue(
                text,
                JUMPER_UNKNOWN_TERMINAL,
                format!("存在しない端子 {list} を指しています (端子は1〜{terminal_count})"),
            ));
            continue;
        }
        if b - a != 1 {
            issues.push(issue(
                text,
                JUMPER_INVALID,
                "ジャンパは隣り合う端子どうしにしか掛けられません".into(),
            ));
            continue;
        }
        pairs.push((a, b));
    }
    pairs.sort_unstable();
    pairs.dedup();
    Jumpers { pairs, issues }
}

/// 正規化済みジャンパを `attrs["jumpers"]` に保存する文字列へ戻す (`"1-2,3-4"`)。
pub fn jumpers_to_string(pairs: &[(u32, u32)]) -> String {
    pairs
        .iter()
        .map(|(a, b)| format!("{a}-{b}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// 端子の片側 (接続点1つ) に繋がっているもの。
struct Side<'a> {
    /// 相手側の表記 (`"K1:A1"` / ネットラベル名)。辞書順。
    labels: Vec<String>,
    wires: Vec<&'a Wire>,
}

fn near(a: &Point, b: &Point) -> bool {
    a.distance_to(b) < CONNECT_EPS
}

/// 接続点`at`に繋がる電線と、その反対側の接続先を集める。
fn side_at<'a>(sheet: &'a Sheet, defs: &[SymbolDef], at: &Point) -> Side<'a> {
    let mut labels: Vec<String> = Vec::new();
    let mut wires: Vec<&Wire> = Vec::new();
    for entity in sheet.entities.values() {
        let Entity::Wire(w) = entity else { continue };
        if !w.points.iter().any(|p| near(p, at)) {
            continue;
        }
        wires.push(w);
        for end in [w.points.first(), w.points.last()].into_iter().flatten() {
            if near(end, at) {
                continue;
            }
            let label = endpoint_label(sheet, defs, end);
            if !label.is_empty() && !labels.contains(&label) {
                labels.push(label);
            }
        }
    }
    labels.sort();
    Side { labels, wires }
}

/// 電線の仕様表記 (線色・線径sq・品番)。空の項目は省く。
fn wire_desc(w: &Wire) -> String {
    let sq = fmt_num(w.sq);
    [
        w.color.clone(),
        if sq.is_empty() { String::new() } else { format!("{sq}sq") },
        w.part_no.clone().unwrap_or_default(),
    ]
    .into_iter()
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join(" ")
}

/// 重複を除いて " / " で連結する (内部側→外部側の順)。
fn join_unique(values: impl IntoIterator<Item = String>, sep: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for v in values {
        if !v.is_empty() && !out.contains(&v) {
            out.push(v);
        }
    }
    out.join(sep)
}

/// 端子台1つのチャートを図面から導出する。`tb_id`が端子台シンボルでなければNone。
///
/// **内部側 (盤内) は用紙上で左に来る接続点、外部側 (盤外) は右の接続点**とする。
/// 端子台を回転させた場合も回転後の見た目で判定し、縦向き (90/270度) では上側が内部側になる。
pub fn terminal_chart(sheet: &Sheet, tb_id: EntityId) -> Option<TerminalChart> {
    let Some(Entity::Symbol(inst)) = sheet.entities.get(&tb_id) else {
        return None;
    };
    if !is_terminal_block(&inst.symbol_id) {
        return None;
    }
    let def = resolve_symbol(&inst.symbol_id)?;
    let defs = sheet_symbol_defs(sheet);

    // 端子番号ごとの接続点 (番号は数値順)
    let mut by_no: BTreeMap<(u32, String), Vec<Point>> = BTreeMap::new();
    for pin in &def.pins {
        let key = (pin.number.parse::<u32>().unwrap_or(u32::MAX), pin.number.clone());
        by_no.entry(key).or_default().push(transform_local(pin.at, inst));
    }
    let terminal_count = by_no.len();
    let jumpers = parse_jumpers(
        inst.attrs.get("jumpers").map(String::as_str).unwrap_or(""),
        terminal_count,
    );

    let mut rows = Vec::with_capacity(terminal_count);
    for ((num, terminal), mut points) in by_no {
        // 用紙上の左 (同じなら上) が内部側
        points.sort_by(|a, b| {
            if (a.x - b.x).abs() < CONNECT_EPS {
                a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal)
            } else {
                a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal)
            }
        });
        let inside = side_at(sheet, &defs, &points[0]);
        let outside = match points.len() {
            0 | 1 => Side { labels: Vec::new(), wires: Vec::new() },
            n => side_at(sheet, &defs, &points[n - 1]),
        };
        let jumper = jumpers
            .pairs
            .iter()
            .filter(|(a, b)| *a == num || *b == num)
            .map(|(a, b)| format!("{a}-{b}"))
            .collect::<Vec<_>>()
            .join(",");
        let all_wires = || inside.wires.iter().chain(outside.wires.iter()).copied();
        let mut numbers: Vec<String> = all_wires()
            .filter_map(|w| w.net.clone())
            .collect();
        numbers.sort();
        let harness_of = |side: &Side| {
            join_unique(
                side.wires
                    .iter()
                    .map(|w| crate::harness::harness_name_of_wire(sheet, w)),
                ", ",
            )
        };
        rows.push(TerminalRow {
            terminal,
            internal: inside.labels.join(", "),
            external: outside.labels.join(", "),
            wire_no: join_unique(numbers, "/"),
            wire: join_unique(all_wires().map(wire_desc), " / "),
            internal_wire: join_unique(inside.wires.iter().copied().map(wire_desc), " / "),
            external_wire: join_unique(outside.wires.iter().copied().map(wire_desc), " / "),
            internal_harness: harness_of(&inside),
            external_harness: harness_of(&outside),
            jumper,
            spare: inside.wires.is_empty() && outside.wires.is_empty(),
        });
    }

    Some(TerminalChart {
        entity_id: tb_id,
        reference: inst.reference.clone(),
        value: inst.value.clone(),
        sheet_name: sheet.name.clone(),
        terminal_count,
        rows,
        jumpers: jumpers.pairs,
        jumper_issues: jumpers.issues,
    })
}

/// 端子台チャートのCSV。見出しは [`TERMINAL_CHART_COLUMNS`]。端子台以外を指すと空文字。
pub fn terminal_chart_csv(sheet: &Sheet, tb_id: EntityId) -> String {
    let Some(chart) = terminal_chart(sheet, tb_id) else {
        return String::new();
    };
    let mut out = TERMINAL_CHART_COLUMNS.join(",");
    out.push('\n');
    for row in chart.cells() {
        out.push_str(&csv_row(&row));
        out.push('\n');
    }
    out
}

/// プロジェクト全体の端子台チャートCSV。見出しは [`TERMINAL_CHART_PROJECT_COLUMNS`]。
///
/// 端子台1つ分のCSV ([`terminal_chart_csv`]) の先頭に「端子台」列 (参照記号) を足したもので、
/// 全端子台の行が シート順→参照記号順→端子番号順 に並ぶ。
pub fn terminal_charts_csv(project: &Project) -> String {
    let mut out = TERMINAL_CHART_PROJECT_COLUMNS.join(",");
    out.push('\n');
    for id in terminal_block_ids(project) {
        let Some(chart) = terminal_chart_in_project(project, id) else {
            continue;
        };
        for cells in chart.cells() {
            let mut row = vec![chart.reference.clone()];
            row.extend(cells);
            out.push_str(&csv_row(&row));
            out.push('\n');
        }
    }
    out
}

/// 端子台チェック: 未結線の端子 (Info) と不正なジャンパ (Error) を報告する。
pub fn check_terminal_block(sheet: &Sheet, tb_id: EntityId) -> Vec<Diagnostic> {
    let Some(chart) = terminal_chart(sheet, tb_id) else {
        return Vec::new();
    };
    let mut diags: Vec<Diagnostic> = chart
        .jumper_issues
        .iter()
        .map(|i| Diagnostic {
            severity: Severity::Error,
            code: i.code.clone(),
            message: format!(
                "端子台 {} のジャンパ「{}」: {}",
                chart.reference, i.text, i.message
            ),
            sheet_id: sheet.id,
            entity_ids: vec![tb_id],
        })
        .collect();
    diags.extend(chart.rows.iter().filter(|r| r.spare).map(|r| Diagnostic {
        severity: Severity::Info,
        code: TERMINAL_UNCONNECTED.into(),
        message: format!(
            "端子台 {} の端子 {} は未結線です (予備端子)",
            chart.reference, r.terminal
        ),
        sheet_id: sheet.id,
        entity_ids: vec![tb_id],
    }));
    diags
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use uuid::Uuid;

    /// 端子台TB1を(100,50)に置いたシート。端子1..nはy=50-(n-1)*2.5から2.5mm…5mm間隔。
    fn sheet_with_tb(poles: usize, rotation: u16) -> (Sheet, EntityId) {
        let mut sheet = Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape);
        let tb = SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: format!("terminal_block_{poles}p"),
            at: Point::new(100.0, 50.0),
            rotation,
            mirror: false,
            reference: "TB1".into(),
            value: "BN 4P".into(),
            attrs: Default::default(),
        };
        let id = tb.id;
        sheet.entities.insert(id, Entity::Symbol(tb));
        (sheet, id)
    }

    /// ピン1が(x-7.5,y)、ピン2が(x+7.5,y)にある抵抗を置く。
    fn add_resistor(sheet: &mut Sheet, reference: &str, x: f64, y: f64) {
        let e = Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "resistor".into(),
            at: Point::new(x, y),
            rotation: 0,
            mirror: false,
            reference: reference.into(),
            value: String::new(),
            attrs: Default::default(),
        });
        sheet.entities.insert(e.id(), e);
    }

    /// a-b間にワイヤを1本引く (属性はクロージャで調整)。
    fn add_wire(sheet: &mut Sheet, a: (f64, f64), b: (f64, f64), f: impl FnOnce(&mut Wire)) {
        let mut w = Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(a.0, a.1), Point::new(b.0, b.1)],
            color: String::new(),
            sq: 0.0,
            length_m: None,
            length_source: Default::default(),
            part_no: None,
            net: None,
        };
        f(&mut w);
        sheet.entities.insert(w.id, Entity::Wire(w));
    }

    /// 端子番号1の左右接続点にそれぞれ抵抗を繋いだ4極端子台 (回転0)。
    fn wired_tb() -> (Sheet, EntityId) {
        let (mut sheet, id) = sheet_with_tb(4, 0);
        add_resistor(&mut sheet, "R1", 85.0, 42.5);
        add_wire(&mut sheet, (92.5, 42.5), (97.5, 42.5), |_| {});
        add_resistor(&mut sheet, "R2", 115.0, 42.5);
        add_wire(&mut sheet, (102.5, 42.5), (107.5, 42.5), |_| {});
        (sheet, id)
    }

    fn row<'a>(chart: &'a TerminalChart, terminal: &str) -> &'a TerminalRow {
        chart.rows.iter().find(|r| r.terminal == terminal).expect("row")
    }

    /// The chart of a terminal block has exactly one row per terminal, listed in terminal-number order.
    /// 端子台のチャートは端子1個につき1行で、端子番号の順に並ぶ。
    #[test]
    fn the_chart_has_one_row_per_terminal_in_number_order() {
        let (sheet, id) = sheet_with_tb(4, 0);
        let chart = terminal_chart(&sheet, id).expect("chart");
        assert_eq!(chart.reference, "TB1");
        assert_eq!(chart.terminal_count, 4);
        let numbers: Vec<&str> = chart.rows.iter().map(|r| r.terminal.as_str()).collect();
        assert_eq!(numbers, ["1", "2", "3", "4"]);
    }

    /// What is wired to the left of a terminal is the inside (inside the panel) and what is wired to the right is the outside.
    /// 端子の左側に繋がっているものが内部側 (盤内)、右側に繋がっているものが外部側 (盤外) になる。
    #[test]
    fn the_left_side_is_the_inside_and_the_right_side_is_the_outside() {
        let (sheet, id) = wired_tb();
        let chart = terminal_chart(&sheet, id).expect("chart");
        let r = row(&chart, "1");
        assert_eq!(r.internal, "R1:2");
        assert_eq!(r.external, "R2:1");
        assert!(!r.spare);
    }

    /// Turning a terminal block upside down (180 degrees) swaps its sides too: the inside is still whatever is drawn to the left of it on the paper.
    /// 端子台を180度回転させると内外も入れ替わる。内部側はあくまで用紙上で左に描かれている側である。
    #[test]
    fn a_terminal_block_rotated_180_degrees_still_takes_the_paper_left_as_the_inside() {
        let (mut sheet, id) = sheet_with_tb(4, 180);
        // 回転後、端子1の接続点は (97.5, 57.5) と (102.5, 57.5)
        add_resistor(&mut sheet, "R1", 85.0, 57.5);
        add_wire(&mut sheet, (92.5, 57.5), (97.5, 57.5), |_| {});
        add_resistor(&mut sheet, "R2", 115.0, 57.5);
        add_wire(&mut sheet, (102.5, 57.5), (107.5, 57.5), |_| {});
        let chart = terminal_chart(&sheet, id).expect("chart");
        let r = row(&chart, "1");
        assert_eq!(r.internal, "R1:2", "用紙の左が内部側");
        assert_eq!(r.external, "R2:1");
    }

    /// When a terminal block is laid sideways (90 degrees) its two connection points sit one above the other, and the upper one is taken as the inside.
    /// 端子台を90度回して横向きに置くと接続点は上下に並び、上側が内部側として扱われる。
    #[test]
    fn a_sideways_terminal_block_takes_the_upper_connection_as_the_inside() {
        let (mut sheet, id) = sheet_with_tb(4, 90);
        // 回転後、端子1の接続点は (107.5, 47.5) と (107.5, 52.5)
        add_resistor(&mut sheet, "R1", 137.5, 47.5);
        add_wire(&mut sheet, (107.5, 47.5), (130.0, 47.5), |_| {});
        add_resistor(&mut sheet, "R2", 137.5, 52.5);
        add_wire(&mut sheet, (107.5, 52.5), (130.0, 52.5), |_| {});
        let chart = terminal_chart(&sheet, id).expect("chart");
        let r = row(&chart, "1");
        assert_eq!(r.internal, "R1:1", "上側が内部側");
        assert_eq!(r.external, "R2:1");
    }

    /// A terminal with no wire on either side stays in the chart as a spare row with both sides empty.
    /// どちら側にも電線が繋がっていない端子は、内部側・外部側とも空欄の予備端子として行が残る。
    #[test]
    fn an_unwired_terminal_stays_as_a_spare_row() {
        let (sheet, id) = wired_tb();
        let chart = terminal_chart(&sheet, id).expect("chart");
        let r = row(&chart, "3");
        assert_eq!((r.internal.as_str(), r.external.as_str()), ("", ""));
        assert!(r.spare, "予備端子");
        assert!(!row(&chart, "1").spare);
    }

    /// A row shows the wire number written on the wire and the wire itself (color, gauge and part number).
    /// 各行にはその電線に振られた線番と、電線の仕様 (線色・線径sq・品番) が載る。
    #[test]
    fn a_row_shows_the_wire_number_and_the_wire_specification() {
        let (mut sheet, id) = sheet_with_tb(4, 0);
        add_resistor(&mut sheet, "R1", 85.0, 42.5);
        add_wire(&mut sheet, (92.5, 42.5), (97.5, 42.5), |w| {
            w.net = Some("12".into());
            w.color = "red".into();
            w.sq = 0.75;
            w.part_no = Some("SAMPLE0001".into());
        });
        let chart = terminal_chart(&sheet, id).expect("chart");
        let r = row(&chart, "1");
        assert_eq!(r.wire_no, "12");
        assert_eq!(r.wire, "red 0.75sq SAMPLE0001");
    }

    /// When the wire on the inside differs from the wire on the outside, the row lists both.
    /// 内部側と外部側で電線が違うときは、行に両方の電線が並ぶ。
    #[test]
    fn both_wires_are_listed_when_the_inside_and_outside_differ() {
        let (mut sheet, id) = sheet_with_tb(4, 0);
        add_wire(&mut sheet, (92.5, 42.5), (97.5, 42.5), |w| {
            w.color = "black".into();
            w.sq = 0.3;
        });
        add_wire(&mut sheet, (102.5, 42.5), (107.5, 42.5), |w| {
            w.color = "red".into();
            w.sq = 0.75;
        });
        let chart = terminal_chart(&sheet, id).expect("chart");
        assert_eq!(row(&chart, "1").wire, "black 0.3sq / red 0.75sq");
    }

    /// Besides the combined wire column, each row keeps the wire of the inside and of the outside separately.
    /// 行はまとめた電線欄とは別に、内部側の電線と外部側の電線を分けて持つ。
    #[test]
    fn a_row_keeps_the_wire_of_each_side_separately() {
        let (mut sheet, id) = sheet_with_tb(4, 0);
        add_wire(&mut sheet, (92.5, 42.5), (97.5, 42.5), |w| {
            w.color = "black".into();
            w.sq = 0.3;
        });
        add_wire(&mut sheet, (102.5, 42.5), (107.5, 42.5), |w| {
            w.color = "red".into();
            w.sq = 0.75;
        });
        let chart = terminal_chart(&sheet, id).expect("chart");
        let r = row(&chart, "1");
        assert_eq!(r.internal_wire, "black 0.3sq");
        assert_eq!(r.external_wire, "red 0.75sq");
        assert_eq!(row(&chart, "2").internal_wire, "", "未結線の端子は空欄");
    }

    /// Each row records the harness the wire of that side belongs to, and stays empty for a wire in no harness.
    /// 行は各側の電線が属するハーネス名を持ち、どのハーネスにも属さない電線では空欄になる。
    #[test]
    fn a_row_records_the_harness_of_each_side() {
        let (mut sheet, id) = sheet_with_tb(4, 0);
        add_wire(&mut sheet, (102.5, 42.5), (107.5, 42.5), |_| {});
        add_wire(&mut sheet, (92.5, 42.5), (97.5, 42.5), |_| {});
        let h = Entity::Harness(Harness {
            id: Uuid::new_v4(),
            points: crate::harness::rect_points(
                Point::new(101.0, 40.0),
                Point::new(110.0, 45.0),
            ),
            name: "W10".into(),
            note: String::new(),
        });
        sheet.entities.insert(h.id(), h);
        let chart = terminal_chart(&sheet, id).expect("chart");
        let r = row(&chart, "1");
        assert_eq!(r.external_harness, "W10", "外部側の電線だけが囲みの中");
        assert_eq!(r.internal_harness, "");
    }

    /// Jumper text is normalized: each pair is written smaller-larger, duplicates are dropped and the pairs come out in ascending order.
    /// ジャンパの記述は正規化される。各組は小さい番号が先になり、重複は除かれ、昇順に並ぶ。
    #[test]
    fn jumpers_are_normalized() {
        let j = parse_jumpers("3-4, 2-1 ,2-1", 4);
        assert!(j.issues.is_empty(), "{:?}", j.issues);
        assert_eq!(j.pairs, vec![(1, 2), (3, 4)]);
        assert_eq!(jumpers_to_string(&j.pairs), "1-2,3-4");
    }

    /// A jumper between terminals that are not next to each other is rejected, and the valid jumpers in the same text are still kept.
    /// 隣り合っていない端子どうしのジャンパは受け付けられず、同じ記述内の正しいジャンパは残る。
    #[test]
    fn a_jumper_between_non_adjacent_terminals_is_rejected() {
        let j = parse_jumpers("1-3,3-4", 4);
        assert_eq!(j.pairs, vec![(3, 4)]);
        assert_eq!(j.issues.len(), 1);
        assert_eq!(j.issues[0].text, "1-3");
        assert_eq!(j.issues[0].code, JUMPER_INVALID);
    }

    /// Jumper text that is not a pair of terminal numbers is reported instead of crashing.
    /// 端子番号2つの組になっていないジャンパの記述は、異常終了せずエラーとして報告される。
    #[test]
    fn unreadable_jumper_text_is_reported() {
        let j = parse_jumpers("abc,1-,1-2-3,-,1 - 2", 4);
        assert_eq!(j.pairs, vec![(1, 2)], "空白入りの 1 - 2 は読める");
        assert_eq!(j.issues.len(), 4);
        assert!(j.issues.iter().all(|i| i.code == JUMPER_INVALID));
    }

    /// A jumper pointing at a terminal the block does not have is reported as such.
    /// 端子台に無い端子番号を指すジャンパは、存在しない端子として報告される。
    #[test]
    fn a_jumper_to_a_terminal_that_does_not_exist_is_reported() {
        let j = parse_jumpers("4-5,0-1", 4);
        assert!(j.pairs.is_empty());
        assert_eq!(j.issues.len(), 2);
        assert!(j.issues.iter().all(|i| i.code == JUMPER_UNKNOWN_TERMINAL));
        assert!(j.issues[0].message.contains('5'), "{}", j.issues[0].message);
    }

    /// An empty jumper attribute simply means no jumpers.
    /// ジャンパの記述が空なら、ジャンパは無いという意味になる。
    #[test]
    fn no_jumper_text_means_no_jumpers() {
        let j = parse_jumpers("", 4);
        assert!(j.pairs.is_empty() && j.issues.is_empty());
    }

    /// A jumper appears in the jumper column of both terminals it connects.
    /// ジャンパは、それが繋ぐ両方の端子のジャンパ欄に表示される。
    #[test]
    fn a_jumper_is_shown_on_both_of_its_terminals() {
        let (mut sheet, id) = sheet_with_tb(4, 0);
        let Some(Entity::Symbol(s)) = sheet.entities.get_mut(&id) else { panic!() };
        s.attrs.insert("jumpers".into(), "2-1".into());
        let chart = terminal_chart(&sheet, id).expect("chart");
        assert_eq!(chart.jumpers, vec![(1, 2)]);
        assert_eq!(row(&chart, "1").jumper, "1-2");
        assert_eq!(row(&chart, "2").jumper, "1-2");
        assert_eq!(row(&chart, "3").jumper, "");
    }

    /// The terminal chart CSV has the columns terminal, inside, wire number, wire, outside, jumper, and one line per terminal.
    /// 端子台チャートのCSVは 端子・内部側・線番・電線・外部側・ジャンパ の列を持ち、端子1個につき1行になる。
    #[test]
    fn the_terminal_chart_csv_has_the_designed_columns() {
        let (sheet, id) = wired_tb();
        let csv = terminal_chart_csv(&sheet, id);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "端子,内部側,線番,電線,外部側,ジャンパ");
        assert_eq!(lines.len(), 5, "見出し+4端子: {csv}");
        assert_eq!(lines[1], "1,R1:2,,,R2:1,");
    }

    /// Asking for the chart of something that is not a terminal block gives nothing.
    /// 端子台ではないものにチャートを求めても何も返らない。
    #[test]
    fn only_terminal_blocks_have_a_chart() {
        let (mut sheet, _) = sheet_with_tb(4, 0);
        add_resistor(&mut sheet, "R1", 85.0, 42.5);
        let r_id = sheet
            .entities
            .values()
            .find_map(|e| match e {
                Entity::Symbol(s) if s.reference == "R1" => Some(s.id),
                _ => None,
            })
            .expect("resistor");
        assert!(terminal_chart(&sheet, r_id).is_none());
        assert!(terminal_chart_csv(&sheet, r_id).is_empty());
        let refs: Vec<&str> = terminal_blocks(&sheet)
            .iter()
            .map(|s| s.reference.as_str())
            .collect();
        assert_eq!(refs, ["TB1"], "端子台だけが列挙される");
    }

    /// The terminal block check reports every unwired terminal as information, so spares are visible without being treated as mistakes.
    /// 端子台チェックは未結線の端子をすべて情報として報告するので、予備端子は間違い扱いされずに見える。
    #[test]
    fn the_check_reports_unwired_terminals_as_information() {
        let (sheet, id) = wired_tb();
        let diags = check_terminal_block(&sheet, id);
        let spares: Vec<&Diagnostic> = diags
            .iter()
            .filter(|d| d.code == TERMINAL_UNCONNECTED)
            .collect();
        assert_eq!(spares.len(), 3, "端子2,3,4が未結線");
        assert!(spares.iter().all(|d| d.severity == Severity::Info));
        assert!(spares[0].message.contains("TB1"), "{}", spares[0].message);
        assert_eq!(spares[0].entity_ids, vec![id]);
        assert_eq!(spares[0].sheet_id, sheet.id);
    }

    /// The terminal block check reports a jumper between non-adjacent terminals as an error.
    /// 端子台チェックは、隣り合わない端子に掛けられたジャンパをエラーとして報告する。
    #[test]
    fn the_check_reports_an_invalid_jumper_as_an_error() {
        let (mut sheet, id) = sheet_with_tb(4, 0);
        let Some(Entity::Symbol(s)) = sheet.entities.get_mut(&id) else { panic!() };
        s.attrs.insert("jumpers".into(), "1-3".into());
        let diags = check_terminal_block(&sheet, id);
        let bad: Vec<&Diagnostic> = diags.iter().filter(|d| d.code == JUMPER_INVALID).collect();
        assert_eq!(bad.len(), 1);
        assert_eq!(bad[0].severity, Severity::Error);
        assert!(bad[0].message.contains("1-3"), "{}", bad[0].message);
    }

    /// The terminal block check reports a jumper to a terminal that does not exist as an error.
    /// 端子台チェックは、存在しない端子へのジャンパをエラーとして報告する。
    #[test]
    fn the_check_reports_a_jumper_to_a_missing_terminal_as_an_error() {
        let (mut sheet, id) = sheet_with_tb(4, 0);
        let Some(Entity::Symbol(s)) = sheet.entities.get_mut(&id) else { panic!() };
        s.attrs.insert("jumpers".into(), "4-5".into());
        let diags = check_terminal_block(&sheet, id);
        let bad: Vec<&Diagnostic> = diags
            .iter()
            .filter(|d| d.code == JUMPER_UNKNOWN_TERMINAL)
            .collect();
        assert_eq!(bad.len(), 1);
        assert_eq!(bad[0].severity, Severity::Error);
    }

    /// A terminal block with every terminal wired and correct jumpers passes the check with nothing to report.
    /// 全端子が結線されジャンパも正しい端子台は、チェックで何も指摘されない。
    #[test]
    fn a_fully_wired_terminal_block_passes_the_check() {
        let (mut sheet, id) = sheet_with_tb(2, 0);
        // 2極端子台: 端子1 y=47.5、端子2 y=52.5
        for y in [47.5, 52.5] {
            add_wire(&mut sheet, (92.5, y), (97.5, y), |_| {});
            add_wire(&mut sheet, (102.5, y), (107.5, y), |_| {});
        }
        let Some(Entity::Symbol(s)) = sheet.entities.get_mut(&id) else { panic!() };
        s.attrs.insert("jumpers".into(), "1-2".into());
        assert!(check_terminal_block(&sheet, id).is_empty());
    }

    /// The editor lists every terminal block of the project with its sheet, its pole count and its jumpers, sheet by sheet and in reference-designator order.
    /// 端子台エディタの一覧には、プロジェクトの全端子台がシート順・参照記号順に、所在シート・極数・ジャンパ付きで並ぶ。
    #[test]
    fn the_editor_lists_every_terminal_block_with_its_sheet_poles_and_jumpers() {
        let mut project = Project::new("t");
        let (mut sheet1, tb1) = sheet_with_tb(4, 0);
        let Some(Entity::Symbol(tb)) = sheet1.entities.get_mut(&tb1) else { panic!() };
        tb.attrs.insert("jumpers".into(), "1-2".into());
        let (mut sheet2, tb2) = sheet_with_tb(8, 0);
        sheet2.name = "Sheet2".into();
        let Some(Entity::Symbol(tb)) = sheet2.entities.get_mut(&tb2) else { panic!() };
        tb.reference = "TB2".into();
        tb.symbol_id = "terminal_block_8p".into();
        project.sheets = vec![sheet1, sheet2];

        let infos = terminal_block_infos(&project, None);
        assert_eq!(infos.len(), 2);
        assert_eq!(infos[0].reference, "TB1");
        assert_eq!(infos[0].sheet_name, "Sheet1");
        assert_eq!(infos[0].terminal_count, 4);
        assert_eq!(infos[0].jumpers, "1-2");
        assert_eq!(infos[1].reference, "TB2");
        assert_eq!(infos[1].terminal_count, 8);
        assert_eq!(infos[1].jumpers, "", "ジャンパ未設定なら空");
    }

    /// Naming a sheet narrows the list to the terminal blocks drawn on that sheet.
    /// シートを指定すると、そのシートに描かれている端子台だけの一覧になる。
    #[test]
    fn naming_a_sheet_narrows_the_terminal_block_list_to_that_sheet() {
        let mut project = Project::new("t");
        let (sheet1, _) = sheet_with_tb(4, 0);
        let (mut sheet2, tb2) = sheet_with_tb(4, 0);
        sheet2.name = "Sheet2".into();
        let Some(Entity::Symbol(tb)) = sheet2.entities.get_mut(&tb2) else { panic!() };
        tb.reference = "TB2".into();
        let sheet2_id = sheet2.id;
        project.sheets = vec![sheet1, sheet2];

        let infos = terminal_block_infos(&project, Some(sheet2_id));
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].reference, "TB2");
        assert_eq!(infos[0].sheet_id, sheet2_id);
    }

    /// Symbols that are not terminal blocks never show up in the list.
    /// 端子台ではないシンボルは一覧に出ない。
    #[test]
    fn other_symbols_never_show_up_in_the_terminal_block_list() {
        let mut project = Project::new("t");
        let (mut sheet, _) = sheet_with_tb(4, 0);
        add_resistor(&mut sheet, "R1", 85.0, 42.5);
        project.sheets = vec![sheet];
        let refs: Vec<String> = terminal_block_infos(&project, None)
            .into_iter()
            .map(|i| i.reference)
            .collect();
        assert_eq!(refs, ["TB1"]);
    }

    /// The chart and the check of a terminal block can be looked up by entity id alone, without knowing which sheet it sits on.
    /// 端子台のチャートとチェックは、どのシートにあるかを知らなくてもentity idだけで引ける。
    #[test]
    fn a_terminal_block_can_be_looked_up_by_id_across_sheets() {
        let mut project = Project::new("t");
        let (sheet1, _) = sheet_with_tb(4, 0);
        let (mut sheet2, tb2) = sheet_with_tb(4, 0);
        sheet2.name = "Sheet2".into();
        let Some(Entity::Symbol(tb)) = sheet2.entities.get_mut(&tb2) else { panic!() };
        tb.reference = "TB2".into();
        project.sheets = vec![sheet1, sheet2];

        let chart = terminal_chart_in_project(&project, tb2).expect("チャート");
        assert_eq!(chart.reference, "TB2");
        assert_eq!(chart.sheet_name, "Sheet2");
        let diags = check_terminal_block_in_project(&project, tb2);
        assert_eq!(diags.len(), 4, "未結線の4端子が予備として報告される");
        assert!(terminal_chart_in_project(&project, Uuid::new_v4()).is_none());
        assert!(check_terminal_block_in_project(&project, Uuid::new_v4()).is_empty());
    }

    /// Jumpers are set with the ordinary update_entity command, so the chart follows the change and undo takes it back.
    /// ジャンパは通常のupdate_entityコマンドで設定するので、チャートに反映され、undoで元に戻る。
    #[test]
    fn setting_jumpers_through_update_entity_is_undoable() {
        let project = Project::new("t");
        let (sheet, tb_id) = sheet_with_tb(4, 0);
        let sheet_id = project.sheets[0].id;
        let Some(Entity::Symbol(tb)) = sheet.entities.get(&tb_id) else { panic!() };
        let mut engine = crate::command::Engine::new(project);
        engine
            .execute(crate::Command::AddEntity {
                sheet_id,
                entity: Entity::Symbol(tb.clone()),
            })
            .unwrap();

        let mut updated = tb.clone();
        updated.attrs.insert("jumpers".into(), "2-1".into());
        engine
            .execute(crate::Command::UpdateEntity {
                sheet_id,
                entity: Entity::Symbol(updated),
            })
            .unwrap();
        let chart = terminal_chart(&engine.project().sheets[0], tb_id).expect("chart");
        assert_eq!(chart.jumpers, vec![(1, 2)]);
        assert_eq!(row(&chart, "1").jumper, "1-2");

        engine.undo().unwrap().unwrap();
        let chart = terminal_chart(&engine.project().sheets[0], tb_id).expect("chart");
        assert!(chart.jumpers.is_empty(), "undoでジャンパが消える");
        assert_eq!(row(&chart, "1").jumper, "");
    }
}
