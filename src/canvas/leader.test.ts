// Rust側 madake-core/src/leader.rs の leader_line / entity_bounds / leader_for と
// 同一ジオメトリを検証する (parityテスト: 同じ入力に同じ値が出ること)。
import { describe, expect, it } from "vitest";

import type { Entity, Sheet, SymbolDef, TextEntity } from "../ipc";
import { dynamicSymbol } from "./dynamicSymbol";
import { entityBounds, leaderFor, leaderLine, textBounds } from "./leader";

function note(x: number, y: number, text: string, leaderTo?: string | null): TextEntity {
  return {
    id: "note-1",
    at: { x, y },
    text,
    height: 2.5,
    rotation: 0,
    anchor: "start",
    color: null,
    bold: false,
    leader_to: leaderTo ?? null,
  };
}

function sheetWith(entities: Entity[]): Sheet {
  const map: Record<string, Entity> = {};
  for (const e of entities) map[(e as { id: string }).id] = e;
  return {
    id: "s1",
    name: "S1",
    size: "A3",
    orientation: "Landscape",
    zone_cols: 4,
    zone_rows: 6,
    title_block: {},
    revisions: [],
    entities: map,
  };
}

describe("leaderLine", () => {
  // ja: 対象の真上にある注記から引く引き出し線は、注記の下辺の中点から始まり、対象の上辺のいちばん近い点で終わる (Rust側 a_leader_above_its_target_runs_from_the_bottom_edge_to_the_top とparity)
  it("runs from a note's bottom edge to its target's top edge when the note sits above it", () => {
    const text = note(100, 50, "注意");
    const textRect = textBounds(text);
    const targetRect = { min: { x: 90, y: 60 }, max: { x: 110, y: 80 } };
    const [start, end] = leaderLine(textRect, targetRect);
    expect(start.y).toBeCloseTo(textRect.max.y, 9);
    expect(end.y).toBeCloseTo(targetRect.min.y, 9);
  });

  // ja: 対象の真横にある注記から引く引き出し線は、注記の近いほうの縦辺から対象の外接矩形まで水平に伸びる (Rust側 a_leader_beside_its_target_runs_horizontally とparity)
  it("runs horizontally from a note's near vertical edge when the note sits beside its target", () => {
    const text = note(0, 50, "注意");
    const textRect = textBounds(text); // x in [0, w], y in [47.5, 50]
    const targetRect = { min: { x: 50, y: 40 }, max: { x: 70, y: 60 } };
    const [start, end] = leaderLine(textRect, targetRect);
    expect(start.x).toBeCloseTo(textRect.max.x, 9);
    expect(end.x).toBeCloseTo(targetRect.min.x, 9);
  });
});

describe("leaderFor", () => {
  // ja: leader_toが未設定の普通の注記はnull (引き出し線を描かない)
  it("returns null when the note has no leader target", () => {
    const sheet = sheetWith([]);
    const text = note(0, 0, "普通の注記");
    expect(leaderFor(sheet, text, new Map())).toBeNull();
  });

  // ja: 対象がシート上に実在すれば線が引け、注記を動かすと引き出し線も追従して変わる
  it("recomputes after the note moves, following whichever position it is drawn at", () => {
    const target: Entity = { kind: "junction", id: "j1", at: { x: 100, y: 100 } };
    const sheet = sheetWith([target]);
    const before = leaderFor(sheet, note(0, 0, "対象", "j1"), new Map());
    const after = leaderFor(sheet, note(200, 200, "対象", "j1"), new Map());
    expect(before).not.toBeNull();
    expect(after).not.toBeNull();
    expect(before).not.toEqual(after);
  });

  // ja: 対象のエンティティidが存在しない・削除されていればnull (パニックせず何も描かない)
  it("returns null when the target id does not exist on the sheet", () => {
    const sheet = sheetWith([]);
    expect(leaderFor(sheet, note(0, 0, "対象なし", "no-such-id"), new Map())).toBeNull();
  });
});

