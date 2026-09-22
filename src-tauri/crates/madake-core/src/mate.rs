//! 嵌合コネクタ対 (G14)。
//!
//! フィールド機器のコネクタは雄雌一対で使う (プラグ⇔ソケット、`connector_{n}p`同士)。
//! `attrs["mates_with"]`にもう一方の参照記号を書けば、その対を宣言できる
//! (双方に書いて初めて「両側から宣言された対」になる)。
//!
//! - ERC: 極数が食い違えば警告 (`erc.mate_pole_mismatch`)、片側しか宣言していなければ
//!   参考情報 (`erc.mate_one_sided`)、自己参照・不明な参照記号は警告 (`erc.mate_unknown`)
//! - 帳票 (電線リスト・From-To): 宣言した相手の参照記号を「嵌合相手」列に載せる
//!   ([`crate::reports`])
//! - 図面: 両方が同じシート上にあり50mm以内に収まっているときだけ、外形どうしの
//!   最短点を結ぶ短い破線を自動で描く (それ以外は何も描かず、帳票が対を伝える)
//!   ([`crate::svg`]・`src/canvas/mate.ts`)
//!
//! 対象は**コネクタ一族だけ** (`symbol_id`が`connector_`で始まるシンボル。動的な
//! `connector_{n}p`と静的な`connector_plug`/`connector_socket`の両方がこれに当たる)。
//! 端子台 (`terminal_block_*`) はカテゴリこそ同じ「connector」だが対象にしない
//! (端子台は対を組む部品ではない)。

use std::collections::BTreeMap;

use crate::model::{Entity, EntityId, Project, Sheet, SheetId};
use crate::verify::{Diagnostic, Severity};

/// `mates_with`宣言を書き込む属性キー。
pub const MATES_WITH_ATTR: &str = "mates_with";

/// 極数不一致の診断コード (G14)。
pub const MATE_POLE_MISMATCH: &str = "erc.mate_pole_mismatch";
/// 片側宣言の診断コード (G14)。
pub const MATE_ONE_SIDED: &str = "erc.mate_one_sided";
/// 自己参照・不明な参照記号の診断コード (G14)。
pub const MATE_UNKNOWN: &str = "erc.mate_unknown";

/// このシンボルが嵌合対の対象になるコネクタ一族か (静的プラグ/ソケット含む)。
pub fn is_connector_instance(symbol_id: &str) -> bool {
    symbol_id.starts_with("connector_")
}

/// プロジェクト内のコネクタインスタンス1つぶんの索引情報。
struct MateEntry {
    sheet_id: SheetId,
    entity_id: EntityId,
    pole_count: usize,
    /// `mates_with`属性の値 (前後の空白を落とし、空文字はNone扱い)。
    declared: Option<String>,
}

/// 参照記号→索引情報。参照記号が重複しているときは最初に見つかった1件だけを使う
/// (重複自体は`erc.duplicate_reference`が別に報告する)。
fn collect(project: &Project) -> BTreeMap<String, MateEntry> {
    let mut out: BTreeMap<String, MateEntry> = BTreeMap::new();
    for sheet in &project.sheets {
        for entity in sheet.entities.values() {
            let Entity::Symbol(s) = entity else { continue };
            if !is_connector_instance(&s.symbol_id) {
                continue;
            }
            let reference = s.reference.trim();
            if reference.is_empty() || out.contains_key(reference) {
                continue;
            }
            let Some(def) = crate::symbol::resolve_symbol(&s.symbol_id) else {
                continue;
            };
            let declared = s
                .attrs
                .get(MATES_WITH_ATTR)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty());
            out.insert(
                reference.to_string(),
                MateEntry {
                    sheet_id: sheet.id,
                    entity_id: s.id,
                    pole_count: def.pins.len(),
                    declared,
                },
            );
        }
    }
    out
}

/// プロジェクト全体の嵌合対ERC (G14)。`mates_with`は他シートの参照記号も指せるので
/// (プロジェクト全体で見る)、コイル⇔接点クロスリファレンスと同じくプロジェクト単位で評価する
/// ([`crate::verify::verify_project`])。
pub fn mate_diagnostics(project: &Project) -> Vec<Diagnostic> {
    let entries = collect(project);
    let mut diags = Vec::new();
    // 極数不一致は双方が宣言し合っていると同じ組が2回検出されるので、1組1件にまとめる
    let mut mismatch_seen = std::collections::BTreeSet::new();
    for (reference, entry) in &entries {
        let Some(target_ref) = &entry.declared else {
            continue;
        };
        let diag = |severity, code: &str, message: String, ids: Vec<EntityId>| Diagnostic {
            severity,
            code: code.into(),
            message,
            sheet_id: entry.sheet_id,
            entity_ids: ids,
        };
        if target_ref == reference {
            diags.push(diag(
                Severity::Warning,
                MATE_UNKNOWN,
                format!("{reference}: 嵌合相手に自分自身を指定しています"),
                vec![entry.entity_id],
            ));
            continue;
        }
        let Some(target) = entries.get(target_ref) else {
            diags.push(diag(
                Severity::Warning,
                MATE_UNKNOWN,
                format!("{reference}: 嵌合相手の参照記号 {target_ref} が見つかりません"),
                vec![entry.entity_id],
            ));
            continue;
        };
        if target.pole_count != entry.pole_count {
            let mut key = [entry.entity_id, target.entity_id];
            key.sort();
            if mismatch_seen.insert(key) {
                diags.push(diag(
                    Severity::Warning,
                    MATE_POLE_MISMATCH,
                    format!(
                        "{reference} ({}極) と {target_ref} ({}極) で嵌合相手の極数が一致しません",
                        entry.pole_count, target.pole_count
                    ),
                    vec![entry.entity_id, target.entity_id],
                ));
            }
        }
        let reciprocated = target.declared.as_deref() == Some(reference.as_str());
        if !reciprocated {
            diags.push(diag(
                Severity::Info,
                MATE_ONE_SIDED,
                format!(
                    "{reference} は {target_ref} を嵌合相手に指定していますが、{target_ref} 側には対応する指定がありません"
                ),
                vec![entry.entity_id],
            ));
        }
    }
    diags
}

