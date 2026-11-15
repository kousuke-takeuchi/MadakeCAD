// シートごとの配線の描き方 (Rust 側 model.rs の Sheet::wire_weight_mm / hide_wire_notes と同じ規則)
import { describe, expect, it } from "vitest";

import type { Sheet } from "../ipc";
import { junctionDotRMm } from "./renderer";
import { DEFAULT_WIRE_WEIGHT_MM, hidesWireNotes, symbolWeightMm, vividWireColor, vividWireColorScreen, wireWeightMm, wireWeightOf } from "./wireStyle";

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

describe("junction dot on thick wires", () => {
  // ja: 分岐点の丸は、既定の配線の太さでは半径0.6mmのまま、太い配線のシートでは太さの0.9倍になる
  it("keeps 0.6 mm at the default weight and grows to 0.9 times a thick wire", () => {
    expect(junctionDotRMm(sheet({}))).toBe(0.6);
    expect(junctionDotRMm(sheet({ wire_weight_mm: 0.5 }))).toBe(0.6);
    expect(junctionDotRMm(sheet({ wire_weight_mm: 1.0 }))).toBeCloseTo(0.9, 9);
  });
});

describe("outline weight, own wire weight and vivid colours", () => {
  // ja: シンボルの外形は、太さを設定していないシートでは0.3mm、設定したシートではその太さになる。範囲外は0.3mm
  it("uses 0.3 mm for outlines unless the sheet sets a weight in range", () => {
    expect(symbolWeightMm(sheet({}))).toBe(0.3);
    expect(symbolWeightMm(sheet({ symbol_weight_mm: 1.0 }))).toBe(1.0);
    expect(symbolWeightMm(sheet({ symbol_weight_mm: 3 }))).toBe(0.3);
  });

  // ja: 配線は自分の太さ (0.18〜2.0mm) があればそれ、無ければシートの太さで描く
  it("draws a wire at its own weight when it has one, else at the sheet's", () => {
    const s = sheet({ wire_weight_mm: 0.7 });
    expect(wireWeightOf(s, {})).toBe(0.7);
    expect(wireWeightOf(s, { weight_mm: null })).toBe(0.7);
    expect(wireWeightOf(s, { weight_mm: 2.0 })).toBe(2.0);
    expect(wireWeightOf(s, { weight_mm: 2.5 })).toBe(0.7);
    expect(wireWeightOf(null, { weight_mm: 0.18 })).toBe(0.18);
  });

  // ja: 鮮やかな色は純色の表で決まる。知らない色名・黒・#始まりは表に無い (いつもの色を使う)
  it("maps colour names to pure colours and leaves unknown names alone", () => {
    expect(vividWireColor("yellow")).toBe("#ffff00");
    expect(vividWireColor("light_blue")).toBe("#00ffff");
    expect(vividWireColor("桃")).toBe("#ff00ff");
    expect(vividWireColor("black")).toBeNull();
    expect(vividWireColor("#123456")).toBeNull();
  });
});

describe("vivid colours on the dark canvas", () => {
  // ja: キャンバスでは、暗い背景で見えにくい純色 (青・茶・紫) は使わず、いつもの画面用の色で描く。黄・緑・水色・桃・赤・橙は純色で描く
  it("keeps the screen colour for blue, brown and purple and uses pure colours for the rest", () => {
    for (const dark of ["blue", "青", "brown", "purple"]) expect(vividWireColorScreen(dark), dark).toBeNull();
    expect(vividWireColorScreen("yellow")).toBe("#ffff00");
    expect(vividWireColorScreen("light_blue")).toBe("#00ffff");
    expect(vividWireColorScreen("pink")).toBe("#ff00ff");
    expect(vividWireColorScreen("red")).toBe("#ff0000");
    expect(vividWireColorScreen("black")).toBeNull();
  });
});

