// ネットラベルの方向付きタグ形状 (G3 / G19)。Rust側 madake-core/src/net_tag.rs と同一の数式。
// attrs["tag"] が "left" / "right" のときだけ、ネット名を囲む五角形の旗を描く。
//
// 基準点は**先端の付け根**。ネットラベルは必ず配線の頂点の上に置かれるので、本体を
// 基準点から先端と反対側へ伸ばすと、本体が自分の配線や配線注記の上に乗らない。
// "right" は本体が左・先端が右、"left" はその鏡像で、外形の寸法は左右で同じ。
//
// 枠の大きさは文字幅 (textWidthMm) から決まるので、長いネット名でもはみ出さない。
// 文字高さはシートの注記文字高 (annotationText.ts) に従って伸縮し、余白・先端・
// クロスリファレンスの逃がし量も同じ倍率で一緒に縮む。

import type { NetLabel, Point, TextAnchor } from "../ipc";
import { textWidthMm } from "./dynamicSymbol";
import { XREF_GAP } from "./xref";

/** ネットラベル本文の文字高さ (mm)。Rust側svg.rsのLABEL_FONTと一致。注記文字高が既定のときの値。 */
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
  /** グリフの左端 (先端を含む)。 */
  left: number;
  /** グリフの右端 (先端を含む)。 */
  right: number;
  /** クロスリファレンスの基準点。本体側 (先端と反対側) の外へ逃がす。 */
  xrefAt: Point;
  /** クロスリファレンスの寄せ方。左へ逃がすときは右揃え。 */
  xrefAnchor: TextAnchor;
}

/** ラベルのタグ向き。未指定・空・知らない値なら null (従来どおり文字だけ)。 */
export function tagDir(label: NetLabel): TagDir | null {
  const value = label.attrs?.[TAG_ATTR]?.trim();
  return value === "left" || value === "right" ? value : null;
}

/**
 * ラベルのタグ形状。タグ指定の無いラベルは null。
 * `font` はネットラベル本文の文字高さ (mm。シートの注記文字高で決まる)。
 */
export function netTag(label: NetLabel, font: number = TAG_FONT): NetTag | null {
  const dir = tagDir(label);
  if (!dir) return null;
  const scale = font / TAG_FONT;
  const pad = TAG_PAD * scale;
  const tip = TAG_TIP * scale;
  const gap = XREF_GAP * scale;
  const bodyW = textWidthMm(label.name, font) * TAG_WIDTH_SLACK + pad * 2;
  const baseline = label.at.y - TAG_BASELINE_DY * scale;
  const top = baseline - font - pad;
  const bottom = baseline + pad;
  const cy = (top + bottom) / 2;
  const x0 = label.at.x;
  if (dir === "right") {
    // 先端が右 = 本体は基準点の左側。逃がしも左 (右揃え)
    return {
      dir,
      points: [
        { x: x0 - bodyW, y: top },
        { x: x0, y: top },
        { x: x0 + tip, y: cy },
        { x: x0, y: bottom },
        { x: x0 - bodyW, y: bottom },
      ],
      textAt: { x: x0 - bodyW + pad, y: baseline },
      left: x0 - bodyW,
      right: x0 + tip,
      xrefAt: { x: x0 - bodyW - gap, y: baseline },
      xrefAnchor: "end",
    };
  }
  // 先端が左 = 本体は基準点の右側。逃がしも右 (左揃え)
  return {
    dir,
    points: [
      { x: x0 - tip, y: cy },
      { x: x0, y: top },
      { x: x0 + bodyW, y: top },
      { x: x0 + bodyW, y: bottom },
      { x: x0, y: bottom },
    ],
    textAt: { x: x0 + pad, y: baseline },
    left: x0 - tip,
    right: x0 + bodyW,
    xrefAt: { x: x0 + bodyW + gap, y: baseline },
    xrefAnchor: "start",
  };
}
