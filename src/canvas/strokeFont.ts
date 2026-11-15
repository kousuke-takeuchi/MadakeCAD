// ストロークフォント (線で描く字) の配置と、キャンバスでいま有効な書体。
//
// フォントの解析はRust側 (madake-core/src/stroke_font.rs) だけが行い、字形はIPC / Link APIで受け取る
// (`ipc.getStrokeFont`)。ここは配置の規則だけを持つ。規則と数値はRust側の`StrokeFont::layout`と同じ。
//
// 葉モジュール (frameTemplate.ts・textSize.ts の両方から読まれる) なので、DOMにもストアにも依存しない。

import type { Latin, StrokeFontData } from "../ipc";
import { textWidthMm } from "./textWidth";

/** 半角の大文字の高さ ÷ フォントサイズ (図枠の`text_size_is: cap`と同じ比)。 */
export const CAP_HEIGHT_RATIO = 0.72;
/** 半角・全角の字の送り (格子の単位)。 */
export const HALF_ADVANCE = 15;
export const FULL_ADVANCE = 30;
/** 線の太さ ÷ フォントサイズ、線の太さの下限 (mm)、太字の倍率。 */
const STROKE_RATIO = 0.07;
const STROKE_MIN = 0.13;
const BOLD_FACTOR = 1.5;

/** 配置した文字列。原点はベースラインの書き出し位置、y は下向き。 */
export interface StrokeText {
  polylines: [number, number][][];
  /** 文字列の幅 (最後の字の後ろの字間は含めない)。 */
  width: number;
}

/** 文字サイズに応じた線の太さ。 */
export function strokeWidth(size: number, bold: boolean): number {
  return Math.max(STROKE_RATIO * size, STROKE_MIN) * (bold ? BOLD_FACTOR : 1);
}

/** 英数字を全角の字で描くときの文字 (空白は全角の空白)。 */
function fullWidth(c: string): string {
  const u = c.codePointAt(0) ?? 0;
  if (u === 0x20) return "　";
  if (u >= 0x21 && u <= 0x7e) return String.fromCodePoint(u - 0x21 + 0xff01);
  return c;
}

/**
 * 文字列を配置する。フォントに無い字が1つでもあれば null (呼び出し側はシステムの書体で描く)。
 * `spacing` は字の送りに掛ける倍率 (字の間だけを広げる)。
 */
export function layoutStroke(
  font: StrokeFontData,
  text: string,
  size: number,
  latin: Latin,
  spacing = 1,
): StrokeText | null {
  const unit = (CAP_HEIGHT_RATIO * size) / Math.max(1, font.cap_top - font.baseline);
  const polylines: [number, number][][] = [];
  let pen = 0;
  let last = 0;
  for (const c of text) {
    const glyph = (latin === "full" ? font.glyphs[fullWidth(c)] : undefined) ?? font.glyphs[c];
    if (!glyph) return null;
    for (const s of glyph.strokes) {
      const line: [number, number][] = [];
      for (let i = 0; i + 1 < s.length; i += 2) {
        line.push([pen + s[i] * unit, -(s[i + 1] - font.baseline) * unit]);
      }
      polylines.push(line);
    }
    last = (glyph.wide ? FULL_ADVANCE : HALF_ADVANCE) * unit;
    pen += last * spacing;
  }
  return { polylines, width: text === "" ? 0 : pen - last * (spacing - 1) };
}

/** いまキャンバスで有効な書体 (プロジェクトの`text_font`。未設定・読み込み前は null)。 */
let active: { font: StrokeFontData; latin: Latin } | null = null;

/** 有効な書体を切り替える (null でシステムの書体へ戻す)。キャンバス用の配置のキャッシュも捨てる。 */
export function setActiveStrokeFont(font: StrokeFontData | null, latin: Latin): void {
  active = font ? { font, latin } : null;
  unitCache.clear();
}

/**
 * キャンバス用に、大きさ1で配置した文字列 (描くときは文字の大きさ倍に拡大する)。
 * `lines` は線ごとの x, y の並び、`descent` はベースラインから字の格子の下端までの高さ。
 * `path` は Path2D が使える環境でだけ作る (テストの node には無い)。
 */
