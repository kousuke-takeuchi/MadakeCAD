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
use crate::model::{Entity, EntityId, Project, Sheet, SheetId, SymbolInstance};
use crate::symbol::SymbolDef;
use crate::verify::{Diagnostic, Severity};
use crate::wire_note::{NoteAlign, WireNote};

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
/// c接点 (切換接点) のシンボルid。1極でa接点とb接点を兼ねる。
pub const CONTACT_CO_SYMBOL_ID: &str = "relay_contact_co";

/// 接点マップの文字高さ (mm)。
pub const CONTACT_MAP_FONT: f64 = 2.0;
/// 行の中央に文字を置くときの、行の中央高さからベースラインまでの下向きのずれ (mm)。
pub const CONTACT_MAP_BASELINE_SHIFT: f64 = CONTACT_MAP_FONT * 0.35;
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
/// シンボル外形の上端・下端からコイル所在テキストまでの間隔 (mm、G22の上下への逃がし)。
pub const COIL_LOCATION_VGAP: f64 = 1.0;
/// コイルを実機ブロックとして描かせるインスタンス属性のキー (G7)。値`"true"`で有効。
pub const DEVICE_BLOCK_ATTR: &str = "device_block";
/// コイルのピン間隔 (mm)。`relay_coil`シンボルのA1/A2ピンはこの半分(±7.5mm)にある。
/// 実機ブロックの外形はこの幅を下回らない(ピンの位置を変えずに済むように)。
pub const DEVICE_BLOCK_MIN_WIDTH: f64 = 15.0;

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
    /// c接点 (切換接点)。1極で共通・b側・a側の3端子を持ち、a接点にもb接点にも使える。
    ContactCo,
}

/// シンボルidからリレーの機能種別を判定する。リレー以外はNone。
pub fn relay_role(symbol_id: &str) -> Option<RelayRole> {
    match symbol_id {
        COIL_SYMBOL_ID => Some(RelayRole::Coil),
        CONTACT_NO_SYMBOL_ID => Some(RelayRole::ContactNo),
        CONTACT_NC_SYMBOL_ID => Some(RelayRole::ContactNc),
        CONTACT_CO_SYMBOL_ID => Some(RelayRole::ContactCo),
        _ => None,
    }
}

/// コイルが実機ブロック表示 (G7) を使うか。`attrs["device_block"] == "true"`のときだけ有効
/// (分散配置が既定)。コイル以外のシンボルに付いていても効果は無い。
pub fn is_device_block(inst: &SymbolInstance) -> bool {
    inst.attrs.get(DEVICE_BLOCK_ATTR).map(String::as_str) == Some("true")
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

/// 接点構成 (部品DBの実装数)。使用中の接点の数を数えるときにも使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ContactConfig {
    /// a接点の実装数。
    pub no: usize,
    /// b接点の実装数。
    pub nc: usize,
    /// c接点 (切換接点) の実装数。
    #[serde(default)]
    pub co: usize,
}

/// 使用中の接点を接点構成へ割り当てた結果。
///
/// c接点1極は共通端子の倒れる向きでa接点にもb接点にもなるので、**a接点・b接点の
/// 専用極が足りないときはc接点の空き極から充当する**。逆にc接点の図記号を描いた
/// ときは切換動作そのものを使うので、c接点の極でしか賄えない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ContactAllocation {
    /// まだ使っていない極数 (未使用接点の行になる)。
    pub spare: ContactConfig,
    /// 実装数では賄えなかった数 (0でなければ接点数超過)。
    pub shortage: ContactConfig,
}

/// 使用中の接点数 (`used`) を接点構成 (`config`) の極へ割り当てる。
pub fn allocate_contacts(config: ContactConfig, used: ContactConfig) -> ContactAllocation {
    let mut co_left = config.co;
    let mut shortage = ContactConfig::default();
    // c接点はc接点の極でしか作れない
    shortage.co = used.co.saturating_sub(co_left);
    co_left = co_left.saturating_sub(used.co);
    // a接点・b接点は専用極を先に使い、足りない分をc接点の空き極から借りる
    let borrow = |used: usize, available: usize, co_left: &mut usize| {
        let mut deficit = used.saturating_sub(available);
        let taken = deficit.min(*co_left);
        *co_left -= taken;
        deficit -= taken;
        deficit
    };
    shortage.no = borrow(used.no, config.no, &mut co_left);
    shortage.nc = borrow(used.nc, config.nc, &mut co_left);
    ContactAllocation {
        spare: ContactConfig {
            no: config.no.saturating_sub(used.no),
            nc: config.nc.saturating_sub(used.nc),
            co: co_left,
        },
        shortage,
    }
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

/// 接点構成の文字列を読む。例 `"2NO+2NC"` → a接点2・b接点2、`"2CO"` → c接点2。
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
            "CO" => config.co += count,
            _ => return None,
        }
        terms += 1;
    }
    (terms > 0).then_some(config)
}

/// 接点の端子対 (IEC 60947-1 / IEC 60947-5-1)。先頭の数字が接点の連番 (`position`)、
/// 末尾の1桁が機能を表し、a接点は3/4、b接点は1/2になる。
///
/// c接点 (切換接点) は共通=1・b側=2・a側=4 の**3端子**を1極で持つので、対ではなく
/// `11-12-14` と3つ並べて書く (リレーソケットの表記と同じ)。
/// コイルは端子対ではなく固定の端子記号 `A1-A2`。
pub fn terminal_pair(position: usize, role: RelayRole) -> String {
    match role {
        RelayRole::Coil => "A1-A2".to_string(),
        RelayRole::ContactNo => format!("{position}3-{position}4"),
        RelayRole::ContactNc => format!("{position}1-{position}2"),
        RelayRole::ContactCo => format!("{position}1-{position}2-{position}4"),
    }
}

impl RelayDevice {
    /// 接点構成 (読めなければNone)。
    pub fn contact_config(&self) -> Option<ContactConfig> {
        parse_contact_config(&self.contact_config_raw)
    }

