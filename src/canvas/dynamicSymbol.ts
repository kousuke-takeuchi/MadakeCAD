// ピン数可変シンボル(connector_{n}p / terminal_block_{n}p / plc_di_{n}p / plc_do_{n}p / block)の
// TS側生成。Rust側 madake-core/src/symbol.rs の dynamic_symbol・madake-core/src/block.rs と
// 同一座標を返すこと(両者は dynamicSymbol.test.ts と Rust側テストで代表値を突き合わせている)。

import type { Point, Primitive, PinDef, SymbolDef, SymbolInstance, TextSlot } from "../ipc";
import {
  PIN_GROUPS_ATTR,
  PIN_GROUP_CAPTION_GAP_MM,
  PIN_GROUP_CAPTION_LINE_PITCH_MM,
  PIN_GROUP_CAPTION_TEXT_MM,
  pinGroupBrackets,
  type PinGroupBracket,
} from "./pinGroups";
// 循環import防止のため定義はtextWidth.tsへ移した。既存のimport元のために再エクスポートする
import { textWidthMm } from "./textWidth";
export { textWidthMm } from "./textWidth";

/** 動的シンボルの最大極数(Rust側 DYNAMIC_PIN_MAX と一致)。 */
export const DYNAMIC_PIN_MAX = 50;
/** 多段端子台 (`terminal_block_{n}p_{levels}l`、G6) の最大段数(Rust側 TERMINAL_LEVEL_MAX と一致)。 */
export const TERMINAL_LEVEL_MAX = 3;
/** 多段端子台の段(=列)の中心間隔 mm(Rust側 TERMINAL_LEVEL_PITCH と一致)。 */
export const TERMINAL_LEVEL_PITCH = 10.0;
/** 端子の印(貫通端子を表す丸)の半径 mm(Rust側 TERMINAL_MARK_R と一致)。 */
export const TERMINAL_MARK_R = 1.8;
/**
 * 端子番号の文字高さ mm(Rust側 TERMINAL_NUMBER_HEIGHT と一致)。
 * 文字の基準点は端子の印の中心そのもの。描画側がベースラインを基準点から
 * 文字高の半分だけ画面の下へ取るので、端子台を回しても番号は印の中に留まる。
 */
export const TERMINAL_NUMBER_HEIGHT = 2.0;
/** PLCモジュールの最大点数(Rust側 PLC_POINT_MAX と一致)。 */
export const PLC_POINT_MAX = 64;
/** PLCモジュールのI/O点の縦ピッチ mm(Rust側 PLC_POINT_PITCH_MM と一致)。 */
export const PLC_POINT_PITCH_MM = 5.0;

function parseCount(rest: string, max: number): number | null {
  const m = /^([0-9]+)p$/.exec(rest);
  if (!m) return null;
  const n = Number(m[1]);
  return n >= 1 && n <= max ? n : null;
}

function parsePinCount(rest: string): number | null {
  return parseCount(rest, DYNAMIC_PIN_MAX);
}

/**
 * `plc_di_{n}p` / `plc_do_{n}p` からPLC I/Oモジュールのシンボルを作る
 * (Rust側 `plc_module_symbol` と同一の座標)。縦長の箱の左側に点数ぶんの
 * 接続点を5mmピッチで並べ、箱の中に点番号と種別 (DI/DO) を書く。
 */
function plcModuleSymbol(id: string): SymbolDef | null {
  const input = id.startsWith("plc_di_");
  const output = id.startsWith("plc_do_");
  if (!input && !output) return null;
  const n = parseCount(id.slice("plc_di_".length), PLC_POINT_MAX);
  if (n === null) return null;
  const pitch = PLC_POINT_PITCH_MM;
  const at = (i: number) => (i - (n - 1) / 2) * pitch;
  const top = at(0) - pitch;
  const bottom = at(n - 1) + pitch;
  const primitives: Primitive[] = [
    { type: "rect", p1: p(-5, top), p2: p(5, bottom), filled: false },
    { type: "text", at: p(0, top + 3.5), text: input ? "DI" : "DO", height: 3 },
  ];
  const pins: PinDef[] = [];
  for (let i = 0; i < n; i++) {
    const y = at(i);
    primitives.push({ type: "text", at: p(-2.5, y + 1), text: String(i + 1), height: 2 });
    primitives.push({ type: "line", pts: [p(-7.5, y), p(-5, y)] });
    pins.push({ number: String(i + 1), name: "", at: p(-7.5, y), dir: "left" });
  }
  return {
    id,
    name: input ? `PLC input module ${n} points` : `PLC output module ${n} points`,
    name_ja: input ? `PLC入力モジュール(${n}点)` : `PLC出力モジュール(${n}点)`,
    category: "plc",
    ref_prefix: "PLC",
    primitives,
    pins,
  };
}

