// 計算ケースを画面に出すための純ロジック (spec 2026-11-17-sim-cases-design §3)。
// 選んだ部品で編集できる値の並びと、図面に描く故障の印の位置。

import type { Point, Sheet, SimCase, SymbolDef, SymbolInstance } from "../ipc";
import { transformLocal } from "./dynamicSymbol";

const ROLE_ATTRIBUTES: Record<string, string[]> = {
  source: ["voltage", "r_int_ohm", "i_max_a"],
  load: ["current_a", "v_min"],
  fuse: ["rating_a"],
  converter: ["v_out", "eta", "i_out_a", "i_limit_a", "v_in_min"],
};

/** 部品の役割に関係する属性 (最後に必ずピン定格)。 */
export function roleAttributes(def: SymbolDef | undefined): string[] {
  const role = def?.sim?.role;
  return [...(role ? ROLE_ATTRIBUTES[role] ?? [] : []), "pin_rating_a"];
}

/** 電線の長さの半分の所。 */
function halfway(points: Point[]): Point {
  const lens = points.slice(1).map((p, i) => Math.hypot(p.x - points[i].x, p.y - points[i].y));
  let rest = lens.reduce((a, b) => a + b, 0) / 2;
  for (let i = 0; i < lens.length; i += 1) {
    if (rest <= lens[i] && lens[i] > 0) {
      const t = rest / lens[i];
      return { x: points[i].x + (points[i + 1].x - points[i].x) * t, y: points[i].y + (points[i + 1].y - points[i].y) * t };
    }
    rest -= lens[i];
  }
  return points[points.length - 1];
}

/** 故障の印 (短絡 = 電線の始点かピン、断線 = 電線の長さの半分)。対象が無いものは飛ばす。 */
export function faultPoints(
  simCase: SimCase | null,
  sheet: Sheet,
  resolve: (inst: SymbolInstance) => SymbolDef | undefined,
): { kind: "short" | "open"; at: Point }[] {
  const out: { kind: "short" | "open"; at: Point }[] = [];
  for (const f of simCase?.faults ?? []) {
    if (f.kind === "open") {
      const w = sheet.entities[f.wire_id];
      if (w && w.kind === "wire" && w.points.length >= 2) out.push({ kind: "open", at: halfway(w.points) });
    } else if ("wire_id" in f.target) {
      const w = sheet.entities[f.target.wire_id];
      if (w && w.kind === "wire" && w.points.length) out.push({ kind: "short", at: w.points[0] });
    } else {
      const target = f.target;
      const s = sheet.entities[target.entity_id];
      if (!s || s.kind !== "symbol") continue;
      const pin = resolve(s)?.pins.find((p) => p.number.trim() === target.pin.trim());
      if (pin) out.push({ kind: "short", at: transformLocal(pin.at, s) });
    }
  }
  return out;
}