    /// 図面に描かれている接点の数を種別ごとに数えたもの。
    pub fn used_contacts(&self) -> ContactConfig {
        let count = |role: RelayRole| self.contacts.iter().filter(|c| c.role == role).count();
        ContactConfig {
            no: count(RelayRole::ContactNo),
            nc: count(RelayRole::ContactNc),
            co: count(RelayRole::ContactCo),
        }
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
        let spare = allocate_contacts(config, self.used_contacts()).spare;
        let spare = [
            (RelayRole::ContactNo, spare.no),
            (RelayRole::ContactNc, spare.nc),
            (RelayRole::ContactCo, spare.co),
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

    /// このデバイスの実機ブロック表示 (G7) の中身。`part_no`はコイルインスタンスの
    /// `value` (BOMが型番/値として使うのと同じ欄)。接点の並びは接点マップと同じ。
    pub fn device_block(&self, part_no: &str) -> DeviceBlock {
        DeviceBlock {
            reference: self.reference.clone(),
            part_no: part_no.trim().to_string(),
            contacts: self.contact_map(),
        }
    }
}

/// 実機ブロック表示 (G7) の中身: コイル+同じ参照記号を持つ接点を1つの外形にまとめて描く。
/// 分散配置 (コイルを小さく描いて接点マップを下にぶら下げる、既定の表示) との切替は
/// [`is_device_block`] が読む`attrs["device_block"]`で行う。コイル・接点のピン位置は
/// どちらの表示でも変わらないので、切り替えても配線は繋がったまま。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeviceBlock {
    pub reference: String,
    /// 型番 (空なら型番の行を描かない)。
    pub part_no: String,
    /// 接点の「機器イメージ」: 端子対+所在を接点マップと同じ並びで持つ。
    pub contacts: Vec<ContactMapRow>,
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
/// 接点を1つも持たないコイルは含まれない (描く表が無い)。実機ブロック表示 (G7、
/// `attrs["device_block"]`) のコイルも含まれない: 接点の並びはブロックの中に描かれるので、
/// 別表として二重に出さない。
pub fn sheet_contact_maps(
    project: &Project,
    sheet_id: SheetId,
) -> BTreeMap<EntityId, Vec<ContactMapRow>> {
    let mut out = BTreeMap::new();
    let sheet = project.sheet(sheet_id);
    for device in relay_devices(project) {
        let rows = device.contact_map();
        if rows.is_empty() {
            continue;
        }
        for coil in device.coils.iter().filter(|c| c.sheet_id == sheet_id) {
            if instance_is_device_block(sheet, coil.entity_id) {
                continue;
            }
            out.insert(coil.entity_id, rows.clone());
        }
    }
    out
}

/// あるentity idのシンボルが実機ブロック表示を使っているか (見つからなければfalse)。
fn instance_is_device_block(sheet: Option<&Sheet>, entity_id: EntityId) -> bool {
    match sheet.and_then(|s| s.entities.get(&entity_id)) {
        Some(Entity::Symbol(s)) => is_device_block(s),
        _ => false,
    }
}

/// シート1枚分の実機ブロック表示 (コイルのentity id → ブロックの中身)。`device_block`属性の
/// 付いたコイルだけが対象。SVG・キャンバスの描画はこれを引く。
pub fn sheet_device_blocks(project: &Project, sheet_id: SheetId) -> BTreeMap<EntityId, DeviceBlock> {
    let mut out = BTreeMap::new();
    let Some(sheet) = project.sheet(sheet_id) else {
        return out;
    };
    let devices = relay_devices(project);
    for entity in sheet.entities.values() {
        let Entity::Symbol(s) = entity else { continue };
        if s.symbol_id != COIL_SYMBOL_ID || !is_device_block(s) {
            continue;
        }
        let reference = s.reference.trim();
        if reference.is_empty() {
            continue;
        }
        let Some(device) = devices.iter().find(|d| d.reference == reference) else {
            continue;
        };
        out.insert(s.id, device.device_block(&s.value));
    }
    out
}

/// コイル⇔接点クロスリファレンスが所在を自動で注記するインスタンス (プロジェクト全体)。
/// [`sheet_contact_maps`]・[`sheet_device_blocks`]・[`sheet_coil_locations`] が拾うものと
/// 同じ顔ぶれ。デバイス単位のシート間クロスリファレンス (G13、[`crate::xref`]) は、この
/// 顔ぶれだけを避ける: 接点を1つも持たないコイルのように、コイル⇔接点クロスリファレンスが
/// 何も描かないリレーシンボルは避けない (何の所在も出ないままになってしまうので)。
pub fn annotated_instances(project: &Project) -> std::collections::BTreeSet<EntityId> {
    let mut out = std::collections::BTreeSet::new();
    for device in relay_devices(project) {
        // 接点マップ (別表) か実機ブロックが描かれるコイル
        let has_rows = !device.contact_map().is_empty();
        for coil in &device.coils {
            if has_rows || instance_is_device_block(project.sheet(coil.sheet_id), coil.entity_id) {
                out.insert(coil.entity_id);
            }
        }
        // コイル所在が描かれる接点
        if device.coil_location().is_some() {
            out.extend(device.contacts.iter().map(|c| c.entity_id));
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
        self.row_top(i) + self.row_h / 2.0 + CONTACT_MAP_BASELINE_SHIFT
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

/// 実機ブロック表示 (G7) の外形の配置 (**シンボルのローカル座標**mm、配置基準点が原点)。
/// 上からヘッダ行 (参照記号、型番があればもう1行) → 接点の行、の順に積む。
///
/// 表は**常に正立**で描く: インスタンスの回転・ミラーは外形にも行にも効かせない
/// ([`device_block_bounds`]は配置基準点へ平行移動するだけ)。回すのは**ピンの位置だけ**で、
/// 引き出し線は回った先のピンから**ピン自身の軸**(配線が来る向き)に沿って外形まで伸びる
/// ([`device_block_lead_end`])。表を回すと複数行の文字が互いに重なって読めなくなるため。
///
/// ピンの高さ (ローカルY=0) は**ヘッダ1行目の中央**に来る。行の仕切り線がピンの高さに
/// 重なると、2本の引き出し線が1本の線で繋がったように見えてしまうので、行はそこを避けて積む。
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceBlockLayout {
    /// 外形の左上X (ローカル座標)。
    pub x: f64,
    /// 外形の左上Y (ローカル座標)。
    pub y: f64,
    pub width: f64,
    pub row_h: f64,
    /// ヘッダ行数 (参照記号のみなら1、型番もあれば2)。
    pub header_rows: usize,
    pub contact_rows: usize,
    /// 接点の列幅 (端子対の列, 所在の列)。[`contact_map_layout`] と同じ求め方。
    pub col_w: [f64; 2],
}

impl DeviceBlockLayout {
    pub fn height(&self) -> f64 {
        self.row_h * (self.header_rows + self.contact_rows) as f64
    }
    /// i行目 (0始まり) のヘッダ行のセル (外形の全幅)。ローカル座標の (左上, 右下)。
    /// 文字は、このセルを用紙座標へ直した矩形の中に水平に書く。
    pub fn header_cell(&self, i: usize) -> (Point, Point) {
        let top = self.y + self.row_h * i as f64;
        (
            Point::new(self.x, top),
            Point::new(self.x + self.width, top + self.row_h),
        )
    }
    /// i行目の接点行の、col列目 (0または1) のセル。ローカル座標の (左上, 右下)。
    pub fn contact_cell(&self, i: usize, col: usize) -> (Point, Point) {
        let top = self.contact_row_top(i);
        (
            Point::new(self.col_x(col), top),
            Point::new(self.col_x(col) + self.col_w[col], top + self.row_h),
        )
    }
    /// ヘッダ行の下、接点の行が始まるY。
    fn contacts_top(&self) -> f64 {
        self.y + self.row_h * self.header_rows as f64
    }
    /// i行目 (0始まり) の接点行の上端Y。
    pub fn contact_row_top(&self, i: usize) -> f64 {
        self.contacts_top() + self.row_h * i as f64
    }
    /// 接点の表 (2列) の左端X。外形の幅が接点の表より広いときは中央に寄せる。
    fn contacts_x(&self) -> f64 {
        self.x + (self.width - (self.col_w[0] + self.col_w[1])) / 2.0
    }
    /// col列目 (0または1) の左端X。
    pub fn col_x(&self, col: usize) -> f64 {
        self.contacts_x() + if col == 0 { 0.0 } else { self.col_w[0] }
    }
    /// 回していないコイルのA1/A2ピン (ヘッダ1行目の中央高さ、外形の左右端から
    /// [`DEVICE_BLOCK_MIN_WIDTH`]/2の位置) から外形の左右端までの引き出し線の半分の長さ。
    /// 外形が最小幅ちょうどなら0 (ピンが外形の縁に直接つながる)。
    pub fn lead_half_len(&self) -> f64 {
        ((self.width - DEVICE_BLOCK_MIN_WIDTH) / 2.0).max(0.0)
    }
}

/// 実機ブロック表示の外形の配置をシンボルのローカル座標で求める (原点=配置基準点)。
/// 回していないコイルのピンは外形の左右端の、ちょうどピンの高さ(ローカルY=0)を通る位置に
/// つながるので、外形をどれだけ広げてもコイルの配線は繋がったまま
/// (幅が[`DEVICE_BLOCK_MIN_WIDTH`]を超えた分だけ引き出し線が伸びる)。
pub fn device_block_layout(block: &DeviceBlock) -> DeviceBlockLayout {
    let (center_x, at_y) = (0.0, 0.0);
    let header_rows = if block.part_no.is_empty() { 1 } else { 2 };
    let contact_layout = contact_map_layout(&block.contacts, center_x, 0.0);
    let header_w = text_width(&block.reference).max(if header_rows == 2 {
        text_width(&block.part_no)
    } else {
        0.0
    }) + 2.0 * CONTACT_MAP_PAD;
    // 接点が無ければcontact_map_layoutの列幅(最小幅ぶん)は外形の広さに数えない
    let contacts_w = if block.contacts.is_empty() { 0.0 } else { contact_layout.width() };
    let width = contacts_w.max(header_w).max(DEVICE_BLOCK_MIN_WIDTH);
    let row_h = CONTACT_MAP_ROW_H;
    DeviceBlockLayout {
        x: center_x - width / 2.0,
        // ピンの高さはヘッダ**1行目**の中央。ヘッダ行の仕切り線がピンの高さに重ならない
        y: at_y - row_h / 2.0,
        width,
        row_h,
        header_rows,
        contact_rows: block.contacts.len(),
        col_w: contact_layout.col_w,
    }
}

/// 実機ブロックの外形を用紙座標で見た囲み矩形 (最小点, 最大点)。表は常に正立なので、
/// 配置の回転・ミラーでは向きも大きさも変わらない (配置基準点へ平行移動するだけ)。
/// 描画・クリックでコイルを掴む当たり判定・外形の下に置く機能ラベル (G4) はこれを引く。
pub fn device_block_bounds(inst: &SymbolInstance, block: &DeviceBlock) -> (Point, Point) {
    let layout = device_block_layout(block);
    (
        Point::new(inst.at.x + layout.x, inst.at.y + layout.y),
        Point::new(
            inst.at.x + layout.x + layout.width,
            inst.at.y + layout.y + layout.height(),
        ),
    )
}

/// ピンのローカル座標から引き出し線の軸 (**ローカル座標**の外向き単位ベクトル) を求める。
/// シンボルのピンは配置基準点から外へ伸びているので、大きいほうの成分がピンの向き。
/// 用紙座標の軸は、これを配置変換した向き (回転0/180なら水平、90/270なら垂直)。
pub fn device_block_lead_axis(pin_at: Point) -> Point {
    if pin_at.x.abs() >= pin_at.y.abs() {
        Point::new(pin_at.x.signum(), 0.0)
    } else {
        Point::new(0.0, pin_at.y.signum())
    }
}

/// ピン (用紙座標、回転・ミラー済み) から実機ブロックの外形へ引く引き出し線の終点。
///
/// 引き出し線は**ピン自身の軸**に沿って出る (`axis` = 配置変換後の外向き単位ベクトル、
/// [`device_block_lead_axis`])。そこに繋がる配線と同じ向きなので、回転0/180なら水平に
/// 左右の辺へ、90/270なら垂直に上下の辺へ届く。外形が内容で広がってピンが中に入ったときは、
/// 同じ軸を中心と反対向きに伸ばして縁で止める (接点の行の文字を横切らない)。
/// ピンがちょうど縁の上にある・軸の延長が外形に当たらないときは`None` (引き出し線を描かない)。
pub fn device_block_lead_end(pin: Point, axis: Point, bounds: (Point, Point)) -> Option<Point> {
    let (min, max) = bounds;
    let inside = pin.x > min.x && pin.x < max.x && pin.y > min.y && pin.y < max.y;
    let end = if axis.x.abs() > axis.y.abs() {
        // 水平の軸: ピンの高さが外形の高さの範囲に無ければ、軸を伸ばしても外形に当たらない
        if pin.y < min.y - 1e-9 || pin.y > max.y + 1e-9 {
            return None;
        }
        let x = if inside {
            if axis.x > 0.0 { max.x } else { min.x }
        } else {
            pin.x.clamp(min.x, max.x)
        };
        Point::new(x, pin.y)
    } else {
        if pin.x < min.x - 1e-9 || pin.x > max.x + 1e-9 {
            return None;
        }
        let y = if inside {
            if axis.y > 0.0 { max.y } else { min.y }
        } else {
            pin.y.clamp(min.y, max.y)
        };
        Point::new(pin.x, y)
    };
    ((end.x - pin.x).abs() > 1e-9 || (end.y - pin.y).abs() > 1e-9).then_some(end)
}

/// 実機ブロックに描く文字1つ分。表は常に正立で、文字はセルの中に水平に書く。
/// 文字高さはどれも [`CONTACT_MAP_FONT`]。
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceBlockText {
    pub text: String,
    /// ベースラインの基準点 (用紙座標)。
    pub at: Point,
    /// 寄せ方。ヘッダ (参照記号・型番) は中央、接点の行は左。
    pub align: NoteAlign,
    /// この文字を収めるセル (左上, 右下)。
    pub cell: (Point, Point),
}

/// 実機ブロックに描く文字の一覧 (用紙座標)。上からヘッダ行 → 接点の行の順。
/// SVG出力・キャンバス描画はこれをそのまま描く (どちらも同じ位置になる)。
pub fn device_block_texts(inst: &SymbolInstance, block: &DeviceBlock) -> Vec<DeviceBlockText> {
    let layout = device_block_layout(block);
    let cell_at = |c: (Point, Point)| {
        (
            Point::new(inst.at.x + c.0.x, inst.at.y + c.0.y),
            Point::new(inst.at.x + c.1.x, inst.at.y + c.1.y),
        )
    };
    let baseline = |cell: (Point, Point)| (cell.0.y + cell.1.y) / 2.0 + CONTACT_MAP_BASELINE_SHIFT;
    let mut out = Vec::new();
    for (i, text) in [(0usize, &block.reference), (1, &block.part_no)] {
        if i >= layout.header_rows {
            continue;
        }
        let cell = cell_at(layout.header_cell(i));
        out.push(DeviceBlockText {
            text: text.clone(),
            at: Point::new((cell.0.x + cell.1.x) / 2.0, baseline(cell)),
            align: NoteAlign::Center,
            cell,
        });
    }
    for (i, row) in block.contacts.iter().enumerate() {
        for (col, text) in [(0, &row.terminals), (1, &row.address)] {
            let cell = cell_at(layout.contact_cell(i, col));
            out.push(DeviceBlockText {
                text: text.clone(),
                at: Point::new(cell.0.x + CONTACT_MAP_PAD, baseline(cell)),
                align: NoteAlign::Left,
                cell,
            });
        }
    }
    out
}

/// 接点の脇に置くコイル所在テキストの基準点 (テキストは左揃え、G22)。
///
/// 配線注記 ([`crate::wire_note`]) と同じ「反対側へ逃がす」考え方で、まず右
/// (外形右端+[`COIL_LOCATION_GAP`]、シンボル中央高さ) を試し、そこが配線注記と重なるなら
/// 上→下の順に外形の外を試す。切換接点は右2本のピンがシンボル中央高さを1グリッドピッチで
/// 挟むので、横引きの配線注記がちょうどこの右側の位置に来て重なることがある。
///
/// `text`は実際に描くコイル所在の文字列 (重なり判定の幅の見積りに使う)。`notes`はそのシートの
/// 配線注記 ([`crate::wire_note::wire_notes`])、`note_font`はそのシートで配線注記を描く文字高
/// ([`crate::wire_note::wire_note_font`]、注記文字高 (G20) で伸縮する)。逃がし先はその
/// インスタンス自身の参照記号・型番のラベル ([`instance_label_rects`]、外形の上に併記される)
/// も避ける。全ての候補が重なっていれば、最後の逃がし先 (下) を返す。
///
/// コイル所在そのものの文字高は [`CONTACT_MAP_FONT`] 固定 (リレーのクロスリファレンスは
/// 注記文字高で伸縮しない)。伸縮するのは相手の配線注記の側だけ。
pub fn coil_location_at(
    inst: &SymbolInstance,
    def: &SymbolDef,
    text: &str,
    notes: &[WireNote],
    note_font: f64,
) -> Point {
    let (min, max) = crate::svg::symbol_bounds(inst, def);
    let font = CONTACT_MAP_FONT;
    let width = text_width(text);
    let candidates = [
        Point::new(max.x + COIL_LOCATION_GAP, inst.at.y),
        Point::new(min.x, min.y - COIL_LOCATION_VGAP),
        Point::new(min.x, max.y + COIL_LOCATION_VGAP + font),
    ];
    let mut occupied: Vec<crate::tidy::Rect> = notes
        .iter()
        .map(|note| {
            text_rect(
                note.at,
                text_width_at(&note.text, note_font),
                note_font,
                note.align,
            )
        })
        .collect();
    occupied.extend(instance_label_rects(inst, def));
    for at in candidates {
        let probe = text_rect(at, width, font, NoteAlign::Left);
        if !occupied.iter().any(|r| probe.overlaps(r)) {
            return at;
        }
    }
    candidates[candidates.len() - 1]
}

/// インスタンス自身の参照記号・型番のラベルが紙面で占める矩形 (空の欄は含まない)。
/// 位置・文字高はシンボルを描くときと同じ (外形の上に中央揃えで併記する)。
fn instance_label_rects(inst: &SymbolInstance, def: &SymbolDef) -> Vec<crate::tidy::Rect> {
    let top = crate::svg::symbol_top_y(inst, def);
    let font = crate::svg::LABEL_FONT;
    [
        (inst.reference.as_str(), crate::svg::REF_LABEL_DY),
        (inst.value.as_str(), crate::svg::VALUE_LABEL_DY),
    ]
    .into_iter()
    .filter(|(text, _)| !text.is_empty())
    .map(|(text, dy)| {
        text_rect(
            Point::new(inst.at.x, top - dy),
            text_width_at(text, font),
            font,
            NoteAlign::Center,
        )
    })
    .collect()
}

/// 文字列を[`CONTACT_MAP_FONT`]で描いたときの見積り幅 (mm)。
fn text_width(text: &str) -> f64 {
    text_width_at(text, CONTACT_MAP_FONT)
}

/// 文字列を文字高`font`で描いたときの見積り幅 (mm)。
fn text_width_at(text: &str, font: f64) -> f64 {
    font * CHAR_WIDTH_RATIO * text.chars().count() as f64
}

/// ベースライン位置・幅・寄せ方から、[`CONTACT_MAP_FONT`]の高さで見積もる文字の外接矩形。
/// 配線注記の文字高さ ([`crate::wire_note::WIRE_NOTE_FONT`]) は既定でこれと同じ2.0mmなので、
/// 実機ブロック等と同じ近似で衝突判定に使う (シートの注記文字高 (G20) は考慮しない)。
fn text_rect(at: Point, width: f64, height: f64, align: NoteAlign) -> crate::tidy::Rect {
    let x0 = match align {
        NoteAlign::Left => at.x,
        NoteAlign::Center => at.x - width / 2.0,
    };
    crate::tidy::Rect::from_corners(Point::new(x0, at.y - height), Point::new(x0 + width, at.y))
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
        // c接点1極はa接点にもb接点にもなるので、専用極が足りない分はc接点の空き極から充当する
        let shortage = allocate_contacts(config, device.used_contacts()).shortage;
        for (role, label, available, short) in [
            (RelayRole::ContactNo, "a接点", config.no, shortage.no),
            (RelayRole::ContactNc, "b接点", config.nc, shortage.nc),
            (RelayRole::ContactCo, "c接点", config.co, shortage.co),
        ] {
            if short == 0 {
                continue;
            }
            let used: Vec<&RelayFunction> =
                device.contacts.iter().filter(|c| c.role == role).collect();
            over.push(format!("{label} {}個 (実装 {available}個)", used.len()));
            ids.extend(used.iter().map(|c| c.entity_id));
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
        assert_eq!(parse_contact_config("2NO+2NC"), Some(ContactConfig { no: 2, nc: 2, co: 0 }));
        assert_eq!(parse_contact_config("4NO"), Some(ContactConfig { no: 4, nc: 0, co: 0 }));
        assert_eq!(parse_contact_config(" 1no , 3nc "), Some(ContactConfig { no: 1, nc: 3, co: 0 }));
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

    /// A changeover contact counts as a contact of its relay, exactly like a make or a break contact.
    /// c接点(切換接点)も、a接点・b接点と同じようにそのリレーの接点として数えられる。
    #[test]
    fn a_changeover_contact_belongs_to_its_relay() {
        assert_eq!(relay_role(CONTACT_CO_SYMBOL_ID), Some(RelayRole::ContactCo));
        let project = project_with(
            1,
            vec![
                (0, symbol(COIL_SYMBOL_ID, "K1", 20.0, 20.0)),
                (0, symbol(CONTACT_CO_SYMBOL_ID, "K1", 60.0, 20.0)),
            ],
        );
        let device = &relay_devices(&project)[0];
        assert_eq!(device.contacts.len(), 1);
        assert_eq!(device.contacts[0].role, RelayRole::ContactCo);
    }

    /// A relay drawn with changeover contacts is no longer reported as a coil that drives nothing.
    /// c接点で組んだリレーは、「接点を動かしていないコイル」として誤って指摘されなくなる。
    #[test]
    fn a_relay_with_changeover_contacts_is_not_reported_as_contactless() {
        let project = project_with(
            1,
            vec![
                (0, symbol(COIL_SYMBOL_ID, "K1", 20.0, 20.0)),
                (0, symbol(CONTACT_CO_SYMBOL_ID, "K1", 60.0, 20.0)),
                (0, symbol(CONTACT_CO_SYMBOL_ID, "K1", 100.0, 20.0)),
            ],
        );
        let diags = relay_diagnostics(&project);
        assert!(diags.is_empty(), "{diags:?}");
    }

    /// The terminals of a changeover contact are written as three numbers - common, break, make - like "11-12-14".
    /// c接点の端子は「11-12-14」のように、共通・b側・a側の3つを並べて書かれる。
    #[test]
    fn changeover_terminals_are_written_common_break_make() {
        assert_eq!(terminal_pair(1, RelayRole::ContactCo), "11-12-14");
        assert_eq!(terminal_pair(2, RelayRole::ContactCo), "21-22-24");
        assert_eq!(terminal_pair(3, RelayRole::ContactCo), "31-32-34");
    }

    /// The contact map under the coil lists changeover contacts with their three terminals and their address.
    /// コイル下の接点マップには、c接点も3つの端子と所在つきで並ぶ。
    #[test]
    fn contact_map_lists_changeover_contacts() {
        let project = project_with(
            1,
            vec![
                (0, symbol(COIL_SYMBOL_ID, "K1", 20.0, 20.0)),
                (0, symbol(CONTACT_CO_SYMBOL_ID, "K1", 250.0, 70.0)),
            ],
        );
        let rows = relay_devices(&project)[0].contact_map();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].terminals, "11-12-14");
        assert_eq!(rows[0].address, "/1.B3");
        assert_eq!(rows[0].role, RelayRole::ContactCo);
    }

    /// A contact configuration like "2CO" is read as two changeover poles.
    /// 「2CO」のような接点構成は、c接点2極として読み取られる。
    #[test]
    fn contact_config_parses_changeover_poles() {
        assert_eq!(
            parse_contact_config("2CO"),
            Some(ContactConfig { no: 0, nc: 0, co: 2 })
        );
        assert_eq!(
            parse_contact_config("1co+2NO"),
            Some(ContactConfig { no: 2, nc: 0, co: 1 })
        );
    }

    /// The unused poles of a "2CO" relay with one contact drawn are listed as the remaining changeover contact.
    /// 「2CO」のリレーに接点を1つだけ描いたときは、残り1極がc接点の未使用行として並ぶ。
    #[test]
    fn contact_map_shows_the_remaining_changeover_poles() {
        let project = project_with(
            1,
            vec![
                (
                    0,
                    with_attr(symbol(COIL_SYMBOL_ID, "K1", 20.0, 20.0), CONTACT_CONFIG_ATTR, "2CO"),
                ),
                (0, symbol(CONTACT_CO_SYMBOL_ID, "K1", 250.0, 70.0)),
            ],
        );
        let rows = relay_devices(&project)[0].contact_map();
        assert_eq!(
            rows.iter()
                .map(|r| (r.terminals.as_str(), r.address.as_str()))
                .collect::<Vec<_>>(),
            vec![("11-12-14", "/1.B3"), ("21-22-24", CONTACT_UNUSED)]
        );
    }

    /// A changeover pole may be drawn as a plain make or break contact, so a "2CO" relay accepts two make contacts.
    /// c接点の極はa接点・b接点としても描けるので、「2CO」のリレーはa接点2個の使い方も受け付ける。
    #[test]
    fn a_changeover_pole_can_be_used_as_a_make_or_break_contact() {
        let project = project_with(
            1,
            vec![
                (
                    0,
                    with_attr(symbol(COIL_SYMBOL_ID, "K1", 20.0, 20.0), CONTACT_CONFIG_ATTR, "2CO"),
                ),
                (0, symbol(CONTACT_NO_SYMBOL_ID, "K1", 60.0, 20.0)),
                (0, symbol(CONTACT_NC_SYMBOL_ID, "K1", 100.0, 20.0)),
            ],
        );
        let diags = relay_diagnostics(&project);
        assert!(diags.is_empty(), "2極を使い切っているので指摘なし: {diags:?}");
    }

    /// Using more poles than the part has is still an error, and the message names the changeover contact.
    /// 部品の極数を超えて使えばやはりエラーになり、メッセージにc接点と示される。
    #[test]
    fn using_more_changeover_contacts_than_the_part_has_is_an_error() {
        let project = project_with(
            1,
            vec![
                (
                    0,
                    with_attr(symbol(COIL_SYMBOL_ID, "K1", 20.0, 20.0), CONTACT_CONFIG_ATTR, "1CO"),
                ),
                (0, symbol(CONTACT_CO_SYMBOL_ID, "K1", 60.0, 20.0)),
                (0, symbol(CONTACT_CO_SYMBOL_ID, "K1", 100.0, 20.0)),
            ],
        );
        let d = relay_diagnostics(&project)
            .into_iter()
            .find(|d| d.code == "erc.contact_overflow")
            .expect("接点数超過");
        assert_eq!(d.severity, Severity::Error);
        assert!(d.message.contains("c接点 2個 (実装 1個)"), "{}", d.message);
    }

    /// A make contact cannot borrow a changeover pole that a changeover contact already uses.
    /// c接点の図記号が使っている極は、a接点が借りることはできない。
    #[test]
    fn a_make_contact_cannot_borrow_a_pole_already_in_use() {
        let project = project_with(
            1,
            vec![
                (
                    0,
                    with_attr(symbol(COIL_SYMBOL_ID, "K1", 20.0, 20.0), CONTACT_CONFIG_ATTR, "1CO"),
                ),
                (0, symbol(CONTACT_CO_SYMBOL_ID, "K1", 60.0, 20.0)),
                (0, symbol(CONTACT_NO_SYMBOL_ID, "K1", 100.0, 20.0)),
            ],
        );
        let d = relay_diagnostics(&project)
            .into_iter()
            .find(|d| d.code == "erc.contact_overflow")
            .expect("接点数超過");
        assert!(d.message.contains("a接点 1個 (実装 0個)"), "{}", d.message);
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
        let at = coil_location_at(&contact, &contact_def, "(/1.C2)", &[], crate::wire_note::WIRE_NOTE_FONT);
        assert!(at.x > 100.0 + 7.5, "外形(右端+7.5mm)より右: {at:?}");
        assert!((at.y - 50.0).abs() < 1e-9, "シンボルの高さ中央");
    }

    /// A changeover contact's coil location moves off the wire note above it, reproducing the collision described in G22.
    /// 切換接点のコイル所在は、上の配線に付く注記と重ならないよう逃げる (G22の衝突の再現)。
    #[test]
    fn coil_location_moves_off_a_colliding_wire_note() {
        // 切換接点(c接点)は右2本のピンがシンボル中央高さ(y=50)を1グリッドピッチで挟む:
        // 「14」(a側)がy=47.5、「12」(b側)がy=52.5。y=47.5のピンから横に引いた配線の注記は
        // 線番と反対側 (下) の y=47.5+2.5=50.0 に来るので、右側の既定位置とぴったり重なる。
        let contact = SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: CONTACT_CO_SYMBOL_ID.into(),
            at: Point::new(100.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "K1".into(),
            value: String::new(),
            attrs: Default::default(),
        };
        let def = crate::symbol::resolve_symbol(CONTACT_CO_SYMBOL_ID).unwrap();
        let mut sheet = crate::model::Sheet::new(
            "t",
            crate::model::PaperSize::A3,
            crate::model::Orientation::Landscape,
        );
        let wire = Entity::Wire(crate::model::Wire {
            id: Uuid::new_v4(),
            points: vec![Point::new(107.5, 47.5), Point::new(130.0, 47.5)],
            color: "red".into(),
            sq: 0.75,
            length_m: None,
            part_no: None,
            net: None,
            length_source: crate::model::LengthSource::Manual,
        });
        sheet.entities.insert(wire.id(), wire);
        let notes = crate::wire_note::wire_notes(&sheet);
        assert_eq!(notes.len(), 1, "注記が1つ生成されている前提: {notes:?}");
        assert!(
            (notes[0].at.y - 50.0).abs() < 1e-9,
            "既定のコイル所在と同じ高さに来る注記: {notes:?}"
        );

        let without_notes = coil_location_at(&contact, &def, "(/1.C2)", &[], crate::wire_note::WIRE_NOTE_FONT);
        assert!((without_notes.y - 50.0).abs() < 1e-9, "配線注記が無ければ右・中央高さのまま");

        let at = coil_location_at(&contact, &def, "(/1.C2)", &notes, crate::wire_note::WIRE_NOTE_FONT);
        assert!(
            (at.y - 50.0).abs() > 1e-9,
            "右側は注記と重なるので、中央高さのままにはしない: {at:?}"
        );
        assert!(at.y < 50.0, "先に上へ逃げる: {at:?}");
    }

    /// When even the space above is occupied, the coil location falls back to below the outline.
    /// 上も塞がっているときは、コイル所在は外形の下へ逃げる。
    #[test]
    fn coil_location_falls_back_below_when_above_is_also_occupied() {
        let contact = SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: CONTACT_CO_SYMBOL_ID.into(),
            at: Point::new(100.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "K1".into(),
            value: String::new(),
            attrs: Default::default(),
        };
        let def = crate::symbol::resolve_symbol(CONTACT_CO_SYMBOL_ID).unwrap();
        let (min, max) = crate::svg::symbol_bounds(&contact, &def);
        let text = "(/1.C2)";
        let right = Point::new(max.x + COIL_LOCATION_GAP, contact.at.y);
        let above = Point::new(min.x, min.y - COIL_LOCATION_VGAP);
        let notes = vec![
            WireNote { wire_id: Uuid::new_v4(), text: text.into(), at: right, align: NoteAlign::Left },
            WireNote { wire_id: Uuid::new_v4(), text: text.into(), at: above, align: NoteAlign::Left },
        ];
        let at = coil_location_at(&contact, &def, text, &notes, crate::wire_note::WIRE_NOTE_FONT);
        let below = Point::new(min.x, max.y + COIL_LOCATION_VGAP + CONTACT_MAP_FONT);
        assert!((at.x - below.x).abs() < 1e-9 && (at.y - below.y).abs() < 1e-9, "{at:?}");
    }

    /// When the contact carries a part number of its own, the coil location does not escape onto that text: it goes on past it, to the other side of the outline.
    /// 接点自身に型番が書かれているときは、コイル所在はその文字の上へは逃げず、さらに先の外形の反対側へ回る (G22)。
    #[test]
    fn coil_location_does_not_escape_onto_the_instances_own_value_label() {
        let contact = SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: CONTACT_CO_SYMBOL_ID.into(),
            at: Point::new(100.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "K1".into(),
            value: "SAMPLE0001".into(),
            attrs: Default::default(),
        };
        let def = crate::symbol::resolve_symbol(CONTACT_CO_SYMBOL_ID).unwrap();
        let (_, max) = crate::svg::symbol_bounds(&contact, &def);
        let text = "(/1.C2)";
        // 右の既定位置だけをふさぐ配線注記 (上下はまだ空いている)
        let notes = vec![WireNote {
            wire_id: Uuid::new_v4(),
            text: text.into(),
            at: Point::new(max.x + COIL_LOCATION_GAP, contact.at.y),
            align: NoteAlign::Left,
        }];
        let blank = SymbolInstance { value: String::new(), ..contact.clone() };
        assert!(
            coil_location_at(&blank, &def, text, &notes, crate::wire_note::WIRE_NOTE_FONT).y < contact.at.y,
            "型番が無ければ上へ逃げる"
        );
        let at = coil_location_at(&contact, &def, text, &notes, crate::wire_note::WIRE_NOTE_FONT);
        assert!(at.y > max.y, "型番の文字が上にあるので、上ではなく下へ逃げる: {at:?}");
    }

    /// The escape measures the wire note at the height that sheet actually draws it, so a sheet with taller annotation text still moves the coil location out of the way.
    /// 逃がしの判定はそのシートが実際に描く文字高で配線注記を測るので、注記文字を大きくしたシートでもコイル所在はきちんと逃げる (G20とG22)。
    #[test]
    fn coil_location_measures_the_wire_note_at_the_sheets_own_text_height() {
        let contact = SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: CONTACT_CO_SYMBOL_ID.into(),
            at: Point::new(100.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: String::new(),
            value: String::new(),
            attrs: Default::default(),
        };
        let def = crate::symbol::resolve_symbol(CONTACT_CO_SYMBOL_ID).unwrap();
        let (_, max) = crate::svg::symbol_bounds(&contact, &def);
        let text = "(/1.C2)";
        // 既定(2.0mm)の高さで測ると右の既定位置には届かないが、3.5mmで描かれると重なる注記
        let notes = vec![WireNote {
            wire_id: Uuid::new_v4(),
            text: "0.75sq 赤 0.4m".into(),
            at: Point::new(max.x + COIL_LOCATION_GAP, contact.at.y + 2.6),
            align: NoteAlign::Left,
        }];
        let small = coil_location_at(&contact, &def, text, &notes, crate::wire_note::WIRE_NOTE_FONT);
        assert!((small.y - contact.at.y).abs() < 1e-9, "2.0mmなら重ならず右のまま: {small:?}");
        let large = coil_location_at(&contact, &def, text, &notes, 3.5);
        assert!((large.y - contact.at.y).abs() > 1e-9, "3.5mmで描かれる注記からは逃げる: {large:?}");
    }

    // ---- G7: 実機ブロック表示 ----

    /// A coil is only shown as a physical device block when its "device_block" attribute is exactly "true"; any other value, including empty, keeps the distributed placement.
    /// コイルは`attrs["device_block"]`がちょうど`"true"`のときだけ実機ブロック表示になり、それ以外(空文字含む)は従来どおりの分散配置になる。
    #[test]
    fn device_block_attribute_selects_the_display_mode() {
        let plain = symbol(COIL_SYMBOL_ID, "K1", 20.0, 20.0);
        let Entity::Symbol(plain) = &plain else { unreachable!() };
        assert!(!is_device_block(plain));

        let on = with_attr(symbol(COIL_SYMBOL_ID, "K1", 20.0, 20.0), DEVICE_BLOCK_ATTR, "true");
        let Entity::Symbol(on) = &on else { unreachable!() };
        assert!(is_device_block(on));

        let garbled = with_attr(symbol(COIL_SYMBOL_ID, "K1", 20.0, 20.0), DEVICE_BLOCK_ATTR, "TRUE");
        let Entity::Symbol(garbled) = &garbled else { unreachable!() };
        assert!(!is_device_block(garbled), "大文字小文字違いは無効のまま扱う");
    }

    /// The device block's contact image lists each contact with its terminal pair and drawing address, in the same order as the contact map.
    /// 実機ブロックの接点イメージには、接点マップと同じ並びで端子対と所在が並ぶ。
    #[test]
    fn device_block_lists_the_same_contacts_as_the_contact_map() {
        let project = project_with(
            1,
            vec![
                (0, symbol(COIL_SYMBOL_ID, "K1", 20.0, 20.0)),
                (0, symbol(CONTACT_NO_SYMBOL_ID, "K1", 60.0, 20.0)),
                (0, symbol(CONTACT_NC_SYMBOL_ID, "K1", 100.0, 20.0)),
            ],
        );
        let device = &relay_devices(&project)[0];
        let block = device.device_block("SAMPLE0001");
        assert_eq!(block.reference, "K1");
        assert_eq!(block.part_no, "SAMPLE0001");
        assert_eq!(block.contacts, device.contact_map());
    }

    /// A blank part number produces a device block with no part number line, rather than an empty one.
    /// 型番が空のときは、空の型番行ではなく型番行そのものが無い実機ブロックになる。
    #[test]
    fn device_block_has_no_part_number_line_when_blank() {
        let project = project_with(1, vec![(0, symbol(COIL_SYMBOL_ID, "K1", 20.0, 20.0))]);
        let device = &relay_devices(&project)[0];
        assert_eq!(device.device_block("  ").part_no, "");
    }

    /// Only the coils whose instance has "device_block" set are collected into the sheet's device blocks; a coil left in the default distributed placement is not.
    /// シートの実機ブロック一覧に入るのは`device_block`が付いたコイルだけで、従来どおりの分散配置のコイルは入らない。
    #[test]
    fn sheet_device_blocks_only_includes_flagged_coils() {
        let project = project_with(
            1,
            vec![
                (
                    0,
                    with_attr(symbol(COIL_SYMBOL_ID, "K1", 20.0, 20.0), DEVICE_BLOCK_ATTR, "true"),
                ),
                (0, symbol(COIL_SYMBOL_ID, "K2", 20.0, 60.0)),
                (0, symbol(CONTACT_NO_SYMBOL_ID, "K1", 60.0, 20.0)),
            ],
        );
        let sheet_id = project.sheets[0].id;
        let k1_id = project.sheets[0]
            .entities
            .values()
            .find_map(|e| match e {
                Entity::Symbol(s) if s.reference == "K1" && s.symbol_id == COIL_SYMBOL_ID => Some(s.id),
                _ => None,
            })
            .unwrap();
        let blocks = sheet_device_blocks(&project, sheet_id);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[&k1_id].contacts.len(), 1);
    }

    /// A coil shown as a device block no longer gets the separate hanging contact map, because its contacts are already listed inside the block.
    /// 実機ブロックで表示するコイルには、コイルの下にぶら下げる別表としての接点マップはもう付かない(接点はブロックの中に既に並んでいるので)。
    #[test]
    fn device_block_coils_are_excluded_from_the_separate_contact_map() {
        let project = project_with(
            1,
            vec![
                (
                    0,
                    with_attr(symbol(COIL_SYMBOL_ID, "K1", 20.0, 20.0), DEVICE_BLOCK_ATTR, "true"),
                ),
                (0, symbol(CONTACT_NO_SYMBOL_ID, "K1", 60.0, 20.0)),
            ],
        );
        let sheet_id = project.sheets[0].id;
        assert!(sheet_contact_maps(&project, sheet_id).is_empty());
        assert_eq!(sheet_device_blocks(&project, sheet_id).len(), 1);
    }

    /// The device block's outline is at least as wide as the coil's pin span, so its A1/A2 pins land exactly on the outline's left and right edge and no lead line is needed.
    /// 実機ブロックの外形はコイルのピン間隔を下回らないので、幅が最小のときはA1/A2ピンがそのまま外形の左右端になり、引き出し線は要らない。
    #[test]
    fn device_block_layout_is_no_narrower_than_the_pin_span() {
        let block = DeviceBlock { reference: "K1".into(), part_no: "".into(), contacts: vec![] };
        let layout = device_block_layout(&block);
        assert!((layout.width - DEVICE_BLOCK_MIN_WIDTH).abs() < 1e-9, "{layout:?}");
        assert_eq!(layout.header_rows, 1, "型番が無いのでヘッダ1行");
        assert!((layout.lead_half_len()).abs() < 1e-9, "最小幅ちょうどなら引き出し線は無い");
        assert!((layout.x + DEVICE_BLOCK_MIN_WIDTH / 2.0).abs() < 1e-9, "中心はピンの中点");
    }

    /// The device block's minimum width is exactly the relay coil symbol's own pin span, so the narrowest block reaches both of the coil's pins.
    /// 実機ブロックの最小幅はリレーコイルのシンボル定義のピン間隔そのものなので、いちばん細いブロックでもコイルの両ピンに届く。
    #[test]
    fn device_block_min_width_is_the_coil_symbols_pin_span() {
        let def = crate::symbol::resolve_symbol(COIL_SYMBOL_ID).unwrap();
        let xs: Vec<f64> = def.pins.iter().map(|p| p.at.x).collect();
        let (min, max) = (
            xs.iter().cloned().fold(f64::INFINITY, f64::min),
            xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        );
        assert!(
            (max - min - DEVICE_BLOCK_MIN_WIDTH).abs() < 1e-9,
            "ピン間隔 {} と最小幅 {DEVICE_BLOCK_MIN_WIDTH} が食い違っている",
            max - min
        );
        assert!(
            def.pins.iter().all(|p| p.at.y.abs() < 1e-9),
            "A1/A2は配置基準点と同じ高さにある: {:?}",
            def.pins
        );
    }

    /// A part number adds a second header row, and pins still reach the outline through a lead when the content widens the box past the pin span.
    /// 型番があるとヘッダは2行になり、内容で外形がピン間隔より広がっても引き出し線でピンは外形まで届く。
    #[test]
    fn device_block_layout_grows_for_the_part_number_and_keeps_the_pins_reachable() {
        let block = DeviceBlock {
            reference: "K1".into(),
            part_no: "SAMPLE0001-VERY-LONG-PART-NO".into(),
            contacts: vec![ContactMapRow {
                terminals: "13-14".into(),
                address: "/1.B3".into(),
                role: RelayRole::ContactNo,
                entity_id: None,
            }],
        };
        let layout = device_block_layout(&block);
        assert_eq!(layout.header_rows, 2, "型番の行が増える");
        assert!(layout.width > DEVICE_BLOCK_MIN_WIDTH, "長い型番で外形が広がる: {layout:?}");
        assert!(layout.lead_half_len() > 0.0, "外形が広がった分だけ引き出し線が伸びる");
        // ピンはシンボル定義のA1/A2のまま: 外形の左右端から引き出し線の分だけ内側にある
        let def = crate::symbol::resolve_symbol(COIL_SYMBOL_ID).unwrap();
        let left_pin = def.pins.iter().map(|p| p.at.x).fold(f64::INFINITY, f64::min);
        assert!((layout.x - (left_pin - layout.lead_half_len())).abs() < 1e-9);
        // ヘッダ1行目の中央高さがピンの高さ(ローカルY=0)と一致する: 引き出し線は水平に引ける
        assert!((layout.y + layout.row_h / 2.0).abs() < 1e-9);
    }

    /// The row dividers never sit at the height the coil's leads attach, so the two leads are never joined into one line across the block.
    /// 行の仕切り線はコイルの引き出し線が付く高さには来ないので、2本の引き出し線がブロックを横切る1本の線で繋がって見えることはない。
    #[test]
    fn device_block_rows_keep_clear_of_the_lead_height() {
        let block = DeviceBlock {
            reference: "K1".into(),
            part_no: "P1".into(),
            contacts: vec![ContactMapRow {
                terminals: "13-14".into(),
                address: "/1.B3".into(),
                role: RelayRole::ContactNo,
                entity_id: None,
            }],
        };
        let layout = device_block_layout(&block);
        // 仕切り線はヘッダ行の間 (1..header_rows) と接点の行の上端に引かれる
        let mut dividers: Vec<f64> = (1..layout.header_rows)
            .map(|i| layout.y + layout.row_h * i as f64)
            .collect();
        dividers.extend((0..layout.contact_rows).map(|i| layout.contact_row_top(i)));
        assert!(!dividers.is_empty(), "仕切り線のあるブロックで試す前提");
        for y in dividers {
            assert!(y.abs() > 1e-9, "ピンの高さ(ローカルY=0)に仕切り線がある: {y}");
        }
    }

    /// リレーコイルの実機ブロック表示をテストするための、接点`contacts`本・型番`part_no`のブロック。
    fn demo_block(part_no: &str, contacts: usize) -> DeviceBlock {
        DeviceBlock {
            reference: "K1".into(),
            part_no: part_no.into(),
            contacts: (0..contacts)
                .map(|i| ContactMapRow {
                    terminals: format!("{}3-{}4", i + 1, i + 1),
                    address: format!("/1.B{}", i + 1),
                    role: RelayRole::ContactNo,
                    entity_id: None,
                })
                .collect(),
        }
    }

    /// ローカル座標のピン位置と配置から、引き出し線の軸 (用紙座標の外向き単位ベクトル) を作る。
    fn lead_axis(pin_at: Point, inst: &SymbolInstance) -> Point {
        let local = device_block_lead_axis(pin_at);
        let t = crate::netlist::transform_local(local, inst);
        Point::new(t.x - inst.at.x, t.y - inst.at.y)
    }

    /// 実機ブロックのコイル1個 (用紙座標100,50)。
    fn block_coil(rotation: u16, mirror: bool) -> SymbolInstance {
        SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: COIL_SYMBOL_ID.into(),
            at: Point::new(100.0, 50.0),
            rotation,
            mirror,
            reference: "K1".into(),
            value: String::new(),
            attrs: Default::default(),
        }
    }

    /// However wide the outline grows, an unturned coil's leads stay horizontal and end on the left and right edge at the pin's own height, where its wires come in.
    /// 外形がどれだけ横に広がっても、回していないコイルの引き出し線は水平のまま、配線が来るピンの高さで外形の左右の辺に届く (G7)。
    #[test]
    fn device_block_leads_run_along_the_pin_axis_however_wide_the_block() {
        let def = crate::symbol::resolve_symbol(COIL_SYMBOL_ID).unwrap();
        // 最小幅 / 切換接点の行 (19.6mm) / 実際の型番 (24.8mm) の3通り
        for block in [
            demo_block("", 0),
            DeviceBlock {
                contacts: vec![ContactMapRow {
                    terminals: "11-12-14".into(),
                    address: "/1.B3".into(),
                    role: RelayRole::ContactCo,
                    entity_id: None,
                }],
                ..demo_block("", 0)
            },
            demo_block("[redacted](D)/24DC-2Z6", 0),
        ] {
            let width = device_block_layout(&block).width;
            for mirror in [false, true] {
                let coil = block_coil(0, mirror);
                let bounds = device_block_bounds(&coil, &block);
                for pin in &def.pins {
                    let at = crate::netlist::transform_local(pin.at, &coil);
                    let end = device_block_lead_end(at, lead_axis(pin.at, &coil), bounds);
                    if width <= DEVICE_BLOCK_MIN_WIDTH + 1e-9 {
                        assert!(end.is_none(), "最小幅ではピンが縁の上なので引き出し線は要らない");
                        continue;
                    }
                    let end = end.unwrap_or_else(|| {
                        panic!("幅{width}mm mirror={mirror}: ピン{}の引き出し線が無い", pin.number)
                    });
                    assert!(
                        (end.y - at.y).abs() < 1e-9,
                        "幅{width}mm mirror={mirror}: ピン{}の引き出し線が水平でない {at:?}->{end:?}",
                        pin.number
                    );
                    let on_side = (end.x - bounds.0.x).abs() < 1e-9 || (end.x - bounds.1.x).abs() < 1e-9;
                    assert!(on_side, "幅{width}mm: 左右どちらかの辺で止まる {end:?} / {bounds:?}");
                }
            }
        }
    }

    /// A coil turned 90 or 270 degrees sends its leads straight up and down to the top and bottom edge, and they never run through a line of the contact image, however many contacts the block lists.
    /// 90度・270度に回したコイルの引き出し線は真上・真下へ伸びて外形の上端・下端で止まり、接点が何行あっても接点イメージの文字を横切らない (G7)。
    #[test]
    fn a_turned_device_blocks_leads_run_vertically_without_crossing_the_rows() {
        let def = crate::symbol::resolve_symbol(COIL_SYMBOL_ID).unwrap();
        for contacts in [1, 3] {
            let block = demo_block("P1", contacts);
            for rotation in [90, 270] {
                let coil = block_coil(rotation, false);
                let bounds = device_block_bounds(&coil, &block);
                let boxes: Vec<(String, crate::tidy::Rect)> = device_block_texts(&coil, &block)
                    .into_iter()
                    .map(|t| {
                        (
                            t.text.clone(),
                            text_rect(t.at, text_width(&t.text), CONTACT_MAP_FONT, t.align),
                        )
                    })
                    .collect();
                for pin in &def.pins {
                    let at = crate::netlist::transform_local(pin.at, &coil);
                    let end = device_block_lead_end(at, lead_axis(pin.at, &coil), bounds)
                        .unwrap_or_else(|| panic!("接点{contacts}行 rotation={rotation}: ピン{}の引き出し線が無い", pin.number));
                    assert!(
                        (end.x - at.x).abs() < 1e-9,
                        "接点{contacts}行 rotation={rotation}: 引き出し線が垂直でない {at:?}->{end:?}"
                    );
                    let on_edge = (end.y - bounds.0.y).abs() < 1e-9 || (end.y - bounds.1.y).abs() < 1e-9;
                    assert!(on_edge, "上端・下端のどちらかで止まる {end:?} / {bounds:?}");
                    let (lo, hi) = (at.y.min(end.y), at.y.max(end.y));
                    for (text, r) in &boxes {
                        let crosses = r.min.x < at.x - 1e-9
                            && at.x < r.max.x - 1e-9
                            && r.min.y < hi - 1e-9
                            && lo < r.max.y - 1e-9;
                        assert!(
                            !crosses,
                            "接点{contacts}行 rotation={rotation}: 引き出し線が「{text}」を横切る"
                        );
                    }
                }
            }
        }
    }

    /// The device block stays upright whatever the rotation: the outline keeps its size and position, and only the pins turn, so the leads reach the turned pins along their own axis.
    /// 実機ブロックはどの回転でも正立のまま: 外形の大きさも位置も変わらず、回るのはピンだけなので、引き出し線はピン自身の軸に沿って外形へ届く。
    #[test]
    fn device_block_bounds_stay_upright_when_the_instance_turns() {
        let block = DeviceBlock {
            reference: "K1".into(),
            part_no: "P1".into(),
            contacts: vec![ContactMapRow {
                terminals: "13-14".into(),
                address: "/1.B3".into(),
                role: RelayRole::ContactNo,
                entity_id: None,
            }],
        };
        let layout = device_block_layout(&block);
        let coil = SymbolInstance {
            id: Uuid::new_v4(),
            symbol_id: COIL_SYMBOL_ID.into(),
            at: Point::new(100.0, 50.0),
            rotation: 0,
            mirror: false,
            reference: "K1".into(),
            value: "P1".into(),
            attrs: Default::default(),
        };
        let def = crate::symbol::resolve_symbol(COIL_SYMBOL_ID).unwrap();
        let upright = device_block_bounds(&coil, &block);
        assert!((upright.1.x - upright.0.x - layout.width).abs() < 1e-9);
        assert!((upright.1.y - upright.0.y - layout.height()).abs() < 1e-9);
        for rotation in [90, 180, 270] {
            let turned = SymbolInstance { rotation, ..coil.clone() };
            let bounds = device_block_bounds(&turned, &block);
            assert_eq!(bounds, upright, "rotation={rotation}: 外形は正立のまま動かない");
            // 引き出し線は回った先のピンから外形の縁まで届く (ピンが縁の上なら描かない)
            for pin in &def.pins {
                let (number, at) = (&pin.number, crate::netlist::transform_local(pin.at, &turned));
                if let Some(end) = device_block_lead_end(at, lead_axis(pin.at, &turned), bounds) {
                    let on_edge = (end.x - bounds.0.x).abs() < 1e-9
                        || (end.x - bounds.1.x).abs() < 1e-9
                        || (end.y - bounds.0.y).abs() < 1e-9
                        || (end.y - bounds.1.y).abs() < 1e-9;
                    assert!(on_edge, "rotation={rotation} ピン{number}: 終点が縁にない {end:?}");
                }
            }
        }
        let mirrored = SymbolInstance { mirror: true, ..coil.clone() };
        assert_eq!(device_block_bounds(&mirrored, &block), upright, "反転しても正立のまま");
    }

    /// The device block's contact rows sit directly under the header rows, and the outline's total height covers every row with no gap.
    /// 実機ブロックの接点の行はヘッダ行のすぐ下に続き、外形全体の高さは隙間なく全ての行をちょうど覆う。
    #[test]
    fn device_block_layout_stacks_contact_rows_under_the_header() {
        let block = DeviceBlock {
            reference: "K1".into(),
            part_no: "P1".into(),
            contacts: vec![
                ContactMapRow {
                    terminals: "13-14".into(),
                    address: "/1.B3".into(),
                    role: RelayRole::ContactNo,
                    entity_id: None,
                },
                ContactMapRow {
                    terminals: "21-22".into(),
                    address: CONTACT_UNUSED.into(),
                    role: RelayRole::ContactNc,
                    entity_id: None,
                },
            ],
        };
        let layout = device_block_layout(&block);
        assert_eq!(layout.header_rows, 2);
        assert_eq!(layout.contact_rows, 2);
        assert!((layout.contact_row_top(0) - (layout.y + layout.row_h * 2.0)).abs() < 1e-9);
        assert!((layout.contact_row_top(1) - (layout.y + layout.row_h * 3.0)).abs() < 1e-9);
        assert!((layout.height() - layout.row_h * 4.0).abs() < 1e-9);
    }
}
