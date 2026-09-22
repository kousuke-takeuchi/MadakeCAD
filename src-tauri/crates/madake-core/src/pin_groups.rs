//! ピングループ括弧 (G8)。
//!
//! `SymbolInstance.attrs["pin_groups"]` = カンマ区切りの `範囲:名前` (例
//! `"1-4:電源,5-8:信号"`) を解析し、範囲の最初から最後のピンにかけて角括弧を描く
//! (縦書きの近似として、名前は4文字ごとに改行して積む)。ブロック(`block`)・端子台
//! (`terminal_block_*`)・コネクタ(`connector_*`)のどれでも使える汎用の仕組みで、
//! ピンそのものの向き ([`crate::symbol::PinDir`]) から括弧の向きを決めるので、回転・
//! ミラーにも自動で追従する。
//!
//! 読み取り専用の純関数だけを置く (モデルは変更しない)。SVG/PDF出力 (`svg.rs`) と
//! canvas描画 (`src/canvas/pinGroups.ts`) の双方がこの1か所のジオメトリを使う。
//! 不正な項目 (範囲が重なる・存在しないピン・書式が読めない) は
//! [`crate::verify::Diagnostic`] として報告し、その項目だけ描かない (パニックしない)。

use std::collections::BTreeSet;

use crate::geometry::Point;
use crate::model::SymbolInstance;
use crate::symbol::{PinDef, PinDir, SymbolDef};

/// `attrs`のキー。
pub const PIN_GROUPS_ATTR: &str = "pin_groups";

/// 「範囲:名前」の書式が読めない (コロンが無い・範囲の両端が空など)。
pub const PIN_GROUP_INVALID: &str = "erc.pin_group_invalid";
/// 範囲の両端がそのシンボルインスタンスのピン番号に無い。
pub const PIN_GROUP_UNKNOWN_PIN: &str = "erc.pin_group_unknown_pin";
/// 範囲が他のグループと重なっている。
pub const PIN_GROUP_OVERLAP: &str = "erc.pin_group_overlap";
/// ピングループ括弧を描けない表示形式のシンボルに指定されている (実機ブロック表示のコイルなど)。
pub const PIN_GROUP_UNSUPPORTED: &str = "erc.pin_group_unsupported";

/// キャプションの折り返し文字数 (縦書きの近似)。
pub const PIN_GROUP_CAPTION_WRAP: usize = 4;
/// 括弧の縦棒・横棒とピンの外側端 (`PinDef::at`、既に足の先端) との間隔 (mm)。
pub const PIN_GROUP_BRACKET_GAP_MM: f64 = 1.5;
/// 括弧の両端のヒゲ (ピン側へ折り返す部分) の長さ (mm)。
pub const PIN_GROUP_BRACKET_TICK_MM: f64 = 1.2;
/// 括弧の棒とキャプションの**文字の縁**との間隔 (mm)。キャプションは中央揃えで描くので、
/// 基準点は文字の幅・高さの半分だけさらに外へ置く (そうしないと長い名前の手前側が
/// ピン番号の上まで戻ってくる)。
pub const PIN_GROUP_CAPTION_GAP_MM: f64 = 1.5;
/// キャプション文字高さ (mm)。
pub const PIN_GROUP_CAPTION_TEXT_MM: f64 = 2.0;
/// キャプションを複数行に積むときの行送り (mm)。
pub const PIN_GROUP_CAPTION_LINE_PITCH_MM: f64 = 2.5;

/// 解析済みのピングループ1件。
#[derive(Debug, Clone, PartialEq)]
pub struct PinGroup {
    /// 元の範囲表記 (例 "1-4")。
    pub range_text: String,
    pub caption: String,
    /// 範囲の最初・最後のピン番号 (チャート順)。
    pub first: String,
    pub last: String,
}

/// 読み取れなかった項目1件。
#[derive(Debug, Clone, PartialEq)]
pub struct PinGroupIssue {
    /// 元の項目全体 (例 "1-4:電源")。
    pub text: String,
    pub code: String,
    pub message: String,
}

