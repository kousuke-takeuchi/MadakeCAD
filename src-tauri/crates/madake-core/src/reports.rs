//! 帳票生成: BOM(部品表)とFrom-To電線リストのCSV出力。

use std::collections::BTreeMap;

use crate::geometry::Point;
use crate::model::{Entity, Project, Sheet, Wire};
use crate::netlist::{extract_netlist, pin_positions, Net, NetPin, CONNECT_EPS};
use crate::symbol::{sheet_symbol_defs, SymbolDef};

/// 電線リスト・From-To帳票の備考欄に立てる、ネットの相手が**複数**あって1つに絞れなかった
/// ときの印 (G18)。i18n中立な記号 (言語非依存): 表示側で好きな文言に訳し替えられるよう、
/// 固定の英字にする。
pub const REMARK_MULTI_ENDED: &str = "multi";

/// 同じく備考欄の印で、ネットにデバイスのピンが**1つも**無く辿る先が無かったとき (G18)。
/// 多端点 ([`REMARK_MULTI_ENDED`]) とは原因が違うので別の印にする。
pub const REMARK_NO_DEVICE: &str = "none";

/// CSVフィールドのエスケープ(カンマ・引用符・改行を含む場合はクォート)。
pub(crate) fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub(crate) fn csv_row(fields: &[String]) -> String {
    fields
        .iter()
        .map(|f| csv_escape(f))
        .collect::<Vec<_>>()
        .join(",")
}

/// 数値をCSV向けに整形(整数なら小数点なし)。
pub(crate) fn fmt_num(v: f64) -> String {
    if v == 0.0 {
        String::new()
    } else if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v}")
    }
}

/// 部品表の列見出し。
///
/// 先頭の「アイテム番号」は**盤レイアウトのバルーンと同じ番号** (M4仕様 §8)。
/// 盤へ置いた部品だけが埋まり、まだ置いていない部品は空欄になる。
/// この列が入ったぶん、盤レイアウト以前 (4列) のCSVとは列の並びが変わっている。
///
/// 「メーカ」以降の6列は部品カタログ ([`BomCatalog`]。部品DBから引いた表) で「型番/値」を
/// 引き当てた結果 (M4 §5 BOM拡張)。引けない行は空欄になる。この6列が入ったぶん、
/// 5列だった頃のCSVとも非互換。
pub const BOM_COLUMNS: [&str; 11] = [
    "アイテム番号",
    "参照記号",
    "型番/値",
    "シンボル",
    "数量",
    "メーカ",
    "品名",
    "単価",
    "通貨",
    "金額",
    "購入先",
];

/// 部品表の末尾に足す通貨ごとの合計行で、「型番/値」欄に立てる印。
pub const BOM_TOTAL: &str = "合計";

/// 部品カタログの1件 (部品DBの1行のうち部品表が使う欄)。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CatalogEntry {
    pub maker: String,
    /// 品名。
    pub name: String,
    /// 単価 (部品DBの参考価格)。電線は1mあたりとして扱う。
    pub unit_price: Option<f64>,
    pub currency: String,
    pub purchase_url: String,
}

impl From<&crate::parts::Part> for CatalogEntry {
    fn from(part: &crate::parts::Part) -> Self {
        CatalogEntry {
            maker: part.maker.clone(),
            name: part.name.clone(),
            unit_price: part.price,
            currency: part.currency.clone(),
            purchase_url: part.purchase_url.clone(),
        }
    }
}

/// 型番→部品カタログの1件。帳票は部品DBを直接開かず、出力する層 (Tauri・MCP・Link API) が
/// [`crate::parts::PartsDb::bom_catalog`] で作ったこの表を受け取る。空の表なら
/// カタログ由来の欄が空欄になるだけで、部品表はそのまま作れる。
pub type BomCatalog = BTreeMap<String, CatalogEntry>;

/// 部品表で部品カタログを引き当てる型番の一覧 (「型番/値」欄の空でない値。電線集約の
/// `(未割当)`は除く)。[`crate::parts::PartsDb::bom_catalog`] へ渡す。
pub fn bom_part_numbers(project: &Project) -> Vec<String> {
    let mut out: Vec<String> = base_bom_rows(project)
        .into_iter()
        .map(|r| r[2].trim().to_string())
        .filter(|p| !p.is_empty() && p != BOM_WIRE_UNASSIGNED)
        .collect();
    out.sort();
    out.dedup();
    out
}

/// 部品表の行。部品カタログ無し ([`bom_rows_with`] に空の表を渡したのと同じ)。
pub fn bom_rows(project: &Project) -> Vec<Vec<String>> {
    bom_rows_with(project, &BomCatalog::new())
}

/// 部品表の行 ([`BOM_COLUMNS`] と同じ並び)。図面シート化からも使う。
///
/// 先頭5列は図面から: 全シート横断で (型番/値, シンボル) ごとに集計し、参照記号を昇順で
/// 列挙する。アイテム番号は参照記号ごとに [`crate::panel::item_numbers`] から引き、
/// 1行に複数の部品がまとまっているときは番号を小さい順に並べる。電線集約行・端子台付属品行が
/// この順で後ろに続く。
///
/// 後ろ6列は「型番/値」を`catalog`で引き当てる。金額=数量×単価 (電線集約行の数量はmなので
/// 1mあたりの単価として掛ける)。単価が1つでも引けたときだけ、末尾に**通貨ごとの合計行**
/// ([`BOM_TOTAL`]) を足し、単価の無い行があればその数を合計行の品名欄に書く。
pub fn bom_rows_with(project: &Project, catalog: &BomCatalog) -> Vec<Vec<String>> {
    let mut totals: BTreeMap<String, f64> = BTreeMap::new();
    let mut unpriced = 0usize;
    let mut rows: Vec<Vec<String>> = base_bom_rows(project)
        .into_iter()
        .map(|mut row| {
            let hit = catalog.get(row[2].trim()).filter(|_| row[2] != BOM_WIRE_UNASSIGNED);
            let qty = row[4].split_whitespace().next().and_then(|q| q.parse::<f64>().ok());
            let price = hit.and_then(|e| e.unit_price);
            let amount = price.zip(qty).map(|(p, q)| p * q);
            match (amount, hit) {
                (Some(a), Some(e)) => *totals.entry(e.currency.clone()).or_insert(0.0) += a,
                _ => unpriced += 1,
            }
            let e = hit.cloned().unwrap_or_default();
            row.extend([
                e.maker,
                e.name,
                price.map(fmt_money).unwrap_or_default(),
                if hit.is_some() { e.currency } else { String::new() },
                amount.map(fmt_money).unwrap_or_default(),
                e.purchase_url,
            ]);
            row
        })
        .collect();
    let note = if unpriced > 0 { format!("単価未登録 {unpriced} 行を除く") } else { String::new() };
    for (currency, sum) in totals {
        let mut total = vec![String::new(); BOM_COLUMNS.len()];
        total[2] = BOM_TOTAL.into();
        total[6] = note.clone();
        total[8] = currency;
        total[9] = fmt_money(sum);
        rows.push(total);
    }
    rows
}

