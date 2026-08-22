//! 図枠テンプレート = プロットフレーム (M4仕様 §9)。
//!
//! 図枠(外枠・ゾーン目盛・表題欄・改訂欄・ロゴ枠)の**寸法と中身をJSONで定義**し、
//! SVG出力([`crate::svg`])とキャンバス描画(`src/canvas/frameTemplate.ts`)が
//! 同じ定義を読んで描く。図枠を差し替えれば全シートの見た目が一括で変わる。
//!
//! 表題欄のセルには**バインド**(プレースホルダ)を書ける。描画時に実際の値が入る:
//!
//! | バインド | 入る値 |
//! |---|---|
//! | `{project.name}` | プロジェクト名 |
//! | `{project.no}` | プロジェクト番号(モデルにまだ無いので現状は空) |
//! | `{sheet.no}` / `{sheet.total}` | シート番号 / 総シート数 |
//! | `{sheet.name}` | シート名 |
//! | `{rev.latest}` | 最新改訂の記号 ([`crate::svg::effective_rev`]) |
//! | `{title_block.*}` | 表題欄の各欄 (company/title/drawing_no/scale/date/designed/drawn/checked/approved/rev) |
//! | `{date}` `{scale}` `{designer}` `{approver}` | 表題欄の日付・尺度・設計・承認の別名 |
//!
//! **知らないバインドはエラーにせず空文字**にする(図面が出せなくなるより、欄が
//! 空のまま出るほうが実務で困らない)。
//!
//! ファイルの探索順は開始テンプレート([`crate::templates`])と同じ3段解決:
//!
//! 1. 環境変数 [`FRAME_TEMPLATES_PATH_ENV`](配布時にTauriがリソースの実体を入れる)
//! 2. リポジトリの `src-tauri/resources/frame_templates`(開発時)
//! 3. どちらも無ければビルド時に埋め込んだ同梱テンプレート(常に2種が使える)
//!
//! これに加えてユーザーの図枠(`~/MadakeCAD/templates/frames/*.json`)も列挙する。

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::model::{Project, Sheet, TitleBlock};

/// 同梱図枠テンプレートの実ディレクトリを与える環境変数(Tauriのリソース解決・テスト用)。
pub const FRAME_TEMPLATES_PATH_ENV: &str = "MADAKE_FRAME_TEMPLATES_PATH";
/// ユーザー図枠の置き場を差し替える環境変数(既定は`~/MadakeCAD/templates/frames`)。
pub const USER_FRAME_TEMPLATES_PATH_ENV: &str = "MADAKE_USER_FRAME_TEMPLATES_PATH";
/// 未指定・未知idのときに使う図枠テンプレートのid。
pub const DEFAULT_FRAME_TEMPLATE_ID: &str = "jis-a3";

/// 開発時(リポジトリで動かすとき)の同梱図枠テンプレートの位置。
const REPO_FRAME_TEMPLATES: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../resources/frame_templates");

/// 実ファイルが読めないときのフォールバック(ビルド時に埋め込む同内容)。並びは表示順。
const BUNDLED: &[&str] = &[
    include_str!("../../../resources/frame_templates/jis-a3.json"),
    include_str!("../../../resources/frame_templates/jis-a4-portrait.json"),
];

/// 図枠テンプレート1件。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FrameTemplate {
    /// 安定ID (例: "jis-a3")。[`Project::frame_template`]に保存されるのはこれ。
    pub id: String,
    /// 英語名 (UI言語がenのとき表示)。
    pub name: String,
    /// 日本語名。
    pub name_ja: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub description_ja: String,
    /// このテンプレートが想定する用紙 (mm)。**描画にはシートの用紙サイズ・向きを使う**
    /// ので、図枠を替えても用紙は変わらない。一覧表示とプレビューの縮尺に使う。
    pub paper: PaperMm,
    /// 用紙端から図枠外枠までのマージン (mm)。
    pub frame_margin: f64,
    pub zones: Zones,
    pub title_block: TitleBlockDef,
    pub revision_block: RevisionBlockDef,
    /// ロゴ枠 (用紙座標mm)。画像が解決できないときは枠だけを描く。
    #[serde(default)]
    pub logo: Option<LogoFrame>,
    /// 同梱ならtrue、ユーザーの図枠ならfalse。JSONには書かず、読み込んだ場所で決まる。
    #[serde(default, skip_deserializing)]
    pub builtin: bool,
}

/// 用紙寸法 (mm)。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PaperMm {
    pub w: f64,
    pub h: f64,
}

/// ゾーン(図面の住所)の設定。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Zones {
    /// 列数・行数。**これは新しいシートの既定値**で、既存シートは自分の
    /// [`Sheet::zone_cols`]/[`Sheet::zone_rows`]で描かれる(図枠を替えても
    /// 図面の住所が勝手に変わらない)。
    pub cols: u32,
    pub rows: u32,
    /// 列の目盛文字 (既定は数字)。
    pub col_labels: LabelStyle,
    /// 行の目盛文字 (既定は英字)。
    pub row_labels: LabelStyle,
}

