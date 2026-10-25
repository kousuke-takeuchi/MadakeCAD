//! ユーザーシンボルライブラリ (M4仕様 §7)。
//!
//! 同梱シンボル ([`crate::symbol::builtin_symbols`]) では足りない記号を、現場で作って
//! 使えるようにする。保存先はユーザー領域 (`~/MadakeCAD/symbols/*.json`) だけで、
//! ファイルの中身は [`SymbolDef`] のJSONそのまま (エディタが無くてもテキストで書ける)。
//!
//! ```jsonc
//! {
//!   "id": "my_contactor_3p", "name": "My contactor 3P", "name_ja": "自社コンタクタ3P",
//!   "category": "user", "ref_prefix": "K", "keywords": ["contactor", "電磁接触器"],
//!   "primitives": [ { "type": "rect", "p1": {…}, "p2": {…}, "filled": false } ],
//!   "pins": [ { "number": "1", "name": "", "at": { "x": -5.0, "y": -2.5 }, "dir": "left" } ],
//!   "text_slots": [ { "key": "TAG", "at": { "x": 0.0, "y": -12.5 }, "height": 2.5 } ]
//! }
//! ```
//!
//! ## 決めごと
//!
//! - **分類は必ず`user`** ([`USER_CATEGORY`])。ファイルに何を書いても読み込み・保存の
//!   ときに`user`へ正規化するので、部品挿入ダイアログでは同梱の分類の後ろに
//!   「ユーザー」のグループが1つ増える形になる (同梱の分類へ紛れ込ませない)
//! - **同梱シンボル・ピン数可変シンボルとid衝突するユーザーシンボルは拒否**する。
//!   後から置いたファイルが同梱の記号を乗っ取ると、**そのidを使っている既存図面の
//!   見た目が勝手に変わる**ため。衝突は一覧で理由つきの[`SymbolIssue`]として報告する
//! - **図面はシンボル定義のスナップショットを持たない** (エンティティは`symbol_id`だけを
//!   持つ)。つまりユーザーシンボルを編集すると、**すでに置いてある部品の描画・ピン位置・
//!   ネットリストも次の描画から変わる** (ライブラリ追従。ACADE/EPLANの
//!   「ライブラリからシンボルを更新」が常時ONの状態)
//! - **ライブラリはファイルのドメインで、Commandエンジンの外**にある。保存・削除は
//!   `doc:patch`にもundo履歴にも乗らない (**undoで元に戻らない**)。図面を変えるのは
//!   あくまで配置 (`add_entity`) 側
//! - 壊れたJSON・検証に落ちたファイルは**一覧を止めずに**パス+理由で持ち帰る
//!   (開始テンプレート・回路マクロと同じ方針)

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::{Entity, Project};
use crate::symbol::{
    builtin_symbols, dynamic_symbol, PinDef, Primitive, SymbolDef, SLOT_DESC, SLOT_PART,
    SLOT_RATING, SLOT_TAG,
};
use crate::tidy::GRID_PITCH;
use crate::{CoreError, Result};

/// ユーザーシンボルの置き場を差し替える環境変数(既定は`~/MadakeCAD/symbols`)。
pub const USER_SYMBOLS_PATH_ENV: &str = "MADAKE_USER_SYMBOLS_PATH";

/// ユーザーシンボルの分類。部品挿入ダイアログの「ユーザー」グループになる。
pub const USER_CATEGORY: &str = "user";

/// 属性スロットに使えるキー。これ以外は描画側が知らないので保存時に弾く。
pub const SLOT_KEYS: &[&str] = &[SLOT_TAG, SLOT_PART, SLOT_DESC, SLOT_RATING];

/// 検証エラーの安定コード。UIはこれで文言を出し分ける。
pub mod code {
    pub const ID_EMPTY: &str = "symbol.id_empty";
    pub const ID_INVALID: &str = "symbol.id_invalid";
    pub const ID_RESERVED: &str = "symbol.id_reserved";
    pub const NAME_EMPTY: &str = "symbol.name_empty";
    pub const REF_PREFIX_EMPTY: &str = "symbol.ref_prefix_empty";
    pub const NO_PRIMITIVES: &str = "symbol.no_primitives";
    pub const PRIMITIVE_INVALID: &str = "symbol.primitive_invalid";
    pub const NO_PINS: &str = "symbol.no_pins";
    pub const PIN_NUMBER_EMPTY: &str = "symbol.pin_number_empty";
    pub const PIN_OFF_GRID: &str = "symbol.pin_off_grid";
    pub const PIN_DUPLICATE: &str = "symbol.pin_duplicate";
    pub const PIN_OVERLAP: &str = "symbol.pin_overlap";
    pub const SLOT_MISSING: &str = "symbol.slot_missing";
    pub const SLOT_UNKNOWN: &str = "symbol.slot_unknown";
    pub const SLOT_DUPLICATE: &str = "symbol.slot_duplicate";
    pub const SLOT_HEIGHT: &str = "symbol.slot_height";
}

