//! Link API クライアント。
//!
//! HTTP呼び出しは [`LinkApi`] トレイト越しに行うため、CLIのロジック
//! (引数解釈・出力整形) は実サーバー無しでテストできる。

use std::fmt;

use serde_json::{json, Value};

/// Link APIの既定ポート (アプリ内蔵サーバー)。
pub const DEFAULT_PORT: u16 = 9310;

/// `madake export` のエクスポート種別。
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ExportKind {
    /// シート1枚をSVGで出力。
    Svg,
    /// シート1枚をPDFで出力 (印刷品質)。
    Pdf,
    /// 部品表 (帳票)。
    Bom,
    /// From-To電線リスト (帳票)。
    #[value(name = "wire-list", alias = "from-to")]
    WireList,
    /// 端子台チャート (帳票。`--terminal`で1台に絞れる)。
    #[value(name = "terminal-chart")]
    TerminalChart,
    /// 端子接続図 (帳票。グラフィカルなのでPDFのみ。`--terminal`で1台に絞れる)。
    #[value(name = "terminal-diagram")]
    TerminalDiagram,
    /// クロスリファレンス表 (帳票)。
    #[value(name = "xref-table", alias = "xref")]
    XrefTable,
    /// PLC I/Oレポート (帳票)。
    #[value(name = "plc-io", alias = "plc")]
    PlcIo,
    /// 図面一式を1つのPDFへ (表紙+回路図全シート+選択帳票)。
    #[value(name = "pdf-book")]
    PdfBook,
}

impl ExportKind {
    /// `/api/v1` からの相対パス。帳票 ([`ExportKind::report_kind`] がSomeのもの) は
    /// 種類と形式をボディに載せる `/export/report` を使うため、ここには現れない。
    pub fn path(self) -> &'static str {
        match self {
            ExportKind::Svg => "/export/svg",
            ExportKind::Pdf => "/export/pdf",
            ExportKind::PdfBook => "/export/pdf-book",
            _ => "/export/report",
        }
    }

    /// 帳票ならその種類。回路図の出力 (svg/pdf/pdf-book) ならNone。
    pub fn report_kind(self) -> Option<ReportKind> {
        match self {
            ExportKind::Bom => Some(ReportKind::Bom),
            ExportKind::WireList => Some(ReportKind::WireList),
            ExportKind::TerminalChart => Some(ReportKind::TerminalChart),
            ExportKind::TerminalDiagram => Some(ReportKind::TerminalDiagram),
            ExportKind::XrefTable => Some(ReportKind::Xref),
            ExportKind::PlcIo => Some(ReportKind::PlcIo),
            ExportKind::Svg | ExportKind::Pdf | ExportKind::PdfBook => None,
        }
    }

    /// `--terminal` で対象を1つの端子台に絞れる帳票か。
    pub fn takes_terminal(self) -> bool {
        matches!(
            self,
            ExportKind::TerminalChart | ExportKind::TerminalDiagram
        )
    }

    /// 人間向けの表示名。
    pub fn label(self) -> &'static str {
        match self {
            ExportKind::Svg => "SVG",
            ExportKind::Pdf => "PDF",
            ExportKind::Bom => "部品表(BOM)",
            ExportKind::WireList => "From-To電線リスト",
            ExportKind::TerminalChart => "端子台チャート",
            ExportKind::TerminalDiagram => "端子接続図",
            ExportKind::XrefTable => "クロスリファレンス表",
            ExportKind::PlcIo => "PLC I/Oレポート",
            ExportKind::PdfBook => "図面一式PDF",
        }
    }
}

/// 帳票の出力形式 (`madake export ... --format`)。JSON表記はLink API・MCPと同じ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ReportFormat {
    /// 表計算ソフトで開けるCSV (UTF-8)。
    Csv,
    /// 図枠+表題欄付きのA4横ページを綴じたPDF。
    Pdf,
}

