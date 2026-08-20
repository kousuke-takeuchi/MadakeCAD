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

export const ipc = {
  getProject: () => invoke<ProjectSnapshot>("get_project"),
  listSymbols: () => invoke<SymbolDef[]>("list_symbols"),
  executeCommand: (command: Command) => invoke<Patch>("execute_command", { command }),
  undo: () => invoke<Patch | null>("undo"),
  redo: () => invoke<Patch | null>("redo"),
  saveProject: (path: string) => invoke<void>("save_project", { path }),
  loadProject: (path: string) => invoke<Patch>("load_project", { path }),
  newProject: (name: string) => invoke<Patch>("new_project", { name }),
  getNetlist: (sheetId: string) => invoke<Net[]>("get_netlist", { sheetId }),
  exportSvg: (sheetId: string, path: string) => invoke<void>("export_svg", { sheetId, path }),
  exportBom: (path: string) => invoke<void>("export_bom", { path }),
  exportWireList: (path: string) => invoke<void>("export_wire_list", { path }),
  onPatch: (handler: (patch: Patch) => void): Promise<UnlistenFn> =>
    listen<Patch>("doc:patch", (e) => handler(e.payload)),
};
