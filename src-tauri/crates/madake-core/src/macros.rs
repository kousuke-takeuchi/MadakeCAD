//! 回路マクロ (M4仕様 §2): 図面の一部を**そのまま再利用できる回路の部品**として
//! 保存し、別の場所・別のシートへ何度でも挿入する。
//!
//! 開始テンプレート ([`crate::templates`]) と同じ「Command列のJSON」形式で、次を足したもの:
//!
//! - `base_point`: 挿入時にカーソルへ来る基準点。座標はここからの**相対座標**で保存する
//! - `variants`: 同じ回路の代替形 (例 直入れ/正逆転/スターデルタ)。`commands`が既定
//!   (バリアント`"A"`相当) で、`variants`はその代わりに使う
//!
//! ```json
//! {
//!   "id": "motor_dol",
//!   "name": "Motor starter (DOL)",
//!   "name_ja": "モータ直入れ起動",
//!   "category": "motor",
//!   "base_point": { "x": 0.0, "y": 0.0 },
//!   "commands": [ { "type": "add_entity", "sheet_id": "00000000-…", "entity": { … } } ],
//!   "variants": [ { "key": "B", "name": "Reversing", "name_ja": "正逆転", "commands": [ … ] } ]
//! }
//! ```
//!
//! テンプレートと同じく、コマンド中のUUIDは**プレースホルダ**として扱う (nil = 挿入先
//! シート、それ以外 = 挿入のたびに新しいidへ振り直し。[`crate::templates::substitute`])。
//! 挿入は[`crate::command::Engine::execute_batch`]なので**undo一発**で全体が戻る。
//!
//! 保存先はユーザー領域 (`~/MadakeCAD/macros/*.json`) のみ。同梱マクロは無く、
//! 現場で作った回路がそのままライブラリになる。

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::command::{Command, EditOrigin, Engine, Patch};
use crate::geometry::Point;
use crate::model::{Entity, EntityId, Project, SheetId};
use crate::netlist::rotate_local;
use crate::symbol::resolve_symbol;
use crate::templates::TemplateIssue;
use crate::{CoreError, Result};

/// ユーザーマクロの置き場を差し替える環境変数(既定は`~/MadakeCAD/macros`)。
pub const USER_MACROS_PATH_ENV: &str = "MADAKE_USER_MACROS_PATH";

/// 既定バリアントのキー。`variants`に無い場合、`commands`本体がこのキーとして扱われる。
pub const DEFAULT_VARIANT: &str = "A";

/// 読み込めなかったマクロファイル1件(壊れたJSON・未知のコマンド)。
/// 形はテンプレートの[`TemplateIssue`]と同じ(パス+理由)。
pub type MacroIssue = TemplateIssue;

/// 回路マクロ1件。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Macro {
    /// 安定ID (例: "motor_dol")。挿入時の指定に使う。
    pub id: String,
    /// 英語名。
    pub name: String,
    /// 日本語名。
    #[serde(default)]
    pub name_ja: String,
    /// 英語の説明。
    #[serde(default)]
    pub description: String,
    /// 日本語の説明。
    #[serde(default)]
    pub description_ja: String,
    /// 挿入ダイアログのツリーで束ねるための分類 (任意)。
    #[serde(default)]
    pub category: String,
    /// 基準点。保存時の座標原点であり、挿入時はここがカーソル位置へ来る。
    #[serde(default)]
    pub base_point: Point,
    /// 既定バリアント(`"A"`)のCommand列。座標は`base_point`からの相対。
    pub commands: Vec<serde_json::Value>,
    /// 代替バリアント(`"B"`以降)。
    #[serde(default)]
    pub variants: Vec<MacroVariant>,
}

/// マクロの代替バリアント1件 (EPLANのマクロバリアントA〜Hに相当)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroVariant {
    /// バリアントキー (例: "B")。
    pub key: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub name_ja: String,
    /// このバリアントのCommand列 (既定と同じく`base_point`からの相対座標)。
    pub commands: Vec<serde_json::Value>,
}

/// マクロ保存時に付ける情報 (名前・分類)。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MacroMeta {
    /// 安定ID。空なら名前から自動生成する。
    #[serde(default)]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub name_ja: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub description_ja: String,
    #[serde(default)]
    pub category: String,
}

/// マクロの一覧と、読み込めなかったファイルの理由。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MacroList {
    pub macros: Vec<Macro>,
    pub issues: Vec<MacroIssue>,
    /// ユーザーマクロの置き場 (UIが「ここにJSONを置けば並ぶ」と案内する)。
    #[serde(default)]
    pub user_dir: Option<String>,
}

impl Macro {
    /// 使えるバリアントキーを並び順で返す (既定の`"A"` + `variants`のキー)。
    /// 挿入プレビュー中のTab切替はこの並びを巡回する。
    pub fn variant_keys(&self) -> Vec<String> {
        let mut keys = vec![DEFAULT_VARIANT.to_string()];
        for v in &self.variants {
            if !keys.iter().any(|k| k.eq_ignore_ascii_case(&v.key)) {
                keys.push(v.key.clone());
            }
        }
        keys
    }

    /// 指定バリアントのCommand列 (JSONのまま)。`None`と`"A"`は既定の`commands`。
    fn variant_commands(&self, variant_key: Option<&str>) -> Result<&[serde_json::Value]> {
        let Some(key) = variant_key else {
            return Ok(&self.commands);
        };
        if let Some(v) = self.variants.iter().find(|v| v.key.eq_ignore_ascii_case(key)) {
            return Ok(&v.commands);
        }
        if key.eq_ignore_ascii_case(DEFAULT_VARIANT) {
            return Ok(&self.commands);
        }
        Err(CoreError::InvalidCommand(format!(
            "unknown macro variant: {key} (macro {})",
            self.id
        )))
    }

