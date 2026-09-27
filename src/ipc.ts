// Tauri IPCラッパとmadake-coreのserde表現に対応するTS型定義。
// Rust側のtagged enum表現: Command={type}, Entity={kind}, PatchOp={op}

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface Point {
  x: number;
  y: number;
}

export type PaperSize = "A4" | "A3" | "A2" | "A1" | "A0";
export type Orientation = "Landscape" | "Portrait";

export interface TitleBlock {
  company?: string;
  title?: string;
  drawing_no?: string;
  scale?: string;
  date?: string;
  designed?: string;
  drawn?: string;
  checked?: string;
  approved?: string;
  rev?: string;
}

export interface Revision {
  mark: string;
  date: string;
  description: string;
  by: string;
}

export interface SymbolInstance {
  id: string;
  symbol_id: string;
  at: Point;
  rotation: number;
  mirror: boolean;
  reference: string;
  value: string;
  attrs: Record<string, string>;
}

export interface Wire {
  id: string;
  points: Point[];
  color: string;
  sq: number;
  length_m: number | null;
  part_no: string | null;
  net: string | null;
}

export interface Junction {
  id: string;
  at: Point;
}

export interface NetLabel {
  id: string;
  at: Point;
  name: string;
  rotation: number;
  /** 表示の追加指定。`tag`(`left`/`right`)で方向付きタグ形状で描く (canvas/netTag.ts)。 */
  attrs?: Record<string, string>;
}

/** 注記テキストの基準位置 = `at`に文字のどこを合わせるか (G4)。既定は左揃え。 */
export type TextAnchor = "start" | "middle" | "end";

/** 注記色のトークン名 (canvas/textStyle.ts の TEXT_COLOR_TOKENS)。生の色コードは不可。 */
export type TextColorToken = "default" | "accent" | "warning" | "success" | "muted";

export interface TextEntity {
  id: string;
  at: Point;
  text: string;
  height: number;
  rotation: number;
  /** 文字の基準位置 (既定 "start" = 左揃え)。 */
  anchor?: TextAnchor;
  /** 注記色のトークン名。null/未指定は既定色。 */
  color?: TextColorToken | null;
  /** 太字にするか (既定 false)。 */
  bold?: boolean;
  /**
   * 引き出し線の対象エンティティ (G20)。設定されていれば、注記の外接矩形のうち対象に
   * いちばん近い辺の中点から、対象の外接矩形上のいちばん近い点まで線を引く
   * (canvas/leader.ts leaderFor)。null/未指定なら引き出し線は描かない。
   */
  leader_to?: string | null;
}

/**
 * ハーネス境界 (IEC 61082-1 のグループ囲み)。所属配線は幾何学的な内包で決まる。
 * **電線をまとめる境界専用**で、筐体・強調枠の囲みは`Shape`で描く (G11)。
 */
export interface Harness {
  id: string;
  /** 囲みの頂点 (現状は矩形の4点)。 */
  points: Point[];
  /** ハーネス名 (参照記号と同じ命名規則: W1, W2 …)。 */
  name: string;
  /** 備考 (製作指示など)。 */
  note: string;
}

/**
 * 図形の種類 (G10)。`points`の読み方がこれで決まる:
 * `rect`=対角の2点 / `line`=両端の2点 / `circle`=中心と円周上の1点 / `polyline`=通る点を順に。
 */
export type ShapeKind = "rect" | "line" | "circle" | "polyline";

/**
 * 図形 (G10) = 電気的な意味を持たない純粋な作図要素。筐体の囲み・強調枠・注記の枠に使う。
 * ネットリスト・ERC・部品表・電線リストのどれにも出てこないので、図面に何本描いても
 * 帳票は1行も変わらない (筐体をハーネス境界で囲まなくてよくなる = G11)。
 */
export interface Shape {
  id: string;
  /** 図形の種類。`points`の読み方がこれで決まる。 */
  shape: ShapeKind;
  /** 頂点 (用紙座標mm)。 */
  points: Point[];
  /** 線色のトークン名 (注記と同じトークン。生の色コードはコマンドが拒否する)。 */
  stroke: TextColorToken;
  /** 線幅 mm (0.1〜2.0)。 */
  stroke_width_mm: number;
  /** 塗りの色トークン。null/未指定なら塗らない。 */
  fill?: TextColorToken | null;
  /** 破線で描くか (既定 false = 実線)。 */
  dashed: boolean;
}

/** 盤の外形 (筐体) と取付板。盤レイアウトシート専用 (M4 §8)。 */
export interface PanelOutline {
  id: string;
  /** 外形の左上角 (用紙座標mm)。 */
  at: Point;
  w: number;
  h: number;
  /** 盤の名前 (例 "制御盤")。空なら描かない。 */
  name: string;
}

/** DINレール。v1は水平のみ。`y`は取付線 (帯の中心)。 */
export interface DinRail {
  id: string;
  y: number;
  x1: number;
  x2: number;
}

/** 配線ダクト (斜線ハッチで描く)。 */
export interface Duct {
  id: string;
  at: Point;
  w: number;
  h: number;
}

/** 部品が盤面に占める外形。`reference`で回路図のシンボルと結び付く。 */
export interface Footprint {
  id: string;
  /** 描かれる矩形の左上角 (用紙座標mm)。 */
  at: Point;
  /** 部品そのものの幅・高さ mm (回転前の値)。 */
  w: number;
  h: number;
  /** 回転角(度)。v1は0か90のみ。 */
  rotation: number;
  /** 回路図の参照記号 (例 "K1")。空なら盤だけの部品。 */
  reference: string;
  part_no: string;
  /**
   * バルーンに描くアイテム番号 (M4 §8)。`renumber_panel_items`が読み順で 1..N を振る。
   * null/未設定ならバルーンを描かない (まだ採番していない)。
   */
  item_no?: number | null;
}

/**
 * 凡例表 (アイテム番号↔参照記号・型番)。盤レイアウトシート専用。
 * **中身は持たず**、描画のたびに同じシートのフットプリントから導出する。
 */
export interface PanelLegend {
  id: string;
  /** 表の左上角 (用紙座標mm)。 */
  at: Point;
}

/**
 * 電線品番凡例ブロック (色+sq+電線品番+使用長のテーブル、G9残)。回路図シート専用。
 * `PanelLegend`と同じく**中身は持たず**、描画のたびに同じシートの配線から導出する。
 */
export interface WireLegend {
  id: string;
  /** 表の左上角 (用紙座標mm)。 */
  at: Point;
}

