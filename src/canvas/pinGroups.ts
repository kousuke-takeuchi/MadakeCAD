// ピングループ括弧 (G8)。Rust側 madake-core/src/pin_groups.rs と同一ジオメトリの双子。
//
// SymbolInstance.attrs["pin_groups"] = カンマ区切りの「範囲:名前」(例 "1-4:電源,5-8:信号")
// を解析し、範囲の最初から最後のピンにかけて角括弧を描く (縦書きの近似として、名前は
// 4文字ごとに改行して積む)。ピンの向き (PinDir) から括弧の向きを決めるので、回転・
// ミラーにも自動で追従する。読めない・重なる項目はその項目だけを飛ばす (パニックしない)。

import type { Point, PinDef, PinDir, SymbolDef } from "../ipc";

/** `attrs`のキー。Rust側 PIN_GROUPS_ATTR と同じ。 */
export const PIN_GROUPS_ATTR = "pin_groups";

/** 「範囲:名前」の書式が読めない。 */
export const PIN_GROUP_INVALID = "pin_group.invalid";
/** 範囲の両端がそのインスタンスのピン番号に無い。 */
export const PIN_GROUP_UNKNOWN_PIN = "pin_group.unknown_pin";
/** 範囲が他のグループと重なっている。 */
export const PIN_GROUP_OVERLAP = "pin_group.overlap";

/** キャプションの折り返し文字数 (縦書きの近似)。 */
export const PIN_GROUP_CAPTION_WRAP = 4;
/** 括弧の縦棒・横棒とピンの外側端との間隔 (mm)。 */
export const PIN_GROUP_BRACKET_GAP_MM = 1.5;
/** 括弧の両端のヒゲの長さ (mm)。 */
export const PIN_GROUP_BRACKET_TICK_MM = 1.2;
/** 括弧の棒とキャプションの間隔 (mm)。 */
export const PIN_GROUP_CAPTION_GAP_MM = 1.5;
/** キャプション文字高さ (mm)。 */
export const PIN_GROUP_CAPTION_TEXT_MM = 2.0;
/** キャプションを複数行に積むときの行送り (mm)。 */
export const PIN_GROUP_CAPTION_LINE_PITCH_MM = 2.5;

export interface PinGroup {
  rangeText: string;
  caption: string;
  first: string;
  last: string;
}

export interface PinGroupIssue {
  text: string;
  code: string;
  message: string;
}

export interface ParsedPinGroups {
  groups: PinGroup[];
  issues: PinGroupIssue[];
}

/**
 * シンボル定義のピン番号をチャート順 (定義に並んだ順、初出のみ) に列挙する。
 * 端子台の貫通端子は最初に出てくる側だけを残す (Rust側 chart_order_pin_defs と同一)。
 */
export function chartOrderPinDefs(def: SymbolDef): PinDef[] {
  const seen = new Set<string>();
  const out: PinDef[] = [];
  for (const p of def.pins) {
    if (!seen.has(p.number)) {
      seen.add(p.number);
      out.push(p);
    }
  }
  return out;
}

/** chartOrderPinDefsのピン番号だけの列。 */
export function chartOrderPinNumbers(def: SymbolDef): string[] {
  return chartOrderPinDefs(def).map((p) => p.number);
}

/** キャプションをPIN_GROUP_CAPTION_WRAP文字ごとに折り返す (縦書きの近似)。 */
export function wrapCaption(caption: string): string[] {
  if (caption.length === 0) return [""];
  const chars = Array.from(caption);
  const out: string[] = [];
  for (let i = 0; i < chars.length; i += PIN_GROUP_CAPTION_WRAP) {
    out.push(chars.slice(i, i + PIN_GROUP_CAPTION_WRAP).join(""));
  }
  return out;
}

/**
 * attrs["pin_groups"]を解析する (Rust側 parse_pin_groups と同一)。orderはそのインスタンスの
 * チャート順ピン番号 (chartOrderPinNumbers)。
 */