/// 保存できない理由1件。[`validate`]は見つかった理由を**全部**返す(直す回数を減らす)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SymbolError {
    /// 安定コード ([`code`])。
    pub code: String,
    /// 日本語の説明 (どのピン・どのスロットが悪いかまで書く)。
    pub message: String,
}

impl SymbolError {
    fn new(code: &str, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into() }
    }
}

impl std::fmt::Display for SymbolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// 読み込めなかったユーザーシンボル1件(壊れたJSON・検証に落ちた・id衝突)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolIssue {
    pub path: String,
    pub message: String,
}

/// ユーザーシンボルの一覧と、読み込めなかったファイルの理由。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SymbolLibrary {
    pub symbols: Vec<SymbolDef>,
    pub issues: Vec<SymbolIssue>,
    /// ユーザーシンボルの置き場 (UIが「ここにJSONを置けば並ぶ」と案内する)。
    pub user_dir: Option<String>,
}

/// ユーザーシンボルの置き場 (`~/MadakeCAD/symbols`)。ホームが分からなければNone。
pub fn user_dir() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os(USER_SYMBOLS_PATH_ENV) {
        return Some(PathBuf::from(p));
    }
    dirs::home_dir().map(|h| h.join("MadakeCAD").join("symbols"))
}

/// ユーザーシンボルを列挙する(同梱は含まない)。
pub fn list() -> SymbolLibrary {
    list_from(user_dir().as_deref())
}

/// ディレクトリを明示して列挙する(テスト・組み込み用)。
pub fn list_from(dir: Option<&Path>) -> SymbolLibrary {
    let mut out = SymbolLibrary::default();
    let Some(dir) = dir else {
        return out;
    };
    out.user_dir = Some(dir.display().to_string());
    for path in crate::templates::json_files(dir) {
        let loaded = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| serde_json::from_str::<SymbolDef>(&text).map_err(|e| e.to_string()));
        let issue = |message: String| SymbolIssue { path: path.display().to_string(), message };
        match loaded {
            Ok(def) => {
                let def = normalize(&def);
                let errors = validate(&def);
                if !errors.is_empty() {
                    let reasons: Vec<String> =
                        errors.into_iter().map(|e| e.message).collect();
                    out.issues.push(issue(reasons.join(" / ")));
                } else if out.symbols.iter().any(|s| s.id == def.id) {
                    out.issues
                        .push(issue(format!("idが重複しています: {}", def.id)));
                } else {
                    out.symbols.push(def);
                }
            }
            Err(message) => out.issues.push(issue(message)),
        }
    }
    out
}

/// ユーザーシンボル1つをidで取得する。
pub fn find(id: &str) -> Option<SymbolDef> {
    list().symbols.into_iter().find(|s| s.id == id)
}

/// 部品挿入ダイアログに並べる全シンボル = 同梱 + ユーザー(ユーザーが後ろ)。
pub fn all() -> Vec<SymbolDef> {
    all_with(&list())
}

/// 一覧を明示して合流させる(テスト・列挙を使い回すとき用)。
pub fn all_with(user: &SymbolLibrary) -> Vec<SymbolDef> {
    let mut out = builtin_symbols();
    out.extend(user.symbols.iter().cloned());
    out
}

/// 保存・読み込みの前に形を整える。
///
/// - 前後の空白を落とす
/// - 分類は必ず[`USER_CATEGORY`]
/// - 英語名・日本語名は片方だけ書けば、もう片方へコピーする(日本語だけで作れる)
/// - キーワードは空を捨てて重複を除く
///
/// **ピン座標は動かさない** (グリッド外は黙って直さず[`validate`]で指摘する)。
pub fn normalize(def: &SymbolDef) -> SymbolDef {
    let mut out = def.clone();
    out.id = out.id.trim().to_string();
    out.name = out.name.trim().to_string();
    out.name_ja = out.name_ja.trim().to_string();
    if out.name.is_empty() {
        out.name = out.name_ja.clone();
    }
    if out.name_ja.is_empty() {
        out.name_ja = out.name.clone();
    }
    out.ref_prefix = out.ref_prefix.trim().to_string();
    out.category = USER_CATEGORY.to_string();
    let mut keywords: Vec<String> = Vec::new();
    for k in out.keywords.iter() {
        let k = k.trim().to_string();
        if !k.is_empty() && !keywords.contains(&k) {
            keywords.push(k);
        }
    }
    out.keywords = keywords;
    for pin in out.pins.iter_mut() {
        pin.number = pin.number.trim().to_string();
        pin.name = pin.name.trim().to_string();
    }
    for slot in out.text_slots.iter_mut() {
        slot.key = slot.key.trim().to_uppercase();
    }
    out
}

/// 点が2.5mmグリッドに乗っているか。
fn on_grid(v: f64) -> bool {
    (v - (v / GRID_PITCH).round() * GRID_PITCH).abs() < 1e-6
}

/// 座標をマップのキーにする(浮動小数の比較を0.001mmで丸める)。
fn key_of(pin: &PinDef) -> (i64, i64) {
    ((pin.at.x * 1000.0).round() as i64, (pin.at.y * 1000.0).round() as i64)
}