/// ゾーン目盛の文字種。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LabelStyle {
    /// 1, 2, 3 …
    Numeric,
    /// A, B, C … (26を超えるとAへ戻る)
    Alpha,
}

impl LabelStyle {
    /// 0基点の位置に対応する目盛文字。
    pub fn label(&self, i: u32) -> String {
        match self {
            LabelStyle::Numeric => (i + 1).to_string(),
            LabelStyle::Alpha => char::from(b'A' + (i % 26) as u8).to_string(),
        }
    }
}

/// 表題欄の基準隅。既定(かつJIS)は右下。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    BottomRight,
    BottomLeft,
    TopRight,
    TopLeft,
}

/// 表題欄の定義。座標はすべて**表題欄の左上角からの相対mm**(負の値=枠の外)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TitleBlockDef {
    /// 図枠内のどの隅に置くか。
    #[serde(default = "default_origin")]
    pub origin: Origin,
    pub w: f64,
    pub h: f64,
    /// 1行の高さ (mm)。改訂欄も同じ行高で積む。
    pub row_h: f64,
    /// セル罫線。
    #[serde(default)]
    pub grid_lines: Vec<GridLine>,
    /// セル(固定文字またはバインド)。書いた順に描く。
    #[serde(default)]
    pub cells: Vec<Cell>,
}

fn default_origin() -> Origin {
    Origin::BottomRight
}

/// 表題欄の罫線 (表題欄の左上角からの相対mm)。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GridLine {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

/// 表題欄のセル1つ。`text`(固定文字)か`bind`(バインド入り)のどちらか一方を持つ。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Cell {
    /// 文字のアンカー位置 (x) とベースライン (y)。表題欄の左上角からの相対mm。
    pub x: f64,
    pub y: f64,
    /// セルの寸法 (mm)。描画には使わず、プレビューと将来の文字詰めのために持つ。
    #[serde(default)]
    pub w: Option<f64>,
    #[serde(default)]
    pub h: Option<f64>,
    /// 固定文字 (バインドは解決しない)。
    #[serde(default)]
    pub text: Option<String>,
    /// バインド入りの文字列 (例 `"{title_block.drawing_no}  Rev {rev.latest}"`)。
    #[serde(default)]
    pub bind: Option<String>,
    /// 文字高さ (mm)。
    pub size: f64,
    #[serde(default)]
    pub weight: Weight,
    #[serde(default)]
    pub align: Align,
    /// 値が空文字になったらセルごと描かない (会社名など、無いときは何も出さない欄)。
    #[serde(default)]
    pub omit_if_empty: bool,
}

impl Cell {
    /// このセルに実際に描く文字列。`text`は固定、`bind`はバインドを解決する。
    pub fn resolve(&self, ctx: &BindContext) -> String {
        match (&self.text, &self.bind) {
            (Some(t), _) => t.clone(),
            (None, Some(b)) => resolve_binds(b, ctx),
            (None, None) => String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Weight {
    #[default]
    Normal,
    Bold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    #[default]
    Start,
    Middle,
    End,
}

impl Align {
    /// SVGの`text-anchor`値。
    pub fn anchor(&self) -> &'static str {
        match self {
            Align::Start => "start",
            Align::Middle => "middle",
            Align::End => "end",
        }
    }
}

/// 改訂欄の定義。位置は表題欄の直上に固定(同じ右端・同じ幅・下から上へ育つ)。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RevisionBlockDef {
    /// 図面に描く最大行数。溢れた分は古い行から省く(データは保持)。
    pub rows_max: usize,
    /// 列幅 (記号 / 日付 / 内容 / 承認) mm。合計は表題欄の幅と一致させる。
    pub col_w: Vec<f64>,
}

/// ロゴ枠 (用紙座標mm)。`path`はテンプレートフォルダからの相対パス。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct LogoFrame {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// 画像ファイル (テンプレートフォルダ相対)。未指定・未解決なら枠だけを描く。
    #[serde(default)]
    pub path: Option<String>,
}

impl FrameTemplate {
    /// 表題欄の左上角の用紙座標 (mm)。用紙寸法とマージンから基準隅で決まる。
    pub fn title_block_origin(&self, pw: f64, ph: f64) -> (f64, f64) {
        let m = self.frame_margin;
        let (x0, y0) = (m, m);
        let (x1, y1) = (pw - m, ph - m);
        let tb = &self.title_block;
        match tb.origin {
            Origin::BottomRight => (x1 - tb.w, y1 - tb.h),
            Origin::BottomLeft => (x0, y1 - tb.h),
            Origin::TopRight => (x1 - tb.w, y0),
            Origin::TopLeft => (x0, y0),
        }
    }

    /// 改訂欄の各列の左端X (用紙座標mm)。表題欄と同じ左端から列幅を足していく。
    pub fn revision_col_lefts(&self, tx: f64) -> Vec<f64> {
        let mut lefts = Vec::with_capacity(self.revision_block.col_w.len());
        let mut x = tx;
        for w in &self.revision_block.col_w {
            lefts.push(x);
            x += w;
        }
        lefts
    }

