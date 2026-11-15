// 参照記号・型番の置き場所を部品ごとに決める属性 `caption_layout`。
// Rust 側 madake-core/src/caption.rs と同じ規則: JSON `{"ref": {...}, "value": {...}}`、
// 行は x / y (配置点からのずれ mm、用紙の向き。y はベースライン)、h (文字サイズ)、anchor、hide。

export const CAPTION_LAYOUT_ATTR = "caption_layout";
export const MIN_CAPTION_H_MM = 0.5;
export const MAX_CAPTION_H_MM = 20.0;

export interface CaptionLine {
  x: number;
  y: number;
  h: number | null;
  anchor: "start" | "middle" | "end";
  hide: boolean;
}

export interface CaptionLayout {
  ref: CaptionLine | null;
  value: CaptionLine | null;
}

const LINE_KEYS = new Set(["x", "y", "h", "anchor", "hide"]);

function parseLine(name: string, raw: unknown): CaptionLine | null {
  if (raw === undefined || raw === null) return null;
  if (typeof raw !== "object" || Array.isArray(raw)) throw new Error(`${CAPTION_LAYOUT_ATTR} is not valid JSON: ${name} must be an object`);
  const o = raw as Record<string, unknown>;
  for (const k of Object.keys(o)) {
    if (!LINE_KEYS.has(k)) throw new Error(`${CAPTION_LAYOUT_ATTR} is not valid JSON: unknown field ${k}`);
  }
  const num = (k: string): number => {
    const v = o[k];
    if (v === undefined) return 0;
    if (typeof v !== "number") throw new Error(`${CAPTION_LAYOUT_ATTR} is not valid JSON: ${name}.${k} must be a number`);
    return v;
  };
  const x = num("x");
  const y = num("y");
  if (!Number.isFinite(x) || !Number.isFinite(y)) throw new Error(`${CAPTION_LAYOUT_ATTR}.${name}: x and y must be finite numbers`);
  let h: number | null = null;
  if (o.h !== undefined && o.h !== null) {
    if (typeof o.h !== "number") throw new Error(`${CAPTION_LAYOUT_ATTR} is not valid JSON: ${name}.h must be a number`);
    if (!Number.isFinite(o.h) || o.h < MIN_CAPTION_H_MM || o.h > MAX_CAPTION_H_MM) {
      throw new Error(`${CAPTION_LAYOUT_ATTR}.${name}: h out of range: ${o.h} (expected ${MIN_CAPTION_H_MM}..${MAX_CAPTION_H_MM} mm)`);
    }
    h = o.h;
  }
  const anchor = o.anchor === undefined ? "middle" : o.anchor;
  if (anchor !== "start" && anchor !== "middle" && anchor !== "end") {
    throw new Error(`${CAPTION_LAYOUT_ATTR} is not valid JSON: ${name}.anchor must be start, middle or end`);
  }
  if (o.hide !== undefined && typeof o.hide !== "boolean") {
    throw new Error(`${CAPTION_LAYOUT_ATTR} is not valid JSON: ${name}.hide must be true or false`);
  }
  return { x, y, h, anchor, hide: o.hide === true };
}

/** 属性から置き場所を読む。属性が無い・空なら null。読めなければ例外 (メッセージは属性名を含む)。 */
export function parseCaptionLayout(attrs: Record<string, string> | null | undefined): CaptionLayout | null {
  const raw = attrs?.[CAPTION_LAYOUT_ATTR];
  if (raw === undefined || !raw.trim()) return null;
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (e) {
    throw new Error(`${CAPTION_LAYOUT_ATTR} is not valid JSON: ${String(e)}`);
  }
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
    throw new Error(`${CAPTION_LAYOUT_ATTR} is not valid JSON: expected an object`);
  }
  const o = parsed as Record<string, unknown>;
  for (const k of Object.keys(o)) {
    if (k !== "ref" && k !== "value") throw new Error(`${CAPTION_LAYOUT_ATTR} is not valid JSON: unknown field ${k}`);
  }
  return { ref: parseLine("ref", o.ref), value: parseLine("value", o.value) };
}

/** 読める置き場所だけを返す (描画用。壊れた属性は無いものとして扱う)。 */
export function captionLayoutOf(attrs: Record<string, string> | null | undefined): CaptionLayout | null {
  try {
    return parseCaptionLayout(attrs);
  } catch {
    return null;
  }
}
