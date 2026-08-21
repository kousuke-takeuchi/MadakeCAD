// 改訂欄 (ISO 7200 / JIS Z 8311) のレイアウト計算。Rust側 svg.rs と同一ルールであること。
import { describe, expect, it } from "vitest";

import type { Revision, Sheet } from "../ipc";
import {
  REV_COL_W,
  REV_HEADERS,
  effectiveRev,
  revisionLayout,
  visibleRevisions,
} from "./renderer";

function rev(mark: string): Revision {
  return { mark, date: `26-08-${mark}`, description: `変更${mark}`, by: "K.T" };
}

function sheet(marks: string[], titleRev = ""): Sheet {
  return {
    id: "s1",
    name: "t",
    size: "A3",
    orientation: "Landscape",
    zone_cols: 4,
    zone_rows: 6,
    title_block: { rev: titleRev },
    revisions: marks.map(rev),
    entities: {},
  } as unknown as Sheet;
}

describe("revisionLayout", () => {
  // ja: 改訂欄は表題欄の真上に同じ右端・同じ幅で置かれ、行高は表題欄と同じ8mmになる
  it("places the revision table directly above the title block with the same width and row height", () => {
    const l = revisionLayout(sheet(["A", "B"]));
    expect(l).not.toBeNull();
    // A3横 (420x297)、図枠マージン10mm、表題欄120x32mm → 表題欄の上端は y=255、左端は x=290
    expect(l!.x).toBe(290);
    expect(l!.w).toBe(120);
    // 改訂2行+列見出し1行=3行×8mm
    expect(l!.h).toBe(24);
    expect(l!.y + l!.h).toBe(255);
  });

  // ja: 改訂行は古い行が下・新しい行が上に積まれ、列見出しは最下段(表題欄側)に置かれる
  it("stacks revisions oldest at the bottom and newest on top, with the column header row at the very bottom", () => {
    const l = revisionLayout(sheet(["A", "B", "C"]))!;
    expect(l.rows.map((r) => r.rev.mark)).toEqual(["A", "B", "C"]);
    const [a, b, c] = l.rows;
    expect(c.top).toBeLessThan(b.top);
    expect(b.top).toBeLessThan(a.top);
    expect(a.top).toBeLessThan(l.headerTop);
    expect(l.headerTop + 8).toBe(255);
  });

  // ja: 改訂が無いシートには改訂欄を作らない(空の枠も描かない)
  it("lays out nothing for a sheet without revisions", () => {
    expect(revisionLayout(sheet([]))).toBeNull();
  });

  // ja: 改訂が7件あると新しい6件だけが描かれ、最も古い行は省略される(データは残る)
  it("keeps only the newest six revisions in the drawing and drops older rows", () => {
    const s = sheet(["A", "B", "C", "D", "E", "F", "G"]);
    const l = revisionLayout(s)!;
    expect(s.revisions).toHaveLength(7);
    expect(l.rows.map((r) => r.rev.mark)).toEqual(["B", "C", "D", "E", "F", "G"]);
    expect(l.h).toBe(56);
  });

  // ja: 列は記号・日付・内容・承認の4つで、合計幅は表題欄の幅と一致する
  it("splits the title block width into the mark, date, description and approver columns", () => {
    const l = revisionLayout(sheet(["A"]))!;
    expect(REV_HEADERS).toEqual(["記号", "日付", "内容", "承認"]);
    expect(REV_COL_W.reduce((a, b) => a + b, 0)).toBe(120);
    expect(l.colLefts).toEqual([290, 304, 332, 388]);
  });
});

describe("visibleRevisions / effectiveRev", () => {
  // ja: 表示対象の改訂は新しい方から6件までで、古い順のまま返る
  it("returns at most the newest six revisions, still in oldest-first order", () => {
    const all = ["A", "B", "C", "D", "E", "F", "G", "H"].map(rev);
    expect(visibleRevisions(all).map((r) => r.mark)).toEqual(["C", "D", "E", "F", "G", "H"]);
    expect(visibleRevisions(all.slice(0, 2)).map((r) => r.mark)).toEqual(["A", "B"]);
  });

  // ja: 表題欄のRev欄は最新改訂の記号を出し、改訂が無ければ表題欄の値、それも空ならハイフンを出す
  it("shows the newest revision mark in the title block Rev cell, falling back to the stored value or a dash", () => {
    expect(effectiveRev(sheet(["A", "B"], "A"))).toBe("B");
    expect(effectiveRev(sheet([], "A"))).toBe("A");
    expect(effectiveRev(sheet([]))).toBe("-");
  });
});
