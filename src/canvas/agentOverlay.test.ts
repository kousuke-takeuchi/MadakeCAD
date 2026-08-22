import { describe, it, expect } from "vitest";
import type { Entity } from "../ipc";
import {
  AgentOverlay,
  HOLD_MS,
  PULSE_MAX_ALPHA,
  PULSE_MIN_ALPHA,
  REGION_MARGIN_MM,
  SYMBOL_EXTENT_MM,
  entityBox,
  expandBox,
  pulseAlpha,
  toolBox,
} from "./agentOverlay";
import { agentColorAt, agentPalette, theme } from "./theme";

const PLACE = "mcp__madakecad__place_symbol";
const WIRE = "mcp__madakecad__draw_wire";

function wire(id: string, points: { x: number; y: number }[]): Entity {
  return {
    kind: "wire",
    id,
    points,
    color: "red",
    sq: 0.75,
    length_m: null,
    part_no: null,
    net: null,
  };
}

function symbol(id: string, x: number, y: number): Entity {
  return {
    kind: "symbol",
    id,
    symbol_id: "fuse",
    at: { x, y },
    rotation: 0,
    mirror: false,
    reference: "F2",
    value: "",
    attrs: {},
  };
}

describe("toolBox", () => {
  // ja: place_symbolのx/yからシンボル概寸の領域を作る
  it("place_symbol's x/y produces a region around the approximate symbol size", () => {
    expect(toolBox(PLACE, { symbol_id: "fuse", x: 100, y: 50, reference: "F2" })).toEqual({
      min: { x: 100 - SYMBOL_EXTENT_MM, y: 50 - SYMBOL_EXTENT_MM },
      max: { x: 100 + SYMBOL_EXTENT_MM, y: 50 + SYMBOL_EXTENT_MM },
    });
  });

  // ja: draw_wireのpoints列のバウンディングボックスを作る
  it("draw_wire's point list produces its bounding box", () => {
    const b = toolBox(WIRE, {
      points: [
        { x: 40, y: 100 },
        { x: 90, y: 100 },
        { x: 90, y: 70 },
      ],
      color: "red",
      sq: 0.75,
    });
    expect(b).toEqual({ min: { x: 40, y: 70 }, max: { x: 90, y: 100 } });
  });

  // ja: execute_commands内のadd_entityを合成した領域になる
  it("execute_commands regions merge the add_entity commands inside", () => {
    const b = toolBox("mcp__madakecad__execute_commands", {
      commands: [
        { type: "add_entity", sheet_id: "s1", entity: wire("w1", [{ x: 10, y: 10 }, { x: 30, y: 10 }]) },
        { type: "delete_entities", sheet_id: "s1", ids: ["x"] },
        { type: "add_entity", sheet_id: "s1", entity: symbol("k1", 60, 40) },
      ],
    });
    expect(b).toEqual({ min: { x: 10, y: 10 }, max: { x: 70, y: 50 } });
  });

  // ja: 座標を持たないツールや不正なinputは領域を作らない
  it("tools without coordinates and malformed input produce no region", () => {
    expect(toolBox("mcp__madakecad__get_netlist", { sheet_id: "s1" })).toBeNull();
    expect(toolBox("mcp__madakecad__export_svg", { path: "/tmp/a.svg" })).toBeNull();
    expect(toolBox("mcp__madakecad__undo", {})).toBeNull();
    expect(toolBox(PLACE, { symbol_id: "fuse" })).toBeNull();
    expect(toolBox(WIRE, { points: [] })).toBeNull();
    expect(toolBox("Bash", "ls -la")).toBeNull();
  });
});

describe("entityBox", () => {
  // ja: ワイヤは頂点列のbbox、シンボルは基準点±概寸で領域を作る
  it("wires use their vertex bbox; symbols use the anchor plus approximate size", () => {
    expect(entityBox(wire("w1", [{ x: 20, y: 30 }, { x: 20, y: 80 }]))).toEqual({
      min: { x: 20, y: 30 },
      max: { x: 20, y: 80 },
    });
    expect(entityBox(symbol("k1", 200, 100))).toEqual({
      min: { x: 190, y: 90 },
      max: { x: 210, y: 110 },
    });
  });

  // ja: ジャンクション・テキスト・ネットラベルも領域を持つ
  it("junctions, texts and net labels also get regions", () => {
    const junction: Entity = { kind: "junction", id: "j1", at: { x: 50, y: 50 } };
    expect(entityBox(junction)).toEqual({ min: { x: 48.5, y: 48.5 }, max: { x: 51.5, y: 51.5 } });
    const text: Entity = {
      kind: "text",
      id: "t1",
      at: { x: 10, y: 20 },
      text: "AB",
      height: 5,
      rotation: 0,
    };
    expect(entityBox(text)).toEqual({ min: { x: 10, y: 15 }, max: { x: 16, y: 20 } });
  });
});

