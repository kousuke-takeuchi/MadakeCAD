//! MCP の open_project / save_project (外部の MCP クライアントがファイルを開く・保存する) のテスト。

use std::path::{Path, PathBuf};
use std::time::Duration;

use madake_core::{Engine, Project};
use madake_mcp::agent::{manager, open_project_tool, save_project_tool};
use madake_mcp::SharedDoc;
use uuid::Uuid;

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("madake_project_tools_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn doc_named(name: &str) -> SharedDoc {
    SharedDoc::new(Engine::new(Project::new(name)))
}

fn project_name(doc: &SharedDoc) -> String {
    doc.engine.lock().unwrap().project().name.clone()
}

/// A project saved with save_project opens again with open_project, replacing the drawing that was open.
/// save_project で保存したプロジェクトは open_project で開き直せ、開いていた図面と置き換わる。
#[test]
fn a_saved_project_opens_again_and_replaces_the_open_drawing() {
    let dir = temp_dir();
    let path = dir.join("回路配線図.mdkproj");
    let saved = doc_named("保存した図面");
    let agent = manager(&saved, 9310);
    save_project_tool(&saved, Some(&agent), &path).unwrap();
    assert!(path.exists());

    let doc = doc_named("別の図面");
    let agent = manager(&doc, 9310);
    open_project_tool(&doc, Some(&agent), &path).unwrap();
    assert_eq!(project_name(&doc), "保存した図面");
    std::fs::remove_dir_all(&dir).ok();
}

/// Opening a missing file or a file that is not a MadakeCAD project fails with the reason and leaves the open drawing as it was.
/// 無いファイルや MadakeCAD のプロジェクトでないファイルを開こうとすると理由つきで失敗し、開いている図面はそのまま残る。
#[test]
fn opening_a_missing_or_foreign_file_fails_and_keeps_the_drawing() {
    let dir = temp_dir();
    let doc = doc_named("開いている図面");
    let agent = manager(&doc, 9310);
    let err = open_project_tool(&doc, Some(&agent), &dir.join("ない.mdkproj")).unwrap_err();
    assert!(!err.is_empty());
    let sch = dir.join("回路.kicad_sch");
    std::fs::write(&sch, "(kicad_sch)").unwrap();
    let err = open_project_tool(&doc, Some(&agent), &sch).unwrap_err();
    assert!(err.contains("import_kicad"), "{err}");
    assert_eq!(project_name(&doc), "開いている図面");
    std::fs::remove_dir_all(&dir).ok();
}

/// The in-app agent's own tool window (no agent manager attached) cannot open a project, because that would replace the drawing under its own running turn.
/// アプリ内エージェント用のツール窓口 (エージェント管理が付いていない) からはプロジェクトを開けない。自分の応答中に図面が差し替わってしまうため。
#[test]
fn the_in_app_tool_window_cannot_open_a_project() {
    let dir = temp_dir();
    let path = dir.join("a.mdkproj");
    let doc = doc_named("図面");
    save_project_tool(&doc, None, &path).unwrap();
    let err = open_project_tool(&doc, None, &path).unwrap_err();
    assert!(err.contains("開けません"), "{err}");
    std::fs::remove_dir_all(&dir).ok();
}

/// While an agent turn is running, open_project refuses instead of cancelling that turn, so an agent connected over MCP cannot cut off its own reply.
/// エージェントの応答中は、open_project はそのターンを中断せずに断る。MCP でつながったエージェントが自分の応答を断ち切らないようにするため。
#[tokio::test]
async fn opening_is_refused_while_an_agent_turn_is_running() {
    let dir = temp_dir();
    let path = dir.join("b.mdkproj");
    let doc = doc_named("応答中の図面");
    let agent = manager(&doc, 9310);
    save_project_tool(&doc, Some(&agent), &path).unwrap();
    agent.set_executable(Some(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../madake-agent/tests/fixtures/fake_claude_rendezvous.sh"),
    ));
    let wait = dir.join("rv");
    std::fs::create_dir_all(&wait).unwrap();
    let id = agent
        .send(None, &format!("rendezvous:{} 待つ", wait.display()), None, None)
        .await
        .unwrap();
    for _ in 0..40 {
        if agent.is_sending(id) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(agent.is_sending(id), "ターンが始まらない");

    let err = open_project_tool(&doc, Some(&agent), &path).unwrap_err();
    assert!(err.contains("応答中"), "{err}");
    assert!(agent.is_sending(id), "断ったのにターンが止まった");
    agent.cancel_all();
    std::fs::remove_dir_all(&dir).ok();
}

/// Saving from the in-app tool window writes the drawing only; the chat history is saved only where the agent manager is attached.
/// アプリ内のツール窓口から保存すると図面だけを書く。チャット履歴はエージェント管理が付いているときだけ保存する。
#[test]
fn saving_without_the_agent_manager_writes_the_drawing_only() {
    let dir = temp_dir();
    let path = dir.join("c.mdkproj");
    let doc = doc_named("図面だけ");
    save_project_tool(&doc, None, &path).unwrap();
    assert!(path.exists());
    assert!(!madake_agent::chat_path_for(&path).exists());
    std::fs::remove_dir_all(&dir).ok();
}

/// The MCP server offers open_project and save_project, and their descriptions say which file types they take and that opening replaces the drawing.
/// MCP サーバーは open_project と save_project を公開し、説明には扱うファイルの種類と「開くと図面が置き換わる」ことが書かれている。
#[test]
fn the_server_offers_open_and_save_with_clear_descriptions() {
    let tools = madake_mcp::tool_descriptions();
    let find = |n: &str| tools.iter().find(|(t, _)| t == n).map(|(_, d)| d.clone()).unwrap_or_default();
    let open = find("open_project");
    assert!(open.contains(".mdkproj") && open.contains("置き換"), "{open}");
    let save = find("save_project");
    assert!(save.contains(".mdkproj"), "{save}");
}
