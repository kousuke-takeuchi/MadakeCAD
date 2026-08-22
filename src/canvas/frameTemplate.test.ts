// 図枠テンプレート (プロットフレーム、M4 §9) のレイアウト計算とバインド解決。
// Rust側 frame_template.rs / svg.rs と同一の結果になること。
import { describe, expect, it } from "vitest";

import jisA3 from "../../src-tauri/resources/frame_templates/jis-a3.json";
import jisA4Portrait from "../../src-tauri/resources/frame_templates/jis-a4-portrait.json";
import type { Project, Revision, Sheet } from "../ipc";
import {
  DEFAULT_FRAME_TEMPLATE,
  bindContextForProject,
  bindNames,
  bindOverlays,
  bindContextForSheet,
  bindValue,
  cellText,
  frameLayout,
  frameTemplateName,
  frameTemplateSummary,
  resolveBinds,
  revisionLayout,
  titleBlockOrigin,
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
    expect(l.titleBlock.gridLines[0]).toEqual({ x1: 290, y1: 263, x2: 410, y2: 263 });
    expect(l.titleBlock.gridLines[3]).toEqual({ x1: 314, y1: 255, x2: 314, y2: 287 });
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
    expect(frameLayout(sheet(), t).logo).toEqual({ x: 15, y: 15, w: 40, h: 20 });
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