export interface UnitStroke {
  width: number;
  descent: number;
  lines: number[][];
  path: Path2D | null;
}

/** キャッシュに残す文字列の数の上限 (超えたら古いものから捨てる)。 */
export const UNIT_CACHE_MAX = 4096;
const unitCache = new Map<string, UnitStroke | null>();

/** いまキャッシュにある文字列の数 (テスト用)。 */
export function unitStrokeCacheSize(): number {
  return unitCache.size;
}

/**
 * 有効な書体で文字列を大きさ1で配置する (書体・英数字の字・字間・文字列ごとにキャッシュする)。
 * 書体が無い・フォントに無い字を含むときは null。
 */
export function activeUnitStroke(text: string, spacing = 1): UnitStroke | null {
  if (!active) return null;
  const key = `${active.font.id}\u0000${active.latin}\u0000${spacing}\u0000${text}`;
  const hit = unitCache.get(key);
  if (hit !== undefined) {
    // 最近使ったものを後ろへ (上限を超えたとき先頭の古いものから捨てる)
    unitCache.delete(key);
    unitCache.set(key, hit);
    return hit;
  }
  const t = layoutStroke(active.font, text, 1, active.latin, spacing);
  let entry: UnitStroke | null = null;
  if (t) {
    const unit = CAP_HEIGHT_RATIO / Math.max(1, active.font.cap_top - active.font.baseline);
    const lines = t.polylines.map((l) => l.flat());
    let path: Path2D | null = null;
    if (typeof Path2D !== "undefined" && lines.length > 0) {
      path = new Path2D();
      for (const l of lines) {
        for (let i = 0; i + 1 < l.length; i += 2) {
          if (i === 0) path.moveTo(l[i], l[i + 1]);
          else path.lineTo(l[i], l[i + 1]);
        }
      }
    }
    entry = { width: t.width, descent: active.font.baseline * unit, lines, path };
  }
  unitCache.set(key, entry);
  if (unitCache.size > UNIT_CACHE_MAX) {
    const oldest = unitCache.keys().next().value;
    if (oldest !== undefined) unitCache.delete(oldest);
  }
  return entry;
}

export function activeStrokeFont(): { font: StrokeFontData; latin: Latin } | null {
  return active;
}

/** 有効な書体があれば、その間だけ英数字の字 (半角・全角) を上書きする (図枠テンプレートの`text_latin`)。 */
export function withLatin<T>(latin: Latin | null | undefined, fn: () => T): T {
  if (!active || !latin) return fn();
  const previous = active;
  active = { ...active, latin };
  try {
    return fn();
  } finally {
    active = previous;
  }
}

/**
 * 文字列の幅 (mm)。有効な書体がありその字が全てあれば線の幅、そうでなければ見積もりの幅
 * (`textWidthMm`。システムの書体では字間は効かない)。Rust側`svg::text_advance`と同じ。
 */
export function textAdvanceMm(text: string, size: number, spacing = 1): number {
  const t = active ? layoutStroke(active.font, text, size, active.latin, spacing) : null;
  return t ? t.width : textWidthMm(text, size);
}

/**
 * `fn` の間だけシステムの書体で描く (プレビューのキャンバス用。spec: 図枠テンプレート等のプレビューは
 * システムの書体のまま)。終わると (例外でも) 元の書体へ戻す。キャッシュは書体ごとなので捨てない。
 */
export function withSystemFont<T>(fn: () => T): T {
  const previous = active;
  active = null;
  try {
    return fn();
  } finally {
    active = previous;
  }
}

/**
 * 字形の取得を1つのidにつき1回にまとめる関数を作る。失敗は null を返して覚えない
 * (セッションの間ずっとシステムの書体のままにせず、次に読み込むときに取り直す)。
 */
export function strokeFontLoader(
  fetch: (id: string) => Promise<StrokeFontData>,
): (id: string) => Promise<StrokeFontData | null> {
  const loaded = new Map<string, Promise<StrokeFontData | null>>();
  return (id) => {
    let pending = loaded.get(id);
    if (!pending) {
      pending = fetch(id).catch(() => {
        loaded.delete(id);
        return null;
      });
      loaded.set(id, pending);
    }
    return pending;
  };
}
