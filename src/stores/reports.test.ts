// 帳票生成ダイアログ・PDF一括出力ダイアログの仕様テスト
// (docs/internal/specs/m4-industrial-core.md §5、リボン「レポート」タブ)。
import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { ipc, type TerminalBlockInfo } from "../ipc";
import { REPORT_KINDS, usePdfBookStore, useReportDialogStore } from "./reports";

const tb1: TerminalBlockInfo = {
  entity_id: "tb1",
  sheet_id: "s1",
  sheet_name: "Sheet1",
  reference: "TB1",
  value: "UK 2.5N",
  terminal_count: 4,
  jumpers: "",
};

function mockBlocks(blocks: TerminalBlockInfo[] = [tb1]) {
  vi.spyOn(ipc, "listTerminalBlocks").mockResolvedValue(blocks);
}

describe("report generation dialog store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.restoreAllMocks();
    mockBlocks();
  });

  // ja: 帳票の種類は From-To電線リスト・端子台チャート・端子接続図・部品表・XRef表・PLC I/Oレポート の6つ
  it("offers the six report kinds of the reports tab", () => {
    expect(REPORT_KINDS).toEqual([
      "wire-list",
      "terminal-chart",
      "terminal-diagram",
      "bom",
      "xref",
      "plc-io",
    ]);
  });

  // ja: 部品表はプロジェクト全体が対象で、既定はCSV出力
  it("opens a project-wide report as a CSV export", async () => {
    const store = useReportDialogStore();
    await store.openFor("bom");
    expect(store.open).toBe(true);
    expect(store.format).toBe("csv");
    expect(store.entityId).toBeNull();
    expect(store.targets).toHaveLength(1);
    store.path = "/tmp/bom.csv";
    expect(store.request).toEqual({
      kind: "bom",
      format: "csv",
      entityId: null,
      path: "/tmp/bom.csv",
    });
  });

  // ja: 出力形式を図面シートPDFにすると、同じ帳票がPDF要求になる
  it("switches the same report to a framed drawing-sheet PDF", async () => {
    const store = useReportDialogStore();
    await store.openFor("bom");
    store.format = "pdf";
    store.path = "/tmp/bom.pdf";
    expect(store.request).toMatchObject({ kind: "bom", format: "pdf" });
  });

  // ja: 端子台チャートは「プロジェクト全体」に加えて端子台1つを対象に選べる
  it("lets the terminal chart target one terminal block as well as the whole project", async () => {
    const store = useReportDialogStore();
    await store.openFor("terminal-chart");
    expect(store.targets.map((t) => t.id)).toEqual([null, "tb1"]);
    store.entityId = "tb1";
    store.path = "/tmp/tb1.csv";
    expect(store.request).toMatchObject({ kind: "terminal-chart", entityId: "tb1" });
  });

  // ja: 端子接続図はグラフィカルな図面なのでPDFだけを選べる
  it("offers PDF only for the graphical terminal connection diagram", async () => {
    const store = useReportDialogStore();
    await store.openFor("terminal-diagram");
    expect(store.formats).toEqual(["pdf"]);
    expect(store.format).toBe("pdf");
  });

  // ja: 「帳票のシート化」から開くと図面シートPDFを選んだ状態で開く
  it("opens with the drawing-sheet PDF already chosen when asked for it", async () => {
    const store = useReportDialogStore();
    await store.openFor("bom", "pdf");
    expect(store.format).toBe("pdf");
  });

  // ja: 帳票にその出力形式が無ければ、選べる形式に読み替えて開く
  it("falls back to a format the report actually has", async () => {
    const store = useReportDialogStore();
    await store.openFor("terminal-diagram", "csv");
    expect(store.format).toBe("pdf");
  });

  // ja: ダイアログを開いたまま帳票を変えると、対象と出力形式もその帳票のものに入れ替わる
  it("swaps the targets and the formats when another report is picked in the dialog", async () => {
    const store = useReportDialogStore();
    await store.openFor("terminal-chart");
    store.entityId = "tb1";
    store.format = "pdf";
    await store.setKind("bom");
    expect(store.entityId).toBeNull();
    expect(store.targets).toHaveLength(1);
    await store.setKind("terminal-diagram");
    expect(store.format).toBe("pdf");
    expect(store.targets.map((t) => t.id)).toEqual([null, "tb1"]);
  });

  // ja: 既定のファイル名は帳票名と対象と出力形式から決まる
  it("suggests a file name from the report, the target and the format", async () => {
    const store = useReportDialogStore();
    await store.openFor("terminal-chart");
    expect(store.defaultFileName).toBe("terminal-chart.csv");
    store.entityId = "tb1";
    expect(store.defaultFileName).toBe("terminal-chart-TB1.csv");
    store.format = "pdf";
    expect(store.defaultFileName).toBe("terminal-chart-TB1.pdf");
  });

  // ja: 出力先パスが空のままでは生成できない
  it("cannot generate anything while the output path is empty", async () => {
    const store = useReportDialogStore();
    await store.openFor("bom");
    expect(store.request).toBeNull();
    const spy = vi.spyOn(ipc, "exportReport").mockResolvedValue(0);
    expect(await store.run()).toBeNull();
    expect(spy).not.toHaveBeenCalled();
  });

  // ja: 生成すると帳票を1回だけ書き出し、書き出した件数を返して閉じる
  it("writes the report once, reports how much it wrote and closes", async () => {
    const spy = vi.spyOn(ipc, "exportReport").mockResolvedValue(7);
    const store = useReportDialogStore();
    await store.openFor("bom");
    store.path = "/tmp/bom.csv";
    expect(await store.run()).toBe(7);
    expect(spy).toHaveBeenCalledWith("bom", "csv", null, "/tmp/bom.csv");
    expect(store.open).toBe(false);
  });

  // ja: 書き出しに失敗したらダイアログは開いたままエラーを表示する
  it("keeps the dialog open and shows the error when the export fails", async () => {
    vi.spyOn(ipc, "exportReport").mockRejectedValue(new Error("boom"));
    const store = useReportDialogStore();
    await store.openFor("bom");
    store.path = "/tmp/bom.csv";
    expect(await store.run()).toBeNull();
    expect(store.open).toBe(true);
    expect(store.error).toContain("boom");
  });
});

