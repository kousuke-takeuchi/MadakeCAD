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
  | { op: "wire_parts_replaced"; wire_parts: WirePart[] };

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

export interface PinDef {
  number: string;
  name: string;
  at: Point;
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
  primitives: Primitive[];
  pins: PinDef[];
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

/** 帳票の種類 (Rustの`ReportKind`と同じケバブケース表記)。 */
export type ReportKind = "wire-list" | "terminal-chart" | "terminal-diagram" | "bom" | "xref";
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
  newProject(name: string): Promise<Patch>;
  /** 使える開始テンプレートの一覧 (同梱+ユーザーの`~/MadakeCAD/templates`)。 */
  listTemplates(): Promise<TemplateList>;
  /** テンプレートをシートへ適用する。1回の編集なのでundo一発で戻る。 */
  applyTemplate(templateId: string, sheetId: string): Promise<Patch>;
  /** ユーザーテンプレートの置き場をOSのファイラで開く (無ければ作る)。戻り値=そのパス。 */
  openTemplatesFolder(): Promise<string>;
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
  /** シート内 (nullでプロジェクト全体) の端子台一覧。 */
  listTerminalBlocks(sheetId: string | null): Promise<TerminalBlockInfo[]>;
  /** 端子台1つのチャート。端子台でなければnull。 */
  getTerminalChart(entityId: string): Promise<TerminalChart | null>;
  /** 端子台チェック (未結線・不正なジャンパ)。 */
  checkTerminalBlock(entityId: string): Promise<Diagnostic[]>;
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
  newProject: (name: string) => invoke<Patch>("new_project", { name }),
  listTemplates: () => invoke<TemplateList>("list_templates"),
  applyTemplate: (templateId: string, sheetId: string) =>
    invoke<Patch>("apply_template", { templateId, sheetId }),
  openTemplatesFolder: () => invoke<string>("open_templates_folder"),
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
  listTerminalBlocks: (sheetId: string | null) =>
    invoke<TerminalBlockInfo[]>("list_terminal_blocks", { sheetId }),
  getTerminalChart: (entityId: string) =>
    invoke<TerminalChart | null>("get_terminal_chart", { entityId }),
  checkTerminalBlock: (entityId: string) =>
    invoke<Diagnostic[]>("check_terminal_block", { entityId }),
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
  newProject: () => Promise.reject(new Error("browser mode: not supported")),
  listTemplates: () => http<TemplateList>("/templates"),
  applyTemplate: (templateId, sheetId) =>
    http<Patch>("/templates/apply", {
      method: "POST",
      body: JSON.stringify({ template_id: templateId, sheet_id: sheetId }),
    }),
  // ブラウザ検証モードではOSのファイラを開けない (呼び出し側がパスを案内する)
  openTemplatesFolder: () => Promise.reject(new Error("browser mode: not supported")),
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
