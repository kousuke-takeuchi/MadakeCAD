// ブロック(汎用機器)のピン表エディタの下ごしらえ (行の編集・属性の組み立て・検証・
// 配線済みピンの判定) の仕様テスト。
// 計画: docs/superpowers/plans/2026-08-23-phase5-reproduction-parity.md Task 2。
import { describe, expect, it } from "vitest";
import {
  attrsFromDraft,
  addRow,
  blankDraft,
  draftFromAttrs,
  draftIssues,
  draftSize,
  draftSymbol,
  droppedWiredPins,
  moveRow,
  nextPinNo,
  portPinCount,
  splitContacts,
  removeRow,
  snapSize,
  wiredPinNumbers,
  type BlockDraft,
} from "./blockEditor";
import { blockSymbol } from "./dynamicSymbol";
import type { Entity, Sheet, SymbolInstance } from "../ipc";

/** 左右にピンを持つ下書き (ピン名つき)。 */
function draft(): BlockDraft {
  return {
    rows: {
      left: [
        { no: "IN1", name: "入力1" },
        { no: "IN2", name: "入力2" },
      ],
      right: [{ no: "OUT1", name: "出力1" }],
      top: [],
      bottom: [],
    },
    size: null,
    pitch: "coarse",
    numbers: "outside",
  };
}

function sheetWith(entities: Entity[]): Sheet {
  return {
    id: "sheet-1",
    name: "制御回路図",
    size: "A3",
    orientation: "Landscape",
    zone_cols: 8,
    zone_rows: 4,
    title_block: {},
    revisions: [],
    entities: Object.fromEntries(entities.map((e) => [e.id, e])),
  };
}

function wire(id: string, points: { x: number; y: number }[]): Entity {
  return { kind: "wire", id, points, color: "red", sq: 0.75, length_m: null, part_no: null, net: null };
}

function blockAt(attrs: Record<string, string>, rotation = 0): SymbolInstance {
  return {
    id: "u1",
    symbol_id: "block",
    at: { x: 100, y: 100 },
    rotation,
    mirror: false,
    reference: "CTRL1",
    value: "",
    attrs,
  };
}

describe("block pin table draft", () => {
  // ja: 新しいブロックの下書きは左右に1本ずつピンがあり、番号は1と2で外形は自動
  it("starts with one pin on each side, numbered 1 and 2, and an automatic outline", () => {
    const d = blankDraft();
    expect(d.rows.left).toEqual([{ no: "1", name: "" }]);
    expect(d.rows.right).toEqual([{ no: "2", name: "" }]);
    expect(d.rows.top).toEqual([]);
    expect(d.rows.bottom).toEqual([]);
    expect(d.size).toBeNull();
  });

  // ja: ピンを追加すると、いま使っている一番大きい番号の次の番号が振られる
  it("numbers a new pin after the highest number already in the table", () => {
    const d = blankDraft();
    expect(nextPinNo(d)).toBe("3");
    addRow(d, "left");
    expect(d.rows.left).toEqual([
      { no: "1", name: "" },
      { no: "3", name: "" },
    ]);
  });

  // ja: 番号が数字でないピンばかりのときは、ピンの本数+1を新しい番号にする
  it("falls back to the pin count plus one when no pin number is a plain number", () => {
    const d = draft();
    expect(nextPinNo(d)).toBe("4");
  });

  // ja: ピンを削除すると、その行だけが表から消える
  it("removes just the deleted row from the table", () => {
    const d = draft();
    removeRow(d, "left", 0);
    expect(d.rows.left).toEqual([{ no: "IN2", name: "入力2" }]);
  });

  // ja: ピンを上下に動かすと隣の行と入れ替わり、端の行はそれ以上動かない
  it("swaps a pin with its neighbour and keeps the ends in place", () => {
    const d = draft();
    moveRow(d, "left", 1, -1);
    expect(d.rows.left.map((r) => r.no)).toEqual(["IN2", "IN1"]);
    moveRow(d, "left", 0, -1);
    expect(d.rows.left.map((r) => r.no)).toEqual(["IN2", "IN1"]);
    moveRow(d, "left", 1, 1);
    expect(d.rows.left.map((r) => r.no)).toEqual(["IN2", "IN1"]);
  });
});