impl ReportFormat {
    /// Link APIへ載せるJSON表記。
    pub fn as_json(self) -> &'static str {
        match self {
            ReportFormat::Csv => "csv",
            ReportFormat::Pdf => "pdf",
        }
    }

    /// `--format` 省略時の既定。出力先の拡張子が `.pdf` ならPDF、それ以外はCSV。
    pub fn for_path(path: &str) -> Self {
        if path.to_ascii_lowercase().ends_with(".pdf") {
            ReportFormat::Pdf
        } else {
            ReportFormat::Csv
        }
    }

    /// 件数の単位 (CSV=行、PDF=ページ)。
    pub fn count_unit(self) -> &'static str {
        match self {
            ReportFormat::Csv => "行",
            ReportFormat::Pdf => "ページ",
        }
    }
}

/// `madake export pdf-book --reports` で選べる帳票。
/// 値の綴りはLink API・MCPのJSON表記と同じ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ReportKind {
    /// From-To電線リスト。
    #[value(name = "wire-list")]
    WireList,
    /// 端子台チャート。
    #[value(name = "terminal-chart")]
    TerminalChart,
    /// 端子接続図 (グラフィカル)。
    #[value(name = "terminal-diagram")]
    TerminalDiagram,
    /// 部品表。
    Bom,
    /// クロスリファレンス表。
    Xref,
    /// PLC I/Oレポート。
    #[value(name = "plc-io")]
    PlcIo,
}

impl ReportKind {
    /// Link APIへ載せるJSON表記。
    pub fn as_json(self) -> &'static str {
        match self {
            ReportKind::WireList => "wire-list",
            ReportKind::TerminalChart => "terminal-chart",
            ReportKind::TerminalDiagram => "terminal-diagram",
            ReportKind::Bom => "bom",
            ReportKind::Xref => "xref",
            ReportKind::PlcIo => "plc-io",
        }
    }
}

/// `madake renumber` の採番方式 (madake-coreの `RenumberMode` と同じJSON表記)。
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum RenumberMode {
    /// 追い番: 既存の線番・ネットラベルを保持し、未採番のネットにだけ番号を振る。
    Append,
    /// 振り直し: 自動採番済みの番号を捨てて全ネットを振り直す (手動で付けた名前は保持)。
    Renumber,
}

impl RenumberMode {
    /// `renumber_wires` コマンドへ載せるJSON表記。
    pub fn as_json(self) -> &'static str {
        match self {
            RenumberMode::Append => "append",
            RenumberMode::Renumber => "renumber",
        }
    }
}

#[derive(Debug)]
pub enum CliError {
    /// 接続拒否 = アプリ未起動。
    NotRunning { port: u16 },
    /// その他の通信エラー。
    Transport(String),
    /// Link APIが非2xxを返した。
    Api { status: u16, body: String },
    /// ローカルファイルの読み書き。
    Io(String),
    /// 入力JSONの不正。
    Json(String),
    /// オプションの組み合わせが正しくない (clapでは表せない条件)。
    Usage(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::NotRunning { port } => write!(
                f,
                "MadakeCADアプリが起動していません (127.0.0.1:{port} に接続できませんでした)。\n\
                 アプリを起動してから再実行してください。別ポートで起動している場合は --port を指定します。"
            ),
            CliError::Transport(msg) => write!(f, "Link APIとの通信に失敗しました: {msg}"),
            CliError::Api { status, body } => {
                write!(f, "Link APIがエラーを返しました (HTTP {status}): {body}")
            }
            CliError::Io(msg) => write!(f, "ファイル入出力エラー: {msg}"),
            CliError::Json(msg) => write!(f, "JSONが不正です: {msg}"),
            CliError::Usage(msg) => write!(f, "オプションの指定が正しくありません: {msg}"),
        }
    }
}

impl std::error::Error for CliError {}

/// 最小限のURLエンコード (クエリ値用)。
fn urlencoding_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// `http://127.0.0.1:<port>/api/v1`
pub fn base_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/api/v1")
}

/// `base_url` + 相対パス (`/project` など)。
pub fn endpoint(port: u16, path: &str) -> String {
    format!("{}{}", base_url(port), path)
}

