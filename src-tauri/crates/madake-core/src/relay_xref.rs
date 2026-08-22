//! コイル⇔接点クロスリファレンス (M4 §4)。
//!
//! 同じ参照記号 (例 `K1`) を持つリレーコイルと接点は、図面のどこに描かれていても
//! **1つのデバイス**として扱う。このモジュールはその関連付けと、図面に自動で描く
//! 2つの注記を求める。
//!
//! - **接点マップ**: コイルの下に置く表。1行 = 「端子対 | 接点の所在 `/シート.ゾーン`」。
//!   部品DB由来の接点構成 (`attrs["contact_config"]`、例 `"2NO+2NC"`) が分かるときは、
//!   まだ使っていない接点も行として並べ、所在の代わりに「—」を書く
//! - **コイル所在**: 接点の脇に置く `(/1.C2)`。その接点を動かすコイルの住所
//!
//! 住所の求め方 ([`crate::xref::zone_at`]) と表記「/シート.ゾーン」はネットラベルの
//! クロスリファレンスと共通。SVG (`svg.rs`) とキャンバス (`renderer.ts`) は同じ数式で描く。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::geometry::Point;
use crate::model::{Entity, EntityId, Project, SheetId, SymbolInstance};
use crate::symbol::SymbolDef;
use crate::verify::{Diagnostic, Severity};

/// 接点構成を持たせるシンボル属性のキー。部品DBの`contact_config`列を配置時に写す。
pub const CONTACT_CONFIG_ATTR: &str = "contact_config";
/// 未使用の接点行に書く記号 (デザイン確定: 全角ダッシュ)。
pub const CONTACT_UNUSED: &str = "—";
/// リレーコイルのシンボルid。
pub const COIL_SYMBOL_ID: &str = "relay_coil";
/// a接点 (メーク接点) のシンボルid。
pub const CONTACT_NO_SYMBOL_ID: &str = "relay_contact_no";
/// b接点 (ブレーク接点) のシンボルid。
pub const CONTACT_NC_SYMBOL_ID: &str = "relay_contact_nc";

/// 接点マップの文字高さ (mm)。
pub const CONTACT_MAP_FONT: f64 = 2.0;
/// 接点マップの行高さ (mm)。
pub const CONTACT_MAP_ROW_H: f64 = 3.5;
/// 接点マップのセル内余白 (mm)。
pub const CONTACT_MAP_PAD: f64 = 1.0;
/// 接点マップの列の最小幅 (mm)。
pub const CONTACT_MAP_MIN_COL_W: f64 = 8.0;
/// シンボル外形の下端から接点マップ上端までの間隔 (mm)。グリッドピッチと同じ。
pub const CONTACT_MAP_GAP: f64 = 2.5;
/// 文字幅の見積り係数 (文字高さに対する1文字の平均幅)。[`crate::xref`] と共通。
const CHAR_WIDTH_RATIO: f64 = 0.6;
/// シンボル外形の右端からコイル所在テキストまでの間隔 (mm)。
pub const COIL_LOCATION_GAP: f64 = 1.0;

/// リレーデバイスの機能種別。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RelayRole {
    /// コイル (駆動側)。
    Coil,
    /// a接点 (メーク接点)。
    ContactNo,
    /// b接点 (ブレーク接点)。
    ContactNc,
}

/// シンボルidからリレーの機能種別を判定する。リレー以外はNone。
pub fn relay_role(symbol_id: &str) -> Option<RelayRole> {
    match symbol_id {
        COIL_SYMBOL_ID => Some(RelayRole::Coil),
        CONTACT_NO_SYMBOL_ID => Some(RelayRole::ContactNo),
        CONTACT_NC_SYMBOL_ID => Some(RelayRole::ContactNc),
        _ => None,
    }
}

/// デバイスを構成する機能1つ (コイル1個・接点1個) の所在。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RelayFunction {
    pub reference: String,
    pub role: RelayRole,
    pub entity_id: EntityId,
    pub sheet_id: SheetId,
    /// 1始まりのシート表示順。
    pub sheet_no: usize,
    pub sheet_name: String,
    /// ゾーンアドレス (例 "C2")。
    pub zone: String,
}

impl RelayFunction {
    /// 図面上の住所表記 (IEC 61082-1)。「/シート.ゾーン」= 例 `/1.C2`。
    pub fn address(&self) -> String {
        format!("/{}.{}", self.sheet_no, self.zone)
    }
}