/**
 * 回路図にあって、どの盤レイアウトシートにもフットプリントが無い部品
 * (未配置部品パネルの1行。Rustの`madake_core::panel::UnplacedComponent`)。
 */
export interface UnplacedComponent {
  /** 参照記号 (例 "K1")。盤のフットプリントとの結び付き。 */
  reference: string;
  /** 型番 (回路図のシンボルの値)。空なら未指定。 */
  part_no: string;
  /** 代表シンボルのid。 */
  symbol_id: string;
  /** その部品が最初に出てくる回路図シート。 */
  sheet_id: string;
  /** 置くときの外形の幅 mm (部品DBの寸法。無ければ既定の矩形)。 */
  w: number;
  /** 置くときの外形の高さ mm。 */
  h: number;
}

export type Entity =
  | ({ kind: "symbol" } & SymbolInstance)
  | ({ kind: "wire" } & Wire)
  | ({ kind: "junction" } & Junction)
  | ({ kind: "net_label" } & NetLabel)
  | ({ kind: "text" } & TextEntity)
  | ({ kind: "harness" } & Harness)
  | ({ kind: "shape" } & Shape)
  | ({ kind: "panel_outline" } & PanelOutline)
  | ({ kind: "din_rail" } & DinRail)
  | ({ kind: "duct" } & Duct)
  | ({ kind: "footprint" } & Footprint)
  | ({ kind: "panel_legend" } & PanelLegend)
  | ({ kind: "wire_legend" } & WireLegend);

/** シート種別。回路図 (既定) と2D盤レイアウト (M4 §8)。 */
export type SheetKind = "schematic" | "panel";

export interface Sheet {
  id: string;
  name: string;
  /** シート種別。旧ファイルは種別を持たないので省略可 (=回路図)。 */
  kind?: SheetKind;
  size: PaperSize;
  orientation: Orientation;
  zone_cols: number;
  zone_rows: number;
  title_block: TitleBlock;
  revisions: Revision[];
  /**
   * 注記文字高 (mm)。配線注記・線番・ネットタグの大きさをシート単位で決める図面規格
   * (canvas/annotationText.ts)。未設定 (旧ファイル) は既定の2.0mm。
   */
  annotation_text_mm?: number | null;
  entities: Record<string, Entity>;
}

export interface WirePart {
  part_no: string;
  color: string;
  sq: number;
  note: string;
}

export interface Project {
  format_version: number;
  name: string;
  sheets: Sheet[];
  wire_parts: WirePart[];
  /** PLC I/O割付表 (信号名・コメントの正。接続先・線番は図面から導出)。 */
  plc_assignments: PlcAssignment[];
  /**
   * 図枠テンプレート (プロットフレーム、M4 §9) のid。全シートの図枠・表題欄・改訂欄が
   * この定義で描かれる。未指定・未知idは既定の`jis-a3`。
   */
  frame_template?: string | null;
  /** プロジェクト番号 (G16)。表題欄バインド`{project.no}`の供給元。未設定はnull。 */
  no?: string | null;
}

/** PLC I/O割付表の1行 = I/O点1つ (Rustの`PlcAssignment`)。 */
export interface PlcAssignment {
  id: string;
  /** PLCモジュールの参照記号 (例 "PLC1")。 */
  module_ref: string;
  /** I/Oアドレス (例 "X0" / "%I0.0" / "I:0/0")。 */
  address: string;
  signal_name: string;
  comment: string;
}

export type Command =
  | {
      type: "add_sheet";
      name: string;
      size: PaperSize;
      orientation: Orientation;
      /** シート種別 (省略時は回路図)。 */
      kind?: SheetKind;
    }
  | { type: "remove_sheet"; sheet_id: string }
  | { type: "rename_sheet"; sheet_id: string; name: string }
  | { type: "set_title_block"; sheet_id: string; title_block: TitleBlock }
  | { type: "set_revisions"; sheet_id: string; revisions: Revision[] }
  /** シートの注記文字高 (mm) の設定。nullで既定 (2.0mm) へ戻す。範囲は1.0〜3.5mm。 */
  | { type: "set_sheet_annotation_text"; sheet_id: string; height_mm: number | null }
  /**
   * シートの用紙サイズ・向きの変更 (G15)。既存エンティティの座標はそのまま
   * (動かない・消えない)。新しい用紙の外に出たエンティティはERC警告
   * (`erc.outside_paper`) で報告するだけで、動かしたり消したりしない。
   */
  | { type: "set_sheet_paper"; sheet_id: string; size: PaperSize; orientation: Orientation }
  | { type: "add_entity"; sheet_id: string; entity: Entity }
  | { type: "update_entity"; sheet_id: string; entity: Entity }
  | { type: "delete_entities"; sheet_id: string; ids: string[] }
  | { type: "move_entities"; sheet_id: string; ids: string[]; dx: number; dy: number }
  | { type: "set_wire_parts"; wire_parts: WirePart[] }
  /** PLC I/O割付表の置換 (プロジェクト単位)。 */
  | { type: "set_plc_assignments"; assignments: PlcAssignment[] }
  | { type: "set_frame_template"; frame_template: string | null }
  /** 線番のネット単位自動採番。sheet_id省略(null)で全シート。 */
  | {
      type: "renumber_wires";
      sheet_id?: string | null;
      mode: "append" | "renumber";
      start: number;
    }
  /** 線番の直接指定(個別編集)。numberがnullなら線番を消す。 */
  | {
      type: "set_wire_numbers";
      sheet_id: string;
      numbers: { wire_id: string; number: string | null }[];
    }
  /** 盤レイアウトシートのアイテム番号(バルーン)の自動採番。読み順に 1..N を振る。 */
  | { type: "renumber_panel_items"; sheet_id: string }
  /** アイテム番号の直接指定(自動採番の逆コマンド)。item_noがnullなら番号を消す。 */
  | {
      type: "set_panel_item_numbers";
      sheet_id: string;
      numbers: { footprint_id: string; item_no: number | null }[];
    }
  /**
   * プロジェクト名・番号と、全シート共通の表題欄の供給元となる項目の一括設定 (G16)。
   * 各欄は省略(undefined)すれば変更しない。`name`/`no`はプロジェクト単位、
   * `company`/`drawing_no`/`title`は**全シートの表題欄**へ同じ値を書く(undo一発)。
   * `no`だけは3状態で、`null`を送ると番号を**未設定へ戻す**(空文字にはしない)。
   */
  | {
      type: "set_project_meta";
      name?: string;
      no?: string | null;
      company?: string | null;
      drawing_no?: string | null;
      title?: string | null;
    };