/// 保存できるシンボルかを確かめる。**問題は全部返す**(空なら保存してよい)。
///
/// 見るのは「置いたら図面が壊れる/描けない」ものだけ:
/// idの体裁と衝突・名前と参照記号接頭辞・図形0個・接続点(0個/番号なし/グリッド外/
/// 同じ点の重なり)・属性スロット(TAG必須・知らないキー・重複・文字高0)。
///
/// **同じ番号の接続点を離れた位置に置くのは正しい**(端子台の貫通端子=内部短絡)。
/// 弾くのは同じ番号が**同じ点**にある場合と、違う番号が同じ点にある場合。
pub fn validate(def: &SymbolDef) -> Vec<SymbolError> {
    let mut errors = Vec::new();
    let id = def.id.trim();
    if id.is_empty() {
        errors.push(SymbolError::new(code::ID_EMPTY, "idが空です"));
    } else if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        errors.push(SymbolError::new(
            code::ID_INVALID,
            format!("idに使えるのは英数字・アンダースコア・ハイフンだけです: {id}"),
        ));
    } else if builtin_symbols().iter().any(|s| s.id == id) {
        errors.push(SymbolError::new(
            code::ID_RESERVED,
            format!("同梱シンボルと同じidは使えません(既存図面の記号が変わってしまいます): {id}"),
        ));
    } else if dynamic_symbol(id).is_some() {
        errors.push(SymbolError::new(
            code::ID_RESERVED,
            format!("ピン数可変シンボルと同じidは使えません: {id}"),
        ));
    }
    if def.name.trim().is_empty() && def.name_ja.trim().is_empty() {
        errors.push(SymbolError::new(
            code::NAME_EMPTY,
            "名称(英語・日本語のどちらか)を入れてください",
        ));
    }
    if def.ref_prefix.trim().is_empty() {
        errors.push(SymbolError::new(
            code::REF_PREFIX_EMPTY,
            "参照記号の接頭辞(K・PB など)を入れてください",
        ));
    }

    if def.primitives.is_empty() {
        errors.push(SymbolError::new(
            code::NO_PRIMITIVES,
            "図形が1つもありません(線・矩形・円などを描いてください)",
        ));
    }
    for (i, prim) in def.primitives.iter().enumerate() {
        let bad = match prim {
            Primitive::Line { pts } => (pts.len() < 2).then(|| "線の点が1つ以下です".to_string()),
            Primitive::Circle { r, .. } => (*r <= 0.0).then(|| format!("円の半径が0以下です: {r}")),
            Primitive::Arc { r, .. } => (*r <= 0.0).then(|| format!("弧の半径が0以下です: {r}")),
            Primitive::Rect { p1, p2, .. } => {
                ((p1.x - p2.x).abs() < 1e-9 && (p1.y - p2.y).abs() < 1e-9)
                    .then(|| "矩形の幅と高さが0です".to_string())
            }
            Primitive::Text { text, height, .. } => {
                if text.trim().is_empty() {
                    Some("文字が空です".to_string())
                } else {
                    (*height <= 0.0).then(|| format!("文字高が0以下です: {height}"))
                }
            }
        };
        if let Some(message) = bad {
            errors.push(SymbolError::new(
                code::PRIMITIVE_INVALID,
                format!("図形{}: {message}", i + 1),
            ));
        }
    }

    if def.pins.is_empty() {
        errors.push(SymbolError::new(
            code::NO_PINS,
            "接続点(ピン)が1つもありません",
        ));
    }
    let mut seen: BTreeMap<(String, i64, i64), usize> = BTreeMap::new();
    let mut at_point: BTreeMap<(i64, i64), String> = BTreeMap::new();
    for pin in &def.pins {
        let number = pin.number.trim();
        if number.is_empty() {
            errors.push(SymbolError::new(
                code::PIN_NUMBER_EMPTY,
                format!("番号の無い接続点があります ({}, {})", pin.at.x, pin.at.y),
            ));
            continue;
        }
        if !on_grid(pin.at.x) || !on_grid(pin.at.y) {
            errors.push(SymbolError::new(
                code::PIN_OFF_GRID,
                format!(
                    "接続点{number}が{GRID_PITCH}mmグリッドから外れています ({}, {})",
                    pin.at.x, pin.at.y
                ),
            ));
        }
        let (x, y) = key_of(pin);
        if seen.insert((number.to_string(), x, y), 1).is_some() {
            errors.push(SymbolError::new(
                code::PIN_DUPLICATE,
                format!("接続点{number}が同じ位置に2つあります ({}, {})", pin.at.x, pin.at.y),
            ));
            continue;
        }
        match at_point.get(&(x, y)) {
            Some(other) if other != number => errors.push(SymbolError::new(
                code::PIN_OVERLAP,
                format!(
                    "接続点{other}と{number}が同じ位置に重なっています ({}, {})",
                    pin.at.x, pin.at.y
                ),
            )),
            _ => {
                at_point.insert((x, y), number.to_string());
            }
        }
    }

    let mut slot_keys: Vec<&str> = Vec::new();
    for slot in &def.text_slots {
        let key = slot.key.trim();
        if !SLOT_KEYS.contains(&key) {
            errors.push(SymbolError::new(
                code::SLOT_UNKNOWN,
                format!(
                    "知らない属性スロット「{key}」です(使えるのは {})",
                    SLOT_KEYS.join(" / ")
                ),
            ));
            continue;
        }
        if slot_keys.contains(&key) {
            errors.push(SymbolError::new(
                code::SLOT_DUPLICATE,
                format!("属性スロット「{key}」が重複しています"),
            ));
            continue;
        }
        if slot.height <= 0.0 {
            errors.push(SymbolError::new(
                code::SLOT_HEIGHT,
                format!("属性スロット「{key}」の文字高が0以下です: {}", slot.height),
            ));
        }
        slot_keys.push(key);
    }
    if !slot_keys.contains(&SLOT_TAG) {
        errors.push(SymbolError::new(
            code::SLOT_MISSING,
            format!("参照記号のスロット({SLOT_TAG})は必須です"),
        ));
    }

    errors
}

