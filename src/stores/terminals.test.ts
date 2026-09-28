// 端子台エディタの仕様テスト (docs/internal/specs/m4-industrial-core.md §1.1)。
import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import {
  ipc,
  type Command,
  type Diagnostic,
  type Project,
  type RowOrder,
  type TerminalBlockInfo,
  type TerminalChart,
  type TerminalRow,
} from "../ipc";
import { useDocumentStore } from "./document";
import type { RevealTarget } from "../composables/reveal";
import {
  parseJumperSpec,
  formatJumperSpec,
  withJumpers,
  withoutJumpers,
  ROW_ORDERS,
  rowOrderOf,
  useTerminalsStore,
} from "./terminals";

/** TB1 (4極) が1つだけ載ったプロジェクト。ジャンパは`spec`。 */
function project(spec = ""): Project {
  return {
    format_version: 1,
    name: "P",
    wire_parts: [],
    sheets: [
      {
        id: "s1",
        name: "Sheet1",
        size: "A3",
        orientation: "Landscape",
        zone_cols: 4,
        zone_rows: 6,
        title_block: {},
        revisions: [],
        entities: {
          tb1: {
            kind: "symbol",
            id: "tb1",
            symbol_id: "terminal_block_4p",
            at: { x: 100, y: 100 },
            rotation: 0,
            mirror: false,
            reference: "TB1",
            value: "UK 2.5N",
            attrs: spec ? { jumpers: spec } : {},
          },
        },
      },
    ],
  } as unknown as Project;
}

function row(terminal: string, extra: Partial<TerminalRow> = {}): TerminalRow {
  return {
    terminal,
    level: "",
    internal: "",
    external: "",
    wire_no: "",
    wire: "",
    internal_wire: "",
    external_wire: "",
    internal_harness: "",
    external_harness: "",
    jumper: "",
    spare: true,
    ...extra,
  };
}

function chart(
  rows: TerminalRow[],
  jumpers: [string, string][] = [],
  rowOrder: RowOrder = "number",
): TerminalChart {
  return {
    entity_id: "tb1",
    reference: "TB1",
    value: "UK 2.5N",
    sheet_name: "Sheet1",
    terminal_count: rows.length,
    rows,
    jumpers,
    jumper_issues: [],
    row_order: rowOrder,
  };
}

/** 図面どおりの4極チャート (端子1・2は結線済み、3・4は予備)。 */
function wiredChart(jumpers: [string, string][] = []): TerminalChart {
  return chart(
    [
      row("1", {
        internal: "K1:A1",
        external: "LS1:1",
        wire_no: "12",
        wire: "BK 0.75sq KIV-0.75-BK",
        internal_harness: "",
        external_harness: "W1",
        jumper: jumpers.length ? "1-2" : "",
        spare: false,
      }),
      row("2", {
        internal: "K1:A2",
        external: "LS2:1",
        wire_no: "13",
        wire: "BK 0.75sq KIV-0.75-BK",
        jumper: jumpers.length ? "1-2" : "",
        spare: false,
      }),
      row("3"),
      row("4"),
    ],
    jumpers,
  );
}

const block: TerminalBlockInfo = {
  entity_id: "tb1",
  sheet_id: "s1",
  sheet_name: "Sheet1",
  reference: "TB1",
  value: "UK 2.5N",
  terminal_count: 4,
  poles: 4,
  levels: 1,
  jumpers: "",
};

/** 端子台の一覧・チャート・チェックの応答を差し替える。 */
function mockBackend(options: { chart?: TerminalChart; diagnostics?: Diagnostic[] } = {}) {
  vi.spyOn(ipc, "listTerminalBlocks").mockResolvedValue([block]);
  vi.spyOn(ipc, "getTerminalChart").mockResolvedValue(options.chart ?? wiredChart());
  vi.spyOn(ipc, "checkTerminalBlock").mockResolvedValue(options.diagnostics ?? []);
}

/** ストアが送ったコマンドを記録する。 */
function spyCommands(): Command[] {
  const sent: Command[] = [];
  vi.spyOn(ipc, "executeCommand").mockImplementation(async (command) => {
    sent.push(command);
    return { revision: 2, ops: [] };
  });
  return sent;
}

