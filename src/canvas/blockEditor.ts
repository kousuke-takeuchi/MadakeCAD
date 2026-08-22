// ブロック(汎用機器)のピン表エディタの下ごしらえ。
//
// 「行の表」と、図面が持つ属性 (`block_pins` / `block_size`) の相互変換だけをここに置く。
// 実際に描かれる形は `canvas/dynamicSymbol.ts` の `blockSymbol` (Rust側 madake-core/src/block.rs と対)
// がピン表から作るので、プレビューも配置後の図も同じ関数を通る。
//
// 図面への反映はこのモジュールでは行わない (必ずCommand経由。stores/blockEditor.ts)。

import {
  BLOCK_PINS_ATTR,
  BLOCK_PIN_MAX,
  BLOCK_PIN_PITCH_MM,
  BLOCK_SIZE_ATTR,
  blockBox,
  blockSymbol,
  parseBlockSpec,
  type BlockPin,
  type BlockPins,
} from "./dynamicSymbol";
import { transformLocal } from "./renderer";
import type { Point, Sheet, SymbolDef, SymbolInstance } from "../ipc";

/** ピンを並べられる辺 (左右は上→下、上下は左→右)。 */
export type BlockSide = "left" | "right" | "top" | "bottom";

/** 辺の並び順 (エディタの表示順)。左右が主で、上下は折りたたみの中。 */
export const BLOCK_SIDES: readonly BlockSide[] = ["left", "right", "top", "bottom"];

/** 上下辺 (エディタでは折りたたみの中に出す)。 */
export const BLOCK_EXTRA_SIDES: readonly BlockSide[] = ["top", "bottom"];

/** 外形寸法 (mm)。 */
export interface BlockSize {
  w: number;
  h: number;
}

/** ピン表の1行。 */
export interface BlockPinRow {
  no: string;
  name: string;
}

/** 編集中のピン表。`size`がnullなら外形はピンから自動で決まる。 */
export interface BlockDraft {
  rows: Record<BlockSide, BlockPinRow[]>;
  size: BlockSize | null;
}

/** ピン表の指摘 (これがある間は配置・適用できない)。 */
export type BlockIssue =
  /** 同じピン番号が2つ以上ある。 */
  | { code: "duplicate"; no: string }
  /** ピン名だけ書いて番号が空。 */
  | { code: "emptyNo"; side: BlockSide; index: number }
  /** ピンが多すぎる。 */
  | { code: "tooMany"; count: number; max: number }
  /** 手動の外形がピンの並びより小さい。 */
  | { code: "sizeTooSmall"; w: number; h: number };

/** 配線が接続点に載っているとみなす距離 (mm)。Rust側 netlist::CONNECT_EPS と同じ。 */
const CONNECT_EPS = 0.01;

function emptyRows(): Record<BlockSide, BlockPinRow[]> {
  return { left: [], right: [], top: [], bottom: [] };
}

/** 新しいブロックの下書き (左右に1本ずつ・外形は自動)。 */
export function blankDraft(): BlockDraft {
  return {
    rows: { ...emptyRows(), left: [{ no: "1", name: "" }], right: [{ no: "2", name: "" }] },
    size: null,
  };
}

/** 表に並んでいる全ピン (辺の順)。 */
function allRows(draft: BlockDraft): BlockPinRow[] {
  return BLOCK_SIDES.flatMap((side) => draft.rows[side]);
}

/** 書き出す価値のある行か (番号もピン名も空の行はピンにしない)。 */
function isFilled(row: BlockPinRow): boolean {
  return row.no.trim() !== "" || row.name.trim() !== "";
}

/**
 * 次に足すピンの番号。数字の番号が1つでもあればその最大値+1、
 * (`A0`のような)数字でない番号ばかりならピンの本数+1。
 */
export function nextPinNo(draft: BlockDraft): string {
  const rows = allRows(draft).filter(isFilled);
  let max = 0;
  for (const row of rows) {
    const n = Number.parseInt(row.no.trim(), 10);
    if (Number.isFinite(n)) max = Math.max(max, n);
  }
  return String((max || rows.length) + 1);
}

/** 辺の末尾へピンを1本足す (番号は自動)。 */
export function addRow(draft: BlockDraft, side: BlockSide): void {
  draft.rows[side].push({ no: nextPinNo(draft), name: "" });
}

/** 辺の`index`行目を消す。 */
export function removeRow(draft: BlockDraft, side: BlockSide, index: number): void {
  draft.rows[side].splice(index, 1);
}

/** 辺の`index`行目を`delta`行ぶん動かす (端の行は動かない)。 */
export function moveRow(draft: BlockDraft, side: BlockSide, index: number, delta: number): void {
  const rows = draft.rows[side];
  const to = index + delta;
  if (index < 0 || index >= rows.length || to < 0 || to >= rows.length) return;
  const [row] = rows.splice(index, 1);
  rows.splice(to, 0, row);
}

/** 下書きをRust側と同じピン表 (`BlockPins`) へ整える。 */
function pinsOf(draft: BlockDraft): BlockPins {
  const pins: BlockPins = {};
  for (const side of BLOCK_SIDES) {
    const rows = draft.rows[side]
      .filter(isFilled)
      .map((row): BlockPin => {
        const name = row.name.trim();
        return name ? { no: row.no.trim(), name } : { no: row.no.trim() };
      });
    if (rows.length) pins[side] = rows;
  }
  return pins;
}