/// あるシート上の参照記号が持つ`mates_with`宣言の値 (前後の空白を落とし、空なら空文字)。
/// 帳票 (電線リスト・From-To、`crate::reports`) の「嵌合相手」列が使う。宣言が無い・
/// コネクタ一族でない参照記号は空文字。
pub fn declared_mate(sheet: &Sheet, reference: &str) -> String {
    let reference = reference.trim();
    if reference.is_empty() {
        return String::new();
    }
    sheet
        .entities
        .values()
        .find_map(|e| {
            let Entity::Symbol(s) = e else { return None };
            if !is_connector_instance(&s.symbol_id) || s.reference.trim() != reference {
                return None;
            }
            s.attrs.get(MATES_WITH_ATTR).map(|v| v.trim().to_string())
        })
        .unwrap_or_default()
}

/// 図面に破線を描く相手先ペア (G14)。同一シート上・50mm以内の対だけを対象にする。
#[derive(Debug, Clone, PartialEq)]
pub struct MateLink {
    pub sheet_id: SheetId,
    pub a: EntityId,
    pub b: EntityId,
}

/// 図面に破線を描くべき嵌合対を列挙する。両方が**同一シート**にあり、配置基準点どうしの
/// 距離が[`MATE_LINE_MAX_DIST_MM`]以内のときだけ対象になる (それ以外は帳票だけが伝える)。
/// 片側宣言・極数不一致でも対そのものは描く (ERCとは独立)。1組を重複して2回返さない。
pub fn mate_links(project: &Project) -> Vec<MateLink> {
    let entries = collect(project);
    let mut out = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for (reference, entry) in &entries {
        let Some(target_ref) = &entry.declared else {
            continue;
        };
        if target_ref == reference {
            continue;
        }
        let Some(target) = entries.get(target_ref) else {
            continue;
        };
        if entry.sheet_id != target.sheet_id {
            continue;
        }
        let mut key = [entry.entity_id, target.entity_id];
        key.sort();
        if !seen.insert(key) {
            continue;
        }
        let Some(sheet) = project.sheet(entry.sheet_id) else {
            continue;
        };
        let (Some(Entity::Symbol(sa)), Some(Entity::Symbol(sb))) = (
            sheet.entities.get(&entry.entity_id),
            sheet.entities.get(&target.entity_id),
        ) else {
            continue;
        };
        if sa.at.distance_to(&sb.at) > MATE_LINE_MAX_DIST_MM {
            continue;
        }
        out.push(MateLink {
            sheet_id: entry.sheet_id,
            a: key[0],
            b: key[1],
        });
    }
    out
}