export function parsePinGroups(spec: string, order: readonly string[]): ParsedPinGroups {
  const candidates: { group: PinGroup; i0: number; i1: number }[] = [];
  const issues: PinGroupIssue[] = [];

  for (const raw of spec.split(",")) {
    const text = raw.trim();
    if (text === "") continue;
    const colon = text.indexOf(":");
    if (colon < 0) {
      issues.push({ text, code: PIN_GROUP_INVALID, message: "「範囲:名前」の形式で書いてください" });
      continue;
    }
    const range = text.slice(0, colon).trim();
    const caption = text.slice(colon + 1).trim();
    const dash = range.indexOf("-");
    const [first, last] = dash >= 0
      ? [range.slice(0, dash).trim(), range.slice(dash + 1).trim()]
      : [range, range];
    if (first === "" || last === "") {
      issues.push({
        text,
        code: PIN_GROUP_INVALID,
        message: "範囲の両端を書いてください (例 \"1-4\" または単ピンなら \"3\")",
      });
      continue;
    }
    const i0 = order.indexOf(first);
    const i1 = order.indexOf(last);
    if (i0 < 0 || i1 < 0) {
      const unknown = [first, last].filter((p) => !order.includes(p));
      issues.push({
        text,
        code: PIN_GROUP_UNKNOWN_PIN,
        message: `存在しないピン ${unknown.join(", ")} を指しています`,
      });
      continue;
    }
    if (i0 > i1) {
      issues.push({ text, code: PIN_GROUP_INVALID, message: "範囲の始点は終点よりチャート順で前にしてください" });
      continue;
    }
    candidates.push({ group: { rangeText: range, caption, first, last }, i0, i1 });
  }

  const overlaps = (a: { i0: number; i1: number }, b: { i0: number; i1: number }) => a.i0 <= b.i1 && b.i0 <= a.i1;
  const bad = new Set<number>();
  for (let i = 0; i < candidates.length; i++) {
    for (let j = 0; j < candidates.length; j++) {
      if (i !== j && overlaps(candidates[i], candidates[j])) bad.add(i);
    }
  }
  for (const i of bad) {
    const c = candidates[i];
    issues.push({
      text: `${c.group.rangeText}:${c.group.caption}`,
      code: PIN_GROUP_OVERLAP,
      message: "他のピングループと範囲が重なっています",
    });
  }
  const groups = candidates.filter((_, i) => !bad.has(i)).map((c) => c.group);
  return { groups, issues };
}

export interface PinGroupBracket {
  /** 角括弧のポリライン (4点、ローカル座標)。 */
  bracket: Point[];
  /** キャプションの各行と、その中心位置 (ローカル座標)。 */
  captionLines: [string, Point][];
}

/**
 * 1つのピングループの括弧とキャプションのジオメトリを求める (Rust側 pin_group_bracket と
 * 同一)。pinsはグループに属するピン定義 (チャート順)。向きが揃っていなければnull。
 */
export function pinGroupBracket(pins: readonly PinDef[], caption: string): PinGroupBracket | null {
  if (pins.length === 0) return null;
  const firstDir: PinDir = pins[0].dir ?? "right";
  if (pins.some((p) => (p.dir ?? "right") !== firstDir)) return null;
  const lines = wrapCaption(caption);

  if (firstDir === "left" || firstDir === "right") {
    const top = Math.min(...pins.map((p) => p.at.y));
    const bottom = Math.max(...pins.map((p) => p.at.y));
    const x = pins[0].at.x;
    const sign = firstDir === "left" ? -1 : 1;
    const barX = x + sign * PIN_GROUP_BRACKET_GAP_MM;
    const tickX = barX - sign * PIN_GROUP_BRACKET_TICK_MM;
    const bracket: Point[] = [
      { x: tickX, y: top },
      { x: barX, y: top },
      { x: barX, y: bottom },
      { x: tickX, y: bottom },
    ];
    const captionX = barX + sign * PIN_GROUP_CAPTION_GAP_MM;
    const midY = (top + bottom) / 2;
    const total = Math.max(0, lines.length - 1) * PIN_GROUP_CAPTION_LINE_PITCH_MM;
    const startY = midY - total / 2;
    const captionLines: [string, Point][] = lines.map((l, i) => [
      l,
      { x: captionX, y: startY + i * PIN_GROUP_CAPTION_LINE_PITCH_MM },
    ]);
    return { bracket, captionLines };
  }

  const left = Math.min(...pins.map((p) => p.at.x));
  const right = Math.max(...pins.map((p) => p.at.x));
  const y = pins[0].at.y;
  const sign = firstDir === "up" ? -1 : 1;
  const barY = y + sign * PIN_GROUP_BRACKET_GAP_MM;
  const tickY = barY - sign * PIN_GROUP_BRACKET_TICK_MM;
  const bracket: Point[] = [
    { x: left, y: tickY },
    { x: left, y: barY },
    { x: right, y: barY },
    { x: right, y: tickY },
  ];
  const captionY0 = barY + sign * PIN_GROUP_CAPTION_GAP_MM;
  const midX = (left + right) / 2;
  const captionLines: [string, Point][] = lines.map((l, i) => [
    l,
    { x: midX, y: captionY0 + sign * i * PIN_GROUP_CAPTION_LINE_PITCH_MM },
  ]);
  return { bracket, captionLines };
}

/** あるシンボル定義+attrs["pin_groups"]から、描ける括弧を全部求める (Rust側 pin_group_brackets と同一)。 */
export function pinGroupBrackets(def: SymbolDef, spec: string): PinGroupBracket[] {
  if (spec.trim() === "") return [];
  const order = chartOrderPinNumbers(def);
  const defs = chartOrderPinDefs(def);
  const parsed = parsePinGroups(spec, order);
  const out: PinGroupBracket[] = [];
  for (const g of parsed.groups) {
    const i0 = order.indexOf(g.first);
    const i1 = order.indexOf(g.last);
    if (i0 < 0 || i1 < 0) continue;
    const bracket = pinGroupBracket(defs.slice(i0, i1 + 1), g.caption);
    if (bracket) out.push(bracket);
  }
  return out;
}
