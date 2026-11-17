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

import type { NetLabel, Point, TextAnchor, XrefStyle } from "../ipc";
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
/** attrsで行き先の文字を指定するキー (空なら自動)。 */
export const DEST_ATTR = "tag_dest";
/** attrsで旗の先端の置き方を指定するキー ("out" なら先端が本体の遠い端)。 */
export const FLOW_ATTR = "tag_flow";
/** 旗の中に書く行き先の文字高さの、ネット名に対する倍率。 */
export const DEST_SCALE = 0.8;

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
  /** ネット名を太字で描くか (行き先を旗の中に書く書き方のとき)。 */
  bold: boolean;
  /** 旗の中に書く行き先の左端のベースライン (書かないなら null)。 */
  destAt: Point | null;
  /** 行き先の文字高さ (mm)。 */
  destFont: number;
}

/** ラベルのタグ向き。未指定・空・知らない値なら null (従来どおり文字だけ)。 */
export function tagDir(label: NetLabel): TagDir | null {
  const value = label.attrs?.[TAG_ATTR]?.trim();
  return value === "left" || value === "right" ? value : null;
}

/** ラベルの行き先の自由な文字。空・空白だけなら null (自動の行き先を使う)。 */
export function tagDest(label: NetLabel): string | null {
  const value = label.attrs?.[DEST_ATTR]?.trim();
  return value ? value : null;
}

/** 旗の先端を本体の遠い端へ置くか (attrs.tag_flow === "out")。 */
export function tagFlowsOut(label: NetLabel): boolean {
  return label.attrs?.[FLOW_ATTR]?.trim() === "out";
}

/**
 * ラベルのタグ形状。タグ指定の無いラベルは null。
 * `font` はネットラベル本文の文字高さ (mm。シートの注記文字高で決まる)。
 * `style` が "sheet_name" ならネット名を太字にし、`dest` があれば名前のあとに文字高0.8倍で書いて旗を広げる
 * (Rust側 net_tag_with と同一の数式)。
 */
export function netTag(
  label: NetLabel,
  font: number = TAG_FONT,
  style: XrefStyle = "zone",
  dest: string | null = null,
): NetTag | null {
  const dir = tagDir(label);
  if (!dir) return null;
  const out = tagFlowsOut(label);
  const scale = font / TAG_FONT;
  const pad = TAG_PAD * scale;
  const tip = TAG_TIP * scale;
  const gap = XREF_GAP * scale;
  const inside = style === "sheet_name";
  const destText = inside ? (dest?.trim() || null) : null;
  const nameW = textWidthMm(label.name, font) * TAG_WIDTH_SLACK;
  const destFont = font * DEST_SCALE;
  const destGap = font;
  const destW = destText ? textWidthMm(destText, destFont) * TAG_WIDTH_SLACK : 0;
  const bodyW = nameW + (destText ? destGap + destW : 0) + pad * 2;
  const baseline = label.at.y - TAG_BASELINE_DY * scale;
  const top = baseline - font - pad;
  const bottom = baseline + pad;
  const cy = (top + bottom) / 2;
  const x0 = label.at.x;
  const bx0 = dir === "right" ? x0 - bodyW : x0;
  const bx1 = dir === "right" ? x0 : x0 + bodyW;
  let points: Point[];
  let left: number;
  let right: number;
  if (dir === "right" && !out) {
    points = [{ x: bx0, y: top }, { x: x0, y: top }, { x: x0 + tip, y: cy }, { x: x0, y: bottom }, { x: bx0, y: bottom }];
    left = bx0;
    right = x0 + tip;
  } else if (dir === "left" && !out) {
    points = [{ x: x0 - tip, y: cy }, { x: x0, y: top }, { x: bx1, y: top }, { x: bx1, y: bottom }, { x: x0, y: bottom }];
    left = x0 - tip;
    right = bx1;
  } else if (dir === "right") {
    points = [{ x: x0, y: top }, { x: bx0, y: top }, { x: bx0 - tip, y: cy }, { x: bx0, y: bottom }, { x: x0, y: bottom }];
    left = bx0 - tip;
    right = x0;
  } else {
    points = [{ x: x0, y: top }, { x: bx1, y: top }, { x: bx1 + tip, y: cy }, { x: bx1, y: bottom }, { x: x0, y: bottom }];
    left = x0;
    right = bx1 + tip;
  }
  const textX = dir === "right" ? bx0 + pad : x0 + pad;
  return {
    dir,
    points,
    textAt: { x: textX, y: baseline },
    left,
    right,
    // 逃がしは本体側 (右向きは左へ右揃え、左向きは右へ左揃え)
    xrefAt: dir === "right" ? { x: left - gap, y: baseline } : { x: right + gap, y: baseline },
    xrefAnchor: dir === "right" ? "end" : "start",
    bold: inside,
    destAt: destText ? { x: textX + nameW + destGap, y: baseline } : null,
    destFont,
  };
}