const p = (x: number, y: number): Point => ({ x, y });
const offset = (i: number, n: number) => (i - (n - 1) / 2) * 5.0;

/** 段番号(0始まり)を段記号('a','b','c',…)にする(Rust側 level_letter と一致)。 */
export function levelLetter(levelIndex: number): string {
  return String.fromCharCode(97 + levelIndex);
}

/**
 * 端子番号の表記から(極番号, 段記号)を取り出す(Rust側 parse_terminal_no と一致)。
 * 単段は数字だけ("8" → [8, null])、多段端子台 (G6) は末尾に段記号が付く("8a" → [8,"a"])。
 * 読み取れない番号は極番号を Number.MAX_SAFE_INTEGER として末尾に回す。
 */
export function parseTerminalNo(no: string): [number, string | null] {
  const m = /^([0-9]+)([a-zA-Z])$/.exec(no);
  if (m) return [Number(m[1]), m[2]];
  return [/^[0-9]+$/.test(no) ? Number(no) : Number.MAX_SAFE_INTEGER, null];
}

/**
 * `terminal_block_`の後ろを多段端子台の(極数, 段数)にパースする
 * (Rust側 parse_multilevel_terminal_block と同一規則)。`{n}p_{levels}l`の形だけを
 * 受け付け(例 "8p_2l")、単段の`{n}p`はnullを返す(呼び出し側の既存経路がそのまま扱う)。
 */
function parseMultiLevelTerminalBlock(rest: string): { n: number; levels: number } | null {
  const parts = rest.split("_");
  if (parts.length !== 2) return null;
  const n = parseCount(parts[0], DYNAMIC_PIN_MAX);
  const m = /^([0-9]+)l$/.exec(parts[1]);
  if (n === null || !m) return null;
  const levels = Number(m[1]);
  if (levels < 1 || levels > TERMINAL_LEVEL_MAX) return null;
  return { n, levels };
}

/**
 * 端子台のシンボルを作る(Rust側 terminal_block_symbol と同一座標)。
 * 端子は縦にn行(5mmピッチ・2.5mmグリッド)、段(レベル)は**横に並ぶ列**で表す(G6)。
 * 列の中心間隔は TERMINAL_LEVEL_PITCH で、同じ極の端子は段が違っても同じ行に載る。
 * `levels === 1`では列が1本・ピン番号は数字だけになり、従来の単段端子台と同一。
 */
function terminalBlockSymbol(id: string, n: number, levels: number): SymbolDef {
  const columnX = (j: number) => (j - (levels - 1) / 2) * TERMINAL_LEVEL_PITCH;
  const primitives: Primitive[] = [];
  const pins: PinDef[] = [];
  for (let j = 0; j < levels; j++) {
    const cx = columnX(j);
    primitives.push({
      type: "rect",
      p1: p(cx - 2.5, offset(0, n) - 2.5),
      p2: p(cx + 2.5, offset(n - 1, n) + 2.5),
      filled: false,
    });
    for (let i = 0; i < n; i++) {
      const y = offset(i, n);
      const no = levels === 1 ? String(i + 1) : `${i + 1}${levelLetter(j)}`;
      primitives.push({ type: "circle", center: p(cx, y), r: TERMINAL_MARK_R, filled: false });
      // 基準点は印の中心。ベースラインは描画側が画面の下向きに文字高の半分ずらす
      primitives.push({ type: "text", at: p(cx, y), text: no, height: TERMINAL_NUMBER_HEIGHT });
      primitives.push({ type: "line", pts: [p(cx - 2.5, y), p(cx - TERMINAL_MARK_R, y)] });
      primitives.push({ type: "line", pts: [p(cx + TERMINAL_MARK_R, y), p(cx + 2.5, y)] });
      // 貫通端子: 左右2接続点に同一ピン番号(段が違えば番号も違うのでネットも独立)
      pins.push({ number: no, name: "", at: p(cx - 2.5, y), dir: "left" });
      pins.push({ number: no, name: "", at: p(cx + 2.5, y), dir: "right" });
    }
  }
  return {
    id,
    name: levels === 1 ? `Terminal block ${n}P` : `Terminal block ${n}P ${levels}-level`,
    name_ja: levels === 1 ? `端子台(${n}極)` : `端子台(${n}極 ${levels}段)`,
    category: "connector",
    ref_prefix: "TB",
    primitives,
    pins,
  };
}

