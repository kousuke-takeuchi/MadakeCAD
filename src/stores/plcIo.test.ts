// PLC I/O割付表エディタの仕様テスト (docs/internal/specs/m4-industrial-core.md §3)。
import { setActivePinia, createPinia } from "pinia";
import { describe, it, expect, vi, beforeEach } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import {
  ipc,
  type Command,
  type Part,
  type PlcAssignment,
  type PlcModuleInfo,
  type PlcModuleSpec,
  type PlcPoint,
  type Project,
} from "../ipc";
import { useDocumentStore } from "./document";
import { autoAddress, autoAddresses, parseAddressIndex, usePlcIoStore } from "./plcIo";

/** 三菱の16点入力ユニット (X0から8進で数える)。 */
const di16: PlcModuleSpec = {
  points: 16,
  kind: "DI",
  address_prefix: "X",
  address_style: "mitsubishi",
};

/** 部品DBのPLCモジュール1件 (図面のvalue=型番で引き当てる)。 */
const di16Part: Part = {
  part_no: "MDK-PLC-DI16-MITSUBISHI",
  maker: "",
  name: "PLC入力ユニット 16点",
  category: "plc",
  symbol_id: "plc_di_16p",
  rated_voltage: "DC24V",
  currency: "JPY",
  note: "",
  plc_module: JSON.stringify(di16),
} as unknown as Part;

/** 図面に置かれたPLC1 (16点DI)。 */
const module1: PlcModuleInfo = {
  entity_id: "e1",
  sheet_id: "s1",
  sheet_name: "Sheet1",
  reference: "PLC1",
  value: "MDK-PLC-DI16-MITSUBISHI",
  points: 16,
  kind: "DI",
};

function assignment(
  module_ref: string,
  address: string,
  signal_name = "",
  comment = "",
  id = `${module_ref}-${address}`,
): PlcAssignment {
  return { id, module_ref, address, signal_name, comment };
}

/** PLC1が1つ置かれたプロジェクト。割付表は`assignments`。 */
function project(assignments: PlcAssignment[] = []): Project {
  return {
    format_version: 2,
    name: "P",
    wire_parts: [],
    plc_assignments: assignments,
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
          e1: {
            kind: "symbol",
            id: "e1",
            symbol_id: "plc_di_16p",
            at: { x: 100, y: 100 },
            rotation: 0,
            mirror: false,
            reference: "PLC1",
            value: "MDK-PLC-DI16-MITSUBISHI",
            attrs: {},
          },
        },
      },
    ],
  } as unknown as Project;
}

function point(n: number, extra: Partial<PlcPoint> = {}): PlcPoint {
  return { point: n, address: "", signal_name: "", comment: "", target: "", wire_no: "", ...extra };
}

