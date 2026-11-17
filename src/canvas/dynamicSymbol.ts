// ピン数可変シンボル(connector_{n}p / terminal_block_{n}p / plc_di_{n}p / plc_do_{n}p / block)の
// TS側生成。Rust側 madake-core/src/symbol.rs の dynamic_symbol・madake-core/src/block.rs と
// 同一座標を返すこと(両者は dynamicSymbol.test.ts と Rust側テストで代表値を突き合わせている)。

import type { Point, Primitive, PinDef, SymbolDef, SymbolInstance, TextSlot } from "../ipc";
import {
  CONNECTOR_IN_MM,
  CONNECTOR_LABEL_GAP_MM,
  CONNECTOR_LABEL_INSET_MM,
  CONNECTOR_LABEL_TOP_MM,
  CONNECTOR_LEAD_MM,
  CONNECTOR_NUM_GAP_MM,
  CONNECTOR_NUM_SIDE_MM,
  CONNECTOR_OUT_MM,
  connectorGroups,
  connectorPrimitives,
  groupLabel,
  isConnectorKind,
  isGroupedConnector,
  outlineSegments,
  coarseOffsets,
  sideOffset as offset,
  type ConnectorGroup,
  type ConnectorKind,
} from "./connector";
import {
  PIN_GROUPS_ATTR,
  PIN_GROUP_CAPTION_GAP_MM,
  PIN_GROUP_CAPTION_LINE_PITCH_MM,
  PIN_GROUP_CAPTION_TEXT_MM,
  pinGroupBrackets,
  type PinGroupBracket,
} from "./pinGroups";
// 循環import防止のため定義はtextWidth.tsへ移した。既存のimport元のために再エクスポートする
import { isDeviceBlock } from "./relayXref";
import { textWidthMm } from "./textWidth";
import { CAPTION_CAP_RATIO, LABEL_FONT, REF_LABEL_DY, VALUE_LABEL_DY } from "./textStyle";
export { textWidthMm } from "./textWidth";
// 端子番号の読み取りと端子の印の寸法も葉モジュール (terminalNo.ts) へ移した
// (terminalJumpers.ts の端子番号の対応表をここから使うための循環import防止)。
// 既存のimport元のためにここから再エクスポートする
import { levelLetter, TERMINAL_LEVEL_MAX, TERMINAL_MARK_R, TERMINAL_NUMBER_HEIGHT } from "./terminalNo";
export {
  levelLetter,
  parseTerminalNo,
  TERMINAL_LEVEL_MAX,
  TERMINAL_MARK_R,
  TERMINAL_NUMBER_HEIGHT,
} from "./terminalNo";
import { terminalLabels } from "./terminalJumpers";

/** 動的シンボルの最大極数(Rust側 DYNAMIC_PIN_MAX と一致)。 */
export const DYNAMIC_PIN_MAX = 50;
/** 多段端子台の段(=列)の中心間隔 mm(Rust側 TERMINAL_LEVEL_PITCH と一致)。 */
export const TERMINAL_LEVEL_PITCH = 10.0;
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
  // 分割片 `plc_di_{n}p_s{start}` は点番号を start から振る (Rust側と同じ規則)
  const rest = id.slice("plc_di_".length);
  const split = /^([0-9]+)p_s([0-9]+)$/.exec(rest);
  const n = split ? Number(split[1]) : parseCount(rest, PLC_POINT_MAX);
  const start = split ? Number(split[2]) : 1;
  if (n === null || n < 1 || n > PLC_POINT_MAX || start < 1 || start + n - 1 > PLC_POINT_MAX) return null;
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
    const no = String(start + i);
    primitives.push({ type: "text", at: p(-2.5, y + 1), text: no, height: 2 });
    primitives.push({ type: "line", pts: [p(-7.5, y), p(-5, y)] });
    pins.push({ number: no, name: "", at: p(-7.5, y), dir: "left" });
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

/**
 * `terminal_block_`の後ろを多段端子台の(極数, 段数)にパースする
 * (Rust側 parse_multilevel_terminal_block と同一規則)。`{n}p_{levels}l`の形だけを
 * 受け付け(例 "8p_2l")、単段の`{n}p`はnullを返す(呼び出し側の既存経路がそのまま扱う)。
 *
 * **段数は2以上だけを受け付ける** (`_1l`という別名idは作らない。Task 3)。単段は常に
 * `terminal_block_{n}p`という1つのidだけで表すので、`terminal_block_8p_1l`はここでnullになり
 * 呼び出し側 (`dynamicSymbol`) は「知らないシンボル」としてnullを返す (Rust側と同じ)。
 */