/// `parse_pin_groups`の結果。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedPinGroups {
    pub groups: Vec<PinGroup>,
    pub issues: Vec<PinGroupIssue>,
}

/// シンボル定義のピン番号を**チャート順** (定義に並んだ順、初出のみ) に列挙する。
///
/// 端子台の貫通端子 (同じ番号が左右2つ) は最初に出てくる側だけを残す。ブロックは
/// 辺ごとに宣言順のまま (左→右→上→下)。これは端子番号の大小順に並べ替える
/// [`crate::terminal_chart::terminal_numbers_of`] とは**違う**: 多段端子台は列ごとに
/// (`1a,2a,…,1b,2b,…`) 並ぶので、"1a-4a"のような範囲がチャート順では連続する。
pub fn chart_order_pin_defs(def: &SymbolDef) -> Vec<&PinDef> {
    let mut seen = BTreeSet::new();
    def.pins.iter().filter(|p| seen.insert(p.number.clone())).collect()
}

/// [`chart_order_pin_defs`]のピン番号だけの列。
pub fn chart_order_pin_numbers(def: &SymbolDef) -> Vec<String> {
    chart_order_pin_defs(def).into_iter().map(|p| p.number.clone()).collect()
}

/// 全ピンが広がる範囲の中心 (シンボルのローカル座標)。ピンが無ければNone。
fn pin_center(def: &SymbolDef) -> Option<Point> {
    let mut it = def.pins.iter();
    let first = it.next()?.at;
    let (mut min, mut max) = (first, first);
    for pin in it {
        min.x = min.x.min(pin.at.x);
        min.y = min.y.min(pin.at.y);
        max.x = max.x.max(pin.at.x);
        max.y = max.y.max(pin.at.y);
    }
    Some(Point::new((min.x + max.x) / 2.0, (min.y + max.y) / 2.0))
}

/// ある番号のピン定義のうち、ピンの広がりの中心からいちばん遠いものを選ぶ。
///
/// 端子台の貫通端子は同じ番号が左右2つある。多段端子台で右の列 (`1b`…) の括弧を
/// **列の外側**へ出すための規則で、内側 (柱と柱の間) に括弧が入らない。距離が同じなら
/// 定義順で先に出てくるほう (1段の端子台は従来どおり左側)。
pub fn outer_pin_def<'a>(def: &'a SymbolDef, number: &str) -> Option<&'a PinDef> {
    let center = pin_center(def)?;
    let mut best: Option<(&PinDef, f64)> = None;
    for pin in def.pins.iter().filter(|p| p.number == number) {
        let d = pin.at.distance_to(&center);
        if best.is_none_or(|(_, bd)| d > bd) {
            best = Some((pin, d));
        }
    }
    best.map(|(p, _)| p)
}

/// 折り返した各行のうち、いちばん長い行の幅 (mm)。
fn max_line_width(lines: &[String]) -> f64 {
    lines
        .iter()
        .map(|l| crate::tidy::text_width(l, PIN_GROUP_CAPTION_TEXT_MM))
        .fold(0.0, f64::max)
}

/// キャプションを[`PIN_GROUP_CAPTION_WRAP`]文字ごとに折り返す (縦書きの近似)。
/// 空文字は1行 (空行) を返す。
pub fn wrap_caption(caption: &str) -> Vec<String> {
    if caption.is_empty() {
        return vec![String::new()];
    }
    let chars: Vec<char> = caption.chars().collect();
    chars
        .chunks(PIN_GROUP_CAPTION_WRAP)
        .map(|c| c.iter().collect())
        .collect()
}

