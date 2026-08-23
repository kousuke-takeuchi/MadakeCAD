use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::geometry::Point;

pub type EntityId = Uuid;
pub type SheetId = Uuid;

/// `.mdkproj` のファイル形式バージョン。
///
/// - v1: 初版
/// - v2: PLC I/O割付表 ([`Project::plc_assignments`]) を追加。旧ファイルは空の割付表で開き、
///   読み込み時に現行版へ更新される ([`crate::io::load_project`])
/// - v3: 図枠テンプレート ([`Project::frame_template`]) を追加。旧ファイルは未指定で開き、
///   既定の`jis-a3`図枠で描かれる(見た目は従来と同じ)
/// - v4: シート種別 ([`Sheet::kind`]) と盤レイアウトのエンティティ4種
///   ([`Entity::PanelOutline`] / [`Entity::DinRail`] / [`Entity::Duct`] / [`Entity::Footprint`])
///   を追加。種別の無い旧ファイルのシートは回路図シート ([`SheetKind::Schematic`]) として開く
/// - v5: 注記テキストの装飾 ([`TextEntity::anchor`] / [`TextEntity::color`] / [`TextEntity::bold`])
///   を追加。装飾を持たない旧ファイルの注記は左揃え・既定色 (黒)・太字なしで開き、
///   出力される図面はこれまでと1文字も変わらない
/// - v6: 盤レイアウトのアイテム番号 ([`Footprint::item_no`]) と凡例表 ([`Entity::PanelLegend`])
///   を追加。アイテム番号を持たない旧ファイルのフットプリントはバルーンを描かず、
///   採番 (`renumber_panel_items`) すればいつでも振り直せる
/// - v7: シートごとの注記文字高 ([`Sheet::annotation_text_mm`]) を追加。設定の無い旧ファイルは
///   既定 ([`DEFAULT_ANNOTATION_TEXT_MM`] = 2.0mm) で開き、出力される図面は変わらない
/// - v8: 図形エンティティ ([`Entity::Shape`]) を追加。矩形・線・円・折れ線の純粋な作図要素で、
///   図形を1つも持たない旧ファイルは今までどおり開き、出力される図面も変わらない
pub const FORMAT_VERSION: u32 = 8;

/// 注記文字高の既定値 (mm)。この設定が無かった頃の配線注記の文字高さで、
/// 未設定のシートはこの高さで描かれる (旧ファイルの見た目が変わらない)。
pub const DEFAULT_ANNOTATION_TEXT_MM: f64 = 2.0;
/// 注記文字高の下限 (mm)。これより小さいとA3に印刷したとき人の目で読めない。
pub const MIN_ANNOTATION_TEXT_MM: f64 = 1.0;
/// 注記文字高の上限 (mm)。これより大きいと2.5mmピッチの図面で注記同士が必ず重なる。
pub const MAX_ANNOTATION_TEXT_MM: f64 = 3.5;

/// プロジェクト全体。保存形式(.mdkproj)のルート。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Project {
    pub format_version: u32,
    pub name: String,
    pub sheets: Vec<Sheet>,
    /// 電線品番マスタ: 線色+線径から品番を引く。
    #[serde(default)]
    pub wire_parts: Vec<WirePart>,
    /// PLC I/O割付表 (M4仕様 §3)。信号名・コメントはここが正で、接続先・線番は
    /// 図面の結線から導出する ([`crate::plc::plc_points`])。
    #[serde(default)]
    pub plc_assignments: Vec<PlcAssignment>,
    /// 図枠テンプレート(プロットフレーム)のid (M4仕様 §9)。全シートの図枠・表題欄・
    /// 改訂欄がこの定義で描かれる。未指定・未知idは既定の`jis-a3`
    /// ([`crate::frame_template::DEFAULT_FRAME_TEMPLATE_ID`])。
    #[serde(default)]
    pub frame_template: Option<String>,
}

impl Project {
    pub fn new(name: &str) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            name: name.to_string(),
            sheets: vec![Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape)],
            wire_parts: Vec::new(),
            plc_assignments: Vec::new(),
            frame_template: None,
        }
    }

    pub fn sheet(&self, id: SheetId) -> Option<&Sheet> {
        self.sheets.iter().find(|s| s.id == id)
    }

    pub fn sheet_mut(&mut self, id: SheetId) -> Option<&mut Sheet> {
        self.sheets.iter_mut().find(|s| s.id == id)
    }
}

/// PLC I/O割付表の1行 = I/O点1つ (M4仕様 §3)。
///
/// 「どのモジュールの・どのアドレスが・何の信号か」を人が決めて書くところ。
/// 接続先と線番は図面の結線から読み取って表示するだけなのでここには保存しない
/// (図面を直せば表も変わる)。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PlcAssignment {
    /// 行のid (並べ替え・編集の同一性判定用)。
    pub id: Uuid,
    /// PLCモジュールの参照記号 (例 "PLC1")。図面のモジュールシンボルと同じ記号。
    #[serde(default)]
    pub module_ref: String,
    /// I/Oアドレス (例 "X0" / "%I0.0" / "I:0/0")。
    #[serde(default)]
    pub address: String,
    /// 信号名 (例 "起動押釦")。
    #[serde(default)]
    pub signal_name: String,
    /// コメント。
    #[serde(default)]
    pub comment: String,
}