export type PatchOp =
  | { op: "project_replaced"; project: Project }
  | { op: "sheet_added"; index: number; sheet: Sheet }
  | { op: "sheet_removed"; sheet_id: string }
  | { op: "sheet_meta_updated"; sheet: Sheet }
  | { op: "entity_upserted"; sheet_id: string; entity: Entity }
  | { op: "entity_removed"; sheet_id: string; id: string }
  | { op: "wire_parts_replaced"; wire_parts: WirePart[] }
  | { op: "plc_assignments_replaced"; assignments: PlcAssignment[] }
  | { op: "frame_template_changed"; frame_template: string | null }
  /** プロジェクト名・番号が変わった (G16)。表題欄側は各シートの`sheet_meta_updated`で届く。 */
  | { op: "project_meta_changed"; name: string; no: string | null };

export interface Patch {
  revision: number;
  ops: PatchOp[];
}

export interface ProjectSnapshot {
  revision: number;
  project: Project;
  can_undo: boolean;
  can_redo: boolean;
}

/** ピンの接続方向 (シンボルの外側へ電線が出る向き。回転0度基準)。 */
export type PinDir = "up" | "down" | "left" | "right";

export interface PinDef {
  number: string;
  name: string;
  at: Point;
  dir?: PinDir;
}

/** 属性テキストの流し込み位置 (参照記号・型番・説明・定格)。 */
export interface TextSlot {
  key: string;
  at: Point;
  height: number;
}

export type Primitive =
  | { type: "line"; pts: Point[] }
  | { type: "circle"; center: Point; r: number; filled: boolean }
  | { type: "arc"; center: Point; r: number; start_deg: number; end_deg: number }
  | { type: "rect"; p1: Point; p2: Point; filled: boolean }
  | { type: "text"; at: Point; text: string; height: number };

export interface SymbolDef {
  id: string;
  name: string;
  name_ja: string;
  category: string;
  ref_prefix: string;
  /** 検索語 (英語+日本語+略称)。名称に無い呼び方でも部品挿入ダイアログで探せる。 */
  keywords?: string[];
  primitives: Primitive[];
  pins: PinDef[];
  text_slots?: TextSlot[];
}

/** ユーザーシンボルの分類 (部品挿入ダイアログの「ユーザー」グループ)。 */
export const USER_SYMBOL_CATEGORY = "user";

/** 保存できない理由1件 (madake-coreの`SymbolError`)。`code`は`symbol.pin_off_grid`等。 */
export interface SymbolError {
  code: string;
  message: string;
}

/** 読み込めなかったユーザーシンボル1件 (壊れたJSON・検証に落ちた・id重複)。 */
export interface SymbolIssue {
  path: string;
  message: string;
}

/**
 * ユーザーシンボルの一覧と、読み込めなかったファイルの理由 (madake-coreの`SymbolLibrary`)。
 *
 * ライブラリは**ファイルのドメイン**でCommandエンジンの外にあり、保存・削除は
 * undo履歴に乗らない。図面はシンボル定義のスナップショットを持たず`symbol_id`で
 * 参照するだけなので、定義を保存し直すとすでに置いてある部品も描き直される。
 */
export interface SymbolLibrary {
  symbols: SymbolDef[];
  issues: SymbolIssue[];
  /** ユーザーシンボルの置き場 (`~/MadakeCAD/symbols`)。 */
  user_dir?: string | null;
}

/** 部品DB (グローバル共有マスタ) の1部品。 */
export interface Part {
  part_no: string;
  maker: string;
  name: string;
  category: string;
  symbol_id: string;
  rated_voltage: string;
  rated_current_a: number | null;
  purchase_url: string;
  datasheet_url: string;
  price: number | null;
  currency: string;
  note: string;
  model_3d: string;
  mounting: string;
  /** 接点構成 (リレー・コンタクタの実装数。例 "2NO+2NC")。配置時にattrsへ写る。 */
  contact_config?: string;
  /** PLC I/Oモジュールの定義JSON (空欄ならPLCモジュールではない)。 */
  plc_module?: string;
}

/** KiCadインポートの結果要約。 */
export interface KicadImportResult {
  patch: Patch;
  report: {
    symbols: number;
    wires: number;
    junctions: number;
    labels: number;
    texts: number;
    skipped: string[];
    warnings: string[];
  };
}

/** DC動作点シミュレーションの結果。 */
export interface SimOpResult {
  voltage: number;
  nets: { name: string; volts_min: number; volts_max: number; wire_ids: string[] }[];
  components: { reference: string; entity_id: string; amps: number; watts: number }[];
  warnings: string[];
}

export interface Diagnostic {
  severity: "error" | "warning" | "info";
  code: string;
  message: string;
  sheet_id: string;
  entity_ids: string[];
}

export interface NetPin {
  reference: string;
  entity_id: string;
  pin: string;
}

export interface Net {
  /** 表示名。優先順は ネットラベル > 線番 > 自動名 (N001…)。 */
  name: string;
  pins: NetPin[];
  wire_ids: string[];
  /** ネットラベル由来の名前(手動指定)。 */
  label?: string | null;
  /** 線番。ネット内の全Wireで同値。 */
  wire_no?: string | null;
}

/** 端子台チャートの1行 = 端子1個 (madake-coreの`TerminalRow`)。 */
export interface TerminalRow {
  /** 端子番号 (例 "1"。多段端子台では段記号付きで "1a" のようになる)。 */
  terminal: string;
  /** 段記号 (例 "a")。多段端子台 (G6) の端子だけが持ち、単段の端子台では空文字。 */
  level: string;
  /** 内部側 (盤内) の接続先 "参照記号:ピン番号"。 */
  internal: string;
  /** 外部側 (盤外) の接続先。 */
  external: string;
  wire_no: string;
  /** 電線の仕様 (線色・sq・品番)。 */
  wire: string;
  internal_wire: string;
  external_wire: string;
  internal_harness: string;
  external_harness: string;
  /** この端子に掛かっているジャンパ (例 "1-2")。 */
  jumper: string;
  /** 予備端子 (内部側・外部側とも未結線)。 */
  spare: boolean;
}

/** 読み取れなかったジャンパ指定1件。 */
export interface JumperIssue {
  text: string;
  code: string;
  message: string;
}

/** 端子台1つのチャート (madake-coreの`TerminalChart`)。 */
export interface TerminalChart {
  entity_id: string;
  reference: string;
  value: string;
  sheet_name: string;
  /** 端子数 (`rows`の行数)。多段端子台では極数×段数。極数は`TerminalBlockInfo.poles`を見ること。 */
  terminal_count: number;
  rows: TerminalRow[];
  /** 正規化済みのジャンパ (並び順で先に来る端子番号が先)。多段端子台では段記号付き ("1a")。 */
  jumpers: [string, string][];
  jumper_issues: JumperIssue[];
}

