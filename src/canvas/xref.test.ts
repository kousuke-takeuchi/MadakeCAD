// シート間クロスリファレンスの仕様 (docs/internal/specs/m2-drawing-parity.md §4)。
// Rust側 madake-core/src/xref.rs と同一ルール。
import { describe, expect, it } from "vitest";

import type { Entity, Project, Sheet, SymbolInstance } from "../ipc";
import {
  NET_LABEL_RISE,
  deviceXrefAddresses,
  deviceXrefAt,
  sheetDeviceXrefs,
  sheetXrefs,
  xrefJumpTarget,
  xrefSites,
  xrefText,
  xrefTextAt,
  zoneAt,
} from "./xref";
import {
  COIL_SYMBOL_ID,
  CONTACT_NO_SYMBOL_ID,
  sheetCoilLocations,
  sheetContactMaps,
} from "./relayXref";

function sheet(id: string, name: string, entities: Entity[]): Sheet {
  return {
    id,
    name,
    size: "A3",
    orientation: "Landscape",
    zone_cols: 4,
    zone_rows: 6,
    title_block: {},
    revisions: [],
    entities: Object.fromEntries(entities.map((e) => [e.id, e])),
  };
}

function netLabel(id: string, name: string, x: number, y: number): Entity {
  return { kind: "net_label", id, at: { x, y }, name, rotation: 0 };
}

function symbolEntity(
  id: string,
  symbolId: string,
  reference: string,
  x: number,
  y: number,
): SymbolInstance & { kind: "symbol" } {
  return {
    kind: "symbol",
    id,
    symbol_id: symbolId,
    at: { x, y },
    rotation: 0,
    mirror: false,
    reference,
    value: "",
    attrs: {},
  };
}

/** 2枚のA3シートに同じ参照記号のシンボルを1個ずつ置いたプロジェクト (シート1=A1、シート2=B3)。 */
function twoSheetDeviceProject(symbolId: string, reference: string): Project {
  return {
    format_version: 1,
    name: "t",
    wire_parts: [],
    plc_assignments: [],
    sheets: [
      sheet("s1", "Sheet1", [symbolEntity("e1", symbolId, reference, 20, 20)]),
      sheet("s2", "Sheet2", [symbolEntity("e2", symbolId, reference, 250, 70)]),
    ],
  };
}

/** 2枚のA3シートに同名ラベル (シート1=A1、シート2=B3) を置いたプロジェクト。 */
function twoSheetProject(): Project {
  return {
    format_version: 1,
    name: "t",
    wire_parts: [],
    plc_assignments: [],
    sheets: [
      sheet("s1", "Sheet1", [netLabel("l1", "24V_1", 20, 20)]),
      sheet("s2", "Sheet2", [netLabel("l2", "24V_1", 250, 70)]),
    ],
  };
}

describe("zoneAt", () => {
  // ja: ゾーンアドレスは行の英字(上から)と列の数字(左から)を組み合わせた「B3」形式になる
  it("combines the row letter (top to bottom) with the column number (left to right)", () => {
    const s = sheet("s", "Sheet1", []);
    expect(zoneAt(s, { x: 20, y: 20 })).toBe("A1");
    expect(zoneAt(s, { x: 250, y: 70 })).toBe("B3");
    expect(zoneAt(s, { x: 400, y: 280 })).toBe("F4");
  });

  // ja: 図枠の外にある点は、無効なアドレスにはならず最も近いゾーンに丸められる
  it("rounds points outside the drawing frame to the nearest zone", () => {
    const s = sheet("s", "Sheet1", []);
    expect(zoneAt(s, { x: -50, y: -50 })).toBe("A1");
    expect(zoneAt(s, { x: 9999, y: 9999 })).toBe("F4");
  });
});

