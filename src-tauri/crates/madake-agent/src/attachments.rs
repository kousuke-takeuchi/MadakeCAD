//! エージェントへの添付 (画像・PDF・表計算。.pen「添付デザイン - エージェントへ画像・PDFを添付」`s8nJcQ`)。
//!
//! 添付は1ターンのユーザー発言に付く。プロバイダごとの渡し方:
//! - Anthropic API / Gemini: 画像・PDFをそのまま (base64の内容ブロック)
//! - OpenAI互換: 画像だけ (`image_url`のdata URL)。PDFは読めないので送る前に断る
//! - Claude Code CLI: `--input-format stream-json`で内容ブロックごと標準入力へ流す
//! - Copilot CLI: 会話ごとの一時フォルダへ保存し、`--add-dir`で読める場所にしてパスを本文へ添える
//!
//! 表計算 (Excelの.xlsx・CSV) はどのプロバイダも直接は読めないので、送る直前にテキストへ直し
//! ([`spreadsheet_text`])、本文より前のテキストとして渡す (Copilot CLIは本文へ直接入れる)。
//!
//! 会話の保存 ([`crate::conversation::ChatMessage::attachments`]) には名前・種類・大きさだけを残し、
//! 中身は保存しない (次のターンで送り直すのも本文だけ)。

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::AgentError;

/// 1件の上限 (20MB)。
pub const MAX_ATTACHMENT_BYTES: usize = 20 * 1024 * 1024;
/// 1回の発言に付けられる件数。
pub const MAX_ATTACHMENTS: usize = 5;
/// 受け付ける種類 (MIME)。
pub const IMAGE_MIMES: [&str; 4] = ["image/png", "image/jpeg", "image/gif", "image/webp"];
pub const PDF_MIME: &str = "application/pdf";
pub const XLSX_MIME: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
pub const CSV_MIME: &str = "text/csv";
/// 表計算1件をテキストにしたときの上限 (文字数)。超えた分は行の切れ目で省く。
pub const MAX_SPREADSHEET_CHARS: usize = 200_000;
const UNSUPPORTED: &str = "画像 (PNG/JPEG/GIF/WebP)・PDF・Excel (.xlsx)・CSVだけ添付できます";

/// 添付1件 (中身つき)。
#[derive(Debug, Clone, PartialEq)]
pub struct Attachment {
    /// 表示とCLIへの保存に使うファイル名。
    pub name: String,
    pub mime: String,
    pub data: Vec<u8>,
}

/// 会話に残す添付の情報 (中身は持たない)。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentMeta {
    pub name: String,
    pub mime: String,
    pub size: u64,
}

/// 添付の種類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentKind {
    Image,
    Pdf,
    /// Excel (.xlsx) かCSV。テキストに直して渡す。
    Spreadsheet,
}

impl Attachment {
    pub fn kind(&self) -> Option<AttachmentKind> {
        kind_of(&self.mime)
    }

    pub fn meta(&self) -> AttachmentMeta {
        AttachmentMeta { name: self.name.clone(), mime: self.mime.clone(), size: self.data.len() as u64 }
    }

    fn base64(&self) -> String {
        base64::engine::general_purpose::STANDARD.encode(&self.data)
    }
}

/// MIMEから種類を引く (受け付けない種類はNone)。
pub fn kind_of(mime: &str) -> Option<AttachmentKind> {
    if IMAGE_MIMES.contains(&mime) {
        Some(AttachmentKind::Image)
    } else if mime == PDF_MIME {
        Some(AttachmentKind::Pdf)
    } else if mime == XLSX_MIME || mime == CSV_MIME {
        Some(AttachmentKind::Spreadsheet)
    } else {
        None
    }
}

/// 拡張子からMIMEを推す (ファイルを選んで添付したとき)。分からなければNone。
pub fn mime_from_name(name: &str) -> Option<&'static str> {
    let ext = Path::new(name).extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "pdf" => PDF_MIME,
        "xlsx" => XLSX_MIME,
        "csv" => CSV_MIME,
        _ => return None,
    })
}

/// base64の中身から添付を作る (貼り付け・ブラウザ検証モード)。
pub fn from_base64(name: &str, mime: &str, data: &str) -> Result<Attachment, AgentError> {
    let data = base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|e| AgentError::Attachment(format!("{name}: 中身を読めません ({e})")))?;
    Ok(Attachment { name: name.to_string(), mime: mime.to_string(), data })
}

