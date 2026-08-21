// レイヤ=固定の表示クラス (spec §4)。エンティティ→クラス対応の検証。
import { createPinia, setActivePinia } from "pinia";
import { describe, expect, it, vi } from "vitest";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

import type { Entity } from "../ipc";
import { useUiStore } from "../stores/ui";
import { entityViewClass, VIEW_CLASSES } from "./viewClasses";

const e = (kind: string): Entity => ({ kind }) as unknown as Entity;

describe("entityViewClass", () => {
  // ja: エンティティ種別を表示クラスへ対応付ける(ジャンクションは配線扱い)
  it("entity kinds map to view classes (junctions count as wires)", () => {
    expect(entityViewClass(e("wire"))).toBe("wires");
    expect(entityViewClass(e("junction"))).toBe("wires");
    expect(entityViewClass(e("symbol"))).toBe("symbols");
    expect(entityViewClass(e("net_label"))).toBe("net_labels");
    expect(entityViewClass(e("text"))).toBe("texts");
  });

  // ja: VIEW_CLASSESは全クラスを一意に列挙する(線番を加えて8クラス)
  it("VIEW_CLASSES enumerates every class exactly once", () => {
    expect(VIEW_CLASSES.map((c) => c.id)).toEqual([
      "wires",
      "symbols",
      "refs",
      "net_labels",
      "wire_numbers",
      "texts",
      "frame",
      "grid",
    ]);
    expect(VIEW_CLASSES).toHaveLength(8);
    for (const c of VIEW_CLASSES) {
      expect(c.label.length).toBeGreaterThan(0);
    }
  });
});

describe("view class visibility", () => {
  // ja: 表示クラスは初期状態で全て表示ONになっている(線番も最初から見える)
  it("shows every view class, including wire numbers, until it is switched off", () => {
    setActivePinia(createPinia());
    const ui = useUiStore();
    for (const c of VIEW_CLASSES) expect(ui.isClassVisible(c.id), c.id).toBe(true);
    ui.toggleViewClass("wire_numbers");
    expect(ui.isClassVisible("wire_numbers")).toBe(false);
    expect(ui.isClassVisible("wires")).toBe(true);
    ui.toggleViewClass("wire_numbers");
    expect(ui.isClassVisible("wire_numbers")).toBe(true);
  });
});