/** バックエンドの応答を差し替える。 */
function mockBackend(
  options: { modules?: PlcModuleInfo[]; parts?: Part[]; points?: PlcPoint[] } = {},
) {
  vi.spyOn(ipc, "listPlcModules").mockResolvedValue(options.modules ?? [module1]);
  vi.spyOn(ipc, "listPlcModuleParts").mockResolvedValue(options.parts ?? [di16Part]);
  vi.spyOn(ipc, "getPlcAssignments").mockResolvedValue(options.points ?? []);
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

/** PLC1を開いた状態のストア。 */
async function opened(assignments: PlcAssignment[] = []) {
  useDocumentStore().project = project(assignments);
  const store = usePlcIoStore();
  await store.openEditor();
  return store;
}

describe("PLC address numbering", () => {
  // ja: 三菱のアドレスは8進で数えるので X7 の次は X10 になる
  it("counts Mitsubishi addresses in octal so X7 is followed by X10", () => {
    expect(autoAddresses(di16, 0).slice(0, 10)).toEqual([
      "X0", "X1", "X2", "X3", "X4", "X5", "X6", "X7", "X10", "X11",
    ]);
  });

  // ja: Siemensのアドレスは「バイト.ビット」(1バイト8点) で数える
  it("writes Siemens addresses as a byte and a bit", () => {
    const spec: PlcModuleSpec = {
      points: 8,
      kind: "DI",
      address_prefix: "%I",
      address_style: "siemens",
    };
    expect(autoAddress(spec, 0)).toBe("%I0.0");
    expect(autoAddress(spec, 9)).toBe("%I1.1");
  });

  // ja: Allen-Bradleyのアドレスは「ワード/ビット」(1ワード16点) で数える
  it("writes Allen-Bradley addresses as a word and a bit", () => {
    const spec: PlcModuleSpec = {
      points: 16,
      kind: "DI",
      address_prefix: "I:",
      address_style: "ab",
    };
    expect(autoAddress(spec, 0)).toBe("I:0/0");
    expect(autoAddress(spec, 17)).toBe("I:1/1");
  });

  // ja: 自動採番の開始アドレスは何点目から始めるかを表す (2枚目のユニットは続き番号から振れる)
  it("starts the automatic numbering from the point the start address names", () => {
    expect(autoAddresses(di16, 16).slice(0, 3)).toEqual(["X20", "X21", "X22"]);
    expect(parseAddressIndex(di16, "X20")).toBe(16);
    expect(parseAddressIndex(di16, "x0")).toBe(0);
  });

  // ja: アドレス体系に合わない開始アドレスは受け付けない (採番できないため)
  it("refuses a start address that does not match the address style of the module", () => {
    expect(parseAddressIndex(di16, "X8")).toBeNull();
    expect(parseAddressIndex(di16, "Y0")).toBeNull();
    expect(parseAddressIndex(di16, "")).toBeNull();
  });
});

describe("PLC I/O assignment editor", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.restoreAllMocks();
  });

  // ja: 開くと図面に置かれているPLCモジュールを読み込み、先頭のモジュールを表示する
  it("loads the PLC modules placed in the drawing and shows the first one", async () => {
    mockBackend();
    const store = await opened();
    expect(store.open).toBe(true);
    expect(store.modules).toHaveLength(1);
    expect(store.moduleRef).toBe("PLC1");
    expect(store.spec).toEqual(di16);
  });

  // ja: グリッドはモジュールの点数ぶんの行になり、未割付の行にも自動採番の候補アドレスが出る
  it("shows one grid row per I/O point with the address it would get from the numbering", async () => {
    mockBackend();
    const store = await opened();
    expect(store.rows).toHaveLength(16);
    expect(store.rows[0]).toMatchObject({ point: 1, address: "", autoAddress: "X0", signalName: "" });
    expect(store.rows[8].autoAddress).toBe("X10");
  });

  // ja: 割付表に保存済みの行はアドレス・信号名・コメントがそのまま表示される
  it("shows the address, the signal name and the comment of the rows already saved", async () => {
    mockBackend({
      points: [point(1, { address: "X0", signal_name: "非常停止入力", comment: "NC接点" })],
    });
    const store = await opened([assignment("PLC1", "X0", "非常停止入力", "NC接点")]);
    expect(store.rows[0]).toMatchObject({
      address: "X0",
      signalName: "非常停止入力",
      comment: "NC接点",
    });
  });

  // ja: 接続先と線番は図面から読んだ値で、編集はできない (読み取り専用の列)
  it("takes the target and the wire number of each point from the drawing", async () => {
    mockBackend({
      points: [point(1, { address: "X0", target: "PB1:3", wire_no: "15" })],
    });
    const store = await opened([assignment("PLC1", "X0")]);
    expect(store.rows[0]).toMatchObject({ target: "PB1:3", wireNo: "15" });
  });

  // ja: 「自動採番で埋める」は開始アドレスから全行のアドレスを埋め、信号名とコメントは触らない
  it("fills every address from the start address without touching signal names", async () => {
    mockBackend({ points: [point(1, { address: "X0", signal_name: "非常停止入力" })] });
    const store = await opened([assignment("PLC1", "X0", "非常停止入力")]);
    store.fillAddresses();
    expect(store.rows.map((r) => r.address).slice(0, 3)).toEqual(["X0", "X1", "X2"]);
    expect(store.rows[0].signalName).toBe("非常停止入力");
  });

  // ja: 開始アドレスを変えて自動採番すると、その番号から続けて振られる
  it("renumbers from the start address the user typed", async () => {
    mockBackend();
    const store = await opened();
    store.startAddress = "X20";
    store.fillAddresses();
    expect(store.rows.map((r) => r.address).slice(0, 3)).toEqual(["X20", "X21", "X22"]);
  });

  // ja: 保存はset_plc_assignmentsコマンド1回で、編集した表がまとめてプロジェクトへ入る
  it("saves the whole table with a single set_plc_assignments command", async () => {
    mockBackend();
    const sent = spyCommands();
    const store = await opened();
    store.fillAddresses();
    store.updateRow(0, { signalName: "非常停止入力" });
    expect(store.dirty).toBe(true);
    await store.save();
    expect(sent).toHaveLength(1);
    const command = sent[0] as { type: string; assignments: PlcAssignment[] };
    expect(command.type).toBe("set_plc_assignments");
    expect(command.assignments).toHaveLength(16);
    expect(command.assignments[0]).toMatchObject({
      module_ref: "PLC1",
      address: "X0",
      signal_name: "非常停止入力",
    });
  });

  // ja: 保存しても他のモジュールの行は順番も中身もそのまま残る
  it("keeps the rows of the other modules untouched when saving", async () => {
    mockBackend();
    const sent = spyCommands();
    const store = await opened([
      assignment("PLC2", "Y0", "運転表示"),
      assignment("PLC1", "X0", "非常停止入力"),
    ]);
    store.updateRow(0, { comment: "NC接点" });
    await store.save();
    const command = sent[0] as { assignments: PlcAssignment[] };
    expect(command.assignments[0]).toMatchObject({ module_ref: "PLC2", signal_name: "運転表示" });
    expect(command.assignments[1]).toMatchObject({ module_ref: "PLC1", comment: "NC接点" });
  });

  // ja: すでにある行はidを引き継ぐので、保存し直しても表の同一性が保たれる
  it("keeps the id of the rows that already exist in the table", async () => {
    mockBackend({ points: [point(1, { address: "X0" })] });
    const sent = spyCommands();
    const store = await opened([assignment("PLC1", "X0", "", "", "row-1")]);
    store.updateRow(0, { signalName: "非常停止入力" });
    await store.save();
    const command = sent[0] as { assignments: PlcAssignment[] };
    expect(command.assignments[0].id).toBe("row-1");
  });

  // ja: 何も編集していなければ保存してもコマンドは送らない
  it("sends no command when nothing was edited", async () => {
    mockBackend();
    const sent = spyCommands();
    const store = await opened();
    expect(store.dirty).toBe(false);
    await store.save();
    expect(sent).toEqual([]);
  });

  // ja: 末尾の空行は保存しない (途中の空行は点番号がずれないよう残す)
  it("drops the empty rows at the end while keeping a blank row in the middle", async () => {
    mockBackend();
    const sent = spyCommands();
    const store = await opened();
    store.updateRow(0, { address: "X0", signalName: "非常停止入力" });
    store.updateRow(2, { address: "X2", signalName: "リミットSW" });
    await store.save();
    const command = sent[0] as { assignments: PlcAssignment[] };
    expect(command.assignments).toHaveLength(3);
    expect(command.assignments[1]).toMatchObject({ address: "", signal_name: "" });
  });

  // ja: CSVを読み込むと、そのモジュールの行だけがCSVの中身で置き換わる (undo一発で戻る)
  it("imports a CSV file into the table of the selected module", async () => {
    mockBackend();
    const importCsv = vi
      .spyOn(ipc, "importPlcAssignmentsCsv")
      .mockResolvedValue({ revision: 2, ops: [] });
    const store = await opened();
    const csv = "アドレス,信号名,コメント\nX0,非常停止入力,NC接点\n";
    expect(await store.importCsv(csv)).toBe(true);
    expect(importCsv).toHaveBeenCalledWith("PLC1", csv);
  });

  // ja: 列数の合わないCSVはエラーになり、表は一切変わらない
  it("reports the error of a malformed CSV and leaves the table alone", async () => {
    mockBackend();
    vi.spyOn(ipc, "importPlcAssignmentsCsv").mockRejectedValue(new Error("2行目: 列数が2です"));
    const store = await opened();
    expect(await store.importCsv("X0,非常停止入力\n")).toBe(false);
    expect(store.error).toContain("列数");
    expect(store.rows[0].address).toBe("");
  });

  // ja: 図面が外から変わったら (結線の追加やundo) 接続先・線番の列を取り直す
  it("refreshes the wiring columns when the drawing changes from outside the editor", async () => {
    mockBackend({ points: [point(1, { address: "X0" })] });
    const store = await opened([assignment("PLC1", "X0")]);
    expect(store.rows[0].target).toBe("");

    vi.spyOn(ipc, "getPlcAssignments").mockResolvedValue([
      point(1, { address: "X0", target: "PB1:3", wire_no: "15" }),
    ]);
    await store.refresh();
    expect(store.rows[0]).toMatchObject({ target: "PB1:3", wireNo: "15" });
  });

  // ja: 編集の途中で図面が変わっても、まだ保存していない信号名は消えない
  it("keeps the unsaved signal names while refreshing after an outside change", async () => {
    mockBackend();
    const store = await opened();
    store.updateRow(0, { signalName: "非常停止入力" });
    vi.spyOn(ipc, "getPlcAssignments").mockResolvedValue([
      point(1, { address: "X0", target: "PB1:3", wire_no: "15" }),
    ]);
    await store.refresh();
    expect(store.rows[0].signalName).toBe("非常停止入力");
    expect(store.rows[0].target).toBe("PB1:3");
  });

  // ja: 編集していないときの外からの変更は、表そのものを読み直す
  it("reloads the table itself after an outside change when nothing is being edited", async () => {
    mockBackend();
    const store = await opened();
    vi.spyOn(ipc, "getPlcAssignments").mockResolvedValue([
      point(1, { address: "X0", signal_name: "非常停止入力" }),
    ]);
    await store.refresh();
    expect(store.rows[0].signalName).toBe("非常停止入力");
    expect(store.dirty).toBe(false);
  });

  // ja: エディタを閉じている間は図面が変わっても読み込みに行かない
  it("does not fetch anything while the editor is closed", async () => {
    mockBackend();
    const store = usePlcIoStore();
    await store.refresh();
    expect(ipc.listPlcModules).not.toHaveBeenCalled();
  });

  // ja: PLCモジュールが1つも置かれていなければ表は空になり、保存も生成もできない
  it("shows an empty table when no PLC module is placed", async () => {
    mockBackend({ modules: [] });
    const store = await opened();
    expect(store.rows).toEqual([]);
    expect(store.moduleRef).toBeNull();
    expect(store.canGenerate).toBe(false);
  });

  // ja: 閉じると下書きと選択を捨てる
  it("throws away the draft when the editor is closed", async () => {
    mockBackend();
    const store = await opened();
    store.updateRow(0, { signalName: "非常停止入力" });
    store.close();
    expect(store.open).toBe(false);
    expect(store.rows).toEqual([]);
    expect(store.dirty).toBe(false);
  });
});