/// ファイルから添付を作る (ファイル選択・ドラッグ&ドロップ)。種類は拡張子で決める。
pub fn from_path(path: &Path) -> Result<Attachment, AgentError> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| AgentError::Attachment(format!("ファイル名を読めません: {}", path.display())))?
        .to_string();
    let mime = mime_from_name(&name)
        .ok_or_else(|| AgentError::Attachment(format!("{name}: {UNSUPPORTED}")))?;
    let size = std::fs::metadata(path)?.len();
    if size as usize > MAX_ATTACHMENT_BYTES {
        return Err(too_large(&name, size as usize));
    }
    Ok(Attachment { name, mime: mime.to_string(), data: std::fs::read(path)? })
}

fn too_large(name: &str, size: usize) -> AgentError {
    AgentError::Attachment(format!(
        "{name}: 大きすぎます ({:.1}MB > {}MB)",
        size as f64 / 1_048_576.0,
        MAX_ATTACHMENT_BYTES / 1_048_576
    ))
}

/// 画面・Link APIから届く添付の指定。ファイルのパス (ファイル選択・ドラッグ&ドロップ) か、
/// base64の中身 (貼り付け・ブラウザ検証モード) のどちらか。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentInput {
    /// 表示名 (パス指定ならファイル名で上書きしない)。
    #[serde(default)]
    pub name: String,
    /// 種類 (base64のときは必須。パスなら拡張子から決める)。
    #[serde(default)]
    pub mime: String,
    #[serde(default)]
    pub path: Option<String>,
    /// base64の中身。
    #[serde(default)]
    pub data: Option<String>,
}

impl AttachmentInput {
    /// 中身を読み込んで添付にする。パスとbase64のどちらも無ければエラー。
    pub fn resolve(&self) -> Result<Attachment, AgentError> {
        match (&self.path, &self.data) {
            (Some(path), _) => {
                let mut a = from_path(Path::new(path))?;
                if !self.name.trim().is_empty() {
                    a.name = self.name.trim().to_string();
                }
                Ok(a)
            }
            (None, Some(data)) => from_base64(self.name.trim(), &self.mime, data),
            (None, None) => Err(AgentError::Attachment(format!("{}: 中身がありません", self.name))),
        }
    }
}

/// 指定の並びをまとめて読み込む (1件でも読めなければ全体を断る)。
pub fn resolve_all(inputs: &[AttachmentInput]) -> Result<Vec<Attachment>, AgentError> {
    let attachments = inputs.iter().map(AttachmentInput::resolve).collect::<Result<Vec<_>, _>>()?;
    validate(&attachments)?;
    Ok(attachments)
}

/// 選んだ・落としたファイルの下見 (中身は読まない)。入力欄のチップに名前・種類・大きさと、
/// 添付できない理由を出すために使う。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentInfo {
    pub path: String,
    pub name: String,
    /// 種類 (添付できない種類なら空)。
    pub mime: String,
    pub size: u64,
    /// 添付できない理由 (種類・大きさ・読めない)。添付できるならNone。
    pub error: Option<String>,
}

/// ファイルを下見する。
pub fn inspect_path(path: &Path) -> AttachmentInfo {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
    let mime = mime_from_name(&name).unwrap_or_default().to_string();
    let (size, mut error) = match std::fs::metadata(path) {
        Ok(m) if m.is_file() => (m.len(), None),
        Ok(_) => (0, Some(format!("{name}: ファイルではありません"))),
        Err(e) => (0, Some(format!("{name}: 読めません ({e})"))),
    };
    if error.is_none() && mime.is_empty() {
        error = Some(format!("{name}: {UNSUPPORTED}"));
    }
    if error.is_none() && size as usize > MAX_ATTACHMENT_BYTES {
        error = Some(too_large(&name, size as usize).to_string());
    }
    AttachmentInfo { path: path.display().to_string(), name, mime, size, error }
}

/// 送る前の確かめ: 件数・種類・大きさ。どれか1つでも外れたら全体を断る。
pub fn validate(attachments: &[Attachment]) -> Result<(), AgentError> {
    if attachments.len() > MAX_ATTACHMENTS {
        return Err(AgentError::Attachment(format!(
            "添付は1回{MAX_ATTACHMENTS}件までです ({}件)",
            attachments.len()
        )));
    }
    for a in attachments {
        if a.kind().is_none() {
            return Err(AgentError::Attachment(format!("{}: {UNSUPPORTED}", a.name)));
        }
        if a.data.len() > MAX_ATTACHMENT_BYTES {
            return Err(too_large(&a.name, a.data.len()));
        }
    }
    Ok(())
}

