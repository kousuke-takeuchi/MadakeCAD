// 電線品番凡例ブロック (G9残) の幾何計算。
// Rust側 madake-core/src/wire_legend.rs と同じ規則で、画面とSVG/PDF出力の見た目を一致させる:
//   - 中身は持たない。線色・sq・品番ごとに配線を集約し、使用長を合算した行を描画のたびに作る
//   - 線色は図面へ印字する名前へそろえてからまとめる (wireNotes.ts の colorDisplay)。
//     同じシートの配線注記と同じ色名になり、`red`と`赤`が2行に割れない
//   - 行は線色→線径→電線品番の順に並ぶ (線色+線径が同じでも品番が違えば別の行になるため)
//   - 品番はワイヤ自身の指定 > 電線品番表 (wireParts.ts) の引き当ての順で決まる
//   - 使用長は m 単位・小数第2位 (BOMの電線集約行と同じ書式)
// 図面への反映は必ず Command 経由 (CLAUDE.md「アーキテクチャの絶対原則」)。

import type { Command, Entity, Point, Sheet, Wire, WireLegend, WirePart } from "../ipc";
import { colorDisplay } from "./wireNotes";
import { findWirePart } from "./wireParts";

/** 凡例ブロックの列見出し (`legendRows`と同じ並び)。 */
export const LEGEND_COLUMNS = ["線色", "sq", "電線品番", "使用長 m"] as const;
/** 列幅 (mm)。合計が表の幅。 */
export const LEGEND_COL_W = [24, 14, 30, 22] as const;
/** 行の高さ (mm)。列見出しの行も同じ高さ。 */
export const LEGEND_ROW_H = 5;
/** 文字高さ (mm)。 */
export const LEGEND_FONT = 2;

/** 用紙座標の矩形 (左上+寸法) mm。 */
export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** 数値をCSV/表示向けに整形 (整数なら小数点なし、0は空欄)。Rust側 fmt_num と同じ規則。 */
function fmtNum(v: number): string {
  if (v === 0) return "";
  if (Math.abs(v - Math.round(v)) < 1e-9) return String(Math.round(v));
  return String(v);
}

/** シート上の電線品番凡例ブロック (エンティティのまま)。 */
export const wireLegends = (sheet: Sheet) =>
  Object.values(sheet.entities).filter((e): e is Extract<Entity, { kind: "wire_legend" }> =>
    e.kind === "wire_legend",
  );

function wires(sheet: Sheet): Wire[] {
  return Object.values(sheet.entities).filter((e): e is Extract<Entity, { kind: "wire" }> => e.kind === "wire");
}

/**
 * 凡例ブロックに並ぶ行 (`LEGEND_COLUMNS`と同じ並び)。そのシートの配線を
 * (線色, 線径sq, 電線品番) の組ごとにまとめ、使用長 (m) を合算する。線色→線径の順に並ぶ。
 */
export function legendRows(sheet: Sheet, wireParts: WirePart[] | undefined): string[][] {
  const totals = new Map<
    string,
    { color: string; sq: string; sqValue: number; partNo: string; total: number }
  >();
  for (const w of wires(sheet)) {
    // 図面へ印字する色名にそろえてからまとめる (配線注記と同じ表記。`red`と`赤`は同じ行)
    const color = colorDisplay(w.color ?? "");
    const sqValue = w.sq ?? 0;
    const sq = fmtNum(sqValue);
    const own = (w.part_no ?? "").trim();
    const partNo = own || (findWirePart(wireParts, w.color ?? "", sqValue)?.part_no ?? "");
    const key = JSON.stringify([color, sq, partNo]);
    const row = totals.get(key) ?? { color, sq, sqValue, partNo, total: 0 };
    row.total += w.length_m ?? 0;
    totals.set(key, row);
  }
  // 線色→線径の順 (線径は文字列ではなく実際の数値で比較し、二桁以上のsqでも正しく並ぶ)
  return [...totals.values()]
    .sort((a, b) => a.color.localeCompare(b.color) || a.sqValue - b.sqValue || a.partNo.localeCompare(b.partNo))
    // 使用長は合算なので小数第2位で止める (BOMの電線集約行と同じ書式。Rust側と同じ規則)。
    // 整形せずに書くと二進小数の誤差がそのまま出る (0.4を8本足すと3.1999999999999997)
    .map((r) => [r.color, r.sq, r.partNo, r.total.toFixed(2)]);
}

/** 凡例ブロックの矩形。列見出しの1行＋本文の行数ぶんだけ下へ伸びる。 */
export function legendRect(sheet: Sheet, wireParts: WirePart[] | undefined, legend: WireLegend): Rect {
  const w = LEGEND_COL_W.reduce((a, b) => a + b, 0);
  return {
    x: legend.at.x,
    y: legend.at.y,
    w,
    h: LEGEND_ROW_H * (1 + legendRows(sheet, wireParts).length),
  };
}

/**
 * 凡例ブロックを1つ置くコマンド。中身 (線色・sq・電線品番・使用長合計) は描画のたびに
 * 同じシートの配線から導出するので、ここでは置き場所だけを決める。
 */
export function wireLegendAddCommand(sheet: Sheet, at: Point): Command {
  return {
    type: "add_entity",
    sheet_id: sheet.id,
    entity: { kind: "wire_legend", id: crypto.randomUUID(), at },
  };
}
