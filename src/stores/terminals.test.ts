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

  // ja: 「図面へジャンプ」はエディタを閉じてから図面を出す(全画面モーダルの下にジャンプ先が隠れないように)
  it("closes the editor before revealing the drawing so the jump is not hidden behind the modal", async () => {
    mockBackend();
    const store = await opened();
    let openWhenRevealed: boolean | null = null;
    store.gotoTerminal(() => {
      openWhenRevealed = store.open;
    });
    expect(openWhenRevealed).toBe(false);
    expect(store.open).toBe(false);
  });

  // ja: ジャンプで閉じたエディタを開き直すと、同じ端子台と同じ行選択に戻る
  it("restores the same block and row selection when the editor is reopened after a jump", async () => {
    mockBackend({ chart: wiredChart([["1", "2"]]) });
    vi.spyOn(ipc, "listTerminalBlocks").mockResolvedValue([
      { ...block, entity_id: "tb0", reference: "TB0" },
      block,
    ]);
    const store = await opened("1-2");
    await store.selectBlock("tb1");
    store.toggleRow("2");
    store.gotoTerminal(() => {});

    // 開き直す (シートを渡す経路は同じ。覚え書きがあるのでそちらが勝つ)
    await store.openFor("s1");
    expect(store.entityId).toBe("tb1");
    expect(store.selected).toEqual(["2"]);
    // 覚え書きは使い切るので、次に開くときは先頭の端子台に戻る
    store.close();
    await store.openFor("s1");
    expect(store.entityId).toBe("tb0");
    expect(store.selected).toEqual([]);
  });

  // ja: ×やスクリムで閉じたときは覚え書きを残さない(次に開くと先頭の端子台・選択なし)
  it("keeps no resume memo when the editor is closed by hand", async () => {
    mockBackend();
    const store = await opened();
    store.toggleRow("1");
    store.close();
    expect(store.resume).toBeNull();
    await store.openFor("s1");
    expect(store.selected).toEqual([]);
  });

  // ja: 並べ替えを変えたときの読み直しは1回だけ(自前の読み直しと図面のrevision通知で二重に取りに行かない)
  it("reloads the list and chart only once per sort change, not twice", async () => {
    mockBackend();
    const store = await opened();
    const listed = () => vi.mocked(ipc.listTerminalBlocks).mock.calls.length;
    const charted = () => vi.mocked(ipc.getTerminalChart).mock.calls.length;
    const [list0, chart0] = [listed(), charted()];
    // TerminalEditorDialog の doc.revision 監視は、この同じコマンドで refresh() を呼ぶ
    vi.spyOn(ipc, "executeCommand").mockImplementation(async () => {
      void store.refresh();
      return { revision: 2, ops: [] };
    });
    await store.setRowOrder("alnum");
    expect(listed() - list0).toBe(1);
    expect(charted() - chart0).toBe(1);
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

describe("renumber popover", () => {
  // ja: プレビューは旧番号→新番号の対応として並び、多段端子台では段記号を保ったまま出る
  it("previews the renumbering as an old-to-new mapping and keeps level letters", async () => {
    mockBackend();
    const store = await opened();
    expect(store.renumberMapping(11, 1, "")).toEqual([
      { from: "1", to: "11" },
      { from: "2", to: "12" },
      { from: "3", to: "13" },
      { from: "4", to: "14" },
    ]);
    expect(store.renumberIssue(11, 1, "")).toBeNull();
  });

  // ja: 増分0の採番は全端子を同じ番号にしてしまうので、重複するラベルと件数を理由として返す
  it("refuses a step of zero and names the label that would repeat", async () => {
    mockBackend();
    const store = await opened();
    expect(store.renumberIssue(11, 0, "")).toEqual({ code: "duplicate", label: "11", count: 4 });
  });

  // ja: 適用できない案はコマンドを1本も送らずに断られる (図面は変わらない)
  it("sends no command at all for a plan it cannot apply", async () => {
    mockBackend();
    const store = await opened();
    const sent = spyCommands();
    expect(await store.renumberTerminals(11, 0, "")).toBe(false);
    expect(sent).toEqual([]);
  });

  // ja: 負の開始番号・小数の増分は0以上の整数へ丸めてから送る (JSONの型エラーにしない)
  it("rounds a negative start and a fractional step into whole numbers before sending", async () => {
    mockBackend();
    const store = await opened();
    expect(store.renumberCommand(-5, 1.7, "")).toEqual({
      type: "renumber_terminals",
      sheet_id: "s1",
      entity_id: "tb1",
      start: 0,
      step: 1,
      prefix: "",
    });
    expect(store.renumberPreview(-5, 1.7, "")).toEqual(["0", "1", "2", "3"]);
  });

  // ja: 端子台が1つも無ければ採番の対象が無いことを理由として返す
  it("reports that there is nothing to renumber on a sheet without a terminal block", async () => {
    vi.spyOn(ipc, "listTerminalBlocks").mockResolvedValue([]);
    vi.spyOn(ipc, "getTerminalChart").mockResolvedValue(null);
    const store = await opened();
    expect(store.renumberIssue(1, 1, "")).toEqual({ code: "noBlock" });
  });

  // ja: プレビューは採番に追従したあとのジャンパ指定も見せる (適用と同じ書き換え規則)
  it("previews the jumper specification as it will read after the renumbering", async () => {
    vi.spyOn(ipc, "listTerminalBlocks").mockResolvedValue([{ ...block, jumpers: "1-2,3-4" }]);
    vi.spyOn(ipc, "getTerminalChart").mockResolvedValue(wiredChart([["1", "2"]]));
    vi.spyOn(ipc, "checkTerminalBlock").mockResolvedValue([]);
    const store = await opened("1-2,3-4");
    expect(store.renumberJumperPreview(11, 1, "")).toEqual({
      from: "1-2,3-4",
      to: "11-12,13-14",
    });
  });

  // ja: ジャンパが1本も無ければ追従の行は出さない
  it("shows no jumper line when the block carries no jumpers", async () => {
    mockBackend();
    const store = await opened();
    expect(store.renumberJumperPreview(11, 1, "")).toBeNull();
  });
});

describe("multi-level stepper (Task 3: 多段端子の生成・解除)", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.restoreAllMocks();
  });

  // ja: 段数は一覧の`levels`をそのまま見せ、単段は1になる
  it("reports the block's current level count", async () => {
    mockBackend();
    const store = await opened();
    expect(store.levels).toBe(1);
  });

  // ja: 一覧より先にチャートだけが読めているときは、行の段記号の種類数から段数を数える
  it("falls back to counting distinct level letters in the chart rows", () => {
    const store = useTerminalsStore();
    store.chart = chart([row("1a", { level: "a" }), row("1b", { level: "b" })]);
    expect(store.block).toBe(null);
    expect(store.levels).toBe(2);
  });

  // ja: 段数を変えると、set_terminal_levelsコマンド1回だけが送られる
  it("sends one set_terminal_levels command with the requested level count", async () => {
    mockBackend();
    const sent = spyCommands();
    const store = await opened();
    expect(await store.setLevels(2)).toBe(true);
    expect(sent).toEqual([{ type: "set_terminal_levels", sheet_id: "s1", entity_id: "tb1", levels: 2 }]);
  });

  // ja: 今と同じ段数を指定したら何もコマンドを送らない (無駄なundoの段を作らない)
  it("sends nothing when the requested level count matches the current one", async () => {
    mockBackend();
    const sent = spyCommands();
    const store = await opened();
    expect(await store.setLevels(1)).toBe(true);
    expect(sent).toEqual([]);
  });

  // ja: 1〜TERMINAL_LEVEL_MAXの範囲外は、コマンドを送らずに断る (ステッパーの+/-ボタンが外へ出さないための門と同じ判定)
  it("refuses an out-of-range level count without sending a command", async () => {
    mockBackend();
    const sent = spyCommands();
    const store = await opened();
    expect(await store.setLevels(0)).toBe(false);
    expect(await store.setLevels(4)).toBe(false);
    expect(sent).toEqual([]);
  });

  // ja: Rust側が段数変更を拒否したら (消える段に参照が残っている等)、その理由を`error`へ残す (図面は変えない)
  it("keeps the backend's refusal reason in error and leaves the drawing untouched", async () => {
    mockBackend();
    const store = await opened();
    vi.spyOn(ipc, "executeCommand").mockRejectedValue(new Error("段bに結線が残っています"));
    expect(await store.setLevels(2)).toBe(false);
    expect(store.error).toContain("段bに結線が残っています");
  });

  /** 段数ポップオーバー用: symbol_id・attrs・チャート行・一覧の段数を差し替えたTB1を開く。 */
  async function openedLevels(
    symbolId: string,
    attrs: Record<string, string>,
    rows: TerminalRow[],
    levels: number,
  ) {
    const p = project();
    const tb = p.sheets[0].entities.tb1 as { symbol_id: string; attrs: Record<string, string> };
    tb.symbol_id = symbolId;
    tb.attrs = attrs;
    useDocumentStore().project = p;
    vi.spyOn(ipc, "listTerminalBlocks").mockResolvedValue([
      { ...block, levels, terminal_count: rows.length, jumpers: attrs.jumpers ?? "" },
    ]);
    vi.spyOn(ipc, "getTerminalChart").mockResolvedValue(chart(rows));
    vi.spyOn(ipc, "checkTerminalBlock").mockResolvedValue([]);
    const store = useTerminalsStore();
    await store.openFor("s1");
    return store;
  }

  // ja: プレビューは適用と同じ純関数で symbol_id・端子番号・ジャンパの追従を見せる (デザイン「P 段数 (既定)」の3行)
  it("previews the new symbol id, the new terminal numbers and the jumpers that follow", async () => {
    const store = await openedLevels("terminal_block_4p", { jumpers: "1-2" }, [row("1"), row("2"), row("3"), row("4")], 1);
    expect(store.levelsPreview(2)).toEqual({
      symbolFrom: "terminal_block_4p",
      symbolTo: "terminal_block_4p_2l",
      numbersFrom: "1,2,…",
      numbersTo: "1a,1b,2a,2b,…",
      jumpers: { from: "1-2", to: "1a-2a" },
    });
  });

  // ja: 今と同じ段数のプレビューは出さない (何も変わらないので)
  it("shows no preview for the level count the block already has", async () => {
    const store = await openedLevels("terminal_block_4p", {}, [row("1"), row("2"), row("3"), row("4")], 1);
    expect(store.levelsPreview(1)).toBeNull();
  });

  // ja: 段を減らせないときは、消える段の端子とその結線相手を理由付きで並べる (デザイン「P 段数 (解除不可)」の囲み)
  it("lists each vanishing terminal that still has a connection, with its target", async () => {
    const store = await openedLevels(
      "terminal_block_4p_2l",
      {},
      [
        row("1a", { level: "a" }),
        row("1b", { level: "b", internal: "K2:A2", spare: false }),
        ...["2a", "2b", "3a", "3b", "4a", "4b"].map((t) => row(t, { level: t.slice(-1) })),
      ],
      2,
    );
    expect(store.levelsBlockers(1)).toEqual([
      { terminal: "1b", connection: "K2:A2", reason: "wire" },
    ]);
  });

  // ja: 消える段を指すジャンパも、結線と同じように解除を止める理由として並ぶ
  it("lists a jumper that still names a vanishing level as a blocker", async () => {
    const store = await openedLevels(
      "terminal_block_4p_2l",
      { jumpers: "1a-1b" },
      ["1a", "1b", "2a", "2b", "3a", "3b", "4a", "4b"].map((t) => row(t, { level: t.slice(-1) })),
      2,
    );
    expect(store.levelsBlockers(1)).toEqual([
      { terminal: "1b", connection: "", reason: "jumper" },
    ]);
  });

  // ja: 段を増やすときは消える段が無いので、理由の一覧は常に空 (適用ボタンは有効のまま)
  it("never blocks an increase in the level count", async () => {
    const store = await openedLevels("terminal_block_4p", { jumpers: "1-2" }, [row("1", { spare: false })], 1);
    expect(store.levelsBlockers(2)).toEqual([]);
  });

  // ja: 理由が1件でもある段数はコマンドを送らずに断る (ポップオーバーが適用ボタンを無効にしている門と同じ判定)
  it("refuses a level decrease that has a blocker without sending a command", async () => {
    const store = await openedLevels(
      "terminal_block_4p_2l",
      { jumpers: "1a-1b" },
      ["1a", "1b", "2a", "2b", "3a", "3b", "4a", "4b"].map((t) => row(t, { level: t.slice(-1) })),
      2,
    );
    const sent = spyCommands();
    expect(await store.setLevels(1)).toBe(false);
    expect(sent).toEqual([]);
  });
});

