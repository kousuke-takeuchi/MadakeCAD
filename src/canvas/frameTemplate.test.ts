// 図枠テンプレート (プロットフレーム、M4 §9) のレイアウト計算とバインド解決。
// Rust側 frame_template.rs / svg.rs と同一の結果になること。
import { describe, expect, it } from "vitest";

import jisA3 from "../../src-tauri/resources/frame_templates/jis-a3.json";
import jisA4Portrait from "../../src-tauri/resources/frame_templates/jis-a4-portrait.json";
import type { Project, Revision, Sheet, StrokeFontData } from "../ipc";
import { setActiveStrokeFont } from "./strokeFont";
import {
  DEFAULT_FRAME_TEMPLATE,
  bindContextForProject,
  bindNames,
  bindOverlays,
  bindContextForSheet,
  bindValue,
  cellText,
  fittedFont,
  fontSize,
  frameLayout,
  frameMargins,
  frameRect,
  frameTemplateName,
  frameTemplateSummary,
  resolveBinds,
  revisionColumns,
  revisionLayout,
  titleBlockOrigin,
  usesExtendedRevisionTable,
  zoneLabel,
  type FrameTemplate,
} from "./frameTemplate";

function rev(mark: string): Revision {
  return { mark, date: `26-08-${mark}`, description: `変更${mark}`, by: "K.T" };
}

function sheet(marks: string[] = [], overrides: Partial<Sheet> = {}): Sheet {
  return {
    id: "s1",
    name: "動力回路",
    size: "A3",
    orientation: "Landscape",
    zone_cols: 4,
    zone_rows: 6,
    title_block: {
      company: "まだけ電機",
      title: "制御盤 主回路",
      drawing_no: "MDK-001",
      scale: "1:1",
      date: "2026-08-23",
      designed: "K.T",
      drawn: "S.M",
      checked: "Y.N",
      approved: "H.I",
      rev: "A",
    },
    revisions: marks.map(rev),
    entities: {},
    ...overrides,
  } as unknown as Sheet;
}

/** 同梱のA4縦テンプレート (実ファイルをそのまま読む)。 */
function a4Portrait(): FrameTemplate {
  return jisA4Portrait as unknown as FrameTemplate;
}

describe("図枠テンプレート (jis-a3の既定値)", () => {
  // ja: 既定の図枠テンプレートは現行のJIS図枠そのもので、マージン10mm・ゾーン4×6・右下の120×32mm表題欄を持つ
  it("mirrors the JIS frame the drawings have always used: 10 mm margin, 4x6 zones and a 120x32 mm bottom-right title block", () => {
    const t = DEFAULT_FRAME_TEMPLATE;
    expect(t.id).toBe("jis-a3");
    expect(t.frame_margin).toBe(10);
    expect(t.zones).toMatchObject({ cols: 4, rows: 6, col_labels: "numeric", row_labels: "alpha" });
    expect(t.title_block.origin).toBe("bottom_right");
    expect([t.title_block.w, t.title_block.h, t.title_block.row_h]).toEqual([120, 32, 8]);
    expect(t.revision_block.col_w.reduce((a, b) => a + b, 0)).toBe(t.title_block.w);
    expect(titleBlockOrigin(t, 420, 297)).toEqual({ x: 290, y: 255 });
  });

  // ja: キャンバスが持つ既定の図枠は、同梱のjis-a3.jsonと一字一句同じ内容である(印刷とキャンバスで図枠がずれない)
  it("is a byte-for-byte copy of the bundled jis-a3.json, so the canvas and the print output never drift apart", () => {
    expect(DEFAULT_FRAME_TEMPLATE).toEqual(jisA3);
  });

  // ja: ゾーンの目盛文字は数字なら1,2,3…、英字ならA,B,C…でZの次はAへ戻る
  it("numbers zone columns 1,2,3 and letters zone rows A,B,C, wrapping back to A after Z", () => {
    expect(zoneLabel("numeric", 0)).toBe("1");
    expect(zoneLabel("numeric", 9)).toBe("10");
    expect(zoneLabel("alpha", 0)).toBe("A");
    expect(zoneLabel("alpha", 25)).toBe("Z");
    expect(zoneLabel("alpha", 26)).toBe("A");
  });
});