/// `attrs["pin_groups"]`を解析する。`order`はそのインスタンスのチャート順ピン番号
/// ([`chart_order_pin_numbers`])。
///
/// - 各項目は`"範囲:名前"`。範囲は`"a-b"` (aから始まりbで終わる) か単一ピン`"a"`
/// - 範囲の両端は`order`の中にあり、開始は終了よりチャート順で前 (同じでもよい)
/// - 項目どうしの範囲 (チャート順のインデックス) が重なっていれば、**関係する項目全部**を
///   [`PIN_GROUP_OVERLAP`]として報告し、描画対象から外す
/// - 空白・空項目は無視する。壊れた項目があってもパニックせず、他の項目は有効なまま
pub fn parse_pin_groups(spec: &str, order: &[String]) -> ParsedPinGroups {
    let mut candidates: Vec<(PinGroup, usize, usize)> = Vec::new();
    let mut issues: Vec<PinGroupIssue> = Vec::new();

    for raw in spec.split(',') {
        let text = raw.trim();
        if text.is_empty() {
            continue;
        }
        let Some((range, caption)) = text.split_once(':') else {
            issues.push(PinGroupIssue {
                text: text.to_string(),
                code: PIN_GROUP_INVALID.into(),
                message: "「範囲:名前」の形式で書いてください".into(),
            });
            continue;
        };
        let range = range.trim();
        let caption = caption.trim().to_string();
        // ピン番号そのものに「-」が入っている端子 (ブロックのピン番号は自由文字列) は、
        // 範囲の区切り記号と見分けが付かない。「存在しないピン」と誤報せず、使えないことを
        // そのまま伝える (docs/03-schematic-editorに制限として明記)
        if range.contains('-') && order.iter().any(|p| p == range) {
            issues.push(PinGroupIssue {
                text: text.to_string(),
                code: PIN_GROUP_INVALID.into(),
                message: "ピン番号に「-」を含む端子は範囲指定に使えません".into(),
            });
            continue;
        }
        let (first, last) = match range.split_once('-') {
            Some((a, b)) => (a.trim().to_string(), b.trim().to_string()),
            None => (range.to_string(), range.to_string()),
        };
        if first.is_empty() || last.is_empty() {
            issues.push(PinGroupIssue {
                text: text.to_string(),
                code: PIN_GROUP_INVALID.into(),
                message: "範囲の両端を書いてください (例 \"1-4\" または単ピンなら \"3\")".into(),
            });
            continue;
        }
        let (i0, i1) = match (
            order.iter().position(|p| p == &first),
            order.iter().position(|p| p == &last),
        ) {
            (Some(i0), Some(i1)) => (i0, i1),
            _ => {
                let unknown: Vec<&str> = [first.as_str(), last.as_str()]
                    .into_iter()
                    .filter(|p| !order.iter().any(|o| o == p))
                    .collect();
                issues.push(PinGroupIssue {
                    text: text.to_string(),
                    code: PIN_GROUP_UNKNOWN_PIN.into(),
                    message: format!("存在しないピン {} を指しています", unknown.join(", ")),
                });
                continue;
            }
        };
        if i0 > i1 {
            issues.push(PinGroupIssue {
                text: text.to_string(),
                code: PIN_GROUP_INVALID.into(),
                message: "範囲の始点は終点よりチャート順で前にしてください".into(),
            });
            continue;
        }
        candidates.push((
            PinGroup { range_text: range.to_string(), caption, first, last },
            i0,
            i1,
        ));
    }

    // 重なり判定: インデックス範囲 [i0,i1] が他のどれかと重なる項目は全部除く
    let overlaps = |a: (usize, usize), b: (usize, usize)| a.0 <= b.1 && b.0 <= a.1;
    let mut bad: BTreeSet<usize> = BTreeSet::new();
    for i in 0..candidates.len() {
        for j in 0..candidates.len() {
            if i != j && overlaps((candidates[i].1, candidates[i].2), (candidates[j].1, candidates[j].2)) {
                bad.insert(i);
            }
        }
    }
    for &i in &bad {
        issues.push(PinGroupIssue {
            text: format!("{}:{}", candidates[i].0.range_text, candidates[i].0.caption),
            code: PIN_GROUP_OVERLAP.into(),
            message: "他のピングループと範囲が重なっています".into(),
        });
    }
    let groups = candidates
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !bad.contains(i))
        .map(|(_, (g, ..))| g)
        .collect();

    ParsedPinGroups { groups, issues }
}

