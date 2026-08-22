import { describe, expect, it } from "vitest";
import type { DeviceNode, Entity, Project, Sheet } from "../ipc";
import { surferJumpTarget, surferTargetFor } from "./surfer";
import { cycleIndex } from "./search";

function sheet(id: string, name: string): Sheet {
  return {
    id,
    name,
    size: "A3",
    orientation: "Landscape",
    zone_cols: 4,
    zone_rows: 6,
    title_block: {},
    revisions: [],
    entities: {},
  };
}

function project(sheets: Sheet[]): Project {
  return { format_version: 1, name: "t", sheets, wire_parts: [] };
}

function put(s: Sheet, e: Entity): Entity {
  s.entities[e.id] = e;
  return e;
}

function symbol(id: string, reference: string, x: number, y: number): Entity {
  return {
    kind: "symbol",
    id,
    symbol_id: "relay_coil",
    at: { x, y },
    rotation: 0,
    mirror: false,
    reference,
    value: "",
    attrs: {},
  };
}

function netLabel(id: string, name: string, x: number, y: number): Entity {
  return { kind: "net_label", id, at: { x, y }, name, rotation: 0 };
}

function wire(id: string, no: string | null, x: number, y: number): Entity {
  return {
    kind: "wire",
    id,
    points: [
      { x, y },
      { x: x + 20, y },
    ],
    color: "red",
    sq: 0.75,
    length_m: null,
    part_no: null,
    net: no,
  };
}

const K1: DeviceNode = {
  reference: "K1",
  kind: "relay",
  value: "MY2N",
  symbol_id: "relay_coil",
  symbol_name: "Relay coil",
  symbol_name_ja: "リレーコイル",
  poles: 0,
  functions: [
    {
      kind: "coil",
      terminals: "A1-A2",
      entity_id: "coil",
      sheet_id: "s1",
      sheet_no: 1,
      sheet_name: "シート1",
      zone: "C2",
    },
    {
      kind: "contact_no",
      terminals: "13-14",
      entity_id: "contact",
      sheet_id: "s2",
      sheet_no: 2,
      sheet_name: "シート2",
      zone: "B3",
    },
  ],
};

describe("参照サーフィン (Surfer)", () => {
  // ja: 参照記号のあるシンボルをAlt+クリックすると、同じデバイスの全機能が所在つきで並ぶ
  it("Alt-clicking a symbol lists every function of that device with its location", () => {
    const s1 = sheet("s1", "シート1");
    put(s1, symbol("coil", "K1", 100, 100));
    const target = surferTargetFor(project([s1]), [K1], "s1", "coil");
    expect(target).not.toBeNull();
    expect(target!.kind).toBe("device");
    expect(target!.title).toBe("K1");
    expect(target!.sites.map((s) => [s.function, s.terminals, s.address])).toEqual([
      ["coil", "A1-A2", "/1.C2"],
      ["contact_no", "13-14", "/2.B3"],
    ]);
  });

  // ja: ネットラベルをAlt+クリックすると、同名ラベルの所在が自分のシートも含めて並ぶ
  it("Alt-clicking a net label lists every place that name appears, including its own sheet", () => {
    const s1 = sheet("s1", "シート1");
    const s2 = sheet("s2", "シート2");
    put(s1, netLabel("l1", "24V", 30, 30));
    put(s2, netLabel("l2", "24V", 30, 30));
    put(s2, netLabel("l3", "0V", 30, 30));
    const target = surferTargetFor(project([s1, s2]), [], "s1", "l1");
    expect(target!.kind).toBe("net");
    expect(target!.title).toBe("24V");
    expect(target!.sites.map((s) => s.entityId)).toEqual(["l1", "l2"]);
    expect(target!.sites.every((s) => s.function === null)).toBe(true);
  });

  // ja: 線番のついたワイヤをAlt+クリックすると、同じ線番のワイヤが全部並ぶ
  it("Alt-clicking a numbered wire lists every wire carrying that number", () => {
    const s1 = sheet("s1", "シート1");
    put(s1, wire("w1", "101", 30, 30));
    put(s1, wire("w2", "101", 30, 200));
    put(s1, wire("w3", "102", 30, 30));
    const target = surferTargetFor(project([s1]), [], "s1", "w1");
    expect(target!.kind).toBe("wire_no");
    expect(target!.title).toBe("101");
    expect(target!.sites.map((s) => s.entityId)).toEqual(["w1", "w2"]);
  });

  // ja: 所在の並びはシート順→ゾーン順→id順で決まり、同じ図面なら常に同じ順になる
  it("locations come back in sheet, zone and id order, the same way every time", () => {
    const s1 = sheet("s1", "シート1");
    const s2 = sheet("s2", "シート2");
    put(s2, netLabel("b", "24V", 30, 30));
    put(s1, netLabel("z", "24V", 300, 200));
    put(s1, netLabel("a", "24V", 30, 30));
    const sites = surferTargetFor(project([s1, s2]), [], "s1", "a")!.sites;
    expect(sites.map((s) => `${s.entityId}${s.address}`)).toEqual([
      "a/1.A1",
      "z/1.E3",
      "b/2.A1",
    ]);
  });

  // ja: 名前も番号も付いていない要素ではポップアップを出さない
  it("an element with no designator, name or number has nothing to surf", () => {
    const s1 = sheet("s1", "シート1");
    put(s1, symbol("plain", "", 30, 30));
    put(s1, wire("bare", null, 30, 60));
    put(s1, { kind: "junction", id: "j", at: { x: 30, y: 90 } });
    const p = project([s1]);
    expect(surferTargetFor(p, [], "s1", "plain")).toBeNull();
    expect(surferTargetFor(p, [], "s1", "bare")).toBeNull();
    expect(surferTargetFor(p, [], "s1", "j")).toBeNull();
    expect(surferTargetFor(p, [], "s1", "missing")).toBeNull();
    expect(surferTargetFor(null, [], "s1", "plain")).toBeNull();
  });

  // ja: 図面に置かれていない参照記号(デバイスが見つからない)ではポップアップを出さない
  it("a reference designator with no device behind it has nothing to surf", () => {
    const s1 = sheet("s1", "シート1");
    put(s1, symbol("coil", "K9", 100, 100));
    expect(surferTargetFor(project([s1]), [K1], "s1", "coil")).toBeNull();
  });

  // ja: 行を選ぶと、その所在のシートへ切り替えてエンティティを選択+ズームする
  it("choosing a row switches to that sheet and reveals the entity", () => {
    const s1 = sheet("s1", "シート1");
    put(s1, symbol("coil", "K1", 100, 100));
    const sites = surferTargetFor(project([s1]), [K1], "s1", "coil")!.sites;
    expect(surferJumpTarget(sites[1])).toEqual({ sheetId: "s2", entityIds: ["contact"] });
  });

  // ja: ↑↓の巡回は検索のEnter巡回と同じ規則で、端まで行くと回り込む
  it("the up/down cycle follows the same wrap-around rule as the search bar", () => {
    const count = 2;
    expect(cycleIndex(count, 0, 1)).toBe(1);
    expect(cycleIndex(count, 1, 1)).toBe(0);
    expect(cycleIndex(count, 0, -1)).toBe(1);
  });
});