    /// 定義の妥当性を確かめる。壊れたテンプレートは一覧に出さない
    /// ([`list`]がここで弾いて理由を報告する)。
    ///
    /// **エラーにするのは図面が破綻する矛盾だけ**にする。バインドの重複(同じ値を
    /// 2か所へ出す)・表題欄の外へ出るセル(会社名は枠外に置く)・知らないバインドは
    /// 実務で普通に使うので許容する。
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("id が空".into());
        }
        if self.paper.w <= 0.0 || self.paper.h <= 0.0 {
            return Err(format!("paper が不正: {:?}", self.paper));
        }
        if self.frame_margin < 0.0 {
            return Err(format!("frame_margin が負: {}", self.frame_margin));
        }
        if self.zones.cols == 0 || self.zones.rows == 0 {
            return Err("zones.cols / zones.rows は1以上".into());
        }
        let tb = &self.title_block;
        if tb.w <= 0.0 || tb.h <= 0.0 {
            return Err(format!("title_block の大きさが不正: {}x{}", tb.w, tb.h));
        }
        if tb.row_h <= 0.0 {
            return Err(format!("title_block.row_h が不正: {}", tb.row_h));
        }
        for (i, cell) in tb.cells.iter().enumerate() {
            match (&cell.text, &cell.bind) {
                (Some(_), Some(_)) => {
                    return Err(format!("cells[{i}]: text と bind は同時に書けない"))
                }
                (None, None) => return Err(format!("cells[{i}]: text か bind が要る")),
                _ => {}
            }
            if cell.size <= 0.0 {
                return Err(format!("cells[{i}]: size が不正: {}", cell.size));
            }
        }
        let rb = &self.revision_block;
        if rb.col_w.len() != REVISION_COLUMNS {
            return Err(format!(
                "revision_block.col_w は{REVISION_COLUMNS}列 (記号/日付/内容/承認): {}列",
                rb.col_w.len()
            ));
        }
        let sum: f64 = rb.col_w.iter().sum();
        if (sum - tb.w).abs() > 1e-6 {
            return Err(format!(
                "revision_block.col_w の合計 {sum} が表題欄の幅 {} と違う",
                tb.w
            ));
        }
        Ok(())
    }
}

/// 改訂欄の列数 (記号 / 日付 / 内容 / 承認)。
pub const REVISION_COLUMNS: usize = 4;

/// バインドを解決するための文脈 (どのプロジェクトの何枚目のシートか)。
#[derive(Debug, Clone)]
pub struct BindContext<'a> {
    pub project_name: &'a str,
    /// プロジェクト番号。モデルにまだ無いので現状は常に空。
    pub project_no: &'a str,
    /// 1始まりのシート番号。
    pub sheet_no: usize,
    pub sheet_total: usize,
    pub sheet_name: &'a str,
    pub title_block: &'a TitleBlock,
    /// 表題欄のRev欄に出す改訂記号 ([`crate::svg::effective_rev`])。
    pub rev_latest: String,
}

impl<'a> BindContext<'a> {
    /// シート単体の文脈 (プロジェクトが分からないので1枚目/全1枚として扱う)。
    pub fn for_sheet(sheet: &'a Sheet) -> Self {
        Self {
            project_name: "",
            project_no: "",
            sheet_no: 1,
            sheet_total: 1,
            sheet_name: &sheet.name,
            title_block: &sheet.title_block,
            rev_latest: crate::svg::effective_rev(sheet),
        }
    }

    /// プロジェクトの中の1シートとしての文脈 (シート番号・総数が入る)。
    pub fn for_project(project: &'a Project, sheet: &'a Sheet) -> Self {
        let index = project
            .sheets
            .iter()
            .position(|s| s.id == sheet.id)
            .unwrap_or(0);
        Self {
            project_name: &project.name,
            project_no: "",
            sheet_no: index + 1,
            sheet_total: project.sheets.len(),
            sheet_name: &sheet.name,
            title_block: &sheet.title_block,
            rev_latest: crate::svg::effective_rev(sheet),
        }
    }
}

/// 1つのバインド名の値。**知らない名前はNone**(呼び出し側で空文字になる)。
pub fn bind_value(name: &str, ctx: &BindContext) -> Option<String> {
    let tb = ctx.title_block;
    let v = match name {
        "project.name" => ctx.project_name.to_string(),
        "project.no" => ctx.project_no.to_string(),
        "sheet.no" => ctx.sheet_no.to_string(),
        "sheet.total" => ctx.sheet_total.to_string(),
        "sheet.name" => ctx.sheet_name.to_string(),
        "rev.latest" => ctx.rev_latest.clone(),
        "title_block.company" => tb.company.clone(),
        "title_block.title" => tb.title.clone(),
        "title_block.drawing_no" => tb.drawing_no.clone(),
        "title_block.scale" | "scale" => tb.scale.clone(),
        "title_block.date" | "date" => tb.date.clone(),
        "title_block.designed" | "designer" => tb.designed.clone(),
        "title_block.drawn" => tb.drawn.clone(),
        "title_block.checked" => tb.checked.clone(),
        "title_block.approved" | "approver" => tb.approved.clone(),
        "title_block.rev" => tb.rev.clone(),
        _ => return None,
    };
    Some(v)
}