/** 端子台1つの概要 (エディタの切替ドロップダウン・帳票の対象選択)。 */
export interface TerminalBlockInfo {
  entity_id: string;
  sheet_id: string;
  sheet_name: string;
  reference: string;
  value: string;
  terminal_count: number;
  /** 極数 (多段端子台でも段数は掛けない)。 */
  poles: number;
  /** 段数 (単段は1、多段端子台は2以上)。 */
  levels: number;
  /** ジャンパ指定の生の値 (`attrs["jumpers"]`)。 */
  jumpers: string;
}

/** 検索の対象種別 (madake-coreの`SearchKind`)。 */
export type SearchKind = "reference" | "value" | "net" | "wire_no" | "text";

/** デバイス (参照記号1つ) の種別。 */
export type DeviceKind = "relay" | "terminal_block" | "other";

/** デバイスを構成する機能1つの種別。 */
export type DeviceFunctionKind =
  | "coil"
  | "contact_no"
  | "contact_nc"
  | "contact_co"
  | "terminal"
  | "body";

/** 検索で見つかった1件 (madake-coreの`SearchHit`)。 */
export interface SearchHit {
  kind: SearchKind;
  /** マッチした文字列そのもの (結果パネルの先頭列)。 */
  text: string;
  /** 補足 (型番のヒットなら参照記号、参照記号のヒットなら型番)。 */
  detail: string;
  /** シンボルのヒットのとき、そのシンボルがデバイスで果たす機能。 */
  function: DeviceFunctionKind | null;
  /** 機能の端子の呼び名 (例 "13-14")。 */
  terminals: string;
  sheet_id: string;
  /** 1始まりのシート表示順。 */
  sheet_no: number;
  sheet_name: string;
  /** ゾーンアドレス (例 "B3")。 */
  zone: string;
  /** クリックしたときに選択+ズームするエンティティ。 */
  entity_id: string;
}

/** デバイスの機能1つ (コイル・接点・端子群・本体) と、その図面上の所在。 */
export interface DeviceFunction {
  kind: DeviceFunctionKind;
  /** 端子の呼び名 (コイル "A1-A2" / 接点 "13-14" / 端子台 "1-8" / 本体は空)。 */
  terminals: string;
  entity_id: string;
  sheet_id: string;
  sheet_no: number;
  sheet_name: string;
  zone: string;
}

/** 参照記号1つ分のデバイス (デバイスナビゲータのツリーの1ノード)。 */
export interface DeviceNode {
  reference: string;
  kind: DeviceKind;
  /** 型番・値。 */
  value: string;
  symbol_id: string;
  symbol_name: string;
  symbol_name_ja: string;
  /** 端子台の極数 (端子台以外は0)。多段端子台でも段数は掛けない。 */
  poles: number;
  /** 端子台の段数 (単段は1、多段端子台は2以上。端子台以外は0)。 */
  levels: number;
  functions: DeviceFunction[];
}

/**
 * 開始テンプレート1件 (madake-coreの`Template`)。
 *
 * `commands`のUUIDはプレースホルダで、適用時にRust側が差し替える
 * (nil UUID=適用先シート、それ以外=新しいid)。ダイアログのプレビューはこの
 * コマンド列をそのまま描く。
 */
export interface Template {
  id: string;
  /** 英語名 (UI言語=enで表示)。 */
  name: string;
  name_ja: string;
  description: string;
  description_ja: string;
  commands: Command[];
  /** 同梱テンプレートならtrue、ユーザーの`~/MadakeCAD/templates`由来ならfalse。 */
  builtin: boolean;
}

/** 読み込めなかったテンプレートファイル1件。 */
export interface TemplateIssue {
  path: string;
  message: string;
}

/** テンプレート一覧と、読み込めなかったファイルの理由。 */
export interface TemplateList {
  templates: Template[];
  issues: TemplateIssue[];
  /** ユーザーテンプレートの置き場 (「ここへJSONを置けば並ぶ」の案内に使う)。 */
  user_dir?: string | null;
}

// 図枠テンプレート (プロットフレーム、M4 §9) の型はレイアウト計算と同居させている
// (型のみの参照なので実行時の循環は生じない)。
import type { FrameTemplate } from "./canvas/frameTemplate";
export type { FrameTemplate };

/** 図枠テンプレート一覧と、読み込めなかったファイルの理由 (madake-coreの`FrameTemplateList`)。 */
export interface FrameTemplateList {
  templates: FrameTemplate[];
  issues: TemplateIssue[];
  /** ユーザー図枠の置き場 (`~/MadakeCAD/templates/frames`)。 */
  user_dir?: string | null;
}

/**
 * 回路マクロ1件 (madake-coreの`Macro`)。
 *
 * テンプレートと同じ「Command列のJSON」に、挿入時にカーソルへ来る`base_point`と
 * 代替形の`variants`を足したもの。座標は`base_point`からの**相対**で入っている。
 */
export interface Macro {
  id: string;
  name: string;
  name_ja: string;
  description: string;
  description_ja: string;
  /** 挿入ダイアログの左ツリーで束ねる分類 (空=分類なし)。 */
  category: string;
  base_point: Point;
  /** 既定バリアント("A")のCommand列。 */
  commands: Command[];
  variants: MacroVariant[];
  /** 可変値のスロット (定格・型番など。値セットが値を入れる先)。 */
  placeholders: MacroPlaceholder[];
  /** 値セット (挿入時に選ぶと関連する欄が一括設定される)。 */
  value_sets: MacroValueSet[];
}

/**
 * マクロのプレースホルダ1件 = 「この回路のこの値は現場で決める」というスロット。
 * `key`が値セットとの合言葉で、`targets`がその値の行き先。
 */
export interface MacroPlaceholder {
  /** 値セットから参照される名前 (例: "rating")。 */
  key: string;
  label: string;
  label_ja: string;
  targets: PlaceholderTarget[];
}

/** プレースホルダの値の行き先1つ (どのシンボルのどの欄へ書くか)。 */
export interface PlaceholderTarget {
  /** マクロ内のエンティティid (挿入時に新しいidへ振り直される)。 */
  entity: string;
  /** `"value"` (型番・値) または `"attrs.<名前>"`。 */
  field: string;
}