export function dynamicSymbol(id: string): SymbolDef | null {
  if (id === BLOCK_SYMBOL_ID) return blockSymbol({});
  if (id.startsWith("plc_di_") || id.startsWith("plc_do_")) return plcModuleSymbol(id);
  if (id.startsWith("connector_")) {
    const n = parsePinCount(id.slice("connector_".length));
    if (n === null) return null;
    const primitives: Primitive[] = [
      { type: "rect", p1: p(-4, offset(0, n) - 2.5), p2: p(4, offset(n - 1, n) + 2.5), filled: false },
    ];
    const pins: PinDef[] = [];
    for (let i = 0; i < n; i++) {
      const y = offset(i, n);
      primitives.push({ type: "text", at: p(-2, y), text: String(i + 1), height: 2 });
      primitives.push({ type: "line", pts: [p(4, y), p(7.5, y)] });
      pins.push({ number: String(i + 1), name: "", at: p(7.5, y), dir: "right" });
    }
    return {
      id,
      name: `Connector ${n}P`,
      name_ja: `コネクタ(${n}極)`,
      category: "connector",
      ref_prefix: "J",
      primitives,
      pins,
    };
  }
  if (id.startsWith("terminal_block_")) {
    const rest = id.slice("terminal_block_".length);
    const multi = parseMultiLevelTerminalBlock(rest);
    if (multi) return terminalBlockSymbol(id, multi.n, multi.levels);
    const n = parsePinCount(rest);
    if (n === null) return null;
    return terminalBlockSymbol(id, n, 1);
  }
  return null;
}

/** 静的ライブラリ優先でsymbol_idを解決し、無ければ動的生成にフォールバックする。 */
export function resolveSymbolDef(
  id: string,
  defs: ReadonlyMap<string, SymbolDef>,
): SymbolDef | undefined {
  return defs.get(id) ?? dynamicSymbol(id) ?? undefined;
}

/**
 * 配置済みシンボル1個の定義。ブロック ([`BLOCK_SYMBOL_ID`]) だけは形がインスタンスの
 * 属性 (block_pins / block_size) で決まるので、そこから作る
 * (Rust側 `madake_core::symbol::instance_symbol_def` と同じ規則)。
 * 属性が壊れているブロックは`undefined`(描かない)。
 */
export function resolveInstanceDef(
  inst: SymbolInstance,
  defs: ReadonlyMap<string, SymbolDef>,
): SymbolDef | undefined {
  if (inst.symbol_id === BLOCK_SYMBOL_ID) {
    try {
      return blockSymbol(inst.attrs ?? {});
    } catch {
      return undefined;
    }
  }
  return resolveSymbolDef(inst.symbol_id, defs);
}

// ---- ブロックシンボル (madake-core/src/block.rs と対) ----