/// Anthropic Messages API (Claude Code CLIのstream-json入力も同じ形) の内容ブロック。
/// 添付を先に、本文を最後に置く (画像・資料を読んでから指示を読ませる)。
/// 表計算はテキストブロックにする (読めないブックはここで断る)。
pub fn anthropic_content(prompt: &str, attachments: &[Attachment]) -> Result<serde_json::Value, AgentError> {
    let mut blocks = Vec::new();
    for a in attachments {
        blocks.push(match a.kind() {
            Some(AttachmentKind::Spreadsheet) => {
                let text = spreadsheet_text(a)?;
                serde_json::json!({ "type": "text", "text": text })
            }
            Some(AttachmentKind::Pdf) => serde_json::json!({
                "type": "document",
                "source": { "type": "base64", "media_type": a.mime, "data": a.base64() },
            }),
            _ => serde_json::json!({
                "type": "image",
                "source": { "type": "base64", "media_type": a.mime, "data": a.base64() },
            }),
        });
    }
    blocks.push(serde_json::json!({ "type": "text", "text": prompt }));
    Ok(serde_json::Value::Array(blocks))
}

/// Gemini の`parts` (添付は`inline_data`、表計算はテキストの部品)。
pub fn gemini_parts(prompt: &str, attachments: &[Attachment]) -> Result<serde_json::Value, AgentError> {
    let mut parts = Vec::new();
    for a in attachments {
        parts.push(if a.kind() == Some(AttachmentKind::Spreadsheet) {
            let text = spreadsheet_text(a)?;
            serde_json::json!({ "text": text })
        } else {
            serde_json::json!({ "inline_data": { "mime_type": a.mime, "data": a.base64() } })
        });
    }
    parts.push(serde_json::json!({ "text": prompt }));
    Ok(serde_json::Value::Array(parts))
}

/// OpenAI互換の`content`。PDFは読めないので断る (画面でも送る前に止める)。表計算はテキストの部品。
pub fn openai_content(prompt: &str, attachments: &[Attachment]) -> Result<serde_json::Value, AgentError> {
    if let Some(pdf) = attachments.iter().find(|a| a.kind() == Some(AttachmentKind::Pdf)) {
        return Err(AgentError::Attachment(format!(
            "{}: OpenAI互換のモデルはPDFを読めません。画像だけ送るか、モデルを替えてください",
            pdf.name
        )));
    }
    let mut parts = Vec::new();
    for a in attachments {
        parts.push(if a.kind() == Some(AttachmentKind::Spreadsheet) {
            let text = spreadsheet_text(a)?;
            serde_json::json!({ "type": "text", "text": text })
        } else {
            serde_json::json!({
                "type": "image_url",
                "image_url": { "url": format!("data:{};base64,{}", a.mime, a.base64()) },
            })
        });
    }
    parts.push(serde_json::json!({ "type": "text", "text": prompt }));
    Ok(serde_json::Value::Array(parts))
}

/// Claude Code CLI の`--input-format stream-json`へ流す1行 (改行つき)。
pub fn claude_cli_input_line(prompt: &str, attachments: &[Attachment]) -> Result<String, AgentError> {
    let content = anthropic_content(prompt, attachments)?;
    let message = serde_json::json!({
        "type": "user",
        "message": { "role": "user", "content": content },
    });
    Ok(format!("{message}\n"))
}

/// 本文の前へ、表計算の添付をテキストにして入れる (本文しか受け取らないCopilot CLI向け)。
/// 表計算以外の添付は[`save_to_dir`]・[`prompt_with_paths`]で渡す。
pub fn prompt_with_spreadsheets(prompt: &str, attachments: &[Attachment]) -> Result<String, AgentError> {
    let mut out = String::new();
    for a in attachments.iter().filter(|a| a.kind() == Some(AttachmentKind::Spreadsheet)) {
        out.push_str(&spreadsheet_text(a)?);
        out.push('\n');
    }
    out.push_str(prompt);
    Ok(out)
}

/// 表計算の添付をモデルへ渡すテキストにする。
///
/// - Excel (.xlsx): シートごとに`## シート: 名前`の見出しを付け、1行1行・列はタブ区切り。
///   空の行と末尾の空の列は省き、日付の書式のセルは日付で書く (数式は保存されている計算結果)
/// - CSV: 先頭のBOMを除いた本文そのまま (UTF-8だけ。Shift_JISなどは保存し直してもらう)
pub fn spreadsheet_text(a: &Attachment) -> Result<String, AgentError> {
    if a.mime == CSV_MIME {
        let text = std::str::from_utf8(&a.data).map_err(|_| {
            AgentError::Attachment(format!("{}: UTF-8で保存したCSVだけ読めます (Excelなら「CSV UTF-8」で保存し直してください)", a.name))
        })?;
        return Ok(format!("添付ファイル「{}」の内容 (CSV):\n{}", a.name, text.trim_start_matches('\u{feff}')));
    }
    let body = workbook_text(&a.data, MAX_SPREADSHEET_CHARS)
        .map_err(|e| AgentError::Attachment(format!("{}: Excelのブックとして読めません ({e})", a.name)))?;
    Ok(format!("添付ファイル「{}」の内容 (表計算をテキストにしたもの。列はタブ区切り):\n{body}", a.name))
}