describe("AgentOverlay", () => {
  // ja: ツール開始でマージン込みの領域が表示される
  it("a tool start shows a region with margin included", () => {
    const ov = new AgentOverlay();
    ov.noteToolStart(PLACE, { x: 100, y: 50 }, { id: "t1", now: 1000 });
    const regions = ov.activeRegions(1000);
    expect(regions).toHaveLength(1);
    expect(regions[0].min).toEqual({ x: 100 - SYMBOL_EXTENT_MM - REGION_MARGIN_MM, y: 50 - SYMBOL_EXTENT_MM - REGION_MARGIN_MM });
    expect(regions[0].max).toEqual({ x: 100 + SYMBOL_EXTENT_MM + REGION_MARGIN_MM, y: 50 + SYMBOL_EXTENT_MM + REGION_MARGIN_MM });
    expect(regions[0].strength).toBe(1);
  });

  // ja: 領域を作らないツールは無視される
  it("tools that yield no region are ignored", () => {
    const ov = new AgentOverlay();
    ov.noteToolStart("mcp__madakecad__get_project", {}, { id: "t1", now: 0 });
    expect(ov.activeRegions(0)).toHaveLength(0);
    expect(ov.hasActive(0)).toBe(false);
  });

  // ja: 同一idの再通知では領域が重複しない
  it("re-notifying the same tool id does not duplicate the region", () => {
    const ov = new AgentOverlay();
    ov.noteToolStart(WIRE, { points: [{ x: 0, y: 0 }, { x: 10, y: 0 }] }, { id: "t1", now: 0 });
    ov.noteToolStart(WIRE, { points: [{ x: 0, y: 0 }, { x: 10, y: 0 }] }, { id: "t1", now: 10 });
    expect(ov.activeRegions(10)).toHaveLength(1);
  });

  // ja: 完了していないツールの領域は残り続ける
  it("regions of unfinished tools persist", () => {
    const ov = new AgentOverlay();
    ov.noteToolStart(PLACE, { x: 10, y: 10 }, { id: "t1", now: 0 });
    expect(ov.activeRegions(10_000)).toHaveLength(1);
  });

  // ja: 完了後HOLD_MS経過で領域は消える
  it("regions fade out HOLD_MS after completion", () => {
    const ov = new AgentOverlay();
    ov.noteToolStart(PLACE, { x: 10, y: 10 }, { id: "t1", now: 0 });
    ov.noteToolFinish("t1", { now: 500 });
    expect(ov.activeRegions(500)[0].strength).toBe(1);
    expect(ov.activeRegions(500 + HOLD_MS / 2)[0].strength).toBeCloseTo(0.5);
    expect(ov.activeRegions(500 + HOLD_MS)).toHaveLength(0);
    expect(ov.hasActive(500 + HOLD_MS)).toBe(false);
  });

  // ja: idの無い完了通知はツール名で対応付けて畳む
  it("completions without an id are matched by tool name", () => {
    const ov = new AgentOverlay();
    ov.noteToolStart(WIRE, { points: [{ x: 0, y: 0 }, { x: 5, y: 5 }] }, { now: 0 });
    ov.noteToolFinish(WIRE, { now: 100 });
    expect(ov.activeRegions(100 + HOLD_MS)).toHaveLength(0);
  });

  // ja: エンティティのupsertから領域を作り、HOLD_MSで消える
  it("entity upserts create regions that expire after HOLD_MS", () => {
    const ov = new AgentOverlay();
    ov.noteEntityUpserted(symbol("k1", 200, 100), { now: 0 });
    const regions = ov.activeRegions(0);
    expect(regions).toHaveLength(1);
    expect(regions[0]).toMatchObject({
      key: "entity:k1",
      min: { x: 186, y: 86 },
      max: { x: 214, y: 114 },
    });
    expect(ov.activeRegions(HOLD_MS)).toHaveLength(0);
  });

  // ja: 同一エンティティの再upsertで表示期限が延びる
  it("re-upserting the same entity extends its display deadline", () => {
    const ov = new AgentOverlay();
    ov.noteEntityUpserted(wire("w1", [{ x: 0, y: 0 }, { x: 10, y: 0 }]), { now: 0 });
    ov.noteEntityUpserted(wire("w1", [{ x: 0, y: 0 }, { x: 40, y: 0 }]), { now: 1000 });
    const regions = ov.activeRegions(1000 + HOLD_MS / 2);
    expect(regions).toHaveLength(1);
    expect(regions[0].max.x).toBe(40 + REGION_MARGIN_MM);
  });

  // ja: finishAllで進行中の領域も期限切れになる
  it("finishAll expires even in-progress regions", () => {
    const ov = new AgentOverlay();
    ov.noteToolStart(PLACE, { x: 10, y: 10 }, { id: "t1", now: 0 });
    ov.noteToolStart(WIRE, { points: [{ x: 0, y: 0 }, { x: 5, y: 0 }] }, { id: "t2", now: 0 });
    ov.finishAll(200);
    expect(ov.activeRegions(200)).toHaveLength(2);
    expect(ov.activeRegions(200 + HOLD_MS)).toHaveLength(0);
  });

  // ja: clearで全領域が消える
  it("clear removes every region", () => {
    const ov = new AgentOverlay();
    ov.noteToolStart(PLACE, { x: 1, y: 1 }, { id: "t1", now: 0 });
    ov.clear();
    expect(ov.activeRegions(0)).toHaveLength(0);
  });
});