describe("spare insertion popover (Task 4: 予備端子の挿入)", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.restoreAllMocks();
  });

  /** 予備挿入ポップオーバー用: symbol_id・attrs・チャート行・一覧の極数を差し替えたTB1を開く。 */
  async function openedSpare(
    symbolId: string,
    attrs: Record<string, string>,
    rows: TerminalRow[],
    poles: number,
  ) {
    const p = project();
    const tb = p.sheets[0].entities.tb1 as { symbol_id: string; attrs: Record<string, string> };
    tb.symbol_id = symbolId;
    tb.attrs = attrs;
    useDocumentStore().project = p;
    vi.spyOn(ipc, "listTerminalBlocks").mockResolvedValue([
      { ...block, poles, terminal_count: rows.length, jumpers: attrs.jumpers ?? "" },
    ]);
    vi.spyOn(ipc, "getTerminalChart").mockResolvedValue(chart(rows));
    vi.spyOn(ipc, "checkTerminalBlock").mockResolvedValue([]);
    const store = useTerminalsStore();
    await store.openFor("s1");
    return store;
  }

  // ja: 予備を挿入すると、insert_spare_terminalsコマンド1回だけが送られる
  it("sends one insert_spare_terminals command with the requested position and count", async () => {
    mockBackend();
    const sent = spyCommands();
    const store = await opened();
    expect(await store.insertSpareTerminals("3", 1)).toBe(true);
    expect(sent).toEqual([
      {
        type: "insert_spare_terminals",
        sheet_id: "s1",
        entity_id: "tb1",
        after: "3",
        count: 1,
        renumber_following: true,
      },
    ]);
  });

  // ja: 末尾挿入は`after`がnullのコマンドになる
  it("sends null for after when inserting at the end", async () => {
    mockBackend();
    const sent = spyCommands();
    const store = await opened();
    expect(await store.insertSpareTerminals(null, 1)).toBe(true);
    expect(sent).toEqual([
      {
        type: "insert_spare_terminals",
        sheet_id: "s1",
        entity_id: "tb1",
        after: null,
        count: 1,
        renumber_following: true,
      },
    ]);
  });

  // ja: 「空き番号のまま残す」で末尾に挿入すると、後ろの番号を変えずに次の空き番号 (9) が予備に付き、renumber_following=false で送られる
  it("keeps following labels and gives the spare the next free number when 'leave vacant' is chosen at the end", async () => {
    const sent = spyCommands();
    const store = await openedSpare("terminal_block_8p", {}, [1, 2, 3, 4, 5, 6, 7, 8].map((n) => row(String(n))), 8);
    expect(store.sparePreview(null, 1, false)).toEqual({
      symbolFrom: "terminal_block_8p",
      symbolTo: "terminal_block_9p",
      spareLabels: ["9"],
    });
    expect(await store.insertSpareTerminals(null, 1, false)).toBe(true);
    expect(sent.map((c) => (c as { renumber_following?: boolean }).renumber_following)).toEqual([false]);
  });

  // ja: 「空き番号のまま残す」で途中に挿入して番号がぶつかるときは keepDuplicate の指摘になり、コマンドは送られない
  it("reports keepDuplicate and sends nothing when 'leave vacant' would collide with an existing number", async () => {
    const sent = spyCommands();
    const store = await openedSpare("terminal_block_8p", {}, [1, 2, 3, 4, 5, 6, 7, 8].map((n) => row(String(n))), 8);
    expect(store.spareIssue("3", 1, false)).toEqual({ code: "keepDuplicate" });
    expect(await store.insertSpareTerminals("3", 1, false)).toBe(false);
    expect(sent).toEqual([]);
  });

  // ja: 0個以下の指定は、コマンドを送らずに断る
  it("refuses a non-positive count without sending a command", async () => {
    mockBackend();
    const sent = spyCommands();
    const store = await opened();
    expect(await store.insertSpareTerminals(null, 0)).toBe(false);
    expect(sent).toEqual([]);
  });

  // ja: フロントの検証を通っても、Rust側が挿入を拒否したら (エンティティが図面から消えていた等) その理由をerrorへ残す (図面は変えない)
  it("keeps the backend's refusal reason in error and leaves the drawing untouched", async () => {
    mockBackend();
    const store = await opened();
    vi.spyOn(ipc, "executeCommand").mockRejectedValue(new Error("端子台が見つかりません"));
    expect(await store.insertSpareTerminals(null, 1)).toBe(false);
    expect(store.error).toContain("端子台が見つかりません");
  });

  // ja: プレビューは適用と同じ純関数で symbol_id と新しく増える予備のラベルを見せる (デザイン「P 予備端子の挿入 (既定)」の2行)
  it("previews the new symbol id and the new spare's labels", async () => {
    const store = await openedSpare("terminal_block_8p", {}, [1, 2, 3, 4, 5, 6, 7, 8].map((n) => row(String(n))), 8);
    expect(store.sparePreview(null, 1)).toEqual({
      symbolFrom: "terminal_block_8p",
      symbolTo: "terminal_block_9p",
      spareLabels: ["9"],
    });
    expect(store.sparePreview("3", 1)).toEqual({
      symbolFrom: "terminal_block_8p",
      symbolTo: "terminal_block_9p",
      spareLabels: ["4"],
    });
  });

  // ja: 0個以下の指定は指摘 (適用ボタンを無効にする)
  it("flags a non-positive count as an issue", async () => {
    const store = await openedSpare("terminal_block_8p", {}, [row("1")], 8);
    expect(store.spareIssue(null, 0)).toEqual({ code: "count" });
  });

  // ja: 極数の上限を超える指定も指摘になる
  it("flags a count that would exceed the pole cap", async () => {
    const store = await openedSpare(
      `terminal_block_${50}p`,
      {},
      Array.from({ length: 50 }, (_, i) => row(String(i + 1))),
      50,
    );
    expect(store.spareIssue(null, 1)).toEqual({ code: "poleCap", max: 50 });
  });

  // ja: 挿入位置の選択肢は極ごとに1つ、多段端子台では段aのラベルを代表にする
  it("lists one insertion-point option per pole, using level a's label on a multi-level block", async () => {
    const store = await openedSpare(
      "terminal_block_2p_2l",
      {},
      ["1a", "1b", "2a", "2b"].map((t) => row(t, { level: t.slice(-1) })),
      2,
    );
    expect(store.spareAfterOptions).toEqual(["1a", "2a"]);
  });

  // ja: 個数ステッパーの入力は1以上の整数へ丸める
  it("clamps the count stepper's input to a positive integer", async () => {
    const store = await opened();
    expect(store.clampSpareCount(0)).toBe(1);
    expect(store.clampSpareCount(-3)).toBe(1);
    expect(store.clampSpareCount(2.7)).toBe(2);
    expect(store.clampSpareCount(NaN)).toBe(1);
  });
});
