// 図形 (G10) の幾何・当たり判定・コマンド組み立ての仕様テスト。
// Rust側 madake-core/src/shape.rs と同じルールであることを画面側でも固定する。
import { describe, expect, it, vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

import type { Entity, Point, Project, Sheet, ShapeKind } from "../ipc";
import {
  DEFAULT_SHAPE_COLOR,
  DEFAULT_SHAPE_STROKE_MM,
  MAX_SHAPE_STROKE_MM,
  MIN_SHAPE_STROKE_MM,
  SHAPE_DASH,
  SHAPE_FILL_OPACITY,
  SHAPE_TOOLS,
  dedupeConsecutive,
  isShapeTool,
  isValidShapeStrokeMm,
  newShape,
  requiredPoints,
  shapeAddCommand,
  shapeBounds,
  shapeCircle,
  shapeFillColor,
  shapeHitTest,
  shapeRectCorners,
  shapeStrokeColor,
  shapeUpdateCommand,
  shapes,
} from "./shapes";
import { textColors, theme } from "./theme";

const p = (x: number, y: number): Point => ({ x, y });

function shape(kind: ShapeKind, points: Point[]): Extract<Entity, { kind: "shape" }> {
  return newShape(kind, points);
}

function sheetWith(...entities: Entity[]): Sheet {
  return {
    id: "sheet-1",
    name: "Sheet1",
    size: "A3",
    orientation: "Landscape",
    zone_cols: 4,
    zone_rows: 6,
    title_block: {},
    revisions: [],
    entities: Object.fromEntries(entities.map((e) => [e.id, e])),
  } as unknown as Sheet;
}

describe("shape geometry", () => {
  // ja: 新しい図形は既定色・細線・塗りなし・実線で始まる(何も設定せずに引いた線が図枠と同じ見た目になる)
  it("starts a new shape plain: default colour, hairline stroke, no fill, solid line", () => {
    const rect = shape("rect", [p(10, 10), p(50, 30)]);
    expect(rect.stroke).toBe(DEFAULT_SHAPE_COLOR);
    expect(rect.stroke_width_mm).toBe(DEFAULT_SHAPE_STROKE_MM);
    expect(rect.fill).toBeNull();
    expect(rect.dashed).toBe(false);
  });

  // ja: 矩形は対角の2点で、どの向きにドラッグしても同じ矩形になる
  it("normalises a rectangle to the same corners whichever way it was dragged", () => {
    const forward = shapeRectCorners([p(10, 20), p(50, 40)]);
    const backward = shapeRectCorners([p(50, 40), p(10, 20)]);
    expect(forward).toEqual({ min: p(10, 20), max: p(50, 40) });
    expect(backward).toEqual(forward);
  });

  // ja: 円は中心と円周上の点で表すので、半径はその2点の距離になる
  it("reads a circle as a centre plus a point on its edge", () => {
    expect(shapeCircle([p(100, 50), p(100, 60)])).toEqual({ center: p(100, 50), r: 10 });
    expect(shapeBounds(shape("circle", [p(100, 50), p(100, 60)]))).toEqual({
      min: p(90, 40),
      max: p(110, 60),
    });
  });

  // ja: 折れ線は描いた点をそのまま順に持ち、外接矩形は全ての点を含む
  it("keeps every point of a polyline and bounds them all", () => {
    const poly = shape("polyline", [p(0, 0), p(10, 0), p(10, 10)]);
    expect(poly.points).toHaveLength(3);
    expect(shapeBounds(poly)).toEqual({ min: p(0, 0), max: p(10, 10) });
  });

  // ja: 矩形・線・円は2点ちょうど、折れ線は何点でもよい(Rust側と同じ規則)
  it("requires exactly two points for a rectangle, line and circle, and any number for a polyline", () => {
    expect(requiredPoints("rect")).toBe(2);
    expect(requiredPoints("line")).toBe(2);
    expect(requiredPoints("circle")).toBe(2);
    expect(requiredPoints("polyline")).toBeNull();
  });

  // ja: 線幅は0.1〜2.0mmの範囲だけを受け付ける(Commandエンジンと同じ範囲)
  it("accepts only printable stroke widths", () => {
    for (const w of [MIN_SHAPE_STROKE_MM, 0.25, 1, MAX_SHAPE_STROKE_MM]) {
      expect(isValidShapeStrokeMm(w), String(w)).toBe(true);
    }
    for (const w of [0, 0.05, 2.5, -1, Number.NaN]) {
      expect(isValidShapeStrokeMm(w), String(w)).toBe(false);
    }
  });
});

describe("shape colours on screen", () => {
  // ja: 図形は色トークンの画面色で描かれ、選択中は選択色が優先する
  it("draws a shape in its token's screen colour, and in the selection colour while selected", () => {
    const s = shape("rect", [p(0, 0), p(10, 10)]);
    expect(shapeStrokeColor(s, false)).toBe(textColors.default);
    expect(shapeStrokeColor({ ...s, stroke: "accent" }, false)).toBe(textColors.accent);
    expect(shapeStrokeColor(s, true)).toBe(theme.selection);
  });

  // ja: 塗りは下の配線・記号が透けて読める薄さで塗り、塗り無しは塗らない
  it("shades a filled shape lightly so the circuit underneath stays readable", () => {
    const s = shape("rect", [p(0, 0), p(10, 10)]);
    expect(shapeFillColor(s)).toBeNull();
    const filled = shapeFillColor({ ...s, fill: "warning" });
    expect(filled).toContain(`, ${SHAPE_FILL_OPACITY})`);
    expect(filled?.startsWith("rgba(")).toBe(true);
  });

  // ja: 破線の刻みはハーネス境界と同じ3mm/2mm
  it("uses the same dash rhythm as a harness boundary", () => {
    expect(SHAPE_DASH).toEqual([3, 2]);
  });
});

describe("shape hit testing", () => {
  // ja: 矩形は輪郭の線で掴む(塗りの下にある配線を選べるように、面の中では掴まない)
  it("grabs a rectangle by its outline, not by the area inside it", () => {
    const rect = shape("rect", [p(50, 50), p(150, 100)]);
    expect(shapeHitTest(rect, p(100, 50), 2)).toBe(true);
    expect(shapeHitTest(rect, p(50, 75), 2)).toBe(true);
    expect(shapeHitTest(rect, p(100, 75), 2)).toBe(false);
    expect(shapeHitTest(rect, p(200, 75), 2)).toBe(false);
  });

  // ja: 円は円周の上で掴む(中心も外側も掴まない)
  it("grabs a circle on its edge", () => {
    const circle = shape("circle", [p(100, 50), p(110, 50)]);
    expect(shapeHitTest(circle, p(90, 50), 2)).toBe(true);
    expect(shapeHitTest(circle, p(100, 50), 2)).toBe(false);
    expect(shapeHitTest(circle, p(130, 50), 2)).toBe(false);
  });

  // ja: 線と折れ線はどの区間の上でも掴める
  it("grabs a line or a polyline anywhere along it", () => {
    const line = shape("line", [p(50, 60), p(150, 60)]);
    expect(shapeHitTest(line, p(100, 60), 2)).toBe(true);
    expect(shapeHitTest(line, p(100, 70), 2)).toBe(false);
    const poly = shape("polyline", [p(0, 0), p(10, 0), p(10, 10)]);
    expect(shapeHitTest(poly, p(10, 5), 2)).toBe(true);
    expect(shapeHitTest(poly, p(5, 0), 2)).toBe(true);
    expect(shapeHitTest(poly, p(5, 5), 2)).toBe(false);
  });
});

describe("shape commands", () => {
  // ja: 作図の確定は add_entity コマンド1本になる(undo一発で消える)
  it("turns a finished drawing into one add_entity command", () => {
    const sheet = sheetWith();
    const command = shapeAddCommand(sheet, "rect", [p(50, 50), p(150, 100)]);
    expect(command).toMatchObject({ type: "add_entity", sheet_id: "sheet-1" });
    expect(command?.type === "add_entity" && command.entity.kind).toBe("shape");
  });

  // ja: クリックしただけのつぶれた図形はコマンドを送らない(誤操作で点だけの図形が生まれない)
  it("sends nothing for a collapsed shape from a stray click", () => {
    const sheet = sheetWith();
    expect(shapeAddCommand(sheet, "rect", [p(50, 50), p(50, 50)])).toBeNull();
    expect(shapeAddCommand(sheet, "line", [p(50, 50), p(50, 50.1)])).toBeNull();
    expect(shapeAddCommand(sheet, "circle", [p(50, 50), p(50, 50)])).toBeNull();
    expect(shapeAddCommand(sheet, "polyline", [p(50, 50)])).toBeNull();
    expect(shapeAddCommand(sheet, "rect", [p(50, 50)])).toBeNull();
  });

  // ja: 折れ線をダブルクリックで確定したときに重なる最後の点は1つにまとめられる
  it("merges the duplicate point a double-click leaves at the end of a polyline", () => {
    const sheet = sheetWith();
    const command = shapeAddCommand(sheet, "polyline", [p(0, 0), p(10, 0), p(10, 10), p(10, 10)]);
    if (command?.type !== "add_entity" || command.entity.kind !== "shape") throw new Error("図形の追加");
    expect(command.entity.points).toEqual([p(0, 0), p(10, 0), p(10, 10)]);
    expect(dedupeConsecutive([p(0, 0), p(0, 0), p(1, 1)])).toEqual([p(0, 0), p(1, 1)]);
  });

  // ja: 見た目の編集は update_entity コマンドになり、何も変わっていなければ送らない
  it("updates the look with one update_entity command, and sends nothing when nothing changed", () => {
    const s = shape("rect", [p(0, 0), p(10, 10)]);
    const command = shapeUpdateCommand("sheet-1", s, {
      stroke: "accent",
      stroke_width_mm: 0.5,
      fill: "muted",
      dashed: true,
    });
    expect(command).toMatchObject({ type: "update_entity", sheet_id: "sheet-1" });
    expect(command?.type === "update_entity" && command.entity).toMatchObject({
      stroke: "accent",
      stroke_width_mm: 0.5,
      fill: "muted",
      dashed: true,
    });
    expect(shapeUpdateCommand("sheet-1", s, {})).toBeNull();
    expect(shapeUpdateCommand("sheet-1", s, { stroke: DEFAULT_SHAPE_COLOR })).toBeNull();
  });

  // ja: 範囲外の線幅は送らない(エンジンも同じ範囲で拒否する)
  it("refuses to send a stroke width the engine would reject", () => {
    const s = shape("rect", [p(0, 0), p(10, 10)]);
    expect(shapeUpdateCommand("sheet-1", s, { stroke_width_mm: 5 })).toBeNull();
    expect(shapeUpdateCommand("sheet-1", s, { stroke_width_mm: 0 })).toBeNull();
  });

  // ja: 図形以外のエンティティにはコマンドを作らない
  it("makes no command for anything that is not a shape", () => {
    const wire = { kind: "wire", id: "w1", points: [], color: "red", sq: 0.75 } as unknown as Entity;
    expect(shapeUpdateCommand("sheet-1", wire, { dashed: true })).toBeNull();
  });

  // ja: シートの図形を数えると図形だけが返る(ハーネス境界は混ざらない)
  it("lists only the shapes of a sheet", () => {
    const rect = shape("rect", [p(0, 0), p(10, 10)]);
    const harness = {
      kind: "harness",
      id: "h1",
      points: [p(0, 0), p(1, 1)],
      name: "W1",
      note: "",
    } as unknown as Entity;
    expect(shapes(sheetWith(rect, harness)).map((s) => s.id)).toEqual([rect.id]);
  });

  // ja: 作図ツールのidは4種の図形にひとつずつ対応する
  it("maps each drawing tool to one kind of shape", () => {
    expect(SHAPE_TOOLS).toEqual({
      shape_rect: "rect",
      shape_line: "line",
      shape_circle: "circle",
      shape_polyline: "polyline",
    });
    expect(isShapeTool("shape_rect")).toBe(true);
    expect(isShapeTool("harness")).toBe(false);
  });
});

describe("shapes and the reports", () => {
  // ja: 図形は電線リストのハーネス列を汚さない(筐体をハーネスで囲まなくてよくなる理由)
  it("keeps a shape out of the harness column of the wire list", () => {
    // 画面側で電線リストを組み立てることはないので、ここで固定するのはデータの形:
    // 図形は harness エンティティではないので、ハーネス所属の判定に一切関わらない
    const enclosure = shape("rect", [p(0, 0), p(200, 200)]);
    const sheet = sheetWith(enclosure);
    const project = { sheets: [sheet] } as unknown as Project;
    expect(project.sheets[0].entities[enclosure.id].kind).toBe("shape");
    expect(
      Object.values(project.sheets[0].entities).some((e) => e.kind === "harness"),
    ).toBe(false);
  });
});
