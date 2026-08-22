// シート間クロスリファレンスの仕様 (docs/internal/specs/m2-drawing-parity.md §4)。
// Rust側 madake-core/src/xref.rs と同一ルール。
import { describe, expect, it } from "vitest";

import type { Entity, Project, Sheet } from "../ipc";
import {
  NET_LABEL_RISE,
  sheetXrefs,
  xrefJumpTarget,
  xrefSites,
  xrefText,
  xrefTextAt,
  zoneAt,
} from "./xref";

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
