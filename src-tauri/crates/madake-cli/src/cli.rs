//! 引数定義とディスパッチ。`run` は出力文字列を返すだけなので、
//! フェイクの [`LinkApi`] 実装でテストできる。

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde_json::Value;

use crate::client::{base_url, CliError, ExportKind, LinkApi, DEFAULT_PORT};
use crate::format;

#[derive(Debug, Parser)]
#[command(
    name = "madake",
    version,
    about = "MadakeCADのターミナルクライアント (起動中のアプリにLink APIで接続する)"
)]
pub struct Cli {
    /// Link APIのポート (アプリ内蔵サーバー)
    #[arg(long, global = true, default_value_t = DEFAULT_PORT)]
    pub port: u16,

    /// 整形せず生のJSONを出力する
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// 接続確認と図面の概要
    Status,
    /// プロジェクト全体 (シート一覧・電線品番)
    Project,
    /// ネットリスト
    Netlist {
        /// シートID (省略時は先頭シート)
        #[arg(long, value_name = "ID")]
        sheet: Option<String>,
    },
    /// SVG / 部品表 / 電線リストの書き出し
    Export {
        /// 種別
        #[arg(value_enum)]
        kind: ExportKind,
        /// 出力先パス
        path: String,
        /// シートID (svgのみ有効。省略時は先頭シート)
        #[arg(long, value_name = "ID")]
        sheet: Option<String>,
    },
    /// プロジェクトを保存
    Save {
        /// 保存先 .mdkproj
        path: String,
    },
    /// プロジェクトを読み込む
    Open {
        /// 読み込む .mdkproj
        path: String,
    },
    /// Command配列のJSONファイルを実行する (Commandエンジン経由)
    Exec {
        /// Commandの配列を含むJSONファイル
        file: PathBuf,
    },
    /// 直前の編集を元に戻す
    Undo,
    /// 元に戻した編集をやり直す
    Redo,
}

/// JSONを整形して文字列化する。
fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

