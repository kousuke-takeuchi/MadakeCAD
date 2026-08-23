// ネットラベルの方向付きタグ形状 (G3 / G19)。Rust側 net_tag.rs と同じ形になることを確かめる。
import { describe, expect, it } from "vitest";

import type { NetLabel } from "../ipc";
import { TAG_FONT, TAG_TIP, netTag, tagDir } from "./netTag";

function label(name: string, tag?: string): NetLabel {
  return {
    id: "l1",
    at: { x: 100, y: 50 },
    name,
    rotation: 0,
    attrs: tag === undefined ? {} : { tag },
  };
}

describe("netTag", () => {
  // ja: タグ「右」のネットラベルは、置いた点から右へ先端が飛び出した五角形の旗として描かれる
  it("draws a tag pointing right as a five-cornered flag with its point on the right", () => {
    const tag = netTag(label("24V", "right"))!;
    expect(tag.dir).toBe("right");
    expect(tag.points).toHaveLength(5);
    const tip = tag.points[2];
    expect(tip.x).toBeGreaterThan(100);
    expect(tip.x).toBeCloseTo(tag.right, 9);
    expect(tip.y).toBeCloseTo((tag.points[0].y + tag.points[4].y) / 2, 9);
  });

  // ja: タグ「左」のネットラベルは、先端が左へ飛び出した左右反転した旗として描かれる
  it("draws a tag pointing left as the mirrored flag, with its point on the left", () => {
    const tag = netTag(label("24V", "left"))!;
    expect(tag.dir).toBe("left");
    expect(tag.points[0].x).toBeLessThan(100);
    expect(tag.points[0].x).toBeCloseTo(tag.left, 9);
  });

  // ja: 旗の本体はラベルを置いた点を覆わず先端と反対側へ伸びるので、載っている配線が隠れない
  it("grows the flag body away from its point so the wire under the label stays visible", () => {
    const right = netTag(label("24V", "right"))!;
    for (const p of right.points) expect(p.x).toBeLessThanOrEqual(100 + TAG_TIP + 1e-9);
    expect(right.textAt.x).toBeLessThan(100);
    const left = netTag(label("24V", "left"))!;
    for (const p of left.points) expect(p.x).toBeGreaterThanOrEqual(100 - TAG_TIP - 1e-9);
    expect(left.textAt.x).toBeGreaterThan(100);
  });

  // ja: ラベルを置いた点は先端の付け根にあたるので、左向きの旗と右向きの旗はちょうど鏡像になる
  it("puts the label position at the root of the point, mirroring left and right flags", () => {
    const right = netTag(label("24V", "right"))!;
    const left = netTag(label("24V", "left"))!;
    const sorted = (xs: number[]) => [...xs].sort((a, b) => a - b);
    const mirrored = sorted(right.points.map((p) => 200 - p.x));
    const xs = sorted(left.points.map((p) => p.x));
    mirrored.forEach((x, i) => expect(x).toBeCloseTo(xs[i], 9));
    expect(right.points[1].x).toBeCloseTo(100, 9);
    expect(left.points[1].x).toBeCloseTo(100, 9);
  });

  // ja: タグの文字は必ず旗の内側に収まり、旗の幅はネット名の長さに合わせて伸びる
  it("keeps the text inside the flag and widens the flag for a longer net name", () => {
    const short = netTag(label("A", "right"))!;
    const long = netTag(label("24V_CONTROL_1", "right"))!;
    expect(long.right - long.left).toBeGreaterThan(short.right - short.left);
    for (const tag of [short, long]) {
      expect(tag.textAt.x).toBeGreaterThan(tag.left);
      expect(tag.textAt.x).toBeLessThan(tag.right);
    }
  });

  // ja: 左向きと右向きのタグは同じ大きさになるので、左右のラベルが並んでも高さと幅がそろう
  it("gives both directions the same flag size", () => {
    const right = netTag(label("24V", "right"))!;
    const left = netTag(label("24V", "left"))!;
    expect(right.right - right.left).toBeCloseTo(left.right - left.left, 9);
  });

  // ja: シート間クロスリファレンスは旗の本体側の外へ逃がされるので、先端や配線の上には出ない
  it("escapes the cross-reference on the body side of the flag", () => {
    const right = netTag(label("24V", "right"))!;
    expect(right.xrefAt.x).toBeLessThan(right.left);
    expect(right.xrefAnchor).toBe("end");
    expect(right.xrefAt.y).toBeCloseTo(right.textAt.y, 9);
    const left = netTag(label("24V", "left"))!;
    expect(left.xrefAt.x).toBeGreaterThan(left.right);
    expect(left.xrefAnchor).toBe("start");
  });

  // ja: 注記文字高を小さくすると、文字・余白・先端がまとめて縮み、旗の形はそのままで小さくなる
  it("shrinks the whole flag when the annotation text height is smaller", () => {
    const big = netTag(label("24V", "right"))!;
    const small = netTag(label("24V", "right"), TAG_FONT * 0.6)!;
    expect((small.right - small.left) / (big.right - big.left)).toBeCloseTo(0.6, 9);
    const height = (t: typeof big) => t.points[4].y - t.points[0].y;
    expect(height(small) / height(big)).toBeCloseTo(0.6, 9);
  });

  // ja: タグ属性の無いネットラベルは、これまでどおり文字だけの見た目のままになる
  it("leaves a label without the tag attribute as plain text", () => {
    expect(netTag(label("24V"))).toBeNull();
    expect(tagDir(label("24V"))).toBeNull();
  });

  // ja: 知らないタグの値(打ち間違いや "up")は無視され、文字だけの表示に戻る
  it("ignores an unknown tag value instead of breaking", () => {
    for (const value of ["up", "", "LEFT", "右"]) {
      expect(netTag(label("24V", value)), value).toBeNull();
    }
  });
});
