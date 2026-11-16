// 配線ツールの「グリッド外のピンへの吸着」。
//
// 配線の点は普通 2.5mm グリッドへ吸着する。接続点をグリッドに縛らないシンボル (free_pins) のピンは
// グリッドの点から外れているので、グリッドへ吸着した点では 0.01mm 以内に届かず、つながらない。
// カーソルの近くにグリッド外のピンがあれば、グリッドより先にそのピンちょうどへ吸着する。

import type { Point, Sheet, SymbolDef, SymbolInstance } from "../ipc";
import { transformLocal } from "./dynamicSymbol";

/** この距離 (mm) より近いグリッド外のピンへ吸着する。グリッドの半分。 */
export const PIN_SNAP_RADIUS_MM = 1.25;

const onGrid = (v: number): boolean => Math.abs(v / 2.5 - Math.round(v / 2.5)) < 1e-6;

/**
 * `world` (吸着前のカーソル位置) から PIN_SNAP_RADIUS_MM 以内にある、グリッド外のピンのうち一番近いものの位置。
 * 無ければ null (呼び出し側はグリッドへ吸着する)。グリッド上のピンは返さない (グリッドの吸着で届く)。
 */
export function offGridPinNear(
  sheet: Sheet | null | undefined,
  resolve: (inst: SymbolInstance) => SymbolDef | undefined,
  world: Point,
): Point | null {
  if (!sheet) return null;
  let best: Point | null = null;
  let bestDist = PIN_SNAP_RADIUS_MM;
  for (const e of Object.values(sheet.entities)) {
    if (e.kind !== "symbol") continue;
    const def = resolve(e);
    if (!def) continue;
    for (const pin of def.pins) {
      const wp = transformLocal(pin.at, e);
      if (onGrid(wp.x) && onGrid(wp.y)) continue;
      const d = Math.hypot(wp.x - world.x, wp.y - world.y);
      if (d < bestDist) {
        best = wp;
        bestDist = d;
      }
    }
  }
  return best;
}
