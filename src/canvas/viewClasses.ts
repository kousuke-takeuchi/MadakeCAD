// レイヤ=固定の表示クラス (spec §4)。画面表示のみでSVG/PDF出力へは反映しない。

import type { Entity } from "../ipc";

export type ViewClass =
  | "wires"
  | "symbols"
  | "refs"
  | "net_labels"
  | "texts"
  | "frame"
  | "grid";

/** リボン「表示」タブの表示順。iconはlucide名 (デザイン: .pen「リボン - 表示タブ」)。 */
export const VIEW_CLASSES: ReadonlyArray<{ id: ViewClass; label: string; icon: string }> = [
  { id: "wires", label: "配線", icon: "route" },
  { id: "symbols", label: "シンボル", icon: "cpu" },
  { id: "refs", label: "参照記号", icon: "tag" },
  { id: "net_labels", label: "ネットラベル", icon: "hash" },
  { id: "texts", label: "注記", icon: "type" },
  { id: "frame", label: "図枠", icon: "frame" },
  { id: "grid", label: "グリッド", icon: "grid-3x3" },
];

/**
 * エンティティが属する表示クラス。ジャンクションは配線の一部として扱う。
 * "refs"(参照記号・型番)はシンボル内の注記テキストのみを指すため、ここには現れない。
 */
export function entityViewClass(e: Entity): ViewClass {
  switch (e.kind) {
    case "wire":
    case "junction":
      return "wires";
    case "symbol":
      return "symbols";
    case "net_label":
      return "net_labels";
    case "text":
      return "texts";
  }
}
