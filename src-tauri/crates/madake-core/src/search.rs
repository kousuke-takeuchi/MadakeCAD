//! プロジェクト内検索とデバイスツリー (spec §10)。
//!
//! - **検索** ([`search_project`]): 図面の中から「参照記号・型番・ネット名・線番・テキスト」の
//!   5種を横断して探す。部分一致・大文字小文字の区別なし。結果は図面の読み順
//!   (シート番号→ゾーン→entity id) に並ぶので、同じ図面なら常に同じ順になる
//! - **デバイスツリー** ([`device_tree`]): 参照記号ごとに「その部品が図面のどこで何をしているか」
//!   (リレーならコイルと接点、端子台なら端子、それ以外は本体) を並べる。左パネルの
//!   デバイスナビゲータと参照サーフィン (Surfer) が引く
//!
//! 表示用の文言 (「リレー」「コイル」等) はここでは作らない。種別を表す列挙だけを返し、
//! 言語ごとの文言はUIが付ける (i18n)。どちらも読み取り専用の純関数で、モデルは変更しない。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::geometry::Point;
use crate::model::{Entity, EntityId, Project, SheetId, Wire};
use crate::relay_xref::{relay_devices, relay_role, terminal_pair, RelayDevice, RelayRole};
use crate::symbol::resolve_symbol;
use crate::terminal_chart::{is_terminal_block, terminal_count_of};
use crate::xref::zone_at;

/// 検索の対象種別。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SearchKind {
    /// 参照記号 (例 "K1")。
    Reference,
    /// 型番・値 (シンボルの`value`と`attrs`の値)。
    Value,
    /// ネット名 (ネットラベルの名前)。
    Net,
    /// 線番 (ワイヤに割り当てられた番号)。
    WireNo,
    /// 自由テキスト注記。
    Text,
}

impl SearchKind {
    /// 全種別 (検索バーの「すべて」)。
    pub const ALL: [SearchKind; 5] = [
        SearchKind::Reference,
        SearchKind::Value,
        SearchKind::Net,
        SearchKind::WireNo,
        SearchKind::Text,
    ];

    /// クエリ文字列の種別名 (`reference` / `value` / `net` / `wire_no` / `text`) を読む。
    /// 未知の名前は推測せずNoneを返す。
    pub fn parse(name: &str) -> Option<SearchKind> {
        match name.trim().to_ascii_lowercase().as_str() {
            "reference" => Some(SearchKind::Reference),
            "value" => Some(SearchKind::Value),
            "net" => Some(SearchKind::Net),
            "wire_no" | "wireno" => Some(SearchKind::WireNo),
            "text" => Some(SearchKind::Text),
            _ => None,
        }
    }
}

/// 検索で見つかった1件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchHit {
    pub kind: SearchKind,
    /// マッチした文字列そのもの (結果パネルの先頭列)。
    pub text: String,
    /// 補足 (型番のヒットなら持ち主の参照記号、参照記号のヒットならその型番)。無ければ空。
    pub detail: String,
    /// シンボルのヒットのとき、そのシンボルがデバイスとして果たしている機能。
    /// 結果パネルの「種別」列 (例「リレー 接点 13-14」) をUIがこれで組み立てる。
    #[serde(default)]
    pub function: Option<DeviceFunctionKind>,
    /// 機能の端子の呼び名 (例 "13-14")。機能が無ければ空。
    #[serde(default)]
    pub terminals: String,
    pub sheet_id: SheetId,
    /// 1始まりのシート表示順。
    pub sheet_no: usize,
    pub sheet_name: String,
    /// ゾーンアドレス (例 "B3")。
    pub zone: String,
    /// クリックしたときに選択・ズームするエンティティ。
    pub entity_id: EntityId,
}

impl SearchHit {
    /// 図面上の住所表記 (IEC 61082-1)。「/シート.ゾーン」= 例 `/2.B3`。
    pub fn address(&self) -> String {
        format!("/{}.{}", self.sheet_no, self.zone)
    }
}

/// `haystack`に`needle`が含まれるか (大文字小文字を区別しない)。空欄は決して当たらない。
fn matches(haystack: &str, needle_lower: &str) -> bool {
    !haystack.trim().is_empty() && haystack.to_lowercase().contains(needle_lower)
}

