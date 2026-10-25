// ブロックのピンのコネクタの絵 (Rust側 madake-core/src/connector.rs と対。同じ数・同じ順)。
// 両者が同じ形を返すことは src-tauri/crates/madake-core/tests/golden/connector-blocks.json で突き合わせる
// (dynamicSymbol.test.ts)。型だけを dynamicSymbol.ts から読むので、実行時の循環importは無い。

import type { BlockPin } from "./dynamicSymbol";

/** コネクタの形 (仕様の表の順。エディタの選択肢もこの順)。`_f` = メス、`_m` = オス。 */
export const CONNECTOR_KINDS = [
  "usb_a_f",
  "usb_a_m",
  "usb_c_f",
  "usb_c_m",
  "micro_b_f",
  "micro_b_m",
  "jack35_f",
  "jack35_m",
  "dc_f",
  "dc_m",
  "screw",
  "jst_ph_f",
  "jst_xh_f",
] as const;

export type ConnectorKind = (typeof CONNECTOR_KINDS)[number];

/** 知っているコネクタの名前か。 */
export function isConnectorKind(value: unknown): value is ConnectorKind {
  return typeof value === "string" && (CONNECTOR_KINDS as readonly string[]).includes(value);
}

/** 並んだ数本のピンで1つの本体を描く種類 (端子台・JST)。`connector_ref` はこれだけが持てる。 */
export function isGroupedConnector(kind: ConnectorKind): boolean {
  return kind === "screw" || kind === "jst_ph_f" || kind === "jst_xh_f";
}

/** JST のシリーズ名 (`PH` / `XH`)。他はnull。 */
export function connectorSeries(kind: ConnectorKind): string | null {
  if (kind === "jst_ph_f") return "PH";
  if (kind === "jst_xh_f") return "XH";
  return null;
}

/** 1辺の中の、1つの絵で描くピンのまとまり (`start`〜`end` の手前までが辺のピン表の添字)。 */
export interface ConnectorGroup {
  kind: ConnectorKind;
  /** 前後の空白を落とした connector_ref。無ければnull。 */
  ref: string | null;
  start: number;
  end: number;
}

/** 1辺のピンをまとまりに分ける (Rust側 connector_groups と同じ規則)。 */
export function connectorGroups(side: BlockPin[]): ConnectorGroup[] {
  const groups: ConnectorGroup[] = [];
  side.forEach((pin, i) => {
    const kind = pin.connector;
    if (kind == null) return;
    const ref = pin.connector_ref == null ? null : pin.connector_ref.trim();
    const last = groups[groups.length - 1];
    if (isGroupedConnector(kind) && last && last.end === i && last.kind === kind && last.ref === ref) {
      last.end = i + 1;
      return;
    }
    groups.push({ kind, ref, start: i, end: i + 1 });
  });
  return groups;
}

/** まとまりの名前 (JST は `<ref> PH` / `PH`、端子台は `<ref>`、無ければnull)。 */
export function groupLabel(group: ConnectorGroup): string | null {
  const series = connectorSeries(group.kind);
  if (series !== null) return group.ref !== null ? `${group.ref} ${series}` : series;
  if (group.ref !== null && isGroupedConnector(group.kind)) return group.ref;
  return null;
}
