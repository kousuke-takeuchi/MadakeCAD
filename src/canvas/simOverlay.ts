
// シミュレーション結果を図面に重ねる表示の中身 (spec 2026-11-17-sim-overlay-design §2)。
// 結果 (SimOpResult.loads) から「エンティティ id → 色の段階と札」を作る純ロジック。描画は renderer.ts。

import type { EntityLoad, Sheet, SimOpResult } from "../ipc";

export type OverlayLevel = "none" | "near" | "over";

/** この負荷率から「注意」(橙)。 */
export const NEAR_RATIO = 0.8;

export interface OverlayItem {
  entityId: string;
  kind: "wire" | "component";
  level: OverlayLevel;
  /** 超過だけに付く札 (例 "71.0A / 50A")。 */
  label: string | null;
}

export function loadLevel(ratio: number | null): OverlayLevel {
  if (ratio === null || ratio < NEAR_RATIO) return "none";
  return ratio > 1 ? "over" : "near";
}

export function overLabel(l: EntityLoad): string {
  return `${l.amps.toFixed(1)}A / ${l.limit}A`;
}

/** 注意・超過の電線と部品 (計算の後で消えたものは除く)。 */
export function buildOverlay(result: SimOpResult, sheet: Sheet): Map<string, OverlayItem> {
  const out = new Map<string, OverlayItem>();
  for (const l of result.loads ?? []) {
    if (!(l.entity_id in sheet.entities)) continue;
    const level = loadLevel(l.ratio);
    if (level === "none") continue;
    out.set(l.entity_id, { entityId: l.entity_id, kind: l.kind, level, label: level === "over" ? overLabel(l) : null });
  }
  return out;
}

/** プローブ (吹き出し) の文字。電線の電圧は「始点 → 終点」、部品はピンの順に「/」で区切る。 */
export function formatProbe(l: EntityLoad): { amps: string; volts: string; limit: string | null; ratio: string | null } {
  const v = (x: number) => x.toFixed(2);
  const volts = l.kind === "wire" && l.volts.length === 2 ? `${v(l.volts[0])} V → ${v(l.volts[1])} V` : `${l.volts.map(v).join(" / ")} V`;
  return {
    amps: `${l.amps.toFixed(2)} A`,
    volts,
    limit: l.limit === null ? null : `${l.limit} A`,
    ratio: l.ratio === null ? null : `${Math.round(l.ratio * 100)}%`,
  };
}
