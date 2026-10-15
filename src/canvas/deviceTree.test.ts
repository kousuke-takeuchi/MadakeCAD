import { describe, expect, it } from "vitest";
import type { DeviceFunction, DeviceNode } from "../ipc";
import {
  deviceDeleteIds,
  deviceHeadline,
  deviceJumpTarget,
  deviceRows,
  functionAddress,
  functionHeadline,
  matchesFilter,
  unplacedHeadline,
} from "./deviceTree";

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
    levels: 0,
    functions: [
      fn(),
      fn({ kind: "contact_no", terminals: "13-14", entity_id: "e2", sheet_no: 2, zone: "B3" }),
    ],
  };
}

function terminalBlock(): DeviceNode {
  return {
    reference: "TB1",
    kind: "terminal_block",
    value: "",
    symbol_id: "terminal_block_8p",
    symbol_name: "Terminal block",
    symbol_name_ja: "端子台",
    poles: 8,
    levels: 1,
    functions: [fn({ kind: "terminal", terminals: "1-8", entity_id: "e3", zone: "B2" })],
  };
}

function fuse(): DeviceNode {
  return {
    reference: "F1",
    kind: "other",
    value: "",
    symbol_id: "fuse",
    symbol_name: "Fuse",
    symbol_name_ja: "ヒューズ",
    poles: 0,
    levels: 0,
    functions: [fn({ kind: "body", terminals: "", entity_id: "e4" })],
  };
}

describe("デバイスツリーの整形", () => {
  // ja: 展開中のデバイスは、見出しの下に機能の行が続く
  it("an expanded device is followed by one row per function", () => {
    const rows = deviceRows([relay()], new Set());
    expect(rows.map((r) => r.type)).toEqual(["device", "function", "function"]);
    expect(rows[0]).toMatchObject({ type: "device", reference: "K1", expanded: true });
    expect(rows[1]).toMatchObject({ type: "function", index: 0 });
    expect(rows[2]).toMatchObject({ type: "function", index: 1 });
  });

  // ja: 折りたたんだデバイスは見出しだけになり、他のデバイスの展開には影響しない
  it("a collapsed device shows only its heading and leaves other devices alone", () => {
    const rows = deviceRows([relay(), fuse()], new Set(["K1"]));
    expect(rows.map((r) => `${r.type}:${r.reference}`)).toEqual([
      "device:K1",
      "device:F1",
      "function:F1",
    ]);
    expect(rows[0]).toMatchObject({ expanded: false });
  });

  // ja: デバイスの並びも機能の並びもRust側が決めた順のまま変えない
  it("the order of devices and of their functions is kept as the core returned it", () => {
    const rows = deviceRows([terminalBlock(), relay()], new Set());
    expect(rows.filter((r) => r.type === "device").map((r) => r.reference)).toEqual(["TB1", "K1"]);
    const functions = rows.filter((r) => r.type === "function");
    expect(functions.map((r) => (r.type === "function" ? r.fn.terminals : ""))).toEqual([
      "1-8",
      "A1-A2",
      "13-14",
    ]);
  });

  // ja: 空のプロジェクトのツリーは1行も出ない
  it("an empty project produces no rows", () => {
    expect(deviceRows([], new Set())).toEqual([]);
  });
});