/// 描画用に解決したピングループ1件のジオメトリ (**シンボルのローカル座標系**)。
#[derive(Debug, Clone, PartialEq)]
pub struct PinGroupBracket {
    /// 角括弧のポリライン (4点)。
    pub bracket: Vec<Point>,
    /// キャプションの各行と、その中心位置 (文字の幅・高さの中心。描画側が
    /// ベースラインへ`height/2`だけ用紙の下向きにずらす、端子番号と同じ約束)。
    pub caption_lines: Vec<(String, Point)>,
}

/// 1つのピングループの括弧とキャプションのジオメトリを求める。
///
/// `pins`はグループに属するピンの定義 (チャート順、`chart_order_pin_defs`の一部)。
/// 空・ピンの向きが揃っていない (辺をまたぐ範囲) ときはNone (呼び出し側は描かない)。
pub fn pin_group_bracket(pins: &[&PinDef], caption: &str) -> Option<PinGroupBracket> {
    let first_dir = pins.first()?.dir;
    if pins.iter().any(|p| p.dir != first_dir) {
        return None;
    }
    let lines = wrap_caption(caption);
    let bracket_and_caption = match first_dir {
        PinDir::Left | PinDir::Right => {
            let top = pins.iter().map(|p| p.at.y).fold(f64::INFINITY, f64::min);
            let bottom = pins.iter().map(|p| p.at.y).fold(f64::NEG_INFINITY, f64::max);
            let x = pins[0].at.x;
            let sign = if first_dir == PinDir::Left { -1.0 } else { 1.0 };
            let bar_x = x + sign * PIN_GROUP_BRACKET_GAP_MM;
            let tick_x = bar_x - sign * PIN_GROUP_BRACKET_TICK_MM;
            let bracket = vec![
                Point::new(tick_x, top),
                Point::new(bar_x, top),
                Point::new(bar_x, bottom),
                Point::new(tick_x, bottom),
            ];
            let caption_x = bar_x + sign * (PIN_GROUP_CAPTION_GAP_MM + max_line_width(&lines) / 2.0);
            let mid_y = (top + bottom) / 2.0;
            let total = (lines.len().saturating_sub(1)) as f64 * PIN_GROUP_CAPTION_LINE_PITCH_MM;
            let start_y = mid_y - total / 2.0;
            let caption_lines = lines
                .into_iter()
                .enumerate()
                .map(|(i, l)| (l, Point::new(caption_x, start_y + i as f64 * PIN_GROUP_CAPTION_LINE_PITCH_MM)))
                .collect();
            (bracket, caption_lines)
        }
        PinDir::Up | PinDir::Down => {
            let left = pins.iter().map(|p| p.at.x).fold(f64::INFINITY, f64::min);
            let right = pins.iter().map(|p| p.at.x).fold(f64::NEG_INFINITY, f64::max);
            let y = pins[0].at.y;
            let sign = if first_dir == PinDir::Up { -1.0 } else { 1.0 };
            let bar_y = y + sign * PIN_GROUP_BRACKET_GAP_MM;
            let tick_y = bar_y - sign * PIN_GROUP_BRACKET_TICK_MM;
            let bracket = vec![
                Point::new(left, tick_y),
                Point::new(left, bar_y),
                Point::new(right, bar_y),
                Point::new(right, tick_y),
            ];
            let caption_y0 =
                bar_y + sign * (PIN_GROUP_CAPTION_GAP_MM + PIN_GROUP_CAPTION_TEXT_MM / 2.0);
            let mid_x = (left + right) / 2.0;
            let caption_lines = lines
                .into_iter()
                .enumerate()
                .map(|(i, l)| {
                    (l, Point::new(mid_x, caption_y0 + sign * i as f64 * PIN_GROUP_CAPTION_LINE_PITCH_MM))
                })
                .collect();
            (bracket, caption_lines)
        }
    };
    Some(PinGroupBracket { bracket: bracket_and_caption.0, caption_lines: bracket_and_caption.1 })
}