/// 参照記号1つ分のリレーデバイス (コイル+接点群)。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RelayDevice {
    pub reference: String,
    /// コイル (通常1個。2個以上あれば重複配置)。
    pub coils: Vec<RelayFunction>,
    /// 接点 (図面の読み順: シート番号→ゾーン→id)。この並びが端子対の連番になる。
    pub contacts: Vec<RelayFunction>,
    /// 接点構成の生文字列 (`attrs["contact_config"]`。未設定なら空)。
    pub contact_config_raw: String,
}

/// 接点構成 (部品DBの実装数)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ContactConfig {
    /// a接点の実装数。
    pub no: usize,
    /// b接点の実装数。
    pub nc: usize,
}

/// 接点マップの1行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ContactMapRow {
    /// 端子対 (例 "13-14")。
    pub terminals: String,
    /// 所在「/シート.ゾーン」。未使用の接点は [`CONTACT_UNUSED`]。
    pub address: String,
    pub role: RelayRole,
    /// 図面に置かれている接点のentity id。未使用行はNone。
    pub entity_id: Option<EntityId>,
}

/// 接点構成の文字列を読む。例 `"2NO+2NC"` → a接点2・b接点2。
///
/// 区切りは `+` / `,` / 空白。大文字小文字は問わない。個数の無い `"NO"` や
/// 種別の無い `"2"`、未知の種別 (`"2c"` 等) は読めないものとして `None` を返す
/// (数を推測しない)。
pub fn parse_contact_config(raw: &str) -> Option<ContactConfig> {
    let raw = raw.trim();
    // 区切りで始まる/終わる文字列 ("2NO+" 等) は書きかけとみなして読まない
    if raw.starts_with(['+', ',']) || raw.ends_with(['+', ',']) {
        return None;
    }
    let mut config = ContactConfig::default();
    let mut terms = 0;
    for term in raw
        .split(['+', ',', ' ', '\t'])
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        let digits: String = term.chars().take_while(char::is_ascii_digit).collect();
        let count: usize = digits.parse().ok()?;
        match term[digits.len()..].trim().to_ascii_uppercase().as_str() {
            "NO" => config.no += count,
            "NC" => config.nc += count,
            _ => return None,
        }
        terms += 1;
    }
    (terms > 0).then_some(config)
}

/// 接点の端子対 (IEC 60947-1)。先頭の数字が接点の連番 (`position`)、末尾の2桁が機能を表し、
/// a接点は3/4、b接点は1/2になる。コイルは端子対ではなく固定の端子記号 `A1-A2`。
pub fn terminal_pair(position: usize, role: RelayRole) -> String {
    match role {
        RelayRole::Coil => "A1-A2".to_string(),
        RelayRole::ContactNo => format!("{position}3-{position}4"),
        RelayRole::ContactNc => format!("{position}1-{position}2"),
    }
}

impl RelayDevice {
    /// 接点構成 (読めなければNone)。
    pub fn contact_config(&self) -> Option<ContactConfig> {
        parse_contact_config(&self.contact_config_raw)
    }

    /// コイルの下に描く接点マップ。配置済みの接点を図面の読み順に並べ、接点構成が
    /// 分かっていれば残りの未使用接点を [`CONTACT_UNUSED`] の行として続ける。
    pub fn contact_map(&self) -> Vec<ContactMapRow> {
        let mut rows: Vec<ContactMapRow> = self
            .contacts
            .iter()
            .enumerate()
            .map(|(i, c)| ContactMapRow {
                terminals: terminal_pair(i + 1, c.role),
                address: c.address(),
                role: c.role,
                entity_id: Some(c.entity_id),
            })
            .collect();
        let Some(config) = self.contact_config() else {
            return rows;
        };
        let used = |role: RelayRole| self.contacts.iter().filter(|c| c.role == role).count();
        let spare = [
            (RelayRole::ContactNo, config.no.saturating_sub(used(RelayRole::ContactNo))),
            (RelayRole::ContactNc, config.nc.saturating_sub(used(RelayRole::ContactNc))),
        ];
        for (role, count) in spare {
            for _ in 0..count {
                rows.push(ContactMapRow {
                    terminals: terminal_pair(rows.len() + 1, role),
                    address: CONTACT_UNUSED.to_string(),
                    role,
                    entity_id: None,
                });
            }
        }
        rows
    }

    /// 接点の脇に描くコイル所在 (例 `"(/1.C2)"`)。コイルが無ければNone。
    pub fn coil_location(&self) -> Option<String> {
        self.coils.first().map(|c| format!("({})", c.address()))
    }
}