/** 値セット1件 = プレースホルダの値の組 (例: 「1.5kW」を選ぶと決まる値ぜんぶ)。 */
export interface MacroValueSet {
  /** 挿入時に指定するid (例: "1_5_kw")。 */
  id: string;
  label: string;
  label_ja: string;
  /** プレースホルダの`key` → 値。 */
  values: Record<string, string>;
}

/** マクロの代替バリアント1件 (EPLANのマクロバリアントA〜Hに相当)。 */
export interface MacroVariant {
  /** バリアントキー (例 "B")。 */
  key: string;
  name: string;
  name_ja: string;
  commands: Command[];
}

/** マクロ保存時に付ける情報 (idを空にすると名前から自動生成される)。 */
export interface MacroMeta {
  id?: string;
  name: string;
  name_ja?: string;
  description?: string;
  description_ja?: string;
  category?: string;
  /** 可変値のスロット (指定しなければプレースホルダ無しのマクロになる)。 */
  placeholders?: MacroPlaceholder[];
  /** 値セット (指定しなければ値セット無しのマクロになる)。 */
  value_sets?: MacroValueSet[];
}

/** 読み込めなかったマクロファイル1件 (形はテンプレートと同じ)。 */
export type MacroIssue = TemplateIssue;

/** マクロ一覧と、読み込めなかったファイルの理由。 */
export interface MacroList {
  macros: Macro[];
  issues: MacroIssue[];
  /** ユーザーマクロの置き場 (`~/MadakeCAD/macros`)。 */
  user_dir?: string | null;
}

/** PLCモジュールの入出力の種別。 */
export type PlcIoKind = "DI" | "DO";
/** PLCのアドレス体系 (三菱=8進 / Siemens=バイト.ビット / AB=ワード/ビット)。 */
export type PlcAddressStyle = "mitsubishi" | "siemens" | "ab";

/** PLCモジュール1機種の定義 (部品DBの`plc_module`列と同じ形)。 */
export interface PlcModuleSpec {
  points: number;
  kind: PlcIoKind;
  address_prefix: string;
  address_style: PlcAddressStyle;
}

/** 図面に置かれているPLCモジュール1つの概要。 */
export interface PlcModuleInfo {
  entity_id: string;
  sheet_id: string;
  sheet_name: string;
  reference: string;
  value: string;
  points: number;
  kind: PlcIoKind;
}

/** 割付表エディタのグリッド1行 (接続先・線番は図面から読んだ読み取り専用の値)。 */
export interface PlcPoint {
  /** 1起点の点番号 (モジュールシンボルのピン番号)。 */
  point: number;
  address: string;
  signal_name: string;
  comment: string;
  target: string;
  wire_no: string;
}

/** ラダーの形式 (v1は縦バス+横ラングのみ実装)。 */
export type LadderStyle = "vertical-bus" | "horizontal-bus";
/** モジュールの配置方針 (v1はモジュールごとに新ラダーのみ実装)。 */
export type ModulePlacement = "new-ladder" | "share-if-fits" | "share-or-split";

/** I/O図面の生成設定。 */
export interface PlcSheetOptions {
  rung_spacing_mm: number;
  start_skip: number;
  ladder_style: LadderStyle;
  placement: ModulePlacement;
}

/** 帳票の種類 (Rustの`ReportKind`と同じケバブケース表記)。 */
export type ReportKind =
  | "wire-list"
  | "terminal-chart"
  | "terminal-diagram"
  | "bom"
  | "xref"
  | "plc-io"
  | "panel-placement";
/** 帳票の出力形式 (CSV or 図枠付き図面シートのPDF)。 */
export type ReportFormat = "csv" | "pdf";