describe("block attributes built from the draft", () => {
  // ja: 配置に使う属性はピンのある辺だけを書き、ピン名が空の行は番号だけを書く
  it("writes only the sides that have pins, and omits empty pin names", () => {
    const d = draft();
    d.rows.right[0].name = "";
    expect(attrsFromDraft(d)).toEqual({
      block_pins: '{"left":[{"no":"IN1","name":"入力1"},{"no":"IN2","name":"入力2"}],"right":[{"no":"OUT1"}]}',
    });
  });

  // ja: 番号もピン名も空の行は書き出さない(表の空行はピンにならない)
  it("drops rows whose number and name are both blank", () => {
    const d = draft();
    d.rows.left.push({ no: "  ", name: " " });
    expect(attrsFromDraft(d).block_pins).not.toContain('{"no":""}');
  });

  // ja: 外形を手動にしたときだけ、寸法が属性に書かれる
  it("writes the outline size only when the size is set by hand", () => {
    const d = draft();
    expect(attrsFromDraft(d).block_size).toBeUndefined();
    d.size = { w: 40, h: 30 };
    expect(attrsFromDraft(d).block_size).toBe('{"w":40,"h":30}');
  });

  // ja: 配置済みブロックの属性を読み戻すと、同じピン表と外形の下書きに戻る
  it("reads a placed block's attributes back into the same table", () => {
    const d = draft();
    d.size = { w: 40, h: 30 };
    expect(draftFromAttrs(attrsFromDraft(d))).toEqual(d);
  });

  // ja: 読めない属性のブロックは空のピン表として開く(その場で作り直せる)
  it("opens a block with unreadable attributes as an empty table", () => {
    const d = draftFromAttrs({ block_pins: "not json" });
    expect(d.rows.left).toEqual([]);
    expect(d.rows.right).toEqual([]);
    expect(d.size).toBeNull();
  });

  // ja: 外形が自動のときの寸法はピンの数で決まり、5mm刻みに丸められる
  it("derives the automatic outline size from the pins, rounded to 5 mm steps", () => {
    const d: BlockDraft = {
      rows: {
        left: [1, 2, 3, 4, 5].map((n) => ({ no: `IN${n}`, name: "" })),
        right: [1, 2, 3].map((n) => ({ no: `OUT${n}`, name: "" })),
        top: [],
        bottom: [],
      },
      size: null,
      pitch: "coarse",
      numbers: "outside",
    };
    expect(draftSize(d)).toEqual({ w: 15, h: 30 });
    d.size = { w: 40, h: 35 };
    expect(draftSize(d)).toEqual({ w: 40, h: 35 });
  });

  // ja: 手動の外形でも、左右のまとまりの名前が同じ行に並んで広がった幅を表示する (44 の 20 x 20 指定はプレビューと同じ幅になる)
  it("reports the widened size of a manual outline whose group names on both sides share a row", () => {
    const row = (no: number, name: string, ref: string) => ({ no: String(no), name, connector: "jst_xh_f" as const, connectorRef: ref });
    const names = ["VBUS", "D-", "D+", "GND"];
    const d: BlockDraft = {
      rows: {
        left: names.map((n, i) => row(i + 1, n, "X44B")),
        right: names.map((n, i) => row(i + 5, n, "X44A")),
        top: [],
        bottom: [],
      },
      size: { w: 20, h: 20 },
      pitch: "coarse",
      numbers: "outside",
    };
    // プレビューの箱の幅: 右のピンの先 (外形の右端+リード 5 mm) から求める
    const def = draftSymbol(d)!;
    const previewW = 2 * (Math.max(...def.pins.map((p) => p.at.x)) - 5);
    expect(previewW).toBeGreaterThan(20);
    expect(draftSize(d)).toEqual({ w: previewW, h: 20 });
  });

  // ja: プレビューは実際に描かれるシンボル定義そのもの(ピンの数だけ接続点がある)
  it("previews the very symbol that will be drawn, with one connection point per pin", () => {
    const def = draftSymbol(draft());
    expect(def?.pins.map((p) => p.number)).toEqual(["IN1", "IN2", "OUT1"]);
  });
});

