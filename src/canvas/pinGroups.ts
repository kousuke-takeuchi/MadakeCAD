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
export const PIN_GROUP_INVALID = "erc.pin_group_invalid";
/** 範囲の両端がそのインスタンスのピン番号に無い。 */
export const PIN_GROUP_UNKNOWN_PIN = "erc.pin_group_unknown_pin";
/** 範囲が他のグループと重なっている。 */
export const PIN_GROUP_OVERLAP = "erc.pin_group_overlap";
/** ピングループ括弧を描けない表示形式のシンボルに指定されている (実機ブロック表示のコイルなど)。 */
export const PIN_GROUP_UNSUPPORTED = "erc.pin_group_unsupported";

/** キャプションの折り返し文字数 (縦書きの近似)。 */
export const PIN_GROUP_CAPTION_WRAP = 4;
/** 括弧の縦棒・横棒とピンの外側端との間隔 (mm)。 */
export const PIN_GROUP_BRACKET_GAP_MM = 1.5;
/** 括弧の両端のヒゲの長さ (mm)。 */
export const PIN_GROUP_BRACKET_TICK_MM = 1.2;
/**
 * 括弧の棒とキャプションの**文字の縁**との間隔 (mm)。キャプションは中央揃えで描くので、
 * 基準点は文字の幅・高さの半分だけさらに外へ置く (Rust側と同一)。
 */
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

/** chartOrderPinDefsのピン番号だけの列 (**生のピン番号**)。 */
export function chartOrderPinNumbers(def: SymbolDef): string[] {
  return chartOrderPinDefs(def).map((p) => p.number);
}

/**
 * attrs["pin_groups"]を突き合わせる相手 = チャート順の**表示ラベル** (Rust側 display_order と同一)。
 *
 * 端子台の端子番号の上書きがあると、ユーザーが書いた範囲は表示ラベル (`"11-14:電源"`) なのに、
 * 括弧のジオメトリはシンボル定義の生のピン番号でしか引けない。`rename` (生のピン番号→表示ラベル。
 * terminalJumpers.tsの`terminalLabels(def, attrs).renameMap`) を渡すと突き合わせ用の列ができ、
 * インデックスは生の列と1対1のまま保たれる。
 *
 * `rename`を引数で受け取るのは、pinGroups.tsが座標変換もterminalJumpers.tsも参照しない葉モジュール
 * のままでいられるようにするため (循環importを起こさない)。
 */
function displayOrder(
  rawOrder: readonly string[],
  rename: ReadonlyMap<string, string> | null | undefined,
): string[] {
  if (!rename) return [...rawOrder];
  return rawOrder.map((n) => rename.get(n) ?? n);
}

/** 全ピンが広がる範囲の中心 (シンボルのローカル座標)。ピンが無ければnull。 */
function pinCenter(def: SymbolDef): Point | null {
  if (def.pins.length === 0) return null;
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  for (const pin of def.pins) {
    minX = Math.min(minX, pin.at.x);
    minY = Math.min(minY, pin.at.y);
    maxX = Math.max(maxX, pin.at.x);
    maxY = Math.max(maxY, pin.at.y);
  }
  return { x: (minX + maxX) / 2, y: (minY + maxY) / 2 };
}

/**
 * ある番号のピン定義のうち、ピンの広がりの中心からいちばん遠いものを選ぶ
 * (Rust側 outer_pin_def と同一)。多段端子台で右の列の括弧を列の外側へ出すための規則。
 */