describe("表題欄のバインド解決", () => {
  // ja: 表題欄の自由な欄は {field.名前} で解決され、無い名前は空文字になる
  it("resolves a free title block field through {field.name}, and a missing name to empty text", () => {
    const s = sheet([], { title_block: { fields: { title_en: "MAIN CIRCUIT" } } });
    const ctx = bindContextForSheet(s);
    expect(resolveBinds("{field.title_en}/{field.missing}", ctx)).toBe("MAIN CIRCUIT/");
  });

  // ja: 説明書きのバインドはすべて値になる (プロジェクト名・シート番号/総数・シート名・最新改訂・表題欄の全欄)
  it("resolves every documented bind to its value", () => {
    const s = sheet();
    const project = { name: "まだけ制御盤", sheets: [s, sheet([], { id: "s2" })] } as unknown as Project;
    const ctx = bindContextForProject(project, s);
    expect(resolveBinds("{project.name}", ctx)).toBe("まだけ制御盤");
    expect(resolveBinds("{sheet.no}/{sheet.total}", ctx)).toBe("1/2");
    expect(resolveBinds("{sheet.name}", ctx)).toBe("動力回路");
    expect(resolveBinds("{rev.latest}", ctx)).toBe("A");
    expect(resolveBinds("{title_block.drawing_no}", ctx)).toBe("MDK-001");
    expect(resolveBinds("{title_block.company}", ctx)).toBe("まだけ電機");
    expect(resolveBinds("{title_block.drawn}", ctx)).toBe("S.M");
    expect(resolveBinds("{title_block.checked}", ctx)).toBe("Y.N");
    expect(resolveBinds("{title_block.rev}", ctx)).toBe("A");
  });

  // ja: {date} {scale} {designer} {approver} は表題欄の日付・尺度・設計・承認の別名として同じ値になる
  it("treats {date}, {scale}, {designer} and {approver} as aliases of the title-block fields", () => {
    const ctx = bindContextForSheet(sheet());
    expect(resolveBinds("{date}", ctx)).toBe(resolveBinds("{title_block.date}", ctx));
    expect(resolveBinds("{scale}", ctx)).toBe("1:1");
    expect(resolveBinds("{designer}", ctx)).toBe("K.T");
    expect(resolveBinds("{approver}", ctx)).toBe("H.I");
  });

  // ja: 知らないバインドはエラーにならず空文字になる (テンプレートに書き間違いがあっても図面は出せる)
  it("turns an unknown bind into an empty string instead of failing", () => {
    const ctx = bindContextForSheet(sheet());
    expect(bindValue("no.such.field", ctx)).toBeUndefined();
    expect(resolveBinds("[{no.such.field}]", ctx)).toBe("[]");
    expect(resolveBinds("図番 {title_block.drawing_no} / {nope}", ctx)).toBe("図番 MDK-001 / ");
  });

  // ja: バインドの無い文字はそのまま出て、閉じ括弧の無い「{」は行の残りを飲み込まずそのまま残る
  it("leaves plain text and an unclosed brace exactly as they are", () => {
    const ctx = bindContextForSheet(sheet());
    expect(resolveBinds("図番", ctx)).toBe("図番");
    expect(resolveBinds("a {unclosed", ctx)).toBe("a {unclosed");
  });

  // ja: {rev.latest}は保存されたRev欄ではなく最新の改訂行に追従する
  it("follows the newest revision row for {rev.latest} rather than the stored Rev field", () => {
    const ctx = bindContextForSheet(sheet(["A", "C"]));
    expect(resolveBinds("{rev.latest}", ctx)).toBe("C");
  });

  // ja: シート単体では「1枚中1枚目・プロジェクト名は空」として解決される
  it("binds a lone sheet as sheet 1 of 1 with an empty project name", () => {
    const ctx = bindContextForSheet(sheet());
    expect(resolveBinds("{sheet.no}/{sheet.total}", ctx)).toBe("1/1");
    expect(resolveBinds("{project.name}", ctx)).toBe("");
  });

  // ja: 固定文字のセルは中括弧も含めてそのまま出し、バインドのセルだけが値へ置き換わる
  it("prints a fixed-text cell verbatim and resolves only bind cells", () => {
    const ctx = bindContextForSheet(sheet());
    expect(cellText({ x: 0, y: 0, text: "{title_block.title}", size: 3 }, ctx)).toBe(
      "{title_block.title}",
    );
    expect(cellText({ x: 0, y: 0, bind: "{title_block.title}", size: 3 }, ctx)).toBe("制御盤 主回路");
  });
});

describe("frameLayout", () => {
  // ja: 既定の図枠をA3横へ当てると、外枠は10mmマージンの400×277、表題欄は右下(290,255)の120×32mmになる
  it("lays the default frame on an A3 sheet: a 400x277 frame inset by 10 mm and the title block at (290, 255)", () => {
    const l = frameLayout(sheet());
    expect(l.paper).toEqual({ w: 420, h: 297 });
    expect(l.frame).toEqual({ x0: 10, y0: 10, x1: 410, y1: 287 });
    expect(l.titleBlock).toMatchObject({ x: 290, y: 255, w: 120, h: 32, rowH: 8 });
  });

  // ja: ゾーンの分割数はシートの設定に従い、目盛文字の種類はテンプレートが決める(図枠を替えても図面の住所は変わらない)
  it("takes the zone counts from the sheet and only the label style from the template", () => {
    const wide = sheet([], { zone_cols: 8, zone_rows: 4 } as Partial<Sheet>);
    const l = frameLayout(wide);
    expect(l.zones.cols).toBe(8);
    expect(l.zones.rows).toBe(4);
    expect(l.zones.colLabels).toEqual(["1", "2", "3", "4", "5", "6", "7", "8"]);
    expect(l.zones.rowLabels).toEqual(["A", "B", "C", "D"]);
    expect(l.zones.zw).toBe(400 / 8);
    expect(l.zones.zh).toBe(277 / 4);
  });

  // ja: 表題欄の罫線とセルは用紙座標へ移され、セルには解決済みの文字が入る
  it("moves the title-block rules and cells into paper coordinates with their text resolved", () => {
    const l = frameLayout(sheet());
    expect(l.titleBlock.gridLines[0]).toEqual({ x1: 290, y1: 263, x2: 410, y2: 263, stroke: 0.25 });
    expect(l.titleBlock.gridLines[3]).toEqual({ x1: 314, y1: 255, x2: 314, y2: 287, stroke: 0.25 });
    const cells = l.titleBlock.cells;
    expect(cells[0]).toMatchObject({ x: 292, y: 260.5, text: "図番", align: "start", size: 3 });
    expect(cells[1]).toMatchObject({ x: 316, y: 260.5, text: "MDK-001  Rev A" });
    expect(cells.map((c) => c.text)).toContain("K.T  製図 S.M  検図 Y.N  承認 H.I");
  });

  // ja: 会社名のセルは表題欄の左外に右揃えで置かれ、会社名が空のシートでは描かれない
  it("puts the company name outside the left edge of the title block and drops it when empty", () => {
    const l = frameLayout(sheet());
    const company = l.titleBlock.cells.find((c) => c.text === "まだけ電機");
    expect(company).toMatchObject({ x: 288, y: 285, align: "end" });

    const blank = sheet();
    blank.title_block.company = "";
    const cells = frameLayout(blank).titleBlock.cells;
    expect(cells.some((c) => c.align === "end")).toBe(false);
    expect(cells.some((c) => c.text === "図番")).toBe(true);
  });

  // ja: A4縦の図枠へ差し替えると表題欄の幅が110mmへ縮み、改訂欄の列も同じ幅に収まる
  it("narrows the title block and the revision columns to 110 mm with the A4 portrait frame", () => {
    const s = sheet(["A"], { size: "A4", orientation: "Portrait" } as Partial<Sheet>);
    const l = frameLayout(s, a4Portrait());
    expect(l.paper).toEqual({ w: 210, h: 297 });
    expect(l.frame).toEqual({ x0: 10, y0: 10, x1: 200, y1: 287 });
    expect(l.titleBlock).toMatchObject({ x: 90, y: 255, w: 110 });
    expect(l.revision!.colLefts).toEqual([90, 103, 129, 180]);
    expect(l.revision!.w).toBe(110);
  });

  // ja: ロゴ枠を持つ図枠テンプレートでは、ロゴの位置がそのまま配置に出る(画像が無くても場所が決まる)
  it("reports the logo frame rectangle so the space is reserved even without an image", () => {
    const t = structuredClone(DEFAULT_FRAME_TEMPLATE);
    t.logo = { x: 15, y: 15, w: 40, h: 20 };
    expect(frameLayout(sheet(), t).logo).toEqual({ x: 15, y: 15, w: 40, h: 20, dataUrl: null });
    expect(frameLayout(sheet()).logo).toBeNull();
  });
});

