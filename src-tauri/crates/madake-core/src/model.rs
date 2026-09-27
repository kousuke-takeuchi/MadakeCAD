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
pub const FORMAT_VERSION: u32 = 2;

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
}

impl Project {
    pub fn new(name: &str) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            name: name.to_string(),
            sheets: vec![Sheet::new("Sheet1", PaperSize::A3, Orientation::Landscape)],
            wire_parts: Vec::new(),
            plc_assignments: Vec::new(),
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
    Harness(Harness),
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
        }
    }

    /// idを付け替える (複製で新しいidを振るとき用)。
    pub fn set_id(&mut self, id: EntityId) {
        match self {
            Entity::Symbol(e) => e.id = id,
            Entity::Wire(e) => e.id = id,
            Entity::Junction(e) => e.id = id,
            Entity::NetLabel(e) => e.id = id,
            Entity::Text(e) => e.id = id,
            Entity::Harness(e) => e.id = id,
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

/// 1本のワイヤへの線番の割り当て (`set_wire_numbers` コマンドの要素)。
/// numberがNoneなら線番を消す。
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WireNumber {
    pub wire_id: EntityId,
    #[serde(default)]
    pub number: Option<String>,
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

/// ハーネス境界 (IEC 61082-1 のグループ囲み)。まとめて製作・購入する電線束の範囲を
/// 破線で囲んで名前を付ける。所属するワイヤは幾何学的な内包で決まる ([`crate::harness`])。
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

    /// A new project starts with one sheet named "Sheet1", the current file format version and an empty PLC assignment table.
    /// 新規プロジェクトは「Sheet1」という1枚のシート・現行のファイル形式バージョン・空のPLC割付表で始まる。
    #[test]
    fn new_project_has_one_default_sheet() {
        let p = Project::new("demo");
        assert_eq!(p.format_version, FORMAT_VERSION);
        assert_eq!(p.sheets.len(), 1);
        assert_eq!(p.sheets[0].name, "Sheet1");
        assert_eq!(p.sheets[0].zone_cols, 4);
        assert_eq!(p.sheets[0].zone_rows, 6);
        assert!(p.plc_assignments.is_empty());
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
        });
        label.translate(-1.0, -1.0);
        let Entity::NetLabel(l) = &label else { panic!() };
        assert_eq!(l.at, Point::new(0.0, 0.0));
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