/**
 * 図面に持たせる属性。ピンのある辺だけを書き、外形は手動指定のときだけ書く
 * (自動のときは属性を置かない = ピンから毎回決まる)。
 */
export function attrsFromDraft(draft: BlockDraft): Record<string, string> {
  const attrs: Record<string, string> = {};
  const pins = pinsOf(draft);
  if (BLOCK_SIDES.some((side) => pins[side]?.length)) {
    attrs[BLOCK_PINS_ATTR] = JSON.stringify(pins);
  }
  if (draft.size) attrs[BLOCK_SIZE_ATTR] = JSON.stringify(draft.size);
  return attrs;
}

/** 配置済みブロックの属性を下書きへ読み戻す。読めない属性は空のピン表として開く。 */
export function draftFromAttrs(attrs: Record<string, string>): BlockDraft {
  let pins: BlockPins = {};
  let size: BlockSize | null = null;
  try {
    const spec = parseBlockSpec(attrs);
    pins = spec.pins;
    size = spec.size;
  } catch {
    return { rows: emptyRows(), size: null };
  }
  const rows = emptyRows();
  for (const side of BLOCK_SIDES) {
    rows[side] = (pins[side] ?? []).map((p) => ({ no: p.no ?? "", name: p.name ?? "" }));
  }
  return { rows, size };
}

/** いまの外形寸法 (手動ならその値、自動ならピンから決まる寸法)。 */
export function draftSize(draft: BlockDraft): BlockSize {
  if (draft.size) return { ...draft.size };
  const [w, h] = blockBox(pinsOf(draft), null);
  return { w, h };
}

/** プレビュー用のシンボル定義 (配置後に描かれるものと同一)。指摘があるときはnull。 */
export function draftSymbol(draft: BlockDraft): SymbolDef | null {
  try {
    return blockSymbol(attrsFromDraft(draft));
  } catch {
    return null;
  }
}

/** ピン表の指摘 (重複番号・番号なし・ピン過多・外形不足)。 */
export function draftIssues(draft: BlockDraft): BlockIssue[] {
  const issues: BlockIssue[] = [];
  const seen = new Set<string>();
  for (const side of BLOCK_SIDES) {
    draft.rows[side].forEach((row, index) => {
      if (!isFilled(row)) return;
      const no = row.no.trim();
      if (!no) {
        issues.push({ code: "emptyNo", side, index });
        return;
      }
      if (seen.has(no)) issues.push({ code: "duplicate", no });
      seen.add(no);
    });
  }
  const count = allRows(draft).filter(isFilled).length;
  if (count > BLOCK_PIN_MAX) issues.push({ code: "tooMany", count, max: BLOCK_PIN_MAX });
  if (draft.size) {
    const pins = pinsOf(draft);
    const rows = Math.max(pins.left?.length ?? 0, pins.right?.length ?? 0);
    const cols = Math.max(pins.top?.length ?? 0, pins.bottom?.length ?? 0);
    const w = Math.max(cols * BLOCK_PIN_PITCH_MM, BLOCK_PIN_PITCH_MM);
    const h = Math.max(rows * BLOCK_PIN_PITCH_MM, BLOCK_PIN_PITCH_MM);
    if (draft.size.w < w - 1e-9 || draft.size.h < h - 1e-9) {
      issues.push({ code: "sizeTooSmall", w, h });
    }
  }
  return issues;
}

function segmentDistance(p: Point, a: Point, b: Point): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const len2 = dx * dx + dy * dy;
  if (len2 < 1e-12) return Math.hypot(p.x - a.x, p.y - a.y);
  const t = Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / len2));
  return Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy));
}

/**
 * いま配線が届いているピンの番号 (図面の見たままで判定する。Rust側のネットリストと
 * 同じく、接続点に配線が触れていれば接続とみなす)。
 */
export function wiredPinNumbers(sheet: Sheet, inst: SymbolInstance, def: SymbolDef): string[] {
  const wires = Object.values(sheet.entities).filter((e) => e.kind === "wire");
  const wired: string[] = [];
  for (const pin of def.pins) {
    if (wired.includes(pin.number)) continue;
    const at = transformLocal(pin.at, inst);
    const touched = wires.some((w) =>
      w.kind === "wire" && w.points.length === 1
        ? Math.hypot(w.points[0].x - at.x, w.points[0].y - at.y) < CONNECT_EPS
        : w.kind === "wire" &&
          w.points.some(
            (_, i) => i + 1 < w.points.length && segmentDistance(at, w.points[i], w.points[i + 1]) < CONNECT_EPS,
          ),
    );
    if (touched) wired.push(pin.number);
  }
  return wired;
}

/**
 * 編集後のピン表から消えてしまう配線済みピン (接続が外れる = 警告を出す)。
 * 番号を書き換えた場合も、そのピンは消えたものとして挙がる。
 */
export function droppedWiredPins(wired: readonly string[], draft: BlockDraft): string[] {
  const kept = new Set(allRows(draft).filter(isFilled).map((row) => row.no.trim()));
  return wired.filter((no) => !kept.has(no));
}