describe("checks on the pin table", () => {
  // ja: 正しく埋めたピン表には指摘が出ない
  it("reports nothing for a table that is filled in correctly", () => {
    expect(draftIssues(draft())).toEqual([]);
  });

  // ja: 同じ辺に同じピン番号が2つあると指摘される
  it("reports a pin number that is used twice on one side", () => {
    const d = draft();
    d.rows.left[1].no = "IN1";
    expect(draftIssues(d)).toEqual([{ code: "duplicate", no: "IN1" }]);
  });

  // ja: 向かい合う辺に1回ずつ (名前が同じ) なら貫通ピンとして指摘されない
  it("accepts one pin number on each of two opposite sides with the same name", () => {
    const d = draft();
    d.rows.right[0] = { no: "IN1", name: "入力1" };
    expect(draftIssues(d)).toEqual([]);
    const tb = draftFromAttrs({ block_pins: '{"top":[{"no":"1"}],"bottom":[{"no":"1"}]}' });
    expect(draftIssues(tb)).toEqual([]);
  });

  // ja: 隣り合う辺に同じ番号を置いたとき、貫通ピンの組が3本になったときは重複として指摘される
  it("reports a pin number on neighbouring sides or three times as a duplicate", () => {
    const adjacent = draft();
    adjacent.rows.top = [{ no: "IN1", name: "入力1" }];
    expect(draftIssues(adjacent)).toEqual([{ code: "duplicate", no: "IN1" }]);
    const three = draft();
    three.rows.right[0] = { no: "IN1", name: "入力1" };
    three.rows.bottom = [{ no: "IN1", name: "入力1" }];
    expect(draftIssues(three)).toEqual([{ code: "duplicate", no: "IN1" }]);
  });

  // ja: 貫通ピンの両側で名前が違うと指摘される
  it("reports a feed-through pin whose two sides have different names", () => {
    const d = draft();
    d.rows.right[0] = { no: "IN1", name: " 別名 " };
    expect(draftIssues(d)).toEqual([{ code: "feedthroughName", no: "IN1" }]);
  });

  // ja: 貫通ピンに接点やコネクタの絵を付けると指摘される
  it("reports contacts or a connector picture on a feed-through pin", () => {
    const withContacts = draft();
    withContacts.rows.right[0] = { no: "IN1", name: "入力1" };
    withContacts.rows.left[0].contacts = "A, B";
    expect(draftIssues(withContacts)).toEqual([{ code: "feedthroughPicture", no: "IN1" }]);
    const withConnector = draft();
    withConnector.rows.right[0] = { no: "IN1", name: "入力1", connector: "usb_a_m" };
    expect(draftIssues(withConnector)).toEqual([{ code: "feedthroughPicture", no: "IN1" }]);
  });

  // ja: ピン名だけ書いて番号が空の行は指摘される
  it("reports a row that has a name but no pin number", () => {
    const d = draft();
    d.rows.left[0].no = " ";
    expect(draftIssues(d)).toEqual([{ code: "emptyNo", side: "left", index: 0 }]);
  });

  // ja: 手動の外形がピンの並びより小さいと指摘される(必要な寸法つき)
  it("reports a hand-set outline that is too small for its pins", () => {
    const d = draft();
    d.size = { w: 10, h: 5 };
    expect(draftIssues(d)).toEqual([{ code: "sizeTooSmall", w: 5, h: 10 }]);
  });

  // ja: ピンが多すぎる(200本超)ブロックは指摘される
  it("reports a block with more than 200 pins", () => {
    const d = blankDraft();
    d.rows.left = Array.from({ length: 201 }, (_, i) => ({ no: `L${i}`, name: "" }));
    d.rows.right = [];
    expect(draftIssues(d)).toEqual([{ code: "tooMany", count: 201, max: 200 }]);
  });
});

