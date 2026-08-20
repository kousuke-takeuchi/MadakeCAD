use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::geometry::Point;

pub type EntityId = Uuid;
pub type SheetId = Uuid;

/// プロジェクト全体。保存形式(.mdkproj)のルート。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Project {
    pub format_version: u32,
    pub name: String,
    pub sheets: Vec<Sheet>,
    /// 電線品番マスタ: 線色+線径から品番を引く。
    #[serde(default)]
    pub wire_parts: Vec<WirePart>,
}

impl Project {
    pub fn new(name: &str) -> Self {
        Self {
            format_version: 1,
            name: name.to_string(),
            sheets: vec![Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape)],
            wire_parts: Vec::new(),
        }
    }

    pub fn sheet(&self, id: SheetId) -> Option<&Sheet> {
        self.sheets.iter().find(|s| s.id == id)
    }

    pub fn sheet_mut(&mut self, id: SheetId) -> Option<&mut Sheet> {
        self.sheets.iter_mut().find(|s| s.id == id)
    }
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

/// 図面シート1枚。JIS図枠・表題欄・改訂欄を持つ。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Sheet {
    pub id: SheetId,
    pub name: String,
    pub size: PaperSize,
    pub orientation: Orientation,
    /// ゾーン分割数(横方向)。縦方向は用紙比率から自動。
    pub zone_cols: u32,
    pub zone_rows: u32,
    pub title_block: TitleBlock,
    #[serde(default)]
    pub revisions: Vec<Revision>,
    pub entities: BTreeMap<EntityId, Entity>,
}

impl Sheet {
    pub fn new(name: &str, size: PaperSize, orientation: Orientation) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            size,
            orientation,
            zone_cols: 4,
            zone_rows: 6,
            title_block: TitleBlock::default(),
            revisions: Vec::new(),
            entities: BTreeMap::new(),
        }
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
}

impl Entity {
    pub fn id(&self) -> EntityId {
        match self {
            Entity::Symbol(e) => e.id,
            Entity::Wire(e) => e.id,
            Entity::Junction(e) => e.id,
            Entity::NetLabel(e) => e.id,
            Entity::Text(e) => e.id,
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
        }
    }
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
    /// 型番・値(例: "JZX-22F", "10kΩ")。
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

/// 配線の交差接続点。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Junction {
    pub id: EntityId,
    pub at: Point,
}

/// ネットラベル(例: "24-P1")。図面間・シート間の論理接続。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct NetLabel {
    pub id: EntityId,
    pub at: Point,
    pub name: String,
    #[serde(default)]
    pub rotation: u16,
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
}