describe("revisionLayout (図枠テンプレート版)", () => {
  // ja: 改訂欄は表題欄の真上に同じ右端・同じ幅で置かれ、行高は表題欄と同じ8mmになる
  it("places the revision table directly above the title block with the same width and row height", () => {
    const l = revisionLayout(sheet(["A", "B"]))!;
    expect(l.x).toBe(290);
    expect(l.w).toBe(120);
    expect(l.rowH).toBe(8);
    expect(l.h).toBe(24);
    expect(l.y + l.h).toBe(255);
  });

  // ja: 改訂欄の最大行数はテンプレートが決め、上限を超えた分は古い行から省かれる
  it("keeps only as many revision rows as the template allows, dropping the oldest", () => {
    const t = structuredClone(DEFAULT_FRAME_TEMPLATE);
    t.revision_block.rows_max = 3;
    const l = revisionLayout(sheet(["A", "B", "C", "D", "E"]), t)!;
    expect(l.rows.map((r) => r.rev.mark)).toEqual(["C", "D", "E"]);
    expect(l.h).toBe(32);
  });
});

describe("bindOverlays (図枠プレビューの青いプレースホルダ)", () => {
  // ja: 表題欄の中身が空のプロジェクトでは、どのバインドがそこへ入るかを`{...}`のまま重ねて見せる
  it("shows the bind placeholders themselves for the title block cells that would come out blank", () => {
    const blank = sheet([], { title_block: {} } as Partial<Sheet>);
    const overlays = bindOverlays(blank, DEFAULT_FRAME_TEMPLATE, bindContextForSheet(blank));
    expect(overlays.map((o) => o.text)).toContain("{title_block.title}");
    // 位置・大きさ・寄せは、そのセルの描画位置とそろえる (右下の表題欄の中)
    const title = overlays.find((o) => o.text === "{title_block.title}")!;
    expect(title).toMatchObject({ x: 316, y: 268.5, size: 3, align: "start" });
  });

  // ja: 実際の値が入っている欄にはプレースホルダを重ねない(図面の値が読めなくならない)
  it("never covers a cell that already resolves to a real value", () => {
    const overlays = bindOverlays(sheet(), DEFAULT_FRAME_TEMPLATE);
    expect(overlays.map((o) => o.text)).not.toContain("{title_block.title}");
  });

  // ja: 固定文字のセル(「品名」などの見出し)はバインドではないのでプレースホルダを出さない
  it("leaves the fixed heading cells alone because they hold no binds", () => {
    const blank = sheet([], { title_block: {} } as Partial<Sheet>);
    const overlays = bindOverlays(blank, DEFAULT_FRAME_TEMPLATE, bindContextForSheet(blank));
    expect(overlays.map((o) => o.text)).not.toContain("品名");
  });

  // ja: 空欄なら描画されない会社名のセルも、プレビューではどのバインドが入るかを見せる
  it("still shows the placeholder for a cell that the drawing omits when empty (the company name)", () => {
    const blank = sheet([], { title_block: {} } as Partial<Sheet>);
    const overlays = bindOverlays(blank, DEFAULT_FRAME_TEMPLATE, bindContextForSheet(blank));
    const company = overlays.find((o) => o.text === "{title_block.company}")!;
    expect(company).toMatchObject({ align: "end", size: 3 });
  });
});

describe("frameTemplateName (UI言語ごとの図枠名)", () => {
  // ja: 図枠テンプレートの名前は日本語UIでは日本語名、それ以外では英語名で出す
  it("shows the Japanese name in the Japanese UI and the English one everywhere else", () => {
    expect(frameTemplateName(DEFAULT_FRAME_TEMPLATE, "ja")).toBe("JIS A3 横");
    expect(frameTemplateName(DEFAULT_FRAME_TEMPLATE, "en")).toBe("JIS A3 landscape");
  });

  // ja: 日本語名を持たないユーザー図枠は、日本語UIでも英語名をそのまま出す
  it("falls back to the English name for a user frame that has no Japanese name", () => {
    const t = { ...DEFAULT_FRAME_TEMPLATE, name: "Shop frame", name_ja: "" };
    expect(frameTemplateName(t, "ja")).toBe("Shop frame");
  });
});