describe("pins that are already wired", () => {
  const attrs = attrsFromDraft(draft());

  // ja: 配線が接続点に届いているピンは「配線済み」として拾われる
  it("lists the pins a wire actually reaches", () => {
    const inst = blockAt(attrs);
    const def = draftSymbol(draft())!;
    const pin = def.pins.find((p) => p.number === "IN1")!;
    const at = { x: inst.at.x + pin.at.x, y: inst.at.y + pin.at.y };
    const sheet = sheetWith([wire("w1", [{ x: at.x - 20, y: at.y }, at])]);
    expect(wiredPinNumbers(sheet, inst, def)).toEqual([{ side: "left", no: "IN1" }]);
  });

  // ja: 回転して置いたブロックでも、回転後の接続点で配線済みかを見る
  it("follows the rotation of the placed block when looking for wires", () => {
    const inst = blockAt(attrs, 90);
    const def = draftSymbol(draft())!;
    const pin = def.pins.find((p) => p.number === "OUT1")!;
    // 90度回転: (x, y) → (-y, x)
    const at = { x: inst.at.x - pin.at.y, y: inst.at.y + pin.at.x };
    const sheet = sheetWith([wire("w1", [at, { x: at.x, y: at.y + 20 }])]);
    expect(wiredPinNumbers(sheet, inst, def)).toEqual([{ side: "right", no: "OUT1" }]);
  });

  // ja: 配線が1本も届いていないブロックには配線済みピンが無い
  it("finds no wired pins on a block that nothing is connected to", () => {
    const def = draftSymbol(draft())!;
    const sheet = sheetWith([wire("w1", [{ x: 0, y: 0 }, { x: 20, y: 0 }])]);
    expect(wiredPinNumbers(sheet, blockAt(attrs), def)).toEqual([]);
  });

  // ja: 配線済みのピンを消す・番号を変えると、そのピンが「接続が外れる」として挙がる
  it("flags a wired pin that the edit deletes or renumbers", () => {
    const d = draft();
    d.rows.left[0].no = "A1";
    removeRow(d, "right", 0);
    expect(droppedWiredPins([{ side: "left", no: "IN1" }, { side: "left", no: "IN2" }, { side: "right", no: "OUT1" }], d)).toEqual(["IN1", "OUT1"]);
  });

  // ja: 配線済みのピンをそのまま残す編集では警告は出ない
  it("stays quiet when the wired pins survive the edit", () => {
    const d = draft();
    d.rows.left[0].name = "運転指令";
    expect(droppedWiredPins([{ side: "left", no: "IN1" }, { side: "right", no: "OUT1" }], d)).toEqual([]);
  });

  // ja: 貫通ピンは配線のある側ごとに辺つきで挙がり、両側に配線があって片側の行を消すと接続が外れるピンとして1回だけ警告される
  it("lists each wired side of a feed-through pin and warns once when one side row is deleted", () => {
    const fa = { block_pins: '{"left":[{"no":"1"}],"right":[{"no":"1"}]}' };
    const inst = blockAt(fa);
    const def = blockSymbol(fa);
    const wires = def.pins.map((pin, i) => {
      const at = { x: inst.at.x + pin.at.x, y: inst.at.y + pin.at.y };
      return wire(`w${i}`, [{ x: at.x - 20, y: at.y }, at]);
    });
    const both = wiredPinNumbers(sheetWith(wires), inst, def);
    expect(both).toEqual([{ side: "left", no: "1" }, { side: "right", no: "1" }]);
    expect(droppedWiredPins(both, draftFromAttrs(fa))).toEqual([]);
    const oneSide = draftFromAttrs(fa);
    removeRow(oneSide, "right", 0);
    expect(droppedWiredPins(both, oneSide)).toEqual(["1"]);
  });

  // ja: 貫通ピンの片側だけに配線があるとき、その側の行を消す・番号を変えると警告され、配線の無い側の行を消しても警告されない
  it("warns when the wired side of a feed-through pin is deleted or renumbered, but not the unwired side", () => {
    const fa = { block_pins: '{"left":[{"no":"1"}],"right":[{"no":"1"}]}' };
    const rightWired = [{ side: "right" as const, no: "1" }];
    const deleted = draftFromAttrs(fa);
    removeRow(deleted, "right", 0);
    expect(droppedWiredPins(rightWired, deleted)).toEqual(["1"]);
    const renumbered = draftFromAttrs(fa);
    renumbered.rows.right[0].no = "3";
    expect(droppedWiredPins(rightWired, renumbered)).toEqual(["1"]);
    const otherDeleted = draftFromAttrs(fa);
    removeRow(otherDeleted, "left", 0);
    expect(droppedWiredPins(rightWired, otherDeleted)).toEqual([]);
    expect(droppedWiredPins(rightWired, draftFromAttrs(fa))).toEqual([]);
  });

  // ja: 接点はカンマ (半角・読点・全角) で区切り、前後の空白を落として空のものを捨てる
  it("splits contacts on ASCII, Japanese and full-width commas and trims them", () => {
    expect(splitContacts(" VBUS 、D-，D+, GND ,")).toEqual(["VBUS", "D-", "D+", "GND"]);
    expect(splitContacts("")).toEqual([]);
  });

  // ja: 接点を書いた行は block_pins に contacts として入り、読み戻すとカンマ区切りの文字に戻る。接点の無い行の書き出しは今までと同じ
  it("writes contacts into block_pins and reads them back as comma-separated text", () => {
    const d = draft();
    d.rows.right[0].contacts = "VBUS, D-, D+, GND";
    const attrs = attrsFromDraft(d);
    const pins = JSON.parse(attrs.block_pins);
    expect(pins.right[0]).toEqual({ no: "OUT1", name: "出力1", contacts: ["VBUS", "D-", "D+", "GND"] });
    expect(pins.left[0]).toEqual({ no: "IN1", name: "入力1" });
    expect(draftFromAttrs(attrs).rows.right[0].contacts).toBe("VBUS, D-, D+, GND");
    expect(draftFromAttrs(attrs).rows.left[0]).toEqual({ no: "IN1", name: "入力1" });
    expect(portPinCount(draftSymbol(d)!)).toBe(1);
  });

  // ja: 接点が1つだけ・同じ接点が2つ・32を超える行は指摘になる
  it("flags a single contact, a repeated contact and too many contacts", () => {
    const d = draft();
    d.rows.left[0].contacts = "VBUS";
    d.rows.left[1].contacts = "A, A";
    d.rows.right[0].contacts = Array.from({ length: 33 }, (_, i) => `C${i}`).join(",");
    expect(draftIssues(d)).toEqual([
      { code: "contactsTooFew", side: "left", index: 0 },
      { code: "duplicateContact", side: "left", index: 1, contact: "A" },
      { code: "tooManyContacts", side: "right", index: 0, count: 33, max: 32 },
    ]);
  });
});