/** ブロックシンボルの symbol_id。 */
export const BLOCK_SYMBOL_ID = "block";
/** ピン表を持つ属性キー(JSON)。 */
export const BLOCK_PINS_ATTR = "block_pins";
/** 外形寸法を持つ属性キー(JSON `{"w":mm,"h":mm}`)。 */
export const BLOCK_SIZE_ATTR = "block_size";
/** ピンの並びピッチ mm。 */
export const BLOCK_PIN_PITCH_MM = 5.0;
/** 外形からピン接続点までのリード長 mm。 */
export const BLOCK_LEAD_MM = 2.5;
/** ピン番号・ピン名の文字高さ mm。 */
export const BLOCK_PIN_TEXT_MM = 2.0;
/** 外形の内側にピン名を置くときの余白 mm。 */
export const BLOCK_TEXT_PAD_MM = 1.5;
/** 外形寸法の刻み mm。 */
export const BLOCK_SIZE_STEP_MM = 5.0;
/** 外形の最小幅・最小高さ mm。 */
export const BLOCK_MIN_W_MM = 15.0;
export const BLOCK_MIN_H_MM = 10.0;
/** 1ブロックに置けるピンの最大数(全辺の合計)。 */
export const BLOCK_PIN_MAX = 200;
/** 属性スロットの行送り mm。 */
export const BLOCK_SLOT_PITCH_MM = 2.5;
/** 属性スロットのキー(Rust側 SLOT_TAG などと同じ)。 */
const BLOCK_SLOT_KEYS = ["TAG", "PART", "DESC", "RATING"];

/** ブロックの1ピン。 */
export interface BlockPin {
  no: string;
  name?: string;
}

/** ブロックのピン配置(辺ごと。左右は上→下、上下は左→右)。 */
export interface BlockPins {
  left?: BlockPin[];
  right?: BlockPin[];
  top?: BlockPin[];
  bottom?: BlockPin[];
}

const BLOCK_SIDES = ["left", "right", "top", "bottom"] as const;

/**
 * シンボルローカル座標→用紙座標。netlist.rs transform_localと同一の変換。
 * ここに置く (renderer.tsではない) のは、canvas/leader.tsのように図面描画とは
 * 別方向から座標変換だけを必要とするモジュールが、renderer.tsを経由せず (循環importを
 * 起こさず) 読めるようにするため。renderer.tsは互換のため再エクスポートする。
 */
export function transformLocal(p: Point, inst: SymbolInstance): Point {
  const x0 = inst.mirror ? -p.x : p.x;
  const y0 = p.y;
  let rx = x0;
  let ry = y0;
  switch (inst.rotation % 360) {
    case 90:
      rx = -y0;
      ry = x0;
      break;
    case 180:
      rx = -x0;
      ry = -y0;
      break;
    case 270:
      rx = y0;
      ry = -x0;
      break;
  }
  return { x: inst.at.x + rx, y: inst.at.y + ry };
}

/**
 * 配置後のシンボル外形の囲み矩形(用紙座標)。円・弧は回転対称なので中心±rで安全側に
 * 評価し、配置基準点も必ず含める。Rust側 **svg::symbol_bounds** と同一
 * (文字プリミティブは基準点だけを見る。文字の幅・高さまで含める tidy::symbol_bounds とは別物)。
 * 参照記号・接点マップ・引き出し線 (G20) の指す先など、外形を基準にする位置決めに使う。
 * ピングループ括弧 (G8) はシンボルと一緒に描かれるので、その箱も含める。
 */
export function symbolBounds(
  inst: SymbolInstance,
  def: SymbolDef,
): { min: Point; max: Point } {
  const min = { ...inst.at };
  const max = { ...inst.at };
  const visit = (p: Point) => {
    const t = transformLocal(p, inst);
    min.x = Math.min(min.x, t.x);
    min.y = Math.min(min.y, t.y);
    max.x = Math.max(max.x, t.x);
    max.y = Math.max(max.y, t.y);
  };
  for (const prim of def.primitives) {
    switch (prim.type) {
      case "line":
        prim.pts.forEach(visit);
        break;
      case "circle":
      case "arc":
        visit({ x: prim.center.x - prim.r, y: prim.center.y - prim.r });
        visit({ x: prim.center.x + prim.r, y: prim.center.y + prim.r });
        break;
      case "rect":
        visit(prim.p1);
        visit({ x: prim.p2.x, y: prim.p1.y });
        visit(prim.p2);
        visit({ x: prim.p1.x, y: prim.p2.y });
        break;
      case "text":
        visit(prim.at);
        break;
    }
  }
  for (const pin of def.pins) visit(pin.at);
  for (const [a, b] of pinGroupPaperBoxes(inst, def)) {
    min.x = Math.min(min.x, a.x);
    min.y = Math.min(min.y, a.y);
    max.x = Math.max(max.x, b.x);
    max.y = Math.max(max.y, b.y);
  }
  return { min, max };
}

