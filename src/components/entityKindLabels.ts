/**
 * プロパティパネルの「種類」行に出すエンティティ種別のラベル。
 *
 * 戻り値は`Entity["kind"]`を鍵にした**全種類そろったRecord**にしてある。新しい
 * エンティティ種別を`ipc.ts`の`Entity`へ足すと、ここが足りないままではコンパイルが
 * 通らない (種別だけ出て名前が空の「 (1)」という表示にならない)。
 */
import type { Entity } from "../ipc";

export type EntityKind = Entity["kind"];

/** 種別ラベルの一覧。`t`はvue-i18nの翻訳関数。 */
export function entityKindLabels(t: (key: string) => string): Record<EntityKind, string> {
  return {
    wire: "配線",
    symbol: "シンボル",
    junction: "ジャンクション",
    net_label: "ネットラベル",
    text: "テキスト",
    harness: t("harness.kindLabel"),
    shape: t("shapes.kindLabel"),
    panel_outline: t("panel.prop.kind.panel_outline"),
    din_rail: t("panel.prop.kind.din_rail"),
    duct: t("panel.prop.kind.duct"),
    footprint: t("panel.prop.kind.footprint"),
    panel_legend: t("panel.prop.kind.panel_legend"),
    wire_legend: t("wireLegend.kindLabel"),
    cable: t("cable.kindLabel"),
    run_cable: t("cable.kindLabel"),
    nameplate: t("panelMarks.nameplate.kindLabel"),
    dimension: t("panelMarks.dimension.kindLabel"),
  };
}

/** 種別ラベルを持つ全エンティティ種別 (テスト・網羅確認用。`Entity`の並びと同じ)。 */
export const ENTITY_KINDS: EntityKind[] = [
  "symbol",
  "wire",
  "junction",
  "net_label",
  "text",
  "harness",
  "shape",
  "panel_outline",
  "din_rail",
  "duct",
  "footprint",
  "panel_legend",
  "wire_legend",
  "cable",
  "run_cable",
  "nameplate",
  "dimension",
];