/// 電線品番マスタの1行(例: 品番 SAMPLE0001 = 水色 0.3sq)。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WirePart {
    pub part_no: String,
    pub color: String,
    pub sq: f64,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum PaperSize {
    A4,
    A3,
    A2,
    A1,
    A0,
}

impl PaperSize {
    /// 横置き時の (幅, 高さ) mm。
    pub fn dimensions_mm(&self) -> (f64, f64) {
        match self {
            PaperSize::A4 => (297.0, 210.0),
            PaperSize::A3 => (420.0, 297.0),
            PaperSize::A2 => (594.0, 420.0),
            PaperSize::A1 => (841.0, 594.0),
            PaperSize::A0 => (1189.0, 841.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub enum Orientation {
    Landscape,
    Portrait,
}

/// シートの種別 (M4仕様 §8)。
///
/// 同じ図枠の中に何を描くシートかで、使える道具と検証が変わる。
/// - [`SheetKind::Schematic`]: 回路図。配線・シンボルを置き、ERC・電気検証の対象
/// - [`SheetKind::Panel`]: 2D盤レイアウト。盤外形・DINレール・ダクト・フットプリントを置く。
///   配線が無いので回路図のERCは行わない ([`crate::verify::verify_sheet`])
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SheetKind {
    /// 回路図 (既定)。種別を持たない旧ファイルもこれで開く。
    #[default]
    Schematic,
    /// 2D盤レイアウト。
    Panel,
}

/// 図面シート1枚。JIS図枠・表題欄・改訂欄を持つ。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Sheet {
    pub id: SheetId,
    pub name: String,
    /// シート種別 (回路図 / 盤レイアウト)。旧ファイルは回路図。
    #[serde(default)]
    pub kind: SheetKind,
    pub size: PaperSize,
    pub orientation: Orientation,
    /// ゾーン分割数(横方向)。縦方向は用紙比率から自動。
    pub zone_cols: u32,
    pub zone_rows: u32,
    pub title_block: TitleBlock,
    #[serde(default)]
    pub revisions: Vec<Revision>,
    /// 注記文字高 (mm)。図面規格として**シート単位**で決める、配線注記・線番・ネットタグの
    /// 文字の大きさ。`None`は既定 ([`DEFAULT_ANNOTATION_TEXT_MM`])。
    ///
    /// 参考図面のように5mmピッチで機器が並ぶ図面では、既定の2.0mmだと注記が隣の行に
    /// かかるため1.4mm前後まで下げる。文字だけでなく配線からの逃がし量も同じ比率で縮み、
    /// 注記の層全体が小さくなる ([`Sheet::annotation_scale`])。
    #[serde(default)]
    pub annotation_text_mm: Option<f64>,
    pub entities: BTreeMap<EntityId, Entity>,
}

impl Sheet {
    /// 回路図シートを1枚作る。
    pub fn new(name: &str, size: PaperSize, orientation: Orientation) -> Self {
        Self::new_kind(name, size, orientation, SheetKind::Schematic)
    }

    /// 種別を指定してシートを1枚作る (盤レイアウトシートはこちら)。
    pub fn new_kind(
        name: &str,
        size: PaperSize,
        orientation: Orientation,
        kind: SheetKind,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            kind,
            size,
            orientation,
            zone_cols: 4,
            zone_rows: 6,
            title_block: TitleBlock::default(),
            revisions: Vec::new(),
            annotation_text_mm: None,
            entities: BTreeMap::new(),
        }
    }

    /// 盤レイアウトシートか。
    pub fn is_panel(&self) -> bool {
        self.kind == SheetKind::Panel
    }

    /// このシートの注記文字高 (mm)。未設定なら既定 ([`DEFAULT_ANNOTATION_TEXT_MM`])。
    pub fn annotation_text_mm(&self) -> f64 {
        self.annotation_text_mm
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or(DEFAULT_ANNOTATION_TEXT_MM)
    }

    /// 注記の層 (配線注記・線番・ネットタグ) にかける倍率。1.0 = 既定の大きさ。
    /// 文字高さも配線からの逃がし量も、この倍率で一緒に縮む・伸びる。
    pub fn annotation_scale(&self) -> f64 {
        self.annotation_text_mm() / DEFAULT_ANNOTATION_TEXT_MM
    }

    /// 用紙サイズ (幅, 高さ) mm。向きを反映。
    pub fn paper_mm(&self) -> (f64, f64) {
        let (w, h) = self.size.dimensions_mm();
        match self.orientation {
            Orientation::Landscape => (w, h),
            Orientation::Portrait => (h, w),
        }
    }
}

/// JIS表題欄。
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TitleBlock {
    #[serde(default)]
    pub company: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub drawing_no: String,
    #[serde(default)]
    pub scale: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub designed: String,
    #[serde(default)]
    pub drawn: String,
    #[serde(default)]
    pub checked: String,
    #[serde(default)]
    pub approved: String,
    #[serde(default)]
    pub rev: String,
}

/// 改訂欄の1行。
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Revision {
    pub mark: String,
    pub date: String,
    pub description: String,
    pub by: String,
}

/// シート上に置ける図形要素。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Entity {
    Symbol(SymbolInstance),
    Wire(Wire),
    Junction(Junction),
    NetLabel(NetLabel),
    Text(TextEntity),
    Harness(Harness),
    /// 図形 (G10)。電気的な意味を持たない作図要素 ([`crate::shape`])。
    Shape(Shape),
    /// 盤の外形 (筐体) と取付板。盤レイアウトシート専用 (M4仕様 §8)。
    PanelOutline(PanelOutline),
    /// DINレール。盤レイアウトシート専用。
    DinRail(DinRail),
    /// 配線ダクト。盤レイアウトシート専用。
    Duct(Duct),
    /// 部品のフットプリント (盤面に占める外形)。盤レイアウトシート専用。
    Footprint(Footprint),
    /// 凡例表 (アイテム番号↔参照記号・型番)。盤レイアウトシート専用。
    PanelLegend(PanelLegend),
}

impl Entity {
    pub fn id(&self) -> EntityId {
        match self {
            Entity::Symbol(e) => e.id,
            Entity::Wire(e) => e.id,
            Entity::Junction(e) => e.id,
            Entity::NetLabel(e) => e.id,
            Entity::Text(e) => e.id,
            Entity::Harness(e) => e.id,
            Entity::Shape(e) => e.id,
            Entity::PanelOutline(e) => e.id,
            Entity::DinRail(e) => e.id,
            Entity::Duct(e) => e.id,
            Entity::Footprint(e) => e.id,
            Entity::PanelLegend(e) => e.id,
        }
    }