    /// 挿入先シート向けにUUIDを差し替えたCommand列を組み立てる。
    ///
    /// 座標はまだ`base_point`基準の相対のまま。配置(平行移動・回転)と参照記号の
    /// 再採番は[`insert_macro`]が行う。
    pub fn commands_for(
        &self,
        sheet_id: SheetId,
        variant_key: Option<&str>,
    ) -> Result<Vec<Command>> {
        let mut value = serde_json::Value::Array(self.variant_commands(variant_key)?.to_vec());
        let mut remap: BTreeMap<Uuid, Uuid> = BTreeMap::new();
        crate::templates::substitute(&mut value, sheet_id, &mut remap);
        Ok(serde_json::from_value(value)?)
    }
}

/// ユーザーマクロの置き場 (`~/MadakeCAD/macros`)。ホームが分からなければNone。
pub fn user_dir() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os(USER_MACROS_PATH_ENV) {
        return Some(PathBuf::from(p));
    }
    dirs::home_dir().map(|h| h.join("MadakeCAD").join("macros"))
}

/// 使えるマクロを列挙する(同梱は無く、ユーザー領域のみ)。
pub fn list() -> MacroList {
    list_from(user_dir().as_deref())
}

/// ディレクトリを明示して列挙する(テスト・組み込み用)。
///
/// 壊れたファイルが1つあっても残りは使えるように、**列挙は止めずに理由を持ち帰る**
/// (テンプレートと同じ方針)。
pub fn list_from(dir: Option<&Path>) -> MacroList {
    let mut out = MacroList::default();
    let Some(dir) = dir else {
        return out;
    };
    out.user_dir = Some(dir.display().to_string());
    for path in crate::templates::json_files(dir) {
        let loaded = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| parse(&text).map_err(|e| e.to_string()));
        match loaded {
            Ok(m) => match out.macros.iter_mut().find(|x| x.id == m.id) {
                Some(slot) => *slot = m,
                None => out.macros.push(m),
            },
            Err(message) => out.issues.push(MacroIssue {
                path: path.display().to_string(),
                message,
            }),
        }
    }
    out
}

/// 1つのマクロをidで取得する。
pub fn find(id: &str) -> Option<Macro> {
    list().macros.into_iter().find(|m| m.id == id)
}

/// JSONを読み、コマンド列 (既定・全バリアント) が本当にCommandとして読めるかまで確かめる。
fn parse(text: &str) -> serde_json::Result<Macro> {
    let m: Macro = serde_json::from_str(text)?;
    let _: Vec<Command> =
        serde_json::from_value(serde_json::Value::Array(m.commands.clone()))?;
    for v in &m.variants {
        let _: Vec<Command> =
            serde_json::from_value(serde_json::Value::Array(v.commands.clone()))?;
    }
    Ok(m)
}

/// 選択したエンティティを回路マクロへ逆変換する。
///
/// - 座標は基準点 ([`base_point`]) からの**相対座標**になる
/// - ワイヤの線番 ([`crate::model::Wire::net`]) は**捨てる** (挿入先の図面で振り直す)。
///   ネットラベルは回路の一部なのでそのまま残る
/// - エンティティidはプレースホルダとして保存し、挿入のたびに振り直される
///
/// 選択が空、または対象シートに無いidが混ざっているとエラー。
pub fn save_macro(
    project: &Project,
    sheet_id: SheetId,
    entity_ids: &[EntityId],
    meta: &MacroMeta,
) -> Result<Macro> {
    let sheet = project
        .sheet(sheet_id)
        .ok_or(CoreError::SheetNotFound(sheet_id))?;
    if entity_ids.is_empty() {
        return Err(CoreError::InvalidCommand(
            "macro selection is empty".into(),
        ));
    }
    let mut seen: BTreeSet<EntityId> = BTreeSet::new();
    let mut picked: Vec<&Entity> = Vec::new();
    for id in entity_ids {
        let entity = sheet
            .entities
            .get(id)
            .ok_or(CoreError::EntityNotFound(*id))?;
        if seen.insert(*id) {
            picked.push(entity);
        }
    }

    let base = base_point(&picked);
    let mut commands = Vec::with_capacity(picked.len());
    for entity in picked {
        let mut entity = entity.clone();
        entity.translate(-base.x, -base.y);
        if let Entity::Wire(w) = &mut entity {
            w.net = None;
        }
        commands.push(serde_json::to_value(Command::AddEntity {
            sheet_id: Uuid::nil(),
            entity,
        })?);
    }

    let id = if meta.id.trim().is_empty() {
        derive_id(&meta.name)
    } else {
        meta.id.trim().to_string()
    };
    Ok(Macro {
        id,
        name: meta.name.clone(),
        name_ja: meta.name_ja.clone(),
        description: meta.description.clone(),
        description_ja: meta.description_ja.clone(),
        category: meta.category.clone(),
        base_point: base,
        commands,
        variants: Vec::new(),
    })
}

/// 選択範囲の基準点: **左下に最も近いピン**。ピンが1つも無ければバウンディング
/// ボックスの左下角 (用紙座標系はY下向きなので x最小・y最大)。
///
/// 挿入時にこの点がカーソル位置へ来るので、ピンに合わせておくと配線の接続位置が
/// グリッドに乗ったまま置ける。
pub fn base_point(entities: &[&Entity]) -> Point {
    let mut pins: Vec<Point> = Vec::new();
    let mut pts: Vec<Point> = Vec::new();
    for entity in entities {
        match entity {
            Entity::Symbol(s) => {
                pts.push(s.at);
                if let Some(def) = resolve_symbol(&s.symbol_id) {
                    for (_, p) in crate::netlist::pin_positions(s, &def) {
                        pins.push(p);
                        pts.push(p);
                    }
                }
            }
            Entity::Wire(w) => pts.extend(w.points.iter().copied()),
            Entity::Harness(h) => pts.extend(h.points.iter().copied()),
            Entity::Junction(j) => pts.push(j.at),
            Entity::NetLabel(l) => pts.push(l.at),
            Entity::Text(t) => pts.push(t.at),
        }
    }
    if pts.is_empty() {
        return Point::default();
    }
    let min_x = pts.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let max_y = pts.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
    let corner = Point::new(min_x, max_y);
    pins.into_iter()
        .min_by(|a, b| {
            a.distance_to(&corner)
                .total_cmp(&b.distance_to(&corner))
                .then(a.x.total_cmp(&b.x))
                .then(b.y.total_cmp(&a.y))
        })
        .unwrap_or(corner)
}