/// .xlsxの中身を、シートごとの見出し+タブ区切りの行にする。`limit`文字を超える分は行の切れ目で省く。
fn workbook_text(data: &[u8], limit: usize) -> Result<String, calamine::XlsxError> {
    use calamine::{DataType as _, Reader as _};
    let mut book: calamine::Xlsx<_> = calamine::open_workbook_from_rs(std::io::Cursor::new(data))?;
    let mut out = String::new();
    let mut omitted = 0usize;
    for (i, name) in book.sheet_names().into_iter().enumerate() {
        let range = book.worksheet_range(&name)?;
        if omitted > 0 {
            omitted += range.rows().filter(|r| r.iter().any(|c| !c.is_empty())).count();
            continue;
        }
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&format!("## シート: {name}\n"));
        for row in range.rows() {
            let mut cells: Vec<String> = row.iter().map(cell_text).collect();
            while cells.last().is_some_and(|c| c.is_empty()) {
                cells.pop();
            }
            if cells.is_empty() {
                continue;
            }
            let line = cells.join("\t");
            if omitted > 0 || out.chars().count() + line.chars().count() + 1 > limit {
                omitted += 1;
                continue;
            }
            out.push_str(&line);
            out.push('\n');
        }
    }
    if omitted > 0 {
        out.push_str(&format!("…以下省略 ({omitted} 行)\n"));
    }
    Ok(out)
}

