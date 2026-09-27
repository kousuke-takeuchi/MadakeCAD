// 整えバリアント (2〜4案の並列比較) の仕様テスト (docs/internal/specs/m3-ai-first.md §3・§4)。
import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { ipc, type Patch, type Project, type Sheet, type TidyMetrics } from "../ipc";
import { agentApi, useChatStore } from "./chat";
import { useDocumentStore } from "./document";
import { useUiStore } from "./ui";
import { mapSelection, metricsTotal, useVariantsStore } from "./variants";

const ORIGINAL = "aaaaaaaa-0000-4000-8000-000000000001";
const COPY_A = "aaaaaaaa-0000-4000-8000-00000000000a";
const COPY_B = "aaaaaaaa-0000-4000-8000-00000000000b";
const CONV_A = "cccccccc-0000-4000-8000-00000000000a";
const CONV_B = "cccccccc-0000-4000-8000-00000000000b";

function sheet(id: string, name: string, entityIds: string[]): Sheet {
  const entities: Sheet["entities"] = {};
  for (const eid of entityIds) {
    entities[eid] = {
      kind: "junction",
      id: eid,
      at: { x: 10, y: 10 },
    } as Sheet["entities"][string];
  }
  return { id, name, size: "A3", orientation: "Landscape", zone_cols: 4, zone_rows: 6, title_block: {}, revisions: [], entities };
}

function metrics(total: number): TidyMetrics {
  return { crossings: total, label_overlaps: 0, symbol_overlaps: 0, off_grid: 0 };
}

/** 開始IPCの応答: 複製2枚が入ったpatchと、対応表つきの案情報。 */
function startResult(): { patch: Patch; run: { original_sheet_id: string; variants: { sheet_id: string; label: string; id_map: Record<string, string> }[] } } {
  return {
    patch: {
      revision: 2,
      ops: [
        { op: "sheet_added", index: 1, sheet: sheet(COPY_A, "Sheet1 · 案A", ["e1a", "e2a"]) },
        { op: "sheet_added", index: 2, sheet: sheet(COPY_B, "Sheet1 · 案B", ["e1b", "e2b"]) },
      ],
    },
    run: {
      original_sheet_id: ORIGINAL,
      variants: [
        { sheet_id: COPY_A, label: "案A", id_map: { e1: "e1a", e2: "e2a" } },
        { sheet_id: COPY_B, label: "案B", id_map: { e1: "e1b", e2: "e2b" } },
      ],
    },
  };
}

function setup() {
  const doc = useDocumentStore();
  const project: Project = {
    format_version: 2,
    name: "t",
    wire_parts: [],
    plc_assignments: [],
    sheets: [sheet(ORIGINAL, "Sheet1", ["e1", "e2"])],
  };
  doc.project = project;
  doc.activeSheetId = ORIGINAL;
  doc.revision = 1;
  const sends: string[] = [];
  const ids = [CONV_A, CONV_B];
  vi.spyOn(agentApi, "send").mockImplementation(async (_id, prompt) => {
    sends.push(prompt);
    return ids[sends.length - 1];
  });
  vi.spyOn(agentApi, "cancel").mockResolvedValue();
  vi.spyOn(ipc, "startVariants").mockResolvedValue(startResult());
  vi.spyOn(ipc, "getTidyMetrics").mockImplementation(async (sheetId) =>
    metrics(sheetId === ORIGINAL ? 5 : sheetId === COPY_A ? 1 : 3),
  );
  vi.spyOn(ipc, "finishVariants").mockResolvedValue({
    revision: 9,
    ops: [
      { op: "sheet_removed", sheet_id: COPY_A },
      { op: "sheet_removed", sheet_id: COPY_B },
    ],
  });
  return { doc, sends };
}

