// 図枠エディタ (M4残り⑥) の純粋ロジックの仕様テスト。
import { describe, expect, it } from "vitest";
import {
  addLine,
  addRect,
  addTextCell,
  addTitleCell,
  bindsIn,
  constrainHV,
  copyId,
  deleteItem,
  filterPlaceholders,
  frameChecks,
  hitItem,
  isValidFrameId,
  moveItem,
  setLogo,
  tbOrigin,
  toTb,
  updateCell,
} from "./frameEditor";
import { DEFAULT_FRAME_TEMPLATE } from "./frameTemplate";

const base = () => JSON.parse(JSON.stringify(DEFAULT_FRAME_TEMPLATE)) as typeof DEFAULT_FRAME_TEMPLATE;

describe("frame editor geometry", () => {
  // ja: 図枠エディタは用紙座標で見せ、表題欄の左上角からの相対座標で保存する (A3の既定図枠なら表題欄は(290,255))
  it("shows paper coordinates and stores them relative to the title block corner", () => {
    expect(tbOrigin(base())).toEqual({ x: 290, y: 255 });
    expect(toTb(base(), { x: 300, y: 260 })).toEqual({ x: 10, y: 5 });
  });

  // ja: 線ツールは表題欄の罫線を1本足し、元のテンプレートは変えない
  it("adds one title-block grid line without touching the original template", () => {
    const t = base();
    const next = addLine(t, { x: 290, y: 255 }, { x: 410, y: 255 });
    expect(next.title_block.grid_lines.slice(-1)[0]).toEqual({ x1: 0, y1: 0, x2: 120, y2: 0 });
    expect(t.title_block.grid_lines).toHaveLength(DEFAULT_FRAME_TEMPLATE.title_block.grid_lines.length);
  });

  // ja: 長さ0の線や幅0の矩形は何も足さない
  it("adds nothing for a zero-length line or a flat rectangle", () => {
    const t = base();
    expect(addLine(t, { x: 1, y: 1 }, { x: 1, y: 1 })).toBe(t);
    expect(addRect(t, { x: 1, y: 1 }, { x: 9, y: 1 })).toBe(t);
  });

  // ja: 矩形ツールは対角の2点から4本の罫線を足す (どちら向きに引いても同じ)
  it("turns a rectangle into four grid lines whichever way it is dragged", () => {
    const a = addRect(base(), { x: 300, y: 260 }, { x: 320, y: 270 });
    const b = addRect(base(), { x: 320, y: 270 }, { x: 300, y: 260 });
    expect(a.title_block.grid_lines.slice(-4)).toEqual(b.title_block.grid_lines.slice(-4));
    expect(a.title_block.grid_lines.length - base().title_block.grid_lines.length).toBe(4);
  });

  // ja: Shiftの水平/垂直拘束は、変化の大きいほうの軸だけを残す
  it("keeps only the dominant axis when Shift constrains a line", () => {
    expect(constrainHV({ x: 0, y: 0 }, { x: 10, y: 3 })).toEqual({ x: 10, y: 0 });
    expect(constrainHV({ x: 0, y: 0 }, { x: 2, y: -8 })).toEqual({ x: 0, y: -8 });
  });

  // ja: 文字ツールは固定文字のセル、プレースホルダツールはバインドのセルを足す
  it("adds a fixed-text cell for the text tool and a bound cell for the placeholder tool", () => {
    const { template: t1, index: i1 } = addTextCell(base(), { x: 300, y: 260 }, { text: "社外秘" });
    expect(t1.title_block.cells[i1]).toMatchObject({ x: 10, y: 5, text: "社外秘", size: 3.5 });
    const { template: t2, index: i2 } = addTextCell(base(), { x: 300, y: 260 }, { bind: "{project.name}" });
    expect(t2.title_block.cells[i2].bind).toBe("{project.name}");
  });

  // ja: 表題欄セルツールは大きさ付きのバインドセルと外周の罫線4本を足す
  it("adds a sized bound cell with its four border lines for the title-cell tool", () => {
    const { template, index } = addTitleCell(base(), { x: 300, y: 260 }, { x: 348, y: 268 });
    expect(template.title_block.cells[index]).toMatchObject({ x: 12, w: 48, h: 8, bind: "{project.name}" });
    expect(template.title_block.grid_lines.length - base().title_block.grid_lines.length).toBe(4);
  });

  // ja: ロゴ枠ツールは矩形をロゴ枠にし、もう一度引けば置き換わる
  it("makes the rectangle the logo frame and replaces it when drawn again", () => {
    const once = setLogo(base(), { x: 20, y: 20 }, { x: 60, y: 40 });
    expect(once.logo).toMatchObject({ x: 20, y: 20, w: 40, h: 20 });
    expect(setLogo(once, { x: 30, y: 30 }, { x: 40, y: 35 }).logo).toMatchObject({ x: 30, w: 10, h: 5 });
  });
});

