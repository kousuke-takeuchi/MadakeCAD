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
  it("place_symbolのx/yからシンボル概寸の領域を作る", () => {
    expect(toolBox(PLACE, { symbol_id: "fuse", x: 100, y: 50, reference: "F2" })).toEqual({
      min: { x: 100 - SYMBOL_EXTENT_MM, y: 50 - SYMBOL_EXTENT_MM },
      max: { x: 100 + SYMBOL_EXTENT_MM, y: 50 + SYMBOL_EXTENT_MM },
    });
  });

  it("draw_wireのpoints列のbboxを作る", () => {
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

  it("execute_commandsのadd_entityを合成する", () => {
    const b = toolBox("mcp__madakecad__execute_commands", {
      commands: [
        { type: "add_entity", sheet_id: "s1", entity: wire("w1", [{ x: 10, y: 10 }, { x: 30, y: 10 }]) },
        { type: "delete_entities", sheet_id: "s1", ids: ["x"] },
        { type: "add_entity", sheet_id: "s1", entity: symbol("k1", 60, 40) },
      ],
    });
    expect(b).toEqual({ min: { x: 10, y: 10 }, max: { x: 70, y: 50 } });
  });

  it("座標を持たないツールと不正なinputはnull", () => {
    expect(toolBox("mcp__madakecad__get_netlist", { sheet_id: "s1" })).toBeNull();
    expect(toolBox("mcp__madakecad__export_svg", { path: "/tmp/a.svg" })).toBeNull();
    expect(toolBox("mcp__madakecad__undo", {})).toBeNull();
    expect(toolBox(PLACE, { symbol_id: "fuse" })).toBeNull();
    expect(toolBox(WIRE, { points: [] })).toBeNull();
    expect(toolBox("Bash", "ls -la")).toBeNull();
  });
});

describe("entityBox", () => {
  it("wireは頂点列のbbox、symbolはat±概寸", () => {
    expect(entityBox(wire("w1", [{ x: 20, y: 30 }, { x: 20, y: 80 }]))).toEqual({
      min: { x: 20, y: 30 },
      max: { x: 20, y: 80 },
    });
    expect(entityBox(symbol("k1", 200, 100))).toEqual({
      min: { x: 190, y: 90 },
      max: { x: 210, y: 110 },
    });
  });

  it("junction/text/net_labelも領域を持つ", () => {
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
  it("ツール開始でマージン込みの領域が出る", () => {
    const ov = new AgentOverlay();
    ov.noteToolStart(PLACE, { x: 100, y: 50 }, { id: "t1", now: 1000 });
    const regions = ov.activeRegions(1000);
    expect(regions).toHaveLength(1);
    expect(regions[0].min).toEqual({ x: 100 - SYMBOL_EXTENT_MM - REGION_MARGIN_MM, y: 50 - SYMBOL_EXTENT_MM - REGION_MARGIN_MM });
    expect(regions[0].max).toEqual({ x: 100 + SYMBOL_EXTENT_MM + REGION_MARGIN_MM, y: 50 + SYMBOL_EXTENT_MM + REGION_MARGIN_MM });
    expect(regions[0].strength).toBe(1);
  });

  it("領域を作らないツールは無視される", () => {
    const ov = new AgentOverlay();
    ov.noteToolStart("mcp__madakecad__get_project", {}, { id: "t1", now: 0 });
    expect(ov.activeRegions(0)).toHaveLength(0);
    expect(ov.hasActive(0)).toBe(false);
  });

  it("同一idの再通知では領域が重複しない", () => {
    const ov = new AgentOverlay();
    ov.noteToolStart(WIRE, { points: [{ x: 0, y: 0 }, { x: 10, y: 0 }] }, { id: "t1", now: 0 });
    ov.noteToolStart(WIRE, { points: [{ x: 0, y: 0 }, { x: 10, y: 0 }] }, { id: "t1", now: 10 });
    expect(ov.activeRegions(10)).toHaveLength(1);
  });

  it("完了していないツールの領域は残り続ける", () => {
    const ov = new AgentOverlay();
    ov.noteToolStart(PLACE, { x: 10, y: 10 }, { id: "t1", now: 0 });
    expect(ov.activeRegions(10_000)).toHaveLength(1);
  });

  it("完了後HOLD_MSで消滅する", () => {
    const ov = new AgentOverlay();
    ov.noteToolStart(PLACE, { x: 10, y: 10 }, { id: "t1", now: 0 });
    ov.noteToolFinish("t1", { now: 500 });
    expect(ov.activeRegions(500)[0].strength).toBe(1);
    expect(ov.activeRegions(500 + HOLD_MS / 2)[0].strength).toBeCloseTo(0.5);
    expect(ov.activeRegions(500 + HOLD_MS)).toHaveLength(0);
    expect(ov.hasActive(500 + HOLD_MS)).toBe(false);
  });

  it("idが無い完了通知はツール名で畳む", () => {
    const ov = new AgentOverlay();
    ov.noteToolStart(WIRE, { points: [{ x: 0, y: 0 }, { x: 5, y: 5 }] }, { now: 0 });
    ov.noteToolFinish(WIRE, { now: 100 });
    expect(ov.activeRegions(100 + HOLD_MS)).toHaveLength(0);
  });

  it("entity upsertから領域を作り、HOLD_MSで消える", () => {
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

  it("同一entityの再upsertで表示期限が延びる", () => {
    const ov = new AgentOverlay();
    ov.noteEntityUpserted(wire("w1", [{ x: 0, y: 0 }, { x: 10, y: 0 }]), { now: 0 });
    ov.noteEntityUpserted(wire("w1", [{ x: 0, y: 0 }, { x: 40, y: 0 }]), { now: 1000 });
    const regions = ov.activeRegions(1000 + HOLD_MS / 2);
    expect(regions).toHaveLength(1);
    expect(regions[0].max.x).toBe(40 + REGION_MARGIN_MM);
  });

  it("finishAllで進行中の領域も期限切れになる", () => {
    const ov = new AgentOverlay();
    ov.noteToolStart(PLACE, { x: 10, y: 10 }, { id: "t1", now: 0 });
    ov.noteToolStart(WIRE, { points: [{ x: 0, y: 0 }, { x: 5, y: 0 }] }, { id: "t2", now: 0 });
    ov.finishAll(200);
    expect(ov.activeRegions(200)).toHaveLength(2);
    expect(ov.activeRegions(200 + HOLD_MS)).toHaveLength(0);
  });

  it("clearで全領域が消える", () => {
    const ov = new AgentOverlay();
    ov.noteToolStart(PLACE, { x: 1, y: 1 }, { id: "t1", now: 0 });
    ov.clear();
    expect(ov.activeRegions(0)).toHaveLength(0);
  });
});

describe("pulseAlpha", () => {
  it("常に0.1〜0.25の範囲でsin波を描く", () => {
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
  it("元の箱を変更せずマージンを足す", () => {
    const src = { min: { x: 0, y: 0 }, max: { x: 10, y: 10 } };
    const out = expandBox(src, 2);
    expect(out).toEqual({ min: { x: -2, y: -2 }, max: { x: 12, y: 12 } });
    expect(src).toEqual({ min: { x: 0, y: 0 }, max: { x: 10, y: 10 } });
  });
});