/// ユーザー領域(`~/MadakeCAD/symbols`)へ保存する。
pub fn save(def: &SymbolDef) -> std::result::Result<(SymbolDef, PathBuf), Vec<SymbolError>> {
    let dir = user_dir().ok_or_else(|| {
        vec![SymbolError::new(
            code::ID_INVALID,
            "ユーザーシンボルの置き場(ホームディレクトリ)が見つかりません",
        )]
    })?;
    save_to(&dir, def)
}

/// ディレクトリを明示して保存する。ファイル名は`{id}.json`。
///
/// **検証に1つでも落ちたら何も書かない**(半分だけ直った定義をライブラリに残さない)。
/// 保存はCommandエンジンを通らないので**undoでは戻らない**。
pub fn save_to(
    dir: &Path,
    def: &SymbolDef,
) -> std::result::Result<(SymbolDef, PathBuf), Vec<SymbolError>> {
    let def = normalize(def);
    let errors = validate(&def);
    if !errors.is_empty() {
        return Err(errors);
    }
    let io = |e: std::io::Error| vec![SymbolError::new(code::ID_INVALID, e.to_string())];
    std::fs::create_dir_all(dir).map_err(io)?;
    let path = dir.join(format!("{}.json", def.id));
    let text = serde_json::to_string_pretty(&def)
        .map_err(|e| vec![SymbolError::new(code::ID_INVALID, e.to_string())])?;
    std::fs::write(&path, text).map_err(io)?;
    Ok((def, path))
}

/// ユーザーシンボルを削除する(ファイルを消す)。**undoでは戻らない**。
pub fn delete(id: &str) -> Result<PathBuf> {
    let dir = user_dir().ok_or_else(|| {
        CoreError::InvalidCommand("home directory not found for the symbols folder".into())
    })?;
    delete_from(&dir, id)
}

/// ディレクトリを明示して削除する。同梱シンボル・無いidはエラー(何も消さない)。
pub fn delete_from(dir: &Path, id: &str) -> Result<PathBuf> {
    let library = list_from(Some(dir));
    if !library.symbols.iter().any(|s| s.id == id) {
        return Err(CoreError::SymbolNotFound(id.to_string()));
    }
    let path = dir.join(format!("{id}.json"));
    std::fs::remove_file(&path)?;
    Ok(path)
}

/// 既存シンボルを複製して編集を始める(ACADE方式の既定動線)。
///
/// 同梱・ピン数可変・ユーザーのどれからでも複製できる。返すのは**下書き**で、
/// ファイルにはまだ書かない(保存は[`save`])。
pub fn duplicate_from(source_id: &str) -> Result<SymbolDef> {
    duplicate_from_in(user_dir().as_deref(), source_id)
}

/// ディレクトリを明示して複製する(テスト・組み込み用)。
pub fn duplicate_from_in(dir: Option<&Path>, source_id: &str) -> Result<SymbolDef> {
    let library = list_from(dir);
    let source = builtin_symbols()
        .into_iter()
        .find(|s| s.id == source_id)
        .or_else(|| dynamic_symbol(source_id))
        .or_else(|| library.symbols.iter().find(|s| s.id == source_id).cloned())
        .ok_or_else(|| CoreError::SymbolNotFound(source_id.to_string()))?;

    let taken = |id: &str| {
        builtin_symbols().iter().any(|s| s.id == id)
            || dynamic_symbol(id).is_some()
            || library.symbols.iter().any(|s| s.id == id)
    };
    let base = format!("{}_copy", source.id);
    let mut id = base.clone();
    let mut n = 2;
    while taken(&id) {
        id = format!("{base}{n}");
        n += 1;
    }

    let mut out = normalize(&source);
    out.id = id;
    out.name = format!("{} (copy)", out.name);
    out.name_ja = format!("{}のコピー", out.name_ja);
    Ok(out)
}