    pub fn translate(&mut self, dx: f64, dy: f64) {
        match self {
            Entity::Symbol(e) => e.at = e.at.translated(dx, dy),
            Entity::Wire(e) => {
                for p in &mut e.points {
                    *p = p.translated(dx, dy);
                }
            }
            Entity::Junction(e) => e.at = e.at.translated(dx, dy),
            Entity::NetLabel(e) => e.at = e.at.translated(dx, dy),
            Entity::Text(e) => e.at = e.at.translated(dx, dy),
            Entity::Harness(e) => {
                for p in &mut e.points {
                    *p = p.translated(dx, dy);
                }
            }
            Entity::Shape(e) => {
                for p in &mut e.points {
                    *p = p.translated(dx, dy);
                }
            }
            Entity::PanelOutline(e) => e.at = e.at.translated(dx, dy),
            Entity::DinRail(e) => {
                e.x1 += dx;
                e.x2 += dx;
                e.y += dy;
            }
            Entity::Duct(e) => e.at = e.at.translated(dx, dy),
            Entity::Footprint(e) => e.at = e.at.translated(dx, dy),
            Entity::PanelLegend(e) => e.at = e.at.translated(dx, dy),
        }
    }
}

/// 盤の外形 (筐体) と取付板 (M4仕様 §8)。
///
/// 1エンティティで盤の外周と、その内側の取付板 (部品を留める板) の2重の矩形を表す。
/// 取付板の位置は [`crate::panel::plate_rect`] が決める (外周から一定量内側)。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PanelOutline {
    pub id: EntityId,
    /// 外形の左上角 (用紙座標mm)。
    pub at: Point,
    /// 外形の幅 mm。
    pub w: f64,
    /// 外形の高さ mm。
    pub h: f64,
    /// 盤の名前 (例 "制御盤")。空なら描かない。
    #[serde(default)]
    pub name: String,
}

/// DINレール (M4仕様 §8)。v1は水平のみ。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DinRail {
    pub id: EntityId,
    /// レールの取付線 (帯の中心) のy座標 mm。部品はこの線に合わせて乗る。
    pub y: f64,
    /// 左右の端 mm (どちらが大きくてもよい)。
    pub x1: f64,
    pub x2: f64,
}

/// 配線ダクト (M4仕様 §8)。斜線 (ハッチ) で塗った矩形として描く。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Duct {
    pub id: EntityId,
    /// 左上角 (用紙座標mm)。
    pub at: Point,
    pub w: f64,
    pub h: f64,
}