/// ワイヤの代表点 (最も上、同じ高さなら最も左)。線番の採番順と同じ基準。
fn wire_anchor(wire: &Wire) -> Point {
    wire.points
        .iter()
        .copied()
        .reduce(|best, p| if (p.y, p.x) < (best.y, best.x) { p } else { best })
        .unwrap_or(Point::new(0.0, 0.0))
}

/// プロジェクト全体を横断検索する。
///
/// `query`は部分一致で、大文字小文字を区別しない。前後の空白は無視し、空のクエリは
/// 何も返さない (全件を並べたりしない)。`kinds`が空なら全種別を対象にする。
///
/// 並びは図面の読み順 = シート番号→ゾーン→entity id→種別で、同じ図面なら常に同じ順になる。
pub fn search_project(project: &Project, query: &str, kinds: &[SearchKind]) -> Vec<SearchHit> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }
    let wanted: Vec<SearchKind> = if kinds.is_empty() {
        SearchKind::ALL.to_vec()
    } else {
        kinds.to_vec()
    };
    let want = |k: SearchKind| wanted.contains(&k);
    // シンボルのヒットに「何をしている部品か」を添えるための索引 (entity id → 機能)
    let functions = function_index(project);

    let mut hits: Vec<SearchHit> = Vec::new();
    for (i, sheet) in project.sheets.iter().enumerate() {
        for entity in sheet.entities.values() {
            let mut push =
                |kind: SearchKind, text: &str, detail: &str, entity_id: EntityId, at: Point| {
                    let function = functions.get(&entity_id);
                    hits.push(SearchHit {
                        kind,
                        text: text.trim().to_string(),
                        detail: detail.trim().to_string(),
                        function: function.map(|f| f.0),
                        terminals: function.map(|f| f.1.clone()).unwrap_or_default(),
                        sheet_id: sheet.id,
                        sheet_no: i + 1,
                        sheet_name: sheet.name.clone(),
                        zone: zone_at(sheet, at),
                        entity_id,
                    });
                };
            match entity {
                Entity::Symbol(s) => {
                    if want(SearchKind::Reference) && matches(&s.reference, &needle) {
                        push(SearchKind::Reference, &s.reference, &s.value, s.id, s.at);
                    }
                    if want(SearchKind::Value) {
                        if matches(&s.value, &needle) {
                            push(SearchKind::Value, &s.value, &s.reference, s.id, s.at);
                        }
                        for value in s.attrs.values() {
                            if matches(value, &needle) {
                                push(SearchKind::Value, value, &s.reference, s.id, s.at);
                            }
                        }
                    }
                }
                Entity::NetLabel(l) => {
                    if want(SearchKind::Net) && matches(&l.name, &needle) {
                        push(SearchKind::Net, &l.name, "", l.id, l.at);
                    }
                }
                Entity::Wire(w) => {
                    if let Some(no) = w.net.as_deref() {
                        if want(SearchKind::WireNo) && matches(no, &needle) {
                            push(SearchKind::WireNo, no, "", w.id, wire_anchor(w));
                        }
                    }
                }
                Entity::Text(t) => {
                    if want(SearchKind::Text) && matches(&t.text, &needle) {
                        push(SearchKind::Text, &t.text, "", t.id, t.at);
                    }
                }
                Entity::Junction(_) | Entity::Harness(_) => {}
            }
        }
    }
    hits.sort_by(|a, b| {
        (a.sheet_no, &a.zone, a.entity_id, a.kind, &a.text).cmp(&(
            b.sheet_no,
            &b.zone,
            b.entity_id,
            b.kind,
            &b.text,
        ))
    });
    hits
}

/// entity id → そのシンボルが果たす機能 (種別と端子の呼び名)。
/// 1つのシンボルが複数の機能を持つ端子台は、まとめた1つの機能 (例 "1-8") になる。
fn function_index(project: &Project) -> BTreeMap<EntityId, (DeviceFunctionKind, String)> {
    device_tree(project)
        .into_iter()
        .flat_map(|d| d.functions)
        .map(|f| (f.entity_id, (f.kind, f.terminals)))
        .collect()
}

