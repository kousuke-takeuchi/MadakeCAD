// 改訂欄 (ISO 7200 / JIS Z 8311) のレイアウト計算。Rust側 svg.rs と同一ルールであること。
import { describe, expect, it } from "vitest";

import type { Primitive, Revision, Sheet, SymbolDef, SymbolInstance } from "../ipc";
import { dynamicSymbol, TERMINAL_MARK_R, TERMINAL_NUMBER_HEIGHT } from "./dynamicSymbol";
import {
  REV_COL_W,
  REV_HEADERS,
  drawDeviceBlock,
  drawSymbol,
  effectiveRev,
  revisionLayout,
  rotateArcAngles,
  transformLocal,
  visibleRevisions,
} from "./renderer";
import {
  CONTACT_MAP_FONT,
  deviceBlockBounds,
  deviceBlockTexts,
  type DeviceBlock,
} from "./relayXref";
import { theme } from "./theme";
import { JUMPER_BAR_STROKE_MM } from "./terminalJumpers";
import { Viewport } from "./viewport";

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

describe("rotateArcAngles", () => {
  const inst = (rotation: number, mirror = false): SymbolInstance => ({
    id: "s",
    symbol_id: "contactor_3p",
    at: { x: 0, y: 0 },
    rotation,
    mirror,
    reference: "K1",
    value: "",
    attrs: {},
  });

  // ja: シンボル内の弧はシンボルと一緒に回るので、電磁接触器の半円はどの回転角でも可動接点の側を向く
  it("turns arcs with the symbol so the contactor half circle keeps facing its moving contact", () => {
    expect(rotateArcAngles(0, 180, inst(0))).toEqual([0, 180]);
    expect(rotateArcAngles(0, 180, inst(90))).toEqual([90, 270]);
    expect(rotateArcAngles(0, 180, inst(180))).toEqual([180, 360]);
    expect(rotateArcAngles(0, 180, inst(270))).toEqual([270, 450]);
  });

  // ja: ミラーは弧を左右反転させるが、描画方向(開始→終了を時計回り)は保つ
  it("mirrors arcs left to right while keeping the clockwise drawing direction", () => {
    expect(rotateArcAngles(0, 180, inst(0, true))).toEqual([0, 180]);
    expect(rotateArcAngles(270, 450, inst(0, true))).toEqual([90, 270]);
  });
});