/// 部品のフットプリント = 盤面に占める外形 (M4仕様 §8)。
///
/// `reference`で回路図のシンボル (同じ参照記号) と結び付く。回路図にあってフットプリントが
/// 無い部品は「未配置」 ([`crate::panel::unplaced_components`])、逆に回路図に無い参照記号の
/// フットプリントは検証で報告される ([`crate::panel::panel_diagnostics`])。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Footprint {
    pub id: EntityId,
    /// 描かれる矩形の左上角 (用紙座標mm)。
    pub at: Point,
    /// 部品そのものの幅 mm (部品DBの`footprint_w`。回転前の値)。
    pub w: f64,
    /// 部品そのものの高さ mm (部品DBの`footprint_h`。回転前の値)。
    pub h: f64,
    /// 回転角(度)。v1は0か90のみ (90で幅と高さが入れ替わる)。
    #[serde(default)]
    pub rotation: u16,
    /// 回路図のシンボルと結び付く参照記号 (例 "K1")。空なら盤だけの部品。
    #[serde(default)]
    pub reference: String,
    /// 型番。
    #[serde(default)]
    pub part_no: String,
    /// バルーンに描くアイテム番号 (M4仕様 §8)。[`crate::command::Command::RenumberPanelItems`]
    /// が盤シートの読み順で 1..N を振る。**Noneならバルーンを描かない** (まだ採番していない)。
    /// 部品表 ([`crate::reports::bom_rows`]) と盤内部品配置表はこの番号で連動する。
    #[serde(default)]
    pub item_no: Option<u32>,
}

/// 凡例表 = アイテム番号と参照記号・型番の対応表 (M4仕様 §8)。盤レイアウトシート専用。
///
/// **中身は持たない**。表に並ぶ行は描画のたびに同じシートのフットプリントから導出する
/// ([`crate::panel::legend_rows`]) ので、部品を足しても採番し直しても手直しが要らない
/// (帳票と同じ「図面から導出する」原則)。このエンティティが決めるのは**置き場所だけ**で、
/// 移動・削除・undoは他のエンティティと同じコマンドで効く。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PanelLegend {
    pub id: EntityId,
    /// 表の左上角 (用紙座標mm)。
    pub at: Point,
}

/// ライブラリシンボルの配置インスタンス。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SymbolInstance {
    pub id: EntityId,
    /// シンボルライブラリのキー(例: "relay_coil")。
    pub symbol_id: String,
    pub at: Point,
    /// 回転角(度)。0/90/180/270のみ。
    #[serde(default)]
    pub rotation: u16,
    #[serde(default)]
    pub mirror: bool,
    /// 参照記号(例: "K1", "J3")。
    #[serde(default)]
    pub reference: String,
    /// 型番・値(例: "[redacted]", "10kΩ")。
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub attrs: BTreeMap<String, String>,
}

/// 配線。直交セグメントのポリライン。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Wire {
    pub id: EntityId,
    pub points: Vec<Point>,
    /// 線色(例: "red", "black", "light_blue")。
    #[serde(default)]
    pub color: String,
    /// 線径 sq (mm2)。例: 0.3, 0.75, 3.5。
    #[serde(default)]
    pub sq: f64,
    /// 電線長(m)。BOM/電線リスト用。
    #[serde(default)]
    pub length_m: Option<f64>,
    /// 電線品番。
    #[serde(default)]
    pub part_no: Option<String>,
    /// ネット名(明示指定時のみ。通常はネットリスト抽出で導出)。
    #[serde(default)]
    pub net: Option<String>,
}

/// 1本のワイヤへの線番の割り当て (`set_wire_numbers` コマンドの要素)。
/// numberがNoneなら線番を消す。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WireNumber {
    pub wire_id: EntityId,
    #[serde(default)]
    pub number: Option<String>,
}

/// 1つのフットプリントへのアイテム番号の割り当て (`set_panel_item_numbers` コマンドの要素)。
/// item_noがNoneなら番号を消す (バルーンも消える)。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PanelItemNumber {
    pub footprint_id: EntityId,
    #[serde(default)]
    pub item_no: Option<u32>,
}

/// 配線の交差接続点。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Junction {
    pub id: EntityId,
    pub at: Point,
}

/// ネットラベル(例: "[redacted]")。図面間・シート間の論理接続。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NetLabel {
    pub id: EntityId,
    pub at: Point,
    pub name: String,
    #[serde(default)]
    pub rotation: u16,
    /// 表示の追加指定。現状のキーは`tag` (`left`/`right`) で、方向付きタグ形状で描く
    /// ([`crate::net_tag`])。省略・未知の値なら従来どおり文字だけを描く。
    #[serde(default)]
    pub attrs: BTreeMap<String, String>,
}

/// ハーネス境界 (IEC 61082-1 のグループ囲み)。まとめて製作・購入する電線束の範囲を
/// 破線で囲んで名前を付ける。所属するワイヤは幾何学的な内包で決まる ([`crate::harness`])。
///
/// **電線をまとめる境界専用**。筐体・盤・強調枠の囲みは [`Entity::Shape`] で描く (G11):
/// ハーネスで囲むと、中にあるだけの導線まで電線リストのハーネス列に載ってしまう。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Harness {
    pub id: EntityId,
    /// 囲みの頂点 (現状は矩形の4点。多角形は将来)。
    pub points: Vec<Point>,
    /// ハーネス名 (参照記号と同じ命名規則: W1, W2 …)。
    #[serde(default)]
    pub name: String,
    /// 備考 (製作指示など)。
    #[serde(default)]
    pub note: String,
}