describe("variants store: 整え案の開始", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    setActivePinia(createPinia());
  });

  // ja: 開始すると表示中のシートが案の数だけ複製され、案ごとに別の会話で同じ整え指示が送られ、比較パネルが開く
  it("start copies the active sheet once per variant, sends the same tidy prompt in a separate conversation per copy, and opens the panel", async () => {
    const { doc, sends } = setup();
    const variants = useVariantsStore();
    expect(await variants.start("layout", 2)).toBe(true);
    expect(ipc.startVariants).toHaveBeenCalledWith(ORIGINAL, 2);
    expect(doc.project?.sheets.map((s) => s.name)).toEqual(["Sheet1", "Sheet1 · 案A", "Sheet1 · 案B"]);
    expect(sends).toHaveLength(2);
    expect(sends[0]).toContain("配置整理");
    expect(sends[0]).toContain(`sheet_id: ${COPY_A}`);
    expect(sends[1]).toContain(`sheet_id: ${COPY_B}`);
    expect(variants.run?.variants.map((v) => v.conversationId)).toEqual([CONV_A, CONV_B]);
    expect(variants.panelOpen).toBe(true);
    expect(useUiStore().commandHistory.join("\n")).toContain("as 2 variants");
  });

  // ja: 選択があれば、その要素の対応する複製側の要素だけを整える範囲として指示する
  it("a selection is mapped to the copy's entity ids in each prompt", async () => {
    const { doc, sends } = setup();
    doc.selection = new Set(["e1"]);
    await useVariantsStore().start("wiring", 2);
    expect(sends[0]).toContain("id: e1a");
    expect(sends[0]).not.toContain("id: e1)");
    expect(sends[1]).toContain("id: e1b");
    expect(mapSelection(["e1", "zzz"], { e1: "e1a" })).toEqual(["e1a"]);
  });

  // ja: 開始直後に元の図面と各案の指標が取られる
  it("metrics of the original and every variant are fetched after start", async () => {
    setup();
    const variants = useVariantsStore();
    await variants.start("labels", 2);
    expect(variants.run?.baseline).toEqual(metrics(5));
    expect(variants.run?.variants.map((v) => v.metrics && metricsTotal(v.metrics))).toEqual([1, 3]);
  });

  // ja: シートが無い・案の数が2〜4以外・比較中は開始しない
  it("start refuses without a sheet, with a count outside 2..4, and while a comparison is in progress", async () => {
    const { doc } = setup();
    const variants = useVariantsStore();
    expect(await variants.start("layout", 5)).toBe(false);
    expect(await variants.start("layout", 1)).toBe(false);
    doc.project!.sheets = [];
    doc.activeSheetId = null;
    expect(await variants.start("layout", 2)).toBe(false);
    expect(ipc.startVariants).not.toHaveBeenCalled();
    doc.project!.sheets = [sheet(ORIGINAL, "Sheet1", ["e1"])];
    doc.activeSheetId = ORIGINAL;
    expect(await variants.start("layout", 2)).toBe(true);
    expect(await variants.start("layout", 2)).toBe(false);
    expect(ipc.startVariants).toHaveBeenCalledTimes(1);
  });

  // ja: 複製に失敗したら理由をログに出し、何も始まらない
  it("a failed copy is logged and nothing starts", async () => {
    setup();
    vi.spyOn(ipc, "startVariants").mockRejectedValue(new Error("sheet not found"));
    const variants = useVariantsStore();
    expect(await variants.start("layout", 2)).toBe(false);
    expect(variants.run).toBeNull();
    expect(agentApi.send).not.toHaveBeenCalled();
    expect(useUiStore().lastMessage).toContain("sheet not found");
  });
});