/// プロジェクト内の全リレーデバイス (参照記号の昇順)。同じ参照記号のコイルと接点が
/// 1デバイスにまとまる。参照記号が空のシンボルはどのデバイスにも属さない。
pub fn relay_devices(project: &Project) -> Vec<RelayDevice> {
    let mut by_ref: BTreeMap<String, RelayDevice> = BTreeMap::new();
    for (i, sheet) in project.sheets.iter().enumerate() {
        for entity in sheet.entities.values() {
            let Entity::Symbol(s) = entity else { continue };
            let Some(role) = relay_role(&s.symbol_id) else { continue };
            let reference = s.reference.trim();
            if reference.is_empty() {
                continue;
            }
            let device = by_ref
                .entry(reference.to_string())
                .or_insert_with(|| RelayDevice {
                    reference: reference.to_string(),
                    coils: Vec::new(),
                    contacts: Vec::new(),
                    contact_config_raw: String::new(),
                });
            let function = RelayFunction {
                reference: reference.to_string(),
                role,
                entity_id: s.id,
                sheet_id: sheet.id,
                sheet_no: i + 1,
                sheet_name: sheet.name.clone(),
                zone: crate::xref::zone_at(sheet, s.at),
            };
            match role {
                RelayRole::Coil => device.coils.push(function),
                _ => device.contacts.push(function),
            }
            // 接点構成はコイルに付いたものを優先し、無ければ接点のものを使う
            if let Some(raw) = s.attrs.get(CONTACT_CONFIG_ATTR).map(|v| v.trim()) {
                if !raw.is_empty()
                    && (device.contact_config_raw.is_empty() || role == RelayRole::Coil)
                {
                    device.contact_config_raw = raw.to_string();
                }
            }
        }
    }
    let order = |f: &RelayFunction| (f.sheet_no, f.zone.clone(), f.entity_id);
    by_ref
        .into_values()
        .map(|mut d| {
            d.coils.sort_by_key(order);
            d.contacts.sort_by_key(order);
            d
        })
        .collect()
}

/// シート1枚分の接点マップ表 (コイルのentity id → 表の行)。SVG・キャンバスの描画はこれを引く。
/// 接点を1つも持たないコイルは含まれない (描く表が無い)。
pub fn sheet_contact_maps(
    project: &Project,
    sheet_id: SheetId,
) -> BTreeMap<EntityId, Vec<ContactMapRow>> {
    let mut out = BTreeMap::new();
    for device in relay_devices(project) {
        let rows = device.contact_map();
        if rows.is_empty() {
            continue;
        }
        for coil in device.coils.iter().filter(|c| c.sheet_id == sheet_id) {
            out.insert(coil.entity_id, rows.clone());
        }
    }
    out
}

/// シート1枚分のコイル所在表 (接点のentity id → `"(/1.C2)"`)。
/// コイルの見つからない接点は含まれない (何も描かない)。
pub fn sheet_coil_locations(project: &Project, sheet_id: SheetId) -> BTreeMap<EntityId, String> {
    let mut out = BTreeMap::new();
    for device in relay_devices(project) {
        let Some(location) = device.coil_location() else { continue };
        for contact in device.contacts.iter().filter(|c| c.sheet_id == sheet_id) {
            out.insert(contact.entity_id, location.clone());
        }
    }
    out
}

/// 接点マップの表の配置 (用紙座標mm)。行はY下方向に積む。
#[derive(Debug, Clone, PartialEq)]
pub struct ContactMapLayout {
    /// 表の左上X。
    pub x: f64,
    /// 表の左上Y。
    pub y: f64,
    /// 列幅 (端子対の列, 所在の列)。
    pub col_w: [f64; 2],
    pub row_h: f64,
    pub rows: usize,
}

impl ContactMapLayout {
    pub fn width(&self) -> f64 {
        self.col_w[0] + self.col_w[1]
    }
    pub fn height(&self) -> f64 {
        self.row_h * self.rows as f64
    }
    /// i行目 (0始まり) の上端Y。
    pub fn row_top(&self, i: usize) -> f64 {
        self.y + self.row_h * i as f64
    }
    /// i行目の文字のベースラインY (行の中央に文字を置く)。
    pub fn baseline(&self, i: usize) -> f64 {
        self.row_top(i) + self.row_h / 2.0 + CONTACT_MAP_FONT * 0.35
    }
    /// col列目 (0または1) の左端X。
    pub fn col_x(&self, col: usize) -> f64 {
        self.x + if col == 0 { 0.0 } else { self.col_w[0] }
    }
    /// col列目の文字の左端X (セル内余白を空ける)。
    pub fn text_x(&self, col: usize) -> f64 {
        self.col_x(col) + CONTACT_MAP_PAD
    }
}

