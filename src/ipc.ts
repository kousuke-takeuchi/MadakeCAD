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

export type Entity =
  | ({ kind: "symbol" } & SymbolInstance)
  | ({ kind: "wire" } & Wire)
  | ({ kind: "junction" } & Junction)
  | ({ kind: "net_label" } & NetLabel)
  | ({ kind: "text" } & TextEntity);

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
  | { type: "set_wire_parts"; wire_parts: WirePart[] };

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
  name: string;
  pins: NetPin[];
  wire_ids: string[];
}

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