describe("variants store: 比較・採用・破棄", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    setActivePinia(createPinia());
  });

  // ja: 案の会話が答えている間は「実行中」で、全部終わると採用できる
  it("variants count as running while their conversation answers, and adopt waits for all of them", async () => {
    setup();
    const variants = useVariantsStore();
    const chat = useChatStore();
    await variants.start("layout", 2);
    expect(variants.runningCount).toBe(2);
    expect(variants.allDone).toBe(false);
    expect(await variants.adopt(COPY_A)).toBe(false);
    expect(ipc.finishVariants).not.toHaveBeenCalled();
    chat.setRunning(CONV_A, false);
    expect(variants.runningCount).toBe(1);
    chat.setRunning(CONV_B, false);
    expect(variants.allDone).toBe(true);
  });

  // ja: 採用すると選んだ案が終了IPCへ渡り、元の図面へ戻って比較は終わる (undo一発で戻せる旨をログ)
  it("adopt finishes with the chosen sheet, returns to the original sheet and ends the comparison", async () => {
    const { doc } = setup();
    const variants = useVariantsStore();
    const chat = useChatStore();
    await variants.start("layout", 2);
    chat.setRunning(CONV_A, false);
    chat.setRunning(CONV_B, false);
    variants.show(COPY_A);
    expect(doc.activeSheetId).toBe(COPY_A);
    expect(await variants.adopt(COPY_A)).toBe(true);
    expect(ipc.finishVariants).toHaveBeenCalledWith(
      {
        original_sheet_id: ORIGINAL,
        variants: [
          { sheet_id: COPY_A, label: "案A", id_map: { e1: "e1a", e2: "e2a" } },
          { sheet_id: COPY_B, label: "案B", id_map: { e1: "e1b", e2: "e2b" } },
        ],
      },
      COPY_A,
    );
    expect(doc.project?.sheets.map((s) => s.id)).toEqual([ORIGINAL]);
    expect(doc.activeSheetId).toBe(ORIGINAL);
    expect(variants.run).toBeNull();
    expect(variants.panelOpen).toBe(false);
    expect(useUiStore().lastMessage).toContain("案A");
  });

  // ja: 破棄は実行中の案の会話を止めてから全複製を消す
  it("discard cancels the conversations still running and removes every copy", async () => {
    setup();
    const variants = useVariantsStore();
    const chat = useChatStore();
    await variants.start("layout", 2);
    chat.setRunning(CONV_A, false);
    expect(await variants.discard()).toBe(true);
    expect(agentApi.cancel).toHaveBeenCalledTimes(1);
    expect(agentApi.cancel).toHaveBeenCalledWith(CONV_B);
    expect(ipc.finishVariants).toHaveBeenCalledWith(expect.anything(), null);
    expect(variants.run).toBeNull();
    expect(useUiStore().lastMessage).toContain("Discarded");
  });

  // ja: 終了に失敗したら理由をログに出し、比較はそのまま続く
  it("a failed finish is logged and the comparison stays open", async () => {
    setup();
    vi.spyOn(ipc, "finishVariants").mockRejectedValue(new Error("busy"));
    const variants = useVariantsStore();
    await variants.start("layout", 2);
    expect(await variants.discard()).toBe(false);
    expect(variants.run).not.toBeNull();
    expect(useUiStore().lastMessage).toContain("busy");
  });

  // ja: パネルは閉じても比較は続き、開き直せる。比較が無ければ開かない
  it("closing the panel keeps the comparison and it can be reopened; nothing opens without a run", async () => {
    setup();
    const variants = useVariantsStore();
    variants.open();
    expect(variants.panelOpen).toBe(false);
    await variants.start("layout", 2);
    variants.close();
    expect(variants.panelOpen).toBe(false);
    expect(variants.run).not.toBeNull();
    variants.open();
    expect(variants.panelOpen).toBe(true);
  });

  // ja: 指標の取り直しで取れなかった案は「指標なし」になる
  it("refreshing metrics leaves a variant without metrics when its sheet cannot be measured", async () => {
    setup();
    const variants = useVariantsStore();
    await variants.start("layout", 2);
    vi.spyOn(ipc, "getTidyMetrics").mockImplementation(async (sheetId) => {
      if (sheetId === COPY_B) throw new Error("gone");
      return metrics(0);
    });
    await variants.refreshMetrics();
    expect(variants.run?.variants[0].metrics).toEqual(metrics(0));
    expect(variants.run?.variants[1].metrics).toBeNull();
  });
});
