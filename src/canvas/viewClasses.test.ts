// レイヤ=固定の表示クラス (spec §4)。エンティティ→クラス対応の検証。
import { describe, expect, it } from "vitest";

import type { Entity } from "../ipc";
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

  // ja: VIEW_CLASSESは全クラスを一意に列挙する
  it("VIEW_CLASSES enumerates every class exactly once", () => {
    expect(VIEW_CLASSES.map((c) => c.id)).toEqual([
      "wires",
      "symbols",
      "refs",
      "net_labels",
      "texts",
      "frame",
      "grid",
    ]);
    for (const c of VIEW_CLASSES) {
      expect(c.label.length).toBeGreaterThan(0);
    }
  });
});
