// シートごとの注記文字高 (G20)。Rust側 madake-core/src/model.rs の
// Sheet::annotation_text_mm / annotation_scale と同一ルール。
//
// 配線注記・線番・ネットタグは「図面規格」としてシート単位で大きさを決める。
// 未設定のシートは既定 (2.0mm) で、これまでの図面と1文字も変わらない。
// 文字だけでなく配線からの逃がし量も同じ倍率で縮むので、注記の層ごと小さくなる。

import type { Sheet } from "../ipc";

/** 注記文字高の既定値 (mm)。設定の無いシートはこの高さで描かれる。 */
export const DEFAULT_ANNOTATION_TEXT_MM = 2.0;
/** 注記文字高の下限 (mm)。これより小さいと紙に印刷したとき読めない。 */
export const MIN_ANNOTATION_TEXT_MM = 1.0;
/** 注記文字高の上限 (mm)。これより大きいと2.5mmピッチの図面で注記同士が必ず重なる。 */
export const MAX_ANNOTATION_TEXT_MM = 3.5;

/** シートの注記文字高 (mm)。未設定・不正な値なら既定。 */
export function annotationTextMm(sheet: Sheet | null | undefined): number {
  const value = sheet?.annotation_text_mm;
  return typeof value === "number" && Number.isFinite(value) && value > 0
    ? value
    : DEFAULT_ANNOTATION_TEXT_MM;
}

/** 注記の層 (配線注記・線番・ネットタグ) にかける倍率。1.0 = 既定の大きさ。 */
export function annotationScale(sheet: Sheet | null | undefined): number {
  return annotationTextMm(sheet) / DEFAULT_ANNOTATION_TEXT_MM;
}

/** 入力された注記文字高が設定できる範囲かどうか (範囲外はコマンドが拒否する)。 */
export function isValidAnnotationTextMm(value: number): boolean {
  return (
    Number.isFinite(value) && value >= MIN_ANNOTATION_TEXT_MM && value <= MAX_ANNOTATION_TEXT_MM
  );
}