/// デバイス (参照記号1つ) の種別。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    /// コイル・接点を持つリレー。
    Relay,
    /// 端子台。
    TerminalBlock,
    /// それ以外の部品 (ランプ・ヒューズ等)。
    Other,
}

/// デバイスを構成する機能1つの種別。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum DeviceFunctionKind {
    /// リレーのコイル。
    Coil,
    /// a接点 (メーク接点)。
    ContactNo,
    /// b接点 (ブレーク接点)。
    ContactNc,
    /// 端子台の端子群。
    Terminal,
    /// 機能に分かれない部品の本体。
    Body,
}

/// デバイスの機能1つ (コイル・接点・端子群・本体) と、その図面上の所在。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeviceFunction {
    pub kind: DeviceFunctionKind,
    /// 端子の呼び名 (コイル "A1-A2" / 接点 "13-14" / 端子台 "1-8" / 本体は空)。
    pub terminals: String,
    pub entity_id: EntityId,
    pub sheet_id: SheetId,
    /// 1始まりのシート表示順。
    pub sheet_no: usize,
    pub sheet_name: String,
    /// ゾーンアドレス (例 "C2")。
    pub zone: String,
}

impl DeviceFunction {
    /// 図面上の住所表記「/シート.ゾーン」= 例 `/1.C2`。
    pub fn address(&self) -> String {
        format!("/{}.{}", self.sheet_no, self.zone)
    }
}

/// 参照記号1つ分のデバイス (デバイスナビゲータのツリーの1ノード)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeviceNode {
    pub reference: String,
    pub kind: DeviceKind,
    /// 型番・値 (最初に見つかった空でないもの)。
    pub value: String,
    /// 代表シンボルのライブラリキー (読み順で最初のもの)。
    pub symbol_id: String,
    /// 代表シンボルの名前 (英語)。ツリーの見出しに使う。
    pub symbol_name: String,
    /// 代表シンボルの名前 (日本語)。
    pub symbol_name_ja: String,
    /// 端子台の極数 (端子台以外は0)。
    pub poles: usize,
    /// この参照記号が図面で果たしている機能の一覧。
    pub functions: Vec<DeviceFunction>,
}

/// シンボル1個分の所在 (デバイスツリーの組み立て用)。
struct Placement {
    entity_id: EntityId,
    sheet_id: SheetId,
    sheet_no: usize,
    sheet_name: String,
    zone: String,
    symbol_id: String,
    value: String,
}

/// プロジェクト内の全デバイスを参照記号ごとにまとめたツリー (参照記号の昇順)。
///
/// - **リレー** (コイル・接点シンボル): 機能=コイル+接点。端子の呼び名は接点マップと同じ
///   IEC 60947-1の連番 (コイル `A1-A2`、1個目のa接点 `13-14` …)
/// - **端子台**: 機能=端子群1つ (`"1-8"`)。極数はシンボル定義のピン数から
/// - **その他の部品**: 機能=本体1つ
///
/// 参照記号の付いていないシンボルはどのデバイスにも属さない (ツリーに出ない)。
pub fn device_tree(project: &Project) -> Vec<DeviceNode> {
    let relays: BTreeMap<String, RelayDevice> = relay_devices(project)
        .into_iter()
        .map(|d| (d.reference.clone(), d))
        .collect();

    let mut by_ref: BTreeMap<String, Vec<Placement>> = BTreeMap::new();
    for (i, sheet) in project.sheets.iter().enumerate() {
        for entity in sheet.entities.values() {
            let Entity::Symbol(s) = entity else { continue };
            let reference = s.reference.trim();
            if reference.is_empty() {
                continue;
            }
            by_ref
                .entry(reference.to_string())
                .or_default()
                .push(Placement {
                    entity_id: s.id,
                    sheet_id: sheet.id,
                    sheet_no: i + 1,
                    sheet_name: sheet.name.clone(),
                    zone: zone_at(sheet, s.at),
                    symbol_id: s.symbol_id.clone(),
                    value: s.value.trim().to_string(),
                });
        }
    }

    by_ref
        .into_iter()
        .map(|(reference, mut places)| {
            places.sort_by(|a, b| {
                (a.sheet_no, &a.zone, a.entity_id).cmp(&(b.sheet_no, &b.zone, b.entity_id))
            });
            let value = places
                .iter()
                .find(|p| !p.value.is_empty())
                .map(|p| p.value.clone())
                .unwrap_or_default();
            let kind = if places.iter().any(|p| relay_role(&p.symbol_id).is_some()) {
                DeviceKind::Relay
            } else if places.iter().any(|p| is_terminal_block(&p.symbol_id)) {
                DeviceKind::TerminalBlock
            } else {
                DeviceKind::Other
            };
            let functions = match kind {
                DeviceKind::Relay => relay_functions(&reference, &relays, &places),
                DeviceKind::TerminalBlock => terminal_functions(&places),
                DeviceKind::Other => places
                    .iter()
                    .map(|p| function(p, DeviceFunctionKind::Body, String::new()))
                    .collect(),
            };
            let head = places
                .iter()
                .find(|p| match kind {
                    DeviceKind::Relay => relay_role(&p.symbol_id) == Some(RelayRole::Coil),
                    DeviceKind::TerminalBlock => is_terminal_block(&p.symbol_id),
                    DeviceKind::Other => true,
                })
                .or_else(|| places.first());
            let symbol_id = head.map(|p| p.symbol_id.clone()).unwrap_or_default();
            let def = resolve_symbol(&symbol_id);
            DeviceNode {
                reference,
                kind,
                value,
                symbol_name: def.as_ref().map(|d| d.name.clone()).unwrap_or_default(),
                symbol_name_ja: def.as_ref().map(|d| d.name_ja.clone()).unwrap_or_default(),
                poles: if kind == DeviceKind::TerminalBlock {
                    terminal_count_of(&symbol_id)
                } else {
                    0
                },
                symbol_id,
                functions,
            }
        })
        .collect()
}