/// あるシンボル定義+`attrs["pin_groups"]`から、描ける括弧を全部求める。
/// 読めない・重なる項目は黙って飛ばす (診断は[`crate::verify`]側が
/// [`parse_pin_groups`]を直接使って報告する)。
///
/// 範囲の順番はチャート順 ([`chart_order_pin_numbers`]) で決めるが、括弧を掛ける位置は
/// 番号ごとに[`outer_pin_def`]で選び直す (多段端子台で括弧が列の内側に入らないように)。
pub fn pin_group_brackets(def: &SymbolDef, spec: &str) -> Vec<PinGroupBracket> {
    if spec.trim().is_empty() {
        return Vec::new();
    }
    let order = chart_order_pin_numbers(def);
    let parsed = parse_pin_groups(spec, &order);
    parsed
        .groups
        .iter()
        .filter_map(|g| {
            let i0 = order.iter().position(|p| p == &g.first)?;
            let i1 = order.iter().position(|p| p == &g.last)?;
            let pins: Vec<&PinDef> = order[i0..=i1]
                .iter()
                .filter_map(|no| outer_pin_def(def, no))
                .collect();
            pin_group_bracket(&pins, &g.caption)
        })
        .collect()
}

/// 括弧とキャプションが**用紙の上で**占める矩形の一覧 (括弧の外接矩形1つ+キャプション1行1つ)。
///
/// 文字は回転しても横書きのままなので、キャプションの箱は用紙座標で文字幅・文字高から取る。
/// 参照記号・型番・機能ラベルの位置決め ([`crate::svg::symbol_bounds`]) が、括弧とキャプションを
/// 避けるためにこれを使う。
pub fn pin_group_paper_boxes(inst: &SymbolInstance, def: &SymbolDef) -> Vec<(Point, Point)> {
    let Some(spec) = inst.attrs.get(PIN_GROUPS_ATTR) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for bracket in pin_group_brackets(def, spec) {
        let pts: Vec<Point> = bracket
            .bracket
            .iter()
            .map(|p| crate::netlist::transform_local(*p, inst))
            .collect();
        if let Some(first) = pts.first() {
            let (mut min, mut max) = (*first, *first);
            for p in &pts {
                min.x = min.x.min(p.x);
                min.y = min.y.min(p.y);
                max.x = max.x.max(p.x);
                max.y = max.y.max(p.y);
            }
            out.push((min, max));
        }
        for (text, at) in &bracket.caption_lines {
            if text.is_empty() {
                continue;
            }
            let c = crate::netlist::transform_local(*at, inst);
            let hw = crate::tidy::text_width(text, PIN_GROUP_CAPTION_TEXT_MM) / 2.0;
            let hh = PIN_GROUP_CAPTION_TEXT_MM / 2.0;
            out.push((
                Point::new(c.x - hw, c.y - hh),
                Point::new(c.x + hw, c.y + hh),
            ));
        }
    }
    out
}

