// シートごとの配線の描き方。Rust側 madake-core/src/model.rs の
// Sheet::wire_weight_mm / hide_wire_notes と同一ルール。
//
// 未設定のシートは 0.35mm・配線注記ありで、これまでの図面と変わらない。

import type { Sheet, Wire } from "../ipc";

/** 配線の線の太さの既定値 (mm)。設定の無いシートはこの太さで描かれる。 */
export const DEFAULT_WIRE_WEIGHT_MM = 0.35;
/** 配線の線の太さの下限 (mm)。 */
export const MIN_WIRE_WEIGHT_MM = 0.18;
/** 配線の線の太さの上限 (mm)。 */
export const MAX_WIRE_WEIGHT_MM = 1.0;

/** シートの配線の線の太さ (mm)。未設定・範囲外なら既定。 */
export function wireWeightMm(sheet: Sheet | null | undefined): number {
  const value = sheet?.wire_weight_mm;
  return typeof value === "number" &&
    Number.isFinite(value) &&
    value >= MIN_WIRE_WEIGHT_MM &&
    value <= MAX_WIRE_WEIGHT_MM
    ? value
    : DEFAULT_WIRE_WEIGHT_MM;
}

/** シートが配線注記 (線径・色名・長さ) を描かない設定か。 */
export function hidesWireNotes(sheet: Sheet | null | undefined): boolean {
  return sheet?.hide_wire_notes === true;
}

/** 配線1本ごとに指定する太さの上限 (mm)。 */
export const MAX_WIRE_OWN_WEIGHT_MM = 2.0;
/** シンボル・ブロックの外形の線の太さの既定値 (mm)。 */
export const DEFAULT_SYMBOL_WEIGHT_MM = 0.3;
/** 破線の配線の線と空きの長さ (mm)。svg.rs の WIRE_DASH と同じ。 */
export const WIRE_DASH: [number, number] = [4, 2.5];

/** シートのシンボル・ブロックの外形の線の太さ (mm)。未設定・範囲外なら既定。 */
export function symbolWeightMm(sheet: Sheet | null | undefined): number {
  const value = sheet?.symbol_weight_mm;
  return typeof value === "number" &&
    Number.isFinite(value) &&
    value >= MIN_WIRE_WEIGHT_MM &&
    value <= MAX_WIRE_WEIGHT_MM
    ? value
    : DEFAULT_SYMBOL_WEIGHT_MM;
}

/** この配線を描く太さ (mm): 配線自身の太さがあればそれ、無ければシートの太さ。 */
export function wireWeightOf(sheet: Sheet | null | undefined, wire: Pick<Wire, "weight_mm">): number {
  const own = wire.weight_mm;
  return typeof own === "number" && Number.isFinite(own) && own >= MIN_WIRE_WEIGHT_MM && own <= MAX_WIRE_OWN_WEIGHT_MM
    ? own
    : wireWeightMm(sheet);
}

/** 線色名→鮮やかな表示色 (純色)。知らない色名・`#`始まり・白・黒は null (いつもの色を使う)。svg.rs の color_hex_vivid と同じ表。 */
export function vividWireColor(color: string): string | null {
  switch (color) {
    case "red":
    case "赤":
      return "#ff0000";
    case "blue":
    case "青":
      return "#0000ff";
    case "yellow":
    case "黄":
      return "#ffff00";
    case "green":
    case "緑":
      return "#00ff00";
    case "orange":
    case "橙":
      return "#ff8000";
    case "purple":
    case "紫":
      return "#8000ff";
    case "brown":
    case "茶":
      return "#800000";
    case "pink":
    case "桃":
      return "#ff00ff";
    case "light_blue":
    case "sky":
    case "水":
    case "水色":
      return "#00ffff";
    default:
      return null;
  }
}

/**
 * キャンバス (暗い作図領域) で使う鮮やかな色。純色のうち暗い背景で見えにくいもの (青・茶・紫) は
 * null を返し、いつもの画面用の色 (theme.wireColorScreen) で描く。紙の出力は svg.rs の純色のまま。
 */
export function vividWireColorScreen(color: string): string | null {
  switch (color) {
    case "blue":
    case "青":
    case "brown":
    case "茶":
    case "purple":
    case "紫":
      return null;
    default:
      return vividWireColor(color);
  }
}