describe("frameTemplateSummary (一覧・情報行の要約)", () => {
  // ja: 情報行は用紙・ゾーン分割・表題欄の大きさ・改訂欄の有無をまとめて示す
  it("summarises the paper, the zone grid, the title block size and the revision table", () => {
    expect(frameTemplateSummary(DEFAULT_FRAME_TEMPLATE)).toEqual({
      paper: "420×297",
      zones: "4×6",
      titleBlock: "120×32",
      revisionRows: 6,
    });
  });
});

describe("bindNames (フィールドバインド表)", () => {
  // ja: 表題欄が使っているバインドを、出てくる順に重複なく並べる(1つのセルに複数書いてもよい)
  it("lists the binds the title block uses once each, in the order they appear", () => {
    expect(bindNames(DEFAULT_FRAME_TEMPLATE)).toEqual([
      "title_block.drawing_no",
      "rev.latest",
      "title_block.title",
      "scale",
      "date",
      "title_block.designed",
      "title_block.drawn",
      "title_block.checked",
      "approver",
      "title_block.company",
    ]);
  });

  // ja: バインドを1つも使わない図枠では表は空になる(見出しだけの図枠も作れる)
  it("comes back empty for a frame whose title block holds only fixed text", () => {
    const t = structuredClone(DEFAULT_FRAME_TEMPLATE);
    t.title_block.cells = [{ x: 2, y: 5.5, text: "図番", size: 3 }];
    expect(bindNames(t)).toEqual([]);
  });
});

