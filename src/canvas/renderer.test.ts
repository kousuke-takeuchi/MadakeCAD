// 改訂欄 (ISO 7200 / JIS Z 8311) のレイアウト計算。Rust側 svg.rs と同一ルールであること。
import { describe, expect, it } from "vitest";

import type { Entity, Point, Primitive, Revision, Sheet, SymbolDef, SymbolInstance } from "../ipc";
import { dynamicSymbol, TERMINAL_MARK_R, TERMINAL_NUMBER_HEIGHT } from "./dynamicSymbol";
import {
  JUNCTION_DOT_R_MM,
  REV_COL_W,
  REV_HEADERS,
  drawDeviceBlock,
  drawSymbol,
  effectiveRev,
  revisionLayout,
  renderSheet,
  rotateArcAngles,
  transformLocal,
  visibleRevisions,
} from "./renderer";
import {
  CONTACT_MAP_FONT,
  DEVICE_BLOCK_MIN_WIDTH,
  deviceBlockBounds,
  deviceBlockLayout,
  deviceBlockLeadAxis,
  deviceBlockLeadEnd,
  deviceBlockTexts,
  type DeviceBlock,
} from "./relayXref";
import { theme } from "./theme";
import { JUMPER_BAR_STROKE_MM } from "./terminalJumpers";
import type { ViewClass } from "./viewClasses";
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

  /** 端子台1台を、ジャンパに加えて任意のattrs(端子番号の上書きなど)付きで画面に描く。 */
  function drawTerminalBlockWithAttrs(
    symbolId: string,
    attrs: Record<string, string>,
    rotation = 0,
  ) {
    const def = dynamicSymbol(symbolId)!;
    const rec = recorder();
    const vp = new Viewport();
    vp.scale = 4;
    const inst: SymbolInstance = {
      id: "s",
      symbol_id: symbolId,
      at: { x: 100, y: 50 },
      rotation,
      mirror: false,
      reference: "",
      value: "",
      attrs,
    };
    drawSymbol(rec.ctx, vp, inst, def, false, undefined, false);
    return { ...rec, def, vp, inst };
  }

  // ja: attrs["terminal_numbers"]の上書きは、シンボル定義の生のピン番号ではなく上書きラベルを描く (Rust側 svg_draws_the_overridden_terminal_number_instead_of_the_raw_pin_number とparity)
  it("draws the overridden terminal number instead of the symbol's raw pin number", () => {
    const { texts } = drawTerminalBlockWithAttrs("terminal_block_4p", {
      terminal_numbers: "11,12,13,14",
    });
    const bodies = texts.map((t) => t.text).sort();
    expect(bodies).toEqual(["11", "12", "13", "14"]);
    for (const raw of ["1", "2", "3", "4"]) {
      expect(texts.some((t) => t.text === raw), `生のピン番号${raw}が残っている`).toBe(false);
    }
  });

  // ja: 採番後のジャンパバーは、単なる幾何的な近さではなく実際に繋ぐ端子の印刷ラベルへ紐付けて帰属確認できる (Rust側 a_jumper_bar_is_attributed_to_its_terminals_by_label_text_after_renumbering とparity。フェーズ6の「最寄りマーク帰属のテストが端子番号の入れ替わりを検出しない」を閉じる)
  it("attributes a jumper bar between renumbered terminals to the marks whose printed labels it connects", () => {
    const def = dynamicSymbol("terminal_block_4p")!;
    const vp = new Viewport();
    vp.scale = 4;
    const texts: { text: string; x: number; y: number }[] = [];
    const arcs: { x: number; y: number; r: number }[] = [];
    // ジャンパバー専用の線幅で引かれた線分の始点・終点だけを拾う
    const barSegments: { a: { x: number; y: number }; b: { x: number; y: number } }[] = [];
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
        // 線幅はstroke()の前、moveTo/lineToより先に設定される(drawSymbolの描画順)ので、
        // ここでctx.lineWidthを直接見ればそのセグメントの太さが分かる
        if (Math.abs(ctx.lineWidth - JUMPER_BAR_STROKE_MM * vp.scale) < 1e-9) {
          barSegments.push({ a: cursor, b: { x, y } });
        }
        cursor = { x, y };
      },
      closePath() {},
      stroke() {},
      fill() {},
      arc(x: number, y: number, r: number) {
        arcs.push({ x, y, r });
      },
      fillText(text: string, x: number, y: number) {
        texts.push({ text, x, y });
      },
    };
    const inst: SymbolInstance = {
      id: "s",
      symbol_id: "terminal_block_4p",
      at: { x: 100, y: 50 },
      rotation: 0,
      mirror: false,
      reference: "",
      value: "",
      attrs: { terminal_numbers: "P11,P12,P13,P14", jumpers: "P11-P12" },
    };
    drawSymbol(ctx as unknown as CanvasRenderingContext2D, vp, inst, def, false, undefined, false);

    const markFor = (label: string) => {
      const t = texts.find((q) => q.text === label);
      expect(t, `ラベル${label}が描かれていない`).toBeDefined();
      return arcs.reduce((best, a) =>
        Math.hypot(t!.x - a.x, t!.y - a.y) < Math.hypot(t!.x - best.x, t!.y - best.y) ? a : best,
      );
    };
    const p11 = markFor("P11");
    const p12 = markFor("P12");

    expect(barSegments).toHaveLength(1);
    const near = (p: { x: number; y: number }, m: { x: number; y: number }) =>
      Math.hypot(p.x - m.x, p.y - m.y) < TERMINAL_MARK_R * vp.scale;
    const { a, b } = barSegments[0];
    // 印が入れ替わっていたら、あるいはラベルが取り違えられていたら失敗する
    expect((near(a, p11) && near(b, p12)) || (near(a, p12) && near(b, p11))).toBe(true);
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

  /** ローカル座標のピン位置と配置から、引き出し線の軸 (用紙座標の外向き単位ベクトル) を作る。 */
  const leadAxis = (pinAt: Point, inst: SymbolInstance) => {
    const t = transformLocal(deviceBlockLeadAxis(pinAt), inst);
    return { x: t.x - inst.at.x, y: t.y - inst.at.y };
  };

  /** 接点contacts本・型番partNoの実機ブロック。 */
  const demoBlock = (partNo: string, contacts: number): DeviceBlock => ({
    reference: "K1",
    partNo,
    contacts: Array.from({ length: contacts }, (_, i) => ({
      terminals: `${i + 1}3-${i + 1}4`,
      address: `/1.B${i + 1}`,
      role: "contact_no" as const,
      entityId: null,
    })),
  });

  // ja: 外形がどれだけ横に広がっても、回していないコイルの引き出し線は水平のまま、配線が来るピンの高さで外形の左右の辺に届く
  it("keeps the leads on the pin axis however wide the block grows", () => {
    // 最小幅 / 切換接点の行 (19.6mm) / 実際の型番 (24.8mm) の3通り
    const blocks: DeviceBlock[] = [
      demoBlock("", 0),
      {
        ...demoBlock("", 0),
        contacts: [{ terminals: "11-12-14", address: "/1.B3", role: "contact_co", entityId: null }],
      },
      demoBlock("RLY-24V-2CO/24VDC-D", 0),
    ];
    for (const b of blocks) {
      const width = deviceBlockLayout(b).width;
      for (const mirror of [false, true]) {
        const inst = coil(0, mirror);
        const bounds = deviceBlockBounds(inst, b);
        for (const pin of coilDef.pins) {
          const at = transformLocal(pin.at, inst);
          const end = deviceBlockLeadEnd(at, leadAxis(pin.at, inst), bounds);
          if (width <= DEVICE_BLOCK_MIN_WIDTH + 1e-9) {
            expect(end, `幅${width}mm: ピンが縁の上なら引き出し線は無い`).toBeNull();
            continue;
          }
          expect(end, `幅${width}mm mirror=${mirror}: ピン${pin.number}`).not.toBeNull();
          expect(end!.y).toBeCloseTo(at.y, 9);
          expect(Math.min(Math.abs(end!.x - bounds.min.x), Math.abs(end!.x - bounds.max.x))).toBeCloseTo(0, 9);
        }
      }
    }
  });

  // ja: 90度・270度に回したコイルの引き出し線は真上・真下へ伸びて外形の上端・下端で止まり、接点が何行あっても接点イメージの文字を横切らない
  it("runs a turned coil's leads vertically without crossing the contact rows", () => {
    for (const contacts of [1, 3]) {
      const b = demoBlock("P1", contacts);
      for (const rotation of [90, 270]) {
        const inst = coil(rotation);
        const bounds = deviceBlockBounds(inst, b);
        const boxes = deviceBlockTexts(inst, b).map((t) => {
          const w = CONTACT_MAP_FONT * 0.6 * [...t.text].length;
          const x0 = t.align === "center" ? t.at.x - w / 2 : t.at.x;
          return { text: t.text, x0, y0: t.at.y - CONTACT_MAP_FONT, x1: x0 + w, y1: t.at.y };
        });
        for (const pin of coilDef.pins) {
          const at = transformLocal(pin.at, inst);
          const end = deviceBlockLeadEnd(at, leadAxis(pin.at, inst), bounds);
          expect(end, `接点${contacts}行 rot=${rotation}: ピン${pin.number}`).not.toBeNull();
          expect(end!.x).toBeCloseTo(at.x, 9);
          expect(Math.min(Math.abs(end!.y - bounds.min.y), Math.abs(end!.y - bounds.max.y))).toBeCloseTo(0, 9);
          const [lo, hi] = [Math.min(at.y, end!.y), Math.max(at.y, end!.y)];
          for (const t of boxes) {
            const crosses = t.x0 < at.x - 1e-9 && at.x < t.x1 - 1e-9 && t.y0 < hi - 1e-9 && lo < t.y1 - 1e-9;
            expect(crosses, `接点${contacts}行 rot=${rotation}: 「${t.text}」を横切る`).toBe(false);
          }
        }
      }
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

describe("renderSheet mate links", () => {
  /** setLineDashで破線にしたまま引かれた線分だけを記録するキャンバス。 */
  function dashRecorder() {
    const dashed: { from: Point; to: Point }[] = [];
    let dash: number[] = [];
    let last: Point = { x: 0, y: 0 };
    let start: Point = { x: 0, y: 0 };
    const ctx = {
      canvas: { width: 800, height: 600 },
      strokeStyle: "",
      fillStyle: "",
      lineWidth: 0,
      font: "",
      textAlign: "" as CanvasTextAlign,
      beginPath() {},
      moveTo(x: number, y: number) {
        start = { x, y };
        last = start;
      },
      lineTo(x: number, y: number) {
        last = { x, y };
      },
      arcTo() {},
      rect() {},
      closePath() {},
      stroke() {
        if (dash.length) dashed.push({ from: start, to: last });
      },
      fill() {},
      fillRect() {},
      strokeRect() {},
      setLineDash(d: number[]) {
        dash = d;
      },
      measureText: (t: string) => ({ width: t.length * 6 }),
      arc() {},
      fillText() {},
    };
    return { ctx: ctx as unknown as CanvasRenderingContext2D, dashed };
  }

  function connectorPair(): { sheet: Sheet; defs: SymbolDef[] } {
    const def = dynamicSymbol("connector_3p")!;
    const inst = (id: string, reference: string, x: number, mate: string): Entity =>
      ({
        kind: "symbol",
        id,
        symbol_id: "connector_3p",
        at: { x, y: 100 },
        rotation: 0,
        mirror: false,
        reference,
        value: "",
        attrs: { mates_with: mate },
      }) as unknown as Entity;
    const sheet = {
      id: "s1",
      name: "S1",
      size: "A3",
      orientation: "Landscape",
      zone_cols: 4,
      zone_rows: 6,
      title_block: {},
      revisions: [],
      entities: { a: inst("a", "J1", 100, "J12"), b: inst("b", "J12", 130, "J1") },
    } as unknown as Sheet;
    return { sheet, defs: [def] };
  }

  function dashedMateLines(hidden: ViewClass[]): number {
    const { sheet, defs } = connectorPair();
    const rec = dashRecorder();
    const vp = new Viewport();
    renderSheet(rec.ctx, sheet, defs, vp, {
      selection: new Set(),
      cursor: null,
      hidden: new Set(hidden),
      mateLinks: [{ sheetId: "s1", a: "a", b: "b" }],
    });
    return rec.dashed.length;
  }

  // ja: 嵌合対の破線は表示クラス「シンボル」に属し、シンボルを隠すと一緒に消える (結ぶ相手が見えないため)
  it("hides the mate link together with the symbols view class", () => {
    expect(dashedMateLines([])).toBe(1);
    expect(dashedMateLines(["symbols"])).toBe(0);
  });

  // ja: 参照記号を隠しても嵌合対の破線は残る (参照記号の表示クラスには属さない)
  it("keeps the mate link when only the reference-designator class is hidden", () => {
    expect(dashedMateLines(["refs"])).toBe(1);
  });
});

describe("jumper bar view-class gating (Task 3, フェーズ6積み残し)", () => {
  /** ジャンパ付きの端子台1台だけを持つシートを、指定の表示クラスを隠して描き、ジャンパバー専用の線幅で描かれたstroke回数を返す。 */
  function jumperBarStrokeCount(hidden: ViewClass[]): number {
    const def = dynamicSymbol("terminal_block_4p")!;
    const inst = {
      kind: "symbol",
      id: "tb1",
      symbol_id: "terminal_block_4p",
      at: { x: 100, y: 50 },
      rotation: 0,
      mirror: false,
      reference: "TB1",
      value: "",
      attrs: { jumpers: "1-2" },
    } as unknown as Entity;
    const sheet = {
      id: "s1",
      name: "S1",
      size: "A3",
      orientation: "Landscape",
      zone_cols: 4,
      zone_rows: 6,
      title_block: {},
      revisions: [],
      entities: { tb1: inst },
    } as unknown as Sheet;
    const strokeWidths: number[] = [];
    const ctx = {
      canvas: { width: 800, height: 600 },
      strokeStyle: "",
      fillStyle: "",
      lineWidth: 0,
      font: "",
      textAlign: "" as CanvasTextAlign,
      beginPath() {},
      moveTo() {},
      lineTo() {},
      arcTo() {},
      rect() {},
      closePath() {},
      stroke() {
        strokeWidths.push(ctx.lineWidth);
      },
      fill() {},
      fillRect() {},
      strokeRect() {},
      setLineDash() {},
      measureText: (t: string) => ({ width: t.length * 6 }),
      arc() {},
      fillText() {},
    };
    const vp = new Viewport();
    vp.scale = 4;
    renderSheet(ctx as unknown as CanvasRenderingContext2D, sheet, [def], vp, {
      selection: new Set(),
      cursor: null,
      hidden: new Set(hidden),
    });
    const barWidth = JUMPER_BAR_STROKE_MM * vp.scale;
    return strokeWidths.filter((w) => Math.abs(w - barWidth) < 1e-9).length;
  }

  // ja: ジャンパバー(G5)は他のシンボル付随描画(嵌合対の破線など)と同じ表示クラス「シンボル」に属し、シンボルを隠すと一緒に消える (フェーズ6積み残しの解消)
  it("hides the jumper bar together with the symbols view class, matching the other symbol-attached drawings", () => {
    expect(jumperBarStrokeCount([])).toBe(1);
    expect(jumperBarStrokeCount(["symbols"])).toBe(0);
  });

  // ja: 参照記号を隠してもジャンパバーは残る (参照記号の表示クラスには属さない)
  it("keeps the jumper bar when only the reference-designator class is hidden", () => {
    expect(jumperBarStrokeCount(["refs"])).toBe(1);
  });
});

describe("renderSheet dots", () => {
  /** renderSheetが触るCanvas2D APIだけを備えた、arcの記録用キャンバス。 */
  function sheetRecorder() {
    const arcs: { x: number; y: number; r: number }[] = [];
    const ctx = {
      canvas: { width: 800, height: 600 },
      strokeStyle: "",
      fillStyle: "",
      lineWidth: 0,
      font: "",
      textAlign: "" as CanvasTextAlign,
      beginPath() {},
      moveTo() {},
      lineTo() {},
      arcTo() {},
      rect() {},
      closePath() {},
      stroke() {},
      fill() {},
      fillRect() {},
      strokeRect() {},
      setLineDash() {},
      measureText: (t: string) => ({ width: t.length * 6 }),
      arc(x: number, y: number, r: number) {
        arcs.push({ x, y, r });
      },
      fillText() {},
    };
    return { ctx: ctx as unknown as CanvasRenderingContext2D, arcs };
  }

  // ja: 注記の引き出し線の対象端に描く丸印は、配線の分岐点の丸印とぴったり同じ大きさで描かれる (図面上で同じ印に見える。Rust側 svg_leader_dot_is_the_size_of_a_wire_junction_dot とparity)
  it("draws a note's leader dot exactly the size of a wire junction dot", () => {
    const junction = { kind: "junction", id: "j1", at: { x: 150, y: 100 } };
    const note = {
      kind: "text",
      id: "t1",
      at: { x: 120, y: 80 },
      text: "注意",
      height: 2.5,
      rotation: 0,
      anchor: "start",
      color: null,
      bold: false,
      leader_to: "j1",
    };
    const sheet = {
      id: "s1",
      name: "S1",
      size: "A3",
      orientation: "Landscape",
      zone_cols: 4,
      zone_rows: 6,
      title_block: {},
      revisions: [],
      entities: { j1: junction, t1: note },
    } as unknown as Sheet;
    const rec = sheetRecorder();
    const vp = new Viewport();
    vp.scale = 4;
    renderSheet(rec.ctx, sheet, [], vp, { selection: new Set(), cursor: null });
    expect(rec.arcs).toHaveLength(2);
    expect(rec.arcs[0].r).toBe(rec.arcs[1].r);
    expect(rec.arcs[0].r).toBe(Math.max(2, JUNCTION_DOT_R_MM * vp.scale));
  });
});

describe("wire legend block drawing", () => {
  /** 文字と外形矩形を記録するキャンバス。 */
  function legendRecorder() {
    const texts: { text: string; x: number; y: number }[] = [];
    const rects: { x: number; y: number; w: number; h: number; style: string }[] = [];
    const ctx = {
      canvas: { width: 800, height: 600 },
      save() {},
      restore() {},
      translate() {},
      scale() {},
      strokeStyle: "" as string,
      fillStyle: "" as string,
      lineWidth: 0,
      font: "",
      textAlign: "" as CanvasTextAlign,
      beginPath() {},
      moveTo() {},
      lineTo() {},
      closePath() {},
      stroke() {},
      fill() {},
      fillRect() {},
      arc() {},
      setLineDash() {},
      measureText: (t: string) => ({ width: t.length * 6 }),
      strokeRect(x: number, y: number, w: number, h: number) {
        rects.push({ x, y, w, h, style: ctx.strokeStyle });
      },
      fillText(text: string, x: number, y: number) {
        texts.push({ text, x, y });
      },
    };
    return { ctx: ctx as unknown as CanvasRenderingContext2D, texts, rects };
  }

  /** 赤0.75sqの配線2本 (1.2m + 2.0m) と凡例ブロック1つのシート。 */
  function legendSheet(): Sheet {
    const wire = (id: string, length: number): Entity =>
      ({
        kind: "wire",
        id,
        points: [
          { x: 10, y: 10 },
          { x: 30, y: 10 },
        ],
        color: id === "w2" ? "赤" : "red",
        sq: 0.75,
        length_m: length,
        part_no: null,
        net: null,
      }) as unknown as Entity;
    return {
      id: "s1",
      name: "S1",
      size: "A3",
      orientation: "Landscape",
      zone_cols: 4,
      zone_rows: 6,
      title_block: {},
      revisions: [],
      entities: {
        w1: wire("w1", 1.2),
        w2: wire("w2", 2.0),
        l1: { kind: "wire_legend", id: "l1", at: { x: 100, y: 60 } } as unknown as Entity,
      },
    } as unknown as Sheet;
  }

  // ja: 凡例ブロックは列見出しと、シートの配線から導出した行 (色・線径・品番・使用長) を表として描く
  it("draws the header row and the rows derived from the sheet's wires", () => {
    const rec = legendRecorder();
    const vp = new Viewport();
    renderSheet(rec.ctx, legendSheet(), [], vp, {
      selection: new Set(),
      cursor: null,
      wireParts: [{ part_no: "DEMO-R075", color: "赤", sq: 0.75, note: "" }],
    });
    const drawn = rec.texts.map((t) => t.text);
    expect(drawn).toEqual(expect.arrayContaining(["線色", "sq", "電線品番", "使用長 m"]));
    // red と 赤 は同じ電線なので1行にまとまり、使用長は合算されて小数第2位まで出る
    expect(drawn).toEqual(expect.arrayContaining(["赤", "0.75", "DEMO-R075", "3.20"]));
    expect(drawn.filter((t) => t === "赤")).toHaveLength(1);
  });

  // ja: 選択中の凡例ブロックは、外形の矩形を選択色で描く
  it("outlines the legend block in the selection colour while it is selected", () => {
    const rec = legendRecorder();
    const vp = new Viewport();
    renderSheet(rec.ctx, legendSheet(), [], vp, { selection: new Set(["l1"]), cursor: null });
    expect(rec.rects.some((r) => r.style === theme.selection)).toBe(true);
  });
});

describe("hidden designator", () => {
  const plug: SymbolDef = {
    id: "connector_plug",
    name: "plug",
    name_ja: "プラグ",
    category: "connector",
    ref_prefix: "J",
    primitives: [{ type: "line", pts: [{ x: -7.5, y: 0 }, { x: 2.5, y: 0 }] }],
    pins: [{ number: "1", name: "", at: { x: -7.5, y: 0 } }],
  } as SymbolDef;

  const texts = (attrs: Record<string, string>) => {
    const drawn: string[] = [];
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
      stroke() {},
      fill() {},
      arc() {},
      fillText(text: string) {
        drawn.push(text);
      },
    } as unknown as CanvasRenderingContext2D;
    const vp = new Viewport();
    vp.scale = 4;
    const inst: SymbolInstance = {
      id: "j5",
      symbol_id: "connector_plug",
      at: { x: 100, y: 50 },
      rotation: 0,
      mirror: false,
      reference: "J5",
      value: "",
      attrs,
    };
    drawSymbol(ctx, vp, inst, plug, false, undefined, true);
    return drawn;
  };

  // ja: hide_reference="true" のシンボルは参照記号を画面に描かない (嵌合の組の片方が同じ記号を二重に描かない。svg.rsと同ルール)
  it("does not draw the designator when hide_reference is true", () => {
    expect(texts({})).toContain("J5");
    expect(texts({ hide_reference: "true" })).not.toContain("J5");
  });
});