/// 嵌合対の破線を描く距離の上限 (mm、plan text = 50mm)。
pub const MATE_LINE_MAX_DIST_MM: f64 = 50.0;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::*;
    use uuid::Uuid;

    fn connector(reference: &str, symbol_id: &str, x: f64, mate: Option<&str>) -> Entity {
        let mut attrs = std::collections::BTreeMap::new();
        if let Some(m) = mate {
            attrs.insert(MATES_WITH_ATTR.to_string(), m.to_string());
        }
        Entity::Symbol(SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: symbol_id.into(),
            at: Point::new(x, 0.0),
            rotation: 0,
            mirror: false,
            reference: reference.into(),
            value: String::new(),
            attrs,
        })
    }

    fn project_with(entities: Vec<Entity>) -> Project {
        let mut project = Project::new("t");
        let sid = project.sheets[0].id;
        let sheet = project.sheet_mut(sid).unwrap();
        for e in entities {
            sheet.entities.insert(e.id(), e);
        }
        project
    }

    /// Two connectors that declare each other as their mate, with matching pole counts, produce no ERC finding at all.
    /// 互いを嵌合相手に指定し、極数も一致している2つのコネクタは、ERCの指摘を1つも出さない。
    #[test]
    fn a_reciprocated_matching_pair_has_no_erc_findings() {
        let project = project_with(vec![
            connector("J1", "connector_3p", 0.0, Some("J12")),
            connector("J12", "connector_3p", 10.0, Some("J1")),
        ]);
        let diags = mate_diagnostics(&project);
        assert!(diags.is_empty(), "{diags:?}");
    }

    /// A connector that declares a mate with a different pole count gets a pole-mismatch warning.
    /// 極数の違う相手を嵌合相手に指定したコネクタには、極数不一致の警告が出る。
    #[test]
    fn a_pole_count_mismatch_is_a_warning() {
        let project = project_with(vec![
            connector("J1", "connector_3p", 0.0, Some("J12")),
            connector("J12", "connector_4p", 10.0, Some("J1")),
        ]);
        let diags = mate_diagnostics(&project);
        let hit: Vec<_> = diags.iter().filter(|d| d.code == MATE_POLE_MISMATCH).collect();
        assert_eq!(hit.len(), 1, "{diags:?}");
        assert_eq!(hit[0].severity, Severity::Warning);
    }

    /// A connector that declares a mate the other side does not declare back gets an informational one-sided-declaration finding.
    /// 相手側からは指定されていない嵌合相手を宣言したコネクタには、片側宣言の参考情報が出る。
    #[test]
    fn a_one_sided_declaration_is_info() {
        let project = project_with(vec![
            connector("J1", "connector_3p", 0.0, Some("J12")),
            connector("J12", "connector_3p", 10.0, None),
        ]);
        let diags = mate_diagnostics(&project);
        let hit: Vec<_> = diags.iter().filter(|d| d.code == MATE_ONE_SIDED).collect();
        assert_eq!(hit.len(), 1, "{diags:?}");
        assert_eq!(hit[0].severity, Severity::Info);
        assert_eq!(hit[0].entity_ids.len(), 1, "片側宣言した本人だけを指す");
    }

    /// Declaring an unknown reference designator as a mate is a warning, and so is declaring oneself.
    /// 存在しない参照記号を嵌合相手に指定すると警告になり、自分自身を指定した場合も同様。
    #[test]
    fn an_unknown_or_self_reference_is_a_warning() {
        let project = project_with(vec![
            connector("J1", "connector_3p", 0.0, Some("J99")),
            connector("J2", "connector_3p", 10.0, Some("J2")),
        ]);
        let diags = mate_diagnostics(&project);
        let hit: Vec<_> = diags.iter().filter(|d| d.code == MATE_UNKNOWN).collect();
        assert_eq!(hit.len(), 2, "{diags:?}");
        assert!(hit.iter().all(|d| d.severity == Severity::Warning));
    }

    /// A connector with no `mates_with` attribute produces no mate-related finding at all.
    /// `mates_with`属性を持たないコネクタは、嵌合関連の指摘を一切出さない。
    #[test]
    fn a_connector_with_no_declaration_is_silent() {
        let project = project_with(vec![connector("J1", "connector_3p", 0.0, None)]);
        assert!(mate_diagnostics(&project).is_empty());
    }

    /// A terminal block, even though it shares the "connector" category, is not treated as a mating connector (no mate diagnostics, even with the attribute set).
    /// 端子台は「connector」カテゴリを共有していても嵌合コネクタとしては扱わない (属性を付けても嵌合の指摘は出ない)。
    #[test]
    fn a_terminal_block_is_not_a_mating_connector() {
        let project = project_with(vec![connector("TB1", "terminal_block_3p", 0.0, Some("J99"))]);
        assert!(mate_diagnostics(&project).is_empty());
    }

    /// Two mated connectors on the same sheet within 50mm produce exactly one drawn mate link.
    /// 同一シート上で50mm以内にある嵌合対は、図面に描く相手先ペアがちょうど1組できる。
    #[test]
    fn a_close_pair_on_one_sheet_produces_one_mate_link() {
        let project = project_with(vec![
            connector("J1", "connector_3p", 0.0, Some("J12")),
            connector("J12", "connector_3p", 30.0, Some("J1")),
        ]);
        let links = mate_links(&project);
        assert_eq!(links.len(), 1, "{links:?}");
    }

    /// Two mated connectors more than 50mm apart on the same sheet produce no drawn mate link (the reports still carry the pair).
    /// 同一シート上でも50mmを超えて離れている嵌合対は、図面に線を描かない(対は帳票だけが伝える)。
    #[test]
    fn a_far_pair_produces_no_mate_link() {
        let project = project_with(vec![
            connector("J1", "connector_3p", 0.0, Some("J12")),
            connector("J12", "connector_3p", 200.0, Some("J1")),
        ]);
        assert!(mate_links(&project).is_empty());
    }

    /// The mate declared on a connector instance is what the wire list / from-to report's mate column reads.
    /// コネクタインスタンスに宣言された嵌合相手が、電線リスト・From-To帳票の「嵌合相手」列にそのまま読まれる。
    #[test]
    fn declared_mate_reads_the_attribute() {
        let project = project_with(vec![connector("J1", "connector_3p", 0.0, Some("J12"))]);
        let sheet = &project.sheets[0];
        assert_eq!(declared_mate(sheet, "J1"), "J12");
        assert_eq!(declared_mate(sheet, "J2"), "", "宣言が無ければ空");
    }
}