describe("drawSymbol text placement", () => {
  /** fillText / arc / stroke の呼び出しだけを記録する最小のキャンバス。 */
  function recorder() {
    const texts: { text: string; x: number; y: number }[] = [];
    const arcs: { x: number; y: number; r: number }[] = [];
    const strokeWidths: number[] = [];
    const ctx = {
      strokeStyle: "",
      fillStyle: "",
      lineWidth: 0,
      font: "",
      textAlign: "" as CanvasTextAlign,
      beginPath() {},
      moveTo() {},
      lineTo() {},
      closePath() {},
      stroke() {
        strokeWidths.push(ctx.lineWidth);
      },
      fill() {},
      arc(x: number, y: number, r: number) {
        arcs.push({ x, y, r });
      },
      fillText(text: string, x: number, y: number) {
        texts.push({ text, x, y });
      },
    };
    return { ctx: ctx as unknown as CanvasRenderingContext2D, texts, arcs, strokeWidths };
  }

  /** 端子台1台を画面に描いて、記録したfillText/arcを返す。 */
  function drawTerminalBlock(symbolId: string, jumpers: string, rotation: number, mirror = false) {
    const def = dynamicSymbol(symbolId)!;
    const rec = recorder();
    const vp = new Viewport();
    vp.scale = 4;
    const inst: SymbolInstance = {
      id: "s",
      symbol_id: symbolId,
      at: { x: 100, y: 50 },
      rotation,
      mirror,
      reference: "",
      value: "",
      attrs: jumpers ? { jumpers } : {},
    };
    drawSymbol(rec.ctx, vp, inst, def, false, undefined, false);
    return { ...rec, def, vp, inst };
  }

  /** モータのように、外形の中心に文字を1つ持つシンボル。 */
  const motor: SymbolDef = {
    id: "motor",
    name: "Motor",
    name_ja: "モータ",
    category: "output",
    ref_prefix: "M",
    primitives: [
      { type: "circle", center: { x: 0, y: 0 }, r: 5, filled: false },
      { type: "text", at: { x: 0, y: 0 }, text: "M", height: 4 },
    ],
    pins: [],
  };

  const placed = (rotation: number, mirror = false): SymbolInstance => ({
    id: "s",
    symbol_id: "motor",
    at: { x: 100, y: 50 },
    rotation,
    mirror,
    reference: "",
    value: "",
    attrs: {},
  });

  const draw = (rotation: number, mirror = false) => {
    const { ctx, texts } = recorder();
    const vp = new Viewport();
    vp.scale = 4;
    drawSymbol(ctx, vp, placed(rotation, mirror), motor, false, undefined, false);
    return texts;
  };

  // ja: シンボルの中の文字は、シンボルを回しても反転しても定義どおりの場所に描かれる
  it("keeps the lettering inside a symbol where the symbol definition puts it, whatever the rotation", () => {
    for (const [rotation, mirror] of [
      [0, false],
      [90, false],
      [180, false],
      [270, false],
      [0, true],
    ] as const) {
      const texts = draw(rotation, mirror);
      expect(texts, `rot=${rotation} mirror=${mirror}`).toHaveLength(1);
      // 円の中心 (用紙100,50 → 画面400,200) を基準に、ベースラインは画面の下向きへ
      expect(texts[0].x, `rot=${rotation} mirror=${mirror}`).toBeCloseTo(400, 9);
      expect(texts[0].y, `rot=${rotation} mirror=${mirror}`).toBeCloseTo(200 + (4 * 4) / 2.8, 9);
    }
  });

  // ja: 1〜2文字の端子番号は、どの向きに回しても反転しても自分の印(丸)の中に丸ごと収まる
  it("keeps a one- or two-character terminal number inside its own mark, whatever the rotation", () => {
    for (const [symbolId, count] of [
      ["terminal_block_4p", 4],
      ["terminal_block_12p", 12],
    ] as const) {
      for (const [rotation, mirror] of [
        [0, false],
        [90, false],
        [180, false],
        [270, false],
        [0, true],
      ] as const) {
        const { texts, def, vp, inst } = drawTerminalBlock(symbolId, "", rotation, mirror);
        expect(texts).toHaveLength(count);
        const marks = def.primitives
          .filter((q): q is Extract<Primitive, { type: "circle" }> => q.type === "circle")
          .map((q) => vp.toScreen(transformLocal(q.center, inst)));
        for (const t of texts) {
          // 文字は中心揃え・ベースライン基準。幅=文字数x文字高x0.6、高さ=文字高x0.7
          const half = (t.text.length * TERMINAL_NUMBER_HEIGHT * 0.6 * vp.scale) / 2;
          const top = t.y - TERMINAL_NUMBER_HEIGHT * 0.7 * vp.scale;
          const corners = [
            [t.x - half, top],
            [t.x + half, top],
            [t.x - half, t.y],
            [t.x + half, t.y],
          ];
          // 四隅が印の中に収まり、文字の中心が印の中心の近く (半径の半分以内) にある
          const centre = [t.x, (top + t.y) / 2];
          const onMark = marks.some(
            (m) =>
              Math.hypot(centre[0] - m.x, centre[1] - m.y) <= (TERMINAL_MARK_R / 2) * vp.scale &&
              corners.every(([x, y]) => Math.hypot(x - m.x, y - m.y) <= TERMINAL_MARK_R * vp.scale),
          );
          expect(onMark, `${symbolId} rot=${rotation} mirror=${mirror}: ${t.text}`).toBe(true);
        }
      }
    }
  });

  // ja: 画面のキャンバスでは、回していない端子台の番号は印の中心から文字高の約1/3(1/2.8)だけ下にベースラインが来る(出力図面の1/2より小さい)
  it("puts an unturned terminal block's number just over a third of a character height below its mark centre on the canvas, a smaller offset than the exported drawing's half", () => {
    for (const symbolId of ["terminal_block_4p", "terminal_block_4p_2l"]) {
      const { texts, def, vp, inst } = drawTerminalBlock(symbolId, "", 0);
      const marks = def.primitives
        .filter((q): q is Extract<Primitive, { type: "circle" }> => q.type === "circle")
        .map((q) => vp.toScreen(transformLocal(q.center, inst)));
      expect(texts).toHaveLength(marks.length);
      // 基準点は印の中心。キャンバスのベースラインは基準点から文字高x scale/2.8 下
      // (SVG側は文字高/2 下。この差は既知で、回転0の描画を変えないために残している)
      const baseline = (TERMINAL_NUMBER_HEIGHT * vp.scale) / 2.8;
      for (const m of marks) {
        const at = texts.filter(
          (q) => Math.abs(q.x - m.x) < 1e-9 && Math.abs(q.y - (m.y + baseline)) < 1e-9,
        );
        expect(at, `${symbolId}: 印(${m.x},${m.y})の番号`).toHaveLength(1);
      }
    }
  });

  // ja: ジャンパバーは太い線だけで描かれ、両端に丸(円)は描かれない
  it("draws a jumper as a bar only, with no round mark at either end", () => {
    const { arcs, strokeWidths, def, vp } = drawTerminalBlock("terminal_block_4p", "1-2,3-4", 0);
    // ジャンパのバーが実際に2本描かれている (太さはジャンパバー専用の線幅)
    const barWidth = JUMPER_BAR_STROKE_MM * vp.scale;
    expect(strokeWidths.filter((w) => Math.abs(w - barWidth) < 1e-9)).toHaveLength(2);
    // それでも描かれる円は端子の印だけ (ジャンパは円をまったく足さない)
    const markCount = def.primitives.filter((q) => q.type === "circle").length;
    expect(arcs).toHaveLength(markCount);
    for (const a of arcs) {
      expect(a.r, `円の半径 ${a.r}`).toBeCloseTo(TERMINAL_MARK_R * vp.scale, 9);
    }
  });
});

