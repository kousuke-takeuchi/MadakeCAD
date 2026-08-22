// 帳票生成ダイアログとPDF一括出力ダイアログの状態
// (仕様: docs/internal/specs/m4-industrial-core.md §5、リボン「レポート」タブ)。
//
// どちらも「何を・どの範囲で・どの形式で・どこへ」を組み立ててRust側へ渡すだけで、
// 帳票の中身の組み立ては全てmadake-core側にある。

import { defineStore } from "pinia";
import { ipc, type ReportFormat, type ReportKind, type TerminalBlockInfo } from "../ipc";

/** リボン「レポート」タブに出る帳票の種類 (この並び順がPDF一括の綴じ順)。 */
export const REPORT_KINDS: ReportKind[] = [
  "wire-list",
  "terminal-chart",
  "terminal-diagram",
  "bom",
  "xref",
  "plc-io",
];

/** 端子台1つを対象に選べる帳票 (それ以外は常にプロジェクト全体)。 */
const TERMINAL_KINDS: ReportKind[] = ["terminal-chart", "terminal-diagram"];

/** その帳票で選べる出力形式。端子接続図は図面なのでPDFのみ。 */
export function formatsOf(kind: ReportKind): ReportFormat[] {
  return kind === "terminal-diagram" ? ["pdf"] : ["csv", "pdf"];
}

/** 帳票の出力対象1件。id=nullはプロジェクト全体。 */
export interface ReportTarget {
  id: string | null;
  reference: string;
  sheetName: string;
}

/** 帳票の生成要求 (そのままipc.exportReportへ渡る)。 */
export interface ReportRequest {
  kind: ReportKind;
  format: ReportFormat;
  entityId: string | null;
  path: string;
}

interface ReportDialogState {
  open: boolean;
  kind: ReportKind;
  format: ReportFormat;
  /** 対象の端子台 (nullでプロジェクト全体)。 */
  entityId: string | null;
  /** 対象に選べる端子台。 */
  blocks: TerminalBlockInfo[];
  path: string;
  running: boolean;
  error: string | null;
}

export const useReportDialogStore = defineStore("reportDialog", {
  state: (): ReportDialogState => ({
    open: false,
    kind: "bom",
    format: "csv",
    entityId: null,
    blocks: [],
    path: "",
    running: false,
    error: null,
  }),

  getters: {
    /** この帳票で選べる出力形式。 */
    formats(state): ReportFormat[] {
      return formatsOf(state.kind);
    },
    /** 対象の候補。端子台の帳票だけプロジェクト全体+端子台ごとになる。 */
    targets(state): ReportTarget[] {
      const all: ReportTarget = { id: null, reference: "", sheetName: "" };
      if (!TERMINAL_KINDS.includes(state.kind)) return [all];
      return [
        all,
        ...state.blocks.map((b) => ({
          id: b.entity_id,
          reference: b.reference,
          sheetName: b.sheet_name,
        })),
      ];
    },
    /** 選択中の対象の端子台。 */
    target(state): TerminalBlockInfo | null {
      return state.blocks.find((b) => b.entity_id === state.entityId) ?? null;
    },
    /** 既定のファイル名 (帳票の種類-対象.拡張子)。 */
    defaultFileName(state): string {
      const target = this.target;
      const suffix = target ? `-${target.reference}` : "";
      return `${state.kind}${suffix}.${state.format}`;
    },
    /** 生成要求。出力先パスが空ならnull。 */
    request(state): ReportRequest | null {
      const path = state.path.trim();
      if (!path) return null;
      return {
        kind: state.kind,
        format: state.format,
        entityId: state.entityId,
        path,
      };
    },
  },

  actions: {
    /**
     * その帳票の生成ダイアログを既定値で開く (端子台の帳票なら対象一覧も読む)。
     * `format`を渡すとその出力形式を選んだ状態で開く (帳票にその形式が無ければ無視)。
     */
    async openFor(kind: ReportKind, format?: ReportFormat) {
      this.path = "";
      this.error = null;
      this.open = true;
      await this.setKind(kind, format);
    },

    /**
     * ダイアログを開いたまま帳票の種類を変える。出力形式はその帳票にある形式へ、
     * 対象はプロジェクト全体へ戻し、端子台の帳票なら対象の候補を読み直す。
     */
    async setKind(kind: ReportKind, format?: ReportFormat) {
      this.kind = kind;
      const available = formatsOf(kind);
      this.format = format && available.includes(format) ? format : available[0];
      this.entityId = null;
      this.blocks = TERMINAL_KINDS.includes(kind) ? await ipc.listTerminalBlocks(null) : [];
    },

    cancel() {
      this.open = false;
      this.error = null;
    },

    /** 帳票を書き出して閉じる。戻り値=書き出した件数 (CSVは行数、PDFはページ数)。 */
    async run(): Promise<number | null> {
      const request = this.request;
      if (!request || this.running) return null;
      this.running = true;
      this.error = null;
      try {
        const count = await ipc.exportReport(
          request.kind,
          request.format,
          request.entityId,
          request.path,
        );
        this.open = false;
        return count;
      } catch (e) {
        this.error = String(e);
        return null;
      } finally {
        this.running = false;
      }
    },
  },
});

/** PDF一括出力の要求。 */
export interface PdfBookRequest {
  path: string;
  reports: ReportKind[];
  cover: boolean;
}

interface PdfBookState {
  open: boolean;
  /** 綴じる帳票のON/OFF。 */
  reports: Record<ReportKind, boolean>;
  /** 表紙 (プロジェクト名・シート一覧・最新改訂) を付けるか。 */
  cover: boolean;
  path: string;
  running: boolean;
  error: string | null;
}

function allReports(on: boolean): Record<ReportKind, boolean> {
  return Object.fromEntries(REPORT_KINDS.map((k) => [k, on])) as Record<ReportKind, boolean>;
}

export const usePdfBookStore = defineStore("pdfBook", {
  state: (): PdfBookState => ({
    open: false,
    reports: allReports(true),
    cover: true,
    path: "",
    running: false,
    error: null,
  }),

  getters: {
    /** 選ばれた帳票 ([`REPORT_KINDS`] の並び順)。 */
    selectedReports(state): ReportKind[] {
      return REPORT_KINDS.filter((k) => state.reports[k]);
    },
    /** 出力要求。出力先パスが空ならnull。 */
    request(state): PdfBookRequest | null {
      const path = state.path.trim();
      if (!path) return null;
      return { path, reports: this.selectedReports, cover: state.cover };
    },
  },

  actions: {
    /** 既定 (表紙あり・全帳票) で開く。 */
    openDialog() {
      this.reports = allReports(true);
      this.cover = true;
      this.path = "";
      this.error = null;
      this.open = true;
    },

    cancel() {
      this.open = false;
      this.error = null;
    },

    /** 1つのPDFへ書き出して閉じる。戻り値=ページ数。 */
    async run(): Promise<number | null> {
      const request = this.request;
      if (!request || this.running) return null;
      this.running = true;
      this.error = null;
      try {
        const pages = await ipc.exportPdfBook(request.path, request.reports, request.cover);
        this.open = false;
        return pages;
      } catch (e) {
        this.error = String(e);
        return null;
      } finally {
        this.running = false;
      }
    },
  },
});
