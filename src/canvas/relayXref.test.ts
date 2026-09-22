import { describe, expect, it } from "vitest";
import type { Entity, Project, Sheet, SymbolInstance } from "../ipc";
import {
  CONTACT_MAP_FONT,
  CONTACT_MAP_GAP,
  COIL_LOCATION_GAP,
  COIL_LOCATION_VGAP,
  DEVICE_BLOCK_ATTR,
  DEVICE_BLOCK_MIN_WIDTH,
  allocateContacts,
  CONTACT_MAP_MIN_COL_W,
  CONTACT_MAP_ROW_H,
  CONTACT_UNUSED,
  coilLocationAt,
  contactMapLayout,
  contactMapOrigin,
  deviceBlockLayout,
  isDeviceBlock,
  parseContactConfig,
  relayDevices,
  relayRole,
  sheetCoilLocations,
  sheetContactMaps,
  sheetDeviceBlocks,
  terminalPair,
  type ContactMapRow,
  type DeviceBlock,
} from "./relayXref";
import { symbolBounds } from "./renderer";
import { WIRE_NOTE_FONT, type WireNoteLabel } from "./wireNotes";

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
    plc_assignments: [],
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
    expect(parseContactConfig("2NO+2NC")).toEqual({ no: 2, nc: 2, co: 0 });
    expect(parseContactConfig("4NO")).toEqual({ no: 4, nc: 0, co: 0 });
    expect(parseContactConfig(" 1no , 3nc ")).toEqual({ no: 1, nc: 3, co: 0 });
    expect(parseContactConfig("2c")).toBeNull();
    expect(parseContactConfig("2NO+")).toBeNull();
    expect(parseContactConfig("")).toBeNull();
  });

  // ja: c接点(切換接点)も、a接点・b接点と同じようにそのリレーの接点として数えられる
  it("counts a changeover contact as a contact of its relay", () => {
    expect(relayRole("relay_contact_co")).toBe("contact_co");
    const p = project([
      [symbol("relay_coil", "K1", 20, 20), symbol("relay_contact_co", "K1", 60, 20)],
    ]);
    const device = relayDevices(p)[0];
    expect(device.contacts).toHaveLength(1);
    expect(device.contacts[0].role).toBe("contact_co");
  });

  // ja: c接点の端子は「11-12-14」のように、共通・b側・a側の3つを並べて書かれる
  it("writes the terminals of a changeover contact as common, break and make", () => {
    expect(terminalPair(1, "contact_co")).toBe("11-12-14");
    expect(terminalPair(2, "contact_co")).toBe("21-22-24");
  });

  // ja: 「2CO」のリレーに接点を1つだけ描いたときは、残り1極がc接点の未使用行として並ぶ
  it("lists the remaining changeover poles of a 2CO relay as unused rows", () => {
    expect(parseContactConfig("2CO")).toEqual({ no: 0, nc: 0, co: 2 });
    const p = project([
      [
        symbol("relay_coil", "K1", 20, 20, { contact_config: "2CO" }),
        symbol("relay_contact_co", "K1", 250, 70),
      ],
    ]);
    expect(relayDevices(p)[0].contactMap.map((r) => [r.terminals, r.address])).toEqual([
      ["11-12-14", "/1.B3"],
      ["21-22-24", CONTACT_UNUSED],
    ]);
  });

  // ja: c接点の極はa接点・b接点としても描けるので、「2CO」のリレーはa接点2個の使い方も受け付ける
  it("lets a changeover pole be drawn as a plain make or break contact", () => {
    const config = { no: 0, nc: 0, co: 2 };
    const both = allocateContacts(config, { no: 1, nc: 1, co: 0 });
    expect(both.shortage).toEqual({ no: 0, nc: 0, co: 0 });
    expect(both.spare.co).toBe(0);
    // c接点の図記号が使っている極は借りられない
    const taken = allocateContacts({ no: 0, nc: 0, co: 1 }, { no: 1, nc: 0, co: 1 });
    expect(taken.shortage.no).toBe(1);
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
    expect(coilLocationAt(inst, bounds, "(/1.C2)", [], WIRE_NOTE_FONT).x).toBeGreaterThan(107.5);
    expect(coilLocationAt(inst, bounds, "(/1.C2)", [], WIRE_NOTE_FONT).y).toBe(50);
  });

  // ja: 切換接点のコイル所在は、上の配線に付く注記と重ならないよう逃げる (G22の衝突の再現)。
  it("moves the coil location off a colliding wire note", () => {
    // 切換接点(c接点)は右2本のピンがシンボル中央高さ(y=50)を1グリッドピッチで挟む:
    // 「14」(a側)がy=47.5、「12」(b側)がy=52.5。y=47.5のピンから横に引いた配線の注記は
    // 線番と反対側 (下) の y=47.5+2.5=50.0 に来るので、右側の既定位置とぴったり重なる。
    const def = {
      id: "relay_contact_co",
      name: "",
      name_ja: "",
      category: "relay",
      ref_prefix: "K",
      primitives: [
        { type: "line" as const, pts: [{ x: -7.5, y: 0 }, { x: -2.5, y: 0 }] },
        { type: "line" as const, pts: [{ x: 2.5, y: 2.5 }, { x: 7.5, y: 2.5 }] },
        { type: "line" as const, pts: [{ x: 2.5, y: -2.5 }, { x: 7.5, y: -2.5 }] },
      ],
      pins: [
        { number: "11", name: "", at: { x: -7.5, y: 0 } },
        { number: "12", name: "", at: { x: 7.5, y: 2.5 } },
        { number: "14", name: "", at: { x: 7.5, y: -2.5 } },
      ],
    };
    const inst = symbol("relay_contact_co", "K1", 100, 50);
    const bounds = symbolBounds(inst, def);
    const notes: WireNoteLabel[] = [
      { wireId: "w1", text: "0.75sq 赤", at: { x: 118.75, y: 50 }, align: "center" },
    ];
    expect(coilLocationAt(inst, bounds, "(/1.C2)", [], WIRE_NOTE_FONT).y).toBe(50);
    const at = coilLocationAt(inst, bounds, "(/1.C2)", notes, WIRE_NOTE_FONT);
    expect(at.y).not.toBe(50);
    expect(at.y).toBeLessThan(50);
  });

  // ja: 上も塞がっているときは、コイル所在は外形の下へ逃げる。
  it("falls back below the outline when the space above is also occupied", () => {
    const def = {
      id: "relay_contact_co",
      name: "",
      name_ja: "",
      category: "relay",
      ref_prefix: "K",
      primitives: [],
      pins: [
        { number: "11", name: "", at: { x: -7.5, y: 0 } },
        { number: "12", name: "", at: { x: 7.5, y: 2.5 } },
        { number: "14", name: "", at: { x: 7.5, y: -2.5 } },
      ],
    };
    const inst = symbol("relay_contact_co", "K1", 100, 50);
    const bounds = symbolBounds(inst, def);
    const text = "(/1.C2)";
    const right = coilLocationAt(inst, bounds, text, [], WIRE_NOTE_FONT);
    const notes: WireNoteLabel[] = [
      { wireId: "w1", text, at: right, align: "left" },
      { wireId: "w2", text, at: { x: bounds.min.x, y: bounds.min.y - COIL_LOCATION_VGAP }, align: "left" },
    ];
    const at = coilLocationAt(inst, bounds, text, notes, WIRE_NOTE_FONT);
    expect(at.x).toBeCloseTo(bounds.min.x, 9);
    expect(at.y).toBeCloseTo(bounds.max.y + COIL_LOCATION_VGAP + CONTACT_MAP_FONT, 9);
  });

  // ja: 接点自身に型番が書かれているときは、コイル所在はその文字の上へは逃げず、さらに先の外形の反対側へ回る (G22)。
  it("does not escape the coil location onto the contact's own value label", () => {
    const def = {
      id: "relay_contact_co",
      name: "",
      name_ja: "",
      category: "relay",
      ref_prefix: "K",
      primitives: [],
      pins: [
        { number: "11", name: "", at: { x: -7.5, y: 0 } },
        { number: "12", name: "", at: { x: 7.5, y: 2.5 } },
        { number: "14", name: "", at: { x: 7.5, y: -2.5 } },
      ],
    };
    const blank = symbol("relay_contact_co", "K1", 100, 50);
    const withValue = { ...blank, value: "SAMPLE0001" };
    const bounds = symbolBounds(blank, def);
    const text = "(/1.C2)";
    // 右の既定位置だけをふさぐ配線注記 (上下はまだ空いている)
    const notes: WireNoteLabel[] = [
      { wireId: "w1", text, at: coilLocationAt(blank, bounds, text, [], WIRE_NOTE_FONT), align: "left" },
    ];
    expect(coilLocationAt(blank, bounds, text, notes, WIRE_NOTE_FONT).y).toBeLessThan(blank.at.y);
    expect(coilLocationAt(withValue, bounds, text, notes, WIRE_NOTE_FONT).y).toBeGreaterThan(bounds.max.y);
  });

  // ja: 逃がしの判定はそのシートが実際に描く文字高で配線注記を測るので、注記文字を大きくしたシートでもコイル所在はきちんと逃げる (G20とG22)
  it("measures the wire note at the text height that sheet draws it with", () => {
    const def = {
      id: "relay_contact_co",
      name: "",
      name_ja: "",
      category: "relay",
      ref_prefix: "K",
      primitives: [],
      pins: [
        { number: "11", name: "", at: { x: -7.5, y: 0 } },
        { number: "12", name: "", at: { x: 7.5, y: 2.5 } },
        { number: "14", name: "", at: { x: 7.5, y: -2.5 } },
      ],
    };
    const inst = { ...symbol("relay_contact_co", "", 100, 50), reference: "" };
    const bounds = symbolBounds(inst, def);
    const text = "(/1.C2)";
    // 既定(2.0mm)の高さで測ると右の既定位置には届かないが、3.5mmで描かれると重なる注記
    const notes: WireNoteLabel[] = [
      {
        wireId: "w1",
        text: "0.75sq 赤 0.4m",
        at: { x: bounds.max.x + COIL_LOCATION_GAP, y: inst.at.y + 2.6 },
        align: "left",
      },
    ];
    expect(coilLocationAt(inst, bounds, text, notes, WIRE_NOTE_FONT).y).toBe(50);
    expect(coilLocationAt(inst, bounds, text, notes, 3.5).y).not.toBe(50);
  });

  // ---- G7: 実機ブロック表示 ----

  // ja: コイルはattrs.device_blockがちょうど"true"のときだけ実機ブロック表示になり、それ以外(空文字含む)は従来どおりの分散配置になる。
  it("selects the display mode from the device_block attribute", () => {
    expect(isDeviceBlock(symbol("relay_coil", "K1", 20, 20))).toBe(false);
    expect(isDeviceBlock(symbol("relay_coil", "K1", 20, 20, { [DEVICE_BLOCK_ATTR]: "true" }))).toBe(true);
    expect(isDeviceBlock(symbol("relay_coil", "K1", 20, 20, { [DEVICE_BLOCK_ATTR]: "TRUE" }))).toBe(
      false,
    );
  });

  // ja: シートの実機ブロック一覧に入るのはdevice_blockが付いたコイルだけで、従来どおりの分散配置のコイルは入らない。
  it("only collects coils flagged as a device block", () => {
    const p = project([
      [
        symbol("relay_coil", "K1", 20, 20, { [DEVICE_BLOCK_ATTR]: "true" }),
        symbol("relay_coil", "K2", 20, 60),
        symbol("relay_contact_no", "K1", 60, 20),
      ],
    ]);
    const blocks = sheetDeviceBlocks(p, p.sheets[0].id);
    expect(blocks.size).toBe(1);
    const k1Id = p.sheets[0].entities[Object.keys(p.sheets[0].entities)[0]].id;
    expect(blocks.get(k1Id)?.contacts.length).toBe(1);
  });

  // ja: 実機ブロックで表示するコイルには、コイルの下にぶら下げる別表としての接点マップはもう付かない。
  it("excludes device-block coils from the separate contact map", () => {
    const p = project([
      [
        symbol("relay_coil", "K1", 20, 20, { [DEVICE_BLOCK_ATTR]: "true" }),
        symbol("relay_contact_no", "K1", 60, 20),
      ],
    ]);
    expect(sheetContactMaps(p, p.sheets[0].id).size).toBe(0);
    expect(sheetDeviceBlocks(p, p.sheets[0].id).size).toBe(1);
  });

  // ja: 実機ブロックの外形はコイルのピン間隔を下回らないので、幅が最小のときはA1/A2ピンがそのまま外形の左右端になり、引き出し線は要らない。
  it("keeps the device block layout no narrower than the pin span", () => {
    const block: DeviceBlock = { reference: "K1", partNo: "", contacts: [] };
    const layout = deviceBlockLayout(block);
    expect(layout.width).toBeCloseTo(DEVICE_BLOCK_MIN_WIDTH, 9);
    expect(layout.headerRows).toBe(1);
    expect(layout.leadHalfLen).toBeCloseTo(0, 9);
    expect(layout.x).toBeCloseTo(-DEVICE_BLOCK_MIN_WIDTH / 2, 9);
  });

  // ja: 型番があるとヘッダは2行になり、内容で外形がピン間隔より広がっても引き出し線でピンは外形まで届く。
  it("grows the device block for the part number and keeps the pins reachable", () => {
    const block: DeviceBlock = {
      reference: "K1",
      partNo: "SAMPLE0001-VERY-LONG-PART-NO",
      contacts: [{ terminals: "13-14", address: "/1.B3", role: "contact_no", entityId: null }],
    };
    const layout = deviceBlockLayout(block);
    expect(layout.headerRows).toBe(2);
    expect(layout.width).toBeGreaterThan(DEVICE_BLOCK_MIN_WIDTH);
    expect(layout.leadHalfLen).toBeGreaterThan(0);
    expect(layout.x).toBeCloseTo(-DEVICE_BLOCK_MIN_WIDTH / 2 - layout.leadHalfLen, 9);
    // ヘッダ1行目の中央がピンの高さ(ローカルY=0)
    expect(layout.y + layout.rowH / 2).toBeCloseTo(0, 9);
  });

  // ja: 行の仕切り線はコイルの引き出し線が付く高さには来ないので、2本の引き出し線が1本の線で繋がって見えることはない
  it("keeps the row dividers clear of the height the leads attach at", () => {
    const block: DeviceBlock = {
      reference: "K1",
      partNo: "P1",
      contacts: [{ terminals: "13-14", address: "/1.B3", role: "contact_no", entityId: null }],
    };
    const layout = deviceBlockLayout(block);
    const dividers = [
      ...Array.from({ length: layout.headerRows - 1 }, (_, i) => layout.y + layout.rowH * (i + 1)),
      ...Array.from({ length: layout.contactRows }, (_, i) => layout.contactRowTop(i)),
    ];
    expect(dividers.length).toBeGreaterThan(0);
    for (const y of dividers) expect(Math.abs(y)).toBeGreaterThan(1e-9);
  });

  // ja: 実機ブロックの接点の行はヘッダ行のすぐ下に続き、外形全体の高さは隙間なく全ての行をちょうど覆う。
  it("stacks the device block's contact rows under the header", () => {
    const block: DeviceBlock = {
      reference: "K1",
      partNo: "P1",
      contacts: [
        { terminals: "13-14", address: "/1.B3", role: "contact_no", entityId: null },
        { terminals: "21-22", address: CONTACT_UNUSED, role: "contact_nc", entityId: null },
      ],
    };
    const layout = deviceBlockLayout(block);
    expect(layout.headerRows).toBe(2);
    expect(layout.contactRows).toBe(2);
    expect(layout.contactRowTop(0)).toBeCloseTo(layout.y + layout.rowH * 2, 9);
    expect(layout.contactRowTop(1)).toBeCloseTo(layout.y + layout.rowH * 3, 9);
    expect(layout.height).toBeCloseTo(layout.rowH * 4, 9);
  });
});
