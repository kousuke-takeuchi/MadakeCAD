// レイヤ=固定の表示クラス (spec §4)。画面表示のみでSVG/PDF出力へは反映しない。

import type { Entity } from "../ipc";

export type ViewClass =
  | "wires"
  | "symbols"
  | "refs"
  | "net_labels"
  | "wire_numbers"
  | "wire_notes"
  | "harness"
  | "panel"
  | "texts"
  | "frame"
  | "grid";

/**
 * リボン「表示」タブの表示順。iconはlucide名 (デザイン: .pen「リボン - 表示タブ」)、
 * labelKeyはi18nのキー (`view.<id>`)。
 */
export const VIEW_CLASSES: ReadonlyArray<{
  id: ViewClass;
  labelKey: string;
  icon: string;
  /** ツールチップのi18nキー (補足が要るクラスだけ)。 */
  hintKey?: string;
}> = [
  { id: "wires", labelKey: "view.wires", icon: "route" },
  { id: "symbols", labelKey: "view.symbols", icon: "cpu" },
  { id: "refs", labelKey: "view.refs", icon: "tag" },
  { id: "net_labels", labelKey: "view.netLabels", icon: "hash" },
  { id: "wire_numbers", labelKey: "view.wireNumbers", icon: "list-ordered" },
  { id: "wire_notes", labelKey: "view.wireNotes", icon: "ruler", hintKey: "view.wireNotesHint" },
  { id: "harness", labelKey: "view.harness", icon: "square-dashed" },
  { id: "panel", labelKey: "view.panel", icon: "layout-panel-top" },
  { id: "texts", labelKey: "view.texts", icon: "type" },
  { id: "frame", labelKey: "view.frame", icon: "frame" },
  { id: "grid", labelKey: "view.grid", icon: "grid-3x3" },
];

/**
 * エンティティが属する表示クラス。ジャンクションは配線の一部として扱う。
 * "refs"(参照記号・型番)・"wire_numbers"(線番)・"wire_notes"(配線注記)は
 * エンティティではなく描画時に生成する注記なので、ここには現れない。
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
    case "harness":
      return "harness";
    case "panel_outline":
    case "din_rail":
    case "duct":
    case "footprint":
    case "panel_legend":
      return "panel";
  }
}