/// 金額・単価の表記: 小数第2位で丸め、末尾の0と小数点は書かない (`12.50`→`12.5`、`25.00`→`25`)。
fn fmt_money(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// 図面から数えた部品表の先頭5列 (カタログを引く前)。
fn base_bom_rows(project: &Project) -> Vec<Vec<String>> {
    let item_numbers = crate::panel::item_numbers(project);
    let mut groups: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for sheet in &project.sheets {
        for entity in sheet.entities.values() {
            if let Entity::Symbol(s) = entity {
                groups
                    .entry((s.value.clone(), s.symbol_id.clone()))
                    .or_default()
                    .push(s.reference.clone());
            }
        }
    }
    let mut rows: Vec<Vec<String>> = groups
        .into_iter()
        .map(|((value, symbol_id), mut refs)| {
            refs.sort();
            let mut items: Vec<u32> = refs
                .iter()
                .filter_map(|r| item_numbers.get(r.trim()).copied())
                .collect();
            items.sort_unstable();
            items.dedup();
            vec![
                items
                    .iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
                refs.join(", "),
                value,
                symbol_id,
                refs.len().to_string(),
            ]
        })
        .collect();
    rows.extend(wire_rollup_rows(project));
    rows.extend(accessory_rows(project));
    rows
}

/// 付属品行の「シンボル」欄に立てる印 (回路のシンボルではなく端子台付属品の行であることを表す)。
pub const BOM_ACCESSORY_KIND: &str = "端子台付属品";

/// 部品表末尾に足す端子台付属品の行 (M4 §1.1)。全端子台の`attrs["accessories"]`を
/// 型番ごとに数え、「数量」欄へ個数、「参照記号」欄へ使っている端子台を書く。
/// **型番が空の付属品は載せない** (部品表に空行を作らない)。端子台チャートと同じ解析
/// ([`crate::terminal_chart::parse_accessories`]) を通るので、読めない項も数えない。
/// 電線集約行の**後ろ**に付く。
fn accessory_rows(project: &Project) -> Vec<Vec<String>> {
    let mut groups: BTreeMap<String, (usize, Vec<String>)> = BTreeMap::new();
    for sheet in &project.sheets {
        for (id, entity) in &sheet.entities {
            let Entity::Symbol(s) = entity else { continue };
            if !crate::terminal_chart::is_terminal_block(&s.symbol_id) {
                continue;
            }
            let Some(chart) = crate::terminal_chart::terminal_chart(sheet, *id) else {
                continue;
            };
            for a in chart.accessories.iter().filter(|a| !a.part_no.is_empty()) {
                let entry = groups.entry(a.part_no.clone()).or_default();
                entry.0 += 1;
                if !entry.1.contains(&s.reference) {
                    entry.1.push(s.reference.clone());
                }
            }
        }
    }
    groups
        .into_iter()
        .map(|(part_no, (count, mut refs))| {
            refs.sort();
            vec![
                String::new(),
                refs.join(", "),
                part_no,
                BOM_ACCESSORY_KIND.into(),
                count.to_string(),
            ]
        })
        .collect()
}

/// 電線集約行のうち、電線品番が引けなかった配線をまとめる行の「型番/値」欄。
pub const BOM_WIRE_UNASSIGNED: &str = "(未割当)";

/// 電線集約行の「シンボル」欄に立てる印。回路のシンボルではなく配線の集約行であることを表す。
pub const BOM_WIRE_KIND: &str = "電線";

/// 部品表末尾に足す電線集約行 (G9残)。プロジェクト全体の配線を電線品番ごとに合算し、
/// 総使用長を「数量」欄へ m 単位・小数第2位までで書く (電線品番凡例ブロック
/// [`crate::wire_legend`] の使用長と同じ書式)。品番が引けない配線は
/// [`BOM_WIRE_UNASSIGNED`]の1行にまとめる (勝手に品番を作らない。合計が0mなら行を出さない)。
/// 数量欄は部品行と違い**単位付き** (`"3.75 m"`)。個数ではなく長さであることを一目で示すため。
/// 部品行の**後ろ**に付く (呼び出し元の[`bom_rows`])。列数は部品行と同じ5列のまま。
fn wire_rollup_rows(project: &Project) -> Vec<Vec<String>> {
    let mut totals: BTreeMap<String, f64> = BTreeMap::new();
    let mut unassigned = 0.0_f64;
    let mut has_unassigned = false;
    for sheet in &project.sheets {
        for entity in sheet.entities.values() {
            let Entity::Wire(w) = entity else { continue };
            let len = w.length_m.unwrap_or(0.0);
            match crate::wire_parts::wire_part_number(project, w) {
                Some(no) => *totals.entry(no).or_insert(0.0) += len,
                None => {
                    has_unassigned = true;
                    unassigned += len;
                }
            }
        }
    }
    let mut rows: Vec<Vec<String>> = totals
        .into_iter()
        .map(|(part_no, total)| {
            vec![String::new(), String::new(), part_no, BOM_WIRE_KIND.into(), format!("{total:.2} m")]
        })
        .collect();
    // 使用長の入っていない配線しか無いとき「(未割当) 0.00 m」は何も伝えないので出さない
    if has_unassigned && unassigned > 0.0 {
        rows.push(vec![
            String::new(),
            String::new(),
            BOM_WIRE_UNASSIGNED.into(),
            BOM_WIRE_KIND.into(),
            format!("{unassigned:.2} m"),
        ]);
    }
    rows
}

/// 部品表CSV (部品カタログ無し)。見出しは [`BOM_COLUMNS`]。
pub fn bom_csv(project: &Project) -> String {
    bom_csv_with(project, &BomCatalog::new())
}

/// 部品表CSV。見出しは [`BOM_COLUMNS`]、行は [`bom_rows_with`]。
pub fn bom_csv_with(project: &Project, catalog: &BomCatalog) -> String {
    let mut out = BOM_COLUMNS.join(",");
    out.push('\n');
    for row in bom_rows_with(project, catalog) {
        out.push_str(&csv_row(&row));
        out.push('\n');
    }
    out
}

/// From-To電線リストの列見出し。
///
/// **嵌合相手・備考は新設でTo列の直後に入るため、CSVは従来の9列とは非互換**
/// (旧: シート,From,To,線番,線色,線径sq,長さm,電線品番,ハーネス。列位置で読んでいる
/// スクリプトは直す必要がある。`docs/05-wire-management.md`参照)。
pub const WIRE_LIST_COLUMNS: [&str; 11] = [
    "シート",
    "From",
    "To",
    "嵌合相手",
    "備考",
    "線番",
    "線色",
    "線径sq",
    "長さm",
    "電線品番",
    "ハーネス",
];

/// ワイヤ端点1つの解決結果。ピンかネットラベルかを区別する (G18: ラベル端をネット越しに
/// 辿るには、反対端がピンかどうかを先に知る必要があるため`endpoint_label`より詳しい)。
enum EndKind {
    Pin { entity_id: crate::EntityId, reference: String, pin: String },
    Label(String),
    None,
}

fn resolve_endpoint(sheet: &Sheet, defs: &[SymbolDef], at: &Point) -> EndKind {
    let by_id = crate::symbol::DefIndex::new(defs);
    let mut pin_hit: Option<(String, crate::EntityId, String, String)> = None;
    let mut label_hit: Option<String> = None;
    for entity in sheet.entities.values() {
        match entity {
            Entity::Symbol(s) => {
                let Some(def) = by_id.get(s) else {
                    continue;
                };
                for (no, pos) in pin_positions(s, &def) {
                    if pos.distance_to(at) < CONNECT_EPS {
                        let text = format!("{}:{}", s.reference, no);
                        if pin_hit.as_ref().is_none_or(|(cur, ..)| text < *cur) {
                            pin_hit = Some((text, s.id, s.reference.clone(), no));
                        }
                    }
                }
            }
            Entity::NetLabel(l)
                if l.at.distance_to(at) < CONNECT_EPS
                    && label_hit.as_ref().is_none_or(|cur| l.name < *cur) =>
            {
                label_hit = Some(l.name.clone());
            }
            _ => {}
        }
    }
    match (pin_hit, label_hit) {
        (Some((_, entity_id, reference, pin)), _) => EndKind::Pin { entity_id, reference, pin },
        (None, Some(l)) => EndKind::Label(l),
        (None, None) => EndKind::None,
    }
}

/// ワイヤ端点1つの接続先表記。
///
/// - シンボルのピンに一致 → `"参照記号:ピン番号"` (例 `K1:A1`、`TB1:3`)
/// - ネットラベルに一致 → ラベル名 (このネットを辿った解決はしない。辿るのは
///   [`wire_from_to`]の役目 — G18)
/// - どちらでもない (未接続) → 空文字
///
/// 同じ点に複数の候補があるときはピンを優先し、同種の候補同士では辞書順で最小のものを選ぶ
/// (出力を決定的にするため)。貫通端子台の左右の接続点は同じピン番号を持つので、
/// 内側・外側どちらのワイヤも同じ `TB1:1` 表記になる。
pub fn endpoint_label(sheet: &Sheet, defs: &[SymbolDef], at: &Point) -> String {
    match resolve_endpoint(sheet, defs, at) {
        EndKind::Pin { reference, pin, .. } => format!("{reference}:{pin}"),
        EndKind::Label(name) => name,
        EndKind::None => String::new(),
    }
}

/// ラベル端点をネット越しに辿る (G18)。
///
/// `exclude`にはこのワイヤ自身の反対端がピンだったときのそのピン (entity id・ピン番号) を渡す
/// (自分自身に辿り着かないようにするため)。ネットの中に`exclude`を除いてピンが**ちょうど1つ**
/// あれば`"参照記号:ピン番号"`へ解決する。0個(辿る先のデバイスが無い)なら
/// [`REMARK_NO_DEVICE`]、2個以上(多端点で1つに絞れない)なら[`REMARK_MULTI_ENDED`]の印を
/// 添えて、表記はラベル名のまま残す。
fn resolve_label_end(
    nets: &[Net],
    label_name: &str,
    exclude: Option<&(crate::EntityId, String)>,
) -> (String, &'static str) {
    let Some(net) = nets.iter().find(|n| n.label.as_deref() == Some(label_name)) else {
        return (label_name.to_string(), "");
    };
    let others: Vec<&NetPin> = net
        .pins
        .iter()
        .filter(|p| exclude.is_none_or(|(id, pin)| *id != p.entity_id || pin != &p.pin))
        .collect();
    match others.as_slice() {
        [only] => (format!("{}:{}", only.reference, only.pin), ""),
        [] => (label_name.to_string(), REMARK_NO_DEVICE),
        _ => (label_name.to_string(), REMARK_MULTI_ENDED),
    }
}

/// 表記から参照記号部分を取り出す (`"K1:A1"` → `"K1"`)。ラベル名のまま (コロンを含まない)
/// なら解決先のデバイスが無いので嵌合相手も引けない。
fn reference_of(display: &str) -> Option<&str> {
    display.split_once(':').map(|(r, _)| r)
}

/// ワイヤの (From, To, 嵌合相手, 備考)。
///
/// - From/Toは両端の接続先を解決する。ピンに届いた端はそのまま`参照記号:ピン番号`。
///   ネットラベルに届いた端は、そのネットの中でこのワイヤ自身の反対端を除いた
///   ピンが**ちょうど1つ**ならそのデバイスへ辿る (G18)。それ以外はラベル名のまま残る
/// - 表記の辞書順で小さい方をFrom、大きい方をTo。未接続(空文字)の端は常にTo側
/// - 嵌合相手はFrom/Toが指すデバイスのうち、コネクタが`mates_with`を宣言していれば
///   その値をカンマ区切りで列挙する (G14。From側→To側の順。重複は1つにまとめる)
/// - 備考はFrom/Toのどちらかがネットを1対1で辿れなかったときに立つ (G18):
///   多端点で絞れなければ[`REMARK_MULTI_ENDED`] ("multi")、辿る先のデバイスが
///   無ければ[`REMARK_NO_DEVICE`] ("none")。両方当てはまるときは"multi"を優先する
///
/// `nets`はこのシートのネットリスト ([`extract_netlist`]) を呼び出し側から渡す
/// (電線リストは1シートに何十本もワイヤがあるので、シートに1回だけ作る)。
pub fn wire_from_to(
    sheet: &Sheet,
    defs: &[SymbolDef],
    nets: &[Net],
    wire: &Wire,
) -> (String, String, String, String) {
    let (Some(a), Some(b)) = (wire.points.first(), wire.points.last()) else {
        return (String::new(), String::new(), String::new(), String::new());
    };
    let kinds = [resolve_endpoint(sheet, defs, a), resolve_endpoint(sheet, defs, b)];
    let mut multi = false;
    let mut no_device = false;
    let mut resolved: Vec<String> = Vec::with_capacity(2);
    for (i, kind) in kinds.iter().enumerate() {
        let text = match kind {
            EndKind::Pin { reference, pin, .. } => format!("{reference}:{pin}"),
            EndKind::None => String::new(),
            EndKind::Label(name) => {
                let exclude = match &kinds[1 - i] {
                    EndKind::Pin { entity_id, pin, .. } => Some((*entity_id, pin.clone())),
                    _ => None,
                };
                let (text, mark) = resolve_label_end(nets, name, exclude.as_ref());
                multi |= mark == REMARK_MULTI_ENDED;
                no_device |= mark == REMARK_NO_DEVICE;
                text
            }
        };
        resolved.push(text);
    }

    // From/To を決めてから嵌合相手列を組み立てる (描いた向きで並びが変わらないように)
    resolved.sort_by(|x, y| (x.is_empty(), x.as_str()).cmp(&(y.is_empty(), y.as_str())));
    let mut mates: Vec<String> = Vec::new();
    for text in &resolved {
        if let Some(reference) = reference_of(text) {
            let m = crate::mate::declared_mate(sheet, reference);
            if !m.is_empty() && !mates.contains(&m) {
                mates.push(m);
            }
        }
    }

    let [from, to] = [resolved[0].clone(), resolved[1].clone()];
    let remark = if multi {
        REMARK_MULTI_ENDED
    } else if no_device {
        REMARK_NO_DEVICE
    } else {
        ""
    };
    (from, to, mates.join(", "), remark.to_string())
}

/// From-To電線リストの行 ([`WIRE_LIST_COLUMNS`] と同じ並び)。図面シート化からも使う。
/// 行はシート順、シート内はFrom→Toの辞書順で決定的に並ぶ。
pub fn wire_list_rows(project: &Project) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for sheet in &project.sheets {
        let defs = sheet_symbol_defs(sheet);
        // ネットリストはワイヤごとではなくシートごとに1回だけ作る (G18の解決に全ワイヤが使う)
        let nets = extract_netlist(sheet, &defs);
        let mut sheet_rows: Vec<Vec<String>> = sheet
            .entities
            .values()
            .filter_map(|e| match e {
                Entity::Wire(w) => Some(w),
                _ => None,
            })
            .map(|w| {
                let (from, to, mate, remark) = wire_from_to(sheet, &defs, &nets, w);
                vec![
                    sheet.name.clone(),
                    from,
                    to,
                    mate,
                    remark,
                    w.net.clone().unwrap_or_default(),
                    w.color.clone(),
                    fmt_num(w.sq),
                    w.length_m.map(fmt_num).unwrap_or_default(),
                    crate::wire_parts::wire_part_number(project, w).unwrap_or_default(),
                    crate::harness::harness_name_of_wire(sheet, w),
                ]
            })
            .collect();
        sheet_rows.sort_by(|a, b| (&a[1], &a[2], &a[5]).cmp(&(&b[1], &b[2], &b[5])));
        rows.extend(sheet_rows);
    }
    rows
}

