//! エージェントへの添付 (画像・PDF。.pen「添付デザイン - エージェントへ画像・PDFを添付」`s8nJcQ`)。
//!
//! 添付は1ターンのユーザー発言に付く。プロバイダごとの渡し方:
//! - Anthropic API / Gemini: 画像・PDFをそのまま (base64の内容ブロック)
//! - OpenAI互換: 画像だけ (`image_url`のdata URL)。PDFは読めないので送る前に断る
//! - Claude Code CLI: `--input-format stream-json`で内容ブロックごと標準入力へ流す
//! - Copilot CLI: 会話ごとの一時フォルダへ保存し、`--add-dir`で読める場所にしてパスを本文へ添える
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
        .ok_or_else(|| AgentError::Attachment(format!("{name}: 画像 (PNG/JPEG/GIF/WebP) かPDFだけ添付できます")))?;
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
        error = Some(format!("{name}: 画像 (PNG/JPEG/GIF/WebP) かPDFだけ添付できます"));
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
            return Err(AgentError::Attachment(format!(
                "{}: 画像 (PNG/JPEG/GIF/WebP) かPDFだけ添付できます",
                a.name
            )));
        }
        if a.data.len() > MAX_ATTACHMENT_BYTES {
            return Err(too_large(&a.name, a.data.len()));
        }
    }
    Ok(())
}

/// Anthropic Messages API (Claude Code CLIのstream-json入力も同じ形) の内容ブロック。
/// 添付を先に、本文を最後に置く (画像・資料を読んでから指示を読ませる)。
pub fn anthropic_content(prompt: &str, attachments: &[Attachment]) -> serde_json::Value {
    let mut blocks: Vec<serde_json::Value> = attachments
        .iter()
        .map(|a| {
            let ty = if a.kind() == Some(AttachmentKind::Pdf) { "document" } else { "image" };
            serde_json::json!({
                "type": ty,
                "source": { "type": "base64", "media_type": a.mime, "data": a.base64() },
            })
        })
        .collect();
    blocks.push(serde_json::json!({ "type": "text", "text": prompt }));
    serde_json::Value::Array(blocks)
}

/// Gemini の`parts` (添付は`inline_data`)。
pub fn gemini_parts(prompt: &str, attachments: &[Attachment]) -> serde_json::Value {
    let mut parts: Vec<serde_json::Value> = attachments
        .iter()
        .map(|a| serde_json::json!({ "inline_data": { "mime_type": a.mime, "data": a.base64() } }))
        .collect();
    parts.push(serde_json::json!({ "text": prompt }));
    serde_json::Value::Array(parts)
}

/// OpenAI互換の`content`。PDFは読めないので断る (画面でも送る前に止める)。
pub fn openai_content(prompt: &str, attachments: &[Attachment]) -> Result<serde_json::Value, AgentError> {
    if let Some(pdf) = attachments.iter().find(|a| a.kind() == Some(AttachmentKind::Pdf)) {
        return Err(AgentError::Attachment(format!(
            "{}: OpenAI互換のモデルはPDFを読めません。画像だけ送るか、モデルを替えてください",
            pdf.name
        )));
    }
    let mut parts: Vec<serde_json::Value> = attachments
        .iter()
        .map(|a| {
            serde_json::json!({
                "type": "image_url",
                "image_url": { "url": format!("data:{};base64,{}", a.mime, a.base64()) },
            })
        })
        .collect();
    parts.push(serde_json::json!({ "type": "text", "text": prompt }));
    Ok(serde_json::Value::Array(parts))
}

/// Claude Code CLI の`--input-format stream-json`へ流す1行 (改行つき)。
pub fn claude_cli_input_line(prompt: &str, attachments: &[Attachment]) -> String {
    let message = serde_json::json!({
        "type": "user",
        "message": { "role": "user", "content": anthropic_content(prompt, attachments) },
    });
    format!("{message}\n")
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

    /// Images (PNG/JPEG/GIF/WebP) and PDFs can be attached; anything else, more than five files or a file over 20MB is refused as a whole.
    /// 画像 (PNG/JPEG/GIF/WebP) とPDFは添付でき、それ以外の種類・6件以上・20MBを超えるファイルは全体を断る。
    #[test]
    fn only_images_and_pdfs_within_the_limits_are_accepted() {
        assert!(validate(&[png("a.png"), pdf("b.pdf")]).is_ok());
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
        assert_eq!(mime_from_name("notes.txt"), None);
    }

    /// For the Anthropic API an image becomes an image block and a PDF a document block, both before the text.
    /// Anthropic APIへは画像を画像ブロック、PDFを文書ブロックにして、本文より前に置く。
    #[test]
    fn anthropic_gets_image_and_document_blocks_before_the_text() {
        let content = anthropic_content("照合して", &[png("a.png"), pdf("b.pdf")]);
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
        let parts = gemini_parts("要約して", &[pdf("b.pdf")]);
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
        let line = claude_cli_input_line("照合して", &[png("a.png")]);
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
        assert!(inspect_path(&dir.join("notes.txt")).error.unwrap().contains("PDFだけ"));
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