interface Ipc {
  getProject(): Promise<ProjectSnapshot>;
  /** 部品挿入に並ぶ全シンボル (同梱+ユーザーの`~/MadakeCAD/symbols`。ユーザーが後ろ)。 */
  listSymbols(): Promise<SymbolDef[]>;
  /** ユーザーシンボルだけの一覧 (置き場・読めなかったファイルの理由つき)。 */
  listUserSymbols(): Promise<SymbolLibrary>;
  /**
   * ユーザーシンボルを保存する。検証に落ちたら**何も書かず**理由を全部投げる
   * (`SymbolError[]`)。undo履歴には乗らない。
   */
  saveUserSymbol(symbol: SymbolDef): Promise<{ symbol: SymbolDef; path: string }>;
  /** ユーザーシンボルを削除する。使用中は`force`なしでは消さない (undoで戻らない)。 */
  deleteUserSymbol(id: string, force: boolean): Promise<{ path: string; in_use: number }>;
  /** 既存シンボルを複製した**下書き**を返す (ファイルにはまだ書かない)。 */
  duplicateSymbol(sourceId: string): Promise<SymbolDef>;
  /** ユーザーシンボルの置き場をOSのファイラで開く (無ければ作る)。戻り値=そのパス。 */
  openSymbolsFolder(): Promise<string>;
  executeCommand(command: Command): Promise<Patch>;
  undo(): Promise<Patch | null>;
  redo(): Promise<Patch | null>;
  saveProject(path: string): Promise<void>;
  loadProject(path: string): Promise<Patch>;
  importKicad(path: string): Promise<KicadImportResult>;
  newProject(name: string): Promise<Patch>;
  /** 使える開始テンプレートの一覧 (同梱+ユーザーの`~/MadakeCAD/templates`)。 */
  listTemplates(): Promise<TemplateList>;
  /** テンプレートをシートへ適用する。1回の編集なのでundo一発で戻る。 */
  applyTemplate(templateId: string, sheetId: string): Promise<Patch>;
  /** ユーザーテンプレートの置き場をOSのファイラで開く (無ければ作る)。戻り値=そのパス。 */
  openTemplatesFolder(): Promise<string>;
  /** 使える図枠テンプレートの一覧 (同梱+ユーザーの`~/MadakeCAD/templates/frames`)。 */
  listFrameTemplates(): Promise<FrameTemplateList>;
  /** ユーザー図枠の置き場をOSのファイラで開く (無ければ作る)。戻り値=そのパス。 */
  openFrameTemplatesFolder(): Promise<string>;
  /** 使える回路マクロの一覧 (ユーザーの`~/MadakeCAD/macros`)。 */
  listMacros(): Promise<MacroList>;
  /** 選択範囲をマクロへ組み立てるだけ (ファイルに書かない)。プレビューと⌘Cが使う。 */
  buildMacro(sheetId: string, entityIds: string[], meta: MacroMeta): Promise<Macro>;
  /** 選択範囲をマクロとしてユーザー領域へ保存する。図面は変わらない。 */
  saveMacro(
    sheetId: string,
    entityIds: string[],
    meta: MacroMeta,
  ): Promise<{ macro: Macro; path: string }>;
  /**
   * ライブラリのマクロをidで挿入する。1回の編集なのでundo一発で戻る。
   * `valueSet`を渡すと、その値セットの値がプレースホルダの行き先へ一括で入る。
   */
  applyMacro(
    macroId: string,
    variant: string | null,
    valueSet: string | null,
    sheetId: string,
    at: Point,
    rotation: number,
  ): Promise<Patch>;
  /** マクロそのものを挿入する (⌘C/Vの無名マクロ。ライブラリのidを引かない)。 */
  applyMacroInline(
    macro: Macro,
    variant: string | null,
    valueSet: string | null,
    sheetId: string,
    at: Point,
    rotation: number,
  ): Promise<Patch>;
  /** ユーザーマクロの置き場をOSのファイラで開く (無ければ作る)。戻り値=そのパス。 */
  openMacrosFolder(): Promise<string>;
  getNetlist(sheetId: string): Promise<Net[]>;
  /** 図面検証 (ERC+電気検証)。sheetId=nullで全シート。 */
  verify(sheetId: string | null): Promise<Diagnostic[]>;
  /** 部品DB検索 (query: 部分一致、category: 完全一致)。 */
  searchParts(query: string, category?: string): Promise<Part[]>;
  /** DC動作点シミュレーション。openSwitchesの参照記号は開路扱い。 */
  simulateOp(sheetId: string | null, openSwitches: string[]): Promise<SimOpResult>;
  exportSvg(sheetId: string, path: string): Promise<void>;
  exportPdf(sheetId: string, path: string): Promise<void>;
  exportBom(path: string): Promise<void>;
  exportWireList(path: string): Promise<void>;
  /**
   * プロジェクト内検索 (⌘F)。kindsが空なら全種別を対象にする。
   * 結果は図面の読み順 (シート→ゾーン→id) で返る。
   */
  searchProject(query: string, kinds: SearchKind[]): Promise<SearchHit[]>;
  /** デバイスナビゲータのツリー (参照記号 → 機能)。 */
  getDeviceTree(): Promise<DeviceNode[]>;
  /**
   * 未配置部品 (回路図にあって、どの盤レイアウトシートにもフットプリントが無い部品)。
   * 外形寸法は部品DBから引き、登録の無い型番は既定の矩形になる (M4仕様 §8)。
   */
  unplacedComponents(): Promise<UnplacedComponent[]>;
  /** シート内 (nullでプロジェクト全体) の端子台一覧。 */
  listTerminalBlocks(sheetId: string | null): Promise<TerminalBlockInfo[]>;
  /** 端子台1つのチャート。端子台でなければnull。 */
  getTerminalChart(entityId: string): Promise<TerminalChart | null>;
  /** 端子台チェック (未結線・不正なジャンパ)。 */
  checkTerminalBlock(entityId: string): Promise<Diagnostic[]>;
  /** 図面に置かれているPLC I/Oモジュールの一覧。 */
  listPlcModules(): Promise<PlcModuleInfo[]>;
  /** 部品DBのPLCモジュールライブラリ (機種一覧)。 */
  listPlcModuleParts(): Promise<Part[]>;
  /** 1つのモジュールのI/O割付表 (接続先・線番は図面から導出)。 */
  getPlcAssignments(moduleRef: string): Promise<PlcPoint[]>;
  /** I/O割付表を丸ごと置き換える (undo一発で戻る)。 */
  setPlcAssignments(assignments: PlcAssignment[]): Promise<Patch>;
  /** 割付表のCSVを取り込む (対象モジュールの行だけ置き換え)。 */
  importPlcAssignmentsCsv(moduleRef: string, csv: string): Promise<Patch>;
  /**
   * 電線品番表 (線色+線径→品番) のCSVを取り込む。表全体を置き換え、undo一発で戻る。
   * 列は 線色・線径sq・電線品番 (+備考)。読めないCSVは理由つきで失敗し、表は変わらない。
   */
  importWirePartsCsv(csv: string): Promise<Patch>;
  /** PLC I/O図面 (ラダーページ) を生成する (1回の編集 = undo一発)。 */
  generatePlcSheet(
    moduleRef: string,
    module: PlcModuleSpec,
    options?: PlcSheetOptions,
  ): Promise<Patch>;
  /**
   * 帳票1種を1ファイルへ書き出す。entityIdは端子台チャート・端子接続図の対象を
   * 1つの端子台に絞るときだけ渡す。戻り値=CSVなら行数、PDFならページ数。
   */
  exportReport(
    kind: ReportKind,
    format: ReportFormat,
    entityId: string | null,
    path: string,
  ): Promise<number>;
  /** 表紙+回路図全シート+選択帳票を1つのPDFへ。戻り値=ページ数。 */
  exportPdfBook(path: string, reports: ReportKind[], cover: boolean): Promise<number>;
  onPatch(handler: (patch: Patch) => void): Promise<UnlistenFn>;
}