describe("見出しの文言", () => {
  // ja: 多段端子台は端子の数ではなく「何極何段」で呼ぶ
  it("a multi-level terminal block is named by its poles and its levels, not by its terminal count", () => {
    const block: DeviceNode = { ...terminalBlock(), poles: 4, levels: 2 };
    expect(deviceHeadline(block, true)).toEqual({
      key: "devices.head.terminalBlockMultiLevel",
      params: { poles: "4", levels: "2" },
    });
  });

  // ja: リレーは種別と型番で呼ぶ(型番が無ければ種別だけ)
  it("a relay is named by its kind and part number", () => {
    expect(deviceHeadline(relay(), true)).toEqual({
      key: "devices.head.relayWithValue",
      params: { value: "MY2N" },
    });
    expect(deviceHeadline({ ...relay(), value: "" }, true)).toEqual({
      key: "devices.head.relay",
      params: { value: "" },
    });
  });

  // ja: 端子台は極数つきで呼ぶ
  it("a terminal block is named with its pole count", () => {
    expect(deviceHeadline(terminalBlock(), true)).toEqual({
      key: "devices.head.terminalBlock",
      params: { poles: "8", levels: "1" },
    });
  });

  // ja: その他の部品はシンボルの名前で呼び、UI言語に合わせて英語名と日本語名を使い分ける
  it("other parts are named after their symbol, in the UI language", () => {
    expect(deviceHeadline(fuse(), true)).toEqual({
      key: "devices.head.other",
      params: { name: "ヒューズ", value: "" },
    });
    expect(deviceHeadline(fuse(), false)).toEqual({
      key: "devices.head.other",
      params: { name: "Fuse", value: "" },
    });
  });

  // ja: 機能の行の名前と所在バッジは「接点 13-14」「/2.B3」の形になる
  it("a function row reads like 'contact 13-14' with a '/2.B3' badge", () => {
    const contact = relay().functions[1];
    expect(functionHeadline(contact)).toEqual({
      key: "devices.function.contactNo",
      params: { terminals: "13-14" },
    });
    expect(functionAddress(contact)).toBe("/2.B3");
  });
});

describe("ツリーからの操作", () => {
  // ja: 機能の行をクリックすると、その機能のシートへ切り替えてエンティティを選ぶ
  it("clicking a function row switches to its sheet and selects its entity", () => {
    expect(deviceJumpTarget(relay().functions[1])).toEqual({
      sheetId: "sheet-1",
      entityIds: ["e2"],
    });
  });

  // ja: デバイスの削除は、その参照記号の全機能のエンティティを重複なく消す
  it("deleting a device removes every entity of that reference designator, without duplicates", () => {
    expect(deviceDeleteIds(relay())).toEqual(["e1", "e2"]);
    const doubled: DeviceNode = {
      ...terminalBlock(),
      functions: [
        fn({ kind: "terminal", terminals: "1", entity_id: "e3" }),
        fn({ kind: "terminal", terminals: "2", entity_id: "e3" }),
      ],
    };
    expect(deviceDeleteIds(doubled)).toEqual(["e3"]);
  });
});

describe("未配置機能 (予約接点)", () => {
  function reserved(): DeviceNode {
    return {
      ...relay(),
      unplaced: [
        { role: "contact_no", terminals: "23-24", symbol_id: "relay_contact_no" },
        { role: "contact_nc", terminals: "31-32", symbol_id: "relay_contact_nc" },
      ],
    };
  }

  // ja: 未配置の接点は、配置済みの機能の後ろに1行ずつ続く
  it("unplaced contacts follow the placed functions, one row each", () => {
    const rows = deviceRows([reserved()], new Set());
    expect(rows.map((r) => r.type)).toEqual(["device", "function", "function", "unplaced", "unplaced"]);
    const last = rows[4];
    expect(last.type === "unplaced" && last.unplaced.terminals).toBe("31-32");
  });

  // ja: 折りたたんだデバイスは未配置行も出さない
  it("a collapsed device hides its unplaced rows too", () => {
    expect(deviceRows([reserved()], new Set(["K1"])).map((r) => r.type)).toEqual(["device"]);
  });

  // ja: 絞り込み「未配置あり」は未配置機能のあるデバイスだけ、「配置済み」は未配置機能の無いデバイスだけを出す
  it("the filter keeps devices with unplaced functions, or only fully placed ones", () => {
    const placed = relay();
    expect(matchesFilter(reserved(), "unplaced")).toBe(true);
    expect(matchesFilter(placed, "unplaced")).toBe(false);
    expect(matchesFilter(placed, "placed")).toBe(true);
    expect(matchesFilter(reserved(), "placed")).toBe(false);
    expect(deviceRows([reserved(), { ...placed, reference: "K2" }], new Set(), "unplaced").filter((r) => r.type === "device").map((r) => r.reference)).toEqual(["K1"]);
  });

  // ja: 未配置行の名前は接点の種別と端子番号で読める (例「接点 23-24 NO」)
  it("an unplaced row is named by its contact kind and terminals", () => {
    expect(unplacedHeadline(reserved().unplaced![0])).toEqual({ key: "devices.unplaced.contactNo", params: { terminals: "23-24" } });
  });
});
