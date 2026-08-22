// PLC I/O割付表エディタ + I/O図面の生成設定の状態 (仕様: docs/internal/specs/m4-industrial-core.md §3)。
//
// 編集はこのストアの下書き(draft)の上だけで行い、保存時に set_plc_assignments コマンドを
// 1回だけ実行する (undo一発)。モデル(document ストアのミラー)は直接書き換えない。
// 接続先・線番は図面から導出した読み取り専用の値で、Rust側 (madake-coreの`plc_points`) が
// 返したものをそのまま並べる。
// アドレスの自動採番はRust側の`PlcModuleSpec::address_at`と同じ規則をここでも持つ
// (入力中のプレビューに使うため。保存した表が正)。

import { defineStore } from "pinia";
import {
  ipc,
  type Part,
  type PlcAddressStyle,
  type PlcAssignment,
  type PlcModuleInfo,
  type PlcModuleSpec,
  type PlcPoint,
  type PlcSheetOptions,
} from "../ipc";
import { useDocumentStore } from "./document";

/** グリッドの1行 (デザイン「PLC割付表エディタ」の列順)。 */
export interface PlcEditorRow {
  /** 1起点の点番号 (モジュールシンボルのピン番号)。 */
  point: number;
  address: string;
  /** 未割付の行に出す自動採番の候補アドレス (入力欄のプレースホルダ)。 */
  autoAddress: string;
  signalName: string;
  comment: string;
  /** 図面から読んだ接続先 (読み取り専用)。 */
  target: string;
  /** 図面から読んだ線番 (読み取り専用)。 */
  wireNo: string;
}

/** 下書きの1行 (編集できる3列だけ)。 */
interface PlcDraftRow {
  address: string;
  signalName: string;
  comment: string;
}

/** 0起点の点番号 → アドレスの後半部分 (Rustの`PlcAddressStyle::suffix`と同じ)。 */
function addressSuffix(style: PlcAddressStyle, index: number): string {
  if (style === "mitsubishi") return index.toString(8);
  if (style === "siemens") return `${Math.floor(index / 8)}.${index % 8}`;
  return `${Math.floor(index / 16)}/${index % 16}`;
}

/** 0起点の点番号に対応するアドレス (例 `X10`)。 */
export function autoAddress(spec: PlcModuleSpec, index: number): string {
  return `${spec.address_prefix}${addressSuffix(spec.address_style, index)}`;
}

/** `start`点目から始めて点数ぶんのアドレスを並べる (自動採番)。 */
export function autoAddresses(spec: PlcModuleSpec, start: number): string[] {
  return Array.from({ length: spec.points }, (_, i) => autoAddress(spec, start + i));
}

/**
 * 開始アドレスの文字列を0起点の点番号へ読み戻す。
 * アドレス体系に合わない書き方 (三菱なのに`X8`、接頭辞違い、空欄) はnull。
 */
export function parseAddressIndex(spec: PlcModuleSpec, text: string): number | null {
  const body = text.trim().toUpperCase();
  const prefix = spec.address_prefix.toUpperCase();
  if (!body.startsWith(prefix)) return null;
  const rest = body.slice(prefix.length);
  if (rest === "") return null;
  if (spec.address_style === "mitsubishi") {
    if (!/^[0-7]+$/.test(rest)) return null;
    return parseInt(rest, 8);
  }
  const separator = spec.address_style === "siemens" ? "." : "/";
  const parts = rest.split(separator);
  if (parts.length !== 2 || !/^\d+$/.test(parts[0]) || !/^\d+$/.test(parts[1])) return null;
  const size = spec.address_style === "siemens" ? 8 : 16;
  const bit = Number(parts[1]);
  if (bit >= size) return null;
  return Number(parts[0]) * size + bit;
}