/// このシンボルidを使っている配置の数(プロジェクト全体)。
/// 削除の前に「まだ使われている」と知らせるために使う。
pub fn usage_count(project: &Project, symbol_id: &str) -> usize {
    project
        .sheets
        .iter()
        .flat_map(|s| s.entities.values())
        .filter(|e| matches!(e, Entity::Symbol(s) if s.symbol_id == symbol_id))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;
    use crate::model::{Sheet, SymbolInstance};
    use crate::symbol::{PinDir, TextSlot};

    fn tmp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "madake-symlib-{}-{tag}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    fn pin(number: &str, x: f64, y: f64) -> PinDef {
        PinDef {
            number: number.into(),
            name: String::new(),
            at: Point::new(x, y),
            dir: PinDir::Left,
            contacts: Vec::new(),
        }
    }

    fn sample() -> SymbolDef {
        SymbolDef {
            id: "my_contactor".into(),
            name: "My contactor".into(),
            name_ja: "自社コンタクタ".into(),
            category: "relay".into(),
            ref_prefix: "K".into(),
            keywords: vec!["contactor".into(), "電磁接触器".into()],
            primitives: vec![Primitive::Rect {
                p1: Point::new(-5.0, -5.0),
                p2: Point::new(5.0, 5.0),
                filled: false,
            }],
            pins: vec![pin("1", -5.0, -2.5), pin("2", 5.0, -2.5)],
            text_slots: vec![TextSlot {
                key: SLOT_TAG.into(),
                at: Point::new(0.0, -7.5),
                height: 2.5,
            }],
        }
    }

    /// A folder with no symbol files yields an empty user library that still reports where the user should drop their JSON files.
    /// シンボルのファイルが1つも無いフォルダではユーザーシンボルは0件になり、「JSONをここへ置く」置き場のパスだけが返る。
    #[test]
    fn an_empty_symbols_folder_lists_nothing_but_its_path() {
        let dir = tmp("empty");
        let library = list_from(Some(&dir));
        assert!(library.symbols.is_empty());
        assert!(library.issues.is_empty());
        assert_eq!(library.user_dir.as_deref(), Some(dir.display().to_string().as_str()));
    }

    /// A saved user symbol is written as one JSON file named after its id and comes back from the listing unchanged (shapes, pins and attribute slots).
    /// 保存したユーザーシンボルはidの名前のJSONファイル1つになり、図形・接続点・属性スロットがそのまま一覧に戻ってくる。
    #[test]
    fn a_saved_user_symbol_round_trips_through_the_folder() {
        let dir = tmp("roundtrip");
        let (saved, path) = save_to(&dir, &sample()).expect("保存できる");
        assert_eq!(path, dir.join("my_contactor.json"));
        let library = list_from(Some(&dir));
        assert_eq!(library.symbols.len(), 1);
        let back = &library.symbols[0];
        assert_eq!(back.id, saved.id);
        assert_eq!(back.pins.len(), 2);
        assert_eq!(back.pins[1].number, "2");
        assert_eq!(back.primitives.len(), 1);
        assert_eq!(back.text_slots[0].key, SLOT_TAG);
        assert!(library.issues.is_empty());
    }

    /// Whatever category the file declares, a user symbol is always listed under the single "user" category so it appears as its own group in the insert dialog.
    /// ファイルにどの分類が書かれていても、ユーザーシンボルの分類は常に「user」になり、部品挿入ダイアログでは独立した「ユーザー」グループに並ぶ。
    #[test]
    fn a_user_symbol_is_always_filed_under_the_user_category() {
        let dir = tmp("category");
        let (saved, _) = save_to(&dir, &sample()).expect("保存できる");
        assert_eq!(saved.category, USER_CATEGORY);
        assert_eq!(list_from(Some(&dir)).symbols[0].category, USER_CATEGORY);
    }

    /// The insert list is the bundled library first and the user symbols after it, so the user's own group never breaks the bundled category order.
    /// 部品挿入の一覧は「同梱が先・ユーザーが後」の並びになり、ユーザーの記号が同梱の分類の並びに割り込まない。
    #[test]
    fn the_insert_list_puts_user_symbols_after_the_bundled_ones() {
        let dir = tmp("merge");
        save_to(&dir, &sample()).expect("保存できる");
        let all = all_with(&list_from(Some(&dir)));
        assert_eq!(all.len(), builtin_symbols().len() + 1);
        assert_eq!(all.last().expect("最後").id, "my_contactor");
        assert!(all[..all.len() - 1].iter().all(|s| s.category != USER_CATEGORY));
    }

    /// A file that is not valid JSON does not stop the listing: the rest of the library still loads and the broken file is reported with its path and the reason.
    /// JSONとして壊れているファイルがあっても一覧は止まらず、残りのシンボルは読み込まれ、壊れたファイルはパスと理由つきで報告される。
    #[test]
    fn a_broken_json_file_is_reported_with_its_path_and_reason() {
        let dir = tmp("broken");
        save_to(&dir, &sample()).expect("保存できる");
        std::fs::write(dir.join("aaa_broken.json"), "{ これはJSONではない").expect("write");
        let library = list_from(Some(&dir));
        assert_eq!(library.symbols.len(), 1, "壊れていないシンボルは読める");
        assert_eq!(library.issues.len(), 1);
        assert!(library.issues[0].path.ends_with("aaa_broken.json"));
        assert!(!library.issues[0].message.is_empty());
    }

    /// A file whose content is valid JSON but not a usable symbol (here: a pin off the 2.5 mm grid) is reported as an issue instead of being listed.
    /// JSONとしては読めても使えないシンボル(ここでは2.5mmグリッドから外れた接続点)は、一覧に載らず理由つきで報告される。
    #[test]
    fn a_symbol_file_that_fails_validation_is_reported_instead_of_listed() {
        let dir = tmp("invalid-file");
        let mut def = sample();
        def.pins[0].at = Point::new(-5.1, -2.5);
        std::fs::write(
            dir.join("my_contactor.json"),
            serde_json::to_string_pretty(&def).unwrap(),
        )
        .expect("write");
        let library = list_from(Some(&dir));
        assert!(library.symbols.is_empty());
        assert_eq!(library.issues.len(), 1);
        assert!(library.issues[0].message.contains("グリッド"));
    }

    /// A user symbol may not take the id of a bundled symbol: otherwise dropping a file would silently change every drawing that already uses that symbol.
    /// ユーザーシンボルは同梱シンボルと同じidを名乗れない。許すと、ファイルを置いただけで、そのシンボルを使っている既存図面の見た目が黙って変わってしまうため。
    #[test]
    fn a_user_symbol_may_not_take_the_id_of_a_bundled_symbol() {
        let mut def = sample();
        def.id = "relay_coil".into();
        let errors = validate(&normalize(&def));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, code::ID_RESERVED);
    }

    /// A user symbol may not take the id of a parametric symbol either (for example terminal_block_8p), because that name is generated on demand.
    /// ピン数可変シンボル(例 terminal_block_8p)のidも名乗れない。その名前は必要に応じて自動生成されるものだから。
    #[test]
    fn a_user_symbol_may_not_take_a_parametric_symbol_id() {
        let mut def = sample();
        def.id = "terminal_block_8p".into();
        let errors = validate(&normalize(&def));
        assert_eq!(errors[0].code, code::ID_RESERVED);
    }

    /// Two files declaring the same id keep the first one (file name order) and report the second, so which symbol wins never depends on the order the folder is read in.
    /// 同じidのファイルが2つあるときは先のファイル(ファイル名順)を採用し、後のファイルは理由つきで報告する。どちらが勝つかがフォルダの読み順で変わらないようにするため。
    #[test]
    fn two_files_with_the_same_id_keep_the_first_and_report_the_second() {
        let dir = tmp("dup-id");
        let a = sample();
        let mut b = sample();
        b.name_ja = "あとから置いたほう".into();
        std::fs::write(dir.join("a.json"), serde_json::to_string(&a).unwrap()).unwrap();
        std::fs::write(dir.join("b.json"), serde_json::to_string(&b).unwrap()).unwrap();
        let library = list_from(Some(&dir));
        assert_eq!(library.symbols.len(), 1);
        assert_eq!(library.symbols[0].name_ja, "自社コンタクタ");
        assert_eq!(library.issues.len(), 1);
        assert!(library.issues[0].message.contains("重複"));
    }

    /// Saving reports every problem at once and writes no file at all, so a half-fixed symbol never lands in the library.
    /// 保存は問題を一度に全部返し、ファイルは1つも書かない。直しかけの定義がライブラリに残らないようにするため。
    #[test]
    fn saving_an_invalid_symbol_reports_every_problem_and_writes_nothing() {
        let dir = tmp("all-errors");
        let mut def = sample();
        def.primitives.clear();
        def.pins[0].at = Point::new(-5.1, -2.5);
        def.text_slots.clear();
        let errors = save_to(&dir, &def).expect_err("保存できない");
        let codes: Vec<&str> = errors.iter().map(|e| e.code.as_str()).collect();
        assert!(codes.contains(&code::NO_PRIMITIVES), "{codes:?}");
        assert!(codes.contains(&code::PIN_OFF_GRID), "{codes:?}");
        assert!(codes.contains(&code::SLOT_MISSING), "{codes:?}");
        assert!(!dir.join("my_contactor.json").exists(), "ファイルは書かれない");
    }

    /// Connection points must sit on the 2.5 mm grid, so wires drawn on the grid always meet them exactly.
    /// 接続点は2.5mmグリッドの上に無ければならない。グリッドに沿って引いた配線がぴったり接続するため。
    #[test]
    fn pins_must_sit_on_the_grid() {
        let mut def = sample();
        def.pins[1].at = Point::new(5.0, -1.0);
        let errors = validate(&normalize(&def));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, code::PIN_OFF_GRID);
        assert!(errors[0].message.contains('2'), "{}", errors[0].message);
    }

    /// The same pin number at two different points is allowed: that is how a through terminal (internally shorted left and right side) is drawn.
    /// 同じ番号の接続点を離れた2点に置くのは正しい。貫通端子(左右が内部で短絡している端子)はこう描くため。
    #[test]
    fn the_same_pin_number_at_two_points_is_a_through_terminal() {
        let mut def = sample();
        def.pins = vec![pin("1", -2.5, 0.0), pin("1", 2.5, 0.0)];
        assert_eq!(validate(&normalize(&def)), vec![]);
    }

    /// Two connection points with the same number at the same place are rejected: the second one can never be reached.
    /// 同じ番号の接続点が同じ位置に2つあるのは拒否する(2つ目には決して接続できないため)。
    #[test]
    fn the_same_pin_number_at_the_same_point_is_rejected() {
        let mut def = sample();
        def.pins = vec![pin("1", -2.5, 0.0), pin("1", -2.5, 0.0)];
        let errors = validate(&normalize(&def));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, code::PIN_DUPLICATE);
    }

    /// Two connection points with different numbers at the same place are rejected: a wire ending there would not know which terminal it touches.
    /// 違う番号の接続点が同じ位置に重なっているのは拒否する(そこへ引いた配線がどちらの端子に繋がったのか決まらないため)。
    #[test]
    fn two_different_pin_numbers_at_the_same_point_are_rejected() {
        let mut def = sample();
        def.pins = vec![pin("1", -2.5, 0.0), pin("2", -2.5, 0.0)];
        let errors = validate(&normalize(&def));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, code::PIN_OVERLAP);
    }

    /// A connection point needs a number (it is what the netlist and the terminal chart print), but its name is optional.
    /// 接続点には番号が必要(ネットリストや端子台チャートに出るのはこの番号)。名前は空でもよい。
    #[test]
    fn a_pin_needs_a_number_but_not_a_name() {
        let mut def = sample();
        def.pins[0].number = "  ".into();
        let errors = validate(&normalize(&def));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, code::PIN_NUMBER_EMPTY);

        let mut named = sample();
        named.pins[0].name = String::new();
        assert_eq!(validate(&normalize(&named)), vec![]);
    }

    /// A symbol must carry the reference-designator slot (TAG): without it the placed part could never show its K1 / PB2 label.
    /// シンボルには参照記号のスロット(TAG)が必須。無いと、配置した部品にK1・PB2といった記号を表示できないため。
    #[test]
    fn a_symbol_without_a_tag_slot_is_rejected() {
        let mut def = sample();
        def.text_slots.clear();
        let errors = validate(&normalize(&def));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, code::SLOT_MISSING);
    }

    /// An attribute slot key the renderer does not know (a typo, say) is rejected instead of silently drawing nothing.
    /// 描画側が知らない属性スロットのキー(打ち間違いなど)は、黙って何も描かれるより先に拒否する。
    #[test]
    fn an_unknown_attribute_slot_key_is_rejected() {
        let mut def = sample();
        def.text_slots.push(TextSlot {
            key: "MFG".into(),
            at: Point::new(0.0, 10.0),
            height: 2.5,
        });
        let errors = validate(&normalize(&def));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, code::SLOT_UNKNOWN);
    }

    /// A symbol needs at least one shape and at least one connection point, otherwise there is nothing to see and nothing to wire.
    /// シンボルには図形が最低1つ・接続点が最低1つ必要。無ければ何も見えないし、配線もできないため。
    #[test]
    fn a_symbol_needs_at_least_one_shape_and_one_pin() {
        let mut def = sample();
        def.primitives.clear();
        def.pins.clear();
        let codes: Vec<String> = validate(&normalize(&def)).into_iter().map(|e| e.code).collect();
        assert!(codes.contains(&code::NO_PRIMITIVES.to_string()));
        assert!(codes.contains(&code::NO_PINS.to_string()));
    }

    /// The id is used as the file name, so only letters, digits, underscore and hyphen are allowed.
    /// idはそのままファイル名になるので、使えるのは英数字・アンダースコア・ハイフンだけ。
    #[test]
    fn an_id_with_unusable_characters_is_rejected() {
        let mut def = sample();
        def.id = "my/contactor 3P".into();
        let errors = validate(&normalize(&def));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, code::ID_INVALID);
    }

    /// A symbol named only in Japanese is accepted and the Japanese name is copied into the English field, so it still shows up in an English UI.
    /// 日本語だけで名前を付けたシンボルも保存でき、英語名には日本語名がコピーされる(英語UIでも名前が出るようにするため)。
    #[test]
    fn a_japanese_only_name_fills_both_name_fields() {
        let mut def = sample();
        def.name = String::new();
        let out = normalize(&def);
        assert_eq!(out.name, "自社コンタクタ");
        assert_eq!(out.name_ja, "自社コンタクタ");
        assert_eq!(validate(&out), vec![]);
    }

    /// Duplicating a bundled symbol gives a draft with a new free id, a "copy" name and the user category, and writes nothing to disk yet.
    /// 同梱シンボルを複製すると、空いている新しいid・「コピー」の名前・ユーザー分類を持った下書きが返る(この時点ではファイルには書かない)。
    #[test]
    fn duplicating_a_bundled_symbol_gives_a_free_id_and_a_copy_name() {
        let dir = tmp("dup");
        let copy = duplicate_from_in(Some(&dir), "relay_coil").expect("複製できる");
        assert_eq!(copy.id, "relay_coil_copy");
        assert!(copy.name_ja.ends_with("のコピー"), "{}", copy.name_ja);
        assert!(copy.name.ends_with("(copy)"), "{}", copy.name);
        assert_eq!(copy.category, USER_CATEGORY);
        assert_eq!(copy.pins.len(), resolve_source_pins("relay_coil"));
        assert!(!dir.join("relay_coil_copy.json").exists(), "保存はまだしない");
        assert_eq!(validate(&copy), vec![], "複製した下書きはそのまま保存できる");
    }

    fn resolve_source_pins(id: &str) -> usize {
        builtin_symbols().iter().find(|s| s.id == id).expect("同梱").pins.len()
    }

    /// Duplicating the same symbol twice never reuses an id: the second copy is numbered.
    /// 同じシンボルを2回複製してもidは重複しない(2つ目には番号が付く)。
    #[test]
    fn duplicating_twice_never_reuses_an_id() {
        let dir = tmp("dup2");
        let first = duplicate_from_in(Some(&dir), "relay_coil").expect("複製できる");
        save_to(&dir, &first).expect("保存できる");
        let second = duplicate_from_in(Some(&dir), "relay_coil").expect("複製できる");
        assert_eq!(second.id, "relay_coil_copy2");
    }

    /// A user symbol can be duplicated too (that is how a family of variants is built), and a parametric symbol can be the starting point as well.
    /// ユーザーシンボルも複製できる(これで似た記号の一族を作る)。ピン数可変シンボルからも始められる。
    #[test]
    fn a_user_symbol_and_a_parametric_symbol_can_both_be_duplicated() {
        let dir = tmp("dup-user");
        save_to(&dir, &sample()).expect("保存できる");
        let from_user = duplicate_from_in(Some(&dir), "my_contactor").expect("複製できる");
        assert_eq!(from_user.id, "my_contactor_copy");
        let from_dynamic = duplicate_from_in(Some(&dir), "terminal_block_4p").expect("複製できる");
        assert_eq!(from_dynamic.id, "terminal_block_4p_copy");
        assert_eq!(from_dynamic.pins.len(), 8, "貫通端子は4極で8接続点");
    }

    /// Duplicating a symbol that does not exist fails instead of creating an empty one.
    /// 存在しないシンボルの複製は、空のシンボルを作らずにエラーになる。
    #[test]
    fn duplicating_an_unknown_symbol_fails() {
        let dir = tmp("dup-unknown");
        assert!(duplicate_from_in(Some(&dir), "no_such_symbol").is_err());
    }

    /// Deleting a user symbol removes its file from the folder; deleting an unknown id fails and touches nothing.
    /// ユーザーシンボルを削除するとフォルダからファイルが消える。知らないidの削除はエラーになり、何も消えない。
    #[test]
    fn deleting_a_user_symbol_removes_its_file() {
        let dir = tmp("delete");
        save_to(&dir, &sample()).expect("保存できる");
        let path = delete_from(&dir, "my_contactor").expect("削除できる");
        assert!(!path.exists());
        assert!(list_from(Some(&dir)).symbols.is_empty());
        assert!(delete_from(&dir, "my_contactor").is_err(), "2回目は失敗する");
        assert!(delete_from(&dir, "relay_coil").is_err(), "同梱シンボルは消せない");
    }

    /// The usage count tells how many placements of a symbol the project has, across every sheet, so the UI can warn before a symbol is deleted.
    /// 使用数は、そのシンボルがプロジェクト全体(全シート)で何個置かれているかを返す。削除の前に警告を出すために使う。
    #[test]
    fn the_usage_count_counts_placements_across_every_sheet() {
        let mut project = Project::new("使用数テスト");
        project.sheets.push(Sheet::new(
            "2",
            crate::model::PaperSize::A3,
            crate::model::Orientation::Landscape,
        ));
        for i in 0..3 {
            let sheet = &mut project.sheets[i % 2];
            let id = uuid::Uuid::new_v4();
            sheet.entities.insert(
                id,
                Entity::Symbol(SymbolInstance {
                    id,
                    symbol_id: "my_contactor".into(),
                    at: Point::new(10.0, 10.0),
                    rotation: 0,
                    mirror: false,
                    reference: format!("K{}", i + 1),
                    value: String::new(),
                    attrs: Default::default(),
                }),
            );
        }
        assert_eq!(usage_count(&project, "my_contactor"), 3);
        assert_eq!(usage_count(&project, "relay_coil"), 0);
    }
}