/// 名前から安定したidを作る (英数字以外は`_`)。英数字が無い名前(日本語だけ等)は
/// 重複しないidを振る。
fn derive_id(name: &str) -> String {
    let mut id = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            id.push(ch.to_ascii_lowercase());
        } else if !id.ends_with('_') {
            id.push('_');
        }
    }
    let id = id.trim_matches('_').to_string();
    if id.is_empty() {
        format!("macro_{}", Uuid::new_v4().simple())
    } else {
        id
    }
}

/// マクロをユーザー領域(`~/MadakeCAD/macros`)へJSONとして書き出す。
pub fn write_macro(m: &Macro) -> Result<PathBuf> {
    let dir = user_dir().ok_or_else(|| {
        CoreError::InvalidCommand("home directory not found for the macros folder".into())
    })?;
    write_macro_to(&dir, m)
}

/// ディレクトリを明示してマクロを書き出す。ファイル名は`{id}.json`。
pub fn write_macro_to(dir: &Path, m: &Macro) -> Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let file: String = m
        .id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let path = dir.join(format!("{file}.json"));
    std::fs::write(&path, serde_json::to_string_pretty(m)?)?;
    Ok(path)
}

/// 読み込み済みのマクロを図面へ挿入する。**undo一発**で全体が戻る1件の編集になる。
///
/// - `at`: 基準点が来る位置 (カーソル位置)
/// - `rotation`: 0/90/180/270。回路全体を`at`まわりに回し、各シンボルの向きも回す
/// - 参照記号は図面で使用済みの最大番号の次から**自動再採番**される
///   ([`plan_references`]。マクロ内の相対関係は保たれる)
/// - ワイヤの線番は挿入時にクリアされる (図面ごとに振り直すもののため)
#[allow(clippy::too_many_arguments)]
pub fn insert_macro(
    engine: &mut Engine,
    m: &Macro,
    variant_key: Option<&str>,
    sheet_id: SheetId,
    at: Point,
    rotation: u16,
    origin: EditOrigin,
) -> Result<Patch> {
    if engine.project().sheet(sheet_id).is_none() {
        return Err(CoreError::SheetNotFound(sheet_id));
    }
    let mut commands = m.commands_for(sheet_id, variant_key)?;
    let renames = plan_references(engine.project(), &commands);
    for cmd in &mut commands {
        let Command::AddEntity { entity, .. } = cmd else {
            continue;
        };
        place_entity(entity, rotation, at);
        match entity {
            Entity::Symbol(s) => {
                if let Some(new) = renames.get(&s.reference) {
                    s.reference = new.clone();
                }
            }
            Entity::Wire(w) => w.net = None,
            _ => {}
        }
    }
    engine.execute_batch(commands, origin)
}

/// マクロをidで挿入する。未知のidは[`CoreError::InvalidCommand`]。
#[allow(clippy::too_many_arguments)]
pub fn apply(
    engine: &mut Engine,
    id: &str,
    variant_key: Option<&str>,
    sheet_id: SheetId,
    at: Point,
    rotation: u16,
    origin: EditOrigin,
) -> Result<Patch> {
    let m = find(id).ok_or_else(|| CoreError::InvalidCommand(format!("unknown macro: {id}")))?;
    insert_macro(engine, &m, variant_key, sheet_id, at, rotation, origin)
}

/// 相対座標のエンティティを挿入位置へ置く (回転 → 平行移動)。
/// 回転規則はシンボルのピン解決と共通 ([`rotate_local`])。
fn place_entity(entity: &mut Entity, rotation: u16, at: Point) {
    let map = |p: Point| {
        let r = rotate_local(p, rotation, false);
        Point::new(at.x + r.x, at.y + r.y)
    };
    match entity {
        Entity::Symbol(s) => {
            s.at = map(s.at);
            s.rotation = (s.rotation + rotation) % 360;
        }
        Entity::Wire(w) => {
            for p in &mut w.points {
                *p = map(*p);
            }
        }
        Entity::Harness(h) => {
            for p in &mut h.points {
                *p = map(*p);
            }
        }
        Entity::Junction(j) => j.at = map(j.at),
        Entity::NetLabel(l) => {
            l.at = map(l.at);
            l.rotation = (l.rotation + rotation) % 360;
        }
        Entity::Text(t) => {
            t.at = map(t.at);
            t.rotation = (t.rotation + rotation) % 360;
        }
    }
}

/// 参照記号の再採番計画 (マクロの記号 → 挿入後の記号)。
///
/// 接頭辞ごとに、図面(プロジェクト全体)で使用済みの最大番号の次から、マクロ内の
/// 番号の**小さい順**に連番を割り当てる。したがって:
///
/// - マクロ内の相対関係は保たれる (K1/K2 → K5/K6)
/// - 同じ記号は同じ記号のまま (コイルK1とその接点K1は挿入後も同じK5)
/// - 既存の記号とは衝突しない (2回挿入しても重複しない)
///
/// 番号の無い記号 (空文字・"K"だけ等) はそのまま残す。
pub fn plan_references(project: &Project, commands: &[Command]) -> BTreeMap<String, String> {
    let mut used: BTreeMap<String, u32> = BTreeMap::new();
    for sheet in &project.sheets {
        for entity in sheet.entities.values() {
            if let Entity::Symbol(s) = entity {
                if let Some((prefix, n)) = split_reference(&s.reference) {
                    let slot = used.entry(prefix).or_insert(0);
                    *slot = (*slot).max(n);
                }
            }
        }
    }
    let mut wanted: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
    for cmd in commands {
        if let Command::AddEntity {
            entity: Entity::Symbol(s),
            ..
        } = cmd
        {
            if let Some((prefix, n)) = split_reference(&s.reference) {
                wanted.entry(prefix).or_default().insert(n);
            }
        }
    }
    let mut renames = BTreeMap::new();
    for (prefix, numbers) in wanted {
        let mut next = used.get(&prefix).copied().unwrap_or(0);
        for n in numbers {
            next += 1;
            renames.insert(format!("{prefix}{n}"), format!("{prefix}{next}"));
        }
    }
    renames
}