function parseMultiLevelTerminalBlock(rest: string): { n: number; levels: number } | null {
  const parts = rest.split("_");
  if (parts.length !== 2) return null;
  const n = parseCount(parts[0], DYNAMIC_PIN_MAX);
  const m = /^([0-9]+)l$/.exec(parts[1]);
  if (n === null || !m) return null;
  const levels = Number(m[1]);
  if (levels < 2 || levels > TERMINAL_LEVEL_MAX) return null;
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
/** ピン間隔を持つ属性キー ("5" か "2.5"。無ければ5)。 */
export const BLOCK_PITCH_ATTR = "block_pitch";
/** ピン番号を外形の外に描くか内に描くかを持つ属性キー ("outside" か "inside"。無ければ外)。 */
export const BLOCK_NUMBERS_ATTR = "block_numbers";
/** ピン名・見出し・番号の文字の高さを持つ属性キー (mm、1.5〜5.0。無ければ BLOCK_PIN_TEXT_MM)。Rust の BLOCK_PIN_TEXT_ATTR と同じ。 */
export const BLOCK_PIN_TEXT_ATTR = "block_pin_text";
/** block_pin_text の下限・上限 mm。 */
export const BLOCK_PIN_TEXT_MIN_MM = 1.5;
export const BLOCK_PIN_TEXT_MAX_MM = 5.0;
/** 1つのすき間の行の最大の行数 (Rust側 BLOCK_GAP_MAX)。 */
export const BLOCK_GAP_MAX = 20;
/** 1つのブロックの行の最大数 (Rust側 BLOCK_ROW_MAX)。全辺の合計。 */
export const BLOCK_ROW_MAX = 400;
/** 細かいピン間隔 mm。グリッドと同じ。 */
export const BLOCK_FINE_PITCH_MM = 2.5;
/** 細かい間隔のブロックの最小の外形 mm (幅・高さとも)。 */
const BLOCK_FINE_MIN_MM = 5.0;
/** 番号を内側に描くとき、左右の番号のあいだに空けるすき間 mm。 */
const BLOCK_INSIDE_GAP_MM = 1.0;
/** ブロックのピン間隔 (Rust側 BlockPitch)。 */
export type BlockPitch = "coarse" | "fine";
/** ピン番号を描く場所 (Rust側 BlockNumbers)。 */
export type BlockNumbers = "outside" | "inside";
/** リード線を描くかを持つ属性キー ("lead" か "none")。Rust の BLOCK_LEADS_ATTR と同じ。 */
export const BLOCK_LEADS_ATTR = "block_leads";
export type BlockLeads = "lead" | "none";
/** 外形からピン接続点までの長さ (mm)。 */
export function blockLeadMm(leads: BlockLeads): number {
  return leads === "none" ? 0 : BLOCK_LEAD_MM;
}
/** 参照記号・型番を外形の上に描くか中に描くかを持つ属性キー。Rust の BLOCK_CAPTION_ATTR と同じ。 */
export const BLOCK_CAPTION_ATTR = "block_caption";
export type BlockCaption = "above" | "inside";
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
/** 1本のピンが持てる接点の最大数 (Rust側 BLOCK_CONTACT_MAX)。 */
export const BLOCK_CONTACT_MAX = 32;
/** 属性スロットの行送り mm。 */
export const BLOCK_SLOT_PITCH_MM = 2.5;
/** 属性スロットのキー(Rust側 SLOT_TAG などと同じ)。 */
const BLOCK_SLOT_KEYS = ["TAG", "PART", "DESC", "RATING"];

/** ピン間隔 mm (Rust側 BlockPitch::mm)。 */
export function blockPitchMm(pitch: BlockPitch): number {
  return pitch === "fine" ? BLOCK_FINE_PITCH_MM : BLOCK_PIN_PITCH_MM;
}

/** 外形寸法の刻み mm (Rust側 BlockPitch::size_step)。 */
export function blockSizeStep(pitch: BlockPitch): number {
  return pitch === "fine" ? BLOCK_FINE_PITCH_MM : BLOCK_SIZE_STEP_MM;
}

/** ピンがn本の辺に要る長さ mm (Rust側 side_need)。5mm間隔は n×5 (最小5)、2.5mm間隔は (n+1)×2.5 (最小5)。 */
export function sideNeed(n: number, pitch: BlockPitch): number {
  return pitch === "fine"
    ? Math.max((n + 1) * BLOCK_FINE_PITCH_MM, BLOCK_FINE_MIN_MM)
    : Math.max(n * BLOCK_PIN_PITCH_MM, BLOCK_SIZE_STEP_MM);
}

function isGrid(v: number): boolean {
  const k = v / BLOCK_FINE_PITCH_MM;
  return Math.abs(k - Math.round(k)) < 1e-9;
}

/** 外形をその軸でずらす量 mm (Rust側 axis_shift)。2.5mm間隔で大きさが2.5mmの奇数倍のとき1.25、ほかは0。 */
function axisShift(size: number, pitch: BlockPitch): number {
  return pitch === "fine" && !isGrid(size / 2) ? BLOCK_FINE_PITCH_MM / 2 : 0;
}

/** 辺のピンn本の、外形の中心から測ったオフセット mm (Rust側 row_offsets)。 */
function rowOffsets(n: number, size: number, pitch: BlockPitch): number[] {
  if (n === 0) return [];
  const step = blockPitchMm(pitch);
  const first = (-(n - 1) / 2) * step;
  const nudge = isGrid(first + axisShift(size, pitch)) ? 0 : BLOCK_FINE_PITCH_MM / 2;
  return Array.from({ length: n }, (_, i) => first + i * step + nudge);
}

/**
 * 辺の各項目 (添字は辺のピン表と同じ) の最初の行の、外形の中心から測ったオフセット mm (Rust側 side_offsets)。
 * 行の数で中央揃えにするので、すき間も箱の大きさとピンの位置に効く。
 */
function sideOffsets(side: readonly BlockPin[], size: number, pitch: BlockPitch): number[] {
  const rows = rowOffsets(sideRows(side), size, pitch);
  let next = 0;
  return side.map((e) => {
    const at = rows[next];
    next += blockRows(e);
    return at;
  });
}

/**
 * ブロックの1項目 (Rust側 BlockPin): 普通のピン、すき間の行 (gap)、見出しの行 (caption) のどれか。
 * 番号が空 (無い) ピンはピン名が鍵になる。
 */
export interface BlockPin {
  no?: string;
  name?: string;
  /** ポートの接点 (Rust側 BlockPin::contacts)。 */
  contacts?: string[];
  /** 外形の辺に描くコネクタの形 (Rust側 BlockPin::connector)。無ければ普通のピン。 */
  connector?: ConnectorKind;
  /** まとめて1つの本体に描くコネクタ (端子台・JST) の名前 (Rust側 BlockPin::connector_ref)。 */
  connector_ref?: string;
  /** すき間の行: その辺のピンの位置を n 行 (1〜20) 空ける。ピンではない。 */
  gap?: number;
  /** 見出しの行: その辺の1行を使い、箱の内側に太字で書く。ピンではない。 */
  caption?: string;
}

/** 普通のピンか (すき間・見出しの行ではない)。 */
export function isBlockPin(e: BlockPin): boolean {
  return e.gap === undefined && e.caption === undefined;
}

/** 項目がその辺で使う行の数 (Rust側 BlockPin::rows)。 */
export function blockRows(e: BlockPin): number {
  return e.gap ?? 1;
}

/** 辺の行の数 (Rust側 side_rows)。 */
export function sideRows(side: readonly BlockPin[]): number {
  return side.reduce((n, e) => n + blockRows(e), 0);
}

/** 番号があるか (Rust側 BlockPin::numbered)。 */
export function isNumbered(e: BlockPin): boolean {
  return (e.no ?? "").trim() !== "";
}

/** ピンの鍵: 番号。番号が無ければピン名 (どちらも前後の空白を除く。Rust側 BlockPin::key)。 */
export function blockPinKey(e: BlockPin): string {
  return isNumbered(e) ? (e.no ?? "").trim() : (e.name ?? "").trim();
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
 * 参照記号・型番を上に、機能ラベル・説明・定格を下に置くための外形 (Rust側 svg::caption_bounds と同一)。
 * 普通のシンボルは symbolBounds そのもの。ブロックは、ピン番号などの文字が基準点だけでなく文字の高さの
 * ぶん上下へはみ出す (文字は回転しても横書きで、回転すると左右辺のピン番号が上下辺に来る) ので、
 * 文字の高さも含める。文字は常に横書きなので、上下の範囲に効くのは文字の高さだけ。
 */
export function captionBounds(inst: SymbolInstance, def: SymbolDef): { min: Point; max: Point } {
  const { min, max } = symbolBounds(inst, def);
  if (def.id !== BLOCK_SYMBOL_ID) return { min, max };
  for (const prim of def.primitives) {
    if (prim.type !== "text") continue;
    const c = transformLocal(prim.at, inst);
    min.y = Math.min(min.y, c.y - prim.height / 2);
    max.y = Math.max(max.y, c.y + prim.height / 2);
  }
  return { min, max };
}

/** 参照記号・型番の置き場所 (Rust側 svg::CaptionAnchor と同一): 基準X・基準Y・右揃えか。
 * 各行のベースラインは top - REF_LABEL_DY / top - VALUE_LABEL_DY。 */
export interface CaptionAnchor {
  x: number;
  top: number;
  end: boolean;
}

/** 外形の左辺と、右揃えにした参照記号・型番の右端との間隔 (mm)。Rust側 svg::CORNER_CAPTION_GAP と同じ。 */
export const CORNER_CAPTION_GAP = 1.0;

/**
 * 参照記号・型番の置き場所 (Rust側 svg::caption_anchor と同一)。
 * 普通は配置基準点の真上に中央揃え。ただしブロックの描かれた上辺 (回転・ミラー後) にピンが
 * あるときは、そこから出る配線が中央の文字を横切るので、外形の左上の角の外 (左辺の
 * CORNER_CAPTION_GAP 左・上辺の上) に右揃えで置く。ブロックが描く他のもの (他の辺のリード・番号・
 * コネクタの絵・まとまりの名前) に文字が重なるときは、その上まで上げる。
 */
export function captionAnchor(inst: SymbolInstance, def: SymbolDef): CaptionAnchor {
  return (
    insideCaptionAnchor(inst, def) ??
    topLeftCornerAnchor(inst, def) ?? { x: centredCaptionX(inst, def), top: captionBounds(inst, def).min.y, end: false }
  );
}

/**
 * 中央揃えの参照記号・型番の基準X (Rust側 svg::centred_caption_x)。普通は配置点、ブロックは描かれた外形の
 * 中心 (回転・ミラーの後)。2.5mm間隔で幅が2.5mmの奇数倍のブロックは外形が1.25mmずれるため。
 * ローカルの中心を変換するので、5mm間隔のブロック (ずれ0) は配置点ちょうどのまま。
 */
/** block_caption = "inside" のブロックの置き場所: 描かれた外形 (回転・ミラーの後) の中央。
 * 2行あれば2行のまとまりの中心を、1行だけならその行の大文字の中心を、外形の中心の高さにそろえる。
 * Rust 側 svg::inside_caption_anchor と同じ。 */
function insideCaptionAnchor(inst: SymbolInstance, def: SymbolDef): CaptionAnchor | null {
  if (def.id !== BLOCK_SYMBOL_ID) return null;
  let corners: [Point, Point];
  try {
    const spec = parseBlockSpec(inst.attrs ?? {});
    if (spec.caption !== "inside") return null;
    corners = blockOutline(spec.pins, spec.size, spec.pitch, spec.numbers, spec.pinText);
  } catch {
    return null;
  }
  const c = transformLocal(p((corners[0].x + corners[1].x) / 2, (corners[0].y + corners[1].y) / 2), inst);
  const cap = CAPTION_CAP_RATIO * LABEL_FONT;
  const hasRef = !!inst.reference && inst.attrs?.hide_reference !== "true";
  const hasValue = !!inst.value;
  let top: number;
  if (hasRef && hasValue) top = c.y + (REF_LABEL_DY + cap + VALUE_LABEL_DY) / 2;
  else if (hasRef) top = c.y + cap / 2 + REF_LABEL_DY;
  else top = c.y + cap / 2 + VALUE_LABEL_DY;
  return { x: c.x, top, end: false };
}

function centredCaptionX(inst: SymbolInstance, def: SymbolDef): number {
  if (def.id !== BLOCK_SYMBOL_ID) return inst.at.x;
  let corners: [Point, Point];
  try {
    const { pins, size, pitch, numbers, pinText } = parseBlockSpec(inst.attrs ?? {});
    corners = blockOutline(pins, size, pitch, numbers, pinText);
  } catch {
    return inst.at.x;
  }
  return transformLocal(p((corners[0].x + corners[1].x) / 2, (corners[0].y + corners[1].y) / 2), inst).x;
}

function topLeftCornerAnchor(inst: SymbolInstance, def: SymbolDef): CaptionAnchor | null {
  if (def.id !== BLOCK_SYMBOL_ID) return null;
  let corners: [Point, Point];
  try {
    const { pins, size, pitch, numbers, pinText } = parseBlockSpec(inst.attrs ?? {});
    corners = blockOutline(pins, size, pitch, numbers, pinText);
  } catch {
    return null;
  }
  const a = transformLocal(corners[0], inst);
  const b = transformLocal(corners[1], inst);
  const body = {
    min: { x: Math.min(a.x, b.x), y: Math.min(a.y, b.y) },
    max: { x: Math.max(a.x, b.x), y: Math.max(a.y, b.y) },
  };
  const eps = 1e-6;
  const hasTopPin = def.pins.some((pin) => {
    const t = transformLocal(pin.at, inst);
    return t.y < body.min.y - eps && t.x > body.min.x - eps && t.x < body.max.x + eps;
  });
  if (!hasTopPin) return null;
  const x = body.min.x - CORNER_CAPTION_GAP;
  // 描かれるもの (図形の外接矩形・文字の箱全体・ピングループの括弧)
  const drawn: { min: Point; max: Point }[] = [];
  const addBox = (p: Point, q: Point) =>
    drawn.push({
      min: { x: Math.min(p.x, q.x), y: Math.min(p.y, q.y) },
      max: { x: Math.max(p.x, q.x), y: Math.max(p.y, q.y) },
    });
  for (const prim of def.primitives) {
    let lo: Point;
    let hi: Point;
    switch (prim.type) {
      case "text": {
        const c = transformLocal(prim.at, inst);
        const half = textWidthMm(prim.text, prim.height) / 2;
        addBox({ x: c.x - half, y: c.y - prim.height / 2 }, { x: c.x + half, y: c.y + prim.height / 2 });
        continue;
      }
      case "line": {
        const xs = prim.pts.map((q) => q.x);
        const ys = prim.pts.map((q) => q.y);
        lo = { x: Math.min(...xs), y: Math.min(...ys) };
        hi = { x: Math.max(...xs), y: Math.max(...ys) };
        break;
      }
      case "circle":
      case "arc":
        lo = { x: prim.center.x - prim.r, y: prim.center.y - prim.r };
        hi = { x: prim.center.x + prim.r, y: prim.center.y + prim.r };
        break;
      case "rect":
        lo = { x: Math.min(prim.p1.x, prim.p2.x), y: Math.min(prim.p1.y, prim.p2.y) };
        hi = { x: Math.max(prim.p1.x, prim.p2.x), y: Math.max(prim.p1.y, prim.p2.y) };
        break;
    }
    addBox(transformLocal(lo, inst), transformLocal(hi, inst));
  }
  for (const [lo, hi] of pinGroupPaperBoxes(inst, def)) addBox(lo, hi);
  // 参照記号 (先) と型番の文字の箱 (広いほうの幅)。非表示・空の行は幅に入れない
  let width = 0;
  if (inst.reference && inst.attrs?.hide_reference !== "true") {
    width = Math.max(width, textWidthMm(inst.reference, LABEL_FONT));
  }
  if (inst.value) width = Math.max(width, textWidthMm(inst.value, LABEL_FONT));
  // 上辺の 1.0 mm 上から始め、重なるものの上端が見つかるたびにその上まで上げる
  let top = body.min.y;
  for (let i = 0; i <= drawn.length; i++) {
    const cap = { x0: x - width, x1: x, y0: top - REF_LABEL_DY - LABEL_FONT, y1: top - VALUE_LABEL_DY };
    // 重なりは Rust側 tidy::Rect::overlaps と同じ (CONNECT_EPS = 0.01 mm を超える食い込み)
    const hits = drawn.filter(
      (r) => cap.x0 < r.max.x - 0.01 && r.min.x < cap.x1 - 0.01 && cap.y0 < r.max.y - 0.01 && r.min.y < cap.y1 - 0.01,
    );
    if (hits.length === 0) break;
    top = Math.min(top, ...hits.map((r) => r.min.y));
  }
  return { x, top, end: true };
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
  // 実機ブロック表示 (G7) のコイルは括弧を描かない (ERCが理由を報告する) ので、
  // 描かれない箱のために参照記号・接点マップを押しのけない (Rust側と同一)
  if (isDeviceBlock(inst)) return [];
  const spec = inst.attrs?.[PIN_GROUPS_ATTR];
  if (!spec) return [];
  // 端子番号の上書き (spec §1.1) がある端子台では、範囲は表示ラベルで書かれている
  const rename = terminalLabels(def, inst.attrs).renameMap;
  const out: [Point, Point][] = [];
  for (const bracket of pinGroupBrackets(def, spec, rename)) {
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

function nameWidth(name: string, th: number): number {
  return textWidthMm(name, th);
}

function maxNameWidth(side: BlockPin[], th: number): number {
  return side.reduce((w, p) => Math.max(w, nameWidth(p.caption !== undefined ? p.caption.trim() : (p.name ?? ""), th)), 0);
}

/** 上辺・下辺のピンが外形の内側で使う高さ mm (Rust側 caption_inset)。文字の高さに比例する (2mm で 5 / 7.5mm)。 */
function captionInset(side: BlockPin[], th: number): number {
  const k = th / BLOCK_PIN_TEXT_MM;
  if (side.length === 0) return 0;
  if (connectorGroups(side).some((g) => groupLabel(g) !== null)) return CONNECTOR_LABEL_INSET_MM;
  return side.some((p) => (p.name ?? "") !== "" || p.caption !== undefined) ? 7.5 * k : 5.0 * k;
}

/** 左右の辺の、外形からピン名までの余白 mm (Rust側 side_pad)。 */
function sidePad(side: BlockPin[]): number {
  return side.some((p) => p.connector != null) ? BLOCK_TEXT_PAD_MM + CONNECTOR_IN_MM : BLOCK_TEXT_PAD_MM;
}

/** 左右の辺の内側に書く文字の最大幅 mm (Rust側 side_text_width)。 */
function sideTextWidth(side: BlockPin[], th: number): number {
  let width = maxNameWidth(side, th);
  for (const group of connectorGroups(side)) {
    const label = groupLabel(group);
    if (label === null) continue;
    const labelW = nameWidth(label, th);
    const name = side[group.start].name ?? "";
    const need =
      group.end - group.start >= 2 || name === "" ? labelW : nameWidth(name, th) + CONNECTOR_LABEL_GAP_MM + labelW;
    width = Math.max(width, need);
  }
  return width;
}

function sideOf(pins: BlockPins, key: (typeof BLOCK_SIDES)[number]): BlockPin[] {
  return pins[key] ?? [];
}

/**
 * 向かい合う2辺 (a と b) の貫通ピンの組のうち、2本が同じ線上 (辺に沿ったオフセットが同じ) に並ぶものの
 * [a の添字, b の添字] (Rust側 same_line_pairs)。size はその辺に沿った外形の寸法。
 */
function sameLinePairs(a: BlockPin[], b: BlockPin[], size: number, pitch: BlockPitch): [number, number][] {
  const oa = sideOffsets(a, size, pitch);
  const ob = sideOffsets(b, size, pitch);
  const pairs: [number, number][] = [];
  a.forEach((pa, i) => {
    // 貫通ピンは番号のあるピンだけ (すき間・見出し・番号の無いピンは組にならない)
    if (!isBlockPin(pa) || !isNumbered(pa)) return;
    const j = b.findIndex((pb) => isBlockPin(pb) && isNumbered(pb) && (pb.no ?? "").trim() === (pa.no ?? "").trim());
    if (j >= 0 && Math.abs(oa[i] - ob[j]) < 1e-9) pairs.push([i, j]);
  });
  return pairs;
}

/**
 * 番号を内側に描くとき、左右の番号が収まる幅 mm (Rust側 inside_numbers_width)。同じ線上で向かい合う
 * 貫通ピンの組の番号は中央に1つだけ描くので、その最大幅と、残りの左の最大幅+残りの右の最大幅+間
 * (両方あるとき) の大きいほうが収まればよい。h は外形の高さ。
 */
function insideNumbersWidth(pins: BlockPins, h: number, pitch: BlockPitch, th: number): number {
  const left = sideOf(pins, "left");
  const right = sideOf(pins, "right");
  const pairs =
    sideOf(pins, "top").length === 0 && sideOf(pins, "bottom").length === 0 ? sameLinePairs(left, right, h, pitch) : [];
  const rest = (list: BlockPin[], second: boolean): [number, number] => {
    let widest = 0;
    let count = 0;
    list.forEach((pin, i) => {
      if (!isBlockPin(pin) || pairs.some((q) => q[second ? 1 : 0] === i)) return;
      // 番号の無いピンは番号の位置に名前を書く
      widest = Math.max(widest, nameWidth(blockPinKey(pin), th));
      count++;
    });
    return [widest, count];
  };
  const [lw, ln] = rest(left, false);
  const [rw, rn] = rest(right, true);
  const merged = pairs.reduce((m, [i]) => Math.max(m, nameWidth((left[i].no ?? "").trim(), th)), 0);
  const sides = lw + rw + (ln > 0 && rn > 0 ? BLOCK_INSIDE_GAP_MM : 0);
  // 見出しは番号と同じ行には無いが、箱の中に収まる幅は要る
  const caption = [...left, ...right].reduce((m, e) => (e.caption !== undefined ? Math.max(m, nameWidth(e.caption.trim(), th)) : m), 0);
  return 2 * BLOCK_TEXT_PAD_MM + Math.max(merged, sides, caption);
}

/**
 * 属性からピン表と外形寸法を読み取る。Rust側 `parse_block_spec` と同じ検証をし、
 * 同じ理由でエラーを投げる(番号の重複・空番号・ピン過多・寸法不足・壊れたJSON)。
 */
/** ピンの接点を調べる (Rust側 check_contacts と同じ順・同じ文)。 */
function checkContacts(no: string, contacts: string[]): void {
  if (contacts.length === 0) return;
  if (contacts.length > BLOCK_CONTACT_MAX) {
    throw new Error(`block pin ${no}: too many contacts: ${contacts.length} (max ${BLOCK_CONTACT_MAX})`);
  }
  if (contacts.length < 2) {
    throw new Error(`block pin ${no}: a port needs at least two contacts (one conductor is a plain pin)`);
  }
  const seen = new Set<string>();
  for (const raw of contacts) {
    const c = raw.trim();
    if (!c) throw new Error(`block pin ${no}: contact names must not be empty`);
    if (seen.has(c)) throw new Error(`block pin ${no}: duplicate contact ${c}`);
    seen.add(c);
  }
}

/** コネクタの名前を調べる (Rust側 check_connectors と同じ順・同じ文)。 */
function checkConnectors(pins: BlockPins): void {
  for (const side of BLOCK_SIDES) {
    for (const pin of sideOf(pins, side)) {
      if (pin.connector_ref == null) continue;
      const no = blockPinKey(pin);
      if (pin.connector_ref.trim() === "") throw new Error(`block pin ${no}: connector_ref must not be empty`);
      if (pin.connector == null) {
        throw new Error(`block pin ${no}: connector_ref needs a grouped connector (screw, jst_ph_f, jst_xh_f)`);
      }
      if (!isGroupedConnector(pin.connector)) {
        throw new Error(`block pin ${no}: connector_ref is only for screw, jst_ph_f and jst_xh_f, not ${pin.connector}`);
      }
    }
  }
  const seen = new Set<string>();
  for (const side of BLOCK_SIDES) {
    const list = sideOf(pins, side);
    for (const group of connectorGroups(list)) {
      if (group.ref === null) continue;
      if (seen.has(group.ref)) {
        throw new Error(
          `block pin ${blockPinKey(list[group.start])}: connector_ref ${group.ref} is used by another group; its pins must be next to each other on one side`,
        );
      }
      seen.add(group.ref);
    }
  }
}

export function parseBlockSpec(attrs: Record<string, string>): {
  pins: BlockPins;
  size: { w: number; h: number } | null;
  pitch: BlockPitch;
  numbers: BlockNumbers;
  caption: BlockCaption;
  leads: BlockLeads;
  /** ピン名・見出し・番号の文字の高さ mm。属性が無ければnull (今までの BLOCK_PIN_TEXT_MM)。 */
  pinText: number | null;
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
    // Rust では serde が読むときに断る (知らない種類・文字でない名前)
    for (const pin of BLOCK_SIDES.flatMap((s) => sideOf(pins, s))) {
      if (pin.connector != null && !isConnectorKind(pin.connector)) {
        throw new Error(`${BLOCK_PINS_ATTR} is not valid JSON: unknown connector ${String(pin.connector)}`);
      }
      if (pin.connector_ref != null && typeof pin.connector_ref !== "string") {
        throw new Error(`${BLOCK_PINS_ATTR} is not valid JSON: connector_ref must be a string`);
      }
      // Rust では serde が読むときに断る (gap は 0 以上の整数、caption は文字列)
      if (pin.gap != null && (typeof pin.gap !== "number" || !Number.isInteger(pin.gap) || pin.gap < 0)) {
        throw new Error(`${BLOCK_PINS_ATTR} is not valid JSON: gap must be a non-negative integer`);
      }
      if (pin.caption != null && typeof pin.caption !== "string") {
        throw new Error(`${BLOCK_PINS_ATTR} is not valid JSON: caption must be a string`);
      }
    }
  }
  // null は「無い」と同じ (Rust の Option)。以降は undefined だけを見る
  for (const pin of BLOCK_SIDES.flatMap((s) => sideOf(pins, s))) {
    if (pin.gap === null) delete pin.gap;
    if (pin.caption === null) delete pin.caption;
  }
  const all = BLOCK_SIDES.flatMap((s) => sideOf(pins, s)).filter(isBlockPin);
  if (all.length > BLOCK_PIN_MAX) {
    throw new Error(`block has too many pins: ${all.length} (max ${BLOCK_PIN_MAX})`);
  }
  // 同じ番号は、向かい合う辺に1回ずつだけ置ける (貫通ピン、Rust側 parse_block_spec と同じ決まり)。辺は左・右・上・下の順
  const seen = new Map<string, { side: number; pin: BlockPin; paired: boolean }>();
  BLOCK_SIDES.forEach((key, side) => {
    for (const pin of sideOf(pins, key)) {
      // すき間・見出しの行は、他の項目を混ぜず・範囲内なら、ピンの決まりの外
      if (!isBlockPin(pin)) {
        checkEntry(pin);
        continue;
      }
      // 番号が無いピンは名前が鍵。どちらも無ければ断る
      const no = blockPinKey(pin);
      if (!no) throw new Error("block pin number must not be empty");
      const first = seen.get(no);
      if (!first) {
        seen.set(no, { side, pin, paired: false });
      } else {
        // 番号の無いピンは貫通できない
        if (!isNumbered(pin) || !isNumbered(first.pin)) throw new Error(`duplicate block pin name: ${no}`);
        if (first.paired || Math.floor(first.side / 2) !== Math.floor(side / 2) || first.side === side) {
          throw new Error(`duplicate block pin number: ${no}`);
        }
        if ((first.pin.name ?? "").trim() !== (pin.name ?? "").trim()) {
          throw new Error(`block pin ${no}: both sides of a feed-through pin must have the same name`);
        }
        const plain = (q: BlockPin) => (q.contacts ?? []).length === 0 && q.connector == null;
        if (!plain(first.pin) || !plain(pin)) {
          throw new Error(`block pin ${no}: a feed-through pin cannot have contacts or a connector picture`);
        }
        first.paired = true;
      }
      checkContacts(no, pin.contacts ?? []);
    }
  });
  // 番号の無いピンの名前は、番号付きのピンの名前とも重ならない (Rust側 parse_block_spec と同じ)
  const numberedNames = new Set(all.filter(isNumbered).map((p) => (p.name ?? "").trim()));
  const clash = all.find((p) => !isNumbered(p) && numberedNames.has(blockPinKey(p)));
  if (clash) throw new Error(`duplicate block pin name: ${blockPinKey(clash)}`);
  checkConnectors(pins);
  const rowCount = BLOCK_SIDES.reduce((n, k) => n + sideRows(sideOf(pins, k)), 0);
  if (rowCount > BLOCK_ROW_MAX) {
    throw new Error(`block has too many rows: ${rowCount} (max ${BLOCK_ROW_MAX})`);
  }

  const rawPitch = attrs[BLOCK_PITCH_ATTR]?.trim();
  let pitch: BlockPitch;
  if (rawPitch === undefined || rawPitch === "5") pitch = "coarse";
  else if (rawPitch === "2.5") pitch = "fine";
  else throw new Error(`${BLOCK_PITCH_ATTR} must be "5" or "2.5": ${JSON.stringify(rawPitch)}`);
  const rawNumbers = attrs[BLOCK_NUMBERS_ATTR]?.trim();
  let numbers: BlockNumbers;
  if (rawNumbers === undefined || rawNumbers === "outside") numbers = "outside";
  else if (rawNumbers === "inside") numbers = "inside";
  else throw new Error(`${BLOCK_NUMBERS_ATTR} must be "outside" or "inside": ${JSON.stringify(rawNumbers)}`);
  const rawCaption = attrs[BLOCK_CAPTION_ATTR]?.trim();
  let caption: BlockCaption;
  if (rawCaption === undefined || rawCaption === "above") caption = "above";
  else if (rawCaption === "inside") caption = "inside";
  else throw new Error(`${BLOCK_CAPTION_ATTR} must be "above" or "inside": ${JSON.stringify(rawCaption)}`);
  const rawPinText = attrs[BLOCK_PIN_TEXT_ATTR]?.trim();
  let pinText: number | null = null;
  if (rawPinText !== undefined) {
    const v = rawPinText === "" ? NaN : Number(rawPinText);
    if (!Number.isFinite(v) || v < BLOCK_PIN_TEXT_MIN_MM || v > BLOCK_PIN_TEXT_MAX_MM) {
      throw new Error(
        `${BLOCK_PIN_TEXT_ATTR} must be between ${BLOCK_PIN_TEXT_MIN_MM.toFixed(1)} and ${BLOCK_PIN_TEXT_MAX_MM.toFixed(1)} mm: ${JSON.stringify(rawPinText)}`,
      );
    }
    pinText = v;
  }
  if (pitch === "fine") {
    const pin = all.find((q) => q.connector != null);
    if (pin) {
      throw new Error(
        `block pin ${blockPinKey(pin)}: a connector picture cannot be used with ${BLOCK_PITCH_ATTR} 2.5`,
      );
    }
  }
  // コネクタのピンは番号を絵の外に描き、内側の番号の幅も絵の深さを数えないので、組み合わせない (Rust側と同じ)
  if (numbers === "inside") {
    const pin = all.find((q) => q.connector != null);
    if (pin) {
      throw new Error(
        `block pin ${blockPinKey(pin)}: a connector picture cannot be used with ${BLOCK_NUMBERS_ATTR} inside`,
      );
    }
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
    size = { w: snapUp(w, blockSizeStep(pitch)), h: snapUp(h, blockSizeStep(pitch)) };
    const rows = Math.max(sideRows(sideOf(pins, "left")), sideRows(sideOf(pins, "right")));
    const cols = Math.max(sideRows(sideOf(pins, "top")), sideRows(sideOf(pins, "bottom")));
    const needW = sideNeed(cols, pitch);
    const needH = sideNeed(rows, pitch);
    if (size.w < needW - 1e-9 || size.h < needH - 1e-9) {
      throw new Error(
        `block_size ${size.w}x${size.h}mm is too small for its pins (needs at least ${needW}x${needH}mm)`,
      );
    }
  }
  const rawLeads = attrs[BLOCK_LEADS_ATTR]?.trim();
  let leads: BlockLeads;
  if (rawLeads === undefined || rawLeads === "lead") leads = "lead";
  else if (rawLeads === "none") leads = "none";
  else throw new Error(`${BLOCK_LEADS_ATTR} must be "lead" or "none": ${JSON.stringify(rawLeads)}`);
  if (leads === "none" && numbers !== "inside") {
    throw new Error(`${BLOCK_LEADS_ATTR} none needs ${BLOCK_NUMBERS_ATTR} inside`);
  }
  return { pins, size, pitch, numbers, caption, leads, pinText };
}

/** すき間・見出しの行を調べる (Rust側 check_entry と同じ順・同じ文)。 */
function checkEntry(e: BlockPin): void {
  const mixed =
    (e.gap !== undefined && e.caption !== undefined) ||
    isNumbered(e) ||
    (e.name ?? "").trim() !== "" ||
    (e.contacts ?? []).length > 0 ||
    e.connector != null ||
    e.connector_ref != null;
  if (mixed) throw new Error("block gap or caption entry cannot have other fields");
  if (e.gap !== undefined && (e.gap < 1 || e.gap > BLOCK_GAP_MAX)) {
    throw new Error(`block gap must be 1 to ${BLOCK_GAP_MAX} rows: ${e.gap}`);
  }
  if (e.caption !== undefined && e.caption.trim() === "") throw new Error("block caption must not be blank");
}

/** 左右の辺の内側に書く文字の1つ分: 縦の位置・外形の内壁からの張り出し・まとまりの名前か (Rust側 SideText)。 */
interface SideText {
  y: number;
  extent: number;
  label: boolean;
}

/** 左右の辺の内側に書く文字 (ピン名とまとまりの名前) の位置 (Rust側 side_texts。pinLayout / labelAt と同じ式)。 */
function sideTexts(side: BlockPin[], th: number): SideText[] {
  const offs = coarseOffsets(side);
  const nameIn = BLOCK_TEXT_PAD_MM + CONNECTOR_IN_MM;
  const out: SideText[] = [];
  side.forEach((pin, i) => {
    if (pin.caption !== undefined) {
      out.push({ y: offs[i], extent: BLOCK_TEXT_PAD_MM + nameWidth(pin.caption.trim(), th), label: false });
      return;
    }
    const name = pin.name ?? "";
    if (name === "") return;
    const start = pin.connector != null ? nameIn : BLOCK_TEXT_PAD_MM;
    out.push({ y: offs[i], extent: start + nameWidth(name, th), label: false });
  });
  for (const group of connectorGroups(side)) {
    const label = groupLabel(group);
    if (label === null) continue;
    const first = offs[group.start];
    const count = group.end - group.start;
    let y = first;
    let start = nameIn;
    if (count >= 2) {
      y = first + BLOCK_PIN_PITCH_MM / 2 + BLOCK_PIN_PITCH_MM * Math.floor((count - 1) / 2);
    } else {
      const name = side[group.start].name ?? "";
      if (name !== "") start = nameIn + nameWidth(name, th) + CONNECTOR_LABEL_GAP_MM;
    }
    out.push({ y, extent: start + nameWidth(label, th), label: true });
  }
  return out;
}

/**
 * 左右の辺の文字が同じ行 (文字の高さの範囲が重なる) に並び、そのどちらかがまとまりの名前のとき、
 * 左の文字・文字の間隔・右の文字が収まる幅 mm。そういう行が無ければ0 (Rust側 shared_row_width)。
 */
function sharedRowWidth(pins: BlockPins, th: number): number {
  const left = sideTexts(sideOf(pins, "left"), th);
  const right = sideTexts(sideOf(pins, "right"), th);
  let need = 0;
  for (const l of left) {
    for (const r of right) {
      if ((l.label || r.label) && Math.abs(l.y - r.y) < th - 1e-9) {
        need = Math.max(need, l.extent + CONNECTOR_LABEL_GAP_MM + r.extent);
      }
    }
  }
  return need;
}

/** 番号を外に描くとき、左右のピン名・見出しが向かい合って収まる幅 mm (Rust側 outside_text_width)。 */
function outsideTextWidth(pins: BlockPins, pitchMm: number, th: number): number {
  const left = sideOf(pins, "left");
  const right = sideOf(pins, "right");
  return sidePad(left) + sidePad(right) + sideTextWidth(left, th) + sideTextWidth(right, th) + pitchMm;
}

/**
 * ブロックの外形寸法 mm (Rust側 `block_box` と同じ)。`pinText` はピン名・見出し・番号の文字の高さ
 * (block_pin_text。nullなら今までの BLOCK_PIN_TEXT_MM)。
 */
export function blockBox(
  pins: BlockPins,
  size: { w: number; h: number } | null,
  pitch: BlockPitch = "coarse",
  numbers: BlockNumbers = "outside",
  pinText: number | null = null,
): [number, number] {
  const step = blockSizeStep(pitch);
  const th = pinText ?? BLOCK_PIN_TEXT_MM;
  if (size) {
    // 指定の幅でも、左右のまとまりの名前が同じ行で重なるなら、収まる幅まで広げる
    let w = Math.max(size.w, snapUp(sharedRowWidth(pins, th), step));
    if (numbers === "inside") {
      w = Math.max(w, snapUp(insideNumbersWidth(pins, size.h, pitch, th), step));
    } else if (pinText !== null) {
      // 文字の大きさを決めたブロックは、手動の幅でも向かい合う名前がぶつからない幅まで広げる
      w = Math.max(w, snapUp(outsideTextWidth(pins, blockPitchMm(pitch), th), step));
    }
    return [w, size.h];
  }
  const left = sideOf(pins, "left");
  const right = sideOf(pins, "right");
  const top = sideOf(pins, "top");
  const bottom = sideOf(pins, "bottom");
  const rows = Math.max(sideRows(left), sideRows(right));
  const cols = Math.max(sideRows(top), sideRows(bottom));
  const pm = blockPitchMm(pitch);
  const minW = pitch === "fine" ? BLOCK_FINE_MIN_MM : BLOCK_MIN_W_MM;
  const minH = pitch === "fine" ? BLOCK_FINE_MIN_MM : BLOCK_MIN_H_MM;
  const needCols = pitch === "fine" ? sideNeed(cols, pitch) : cols * pm + pm;
  const needRows = pitch === "fine" ? sideNeed(rows, pitch) : rows * pm + pm;
  const h = snapUp(Math.max(minH, needRows, captionInset(top, th) + captionInset(bottom, th) + pm), step);
  const wText =
    numbers === "inside" ? insideNumbersWidth(pins, h, pitch, th) : outsideTextWidth(pins, pm, th);
  const w = Math.max(minW, wText, needCols);
  return [snapUp(w, step), h];
}

/**
 * 描かれる外形の2つの角 (シンボルのローカル座標。Rust側 block_outline)。2.5mm間隔で大きさが
 * 2.5mmの奇数倍の軸は 1.25mm ずれる。外形を知りたいコードは blockBox から作らずこれを使う。
 */
export function blockOutline(
  pins: BlockPins,
  size: { w: number; h: number } | null,
  pitch: BlockPitch = "coarse",
  numbers: BlockNumbers = "outside",
  pinText: number | null = null,
): [Point, Point] {
  const [w, h] = blockBox(pins, size, pitch, numbers, pinText);
  const sx = axisShift(w, pitch);
  const sy = axisShift(h, pitch);
  return [p(-w / 2 + sx, -h / 2 + sy), p(w / 2 + sx, h / 2 + sy)];
}

type BlockDir = "left" | "right" | "up" | "down";

/** ピンが外形と交わる点 (Rust側 edge_point)。 */
function edgePoint(dir: BlockDir, off: number, hw: number, hh: number): Point {
  if (dir === "left") return p(-hw, off);
  if (dir === "right") return p(hw, off);
  if (dir === "up") return p(off, -hh);
  return p(off, hh);
}

/** コネクタのまとまりの名前の位置 (Rust側 label_at と同じ式)。 */
function labelAt(
  side: BlockPin[],
  offs: number[],
  group: ConnectorGroup,
  label: string,
  dir: BlockDir,
  hw: number,
  hh: number,
  pitchMm: number,
  th: number,
): Point {
  const first = offs[group.start];
  const labelW = nameWidth(label, th);
  const nameIn = BLOCK_TEXT_PAD_MM + CONNECTOR_IN_MM;
  if (dir === "left" || dir === "right") {
    const count = group.end - group.start;
    let y = first;
    let start = nameIn;
    if (count >= 2) {
      y = first + pitchMm / 2 + pitchMm * Math.floor((count - 1) / 2);
    } else {
      const name = side[group.start].name ?? "";
      if (name !== "") start = nameIn + nameWidth(name, th) + CONNECTOR_LABEL_GAP_MM;
    }
    return p(dir === "left" ? -hw + start + labelW / 2 : hw - start - labelW / 2, y);
  }
  const mid = (first + offs[group.end - 1]) / 2;
  return p(mid, dir === "up" ? -hh + CONNECTOR_LABEL_TOP_MM : hh - CONNECTOR_LABEL_TOP_MM);
}

/** 見出しの行の文字の中心 (Rust側 caption_at)。左辺は左寄せ・右辺は右寄せ、上下辺はその列の名前の行。 */
function captionAt(text: string, off: number, dir: BlockDir, hw: number, hh: number, pitchMm: number, th: number): Point {
  const w = nameWidth(text, th);
  if (dir === "left") return p(-hw + BLOCK_TEXT_PAD_MM + w / 2, off);
  if (dir === "right") return p(hw - BLOCK_TEXT_PAD_MM - w / 2, off);
  if (dir === "up") return p(off, -hh + th + pitchMm / 2);
  return p(off, hh - th - pitchMm / 2);
}

/** 1本のピンの描き位置 (Rust側 pin_layout と同じ式)。 */
function pinLayout(
  bp: BlockPin,
  off: number,
  dir: BlockDir,
  hw: number,
  hh: number,
  numbers: BlockNumbers = "outside",
  pitchMm: number = BLOCK_PIN_PITCH_MM,
  leadMm: number = BLOCK_LEAD_MM,
  th: number = BLOCK_PIN_TEXT_MM,
): { at: Point; leadFrom: Point; numAt: Point; nameAt: Point } {
  const leadFrom = edgePoint(dir, off, hw, hh);
  const name = bp.name ?? "";
  if (bp.connector == null) {
    const inside = numbers === "inside";
    const numW = nameWidth(blockPinKey(bp), th);
    if (dir === "left") {
      return {
        at: p(-hw - leadMm, off),
        leadFrom,
        numAt: inside
          ? p(-hw + BLOCK_TEXT_PAD_MM + numW / 2, off)
          : p(-hw - leadMm / 2, off - th),
        nameAt: p(-hw + BLOCK_TEXT_PAD_MM + nameWidth(name, th) / 2, off),
      };
    }
    if (dir === "right") {
      return {
        at: p(hw + leadMm, off),
        leadFrom,
        numAt: inside
          ? p(hw - BLOCK_TEXT_PAD_MM - numW / 2, off)
          : p(hw + leadMm / 2, off - th),
        nameAt: p(hw - BLOCK_TEXT_PAD_MM - nameWidth(name, th) / 2, off),
      };
    }
    if (dir === "up") {
      return {
        at: p(off, -hh - leadMm),
        leadFrom,
        numAt: p(off, -hh + th),
        nameAt: p(off, -hh + th + pitchMm / 2),
      };
    }
    return {
      at: p(off, hh + leadMm),
      leadFrom,
      numAt: p(off, hh - th),
      nameAt: p(off, hh - th - pitchMm / 2),
    };
  }
  const numW = nameWidth(blockPinKey(bp), th);
  const nameW = nameWidth(name, th);
  const numOut = CONNECTOR_OUT_MM + CONNECTOR_NUM_GAP_MM;
  const nameIn = BLOCK_TEXT_PAD_MM + CONNECTOR_IN_MM;
  const numTb = (CONNECTOR_OUT_MM + CONNECTOR_LEAD_MM) / 2;
  if (dir === "left") {
    return {
      at: p(-hw - CONNECTOR_LEAD_MM, off),
      leadFrom: p(-hw - CONNECTOR_OUT_MM, off),
      numAt: p(-(hw + numOut + numW / 2), off - th),
      nameAt: p(-hw + nameIn + nameW / 2, off),
    };
  }
  if (dir === "right") {
    return {
      at: p(hw + CONNECTOR_LEAD_MM, off),
      leadFrom: p(hw + CONNECTOR_OUT_MM, off),
      numAt: p(hw + numOut + numW / 2, off - th),
      nameAt: p(hw - nameIn - nameW / 2, off),
    };
  }
  if (dir === "up") {
    return {
      at: p(off, -hh - CONNECTOR_LEAD_MM),
      leadFrom: p(off, -hh - CONNECTOR_OUT_MM),
      numAt: p(off + CONNECTOR_NUM_SIDE_MM + numW / 2, -(hh + numTb)),
      nameAt: p(off, -hh + th + pitchMm / 2),
    };
  }
  return {
    at: p(off, hh + CONNECTOR_LEAD_MM),
    leadFrom: p(off, hh + CONNECTOR_OUT_MM),
    numAt: p(off + CONNECTOR_NUM_SIDE_MM + numW / 2, hh + numTb),
    nameAt: p(off, hh - th - pitchMm / 2),
  };
}

/** 図形のすべての点を (dx, dy) だけ動かした写しを返す (Rust側 translate_primitive)。図形の種類を増やしたらここが型エラーになる。 */
function translatePrimitive(prim: Primitive, dx: number, dy: number): Primitive {
  const mv = (pt: Point): Point => p(pt.x + dx, pt.y + dy);
  switch (prim.type) {
    case "line":
      return { ...prim, pts: prim.pts.map(mv) };
    case "circle":
      return { ...prim, center: mv(prim.center) };
    case "arc":
      return { ...prim, center: mv(prim.center) };
    case "rect":
      return { ...prim, p1: mv(prim.p1), p2: mv(prim.p2) };
    case "text":
      return { ...prim, at: mv(prim.at) };
    default: {
      const unreachable: never = prim;
      return unreachable;
    }
  }
}

/**
 * 属性からブロックのシンボル定義を作る(Rust側 `block_symbol` と同一座標)。
 * 外形は原点中心の長方形。ピンは辺ごとに中央揃え・5mmピッチで、
 * ピン番号は外形の外側、ピン名は内側に描く(上下辺は番号・名前とも内側)。
 * block_pitch = "2.5" ならピン間隔2.5mm・大きさの刻み2.5mmで、大きさが2.5mmの奇数倍の軸は外形を1.25mm (右・下へ)、
 * グリッドから外れるピンの列も1.25mmずらす (axisShift・rowOffsets)。
 * block_numbers = "inside" なら左右の番号を外形の内側に描き、ピン名は描かない。
 */
export function blockSymbol(attrs: Record<string, string>): SymbolDef {
  const { pins: table, size, pitch, numbers, leads, pinText } = parseBlockSpec(attrs);
  const leadMm = blockLeadMm(leads);
  const th = pinText ?? BLOCK_PIN_TEXT_MM;
  const [w, h] = blockBox(table, size, pitch, numbers, pinText);
  const hw = w / 2;
  const hh = h / 2;
  const sx = axisShift(w, pitch);
  const sy = axisShift(h, pitch);
  const pitchMm = blockPitchMm(pitch);
  // コネクタの無いブロックは今までどおり長方形1つ。あれば絵の所を空けた線分 (Rust側と同じ)
  const hasConnectors = BLOCK_SIDES.some((s) => sideOf(table, s).some((pin) => pin.connector != null));
  const primitives: Primitive[] = hasConnectors
    ? outlineSegments(table, hw, hh)
    : [{ type: "rect", p1: p(-hw, -hh), p2: p(hw, hh), filled: false }];
  const pins: PinDef[] = [];
  // 番号を内側に描くとき、同じ線上で向かい合う貫通ピンの組は番号を1つだけ外形の中央に描く (組の最初のピンが描き、2本目は描かない)
  // 交わる辺 (左右の組なら上下、上下の組なら左右) にピンがあると中央の1つが他の番号と重なるので、そのときはまとめない
  const lrFree = sideOf(table, "top").length === 0 && sideOf(table, "bottom").length === 0;
  const tbFree = sideOf(table, "left").length === 0 && sideOf(table, "right").length === 0;
  const lrPairs =
    numbers === "inside" && lrFree ? sameLinePairs(sideOf(table, "left"), sideOf(table, "right"), h, pitch) : [];
  const tbPairs =
    numbers === "inside" && tbFree ? sameLinePairs(sideOf(table, "top"), sideOf(table, "bottom"), w, pitch) : [];
  const push = (side: BlockPin[], dir: BlockDir) => {
    const n = side.length;
    const offs = sideOffsets(side, dir === "left" || dir === "right" ? h : w, pitch);
    const groups = connectorGroups(side);
    for (let i = 0; i < n; i++) {
      const bp = side[i];
      // すき間は何も描かない。見出しは箱の内側のその行に太字で書く (接続点は無い)
      if (!isBlockPin(bp)) {
        if (bp.caption !== undefined) {
          const caption = bp.caption.trim();
          primitives.push({ type: "text", at: captionAt(caption, offs[i], dir, hw, hh, pitchMm, th), text: caption, height: th, bold: true });
        }
        continue;
      }
      const no = blockPinKey(bp);
      const name = bp.name ?? "";
      // まとまりの最初のピンの前に、そのまとまりの絵と名前を描く
      const group = groups.find((g) => g.start === i);
      if (group) {
        const edges: Point[] = [];
        for (let j = group.start; j < group.end; j++) edges.push(edgePoint(dir, offs[j], hw, hh));
        primitives.push(...connectorPrimitives(group.kind, dir, edges));
        const label = groupLabel(group);
        if (label !== null) {
          primitives.push({ type: "text", at: labelAt(side, offs, group, label, dir, hw, hh, pitchMm, th), text: label, height: th });
        }
      }
      const { at, leadFrom, numAt, nameAt } = pinLayout(bp, offs[i], dir, hw, hh, numbers, pitchMm, leadMm, th);
      // リード線なしのブロックは線を描かない (ピン接続点が外形の辺の上)
      if (Math.abs(leadFrom.x - at.x) > 1e-9 || Math.abs(leadFrom.y - at.y) > 1e-9) {
        primitives.push({ type: "line", pts: [leadFrom, at] });
      }
      const vertical = dir === "left" || dir === "right";
      const pairs = vertical ? lrPairs : tbPairs;
      const isFirst = (dir === "left" || dir === "up") && pairs.some((q) => q[0] === i);
      const isSecond = (dir === "right" || dir === "down") && pairs.some((q) => q[1] === i);
      if (isFirst) {
        primitives.push({ type: "text", at: vertical ? p(0, offs[i]) : p(offs[i], 0), text: no, height: th });
      } else if (!isSecond && (isNumbered(bp) || numbers === "inside")) {
        // 番号の無いピンは、外には何も書かない。番号を内側に描くブロックでは番号の位置に名前を書く
        primitives.push({ type: "text", at: numAt, text: no, height: th });
      }
      if (name && numbers === "outside") {
        primitives.push({ type: "text", at: nameAt, text: name, height: th });
      }
      const contacts = (bp.contacts ?? []).map((c) => c.trim());
      pins.push(contacts.length ? { number: no, name, at, dir, contacts } : { number: no, name, at, dir });
    }
  };
  push(sideOf(table, "left"), "left");
  push(sideOf(table, "right"), "right");
  push(sideOf(table, "top"), "up");
  push(sideOf(table, "bottom"), "down");

  let text_slots: TextSlot[] = BLOCK_SLOT_KEYS.map((key, i) => ({
    key,
    at: p(0, (i - 1.5) * BLOCK_SLOT_PITCH_MM),
    height: 2.5,
  }));
  // 外形がグリッドに載るようにずらす (Rust側と同じ: 図形・ピン・文字の置き場所をまとめて動かす)
  if (sx !== 0 || sy !== 0) {
    for (let i = 0; i < primitives.length; i++) primitives[i] = translatePrimitive(primitives[i], sx, sy);
    for (const pin of pins) pin.at = p(pin.at.x + sx, pin.at.y + sy);
    text_slots = text_slots.map((slot) => ({ ...slot, at: p(slot.at.x + sx, slot.at.y + sy) }));
  }
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
