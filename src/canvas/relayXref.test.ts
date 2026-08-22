import { describe, expect, it } from "vitest";
import type { Entity, Project, Sheet, SymbolInstance } from "../ipc";
import {
  CONTACT_MAP_GAP,
  CONTACT_MAP_MIN_COL_W,
  CONTACT_MAP_ROW_H,
  CONTACT_UNUSED,
  coilLocationAt,
  contactMapLayout,
  contactMapOrigin,
  parseContactConfig,
  relayDevices,
  relayRole,
  sheetCoilLocations,
  sheetContactMaps,
  terminalPair,
  type ContactMapRow,
} from "./relayXref";
import { symbolBounds } from "./renderer";

function sheet(name: string): Sheet {
  return {
    id: `sheet-${name}`,
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

let seq = 0;
function symbol(
  symbolId: string,
  reference: string,
  x: number,
  y: number,
  attrs: Record<string, string> = {},
): SymbolInstance & { kind: "symbol" } {
  seq += 1;
  return {
    kind: "symbol",
    id: `e${seq}`,
    symbol_id: symbolId,
    at: { x, y },
    rotation: 0,
    mirror: false,
    reference,
    value: "",
    attrs,
  };
}

function project(sheets: Entity[][]): Project {
  return {
    format_version: 1,
    name: "t",
    wire_parts: [],
    sheets: sheets.map((entities, i) => {
      const s = sheet(String(i + 1));
      for (const e of entities) s.entities[e.id] = e;
      return s;
    }),
  };
}

describe("relayXref", () => {
  // ja: 同じ参照記号を持つコイルと接点は、1つのリレーデバイスとしてまとめられる。
  it("groups the coil and contacts that share a reference into one device", () => {
    const p = project([
      [
        symbol("relay_coil", "K1", 20, 20),
        symbol("relay_contact_no", "K1", 60, 20),
        symbol("relay_contact_nc", "K1", 100, 20),
        symbol("relay_coil", "K2", 20, 60),
      ],
    ]);
    const devices = relayDevices(p);
    expect(devices.map((d) => d.reference)).toEqual(["K1", "K2"]);
    expect(devices[0].coils).toHaveLength(1);
    expect(devices[0].contacts).toHaveLength(2);
  });

  // ja: リレーのコイル・接点以外のシンボルは、リレーデバイスには含まれない。
  it("ignores symbols that are neither relay coils nor relay contacts", () => {
    expect(relayRole("lamp")).toBeNull();
    expect(relayDevices(project([[symbol("lamp", "L1", 20, 20)]]))).toHaveLength(0);
  });

  // ja: デバイスの接点はシート順→ゾーン順に並ぶので、端子対の連番は図面の読み順どおりになる。
  it("orders the contacts of a device by sheet and then by zone", () => {
    const p = project([
      [symbol("relay_coil", "K1", 20, 20), symbol("relay_contact_no", "K1", 250, 70)],
      [symbol("relay_contact_no", "K1", 20, 20)],
    ]);
    const device = relayDevices(p)[0];
    expect(device.contacts.map((c) => c.address)).toEqual(["/1.B3", "/2.A1"]);
  });

  // ja: 端子対はIEC 60947-1に従い、先頭の数字が接点の連番、末尾がa接点なら3/4・b接点なら1/2になる。
  it("builds terminal pairs from the contact position and its function digits", () => {
    expect(terminalPair(1, "contact_no")).toBe("13-14");
    expect(terminalPair(2, "contact_no")).toBe("23-24");
    expect(terminalPair(2, "contact_nc")).toBe("21-22");
    expect(terminalPair(3, "contact_nc")).toBe("31-32");
    expect(terminalPair(1, "coil")).toBe("A1-A2");
  });

  // ja: コイル下の接点マップには、配置済みの接点が端子対と図面上の住所とともに並ぶ。
  it("lists every placed contact with its terminal pair and address", () => {
    const p = project([
      [symbol("relay_coil", "K1", 20, 20), symbol("relay_contact_no", "K1", 250, 70)],
      [symbol("relay_contact_nc", "K1", 250, 70)],
    ]);
    const rows = relayDevices(p)[0].contactMap;
    expect(rows.map((r) => [r.terminals, r.address])).toEqual([
      ["13-14", "/1.B3"],
      ["21-22", "/2.B3"],
    ]);
  });

  // ja: 部品の接点構成が分かっているときは、まだ使っていない接点も行として並び、所在の代わりに「—」が入る。
  it("adds a dash row for every unused contact of the assigned part", () => {
    const p = project([
      [
        symbol("relay_coil", "K1", 20, 20, { contact_config: "2NO+2NC" }),
        symbol("relay_contact_no", "K1", 250, 70),
      ],
    ]);
    const rows = relayDevices(p)[0].contactMap;
    expect(rows.map((r) => [r.terminals, r.address])).toEqual([
      ["13-14", "/1.B3"],
      ["23-24", CONTACT_UNUSED],
      ["31-32", CONTACT_UNUSED],
      ["41-42", CONTACT_UNUSED],
    ]);
  });

  // ja: 接点構成が分からないときは実際に描かれた接点だけが並び、空の行は作られない。
  it("lists only the drawn contacts when no contact configuration is known", () => {
    const p = project([
      [symbol("relay_coil", "K1", 20, 20), symbol("relay_contact_no", "K1", 250, 70)],
    ]);
    expect(relayDevices(p)[0].contactMap).toHaveLength(1);
  });

  // ja: シートごとの接点マップはコイルのentity idで引くので、そのシートに居るコイルだけが表を持つ。
  it("keys the per-sheet contact maps by the coil on that sheet", () => {
    const coil = symbol("relay_coil", "K1", 20, 20);
    const p = project([[coil], [symbol("relay_contact_no", "K1", 20, 20)]]);
    const maps = sheetContactMaps(p, p.sheets[0].id);
    expect([...maps.keys()]).toEqual([coil.id]);
    expect(sheetContactMaps(p, p.sheets[1].id).size).toBe(0);
  });

  // ja: それぞれの接点の脇には、その接点を動かすコイルの住所が丸括弧付きで出る。
  it("shows the address of the driving coil beside each contact", () => {
    const contact = symbol("relay_contact_no", "K1", 20, 20);
    const p = project([[symbol("relay_coil", "K1", 250, 70)], [contact]]);
    expect(sheetCoilLocations(p, p.sheets[1].id).get(contact.id)).toBe("(/1.B3)");
  });

  // ja: コイルが見つからない接点には、コイル所在が一切表示されない。
  it("shows no coil location for a contact whose coil is missing", () => {
    const p = project([[symbol("relay_contact_no", "K9", 20, 20)]]);
    expect(sheetCoilLocations(p, p.sheets[0].id).size).toBe(0);
  });

  // ja: 「2NO+2NC」のような接点構成は、a接点・b接点の数として読み取られ、読めない値は無視される。
  it("parses make and break counts and ignores unreadable configurations", () => {
    expect(parseContactConfig("2NO+2NC")).toEqual({ no: 2, nc: 2 });
    expect(parseContactConfig("4NO")).toEqual({ no: 4, nc: 0 });
    expect(parseContactConfig(" 1no , 3nc ")).toEqual({ no: 1, nc: 3 });
    expect(parseContactConfig("2c")).toBeNull();
    expect(parseContactConfig("2NO+")).toBeNull();
    expect(parseContactConfig("")).toBeNull();
  });

  // ja: 接点マップの表はコイルの真下に中央揃えで置かれ、接点1個につき1行ずつ下へ伸びる。
  it("centres the contact map table under the coil", () => {
    const rows: ContactMapRow[] = [
      { terminals: "13-14", address: "/2.B3", role: "contact_no", entityId: null },
      { terminals: "21-22", address: CONTACT_UNUSED, role: "contact_nc", entityId: null },
    ];
    const layout = contactMapLayout(rows, 100, 60);
    expect(layout.x + layout.width / 2).toBeCloseTo(100, 9);
    expect(layout.y).toBeCloseTo(60, 9);
    expect(layout.height).toBeCloseTo(2 * CONTACT_MAP_ROW_H, 9);
    expect(layout.rowTop(1)).toBeCloseTo(60 + CONTACT_MAP_ROW_H, 9);
    expect(layout.colW[0]).toBeGreaterThanOrEqual(CONTACT_MAP_MIN_COL_W);
    expect(layout.colX(1)).toBeCloseTo(layout.x + layout.colW[0], 9);
  });

  // ja: 接点マップはコイルの外形の下にぶら下がり、コイル所在の文字は接点シンボルの右脇に置かれる。
  it("anchors both annotations to the symbol outline", () => {
    const def = {
      id: "relay_coil",
      name: "",
      name_ja: "",
      category: "relay",
      ref_prefix: "K",
      primitives: [
        { type: "rect" as const, p1: { x: -5, y: -3 }, p2: { x: 5, y: 3 }, filled: false },
      ],
      pins: [
        { number: "A1", name: "", at: { x: -7.5, y: 0 } },
        { number: "A2", name: "", at: { x: 7.5, y: 0 } },
      ],
    };
    const inst = symbol("relay_coil", "K1", 100, 50);
    const bounds = symbolBounds(inst, def);
    expect(contactMapOrigin(bounds, inst.at.x)).toEqual({ x: 100, y: 53 + CONTACT_MAP_GAP });
    expect(coilLocationAt(bounds, inst.at.y).x).toBeGreaterThan(107.5);
    expect(coilLocationAt(bounds, inst.at.y).y).toBe(50);
  });
});
