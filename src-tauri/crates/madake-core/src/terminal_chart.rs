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
use crate::symbol::{parse_terminal_no, resolve_symbol, sheet_symbol_defs, SymbolDef};
use crate::verify::{Diagnostic, Severity};

/// 端子台チャートの列見出し (デザイン「紙 端子台チャート」準拠)。
/// `Lv`は多段端子台 (G6) の段記号列。単段の端子台では常に空欄になる。
pub const TERMINAL_CHART_COLUMNS: [&str; 7] =
    ["端子", "Lv", "内部側", "線番", "電線", "外部側", "ジャンパ"];

/// プロジェクト全体の端子台チャート (全端子台を1つのCSVにまとめる) の列見出し。
/// [`TERMINAL_CHART_COLUMNS`] の先頭に端子台の参照記号列を足したもの。
pub const TERMINAL_CHART_PROJECT_COLUMNS: [&str; 8] = [
    "端子台", "端子", "Lv", "内部側", "線番", "電線", "外部側", "ジャンパ",
];

/// ジャンパバー (G5) の線幅 (mm)。シンボルの線 (0.3mm) の2倍以上にして、
/// 印刷してもショートバーだと分かる太さにする。
///
/// 隣り合う端子の印の隙間は5mmピッチ・半径1.8mmだと1.4mmしかなく、ここへ太いバーと
/// 「両端の丸」を両方入れると端子番号に掛かってしまうので、**バーだけ**を描く
/// (バーの端がどこで終わるかは端子の印の縁そのものが示す)。
pub(crate) const JUMPER_BAR_STROKE: f64 = 0.8;

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
    /// 端子数。多段端子台 (G6) では極数×段数になる。
    pub terminal_count: usize,
    /// 極数 (端子台の「何極」。多段でも段数は掛けない)。
    pub poles: usize,
    /// 段数。単段の端子台は1、多段端子台 (G6) は2以上。
    pub levels: usize,
    /// ジャンパ指定の生の値 (`attrs["jumpers"]`。未設定なら空)。
    pub jumpers: String,
}

/// 端子番号の並べ替えキー (極番号→段記号→表記)。単段は段記号なし、多段 (G6) は `1a` の `a`。
fn terminal_sort_key(no: &str) -> (u32, Option<char>, String) {
    let (pole, level) = parse_terminal_no(no);
    (pole, level, no.to_string())
}

/// `attrs["row_order"]` のキー名。
pub const ROW_ORDER_ATTR: &str = "row_order";

/// 端子台チャートの行の並べ替え順 (spec §1.1)。`attrs["row_order"]` の値。
///
/// どの並びでも**極→段の順で必ず隣り合う** ([`sort_terminal_rows`] が極単位でまとめて動かすため。
/// EPLANの「多段端子はグループを保って移動」を、行の並べ替えではなくキーの組で満たす)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RowOrder {
    /// 数値順 (既定)。極番号を数値として昇順に並べる (現在のチャートの自然順と同じ)。
    #[default]
    Number,
    /// 英数順。極番号を**文字列**として辞書順に並べる (2桁以上で数値順と結果が変わる。
    /// 例: "10" は "2" より前に来る)。
    Alnum,
    /// ページ順 (回路図の出現順)。その極の配線 (内部側・外部側どちらか) が、端子台と
    /// 同じシート上で読み順 (上→下、同じ行なら左→右) に最初に現れる位置の順に並べる
    /// ([`crate::xref::zone_at`] のゾーンで比較)。どちらの側にも配線が無い極 (予備) は
    /// 最後に回り、その中では極番号順。
    Page,
    /// 外部ケーブル順。外部側 (盤外) のハーネス名が最初に現れた極の順にまとめる
    /// (名前の辞書順ではなく、初出順。同じ名前の極どうしは隣り合う)。
    /// ハーネス名を持たない極は最後に回り、その中では極番号順。
    Cable,
    /// ジャンパ順。ジャンパが1本でも掛かっている極を極番号順で先頭にまとめ、
    /// ジャンパの無い極をその後ろに極番号順で並べる。
    Jumper,
}

impl RowOrder {
    /// `attrs["row_order"]` に書く文字列表現。
    pub fn as_str(self) -> &'static str {
        match self {
            RowOrder::Number => "number",
            RowOrder::Alnum => "alnum",
            RowOrder::Page => "page",
            RowOrder::Cable => "cable",
            RowOrder::Jumper => "jumper",
        }
    }
}

/// `attrs["row_order"]` を解析する。未設定・未知語は [`RowOrder::Number`] に落ちる。
pub fn row_order_of(attrs: &BTreeMap<String, String>) -> RowOrder {
    match attrs.get(ROW_ORDER_ATTR).map(String::as_str) {
        Some("alnum") => RowOrder::Alnum,
        Some("page") => RowOrder::Page,
        Some("cable") => RowOrder::Cable,
        Some("jumper") => RowOrder::Jumper,
        _ => RowOrder::Number,
    }
}

/// 図面上の位置を読み順 (行=英字を上から、列=数字を左から) で比較できる形にする
/// ([`crate::xref::zone_at`] が作る `"B3"` のような表記を`(行, 列)`へ戻すだけ)。
fn reading_order_key(sheet: &Sheet, p: Point) -> (char, u32) {
    let zone = crate::xref::zone_at(sheet, p);
    let mut chars = zone.chars();
    let row = chars.next().unwrap_or('A');
    let col: u32 = chars.as_str().parse().unwrap_or(0);
    (row, col)
}