describe("pulseAlpha", () => {
  // ja: パルスの透明度は常に0.1〜0.25のsin波に収まる
  it("the pulse alpha stays within 0.1-0.25 following a sine wave", () => {
    for (let t = 0; t <= 3200; t += 37) {
      const a = pulseAlpha(t);
      expect(a).toBeGreaterThanOrEqual(PULSE_MIN_ALPHA - 1e-9);
      expect(a).toBeLessThanOrEqual(PULSE_MAX_ALPHA + 1e-9);
    }
    // 立ち上がりは中央値、1/4周期で上限
    expect(pulseAlpha(0)).toBeCloseTo((PULSE_MIN_ALPHA + PULSE_MAX_ALPHA) / 2);
    expect(pulseAlpha(400)).toBeCloseTo(PULSE_MAX_ALPHA);
    expect(pulseAlpha(1200)).toBeCloseTo(PULSE_MIN_ALPHA);
  });
});

describe("expandBox", () => {
  // ja: 元の矩形を変更せずマージンを足す
  it("margin is added without mutating the original box", () => {
    const src = { min: { x: 0, y: 0 }, max: { x: 10, y: 10 } };
    const out = expandBox(src, 2);
    expect(out).toEqual({ min: { x: -2, y: -2 }, max: { x: 12, y: 12 } });
    expect(src).toEqual({ min: { x: 0, y: 0 }, max: { x: 10, y: 10 } });
  });
});

describe("並列エージェント: 会話ごとの色", () => {
  // ja: 会話の色を指定すると、その会話の編集領域はその色で描かれる
  it("a region is painted in the color of the conversation that made the edit", () => {
    const ov = new AgentOverlay();
    ov.noteToolStart(PLACE, { x: 10, y: 10 }, { id: "t1", now: 0, color: agentColorAt(1) });
    expect(ov.activeRegions(0)[0].color).toBe(agentColorAt(1));
  });

  // ja: 色を指定しない編集領域は既定色(会話1本目の色)になる
  it("a region without a conversation color falls back to the default agent color", () => {
    const ov = new AgentOverlay();
    ov.noteToolStart(PLACE, { x: 10, y: 10 }, { id: "t1", now: 0 });
    expect(ov.activeRegions(0)[0].color).toBe(theme.agent);
    expect(theme.agent).toBe(agentColorAt(0));
  });

  // ja: 同時に走る2会話の編集領域は、それぞれの色を保ったまま並ぶ
  it("regions of two conversations running at once keep their own colors side by side", () => {
    const ov = new AgentOverlay();
    ov.noteToolStart(PLACE, { x: 10, y: 10 }, { id: "a1", now: 0, color: agentColorAt(0) });
    ov.noteToolStart(WIRE, { points: [{ x: 60, y: 60 }, { x: 90, y: 60 }] }, {
      id: "b1",
      now: 0,
      color: agentColorAt(1),
    });
    ov.noteEntityUpserted(symbol("k9", 120, 40), { now: 0, color: agentColorAt(2) });

    const colors = ov.activeRegions(0).map((r) => r.color);
    expect(colors).toEqual([agentColorAt(0), agentColorAt(1), agentColorAt(2)]);
  });

  // ja: 会話の色は開始順(0始まり)で決まり、5本目からは先頭の色へ戻る
  it("conversation colors follow the start order and wrap around after the fourth", () => {
    expect(agentPalette).toHaveLength(4);
    expect(new Set(agentPalette).size).toBe(4);
    expect(agentColorAt(4)).toBe(agentColorAt(0));
    expect(agentColorAt(5)).toBe(agentColorAt(1));
    expect(agentColorAt(-1)).toBe(agentColorAt(3));
  });
});
