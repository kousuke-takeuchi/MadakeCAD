// 参照サーフィン (Surfer) ポップアップの仕様テスト
// (計画: docs/superpowers/plans/2026-08-22-m4-phase2-macros-xref-nav.md Task 4)。
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
import { ipc, type DeviceNode, type Entity, type Project, type Sheet } from "../ipc";
import { useSurferStore } from "./surfer";

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

function symbol(id: string, reference: string): Entity {
  return {
    kind: "symbol",
    id,
    symbol_id: "relay_coil",
    at: { x: 100, y: 100 },
    rotation: 0,
    mirror: false,
    reference,
    value: "",
    attrs: {},
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

function project(): Project {
  return {
    format_version: 1,
    name: "t",
    sheets: [
      sheet("s1", "シート1", [symbol("coil", "K1"), symbol("plain", "")]),
      sheet("s2", "シート2", [symbol("contact", "K1")]),
    ],
    wire_parts: [],
    plc_assignments: [],
  };
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.restoreAllMocks();
  vi.spyOn(ipc, "getDeviceTree").mockResolvedValue([K1]);
});

describe("Surferポップアップ", () => {
  // ja: 参照記号のあるシンボルをAlt+クリックすると、同じデバイスの所在一覧が開く
  it("Alt-clicking a symbol with a designator opens the list of where that device appears", async () => {
    const surfer = useSurferStore();
    expect(surfer.open).toBe(false);
    expect(await surfer.openAt(project(), "s1", "coil", { x: 120, y: 80 })).toBe(true);
    expect(surfer.open).toBe(true);
    expect(surfer.target?.title).toBe("K1");
    expect(surfer.sites).toHaveLength(2);
    expect(surfer.at).toEqual({ x: 120, y: 80 });
  });

  // ja: 巡回はクリックした要素そのものから始まる
  it("the walk starts at the element that was clicked", async () => {
    const surfer = useSurferStore();
    await surfer.openAt(project(), "s2", "contact", { x: 0, y: 0 });
    expect(surfer.index).toBe(1);
    expect(surfer.activeSite?.entityId).toBe("contact");
  });

  // ja: 巡回先の無い要素ではポップアップを出さない
  it("an element with nothing to surf does not open the popup", async () => {
    const surfer = useSurferStore();
    expect(await surfer.openAt(project(), "s1", "plain", { x: 0, y: 0 })).toBe(false);
    expect(surfer.open).toBe(false);
  });

  // ja: ↑↓で所在を巡回し、端まで行くと回り込む
  it("the arrow keys walk through the locations and wrap around", async () => {
    const surfer = useSurferStore();
    await surfer.openAt(project(), "s1", "coil", { x: 0, y: 0 });
    expect(surfer.step(1)?.entityId).toBe("contact");
    expect(surfer.step(1)?.entityId).toBe("coil");
    expect(surfer.step(-1)?.entityId).toBe("contact");
  });

  // ja: 行をクリックするとその所在が選ばれ、範囲外の行は無視される
  it("clicking a row picks that location and out-of-range rows are ignored", async () => {
    const surfer = useSurferStore();
    await surfer.openAt(project(), "s1", "coil", { x: 0, y: 0 });
    expect(surfer.select(1)?.entityId).toBe("contact");
    expect(surfer.index).toBe(1);
    expect(surfer.select(5)).toBeNull();
    expect(surfer.index).toBe(1);
  });

  // ja: Escで閉じると巡回位置も先頭へ戻る
  it("closing with Escape also resets the walk position", async () => {
    const surfer = useSurferStore();
    await surfer.openAt(project(), "s1", "coil", { x: 0, y: 0 });
    surfer.step(1);
    surfer.close();
    expect(surfer.open).toBe(false);
    expect(surfer.index).toBe(0);
    expect(surfer.activeSite).toBeNull();
  });
});