/**
 * ピングループ括弧 (G8) とキャプションが用紙の上で占める矩形の一覧
 * (Rust側 pin_groups::pin_group_paper_boxes と同一)。文字は回転しても横書きのままなので、
 * キャプションの箱は用紙座標で文字幅・文字高から取る。
 *
 * ここに置く (pinGroups.tsではない) のは、pinGroups.tsが座標変換も文字幅も持たない
 * 葉モジュールのままでいられるようにするため (循環importを起こさない)。
 */
export function pinGroupPaperBoxes(
  inst: SymbolInstance,
  def: SymbolDef,
): [Point, Point][] {
  const spec = inst.attrs?.[PIN_GROUPS_ATTR];
  if (!spec) return [];
  const out: [Point, Point][] = [];
  for (const bracket of pinGroupBrackets(def, spec)) {
    const pts = bracket.bracket.map((p) => transformLocal(p, inst));
    if (pts.length > 0) {
      const min = { x: Infinity, y: Infinity };
      const max = { x: -Infinity, y: -Infinity };
      for (const p of pts) {
        min.x = Math.min(min.x, p.x);
        min.y = Math.min(min.y, p.y);
        max.x = Math.max(max.x, p.x);
        max.y = Math.max(max.y, p.y);
      }
      out.push([min, max]);
    }
    for (const [text, c] of captionLineAnchors(inst, bracket)) {
      const hw = textWidthMm(text, PIN_GROUP_CAPTION_TEXT_MM) / 2;
      const hh = PIN_GROUP_CAPTION_TEXT_MM / 2;
      out.push([
        { x: c.x - hw, y: c.y - hh },
        { x: c.x + hw, y: c.y + hh },
      ]);
    }
  }
  return out;
}

/**
 * キャプション各行の描画位置 (用紙座標の、文字の幅・高さの中心)
 * (Rust側 pin_groups::caption_line_anchors と同一)。
 *
 * 文字はシンボルを回してもいつも横書きなので、行は必ず用紙の下方向へ積む (ローカル座標で
 * 積むと、回転90/270度では行送りが横方向のずれになって行同士が重なる)。キャプションの塊
 * 全体が括弧の棒より外に出るように、外向きが用紙の横軸なら塊の幅の半分、縦軸なら塊の高さの
 * 半分だけ間隔に足して塊の中心を置く。空行は描かないので返さない。
 */
export function captionLineAnchors(
  inst: SymbolInstance,
  bracket: PinGroupBracket,
): [string, Point][] {
  const lines = bracket.caption.filter((l) => l !== "");
  if (lines.length === 0) return [];
  const from = transformLocal(bracket.captionFrom, inst);
  const tip = transformLocal(bracket.captionDir, inst);
  const dir = { x: tip.x - inst.at.x, y: tip.y - inst.at.y };
  const width = lines.reduce((w, l) => Math.max(w, textWidthMm(l, PIN_GROUP_CAPTION_TEXT_MM)), 0);
  const pitch = PIN_GROUP_CAPTION_LINE_PITCH_MM;
  const height = (lines.length - 1) * pitch + PIN_GROUP_CAPTION_TEXT_MM;
  // 回転は0/90/180/270だけなので、外向きは必ず用紙の縦横どちらかに揃っている
  const half = Math.abs(dir.x) > Math.abs(dir.y) ? width / 2 : height / 2;
  const center = {
    x: from.x + dir.x * (PIN_GROUP_CAPTION_GAP_MM + half),
    y: from.y + dir.y * (PIN_GROUP_CAPTION_GAP_MM + half),
  };
  const firstY = center.y - ((lines.length - 1) * pitch) / 2;
  return lines.map((l, i) => [l, { x: center.x, y: firstY + i * pitch }]);
}

function snapUp(v: number, step: number): number {
  return Math.ceil(v / step - 1e-9) * step;
}

function nameWidth(name: string): number {
  return textWidthMm(name, BLOCK_PIN_TEXT_MM);
}

function maxNameWidth(side: BlockPin[]): number {
  return side.reduce((w, p) => Math.max(w, nameWidth(p.name ?? "")), 0);
}

function captionInset(side: BlockPin[]): number {
  if (side.length === 0) return 0;
  return side.some((p) => (p.name ?? "") !== "") ? 7.5 : 5.0;
}

