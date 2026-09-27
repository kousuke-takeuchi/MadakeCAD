// 電線品番の引き当て (G11)。Rust側 `madake-core/src/wire_parts.rs` と同じ規則を、
// プロパティ表示のためにフロントでも引けるようにしたもの。
//
// - ワイヤ自身に書かれた品番が最優先。表から引くのは品番が空のワイヤだけ
// - 線色は前後の空白を無視・英字の大小と英日の色名を同一視 (light_blue = 水色)、
//   線径は完全一致 (浮動小数の誤差まで)
// - 同じ組み合わせが2行あれば先に書いた行が勝つ。無ければ空 (勝手に品番を作らない)
import type { Project, WirePart } from "../ipc";
import { colorDisplay } from "./wireNotes";

/** 線径 (sq) の一致とみなす誤差。 */
export const SQ_EPS = 1e-9;

/** 線色の比較キー。空白を落として小文字化し、日本語の色名へそろえる。 */
function colorKey(color: string): string {
  // 図面へ色名を印字する関数と同じなので、引き当てのキーと図面の表記は必ず一致する
  return colorDisplay(color);
}

/** 線色+線径で電線品番表を引く (先に書いた行が勝つ)。 */
export function findWirePart(
  parts: WirePart[] | undefined,
  color: string,
  sq: number,
): WirePart | null {
  const key = colorKey(color ?? "");
  if (!key || !(sq > 0)) return null;
  return (parts ?? []).find((p) => colorKey(p.color) === key && Math.abs(p.sq - sq) < SQ_EPS) ?? null;
}

/**
 * 線色+線径から自動で決まる電線品番。ワイヤの品番欄が空のときにこれが使われる
 * (帳票の品番列と同じ規則)。表に無ければ空文字。
 */
export function autoWirePartNo(project: Project | null, color: string, sq: number): string {
  return findWirePart(project?.wire_parts, color, sq)?.part_no ?? "";
}

/** ワイヤに載る電線品番。手書きの品番 > 表からの引き当て > 空。 */
export function wirePartNo(
  project: Project | null,
  wire: { color?: string; sq?: number; part_no?: string | null },
): string {
  const own = (wire.part_no ?? "").trim();
  if (own) return own;
  return autoWirePartNo(project, wire.color ?? "", wire.sq ?? 0);
}
