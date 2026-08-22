//! 同梱規格知識の実ファイル解決テスト。
//!
//! `MADAKE_STANDARDS_PATH`はプロセス全体に効くため、このバイナリではテストを1本だけ持つ
//! (他のテストと並行に走らせない)。Tauriは起動時にリソースの実体パスをここへ入れる。

use madake_agent::knowledge::{standards_text, STANDARDS_PATH_ENV};
use uuid::Uuid;

/// The bundled standards note is read from the resource file when one is available.
/// 同梱の規格知識は、実ファイル(Tauriのリソース・開発時のパス)があればそちらから読む。
#[test]
fn the_standards_note_is_read_from_the_resource_file_when_present() {
    // 実ファイルが無い(または読めない)ときは、ビルドへ埋め込んだ内容へフォールバックする
    std::env::set_var(STANDARDS_PATH_ENV, "/does/not/exist/standards.md");
    assert!(standards_text().contains("JIS C 0617"));

    let dir = std::env::temp_dir().join(format!("madake_standards_env_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("standards.md");
    std::fs::write(&path, "# 差し替えた規格ノート\n").unwrap();
    std::env::set_var(STANDARDS_PATH_ENV, &path);
    let text = standards_text();
    assert!(text.contains("差し替えた規格ノート"), "{text}");
    assert!(!text.contains("JIS C 0617"), "{text}");

    std::env::remove_var(STANDARDS_PATH_ENV);
    assert!(standards_text().contains("JIS C 0617"));
    std::fs::remove_dir_all(&dir).ok();
}