const tauriIpc: Ipc = {
  getProject: () => invoke<ProjectSnapshot>("get_project"),
  listSymbols: () => invoke<SymbolDef[]>("list_symbols"),
  listUserSymbols: () => invoke<SymbolLibrary>("list_user_symbols"),
  saveUserSymbol: (symbol: SymbolDef) =>
    invoke<{ symbol: SymbolDef; path: string }>("save_user_symbol", { symbol }),
  deleteUserSymbol: (id: string, force: boolean) =>
    invoke<{ path: string; in_use: number }>("delete_user_symbol", { id, force }),
  duplicateSymbol: (sourceId: string) => invoke<SymbolDef>("duplicate_symbol", { sourceId }),
  openSymbolsFolder: () => invoke<string>("open_symbols_folder"),
  executeCommand: (command: Command) => invoke<Patch>("execute_command", { command }),
  undo: () => invoke<Patch | null>("undo"),
  redo: () => invoke<Patch | null>("redo"),
  saveProject: (path: string) => invoke<void>("save_project", { path }),
  loadProject: (path: string) => invoke<Patch>("load_project", { path }),
  importKicad: (path: string) => invoke<KicadImportResult>("import_kicad", { path }),
  newProject: (name: string) => invoke<Patch>("new_project", { name }),
  listTemplates: () => invoke<TemplateList>("list_templates"),
  applyTemplate: (templateId: string, sheetId: string) =>
    invoke<Patch>("apply_template", { templateId, sheetId }),
  openTemplatesFolder: () => invoke<string>("open_templates_folder"),
  listFrameTemplates: () => invoke<FrameTemplateList>("list_frame_templates"),
  openFrameTemplatesFolder: () => invoke<string>("open_frame_templates_folder"),
  listMacros: () => invoke<MacroList>("list_macros"),
  buildMacro: (sheetId, entityIds, meta) =>
    invoke<Macro>("build_macro", { sheetId, entityIds, meta }),
  saveMacro: (sheetId, entityIds, meta) =>
    invoke<{ macro: Macro; path: string }>("save_macro", { sheetId, entityIds, meta }),
  applyMacro: (macroId, variant, valueSet, sheetId, at, rotation) =>
    invoke<Patch>("apply_macro", { macroId, variant, valueSet, sheetId, at, rotation }),
  applyMacroInline: (macro, variant, valueSet, sheetId, at, rotation) =>
    invoke<Patch>("apply_macro_inline", { macro, variant, valueSet, sheetId, at, rotation }),
  openMacrosFolder: () => invoke<string>("open_macros_folder"),
  getNetlist: (sheetId: string) => invoke<Net[]>("get_netlist", { sheetId }),
  verify: (sheetId: string | null) => invoke<Diagnostic[]>("run_verification", { sheetId }),
  searchParts: (query: string, category?: string) =>
    invoke<Part[]>("search_parts", { query, category }),
  simulateOp: (sheetId: string | null, openSwitches: string[]) =>
    invoke<SimOpResult>("simulate_op", { sheetId, openSwitches }),
  exportSvg: (sheetId: string, path: string) => invoke<void>("export_svg", { sheetId, path }),
  exportPdf: (sheetId: string, path: string) => invoke<void>("export_pdf", { sheetId, path }),
  exportBom: (path: string) => invoke<void>("export_bom", { path }),
  exportWireList: (path: string) => invoke<void>("export_wire_list", { path }),
  searchProject: (query: string, kinds: SearchKind[]) =>
    invoke<SearchHit[]>("search_project", { query, kinds }),
  getDeviceTree: () => invoke<DeviceNode[]>("get_device_tree"),
  unplacedComponents: () => invoke<UnplacedComponent[]>("unplaced_components"),
  listTerminalBlocks: (sheetId: string | null) =>
    invoke<TerminalBlockInfo[]>("list_terminal_blocks", { sheetId }),
  getTerminalChart: (entityId: string) =>
    invoke<TerminalChart | null>("get_terminal_chart", { entityId }),
  checkTerminalBlock: (entityId: string) =>
    invoke<Diagnostic[]>("check_terminal_block", { entityId }),
  listPlcModules: () => invoke<PlcModuleInfo[]>("list_plc_modules"),
  listPlcModuleParts: () => invoke<Part[]>("list_plc_module_parts"),
  getPlcAssignments: (moduleRef: string) =>
    invoke<PlcPoint[]>("get_plc_assignments", { moduleRef }),
  setPlcAssignments: (assignments: PlcAssignment[]) =>
    invoke<Patch>("set_plc_assignments", { assignments }),
  importPlcAssignmentsCsv: (moduleRef: string, csv: string) =>
    invoke<Patch>("import_plc_assignments_csv", { moduleRef, csv }),
  importWirePartsCsv: (csv: string) => invoke<Patch>("import_wire_parts_csv", { csv }),
  generatePlcSheet: (moduleRef, module, options) =>
    invoke<Patch>("generate_plc_sheet", { moduleRef, module, options }),
  exportReport: (kind, format, entityId, path) =>
    invoke<number>("export_report", { kind, format, entityId, path }),
  exportPdfBook: (path, reports, cover) =>
    invoke<number>("export_pdf_book", { path, includeReports: reports, cover }),
  onPatch: (handler: (patch: Patch) => void): Promise<UnlistenFn> =>
    listen<Patch>("doc:patch", (e) => handler(e.payload)),
};

// Tauri外(ブラウザでのUI開発・E2E検証)では、起動中のMadakeCADのLink APIに接続する。
// **接続先のホスト名はページと同じものを使う**: `localhost:1420` で開いたページから
// `127.0.0.1:9310` へ繋ぐと、ブラウザによってはプライベートネットワーク扱いで
// 応答が返ってこないため。ページのホスト名が取れない場合だけ 127.0.0.1 へ倒す。
export const LINK_API_PORT = 9310;
export const linkApiBase = (): string => {
  const host =
    typeof window !== "undefined" && window.location?.hostname
      ? window.location.hostname
      : "127.0.0.1";
  return `http://${host}:${LINK_API_PORT}/api/v1`;
};
const API_BASE = linkApiBase();

async function http<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, {
    headers: { "Content-Type": "application/json" },
    ...init,
  });
  if (!res.ok) throw new Error(`Link API ${res.status}: ${await res.text()}`);
  return (await res.json()) as T;
}

