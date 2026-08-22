use std::path::Path;

use crate::model::Project;
use crate::Result;

/// プロジェクトを整形JSONで保存する(git差分が読める形式)。
pub fn save_project(path: &Path, project: &Project) -> Result<()> {
    let json = serde_json::to_string_pretty(project)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// プロジェクトを読み込む。古い形式のファイルは現行形式へ移行してから返す ([`migrate`])。
pub fn load_project(path: &Path) -> Result<Project> {
    let json = std::fs::read_to_string(path)?;
    let mut project: Project = serde_json::from_str(&json)?;
    migrate(&mut project);
    Ok(project)
}

/// 旧形式のプロジェクトを現行形式へ移行する。
///
/// 追加されたフィールドはserdeの既定値 (空リスト等) で埋まるので、ここでやることは
/// 版番号を現行へ上げることだけ。**フィールドの意味を変える変更を入れるときは、
/// ここに版ごとの変換を書く**。
pub fn migrate(project: &mut Project) {
    if project.format_version < crate::model::FORMAT_VERSION {
        project.format_version = crate::model::FORMAT_VERSION;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{Command, Engine};
    use crate::geometry::Point;
    use crate::model::*;
    use uuid::Uuid;

    /// A project saved to .mdkproj and loaded back is identical, including all entities.
    /// .mdkprojへ保存して読み直したプロジェクトは、全エンティティを含めて同一である。
    #[test]
    fn project_file_roundtrip() {
        let mut engine = Engine::new(Project::new("roundtrip"));
        let sheet_id = engine.project().sheets[0].id;
        engine
            .execute(Command::AddEntity {
                sheet_id,
                entity: Entity::Symbol(SymbolInstance {
                    id: Uuid::new_v4(),
                    symbol_id: "relay_coil".into(),
                    at: Point::new(100.0, 50.0),
                    rotation: 90,
                    mirror: false,
                    reference: "K1".into(),
                    value: "JZX-22F".into(),
                    attrs: Default::default(),
                }),
            })
            .unwrap();

        let dir = std::env::temp_dir().join("madake_core_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("roundtrip.mdkproj");
        save_project(&path, engine.project()).unwrap();
        let loaded = load_project(&path).unwrap();
        assert_eq!(loaded.name, "roundtrip");
        assert_eq!(loaded.sheets.len(), 1);
        assert_eq!(loaded.sheets[0].entities.len(), 1);
        std::fs::remove_file(&path).ok();
    }

    /// The saved .mdkproj file is pretty-printed JSON with a format_version field, so it diffs well in git.
    /// 保存された.mdkprojはformat_version付きの整形JSONで、gitの差分が読みやすい。
    #[test]
    fn saved_file_is_pretty_json_with_format_version() {
        let engine = Engine::new(Project::new("pretty"));
        let dir = std::env::temp_dir().join(format!("madake-io-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("p.mdkproj");
        save_project(&path, engine.project()).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains(&format!("\n  \"format_version\": {FORMAT_VERSION}")),
            "{text}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Loading a missing file returns an error instead of panicking.
    /// 存在しないファイルの読み込みはパニックせずエラーを返す。
    #[test]
    fn loading_missing_file_is_an_error() {
        assert!(load_project(std::path::Path::new("/nonexistent/x.mdkproj")).is_err());
    }

    /// A project file saved before the PLC assignment table existed still opens: it gets an empty table and is brought up to the current format version.
    /// PLC割付表が無かった頃の古いプロジェクトファイルもそのまま開ける (割付表は空になり、現行のファイル形式へ更新される)。
    #[test]
    fn an_old_project_file_opens_with_an_empty_plc_assignment_table() {
        let dir = std::env::temp_dir().join(format!("madake-io-old-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("old.mdkproj");
        // format_version 1の頃のファイル (plc_assignmentsが無い)
        std::fs::write(
            &path,
            r#"{"format_version":1,"name":"旧図面","sheets":[],"wire_parts":[]}"#,
        )
        .unwrap();
        let loaded = load_project(&path).unwrap();
        assert_eq!(loaded.name, "旧図面");
        assert!(loaded.plc_assignments.is_empty());
        assert_eq!(loaded.format_version, FORMAT_VERSION, "現行形式へ更新される");
        std::fs::remove_dir_all(&dir).ok();
    }
}
