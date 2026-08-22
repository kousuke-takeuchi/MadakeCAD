// ネットラベルの方向付きタグ形状 (G3)。Rust側 net_tag.rs と同じ形になることを確かめる。
import { describe, expect, it } from "vitest";

import type { NetLabel } from "../ipc";
import { netTag, tagDir } from "./netTag";

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
  // ja: タグ「右」のネットラベルは、右へ先端が飛び出した五角形の旗として描かれる
  it("draws a tag pointing right as a five-cornered flag with its point on the right", () => {
    const tag = netTag(label("24V", "right"))!;
    expect(tag.dir).toBe("right");
    expect(tag.points).toHaveLength(5);
    const tip = tag.points[2];
    expect(tip.x).toBeGreaterThan(100);
    expect(tip.x).toBeCloseTo(tag.right, 9);
    expect(tip.y).toBeCloseTo((tag.points[0].y + tag.points[4].y) / 2, 9);
  });

  // ja: タグ「左」のネットラベルは、先端が左にある左右反転した旗として描かれる
  it("draws a tag pointing left as the mirrored flag, with its point on the left", () => {
    const tag = netTag(label("24V", "left"))!;
    expect(tag.dir).toBe("left");
    expect(tag.points[0].x).toBeCloseTo(100, 9);
    for (const p of tag.points.slice(1)) expect(p.x).toBeGreaterThan(100);
  });

  // ja: タグの文字は必ず旗の内側に収まり、旗の幅はネット名の長さに合わせて伸びる
  it("keeps the text inside the flag and widens the flag for a longer net name", () => {
    const short = netTag(label("A", "right"))!;
    const long = netTag(label("24V_CONTROL_1", "right"))!;
    expect(long.right).toBeGreaterThan(short.right);
    for (const tag of [short, long]) {
      expect(tag.textAt.x).toBeGreaterThan(tag.points[0].x);
      expect(tag.textAt.x).toBeLessThan(tag.right);
    }
  });

  // ja: 左向きと右向きのタグは同じ大きさになるので、左右のラベルが並んでも高さと幅がそろう
  it("gives both directions the same flag size", () => {
    const right = netTag(label("24V", "right"))!;
    const left = netTag(label("24V", "left"))!;
    const width = (t: typeof right) => t.right - Math.min(...t.points.map((p) => p.x));
    expect(width(right)).toBeCloseTo(width(left), 9);
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