export function outerPinDef(def: SymbolDef, number: string): PinDef | null {
  const center = pinCenter(def);
  if (!center) return null;
  let best: PinDef | null = null;
  let bestD = -Infinity;
  for (const pin of def.pins) {
    if (pin.number !== number) continue;
    const d = Math.hypot(pin.at.x - center.x, pin.at.y - center.y);
    if (d > bestD) {
      bestD = d;
      best = pin;
    }
  }
  return best;
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
    // ピン番号そのものに「-」が入っている端子は範囲の区切りと見分けが付かない
    // (Rust側 parse_pin_groups と同一)
    if (range.includes("-") && order.includes(range)) {
      issues.push({
        text,
        code: PIN_GROUP_INVALID,
        message: "ピン番号に「-」を含む端子は範囲指定に使えません",
      });
      continue;
    }
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

/**
 * 描画用に解決したピングループ1件のジオメトリ (Rust側 PinGroupBracket と同一)。
 * キャプションの位置はここで決めない: 文字は回転しても横書きのままなので、行の積み上げは
 * インスタンスの変換を通したあとの用紙座標で行う (dynamicSymbol.ts の captionLineAnchors)。
 */
export interface PinGroupBracket {
  /** 角括弧のポリライン (4点、ローカル座標)。 */
  bracket: Point[];
  /** 折り返し済みのキャプション各行。 */
  caption: string[];
  /** 括弧の棒の中点 (ローカル座標)。キャプションはここから外向きに置く。 */
  captionFrom: Point;
  /** キャプションを置く外向き (ローカル座標の単位ベクトル)。 */
  captionDir: Point;
}

/**
 * 1つのピングループの括弧のジオメトリとキャプションの置き方を求める
 * (Rust側 pin_group_bracket と同一)。pinsはグループに属するピン定義 (チャート順)。
 * 向きが揃っていなければnull (段をまたぐ範囲・2辺にまたがる範囲)。
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
    return {
      bracket,
      caption: lines,
      captionFrom: { x: barX, y: (top + bottom) / 2 },
      captionDir: { x: sign, y: 0 },
    };
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
  return {
    bracket,
    caption: lines,
    captionFrom: { x: (left + right) / 2, y: barY },
    captionDir: { x: 0, y: sign },
  };
}

/**
 * あるシンボル定義+attrs["pin_groups"]から、描ける括弧を全部求める (Rust側 pin_group_brackets と同一)。
 * 範囲の順番はチャート順で決めるが、括弧を掛ける位置は番号ごとにouterPinDefで選び直す。
 *
 * `rename`は端子番号の上書きの対応表 (terminalJumpers.tsの`terminalLabels(def, attrs).renameMap`)。
 * 渡すと範囲は表示ラベルで突き合わせられるので、**採番しても括弧は描かれ続ける**。
 */
export function pinGroupBrackets(
  def: SymbolDef,
  spec: string,
  rename?: ReadonlyMap<string, string> | null,
): PinGroupBracket[] {
  if (spec.trim() === "") return [];
  const rawOrder = chartOrderPinNumbers(def);
  const order = displayOrder(rawOrder, rename);
  const parsed = parsePinGroups(spec, order);
  const out: PinGroupBracket[] = [];
  for (const g of parsed.groups) {
    const bracket = pinGroupBracket(groupPinDefs(def, order, rawOrder, g), g.caption);
    if (bracket) out.push(bracket);
  }
  return out;
}

/**
 * 1つのグループが括弧を掛けるピン定義 (Rust側 group_pin_defs と同一)。
 * 範囲の両端は表示ラベルなので`order`で位置を求め、ピン定義は同じインデックスの生のピン番号
 * (`rawOrder`) で引く。
 */
function groupPinDefs(
  def: SymbolDef,
  order: readonly string[],
  rawOrder: readonly string[],
  group: PinGroup,
): PinDef[] {
  const i0 = order.indexOf(group.first);
  const i1 = order.indexOf(group.last);
  if (i0 < 0 || i1 < 0) return [];
  return rawOrder
    .slice(i0, i1 + 1)
    .map((no) => outerPinDef(def, no))
    .filter((p): p is PinDef => p !== null);
}

/**
 * 書式としては読めたのに括弧を描けないグループの診断 (Rust側 undrawable_group_issues と同一)。
 * 多段端子台で段をまたぐ範囲 (`4a-1b`) や、シンボルの2辺にまたがる範囲がこれになる。
 */
export function undrawableGroupIssues(
  def: SymbolDef,
  groups: readonly PinGroup[],
  rename?: ReadonlyMap<string, string> | null,
): PinGroupIssue[] {
  const rawOrder = chartOrderPinNumbers(def);
  const order = displayOrder(rawOrder, rename);
  return groups
    .filter((g) => pinGroupBracket(groupPinDefs(def, order, rawOrder, g), g.caption) === null)
    .map((g) => ({
      text: `${g.rangeText}:${g.caption}`,
      code: PIN_GROUP_INVALID,
      message:
        "範囲がピンの向きをまたいでいます (多段端子台の段をまたぐ範囲や、シンボルの2辺にまたがる範囲には括弧を描けません)",
    }));
}

/**
 * attrs["pin_groups"]の診断を全部集める (Rust側 pin_group_issues と同一)。
 * `rename`はpinGroupBracketsと同じ端子番号の上書きの対応表 (渡さないと採番済みの端子台で
 * 「存在しないピン」を誤報する)。
 */
export function pinGroupIssues(
  def: SymbolDef,
  spec: string,
  rename?: ReadonlyMap<string, string> | null,
): PinGroupIssue[] {
  const parsed = parsePinGroups(spec, displayOrder(chartOrderPinNumbers(def), rename));
  return [...parsed.issues, ...undrawableGroupIssues(def, parsed.groups, rename)];
}