/// 文字列中の`{バインド名}`を実際の値へ置き換える。知らないバインドは空文字。
/// `{`に対応する`}`が無いところはそのまま残す。
pub fn resolve_binds(text: &str, ctx: &BindContext) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                let name = &after[..close];
                out.push_str(&bind_value(name, ctx).unwrap_or_default());
                rest = &after[close + 1..];
            }
            None => {
                // 閉じ括弧が無い: そのまま残す
                out.push_str(&rest[open..]);
                return out;
            }
        }
    }
    out.push_str(rest);
    out
}

/// 読み込めなかった図枠テンプレート1件。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameTemplateIssue {
    pub path: String,
    pub message: String,
}

/// 図枠テンプレートの一覧と、読み込めなかったファイルの理由。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FrameTemplateList {
    pub templates: Vec<FrameTemplate>,
    pub issues: Vec<FrameTemplateIssue>,
    /// ユーザー図枠の置き場 (UIが「ここにJSONを置けば並ぶ」と案内する)。
    #[serde(default)]
    pub user_dir: Option<String>,
}

/// 同梱図枠のディレクトリ (環境変数 → リポジトリの順)。どちらも無ければNone。
pub fn bundled_dir() -> Option<PathBuf> {
    let from_env = std::env::var_os(FRAME_TEMPLATES_PATH_ENV).map(PathBuf::from);
    [from_env, Some(PathBuf::from(REPO_FRAME_TEMPLATES))]
        .into_iter()
        .flatten()
        .find(|p| p.is_dir())
}

/// ユーザー図枠の置き場 (`~/MadakeCAD/templates/frames`)。ホームが分からなければNone。
pub fn user_dir() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os(USER_FRAME_TEMPLATES_PATH_ENV) {
        return Some(PathBuf::from(p));
    }
    dirs::home_dir().map(|h| h.join("MadakeCAD").join("templates").join("frames"))
}

/// 使える図枠テンプレートを列挙する(同梱 → ユーザーの順)。
/// 同じidのユーザー図枠は同梱のものを差し替える。
pub fn list() -> FrameTemplateList {
    list_from(bundled_dir().as_deref(), user_dir().as_deref())
}

/// ディレクトリを明示して列挙する(テスト・組み込み用)。
/// `bundled`がNoneならビルドへ埋め込んだ同梱テンプレートを使う。
pub fn list_from(bundled: Option<&Path>, user: Option<&Path>) -> FrameTemplateList {
    let mut out = FrameTemplateList::default();
    match bundled {
        Some(dir) => read_dir_frames(dir, true, &mut out),
        None => {
            for text in BUNDLED {
                match parse(text) {
                    Ok(mut t) => {
                        t.builtin = true;
                        push(&mut out.templates, t);
                    }
                    Err(message) => out.issues.push(FrameTemplateIssue {
                        path: "<bundled>".into(),
                        message,
                    }),
                }
            }
        }
    }
    if let Some(dir) = user {
        out.user_dir = Some(dir.display().to_string());
        read_dir_frames(dir, false, &mut out);
    }
    out
}

/// 1つの図枠テンプレートをidで取得する。
pub fn find(id: &str) -> Option<FrameTemplate> {
    list().templates.into_iter().find(|t| t.id == id)
}

/// ビルドへ埋め込んだ既定の図枠 (`jis-a3`)。ファイルが1つも読めない環境でも必ず返る。
pub fn default_template() -> &'static FrameTemplate {
    static DEFAULT: OnceLock<FrameTemplate> = OnceLock::new();
    DEFAULT.get_or_init(|| {
        let mut t = parse(BUNDLED[0]).expect("同梱のjis-a3は常に読める");
        t.builtin = true;
        t
    })
}

/// idから図枠テンプレートを解決する。
///
/// 未指定 (`None`) は既定の`jis-a3`。**知らないidは既定へフォールバックし、警告を出す**
/// (他所で作った図面が開けなくなるより、既定の図枠で開くほうが実務で困らない)。
pub fn resolve(id: Option<&str>) -> FrameTemplate {
    let Some(id) = id.map(str::trim).filter(|s| !s.is_empty()) else {
        return default_template().clone();
    };
    if let Some(t) = find(id) {
        return t;
    }
    eprintln!("[madake] 未知の図枠テンプレート「{id}」: 既定の{DEFAULT_FRAME_TEMPLATE_ID}で描画します");
    default_template().clone()
}

/// プロジェクトに設定された図枠テンプレート ([`Project::frame_template`]) を解決する。
pub fn for_project(project: &Project) -> FrameTemplate {
    resolve(project.frame_template.as_deref())
}

/// ディレクトリ内の`*.json`をファイル名順に読む。無いディレクトリは静かに無視する。
fn read_dir_frames(dir: &Path, builtin: bool, out: &mut FrameTemplateList) {
    for path in crate::templates::json_files(dir) {
        let loaded = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| parse(&text));
        match loaded {
            Ok(mut template) => {
                template.builtin = builtin;
                push(&mut out.templates, template);
            }
            Err(message) => out.issues.push(FrameTemplateIssue {
                path: path.display().to_string(),
                message,
            }),
        }
    }
}