fn function(p: &Placement, kind: DeviceFunctionKind, terminals: String) -> DeviceFunction {
    DeviceFunction {
        kind,
        terminals,
        entity_id: p.entity_id,
        sheet_id: p.sheet_id,
        sheet_no: p.sheet_no,
        sheet_name: p.sheet_name.clone(),
        zone: p.zone.clone(),
    }
}

/// リレーの機能一覧。コイル→接点の順で、端子の呼び名は接点マップ ([`crate::relay_xref`]) と揃える。
/// 同じ参照記号のリレー以外のシンボルは本体として末尾に残す。
fn relay_functions(
    reference: &str,
    relays: &BTreeMap<String, RelayDevice>,
    places: &[Placement],
) -> Vec<DeviceFunction> {
    let find = |id: EntityId| places.iter().find(|p| p.entity_id == id);
    let mut out = Vec::new();
    if let Some(device) = relays.get(reference) {
        for coil in &device.coils {
            if let Some(p) = find(coil.entity_id) {
                out.push(function(
                    p,
                    DeviceFunctionKind::Coil,
                    terminal_pair(1, RelayRole::Coil),
                ));
            }
        }
        for (i, contact) in device.contacts.iter().enumerate() {
            let Some(p) = find(contact.entity_id) else { continue };
            let kind = match contact.role {
                RelayRole::ContactNc => DeviceFunctionKind::ContactNc,
                _ => DeviceFunctionKind::ContactNo,
            };
            out.push(function(p, kind, terminal_pair(i + 1, contact.role)));
        }
    }
    for p in places.iter().filter(|p| relay_role(&p.symbol_id).is_none()) {
        out.push(function(p, DeviceFunctionKind::Body, String::new()));
    }
    out
}

