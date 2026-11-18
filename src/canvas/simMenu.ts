// シミュレーションの右クリックメニューの中身 (spec 2026-11-17-sim-cases-design §3)。
// 右クリックした電線・部品と、選んでいる計算ケースから、出す項目を決める純ロジック。

import type { Point, Sheet, SimCase, SimFault, SymbolDef, SymbolInstance } from "../ipc";
import { throughPinKey, transformLocal } from "./dynamicSymbol";

export type SimMenuItem =
  | { kind: "short_wire"; wireId: string }
  | { kind: "open_wire"; wireId: string }
  | { kind: "short_pin"; entityId: string; pin: string }
  | { kind: "remove"; fault: SimFault };

/** マウスに一番近いピンの番号 (ピンが無ければ null)。 */
export function nearestPin(inst: SymbolInstance, def: SymbolDef, world: Point): string | null {
  let best: string | null = null;
  let bestDist = Infinity;
  for (const pin of def.pins) {
    const p = transformLocal(pin.at, inst);
    const d = Math.hypot(p.x - world.x, p.y - world.y);
    if (d < bestDist) {
      best = throughPinKey(inst, def, pin.number);
      bestDist = d;
    }
  }
  return best;
}

/** 故障がその電線・部品を指すか。 */
function faultOn(f: SimFault, id: string): boolean {
  if (f.kind === "open") return f.wire_id === id;
  return "wire_id" in f.target ? f.target.wire_id === id : f.target.entity_id === id;
}

/** 右クリックで出す項目。電線・部品以外や何も無い所では空。 */
export function simMenuItems(
  hitId: string | null,
  sheet: Sheet,
  resolve: (inst: SymbolInstance) => SymbolDef | undefined,
  world: Point,
  simCase: SimCase | null,
): SimMenuItem[] {
  if (!hitId) return [];
  const e = sheet.entities[hitId];
  if (!e) return [];
  const removes: SimMenuItem[] = (simCase?.faults ?? []).filter((f) => faultOn(f, hitId)).map((fault) => ({ kind: "remove", fault }));
  if (e.kind === "wire") {
    return [{ kind: "short_wire", wireId: hitId }, { kind: "open_wire", wireId: hitId }, ...removes];
  }
  if (e.kind === "symbol") {
    const def = resolve(e);
    const pin = def ? nearestPin(e, def, world) : null;
    const items: SimMenuItem[] = pin ? [{ kind: "short_pin", entityId: hitId, pin }] : [];
    return [...items, ...removes];
  }
  return [];
}