/// JSONを読み、[`FrameTemplate::validate`]まで通ったものだけを返す。
fn parse(text: &str) -> std::result::Result<FrameTemplate, String> {
    let template: FrameTemplate = serde_json::from_str(text).map_err(|e| e.to_string())?;
    template.validate()?;
    Ok(template)
}

/// 同じidが既にあれば差し替え、無ければ末尾へ足す。
fn push(templates: &mut Vec<FrameTemplate>, template: FrameTemplate) {
    match templates.iter_mut().find(|t| t.id == template.id) {
        Some(slot) => *slot = template,
        None => templates.push(template),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Orientation, PaperSize, Revision, Sheet};
    use uuid::Uuid;

    fn sheet() -> Sheet {
        let mut s = Sheet::new("動力回路", PaperSize::A3, Orientation::Landscape);
        s.title_block = TitleBlock {
            company: "まだけ電機".into(),
            title: "制御盤 主回路".into(),
            drawing_no: "MDK-001".into(),
            scale: "1:1".into(),
            date: "2026-08-23".into(),
            designed: "K.T".into(),
            drawn: "S.M".into(),
            checked: "Y.N".into(),
            approved: "H.I".into(),
            rev: "A".into(),
        };
        s
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("madake-frames-{tag}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The two bundled frame templates (JIS A3 landscape and JIS A4 portrait) are listed in that order with English and Japanese names, and both are valid.
    /// 同梱の図枠テンプレートは「JIS A3 横」「JIS A4 縦」の2種がこの順で並び、英語名と日本語名を持ち、どちらも定義として正しい。
    #[test]
    fn the_two_bundled_frame_templates_are_listed_in_both_languages() {
        let list = list();
        assert!(list.issues.is_empty(), "同梱図枠は全て読める: {:?}", list.issues);
        let ids: Vec<&str> = list.templates.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["jis-a3", "jis-a4-portrait"]);
        for t in &list.templates {
            assert!(t.builtin, "{} は同梱", t.id);
            assert!(!t.name.is_empty() && !t.name_ja.is_empty(), "{}", t.id);
            assert!(t.validate().is_ok(), "{}: {:?}", t.id, t.validate());
        }
        assert_eq!(list.templates[0].name_ja, "JIS A3 横");
        assert_eq!(list.templates[1].name_ja, "JIS A4 縦");
    }

    /// Even without the resource directory, the frame templates compiled into the build are still available.
    /// リソースのディレクトリが無い環境でも、ビルドへ埋め込んだ同梱図枠が使える。
    #[test]
    fn frame_templates_are_available_without_the_resource_directory() {
        let list = list_from(None, None);
        assert_eq!(list.templates.len(), 2);
        assert!(list.issues.is_empty(), "{:?}", list.issues);
        assert_eq!(list.templates[0].id, DEFAULT_FRAME_TEMPLATE_ID);
    }

    /// The jis-a3 template describes the current JIS frame: 10 mm margin, 4x6 zones with numeric columns and alphabetic rows, and a 120x32 mm title block anchored at the bottom-right corner.
    /// jis-a3テンプレートは現行のJIS図枠をそのまま表す: マージン10mm、ゾーンは4×6で列が数字・行が英字、表題欄は右下基準の120×32mm。
    #[test]
    fn the_jis_a3_template_describes_the_current_jis_frame() {
        let t = default_template();
        assert_eq!(t.frame_margin, 10.0);
        assert_eq!(t.paper, PaperMm { w: 420.0, h: 297.0 });
        assert_eq!((t.zones.cols, t.zones.rows), (4, 6));
        assert_eq!(t.zones.col_labels, LabelStyle::Numeric);
        assert_eq!(t.zones.row_labels, LabelStyle::Alpha);
        assert_eq!(t.title_block.origin, Origin::BottomRight);
        assert_eq!((t.title_block.w, t.title_block.h), (120.0, 32.0));
        assert_eq!(t.title_block.row_h, 8.0);
        assert_eq!(t.revision_block.rows_max, 6);
        assert_eq!(t.revision_block.col_w, vec![14.0, 28.0, 56.0, 22.0]);
        // A3横 (420x297) では表題欄の左上角は (290, 255)
        assert_eq!(t.title_block_origin(420.0, 297.0), (290.0, 255.0));
    }

    /// The jis-a4-portrait template is a 210x297 frame whose title block keeps the same cells as jis-a3 but is narrowed to 110 mm, with revision columns that still add up to the title-block width.
    /// jis-a4-portraitテンプレートは210×297の図枠で、表題欄のセル構成はjis-a3と同じまま幅だけ110mmへ詰めてあり、改訂欄の列幅の合計も表題欄の幅と一致する。
    #[test]
    fn the_a4_portrait_template_narrows_the_same_title_block() {
        let t = find("jis-a4-portrait").expect("同梱のA4縦");
        assert_eq!(t.paper, PaperMm { w: 210.0, h: 297.0 });
        assert_eq!(t.frame_margin, 10.0);
        assert_eq!((t.zones.cols, t.zones.rows), (4, 6));
        assert_eq!((t.title_block.w, t.title_block.h), (110.0, 32.0));
        let a3 = default_template();
        let labels = |t: &FrameTemplate| -> Vec<String> {
            t.title_block
                .cells
                .iter()
                .filter_map(|c| c.text.clone().or_else(|| c.bind.clone()))
                .collect()
        };
        assert_eq!(labels(&t), labels(a3), "セル構成 (ラベルとバインド) は同じ");
        let sum: f64 = t.revision_block.col_w.iter().sum();
        assert_eq!(sum, t.title_block.w);
        // A4縦 (210x297) では表題欄の左上角は (90, 255)
        assert_eq!(t.title_block_origin(210.0, 297.0), (90.0, 255.0));
    }

    /// Every documented bind resolves to its value: project name and sheet number/total, sheet name, latest revision, all title-block fields and the {date}/{scale}/{designer}/{approver} aliases.
    /// 説明書きのバインドはすべて値になる: プロジェクト名・シート番号/総数・シート名・最新改訂・表題欄の全欄・{date}/{scale}/{designer}/{approver}の別名。
    #[test]
    fn every_documented_bind_resolves_to_its_value() {
        let mut project = Project::new("まだけ制御盤");
        project.sheets[0] = sheet();
        project
            .sheets
            .push(Sheet::new("Sheet2", PaperSize::A3, Orientation::Landscape));
        let s = &project.sheets[0];
        let ctx = BindContext::for_project(&project, s);
        let got = |b: &str| resolve_binds(b, &ctx);
        assert_eq!(got("{project.name}"), "まだけ制御盤");
        assert_eq!(got("{sheet.no}/{sheet.total}"), "1/2");
        assert_eq!(got("{sheet.name}"), "動力回路");
        assert_eq!(got("{rev.latest}"), "A", "改訂が無いので表題欄のRev欄");
        assert_eq!(got("{title_block.company}"), "まだけ電機");
        assert_eq!(got("{title_block.title}"), "制御盤 主回路");
        assert_eq!(got("{title_block.drawing_no}"), "MDK-001");
        assert_eq!(got("{title_block.rev}"), "A");
        assert_eq!(got("{title_block.drawn}"), "S.M");
        assert_eq!(got("{title_block.checked}"), "Y.N");
        // 別名
        assert_eq!(got("{date}"), "2026-08-23");
        assert_eq!(got("{scale}"), "1:1");
        assert_eq!(got("{designer}"), "K.T");
        assert_eq!(got("{approver}"), "H.I");
        assert_eq!(got("{title_block.date}"), got("{date}"));
        assert_eq!(got("{title_block.approved}"), got("{approver}"));
    }

    /// {rev.latest} follows the newest revision row rather than the stored Rev field.
    /// {rev.latest}は保存されたRev欄ではなく、最新の改訂行の記号に追従する。
    #[test]
    fn the_latest_revision_bind_follows_the_revision_table() {
        let mut s = sheet();
        s.revisions = vec![
            Revision { mark: "A".into(), ..Default::default() },
            Revision { mark: "C".into(), ..Default::default() },
        ];
        let ctx = BindContext::for_sheet(&s);
        assert_eq!(resolve_binds("{rev.latest}", &ctx), "C");
    }

    /// An unknown bind becomes an empty string instead of an error, so a drawing with a typo in its template still prints.
    /// 知らないバインドはエラーではなく空文字になる(テンプレートに書き間違いがあっても図面は出せる)。
    #[test]
    fn an_unknown_bind_becomes_an_empty_string() {
        let s = sheet();
        let ctx = BindContext::for_sheet(&s);
        assert_eq!(bind_value("no.such.field", &ctx), None);
        assert_eq!(resolve_binds("[{no.such.field}]", &ctx), "[]");
        assert_eq!(resolve_binds("図番 {title_block.drawing_no} / {nope}", &ctx), "図番 MDK-001 / ");
    }

    /// Text with no binds is printed as is, and an unclosed brace is left alone instead of swallowing the rest of the line.
    /// バインドの無い文字はそのまま出て、閉じ括弧の無い「{」は行の残りを飲み込まずそのまま残る。
    #[test]
    fn plain_text_and_unclosed_braces_are_left_alone() {
        let s = sheet();
        let ctx = BindContext::for_sheet(&s);
        assert_eq!(resolve_binds("図番", &ctx), "図番");
        assert_eq!(resolve_binds("a {unclosed", &ctx), "a {unclosed");
    }

    /// On its own, a sheet is bound as sheet 1 of 1 with an empty project name; inside a project it gets its real position.
    /// シート単体では「1枚中1枚目・プロジェクト名は空」として解決され、プロジェクトの中では実際の位置が入る。
    #[test]
    fn a_lone_sheet_is_bound_as_sheet_one_of_one() {
        let s = sheet();
        let ctx = BindContext::for_sheet(&s);
        assert_eq!(resolve_binds("{sheet.no}/{sheet.total}", &ctx), "1/1");
        assert_eq!(resolve_binds("{project.name}", &ctx), "");
    }

    /// The project number bind is known but always empty for now, because the project model has no number field yet.
    /// プロジェクト番号のバインドは名前としては知られているが、プロジェクトにまだ番号欄が無いので現状は常に空になる。
    #[test]
    fn the_project_number_bind_is_known_but_empty_for_now() {
        let s = sheet();
        let ctx = BindContext::for_sheet(&s);
        assert_eq!(bind_value("project.no", &ctx), Some(String::new()));
    }

    /// A cell with fixed text prints it verbatim, braces included, while a cell with a bind resolves it.
    /// 固定文字のセルは中括弧も含めてそのまま出し、バインドのセルだけが値へ置き換わる。
    #[test]
    fn a_fixed_text_cell_is_printed_verbatim() {
        let s = sheet();
        let ctx = BindContext::for_sheet(&s);
        let fixed = Cell {
            x: 0.0, y: 0.0, w: None, h: None,
            text: Some("{title_block.title}".into()),
            bind: None,
            size: 3.0,
            weight: Weight::Normal,
            align: Align::Start,
            omit_if_empty: false,
        };
        assert_eq!(fixed.resolve(&ctx), "{title_block.title}");
        let bound = Cell { text: None, bind: Some("{title_block.title}".into()), ..fixed };
        assert_eq!(bound.resolve(&ctx), "制御盤 主回路");
    }

    /// A template whose cell has both text and bind, or neither, is rejected with a message naming the cell.
    /// セルに text と bind を両方書いた(またはどちらも書かなかった)テンプレートは、セル番号を添えて拒否される。
    #[test]
    fn a_cell_must_have_either_text_or_bind() {
        let mut t = default_template().clone();
        t.title_block.cells[0].bind = Some("{date}".into());
        assert!(t.validate().unwrap_err().contains("cells[0]"), "{:?}", t.validate());

        let mut t2 = default_template().clone();
        t2.title_block.cells[1].bind = None;
        assert!(t2.validate().unwrap_err().contains("cells[1]"));
    }

    /// The revision columns must be the four JIS columns and add up to the title-block width, otherwise the revision table would not line up with the title block.
    /// 改訂欄の列はJISの4列で、幅の合計が表題欄の幅と一致していなければならない(ずれた表は表題欄と揃わない)。
    #[test]
    fn the_revision_columns_must_add_up_to_the_title_block_width() {
        let mut t = default_template().clone();
        t.revision_block.col_w = vec![14.0, 28.0, 56.0, 20.0];
        assert!(t.validate().unwrap_err().contains("合計"), "{:?}", t.validate());

        let mut t2 = default_template().clone();
        t2.revision_block.col_w = vec![60.0, 60.0];
        assert!(t2.validate().unwrap_err().contains("4列"), "{:?}", t2.validate());
    }

    /// Zero zone columns or rows, a negative margin and a zero-height title block are all rejected.
    /// ゾーンが0列・0行、マージンが負、表題欄の高さが0といった定義は拒否される。
    #[test]
    fn degenerate_frame_definitions_are_rejected() {
        let base = default_template().clone();
        let mut a = base.clone();
        a.zones.cols = 0;
        assert!(a.validate().is_err());
        let mut b = base.clone();
        b.frame_margin = -1.0;
        assert!(b.validate().is_err());
        let mut c = base.clone();
        c.title_block.h = 0.0;
        assert!(c.validate().is_err());
        let mut d = base;
        d.title_block.cells[0].size = 0.0;
        assert!(d.validate().is_err());
    }

    /// Two cells may carry the same bind and a cell may sit outside the title-block rectangle: both are normal (the company name is printed to the left of the block).
    /// 同じバインドを2つのセルに書くことも、表題欄の外にセルを置くことも許される(会社名は表題欄の左外に出す)。
    #[test]
    fn duplicate_binds_and_cells_outside_the_block_are_allowed() {
        let mut t = default_template().clone();
        let mut extra = t.title_block.cells[1].clone();
        extra.y = 100.0;
        extra.x = -50.0;
        t.title_block.cells.push(extra);
        assert!(t.validate().is_ok(), "{:?}", t.validate());
        // 同梱のjis-a3自身、会社名セルが表題欄の左外 (x<0) にある
        assert!(default_template().title_block.cells.iter().any(|c| c.x < 0.0));
    }

    /// Frame templates the user drops into ~/MadakeCAD/templates/frames are listed after the bundled ones, and one with a bundled id replaces it.
    /// ユーザーが`~/MadakeCAD/templates/frames`へ置いた図枠は同梱の後に並び、同梱と同じidを使えば差し替えられる。
    #[test]
    fn user_frame_templates_are_listed_after_the_bundled_ones() {
        let dir = temp_dir("user");
        let mut mine = default_template().clone();
        mine.id = "our-a3".into();
        mine.name = "Our A3".into();
        mine.name_ja = "自社A3".into();
        std::fs::write(dir.join("our-a3.json"), serde_json::to_string(&mine).unwrap()).unwrap();

        let list = list_from(None, Some(&dir));
        assert!(list.issues.is_empty(), "{:?}", list.issues);
        assert_eq!(
            list.user_dir.as_deref(),
            Some(dir.display().to_string().as_str()),
            "UIが案内できるよう置き場も返す"
        );
        let last = list.templates.last().expect("末尾はユーザー図枠");
        assert_eq!(last.id, "our-a3");
        assert_eq!(last.name_ja, "自社A3");
        assert!(!last.builtin);

        // 同梱と同じidなら差し替え
        let mut ours = default_template().clone();
        ours.name_ja = "自社の標準図枠".into();
        std::fs::write(dir.join("jis-a3.json"), serde_json::to_string(&ours).unwrap()).unwrap();
        let list2 = list_from(None, Some(&dir));
        assert_eq!(list2.templates.len(), 3, "件数は増えない (2種+our-a3)");
        assert_eq!(list2.templates[0].id, "jis-a3");
        assert_eq!(list2.templates[0].name_ja, "自社の標準図枠");
        assert!(!list2.templates[0].builtin);

        std::fs::remove_dir_all(&dir).ok();
    }

    /// A frame template file that is broken JSON, or valid JSON with an invalid definition, is reported with its path and reason while the other templates stay usable.
    /// JSONとして壊れた図枠ファイルや、読めても定義が不正な図枠ファイルは、パスと理由を添えて報告され、他の図枠はそのまま使える。
    #[test]
    fn a_broken_frame_template_file_is_reported_with_its_path() {
        let dir = temp_dir("broken");
        std::fs::write(dir.join("broken.json"), "{ this is not json").unwrap();
        let mut bad = default_template().clone();
        bad.id = "bad".into();
        bad.title_block.w = 100.0; // 改訂欄の列幅合計 (120) と合わなくなる
        std::fs::write(dir.join("invalid.json"), serde_json::to_string(&bad).unwrap()).unwrap();

        let list = list_from(None, Some(&dir));
        assert_eq!(list.templates.len(), 2, "同梱図枠は残る");
        assert_eq!(list.issues.len(), 2, "壊れた2ファイルを報告する");
        let paths: Vec<&str> = list.issues.iter().map(|i| i.path.as_str()).collect();
        assert!(paths.iter().any(|p| p.ends_with("broken.json")), "{paths:?}");
        assert!(paths.iter().any(|p| p.ends_with("invalid.json")), "{paths:?}");
        assert!(list.issues.iter().all(|i| !i.message.is_empty()), "理由を添える");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// Resolving no id at all, or an id nobody knows, both fall back to the bundled jis-a3 frame so the drawing still prints.
    /// 図枠を指定していないときも、誰も知らないidのときも、同梱のjis-a3へフォールバックするので図面は必ず出せる。
    #[test]
    fn an_unknown_frame_template_id_falls_back_to_jis_a3() {
        assert_eq!(resolve(None).id, DEFAULT_FRAME_TEMPLATE_ID);
        assert_eq!(resolve(Some("")).id, DEFAULT_FRAME_TEMPLATE_ID);
        assert_eq!(resolve(Some("no-such-frame")).id, DEFAULT_FRAME_TEMPLATE_ID);
        assert_eq!(resolve(Some("jis-a4-portrait")).id, "jis-a4-portrait");

        let mut project = Project::new("t");
        assert_eq!(for_project(&project).id, DEFAULT_FRAME_TEMPLATE_ID);
        project.frame_template = Some("jis-a4-portrait".into());
        assert_eq!(for_project(&project).id, "jis-a4-portrait");
    }

    /// A frame template is a paper-independent definition: it is drawn on whatever paper the sheet uses, so switching frames never resizes the sheet.
    /// 図枠テンプレートは用紙に依存しない定義で、シートの用紙にそのまま描かれる(図枠を替えても用紙サイズは変わらない)。
    #[test]
    fn a_frame_template_does_not_change_the_paper_size() {
        let t = find("jis-a4-portrait").expect("A4縦");
        // A4縦の図枠をA3横 (420x297) のシートへ当てても、基準は用紙の実寸のまま
        assert_eq!(t.title_block_origin(420.0, 297.0), (300.0, 255.0));
        assert_eq!(t.paper, PaperMm { w: 210.0, h: 297.0 }, "paperは想定用紙の記録");
    }

    /// Zone labels come from the template: numeric columns count 1,2,3 and alphabetic rows run A,B,C wrapping back to A after Z.
    /// ゾーンの目盛文字はテンプレートが決める: 数字は1,2,3…、英字はA,B,C…でZの次はAへ戻る。
    #[test]
    fn zone_labels_follow_the_configured_style() {
        assert_eq!(LabelStyle::Numeric.label(0), "1");
        assert_eq!(LabelStyle::Numeric.label(9), "10");
        assert_eq!(LabelStyle::Alpha.label(0), "A");
        assert_eq!(LabelStyle::Alpha.label(25), "Z");
        assert_eq!(LabelStyle::Alpha.label(26), "A");
    }
}