// 図枠テンプレートの拡張 (辺別余白・マーク・罫線の太さ・セルの回転/枠/収め・改訂欄の列・ロゴ画像)。
// Rust側 frame_template.rs / svg.rs (frame_marks, frame_cell, revision_table) と同じ規則・同じ数値。
describe("frame template extensions", () => {
  const base = (): FrameTemplate => structuredClone(jisA3) as unknown as FrameTemplate;
  const a0 = (): Sheet => sheet([], { size: "A0", orientation: "Landscape" });
  const sixColumns = (): FrameTemplate => {
    const t = base();
    t.revision_block.columns = [
      { field: "mark", header: "SYMB", w: 14 },
      { field: "description", header: "CHANGELOG", w: 56 },
      { field: "rev_no", header: "ECN NO", w: 16, align: "middle" },
      { field: "date", header: "DATE", w: 12, align: "middle" },
      { field: "by", header: "BY", w: 11, align: "middle" },
      { field: "approved", header: "OKAY", w: 11, align: "end" },
    ];
    t.revision_block.row_h = 7;
    return t;
  };
  const rev6 = (over: Partial<Revision> = {}): Revision => ({
    mark: "R1", date: "24/01", description: "変更", by: "KT", rev_no: "E-204", approved: "MU", count: "x3", ...over,
  });

  // ja: 新しい項目の無いテンプレートは今までどおりの値を返す(4辺同じ余白、細い罫線、書いたとおりの文字サイズ、従来の改訂欄4列)
  it("keeps today's values for a template without the new items", () => {
    const t = base();
    expect(frameMargins(t)).toEqual({ left: 10, right: 10, top: 10, bottom: 10 });
    expect(frameRect(t, 420, 297)).toEqual({ x0: 10, y0: 10, x1: 410, y1: 287 });
    expect(fontSize(t, 3)).toBe(3);
    expect(usesExtendedRevisionTable(t)).toBe(false);
    expect(revisionColumns(t).map((c) => c.field)).toEqual(["mark", "date", "description", "by"]);
    expect(revisionColumns(t).map((c) => c.header)).toEqual(["記号", "日付", "内容", "承認"]);
    const l = frameLayout(sheet(["A"]), t);
    expect(l.zones.show).toBe(true);
    expect(l.marks).toEqual([]);
    expect(l.titleBlock.gridLines.every((g) => g.stroke === 0.25)).toBe(true);
    expect(l.titleBlock.cells.every((c) => c.rotation === 0 && c.box === null && c.lines.length === 1)).toBe(true);
    expect(l.revision!.rows[0].triangle).toBeNull();
  });

  // ja: 辺ごとの余白を指定すると外枠と表題欄の隅がそれに従い、基準隅が左上ならその隅に置かれる
  it("moves the frame and the title block corner with margins per side", () => {
    const t = base();
    t.frame_margins = { left: 30, right: 15, top: 12, bottom: 10 };
    const l = frameLayout(a0(), t);
    expect(l.frame).toEqual({ x0: 30, y0: 12, x1: 1174, y1: 831 });
    expect(l.titleBlock.x).toBe(1174 - t.title_block.w);
    expect(l.titleBlock.y).toBe(831 - t.title_block.h);
    t.title_block.origin = "top_left";
    expect(titleBlockOrigin(t, 1189, 841)).toEqual({ x: 30, y: 12 });
  });

  // ja: ゾーン目盛を隠すとレイアウトのshowがfalseになる
  it("reports hidden zone labels", () => {
    const t = base();
    t.zones.show = false;
    expect(frameLayout(a0(), t).zones.show).toBe(false);
  });

  // ja: 裁断マークは用紙四隅のL字で線8本、中心マークは外枠の各辺の中央から外向きに4本(長さは余白、最大10mm)
  it("lays out eight trim mark lines and four centre mark lines", () => {
    const t = base();
    t.frame_margins = { left: 30, right: 15, top: 12, bottom: 6 };
    t.marks = { trim: true, centre: true };
    const m = frameLayout(a0(), t).marks;
    expect(m).toHaveLength(12);
    expect(m).toContainEqual({ x1: 4, y1: 4, x2: 19, y2: 4, stroke: 0.5 });
    expect(m).toContainEqual({ x1: 4, y1: 4, x2: 4, y2: 19, stroke: 0.5 });
    expect(m).toContainEqual({ x1: 1185, y1: 4, x2: 1170, y2: 4, stroke: 0.5 });
    expect(m).toContainEqual({ x1: 4, y1: 837, x2: 4, y2: 822, stroke: 0.5 });
    expect(m).toContainEqual({ x1: 1185, y1: 837, x2: 1170, y2: 837, stroke: 0.5 });
    expect(m).toContainEqual({ x1: 602, y1: 12, x2: 602, y2: 2, stroke: 0.5 });
    expect(m).toContainEqual({ x1: 602, y1: 835, x2: 602, y2: 841, stroke: 0.5 });
    expect(m).toContainEqual({ x1: 30, y1: 423.5, x2: 20, y2: 423.5, stroke: 0.5 });
    expect(m).toContainEqual({ x1: 1174, y1: 423.5, x2: 1184, y2: 423.5, stroke: 0.5 });
  });

  // ja: 罫線の太さは thin 0.25 / medium 0.5 / heavy 1 mm
  it("gives each rule its stroke width", () => {
    const t = base();
    t.title_block.grid_lines[0].weight = "medium";
    t.title_block.grid_lines[1].weight = "heavy";
    const g = frameLayout(sheet(), t).titleBlock.gridLines;
    expect([g[0].stroke, g[1].stroke, g[2].stroke]).toEqual([0.5, 1, 0.25]);
  });

  // ja: 大文字高さの指定ではフォントはサイズ÷0.72。fitのセルは幅に収まるまで縮み、収まる文字と幅の無いセルは縮まない
  it("converts capital heights and shrinks fitted cells only when they overflow", () => {
    const t = base();
    const cell = (over: object) => {
      t.title_block.cells = [{ x: 10, y: 20, w: 9, text: "ABCDEFGHIJ", size: 3, fit: true, ...over }];
      return frameLayout(sheet(), t).titleBlock.cells[0].size;
    };
    expect(cell({})).toBeCloseTo(1.5, 6);
    expect(cell({ w: 40 })).toBe(3);
    expect(cell({ w: null })).toBe(3);
    expect(cell({ fit: false })).toBe(3);
    expect(fittedFont(["AB", "ABCDEFGHIJ"], 3, 9)).toBeCloseTo(1.5, 6);
    t.text_size_is = "cap";
    expect(cell({ size: 3.6, fit: false })).toBeCloseTo(5, 6);
  });

  // ja: 改行を含むセルは複数行になり、回転と枠(文字の箱の0.8mm外側、揃えに従う)がレイアウトに入る
  it("splits lines and carries rotation and the box", () => {
    const t = base();
    const cell = (over: object) => {
      t.title_block.cells = [{ x: 10, y: 20, text: "ABCDEFGHIJ", size: 3, boxed: true, ...over }];
      return frameLayout(sheet(), t).titleBlock.cells[0];
    };
    const c = cell({ rotation: 90 });
    expect(c.rotation).toBe(90);
    // 表題欄の左上は (290, 255) → セルは (300, 275)。幅18、フォント3
    expect(c.box!.x).toBeCloseTo(299.2, 6);
    expect(c.box!.y).toBeCloseTo(271.2, 6);
    expect(c.box!.w).toBeCloseTo(19.6, 6);
    expect(c.box!.h).toBeCloseTo(5.35, 6);
    expect(cell({ align: "middle" }).box!.x).toBeCloseTo(290.2, 6);
    expect(cell({ align: "end" }).box!.x).toBeCloseTo(281.2, 6);
    const two = cell({ text: "AB\nCD", boxed: false });
    expect(two.lines).toEqual(["AB", "CD"]);
    expect(two.box).toBeNull();
  });

  // ja: 独自の列を持つ改訂欄は、テンプレートの見出しと各列の項目を、指定の行高と揃えで配置する
  it("lays out a configured revision table", () => {
    const s = sheet([]);
    s.revisions = [rev6()];
    const l = revisionLayout(s, sixColumns())!;
    expect([l.x, l.y, l.w, l.h, l.rowH]).toEqual([290, 241, 120, 14, 7]);
    expect(l.ruleYs).toEqual([248]);
    expect(l.columns.map((c) => c.left)).toEqual([290, 304, 360, 376, 388, 399]);
    expect(l.columns.map((c) => c.header.lines[0])).toEqual(["SYMB", "CHANGELOG", "ECN NO", "DATE", "BY", "OKAY"]);
    // 見出し行は最下段 (248〜255)。フォント2.2 → ベースライン 248 + 3.5 + 1.1
    expect(l.columns[0].header.baselines[0]).toBeCloseTo(252.6, 6);
    expect(l.columns[0].header.x).toBe(292);
    const cells = l.rows[0].cells;
    expect(cells.map((c) => c.lines.join("|"))).toEqual(["R1", "変更", "E-204", "24/01", "KT", "MU"]);
    // ECN NO 列 (左端360、幅16) は中央揃え → x=368、行上端241 → ベースライン 241 + 3.5 + 1.25
    expect([cells[2].x, cells[2].align, cells[2].size]).toEqual([368, "middle", 2.5]);
    expect(cells[2].baselines[0]).toBeCloseTo(245.75, 6);
    // OKAY 列は右揃え → 410 - 2
    expect([cells[5].x, cells[5].align]).toEqual([408, "end"]);
  });

  // ja: 行数を固定すると、改訂が無くても表が配置され、改訂のほうが多いときは新しい行だけになり、表の高さは変わらない
  it("keeps a fixed number of rows", () => {
    const t = sixColumns();
    t.revision_block.rows_fixed = 3;
    const empty = revisionLayout(sheet([]), t)!;
    expect([empty.y, empty.h]).toEqual([227, 28]);
    expect(empty.ruleYs).toEqual([234, 241, 248]);
    expect(empty.rows).toHaveLength(0);
    const full = revisionLayout(sheet(["1", "2", "3", "4", "5"]), t)!;
    expect([full.y, full.h]).toEqual([227, 28]);
    expect(full.rows.map((r) => r.rev.mark)).toEqual(["3", "4", "5"]);
    // 古い行ほど下: "3" が見出しの直上 (241〜248)、"5" が最上段 (227〜234)
    expect(full.rows.map((r) => r.top)).toEqual([241, 234, 227]);
  });

  // ja: 列より長い内容は列幅まで縮み、2行の内容は行の中央に2行で並び、3行の内容は行高に収まるまで縮む
  it("fits long, two-line and three-line descriptions inside the cell", () => {
    const one = (description: string) => {
      const s = sheet([]);
      s.revisions = [rev6({ description })];
      return revisionLayout(s, sixColumns())!.rows[0].cells[1];
    };
    // 40全角 × 2.5 = 100mm → 列の内側 56 - 4 = 52mm: 2.5 × 52 / 100 = 1.3
    expect(one("あ".repeat(40)).size).toBeCloseTo(1.3, 6);
    const two = one("一行目\n二行目");
    expect(two.lines).toEqual(["一行目", "二行目"]);
    expect(two.size).toBe(2.5);
    // 行上端241: 241 + 3.5 + 1.25 - 3.125/2 = 244.1875、次の行は +3.125
    expect(two.baselines[0]).toBeCloseTo(244.1875, 6);
    expect(two.baselines[1]).toBeCloseTo(247.3125, 6);
    // 3行: 3 × 2.5 × 1.25 = 9.375 > 7 → 2.5 × 7 / 9.375
    const three = one("a\nb\nc");
    expect(three.size).toBeCloseTo((2.5 * 7) / 9.375, 6);
    expect(three.baselines[2] - three.baselines[0]).toBeCloseTo(2 * three.size * 1.25, 6);
  });

  // ja: 記号の形が三角なら、記号は上向きの三角の中に中央揃えで置かれる
  it("adds a triangle around each mark when the mark shape is a triangle", () => {
    const t = sixColumns();
    t.revision_block.mark_shape = "triangle";
    const s = sheet([]);
    s.revisions = [rev6()];
    const row = revisionLayout(s, t)!.rows[0];
    // th = 5.6、hb = 5.6/√3、cx = 290 + 2 + hb、底辺 by = 241 + 3.5 + 2.8 = 247.3
    const hb = 5.6 / Math.sqrt(3);
    const cx = 292 + hb;
    expect(row.triangle).toHaveLength(3);
    expect(row.triangle![0][0]).toBeCloseTo(cx, 6);
    expect(row.triangle![0][1]).toBeCloseTo(247.3 - 5.6, 6);
    expect(row.triangle![1][0]).toBeCloseTo(cx + hb, 6);
    expect(row.triangle![2][0]).toBeCloseTo(cx - hb, 6);
    const mark = row.cells[0];
    expect(mark.align).toBe("middle");
    expect(mark.x).toBeCloseTo(cx, 6);
    expect(mark.baselines[0]).toBeCloseTo(247.3 - 5.6 * 0.12, 6);
    expect(mark.size).toBeCloseTo(2.5, 6);
  });

  // ja: 大文字高さだけを指定したテンプレートは、従来の4列と日本語の見出しのまま、文字を0.72で割った大きさで配置する
  it("lays out the classic four columns with capital-height sizes", () => {
    const t = base();
    t.text_size_is = "cap";
    expect(usesExtendedRevisionTable(t)).toBe(true);
    const l = revisionLayout(sheet(["A"]), t)!;
    expect(l.columns.map((c) => c.header.lines[0])).toEqual(["記号", "日付", "内容", "承認"]);
    expect(l.columns[0].header.size).toBeCloseTo(2.2 / 0.72, 6);
    expect(l.rows[0].cells[0].size).toBeCloseTo(2.5 / 0.72, 6);
  });

  // ja: 読み込んだロゴ画像はレイアウトのdataUrlに入り、画像が無ければnullになる
  it("carries the loaded logo image", () => {
    const t = base();
    t.logo = { x: 15, y: 15, w: 40, h: 20, path: "l.png", data_url: "data:image/png;base64,AAAA" };
    expect(frameLayout(sheet(), t).logo).toEqual({ x: 15, y: 15, w: 40, h: 20, dataUrl: "data:image/png;base64,AAAA" });
    t.logo = { x: 15, y: 15, w: 40, h: 20 };
    expect(frameLayout(sheet(), t).logo).toEqual({ x: 15, y: 15, w: 40, h: 20, dataUrl: null });
  });
});