/// サブコマンドを実行し、標準出力に出す文字列を返す。
pub fn run(cli: &Cli, api: &dyn LinkApi) -> Result<String, CliError> {
    match &cli.command {
        Commands::Status => {
            let health = api.health()?;
            let snapshot = api.project()?;
            if cli.json {
                return Ok(pretty(&serde_json::json!({
                    "endpoint": base_url(cli.port),
                    "api": health,
                    "project": snapshot,
                })));
            }
            Ok(format::status(&health, &snapshot, cli.port))
        }
        Commands::Project => {
            let snapshot = api.project()?;
            if cli.json {
                return Ok(pretty(&snapshot));
            }
            Ok(format::project(&snapshot))
        }
        Commands::Netlist { sheet } => {
            let nets = api.netlist(sheet.as_deref())?;
            if cli.json {
                return Ok(pretty(&nets));
            }
            Ok(format::netlist(&nets))
        }
        Commands::Export { kind, path, sheet } => {
            let result = api.export(*kind, path, sheet.as_deref())?;
            if cli.json {
                return Ok(pretty(&result));
            }
            Ok(format::exported(*kind, &result))
        }
        Commands::Save { path } => {
            let result = api.save(path)?;
            if cli.json {
                return Ok(pretty(&result));
            }
            Ok(format::saved(&result))
        }
        Commands::Open { path } => {
            let patch = api.open(path)?;
            if cli.json {
                return Ok(pretty(&patch));
            }
            Ok(format::opened(&patch, path))
        }
        Commands::Exec { file } => {
            let text = std::fs::read_to_string(file)
                .map_err(|e| CliError::Io(format!("{}: {e}", file.display())))?;
            let commands: Value = serde_json::from_str(&text)
                .map_err(|e| CliError::Json(format!("{}: {e}", file.display())))?;
            let count = match commands.as_array() {
                Some(a) => a.len(),
                None => {
                    return Err(CliError::Json(format!(
                        "{}: Commandの配列 (JSON array) である必要があります",
                        file.display()
                    )))
                }
            };
            let patches = api.exec(commands)?;
            if cli.json {
                return Ok(pretty(&patches));
            }
            Ok(format::exec_result(count, &patches))
        }
        Commands::Undo => {
            let patch = api.undo()?;
            if cli.json {
                return Ok(pretty(&patch));
            }
            Ok(format::history_result(
                &patch,
                "元に戻しました",
                "元に戻せる操作がありません。",
            ))
        }
        Commands::Redo => {
            let patch = api.redo()?;
            if cli.json {
                return Ok(pretty(&patch));
            }
            Ok(format::history_result(
                &patch,
                "やり直しました",
                "やり直せる操作がありません。",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use serde_json::json;

    use super::*;

    #[derive(Default)]
    struct FakeApi {
        calls: RefCell<Vec<String>>,
        netlist: Value,
        project: Value,
        health: Value,
        result: Value,
    }

    impl FakeApi {
        fn calls(&self) -> Vec<String> {
            self.calls.borrow().clone()
        }
        fn record(&self, s: String) {
            self.calls.borrow_mut().push(s);
        }
    }

    impl LinkApi for FakeApi {
        fn health(&self) -> Result<Value, CliError> {
            self.record("health".into());
            Ok(self.health.clone())
        }
        fn project(&self) -> Result<Value, CliError> {
            self.record("project".into());
            Ok(self.project.clone())
        }
        fn netlist(&self, sheet_id: Option<&str>) -> Result<Value, CliError> {
            self.record(format!("netlist({sheet_id:?})"));
            Ok(self.netlist.clone())
        }
        fn exec(&self, commands: Value) -> Result<Value, CliError> {
            self.record(format!("exec({commands})"));
            Ok(self.result.clone())
        }
        fn undo(&self) -> Result<Value, CliError> {
            self.record("undo".into());
            Ok(self.result.clone())
        }
        fn redo(&self) -> Result<Value, CliError> {
            self.record("redo".into());
            Ok(self.result.clone())
        }
        fn save(&self, path: &str) -> Result<Value, CliError> {
            self.record(format!("save({path})"));
            Ok(json!({ "written": path }))
        }
        fn open(&self, path: &str) -> Result<Value, CliError> {
            self.record(format!("open({path})"));
            Ok(self.result.clone())
        }
        fn export(
            &self,
            kind: ExportKind,
            path: &str,
            sheet_id: Option<&str>,
        ) -> Result<Value, CliError> {
            self.record(format!("export({kind:?}, {path}, {sheet_id:?})"));
            Ok(json!({ "written": path }))
        }
    }

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).expect("引数の解析")
    }

    fn snapshot() -> Value {
        json!({
            "revision": 3,
            "can_undo": false,
            "can_redo": false,
            "project": {
                "format_version": 1,
                "name": "P",
                "wire_parts": [],
                "sheets": [{
                    "id": "s1", "name": "Sheet1", "size": "A3", "orientation": "Landscape",
                    "entities": {}
                }]
            }
        })
    }

    #[test]
    fn default_port_is_9310_and_json_is_off() {
        let cli = parse(&["madake", "status"]);
        assert_eq!(cli.port, DEFAULT_PORT);
        assert!(!cli.json);
    }

    #[test]
    fn port_and_json_are_global_options_after_subcommand() {
        let cli = parse(&["madake", "netlist", "--port", "19310", "--json"]);
        assert_eq!(cli.port, 19310);
        assert!(cli.json);
    }

    #[test]
    fn export_kind_accepts_wire_list_spelling() {
        let cli = parse(&["madake", "export", "wire-list", "/tmp/w.csv"]);
        match cli.command {
            Commands::Export { kind, path, sheet } => {
                assert_eq!(kind, ExportKind::WireList);
                assert_eq!(path, "/tmp/w.csv");
                assert!(sheet.is_none());
            }
            _ => panic!("exportではない"),
        }
    }

    #[test]
    fn status_queries_health_and_project() {
        let api = FakeApi {
            health: json!({ "name": "MadakeCAD Link API", "version": 1 }),
            project: snapshot(),
            ..Default::default()
        };
        let out = run(&parse(&["madake", "status"]), &api).unwrap();
        assert_eq!(api.calls(), vec!["health", "project"]);
        assert!(out.contains("Sheet1"));
    }

    #[test]
    fn json_flag_emits_raw_json() {
        let api = FakeApi {
            netlist: json!([{ "name": "N001", "pins": [], "wire_ids": [] }]),
            ..Default::default()
        };
        let out = run(&parse(&["madake", "netlist", "--json"]), &api).unwrap();
        let parsed: Value = serde_json::from_str(&out).expect("生JSON");
        assert_eq!(parsed[0]["name"], "N001");
    }

    #[test]
    fn netlist_forwards_sheet_option() {
        let api = FakeApi {
            netlist: json!([]),
            ..Default::default()
        };
        run(&parse(&["madake", "netlist", "--sheet", "abc"]), &api).unwrap();
        assert_eq!(api.calls(), vec![r#"netlist(Some("abc"))"#]);
    }

    #[test]
    fn export_forwards_kind_path_and_sheet() {
        let api = FakeApi::default();
        let out = run(
            &parse(&["madake", "export", "svg", "/tmp/a.svg", "--sheet", "s1"]),
            &api,
        )
        .unwrap();
        assert_eq!(api.calls(), vec![r#"export(Svg, /tmp/a.svg, Some("s1"))"#]);
        assert!(out.contains("/tmp/a.svg"));
    }

    #[test]
    fn exec_posts_command_array_from_file() {
        let dir = std::env::temp_dir().join("madake-cli-test-exec");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("cmds.json");
        std::fs::write(
            &file,
            r#"[{"type":"add_sheet","name":"S2","size":"A3","orientation":"Landscape"}]"#,
        )
        .unwrap();
        let api = FakeApi {
            result: json!([{ "revision": 4, "ops": [{ "op": "sheet_added" }] }]),
            ..Default::default()
        };
        let out = run(&parse(&["madake", "exec", file.to_str().unwrap()]), &api).unwrap();
        let calls = api.calls();
        assert!(calls[0].starts_with("exec(["), "{calls:?}");
        assert!(calls[0].contains("add_sheet"));
        assert!(out.contains("1 件"));
        assert!(out.contains("revision 4"));
    }

    #[test]
    fn exec_rejects_non_array_json() {
        let dir = std::env::temp_dir().join("madake-cli-test-exec");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("bad.json");
        std::fs::write(&file, r#"{"type":"add_sheet"}"#).unwrap();
        let err = run(
            &parse(&["madake", "exec", file.to_str().unwrap()]),
            &FakeApi::default(),
        )
        .unwrap_err();
        assert!(matches!(err, CliError::Json(_)), "{err:?}");
    }

    #[test]
    fn exec_reports_missing_file() {
        let err = run(
            &parse(&["madake", "exec", "/nonexistent/madake-cli/none.json"]),
            &FakeApi::default(),
        )
        .unwrap_err();
        assert!(matches!(err, CliError::Io(_)), "{err:?}");
    }

    #[test]
    fn undo_reports_empty_history() {
        let api = FakeApi {
            result: Value::Null,
            ..Default::default()
        };
        let out = run(&parse(&["madake", "undo"]), &api).unwrap();
        assert_eq!(api.calls(), vec!["undo"]);
        assert!(out.contains("元に戻せる操作がありません"));
    }

    #[test]
    fn redo_reports_revision() {
        let api = FakeApi {
            result: json!({ "revision": 9, "ops": [] }),
            ..Default::default()
        };
        let out = run(&parse(&["madake", "redo"]), &api).unwrap();
        assert!(out.contains("revision 9"));
    }

    #[test]
    fn save_and_open_report_path() {
        let api = FakeApi {
            result: json!({ "revision": 1, "ops": [] }),
            ..Default::default()
        };
        assert!(run(&parse(&["madake", "save", "/tmp/a.mdkproj"]), &api)
            .unwrap()
            .contains("/tmp/a.mdkproj"));
        assert!(run(&parse(&["madake", "open", "/tmp/b.mdkproj"]), &api)
            .unwrap()
            .contains("/tmp/b.mdkproj"));
    }
}