describe("frame editor selection and editing", () => {
  // ja: クリックは罫線の上なら罫線、セルの中ならセルを拾い、何も無い所ではnull
  it("picks the grid line when on it, the cell when inside it and nothing on empty paper", () => {
    const t = base();
    const first = t.title_block.cells[0];
    expect(hitItem(t, { x: 290 + first.x + 1, y: 255 + first.y - 1 }, 0.5)).toEqual({ kind: "cell", index: 0 });
    expect(hitItem(t, { x: 350, y: 263 }, 0.5)).toEqual({ kind: "line", index: 0 });
    expect(hitItem(t, { x: 100, y: 100 }, 0.5)).toBeNull();
  });

  // ja: 移動は要素を(dx,dy)だけずらし、消去は要素を取り除く
  it("moves an item by the drag and deletes it", () => {
    const moved = moveItem(base(), { kind: "line", index: 0 }, 2, -1);
    expect(moved.title_block.grid_lines[0]).toEqual({ x1: 2, y1: 7, x2: 122, y2: 7 });
    const removed = deleteItem(base(), { kind: "cell", index: 0 });
    expect(removed.title_block.cells).toHaveLength(base().title_block.cells.length - 1);
  });

  // ja: セルに固定文字を入れるとバインドは消え、バインドを入れると固定文字は消える
  it("keeps either the fixed text or the bind on a cell, never both", () => {
    const toBind = updateCell(base(), 0, { bind: "{sheet.no}" });
    expect(toBind.title_block.cells[0]).toMatchObject({ bind: "{sheet.no}", text: null });
    const toText = updateCell(toBind, 0, { text: "図番" });
    expect(toText.title_block.cells[0]).toMatchObject({ text: "図番", bind: null });
  });
});

describe("frame editor placeholders and pre-save checks", () => {
  // ja: プレースホルダは名前の部分一致で絞れ、空の検索語なら全部出る
  it("filters placeholders by part of the name and lists all for an empty query", () => {
    expect(filterPlaceholders("NAME").map((p) => p.name)).toEqual(["project.name", "sheet.name"]);
    expect(filterPlaceholders("").length).toBeGreaterThan(10);
  });

  // ja: バインド文字列から{名前}を出てくる順に取り出す
  it("reads the {names} out of a bind string in order", () => {
    expect(bindsIn("{sheet.no}/{sheet.total} rev {rev.latest}")).toEqual(["sheet.no", "sheet.total", "rev.latest"]);
  });

  // ja: 同梱の図枠は保存前チェックがすべて0件
  it("finds nothing to report on the bundled frame", () => {
    expect(frameChecks(base())).toEqual({ errors: [], unknownBinds: [], outside: 0 });
  });

  // ja: 知らないプレースホルダと枠外の要素は数えるが、保存は止めない
  it("counts unknown placeholders and items outside the frame without blocking the save", () => {
    let t = addTextCell(base(), { x: 300, y: 260 }, { bind: "{customer.name}" }).template;
    t = addLine(t, { x: 2, y: 2 }, { x: 8, y: 2 });
    const checks = frameChecks(t);
    expect(checks.unknownBinds).toEqual(["customer.name"]);
    expect(checks.outside).toBe(1);
    expect(checks.errors).toEqual([]);
  });

  // ja: 改訂欄の列幅の合計が表題欄の幅と違う・文字高が0のセルは保存できない誤りになる
  it("reports revision widths that do not add up and a zero text height as blocking errors", () => {
    const t = base();
    t.revision_block.col_w = [10, 10, 10, 10];
    t.title_block.cells[0].size = 0;
    expect(frameChecks(t).errors).toEqual(["cells[0].size", "revision_block.col_w"]);
  });

  // ja: 別名で保存の既定idは元のid+-copyで、使われていれば番号を足す。idは英数字・-・_だけ
  it("suggests an unused -copy id and accepts only file-name-safe ids", () => {
    expect(copyId("jis-a3", ["jis-a3"])).toBe("jis-a3-copy");
    expect(copyId("jis-a3", ["jis-a3-copy"])).toBe("jis-a3-copy2");
    expect(isValidFrameId("custom_2026-a")).toBe(true);
    expect(isValidFrameId("../x")).toBe(false);
  });
});