/** TB1を開いた状態のストア。 */
async function opened(spec = "") {
  useDocumentStore().project = project(spec);
  const store = useTerminalsStore();
  await store.openFor("s1");
  return store;
}

describe("jumper specification", () => {
  // ja: ジャンパ指定は「小さい端子番号が先・昇順・重複なし」に正規化される
  it("normalizes a jumper list to ascending, low-first, duplicate-free pairs", () => {
    expect(parseJumperSpec("3-2,1-2,2-1")).toEqual([
      ["1", "2"],
      ["2", "3"],
    ]);
    expect(formatJumperSpec(parseJumperSpec("3-2,1-2,2-1"))).toBe("1-2,2-3");
  });

  // ja: 壊れた記述と隣り合わない端子のジャンパは読み飛ばす
  it("drops malformed entries and jumpers between terminals that are not neighbours", () => {
    expect(parseJumperSpec("1-3")).toEqual([]);
    expect(parseJumperSpec("abc, ,1-,1-2")).toEqual([["1", "2"]]);
  });

  // ja: 隣り合う端子を選んでジャンパを掛けると既存のジャンパは残る
  it("adds a jumper for adjacent terminals while keeping the existing ones", () => {
    expect(withJumpers("3-4", ["1", "2"])).toBe("1-2,3-4");
  });

  // ja: 3つ以上の端子を選ぶと隣どうしを順につないだジャンパになる
  it("chains jumpers along a run of three or more adjacent terminals", () => {
    expect(withJumpers("", ["1", "2", "3"])).toBe("1-2,2-3");
  });

  // ja: 隣り合わない端子どうしにはジャンパを掛けられない
  it("refuses to jumper terminals that are not adjacent", () => {
    expect(withJumpers("", ["1", "3"])).toBeNull();
    expect(withJumpers("", ["2"])).toBeNull();
    expect(withJumpers("", [])).toBeNull();
  });

  // ja: ジャンパ削除は選んだ端子に掛かっているものだけを外す
  it("removes only the jumpers that touch the selected terminals", () => {
    expect(withoutJumpers("1-2,3-4", ["3"])).toBe("1-2");
    expect(withoutJumpers("1-2,2-3,3-4", ["2"])).toBe("3-4");
    expect(withoutJumpers("1-2", ["4"])).toBe("1-2");
  });
});

describe("row order (parity with madake-core terminal_chart::RowOrder)", () => {
  // ja: 5つの並べ替え順はRust側`RowOrder`のバリアントと同じ5つ、同じ表記
  it("lists the same five row orders as the Rust RowOrder enum", () => {
    expect(ROW_ORDERS).toEqual(["number", "alnum", "page", "cable", "jumper"]);
  });

  // ja: 未設定・未知語は数値順に落ちる(Rust側 row_order_of と同じ既定)
  it("falls back to number order for an unset or unrecognised attribute", () => {
    expect(rowOrderOf(undefined)).toBe("number");
    expect(rowOrderOf({})).toBe("number");
    expect(rowOrderOf({ row_order: "bogus" })).toBe("number");
  });

  // ja: 5つの既知の語はそのまま対応する並びに解決される
  it("resolves every known word to its row order", () => {
    for (const order of ROW_ORDERS) expect(rowOrderOf({ row_order: order })).toBe(order);
  });
});

