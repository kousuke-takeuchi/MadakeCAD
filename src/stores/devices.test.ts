// デバイスナビゲータ (左パネル「デバイス」タブ) の仕様テスト
// (計画: docs/superpowers/plans/2026-08-22-m4-phase2-macros-xref-nav.md Task 4)。
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { ipc, type DeviceFunction, type DeviceNode } from "../ipc";
import { useDevicesStore } from "./devices";
import { useDocumentStore } from "./document";

function fn(over: Partial<DeviceFunction> = {}): DeviceFunction {
  return {
    kind: "coil",
    terminals: "A1-A2",
    entity_id: "e1",
    sheet_id: "sheet-1",
    sheet_no: 1,
    sheet_name: "シート1",
    zone: "C2",
    ...over,
  };
}

function relay(): DeviceNode {
  return {
    reference: "K1",
    kind: "relay",
    value: "MY2N",
    symbol_id: "relay_coil",
    symbol_name: "Relay coil",
    symbol_name_ja: "リレーコイル",
    poles: 0,
    functions: [
      fn(),
      fn({
        kind: "contact_no",
        terminals: "13-14",
        entity_id: "e2",
        sheet_id: "sheet-2",
        sheet_no: 2,
        zone: "B3",
      }),
    ],
  };
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.restoreAllMocks();
});

describe("デバイスツリーの読み込み", () => {
  // ja: タブを開くとツリーを読み込み、デバイスの件数が分かる
  it("opening the tab loads the tree and knows how many devices there are", async () => {
    const devices = useDevicesStore();
    vi.spyOn(ipc, "getDeviceTree").mockResolvedValue([relay()]);
    await devices.load();
    expect(devices.count).toBe(1);
    expect(devices.rows.map((r) => r.type)).toEqual(["device", "function", "function"]);
  });

  // ja: 図面が変わっていなければ読み直さない(タブを行き来しても無駄に問い合わせない)
  it("the tree is not reloaded while the drawing has not changed", async () => {
    const devices = useDevicesStore();
    const call = vi.spyOn(ipc, "getDeviceTree").mockResolvedValue([relay()]);
    await devices.load();
    await devices.load();
    expect(call).toHaveBeenCalledTimes(1);
  });

  // ja: 図面が変われば読み直す
  it("the tree is reloaded once the drawing has changed", async () => {
    const devices = useDevicesStore();
    const store = useDocumentStore();
    const call = vi.spyOn(ipc, "getDeviceTree").mockResolvedValue([relay()]);
    await devices.load();
    store.revision = 7;
    await devices.load();
    expect(call).toHaveBeenCalledTimes(2);
  });
});

describe("折りたたみと選択", () => {
  // ja: デバイス行をたたむと機能の行が隠れ、もう一度たたむと戻る
  it("collapsing a device hides its functions and expanding brings them back", async () => {
    const devices = useDevicesStore();
    vi.spyOn(ipc, "getDeviceTree").mockResolvedValue([relay()]);
    await devices.load();
    devices.toggle("K1");
    expect(devices.rows.map((r) => r.type)).toEqual(["device"]);
    devices.toggle("K1");
    expect(devices.rows).toHaveLength(3);
  });

  // ja: 行を選ぶと選択が保持される
  it("a picked row stays selected", () => {
    const devices = useDevicesStore();
    devices.select("K1#1");
    expect(devices.selectedKey).toBe("K1#1");
    devices.select(null);
    expect(devices.selectedKey).toBeNull();
  });
});

describe("デバイスの削除", () => {
  // ja: デバイスの削除はCommand経由なのでCmd+Zで戻せる。シートを跨ぐぶんはシートごとに送る
  it("deleting a device goes through the command engine, one command per sheet", async () => {
    const devices = useDevicesStore();
    const store = useDocumentStore();
    vi.spyOn(ipc, "getDeviceTree").mockResolvedValue([relay()]);
    const execute = vi.spyOn(store, "execute").mockResolvedValue({} as never);
    await devices.load();
    const removed = await devices.deleteDevice(relay());
    expect(removed).toBe(2);
    expect(execute).toHaveBeenNthCalledWith(1, {
      type: "delete_entities",
      sheet_id: "sheet-1",
      ids: ["e1"],
    });
    expect(execute).toHaveBeenNthCalledWith(2, {
      type: "delete_entities",
      sheet_id: "sheet-2",
      ids: ["e2"],
    });
  });

  // ja: 1つのシンボルが複数の機能を持つ端子台でも、削除は1回だけ送る
  it("a terminal block whose one symbol carries several functions is deleted only once", async () => {
    const devices = useDevicesStore();
    const store = useDocumentStore();
    vi.spyOn(ipc, "getDeviceTree").mockResolvedValue([]);
    const execute = vi.spyOn(store, "execute").mockResolvedValue({} as never);
    const block: DeviceNode = {
      reference: "TB1",
      kind: "terminal_block",
      value: "",
      symbol_id: "terminal_block_8p",
      symbol_name: "Terminal block",
      symbol_name_ja: "端子台",
      poles: 8,
      functions: [
        fn({ kind: "terminal", terminals: "1", entity_id: "tb" }),
        fn({ kind: "terminal", terminals: "2", entity_id: "tb" }),
      ],
    };
    expect(await devices.deleteDevice(block)).toBe(1);
    expect(execute).toHaveBeenCalledTimes(1);
    expect(execute).toHaveBeenCalledWith({
      type: "delete_entities",
      sheet_id: "sheet-1",
      ids: ["tb"],
    });
  });

  // ja: 消したデバイスの行が選ばれていたら、選択も外してツリーを読み直す
  it("deleting the selected device clears the selection and reloads the tree", async () => {
    const devices = useDevicesStore();
    const store = useDocumentStore();
    const call = vi.spyOn(ipc, "getDeviceTree").mockResolvedValue([relay()]);
    vi.spyOn(store, "execute").mockResolvedValue({} as never);
    await devices.load();
    devices.select("K1#0");
    await devices.deleteDevice(relay());
    expect(devices.selectedKey).toBeNull();
    expect(call).toHaveBeenCalledTimes(2);
  });
});
