//! 開始テンプレート (M3仕様 §2): Command列のJSONを**1回の実行**で図面へ入れる。
//!
//! テンプレートは編集可能なJSONファイルとして同梱する。形式は:
//!
//! ```json
//! {
//!   "id": "control_24v_basic",
//!   "name": "24 V control basics",
//!   "name_ja": "24V制御基本",
//!   "description": "...",
//!   "description_ja": "...",
//!   "commands": [ { "type": "add_entity", "sheet_id": "00000000-...", "entity": { … } } ]
//! }
//! ```
//!
//! コマンドの中のUUIDは**プレースホルダ**として扱う:
//!
//! - nil UUID (`00000000-0000-0000-0000-000000000000`) = 適用先シートのid
//! - それ以外のUUID = 適用のたびに新しいidへ振り直す(同じテンプレートを何度でも
//!   適用でき、id衝突で失敗しない)
//!
//! 適用は[`Engine::execute_batch`]なので、**undo一発**でテンプレート全体が戻る。
//!
//! ファイルの探索順(3段解決。規格ノートと同じ考え方):
//!
//! 1. 環境変数[`TEMPLATES_PATH_ENV`](配布時にTauriがリソースの実体を入れる)
//! 2. リポジトリの`src-tauri/resources/templates`(開発時)
//! 3. どちらも無ければビルド時に埋め込んだ同梱テンプレート(常に3種が使える)
//!
//! これに加えてユーザーのテンプレート(`~/MadakeCAD/templates/*.json`)も列挙する。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::command::{Command, EditOrigin, Engine, Patch};
use crate::model::SheetId;
use crate::{CoreError, Result};

/// 同梱テンプレートの実ディレクトリを与える環境変数(Tauriのリソース解決・テスト用)。
pub const TEMPLATES_PATH_ENV: &str = "MADAKE_TEMPLATES_PATH";
/// ユーザーテンプレートの置き場を差し替える環境変数(既定は`~/MadakeCAD/templates`)。
pub const USER_TEMPLATES_PATH_ENV: &str = "MADAKE_USER_TEMPLATES_PATH";

/// 開発時(リポジトリで動かすとき)の同梱テンプレートの位置。
const REPO_TEMPLATES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../resources/templates");

/// 実ファイルが読めないときのフォールバック(ビルド時に埋め込む同内容)。並びは表示順。
const BUNDLED: &[&str] = &[
    include_str!("../../../resources/templates/01-control-24v-basic.json"),
    include_str!("../../../resources/templates/02-motor-starter.json"),
    include_str!("../../../resources/templates/03-emergency-stop.json"),
];

/// 開始テンプレート1件。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    /// 安定ID (例: "control_24v_basic")。適用時の指定に使う。
    pub id: String,
    /// 英語名 (UI言語がenのとき表示)。
    pub name: String,
    /// 日本語名。
    pub name_ja: String,
    /// 英語の説明 (何が入るか)。
    #[serde(default)]
    pub description: String,
    /// 日本語の説明。
    #[serde(default)]
    pub description_ja: String,
    /// 適用するCommand列 (UUIDはプレースホルダ。モジュールのドキュメント参照)。
    pub commands: Vec<serde_json::Value>,
    /// 同梱テンプレートならtrue、ユーザーテンプレートならfalse。
    /// JSONには書かず、読み込んだ場所から決まる。
    #[serde(default, skip_deserializing)]
    pub builtin: bool,
}

/// 読み込めなかったテンプレートファイル1件(壊れたJSON・未知のコマンド)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateIssue {
    /// 問題のあったファイルのパス。
    pub path: String,
    /// 理由(JSONのパースエラー等)。
    pub message: String,
}

/// テンプレートの一覧と、読み込めなかったファイルの理由。
///
/// 壊れたファイルが1つあっても残りは使えるように、**列挙は止めずに理由を持ち帰る**。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TemplateList {
    pub templates: Vec<Template>,
    pub issues: Vec<TemplateIssue>,
    /// ユーザーテンプレートの置き場 (UIが「ここにJSONを置けば並ぶ」と案内する)。
    #[serde(default)]
    pub user_dir: Option<String>,
}

impl Template {
    /// 適用先シート向けにUUIDを差し替えたCommand列を組み立てる。
    pub fn commands_for(&self, sheet_id: SheetId) -> Result<Vec<Command>> {
        let mut value = serde_json::Value::Array(self.commands.clone());
        let mut remap: BTreeMap<Uuid, Uuid> = BTreeMap::new();
        substitute(&mut value, sheet_id, &mut remap);
        Ok(serde_json::from_value(value)?)
    }
}