/// 接点マップの表の配置を求める。`center_x`を中心に左右対称、`top_y`から下へ伸びる。
/// 列幅は中身の文字数から見積り、[`CONTACT_MAP_MIN_COL_W`] を下回らない。
pub fn contact_map_layout(rows: &[ContactMapRow], center_x: f64, top_y: f64) -> ContactMapLayout {
    let width_of = |texts: &dyn Fn(&ContactMapRow) -> &str| {
        let chars = rows.iter().map(|r| texts(r).chars().count()).max().unwrap_or(0);
        (CONTACT_MAP_FONT * CHAR_WIDTH_RATIO * chars as f64 + 2.0 * CONTACT_MAP_PAD)
            .max(CONTACT_MAP_MIN_COL_W)
    };
    let col_w = [
        width_of(&|r: &ContactMapRow| r.terminals.as_str()),
        width_of(&|r: &ContactMapRow| r.address.as_str()),
    ];
    ContactMapLayout {
        x: center_x - (col_w[0] + col_w[1]) / 2.0,
        y: top_y,
        col_w,
        row_h: CONTACT_MAP_ROW_H,
        rows: rows.len(),
    }
}

/// コイルの接点マップの基準点 (表の中心X, 表の上端Y)。シンボル外形の下端から
/// [`CONTACT_MAP_GAP`] だけ空けてぶら下げる。
pub fn contact_map_origin(inst: &SymbolInstance, def: &SymbolDef) -> Point {
    let (_, max) = crate::svg::symbol_bounds(inst, def);
    Point::new(inst.at.x, max.y + CONTACT_MAP_GAP)
}

/// 接点の脇に置くコイル所在テキストの基準点 (テキストは左揃え)。
/// シンボル外形の右端から [`COIL_LOCATION_GAP`] だけ空ける。
pub fn coil_location_at(inst: &SymbolInstance, def: &SymbolDef) -> Point {
    let (_, max) = crate::svg::symbol_bounds(inst, def);
    Point::new(max.x + COIL_LOCATION_GAP, inst.at.y)
}