describe("PDF book dialog store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.restoreAllMocks();
  });

  // ja: 既定では表紙付きで、全ての帳票が回路図の後ろに付く
  it("includes the cover and every report by default", () => {
    const store = usePdfBookStore();
    store.openDialog();
    expect(store.cover).toBe(true);
    expect(store.selectedReports).toEqual(REPORT_KINDS);
  });

  // ja: 選んだ帳票は種類の並び順どおりに並ぶ(選んだ順ではない)
  it("keeps the chosen reports in the fixed report order", () => {
    const store = usePdfBookStore();
    store.openDialog();
    for (const kind of REPORT_KINDS) store.reports[kind] = false;
    store.reports.bom = true;
    store.reports["wire-list"] = true;
    expect(store.selectedReports).toEqual(["wire-list", "bom"]);
  });

  // ja: 表紙は外せる。帳票を1つも選ばなければ回路図だけのPDFになる
  it("can drop the cover and every report to export the schematics alone", () => {
    const store = usePdfBookStore();
    store.openDialog();
    store.cover = false;
    for (const kind of REPORT_KINDS) store.reports[kind] = false;
    store.path = "/tmp/book.pdf";
    expect(store.request).toEqual({ path: "/tmp/book.pdf", reports: [], cover: false });
  });

  // ja: 出力先パスが空のままでは出力できない
  it("cannot export while the output path is empty", async () => {
    const spy = vi.spyOn(ipc, "exportPdfBook").mockResolvedValue(0);
    const store = usePdfBookStore();
    store.openDialog();
    expect(store.request).toBeNull();
    expect(await store.run()).toBeNull();
    expect(spy).not.toHaveBeenCalled();
  });

  // ja: 出力すると1ファイルに書き出し、ページ数を返して閉じる
  it("writes one file, reports the page count and closes", async () => {
    const spy = vi.spyOn(ipc, "exportPdfBook").mockResolvedValue(9);
    const store = usePdfBookStore();
    store.openDialog();
    store.path = "/tmp/book.pdf";
    expect(await store.run()).toBe(9);
    expect(spy).toHaveBeenCalledWith("/tmp/book.pdf", REPORT_KINDS, true);
    expect(store.open).toBe(false);
  });
});