/// JSON中のUUID文字列を差し替える (nil=適用先シート、それ以外=新しいidへ振り直し)。
fn substitute(value: &mut serde_json::Value, sheet_id: SheetId, remap: &mut BTreeMap<Uuid, Uuid>) {
    match value {
        serde_json::Value::String(s) => {
            if let Ok(id) = Uuid::parse_str(s) {
                let new = if id.is_nil() {
                    sheet_id
                } else {
                    *remap.entry(id).or_insert_with(Uuid::new_v4)
                };
                *s = new.to_string();
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                substitute(item, sheet_id, remap);
            }
        }
        serde_json::Value::Object(map) => {
            for item in map.values_mut() {
                substitute(item, sheet_id, remap);
            }
        }
        _ => {}
    }
}

/// 同梱テンプレートのディレクトリ (環境変数 → リポジトリの順)。どちらも無ければNone。
pub fn bundled_dir() -> Option<PathBuf> {
    let from_env = std::env::var_os(TEMPLATES_PATH_ENV).map(PathBuf::from);
    [from_env, Some(PathBuf::from(REPO_TEMPLATES))]
        .into_iter()
        .flatten()
        .find(|p| p.is_dir())
}

/// ユーザーテンプレートの置き場 (`~/MadakeCAD/templates`)。ホームが分からなければNone。
pub fn user_dir() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os(USER_TEMPLATES_PATH_ENV) {
        return Some(PathBuf::from(p));
    }
    dirs::home_dir().map(|h| h.join("MadakeCAD").join("templates"))
}

/// 使えるテンプレートを列挙する(同梱 → ユーザーの順)。
///
/// 同じidのユーザーテンプレートは同梱のものを差し替える(上書きできる)。
pub fn list() -> TemplateList {
    list_from(bundled_dir().as_deref(), user_dir().as_deref())
}

/// ディレクトリを明示して列挙する(テスト・組み込み用)。
///
/// `bundled`がNone(見つからない)のときは、ビルド時に埋め込んだ同梱テンプレートを使う。
pub fn list_from(bundled: Option<&Path>, user: Option<&Path>) -> TemplateList {
    let mut out = TemplateList::default();
    match bundled {
        Some(dir) => read_dir_templates(dir, true, &mut out),
        None => {
            for text in BUNDLED {
                match parse(text) {
                    Ok(mut t) => {
                        t.builtin = true;
                        push(&mut out.templates, t);
                    }
                    Err(e) => out.issues.push(TemplateIssue {
                        path: "<bundled>".into(),
                        message: e.to_string(),
                    }),
                }
            }
        }
    }
    if let Some(dir) = user {
        out.user_dir = Some(dir.display().to_string());
        read_dir_templates(dir, false, &mut out);
    }
    out
}

/// 1つのテンプレートをidで取得する。
pub fn find(id: &str) -> Option<Template> {
    list().templates.into_iter().find(|t| t.id == id)
}

/// テンプレートをidで適用する。**undo一発**で全体が戻る1件の編集になる。
///
/// 未知のidは[`CoreError::InvalidCommand`]、存在しないシートは
/// [`CoreError::SheetNotFound`]。
pub fn apply(
    engine: &mut Engine,
    id: &str,
    sheet_id: SheetId,
    origin: EditOrigin,
) -> Result<Patch> {
    let template = find(id)
        .ok_or_else(|| CoreError::InvalidCommand(format!("unknown template: {id}")))?;
    apply_template(engine, &template, sheet_id, origin)
}

/// 読み込み済みのテンプレートを適用する。
pub fn apply_template(
    engine: &mut Engine,
    template: &Template,
    sheet_id: SheetId,
    origin: EditOrigin,
) -> Result<Patch> {
    if engine.project().sheet(sheet_id).is_none() {
        return Err(CoreError::SheetNotFound(sheet_id));
    }
    let commands = template.commands_for(sheet_id)?;
    engine.execute_batch(commands, origin)
}