/// コイル⇔接点クロスリファレンスの例外レポート (ACADEのException report相当)。
///
/// - 親の無い接点 = Error: 同じ参照記号のコイルがどこにも無い接点は、どの信号で動くか
///   決まらないので図面として成立しない
/// - 接点の無いコイル = Warning: 作図途中でこれから接点を足す場合があるため
/// - 接点数超過 = Error: 部品の実装数を超えて接点を使っている
/// - 読めない接点構成 = Warning: 接点数の検証が黙って効かなくなるため知らせる
pub fn relay_diagnostics(project: &Project) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    for device in relay_devices(project) {
        let reference = &device.reference;
        if device.coils.is_empty() {
            let sheet_id = device.contacts[0].sheet_id;
            diags.push(Diagnostic {
                severity: Severity::Error,
                code: "erc.orphan_contact".into(),
                message: format!("{reference}: 接点に対応するコイルがありません"),
                sheet_id,
                entity_ids: device.contacts.iter().map(|c| c.entity_id).collect(),
            });
            continue;
        }
        let sheet_id = device.coils[0].sheet_id;
        if device.contacts.is_empty() {
            diags.push(Diagnostic {
                severity: Severity::Warning,
                code: "erc.coil_without_contact".into(),
                message: format!("{reference}: コイルが接点を1つも動かしていません"),
                sheet_id,
                entity_ids: device.coils.iter().map(|c| c.entity_id).collect(),
            });
        }
        if device.contact_config_raw.is_empty() {
            continue;
        }
        let Some(config) = device.contact_config() else {
            diags.push(Diagnostic {
                severity: Severity::Warning,
                code: "erc.invalid_contact_config".into(),
                message: format!(
                    "{reference}: 接点構成「{}」を読み取れないので接点数を検証できません (例: 2NO+2NC)",
                    device.contact_config_raw
                ),
                sheet_id,
                entity_ids: device.coils.iter().map(|c| c.entity_id).collect(),
            });
            continue;
        };
        let mut over: Vec<String> = Vec::new();
        let mut ids: Vec<EntityId> = device.coils.iter().map(|c| c.entity_id).collect();
        for (role, label, available) in [
            (RelayRole::ContactNo, "a接点", config.no),
            (RelayRole::ContactNc, "b接点", config.nc),
        ] {
            let used: Vec<&RelayFunction> =
                device.contacts.iter().filter(|c| c.role == role).collect();
            if used.len() > available {
                over.push(format!("{label} {}個 (実装 {available}個)", used.len()));
                ids.extend(used.iter().map(|c| c.entity_id));
            }
        }
        if !over.is_empty() {
            diags.push(Diagnostic {
                severity: Severity::Error,
                code: "erc.contact_overflow".into(),
                message: format!(
                    "{reference}: 接点構成「{}」の実装数を超えて使っています: {}",
                    device.contact_config_raw,
                    over.join(", ")
                ),
                sheet_id,
                entity_ids: ids,
            });
        }
    }
    diags
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;
    use uuid::Uuid;

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

    fn with_attr(entity: Entity, key: &str, value: &str) -> Entity {
        let Entity::Symbol(mut s) = entity else { unreachable!() };
        s.attrs.insert(key.into(), value.into());
        Entity::Symbol(s)
    }

    /// エンティティを並べたプロジェクトを組み立てる (`sheet index`, entity)。
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

    /// A coil and the contacts that carry the same reference designator form one relay device.
    /// 同じ参照記号を持つコイルと接点は、1つのリレーデバイスとしてまとめられる。
    #[test]
    fn same_reference_groups_coil_and_contacts_into_one_device() {
        let project = project_with(
            1,
            vec![
                (0, symbol(COIL_SYMBOL_ID, "K1", 20.0, 20.0)),
                (0, symbol(CONTACT_NO_SYMBOL_ID, "K1", 60.0, 20.0)),
                (0, symbol(CONTACT_NC_SYMBOL_ID, "K1", 100.0, 20.0)),
                (0, symbol(COIL_SYMBOL_ID, "K2", 20.0, 60.0)),
            ],
        );
        let devices = relay_devices(&project);
        assert_eq!(
            devices.iter().map(|d| d.reference.as_str()).collect::<Vec<_>>(),
            vec!["K1", "K2"],
            "参照記号順に並ぶ"
        );
        assert_eq!(devices[0].coils.len(), 1);
        assert_eq!(devices[0].contacts.len(), 2);
        assert_eq!(devices[1].contacts.len(), 0);
    }

    /// Symbols that are not relay coils or contacts never become part of a relay device.
    /// リレーのコイル・接点以外のシンボルは、リレーデバイスには含まれない。
    #[test]
    fn non_relay_symbols_are_not_relay_devices() {
        let project = project_with(1, vec![(0, symbol("lamp", "L1", 20.0, 20.0))]);
        assert!(relay_devices(&project).is_empty());
        assert_eq!(relay_role("lamp"), None);
    }

    /// The contacts of a device are ordered by sheet, then by zone, so the numbering follows the reading order of the drawing.
    /// デバイスの接点はシート順→ゾーン順に並ぶので、端子対の連番は図面の読み順どおりになる。
    #[test]
    fn contacts_are_ordered_by_sheet_then_zone() {
        let project = project_with(
            2,
            vec![
                (0, symbol(COIL_SYMBOL_ID, "K1", 20.0, 20.0)),
                // シート2のゾーンB3 (後ろ) を先に入れる
                (1, symbol(CONTACT_NO_SYMBOL_ID, "K1", 250.0, 70.0)),
                (0, symbol(CONTACT_NO_SYMBOL_ID, "K1", 250.0, 70.0)),
                (0, symbol(CONTACT_NO_SYMBOL_ID, "K1", 20.0, 20.0)),
            ],
        );
        let device = &relay_devices(&project)[0];
        let addresses: Vec<String> = device.contacts.iter().map(RelayFunction::address).collect();
        assert_eq!(addresses, vec!["/1.A1", "/1.B3", "/2.B3"]);
    }

    /// Terminal pairs follow IEC 60947-1: the leading digit is the contact position, the last digits are 3/4 for make contacts and 1/2 for break contacts.
    /// 端子対はIEC 60947-1に従い、先頭の数字が接点の連番、末尾がa接点なら3/4・b接点なら1/2になる。
    #[test]
    fn terminal_pairs_follow_iec_position_and_function_digits() {
        assert_eq!(terminal_pair(1, RelayRole::ContactNo), "13-14");
        assert_eq!(terminal_pair(2, RelayRole::ContactNo), "23-24");
        assert_eq!(terminal_pair(1, RelayRole::ContactNc), "11-12");
        assert_eq!(terminal_pair(2, RelayRole::ContactNc), "21-22");
        assert_eq!(terminal_pair(3, RelayRole::ContactNc), "31-32");
        // コイルは端子対ではなく固定の端子記号
        assert_eq!(terminal_pair(1, RelayRole::Coil), "A1-A2");
    }

    /// The contact map under a coil lists every placed contact with its terminal pair and its drawing address.
    /// コイル下の接点マップには、配置済みの接点が端子対と図面上の住所とともに並ぶ。
    #[test]
    fn contact_map_lists_used_contacts_with_their_addresses() {
        let project = project_with(
            2,
            vec![
                (0, symbol(COIL_SYMBOL_ID, "K1", 20.0, 20.0)),
                (0, symbol(CONTACT_NO_SYMBOL_ID, "K1", 250.0, 70.0)),
                (1, symbol(CONTACT_NC_SYMBOL_ID, "K1", 250.0, 70.0)),
            ],
        );
        let rows = relay_devices(&project)[0].contact_map();
        assert_eq!(
            rows.iter()
                .map(|r| (r.terminals.as_str(), r.address.as_str()))
                .collect::<Vec<_>>(),
            vec![("13-14", "/1.B3"), ("21-22", "/2.B3")],
            "1個目=a接点13-14、2個目=b接点21-22"
        );
        assert!(rows.iter().all(|r| r.entity_id.is_some()));
    }

    /// When the part's contact configuration is known, the unused contacts are listed too, with a dash instead of an address.
    /// 部品の接点構成が分かっているときは、まだ使っていない接点も行として並び、所在の代わりに「—」が入る。
    #[test]
    fn contact_map_shows_dash_for_unused_contacts() {
        let project = project_with(
            1,
            vec![
                (
                    0,
                    with_attr(
                        symbol(COIL_SYMBOL_ID, "K1", 20.0, 20.0),
                        CONTACT_CONFIG_ATTR,
                        "2NO+2NC",
                    ),
                ),
                (0, symbol(CONTACT_NO_SYMBOL_ID, "K1", 250.0, 70.0)),
            ],
        );
        let rows = relay_devices(&project)[0].contact_map();
        assert_eq!(
            rows.iter()
                .map(|r| (r.terminals.as_str(), r.address.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("13-14", "/1.B3"),
                ("23-24", CONTACT_UNUSED),
                ("31-32", CONTACT_UNUSED),
                ("41-42", CONTACT_UNUSED),
            ],
            "使用中の1個目に続けて、残りのa接点1個・b接点2個が未使用行になる"
        );
        assert!(rows[1..].iter().all(|r| r.entity_id.is_none()));
    }

    /// Without a contact configuration only the contacts actually drawn are listed; no empty rows are invented.
    /// 接点構成が分からないときは実際に描かれた接点だけが並び、空の行は作られない。
    #[test]
    fn contact_map_omits_unused_rows_without_contact_config() {
        let project = project_with(
            1,
            vec![
                (0, symbol(COIL_SYMBOL_ID, "K1", 20.0, 20.0)),
                (0, symbol(CONTACT_NO_SYMBOL_ID, "K1", 250.0, 70.0)),
            ],
        );
        let rows = relay_devices(&project)[0].contact_map();
        assert_eq!(rows.len(), 1);
    }

    /// Each contact shows the address of the coil that drives it, in parentheses.
    /// それぞれの接点の脇には、その接点を動かすコイルの住所が丸括弧付きで出る。
    #[test]
    fn contact_shows_the_location_of_its_coil() {
        let project = project_with(
            2,
            vec![
                (0, symbol(COIL_SYMBOL_ID, "K1", 250.0, 70.0)),
                (1, symbol(CONTACT_NO_SYMBOL_ID, "K1", 20.0, 20.0)),
            ],
        );
        let s2 = project.sheets[1].id;
        let contact_id = project.sheets[1].entities.keys().next().copied().unwrap();
        assert_eq!(
            sheet_coil_locations(&project, s2).get(&contact_id).map(String::as_str),
            Some("(/1.B3)")
        );
        // コイル側のシートには接点が無いので所在表示も無い
        assert!(sheet_coil_locations(&project, project.sheets[0].id).is_empty());
    }

    /// A contact whose coil is missing shows no coil location at all.
    /// コイルが見つからない接点には、コイル所在が一切表示されない。
    #[test]
    fn contact_without_a_coil_shows_no_location() {
        let project = project_with(1, vec![(0, symbol(CONTACT_NO_SYMBOL_ID, "K9", 20.0, 20.0))]);
        assert!(sheet_coil_locations(&project, project.sheets[0].id).is_empty());
    }

    /// The per-sheet contact maps are keyed by the coil entity, so only coils on that sheet carry a table.
    /// シートごとの接点マップはコイルのentity idで引くので、そのシートに居るコイルだけが表を持つ。
    #[test]
    fn sheet_contact_maps_are_keyed_by_the_coil_on_that_sheet() {
        let project = project_with(
            2,
            vec![
                (0, symbol(COIL_SYMBOL_ID, "K1", 20.0, 20.0)),
                (1, symbol(CONTACT_NO_SYMBOL_ID, "K1", 20.0, 20.0)),
            ],
        );
        let coil_id = project.sheets[0].entities.keys().next().copied().unwrap();
        let maps = sheet_contact_maps(&project, project.sheets[0].id);
        assert_eq!(maps.len(), 1);
        assert_eq!(maps[&coil_id].len(), 1);
        assert!(sheet_contact_maps(&project, project.sheets[1].id).is_empty());
    }

    /// A contact configuration like "2NO+2NC" is read as the number of make and break contacts the part actually has.
    /// 「2NO+2NC」のような接点構成は、その部品が実際に持つa接点・b接点の数として読み取られる。
    #[test]
    fn contact_config_parses_make_and_break_counts() {
        assert_eq!(parse_contact_config("2NO+2NC"), Some(ContactConfig { no: 2, nc: 2 }));
        assert_eq!(parse_contact_config("4NO"), Some(ContactConfig { no: 4, nc: 0 }));
        assert_eq!(parse_contact_config(" 1no , 3nc "), Some(ContactConfig { no: 1, nc: 3 }));
    }

    /// An unreadable contact configuration is ignored instead of guessing a number of contacts.
    /// 読み取れない接点構成は、接点数を推測せずに無視される。
    #[test]
    fn unreadable_contact_config_is_ignored() {
        assert_eq!(parse_contact_config(""), None);
        assert_eq!(parse_contact_config("2c"), None);
        assert_eq!(parse_contact_config("NO+NC"), None);
        assert_eq!(parse_contact_config("2NO+"), None);
    }

    /// Using more contacts than the assigned part provides is an error, naming the type that ran out.
    /// 割り当てた部品が持つ数より多くの接点を使うとエラーになり、足りない接点の種別が示される。
    #[test]
    fn using_more_contacts_than_the_part_has_is_an_error() {
        let project = project_with(
            1,
            vec![
                (
                    0,
                    with_attr(
                        symbol(COIL_SYMBOL_ID, "K1", 20.0, 20.0),
                        CONTACT_CONFIG_ATTR,
                        "2NO+2NC",
                    ),
                ),
                (0, symbol(CONTACT_NO_SYMBOL_ID, "K1", 60.0, 20.0)),
                (0, symbol(CONTACT_NO_SYMBOL_ID, "K1", 100.0, 20.0)),
                (0, symbol(CONTACT_NO_SYMBOL_ID, "K1", 140.0, 20.0)),
            ],
        );
        let diags = relay_diagnostics(&project);
        let d = diags
            .iter()
            .find(|d| d.code == "erc.contact_overflow")
            .expect("接点数超過");
        assert_eq!(d.severity, Severity::Error);
        assert!(d.message.contains("K1"), "{}", d.message);
        assert!(d.message.contains("a接点"), "{}", d.message);
        assert!(d.message.contains("2NO+2NC"), "{}", d.message);
        // 2個までなら指摘は出ない
        assert!(!diags.iter().any(|d| d.code == "erc.contact_overflow" && d.message.contains("b接点")));
    }

    /// A contact with no coil of the same reference anywhere in the project is an error.
    /// プロジェクトのどこにも同じ参照記号のコイルが無い接点はエラーになる。
    #[test]
    fn a_contact_without_a_coil_is_an_error() {
        let project = project_with(1, vec![(0, symbol(CONTACT_NO_SYMBOL_ID, "K9", 20.0, 20.0))]);
        let d = relay_diagnostics(&project)
            .into_iter()
            .find(|d| d.code == "erc.orphan_contact")
            .expect("親の無い接点");
        assert_eq!(d.severity, Severity::Error);
        assert!(d.message.contains("K9"), "{}", d.message);
        assert_eq!(d.entity_ids.len(), 1);
    }

    /// A coil that drives no contact at all is only a warning, because the contact may still be planned.
    /// 接点を1つも動かしていないコイルは、これから足す可能性があるので警告にとどまる。
    #[test]
    fn a_coil_without_contacts_is_a_warning() {
        let project = project_with(1, vec![(0, symbol(COIL_SYMBOL_ID, "K1", 20.0, 20.0))]);
        let d = relay_diagnostics(&project)
            .into_iter()
            .find(|d| d.code == "erc.coil_without_contact")
            .expect("接点の無いコイル");
        assert_eq!(d.severity, Severity::Warning);
        assert!(d.message.contains("K1"), "{}", d.message);
    }

    /// A contact configuration that cannot be read is reported as a warning, because it silently disables the contact count check.
    /// 読み取れない接点構成は、接点数の検証が黙って効かなくなるため警告として報告される。
    #[test]
    fn an_unreadable_contact_config_is_a_warning() {
        let project = project_with(
            1,
            vec![
                (
                    0,
                    with_attr(symbol(COIL_SYMBOL_ID, "K1", 20.0, 20.0), CONTACT_CONFIG_ATTR, "2c"),
                ),
                (0, symbol(CONTACT_NO_SYMBOL_ID, "K1", 60.0, 20.0)),
            ],
        );
        let diags = relay_diagnostics(&project);
        let d = diags
            .iter()
            .find(|d| d.code == "erc.invalid_contact_config")
            .expect("不正な接点構成");
        assert_eq!(d.severity, Severity::Warning);
        assert!(d.message.contains("2c"), "{}", d.message);
        assert!(!diags.iter().any(|d| d.code == "erc.contact_overflow"), "超過検証はしない");
    }

    /// A correctly wired relay (coil plus contacts within the part's configuration) produces no cross-reference diagnostics.
    /// 正しく組まれたリレー(コイル+接点構成の範囲内の接点)には、クロスリファレンスの指摘が一切出ない。
    #[test]
    fn a_correct_relay_produces_no_diagnostics() {
        let project = project_with(
            1,
            vec![
                (
                    0,
                    with_attr(
                        symbol(COIL_SYMBOL_ID, "K1", 20.0, 20.0),
                        CONTACT_CONFIG_ATTR,
                        "2NO+2NC",
                    ),
                ),
                (0, symbol(CONTACT_NO_SYMBOL_ID, "K1", 60.0, 20.0)),
                (0, symbol(CONTACT_NC_SYMBOL_ID, "K1", 100.0, 20.0)),
            ],
        );
        assert!(relay_diagnostics(&project).is_empty(), "{:?}", relay_diagnostics(&project));
    }

    /// The whole-project verification includes the relay cross-reference checks.
    /// プロジェクト全体の検証には、コイル⇔接点クロスリファレンスの検査が含まれる。
    #[test]
    fn project_verification_includes_relay_checks() {
        let project = project_with(1, vec![(0, symbol(CONTACT_NO_SYMBOL_ID, "K9", 20.0, 20.0))]);
        let diags = crate::verify::verify_project(&project);
        assert!(diags.iter().any(|d| d.code == "erc.orphan_contact"), "{diags:?}");
    }

    /// The contact map table is centred under the coil and grows downwards, one row per contact.
    /// 接点マップの表はコイルの真下に中央揃えで置かれ、接点1個につき1行ずつ下へ伸びる。
    #[test]
    fn contact_map_table_is_centred_under_the_coil() {
        let rows = vec![
            ContactMapRow {
                terminals: "13-14".into(),
                address: "/2.B3".into(),
                role: RelayRole::ContactNo,
                entity_id: None,
            },
            ContactMapRow {
                terminals: "21-22".into(),
                address: CONTACT_UNUSED.into(),
                role: RelayRole::ContactNc,
                entity_id: None,
            },
        ];
        let layout = contact_map_layout(&rows, 100.0, 60.0);
        assert!((layout.x + layout.width() / 2.0 - 100.0).abs() < 1e-9, "中央揃え");
        assert!((layout.y - 60.0).abs() < 1e-9);
        assert!((layout.height() - 2.0 * CONTACT_MAP_ROW_H).abs() < 1e-9);
        assert!((layout.row_top(1) - (60.0 + CONTACT_MAP_ROW_H)).abs() < 1e-9);
        // 2列: 端子対の列 + 所在の列。どちらも最小幅以上
        assert!(layout.col_w[0] >= CONTACT_MAP_MIN_COL_W);
        assert!(layout.col_w[1] >= CONTACT_MAP_MIN_COL_W);
        assert!((layout.col_x(1) - (layout.x + layout.col_w[0])).abs() < 1e-9);
    }

    /// The contact map hangs below the coil symbol's outline, and the coil location text sits to the right of the contact symbol.
    /// 接点マップはコイルの外形の下にぶら下がり、コイル所在の文字は接点シンボルの右脇に置かれる。
    #[test]
    fn annotations_are_anchored_to_the_symbol_outline() {
        let coil = SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: COIL_SYMBOL_ID.into(),
            at: Point::new(100.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "K1".into(),
            value: String::new(),
            attrs: Default::default(),
        };
        let def = crate::symbol::resolve_symbol(COIL_SYMBOL_ID).unwrap();
        let origin = contact_map_origin(&coil, &def);
        assert!((origin.x - 100.0).abs() < 1e-9, "コイルの中心に揃える");
        assert!(origin.y > 50.0 + 3.0, "外形(高さ±3mm)より下: {origin:?}");

        let contact_def = crate::symbol::resolve_symbol(CONTACT_NO_SYMBOL_ID).unwrap();
        let contact = SymbolInstance {
            symbol_id: CONTACT_NO_SYMBOL_ID.into(),
            ..coil.clone()
        };
        let at = coil_location_at(&contact, &contact_def);
        assert!(at.x > 100.0 + 7.5, "外形(右端+7.5mm)より右: {at:?}");
        assert!((at.y - 50.0).abs() < 1e-9, "シンボルの高さ中央");
    }
}