describe("cross references", () => {
  // ja: ラベルの相手先は、他のシートにある同名ラベルの住所「/シート.ゾーン」になる
  it("addresses a counterpart as /sheet.zone", () => {
    const project = twoSheetProject();
    expect(xrefText(project, "s1", "24V_1")).toBe("/2.B3");
  });

  // ja: 相手側のシートのラベルからも元のシートが見えるので、双方に相手先が表示される
  it("shows the counterpart on both sides", () => {
    const project = twoSheetProject();
    expect(xrefText(project, "s2", "24V_1")).toBe("/1.A1");
  });

  // ja: 同じネットが複数のシートに続くときは、相手先の住所が全て列挙される
  it("lists every counterpart when the net continues onto several sheets", () => {
    const project = twoSheetProject();
    project.sheets.push(sheet("s3", "Sheet3", [netLabel("l3", "24V_1", 20, 20)]));
    expect(xrefText(project, "s1", "24V_1")).toBe("/2.B3 /3.A1");
  });

  // ja: 自分のシート内の所在は、同名ラベルが2つあっても相手先には出ない
  it("never lists the label's own sheet as a counterpart", () => {
    const project = twoSheetProject();
    project.sheets[0].entities["l1b"] = netLabel("l1b", "24V_1", 250, 70);
    expect(xrefText(project, "s1", "24V_1")).toBe("/2.B3");
  });

  // ja: 他のシートに相手がいないラベルには、クロスリファレンスが一切表示されない
  it("shows nothing for a label without a counterpart", () => {
    const project = twoSheetProject();
    project.sheets[0].entities["only"] = netLabel("only", "ONLY_HERE", 100, 100);
    expect(xrefText(project, "s1", "ONLY_HERE")).toBeNull();
    expect(sheetXrefs(project, "s1").has("only")).toBe(false);
  });

  // ja: 相手先の一覧はジャンプ先のシートidとラベルidを持つので、クリックで飛べる
  it("returns the target sheet and label id so the panel can jump to it", () => {
    const project = twoSheetProject();
    const sites = xrefSites(project, "s1", "24V_1");
    expect(sites).toHaveLength(1);
    expect(sites[0]).toMatchObject({
      sheet_id: "s2",
      sheet_no: 2,
      sheet_name: "Sheet2",
      zone: "B3",
      label_id: "l2",
      address: "/2.B3",
    });
  });

  // ja: シートごとのクロスリファレンス表は、各ラベルのidを脇に描くテキストへ対応付ける
  it("maps each label id to the text drawn beside it", () => {
    const table = sheetXrefs(twoSheetProject(), "s1");
    expect(table.get("l1")).toBe("/2.B3");
  });
});

describe("xrefJumpTarget", () => {
  // ja: 相手先をクリックすると、相手のシートへ切り替えて相手のラベルを選択・ズームする指示になる
  it("names the sheet to switch to and the label to select and zoom", () => {
    const [site] = xrefSites(twoSheetProject(), "s1", "24V_1");
    expect(xrefJumpTarget(site)).toEqual({ sheetId: "s2", entityIds: ["l2"] });
  });
});

describe("xrefTextAt", () => {
  // ja: クロスリファレンスのテキストは、ラベル本文の右側に同じベースラインで並ぶ
  it("places the text right of the label text on the same baseline", () => {
    const at = xrefTextAt({ at: { x: 100, y: 50 }, name: "24V_1" });
    expect(at.x).toBeCloseTo(100 + 2.5 * 0.6 * 5 + 1, 9);
    expect(at.y).toBeCloseTo(50 - NET_LABEL_RISE, 9);
  });
});