/// 参照記号を接頭辞と番号へ分ける ("K12" → ("K", 12))。末尾が数字でない、または
/// 数字しかない記号はNone。
fn split_reference(reference: &str) -> Option<(String, u32)> {
    let r = reference.trim();
    let split = r
        .char_indices()
        .rev()
        .find(|(_, c)| !c.is_ascii_digit())
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    let (prefix, number) = r.split_at(split);
    if prefix.is_empty() || number.is_empty() {
        return None;
    }
    number.parse::<u32>().ok().map(|n| (prefix.to_string(), n))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Orientation, PaperSize, TextEntity};
    use crate::model::{NetLabel, SymbolInstance, Wire};
    use crate::netlist::pin_positions;
    use crate::verify::{verify_project, Severity};

    fn new_engine() -> (Engine, SheetId) {
        let project = Project::new("test");
        let sheet_id = project.sheets[0].id;
        (Engine::new(project), sheet_id)
    }

    fn add(engine: &mut Engine, sheet_id: SheetId, entity: Entity) -> EntityId {
        let id = entity.id();
        engine
            .execute(Command::AddEntity { sheet_id, entity })
            .expect("エンティティ追加");
        id
    }

    fn symbol(engine: &mut Engine, sheet_id: SheetId, symbol_id: &str, reference: &str, x: f64, y: f64) -> EntityId {
        add(
            engine,
            sheet_id,
            Entity::Symbol(SymbolInstance {
                id: Uuid::new_v4(),
                symbol_id: symbol_id.into(),
                at: Point::new(x, y),
                rotation: 0,
                mirror: false,
                reference: reference.into(),
                value: String::new(),
                attrs: Default::default(),
            }),
        )
    }

    fn wire(engine: &mut Engine, sheet_id: SheetId, pts: &[(f64, f64)], net: Option<&str>) -> EntityId {
        add(
            engine,
            sheet_id,
            Entity::Wire(Wire {
                id: Uuid::new_v4(),
                points: pts.iter().map(|&(x, y)| Point::new(x, y)).collect(),
                color: "red".into(),
                sq: 0.75,
                length_m: None,
                part_no: None,
                net: net.map(|s| s.to_string()),
            }),
        )
    }

    fn meta(id: &str) -> MacroMeta {
        MacroMeta {
            id: id.into(),
            name: "Test macro".into(),
            name_ja: "テストマクロ".into(),
            ..Default::default()
        }
    }

    fn sheet_ids(engine: &Engine, sheet_id: SheetId) -> Vec<EntityId> {
        engine
            .project()
            .sheet(sheet_id)
            .unwrap()
            .entities
            .keys()
            .copied()
            .collect()
    }

    /// Commandへ読み戻す(保存されたJSONが本当にCommand列であることの確認を兼ねる)。
    fn as_commands(values: &[serde_json::Value]) -> Vec<Command> {
        serde_json::from_value(serde_json::Value::Array(values.to_vec())).expect("Command列")
    }

    /// シート上のシンボルを (参照記号, symbol_id, 位置, 回転) で拾う。
    fn symbols(engine: &Engine, sheet_id: SheetId) -> Vec<(String, String, Point, u16)> {
        let mut out: Vec<(String, String, Point, u16)> = engine
            .project()
            .sheet(sheet_id)
            .unwrap()
            .entities
            .values()
            .filter_map(|e| match e {
                Entity::Symbol(s) => Some((
                    s.reference.clone(),
                    s.symbol_id.clone(),
                    s.at,
                    s.rotation,
                )),
                _ => None,
            })
            .collect();
        out.sort_by(|a, b| (a.2.x, a.2.y, &a.0).partial_cmp(&(b.2.x, b.2.y, &b.0)).unwrap());
        out
    }

    /// 図面の幾何(全シンボルのピン絶対座標と全ワイヤ頂点)を並べ替え済みで返す。
    fn geometry(engine: &Engine, sheet_id: SheetId) -> Vec<(String, String)> {
        let sheet = engine.project().sheet(sheet_id).unwrap();
        let mut out = Vec::new();
        for e in sheet.entities.values() {
            match e {
                Entity::Symbol(s) => {
                    if let Some(def) = resolve_symbol(&s.symbol_id) {
                        for (pin, p) in pin_positions(s, &def) {
                            out.push((
                                format!("{}:{}", s.symbol_id, pin),
                                format!("{:.3},{:.3}", p.x, p.y),
                            ));
                        }
                    }
                }
                Entity::Wire(w) => {
                    for p in &w.points {
                        out.push(("wire".into(), format!("{:.3},{:.3}", p.x, p.y)));
                    }
                }
                Entity::Junction(j) => {
                    out.push(("junction".into(), format!("{:.3},{:.3}", j.at.x, j.at.y)))
                }
                Entity::NetLabel(l) => out.push((
                    format!("label:{}", l.name),
                    format!("{:.3},{:.3}", l.at.x, l.at.y),
                )),
                _ => {}
            }
        }
        out.sort();
        out
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("madake-macros-{tag}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Saving a macro stores every coordinate relative to the base point, which is the pin closest to the bottom-left of the selection.
    /// マクロの保存では全座標が基準点からの相対座標になり、基準点は選択範囲の左下に最も近いピンになる。
    #[test]
    fn saving_a_macro_uses_the_bottom_left_pin_as_the_base_point() {
        let (mut engine, sheet_id) = new_engine();
        let coil = symbol(&mut engine, sheet_id, "relay_coil", "K1", 100.0, 100.0);
        let lamp = symbol(&mut engine, sheet_id, "lamp", "L1", 100.0, 130.0);
        let w = wire(
            &mut engine,
            sheet_id,
            &[(92.5, 130.0), (80.0, 130.0), (80.0, 100.0), (92.5, 100.0)],
            None,
        );

        let m = save_macro(
            engine.project(),
            sheet_id,
            &[coil, lamp, w],
            &meta("bottom_left"),
        )
        .expect("マクロ保存");

        // ランプの左ピン (92.5, 130) が選択範囲の左下角 (80, 130) に最も近い
        assert_eq!(m.base_point, Point::new(92.5, 130.0));

        let cmds = as_commands(&m.commands);
        assert_eq!(cmds.len(), 3);
        let mut placed = Vec::new();
        for cmd in &cmds {
            let Command::AddEntity { sheet_id, entity } = cmd else {
                panic!("マクロはadd_entityのCommand列 {cmd:?}");
            };
            assert!(sheet_id.is_nil(), "シートidはプレースホルダ(nil)");
            match entity {
                Entity::Symbol(s) => placed.push((s.reference.clone(), s.at)),
                Entity::Wire(w) => {
                    assert_eq!(w.points[0], Point::new(0.0, 0.0), "基準点が原点になる");
                }
                _ => {}
            }
        }
        placed.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            placed,
            vec![
                ("K1".to_string(), Point::new(7.5, -30.0)),
                ("L1".to_string(), Point::new(7.5, 0.0)),
            ]
        );
    }

    /// When the selection has no symbol pins at all, the base point falls back to the bottom-left corner of the bounding box.
    /// 選択範囲にシンボルのピンが1つも無いときは、基準点はバウンディングボックスの左下角になる。
    #[test]
    fn the_base_point_falls_back_to_the_bounding_box_corner_without_pins() {
        let (mut engine, sheet_id) = new_engine();
        let w = wire(&mut engine, sheet_id, &[(30.0, 10.0), (30.0, 50.0)], None);
        let t = add(
            &mut engine,
            sheet_id,
            Entity::Text(TextEntity {
                id: Uuid::new_v4(),
                at: Point::new(10.0, 20.0),
                text: "note".into(),
                height: 3.5,
                rotation: 0,
            }),
        );

        let m = save_macro(engine.project(), sheet_id, &[w, t], &meta("no_pins")).expect("保存");
        assert_eq!(m.base_point, Point::new(10.0, 50.0));
    }

    /// Wire numbers are dropped when a macro is saved (they are renumbered per drawing), while net labels are kept as part of the circuit.
    /// マクロの保存では線番は捨てられ(図面ごとに振り直すため)、ネットラベルは回路の一部としてそのまま残る。
    #[test]
    fn saving_a_macro_drops_wire_numbers_but_keeps_net_labels() {
        let (mut engine, sheet_id) = new_engine();
        let w = wire(
            &mut engine,
            sheet_id,
            &[(50.0, 50.0), (80.0, 50.0)],
            Some("101"),
        );
        let l = add(
            &mut engine,
            sheet_id,
            Entity::NetLabel(NetLabel {
                id: Uuid::new_v4(),
                at: Point::new(80.0, 50.0),
                name: "24V".into(),
                rotation: 0,
            }),
        );

        let m = save_macro(engine.project(), sheet_id, &[w, l], &meta("no_wire_no")).expect("保存");
        let cmds = as_commands(&m.commands);
        let mut saw_label = false;
        for cmd in &cmds {
            let Command::AddEntity { entity, .. } = cmd else {
                panic!()
            };
            match entity {
                Entity::Wire(w) => assert_eq!(w.net, None, "線番は保存されない"),
                Entity::NetLabel(l) => {
                    saw_label = true;
                    assert_eq!(l.name, "24V", "ネットラベルはそのまま残る");
                }
                _ => {}
            }
        }
        assert!(saw_label);
    }

    /// Saving refuses an empty selection, and an entity id that is not on the sheet is reported instead of silently skipped.
    /// 選択が空のマクロ保存は拒否され、そのシートに無いエンティティidは黙って飛ばさずエラーになる。
    #[test]
    fn saving_a_macro_refuses_an_empty_or_unknown_selection() {
        let (mut engine, sheet_id) = new_engine();
        let err = save_macro(engine.project(), sheet_id, &[], &meta("empty")).expect_err("空選択");
        assert!(matches!(err, CoreError::InvalidCommand(_)), "{err}");

        let ghost = Uuid::new_v4();
        let err = save_macro(engine.project(), sheet_id, &[ghost], &meta("ghost"))
            .expect_err("知らないid");
        assert!(err.to_string().contains(&ghost.to_string()), "{err}");
        let _ = &mut engine;
    }

    /// A macro saved without an id gets a stable one derived from its name, so the save dialog only has to ask for a name.
    /// idを指定せずに保存したマクロは名前から安定したidが作られる(保存ダイアログは名前だけ聞けばよい)。
    #[test]
    fn a_macro_without_an_id_derives_one_from_its_name() {
        let (mut engine, sheet_id) = new_engine();
        let s = symbol(&mut engine, sheet_id, "lamp", "L1", 50.0, 50.0);
        let m = save_macro(
            engine.project(),
            sheet_id,
            &[s],
            &MacroMeta {
                name: "Motor starter DOL".into(),
                ..Default::default()
            },
        )
        .expect("保存");
        assert_eq!(m.id, "motor_starter_dol");
    }

    /// Inserting a macro puts its base point exactly under the cursor position.
    /// マクロを挿入すると、基準点がちょうど指定した位置(カーソル位置)へ来る。
    #[test]
    fn inserting_a_macro_lands_the_base_point_on_the_cursor() {
        let (mut engine, sheet_id) = new_engine();
        let coil = symbol(&mut engine, sheet_id, "relay_coil", "K1", 100.0, 100.0);
        let lamp = symbol(&mut engine, sheet_id, "lamp", "L1", 100.0, 130.0);
        let m = save_macro(engine.project(), sheet_id, &[coil, lamp], &meta("place")).expect("保存");

        let (mut target, target_sheet) = new_engine();
        insert_macro(
            &mut target,
            &m,
            None,
            target_sheet,
            Point::new(200.0, 200.0),
            0,
            EditOrigin::User,
        )
        .expect("挿入");

        let placed = symbols(&target, target_sheet);
        assert_eq!(
            placed,
            vec![
                ("K1".into(), "relay_coil".into(), Point::new(207.5, 170.0), 0),
                ("L1".into(), "lamp".into(), Point::new(207.5, 200.0), 0),
            ]
        );
    }

    /// Inserting a macro rotated turns the whole circuit around the insertion point and turns each symbol with it.
    /// 回転を指定して挿入すると、回路全体が挿入点を中心に回り、各シンボルの向きも一緒に回る。
    #[test]
    fn inserting_a_macro_rotated_turns_the_whole_circuit() {
        let (mut engine, sheet_id) = new_engine();
        let coil = symbol(&mut engine, sheet_id, "relay_coil", "K1", 100.0, 100.0);
        let lamp = symbol(&mut engine, sheet_id, "lamp", "L1", 100.0, 130.0);
        let m = save_macro(engine.project(), sheet_id, &[coil, lamp], &meta("rot")).expect("保存");

        let (mut target, target_sheet) = new_engine();
        insert_macro(
            &mut target,
            &m,
            None,
            target_sheet,
            Point::new(200.0, 200.0),
            90,
            EditOrigin::User,
        )
        .expect("挿入");

        // 相対 (7.5, -30) は時計回り90度で (30, 7.5) になる
        let placed = symbols(&target, target_sheet);
        assert_eq!(
            placed,
            vec![
                ("L1".into(), "lamp".into(), Point::new(200.0, 207.5), 90),
                ("K1".into(), "relay_coil".into(), Point::new(230.0, 207.5), 90),
            ]
        );
    }

    /// Inserting a macro renumbers its reference designators from the highest one already used in the project, keeping the macro's own relations (K1/K2 stay two different relays, and a coil and its contact keep sharing one designator).
    /// マクロの挿入では参照記号が図面で使用済みの最大値の次から振り直され、マクロ内の関係は保たれる(K1/K2は別のリレーのまま、コイルとその接点は同じ記号を共有し続ける)。
    #[test]
    fn inserting_a_macro_renumbers_references_after_the_existing_ones() {
        let (mut engine, sheet_id) = new_engine();
        symbol(&mut engine, sheet_id, "relay_coil", "K4", 50.0, 50.0);
        let k1 = symbol(&mut engine, sheet_id, "relay_coil", "K1", 100.0, 100.0);
        let k1c = symbol(&mut engine, sheet_id, "relay_contact_no", "K1", 100.0, 120.0);
        let k2 = symbol(&mut engine, sheet_id, "relay_coil", "K2", 100.0, 140.0);
        let l1 = symbol(&mut engine, sheet_id, "lamp", "L1", 140.0, 100.0);
        let m = save_macro(
            engine.project(),
            sheet_id,
            &[k1, k1c, k2, l1],
            &meta("renumber"),
        )
        .expect("保存");

        let before = sheet_ids(&engine, sheet_id);
        insert_macro(
            &mut engine,
            &m,
            None,
            sheet_id,
            Point::new(250.0, 100.0),
            0,
            EditOrigin::User,
        )
        .expect("挿入");

        let sheet = engine.project().sheet(sheet_id).unwrap();
        let mut inserted: Vec<(String, String)> = sheet
            .entities
            .iter()
            .filter(|(id, _)| !before.contains(id))
            .filter_map(|(_, e)| match e {
                Entity::Symbol(s) => Some((s.symbol_id.clone(), s.reference.clone())),
                _ => None,
            })
            .collect();
        inserted.sort();
        assert_eq!(
            inserted,
            vec![
                ("lamp".to_string(), "L2".to_string()),
                ("relay_coil".to_string(), "K5".to_string()),
                ("relay_coil".to_string(), "K6".to_string()),
                ("relay_contact_no".to_string(), "K5".to_string()),
            ],
            "K1→K5・K2→K6(既存はK4まで)、K1のコイルと接点は同じK5、L1→L2"
        );
    }

    /// Any wire number left in a hand-written macro file is cleared on insert, so numbering always belongs to the drawing it lands in.
    /// 手書きのマクロファイルに線番が残っていても挿入時に消される(線番は挿入先の図面のものだから)。
    #[test]
    fn wire_numbers_in_a_macro_file_are_cleared_on_insert() {
        let m: Macro = serde_json::from_str(
            r#"{
                "id": "with_wire_no",
                "name": "with wire no",
                "base_point": { "x": 0.0, "y": 0.0 },
                "commands": [
                    {
                        "type": "add_entity",
                        "sheet_id": "00000000-0000-0000-0000-000000000000",
                        "entity": {
                            "kind": "wire",
                            "id": "00000000-0000-0000-0000-000000000001",
                            "points": [ { "x": 0.0, "y": 0.0 }, { "x": 10.0, "y": 0.0 } ],
                            "color": "red", "sq": 0.75, "net": "7"
                        }
                    }
                ]
            }"#,
        )
        .expect("マクロJSON");

        let (mut engine, sheet_id) = new_engine();
        insert_macro(
            &mut engine,
            &m,
            None,
            sheet_id,
            Point::new(50.0, 50.0),
            0,
            EditOrigin::User,
        )
        .expect("挿入");
        let sheet = engine.project().sheet(sheet_id).unwrap();
        let Entity::Wire(w) = sheet.entities.values().next().unwrap() else {
            panic!("ワイヤ")
        };
        assert_eq!(w.net, None, "線番は挿入時にクリアされる");
        assert_eq!(w.points[0], Point::new(50.0, 50.0));
    }

    /// A macro can hold alternative variants: the default commands are variant "A" and any other key picks its own circuit.
    /// マクロは代替バリアントを持てる。既定のcommandsがバリアント「A」で、他のキーを指定するとそのバリアントの回路が入る。
    #[test]
    fn a_variant_can_be_chosen_when_inserting() {
        let m: Macro = serde_json::from_str(
            r#"{
                "id": "variants",
                "name": "variants",
                "name_ja": "バリアント",
                "base_point": { "x": 0.0, "y": 0.0 },
                "commands": [
                    { "type": "add_entity", "sheet_id": "00000000-0000-0000-0000-000000000000",
                      "entity": { "kind": "symbol", "id": "00000000-0000-0000-0000-000000000001",
                                  "symbol_id": "lamp", "at": { "x": 0.0, "y": 0.0 }, "reference": "L1" } }
                ],
                "variants": [
                    { "key": "B", "name": "Motor", "name_ja": "モータ",
                      "commands": [
                        { "type": "add_entity", "sheet_id": "00000000-0000-0000-0000-000000000000",
                          "entity": { "kind": "symbol", "id": "00000000-0000-0000-0000-000000000002",
                                      "symbol_id": "motor", "at": { "x": 0.0, "y": 0.0 }, "reference": "M1" } }
                      ] }
                ]
            }"#,
        )
        .expect("マクロJSON");
        assert_eq!(m.variant_keys(), vec!["A".to_string(), "B".to_string()]);

        for (key, expected) in [
            (None, "lamp"),
            (Some("A"), "lamp"),
            (Some("B"), "motor"),
        ] {
            let (mut engine, sheet_id) = new_engine();
            insert_macro(
                &mut engine,
                &m,
                key,
                sheet_id,
                Point::new(100.0, 100.0),
                0,
                EditOrigin::User,
            )
            .expect("挿入");
            assert_eq!(symbols(&engine, sheet_id)[0].1, expected, "variant={key:?}");
        }
    }

    /// An unknown variant key is refused with an error naming the key, and the drawing is left untouched.
    /// 知らないバリアントキーはキーを添えたエラーで拒否され、図面は変わらない。
    #[test]
    fn an_unknown_variant_key_is_refused() {
        let (mut engine, sheet_id) = new_engine();
        let s = symbol(&mut engine, sheet_id, "lamp", "L1", 50.0, 50.0);
        let m = save_macro(engine.project(), sheet_id, &[s], &meta("v")).expect("保存");
        let depth = engine.undo_depth();

        let err = insert_macro(
            &mut engine,
            &m,
            Some("Z"),
            sheet_id,
            Point::new(100.0, 100.0),
            0,
            EditOrigin::User,
        )
        .expect_err("知らないバリアント");
        assert!(err.to_string().contains('Z'), "{err}");
        assert_eq!(engine.project().sheet(sheet_id).unwrap().entities.len(), 1);
        assert_eq!(engine.undo_depth(), depth, "履歴も積まれない");
    }

    /// Inserting a macro is one edit: a single undo takes the whole circuit back out, and a single redo brings it back.
    /// マクロの挿入は1回の編集なので、undo一発で回路全体が消え、redo一発で戻ってくる。
    #[test]
    fn inserting_a_macro_is_undone_in_one_step() {
        let (mut engine, sheet_id) = new_engine();
        crate::templates::apply(&mut engine, "motor_starter", sheet_id, EditOrigin::User)
            .expect("テンプレート");
        let ids = sheet_ids(&engine, sheet_id);
        let m = save_macro(engine.project(), sheet_id, &ids, &meta("motor")).expect("保存");

        let (mut target, target_sheet) = new_engine();
        insert_macro(
            &mut target,
            &m,
            None,
            target_sheet,
            Point::new(100.0, 100.0),
            0,
            EditOrigin::User,
        )
        .expect("挿入");
        let placed = target.project().sheet(target_sheet).unwrap().entities.len();
        assert_eq!(placed, ids.len(), "選択した全エンティティが入る");
        assert_eq!(target.undo_depth(), 1, "履歴は1件");

        target.undo().expect("undo").expect("戻せる");
        assert!(target
            .project()
            .sheet(target_sheet)
            .unwrap()
            .entities
            .is_empty());
        target.redo().expect("redo").expect("やり直せる");
        assert_eq!(
            target.project().sheet(target_sheet).unwrap().entities.len(),
            placed
        );
    }

    /// Saving a circuit as a macro and inserting it back at its base point reproduces the drawing exactly: same pins, wires, junctions and labels at the same coordinates.
    /// 回路をマクロとして保存し基準点の位置へ挿入し直すと、図面がそのまま再現される(ピン・配線・接続点・ラベルが同じ座標に来る)。
    #[test]
    fn a_saved_macro_reproduces_the_drawing_when_inserted_back() {
        let (mut engine, sheet_id) = new_engine();
        crate::templates::apply(&mut engine, "motor_starter", sheet_id, EditOrigin::User)
            .expect("テンプレート");
        let ids = sheet_ids(&engine, sheet_id);
        let m = save_macro(engine.project(), sheet_id, &ids, &meta("roundtrip")).expect("保存");

        let (mut target, target_sheet) = new_engine();
        insert_macro(
            &mut target,
            &m,
            None,
            target_sheet,
            m.base_point,
            0,
            EditOrigin::User,
        )
        .expect("挿入");

        assert_eq!(
            geometry(&target, target_sheet),
            geometry(&engine, sheet_id),
            "往復しても図面は幾何学的に等価"
        );
        let nets = crate::netlist::extract_netlist(
            target.project().sheet(target_sheet).unwrap(),
            &crate::symbol::sheet_symbol_defs(target.project().sheet(target_sheet).unwrap()),
        );
        let original = crate::netlist::extract_netlist(
            engine.project().sheet(sheet_id).unwrap(),
            &crate::symbol::sheet_symbol_defs(engine.project().sheet(sheet_id).unwrap()),
        );
        assert_eq!(nets.len(), original.len(), "ネットの数も同じ");
    }

    /// A motor circuit saved as a macro can be inserted twice into another sheet: the two copies never share a reference designator and the drawing still passes verification with no errors or ERC warnings.
    /// モータ回路のマクロは別シートへ2回挿入でき、2つのコピーが参照記号を取り合うことはなく、図面は検証でエラー0・ERC警告0のまま通る。
    #[test]
    fn a_macro_inserted_twice_keeps_references_unique_and_passes_verification() {
        let (mut engine, sheet_id) = new_engine();
        crate::templates::apply(&mut engine, "motor_starter", sheet_id, EditOrigin::User)
            .expect("テンプレート");
        let ids = sheet_ids(&engine, sheet_id);
        let m = save_macro(engine.project(), sheet_id, &ids, &meta("motor")).expect("保存");

        engine
            .execute(Command::AddSheet {
                name: "Sheet2".into(),
                size: PaperSize::A3,
                orientation: Orientation::Landscape,
            })
            .unwrap();
        let second = engine.project().sheets[1].id;

        let refs = |engine: &Engine, sheet: SheetId| -> BTreeSet<String> {
            engine
                .project()
                .sheet(sheet)
                .unwrap()
                .entities
                .values()
                .filter_map(|e| match e {
                    Entity::Symbol(s) if !s.reference.is_empty() => Some(s.reference.clone()),
                    _ => None,
                })
                .collect()
        };
        let sheet1_refs = refs(&engine, sheet_id);

        insert_macro(
            &mut engine,
            &m,
            None,
            second,
            Point::new(60.0, 40.0),
            0,
            EditOrigin::User,
        )
        .expect("1回目");
        let first_copy = refs(&engine, second);
        insert_macro(
            &mut engine,
            &m,
            None,
            second,
            Point::new(60.0, 160.0),
            0,
            EditOrigin::User,
        )
        .expect("2回目");
        let second_copy: BTreeSet<String> = refs(&engine, second)
            .difference(&first_copy)
            .cloned()
            .collect();

        assert!(!first_copy.is_empty() && !second_copy.is_empty());
        assert!(
            first_copy.is_disjoint(&second_copy),
            "2つのコピーで参照記号が重複しない {first_copy:?} / {second_copy:?}"
        );
        assert!(
            sheet1_refs.is_disjoint(&first_copy) && sheet1_refs.is_disjoint(&second_copy),
            "元の回路の参照記号とも重複しない"
        );

        let diags = verify_project(engine.project());
        let bad: Vec<&str> = diags
            .iter()
            .filter(|d| d.severity != Severity::Info)
            .map(|d| d.message.as_str())
            .collect();
        assert!(bad.is_empty(), "エラー・警告が残っている {bad:?}");
    }

    /// Macros are written to and listed from the user's macros folder, and a file that is not a valid macro is reported with its path and reason while the others stay usable.
    /// マクロはユーザーのマクロフォルダへ書き出され、そこから一覧される。マクロとして読めないファイルはパスと理由を添えて報告され、他のマクロはそのまま使える。
    #[test]
    fn macros_round_trip_through_the_user_folder_and_broken_files_are_reported() {
        let dir = temp_dir("list");
        let (mut engine, sheet_id) = new_engine();
        let s = symbol(&mut engine, sheet_id, "lamp", "L1", 50.0, 50.0);
        let m = save_macro(engine.project(), sheet_id, &[s], &meta("my_lamp")).expect("保存");
        let path = write_macro_to(&dir, &m).expect("書き出し");
        assert!(path.exists(), "{}", path.display());

        std::fs::write(dir.join("broken.json"), "{ this is not json").unwrap();
        std::fs::write(
            dir.join("unknown-command.json"),
            r#"{ "id": "x", "name": "x", "commands": [ { "type": "fly_away" } ] }"#,
        )
        .unwrap();

        let list = list_from(Some(&dir));
        assert_eq!(list.macros.len(), 1, "読めたマクロは残る");
        assert_eq!(list.macros[0].id, "my_lamp");
        assert_eq!(list.macros[0].name_ja, "テストマクロ");
        assert_eq!(list.macros[0].base_point, m.base_point);
        assert_eq!(
            list.user_dir.as_deref(),
            Some(dir.display().to_string().as_str()),
            "UIが案内できるよう置き場も返す"
        );
        assert_eq!(list.issues.len(), 2, "壊れた2ファイルを報告する");
        let paths: Vec<&str> = list.issues.iter().map(|i| i.path.as_str()).collect();
        assert!(paths.iter().any(|p| p.ends_with("broken.json")), "{paths:?}");
        assert!(
            paths.iter().any(|p| p.ends_with("unknown-command.json")),
            "{paths:?}"
        );
        assert!(list.issues.iter().all(|i| !i.message.is_empty()));

        // 読み戻したマクロもそのまま挿入できる
        let (mut target, target_sheet) = new_engine();
        insert_macro(
            &mut target,
            &list.macros[0],
            None,
            target_sheet,
            Point::new(10.0, 10.0),
            0,
            EditOrigin::User,
        )
        .expect("挿入");
        assert_eq!(symbols(&target, target_sheet).len(), 1);

        std::fs::remove_dir_all(&dir).ok();
    }

    /// Inserting into a sheet that is not in the project is refused, and an unknown macro id is refused by name.
    /// プロジェクトに無いシートへの挿入は拒否され、知らないマクロidは名前を添えて拒否される。
    #[test]
    fn inserting_into_an_unknown_sheet_or_by_an_unknown_id_is_refused() {
        let (mut engine, sheet_id) = new_engine();
        let s = symbol(&mut engine, sheet_id, "lamp", "L1", 50.0, 50.0);
        let m = save_macro(engine.project(), sheet_id, &[s], &meta("x")).expect("保存");

        let err = insert_macro(
            &mut engine,
            &m,
            None,
            Uuid::new_v4(),
            Point::new(0.0, 0.0),
            0,
            EditOrigin::User,
        )
        .expect_err("知らないシート");
        assert!(matches!(err, CoreError::SheetNotFound(_)), "{err}");

        let err = apply(
            &mut engine,
            "no_such_macro",
            None,
            sheet_id,
            Point::new(0.0, 0.0),
            0,
            EditOrigin::User,
        )
        .expect_err("知らないid");
        assert!(err.to_string().contains("no_such_macro"), "{err}");
    }
}