/// ディレクトリ内の`*.json`をファイル名順に読む。無いディレクトリは静かに無視する
/// (ユーザーテンプレートを1つも置いていないのは正常な状態)。
fn read_dir_templates(dir: &Path, builtin: bool, out: &mut TemplateList) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    paths.sort();
    for path in paths {
        let loaded = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| parse(&text).map_err(|e| e.to_string()));
        match loaded {
            Ok(mut template) => {
                template.builtin = builtin;
                push(&mut out.templates, template);
            }
            Err(message) => out.issues.push(TemplateIssue {
                path: path.display().to_string(),
                message,
            }),
        }
    }
}

/// JSONを読み、コマンド列が本当にCommandとして読めるかまで確かめる。
fn parse(text: &str) -> serde_json::Result<Template> {
    let template: Template = serde_json::from_str(text)?;
    // 適用時ではなく読み込み時に弾く: 壊れたテンプレートを一覧に出さない
    let _: Vec<Command> = serde_json::from_value(serde_json::Value::Array(
        template.commands.clone(),
    ))?;
    Ok(template)
}

/// 同じidが既にあれば差し替え、無ければ末尾へ足す。
fn push(templates: &mut Vec<Template>, template: Template) {
    match templates.iter_mut().find(|t| t.id == template.id) {
        Some(slot) => *slot = template,
        None => templates.push(template),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Entity, Project};
    use crate::verify::{verify_project, Severity};

    fn engine() -> (Engine, SheetId) {
        let project = Project::new("test");
        let sheet_id = project.sheets[0].id;
        (Engine::new(project), sheet_id)
    }

    fn apply_by_id(engine: &mut Engine, id: &str, sheet_id: SheetId) {
        apply(engine, id, sheet_id, EditOrigin::User).expect("テンプレートの適用");
    }

    /// The three bundled templates (24 V control basics, motor starter, emergency stop) are listed in that order, each with an English and a Japanese name and description.
    /// 同梱テンプレートは「24V制御基本・モータ起動回路・非常停止回路」の3種がこの順で並び、それぞれ英語と日本語の名前・説明を持つ。
    #[test]
    fn the_three_bundled_templates_are_listed_in_both_languages() {
        let list = list();
        assert!(list.issues.is_empty(), "同梱テンプレは全て読める: {:?}", list.issues);
        let ids: Vec<&str> = list.templates.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["control_24v_basic", "motor_starter", "emergency_stop"]
        );
        for t in &list.templates {
            assert!(t.builtin, "{} は同梱テンプレート", t.id);
            assert!(!t.name.is_empty() && !t.name_ja.is_empty(), "{}", t.id);
            assert!(
                !t.description.is_empty() && !t.description_ja.is_empty(),
                "{}",
                t.id
            );
            assert!(!t.commands.is_empty(), "{}", t.id);
        }
        assert_eq!(list.templates[0].name_ja, "24V制御基本");
        assert_eq!(list.templates[1].name_ja, "モータ起動回路");
        assert_eq!(list.templates[2].name_ja, "非常停止回路");
    }

    /// Even without the resource directory, the templates bundled into the build are still available.
    /// リソースのディレクトリが無い環境でも、ビルドへ埋め込んだ同梱テンプレートが使える。
    #[test]
    fn templates_are_available_without_the_resource_directory() {
        let list = list_from(None, None);
        assert_eq!(list.templates.len(), 3);
        assert!(list.issues.is_empty());
        assert_eq!(list.templates[0].id, "control_24v_basic");
    }

    /// Applying "24 V control basics" puts its parts on the sheet: the DC source, the fuse, the 4-pole terminal block and the 24V/0V net labels.
    /// 「24V制御基本」を適用すると、直流電源・ヒューズ・4極端子台と24V/0Vのネットラベルが図面に入る。
    #[test]
    fn applying_the_24v_template_places_its_parts_on_the_sheet() {
        let (mut engine, sheet_id) = engine();
        apply_by_id(&mut engine, "control_24v_basic", sheet_id);

        let sheet = engine.project().sheet(sheet_id).expect("シート");
        let symbols: Vec<(&str, &str)> = sheet
            .entities
            .values()
            .filter_map(|e| match e {
                Entity::Symbol(s) => Some((s.symbol_id.as_str(), s.reference.as_str())),
                _ => None,
            })
            .collect();
        assert!(symbols.contains(&("battery", "BT1")), "{symbols:?}");
        assert!(symbols.contains(&("fuse", "F1")), "{symbols:?}");
        assert!(symbols.contains(&("terminal_block_4p", "TB1")), "{symbols:?}");

        let mut labels: Vec<&str> = sheet
            .entities
            .values()
            .filter_map(|e| match e {
                Entity::NetLabel(l) => Some(l.name.as_str()),
                _ => None,
            })
            .collect();
        labels.sort();
        assert_eq!(labels, vec!["0V", "24V"]);
    }

    /// A template is applied as one edit: a single undo empties the drawing again, and a single redo brings the whole template back.
    /// テンプレートの適用は1回の編集なので、undo一発で図面が空に戻り、redo一発で全部戻ってくる。
    #[test]
    fn applying_a_template_is_undone_in_one_step() {
        for id in ["control_24v_basic", "motor_starter", "emergency_stop"] {
            let (mut engine, sheet_id) = engine();
            apply_by_id(&mut engine, id, sheet_id);
            let placed = engine.project().sheet(sheet_id).unwrap().entities.len();
            assert!(placed > 5, "{id}: {placed}件");
            assert_eq!(engine.undo_depth(), 1, "{id}: 履歴は1件");

            engine.undo().expect("undo").expect("戻せる");
            assert!(
                engine.project().sheet(sheet_id).unwrap().entities.is_empty(),
                "{id}: undo一発で空図面へ戻る"
            );
            engine.redo().expect("redo").expect("やり直せる");
            assert_eq!(
                engine.project().sheet(sheet_id).unwrap().entities.len(),
                placed,
                "{id}: redo一発で戻ってくる"
            );
        }
    }

    /// Every bundled template passes verification with zero errors and zero ERC warnings: they are fully wired starting points, not sketches with loose pins.
    /// 同梱テンプレートはどれも検証でエラー0・ERC警告0になる(浮いたピンの無い、そのまま使える出発点)。
    #[test]
    fn every_bundled_template_verifies_without_errors_or_erc_warnings() {
        for id in ["control_24v_basic", "motor_starter", "emergency_stop"] {
            let (mut engine, sheet_id) = engine();
            apply_by_id(&mut engine, id, sheet_id);
            let diags = verify_project(engine.project());
            let errors: Vec<&str> = diags
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .map(|d| d.message.as_str())
                .collect();
            assert!(errors.is_empty(), "{id}: エラーが残っている {errors:?}");
            let warnings: Vec<&str> = diags
                .iter()
                .filter(|d| d.severity == Severity::Warning)
                .map(|d| d.message.as_str())
                .collect();
            assert!(warnings.is_empty(), "{id}: 警告が残っている {warnings:?}");
        }
    }

    /// The same template can be applied twice: the second copy gets fresh entity ids instead of colliding with the first.
    /// 同じテンプレートは2回適用できる(2回目は新しいidが振られ、1回目と衝突しない)。
    #[test]
    fn the_same_template_can_be_applied_twice() {
        let (mut engine, sheet_id) = engine();
        apply_by_id(&mut engine, "control_24v_basic", sheet_id);
        let first = engine.project().sheet(sheet_id).unwrap().entities.len();
        apply_by_id(&mut engine, "control_24v_basic", sheet_id);
        assert_eq!(
            engine.project().sheet(sheet_id).unwrap().entities.len(),
            first * 2
        );
    }

    /// A template lands on the sheet it was asked for, even when the project has several sheets.
    /// テンプレートは複数シートのプロジェクトでも、指定したシートにだけ入る。
    #[test]
    fn a_template_lands_on_the_requested_sheet() {
        use crate::model::{Orientation, PaperSize};
        let (mut engine, first) = engine();
        engine
            .execute(Command::AddSheet {
                name: "Sheet2".into(),
                size: PaperSize::A3,
                orientation: Orientation::Landscape,
            })
            .unwrap();
        let second = engine.project().sheets[1].id;
        apply_by_id(&mut engine, "emergency_stop", second);
        assert!(engine.project().sheet(first).unwrap().entities.is_empty());
        assert!(!engine.project().sheet(second).unwrap().entities.is_empty());
    }

    /// An unknown template id is refused with an error naming the id, and the drawing is left untouched.
    /// 知らないテンプレートidはidを添えたエラーで拒否され、図面は変わらない。
    #[test]
    fn an_unknown_template_id_is_refused() {
        let (mut engine, sheet_id) = engine();
        let err = apply(&mut engine, "no_such_template", sheet_id, EditOrigin::User)
            .expect_err("未知のidはエラー");
        assert!(err.to_string().contains("no_such_template"), "{err}");
        assert!(engine.project().sheet(sheet_id).unwrap().entities.is_empty());
        assert_eq!(engine.undo_depth(), 0);
    }

    /// Templates the user drops into their own templates folder are listed after the bundled ones and can be applied the same way.
    /// ユーザーが自分のテンプレートフォルダに置いたテンプレートは同梱テンプレートの後に並び、同じように適用できる。
    #[test]
    fn user_templates_are_listed_after_the_bundled_ones() {
        let dir = std::env::temp_dir().join(format!("madake-templates-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("my-panel.json"),
            r#"{
                "id": "my_panel",
                "name": "My panel",
                "name_ja": "自社の盤",
                "description": "one terminal block",
                "description_ja": "端子台1台",
                "commands": [
                    {
                        "type": "add_entity",
                        "sheet_id": "00000000-0000-0000-0000-000000000000",
                        "entity": {
                            "kind": "symbol",
                            "id": "00000000-0000-0000-0000-000000000001",
                            "symbol_id": "terminal_block_4p",
                            "at": { "x": 100.0, "y": 100.0 },
                            "rotation": 0,
                            "mirror": false,
                            "reference": "TB9",
                            "value": "",
                            "attrs": {}
                        }
                    }
                ]
            }"#,
        )
        .unwrap();

        let list = list_from(None, Some(&dir));
        assert!(list.issues.is_empty(), "{:?}", list.issues);
        assert_eq!(
            list.user_dir.as_deref(),
            Some(dir.display().to_string().as_str()),
            "UIが案内できるようテンプレートの置き場も返す"
        );
        let mine = list.templates.last().expect("末尾はユーザーテンプレート");
        assert_eq!(mine.id, "my_panel");
        assert_eq!(mine.name_ja, "自社の盤");
        assert!(!mine.builtin, "ユーザーテンプレートとして区別される");

        let (mut engine, sheet_id) = engine();
        apply_template(&mut engine, mine, sheet_id, EditOrigin::User).unwrap();
        assert_eq!(engine.project().sheet(sheet_id).unwrap().entities.len(), 1);

        std::fs::remove_dir_all(&dir).ok();
    }

    /// A template file that is not valid JSON (or holds an unknown command) is reported with its path and reason, and the other templates stay usable.
    /// JSONとして壊れている(または知らないコマンドを含む)テンプレートは、パスと理由を添えて報告され、他のテンプレートはそのまま使える。
    #[test]
    fn a_broken_template_file_is_reported_with_its_path() {
        let dir = std::env::temp_dir().join(format!("madake-templates-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("broken.json"), "{ this is not json").unwrap();
        std::fs::write(
            dir.join("unknown-command.json"),
            r#"{ "id": "x", "name": "x", "name_ja": "x", "commands": [ { "type": "fly_away" } ] }"#,
        )
        .unwrap();

        let list = list_from(None, Some(&dir));
        assert_eq!(list.templates.len(), 3, "同梱テンプレートは残る");
        assert_eq!(list.issues.len(), 2, "壊れた2ファイルを報告する");
        let paths: Vec<&str> = list.issues.iter().map(|i| i.path.as_str()).collect();
        assert!(paths.iter().any(|p| p.ends_with("broken.json")), "{paths:?}");
        assert!(
            paths.iter().any(|p| p.ends_with("unknown-command.json")),
            "{paths:?}"
        );
        assert!(
            list.issues.iter().all(|i| !i.message.is_empty()),
            "理由を添える"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// A user template may replace a bundled one by reusing its id (the drawing office's own version wins).
    /// ユーザーテンプレートは同梱テンプレートと同じidを使うことで差し替えられる(自社版が優先される)。
    #[test]
    fn a_user_template_replaces_the_bundled_one_with_the_same_id() {
        let dir = std::env::temp_dir().join(format!("madake-templates-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("ours.json"),
            r#"{ "id": "control_24v_basic", "name": "Ours", "name_ja": "自社の24V", "commands": [] }"#,
        )
        .unwrap();

        let list = list_from(None, Some(&dir));
        assert_eq!(list.templates.len(), 3, "件数は増えない");
        assert_eq!(list.templates[0].id, "control_24v_basic");
        assert_eq!(list.templates[0].name_ja, "自社の24V");
        assert!(!list.templates[0].builtin);

        std::fs::remove_dir_all(&dir).ok();
    }
}
