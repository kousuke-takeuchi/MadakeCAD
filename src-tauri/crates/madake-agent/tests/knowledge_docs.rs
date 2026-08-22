//! 同梱ドキュメント(docs/)の実ファイル解決テスト。
//!
//! `MADAKE_DOCS_PATH`はプロセス全体に効くため、このバイナリではテストを1本だけ持つ
//! (他のテストと並行に走らせない)。Tauriは起動時にドキュメントの実体パスをここへ入れる。

use madake_agent::knowledge::{docs_dir, system_prompt, DOCS_PATH_ENV};
use madake_agent::AppSettings;
use uuid::Uuid;

/// The agent is pointed at the documentation folder the app resolved, so it can read the manual wherever it is installed.
/// エージェントにはアプリが解決したドキュメントの場所が伝わり、どこにインストールされていてもマニュアルを読める。
#[test]
fn the_documentation_folder_of_the_installed_app_is_handed_to_the_agent() {
    let dir = std::env::temp_dir().join(format!("madake_docs_env_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("01-overview.md"), "# Overview\n").unwrap();

    std::env::set_var(DOCS_PATH_ENV, &dir);
    assert_eq!(docs_dir().as_deref(), Some(dir.as_path()));
    let prompt = system_prompt(&AppSettings::default(), None);
    assert!(prompt.contains(&dir.display().to_string()), "{prompt}");

    std::env::remove_var(DOCS_PATH_ENV);
    std::fs::remove_dir_all(&dir).ok();
}
