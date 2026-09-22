// 部品挿入ダイアログの極数バーの規則 (SymbolPickerDialog.vueが使う)。
// 極数・段数の丸めと、そこから決まる配置シンボルidを1か所にまとめてテストできるようにする。
import { DYNAMIC_PIN_MAX, TERMINAL_LEVEL_MAX } from "../canvas/dynamicSymbol";

/** 極数バーを持つ可変シンボルの一族。 */
export type DynamicBase = "terminal_block" | "connector";

/** 極数ステッパーの値を1〜`DYNAMIC_PIN_MAX`極の整数へ収める。 */
export function clampPins(n: number): number {
  return Math.min(DYNAMIC_PIN_MAX, Math.max(1, Math.round(n) || 1));
}

/** 段数ステッパーの値を1〜`TERMINAL_LEVEL_MAX`段の整数へ収める (G6)。 */
export function clampLevels(n: number): number {
  return Math.min(TERMINAL_LEVEL_MAX, Math.max(1, Math.round(n) || 1));
}

/** 段数ステッパーを出す一族か (段は端子台だけの概念)。 */
export function supportsLevels(base: DynamicBase | null): boolean {
  return base === "terminal_block";
}

/**
 * 極数バーの現在値で配置するシンボルid。
 * 段数1は既存の単段id (`terminal_block_{n}p`) のままにして、既存プロジェクトとの互換を保つ。
 */
export function dynamicSymbolId(base: DynamicBase, pins: number, levels: number): string {
  if (supportsLevels(base) && levels > 1) return `${base}_${pins}p_${levels}l`;
  return `${base}_${pins}p`;
}
