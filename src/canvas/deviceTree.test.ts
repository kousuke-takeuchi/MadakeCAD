import { describe, expect, it } from "vitest";
import type { DeviceFunction, DeviceNode } from "../ipc";
import {
  deviceDeleteIds,
  deviceHeadline,
  deviceJumpTarget,
  deviceRows,
  functionAddress,
  functionHeadline,
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
      params: { poles: "8" },
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