/// `/netlist` のURL。シート指定時は `?sheet_id=<uuid>` を付ける
/// (Link API側のクエリ名は `sheet_id`)。
pub fn netlist_url(port: u16, sheet_id: Option<&str>) -> String {
    match sheet_id {
        Some(id) => format!("{}?sheet_id={}", endpoint(port, "/netlist"), id),
        None => endpoint(port, "/netlist"),
    }
}

/// `/verify` のURL。シート指定時は `?sheet_id=<uuid>` を付ける。
pub fn verify_url(port: u16, sheet_id: Option<&str>) -> String {
    match sheet_id {
        Some(id) => format!("{}?sheet_id={}", endpoint(port, "/verify"), id),
        None => endpoint(port, "/verify"),
    }
}

/// `/terminals` のURL。シート指定時は `?sheet_id=<uuid>` を付ける。
pub fn terminals_url(port: u16, sheet_id: Option<&str>) -> String {
    match sheet_id {
        Some(id) => format!("{}?sheet_id={}", endpoint(port, "/terminals"), id),
        None => endpoint(port, "/terminals"),
    }
}

/// 8-4-4-4-12桁の16進 = UUID表記か。`--terminal` が参照記号かentity idかの判別に使う。
pub fn looks_like_uuid(s: &str) -> bool {
    let groups = [8usize, 4, 4, 4, 12];
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == groups.len()
        && parts
            .iter()
            .zip(groups)
            .all(|(p, n)| p.len() == n && p.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Link APIの呼び出し口。テストではフェイク実装に差し替える。
pub trait LinkApi {
    /// `GET /api/v1`
    fn health(&self) -> Result<Value, CliError>;
    /// `GET /api/v1/project`
    fn project(&self) -> Result<Value, CliError>;
    /// `GET /api/v1/netlist[?sheet_id=..]`
    fn netlist(&self, sheet_id: Option<&str>) -> Result<Value, CliError>;
    /// `GET /api/v1/verify[?sheet_id=..]`
    fn verify(&self, sheet_id: Option<&str>) -> Result<Value, CliError>;
    /// `GET /api/v1/parts[?query=..&category=..]`
    fn parts(&self, query: Option<&str>, category: Option<&str>) -> Result<Value, CliError>;
    /// `POST /api/v1/simulate/op`
    fn simulate_op(&self, sheet_id: Option<&str>, open_switches: &[String]) -> Result<Value, CliError>;
    /// `POST /api/v1/commands` (Command配列 → Patch配列)
    fn exec(&self, commands: Value) -> Result<Value, CliError>;
    /// `POST /api/v1/undo`
    fn undo(&self) -> Result<Value, CliError>;
    /// `POST /api/v1/redo`
    fn redo(&self) -> Result<Value, CliError>;
    /// `POST /api/v1/save` (`{"path": ...}`)
    fn save(&self, path: &str) -> Result<Value, CliError>;
    /// `POST /api/v1/load` (`{"path": ...}`)
    fn open(&self, path: &str) -> Result<Value, CliError>;
    /// `POST /api/v1/import/kicad` (`{"path": ...}`)
    fn import_kicad(&self, path: &str) -> Result<Value, CliError>;
    /// `GET /api/v1/terminals[?sheet_id=..]` (端子台の一覧)
    fn terminals(&self, sheet_id: Option<&str>) -> Result<Value, CliError>;
    /// `POST /api/v1/export/{svg,pdf}`
    fn export(
        &self,
        kind: ExportKind,
        path: &str,
        sheet_id: Option<&str>,
    ) -> Result<Value, CliError>;
    /// `POST /api/v1/export/report` (`{"path":..., "kind":..., "format":..., "entity_id":...}`)
    fn export_report(
        &self,
        kind: ReportKind,
        format: ReportFormat,
        path: &str,
        entity_id: Option<&str>,
    ) -> Result<Value, CliError>;
    /// `POST /api/v1/export/pdf-book` (`{"path":..., "include_reports":[...], "cover":bool}`)
    fn export_pdf_book(
        &self,
        path: &str,
        reports: &[ReportKind],
        cover: bool,
    ) -> Result<Value, CliError>;
}

/// reqwest blocking による実装。
pub struct HttpClient {
    port: u16,
    http: reqwest::blocking::Client,
}

impl HttpClient {
    pub fn new(port: u16) -> Result<Self, CliError> {
        let http = reqwest::blocking::Client::builder()
            .build()
            .map_err(|e| CliError::Transport(e.to_string()))?;
        Ok(Self { port, http })
    }

    fn send(&self, req: reqwest::blocking::RequestBuilder) -> Result<Value, CliError> {
        let res = req.send().map_err(|e| self.map_send_error(&e))?;
        let status = res.status();
        let body = res.text().map_err(|e| CliError::Transport(e.to_string()))?;
        if !status.is_success() {
            return Err(CliError::Api {
                status: status.as_u16(),
                body: body.trim().to_string(),
            });
        }
        if body.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&body).map_err(|e| CliError::Json(e.to_string()))
    }

    /// 接続拒否 (アプリ未起動) を専用エラーに落とす。
    fn map_send_error(&self, err: &reqwest::Error) -> CliError {
        if err.is_connect() {
            CliError::NotRunning { port: self.port }
        } else {
            CliError::Transport(err.to_string())
        }
    }

    fn get(&self, url: String) -> Result<Value, CliError> {
        self.send(self.http.get(url))
    }

    fn post(&self, path: &str, body: Value) -> Result<Value, CliError> {
        self.send(self.http.post(endpoint(self.port, path)).json(&body))
    }
}

impl LinkApi for HttpClient {
    fn health(&self) -> Result<Value, CliError> {
        self.get(base_url(self.port))
    }

    fn project(&self) -> Result<Value, CliError> {
        self.get(endpoint(self.port, "/project"))
    }

    fn netlist(&self, sheet_id: Option<&str>) -> Result<Value, CliError> {
        self.get(netlist_url(self.port, sheet_id))
    }

    fn verify(&self, sheet_id: Option<&str>) -> Result<Value, CliError> {
        self.get(verify_url(self.port, sheet_id))
    }

    fn simulate_op(
        &self,
        sheet_id: Option<&str>,
        open_switches: &[String],
    ) -> Result<Value, CliError> {
        self.post(
            "/simulate/op",
            json!({ "sheet_id": sheet_id, "open_switches": open_switches }),
        )
    }

    fn parts(&self, query: Option<&str>, category: Option<&str>) -> Result<Value, CliError> {
        let mut url = format!("{}?", endpoint(self.port, "/parts"));
        if let Some(q) = query {
            url.push_str(&format!("query={}&", urlencoding_encode(q)));
        }
        if let Some(cat) = category {
            url.push_str(&format!("category={}", urlencoding_encode(cat)));
        }
        self.get(url.trim_end_matches(['?', '&']).to_string())
    }

    fn exec(&self, commands: Value) -> Result<Value, CliError> {
        self.post("/commands", commands)
    }

    fn undo(&self) -> Result<Value, CliError> {
        self.post("/undo", json!({}))
    }

    fn redo(&self) -> Result<Value, CliError> {
        self.post("/redo", json!({}))
    }

    fn save(&self, path: &str) -> Result<Value, CliError> {
        self.post("/save", json!({ "path": path }))
    }

    fn open(&self, path: &str) -> Result<Value, CliError> {
        self.post("/load", json!({ "path": path }))
    }

    fn import_kicad(&self, path: &str) -> Result<Value, CliError> {
        self.post("/import/kicad", json!({ "path": path }))
    }

    fn terminals(&self, sheet_id: Option<&str>) -> Result<Value, CliError> {
        self.get(terminals_url(self.port, sheet_id))
    }

    fn export(
        &self,
        kind: ExportKind,
        path: &str,
        sheet_id: Option<&str>,
    ) -> Result<Value, CliError> {
        // SVG/PDFのみシート指定を受け付ける (Link API: ExportSvgBody)。
        let body = match kind {
            ExportKind::Svg | ExportKind::Pdf => json!({ "sheet_id": sheet_id, "path": path }),
            _ => json!({ "path": path }),
        };
        self.post(kind.path(), body)
    }

    fn export_report(
        &self,
        kind: ReportKind,
        format: ReportFormat,
        path: &str,
        entity_id: Option<&str>,
    ) -> Result<Value, CliError> {
        self.post(
            "/export/report",
            json!({
                "path": path,
                "kind": kind.as_json(),
                "format": format.as_json(),
                "entity_id": entity_id,
            }),
        )
    }

    fn export_pdf_book(
        &self,
        path: &str,
        reports: &[ReportKind],
        cover: bool,
    ) -> Result<Value, CliError> {
        let include: Vec<&str> = reports.iter().map(|r| r.as_json()).collect();
        self.post(
            ExportKind::PdfBook.path(),
            json!({ "path": path, "include_reports": include, "cover": cover }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The CLI talks to http://127.0.0.1:<port>/api/v1 (loopback only).
    /// CLIは http://127.0.0.1:<ポート>/api/v1(ループバックのみ)へ接続する。
    #[test]
    fn base_url_uses_loopback_and_api_v1() {
        assert_eq!(base_url(9310), "http://127.0.0.1:9310/api/v1");
        assert_eq!(base_url(19310), "http://127.0.0.1:19310/api/v1");
    }

    /// endpoint() joins the base URL with a relative path.
    /// endpoint()はベースURLに相対パスを連結する。
    #[test]
    fn endpoint_appends_path() {
        assert_eq!(
            endpoint(9310, "/project"),
            "http://127.0.0.1:9310/api/v1/project"
        );
        assert_eq!(
            endpoint(9310, "/export/wire-list"),
            "http://127.0.0.1:9310/api/v1/export/wire-list"
        );
    }

    /// The netlist URL has no query string when no sheet is specified.
    /// シート未指定のときnetlist URLにクエリは付かない。
    #[test]
    fn netlist_url_omits_query_without_sheet() {
        assert_eq!(
            netlist_url(9310, None),
            "http://127.0.0.1:9310/api/v1/netlist"
        );
    }

    /// Sheet selection uses the sheet_id query parameter, matching the server.
    /// シート指定はサーバー側と同じsheet_idクエリ名を使う。
    #[test]
    fn netlist_url_uses_sheet_id_query_name() {
        assert_eq!(
            netlist_url(9310, Some("2f4e0b9a-0000-4000-8000-000000000001")),
            "http://127.0.0.1:9310/api/v1/netlist?sheet_id=2f4e0b9a-0000-4000-8000-000000000001"
        );
    }

    /// Schematic exports (svg/pdf/pdf-book) each have their own REST route, while every report goes to the shared /export/report route.
    /// 回路図の出力(svg/pdf/pdf-book)は専用ルートを持ち、帳票は共通の/export/reportへまとまる。
    #[test]
    fn export_kind_paths_match_link_api_routes() {
        assert_eq!(ExportKind::Svg.path(), "/export/svg");
        assert_eq!(ExportKind::Pdf.path(), "/export/pdf");
        assert_eq!(ExportKind::PdfBook.path(), "/export/pdf-book");
        assert_eq!(ExportKind::Bom.path(), "/export/report");
        assert_eq!(ExportKind::TerminalChart.path(), "/export/report");
    }

    /// The five report kinds map to the report names the Link API knows; schematic exports are not reports.
    /// 帳票5種はLink APIの帳票名へ対応付き、回路図の出力は帳票ではない。
    #[test]
    fn export_kinds_map_to_the_five_reports() {
        assert_eq!(ExportKind::WireList.report_kind(), Some(ReportKind::WireList));
        assert_eq!(
            ExportKind::TerminalChart.report_kind(),
            Some(ReportKind::TerminalChart)
        );
        assert_eq!(
            ExportKind::TerminalDiagram.report_kind(),
            Some(ReportKind::TerminalDiagram)
        );
        assert_eq!(ExportKind::Bom.report_kind(), Some(ReportKind::Bom));
        assert_eq!(ExportKind::XrefTable.report_kind(), Some(ReportKind::Xref));
        assert_eq!(ExportKind::Svg.report_kind(), None);
        assert_eq!(ExportKind::PdfBook.report_kind(), None);
    }

    /// Only the two terminal reports can be narrowed to a single terminal block.
    /// 対象を1つの端子台に絞れるのは端子台チャートと端子接続図だけ。
    #[test]
    fn only_terminal_reports_take_a_terminal_option() {
        assert!(ExportKind::TerminalChart.takes_terminal());
        assert!(ExportKind::TerminalDiagram.takes_terminal());
        assert!(!ExportKind::WireList.takes_terminal());
        assert!(!ExportKind::XrefTable.takes_terminal());
    }

    /// Without --format the output format follows the file extension: .pdf writes a PDF, anything else writes CSV.
    /// --format省略時は出力先の拡張子に従い、.pdfならPDF、それ以外はCSVになる。
    #[test]
    fn report_format_defaults_to_the_file_extension() {
        assert_eq!(ReportFormat::for_path("/tmp/chart.pdf"), ReportFormat::Pdf);
        assert_eq!(ReportFormat::for_path("/tmp/CHART.PDF"), ReportFormat::Pdf);
        assert_eq!(ReportFormat::for_path("/tmp/chart.csv"), ReportFormat::Csv);
        assert_eq!(ReportFormat::for_path("/tmp/chart"), ReportFormat::Csv);
    }

    /// The report format names sent to the Link API are "csv" and "pdf".
    /// Link APIへ送る出力形式の綴りは "csv" と "pdf"。
    #[test]
    fn report_format_json_names() {
        assert_eq!(ReportFormat::Csv.as_json(), "csv");
        assert_eq!(ReportFormat::Pdf.as_json(), "pdf");
    }

    /// The terminals URL uses the same sheet_id query name as the other endpoints.
    /// 端子台一覧のURLも他のエンドポイントと同じsheet_idクエリ名を使う。
    #[test]
    fn terminals_url_uses_sheet_id_query_name() {
        assert_eq!(
            terminals_url(9310, None),
            "http://127.0.0.1:9310/api/v1/terminals"
        );
        assert_eq!(
            terminals_url(9310, Some("s1")),
            "http://127.0.0.1:9310/api/v1/terminals?sheet_id=s1"
        );
    }

    /// A --terminal value is treated as an entity id only when it is spelled like a UUID; anything else is a reference designator.
    /// --terminalの値はUUID表記のときだけentity idとして扱い、それ以外は参照記号とみなす。
    #[test]
    fn uuid_shaped_values_are_recognized() {
        assert!(looks_like_uuid("2f4e0b9a-0000-4000-8000-000000000001"));
        assert!(!looks_like_uuid("TB1"));
        assert!(!looks_like_uuid("2f4e0b9a-0000-4000-8000"));
        assert!(!looks_like_uuid("2f4e0b9z-0000-4000-8000-000000000001"));
    }

    /// The report names sent for a PDF book are spelled the same as in the Link API and MCP JSON.
    /// PDF一括出力で送る帳票名の綴りは、Link API・MCPのJSON表記と同じになる。
    #[test]
    fn report_kind_json_names_match_the_link_api() {
        assert_eq!(ReportKind::WireList.as_json(), "wire-list");
        assert_eq!(ReportKind::TerminalChart.as_json(), "terminal-chart");
        assert_eq!(ReportKind::TerminalDiagram.as_json(), "terminal-diagram");
        assert_eq!(ReportKind::Bom.as_json(), "bom");
        assert_eq!(ReportKind::Xref.as_json(), "xref");
    }

    /// When the app is not running, the CLI explains it explicitly (with the port) instead of a cryptic error.
    /// アプリ未起動時は不可解なエラーではなく、ポート付きの明快なメッセージを出す。
    #[test]
    fn not_running_error_is_explicit() {
        let msg = CliError::NotRunning { port: 9310 }.to_string();
        assert!(msg.contains("MadakeCADアプリが起動していません"));
        assert!(msg.contains("9310"));
    }
}
