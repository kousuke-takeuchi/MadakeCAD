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
}

export interface TextEntity {
  id: string;
  at: Point;
  text: string;
  height: number;
  rotation: number;
}

/** ハーネス境界 (IEC 61082-1 のグループ囲み)。所属配線は幾何学的な内包で決まる。 */
export interface Harness {
  id: string;
  /** 囲みの頂点 (現状は矩形の4点)。 */
  points: Point[];
  /** ハーネス名 (参照記号と同じ命名規則: W1, W2 …)。 */
  name: string;
  /** 備考 (製作指示など)。 */
  note: string;
}

export type Entity =
  | ({ kind: "symbol" } & SymbolInstance)
  | ({ kind: "wire" } & Wire)
  | ({ kind: "junction" } & Junction)
  | ({ kind: "net_label" } & NetLabel)
  | ({ kind: "text" } & TextEntity)
  | ({ kind: "harness" } & Harness);

export interface Sheet {
  id: string;
  name: string;
  size: PaperSize;
  orientation: Orientation;
  zone_cols: number;
  zone_rows: number;
  title_block: TitleBlock;
  revisions: Revision[];
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
  | { type: "add_sheet"; name: string; size: PaperSize; orientation: Orientation }
  | { type: "remove_sheet"; sheet_id: string }
  | { type: "rename_sheet"; sheet_id: string; name: string }
  | { type: "set_title_block"; sheet_id: string; title_block: TitleBlock }
  | { type: "set_revisions"; sheet_id: string; revisions: Revision[] }
  | { type: "add_entity"; sheet_id: string; entity: Entity }
  | { type: "update_entity"; sheet_id: string; entity: Entity }
  | { type: "delete_entities"; sheet_id: string; ids: string[] }
  | { type: "move_entities"; sheet_id: string; ids: string[]; dx: number; dy: number }
  | { type: "set_wire_parts"; wire_parts: WirePart[] }
  /** PLC I/O割付表の置換 (プロジェクト単位)。 */
  | { type: "set_plc_assignments"; assignments: PlcAssignment[] }
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
    };

export type PatchOp =
  | { op: "project_replaced"; project: Project }
  | { op: "sheet_added"; index: number; sheet: Sheet }
  | { op: "sheet_removed"; sheet_id: string }
  | { op: "sheet_meta_updated"; sheet: Sheet }
  | { op: "entity_upserted"; sheet_id: string; entity: Entity }
  | { op: "entity_removed"; sheet_id: string; id: string }
  | { op: "wire_parts_replaced"; wire_parts: WirePart[] }
  | { op: "plc_assignments_replaced"; assignments: PlcAssignment[] };

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
  /** 端子番号 (例 "1")。 */
  terminal: string;
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
  terminal_count: number;
  rows: TerminalRow[];
  /** 正規化済みのジャンパ (小さい端子番号が先)。 */
  jumpers: [number, number][];
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
  /** ジャンパ指定の生の値 (`attrs["jumpers"]`)。 */
  jumpers: string;
}

/** 検索の対象種別 (madake-coreの`SearchKind`)。 */
export type SearchKind = "reference" | "value" | "net" | "wire_no" | "text";

/** デバイス (参照記号1つ) の種別。 */
export type DeviceKind = "relay" | "terminal_block" | "other";

/** デバイスを構成する機能1つの種別。 */
export type DeviceFunctionKind = "coil" | "contact_no" | "contact_nc" | "terminal" | "body";

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
  /** 端子台の極数 (端子台以外は0)。 */
  poles: number;
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
  | "plc-io";
/** 帳票の出力形式 (CSV or 図枠付き図面シートのPDF)。 */
export type ReportFormat = "csv" | "pdf";

