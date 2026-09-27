// FreeCAD対応付け (M5-2/M5-3) の表示用整形。FreeCADアドオン側の
// `madakecad_link/routes.py::format_placement` と同じ文面を作る双子の実装なので、
// 片方だけ直すとパネルとプロパティで表示が食い違う (テストで固定してある)。

import type { MechLink, MechPlacement } from "../ipc";

/**
 * 座標・角度の表示値。Pythonの`%g`と同じで、有効6桁に丸めて余分な0を落とす
 * (`45.500000000000004` → `45.5`、`120` → `120`)。数でない値は`0`。
 */
export function formatMechNumber(value: number | null | undefined): string {
  if (typeof value !== "number" || !Number.isFinite(value)) return "0";
  return String(Number(value.toPrecision(6)));
}

/** `(120, 45.5, 0) mm / 90°`。配置が無ければ空文字 (未同期)。 */
export function formatPlacement(placement: MechPlacement | null | undefined): string {
  if (!placement) return "";
  const { x_mm, y_mm, z_mm, rotation_deg } = placement;
  const at = [x_mm, y_mm, z_mm].map(formatMechNumber).join(", ");
  return `(${at}) mm / ${formatMechNumber(rotation_deg)}°`;
}

/**
 * プロパティパネルの「3D対応付け」行の文面: FreeCADオブジェクト名と、
 * 「Sync placements」済みならその配置。対応付けが無ければ空文字。
 */
export function mechLinkText(link: MechLink | null | undefined): string {
  if (!link) return "";
  const placement = formatPlacement(link.placement);
  return placement ? `${link.object_name} @ ${placement}` : link.object_name;
}