/// セル1つの文字。整数の浮動小数は「.0」を付けず、日付の書式は`YYYY-MM-DD` (時刻があれば` HH:MM`)。
/// タブ・改行は1行1行・タブ区切りを崩すので空白にする。
fn cell_text(cell: &calamine::Data) -> String {
    use calamine::Data;
    let text = match cell {
        Data::Float(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", *f as i64),
        Data::DateTime(d) if d.is_datetime() => {
            let (y, mo, da, h, mi, se, _) = d.to_ymd_hms_milli();
            match (h, mi, se) {
                (0, 0, 0) => format!("{y:04}-{mo:02}-{da:02}"),
                (_, _, 0) => format!("{y:04}-{mo:02}-{da:02} {h:02}:{mi:02}"),
                _ => format!("{y:04}-{mo:02}-{da:02} {h:02}:{mi:02}:{se:02}"),
            }
        }
        other => other.to_string(),
    };
    text.replace(['\t', '\r', '\n'], " ")
}

/// ファイル名として安全な名前 (区切り文字・制御文字を`_`に。空なら`attachment`)。
pub fn safe_file_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c == '/' || c == '\\' || c == ':' || c.is_control() { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.').to_string();
    if trimmed.is_empty() {
        "attachment".to_string()
    } else {
        trimmed
    }
}

/// 添付をフォルダへ保存し、保存したパスを返す (CLIに読ませるため)。名前が重なれば番号を足す。
pub fn save_to_dir(dir: &Path, attachments: &[Attachment]) -> Result<Vec<PathBuf>, AgentError> {
    std::fs::create_dir_all(dir)?;
    let mut paths = Vec::new();
    for a in attachments {
        let name = safe_file_name(&a.name);
        let mut path = dir.join(&name);
        let mut n = 2;
        while path.exists() || paths.contains(&path) {
            let stem = Path::new(&name).file_stem().and_then(|s| s.to_str()).unwrap_or("attachment");
            let ext = Path::new(&name).extension().and_then(|s| s.to_str()).map(|e| format!(".{e}")).unwrap_or_default();
            path = dir.join(format!("{stem}-{n}{ext}"));
            n += 1;
        }
        std::fs::write(&path, &a.data)?;
        paths.push(path);
    }
    Ok(paths)
}

/// 本文の後ろへ、保存した添付のパスを足す (CLIのツールで読ませる)。
pub fn prompt_with_paths(prompt: &str, paths: &[PathBuf]) -> String {
    if paths.is_empty() {
        return prompt.to_string();
    }
    let lines: Vec<String> = paths.iter().map(|p| format!("- {}", p.display())).collect();
    format!("{prompt}\n\n添付ファイル (読み込んで参照してください):\n{}", lines.join("\n"))
}

/// 会話ごとの添付の一時フォルダ (OSの一時フォルダの下)。
pub fn conversation_dir(conversation: uuid::Uuid) -> PathBuf {
    std::env::temp_dir().join("madake-attachments").join(conversation.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(name: &str) -> Attachment {
        Attachment { name: name.into(), mime: "image/png".into(), data: vec![1, 2, 3] }
    }
    fn pdf(name: &str) -> Attachment {
        Attachment { name: name.into(), mime: PDF_MIME.into(), data: b"%PDF".to_vec() }
    }
    fn csv(name: &str, data: &[u8]) -> Attachment {
        Attachment { name: name.into(), mime: CSV_MIME.into(), data: data.to_vec() }
    }

    /// Smallest xlsx Excel opens: one worksheet per `(name, sheetData rows)`, with cell style 1 formatted as a date.
    /// Cells are written as `<c r="A1" t="inlineStr"><is><t>..</t></is></c>` or `<c r="B1"><v>..</v></c>`.
    fn xlsx(sheets: &[(&str, &str)]) -> Vec<u8> {
        use std::io::Write as _;
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let mut put = |path: &str, body: String| {
            zip.start_file(path, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        };
        let overrides: String = (1..=sheets.len())
            .map(|i| format!(r#"<Override PartName="/xl/worksheets/sheet{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>"#))
            .collect();
        put("[Content_Types].xml", format!(r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>{overrides}</Types>"#));
        put("_rels/.rels", r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.into());
        let list: String = sheets
            .iter()
            .enumerate()
            .map(|(i, (name, _))| format!(r#"<sheet name="{name}" sheetId="{n}" r:id="rId{n}"/>"#, n = i + 1))
            .collect();
        put("xl/workbook.xml", format!(r#"<?xml version="1.0" encoding="UTF-8"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets>{list}</sheets></workbook>"#));
        let rels: String = (1..=sheets.len())
            .map(|i| format!(r#"<Relationship Id="rId{i}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet{i}.xml"/>"#))
            .collect();
        put("xl/_rels/workbook.xml.rels", format!(r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rels}<Relationship Id="rIdS" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#));
        put("xl/styles.xml", r#"<?xml version="1.0" encoding="UTF-8"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><cellXfs count="2"><xf numFmtId="0"/><xf numFmtId="14" applyNumberFormat="1"/></cellXfs></styleSheet>"#.into());
        for (i, (_, rows)) in sheets.iter().enumerate() {
            put(&format!("xl/worksheets/sheet{}.xml", i + 1), format!(r#"<?xml version="1.0" encoding="UTF-8"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>{rows}</sheetData></worksheet>"#));
        }
        zip.finish().unwrap().into_inner()
    }
    fn s(r: &str, text: &str) -> String {
        format!(r#"<c r="{r}" t="inlineStr"><is><t>{text}</t></is></c>"#)
    }
    fn n(r: &str, v: &str) -> String {
        format!(r#"<c r="{r}"><v>{v}</v></c>"#)
    }
    fn row(i: u32, cells: &[String]) -> String {
        format!(r#"<row r="{i}">{}</row>"#, cells.concat())
    }
    fn sheet_file(name: &str, data: Vec<u8>) -> Attachment {
        Attachment { name: name.into(), mime: XLSX_MIME.into(), data }
    }

    /// An Excel workbook becomes text: every sheet in order under its own heading, one row per line with tab-separated columns, empty rows left out, whole numbers without ".0", and date cells written as dates.
    /// Excelのブックはテキストになる: シートごとに見出しを付けて順に並べ、1行1行・列はタブ区切り。空の行は省き、整数は「.0」を付けず、日付のセルは日付で書く。
    #[test]
    fn an_excel_workbook_becomes_tab_separated_text_per_sheet() {
        let bom = [
            row(1, &[s("A1", "部品表"), s("B1", "")]),
            row(2, &[s("A2", "作成日"), r#"<c r="B2" s="1"><v>46302</v></c>"#.to_string()]),
            row(4, &[s("A4", "No"), s("B4", "名称"), s("C4", "数量")]),
            row(5, &[n("A5", "1"), s("B5", "USB-C\tケーブル\n0.5m"), n("C5", "2.5")]),
        ]
        .concat();
        let history = row(1, &[s("A1", "版数"), n("B1", "2")]);
        let text = spreadsheet_text(&sheet_file("BOM.xlsx", xlsx(&[("BOM", &bom), ("改訂履歴", &history)]))).unwrap();
        assert_eq!(
            text,
            "添付ファイル「BOM.xlsx」の内容 (表計算をテキストにしたもの。列はタブ区切り):\n\
             ## シート: BOM\n\
             部品表\n\
             作成日\t2026-10-07\n\
             No\t名称\t数量\n\
             1\tUSB-C ケーブル 0.5m\t2.5\n\
             \n\
             ## シート: 改訂履歴\n\
             版数\t2\n"
        );
    }

    /// A workbook too long to send whole is cut at a row boundary and says how many rows were left out.
    /// 長すぎて全部は送れないブックは行の切れ目で切り、省いた行数を書き添える。
    #[test]
    fn a_long_workbook_is_cut_and_says_how_many_rows_were_left_out() {
        let rows: String = (1..=10).map(|i| row(i, &[s(&format!("A{i}"), "0123456789")])).collect();
        let text = workbook_text(&xlsx(&[("長い", &rows)]), 60).unwrap();
        assert!(text.starts_with("## シート: 長い\n0123456789\n"));
        let kept = text.matches("0123456789").count();
        assert!(kept < 10, "{text}");
        assert!(text.ends_with(&format!("…以下省略 ({} 行)\n", 10 - kept)), "{text}");
    }

    /// A file that is not a readable workbook is refused with its file name, and nothing is sent.
    /// ブックとして読めないファイルはファイル名を添えて断り、何も送らない。
    #[test]
    fn a_broken_workbook_is_refused_with_its_name() {
        let err = spreadsheet_text(&sheet_file("壊れた.xlsx", b"not a zip".to_vec())).unwrap_err().to_string();
        assert!(err.contains("壊れた.xlsx"), "{err}");
        let err = anthropic_content("見て", &[sheet_file("壊れた.xlsx", b"not a zip".to_vec())]).unwrap_err().to_string();
        assert!(err.contains("壊れた.xlsx"), "{err}");
    }

    /// A CSV file is sent as its text with a leading byte order mark removed; a CSV that is not UTF-8 (for example Shift_JIS) is refused with a hint to save it as UTF-8.
    /// CSVは先頭のBOMを除いた本文をそのまま送る。UTF-8でないCSV (Shift_JISなど) は「UTF-8で保存し直す」案内を付けて断る。
    #[test]
    fn a_csv_is_sent_as_text_and_must_be_utf8() {
        let text = spreadsheet_text(&csv("部品.csv", "\u{feff}No,名称\n1,ヒューズ\n".as_bytes())).unwrap();
        assert_eq!(text, "添付ファイル「部品.csv」の内容 (CSV):\nNo,名称\n1,ヒューズ\n");
        let sjis = [0x95, 0x94, 0x95, 0x69]; // 「部品」のShift_JIS
        let err = spreadsheet_text(&csv("部品.csv", &sjis)).unwrap_err().to_string();
        assert!(err.contains("部品.csv") && err.contains("UTF-8"), "{err}");
    }

    /// Every provider gets a spreadsheet as plain text placed before the message: a text block for Anthropic and the Claude Code CLI, a text part for Gemini and OpenAI-compatible models (so models that cannot read PDFs can still read Excel), and inline in the message for the Copilot CLI.
    /// 表計算はどのプロバイダにも本文より前のテキストとして渡す: Anthropic・Claude Code CLIはテキストブロック、Gemini・OpenAI互換はテキスト部品 (PDFを読めないモデルでもExcelは読める)、Copilot CLIは本文に直接入れる。
    #[test]
    fn every_provider_gets_a_spreadsheet_as_text_before_the_message() {
        let sheet = csv("a.csv", b"x,y\n");
        let anthropic = anthropic_content("見て", &[png("p.png"), sheet.clone()]).unwrap();
        assert_eq!(anthropic[0]["type"], "image");
        assert_eq!(anthropic[1]["type"], "text");
        assert!(anthropic[1]["text"].as_str().unwrap().ends_with("x,y\n"));
        assert_eq!(anthropic[2]["text"], "見て");
        let gemini = gemini_parts("見て", std::slice::from_ref(&sheet)).unwrap();
        assert!(gemini[0]["text"].as_str().unwrap().contains("a.csv"));
        assert_eq!(gemini[1]["text"], "見て");
        let openai = openai_content("見て", std::slice::from_ref(&sheet)).unwrap();
        assert_eq!(openai[0]["type"], "text");
        assert!(openai[0]["text"].as_str().unwrap().ends_with("x,y\n"));
        let cli = claude_cli_input_line("見て", std::slice::from_ref(&sheet)).unwrap();
        assert!(cli.contains("x,y"));
        let copilot = prompt_with_spreadsheets("見て", &[png("p.png"), sheet]).unwrap();
        assert!(copilot.starts_with("添付ファイル「a.csv」の内容 (CSV):\nx,y\n\n"), "{copilot}");
        assert!(copilot.ends_with("見て"));
        assert_eq!(prompt_with_spreadsheets("見て", &[png("p.png")]).unwrap(), "見て");
    }

    /// Images (PNG/JPEG/GIF/WebP), PDFs, Excel workbooks (.xlsx) and CSV files can be attached; anything else, more than five files or a file over 20MB is refused as a whole.
    /// 画像 (PNG/JPEG/GIF/WebP)・PDF・Excel (.xlsx)・CSVは添付でき、それ以外の種類・6件以上・20MBを超えるファイルは全体を断る。
    #[test]
    fn only_images_pdfs_and_spreadsheets_within_the_limits_are_accepted() {
        assert!(validate(&[png("a.png"), pdf("b.pdf"), csv("c.csv", b"x"), sheet_file("d.xlsx", vec![0])]).is_ok());
        let zip = Attachment { name: "c.zip".into(), mime: "application/zip".into(), data: vec![0] };
        assert!(validate(&[zip]).is_err());
        let six: Vec<_> = (0..6).map(|i| png(&format!("{i}.png"))).collect();
        assert!(validate(&six).unwrap_err().to_string().contains("5件"));
        let big = Attachment { name: "big.pdf".into(), mime: PDF_MIME.into(), data: vec![0; MAX_ATTACHMENT_BYTES + 1] };
        assert!(validate(&[big]).unwrap_err().to_string().contains("大きすぎます"));
    }

    /// The kind of a picked file comes from its extension, case-insensitively; unknown extensions are not attachable.
    /// 選んだファイルの種類は拡張子 (大文字小文字は問わない) で決まり、知らない拡張子は添付できない。
    #[test]
    fn the_kind_of_a_picked_file_comes_from_its_extension() {
        assert_eq!(mime_from_name("回路写真.JPG"), Some("image/jpeg"));
        assert_eq!(mime_from_name("データシート.pdf"), Some(PDF_MIME));
        assert_eq!(mime_from_name("AOGU_COCO_BOM.XLSX"), Some(XLSX_MIME));
        assert_eq!(mime_from_name("部品.csv"), Some(CSV_MIME));
        assert_eq!(mime_from_name("notes.txt"), None);
        assert_eq!(mime_from_name("old.xls"), None);
    }

    /// For the Anthropic API an image becomes an image block and a PDF a document block, both before the text.
    /// Anthropic APIへは画像を画像ブロック、PDFを文書ブロックにして、本文より前に置く。
    #[test]
    fn anthropic_gets_image_and_document_blocks_before_the_text() {
        let content = anthropic_content("照合して", &[png("a.png"), pdf("b.pdf")]).unwrap();
        assert_eq!(content[0]["type"], "image");
        assert_eq!(content[0]["source"]["media_type"], "image/png");
        assert_eq!(content[0]["source"]["data"], "AQID");
        assert_eq!(content[1]["type"], "document");
        assert_eq!(content[2], serde_json::json!({ "type": "text", "text": "照合して" }));
    }

    /// Gemini receives attachments as inline data parts followed by the text part.
    /// Geminiへは添付をinline_dataの部品にして、本文の部品の前に置く。
    #[test]
    fn gemini_gets_inline_data_parts() {
        let parts = gemini_parts("要約して", &[pdf("b.pdf")]).unwrap();
        assert_eq!(parts[0]["inline_data"]["mime_type"], PDF_MIME);
        assert_eq!(parts[1]["text"], "要約して");
    }

    /// An OpenAI-compatible model gets images as data URLs, and a PDF is refused before anything is sent.
    /// OpenAI互換のモデルへは画像をdata URLで渡し、PDFは何も送る前に断る。
    #[test]
    fn openai_gets_data_url_images_and_refuses_pdfs() {
        let content = openai_content("見て", &[png("a.png")]).unwrap();
        assert_eq!(content[0]["image_url"]["url"], "data:image/png;base64,AQID");
        assert!(openai_content("見て", &[pdf("b.pdf")]).unwrap_err().to_string().contains("PDF"));
    }

    /// The Claude Code CLI gets one stream-json user message line carrying the same content blocks.
    /// Claude Code CLIへは同じ内容ブロックを持つstream-jsonのユーザーメッセージを1行で渡す。
    #[test]
    fn claude_cli_gets_one_stream_json_line() {
        let line = claude_cli_input_line("照合して", &[png("a.png")]).unwrap();
        assert!(line.ends_with('\n'));
        let v: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(v["type"], "user");
        assert_eq!(v["message"]["role"], "user");
        assert_eq!(v["message"]["content"][0]["type"], "image");
        assert_eq!(v["message"]["content"][1]["text"], "照合して");
    }

    /// Saving for a CLI keeps the file names, makes path separators harmless and numbers duplicates, and the paths are listed after the text.
    /// CLI向けの保存はファイル名を保ち、区切り文字を無害にし、同名には番号を足す。保存先のパスは本文の後ろに並ぶ。
    #[test]
    fn saving_for_a_cli_keeps_safe_unique_names() {
        let dir = std::env::temp_dir().join(format!("madake-att-test-{}", uuid::Uuid::new_v4()));
        let paths = save_to_dir(&dir, &[png("../evil.png"), png("a.png"), png("a.png")]).unwrap();
        let names: Vec<_> = paths.iter().map(|p| p.file_name().unwrap().to_str().unwrap().to_string()).collect();
        assert_eq!(names, ["_evil.png", "a.png", "a-2.png"]);
        assert!(paths.iter().all(|p| p.starts_with(&dir) && std::fs::read(p).unwrap() == [1, 2, 3]));
        let prompt = prompt_with_paths("見て", &paths);
        assert!(prompt.starts_with("見て\n\n添付ファイル"));
        assert!(prompt.contains(&paths[2].display().to_string()));
        assert_eq!(prompt_with_paths("見て", &[]), "見て");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A picked file is read with its kind from the extension, and pasted base64 content is decoded; broken base64 is refused.
    /// 選んだファイルは拡張子の種類で読み込み、貼り付けのbase64は復号する。壊れたbase64は断る。
    #[test]
    fn files_and_pasted_content_become_attachments() {
        let dir = std::env::temp_dir().join(format!("madake-att-read-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("写真.png");
        std::fs::write(&path, [9, 9]).unwrap();
        let a = from_path(&path).unwrap();
        assert_eq!((a.name.as_str(), a.mime.as_str(), a.data.clone()), ("写真.png", "image/png", vec![9, 9]));
        std::fs::write(dir.join("memo.txt"), "x").unwrap();
        assert!(from_path(&dir.join("memo.txt")).is_err());
        assert_eq!(from_base64("貼り付け画像 1.png", "image/png", "AQID").unwrap().data, vec![1, 2, 3]);
        assert!(from_base64("x.png", "image/png", "@@@").is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Previewing a picked file reports its name, kind and size, and the reason when it cannot be attached (wrong kind or missing file).
    /// 選んだファイルの下見は名前・種類・大きさを返し、添付できないとき (種類違い・ファイルが無い) はその理由を返す。
    #[test]
    fn previewing_a_file_reports_its_kind_size_and_problems() {
        let dir = std::env::temp_dir().join(format!("madake-att-info-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("photo.jpeg"), [0u8; 10]).unwrap();
        std::fs::write(dir.join("notes.txt"), "x").unwrap();
        let ok = inspect_path(&dir.join("photo.jpeg"));
        assert_eq!((ok.name.as_str(), ok.mime.as_str(), ok.size, ok.error.clone()), ("photo.jpeg", "image/jpeg", 10, None));
        assert!(inspect_path(&dir.join("notes.txt")).error.unwrap().contains("だけ添付できます"));
        assert!(inspect_path(&dir.join("missing.png")).error.unwrap().contains("読めません"));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// An attachment given by the screen is read from its file path or decoded from base64, and one unreadable item refuses the whole message.
    /// 画面から届く添付はファイルのパスから読むかbase64を復号し、1件でも読めなければ発言全体を断る。
    #[test]
    fn inputs_from_the_screen_resolve_from_a_path_or_base64() {
        let dir = std::env::temp_dir().join(format!("madake-att-input-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("sheet.pdf"), b"%PDF").unwrap();
        let by_path = AttachmentInput { name: String::new(), mime: String::new(), path: Some(dir.join("sheet.pdf").display().to_string()), data: None };
        let pasted = AttachmentInput { name: "貼り付け画像 1.png".into(), mime: "image/png".into(), path: None, data: Some("AQID".into()) };
        let got = resolve_all(&[by_path, pasted.clone()]).unwrap();
        assert_eq!((got[0].name.as_str(), got[0].mime.as_str()), ("sheet.pdf", PDF_MIME));
        assert_eq!(got[1].data, vec![1, 2, 3]);
        let empty = AttachmentInput { name: "x".into(), mime: String::new(), path: None, data: None };
        assert!(resolve_all(&[pasted, empty]).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