function sideOf(pins: BlockPins, key: (typeof BLOCK_SIDES)[number]): BlockPin[] {
  return pins[key] ?? [];
}

/**
 * 属性からピン表と外形寸法を読み取る。Rust側 `parse_block_spec` と同じ検証をし、
 * 同じ理由でエラーを投げる(番号の重複・空番号・ピン過多・寸法不足・壊れたJSON)。
 */
export function parseBlockSpec(attrs: Record<string, string>): {
  pins: BlockPins;
  size: { w: number; h: number } | null;
} {
  const rawPins = attrs[BLOCK_PINS_ATTR];
  let pins: BlockPins = {};
  if (rawPins && rawPins.trim()) {
    let parsed: unknown;
    try {
      parsed = JSON.parse(rawPins);
    } catch (e) {
      throw new Error(`${BLOCK_PINS_ATTR} is not valid JSON: ${String(e)}`);
    }
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
      throw new Error(`${BLOCK_PINS_ATTR} is not valid JSON: expected an object`);
    }
    for (const key of Object.keys(parsed as Record<string, unknown>)) {
      if (!BLOCK_SIDES.includes(key as (typeof BLOCK_SIDES)[number])) {
        throw new Error(`${BLOCK_PINS_ATTR} is not valid JSON: unknown side ${key}`);
      }
    }
    pins = parsed as BlockPins;
  }
  const all = BLOCK_SIDES.flatMap((s) => sideOf(pins, s));
  if (all.length > BLOCK_PIN_MAX) {
    throw new Error(`block has too many pins: ${all.length} (max ${BLOCK_PIN_MAX})`);
  }
  const seen = new Set<string>();
  for (const pin of all) {
    const no = (pin.no ?? "").trim();
    if (!no) throw new Error("block pin number must not be empty");
    if (seen.has(no)) throw new Error(`duplicate block pin number: ${no}`);
    seen.add(no);
  }

  const rawSize = attrs[BLOCK_SIZE_ATTR];
  let size: { w: number; h: number } | null = null;
  if (rawSize && rawSize.trim()) {
    let parsed: { w?: number; h?: number };
    try {
      parsed = JSON.parse(rawSize);
    } catch (e) {
      throw new Error(`${BLOCK_SIZE_ATTR} is not valid JSON: ${String(e)}`);
    }
    const { w, h } = parsed;
    if (typeof w !== "number" || typeof h !== "number" || !(w > 0) || !(h > 0)) {
      throw new Error("block_size must be positive");
    }
    size = { w: snapUp(w, BLOCK_SIZE_STEP_MM), h: snapUp(h, BLOCK_SIZE_STEP_MM) };
    const rows = Math.max(sideOf(pins, "left").length, sideOf(pins, "right").length);
    const cols = Math.max(sideOf(pins, "top").length, sideOf(pins, "bottom").length);
    const needW = Math.max(cols * BLOCK_PIN_PITCH_MM, BLOCK_SIZE_STEP_MM);
    const needH = Math.max(rows * BLOCK_PIN_PITCH_MM, BLOCK_SIZE_STEP_MM);
    if (size.w < needW - 1e-9 || size.h < needH - 1e-9) {
      throw new Error(
        `block_size ${size.w}x${size.h}mm is too small for its pins (needs at least ${needW}x${needH}mm)`,
      );
    }
  }
  return { pins, size };
}

/** ブロックの外形寸法 mm (Rust側 `block_box` と同じ)。 */
export function blockBox(pins: BlockPins, size: { w: number; h: number } | null): [number, number] {
  if (size) return [size.w, size.h];
  const left = sideOf(pins, "left");
  const right = sideOf(pins, "right");
  const top = sideOf(pins, "top");
  const bottom = sideOf(pins, "bottom");
  const rows = Math.max(left.length, right.length);
  const cols = Math.max(top.length, bottom.length);
  const wNames =
    2 * BLOCK_TEXT_PAD_MM + maxNameWidth(left) + maxNameWidth(right) + BLOCK_PIN_PITCH_MM;
  const w = Math.max(BLOCK_MIN_W_MM, wNames, cols * BLOCK_PIN_PITCH_MM + BLOCK_PIN_PITCH_MM);
  const h = Math.max(
    BLOCK_MIN_H_MM,
    rows * BLOCK_PIN_PITCH_MM + BLOCK_PIN_PITCH_MM,
    captionInset(top) + captionInset(bottom) + BLOCK_PIN_PITCH_MM,
  );
  return [snapUp(w, BLOCK_SIZE_STEP_MM), snapUp(h, BLOCK_SIZE_STEP_MM)];
}