describe("G13 device cross-reference", () => {
  // ja: 2枚の違うシートに描かれた参照記号は相互に参照し合う: それぞれのインスタンスにもう一方のシートの住所が出る
  it("cross-references a designator on two sheets both ways", () => {
    const project = twoSheetDeviceProject("motor", "M1");
    expect(deviceXrefAddresses(project, "s1", "M1")).toEqual(["/2.B3"]);
    expect(deviceXrefAddresses(project, "s2", "M1")).toEqual(["/1.A1"]);
  });

  // ja: 1枚のシートにしか描かれていない参照記号には、クロスリファレンスが一切付かない
  it("shows nothing for a designator on a single sheet", () => {
    const project: Project = {
      format_version: 1,
      name: "t",
      wire_parts: [],
      plc_assignments: [],
      sheets: [sheet("s1", "Sheet1", [symbolEntity("e1", "motor", "M1", 20, 20)])],
    };
    expect(deviceXrefAddresses(project, "s1", "M1")).toEqual([]);
    expect(sheetDeviceXrefs(project, "s1").size).toBe(0);
  });

  // ja: 接点を従えたリレーのコイルと、その接点は、デバイス単位のクロスリファレンスの対象から外れる(コイル⇔接点クロスリファレンスが既に互いの所在を書いているため)
  it("skips a relay coil and the contact it drives", () => {
    const project: Project = {
      format_version: 1,
      name: "t",
      wire_parts: [],
      plc_assignments: [],
      sheets: [
        sheet("s1", "Sheet1", [symbolEntity("e1", COIL_SYMBOL_ID, "K1", 20, 20)]),
        sheet("s2", "Sheet2", [symbolEntity("e2", CONTACT_NO_SYMBOL_ID, "K1", 250, 70)]),
      ],
    };
    expect(sheetContactMaps(project, "s1").size).toBe(1);
    expect(sheetCoilLocations(project, "s2").size).toBe(1);
    expect(deviceXrefAddresses(project, "s1", "K1")).toEqual([]);
    expect(sheetDeviceXrefs(project, "s1").size).toBe(0);
    expect(sheetDeviceXrefs(project, "s2").size).toBe(0);
  });

  // ja: 接点をどこにも持たないコイルにはコイル⇔接点クロスリファレンスが何も描かないので、そういうコイルだけの参照記号には普通のデバイス単位クロスリファレンスが付く (G13)
  it("still cross-references a contactless coil that appears on two sheets", () => {
    const project = twoSheetDeviceProject(COIL_SYMBOL_ID, "K1");
    expect(sheetContactMaps(project, "s1").size).toBe(0);
    expect(deviceXrefAddresses(project, "s1", "K1")).toEqual(["/2.B3"]);
    expect(sheetDeviceXrefs(project, "s1").size).toBe(1);
    expect(sheetDeviceXrefs(project, "s2").size).toBe(1);
  });

  // ja: シートごとのデバイスXRef表は、各シンボルのentity idを脇に描くテキストへ対応付ける
  it("maps each symbol entity id to the text drawn beside it", () => {
    const table = sheetDeviceXrefs(twoSheetDeviceProject("motor", "M1"), "s1");
    expect(table.get("e1")).toBe("/2.B3");
  });

  // ja: 3枚のシートに跨る参照記号は、シート順で並んだ他シート全部の住所を1行に並べる
  it("lists every other sheet for a designator on three sheets", () => {
    const project: Project = {
      format_version: 1,
      name: "t",
      wire_parts: [],
      plc_assignments: [],
      sheets: [
        sheet("s1", "Sheet1", [symbolEntity("e1", "motor", "M1", 20, 20)]),
        sheet("s2", "Sheet2", [symbolEntity("e2", "motor", "M1", 20, 20)]),
        sheet("s3", "Sheet3", [symbolEntity("e3", "motor", "M1", 20, 20)]),
      ],
    };
    expect(deviceXrefAddresses(project, "s1", "M1")).toEqual(["/2.A1", "/3.A1"]);
  });

  // ja: デバイスXRefのテキストはシンボル外形の右側、参照記号のラベルと同じ高さに置かれる
  it("places the text right of the symbol outline, level with the designator label", () => {
    const bounds = { min: { x: 92.5, y: 47 }, max: { x: 107.5, y: 53 } };
    const topY = 47;
    const at = deviceXrefAt(symbolEntity("e1", "motor", "M1", 100, 50), bounds, topY);
    expect(at.x).toBeGreaterThan(bounds.max.x);
    expect(at.y).toBeCloseTo(topY - 4.5, 9);
  });

  // ja: 参照記号のラベルがシンボルの外形より横に長いときは、デバイスXRefはその右まで下がるので、ラベルと重ならない
  it("clears a designator label wider than the symbol itself", () => {
    const bounds = { min: { x: 92.5, y: 47 }, max: { x: 107.5, y: 53 } };
    const inst = symbolEntity("e1", "motor", "M1-CONVEYOR-DRIVE", 100, 50);
    // 参照記号は配置基準点を中心に描かれるので、右端は中心+幅の半分 (文字高2.5mm)
    const labelRight = inst.at.x + (2.5 * 0.6 * inst.reference.length) / 2;
    expect(labelRight).toBeGreaterThan(bounds.max.x);
    expect(deviceXrefAt(inst, bounds, 47).x).toBeGreaterThanOrEqual(labelRight + 1.0 - 1e-9);
  });
});
