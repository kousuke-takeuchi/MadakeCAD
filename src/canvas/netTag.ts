// ネットラベルの方向付きタグ形状 (G3)。Rust側 madake-core/src/net_tag.rs と同一の数式。
// attrs["tag"] が "left" / "right" のときだけ、ネット名を囲む五角形の旗を描く。
// 枠の大きさは文字幅 (textWidthMm) から決まるので、長いネット名でもはみ出さない。

import type { NetLabel, Point } from "../ipc";
import { textWidthMm } from "./dynamicSymbol";

/** ネットラベル本文の文字高さ (mm)。Rust側svg.rsのLABEL_FONTと一致。 */
export const TAG_FONT = 2.5;
/** ラベル本文のベースラインを基準点からどれだけ上へ置くか (mm)。svg.rsのNET_LABEL_DY。 */
export const TAG_BASELINE_DY = 1.0;
/** タグ枠と文字の間の余白 (mm)。 */
export const TAG_PAD = 1.0;
/** 文字幅の見積りに持たせる余裕 (サンセリフの大文字は等幅見積りより1割ほど広い)。 */
export const TAG_WIDTH_SLACK = 1.1;
/** 先端の三角の長さ (mm)。 */
export const TAG_TIP = 2.0;
/** attrsでタグの向きを指定するキー。 */
export const TAG_ATTR = "tag";

export type TagDir = "left" | "right";

/** 描画するタグ形状 (用紙座標 mm)。 */
export interface NetTag {
  dir: TagDir;
  /** 五角形の頂点 (閉じた多角形として描く)。 */
  points: Point[];
  /** ネット名のベースライン左端 (左揃えで描く)。 */
  textAt: Point;
  /** グリフの右端。クロスリファレンスはこの右へ逃がす。 */
  right: number;
}

/** ラベルのタグ向き。未指定・空・知らない値なら null (従来どおり文字だけ)。 */
export function tagDir(label: NetLabel): TagDir | null {
  const value = label.attrs?.[TAG_ATTR]?.trim();
  return value === "left" || value === "right" ? value : null;
}

/** ラベルのタグ形状。タグ指定の無いラベルは null。 */
export function netTag(label: NetLabel): NetTag | null {
  const dir = tagDir(label);
  if (!dir) return null;
  const bodyW = textWidthMm(label.name, TAG_FONT) * TAG_WIDTH_SLACK + TAG_PAD * 2;
  const baseline = label.at.y - TAG_BASELINE_DY;
  const top = baseline - TAG_FONT - TAG_PAD;
  const bottom = baseline + TAG_PAD;
  const cy = (top + bottom) / 2;
  const x0 = label.at.x;
  if (dir === "right") {
    return {
      dir,
      points: [
        { x: x0, y: top },
        { x: x0 + bodyW, y: top },
        { x: x0 + bodyW + TAG_TIP, y: cy },
        { x: x0 + bodyW, y: bottom },
        { x: x0, y: bottom },
      ],
      textAt: { x: x0 + TAG_PAD, y: baseline },
      right: x0 + bodyW + TAG_TIP,
    };
  }
  return {
    dir,
    points: [
      { x: x0, y: cy },
      { x: x0 + TAG_TIP, y: top },
      { x: x0 + TAG_TIP + bodyW, y: top },
      { x: x0 + TAG_TIP + bodyW, y: bottom },
      { x: x0 + TAG_TIP, y: bottom },
    ],
    textAt: { x: x0 + TAG_TIP + TAG_PAD, y: baseline },
    right: x0 + TAG_TIP + bodyW,
  };
}