/// 図形の種類 (G10)。`points`の読み方がこれで決まる ([`crate::shape`])。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind {
    /// 矩形。`points`は**対角の2点**(どちら回りでもよい)。
    #[default]
    Rect,
    /// 直線。`points`は**両端の2点**。
    Line,
    /// 円。`points`は**中心と円周上の1点**で、その距離が半径。
    Circle,
    /// 折れ線。`points`は**通る点を順に**(2点以上)。
    Polyline,
}

/// 図形 (G10) = 電気的な意味を持たない、純粋な作図要素。
///
/// 筐体の囲み・強調枠・注記の枠・塗りつぶしの機器ブロックはこれで描く。
/// **ネットリスト・ERC・部品表・From-To表・整頓メトリクスのどれにも出てこない**ので、
/// 図面に何本描いても電線リストや検証結果は1行も変わらない (これがハーネス境界
/// [`Harness`] を筐体の囲みに流用しないで済む理由 = G11)。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Shape {
    pub id: EntityId,
    /// 図形の種類。`points`の読み方がこれで決まる。
    #[serde(default, rename = "shape")]
    pub shape: ShapeKind,
    /// 頂点 (用紙座標mm)。必要な数は種別ごとに決まる
    /// ([`crate::shape::required_points`])。
    pub points: Vec<Point>,
    /// 線色の**トークン名** (`default` / `accent` / `warning` …
    /// [`crate::text_style::TEXT_COLORS`])。注記と同じトークンを共有する。
    ///
    /// 生の色コード (`#ff0000`) は受け付けない: 画面 (暗い作図領域) と紙 (白地) で
    /// 見やすい色が違うので、図面には**意味**だけを持たせて実際の色は描くときに選ぶ。
    #[serde(default = "default_shape_stroke")]
    pub stroke: String,
    /// 線幅 mm ([`crate::shape::MIN_SHAPE_STROKE_MM`]〜[`crate::shape::MAX_SHAPE_STROKE_MM`])。
    #[serde(default = "default_shape_stroke_width")]
    pub stroke_width_mm: f64,
    /// 塗りの色トークン。`None`なら塗らない。塗っても下の回路が透けて読める濃さで塗る
    /// ([`crate::shape::SHAPE_FILL_OPACITY`])。
    #[serde(default)]
    pub fill: Option<String>,
    /// 破線で描くか (既定=false = 実線)。
    #[serde(default)]
    pub dashed: bool,
}

fn default_shape_stroke() -> String {
    crate::shape::DEFAULT_SHAPE_COLOR.to_string()
}

fn default_shape_stroke_width() -> f64 {
    crate::shape::DEFAULT_SHAPE_STROKE_MM
}

/// 注記テキストの基準位置 = `at`に文字のどこを合わせるか (G4)。
///
/// 図面の右側に並ぶ機器へ注記を寄せて書くときは[`TextAnchor::End`] (右揃え) にすると、
/// 文字の長さが変わっても右端がそろう。
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TextAnchor {
    /// 左端を合わせる (既定。装飾が無かった頃の注記と同じ)。
    #[default]
    Start,
    /// 中央を合わせる。
    Middle,
    /// 右端を合わせる。
    End,
}