describe("PLC I/O sheet generation settings", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.restoreAllMocks();
  });

  // ja: 生成設定の既定はJIS慣行の縦バス+横ラング・ラング間隔10mm・先頭スキップ0
  it("defaults to a vertical bus ladder with a 10mm rung spacing", async () => {
    mockBackend();
    const store = await opened();
    store.openGenerate();
    expect(store.generateOpen).toBe(true);
    expect(store.options).toEqual({
      rung_spacing_mm: 10,
      start_skip: 0,
      ladder_style: "vertical-bus",
      placement: "new-ladder",
    });
  });

  // ja: 生成は選んだ設定と、部品DBから引いたモジュール定義でI/O図面を1ページ作る (undo一発)
  it("generates the I/O sheet from the settings and the module spec of the parts library", async () => {
    mockBackend();
    const generate = vi.spyOn(ipc, "generatePlcSheet").mockResolvedValue({ revision: 3, ops: [] });
    const store = await opened([assignment("PLC1", "X0", "非常停止入力")]);
    store.openGenerate();
    store.options.rung_spacing_mm = 12.5;
    store.options.start_skip = 2;
    expect(await store.generate()).toBe(true);
    expect(generate).toHaveBeenCalledWith("PLC1", di16, {
      rung_spacing_mm: 12.5,
      start_skip: 2,
      ladder_style: "vertical-bus",
      placement: "new-ladder",
    });
    expect(store.generateOpen).toBe(false);
  });

  // ja: 未実装のラダー形式 (横バス) では生成できない
  it("refuses to generate with a ladder style that is not implemented yet", async () => {
    mockBackend();
    const generate = vi.spyOn(ipc, "generatePlcSheet").mockResolvedValue({ revision: 3, ops: [] });
    const store = await opened();
    store.openGenerate();
    store.options.ladder_style = "horizontal-bus";
    expect(store.canGenerate).toBe(false);
    expect(await store.generate()).toBe(false);
    expect(generate).not.toHaveBeenCalled();
  });

  // ja: 未実装の配置方針 (前モジュールと同居) では生成できない
  it("refuses to generate with a module placement that is not implemented yet", async () => {
    mockBackend();
    const generate = vi.spyOn(ipc, "generatePlcSheet").mockResolvedValue({ revision: 3, ops: [] });
    const store = await opened();
    store.openGenerate();
    store.options.placement = "share-or-split";
    expect(store.canGenerate).toBe(false);
    expect(await store.generate()).toBe(false);
    expect(generate).not.toHaveBeenCalled();
  });

  // ja: 未保存の編集があれば、生成の前に割付表を保存する (図面のラベルが下書きと一致する)
  it("saves the pending edits before generating the sheet", async () => {
    mockBackend();
    const sent = spyCommands();
    vi.spyOn(ipc, "generatePlcSheet").mockResolvedValue({ revision: 3, ops: [] });
    const store = await opened();
    store.updateRow(0, { address: "X0", signalName: "非常停止入力" });
    store.openGenerate();
    expect(await store.generate()).toBe(true);
    expect(sent).toHaveLength(1);
    expect(sent[0]).toMatchObject({ type: "set_plc_assignments" });
  });

  // ja: 部品DBに無い型番のモジュールは、種別どおりの既定のアドレス体系 (三菱) で扱う
  it("falls back to the default address style when the module is not in the parts library", async () => {
    mockBackend({ parts: [] });
    const store = await opened();
    expect(store.spec).toEqual({
      points: 16,
      kind: "DI",
      address_prefix: "X",
      address_style: "mitsubishi",
    });
    expect(store.rows[0].autoAddress).toBe("X0");
  });
});
