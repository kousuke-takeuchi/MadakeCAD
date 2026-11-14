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
//! | `{project.no}` | プロジェクト番号 ([`crate::model::Project::no`]、G16。`set_project_meta`コマンドで設定) |
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

/// 大文字の高さ ÷ フォントサイズ (`text_size_is: cap` の換算)。
pub const CAP_HEIGHT_RATIO: f64 = 0.72;
/// 複数行の文字の行送り ÷ フォントサイズ。
pub const LINE_PITCH: f64 = 1.25;
/// ロゴ画像の大きさの上限 (バイト)。
pub const LOGO_MAX_BYTES: u64 = 2_097_152;

fn yes() -> bool { true }
fn is_true(v: &bool) -> bool { *v }
fn is_false(v: &bool) -> bool { !*v }
fn is_zero(v: &u16) -> bool { *v == 0 }
fn is_default<T: Default + PartialEq>(v: &T) -> bool { *v == T::default() }

/// 辺ごとの図枠の余白 (mm)。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FrameMargins { pub left: f64, pub right: f64, pub top: f64, pub bottom: f64 }

/// 図枠の外に描くマーク。
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Marks {
    /// 用紙の四隅の裁断マーク (L字)。
    #[serde(default)]
    pub trim: bool,
    /// 外枠の各辺の中央の中心マーク。
    #[serde(default)]
    pub centre: bool,
    /// 裁断マークの角の位置: 用紙の各辺からの距離 (mm)。無ければ4辺とも4mm。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trim_insets: Option<FrameMargins>,
    /// 裁断マークの腕の長さ (mm)。無ければ15mm。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trim_arm: Option<f64>,
    /// 中心マークを外枠から裁断マークの線 (`trim_insets`) まで伸ばす。無ければ余白と10mmの小さいほう。
    #[serde(default, skip_serializing_if = "is_false")]
    pub centre_to_trim: bool,
    /// 中心マークの外側の端に矢印 (開いたV字) を付ける。
    #[serde(default, skip_serializing_if = "is_false")]
    pub centre_arrow: bool,
    /// 中心マークの線の太さ。無ければ0.5mm (medium)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub centre_weight: Option<LineWeight>,
}

/// 裁断マークの既定の位置 (用紙の端から mm) と腕の長さ (mm)。
pub const TRIM_INSET: f64 = 4.0;
pub const TRIM_ARM: f64 = 15.0;

impl Marks {
    /// 裁断マークの角の位置 (用紙の各辺からの距離)。
    pub fn trim_insets(&self) -> FrameMargins {
        self.trim_insets.unwrap_or(FrameMargins { left: TRIM_INSET, right: TRIM_INSET, top: TRIM_INSET, bottom: TRIM_INSET })
    }

    /// 裁断マークの腕の長さ。
    pub fn trim_arm(&self) -> f64 {
        self.trim_arm.unwrap_or(TRIM_ARM)
    }
}

/// 改訂記号を囲む三角の形。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MarkStyle {
    /// 三角の高さ ÷ 行高 (既定0.8)。
    #[serde(default = "mark_h")]
    pub h: f64,
    /// 底辺 ÷ 高さ (既定は正三角形の 2/√3)。
    #[serde(default = "mark_aspect")]
    pub aspect: f64,
    /// 斜辺の中ほどを空ける割合 (0〜0.9、既定0 = 切れ目なし)。
    #[serde(default)]
    pub gap: f64,
    /// 列の左端から三角の左下の角までの距離 (mm、既定2)。
    #[serde(default = "mark_pad")]
    pub pad: f64,
    /// 改訂の無い行にも三角を描く (`rows_fixed`の空行)。
    #[serde(default)]
    pub in_empty_rows: bool,
}

fn mark_h() -> f64 { 0.8 }
fn mark_aspect() -> f64 { 2.0 / 3f64.sqrt() }
fn mark_pad() -> f64 { 2.0 }

impl Default for MarkStyle {
    fn default() -> Self {
        Self { h: mark_h(), aspect: mark_aspect(), gap: 0.0, pad: mark_pad(), in_empty_rows: false }
    }
}

/// 罫線の太さ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LineWeight { #[default] Thin, Medium, Heavy }

impl LineWeight {
    /// 線幅 (mm)。
    pub fn stroke(&self) -> f64 {
        match self { LineWeight::Thin => 0.25, LineWeight::Medium => 0.5, LineWeight::Heavy => 1.0 }
    }
}

/// `Cell.size`の意味。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextSizeIs { #[default] Font, Cap }

/// 改訂記号を囲む形。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarkShape { #[default] None, Triangle }

/// 改訂欄の列が表示する項目。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RevisionField { Mark, Count, Description, RevNo, Date, By, Approved }

impl RevisionField {
    /// この列に出す値。
    pub fn value<'a>(&self, rev: &'a crate::model::Revision) -> &'a str {
        match self {
            RevisionField::Mark => &rev.mark,
            RevisionField::Count => &rev.count,
            RevisionField::Description => &rev.description,
            RevisionField::RevNo => &rev.rev_no,
            RevisionField::Date => &rev.date,
            RevisionField::By => &rev.by,
            RevisionField::Approved => &rev.approved,
        }
    }
}