describe("entityBounds", () => {
  // ja: シンボルの外形はその定義から求まるので、引き出し線はシンボルも対象にできる
  it("resolves a symbol instance's bounding box from its definition", () => {
    const def: SymbolDef = {
      id: "test_symbol",
      name: "Test",
      name_ja: "テスト",
      category: "misc",
      ref_prefix: "X",
      keywords: [],
      primitives: [{ type: "rect", p1: { x: -5, y: -5 }, p2: { x: 5, y: 5 }, filled: false }],
      pins: [],
      text_slots: [],
    };
    const inst: Entity = {
      kind: "symbol",
      id: "s1",
      symbol_id: "test_symbol",
      at: { x: 50, y: 50 },
      rotation: 0,
      mirror: false,
      reference: "X1",
      value: "",
      attrs: {},
    };
    const rect = entityBounds(inst, new Map([["test_symbol", def]]));
    expect(rect).toEqual({ min: { x: 45, y: 45 }, max: { x: 55, y: 55 } });
  });

  // ja: シンボルを指す引き出し線は図面に描かれている外形ちょうどで止まる: 配置基準点は必ず外形の内側に入り、シンボルの中の文字は文字の箱ではなく基準点1点として数える (Rust側 a_leader_targets_exactly_the_outline_the_drawing_shows とparity)
  it("ends on exactly the outline the drawing shows, counting a text mark as its own point", () => {
    const def: SymbolDef = {
      id: "leader_probe",
      name: "Probe",
      name_ja: "試験",
      category: "misc",
      ref_prefix: "X",
      keywords: [],
      // 外形が配置基準点を囲まず、文字が外形の外にはみ出す定義 (2つの数え方で差が出る形)
      primitives: [
        { type: "rect", p1: { x: 10, y: 10 }, p2: { x: 20, y: 20 }, filled: false },
        { type: "text", at: { x: 30, y: 0 }, text: "XYZ", height: 2.5 },
      ],
      pins: [],
      text_slots: [],
    };
    const inst: Entity = {
      kind: "symbol",
      id: "s1",
      symbol_id: "leader_probe",
      at: { x: 100, y: 100 },
      rotation: 0,
      mirror: false,
      reference: "X1",
      value: "",
      attrs: {},
    };
    expect(entityBounds(inst, new Map([["leader_probe", def]]))).toEqual({
      min: { x: 100, y: 100 },
      max: { x: 130, y: 120 },
    });
  });

  // ja: ピングループ括弧の付いたシンボルを指す引き出し線は、シンボル本体ではなく括弧の脇のキャプションの外側で止まる (括弧もキャプションもそのシンボルと一緒に描かれるため。Rust側 a_leader_stops_outside_a_symbols_pin_group_caption とparity)
  it("stops outside a symbol's pin-group caption, not on the symbol body", () => {
    const def = dynamicSymbol("terminal_block_8p")!;
    const defs = new Map([["terminal_block_8p", def]]);
    const inst = (attrs: Record<string, string>): Entity =>
      ({
        kind: "symbol",
        id: "tb1",
        symbol_id: "terminal_block_8p",
        at: { x: 100, y: 100 },
        rotation: 0,
        mirror: false,
        reference: "TB1",
        value: "",
        attrs,
      }) as Entity;
    const plainLeft = entityBounds(inst({}), defs)!.min.x;
    const target = inst({ pin_groups: "1-4:電源系統一式" });
    const groupedLeft = entityBounds(target, defs)!.min.x;
    expect(groupedLeft).toBeLessThan(plainLeft); // キャプションのぶん外形が左へ広がる

    const sheet = sheetWith([target]);
    const leader = leaderFor(sheet, note(40, 100, "左の注記", "tb1"), defs);
    expect(leader).not.toBeNull();
    expect(leader![1].x).toBeCloseTo(groupedLeft, 9);
  });

  // ja: 図形・ハーネス・配線・ジャンクションも対象にでき、盤レイアウト専用のエンティティはnull
  it("resolves shapes, harnesses, wires and junctions, and refuses panel-only entities", () => {
    const shape: Entity = {
      kind: "shape",
      id: "sh1",
      shape: "rect",
      points: [
        { x: 0, y: 0 },
        { x: 10, y: 10 },
      ],
      stroke: "default",
      stroke_width_mm: 0.3,
      dashed: false,
    };
    expect(entityBounds(shape, new Map())).toEqual({ min: { x: 0, y: 0 }, max: { x: 10, y: 10 } });

    const wire: Entity = {
      kind: "wire",
      id: "w1",
      points: [
        { x: 0, y: 5 },
        { x: 20, y: 5 },
      ],
      color: "black",
      sq: 0.75,
      length_m: null,
      part_no: null,
      net: null,
    };
    expect(entityBounds(wire, new Map())).toEqual({ min: { x: 0, y: 5 }, max: { x: 20, y: 5 } });

    const panelOutline: Entity = { kind: "panel_outline", id: "p1", at: { x: 0, y: 0 }, w: 10, h: 10, name: "" };
    expect(entityBounds(panelOutline, new Map())).toBeNull();
  });
});