/// 端子台の機能 = 端子群1つ (例 8極なら "1-8")。極数が1なら "1"、読めなければ空。
fn terminal_functions(places: &[Placement]) -> Vec<DeviceFunction> {
    places
        .iter()
        .map(|p| {
            if !is_terminal_block(&p.symbol_id) {
                return function(p, DeviceFunctionKind::Body, String::new());
            }
            let terminals = match terminal_count_of(&p.symbol_id) {
                0 => String::new(),
                1 => "1".to_string(),
                n => format!("1-{n}"),
            };
            function(p, DeviceFunctionKind::Terminal, terminals)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use crate::relay_xref::{COIL_SYMBOL_ID, CONTACT_NC_SYMBOL_ID, CONTACT_NO_SYMBOL_ID};
    use uuid::Uuid;

    fn symbol(symbol_id: &str, reference: &str, value: &str, x: f64, y: f64) -> Entity {
        Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: symbol_id.into(),
            at: Point::new(x, y),
            rotation: 0,
            mirror: false,
            reference: reference.into(),
            value: value.into(),
            attrs: Default::default(),
        })
    }

    fn net_label(name: &str, x: f64, y: f64) -> Entity {
        Entity::NetLabel(NetLabel {
            id: Uuid::new_v4(),
            at: Point::new(x, y),
            name: name.into(),
            rotation: 0,
        })
    }

    fn wire(no: Option<&str>, x: f64, y: f64) -> Entity {
        Entity::Wire(Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(x, y), Point::new(x + 20.0, y)],
            color: "red".into(),
            sq: 0.75,
            length_m: None,
            length_source: Default::default(),
            part_no: None,
            net: no.map(str::to_string),
        })
    }

    fn text(body: &str, x: f64, y: f64) -> Entity {
        Entity::Text(TextEntity {
            id: Uuid::new_v4(),
            at: Point::new(x, y),
            text: body.into(),
            height: 2.5,
            rotation: 0,
        })
    }

    /// エンティティを並べたプロジェクトを組み立てる (シート添字, エンティティ)。
    fn project_with(sheets: usize, items: Vec<(usize, Entity)>) -> Project {
        let mut project = Project::new("t");
        for i in 1..sheets {
            project.sheets.push(Sheet::new(
                &format!("Sheet{}", i + 1),
                PaperSize::A3,
                Orientation::Landscape,
            ));
        }
        for (index, e) in items {
            let id = project.sheets[index].id;
            project.sheet_mut(id).unwrap().entities.insert(e.id(), e);
        }
        project
    }

    fn demo() -> Project {
        project_with(
            1,
            vec![
                (0, symbol(COIL_SYMBOL_ID, "K1", "MY2N", 30.0, 30.0)),
                (0, net_label("24V", 60.0, 30.0)),
                (0, wire(Some("101"), 90.0, 30.0)),
                (0, text("K1動作時に点灯", 120.0, 30.0)),
            ],
        )
    }

    /// One search covers all five targets at once: reference designators, part numbers, net names, wire numbers and free text.
    /// 1回の検索で5種すべて(参照記号・型番・ネット名・線番・テキスト)を対象にする。
    #[test]
    fn search_covers_all_five_targets() {
        let project = demo();
        let kinds = |q: &str| {
            search_project(&project, q, &[])
                .into_iter()
                .map(|h| h.kind)
                .collect::<Vec<_>>()
        };
        assert_eq!(kinds("MY2N"), vec![SearchKind::Value], "型番");
        assert_eq!(kinds("24V"), vec![SearchKind::Net], "ネット名");
        assert_eq!(kinds("101"), vec![SearchKind::WireNo], "線番");
        assert_eq!(kinds("点灯"), vec![SearchKind::Text], "テキスト");
        // "K1" は参照記号にも注記テキストにも含まれるので2件出る
        assert_eq!(
            kinds("K1"),
            vec![SearchKind::Reference, SearchKind::Text],
            "参照記号"
        );
    }

    /// The query matches any part of a word and ignores upper/lower case, so "my2" finds the part number "MY2N".
    /// クエリは語の一部にも当たり、大文字小文字も区別しない。「my2」で型番「MY2N」が見つかる。
    #[test]
    fn search_is_case_insensitive_and_partial() {
        let project = demo();
        let hit = search_project(&project, "my2", &[]);
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].text, "MY2N");
        assert_eq!(search_project(&project, "k1", &[]).len(), 2, "小文字でも当たる");
        assert_eq!(search_project(&project, "24v", &[]).len(), 1);
    }

    /// Ticking only some filter chips narrows the results to those targets, so searching "K1" with the reference filter drops the text note.
    /// フィルタチップで対象を絞ると、その種別だけが残る。「K1」を参照記号だけで探すと注記のヒットは消える。
    #[test]
    fn kinds_filter_narrows_the_targets() {
        let project = demo();
        let refs = search_project(&project, "K1", &[SearchKind::Reference]);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].kind, SearchKind::Reference);
        assert_eq!(refs[0].text, "K1");
        let texts = search_project(&project, "K1", &[SearchKind::Text]);
        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0].kind, SearchKind::Text);
        // 複数チップの併用は和集合 (検索バーの「ネット」= ネット名+線番もこの形)
        assert_eq!(
            search_project(&project, "K1", &[SearchKind::Reference, SearchKind::Text]).len(),
            2
        );
    }

    /// An empty query finds nothing at all (it never dumps the whole drawing), and blank fields are never matched.
    /// 空のクエリは何も返さない(図面全体を並べたりしない)。空欄のフィールドもヒットしない。
    #[test]
    fn empty_query_finds_nothing() {
        let project = project_with(
            1,
            vec![
                (0, symbol("lamp", "", "", 30.0, 30.0)),
                (0, wire(None, 60.0, 30.0)),
            ],
        );
        assert!(search_project(&project, "", &[]).is_empty());
        assert!(search_project(&project, "   ", &[]).is_empty());
        // 参照記号も型番も空のシンボルは、どんな1文字クエリにも当たらない
        assert!(search_project(&project, "a", &[]).is_empty());
    }

    /// Results come back in reading order — sheet number, then zone, then entity — so the same drawing always lists them the same way.
    /// 結果は図面の読み順(シート番号→ゾーン→エンティティ)で返るので、同じ図面なら並びは常に同じになる。
    #[test]
    fn results_are_in_reading_order() {
        let project = project_with(
            2,
            vec![
                (1, symbol("lamp", "L1", "", 30.0, 30.0)),
                (0, symbol("lamp", "L1", "", 300.0, 200.0)),
                (0, symbol("lamp", "L1", "", 30.0, 30.0)),
            ],
        );
        let order: Vec<String> = search_project(&project, "L1", &[])
            .iter()
            .map(|h| h.address())
            .collect();
        assert_eq!(order, vec!["/1.A1", "/1.E3", "/2.A1"]);
        // 2回呼んでも同じ並び
        let again: Vec<String> = search_project(&project, "L1", &[])
            .iter()
            .map(|h| h.address())
            .collect();
        assert_eq!(order, again);
    }

    /// Each hit knows where it lives — sheet, zone and the entity to select — and prints its address as "/sheet.zone".
    /// ヒットは所在(シート・ゾーン・選択するエンティティ)を持ち、住所を「/シート.ゾーン」の形で表す。
    #[test]
    fn hit_carries_its_location() {
        let project = project_with(2, vec![(1, symbol("lamp", "L9", "", 30.0, 30.0))]);
        let hits = search_project(&project, "L9", &[]);
        assert_eq!(hits.len(), 1);
        let hit = &hits[0];
        assert_eq!(hit.sheet_no, 2);
        assert_eq!(hit.sheet_name, "Sheet2");
        assert_eq!(hit.sheet_id, project.sheets[1].id);
        assert_eq!(hit.zone, "A1");
        assert_eq!(hit.address(), "/2.A1");
        assert!(project.sheets[1].entities.contains_key(&hit.entity_id));
    }

    /// A part-number hit shows which device it belongs to, and a reference hit shows that device's part number.
    /// 型番のヒットはどの部品のものかを、参照記号のヒットはその部品の型番を、それぞれ添えて返す。
    #[test]
    fn hits_carry_the_partner_field_as_detail() {
        let project = project_with(1, vec![(0, symbol(COIL_SYMBOL_ID, "K7", "MY4N", 30.0, 30.0))]);
        let value = &search_project(&project, "MY4N", &[])[0];
        assert_eq!(value.text, "MY4N");
        assert_eq!(value.detail, "K7");
        let reference = &search_project(&project, "K7", &[])[0];
        assert_eq!(reference.text, "K7");
        assert_eq!(reference.detail, "MY4N");
    }

    /// A hit on a symbol also says what that symbol does in its device, so the result list can read "relay contact 13-14".
    /// シンボルのヒットは、そのシンボルがデバイスの中で果たす機能も返すので、結果一覧に「リレー 接点 13-14」と出せる。
    #[test]
    fn symbol_hits_say_what_the_symbol_does() {
        let project = project_with(
            1,
            vec![
                (0, symbol(COIL_SYMBOL_ID, "K1", "", 30.0, 30.0)),
                (0, symbol(CONTACT_NO_SYMBOL_ID, "K1", "", 200.0, 30.0)),
            ],
        );
        let hits = search_project(&project, "K1", &[SearchKind::Reference]);
        assert_eq!(
            hits.iter()
                .map(|h| (h.function, h.terminals.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (Some(DeviceFunctionKind::Coil), "A1-A2"),
                (Some(DeviceFunctionKind::ContactNo), "13-14"),
            ]
        );
        // ネットラベル・線番・注記には機能が無い
        let project = project_with(1, vec![(0, net_label("24V", 30.0, 30.0))]);
        assert_eq!(search_project(&project, "24V", &[])[0].function, None);
    }

    /// Attribute values such as the contact configuration are searched together with the part number.
    /// 接点構成のような属性の値も、型番と一緒に検索できる。
    #[test]
    fn attribute_values_are_searched_as_part_numbers() {
        let mut project = project_with(1, vec![(0, symbol(COIL_SYMBOL_ID, "K1", "", 30.0, 30.0))]);
        let sheet_id = project.sheets[0].id;
        let id = *project.sheets[0].entities.keys().next().unwrap();
        if let Some(Entity::Symbol(s)) = project.sheet_mut(sheet_id).unwrap().entities.get_mut(&id) {
            s.attrs.insert("contact_config".into(), "2NO+2NC".into());
        }
        let hits = search_project(&project, "2no", &[SearchKind::Value]);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].text, "2NO+2NC");
        assert_eq!(hits[0].detail, "K1");
    }

    /// A relay device lists its coil first and then its contacts, using the same terminal numbers as the contact map.
    /// リレーのデバイスはコイルを先に、続けて接点を並べ、端子の呼び名は接点マップと同じになる。
    #[test]
    fn relay_device_lists_coil_then_contacts() {
        let project = project_with(
            2,
            vec![
                (0, symbol(COIL_SYMBOL_ID, "K1", "MY2N", 30.0, 100.0)),
                (0, symbol(CONTACT_NO_SYMBOL_ID, "K1", "", 200.0, 40.0)),
                (1, symbol(CONTACT_NC_SYMBOL_ID, "K1", "", 30.0, 40.0)),
            ],
        );
        let tree = device_tree(&project);
        assert_eq!(tree.len(), 1);
        let device = &tree[0];
        assert_eq!(device.reference, "K1");
        assert_eq!(device.kind, DeviceKind::Relay);
        assert_eq!(device.value, "MY2N", "型番はデバイス見出しに出す");
        let rows: Vec<(DeviceFunctionKind, String, String)> = device
            .functions
            .iter()
            .map(|f| (f.kind, f.terminals.clone(), f.address()))
            .collect();
        assert_eq!(
            rows,
            vec![
                (DeviceFunctionKind::Coil, "A1-A2".to_string(), "/1.B1".to_string()),
                (DeviceFunctionKind::ContactNo, "13-14".to_string(), "/1.A2".to_string()),
                (DeviceFunctionKind::ContactNc, "21-22".to_string(), "/2.A1".to_string()),
            ]
        );
    }

    /// A terminal block appears as one "terminals" row covering every pole, with the pole count on the device.
    /// 端子台は全極をまとめた「端子」1行として並び、極数はデバイス側に持つ。
    #[test]
    fn terminal_block_device_shows_its_terminal_range() {
        let project = project_with(
            1,
            vec![(0, symbol("terminal_block_8p", "TB1", "", 30.0, 30.0))],
        );
        let tree = device_tree(&project);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].kind, DeviceKind::TerminalBlock);
        assert_eq!(tree[0].poles, 8);
        assert_eq!(tree[0].functions.len(), 1);
        assert_eq!(tree[0].functions[0].kind, DeviceFunctionKind::Terminal);
        assert_eq!(tree[0].functions[0].terminals, "1-8");
    }

    /// An ordinary part that has no separable functions shows up as a single "body" row, named after its symbol.
    /// 機能に分かれない普通の部品は、「本体」1行だけのデバイスとして、シンボルの名前つきで出る。
    #[test]
    fn plain_part_is_a_single_body_row() {
        let project = project_with(1, vec![(0, symbol("fuse", "F1", "GF-8", 30.0, 30.0))]);
        let tree = device_tree(&project);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].kind, DeviceKind::Other);
        assert_eq!(tree[0].value, "GF-8");
        assert_eq!(tree[0].symbol_id, "fuse");
        assert!(!tree[0].symbol_name.is_empty(), "英語名でツリーの見出しを作る");
        assert!(!tree[0].symbol_name_ja.is_empty(), "日本語名も返す");
        assert_eq!(tree[0].poles, 0);
        assert_eq!(tree[0].functions.len(), 1);
        assert_eq!(tree[0].functions[0].kind, DeviceFunctionKind::Body);
        assert_eq!(tree[0].functions[0].terminals, "");
    }

    /// Devices are keyed by reference designator, so the same designator on several sheets is one device, and symbols without a designator are not devices at all.
    /// デバイスは参照記号でまとまるので、複数シートに散っていても1デバイスになる。参照記号の無いシンボルはデバイスにならない。
    #[test]
    fn devices_group_by_reference_across_sheets() {
        let project = project_with(
            2,
            vec![
                (0, symbol("lamp", "PL1", "", 30.0, 30.0)),
                (1, symbol("lamp", "PL1", "", 30.0, 30.0)),
                (0, symbol("lamp", "", "", 60.0, 30.0)),
                (0, symbol("lamp", "PL2", "", 90.0, 30.0)),
            ],
        );
        let tree = device_tree(&project);
        assert_eq!(
            tree.iter().map(|d| d.reference.as_str()).collect::<Vec<_>>(),
            vec!["PL1", "PL2"]
        );
        assert_eq!(tree[0].functions.len(), 2, "2シートに散った本体が両方並ぶ");
        assert_eq!(tree[0].functions[0].sheet_no, 1);
        assert_eq!(tree[0].functions[1].sheet_no, 2);
    }

    /// An empty project has no devices, and the tree is ordered by reference designator.
    /// 空のプロジェクトにデバイスは無く、ツリーは参照記号の順に並ぶ。
    #[test]
    fn device_tree_is_ordered_and_can_be_empty() {
        assert!(device_tree(&Project::new("empty")).is_empty());
        let project = project_with(
            1,
            vec![
                (0, symbol("lamp", "PL2", "", 30.0, 30.0)),
                (0, symbol("lamp", "PL1", "", 60.0, 30.0)),
                (0, symbol("fuse", "F1", "", 90.0, 30.0)),
            ],
        );
        assert_eq!(
            device_tree(&project)
                .iter()
                .map(|d| d.reference.as_str())
                .collect::<Vec<_>>(),
            vec!["F1", "PL1", "PL2"]
        );
    }

    /// Every function row knows the sheet and zone to jump to, which is what the navigator and the reference surfer use to reveal it.
    /// 機能の行はジャンプ先のシートとゾーンを持つ。ナビゲータと参照サーフィンはこれを使って図面を表示する。
    #[test]
    fn every_function_knows_where_to_jump() {
        let project = project_with(2, vec![(1, symbol(COIL_SYMBOL_ID, "K3", "", 200.0, 150.0))]);
        let device = &device_tree(&project)[0];
        let coil = &device.functions[0];
        assert_eq!(coil.sheet_id, project.sheets[1].id);
        assert_eq!(coil.sheet_no, 2);
        assert_eq!(coil.sheet_name, "Sheet2");
        assert_eq!(coil.address(), "/2.D2");
        assert!(project.sheets[1].entities.contains_key(&coil.entity_id));
    }

    /// Filter names travel over the API as snake_case strings, and unknown names are rejected rather than guessed.
    /// フィルタ名はAPI上ではsnake_caseの文字列で、未知の名前は推測せずに拒否する。
    #[test]
    fn filter_names_parse_from_the_api() {
        assert_eq!(SearchKind::parse("reference"), Some(SearchKind::Reference));
        assert_eq!(SearchKind::parse(" Wire_No "), Some(SearchKind::WireNo));
        assert_eq!(SearchKind::parse("text"), Some(SearchKind::Text));
        assert_eq!(SearchKind::parse("colour"), None);
        assert_eq!(SearchKind::ALL.len(), 5);
    }
}