describe("terminal strip editor store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.restoreAllMocks();
  });

  // ja: 開くとそのシートの端子台一覧を読み込み、先頭の端子台のチャートを表示する
  it("loads the terminal blocks of the sheet and shows the first one", async () => {
    mockBackend();
    const store = await opened();
    expect(store.open).toBe(true);
    expect(store.blocks).toHaveLength(1);
    expect(store.entityId).toBe("tb1");
    expect(store.reference).toBe("TB1");
    expect(store.terminalCount).toBe(4);
  });

  // ja: グリッドは端子番号ごとに1行で、外部側・内部側・線番・電線が図面どおりに並ぶ
  it("shows one grid row per terminal with the wiring taken from the drawing", async () => {
    mockBackend();
    const store = await opened();
    expect(store.rows).toHaveLength(4);
    expect(store.rows[0]).toMatchObject({
      terminal: "1",
      external: "LS1:1",
      externalCable: "W1",
      internal: "K1:A1",
      wireNo: "12",
      wire: "BK 0.75sq KIV-0.75-BK",
      jumper: "",
      spare: false,
      selected: false,
    });
  });

  // ja: グリッドの行は段記号(Lv)も運ぶ(単段は空欄、多段端子台は"a"/"b"のような文字)
  it("carries the level letter of each row for multi-level terminal blocks", async () => {
    vi.spyOn(ipc, "listTerminalBlocks").mockResolvedValue([{ ...block, levels: 2 }]);
    vi.spyOn(ipc, "getTerminalChart").mockResolvedValue(
      chart([row("1a", { level: "a" }), row("1b", { level: "b" })]),
    );
    vi.spyOn(ipc, "checkTerminalBlock").mockResolvedValue([]);
    const store = await opened();
    expect(store.rows.map((r) => r.level)).toEqual(["a", "b"]);
    expect(store.hasLevels).toBe(true);
  });

  // ja: 端子台の一覧より先にチャートだけが読めているときは、行の段記号からLv列の要否を判定する
  it("falls back to the chart rows to decide whether the level column is needed", () => {
    const store = useTerminalsStore();
    store.chart = chart([row("1a", { level: "a" }), row("1b", { level: "b" })]);
    expect(store.block).toBe(null);
    expect(store.hasLevels).toBe(true);
  });

  // ja: 単段の端子台ではLv列は空欄で、多段端子台とは判定される
  it("reports no levels for a single-level terminal block", async () => {
    mockBackend();
    const store = await opened();
    expect(store.rows.every((r) => r.level === "")).toBe(true);
    expect(store.hasLevels).toBe(false);
  });

  // ja: 電線が1本も繋がっていない端子は予備端子として印が付く
  it("marks terminals with no wire at all as spare", async () => {
    mockBackend();
    const store = await opened();
    expect(store.rows.filter((r) => r.spare).map((r) => r.terminal)).toEqual(["3", "4"]);
    expect(store.spareCount).toBe(2);
  });

  // ja: 隣り合う2端子を選んでジャンパを生成すると、update_entityコマンド1回で属性に書き込まれる
  it("writes a jumper into the symbol attributes with one update_entity command", async () => {
    mockBackend();
    const sent = spyCommands();
    const store = await opened();
    store.toggleRow("1");
    store.toggleRow("2");
    expect(store.canAddJumper).toBe(true);
    await store.addJumper();
    expect(sent).toHaveLength(1);
    expect(sent[0]).toMatchObject({
      type: "update_entity",
      sheet_id: "s1",
      entity: { id: "tb1", kind: "symbol", attrs: { jumpers: "1-2" } },
    });
  });

  // ja: 多段端子台の見出しには端子数ではなく極数を使う(4極2段は「8極」ではなく「4極」)
  it("names a terminal block by its pole count, not by its terminal count", async () => {
    vi.spyOn(ipc, "listTerminalBlocks").mockResolvedValue([
      { ...block, poles: 4, levels: 2, terminal_count: 8 },
    ]);
    vi.spyOn(ipc, "getTerminalChart").mockResolvedValue({ ...wiredChart(), terminal_count: 8 });
    vi.spyOn(ipc, "checkTerminalBlock").mockResolvedValue([]);
    const store = await opened();
    expect(store.poleCount).toBe(4);
    expect(store.terminalCount).toBe(8);
  });

  // ja: 端子台の一覧より先にチャートだけが読めているときは、極数をチャートの行から数える(4極2段のチャートなら「4極」)
  it("counts the pole count from the chart rows while the block list is not loaded yet", () => {
    const store = useTerminalsStore();
    store.chart = chart(["1a", "1b", "2a", "2b", "3a", "3b", "4a", "4b"].map((no) => row(no)));
    expect(store.block).toBe(null);
    expect(store.poleCount).toBe(4);
    expect(store.terminalCount).toBe(8);
  });

  // ja: チャートも端子台の一覧も読めていなければ極数は0極になる
  it("reports no poles while neither the block list nor the chart is loaded", () => {
    expect(useTerminalsStore().poleCount).toBe(0);
  });

  // ja: 端子を選んでいないとジャンパは生成できない
  it("cannot generate a jumper while no terminal is selected", async () => {
    mockBackend();
    const sent = spyCommands();
    const store = await opened();
    expect(store.canAddJumper).toBe(false);
    await store.addJumper();
    expect(sent).toEqual([]);
  });

  // ja: 隣り合わない端子を選んでもジャンパは生成できない
  it("cannot generate a jumper between terminals that are not adjacent", async () => {
    mockBackend();
    const sent = spyCommands();
    const store = await opened();
    store.toggleRow("1");
    store.toggleRow("3");
    expect(store.canAddJumper).toBe(false);
    await store.addJumper();
    expect(sent).toEqual([]);
  });

  // ja: ジャンパ削除は選んだ端子に掛かるジャンパだけを外したコマンドになる
  it("clears only the jumpers on the selected terminals", async () => {
    mockBackend({
      chart: wiredChart([
        ["1", "2"],
        ["3", "4"],
      ]),
    });
    const sent = spyCommands();
    const store = await opened("1-2,3-4");
    store.toggleRow("1");
    expect(store.canRemoveJumper).toBe(true);
    await store.removeJumper();
    expect(sent[0]).toMatchObject({
      type: "update_entity",
      entity: { id: "tb1", attrs: { jumpers: "3-4" } },
    });
  });

  // ja: ジャンパが1本も残らないときは属性ごと消す
  it("drops the jumper attribute entirely once no jumper is left", async () => {
    mockBackend({ chart: wiredChart([["1", "2"]]) });
    const sent = spyCommands();
    const store = await opened("1-2");
    store.toggleRow("2");
    await store.removeJumper();
    const entity = (sent[0] as { entity: { attrs: Record<string, string> } }).entity;
    expect(entity.attrs.jumpers).toBeUndefined();
  });

  // ja: ジャンパを編集したらチャートを読み直し、グリッドが図面と一致し続ける
  it("reloads the chart after editing a jumper so the grid keeps matching the model", async () => {
    mockBackend();
    spyCommands();
    const store = await opened();
    vi.spyOn(ipc, "getTerminalChart").mockResolvedValue(wiredChart([["1", "2"]]));
    store.toggleRow("1");
    store.toggleRow("2");
    await store.addJumper();
    expect(store.rows[0].jumper).toBe("1-2");
    expect(store.selected).toEqual([]);
  });

  // ja: 並べ替え順を変えると、update_entityコマンド1回で属性(row_order)に書き込まれる
  it("changes the row order with one update_entity command", async () => {
    mockBackend();
    const sent = spyCommands();
    const store = await opened();
    expect(store.rowOrder).toBe("number");
    await store.setRowOrder("alnum");
    expect(sent).toHaveLength(1);
    expect(sent[0]).toMatchObject({
      type: "update_entity",
      sheet_id: "s1",
      entity: { id: "tb1", kind: "symbol", attrs: { row_order: "alnum" } },
    });
  });

  // ja: 既定の数値順を選び直すと、row_order属性ごと消す(未設定の端子台と同じ状態に戻す)
  it("drops the row_order attribute entirely when number order is chosen again", async () => {
    mockBackend();
    const sent = spyCommands();
    const store = await opened("");
    const tb1 = useDocumentStore().project!.sheets[0].entities.tb1 as unknown as {
      attrs: Record<string, string>;
    };
    tb1.attrs = { row_order: "alnum" };
    await store.setRowOrder("number");
    const entity = (sent[0] as { entity: { attrs: Record<string, string> } }).entity;
    expect(entity.attrs.row_order).toBeUndefined();
  });

  // ja: 現在のチャートの並べ替え順をそのまま見せる(サーバ側で並べ替え済み)
  it("reflects the row order the chart already came back sorted in", async () => {
    mockBackend({ chart: chart([row("1"), row("2")], [], "page") });
    const store = await opened();
    expect(store.rowOrder).toBe("page");
  });

  // ja: 「図面へジャンプ」は表示中の端子台のシート・entity idをrevealへそのまま渡す
  it("jumps to the drawing by passing the sheet and entity id of the current block to reveal", async () => {
    mockBackend();
    const store = await opened();
    const targets: RevealTarget[] = [];
    store.gotoTerminal((t) => targets.push(t));
    expect(targets).toEqual([{ sheetId: "s1", entityIds: ["tb1"] }]);
  });

  // ja: 端子台が無ければ「図面へジャンプ」は何もしない
  it("does nothing when asked to jump to the drawing without a terminal block", () => {
    const store = useTerminalsStore();
    const targets: RevealTarget[] = [];
    store.gotoTerminal((t) => targets.push(t));
    expect(targets).toEqual([]);
  });

  // ja: 図面が外から変わったら (undoやAIの編集) グリッドを取り直し、古いチェック結果は捨てる
  it("reloads the grid when the drawing changes from outside the editor, e.g. an undo", async () => {
    mockBackend({ chart: wiredChart([["1", "2"]]) });
    const store = await opened("1-2");
    await store.runCheck();
    store.toggleRow("1");
    expect(store.rows[0].jumper).toBe("1-2");

    // undoでジャンパが消えた図面に差し替わる
    vi.spyOn(ipc, "getTerminalChart").mockResolvedValue(wiredChart());
    await store.refresh();
    expect(store.rows[0].jumper).toBe("");
    expect(store.selected).toEqual([]);
    expect(store.diagnostics).toBeNull();
  });

  // ja: 端子台エディタを閉じている間は図面が変わっても読み込みに行かない
  it("does not fetch anything while the editor is closed", async () => {
    mockBackend();
    const store = useTerminalsStore();
    await store.refresh();
    expect(ipc.listTerminalBlocks).not.toHaveBeenCalled();
  });

  // ja: 端子台チェックの結果は重大度ごとの件数に整形される
  it("summarizes the terminal block check as counts per severity", async () => {
    mockBackend({
      diagnostics: [
        { severity: "error", code: "terminal.jumper_invalid", message: "bad", sheet_id: "s1", entity_ids: ["tb1"] },
        { severity: "info", code: "terminal.unconnected", message: "spare", sheet_id: "s1", entity_ids: ["tb1"] },
        { severity: "info", code: "terminal.unconnected", message: "spare", sheet_id: "s1", entity_ids: ["tb1"] },
      ],
    });
    const store = await opened();
    await store.runCheck();
    expect(store.checkCounts).toEqual({ error: 1, warning: 0, info: 2 });
    expect(store.checkOk).toBe(false);
    expect(store.diagnostics?.[0].severity).toBe("error");
  });

  // ja: 問題が1件も無ければチェックは「問題なし」になる
  it("reports a clean terminal block when the check finds nothing", async () => {
    mockBackend();
    const store = await opened();
    await store.runCheck();
    expect(store.checkCounts).toEqual({ error: 0, warning: 0, info: 0 });
    expect(store.checkOk).toBe(true);
  });

  // ja: 端子台を切り替えると選択と直前のチェック結果は消える
  it("clears the selection and the previous check result when another block is picked", async () => {
    mockBackend();
    const store = await opened();
    await store.runCheck();
    store.toggleRow("1");
    await store.selectBlock("tb1");
    expect(store.selected).toEqual([]);
    expect(store.diagnostics).toBeNull();
  });

  // ja: 端子台が1つも無いシートでは行が空になり、生成もチェックもできない
  it("shows an empty grid on a sheet without any terminal block", async () => {
    vi.spyOn(ipc, "listTerminalBlocks").mockResolvedValue([]);
    vi.spyOn(ipc, "getTerminalChart").mockResolvedValue(null);
    const store = await opened();
    expect(store.rows).toEqual([]);
    expect(store.entityId).toBeNull();
    expect(store.canAddJumper).toBe(false);
  });

  // ja: 閉じると選択・チャート・チェック結果を捨てる
  it("throws away the chart, the selection and the check result when closed", async () => {
    mockBackend();
    const store = await opened();
    store.toggleRow("1");
    store.close();
    expect(store.open).toBe(false);
    expect(store.chart).toBeNull();
    expect(store.selected).toEqual([]);
    expect(store.diagnostics).toBeNull();
  });
});
