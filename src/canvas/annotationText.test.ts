// シートごとの注記文字高 (G20)。Rust側 Sheet::annotation_text_mm と同じ既定・同じ倍率であること。
import { describe, expect, it } from "vitest";

import type { Sheet } from "../ipc";
import {
  DEFAULT_ANNOTATION_TEXT_MM,
  annotationScale,
  annotationTextMm,
  isValidAnnotationTextMm,
} from "./annotationText";

const sheet = (height?: number | null): Sheet =>
  ({ id: "s1", annotation_text_mm: height, entities: {} }) as unknown as Sheet;

describe("annotationText", () => {
  // ja: 注記文字高を設定していないシートは既定の2.0mmで、注記の大きさはこれまでと変わらない
  it("uses the default 2.0mm on a sheet with no setting", () => {
    expect(annotationTextMm(sheet())).toBe(DEFAULT_ANNOTATION_TEXT_MM);
    expect(annotationTextMm(sheet(null))).toBe(2.0);
    expect(annotationScale(sheet())).toBe(1);
  });

  // ja: 注記文字高1.4mmのシートでは、注記の層がまとめて0.7倍の大きさになる
  it("scales the whole annotation layer to 0.7 at a 1.4mm text height", () => {
    expect(annotationTextMm(sheet(1.4))).toBe(1.4);
    expect(annotationScale(sheet(1.4))).toBeCloseTo(0.7, 9);
  });

  // ja: 壊れた値 (0・負・数値でない) が入っていても既定に落ちるので、図面が消えることはない
  it("falls back to the default for a broken value instead of losing the annotation", () => {
    for (const bad of [0, -1, Number.NaN]) {
      expect(annotationTextMm(sheet(bad)), String(bad)).toBe(DEFAULT_ANNOTATION_TEXT_MM);
    }
    expect(annotationTextMm(null)).toBe(DEFAULT_ANNOTATION_TEXT_MM);
  });

  // ja: 設定できる注記文字高は1.0〜3.5mmで、その外はエンジンが受け付けない
  it("accepts only heights between 1.0 and 3.5mm", () => {
    for (const ok of [1.0, 1.4, 2.0, 3.5]) expect(isValidAnnotationTextMm(ok), String(ok)).toBe(true);
    for (const bad of [0.9, 3.6, Number.NaN]) {
      expect(isValidAnnotationTextMm(bad), String(bad)).toBe(false);
    }
  });
});
