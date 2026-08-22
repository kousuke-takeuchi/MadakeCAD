//! 規格知識(standards.md)の注入テスト。
//!
//! 実ファイルの上書き(`MADAKE_STANDARDS_PATH`)は環境変数がプロセス全体に効くため、
//! 別のテストバイナリ(`knowledge_env.rs`)に分けてある。

use std::path::PathBuf;

use madake_agent::knowledge::{system_prompt, standards_text};
use madake_agent::AppSettings;
use uuid::Uuid;

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("madake_knowledge_test_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The bundled standards note covers symbols, reference designators, wire colors, numbering and layout.
/// 同梱の規格知識には図記号・参照記号・線色/sq・線番・配置の決まりが書かれている。
#[test]
fn bundled_standards_cover_the_drawing_conventions() {
    let text = standards_text();
    for needle in [
        "JIS C 0617",
        "参照記号",
        "PB",
        "TB",
        "緑/黄",
        "0.75",
        "2.5mm",
        "renumber_wires",
        "erc.duplicate_reference",
    ] {
        assert!(text.contains(needle), "同梱の規格知識に「{needle}」が無い");
    }
}

/// The system prompt carries the bundled standards and the verification-loop rule.
/// システムプロンプトには同梱の規格知識と検証ループの指示が載る。
#[test]
fn system_prompt_carries_the_standards_and_the_verification_loop() {
    let prompt = system_prompt(&AppSettings::default(), None);
    assert!(prompt.contains("JIS C 0617"), "{prompt}");
    // 編集したら検証、Errorが残れば直して再検証(上限3回)、最後に件数を報告
    assert!(prompt.contains("run_verification"), "{prompt}");
    assert!(prompt.contains("最大3回"), "{prompt}");
    assert!(prompt.contains("検証: エラー"), "{prompt}");
}

/// The drawing context comes first in the system prompt, with the knowledge behind it.
/// システムプロンプトは図面コンテキストが先頭で、規格知識はその後ろに続く。
#[test]
fn system_prompt_puts_the_drawing_context_first() {
    let prompt = system_prompt(&AppSettings::default(), Some("## 現在の図面\n- 盤A\n"));
    let drawing = prompt.find("盤A").expect("図面コンテキストが無い");
    let knowledge = prompt.find("JIS C 0617").expect("規格知識が無い");
    assert!(drawing < knowledge, "{prompt}");
}

/// A knowledge file set in the settings is appended after the bundled note.
/// 設定の知識ファイルは同梱ノートの後ろへ追記される。
#[test]
fn user_knowledge_file_is_appended_to_the_prompt() {
    let dir = temp_dir();
    let path = dir.join("house-rules.md");
    std::fs::write(&path, "## 社内ルール\n- 制御電源は24VDCのみ\n").unwrap();

    let settings = AppSettings {
        knowledge_path: Some(path.clone()),
        ..AppSettings::default()
    };
    let prompt = system_prompt(&settings, None);
    assert!(prompt.contains("社内ルール"), "{prompt}");
    assert!(prompt.contains("制御電源は24VDCのみ"), "{prompt}");
    // どのファイルから来た知識かが分かる(出典を示せるように)
    assert!(prompt.contains(&path.display().to_string()), "{prompt}");
    // 同梱ノートより後ろ(後勝ちで上書きできる)
    assert!(
        prompt.find("JIS C 0617") < prompt.find("社内ルール"),
        "{prompt}"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// Without a knowledge file the prompt holds the bundled note only.
/// 知識ファイル未設定なら、プロンプトには同梱ノートだけが載る。
#[test]
fn without_a_knowledge_file_only_the_bundled_note_is_used() {
    let prompt = system_prompt(&AppSettings::default(), None);
    assert!(prompt.contains("JIS C 0617"), "{prompt}");
    assert!(!prompt.contains("追加の知識"), "{prompt}");
}

/// An unreadable knowledge file is skipped without losing the bundled note.
/// 読めない知識ファイルは読み飛ばされ、同梱ノートは失われない。
#[test]
fn an_unreadable_knowledge_file_is_skipped() {
    let settings = AppSettings {
        knowledge_path: Some(PathBuf::from("/does/not/exist/house-rules.md")),
        ..AppSettings::default()
    };
    let prompt = system_prompt(&settings, None);
    assert!(prompt.contains("JIS C 0617"), "{prompt}");
    assert!(!prompt.contains("追加の知識"), "{prompt}");
}
