// 配線注記 (G2) の文言と配置。Rust側 wire_note.rs と同じ結果になることを確かめる。
import { describe, expect, it } from "vitest";

import type { Entity, Sheet, Wire } from "../ipc";
import { WIRE_NOTE_GAP, colorJa, wireNoteFont, wireNoteLabels, wireNoteText } from "./wireNotes";

function wire(
  id: string,
  points: [number, number][],
  color: string,
  sq: number,
  length: number | null,
): Entity {
  return {
    kind: "wire",
    id,
    points: points.map(([x, y]) => ({ x, y })),
    color,
    sq,
    length_m: length,
    part_no: null,
    net: null,
  } as Entity;
}

function sheetOf(wires: Entity[]): Sheet {
  return {
    entities: Object.fromEntries(wires.map((w) => [w.id, w])),
  } as unknown as Sheet;
}

const asWire = (e: Entity): Wire => e as unknown as Wire;

describe("wireNoteText", () => {
  // ja: 線径・線色・電線長がそろったワイヤには「0.75sq 白 0.4m」という注記が付く
  it("annotates a wire with its gauge, colour and cut length", () => {
    expect(wireNoteText(asWire(wire("w", [[0, 0], [10, 0]], "white", 0.75, 0.4)))).toBe(
      "0.75sq 白 0.4m",
    );
  });

  // ja: 欠けている属性は注記から省かれ、長さの無いワイヤは「0.75sq 白」と書かれる
  it("leaves out attributes the wire does not have", () => {
    expect(wireNoteText(asWire(wire("w", [[0, 0], [10, 0]], "white", 0.75, null)))).toBe("0.75sq 白");
    expect(wireNoteText(asWire(wire("w", [[0, 0], [10, 0]], "", 0, 0.4)))).toBe("0.4m");
  });

  // ja: 線径も色も長さも無いワイヤには、空の注記ではなく注記そのものが付かない
  it("gives a wire with no attributes no note at all", () => {
    expect(wireNoteText(asWire(wire("w", [[0, 0], [10, 0]], "", 0, null)))).toBe("");
  });

  // ja: 線径も長さも無い黒の配線には注記を付けない (黒は既定の色)。線径か長さのある黒の配線は色名も書く (wire_note.rsと同ルール)
  it("gives a plain black wire no note", () => {
    const note = (color: string, sq: number) => wireNoteText(asWire(wire("w", [[0, 0], [10, 0]], color, sq, null)));
    for (const color of ["black", "Black", "黒", " black "]) expect(note(color, 0)).toBe("");
    expect(note("black", 0.75)).toBe("0.75sq 黒");
    expect(note("red", 0)).toBe("赤");
  });

  // ja: 色名は英字の大小と前後の空白を無視して照合される。知っている色は表示用の名前で、知らない色は入力のまま (ただし小文字にそろえて) 書かれる
  it("matches colour names ignoring capitals and surrounding space", () => {
    const note = (color: string) => wireNoteText(asWire(wire("w", [[0, 0], [10, 0]], color, 0.3, null)));
    expect(note("白")).toBe("0.3sq 白");
    expect(note("Red")).toBe("0.3sq 赤");
    expect(note("  red  ")).toBe("0.3sq 赤");
    expect(note("Turquoise")).toBe("0.3sq turquoise");
    expect(note("#FF0000")).toBe("0.3sq #ff0000");
    // 変換表そのものは入力をそのまま引く (そろえるのは colorDisplay の役目)
    expect(colorJa("turquoise")).toBe("turquoise");
    expect(colorJa("light_blue")).toBe("水色");
  });
});

describe("wireNoteLabels", () => {
  // ja: 横向きのワイヤの注記は配線の下に置かれ、上に出る線番とは反対側になる
  it("places the note of a horizontal wire below the wire, opposite its wire number", () => {
    const labels = wireNoteLabels(sheetOf([wire("w1", [[0, 20], [40, 20]], "red", 0.5, 1.2)]));
    expect(labels).toHaveLength(1);
    expect(labels[0].text).toBe("0.5sq 赤 1.2m");
    expect(labels[0].at).toEqual({ x: 20, y: 20 + WIRE_NOTE_GAP });
    expect(labels[0].align).toBe("center");
  });

  // ja: 縦向きのワイヤの注記は配線の右に置かれ、左に出る線番とは反対側になる
  it("places the note of a vertical wire to the right of the wire, opposite its wire number", () => {
    const labels = wireNoteLabels(sheetOf([wire("w1", [[30, 0], [30, 40]], "black", 2, null)]));
    expect(labels[0].at).toEqual({ x: 30 + WIRE_NOTE_GAP, y: 20 });
    expect(labels[0].align).toBe("left");
  });

  // ja: 注記はワイヤ1本ごとに付く(ネットごとに1回だけ描かれる線番とは違う)
  it("annotates every wire, unlike the wire number which is drawn once per net", () => {
    const labels = wireNoteLabels(
      sheetOf([
        wire("w1", [[0, 10], [20, 10]], "red", 0.5, 0.3),
        wire("w2", [[20, 10], [20, 40]], "red", 0.5, 0.9),
      ]),
    );
    expect(labels.map((l) => l.text)).toEqual(["0.5sq 赤 0.3m", "0.5sq 赤 0.9m"]);
  });

  // ja: 折れ曲がったワイヤの注記は、いちばん長い直線部分に置かれて読みやすくなる
  it("puts the note on the longest straight run of a bent wire", () => {
    const labels = wireNoteLabels(
      sheetOf([wire("w1", [[0, 10], [5, 10], [5, 90]], "blue", 0.3, null)]),
    );
    expect(labels[0].at.y).toBe(50);
    expect(labels[0].align).toBe("left");
  });

  // ja: 属性の無いワイヤは注記の一覧に現れない
  it("skips wires that carry no attributes", () => {
    expect(wireNoteLabels(sheetOf([wire("w1", [[0, 0], [10, 0]], "", 0, null)]))).toEqual([]);
  });
});

describe("annotation text height", () => {
  // ja: 注記文字高を設定していないシートは、これまでどおり2.0mmの注記で描かれる
  it("keeps the default 2.0mm note height on a sheet with no setting", () => {
    const s = sheetOf([wire("w", [[0, 20], [40, 20]], "red", 0.5, null)]);
    expect(wireNoteFont(s)).toBeCloseTo(2.0, 9);
    expect(wireNoteLabels(s)[0].at.y).toBeCloseTo(20 + WIRE_NOTE_GAP, 9);
  });

  // ja: シートの注記文字高を1.4mmにすると、配線注記はちょうどその高さで描かれ配線にも寄る
  it("draws the wire notes at the sheet's annotation text height and moves them closer", () => {
    const s = { ...sheetOf([wire("w", [[0, 20], [40, 20]], "red", 0.5, null)]), annotation_text_mm: 1.4 };
    expect(wireNoteFont(s)).toBeCloseTo(1.4, 9);
    expect(wireNoteLabels(s)[0].at.y).toBeCloseTo(20 + WIRE_NOTE_GAP * 0.7, 9);
  });
});