/** 部品DBに無い型番のモジュールに使う既定の定義 (三菱・入力=X / 出力=Y)。 */
function fallbackSpec(module: PlcModuleInfo): PlcModuleSpec {
  return {
    points: module.points,
    kind: module.kind,
    address_prefix: module.kind === "DI" ? "X" : "Y",
    address_style: "mitsubishi",
  };
}

/** 部品DBの`plc_module`列 (JSON) を読む。空欄・壊れたJSONならnull。 */
function parseSpec(part: Part | undefined): PlcModuleSpec | null {
  if (!part?.plc_module) return null;
  try {
    const spec = JSON.parse(part.plc_module) as PlcModuleSpec;
    return spec.points > 0 ? spec : null;
  } catch {
    return null;
  }
}

function blankRow(): PlcDraftRow {
  return { address: "", signalName: "", comment: "" };
}

/** 保存する行を整える: 各欄をトリムし、**末尾の**全欄が空の行を落とす。 */
function normalize(draft: readonly PlcDraftRow[]): PlcDraftRow[] {
  const rows = draft.map((r) => ({
    address: r.address.trim(),
    signalName: r.signalName.trim(),
    comment: r.comment.trim(),
  }));
  // 途中の空行は残す (n行目=点n番の対応がずれるため)
  while (rows.length > 0) {
    const last = rows[rows.length - 1];
    if (last.address || last.signalName || last.comment) break;
    rows.pop();
  }
  return rows;
}

/** 新しい行のid。テスト環境も含めてcrypto.randomUUIDがあればそれを使う。 */
function newId(): string {
  return globalThis.crypto?.randomUUID?.() ?? `plc-${Math.random().toString(36).slice(2)}`;
}

/** 生成設定の既定値 (Rustの`PlcSheetOptions::default`と同じ)。 */
function defaultOptions(): PlcSheetOptions {
  return {
    rung_spacing_mm: 10,
    start_skip: 0,
    ladder_style: "vertical-bus",
    placement: "new-ladder",
  };
}

interface PlcIoState {
  open: boolean;
  /** 生成設定ダイアログを開いているか。 */
  generateOpen: boolean;
  /** 図面に置かれているPLCモジュール。 */
  modules: PlcModuleInfo[];
  /** 部品DBのPLCモジュール機種 (図面のvalue=型番で引き当てる)。 */
  parts: Part[];
  /** 表示中のモジュールの参照記号。 */
  moduleRef: string | null;
  /** 図面から導出した点の一覧 (接続先・線番の元)。 */
  points: PlcPoint[];
  /** 編集中の表 (モジュールの点数ぶん)。 */
  draft: PlcDraftRow[];
  /** 読み込んだ時点の表。変更の有無の判定に使う。 */
  original: PlcDraftRow[];
  /** 自動採番の開始アドレス (空欄なら先頭から)。 */
  startAddress: string;
  options: PlcSheetOptions;
  loading: boolean;
  saving: boolean;
  error: string | null;
}

