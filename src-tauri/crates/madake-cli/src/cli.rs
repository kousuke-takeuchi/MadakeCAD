//! 引数定義とディスパッチ。`run` は出力文字列を返すだけなので、
//! フェイクの [`LinkApi`] 実装でテストできる。

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde_json::Value;

use crate::client::{
    base_url, looks_like_uuid, CliError, ExportKind, LinkApi, RenumberMode, ReportFormat,
    ReportKind, DEFAULT_PORT,
};
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
    /// 部品DBを検索する (グローバル共有マスタ)
    Parts {
        /// 型番・名称・メーカの部分一致 (省略時は全件)
        query: Option<String>,
        /// カテゴリ完全一致 (例: relay, connector)
        #[arg(long)]
        category: Option<String>,
    },
    /// DC動作点シミュレーション (ngspice)
    Sim {
        /// シートID (省略時は先頭シート)
        #[arg(long, value_name = "ID")]
        sheet: Option<String>,
        /// 開路にするスイッチ/接点の参照記号 (カンマ区切り。例: SW1,K1)
        #[arg(long, value_delimiter = ',')]
        open: Vec<String>,
    },
    /// 図面検証 (ERC+電気検証)
    Verify {
        /// シートID (省略時は全シート)
        #[arg(long, value_name = "ID")]
        sheet: Option<String>,
    },
    /// 回路図 (SVG/PDF/図面一式PDF) と帳票5種の書き出し
    Export {
        /// 種別
        #[arg(value_enum)]
        kind: ExportKind,
        /// 出力先パス
        path: String,
        /// シートID (svg/pdfは対象シート。帳票では--terminalの探索範囲)
        #[arg(long, value_name = "ID")]
        sheet: Option<String>,
        /// 対象を1つの端子台に絞る (terminal-chart/terminal-diagramのみ。参照記号 例:TB1 かentity id)
        #[arg(long, value_name = "REF|ID")]
        terminal: Option<String>,
        /// 帳票の出力形式 (省略時は出力先の拡張子から判定。.pdf以外はcsv)
        #[arg(long, value_enum, value_name = "FORMAT")]
        format: Option<ReportFormat>,
        /// 一括PDFに付ける帳票 (pdf-bookのみ有効。カンマ区切りで並べた順に付く)
        #[arg(long, value_enum, value_delimiter = ',', value_name = "KIND")]
        reports: Vec<ReportKind>,
        /// 一括PDFの表紙を付けない (pdf-bookのみ有効)
        #[arg(long)]
        no_cover: bool,
    },
    /// 端子台の一覧 (参照記号・極数・ジャンパ。export --terminal に渡す対象を調べる)
    Terminals {
        /// シートID (省略時はプロジェクト全体)
        #[arg(long, value_name = "ID")]
        sheet: Option<String>,
    },
    /// プロジェクトを保存
    Save {
        /// 保存先 .mdkproj
        path: String,
    },
    /// プロジェクトを読み込む (.mdkproj / .kicad_sch)
    Open {
        /// 読み込むファイル (.kicad_schはKiCadインポート)
        path: String,
    },
    /// 線番をネット単位で自動採番する (renumber_wiresコマンド。undo可)
    Renumber {
        /// シートID (省略時はプロジェクト全体で一意に採番)
        #[arg(long, value_name = "ID")]
        sheet: Option<String>,
        /// 採番方式: append=未採番のネットだけ追い番 / renumber=全て振り直し
        #[arg(long, value_enum, default_value_t = RenumberMode::Append)]
        mode: RenumberMode,
        /// 開始番号
        #[arg(long, default_value_t = 1)]
        start: u32,
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

/// `--terminal` の値をentity idに解決する。
///
/// UUID表記ならそのまま使い、そうでなければ参照記号 (`TB1`) とみなして
/// `/terminals` の一覧から引く。見つからない・複数該当する場合は候補を添えて中断する。
fn resolve_terminal(
    api: &dyn LinkApi,
    value: &str,
    sheet_id: Option<&str>,
) -> Result<String, CliError> {
    if looks_like_uuid(value) {
        return Ok(value.to_string());
    }
    let list = api.terminals(sheet_id)?;
    let blocks = list.as_array().cloned().unwrap_or_default();
    let hits: Vec<&Value> = blocks
        .iter()
        .filter(|b| {
            b["reference"]
                .as_str()
                .is_some_and(|r| r.eq_ignore_ascii_case(value))
        })
        .collect();
    let label = |b: &Value| {
        format!(
            "{} ({})",
            b["reference"].as_str().unwrap_or("?"),
            b["sheet_name"].as_str().unwrap_or("?")
        )
    };
    match hits.as_slice() {
        [one] => Ok(one["entity_id"].as_str().unwrap_or_default().to_string()),
        [] => {
            let names: Vec<String> = blocks.iter().map(label).collect();
            Err(CliError::Usage(format!(
                "端子台 {value} が見つかりません。図面にある端子台: {}",
                if names.is_empty() {
                    "(なし)".to_string()
                } else {
                    names.join(", ")
                }
            )))
        }
        many => Err(CliError::Usage(format!(
            "端子台 {value} が複数のシートにあります ({})。--sheet <ID> でシートを指定してください",
            many.iter().map(|b| label(b)).collect::<Vec<_>>().join(", ")
        ))),
    }
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
        Commands::Parts { query, category } => {
            let parts = api.parts(query.as_deref(), category.as_deref())?;
            if cli.json {
                return Ok(pretty(&parts));
            }
            Ok(format::parts(&parts))
        }
        Commands::Sim { sheet, open } => {
            let result = api.simulate_op(sheet.as_deref(), open)?;
            if cli.json {
                return Ok(pretty(&result));
            }
            Ok(format::sim_op(&result))
        }
        Commands::Verify { sheet } => {
            let diags = api.verify(sheet.as_deref())?;
            if cli.json {
                return Ok(pretty(&diags));
            }
            Ok(format::diagnostics(&diags))
        }
        Commands::Export {
            kind,
            path,
            sheet,
            terminal,
            format,
            reports,
            no_cover,
        } => {
            if terminal.is_some() && !kind.takes_terminal() {
                return Err(CliError::Usage(format!(
                    "--terminal は terminal-chart / terminal-diagram でのみ使えます ({} には指定できません)",
                    kind.label()
                )));
            }
            let Some(report) = kind.report_kind() else {
                // 回路図の出力 (svg / pdf / pdf-book)
                if format.is_some() {
                    return Err(CliError::Usage(format!(
                        "--format は帳票の出力形式です ({} には指定できません)",
                        kind.label()
                    )));
                }
                let result = match kind {
                    ExportKind::PdfBook => api.export_pdf_book(path, reports, !no_cover)?,
                    _ => api.export(*kind, path, sheet.as_deref())?,
                };
                return Ok(if cli.json {
                    pretty(&result)
                } else {
                    format::exported(*kind, &result)
                });
            };
            // 帳票はプロジェクト全体が対象。絞り込めるのは端子台単位だけ
            if sheet.is_some() && terminal.is_none() {
                return Err(CliError::Usage(
                    "帳票はプロジェクト全体が対象です。対象を絞るには --terminal <参照記号> を指定してください (--sheet はその探索範囲)".into(),
                ));
            }
            let entity_id = match terminal {
                Some(t) => Some(resolve_terminal(api, t, sheet.as_deref())?),
                None => None,
            };
            let fmt = format.unwrap_or_else(|| ReportFormat::for_path(path));
            let result = api.export_report(report, fmt, path, entity_id.as_deref())?;
            if cli.json {
                return Ok(pretty(&result));
            }
            Ok(format::exported_report(*kind, fmt, &result))
        }
        Commands::Terminals { sheet } => {
            let list = api.terminals(sheet.as_deref())?;
            if cli.json {
                return Ok(pretty(&list));
            }
            Ok(format::terminals(&list))
        }
        Commands::Save { path } => {
            let result = api.save(path)?;
            if cli.json {
                return Ok(pretty(&result));
            }
            Ok(format::saved(&result))
        }
        Commands::Open { path } => {
            if path.ends_with(".kicad_sch") {
                let result = api.import_kicad(path)?;
                if cli.json {
                    return Ok(pretty(&result));
                }
                return Ok(format::kicad_imported(&result, path));
            }
            let patch = api.open(path)?;
            if cli.json {
                return Ok(pretty(&patch));
            }
            Ok(format::opened(&patch, path))
        }
        Commands::Renumber { sheet, mode, start } => {
            // 薄いクライアント: 採番ロジックはコア側。ここはCommandを組み立てて送るだけ
            let mut cmd = serde_json::json!({
                "type": "renumber_wires",
                "mode": mode.as_json(),
                "start": start,
            });
            if let Some(id) = sheet {
                cmd["sheet_id"] = Value::String(id.clone());
            }
            let patches = api.exec(Value::Array(vec![cmd]))?;
            if cli.json {
                return Ok(pretty(&patches));
            }
            Ok(format::renumbered(&patches))
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
        terminals: Value,
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
        fn verify(&self, sheet_id: Option<&str>) -> Result<Value, CliError> {
            self.record(format!("verify({sheet_id:?})"));
            Ok(json!([]))
        }
        fn parts(&self, query: Option<&str>, category: Option<&str>) -> Result<Value, CliError> {
            self.record(format!("parts({query:?}, {category:?})"));
            Ok(json!([]))
        }
        fn simulate_op(
            &self,
            sheet_id: Option<&str>,
            open_switches: &[String],
        ) -> Result<Value, CliError> {
            self.record(format!("simulate_op({sheet_id:?}, {open_switches:?})"));
            Ok(json!({"voltage": 24.0, "nets": [], "components": [], "warnings": []}))
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
        fn import_kicad(&self, path: &str) -> Result<Value, CliError> {
            self.record(format!("import_kicad({path})"));
            Ok(json!({"patch": {"revision": 1}, "report": {"symbols": 0, "wires": 0, "skipped": [], "warnings": []}}))
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
        fn terminals(&self, sheet_id: Option<&str>) -> Result<Value, CliError> {
            self.record(format!("terminals({sheet_id:?})"));
            Ok(self.terminals.clone())
        }
        fn export_report(
            &self,
            kind: ReportKind,
            format: ReportFormat,
            path: &str,
            entity_id: Option<&str>,
        ) -> Result<Value, CliError> {
            self.record(format!(
                "export_report({}, {}, {path}, {entity_id:?})",
                kind.as_json(),
                format.as_json()
            ));
            Ok(json!({ "written": path, "count": 3 }))
        }
        fn export_pdf_book(
            &self,
            path: &str,
            reports: &[ReportKind],
            cover: bool,
        ) -> Result<Value, CliError> {
            let names: Vec<&str> = reports.iter().map(|r| r.as_json()).collect();
            self.record(format!("export_pdf_book({path}, {names:?}, cover={cover})"));
            Ok(json!({ "written": path, "pages": 4 }))
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

    /// The default port is 9310 and --json is off unless requested.
    /// 既定ポートは9310で、--jsonは指定しない限り無効。
    #[test]
    fn default_port_is_9310_and_json_is_off() {
        let cli = parse(&["madake", "status"]);
        assert_eq!(cli.port, DEFAULT_PORT);
        assert!(!cli.json);
    }

    /// --port and --json are global options and may appear after the subcommand.
    /// --portと--jsonはグローバルオプションで、サブコマンドの後にも書ける。
    #[test]
    fn port_and_json_are_global_options_after_subcommand() {
        let cli = parse(&["madake", "netlist", "--port", "19310", "--json"]);
        assert_eq!(cli.port, 19310);
        assert!(cli.json);
    }

    /// The export kind accepts the 'wire-list' spelling used in documentation.
    /// エクスポート種別はドキュメント表記どおりの'wire-list'を受け付ける。
    #[test]
    fn export_kind_accepts_wire_list_spelling() {
        let cli = parse(&["madake", "export", "wire-list", "/tmp/w.csv"]);
        match cli.command {
            Commands::Export { kind, path, sheet, .. } => {
                assert_eq!(kind, ExportKind::WireList);
                assert_eq!(path, "/tmp/w.csv");
                assert!(sheet.is_none());
            }
            _ => panic!("exportではない"),
        }
    }

    /// madake export pdf-book sends the reports listed with --reports, in that order, and asks for a cover by default.
    /// madake export pdf-book は --reports に並べた帳票をその順で送り、既定では表紙付きで依頼する。
    #[test]
    fn export_pdf_book_sends_the_requested_reports_in_order() {
        let api = FakeApi::default();
        let out = run(
            &parse(&[
                "madake",
                "export",
                "pdf-book",
                "/tmp/book.pdf",
                "--reports",
                "wire-list,terminal-chart",
            ]),
            &api,
        )
        .unwrap();
        assert_eq!(
            api.calls(),
            vec![
                "export_pdf_book(/tmp/book.pdf, [\"wire-list\", \"terminal-chart\"], cover=true)"
                    .to_string()
            ]
        );
        assert!(out.contains("図面一式PDF"), "{out}");
        assert!(out.contains("4 ページ"), "{out}");
    }

    /// --no-cover drops the cover page from the PDF book.
    /// --no-cover を付けると一括PDFから表紙が外れる。
    #[test]
    fn export_pdf_book_can_drop_the_cover() {
        let api = FakeApi::default();
        run(
            &parse(&["madake", "export", "pdf-book", "/tmp/book.pdf", "--no-cover"]),
            &api,
        )
        .unwrap();
        assert_eq!(
            api.calls(),
            vec!["export_pdf_book(/tmp/book.pdf, [], cover=false)".to_string()]
        );
    }

    fn terminal_list() -> Value {
        json!([
            {
                "entity_id": "2f4e0b9a-0000-4000-8000-00000000000a",
                "sheet_id": "2f4e0b9a-0000-4000-8000-000000000001",
                "sheet_name": "Sheet1",
                "reference": "TB1",
                "value": "",
                "terminal_count": 4,
                "jumpers": ""
            },
            {
                "entity_id": "2f4e0b9a-0000-4000-8000-00000000000b",
                "sheet_id": "2f4e0b9a-0000-4000-8000-000000000002",
                "sheet_name": "電源系統",
                "reference": "TB2",
                "value": "",
                "terminal_count": 8,
                "jumpers": "1-2"
            }
        ])
    }

    /// madake terminals lists the terminal blocks of the whole project, or of one sheet with --sheet.
    /// madake terminalsは図面全体の端子台を一覧し、--sheetでシートを絞れる。
    #[test]
    fn terminals_lists_blocks_and_forwards_sheet() {
        let api = FakeApi {
            terminals: terminal_list(),
            ..Default::default()
        };
        let out = run(&parse(&["madake", "terminals"]), &api).unwrap();
        assert_eq!(api.calls(), vec!["terminals(None)"]);
        assert!(out.contains("TB1"), "{out}");
        assert!(out.contains("8極"), "{out}");

        let api = FakeApi {
            terminals: terminal_list(),
            ..Default::default()
        };
        run(&parse(&["madake", "terminals", "--sheet", "s1"]), &api).unwrap();
        assert_eq!(api.calls(), vec![r#"terminals(Some("s1"))"#]);
    }

    /// madake export terminal-chart writes the whole project's chart, choosing CSV from the .csv extension.
    /// madake export terminal-chart は図面全体のチャートを、拡張子.csvからCSVと判断して書き出す。
    #[test]
    fn export_terminal_chart_defaults_to_csv_for_the_whole_project() {
        let api = FakeApi::default();
        let out = run(
            &parse(&["madake", "export", "terminal-chart", "/tmp/tb.csv"]),
            &api,
        )
        .unwrap();
        assert_eq!(
            api.calls(),
            vec!["export_report(terminal-chart, csv, /tmp/tb.csv, None)".to_string()]
        );
        assert!(out.contains("端子台チャート"), "{out}");
        assert!(out.contains("3 行"), "{out}");
    }

    /// A .pdf output path selects the PDF (framed drawing sheet) form of a report without asking for --format.
    /// 出力先が.pdfなら--format無しでも帳票のPDF(図枠付き図面シート)形式になる。
    #[test]
    fn export_report_picks_pdf_from_the_extension() {
        let api = FakeApi::default();
        let out = run(
            &parse(&["madake", "export", "xref-table", "/tmp/x.pdf"]),
            &api,
        )
        .unwrap();
        assert_eq!(
            api.calls(),
            vec!["export_report(xref, pdf, /tmp/x.pdf, None)".to_string()]
        );
        assert!(out.contains("3 ページ"), "{out}");
    }

    /// --format wins over the file extension.
    /// --formatは拡張子より優先される。
    #[test]
    fn export_report_format_option_overrides_the_extension() {
        let api = FakeApi::default();
        run(
            &parse(&[
                "madake",
                "export",
                "wire-list",
                "/tmp/w.out",
                "--format",
                "pdf",
            ]),
            &api,
        )
        .unwrap();
        assert_eq!(
            api.calls(),
            vec!["export_report(wire-list, pdf, /tmp/w.out, None)".to_string()]
        );
    }

    /// --terminal takes a reference designator (TB1) and looks its entity id up in the terminal list.
    /// --terminalは参照記号(TB1)を受け取り、端子台一覧からentity idを引く。
    #[test]
    fn export_terminal_diagram_resolves_a_reference_designator() {
        let api = FakeApi {
            terminals: terminal_list(),
            ..Default::default()
        };
        run(
            &parse(&[
                "madake",
                "export",
                "terminal-diagram",
                "/tmp/d.pdf",
                "--terminal",
                "tb2",
            ]),
            &api,
        )
        .unwrap();
        assert_eq!(
            api.calls(),
            vec![
                "terminals(None)".to_string(),
                "export_report(terminal-diagram, pdf, /tmp/d.pdf, Some(\"2f4e0b9a-0000-4000-8000-00000000000b\"))".to_string(),
            ]
        );
    }

    /// A --terminal value spelled as an entity id is sent as-is, with no lookup.
    /// --terminalにentity idを渡した場合は一覧を引かずそのまま送る。
    #[test]
    fn export_terminal_chart_accepts_an_entity_id_directly() {
        let api = FakeApi::default();
        run(
            &parse(&[
                "madake",
                "export",
                "terminal-chart",
                "/tmp/tb.csv",
                "--terminal",
                "2f4e0b9a-0000-4000-8000-00000000000a",
            ]),
            &api,
        )
        .unwrap();
        assert_eq!(
            api.calls(),
            vec!["export_report(terminal-chart, csv, /tmp/tb.csv, Some(\"2f4e0b9a-0000-4000-8000-00000000000a\"))".to_string()]
        );
    }

    /// An unknown terminal reference stops with the list of terminal blocks that do exist.
    /// 存在しない端子台を指定した場合は、図面にある端子台の一覧を添えて中断する。
    #[test]
    fn export_reports_unknown_terminal_with_candidates() {
        let api = FakeApi {
            terminals: terminal_list(),
            ..Default::default()
        };
        let err = run(
            &parse(&[
                "madake",
                "export",
                "terminal-chart",
                "/tmp/tb.csv",
                "--terminal",
                "TB9",
            ]),
            &api,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("TB9"), "{msg}");
        assert!(msg.contains("TB1"), "{msg}");
        assert!(msg.contains("TB2"), "{msg}");
    }

    /// --terminal is refused for reports that cover the whole project.
    /// 図面全体が対象の帳票に--terminalを付けると拒否する。
    #[test]
    fn export_rejects_terminal_option_on_other_reports() {
        let err = run(
            &parse(&[
                "madake",
                "export",
                "bom",
                "/tmp/b.csv",
                "--terminal",
                "TB1",
            ]),
            &FakeApi::default(),
        )
        .unwrap_err();
        assert!(matches!(err, CliError::Usage(_)), "{err:?}");
    }

    /// Reports always cover the whole project, so --sheet alone is refused and points at --terminal.
    /// 帳票は常に図面全体が対象のため、--sheetだけの指定は--terminalを案内して拒否する。
    #[test]
    fn export_rejects_sheet_only_narrowing_for_reports() {
        let err = run(
            &parse(&[
                "madake",
                "export",
                "terminal-chart",
                "/tmp/tb.csv",
                "--sheet",
                "s1",
            ]),
            &FakeApi::default(),
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("--terminal"), "{msg}");
    }

    /// --format is refused for schematic exports (svg/pdf/pdf-book), which are not reports.
    /// 帳票ではない回路図の出力(svg/pdf/pdf-book)に--formatを付けると拒否する。
    #[test]
    fn export_rejects_format_option_on_schematic_exports() {
        let err = run(
            &parse(&["madake", "export", "svg", "/tmp/a.svg", "--format", "pdf"]),
            &FakeApi::default(),
        )
        .unwrap_err();
        assert!(matches!(err, CliError::Usage(_)), "{err:?}");
    }

    /// madake status calls the health endpoint and the project snapshot.
    /// madake statusはヘルスチェックとプロジェクト概要を取得する。
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

    /// --json prints raw pretty-printed JSON for piping into jq and similar tools.
    /// --jsonはjq等へ渡せる整形JSONをそのまま出力する。
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

    /// madake netlist forwards the --sheet option to the API.
    /// madake netlistは--sheetオプションをAPIへ引き渡す。
    #[test]
    fn netlist_forwards_sheet_option() {
        let api = FakeApi {
            netlist: json!([]),
            ..Default::default()
        };
        run(&parse(&["madake", "netlist", "--sheet", "abc"]), &api).unwrap();
        assert_eq!(api.calls(), vec![r#"netlist(Some("abc"))"#]);
    }

    /// madake export forwards kind, output path and optional sheet to the API.
    /// madake exportは種別・出力パス・シート指定をAPIへ引き渡す。
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

    /// madake exec reads a JSON file containing a Command array and posts it to /commands.
    /// madake execはCommand配列のJSONファイルを読み、/commandsへ送信する。
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

    /// madake exec rejects JSON that is not an array, with a clear message.
    /// madake execは配列でないJSONを明確なメッセージで拒否する。
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

    /// A missing input file is reported as a file error, not a panic.
    /// 入力ファイルが無い場合はパニックせずファイルエラーとして報告する。
    #[test]
    fn exec_reports_missing_file() {
        let err = run(
            &parse(&["madake", "exec", "/nonexistent/madake-cli/none.json"]),
            &FakeApi::default(),
        )
        .unwrap_err();
        assert!(matches!(err, CliError::Io(_)), "{err:?}");
    }

    /// madake renumber numbers every sheet from 1 in top-up mode unless told otherwise.
    /// madake renumberは既定でプロジェクト全体を1から追い番で採番する。
    #[test]
    fn renumber_defaults_to_whole_project_append_from_one() {
        let api = FakeApi {
            result: json!([{ "revision": 5, "ops": [{ "op": "entity_upserted" }] }]),
            ..Default::default()
        };
        let out = run(&parse(&["madake", "renumber"]), &api).unwrap();
        let calls = api.calls();
        let sent: Value = serde_json::from_str(
            calls[0].trim_start_matches("exec(").trim_end_matches(')'),
        )
        .expect("送信したCommand");
        assert_eq!(
            sent,
            json!([{ "type": "renumber_wires", "mode": "append", "start": 1 }])
        );
        assert!(out.contains("線番を採番しました"), "{out}");
    }

    /// madake renumber passes the target sheet, the numbering mode and the start number through.
    /// madake renumberは対象シート・採番方式・開始番号をそのままコマンドへ載せる。
    #[test]
    fn renumber_forwards_sheet_mode_and_start() {
        let api = FakeApi {
            result: json!([{ "revision": 6, "ops": [] }]),
            ..Default::default()
        };
        run(
            &parse(&[
                "madake",
                "renumber",
                "--sheet",
                "s1",
                "--mode",
                "renumber",
                "--start",
                "100",
            ]),
            &api,
        )
        .unwrap();
        let calls = api.calls();
        let sent: Value = serde_json::from_str(
            calls[0].trim_start_matches("exec(").trim_end_matches(')'),
        )
        .expect("送信したCommand");
        assert_eq!(
            sent,
            json!([{ "type": "renumber_wires", "sheet_id": "s1", "mode": "renumber", "start": 100 }])
        );
    }

    /// madake renumber says so when every net already had a wire number.
    /// madake renumberは全ネットが採番済みで変更が無かったことを伝える。
    #[test]
    fn renumber_reports_when_nothing_changed() {
        let api = FakeApi {
            result: json!([{ "revision": 7, "ops": [] }]),
            ..Default::default()
        };
        let out = run(&parse(&["madake", "renumber"]), &api).unwrap();
        assert!(out.contains("線番の変更はありません"), "{out}");
    }

    /// madake undo tells the user when there is nothing to undo.
    /// madake undoは戻す操作が無いことをユーザーに伝える。
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

    /// madake redo reports the new document revision on success.
    /// madake redoは成功時に新しいドキュメントrevisionを報告する。
    #[test]
    fn redo_reports_revision() {
        let api = FakeApi {
            result: json!({ "revision": 9, "ops": [] }),
            ..Default::default()
        };
        let out = run(&parse(&["madake", "redo"]), &api).unwrap();
        assert!(out.contains("revision 9"));
    }

    /// madake save/open report the file path they acted on.
    /// madake save/openは対象のファイルパスを報告する。
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
