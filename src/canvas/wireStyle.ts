// シートごとの配線の描き方。Rust側 madake-core/src/model.rs の
// Sheet::wire_weight_mm / hide_wire_notes と同一ルール。
//
// 未設定のシートは 0.35mm・配線注記ありで、これまでの図面と変わらない。

import type { Sheet } from "../ipc";

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