export const usePlcIoStore = defineStore("plcIo", {
  state: (): PlcIoState => ({
    open: false,
    generateOpen: false,
    modules: [],
    parts: [],
    moduleRef: null,
    points: [],
    draft: [],
    original: [],
    startAddress: "",
    options: defaultOptions(),
    loading: false,
    saving: false,
    error: null,
  }),

  getters: {
    /** 表示中のモジュールの概要。 */
    module(state): PlcModuleInfo | null {
      return state.modules.find((m) => m.reference === state.moduleRef) ?? null;
    },
    /** 表示中のモジュールの定義 (部品DB優先、無ければ種別どおりの既定)。 */
    spec(state): PlcModuleSpec | null {
      const module = this.module;
      if (!module) return null;
      const part = state.parts.find((p) => p.part_no === module.value);
      return parseSpec(part) ?? fallbackSpec(module);
    },
    /** 自動採番の開始点 (0起点)。開始アドレスが読めなければ0。 */
    startIndex(state): number {
      const spec = this.spec;
      if (!spec) return 0;
      return parseAddressIndex(spec, state.startAddress) ?? 0;
    },
    /** グリッドの行 (点番号順)。 */
    rows(state): PlcEditorRow[] {
      const spec = this.spec;
      const start = this.startIndex;
      return state.draft.map((r, i) => ({
        point: i + 1,
        address: r.address,
        autoAddress: spec ? autoAddress(spec, start + i) : "",
        signalName: r.signalName,
        comment: r.comment,
        target: state.points[i]?.target ?? "",
        wireNo: state.points[i]?.wire_no ?? "",
      }));
    },
    /** 読み込んだ時点から表の中身が変わったか。 */
    dirty(state): boolean {
      return JSON.stringify(normalize(state.draft)) !== JSON.stringify(normalize(state.original));
    },
    /** 保存する割付表 (プロジェクト全体。他モジュールの行はそのまま)。 */
    assignments(state): PlcAssignment[] {
      const moduleRef = state.moduleRef;
      if (!moduleRef) return [];
      const existing = useDocumentStore().project?.plc_assignments ?? [];
      const rows: PlcAssignment[] = normalize(state.draft).map((r, i) => ({
        id: existing.filter((a) => a.module_ref === moduleRef)[i]?.id ?? newId(),
        module_ref: moduleRef,
        address: r.address,
        signal_name: r.signalName,
        comment: r.comment,
      }));
      // Rustの`replaced_assignments`と同じ: 元の位置へ差し込み、無ければ末尾へ足す
      const out: PlcAssignment[] = [];
      let inserted = false;
      for (const a of existing) {
        if (a.module_ref === moduleRef) {
          if (!inserted) {
            out.push(...rows);
            inserted = true;
          }
          continue;
        }
        out.push(a);
      }
      if (!inserted) out.push(...rows);
      return out;
    },
    /** I/O図面を生成できるか (v1が対応する設定で、モジュールが選ばれているか)。 */
    canGenerate(state): boolean {
      if (!this.module || !this.spec) return false;
      return (
        state.options.ladder_style === "vertical-bus" && state.options.placement === "new-ladder"
      );
    },
  },

  actions: {
    /** 図面のPLCモジュールを読み込み、先頭のモジュールの割付表を開く。 */
    async openEditor() {
      this.error = null;
      this.open = true;
      this.startAddress = "";
      await this.reload();
    },

    /** モジュール一覧・部品DB・割付表を読み直す (下書きは読み直した表で作り直す)。 */
    async reload() {
      this.loading = true;
      try {
        this.modules = await ipc.listPlcModules();
        this.parts = await ipc.listPlcModuleParts();
        const keep = this.modules.some((m) => m.reference === this.moduleRef);
        this.moduleRef = keep ? this.moduleRef : (this.modules[0]?.reference ?? null);
        await this.reloadPoints();
        this.resetDraft();
      } catch (e) {
        this.error = String(e);
      } finally {
        this.loading = false;
      }
    },

    /** 表示中のモジュールの点 (接続先・線番つき) を読み直す。 */
    async reloadPoints() {
      this.points = this.moduleRef ? await ipc.getPlcAssignments(this.moduleRef) : [];
    },

    /** 下書きを図面・割付表の中身で作り直す (未保存の編集は消える)。 */
    resetDraft() {
      const count = this.module?.points ?? 0;
      const rows: PlcDraftRow[] = Array.from({ length: count }, (_, i) => {
        const p = this.points[i];
        return p
          ? { address: p.address, signalName: p.signal_name, comment: p.comment }
          : blankRow();
      });
      this.draft = rows;
      this.original = rows.map((r) => ({ ...r }));
    },

    /** 別のモジュールへ切り替える (未保存の編集は捨てる)。 */
    async selectModule(reference: string) {
      this.moduleRef = reference;
      this.startAddress = "";
      this.error = null;
      await this.reloadPoints();
      this.resetDraft();
    },

    /** 1行の編集できる列を書き換える。 */
    updateRow(index: number, patch: Partial<PlcDraftRow>) {
      const row = this.draft[index];
      if (row) Object.assign(row, patch);
    },

    /** 開始アドレスから全行のアドレスを自動採番で埋める (信号名・コメントはそのまま)。 */
    fillAddresses() {
      const spec = this.spec;
      if (!spec) return;
      const start = this.startIndex;
      this.draft.forEach((row, i) => {
        row.address = autoAddress(spec, start + i);
      });
    },

    /**
     * set_plc_assignmentsコマンドを1回実行する。変更が無ければ何も送らない。
     * 戻り値=保存した行数 (未送信ならnull)。
     */
    async save(): Promise<number | null> {
      if (!this.moduleRef || this.saving) return null;
      if (!this.dirty) return null;
      const assignments = this.assignments;
      const saved = normalize(this.draft).length;
      this.saving = true;
      this.error = null;
      try {
        await useDocumentStore().execute({ type: "set_plc_assignments", assignments });
      } catch (e) {
        this.error = String(e);
        return null;
      } finally {
        this.saving = false;
      }
      this.original = this.draft.map((r) => ({ ...r }));
      await this.reloadPoints();
      return saved;
    },

    /**
     * CSVの中身を取り込む (対象モジュールの行だけ置き換え。Command経由なのでundo一発)。
     * 読めないCSVならエラーを表示して表は変えない。
     */
    async importCsv(csv: string): Promise<boolean> {
      if (!this.moduleRef) return false;
      this.error = null;
      try {
        const patch = await ipc.importPlcAssignmentsCsv(this.moduleRef, csv);
        useDocumentStore().applyEdit(patch);
      } catch (e) {
        this.error = String(e);
        return false;
      }
      await this.reloadPoints();
      this.resetDraft();
      return true;
    },

    /** 生成設定ダイアログを開く。 */
    openGenerate() {
      this.error = null;
      this.generateOpen = true;
    },

    /** 生成設定ダイアログを閉じる (生成しない)。 */
    cancelGenerate() {
      this.generateOpen = false;
    },

    /**
     * I/O図面 (ラダーページ) を生成する。未保存の編集があれば先に割付表を保存する。
     * v1が対応しない設定 (横バス・同居配置) では何もしない。
     */
    async generate(): Promise<boolean> {
      const module = this.module;
      const spec = this.spec;
      if (!module || !spec || !this.canGenerate) return false;
      this.error = null;
      if (this.dirty && (await this.save()) === null) return false;
      try {
        const patch = await ipc.generatePlcSheet(module.reference, spec, { ...this.options });
        useDocumentStore().applyEdit(patch);
      } catch (e) {
        this.error = String(e);
        return false;
      }
      this.generateOpen = false;
      await this.reloadPoints();
      this.resetDraft();
      return true;
    },

    /**
     * 図面が変わったときに表を追従させる (undo/redo・AIチャット・CLIなど、
     * このダイアログの外からの編集を含む)。編集中の下書きは消さず、
     * 読み取り専用の列 (接続先・線番) だけを取り直す。
     */
    async refresh() {
      if (!this.open) return;
      const dirty = this.dirty;
      try {
        this.modules = await ipc.listPlcModules();
        const keep = this.modules.some((m) => m.reference === this.moduleRef);
        this.moduleRef = keep ? this.moduleRef : (this.modules[0]?.reference ?? null);
        await this.reloadPoints();
        if (!dirty) this.resetDraft();
      } catch (e) {
        this.error = String(e);
      }
    },

    /** 閉じる (下書きは捨てる)。 */
    close() {
      this.open = false;
      this.generateOpen = false;
      this.modules = [];
      this.points = [];
      this.draft = [];
      this.original = [];
      this.moduleRef = null;
      this.startAddress = "";
      this.error = null;
    },
  },
});