/**
 * 属性からブロックのシンボル定義を作る(Rust側 `block_symbol` と同一座標)。
 * 外形は原点中心の長方形。ピンは辺ごとに中央揃え・5mmピッチで、
 * ピン番号は外形の外側、ピン名は内側に描く(上下辺は番号・名前とも内側)。
 */
export function blockSymbol(attrs: Record<string, string>): SymbolDef {
  const { pins: table, size } = parseBlockSpec(attrs);
  const [w, h] = blockBox(table, size);
  const hw = w / 2;
  const hh = h / 2;
  const primitives: Primitive[] = [
    { type: "rect", p1: p(-hw, -hh), p2: p(hw, hh), filled: false },
  ];
  const pins: PinDef[] = [];
  const push = (side: BlockPin[], dir: "left" | "right" | "up" | "down") => {
    const n = side.length;
    for (let i = 0; i < n; i++) {
      const bp = side[i];
      const off = offset(i, n);
      const no = (bp.no ?? "").trim();
      const name = bp.name ?? "";
      let at: Point;
      let edge: Point;
      let numAt: Point;
      let nameAt: Point;
      if (dir === "left") {
        at = p(-hw - BLOCK_LEAD_MM, off);
        edge = p(-hw, off);
        numAt = p(-hw - BLOCK_LEAD_MM / 2, off - BLOCK_PIN_TEXT_MM);
        nameAt = p(-hw + BLOCK_TEXT_PAD_MM + nameWidth(name) / 2, off);
      } else if (dir === "right") {
        at = p(hw + BLOCK_LEAD_MM, off);
        edge = p(hw, off);
        numAt = p(hw + BLOCK_LEAD_MM / 2, off - BLOCK_PIN_TEXT_MM);
        nameAt = p(hw - BLOCK_TEXT_PAD_MM - nameWidth(name) / 2, off);
      } else if (dir === "up") {
        at = p(off, -hh - BLOCK_LEAD_MM);
        edge = p(off, -hh);
        numAt = p(off, -hh + BLOCK_PIN_TEXT_MM);
        nameAt = p(off, -hh + BLOCK_PIN_TEXT_MM + BLOCK_PIN_PITCH_MM / 2);
      } else {
        at = p(off, hh + BLOCK_LEAD_MM);
        edge = p(off, hh);
        numAt = p(off, hh - BLOCK_PIN_TEXT_MM);
        nameAt = p(off, hh - BLOCK_PIN_TEXT_MM - BLOCK_PIN_PITCH_MM / 2);
      }
      primitives.push({ type: "line", pts: [edge, at] });
      primitives.push({ type: "text", at: numAt, text: no, height: BLOCK_PIN_TEXT_MM });
      if (name) {
        primitives.push({ type: "text", at: nameAt, text: name, height: BLOCK_PIN_TEXT_MM });
      }
      pins.push({ number: no, name, at, dir });
    }
  };
  push(sideOf(table, "left"), "left");
  push(sideOf(table, "right"), "right");
  push(sideOf(table, "top"), "up");
  push(sideOf(table, "bottom"), "down");

  const text_slots: TextSlot[] = BLOCK_SLOT_KEYS.map((key, i) => ({
    key,
    at: p(0, (i - 1.5) * BLOCK_SLOT_PITCH_MM),
    height: 2.5,
  }));
  return {
    id: BLOCK_SYMBOL_ID,
    name: "Block (generic device)",
    name_ja: "ブロック(汎用機器)",
    category: "block",
    ref_prefix: "U",
    keywords: [
      "block",
      "device",
      "module",
      "controller",
      "unit",
      "ブロック",
      "機器",
      "モジュール",
      "コントローラ",
      "汎用",
    ],
    primitives,
    pins,
    text_slots,
  };
}
