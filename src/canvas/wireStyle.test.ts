// シートごとの配線の描き方 (Rust 側 model.rs の Sheet::wire_weight_mm / hide_wire_notes と同じ規則)
import { describe, expect, it } from "vitest";

import type { Sheet } from "../ipc";
import { DEFAULT_WIRE_WEIGHT_MM, hidesWireNotes, wireWeightMm } from "./wireStyle";

const sheet = (extra: Record<string, unknown>): Sheet => ({ entities: {}, ...extra }) as unknown as Sheet;

describe("sheet wire style", () => {
  // ja: 配線の太さを設定していないシートは0.35mm、設定したシートはその太さになる
  it("uses 0.35 mm when the sheet sets no wire weight, and the sheet's weight when it does", () => {
    expect(DEFAULT_WIRE_WEIGHT_MM).toBe(0.35);
    expect(wireWeightMm(sheet({}))).toBe(0.35);
    expect(wireWeightMm(sheet({ wire_weight_mm: null }))).toBe(0.35);
    expect(wireWeightMm(sheet({ wire_weight_mm: 0.7 }))).toBe(0.7);
    expect(wireWeightMm(null)).toBe(0.35);
  });

  // ja: 0.18〜1.0mm の外の太さや数でない太さは使わず、既定の0.35mmで描く
  it("falls back to 0.35 mm for a weight outside 0.18 to 1.0 mm or not a number", () => {
    for (const bad of [0, 0.1, 1.5, -0.35, Number.NaN, Number.POSITIVE_INFINITY]) {
      expect(wireWeightMm(sheet({ wire_weight_mm: bad })), String(bad)).toBe(0.35);
    }
    expect(wireWeightMm(sheet({ wire_weight_mm: 0.18 }))).toBe(0.18);
    expect(wireWeightMm(sheet({ wire_weight_mm: 1.0 }))).toBe(1.0);
  });

  // ja: 配線注記を描かない設定は、シートが true を持つときだけ有効になる
  it("hides wire notes only when the sheet says so", () => {
    expect(hidesWireNotes(sheet({}))).toBe(false);
    expect(hidesWireNotes(sheet({ hide_wire_notes: false }))).toBe(false);
    expect(hidesWireNotes(sheet({ hide_wire_notes: true }))).toBe(true);
    expect(hidesWireNotes(undefined)).toBe(false);
  });
});