describe("connector columns", () => {
  // ja: コネクタとまとめ名を書いた行は block_pins に connector / connector_ref として入り、読み戻すと同じ属性に戻る
  it("writes the connector and group name into block_pins and reads them back unchanged", () => {
    const d = draft();
    d.rows.left[0].connector = "screw";
    d.rows.left[0].connectorRef = " TB1 ";
    d.rows.left[1].connector = "screw";
    d.rows.left[1].connectorRef = "TB1";
    d.rows.right[0].connector = "usb_a_f";
    const attrs = attrsFromDraft(d);
    const pins = JSON.parse(attrs.block_pins);
    expect(pins.left[0]).toEqual({ no: "IN1", name: "入力1", connector: "screw", connector_ref: "TB1" });
    expect(pins.right[0]).toEqual({ no: "OUT1", name: "出力1", connector: "usb_a_f" });
    expect(draftFromAttrs(attrs).rows.left[0]).toEqual({ no: "IN1", name: "入力1", connector: "screw", connectorRef: "TB1" });
    expect(attrsFromDraft(draftFromAttrs(attrs))).toEqual(attrs);
  });

  // ja: コネクタだけを選んだ行も書き出す行として数える (番号が空なら指摘が出る)
  it("counts a row with only a connector as filled, so a missing number is reported", () => {
    const d = draft();
    d.rows.right.push({ no: "", name: "", connector: "dc_f" });
    expect(draftIssues(d)).toEqual([{ code: "emptyNo", side: "right", index: 1 }]);
  });

  // ja: 端子台・JST 以外のまとめ名と、2つのコネクタに分かれたまとめ名は指摘になる
  it("flags a group name on a kind that cannot have one and a group name split over two connectors", () => {
    const d = draft();
    d.rows.left[0].connector = "usb_a_f";
    d.rows.left[0].connectorRef = "J3";
    d.rows.left[1].connector = "jst_ph_f";
    d.rows.left[1].connectorRef = "J5";
    d.rows.right[0].connector = "jst_ph_f";
    d.rows.right[0].connectorRef = "J5";
    expect(draftIssues(d)).toEqual([
      { code: "connectorRefNotGrouped", side: "left", index: 0 },
      { code: "connectorRefSplit", ref: "J5" },
    ]);
  });

  // ja: 空白だけのまとめ名は書き出さず、指摘にもならない
  it("drops a group name made only of spaces without reporting it", () => {
    const d = draft();
    d.rows.left[0].connector = "jst_ph_f";
    d.rows.left[0].connectorRef = "   ";
    expect(JSON.parse(attrsFromDraft(d).block_pins).left[0]).toEqual({ no: "IN1", name: "入力1", connector: "jst_ph_f" });
    expect(draftIssues(d)).toEqual([]);
    expect(draftSymbol(d)).not.toBeNull();
  });
});