interface Ipc {
  getProject(): Promise<ProjectSnapshot>;
  listSymbols(): Promise<SymbolDef[]>;
  executeCommand(command: Command): Promise<Patch>;
  undo(): Promise<Patch | null>;
  redo(): Promise<Patch | null>;
  saveProject(path: string): Promise<void>;
  loadProject(path: string): Promise<Patch>;
  importKicad(path: string): Promise<KicadImportResult>;
  /** DXF (AutoCAD Electrical / EPLAN の中間形式) を読み込んでプロジェクトを置き換える。 */
  importDxf(path: string, wireLayers?: string[]): Promise<KicadImportResult>;
  newProject(name: string): Promise<Patch>;
  /** 使える開始テンプレートの一覧 (同梱+ユーザーの`~/MadakeCAD/templates`)。 */
  listTemplates(): Promise<TemplateList>;
  /** テンプレートをシートへ適用する。1回の編集なのでundo一発で戻る。 */
  applyTemplate(templateId: string, sheetId: string): Promise<Patch>;
  /** ユーザーテンプレートの置き場をOSのファイラで開く (無ければ作る)。戻り値=そのパス。 */
  openTemplatesFolder(): Promise<string>;
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
  /** シートをDXF (AutoCAD 2000形式) で書き出す。 */
  exportDxf(sheetId: string, path: string): Promise<void>;
  /** シートをKiCad回路図 (.kicad_sch) で書き出す。 */
  exportKicad(sheetId: string, path: string): Promise<void>;
  exportBom(path: string): Promise<void>;
  exportWireList(path: string): Promise<void>;
  /**
   * プロジェクト内検索 (⌘F)。kindsが空なら全種別を対象にする。
   * 結果は図面の読み順 (シート→ゾーン→id) で返る。
   */
  searchProject(query: string, kinds: SearchKind[]): Promise<SearchHit[]>;
  /** デバイスナビゲータのツリー (参照記号 → 機能)。 */
  getDeviceTree(): Promise<DeviceNode[]>;
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
  executeCommand: (command: Command) => invoke<Patch>("execute_command", { command }),
  undo: () => invoke<Patch | null>("undo"),
  redo: () => invoke<Patch | null>("redo"),
  saveProject: (path: string) => invoke<void>("save_project", { path }),
  loadProject: (path: string) => invoke<Patch>("load_project", { path }),
  importKicad: (path: string) => invoke<KicadImportResult>("import_kicad", { path }),
  importDxf: (path: string, wireLayers?: string[]) =>
    invoke<KicadImportResult>("import_dxf", { path, wireLayers: wireLayers ?? null }),
  newProject: (name: string) => invoke<Patch>("new_project", { name }),
  listTemplates: () => invoke<TemplateList>("list_templates"),
  applyTemplate: (templateId: string, sheetId: string) =>
    invoke<Patch>("apply_template", { templateId, sheetId }),
  openTemplatesFolder: () => invoke<string>("open_templates_folder"),
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
  exportDxf: (sheetId: string, path: string) => invoke<void>("export_dxf", { sheetId, path }),
  exportKicad: (sheetId: string, path: string) =>
    invoke<void>("export_kicad", { sheetId, path }),
  exportBom: (path: string) => invoke<void>("export_bom", { path }),
  exportWireList: (path: string) => invoke<void>("export_wire_list", { path }),
  searchProject: (query: string, kinds: SearchKind[]) =>
    invoke<SearchHit[]>("search_project", { query, kinds }),
  getDeviceTree: () => invoke<DeviceNode[]>("get_device_tree"),
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
const API_BASE = "http://127.0.0.1:9310/api/v1";

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
  importDxf: (path, wireLayers) =>
    http<KicadImportResult>("/import/dxf", {
      method: "POST",
      body: JSON.stringify({ path, wire_layers: wireLayers ?? [] }),
    }),
  newProject: () => Promise.reject(new Error("browser mode: not supported")),
  listTemplates: () => http<TemplateList>("/templates"),
  applyTemplate: (templateId, sheetId) =>
    http<Patch>("/templates/apply", {
      method: "POST",
      body: JSON.stringify({ template_id: templateId, sheet_id: sheetId }),
    }),
  // ブラウザ検証モードではOSのファイラを開けない (呼び出し側がパスを案内する)
  openTemplatesFolder: () => Promise.reject(new Error("browser mode: not supported")),
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
  exportDxf: async (sheetId, path) => {
    await http("/export/dxf", { method: "POST", body: JSON.stringify({ sheet_id: sheetId, path }) });
  },
  exportKicad: async (sheetId, path) => {
    await http("/export/kicad", { method: "POST", body: JSON.stringify({ sheet_id: sheetId, path }) });
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