/// From-To電線リストCSV。Wireごとに1行で、両端の接続先 (From/To)・嵌合相手・備考・
/// 線番・電線属性・ハーネスを並べる。
/// 長さ列は測長済みのワイヤだけ埋まる (3D配線からの書き戻しはM5)。
/// 電線品番の列は、ワイヤに書かれた品番か、無ければ線色+線径で電線品番表
/// ([`crate::wire_parts`]) を引いた品番で埋まる。
pub fn wire_list_csv(project: &Project) -> String {
    let mut out = WIRE_LIST_COLUMNS.join(",");
    out.push('\n');
    for row in wire_list_rows(project) {
        out.push_str(&csv_row(&row));
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::*;
    use uuid::Uuid;

    fn sym(reference: &str, value: &str) -> Entity {
        Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "resistor".into(),
            at: Point::new(0.0, 0.0),
            rotation: 0,
            mirror: false,
            reference: reference.into(),
            value: value.into(),
            attrs: Default::default(),
        })
    }

    /// ピン1が(x-7.5, y)、ピン2が(x+7.5, y)にある抵抗。
    fn resistor(reference: &str, x: f64, y: f64) -> Entity {
        let mut e = sym(reference, "");
        if let Entity::Symbol(s) = &mut e {
            s.at = Point::new(x, y);
        }
        e
    }

    fn wire_between(a: (f64, f64), b: (f64, f64)) -> Entity {
        Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(a.0, a.1), Point::new(b.0, b.1)],
            color: "black".into(),
            sq: 0.3,
            length_m: None,
            length_source: Default::default(),
            part_no: None,
            net: None,
        })
    }

    /// 未接続のワイヤ1本だけのプロジェクト。属性はクロージャで調整する。
    fn one_wire_project(f: impl FnOnce(&mut Wire)) -> Project {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        let mut e = wire_between((0.0, 0.0), (10.0, 0.0));
        if let Entity::Wire(w) = &mut e {
            f(w);
        }
        sheet.entities.insert(e.id(), e);
        project
    }

    /// CSVの見出し行を除く各行を、カンマ区切りのセル列へ分解する (テストの想定にクォートは出てこない)。
    fn data_rows(csv: &str) -> Vec<Vec<String>> {
        csv.lines()
            .skip(1)
            .map(|l| l.split(',').map(|c| c.to_string()).collect())
            .collect()
    }

    /// The BOM groups symbols by part number and counts quantities per group.
    /// 部品表はシンボルを型番でまとめ、数量を集計する。
    #[test]
    fn bom_groups_by_value_and_counts() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        for (r, v) in [("R1", "10k"), ("R2", "10k"), ("K1", "RLY-24V")] {
            let e = sym(r, v);
            sheet.entities.insert(e.id(), e);
        }
        let csv = bom_csv(&project);
        let lines: Vec<_> = csv.lines().collect();
        assert_eq!(lines[0], "アイテム番号,参照記号,型番/値,シンボル,数量,メーカ,品名,単価,通貨,金額,購入先");
        assert!(
            lines.iter().any(|l| l.contains("R1, R2") && l.contains("10k") && l.ends_with(",2,,,,,,")),
            "10kが2個に集計される (カタログの無い6列は空欄): {csv}"
        );
        assert!(lines.iter().any(|l| l.contains("RLY-24V") && l.ends_with(",1,,,,,,")));
    }

    /// The parts list carries the panel item number as its first column, so the balloons on the panel drawing and the parts list line up.
    /// 部品表の先頭列は盤のアイテム番号なので、盤図面のバルーンと部品表が突き合わせられる。
    #[test]
    fn bom_starts_with_the_panel_item_number() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        for (r, v) in [("K1", "RLY-24V"), ("K9", "RLY-24V")] {
            let e = sym(r, v);
            project.sheet_mut(sid).unwrap().entities.insert(e.id(), e);
        }
        let mut panel = Sheet::new_kind("盤", PaperSize::A3, Orientation::Landscape, SheetKind::Panel);
        let fp = Entity::Footprint(Footprint {
            id: Uuid::new_v4(),
            at: Point::new(100.0, 100.0),
            w: 20.0,
            h: 40.0,
            rotation: 0,
            reference: "K1".into(),
            part_no: "RLY-24V".into(),
            item_no: Some(3),
        });
        panel.entities.insert(fp.id(), fp);
        project.sheets.push(panel);

        assert_eq!(BOM_COLUMNS[0], "アイテム番号");
        assert_eq!(BOM_COLUMNS.len(), 11, "アイテム番号とカタログ6列のぶん旧4列・旧5列のCSVとは非互換");
        let row = bom_rows(&project)
            .into_iter()
            .find(|r| r[1].contains("K1"))
            .expect("K1の行");
        assert_eq!(row[0], "3", "盤に置いた部品はバルーンの番号が入る");
        assert_eq!(row[1], "K1, K9");
    }

    /// Parts that are not on a panel yet leave the item number column empty; the rest of the parts list is unaffected.
    /// まだ盤に置いていない部品はアイテム番号の欄が空欄になり、部品表の他の欄はそのまま出る。
    #[test]
    fn bom_leaves_the_item_number_empty_for_unplaced_parts() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let e = sym("R1", "10k");
        project.sheet_mut(sid).unwrap().entities.insert(e.id(), e);
        let row = bom_rows(&project).into_iter().next().expect("1行");
        assert_eq!(row[0], "");
        assert_eq!(&row[1..5], ["R1", "10k", "resistor", "1"]);
    }

    /// The BOM has no wire roll-up rows at all when the project has no wires (a project with only components is unaffected).
    /// 配線の無いプロジェクトには電線集約行が1行も付かない (部品だけの部品表はそのまま)。
    #[test]
    fn bom_has_no_wire_rollup_rows_without_any_wires() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let e = sym("R1", "10k");
        project.sheet_mut(sid).unwrap().entities.insert(e.id(), e);
        assert_eq!(bom_rows(&project).len(), 1, "電線集約行は付かない");
    }

    /// After the component rows, the BOM adds one row per wire part number with the project-wide total length (in metres, 2 decimals) in the quantity column.
    /// 部品行の後ろに、電線品番ごとの集約行が付き、数量欄にプロジェクト全体の使用長合計 (m単位・小数第2位まで) が入る。
    #[test]
    fn bom_rolls_up_wires_by_part_number_after_the_component_rows() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        for (len, part_no) in [(1.5, "SAMPLE0001"), (2.25, "SAMPLE0001")] {
            let mut e = wire_between((0.0, 0.0), (10.0, 0.0));
            if let Entity::Wire(w) = &mut e {
                w.length_m = Some(len);
                w.part_no = Some(part_no.into());
            }
            sheet.entities.insert(e.id(), e);
        }
        let rows = bom_rows(&project);
        let roll_up = rows.last().expect("集約行がある");
        assert_eq!(roll_up[2], "SAMPLE0001");
        assert_eq!(roll_up[3], BOM_WIRE_KIND);
        assert_eq!(roll_up[4], "3.75 m", "1.5+2.25の合計を小数第2位まで");
    }

    /// Wires whose part number cannot be resolved roll up under a single "(unassigned)" row, instead of being invented or dropped.
    /// 品番を引けない配線は「(未割当)」の1行にまとまり、勝手に品番を作ったり黙って消えたりしない。
    #[test]
    fn bom_rolls_up_unresolved_wires_under_one_unassigned_row() {
        let project = one_wire_project(|w| w.length_m = Some(4.0));
        let rows = bom_rows(&project);
        let roll_up = rows.last().expect("集約行がある");
        assert_eq!(roll_up[2], BOM_WIRE_UNASSIGNED);
        assert_eq!(roll_up[4], "4.00 m");
    }

    /// 端子台TB(n極)に付属品の指定を付けた記号。
    fn entry(maker: &str, name: &str, price: Option<f64>, currency: &str) -> CatalogEntry {
        CatalogEntry {
            maker: maker.into(),
            name: name.into(),
            unit_price: price,
            currency: currency.into(),
            purchase_url: format!("https://example.com/{name}"),
        }
    }

    fn priced_wire(part: &str, length_m: f64) -> Entity {
        Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)],
            color: "red".into(),
            sq: 0.75,
            length_m: Some(length_m),
            length_source: Default::default(),
            part_no: Some(part.into()),
            net: None,
        })
    }

    /// 型番SAMPLE-Aの抵抗2本 (R1, R2)・型番SAMPLE-Bの抵抗1本 (R3)・電線SAMPLE-W 12.3m を置いたプロジェクト。
    fn priced_project() -> Project {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        for e in [sym("R1", "SAMPLE-A"), sym("R2", "SAMPLE-A"), sym("R3", "SAMPLE-B"), priced_wire("SAMPLE-W", 12.3)] {
            sheet.entities.insert(e.id(), e);
        }
        project
    }

    fn row_of<'a>(rows: &'a [Vec<String>], part: &str) -> &'a Vec<String> {
        rows.iter().find(|r| r[2] == part).unwrap_or_else(|| panic!("{part}の行: {rows:?}"))
    }

    /// Each BOM row looks its part number up in the parts catalog and shows the maker, name, unit price, currency, amount (quantity times unit price) and where to buy it.
    /// 部品表の各行は型番を部品カタログで引き当て、メーカ・品名・単価・通貨・金額 (数量×単価)・購入先を表示する。
    #[test]
    fn bom_fills_maker_name_price_and_amount_from_the_catalog() {
        let catalog: BomCatalog = [("SAMPLE-A".to_string(), entry("MakerA", "Resistor A", Some(12.5), "JPY"))].into();
        let rows = bom_rows_with(&priced_project(), &catalog);
        assert_eq!(BOM_COLUMNS.len(), 11);
        let a = row_of(&rows, "SAMPLE-A");
        assert_eq!(
            &a[4..],
            ["2", "MakerA", "Resistor A", "12.5", "JPY", "25", "https://example.com/Resistor A"]
        );
    }

    /// A part number the catalog does not know keeps its BOM row; only the catalog columns stay empty.
    /// 部品カタログに無い型番も部品表の行は残り、カタログ由来の欄だけが空欄になる。
    #[test]
    fn a_part_missing_from_the_catalog_keeps_its_row_with_empty_catalog_columns() {
        let rows = bom_rows_with(&priced_project(), &BomCatalog::new());
        let b = row_of(&rows, "SAMPLE-B");
        assert_eq!(b.len(), BOM_COLUMNS.len());
        assert_eq!(&b[1..5], ["R3", "SAMPLE-B", "resistor", "1"]);
        assert!(b[5..].iter().all(String::is_empty), "{b:?}");
        assert!(rows.iter().all(|r| r[2] != BOM_TOTAL), "単価が無ければ合計行は出ない");
    }

    /// A wire's unit price is per metre: the amount of a wire roll-up row is its total length in metres times the unit price.
    /// 電線の単価は1mあたりとして扱い、電線集約行の金額は合計の長さ (m) × 単価になる。
    #[test]
    fn a_wire_roll_up_row_is_priced_per_metre() {
        let catalog: BomCatalog = [("SAMPLE-W".to_string(), entry("MakerW", "Wire", Some(50.0), "JPY"))].into();
        let rows = bom_rows_with(&priced_project(), &catalog);
        let w = row_of(&rows, "SAMPLE-W");
        assert_eq!(w[4], "12.30 m");
        assert_eq!(w[7], "50");
        assert_eq!(w[9], "615");
    }

    /// When at least one row has a price, the BOM ends with one total row per currency, and says how many rows were left out for having no price.
    /// 単価のある行が1つでもあれば、部品表の末尾に通貨ごとの合計行が付き、単価が無くて合計から除いた行の数も書かれる。
    #[test]
    fn the_bom_ends_with_a_total_per_currency_noting_rows_without_a_price() {
        let catalog: BomCatalog = [
            ("SAMPLE-A".to_string(), entry("MakerA", "Resistor A", Some(12.5), "JPY")),
            ("SAMPLE-B".to_string(), entry("MakerB", "Resistor B", Some(1.25), "USD")),
            ("SAMPLE-W".to_string(), entry("MakerW", "Wire", None, "JPY")),
        ]
        .into();
        let rows = bom_rows_with(&priced_project(), &catalog);
        let totals: Vec<&Vec<String>> = rows.iter().filter(|r| r[2] == BOM_TOTAL).collect();
        assert_eq!(totals.len(), 2, "{rows:?}");
        assert_eq!((totals[0][8].as_str(), totals[0][9].as_str()), ("JPY", "25"));
        assert_eq!((totals[1][8].as_str(), totals[1][9].as_str()), ("USD", "1.25"));
        assert_eq!(totals[0][6], "単価未登録 1 行を除く");
        assert!(rows.len() >= 2 && rows[rows.len() - 2..].iter().all(|r| r[2] == BOM_TOTAL), "合計行は末尾");
    }

    /// The part numbers looked up in the parts catalog are every non-empty part number in the BOM, once each, without the "(unassigned)" wire row.
    /// 部品カタログで引き当てる型番は、部品表に出る空でない型番を1回ずつで、電線の「(未割当)」行は含まない。
    #[test]
    fn the_part_numbers_to_look_up_skip_blanks_and_the_unassigned_wire_row() {
        let mut project = priced_project();
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        let mut loose = priced_wire("x", 2.0);
        if let Entity::Wire(w) = &mut loose {
            w.part_no = None;
            w.color = String::new();
        }
        for e in [sym("R9", ""), loose] {
            sheet.entities.insert(e.id(), e);
        }
        assert_eq!(bom_part_numbers(&project), ["SAMPLE-A", "SAMPLE-B", "SAMPLE-W"]);
    }

    /// The BOM CSV has the same eleven columns as the BOM drawing sheet, with the catalog columns filled in.
    /// 部品表CSVは部品表の図面シートと同じ11列で、カタログ由来の欄が埋まる。
    #[test]
    fn the_bom_csv_carries_the_catalog_columns() {
        let catalog: BomCatalog = [("SAMPLE-A".to_string(), entry("MakerA", "Resistor A", Some(12.5), "JPY"))].into();
        let csv = bom_csv_with(&priced_project(), &catalog);
        let mut lines = csv.lines();
        assert_eq!(lines.next().unwrap(), BOM_COLUMNS.join(","));
        assert!(
            csv.contains(",\"R1, R2\",SAMPLE-A,resistor,2,MakerA,Resistor A,12.5,JPY,25,https://example.com/Resistor A\n"),
            "{csv}"
        );
    }

    fn tb_with_accessories(reference: &str, poles: usize, spec: &str) -> Entity {
        let mut attrs = std::collections::BTreeMap::new();
        attrs.insert("accessories".to_string(), spec.to_string());
        Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: format!("terminal_block_{poles}p"),
            at: Point::new(100.0, 100.0),
            rotation: 0,
            mirror: false,
            reference: reference.into(),
            value: "SAMPLE0001".into(),
            attrs,
        })
    }

    /// Terminal block accessories appear in the BOM as one row per part number with how many are used, listing the terminal blocks that use them, after the wire roll-up rows.
    /// 端子台の付属品は部品表に型番ごとの1行として載り、数量欄に使用個数、参照記号欄に使っている端子台が入り、電線集約行の後ろに来る。
    #[test]
    fn bom_counts_terminal_accessories_by_part_number() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        for e in [
            tb_with_accessories("TB1", 4, "1:end_bracket:SAMPLE-EB,end:end_bracket:SAMPLE-EB,3:partition:SAMPLE-PT"),
            tb_with_accessories("TB2", 2, "end:end_bracket:SAMPLE-EB"),
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let rows = bom_rows(&project);
        let acc: Vec<&Vec<String>> = rows.iter().filter(|r| r[3] == BOM_ACCESSORY_KIND).collect();
        assert_eq!(acc.len(), 2);
        assert_eq!(&acc[0][1..5], ["TB1, TB2", "SAMPLE-EB", BOM_ACCESSORY_KIND, "3"]);
        assert_eq!(&acc[1][1..5], ["TB1", "SAMPLE-PT", BOM_ACCESSORY_KIND, "1"]);
    }

    /// An accessory without a part number, or one the terminal check cannot read, adds no row to the BOM.
    /// 型番の無い付属品や、端子台チェックが読めない付属品の項は、部品表に行を作らない。
    #[test]
    fn bom_skips_accessories_without_a_part_number_or_that_cannot_be_read() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let e = tb_with_accessories("TB1", 2, "1:end_bracket,9:end_plate:SAMPLE-EP,2:clip:SAMPLE-X");
        project.sheet_mut(sid).unwrap().entities.insert(e.id(), e);
        let rows = bom_rows(&project);
        assert!(rows.iter().all(|r| r[3] != BOM_ACCESSORY_KIND), "{rows:?}");
    }

    /// When the only wires whose part number cannot be resolved have no length, the BOM adds no "(unassigned) 0.00 m" row.
    /// 品番を引けない配線に使用長が1本も入っていないときは、「(未割当) 0.00 m」の行を出さない。
    #[test]
    fn bom_adds_no_unassigned_row_when_those_wires_have_no_length() {
        let project = one_wire_project(|w| w.length_m = None);
        let rows = bom_rows(&project);
        assert!(rows.iter().all(|r| r[2] != BOM_WIRE_UNASSIGNED), "{rows:?}");
    }

    /// BOM fields containing commas are quoted so the CSV stays valid.
    /// カンマを含む項目は引用符で囲まれ、CSVが壊れない。
    #[test]
    fn bom_escapes_fields_with_commas() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        let e = sym("K1", "RLY-24V, 24VDC");
        sheet.entities.insert(e.id(), e);
        let csv = bom_csv(&project);
        assert!(csv.contains("\"RLY-24V, 24VDC\""), "カンマ入りはクォート: {csv}");
    }

    /// The wire list is a From-To list: every row starts with the sheet name and the two ends of the wire, followed by the mate and remark columns (G14/G18), and the older column order (sheet, wire number, harness, ...) is no longer produced.
    /// 電線リストはFrom-To形式で、各行はシート名とワイヤ両端の接続先、続いて嵌合相手・備考列 (G14/G18) が並ぶ。旧来の列順 (シート・線番・ハーネス…) はもう出力されない。
    #[test]
    fn wire_list_is_a_from_to_list() {
        let project = one_wire_project(|_| {});
        let csv = wire_list_csv(&project);
        assert_eq!(
            csv.lines().next().unwrap(),
            "シート,From,To,嵌合相手,備考,線番,線色,線径sq,長さm,電線品番,ハーネス"
        );
        assert_eq!(WIRE_LIST_COLUMNS.len(), 11);
    }

    /// An end of a wire that lands on a symbol pin is written as "reference:pin number" (for example K1:A1).
    /// シンボルのピンに届いているワイヤの端は「参照記号:ピン番号」(例 K1:A1) と書かれる。
    #[test]
    fn a_wire_end_on_a_pin_is_written_as_reference_and_pin_number() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        // R1のピン2 (x=107.5) とR2のピン1 (x=142.5) を1本のワイヤで結ぶ
        for e in [
            resistor("R1", 100.0, 50.0),
            resistor("R2", 150.0, 50.0),
            wire_between((107.5, 50.0), (142.5, 50.0)),
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let row = data_rows(&wire_list_csv(&project))[0].clone();
        assert_eq!(&row[1..3], ["R1:2", "R2:1"]);
    }

    /// A wire end on a block pin is written with the pin's name (for example CTRL1:OUT1), and the block itself is counted in the parts list.
    /// ブロックのピンに届いたワイヤの端はピン名つき(例 CTRL1:OUT1)で書かれ、ブロック自体も部品表に数えられる。
    #[test]
    fn a_wire_end_on_a_block_pin_uses_the_pin_name_and_the_block_is_in_the_parts_list() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        // 幅20mmのブロック: 右ピンOUT1は (112.5, 50)。R1のピン1 (142.5, 50) へ配線
        for e in [
            crate::block::test_block(
                "CTRL1",
                "MainController",
                100.0,
                50.0,
                r#"{"left":[{"no":"IN1","name":"入力1"}],"right":[{"no":"OUT1","name":"出力1"}]}"#,
            ),
            resistor("R1", 150.0, 50.0),
            wire_between((112.5, 50.0), (142.5, 50.0)),
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let row = data_rows(&wire_list_csv(&project))[0].clone();
        assert_eq!(&row[1..3], ["CTRL1:OUT1", "R1:1"]);
        let bom = bom_rows(&project);
        assert!(
            bom.iter()
                .any(|r| r[1] == "CTRL1" && r[2] == "MainController" && r[3] == "block" && r[4] == "1"),
            "部品表にブロックが無い: {bom:?}"
        );
    }

    /// An end of a wire that carries a net label is written as the label name.
    /// ネットラベルが付いているワイヤの端は、そのラベル名で書かれる。
    #[test]
    fn a_wire_end_with_a_net_label_is_written_as_the_label_name() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        let label = Entity::NetLabel(NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(0.0, 0.0),
            name: "24V".into(),
            rotation: 0,
            attrs: Default::default(),
        });
        for e in [
            resistor("R1", 20.0, 0.0),
            wire_between((0.0, 0.0), (12.5, 0.0)),
            label,
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let row = data_rows(&wire_list_csv(&project))[0].clone();
        assert_eq!(&row[1..3], ["24V", "R1:1"]);
    }

    fn net_label(name: &str, x: f64, y: f64) -> Entity {
        Entity::NetLabel(NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(x, y),
            name: name.into(),
            rotation: 0,
            attrs: Default::default(),
        })
    }

    /// A wire end on a net label resolves through the net to the single device pin on the other side (G18), when exactly one other pin belongs to that net.
    /// ネットラベルに届いたワイヤの端は、そのネットに他のピンがちょうど1つだけあれば、そのデバイスのピンまで辿って解決される (G18)。
    #[test]
    fn a_label_end_resolves_to_the_one_to_one_device() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        for e in [
            resistor("R1", 20.0, 0.0),
            wire_between((0.0, 0.0), (12.5, 0.0)),
            net_label("N1", 0.0, 0.0),
            resistor("R2", 100.0, 0.0),
            wire_between((92.5, 0.0), (80.0, 0.0)),
            net_label("N1", 80.0, 0.0),
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let rows = data_rows(&wire_list_csv(&project));
        let ends: Vec<(String, String)> = rows.iter().map(|r| (r[1].clone(), r[2].clone())).collect();
        assert!(ends.contains(&("R1:1".to_string(), "R2:1".to_string())), "{ends:?}");
        for row in &rows {
            assert_eq!(row[4], "", "1対1で解決できたので備考は空欄: {row:?}");
        }
    }

    /// The net still resolves correctly when it is stitched together purely by two same-named net label instances with no direct wire between them (a chain through two labels), and both wires in the chain independently resolve to the device across the chain.
    /// 直接の配線が無くても同名のネットラベル2つだけで繋がったネット (2つのラベルを介した鎖) でも正しく解決され、鎖の両側のワイヤがそれぞれ反対側のデバイスへ解決される。
    #[test]
    fn the_net_chains_through_two_same_named_labels() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        for e in [
            resistor("R1", 20.0, 0.0),
            wire_between((0.0, 0.0), (12.5, 0.0)),
            net_label("CHAIN", 0.0, 0.0),
            resistor("R2", 500.0, 500.0),
            wire_between((492.5, 500.0), (480.0, 500.0)),
            net_label("CHAIN", 480.0, 500.0),
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let rows = data_rows(&wire_list_csv(&project));
        let ends: Vec<(String, String)> = rows.iter().map(|r| (r[1].clone(), r[2].clone())).collect();
        assert_eq!(rows.len(), 2, "{rows:?}");
        assert!(ends.contains(&("R1:1".to_string(), "R2:1".to_string())), "R1側の鎖: {ends:?}");
    }

    /// When a net label's net has more than one other pin, the label end keeps the net name instead of picking one device, and the remark column marks the row as multi-ended (G18).
    /// ネットラベルのネットに他のピンが複数あるときは、ラベル端は1つのデバイスを選ばずネット名のまま残り、備考列がその行を多端点として印す (G18)。
    #[test]
    fn a_label_end_with_multiple_other_pins_keeps_the_label_and_marks_the_remark() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        for e in [
            resistor("R1", 20.0, 0.0),
            wire_between((0.0, 0.0), (12.5, 0.0)),
            net_label("N2", 0.0, 0.0),
            resistor("R2", 100.0, 0.0),
            wire_between((92.5, 0.0), (80.0, 0.0)),
            net_label("N2", 80.0, 0.0),
            resistor("R3", 200.0, 0.0),
            wire_between((192.5, 0.0), (180.0, 0.0)),
            net_label("N2", 180.0, 0.0),
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let rows = data_rows(&wire_list_csv(&project));
        let row = rows.iter().find(|r| r[1] == "N2" || r[2] == "N2").expect("N2を含む行");
        assert!(row[1] == "N2" || row[2] == "N2", "{row:?}");
        assert_eq!(row[4], REMARK_MULTI_ENDED, "多端点は備考にmulti: {row:?}");
    }

    /// A net that is entirely label-to-label with no device pin anywhere leaves the label name in place on both ends and remarks `none` (nothing to reach), which is a different reason from the multi-ended one.
    /// どこにもデバイスのピンが無い、ラベル同士だけのネットは両端ともラベル名のまま残り、備考は多端点とは別の「none」(辿る先が無い) になる。
    #[test]
    fn a_label_to_label_net_with_no_device_keeps_the_label_name() {
        let project = one_wire_project(|_| {});
        let sid = project.sheets[0].id;
        let mut project = project;
        let sheet = project.sheet_mut(sid).unwrap();
        for e in [net_label("N3", 0.0, 0.0), net_label("N3", 10.0, 0.0)] {
            sheet.entities.insert(e.id(), e);
        }
        let row = data_rows(&wire_list_csv(&project))[0].clone();
        assert_eq!(&row[1..3], ["N3", "N3"], "{row:?}");
        assert_eq!(row[4], REMARK_NO_DEVICE, "{row:?}");
    }

    fn connector(reference: &str, mate: Option<&str>, x: f64, y: f64) -> Entity {
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

    /// A wire ending on a connector that declares a mate carries the mate's reference designator in the wire list's mate column (G14).
    /// 嵌合相手を宣言したコネクタに届くワイヤは、電線リストの嵌合相手列にその参照記号を載せる (G14)。
    #[test]
    fn wire_list_carries_the_declared_mate() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        for e in [
            connector("J1", Some("J12"), 20.0, 0.0),
            resistor("R1", 0.0, 0.0),
            wire_between((7.5, 0.0), (12.5, 0.0)),
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let row = data_rows(&wire_list_csv(&project))[0].clone();
        assert_eq!(row[3], "J12");
    }

    /// A wire whose two ends are both connectors with their own declared mates lists both mates in the same order however the wire was drawn (G14).
    /// 両端とも嵌合相手を宣言したコネクタに届くワイヤは、どちら向きに描いても嵌合相手列に同じ並びで両方が載る (G14)。
    #[test]
    fn the_mate_column_does_not_depend_on_the_direction_the_wire_was_drawn() {
        let mate_column = |a: (f64, f64), b: (f64, f64)| {
            let mut project = Project::new("t");
            let sid = project.sheets[0].id;
            let sheet = project.sheet_mut(sid).unwrap();
            for e in [
                connector("J1", Some("J11"), 20.0, 0.0),
                connector("J2", Some("J22"), 60.0, 0.0),
                wire_between(a, b),
            ] {
                sheet.entities.insert(e.id(), e);
            }
            // 嵌合相手列はカンマ区切りの値を持つので、CSVの引用を挟まない行データで見る
            wire_list_rows(&project)[0][3].clone()
        };
        let forward = mate_column((12.5, 0.0), (52.5, 0.0));
        let backward = mate_column((52.5, 0.0), (12.5, 0.0));
        assert_eq!(forward, backward, "描いた向きで並びが変わらない");
        assert_eq!(forward, "J11, J22", "From側→To側の順");
    }

    /// A wire between two devices that neither is a connector with a declared mate leaves the mate column empty.
    /// どちらも嵌合相手を宣言したコネクタではない2つのデバイス間のワイヤは、嵌合相手列が空欄のままになる。
    #[test]
    fn wire_list_mate_column_is_empty_without_a_declaration() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        for e in [resistor("R1", 0.0, 0.0), resistor("R2", 50.0, 0.0), wire_between((7.5, 0.0), (42.5, 0.0))] {
            sheet.entities.insert(e.id(), e);
        }
        let row = data_rows(&wire_list_csv(&project))[0].clone();
        assert_eq!(row[3], "");
    }

    /// An end of a wire that touches nothing is left empty in the list.
    /// 何にも接続していないワイヤの端は、リストでは空欄になる。
    #[test]
    fn an_unconnected_wire_end_is_empty() {
        let project = one_wire_project(|_| {});
        let row = data_rows(&wire_list_csv(&project))[0].clone();
        assert_eq!(&row[1..3], ["", ""], "両端とも未接続なら From/To とも空欄");
    }

    /// Of the two ends, the one whose text sorts first alphabetically becomes From, so the same wire always yields the same From and To no matter which end was drawn first.
    /// 両端のうち表記の辞書順で小さい方がFromになるので、どちら向きに描いたワイヤでもFrom/Toは常に同じになる。
    #[test]
    fn from_is_the_alphabetically_smaller_of_the_two_ends() {
        for (a, b) in [((107.5, 50.0), (142.5, 50.0)), ((142.5, 50.0), (107.5, 50.0))] {
            let mut project = Project::new("t");
            let sid = project.sheets[0].id;
            let sheet = project.sheet_mut(sid).unwrap();
            for e in [
                resistor("R1", 100.0, 50.0),
                resistor("R2", 150.0, 50.0),
                wire_between(a, b),
            ] {
                sheet.entities.insert(e.id(), e);
            }
            let row = data_rows(&wire_list_csv(&project))[0].clone();
            assert_eq!(&row[1..3], ["R1:2", "R2:1"], "描いた向きで結果が変わらない");
        }
    }

    /// When only one end is connected, that end is always From and the empty one is always To.
    /// 片側だけ接続しているワイヤでは、接続している方が必ずFrom、空欄の方が必ずToになる。
    #[test]
    fn a_connected_end_becomes_from_and_the_unconnected_end_becomes_to() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        for e in [
            resistor("R1", 100.0, 50.0),
            wire_between((107.5, 50.0), (140.0, 50.0)),
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let row = data_rows(&wire_list_csv(&project))[0].clone();
        assert_eq!(&row[1..3], ["R1:2", ""]);
    }

    /// Both sides of a feed-through terminal share one terminal number, so wires on the inside and the outside of the same terminal are both written as "TB1:1".
    /// 貫通端子の左右は同じ端子番号なので、同じ端子の内側・外側につながる電線はどちらも「TB1:1」と書かれる。
    #[test]
    fn both_sides_of_a_feed_through_terminal_use_the_same_terminal_number() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        // 2極端子台TB1を(100,50)へ。端子1は y=47.5、左右の接続点は x=97.5 / 102.5
        let tb = Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: "terminal_block_2p".into(),
            at: Point::new(100.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "TB1".into(),
            value: String::new(),
            attrs: Default::default(),
        });
        for e in [
            tb,
            resistor("R1", 80.0, 47.5),
            wire_between((87.5, 47.5), (97.5, 47.5)),
            wire_between((102.5, 47.5), (120.0, 47.5)),
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let rows = data_rows(&wire_list_csv(&project));
        let ends: Vec<_> = rows.iter().map(|r| (r[1].clone(), r[2].clone())).collect();
        assert!(ends.contains(&("R1:2".to_string(), "TB1:1".to_string())), "内側: {ends:?}");
        assert!(ends.contains(&("TB1:1".to_string(), String::new())), "外側: {ends:?}");
    }

    /// The wire list keeps the wire-number column introduced with wire numbering: it holds the number written on the wire, or is empty when the wire is not numbered yet.
    /// 電線リストには線番採番で入った線番の列があり、そのワイヤに書かれた線番が入る (未採番なら空欄)。
    #[test]
    fn wire_list_has_a_wire_number_column() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        for (net, y) in [(Some("12".to_string()), 0.0), (None, 20.0)] {
            let mut e = wire_between((0.0, y), (10.0, y));
            if let Entity::Wire(w) = &mut e {
                w.net = net;
            }
            sheet.entities.insert(e.id(), e);
        }
        let numbers: Vec<String> = data_rows(&wire_list_csv(&project))
            .iter()
            .map(|r| r[5].clone())
            .collect();
        assert!(numbers.contains(&"12".to_string()), "線番12の行: {numbers:?}");
        assert!(numbers.contains(&String::new()), "未採番は空欄: {numbers:?}");
    }

    /// The wire list keeps the harness column: it carries the name of the harness boundary that fully encloses the wire, and stays empty for wires outside every harness.
    /// 電線リストにはハーネス列があり、そのワイヤを完全に囲んでいるハーネス境界の名前が入る (どの囲みにも入らない線は空欄)。
    #[test]
    fn wire_list_has_a_harness_column() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        let mut inside = wire_between((60.0, 60.0), (140.0, 60.0));
        if let Entity::Wire(w) = &mut inside {
            w.net = Some("1".into());
        }
        let mut outside = wire_between((200.0, 200.0), (250.0, 200.0));
        if let Entity::Wire(w) = &mut outside {
            w.net = Some("2".into());
        }
        let h = Entity::Harness(Harness {
            id: Uuid::new_v4(),
            points: crate::harness::rect_points(Point::new(50.0, 50.0), Point::new(150.0, 100.0)),
            name: "W1".into(),
            note: String::new(),
        });
        for e in [inside, outside, h] {
            sheet.entities.insert(e.id(), e);
        }
        let rows = data_rows(&wire_list_csv(&project));
        let by_no = |no: &str| {
            rows.iter().find(|r| r[5] == no).unwrap_or_else(|| panic!("線番{no}の行: {rows:?}")).clone()
        };
        assert_eq!(by_no("1")[10], "W1", "囲まれた線");
        assert_eq!(by_no("2")[10], "", "囲みの外は空欄");
    }

    /// The wire list contains each wire's color, gauge, length and part number.
    /// 電線リストには各ワイヤの線色・線径・長さ・品番が載る。
    #[test]
    fn wire_list_contains_attributes() {
        let project = one_wire_project(|w| {
            w.color = "red".into();
            w.sq = 0.75;
            w.length_m = Some(0.4);
            w.part_no = Some("SAMPLE0001".into());
        });
        let row = data_rows(&wire_list_csv(&project))[0].clone();
        assert_eq!(&row[6..11], ["red", "0.75", "0.4", "SAMPLE0001", ""]);
    }

    /// The part-number column of the wire list is filled from the project's wire-part table (colour + gauge), so wires never have to be given a part number one by one.
    /// 電線リストの品番列はプロジェクトの電線品番表 (線色+線径) から埋まるので、ワイヤ1本ずつ品番を書く必要はない。
    #[test]
    fn wire_list_part_number_comes_from_the_wire_part_table() {
        let mut project = one_wire_project(|w| {
            w.color = "水色".into();
            w.sq = 0.3;
        });
        project.wire_parts = vec![WirePart {
            part_no: "SAMPLE0001".into(),
            color: "水色".into(),
            sq: 0.3,
            note: String::new(),
        }];
        assert_eq!(data_rows(&wire_list_csv(&project))[0][9], "SAMPLE0001");
    }

    /// A combination that is not in the wire-part table leaves the part-number column empty.
    /// 電線品番表に無い線色+線径の組み合わせは、品番の列が空欄のままになる。
    #[test]
    fn wire_list_part_number_is_empty_when_the_table_has_no_match() {
        let mut project = one_wire_project(|w| {
            w.color = "橙".into();
            w.sq = 2.0;
        });
        project.wire_parts = vec![WirePart {
            part_no: "SAMPLE0001".into(),
            color: "水色".into(),
            sq: 0.3,
            note: String::new(),
        }];
        assert_eq!(data_rows(&wire_list_csv(&project))[0][9], "");
    }

    /// A part number written on the wire itself is kept in the list even when the table says otherwise.
    /// ワイヤ自身に書かれた品番は、表に別の品番があってもそのまま電線リストに載る。
    #[test]
    fn wire_list_keeps_a_part_number_written_on_the_wire() {
        let mut project = one_wire_project(|w| {
            w.color = "水色".into();
            w.sq = 0.3;
            w.part_no = Some("MY-WIRE-9".into());
        });
        project.wire_parts = vec![WirePart {
            part_no: "SAMPLE0001".into(),
            color: "水色".into(),
            sq: 0.3,
            note: String::new(),
        }];
        assert_eq!(data_rows(&wire_list_csv(&project))[0][9], "MY-WIRE-9");
    }

    /// A wire with no measured length leaves the length column empty (lengths come back from the 3D routing later).
    /// 長さが決まっていないワイヤの長さ列は空欄になる (長さは後で3D配線から書き戻される)。
    #[test]
    fn wire_list_length_is_empty_until_it_is_known() {
        let project = one_wire_project(|w| w.length_m = None);
        assert_eq!(data_rows(&wire_list_csv(&project))[0][8], "");
    }

    /// Rows are sorted by From then To within each sheet, so exporting the same drawing twice gives byte-identical files.
    /// 行はシートごとにFrom→Toの順に並ぶので、同じ図面を2回出力すると全く同じファイルになる。
    #[test]
    fn rows_are_sorted_by_from_then_to() {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        for e in [
            resistor("R1", 100.0, 50.0),
            resistor("R2", 150.0, 50.0),
            resistor("R3", 200.0, 50.0),
            wire_between((157.5, 50.0), (192.5, 50.0)),
            wire_between((107.5, 50.0), (142.5, 50.0)),
        ] {
            sheet.entities.insert(e.id(), e);
        }
        let froms: Vec<String> = data_rows(&wire_list_csv(&project))
            .iter()
            .map(|r| r[1].clone())
            .collect();
        assert_eq!(froms, vec!["R1:2", "R2:2"]);
    }
}