describe("block pin pitch and number placement in the editor", () => {
  // ja: 2.5mm間隔・番号内側のブロックを編集して書き戻しても、2つの属性は残る
  it("keeps block_pitch and block_numbers through an edit", () => {
    const attrs = { block_pins: '{"left":[{"no":"1"}]}', block_pitch: "2.5", block_numbers: "inside", block_size: '{"w":5,"h":5}' };
    const out = attrsFromDraft(draftFromAttrs(attrs));
    expect(out.block_pitch).toBe("2.5");
    expect(out.block_numbers).toBe("inside");
  });

  // ja: 属性の無いブロックを編集して書き戻しても、2つの属性は増えない
  it("does not add the attributes to a block that has none", () => {
    const out = attrsFromDraft(draftFromAttrs({ block_pins: '{"left":[{"no":"1"}]}' }));
    expect("block_pitch" in out).toBe(false);
    expect("block_numbers" in out).toBe(false);
  });

  // ja: 2.5mm間隔のブロックでピンが収まらないとき、必要な大きさは2.5mm間隔の規則で出る (左に3本なら高さ10mm)
  it("reports the needed size of a fine block with the fine rule", () => {
    const d = draftFromAttrs({ block_pins: '{"left":[{"no":"1"},{"no":"2"},{"no":"3"}]}', block_pitch: "2.5" });
    d.size = { w: 5, h: 7.5 };
    expect(draftIssues(d)).toEqual([{ code: "sizeTooSmall", w: 5, h: 10 }]);
  });

  // ja: 2.5mm間隔の下書きのプレビューは、配置後に描かれるシンボルと同じ (左2本・大きさ自動)
  it("previews a fine draft exactly as the placed block is drawn", () => {
    const d = draftFromAttrs({ block_pins: '{"left":[{"no":"1"},{"no":"2"}]}', block_pitch: "2.5", block_numbers: "inside" });
    expect(draftSymbol(d)).toEqual(blockSymbol(attrsFromDraft(d)));
    expect(draftSize(d).h).toBe(7.5);
  });
});

describe("connector pictures on a fine block and size snapping", () => {
  const fineRows = '{"left":[{"no":"1"},{"no":"2"}]}';

  // ja: 2.5mm間隔のブロックでピンにコネクタの絵を付けると指摘される (間隔が5mmのブロックでは指摘されない)
  it("reports a connector picture on a fine block but not on a coarse one", () => {
    const fine = draftFromAttrs({ block_pins: fineRows, block_pitch: "2.5" });
    fine.rows.left[1].connector = "jst_xh_f";
    expect(draftIssues(fine)).toContainEqual({ code: "connectorOnFine", no: "2" });
    const coarse = draftFromAttrs({ block_pins: fineRows });
    coarse.rows.left[1].connector = "jst_xh_f";
    expect(draftIssues(coarse).some((i) => i.code === "connectorOnFine")).toBe(false);
  });

  // ja: 番号を内側に描くブロックでピンにコネクタの絵を付けると、5mm間隔でも指摘される (番号が外側のブロックでは指摘されない)
  it("reports a connector picture on a block with numbers inside but not with numbers outside", () => {
    const inside = draftFromAttrs({ block_pins: fineRows, block_numbers: "inside" });
    inside.rows.left[1].connector = "jst_xh_f";
    expect(draftIssues(inside)).toContainEqual({ code: "connectorOnInside", no: "2" });
    const outside = draftFromAttrs({ block_pins: fineRows });
    outside.rows.left[1].connector = "jst_xh_f";
    expect(draftIssues(outside).some((i) => i.code === "connectorOnInside")).toBe(false);
  });

  // ja: 2.5mm間隔のブロックの大きさは2.5mm刻みに丸められ、5mm未満にはならない
  it("snaps a fine block size to 2.5 mm steps, never below 5 mm", () => {
    expect(snapSize(7, "fine")).toBe(7.5);
    expect(snapSize(1, "fine")).toBe(5);
    expect(snapSize(6.2, "fine")).toBe(5);
  });

  // ja: 5mm間隔のブロックの大きさは5mm刻みに丸められ、5mm未満にはならない
  it("snaps a coarse block size to 5 mm steps, never below 5 mm", () => {
    expect(snapSize(12, "coarse")).toBe(10);
    expect(snapSize(1, "coarse")).toBe(5);
  });
});