/// [`TerminalChart::rows`] を`order`に並べ替える。多段端子は**極単位でまとめて動かす**ので、
/// どの並びでも極→段の順が崩れない ([`RowOrder`] 参照)。
///
/// ページ順だけ図面上の位置が要るので、`page_points` (`rows`と同じ順で、その行の配線の
/// 反対側にある代表点。配線が無い行はNone) と`sheet`を渡す。それ以外の並びは`rows`自身が
/// 持つ情報 (ジャンパ欄・外部ハーネス欄) だけで決まる。
///
/// これがチャートの並び順を決める**唯一の関数**。CSV・図枠付き図面シート・端子接続図は
/// いずれもここが返した [`TerminalChart::rows`] をそのまま使って描く (順序を独自に持たない)
/// ので、3つの出力は常に同じ並びになる。
pub fn sort_terminal_rows(
    rows: &mut Vec<TerminalRow>,
    order: RowOrder,
    page_points: &[Option<Point>],
    sheet: &Sheet,
) {
    if order == RowOrder::Number || rows.len() <= 1 {
        return;
    }
    // 極ごとにまとめる (rowsは既に極→段の順で渡ってくるので、隣り合う同じ極をまとめるだけでよい)
    let mut clusters: Vec<(u32, Vec<usize>)> = Vec::new();
    for (i, r) in rows.iter().enumerate() {
        let pole = parse_terminal_no(&r.terminal).0;
        match clusters.last_mut() {
            Some((p, idxs)) if *p == pole => idxs.push(i),
            _ => clusters.push((pole, vec![i])),
        }
    }

    // 外部ケーブル順: ハーネス名の初出順を先に求めておく
    let mut cable_rank: BTreeMap<String, usize> = BTreeMap::new();
    if order == RowOrder::Cable {
        for (_, idxs) in &clusters {
            for &i in idxs {
                let h = &rows[i].external_harness;
                if !h.is_empty() && !cable_rank.contains_key(h) {
                    let rank = cable_rank.len();
                    cable_rank.insert(h.clone(), rank);
                }
            }
        }
    }

    clusters.sort_by(|a, b| match order {
        RowOrder::Number => a.0.cmp(&b.0),
        RowOrder::Alnum => a.0.to_string().cmp(&b.0.to_string()).then(a.0.cmp(&b.0)),
        RowOrder::Page => {
            let key = |c: &(u32, Vec<usize>)| -> (bool, Option<(char, u32)>, u32) {
                let best = c
                    .1
                    .iter()
                    .filter_map(|&i| page_points[i])
                    .map(|p| reading_order_key(sheet, p))
                    .min();
                (best.is_none(), best, c.0)
            };
            key(a).cmp(&key(b))
        }
        RowOrder::Cable => {
            let key = |c: &(u32, Vec<usize>)| -> (bool, usize, u32) {
                let rank = c
                    .1
                    .iter()
                    .filter_map(|&i| {
                        let h = &rows[i].external_harness;
                        if h.is_empty() { None } else { cable_rank.get(h).copied() }
                    })
                    .min();
                (rank.is_none(), rank.unwrap_or(usize::MAX), c.0)
            };
            key(a).cmp(&key(b))
        }
        RowOrder::Jumper => {
            let has_jumper = |c: &(u32, Vec<usize>)| c.1.iter().any(|&i| !rows[i].jumper.is_empty());
            (!has_jumper(a), a.0).cmp(&(!has_jumper(b), b.0))
        }
    });

    let order_idx: Vec<usize> = clusters.into_iter().flat_map(|(_, idxs)| idxs).collect();
    let mut taken: Vec<Option<TerminalRow>> = rows.drain(..).map(Some).collect();
    for i in order_idx {
        rows.push(taken[i].take().expect("各行は1回しか並べ替えに出てこない"));
    }
}

/// シンボル定義が持つ端子番号を、チャートに並ぶ順 (極番号→段記号) に列挙する。
///
/// 貫通端子は同じ番号のピンが左右2つあるので、重複を除いた「端子の一覧」になる。
/// ジャンパ指定 ([`parse_jumpers`]) が存在する端子を判定するときの正でもある。
pub fn terminal_numbers_of(def: &SymbolDef) -> Vec<String> {
    let mut keys: Vec<(u32, Option<char>, String)> =
        def.pins.iter().map(|p| terminal_sort_key(&p.number)).collect();
    keys.sort();
    keys.dedup();
    keys.into_iter().map(|(_, _, no)| no).collect()
}

/// 端子台シンボルの端子数 (ピン番号の種類数)。端子台以外・未知のシンボルは0。
/// 多段端子台 (G6) では極数×段数になる。**極数が欲しいときは
/// [`poles_and_levels_of`] を使うこと** (多段端子台では両者が食い違う)。
pub fn terminal_count_of(symbol_id: &str) -> usize {
    resolve_symbol(symbol_id)
        .map(|def| terminal_numbers_of(&def).len())
        .unwrap_or(0)
}

/// 端子台シンボルの (極数, 段数)。多段端子台 (G6) は段数が2以上になり、端子数は極数×段数。
/// 単段の端子台は段数1で、極数=端子数。未知のシンボルは (0, 0)。
///
/// 端子番号 (`1` / `1a`) から数えるので、ユーザーシンボルの端子台にも効く。
pub fn poles_and_levels_of(symbol_id: &str) -> (usize, usize) {
    let Some(def) = resolve_symbol(symbol_id) else {
        return (0, 0);
    };
    let mut poles = std::collections::BTreeSet::new();
    let mut levels = std::collections::BTreeSet::new();
    for no in terminal_numbers_of(&def) {
        let (pole, level) = parse_terminal_no(&no);
        poles.insert(pole);
        if let Some(letter) = level {
            levels.insert(letter);
        }
    }
    (poles.len(), levels.len().max(1))
}

