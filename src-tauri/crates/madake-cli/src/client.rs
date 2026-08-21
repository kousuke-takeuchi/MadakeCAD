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
    /// 部品表 (CSV)。
    Bom,
    /// 電線リスト (CSV)。
    #[value(name = "wire-list")]
    WireList,
}

impl ExportKind {
    /// `/api/v1` からの相対パス。
    pub fn path(self) -> &'static str {
        match self {
            ExportKind::Svg => "/export/svg",
            ExportKind::Pdf => "/export/pdf",
            ExportKind::Bom => "/export/bom",
            ExportKind::WireList => "/export/wire-list",
        }
    }

    /// 人間向けの表示名。
    pub fn label(self) -> &'static str {
        match self {
            ExportKind::Svg => "SVG",
            ExportKind::Pdf => "PDF",
            ExportKind::Bom => "部品表(BOM)",
            ExportKind::WireList => "電線リスト",
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
    /// `POST /api/v1/export/{svg,pdf,bom,wire-list}`
    fn export(
        &self,
        kind: ExportKind,
        path: &str,
        sheet_id: Option<&str>,
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

    /// Each export kind (svg/pdf/bom/wire-list) maps to its REST route.
    /// 各エクスポート種別(svg/pdf/bom/wire-list)は対応するRESTルートへ対応付く。
    #[test]
    fn export_kind_paths_match_link_api_routes() {
        assert_eq!(ExportKind::Svg.path(), "/export/svg");
        assert_eq!(ExportKind::Pdf.path(), "/export/pdf");
        assert_eq!(ExportKind::Bom.path(), "/export/bom");
        assert_eq!(ExportKind::WireList.path(), "/export/wire-list");
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