/// [`pin_group_paper_boxes`]をひとまとめにした外接矩形。ピングループが無ければNone。
pub fn pin_group_paper_bounds(inst: &SymbolInstance, def: &SymbolDef) -> Option<(Point, Point)> {
    let boxes = pin_group_paper_boxes(inst, def);
    let (mut min, mut max) = *boxes.first()?;
    for (a, b) in &boxes {
        min.x = min.x.min(a.x);
        min.y = min.y.min(a.y);
        max.x = max.x.max(b.x);
        max.y = max.y.max(b.y);
    }
    Some((min, max))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::resolve_symbol;

    fn order_of(id: &str) -> Vec<String> {
        chart_order_pin_numbers(&resolve_symbol(id).unwrap())
    }

    /// A single "range:caption" item, and a single-pin item with no dash, both parse to one group each.
    /// 「範囲:名前」1項目と、ダッシュの無い単ピン指定は、それぞれ1つのグループとして解析される。
    #[test]
    fn a_range_and_a_single_pin_each_parse_to_one_group() {
        let order = order_of("terminal_block_8p");
        let parsed = parse_pin_groups("1-4:電源,6:予備", &order);
        assert!(parsed.issues.is_empty(), "{:?}", parsed.issues);
        assert_eq!(parsed.groups.len(), 2);
        assert_eq!(parsed.groups[0], PinGroup {
            range_text: "1-4".into(),
            caption: "電源".into(),
            first: "1".into(),
            last: "4".into(),
        });
        assert_eq!(parsed.groups[1].first, "6");
        assert_eq!(parsed.groups[1].last, "6");
    }

    /// A range written back to front (end before start in chart order) is reported as invalid and produces no group.
    /// チャート順で終点が始点より前に書かれた範囲は不正として報告され、グループにならない。
    #[test]
    fn a_backwards_range_is_reported_as_invalid() {
        let order = order_of("terminal_block_8p");
        let parsed = parse_pin_groups("4-1:電源", &order);
        assert!(parsed.groups.is_empty());
        assert_eq!(parsed.issues[0].code, PIN_GROUP_INVALID);
    }

    /// A range naming a pin the instance does not have is reported with the unknown-pin code and produces no group.
    /// そのインスタンスに無いピンを指す範囲は不明ピンの診断コードで報告され、グループにならない。
    #[test]
    fn a_range_with_an_unknown_pin_is_reported() {
        let order = order_of("terminal_block_8p");
        let parsed = parse_pin_groups("1-99:電源", &order);
        assert!(parsed.groups.is_empty());
        assert_eq!(parsed.issues[0].code, PIN_GROUP_UNKNOWN_PIN);
    }

    /// Unparseable text (no colon, or an empty range) is reported as invalid instead of panicking, and the rest of the spec still parses.
    /// コロンが無い・範囲が空といった読めない記述はパニックせず不正として報告され、残りの項目は解析される。
    #[test]
    fn unparseable_text_is_reported_without_panicking() {
        let order = order_of("terminal_block_8p");
        let parsed = parse_pin_groups("nonsense,-4:x,1-2:良い", &order);
        assert_eq!(parsed.groups.len(), 1);
        assert_eq!(parsed.groups[0].caption, "良い");
        assert_eq!(parsed.issues.len(), 2);
        assert!(parsed.issues.iter().all(|i| i.code == PIN_GROUP_INVALID));
    }

    /// Two groups whose chart-order ranges overlap are both reported and neither produces a group, while a third, non-overlapping group is kept.
    /// チャート順の範囲が重なる2つのグループはどちらも報告されグループにならないが、重ならない3つ目は残る。
    #[test]
    fn overlapping_groups_are_both_rejected() {
        let order = order_of("terminal_block_8p");
        let parsed = parse_pin_groups("1-4:A,3-5:B,6-8:C", &order);
        assert_eq!(parsed.groups.len(), 1);
        assert_eq!(parsed.groups[0].caption, "C");
        assert_eq!(parsed.issues.len(), 2);
        assert!(parsed.issues.iter().all(|i| i.code == PIN_GROUP_OVERLAP));
    }

    /// A multi-level terminal block's chart order runs level by level (1a,2a,…,1b,2b,…), so a range like "1a-4a" names a contiguous run within one level.
    /// 多段端子台のチャート順は段ごとに並ぶ (1a,2a,…,1b,2b,…) ので、"1a-4a"のような範囲は1段の中で連続する。
    #[test]
    fn a_multilevel_range_within_one_level_parses() {
        let order = order_of("terminal_block_8p_2l");
        assert_eq!(&order[..4], &["1a", "2a", "3a", "4a"]);
        let parsed = parse_pin_groups("1a-4a:A系統", &order);
        assert!(parsed.issues.is_empty(), "{:?}", parsed.issues);
        assert_eq!(parsed.groups.len(), 1);
    }

    /// Wrapping a caption splits it into chunks of at most 4 characters, in order, approximating vertical writing with stacked horizontal lines.
    /// キャプションの折り返しは4文字ごとに順番どおり分割し、横書きの行を積んで縦書きを近似する。
    #[test]
    fn wrap_caption_splits_into_chunks_of_four() {
        assert_eq!(wrap_caption("電源系統一式"), vec!["電源系統", "一式"]);
        assert_eq!(wrap_caption("信号"), vec!["信号"]);
        assert_eq!(wrap_caption(""), vec![""]);
    }

    /// A bracket over pins on the left side of a symbol is a vertical bar to the left of the pins, spanning from the topmost to the bottommost pin, with its caption further left still.
    /// シンボル左側のピンにかける括弧は、いちばん上から下のピンまで縦棒でつながり、キャプションはさらに左に置かれる。
    #[test]
    fn a_bracket_on_left_pins_is_a_vertical_bar_further_left() {
        let def = resolve_symbol("terminal_block_4p").unwrap();
        let defs = chart_order_pin_defs(&def);
        let pins: Vec<&PinDef> = defs[0..2].to_vec(); // "1","2" (左側の足)
        let bracket = pin_group_bracket(&pins, "電源").unwrap();
        assert!(bracket.bracket.iter().all(|p| p.x < pins[0].at.x), "括弧はピンより外側 (左)");
        let ys: Vec<f64> = bracket.bracket.iter().map(|p| p.y).collect();
        assert!((ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            - ys.iter().cloned().fold(f64::INFINITY, f64::min))
            >= (pins[1].at.y - pins[0].at.y).abs() - 1e-9);
        assert!(
            bracket.caption_lines.iter().all(|(_, p)| p.x < bracket.bracket[0].x),
            "キャプションは括弧よりさらに外側"
        );
    }

    /// A pin whose own number contains a hyphen cannot be named in a pin group, and says so instead of being reported as a pin that does not exist.
    /// ピン番号そのものに「-」が入っている端子はピングループに指定できず、「存在しないピン」ではなく使えない理由がそのまま報告される。
    #[test]
    fn a_pin_number_containing_a_hyphen_is_refused_with_its_own_reason() {
        let order = vec!["A-1".to_string(), "B".to_string()];
        let parsed = parse_pin_groups("A-1:電源", &order);
        assert!(parsed.groups.is_empty());
        assert_eq!(parsed.issues[0].code, PIN_GROUP_INVALID);
        assert!(parsed.issues[0].message.contains("-"), "{:?}", parsed.issues[0]);
    }

    /// On a multi-level terminal block, a group over the second level's terminals brackets the outside of that level's own column, never the gap between the columns.
    /// 多段端子台で2段目の端子にかけたグループの括弧は、その段の列の外側に出る (列と列の間には入らない)。
    #[test]
    fn a_multilevel_group_brackets_the_outside_of_its_own_column() {
        let def = resolve_symbol("terminal_block_8p_2l").unwrap();
        // 2段の列は中心x=-5と+5、箱はそれぞれ±2.5なので右の列の外縁はx=7.5
        let brackets = pin_group_brackets(&def, "1b-4b:B系統");
        assert_eq!(brackets.len(), 1);
        assert!(
            brackets[0].bracket.iter().all(|p| p.x > 7.5),
            "括弧が列の外側に出ていない: {:?}",
            brackets[0].bracket
        );
        assert!(
            brackets[0].caption_lines.iter().all(|(_, p)| p.x > 7.5),
            "キャプションが列の外側に出ていない: {:?}",
            brackets[0].caption_lines
        );
    }

    /// A bracket spans pins from two different sides of a symbol (mixed pin direction) is refused: the shape would not be a straight bracket.
    /// シンボルの2つの辺にまたがるピン (向きが揃わない) への括弧は拒否される: まっすぐな括弧にならないため。
    #[test]
    fn a_bracket_across_mixed_pin_directions_is_refused() {
        let left = PinDef { number: "1".into(), name: String::new(), at: Point::new(-10.0, 0.0), dir: PinDir::Left };
        let top = PinDef { number: "2".into(), name: String::new(), at: Point::new(0.0, -10.0), dir: PinDir::Up };
        assert!(pin_group_bracket(&[&left, &top], "x").is_none());
    }
}