/// 改訂欄の1列。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RevisionColumn {
    pub field: RevisionField,
    /// 見出し行に印字する文字。
    pub header: String,
    /// 列幅 (mm)。
    pub w: f64,
    #[serde(default)]
    pub align: Align,
    /// 同じセルの中で、主の項目のあとに続けて出す項目 (例: 記号の横の個数)。間に罫線は入らない。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suffix: Option<RevisionField>,
    /// 見出しの揃え。無ければ`align`と同じ (内容は左揃え・見出しは中央、のように分けたいときに書く)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_align: Option<Align>,
}

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
    /// 辺ごとの余白。あれば`frame_margin`の代わりに使う。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_margins: Option<FrameMargins>,
    pub zones: Zones,
    pub title_block: TitleBlockDef,
    pub revision_block: RevisionBlockDef,
    #[serde(default, skip_serializing_if = "is_default")]
    pub marks: Marks,
    #[serde(default, skip_serializing_if = "is_default")]
    pub text_size_is: TextSizeIs,
    /// ストロークフォントのとき、図枠の文字 (表題欄のセル・改訂欄) の英数字を半角・全角どちらで描くか。
    /// 無ければプロジェクトの設定に従う。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_latin: Option<crate::model::Latin>,
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
    /// ゾーン目盛を描くか (既定: 描く)。
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub show: bool,
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
    #[serde(default, skip_serializing_if = "is_default")]
    pub weight: LineWeight,
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
    /// 回転 (0/90/180/270、画面上で時計回り)。
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rotation: u16,
    /// 文字を枠で囲む。
    #[serde(default, skip_serializing_if = "is_false")]
    pub boxed: bool,
    /// `w`に収まるよう文字を縮める。
    #[serde(default, skip_serializing_if = "is_false")]
    pub fit: bool,
    /// 字間 (字の送りに掛ける倍率、既定1)。ストロークフォントのときだけ効く。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spacing: Option<f64>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<RevisionColumn>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_h: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rows_fixed: Option<usize>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub mark_shape: MarkShape,
    /// 改訂行の文字サイズ (mm)。無ければ2.5。`text_size_is`に従う。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_size: Option<f64>,
    /// 見出し行の文字サイズ (mm)。無ければ2.2。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_size: Option<f64>,
    /// 記号を囲む三角の形。無ければ既定 (行高の0.8倍の正三角形、切れ目なし)。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mark_style: Option<MarkStyle>,
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
    /// 読み込んだ画像 (data URI)。テンプレートのファイルからは読まず、書きもしない。
    #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub data_url: Option<String>,
}

impl FrameTemplate {
    /// 表題欄の左上角の用紙座標 (mm)。用紙寸法とマージンから基準隅で決まる。
    /// 4辺の余白。`frame_margins`が無ければ`frame_margin`を4辺に使う。
    pub fn margins(&self) -> FrameMargins {
        self.frame_margins.unwrap_or(FrameMargins {
            left: self.frame_margin, right: self.frame_margin,
            top: self.frame_margin, bottom: self.frame_margin,
        })
    }

    /// 外枠の矩形 (x0, y0, x1, y1) 用紙mm。
    pub fn frame_rect(&self, pw: f64, ph: f64) -> (f64, f64, f64, f64) {
        let m = self.margins();
        (m.left, m.top, pw - m.right, ph - m.bottom)
    }

    /// 描くフォントサイズ。`text_size_is: cap`なら大文字高さから換算する。
    pub fn font_size(&self, size: f64) -> f64 {
        match self.text_size_is {
            TextSizeIs::Font => size,
            TextSizeIs::Cap => size / CAP_HEIGHT_RATIO,
        }
    }

    /// 改訂欄の列。`columns`が無ければ従来の4列 (記号/日付/内容/承認)。
    pub fn revision_columns(&self) -> Vec<RevisionColumn> {
        if let Some(cols) = &self.revision_block.columns {
            return cols.clone();
        }
        let fields = [RevisionField::Mark, RevisionField::Date, RevisionField::Description, RevisionField::By];
        let headers = ["記号", "日付", "内容", "承認"];
        self.revision_block.col_w.iter().zip(fields).zip(headers)
            .map(|((w, field), header)| RevisionColumn { field, header: header.into(), w: *w, align: Align::Start, suffix: None, header_align: None })
            .collect()
    }

    /// 改訂欄の行高。
    pub fn revision_row_h(&self) -> f64 {
        self.revision_block.row_h.unwrap_or(self.title_block.row_h)
    }

    /// 改訂欄が新しい項目を使っているか (使っていなければ従来の描画を1バイトも変えない)。
    pub fn uses_extended_revision_table(&self) -> bool {
        let rb = &self.revision_block;
        rb.columns.is_some() || rb.row_h.is_some() || rb.rows_fixed.is_some()
            || rb.mark_shape != MarkShape::None || self.text_size_is != TextSizeIs::Font
            || rb.text_size.is_some() || rb.header_size.is_some() || rb.mark_style.is_some()
    }