// 図枠テンプレートの拡張項目 (辺別余白・マーク・罫線の太さ・回転文字・改訂欄の列など)。
// エディタはこれらを編集する画面をまだ持たないが、読み込んだテンプレートから失ってはならない。
describe("frame editor and the extended template items", () => {
  const extended = () => {
    const t = base();
    t.frame_margins = { left: 30, right: 15, top: 12, bottom: 10 };
    t.zones.show = false;
    t.marks = { trim: true, centre: true };
    t.text_size_is = "cap";
    t.title_block.grid_lines[0].weight = "medium";
    t.title_block.cells[0] = { ...t.title_block.cells[0], rotation: 90, boxed: true, fit: true };
    t.revision_block.columns = [
      { field: "mark", header: "SYMB", w: 14 },
      { field: "description", header: "CHANGELOG", w: t.title_block.w - 14 },
    ];
    t.revision_block.row_h = 7;
    t.revision_block.rows_fixed = 7;
    t.revision_block.mark_shape = "triangle";
    t.logo = { x: 40, y: 40, w: 40, h: 10, path: "logo.png", data_url: "data:image/png;base64,AAAA" };
    return t;
  };

  // ja: 新しい項目を持つテンプレートに無関係な編集(線の追加・セルの移動と変更・線の削除)をしても、新しい項目は失われない
  it("keeps the new items through unrelated edits", () => {
    const before = extended();
    let t = addLine(before, { x: 300, y: 260 }, { x: 340, y: 260 });
    t = moveItem(t, { kind: "cell", index: 0 }, 5, 0);
    t = updateCell(t, 0, { size: 4 });
    t = deleteItem(t, { kind: "line", index: t.title_block.grid_lines.length - 1 });
    expect(t.frame_margins).toEqual(before.frame_margins);
    expect(t.zones.show).toBe(false);
    expect(t.marks).toEqual(before.marks);
    expect(t.text_size_is).toBe("cap");
    expect(t.title_block.grid_lines[0].weight).toBe("medium");
    expect(t.title_block.cells[0]).toMatchObject({ rotation: 90, boxed: true, fit: true, size: 4 });
    expect(t.revision_block).toMatchObject({ row_h: 7, rows_fixed: 7, mark_shape: "triangle" });
    expect(t.revision_block.columns).toEqual(before.revision_block.columns);
    expect(t.logo).toEqual(before.logo);
  });

  // ja: 列を自分で持つ改訂欄は、列の数・項目の重複・幅・幅の合計で検査される
  it("checks a table with its own columns by count, duplicates and the sum of widths", () => {
    expect(frameChecks(extended()).errors).toEqual([]);
    const short = extended();
    short.revision_block.columns![1].w -= 5;
    expect(frameChecks(short).errors).toEqual(["revision_block.columns"]);
    const twice = extended();
    twice.revision_block.columns![1].field = "mark";
    expect(frameChecks(twice).errors).toEqual(["revision_block.columns"]);
    const none = extended();
    none.revision_block.columns = [];
    expect(frameChecks(none).errors).toEqual(["revision_block.columns"]);
    const zero = extended();
    zero.revision_block.columns![0].w = 0;
    expect(frameChecks(zero).errors).toEqual(["revision_block.columns"]);
  });

  // ja: 辺ごとの余白は4辺とも0以上でなければならず、外枠の外にある要素は辺ごとの余白で数える
  it("checks margins per side and counts outside items against them", () => {
    const bad = extended();
    bad.frame_margins = { left: -1, right: 20, top: 19, bottom: 18 };
    expect(frameChecks(bad).errors).toEqual(["frame_margins"]);
    const t = base();
    t.title_block.cells = [];
    t.title_block.grid_lines = [];
    // A3 (420×297)。左余白34だと x=20 は外、10だと中
    t.logo = { x: 20, y: 100, w: 30, h: 10 };
    expect(frameChecks(t).outside).toBe(0);
    t.frame_margins = { left: 30, right: 10, top: 10, bottom: 10 };
    expect(frameChecks(t).outside).toBe(1);
  });

  // ja: 行高・固定行数・回転の不正な値は項目名つきで報告される
  it("reports a bad row height, fixed row count and rotation by name", () => {
    const t = extended();
    t.revision_block.row_h = 0;
    t.revision_block.rows_fixed = 0;
    t.title_block.cells[0].rotation = 45;
    expect(frameChecks(t).errors).toEqual(["cells[0].rotation", "revision_block.row_h", "revision_block.rows_fixed"]);
  });

  // ja: 記号の三角・改訂欄の文字サイズ・同じセルに続ける項目・裁断マークの不正な値は項目名つきで報告され、正しい値は通る
  it("reports bad mark, revision text and trim values by name", () => {
    const good = extended();
    good.marks = { trim: true, centre: true, trim_insets: { left: 4, right: 6, top: 5, bottom: 3 }, trim_arm: 12, centre_to_trim: true, centre_arrow: true, centre_weight: "thin" };
    good.revision_block.text_size = 3;
    good.revision_block.header_size = 2;
    good.revision_block.mark_style = { h: 1, aspect: 0.75, gap: 0.5, pad: 0, in_empty_rows: true };
    good.revision_block.columns![0].suffix = "count";
    expect(frameChecks(good).errors).toEqual([]);
    const bad = extended();
    bad.marks = { trim_insets: { left: -1, right: 0, top: 0, bottom: 0 }, trim_arm: 0 };
    bad.revision_block.text_size = 0;
    bad.revision_block.header_size = -1;
    bad.revision_block.mark_style = { h: 1.5 };
    bad.revision_block.columns![0].suffix = "mark";
    expect(frameChecks(bad).errors).toEqual([
      "revision_block.columns.suffix",
      "revision_block.text_size",
      "revision_block.header_size",
      "revision_block.mark_style",
      "marks.trim_insets",
      "marks.trim_arm",
    ]);
  });

  // ja: 表題欄の自由な欄 {field.名前} は知らないプレースホルダとして報告されない
  it("does not report a free field bind as unknown", () => {
    const t = base();
    t.title_block.cells[0] = { x: 2, y: 5, bind: "{field.title_en} {nonsense}", size: 3 };
    expect(frameChecks(t).unknownBinds).toEqual(["nonsense"]);
  });
});