// 裁断マーク・中心マークの形、改訂欄の文字サイズ・記号の三角の形・同じセルに続ける項目。
// Rust側 svg.rs (frame_marks, mark_triangle, revision_table) と同じ規則・同じ数値。
describe("mark and revision styles", () => {
  const base = (): FrameTemplate => structuredClone(jisA3) as unknown as FrameTemplate;
  const a0 = (): Sheet => sheet([], { size: "A0", orientation: "Landscape" });
  const bindingEdgeMarks = (): FrameTemplate => {
    const t = base();
    t.frame_margins = { left: 30, right: 20, top: 18, bottom: 16 };
    t.marks = {
      trim: true,
      centre: true,
      trim_insets: { left: 4, right: 6, top: 5, bottom: 3 },
      centre_to_trim: true,
      centre_arrow: true,
      centre_weight: "thin",
    };
    return t;
  };
  const styled = (): FrameTemplate => {
    const t = base();
    t.revision_block.columns = [
      { field: "mark", header: "SYMB", w: 14, suffix: "count" },
      { field: "description", header: "CHANGELOG", w: 56 },
      { field: "rev_no", header: "ECN NO", w: 16, align: "middle" },
      { field: "date", header: "DATE", w: 12, align: "middle" },
      { field: "by", header: "BY", w: 22, align: "middle", suffix: "approved" },
    ];
    t.revision_block.row_h = 7;
    t.revision_block.rows_fixed = 2;
    t.revision_block.mark_shape = "triangle";
    t.revision_block.mark_style = { h: 1, aspect: 0.8, gap: 0.5, pad: 0, in_empty_rows: true };
    return t;
  };
  const one = (over: Partial<Revision> = {}): Sheet => {
    const s = sheet([]);
    s.revisions = [
      { mark: "1", date: "24/01", description: "変更", by: "KT", rev_no: "E-204", approved: "MU", count: "x3", ...over },
    ];
    return s;
  };
  const close = (got: number[], want: number[]) => want.forEach((w, i) => expect(got[i]).toBeCloseTo(w, 6));

  // ja: 裁断マークは辺ごとに指定した位置に従い、裁断線の四隅それぞれに腕が2本、線幅0.5で配置される
  it("lays out trim marks at the insets given per side", () => {
    const m = frameLayout(a0(), bindingEdgeMarks()).marks.filter((l) => l.stroke === 0.5);
    expect(m).toHaveLength(8);
    const has = (x1: number, y1: number, x2: number, y2: number) =>
      m.some((l) => [l.x1 - x1, l.y1 - y1, l.x2 - x2, l.y2 - y2].every((d) => Math.abs(d) < 1e-9));
    expect(has(4, 5, 19, 5)).toBe(true);
    expect(has(4, 5, 4, 20)).toBe(true);
    expect(has(1183, 5, 1168, 5)).toBe(true);
    expect(has(1183, 5, 1183, 20)).toBe(true);
    expect(has(4, 838, 19, 838)).toBe(true);
    expect(has(4, 838, 4, 823)).toBe(true);
    expect(has(1183, 838, 1168, 838)).toBe(true);
    expect(has(1183, 838, 1183, 823)).toBe(true);
  });

  // ja: 裁断線まで伸ばす中心マークは、外枠の各辺の中央から裁断線まで指定の太さで引かれ、その端に開いた矢印が付く
  it("runs centre marks to the trim line with an open arrow head", () => {
    const l = frameLayout(a0(), bindingEdgeMarks());
    const c = l.marks.filter((x) => x.stroke === 0.25);
    expect(c).toHaveLength(4);
    close([c[0].x1, c[0].y1, c[0].x2, c[0].y2], [599.5, 18, 599.5, 5]);
    close([c[1].x1, c[1].y1, c[1].x2, c[1].y2], [599.5, 825, 599.5, 838]);
    close([c[2].x1, c[2].y1, c[2].x2, c[2].y2], [30, 421.5, 4, 421.5]);
    close([c[3].x1, c[3].y1, c[3].x2, c[3].y2], [1169, 421.5, 1183, 421.5]);
    expect(l.markHeads).toHaveLength(4);
    expect(l.markHeads.every((h) => h.stroke === 0.25)).toBe(true);
    close(l.markHeads[0].points.flat(), [600, 6.9, 599.5, 5, 599, 6.9]);
    close(l.markHeads[2].points.flat(), [5.9, 421, 4, 421.5, 5.9, 422]);
  });

  // ja: 余白の無い辺には中心マークを配置せず、矢印を頼んでいなければ矢印も無い
  it("lays out no centre mark where there is no room, and no heads unless asked", () => {
    const t = base();
    t.frame_margins = { left: 10, right: 10, top: 10, bottom: 0 };
    t.marks = { centre: true };
    const l = frameLayout(a0(), t);
    expect(l.marks).toHaveLength(3);
    expect(l.markHeads).toEqual([]);
  });

  // ja: 改訂欄に指定した文字サイズが改訂行と見出しに使われる
  it("uses the revision table's own text sizes", () => {
    const t = styled();
    t.revision_block.text_size = 2;
    t.revision_block.header_size = 1.5;
    expect(usesExtendedRevisionTable({ ...base(), revision_block: { ...base().revision_block, text_size: 2 } })).toBe(true);
    const l = revisionLayout(one(), t)!;
    expect(l.columns[0].header.size).toBe(1.5);
    expect(l.rows[0].cells[2].size).toBe(2);
    expect(l.rows[0].cells[2].baselines[0]).toBeCloseTo(241 + 3.5 + 1, 6);
  });

  // ja: 形を指定した記号の三角は、行と同じ高さ・指定の縦横比で列の左端から始まり、斜辺の中ほどを空け、改訂の無い行にも配置される
  it("lays out a styled mark triangle in every row", () => {
    const l = revisionLayout(one(), styled())!;
    expect(l.markShapes.polygons).toEqual([]);
    // 1行あたり 斜辺2本×2 + 底辺 = 5本、2行で10本
    expect(l.markShapes.lines).toHaveLength(10);
    const row = l.markShapes.lines.slice(0, 5).map((s) => [s.x1, s.y1, s.x2, s.y2]);
    close(row[0], [292.8, 241, 293.5, 242.75]);
    close(row[1], [294.9, 246.25, 295.6, 248]);
    close(row[2], [292.8, 241, 292.1, 242.75]);
    close(row[3], [290.7, 246.25, 290, 248]);
    close(row[4], [290, 248, 295.6, 248]);
    const empty = l.markShapes.lines[5];
    close([empty.x1, empty.y1, empty.x2, empty.y2], [292.8, 234, 293.5, 235.75]);
    const mark = l.rows[0].cells[0];
    expect([mark.align, mark.size, mark.lines]).toEqual(["middle", 2.5, ["1"]]);
    close([mark.x, mark.baselines[0]], [292.8, 247.16]);
    // 切れ目の無い既定の三角は多角形になる
    const t = styled();
    t.revision_block.mark_style = { in_empty_rows: true };
    const plain = revisionLayout(one(), t)!;
    expect(plain.markShapes.lines).toEqual([]);
    expect(plain.markShapes.polygons).toHaveLength(2);
  });

  // ja: 列のsuffixの項目は、記号の列では三角の横に、その他の列では主の文字のあとに続けて同じセルに配置される
  it("puts a suffix field in the same cell", () => {
    const l = revisionLayout(one(), styled())!;
    const extra = l.rows[0].extras;
    expect(extra).toHaveLength(1);
    expect([extra[0].align, extra[0].size, extra[0].lines]).toEqual(["start", 2.5, ["x3"]]);
    close([extra[0].x, extra[0].baselines[0]], [296.6, 245.75]);
    const by = l.rows[0].cells[4];
    expect(by.lines).toEqual(["KT MU"]);
    close([by.x, by.baselines[0], by.size], [399, 245.75, 2.5]);
    const none = revisionLayout(one({ count: "", approved: "" }), styled())!;
    expect(none.rows[0].extras).toEqual([]);
    expect(none.rows[0].cells[4].lines).toEqual(["KT"]);
  });

  // ja: 見出しの揃えを別に指定した列は、見出しをその揃えで、改訂行を列の揃えで配置する
  it("aligns a header apart from its rows", () => {
    const t = styled();
    t.revision_block.columns![1].header_align = "middle";
    const l = revisionLayout(one(), t)!;
    expect([l.columns[1].header.align, l.columns[1].header.x]).toEqual(["middle", 332]);
    expect([l.rows[0].cells[1].align, l.rows[0].cells[1].x]).toEqual(["start", 306]);
  });

  // ja: 三角より長い記号は三角に収まるまで縮み、値が空の枠付きセルには枠が無い
  it("fits a long mark to the triangle and draws no box around an empty cell", () => {
    const t = base();
    t.revision_block.row_h = 7;
    t.revision_block.mark_shape = "triangle";
    const mark = revisionLayout(one({ mark: "R100" }), t)!.rows[0].cells[0];
    // th = 5.6、hb = 5.6/√3 = 3.233。"R100" は 6mm → 2.5 × 3.233 / 6
    expect(mark.size).toBeCloseTo((2.5 * (5.6 / Math.sqrt(3))) / 6, 6);
    t.title_block.cells = [{ x: 10, y: 20, text: "", size: 3, boxed: true }];
    expect(frameLayout(sheet(), t).titleBlock.cells[0].box).toBeNull();
  });
});

