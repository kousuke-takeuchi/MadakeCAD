//! 規格知識(standards.md)の注入テスト。
//!
//! 実ファイルの上書き(`MADAKE_STANDARDS_PATH`)は環境変数がプロセス全体に効くため、
//! 別のテストバイナリ(`knowledge_env.rs`)に分けてある。

use std::path::PathBuf;

use madake_agent::knowledge::{docs_guide, system_prompt, standards_text};
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

/// The system prompt tells the agent to start a blank drawing from a template instead of drawing everything by hand.
/// システムプロンプトは、白紙から作り始めるときはまず開始テンプレートを使うよう指示する。
#[test]
fn system_prompt_points_at_the_start_templates() {
    let prompt = system_prompt(&AppSettings::default(), None);
    assert!(prompt.contains("list_templates"), "{prompt}");
    assert!(prompt.contains("apply_template"), "{prompt}");
}

/// The system prompt says the shell is unavailable and that entity ids are written by hand.
/// システムプロンプトは、シェルが使えないこととエンティティidを自分で書くことを伝える。
#[test]
fn system_prompt_tells_the_agent_no_shell_is_available() {
    let prompt = system_prompt(&AppSettings::default(), None);
    assert!(prompt.contains("Bash"), "{prompt}");
    assert!(prompt.contains("シェルで生成せず自分で書く"), "{prompt}");
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

/// The prompt lists the bundled manual (01-13) so how-to questions are answered from the documentation with a source.
/// プロンプトには同梱マニュアル(01〜13)の目次が載り、操作方法の質問へ出典つきで答えられる。
#[test]
fn system_prompt_lists_the_bundled_documentation() {
    let prompt = system_prompt(&AppSettings::default(), None);
    for needle in [
        "01-overview.md",
        "02-getting-started.md",
        "03-schematic-editor.md",
        "04-standards-output.md",
        "05-wire-management.md",
        "06-verification-simulation.md",
        "07-parts-database.md",
        "08-import-export.md",
        "09-ai-assistant.md",
        "10-automation-api.md",
        "11-mechanical-integration.md",
        "12-roadmap.md",
        "13-specification.md",
        // 推測ではなく読んでから答え、出典(ファイル名)を添える
        "出典",
    ] {
        assert!(prompt.contains(needle), "ドキュメント案内に「{needle}」が無い");
    }
}

/// Questions about missing features are answered from the roadmap and the feature inventory.
/// 未対応機能の質問には、ロードマップと機能インベントリを見てマイルストーンを答える。
#[test]
fn system_prompt_points_unsupported_features_at_the_roadmap() {
    let prompt = system_prompt(&AppSettings::default(), None);
    assert!(prompt.contains("feature-inventory.md"), "{prompt}");
    assert!(prompt.contains("12-roadmap.md"), "{prompt}");
    assert!(prompt.contains("未対応"), "{prompt}");
}

/// Without bundled documentation the guide is dropped instead of pointing at files that are not there.
/// ドキュメントが同梱されていない環境では、存在しないファイルを案内せずガイドごと省く。
#[test]
fn the_documentation_guide_is_dropped_when_the_docs_are_missing() {
    // そもそも場所が分からない
    assert!(docs_guide(None).is_none());
    // 場所はあるが中身が違う(01-overview.mdが無い)ディレクトリも案内しない
    let dir = temp_dir();
    assert!(docs_guide(Some(&dir)).is_none());
    std::fs::remove_dir_all(&dir).ok();
}

/// A review request runs the deterministic verification first, then the five habit-based checkpoints.
/// レビュー依頼ではまず`run_verification`を実行し、その後で慣行の5観点を点検する。
#[test]
fn system_prompt_carries_the_review_checklist() {
    let prompt = system_prompt(&AppSettings::default(), None);
    let review = prompt.find("設計レビュー").expect("レビュー節が無い");
    let section = &prompt[review..];
    for needle in [
        "run_verification",
        "参照記号",
        "線色",
        "線番",
        "レイアウト",
        "表題欄",
        "改訂欄",
    ] {
        assert!(section.contains(needle), "レビュー観点に「{needle}」が無い");
    }
}

/// Review findings are listed as severity / target / finding / suggestion, and fixes wait for the user's approval.
/// レビューの指摘は「重要度|対象|指摘|提案」で並べ、修正はユーザー承認を待つ。
#[test]
fn review_findings_use_the_severity_table_and_wait_for_approval() {
    let prompt = system_prompt(&AppSettings::default(), None);
    assert!(
        prompt.contains("| 重要度 | 対象 | 指摘 | 提案 |"),
        "{prompt}"
    );
    assert!(prompt.contains("高"), "{prompt}");
    assert!(prompt.contains("承認"), "{prompt}");
}

/// A test-plan request produces a step / action / expected-result table and does not write notes into the drawing on its own.
/// 動作確認手順の依頼には「手順|操作|期待結果」の表で答え、図面へ勝手に注記を書き込まない。
#[test]
fn system_prompt_carries_the_test_plan_table() {
    let prompt = system_prompt(&AppSettings::default(), None);
    assert!(prompt.contains("| 手順 | 操作 | 期待結果 |"), "{prompt}");
    let plan = prompt.find("動作確認手順").expect("検証計画の節が無い");
    assert!(prompt[plan..].contains("指示"), "{prompt}");
}

/// Parts selection searches the parts database and answers with a comparison table.
/// 部品選定では部品DBを検索し、比較表で答える。
#[test]
fn system_prompt_carries_the_parts_comparison_table() {
    let prompt = system_prompt(&AppSettings::default(), None);
    assert!(prompt.contains("search_parts"), "{prompt}");
    assert!(
        prompt.contains("| 型番 | メーカ | 定格 | 価格 | 購入先 | 差分 |"),
        "{prompt}"
    );
    // 置換は提案どまり。実行はユーザーの承認後
    let parts = prompt.find("部品選定").expect("部品選定の節が無い");
    assert!(prompt[parts..].contains("承認"), "{prompt}");
    assert!(prompt[parts..].contains("execute_commands"), "{prompt}");
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

/// The system prompt tells the agent to measure tidiness with get_tidy_metrics and to stop once it stops improving.
/// システムプロンプトは、整えるときは`get_tidy_metrics`で数値を測り、改善が止まったらやめるよう指示する。
#[test]
fn system_prompt_carries_the_tidy_loop() {
    let prompt = system_prompt(&AppSettings::default(), None);
    // 直す前に測り、編集したらもう一度測る
    assert!(prompt.contains("get_tidy_metrics"), "{prompt}");
    // 改善が止まるか3回で終わる(いつまでもいじらない)
    assert!(prompt.contains("改善が止まったら"), "{prompt}");
    assert!(prompt.contains("3回"), "{prompt}");
    // 最終応答にビフォー/アフターの数値を書く
    assert!(prompt.contains("ビフォー"), "{prompt}");
}