describe("drawDeviceBlock", () => {
  /** 線分 (moveTo→lineTo) と文字と矩形と色を記録するキャンバス。 */
  function recorder() {
    const segments: { a: { x: number; y: number }; b: { x: number; y: number } }[] = [];
    const texts: { text: string; x: number; y: number }[] = [];
    const rects: { x: number; y: number; w: number; h: number; style: string }[] = [];
    let cursor = { x: 0, y: 0 };
    const ctx = {
      strokeStyle: "",
      fillStyle: "",
      lineWidth: 0,
      font: "",
      textAlign: "" as CanvasTextAlign,
      beginPath() {},
      moveTo(x: number, y: number) {
        cursor = { x, y };
      },
      lineTo(x: number, y: number) {
        segments.push({ a: cursor, b: { x, y } });
        cursor = { x, y };
      },
      closePath() {},
      stroke() {},
      fill() {},
      arc() {},
      strokeRect(x: number, y: number, w: number, h: number) {
        rects.push({ x, y, w, h, style: ctx.strokeStyle });
      },
      fillText(text: string, x: number, y: number) {
        texts.push({ text, x, y });
      },
    };
    return { ctx: ctx as unknown as CanvasRenderingContext2D, segments, texts, rects };
  }

  /** リレーコイル (A1/A2ピンは±7.5mm)。 */
  const coilDef: SymbolDef = {
    id: "relay_coil",
    name: "Relay coil",
    name_ja: "リレーコイル",
    category: "relay",
    ref_prefix: "K",
    primitives: [{ type: "rect", p1: { x: -5, y: -3 }, p2: { x: 5, y: 3 }, filled: false }],
    pins: [
      { number: "A1", name: "", at: { x: -7.5, y: 0 } },
      { number: "A2", name: "", at: { x: 7.5, y: 0 } },
    ],
  };

  const block: DeviceBlock = {
    reference: "K1",
    partNo: "SAMPLE0001-VERY-LONG-PART-NO",
    contacts: [{ terminals: "13-14", address: "/1.B3", role: "contact_no", entityId: null }],
  };

  const coil = (rotation: number, mirror = false, attrs: Record<string, string> = {}): SymbolInstance => ({
    id: "k1",
    symbol_id: "relay_coil",
    at: { x: 100, y: 50 },
    rotation,
    mirror,
    reference: "K1",
    value: "SAMPLE0001-VERY-LONG-PART-NO",
    attrs,
  });

  const paint = (inst: SymbolInstance, selected = false, showContacts = true) => {
    const rec = recorder();
    const vp = new Viewport();
    vp.scale = 4;
    drawDeviceBlock(rec.ctx, vp, inst, coilDef, block, selected, showContacts);
    return { ...rec, vp, inst };
  };

  // ja: 実機ブロックをどの向きに回しても反転しても、引き出し線は必ず実際のA1/A2ピンの点から出るので、そこに繋いだ配線は繋がったまま (G7)
  it("keeps the lead lines on the coil's own pins, whatever the rotation", () => {
    for (const [rotation, mirror] of [
      [0, false],
      [90, false],
      [180, false],
      [270, false],
      [0, true],
    ] as const) {
      const { segments, vp, inst } = paint(coil(rotation, mirror));
      for (const pin of coilDef.pins) {
        const at = vp.toScreen(transformLocal(pin.at, inst));
        const onPin = (p: { x: number; y: number }) =>
          Math.abs(p.x - at.x) < 1e-6 && Math.abs(p.y - at.y) < 1e-6;
        expect(
          segments.some((s) => onPin(s.a) || onPin(s.b)),
          `rot=${rotation} mirror=${mirror}: ピン${pin.number}から出る引き出し線`,
        ).toBe(true);
      }
    }
  });

  // ja: 実機ブロックの表はどの回転でも正立のままで、外形の位置も大きさも変わらない (回るのはピンだけ)
  it("draws the same upright outline whatever the rotation", () => {
    const flat = paint(coil(0)).rects[0];
    for (const [rotation, mirror] of [
      [90, false],
      [180, false],
      [270, false],
      [0, true],
    ] as const) {
      const r = paint(coil(rotation, mirror)).rects[0];
      expect([r.x, r.y, r.w, r.h], `rot=${rotation} mirror=${mirror}`).toEqual([
        flat.x,
        flat.y,
        flat.w,
        flat.h,
      ]);
    }
  });

  // ja: 実機ブロックの文字はどの回転でも自分のセルの中に収まり、ほかの行と重ならない
  it("keeps every line of text inside its own cell and clear of the others, whatever the rotation", () => {
    for (const [rotation, mirror] of [
      [0, false],
      [90, false],
      [180, false],
      [270, false],
      [0, true],
    ] as const) {
      const inst = coil(rotation, mirror);
      const boxes = deviceBlockTexts(inst, block).map((t) => {
        const w = CONTACT_MAP_FONT * 0.6 * [...t.text].length;
        const x0 = t.align === "center" ? t.at.x - w / 2 : t.at.x;
        return { text: t.text, cell: t.cell, box: { x0, y0: t.at.y - CONTACT_MAP_FONT, x1: x0 + w, y1: t.at.y } };
      });
      expect(boxes.length).toBeGreaterThan(1);
      for (const { text, cell, box } of boxes) {
        expect(
          box.x0 >= cell.min.x - 1e-6 &&
            box.x1 <= cell.max.x + 1e-6 &&
            box.y0 >= cell.min.y - 1e-6 &&
            box.y1 <= cell.max.y + 1e-6,
          `rot=${rotation} mirror=${mirror}: 「${text}」がセルから出ている`,
        ).toBe(true);
      }
      for (let i = 0; i < boxes.length; i += 1) {
        for (let j = i + 1; j < boxes.length; j += 1) {
          const [a, b] = [boxes[i].box, boxes[j].box];
          expect(
            a.x0 >= b.x1 - 1e-6 || b.x0 >= a.x1 - 1e-6 || a.y0 >= b.y1 - 1e-6 || b.y0 >= a.y1 - 1e-6,
            `rot=${rotation} mirror=${mirror}: 「${boxes[i].text}」と「${boxes[j].text}」が重なる`,
          ).toBe(true);
        }
      }
    }
  });

  // ja: 選択中の実機ブロックは、ほかのシンボルと同じように選択色の外形で描かれる
  it("strokes the outline in the selection colour while the coil is selected", () => {
    expect(paint(coil(0), false).rects[0].style).toBe(theme.line);
    expect(paint(coil(0), true).rects[0].style).toBe(theme.selection);
  });

  // ja: 実機ブロックのコイルにも機能ラベルは外形の下に描かれる (G4)
  it("prints the function label under the block outline", () => {
    const { texts, vp, inst } = paint(coil(0, false, { function_label: "運転" }));
    const label = texts.find((t) => t.text === "運転");
    expect(label).toBeDefined();
    const bottom = vp.toScreen(deviceBlockBounds(inst, block).max).y;
    expect(label!.y).toBeGreaterThan(bottom);
  });

  // ja: 表示クラス「参照記号」を隠すと、実機ブロックの接点イメージ(端子対と所在)も消える(外形とヘッダは残る)
  it("hides the contact image rows with the reference-designator view class", () => {
    expect(paint(coil(0), false, true).texts.map((t) => t.text)).toContain("13-14");
    const hidden = paint(coil(0), false, false).texts.map((t) => t.text);
    expect(hidden).not.toContain("13-14");
    expect(hidden).not.toContain("/1.B3");
    expect(hidden).toContain("K1");
  });
});