const httpIpc: Ipc = {
  getProject: () => http<ProjectSnapshot>("/project"),
  listSymbols: () => http<SymbolDef[]>("/symbols"),
  listUserSymbols: () => http<SymbolLibrary>("/symbols/user"),
  saveUserSymbol: async (symbol) => {
    // 検証に落ちたときは`{ errors: SymbolError[] }`が400で返る (理由を全部拾って投げる)
    const res = await fetch(`${API_BASE}/symbols/user/save`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(symbol),
    });
    const body = await res.json().catch(() => null);
    if (!res.ok) {
      const errors = (body?.errors ?? []) as SymbolError[];
      throw new Error(errors.map((e) => e.message).join(" / ") || `Link API ${res.status}`);
    }
    return body as { symbol: SymbolDef; path: string };
  },
  deleteUserSymbol: (id, force) =>
    http<{ path: string; in_use: number }>("/symbols/user/delete", {
      method: "POST",
      body: JSON.stringify({ id, force }),
    }),
  duplicateSymbol: (sourceId) =>
    http<SymbolDef>("/symbols/duplicate", {
      method: "POST",
      body: JSON.stringify({ source_id: sourceId }),
    }),
  openSymbolsFolder: () => Promise.reject(new Error("browser mode: not supported")),
  executeCommand: async (command) => {
    const patches = await http<Patch[]>("/commands", {
      method: "POST",
      body: JSON.stringify([command]),
    });
    return patches[0];
  },
  undo: () => http<Patch | null>("/undo", { method: "POST", body: "{}" }),
  redo: () => http<Patch | null>("/redo", { method: "POST", body: "{}" }),
  saveProject: async (path) => {
    await http("/save", { method: "POST", body: JSON.stringify({ path }) });
  },
  loadProject: (path) => http<Patch>("/load", { method: "POST", body: JSON.stringify({ path }) }),
  importKicad: (path) =>
    http<KicadImportResult>("/import/kicad", { method: "POST", body: JSON.stringify({ path }) }),
  newProject: () => Promise.reject(new Error("browser mode: not supported")),
  listTemplates: () => http<TemplateList>("/templates"),
  applyTemplate: (templateId, sheetId) =>
    http<Patch>("/templates/apply", {
      method: "POST",
      body: JSON.stringify({ template_id: templateId, sheet_id: sheetId }),
    }),
  // ブラウザ検証モードではOSのファイラを開けない (呼び出し側がパスを案内する)
  openTemplatesFolder: () => Promise.reject(new Error("browser mode: not supported")),
  listFrameTemplates: () => http<FrameTemplateList>("/frame-templates"),
  openFrameTemplatesFolder: () => Promise.reject(new Error("browser mode: not supported")),
  listMacros: () => http<MacroList>("/macros"),
  buildMacro: (sheetId, entityIds, meta) =>
    http<Macro>("/macros/build", {
      method: "POST",
      body: JSON.stringify({ sheet_id: sheetId, entity_ids: entityIds, ...meta }),
    }),
  saveMacro: (sheetId, entityIds, meta) =>
    http<{ macro: Macro; path: string }>("/macros/save", {
      method: "POST",
      body: JSON.stringify({ sheet_id: sheetId, entity_ids: entityIds, ...meta }),
    }),
  applyMacro: (macroId, variant, valueSet, sheetId, at, rotation) =>
    http<Patch>("/macros/apply", {
      method: "POST",
      body: JSON.stringify({
        id: macroId,
        variant,
        value_set: valueSet,
        sheet_id: sheetId,
        at,
        rotation,
      }),
    }),
  applyMacroInline: (macro, variant, valueSet, sheetId, at, rotation) =>
    http<Patch>("/macros/apply-inline", {
      method: "POST",
      body: JSON.stringify({ macro, variant, value_set: valueSet, sheet_id: sheetId, at, rotation }),
    }),
  openMacrosFolder: () => Promise.reject(new Error("browser mode: not supported")),
  getNetlist: (sheetId) => http<Net[]>(`/netlist?sheet_id=${sheetId}`),
  verify: (sheetId) => http<Diagnostic[]>(sheetId ? `/verify?sheet_id=${sheetId}` : "/verify"),
  simulateOp: (sheetId, openSwitches) =>
    http<SimOpResult>("/simulate/op", {
      method: "POST",
      body: JSON.stringify({ sheet_id: sheetId, open_switches: openSwitches }),
    }),
  searchParts: (query, category) => {
    const params = new URLSearchParams();
    if (query) params.set("query", query);
    if (category) params.set("category", category);
    const qs = params.toString();
    return http<Part[]>(`/parts${qs ? `?${qs}` : ""}`);
  },
  exportSvg: async (sheetId, path) => {
    await http("/export/svg", { method: "POST", body: JSON.stringify({ sheet_id: sheetId, path }) });
  },
  exportPdf: async (sheetId, path) => {
    await http("/export/pdf", { method: "POST", body: JSON.stringify({ sheet_id: sheetId, path }) });
  },
  exportBom: async (path) => {
    await http("/export/bom", { method: "POST", body: JSON.stringify({ path }) });
  },
  exportWireList: async (path) => {
    await http("/export/wire-list", { method: "POST", body: JSON.stringify({ path }) });
  },
  searchProject: async (query, kinds) => {
    const params = new URLSearchParams({ q: query });
    if (kinds.length) params.set("kinds", kinds.join(","));
    const res = await http<{ hits: SearchHit[] }>(`/search?${params.toString()}`);
    return res.hits;
  },
  getDeviceTree: async () => (await http<{ devices: DeviceNode[] }>("/devices")).devices,
  unplacedComponents: async () =>
    (await http<{ unplaced: UnplacedComponent[] }>("/panel/unplaced")).unplaced,
  listTerminalBlocks: (sheetId) =>
    http<TerminalBlockInfo[]>(`/terminals${sheetId ? `?sheet_id=${sheetId}` : ""}`),
  getTerminalChart: async (entityId) => {
    try {
      return await http<TerminalChart>(`/terminals/chart?entity_id=${entityId}`);
    } catch {
      // 端子台ではない (400) 場合はチャート無し
      return null;
    }
  },
  checkTerminalBlock: (entityId) => http<Diagnostic[]>(`/terminals/check?entity_id=${entityId}`),
  listPlcModules: async () =>
    (await http<{ modules: PlcModuleInfo[] }>("/plc/modules")).modules,
  listPlcModuleParts: () => http<Part[]>("/parts?category=plc"),
  getPlcAssignments: async (moduleRef) => {
    const res = await http<{ modules: { module_ref: string; points: PlcPoint[] }[] }>(
      `/plc/assignments?module_ref=${encodeURIComponent(moduleRef)}`,
    );
    return res.modules[0]?.points ?? [];
  },
  setPlcAssignments: (assignments) =>
    http<Patch>("/plc/assignments", { method: "PUT", body: JSON.stringify({ assignments }) }),
  importWirePartsCsv: (csv) =>
    http<Patch>("/project/wire-parts/csv", { method: "POST", body: JSON.stringify({ csv }) }),
  importPlcAssignmentsCsv: (moduleRef, csv) =>
    http<Patch>("/plc/assignments/import", {
      method: "POST",
      body: JSON.stringify({ module_ref: moduleRef, csv }),
    }),
  generatePlcSheet: (moduleRef, module, options) =>
    http<Patch>("/plc/generate", {
      method: "POST",
      body: JSON.stringify({ module_ref: moduleRef, module, options }),
    }),
  exportReport: async (kind, format, entityId, path) => {
    const res = await http<{ count: number }>("/export/report", {
      method: "POST",
      body: JSON.stringify({ kind, format, entity_id: entityId, path }),
    });
    return res.count;
  },
  exportPdfBook: async (path, reports, cover) => {
    const res = await http<{ pages: number }>("/export/pdf-book", {
      method: "POST",
      body: JSON.stringify({ path, include_reports: reports, cover }),
    });
    return res.pages;
  },
  onPatch: (handler) => {
    const es = new EventSource(`${API_BASE}/events`);
    es.addEventListener("patch", (e) => {
      try {
        handler(JSON.parse((e as MessageEvent).data) as Patch);
      } catch {
        // 不正なイベントは無視
      }
    });
    return Promise.resolve(() => es.close());
  },
};

/** Tauri内で動作しているか。falseならLink API経由(ブラウザ検証モード)。 */
export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const ipc: Ipc = inTauri ? tauriIpc : httpIpc;