// ストロークフォントが有効なときの図枠 (Rust側 svg.rs の text_advance / latin_scope と同じ)
describe("frame layout with a stroke font", () => {
  const base = (): FrameTemplate => structuredClone(jisA3) as unknown as FrameTemplate;
  // 作り物のフォント: 半角 "A" "B" と、その全角 "Ａ" "Ｂ"
  const g = (wide: boolean) => ({ wide, strokes: [[1, 0, 1, 25]] });
  const font: StrokeFontData = { id: "t", baseline: 0, cap_top: 25, glyphs: { A: g(false), B: g(false), "Ａ": g(true), "Ｂ": g(true) } };
  const cellOf = (t: FrameTemplate) => frameLayout(sheet(), t).titleBlock.cells[0];

  // ja: 幅に収めるセルは、ストロークフォントの線の幅と字間で縮み、フォントを外すと見積もりの幅で縮む
  it("fits a cell by the stroke width and its spacing", () => {
    const t = base();
    t.title_block.cells = [{ x: 10, y: 20, w: 5.4, text: "AB", size: 10, fit: true, spacing: 1.5 }];
    setActiveStrokeFont(font, "half");
    // 幅 = 4.32 × 1.5 + 4.32 = 10.8 → 5.4 に収めると 5
    expect(cellOf(t).size).toBeCloseTo(5, 9);
    expect(cellOf(t).spacing).toBe(1.5);
    setActiveStrokeFont(null, "half");
    // 見積もり: 2 × 0.6 × 10 = 12 → 5.4/12 × 10 = 4.5
    expect(cellOf(t).size).toBeCloseTo(4.5, 9);
  });

  // ja: 英数字を全角にするテンプレートは、図枠の文字だけを全角の字の幅で測る
  it("measures frame text with full-width Latin letters when the template asks for them", () => {
    const t = base();
    t.title_block.cells = [{ x: 10, y: 20, w: 9, text: "AB", size: 10, fit: true }];
    setActiveStrokeFont(font, "half");
    expect(cellOf(t).size).toBe(10);
    t.text_latin = "full";
    // 全角の幅 = 17.28 → 9 に収めると 10 × 9 / 17.28
    expect(cellOf(t).size).toBeCloseTo((10 * 9) / 17.28, 9);
    setActiveStrokeFont(null, "half");
  });
});