/// 自由テキスト注記。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TextEntity {
    pub id: EntityId,
    pub at: Point,
    pub text: String,
    /// 文字高さ mm。
    pub height: f64,
    #[serde(default)]
    pub rotation: u16,
    /// 文字の基準位置 (既定=左揃え)。
    #[serde(default)]
    pub anchor: TextAnchor,
    /// 注記色の**トークン名** (`default` / `accent` / `warning` …
    /// [`crate::text_style::TEXT_COLORS`])。`None`は既定色 (黒)。
    ///
    /// 生の色コード (`#ff0000`) は受け付けない: 画面 (暗い作図領域) と紙 (白地) で
    /// 見やすい色が違うので、図面には**意味**だけを持たせて実際の色は描くときに選ぶ。
    /// 知らない名前を入れたコマンドは[`crate::command::Engine::execute`]が拒否する。
    #[serde(default)]
    pub color: Option<String>,
    /// 太字にするか (既定=false)。
    #[serde(default)]
    pub bold: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Paper sizes follow ISO A-series dimensions, and portrait orientation swaps width and height.
    /// 用紙サイズはISO A列の寸法に従い、縦置きでは幅と高さが入れ替わる。
    #[test]
    fn paper_sizes_match_iso_and_orientation_swaps() {
        assert_eq!(PaperSize::A3.dimensions_mm(), (420.0, 297.0));
        assert_eq!(PaperSize::A0.dimensions_mm(), (1189.0, 841.0));
        let mut sheet = Sheet::new("t", PaperSize::A4, Orientation::Landscape);
        assert_eq!(sheet.paper_mm(), (297.0, 210.0));
        sheet.orientation = Orientation::Portrait;
        assert_eq!(sheet.paper_mm(), (210.0, 297.0));
    }

    /// A new project starts with one sheet named "Sheet1", the current file format version, an empty PLC assignment table and no explicit frame template (so it uses the default JIS A3 frame).
    /// 新規プロジェクトは「Sheet1」という1枚のシート・現行のファイル形式バージョン・空のPLC割付表で始まり、図枠テンプレートは未指定(=既定のJIS A3図枠)である。
    #[test]
    fn new_project_has_one_default_sheet() {
        let p = Project::new("demo");
        assert_eq!(p.format_version, FORMAT_VERSION);
        assert_eq!(p.sheets.len(), 1);
        assert_eq!(p.sheets[0].name, "Sheet1");
        assert_eq!(p.sheets[0].zone_cols, 4);
        assert_eq!(p.sheets[0].zone_rows, 6);
        assert!(p.plc_assignments.is_empty());
        assert_eq!(p.frame_template, None);
    }

    /// Entity::translate moves every coordinate of the entity: all wire points, or the anchor of symbols/labels/text.
    /// Entity::translateはエンティティの全座標を動かす: ワイヤは全頂点、シンボル/ラベル/テキストは基準点。
    #[test]
    fn translate_moves_all_coordinates() {
        let mut wire = Entity::Wire(Wire {
            id: uuid::Uuid::new_v4(),
            points: vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)],
            color: "red".into(),
            sq: 0.3,
            length_m: None,
            part_no: None,
            net: None,
        });
        wire.translate(5.0, 2.5);
        let Entity::Wire(w) = &wire else { panic!() };
        assert_eq!(w.points[0], Point::new(5.0, 2.5));
        assert_eq!(w.points[1], Point::new(15.0, 2.5));

        let mut label = Entity::NetLabel(NetLabel {
            id: uuid::Uuid::new_v4(),
            at: Point::new(1.0, 1.0),
            name: "24V".into(),
            rotation: 0,
            attrs: Default::default(),
        });
        label.translate(-1.0, -1.0);
        let Entity::NetLabel(l) = &label else { panic!() };
        assert_eq!(l.at, Point::new(0.0, 0.0));
    }

    /// A new sheet is a schematic sheet, and a panel-layout sheet is a separate sheet kind recorded in the file.
    /// 新しいシートは回路図シートで、盤レイアウトシートはファイルに記録される別のシート種別である。
    #[test]
    fn a_new_sheet_is_schematic_and_a_panel_sheet_is_a_separate_kind() {
        let schematic = Sheet::new("t", PaperSize::A3, Orientation::Landscape);
        assert_eq!(schematic.kind, SheetKind::Schematic);
        assert!(!schematic.is_panel());
        let panel = Sheet::new_kind("盤", PaperSize::A3, Orientation::Landscape, SheetKind::Panel);
        assert_eq!(panel.kind, SheetKind::Panel);
        assert!(panel.is_panel());
    }

    /// A sheet saved before panel layouts existed carries no sheet kind, and it opens as an ordinary schematic sheet.
    /// 盤レイアウトが無かった頃に保存されたシートは種別を持たないが、通常の回路図シートとして開く。
    #[test]
    fn a_sheet_without_a_kind_opens_as_a_schematic_sheet() {
        let json = r#"{"id":"11111111-1111-4111-8111-111111111111","name":"S1","size":"A3",
            "orientation":"Landscape","zone_cols":4,"zone_rows":6,"title_block":{},"entities":{}}"#;
        let sheet: Sheet = serde_json::from_str(json).expect("旧シートが読める");
        assert_eq!(sheet.kind, SheetKind::Schematic);
    }

    /// The sheet kind travels as JSON in snake_case ("schematic" / "panel").
    /// シート種別は snake_case ("schematic" / "panel") のJSONで往復する。
    #[test]
    fn the_sheet_kind_is_serialized_in_snake_case() {
        let json = serde_json::to_string(&SheetKind::Panel).unwrap();
        assert_eq!(json, "\"panel\"");
        let back: SheetKind = serde_json::from_str("\"schematic\"").unwrap();
        assert_eq!(back, SheetKind::Schematic);
    }

    /// Moving a panel sheet moves its panel entities too: outline, duct and footprint move by their top-left corner, and a DIN rail moves along both axes.
    /// 盤シートを動かすと盤エンティティも動く: 盤外形・ダクト・フットプリントは左上の角が動き、DINレールは左右にも上下にも動く。
    #[test]
    fn translate_moves_panel_entities() {
        let id = uuid::Uuid::new_v4();
        let mut outline = Entity::PanelOutline(PanelOutline {
            id,
            at: Point::new(10.0, 20.0),
            w: 100.0,
            h: 50.0,
            name: "盤A".into(),
        });
        outline.translate(5.0, -2.5);
        let Entity::PanelOutline(o) = &outline else { panic!() };
        assert_eq!(o.at, Point::new(15.0, 17.5));
        assert_eq!((o.w, o.h), (100.0, 50.0), "寸法は変わらない");

        let mut rail = Entity::DinRail(DinRail {
            id,
            y: 60.0,
            x1: 20.0,
            x2: 120.0,
        });
        rail.translate(10.0, 5.0);
        let Entity::DinRail(r) = &rail else { panic!() };
        assert_eq!((r.x1, r.x2, r.y), (30.0, 130.0, 65.0));

        let mut duct = Entity::Duct(Duct {
            id,
            at: Point::new(0.0, 0.0),
            w: 40.0,
            h: 10.0,
        });
        duct.translate(2.5, 2.5);
        let Entity::Duct(d) = &duct else { panic!() };
        assert_eq!(d.at, Point::new(2.5, 2.5));

        let mut fp = Entity::Footprint(Footprint {
            id,
            at: Point::new(50.0, 50.0),
            w: 20.0,
            h: 40.0,
            rotation: 0,
            reference: "K1".into(),
            part_no: "MDK-RLY-24V".into(),
            item_no: None,
        });
        fp.translate(-10.0, 0.0);
        let Entity::Footprint(f) = &fp else { panic!() };
        assert_eq!(f.at, Point::new(40.0, 50.0));
        assert_eq!(f.reference, "K1");
    }

    /// Panel entities travel as JSON under their own kind tags, so any client can send them.
    /// 盤エンティティはそれぞれの種別タグでJSONを往復でき、どのクライアントからも送れる。
    #[test]
    fn panel_entities_have_their_own_json_kind_tags() {
        let id = uuid::Uuid::new_v4();
        for (entity, tag) in [
            (
                Entity::PanelOutline(PanelOutline {
                    id,
                    at: Point::new(0.0, 0.0),
                    w: 10.0,
                    h: 10.0,
                    name: String::new(),
                }),
                "panel_outline",
            ),
            (
                Entity::DinRail(DinRail {
                    id,
                    y: 0.0,
                    x1: 0.0,
                    x2: 10.0,
                }),
                "din_rail",
            ),
            (
                Entity::Duct(Duct {
                    id,
                    at: Point::new(0.0, 0.0),
                    w: 10.0,
                    h: 10.0,
                }),
                "duct",
            ),
            (
                Entity::Footprint(Footprint {
                    id,
                    at: Point::new(0.0, 0.0),
                    w: 10.0,
                    h: 10.0,
                    rotation: 90,
                    reference: "K1".into(),
                    part_no: String::new(),
                    item_no: None,
                }),
                "footprint",
            ),
        ] {
            let json = serde_json::to_string(&entity).unwrap();
            assert!(json.contains(&format!("\"kind\":\"{tag}\"")), "{json}");
            let back: Entity = serde_json::from_str(&json).unwrap();
            assert_eq!(back.id(), id);
        }
    }

    /// A note written before text styling existed opens with the look it always had: left-aligned, plain black, not bold.
    /// テキスト装飾が無かった頃に書かれた注記は、従来どおりの見た目 (左揃え・黒・太字でない) で開く。
    #[test]
    fn an_old_note_opens_with_the_previous_look() {
        let json = r#"{"kind":"text","id":"33333333-3333-4333-8333-333333333333",
            "at":{"x":10.0,"y":20.0},"text":"注意","height":2.5,"rotation":0}"#;
        let entity: Entity = serde_json::from_str(json).expect("旧い注記が読める");
        let Entity::Text(t) = entity else { panic!("注記であること") };
        assert_eq!(t.anchor, TextAnchor::Start, "従来どおり左揃え");
        assert_eq!(t.color, None, "色指定なし = 既定色 (黒)");
        assert!(!t.bold, "従来どおり太字でない");
    }

    /// The text anchor travels as JSON in snake_case ("start" / "middle" / "end").
    /// テキストの基準位置は snake_case ("start" / "middle" / "end") のJSONで往復する。
    #[test]
    fn the_text_anchor_is_serialized_in_snake_case() {
        assert_eq!(serde_json::to_string(&TextAnchor::Middle).unwrap(), "\"middle\"");
        assert_eq!(serde_json::to_string(&TextAnchor::End).unwrap(), "\"end\"");
        let back: TextAnchor = serde_json::from_str("\"start\"").unwrap();
        assert_eq!(back, TextAnchor::Start);
        assert_eq!(TextAnchor::default(), TextAnchor::Start);
    }

    /// A styled note keeps its anchor, colour token and bold flag through a JSON round trip.
    /// 装飾を付けた注記は、基準位置・色トークン・太字の指定をJSONの往復でも保つ。
    #[test]
    fn a_styled_note_survives_a_json_round_trip() {
        let entity = Entity::Text(TextEntity {
            id: uuid::Uuid::new_v4(),
            at: Point::new(1.0, 2.0),
            text: "非常停止".into(),
            height: 3.0,
            rotation: 0,
            anchor: TextAnchor::End,
            color: Some("warning".into()),
            bold: true,
        });
        let json = serde_json::to_string(&entity).unwrap();
        let Entity::Text(back) = serde_json::from_str::<Entity>(&json).unwrap() else {
            panic!("注記であること")
        };
        assert_eq!(back.anchor, TextAnchor::End);
        assert_eq!(back.color.as_deref(), Some("warning"));
        assert!(back.bold);
    }

    /// Moving a shape shifts every one of its points, so a rectangle keeps its size and a polyline keeps its bends.
    /// 図形を動かすと全ての頂点が動くので、矩形は大きさを保ち、折れ線は折れ方を保つ。
    #[test]
    fn translate_moves_every_point_of_a_shape() {
        let mut rect = Entity::Shape(Shape {
            id: uuid::Uuid::new_v4(),
            shape: ShapeKind::Rect,
            points: vec![Point::new(10.0, 20.0), Point::new(50.0, 40.0)],
            stroke: "accent".into(),
            stroke_width_mm: 0.25,
            fill: None,
            dashed: true,
        });
        rect.translate(5.0, -2.5);
        let Entity::Shape(s) = &rect else { panic!() };
        assert_eq!(s.points[0], Point::new(15.0, 17.5));
        assert_eq!(s.points[1], Point::new(55.0, 37.5));
        assert_eq!(s.stroke, "accent", "見た目の設定は動かしても変わらない");

        let mut poly = Entity::Shape(Shape {
            id: uuid::Uuid::new_v4(),
            shape: ShapeKind::Polyline,
            points: vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0), Point::new(10.0, 10.0)],
            stroke: "default".into(),
            stroke_width_mm: 0.25,
            fill: None,
            dashed: false,
        });
        poly.translate(1.0, 1.0);
        let Entity::Shape(s) = &poly else { panic!() };
        assert_eq!(s.points[2], Point::new(11.0, 11.0));
    }

    /// A shape travels as JSON under the kind tag "shape", with its own kind of shape in snake_case, so any client can send one.
    /// 図形は kind="shape" のJSONで往復し、図形の種類は snake_case で入るので、どのクライアントからも送れる。
    #[test]
    fn a_shape_is_serialized_under_the_shape_kind_tag() {
        let id = uuid::Uuid::new_v4();
        let entity = Entity::Shape(Shape {
            id,
            shape: ShapeKind::Circle,
            points: vec![Point::new(0.0, 0.0), Point::new(5.0, 0.0)],
            stroke: "warning".into(),
            stroke_width_mm: 0.5,
            fill: Some("muted".into()),
            dashed: true,
        });
        let json = serde_json::to_string(&entity).unwrap();
        assert!(json.contains("\"kind\":\"shape\""), "{json}");
        assert!(json.contains("\"shape\":\"circle\""), "{json}");
        let Entity::Shape(back) = serde_json::from_str::<Entity>(&json).unwrap() else {
            panic!("図形であること")
        };
        assert_eq!(back.id, id);
        assert_eq!(back.shape, ShapeKind::Circle);
        assert_eq!(back.fill.as_deref(), Some("muted"));
        assert!(back.dashed);
        assert_eq!(back.stroke_width_mm, 0.5);
    }

    /// The four kinds of shape all travel in snake_case.
    /// 4種類の図形はいずれも snake_case でJSONを往復する。
    #[test]
    fn the_shape_kinds_are_serialized_in_snake_case() {
        for (kind, tag) in [
            (ShapeKind::Rect, "\"rect\""),
            (ShapeKind::Line, "\"line\""),
            (ShapeKind::Circle, "\"circle\""),
            (ShapeKind::Polyline, "\"polyline\""),
        ] {
            assert_eq!(serde_json::to_string(&kind).unwrap(), tag);
            assert_eq!(serde_json::from_str::<ShapeKind>(tag).unwrap(), kind);
        }
    }

    /// A shape sent without the optional look fields opens plain: default colour, hairline stroke, no fill, solid line.
    /// 見た目の欄を省いて送られた図形は、既定色・細線・塗りなし・実線という素の形で開く。
    #[test]
    fn a_shape_without_look_fields_opens_plain() {
        let json = r#"{"kind":"shape","id":"44444444-4444-4444-8444-444444444444",
            "shape":"rect","points":[{"x":0.0,"y":0.0},{"x":10.0,"y":10.0}]}"#;
        let Entity::Shape(s) = serde_json::from_str::<Entity>(json).expect("図形が読める") else {
            panic!("図形であること")
        };
        assert_eq!(s.stroke, crate::shape::DEFAULT_SHAPE_COLOR);
        assert_eq!(s.stroke_width_mm, crate::shape::DEFAULT_SHAPE_STROKE_MM);
        assert_eq!(s.fill, None);
        assert!(!s.dashed);
    }

    /// Entity::id() returns the inner entity's UUID regardless of the entity kind.
    /// Entity::id()は種別によらず内側エンティティのUUIDを返す。
    #[test]
    fn entity_id_is_uniform_across_kinds() {
        let id = uuid::Uuid::new_v4();
        let j = Entity::Junction(Junction { id, at: Point::new(0.0, 0.0) });
        assert_eq!(j.id(), id);
    }
}
