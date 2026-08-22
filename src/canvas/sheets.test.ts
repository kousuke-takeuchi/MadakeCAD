// シート追加 (種別つき) の仕様テスト
// (M4仕様 §8、計画 docs/superpowers/plans/2026-08-23-m4-phase4-templates-symbol-editor-panel.md Task 6)。
import { describe, expect, it } from "vitest";

import type { Project, Sheet, SheetKind } from "../ipc";
import { addSheetCommand, isPanelSheet, sheetKindLabelKey } from "./sheets";

function sheet(id: string, name: string, kind?: SheetKind): Sheet {
  return {
    id,
    name,
    ...(kind ? { kind } : {}),
    size: "A3",
    orientation: "Landscape",
    zone_cols: 4,
    zone_rows: 6,
    title_block: {},
    revisions: [],
    entities: {},
  };
}

function project(...sheets: Sheet[]): Project {
  return {
    format_version: 4,
    name: "t",
    sheets,
    wire_parts: [],
    plc_assignments: [],
  };
}

describe("sheet kind (schematic / panel)", () => {
  // ja: シートを追加するときに種別を選べる。盤レイアウトを選ぶと盤シートが1枚増える。
  it("adds a panel sheet when the panel kind is chosen", () => {
    const command = addSheetCommand(project(sheet("s1", "Sheet1")), "panel");
    expect(command).toEqual({
      type: "add_sheet",
      name: "Panel1",
      size: "A3",
      orientation: "Landscape",
      kind: "panel",
    });
  });

  // ja: 種別に回路図を選ぶと、これまでどおりの回路図シートが増える。
  it("adds a schematic sheet when the schematic kind is chosen", () => {
    const command = addSheetCommand(project(sheet("s1", "Sheet1")), "schematic");
    expect(command).toEqual({
      type: "add_sheet",
      name: "Sheet2",
      size: "A3",
      orientation: "Landscape",
      kind: "schematic",
    });
  });

  // ja: 名前は種別ごとに数える (回路図が3枚あっても最初の盤は「Panel1」になる)。
  it("numbers the new sheet within its own kind", () => {
    const p = project(sheet("s1", "Sheet1"), sheet("s2", "Sheet2"), sheet("s3", "Panel1", "panel"));
    expect(addSheetCommand(p, "panel")).toMatchObject({ name: "Panel2" });
    expect(addSheetCommand(p, "schematic")).toMatchObject({ name: "Sheet3" });
  });

  // ja: すでに同じ名前のシートがあるときは、空いている番号まで数を進める。
  it("skips a number that another sheet already uses", () => {
    const p = project(sheet("s1", "Sheet1"), sheet("s2", "Panel1", "panel"), sheet("s3", "Sheet2"));
    expect(addSheetCommand(p, "schematic")).toMatchObject({ name: "Sheet3" });
  });

  // ja: 用紙は表示中のシートに合わせる (盤シートだけA4になってしまわない)。
  it("keeps the paper size and orientation of the sheet in front", () => {
    const wide = { ...sheet("s1", "Sheet1"), size: "A2" as const, orientation: "Portrait" as const };
    expect(addSheetCommand(project(wide), "panel")).toMatchObject({
      size: "A2",
      orientation: "Portrait",
    });
  });

  // ja: 種別を書いていない古いシートは回路図シートとして扱う。
  it("treats a sheet with no kind as a schematic sheet", () => {
    expect(isPanelSheet(sheet("s1", "Sheet1"))).toBe(false);
    expect(isPanelSheet(sheet("s1", "Sheet1", "schematic"))).toBe(false);
    expect(isPanelSheet(sheet("s2", "Panel1", "panel"))).toBe(true);
    expect(isPanelSheet(null)).toBe(false);
  });

  // ja: 図面タブの「シート種別」の表示は種別ごとの文言になる。
  it("labels the sheet kind shown next to the drawing tabs", () => {
    expect(sheetKindLabelKey(sheet("s1", "Sheet1"))).toBe("sheets.kind.schematic");
    expect(sheetKindLabelKey(sheet("s2", "Panel1", "panel"))).toBe("sheets.kind.panel");
  });
});