    pub fn title_block_origin(&self, pw: f64, ph: f64) -> (f64, f64) {
        let (x0, y0, x1, y1) = self.frame_rect(pw, ph);
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
        // 数値は「有限で、範囲の中」だけを通す (NaN・無限大は比較をすり抜けるので、否定形で確かめる)
        let pos = |v: f64| v.is_finite() && v > 0.0;
        let non_neg = |v: f64| v.is_finite() && v >= 0.0;
        if !pos(self.paper.w) || !pos(self.paper.h) {
            return Err(format!("paper が不正: {:?}", self.paper));
        }
        if !non_neg(self.frame_margin) {
            return Err(format!("frame_margin が不正: {}", self.frame_margin));
        }
        if self.zones.cols == 0 || self.zones.rows == 0 {
            return Err("zones.cols / zones.rows は1以上".into());
        }
        if let Some(m) = &self.frame_margins {
            if [m.left, m.right, m.top, m.bottom].iter().any(|v| !non_neg(*v)) {
                return Err(format!("frame_margins が不正: {m:?}"));
            }
        }
        if let Some(i) = &self.marks.trim_insets {
            if [i.left, i.right, i.top, i.bottom].iter().any(|v| !non_neg(*v)) {
                return Err(format!("marks.trim_insets が不正: {i:?}"));
            }
        }
        if let Some(arm) = self.marks.trim_arm {
            if !pos(arm) {
                return Err(format!("marks.trim_arm が不正: {arm}"));
            }
        }
        let tb = &self.title_block;
        if !pos(tb.w) || !pos(tb.h) {
            return Err(format!("title_block の大きさが不正: {}x{}", tb.w, tb.h));
        }
        if !pos(tb.row_h) {
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
            if !pos(cell.size) {
                return Err(format!("cells[{i}]: size が不正: {}", cell.size));
            }
            if let Some(sp) = cell.spacing {
                if !pos(sp) {
                    return Err(format!("cells[{i}].spacing が不正: {sp}"));
                }
            }
            if ![0, 90, 180, 270].contains(&cell.rotation) {
                return Err(format!("cells[{i}]: rotation は 0/90/180/270: {}", cell.rotation));
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
        if let Some(cols) = &rb.columns {
            if cols.is_empty() || cols.len() > 8 {
                return Err(format!("revision_block.columns は1〜8列: {}列", cols.len()));
            }
            for (i, c) in cols.iter().enumerate() {
                if cols[..i].iter().any(|p| p.field == c.field) {
                    return Err(format!("revision_block.columns: {:?} が2回ある", c.field));
                }
                if !pos(c.w) {
                    return Err(format!("revision_block.columns[{i}].w が不正: {}", c.w));
                }
                if c.suffix == Some(c.field) {
                    return Err(format!("revision_block.columns[{i}].suffix が列の項目と同じ: {:?}", c.field));
                }
            }
            let sum: f64 = cols.iter().map(|c| c.w).sum();
            if (sum - tb.w).abs() > 0.01 {
                return Err(format!("revision_block.columns の幅の合計 {sum} が表題欄の幅 {} と違う", tb.w));
            }
        }
        if let Some(h) = rb.row_h {
            if !pos(h) { return Err(format!("revision_block.row_h が不正: {h}")); }
        }
        if let Some(v) = rb.text_size {
            if !pos(v) { return Err(format!("revision_block.text_size が不正: {v}")); }
        }
        if let Some(v) = rb.header_size {
            if !pos(v) { return Err(format!("revision_block.header_size が不正: {v}")); }
        }
        if let Some(st) = &rb.mark_style {
            if !(pos(st.h) && st.h <= 1.0) {
                return Err(format!("revision_block.mark_style.h は0より大きく1以下: {}", st.h));
            }
            if !pos(st.aspect) {
                return Err(format!("revision_block.mark_style.aspect が不正: {}", st.aspect));
            }
            if !(non_neg(st.gap) && st.gap <= 0.9) {
                return Err(format!("revision_block.mark_style.gap は0〜0.9: {}", st.gap));
            }
            if !non_neg(st.pad) {
                return Err(format!("revision_block.mark_style.pad が不正: {}", st.pad));
            }
        }
        if rb.rows_fixed == Some(0) {
            return Err("revision_block.rows_fixed は1以上".into());
        }
        if let Some(path) = self.logo.as_ref().and_then(|l| l.path.as_deref()) {
            let p = std::path::Path::new(path);
            if p.is_absolute() || p.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                return Err(format!("logo.path はテンプレートのフォルダの中だけ: {path}"));
            }
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
    /// プロジェクト番号 ([`crate::model::Project::no`]、G16)。未設定なら空。
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
            project_no: project.no.as_deref().unwrap_or(""),
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
        _ if name.starts_with("field.") => tb.fields.get(&name["field.".len()..]).cloned().unwrap_or_default(),
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

/// ロゴ画像のMIME型 (拡張子で決める。大文字小文字は区別しない)。知らない拡張子はNone。
pub fn logo_mime(path: &str) -> Option<&'static str> {
    let ext = std::path::Path::new(path).extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

/// ロゴ画像を読んで`logo.data_url`へ入れる。読めない・大きすぎる・知らない種類なら何もしない
/// (図面は枠だけで出る)。
pub fn load_logo(template: &mut FrameTemplate, dir: &Path) {
    use base64::Engine as _;
    let Some(logo) = template.logo.as_mut() else { return };
    logo.data_url = None;
    let Some(path) = logo.path.as_deref() else { return };
    let Some(mime) = logo_mime(path) else { return };
    let file = dir.join(path);
    match std::fs::metadata(&file) {
        Ok(meta) if meta.len() <= LOGO_MAX_BYTES => {}
        _ => return,
    }
    if let Ok(bytes) = std::fs::read(&file) {
        let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
        logo.data_url = Some(format!("data:{mime};base64,{b64}"));
    }
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
                load_logo(&mut template, dir);
                push(&mut out.templates, template);
            }
            Err(message) => out.issues.push(FrameTemplateIssue {
                path: path.display().to_string(),
                message,
            }),
        }
    }
}

/// 図枠エディタ (M4残り⑥) の保存: ユーザー図枠の置き場 (`~/MadakeCAD/templates/frames`)
/// へ`{id}.json`として書く。同梱の図枠とidがぶつかるものは拒否する (別名で保存させる)。
pub fn save(template: &FrameTemplate) -> std::result::Result<(FrameTemplate, PathBuf), String> {
    let dir = user_dir().ok_or("ユーザー図枠の置き場 (ホームディレクトリ) が見つかりません")?;
    let builtin: Vec<String> = list_from(bundled_dir().as_deref(), None)
        .templates
        .into_iter()
        .map(|t| t.id)
        .collect();
    save_to(&dir, template, &builtin)
}

/// ディレクトリを明示して保存する (テスト・組み込み用)。`builtin_ids`は上書きを拒む同梱のid。
///
/// **検証に落ちたら何も書かない** ([`FrameTemplate::validate`]と同じ判断)。idはファイル名に
/// なるので英数字・`-`・`_`だけ。保存はCommandエンジンの外 (ファイルのドメイン) なので
/// 図面のundo履歴には乗らない。同じidを使う図面は次に描くときから新しい図枠で描かれる。
pub fn save_to(
    dir: &Path,
    template: &FrameTemplate,
    builtin_ids: &[String],
) -> std::result::Result<(FrameTemplate, PathBuf), String> {
    let mut template = template.clone();
    template.id = template.id.trim().to_string();
    template.builtin = false;
    if template.id.is_empty()
        || !template.id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!("id「{}」は英数字・-・_ だけで書いてください", template.id));
    }
    if builtin_ids.iter().any(|b| b == &template.id) {
        return Err(format!("id「{}」は同梱の図枠と同じです。別名で保存してください", template.id));
    }
    template.validate()?;
    let mut value = serde_json::to_value(&template).map_err(|e| e.to_string())?;
    if let Some(obj) = value.as_object_mut() {
        // 同梱かどうかは読み込んだ場所で決まるのでファイルには書かない
        obj.remove("builtin");
        if let Some(logo) = obj.get_mut("logo").and_then(|l| l.as_object_mut()) {
            logo.remove("data_url");
        }
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{}.json", template.id));
    let text = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
    std::fs::write(&path, text + "\n").map_err(|e| e.to_string())?;
    Ok((template, path))
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

    const PNG_1X1_B64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFBQIAX8jx0gAAAABJRU5ErkJggg==";

    pub(crate) fn png_1x1() -> Vec<u8> {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.decode(PNG_1X1_B64).unwrap()
    }

    pub(crate) fn user_dir_with_logo(file: &str, bytes: &[u8]) -> TempDir {
        let dir = std::env::temp_dir().join(format!("madake-logo-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(file), bytes).unwrap();
        let mut t = base();
        t.id = "with-logo".into();
        t.logo = Some(LogoFrame { x: 300.0, y: 260.0, w: 40.0, h: 10.0, path: Some(file.into()), data_url: None });
        let mut v = serde_json::to_value(&t).unwrap();
        v.as_object_mut().unwrap().remove("builtin");
        std::fs::write(dir.join("with-logo.json"), serde_json::to_string(&v).unwrap()).unwrap();
        TempDir(dir)
    }

    /// テスト用の一時フォルダ。使い終わったら消す。
    pub(crate) struct TempDir(std::path::PathBuf);

    impl std::ops::Deref for TempDir {
        type Target = std::path::Path;
        fn deref(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn loaded_url(dir: &std::path::Path) -> Option<String> {
        let list = list_from(None, Some(dir));
        let t = list.templates.iter().find(|t| t.id == "with-logo").expect("listed");
        t.logo.as_ref().unwrap().data_url.clone()
    }

    /// A template whose logo names a PNG next to it is listed with the image loaded as a data URI.
    /// ロゴが隣のPNGを指すテンプレートは、画像がdata URIとして読み込まれた状態で一覧に出る。
    #[test]
    fn a_logo_png_next_to_the_template_is_loaded() {
        let dir = user_dir_with_logo("logo.png", &png_1x1());
        let url = loaded_url(&dir).expect("loaded");
        assert!(url.starts_with("data:image/png;base64,iVBOR"), "{url}");
    }

    /// A logo that is missing, too large or of an unknown type leaves the template listed without an image.
    /// ロゴが無い・大きすぎる・知らない種類のときも、テンプレートは画像無しで一覧に出る。
    #[test]
    fn an_unusable_logo_leaves_the_template_listed_without_an_image() {
        for (file, bytes) in [("logo.bmp", png_1x1()), ("logo.png", vec![0u8; (LOGO_MAX_BYTES + 1) as usize])] {
            let dir = user_dir_with_logo(file, &bytes);
            assert!(loaded_url(&dir).is_none(), "{file}");
        }
        let dir = user_dir_with_logo("logo.png", &png_1x1());
        std::fs::remove_file(dir.join("logo.png")).unwrap();
        assert!(loaded_url(&dir).is_none());
    }

    /// A JPEG logo is loaded with the image/jpeg type.
    /// JPEGのロゴはimage/jpegの種類で読み込まれる。
    #[test]
    fn a_jpeg_logo_is_loaded_with_the_jpeg_type() {
        let dir = user_dir_with_logo("logo.jpg", &[0xFF, 0xD8, 0xFF, 0xD9]);
        let url = loaded_url(&dir).expect("loaded");
        assert!(url.starts_with("data:image/jpeg;base64,"), "{url}");
    }

    /// A logo file name with an upper-case extension (LOGO.PNG) is accepted.
    /// 拡張子が大文字のロゴ画像 (LOGO.PNG) も受け付ける。
    #[test]
    fn an_upper_case_logo_extension_is_accepted() {
        let dir = user_dir_with_logo("LOGO.PNG", &png_1x1());
        let url = loaded_url(&dir).expect("loaded");
        assert!(url.starts_with("data:image/png;base64,"), "{url}");
    }

    /// A sheet drawn with a template whose logo was loaded from a real PNG file is exported to a PDF that contains the image.
    /// 実際のPNGファイルから読み込んだロゴを持つテンプレートで描いた図面は、その画像を含むPDFとして書き出される。
    #[test]
    fn a_loaded_logo_reaches_the_pdf() {
        let dir = user_dir_with_logo("logo.png", &png_1x1());
        let t = list_from(None, Some(&dir)).templates.into_iter().find(|t| t.id == "with-logo").unwrap();
        let svg = crate::svg::sheet_to_svg_with_frame(&sheet(), &[], &t, &[]);
        assert!(svg.contains("<image "), "{svg}");
        let pdf = crate::pdf::svgs_to_pdf(&[svg]).expect("pdf");
        assert!(String::from_utf8_lossy(&pdf).contains("/Subtype /Image"), "PDFに画像が無い");
    }

    /// A logo file that has an image extension but is not a readable image does not stop the export: the sheet still becomes a PDF.
    /// 画像の拡張子を持つが画像として読めないロゴファイルがあっても、書き出しは止まらない。図面はPDFになる。
    #[test]
    fn a_broken_logo_image_does_not_stop_the_pdf_export() {
        for file in ["logo.png", "logo.jpg", "logo.svg"] {
            let dir = user_dir_with_logo(file, b"not an image");
            let t = list_from(None, Some(&dir)).templates.into_iter().find(|t| t.id == "with-logo").unwrap();
            let svg = crate::svg::sheet_to_svg_with_frame(&sheet(), &[], &t, &[]);
            let pdf = crate::pdf::svgs_to_pdf(&[svg]);
            assert!(pdf.is_ok(), "{file}: {:?}", pdf.err());
        }
    }

    /// Saving a template never writes the loaded image or the built-in flag into the file.
    /// テンプレートの保存では、読み込んだ画像も同梱フラグもファイルへ書かない。
    #[test]
    fn saving_does_not_write_the_loaded_image() {
        let dir = user_dir_with_logo("logo.png", &png_1x1());
        let t = list_from(None, Some(&dir)).templates.into_iter().find(|t| t.id == "with-logo").unwrap();
        let (_, path) = save_to(&dir, &t, &[]).expect("saved");
        let text = std::fs::read_to_string(path).unwrap();
        assert!(!text.contains("data_url") && !text.contains("builtin"), "{text}");
        assert!(text.contains("\"path\": \"logo.png\""), "{text}");
    }


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
            ..Default::default()
        };
        s
    }

    /// A free title block field is printed where the template binds `{field.name}`; a name that is not set prints nothing.
    /// 表題欄の自由な欄は、テンプレートが`{field.名前}`でバインドした場所に印字される。設定の無い名前は何も出ない。
    #[test]
    fn free_fields_resolve_through_field_binds() {
        let mut s = sheet();
        s.title_block.fields.insert("title_en".into(), "MAIN CIRCUIT".into());
        let ctx = BindContext::for_sheet(&s);
        assert_eq!(resolve_binds("{field.title_en}/{field.missing}", &ctx), "MAIN CIRCUIT/");
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
        // ユーザーの図枠は見ない (開発者の ~/MadakeCAD/templates/frames に何があっても結果が変わらない)
        let list = list_from(bundled_dir().as_deref(), None);
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

    /// Outside a project (a lone sheet) the project number bind is known but empty, since there is no project to read it from.
    /// プロジェクトの外 (シート単体) ではプロジェクト番号のバインドは名前として知られているが、読み取り元のプロジェクトが無いので空になる。
    #[test]
    fn the_project_number_bind_is_empty_outside_a_project() {
        let s = sheet();
        let ctx = BindContext::for_sheet(&s);
        assert_eq!(bind_value("project.no", &ctx), Some(String::new()));
    }

    /// Inside a project, the project number bind reads `Project::no` (G16), the same field `set_project_meta` writes.
    /// プロジェクトの中ではプロジェクト番号のバインドが`Project::no` (G16) を読む。`set_project_meta`コマンドが書き込む欄と同じ。
    #[test]
    fn the_project_number_bind_reads_the_project_number() {
        let mut project = Project::new("t");
        project.no = Some("MDK-2026-001".into());
        let s = project.sheets[0].clone();
        let ctx = BindContext::for_project(&project, &s);
        assert_eq!(bind_value("project.no", &ctx), Some("MDK-2026-001".to_string()));
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
            rotation: 0, boxed: false, fit: false, spacing: None,
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

    /// The frame editor saves a user frame as {id}.json in the user folder, and the list then shows it as a user frame with the edits.
    /// 図枠エディタはユーザー図枠をユーザーの置き場へ{id}.jsonとして保存し、一覧にはその編集内容がユーザー図枠として並ぶ。
    #[test]
    fn the_frame_editor_saves_a_user_frame_that_lists_back() {
        let dir = temp_dir("save");
        let mut t = default_template().clone();
        t.id = "custom-2026".into();
        t.name_ja = "自社様式".into();
        t.title_block.cells[0].text = Some("図面番号".into());
        let (saved, path) = save_to(&dir, &t, &["jis-a3".into()]).expect("保存できる");
        assert!(!saved.builtin);
        assert_eq!(path, dir.join("custom-2026.json"));
        assert!(!std::fs::read_to_string(&path).unwrap().contains("builtin"), "builtinはファイルに書かない");
        let list = list_from(None, Some(&dir));
        let back = list.templates.iter().find(|x| x.id == "custom-2026").expect("一覧に出る");
        assert!(!back.builtin);
        assert_eq!(back.title_block.cells[0].text.as_deref(), Some("図面番号"));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Saving under the id of a bundled frame is refused and nothing is written, so the user must save under another name.
    /// 同梱の図枠と同じidでの保存は拒否して何も書かない(別名で保存させる)。
    #[test]
    fn saving_over_a_bundled_frame_id_is_refused() {
        let dir = temp_dir("builtin");
        let t = default_template().clone();
        let err = save_to(&dir, &t, &["jis-a3".into()]).unwrap_err();
        assert!(err.contains("別名"), "{err}");
        assert!(!dir.join("jis-a3.json").exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A broken frame (revision columns not adding up to the title block width) or an id that cannot be a file name is refused and nothing is written.
    /// 壊れた図枠(改訂欄の列幅の合計が表題欄の幅と違う)や、ファイル名にできないidは拒否して何も書かない。
    #[test]
    fn an_invalid_frame_or_id_is_not_saved() {
        let dir = temp_dir("invalid");
        let mut broken = default_template().clone();
        broken.id = "mine".into();
        broken.revision_block.col_w = vec![10.0, 10.0, 10.0, 10.0];
        assert!(save_to(&dir, &broken, &[]).unwrap_err().contains("col_w"));
        let mut bad_id = default_template().clone();
        bad_id.id = "../evil".into();
        assert!(save_to(&dir, &bad_id, &[]).is_err());
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "何も書かない");
        std::fs::remove_dir_all(&dir).ok();
    }

    fn base() -> FrameTemplate {
        let mut t = default_template().clone();
        // `builtin`は読み込み時に決まる値で書き戻さないので、往復比較のため外しておく
        t.builtin = false;
        t
    }

    /// Without the new items a template reports today's values: one margin on all four sides, thin rules, font sizes as written, the four classic revision columns.
    /// 新しい項目の無いテンプレートは今までどおりの値を返す(4辺同じ余白、細い罫線、書いたとおりの文字サイズ、従来の改訂欄4列)。
    #[test]
    fn a_template_without_the_new_items_keeps_todays_values() {
        let t = base();
        let m = t.margins();
        assert_eq!((m.left, m.right, m.top, m.bottom), (10.0, 10.0, 10.0, 10.0));
        assert_eq!(t.frame_rect(420.0, 297.0), (10.0, 10.0, 410.0, 287.0));
        assert_eq!(t.font_size(3.0), 3.0);
        assert!(t.zones.show);
        assert!(!t.uses_extended_revision_table());
        let cols = t.revision_columns();
        assert_eq!(cols.len(), 4);
        assert_eq!(cols[0].field, RevisionField::Mark);
        assert_eq!(cols[1].field, RevisionField::Date);
        assert_eq!(cols[2].field, RevisionField::Description);
        assert_eq!(cols[3].field, RevisionField::By);
        assert_eq!(cols.iter().map(|c| c.w).sum::<f64>(), t.title_block.w);
        assert_eq!(t.revision_row_h(), t.title_block.row_h);
    }

    /// Margins given per side move the frame and the title block corner; the single margin is then ignored.
    /// 辺ごとの余白を指定すると外枠と表題欄の隅がそれに従い、1値の余白は無視される。
    #[test]
    fn margins_per_side_move_the_frame_and_the_title_block() {
        let mut t = base();
        t.frame_margins = Some(FrameMargins { left: 30.0, right: 15.0, top: 12.0, bottom: 10.0 });
        assert_eq!(t.frame_rect(1189.0, 841.0), (30.0, 12.0, 1174.0, 831.0));
        let (tx, ty) = t.title_block_origin(1189.0, 841.0);
        assert_eq!((tx, ty), (1174.0 - t.title_block.w, 831.0 - t.title_block.h));
        t.title_block.origin = Origin::TopLeft;
        assert_eq!(t.title_block_origin(1189.0, 841.0), (30.0, 12.0));
    }

    /// With capital-height sizes, a size of 3.6 mm is drawn with a 5 mm font.
    /// 文字サイズが大文字高さの指定なら、3.6mmは5mmのフォントで描く。
    #[test]
    fn capital_height_sizes_are_converted_to_font_sizes() {
        let mut t = base();
        t.text_size_is = TextSizeIs::Cap;
        assert!((t.font_size(3.6) - 5.0).abs() < 1e-9);
    }

    /// The new items are optional keys: a template file written before them still loads, and saving a template that does not use them writes none of the new keys.
    /// 新しい項目は省略できるキーである。追加前のテンプレートファイルはそのまま読め、使っていないテンプレートを保存しても新しいキーは書かれない。
    #[test]
    fn old_template_files_load_and_unused_items_are_not_written() {
        let t = parse(BUNDLED[0]).expect("jis-a3");
        let json = serde_json::to_string(&t).unwrap();
        for key in ["\"frame_margins\"", "\"marks\"", "\"text_size_is\"", "\"show\"", "\"weight\":\"thin\"", "\"rotation\"", "\"boxed\"", "\"fit\"", "\"columns\"", "\"rows_fixed\"", "\"mark_shape\"", "\"data_url\""] {
            assert!(!json.contains(key), "{key} が書かれている: {json}");
        }
    }

    /// A template that uses every new item survives a write and a read unchanged.
    /// 新しい項目を全て使うテンプレートは、書いて読み直しても変わらない。
    #[test]
    fn a_template_with_every_new_item_round_trips() {
        let t = extended();
        let back: FrameTemplate = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
        assert_eq!(back, t);
        back.validate().expect("valid");
    }

    fn extended() -> FrameTemplate {
        let mut t = base();
        t.frame_margins = Some(FrameMargins { left: 30.0, right: 15.0, top: 12.0, bottom: 10.0 });
        t.zones.show = false;
        t.marks = Marks { trim: true, centre: true, ..Default::default() };
        t.text_size_is = TextSizeIs::Cap;
        t.title_block.grid_lines[0].weight = LineWeight::Medium;
        t.title_block.cells[0].rotation = 90;
        t.title_block.cells[0].boxed = true;
        t.title_block.cells[0].fit = true;
        let w = t.title_block.w;
        t.revision_block.columns = Some(vec![
            RevisionColumn { field: RevisionField::Mark, header: "SYMB".into(), w: 14.0, align: Align::Start, suffix: None, header_align: None },
            RevisionColumn { field: RevisionField::Description, header: "CHANGELOG".into(), w: w - 64.0, align: Align::Start, suffix: None, header_align: None },
            RevisionColumn { field: RevisionField::RevNo, header: "ECN NO".into(), w: 16.0, align: Align::Middle, suffix: None, header_align: None },
            RevisionColumn { field: RevisionField::Date, header: "DATE".into(), w: 12.0, align: Align::Middle, suffix: None, header_align: None },
            RevisionColumn { field: RevisionField::By, header: "BY".into(), w: 11.0, align: Align::Middle, suffix: None, header_align: None },
            RevisionColumn { field: RevisionField::Approved, header: "OKAY".into(), w: 11.0, align: Align::Middle, suffix: None, header_align: None },
        ]);
        t.revision_block.row_h = Some(7.0);
        t.revision_block.rows_fixed = Some(7);
        t.revision_block.mark_shape = MarkShape::Triangle;
        t
    }

    /// Broken values are refused with a message that names the item.
    /// 壊れた値は、項目名を示すメッセージで拒否される。
    #[test]
    fn invalid_new_items_are_refused_by_name() {
        let cases: Vec<(&str, Box<dyn Fn(&mut FrameTemplate)>)> = vec![
            ("frame_margins", Box::new(|t| t.frame_margins = Some(FrameMargins { left: -1.0, right: 0.0, top: 0.0, bottom: 0.0 }))),
            ("rotation", Box::new(|t| t.title_block.cells[0].rotation = 45)),
            ("revision_block.columns", Box::new(|t| t.revision_block.columns = Some(vec![]))),
            ("revision_block.columns", Box::new(|t| {
                let w = t.title_block.w;
                t.revision_block.columns = Some(vec![
                    RevisionColumn { field: RevisionField::Mark, header: "A".into(), w: w / 2.0, align: Align::Start, suffix: None, header_align: None },
                    RevisionColumn { field: RevisionField::Mark, header: "B".into(), w: w / 2.0, align: Align::Start, suffix: None, header_align: None },
                ])
            })),
            ("revision_block.columns", Box::new(|t| t.revision_block.columns = Some(vec![
                RevisionColumn { field: RevisionField::Mark, header: "A".into(), w: 1.0, align: Align::Start, suffix: None, header_align: None },
            ]))),
            ("revision_block.row_h", Box::new(|t| t.revision_block.row_h = Some(0.0))),
            ("revision_block.rows_fixed", Box::new(|t| t.revision_block.rows_fixed = Some(0))),
            ("logo.path", Box::new(|t| t.logo = Some(LogoFrame { x: 0.0, y: 0.0, w: 10.0, h: 5.0, path: Some("../x.png".into()), data_url: None }))),
            ("logo.path", Box::new(|t| t.logo = Some(LogoFrame { x: 0.0, y: 0.0, w: 10.0, h: 5.0, path: Some("/etc/x.png".into()), data_url: None }))),
        ];
        for (name, edit) in cases {
            let mut t = base();
            edit(&mut t);
            let err = t.validate().expect_err(name);
            assert!(err.contains(name), "{name}: {err}");
        }
    }

    /// Values that are not finite numbers, too many columns, a column without width and bad mark or revision styles are refused by name.
    /// 有限の数でない値、多すぎる列、幅の無い列、記号や改訂欄の不正な形は、項目名つきで拒否される。
    #[test]
    fn more_invalid_items_are_refused_by_name() {
        let col = |field, w| RevisionColumn { field, header: "H".into(), w, align: Align::Start, suffix: None, header_align: None };
        let cases: Vec<(&str, Box<dyn Fn(&mut FrameTemplate)>)> = vec![
            ("frame_margin", Box::new(|t| t.frame_margin = f64::NAN)),
            ("frame_margins", Box::new(|t| t.frame_margins = Some(FrameMargins { left: f64::NAN, right: 0.0, top: 0.0, bottom: 0.0 }))),
            ("revision_block.row_h", Box::new(|t| t.revision_block.row_h = Some(f64::INFINITY))),
            ("size", Box::new(|t| t.title_block.cells[0].size = f64::NAN)),
            ("revision_block.columns", Box::new(move |t| {
                let w = t.title_block.w / 9.0;
                use RevisionField::*;
                // 項目は7種類しかないので、9列にするには重複が要る。列数の検査が先に効くことを確かめる
                t.revision_block.columns = Some([Mark, Count, Description, RevNo, Date, By, Approved, Mark, Count].into_iter().map(|f| col(f, w)).collect());
            })),
            ("revision_block.columns[1].w", Box::new(move |t| {
                let w = t.title_block.w;
                t.revision_block.columns = Some(vec![col(RevisionField::Mark, w), col(RevisionField::Date, 0.0)]);
            })),
            ("revision_block.columns[0].suffix", Box::new(move |t| {
                let w = t.title_block.w;
                t.revision_block.columns = Some(vec![RevisionColumn { suffix: Some(RevisionField::Mark), ..col(RevisionField::Mark, w) }]);
            })),
            ("cells[0].spacing", Box::new(|t| t.title_block.cells[0].spacing = Some(0.0))),
            ("cells[0].spacing", Box::new(|t| t.title_block.cells[0].spacing = Some(f64::NAN))),
            ("revision_block.text_size", Box::new(|t| t.revision_block.text_size = Some(0.0))),
            ("revision_block.header_size", Box::new(|t| t.revision_block.header_size = Some(-1.0))),
            ("revision_block.mark_style.h", Box::new(|t| t.revision_block.mark_style = Some(MarkStyle { h: 0.0, ..Default::default() }))),
            ("revision_block.mark_style.h", Box::new(|t| t.revision_block.mark_style = Some(MarkStyle { h: 1.5, ..Default::default() }))),
            ("revision_block.mark_style.aspect", Box::new(|t| t.revision_block.mark_style = Some(MarkStyle { aspect: 0.0, ..Default::default() }))),
            ("revision_block.mark_style.gap", Box::new(|t| t.revision_block.mark_style = Some(MarkStyle { gap: 0.95, ..Default::default() }))),
            ("revision_block.mark_style.pad", Box::new(|t| t.revision_block.mark_style = Some(MarkStyle { pad: -1.0, ..Default::default() }))),
            ("marks.trim_insets", Box::new(|t| t.marks.trim_insets = Some(FrameMargins { left: -1.0, right: 0.0, top: 0.0, bottom: 0.0 }))),
            ("marks.trim_arm", Box::new(|t| t.marks.trim_arm = Some(0.0))),
        ];
        for (name, edit) in cases {
            let mut t = base();
            edit(&mut t);
            let err = t.validate().expect_err(name);
            assert!(err.contains(name), "{name}: {err}");
        }
    }

    /// The mark and revision styles are optional too: defaults are 4 mm trim insets, 15 mm arms and an equilateral triangle 0.8 of the row high, and a template using every style item survives a write and a read.
    /// 記号と改訂欄の形も省略できる。既定は裁断マークが4mm内側・腕15mm、三角は行高の0.8倍の正三角形で、全ての項目を使うテンプレートは書いて読み直しても変わらない。
    #[test]
    fn mark_and_revision_styles_default_and_round_trip() {
        let t = base();
        let i = t.marks.trim_insets();
        assert_eq!((i.left, i.right, i.top, i.bottom, t.marks.trim_arm()), (4.0, 4.0, 4.0, 4.0, 15.0));
        let st = MarkStyle::default();
        assert_eq!((st.h, st.gap, st.pad, st.in_empty_rows), (0.8, 0.0, 2.0, false));
        assert!((st.aspect - 2.0 / 3f64.sqrt()).abs() < 1e-12);
        let json = serde_json::to_string(&t).unwrap();
        for key in ["\"text_latin\"", "\"spacing\"", "\"trim_insets\"", "\"trim_arm\"", "\"centre_to_trim\"", "\"centre_arrow\"", "\"centre_weight\"", "\"suffix\"", "\"text_size\"", "\"header_size\"", "\"mark_style\""] {
            assert!(!json.contains(key), "{key} が書かれている");
        }
        let mut t = extended();
        t.marks = Marks {
            trim: true, centre: true,
            trim_insets: Some(FrameMargins { left: 4.0, right: 6.0, top: 5.0, bottom: 3.0 }),
            trim_arm: Some(12.0), centre_to_trim: true, centre_arrow: true, centre_weight: Some(LineWeight::Thin),
        };
        t.text_latin = Some(crate::model::Latin::Full);
        t.title_block.cells[1].spacing = Some(1.2);
        t.revision_block.text_size = Some(3.0);
        t.revision_block.header_size = Some(2.0);
        t.revision_block.mark_style = Some(MarkStyle { h: 1.0, aspect: 0.75, gap: 0.5, pad: 0.0, in_empty_rows: true });
        t.revision_block.columns.as_mut().unwrap()[0].suffix = Some(RevisionField::Count);
        let back: FrameTemplate = serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
        assert_eq!(back, t);
        back.validate().expect("valid");
        assert!(base().uses_extended_revision_table() == false);
        let mut only_size = base();
        only_size.revision_block.text_size = Some(3.0);
        assert!(only_size.uses_extended_revision_table());
    }
}