/// 端子台シンボルの端子番号の範囲表記 (例 `"1-8"` / 多段なら `"1a-4b"`)。
/// 端子が1つなら番号そのもの、端子が無ければ空文字。
pub fn terminal_range_of(symbol_id: &str) -> String {
    let Some(def) = resolve_symbol(symbol_id) else {
        return String::new();
    };
    match terminal_numbers_of(&def).as_slice() {
        [] => String::new(),
        [only] => only.clone(),
        [first, .., last] => format!("{first}-{last}"),
    }
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
            terminal_blocks(sheet).into_iter().map(move |tb| {
                let (poles, levels) = poles_and_levels_of(&tb.symbol_id);
                TerminalBlockInfo {
                    entity_id: tb.id,
                    sheet_id: sheet.id,
                    sheet_name: sheet.name.clone(),
                    reference: tb.reference.clone(),
                    value: tb.value.clone(),
                    terminal_count: terminal_count_of(&tb.symbol_id),
                    poles,
                    levels,
                    jumpers: tb.attrs.get("jumpers").cloned().unwrap_or_default(),
                }
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
    /// 端子番号 (例 "1"。多段端子台では段記号付きで "1a" のようになる)。
    pub terminal: String,
    /// 段記号 (例 "a")。多段端子台 (G6) の端子だけが持ち、単段の端子台では空文字。
    pub level: String,
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
    /// 端子数 (`rows`の行数)。多段端子台 (G6) では極数×段数になるので、
    /// **極数が欲しいときは [`poles_and_levels_of`] を使うこと**。
    pub terminal_count: usize,
    pub rows: Vec<TerminalRow>,
    /// 正規化済みのジャンパ (並び順で先に来る端子番号が先、昇順、重複なし)。
    /// 多段端子台 (G6) では段記号付きの端子番号 (`("1a","1b")`)。
    pub jumpers: Vec<(String, String)>,
    /// 読み取れなかった・不正なジャンパ指定。
    pub jumper_issues: Vec<JumperIssue>,
    /// この並びを作った並べ替え順 (`attrs["row_order"]` を解析した結果。未設定・未知語は
    /// [`RowOrder::Number`])。CSV・図枠付き図面シート・端子接続図はここを見て並べ替え
    /// 直したりしない — [`TerminalChart::rows`] がすでにこの順に並んでいる。
    pub row_order: RowOrder,
}

impl TerminalChart {
    /// [`TERMINAL_CHART_COLUMNS`] と同じ並びのセル行。CSV・図面シート化から使う。
    pub fn cells(&self) -> Vec<Vec<String>> {
        self.rows
            .iter()
            .map(|r| {
                vec![
                    r.terminal.clone(),
                    r.level.clone(),
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
    /// 正規化済みのジャンパ (端子番号の組)。多段端子台 (G6) では段記号付き (`("1a","1b")`)。
    pub pairs: Vec<(String, String)>,
    /// 不正な記述 (該当分だけ捨てられ、残りは有効)。
    pub issues: Vec<JumperIssue>,
}

/// 2つの端子が隣り合っているか (ジャンパを掛けられる並び)。
///
/// 端子台は極が縦、段 (G6) が横に並ぶので、隣り合うのは
/// **同じ段で極番号が1違い** (縦の隣) か、**同じ極で段記号が1違い** (横の隣) の組だけ。
/// 極も段も違う斜めの組 (`1a-2b`) は隣り合っていない。
fn terminals_are_adjacent(a: &str, b: &str) -> bool {
    let (pole_a, level_a) = parse_terminal_no(a);
    let (pole_b, level_b) = parse_terminal_no(b);
    if level_a == level_b {
        return pole_a.abs_diff(pole_b) == 1;
    }
    match (pole_a == pole_b, level_a, level_b) {
        (true, Some(x), Some(y)) => (x as i32 - y as i32).abs() == 1,
        _ => false,
    }
}

/// ジャンパ指定 (`"1-2,3-4"`、多段端子台なら `"1a-1b,1a-2a"`) を解析して正規化する。
///
/// - 各項は「端子番号-端子番号」。並び順 (極番号→段記号) で先に来る端子が前になるよう
///   入れ替え、重複を除き、昇順に並べる
/// - 掛けられるのは**隣り合う端子どうし**だけ ([`terminals_are_adjacent`]。`"1-3"` は不正)
/// - `terminals` (その端子台が実際に持つ端子番号) に無い番号は「存在しない端子」として報告する
/// - 多段端子台に段記号なしの数字だけの指定 (`"1-2"`) を書いた場合は、どの段に掛けるのか
///   決まらないので不正として報告する (図面のバーと帳票が食い違わないようにするため)
/// - 空白と空の項は無視する。不正な項があってもパニックせず、問題は [`Jumpers::issues`] に並ぶ
pub fn parse_jumpers(spec: &str, terminals: &[String]) -> Jumpers {
    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut issues: Vec<JumperIssue> = Vec::new();
    let issue = |text: &str, code: &str, message: String| JumperIssue {
        text: text.to_string(),
        code: code.to_string(),
        message,
    };
    // 段記号付きの端子を持つ=多段端子台 (G6)
    let multilevel = terminals.iter().any(|t| parse_terminal_no(t).1.is_some());
    let range = || match (terminals.first(), terminals.last()) {
        (Some(first), Some(last)) => format!("{first}〜{last}"),
        _ => "なし".to_string(),
    };
    for raw in spec.split(',') {
        let text = raw.trim();
        if text.is_empty() {
            continue;
        }
        let parts: Vec<&str> = text.split('-').map(|p| p.trim()).collect();
        if parts.len() != 2 || parts.iter().any(|p| p.is_empty()) {
            issues.push(issue(
                text,
                JUMPER_INVALID,
                "「端子番号-端子番号」の形式で書いてください".into(),
            ));
            continue;
        }
        if multilevel && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())) {
            issues.push(issue(
                text,
                JUMPER_INVALID,
                format!("多段端子台のジャンパは段記号まで書いてください (端子は{})", range()),
            ));
            continue;
        }
        let unknown: Vec<&str> = parts
            .iter()
            .copied()
            .filter(|p| !terminals.iter().any(|t| t == p))
            .collect();
        if !unknown.is_empty() {
            issues.push(issue(
                text,
                JUMPER_UNKNOWN_TERMINAL,
                format!(
                    "存在しない端子 {} を指しています (端子は{})",
                    unknown.join(", "),
                    range()
                ),
            ));
            continue;
        }
        if !terminals_are_adjacent(parts[0], parts[1]) {
            issues.push(issue(
                text,
                JUMPER_INVALID,
                "ジャンパは隣り合う端子どうしにしか掛けられません".into(),
            ));
            continue;
        }
        let (a, b) = if terminal_sort_key(parts[0]) <= terminal_sort_key(parts[1]) {
            (parts[0], parts[1])
        } else {
            (parts[1], parts[0])
        };
        pairs.push((a.to_string(), b.to_string()));
    }
    pairs.sort_by(|x, y| {
        (terminal_sort_key(&x.0), terminal_sort_key(&x.1))
            .cmp(&(terminal_sort_key(&y.0), terminal_sort_key(&y.1)))
    });
    pairs.dedup();
    Jumpers { pairs, issues }
}

/// 正規化済みジャンパを `attrs["jumpers"]` に保存する文字列へ戻す (`"1-2,3-4"`)。
pub fn jumpers_to_string(pairs: &[(String, String)]) -> String {
    pairs
        .iter()
        .map(|(a, b)| format!("{a}-{b}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// シンボル定義の中で、端子番号(文字列)の中心点を求める(同じ番号のピンの中点)。
/// 貫通端子は左右2つの接続点が同じ番号を持つので、その中点が端子の印(丸)の位置になる。
fn terminal_center(def: &SymbolDef, number: &str) -> Option<Point> {
    let pts: Vec<Point> = def
        .pins
        .iter()
        .filter(|p| p.number == number)
        .map(|p| p.at)
        .collect();
    if pts.is_empty() {
        return None;
    }
    let (sx, sy) = pts.iter().fold((0.0, 0.0), |(sx, sy), p| (sx + p.x, sy + p.y));
    Some(Point::new(sx / pts.len() as f64, sy / pts.len() as f64))
}

/// ジャンパ (正規化済みの端子番号の組) を、シンボル定義のローカル座標系での線分に変換する (G5)。
/// 見つからない端子番号の組は結果から省く (パニックしない。端子台以外・壊れた定義にも安全)。
///
/// 形はサドルジャンパの実物どおり、**隣り合う2端子の中心を結ぶ軸の上、2つの印の
/// 隙間だけ**に描く (出典: EPLAN "Using Jumpers"
/// <https://www.eplan.help/en-us/Infoportal/Content/Plattform/2025/Content/htm/terminalgui_k_verwendungbruecken.htm>
/// — サドルジャンパは端子の真ん中の溝に入り、配線用の接続点とは別の接続点を使う)。
///
/// - バーは片方の印の縁 ([`crate::symbol::TERMINAL_MARK_R`]) からもう片方の印の縁まで。
///   縦並び (同じ段) なら列の中心線上の短いバー、横並び (段をまたぐ) なら極の行の上の
///   バーになる (後者は内側のピンの足を跨ぐが、電気的に同じ節点なので問題にならない)
/// - バーの端がどの端子のものかは印の縁が示すので、両端に別の印は描かない
///   ([`JUMPER_BAR_STROKE`] 参照)
/// - [`terminals_are_adjacent`] がジャンパを隣り合う端子どうしに限っているので、
///   途中の端子を跨ぐバーは存在しない
pub fn jumper_bars(def: &SymbolDef, pairs: &[(String, String)]) -> Vec<(Point, Point)> {
    let mark = crate::symbol::TERMINAL_MARK_R;
    pairs
        .iter()
        .filter_map(|(a, b)| {
            let (pa, pb) = (terminal_center(def, a)?, terminal_center(def, b)?);
            let (dx, dy) = (pb.x - pa.x, pb.y - pa.y);
            let len = (dx * dx + dy * dy).sqrt();
            if len < CONNECT_EPS {
                return None;
            }
            let (ux, uy) = (dx / len, dy / len);
            let along = |from: &Point, d: f64, sign: f64| {
                Point::new(from.x + ux * d * sign, from.y + uy * d * sign)
            };
            Some((along(&pa, mark, 1.0), along(&pb, mark, -1.0)))
        })
        .collect()
}

/// 端子の片側 (接続点1つ) に繋がっているもの。
struct Side<'a> {
    /// 相手側の表記 (`"K1:A1"` / ネットラベル名)。辞書順。
    labels: Vec<String>,
    wires: Vec<&'a Wire>,
    /// 各配線の反対側の点 ([`RowOrder::Page`] の並べ替えに使う代表点)。
    points: Vec<Point>,
}

fn near(a: &Point, b: &Point) -> bool {
    a.distance_to(b) < CONNECT_EPS
}

/// 接続点`at`に繋がる電線と、その反対側の接続先を集める。
fn side_at<'a>(sheet: &'a Sheet, defs: &[SymbolDef], at: &Point) -> Side<'a> {
    let mut labels: Vec<String> = Vec::new();
    let mut wires: Vec<&Wire> = Vec::new();
    let mut points: Vec<Point> = Vec::new();
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
            points.push(*end);
            let label = endpoint_label(sheet, defs, end);
            if !label.is_empty() && !labels.contains(&label) {
                labels.push(label);
            }
        }
    }
    labels.sort();
    Side { labels, wires, points }
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

    // 端子番号ごとの接続点 (行番号→段記号→表記の順で並ぶ。単段は段記号なし)
    let mut by_no: BTreeMap<(u32, Option<char>, String), Vec<Point>> = BTreeMap::new();
    for pin in &def.pins {
        let (num, level) = parse_terminal_no(&pin.number);
        let key = (num, level, pin.number.clone());
        by_no.entry(key).or_default().push(transform_local(pin.at, inst));
    }
    let terminals: Vec<String> = by_no.keys().map(|(_, _, no)| no.clone()).collect();
    let terminal_count = terminals.len();
    let jumpers = parse_jumpers(
        inst.attrs.get("jumpers").map(String::as_str).unwrap_or(""),
        &terminals,
    );

    let mut rows = Vec::with_capacity(terminal_count);
    let mut page_points: Vec<Option<Point>> = Vec::with_capacity(terminal_count);
    for ((_num, level, terminal), mut points) in by_no {
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
            0 | 1 => Side { labels: Vec::new(), wires: Vec::new(), points: Vec::new() },
            n => side_at(sheet, &defs, &points[n - 1]),
        };
        // ジャンパは、それが実際に繋ぐ端子の行にだけ載る (同じ極の別の段には載らない)
        let jumper = jumpers
            .pairs
            .iter()
            .filter(|(a, b)| *a == terminal || *b == terminal)
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
            level: level.map(String::from).unwrap_or_default(),
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
        // ページ順 (RowOrder::Page) の代表点: 内部側・外部側どちらの配線でも、読み順で
        // 最初に来る方を使う (どちらにも配線が無ければNone=予備)
        page_points.push(
            inside
                .points
                .iter()
                .chain(outside.points.iter())
                .copied()
                .min_by(|a, b| reading_order_key(sheet, *a).cmp(&reading_order_key(sheet, *b))),
        );
    }

    let row_order = row_order_of(&inst.attrs);
    sort_terminal_rows(&mut rows, row_order, &page_points, sheet);

    Some(TerminalChart {
        entity_id: tb_id,
        reference: inst.reference.clone(),
        value: inst.value.clone(),
        sheet_name: sheet.name.clone(),
        terminal_count,
        rows,
        jumpers: jumpers.pairs,
        jumper_issues: jumpers.issues,
        row_order,
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

    /// 多段端子台TB1を(100,50)に置いたシート (ジャンパ指定付き)。
    fn sheet_with_multilevel_tb(poles: usize, levels: usize, jumpers: &str) -> (Sheet, EntityId) {
        let mut sheet = Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape);
        let mut attrs = std::collections::BTreeMap::new();
        if !jumpers.is_empty() {
            attrs.insert("jumpers".to_string(), jumpers.to_string());
        }
        let tb = SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: format!("terminal_block_{poles}p_{levels}l"),
            at: Point::new(100.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "TB1".into(),
            value: String::new(),
            attrs,
        };
        let id = tb.id;
        sheet.entities.insert(id, Entity::Symbol(tb));
        (sheet, id)
    }

    /// 1〜nの端子番号 (単段の端子台の端子一覧)。
    fn numbered(n: usize) -> Vec<String> {
        (1..=n).map(|i| i.to_string()).collect()
    }

    /// 端子番号の組 (ジャンパ) を作る。
    fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
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

    /// 端子`terminal`の外部側 (盤外、右) の接続点の絶対座標 (回転0のtb向け)。
    fn outer_point(sheet: &Sheet, id: EntityId, terminal: &str) -> Point {
        let Some(Entity::Symbol(inst)) = sheet.entities.get(&id) else { panic!() };
        let def = resolve_symbol(&inst.symbol_id).expect("端子台の定義");
        let mut pts: Vec<Point> = def
            .pins
            .iter()
            .filter(|p| p.number == terminal)
            .map(|p| transform_local(p.at, inst))
            .collect();
        pts.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());
        *pts.last().expect("ピンが無い端子番号")
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
        let j = parse_jumpers("3-4, 2-1 ,2-1", &numbered(4));
        assert!(j.issues.is_empty(), "{:?}", j.issues);
        assert_eq!(j.pairs, pairs(&[("1", "2"), ("3", "4")]));
        assert_eq!(jumpers_to_string(&j.pairs), "1-2,3-4");
    }

    /// A jumper between terminals that are not next to each other is rejected, and the valid jumpers in the same text are still kept.
    /// 隣り合っていない端子どうしのジャンパは受け付けられず、同じ記述内の正しいジャンパは残る。
    #[test]
    fn a_jumper_between_non_adjacent_terminals_is_rejected() {
        let j = parse_jumpers("1-3,3-4", &numbered(4));
        assert_eq!(j.pairs, pairs(&[("3", "4")]));
        assert_eq!(j.issues.len(), 1);
        assert_eq!(j.issues[0].text, "1-3");
        assert_eq!(j.issues[0].code, JUMPER_INVALID);
    }

    /// Jumper text that is not a pair of terminal numbers is reported instead of crashing.
    /// 端子番号2つの組になっていないジャンパの記述は、異常終了せずエラーとして報告される。
    #[test]
    fn unreadable_jumper_text_is_reported() {
        let j = parse_jumpers("abc,1-,1-2-3,-,1 - 2", &numbered(4));
        assert_eq!(j.pairs, pairs(&[("1", "2")]), "空白入りの 1 - 2 は読める");
        assert_eq!(j.issues.len(), 4);
        assert!(j.issues.iter().all(|i| i.code == JUMPER_INVALID));
    }

    /// A jumper pointing at a terminal the block does not have is reported as such.
    /// 端子台に無い端子番号を指すジャンパは、存在しない端子として報告される。
    #[test]
    fn a_jumper_to_a_terminal_that_does_not_exist_is_reported() {
        let j = parse_jumpers("4-5,0-1", &numbered(4));
        assert!(j.pairs.is_empty());
        assert_eq!(j.issues.len(), 2);
        assert!(j.issues.iter().all(|i| i.code == JUMPER_UNKNOWN_TERMINAL));
        assert!(j.issues[0].message.contains('5'), "{}", j.issues[0].message);
    }

    /// An empty jumper attribute simply means no jumpers.
    /// ジャンパの記述が空なら、ジャンパは無いという意味になる。
    #[test]
    fn no_jumper_text_means_no_jumpers() {
        let j = parse_jumpers("", &numbered(4));
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
        assert_eq!(chart.jumpers, pairs(&[("1", "2")]));
        assert_eq!(row(&chart, "1").jumper, "1-2");
        assert_eq!(row(&chart, "2").jumper, "1-2");
        assert_eq!(row(&chart, "3").jumper, "");
    }

    /// The terminal chart CSV has the columns terminal, level, inside, wire number, wire, outside, jumper, and one line per terminal.
    /// 端子台チャートのCSVは 端子・Lv・内部側・線番・電線・外部側・ジャンパ の列を持ち、端子1個につき1行になる。
    #[test]
    fn the_terminal_chart_csv_has_the_designed_columns() {
        let (sheet, id) = wired_tb();
        let csv = terminal_chart_csv(&sheet, id);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "端子,Lv,内部側,線番,電線,外部側,ジャンパ");
        assert_eq!(lines.len(), 5, "見出し+4端子: {csv}");
        assert_eq!(lines[1], "1,,R1:2,,,R2:1,", "単段の端子台はLv列が空欄");
    }

    /// A multi-level terminal block shows its level letter in the chart's Lv column, and its terminal column carries the full pin number ("1a"), so the two levels never look like the same terminal.
    /// 多段端子台はチャートのLv列に段記号が入り、端子列は完全なピン番号 ("1a") になるので、2つの段が同じ端子に見えることはない。
    #[test]
    fn a_multilevel_terminal_block_shows_its_level_letter_in_the_lv_column() {
        let mut sheet = Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape);
        let tb = SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "terminal_block_2p_2l".into(),
            at: Point::new(100.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "TB1".into(),
            value: String::new(),
            attrs: Default::default(),
        };
        let id = tb.id;
        sheet.entities.insert(id, Entity::Symbol(tb));
        let chart = terminal_chart(&sheet, id).expect("chart");
        assert_eq!(chart.terminal_count, 4, "2極x2段=4端子");
        let terminals: Vec<&str> = chart.rows.iter().map(|r| r.terminal.as_str()).collect();
        assert_eq!(terminals, ["1a", "1b", "2a", "2b"]);
        let levels: Vec<&str> = chart.rows.iter().map(|r| r.level.as_str()).collect();
        assert_eq!(levels, ["a", "b", "a", "b"]);
    }

    /// On a multi-level terminal block a jumper is written with the level letter, so "1a-1b" links the two levels of the same pole and is accepted.
    /// 多段端子台のジャンパは段記号まで書く。"1a-1b" は同じ極の2つの段を繋ぐ指定として受け付けられる。
    #[test]
    fn a_jumper_can_link_the_two_levels_of_one_pole() {
        let (sheet, id) = sheet_with_multilevel_tb(4, 2, "1b-1a");
        let chart = terminal_chart(&sheet, id).expect("chart");
        assert!(chart.jumper_issues.is_empty(), "{:?}", chart.jumper_issues);
        assert_eq!(row(&chart, "1a").jumper, "1a-1b", "段の小さい方が先に正規化される");
        assert_eq!(row(&chart, "1b").jumper, "1a-1b");
    }

    /// On a multi-level terminal block a jumper between neighbouring poles of the same level ("1a-2a") is accepted just like on a single-level block.
    /// 多段端子台でも、同じ段の隣り合う極どうしのジャンパ ("1a-2a") は単段の端子台と同じように受け付けられる。
    #[test]
    fn a_jumper_can_link_neighbouring_poles_of_one_level() {
        let (sheet, id) = sheet_with_multilevel_tb(4, 2, "1a-2a");
        let chart = terminal_chart(&sheet, id).expect("chart");
        assert!(chart.jumper_issues.is_empty(), "{:?}", chart.jumper_issues);
        assert_eq!(row(&chart, "1a").jumper, "1a-2a");
        assert_eq!(row(&chart, "2a").jumper, "1a-2a");
    }

    /// A jumper written without the level letter ("1-2") on a multi-level terminal block is refused, because it does not say which level it is on.
    /// 多段端子台に段記号なしのジャンパ ("1-2") を書くと、どの段に掛けるのか分からないので受け付けられない。
    #[test]
    fn a_jumper_without_a_level_letter_is_refused_on_a_multilevel_block() {
        let (sheet, id) = sheet_with_multilevel_tb(4, 2, "1-2");
        let chart = terminal_chart(&sheet, id).expect("chart");
        assert!(chart.jumpers.is_empty(), "バーもチャートも出さない");
        assert_eq!(chart.jumper_issues.len(), 1);
        assert_eq!(chart.jumper_issues[0].code, JUMPER_INVALID);
        assert!(chart.rows.iter().all(|r| r.jumper.is_empty()), "どの行にも載らない");
    }

    /// A jumper pointing at a level the multi-level terminal block does not have ("1a-1c" on a 2-level block) is refused as a terminal that does not exist.
    /// 多段端子台に無い段を指すジャンパ (2段の端子台への "1a-1c") は、存在しない端子として受け付けられない。
    #[test]
    fn a_jumper_to_a_level_the_block_does_not_have_is_refused() {
        let (sheet, id) = sheet_with_multilevel_tb(4, 2, "1a-1c");
        let chart = terminal_chart(&sheet, id).expect("chart");
        assert!(chart.jumpers.is_empty());
        assert_eq!(chart.jumper_issues.len(), 1);
        assert_eq!(chart.jumper_issues[0].code, JUMPER_UNKNOWN_TERMINAL);
    }

    /// A diagonal jumper ("1a-2b" — a different pole and a different level at once) is refused, because those two terminals are not next to each other.
    /// 斜めのジャンパ ("1a-2b"、極も段も違う組) は、その2端子が隣り合っていないので受け付けられない。
    #[test]
    fn a_diagonal_jumper_across_a_pole_and_a_level_is_refused() {
        let (sheet, id) = sheet_with_multilevel_tb(4, 2, "1a-2b");
        let chart = terminal_chart(&sheet, id).expect("chart");
        assert!(chart.jumpers.is_empty());
        assert_eq!(chart.jumper_issues.len(), 1);
        assert_eq!(chart.jumper_issues[0].code, JUMPER_INVALID);
    }

    /// The jumper column of a multi-level chart shows a jumper only on the two rows it actually links, never on the other levels of the same poles.
    /// 多段端子台のチャートのジャンパ欄には、そのジャンパが実際に繋ぐ2行にだけ表示され、同じ極の別の段には出ない。
    #[test]
    fn a_multilevel_jumper_is_shown_only_on_the_rows_it_links() {
        let (sheet, id) = sheet_with_multilevel_tb(2, 2, "1a-1b");
        let chart = terminal_chart(&sheet, id).expect("chart");
        assert_eq!(row(&chart, "1a").jumper, "1a-1b");
        assert_eq!(row(&chart, "1b").jumper, "1a-1b");
        assert_eq!(row(&chart, "2a").jumper, "");
        assert_eq!(row(&chart, "2b").jumper, "");
    }

    /// The terminal block check reports a jumper written without its level letter on a multi-level block as an error, so the drawing and the report can never disagree about it.
    /// 端子台チェックは、多段端子台に段記号なしで書かれたジャンパをエラーとして報告するので、図面と帳票が食い違うことはない。
    #[test]
    fn the_check_reports_a_jumper_without_a_level_letter_as_an_error() {
        let (sheet, id) = sheet_with_multilevel_tb(4, 2, "1-2");
        let diags = check_terminal_block(&sheet, id);
        let bad: Vec<&Diagnostic> = diags.iter().filter(|d| d.code == JUMPER_INVALID).collect();
        assert_eq!(bad.len(), 1);
        assert_eq!(bad[0].severity, Severity::Error);
        assert!(bad[0].message.contains("1-2"), "{}", bad[0].message);
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

    /// The summary of a multi-level terminal block tells its pole count and its level count apart from its terminal count: a 4-pole 2-level block has 4 poles, 2 levels and 8 terminals.
    /// 多段端子台の概要は極数・段数を端子数と区別して持つ: 4極2段の端子台は極数4・段数2・端子数8になる。
    #[test]
    fn a_multilevel_terminal_block_summary_separates_poles_levels_and_terminals() {
        let mut project = Project::new("t");
        let (sheet, _) = sheet_with_multilevel_tb(4, 2, "");
        project.sheets = vec![sheet];
        let infos = terminal_block_infos(&project, None);
        assert_eq!((infos[0].poles, infos[0].levels, infos[0].terminal_count), (4, 2, 8));
    }

    /// A single-level terminal block reports one level, so its pole count and its terminal count are the same number.
    /// 単段の端子台は段数1として報告されるので、極数と端子数は同じ数になる。
    #[test]
    fn a_single_level_terminal_block_summary_has_one_level() {
        let mut project = Project::new("t");
        let (sheet, _) = sheet_with_tb(4, 0);
        project.sheets = vec![sheet];
        let infos = terminal_block_infos(&project, None);
        assert_eq!((infos[0].poles, infos[0].levels, infos[0].terminal_count), (4, 1, 4));
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

    /// The jumper bar between the two levels of one pole lies on that pole's row, in the gap between the two level columns' marks.
    /// 同じ極の2つの段を繋ぐジャンパバーは、その極の行の上、2つの段の列の印の隙間に引かれる。
    #[test]
    fn a_cross_level_jumper_bar_lies_on_the_row_between_the_two_marks() {
        let def = resolve_symbol("terminal_block_4p_2l").expect("多段端子台");
        let bars = jumper_bars(&def, &pairs(&[("1a", "1b")]));
        assert_eq!(bars.len(), 1);
        let mark = crate::symbol::TERMINAL_MARK_R;
        assert_eq!(
            (bars[0].0.x, bars[0].1.x),
            (-5.0 + mark, 5.0 - mark),
            "段aの印の縁から段bの印の縁まで"
        );
        assert_eq!((bars[0].0.y, bars[0].1.y), (-7.5, -7.5), "極1の行の上");
    }

    /// Each end of a jumper bar sits exactly on the edge of its own terminal's round mark, and every other terminal of the block is farther away, so a jumper always reads as joining the right pair.
    /// ジャンパバーの両端はちょうど自分の端子の印の縁の上にあり、ほかのどの端子よりも自分の端子に近いので、どの2端子を繋いでいるか読み違えようがない。
    #[test]
    fn each_jumper_bar_end_sits_on_the_edge_of_its_own_terminal_mark() {
        let mark = crate::symbol::TERMINAL_MARK_R;
        for (id, spec) in [
            ("terminal_block_4p", "1-2,2-3"),
            ("terminal_block_4p_2l", "1a-1b,2a-3a"),
            ("terminal_block_4p_3l", "1a-1b,1b-1c"),
            ("terminal_block_12p_2l", "10a-10b,11a-12a"),
        ] {
            let def = resolve_symbol(id).expect("端子台");
            let terminals = terminal_numbers_of(&def);
            let jumpers = parse_jumpers(spec, &terminals).pairs;
            assert!(!jumpers.is_empty(), "{id} {spec}");
            for ((ea, eb), (a, b)) in jumper_bars(&def, &jumpers).iter().zip(&jumpers) {
                for (end, owner) in [(ea, a), (eb, b)] {
                    let own = terminal_center(&def, owner).expect(owner).distance_to(end);
                    assert!(
                        (own - mark).abs() < 1e-9,
                        "{id} {spec}: {owner}側の端が印の縁 ({mark}mm) に無い ({own}mm)"
                    );
                    for no in &terminals {
                        if no == owner {
                            continue;
                        }
                        let other = terminal_center(&def, no).expect(no).distance_to(end);
                        assert!(own < other, "{id} {spec}: {owner}側の端が{no}に近い ({own} >= {other})");
                    }
                }
            }
        }
    }

    /// A jumper bar is exactly as long as the gap between the two terminal marks it joins, that is the terminals' own spacing less one mark radius at each end.
    /// ジャンパバーの長さは、繋ぐ2つの端子の印の隙間ちょうど、つまりその端子台の端子の間隔から両端の印の半径を引いた長さになる。
    #[test]
    fn a_jumper_bar_is_exactly_as_long_as_the_gap_between_the_two_marks() {
        let mark = crate::symbol::TERMINAL_MARK_R;
        for (id, a, b) in [
            ("terminal_block_4p", "1", "2"),
            ("terminal_block_4p_2l", "1a", "2a"),
            ("terminal_block_4p_2l", "1a", "1b"),
        ] {
            let def = resolve_symbol(id).expect("端子台");
            // 端子の間隔はシンボル定義のピンの位置から取る (図面上の実際の間隔)
            let pitch = terminal_center(&def, a)
                .expect(a)
                .distance_to(&terminal_center(&def, b).expect(b));
            let bars = jumper_bars(&def, &pairs(&[(a, b)]));
            assert_eq!(bars.len(), 1);
            let len = bars[0].0.distance_to(&bars[0].1);
            assert!(
                (len - (pitch - 2.0 * mark)).abs() < 1e-9,
                "{id} {a}-{b}: 間隔{pitch}mmの隙間は{}mmなのにバーは{len}mm",
                pitch - 2.0 * mark
            );
        }
    }

    /// A jumper between two terminals of one column is drawn entirely in the gap between their two marks, so it never runs over a wire connection point of the terminal block.
    /// 同じ列の2端子を繋ぐジャンパは、2つの印の間の隙間だけに描かれるので、端子台の配線接続点の上を通ることはない。
    #[test]
    fn a_column_jumper_stays_in_the_gap_between_the_two_marks() {
        let def = resolve_symbol("terminal_block_4p").expect("端子台");
        let bars = jumper_bars(&def, &pairs(&[("1", "2")]));
        assert_eq!(bars.len(), 1);
        let (a, b) = bars[0];
        assert_eq!((a.x, b.x), (0.0, 0.0), "列の中心線の上");
        assert_eq!(
            (a.y, b.y),
            (-7.5 + crate::symbol::TERMINAL_MARK_R, -2.5 - crate::symbol::TERMINAL_MARK_R),
            "端子1の印の縁から端子2の印の縁まで"
        );
        // どの接続点も、バーの太さの外にある
        for pin in &def.pins {
            assert!(
                (pin.at.x - a.x).abs() > JUMPER_BAR_STROKE / 2.0,
                "接続点{:?}がジャンパバーに掛かっている",
                pin.at
            );
        }
    }

    /// Several jumpers on the same terminal block each produce their own bar segment.
    /// 1つの端子台に複数のジャンパがあれば、それぞれ独立したバーの線分になる。
    #[test]
    fn multiple_jumpers_each_produce_their_own_bar_segment() {
        let def = resolve_symbol("terminal_block_4p").expect("端子台");
        assert_eq!(jumper_bars(&def, &pairs(&[("1", "2"), ("3", "4")])).len(), 2);
    }

    /// A jumper referencing a terminal number the block does not have produces no segment instead of panicking.
    /// 端子台に無い端子番号を指すジャンパは、パニックせず線分を作らない。
    #[test]
    fn a_jumper_to_a_missing_terminal_produces_no_bar_segment() {
        let def = resolve_symbol("terminal_block_4p").expect("端子台");
        assert!(jumper_bars(&def, &pairs(&[("4", "5")])).is_empty());
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
        assert_eq!(chart.jumpers, pairs(&[("1", "2")]));
        assert_eq!(row(&chart, "1").jumper, "1-2");

        engine.undo().unwrap().unwrap();
        let chart = terminal_chart(&engine.project().sheets[0], tb_id).expect("chart");
        assert!(chart.jumpers.is_empty(), "undoでジャンパが消える");
        assert_eq!(row(&chart, "1").jumper, "");
    }

    /// 端子番号だけの並び (行のterminal欄の並び)。
    fn terminal_order(chart: &TerminalChart) -> Vec<&str> {
        chart.rows.iter().map(|r| r.terminal.as_str()).collect()
    }

    /// `attrs["row_order"]`が未設定・空・未知語なら数値順 (既定) に落ちる。
    /// An unset, empty or unrecognised `row_order` attribute falls back to number order.
    #[test]
    fn an_unset_or_unknown_row_order_falls_back_to_number_order() {
        assert_eq!(row_order_of(&BTreeMap::new()), RowOrder::Number);
        let mut attrs = BTreeMap::new();
        attrs.insert("row_order".to_string(), "bogus".to_string());
        assert_eq!(row_order_of(&attrs), RowOrder::Number);

        let (mut sheet, id) = sheet_with_tb(4, 0);
        let Some(Entity::Symbol(s)) = sheet.entities.get_mut(&id) else { panic!() };
        s.attrs.insert("row_order".into(), "bogus".into());
        let chart = terminal_chart(&sheet, id).expect("chart");
        assert_eq!(chart.row_order, RowOrder::Number);
        assert_eq!(terminal_order(&chart), ["1", "2", "3", "4"], "未知語は数値順のまま");
    }

    /// 数値順 (既定) は極番号を数として昇順に並べる。10極以上でも桁の大小どおりになる。
    /// Number order (the default) sorts poles as integers, so it stays correct past nine poles.
    #[test]
    fn number_order_sorts_poles_as_integers() {
        let (sheet, id) = sheet_with_tb(12, 0);
        let chart = terminal_chart(&sheet, id).expect("chart");
        assert_eq!(chart.row_order, RowOrder::Number);
        assert_eq!(
            terminal_order(&chart),
            ["1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12"]
        );
    }

    /// 英数順は極番号を文字列として辞書順に並べるので、10極以上では数値順と違う結果になる
    /// ("10"が"2"より前に来る)。
    /// Alphanumeric order compares pole numbers as strings, so past nine poles it disagrees with number order ("10" sorts before "2").
    #[test]
    fn alnum_order_compares_pole_numbers_as_strings() {
        let (mut sheet, id) = sheet_with_tb(12, 0);
        let Some(Entity::Symbol(s)) = sheet.entities.get_mut(&id) else { panic!() };
        s.attrs.insert("row_order".into(), "alnum".into());
        let chart = terminal_chart(&sheet, id).expect("chart");
        assert_eq!(chart.row_order, RowOrder::Alnum);
        assert_eq!(
            terminal_order(&chart),
            ["1", "10", "11", "12", "2", "3", "4", "5", "6", "7", "8", "9"]
        );
    }

    /// ページ順は、その極の配線が図面上で読み順(上→下、左→右)に最初に現れる位置で並べる。
    /// 番号は逆でも、読み順で先に来る方が先になる。配線の無い極(予備)は最後に極番号順で並ぶ。
    /// Page order sorts poles by where their wiring first appears on the sheet in reading order (top-to-bottom, then left-to-right) — even against numeric order — and puts unwired (spare) poles last, in pole-number order.
    #[test]
    fn page_order_follows_where_the_wiring_first_appears_on_the_sheet() {
        let (mut sheet, id) = sheet_with_tb(4, 0);
        // 端子1の外部側配線は用紙の下の方 (読み順で後)、端子2の外部側配線は用紙の上の方 (読み順で先)
        let p1 = outer_point(&sheet, id, "1");
        add_wire(&mut sheet, (p1.x, p1.y), (390.0, 280.0), |_| {});
        let p2 = outer_point(&sheet, id, "2");
        add_wire(&mut sheet, (p2.x, p2.y), (15.0, 15.0), |_| {});
        let Some(Entity::Symbol(s)) = sheet.entities.get_mut(&id) else { panic!() };
        s.attrs.insert("row_order".into(), "page".into());
        let chart = terminal_chart(&sheet, id).expect("chart");
        assert_eq!(chart.row_order, RowOrder::Page);
        assert_eq!(
            terminal_order(&chart),
            ["2", "1", "3", "4"],
            "端子2の配線の方が読み順で先に現れるので、番号が逆でも先に来る。3・4は未結線なので最後"
        );
    }

    /// 外部ケーブル順は、外部ハーネス名が最初に現れた極の順にその名前の極をまとめる
    /// (名前の辞書順ではなく初出順)。ハーネス名の無い極は最後に極番号順で並ぶ。
    /// Cable order gathers the poles of each external harness together in the order that harness name first appears (not alphabetically), putting poles with no harness last in pole-number order.
    #[test]
    fn cable_order_groups_poles_by_their_external_harness_in_first_seen_order() {
        let (mut sheet, id) = sheet_with_tb(4, 0);
        for (terminal, target_x) in [("1", 130.0), ("2", 135.0), ("3", 140.0)] {
            let p = outer_point(&sheet, id, terminal);
            add_wire(&mut sheet, (p.x, p.y), (target_x, p.y), |_| {});
        }
        let h = |name: &str, y: f64| {
            Entity::Harness(Harness {
                id: Uuid::new_v4(),
                points: crate::harness::rect_points(Point::new(100.0, y - 2.0), Point::new(145.0, y + 2.0)),
                name: name.into(),
                note: String::new(),
            })
        };
        let p1 = outer_point(&sheet, id, "1");
        let p2 = outer_point(&sheet, id, "2");
        let p3 = outer_point(&sheet, id, "3");
        sheet.entities.insert(Uuid::new_v4(), h("W2", p1.y)); // 端子1が最初に載せる名前
        sheet.entities.insert(Uuid::new_v4(), h("W1", p2.y)); // 端子2が最初に載せる名前
        sheet.entities.insert(Uuid::new_v4(), h("W2", p3.y)); // 端子3は端子1と同じ"W2"
        let Some(Entity::Symbol(s)) = sheet.entities.get_mut(&id) else { panic!() };
        s.attrs.insert("row_order".into(), "cable".into());
        let chart = terminal_chart(&sheet, id).expect("chart");
        assert_eq!(chart.row_order, RowOrder::Cable);
        assert_eq!(row(&chart, "1").external_harness, "W2");
        assert_eq!(row(&chart, "2").external_harness, "W1");
        assert_eq!(row(&chart, "3").external_harness, "W2");
        assert_eq!(
            terminal_order(&chart),
            ["1", "3", "2", "4"],
            "W2(端子1で初出)の端子がまとまって先に来て、W1(端子2で初出)がその後、\
             ハーネスの無い端子4は最後"
        );
    }

    /// ジャンパ順は、ジャンパが1本でも掛かっている極を極番号順でまとめて先頭に出し、
    /// ジャンパの無い極をその後ろに極番号順で並べる。
    /// Jumper order brings every pole that has any jumper to the front (in pole-number order), followed by the poles with no jumper (also in pole-number order).
    #[test]
    fn jumper_order_brings_every_jumpered_pole_to_the_front() {
        let (mut sheet, id) = sheet_with_tb(5, 0);
        let Some(Entity::Symbol(s)) = sheet.entities.get_mut(&id) else { panic!() };
        s.attrs.insert("jumpers".into(), "2-3".into());
        s.attrs.insert("row_order".into(), "jumper".into());
        let chart = terminal_chart(&sheet, id).expect("chart");
        assert_eq!(chart.row_order, RowOrder::Jumper);
        assert_eq!(
            terminal_order(&chart),
            ["2", "3", "1", "4", "5"],
            "ジャンパの掛かった2・3が先頭にまとまり、残りは番号順"
        );
    }

    /// 3段の端子台では、どの並べ替え順を選んでも同じ極の3段 (a/b/c) が並びの中で
    /// 必ず隣り合う (段がバラバラの位置に散らばらない)。
    /// On a 3-level terminal block, whichever row order is chosen, the three levels of one pole always stay next to each other in the resulting order.
    #[test]
    fn every_row_order_keeps_the_three_levels_of_one_pole_adjacent() {
        for spec in ["number", "alnum", "page", "cable", "jumper"] {
            let (mut sheet, id) = sheet_with_multilevel_tb(4, 3, "1a-1b");
            let Some(Entity::Symbol(s)) = sheet.entities.get_mut(&id) else { panic!() };
            s.attrs.insert("row_order".into(), spec.into());
            // ページ順・外部ケーブル順の差が出るよう、一部の端子に配線を足す
            let p = outer_point(&sheet, id, "3a");
            add_wire(&mut sheet, (p.x, p.y), (15.0, 15.0), |_| {});
            let chart = terminal_chart(&sheet, id).expect("chart: {spec}");
            for pole in 1..=4u32 {
                let positions: Vec<usize> = chart
                    .rows
                    .iter()
                    .enumerate()
                    .filter(|(_, r)| parse_terminal_no(&r.terminal).0 == pole)
                    .map(|(i, _)| i)
                    .collect();
                assert_eq!(positions.len(), 3, "{spec}: 極{pole}は3段");
                assert_eq!(
                    positions[2] - positions[0],
                    2,
                    "{spec}: 極{pole}の3段が隣り合っていない ({positions:?})"
                );
            }
        }
    }

    /// チャートCSVの行順は、常にチャート(エディタ)の行順と一致する
    /// (row_orderで並べ替えても、CSVは`chart.rows`をそのまま使うので食い違わない)。
    /// The terminal chart CSV always lists rows in the same order as the chart (the editor), for every row order.
    #[test]
    fn the_csv_row_order_always_matches_the_chart_row_order() {
        let (mut sheet, id) = sheet_with_tb(12, 0);
        let Some(Entity::Symbol(s)) = sheet.entities.get_mut(&id) else { panic!() };
        s.attrs.insert("row_order".into(), "alnum".into());
        let chart = terminal_chart(&sheet, id).expect("chart");
        let csv = terminal_chart_csv(&sheet, id);
        let csv_terminals: Vec<&str> = csv.lines().skip(1).map(|line| line.split(',').next().unwrap()).collect();
        assert_eq!(csv_terminals, terminal_order(&chart));
        assert_eq!(csv_terminals[0..3], ["1", "10", "11"], "英数順がCSVにも反映される");
    }
}
